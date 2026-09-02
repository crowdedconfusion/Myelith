// ⚑ Kein innerer Doc-Kommentar und kein inneres Attribut in dieser
// Datei. Sie wird per `include!` in die Fuzz-Ziele eingesetzt, und
// `include!` darf keine inneren Attribute liefern: Rust verlangt sie
// lexikalisch am Dateianfang.
//
// Die Fuzz-Ziele des Konsens-Drahtformats, an **einer** Stelle.
//
// Benutzt von zwei Seiten: vom stabilen Lauf in `tests/fuzzkorpus.rs`
// und von den `libfuzzer`-Zielen unter `fuzz/`. Zwei Fassungen desselben
// Ziels waeren zwei Wahrheiten, und der Fuzzer faende dann etwas, das
// die Regression hinterher nicht festhaelt.
//
// # Die Aussage
//
// Kanonizitaet: Liest sich eine Bytefolge vollstaendig als `T`, dann
// ergibt `borsh::to_vec` genau diese Bytefolge wieder. Wo zwei
// verschiedene Bytefolgen denselben Wert ergeben, ist eine davon nicht
// kanonisch, und im Gossip heisst das: zwei Nachrichten-Kennungen fuer
// denselben Inhalt.
//
// ⚑ **Hier zaehlt das mehr als bei den Grundtypen.** Ein `Block` traegt
// die Zustandswurzel, und der Knoten verwirft ihn, wenn beim Lesen etwas
// uebrig bleibt. Eine formbare Kodierung waere ein zweiter Weg, denselben
// Block zu verbreiten, und Gossipsub verwuerfe ihn nicht als Dublette.
//
// # Was ein Ziel nicht tut
//
// Es prueft keine Signatur. Ein Fuzzer, der je Eingabe eine BLS-Pruefung
// rechnet, schafft dreistellig statt sechsstellig viele Laeufe je
// Sekunde, und die erzeugten Bytes kommen ohnehin nie durch.

use borsh::{BorshDeserialize, BorshSerialize};

// Liest `daten` als `T` und verlangt, dass die Kodierung kanonisch ist.
//
// Rueckgabe: `true`, wenn sich die Bytes vollstaendig als `T` lesen
// liessen. Die eigentliche Aussage steht im `assert_eq!`.
#[allow(dead_code)]
pub fn kanonisch<T>(daten: &[u8]) -> bool
where
    T: BorshDeserialize + BorshSerialize,
{
    let mut rest = daten;
    let Ok(wert) = T::deserialize(&mut rest) else {
        return false;
    };
    if !rest.is_empty() {
        return false;
    }
    let zurueck = match borsh::to_vec(&wert) {
        Ok(b) => b,
        Err(e) => panic!("gelesen, aber nicht schreibbar: {e}"),
    };
    assert_eq!(
        zurueck.as_slice(),
        daten,
        "nicht kanonisch: zwei Bytefolgen ergeben denselben Wert von {}",
        std::any::type_name::<T>()
    );
    true
}

// Ein Ziel je Typ, der ueber `/myelith/consensus/1` oder ueber Gossip
// kommt. Die Namen sind zugleich die Korpusverzeichnisse.
// Ein Ziel nimmt rohe Bytes und sagt, ob sie sich vollstaendig als
// ihr Typ lesen liessen. Als eigener Name, weil `clippy` das Paar aus
// Name und Funktionszeiger sonst als zu verschachtelt meldet, und die
// CI mit `-D warnings` uebersetzt.
#[allow(dead_code)]
pub type Ziel = fn(&[u8]) -> bool;

#[allow(dead_code)]
pub const ZIELE: &[(&str, Ziel)] = &[
    ("block", |d| kanonisch::<myl_consensus::block::Block>(d)),
    ("transaktion", |d| {
        kanonisch::<myl_consensus::block::Transaktion>(d)
    }),
    ("anweisung", |d| {
        kanonisch::<myl_consensus::block::Anweisung>(d)
    }),
    ("konsensnachricht", |d| {
        kanonisch::<myl_consensus::bft::Konsensnachricht>(d)
    }),
    ("stimme", |d| kanonisch::<myl_consensus::bft::Vote>(d)),
    ("polka", |d| {
        kanonisch::<myl_consensus::round_change::PolkaCertificate>(d)
    }),
];

#[allow(dead_code)]
pub fn ziel(name: &str) -> Ziel {
    ZIELE
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, f)| *f)
        .unwrap_or_else(|| panic!("kein Ziel namens {name}"))
}
