//! **Das Spektrum**: Log-Mel wie whisper, ohne fremde Laufzeit.
//!
//! Der erste gemeinsame Baustein der Sinne (CLIENT 8.0, Baustein S): Hoeren
//! braucht das Log-Mel als Eingang des Encoders, Sprechen spaeter dasselbe
//! Spektrum rueckwaerts im Vocoder. Ein Modul fuer beide Richtungen.
//!
//! # ⚑ Was hier genau gerechnet wird
//!
//! Wie `WhisperFeatureExtractor` aus `transformers` (Vorlage, nicht
//! Baustein; neu geschrieben):
//!
//! 1. Der Ton (16 kHz, einkanalig) wird mit Nullen auf 30 s aufgefuellt
//!    oder abgeschnitten.
//! 2. Zentriert: 200 Proben Spiegelrand vorn und hinten, dann Fenster zu
//!    400 Proben im Schritt von 160, ein Hann-Fenster (periodisch).
//! 3. Leistungsspektrum der 201 Frequenzen, Mel-Filter nach Slaney von 0
//!    bis 8 kHz, mit Slaney-Normierung.
//! 4. `log10` (unten bei 1e-10 abgeschnitten), alles unter dem Maximum
//!    minus 8 auf diesen Wert, dann `(x + 4) / 4`. Das letzte Fenster
//!    entfaellt, es bleiben 3000.
//!
//! ⚑ **Gleitkomma ist hier erlaubt**: Die Sinne stehen nicht im
//! Rechenpfad. Gerechnet wird in `f64`, ausgegeben in `f32`; die Referenz
//! rechnet in `f32`, die Probe vergleicht mit einer Schranke.
//!
//! # Die Fourier-Transformation
//!
//! Eigener Cooley-Tukey mit gemischter Basis: 400 = 2⁴·5², zerlegt nach
//! dem kleinsten Faktor, ein Faktor ohne Zerlegung faellt auf die direkte
//! Summe zurueck. Fuer 400 Punkte reicht das; eine schnellere Fassung und
//! die Grafikeinheit kommen mit der gemeinsamen Rechenschicht.

/// Abtastrate, die whisper erwartet.
pub const ABTASTRATE: usize = 16_000;
/// Fensterlaenge in Proben.
pub const FENSTER: usize = 400;
/// Schritt zwischen zwei Fenstern.
pub const SCHRITT: usize = 160;
/// Proben in einem Stueck von 30 s.
pub const STUECK: usize = 30 * ABTASTRATE;
/// Fenster je Stueck nach dem Abschneiden des letzten.
pub const FENSTER_JE_STUECK: usize = STUECK / SCHRITT;
/// Frequenzen des Leistungsspektrums.
pub const FREQUENZEN: usize = FENSTER / 2 + 1;

/// Eine komplexe Zahl.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Komplex {
    pub re: f64,
    pub im: f64,
}

impl Komplex {
    fn mal(self, o: Komplex) -> Komplex {
        Komplex { re: self.re * o.re - self.im * o.im, im: self.re * o.im + self.im * o.re }
    }
    fn plus(self, o: Komplex) -> Komplex {
        Komplex { re: self.re + o.re, im: self.im + o.im }
    }
    fn einheit(winkel: f64) -> Komplex {
        Komplex { re: winkel.cos(), im: winkel.sin() }
    }
}

/// Die diskrete Fourier-Transformation `X[k] = Σ x[n]·e^(−2πi·kn/N)`.
pub fn fft(x: &[Komplex]) -> Vec<Komplex> {
    let n = x.len();
    if n <= 1 {
        return x.to_vec();
    }
    let Some(p) = (2..=n).find(|p| n.is_multiple_of(*p) && (*p <= 5 || *p == n)) else {
        return direkt(x);
    };
    if p == n {
        return direkt(x);
    }
    let m = n / p;
    // p Teilfolgen x[r], x[r+p], x[r+2p], …, je der Laenge m.
    let teile: Vec<Vec<Komplex>> = (0..p).map(|r| fft(&x.iter().skip(r).step_by(p).copied().collect::<Vec<_>>())).collect();
    let mut aus = vec![Komplex { re: 0.0, im: 0.0 }; n];
    let basis = -2.0 * std::f64::consts::PI / n as f64;
    for (k, ziel) in aus.iter_mut().enumerate() {
        let mut s = Komplex { re: 0.0, im: 0.0 };
        for (r, teil) in teile.iter().enumerate() {
            s = s.plus(teil[k % m].mal(Komplex::einheit(basis * (r * k) as f64)));
        }
        *ziel = s;
    }
    aus
}

fn direkt(x: &[Komplex]) -> Vec<Komplex> {
    let n = x.len();
    let basis = -2.0 * std::f64::consts::PI / n as f64;
    (0..n)
        .map(|k| {
            x.iter().enumerate().fold(Komplex { re: 0.0, im: 0.0 }, |s, (j, v)| {
                s.plus(v.mal(Komplex::einheit(basis * ((j * k) % n) as f64)))
            })
        })
        .collect()
}

/// Hann-Fenster, periodisch: `0,5 − 0,5·cos(2πn/N)`.
pub fn hann(n: usize) -> Vec<f64> {
    (0..n).map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos()).collect()
}

fn hertz_zu_mel(f: f64) -> f64 {
    if f >= 1000.0 {
        15.0 + (f / 1000.0).ln() * (27.0 / 6.4f64.ln())
    } else {
        3.0 * f / 200.0
    }
}

