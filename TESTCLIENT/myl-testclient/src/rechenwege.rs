//! Die Rechenwege dieser Maschine, als siebte Stufe des Sammellaufs.
//!
//! # ⚑ Was geprüft wird (2026-09-14)
//!
//! **Dieselbe Rechnung auf jedem Rechenweg, den diese Maschine bietet**:
//! über die skalare Referenz, über die vektorisierte Fassung, über die GPU
//! und auf einem einzigen Kern. Alle müssen denselben Abdruck liefern,
//! Logits und Token jedes Schritts, so wie der Determinismuslauf ihn
//! bildet.
//!
//! Bis zu diesem Tag entschied sich der Rechenweg ausschließlich beim
//! Übersetzen, und ein Vergleich zweier Wege brauchte zwei Bauten und zwei
//! Läufe. Seitdem schalten `kernels::dot::skalar_erzwingen`,
//! `kernels::metal::schwelle_setzen` und `kernels::linear::kerngrenze_setzen`
//! zur Laufzeit um, und ein Lauf prüft alle.
//!
//! # ⚑ Ein Vergleichswert, nicht einer je Weg
//!
//! `vergleich` führt einen Lauf, der andere Vergleichswerte abdeckt als
//! die übrigen, als unvollständig (Fund 35). Eine Maschine ohne GPU hätte
//! einen Wert weniger als eine mit, und zwei ehrliche Läufe wären nicht
//! mehr vergleichbar. **Der Wert ist deshalb der Abdruck der Referenz**,
//! und er steht nur dann als bestanden da, wenn jeder Weg ihn getroffen
//! hat. Welche Wege es waren, steht im Klartext daneben und in je einem
//! Messschritt.
//!
//! # ⚑ Belegt, nicht behauptet (Lehre aus Fund 33)
//!
//! Jeder Weg zeigt, dass er der ist, der er zu sein behauptet: Der Weg
//! über die GPU muss dort Bündel gerechnet haben, jeder andere darf es
//! nicht; der skalare Weg muss auf einer vektorisierenden Übersetzung
//! erzwungen skalar gerechnet haben; der Weg auf einem Kern muss die
//! Kerngrenze eins sehen.
//!
//! # ⚑ Der Zeitrahmen
//!
//! Die Stufe soll nicht lange dauern. Die Wege laufen vom schnellsten zum
//! langsamsten; **der Weg auf einem Kern wird vorab geschätzt** (die Dauer
//! desselben Wegs auf allen Kernen mal die Kernzahl) und übersprungen,
//! wenn er nicht mehr in [`ZEITRAHMEN`] passt. Das trifft bei den großen
//! Modellen zu, und die Meldung sagt es. Die Referenz wird nie
//! übersprungen: Sie ist der Maßstab.

use crate::logging::{sha256_hex, Event, RunLog};
use integer_llm_kernels::{dot, linear, metal};
use std::path::Path;
use std::time::{Duration, Instant};

/// Name des Vergleichswerts.
pub const WERT: &str = "rechenwege";

/// Der Text, der auf jedem Weg gerechnet wird.
///
/// ⚑ **Lang genug, dass die Vorbereitung bündelt und die GPU mehr als eine
/// Kachel füllt** (gut vierzig Token; eine Kachel sind sechzehn
/// Eingaben), und fest im Programm, damit zwei Maschinen mit derselben
/// Fassung dieselbe Arbeit rechnen. Sein Hash steht im Protokoll.
pub const ARBEIT: &str = "Myelith prüft, ob jede Maschine dieselben Zahlen rechnet. \
Derselbe Text läuft deshalb über jeden Rechenweg dieser Maschine: über die skalare \
Referenz, über die vektorisierte Fassung, über einen einzigen Kern und über alle Kerne, \
und wo eine Grafikkarte da ist, auch über sie. Weicht ein Weg ab, rechnet diese \
Maschine anders.";

