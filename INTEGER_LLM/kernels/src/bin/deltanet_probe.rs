//! **Numerikstudie: bleibt ein Gated-DeltaNet-Zustand ganzzahlig stabil?**
//!
//! # ⚑ Die eine Frage vor jedem Kernel
//!
//! Die neue Qwen-Linie (`qwen3_5`, `qwen4_exp`) rechnet drei Viertel
//! ihrer Ebenen nicht mit Aufmerksamkeit, sondern mit **Gated DeltaNet**:
//! einem Zustand, der ueber die ganze Sequenz **fortgeschrieben** wird.
//! Die Referenz fuehrt ihn ausdruecklich in `float32`
//! (`mamba_ssm_dtype`), und die Modelle nennen 262 144 Positionen.
//!
//! **Der ganze Vertrag dieses Projekts lautet: exakt darstellbar,
//! Umskalierung nur als Zweierpotenz.** Ein Fehler in einem
//! Vorwaertsoperator bleibt lokal; ein Fehler in einem
//! fortgeschriebenen Zustand **waechst mit der Sequenz**. Ob er das tut,
//! ist keine Herleitung, sondern eine Messung, und diese Datei ist sie.
//!
//! # Die Rekursion
//!
//! Je Kopf, mit `S` von der Form `d_v x d_k`:
//!
//! ```text
//!   u_t = S_{t-1} k_t                      (d_v)
//!   S_t = a_t S_{t-1} + b_t (v_t - a_t u_t) k_t^T
//!   o_t = S_t q_t                          (d_v)
//! ```
//!
//! `a` ist das Vergessen (Gate), `b` die Schreibstaerke der Delta-Regel,
//! `k` ist auf Laenge eins normiert. Die Form oben ist die
//! ausmultiplizierte Fassung von `S_{t-1}(a(I - b k k^T)) + b v k^T`:
//! ein Matrixvektorprodukt, ein Rang-1-Zuschlag, ein zweites
//! Matrixvektorprodukt.
//!
//! # ⚠️ Was hier NICHT gemessen wird
//!
//! Die Qualitaet eines Modells. Gemessen wird der **Abstand zwischen
//! Gleitkomma und Ganzzahl** auf demselben Eingangsstrom, und zwar als
//! Funktion der Sequenzlaenge. **Ein Modell, dessen Zustand auseinander
//! laeuft, ist auch ohne Perplexitaet erledigt**; eines, dessen Zustand
//! haelt, ist damit noch nicht gut.
//!
//! Aufruf: `deltanet_probe [laenge] [d_k] [zerfall_min] [zerfall_max]`

use integer_llm_kernels::fixed_point::{clamp_i16_from_i64, rescale_i64};

/// Ein einfacher, wiederholbarer Wurf.
struct Wurf(u64);

impl Wurf {
    fn neu(saat: u64) -> Self {
        Self(saat | 1)
    }
    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Gleichverteilt in [-1, 1).
    fn wert(&mut self) -> f64 {
        (self.naechste() % 2_000_001) as f64 / 1_000_000.0 - 1.0
    }
    /// Gleichverteilt in [a, b).
    fn zwischen(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * ((self.naechste() % 1_000_001) as f64 / 1_000_000.0)
    }
}

/// Die Skalen der ganzzahligen Fassung, alle als Zweierpotenz.
#[derive(Clone, Copy)]
struct Skalen {
    /// Nachkommabits des Zustands.
    s_frac: u8,
    /// Nachkommabits von k, v und q.
    ein_frac: u8,
    /// Nachkommabits der Gates (Q15).
    gate_frac: u8,
    /// Nachkommabits der Ausgabe.
    aus_frac: u8,
    /// Wie breit der Zustand gehalten wird: 16 oder 32 Bit.
    breit: bool,
}

/// Ein Lauf in Gleitkomma, als Wahrheit.
struct Gleitkomma {
    s: Vec<f64>,
    dk: usize,
    dv: usize,
}

impl Gleitkomma {
    fn neu(dk: usize, dv: usize) -> Self {
        Self { s: vec![0.0; dv * dk], dk, dv }
    }

