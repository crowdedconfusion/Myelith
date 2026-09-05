//! Der Trainingsschritt als sechste Stufe des Sammellaufs.
//!
//! # ⚑ Warum diese Stufe existiert
//!
//! Die fünf Stufen davor belegen, dass zwei Maschinen dieselbe
//! **Inferenz** rechnen. Die Kernthese des Projekts trägt aber eine
//! zweite Hälfte: Auch das **Training** muss nachrechenbar sein, sonst
//! ist bezahlte Trainingsarbeit unprüfbar.
//!
//! Der Konformitätslauf prüft dafür einzelne Rückwärtskerne gegen feste
//! Sollvektoren. Was er nicht prüft, ist der Weg als Ganzes: sechs
//! Positionen durch vierundzwanzig eingefrorene Ebenen, eine trainierte
//! Ebene, Kopf, Softmax, Kreuzentropie, und dreissig Schritte zurück in
//! fünfzehn Millionen Gewichte. ⚑ **Dafür gibt es kein Soll und kann
//! keins geben**, denn die Sollwerte wären die Antwort auf genau die
//! Frage. Also vergleichen die Maschinen ihre Ergebnisse miteinander.
//!
//! # ⚑ Was der Abdruck belegt und was nicht
//!
//! **Belegt:** Zwei Maschinen haben denselben Zustand erreicht.
//! **Nicht belegt:** dass er richtig ist. Zwei Maschinen, die denselben
//! Fehler machen, bekommen denselben Abdruck. Das ist kein Mangel des
//! Verfahrens, sondern seine Aufgabe: Verifizieren heisst nachrechnen,
//! und nachrechnen heisst dasselbe herausbekommen. Ob das Ergebnis
//! **richtig** ist, sagen die Konformitätsvektoren.
//!
//! ⚑ **Und der Lauf belegt nicht, dass ein Modell lernt.** Eine Folge,
//! ein Zielwort. Der fallende Verlust zeigt, dass ein echtes Ziel echte
//! Gewichte bewegt, mehr nicht.

use crate::logging::{Event, RunLog};
use integer_llm_runtime::trainingsschleife::{trainingsschleife, Trainingsvorgaben};
use std::path::Path;

/// Name des Vergleichswerts.
pub const WERT: &str = "training";

