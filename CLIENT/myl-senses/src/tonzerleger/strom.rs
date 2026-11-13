//! **Der Rueckweg mit Zustand**: Bild fuer Bild weiter, und jedes Stueck
//! klingt, wie es im ganzen Satz geklungen haette (CLIENT 8.5).
//!
//! Der Rueckweg ist kausal; was ein Bild braucht, liegt davor. Der Strom
//! haelt genau das fest:
//!
//! - je kausaler Faltung die letzten `(K − 1) · d` Eingaben,
//! - je transponierter Faltung den Ueberhang von `K − s` Ausgaben, die
//!   schon Beitraege tragen, aber noch auf die naechste Eingabe warten,
//! - je Transformerebene Schluessel und Werte der letzten `fenster − 1`
//!   Stellen und die Stelle selbst (RoPE).
//!
//! ⚑ **Ein Weg fuer alles**: [`super::zurueck_mit`] ist ein Strom, der alle
//! Bilder auf einmal bekommt. In `f32` rechnet jede Zeile dieselbe Summe in
//! derselben Reihenfolge, gleich wie die Bilder gestueckelt ankommen; die
//! Probe `tonzerleger` prueft das bitgleich. ⚠️ In int8 haengen die
//! Ausgabebits einer Matrix davon ab, was ein Aufruf zusammen rechnet;
//! gestueckelt ist der Ton dort gleichwertig, nicht bitgleich.

use std::collections::BTreeMap;

use super::{faltung, faltung_je_kanal, gelu, layernorm, linear, linear_genau, quer_gewicht, rechne_ganz, rmsnorm, snake, Signal, Tonzerleger, CODES};

/// **Der Zustand des Rueckwegs** nach den Bildern, die er schon bekam.
/// Eine Kopie setzt an derselben Stelle fort (etwa hinter einer
/// Stimmprobe, fuer jeden Satz neu).
#[derive(Debug, Clone, Default)]
pub struct Rueckstrom {
    /// Je kausaler Faltung die letzten Eingaben, kanalweise.
    gedaechtnis: BTreeMap<String, Signal>,
    /// Je transponierter Faltung der Ueberhang, kanalweise `[o][K − s]`.
    ueberhang: BTreeMap<String, Vec<f32>>,
    /// Je Transformerebene Schluessel und Werte.
    speicher: Vec<(Vec<f32>, Vec<f32>)>,
    /// Wie viele Bilder schon kamen.
    bilder: usize,
}

impl Rueckstrom {
    /// Ein Strom am Anfang.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Wie viele Bilder schon kamen.
    pub fn bilder(&self) -> usize {
        self.bilder
    }

    /// **Die naechsten Bilder zu Ton**: genau ihre Proben, 24 kHz in `[-1, 1]`.
    pub fn weiter(&mut self, codes: &[[u16; CODES]], z: &Tonzerleger) -> Vec<f32> {
        self.weiter_mit(codes, z, |_, _| ())
    }

