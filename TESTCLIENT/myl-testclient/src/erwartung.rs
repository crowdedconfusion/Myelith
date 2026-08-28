//! `--erwarte <digest>`: der Lauf schlägt fehl, wenn er etwas anderes
//! rechnet als angegeben (Punkt 3.2).
//!
//! **Wofür das da ist.** Bei einem θ_v-Wechsel ändern sich die
//! Vergleichswerte **zwangsläufig**. Die Frage ist dann nicht „gleich
//! oder nicht", sondern „erwartet oder nicht". Wer den neuen Wert einmal
//! festgestellt hat, schreibt ihn in den CI-Aufruf; ab da meldet sich
//! jede weitere Änderung von selbst, statt beim nächsten Partnerlauf
//! aufzufallen.
//!
//! **Was das ausdrücklich nicht ist:** ein Determinismusnachweis. Der
//! braucht zwei Maschinen und entsteht in `vergleich`. Hier steht der
//! erwartete Wert in der Befehlszeile, also auf derselben Seite wie der
//! gemessene; das prüft eine Erwartung, keine Übereinstimmung zweier
//! unabhängiger Läufe.
//!
//! ## Warum ein Präfix genügt, und wo die Grenze liegt
//!
//! Auf dem Bildschirm stehen die Vergleichswerte in ihrer Kurzform mit
//! **16 Hexzeichen**, und genau die tippt jemand ab. Ein Präfixvergleich
//! ist deshalb der Normalfall und wird angenommen, ab 16 Zeichen.
//!
//! Das sind 64 Bit. Gegen einen Zufall oder eine versehentliche Änderung
//! reicht das mit großem Abstand; gegen jemanden, der einen passenden
//! Digest **sucht**, reicht es nicht. Für diesen Zweck ist das in
//! Ordnung, denn die Erwartung steht in derselben Befehlszeile wie der
//! Lauf: Wer sie fälschen wollte, könnte sie ebenso gut weglassen. Das
//! Protokoll hält trotzdem fest, wie viele Zeichen verglichen wurden,
//! damit niemand einen Präfixvergleich für einen vollen hält.

/// Ergebnis einer Erwartungsprüfung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Befund {
    /// Der Lauf hat den erwarteten Wert geliefert.
    Stimmt {
        name: String,
        verglichene_zeichen: usize,
    },
    /// Der Lauf hat gerechnet, aber etwas anderes.
    Abweichung {
        name: String,
        erwartet: String,
        erhalten: String,
    },
    /// Der Lauf hat keinen Vergleichswert erzeugt.
    OhneWert,
    /// Die Erwartung selbst taugt nicht als Maßstab.
    ErwartungUnbrauchbar(&'static str),
}

impl Befund {
    pub fn ist_erfuellt(&self) -> bool {
        matches!(self, Befund::Stimmt { .. })
    }
}

/// Die kürzeste Erwartung, die noch etwas aussagt.
///
/// Entspricht der Kurzform, die der Client anzeigt. Kürzer wäre kein
/// Vergleich mehr, sondern ein Glücksspiel: Vier Zeichen treffen im
/// Mittel jeden 65 536. Lauf.
pub const MINDESTZEICHEN: usize = 16;

/// Prüft den Vergleichswert eines Laufs gegen die Erwartung.
///
/// `leitwert` ist `(Name, Digest)` des zuletzt protokollierten
/// Vergleichswerts, also der Gesamtwert des Laufs.
pub fn pruefen(leitwert: Option<(&str, &str)>, erwartet: &str) -> Befund {
    let erwartet = erwartet.trim().to_ascii_lowercase();

    if erwartet.len() < MINDESTZEICHEN {
        return Befund::ErwartungUnbrauchbar(
            "zu kurz: mindestens 16 Hexzeichen, so viele zeigt der Client auch an",
        );
    }
    if !erwartet.chars().all(|c| c.is_ascii_hexdigit()) {
        return Befund::ErwartungUnbrauchbar(
            "keine Hexzahl: erwartet wird ein Vergleichswert, kein Text",
        );
    }

    let Some((name, digest)) = leitwert else {
        // **Kein stiller Durchlauf.** Ein Lauf ohne Vergleichswert hat
        // nichts gemessen; das als „Erwartung erfüllt" zu buchen wäre
        // dieselbe Klasse wie Fund 35, wo ein abgebrochener Lauf einen
        // Nachweis trug.
        return Befund::OhneWert;
    };

    if erwartet.len() > digest.len() {
        return Befund::ErwartungUnbrauchbar(
            "länger als der Vergleichswert des Laufs, kann also nicht sein Präfix sein",
        );
    }

    if digest.to_ascii_lowercase().starts_with(&erwartet) {
        Befund::Stimmt {
            name: name.to_string(),
            verglichene_zeichen: erwartet.len(),
        }
    } else {
        Befund::Abweichung {
            name: name.to_string(),
            erwartet: erwartet.clone(),
            erhalten: digest[..erwartet.len().min(digest.len())].to_string(),
        }
    }
}

