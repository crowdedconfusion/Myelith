//! Phase-1-Akzeptanztest: 4-Node-Pod mit deterministischer Token-Ausgabe
//! und Manipulationserkennung.
//!
//! Akzeptanzkriterien (COMPUTE_PIPELINE Phase 1):
//! 1. Der 4-Node-Pod liefert bei wiederholtem identischem Prompt eine
//!    **bitgleiche** Token-Sequenz (Determinismus).
//! 2. Die Pod-Ausgabe ist **bitgleich mit der Einzelknoten-Runtime**
//!    (derselbe rechenkorrekte Forward-Pass, nur anders verteilt).
//!    Gemessen wird das über den **Dekodier-Digest**, also über die
//!    gerechneten Zahlen, und nicht über die erzeugten Token: Bis zum
//!    Abschluss von Fund 36 verglich genau dieser Test Token gegen Token
//!    und trug trotzdem das Wort „bitgleich". Ein Token ist ein Argmax
//!    über `vocab_size` Zahlen und ändert sich erst, wenn deren
//!    Rangfolge kippt; an 0,5B blieb er bei 0,1 % veränderter
//!    Modellbytes unverändert.
//! 3. Die Eingangs-Hash-Prüfung lehnt **manipulierte Aktivierungen**
//!    zuverlässig ab.

use std::path::PathBuf;
use std::sync::Arc;

use integer_llm_runtime::generate::dekodieren_mit_digest;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_pod::coordinator::Coordinator;
use myl_pod::shard::{ShardNode, ShardOut};
use myl_pod::wire::{self, PodMessage};
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};

/// Das Artefaktverzeichnis, standardmäßig Qwen2.5-0,5B.
///
/// ⚑ **Über `MYL_POD_MODELL` wählbar** (2026-08-25). Vorher stand hier
/// ein fester Modellname, und die Zuschnittsinvarianz war damit
/// ausschließlich an einem **dichten** Modell geprüft. Genau diese
/// Eigenschaft ist aber die Zusage, auf die sich ein
/// Mixture-of-Experts-Modell stützen muss: Ein Knoten hält alle Experten
/// seiner Layer, und wenn der Zuschnitt das Ergebnis änderte, wäre der
/// ganze Entwurf hinfällig.
///
/// Beispiel: `MYL_POD_MODELL=qwen3-30b-a3b cargo test --test pod_e2e`
fn artifacts_dir() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "qwen2.5-0.5b".to_string());
    let mut p = PathBuf::from(manifest);
    // COMPUTE_PIPELINE/myl-pod → INTEGER_LLM/artifacts/<modell>
    p.push("..");
    p.push("..");
    p.push("INTEGER_LLM");
    p.push("artifacts");
    p.push(modell);
    p
}

fn build_shards(model: Arc<integer_llm_runtime::model::IntegerModel>, max_tokens: u64) -> Vec<Arc<ShardNode>> {
    let num_layers = model.num_layers;
    let boundaries = [0usize, 6, 12, 18, num_layers];
    let mut shards = Vec::new();
    for s in 0..4 {
        let layer_start = boundaries[s];
        let layer_end = boundaries[s + 1];
        let has_embedding = s == 0;
        let has_lm_head = s == 3;
        let ikm = [(s as u8 + 1) * 17; 32];
        let sk = BlsSecretKey::key_gen(&ikm).expect("BLS KeyGen");
        let shard = ShardNode::new(
            s,
            layer_start,
            layer_end,
            has_embedding,
            has_lm_head,
            model.clone(),
            sk,
            max_tokens,
        );
        shards.push(Arc::new(shard));
    }
    shards
}

const PROMPT: &str = "Die Hauptstadt von Frankreich ist";
const MAX_NEW_TOKENS: u64 = 6;

