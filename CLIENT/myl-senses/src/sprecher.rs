//! **Der Sprecher von Qwen3-TTS**: aus Text werden Codes (CLIENT 8.4, 8.5).
//!
//! Zwei Spuren, je Stelle addiert: eine Textspur (Qwen-Token ueber
//! `text_embedding` und eine Projektion von 2048 auf 1024) und eine
//! Codespur (Sprache, Sprecherabdruck, die Codes des letzten Bildes). Ein
//! Decoder wie Qwen3 (28 Ebenen, RMSNorm je Kopf auf `q` und `k`, GQA, RoPE)
//! waehlt je Bild den ersten von 16 Codes; die Code-Vorhersage (5 Ebenen,
//! je Code ein eigener Kopf) ergaenzt die uebrigen fuenfzehn. Aus den Codes
//! macht [`crate::tonzerleger`] Ton. Vorlage
//! `Qwen3TTSForConditionalGeneration.generate` aus dem Paket `qwen-tts`
//! (Apache 2.0), neu geschrieben.
//!
//! ⚑ **Gierig**, damit der Weg Code fuer Code mit der Referenz vergleichbar
//! ist; mit Wiederholungsstrafe, gesperrten Codes und Mindestlaenge wie
//! dort. Ausserhalb des Rechenpfads.
//!
//! ⚑ **Die Matrizen in int8, eine Stelle je Gewicht** ([`Sprecher::laden`]),
//! wie die Gewichte des Sprachmodells: int8 mal int16, exakt in i64, eine
//! Zweierpotenz je Zeile. [`Sprecher::laden_genau`] rechnet in `f32`, fuer
//! die Proben gegen die Referenz.

use std::collections::BTreeMap;
use std::path::Path;

use crate::tonzerleger::CODES;
use crate::whisper::{alle_kerne, json, kopf_lesen, linear_ganz, linear_genau, skalar, tensor_lesen, zeile_lesen, Eintrag, Ganzmatrix, Json, Tensor};

/// Die Masse eines Decoders (Sprecher oder Code-Vorhersage).
#[derive(Debug, Clone)]
struct Decoder {
    praefix: String,
    ebenen: usize,
    koepfe: usize,
    kv_koepfe: usize,
    kopfbreite: usize,
    rope_basis: f64,
    eps: f32,
}

/// Die Sondertoken der Codespur und der Textspur.
#[derive(Debug, Clone)]
struct Zeichen {
    codec_bos: u32,
    codec_eos: u32,
    codec_pad: u32,
    denken: u32,
    denken_anfang: u32,
    denken_ende: u32,
    nicht_denken: u32,
    sprachen: BTreeMap<String, u32>,
    tts_bos: u32,
    tts_eos: u32,
    tts_pad: u32,
    woerter: usize,
}

/// **Die Gewichte des Sprechers**, aus `model.safetensors` (ohne den
/// Sprecherabdruck, der eine eigene Stufe ist).
pub struct Sprecher {
    t: BTreeMap<String, Tensor>,
    /// Die Matrizen in int8, wenn mit [`Sprecher::laden`] gelesen.
    ganz: BTreeMap<String, Ganzmatrix>,
    /// **Die Texteinbettung bleibt auf der Platte**, Zeile fuer Zeile
    /// gelesen ([`Sprecher::text`]). ⚑ Sie ist mit 311 M Werten der groesste
    /// Teil des Modells (1,24 GB in `f32`), und ein Satz braucht davon ein
    /// paar Dutzend Zeilen zu je 4 KB.
    texteinbettung: (std::path::PathBuf, Eintrag),
    sprecher: Decoder,
    vorhersage: Decoder,
    z: Zeichen,
}

fn zahl(o: &BTreeMap<String, Json>, k: &str) -> Result<f64, String> {
    match o.get(k) {
        Some(Json::Zahl(z)) => Ok(*z),
        _ => Err(format!("config.json: {k} fehlt")),
    }
}

/// Wird diese Matrix in int8 gerechnet? Alle Projektionen und Koepfe; die
/// Einbettungen und Normen bleiben.
fn in_int8(name: &str) -> bool {
    name.ends_with("_proj.weight") || name.contains("lm_head.") || name == "codec_head.weight" || (name.contains("linear_fc") && name.ends_with(".weight"))
}