    /// Wie [`Self::weiter`], und `stufe` bekommt jede Zwischenstufe dieser
    /// Bilder mit Namen, kanalweise (fuer die Probe).
    pub fn weiter_mit(&mut self, codes: &[[u16; CODES]], z: &Tonzerleger, mut stufe: impl FnMut(&str, &[f32])) -> Vec<f32> {
        let n = codes.len();
        if n == 0 {
            return Vec::new();
        }
        // 1. Codebuecher.
        let q = codebuecher(codes, z);
        stufe("quantisiert", &q.werte);
        // 2. und 3.
        let h = self.faltung(&q, z, "pre_conv.conv", 1);
        let x = h.quer();
        stufe("vorfaltung", &x);
        let mut x2 = linear(&x, n, z, "pre_transformer.input_proj", true);
        let ebenen = (0..).take_while(|i| z.t.contains_key(&format!("pre_transformer.layers.{i}.input_layernorm.weight"))).count();
        self.speicher.resize(ebenen, Default::default());
        for nr in 0..ebenen {
            super::ebene(&mut x2, z, nr, &mut self.speicher[nr], self.bilder);
        }
        let x2 = rmsnorm(&x2, z.w("pre_transformer.norm.weight"), z.m.rms_eps);
        let x = linear(&x2, n, z, "pre_transformer.output_proj", true);
        stufe("transformer", &x);
        // 4.
        let mut s = Signal::aus_quer(&x, n, h.kanaele);
        for (i, &f) in z.m.vorab.iter().enumerate() {
            s = self.transponiert(&s, z, &format!("upsample.{i}.0.conv"), f);
            s = self.convnext(&s, z, &format!("upsample.{i}.1"));
        }
        stufe("hochgetastet", &s.werte);
        // 5.
        let mut s = self.faltung(&s, z, "decoder.0.conv", 1);
        stufe("faltung0", &s.werte);
        for (i, &r) in z.m.hochtasten.iter().enumerate() {
            let p = format!("decoder.{}.block", i + 1);
            snake(&mut s, z, &format!("{p}.0"));
            s = self.transponiert(&s, z, &format!("{p}.1.conv"), r);
            for (j, d) in [1usize, 3, 9].into_iter().enumerate() {
                let rp = format!("{p}.{}", j + 2);
                let rest = s.clone();
                snake(&mut s, z, &format!("{rp}.act1"));
                s = self.faltung(&s, z, &format!("{rp}.conv1.conv"), d);
                snake(&mut s, z, &format!("{rp}.act2"));
                s = self.faltung(&s, z, &format!("{rp}.conv2.conv"), 1);
                s.werte.iter_mut().zip(&rest.werte).for_each(|(a, b)| *a += b);
            }
            stufe(&format!("block{}", i + 1), &s.werte);
        }
        let letzte = z.m.hochtasten.len() + 1;
        snake(&mut s, z, &format!("decoder.{letzte}"));
        let s = self.faltung(&s, z, &format!("decoder.{}.conv", letzte + 1), 1);
        let ton: Vec<f32> = s.werte.iter().map(|v| v.clamp(-1.0, 1.0)).collect();
        stufe("ton", &ton);
        self.bilder += n;
        ton
    }

    /// **Kausale Faltung mit Gedaechtnis**: Die gemerkten Eingaben stehen
    /// vor den neuen, [`faltung`] rechnet ueber beide, und nur die Ausgaben
    /// der neuen zaehlen. Am Anfang sind es Nullen, wie der Rand der Faltung.
    fn faltung(&mut self, x: &Signal, z: &Tonzerleger, name: &str, dilatation: usize) -> Signal {
        let k = z.form(&format!("{name}.weight"))[2];
        self.mit_gedaechtnis(x, name, (k - 1) * dilatation, |e| faltung(e, z, name, dilatation))
    }

    /// Dasselbe fuer die Faltung je Kanal.
    fn je_kanal(&mut self, x: &Signal, z: &Tonzerleger, name: &str) -> Signal {
        let k = z.w(&format!("{name}.weight")).len() / x.kanaele;
        self.mit_gedaechtnis(x, name, k - 1, |e| faltung_je_kanal(e, z, name))
    }

    fn mit_gedaechtnis(&mut self, x: &Signal, name: &str, rand: usize, rechnen: impl Fn(&Signal) -> Signal) -> Signal {
        let alt = self.gedaechtnis.remove(name).unwrap_or_else(|| Signal::null(x.kanaele, rand));
        let ganz = alt.daneben(x);
        let y = rechnen(&ganz).ab(rand);
        self.gedaechtnis.insert(name.to_string(), ganz.ab(ganz.laenge - rand));
        y
    }

