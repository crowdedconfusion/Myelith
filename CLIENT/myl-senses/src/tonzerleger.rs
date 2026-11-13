//! **Der Tonzerleger von Qwen3-TTS, der Rueckweg**: aus Codes wird Ton
//! (CLIENT 8.4 und 8.5).
//!
//! Qwen3-TTS spricht in Codes, je Bild (12,5 je Sekunde) sechzehn: einen
//! semantischen und fuenfzehn akustische. Dieser Weg macht daraus 24 kHz,
//! Vorlage `Qwen3TTSTokenizerV2Decoder` aus dem Paket `qwen-tts` (Apache
//! 2.0), neu geschrieben:
//!
//! 1. Codebuecher: je Code der Eintrag `embedding_sum / cluster_usage`,
//!    summiert, je Gruppe (der erste Code, die uebrigen) eine 1×1-Projektion
//!    auf 512, beide addiert.
//! 2. Kausale Faltung auf 1024, Kern 3.
//! 3. Acht Transformerebenen der Breite 512: RMSNorm, RoPE, kausale
//!    Aufmerksamkeit mit Fenster, SiLU-MLP, LayerScale.
//! 4. Zweimal transponierte Faltung um den Faktor 2 mit einem
//!    ConvNeXt-Block.
//! 5. Faltung auf 1536, vier Bloecke (SnakeBeta, transponierte Faltung um
//!    8, 5, 4, 3, drei Restbloecke mit Dilatation 1, 3, 9), SnakeBeta,
//!    Faltung auf einen Kanal, geklemmt auf ±1.
//!
//! ⚑ **Erst richtig, dann schnell**, wie bei [`crate::whisper`]: Dieser Weg
//! wird in `f32` Stufe fuer Stufe mit der Referenz verglichen
//! ([`Tonzerleger::laden_genau`]). Er steht ausserhalb des Rechenpfads.
//!
//! ⚑ **Die Faltungen der Decoderbloecke in W8A8** ([`Tonzerleger::laden`]),
//! wie der Encoder von whisper: Dort, bei voller Abtastrate, lag fast die
//! ganze Zeit (gemessen 3,25 von 3,46 s fuer 2,8 s Ton in `f32`).

use std::collections::BTreeMap;
use std::path::Path;

use crate::whisper::{alle_kerne, gelu, je_kern, json, kopf_lesen, linear_a8, linear_genau, tensor_lesen, Ganzmatrix, Json, Tensor, GEBUENDELT_AB};

mod hin;
mod strom;
pub use hin::{hin, hin_mit, Hinweg};
pub use strom::Rueckstrom;

/// Codes je Bild.
pub const CODES: usize = 16;
/// Abtastrate des Tons.
pub const RATE: u32 = 24_000;

/// Die Masse aus `decoder_config` in `config.json`.
#[derive(Debug, Clone)]
struct Masse {
    koepfe: usize,
    kopfbreite: usize,
    fenster: usize,
    rope_basis: f64,
    rms_eps: f32,
    hochtasten: Vec<usize>,
    vorab: Vec<usize>,
}

/// **Die Gewichte des Rueckwegs**, aus `speech_tokenizer/model.safetensors`.
pub struct Tonzerleger {
    t: BTreeMap<String, Tensor>,
    m: Masse,
    /// Die Faltungen der Decoderbloecke in int8, wenn mit
    /// [`Tonzerleger::laden`] gelesen; transponierte schon umgeordnet.
    ganz: BTreeMap<String, Ganzmatrix>,
    /// Rechnen die Buendel W8A8 ([`Tonzerleger::laden`]) statt W8A16 mit
    /// zwei Stellen ([`Tonzerleger::laden_w8a16`])?
    acht: bool,
}

impl Tonzerleger {
    /// Liest den Rueckweg (`decoder.…`) und die Masse aus `config.json`
    /// daneben, die Faltungen der Decoderbloecke in int8.
    pub fn laden(datei: &Path) -> Result<Self, String> {
        Self::laden_mit(datei, true, true)
    }

    /// Wie [`Self::laden`], die Buendel aber W8A16 mit zwei Stellen je
    /// Gewicht (wie [`crate::whisper::Gewichte::laden_w8a16`]).
    pub fn laden_w8a16(datei: &Path) -> Result<Self, String> {
        Self::laden_mit(datei, true, false)
    }

