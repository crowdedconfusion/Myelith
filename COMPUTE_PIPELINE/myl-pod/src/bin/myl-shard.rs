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
//! ⚑ **Oder aus der Datei des Konsensschlüssels** (`--konsensschluessel`,
//! Stufe 4, P7, 2026-10-22): So startet der Knoten seinen Shard, und der
//! Shard unterschreibt mit dem Schlüssel, unter dem der Miner angemeldet
//! ist. Dieselbe Schreibweise wie dort: die erste Zeile ohne `#`, 64
//! Hexzeichen.
//!
//! Für Vergleichsläufe gibt es `--probeschluessel`, das denselben Wert
//! ableitet wie `Pipelinewerk`. **Es steht ausdrücklich da und heisst
//! so**, damit niemand einen Probelauf für einen Betrieb hält.

use std::sync::Arc;

use myl_pod::shard::ShardNode;
use myl_pod::shardweg::Shardstelle;
use myl_types::bls::BlsSecretKey;

/// Die 32 Bytes Startwert aus einer Datei des Konsensschlüssels: die erste
/// Zeile, die nach Abzug von Kommentaren nicht leer ist, als 64 Hexzeichen.
fn konsens_ikm(datei: &str) -> Result<[u8; 32], String> {
    let text = std::fs::read_to_string(datei).map_err(|e| e.to_string())?;
    let wort = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .find(|l| !l.is_empty())
        .ok_or("keine Zeile mit einem Schluessel")?;
    if wort.len() != 64 {
        return Err(format!("{} statt 64 Hexzeichen", wort.len()));
    }
    let mut aus = [0u8; 32];
    for (i, b) in aus.iter_mut().enumerate() {
        *b = u8::from_str_radix(&wort[i * 2..i * 2 + 2], 16).map_err(|_| "kein Hex".to_string())?;
    }
    Ok(aus)
}

fn main() {
    let mut artefakte = String::new();
    let mut adresse = "127.0.0.1:0".to_string();
    let mut nummer: usize = 0;
    let mut shards: usize = 4;
    let mut deckel: u64 = 16;
    let mut schluesseldatei: Option<String> = None;
    let mut konsensdatei: Option<String> = None;
    let mut ausweisdatei: Option<String> = None;
    let mut probeschluessel = false;
    let mut training = false;
    let mut von_bis: (Option<usize>, Option<usize>) = (None, None);

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
            // ⚑ **Der Ebenenbereich als Eingabe** (Stufe 4 des Dispatchers,
            //   2026-10-21): Die Verteilung im Netz schneidet ungleich, nach
            //   Kapazitaet und Ebenenzeit der Mitglieder. Ohne diese beiden
            //   gilt der gleiche Schnitt aus `--shard` und `--shards`.
            "--von" => {
                i += 1;
                von_bis.0 = args.get(i).and_then(|s| s.parse().ok());
            }
            "--bis" => {
                i += 1;
                von_bis.1 = args.get(i).and_then(|s| s.parse().ok());
            }
            "--deckel" => {
                i += 1;
                deckel = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(16);
            }
            "--schluessel" => {
                i += 1;
                schluesseldatei = args.get(i).cloned();
            }
            "--konsensschluessel" => {
                i += 1;
                konsensdatei = args.get(i).cloned();
            }
            // ⚑ **Der Ausweis der Tür** (Fund 611): 32 Bytes in einer
            //   Datei, die der Startende mit `0600` anlegt. Ohne ihn nimmt
            //   die Tür nichts an, also startet sie ohne ihn nicht.
            "--ausweis" => {
                i += 1;
                ausweisdatei = args.get(i).cloned();
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

    let aus_konsens = konsensdatei.as_deref().map(|datei| {
        konsens_ikm(datei).unwrap_or_else(|e| {
            eprintln!("[myl-shard] Konsensschluessel {datei}: {e}");
            std::process::exit(1);
        })
    });
    let ikm: [u8; 32] = match (&schluesseldatei, probeschluessel) {
        _ if aus_konsens.is_some() => aus_konsens.unwrap_or_default(),
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

    // ⚑ **Mit Bereich nur seine Dateien** (Teilablage, Fund 589): Ein
    //   Knoten, der nur einen Teil des Artefakts haelt, startet den Shard
    //   fuer genau diesen Teil.
    let geladen = match von_bis {
        (Some(v), Some(b)) => integer_llm_runtime::loader::load_model_fuer_netz_bereich(std::path::Path::new(&artefakte), v, b),
        _ => integer_llm_runtime::loader::load_model_fuer_netz(std::path::Path::new(&artefakte)),
    };
    let modell = match geladen {
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
    let (von, bis) = match von_bis {
        (None, None) => (layer * nummer / shards, layer * (nummer + 1) / shards),
        (Some(v), Some(b)) if v < b && b <= layer => (v, b),
        (Some(v), Some(b)) => {
            eprintln!("[myl-shard] Bereich {v} bis {b} passt nicht zu {layer} Ebenen (gebraucht: von < bis <= {layer})");
            std::process::exit(2);
        }
        _ => {
            eprintln!("[myl-shard] --von und --bis gehoeren zusammen");
            std::process::exit(2);
        }
    };
    let modell_fuer_training = modell.clone();
    let shard = Arc::new(ShardNode::new(
        nummer,
        von,
        bis,
        von == 0,
        bis == layer,
        modell,
        sk,
        deckel,
    ));

    let ausweis = match ausweisdatei.as_deref().map(|d| myl_types::ortsleitung::schluessel_lesen(std::path::Path::new(d))) {
        Some(Ok(a)) => a,
        Some(Err(e)) => {
            eprintln!("[myl-shard] Ausweis nicht lesbar: {e}");
            std::process::exit(2);
        }
        None => {
            eprintln!(
                "[myl-shard] --ausweis <datei> fehlt: 32 Bytes, mit denen jede Anfrage an die Tuer \
                 beginnt. Ohne ihn nimmt die Tuer von jedem Prozess dieser Maschine an."
            );
            std::process::exit(2);
        }
    };

    let stelle = if training {
        match Shardstelle::oeffnen_mit_training(shard, modell_fuer_training, &adresse, ausweis) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[myl-shard] Tuer {adresse} mit Training: {e}");
                std::process::exit(1);
            }
        }
    } else {
        match Shardstelle::oeffnen(shard, &adresse, ausweis) {
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
