//! **Der Sprecherabdruck von Qwen3-TTS**: aus einer Stimmprobe ein Vektor
//! von 1024 Werten, den der Sprecher als Stimme nimmt (CLIENT 8.5).
//!
//! Ein ECAPA-TDNN (Desplanques u. a. 2020), Vorlage
//! `Qwen3TTSSpeakerEncoder` aus dem Paket `qwen-tts` (Apache 2.0), neu
//! geschrieben:
//!
//! 1. Log-Mel bei 24 kHz: Fenster 1024, Schritt 256, 128 Baender nach
//!    Slaney bis 12 kHz, Betrag statt Leistung, natuerlicher Logarithmus.
//! 2. Eine Faltung auf 512 (Kern 5), drei SE-Res2Net-Bloecke (Kern 3,
//!    Dilatation 2, 3, 4), alle mit Rand `same` durch Spiegeln.
//! 3. Die drei Bloecke nebeneinander auf 1536, aufmerksame Statistik
//!    (gewichtetes Mittel und Streuung), eine Projektion auf 1024.
//!
//! ⚑ **Erst richtig, dann schnell**, wie bei [`crate::tonzerleger`]: in
//! `f32`, ausserhalb des Rechenpfads, Stufe fuer Stufe mit der Referenz
//! verglichen.

use std::collections::BTreeMap;
use std::path::Path;

use crate::spektrum::{fft, hann, mel_filter_fuer, Komplex};
use crate::whisper::{alle_kerne, kopf_lesen, linear_genau, tensor_lesen, Tensor};

/// Abtastrate der Stimmprobe.
pub const RATE: u32 = 24_000;
/// Laenge des Abdrucks.
pub const BREITE: usize = 1024;

const FENSTER: usize = 1024;
const SCHRITT: usize = 256;
const BAENDER: usize = 128;
/// Dilatation der drei SE-Res2Net-Bloecke.
const DILATATION: [usize; 3] = [2, 3, 4];
/// Teile je Res2Net-Block.
const TEILE: usize = 8;

/// **Die Gewichte**, `speaker_encoder.…` aus `model.safetensors`.
pub struct Stimmabdruck {
    t: BTreeMap<String, Tensor>,
}

/// Ein Signal kanalweise: `werte[c * laenge + t]`.
struct Signal {
    kanaele: usize,
    laenge: usize,
    werte: Vec<f32>,
}

impl Stimmabdruck {
    /// Liest den Abdruck aus dem `model.safetensors` des Modells.
    pub fn laden(datei: &Path) -> Result<Self, String> {
        let eintraege = kopf_lesen(datei, |n| n.strip_prefix("speaker_encoder.").map(str::to_string))?;
        if eintraege.is_empty() {
            return Err(format!("{}: kein speaker_encoder", datei.display()));
        }
        let t = alle_kerne(eintraege, |e| tensor_lesen(datei, &e).map(|t| (e.name, t)))
            .into_iter()
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        Ok(Self { t })
    }

    fn w(&self, name: &str) -> &Tensor {
        self.t.get(name).unwrap_or_else(|| panic!("Gewicht {name} fehlt"))
    }

    /// **Faltung mit Rand `same` durch Spiegeln**, danach ReLU, wenn `relu`.
    fn faltung(&self, x: &Signal, name: &str, dilatation: usize, relu: bool) -> Signal {
        let w = self.w(&format!("{name}.weight"));
        let (aus, ein, k) = (w.form[0], w.form[1], w.form[2]);
        assert_eq!(ein, x.kanaele, "{name}: {} Kanaele statt {ein}", x.kanaele);
        let b = &self.w(&format!("{name}.bias")).werte;
        let links = (dilatation * (k - 1) / 2) as isize;
        let n = x.laenge as isize;
        let spiegel = |i: isize| if i < 0 { -i } else if i >= n { 2 * (n - 1) - i } else { i } as usize;
        let mut spalten = vec![0f32; x.laenge * ein * k];
        for r in 0..x.laenge {
            for c in 0..ein {
                let zeile = &x.werte[c * x.laenge..][..x.laenge];
                for j in 0..k {
                    spalten[(r * ein + c) * k + j] = zeile[spiegel(r as isize + (j * dilatation) as isize - links)];
                }
            }
        }
        let y = linear_genau(&spalten, x.laenge, &w.werte, aus);
        let mut s = Signal { kanaele: aus, laenge: x.laenge, werte: vec![0f32; aus * x.laenge] };
        for r in 0..x.laenge {
            for o in 0..aus {
                let v = y[r * aus + o] + b[o];
                s.werte[o * x.laenge + r] = if relu { v.max(0.0) } else { v };
            }
        }
        s
    }