    fn schritt(&mut self, k: &[f64], v: &[f64], q: &[f64], a: f64, b: f64) -> Vec<f64> {
        // u = S k
        let mut u = vec![0.0; self.dv];
        for (i, ui) in u.iter_mut().enumerate() {
            let zeile = &self.s[i * self.dk..(i + 1) * self.dk];
            *ui = zeile.iter().zip(k).map(|(s, kk)| s * kk).sum();
        }
        // S = a S + b (v - a u) k^T
        for i in 0..self.dv {
            let zuschlag = b * (v[i] - a * u[i]);
            let zeile = &mut self.s[i * self.dk..(i + 1) * self.dk];
            for (j, sij) in zeile.iter_mut().enumerate() {
                *sij = a * *sij + zuschlag * k[j];
            }
        }
        // o = S q
        let mut o = vec![0.0; self.dv];
        for (i, oi) in o.iter_mut().enumerate() {
            let zeile = &self.s[i * self.dk..(i + 1) * self.dk];
            *oi = zeile.iter().zip(q).map(|(s, qq)| s * qq).sum();
        }
        o
    }

    fn energie(&self) -> f64 {
        (self.s.iter().map(|x| x * x).sum::<f64>() / self.s.len() as f64).sqrt()
    }
}

/// Derselbe Lauf, ganzzahlig.
struct Ganzzahl {
    /// Der Zustand; als i32 gehalten, aber nach jedem Schritt auf die
    /// vereinbarte Breite geklemmt.
    s: Vec<i32>,
    dk: usize,
    dv: usize,
    sk: Skalen,
    /// Wie oft ein Zustandswert an seine Grenze gestossen ist.
    geklemmt: u64,
}

impl Ganzzahl {
    fn neu(dk: usize, dv: usize, sk: Skalen) -> Self {
        Self { s: vec![0; dv * dk], dk, dv, sk, geklemmt: 0 }
    }

    /// ⚑ **Ohne Leihe auf `self`**, damit die Zustandsschleife
    /// mitzaehlen kann, waehrend sie `self.s` haelt. Sie gibt deshalb
    /// zurueck, **ob** geklemmt wurde, statt selbst zu zaehlen; der
    /// Zaehler steht an einer Stelle, die Grenze auch.
    fn klemmen(breit: bool, x: i64) -> (i32, bool) {
        if breit {
            (
                x.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                x > i32::MAX as i64 || x < i32::MIN as i64,
            )
        } else {
            (
                clamp_i16_from_i64(x) as i32,
                x > i16::MAX as i64 || x < i16::MIN as i64,
            )
        }
    }

    fn schritt(&mut self, k: &[i16], v: &[i16], q: &[i16], a: i32, b: i32) -> Vec<i16> {
        let sk = self.sk;
        // u = S k, auf der Zustandsskala gehalten
        let mut u = vec![0i64; self.dv];
        for (i, ui) in u.iter_mut().enumerate() {
            let zeile = &self.s[i * self.dk..(i + 1) * self.dk];
            let acc: i64 = zeile.iter().zip(k).map(|(s, kk)| *s as i64 * *kk as i64).sum();
            // Skala: s_frac + ein_frac -> s_frac
            *ui = rescale_i64(acc, sk.s_frac + sk.ein_frac, sk.s_frac);
        }
        // zuschlag = b (v - a u), auf der Zustandsskala
        let zuschlag: Vec<i64> = v
            .iter()
            .zip(u.iter())
            .map(|(vj, uj)| {
                // v auf die Zustandsskala bringen
                let vi = rescale_i64(*vj as i64, sk.ein_frac, sk.s_frac);
                let au = rescale_i64(a as i64 * *uj, sk.gate_frac, 0);
                rescale_i64(b as i64 * (vi - au), sk.gate_frac, 0)
            })
            .collect();
        // S = a S + zuschlag k^T
        //
        // ⚑ **Zeilenweise ueber `chunks_exact_mut`**, nicht ueber einen
        // Index. Die Zeile `i` des Zustands ist genau ein Abschnitt von
        // `dk` Eintraegen; der Index war dieselbe Rechnung von Hand.
        //
        // ⚠️ **Der Klemmzaehler muss mitlaufen**, denn „keine einzige
        // Klemmung" ist ein Ergebnis dieser Sonde und keine Nebensache.
        // `klemmen` nimmt `&mut self` und kollidiert deshalb mit der
        // Leihe auf `self.s`; gezaehlt wird hier lokal und danach
        // gebucht. **Dieselbe Rechnung, dieselbe Zahl.**
        let breit = sk.breit;
        let mut geklemmt = 0u64;
        for (z, zeile) in zuschlag.iter().zip(self.s.chunks_exact_mut(self.dk)) {
            for (sij, kj) in zeile.iter_mut().zip(k.iter()) {
                let alt = rescale_i64(a as i64 * *sij as i64, sk.gate_frac, 0);
                let neu = rescale_i64(*z * *kj as i64, sk.ein_frac, 0);
                let (wert, ueber) = Self::klemmen(breit, alt + neu);
                geklemmt += u64::from(ueber);
                *sij = wert;
            }
        }
        self.geklemmt += geklemmt;
        // o = S q
        let mut o = vec![0i16; self.dv];
        for (i, oi) in o.iter_mut().enumerate() {
            let zeile = &self.s[i * self.dk..(i + 1) * self.dk];
            let acc: i64 = zeile.iter().zip(q).map(|(s, qq)| *s as i64 * *qq as i64).sum();
            *oi = clamp_i16_from_i64(rescale_i64(acc, sk.s_frac + sk.ein_frac, sk.aus_frac));
        }
        o
    }

