//! **Zwischenrufe in der Schleife**, in einem eigenen Prozess.
//!
//! ⚑ **Eine eigene Datei, und das ist der Punkt.** Die Schlange der
//! Zwischenrufe ist prozessweit; liefe diese Probe neben den anderen
//! Schleifenproben im selben Prozess, koennte deren Lauf einen Zwischenruf
//! abholen, der hierher gehoert. Dieselbe Lehre wie Fund 528 (zwei Proben,
//! eine prozessweite Umgebungsvariable). Jede Datei unter `tests/` ist ein
//! eigenes Programm.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use myl_agent::manifest::{Herkunft, Werkzeugart, Werkzeugmanifest};
use myl_agent::registratur::Registratur;
use myl_types::hash::Hash;
use myl_types::ids::{Address, EpochId, MerkleRoot};
use myl_types::sitzung::{Grenzen, Sitzungskontrakt};

use myl_local_agent::ausfuehrung::{Werkzeugausfuehrung, Werkzeugfehler, Werkzeugkasten};
use myl_local_agent::betrieb::Betriebsart;
use myl_local_agent::schleife::{Ende, Lauf};
use myl_local_agent::vollmacht_grenzen::Sitzungsgrenzen;
use myl_local_agent::werkzeug::Werkzeug;
use myl_local_agent::zwischenruf;

struct Zeit;
impl Werkzeugausfuehrung for Zeit {
    fn name(&self) -> &str {
        "zeit"
    }
    fn ausfuehren(&self, _a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        Ok("12:00".to_string())
    }
}

/// Antwortet aus einer Liste und stellt bei bestimmten Aufrufen einen
/// Zwischenruf an, so als haette der Mensch waehrend dieses Schritts
/// getippt.
struct Weg {
    antworten: RefCell<VecDeque<String>>,
    zurufe: RefCell<VecDeque<(usize, String)>>,
    gerufen: Cell<usize>,
    gesehen: RefCell<Vec<Vec<myl_local_agent::Nachricht>>>,
}

impl myl_local_agent::Modellweg for Weg {
    fn chat(
        &self,
        _modell: &str,
        nachrichten: &[myl_local_agent::Nachricht],
        _max_tokens: Option<u32>,
    ) -> Result<myl_local_agent::Antwort, myl_local_agent::Tuerfehler> {
        let n = self.gerufen.get() + 1;
        self.gerufen.set(n);
        self.gesehen.borrow_mut().push(nachrichten.to_vec());
        if self.zurufe.borrow().front().is_some_and(|(bei, _)| *bei == n) {
            let (_, text) = self.zurufe.borrow_mut().pop_front().expect("Zuruf");
            zwischenruf::einreihen(&text);
        }
        Ok(myl_local_agent::Antwort {
            text: self.antworten.borrow_mut().pop_front().unwrap_or_default(),
            abschlussgrund: None,
            kennung: "oertlich".to_string(),
            segment: Default::default(),
            prompt_token: 0,
            antwort_token: 0,
        })
    }
}