#[test]
fn pod_deterministisch_und_bitgleich_mit_einzelknoten() {
    let dir = artifacts_dir();
    if !dir.exists() {
        // Artefakte nicht vorhanden (z.B. in CI) — Test ueberspringen
        eprintln!("SKIP: Artefakte fehlen: {:?}", dir);
        return;
    }
    let model = load_model(&dir).expect("Modell-Ladung");
    let tokenizer = Tokenizer::from_file(
        dir.join("tokenizer.json").to_str().expect("Pfad-UTF-8"),
    )
    .expect("Tokenizer-Ladung");

    // Referenz: Einzelknoten-Runtime. `dekodieren_mit_digest` liefert
    // neben den Token den Digest über Logits **und** Token, also den
    // Wert, gegen den der Pod zu halten ist.
    let ref_ids = tokenizer.encode(PROMPT);
    let (ref_out, ref_digest) =
        dekodieren_mit_digest(&model, &ref_ids, MAX_NEW_TOKENS as usize, 0, true);
    let ref_tokens: Vec<u32> = ref_out.iter().map(|t| *t as u32).collect();

    let model = Arc::new(model);
    let shards = build_shards(model.clone(), MAX_NEW_TOKENS);
    let mut coordinator = Coordinator::new(
        PodId::new([0xAA; 32]),
        EpochId(0),
        shards,
        myl_pod::coordinator::DEFAULT_WINDOW_MS,
    );

    let prompt_ids = tokenizer.encode(PROMPT);
    let prompt_tokens: Vec<u32> = prompt_ids.iter().map(|t| *t as u32).collect();

    // Lauf 1.
    let pod_tokens_1 = coordinator.run_prompt(1, &prompt_tokens, MAX_NEW_TOKENS);
    // Lauf 2 (dieselbe Session-Id, derselbe Prompt).
    let pod_tokens_2 = coordinator.run_prompt(2, &prompt_tokens, MAX_NEW_TOKENS);

    let (digest_1, schritte_1) = coordinator
        .dekodier_digest(1)
        .expect("Pod muss einen Dekodier-Digest liefern");
    let (digest_2, schritte_2) = coordinator
        .dekodier_digest(2)
        .expect("Pod muss einen Dekodier-Digest liefern");

    // Akzeptanzkriterium 1: Determinismus, über die Zahlen.
    assert_eq!(
        digest_1, digest_2,
        "zwei Pod-Läufe müssen dieselben Zahlen rechnen, nicht nur dieselben Token"
    );
    assert_eq!(
        pod_tokens_1, pod_tokens_2,
        "zwei Pod-Läufe müssen bitgleiche Token-Sequenzen liefern"
    );

    // Akzeptanzkriterium 2: Bitgleichheit mit dem Einzelknoten.
    //
    // **Die Reihenfolge der beiden Zusicherungen ist Absicht.** Der
    // Digest ist das eigentliche Urteil; der Token-Vergleich steht
    // darunter, weil er das schwächere ist und allein die Aussage
    // „bitgleich" nicht trägt (Fund 36).
    assert_eq!(schritte_1, MAX_NEW_TOKENS as usize, "Schrittzahl des Pods");
    assert_eq!(schritte_2, MAX_NEW_TOKENS as usize, "Schrittzahl des Pods");
    assert_eq!(
        digest_1, ref_digest,
        "Pod und Einzelknoten müssen dieselben Zahlen rechnen, nicht nur dieselben Token"
    );
    assert_eq!(
        pod_tokens_1, ref_tokens,
        "Pod-Ausgabe muss bitgleich mit der Einzelknoten-Runtime sein"
    );
    assert!(!pod_tokens_1.is_empty());
}

