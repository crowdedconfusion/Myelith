//! Der Konformitätslauf (Punkt 4.1).
//!
//! Prüft die Golden Vectors unter `INTEGER_LLM/conformance/vectors/`
//! gegen diesen Bau: die sechs Operations-Vektoren immer, die Layer- und
//! E2E-Vektoren nur, wenn das gewählte Artefakt zu ihnen passt.
//!
//! **Warum es diesen Lauf im Client gibt.** Die Vektoren waren vorher
//! nur über ein Shell-Skript erreichbar, das kein Protokoll schreibt.
//! Ein Konformitätsnachweis, den niemand vergleichen kann, ist eine
//! Terminalausgabe, die jemand abtippt — und damit kein Nachweis. Hier
//! entsteht wie bei jedem Lauf eine `.jsonl` mit einer Zeile je Vektor
//! und einem Gesamtwert, und `vergleich` urteilt darüber wie über die
//! anderen Vergleichswerte.
//!
//! ## Der Umfang gehört zum Messverfahren
//!
//! Zwei Läufe können verschiedene Mengen an Vektoren geprüft haben:
//! ohne Artefakt nur die sechs Operations-Vektoren, mit passendem
//! Artefakt alle dreiunddreißig. Der Gesamtwert trägt deshalb den
//! Umfang (`konformitaet_umfang`), und `vergleich` behandelt zwei
//! verschiedene Umfänge wie zwei verschiedene Modellstände:
//! unvergleichbar, ausdrücklich kein Hardware-Befund. Dasselbe Prinzip
//! wie `digest_umfang` (Fund 36).

use crate::logging::{sha256_hex, Event, RunLog};
use crate::runs;
use std::path::{Path, PathBuf};

/// Name des Vergleichswerts über den ganzen Lauf.
pub const WERT: &str = "konformitaet";

/// Name des Vergleichswerts der **Operations-Stufe allein**.
///
/// # ⚑ Warum es ihn seit dem 2026-09-04 gibt
///
/// Mit den Trainingsvektoren wächst der Umfang, und damit ändert sich
/// der Wert über den ganzen Lauf. **Der Wert der Operations-Stufe darf
/// sich davon nicht bewegen:** Er steht an sechs Stellen des
/// Repositoriums fest, darunter beide CI-Läufe, und eine bestehende
/// Zusage bricht man nicht, um eine neue aufzustellen.
///
/// ⚑ **Und je Stufe ein Wert grenzt eine Abweichung ohne zweiten Lauf
/// ein.** Weichen beide ab, sitzt es unter dem Modell; weicht nur der
/// Trainingswert ab, sitzt es im Rückwärtspass. Dieselbe Überlegung wie
/// bei `digest_umfang` und der Fallunterscheidung des Sammellaufs.
pub const WERT_OP: &str = "konformitaet_op";

/// Name des Vergleichswerts der **Trainings-Stufe allein**.
pub const WERT_TRAINING: &str = "konformitaet_training";

/// Name des Vergleichswerts des **MoE-Routingpfades**.
///
/// ⚑ **Seit dem 2026-09-05, und der Anlass ist Fund 180:**
/// `route_top_k` und `mische_experten` waren nirgends gegen ein festes
/// Soll geprüft. Das ist der Pfad, der entscheidet, **welche Experten
/// rechnen**; zwei Knoten, die verschieden routen, rechnen verschiedene
/// Netze, und der Redundanzvergleich meldete beide als fehlerhaft, ohne
/// dass einer gelogen hätte.
pub const WERT_MOE: &str = "konformitaet_moe";

/// Nur die Operations-Vektoren wurden geprüft.
pub const UMFANG_OP: &str = "op";
/// Operations- und Trainingsvektoren, ohne Modell.
pub const UMFANG_OHNE_MODELL: &str = "op+training+moe";
/// Alles: Operations-, Trainings-, Layer- und E2E-Vektoren.
pub const UMFANG_VOLL: &str = "op+training+moe+layer+e2e";

