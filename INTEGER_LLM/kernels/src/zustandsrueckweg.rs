//! **Der Rueckwaertspass der Rekurrenz**, ein Kopf, eine Folge.
//!
//! # ⚑ Was abgeleitet wird
//!
//! Die Rekurrenz aus [`crate::zustandsschicht`], je Token `t`:
//!
//! ```text
//! S'  = g * S_(t-1)
//! kv  = Summe_a S'[a][b] * k[a]
//! d   = (v - kv) * beta
//! S_t = S' + k (x) d
//! o   = Summe_a S_t[a][b] * q[a]
//! ```
//!
//! Rueckwaerts durch die Zeit, mit `dS` als Gradient nach `S_t`:
//!
//! ```text
//! dS     += q (x) dO               dq   = S_t  . dO
//! dd      = dS^T k                 dk   = dS   . d
//! dv      = dd * beta              dkv  = -dd * beta
//! dbeta   = Summe_b dd * (v - kv)
//! dS'     = dS + k (x) dkv         dk  += S'   . dkv
//! dg      = Summe dS' * S_(t-1)
//! dS_(t-1) = g * dS'
//! ```
//!
//! ⚑ **Abgeleitet wird die Implementierung, nicht die Formel**, wie im
//! Kopf von [`crate::backward`]: Die Zwischenwerte `kv` und `d` kommen aus
//! demselben Kern wie vorwaerts ([`schritt_mit_spur`]), `S'` aus derselben
//! Rundung ([`verblasst`]). Die Rundungen selbst leitet der Pass nicht ab;
//! er behandelt sie als Identitaet, wie jeder Rueckwaertspass hier.
//!
//! # ⚑ Die Skalen
//!
//! Alle Gradienten nach aussen liegen auf dem **Bus** des eingehenden
//! `dO`: Ein Bus-Wert ist `G * dL/dx` fuer den **reellen** Wert von `x`, mit
//! demselben `G` fuer alle. `dO` gilt dem reellen Ausgang der Rekurrenz,
//! unabhaengig von dessen Skala je Kopf und Token (Fund 418).
//!
//! Innen tragen `dS`, `dd` und `dkv` [`RUECK_EXTRA`] Bruchbits mehr als der
//! Bus. `dS` waechst ueber die Folge aus vielen kleinen Rang-1-Zuschlaegen,
//! und jeder wuerde sonst auf eine Bus-Stelle gerundet, bevor er verblasst.
//!
//! # ⚑ Der Speicher: Zustaende in Abstaenden, dazwischen neu gerechnet
//!
//! Ein Zustand sind `schluessel_dim * wert_dim` Werte in `i64`, beim
//! grossen Modell 128 KiB je Kopf. Alle Zustaende einer Folge
//! aufzuheben, kostete bei 512 Token und 32 Koepfen 2 GiB je Ebene. Der
//! Pass haelt deshalb nur jeden `abstand`-ten Zustand und rechnet die
//! dazwischen aus dem letzten gehaltenen neu, Abschnitt fuer Abschnitt von
//! hinten. **Die Rekurrenz ist ganzzahlig, also ist der neu gerechnete
//! Zustand derselbe**, Bit fuer Bit; der Abstand aendert keine Zahl,
//! nur Speicher und Zeit (Probe `der_abstand_aendert_keine_zahl`).

use crate::backward::Grad;
use crate::trainingsschritt::begrenze;
use crate::fixed_point::{rescale, rshift_round_i128, rshift_round_i64};
use crate::integer_math::sigmoid_nachschlagen;
use crate::zustandsschicht::{
    schritt, schritt_mit_spur, verblasst, zeile_verblassen, Torwerte, Zustand, Zustandstore, INTERN_FRAC, NORM_FRAC, WERT_FRAC,
    ZUSTAND_FRAC,
};

/// Zusaetzliche Bruchbits von `dS`, `dd` und `dkv` gegenueber dem Bus.
///
/// ⚑ **Gleich [`NORM_FRAC`]**, und das ist kein Zufall: Der groesste
/// Zuschlag auf `dS` ist `q (x) dO`, und mit dieser Wahl landet er ohne
/// Rundung, denn `q` traegt genau so viele Bruchbits.
pub const RUECK_EXTRA: u32 = NORM_FRAC;

/// **Die Eingaben eines Kopfes ueber eine Folge**, wie sie der Schritt
/// nimmt: `q` und `k` normiert auf `2^-NORM_FRAC`, `v` und `beta` auf
/// `2^-WERT_FRAC`, `g` auf `2^-ZERFALL_FRAC`. Je Token ein Eintrag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kopffolge {
    pub schluessel_dim: usize,
    pub wert_dim: usize,
    pub q: Vec<Vec<i16>>,
    pub k: Vec<Vec<i16>>,
    pub v: Vec<Vec<i16>>,
    pub g: Vec<i64>,
    pub beta: Vec<i16>,
}

impl Kopffolge {
    /// Die Zahl der Token.
    pub fn len(&self) -> usize {
        self.g.len()
    }

    /// Ob die Folge leer ist.
    pub fn is_empty(&self) -> bool {
        self.g.is_empty()
    }