fn lauf(weg: &Weg, max_schritte: u32) -> myl_local_agent::schleife::Ergebnis {
    let mut kasten = Werkzeugkasten::neu();
    kasten.einhaengen(Werkzeug::ohne_parameter("zeit", "Die Zeit."), Box::new(Zeit)).expect("eingehaengt");
    let mut registratur = Registratur::neu();
    let a_zeit = registratur
        .nimm_werkzeug(Werkzeugmanifest {
            name: "zeit".into(),
            anbieter: "Myelith".into(),
            revision: "1".into(),
            lizenz: "PolyForm-Shield-1.0.0".into(),
            art: Werkzeugart::Deterministisch,
            herkunft: Herkunft::Verankert,
        })
        .expect("ok");
    let grenzen_kontrakt = Grenzen { budget: 1000, einzellimit: 100, schwelle: u64::MAX, zeugenleiter: Vec::new() };
    let kontrakt = Sitzungskontrakt::neu(
        Address::new([1u8; 32]),
        Address::new([2u8; 32]),
        grenzen_kontrakt.clone(),
        grenzen_kontrakt,
        vec![Address::new([9u8; 32])],
        EpochId(0),
        EpochId(100),
        max_schritte,
    )
    .expect("Kontrakt");
    let grenzen = Sitzungsgrenzen::neu(kontrakt, kasten.angebote());
    let finden = move |n: &str| -> Option<MerkleRoot> { (n == "zeit").then_some(a_zeit) };
    Lauf {
        hausregel: None,
        mitschnitt: None,
        klient: weg,
        modell: "oertlich",
        grenzen: &grenzen,
        betriebsart: Betriebsart::Alles,
        einhaengung: None,
        kasten: &kasten,
        registratur: &registratur,
        adressen: &finden,
        anker: Hash::from_bytes([7u8; 32]),
        max_tokens: Some(32),
        ansageform: Default::default(),
        melder: None,
    }
    .fahren("Wie spaet ist es?")
}

fn weg(antworten: &[&str], zurufe: &[(usize, &str)]) -> Weg {
    Weg {
        antworten: RefCell::new(antworten.iter().map(|s| s.to_string()).collect()),
        zurufe: RefCell::new(zurufe.iter().map(|(n, t)| (*n, t.to_string())).collect()),
        gerufen: Cell::new(0),
        gesehen: RefCell::new(Vec::new()),
    }
}

/// ⚑ **Ein Zwischenruf kommt nach dem Schritt an, in dem er getippt wurde**,
/// als Nachricht des Menschen hinter dem Werkzeugergebnis. Und hat das
/// Modell gerade abgeschlossen, bekommt es ihn trotzdem und arbeitet
/// weiter. Ausgeschaltet holt die Schleife nichts ab.
#[test]
fn zwischenrufe_kommen_nach_dem_schritt_an() {
    let _ = zwischenruf::abholen();

    // Ausgeschaltet: Der Zuruf bleibt liegen, der Lauf endet wie immer.
    zwischenruf::annehmen(false);
    let w = weg(&["<tool_call>{\"name\":\"zeit\",\"arguments\":{}}</tool_call>", "Es ist 12:00."], &[(1, "liegen lassen")]);
    let e = lauf(&w, 5);
    assert_eq!(e.ende, Ende::Fertig);
    assert_eq!(w.gerufen.get(), 2);
    assert_eq!(zwischenruf::abholen(), vec!["liegen lassen".to_string()], "ausgeschaltet holt die Schleife nichts ab");

    // Eingeschaltet: getippt waehrend Schritt 1, an nach dem Werkzeug.
    zwischenruf::annehmen(true);
    let w = weg(
        &["<tool_call>{\"name\":\"zeit\",\"arguments\":{}}</tool_call>", "Es ist zwoelf Uhr.", "Gern, auf Deutsch: zwoelf Uhr."],
        &[(1, "Bitte auf Deutsch"), (2, "Und ohne Ziffern")],
    );
    let e = lauf(&w, 5);
    assert_eq!(e.ende, Ende::Fertig, "{}", e.ende);
    let zweiter = &w.gesehen.borrow()[1];
    let i_tool = zweiter.iter().rposition(|n| n.role == "tool").expect("Werkzeugergebnis");
    let i_ruf = zweiter.iter().rposition(|n| n.role == "user" && n.content.contains("Bitte auf Deutsch")).expect("Zwischenruf fehlt");
    assert!(i_ruf > i_tool, "der Zwischenruf steht vor dem Werkzeugergebnis");
    // Getippt waehrend Schritt 2, in dem das Modell abschloss: ein dritter Schritt.
    assert_eq!(w.gerufen.get(), 3, "nach dem Abschluss kam der Zwischenruf nicht an");
    assert!(w.gesehen.borrow()[2].iter().any(|n| n.content.contains("Und ohne Ziffern")));
    assert!(zwischenruf::abholen().is_empty());
    zwischenruf::annehmen(false);
}
