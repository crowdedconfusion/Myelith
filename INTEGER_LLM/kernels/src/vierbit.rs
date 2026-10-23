//! Vierbitgewichte: je Gewicht ein Code `q` von 0 bis 15, dazu je Gruppe
//! von [`GRUPPE`] Eingaengen ein ganzer Schritt `a` und ein ganzer Versatz
//! `b`. Der Wert eines Gewichts ist `a * q + b`.
//!
//! # Wozu
//!
//! Die Experten eines Gemischs tragen fast alle Gewichte des Modells. Am
//! int8-35B (2026-10-16) ergibt ein Gitter mit 16 gleichmaessigen Stufen je
//! Gruppe von 128, kalibriert mit GPTQ und geschrumpfter Kovarianz, eine
//! Perplexitaet 0,5 % ueber dem int8, ohne Training. Im int8-Behaelter
//! liegen dabei acht Bit je Gewicht, wo vier und ein Achtel genuegen: Das
//! Artefakt schrumpft von 33 auf rund 18 GB und passt in den Speicher einer
//! Maschine mit 24 GiB, statt bei jedem Token Experten nachzuladen.
//!
//! # Das Format
//!
//! Je Gruppe [`BYTES_JE_GRUPPE`] Byte in vier Bloecken zu 16 Byte fuer je 32
//! Gewichte; **Byte `j` eines Blocks traegt in den Bits 0 bis 3 den Code
//! des Gewichts `j` und in den Bits 4 bis 7 den des Gewichts `16 + j`**
//! dieses Blocks. Dazu je Gruppe zwei Byte: der Schritt `a` ohne Vorzeichen
//! (0 bis 255) und der Versatz `b` mit Vorzeichen (-128 bis 127). Zeile fuer
//! Zeile, Gruppe fuer Gruppe.
//!
//! ⚑ **Warum verschraenkt nach Position.** Ein Vektor aus 16 Bytes ergibt
//! mit einem UND die Codes von 16 aufeinander folgenden Gewichten und mit
//! einem Schieben die der naechsten 16, je einer in seiner Lane, ohne
//! Tabellenabfrage. Dieselbe Idee wie im ternaeren Format
//! ([`crate::ternaer`]), mit zwei statt vier Codes je Byte.
//!
//! ⚑ **Ganze Stufen statt einer Skala.** Ein Gitter mit gebrochenem
//! Schritt waere im int8-Behaelter erst nach dem Runden ganzzahlig, und
//! dann sind die 16 Stufen nicht mehr gleichmaessig. Mit ganzem `a` und
//! `b` ist jede Gruppe genau die Menge, die der Behaelter traegt; gemessen
//! kostet die Einschraenkung am 35B rund 4 % mehr Fehler je Matrix.
//!
//! # ⚑ Warum das bitgleich zum int8-Weg ist, per Konstruktion
//!
//! Der int8-Weg rechnet `sum_i w_i * x_i` exakt in i64. Hier ist
//! `w_i = a_g * q_i + b_g`, also
//! `sum_g (a_g * sum_{i in g} q_i * x_i + b_g * sum_{i in g} x_i)`:
//! **dieselbe ganze Zahl**, nur anders geklammert. Ein Artefakt, das aus
//! einem int8-Artefakt mit solchen Gruppen umgepackt ist, liefert deshalb
//! dieselben Logits, Bit fuer Bit, und was am int8-Artefakt gemessen wurde,
//! gilt fuer das gepackte.
//!
//! # Wertebereich
//!
//! `|a * q + b|` ist hoechstens 255 * 15 + 128 = 3953 < 2^12. Eine
//! Gruppensumme hat 128 Summanden vom Betrag bis 2^12 * 2^15, liegt also
//! unter 2^34; ueber alle Gruppen einer Zeile wird in i64 summiert. Die
//! Teilsummen der vektorisierten Fassungen haengen nur an `q` und `x`, nie
//! an `a` und `b` (Bereiche dort).

use crate::ternaer::Eingabe;

/// Gewichte je Gruppe, also je Schritt und Versatz.
pub const GRUPPE: usize = 128;

/// Bytes je Gruppe: vier Bit je Gewicht.
pub const BYTES_JE_GRUPPE: usize = GRUPPE / 2;

/// Bytes je Gruppe fuer das Gitter: Schritt und Versatz.
pub const GITTER_JE_GRUPPE: usize = 2;

/// Gewichte je Block von 16 Bytes.
const BLOCK: usize = 32;

/// Eine Vierbitmatrix, gelesen aus Codes und Gittern, die jemand anderes
/// haelt (ein Abbild der Artefaktdatei oder ein Test).
#[derive(Clone, Copy, Debug)]
pub struct Vierbitmatrix<'a> {
    codes: &'a [u8],
    gitter: &'a [u8],
    spalten: usize,
}