/// Das Manifest bei den Vektoren: welches Artefakt sie erzeugt hat.
///
/// Ohne diese Angabe könnte ein gewähltes Artefakt nur blind gegen die
/// Layer-/E2E-Vektoren laufen; ein falsches Modell „bestünde" dann nie
/// und „verfehlte" immer, und beides wäre keine Aussage über den Bau.
pub struct Manifest {
    pub modell: String,
    pub theta_v_hash: String,
}

/// Liest das Manifest. `None`, wenn es fehlt oder unbrauchbar ist: Ein
/// fehlendes Manifest ist kein Abbruchgrund, aber die Layer-/E2E-Stufe
/// muss dann ausfallen — sie weiß nicht, wofür sie gilt.
pub fn manifest_lesen(vektoren: &Path) -> Option<Manifest> {
    let inhalt = std::fs::read_to_string(vektoren.join("manifest.json")).ok()?;
    let modell = feld(&inhalt, "modell")?;
    let theta_v_hash = feld(&inhalt, "theta_v_hash")?;
    if modell.is_empty() {
        return None;
    }
    Some(Manifest { modell, theta_v_hash })
}

/// Liest ein Zeichenkettenfeld aus dem Manifest.
///
/// Derselbe Verzicht wie in [`crate::artefakte`]: Das Format ist flach
/// und selbstgeschrieben, ein JSON-Crate wäre eine Abhängigkeit für zwei
/// Felder. Unbekanntes bleibt stehen statt geraten zu werden.
fn feld(text: &str, schluessel: &str) -> Option<String> {
    let praefix = format!("\"{}\"", schluessel);
    for zeile in text.lines() {
        let z = zeile.trim();
        let Some(rest) = z.strip_prefix(&praefix) else { continue };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix(':') else { continue };
        let rest = rest.trim().trim_end_matches(',').trim();
        let inner = rest.strip_prefix('"')?.strip_suffix('"')?;
        return Some(inner.to_string());
    }
    None
}

/// Was geprüft wird, als reine Entscheidung.
///
/// Die Operations-Vektoren brauchen kein Modell und laufen immer. Layer
/// und E2E sind mit einem bestimmten Artefakt erzeugt; läuft ein anderes
/// dagegen, misst der Lauf etwas, das er nicht messen kann. Dann wird
/// übersprungen — ausdrücklich und protokolliert, nicht still.
pub struct Entscheidung {
    pub layer_e2e: bool,
    /// Warum übersprungen wurde; leer, wenn gelaufen wird.
    pub begruendung: String,
}

pub fn entscheide_layer_e2e(artefakt_name: Option<&str>, manifest: Option<&Manifest>) -> Entscheidung {
    let Some(artefakt) = artefakt_name else {
        return Entscheidung {
            layer_e2e: false,
            begruendung: "kein Artefakt gewählt; nur die Operations-Vektoren laufen".to_string(),
        };
    };
    let Some(manifest) = manifest else {
        return Entscheidung {
            layer_e2e: false,
            begruendung: format!(
                "die Vektoren tragen kein Manifest; nicht entscheidbar, ob {} zu ihnen passt",
                artefakt
            ),
        };
    };
    if artefakt == manifest.modell {
        Entscheidung { layer_e2e: true, begruendung: String::new() }
    } else {
        Entscheidung {
            layer_e2e: false,
            begruendung: format!(
                "das Artefakt {} passt nicht zum Modell {} der Layer-/E2E-Vektoren",
                artefakt, manifest.modell
            ),
        }
    }
}

/// Ein geprüftes Ergebnis, eine Zeile der kanonischen Bytefolge.
///
/// Der Name trägt die Ebene, damit ein Layer-Vektor und ein
/// Operations-Vektor desselben Namens sich nie vermischen.
pub fn zeile(ebene: &str, name: &str, bestanden: bool) -> String {
    format!(
        "{}/{}:{}",
        ebene,
        name,
        if bestanden { "bestanden" } else { "fehlschlagen" }
    )
}

