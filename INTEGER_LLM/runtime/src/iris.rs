//! **Iris, der Sichtturm**, ganzzahlig: Flicken eines Bildes rein,
//! Bildtoken fuer den Decoder raus.
//!
//! ⚑ **Der Name** (Festlegung des Projektinhabers, 2026-10-23): Die Iris
//! bestimmt, was ins Auge hineinkommt; dieser Teil bestimmt, was vom Bild
//! beim Decoder ankommt. Gemeint ist immer derselbe Turm, wie ihn die
//! Vorlage `visual` nennt.
//!
//! Gleiche Bauart bei Qwen3.6-35B-A3B, Qwen3.8-27B und Qwen3.8-Flash-Next:
//! 27 Bloecke zu 1152 mit 16 Koepfen, ein Projektor, der je vier Flicken zu
//! einem Bildtoken der Decoderbreite macht. Die Masse kommen aus
//! `iris/iris.json` des Artefakts, die Gewichte aus dessen
//! `weights_manifest.json` im Format des Decoders.
//!
//! Ablauf je Bild (Flicken in der Folge von [`flickenfolge`]):
//! 1. Flickeneinbettung (768 -> 1152, Eingang `2p - 255` mit 8 Bruchstellen)
//!    plus Positions-Einbettung, bilinear aus dem 48er-Gitter.
//! 2. Je Block `x + attn(LN1(x))`, dann `x + mlp(LN2(x))`; Achtsamkeit ohne
//!    Maske ueber alle Flicken des Bildes, Drehung ueber Zeile und Spalte.
//! 3. Projektor: LN je Flicken, je vier hintereinander zu 4608, `fc1`, GELU
//!    (exakt), `fc2` auf die Eingangsskala der ersten Decoderebene.
//!
//! ⚑ **Die Tabellen fuer rsqrt und exp sind die des Decoders**, und dessen
//! Spezifikation gibt die Skalen der Softmax vor. Der Turm bringt nur mit,
//! was der Decoder nicht kennt: GELU in zwei Formen und die Drehtabelle.

use std::path::Path;

use integer_llm_kernels::fadenpool::{faeden, verteilen};
use integer_llm_kernels::fixed_point::{clamp_i16_from_i64, inv_sqrt_q15, rescale_i64};
use integer_llm_kernels::integer_math::gelu_nachschlagen;
use integer_llm_kernels::linear::linear_w8a16_pc_stapel;
use integer_llm_kernels::rmsnorm::inv_n_q20;
use integer_llm_kernels::rope::rotate_half_split_i16;
use integer_llm_kernels::iris::{achtsamkeit_spaet_normiert, drehzeile, flickenfolge, layernorm_i16, positions_einbettung};
use serde::Deserialize;

use crate::loader::{load_luts, load_model_dims, load_weights, BiasTensor, LoadedWeights};
use crate::model::{ModelConfig, QTensor};

/// Ordner des Teilartefakts im Artefakt des Decoders.
pub const ORDNER: &str = "iris";

/// Werte je Flicken am Eingang: 3 Kanaele mal 16 mal 16.
pub const FLICKENWERTE: usize = 3 * 16 * 16;

/// Skalen eines Blocks, wie die Kalibrierung sie festlegt.
#[derive(Debug, Clone, Deserialize)]
pub struct Blockskalen {
    pub rest_ein: Vec<u8>,
    pub n1: u8,
    pub q: u8,
    pub k: u8,
    pub v: u8,
    /// Ausgang des Achtsamkeitszweigs, je Kanal (siehe [`restsumme`]).
    pub attn: Vec<u8>,
    pub rest_mitte: Vec<u8>,
    /// Ausgang des MLP-Zweigs, je Kanal.
    pub mlp: Vec<u8>,
    pub n2: u8,
    /// Je Kanal: Wenige grosse Kanaele verdarben sonst allen das Raster.
    pub fc1: Vec<u8>,
    pub gelu: u8,
}