impl<'a> Vierbitmatrix<'a> {
    /// Prueft die Laengen gegen die Form. ⚑ **Hier und nur hier**: Die
    /// vektorisierte Fassung liest ohne Grenzpruefung und verlaesst sich
    /// darauf.
    pub fn neu(codes: &'a [u8], gitter: &'a [u8], zeilen: usize, spalten: usize) -> Result<Self, String> {
        if spalten == 0 || !spalten.is_multiple_of(GRUPPE) {
            return Err(format!("Vierbitmatrix: {spalten} Spalten sind kein Vielfaches von {GRUPPE}"));
        }
        let gruppen = zeilen
            .checked_mul(spalten / GRUPPE)
            .ok_or_else(|| format!("Vierbitmatrix: {zeilen} x {spalten} ist zu gross"))?;
        if gitter.len() != gruppen * GITTER_JE_GRUPPE {
            return Err(format!(
                "Vierbitmatrix: {} Gitterbytes, aber {zeilen} x {spalten} braucht {}",
                gitter.len(),
                gruppen * GITTER_JE_GRUPPE
            ));
        }
        if codes.len() != gruppen * BYTES_JE_GRUPPE {
            return Err(format!(
                "Vierbitmatrix: {} Codebytes, aber {zeilen} x {spalten} braucht {}",
                codes.len(),
                gruppen * BYTES_JE_GRUPPE
            ));
        }
        Ok(Vierbitmatrix { codes, gitter, spalten })
    }

    pub fn zeilen(&self) -> usize {
        self.gitter.len() / GITTER_JE_GRUPPE / (self.spalten / GRUPPE)
    }

    pub fn spalten(&self) -> usize {
        self.spalten
    }

    /// Die Codebytes, Zeile fuer Zeile.
    pub fn codes(&self) -> &'a [u8] {
        self.codes
    }

    /// Schritt und Versatz, zwei Byte je Gruppe, Zeile fuer Zeile.
    pub fn gitter(&self) -> &'a [u8] {
        self.gitter
    }

    /// `sum_i w[z][i] * x[i]`, exakt, dieselbe Zahl wie `dot_i8_i16` ueber
    /// die entpackte Zeile; `e` aus [`Eingabe::neu`].
    ///
    /// ⚠️ **`x` muss mindestens eine Zeile lang sein**, sonst Panik, wie
    /// beim ternaeren Format: Eine falsche Laenge soll auffallen und nicht
    /// eine schlechte Zahl werden.
    #[inline]
    pub fn zeile_mal_mit(&self, z: usize, e: &Eingabe<'_>) -> i64 {
        let n = self.spalten;
        assert!(e.x().len() >= n, "Vierbitzeile: Eingabe mit {} Werten, die Zeile hat {n}", e.x().len());
        let g = n / GRUPPE;
        let (hoch, tief, summen) = e.teile(n, g);
        crate::dot::vierbit_zeile(
            &self.codes[z * g * BYTES_JE_GRUPPE..(z + 1) * g * BYTES_JE_GRUPPE],
            &self.gitter[z * g * GITTER_JE_GRUPPE..(z + 1) * g * GITTER_JE_GRUPPE],
            &e.x()[..n],
            hoch,
            tief,
            summen,
        )
    }

    /// **Dieselbe Zeile mal mehreren Eingaben**: `aus[k]` ist genau
    /// [`Self::zeile_mal_mit`] fuer `e[k]`. Die Codes einer Gruppe werden
    /// einmal entpackt und fuer jede Eingabe benutzt.
    #[inline]
    pub fn zeile_mal_viele(&self, z: usize, e: &[Eingabe<'_>], aus: &mut [i64]) {
        assert_eq!(e.len(), aus.len(), "Vierbitzeile: {} Eingaben, {} Ausgaben", e.len(), aus.len());
        let n = self.spalten;
        let g = n / GRUPPE;
        for ein in e {
            assert!(ein.x().len() >= n, "Vierbitzeile: Eingabe mit {} Werten, die Zeile hat {n}", ein.x().len());
        }
        crate::dot::vierbit_zeile_viele(
            &self.codes[z * g * BYTES_JE_GRUPPE..(z + 1) * g * BYTES_JE_GRUPPE],
            &self.gitter[z * g * GITTER_JE_GRUPPE..(z + 1) * g * GITTER_JE_GRUPPE],
            e,
            aus,
        );
    }

    /// Wie [`Self::zeile_mal_mit`], mit einer Vorbereitung je Aufruf. Fuer
    /// Pruefungen; im Rechenpfad wird einmal je Eingabe vorbereitet.
    pub fn zeile_mal(&self, z: usize, x: &[i16]) -> i64 {
        self.zeile_mal_mit(z, &Eingabe::neu(x))
    }

    /// Die Zeile `z` als ganze Zahlen, `a * q + b`. Fuer Pruefungen und
    /// Werkzeuge, nicht fuer den Rechenpfad.
    pub fn zeile_entpacken(&self, z: usize) -> Vec<i32> {
        let g = self.spalten / GRUPPE;
        let mut aus = Vec::with_capacity(self.spalten);
        for gruppe in z * g..(z + 1) * g {
            let c = &self.codes[gruppe * BYTES_JE_GRUPPE..(gruppe + 1) * BYTES_JE_GRUPPE];
            let (a, b) = schritt_versatz(&self.gitter[gruppe * GITTER_JE_GRUPPE..]);
            for i in 0..GRUPPE {
                aus.push(a * code(c, i) + b);
            }
        }
        aus
    }
}

/// Wo der Code des Gewichts `i` einer Gruppe steht: Byte und Bitversatz.
#[inline]
fn stelle(i: usize) -> (usize, usize) {
    let (block, rest) = (i / BLOCK, i % BLOCK);
    (block * 16 + rest % 16, 4 * (rest / 16))
}

/// Der Code `q` des Gewichts `i` einer Gruppe.
#[inline]
fn code(c: &[u8], i: usize) -> i32 {
    let (byte, versatz) = stelle(i);
    i32::from((c[byte] >> versatz) & 0x0F)
}

