//! **Vorschlaege mit exakter Pruefung**: mehrere Token je Vorwaertspass,
//! ohne dass sich ein einziges Token der Ausgabe aendert.
//!
//! # Die Idee
//!
//! Im Decode liest jeder Schritt alle Gewichte, um **ein** Token zu
//! rechnen. Liegt eine Vermutung fuer die naechsten Token vor, laufen das
//! gewaehlte Token und die vermuteten in **einem** gebuendelten Durchgang
//! ([`crate::model::IntegerModel::logits_stapel`]); die Gewichte werden
//! einmal gelesen und fuer mehrere Positionen benutzt.
//!
//! # ⚑ Warum sich dabei nichts aendert
//!
//! Angenommen wird ein vermutetes Token **nur, wenn es genau das ist, was
//! die Auswahl an dieser Stelle ohnehin gewaehlt haette**: dieselben
//! Logits (der gebuendelte Durchgang rechnet Position fuer Position
//! dasselbe wie der tokenweise, geprueft in
//! `die_gebuendelten_logits_sind_die_tokenweisen`), dieselbe Auswahl,
//! dieselbe Saatkette, denn je ausgegebenem Token wird genau einmal
//! gewaehlt, in derselben Reihenfolge. Beim ersten Unterschied gilt das
//! gewaehlte Token, und der Speicher wird auf die angenommenen
//! zurueckgesetzt.
//!
//! **Damit ist die Ausgabe Token fuer Token die des gewoehnlichen Laufs**,
//! gleich ob die Vermutungen gut, schlecht oder gar keine sind. Eine gute
//! Vermutung spart Zeit, eine schlechte kostet welche, keine aendert etwas.
//! Ein Pruefer im Netz braucht die Vermutungen nicht: Er rechnet die Folge
//! auf dem gewoehnlichen Weg nach und kommt auf dieselben Zahlen.
//!
//! # Woher die Vermutungen kommen
//!
//! Hier zuerst aus dem Text selbst ([`aus_dem_text`]): Wiederholt die
//! Antwort etwas, das schon dasteht (ein Codeausschnitt, der geaendert
//! wird, ein Pfad, ein Zitat), steht die Fortsetzung schon im Verlauf. Das
//! braucht keine zusaetzlichen Gewichte und baut den Pruefweg, auf dem
//! spaeter auch eine Vorhersageschicht des Modells laufen kann.
//!
//! ⚑ **Die Idee stammt aus einer fremden Laufzeit fuer Expertengemische
//! und ist hier neu geschrieben**, gegen den Ganzzahlpfad; dort runden die
//! Kerne je nach Buendelgroesse verschieden, und die Ausgabe haengt am
//! Vorschlagen. Hier nicht, und das ist der Punkt dieses Moduls.

/// Woher die Vermutungen kommen.
///
/// ⚑ **Eine Einstellung der Geschwindigkeit und nicht der Rechnung**: Die
/// Ausgabe ist bei jeder Quelle dieselbe. Deshalb steht sie am
/// Gespraechsspeicher ([`crate::generate::Fortsetzung`]) und nicht in der
/// Beschreibung eines Laufs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vorschlagsquelle {
    /// Keine Vermutungen: Token fuer Token, wie bisher.
    #[default]
    Aus,
    /// Aus dem Verlauf selbst, siehe [`aus_dem_text`].
    AusDemText {
        /// Wie viele Token hoechstens vermutet werden.
        hoechstens: usize,
        /// Wie viele der letzten Token mindestens schon einmal so dastehen
        /// muessen, bevor vermutet wird.
        mindestlauf: usize,
    },
}

impl Vorschlagsquelle {
    /// Die Vorgabe fuer Vermutungen aus dem Text: bis zu fuenf Token, wenn
    /// die letzten zwei schon einmal so dastanden.
    pub const TEXT: Vorschlagsquelle = Vorschlagsquelle::AusDemText { hoechstens: 5, mindestlauf: 2 };

    /// Die Vermutungen fuer den Verlauf `verlauf`, dessen letztes Token
    /// gerade gewaehlt ist.
    pub fn vorschlagen(&self, verlauf: &[usize]) -> Vec<usize> {
        match *self {
            Vorschlagsquelle::Aus => Vec::new(),
            Vorschlagsquelle::AusDemText { hoechstens, mindestlauf } => aus_dem_text(verlauf, hoechstens, mindestlauf),
        }
    }
}

/// Wie lang ein Lauf hoechstens verglichen wird.
const LAENGSTER_LAUF: usize = 4;

/// **Die Fortsetzung, die schon im Text steht.**
///
/// Gesucht wird das letzte fruehere Vorkommen der letzten `n` Token, fuer
/// `n` von [`LAENGSTER_LAUF`] abwaerts bis `mindestlauf`; der laengste
/// Treffer gewinnt, und unter gleich langen der juengste. Vorgeschlagen
/// wird, was dort folgte, hoechstens `hoechstens` Token.
///
/// ⚑ **Der juengste Treffer**, weil eine Wiederholung meist das wiederholt,
/// was zuletzt dastand (die Zeile, die gerade geaendert wird, nicht ihre
/// Vorlage weiter oben).
pub fn aus_dem_text(verlauf: &[usize], hoechstens: usize, mindestlauf: usize) -> Vec<usize> {
    let ende = verlauf.len();
    if hoechstens == 0 || mindestlauf == 0 || ende < mindestlauf + 1 {
        return Vec::new();
    }
    for n in (mindestlauf..=LAENGSTER_LAUF.min(ende - 1)).rev() {
        let suche = &verlauf[ende - n..];
        // Fruehere Vorkommen enden spaetestens eine Stelle vor dem Ende,
        // damit hinter ihnen etwas steht.
        let treffer = (0..ende - n).rev().find(|&i| &verlauf[i..i + n] == suche);
        if let Some(i) = treffer {
            let ab = i + n;
            let bis = (ab + hoechstens).min(ende);
            return verlauf[ab..bis].to_vec();
        }
    }
    Vec::new()
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ **Der laengste und juengste Treffer**, und ohne Treffer nichts.
    #[test]
    fn die_fortsetzung_kommt_aus_dem_verlauf() {
        // ... 1 2 3 9 9 ... 1 2 3 7 8 ... 1 2 3  -> 7 8 (der juengere)
        let v = [5, 1, 2, 3, 9, 9, 6, 1, 2, 3, 7, 8, 4, 1, 2, 3];
        assert_eq!(aus_dem_text(&v, 5, 2), vec![7, 8, 4, 1, 2]);
        assert_eq!(aus_dem_text(&v, 2, 2), vec![7, 8]);
        // Ein laengerer Lauf schlaegt einen juengeren kuerzeren.
        let w = [1, 2, 3, 4, 50, 9, 3, 4, 60, 1, 2, 3, 4];
        assert_eq!(aus_dem_text(&w, 1, 2), vec![50]);
        // Kein Vorkommen, zu kurz, oder aus: nichts.
        assert!(aus_dem_text(&[1, 2, 3, 4], 5, 2).is_empty());
        assert!(aus_dem_text(&[1], 5, 1).is_empty());
        assert!(aus_dem_text(&v, 0, 2).is_empty());
        assert!(Vorschlagsquelle::Aus.vorschlagen(&v).is_empty());
        // Am Ende des Verlaufs wird nur vorgeschlagen, was dasteht.
        assert_eq!(aus_dem_text(&[7, 7, 7], 5, 1), vec![7]);
    }
}
