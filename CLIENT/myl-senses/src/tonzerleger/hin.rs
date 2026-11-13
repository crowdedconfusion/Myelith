//! **Der Hinweg des Tonzerlegers**: aus 24 kHz werden Codes, fuer das
//! Klonen mit Mitschrift (CLIENT 8.5).
//!
//! Ein Kodierer wie Mimi (Defossez u. a. 2024), Vorlage `MimiModel.encode`
//! aus `transformers` und `Qwen3TTSTokenizerV2Model.encode` aus dem Paket
//! `qwen-tts` (beide Apache 2.0), neu geschrieben:
//!
//! 1. SEANet: Faltung auf 64, viermal Restblock, ELU und Faltung mit
//!    Schritt 4, 5, 6, 8 bei doppelter Breite, ELU, Faltung auf 512. Alle
//!    Faltungen kausal mit Nullrand links.
//! 2. Acht Transformerebenen bei 25 Hz: LayerNorm, RoPE, kausale
//!    Aufmerksamkeit, GELU-MLP, LayerScale.
//! 3. Verdichten auf 12,5 Hz (Faltung mit Schritt 2, Rand durch Wiederholen).
//! 4. Restquantisierung: ein semantischer Code, dann fuenfzehn akustische,
//!    beide Gruppen aus derselben Verdichtung, je Gruppe eine Projektion auf
//!    256 und je Code der naechste Eintrag des Codebuchs.
//!
//! ⚠️ **Die Vorlage nimmt das Fenster aus `sliding_window` nicht**: Die
//! Maske ist voll kausal, und so rechnet auch dieser Weg.

use std::collections::BTreeMap;
use std::path::Path;

use super::{aufmerksamkeit, drehen, layernorm, Signal, CODES, FALTSTUECK};
use crate::whisper::{alle_kerne, gelu, json, kopf_lesen, linear_genau, tensor_lesen, Json, Tensor};

/// Die Masse aus `encoder_config` in `config.json`.
#[derive(Debug, Clone)]
struct Masse {
    koepfe: usize,
    kopfbreite: usize,
    rope_basis: f64,
    eps: f64,
    /// Die Schritte der SEANet-Faltungen, in Rechenreihenfolge.
    schritte: Vec<usize>,
    semantisch: usize,
}

/// **Die Gewichte des Hinwegs**, `encoder.…` aus
/// `speech_tokenizer/model.safetensors`.
pub struct Hinweg {
    t: BTreeMap<String, Tensor>,
    m: Masse,
}

impl Hinweg {
    /// Liest den Hinweg und die Masse aus `config.json` daneben.
    pub fn laden(datei: &Path) -> Result<Self, String> {
        let ordner = datei.parent().unwrap_or(Path::new("."));
        let roh = std::fs::read(ordner.join("config.json")).map_err(|f| format!("config.json: {f}"))?;
        let Json::Objekt(o) = json(&roh, &mut 0)? else { return Err("config.json ist kein Objekt".into()) };
        let Some(Json::Objekt(e)) = o.get("encoder_config") else { return Err("config.json: encoder_config fehlt".into()) };
        let zahl = |k: &str| match e.get(k) {
            Some(Json::Zahl(z)) => Ok(*z),
            _ => Err(format!("encoder_config: {k} fehlt")),
        };
        let mut schritte = match e.get("upsampling_ratios") {
            Some(Json::Liste(l)) => l.iter().filter_map(|j| if let Json::Zahl(z) = j { Some(*z as usize) } else { None }).collect::<Vec<_>>(),
            _ => return Err("encoder_config: upsampling_ratios fehlt".into()),
        };
        schritte.reverse();
        let m = Masse {
            koepfe: zahl("num_attention_heads")? as usize,
            kopfbreite: zahl("head_dim")? as usize,
            rope_basis: zahl("rope_theta")?,
            eps: zahl("norm_eps")?,
            schritte,
            semantisch: zahl("num_semantic_quantizers")? as usize,
        };
        let gueltig = match o.get("encoder_valid_num_quantizers") {
            Some(Json::Zahl(z)) => *z as usize,
            _ => return Err("config.json: encoder_valid_num_quantizers fehlt".into()),
        };
        if zahl("num_key_value_heads")? as usize != m.koepfe
            || gueltig != CODES
            || zahl("num_residual_layers")? as usize != 1
            || zahl("compress")? as usize != 2
        {
            return Err("dieser Weg rechnet einen Restblock je Stufe, ohne Abkuerzung, und 16 Codes".into());
        }
        let eintraege = kopf_lesen(datei, |n| n.strip_prefix("encoder.").map(str::to_string))?;
        let t = alle_kerne(eintraege, |e| tensor_lesen(datei, &e).map(|t| (e.name, t)))
            .into_iter()
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        if t.keys().any(|k| k.contains(".shortcut.")) {
            return Err("dieser Weg rechnet die Restbloecke ohne Abkuerzung".into());
        }
        Ok(Self { t, m })
    }

