//! Fund 654: Eine verlorene Runde zu einem Shard kostet nicht den ganzen
//! Auftrag.
//!
//! Gesehen im langen Lauf über Docker: Eine Runde zu Shard 1 ging auf der
//! Leitung verloren, dem Shard fehlte damit eine Position im KV-Speicher,
//! und jede folgende lehnte er ab. Hier geht genau eine Runde verloren,
//! ohne dass der Shard sie sieht; verlangt ist dieselbe Antwort wie ohne
//! Verlust und kein doppeltes oder halbes Segment.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_pod::coordinator::Coordinator;
use myl_pod::shard::ShardNode;
use myl_pod::shardweg::{ImProzess, Shardanfrage, Shardantwort, Shardweg};
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};

fn artefakte() -> std::path::PathBuf {
    let m = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "sparrow-0.6b-int8".to_string());
    std::path::PathBuf::from(m).join("../../INTEGER_LLM/artifacts").join(modell)
}

/// Ein Weg im Prozess, auf dem Rechenrunden zu einem Shard verloren gehen:
/// die `ab`-te und, wenn `dauerhaft`, jede danach. Der Shard sieht sie nicht.
struct Stolperweg {
    innen: ImProzess,
    shard: usize,
    ab: usize,
    dauerhaft: bool,
    gezaehlt: AtomicUsize,
}

impl Shardweg for Stolperweg {
    fn shardzahl(&self) -> usize {
        self.innen.shardzahl()
    }
    fn frage(&self, nummer: usize, anfrage: &Shardanfrage) -> Result<Shardantwort, String> {
        if nummer == self.shard && matches!(anfrage, Shardanfrage::Rechne(_)) {
            let n = self.gezaehlt.fetch_add(1, Ordering::SeqCst) + 1;
            if n == self.ab || (self.dauerhaft && n > self.ab) {
                return Err("IO error on outbound stream: unexpected end of file".into());
            }
        }
        self.innen.frage(nummer, anfrage)
    }
}

fn shards(dir: &std::path::Path) -> Vec<Arc<ShardNode>> {
    let model = Arc::new(load_model(dir).expect("Modell"));
    let nl = model.num_layers;
    (0..4usize)
        .map(|s| {
            let sk = BlsSecretKey::key_gen(&[(s as u8 + 1) * 17; 32]).expect("BLS");
            Arc::new(ShardNode::new(s, nl * s / 4, nl * (s + 1) / 4, s == 0, s == 3, model.clone(), sk, 4))
        })
        .collect()
}

fn koordinator(dir: &std::path::Path, ab: usize, dauerhaft: bool) -> Coordinator {
    let weg = Arc::new(Stolperweg { innen: ImProzess::neu(shards(dir)), shard: 1, ab, dauerhaft, gezaehlt: AtomicUsize::new(0) });
    Coordinator::ueber_weg(PodId::new([0xAA; 32]), EpochId(0), weg, myl_pod::coordinator::DEFAULT_WINDOW_MS).expect("Koordinator")
}

#[test]
fn eine_verlorene_runde_kostet_nicht_den_auftrag() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad")).expect("Wortschatz");
    let ids: Vec<u32> = tok.encode("Die Hauptstadt von Frankreich ist").iter().map(|t| *t as u32).collect();

    // Die Referenz: kein Verlust.
    let mut glatt = koordinator(&dir, usize::MAX, false);
    let soll = glatt.run_prompt(7, &ids, 4);
    assert_eq!(soll.len(), 4, "die Referenz liefert nicht vier Token");
    let segmente = glatt.completed_segments().len();

    // Die zweite Rechenrunde zu Shard 1 geht verloren: Position 1 fehlt ihm.
    let mut gestolpert = koordinator(&dir, 2, false);
    let ist = gestolpert.run_prompt(7, &ids, 4);
    assert_eq!(ist, soll, "nach einer verlorenen Runde kam eine andere Antwort");
    assert_eq!(
        gestolpert.completed_segments().len(),
        segmente,
        "nach der Wiederholung stehen Segmente doppelt oder halb im Bündel"
    );
    let mut ids_seg: Vec<_> = gestolpert.completed_segments().iter().map(|c| c.id).collect();
    ids_seg.sort();
    ids_seg.dedup();
    assert_eq!(ids_seg.len(), segmente, "eine Segmentkennung steht doppelt");

    // ⛔️ Gegenprobe: Shard 1 bleibt ab der zweiten Runde weg. Dann kommt
    // nichts, und es bleibt kein halbes Segment stehen.
    let mut weg = koordinator(&dir, 2, true);
    assert!(weg.run_prompt(7, &ids, 4).is_empty(), "ohne Shard 1 kamen Token");
    assert_eq!(weg.completed_segments().len(), 0, "ein gescheiterter Auftrag hinterliess Segmente");
}
