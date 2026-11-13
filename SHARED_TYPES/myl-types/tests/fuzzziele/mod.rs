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

/// **Adresstext** (CLIENT 7.15): Ein Text, den `aus_text` annimmt, ist bis
/// auf Grossschreibung und Leerraum die kanonische Form `als_text`; gemischte
/// Schreibung nimmt er nie an (BIP-350), und die kanonische Form liest sich
/// zur selben Adresse zurueck. 64 Hexzeichen sind die erlaubte zweite Form.
///
/// ⚑ Dieselbe Frage wie bei `kanonisch`, nur fuer Text: Zwei Texte fuer eine
/// Adresse waeren zwei Kennungen fuer denselben Empfaenger (etwa ueber nicht
/// genullte Fuellbits, nachgesehen in `zu_acht`).
#[allow(dead_code)]
pub fn adresstext(daten: &[u8]) -> bool {
    use myl_types::adresstext::{als_text, aus_text, PRAEFIX_TESTNETZ};
    let Ok(text) = std::str::from_utf8(daten) else {
        return false;
    };
    let Ok(adresse) = aus_text(text, PRAEFIX_TESTNETZ) else {
        return false;
    };
    let kanonisch = als_text(&adresse, PRAEFIX_TESTNETZ);
    assert_eq!(aus_text(&kanonisch, PRAEFIX_TESTNETZ).ok(), Some(adresse), "die kanonische Form liest sich anders");
    let t = text.trim();
    if !(t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit())) {
        assert!(
            !(t.bytes().any(|b| b.is_ascii_lowercase()) && t.bytes().any(|b| b.is_ascii_uppercase())),
            "gemischte Schreibung angenommen: {t:?}"
        );
        assert_eq!(t.to_ascii_lowercase(), kanonisch, "zwei Texte, eine Adresse: {t:?}");
    }
    true
}

/// **Adresstext mit angerechneter Pruefsumme** (CLIENT 7.15): dieselbe Aussage
/// wie `adresstext`, nur baut das Ziel den Text selbst. Das erste Byte waehlt
/// Praefix (Bit 0) und Grossschreibung (Bit 1), jedes weitere ist eine
/// Fuenfergruppe; die bech32m-Summe rechnet das Ziel an.
///
/// 📌 Gemessen am 2026-11-12: Mit einem eingebauten Fehler in `zu_acht` (Fuellbits
/// nicht geprueft) fand `adresstext` ohne Saat in 60 s und 5,6 Millionen Laeufen
/// nichts, denn ein zufaelliger Text hat fast nie eine gueltige Summe, und jede
/// Mutation einer gueltigen Saat zerstoert sie wieder. Hinter der Summe sucht nur
/// dieses Ziel.
///
/// ⚑ Die Summe ist hier eigens nach BIP-173/BIP-350 geschrieben und nicht aus
/// `adresstext.rs` geliehen: Stimmen beide nicht ueberein, liest `aus_text` den
/// Text als Tippfehler, und das Ziel prueft nichts mehr; der Regressionslauf
/// zaehlt deshalb mit, wie viele Eingaben sich lesen liessen.
#[allow(dead_code)]
pub fn adresstext_mit_summe(daten: &[u8]) -> bool {
    const ZEICHEN: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    const GEN: [u32; 5] = [0x3b6a_57b2, 0x2650_8e6d, 0x1ea1_19fa, 0x3d42_33dd, 0x2a14_62b3];
    let Some((&art, rest)) = daten.split_first() else {
        return false;
    };
    let praefix: &[u8] = if art & 1 == 0 { b"tmyl" } else { b"myl" };
    let fuenfer: Vec<u8> = rest.iter().take(90).map(|b| b & 31).collect();
    let mut werte: Vec<u8> = praefix.iter().map(|b| b >> 5).collect();
    werte.push(0);
    werte.extend(praefix.iter().map(|b| b & 31));
    werte.extend_from_slice(&fuenfer);
    werte.extend_from_slice(&[0; 6]);
    let mut chk: u32 = 1;
    for v in werte {
        let oben = chk >> 25;
        chk = ((chk & 0x01ff_ffff) << 5) ^ u32::from(v);
        for (i, g) in GEN.iter().enumerate() {
            if (oben >> i) & 1 == 1 {
                chk ^= g;
            }
        }
    }
    let summe = chk ^ 0x2bc8_30a3;
    let mut text: Vec<u8> = praefix.to_vec();
    text.push(b'1');
    text.extend(fuenfer.iter().map(|f| ZEICHEN[usize::from(*f)]));
    text.extend((0..6).map(|i| ZEICHEN[((summe >> (5 * (5 - i))) & 31) as usize]));
    if art & 2 != 0 {
        text.make_ascii_uppercase();
    }
    adresstext(&text)
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
    // 2026-10-26: Die lokalen Leitungen (Knoten zu Shard, Koordinator und
    // Wallet zu Knoten) und die Vollmacht, die an der Tuer ankommt. Alles,
    // was ueber eine Leitung oder durch die Tuer kommt, gehoert unter
    // dasselbe Praedikat (Angriffsklassen A16 und A20).
    ("ortsfrage", |d| kanonisch::<myl_types::ortsleitung::Ortsfrage>(d)),
    ("ortsantwort", |d| kanonisch::<myl_types::ortsleitung::Ortsantwort>(d)),
    ("relaisfrage", |d| kanonisch::<myl_types::relais::Relaisfrage>(d)),
    ("relaisantwort", |d| kanonisch::<myl_types::relais::Relaisantwort>(d)),
    ("vollmacht", |d| kanonisch::<myl_types::vollmacht::Vollmacht>(d)),
    // 2026-11-12, CLIENT 7.15: der Adresstext, den Wallet und Konsole lesen.
    ("adresstext", adresstext),
    ("adresstext_mit_summe", adresstext_mit_summe),
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