/// Der Digest muss auf eine Änderung an den **Zahlen** reagieren, auch
/// wenn die Entscheidung dieselbe bleibt.
///
/// Ohne diese Gegenprobe wäre der neue Vergleichswert nur eine Behauptung:
/// Ein Digest, der immer gleich ist, besteht jeden Gleichheitstest. Statt
/// das Modell zu verändern, wird hier der Digest-Vertrag selbst geprüft,
/// mit einem einzigen um eins verschobenen Logit. Der Token bleibt
/// derselbe, weil die Rangfolge nicht kippt.
#[test]
fn ein_verschobenes_logit_bewegt_den_digest() {
    use integer_llm_runtime::generate::DekodierDigest;

    let logits: Vec<i32> = (0..64).map(|i| 1000 - i).collect();
    let token = 0u32; // Argmax, hier über den Wert 1000.

    let mut a = DekodierDigest::neu();
    a.schritt(&logits, token);

    // Ein Wert weit unterhalb des Argmax um eins verschoben: dieselbe
    // Entscheidung, andere Zahlen.
    let mut verschoben = logits.clone();
    verschoben[40] += 1;
    let mut b = DekodierDigest::neu();
    b.schritt(&verschoben, token);

    assert_ne!(
        a.hex(),
        b.hex(),
        "ein verschobenes Logit muss den Digest bewegen, sonst misst er die \
         Entscheidung statt der Rechnung"
    );

    // Und die Gegenrichtung: derselbe Eingang ergibt denselben Wert.
    let mut c = DekodierDigest::neu();
    c.schritt(&logits, token);
    assert_eq!(a.hex(), c.hex());
    assert_eq!(a.schritte(), 1);
}

#[test]
fn manipulierte_aktivierung_wird_abgelehnt() {
    let dir = artifacts_dir();
    if !dir.exists() {
        // Artefakte nicht vorhanden (z.B. in CI) — Test ueberspringen
        eprintln!("SKIP: Artefakte fehlen: {:?}", dir);
        return;
    }
    let model = load_model(&dir).expect("Modell-Ladung");
    let tokenizer = Tokenizer::from_file(
        dir.join("tokenizer.json").to_str().expect("Pfad-UTF-8"),
    )
    .expect("Tokenizer-Ladung");
    let model = Arc::new(model);
    let shards = build_shards(model.clone(), MAX_NEW_TOKENS);

    // Shard 0 mit einem Prompt-Token füttern, um eine Aktivierung zu erhalten.
    let prompt_ids = tokenizer.encode(PROMPT);
    let first_token = prompt_ids[0] as u32;
    let packed = wire::pack_tokens(&[first_token]);
    let seg = myl_types::ids::SegmentId::new([1u8; 32]);
    let msg = PodMessage::token_input(seg, 7, 0, packed, 0);
    let out = shards[0].process(&msg).expect("Shard 0 verarbeitet Token");
    let forward = match out {
        ShardOut::Forward(next) => next,
        _ => panic!("erwarte Forward von Shard 0"),
    };

    // Unmanipuliert: Shard 1 akzeptiert.
    let ok = shards[1].process(&forward);
    assert!(ok.is_ok(), "unmanipulierte Aktivierung muss akzeptiert werden");

    // Manipuliert: ein Aktivierungs-Byte verfälschen ⇒ Ablehnung.
    let mut tampered = forward.clone();
    tampered.payload[5] = tampered.payload[5].wrapping_add(1);
    let rejected = shards[1].process(&tampered);
    assert!(
        rejected.is_err(),
        "manipulierte Aktivierung muss abgelehnt werden"
    );

    // Auch die Manipulation des Spur-Hashes (ohne Payload-Änderung) muss
    // auffallen: Der Hash der Payload passt dann nicht mehr zur Spur.
    let mut tampered_trace = forward.clone();
    if let Some(last) = tampered_trace.trace.last_mut() {
        last[0] ^= 0xFF;
    }
    let rejected2 = shards[1].process(&tampered_trace);
    assert!(
        rejected2.is_err(),
        "manipulierter Spur-Hash muss abgelehnt werden"
    );
}