impl Sprecher {
    /// Liest den Sprecher, die Matrizen in int8 ([`in_int8`]).
    pub fn laden(datei: &Path) -> Result<Self, String> {
        Self::laden_mit(datei, true)
    }

    /// Liest den Sprecher ganz in `f32`.
    pub fn laden_genau(datei: &Path) -> Result<Self, String> {
        Self::laden_mit(datei, false)
    }

    fn laden_mit(datei: &Path, ganzzahlig: bool) -> Result<Self, String> {
        let ordner = datei.parent().unwrap_or(Path::new("."));
        let roh = std::fs::read(ordner.join("config.json")).map_err(|f| format!("config.json: {f}"))?;
        let Json::Objekt(o) = json(&roh, &mut 0)? else { return Err("config.json ist kein Objekt".into()) };
        let Some(Json::Objekt(t)) = o.get("talker_config") else { return Err("talker_config fehlt".into()) };
        let Some(Json::Objekt(p)) = t.get("code_predictor_config") else { return Err("code_predictor_config fehlt".into()) };
        let decoder = |c: &BTreeMap<String, Json>, praefix: &str| -> Result<Decoder, String> {
            Ok(Decoder {
                praefix: praefix.into(),
                ebenen: zahl(c, "num_hidden_layers")? as usize,
                koepfe: zahl(c, "num_attention_heads")? as usize,
                kv_koepfe: zahl(c, "num_key_value_heads")? as usize,
                kopfbreite: zahl(c, "head_dim")? as usize,
                rope_basis: zahl(c, "rope_theta")?,
                eps: zahl(c, "rms_norm_eps")? as f32,
            })
        };
        let sprachen = match t.get("codec_language_id") {
            Some(Json::Objekt(s)) => s.iter().filter_map(|(k, v)| if let Json::Zahl(z) = v { Some((k.clone(), *z as u32)) } else { None }).collect(),
            _ => return Err("codec_language_id fehlt".into()),
        };
        let id = |c: &BTreeMap<String, Json>, k: &str| zahl(c, k).map(|v| v as u32);
        let z = Zeichen {
            codec_bos: id(t, "codec_bos_id")?,
            codec_eos: id(t, "codec_eos_token_id")?,
            codec_pad: id(t, "codec_pad_id")?,
            denken: id(t, "codec_think_id")?,
            denken_anfang: id(t, "codec_think_bos_id")?,
            denken_ende: id(t, "codec_think_eos_id")?,
            nicht_denken: id(t, "codec_nothink_id")?,
            sprachen,
            tts_bos: id(&o, "tts_bos_token_id")?,
            tts_eos: id(&o, "tts_eos_token_id")?,
            tts_pad: id(&o, "tts_pad_token_id")?,
            woerter: zahl(t, "vocab_size")? as usize,
        };
        if zahl(t, "num_code_groups")? as usize != CODES {
            return Err("dieser Weg rechnet 16 Codes je Bild".into());
        }
        let mut eintraege = kopf_lesen(datei, |n| n.strip_prefix("talker.").map(str::to_string))?;
        let Some(stelle) = eintraege.iter().position(|e| e.name == "model.text_embedding.weight") else {
            return Err("model.text_embedding.weight fehlt".into());
        };
        let texteinbettung = (datei.to_path_buf(), eintraege.remove(stelle));
        // Gleich beim Lesen gewandelt: Der ganze Satz liegt nie in `f32` da.
        let (mut tensoren, mut ganz) = (BTreeMap::new(), BTreeMap::new());
        for teil in alle_kerne(eintraege, |e| {
            tensor_lesen(datei, &e).map(|t| if ganzzahlig && in_int8(&e.name) { (e.name, Err(Ganzmatrix::eine_stelle(&t.werte, t.form[0]))) } else { (e.name, Ok(t)) })
        }) {
            match teil? {
                (n, Ok(t)) => {
                    tensoren.insert(n, t);
                }
                (n, Err(m)) => {
                    ganz.insert(n, m);
                }
            }
        }
        Ok(Self { t: tensoren, ganz, texteinbettung, sprecher: decoder(t, "model")?, vorhersage: decoder(p, "code_predictor.model")?, z })
    }