/// Skalen des Projektors.
#[derive(Debug, Clone, Deserialize)]
pub struct Projektorskalen {
    pub rest_ende: Vec<u8>,
    pub n: u8,
    pub fc1: Vec<u8>,
    pub gelu: u8,
    /// Skala der Bildtoken, je Kanal: die des Turms, nicht die des
    /// Decoders (siehe [`Iris::ausgangsskala`]).
    pub aus: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
struct Skalen {
    bloecke: Vec<Blockskalen>,
    projektor: Projektorskalen,
}

/// Masse des Turms aus `iris.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Masse {
    pub tiefe: usize,
    pub breite: usize,
    pub mlp: usize,
    pub koepfe: usize,
    pub kopf: usize,
    pub flicken: usize,
    pub flicken_ein_frac: u8,
    pub zusammen: usize,
    pub positionsseite: usize,
    pub ausgabe: usize,
    pub drehfrequenzen: usize,
    /// Bruchstellen der Drehtabelle des Turms (14, nicht die 8 des Decoders).
    pub dreh_frac: u8,
    /// Raster, Aufloesung und Versatz der GELU-Tabellen des Turms.
    pub gelu_ein_frac: u8,
    pub gelu_aus_frac: u8,
    pub gelu_versatz: i16,
    pub eps_q40: i64,
    pub bildtoken: Option<u32>,
    pub bild_anfang: Option<u32>,
    pub bild_ende: Option<u32>,
    pub hoechstens_pixel: usize,
    skalen: Skalen,
    /// Skala des Produkts jeder linearen Schicht **ohne** Bias, je Kanal,
    /// nach dem Namen der Schicht (siehe [`Linear::rechnen`]).
    roh: std::collections::HashMap<String, Vec<u8>>,
}

/// Eine LayerNorm: Gamma in int8, Beta in int16, beide je Element skaliert.
#[derive(Debug)]
struct Norm {
    gamma: QTensor,
    beta: BiasTensor,
}

/// Eine lineare Schicht mit Bias.
#[derive(Debug)]
struct Linear {
    w: QTensor,
    b: BiasTensor,
    /// Skala des Produkts ohne Bias, je Kanal.
    roh: Vec<u8>,
}

impl Linear {
    /// **Produkt plus Bias, einmal gerundet**, auf `aus` je Kanal.
    ///
    /// ⛔️ **Das Produkt hat seine eigene Skala.** Wo Produkt und Bias sich
    /// fast aufheben, passt die Summe auf eine feine Skala, das Produkt
    /// allein nicht. Lag es schon auf der Skala der Summe, klemmte es: `fc2`
    /// in Block 0 lag so auf demselben Eingang 16 % neben der
    /// Gleitkommarechnung, 99 % davon in fuenf Kanaelen (2026-10-23). Jetzt
    /// entsteht das Produkt auf der feineren der beiden Skalen, die es
    /// fassen kann, und Bias und Produkt werden in i64 addiert.
    fn rechnen(&self, xs: &[&[i16]], ein: usize, x_frac: u8, aus: &[u8]) -> Vec<Vec<i16>> {
        let zwischen: Vec<u8> = self.roh.iter().zip(aus).map(|(&r, &a)| r.min(a)).collect();
        let mut y = linear_w8a16_pc_stapel(xs, &self.w.data, ein, &self.w.shifts, x_frac, &zwischen);
        for zeile in y.iter_mut() {
            for (c, o) in zeile.iter_mut().enumerate() {
                let fein = zwischen[c].max(self.b.shifts[c]).max(aus[c]);
                let p = i64::from(*o) << u32::from(fein - zwischen[c]);
                let b = rescale_i64(i64::from(self.b.data[c]), self.b.shifts[c], fein);
                *o = clamp_i16_from_i64(rescale_i64(p + b, fein, aus[c]));
            }
        }
        y
    }
}

#[derive(Debug)]
struct Block {
    n1: Norm,
    qkv: Linear,
    proj: Linear,
    n2: Norm,
    fc1: Linear,
    fc2: Linear,
}

/// Was der Turm vom Decoder braucht: dessen Tabellen und Spezifikation.
#[derive(Debug, Clone)]
pub struct Decoderanteil {
    pub rsqrt_lut: Vec<i16>,
    pub exp_lut: Vec<i16>,
    pub config: ModelConfig,
    /// Breite des Decoders; die Ausgabe des Projektors muss sie treffen.
    pub breite: usize,
}

