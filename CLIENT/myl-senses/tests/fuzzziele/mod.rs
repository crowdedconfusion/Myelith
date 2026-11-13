// ⚑ Kein innerer Doc-Kommentar und kein inneres Attribut in dieser
// Datei. Sie wird per `include!` in die Fuzz-Ziele eingesetzt.
//
// Die Fuzz-Ziele der Sinne (CLIENT 8.1), an **einer** Stelle: vom stabilen
// Lauf in `tests/fuzzkorpus.rs` als Modul eingebunden, von den
// `libfuzzer`-Zielen unter `fuzz/` per `include!`. Dasselbe Muster wie in
// `SHARED_TYPES/myl-types` und `CLIENT/myl-client`.
//
// `wav`: Was sich liest, laesst sich umtasten, mit begrenzter Laenge.
//
// `flac`: Jede Eingabe endet mit einem Ergebnis oder einem Fehler; ein
// Ergebnis hat in jedem Kanal gleich viele Proben, alle innerhalb der
// Breite, und `lesen` gibt Proben in [-1, 1].
//
// `kennzeichen`: Das Manifest nach C2PA einer fremden Datei (RIFF, JUMBF,
// CBOR, Zertifikat) endet mit einem Befund. ⚑ Die Suche nach dem
// Wasserzeichen ist reine Rechnung ueber die Proben in allen 4 096
// Verschiebungen (rund 16 Millionen Schritte je Datei, auch bei einer
// kurzen) und bleibt draussen; mit ihr kam der Fuzzer auf sechs Laeufe je
// Sekunde.

/// **FLAC**: gelesen heisst stimmig.
#[allow(dead_code)]
pub fn flac(daten: &[u8]) -> bool {
    let Ok(f) = myl_senses::flac::dekodieren(daten) else {
        return false;
    };
    let n = f.kanaele[0].len();
    assert!(f.kanaele.iter().all(|k| k.len() == n), "Kanaele verschieden lang");
    let grenze = 1i64 << (f.bits - 1);
    assert!(f.kanaele.iter().flatten().all(|&v| (-grenze..grenze).contains(&i64::from(v))), "Probe ausserhalb der Breite");
    let t = myl_senses::flac::lesen(daten).expect("dekodiert, also lesbar");
    assert!(t.proben.iter().all(|p| p.abs() <= 1.0), "Probe ausserhalb [-1, 1]");
    true
}

/// **WAV**: was sich liest, laesst sich auf 16 kHz umtasten, und daraus
/// werden hoechstens sechzehnmal so viele Proben (die kleinste erlaubte Rate
/// ist 1 kHz, [`myl_senses::ton::RATEN`]).
#[allow(dead_code)]
pub fn wav(daten: &[u8]) -> bool {
    let Ok(t) = myl_senses::ton::lesen(daten) else {
        return false;
    };
    if t.proben.len() > 1 << 16 {
        return true;
    }
    let u = myl_senses::ton::umtasten(&t, 16_000);
    assert!(u.proben.len() <= t.proben.len() * 16 + 1, "{} Proben aus {}", u.proben.len(), t.proben.len());
    true
}

/// **Kennzeichen**: gelesen oder abgelehnt, nie abgestuerzt.
#[allow(dead_code)]
pub fn kennzeichen(daten: &[u8]) -> bool {
    use myl_senses::kennzeichnung::{herkunft, Herkunft};
    herkunft(daten) != Herkunft::Fehlt
}

/// Ein Ziel nimmt rohe Bytes und sagt, ob sie sich lesen liessen.
#[allow(dead_code)]
pub type Ziel = fn(&[u8]) -> bool;

/// Die Namen sind zugleich die Korpusverzeichnisse unter `fuzz/corpus/`.
#[allow(dead_code)]
pub const ZIELE: &[(&str, Ziel)] = &[("flac", flac), ("kennzeichen", kennzeichen), ("wav", wav)];

/// Sucht ein Ziel nach Namen.
#[allow(dead_code)]
pub fn ziel(name: &str) -> Ziel {
    ZIELE.iter().find(|(n, _)| *n == name).map(|(_, f)| *f).unwrap_or_else(|| panic!("kein Ziel namens {name}"))
}