    fn w(&self, name: &str) -> &[f32] {
        &self.t.get(name).unwrap_or_else(|| panic!("Gewicht {name} fehlt")).werte
    }

    fn zeilen(&self, name: &str) -> usize {
        match self.ganz.get(name) {
            Some(m) => m.aus,
            None => self.t.get(name).unwrap_or_else(|| panic!("Gewicht {name} fehlt")).form[0],
        }
    }

    /// `y = W · x` fuer die Matrix `name`, in int8 oder in `f32`.
    fn lin(&self, x: &[f32], zeilen: usize, name: &str) -> Vec<f32> {
        match self.ganz.get(name) {
            Some(m) => linear_ganz(x, zeilen, m),
            None => linear_genau(x, zeilen, self.w(name), self.zeilen(name)),
        }
    }

    /// Das Sprachtoken der Codespur, etwa fuer `german`.
    pub fn sprache(&self, name: &str) -> Option<u32> {
        self.z.sprachen.get(name).copied()
    }

    fn breite(&self) -> usize {
        self.w("model.norm.weight").len()
    }

    /// Eine Zeile einer Einbettung.
    fn einbettung(&self, name: &str, id: u32) -> &[f32] {
        let b = self.w(name).len() / self.zeilen(name);
        &self.w(name)[id as usize * b..][..b]
    }

    /// Textspur: `fc2(silu(fc1(e) + b1)) + b2` fuer die Token `ids`.
    fn text(&self, ids: &[u32]) -> Vec<f32> {
        let (datei, eintrag) = &self.texteinbettung;
        let e: Vec<f32> = ids.iter().flat_map(|&i| zeile_lesen(datei, eintrag, i as usize).unwrap_or_else(|f| panic!("Texteinbettung: {f}"))).collect();
        let mut h = linear_b(&e, ids.len(), self, "text_projection.linear_fc1");
        h.iter_mut().for_each(|v| *v /= 1.0 + (-*v).exp());
        linear_b(&h, ids.len(), self, "text_projection.linear_fc2")
    }

    /// Codespur: die Einbettung eines Codes des ersten Codebuchs.
    fn code(&self, id: u32) -> Vec<f32> {
        self.einbettung("model.codec_embedding.weight", id).to_vec()
    }
}

fn linear_b(x: &[f32], zeilen: usize, s: &Sprecher, name: &str) -> Vec<f32> {
    let aus = s.zeilen(&format!("{name}.weight"));
    let mut y = s.lin(x, zeilen, &format!("{name}.weight"));
    if let Some(b) = s.t.get(&format!("{name}.bias")) {
        y.chunks_mut(aus).for_each(|r| r.iter_mut().zip(&b.werte).for_each(|(v, c)| *v += c));
    }
    y
}

fn rmsnorm(x: &mut [f32], g: &[f32], eps: f32) {
    for z in x.chunks_mut(g.len()) {
        let q = z.iter().map(|v| v * v).sum::<f32>() / g.len() as f32;
        let n = 1.0 / (q + eps).sqrt();
        z.iter_mut().zip(g).for_each(|(v, w)| *v = w * (*v * n));
    }
}

/// RoPE wie `rotate_half` an den Stellen `ab..`.
fn drehen(x: &mut [f32], koepfe: usize, kb: usize, basis: f64, ab: usize) {
    let halb = kb / 2;
    for (t, zeile) in x.chunks_mut(koepfe * kb).enumerate() {
        let pos = (ab + t) as f32;
        for k in zeile.chunks_mut(kb) {
            for i in 0..halb {
                let inv = (1.0 / basis.powf(2.0 * i as f64 / kb as f64)) as f32;
                let f = pos * inv;
                let (c, s) = (f.cos(), f.sin());
                let (a, b) = (k[i], k[i + halb]);
                k[i] = a * c - b * s;
                k[i + halb] = b * c + a * s;
            }
        }
    }
}

/// Schluessel und Werte je Ebene, fuer die Stellen, die schon gelesen sind.
#[derive(Default)]
struct Speicher {
    kv: Vec<(Vec<f32>, Vec<f32>)>,
    gelesen: usize,
}

