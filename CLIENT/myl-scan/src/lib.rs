//! **Gescannte Buecher lesen**, ohne Netz und ohne Fremdprogramm (Arbeitsname).
//!
//! Kleine Modelle, im Programm eingebaut: Seitenaufbau (Spalten, Titel,
//! Abbildungen, Tabellen, Kopf- und Fusszeilen), Textzeilen finden, Zeilen
//! lesen. Eine Seite braucht rund eine Sekunde auf der CPU; ein Sehmodell oder
//! eine GPU kommt hier nicht vor.
//!
//! ⚠️ **Nicht Teil des Rechenpfads.** Hier wird mit Gleitkomma gerechnet, und
//! zwischen Prozessoren koennen die Texte in Einzelheiten abweichen. Das
//! Ergebnis ist fremder Text fuer Markdown und geht nie in den Konsens.

mod aufbau;
mod auftrag;
mod netz;
mod pdf;
mod text;

pub use auftrag::{ausfuehren, Ende};
pub use aufbau::{Art, Bereich};
pub use pdf::{seiten_rendern, seitenzahl, Renderfehler, DPI, FRIST_JE_SEITE, KANTE_HOECHSTENS, PIXEL_HOECHSTENS};
pub use text::Zeile;

use image::RgbImage;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use tract_onnx::prelude::*;

/// Wo im Markdown eine Abbildung steht; der Aufrufer setzt sie dort ein.
pub const ABBILDUNG: &str = "@@ABBILDUNG@@";
/// Kleinere Ausschnitte sind Zierrat.
const KLEINSTE_KANTE: u32 = 96;

pub(crate) type Plan = Arc<TypedSimplePlan>;

pub(crate) struct Plaene {
    daten: &'static [u8],
    fertig: Mutex<HashMap<Vec<usize>, Plan>>,
}

impl Plaene {
    fn neu(daten: &'static [u8]) -> Self {
        Self { daten, fertig: Mutex::new(HashMap::new()) }
    }

    /// Ein Plan je Eingabegroesse; gebaut wird er einmal.
    pub(crate) fn fuer(&self, form: &[usize]) -> TractResult<Plan> {
        if let Some(p) = self.fertig.lock().expect("Plaene").get(form) {
            return Ok(p.clone());
        }
        let mut modell = auspacken(self.daten)?;
        formen_vergessen(&mut modell)?;
        let plan = modell.with_input_fact(0, f32::fact(form).into())?.into_optimized()?.into_runnable()?;
        self.fertig.lock().expect("Plaene").insert(form.to_vec(), plan.clone());
        Ok(plan)
    }
}

/// Liest ein Modell, dessen Gewichte auf wenige Bit gepackt sind (`modelle/packen.py`).
fn auspacken(daten: &[u8]) -> TractResult<InferenceModel> {
    let onnx = tract_onnx::onnx();
    let mut m = onnx.proto_model_for_read(&mut Cursor::new(daten))?;
    let graph = m.graph.as_mut().ok_or_else(|| TractError::msg("Modell ohne Graph"))?;
    for t in &mut graph.initializer {
        let Some(kopf) = t.doc_string.strip_prefix("q:") else {
            continue;
        };
        let (bit, achse, gruppen) = gepackter_kopf(kopf).ok_or_else(|| TractError::msg("Kopf einer gepackten Tabelle"))?;
        let zahl: usize = t.dims.iter().map(|d| *d as usize).product();
        t.raw_data = stufen_auspacken(&t.raw_data, zahl, bit, achse, gruppen);
        t.doc_string.clear();
    }
    onnx.model_for_proto_model(&m)
}

fn gepackter_kopf(k: &str) -> Option<(u32, i32, usize)> {
    let mut t = k.split(',');
    Some((t.next()?.parse().ok()?, t.next()?.parse().ok()?, t.next()?.parse().ok()?))
}