/// Die beanspruchte Arbeitsmenge hängt am Modell und an den Token, nicht
/// am Zuschnitt.
///
/// **Das ist die ökonomische Bedingung für variable Knotenzahl je
/// Pipeline.** Zwei Pipelines mit verschiedenem `k` rechnen dasselbe
/// Segment gegeneinander; beanspruchten sie dafür verschieden viel
/// Arbeit, wäre die gemischte Paarung aus dem COMPUTE_PIPELINE-Entwurf
/// nicht neutral, und der billigere Zuschnitt würde sich durchsetzen,
/// ohne besser zu sein.
///
/// Bis zum 2026-08-23 beanspruchte `build_poi_bundle` die **Zahl der
/// Segmente**: ein Bündel über tausend Token dieselbe eine Einheit wie
/// eines über zwei.
#[test]
fn beanspruchte_arbeit_haengt_nicht_am_zuschnitt() {
    let dir = artifacts_dir();
    if !dir.exists() {
        eprintln!("SKIP: Artefakte fehlen: {:?}", dir);
        return;
    }
    let model = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let tokenizer =
        Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad-UTF-8"))
            .expect("Tokenizer-Ladung");
    let prompt_tokens: Vec<u32> = tokenizer.encode(PROMPT).iter().map(|t| *t as u32).collect();

    let vtfe_bei = |k: usize| -> u64 {
        let num_layers = model.num_layers;
        let basis = num_layers / k;
        let rest = num_layers % k;
        let mut grenzen = vec![0usize];
        for s in 0..k {
            let letzte = *grenzen.last().unwrap();
            grenzen.push(letzte + basis + usize::from(s < rest));
        }
        let shards: Vec<Arc<ShardNode>> = (0..k)
            .map(|s| {
                let sk = BlsSecretKey::key_gen(&[(s as u8 + 1).wrapping_mul(17); 32])
                    .expect("BLS KeyGen");
                Arc::new(ShardNode::new(
                    s,
                    grenzen[s],
                    grenzen[s + 1],
                    s == 0,
                    s + 1 == k,
                    model.clone(),
                    sk,
                    MAX_NEW_TOKENS,
                ))
            })
            .collect();
        let mut coordinator = Coordinator::new(
            PodId::new([0xAA; 32]),
            EpochId(0),
            shards,
            myl_pod::coordinator::DEFAULT_WINDOW_MS,
        );
        coordinator.run_prompt(1, &prompt_tokens, MAX_NEW_TOKENS);
        coordinator.beanspruchte_vtfe().expect("vTFE berechenbar")
    };

    let referenz = vtfe_bei(4);

    // **Gezählt werden Vorwärtspässe, nicht erzeugte Token.** Der Prompt
    // hat sieben Token, also sieben Prefill-Positionen; die letzte
    // sampelt das erste Ausgabetoken, danach folgen fünf
    // Feedback-Positionen für die übrigen fünf. Zwölf Vorwärtspässe für
    // sechs Token, und jeder ist ein Token-Forward-Äquivalent.
    //
    // Prefill kostet dieselbe Rechnung wie Decode und wird deshalb
    // vergütet. Vor der Umstellung auf „ein Segment ist eine Position"
    // zählte diese Prüfung sechs statt zwölf und übersah damit die Hälfte
    // der geleisteten Arbeit.
    let paesse = prompt_tokens.len() as u64 + MAX_NEW_TOKENS - 1;
    let voll = paesse * 1_000_000;
    assert!(
        (voll - 24..=voll).contains(&referenz),
        "vier Shards beanspruchen {referenz} statt rund {voll} ({paesse} Vorwärtspässe)"
    );

    for k in [1usize, 2, 3, 6, 8, 12, 24] {
        let andere = vtfe_bei(k);
        assert!(
            referenz.abs_diff(andere) < 24,
            "k={k} beansprucht {andere} gegen {referenz}, mehr als die Abrundung erklärt"
        );
    }
}