    fn pruefen(&self) {
        let n = self.len();
        assert!(
            self.q.len() == n && self.k.len() == n && self.v.len() == n && self.beta.len() == n,
            "die Eingaben eines Kopfes sind verschieden lang"
        );
        let (sd, wd) = (self.schluessel_dim, self.wert_dim);
        assert!(
            self.q.iter().chain(&self.k).all(|x| x.len() == sd) && self.v.iter().all(|x| x.len() == wd),
            "eine Eingabe passt nicht zu den Massen des Zustands"
        );
    }

    /// **Der Vorwaertspass, ab leerem Zustand**: je Token die Ausgabe und
    /// ihre Skala, genau wie [`schritt`] in der Schleife.
    pub fn vorwaerts(&self) -> (Vec<Vec<i16>>, Vec<u8>) {
        self.pruefen();
        let mut z = Zustand::leer(self.schluessel_dim, self.wert_dim);
        let mut aus = Vec::with_capacity(self.len());
        let mut skalen = Vec::with_capacity(self.len());
        for t in 0..self.len() {
            let mut o = vec![0i16; self.wert_dim];
            skalen.push(schritt(&mut z, &self.q[t], &self.k[t], &self.v[t], self.g[t], self.beta[t], &mut o));
            aus.push(o);
        }
        (aus, skalen)
    }
}

/// **Die Gradienten eines Kopfes**, je Token, alle auf dem Bus des
/// eingehenden Gradienten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kopfgradienten {
    pub q: Vec<Vec<Grad>>,
    pub k: Vec<Vec<Grad>>,
    pub v: Vec<Vec<Grad>>,
    pub g: Vec<Grad>,
    pub beta: Vec<Grad>,
}

/// Was ein Token im Rueckwaertspass von vorwaerts braucht.
struct Tokenspur {
    /// `S_(t-1)`
    davor: Vec<i64>,
    /// `S' = g * S_(t-1)`, mit der Rundung des Schritts
    verblasst: Vec<i64>,
    /// `S_t`
    danach: Vec<i64>,
    /// `kv` und `d`, auf `INTERN_FRAC`
    kv: Vec<i32>,
    delta: Vec<i32>,
}

/// **Der Rueckwaertspass eines Kopfes ueber eine Folge ab leerem Zustand.**
///
/// `d_aus[t]` ist der Gradient nach dem reellen Ausgang bei Token `t`, auf
/// dem Bus. `abstand` ist der Abstand der gehaltenen Zustaende (siehe
/// Modulkopf); er aendert keine Zahl.
///
/// # Panics
///
/// Wenn die Eingaben nicht zusammenpassen oder `abstand` null ist.
pub fn kopf_rueckwaerts(folge: &Kopffolge, d_aus: &[Vec<Grad>], abstand: usize) -> Kopfgradienten {
    folge.pruefen();
    assert!(abstand > 0, "der Abstand der gehaltenen Zustaende ist null");
    let n = folge.len();
    assert_eq!(d_aus.len(), n, "ein Ausgangsgradient je Token");
    let (sd, wd) = (folge.schluessel_dim, folge.wert_dim);
    assert!(d_aus.iter().all(|d| d.len() == wd), "ein Ausgangsgradient passt nicht zur Wertdimension");

    // Vorwaerts: nur jeden `abstand`-ten Zustand halten.
    let mut gehalten: Vec<Zustand> = Vec::with_capacity(n.div_ceil(abstand));
    {
        let mut z = Zustand::leer(sd, wd);
        let mut o = vec![0i16; wd];
        for t in 0..n {
            if t % abstand == 0 {
                gehalten.push(z.clone());
            }
            schritt(&mut z, &folge.q[t], &folge.k[t], &folge.v[t], folge.g[t], folge.beta[t], &mut o);
        }
    }

    let mut aus = Kopfgradienten {
        q: vec![Vec::new(); n],
        k: vec![Vec::new(); n],
        v: vec![Vec::new(); n],
        g: vec![0; n],
        beta: vec![0; n],
    };
    // `dS` nach dem Zustand hinter dem gerade behandelten Token, auf
    // Bus * 2^RUECK_EXTRA. Hinter dem letzten Token liest niemand mehr.
    let mut ds = vec![0i64; sd * wd];
    for (abschnitt, anfang) in gehalten.iter().enumerate().rev() {
        let t0 = abschnitt * abstand;
        let t1 = (t0 + abstand).min(n);
        // Den Abschnitt neu rechnen, mit allem, was rueckwaerts gebraucht wird.
        let mut spuren = Vec::with_capacity(t1 - t0);
        let mut z = anfang.clone();
        for t in t0..t1 {
            let davor = z.werte().to_vec();
            let verblasst = verblasst(&z, folge.g[t]);
            let (mut kv, mut delta, mut o) = (vec![0i32; wd], vec![0i32; wd], vec![0i16; wd]);
            schritt_mit_spur(
                &mut z, &folge.q[t], &folge.k[t], &folge.v[t], folge.g[t], folge.beta[t], &mut o, &mut kv, &mut delta,
            );
            spuren.push(Tokenspur { davor, verblasst, danach: z.werte().to_vec(), kv, delta });
        }
        for t in (t0..t1).rev() {
            token_rueckwaerts(folge, t, &spuren[t - t0], &d_aus[t], &mut ds, &mut aus);
        }
    }
    aus
}

