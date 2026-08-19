//! Skalarprodukt int8 × int16 — der Kern jeder linearen Schicht.
//!
//! ## Warum ausgerechnet hier
//!
//! Das Operationsprofil (`bin/op_profile.rs`, 2026-08-19) hat gemessen,
//! wohin die Zeit geht:
//!
//! | Operation | Anteil |
//! |---|---|
//! | `linear_w8a16` (Layer + LM-Head) | **99,4 %** |
//! | rmsnorm | 0,4 % |
//! | rope + softmax | 0,15 % |
//!
//! Bis dahin waren im SIMD-Backend Softmax, RoPE und Attention
//! vektorisiert — zusammen 0,15 % der Laufzeit. Das erklärt, warum der
//! Durchsatz-Benchmark für `--features cpu-simd` keinen Vorteil zeigte:
//! Es war die falsche Operation optimiert, und niemand hatte nachgesehen.
//!
//! ## Warum Vektorisierung hier die Bitgleichheit nicht gefährdet
//!
//! Das ist der Punkt, an dem die Kernthese des Projekts für uns
//! arbeitet. Die Akkumulation läuft in `i64` und ist **exakt** — bei
//! höchstens `127 × 32767 ≈ 4,2 · 10⁶` je Produkt und realistisch
//! einigen Tausend Summanden bleibt sie um zehn Größenordnungen unter
//! dem i64-Bereich. Exakte Ganzzahladdition ist assoziativ und
//! kommutativ, also liefert **jede** Summationsreihenfolge dasselbe
//! Ergebnis.
//!
//! Die vektorisierte Fassung summiert in anderer Reihenfolge (paarweise,
//! über Lanes) und ist deshalb **per Konstruktion** bitgleich, nicht
//! bloß „getestet gleich". Genau dieselbe Eigenschaft trägt den Konsens
//! über verschiedene Hardware (Whitepaper Kap. 6.2). Bei Gleitkomma wäre
//! dieselbe Umstellung unzulässig.
//!
//! Der Paritätstest existiert trotzdem — eine Herleitung ersetzt keine
//! Messung, sie macht sie nur erwartbar.
//!
//! ## Warum kein AVX2 in diesem Patch
//!
//! Bewusste Auslassung. Diese Maschine ist aarch64; eine
//! AVX2-Implementierung ließe sich hier übersetzen, aber **nicht
//! ausführen und nicht auf Parität prüfen**. Unverifizierte Numerik in
//! einen Konsenspfad zu geben, ist die eine Sache, die dieses Projekt
//! sich nicht leisten kann — ein Miner mit abweichendem Kernel wird
//! beim Redundanzvergleich geslasht, ohne etwas falsch gemacht zu haben.
//! Der AVX2-Pfad ist als eigener Fahrplanpunkt vermerkt und gehört auf
//! echte x86_64-Hardware (Kritikpunkt K1, Fund A19).

/// Skalarprodukt zweier gleich langer Folgen: `Σ w[i] · x[i]`.
///
/// Akkumuliert exakt in `i64`. Ist `cpu-simd` aktiv und die Zielplattform
/// aarch64, läuft eine vektorisierte Fassung mit identischem Ergebnis
/// (siehe Modulkopf).
///
/// Verarbeitet werden `min(w.len(), x.len())` Elemente — dieselbe
/// Semantik wie das `zip` der Skalarfassung, damit der Austausch nichts
/// an den Aufrufstellen ändert.
#[inline]
pub fn dot_i8_i16(w: &[i8], x: &[i16]) -> i64 {
    #[cfg(all(feature = "cpu-simd", target_arch = "aarch64"))]
    {
        // NEON ist auf aarch64 Teil der Basis-Architektur, keine
        // Laufzeit-Erkennung nötig.
        return unsafe { neon::dot_neon(w, x) };
    }
    #[allow(unreachable_code)]
    dot_scalar(w, x)
}