/// Ein Shard mit mehr Layern beansprucht mehr, und der letzte bekommt den
/// LM-Kopf dazu.
#[test]
fn der_lm_kopf_zaehlt_beim_letzten_shard_mit() {
    let dir = artifacts_dir();
    if !dir.exists() {
        eprintln!("SKIP: Artefakte fehlen: {:?}", dir);
        return;
    }
    let model = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let shards = build_shards(model.clone(), MAX_NEW_TOKENS);
    let profil = shards[0].modell_profil();

    let erster = myl_tokenomics::vtfe_gutschrift(&profil, &shards[0].zuschnitt(), 100).unwrap();
    let letzter = myl_tokenomics::vtfe_gutschrift(&profil, &shards[3].zuschnitt(), 100).unwrap();

    // Gleich viele Layer (6 von 24), aber der letzte hält den LM-Kopf,
    // und der wiegt bei 0,5B über neun Layer.
    assert_eq!(shards[0].layer_end - shards[0].layer_start, shards[3].layer_end - shards[3].layer_start);
    assert!(
        letzter > erster * 2,
        "letzter Shard {letzter}, erster {erster}: der LM-Kopf muss durchschlagen"
    );
}

/// **Der Punkt des ganzen Umbaus:** Die Spur hängt am Modell, nicht am
/// Zuschnitt.
///
/// Vorher war sie Shard-granular, ihre Länge also gleich `k`. Zwei Pods
/// mit verschiedenem `k` lieferten verschieden lange Spuren, und
/// `myl_verifier::compare_commitments` lehnt das zu Recht mit
/// `LengthMismatch` ab. Genau daran hing der Entwurf für variable
/// Knotenzahl je Pipeline.
#[test]
fn die_spur_haengt_am_modell_nicht_am_zuschnitt() {
    let dir = artifacts_dir();
    if !dir.exists() {
        eprintln!("SKIP: Artefakte fehlen: {:?}", dir);
        return;
    }
    let model = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let tokenizer =
        Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad-UTF-8"))
            .expect("Tokenizer-Ladung");
    let prompt_tokens: Vec<u32> = tokenizer.encode(PROMPT).iter().map(|t| *t as u32).collect();
    let num_layers = model.num_layers;

    let spuren_bei = |k: usize| -> Vec<Vec<[u8; 32]>> {
        let basis = num_layers / k;
        let rest = num_layers % k;
        let mut grenzen = vec![0usize];
        for s in 0..k {
            let letzte = *grenzen.last().unwrap();
            grenzen.push(letzte + basis + usize::from(s < rest));
        }
        let shards: Vec<Arc<ShardNode>> = (0..k)
            .map(|s| {
                let sk = BlsSecretKey::key_gen(&[(s as u8 + 1).wrapping_mul(17); 32])
                    .expect("BLS KeyGen");
                Arc::new(ShardNode::new(
                    s,
                    grenzen[s],
                    grenzen[s + 1],
                    s == 0,
                    s + 1 == k,
                    model.clone(),
                    sk,
                    MAX_NEW_TOKENS,
                ))
            })
            .collect();
        let mut coordinator = Coordinator::new(
            PodId::new([0xAA; 32]),
            EpochId(0),
            shards,
            myl_pod::coordinator::DEFAULT_WINDOW_MS,
        );
        coordinator.run_prompt(1, &prompt_tokens, MAX_NEW_TOKENS);
        coordinator
            .completed_segments()
            .iter()
            .map(|c| c.trace.clone())
            .collect()
    };

    let referenz = spuren_bei(4);

    // Je Segment genau `num_layers` Einträge, einer je Layer.
    for spur in &referenz {
        assert_eq!(
            spur.len(),
            num_layers,
            "eine Spur muss so viele Einträge haben wie das Modell Layer"
        );
    }

    // Und dieselben Einträge, gleich wie fein geschnitten wird.
    for k in [1usize, 2, 3, 6, 8, 12, 24] {
        assert_eq!(
            spuren_bei(k),
            referenz,
            "k={k} liefert eine andere Spur als k=4"
        );
    }
}