    /// Liest den Rueckweg ganz in `f32`, fuer die Probe gegen die Referenz.
    pub fn laden_genau(datei: &Path) -> Result<Self, String> {
        Self::laden_mit(datei, false, false)
    }

    fn laden_mit(datei: &Path, ganzzahlig: bool, acht: bool) -> Result<Self, String> {
        let ordner = datei.parent().unwrap_or(Path::new("."));
        let roh = std::fs::read(ordner.join("config.json")).map_err(|f| format!("config.json: {f}"))?;
        let Json::Objekt(o) = json(&roh, &mut 0)? else { return Err("config.json ist kein Objekt".into()) };
        let Some(Json::Objekt(d)) = o.get("decoder_config") else { return Err("config.json: decoder_config fehlt".into()) };
        let zahl = |k: &str| match d.get(k) {
            Some(Json::Zahl(z)) => Ok(*z),
            _ => Err(format!("decoder_config: {k} fehlt")),
        };
        let liste = |k: &str| match d.get(k) {
            Some(Json::Liste(l)) => Ok(l.iter().filter_map(|j| if let Json::Zahl(z) = j { Some(*z as usize) } else { None }).collect::<Vec<_>>()),
            _ => Err(format!("decoder_config: {k} fehlt")),
        };
        let m = Masse {
            koepfe: zahl("num_attention_heads")? as usize,
            kopfbreite: zahl("head_dim")? as usize,
            fenster: zahl("sliding_window")? as usize,
            rope_basis: zahl("rope_theta")?,
            rms_eps: zahl("rms_norm_eps")? as f32,
            hochtasten: liste("upsample_rates")?,
            vorab: liste("upsampling_ratios")?,
        };
        if zahl("num_key_value_heads")? as usize != m.koepfe || zahl("num_quantizers")? as usize != CODES {
            return Err("dieser Weg rechnet so viele Schluesselkoepfe wie Koepfe und 16 Codes".into());
        }
        let eintraege = kopf_lesen(datei, |n| n.strip_prefix("decoder.").map(str::to_string))?;
        let t = alle_kerne(eintraege, |e| tensor_lesen(datei, &e).map(|t| (e.name, t)))
            .into_iter()
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let mut z = Self { t, m, ganz: BTreeMap::new(), acht };
        if ganzzahlig {
            // Die Bloecke `decoder.1` bis `decoder.4`: transponierte Faltung
            // (`block.1`) und die Faltungen der Restbloecke.
            let namen: Vec<String> = z.t.keys().filter(|n| n.starts_with("decoder.") && n.contains(".block.") && n.ends_with(".conv.weight")).cloned().collect();
            for n in namen {
                let w = z.t.remove(&n).expect("eben gesehen");
                let m = if n.ends_with(".block.1.conv.weight") {
                    let (ein, aus, k) = (w.form[0], w.form[1], w.form[2]);
                    Ganzmatrix::aus_f32(&quer_gewicht(&w.werte, ein, aus, k), aus * k)
                } else {
                    Ganzmatrix::aus_f32(&w.werte, w.form[0])
                };
                z.t.insert(n.clone(), Tensor { form: w.form, werte: Vec::new() });
                z.ganz.insert(n, m);
            }
        }
        Ok(z)
    }

    fn w(&self, name: &str) -> &[f32] {
        &self.t.get(name).unwrap_or_else(|| panic!("Gewicht {name} fehlt")).werte
    }

    fn form(&self, name: &str) -> &[usize] {
        &self.t.get(name).unwrap_or_else(|| panic!("Gewicht {name} fehlt")).form
    }

    /// Wie viele Proben ein Bild ergibt (1920).
    pub fn proben_je_bild(&self) -> usize {
        self.m.hochtasten.iter().chain(&self.m.vorab).product()
    }
}

/// Ein Signal kanalweise: `werte[c * laenge + t]`.
#[derive(Debug, Clone)]
struct Signal {
    kanaele: usize,
    laenge: usize,
    werte: Vec<f32>,
}

impl Signal {
    fn null(kanaele: usize, laenge: usize) -> Self {
        Self { kanaele, laenge, werte: vec![0.0; kanaele * laenge] }
    }