fn mel_zu_hertz(m: f64) -> f64 {
    if m >= 15.0 {
        1000.0 * ((m - 15.0) * (6.4f64.ln() / 27.0)).exp()
    } else {
        200.0 * m / 3.0
    }
}

/// Die Mel-Filter nach Slaney, `FREQUENZEN × baender`, zeilenweise je
/// Frequenz.
pub fn mel_filter(baender: usize) -> Vec<f64> {
    mel_filter_fuer(FREQUENZEN, 8_000.0, baender)
}

/// Die Mel-Filter nach Slaney fuer `frequenzen` Stuetzstellen von 0 bis
/// `oben` Hertz (die halbe Abtastrate), Baender von 0 bis `oben`.
pub fn mel_filter_fuer(frequenzen: usize, oben_hz: f64, baender: usize) -> Vec<f64> {
    let (unten, oben) = (hertz_zu_mel(0.0), hertz_zu_mel(oben_hz));
    let mitten: Vec<f64> = (0..baender + 2)
        .map(|i| mel_zu_hertz(unten + (oben - unten) * i as f64 / (baender + 1) as f64))
        .collect();
    let mut f = vec![0.0; frequenzen * baender];
    for k in 0..frequenzen {
        // Wie die Vorlage: die Frequenzen gleichmaessig von 0 bis `oben`.
        let hz = oben_hz * k as f64 / (frequenzen - 1) as f64;
        for m in 0..baender {
            let ab = (hz - mitten[m]) / (mitten[m + 1] - mitten[m]);
            let auf = (mitten[m + 2] - hz) / (mitten[m + 2] - mitten[m + 1]);
            let dreieck = ab.min(auf).max(0.0);
            f[k * baender + m] = dreieck * 2.0 / (mitten[m + 2] - mitten[m]);
        }
    }
    f
}

/// Ein Log-Mel: `baender` Zeilen zu je [`FENSTER_JE_STUECK`] Werten.
#[derive(Debug, Clone, PartialEq)]
pub struct LogMel {
    pub baender: usize,
    pub werte: Vec<f32>,
}

impl LogMel {
    /// Der Wert von Band `m` in Fenster `t`.
    pub fn wert(&self, m: usize, t: usize) -> f32 {
        self.werte[m * FENSTER_JE_STUECK + t]
    }
}

/// **Das Log-Mel eines Stuecks**, genau wie whisper es erwartet.
pub fn log_mel(ton: &[f32], baender: usize) -> LogMel {
    let mut x: Vec<f64> = ton.iter().take(STUECK).map(|v| f64::from(*v)).collect();
    x.resize(STUECK, 0.0);
    // Zentriert mit Spiegelrand (ohne den Randwert zu wiederholen).
    let rand = FENSTER / 2;
    let n = x.len();
    let probe = |i: isize| -> f64 {
        let j = if i < 0 { -i } else if i as usize >= n { 2 * (n as isize - 1) - i } else { i };
        x[j as usize]
    };
    let fenster = hann(FENSTER);
    let filter = mel_filter(baender);
    let mut mel = vec![0.0f64; baender * FENSTER_JE_STUECK];
    let mut rahmen = vec![Komplex { re: 0.0, im: 0.0 }; FENSTER];
    for t in 0..FENSTER_JE_STUECK {
        let start = (t * SCHRITT) as isize - rand as isize;
        for (i, r) in rahmen.iter_mut().enumerate() {
            *r = Komplex { re: probe(start + i as isize) * fenster[i], im: 0.0 };
        }
        let spektrum = fft(&rahmen);
        for (k, z) in spektrum.iter().take(FREQUENZEN).enumerate() {
            let leistung = z.re * z.re + z.im * z.im;
            for m in 0..baender {
                mel[m * FENSTER_JE_STUECK + t] += filter[k * baender + m] * leistung;
            }
        }
    }
    let log: Vec<f64> = mel.iter().map(|v| v.max(1e-10).log10()).collect();
    let hoechst = log.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let werte = log.iter().map(|v| ((v.max(hoechst - 8.0) + 4.0) / 4.0) as f32).collect();
    LogMel { baender, werte }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zufall(n: usize) -> Vec<Komplex> {
        let mut s = 7u64;
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let a = ((s >> 33) as f64 / (1u64 << 31) as f64) - 0.5;
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let b = ((s >> 33) as f64 / (1u64 << 31) as f64) - 0.5;
                Komplex { re: a, im: b }
            })
            .collect()
    }

    /// Die zerlegte Transformation gleicht der direkten Summe, fuer die
    /// Laenge, die whisper braucht, und fuer Laengen mit anderen Faktoren.
    #[test]
    fn die_fft_gleicht_der_direkten_summe() {
        for n in [1usize, 2, 3, 5, 7, 12, 25, 49, 400] {
            let x = zufall(n);
            let a = fft(&x);
            let b = direkt(&x);
            for (u, v) in a.iter().zip(&b) {
                assert!((u.re - v.re).abs() < 1e-9 && (u.im - v.im).abs() < 1e-9, "n = {n}");
            }
        }
    }

    /// Jedes Filter hat Gewicht, und keines reicht ueber 8 kHz hinaus.
    #[test]
    fn die_filter_decken_das_band() {
        for baender in [80, 128] {
            let f = mel_filter(baender);
            for m in 0..baender {
                assert!((0..FREQUENZEN).any(|k| f[k * baender + m] > 0.0), "Band {m} von {baender} ist leer");
            }
        }
    }
}