/// **`n` neue Stellen durch einen Decoder**; gibt die Zustaende nach der
/// letzten Norm zurueck, zeilenweise.
fn vorwaerts(s: &Sprecher, d: &Decoder, mut x: Vec<f32>, sp: &mut Speicher) -> Vec<f32> {
    let breite = s.breite();
    let n = x.len() / breite;
    if sp.kv.is_empty() {
        sp.kv = vec![(Vec::new(), Vec::new()); d.ebenen];
    }
    let (kp, kv, kb) = (d.koepfe, d.kv_koepfe, d.kopfbreite);
    let gruppe = kp / kv;
    let skala = 1.0 / (kb as f32).sqrt();
    let v0 = sp.gelesen;
    for (nr, (ks, vs)) in sp.kv.iter_mut().enumerate() {
        let p = format!("{}.layers.{nr}", d.praefix);
        let mut a = x.clone();
        rmsnorm(&mut a, s.w(&format!("{p}.input_layernorm.weight")), d.eps);
        let mut q = s.lin(&a, n, &format!("{p}.self_attn.q_proj.weight"));
        let mut k = s.lin(&a, n, &format!("{p}.self_attn.k_proj.weight"));
        let v = s.lin(&a, n, &format!("{p}.self_attn.v_proj.weight"));
        rmsnorm(&mut q, s.w(&format!("{p}.self_attn.q_norm.weight")), d.eps);
        rmsnorm(&mut k, s.w(&format!("{p}.self_attn.k_norm.weight")), d.eps);
        drehen(&mut q, kp, kb, d.rope_basis, v0);
        drehen(&mut k, kv, kb, d.rope_basis, v0);
        ks.extend_from_slice(&k);
        vs.extend_from_slice(&v);
        let alle = v0 + n;
        let mut o = vec![0f32; n * kp * kb];
        let mut w = vec![0f32; alle];
        for i in 0..n {
            let bis = v0 + i + 1;
            for h in 0..kp {
                let g = h / gruppe;
                let qi = &q[(i * kp + h) * kb..][..kb];
                let mut hoechst = f32::NEG_INFINITY;
                for j in 0..bis {
                    w[j] = skalar(qi, &ks[(j * kv + g) * kb..][..kb]) * skala;
                    hoechst = hoechst.max(w[j]);
                }
                let mut summe = 0f32;
                for wj in &mut w[..bis] {
                    *wj = (*wj - hoechst).exp();
                    summe += *wj;
                }
                let ziel = &mut o[(i * kp + h) * kb..][..kb];
                for j in 0..bis {
                    let f = w[j] / summe;
                    for (z, vv) in ziel.iter_mut().zip(&vs[(j * kv + g) * kb..][..kb]) {
                        *z += f * vv;
                    }
                }
            }
        }
        let o = s.lin(&o, n, &format!("{p}.self_attn.o_proj.weight"));
        x.iter_mut().zip(&o).for_each(|(a, b)| *a += b);
        let mut a = x.clone();
        rmsnorm(&mut a, s.w(&format!("{p}.post_attention_layernorm.weight")), d.eps);
        let gate = s.lin(&a, n, &format!("{p}.mlp.gate_proj.weight"));
        let up = s.lin(&a, n, &format!("{p}.mlp.up_proj.weight"));
        let h: Vec<f32> = gate.iter().zip(&up).map(|(g, u)| g / (1.0 + (-g).exp()) * u).collect();
        let m = s.lin(&h, n, &format!("{p}.mlp.down_proj.weight"));
        x.iter_mut().zip(&m).for_each(|(a, b)| *a += b);
    }
    sp.gelesen += n;
    rmsnorm(&mut x, s.w(&format!("{}.norm.weight", d.praefix)), d.eps);
    x
}

fn argmax(l: &[f32]) -> u32 {
    l.iter().enumerate().fold((0usize, f32::NEG_INFINITY), |b, (i, &v)| if v > b.1 { (i, v) } else { b }).0 as u32
}

