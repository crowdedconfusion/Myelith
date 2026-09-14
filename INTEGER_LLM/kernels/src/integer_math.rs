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
/// **Drei Vorbedingungen** (Fund 75, verschaerft mit Fund 349):
///
/// 1. **`shift <= 15`**, sonst ist die Schiebeweite fuer `i16` zu gross.
/// 2. **Der Index `(x >> shift) + offset` liegt in der Tabelle**, also in
///    `0..lut.len()`. Wer einen Eingang ausserhalb der Tabelle hat, muss
///    **vorher** entscheiden, was die Funktion dort ist; fuer die SiLU tut
///    das [`silu_nachschlagen`].
/// 3. **`1 <= lut.len() <= 32767`.**
///
/// 📌 **Fund 349 (2026-09-14): Die Vorbedingung stand an der falschen
/// Grenze.** Bis dahin verlangte sie nur, dass der Index in `i16` passt.
/// Die SiLU-Tabelle endet aber bei 8191 vor dem Versatz; alles zwischen
/// 8192 und 24 575 wurde **still** auf den letzten Eintrag geklemmt, und
/// erst darueber schlug die Zusicherung an. Eine Grenze, die mit der
/// Tabelle nichts zu tun hat, prueft nichts ueber die Tabelle.
///
/// 📌 **Und das Klemmen war an dieser Stelle eine Aussage ueber die
/// Funktion, die niemand getroffen hatte.** Fuer die SiLU heisst ein
/// Eingang ueber 128 nicht „128", sondern der Eingang selbst. Die
/// gemessenen Gate-Werte reichen bei `myelith-0.6b` bis 278 und bei
/// `myelith-4b` bis 192 (Fund 364); die Inferenz lag dort flach.
///
/// ⚑ **Die Klemme im Rumpf bleibt**, in `i32` gerechnet (Fund 348): Ein
/// Verstoss gegen Vorbedingung 2 im ausgelieferten Bau, in dem die
/// Zusicherung fehlt, liefert so wenigstens den Randwert statt eines
/// Wertes vom anderen Ende.
#[inline(always)]
pub fn lut_lookup(x: i16, lut: &[i16], shift: u8, offset: i16) -> i16 {
    debug_assert!(shift <= 15, "lut_lookup: shift {} ueber der Grenze 15 (Fund 75)", shift);
    debug_assert!(
        !lut.is_empty() && lut.len() <= i16::MAX as usize,
        "lut_lookup: Tabellenlaenge {} ausserhalb 1..=32767 (Fund 75)",
        lut.len()
    );
    let idx = (x >> shift) as i32 + offset as i32;
    debug_assert!(
        idx >= 0 && idx < lut.len() as i32,
        "lut_lookup: Index {} + {} liegt ausserhalb der Tabelle 0..{} (Fund 349)",
        x >> shift,
        offset,
        lut.len()
    );
    // 📌 **Fund 348: die Klemme sass hinter dem Ueberlauf.** Hier stand
    // die Addition in `i16`, und danach wurde geklemmt. `25412 + 8192`
    // lief auf `-31932` um, `.max(0)` machte daraus `0`, und die SiLU
    // lieferte die Antwort fuer den kleinsten Eingang statt fuer den
    // groessten. Seither in `i32` gerechnet und erst dann geklemmt.
    let idx = idx.clamp(0, lut.len() as i32 - 1) as usize;
    lut[idx]
}