    fn w(&self, name: &str) -> &Tensor {
        self.t.get(name).unwrap_or_else(|| panic!("Gewicht {name} fehlt"))
    }

    /// **Kausale Faltung wie `MimiConv1d`**: links `K − s` Rand, rechts so
    /// viel, dass `⌈L / s⌉` Bilder herauskommen; Rand aus Nullen oder, mit
    /// `wiederholen`, aus dem Randwert.
    fn faltung(&self, x: &Signal, name: &str, schritt: usize, dilatation: usize, wiederholen: bool) -> Signal {
        let w = self.w(&format!("{name}.weight"));
        let (aus, ein, k) = (w.form[0], w.form[1], w.form[2]);
        assert_eq!(ein, x.kanaele, "{name}: {} Kanaele statt {ein}", x.kanaele);
        let b = self.t.get(&format!("{name}.bias")).map(|t| &t.werte);
        let rand = ((k - 1) * dilatation + 1 - schritt) as isize;
        let laenge = x.laenge.div_ceil(schritt);
        let l = x.laenge as isize;
        let mut y = Signal::null(aus, laenge);
        for anfang in (0..laenge).step_by(FALTSTUECK) {
            let n = FALTSTUECK.min(laenge - anfang);
            let mut spalten = vec![0f32; n * ein * k];
            for r in 0..n {
                for c in 0..ein {
                    let zeile = &x.werte[c * x.laenge..][..x.laenge];
                    for j in 0..k {
                        let q = ((anfang + r) * schritt + j * dilatation) as isize - rand;
                        spalten[(r * ein + c) * k + j] = if (0..l).contains(&q) {
                            zeile[q as usize]
                        } else if wiederholen {
                            zeile[q.clamp(0, l - 1) as usize]
                        } else {
                            0.0
                        };
                    }
                }
            }
            let teil = linear_genau(&spalten, n, &w.werte, aus);
            for r in 0..n {
                for o in 0..aus {
                    y.werte[o * laenge + anfang + r] = teil[r * aus + o] + b.map_or(0.0, |b| b[o]);
                }
            }
        }
        y
    }

    fn linear(&self, x: &[f32], zeilen: usize, name: &str) -> Vec<f32> {
        let w = self.w(&format!("{name}.weight"));
        linear_genau(x, zeilen, &w.werte, w.form[0])
    }

    /// Eine Transformerebene, `x` zeilenweise `[t][512]`.
    fn ebene(&self, x: &mut [f32], nr: usize) {
        let p = format!("encoder_transformer.layers.{nr}");
        let m = &self.m;
        let breite = self.w(&format!("{p}.input_layernorm.weight")).form[0];
        let n = x.len() / breite;
        let norm = |x: &[f32], name: &str| layernorm(x, &self.w(&format!("{p}.{name}.weight")).werte, &self.w(&format!("{p}.{name}.bias")).werte, m.eps);
        let a = norm(x, "input_layernorm");
        let mut q = self.linear(&a, n, &format!("{p}.self_attn.q_proj"));
        let mut k = self.linear(&a, n, &format!("{p}.self_attn.k_proj"));
        let v = self.linear(&a, n, &format!("{p}.self_attn.v_proj"));
        drehen(&mut q, m.koepfe, m.kopfbreite, m.rope_basis);
        drehen(&mut k, m.koepfe, m.kopfbreite, m.rope_basis);
        let o = aufmerksamkeit(&q, &k, &v, m.koepfe, m.kopfbreite, usize::MAX);
        let o = self.linear(&o, n, &format!("{p}.self_attn.o_proj"));
        let s = &self.w(&format!("{p}.self_attn_layer_scale.scale")).werte;
        x.chunks_mut(breite).zip(o.chunks(breite)).for_each(|(xz, oz)| xz.iter_mut().zip(oz).zip(s).for_each(|((v, d), c)| *v += c * d));
        let a = norm(x, "post_attention_layernorm");
        let mut h = self.linear(&a, n, &format!("{p}.mlp.fc1"));
        h.iter_mut().for_each(|v| *v = gelu(*v));
        let d = self.linear(&h, n, &format!("{p}.mlp.fc2"));
        let s = &self.w(&format!("{p}.mlp_layer_scale.scale")).werte;
        x.chunks_mut(breite).zip(d.chunks(breite)).for_each(|(xz, dz)| xz.iter_mut().zip(dz).zip(s).for_each(|((v, d), c)| *v += c * d));
    }

