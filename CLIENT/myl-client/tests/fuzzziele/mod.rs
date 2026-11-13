// ⚑ Kein innerer Doc-Kommentar und kein inneres Attribut in dieser
// Datei. Sie wird per `include!` in die Fuzz-Ziele eingesetzt, und
// `include!` darf keine inneren Attribute liefern.
//
// Die Fuzz-Ziele des Wallets (CLIENT 7.15), an **einer** Stelle: vom
// stabilen Lauf in `tests/fuzzkorpus.rs` als Modul eingebunden, von den
// `libfuzzer`-Zielen unter `fuzz/` per `include!`. Dasselbe Muster wie in
// `SHARED_TYPES/myl-types`: Zwei Fassungen desselben Ziels waeren zwei
// Wahrheiten.
//
// # Was jedes Ziel prueft
//
// `woerter`: Was `bip39::entropie` annimmt, ist bis auf Gross- und
// Kleinschreibung und Leerraum genau die kanonische Folge der zwoelf
// Woerter. Sonst gaebe es zwei Schreibweisen, die verschieden aussehen
// und dasselbe Wallet oeffnen, oder eine, die ein anderes oeffnet als
// gedacht.
//
// `keystore`: Jede Datei liest sich oder scheitert sauber, ohne Panik;
// eine gelesene schreibt sich zu derselben zurueck, und das Entschluesseln
// endet mit einem Ergebnis oder einem Fehler.

/// So viel darf die Ableitung im Ziel hoechstens kosten: scrypt mit
/// `128 · n · r · p` Bytes Speicher, PBKDF2 mit `c` Runden. Teurere Dateien
/// werden nur gelesen. ⚠️ Der Keystore selbst erlaubt bis 1 GiB; ein Ziel,
/// das das bezahlt, schafft eine Eingabe je Sekunde und findet nichts.
#[allow(dead_code)]
pub const SCRYPT_SPEICHER_IM_ZIEL: u128 = 1 << 20;
#[allow(dead_code)]
pub const PBKDF2_RUNDEN_IM_ZIEL: u32 = 1 << 12;

/// **Woerter**: angenommen heisst kanonisch.
#[allow(dead_code)]
pub fn woerter(daten: &[u8]) -> bool {
    use myl_client::bip39::{entropie, woerter};
    let Ok(text) = std::str::from_utf8(daten) else {
        return false;
    };
    let Ok(e) = entropie(text) else {
        return false;
    };
    let eingegeben: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    assert_eq!(woerter(&e).join(" "), eingegeben.join(" "), "angenommen, aber nicht kanonisch: {text:?}");
    true
}

/// **Keystore**: lesen, zurueckschreiben, und wenn es billig ist,
/// entschluesseln.
#[allow(dead_code)]
pub fn keystore(daten: &[u8]) -> bool {
    use myl_client::keystore::{entschluesseln, KdfParameter, Keystore};
    let Ok(k) = serde_json::from_slice::<Keystore>(daten) else {
        return false;
    };
    let zurueck: Keystore = serde_json::from_str(&serde_json::to_string(&k).expect("schreibbar")).expect("liest sich zurueck");
    assert_eq!(zurueck, k, "ein gelesener Keystore schreibt sich zu einem anderen zurueck");
    let billig = match &k.crypto.kdf.params {
        KdfParameter::Scrypt { n, r, p, .. } => 128 * u128::from(*n) * u128::from(*r) * u128::from(*p) <= SCRYPT_SPEICHER_IM_ZIEL,
        KdfParameter::Pbkdf2 { c, .. } => *c <= PBKDF2_RUNDEN_IM_ZIEL,
    };
    if billig {
        let _ = entschluesseln(&k, "ein Passwort fuer das Ziel");
    }
    true
}

/// **Markdown**: Was das Fenster aus einer Antwort gliedert, enthaelt nie
/// mehr Zeichen als die Antwort; Auszeichnung entsteht nicht, nur Teile aus
/// Text.
#[allow(dead_code)]
pub fn markdown(daten: &[u8]) -> bool {
    use myl_client::markdown::{zerlegen, Block, Teil};
    let text = String::from_utf8_lossy(daten);
    fn teile(t: &[Teil]) -> usize {
        t.iter()
            .map(|t| match t {
                Teil::Text { text } | Teil::Fett { text } | Teil::Kursiv { text } | Teil::Code { text } => text.chars().count(),
                Teil::Verweis { text, ziel } => text.chars().count() + ziel.chars().count(),
            })
            .sum()
    }
    let zeichen: usize = zerlegen(&text)
        .iter()
        .map(|b| match b {
            Block::Absatz { inhalt } | Block::Ueberschrift { inhalt, .. } | Block::Zitat { inhalt } => teile(inhalt),
            Block::Liste { punkte, .. } => punkte.iter().map(|p| teile(p)).sum(),
            Block::Code { sprache, text } => sprache.chars().count() + text.chars().count(),
            Block::Linie => 0,
            Block::Tabelle { kopf, zeilen } => {
                kopf.iter().map(|z| teile(z)).sum::<usize>() + zeilen.iter().flatten().map(|z| teile(z)).sum::<usize>()
            }
        })
        .sum();
    assert!(zeichen <= text.chars().count(), "{zeichen} Zeichen aus {}", text.chars().count());
    true
}

/// Ein Ziel nimmt rohe Bytes und sagt, ob sie sich lesen liessen.
#[allow(dead_code)]
pub type Ziel = fn(&[u8]) -> bool;

/// Die Namen sind zugleich die Korpusverzeichnisse unter `fuzz/corpus/`.
#[allow(dead_code)]
pub const ZIELE: &[(&str, Ziel)] = &[("woerter", woerter), ("keystore", keystore), ("markdown", markdown)];

/// Sucht ein Ziel nach Namen.
#[allow(dead_code)]
pub fn ziel(name: &str) -> Ziel {
    ZIELE.iter().find(|(n, _)| *n == name).map(|(_, f)| *f).unwrap_or_else(|| panic!("kein Ziel namens {name}"))
}