/// Der Vergleichswert des Laufs: SHA-256 über die **sortierten**
/// Ergebniszeilen.
///
/// Sortiert, weil die Verzeichnisreihenfolge zwischen Maschinen und
/// Dateisystemen nicht verbürgt ist; der Digest darf davon nicht
/// abhängen. Nicht über eine formatierte Ausgabe: Formatierung ändert
/// sich, Bytes nicht.
pub fn digest_aus_ergebnissen(ergebnisse: &[(String, bool)]) -> String {
    let mut zeilen: Vec<String> = ergebnisse
        .iter()
        .map(|(name, bestanden)| {
            // Name trägt die Ebene bereits, siehe Sammelschleife.
            format!("{}:{}", name, if *bestanden { "bestanden" } else { "fehlschlagen" })
        })
        .collect();
    zeilen.sort();
    sha256_hex(zeilen.join("\n").as_bytes())
}

/// Findet die Vektor-Dateien einer Ebene, aufsteigend sortiert.
fn vektor_dateien(vektoren: &Path, ebene: &str) -> Vec<PathBuf> {
    let dir = vektoren.join(ebene);
    let Ok(eintraege) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut dateien: Vec<PathBuf> = eintraege
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".golden.json"))
        .collect();
    dateien.sort();
    dateien
}