    /// Eine Gruppe der Restquantisierung: Projektion, dann je Stufe der
    /// naechste Eintrag und der Rest weiter.
    fn quantisieren(&self, x: &[f32], n: usize, gruppe: &str, stufen: usize, codes: &mut [[u16; CODES]], ab: usize) {
        let mut rest = self.linear(x, n, &format!("quantizer.{gruppe}.input_proj"));
        let dim = rest.len() / n;
        for s in 0..stufen {
            let p = format!("quantizer.{gruppe}.layers.{s}.codebook");
            let (summe, nutzung) = (&self.w(&format!("{p}.embed_sum")).werte, &self.w(&format!("{p}.cluster_usage")).werte);
            let buch: Vec<f32> = summe.chunks(dim).zip(nutzung).flat_map(|(z, u)| z.iter().map(move |v| v / u.max(1e-5))).collect();
            for t in 0..n {
                let r = &mut rest[t * dim..][..dim];
                let (mut bester, mut abstand) = (0usize, f32::INFINITY);
                for (i, e) in buch.chunks(dim).enumerate() {
                    let d: f32 = r.iter().zip(e).map(|(a, b)| (a - b) * (a - b)).sum();
                    if d < abstand {
                        (bester, abstand) = (i, d);
                    }
                }
                codes[t][ab + s] = bester as u16;
                r.iter_mut().zip(&buch[bester * dim..][..dim]).for_each(|(a, b)| *a -= b);
            }
        }
    }
}

/// **Ton zu Codes**: 24 kHz in `[-1, 1]`, je 1920 Proben ein Bild (das
/// letzte angebrochene zaehlt mit). `stufe` bekommt `seanet`,
/// `transformer` und `verdichtet`, fuer die Probe.
pub fn hin_mit(ton: &[f32], h: &Hinweg, mut stufe: impl FnMut(&str, &[f32])) -> Vec<[u16; CODES]> {
    let elu = |s: &mut Signal| s.werte.iter_mut().for_each(|v| *v = if *v > 0.0 { *v } else { v.exp_m1() });
    let mut s = Signal { kanaele: 1, laenge: ton.len(), werte: ton.to_vec() };
    s = h.faltung(&s, "encoder.layers.0.conv", 1, 1, false);
    let mut nr = 1;
    for &schritt in &h.m.schritte {
        let rest = s.clone();
        elu(&mut s);
        s = h.faltung(&s, &format!("encoder.layers.{nr}.block.1.conv"), 1, 1, false);
        elu(&mut s);
        s = h.faltung(&s, &format!("encoder.layers.{nr}.block.3.conv"), 1, 1, false);
        s.werte.iter_mut().zip(&rest.werte).for_each(|(a, b)| *a += b);
        elu(&mut s);
        s = h.faltung(&s, &format!("encoder.layers.{}.conv", nr + 2), schritt, 1, false);
        nr += 3;
    }
    elu(&mut s);
    let s = h.faltung(&s, &format!("encoder.layers.{}.conv", nr + 1), 1, 1, false);
    stufe("seanet", &s.werte);
    let mut x = s.quer();
    let ebenen = (0..).take_while(|i| h.t.contains_key(&format!("encoder_transformer.layers.{i}.input_layernorm.weight"))).count();
    for e in 0..ebenen {
        h.ebene(&mut x, e);
    }
    stufe("transformer", &x);
    let s = h.faltung(&Signal::aus_quer(&x, s.laenge, s.kanaele), "downsample.conv", 2, 1, true);
    stufe("verdichtet", &s.werte);
    // Je Bild so viele Proben, wie die Schritte zusammen und das Verdichten nehmen.
    let n = s.laenge.min(ton.len().div_ceil(h.m.schritte.iter().product::<usize>() * 2));
    let x = s.quer();
    let mut codes = vec![[0u16; CODES]; s.laenge];
    h.quantisieren(&x, s.laenge, "semantic_residual_vector_quantizer", h.m.semantisch, &mut codes, 0);
    h.quantisieren(&x, s.laenge, "acoustic_residual_vector_quantizer", CODES - h.m.semantisch, &mut codes, h.m.semantisch);
    codes.truncate(n);
    codes
}

/// **Ton zu Codes**, ohne Zwischenstufen.
pub fn hin(ton: &[f32], h: &Hinweg) -> Vec<[u16; CODES]> {
    hin_mit(ton, h, |_, _| ())
}