    /// Zeilenweise je Zeitpunkt: `[t][c]`.
    fn quer(&self) -> Vec<f32> {
        let mut y = vec![0f32; self.werte.len()];
        je_kern(&mut y, self.kanaele, |t0, teil| {
            for (dt, zeile) in teil.chunks_mut(self.kanaele).enumerate() {
                for (c, v) in zeile.iter_mut().enumerate() {
                    *v = self.werte[c * self.laenge + t0 + dt];
                }
            }
        });
        y
    }

    /// Dieses Signal und danach `x`, Kanal fuer Kanal.
    fn daneben(&self, x: &Signal) -> Signal {
        let laenge = self.laenge + x.laenge;
        let mut s = Self::null(self.kanaele, laenge);
        for c in 0..self.kanaele {
            s.werte[c * laenge..][..self.laenge].copy_from_slice(&self.werte[c * self.laenge..][..self.laenge]);
            s.werte[c * laenge + self.laenge..][..x.laenge].copy_from_slice(&x.werte[c * x.laenge..][..x.laenge]);
        }
        s
    }

    /// Die Zeitpunkte ab `t0`.
    fn ab(&self, t0: usize) -> Signal {
        let laenge = self.laenge - t0;
        let mut s = Self::null(self.kanaele, laenge);
        for c in 0..self.kanaele {
            s.werte[c * laenge..][..laenge].copy_from_slice(&self.werte[c * self.laenge + t0..][..laenge]);
        }
        s
    }

    /// Aus `[t][c]` zurueck.
    fn aus_quer(x: &[f32], laenge: usize, kanaele: usize) -> Self {
        let mut s = Self::null(kanaele, laenge);
        je_kern(&mut s.werte, laenge, |c0, teil| {
            for (dc, zeile) in teil.chunks_mut(laenge).enumerate() {
                for (t, v) in zeile.iter_mut().enumerate() {
                    *v = x[t * kanaele + c0 + dc];
                }
            }
        });
        s
    }
}

/// So viele Zeitpunkte faltet [`faltung`] auf einmal, damit die
/// umgeordnete Eingabe bei langen Saetzen nicht den Speicher fuellt.
const FALTSTUECK: usize = 4096;

/// **Kausale Faltung**, Schritt 1: `y[o][t] = b[o] + Σ_c Σ_j w[o][c][j] ·
/// x[c][t − (k−1)·d + j·d]`, links mit Nullen.
fn faltung(x: &Signal, z: &Tonzerleger, name: &str, dilatation: usize) -> Signal {
    let (form, w) = (z.form(&format!("{name}.weight")), z.w(&format!("{name}.weight")));
    let (aus, ein, k) = (form[0], form[1], form[2]);
    assert_eq!(ein, x.kanaele, "{name}: {} Kanaele statt {ein}", x.kanaele);
    let b = z.w(&format!("{name}.bias"));
    let ganz = z.ganz.get(&format!("{name}.weight"));
    let rand = (k - 1) * dilatation;
    let mut y = Signal::null(aus, x.laenge);
    for anfang in (0..x.laenge).step_by(FALTSTUECK) {
        let n = FALTSTUECK.min(x.laenge - anfang);
        let mut spalten = vec![0f32; n * ein * k];
        je_kern(&mut spalten, ein * k, |r0, teil| {
            for (dr, zeile) in teil.chunks_mut(ein * k).enumerate() {
                let r = r0 + dr;
                for c in 0..ein {
                    for j in 0..k {
                        let q = (anfang + r + j * dilatation) as isize - rand as isize;
                        if q >= 0 {
                            zeile[c * k + j] = x.werte[c * x.laenge + q as usize];
                        }
                    }
                }
            }
        });
        let teil = match ganz {
            Some(m) => rechne_ganz(&spalten, n, m, z.acht),
            None => linear_genau(&spalten, n, w, aus),
        };
        je_kern(&mut y.werte, x.laenge, |o0, stueck| {
            for (dobj, zeile) in stueck.chunks_mut(x.laenge).enumerate() {
                let o = o0 + dobj;
                for r in 0..n {
                    zeile[anfang + r] = teil[r * aus + o] + b[o];
                }
            }
        });
    }
    y
}

