//! Fund 649: Ein Auftrag über die Epochengrenze bekommt seine Antwort.
//!
//! Im langen Lauf über Docker scheiterten fünf von sechs Ausfällen daran:
//! Am Epochenwechsel beendeten die Knoten ihre Shards, auch mitten in einem
//! Auftrag, und das Podwerk gab auf. Hier steht ein Relais im Prozess, das
//! dasselbe Drahtformat spricht wie der Knoten: je Epoche ein Pod aus vier
//! eigenen Shards, und nach einer festen Zahl von Rechenrunden wechselt es
//! mitten im Auftrag die Epoche. Danach antworten die alten Mitglieder
//! „kein Shard an diesem Knoten“, wie im Netz.

use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::IntegerModel;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_pod::coordinator::Coordinator;
use myl_pod::entsiegelung::Klartextwerk;
use myl_pod::podwerk::Podwerk;
use myl_pod::shard::ShardNode;
use myl_pod::shardweg::{ImProzess, Shardanfrage, Shardweg};
use myl_types::bls::BlsSecretKey;
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, MinerId, PodId};
use myl_types::inferenzauftrag::{Inferenzantwort, Inferenzauftrag};
use myl_types::latency_attest::PeerIdBytes;
use myl_types::relais::{Podbild, Podeintrag, Podmitglied, Relaisantwort, Relaisfrage, MAX_RELAIS_BYTES};
use myl_types::sitzung::Anfragebindung;

const ERSTE: u64 = 5;
const PROMPT: &str = "Die Hauptstadt von Frankreich ist";

fn artefakte() -> std::path::PathBuf {
    let m = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "sparrow-0.6b-int8".to_string());
    std::path::PathBuf::from(m).join("../../INTEGER_LLM/artifacts").join(modell)
}

fn shards(model: &Arc<IntegerModel>, saat: u8) -> Vec<Arc<ShardNode>> {
    shards_mit(model, saat, 4)
}

fn shards_mit(model: &Arc<IntegerModel>, saat: u8, deckel: u64) -> Vec<Arc<ShardNode>> {
    let nl = model.num_layers;
    (0..4usize)
        .map(|s| {
            let sk = BlsSecretKey::key_gen(&[(s as u8 + 1).wrapping_mul(17) ^ saat; 32]).expect("BLS");
            Arc::new(ShardNode::new(s, nl * s / 4, nl * (s + 1) / 4, s == 0, s == 3, Arc::clone(model), sk, deckel))
        })
        .collect()
}

fn mitglied(epoche: u64, i: usize) -> PeerIdBytes {
    PeerIdBytes([(epoche as u8) * 16 + i as u8; 32])
}

/// Das Relais: ein Pod je Epoche, Wechsel nach `wechsel_nach` Rechenrunden.
/// Nach dem Wechsel antworten die neuen Mitglieder `anlauf` Fragen lang
/// „kein Shard an diesem Knoten“, wie ein Knoten, der seinen Shard noch
/// startet.
struct Relais {
    epoche: AtomicU64,
    runden: AtomicUsize,
    wechsel_nach: usize,
    anlauf: AtomicUsize,
    pods: Vec<ImProzess>,
    schichten: usize,
    /// Fund 726: Die Mitglieder der alten Epoche rechnen nach dem Wechsel
    /// weiter, mit dieser Verzögerung je Frage (wie ein Knoten, der seinen
    /// Shard noch nicht abgebaut hat); `None` heisst „kein Shard“.
    alt_weiter: Option<std::time::Duration>,
    /// Wie viele Rechenfragen der alte Pod nach dem Wechsel bekam.
    alt_danach: AtomicUsize,
    /// Fund 730: so lange braucht jede Rechenfrage im Pod der Epoche.
    pause: Option<std::time::Duration>,
}

