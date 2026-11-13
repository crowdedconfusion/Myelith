//! **Der Textzerleger von Qwen** (byteweises BPE), fuer den Sprecher
//! von Qwen3-TTS (CLIENT 8.4).
//!
//! Aus `vocab.json` und `merges.txt` des Modellordners, ohne Fremdkiste:
//!
//! 1. NFC, soweit ohne Unicode-Tabellen moeglich ([`zusammensetzen`]).
//! 2. Vorzerlegen nach dem Muster von Qwen2 ([`vorzerlegen`]).
//! 3. Jedes Stueck als Bytes, je Byte ein Zeichen der Tabelle von GPT-2,
//!    dann die Merges nach Rang.
//!
//! ⚑ Wortschatz und Merges sind dieselben wie bei Qwen3 (151 643 Woerter,
//! 151 387 Merges, am 2026-11-11 verglichen). Die Referenzumsetzung
//! `qwen-tts` laedt mit `fix_mistral_regex=True` ein anderes Muster zum
//! Vorzerlegen; an 26 Saetzen in zehn Sprachen ergaben beide dieselben
//! Token, die Probe vergleicht mit der Referenz in dieser Einstellung.

use std::collections::HashMap;
use std::path::Path;

use crate::whisper::{json, Json};

/// `<|im_start|>` und `<|im_end|>` der Vorlage.
pub const IM_START: u32 = 151_644;
pub const IM_END: u32 = 151_645;

/// **Der Zerleger**: Woerter und Raenge der Merges.
pub struct Textzerleger {
    woerter: HashMap<String, u32>,
    raenge: HashMap<(String, String), usize>,
    tafel: [char; 256],
}

/// Die Tabelle von GPT-2: Byte zu Zeichen.
fn byte_zeichen() -> [char; 256] {
    let mut tafel = ['\0'; 256];
    let mut n = 0u32;
    for b in 0..256u32 {
        let sichtbar = (33..=126).contains(&b) || (161..=172).contains(&b) || (174..=255).contains(&b);
        tafel[b as usize] = if sichtbar {
            char::from_u32(b).expect("Zeichen")
        } else {
            n += 1;
            char::from_u32(255 + n).expect("Zeichen")
        };
    }
    tafel
}

impl Textzerleger {
    /// Liest `vocab.json` und `merges.txt` aus `ordner`.
    pub fn laden(ordner: &Path) -> Result<Self, String> {
        let roh = std::fs::read(ordner.join("vocab.json")).map_err(|f| format!("vocab.json: {f}"))?;
        let Json::Objekt(o) = json(&roh, &mut 0)? else { return Err("vocab.json ist kein Objekt".into()) };
        let woerter = o.into_iter().filter_map(|(k, v)| if let Json::Zahl(z) = v { Some((k, z as u32)) } else { None }).collect();
        let merges = std::fs::read_to_string(ordner.join("merges.txt")).map_err(|f| format!("merges.txt: {f}"))?;
        let raenge = merges
            .lines()
            .filter(|l| !l.starts_with("#version") && !l.trim().is_empty())
            .enumerate()
            .filter_map(|(i, l)| l.split_once(' ').map(|(a, b)| ((a.to_string(), b.to_string()), i)))
            .collect();
        Ok(Self { woerter, raenge, tafel: byte_zeichen() })
    }

    /// **Text zu Token**, ohne Sondertoken.
    pub fn zerlegen(&self, text: &str) -> Vec<u32> {
        let text = zusammensetzen(text);
        let mut ids = Vec::new();
        for stueck in vorzerlegen(&text) {
            let zeichen: Vec<String> = stueck.bytes().map(|b| self.tafel[b as usize].to_string()).collect();
            for t in self.bpe(zeichen) {
                if let Some(&id) = self.woerter.get(&t) {
                    ids.push(id);
                }
            }
        }
        ids
    }