/// Schritt und Versatz aus den ersten zwei Bytes von `g`.
#[inline]
fn schritt_versatz(g: &[u8]) -> (i32, i32) {
    (i32::from(g[0]), i32::from(g[1] as i8))
}

/// Skalare Referenzfassung, der numerische Vertrag.
///
/// Vorausgesetzt (geprueft in [`crate::dot::vierbit_zeile`]):
/// `codes.len() == gruppen * BYTES_JE_GRUPPE`,
/// `gitter.len() == gruppen * GITTER_JE_GRUPPE` und
/// `x.len() == gruppen * GRUPPE`.
pub fn zeile_skalar(codes: &[u8], gitter: &[u8], x: &[i16]) -> i64 {
    let mut gesamt: i64 = 0;
    for (g, gg) in gitter.as_chunks::<GITTER_JE_GRUPPE>().0.iter().enumerate() {
        let (a, b) = schritt_versatz(gg);
        let c = &codes[g * BYTES_JE_GRUPPE..(g + 1) * BYTES_JE_GRUPPE];
        for (i, &xi) in x[g * GRUPPE..(g + 1) * GRUPPE].iter().enumerate() {
            gesamt += i64::from(a * code(c, i) + b) * i64::from(xi);
        }
    }
    gesamt
}

#[cfg(all(feature = "cpu-simd", target_arch = "aarch64"))]
pub(crate) mod neon {
    use super::{BYTES_JE_GRUPPE, GITTER_JE_GRUPPE, GRUPPE};
    use crate::ternaer::neon::{sdot, udot};
    use std::arch::aarch64::*;

    /// Die acht Codevektoren einer Gruppe: je Block `B & 15` (Gewichte 0
    /// bis 15 des Blocks) und `B >> 4` (16 bis 31).
    #[inline]
    unsafe fn codes_der_gruppe(c: *const u8) -> [uint8x16_t; 8] {
        let f = vdupq_n_u8(0x0F);
        let b0 = vld1q_u8(c);
        let b1 = vld1q_u8(c.add(16));
        let b2 = vld1q_u8(c.add(32));
        let b3 = vld1q_u8(c.add(48));
        [
            vandq_u8(b0, f),
            vshrq_n_u8::<4>(b0),
            vandq_u8(b1, f),
            vshrq_n_u8::<4>(b1),
            vandq_u8(b2, f),
            vshrq_n_u8::<4>(b2),
            vandq_u8(b3, f),
            vshrq_n_u8::<4>(b3),
        ]
    }

    /// `a * sum(q*x) + b * sum(x)` einer Gruppe.
    #[inline]
    unsafe fn gruppe_fertig(gitter: &[u8], g: usize, s_qx: i64, s_x: i64) -> i64 {
        let a = i64::from(*gitter.get_unchecked(g * GITTER_JE_GRUPPE));
        let b = i64::from(*gitter.get_unchecked(g * GITTER_JE_GRUPPE + 1) as i8);
        a * s_qx + b * s_x
    }

    /// Je Codevektor gegen `hoch` (`vmlal_s8`; der Code ist 0 bis 15 und
    /// damit auch als `i8` richtig) und gegen `tief` (`vmlal_u8`). Je Gruppe
    /// dann `sum q*x = 256 * sum q*hoch + sum q*tief`, siehe
    /// [`crate::ternaer::Eingabe`].
    ///
    /// **Wertebereich:** Jede Lane der vier Akkumulatoren bekommt je Gruppe
    /// acht Produkte. Mit `hoch` hoechstens 15 * 128 = 1920, also 15 360 in
    /// `i16`; mit `tief` hoechstens 15 * 255 = 3825, also 30 600 in `u16`.
    /// Zwei Akkumulatoren zusammen: 30 720 und 61 200, beide passen.
    ///
    /// # Sicherheit
    ///
    /// Der Aufrufer sichert `codes.len() == g * 64`, `gitter.len() == g * 2`,
    /// `hoch.len() == tief.len() == g * 128` und `summen.len() == g` zu.
    #[inline]
    pub unsafe fn zeile(codes: &[u8], gitter: &[u8], hoch: &[i8], tief: &[u8], summen: &[i64]) -> i64 {
        let mut gesamt: i64 = 0;
        for g in 0..gitter.len() / GITTER_JE_GRUPPE {
            let cv = codes_der_gruppe(codes.as_ptr().add(g * BYTES_JE_GRUPPE));
            let h = hoch.as_ptr().add(g * GRUPPE);
            let t = tief.as_ptr().add(g * GRUPPE);
            let mut h0 = vdupq_n_s16(0);
            let mut h1 = vdupq_n_s16(0);
            let mut t0 = vdupq_n_u16(0);
            let mut t1 = vdupq_n_u16(0);
            for (s, c) in cv.iter().enumerate() {
                let hv = vld1q_s8(h.add(16 * s));
                let tv = vld1q_u8(t.add(16 * s));
                let cs = vreinterpretq_s8_u8(*c);
                h0 = vmlal_s8(h0, vget_low_s8(cs), vget_low_s8(hv));
                h1 = vmlal_high_s8(h1, cs, hv);
                t0 = vmlal_u8(t0, vget_low_u8(*c), vget_low_u8(tv));
                t1 = vmlal_high_u8(t1, *c, tv);
            }
            let s_hoch = i64::from(vaddlvq_s16(vaddq_s16(h0, h1)));
            let s_tief = i64::from(vaddlvq_u16(vaddq_u16(t0, t1)));
            gesamt += gruppe_fertig(gitter, g, 256 * s_hoch + s_tief, *summen.get_unchecked(g));
        }
        gesamt
    }