/// Schreibt den Befund ins Protokoll und liefert, ob der Lauf besteht.
pub fn protokollieren(log: &mut crate::logging::RunLog, erwartet: &str) -> bool {
    let befund = pruefen(log.leitwert(), erwartet);
    match &befund {
        Befund::Stimmt {
            name,
            verglichene_zeichen,
        } => {
            let wie = if *verglichene_zeichen >= 64 {
                "vollständig".to_string()
            } else {
                format!("über die ersten {} Zeichen", verglichene_zeichen)
            };
            log.note(format!(
                "Erwartung erfüllt: {} stimmt {} mit dem angegebenen Wert überein",
                name, wie
            ));
        }
        Befund::Abweichung {
            name,
            erwartet,
            erhalten,
        } => {
            log.event(crate::logging::Event::Mismatch {
                name: format!("erwartung_{}", name),
                expected: erwartet.clone(),
                actual: erhalten.clone(),
            });
            log.note(
                "Bei einem θ_v- oder Artefaktwechsel ist eine Abweichung hier der \
                 Normalfall: Der erwartete Wert gehört dann neu festgestellt. Ohne \
                 einen solchen Wechsel ist sie ein Befund über diese Maschine oder \
                 diesen Bau.",
            );
        }
        Befund::OhneWert => {
            log.error(
                "Der Lauf hat keinen Vergleichswert erzeugt, es gibt also nichts zu \
                 erwarten. Ein bestandenes --erwarte wäre hier eine Aussage über \
                 einen Lauf, der nicht stattgefunden hat",
            );
        }
        Befund::ErwartungUnbrauchbar(grund) => {
            log.error(format!("--erwarte taugt nicht als Maßstab, {}", grund));
        }
    }
    befund.ist_erfuellt()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VOLL: &str = "272f1ee8f45f2c78ddeb9868b3581c9fc03bf4cb5bc256d9f4bff935b6196fba";

    #[test]
    fn voller_digest_stimmt() {
        let b = pruefen(Some(("determinismus", VOLL)), VOLL);
        assert_eq!(
            b,
            Befund::Stimmt {
                name: "determinismus".into(),
                verglichene_zeichen: 64
            }
        );
    }

    /// Der Normalfall: abgetippt wird die Kurzform vom Bildschirm.
    #[test]
    fn kurzform_vom_bildschirm_genuegt() {
        let b = pruefen(Some(("determinismus", VOLL)), "272f1ee8f45f2c78");
        assert!(b.ist_erfuellt());
    }

    #[test]
    fn grossschreibung_und_leerzeichen_stoeren_nicht() {
        let b = pruefen(Some(("determinismus", VOLL)), "  272F1EE8F45F2C78  ");
        assert!(b.ist_erfuellt());
    }

    #[test]
    fn ein_verschobenes_zeichen_faellt_auf() {
        let b = pruefen(Some(("determinismus", VOLL)), "272f1ee8f45f2c79");
        match b {
            Befund::Abweichung {
                erwartet, erhalten, ..
            } => {
                assert_eq!(erwartet, "272f1ee8f45f2c79");
                assert_eq!(erhalten, "272f1ee8f45f2c78");
            }
            other => panic!("erwartet Abweichung, bekam {:?}", other),
        }
    }

    /// **Der Fall, der nicht durchrutschen darf.** Ein Lauf, der nichts
    /// gemessen hat, erfüllt keine Erwartung.
    #[test]
    fn ohne_vergleichswert_ist_die_erwartung_nicht_erfuellt() {
        let b = pruefen(None, VOLL);
        assert_eq!(b, Befund::OhneWert);
        assert!(!b.ist_erfuellt());
    }

    #[test]
    fn zu_kurze_erwartung_wird_abgelehnt() {
        for kurz in ["272f", "272f1ee8f45f2c7", ""] {
            let b = pruefen(Some(("determinismus", VOLL)), kurz);
            assert!(
                matches!(b, Befund::ErwartungUnbrauchbar(_)),
                "{:?} für {:?}",
                b,
                kurz
            );
        }
    }

    #[test]
    fn text_statt_digest_wird_abgelehnt() {
        let b = pruefen(Some(("determinismus", VOLL)), "bitte-den-alten-wert");
        assert!(matches!(b, Befund::ErwartungUnbrauchbar(_)));
    }

    /// Eine Erwartung, die länger ist als der Wert selbst, kann kein
    /// Präfix sein. Das als schlichte Abweichung zu melden wäre
    /// irreführend, denn der Fehler liegt in der Eingabe.
    #[test]
    fn zu_lange_erwartung_wird_abgelehnt() {
        let b = pruefen(Some(("determinismus", VOLL)), &format!("{}ab", VOLL));
        assert!(matches!(b, Befund::ErwartungUnbrauchbar(_)));
    }

    /// Der Leitwert ist der **zuletzt** protokollierte, also der
    /// Gesamtwert des Laufs, nicht der eines einzelnen Prompts.
    #[test]
    fn geprueft_wird_gegen_den_gesamtwert() {
        let dir = std::env::temp_dir().join(format!("myl-testclient-erwartung-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut log = crate::logging::RunLog::new(&dir, "probe", false);
        log.result("prompt_1", "aaaaaaaaaaaaaaaa1111", "erster Prompt");
        log.result("determinismus", VOLL, "Gesamtwert");

        assert!(protokollieren(&mut log, "272f1ee8f45f2c78"));
        assert!(!protokollieren(&mut log, "aaaaaaaaaaaaaaaa"));
        log.finish(true);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