/// Ein Token rueckwaerts: liest `dS` nach `S_t`, schreibt die Gradienten
/// des Tokens und hinterlaesst `dS` nach `S_(t-1)`.
fn token_rueckwaerts(
    folge: &Kopffolge,
    t: usize,
    spur: &Tokenspur,
    d_o: &[Grad],
    ds: &mut [i64],
    aus: &mut Kopfgradienten,
) {
    let (sd, wd) = (folge.schluessel_dim, folge.wert_dim);
    let (q, k, v) = (&folge.q[t], &folge.k[t], &folge.v[t]);
    let beta = i64::from(folge.beta[t]);
    let e = RUECK_EXTRA;

    // --- Lesen mit der Abfrage: `dS += q (x) dO`, `dq = S_t . dO`.
    //
    // ⚑ `q` traegt `NORM_FRAC = RUECK_EXTRA` Bruchbits, also landet der
    //   Zuschlag ohne Rundung auf `Bus * 2^RUECK_EXTRA`.
    let mut dq = Vec::with_capacity(sd);
    for a in 0..sd {
        let zeile = &spur.danach[a * wd..(a + 1) * wd];
        let mut summe = 0i128;
        for (s, &d) in zeile.iter().zip(d_o) {
            summe += i128::from(*s) * i128::from(d);
        }
        dq.push(begrenze(rshift_round_i128(summe, ZUSTAND_FRAC) as i64));
        let q_a = i64::from(q[a]);
        for (x, &d) in ds[a * wd..(a + 1) * wd].iter_mut().zip(d_o) {
            *x += q_a * i64::from(d);
        }
    }

    // --- Rang-1-Fortschreibung: `dd = dS^T k` (auf `Bus * 2^e`) und der
    //     erste Teil von `dk = dS . d`.
    let mut dd_roh = vec![0i128; wd];
    let mut dk_roh = vec![0i128; sd];
    for a in 0..sd {
        let zeile = &ds[a * wd..(a + 1) * wd];
        let k_a = i128::from(k[a]);
        let mut summe = 0i128;
        for ((x, ziel), &d) in zeile.iter().zip(dd_roh.iter_mut()).zip(&spur.delta) {
            *ziel += i128::from(*x) * k_a;
            summe += i128::from(*x) * i128::from(d);
        }
        // `dS * d` liegt auf `Bus * 2^(e + INTERN_FRAC)`; auf die Skala
        // des zweiten Teils unten gehoben, `Bus * 2^(e + ZUSTAND_FRAC)`.
        dk_roh[a] = summe << (ZUSTAND_FRAC - INTERN_FRAC);
    }
    let dd: Vec<i64> = dd_roh.iter().map(|&x| rshift_round_i128(x, NORM_FRAC) as i64).collect();

    // --- Die Korrektur: `dv`, `dkv`, `dbeta`.
    let mut dv = Vec::with_capacity(wd);
    let mut dkv = Vec::with_capacity(wd);
    let mut dbeta = 0i128;
    for b in 0..wd {
        dv.push(begrenze(rshift_round_i64(dd[b] * beta, (WERT_FRAC + e) as u8)));
        dkv.push(-rshift_round_i64(dd[b] * beta, WERT_FRAC as u8));
        let fehler = (i64::from(v[b]) << (INTERN_FRAC - WERT_FRAC)) - i64::from(spur.kv[b]);
        dbeta += i128::from(dd[b]) * i128::from(fehler);
    }
    aus.v[t] = dv;
    aus.beta[t] = begrenze(rshift_round_i128(dbeta, INTERN_FRAC + e) as i64);

    // --- Lesen mit dem Schluessel: `dS' = dS + k (x) dkv`, und der zweite
    //     Teil von `dk = S' . dkv`.
    for a in 0..sd {
        let zeile = &spur.verblasst[a * wd..(a + 1) * wd];
        let mut summe = 0i128;
        for (s, &d) in zeile.iter().zip(&dkv) {
            summe += i128::from(*s) * i128::from(d);
        }
        dk_roh[a] += summe;
        let k_a = i64::from(k[a]);
        for (x, &d) in ds[a * wd..(a + 1) * wd].iter_mut().zip(&dkv) {
            *x += rshift_round_i64(k_a * d, NORM_FRAC as u8);
        }
    }
    aus.k[t] = dk_roh.iter().map(|&x| begrenze(rshift_round_i128(x, ZUSTAND_FRAC + e) as i64)).collect();
    aus.q[t] = dq;

    // --- Der Zerfall: `dg = Summe dS' * S_(t-1)`, dann `dS_(t-1) = g * dS'`
    //     mit derselben Rundung wie vorwaerts.
    let mut dg = 0i128;
    for (x, s) in ds.iter().zip(&spur.davor) {
        dg += i128::from(*x) * i128::from(*s);
    }
    aus.g[t] = begrenze(rshift_round_i128(dg, ZUSTAND_FRAC + e) as i64);
    for zeile in ds.chunks_exact_mut(wd.max(1)) {
        zeile_verblassen(zeile, folge.g[t]);
    }
}

