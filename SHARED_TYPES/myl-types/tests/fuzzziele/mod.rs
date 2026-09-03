// ⚑ Kein innerer Doc-Kommentar und kein inneres Attribut in dieser
// Datei. Sie wird per `include!` in die Fuzz-Ziele eingesetzt, und
// `include!` darf keine inneren Attribute liefern: Rust verlangt sie
// lexikalisch am Dateianfang. Deshalb `//` statt `//!` und
// `#[allow(...)]` je Element statt `#![allow(...)]` oben.
//
// Die Fuzz-Ziele des Protokollformats, an **einer** Stelle.
//
// # Warum diese Datei nicht im Crate liegt
//
// Sie wird von zwei Seiten benutzt: vom stabilen Lauf in
// `tests/fuzzkorpus.rs` und von den `libfuzzer`-Zielen unter `fuzz/`,
// die eine Nightly-Werkzeugkette brauchen. Beide binden **dieselbe
// Datei** ein, das eine als Modul, das andere per `include!`.
//
// ⚑ **Zwei Fassungen desselben Ziels wären genau der Fehler, den
// dieses Projekt am häufigsten gefunden hat:** ein Wert, einmal kopiert
// und danach nie wieder gegen seine Quelle gehalten. Ein Fuzzer, der
// etwas anderes prüft als der Regressionslauf, findet einen Fehler, den
// die Regression danach nicht festhält.
//
// Im Crate selbst liegt sie deshalb auch nicht: Dort wäre sie
// ausgeliefertes Gewicht, oder sie hinge an einem Feature, das
// irgendwann niemand mehr einschaltet.
//
// # Die Aussage, die jedes Ziel prüft
//
// Nicht „stürzt nicht ab". Das prüft `libfuzzer` von selbst, und es ist
// die schwächere Hälfte. Geprüft wird **Kanonizität**:
//
// > Liest sich eine Bytefolge vollständig als `T`, dann ergibt
// > `borsh::to_vec` genau diese Bytefolge wieder.
//
// ⚑ **Warum das die interessante Aussage ist.** Der Validator des
// Knotens verlangt, dass beim Lesen nichts übrig bleibt, und begründet
// das so: Ein Anhängsel ist ein Kanal, denn zwei Nachrichten mit
// gleichem Inhalt und verschiedenen Bytes haben verschiedene
// Nachrichten-Kennungen und laufen beide durchs Netz. Genau das ist
// Formbarkeit, und genau die findet dieses Prädikat: Wo zwei
// verschiedene Bytefolgen denselben Wert ergeben, ist eine davon nicht
// kanonisch.
//
// Dieselbe Klasse wie Fund 77, nur eine Ebene tiefer: Dort trugen zwei
// Blattfolgen eine Wurzel, hier tragen zwei Bytefolgen einen Wert.
//
// # Was ein Ziel **nicht** tut
//
// Es prüft keine Signatur und keine Semantik. Ein Fuzzer, der eine
// BLS-Prüfung je Eingabe rechnet, schafft dreistellig statt
// sechsstellig viele Läufe je Sekunde, und die Bytes, die er dabei
// erzeugt, kommen ohnehin nie durch. Signaturpfade gehören in ein
// eigenes Ziel mit vorbereiteten Schlüsseln.

use borsh::{BorshDeserialize, BorshSerialize};

/// Liest `daten` als `T` und verlangt, dass die Kodierung kanonisch ist.
///
/// Rückgabe: `true`, wenn sich die Bytes vollständig als `T` lesen
/// ließen. Der Rückgabewert dient allein der Auskunft (wie viele
/// Eingaben überhaupt ankommen); die eigentliche Aussage steht im
/// `assert_eq!`.
///
/// ⚑ **`rest.is_empty()` gehört zur Bedingung, nicht zur Kür.**
/// `borsh::from_slice` ist an dieser Stelle nicht dasselbe: Der Knoten
/// liest über `T::deserialize` und prüft den Rest selbst, und ein Ziel,
/// das eine andere Funktion aufruft als der Betrieb, prüft eine andere
/// Frage.
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
        // Ein Wert, der sich lesen ließ und nicht schreiben lässt, ist
        // selbst ein Fund: Dann gibt es Werte des Typs, die im Netz
        // ankommen und nie wieder hinausgehen.
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

/// Ein Ziel je Typ, der über das Netz oder über die Platte kommt.
///
/// Die Namen sind zugleich die Namen der Korpusverzeichnisse unter
/// `fuzz/corpus/`, deshalb ohne Umlaute und ohne Leerzeichen.
// Ein Ziel nimmt rohe Bytes und sagt, ob sie sich vollstaendig als
// ihr Typ lesen liessen. Als eigener Name, weil `clippy` das Paar aus
// Name und Funktionszeiger sonst als zu verschachtelt meldet, und die
// CI mit `-D warnings` uebersetzt.
#[allow(dead_code)]
pub type Ziel = fn(&[u8]) -> bool;

#[allow(dead_code)]
pub const ZIELE: &[(&str, Ziel)] = &[
    ("segment", |d| kanonisch::<myl_types::Segment>(d)),
    ("poi_buendel", |d| kanonisch::<myl_types::PoIBundle>(d)),
    ("credit", |d| kanonisch::<myl_types::InferenceCredit>(d)),
    ("latenz_attest", |d| kanonisch::<myl_types::LatencyAttest>(d)),
    ("anfechtung", |d| kanonisch::<myl_types::Challenge>(d)),
    ("merkle_beweis", |d| kanonisch::<myl_types::MerkleProof>(d)),
    ("uebergang", |d| kanonisch::<myl_types::TransitionSig>(d)),
    ("miner_anmeldung", |d| kanonisch::<myl_types::MinerRegistration>(d)),
    ("knoten_metadaten", |d| kanonisch::<myl_types::NodeMetadata>(d)),
    ("spuranfrage", |d| kanonisch::<myl_types::Spuranfrage>(d)),
    ("spurantwort", |d| kanonisch::<myl_types::Spurantwort>(d)),
    // 2026-09-03, GATEWAY Stufe 4: Auftrag und Antwort gehen ueber die
    // Leitung und gehoeren damit unter dasselbe Praedikat wie alles
    // andere, was sie geht.
    ("inferenzauftrag", |d| {
        kanonisch::<myl_types::inferenzauftrag::Inferenzauftrag>(d)
    }),
    ("inferenzantwort", |d| {
        kanonisch::<myl_types::inferenzauftrag::Inferenzantwort>(d)
    }),
];

/// Sucht ein Ziel nach Namen.
#[allow(dead_code)]
pub fn ziel(name: &str) -> Ziel {
    ZIELE
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, f)| *f)
        .unwrap_or_else(|| panic!("kein Ziel namens {name}"))
}