impl Relais {
    fn antworte(&self, frage: Relaisfrage) -> Relaisantwort {
        let e = self.epoche.load(Ordering::SeqCst);
        match frage {
            Relaisfrage::Podbild => Relaisantwort::Podbild(Podbild {
                epoche: e,
                hoehe: e * 1800 + 500,
                bloecke_je_epoche: 1800,
                pods: vec![Podeintrag {
                    pod_index: 0,
                    kennung: myl_types::miner::pod_kennung(e, 0),
                    mitglieder: (0..4)
                        .map(|i| Podmitglied {
                            miner: MinerId::new([i as u8 + 1; 32]),
                            netzadresse: mitglied(e, i),
                            von: (self.schichten * i / 4) as u32,
                            bis: (self.schichten * (i + 1) / 4) as u32,
                        })
                        .collect(),
                    zwilling: None,
                    reserve: Vec::new(),
                }],
            }),
            Relaisfrage::Shard { an, anfrage } => {
                let Some(i) = (0..4).find(|i| mitglied(e, *i) == an) else {
                    // Ein Mitglied der alten Epoche, das noch rechnet.
                    if let (Some(pause), Some(i)) = (self.alt_weiter, (0..4).find(|i| e > ERSTE && mitglied(e - 1, *i) == an)) {
                        let Ok(anfrage) = borsh::from_slice::<Shardanfrage>(&anfrage) else {
                            return Relaisantwort::Fehler("unlesbar".into());
                        };
                        if matches!(anfrage, Shardanfrage::Rechne(_) | Shardanfrage::RechneStapel(_)) {
                            self.alt_danach.fetch_add(1, Ordering::SeqCst);
                            std::thread::sleep(pause);
                        }
                        return match self.pods[(e - 1 - ERSTE) as usize].frage(i, &anfrage) {
                            Ok(a) => Relaisantwort::Shard(borsh::to_vec(&a).expect("Borsh")),
                            Err(f) => Relaisantwort::Fehler(f),
                        };
                    }
                    return Relaisantwort::Fehler("kein Shard an diesem Knoten".into());
                };
                if e > ERSTE && self.anlauf.load(Ordering::SeqCst) > 0 {
                    self.anlauf.fetch_sub(1, Ordering::SeqCst);
                    return Relaisantwort::Fehler("kein Shard an diesem Knoten".into());
                }
                let Ok(anfrage) = borsh::from_slice::<Shardanfrage>(&anfrage) else {
                    return Relaisantwort::Fehler("unlesbar".into());
                };
                if let (Some(p), true) = (self.pause, matches!(anfrage, Shardanfrage::Rechne(_) | Shardanfrage::RechneStapel(_))) {
                    std::thread::sleep(p);
                }
                if matches!(anfrage, Shardanfrage::Rechne(_)) && self.runden.fetch_add(1, Ordering::SeqCst) + 1 == self.wechsel_nach {
                    // Der Wechsel kommt mitten im Auftrag: Diese Runde geht
                    // noch durch, die nächste trifft schon die neue Epoche.
                    self.epoche.fetch_add(1, Ordering::SeqCst);
                }
                let pod = &self.pods[(e - ERSTE) as usize];
                match pod.frage(i, &anfrage) {
                    Ok(a) => Relaisantwort::Shard(borsh::to_vec(&a).expect("Borsh")),
                    Err(f) => Relaisantwort::Fehler(f),
                }
            }
            _ => Relaisantwort::Fehler("nicht in diesem Test".into()),
        }
    }
}

fn relais_starten(relais: Arc<Relais>, ausweis: [u8; 32]) -> std::net::SocketAddr {
    let lauscher = TcpListener::bind("127.0.0.1:0").expect("binden");
    let wo = lauscher.local_addr().expect("Adresse");
    std::thread::spawn(move || {
        for strom in lauscher.incoming().flatten() {
            let r = Arc::clone(&relais);
            std::thread::spawn(move || {
                let mut strom = strom;
                let Ok(roh) = myl_types::zeitprobe::rahmen_empfangen_mit_ausweis(&mut strom, &ausweis, MAX_RELAIS_BYTES) else {
                    return;
                };
                let Some(frage) = Relaisfrage::aus_bytes(&roh) else { return };
                if let Some(b) = r.antworte(frage).als_bytes() {
                    let _ = myl_types::zeitprobe::rahmen_senden(&mut strom, &b, MAX_RELAIS_BYTES);
                }
            });
        }
    });
    wo
}

fn auftrag(sitzung: u64) -> Inferenzauftrag {
    auftrag_mit(sitzung, 4, PROMPT)
}