/// Zahlen zu je `bit` Bit zurueck auf f32. Vorn stehen die Skalen je Gruppe, dann die
/// Bitfolge, kleines Bit zuerst; bei Achse 1 liegen die Gruppen spaltenweise.
pub(crate) fn stufen_auspacken(roh: &[u8], zahl: usize, bit: u32, achse: i32, gruppen: usize) -> Vec<u8> {
    let grenze = (1u32 << bit) - 1;
    let skalen = 8 * gruppen;
    let je_gruppe = zahl / gruppen.max(1);
    let mut werte = vec![0.0f32; zahl];
    let (mut speicher, mut belegt, mut quelle) = (0u32, 0u32, roh[skalen.min(roh.len())..].iter());
    for g in 0..gruppen {
        let zahlen: Vec<f32> = roh[g * 8..g * 8 + 8].chunks(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let (tief, schritt) = (zahlen[0], (zahlen[1] - zahlen[0]) / grenze as f32);
        for i in 0..je_gruppe {
            while belegt < bit {
                speicher |= u32::from(*quelle.next().unwrap_or(&0)) << belegt;
                belegt += 8;
            }
            let ort = if achse == 1 { i * gruppen + g } else { g * je_gruppe + i };
            werte[ort] = tief + (speicher & grenze) as f32 * schritt;
            speicher >>= bit;
            belegt -= bit;
        }
    }
    werte.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Diese Modelle tragen Formen aus ihrer Ausfuhr fuer genau eine Bildgroesse; die
/// Eingabe hier hat eine andere, also muessen sie neu hergeleitet werden.
fn formen_vergessen(m: &mut InferenceModel) -> TractResult<()> {
    let bleiben: Vec<OutletId> = m.input_outlets()?.to_vec();
    for n in 0..m.nodes.len() {
        if m.nodes[n].op.name() == "Const" {
            continue;
        }
        for ausgang in 0..m.nodes[n].outputs.len() {
            let o = OutletId::new(n, ausgang);
            if !bleiben.contains(&o) {
                m.set_outlet_fact(o, InferenceFact::default())?;
            }
        }
    }
    Ok(())
}

/// Was von einer Seite uebrig bleibt.
#[derive(Debug, Default)]
pub struct Seite {
    /// Markdown; jede Abbildung als [`ABBILDUNG`] in einem eigenen Block.
    pub markdown: String,
    /// Die Abbildungen als PNG, in der Reihenfolge der Marken.
    pub abbildungen: Vec<Vec<u8>>,
    /// Die groesste Schrift der Seite; auf einer Titelseite der Titel.
    pub titel: String,
}

/// Eine Schrift, die gelesen werden kann.
pub struct Schrift {
    pub name: &'static str,
    daten: &'static [u8],
    /// Leer bei den kleinen Zeilennetzen.
    zeichen: &'static str,
    /// Ein Zeilennetz statt eines ONNX-Modells.
    pub(crate) netz: bool,
    /// Das Modell liest in der Lage der Glyphen, von links nach rechts.
    pub(crate) von_rechts: bool,
    /// Kann in senkrechten Spalten stehen.
    pub(crate) senkrecht: bool,
}

macro_rules! schrift {
    ($name:literal, $von_rechts:literal) => {
        Schrift {
            name: $name,
            daten: include_bytes!(concat!("../modelle/lesen-", $name, ".onnx")),
            zeichen: include_str!(concat!("../modelle/lesen-", $name, ".zeichen")),
            netz: false,
            von_rechts: $von_rechts,
            senkrecht: matches!($name.as_bytes(), b"chinesisch" | b"koreanisch"),
        }
    };
}

macro_rules! netz {
    ($name:literal, $von_rechts:literal) => {
        Schrift { name: $name, daten: include_bytes!(concat!("../modelle/netz-", $name, ".bin")), zeichen: "", netz: true, von_rechts: $von_rechts, senkrecht: false }
    };
}

/// Alle Schriften; bei Gleichstand gewinnt die erste.
pub const SCHRIFTEN: &[Schrift] = &[
    schrift!("latein", false),
    schrift!("kyrillisch", false),
    schrift!("griechisch", false),
    netz!("arabisch", true),
    schrift!("devanagari", false),
    schrift!("thai", false),
    schrift!("tamil", false),
    schrift!("telugu", false),
    schrift!("chinesisch", false),
    schrift!("koreanisch", false),
    netz!("fraktur", false),
    netz!("altgriechisch", false),
    netz!("hebraeisch", true),
    netz!("bengalisch", false),
    netz!("tibetisch", false),
    netz!("syrisch", true),
];

impl Schrift {
    /// Ob Modell und Zeichenliste zusammengehoeren: eine verrutschte Liste liest
    /// fehlerfrei und falsch.
    pub fn pruefen(&'static self) -> Result<(), String> {
        let l = Leser::neu(self)?;
        if l.netz.is_some() {
            return Ok(());
        }
        let plan = l.plaene.fuer(&[1, 3, 48, 320]).map_err(|f| format!("{f:?}"))?;
        let e: Tensor = tract_ndarray::Array4::<f32>::zeros((1, 3, 48, 320)).into();
        let aus = plan.run(tvec!(e.into())).map_err(|f| format!("{f:?}"))?;
        let klassen = aus[0].shape()[2];
        if klassen == l.zeichen.len() {
            Ok(())
        } else {
            Err(format!("{klassen} Klassen, {} Zeichen", l.zeichen.len()))
        }
    }
}

/// Das Lesemodell einer Schrift.
pub(crate) struct Leser {
    pub(crate) schrift: &'static Schrift,
    pub(crate) plaene: Plaene,
    pub(crate) zeichen: Vec<String>,
    pub(crate) netz: Option<netz::Netz>,
}

impl Leser {
    fn neu(schrift: &'static Schrift) -> Result<Self, String> {
        let mut zeichen: Vec<String> = vec![String::new()];
        zeichen.extend(schrift.zeichen.lines().map(str::to_string));
        zeichen.push(" ".to_string());
        let netz = if schrift.netz { Some(netz::Netz::neu(schrift.daten).map_err(|f| format!("{}: {f}", schrift.name))?) } else { None };
        Ok(Self { schrift, plaene: Plaene::neu(schrift.daten), zeichen, netz })
    }
}

/// Die geladenen Modelle.
pub struct Erkenner {
    pub(crate) finden: Plaene,
    leser: Leser,
    aufbau: Plan,
}

/// Schriften, die sich am Lesesicherheit kaum unterscheiden: Liest das Fachmodell
/// genug von seinen Kennzeichen (langes s, Hauchzeichen), gehoert das Werk ihm.
type Kennzeichen = fn(char) -> bool;

const VERWANDTE: [(&str, &str, Kennzeichen, usize); 2] = [
    ("latein", "fraktur", |c| c == '\u{17f}', 150),
    ("griechisch", "altgriechisch", |c| ('\u{1f00}'..='\u{1fff}').contains(&c), 20),
];

fn verwandte_pruefen(s: &'static Schrift, proben: &mut [(&RgbImage, Vec<Zeile>)]) -> Result<&'static Schrift, String> {
    let Some((_, fach, zeichen, je)) = VERWANDTE.iter().find(|v| v.0 == s.name) else {
        return Ok(s);
    };
    let f = SCHRIFTEN.iter().find(|x| x.name == *fach).ok_or("Fachschrift fehlt")?;
    let leser = Leser::neu(f)?;
    let (mut buchstaben, mut treffer) = (0usize, 0usize);
    for (b, z) in proben.iter_mut() {
        if z.is_empty() {
            continue;
        }
        text::lesen(&leser, b, z).map_err(|f| format!("Lesen: {f:?}"))?;
        for c in z.iter().flat_map(|l| l.text.chars()) {
            buchstaben += usize::from(c.is_alphabetic());
            treffer += usize::from(zeichen(c));
        }
    }
    Ok(if treffer > 0 && treffer * je >= buchstaben { f } else { s })
}

/// Die Buchstaben einer Schrift, ohne Ziffern und Satzzeichen.
fn eigene_buchstaben(name: &str) -> &'static [(u32, u32)] {
    match name {
        "latein" | "fraktur" => &[(0x41, 0x5a), (0x61, 0x7a), (0xc0, 0x24f)],
        "kyrillisch" => &[(0x400, 0x52f), (0x1c80, 0x1c8f), (0xa640, 0xa69f)],
        "griechisch" | "altgriechisch" => &[(0x370, 0x3ff), (0x1f00, 0x1fff)],
        "arabisch" => &[(0x620, 0x65f), (0x66e, 0x6d3), (0x750, 0x77f), (0xfb50, 0xfdff), (0xfe70, 0xfeff)],
        "hebraeisch" => &[(0x591, 0x5f4), (0xfb1d, 0xfb4f)],
        "syrisch" => &[(0x710, 0x74f)],
        "devanagari" => &[(0x900, 0x963), (0x971, 0x97f)],
        "bengalisch" => &[(0x980, 0x9e3), (0x9f0, 0x9f1)],
        "tamil" => &[(0xb80, 0xbe5)],
        "telugu" => &[(0xc00, 0xc65)],
        "thai" => &[(0xe01, 0xe4e)],
        "tibetisch" => &[(0xf00, 0xf1f), (0xf34, 0xfff)],
        "chinesisch" => &[(0x3041, 0x30ff), (0x3400, 0x9fff), (0xf900, 0xfaff)],
        "koreanisch" => &[(0x1100, 0x11ff), (0x3131, 0x318e), (0xac00, 0xd7a3)],
        _ => &[],
    }
}

/// Wie viel des Gelesenen wirklich Buchstaben dieser Schrift sind.
///
/// 📌 Die Sicherheit allein taeuscht: Wer die falsche Schrift liest, liefert oft Ziffern,
/// Satzzeichen oder fremde Buchstaben, und das mit hoher Sicherheit. Das tibetische Netz
/// las einen arabischen Druck von 1888 mit 0,62 und das arabische Modell nur mit 0,56.
/// Gemessen an 60 Proben, darunter alte Drucke und stark verschlechterte Scans: 59 richtig.
/// Eine Liste haeufiger Woerter brachte nichts, Unsinn trifft die kurzen ebenso.
fn anteil_eigener<'a>(s: &Schrift, texte: impl Iterator<Item = &'a str>) -> f32 {
    let bereiche = eigene_buchstaben(s.name);
    let (mut eigen, mut alle) = (0usize, 0usize);
    for c in texte.flat_map(str::chars).filter(|c| !c.is_whitespace()) {
        alle += 1;
        eigen += usize::from(bereiche.iter().any(|&(a, b)| (a..=b).contains(&(c as u32))));
    }
    eigen as f32 / alle.max(1) as f32
}

/// Darunter ist auch die beste Schrift nur geraten; Lesende sollen sie dann festlegen koennen.
const SCHRIFT_SICHER: f32 = 0.7;

/// So viele Zeilen entscheiden ueber die Schrift eines Werks.
const PROBEZEILEN: usize = 16;
/// Um so viel muss eine andere Schrift besser lesen als Latein.
const VORSPRUNG: f32 = 0.02;

impl Erkenner {
    /// Mit `schrift`-Namen fest, sonst zunaechst Latein bis zu [`Erkenner::schrift_waehlen`].
    pub fn neu(schrift: Option<&str>) -> Result<Self, String> {
        let gewaehlt = match schrift {
            None => &SCHRIFTEN[0],
            Some(n) => SCHRIFTEN.iter().find(|s| s.name == n).ok_or_else(|| {
                format!("die Schrift {n} gibt es nicht; moeglich: {}", SCHRIFTEN.iter().map(|s| s.name).collect::<Vec<_>>().join(", "))
            })?,
        };
        let aufbau = (|| -> TractResult<Plan> {
            auspacken(include_bytes!("../modelle/aufbau.onnx"))?
                .with_input_fact(0, f32::fact([1, 3, aufbau::KANTE, aufbau::KANTE]).into())?
                .with_input_fact(1, f32::fact([1, 2]).into())?
                .into_optimized()?
                .into_runnable()
        })()
        .map_err(|f| format!("das Aufbaumodell laedt nicht: {f}"))?;
        Ok(Self { finden: Plaene::neu(include_bytes!("../modelle/text-finden.onnx")), leser: Leser::neu(gewaehlt)?, aufbau })
    }

    /// Welche Schrift gerade gelesen wird.
    pub fn schrift(&self) -> &'static str {
        self.leser.schrift.name
    }

    /// Bestimmt die Schrift eines Werks aus einigen Seiten: Es gewinnt das Modell,
    /// das sich beim Lesen der laengsten Zeilen am sichersten ist.
    pub fn schrift_waehlen(&mut self, bilder: &[RgbImage]) -> Result<(), String> {
        let mut proben: Vec<(&RgbImage, Vec<Zeile>)> = Vec::new();
        for b in bilder {
            let mut z = text::finden(self, b).map_err(|f| format!("Textzeilen: {f:?}"))?;
            z.sort_by(|a, c| (c.laenge()).total_cmp(&a.laenge()));
            z.truncate(PROBEZEILEN / bilder.len().max(1) + 1);
            proben.push((b, z));
        }
        if proben.iter().all(|p| p.1.is_empty()) {
            return Ok(());
        }
        let mut beste: Option<(f32, f32, &'static Schrift)> = None;
        for s in SCHRIFTEN {
            let leser = Leser::neu(s)?;
            let (mut summe, mut zahl) = (0.0f32, 0usize);
            for (b, z) in &mut proben {
                if z.is_empty() {
                    continue;
                }
                summe += text::lesen(&leser, b, z).map_err(|f| format!("Lesen: {f:?}"))?;
                zahl += 1;
            }
            let wert = summe / zahl.max(1) as f32 * anteil_eigener(s, proben.iter().flat_map(|p| p.1.iter()).map(|z| z.text.as_str()));
            let g = wert - if std::ptr::eq(s, &SCHRIFTEN[0]) { 0.0 } else { VORSPRUNG };
            if beste.is_none_or(|(h, _, _)| g > h) {
                beste = Some((g, wert, s));
            }
        }
        if let Some((_, wert, s)) = beste {
            if wert < SCHRIFT_SICHER {
                eprintln!("[myl scan] Schrift unsicher erkannt: {} ({wert:.2}). Falls das nicht stimmt, die Schrift festlegen.", s.name);
            }
            self.leser = Leser::neu(verwandte_pruefen(s, &mut proben)?)?;
        }
        Ok(())
    }

    /// Eine Seite lesen. Mit `nur_abbildungen` bleibt der Text ungelesen; dann
    /// ist nur `abbildungen` gefuellt, fuer Seiten, die eine Textebene haben.
    pub fn seite(&self, bild: &RgbImage, nur_abbildungen: bool) -> Result<Seite, String> {
        let bereiche = aufbau::finden(&self.aufbau, bild).map_err(|f| format!("Seitenaufbau: {f}"))?;
        let mut zeilen = text::finden(self, bild).map_err(|f| format!("Textzeilen: {f:?}"))?;
        if !nur_abbildungen {
            text::lesen(&self.leser, bild, &mut zeilen).map_err(|f| format!("Lesen: {f}"))?;
        }
        let (markdown, ausschnitte) = aufbau::zusammensetzen(bild, &bereiche, &zeilen);
        let mut abbildungen = Vec::new();
        for b in ausschnitte {
            let mut aus = Vec::new();
            b.write_to(&mut Cursor::new(&mut aus), image::ImageFormat::Png).map_err(|f| format!("PNG: {f}"))?;
            abbildungen.push(aus);
        }
        let titel = if nur_abbildungen { String::new() } else { aufbau::titelkandidat(&zeilen) };
        Ok(Seite { markdown: if nur_abbildungen { String::new() } else { markdown }, abbildungen, titel })
    }

    /// Der Text in einer Abbildung, Zeile fuer Zeile von oben nach unten.
    pub fn bildtext(&self, bild: &RgbImage) -> Result<Vec<String>, String> {
        let mut zeilen = text::finden(self, bild).map_err(|f| format!("Textzeilen: {f:?}"))?;
        text::lesen(&self.leser, bild, &mut zeilen).map_err(|f| format!("Lesen: {f}"))?;
        if text::senkrecht(&zeilen) {
            text::spalten_ordnen(&mut zeilen, |z| z);
        } else {
            zeilen.sort_by(|a, b| (a.y0 + a.y1).total_cmp(&(b.y0 + b.y1)));
        }
        Ok(zeilen.into_iter().map(|z| z.text).filter(|t| !t.trim().is_empty()).collect())
    }
}

pub(crate) fn klein_genug(b: &RgbImage) -> bool {
    b.width().min(b.height()) < KLEINSTE_KANTE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gepackte_zahlen_kommen_zurueck() {
        // Zwei Gruppen je zwei Werte zu 4 Bit, zeilenweise.
        let mut roh = Vec::new();
        for z in [0.0f32, 15.0, -1.0, 1.0] {
            roh.extend_from_slice(&z.to_le_bytes());
        }
        roh.extend_from_slice(&[0xF3, 0xF0]);
        let f = |b: Vec<u8>| -> Vec<f32> { b.chunks(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect() };
        assert_eq!(f(stufen_auspacken(&roh, 4, 4, 0, 2)), [3.0, 15.0, -1.0, 1.0]);
        // Achse 1: Die Gruppen sind Spalten.
        assert_eq!(f(stufen_auspacken(&roh, 4, 4, 1, 2)), [3.0, -1.0, 15.0, 1.0]);
    }

    fn schrift(name: &str) -> &'static Schrift {
        SCHRIFTEN.iter().find(|s| s.name == name).unwrap()
    }

    #[test]
    fn nur_eigene_buchstaben_zaehlen() {
        assert_eq!(anteil_eigener(schrift("kyrillisch"), ["rogue went"].into_iter()), 0.0);
        assert_eq!(anteil_eigener(schrift("latein"), ["rogue went"].into_iter()), 1.0);
        assert_eq!(anteil_eigener(schrift("arabisch"), ["22 ⏎ 72"].into_iter()), 0.0);
        assert_eq!(anteil_eigener(schrift("arabisch"), ["في من", "42"].into_iter()), 4.0 / 6.0);
        assert_eq!(anteil_eigener(schrift("tibetisch"), ["ཀ༢"].into_iter()), 0.5, "tibetische Ziffern sind keine Buchstaben");
        assert_eq!(anteil_eigener(schrift("latein"), [""].into_iter()), 0.0);
    }

    #[test]
    fn jede_schrift_kennt_ihre_buchstaben() {
        for s in SCHRIFTEN {
            assert!(!eigene_buchstaben(s.name).is_empty(), "{}", s.name);
        }
    }

    #[test]
    fn koreanisch_steht_auch_senkrecht() {
        assert!(schrift("koreanisch").senkrecht && schrift("chinesisch").senkrecht && !schrift("latein").senkrecht);
    }

    #[test]
    fn jedes_zeilennetz_laedt() {
        for s in SCHRIFTEN.iter().filter(|s| s.netz) {
            assert!(netz::Netz::neu(s.daten).is_ok(), "{}", s.name);
        }
    }

    #[test]
    fn ein_zerschnittenes_zeilennetz_ist_ein_fehler() {
        let s = SCHRIFTEN.iter().find(|s| s.netz).unwrap();
        for n in [0, 3, 40, s.daten.len() / 2, s.daten.len() - 1] {
            assert!(netz::Netz::neu(&s.daten[..n]).is_err(), "bei {n} Bytes");
        }
    }
}