/// Wie viele Token nach dem Text erzeugt werden. Der Decode rechnet auf
/// der CPU, die Vorbereitung auch auf der GPU; beide gehen in den Abdruck.
pub const SCHRITTE: usize = 8;

/// Wie lange die Stufe ohne das Laden des Modells höchstens dauern soll.
pub const ZEITRAHMEN: Duration = Duration::from_secs(60);

/// Ein Rechenweg und wie er eingestellt wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weg {
    /// Name im Protokoll.
    pub name: &'static str,
    /// Die skalare Fassung erzwingen.
    pub skalar: bool,
    /// Die GPU jede Bündelung rechnen lassen; sonst ist sie abgeschaltet.
    pub gpu: bool,
    /// Ein Kern statt aller.
    pub ein_kern: bool,
}

/// Die Wege dieser Maschine, vom schnellsten zum langsamsten.
///
/// ⚑ **Aus dem, was rechnet, nicht aus dem, was übersetzt ist**: Die
/// Angaben kommen aus `kernels` (`dot::VEKTORISIERT`, `metal::verfuegbar`).
/// Ein Weg auf einem Kern entfällt, wenn die Maschine ohnehin nur einen
/// hat; er wäre derselbe wie der auf allen.
pub fn wege(vektorisiert: bool, gpu: bool, kerne: usize) -> Vec<Weg> {
    let mut w = Vec::new();
    if gpu {
        w.push(Weg { name: "metal", skalar: false, gpu: true, ein_kern: false });
    }
    if vektorisiert {
        w.push(Weg { name: "cpu-simd", skalar: false, gpu: false, ein_kern: false });
    }
    w.push(Weg { name: "reference", skalar: true, gpu: false, ein_kern: false });
    if kerne > 1 {
        w.push(if vektorisiert {
            Weg { name: "cpu-simd, ein Kern", skalar: false, gpu: false, ein_kern: true }
        } else {
            Weg { name: "reference, ein Kern", skalar: true, gpu: false, ein_kern: true }
        });
    }
    w
}

/// Geschätzte Dauer eines Wegs, oder `None`, wenn er ohne Schätzung läuft.
///
/// Nur der Weg auf einem Kern wird geschätzt: Dauer desselben Wegs auf
/// allen Kernen mal die Kernzahl. Das ist eine obere Grenze, denn kein
/// Teil der Rechnung wird mit mehr Kernen langsamer.
pub fn schaetzung(weg: &Weg, gemessen: &[(Weg, Duration)], kerne: usize) -> Option<Duration> {
    if !weg.ein_kern {
        return None;
    }
    gemessen
        .iter()
        .find(|(w, _)| !w.ein_kern && !w.gpu && w.skalar == weg.skalar)
        .map(|(_, d)| d.saturating_mul(kerne as u32))
}

/// Die Einstellung vor der Stufe, zum Zurücksetzen.
struct Stand {
    schwelle: usize,
}

fn einstellen(weg: &Weg) {
    dot::skalar_erzwingen(weg.skalar);
    metal::schwelle_setzen(if weg.gpu { 1 } else { 0 });
    linear::kerngrenze_setzen(if weg.ein_kern { 1 } else { 0 });
}

/// ⚑ **Die Kerngrenze geht auf null, also auf die ganze Maschine**: Der
/// Testclient setzt sie sonst nirgends.
fn zuruecksetzen(stand: &Stand) {
    dot::skalar_erzwingen(false);
    metal::schwelle_setzen(stand.schwelle);
    linear::kerngrenze_setzen(0);
}