/// Jede Position bekommt ihr eigenes Segment und ihr eigenes Archiv.
///
/// **Der Fund vom 2026-08-23:** `DaStore` war mit `(segment_id,
/// shard_index)` verschlüsselt und kannte keine Position, `archive` wurde
/// aber je Position aufgerufen. Jede Position überschrieb die vorige, und
/// am Ende lag nur die letzte im Archiv. Ein Angeklagter hätte die
/// Aktivierung jeder früheren Position nicht liefern können,
/// `adjudicate` hätte `NoResponse` gesehen, und das heißt schuldig:
/// **ein ehrlicher Knoten wäre geslasht worden.**
#[test]
fn jede_position_ist_ein_eigenes_segment_mit_eigenem_archiv() {
    let dir = artifacts_dir();
    if !dir.exists() {
        eprintln!("SKIP: Artefakte fehlen: {:?}", dir);
        return;
    }
    let model = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let tokenizer =
        Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad-UTF-8"))
            .expect("Tokenizer-Ladung");
    let prompt_tokens: Vec<u32> = tokenizer.encode(PROMPT).iter().map(|t| *t as u32).collect();

    let shards = build_shards(model.clone(), MAX_NEW_TOKENS);
    let mut coordinator = Coordinator::new(
        PodId::new([0xAA; 32]),
        EpochId(0),
        shards,
        myl_pod::coordinator::DEFAULT_WINDOW_MS,
    );
    coordinator.run_prompt(1, &prompt_tokens, MAX_NEW_TOKENS);

    let segmente = coordinator.completed_segments();

    // Ein Vorwärtspass je Prompt-Token plus die Feedback-Positionen.
    let erwartet = prompt_tokens.len() + MAX_NEW_TOKENS as usize - 1;
    assert_eq!(segmente.len(), erwartet, "ein Segment je Vorwärtspass");

    // Verschiedene Positionen, verschiedene Ids.
    let ids: std::collections::BTreeSet<_> = segmente.iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), segmente.len(), "Segment-Ids müssen verschieden sein");

    // ⚑ **Und jede Position hat eine eigene Spur** (E10, 2026-08-30).
    //
    // Hier stand bis zum 2026-08-29 eine Prüfung auf das **Archiv**:
    // Jede Position musste ihre Aktivierung noch abrufbar haben. Seit
    // E10 archiviert der Shard nichts mehr, denn die strittige Eingabe
    // bringt im Streitfall der **Ankläger** mit, der das Segment gerade
    // nachgerechnet hat.
    //
    // Was bleibt und was hier geprüft wird, ist die **Spur**: Sie ist
    // die Zusicherung, gegen die geurteilt wird, sie ist je Position
    // eine andere, und sie ist mit 32 Byte je Layer klein genug, um über
    // die Streitfrist zu bleiben.
    let erste = &segmente[0];
    let letzte = &segmente[segmente.len() - 1];
    assert_ne!(
        erste.trace, letzte.trace,
        "zwei Positionen dürfen nicht dieselbe Spur tragen"
    );
    assert_ne!(
        erste.spurwurzel, letzte.spurwurzel,
        "und damit auch nicht dieselbe Zusicherung"
    );

    // ⚑ Die Zusicherung muss zur Kette passen, sonst bezeugt das Bündel
    // etwas anderes als das, was gerechnet wurde (Fund 100).
    assert_eq!(
        erste.spurwurzel,
        myl_types::spurwurzel(&erste.kette()).expect("Wurzel"),
        "die Zusicherung muss die Wurzel über die eigene Kette sein"
    );

    // ⚑ **Und die Kette beginnt beim Eingang** (Fund 102). Ohne diesen
    // ersten Eintrag hinge die Eingabe der **ersten** Layer an nichts,
    // und die Schiedsrunde prüfte dort einen Hash des Anklägers gegen
    // einen zweiten Hash desselben Anklägers.
    assert_eq!(erste.kette().len(), erste.trace.len() + 1);
    assert_eq!(erste.kette()[0], erste.eingangs_commitment);
    assert_eq!(erste.kette()[1], erste.trace[0]);
    assert_ne!(
        erste.eingangs_commitment, erste.trace[0],
        "Eingang und erste Ausgabe dürfen nicht derselbe Wert sein"
    );
    assert_ne!(
        erste.eingangs_commitment, letzte.eingangs_commitment,
        "zwei Positionen haben verschiedene Eingaben"
    );

    // ⚑ **Der Protokoll-Beleg ist eine Projektion, keine zweite
    // Aufzeichnung.** `myl_types::Segment` beschreibt ihn seit dem
    // 2026-08-13, erzeugt hat ihn bis zum 2026-08-30 niemand, und die
    // beiden führten verschiedene Felder: Genau daraus entstand
    // Fund 102.
    let modell = myl_types::ids::MerkleRoot::new([3u8; 32]);
    let beleg = erste.zu_segment(modell);
    assert_eq!(beleg.id, erste.id);
    assert_eq!(beleg.model_version, modell);
    assert_eq!(beleg.input_commitment.0, erste.eingangs_commitment);
    assert_eq!(
        beleg.output_commitment.0,
        *erste.trace.last().unwrap(),
        "die Ausgabe des Segments ist der letzte Spur-Eintrag"
    );
    assert_eq!(beleg.trace.len(), erste.trace.len());
    assert_eq!(beleg.pod_path, erste.pod_path);

}

