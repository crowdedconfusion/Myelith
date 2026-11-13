//! Der Abruf aus dem Gedaechtnis: lexikalisch, in ganzen Zahlen.
//!
//! [`crate::gedaechtnis`] haelt fest, was hinein darf. Hier steht, was
//! zu einer Anfrage wieder herauskommt, und **mit welcher Truebung**.
//!
//! ## ⚑ Warum lexikalisch und nicht ueber Einbettungen
//!
//! Ein Abruf, der in einen Plan einfliesst, muss deterministisch sein,
//! sonst melden zwei ehrliche Pods einander als fehlerhaft. Ein Vergleich
//! von Woertern ist es durch Bauart: kein Modell, keine Gleitkommazahl,
//! keine Reihenfolge aus einem Index. Einbettungen vom eigenen
//! Ganzzahlmodell waeren ebenfalls bitgleich, kosten aber eine Inferenz je
//! Eintrag und je Anfrage und haengen an der Modellfassung; sie kommen
//! erst nach einer Messung, die ihren Vorteil belegt.
//!
//! ## Die Regeln, alle fest
//!
//! - **Woerter** sind Folgen von Bytes ohne Trenner. Trenner ist jedes
//!   ASCII-Zeichen, das weder Buchstabe noch Ziffer ist; Bytes ab `0x80`
//!   gehoeren zum Wort, damit Umlaute und andere Schriften nicht
//!   zerfallen. Nur ASCII wird klein geschrieben. ⚠️ Eine Faltung ueber
//!   Unicode hinge an den Tabellen der Werkzeugkette, und zwei Fassungen
//!   koennten verschieden falten; deshalb sind `Übung` und `übung` hier
//!   zwei Woerter. Woerter unter zwei Bytes zaehlen nicht.
//! - **Die Wertung** ist die Zahl der verschiedenen Woerter der Anfrage,
//!   die im Eintrag vorkommen. Ein Eintrag mit Wertung null kommt nicht
//!   zurueck.
//! - **Die Rangfolge** ist (Wertung fallend, Stelle steigend), ueber
//!   **alle** Eintraege; kein Naeherungsindex, denn dessen Aufbau haengt an
//!   Reihenfolge und Nebenlaeufigkeit.
//! - **Der Text muss zum Commitment passen.** Der Speicher haelt nur das
//!   Ausgabe-Commitment eines Schritts (SHA-256 ueber die Ausgabe), der
//!   Text liegt beim Aufrufer. Ein Text, der nicht passt, wird nicht
//!   gewertet; seine Stelle steht in [`Abruf::ohne_text`].
//!
//! ## ⚑ Die Truebung des Ergebnisses (Fund 695, zweite Haelfte)
//!
//! **Ein Abruf ist getruebt, sobald einer seiner Treffer es ist**,
//! dieselbe Regel wie [`crate::plan::Plan::truebung`] fuer Argumente. Die
//! Truebung je Eintrag steht seit `myl-agent` 0.9.0 im Eintrag und ist bei
//! der Aufnahme geprueft.
//!
//! Kritisch geprueft, ob das reicht: Kann ein getruebter Eintrag, der
//! **nicht** zurueckkommt, das Ergebnis lenken? Die Auswahl ist die Spitze
//! einer festen Rangfolge. Ein Eintrag beeinflusst sie nur, indem er
//! hineinkommt; liegt er darunter, ist die Spitze dieselbe, gleich was er
//! enthaelt. Ein Fluss ueber die Auswahl, an dem die Truebung vorbeiginge,
//! entsteht also nur ueber einen Treffer, und der traegt sie.
//! [`Abruf::ohne_text`] haengt nur daran, welche Texte vorliegen, nicht an
//! ihrem Inhalt.

use std::collections::BTreeSet;

use myl_types::hash::Hash;

use crate::gedaechtnis::{Eintrag, Gedaechtnis};
use crate::plan::Truebung;

/// Kuerzeste Wortlaenge in Bytes, die zaehlt.
pub const MIN_WORTLAENGE: usize = 2;

/// Ein Treffer: Stelle im Speicher, Wertung, Truebung des Eintrags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Treffer {
    /// Die Stelle des Eintrags im Speicher.
    pub stelle: u64,
    /// Wie viele verschiedene Woerter der Anfrage er enthaelt.
    pub wertung: u32,
    /// Die Truebung des Eintrags.
    pub truebung: Truebung,
}

/// Was ein Abruf ergibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abruf {
    /// Die Treffer in Rangfolge, hoechstens so viele wie verlangt.
    pub treffer: Vec<Treffer>,
    /// Getruebt, sobald ein Treffer es ist; ohne Treffer rein.
    pub truebung: Truebung,
    /// Stellen, zu denen kein passender Text vorlag, aufsteigend.
    pub ohne_text: Vec<u64>,
}

