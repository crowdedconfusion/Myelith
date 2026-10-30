//! ⚑ **Beweiser und Prüfer kommen unabhängig zur selben Spur.**
//!
//! Das ist der Test, auf den Stufe 2 hinausläuft. `myl-pod` rechnet die
//! Spur eines Shards, wie er es im Betrieb tut; `myl_verifier::ModellAuditor`
//! rechnet sie **auf eigenem Weg** nach. Beide teilen nur den
//! Spur-Vertrag `myl_types::uebergang::activation_hash`.
//!
//! **Ohne diesen Test wäre der Nachrechner eine Vermutung.** Er könnte
//! richtig aussehen und systematisch etwas anderes rechnen; dann
//! beschuldigte Stufe 2 ehrliche Miner, und zwar alle.

mod artefakte;

use std::path::PathBuf;
use std::sync::Arc;

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use myl_types::hash::Hash;
use myl_types::ids::SegmentId;
use myl_verifier::{check_segment, CheckResult, ModellAuditor, SegmentAuditor};

fn artifacts_dir() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let mut p = PathBuf::from(manifest);
    p.push("..");
    p.push("..");
    p.push("INTEGER_LLM");
    p.push("artifacts");
    p.push(std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "sparrow-0.6b-int8".to_string()));
    p
}

/// Die Spur, wie der Shard sie erzeugt: ein Hash je Layer.
fn spur_des_beweisers(
    modell: &integer_llm_runtime::model::IntegerModel,
    ein: &[i16],
    pos: usize,
    von: usize,
    bis: usize,
) -> Vec<Hash> {
    let mut hidden = ein.to_vec();
    let mut cache = KVCache::new(modell.num_layers, modell.num_kv_heads);
    let mut spur = Vec::new();
    for i in von..bis {
        hidden = modell.run_layers(hidden, pos, &mut cache, i, i + 1);
        spur.push(Hash(myl_pod::trace::activation_hash(&hidden)));
    }
    spur
}

fn eingabe(modell: &integer_llm_runtime::model::IntegerModel) -> Vec<i16> {
    (0..modell.hidden_size)
        .map(|i| ((i * 13 + 7) % 61) as i16 - 30)
        .collect()
}

#[test]
fn beweiser_und_pruefer_kommen_zur_selben_spur() {
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let (von, bis, pos) = (0usize, 4usize.min(modell.num_layers), 0usize);
    let ein = eingabe(&modell);

    let erwartet = spur_des_beweisers(&modell, &ein, pos, von, bis);
    assert_eq!(erwartet.len(), bis - von, "je Layer ein Eintrag");

    let roh: Vec<u8> = ein.iter().flat_map(|v| v.to_le_bytes()).collect();
    let auditor = ModellAuditor::neu(Arc::clone(&modell), von, bis, pos).expect("Bereich");
    assert_eq!(
        check_segment(&auditor, SegmentId::new([1; 32]), &roh, &erwartet).expect("Nachrechnen"),
        CheckResult::Valid
    );
}

/// ⚑ **Und eine verfälschte Spur wird an der richtigen Stelle gefunden.**
///
/// Die Bisektion beginnt bei der ersten Abweichung; meldete der
/// Nachrechner eine falsche Position, stritte die Schiedsrunde über die
/// falsche Layer.
#[test]
fn eine_verfaelschte_spur_wird_an_der_richtigen_stelle_gefunden() {
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let (von, bis, pos) = (0usize, 4usize.min(modell.num_layers), 0usize);
    if bis - von < 3 {
        return;
    }
    let ein = eingabe(&modell);
    let mut spur = spur_des_beweisers(&modell, &ein, pos, von, bis);
    spur[2] = Hash::sha256(b"gefaelscht");

    let roh: Vec<u8> = ein.iter().flat_map(|v| v.to_le_bytes()).collect();
    let auditor = ModellAuditor::neu(Arc::clone(&modell), von, bis, pos).expect("Bereich");
    assert_eq!(
        check_segment(&auditor, SegmentId::new([1; 32]), &roh, &spur).expect("Nachrechnen"),
        CheckResult::Invalid { first_divergence: 2 }
    );
}

/// Ein leerer oder verdrehter Bereich ergibt keinen Auditor.
///
/// ⚑ **Ein leerer Bereich ergäbe eine leere Spur, und die verglände sich
/// mit allem.**
#[test]
fn ein_unmoeglicher_bereich_ergibt_keinen_auditor() {
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    assert!(ModellAuditor::neu(Arc::clone(&modell), 2, 2, 0).is_none(), "leer");
    assert!(ModellAuditor::neu(Arc::clone(&modell), 3, 1, 0).is_none(), "verdreht");
    assert!(
        ModellAuditor::neu(Arc::clone(&modell), 0, modell.num_layers + 1, 0).is_none(),
        "ueber das Modell hinaus"
    );
}

