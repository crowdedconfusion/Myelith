//! Fund 650: der Prefill als Stapel, eine Runde je Shard.
//!
//! Verlangt ist **Bitgleichheit mit dem Weg Token für Token**: dieselben
//! Token, dieselben Segmente mit denselben Spuren und Unterschriften, und
//! dabei für einen Prompt von P Token nur eine Rechenrunde je Shard statt P.
//! Den Weg Token für Token erzwingt ein Weg, der den Stapel ablehnt: Dann
//! wiederholt der Koordinator einzeln (Fund 654).

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

/// Ein Weg im Prozess, der Rechenrunden zählt und den Stapel wahlweise
/// ablehnt, wie ein Shard, der ihn nicht kennt.
struct Zaehlweg {
    innen: ImProzess,
    stapel_erlaubt: bool,
    einzeln: AtomicUsize,
    gestapelt: AtomicUsize,
}

impl Shardweg for Zaehlweg {
    fn shardzahl(&self) -> usize {
        self.innen.shardzahl()
    }
    fn frage(&self, nummer: usize, anfrage: &Shardanfrage) -> Result<Shardantwort, String> {
        match anfrage {
            Shardanfrage::Rechne(_) => {
                self.einzeln.fetch_add(1, Ordering::SeqCst);
            }
            Shardanfrage::RechneStapel(_) => {
                if !self.stapel_erlaubt {
                    return Ok(Shardantwort::Fehler("unbekannte Anfrage".into()));
                }
                self.gestapelt.fetch_add(1, Ordering::SeqCst);
            }
            _ => {}
        }
        self.innen.frage(nummer, anfrage)
    }
}

fn koordinator(dir: &std::path::Path, stapel_erlaubt: bool) -> (Coordinator, Arc<Zaehlweg>) {
    let model = Arc::new(load_model(dir).expect("Modell"));
    let nl = model.num_layers;
    let shards = (0..4usize)
        .map(|s| {
            let sk = BlsSecretKey::key_gen(&[(s as u8 + 1) * 17; 32]).expect("BLS");
            Arc::new(ShardNode::new(s, nl * s / 4, nl * (s + 1) / 4, s == 0, s == 3, model.clone(), sk, 4))
        })
        .collect();
    let weg = Arc::new(Zaehlweg {
        innen: ImProzess::neu(shards),
        stapel_erlaubt,
        einzeln: AtomicUsize::new(0),
        gestapelt: AtomicUsize::new(0),
    });
    let k = Coordinator::ueber_weg(PodId::new([0xAA; 32]), EpochId(0), weg.clone(), myl_pod::coordinator::DEFAULT_WINDOW_MS)
        .expect("Koordinator");
    (k, weg)
}

#[test]
fn der_stapel_rechnet_bitgleich_in_einer_runde_je_shard() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad")).expect("Wortschatz");
    let ids: Vec<u32> = tok.encode("Die Hauptstadt von Frankreich ist").iter().map(|t| *t as u32).collect();
    assert!(ids.len() > 2, "der Prompt ist zu kurz, um etwas zu zeigen");

    let (mut gestapelt, weg_s) = koordinator(&dir, true);
    let mit = gestapelt.run_prompt(7, &ids, 4);
    // ⛔️ Gegenprobe und Referenz: Token für Token.
    let (mut einzeln, weg_e) = koordinator(&dir, false);
    let ohne = einzeln.run_prompt(7, &ids, 4);

    assert_eq!(mit.len(), 4);
    assert_eq!(mit, ohne, "der Stapel rechnet andere Token");
    let a = gestapelt.completed_segments();
    let b = einzeln.completed_segments();
    assert_eq!(a.len(), b.len(), "andere Zahl Segmente");
    for (x, y) in a.iter().zip(b) {
        assert_eq!((x.id, x.position, x.spurwurzel, &x.trace), (y.id, y.position, y.spurwurzel, &y.trace), "ein Segment weicht ab");
        assert_eq!(x.signatures, y.signatures, "die Unterschriften weichen ab");
    }
    // Der Prefill: eine Runde je Shard statt eine je Token und Shard.
    assert_eq!(weg_s.gestapelt.load(Ordering::SeqCst), 4, "nicht eine Stapelrunde je Shard");
    // Danach die neuen Token einzeln: (4 - 1) Rückkopplungen durch vier Shards.
    assert_eq!(weg_s.einzeln.load(Ordering::SeqCst), 3 * 4);
    assert_eq!(weg_e.einzeln.load(Ordering::SeqCst), (ids.len() + 3) * 4);
}
