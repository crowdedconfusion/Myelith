//! Integer-Mathematik (sqrt, rsqrt, LUT-Lookup)
//!
//! ## ⚑ Fund 75: drei Vorbedingungen, und zwei tote Funktionen
//!
//! Bis zum 2026-08-28 hatte dieses Modul **keinen einzigen Test** und
//! keine dokumentierte Vorbedingung. Beim Nachsehen fielen zwei Dinge
//! auf, die zusammengehoeren:
//!
//! **1. Die Grenzen der Schiebeweiten standen nirgends.** `sqrt_q`
//! schiebt `x` um `frac_bits` nach links und braucht dafuer
//! `frac_bits <= 32`; `rsqrt_q` rechnet mit dem doppelten Wert und
//! braucht `frac_bits <= 31`; `lut_lookup` schiebt einen `i16` und
//! braucht `shift <= 15`.
//!
//! ⚑ **Der schlimmste Fall bricht nirgends ab.** `sqrt_q(i32::MAX, 33)`
//! liefert `0`, in **beiden** Bauprofilen. Der Linksschieber laesst die
//! oberen Bits fallen, ohne dass die Ueberlaufpruefung anspringt, denn
//! sie prueft die Schiebe*weite*, nicht den Wert. Erst ab `frac_bits =
//! 64` bricht es laut ab. Zwischen 33 und 63 liegt also ein Bereich, in
//! dem eine Wurzelfunktion still Null zurueckgibt.
//!
//! **2. `sqrt_q` und `rsqrt_q` haben keinen Aufrufer.** Nicht in diesem
//! Crate, nicht in `runtime`, nirgends im Repositorium; `rsqrt_q` ruft
//! `sqrt_q`, und das ist die einzige Kante. Beide sind trotzdem
//! oeffentlich. Die im Betrieb genutzte reziproke Wurzel ist
//! [`crate::fixed_point::inv_sqrt_q15`], die getestet ist, und die
//! genutzte Ganzzahlwurzel ist `isqrt_round` im selben Modul. Es gibt
//! also **drei** Ganzzahlwurzeln in diesem Crate, von denen zwei tot
//! sind.
//!
//! **Sie werden hier nicht entfernt**, weil das Loeschen oeffentlicher
//! Schnittstellen eine Entscheidung ist und kein Aufraeumen. Sie
//! bekommen aber, was jede Funktion im Rechenpfad braucht: eine
//! benannte Vorbedingung, eine Pruefung und einen Test. Solange sie
//! oeffentlich sind, kann sie jemand rufen.
//!
//! `lut_lookup` dagegen ist **nicht** tot: `mlp.rs`, `backward.rs` und
//! `layer_probe` rufen es, jeweils mit `shift = 0`.

use crate::fixed_point::clamp_i16;