/// ⚑ **Eine ungerade Byte-Zahl ist kein halber Wert.**
///
/// Still abzuschneiden hieße, über eine andere Eingabe zu rechnen als
/// der Beschuldigte, und die Abweichung fiele ihm zur Last.
#[test]
fn eine_ungerade_eingabe_wird_abgewiesen() {
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let auditor = ModellAuditor::neu(Arc::clone(&modell), 0, 1, 0).expect("Bereich");
    assert!(auditor.audit_segment(SegmentId::new([1; 32]), &[1, 2, 3]).is_err());
}

/// **Hinter Position null lehnt der Nachrechner ab, statt ohne Verlauf
/// zu rechnen** (Fund 373).
///
/// Die Gegenprobe steht daneben: Ein Shard, der die Positionen davor
/// gerechnet hat, kommt an Position eins zu einer anderen Spur als ein
/// leerer Speicher an derselben Stelle. Genau diese Spur haette der
/// Nachrechner verlangt.
#[test]
fn hinter_position_null_lehnt_der_nachrechner_ab() {
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let ein = eingabe(&modell);
    let bytes: Vec<u8> = ein.iter().flat_map(|v| v.to_le_bytes()).collect();
    let auditor = ModellAuditor::neu(Arc::clone(&modell), 0, 2, 1).expect("Bereich");
    assert_eq!(
        auditor.audit_segment(SegmentId::new([1; 32]), &bytes),
        Err(myl_verifier::CheckError::OhneVerlauf { position: 1 })
    );

    // Zwei Verlaeufe fuer Position eins, die sich nur in Position null
    // unterscheiden. ⚠️ Mit derselben Eingabe an beiden Positionen waeren
    // auch die Values gleich, und jede Gewichtung ergaebe dasselbe.
    let spur_bei_eins = |vorher: &[i16]| {
        let mut cache = KVCache::new(modell.num_layers, modell.num_kv_heads);
        modell.run_layers(vorher.to_vec(), 0, &mut cache, 0, 2);
        modell.run_layers(ein.clone(), 1, &mut cache, 0, 2)
    };
    let anders: Vec<i16> = ein.iter().map(|v| v.wrapping_mul(3).wrapping_sub(11)).collect();
    assert!(
        spur_bei_eins(&ein) != spur_bei_eins(&anders),
        "Position eins haengt an Position null, also braucht der Nachrechner den Verlauf"
    );
    let ohne = std::panic::catch_unwind(|| {
        let mut leer = KVCache::new(modell.num_layers, modell.num_kv_heads);
        modell.run_layers(ein.clone(), 1, &mut leer, 0, 1)
    });
    assert!(ohne.is_err(), "ein leerer Speicher an Position eins ist eine Luecke");
}

/// Ein Pod aus vier Shards im Prozess, wie in `pod_e2e.rs`.
fn vier_shards(modell: Arc<integer_llm_runtime::model::IntegerModel>, neu: u64) -> Vec<Arc<myl_pod::shard::ShardNode>> {
    let l = modell.num_layers;
    let je = l / 4;
    let grenzen = [0usize, je, 2 * je, 3 * je, l];
    (0..4)
        .map(|s| {
            let sk = myl_types::bls::BlsSecretKey::key_gen(&[(s as u8 + 1) * 17; 32]).expect("BLS KeyGen");
            Arc::new(myl_pod::shard::ShardNode::new(s, grenzen[s], grenzen[s + 1], s == 0, s == 3, modell.clone(), sk, neu))
        })
        .collect()
}