    /// Die Merges: immer das Paar mit dem kleinsten Rang zusammen, bis keins
    /// mehr geht.
    fn bpe(&self, mut teile: Vec<String>) -> Vec<String> {
        loop {
            let bestes = teile
                .windows(2)
                .enumerate()
                .filter_map(|(i, p)| self.raenge.get(&(p[0].clone(), p[1].clone())).map(|&r| (r, i)))
                .min();
            let Some((_, i)) = bestes else { return teile };
            let paar = (teile[i].clone(), teile[i + 1].clone());
            // Alle Vorkommen dieses Paars von links, wie GPT-2.
            let mut neu = Vec::with_capacity(teile.len());
            let mut j = 0;
            while j < teile.len() {
                if j + 1 < teile.len() && teile[j] == paar.0 && teile[j + 1] == paar.1 {
                    neu.push(format!("{}{}", paar.0, paar.1));
                    j += 2;
                } else {
                    neu.push(teile[j].clone());
                    j += 1;
                }
            }
            teile = neu;
        }
    }

    /// **Die Vorlage des Sprechers**:
    /// `<|im_start|>assistant\n{text}<|im_end|>\n<|im_start|>assistant\n`.
    pub fn vorlage(&self, text: &str) -> Vec<u32> {
        let rolle = self.zerlegen("assistant\n");
        let mut ids = vec![IM_START];
        ids.extend(&rolle);
        ids.extend(self.zerlegen(text));
        ids.push(IM_END);
        ids.extend(self.zerlegen("\n"));
        ids.push(IM_START);
        ids.extend(&rolle);
        ids
    }
}

/// Buchstabe im Sinn von `\p{L}`: alphabetisch, aber keine Zahl (`Nl`).
fn buchstabe(c: char) -> bool {
    c.is_alphabetic() && !c.is_numeric()
}

/// **Vorzerlegen nach dem Muster von Qwen2**, von Hand statt mit einer
/// Regex-Kiste:
///
/// ```text
/// (?i:'s|'t|'re|'ve|'m|'ll|'d) | [^\r\n\p{L}\p{N}]?\p{L}+ | \p{N}
///   | ?[^\s\p{L}\p{N}]+[\r\n]* | \s*[\r\n]+ | \s+(?!\S) | \s+
/// ```
///
/// Die Zweige in dieser Reihenfolge, je Stelle der erste, der passt
/// (wie die Alternation eines Regex).
pub fn vorzerlegen(text: &str) -> Vec<&str> {
    let z: Vec<(usize, char)> = text.char_indices().collect();
    let ende = text.len();
    let ort = |i: usize| if i < z.len() { z[i].0 } else { ende };
    let mut stuecke = Vec::new();
    let mut i = 0;
    while i < z.len() {
        let c = z[i].1;
        let n = |k: usize| z.get(k).map(|p| p.1);
        // 1. Kuerzel, ohne Ruecksicht auf Gross und Klein.
        if c == '\'' {
            let rest: String = z[i + 1..].iter().take(2).map(|p| p.1.to_ascii_lowercase()).collect();
            let laenge = if rest.starts_with("re") || rest.starts_with("ve") || rest.starts_with("ll") {
                3
            } else if rest.starts_with('s') || rest.starts_with('t') || rest.starts_with('m') || rest.starts_with('d') {
                2
            } else {
                0
            };
            if laenge > 0 {
                stuecke.push(&text[ort(i)..ort(i + laenge)]);
                i += laenge;
                continue;
            }
        }
        // 2. Hoechstens ein Zeichen ohne Buchstabe, Ziffer, Zeilenende, dann Buchstaben.
        let vor = !buchstabe(c) && !c.is_numeric() && c != '\r' && c != '\n';
        let ab = if vor { i + 1 } else { i };
        if n(ab).is_some_and(buchstabe) {
            let mut j = ab;
            while n(j).is_some_and(buchstabe) {
                j += 1;
            }
            stuecke.push(&text[ort(i)..ort(j)]);
            i = j;
            continue;
        }
        // 3. Eine einzelne Ziffer.
        if c.is_numeric() {
            stuecke.push(&text[ort(i)..ort(i + 1)]);
            i += 1;
            continue;
        }
        // 4. Hoechstens ein Leerzeichen, Zeichen ohne Leerraum, Buchstabe, Ziffer, dann Zeilenenden.
        let zeichen = |x: char| !x.is_whitespace() && !buchstabe(x) && !x.is_numeric();
        let ab = if c == ' ' { i + 1 } else { i };
        if n(ab).is_some_and(zeichen) {
            let mut j = ab;
            while n(j).is_some_and(zeichen) {
                j += 1;
            }
            while n(j).is_some_and(|x| x == '\r' || x == '\n') {
                j += 1;
            }
            stuecke.push(&text[ort(i)..ort(j)]);
            i = j;
            continue;
        }
        // 5. Leerraum, der in Zeilenenden endet: bis zum letzten Zeilenende des Laufs.
        if c.is_whitespace() {
            let mut j = i;
            while n(j).is_some_and(char::is_whitespace) {
                j += 1;
            }
            let lauf = &z[i..j];
            if let Some(letztes) = lauf.iter().rposition(|p| p.1 == '\r' || p.1 == '\n') {
                stuecke.push(&text[ort(i)..ort(i + letztes + 1)]);
                i += letztes + 1;
                continue;
            }
            // 6. Leerraum, dem nichts anderes folgt: ganz; sonst bis vor das letzte Zeichen.
            if j == z.len() || j - i == 1 {
                stuecke.push(&text[ort(i)..ort(j)]);
                i = j;
            } else {
                stuecke.push(&text[ort(i)..ort(j - 1)]);
                i = j - 1;
            }
            continue;
        }
        // Was kein Zweig nimmt, steht allein (kommt nach dem Muster nicht vor).
        stuecke.push(&text[ort(i)..ort(i + 1)]);
        i += 1;
    }
    stuecke
}

