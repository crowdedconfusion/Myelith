//! Das Gespraech einer Sitzung: was ueber Auftraege hinweg mitgeht, wie
//! viel Kontext es belegt, und wie es verdichtet wird.
//!
//! # ⚑ Warum es das gibt (2026-09-14)
//!
//! Bis hierher stand im Agenten jeder Auftrag fuer sich (Entscheidung C2).
//! Eine Nachfrage wie „und jetzt dasselbe fuer die andere Datei" lief ins
//! Leere, und eine Anzeige des Kontexts haette nur den einen laufenden
//! Auftrag zeigen koennen. **Konsole und Fenster halten jetzt ein
//! Gespraech**, und beide benutzen dafuer diese eine Stelle: Wie gezaehlt
//! und wie verdichtet wird, soll nicht an zwei Orten verschieden werden.
//!
//! ⚑ **Schrittbudget und Belegkette bleiben je Auftrag.** Das Gespraech ist
//! Eingabe eines Laufs, keine Fortsetzung seines Vertrags; die Begruendung
//! steht an `Lauf::fahren_mit_verlauf`.

use myl_local_agent::verdichtung;
use myl_local_agent::{Modellweg, Nachricht, Tuerfehler};

/// Die Nachrichten, die ueber Auftraege hinweg mitgehen.
///
/// ⚑ **Ohne Werkzeugansage.** Die gehoert dem jeweiligen Lauf und steht
/// dort vorn; im Gespraech veraltete sie, sobald sich die Werkzeuge
/// aendern.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gespraech {
    nachrichten: Vec<Nachricht>,
}

impl Gespraech {
    /// Ein leeres Gespraech.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Aus vorhandenen Nachrichten, etwa aus dem Speicher des Fensters.
    pub fn aus(nachrichten: Vec<Nachricht>) -> Self {
        Self { nachrichten: ohne_system(&nachrichten) }
    }

    pub fn nachrichten(&self) -> &[Nachricht] {
        &self.nachrichten
    }

    pub fn ist_leer(&self) -> bool {
        self.nachrichten.is_empty()
    }

    /// **Uebernimmt die Nachrichten eines beendeten Laufs.**
    ///
    /// ⚑ **Ersetzt und haengt nicht an**: Der Lauf hat das bisherige
    /// Gespraech schon vor seinem Auftrag stehen, und hat er verdichtet,
    /// steht dort die Zusammenfassung statt der alten Nachrichten.
    pub fn nach_dem_lauf(&mut self, nachrichten: &[Nachricht]) {
        self.nachrichten = ohne_system(nachrichten);
    }

    /// Vergisst alles.
    pub fn leeren(&mut self) {
        self.nachrichten.clear();
    }
}

fn ohne_system(nachrichten: &[Nachricht]) -> Vec<Nachricht> {
    nachrichten.iter().filter(|n| n.role != "system").cloned().collect()
}

/// Was die Kontextanzeige zeigt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Kontextanzeige {
    /// Token von Ansage und Gespraech zusammen.
    pub belegt: usize,
    /// Positionen des Modells.
    pub grenze: usize,
    /// Davon die Werkzeugansage.
    pub ansage: usize,
    /// Wie viele Nachrichten das Gespraech hat.
    pub nachrichten: usize,
    /// Belegter Anteil in ganzen Prozent.
    pub prozent: usize,
}

/// **Wie viel Kontext Ansage und Gespraech belegen**, so gezaehlt, wie das
/// Modell sie als Prompt saehe. `None`, wenn das Modell es nicht weiss.
pub fn anzeige(modell: &dyn Modellweg, ansage: Option<&Nachricht>, gespraech: &Gespraech) -> Option<Kontextanzeige> {
    let mut alle: Vec<Nachricht> = ansage.cloned().into_iter().collect();
    alle.extend(gespraech.nachrichten.iter().cloned());
    let stand = modell.kontext(&alle)?;
    let ansage = match ansage {
        Some(a) => modell.kontext(std::slice::from_ref(a)).map(|s| s.belegt).unwrap_or(0),
        None => 0,
    };
    Some(Kontextanzeige {
        belegt: stand.belegt,
        grenze: stand.grenze,
        ansage,
        nachrichten: gespraech.nachrichten.len(),
        prozent: stand.prozent(),
    })
}

/// **Die Zusammenfassung, mit der das Gespraech beginnt**, falls es
/// verdichtet wurde.
///
/// ⚑ Das Fenster legt sie ab, damit ein verdichtetes Gespraech nach einem
/// Neustart nicht wieder mit dem ganzen alten Verlauf beginnt; die
/// uebrigen Nachrichten leitet es aus den Beitraegen her.
pub fn zusammenfassung(gespraech: &Gespraech) -> Option<String> {
    gespraech
        .nachrichten
        .first()
        .filter(|n| n.role == "user" && n.content.starts_with(verdichtung::KOPF))
        .map(|n| n.content.clone())
}

/// Die Werkzeugansage, wie ein Lauf mit dieser Ruestung sie vorn stehen hat.
pub fn ansage(ruestung: &crate::ruestung::Ruestung) -> Nachricht {
    myl_local_agent::werkzeug::angebot(ruestung.kasten.angebote(), ruestung.form)
}