    /// Eine 1×1-Faltung auf einem einzelnen Vektor.
    fn vektor(&self, x: &[f32], name: &str) -> Vec<f32> {
        let w = self.w(&format!("{name}.weight"));
        let b = &self.w(&format!("{name}.bias")).werte;
        let mut y = linear_genau(x, 1, &w.werte, w.form[0]);
        y.iter_mut().zip(b).for_each(|(v, b)| *v += b);
        y
    }

    /// **Ein SE-Res2Net-Block**: TDNN, Res2Net, TDNN, Quetschen und Anregen,
    /// dazu der Eingang.
    fn block(&self, x: &Signal, p: &str, dilatation: usize) -> Signal {
        let h = self.faltung(x, &format!("{p}.tdnn1.conv"), 1, true);
        // Res2Net: der erste Teil bleibt, jeder weitere nimmt den vorigen dazu.
        let teil = h.kanaele / TEILE;
        let laenge = h.laenge;
        let mut r = Signal { kanaele: h.kanaele, laenge, werte: h.werte[..teil * laenge].to_vec() };
        let mut vorher: Option<Vec<f32>> = None;
        for i in 1..TEILE {
            let mut ein = h.werte[i * teil * laenge..][..teil * laenge].to_vec();
            if let Some(v) = &vorher {
                ein.iter_mut().zip(v).for_each(|(a, b)| *a += b);
            }
            let aus = self.faltung(&Signal { kanaele: teil, laenge, werte: ein }, &format!("{p}.res2net_block.blocks.{}.conv", i - 1), dilatation, true);
            r.werte.extend_from_slice(&aus.werte);
            vorher = Some(aus.werte);
        }
        let mut h = self.faltung(&r, &format!("{p}.tdnn2.conv"), 1, true);
        // Quetschen und Anregen: Mittel ueber die Zeit, zwei 1×1, Sigmoid.
        let mittel: Vec<f32> = h.werte.chunks(laenge).map(|z| z.iter().sum::<f32>() / laenge as f32).collect();
        let mut q = self.vektor(&mittel, &format!("{p}.se_block.conv1"));
        q.iter_mut().for_each(|v| *v = v.max(0.0));
        let g = self.vektor(&q, &format!("{p}.se_block.conv2"));
        for (c, zeile) in h.werte.chunks_mut(laenge).enumerate() {
            let s = 1.0 / (1.0 + (-g[c]).exp());
            zeile.iter_mut().zip(&x.werte[c * laenge..][..laenge]).for_each(|(v, e)| *v = *v * s + e);
        }
        h
    }

    /// **Der Abdruck einer Stimmprobe** (24 kHz, Werte in ±1).
    pub fn abdruck(&self, ton: &[f32]) -> Vec<f32> {
        self.abdruck_mit(ton, |_, _| {})
    }

    /// Wie [`Self::abdruck`], und `stufe` bekommt jede Zwischenstufe
    /// (`mel`, `block0` bis `block3`, `mfa`, `asp`), fuer die Probe.
    pub fn abdruck_mit(&self, ton: &[f32], mut stufe: impl FnMut(&str, &[f32])) -> Vec<f32> {
        let (mel, bilder) = mel(ton);
        stufe("mel", &mel);
        let x = Signal { kanaele: BAENDER, laenge: bilder, werte: mel };
        let mut h = self.faltung(&x, "blocks.0.conv", 1, true);
        stufe("block0", &h.werte);
        let mut zusammen = Vec::with_capacity(3 * h.werte.len());
        for (i, d) in DILATATION.iter().enumerate() {
            h = self.block(&h, &format!("blocks.{}", i + 1), *d);
            stufe(&format!("block{}", i + 1), &h.werte);
            zusammen.extend_from_slice(&h.werte);
        }
        let x = Signal { kanaele: 3 * h.kanaele, laenge: bilder, werte: zusammen };
        let x = self.faltung(&x, "mfa.conv", 1, true);
        stufe("mfa", &x.werte);
        let gebuendelt = self.statistik(&x);
        stufe("asp", &gebuendelt);
        self.vektor(&gebuendelt, "fc")
    }