    fn energie(&self) -> f64 {
        let n = (1i64 << self.sk.s_frac) as f64;
        (self.s.iter().map(|&x| (x as f64 / n).powi(2)).sum::<f64>() / self.s.len() as f64).sqrt()
    }

    /// Wie viele Zustandswerte auf null gefallen sind.
    fn verloren(&self) -> f64 {
        self.s.iter().filter(|&&x| x == 0).count() as f64 / self.s.len() as f64
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let laenge: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(16_384);
    let dk: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(128);
    let zerfall_min: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.95);
    let zerfall_max: f64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.999);
    let dv = dk;

    println!("# Gated DeltaNet, ganzzahlig gegen Gleitkomma");
    println!("# d_k = d_v = {dk}, Laenge {laenge}, Zerfall in [{zerfall_min}, {zerfall_max}]");
    println!();

    for breit in [false, true] {
        let sk = Skalen {
            s_frac: if breit { 20 } else { 8 },
            ein_frac: 12,
            gate_frac: 15,
            aus_frac: 8,
            breit,
        };
        println!(
            "## Zustand in {} Bit, s_frac {}",
            if breit { 32 } else { 16 },
            sk.s_frac
        );
        println!("| t | rel. Fehler o | Energie f64 | Energie int | Nullanteil | geklemmt |");
        println!("|---|---|---|---|---|---|");

        let mut wurf = Wurf::neu(20260911);
        let mut f = Gleitkomma::neu(dk, dv);
        let mut g = Ganzzahl::neu(dk, dv, sk);
        let mut marken: Vec<usize> = vec![1, 4, 16, 64, 256, 1024, 4096, 16384, 65536, 262144];
        marken.retain(|&m| m <= laenge);
        if *marken.last().unwrap_or(&0) != laenge {
            marken.push(laenge);
        }

        for t in 1..=laenge {
            // k auf Laenge eins, v und q gleichverteilt.
            let mut k: Vec<f64> = (0..dk).map(|_| wurf.wert()).collect();
            let norm = k.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-12);
            for x in k.iter_mut() {
                *x /= norm;
            }
            let v: Vec<f64> = (0..dv).map(|_| wurf.wert()).collect();
            let q: Vec<f64> = (0..dk).map(|_| wurf.wert()).collect();
            let a = wurf.zwischen(zerfall_min, zerfall_max);
            let b = wurf.zwischen(0.1, 1.0);

            let eins = (1i64 << sk.ein_frac) as f64;
            let gate_eins = (1i64 << sk.gate_frac) as f64;
            let ki: Vec<i16> = k.iter().map(|x| (x * eins).round() as i16).collect();
            let vi: Vec<i16> = v.iter().map(|x| (x * eins).round() as i16).collect();
            let qi: Vec<i16> = q.iter().map(|x| (x * eins).round() as i16).collect();
            let ai = (a * gate_eins).round() as i32;
            let bi = (b * gate_eins).round() as i32;

            let of = f.schritt(&k, &v, &q, a, b);
            let og = g.schritt(&ki, &vi, &qi, ai, bi);

            if marken.contains(&t) {
                let n = (1i64 << sk.aus_frac) as f64;
                let groesse = of.iter().fold(0.0f64, |m, x| m.max(x.abs())).max(1e-12);
                let fehler = of
                    .iter()
                    .zip(&og)
                    .fold(0.0f64, |m, (a, b)| m.max((a - *b as f64 / n).abs()));
                println!(
                    "| {t} | {:.3} % | {:.4} | {:.4} | {:.1} % | {} |",
                    100.0 * fehler / groesse,
                    f.energie(),
                    g.energie(),
                    100.0 * g.verloren(),
                    g.geklemmt
                );
            }
        }
        println!();
    }
}