/// **Wie gewaehlt wird.**
///
/// ⚑ **Im Betrieb mit Zufall, wie die Vorgabe der Vorlage**
/// (`generation_config.json`: top-k 50, Temperatur 0,9, fuer Sprecher und
/// Code-Vorhersage). Gierig findet das Modell nicht immer ein Ende (siehe
/// [`bildgrenze`]); gierig bleibt fuer die Proben gegen die Referenz.
#[derive(Debug, Clone, Copy)]
pub enum Wahl {
    Gierig,
    /// Stichprobe mit diesem Samen; derselbe Samen ergibt auf derselben
    /// Maschine dieselben Codes.
    Zufall(u64),
}

/// **Ein frischer Samen**: Uhr und Zaehler durch `splitmix64`.
pub fn frischer_samen() -> u64 {
    static ZAEHLER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    let z = ZAEHLER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    integer_llm_kernels::prng::splitmix64(n ^ z.rotate_left(32)).1
}

/// Wie oft [`sprechen_frei`] hoechstens neu zieht, wenn ein Lauf die
/// [`bildgrenze`] erreicht.
pub const NEUVERSUCHE: usize = 2;

/// **Text zu Codes im Betrieb**: je Aufruf ein frischer Samen, und wer die
/// [`bildgrenze`] erreicht, zieht neu (hoechstens [`NEUVERSUCHE`] Mal).
/// Gibt den Samen des Laufs mit, der zaehlt; mit ihm und
/// [`Wahl::Zufall`] laesst sich der Lauf auf demselben Rechenweg genau
/// nachrechnen.
///
/// ⚑ **Jedes Mal neu, der Samen wird mitgeschrieben** (Festlegung des
/// Projektinhabers, 2026-11-11): natuerliche Abwechslung bei gleichem Text,
/// nachrechenbar ueber den Samen. ⚠️ Bitgleich nur auf demselben Rechenweg:
/// Metal und CPU, NEON und AVX2 runden in `f32` verschieden, und eine
/// Stichprobe kann dann bei gleichem Samen anders ziehen.
pub fn sprechen_frei(s: &Sprecher, ids: &[u32], sprache: Option<u32>, stimme: Option<&Stimme>) -> (Vec<[u16; CODES]>, Spur, u64) {
    let grenze = bildgrenze(ids);
    let mut versuch = 0;
    loop {
        let samen = frischer_samen();
        let (codes, spur) = sprechen(s, ids, sprache, stimme, grenze, Wahl::Zufall(samen));
        if codes.len() < grenze || versuch == NEUVERSUCHE {
            return (codes, spur, samen);
        }
        versuch += 1;
    }
}

/// Aus wie vielen der wahrscheinlichsten Codes gezogen wird.
const TOP_K: usize = 50;
/// Die Temperatur der Stichprobe.
const TEMPERATUR: f32 = 0.9;

/// Der Zustand der Wahl: gierig oder ein Zufallsstrom (`splitmix64` der
/// Kernkiste).
struct Waehler(Option<u64>);

impl Waehler {
    fn neu(w: Wahl) -> Self {
        Self(match w {
            Wahl::Gierig => None,
            Wahl::Zufall(samen) => Some(samen),
        })
    }

    /// Waehlt aus den Logits: gierig das groesste, sonst aus den `TOP_K`
    /// groessten nach `softmax(l / TEMPERATUR)`.
    fn waehlen(&mut self, l: &[f32]) -> u32 {
        let Some(zustand) = &mut self.0 else { return argmax(l) };
        let mut beste: Vec<(f32, u32)> = l.iter().enumerate().filter(|(_, v)| v.is_finite()).map(|(i, &v)| (v, i as u32)).collect();
        if beste.is_empty() {
            return argmax(l);
        }
        let k = TOP_K.min(beste.len());
        beste.select_nth_unstable_by(k - 1, |a, b| b.0.total_cmp(&a.0));
        beste.truncate(k);
        beste.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let hoechst = beste[0].0;
        let gewichte: Vec<f64> = beste.iter().map(|(v, _)| f64::from((v - hoechst) / TEMPERATUR).exp()).collect();
        let (neu, z) = integer_llm_kernels::prng::splitmix64(*zustand);
        *zustand = neu;
        let mut ziel = (z >> 11) as f64 / (1u64 << 53) as f64 * gewichte.iter().sum::<f64>();
        for (g, (_, i)) in gewichte.iter().zip(&beste) {
            ziel -= g;
            if ziel < 0.0 {
                return *i;
            }
        }
        beste[k - 1].1
    }
}

