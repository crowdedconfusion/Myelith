//! Der Skalarkoerper von BLS12-381: Zahlen modulo der Gruppenordnung `r`.
//!
//! ⚑ **Wozu** (2026-11-03): Die Schwellensignatur ([`crate::schwelle`])
//! rechnet mit Polynomen ueber diesem Koerper: Anteile auswerten, Anteile
//! summieren, Lagrange-Gewichte bilden. `blst` hat diese Rechnung, aber nur
//! hinter `unsafe`, und dieses Crate verbietet `unsafe`. Also steht sie
//! hier, in sicherem Rust.
//!
//! ⚑ **In konstanter Zeit.** Polynomkoeffizienten und Anteile sind geheim.
//! Addition, Subtraktion und Multiplikation verzweigen nie nach dem Wert,
//! die Korrektur nach einem Ueberlauf geschieht ueber eine Maske. Das
//! Invertieren laeuft ueber den festen Exponenten `r − 2` und haengt damit
//! nur an der Bitfolge von `r`, nicht an der Eingabe.
//!
//! **Darstellung:** Montgomery-Form mit vier 64-Bit-Gliedern, wie bei
//! jeder verbreiteten Umsetzung dieser Kurve. Vorlage fuer die Reduktion
//! war die bekannte Montgomery-Reduktion nach Koc, Acar, Kaliski (CIOS
//! getrennt in Produkt und Reduktion); geschrieben ist sie hier neu.
//!
//! 📌 **Geprueft gegen `blst`, nicht nur gegen sich selbst.** Die Tests
//! vergleichen ueber den Gruppenhomomorphismus: `pk(a + b)` muss
//! `pk(a) + pk(b)` sein und `pk(a · b)` das `b`-fache von `pk(a)`, beides
//! von `blst` gerechnet. Eine falsche Konstante faellt dabei sofort auf.

use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};

/// Die Gruppenordnung `r`, Glieder von niedrig nach hoch.
const MODUL: [u64; 4] = [
    0xffff_ffff_0000_0001,
    0x53bd_a402_fffe_5bfe,
    0x3339_d808_09a1_d805,
    0x73ed_a753_299d_7d48,
];

/// `−r⁻¹ mod 2⁶⁴`.
const INV: u64 = 0xffff_fffe_ffff_ffff;

/// `2²⁵⁶ mod r`, die Eins in Montgomery-Form.
const R: [u64; 4] = [
    0x0000_0001_ffff_fffe,
    0x5884_b7fa_0003_4802,
    0x998c_4fef_ecbc_4ff5,
    0x1824_b159_acc5_056f,
];

/// `2⁵¹² mod r`, zum Hineinrechnen in die Montgomery-Form.
const R2: [u64; 4] = [
    0xc999_e990_f3f2_9c6d,
    0x2b6c_edcb_8792_5c23,
    0x05d3_1496_7254_398f,
    0x0748_d9d9_9f59_ff11,
];

/// `r − 2`, der Exponent fuer das Inverse nach Fermat.
const MODUL_MINUS_2: [u64; 4] = [
    0xffff_fffe_ffff_ffff,
    0x53bd_a402_fffe_5bfe,
    0x3339_d808_09a1_d805,
    0x73ed_a753_299d_7d48,
];

#[inline(always)]
fn adc(a: u64, b: u64, uebertrag: u64) -> (u64, u64) {
    let s = (a as u128) + (b as u128) + (uebertrag as u128);
    (s as u64, (s >> 64) as u64)
}

#[inline(always)]
fn sbb(a: u64, b: u64, borgen: u64) -> (u64, u64) {
    let d = (a as u128).wrapping_sub((b as u128) + ((borgen >> 63) as u128));
    (d as u64, (d >> 64) as u64)
}

#[inline(always)]
fn mac(a: u64, b: u64, c: u64, uebertrag: u64) -> (u64, u64) {
    let s = (a as u128) + (b as u128) * (c as u128) + (uebertrag as u128);
    (s as u64, (s >> 64) as u64)
}

