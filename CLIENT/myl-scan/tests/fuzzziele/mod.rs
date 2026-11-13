// ⚑ Kein innerer Doc-Kommentar und kein inneres Attribut in dieser
// Datei. Sie wird per `include!` in die Fuzz-Ziele eingesetzt.
//
// Das Fuzz-Ziel fuer gescannte Buecher, an **einer** Stelle: vom stabilen
// Lauf in `tests/fuzzkorpus.rs` als Modul eingebunden, vom
// `libfuzzer`-Ziel unter `fuzz/` per `include!`. Dasselbe Muster wie in
// `SHARED_TYPES/myl-types` und `CLIENT/myl-senses`.
//
// `pdf`: Jede Eingabe endet beim Zaehlen und beim Rendern der ersten Seite
// mit einem Ergebnis oder einem Fehler; ein Bild bleibt in den Grenzen von
// `myl_scan::pdf`. ⚠️ Ein Absturz in `hayro` waere im Programm ein Fehler
// der Seite (der Renderer laeuft in einem eigenen Faden), im Fuzzer aber ein
// Fund: `libfuzzer-sys` bricht bei jeder Panik ab. Eine Schleife meldet
// libFuzzer ueber `-timeout`, lange vor der Frist je Seite.

/// **PDF**: gezaehlt und gerendert ohne Panik, das Bild in den Grenzen.
#[allow(dead_code)]
pub fn pdf(daten: &[u8]) -> bool {
    let Ok(n) = myl_scan::seitenzahl(daten.to_vec()) else {
        return false;
    };
    if n == 0 {
        return false;
    }
    let mut gerendert = false;
    let _ = myl_scan::seiten_rendern(daten.to_vec(), &[1], myl_scan::FRIST_JE_SEITE, |_, bild| {
        let (b, h) = (bild.width() as f32, bild.height() as f32);
        let kante = myl_scan::KANTE_HOECHSTENS + 1.0;
        assert!(b <= kante && h <= kante && b * h <= myl_scan::PIXEL_HOECHSTENS * 1.01, "{b} x {h} Pixel");
        gerendert = true;
        Ok(())
    });
    gerendert
}

/// Ein Ziel nimmt rohe Bytes und sagt, ob sie sich lesen liessen.
#[allow(dead_code)]
pub type Ziel = fn(&[u8]) -> bool;

/// Die Namen sind zugleich die Korpusverzeichnisse unter `fuzz/corpus/`.
#[allow(dead_code)]
pub const ZIELE: &[(&str, Ziel)] = &[("pdf", pdf)];

/// Sucht ein Ziel nach Namen.
#[allow(dead_code)]
pub fn ziel(name: &str) -> Ziel {
    ZIELE.iter().find(|(n, _)| *n == name).map(|(_, f)| *f).unwrap_or_else(|| panic!("kein Ziel namens {name}"))
}