/// **Kausale Faltung je Kanal** (`groups = kanaele`), Kern `k`.
fn faltung_je_kanal(x: &Signal, z: &Tonzerleger, name: &str) -> Signal {
    let (w, b) = (z.w(&format!("{name}.weight")), z.w(&format!("{name}.bias")));
    let k = w.len() / x.kanaele;
    let mut y = Signal::null(x.kanaele, x.laenge);
    for c in 0..x.kanaele {
        let (ein, wc) = (&x.werte[c * x.laenge..][..x.laenge], &w[c * k..][..k]);
        for t in 0..x.laenge {
            let mut s = 0f32;
            for (j, wj) in wc.iter().enumerate() {
                let q = (t + j) as isize - (k - 1) as isize;
                if q >= 0 {
                    s += wj * ein[q as usize];
                }
            }
            y.werte[c * x.laenge + t] = s + b[c];
        }
    }
    y
}

/// Das Gewicht einer transponierten Faltung `[c][o][j]` als Zeilen
/// `[o·K + j][c]`, damit es eine gewoehnliche Matrix ist.
fn quer_gewicht(w: &[f32], ein: usize, aus: usize, k: usize) -> Vec<f32> {
    let mut wt = vec![0f32; ein * aus * k];
    for c in 0..ein {
        for ok in 0..aus * k {
            wt[ok * ein + c] = w[c * aus * k + ok];
        }
    }
    wt
}

/// Eine int8-Matrix: im Buendel W8A8 (oder mit `acht == false` W8A16 mit
/// zwei Stellen), sonst der exakte Weg.
fn rechne_ganz(x: &[f32], zeilen: usize, m: &Ganzmatrix, acht: bool) -> Vec<f32> {
    if acht && zeilen >= GEBUENDELT_AB {
        linear_a8(x, zeilen, m)
    } else {
        crate::whisper::linear_ganz(x, zeilen, m)
    }
}

/// SnakeBeta: `x + sin²(x · e^α) / (e^β + 10⁻⁹)` je Kanal.
fn snake(x: &mut Signal, z: &Tonzerleger, name: &str) {
    let (a, b) = (z.w(&format!("{name}.alpha")), z.w(&format!("{name}.beta")));
    let laenge = x.laenge;
    je_kern(&mut x.werte, laenge, |c0, teil| {
        for (dc, zeile) in teil.chunks_mut(laenge).enumerate() {
            let c = c0 + dc;
            let (ac, bc) = (a[c].exp(), 1.0 / (b[c].exp() + 0.000_000_001));
            for v in zeile {
                let s = (*v * ac).sin();
                *v += bc * s * s;
            }
        }
    });
}

fn rmsnorm(x: &[f32], g: &[f32], eps: f32) -> Vec<f32> {
    let mut y = vec![0f32; x.len()];
    for (xz, yz) in x.chunks(g.len()).zip(y.chunks_mut(g.len())) {
        let q = xz.iter().map(|v| v * v).sum::<f32>() / g.len() as f32;
        let n = 1.0 / (q + eps).sqrt();
        for ((y, x), w) in yz.iter_mut().zip(xz).zip(g) {
            *y = w * (x * n);
        }
    }
    y
}

fn layernorm(x: &[f32], g: &[f32], b: &[f32], eps: f64) -> Vec<f32> {
    let mut y = vec![0f32; x.len()];
    for (xz, yz) in x.chunks(g.len()).zip(y.chunks_mut(g.len())) {
        let n = g.len() as f64;
        let mittel = xz.iter().map(|v| f64::from(*v)).sum::<f64>() / n;
        let streu = xz.iter().map(|v| (f64::from(*v) - mittel).powi(2)).sum::<f64>() / n;
        let s = 1.0 / (streu + eps).sqrt();
        for i in 0..g.len() {
            yz[i] = ((f64::from(xz[i]) - mittel) * s) as f32 * g[i] + b[i];
        }
    }
    y
}

/// `linear_genau` mit Verschiebung.
fn linear(x: &[f32], zeilen: usize, z: &Tonzerleger, name: &str, mit_b: bool) -> Vec<f32> {
    let w = z.w(&format!("{name}.weight"));
    let aus = z.form(&format!("{name}.weight"))[0];
    let mut y = linear_genau(x, zeilen, w, aus);
    if mit_b {
        let b = z.w(&format!("{name}.bias"));
        y.chunks_mut(aus).for_each(|r| r.iter_mut().zip(b).for_each(|(v, c)| *v += c));
    }
    y
}