/// Skalare Referenzfassung — der numerische Vertrag.
#[inline]
pub fn dot_scalar(w: &[i8], x: &[i16]) -> i64 {
    let mut acc: i64 = 0;
    for (a, b) in w.iter().zip(x.iter()) {
        acc += (*a as i64) * (*b as i64);
    }
    acc
}

#[cfg(all(feature = "cpu-simd", target_arch = "aarch64"))]
mod neon {
    use std::arch::aarch64::*;

    /// Sechzehn Produkte je Durchlauf auf vier unabhängigen
    /// i32-Akkumulatoren.
    ///
    /// **Warum vier und nicht einer:** Die erste Fassung akkumulierte mit
    /// `vpadalq_s32` in einen einzigen i64-Vektor und war **langsamer als
    /// die Skalarfassung** (12,4 gegen 18,9 tok/s). Ursache war nicht der
    /// Rechenaufwand, sondern die serielle Abhängigkeitskette: Jede
    /// Iteration wartete auf die vorige, sodass die Latenz der
    /// Akkumulation die Laufzeit bestimmte statt des Durchsatzes der
    /// Multiplikation. Mit vier unabhängigen Ketten liegen die
    /// Multiplikationen überlappend in der Pipeline.
    ///
    /// **Sicherheit:** Es werden ausschließlich Elemente innerhalb von
    /// `w[..n]` und `x[..n]` gelesen, mit `n = min(len)`; die Schleife
    /// bricht ab, bevor ein Sechzehnerblock über das Ende ragen würde,
    /// der Rest läuft skalar.
    ///
    /// **Exaktheit:** `vmlal_s16` addiert das volle 32-Bit-Produkt (kein
    /// Sättigen) auf den Akkumulator. Die i32-Akkumulatoren werden alle
    /// `BLOCK` Elemente nach i64 ausgeräumt, bevor sie überlaufen können —
    /// die Grenze ist unten hergeleitet.
    #[inline]
    pub unsafe fn dot_neon(w: &[i8], x: &[i16]) -> i64 {
        let n = w.len().min(x.len());
        // Vier unabhaengige i32-Akkumulatoren. Ein einziger haette eine
        // serielle Abhaengigkeitskette ueber die ganze Schleife — die
        // Latenz der Akkumulation, nicht der Durchsatz der Multiplikation,
        // bestimmt dann die Laufzeit.
        let mut a0 = vdupq_n_s32(0);
        let mut a1 = vdupq_n_s32(0);
        let mut a2 = vdupq_n_s32(0);
        let mut a3 = vdupq_n_s32(0);
        let mut gesamt: i64 = 0;
        let mut i = 0usize;

        // Nach je `BLOCK` Elementen werden die i32-Akkumulatoren nach i64
        // ausgeraeumt. Grenze: je Lane hoechstens `BLOCK/4` Produkte a
        // 4,2 Mio — bei BLOCK = 1024 sind das 256 * 4,2e6 = 1,07e9 und
        // damit sicher unter i32::MAX (2,15e9).
        const BLOCK: usize = 1024;

        while i + 16 <= n {
            let block_ende = (i + BLOCK).min(n);
            while i + 16 <= block_ende {
                let w0 = vmovl_s8(vld1_s8(w.as_ptr().add(i)));
                let x0 = vld1q_s16(x.as_ptr().add(i));
                let w1 = vmovl_s8(vld1_s8(w.as_ptr().add(i + 8)));
                let x1 = vld1q_s16(x.as_ptr().add(i + 8));

                a0 = vmlal_s16(a0, vget_low_s16(w0), vget_low_s16(x0));
                a1 = vmlal_s16(a1, vget_high_s16(w0), vget_high_s16(x0));
                a2 = vmlal_s16(a2, vget_low_s16(w1), vget_low_s16(x1));
                a3 = vmlal_s16(a3, vget_high_s16(w1), vget_high_s16(x1));
                i += 16;
            }
            gesamt += summe_i32x4(a0) + summe_i32x4(a1) + summe_i32x4(a2) + summe_i32x4(a3);
            a0 = vdupq_n_s32(0);
            a1 = vdupq_n_s32(0);
            a2 = vdupq_n_s32(0);
            a3 = vdupq_n_s32(0);
        }
        gesamt += summe_i32x4(a0) + summe_i32x4(a1) + summe_i32x4(a2) + summe_i32x4(a3);

        while i < n {
            gesamt += (*w.get_unchecked(i) as i64) * (*x.get_unchecked(i) as i64);
            i += 1;
        }
        gesamt
    }