/// Die Stufe.
pub fn laufen(log: &mut RunLog, artefakt: &Path) -> bool {
    if !crate::runs::backend_taugt(log) {
        return false;
    }
    let modell = log.timed("rechenwege_modell_laden", &artefakt.display().to_string(), || {
        integer_llm_runtime::loader::load_model(artefakt)
    });
    let modell = match modell {
        Ok(m) => m,
        Err(e) => {
            log.error(format!("Modell-Ladung fehlgeschlagen: {e}"));
            return false;
        }
    };
    let ids: Vec<usize> = match crate::runs::encode_prompt(artefakt, ARBEIT) {
        Ok(v) => v.into_iter().map(|t| t as usize).collect(),
        Err(e) => {
            log.error(e);
            return false;
        }
    };
    log.event(Event::PromptAccepted {
        token_count: ids.len(),
        prompt_sha256: sha256_hex(ARBEIT.as_bytes()),
    });
    if ids.len() <= 16 {
        log.note(format!(
            "der Text ergibt mit diesem Tokenizer nur {} Token und füllt keine GPU-Kachel",
            ids.len()
        ));
    }

    let vektorisiert = dot::VEKTORISIERT;
    let gpu = metal::verfuegbar();
    let kerne = linear::kerngrenze();
    if let Some(grund) = metal::nicht_verfuegbar_weil() {
        log.note(format!("GPU nicht dabei: {grund}"));
    }
    let wege = wege(vektorisiert, gpu, kerne);

    let stand = Stand { schwelle: metal::schwelle() };
    let beginn = Instant::now();
    let mut dauern: Vec<(Weg, Duration)> = Vec::new();
    let mut abdruecke: Vec<(Weg, String)> = Vec::new();
    let mut ok = true;

    for weg in &wege {
        if let Some(geschaetzt) = schaetzung(weg, &dauern, kerne) {
            let rest = ZEITRAHMEN.saturating_sub(beginn.elapsed());
            if geschaetzt > rest {
                log.note(format!(
                    "{}: übersprungen, geschätzt {} s, im Zeitrahmen bleiben {} s",
                    weg.name,
                    geschaetzt.as_secs(),
                    rest.as_secs()
                ));
                continue;
            }
        }

        einstellen(weg);
        let kerne_im_weg = linear::kerngrenze();
        let gpu_vorher = metal::gerechnet();
        let skalar_vorher = dot::erzwungen_skalar_gerechnet();
        let t0 = Instant::now();
        let (_, abdruck) =
            integer_llm_runtime::generate::dekodieren_mit_digest(&modell, &ids, SCHRITTE, 0, true);
        let dauer = t0.elapsed();
        let gpu_buendel = metal::gerechnet() - gpu_vorher;
        let skalar_gerechnet = dot::erzwungen_skalar_gerechnet() - skalar_vorher;
        zuruecksetzen(&stand);

        log.event(Event::Step {
            name: format!("rechenweg {}", weg.name),
            millis: dauer.as_millis() as u64,
            detail: format!(
                "Abdruck {}, {} Kern(e), GPU-Bündel {}, erzwungen skalar {}",
                &abdruck[..16.min(abdruck.len())],
                kerne_im_weg,
                gpu_buendel,
                skalar_gerechnet
            ),
        });

        // Die Belege, siehe Modulkopf.
        if weg.gpu && gpu_buendel == 0 {
            log.error(format!("{}: die GPU hat nicht gerechnet", weg.name));
            ok = false;
        }
        if !weg.gpu && gpu_buendel > 0 {
            log.error(format!("{}: die GPU hat gerechnet, obwohl sie abgeschaltet war", weg.name));
            ok = false;
        }
        if weg.skalar && vektorisiert && skalar_gerechnet == 0 {
            log.error(format!("{}: die skalare Fassung wurde nicht benutzt", weg.name));
            ok = false;
        }
        if weg.ein_kern && kerne_im_weg != 1 {
            log.error(format!("{}: die Kerngrenze stand auf {kerne_im_weg}", weg.name));
            ok = false;
        }
        dauern.push((*weg, dauer));
        abdruecke.push((*weg, abdruck));
    }
    zuruecksetzen(&stand);

    let Some((_, referenz)) = abdruecke.iter().find(|(w, _)| w.name == "reference").cloned() else {
        log.error("die Referenz hat nicht gerechnet");
        return false;
    };
    let mut gleich = true;
    for (weg, abdruck) in &abdruecke {
        if *abdruck != referenz {
            gleich = false;
            log.event(Event::Mismatch {
                name: format!("rechenweg {}", weg.name),
                expected: referenz.clone(),
                actual: abdruck.clone(),
            });
            log.error(format!("{} rechnet anders als die Referenz", weg.name));
        }
    }
    let namen: Vec<&str> = abdruecke.iter().map(|(w, _)| w.name).collect();
    log.result(
        WERT,
        &referenz,
        if gleich {
            format!("{} Rechenwege bitgleich: {}", namen.len(), namen.join("; "))
        } else {
            format!("ABWEICHUNG unter {}", namen.join("; "))
        },
    );
    ok && gleich
}