    /// **Dieselbe Rechnung mit `sdot` und `udot`**, wie im ternaeren Kern.
    ///
    /// **Wertebereich:** Eine Lane bekommt je Gruppe 32 Produkte, mit
    /// `hoch` hoechstens 32 * 1920 = 61 440, mal 256 rund 1,6 * 10^7, mit
    /// `tief` hoechstens 32 * 3825 = 122 400. Ueber die vier Lanes bleibt
    /// die Summe unter 2^26, also weit in 32 Bit.
    ///
    /// # Sicherheit
    ///
    /// Wie [`zeile`], und die Maschine muss `dotprod` haben.
    #[target_feature(enable = "dotprod")]
    pub unsafe fn zeile_dot(codes: &[u8], gitter: &[u8], hoch: &[i8], tief: &[u8], summen: &[i64]) -> i64 {
        let mut gesamt: i64 = 0;
        for g in 0..gitter.len() / GITTER_JE_GRUPPE {
            let cv = codes_der_gruppe(codes.as_ptr().add(g * BYTES_JE_GRUPPE));
            let s = gruppe_dot(&cv, hoch.as_ptr().add(g * GRUPPE), tief.as_ptr().add(g * GRUPPE));
            gesamt += gruppe_fertig(gitter, g, s, *summen.get_unchecked(g));
        }
        gesamt
    }

    /// `sum q*x` einer Gruppe aus ihren Codevektoren, zwei Ketten je Summe,
    /// damit ein Befehl nicht auf den vorigen wartet.
    #[inline]
    #[target_feature(enable = "dotprod")]
    unsafe fn gruppe_dot(cv: &[uint8x16_t; 8], h: *const i8, t: *const u8) -> i64 {
        let mut sh0 = vdupq_n_s32(0);
        let mut sh1 = vdupq_n_s32(0);
        let mut st0 = vdupq_n_u32(0);
        let mut st1 = vdupq_n_u32(0);
        for (s, c) in cv.iter().enumerate() {
            let hv = vld1q_s8(h.add(16 * s));
            let tv = vld1q_u8(t.add(16 * s));
            if s % 2 == 0 {
                sh0 = sdot(sh0, vreinterpretq_s8_u8(*c), hv);
                st0 = udot(st0, *c, tv);
            } else {
                sh1 = sdot(sh1, vreinterpretq_s8_u8(*c), hv);
                st1 = udot(st1, *c, tv);
            }
        }
        // Je Lane zusammengefuehrt und einmal quer summiert; Bereich oben.
        let hoch = vshlq_n_s32::<8>(vaddq_s32(sh0, sh1));
        let tief = vreinterpretq_s32_u32(vaddq_u32(st0, st1));
        i64::from(vaddvq_s32(vaddq_s32(hoch, tief)))
    }

    /// **Eine Zeile mal mehreren Eingaben**, `aus[k]` wie [`zeile_dot`] fuer
    /// die Eingabe `k`. Die acht Codevektoren einer Gruppe entstehen einmal
    /// und bleiben in Registern.
    ///
    /// # Sicherheit
    ///
    /// Wie [`zeile_dot`], fuer jede Eingabe; `aus.len() == eingaben.len()`.
    #[target_feature(enable = "dotprod")]
    pub unsafe fn zeile_dot_viele(codes: &[u8], gitter: &[u8], eingaben: &[crate::ternaer::Eingabe<'_>], aus: &mut [i64]) {
        for a in aus.iter_mut() {
            *a = 0;
        }
        for g in 0..gitter.len() / GITTER_JE_GRUPPE {
            let cv = codes_der_gruppe(codes.as_ptr().add(g * BYTES_JE_GRUPPE));
            for (k, e) in eingaben.iter().enumerate() {
                let (hoch, tief, summen) = e.roh_teile();
                let s = gruppe_dot(&cv, hoch.as_ptr().add(g * GRUPPE), tief.as_ptr().add(g * GRUPPE));
                *aus.get_unchecked_mut(k) += gruppe_fertig(gitter, g, s, *summen.get_unchecked(g));
            }
        }
    }
}

/// **Die Fassung fuer x86_64 mit SSE2**, das jede x86_64-Maschine hat:
/// kein Laufzeittest, keine Zerlegung der Eingabe.
///
/// ⚑ **Warum SSE2 und nicht AVX2.** `_mm_madd_epi16` multipliziert je acht
/// Codes, als `i16` von 0 bis 15, mit acht Werten von `x` und addiert je zwei
/// Produkte exakt in `i32`; mehr braucht die Rechnung nicht. AVX2 rechnete
/// dasselbe doppelt so breit, laesst sich auf der Entwicklungsmaschine aber
/// nicht ausfuehren (Rosetta kennt kein AVX), und ungepruefte Numerik kommt
/// nicht in den Rechenpfad (Kopf von `crate::dot`). SSE2 laeuft dort unter
/// Rosetta und in der CI auf echter x86_64-Hardware.
///
/// **Wertebereich:** Ein Paar aus `madd` ist hoechstens 2 * 15 * 32 768 =
/// 983 040 dem Betrag nach, jede Lane bekommt je Gruppe 16 Paare, also unter
/// 2^24; die Summe der Eingaben je Lane hoechstens 16 * 65 536 = 2^20. Ueber
/// die vier Lanes bleibt beides unter 2^26, weit in `i32`; Schritt und
/// Versatz kommen erst in `i64` dazu.
#[cfg(all(feature = "cpu-simd", target_arch = "x86_64"))]
pub(crate) mod sse2 {
    use super::{BYTES_JE_GRUPPE, GITTER_JE_GRUPPE, GRUPPE};
    use std::arch::x86_64::*;