/// **NFC, soweit ohne Tabellen moeglich**: ein Grundbuchstabe mit einem
/// folgenden kombinierenden Akzent (Gravis, Akut, Zirkumflex, Tilde,
/// Trema, Ring, Cedille) wird zum fertigen Zeichen, wo es das in Latin-1
/// gibt; das deckt die Sprachen von Qwen3-TTS mit lateinischer Schrift ab.
/// ⚠️ Andere zerlegte Zeichen bleiben zerlegt.
pub fn zusammensetzen(text: &str) -> String {
    const AKZENTE: [(char, &str, &str); 7] = [
        ('\u{300}', "AEIOUaeiou", "ÀÈÌÒÙàèìòù"),
        ('\u{301}', "AEIOUYaeiouy", "ÁÉÍÓÚÝáéíóúý"),
        ('\u{302}', "AEIOUaeiou", "ÂÊÎÔÛâêîôû"),
        ('\u{303}', "ANOano", "ÃÑÕãñõ"),
        ('\u{308}', "AEIOUaeiouy", "ÄËÏÖÜäëïöüÿ"),
        ('\u{30A}', "Aa", "Åå"),
        ('\u{327}', "Cc", "Çç"),
    ];
    let mut aus = String::with_capacity(text.len());
    let mut zeichen = text.chars().peekable();
    while let Some(c) = zeichen.next() {
        if let Some(&a) = zeichen.peek() {
            if let Some((_, von, nach)) = AKZENTE.iter().find(|(k, _, _)| *k == a) {
                if let Some(p) = von.chars().position(|b| b == c) {
                    aus.push(nach.chars().nth(p).expect("gleich lang"));
                    zeichen.next();
                    continue;
                }
            }
        }
        aus.push(c);
    }
    aus
}

#[cfg(test)]
mod proben {
    use super::*;

    #[test]
    fn das_vorzerlegen_folgt_dem_muster() {
        assert_eq!(vorzerlegen("Hallo Welt"), ["Hallo", " Welt"]);
        assert_eq!(vorzerlegen("It's 12!"), ["It", "'s", " ", "1", "2", "!"]);
        assert_eq!(vorzerlegen("a  b"), ["a", " ", " b"], "Leerraum vor einem Wort laesst eines stehen");
        assert_eq!(vorzerlegen("a\n\nb"), ["a", "\n\n", "b"]);
        assert_eq!(vorzerlegen("Ende   "), ["Ende", "   "]);
        assert_eq!(vorzerlegen("x...\n"), ["x", "...\n"]);
    }

    #[test]
    fn zerlegte_umlaute_werden_zusammengesetzt() {
        assert_eq!(zusammensetzen("Pru\u{308}fung Gro\u{308}ße"), "Prüfung Größe");
        // Gegenprobe: ein Akzent ohne passenden Grundbuchstaben bleibt.
        assert_eq!(zusammensetzen("x\u{308}"), "x\u{308}");
    }
}