impl Decoderanteil {
    /// **Den Anteil des Decoders aus seinem Artefakt**, ohne dessen Gewichte:
    /// Tabellen, Spezifikation und die Eingangsskala der ersten Ebene. Fuer
    /// Proben, die den Turm allein rechnen; neben einem geladenen Decoder
    /// nimmt man dessen Werte.
    pub fn aus_artefakt(artefakt: &Path) -> Result<Self, String> {
        let luts = load_luts(artefakt)?;
        let lut = |n: &str| luts.tables.get(n).cloned().ok_or_else(|| format!("Decoder: Tabelle {n} fehlt"));
        Ok(Decoderanteil {
            rsqrt_lut: lut("rsqrt")?,
            exp_lut: lut("exp")?,
            config: crate::loader::spec_model_params()?,
            breite: load_model_dims(artefakt)?.hidden_size,
        })
    }
}

/// Iris, geladen.
#[derive(Debug)]
pub struct Iris {
    pub masse: Masse,
    flicken: Linear,
    pos: QTensor,
    bloecke: Vec<Block>,
    proj_norm: Norm,
    proj_fc1: Linear,
    proj_fc2: Linear,
    gelu_tanh: Vec<i16>,
    gelu_erf: Vec<i16>,
    dreh_cos: Vec<i16>,
    dreh_sin: Vec<i16>,
    decoder: Decoderanteil,
}

/// Ein Mitschnitt je Stufe: Name, Werte je Flicken, Skala je Kanal.
pub type Beobachter<'a> = &'a mut dyn FnMut(&str, &[Vec<i16>], &[u8]);

fn tensor(w: &LoadedWeights, name: &str) -> Result<QTensor, String> {
    w.get(name).cloned().ok_or_else(|| format!("{ORDNER}: Tensor {name} fehlt"))
}

fn bias(w: &LoadedWeights, name: &str) -> Result<BiasTensor, String> {
    let key = name.replace('.', "_");
    w.biases.get(&key).cloned().ok_or_else(|| format!("{ORDNER}: Bias {name} fehlt"))
}

fn linear(w: &LoadedWeights, roh: &std::collections::HashMap<String, Vec<u8>>, stamm: &str) -> Result<Linear, String> {
    let roh = roh.get(stamm).cloned().ok_or_else(|| format!("{ORDNER}: Rohskala {stamm} fehlt"))?;
    Ok(Linear { w: tensor(w, &format!("{stamm}.weight"))?, b: bias(w, &format!("{stamm}.bias"))?, roh })
}

fn norm(w: &LoadedWeights, stamm: &str) -> Result<Norm, String> {
    Ok(Norm { gamma: tensor(w, &format!("{stamm}.weight"))?, beta: bias(w, &format!("{stamm}.bias"))? })
}

impl Iris {
    /// Liegt neben diesem Decoder ein Turm?
    pub fn vorhanden(artefakt: &Path) -> bool {
        artefakt.join(ORDNER).join("iris.json").is_file()
    }

