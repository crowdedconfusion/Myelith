//! Der Prüfstandslauf: **ein Weg, keine Wahl.**
//!
//! # ⚑ Warum es diesen Ablauf gibt (2026-09-11)
//!
//! Bis zu diesem Tag stand nach der Namenseingabe ein Menü mit sechs
//! Punkten, und zwei davon waren Entscheidungen, die ein Teilnehmer
//! nicht treffen kann: welches Artefakt, welcher Testplan. **Beide
//! Antworten müssen auf allen Maschinen gleich sein**, sonst sind die
//! Protokolle nicht vergleichbar, und genau das ist der Zweck des
//! ganzen Laufs.
//!
//! Eine Einstellung, die alle gleich setzen müssen, ist keine
//! Einstellung. Sie steht jetzt im Quelltext
//! ([`crate::spec::pruefstand`]), und der Teilnehmer tippt seinen Namen
//! und sieht zu.
//!
//! **Der Klient trägt das nicht mehr mit.** Zum Ausprobieren eines
//! Modells gibt es seit v0.17.0 einen eigenen Client mit
//! Konsolenfassung; der Testclient ist ein Messgerät, und ein Messgerät
//! hat einen Knopf.
//!
//! # Was hier NICHT hineingehört
//!
//! ⚠️ **Die Einzelstufen bleiben auf der Befehlszeile.**
//! `myl-test hardware`, `determinismus`, `shard`, `stack`,
//! `konformitaet`, `training` und `testlauf` sind unverändert
//! erreichbar. Wer eine Einzelmessung will, weiß das dort; im Menü sah
//! es aus wie eine Auswahl zwischen gleichwertigen Wegen.
//!
//! ⚠️ **Der Netzlauf gehört nicht dazu.** Er beantwortet eine andere
//! Frage (finden mehrere Rechner einander) und braucht eine Adresse vom
//! Koordinator. Ein Lauf, der ohne Eingabe durchläuft, kann ihn nicht
//! enthalten.

use std::path::PathBuf;

use crate::menu::Einstellungen;
use crate::spec::{pruefstand, PRUEFSTAND_MODELL, PRUEFSTAND_WIEDERHOLUNGEN};
use crate::{LogZiel, RunLog};

/// Die Einstellungen eines Prüfstandslaufs, vollständig aus dem festen
/// Prüfstand abgeleitet.
///
/// **Der Name ist das Einzige, was von aussen kommt.** Er benennt die
/// Ergebnisdatei; alles andere steht fest, damit zwei Maschinen
/// dasselbe rechnen.
pub fn einstellungen(name: &str) -> Einstellungen {
    let plan = pruefstand();
    Einstellungen {
        einstellungen_id: plan.short_id(),
        testdatei: Some(plan.plan_id.clone()),
        prompts: plan.prompts,
        steps: plan.steps,
        shards: plan.shards,
        artifacts: None,
        logs: crate::default_ergebnis_dir(),
        teilnehmer: name.to_string(),
        wiederholungen: PRUEFSTAND_WIEDERHOLUNGEN,
    }
}

/// Das Ergebnis eines Prüfstandslaufs.
pub struct Ausgang {
    /// Ob alle Stufen bestanden haben.
    pub bestanden: bool,
    /// Wo die Ergebnisdatei liegt, falls eine geschrieben wurde.
    pub ergebnis: Option<PathBuf>,
}