/// Die uebrigen fuenfzehn Codes eines Bildes, gierig: Vorlauf aus dem
/// letzten Zustand des Sprechers und dem ersten Code, dann je Code ein
/// Schritt mit eigener Einbettung und eigenem Kopf.
fn ergaenzen(s: &Sprecher, zustand: &[f32], erster: u32, w: &mut Waehler) -> [u16; CODES] {
    let mut bild = [0u16; CODES];
    bild[0] = erster as u16;
    let mut sp = Speicher::default();
    let mut x = zustand.to_vec();
    x.extend(s.code(erster));
    let mut h = vorwaerts(s, &s.vorhersage, x, &mut sp);
    let b = s.breite();
    for k in 0..CODES - 1 {
        let letzte = &h[h.len() - b..];
        let l = s.lin(letzte, 1, &format!("code_predictor.lm_head.{k}.weight"));
        let c = w.waehlen(&l);
        bild[k + 1] = c as u16;
        if k + 1 < CODES - 1 {
            let e = s.einbettung(&format!("code_predictor.model.codec_embedding.{k}.weight"), c).to_vec();
            h = vorwaerts(s, &s.vorhersage, e, &mut sp);
        }
    }
    bild
}

/// Die Summe der Einbettungen aller sechzehn Codes eines Bildes.
fn bildsumme(s: &Sprecher, bild: &[u16; CODES]) -> Vec<f32> {
    let mut x = s.code(u32::from(bild[0]));
    for k in 0..CODES - 1 {
        let e = s.einbettung(&format!("code_predictor.model.codec_embedding.{k}.weight"), u32::from(bild[k + 1]));
        x.iter_mut().zip(e).for_each(|(a, b)| *a += b);
    }
    x
}

/// Was die Probe neben den Codes vergleicht.
pub struct Spur {
    pub vorlauf_laenge: usize,
    pub vorlauf_letzter: Vec<f32>,
    /// Rechenzeit im Sprecher (Vorlauf und je Bild ein Schritt).
    pub dauer_sprecher: std::time::Duration,
    /// Rechenzeit in der Code-Vorhersage (je Bild fuenfzehn Schritte).
    pub dauer_vorhersage: std::time::Duration,
}

/// **Wie viele Bilder ein Text hoechstens bekommt**: sechs je Token der
/// Vorlage und 25 dazu.
///
/// ⚑ Gemessen spricht das Modell rund 2,3 Bilder je Token (15 Token, 35
/// Bilder); die Grenze laesst das Zweieinhalbfache. 📌 **Gierig findet das
/// Modell nicht immer ein Ende**: Ein Satz lief in `f32` bis zur alten
/// Grenze von 2000 Bildern und hielt dabei 4,6 GB. Wer die Grenze erreicht,
/// hat keinen Satz gesprochen, sondern eine Schleife.
pub fn bildgrenze(ids: &[u32]) -> usize {
    25 + 6 * ids.len()
}

/// **Die Stimme**, mit der gesprochen wird.
pub struct Stimme {
    /// Der Abdruck der Stimmprobe ([`crate::stimmabdruck`]).
    pub abdruck: Vec<f32>,
    /// Zum Klonen mit Mitschrift: die Mitschrift der Stimmprobe als
    /// Qwen-Token ohne Vorlage ([`crate::textzerleger::Textzerleger::zerlegen`])
    /// und die Codes der Stimmprobe ([`crate::tonzerleger::hin`]). Ohne nur
    /// mit dem Abdruck.
    pub vorbild: Option<(Vec<u32>, Vec<[u16; CODES]>)>,
}