#[cfg(test)]
mod tests {
    use super::*;

    fn namen(w: &[Weg]) -> Vec<&'static str> {
        w.iter().map(|w| w.name).collect()
    }

    /// **Die Referenz ist immer dabei**, und jeder Name kommt einmal vor.
    #[test]
    fn die_referenz_ist_immer_dabei() {
        for vekt in [false, true] {
            for gpu in [false, true] {
                for kerne in [1, 2, 15] {
                    let w = wege(vekt, gpu, kerne);
                    let n = namen(&w);
                    assert!(n.contains(&"reference"), "{vekt} {gpu} {kerne}: {n:?}");
                    let eindeutig: std::collections::HashSet<_> = n.iter().collect();
                    assert_eq!(eindeutig.len(), n.len());
                    assert_eq!(n.contains(&"metal"), gpu);
                    assert_eq!(n.contains(&"cpu-simd"), vekt);
                    assert_eq!(w.iter().any(|w| w.ein_kern), kerne > 1);
                }
            }
        }
    }

    /// **Vom schnellsten zum langsamsten**, und nur ein Weg rechnet auf der GPU.
    #[test]
    fn die_reihenfolge_steht_fest() {
        assert_eq!(
            namen(&wege(true, true, 15)),
            ["metal", "cpu-simd", "reference", "cpu-simd, ein Kern"]
        );
        assert_eq!(namen(&wege(false, false, 4)), ["reference", "reference, ein Kern"]);
        assert_eq!(wege(true, true, 15).iter().filter(|w| w.gpu).count(), 1);
    }

    /// **Nur der Weg auf einem Kern wird geschätzt**, aus dem passenden
    /// Weg auf allen Kernen.
    #[test]
    fn die_schaetzung_nimmt_den_passenden_weg() {
        let w = wege(true, true, 10);
        let gemessen = vec![
            (w[0], Duration::from_millis(100)),
            (w[1], Duration::from_millis(300)),
            (w[2], Duration::from_millis(900)),
        ];
        assert_eq!(schaetzung(&w[0], &gemessen, 10), None);
        assert_eq!(schaetzung(&w[2], &gemessen, 10), None);
        assert_eq!(schaetzung(&w[3], &gemessen, 10), Some(Duration::from_millis(3000)));

        let r = wege(false, false, 4);
        let gemessen = vec![(r[0], Duration::from_secs(20))];
        assert_eq!(schaetzung(&r[1], &gemessen, 4), Some(Duration::from_secs(80)));
        assert!(schaetzung(&r[1], &gemessen, 4).unwrap() > ZEITRAHMEN);
    }

    /// **Ein grober Riegel gegen einen versehentlich gekürzten Text.** Ob
    /// er mit dem Tokenizer des Artefakts eine GPU-Kachel füllt, prüft die
    /// Stufe selbst und meldet es.
    #[test]
    fn der_text_ist_nicht_gekuerzt() {
        assert!(ARBEIT.chars().count() > 200);
    }
}