    /// Horizontale Summe eines i32-Vektors, verlustfrei nach i64.
    #[inline]
    unsafe fn summe_i32x4(v: int32x4_t) -> i64 {
        let breit = vpaddlq_s32(v); // i32x4 -> i64x2, paarweise
        vgetq_lane_s64(breit, 0) + vgetq_lane_s64(breit, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministischer PRNG — die Testsuite soll ohne
    /// Zufalls-Abhängigkeit reproduzierbar sein.
    struct Xorshift(u64);
    impl Xorshift {
        fn next(&mut self) -> u64 {
            let mut v = self.0;
            v ^= v << 13;
            v ^= v >> 7;
            v ^= v << 17;
            self.0 = v;
            v
        }
    }

    fn paar(n: usize, seed: u64) -> (Vec<i8>, Vec<i16>) {
        let mut r = Xorshift(seed);
        let w = (0..n).map(|_| (r.next() % 256) as i8).collect();
        let x = (0..n).map(|_| (r.next() % 65536) as i16).collect();
        (w, x)
    }

    #[test]
    fn dot_stimmt_mit_der_handrechnung() {
        assert_eq!(dot_i8_i16(&[1, 2, 3], &[10, 20, 30]), 10 + 40 + 90);
        assert_eq!(dot_i8_i16(&[-1, 2], &[100, -50]), -100 - 100);
        assert_eq!(dot_i8_i16(&[], &[]), 0);
    }

    /// Die tragende Eigenschaft: Skalar und vektorisiert müssen
    /// **bitgleich** sein. Bei aktivem `cpu-simd` auf aarch64 vergleicht
    /// dieser Test zwei verschiedene Implementierungen, sonst eine mit
    /// sich selbst — beides ist richtig, nur unterschiedlich scharf.
    #[test]
    fn vektorisiert_ist_bitgleich_zur_skalarfassung() {
        // Längen um die Vektorbreite herum, damit der skalare Rest
        // jedes Mal eine andere Größe hat.
        for n in [0usize, 1, 3, 7, 8, 9, 15, 16, 17, 31, 64, 127, 896, 4864] {
            for seed in [1u64, 0xdead_beef, 42] {
                let (w, x) = paar(n, seed);
                assert_eq!(
                    dot_i8_i16(&w, &x),
                    dot_scalar(&w, &x),
                    "n={} seed={}",
                    n,
                    seed
                );
            }
        }
    }

    #[test]
    fn extremwerte_ueberlaufen_nicht() {
        // Der ungünstigste Fall: alle Produkte maximal und gleiches
        // Vorzeichen.
        let n = 4864;
        let w = vec![i8::MAX; n];
        let x = vec![i16::MAX; n];
        let erwartet = (i8::MAX as i64) * (i16::MAX as i64) * n as i64;
        assert_eq!(dot_i8_i16(&w, &x), erwartet);

        let w_min = vec![i8::MIN; n];
        assert_eq!(
            dot_i8_i16(&w_min, &x),
            (i8::MIN as i64) * (i16::MAX as i64) * n as i64
        );
    }

    #[test]
    fn ungleiche_laengen_verhalten_sich_wie_zip() {
        let w = [1i8, 2, 3, 4, 5];
        let x = [10i16, 20, 30];
        assert_eq!(dot_i8_i16(&w, &x), 10 + 40 + 90);
        assert_eq!(dot_i8_i16(&w[..2], &x), 10 + 40);
    }
}