/// Die Woerter eines Textes nach den Regeln im Modulkopf.
pub fn woerter(text: &[u8]) -> BTreeSet<Vec<u8>> {
    let mut alle = BTreeSet::new();
    let mut wort = Vec::new();
    for &b in text {
        if b >= 0x80 || b.is_ascii_alphanumeric() {
            wort.push(b.to_ascii_lowercase());
        } else if !wort.is_empty() {
            if wort.len() >= MIN_WORTLAENGE {
                alle.insert(std::mem::take(&mut wort));
            } else {
                wort.clear();
            }
        }
    }
    if wort.len() >= MIN_WORTLAENGE {
        alle.insert(wort);
    }
    alle
}

/// Ruft zu `anfrage` hoechstens `anzahl` Eintraege ab.
///
/// `text` liefert zu Stelle und Eintrag die Ausgabe des Schritts, wenn
/// der Aufrufer sie hat. **Gewertet wird sie nur, wenn sie zum Commitment
/// im Eintrag passt.**
pub fn abrufen<'t>(
    gedaechtnis: &Gedaechtnis,
    mut text: impl FnMut(u64, &Eintrag) -> Option<&'t [u8]>,
    anfrage: &[u8],
    anzahl: usize,
) -> Abruf {
    let gesucht = woerter(anfrage);
    let mut gewertet: Vec<Treffer> = Vec::new();
    let mut ohne_text = Vec::new();
    for (i, e) in gedaechtnis.eintraege().iter().enumerate() {
        let stelle = i as u64;
        let Some(t) = text(stelle, e).filter(|t| Hash::sha256(t) == e.glied.ausgabe) else {
            ohne_text.push(stelle);
            continue;
        };
        if gesucht.is_empty() {
            continue;
        }
        let vorhanden = woerter(t);
        let wertung = gesucht.iter().filter(|w| vorhanden.contains(*w)).count() as u32;
        if wertung > 0 {
            gewertet.push(Treffer { stelle, wertung, truebung: e.truebung });
        }
    }
    // (Wertung fallend, Stelle steigend); die Stelle ist eindeutig, also
    // ist die Folge vollstaendig bestimmt.
    gewertet.sort_by(|a, b| b.wertung.cmp(&a.wertung).then(a.stelle.cmp(&b.stelle)));
    gewertet.truncate(anzahl);
    let truebung = gewertet.iter().map(|t| t.truebung).max().unwrap_or(Truebung::Rein);
    Abruf { treffer: gewertet, truebung, ohne_text }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kette::{anker, kettenwert, Kettenglied};
    use crate::manifest::{Herkunft, Werkzeugart, Werkzeugmanifest};
    use crate::plan::{Plan, Quelle, Schritt};
    use crate::registratur::Registratur;
    use myl_types::ids::{MerkleRoot, SegmentId, SitzungId};

    const TEXTE: [&str; 4] = [
        "Der Bericht ueber den Durchsatz im Netz",
        "Durchsatz Messung aus dem Netz, extern",
        "Zusammenfassung der externen Messung",
        "Durchsatz, Bericht; Netz: Durchsatz",
    ];

    fn registratur() -> (Registratur, MerkleRoot, MerkleRoot) {
        let mut r = Registratur::neu();
        let mach = |name: &str, art, herkunft| Werkzeugmanifest {
            name: name.into(),
            anbieter: "pruefstand".into(),
            revision: "1".into(),
            lizenz: "Apache-2.0".into(),
            art,
            herkunft,
        };
        let rein = r.nimm_werkzeug(mach("rechnen", Werkzeugart::Deterministisch, Herkunft::Verankert)).expect("gueltig");
        let welt = r.nimm_werkzeug(mach("abrufen", Werkzeugart::Extern, Herkunft::Bibliothek)).expect("gueltig");
        (r, rein, welt)
    }

    /// Ein Speicher mit vier Eintraegen: 0 rein, 1 befragt die Welt, 2
    /// rechnet auf 1 und erbt die Truebung, 3 rechnet auf nichts und ist rein.
    fn speicher() -> Gedaechtnis {
        let (reg, rein, welt) = registratur();
        let werkzeuge = [rein, welt, rein, rein];
        let p = Plan::neu(
            werkzeuge
                .iter()
                .enumerate()
                .map(|(i, w)| Schritt {
                    werkzeug: *w,
                    argumente: if i == 2 { vec![Quelle::Schritt(1)] } else { vec![] },
                })
                .collect(),
        )
        .expect("gueltiger Plan");
        let g: Vec<Kettenglied> = TEXTE
            .iter()
            .enumerate()
            .map(|(i, t)| Kettenglied { segment: SegmentId::new([i as u8 + 1; 32]), ausgabe: Hash::sha256(t.as_bytes()) })
            .collect();
        let sitzung = SitzungId::new([5u8; 32]);
        let wert = kettenwert(&anker(&sitzung, &p.adresse()), &p, &g).expect("Laenge passt");
        let truebung = p.truebung(&reg);
        let mut s = Gedaechtnis::neu();
        for i in 0..4u16 {
            let e = Eintrag { sitzung, plan: p.adresse(), stelle: i, glied: g[i as usize].clone(), truebung: truebung[i as usize] };
            s.nimm(e, &p, &g, &wert, &reg).expect("belegt");
        }
        s
    }

    fn echt(stelle: u64, _: &Eintrag) -> Option<&'static [u8]> {
        TEXTE.get(stelle as usize).map(|t| t.as_bytes())
    }

    fn stellen(a: &Abruf) -> Vec<u64> {
        a.treffer.iter().map(|t| t.stelle).collect()
    }

    /// Rangfolge nach Wertung, bei Gleichstand nach Stelle; Wertung null
    /// kommt nicht zurueck.
    #[test]
    fn die_rangfolge_ist_wertung_dann_stelle() {
        let s = speicher();
        let a = abrufen(&s, echt, b"Bericht Durchsatz Netz", 10);
        assert_eq!(stellen(&a), vec![0, 3, 1]);
        assert_eq!(a.treffer.iter().map(|t| t.wertung).collect::<Vec<_>>(), vec![3, 3, 2]);
        assert!(a.ohne_text.is_empty());
    }

    /// ⚑ **Fund 695, zweite Haelfte, als Test:** Der Abruf ist getruebt,
    /// sobald ein Treffer es ist. Gegenprobe: Bleibt der getruebte Eintrag
    /// unter der Spitze, ist das Ergebnis rein.
    #[test]
    fn der_abruf_erbt_die_truebung_seiner_treffer() {
        let s = speicher();
        let zwei = abrufen(&s, echt, b"Bericht Durchsatz Netz", 2);
        assert_eq!(stellen(&zwei), vec![0, 3]);
        assert_eq!(zwei.truebung, Truebung::Rein, "nur reine Treffer");
        let drei = abrufen(&s, echt, b"Bericht Durchsatz Netz", 3);
        assert_eq!(drei.treffer[2].truebung, Truebung::Getruebt);
        assert_eq!(drei.truebung, Truebung::Getruebt, "ein getruebter Treffer truebt den Abruf");
        // Der geerbte Fall: Eintrag 2 befragt die Welt nicht selbst.
        let geerbt = abrufen(&s, echt, b"Zusammenfassung", 5);
        assert_eq!(stellen(&geerbt), vec![2]);
        assert_eq!(geerbt.truebung, Truebung::Getruebt);
        // Ohne Treffer gibt es nichts, was getruebt sein koennte.
        assert_eq!(abrufen(&s, echt, b"Kuchenrezept", 5).truebung, Truebung::Rein);
    }

    /// Ein Text, der nicht zum Commitment passt, wird nicht gewertet und
    /// steht in `ohne_text`; ebenso einer, der fehlt.
    #[test]
    fn nur_ein_passender_text_wird_gewertet() {
        let s = speicher();
        let falsch = |stelle: u64, e: &Eintrag| -> Option<&'static [u8]> {
            if stelle == 3 { Some(&b"Bericht Durchsatz Netz untergeschoben"[..]) } else { echt(stelle, e) }
        };
        let a = abrufen(&s, falsch, b"Bericht Durchsatz Netz", 10);
        assert_eq!(stellen(&a), vec![0, 1]);
        assert_eq!(a.ohne_text, vec![3]);
        let ohne = abrufen(&s, |_, _| None, b"Bericht", 10);
        assert!(ohne.treffer.is_empty());
        assert_eq!(ohne.ohne_text, vec![0, 1, 2, 3]);
    }

    /// Nur ASCII wird klein geschrieben, Bytes ab 0x80 bleiben im Wort,
    /// Woerter unter zwei Bytes zaehlen nicht, und die Reihenfolge der
    /// Anfrage aendert nichts.
    #[test]
    fn die_zerlegung_ist_fest() {
        let w = woerter("BERICHT, über Übung a 42 x9".as_bytes());
        let erwartet: BTreeSet<Vec<u8>> =
            ["bericht", "über", "Übung", "42", "x9"].iter().map(|s| s.as_bytes().to_vec()).collect();
        assert_eq!(w, erwartet);
        assert!(!w.contains("übung".as_bytes()), "keine Faltung ueber ASCII hinaus");
        let s = speicher();
        assert_eq!(abrufen(&s, echt, b"netz BERICHT durchsatz", 10), abrufen(&s, echt, b"Bericht Durchsatz Netz", 10));
        assert!(abrufen(&s, echt, b"a b c", 10).treffer.is_empty());
        assert!(abrufen(&s, echt, b"Bericht", 0).treffer.is_empty());
    }
}
