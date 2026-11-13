//! **Qwen3-TTS als Sprechweg**: der eigene Rechenweg im Prozess, statt
//! eines Laeufers mit Python (CLIENT 8.5).
//!
//! Zerleger, Sprecher und Rueckweg bleiben geladen, solange der
//! [`crate::sprechen::Dauersprecher`] steht; die Stimme wird einmal aus der
//! Stimmprobe vorbereitet: der Abdruck immer, mit dem Text der Probe dazu
//! ihre Codes fuer das Klonen mit Mitschrift. Je Satz ein frischer Seed
//! ([`crate::sprecher::sprechen_frei`]), ohne Sprachtoken (das Modell
//! erkennt die Sprache am Text, wie `auto` der Vorlage).
//!
//! ⚑ **Gestroemt**: Der Rueckweg haelt seinen Zustand ([`Rueckstrom`]); die
//! Stimmprobe geht beim Laden einmal hindurch, jeder Satz setzt eine Kopie
//! fort und gibt Stuecke zu [`STUECK_BILDER`] Bildern aus, sobald sie
//! gewaehlt sind. Der Ton ist derselbe wie in einem Zug (in `f32` bitgleich).
//!
//! ⚑ **Der Seed wird mitgeschrieben** (Festlegung des Projektinhabers,
//! 2026-11-11), mit einem Fingerabdruck des Satzes statt des Satzes: Der
//! Text gehoert dem Gespraech und nicht dem Protokoll.

use std::path::Path;

use crate::sprecher::{bildgrenze, frischer_samen, sprechen_mit, Sprecher, Stimme, Wahl};
use crate::stimmabdruck::Stimmabdruck;
use crate::textzerleger::Textzerleger;
use crate::tonzerleger::{hin, Hinweg, Rueckstrom, Tonzerleger, RATE, STUECK};

/// Der Ordner der Gewichte unter einem Gewichtsort.
pub const ORDNER: &str = "qwen3-tts-0.6b-base";

/// **Ein fester Seed fuer jeden Satz**, statt eines frischen: zum
/// Nachrechnen eines Laufs aus dem Protokoll und fuer die Proben.
pub const SEED_UMGEBUNG: &str = "MYL_SPRECHEN_SEED";

/// Der feste Seed aus [`SEED_UMGEBUNG`], wenn er gesetzt und lesbar ist.
fn fester_seed() -> Option<u64> {
    std::env::var(SEED_UMGEBUNG).ok().and_then(|w| w.trim().parse().ok())
}

/// **Bilder je gestroemtem Stueck** (0,96 s Ton). ⚑ Gleich gross statt
/// wachsend: Solange das Verhaeltnis von Rechenzeit zu Tondauer unter eins
/// liegt, ist das naechste Stueck fertig, bevor das vorige verklungen ist.
pub const STUECK_BILDER: usize = 12;

/// **Bis zu welchem Verhaeltnis von Rechenzeit zu Tondauer gestroemt wird.**
/// ⚑ Ueber eins, mit Luft: Auf dem Mac liegt es bei 0,97 bis 1,01, und eine
/// Antwort aus fuenf Saetzen klang gestroemt ohne Luecke; die Referenzklasse
/// liegt bei 1,5.
pub const STROM_BIS: f64 = 1.15;

/// **Die geladenen Teile und die vorbereitete Stimme.**
pub struct Eigensprecher {
    zerleger: Textzerleger,
    sprecher: Sprecher,
    rueckweg: Tonzerleger,
    stimme: Option<Stimme>,
    /// Der Rueckweg nach der Stimmprobe (mit Mitschrift), sonst am Anfang.
    strom: Rueckstrom,
    /// Rechenzeit durch Tondauer des letzten Satzes; vor dem ersten die
    /// Vermutung nach der Maschine (mit Metal schnell genug).
    tempo: std::sync::Mutex<f64>,
}

impl std::fmt::Debug for Eigensprecher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (stimme, mitschrift) = self.stimme();
        f.debug_struct("Eigensprecher").field("stimme", &stimme).field("mitschrift", &mitschrift).finish()
    }
}

/// Was ein Satz ergab.
pub struct Gesprochen {
    pub ton: crate::ton::Ton,
    pub samen: u64,
    pub bilder: usize,
}

impl Eigensprecher {
    /// **Laedt aus `ordner`** und bereitet die Stimme vor, falls eine
    /// Stimmprobe liegt (`probe`, beliebige Abtastrate; `probentext` ihr
    /// Text, dann mit Mitschrift geklont).
    pub fn laden(ordner: &Path, probe: Option<&Path>, probentext: Option<&Path>) -> Result<Self, String> {
        let gewichte = ordner.join("model.safetensors");
        let ton = ordner.join("speech_tokenizer/model.safetensors");
        crate::wachen(crate::Art::Ton, &[&gewichte, &ton])?;
        let zerleger = Textzerleger::laden(ordner)?;
        let sprecher = Sprecher::laden(&gewichte)?;
        // ⚑ **Der Rueckweg nach der Maschine.** Mit Metal zwei int8-Stellen
        // (W8A16): W8A8 kostete gegen `f32` 0,025 Aehnlichkeit der Stimme (15
        // Saetze, CAM++), W8A16 nichts, bei 17 Prozent mehr Zeit. Ohne
        // Grafikeinheit W8A8: Auf 4 Kernen war W8A16 dreieinhalbmal so
        // langsam (13,1 gegen 3,8 s fuer 4,3 s Ton).
        let metal = integer_llm_kernels::ressourcen::metal::verfuegbar();
        let rueckweg = if metal { Tonzerleger::laden_w8a16(&ton)? } else { Tonzerleger::laden(&ton)? };
        let stimme = match probe {
            None => None,
            Some(p) => Some(stimme_vorbereiten(&zerleger, &gewichte, &ton, p, probentext)?),
        };
        // ⚑ Die Stimmprobe einmal durch den Rueckweg, nicht je Satz.
        let mut strom = Rueckstrom::neu();
        if let Some((_, vorbild)) = stimme.as_ref().and_then(|s| s.vorbild.as_ref()) {
            for teil in vorbild.chunks(STUECK) {
                strom.weiter(teil, &rueckweg);
            }
        }
        let tempo = std::sync::Mutex::new(if metal { 0.0 } else { f64::INFINITY });
        Ok(Self { zerleger, sprecher, rueckweg, stimme, strom, tempo })
    }

