//! ⛔️ **Fund 719 (2026-11-23): Nach einem Transportfehler wird ein Stapel
//! nicht still einzeln nachgerechnet.**
//!
//! Im Kurzlauf mit dem 35B rechnete der Shard des Lügners einen Stapel,
//! die Antwort ging verloren („unexpected end of file“), und der Koordinator
//! rechnete dieselben Nachrichten einzeln nach. Der Shard nahm die schon
//! gerechneten Positionen ein zweites Mal an; die lineare Attention des
//! Netzmodells schob ihren Zustand weiter, und die Zwillinge lieferten
//! verschiedene Token, ohne dass eine Zeile davon sprach.
//!
//! Hier rechnet ein Weg den ersten Stapel wirklich und meldet danach einen
//! Transportfehler. Erwartet: kein Einzelrechnen dieses Stapels, und die
//! Token sind dieselben wie ohne Fehler.
//!
//! ⚑ **Fund 723 (2026-11-24):** Bis hierher galt die Runde danach als
//! verloren und wurde von vorn gerechnet; unter Last lief das in eine
//! Spirale. Jetzt fragt der Koordinator dieselbe Anfrage noch einmal, der
//! Shard antwortet aus dem Speicher, und der Lauf gilt **nicht** als
//! wiederholt.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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

fn shards(dir: &std::path::Path) -> (Vec<Arc<ShardNode>>, Tokenizer) {
    let model = Arc::new(load_model(dir).expect("Modell"));
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad")).expect("Wortschatz");
    let nl = model.num_layers;
    let shards = (0..4usize)
        .map(|s| {
            let sk = BlsSecretKey::key_gen(&[(s as u8 + 1) * 19; 32]).expect("BLS");
            Arc::new(ShardNode::new(s, nl * s / 4, nl * (s + 1) / 4, s == 0, s == 3, model.clone(), sk, 8))
        })
        .collect();
    (shards, tok)
}

/// Rechnet den ersten Stapel an Shard 1 wirklich und meldet dann einen
/// Transportfehler; zählt danach einzelne Rechnungen an Shard 1, bis der
/// Koordinator die Sitzung vergessen lässt, und die Stapel an Shard 1.
struct VerlorenerStapel {
    innen: ImProzess,
    verloren: AtomicBool,
    einzeln_danach: AtomicUsize,
    vergessen: AtomicBool,
    stapel_an_1: AtomicUsize,
    /// Die Positionen des verlorenen Stapels.
    verlorene: std::sync::Mutex<Vec<u64>>,
}

impl Shardweg for VerlorenerStapel {
    fn shardzahl(&self) -> usize {
        self.innen.shardzahl()
    }

    fn frage(&self, nummer: usize, anfrage: &Shardanfrage) -> Result<Shardantwort, String> {
        // 📌 Verloren geht der erste Stapel an Shard 1 **hinter Position
        //   0** (der Prompt ist länger als ein Stapel): Ein Stapel ab 0 beginnt
        //   die Sitzung neu (Fund 709), und seine Wiederholung gelänge auch
        //   ohne den Speicher des Shards; die erste Fassung dieser Probe blieb
        //   so in der Gegenprobe grün.
        let positionen: Vec<u64> = match anfrage {
            Shardanfrage::RechneStapel(ms) => ms.iter().map(|m| m.position).collect(),
            _ => Vec::new(),
        };
        if nummer == 1 && !positionen.is_empty() && self.verlorene.lock().expect("Sperre").as_slice() == positionen.as_slice() {
            self.stapel_an_1.fetch_add(1, Ordering::SeqCst);
        }
        match anfrage {
            Shardanfrage::RechneStapel(_)
                if nummer == 1 && positionen.first().is_some_and(|p| *p > 0) && !self.verloren.swap(true, Ordering::SeqCst) =>
            {
                *self.verlorene.lock().expect("Sperre") = positionen.clone();
                self.stapel_an_1.fetch_add(1, Ordering::SeqCst);
                let _ = self.innen.frage(nummer, anfrage);
                return Err("IO error on outbound stream: unexpected end of file".to_string());
            }
            Shardanfrage::SitzungVergessen(_) => self.vergessen.store(true, Ordering::SeqCst),
            // Nur Positionen des verlorenen Stapels: Die Decode-Schritte
            // danach gehen ohnehin einzeln.
            Shardanfrage::Rechne(m)
                if nummer == 1
                    && self.verloren.load(Ordering::SeqCst)
                    && !self.vergessen.load(Ordering::SeqCst)
                    && self.verlorene.lock().expect("Sperre").contains(&m.position) =>
            {
                self.einzeln_danach.fetch_add(1, Ordering::SeqCst);
            }
            _ => {}
        }
        self.innen.frage(nummer, anfrage)
    }
}

#[test]
fn nach_einem_transportfehler_wird_der_stapel_nicht_still_einzeln_gerechnet() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let (s, tok) = shards(&dir);
    let soll = Coordinator::new(PodId::new([0xAB; 32]), EpochId(0), s);
    let text = "Eins, zwei, drei, vier, fünf, sechs, sieben, acht, neun, zehn, ".repeat(4);
    let prompt: Vec<u32> = tok.encode(&text).iter().map(|t| *t as u32).collect();
    assert!(prompt.len() > myl_pod::shardweg::MAX_STAPEL + 1, "der Prompt muss über einen Stapel hinausgehen, {} Token", prompt.len());
    let erwartet = soll.rechne(9, &prompt, 6);

    let (s, _) = shards(&dir);
    let weg = Arc::new(VerlorenerStapel {
        innen: ImProzess::neu(s),
        verloren: AtomicBool::new(false),
        einzeln_danach: AtomicUsize::new(0),
        vergessen: AtomicBool::new(false),
        stapel_an_1: AtomicUsize::new(0),
        verlorene: std::sync::Mutex::new(Vec::new()),
    });
    let k = Coordinator::ueber_weg(PodId::new([0xAB; 32]), EpochId(0), Arc::clone(&weg) as Arc<dyn Shardweg>).expect("Koordinator");
    let lauf = k.rechne(9, &prompt, 6);

    assert!(weg.verloren.load(Ordering::SeqCst), "der Fehler wurde nie ausgelöst; die Probe prüft nichts");
    assert_eq!(weg.einzeln_danach.load(Ordering::SeqCst), 0, "der verlorene Stapel wurde still einzeln nachgerechnet");
    assert!(weg.stapel_an_1.load(Ordering::SeqCst) >= 2, "dieselbe Anfrage wurde nicht noch einmal gefragt");
    assert!(!lauf.wiederholt, "die Runde wurde von vorn gerechnet, statt die Antwort noch einmal zu holen (Fund 723)");
    assert_eq!(lauf.token, erwartet.token, "nach dem Transportfehler andere Token");
}