/// ⚑ **T6c, Stufe 2 von Anfang bis Ende** (2026-10-29): Ein Pod rechnet
/// zwei Sitzungen, legt die Spuren mit den Tokens ab wie das Podwerk, und
/// ein Checker holt zum letzten Segment einer Sitzung den Verlauf, prüft
/// ihn gegen die Wurzel und rechnet ihn **ab den Tokens** auf eigenem Weg
/// nach. Ehrlich ergibt das Gleichheit an jeder Position und Layer; eine
/// gefälschte Layer wird genau dort gefunden, ein anderes Token an seiner
/// Position.
#[test]
fn ein_checker_rechnet_den_verlauf_ab_den_tokens_nach() {
    use myl_types::ids::{EpochId, MerkleRoot, PodId};
    use myl_verifier::{verlauf_nachrechnen, Verlaufsergebnis};
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let neu = 4u64;
    let mut koordinator =
        myl_pod::coordinator::Coordinator::new(PodId::new([0xAA; 32]), EpochId(0), vier_shards(modell.clone(), neu), myl_pod::coordinator::DEFAULT_WINDOW_MS);
    assert!(!koordinator.run_prompt(1, &[9_707, 11, 1_879, 374], neu).is_empty(), "Sitzung 1");
    assert!(!koordinator.run_prompt(2, &[785, 6_722], neu).is_empty(), "Sitzung 2");

    // Die Ablage, wie `podwerk::spuren_ablegen` sie baut.
    let segmente = koordinator.completed_segments();
    let eintraege: Vec<(myl_types::Segmentzeugnis, Vec<Hash>)> = segmente
        .iter()
        .map(|c| (myl_types::Segmentzeugnis { id: c.id, spurwurzel: c.spurwurzel }, c.kette().into_iter().map(Hash).collect()))
        .collect();
    let zeugnisse: Vec<_> = eintraege.iter().map(|(z, _)| *z).collect();
    let wurzel: MerkleRoot = myl_types::segments_root(&zeugnisse).expect("Wurzel");
    let ablage = myl_types::Spurablage {
        epoche: EpochId(0),
        pod: PodId::new([0xAA; 32]),
        segments_root: wurzel,
        eintraege,
        eingaben: segmente.iter().map(|c| c.eingabe.clone()).collect(),
    };
    let n = segmente.len() as u32;

    // Gezogen: das letzte Segment der ersten Sitzung.
    let gezogen = segmente
        .iter()
        .enumerate()
        .filter(|(_, c)| myl_types::sitzung_und_position(&c.id).0 == 1)
        .max_by_key(|(_, c)| c.position)
        .map(|(i, _)| i as u32)
        .expect("Sitzung 1 hat Segmente");
    let frage = myl_types::Spuranfrage { epoche: EpochId(0), pod: PodId::new([0xAA; 32]), segment: gezogen };
    let verlauf = ablage.verlauf(&frage).expect("Verlauf");
    let tokens = myl_types::verlauf_pruefen(&verlauf, &wurzel, n).expect("der Verlauf trägt");
    assert!(tokens.len() > 4, "die vier Prompt-Tokens und mindestens ein erzeugtes: {}", tokens.len());
    assert_eq!(&tokens[..4], &[9_707, 11, 1_879, 374], "die Tokens sind die des Prompts");
    let ketten: Vec<Vec<Hash>> = verlauf.iter().map(|a| a.spur.clone()).collect();
    let ehrlich = verlauf_nachrechnen(&modell, &tokens, &ketten).expect("nachgerechnet");
    assert_eq!(ehrlich.ergebnis, Verlaufsergebnis::Gleich, "ein ehrlicher Pod");

    // ⛔️ Gegenproben: eine gefälschte Layer an Position 2 wird dort gefunden;
    //    ein anderes Token an Position 1 dort, an der ersten Layer.
    let mut gefaelscht = ketten.clone();
    gefaelscht[2][6] = Hash::sha256(b"gefaelscht");
    let falsch = verlauf_nachrechnen(&modell, &tokens, &gefaelscht).expect("nachgerechnet");
    assert_eq!(falsch.ergebnis, Verlaufsergebnis::Abweichung { position: 2, eintrag: 6 });
    // ⚑ Die Logits sind die eigenen: Eine gefälschte Spur ändert sie nicht,
    //   denn der Checker rechnet mit seinen Werten weiter.
    assert_eq!(falsch.logits, ehrlich.logits, "die Logits hängen nicht an der Spur des Pods");

    // ⚑ **Die Schiedsrunde** (T6d, Whitepaper 6.6): Der Anfechter bringt die
    //   Eingänge der Layer 5 für die Positionen 0 bis 2; das Komitee rechnet
    //   nur diese Layer nach. Gegen die gefälschte Kette rechnete der Pod
    //   falsch, gegen die ehrliche nicht; ein verfälschter Beleg trägt nicht
    //   und fällt auf den Anfechter.
    let beleg = myl_verifier::schiedsbeleg(&modell, &tokens, 2, 5).expect("Beleg");
    assert_eq!(beleg.len(), 3);
    assert_eq!(myl_verifier::schied_nachrechnen(&modell, &tokens, &gefaelscht, 2, 6, &beleg), Ok(true), "der Pod rechnete falsch");
    assert_eq!(myl_verifier::schied_nachrechnen(&modell, &tokens, &ketten, 2, 6, &beleg), Ok(false), "der Pod rechnete richtig");
    let mut schief = beleg.clone();
    schief[1][0] = schief[1][0].wrapping_add(1);
    assert_eq!(myl_verifier::schied_nachrechnen(&modell, &tokens, &gefaelscht, 2, 6, &schief), Ok(false), "der Beleg trägt nicht");
    // An der ersten Layer braucht es keinen Beleg: Die Eingänge sind die
    // Einbettungen der Tokens.
    let mut erste = ketten.clone();
    erste[1][1] = Hash::sha256(b"gefaelscht");
    assert_eq!(myl_verifier::schiedsbeleg(&modell, &tokens, 1, 0), Ok(Vec::new()));
    assert_eq!(myl_verifier::schied_nachrechnen(&modell, &tokens, &erste, 1, 1, &[]), Ok(true));
    assert_eq!(myl_verifier::schied_nachrechnen(&modell, &tokens, &ketten, 1, 1, &[]), Ok(false));
    let mut anders = tokens.clone();
    anders[1] = 12;
    let anderes = verlauf_nachrechnen(&modell, &anders, &ketten).expect("nachgerechnet");
    assert_eq!(anderes.ergebnis, Verlaufsergebnis::Abweichung { position: 1, eintrag: 1 });
    assert_ne!(anderes.logits, ehrlich.logits, "andere Tokens, andere Logits");
    // Ein Token hinter dem Vokabular ist kein Befund gegen den Pod, sondern
    // ein unbrauchbarer Verlauf.
    let mut hinter = tokens.clone();
    hinter[0] = modell.vocab_size as u32;
    assert!(verlauf_nachrechnen(&modell, &hinter, &ketten).is_err());
}