/// RoPE wie `rotate_half`: die erste und die zweite Haelfte jedes Kopfes
/// drehen sich gegeneinander.
fn drehen(x: &mut [f32], koepfe: usize, kb: usize, basis: f64) {
    drehen_ab(x, koepfe, kb, basis, 0);
}

/// [`drehen`] fuer Zeilen, deren erste an Stelle `ab` steht.
fn drehen_ab(x: &mut [f32], koepfe: usize, kb: usize, basis: f64, ab: usize) {
    let zeilen = x.len() / (koepfe * kb);
    let halb = kb / 2;
    for t in 0..zeilen {
        for h in 0..koepfe {
            let k = &mut x[(t * koepfe + h) * kb..][..kb];
            for i in 0..halb {
                let f = ((ab + t) as f64 / basis.powf(2.0 * i as f64 / kb as f64)) as f32;
                let (c, s) = (f.cos(), f.sin());
                let (a, b) = (k[i], k[i + halb]);
                k[i] = a * c - b * s;
                k[i + halb] = b * c + a * s;
            }
        }
    }
}

/// **Kausale Aufmerksamkeit** ueber `[t][kopf][kb]`, jede Stelle sieht
/// sich und hoechstens `fenster − 1` davor. `k` und `v` duerfen Stellen vor
/// denen von `q` tragen (der Zwischenspeicher des Stroms): Die Anfragen
/// stehen an ihren letzten Zeilen.
fn aufmerksamkeit(q: &[f32], k: &[f32], v: &[f32], kp: usize, kb: usize, fenster: usize) -> Vec<f32> {
    let n = q.len() / (kp * kb);
    let davor = k.len() / (kp * kb) - n;
    let skala = 1.0 / (kb as f32).sqrt();
    let mut o = vec![0f32; n * kp * kb];
    let mut w = vec![0f32; davor + n];
    for (zeile, i) in (davor..davor + n).enumerate() {
        let von = (i + 1).saturating_sub(fenster);
        for h in 0..kp {
            let qi = &q[(zeile * kp + h) * kb..][..kb];
            let mut hoechst = f32::NEG_INFINITY;
            for j in von..=i {
                w[j] = crate::whisper::skalar(qi, &k[(j * kp + h) * kb..][..kb]) * skala;
                hoechst = hoechst.max(w[j]);
            }
            let mut summe = 0f32;
            for wj in &mut w[von..=i] {
                *wj = (*wj - hoechst).exp();
                summe += *wj;
            }
            let ziel = &mut o[(zeile * kp + h) * kb..][..kb];
            for j in von..=i {
                let f = w[j] / summe;
                for (zv, vv) in ziel.iter_mut().zip(&v[(j * kp + h) * kb..][..kb]) {
                    *zv += f * vv;
                }
            }
        }
    }
    o
}