/// Ein Element des Skalarkoerpers, intern in Montgomery-Form.
///
/// Bewusst ohne `Debug` mit Inhalt: Ein Skalar ist oft ein Geheimnis, und
/// ein Geheimnis gehoert in kein Protokoll.
#[derive(Clone, Copy)]
pub struct Skalar([u64; 4]);

impl std::fmt::Debug for Skalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Skalar(..)")
    }
}

impl ConstantTimeEq for Skalar {
    fn ct_eq(&self, other: &Self) -> Choice {
        self.0[0].ct_eq(&other.0[0])
            & self.0[1].ct_eq(&other.0[1])
            & self.0[2].ct_eq(&other.0[2])
            & self.0[3].ct_eq(&other.0[3])
    }
}

impl PartialEq for Skalar {
    fn eq(&self, other: &Self) -> bool {
        bool::from(self.ct_eq(other))
    }
}

impl Eq for Skalar {}

impl ConditionallySelectable for Skalar {
    fn conditional_select(a: &Self, b: &Self, wahl: Choice) -> Self {
        Skalar([
            u64::conditional_select(&a.0[0], &b.0[0], wahl),
            u64::conditional_select(&a.0[1], &b.0[1], wahl),
            u64::conditional_select(&a.0[2], &b.0[2], wahl),
            u64::conditional_select(&a.0[3], &b.0[3], wahl),
        ])
    }
}

impl Skalar {
    /// Die Null.
    pub const NULL: Skalar = Skalar([0, 0, 0, 0]);
    /// Die Eins.
    pub const EINS: Skalar = Skalar(R);

    /// Aus einer kleinen Zahl, etwa einer Sitznummer.
    pub fn aus_u64(x: u64) -> Skalar {
        Skalar([x, 0, 0, 0]).mal(&Skalar(R2))
    }

    /// Aus 32 Bytes, hoechstwertiges zuerst (die Form von `blst` und
    /// IETF). `None`, wenn die Zahl nicht kleiner als `r` ist: Eine
    /// zweite Schreibweise derselben Zahl darf es im Konsens nicht geben.
    pub fn aus_bytes_be(b: &[u8; 32]) -> Option<Skalar> {
        let mut g = [0u64; 4];
        for (i, glied) in g.iter_mut().enumerate() {
            let mut w = [0u8; 8];
            w.copy_from_slice(&b[(3 - i) * 8..(4 - i) * 8]);
            *glied = u64::from_be_bytes(w);
        }
        // Kleiner als r genau dann, wenn g − r borgt.
        let (_, borgen) = sbb(g[0], MODUL[0], 0);
        let (_, borgen) = sbb(g[1], MODUL[1], borgen);
        let (_, borgen) = sbb(g[2], MODUL[2], borgen);
        let (_, borgen) = sbb(g[3], MODUL[3], borgen);
        let kleiner = (borgen as u8) & 1;
        if kleiner == 1 {
            Some(Skalar(g).mal(&Skalar(R2)))
        } else {
            None
        }
    }

    /// Aus 64 gleichverteilten Bytes (etwa SHA-512), modulo `r`. Der
    /// Abstand zur Gleichverteilung ist kleiner als `2⁻²⁵⁶`.
    pub fn aus_breit(b: &[u8; 64]) -> Skalar {
        let glieder = |teil: &[u8]| {
            let mut g = [0u64; 4];
            for (i, glied) in g.iter_mut().enumerate() {
                let mut w = [0u8; 8];
                w.copy_from_slice(&teil[i * 8..(i + 1) * 8]);
                *glied = u64::from_le_bytes(w);
            }
            g
        };
        // Wert = unten + oben · 2²⁵⁶; in Montgomery-Form ist 2²⁵⁶ gerade R2.
        let unten = Skalar(glieder(&b[..32])).mal(&Skalar(R2));
        let oben = Skalar(glieder(&b[32..])).mal(&Skalar(R2));
        unten.plus(&oben.mal(&Skalar(R2)))
    }