fn auftrag_mit(sitzung: u64, max_token: u32, prompt: &str) -> Inferenzauftrag {
    Inferenzauftrag {
        sitzung,
        bindung: Anfragebindung::neu(sitzung, prompt.as_bytes(), EpochId(ERSTE)),
        prompt_versiegelt: Vec::new(),
        max_token,
        pipeline: Hash::sha256(b"probe"),
        hoechstens: u64::MAX,
    }
}

#[test]
fn ein_auftrag_ueber_die_epochengrenze_bekommt_seine_antwort() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let model = Arc::new(load_model(&dir).expect("Modell"));
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad")).expect("Wortschatz");
    let ids: Vec<u32> = tok.encode(PROMPT).iter().map(|t| *t as u32).collect();
    // Die Referenz: derselbe Auftrag ohne Wechsel.
    let soll = Coordinator::new(PodId::new([1; 32]), EpochId(ERSTE), shards(&model, 0))
        .run_prompt(9, &ids, 4);
    assert_eq!(soll.len(), 4);

    let ordner = std::env::temp_dir().join(format!("myl-podwerk-wechsel-{}", std::process::id()));
    std::fs::create_dir_all(&ordner).expect("Ordner");
    let ausweis = [7u8; 32];
    let datei = ordner.join("relais.ausweis");
    std::fs::write(&datei, format!("{}\n", "07".repeat(32))).expect("Ausweis");

    // Der Wechsel nach der sechsten Rechenrunde: mitten im Prefill.
    let relais = Arc::new(Relais {
        epoche: AtomicU64::new(ERSTE),
        runden: AtomicUsize::new(0),
        wechsel_nach: 6,
        anlauf: AtomicUsize::new(5),
        pods: vec![ImProzess::neu(shards(&model, 0)), ImProzess::neu(shards(&model, 1))],
        schichten: model.num_layers,
        alt_weiter: None,
        alt_danach: AtomicUsize::new(0),
        pause: None,
    });
    let wo = relais_starten(Arc::clone(&relais), ausweis);
    let werk = Podwerk::neu(&dir, wo, datei, Hash::sha256(b"probe"), 16).expect("Podwerk");
    let antwort = werk.rechne(&auftrag(9), PROMPT.as_bytes());
    let _ = std::fs::remove_dir_all(&ordner);

    assert!(relais.epoche.load(Ordering::SeqCst) > ERSTE, "der Wechsel kam nicht; der Test prüft nichts");
    match antwort {
        Inferenzantwort::Ergebnis { token, .. } => assert_eq!(token, soll, "über die Grenze kam eine andere Antwort"),
        andere => panic!("über die Epochengrenze keine Antwort: {andere:?}"),
    }
    assert_eq!(relais.anlauf.load(Ordering::SeqCst), 0, "der Anlauf der neuen Shards wurde nie gefragt");
}

/// ⚑ **Ein ungedeckter Auftrag kostet keinen Pod** (Fund 711): Das Podwerk
/// zaehlt die Prompt-Token und antwortet „nicht gedeckt“, bevor es das
/// Relais fragt. Gegenprobe: Mit genug Betrag fragt es und rechnet.
#[test]
fn ein_ungedeckter_auftrag_kostet_keinen_pod() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let model = Arc::new(load_model(&dir).expect("Modell"));
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad")).expect("Wortschatz");
    let p = tok.encode(PROMPT).len() as u64;
    let ordner = std::env::temp_dir().join(format!("myl-podwerk-ungedeckt-{}", std::process::id()));
    std::fs::create_dir_all(&ordner).expect("Ordner");
    let ausweis = [7u8; 32];
    let datei = ordner.join("relais.ausweis");
    std::fs::write(&datei, format!("{}\n", "07".repeat(32))).expect("Ausweis");
    let relais = Arc::new(Relais {
        epoche: AtomicU64::new(ERSTE),
        runden: AtomicUsize::new(0),
        wechsel_nach: usize::MAX,
        anlauf: AtomicUsize::new(0),
        pods: vec![ImProzess::neu(shards(&model, 0))],
        schichten: model.num_layers,
        alt_weiter: None,
        alt_danach: AtomicUsize::new(0),
        pause: None,
    });
    let wo = relais_starten(Arc::clone(&relais), ausweis);
    let werk = Podwerk::neu(&dir, wo, datei, Hash::sha256(b"probe"), 16).expect("Podwerk");

    let mut knapp = auftrag(9);
    knapp.hoechstens = p - 1;
    let antwort = werk.rechne(&knapp, PROMPT.as_bytes());
    assert_eq!(antwort, Inferenzantwort::Ungedeckt { sitzung: 9, prompt_token: p });
    assert_eq!(relais.runden.load(Ordering::SeqCst), 0, "ein ungedeckter Auftrag hat gerechnet");

    let mut genug = auftrag(9);
    genug.hoechstens = myl_types::inferenzauftrag::preis(p, 2);
    let antwort = werk.rechne(&genug, PROMPT.as_bytes());
    let _ = std::fs::remove_dir_all(&ordner);
    match antwort {
        Inferenzantwort::Ergebnis { token, .. } => assert_eq!(token.len(), 2, "auf den Betrag gekuerzt"),
        andere => panic!("gedeckt und nicht gerechnet: {andere:?}"),
    }
    assert!(relais.runden.load(Ordering::SeqCst) > 0, "Gegenprobe: mit Betrag fragt das Podwerk den Pod");
}