/// **Verdichtet das ganze Gespraech zu einer Zusammenfassung.** Zurueck
/// kommen die belegten Token davor und danach, ohne Ansage.
///
/// ⚑ **Das Gespraech bleibt unberuehrt, wenn es schiefgeht.** Eine halbe
/// Verdichtung waere ein Verlust ohne Gegenwert.
pub fn verdichten(modell: &dyn Modellweg, gespraech: &mut Gespraech) -> Result<(usize, usize), Tuerfehler> {
    let vorher = modell.kontext(&gespraech.nachrichten);
    let Some(grenze) = vorher.map(|s| s.grenze) else {
        return Err(Tuerfehler::KontextVoll { belegt: 0, grenze: 0 });
    };
    if gespraech.ist_leer() {
        return Ok((0, 0));
    }
    let text = verdichtung::zusammenfassen(modell, "lokal", &gespraech.nachrichten, verdichtung::antwortlaenge(grenze))?;
    let neu = vec![verdichtung::als_nachricht(&text)];
    let nachher = modell.kontext(&neu).map(|s| s.belegt).unwrap_or(0);
    gespraech.nachrichten = neu;
    Ok((vorher.map(|s| s.belegt).unwrap_or(0), nachher))
}

/// **Ein Balken aus Blockzeichen**, `breite` Zeichen, mit Achteln am Rand.
pub fn balken(belegt: usize, grenze: usize, breite: usize) -> String {
    const ACHTEL: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];
    let achtel = (belegt.min(grenze).saturating_mul(breite * 8)).checked_div(grenze).unwrap_or(0);
    let voll = achtel / 8;
    let mut s: String = std::iter::repeat_n('█', voll).collect();
    if voll < breite {
        s.push(if achtel % 8 == 0 { '░' } else { ACHTEL[achtel % 8] });
        s.extend(std::iter::repeat_n('░', breite - voll - 1));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein Modellweg, der in Zeichen zaehlt und jede Verdichtung mit `KURZ`
    /// beantwortet.
    struct Zeichen(usize);

    impl Modellweg for Zeichen {
        fn chat(&self, _m: &str, _n: &[Nachricht], _t: Option<u32>) -> Result<myl_local_agent::Antwort, Tuerfehler> {
            Ok(myl_local_agent::Antwort {
                text: " KURZ ".to_string(),
                abschlussgrund: None,
                kennung: String::new(),
                segment: None,
                prompt_token: 0,
                antwort_token: 0,
            })
        }
        fn kontext(&self, n: &[Nachricht]) -> Option<myl_local_agent::Kontextstand> {
            Some(myl_local_agent::Kontextstand { belegt: n.iter().map(|m| m.content.len()).sum(), grenze: self.0 })
        }
    }

    #[test]
    fn nach_dem_lauf_steht_das_gespraech_ohne_ansage() {
        let mut g = Gespraech::neu();
        g.nach_dem_lauf(&[Nachricht::system("Ansage"), Nachricht::nutzer("a"), Nachricht::modell("b")]);
        assert_eq!(g.nachrichten(), &[Nachricht::nutzer("a"), Nachricht::modell("b")]);
        g.nach_dem_lauf(&[Nachricht::system("Ansage"), Nachricht::nutzer("z")]);
        assert_eq!(g.nachrichten(), &[Nachricht::nutzer("z")], "ersetzt und haengt nicht an");
    }

    #[test]
    fn die_anzeige_zaehlt_ansage_und_gespraech() {
        let g = Gespraech::aus(vec![Nachricht::nutzer("12345"), Nachricht::modell("123")]);
        let a = anzeige(&Zeichen(40), Some(&Nachricht::system("12")), &g).expect("bekannt");
        assert_eq!((a.belegt, a.ansage, a.nachrichten, a.grenze, a.prozent), (10, 2, 2, 40, 25));
    }

    #[test]
    fn verdichten_ersetzt_das_gespraech_durch_die_zusammenfassung() {
        let mut g = Gespraech::aus(vec![Nachricht::nutzer("x".repeat(50)), Nachricht::modell("y".repeat(50))]);
        let (vorher, nachher) = verdichten(&Zeichen(10_000), &mut g).expect("verdichtet");
        assert_eq!(g.nachrichten(), &[verdichtung::als_nachricht("KURZ")]);
        assert_eq!(vorher, 100);
        assert_eq!(nachher, verdichtung::als_nachricht("KURZ").content.len());
        let mut leer = Gespraech::neu();
        assert_eq!(verdichten(&Zeichen(10), &mut leer), Ok((0, 0)));
    }

    #[test]
    fn eine_zusammenfassung_wird_erkannt() {
        let mut g = Gespraech::aus(vec![Nachricht::nutzer("x"), Nachricht::modell("y")]);
        assert_eq!(zusammenfassung(&g), None);
        verdichten(&Zeichen(10_000), &mut g).expect("verdichtet");
        assert_eq!(zusammenfassung(&g), Some(verdichtung::als_nachricht("KURZ").content));
        let falsch = Gespraech::aus(vec![Nachricht::modell(verdichtung::als_nachricht("KURZ").content)]);
        assert_eq!(zusammenfassung(&falsch), None, "nur als Nutzernachricht am Anfang");
    }

    #[test]
    fn der_balken_ist_immer_gleich_breit() {
        for (belegt, grenze) in [(0, 100), (1, 100), (50, 100), (99, 100), (100, 100), (300, 100), (5, 0)] {
            assert_eq!(balken(belegt, grenze, 20).chars().count(), 20, "{belegt}/{grenze}");
        }
        assert_eq!(balken(50, 100, 4), "██░░");
        assert_eq!(balken(100, 100, 4), "████");
        assert_eq!(balken(0, 100, 4), "░░░░");
    }
}