    /// 32 Bytes, hoechstwertiges zuerst; genau die Form, die
    /// `blst::min_pk::SecretKey::from_bytes` liest.
    pub fn als_bytes_be(&self) -> [u8; 32] {
        let g = self.kanonisch();
        let mut b = [0u8; 32];
        for i in 0..4 {
            b[(3 - i) * 8..(4 - i) * 8].copy_from_slice(&g[i].to_be_bytes());
        }
        b
    }

    /// 32 Bytes, niedrigstwertiges zuerst; die Form der Skalare fuer
    /// `blst::MultiPoint::mult`.
    pub fn als_bytes_le(&self) -> [u8; 32] {
        let g = self.kanonisch();
        let mut b = [0u8; 32];
        for i in 0..4 {
            b[i * 8..(i + 1) * 8].copy_from_slice(&g[i].to_le_bytes());
        }
        b
    }

    /// Ob dies die Null ist.
    pub fn ist_null(&self) -> bool {
        bool::from(self.ct_eq(&Skalar::NULL))
    }

    /// Summe.
    pub fn plus(&self, rhs: &Skalar) -> Skalar {
        let (d0, c) = adc(self.0[0], rhs.0[0], 0);
        let (d1, c) = adc(self.0[1], rhs.0[1], c);
        let (d2, c) = adc(self.0[2], rhs.0[2], c);
        let (d3, _) = adc(self.0[3], rhs.0[3], c);
        // Beide Summanden sind kleiner als r < 2²⁵⁵, die Summe passt also
        // in vier Glieder; einmal r abziehen genuegt.
        Skalar([d0, d1, d2, d3]).minus(&Skalar(MODUL))
    }

    /// Differenz.
    pub fn minus(&self, rhs: &Skalar) -> Skalar {
        let (d0, b) = sbb(self.0[0], rhs.0[0], 0);
        let (d1, b) = sbb(self.0[1], rhs.0[1], b);
        let (d2, b) = sbb(self.0[2], rhs.0[2], b);
        let (d3, b) = sbb(self.0[3], rhs.0[3], b);
        // Bei einem Unterlauf ist `b` lauter Einsen: r wieder dazu, ohne
        // Verzweigung.
        let (d0, c) = adc(d0, MODUL[0] & b, 0);
        let (d1, c) = adc(d1, MODUL[1] & b, c);
        let (d2, c) = adc(d2, MODUL[2] & b, c);
        let (d3, _) = adc(d3, MODUL[3] & b, c);
        Skalar([d0, d1, d2, d3])
    }

    /// Das Negative.
    pub fn negiert(&self) -> Skalar {
        Skalar::NULL.minus(self)
    }

    /// Produkt.
    pub fn mal(&self, rhs: &Skalar) -> Skalar {
        let a = &self.0;
        let b = &rhs.0;
        let (r0, c) = mac(0, a[0], b[0], 0);
        let (r1, c) = mac(0, a[0], b[1], c);
        let (r2, c) = mac(0, a[0], b[2], c);
        let (r3, r4) = mac(0, a[0], b[3], c);

        let (r1, c) = mac(r1, a[1], b[0], 0);
        let (r2, c) = mac(r2, a[1], b[1], c);
        let (r3, c) = mac(r3, a[1], b[2], c);
        let (r4, r5) = mac(r4, a[1], b[3], c);

        let (r2, c) = mac(r2, a[2], b[0], 0);
        let (r3, c) = mac(r3, a[2], b[1], c);
        let (r4, c) = mac(r4, a[2], b[2], c);
        let (r5, r6) = mac(r5, a[2], b[3], c);

        let (r3, c) = mac(r3, a[3], b[0], 0);
        let (r4, c) = mac(r4, a[3], b[1], c);
        let (r5, c) = mac(r5, a[3], b[2], c);
        let (r6, r7) = mac(r6, a[3], b[3], c);

        Skalar::reduzieren([r0, r1, r2, r3, r4, r5, r6, r7])
    }

    /// Das Quadrat.
    pub fn quadrat(&self) -> Skalar {
        self.mal(self)
    }