/// ⚑ **Fund 726 (2026-11-24): Beim Epochenwechsel hält der alte Lauf an.**
/// Im Netz bauten Knoten ihren Shard nicht sofort ab, und der Lauf der alten
/// Epoche rechnete 13 bis 21 Minuten weiter, neben dem Pod der neuen. Hier
/// rechnen die alten Mitglieder nach dem Wechsel weiter, langsam (300 ms je
/// Frage). Verlangt: dieselbe Antwort, vom Pod der neuen Epoche, und der
/// alte bekommt danach höchstens die Fragen bis zur nächsten Prüfung des
/// Wächters (`EPOCHE_PRUEFEN`), nicht den Rest des Auftrags. Gegenprobe ohne
/// Wächter: 40 Token zu je vier Shards, also über 100 Fragen an den alten.
#[test]
fn beim_epochenwechsel_haelt_der_alte_lauf_an() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let model = Arc::new(load_model(&dir).expect("Modell"));
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad")).expect("Wortschatz");
    // Ein Prompt, der weiterzählt: 40 Token ohne Endmarke (sonst bliebe dem
    // alten Pod nach dem Wechsel kaum etwas, und die Gegenprobe ohne Wächter
    // blieb grün).
    let zaehlen = "Eins, zwei, drei, vier, fünf, sechs, sieben, acht, neun, zehn, elf, zwölf,";
    let ids: Vec<u32> = tok.encode(zaehlen).iter().map(|t| *t as u32).collect();
    let soll = Coordinator::new(PodId::new([1; 32]), EpochId(ERSTE), shards_mit(&model, 0, 64)).run_prompt(11, &ids, 40);
    assert_eq!(soll.len(), 40, "der Zählprompt endet vor 40 Token; die Probe prüfte zu wenig");

    let ordner = std::env::temp_dir().join(format!("myl-podwerk-anhalten-{}", std::process::id()));
    std::fs::create_dir_all(&ordner).expect("Ordner");
    let ausweis = [7u8; 32];
    let datei = ordner.join("relais.ausweis");
    std::fs::write(&datei, format!("{}\n", "07".repeat(32))).expect("Ausweis");
    let relais = Arc::new(Relais {
        epoche: AtomicU64::new(ERSTE),
        runden: AtomicUsize::new(0),
        wechsel_nach: 6,
        anlauf: AtomicUsize::new(0),
        pods: vec![ImProzess::neu(shards_mit(&model, 0, 64)), ImProzess::neu(shards_mit(&model, 1, 64))],
        schichten: model.num_layers,
        alt_weiter: Some(std::time::Duration::from_millis(300)),
        alt_danach: AtomicUsize::new(0),
        pause: None,
    });
    let wo = relais_starten(Arc::clone(&relais), ausweis);
    let werk = Podwerk::neu(&dir, wo, datei, Hash::sha256(b"probe"), 64).expect("Podwerk");
    let antwort = werk.rechne(&auftrag_mit(11, 40, zaehlen), zaehlen.as_bytes());
    let _ = std::fs::remove_dir_all(&ordner);

    assert!(relais.epoche.load(Ordering::SeqCst) > ERSTE, "der Wechsel kam nicht; der Test prüft nichts");
    match antwort {
        Inferenzantwort::Ergebnis { token, .. } => assert_eq!(token, soll, "über die Grenze kam eine andere Antwort"),
        andere => panic!("über die Epochengrenze keine Antwort: {andere:?}"),
    }
    let alt = relais.alt_danach.load(Ordering::SeqCst);
    assert!(alt > 0, "der alte Pod rechnete nach dem Wechsel nicht weiter; der Test prüft nichts");
    // Höchstens zwei Prüfabstände zu je 300 ms je Frage, mit Luft.
    let grenze = 2 * (myl_pod::podwerk::EPOCHE_PRUEFEN.as_millis() as usize) / 300 + 4;
    assert!(alt <= grenze, "der alte Lauf bekam nach dem Wechsel noch {alt} Fragen (höchstens {grenze})");
}

