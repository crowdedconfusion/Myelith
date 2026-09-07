//! Ein Shard als eigenes Programm (Fund 186).
//!
//! # ⚑ Was dieses Programm ist, und warum es fehlte
//!
//! Es hält **einen** Shard: einen Layerbereich, einen Schlüssel, eine
//! Tür. Sonst nichts vom Pod.
//!
//! `myl-pod-node` hält demgegenüber über `Pipelinewerk` **alle vier**
//! Shards in einem Prozess, mit vier abgeleiteten Schlüsseln. Solange
//! das der einzige Weg war, gab es keinen geshardeten Pod, sondern eine
//! Maschine, die vier Rollen spielt, und jede Messung über „geshardete
//! Inferenz" mass das.
//!
//! # ⚑ Der Schlüssel kommt aus einer Datei und nicht aus der Nummer
//!
//! `--schluessel <datei>` liest 32 Bytes Startwert. **Ein Miner bringt
//! seinen eigenen mit**; er darf nicht aus dem Shard-Index folgen, sonst
//! kennt jeder, der die Nummer kennt, den Schlüssel.
//!
//! Für Vergleichsläufe gibt es `--probeschluessel`, das denselben Wert
//! ableitet wie `Pipelinewerk`. **Es steht ausdrücklich da und heisst
//! so**, damit niemand einen Probelauf für einen Betrieb hält.

use std::sync::Arc;

use myl_pod::shard::ShardNode;
use myl_pod::shardweg::Shardstelle;
use myl_types::bls::BlsSecretKey;

fn main() {
    let mut artefakte = String::new();
    let mut adresse = "127.0.0.1:0".to_string();
    let mut nummer: usize = 0;
    let mut shards: usize = 4;
    let mut deckel: u64 = 16;
    let mut schluesseldatei: Option<String> = None;
    let mut probeschluessel = false;
    let mut training = false;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--artefakte" => {
                i += 1;
                artefakte = args.get(i).cloned().unwrap_or_default();
            }
            "--adresse" => {
                i += 1;
                adresse = args.get(i).cloned().unwrap_or_default();
            }
            "--shard" => {
                i += 1;
                nummer = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            "--shards" => {
                i += 1;
                shards = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(4);
            }
            "--deckel" => {
                i += 1;
                deckel = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(16);
            }
            "--schluessel" => {
                i += 1;
                schluesseldatei = args.get(i).cloned();
            }
            "--probeschluessel" => probeschluessel = true,
            // ⚑ **Mit Trainingshaelfte starten.** Ohne sie lehnt der
            // Shard Trainingsanfragen ab, statt sie zu verschlucken.
            "--training" => training = true,
            andere => {
                eprintln!("[myl-shard] unbekannte Option: {andere}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    if artefakte.is_empty() {
        eprintln!("[myl-shard] --artefakte fehlt");
        std::process::exit(2);
    }
    if nummer >= shards {
        eprintln!("[myl-shard] Shard {nummer} gibt es bei {shards} Shards nicht");
        std::process::exit(2);
    }

    let ikm: [u8; 32] = match (&schluesseldatei, probeschluessel) {
        (Some(datei), _) => {
            let roh = match std::fs::read(datei) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[myl-shard] Schluesseldatei {datei}: {e}");
                    std::process::exit(1);
                }
            };
            match <[u8; 32]>::try_from(roh.as_slice()) {
                Ok(k) => k,
                Err(_) => {
                    eprintln!("[myl-shard] die Schluesseldatei hat {} statt 32 Bytes", roh.len());
                    std::process::exit(1);
                }
            }
        }
        // ⚑ Derselbe Wert wie in `Pipelinewerk`, damit ein Prozesslauf
        // gegen einen prozessinternen zu halten ist. **Nur dafür.**
        (None, true) => [(nummer as u8 + 1).wrapping_mul(17); 32],
        (None, false) => {
            eprintln!("[myl-shard] --schluessel <datei> oder --probeschluessel wird gebraucht");
            std::process::exit(2);
        }
    };
    let sk = match BlsSecretKey::key_gen(&ikm) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[myl-shard] Schluessel: {e:?}");
            std::process::exit(1);
        }
    };

    let modell = match integer_llm_runtime::loader::load_model(std::path::Path::new(&artefakte)) {
        Ok(m) => Arc::new(m),
        Err(e) => {
            eprintln!("[myl-shard] Modell nicht geladen: {e}");
            std::process::exit(1);
        }
    };
    // ⚑ **Derselbe Zuschnitt wie `Pipelinewerk`**: gleichmässig, Rest
    // nach hinten. Wichen sie ab, rechnete ein Pod aus vier Prozessen
    // etwas anderes als derselbe Pod in einem.
    let layer = modell.num_layers;
    let von = layer * nummer / shards;
    let bis = layer * (nummer + 1) / shards;
    let modell_fuer_training = modell.clone();
    let shard = Arc::new(ShardNode::new(
        nummer,
        von,
        bis,
        nummer == 0,
        nummer + 1 == shards,
        modell,
        sk,
        deckel,
    ));

    let stelle = if training {
        match Shardstelle::oeffnen_mit_training(shard, modell_fuer_training, &adresse) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[myl-shard] Tuer {adresse} mit Training: {e}");
                std::process::exit(1);
            }
        }
    } else {
        match Shardstelle::oeffnen(shard, &adresse) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[myl-shard] Tuer {adresse}: {e}");
                std::process::exit(1);
            }
        }
    };
    let echt = stelle.adresse().expect("Adresse");
    // ⚑ **Die Adresse auf die Standardausgabe, eine Zeile.** Wer diesen
    // Prozess startet, braucht sie; mit Port null steht sie erst jetzt
    // fest. Dasselbe Muster wie beim Poddienst.
    println!("SHARD {nummer} {echt}");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    eprintln!(
        "[myl-shard] Shard {nummer}: Layer {von} bis {bis}, Tuer {echt}{}",
        if training { ", Trainingshaelfte geladen" } else { "" }
    );
    stelle.laufen();
}
