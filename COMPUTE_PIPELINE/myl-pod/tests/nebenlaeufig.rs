//! ⚑ **Sitzungen nebeneinander rechnen bitgleich wie nacheinander**
//! (2026-11-15, Whitepaper 4.2). Der Koordinator nimmt `&self`; vier Fäden
//! rechnen vier Sitzungen über dieselben vier Shards. Jede Sitzung hat auf
//! jedem Shard ihren eigenen KV-Cache, also darf nichts davon abhängen, was
//! nebenher läuft: nicht die Token, nicht die Segmente, nicht die Wurzel.

use std::sync::Arc;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_pod::coordinator::Coordinator;
use myl_pod::shard::ShardNode;
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};

fn artefakte() -> std::path::PathBuf {
    let m = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "sparrow-0.6b-int8".to_string());
    std::path::PathBuf::from(m)
        .join("../../INTEGER_LLM/artifacts")
        .join(modell)
}

fn koordinator(dir: &std::path::Path) -> (Coordinator, Tokenizer) {
    let (shards, tok) = shards(dir);
    (Coordinator::new(PodId::new([0xAA; 32]), EpochId(0), shards), tok)
}

fn shards(dir: &std::path::Path) -> (Vec<Arc<ShardNode>>, Tokenizer) {
    shards_mit(dir, 4)
}

fn shards_mit(dir: &std::path::Path, deckel: u64) -> (Vec<Arc<ShardNode>>, Tokenizer) {
    let model = load_model(dir).expect("Modell");
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad"))
        .expect("Wortschatz");
    let nl = model.num_layers;
    let model = Arc::new(model);
    let mut shards = Vec::new();
    for s in 0..4usize {
        let sk = BlsSecretKey::key_gen(&[(s as u8 + 1) * 17; 32]).expect("BLS");
        shards.push(Arc::new(ShardNode::new(
            s,
            nl * s / 4,
            nl * (s + 1) / 4,
            s == 0,
            s == 3,
            model.clone(),
            sk,
            deckel,
        )));
    }
    (shards, tok)
}

const PROMPTS: [&str; 4] = [
    "Die Hauptstadt von Frankreich ist",
    "Eins, zwei, drei,",
    "Der Himmel ist",
    "Ein kurzer Satz über das Meer und die Berge",
];

/// Nacheinander gegen nebeneinander: dieselben Token je Sitzung, dieselben
/// Segmente, dieselbe Bündelwurzel und Ausgabenwurzel, und das letzte
/// Segment eines Laufs gehört zu seiner Sitzung.
/// ⚑ Gegenprobe von Hand (2026-11-15): Rechnen die vier Fäden unter
/// derselben Sitzungsnummer, teilen sie sich auf jedem Shard einen
/// KV-Cache, und die Probe wird rot (dreimal von dreien). Sie sieht also
/// Übersprechen zwischen Läufen.
#[test]
fn nebeneinander_rechnet_bitgleich_wie_nacheinander() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let (nacheinander, tok) = koordinator(&dir);
    let (nebeneinander, _) = koordinator(&dir);
    let eingaben: Vec<(u64, Vec<u32>)> = PROMPTS
        .iter()
        .enumerate()
        .map(|(i, p)| (100 + i as u64, tok.encode(p).iter().map(|t| *t as u32).collect()))
        .collect();
    let soll: Vec<myl_pod::coordinator::Lauf> =
        eingaben.iter().map(|(s, ids)| nacheinander.rechne(*s, ids, 4)).collect();
    let ist: Vec<myl_pod::coordinator::Lauf> = std::thread::scope(|faeden| {
        let laeufe: Vec<_> = eingaben
            .iter()
            .map(|(s, ids)| {
                let k = &nebeneinander;
                faeden.spawn(move || k.rechne(*s, ids, 4))
            })
            .collect();
        laeufe.into_iter().map(|l| l.join().expect("Lauf")).collect()
    });
    for ((sitzung, _), (a, b)) in eingaben.iter().zip(soll.iter().zip(&ist)) {
        assert!(!a.token.is_empty(), "Sitzung {sitzung} ohne Token");
        assert_eq!(a.token, b.token, "Sitzung {sitzung}: andere Token nebeneinander");
        let seg = b.letztes_segment.expect("Segment");
        assert_eq!(myl_types::sitzung_und_position(&seg).0, *sitzung, "das letzte Segment gehoert einer anderen Sitzung");
        assert_eq!(a.letztes_segment, b.letztes_segment);
    }
    let ids = |k: &Coordinator| {
        let mut v: Vec<_> = k.completed_segments().iter().map(|c| (c.id, c.spurwurzel, c.eingangs_commitment)).collect();
        v.sort_unstable_by_key(|e| *e.0.as_bytes());
        v
    };
    assert_eq!(ids(&nacheinander), ids(&nebeneinander), "andere Segmente nebeneinander");
    let a = nacheinander.build_poi_bundle().expect("Buendel");
    let b = nebeneinander.build_poi_bundle().expect("Buendel");
    assert_eq!(a.segments_root, b.segments_root);
    assert_eq!(a.ausgabenwurzel, b.ausgabenwurzel);
    assert_eq!(a.segmente, b.segmente);
    assert_eq!(a.vtfe_claimed, b.vtfe_claimed);
    for (s, _) in &eingaben {
        nebeneinander.sitzung_abschliessen(*s);
        nacheinander.sitzung_abschliessen(*s);
    }
    assert_eq!(nebeneinander.gehaltene_sitzungen(), 0);
}