/// ⛔️ **Fund 730 (2026-11-25): Wartet niemand mehr, hält der Lauf an.** Im
/// langen Auftragslauf rechnete eine aufgegebene Runde 27 Minuten weiter und
/// wurde am Wechsel noch an den Pod der neuen Epoche gegeben. Hier braucht
/// jede Rechenfrage 100 ms, 40 Token also über 16 s; nach den ersten
/// Token kommt der Abbruch. Verlangt: keine Antwort, binnen kurzem, und danach
/// höchstens die Frage, die gerade lief. Gegenprobe ohne Abbruch im
/// Koordinator: Der Lauf rechnet zu Ende.
#[test]
fn eine_aufgegebene_anfrage_haelt_an() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let model = Arc::new(load_model(&dir).expect("Modell"));
    let zaehlen = "Eins, zwei, drei, vier, fünf, sechs, sieben, acht, neun, zehn, elf, zwölf,";
    let ordner = std::env::temp_dir().join(format!("myl-podwerk-abbruch-{}", std::process::id()));
    std::fs::create_dir_all(&ordner).expect("Ordner");
    let ausweis = [7u8; 32];
    let datei = ordner.join("relais.ausweis");
    std::fs::write(&datei, format!("{}\n", "07".repeat(32))).expect("Ausweis");
    let relais = Arc::new(Relais {
        epoche: AtomicU64::new(ERSTE),
        runden: AtomicUsize::new(0),
        wechsel_nach: usize::MAX,
        anlauf: AtomicUsize::new(0),
        pods: vec![ImProzess::neu(shards_mit(&model, 0, 64))],
        schichten: model.num_layers,
        alt_weiter: None,
        alt_danach: AtomicUsize::new(0),
        pause: Some(std::time::Duration::from_millis(100)),
    });
    let wo = relais_starten(Arc::clone(&relais), ausweis);
    let werk = Podwerk::neu(&dir, wo, datei, Hash::sha256(b"probe"), 64).expect("Podwerk");
    let (antwort, beim_abbruch, dauer) = std::thread::scope(|f| {
        let lauf = f.spawn(|| werk.rechne(&auftrag_mit(11, 40, zaehlen), zaehlen.as_bytes()));
        // Abgebrochen wird mitten im Dekodieren.
        let warten = std::time::Instant::now();
        while relais.runden.load(Ordering::SeqCst) < 8 && warten.elapsed() < std::time::Duration::from_secs(60) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let beim_abbruch = relais.runden.load(Ordering::SeqCst);
        let beginn = std::time::Instant::now();
        werk.abbrechen(11);
        (lauf.join().expect("Lauf"), beim_abbruch, beginn.elapsed())
    });
    let _ = std::fs::remove_dir_all(&ordner);
    assert!(beim_abbruch > 0, "beim Abbruch war noch nichts gerechnet; der Test prüft nichts");
    assert!(matches!(antwort, Inferenzantwort::Abgelehnt { sitzung: 11 }), "{antwort:?}");
    let danach = relais.runden.load(Ordering::SeqCst) - beim_abbruch;
    assert!(danach <= 2, "nach dem Abbruch noch {danach} Fragen");
    assert!(dauer < std::time::Duration::from_secs(2), "erst {dauer:?} nach dem Abbruch");
}