    /// Ob mit einer Stimmprobe gesprochen wird, und ob mit Mitschrift.
    pub fn stimme(&self) -> (bool, bool) {
        (self.stimme.is_some(), self.stimme.as_ref().is_some_and(|s| s.vorbild.is_some()))
    }

    /// **Spricht einen Satz** und meldet den Seed auf `stderr`.
    pub fn satz(&self, text: &str) -> Result<Gesprochen, String> {
        self.satz_stueckweise(text, &mut |_| ())
    }

    /// **Spricht einen Satz im Strom**: `stueck` bekommt je
    /// [`STUECK_BILDER`] Bildern ihren Ton (24 kHz), sobald er fertig ist;
    /// zurueck kommt der ganze Satz.
    ///
    /// ⚑ **Gestroemt nur, solange etwa so schnell wie Echtzeit**
    /// ([`STROM_BIS`], gemessen am vorigen Satz): Langsamer kaemen die Stuecke spaeter, als das vorige
    /// verklingt, und der Satz stotterte (im Behaelter mit 4 Kernen 50 s
    /// Luecken in 20 s Ton). Dann kommt der Satz am Stueck, als ein Stueck,
    /// und die Pause liegt zwischen den Saetzen statt in den Woertern.
    ///
    /// ⚠️ **Kein Neuversuch an der [`bildgrenze`]**, anders als
    /// [`crate::sprecher::sprechen_frei`]: Was gestroemt ist, hat schon
    /// geklungen. Ein Lauf bis zur Grenze wird gemeldet.
    pub fn satz_stueckweise(&self, text: &str, stueck: &mut dyn FnMut(Vec<f32>)) -> Result<Gesprochen, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("es gibt nichts zu sagen".into());
        }
        let ids = self.zerleger.vorlage(text);
        let uhr = std::time::Instant::now();
        let stroemen = *self.tempo.lock().expect("Tempo") < STROM_BIS;
        let samen = fester_seed().unwrap_or_else(frischer_samen);
        let grenze = bildgrenze(&ids);
        let mut strom = self.strom.clone();
        let (mut offen, mut proben) = (Vec::new(), Vec::new());
        let mut abgeben = |offen: &mut Vec<[u16; crate::tonzerleger::CODES]>, proben: &mut Vec<f32>| {
            if offen.is_empty() {
                return;
            }
            let ton = strom.weiter(offen, &self.rueckweg);
            offen.clear();
            proben.extend_from_slice(&ton);
            if stroemen {
                stueck(ton);
            }
        };
        let (codes, _) = sprechen_mit(&self.sprecher, &ids, None, self.stimme.as_ref(), grenze, Wahl::Zufall(samen), &mut |bild| {
            offen.push(*bild);
            if offen.len() >= STUECK_BILDER {
                abgeben(&mut offen, &mut proben);
            }
        });
        abgeben(&mut offen, &mut proben);
        if !stroemen && !proben.is_empty() {
            stueck(proben.clone());
        }
        let dauer = proben.len() as f64 / f64::from(RATE);
        if dauer > 0.0 {
            *self.tempo.lock().expect("Tempo") = uhr.elapsed().as_secs_f64() / dauer;
        }
        if codes.is_empty() {
            return Err("Qwen3-TTS hat fuer diesen Satz keinen Ton gewaehlt".into());
        }
        let ende = if codes.len() >= grenze { ", kein Ende vor der Bildgrenze" } else { "" };
        eprintln!(
            "[sprechen] Qwen3-TTS: Seed {samen}, Satz {:016x}, {} Bilder{ende}",
            crate::sprechen::fingerabdruck(text.as_bytes()),
            codes.len()
        );
        Ok(Gesprochen { ton: crate::ton::Ton { rate: RATE, proben }, samen, bilder: codes.len() })
    }
}

/// Abdruck und, mit Text, die Codes der Stimmprobe; die Teile dafuer werden
/// nur dafuer geladen und gleich wieder freigegeben.
fn stimme_vorbereiten(zerleger: &Textzerleger, gewichte: &Path, ton: &Path, probe: &Path, probentext: Option<&Path>) -> Result<Stimme, String> {
    let roh = std::fs::read(probe).map_err(|f| format!("{}: {f}", probe.display()))?;
    let probe_ton = crate::ton::lesen(&roh).map_err(|f| format!("{}: {f:?}", probe.display()))?;
    let proben = crate::ton::umtasten(&probe_ton, RATE).proben;
    let abdruck = Stimmabdruck::laden(gewichte)?.abdruck(&proben);
    let vorbild = match probentext {
        Some(t) => {
            let text = std::fs::read_to_string(t).map_err(|f| format!("{}: {f}", t.display()))?;
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                Some((zerleger.zerlegen(text), hin(&proben, &Hinweg::laden(ton)?)))
            }
        }
        None => None,
    };
    Ok(Stimme { abdruck, vorbild })
}
