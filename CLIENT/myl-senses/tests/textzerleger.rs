//! **Der Textzerleger von Qwen3-TTS zerlegt wie die Referenz** (CLIENT 8.4).
//!
//! Referenz `tests/daten/qwen3tts-textzerleger.json`: die Token von
//! `transformers` (`fix_mistral_regex=True`) fuer 22 Saetze in zehn
//! Sprachen, die Vorlage des Sprechers und einen Satz in NFD. Mit
//! `#[ignore]`: braucht `vocab.json` und `merges.txt` aus
//! `MODELS/audio/qwen3-tts-0.6b-base`; in `grosse_proben`.

use myl_senses::textzerleger::Textzerleger;

/// Die Saetze der Referenz, in ihrer Reihenfolge.
const SAETZE: [&str; 22] = [
    "Guten Morgen, der Bericht über die Nacht liegt bereit.",
    "Die Prüfrunde lief mit 5116 Proben ohne Fehler durch!",
    "Straße, Größe, Äpfel und Öl: ÄÖÜ äöü ß.",
    "Er sagte: „Das ist's“ – und ging.",
    "Zeile eins\nZeile zwei\n\nZeile drei",
    "Zwei  Leerzeichen   und drei.",
    "  Vorne Leerraum und hinten   ",
    "The quarterly report isn't ready; we'll send it on Friday (3:45 pm).",
    "I'M SURE THEY'VE GONE, DON'T YOU THINK?",
    "C'est l'été à Paris, n'est-ce pas ? Ça coûte 12,50 €.",
    "¿Dónde está la estación? ¡Muy bien, señor!",
    "Olá, coração! A reunião é às 9h30.",
    "Buongiorno, perché non andiamo all'università?",
    "Привет, как дела? Сегодня 15 ноября 2026 года.",
    "今天天气很好，我们去公园散步吧。",
    "こんにちは、東京へ行きます。ラーメンが好きです！",
    "안녕하세요, 오늘 날씨가 정말 좋네요.",
    "Tabs\tund\tStriche --- und ... Punkte!!!",
    "E-Mail: test@example.org, Preis: 1.234,56 EUR, 100% sicher.",
    "Emoji 😀 und Symbole ★ ✓ → ok",
    "Zahlen 0123456789 und römisch Ⅻ",
    "Myelith rechnet ganzzahlig, damit jede Maschine dasselbe Ergebnis bekommt.",
];

/// Die Zahlenlisten nach jedem Vorkommen von `"schluessel": [`.
fn listen(roh: &str, schluessel: &str) -> Vec<Vec<u32>> {
    roh.split(&format!("\"{schluessel}\": [")).skip(1).map(|t| zahlen(t.split(']').next().expect(schluessel))).collect()
}

fn zahlen(t: &str) -> Vec<u32> {
    t.split(|c: char| !c.is_ascii_digit()).filter_map(|s| s.parse().ok()).collect()
}

fn ordner() -> std::path::PathBuf {
    let o = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/qwen3-tts-0.6b-base");
    assert!(o.join("vocab.json").is_file(), "Der Wortschatz fehlt: {o:?}");
    o
}

#[test]
#[ignore = "braucht vocab.json und merges.txt aus MODELS/audio/qwen3-tts-0.6b-base"]
fn der_textzerleger_zerlegt_wie_die_referenz() {
    let roh = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten/qwen3tts-textzerleger.json")).expect("Referenz");
    let referenz = listen(&roh, "ids");
    assert_eq!(referenz.len(), SAETZE.len(), "Saetze und Referenz passen nicht zusammen");
    let t = std::time::Instant::now();
    let z = Textzerleger::laden(&ordner()).expect("Zerleger");
    eprintln!("[textzerleger] geladen in {:.2} s", t.elapsed().as_secs_f64());
    for (satz, ids) in SAETZE.iter().zip(&referenz) {
        assert!(!ids.is_empty());
        assert_eq!(&z.zerlegen(satz), ids, "anders zerlegt: {satz:?}");
    }
    let vorlage = zahlen(roh.split("\"vorlage\": [").nth(1).expect("vorlage").split(']').next().expect("vorlage"));
    assert_eq!(z.vorlage(SAETZE[0]), vorlage, "die Vorlage weicht ab");
    // NFD: zerlegt geschrieben, zerlegt wie zusammengesetzt.
    let mut nfd = roh.split("\"nfd\": [").nth(1).expect("nfd").split(']').map(zahlen);
    let (zerlegt, zusammen) = (nfd.next().expect("NFD"), nfd.next().expect("NFC"));
    assert_eq!(zerlegt, zusammen, "die Referenz setzt zusammen");
    assert_eq!(z.zerlegen("Pru\u{308}frunde Gro\u{308}ße A\u{308}pfel"), zusammen);
    assert_eq!(z.zerlegen("Prüfrunde Größe Äpfel"), zusammen);
}

/// ⚑ **Gegenprobe**: Ohne Merges zerlegt derselbe Wortschatz byteweise, und
/// die Probe oben muss das bemerken. Sonst vergliche sie nichts.
#[test]
#[ignore = "braucht vocab.json aus MODELS/audio/qwen3-tts-0.6b-base"]
fn ohne_merges_faellt_die_probe() {
    let roh = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten/qwen3tts-textzerleger.json")).expect("Referenz");
    let referenz = listen(&roh, "ids");
    let leer = std::env::temp_dir().join(format!("textzerleger-ohne-merges-{}", std::process::id()));
    std::fs::create_dir_all(&leer).expect("Ordner");
    std::fs::copy(ordner().join("vocab.json"), leer.join("vocab.json")).expect("vocab.json");
    std::fs::write(leer.join("merges.txt"), "#version: 0.2\n").expect("merges.txt");
    let z = Textzerleger::laden(&leer).expect("Zerleger");
    let gleich = SAETZE.iter().zip(&referenz).filter(|(s, ids)| &z.zerlegen(s) == *ids).count();
    let _ = std::fs::remove_dir_all(&leer);
    assert_eq!(gleich, 0, "{gleich} Saetze gleichen der Referenz auch ohne Merges");
    assert_eq!(z.zerlegen("Ab").len(), 2, "byteweise: ein Token je Byte");
}