/// **Die Gradienten eines Torpaares**, auf dem Bus: nach den reellen
/// Projektionswerten `b` und `a` und nach den beiden Vorspannen des Kopfes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Torgradienten {
    pub b: Grad,
    pub a: Grad,
    /// nach `dt_bias`, gleich dem nach `a`, denn beide gehen als Summe ein
    pub dt_bias: Grad,
    /// nach dem reellen `exp_A`
    pub exp_a: Grad,
}

/// **Die Tore rueckwaerts**, ein Wertkopf, ein Token: aus `dL/dbeta` und
/// `dL/dg` (Bus, nach den reellen Werten) die Gradienten nach `b`, `a`
/// und den beiden Vorspannen.
///
/// ```text
/// dL/db     = dL/dbeta * s * (1 - s)                  s = sigmoid(b)
/// dL/dx     = dL/dg * (-g) * exp_A * sigmoid(x)       x = a + dt_bias
/// dL/dexp_A = dL/dg * (-g) * softplus(x)
/// ```
///
/// ⚑ **Die Ableitung des Softplus ist der Sigmoid**, und der kommt aus
/// derselben Tabelle wie `beta`. ⚑ **Und die des Zerfalls ist `-g` selbst**,
/// aus dem Vorwaertswert; jenseits der Zerfallstabelle ist `g` null, und
/// mit ihm der Gradient, wie es die Funktion dort auch ist.
///
/// ⚠️ **Die Sigmoid-Tabelle saettigt an ihren Raendern** auf null und eins;
/// dort ist `s * (1 - s)` null, und die Ableitung der Tabelle ist es
/// tatsaechlich auch.
pub fn tore_rueckwaerts(tore: &Zustandstore<'_>, h: usize, w: &Torwerte, d_beta: Grad, d_g: Grad) -> Torgradienten {
    let fs = tore.sigmoid_aus_frac;
    let eins = 1i64 << fs;
    // dL/db
    let t1 = rshift_round_i64(i64::from(d_beta) * w.s_b, fs);
    let db = rshift_round_i64(t1 * (eins - w.s_b), fs);
    // -dL/dg * g, auf dem Bus
    let mal_g = -rshift_round_i64(i64::from(d_g) * w.g, tore.zerfall_aus_frac);
    // sigmoid(x) aus derselben Tabelle wie `beta`
    let x_dom = rescale(w.x, tore.softplus_ein_frac, tore.sigmoid_ein_frac);
    let s_x = sigmoid_nachschlagen(x_dom, tore.sigmoid_lut, tore.sigmoid_versatz, tore.sigmoid_ein_frac, fs);
    let mal_a = rshift_round_i128(i128::from(mal_g) * i128::from(tore.exp_a[h]), u32::from(tore.exp_a_shifts[h]));
    let dx = begrenze(rshift_round_i128(mal_a * i128::from(s_x), u32::from(fs)) as i64);
    let d_exp_a = rshift_round_i128(i128::from(mal_g) * i128::from(w.sp), u32::from(tore.softplus_aus_frac));
    Torgradienten { b: begrenze(db), a: dx, dt_bias: dx, exp_a: begrenze(d_exp_a as i64) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zustandsschicht::ZERFALL_FRAC;

    const SD: usize = 16;
    const WD: usize = 16;

    /// Ein einfacher, fester Zufall (xorshift), damit die Probe ueberall
    /// dieselben Zahlen sieht.
    struct Zufall(u64);
    impl Zufall {
        fn zahl(&mut self, von: i64, bis: i64) -> i64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            von + (self.0 % (bis - von + 1) as u64) as i64
        }
    }

    fn folge(n: usize, saat: u64) -> Kopffolge {
        let mut z = Zufall(saat);
        let mut vektor = |breite: usize, spanne: i64| -> Vec<i16> {
            (0..breite).map(|_| z.zahl(-spanne, spanne) as i16).collect()
        };
        let q = (0..n).map(|_| vektor(SD, 1400)).collect();
        let k = (0..n).map(|_| vektor(SD, 1400)).collect();
        let v = (0..n).map(|_| vektor(WD, 200)).collect();
        let mut z = Zufall(saat ^ 0x9e37_79b9);
        // Zerfaelle zwischen 0,6 und 0,99, beta zwischen 0,25 und 0,9.
        let g = (0..n).map(|_| z.zahl((1 << 28) * 6 / 10, (1 << 28) * 99 / 100)).collect();
        let beta = (0..n).map(|_| z.zahl(64, 230) as i16).collect();
        Kopffolge { schluessel_dim: SD, wert_dim: WD, q, k, v, g, beta }
    }

    /// Die Gewichte des Verlusts, zugleich `dO` auf dem Bus.
    ///
    /// ⚠️ **Gross, mit Absicht.** Ein Gradient wird am Ende auf eine
    /// Bus-Stelle gerundet; bei Gewichten um tausend lag `dv` bei wenigen
    /// Dutzend Zaehlern, und in einer zufaelligen Richtung kostete diese
    /// Rundung bis zu fuenf Prozent der Vorhersage. Das ist eine Frage der
    /// Busskala und nicht der Ableitung.
    fn gewichte(n: usize, saat: u64) -> Vec<Vec<Grad>> {
        let mut z = Zufall(saat);
        (0..n).map(|_| (0..WD).map(|_| z.zahl(-(1 << 20), 1 << 20) as Grad).collect()).collect()
    }

    /// `L = Summe w * o`, reell, mit `o` aus dem Vorwaertspass.
    fn verlust(f: &Kopffolge, w: &[Vec<Grad>]) -> f64 {
        let (aus, skalen) = f.vorwaerts();
        let mut l = 0.0f64;
        for ((o, &s), w) in aus.iter().zip(&skalen).zip(w) {
            let teil: i64 = o.iter().zip(w).map(|(a, b)| i64::from(*a) * i64::from(*b)).sum();
            l += teil as f64 / 2f64.powi(i32::from(s));
        }
        l
    }

    #[derive(Clone, Copy, Debug)]
    enum Teil {
        Q,
        K,
        V,
        G,
        Beta,
    }

    /// **Die Vorhersage stimmt mit der gemessenen Aenderung**: je Eingang
    /// ein Schritt, vorhergesagt `Summe grad * Schritt`, gemessen am echten
    /// ganzzahligen Vorwaertspass.
    ///
    /// ⚑ **Zwei Richtungen, und beide sind noetig.** Gegen den Gradienten
    /// zeigt, dass er abwaerts fuehrt; aber ein fehlender Summand faellt
    /// dort kaum auf, denn das Verhaeltnis ist dann `<wahr, falsch> /
    /// <falsch, falsch>`, und ein kleiner, fast senkrechter Rest verschiebt
    /// es kaum. 📌 So blieb in der ersten Fassung ein Gradient von `k` ohne
    /// seinen zweiten Summanden gruen. In einer **zufaelligen** Richtung
    /// zaehlt jeder Summand fuer sich.
    fn verhaeltnisse(f: &Kopffolge, w: &[Vec<Grad>], gr: &Kopfgradienten, zufaellig: bool) -> Vec<(Teil, f64)> {
        let l0 = verlust(f, w);
        let betrag: f64 = {
            let (aus, skalen) = f.vorwaerts();
            aus.iter()
                .zip(&skalen)
                .zip(w)
                .map(|((o, &s), w)| {
                    o.iter().zip(w).map(|(a, b)| (i64::from(*a) * i64::from(*b)).abs() as f64).sum::<f64>()
                        / 2f64.powi(i32::from(s))
                })
                .sum()
        };
        let deckel = betrag / 100.0;
        let mut ergebnis = Vec::new();
        for teil in [Teil::Q, Teil::K, Teil::V, Teil::G, Teil::Beta] {
            // Alle Eintraege des Teils als (Gradient, Bruchbits) flach.
            let grads: Vec<i64> = match teil {
                Teil::Q => gr.q.iter().flatten().map(|&x| i64::from(x)).collect(),
                Teil::K => gr.k.iter().flatten().map(|&x| i64::from(x)).collect(),
                Teil::V => gr.v.iter().flatten().map(|&x| i64::from(x)).collect(),
                Teil::G => gr.g.iter().map(|&x| i64::from(x)).collect(),
                Teil::Beta => gr.beta.iter().map(|&x| i64::from(x)).collect(),
            };
            let frac = match teil {
                Teil::Q | Teil::K => NORM_FRAC,
                Teil::V | Teil::Beta => WERT_FRAC,
                Teil::G => ZERFALL_FRAC,
            } as i32;
            let richtung: Vec<i64> = if zufaellig {
                let mut z = Zufall(0x5eed ^ grads.len() as u64);
                grads.iter().map(|_| z.zahl(-1000, 1000)).collect()
            } else {
                grads.iter().map(|g| -g).collect()
            };
            let spitze = richtung.iter().map(|g| g.abs()).max().unwrap_or(0).max(1);
            // Der groesste Schritt je Eintrag. ⚑ `q` und `v` gehen linear in
            // den Ausgang ein (bei festem `k`, `beta`, `g`) und duerfen
            // deshalb weit gehen, was das Rundungsrauschen des Ausgangs
            // klein haelt; die uebrigen bleiben bei rund zwei Prozent ihrer
            // Spanne, darueber zaehlt die Kruemmung mit.
            let obergrenze: i64 = match teil {
                Teil::Q => 256,
                Teil::V => 64,
                Teil::K => 32,
                Teil::Beta => 4,
                Teil::G => 1 << 22,
            };
            let schritt_mit = |weite: i64, vorzeichen: i64| -> (Kopffolge, f64) {
                let deltas: Vec<i64> = richtung.iter().map(|g| vorzeichen * (g * weite / spitze)).collect();
                let vorher: f64 =
                    grads.iter().zip(&deltas).map(|(g, d)| (*g as f64) * (*d as f64)).sum::<f64>() / 2f64.powi(frac);
                let mut neu = f.clone();
                let mut it = deltas.iter();
                let mut auf16 = |x: &mut i16| *x = (i64::from(*x) + it.next().unwrap()).clamp(-32768, 32767) as i16;
                match teil {
                    Teil::Q => neu.q.iter_mut().flatten().for_each(&mut auf16),
                    Teil::K => neu.k.iter_mut().flatten().for_each(&mut auf16),
                    Teil::V => neu.v.iter_mut().flatten().for_each(&mut auf16),
                    Teil::Beta => neu.beta.iter_mut().for_each(&mut auf16),
                    Teil::G => {
                        for (x, d) in neu.g.iter_mut().zip(&deltas) {
                            *x = (*x + d).clamp(0, 1 << 28);
                        }
                    }
                }
                (neu, vorher)
            };
            let mut weite = 1i64;
            while weite < obergrenze && schritt_mit(2 * weite, 1).1.abs() <= deckel {
                weite *= 2;
            }
            // ⚑ **Mittig gemessen**, `(L(x + d) - L(x - d)) / 2`: Der Anteil
            //   der Kruemmung hebt sich auf, und was bleibt, ist die
            //   Ableitung bis auf dritte Ordnung.
            let (neu, vorher) = schritt_mit(weite, 1);
            let (gegen, _) = schritt_mit(weite, -1);
            let gemessen = (verlust(&neu, w) - verlust(&gegen, w)) / 2.0;
            let _ = l0;
            assert!(zufaellig || vorher < 0.0, "{teil:?}: der Schritt sagt keine Senkung voraus ({vorher})");
            ergebnis.push((teil, gemessen / vorher));
        }
        ergebnis
    }

    #[test]
    fn die_rekurrenz_sagt_die_aenderung_voraus() {
        for saat in [3u64, 17, 91] {
            let n = 12;
            let f = folge(n, saat);
            let w = gewichte(n, saat * 7 + 1);
            let gr = kopf_rueckwaerts(&f, &w, 4);
            for zufaellig in [false, true] {
                for (teil, r) in verhaeltnisse(&f, &w, &gr, zufaellig) {
                    let art = if zufaellig { "zufaellig" } else { "abwaerts" };
                    eprintln!("[zustandsrueckweg] Saat {saat} {art} {teil:?}: gemessen / vorhergesagt = {r:.3}");
                    assert!((0.97..=1.03).contains(&r), "Saat {saat}, {art}, {teil:?}: das {r:.3}-fache der Vorhersage");
                }
            }
        }
    }

    /// Tabellen wie die des Modells, fuer die Probe der Tore.
    struct Tabellen {
        sigmoid: Vec<i16>,
        rest: Vec<i32>,
        zerfall: Vec<i32>,
    }

    fn tabellen() -> Tabellen {
        let sigmoid = (0..2 * 8192)
            .map(|i| {
                let x = (i as f64 - 8192.0) / 512.0;
                (1.0 / (1.0 + (-x).exp()) * 16384.0).round() as i16
            })
            .collect();
        let rest = (0..8192).map(|i| ((-(i as f64) / 256.0).exp().ln_1p() * 2f64.powi(30)).round() as i32).collect();
        let zerfall = (0..64 * 256).map(|i| ((-(i as f64) / 256.0).exp() * 2f64.powi(28)).round() as i32).collect();
        Tabellen { sigmoid, rest, zerfall }
    }

    fn tore<'a>(tab: &'a Tabellen, dt: &'a [i16], dt_s: &'a [u8], ea: &'a [i16], ea_s: &'a [u8]) -> Zustandstore<'a> {
        Zustandstore {
            sigmoid_lut: &tab.sigmoid,
            sigmoid_versatz: 8192,
            sigmoid_ein_frac: 9,
            sigmoid_aus_frac: 14,
            b_frac: 10,
            a_frac: 10,
            softplus_rest: &tab.rest,
            softplus_ein_frac: 8,
            softplus_aus_frac: 30,
            zerfall_exp: &tab.zerfall,
            zerfall_raster_frac: 8,
            zerfall_aus_frac: ZERFALL_FRAC as u8,
            dt_bias: dt,
            dt_bias_shifts: dt_s,
            exp_a: ea,
            exp_a_shifts: ea_s,
        }
    }

    /// ⚑ **Die Tore sagen die Aenderung voraus**, je Groesse in einer
    /// zufaelligen Richtung, mittig gemessen. Viele Koepfe, damit sich die
    /// Stufen der Tabellen ausmitteln: Eine Tabelle ist stueckweise
    /// konstant, und erst ueber viele Rasterschritte ist ihre Steigung die
    /// der Funktion.
    #[test]
    fn die_tore_sagen_die_aenderung_voraus() {
        const H: usize = 4096;
        let tab = tabellen();
        let mut z = Zufall(42);
        let b: Vec<i16> = (0..H).map(|_| z.zahl(-4096, 4096) as i16).collect();
        let a: Vec<i16> = (0..H).map(|_| z.zahl(-4096, 4096) as i16).collect();
        let dt: Vec<i16> = (0..H).map(|_| z.zahl(-8192, 8192) as i16).collect();
        let dt_s = vec![12u8; H];
        // `exp_A` zwischen 0,05 und 4, damit `g` ueber die ganze Spanne geht.
        let ea: Vec<i16> = (0..H).map(|_| z.zahl(51, 4096) as i16).collect();
        let ea_s = vec![10u8; H];
        let wb: Vec<i64> = (0..H).map(|_| z.zahl(-(1 << 20), 1 << 20)).collect();
        let wg: Vec<i64> = (0..H).map(|_| z.zahl(-(1 << 20), 1 << 20)).collect();
        let verlust = |b: &[i16], a: &[i16], dt: &[i16], ea: &[i16]| -> f64 {
            let tv = tore(&tab, dt, &dt_s, ea, &ea_s);
            (0..H)
                .map(|h| {
                    let w = tv.werte(h, b[h], a[h]);
                    (wb[h] * i64::from(w.beta)) as f64 / 256.0 + (wg[h] as f64) * (w.g as f64) / 2f64.powi(28)
                })
                .sum()
        };
        let tv = tore(&tab, &dt, &dt_s, &ea, &ea_s);
        let gr: Vec<Torgradienten> = (0..H)
            .map(|h| tore_rueckwaerts(&tv, h, &tv.werte(h, b[h], a[h]), wb[h] as Grad, wg[h] as Grad))
            .collect();
        assert!(gr.iter().any(|g| g.a != 0) && gr.iter().any(|g| g.b != 0), "die Tore geben keinen Gradienten");

        let schritt: Vec<i64> = (0..H).map(|_| z.zahl(-1, 1) * 192).collect();
        let verschoben = |x: &[i16], vz: i64| -> Vec<i16> {
            x.iter().zip(&schritt).map(|(w, d)| (i64::from(*w) + vz * d) as i16).collect()
        };
        let faelle: [(&str, f64, f64); 4] = [
            (
                "b",
                gr.iter().zip(&schritt).map(|(g, d)| f64::from(g.b) * *d as f64).sum::<f64>() / 1024.0,
                verlust(&verschoben(&b, 1), &a, &dt, &ea) - verlust(&verschoben(&b, -1), &a, &dt, &ea),
            ),
            (
                "a",
                gr.iter().zip(&schritt).map(|(g, d)| f64::from(g.a) * *d as f64).sum::<f64>() / 1024.0,
                verlust(&b, &verschoben(&a, 1), &dt, &ea) - verlust(&b, &verschoben(&a, -1), &dt, &ea),
            ),
            (
                "dt_bias",
                gr.iter().zip(&schritt).map(|(g, d)| f64::from(g.dt_bias) * *d as f64).sum::<f64>() / 4096.0,
                verlust(&b, &a, &verschoben(&dt, 1), &ea) - verlust(&b, &a, &verschoben(&dt, -1), &ea),
            ),
            (
                "exp_A",
                gr.iter().zip(&schritt).map(|(g, d)| f64::from(g.exp_a) * (*d as f64 / 16.0)).sum::<f64>() / 1024.0,
                {
                    let klein: Vec<i64> = schritt.iter().map(|d| d / 16).collect();
                    let um = |vz: i64| -> Vec<i16> {
                        ea.iter().zip(&klein).map(|(w, d)| (i64::from(*w) + vz * d) as i16).collect()
                    };
                    verlust(&b, &a, &dt, &um(1)) - verlust(&b, &a, &dt, &um(-1))
                },
            ),
        ];
        for (name, vorher, doppelt) in faelle {
            let r = doppelt / 2.0 / vorher;
            eprintln!("[zustandsrueckweg] Tore {name}: gemessen / vorhergesagt = {r:.3}");
            assert!((0.97..=1.03).contains(&r), "Tore {name}: das {r:.3}-fache der Vorhersage");
        }
    }

    /// ⚑ **Die beiden Normen der Schicht rueckwaerts**, ueber den
    /// vorhandenen `rmsnorm_backward` und in den beiden Lagen, die es dort
    /// bisher nicht gab:
    ///
    /// 1. **Einheitslaenge mit `inv_n = d`** (fuer `q`): `r` haengt dann an
    ///    `d * Summe x^2` statt am Mittel.
    /// 2. **Die torgesteuerte Norm mit Epsilon**, an einem Kopf, der so
    ///    leise ist, dass das Epsilon den Nenner mitbestimmt (Fund 419).
    ///
    /// Beide gegen den echten Vorwaertskern, mittig gemessen, in einer
    /// zufaelligen Richtung.
    #[test]
    fn die_normen_der_zustandsschicht_sagen_die_aenderung_voraus() {
        use crate::backward::{rmsnorm_backward, Normskalen};
        use crate::rmsnorm::{rmsnorm_i16_mit_eps_und_spur, Rmsnormspur};
        const D: usize = 64;
        let lut: Vec<i16> = (0..4096)
            .map(|i| {
                let x = (i as f64) * 16.0;
                if x <= 0.0 { 0 } else { ((1.0 / x.sqrt()) * 4096.0).min(32767.0) as i16 }
            })
            .collect();
        let mut z = Zufall(77);
        // (Name, x_frac, Spanne von x, gamma, inv_n, eps, aus_frac)
        let gamma_eins = (vec![1i8; D], vec![0u8; D]);
        let gamma_echt: (Vec<i8>, Vec<u8>) = ((0..D).map(|_| z.zahl(40, 100) as i8).collect(), vec![6u8; D]);
        let faelle = [
            // ⚠️ **Die Ausgabe hier auf 18 Bruchbits statt auf `NORM_FRAC`.**
            //   Mit `inv_n = d` liegt jede Komponente um `1/d`, auf zwoelf
            //   Bruchbits also bei rund 64 Einheiten, und eine Aenderung um
            //   ein Prozent verschwaende in der Rundung. Die Ableitung haengt
            //   an der Ausgabeskala nicht; gemessen wird sie dort, wo die
            //   Ausgabe sie aufloest.
            ("Einheitslaenge q", 12u8, 3000i64, &gamma_eins, (D as i64) << 20, 0i64, 18u8),
            ("Einheitslaenge k", 12, 3000, &gamma_eins, 1 << 20, 0, 18),
            ("Norm mit Epsilon", 18, 300, &gamma_echt, crate::rmsnorm::inv_n_q20(D), 1_099_512, 12),
        ];
        for (name, x_frac, spanne, gamma, inv_n, eps, aus_frac) in faelle {
            let x: Vec<i16> = (0..D).map(|_| z.zahl(-spanne, spanne) as i16).collect();
            let x_shifts = vec![x_frac; D];
            let w: Vec<i64> = (0..D).map(|_| z.zahl(-(1 << 20), 1 << 20)).collect();
            let vorwaerts = |x: &[i16], spur: Option<&mut Rmsnormspur>| {
                rmsnorm_i16_mit_eps_und_spur(x, &x_shifts, &gamma.0, &gamma.1, &lut, 4, 12, inv_n, aus_frac, eps, spur)
            };
            let verlust = |x: &[i16]| -> f64 {
                vorwaerts(x, None).iter().zip(&w).map(|(a, b)| (i64::from(*a) * b) as f64).sum::<f64>()
                    / 2f64.powi(i32::from(aus_frac))
            };
            let mut spur = Rmsnormspur::Leer;
            vorwaerts(&x, Some(&mut spur));
            let Rmsnormspur::Wert { r, norm_frac, ref_shift } = spur else { panic!("{name}: keine Spur") };
            let g: Vec<Grad> = w.iter().map(|&v| v as Grad).collect();
            let (gx, _) = rmsnorm_backward(
                &g, &x, &x_shifts, &gamma.0, &gamma.1,
                Normskalen { r, norm_frac, ref_shift, inv_n_q20: inv_n, g_frac: 0, gx_frac: 0 },
            );
            // ⚑ **Gegen den Gradienten und nicht zufaellig.** Die Jacobi-Matrix
            //   einer Einheitslaengen-Norm steht fast senkrecht auf `x`; eine
            //   zufaellige Richtung misst dort vor allem Ausloeschung.
            let spitze = gx.iter().map(|g| i64::from(*g).abs()).max().unwrap_or(1).max(1);
            let schritt: Vec<i64> = gx.iter().map(|g| -i64::from(*g) * (spanne / 20) / spitze).collect();
            let um = |vz: i64| -> Vec<i16> { x.iter().zip(&schritt).map(|(a, d)| (i64::from(*a) + vz * d) as i16).collect() };
            let vorher: f64 =
                gx.iter().zip(&schritt).map(|(g, d)| f64::from(*g) * *d as f64).sum::<f64>() / 2f64.powi(i32::from(x_frac));
            let gemessen = (verlust(&um(1)) - verlust(&um(-1))) / 2.0;
            let r = gemessen / vorher;
            eprintln!("[zustandsrueckweg] {name}: gemessen / vorhergesagt = {r:.3}");
            assert!((0.97..=1.03).contains(&r), "{name}: das {r:.3}-fache der Vorhersage");
        }
    }

    #[test]
    fn der_abstand_aendert_keine_zahl() {
        let n = 10;
        let f = folge(n, 5);
        let w = gewichte(n, 6);
        let eins = kopf_rueckwaerts(&f, &w, 1);
        for abstand in [2, 3, 4, 10, 64] {
            assert_eq!(kopf_rueckwaerts(&f, &w, abstand), eins, "Abstand {abstand}");
        }
    }

    #[test]
    fn die_spur_aendert_den_schritt_nicht() {
        let f = folge(6, 11);
        let (aus, skalen) = f.vorwaerts();
        let mut z = Zustand::leer(SD, WD);
        for t in 0..f.len() {
            let (mut kv, mut d, mut o) = (vec![0i32; WD], vec![0i32; WD], vec![0i16; WD]);
            let s = schritt_mit_spur(&mut z, &f.q[t], &f.k[t], &f.v[t], f.g[t], f.beta[t], &mut o, &mut kv, &mut d);
            assert_eq!((o, s), (aus[t].clone(), skalen[t]), "Token {t}");
            assert!(d.iter().any(|&x| x != 0), "Token {t}: die Spur bleibt leer");
        }
    }
}