/// ⚑ **Die Konformitätsprobe für den Prefill** (Frage 5 vom 2026-10-29):
/// Erst wenn der Prefill bitgleich zum schrittweisen Rechnen ist, darf der
/// Nachrechner ihn nehmen; ein Unterschied schlüge einen ehrlichen Pod.
/// Die Ketten entstehen hier Position für Position mit gemeinsamem
/// KV-Speicher und dem Hash des Pods, wie ein Shard rechnet, über 520 Token
/// (mehr als ein Vorbereitungsfenster). Der Prefill muss jede Layer jeder
/// Position treffen, dieselben Logits liefern wie der schrittweise Weg, und
/// eine Fälschung hinter der Fenstergrenze an derselben Stelle finden.
#[test]
fn der_prefill_ist_bitgleich_zum_schrittweisen_rechnen() {
    use myl_verifier::{verlauf_als_prefill, verlauf_schrittweise, Verlaufsergebnis};
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let modell = load_model(&dir).expect("Modell-Ladung");
    let n = integer_llm_runtime::model::VORBEREITUNGSFENSTER + 8;
    let tokens: Vec<u32> = (0..n).map(|i| ((i * 7_919 + 13) % modell.vocab_size) as u32).collect();
    let mut cache = KVCache::new(modell.num_layers, modell.num_kv_heads);
    let mut ketten: Vec<Vec<Hash>> = Vec::with_capacity(n);
    for (pos, t) in tokens.iter().enumerate() {
        let mut hidden = modell.embed_token(*t as usize);
        let mut kette = vec![Hash::sha256(b"eingang")];
        for l in 0..modell.num_layers {
            hidden = modell.run_layers(hidden, pos, &mut cache, l, l + 1);
            kette.push(Hash(myl_pod::trace::activation_hash(&hidden)));
        }
        ketten.push(kette);
    }
    let beginn = std::time::Instant::now();
    let prefill = verlauf_als_prefill(&modell, &tokens, &ketten).expect("gerechnet").expect("das Modell kann Prefill");
    let dauer_prefill = beginn.elapsed();
    let beginn = std::time::Instant::now();
    let schritt = verlauf_schrittweise(&modell, &tokens, &ketten).expect("gerechnet");
    let dauer_schritt = beginn.elapsed();
    assert_eq!(prefill.ergebnis, Verlaufsergebnis::Gleich, "der Prefill trifft jede Layer jeder Position");
    assert_eq!(schritt.ergebnis, Verlaufsergebnis::Gleich);
    assert_eq!(prefill.logits, schritt.logits, "dieselben Logits");
    eprintln!("Prefill {:?}, schrittweise {:?} für {n} Positionen", dauer_prefill, dauer_schritt);

    // Hinter der Fenstergrenze gefälscht: beide finden dieselbe Stelle.
    let mut gefaelscht = ketten.clone();
    gefaelscht[n - 3][9] = Hash::sha256(b"gefaelscht");
    gefaelscht[n - 2][2] = Hash::sha256(b"spaeter");
    let a = verlauf_als_prefill(&modell, &tokens, &gefaelscht).expect("gerechnet").expect("Prefill");
    let b = verlauf_schrittweise(&modell, &tokens, &gefaelscht).expect("gerechnet");
    assert_eq!(a.ergebnis, Verlaufsergebnis::Abweichung { position: n - 3, eintrag: 9 });
    assert_eq!(a, b);
}