    /// Die vier Lanes eines `i32`-Vektors, addiert.
    #[inline]
    unsafe fn quer(v: __m128i) -> i32 {
        let v = _mm_add_epi32(v, _mm_shuffle_epi32::<0b01_00_11_10>(v));
        let v = _mm_add_epi32(v, _mm_shuffle_epi32::<0b10_11_00_01>(v));
        _mm_cvtsi128_si32(v)
    }

    /// `sum_i w_i * x_i` einer Zeile, dieselbe Zahl wie [`super::zeile_skalar`].
    ///
    /// Je Block von 16 Codebytes: `B & 15` sind die Codes der Gewichte 0 bis
    /// 15 des Blocks, `(B >> 4) & 15` die von 16 bis 31 (verschoben wird in
    /// 16-Bit-Lanes; was dabei aus dem hohen Byte ins niedrige rutscht,
    /// liegt in dessen oberem Halbbyte und faellt mit der Maske weg). Je acht
    /// Codes werden mit Nullen zu `i16` erweitert und mit acht Werten von `x`
    /// multipliziert.
    ///
    /// # Sicherheit
    ///
    /// Der Aufrufer sichert `codes.len() == g * 64`, `gitter.len() == g * 2`
    /// und `x.len() == g * 128` zu ([`crate::dot::vierbit_zeile`]).
    #[inline]
    pub unsafe fn zeile(codes: &[u8], gitter: &[u8], x: &[i16]) -> i64 {
        let null = _mm_setzero_si128();
        let maske = _mm_set1_epi8(0x0F);
        let einsen = _mm_set1_epi16(1);
        let mut gesamt: i64 = 0;
        for g in 0..gitter.len() / GITTER_JE_GRUPPE {
            let c = codes.as_ptr().add(g * BYTES_JE_GRUPPE);
            let xg = x.as_ptr().add(g * GRUPPE);
            let mut s_qx = null;
            let mut s_x = null;
            for block in 0..4 {
                let b = _mm_loadu_si128(c.add(16 * block) as *const __m128i);
                let unten = _mm_and_si128(b, maske);
                let oben = _mm_and_si128(_mm_srli_epi16::<4>(b), maske);
                let teile = [
                    _mm_unpacklo_epi8(unten, null),
                    _mm_unpackhi_epi8(unten, null),
                    _mm_unpacklo_epi8(oben, null),
                    _mm_unpackhi_epi8(oben, null),
                ];
                for (k, q) in teile.iter().enumerate() {
                    let xv = _mm_loadu_si128(xg.add(32 * block + 8 * k) as *const __m128i);
                    s_qx = _mm_add_epi32(s_qx, _mm_madd_epi16(*q, xv));
                    s_x = _mm_add_epi32(s_x, _mm_madd_epi16(xv, einsen));
                }
            }
            let a = i64::from(*gitter.get_unchecked(g * GITTER_JE_GRUPPE));
            let v = i64::from(*gitter.get_unchecked(g * GITTER_JE_GRUPPE + 1) as i8);
            gesamt += a * i64::from(quer(s_qx)) + v * i64::from(quer(s_x));
        }
        gesamt
    }
}

/// Ergebnis von [`packen`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gepackt {
    pub codes: Vec<u8>,
    pub gitter: Vec<u8>,
}