    /// **Aufmerksame Statistik**: Mittel und Streuung je Kanal, gewichtet
    /// mit einer Aufmerksamkeit, die selbst Mittel und Streuung sieht.
    fn statistik(&self, x: &Signal) -> Vec<f32> {
        let (c, n) = (x.kanaele, x.laenge);
        // `gewicht` je Kanal und Zeitpunkt, wie `x`.
        let momente = |gewicht: &[f32]| -> (Vec<f32>, Vec<f32>) {
            (0..c)
                .map(|k| {
                    let (z, g) = (&x.werte[k * n..][..n], &gewicht[k * n..][..n]);
                    let m: f32 = z.iter().zip(g).map(|(v, g)| v * g).sum();
                    let s: f32 = z.iter().zip(g).map(|(v, g)| g * (v - m) * (v - m)).sum();
                    (m, s.max(1e-12).sqrt())
                })
                .unzip()
        };
        let (mittel, streuung) = momente(&vec![1.0 / n as f32; c * n]);
        let mut ein = x.werte.clone();
        for v in mittel.iter().chain(&streuung) {
            ein.extend(std::iter::repeat_n(*v, n));
        }
        let mut a = self.faltung(&Signal { kanaele: 3 * c, laenge: n, werte: ein }, "asp.tdnn.conv", 1, true);
        a.werte.iter_mut().for_each(|v| *v = v.tanh());
        let mut a = self.faltung(&a, "asp.conv", 1, false);
        for zeile in a.werte.chunks_mut(n) {
            let hoechst = zeile.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            zeile.iter_mut().for_each(|v| *v = (*v - hoechst).exp());
            let summe: f32 = zeile.iter().sum();
            zeile.iter_mut().for_each(|v| *v /= summe);
        }
        let (mittel, streuung) = momente(&a.werte);
        [mittel, streuung].concat()
    }
}

/// **Das Log-Mel des Abdrucks**, `[band][bild]`, und die Zahl der Bilder.
///
/// Wie die Vorlage: Spiegelrand von `(1024 − 256) / 2` je Seite, nicht
/// zentriert, periodisches Hann-Fenster, Betrag `√(re² + im² + 10⁻⁹)`,
/// `ln(max(mel, 10⁻⁵))`.
pub fn mel(ton: &[f32]) -> (Vec<f32>, usize) {
    let rand = (FENSTER - SCHRITT) / 2;
    let n = ton.len() as isize;
    let probe = |i: isize| -> f64 {
        let j = if i < 0 { -i } else if i >= n { 2 * (n - 1) - i } else { i };
        f64::from(ton[j as usize])
    };
    let laenge = ton.len() + 2 * rand;
    let bilder = if laenge < FENSTER { 0 } else { 1 + (laenge - FENSTER) / SCHRITT };
    let frequenzen = FENSTER / 2 + 1;
    let filter = mel_filter_fuer(frequenzen, f64::from(RATE) / 2.0, BAENDER);
    let fenster = hann(FENSTER);
    let mut mel = vec![0f32; BAENDER * bilder];
    for t in 0..bilder {
        let start = (t * SCHRITT) as isize - rand as isize;
        let rahmen: Vec<Komplex> = (0..FENSTER).map(|i| Komplex { re: probe(start + i as isize) * fenster[i], im: 0.0 }).collect();
        let spektrum = fft(&rahmen);
        let mut summe = vec![0f64; BAENDER];
        for (k, z) in spektrum.iter().take(frequenzen).enumerate() {
            let betrag = (z.re * z.re + z.im * z.im + 1e-9).sqrt();
            for (m, s) in summe.iter_mut().enumerate() {
                *s += filter[k * BAENDER + m] * betrag;
            }
        }
        for (m, s) in summe.iter().enumerate() {
            mel[m * bilder + t] = s.max(1e-5).ln() as f32;
        }
    }
    (mel, bilder)
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ Die Zahl der Bilder folgt dem Rand der Vorlage: 67 200 Proben
    /// ergeben 262 Bilder, und Stille liegt am Boden des Logarithmus.
    #[test]
    fn das_mel_hat_die_laenge_der_vorlage() {
        let (m, bilder) = mel(&vec![0.0; 67_200]);
        assert_eq!(bilder, 262);
        assert_eq!(m.len(), BAENDER * 262);
        assert!(m.iter().all(|v| (*v - 1e-5f32.ln()).abs() < 1e-3), "Stille ergibt den Boden");
    }
}