    /// **Transponierte Faltung mit Ueberhang**, Schritt `s`, Kern `K`:
    /// `y[o][t·s + j] += Σ_c x[c][t] · w[c][o][j]`. Fertig sind nach den
    /// neuen Eingaben die ersten `n · s` Ausgaben; die `K − s` danach tragen
    /// schon Verschiebung und Beitraege und warten im Ueberhang. Die
    /// Reihenfolge der Summen ist dieselbe wie in einem Zug: Verschiebung,
    /// dann die Beitraege nach steigendem `t`.
    fn transponiert(&mut self, x: &Signal, z: &Tonzerleger, name: &str, schritt: usize) -> Signal {
        let form = z.form(&format!("{name}.weight"));
        let (ein, aus, k) = (form[0], form[1], form[2]);
        assert_eq!(ein, x.kanaele, "{name}: {} Kanaele statt {ein}", x.kanaele);
        assert!(k >= schritt, "{name}: Kern {k} kuerzer als der Schritt {schritt}");
        let b = z.w(&format!("{name}.bias"));
        let beitrag = match z.ganz.get(&format!("{name}.weight")) {
            Some(m) => rechne_ganz(&x.quer(), x.laenge, m, z.acht),
            None => linear_genau(&x.quer(), x.laenge, &quer_gewicht(z.w(&format!("{name}.weight")), ein, aus, k), aus * k),
        };
        let (laenge, ueber) = (x.laenge * schritt, k - schritt);
        let alt = self.ueberhang.remove(name).unwrap_or_else(|| (0..aus).flat_map(|o| std::iter::repeat_n(b[o], ueber)).collect());
        // Je Kanal Ausgabe und Ueberhang in einem Zug: `laenge + ueber` Werte.
        let breit = laenge + ueber;
        let mut alles = vec![0f32; aus * breit];
        let n = x.laenge;
        crate::whisper::je_kern(&mut alles, breit, |o0, stueck| {
            for (dobj, summe) in stueck.chunks_mut(breit).enumerate() {
                let o = o0 + dobj;
                summe.iter_mut().for_each(|v| *v = b[o]);
                summe[..ueber].copy_from_slice(&alt[o * ueber..][..ueber]);
                for t in 0..n {
                    for j in 0..k {
                        summe[t * schritt + j] += beitrag[t * aus * k + o * k + j];
                    }
                }
            }
        });
        let mut y = Signal::null(aus, laenge);
        let mut neu = vec![0f32; aus * ueber];
        for (o, summe) in alles.chunks(breit).enumerate() {
            y.werte[o * laenge..][..laenge].copy_from_slice(&summe[..laenge]);
            neu[o * ueber..][..ueber].copy_from_slice(&summe[laenge..]);
        }
        self.ueberhang.insert(name.to_string(), neu);
        y
    }

    /// ConvNeXt-Block: Faltung je Kanal (mit Gedaechtnis), LayerNorm,
    /// 4-fach breit mit GELU, zurueck, mal `gamma`, Rest.
    fn convnext(&mut self, x: &Signal, z: &Tonzerleger, p: &str) -> Signal {
        let h = self.je_kanal(x, z, &format!("{p}.dwconv.conv"));
        let n = h.laenge;
        let a = layernorm(&h.quer(), z.w(&format!("{p}.norm.weight")), z.w(&format!("{p}.norm.bias")), 1e-6);
        let mut a = linear(&a, n, z, &format!("{p}.pwconv1"), true);
        a.iter_mut().for_each(|v| *v = gelu(*v));
        let a = linear(&a, n, z, &format!("{p}.pwconv2"), true);
        let g = z.w(&format!("{p}.gamma"));
        let mut y = x.clone();
        for t in 0..n {
            for c in 0..x.kanaele {
                y.werte[c * n + t] += g[c] * a[t * x.kanaele + c];
            }
        }
        y
    }
}

/// Codebuecher: je Code der Eintrag `embedding_sum / cluster_usage`,
/// summiert, je Gruppe eine 1×1-Projektion, beide addiert.
fn codebuecher(codes: &[[u16; CODES]], z: &Tonzerleger) -> Signal {
    let n = codes.len();
    let mut q = Signal::null(z.form("quantizer.rvq_first.output_proj.weight")[0], n);
    for (gruppe, bereich) in [("quantizer.rvq_first", 0..1), ("quantizer.rvq_rest", 1..CODES)] {
        let dim = z.form(&format!("{gruppe}.output_proj.weight"))[1];
        let mut summe = vec![0f32; n * dim];
        for (nr, c) in bereich.enumerate() {
            let p = format!("{gruppe}.vq.layers.{nr}._codebook");
            let (e, u) = (z.w(&format!("{p}.embedding_sum")), z.w(&format!("{p}.cluster_usage")));
            for (t, bild) in codes.iter().enumerate() {
                let i = usize::from(bild[c]);
                let teiler = u[i].max(1e-5);
                for d in 0..dim {
                    summe[t * dim + d] += e[i * dim + d] / teiler;
                }
            }
        }
        let proj = linear(&summe, n, z, &format!("{gruppe}.output_proj"), false);
        let s = Signal::aus_quer(&proj, n, q.kanaele);
        q.werte.iter_mut().zip(&s.werte).for_each(|(a, b)| *a += b);
    }
    q
}