/// Der Konformitätslauf.
///
/// `artefakt` ist das gewählte Artefaktverzeichnis, falls eines gewählt
/// wurde. Ohne Artefakt läuft nur die Operations-Stufe; ein Download
/// findet hier nie statt (der Lauf fragt nicht, er misst).
pub fn laufen(log: &mut RunLog, artefakt: Option<&Path>) -> bool {
    // Dieselbe Sperre wie vor den anderen Messläufen: Ein Bau, der für
    // ein delegierendes Backend konfiguriert ist, würde die Referenz
    // unter fremdem Namen zertifizieren (Fund 33/34).
    if !runs::backend_taugt(log) {
        return false;
    }

    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let vektoren = repo.join("INTEGER_LLM/conformance/vectors");
    if !vektoren.is_dir() {
        log.error(format!(
            "{} fehlt: ohne Vektoren kein Konformitätslauf",
            vektoren.display()
        ));
        return false;
    }

    // Hardware und Fingerabdruck wie bei jedem Lauf: `vergleich` braucht
    // beides, sonst könnte es die Maschinen nicht unterscheiden.
    runs::log_context(log, artefakt);

    let artefakt_name = artefakt.and_then(|p| {
        p.file_name().map(|n| n.to_string_lossy().into_owned())
    });
    let manifest = manifest_lesen(&vektoren);
    let entscheidung = entscheide_layer_e2e(artefakt_name.as_deref(), manifest.as_ref());

    let mut ergebnisse: Vec<(String, bool)> = Vec::new();
    let mut bestanden = 0usize;
    let mut gesamt = 0usize;

    // Stufe 1: Operations-Vektoren, ohne Modell.
    let op_dateien = vektor_dateien(&vektoren, "op");
    if op_dateien.is_empty() {
        log.error(format!("keine Operations-Vektoren unter {}", vektoren.join("op").display()));
        return false;
    }
    for pfad in &op_dateien {
        match integer_llm_kernels::konformitaet::op_vektor_aus_datei(pfad) {
            Ok(e) => {
                gesamt += 1;
                if e.bestanden {
                    bestanden += 1;
                } else {
                    for grund in &e.gruende {
                        log.error(format!("{}: {}", e.name, grund));
                    }
                }
                let name = zeile("op", &e.name, e.bestanden);
                ergebnisse.push((name.clone(), e.bestanden));
                log.event(Event::Step {
                    name: format!("konformitaet_{}", name),
                    millis: 0,
                    detail: if e.bestanden { "bestanden".into() } else { "fehlschlagen".into() },
                });
            }
            Err(e) => {
                log.error(format!("{}: {}", pfad.display(), e));
                return false;
            }
        }
    }

    // ⚑ **Der Wert der Operations-Stufe, bevor die naechste dazukommt.**
    // Er ist die bestehende Zusage und darf sich nicht bewegen, wenn der
    // Umfang waechst.
    log.result(
        WERT_OP,
        &digest_aus_ergebnissen(&ergebnisse),
        format!("{}/{}", bestanden, gesamt),
    );
    let (op_bestanden, op_gesamt) = (bestanden, gesamt);

    // Stufe 2: der Trainingspfad, ebenfalls ohne Modell.
    //
    // ⚑ **Fuenf Kerne, die bis zum 2026-09-04 kein Vektor deckte.** Drei
    // der acht Rueckwaertskerne rechneten falsch, und der Prueflauf, der
    // 33 von 33 meldete, hat keinen von ihnen je gerechnet.
    let training_dateien = vektor_dateien(&vektoren, "training");
    if training_dateien.is_empty() {
        log.error(format!(
            "keine Trainingsvektoren unter {}",
            vektoren.join("training").display()
        ));
        return false;
    }
    for pfad in &training_dateien {
        match integer_llm_kernels::konformitaet::trainingsvektor_aus_datei(pfad) {
            Ok(e) => {
                gesamt += 1;
                if e.bestanden {
                    bestanden += 1;
                } else {
                    for grund in &e.gruende {
                        log.error(format!("{}: {}", e.name, grund));
                    }
                }
                let name = zeile("training", &e.name, e.bestanden);
                ergebnisse.push((name.clone(), e.bestanden));
                log.event(Event::Step {
                    name: format!("konformitaet_{}", name),
                    millis: 0,
                    detail: if e.bestanden { "bestanden".into() } else { "fehlschlagen".into() },
                });
            }
            Err(e) => {
                log.error(format!("{}: {}", pfad.display(), e));
                return false;
            }
        }
    }
    log.result(
        WERT_TRAINING,
        &digest_aus_ergebnissen(&ergebnisse[op_gesamt..]),
        format!("{}/{}", bestanden - op_bestanden, gesamt - op_gesamt),
    );

    // --- MoE: der Routingpfad (Fund 180) -----------------------------
    //
    // ⚑ **Braucht kein Modell**, wie die Operations- und
    // Trainingsvektoren: feste Logits, feste Tabelle, feste Sollwerte.
    let vor_moe_gesamt = gesamt;
    let vor_moe_bestanden = bestanden;
    let vor_moe_ergebnisse = ergebnisse.len();
    let moe_dateien = vektor_dateien(&vektoren, "moe");
    if moe_dateien.is_empty() {
        log.error(format!(
            "keine MoE-Vektoren unter {}",
            vektoren.join("moe").display()
        ));
        return false;
    }
    for pfad in &moe_dateien {
        match integer_llm_kernels::konformitaet::moe_vektor_aus_datei(pfad) {
            Ok(e) => {
                gesamt += 1;
                if e.bestanden {
                    bestanden += 1;
                } else {
                    for grund in &e.gruende {
                        log.error(format!("{}: {}", e.name, grund));
                    }
                }
                let name = zeile("moe", &e.name, e.bestanden);
                ergebnisse.push((name.clone(), e.bestanden));
                log.event(Event::Step {
                    name: format!("konformitaet_{}", name),
                    millis: 0,
                    detail: if e.bestanden { "bestanden".into() } else { "fehlschlagen".into() },
                });
            }
            Err(e) => {
                log.error(format!("{}: {}", pfad.display(), e));
                return false;
            }
        }
    }
    log.result(
        WERT_MOE,
        &digest_aus_ergebnissen(&ergebnisse[vor_moe_ergebnisse..]),
        format!("{}/{}", bestanden - vor_moe_bestanden, gesamt - vor_moe_gesamt),
    );

    let umfang = if entscheidung.layer_e2e {
        // Stufe 2 und 3 gegen das gewählte Artefakt. Das Modell wird
        // **einmal** geladen: Bei 0,5B kostet die Ladung ein Vielfaches
        // der einzelnen Vektorprüfung.
        let Some(dir) = artefakt else { unreachable!("ohne Artefakt keine Layer/E2E-Stufe") };
        let modell = log.timed(
            "konformitaet_modell_laden",
            &dir.display().to_string(),
            || integer_llm_runtime::loader::load_model(dir),
        );
        let modell = match modell {
            Ok(m) => m,
            Err(e) => {
                log.error(format!("Modell-Ladung fehlgeschlagen: {}", e));
                return false;
            }
        };
        for ebene in ["layer", "e2e"] {
            for pfad in vektor_dateien(&vektoren, ebene) {
                match integer_llm_runtime::konformitaet::vektor_aus_datei(&modell, &pfad) {
                    Ok(e) => {
                        gesamt += 1;
                        if e.bestanden {
                            bestanden += 1;
                        } else {
                            for grund in &e.gruende {
                                log.error(format!("{}: {}", e.name, grund));
                            }
                        }
                        let name = zeile(ebene, &e.name, e.bestanden);
                        ergebnisse.push((name.clone(), e.bestanden));
                        log.event(Event::Step {
                            name: format!("konformitaet_{}", name),
                            millis: 0,
                            detail: if e.bestanden { "bestanden".into() } else { "fehlschlagen".into() },
                        });
                    }
                    Err(e) => {
                        log.error(format!("{}: {}", pfad.display(), e));
                        return false;
                    }
                }
            }
        }
        UMFANG_VOLL
    } else {
        log.note(format!(
            "Layer- und E2E-Vektoren übersprungen: {}",
            entscheidung.begruendung
        ));
        UMFANG_OHNE_MODELL
    };

    // Der Umfang ist Teil des Messverfahrens und steht bei der Hardware,
    // wie `digest_umfang`: zwei verschiedene Umfänge messen verschiedene
    // Dinge und sind unvergleichbar.
    log.event(Event::Hardware {
        key: "konformitaet_umfang".into(),
        value: umfang.to_string(),
    });

    let wert = format!("{}/{}", bestanden, gesamt);
    log.note(format!(
        "Konformität {}: {} Vektoren geprüft",
        wert, umfang
    ));
    log.result(WERT, &digest_aus_ergebnissen(&ergebnisse), wert);

    bestanden == gesamt
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(modell: &str) -> Manifest {
        Manifest { modell: modell.to_string(), theta_v_hash: "sha256:00".to_string() }
    }

    /// Die Kernentscheidung: Nur das Artefakt, mit dem die Vektoren
    /// erzeugt wurden, darf gegen sie laufen.
    #[test]
    fn passendes_modell_laeuft() {
        let e = entscheide_layer_e2e(Some("qwen2.5-0.5b"), Some(&manifest("qwen2.5-0.5b")));
        assert!(e.layer_e2e);
        assert!(e.begruendung.is_empty());
    }

    /// Ein abweichender Name ist kein Fehler, aber ein ehrliches
    /// Überspringen — mit Begründung, nicht still.
    #[test]
    fn abweichendes_modell_wird_uebersprungen() {
        let e = entscheide_layer_e2e(Some("qwen2.5-7b"), Some(&manifest("qwen2.5-0.5b")));
        assert!(!e.layer_e2e);
        assert!(e.begruendung.contains("qwen2.5-7b"));
        assert!(e.begruendung.contains("qwen2.5-0.5b"));
    }

    /// Ohne Artefakt laufen nur die Operations-Vektoren: der Normalfall
    /// auf einer frischen Maschine.
    #[test]
    fn ohne_artefakt_nur_op() {
        let e = entscheide_layer_e2e(None, Some(&manifest("qwen2.5-0.5b")));
        assert!(!e.layer_e2e);
        assert!(e.begruendung.contains("kein Artefakt"));
    }

    /// Ohne Manifest ist nicht entscheidbar, wofür die Vektoren gelten.
    /// Dann lieber ehrlich überspringen als blind laden.
    #[test]
    fn ohne_manifest_wird_uebersprungen() {
        let e = entscheide_layer_e2e(Some("qwen2.5-0.5b"), None);
        assert!(!e.layer_e2e);
        assert!(e.begruendung.contains("kein Manifest"));
    }

    /// Der Digest darf von der Reihenfolge der Ergebnisse nicht abhängen:
    /// Verzeichnisreihenfolgen sind zwischen Maschinen nicht verbürgt.
    #[test]
    fn digest_ist_unabhaengig_von_der_reihenfolge() {
        let a = vec![
            ("op/rmsnorm_basic".to_string(), true),
            ("op/softmax_basic".to_string(), true),
            ("layer/transformer_layer_0".to_string(), true),
        ];
        let b = vec![a[2].clone(), a[0].clone(), a[1].clone()];
        assert_eq!(digest_aus_ergebnissen(&a), digest_aus_ergebnissen(&b));
    }

    /// **Die Gegenprobe.** Ein einzelnes anderes Ergebnis muss den Digest
    /// ändern — sonst wäre der Gesamtwert blind für den Fall, für den er
    /// da ist.
    #[test]
    fn ein_fehlschlag_aendert_den_digest() {
        let alle_gut = vec![
            ("op/rmsnorm_basic".to_string(), true),
            ("op/softmax_basic".to_string(), true),
        ];
        let einer_fehlt = vec![
            ("op/rmsnorm_basic".to_string(), true),
            ("op/softmax_basic".to_string(), false),
        ];
        assert_ne!(digest_aus_ergebnissen(&alle_gut), digest_aus_ergebnissen(&einer_fehlt));
    }

    /// Die Zeilenform ist der Vertrag zwischen Lauf und Digest; sie trägt
    /// die Ebene, damit sich gleichnamige Vektoren verschiedener Ebenen
    /// nie vermischen.
    #[test]
    fn zeile_traegt_ebene_und_ausgang() {
        assert_eq!(zeile("op", "rmsnorm_basic", true), "op/rmsnorm_basic:bestanden");
        assert_eq!(zeile("e2e", "e2e_hello", false), "e2e/e2e_hello:fehlschlagen");
    }

    /// Der Manifest-Leser ist absichtlich klein; er darf sich an fremden
    /// Feldern nicht stoßen und an fehlenden nicht raten.
    #[test]
    fn manifest_lesen_trennt_bekannt_von_unbekannt() {
        let dir = std::env::temp_dir().join(format!("myl-testclient-manifest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("manifest.json"),
            "{\n  \"modell\": \"qwen2.5-0.5b\",\n  \"theta_v_hash\": \"sha256:ab\",\n  \"extra\": 1\n}\n",
        )
        .unwrap();
        let m = manifest_lesen(&dir).expect("Manifest lesbar");
        assert_eq!(m.modell, "qwen2.5-0.5b");
        assert_eq!(m.theta_v_hash, "sha256:ab");

        // Ohne Datei: ehrlich None, kein geratenes Modell.
        std::fs::remove_file(dir.join("manifest.json")).unwrap();
        assert!(manifest_lesen(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Das echte Manifest des Repositoriums muss lesbar sein und das
    /// Modell nennen, mit dem die Vektoren erzeugt wurden.
    #[test]
    fn manifest_des_repositoriums_ist_lesbar() {
        let repo = crate::artefakte::wurzel_zur_laufzeit(Path::new("."));
        let vektoren = repo.join("INTEGER_LLM/conformance/vectors");
        if !vektoren.is_dir() {
            eprintln!("SKIP: {} nicht vorhanden", vektoren.display());
            return;
        }
        let m = manifest_lesen(&vektoren).expect("Manifest muss vorhanden sein");
        assert_eq!(m.modell, "qwen2.5-0.5b");
        assert!(m.theta_v_hash.starts_with("sha256:"));
    }

    /// Die Operations-Vektoren des Repositoriums müssen über den Client
    /// bestehen: Das ist die CI-feste Hälfte des Konformitätslaufs, sie
    /// braucht weder Artefakt noch Modell.
    #[test]
    fn op_vektoren_bestehen_ueber_den_client_pfad() {
        let repo = crate::artefakte::wurzel_zur_laufzeit(Path::new("."));
        let vektoren = repo.join("INTEGER_LLM/conformance/vectors");
        if !vektoren.is_dir() {
            eprintln!("SKIP: {} nicht vorhanden", vektoren.display());
            return;
        }
        let dateien = vektor_dateien(&vektoren, "op");
        assert_eq!(dateien.len(), 6);
        for pfad in &dateien {
            let e = integer_llm_kernels::konformitaet::op_vektor_aus_datei(pfad)
                .expect("prüfbar");
            assert!(e.bestanden, "{}: {:?}", pfad.display(), e.gruende);
        }
    }
}