    /// Laedt den Turm aus `<artefakt>/iris`, mit dem Anteil des Decoders.
    pub fn laden(artefakt: &Path, decoder: Decoderanteil) -> Result<Self, String> {
        let ort = artefakt.join(ORDNER);
        let text = std::fs::read_to_string(ort.join("iris.json"))
            .map_err(|e| format!("{ORDNER}/iris.json: {e}"))?;
        let masse: Masse = serde_json::from_str(&text).map_err(|e| format!("{ORDNER}/iris.json: {e}"))?;
        if masse.skalen.bloecke.len() != masse.tiefe {
            return Err(format!("{ORDNER}: {} Blockskalen fuer {} Bloecke", masse.skalen.bloecke.len(), masse.tiefe));
        }
        if decoder.breite != masse.ausgabe || masse.skalen.projektor.aus.len() != masse.ausgabe {
            return Err(format!(
                "{ORDNER}: Ausgabebreite {} passt nicht zum Decoder ({})",
                masse.ausgabe, decoder.breite
            ));
        }
        let w = load_weights(&ort)?;
        let luts = load_luts(&ort)?;
        let lut = |n: &str| luts.tables.get(n).cloned().ok_or_else(|| format!("{ORDNER}: Tabelle {n} fehlt"));
        let bloecke = (0..masse.tiefe)
            .map(|i| {
                let p = format!("blocks.{i}");
                Ok(Block {
                    n1: norm(&w, &format!("{p}.norm1"))?,
                    qkv: linear(&w, &masse.roh, &format!("{p}.attn.qkv"))?,
                    proj: linear(&w, &masse.roh, &format!("{p}.attn.proj"))?,
                    n2: norm(&w, &format!("{p}.norm2"))?,
                    fc1: linear(&w, &masse.roh, &format!("{p}.mlp.linear_fc1"))?,
                    fc2: linear(&w, &masse.roh, &format!("{p}.mlp.linear_fc2"))?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Iris {
            flicken: linear(&w, &masse.roh, "patch_embed.proj")?,
            pos: tensor(&w, "pos_embed.weight")?,
            bloecke,
            proj_norm: norm(&w, "merger.norm")?,
            proj_fc1: linear(&w, &masse.roh, "merger.linear_fc1")?,
            proj_fc2: linear(&w, &masse.roh, "merger.linear_fc2")?,
            gelu_tanh: lut("gelu_tanh")?,
            gelu_erf: lut("gelu_erf")?,
            dreh_cos: lut("dreh_cos")?,
            dreh_sin: lut("dreh_sin")?,
            masse,
            decoder,
        })
    }

    /// **Die Bildtoken eines Bildes.**
    ///
    /// `flicken` sind `gh·gw` Flicken zu je [`FLICKENWERTE`] Werten `2p - 255`
    /// (Kanal, Zeile, Spalte), in der Folge von [`flickenfolge`]. Zurueck
    /// kommen `gh·gw/4` Vektoren der Decoderbreite auf der Skala
    /// [`Self::ausgangsskala`].
    pub fn vorwaerts(&self, flicken: &[i16], gh: usize, gw: usize) -> Vec<Vec<i16>> {
        self.vorwaerts_mit(flicken, gh, gw, &mut |_, _, _| {})
    }

    /// Dasselbe, mit einem Blick auf jede Stufe.
    pub fn vorwaerts_mit(&self, flicken: &[i16], gh: usize, gw: usize, sieh: Beobachter<'_>) -> Vec<Vec<i16>> {
        let m = &self.masse;
        let n = gh * gw;
        assert_eq!(flicken.len(), n * FLICKENWERTE, "iris: {} Werte fuer {gh}x{gw} Flicken", flicken.len());
        let folge = flickenfolge(gh, gw);
        let faeden = faeden();

        // 1. Flickeneinbettung plus Positions-Einbettung.
        let rest0 = &m.skalen.bloecke[0].rest_ein;
        let eingaenge: Vec<&[i16]> = flicken.as_chunks::<FLICKENWERTE>().0.iter().map(|c| c.as_slice()).collect();
        let mut x = self.flicken.rechnen(&eingaenge, FLICKENWERTE, m.flicken_ein_frac, rest0);
        let pos: Vec<Vec<i16>> = verteilen(n, faeden, |i| {
            positions_einbettung(
                &self.pos.data,
                &self.pos.shifts,
                m.breite,
                m.positionsseite,
                folge[i],
                (gh, gw),
                rest0,
            )
        });
        for (xi, pi) in x.iter_mut().zip(&pos) {
            for (a, &b) in xi.iter_mut().zip(pi) {
                *a = clamp_i16_from_i64(i64::from(*a) + i64::from(b));
            }
        }
        sieh("vor_block_00", &x, rest0);

        // 2. Die Bloecke.
        for (i, blk) in self.bloecke.iter().enumerate() {
            let sk = &m.skalen.bloecke[i];
            let rest_aus = match m.skalen.bloecke.get(i + 1) {
                Some(naechste) => &naechste.rest_ein,
                None => &m.skalen.projektor.rest_ende,
            };
            x = self.block(blk, sk, x, &folge, rest_aus, &mut |stufe, w, f| sieh(&format!("b{i:02}.{stufe}"), w, f));
            sieh(&format!("block_{i:02}"), &x, rest_aus);
        }

        // 3. Der Projektor.
        let pk = &m.skalen.projektor;
        let normiert: Vec<Vec<i16>> =
            verteilen(n, faeden, |j| self.norm(&self.proj_norm, &x[j], &pk.rest_ende, pk.n));
        sieh("p.n", &normiert, &vec![pk.n; m.breite]);
        let gruppe = m.zusammen * m.zusammen;
        let zusammen: Vec<Vec<i16>> = normiert.chunks_exact(gruppe).map(|g| g.concat()).collect();
        let refs: Vec<&[i16]> = zusammen.iter().map(|v| v.as_slice()).collect();
        let breite4 = m.breite * gruppe;
        let mut h = self.proj_fc1.rechnen(&refs, breite4, pk.n, &pk.fc1);
        sieh("p.fc1", &h, &pk.fc1);
        for hi in h.iter_mut() {
            self.gelu(hi, &self.gelu_erf, &pk.fc1, pk.gelu);
        }
        sieh("p.gelu", &h, &vec![pk.gelu; breite4]);
        let refs: Vec<&[i16]> = h.iter().map(|v| v.as_slice()).collect();
        let aus_frac = &pk.aus;
        let aus = self.proj_fc2.rechnen(&refs, breite4, pk.gelu, aus_frac);
        sieh("projektor", &aus, aus_frac);
        aus
    }

    /// **Die Skala der Bildtoken**, je Kanal.
    ///
    /// ⚠️ **Nicht die Eingangsskala der ersten Decoderebene.** Die ist an
    /// Text kalibriert und traegt nur rund +-0,06; Bildtoken erreichen 3,4
    /// und klemmten dort in 2044 von 2048 Kanaelen (gemessen am
    /// 2026-10-23). Die Einspeisung muss deshalb den Decoder auf Folgen mit
    /// Bildern kalibriert vorfinden.
    pub fn ausgangsskala(&self) -> &[u8] {
        &self.masse.skalen.projektor.aus
    }

    fn norm(&self, nm: &Norm, x: &[i16], x_frac: &[u8], aus: u8) -> Vec<i16> {
        let cfg = &self.decoder.config;
        layernorm_i16(
            x,
            x_frac,
            &nm.gamma.data,
            &nm.gamma.shifts,
            &nm.beta.data,
            &nm.beta.shifts,
            &self.decoder.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            inv_n_q20(x.len()),
            self.masse.eps_q40,
            aus,
        )
    }

    /// GELU ueber eine Tabelle, Eingang je Kanal auf `ein`, Ausgang auf `aus`.
    ///
    /// Die Tabelle rastert in `gelu_ein_frac` Bruchstellen und loest in
    /// `gelu_aus_frac` auf. Ein groeberer Eingang wird fuer den Nachschlag
    /// nach links geschoben; ein Wert ueber der Tabelle geht ueber die
    /// Identitaet, in i64, und erst danach auf `aus`.
    fn gelu(&self, h: &mut [i16], lut: &[i16], ein: &[u8], aus: u8) {
        let m = &self.masse;
        for (v, &e) in h.iter_mut().zip(ein) {
            debug_assert!(e <= m.gelu_ein_frac && aus <= m.gelu_aus_frac);
            let x = i32::from(*v) << u32::from(m.gelu_ein_frac - e);
            let y = gelu_nachschlagen(x, lut, m.gelu_versatz, m.gelu_ein_frac, m.gelu_aus_frac);
            *v = clamp_i16_from_i64(rescale_i64(y, m.gelu_aus_frac, aus));
        }
    }

    fn block(
        &self,
        blk: &Block,
        sk: &Blockskalen,
        x: Vec<Vec<i16>>,
        folge: &[(usize, usize)],
        rest_aus: &[u8],
        sieh: Beobachter<'_>,
    ) -> Vec<Vec<i16>> {
        let m = &self.masse;
        let cfg = &self.decoder.config;
        let n = x.len();
        let faeden = faeden();
        let b = m.breite;

        // Achtsamkeit.
        let n1: Vec<Vec<i16>> = verteilen(n, faeden, |j| self.norm(&blk.n1, &x[j], &sk.rest_ein, sk.n1));
        sieh("n1", &n1, &vec![sk.n1; b]);
        let refs: Vec<&[i16]> = n1.iter().map(|v| v.as_slice()).collect();
        let qkv_frac: Vec<u8> = [sk.q, sk.k, sk.v].iter().flat_map(|&f| std::iter::repeat_n(f, b)).collect();
        let mut qkv = blk.qkv.rechnen(&refs, b, sk.n1, &qkv_frac);
        sieh("qkv", &qkv, &qkv_frac);
        let haelfte = 2 * m.drehfrequenzen;
        let gedreht: Vec<Vec<i16>> = verteilen(n, faeden, |j| {
            let mut z = qkv[j].clone();
            let (cos, sin) = drehzeile(&self.dreh_cos, &self.dreh_sin, m.drehfrequenzen, folge[j]);
            debug_assert_eq!(cos.len(), haelfte);
            for teil in 0..2 {
                for kopf in 0..m.koepfe {
                    let a = teil * b + kopf * m.kopf;
                    let neu = rotate_half_split_i16(&z[a..a + m.kopf], &cos, &sin, m.dreh_frac);
                    z[a..a + m.kopf].copy_from_slice(&neu);
                }
            }
            z
        });
        qkv.clear();

        // Je Kopf die Schluessel und Werte aller Flicken hintereinander.
        let schluessel: Vec<(Vec<i16>, Vec<i16>)> = (0..m.koepfe)
            .map(|kopf| {
                let mut k = Vec::with_capacity(n * m.kopf);
                let mut v = Vec::with_capacity(n * m.kopf);
                for z in &gedreht {
                    let a = kopf * m.kopf;
                    k.extend_from_slice(&z[b + a..b + a + m.kopf]);
                    v.extend_from_slice(&z[2 * b + a..2 * b + a + m.kopf]);
                }
                (k, v)
            })
            .collect();
        let score_mult = inv_sqrt_q15(m.kopf);
        let score_shift = (u16::from(sk.q) + u16::from(sk.k) + 15).saturating_sub(u16::from(cfg.score_frac_bits)) as u8;
        let lut_shift = cfg.score_frac_bits.saturating_sub(cfg.exp_input_frac);
        let achtsam: Vec<Vec<i16>> = verteilen(n, faeden, |j| {
            let mut zeile = Vec::with_capacity(b);
            for (kopf, (k, v)) in schluessel.iter().enumerate() {
                let a = kopf * m.kopf;
                let q = &gedreht[j][a..a + m.kopf];
                zeile.extend(achtsamkeit_spaet_normiert(q, k, v, score_mult, score_shift, &self.decoder.exp_lut, lut_shift));
            }
            zeile
        });
        let refs: Vec<&[i16]> = achtsam.iter().map(|v| v.as_slice()).collect();
        sieh("achtsam", &achtsam, &vec![sk.v; b]);
        let proj = blk.proj.rechnen(&refs, b, sk.v, &sk.attn);
        sieh("attn", &proj, &sk.attn);
        let x: Vec<Vec<i16>> = x
            .iter()
            .zip(&proj)
            .map(|(xi, pi)| restsumme(xi, &sk.rest_ein, pi, &sk.attn, &sk.rest_mitte))
            .collect();
        sieh("rest_mitte", &x, &sk.rest_mitte);

        // MLP.
        let n2: Vec<Vec<i16>> = verteilen(n, faeden, |j| self.norm(&blk.n2, &x[j], &sk.rest_mitte, sk.n2));
        sieh("n2", &n2, &vec![sk.n2; b]);
        let refs: Vec<&[i16]> = n2.iter().map(|v| v.as_slice()).collect();
        let mut h = blk.fc1.rechnen(&refs, b, sk.n2, &sk.fc1);
        sieh("fc1", &h, &sk.fc1);
        for hi in h.iter_mut() {
            self.gelu(hi, &self.gelu_tanh, &sk.fc1, sk.gelu);
        }
        sieh("gelu", &h, &vec![sk.gelu; m.mlp]);
        let refs: Vec<&[i16]> = h.iter().map(|v| v.as_slice()).collect();
        let runter = blk.fc2.rechnen(&refs, m.mlp, sk.gelu, &sk.mlp);
        sieh("mlp", &runter, &sk.mlp);
        x.iter()
            .zip(&runter)
            .map(|(xi, ri)| restsumme(xi, &sk.rest_mitte, ri, &sk.mlp, rest_aus))
            .collect()
    }
}

/// **Residualstrom plus Zweig**, je Kanal: beide auf die feinere ihrer
/// Skalen gebracht, in i64 addiert und einmal auf `frac` gerundet.
///
/// ⛔️ **Der Zweig traegt seine eigene Skala.** Laege er schon auf der
/// Skala der Summe, klemmte er dort, wo Residuum und Zweig sich fast
/// aufheben; genau das tun die grossen Kanaele des Turms.
fn restsumme(x: &[i16], x_frac: &[u8], zweig: &[i16], zweig_frac: &[u8], frac: &[u8]) -> Vec<i16> {
    (0..x.len())
        .map(|c| {
            let fein = x_frac[c].max(zweig_frac[c]);
            let a = i64::from(x[c]) << u32::from(fein - x_frac[c]);
            let z = i64::from(zweig[c]) << u32::from(fein - zweig_frac[c]);
            clamp_i16_from_i64(rescale_i64(a + z, fein, frac[c]))
        })
        .collect()
}