/// Der Trainingslauf.
///
/// ⚑ **Die Vorgaben kommen aus der Bibliothek, nicht von hier.** Zwei
/// Maschinen müssen dieselben rechnen, sonst vergleichen sie zwei
/// verschiedene Arbeiten; eine zweite getippte Fassung derselben Zahlen
/// wäre genau der Weg, auf dem das schiefgeht.
pub fn laufen(log: &mut RunLog, artefakt: &Path) -> bool {
    // Dieselbe Sperre wie vor den anderen Messläufen: Ein Bau, der für
    // ein delegierendes Backend konfiguriert ist, würde die Referenz
    // unter fremdem Namen zertifizieren (Fund 33/34).
    if !crate::runs::backend_taugt(log) {
        return false;
    }

    let modell = log.timed("training_modell_laden", &artefakt.display().to_string(), || {
        integer_llm_runtime::loader::load_model(artefakt)
    });
    let modell = match modell {
        Ok(m) => m,
        Err(e) => {
            log.error(format!("Modell-Ladung fehlgeschlagen: {e}"));
            return false;
        }
    };

    let v = Trainingsvorgaben::vorgabe();
    // ⚑ **Die Vorgaben stehen im Protokoll, nicht nur im Quelltext.**
    // Der Abdruck ist nur dann ein Vergleichswert, wenn beide Maschinen
    // dieselbe Arbeit gerechnet haben. Weichen die Abdrücke ab, ist dies
    // die Zeile, die zuerst gelesen wird.
    log.event(Event::Hardware {
        key: "training_vorgaben".into(),
        value: format!(
            "folge={:?} ziel={} schritte={} lr_nenner={} logit_frac={} prob_frac={}",
            v.folge, v.ziel, v.schritte, v.lr_nenner, v.logit_frac, v.prob_frac
        ),
    });

    let ergebnis = log.timed(
        "training_schleife",
        &format!("{} Schritte auf der letzten Ebene", v.schritte),
        || trainingsschleife(&modell, &v),
    );
    let e = match ergebnis {
        Ok(e) => e,
        Err(grund) => {
            log.error(format!("Trainingsschritt nicht möglich: {grund}"));
            return false;
        }
    };

    let (argmax_vorher, argmax_nachher) = e.argmax();
    log.note(format!(
        "Ebene {}: {} von {} Gewichten bewegt",
        e.ebene, e.bewegte_gewichte, e.gewichte_gesamt
    ));
    log.note(format!(
        "Vorhersage des nächsten Wortes: {argmax_vorher} -> {argmax_nachher} (Ziel {})",
        v.ziel
    ));
    // ⚑ **Nur bei einem Expertengemisch, und dort ist es die Zahl, die
    // zählt.** Ein Experte, den der Router nie wählt, bekommt nie einen
    // Gradienten und ist still tot; wie viele überhaupt drankamen, sagt
    // mehr über den Lauf als die Zahl der bewegten Gewichte.
    if let Some((beruehrt, gesamt)) = e.experten_beruehrt {
        log.note(format!(
            "Expertengemisch: {beruehrt} von {gesamt} Experten berührt"
        ));
    }
    if argmax_nachher == v.ziel {
        log.note("Das Ziel ist am Ende die Vorhersage.");
    } else {
        // ⚑ **Kein Fehlschlag.** Ob das Ziel erreicht wird, hängt am
        // Modell und an den Vorgaben; der Abdruck ist auch ohne das ein
        // gültiger Vergleichswert. Gesagt gehört es trotzdem, sonst
        // liest jemand später einen Abdruck als Lernbeleg.
        log.note(format!(
            "Das Ziel wurde nicht erreicht (Vorhersage {argmax_nachher}). Der Abdruck bleibt \
             vergleichbar; ein Lernbeleg ist er ohnehin nicht."
        ));
    }

    // ⚑ **Die einzige Bedingung, die auf EINER Maschine prüfbar ist.**
    // Bewegt sich kein Gewicht, war der Gradient überall null, und der
    // Abdruck ist der des unveränderten Modells: ein Wert, auf den sich
    // zwei Maschinen mühelos einigen und der nichts belegt. Genau dieser
    // Fall trat am 2026-09-04 ein, als der Nenner der Schrittweite zu
    // gross war (Fund 177 nebenan).
    // ⚑ **Die zweite Ausfallart, und sie ist die stillere** (2026-09-05).
    // Ein Lauf, dessen Gewichte die Übertragungsform verlassen, hat
    // gerechnet und bewegt: Er sieht von aussen aus wie ein Erfolg. Nur
    // lässt sich das Ergebnis nicht ins Artefakt zurückschreiben, und
    // das fiele erst beim Zurückschreiben auf.
    if let Some(schritt) = e.aus_der_form {
        log.error(format!(
            "Die Gewichte haben bei Schritt {schritt} die Übertragungsform verlassen: sie              lassen sich nicht mehr als int8 mit Zeilenversatz ausdrücken. Der Lauf hat dort              aufgehört. Abhilfe: einen größeren Nenner der Schrittweite wählen              (aktuell {})."
            , v.lr_nenner
        ));
        log.result(WERT, &e.abdruck, format!("aus der Form bei Schritt {schritt}"));
        return false;
    }

    if e.bewegte_gewichte == 0 {
        log.error(
            "Kein einziges Gewicht hat sich bewegt: der Schritt war wirkungslos, und der \
             Abdruck belegt nichts."
                .to_string(),
        );
        log.result(WERT, &e.abdruck, "0 bewegt");
        return false;
    }

    log.result(
        WERT,
        &e.abdruck,
        format!("{}/{} bewegt", e.bewegte_gewichte, e.gewichte_gesamt),
    );
    true
}