/// Fährt den Prüfstand von Anfang bis Ende.
///
/// # ⚑ Die einzige Rückfrage, und warum sie bleibt
///
/// Fehlt das Artefakt, muss es geholt werden, und das sind rund 2,5 GB
/// über das Netz. **Ein Download dieser Grösse läuft nicht ungefragt**,
/// auch nicht in einem Ablauf, der sonst ohne Eingabe durchläuft: Wer
/// über eine getaktete Verbindung misst oder wenig Platz hat, muss nein
/// sagen können, bevor es losgeht und nicht mittendrin.
///
/// Liegt das Artefakt bereits da, wird nichts gefragt.
pub fn fahren(name: &str, meldung: &mut dyn FnMut(String)) -> Ausgang {
    let mut e = einstellungen(name);

    meldung(format!(
        "  Prüfstand: {} Prompts, {} Token, {} Shards, {} Durchgänge je Prompt.",
        e.prompts.len(),
        e.steps,
        e.shards,
        e.wiederholungen
    ));
    meldung(format!("  Modell: {PRUEFSTAND_MODELL}"));
    meldung(format!("  Kennung: {}\n", e.einstellungen_id));

    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());

    // Erst nachsehen, ob es da ist: Das kostet nichts und erspart die
    // Rückfrage im Regelfall.
    let artefakt = match crate::artefakte::vorhandenes(&repo, PRUEFSTAND_MODELL) {
        Some(p) => {
            meldung(format!("  Artefakt liegt bereit: {}\n", p.display()));
            p
        }
        None => {
            let mut frage =
                |prompt: &str| -> Option<String> { crate::auswahl::frage(&format!("  {prompt}")) };
            let mut f: crate::artefakte::Rueckfrage = Some(&mut frage);
            match crate::artefakte::beschaffen_fuer(&repo, PRUEFSTAND_MODELL, &mut f, &mut |t| {
                meldung(format!("  {t}"))
            }) {
                Ok(p) => p,
                Err(fehler) => {
                    for zeile in fehler.lines() {
                        meldung(format!("  {zeile}"));
                    }
                    meldung(String::from(
                        "\n  Ohne Artefakt kann der Prüfstand nicht laufen.",
                    ));
                    return Ausgang { bestanden: false, ergebnis: None };
                }
            }
        }
    };
    e.artifacts = Some(artefakt.clone());

    let hw = crate::hardware::Fingerprint::collect().short_id();
    let ziel = LogZiel::neu(&e.logs, "pruefstand", &e.teilnehmer, &e.einstellungen_id, &hw);
    let log = RunLog::mit_ziel(ziel, true);
    // ⚑ **Nach dem Anlegen gefragt, nicht davor.** `RunLog` weicht auf
    // einen Zähler aus, wenn der Name schon belegt ist (zwei Läufe in
    // derselben Sekunde). Ein vorher gerechneter Pfad zeigte dann auf
    // die Datei des vorigen Laufs.
    let ergebnis = log.dir().join(format!("{}.jsonl", log.dateiname()));

    let bestanden = crate::menu::stufen_fahren(&e, artefakt, log);
    Ausgang { bestanden, ergebnis: Some(ergebnis) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Der Prüfstand steht fest.** Zwei Aufrufe auf zwei Maschinen
    /// müssen dieselbe Kennung ergeben, sonst legt der Vergleich sie in
    /// getrennte Gruppen und urteilt UNVOLLSTÄNDIG.
    #[test]
    fn zwei_laeufe_tragen_dieselbe_kennung() {
        let a = einstellungen("anna");
        let b = einstellungen("bernd");
        assert_eq!(a.einstellungen_id, b.einstellungen_id);
        assert_eq!(a.prompts, b.prompts);
        assert_eq!((a.steps, a.shards, a.wiederholungen), (b.steps, b.shards, b.wiederholungen));
    }

    /// **Nur der Name unterscheidet zwei Läufe.**
    #[test]
    fn der_name_ist_das_einzige_von_aussen() {
        let e = einstellungen("anna");
        assert_eq!(e.teilnehmer, "anna");
        assert_eq!(e.testdatei.as_deref(), Some("pruefstand"));
    }

    /// **Das Ergebnis landet im Ergebnisordner, nicht in `logs/`.**
    ///
    /// 📌 Die Trennung ist der Grund für den Ordner: `logs/` sammelt
    /// jeden Lauf, auch den abgebrochenen. Was weitergegeben wird, soll
    /// nicht erst herausgesucht werden müssen.
    #[test]
    fn das_ergebnis_geht_in_den_ergebnisordner() {
        let e = einstellungen("anna");
        assert!(
            e.logs.ends_with("Ergebnisse"),
            "der Lauf schreibt nach {:?} statt nach Ergebnisse/",
            e.logs
        );
    }

    /// **Die sechs Prompts decken mehr als eine Sprache und mehr als
    /// eine Textsorte ab.** Ein einzelner Prompt übte einen einzigen
    /// Pfad durch die Nachschlagetabellen aus.
    #[test]
    fn der_pruefstand_uebt_verschiedene_wege() {
        let p = pruefstand();
        assert!(p.prompts.len() >= 4, "zu wenige Prompts: {}", p.prompts.len());
        assert!(
            p.prompts.iter().any(|s| s.contains("Hauptstadt")),
            "kein deutscher Prompt"
        );
        assert!(
            p.prompts.iter().any(|s| s.contains("capital")),
            "kein englischer Prompt"
        );
        assert!(
            p.prompts.iter().any(|s| s.contains("17 times 23")),
            "keine Rechenaufgabe"
        );
        // Die Prompts müssen verschieden sein: zwei gleiche übten
        // denselben Weg zweimal und verlängerten nur den Lauf.
        let mut sortiert = p.prompts.clone();
        sortiert.sort();
        let vorher = sortiert.len();
        sortiert.dedup();
        assert_eq!(vorher, sortiert.len(), "doppelter Prompt im Prüfstand");
    }

    /// **Zwei Durchgänge sind das Minimum.** Bitgleichheit braucht zwei
    /// Ergebnisse; bei einem gäbe es nichts zu vergleichen.
    ///
    /// Als `const`-Block, damit die Zusicherung beim **Übersetzen**
    /// greift und nicht erst beim Testlauf: Ein Prüfstand mit einem
    /// Durchgang soll gar nicht erst bauen.
    #[test]
    fn bitgleichheit_braucht_zwei_durchgaenge() {
        const _: () = assert!(PRUEFSTAND_WIEDERHOLUNGEN >= 2);
        // Und der Wert, den der Lauf tatsächlich benutzt, kommt von dort.
        assert_eq!(einstellungen("anna").wiederholungen, PRUEFSTAND_WIEDERHOLUNGEN);
    }

    /// **Der eingebaute Prüfstand und der mitgelieferte `standard.plan`
    /// sind dasselbe.**
    ///
    /// ⚑ Sie stehen an zwei Orten, und das ist Absicht: Der Prüfstand
    /// im Quelltext ist, was jeder Teilnehmer fährt; die Datei daneben
    /// ist, was ein Koordinator liest, abwandelt und verteilt, wenn er
    /// einmal etwas anderes messen will.
    ///
    /// 📌 **Zwei Orte für dieselbe Angabe laufen auseinander**, und
    /// genau das ist am 2026-09-11 an vier Stellen dieses Projekts
    /// passiert (Lernrate, Modellname, Modelltabelle, Modellgrösse).
    /// Diese Probe ist der Draht dazwischen: Wer einen der beiden
    /// ändert, muss den anderen mitändern oder hier vorbeikommen.
    ///
    /// Ohne die Datei im Baum wird nichts geprüft und nichts behauptet:
    /// Ein Test, der an einem Pfad hängt, fällt sonst auf einem
    /// Teilbaum aus, statt etwas zu sagen.
    #[test]
    fn der_pruefstand_deckt_sich_mit_dem_mitgelieferten_plan() {
        let datei = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("Testpläne")
            .join("standard.plan");
        let Ok(text) = std::fs::read_to_string(&datei) else {
            return;
        };
        let aus_datei = crate::TestPlan::parse(&text).expect("standard.plan ist unlesbar");
        let eingebaut = pruefstand();
        assert_eq!(
            aus_datei.short_id(),
            eingebaut.short_id(),
            "standard.plan ({}) und der eingebaute Prüfstand ({}) messen Verschiedenes",
            aus_datei.short_id(),
            eingebaut.short_id()
        );
        assert_eq!(aus_datei.prompts, eingebaut.prompts);
        assert_eq!((aus_datei.steps, aus_datei.shards), (eingebaut.steps, eingebaut.shards));
    }
}