/// Ein Weg um die Shards im Prozess, der jeden Einzelaufruf zögert, damit
/// sich die übrigen Sitzungen dahinter anstellen, und der die Stapel mit
/// mehr als einer Sitzung zählt.
struct Zoegerer {
    innen: myl_pod::shardweg::ImProzess,
    gemischt: std::sync::atomic::AtomicUsize,
    /// Gemischte Stapel, in denen Prefill-Positionen mitfahren.
    mit_prefill: std::sync::atomic::AtomicUsize,
}

impl myl_pod::shardweg::Shardweg for Zoegerer {
    fn shardzahl(&self) -> usize {
        self.innen.shardzahl()
    }

    fn frage(
        &self,
        nummer: usize,
        anfrage: &myl_pod::shardweg::Shardanfrage,
    ) -> Result<myl_pod::shardweg::Shardantwort, String> {
        match anfrage {
            myl_pod::shardweg::Shardanfrage::Rechne(_) => std::thread::sleep(std::time::Duration::from_millis(40)),
            myl_pod::shardweg::Shardanfrage::RechneStapel(ms) => {
                let erste = ms.first().map(|m| m.session_id);
                if ms.iter().any(|m| Some(m.session_id) != erste) {
                    self.gemischt.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if ms.iter().any(|m| m.flags & myl_pod::wire::FLAG_SAMPLE == 0) {
                        self.mit_prefill.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
            _ => {}
        }
        self.innen.frage(nummer, anfrage)
    }
}

/// ⚑ **Stapel über Sitzungen entstehen und ändern nichts** (Continuous
/// Batching, 2026-11-15): Sechs Sitzungen über einen Weg, der jeden
/// Einzelaufruf zögert; dahinter stellen sich die anderen an und fahren als
/// gemischter Stapel. Token, Segmente und Wurzeln bitgleich wie
/// nacheinander, und gemischte Stapel sind gefahren, auch solche mit
/// Prefill-Positionen neben Nachrichten anderer Sitzungen.
/// ⚑ Gegenprobe von Hand (2026-11-15): Rechnet `ebenen_stapel` jedes Token
/// mit dem ersten Speicher, wird die Probe rot.
#[test]
fn stapel_ueber_sitzungen_rechnen_bitgleich() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let (nacheinander, tok) = koordinator(&dir);
    let (shards, _) = shards(&dir);
    let weg = Arc::new(Zoegerer {
        innen: myl_pod::shardweg::ImProzess::neu(shards),
        gemischt: std::sync::atomic::AtomicUsize::new(0),
        mit_prefill: std::sync::atomic::AtomicUsize::new(0),
    });
    let gestapelt = Coordinator::ueber_weg(
        PodId::new([0xAA; 32]),
        EpochId(0),
        Arc::clone(&weg) as Arc<dyn myl_pod::shardweg::Shardweg>,
    )
    .expect("Koordinator");
    let eingaben: Vec<(u64, Vec<u32>)> = PROMPTS
        .iter()
        .chain(PROMPTS[..2].iter())
        .enumerate()
        .map(|(i, p)| (200 + i as u64, tok.encode(p).iter().map(|t| *t as u32).collect()))
        .collect();
    let soll: Vec<Vec<u32>> = eingaben.iter().map(|(s, ids)| nacheinander.rechne(*s, ids, 4).token).collect();
    let ist: Vec<Vec<u32>> = std::thread::scope(|faeden| {
        let h: Vec<_> = eingaben
            .iter()
            .map(|(s, ids)| {
                let k = &gestapelt;
                faeden.spawn(move || k.rechne(*s, ids, 4).token)
            })
            .collect();
        h.into_iter().map(|l| l.join().expect("Lauf")).collect()
    });
    assert_eq!(soll, ist, "gestapelt andere Token");
    let a = nacheinander.build_poi_bundle().expect("Buendel");
    let b = gestapelt.build_poi_bundle().expect("Buendel");
    assert_eq!((a.segments_root, a.ausgabenwurzel, a.segmente), (b.segments_root, b.ausgabenwurzel, b.segmente));
    let gemischt = weg.gemischt.load(std::sync::atomic::Ordering::Relaxed);
    assert!(gemischt > 0, "kein Stapel ueber Sitzungen gefahren; die Probe prueft nichts");
    // ⚑ Seit der Prefill über die Sammelstelle geht (2026-11-15): Prompts
    //   verschiedener Sitzungen und Decode-Token fahren zusammen.
    let mit_prefill = weg.mit_prefill.load(std::sync::atomic::Ordering::Relaxed);
    assert!(mit_prefill > 0, "kein gemischter Stapel mit Prefill-Positionen");
    eprintln!("[nebenlaeufig] gemischte Stapel: {gemischt}, davon mit Prefill: {mit_prefill}");
}

/// ⚑ **Fund 713: Ein Bündel ist der Anfang der Liste**, auch wenn danach
/// weiter gerechnet wird. Eingereicht wird unter der Lesesperre, neben
/// laufenden Sitzungen; Spurablage und Blätter für den Differenzbeweis
/// müssen dann aus genau den Segmenten des Bündels entstehen.
/// Gegenprobe: die ganze Liste nach einem weiteren Lauf trägt eine andere
/// Wurzel, so wie die Ablage vor dem Fix sie genommen hätte.
#[test]
fn das_buendel_ist_der_anfang_der_liste_auch_wenn_danach_gerechnet_wird() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let (k, tok) = koordinator(&dir);
    let ids = |p: &str| -> Vec<u32> { tok.encode(p).iter().map(|t| *t as u32).collect() };
    k.rechne(100, &ids(PROMPTS[0]), 4);
    k.rechne(101, &ids(PROMPTS[1]), 4);
    let vorher = k.kanonisch();
    let buendel = k.build_signed_poi_bundle().expect("Bündel");
    let n = buendel.segmente as usize;
    assert_eq!(n, vorher.len());

    // Ein Lauf nebenher, während schon das nächste Bündel gebaut wird: Es
    // kommt zustande, und seine Segmentzahl ist ein Anfang der Liste.
    let k = Arc::new(k);
    let nebenher = {
        let k = Arc::clone(&k);
        let eingabe = ids(PROMPTS[2]);
        std::thread::spawn(move || k.rechne(102, &eingabe, 4))
    };
    let zweites = k.build_signed_poi_bundle().expect("Bündel neben einem Lauf");
    nebenher.join().expect("Lauf");
    assert!(zweites.segmente as usize >= n && zweites.segmente as usize <= k.segmentzahl());

    // Nach dem Lauf: der Anfang ist das erste Bündel, Folge und Wurzeln gleich.
    let anfang = k.kanonisch_bis(n);
    assert_eq!(anfang.iter().map(|c| c.id).collect::<Vec<_>>(), vorher.iter().map(|c| c.id).collect::<Vec<_>>());
    let (ausgaben, zeugnisse) = k.blaetter_bis(n);
    assert_eq!(myl_types::segments_root(&zeugnisse).expect("Wurzel"), buendel.segments_root);
    assert_eq!(myl_types::ausgabenwurzel(&ausgaben).expect("Wurzel"), buendel.ausgabenwurzel);
    // ⛔️ Gegenprobe: die ganze Liste gehört nicht mehr zu diesem Bündel.
    let (_, alle) = k.blaetter();
    assert!(alle.len() > n, "der dritte Lauf hat nicht angehängt");
    assert_ne!(myl_types::segments_root(&alle).expect("Wurzel"), buendel.segments_root);
}

/// ⚑ **Ein langer Prefill neben einem laufenden Decode** (Simulation vom
/// 2026-11-20 und Kurzlauf vom 2026-11-23): Der langsamere Zwilling rechnete
/// noch das Decode einer Sitzung, als der lange Prompt der nächsten kam, und
/// lieferte andere Token als der Zwilling, der beides nacheinander rechnete.
/// Hier: Sitzung A dekodiert, Sitzung B kommt mitten hinein mit einem langen
/// Prompt (gruppierte Experten am Gemisch). Beide müssen bitgleich zu
/// „nacheinander“ sein. Am 35B gefahren (`MYL_POD_MODELL=
/// raven-35b-a3b-int4`, 2026-11-23): grün, 48 und 16 Token. Das 0,6B hat
/// keinen rekurrenten Zustand und kein Gemisch. ⚠️ Prefill und Decode fuhren
/// dabei nicht im selben Stapel (der Zähler steht in der Ausgabe); das
/// belegt `stapel_ueber_sitzungen_rechnen_bitgleich`. Die Abweichung aus
/// dem Lauf erklärt diese Probe also nicht.
#[test]
fn ein_langer_prefill_neben_einem_decode_rechnet_bitgleich() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let (shards_a, tok) = shards_mit(&dir, 64);
    let nacheinander = Coordinator::new(PodId::new([0xAA; 32]), EpochId(0), shards_a);
    let (shards, _) = shards_mit(&dir, 64);
    let weg = Arc::new(Zoegerer {
        innen: myl_pod::shardweg::ImProzess::neu(shards),
        gemischt: std::sync::atomic::AtomicUsize::new(0),
        mit_prefill: std::sync::atomic::AtomicUsize::new(0),
    });
    let gestapelt = Coordinator::ueber_weg(
        PodId::new([0xAA; 32]),
        EpochId(0),
        Arc::clone(&weg) as Arc<dyn myl_pod::shardweg::Shardweg>,
    )
    .expect("Koordinator");
    let ids = |p: &str| -> Vec<u32> { tok.encode(p).iter().map(|t| *t as u32).collect() };
    // Ein Prompt, der weiterzählt und nicht nach wenigen Token endet.
    let a = ids("Eins, zwei, drei, vier, fünf, sechs, sieben, acht,");
    let lang: String = (0..48).map(|i| format!("Absatz {i}: {} ", PROMPTS[i % 4])).collect::<Vec<_>>().join("\n");
    let b = ids(&lang);
    assert!(b.len() > 64, "der Prompt muss ueber mehrere Stapel gehen: {}", b.len());
    let soll_a = nacheinander.rechne(300, &a, 48).token;
    let soll_b = nacheinander.rechne(301, &b, 16).token;
    let (ist_a, ist_b) = std::thread::scope(|faeden| {
        let k = &gestapelt;
        let fa = faeden.spawn(|| k.rechne(300, &a, 48).token);
        std::thread::sleep(std::time::Duration::from_millis(400));
        let fb = faeden.spawn(|| k.rechne(301, &b, 16).token);
        (fa.join().expect("A"), fb.join().expect("B"))
    });
    let gemischt = weg.mit_prefill.load(std::sync::atomic::Ordering::Relaxed);
    eprintln!("[nebenlaeufig] A {} Token, B {} Token; gemischte Stapel mit Prefill: {gemischt}", ist_a.len(), ist_b.len());
    assert!(soll_a.len() > 16, "A endete zu frueh, um neben B zu laufen: {}", soll_a.len());
    assert_eq!(soll_a, ist_a, "das Decode von A wurde durch den Prefill von B veraendert");
    assert_eq!(soll_b, ist_b, "der Prefill von B rechnete neben A anders");
}