/// Integer-Quadratwurzel via binaerer Suche.
/// Berechnet floor(sqrt(x * 2^frac_bits)) rein integer.
///
/// **Vorbedingung: `frac_bits <= 32`** (Fund 75). Darueber laesst
/// `(x as i64) << frac_bits` die oberen Bits fallen, und das Ergebnis
/// ist still falsch: `sqrt_q(i32::MAX, 33)` liefert `0`. Ab `64` bricht
/// der Shift ab.
///
/// ⚑ **Zweite Vorbedingung, gefunden am 2026-08-29 (Fund 95):
/// `x <= (2^31-1)^2 >> frac_bits`.** Darueber passt die Wurzel nicht
/// mehr in `i32`, und die Funktion liefert **still `i32::MAX`** statt
/// des richtigen Werts. Bei `frac_bits = 32` liegt die Grenze schon bei
/// `1_073_741_823`, also weit unterhalb von `i32::MAX`.
///
/// **Fund 75 hat acht Vorbedingungen des Ganzzahlpfades aufgeschrieben
/// und diese uebersehen**, weil sie durch Lesen gesucht wurden. Gefunden
/// hat sie ein Generator im ersten Lauf: `sqrt_q(1_764_347_202, 32)`
/// liefert `2_147_483_647`, und die richtige Antwort waere rund
/// `2_753_000_000` gewesen.
///
/// **Kein Aufrufer im Repositorium.** Siehe den Modulkopf.
#[inline]
pub fn sqrt_q(x: i32, frac_bits: u8) -> i32 {
    debug_assert!(
        frac_bits <= 32,
        "sqrt_q: frac_bits {} ueber der Grenze 32, das Ergebnis waere still falsch (Fund 75)",
        frac_bits
    );
    debug_assert!(
        (x as i64) <= (((i32::MAX as i64) * (i32::MAX as i64)) >> frac_bits),
        "sqrt_q: x {} zu gross fuer frac_bits {}, das Ergebnis saettigt still bei i32::MAX (Fund 95)",
        x,
        frac_bits
    );
    if x <= 0 { return 0; }
    let target = (x as i64) << (frac_bits as u32);

    let mut lo = 0i64;
    let mut hi = (target + 1).min(i32::MAX as i64);

    while lo < hi {
        let mid = (lo + hi + 1) >> 1;
        if mid > 0 && mid <= target / mid {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    lo as i32
}

/// Reziproke Quadratwurzel (1/sqrt(x)) als Fixed-Point.
/// Rein integer: (2^(2*frac_bits)) / sqrt(x * 2^frac_bits).
///
/// **Vorbedingung: `frac_bits <= 31`** (Fund 75), also eins strenger als
/// bei [`sqrt_q`], weil hier `1i64 << (2 * frac_bits)` gerechnet wird.
/// Bei `32` bricht der Shift ab.
///
/// **Kein Aufrufer im Repositorium.** Im Betrieb genutzt wird
/// [`crate::fixed_point::inv_sqrt_q15`].
#[inline]
pub fn rsqrt_q(x: i32, frac_bits: u8) -> i32 {
    debug_assert!(
        frac_bits <= 31,
        "rsqrt_q: frac_bits {} ueber der Grenze 31 (Fund 75)",
        frac_bits
    );
    if x <= 0 {
        return 1 << frac_bits;
    }
    let s = sqrt_q(x, frac_bits);
    if s == 0 {
        return 1 << frac_bits;
    }
    let val = (1i64 << (2 * frac_bits as u32)) / (s as i64);
    clamp_i16(val as i32) as i32
}

/// LUT-Lookup mit Index-Berechnung und Clamping.
///
/// **Drei Vorbedingungen** (Fund 75). Der Index wird in `i16` gerechnet,
/// und `i16` ist knapp:
///
/// 1. **`shift <= 15`**, sonst ist die Schiebeweite fuer `i16` zu gross.
/// 2. **`(x >> shift) + offset` passt in `i16`.** Die Addition ist
///    ungeschuetzt; im Debug-Bau bricht sie ab, im Release-Bau wrappt
///    sie, und dann greift das Clamping darunter auf den falschen Wert.
/// 3. **`1 <= lut.len() <= 32767`.** `lut.len() as i16` wrappt bei einer
///    laengeren Tabelle ins Negative, und `min(negativ)` ergibt einen
///    Index, den `as usize` in eine riesige Zahl verwandelt.
///
/// **Zu Punkt 2 gibt es zwei Lesarten im Crate, und keine der beiden
/// ist vollstaendig.** `backward.rs` saettigt den Eingang ausdruecklich
/// (`clamp_i16_sat`) mit der Begruendung, der LUT-Index duerfe nicht
/// wrappen. `mlp.rs` castet an derselben Stelle mit `as i16`,
/// ungesichert. Heute traegt beides, weil die kalibrierten
/// `gate_proj`-Skalen ueber alle vier Modelle zwischen 7 und 13 liegen
/// und `silu.input_frac_bits` bei 6, der Reskalierer also immer
/// verkleinert.
///
/// ⚑ **Der Schutz in `backward.rs` reicht aber auch dann nicht, wenn er
/// greift.** Gesaettigt wird dort auf `i16`, und der Offset kommt
/// **danach**: `32767 + 256` verlaesst `i16` erneut. Die einzige
/// richtige Saettigung ist die **in die LUT-Domaene**, also auf
/// `[-offset, len - 1 - offset]`. Wer den Fall je behebt, behebt ihn an
/// beiden Stellen und in dieser Domaene, nicht in `i16`. Belegt in
/// `mlp.rs`, Test
/// `der_ungesicherte_cast_macht_aus_gross_positiv_klein_negativ`.
#[inline(always)]
pub fn lut_lookup(x: i16, lut: &[i16], shift: u8, offset: i16) -> i16 {
    debug_assert!(shift <= 15, "lut_lookup: shift {} ueber der Grenze 15 (Fund 75)", shift);
    debug_assert!(
        !lut.is_empty() && lut.len() <= i16::MAX as usize,
        "lut_lookup: Tabellenlaenge {} ausserhalb 1..=32767 (Fund 75)",
        lut.len()
    );
    debug_assert!(
        ((x >> shift) as i32 + offset as i32) >= i16::MIN as i32
            && ((x >> shift) as i32 + offset as i32) <= i16::MAX as i32,
        "lut_lookup: Index {} + {} verlaesst i16 (Fund 75)",
        x >> shift,
        offset
    );
    let idx = (x >> shift) + offset;
    let idx = idx.max(0).min(lut.len() as i16 - 1) as usize;
    lut[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `sqrt_q` liefert floor der Wurzel: `s*s <= n < (s+1)*(s+1)`.
    ///
    /// Geprueft wird die **Regel**, nicht eine Liste getippter Paare:
    /// Zu jedem Ergebnis wird die definierende Ungleichung nachgerechnet.
    #[test]
    fn sqrt_q_liefert_floor_der_wurzel() {
        for &frac in &[0u8, 1, 4, 8, 16] {
            for &x in &[1i32, 2, 3, 4, 9, 10, 255, 256, 1000, 65_535, 1 << 20] {
                let s = sqrt_q(x, frac) as i64;
                let n = (x as i64) << frac;
                assert!(s * s <= n, "s*s > n bei x={} frac={} s={}", x, frac, s);
                assert!(
                    (s + 1) * (s + 1) > n,
                    "(s+1)^2 <= n bei x={} frac={} s={}",
                    x, frac, s
                );
            }
        }
    }

    #[test]
    fn sqrt_q_nimmt_nichtpositive_eingaben_als_null() {
        for &x in &[0i32, -1, -1000, i32::MIN] {
            assert_eq!(sqrt_q(x, 8), 0, "x={}", x);
        }
    }

    /// ⚑ Gegenprobe zu Fund 75: Die Grenze ist echt, und der Fehler
    /// dahinter ist still. Ohne die Pruefung liefert `sqrt_q` hier `0`,
    /// ohne abzubrechen, in beiden Bauprofilen.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "ueber der Grenze 32")]
    fn sqrt_q_ueber_der_grenze_bricht_ab_statt_still_null_zu_liefern() {
        let _ = sqrt_q(i32::MAX, 33);
    }

    #[test]
    fn rsqrt_q_ist_die_reziproke_wurzel_im_rahmen_der_aufloesung() {
        // rsqrt_q(x, f) ~ 2^(2f) / sqrt(x * 2^f) = 2^(1.5f) / sqrt(x).
        for &(x, frac) in &[(4i32, 8u8), (16, 8), (64, 8), (1, 8)] {
            let s = sqrt_q(x, frac) as i64;
            let erwartet = ((1i64 << (2 * frac as u32)) / s).clamp(-32768, 32767);
            assert_eq!(rsqrt_q(x, frac) as i64, erwartet, "x={} frac={}", x, frac);
        }
    }

    #[test]
    fn rsqrt_q_faengt_nichtpositive_eingaben_ab() {
        for &x in &[0i32, -5] {
            assert_eq!(rsqrt_q(x, 8), 1 << 8, "x={}", x);
        }
    }

    /// ⚑ Gegenprobe zu Fund 75: eins strenger als bei `sqrt_q`.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "ueber der Grenze 31")]
    fn rsqrt_q_ueber_der_grenze_bricht_ab() {
        let _ = rsqrt_q(4, 32);
    }

    #[test]
    fn lut_lookup_trifft_den_index_und_haelt_die_raender() {
        let lut: Vec<i16> = (0..16).collect();
        // Ohne Offset und ohne Shift ist der Index der Wert selbst.
        assert_eq!(lut_lookup(0, &lut, 0, 0), 0);
        assert_eq!(lut_lookup(7, &lut, 0, 0), 7);
        // Unter null wird auf den ersten, ueber die Laenge auf den
        // letzten Eintrag geklemmt.
        assert_eq!(lut_lookup(-5, &lut, 0, 0), 0);
        assert_eq!(lut_lookup(100, &lut, 0, 0), 15);
        // Der Offset verschiebt die Domaene.
        assert_eq!(lut_lookup(-3, &lut, 0, 8), 5);
        // Der Shift rastert sie.
        assert_eq!(lut_lookup(9, &lut, 2, 0), 2);
    }

    /// ⚑ Gegenprobe zu Fund 75, Punkt 2: Ohne die Pruefung wrappt der
    /// Index im Release-Bau und das Clamping darunter klemmt den
    /// falschen Wert.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "verlaesst i16")]
    fn lut_lookup_index_ueberlauf_bricht_ab() {
        let lut: Vec<i16> = (0..16).collect();
        let _ = lut_lookup(i16::MAX, &lut, 0, 1);
    }

    /// ⚑ Gegenprobe zu Fund 75, Punkt 1.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "ueber der Grenze 15")]
    fn lut_lookup_shift_ueber_der_grenze_bricht_ab() {
        let lut: Vec<i16> = (0..16).collect();
        let _ = lut_lookup(8, &lut, 16, 0);
    }

    /// ⚑ Gegenprobe zu Fund 75, Punkt 3: die leere Tabelle. Ohne die
    /// Pruefung ergibt `lut.len() as i16 - 1` den Wert `-1`, und
    /// `(-1) as usize` ist eine Indizierung weit jenseits der Tabelle.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Tabellenlaenge")]
    fn lut_lookup_leere_tabelle_bricht_ab() {
        let leer: Vec<i16> = Vec::new();
        let _ = lut_lookup(0, &leer, 0, 0);
    }
}