/// Zwei Pods mit **verschiedenem** Zuschnitt sind jetzt vergleichbar.
///
/// Das ist die Bedingung, an der der Entwurf für variable Knotenzahl
/// hing: `compare_commitments` lehnt ungleiche Spurlängen ab.
#[test]
fn vier_gegen_acht_shards_sind_vergleichbar() {
    let dir = artifacts_dir();
    if !dir.exists() {
        eprintln!("SKIP: Artefakte fehlen: {:?}", dir);
        return;
    }
    let model = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let tokenizer =
        Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad-UTF-8"))
            .expect("Tokenizer-Ladung");
    let prompt_tokens: Vec<u32> = tokenizer.encode(PROMPT).iter().map(|t| *t as u32).collect();
    let num_layers = model.num_layers;

    let erste_spur = |k: usize| -> Vec<[u8; 32]> {
        let basis = num_layers / k;
        let mut grenzen = vec![0usize];
        for s in 0..k {
            let letzte = *grenzen.last().unwrap();
            grenzen.push(letzte + basis + usize::from(s < num_layers % k));
        }
        let shards: Vec<Arc<ShardNode>> = (0..k)
            .map(|s| {
                let sk = BlsSecretKey::key_gen(&[(s as u8 + 1).wrapping_mul(17); 32]).unwrap();
                Arc::new(ShardNode::new(
                    s, grenzen[s], grenzen[s + 1], s == 0, s + 1 == k,
                    model.clone(), sk,
                    MAX_NEW_TOKENS,
                ))
            })
            .collect();
        let mut c = Coordinator::new(
            PodId::new([0xAA; 32]), EpochId(0), shards,
            myl_pod::coordinator::DEFAULT_WINDOW_MS,
        );
        c.run_prompt(1, &prompt_tokens, MAX_NEW_TOKENS);
        c.completed_segments()[0].trace.clone()
    };

    let vier: Vec<myl_types::hash::Hash> =
        erste_spur(4).iter().map(|h| myl_types::hash::Hash::from_bytes(*h)).collect();
    let acht: Vec<myl_types::hash::Hash> =
        erste_spur(8).iter().map(|h| myl_types::hash::Hash::from_bytes(*h)).collect();

    let ergebnis = myl_verifier::redundancy::compare_commitments(&vier, &acht)
        .expect("gleiche Spurlängen, also vergleichbar");
    assert_eq!(
        ergebnis,
        myl_verifier::redundancy::CompareResult::Match,
        "vier und acht Shards müssen dieselbe Spur erzeugen"
    );
}