/// **Text zu Codes**. `ids` ist der ganze Text in der Vorlage
/// `<|im_start|>assistant\n{text}<|im_end|>\n<|im_start|>assistant\n` als
/// Qwen-Token ([`crate::textzerleger::Textzerleger::vorlage`]); `sprache`
/// das Sprachtoken ([`Sprecher::sprache`]), ohne erkennt das Modell die
/// Sprache am Text (`auto` der Vorlage); ohne `stimme` spricht das Modell
/// mit seiner eigenen. Hoechstens `bilder` Bilder ([`bildgrenze`]),
/// gewaehlt nach `wahl`.
///
/// ⚑ **Mit Vorbild beginnt der Ton hinter der Stimmprobe**: Die Codes
/// setzen die der Stimmprobe fort und werden mit ihnen zusammen zu Ton
/// ([`crate::tonzerleger::zurueck_hinter`]).
pub fn sprechen(s: &Sprecher, ids: &[u32], sprache: Option<u32>, stimme: Option<&Stimme>, bilder: usize, wahl: Wahl) -> (Vec<[u16; CODES]>, Spur) {
    sprechen_mit(s, ids, sprache, stimme, bilder, wahl, &mut |_| ())
}

/// **Wie [`sprechen`], und `je_bild` bekommt jedes Bild, sobald es gewaehlt
/// ist**, fuer das Streaming ([`crate::tonzerleger::Rueckstrom`]).
pub fn sprechen_mit(
    s: &Sprecher,
    ids: &[u32],
    sprache: Option<u32>,
    stimme: Option<&Stimme>,
    bilder: usize,
    wahl: Wahl,
    je_bild: &mut dyn FnMut(&[u16; CODES]),
) -> (Vec<[u16; CODES]>, Spur) {
    let mut w = Waehler::neu(wahl);
    let z = &s.z;
    assert!(ids.len() > 8, "zu wenige Token fuer die Vorlage");
    let b = s.breite();
    let fuellung = s.text(&[z.tts_bos, z.tts_eos, z.tts_pad]);
    let (tts_bos, tts_eos, tts_pad) = (&fuellung[..b], &fuellung[b..2 * b], &fuellung[2 * b..]);
    // Codespur des Vorlaufs: Denken, Denkbeginn, Sprache, Denkende (ohne
    // Sprache: nicht denken, Denkbeginn, Denkende), Abdruck (wenn einer da
    // ist), Fuellung, Beginn.
    let kopf: Vec<u32> = match sprache {
        Some(sp) => vec![z.denken, z.denken_anfang, sp, z.denken_ende],
        None => vec![z.nicht_denken, z.denken_anfang, z.denken_ende],
    };
    let n_kopf = kopf.len();
    let mut codespur: Vec<Vec<f32>> = kopf.into_iter().chain([z.codec_pad, z.codec_bos]).map(|c| s.code(c)).collect();
    if let Some(st) = stimme {
        assert_eq!(st.abdruck.len(), b, "der Abdruck hat nicht die Breite des Sprechers");
        codespur.insert(n_kopf, st.abdruck.clone());
    }
    let mut x = s.text(&ids[..3]);
    for (i, c) in codespur[..codespur.len() - 1].iter().enumerate() {
        let t = if i + 1 < codespur.len() - 1 { tts_pad } else { tts_bos };
        x.extend(t.iter().zip(c).map(|(a, b)| a + b));
    }
    let beginn = codespur.last().expect("Beginn");
    let nachlauf = match stimme.and_then(|st| st.vorbild.as_ref()) {
        None => {
            let erstes = s.text(&ids[3..4]);
            x.extend(erstes.iter().zip(beginn).map(|(a, b)| a + b));
            // Restlicher Text, dann das Textende; danach Fuellung.
            let mut nachlauf = s.text(&ids[4..ids.len() - 5]);
            nachlauf.extend_from_slice(tts_eos);
            nachlauf
        }
        Some((mitschrift, vorbild)) => {
            // Textspur: Mitschrift, neuer Text, Textende. Codespur: Beginn,
            // dann die Bilder der Stimmprobe. Stelle fuer Stelle addiert; ist
            // der Text laenger, kommt der Rest im Nachlauf, sonst Fuellung.
            let mut text = s.text(&[mitschrift.as_slice(), &ids[3..ids.len() - 5]].concat());
            text.extend_from_slice(tts_eos);
            let mut code = beginn.clone();
            for bild in vorbild {
                code.extend(bildsumme(s, bild));
            }
            while text.len() < code.len() {
                text.extend_from_slice(tts_pad);
            }
            x.extend(text.iter().zip(&code).map(|(a, b)| a + b));
            text[code.len()..].to_vec()
        }
    };
    let nachlauf_n = nachlauf.len() / b;

    let mut sp = Speicher::default();
    let vorlauf_laenge = x.len() / b;
    let uhr = std::time::Instant::now();
    let mut h = vorwaerts(s, &s.sprecher, x, &mut sp);
    let mut spur = Spur { vorlauf_laenge, vorlauf_letzter: h[h.len() - b..].to_vec(), dauer_sprecher: uhr.elapsed(), dauer_vorhersage: std::time::Duration::ZERO };
    let mut gewaehlt: Vec<u32> = Vec::new();
    let mut codes = Vec::new();
    for schritt in 0..bilder {
        let letzte = h[h.len() - b..].to_vec();
        let mut l = s.lin(&letzte, 1, "codec_head.weight");
        // Wiederholungsstrafe 1,05 auf jeden schon gewaehlten Code.
        let mut gesehen = gewaehlt.clone();
        gesehen.sort_unstable();
        gesehen.dedup();
        for &g in &gesehen {
            let v = &mut l[g as usize];
            *v = if *v < 0.0 { *v * 1.05 } else { *v / 1.05 };
        }
        // Gesperrt: die oberen 1024 Codes ausser dem Ende; das Ende erst ab dem dritten Bild.
        for (i, v) in l.iter_mut().enumerate().skip(z.woerter - 1024) {
            if i as u32 != z.codec_eos {
                *v = f32::NEG_INFINITY;
            }
        }
        if schritt < 2 {
            l[z.codec_eos as usize] = f32::NEG_INFINITY;
        }
        let erster = w.waehlen(&l);
        if erster == z.codec_eos {
            break;
        }
        gewaehlt.push(erster);
        let uhr = std::time::Instant::now();
        let bild = ergaenzen(s, &letzte, erster, &mut w);
        spur.dauer_vorhersage += uhr.elapsed();
        codes.push(bild);
        je_bild(&bild);
        let mut e = bildsumme(s, &bild);
        let t = if schritt < nachlauf_n { &nachlauf[schritt * b..][..b] } else { tts_pad };
        e.iter_mut().zip(t).for_each(|(a, c)| *a += c);
        let uhr = std::time::Instant::now();
        h = vorwaerts(s, &s.sprecher, e, &mut sp);
        spur.dauer_sprecher += uhr.elapsed();
    }
    (codes, spur)
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ Gierig ist das groesste; mit Zufall nur einer der 50 groessten, und
    /// derselbe Samen waehlt dasselbe. Gegenprobe: ein anderer Samen waehlt
    /// in 200 Zuegen nicht immer dasselbe.
    #[test]
    fn die_wahl_bleibt_unter_den_besten_und_folgt_dem_samen() {
        let l: Vec<f32> = (0..300).map(|i| ((i * 37) % 300) as f32 / 30.0).collect();
        assert_eq!(Waehler::neu(Wahl::Gierig).waehlen(&l), argmax(&l));
        let mut grenze = l.clone();
        grenze.sort_by(|a, b| b.total_cmp(a));
        let schwelle = grenze[TOP_K - 1];
        let ziehen = |samen| {
            let mut w = Waehler::neu(Wahl::Zufall(samen));
            (0..200).map(|_| w.waehlen(&l)).collect::<Vec<u32>>()
        };
        let a = ziehen(1);
        assert!(a.iter().all(|&i| l[i as usize] >= schwelle), "gezogen ausserhalb der besten {TOP_K}");
        assert_eq!(a, ziehen(1), "derselbe Samen, andere Wahl");
        assert_ne!(a, ziehen(2), "ein anderer Samen aendert nichts");
        // Gesperrte Codes (−∞) werden nie gezogen.
        let mut gesperrt = l.clone();
        gesperrt.iter_mut().skip(10).for_each(|v| *v = f32::NEG_INFINITY);
        let mut w = Waehler::neu(Wahl::Zufall(3));
        assert!((0..200).all(|_| w.waehlen(&gesperrt) < 10));
    }

    /// Zwei frische Samen hintereinander sind verschieden.
    #[test]
    fn frische_samen_unterscheiden_sich() {
        assert_ne!(frischer_samen(), frischer_samen());
    }
}