/// **Die SiLU eines Wertes in der Tabellendomaene, auch jenseits der
/// Tabelle.** Eingang `x` auf `in_frac` Bruchstellen, Ausgabe auf
/// `out_frac`, als `i64`, weil sie oberhalb der Tabelle nicht mehr in
/// `i16` passt.
///
/// | Eingang | Ausgabe |
/// |---|---|
/// | oberhalb der Tabelle | `x`, reskaliert auf `out_frac` |
/// | in der Tabelle | der Tabellenwert |
/// | unterhalb der Tabelle | `0` |
///
/// ⚑ **Warum das die Funktion ist und keine Naeherung.** SiLU(x) ist
/// `x · σ(x)`. Die Tabelle deckt die reale Domaene −128 bis knapp 128
/// ab, und dort ist `σ` bei jeder hier darstellbaren Aufloesung schon 1
/// beziehungsweise 0 (der Fehler ist hoechstens `128 · e^−128`). Oberhalb
/// ist SiLU also die Identitaet, unterhalb null. Die Fortsetzung ist an
/// beiden Raendern **stetig**: Der letzte Tabelleneintrag ist der Eingang
/// selbst, der erste ist null. Dieselbe Zerlegung, mit der ganzzahlige
/// Verfahren nur den beschraenkten Faktor klemmen und mit dem Eingang
/// multiplizieren, statt die ganze Funktion zu klemmen.
///
/// ⚑ **Und der Rueckwaertspass rechnete schon so.** Sein Gradient wird aus
/// dieser Tabelle abgeleitet und saettigte am Rand auf Steigung 1 oben
/// und 0 unten, waehrend der Vorwaertspass flach bei 128 lag. Seit Fund
/// 349 sagen beide Paesse dasselbe (`backward::silu_ableitung_nachschlagen`).
#[inline]
pub fn silu_nachschlagen(x: i32, lut: &[i16], offset: i16, in_frac: u8, out_frac: u8) -> i64 {
    let oben = lut.len() as i32 - 1 - offset as i32;
    let unten = -(offset as i32);
    if x > oben {
        crate::fixed_point::rescale_i64(x as i64, in_frac, out_frac)
    } else if x < unten {
        0
    } else {
        // Zwischen `unten` und `oben` passt `x` in `i16`, denn beide
        // Grenzen tun es (Vorbedingung 3 und `offset: i16`).
        i64::from(lut_lookup(x as i16, lut, 0, offset))
    }
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
    #[cfg_attr(
        not(debug_assertions),
        ignore = "prueft eine debug_assert-Zusicherung; im Release laeuft sie nicht"
    )]
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
    #[cfg_attr(
        not(debug_assertions),
        ignore = "prueft eine debug_assert-Zusicherung; im Release laeuft sie nicht"
    )]
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
        assert_eq!(lut_lookup(15, &lut, 0, 0), 15);
        // Der Offset verschiebt die Domaene.
        assert_eq!(lut_lookup(-3, &lut, 0, 8), 5);
        // Der Shift rastert sie.
        assert_eq!(lut_lookup(9, &lut, 2, 0), 2);
    }

    /// ⚑ Gegenprobe zu Fund 75, Punkt 2, verschaerft mit Fund 349: Ein
    /// Index ausserhalb der Tabelle meldet sich, auch wenn er in `i16`
    /// passt.
    #[test]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "prueft eine debug_assert-Zusicherung; im Release laeuft sie nicht"
    )]
    #[should_panic(expected = "ausserhalb der Tabelle")]
    fn lut_lookup_index_ueberlauf_bricht_ab() {
        let lut: Vec<i16> = (0..16).collect();
        let _ = lut_lookup(i16::MAX, &lut, 0, 1);
    }

    /// ⚑ Gegenprobe zu Fund 75, Punkt 1.
    #[test]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "prueft eine debug_assert-Zusicherung; im Release laeuft sie nicht"
    )]
    #[should_panic(expected = "ueber der Grenze 15")]
    fn lut_lookup_shift_ueber_der_grenze_bricht_ab() {
        let lut: Vec<i16> = (0..16).collect();
        let _ = lut_lookup(8, &lut, 16, 0);
    }

    /// ⚑ Gegenprobe zu Fund 75, Punkt 3: die leere Tabelle. Ohne die
    /// Pruefung ergibt `lut.len() as i16 - 1` den Wert `-1`, und
    /// `(-1) as usize` ist eine Indizierung weit jenseits der Tabelle.
    #[test]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "prueft eine debug_assert-Zusicherung; im Release laeuft sie nicht"
    )]
    #[should_panic(expected = "Tabellenlaenge")]
    fn lut_lookup_leere_tabelle_bricht_ab() {
        let leer: Vec<i16> = Vec::new();
        let _ = lut_lookup(0, &leer, 0, 0);
    }

    /// Eine Tabelle, deren Werte man ohne Gleitkomma nachrechnen kann:
    /// Rampe ab null, also `max(0, i - offset) << (out - in)`. Ihr letzter
    /// Eintrag ist die Identitaet und ihr erster null, genau wie bei der
    /// echten SiLU-Tabelle.
    fn rampe(len: usize, offset: i16, in_frac: u8, out_frac: u8) -> Vec<i16> {
        (0..len as i32)
            .map(|i| ((i - offset as i32).max(0) << (out_frac - in_frac)) as i16)
            .collect()
    }

    /// Fund 349: Oberhalb der Tabelle setzt die SiLU die Identitaet fort,
    /// und zwar stetig am Rand.
    #[test]
    fn silu_setzt_oberhalb_der_tabelle_die_identitaet_fort() {
        let lut = rampe(512, 256, 6, 8);
        let oben = 511 - 256;
        assert_eq!(silu_nachschlagen(oben, &lut, 256, 6, 8), (oben as i64) << 2);
        assert_eq!(silu_nachschlagen(oben + 1, &lut, 256, 6, 8), ((oben + 1) as i64) << 2);
        assert_eq!(
            silu_nachschlagen(oben + 1, &lut, 256, 6, 8) - silu_nachschlagen(oben, &lut, 256, 6, 8),
            1 << 2,
            "am oberen Rand springt der Wert"
        );
        // Der Bergauflauf aus Fund 349 und ein Wert weit jenseits von i16.
        assert_eq!(silu_nachschlagen(25_412, &lut, 256, 6, 8), 25_412i64 << 2);
        assert_eq!(silu_nachschlagen(1 << 20, &lut, 256, 6, 8), (1i64 << 20) << 2);
    }

    #[test]
    fn silu_ist_unterhalb_der_tabelle_null() {
        let lut = rampe(512, 256, 6, 8);
        assert_eq!(silu_nachschlagen(-256, &lut, 256, 6, 8), 0);
        assert_eq!(silu_nachschlagen(-257, &lut, 256, 6, 8), 0);
        assert_eq!(silu_nachschlagen(-(1 << 20), &lut, 256, 6, 8), 0);
    }

    #[test]
    fn silu_ist_in_der_tabelle_der_tabellenwert() {
        let mut lut = rampe(512, 256, 6, 8);
        // Ein Eintrag, den die Rampe nicht hat: Wer ihn trifft, liest die
        // Tabelle und rechnet nicht selbst.
        lut[300] = 7;
        assert_eq!(silu_nachschlagen(300 - 256, &lut, 256, 6, 8), 7);
        for x in [-256, -1, 0, 1, 100, 255] {
            assert_eq!(silu_nachschlagen(x, &lut, 256, 6, 8), i64::from(lut[(x + 256) as usize]));
        }
    }

    /// Die Vorbedingung liegt jetzt an der Tabellengrenze, nicht an der
    /// von `i16` (Fund 349). Ein Index eins hinter der Tabelle meldet sich.
    #[test]
    #[should_panic(expected = "ausserhalb der Tabelle")]
    fn lut_lookup_meldet_einen_index_hinter_der_tabelle() {
        let lut = rampe(512, 256, 6, 8);
        let _ = lut_lookup(256, &lut, 0, 256);
    }

    /// Im ausgelieferten Bau ohne Zusicherungen bleibt die Klemme die
    /// letzte Sicherung: unter null der erste, ueber die Laenge der letzte
    /// Eintrag, auch fuer einen Index, der in `i16` ueberliefe (Fund 348).
    #[test]
    #[cfg_attr(debug_assertions, ignore = "im Pruefprofil meldet sich die Zusicherung vorher")]
    fn lut_lookup_klemmt_ohne_zusicherung_an_den_rand() {
        let lut: Vec<i16> = (0..16).collect();
        assert_eq!(lut_lookup(-5, &lut, 0, 0), 0);
        assert_eq!(lut_lookup(100, &lut, 0, 0), 15);
        assert_eq!(lut_lookup(i16::MAX, &lut, 0, 1), 15);
    }
}