    /// Potenz mit einem **oeffentlichen** Exponenten (Glieder von niedrig
    /// nach hoch). Konstante Zeit in der Basis, nicht im Exponenten.
    pub fn hoch(&self, exponent: &[u64; 4]) -> Skalar {
        let mut erg = Skalar::EINS;
        for glied in exponent.iter().rev() {
            for i in (0..64).rev() {
                erg = erg.quadrat();
                let bit = Choice::from(((glied >> i) & 1) as u8);
                erg = Skalar::conditional_select(&erg, &erg.mal(self), bit);
            }
        }
        erg
    }

    /// Das Inverse; `None` fuer die Null.
    pub fn invers(&self) -> Option<Skalar> {
        if self.ist_null() {
            return None;
        }
        Some(self.hoch(&MODUL_MINUS_2))
    }

    /// Aus der Montgomery-Form zurueck in die gewoehnliche Zahl.
    fn kanonisch(&self) -> [u64; 4] {
        Skalar::reduzieren([self.0[0], self.0[1], self.0[2], self.0[3], 0, 0, 0, 0]).0
    }

    /// Montgomery-Reduktion: `t · 2⁻²⁵⁶ mod r` fuer `t < r · 2²⁵⁶`.
    fn reduzieren(t: [u64; 8]) -> Skalar {
        let [r0, r1, r2, r3, r4, r5, r6, r7] = t;

        let k = r0.wrapping_mul(INV);
        let (_, c) = mac(r0, k, MODUL[0], 0);
        let (r1, c) = mac(r1, k, MODUL[1], c);
        let (r2, c) = mac(r2, k, MODUL[2], c);
        let (r3, c) = mac(r3, k, MODUL[3], c);
        let (r4, c2) = adc(r4, 0, c);

        let k = r1.wrapping_mul(INV);
        let (_, c) = mac(r1, k, MODUL[0], 0);
        let (r2, c) = mac(r2, k, MODUL[1], c);
        let (r3, c) = mac(r3, k, MODUL[2], c);
        let (r4, c) = mac(r4, k, MODUL[3], c);
        let (r5, c2) = adc(r5, c2, c);

        let k = r2.wrapping_mul(INV);
        let (_, c) = mac(r2, k, MODUL[0], 0);
        let (r3, c) = mac(r3, k, MODUL[1], c);
        let (r4, c) = mac(r4, k, MODUL[2], c);
        let (r5, c) = mac(r5, k, MODUL[3], c);
        let (r6, c2) = adc(r6, c2, c);

        let k = r3.wrapping_mul(INV);
        let (_, c) = mac(r3, k, MODUL[0], 0);
        let (r4, c) = mac(r4, k, MODUL[1], c);
        let (r5, c) = mac(r5, k, MODUL[2], c);
        let (r6, c) = mac(r6, k, MODUL[3], c);
        let (r7, _) = adc(r7, c2, c);

        // Das Ergebnis ist kleiner als 2r; einmal r abziehen genuegt.
        Skalar([r4, r5, r6, r7]).minus(&Skalar(MODUL))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blst::min_pk::{AggregatePublicKey, PublicKey, SecretKey};
    use blst::MultiPoint;
    use sha2::{Digest, Sha512};

    fn zufall(i: u64) -> Skalar {
        let mut h = Sha512::new();
        h.update(b"myl-skalar-test");
        h.update(i.to_le_bytes());
        let b: [u8; 64] = h.finalize().into();
        Skalar::aus_breit(&b)
    }

    fn pk(s: &Skalar) -> PublicKey {
        SecretKey::from_bytes(&s.als_bytes_be()).expect("gueltiger Schluessel").sk_to_pk()
    }

    /// Die Konstanten, aus der Definition nachgerechnet statt abgeschrieben.
    #[test]
    fn die_konstanten_stimmen() {
        // INV: r · INV ≡ −1 mod 2⁶⁴.
        assert_eq!(MODUL[0].wrapping_mul(INV), u64::MAX);
        // R = 2²⁵⁶ mod r und R2 = 2⁵¹² mod r, durch Verdoppeln von 1 in
        // gewoehnlicher Darstellung.
        let mut x = [1u64, 0, 0, 0];
        let verdoppeln = |x: [u64; 4]| {
            let (d0, c) = adc(x[0], x[0], 0);
            let (d1, c) = adc(x[1], x[1], c);
            let (d2, c) = adc(x[2], x[2], c);
            let (d3, _) = adc(x[3], x[3], c);
            Skalar([d0, d1, d2, d3]).minus(&Skalar(MODUL)).0
        };
        for _ in 0..256 {
            x = verdoppeln(x);
        }
        assert_eq!(x, R);
        for _ in 0..256 {
            x = verdoppeln(x);
        }
        assert_eq!(x, R2);
        // r − 2.
        let (m0, b) = sbb(MODUL[0], 2, 0);
        let (m1, b) = sbb(MODUL[1], 0, b);
        let (m2, b) = sbb(MODUL[2], 0, b);
        let (m3, _) = sbb(MODUL[3], 0, b);
        assert_eq!([m0, m1, m2, m3], MODUL_MINUS_2);
    }

    #[test]
    fn bytes_hin_und_zurueck() {
        for i in 0..50 {
            let a = zufall(i);
            assert_eq!(Skalar::aus_bytes_be(&a.als_bytes_be()), Some(a));
        }
        assert_eq!(Skalar::aus_u64(7).als_bytes_be()[31], 7);
        assert_eq!(Skalar::aus_u64(7).als_bytes_le()[0], 7);
    }

    #[test]
    fn r_und_mehr_wird_abgelehnt() {
        let mut r = [0u8; 32];
        for i in 0..4 {
            r[(3 - i) * 8..(4 - i) * 8].copy_from_slice(&MODUL[i].to_be_bytes());
        }
        assert!(Skalar::aus_bytes_be(&r).is_none());
        assert!(Skalar::aus_bytes_be(&[0xff; 32]).is_none());
        // r − 1 ist die groesste gueltige Zahl, und sie ist −1.
        r[31] -= 1;
        assert_eq!(Skalar::aus_bytes_be(&r), Some(Skalar::EINS.negiert()));
    }

    #[test]
    fn koerpergesetze() {
        for i in 0..40 {
            let (a, b, c) = (zufall(3 * i), zufall(3 * i + 1), zufall(3 * i + 2));
            assert_eq!(a.plus(&b), b.plus(&a));
            assert_eq!(a.mal(&b), b.mal(&a));
            assert_eq!(a.mal(&b.plus(&c)), a.mal(&b).plus(&a.mal(&c)));
            assert_eq!(a.plus(&b).minus(&b), a);
            assert_eq!(a.plus(&a.negiert()), Skalar::NULL);
            assert_eq!(a.mal(&a.invers().expect("ungleich null")), Skalar::EINS);
        }
        assert!(Skalar::NULL.invers().is_none());
        assert_eq!(Skalar::aus_u64(6).mal(&Skalar::aus_u64(7)), Skalar::aus_u64(42));
        assert_eq!(Skalar::aus_u64(2).minus(&Skalar::aus_u64(5)).plus(&Skalar::aus_u64(3)), Skalar::NULL);
    }

    /// ⚑ Der Abgleich mit `blst`: Addition und Multiplikation muessen mit
    /// der Gruppenrechnung uebereinstimmen.
    #[test]
    fn stimmt_mit_blst_ueberein() {
        for i in 0..10 {
            let (a, b) = (zufall(100 + 2 * i), zufall(101 + 2 * i));
            let summe = AggregatePublicKey::aggregate(&[&pk(&a), &pk(&b)], true).expect("Summe").to_public_key();
            assert_eq!(pk(&a.plus(&b)).compress(), summe.compress());
            let produkt = [pk(&a)].mult(&b.als_bytes_le(), 255).to_public_key();
            assert_eq!(pk(&a.mal(&b)).compress(), produkt.compress());
        }
    }
}