/// Eine Transformerebene, `x` zeilenweise `[t][512]`; `speicher` traegt
/// Schluessel und Werte der Stellen davor (gedreht), `ab` die Stelle der
/// ersten Zeile. Danach haelt der Speicher die letzten `fenster − 1`.
fn ebene(x: &mut [f32], z: &Tonzerleger, nr: usize, speicher: &mut (Vec<f32>, Vec<f32>), ab: usize) {
    let p = format!("pre_transformer.layers.{nr}");
    let m = &z.m;
    let breite = z.form(&format!("{p}.input_layernorm.weight"))[0];
    let n = x.len() / breite;
    let a = rmsnorm(x, z.w(&format!("{p}.input_layernorm.weight")), m.rms_eps);
    let mut q = linear(&a, n, z, &format!("{p}.self_attn.q_proj"), false);
    let mut k = linear(&a, n, z, &format!("{p}.self_attn.k_proj"), false);
    let v = linear(&a, n, z, &format!("{p}.self_attn.v_proj"), false);
    let (kp, kb) = (m.koepfe, m.kopfbreite);
    drehen_ab(&mut q, kp, kb, m.rope_basis, ab);
    drehen_ab(&mut k, kp, kb, m.rope_basis, ab);
    let (ks, vs) = speicher;
    ks.extend_from_slice(&k);
    vs.extend_from_slice(&v);
    let o = aufmerksamkeit(&q, ks, vs, kp, kb, m.fenster);
    let zeile = kp * kb;
    let behalten = (ks.len() / zeile).min(m.fenster.saturating_sub(1)) * zeile;
    ks.drain(..ks.len() - behalten);
    vs.drain(..vs.len() - behalten);
    let o = linear(&o, n, z, &format!("{p}.self_attn.o_proj"), false);
    let s = z.w(&format!("{p}.self_attn_layer_scale.scale"));
    x.chunks_mut(breite).zip(o.chunks(breite)).for_each(|(xz, oz)| xz.iter_mut().zip(oz).zip(s).for_each(|((v, d), c)| *v += c * d));
    let a = rmsnorm(x, z.w(&format!("{p}.post_attention_layernorm.weight")), m.rms_eps);
    let gate = linear(&a, n, z, &format!("{p}.mlp.gate_proj"), false);
    let up = linear(&a, n, z, &format!("{p}.mlp.up_proj"), false);
    let h: Vec<f32> = gate.iter().zip(&up).map(|(g, u)| g / (1.0 + (-g).exp()) * u).collect();
    let d = linear(&h, n, z, &format!("{p}.mlp.down_proj"), false);
    let s = z.w(&format!("{p}.mlp_layer_scale.scale"));
    x.chunks_mut(breite).zip(d.chunks(breite)).for_each(|(xz, dz)| xz.iter_mut().zip(dz).zip(s).for_each(|((v, d), c)| *v += c * d));
}

/// **Codes zu Ton**: `codes[bild][CODES]`, Ergebnis 24 kHz in `[-1, 1]`.
/// `stufe` bekommt jede Zwischenstufe mit Namen, kanalweise (fuer die Probe).
pub fn zurueck_mit(codes: &[[u16; CODES]], z: &Tonzerleger, stufe: impl FnMut(&str, &[f32])) -> Vec<f32> {
    Rueckstrom::neu().weiter_mit(codes, z, stufe)
}

/// So viele Bilder bekommt der Strom in [`zurueck`] hoechstens auf einmal,
/// damit ein langer Text nicht den Speicher fuellt (wie `chunked_decode`
/// der Vorlage).
///
/// ⚑ **Anders als die Vorlage ohne Bruch an der Stueckgrenze.** Die Vorlage
/// rechnet jedes Stueck mit nur 25 Bildern Zusammenhang neu; ein Strom
/// behaelt seinen Zustand und rechnet ueber die Grenze genau weiter. Bis 300
/// Bilder ist beides dieselbe Rechnung.
pub const STUECK: usize = 300;

/// **Codes zu Ton**, in Stuecken zu [`STUECK`] Bildern durch einen Strom.
pub fn zurueck(codes: &[[u16; CODES]], z: &Tonzerleger) -> Vec<f32> {
    let mut strom = Rueckstrom::neu();
    codes.chunks(STUECK).flat_map(|teil| strom.weiter(teil, z)).collect()
}

/// **Codes hinter einem Vorlauf zu Ton**: Der Vorlauf geht durch den Strom,
/// sein Ton faellt weg, die Codes setzen an. So setzt das Klonen mit
/// Mitschrift die Stimmprobe fort, wie die Vorlage, die beide zusammen
/// umwandelt.
///
/// 📌 **Nur 25 Bilder der Stimmprobe als Zusammenhang** waeren schneller
/// gewesen, aber der Ton wich ueber den ganzen Satz um 20 bis 30 dB ab:
/// Acht Ebenen mit Fenster 72 sehen weit hinter 25 Bilder zurueck. Wer
/// viele Saetze hinter derselben Stimmprobe spricht, nimmt einen
/// [`Rueckstrom`] nach dem Vorlauf und setzt je Satz eine Kopie fort.
pub fn zurueck_hinter(vorlauf: &[[u16; CODES]], codes: &[[u16; CODES]], z: &Tonzerleger) -> Vec<f32> {
    let mut strom = Rueckstrom::neu();
    for teil in vorlauf.chunks(STUECK) {
        strom.weiter(teil, z);
    }
    codes.chunks(STUECK).flat_map(|teil| strom.weiter(teil, z)).collect()
}