/// Packt eine flache int8-Matrix (Zeile fuer Zeile, `spalten` je Zeile),
/// wenn **jede** Gruppe vierbittig ist: Ihre Werte sind `a * q + b` mit
/// ganzen `a`, `b` und `q` von 0 bis 15.
///
/// Gewaehlt werden `b` als kleinster Wert der Gruppe und `a` als groesster
/// gemeinsamer Teiler der Abstaende zu ihm. Damit ist jede Gruppe, die
/// ueberhaupt so darstellbar ist, auch so gepackt: Jeder andere Schritt
/// teilt die Abstaende ebenfalls und ist hoechstens so gross. Eine Gruppe
/// mit nur einem Wert bekommt den Schritt null.
///
/// ⚑ **Kein Runden, kein Naehern.** Eine Gruppe, die nicht vierbittig ist,
/// ist ein Fehler mit Zeile und Gruppe, denn das Versprechen dieses
/// Formats ist Bitgleichheit mit der Quelle.
pub fn packen(werte: &[i8], spalten: usize) -> Result<Gepackt, String> {
    if spalten == 0 || !spalten.is_multiple_of(GRUPPE) {
        return Err(format!("packen: {spalten} Spalten sind kein Vielfaches von {GRUPPE}"));
    }
    if !werte.len().is_multiple_of(spalten) {
        return Err(format!("packen: {} Werte sind keine ganzen Zeilen zu {spalten}", werte.len()));
    }
    let je_zeile = spalten / GRUPPE;
    let gruppen = werte.len() / GRUPPE;
    let mut codes = vec![0u8; gruppen * BYTES_JE_GRUPPE];
    let mut gitter = Vec::with_capacity(gruppen * GITTER_JE_GRUPPE);
    for (g, gruppe) in werte.as_chunks::<GRUPPE>().0.iter().enumerate() {
        let b = i32::from(*gruppe.iter().min().expect("eine Gruppe hat 128 Werte"));
        let a = gruppe.iter().fold(0u32, |t, &w| ggt(t, (i32::from(w) - b) as u32));
        let groesster = gruppe.iter().map(|&w| (i32::from(w) - b) as u32).max().unwrap_or(0);
        if a != 0 && groesster / a > 15 {
            return Err(format!(
                "packen: Zeile {}, Gruppe {}: Abstaende bis {groesster} mit dem Teiler {a} brauchen \
                 {} Stufen, die Gruppe ist nicht vierbittig",
                g / je_zeile,
                g % je_zeile,
                groesster / a + 1
            ));
        }
        let c = &mut codes[g * BYTES_JE_GRUPPE..(g + 1) * BYTES_JE_GRUPPE];
        for (i, &w) in gruppe.iter().enumerate() {
            // Schritt null: eine Gruppe mit einem Wert, jeder Code ist 0.
            let q = ((i32::from(w) - b) as u32).checked_div(a).unwrap_or(0);
            let (byte, versatz) = stelle(i);
            c[byte] |= (q as u8) << versatz;
        }
        // Der Teiler zweier Abstaende unter 256 ist selbst unter 256.
        gitter.push(a as u8);
        gitter.push(b as i8 as u8);
    }
    Ok(Gepackt { codes, gitter })
}

fn ggt(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dot::dot_i8_i16;

    // ⚠️ Kein `skalar_erzwingen` in diesen Tests, aus demselben Grund wie
    // in `ternaer`: Der Schalter gilt fuer den ganzen Prozess.

    fn zufall(n: usize, saat: u64) -> Vec<u64> {
        let mut s = saat.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s
            })
            .collect()
    }

    /// Eine Matrix, deren Gruppen `a * q + b` sind, mit den Schritten
    /// reihum aus `schritte`, und der Versatz so, dass alles in int8 passt.
    fn vierbitmatrix(zeilen: usize, spalten: usize, saat: u64, schritte: &[i32]) -> Vec<i8> {
        let r = zufall(zeilen * spalten, saat);
        let mut w = Vec::with_capacity(zeilen * spalten);
        for (g, stueck) in r.chunks(GRUPPE).enumerate() {
            let a = schritte[g % schritte.len()];
            let spanne = 15 * a;
            let b = -127 + (stueck[0] % (255 - spanne as u64).max(1)) as i32;
            for &z in stueck {
                w.push((a * (z % 16) as i32 + b) as i8);
            }
        }
        w
    }

    fn aktivierung(n: usize, saat: u64) -> Vec<i16> {
        zufall(n, saat).iter().map(|&z| z as i16).collect()
    }

    #[test]
    fn gepackt_ist_dieselbe_zahl_wie_int8() {
        let (zeilen, spalten) = (7, 512);
        for (saat, schritte) in [(1u64, vec![1, 3, 7, 16]), (2, vec![0, 2, 5, 11]), (3, vec![8])] {
            let w = vierbitmatrix(zeilen, spalten, saat, &schritte);
            let p = packen(&w, spalten).expect("vierbittig");
            let m = Vierbitmatrix::neu(&p.codes, &p.gitter, zeilen, spalten).unwrap();
            let x = aktivierung(spalten, saat + 10);
            let e = Eingabe::neu(&x);
            for z in 0..zeilen {
                let soll = dot_i8_i16(&w[z * spalten..(z + 1) * spalten], &x);
                assert_eq!(m.zeile_mal_mit(z, &e), soll, "Saat {saat}, Zeile {z}");
                assert_eq!(zeile_skalar(&p.codes[z * 256..(z + 1) * 256], &p.gitter[z * 8..(z + 1) * 8], &x), soll);
                let entpackt: Vec<i32> = w[z * spalten..(z + 1) * spalten].iter().map(|&v| i32::from(v)).collect();
                assert_eq!(m.zeile_entpacken(z), entpackt, "Saat {saat}, Zeile {z}");
            }
        }
    }

    #[test]
    fn die_groessten_werte_laufen_nicht_ueber() {
        // Schritt 255 und Versatz -128 kommen aus keinem int8 (die Spanne
        // waere 3825), sind aber festgelegt; x am Rand von i16.
        let spalten = 256;
        let codes = vec![0xFFu8; 2 * BYTES_JE_GRUPPE];
        let gitter = vec![255u8, 0x80, 255, 0x80];
        let m = Vierbitmatrix::neu(&codes, &gitter, 1, spalten).unwrap();
        for x0 in [i16::MAX, i16::MIN] {
            let x = vec![x0; spalten];
            let soll = (255 * 15 - 128) as i64 * i64::from(x0) * spalten as i64;
            assert_eq!(m.zeile_mal(0, &x), soll);
            assert_eq!(zeile_skalar(&codes, &gitter, &x), soll);
        }
    }

    #[test]
    fn eine_gruppe_mit_siebzehn_stufen_wird_abgewiesen() {
        let mut w: Vec<i8> = (0..128).map(|i| (i % 16) as i8).collect();
        assert!(packen(&w, 128).is_ok());
        w[5] = 16;
        let fehler = packen(&w, 128).unwrap_err();
        assert!(fehler.contains("Zeile 0, Gruppe 0") && fehler.contains("17 Stufen"), "{fehler}");
        // Schritt zwei mit 16 Stufen passt, ein ungerader Ausreisser nicht.
        let mut w: Vec<i8> = (0..128).map(|i| (2 * (i % 16) - 15) as i8).collect();
        assert_eq!(packen(&w, 128).unwrap().gitter, vec![2, (-15i8) as u8]);
        w[0] = 0;
        assert!(packen(&w, 128).is_err());
    }

    #[test]
    fn eine_gruppe_mit_einem_wert_hat_schritt_null() {
        let w = vec![-7i8; 128];
        let p = packen(&w, 128).unwrap();
        assert_eq!(p.gitter, vec![0, (-7i8) as u8]);
        assert!(p.codes.iter().all(|&c| c == 0));
    }

    #[test]
    fn das_format_ist_verschraenkt_nach_position() {
        // Gewicht 16 + j eines Blocks steht im hohen Halbbyte von Byte j.
        let mut w = vec![0i8; 128];
        w[16 + 3] = 1;
        w[32 + 5] = 1;
        let p = packen(&w, 128).unwrap();
        assert_eq!(p.codes[3], 0x10);
        assert_eq!(p.codes[16 + 5], 0x01);
        assert_eq!(p.codes.iter().filter(|&&c| c != 0).count(), 2);
    }

    #[test]
    fn falsche_laengen_fallen_auf() {
        let codes = vec![0u8; 64];
        let gitter = vec![0u8; 2];
        assert!(Vierbitmatrix::neu(&codes, &gitter, 1, 128).is_ok());
        assert!(Vierbitmatrix::neu(&codes, &gitter, 1, 256).is_err());
        assert!(Vierbitmatrix::neu(&codes[..63], &gitter, 1, 128).is_err());
        assert!(Vierbitmatrix::neu(&codes, &gitter[..1], 1, 128).is_err());
        assert!(Vierbitmatrix::neu(&codes, &gitter, 1, 100).is_err());
        assert!(packen(&[0i8; 100], 100).is_err());
    }

    #[test]
    fn die_gemeinsame_fassung_ist_die_einzelne() {
        let (zeilen, spalten) = (5, 384);
        let w = vierbitmatrix(zeilen, spalten, 9, &[1, 4, 9, 13]);
        let p = packen(&w, spalten).unwrap();
        let m = Vierbitmatrix::neu(&p.codes, &p.gitter, zeilen, spalten).unwrap();
        let xs: Vec<Vec<i16>> = (0..6).map(|k| aktivierung(spalten, 40 + k)).collect();
        let es: Vec<Eingabe<'_>> = xs.iter().map(|x| Eingabe::neu(x)).collect();
        for z in 0..zeilen {
            let mut aus = vec![0i64; es.len()];
            m.zeile_mal_viele(z, &es, &mut aus);
            for (k, x) in xs.iter().enumerate() {
                assert_eq!(aus[k], dot_i8_i16(&w[z * spalten..(z + 1) * spalten], x), "Zeile {z}, Eingabe {k}");
            }
        }
    }

    #[cfg(all(feature = "cpu-simd", target_arch = "aarch64"))]
    #[test]
    fn beide_vektorfassungen_sind_die_skalare() {
        let (zeilen, spalten) = (4, 1024);
        let w = vierbitmatrix(zeilen, spalten, 21, &[1, 2, 15, 16]);
        let p = packen(&w, spalten).unwrap();
        // Auch die Raender von i16, wo hoch und tief am groessten sind.
        let mut x = aktivierung(spalten, 22);
        x[0] = i16::MIN;
        x[1] = i16::MAX;
        x[200] = -1;
        let e = Eingabe::neu(&x);
        let (hoch, tief, summen) = e.teile(spalten, spalten / GRUPPE);
        let g = spalten / GRUPPE;
        for z in 0..zeilen {
            let c = &p.codes[z * g * BYTES_JE_GRUPPE..(z + 1) * g * BYTES_JE_GRUPPE];
            let gt = &p.gitter[z * g * GITTER_JE_GRUPPE..(z + 1) * g * GITTER_JE_GRUPPE];
            let soll = zeile_skalar(c, gt, &x);
            assert_eq!(soll, dot_i8_i16(&w[z * spalten..(z + 1) * spalten], &x));
            assert_eq!(unsafe { neon::zeile(c, gt, hoch, tief, summen) }, soll, "vmlal, Zeile {z}");
            if crate::ternaer::neon::mit_dot() {
                assert_eq!(unsafe { neon::zeile_dot(c, gt, hoch, tief, summen) }, soll, "sdot, Zeile {z}");
            }
        }
    }

    /// ⚑ **Die SSE2-Fassung ist die skalare**, auch am Rand: Schritt 255
    /// mit Versatz -128, Schritt 0, Code 15 und `x` am Rand von `i16`.
    #[cfg(all(feature = "cpu-simd", target_arch = "x86_64"))]
    #[test]
    fn die_sse2_fassung_ist_die_skalare() {
        let (zeilen, spalten) = (4, 1024);
        let w = vierbitmatrix(zeilen, spalten, 23, &[1, 2, 15, 16]);
        let p = packen(&w, spalten).unwrap();
        let mut x = aktivierung(spalten, 24);
        x[0] = i16::MIN;
        x[1] = i16::MAX;
        x[200] = -1;
        let g = spalten / GRUPPE;
        for z in 0..zeilen {
            let c = &p.codes[z * g * BYTES_JE_GRUPPE..(z + 1) * g * BYTES_JE_GRUPPE];
            let gt = &p.gitter[z * g * GITTER_JE_GRUPPE..(z + 1) * g * GITTER_JE_GRUPPE];
            let soll = zeile_skalar(c, gt, &x);
            assert_eq!(soll, dot_i8_i16(&w[z * spalten..(z + 1) * spalten], &x));
            assert_eq!(unsafe { sse2::zeile(c, gt, &x) }, soll, "Zeile {z}");
        }
        // Die groessten Werte und zufaellige Gitter ueber den ganzen Bereich.
        let r = zufall(2 * spalten, 25);
        let codes: Vec<u8> = (0..g * BYTES_JE_GRUPPE).map(|i| if i % 7 == 0 { 0xFF } else { r[i] as u8 }).collect();
        let gitter: Vec<u8> = (0..g)
            .flat_map(|k| match k % 3 {
                0 => [255u8, 0x80],
                1 => [0u8, 0x7F],
                _ => [r[k] as u8, (r[k] >> 8) as u8],
            })
            .collect();
        for x0 in [i16::MIN, i16::MAX] {
            let x = vec![x0; spalten];
            assert_eq!(unsafe { sse2::zeile(&codes, &gitter, &x) }, zeile_skalar(&codes, &gitter, &x), "x = {x0}");
        }
        let x = aktivierung(spalten, 26);
        assert_eq!(unsafe { sse2::zeile(&codes, &gitter, &x) }, zeile_skalar(&codes, &gitter, &x));
        // Und ueber die Weiche: die Matrix nimmt auf x86_64 diese Fassung.
        let m = Vierbitmatrix::neu(&codes, &gitter, 1, spalten).unwrap();
        assert_eq!(m.zeile_mal(0, &x), zeile_skalar(&codes, &gitter, &x));
    }

    #[test]
    fn jeder_lineare_kern_rechnet_dasselbe() {
        use crate::linear::*;
        let (zeilen, spalten) = (37usize, 384usize);
        let w8 = vierbitmatrix(zeilen, spalten, 31, &[1, 3, 6, 16]);
        let p = packen(&w8, spalten).unwrap();
        let v: Gewichtsmatrix<'_> = Vierbitmatrix::neu(&p.codes, &p.gitter, zeilen, spalten).unwrap().into();
        let shifts: Vec<u8> = (0..zeilen).map(|z| 6 + (z % 3) as u8).collect();
        let je_kanal: Vec<u8> = (0..zeilen).map(|z| 4 + (z % 5) as u8).collect();
        // Klein genug, dass nicht jede Ausgabe saettigt: Die Gewichte sind
        // hier dicht, nicht wie ternaere zu einem Drittel null.
        let eingaben: Vec<Vec<i16>> =
            (0..11).map(|i| aktivierung(spalten, 40 + i).iter().map(|&v| v >> 8).collect()).collect();
        let scheiben: Vec<&[i16]> = eingaben.iter().map(|v| v.as_slice()).collect();

        for x in &eingaben {
            let soll = linear_w8a16(x, &w8, spalten, &shifts, 3, 5);
            assert!(soll.iter().any(|&v| v != 0 && v != i16::MAX && v != i16::MIN), "nur Nullen oder Saettigung");
            assert_eq!(linear_matrix(x, v, spalten, &shifts, 3, 5), soll);
            assert_eq!(
                linear_matrix_pc(x, v, spalten, &shifts, 3, &je_kanal),
                linear_w8a16_pc(x, &w8, spalten, &shifts, 3, &je_kanal)
            );
        }
        assert_eq!(
            linear_matrix_stapel(&scheiben, v, spalten, &shifts, 3, 5),
            linear_w8a16_stapel(&scheiben, &w8, spalten, &shifts, 3, 5)
        );
        assert_eq!(
            linear_matrix_pc_stapel(&scheiben, v, spalten, &shifts, 3, &je_kanal),
            linear_w8a16_pc_stapel(&scheiben, &w8, spalten, &shifts, 3, &je_kanal)
        );
        fn teil<'a>(w: Gewichtsmatrix<'a>, x: &'a [i16], shifts: &'a [u8], aus: Ausgangsskala<'a>) -> Buendelteil<'a> {
            Buendelteil { w, x, in_features: x.len(), w_shifts: shifts, act_frac_bits: 3, aus }
        }
        let int8 = Gewichtsmatrix::Int8(&w8);
        let gemischt = linear_w8a16_buendel(&[
            teil(v, &eingaben[0], &shifts, Ausgangsskala::Eine(5)),
            teil(int8, &eingaben[1], &shifts, Ausgangsskala::JeZeile(&je_kanal)),
            teil(v, &eingaben[2], &shifts, Ausgangsskala::JeZeile(&je_kanal)),
        ]);
        let nur_int8 = linear_w8a16_buendel(&[
            teil(int8, &eingaben[0], &shifts, Ausgangsskala::Eine(5)),
            teil(int8, &eingaben[1], &shifts, Ausgangsskala::JeZeile(&je_kanal)),
            teil(int8, &eingaben[2], &shifts, Ausgangsskala::JeZeile(&je_kanal)),
        ]);
        assert_eq!(gemischt, nur_int8);
    }
}
