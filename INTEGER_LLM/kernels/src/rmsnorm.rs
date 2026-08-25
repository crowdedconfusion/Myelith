//! RMSNorm – Integer-Implementierung (theta_v 0.5.0)
//!
//! Eingang: int16-Residualstrom; Ausgang: int16-Aktivierung auf einer
//! kalibrierten Per-Layer-Skala. Reine Ganzzahlarithmetik ohne Division im
//! Hot-Path (spec: shift_semantics = arithmetic_right_shift).
// Die Kernel-Signaturen tragen den vollstaendigen Fixed-Point-Vertrag:
// Eingangs- und Ausgangs-frac_bits, Per-Channel-Shifts, LUT-Parameter.
// In eine Parameter-Struct gefasst waere die Entsprechung zu den
// Referenzformeln (Whitepaper Anhang B) beim Nachrechnen nicht mehr
// ablesbar — und genau dieses Nachrechnen ist die Pruefmethode des
// Projekts. Bewusste Abweichung von clippy::too_many_arguments.
#![allow(clippy::too_many_arguments)]

use crate::fixed_point::{clamp_i16, rshift_round_i128};

/// Reziproken-Konstante 2^20 / n (gerundet) — einmalige Initialisierung,
/// NICHT Teil des tokenweisen Hot-Path. Damit wird der Mittelwert im
/// Inferenzpfad selbst divisionsfrei: mean ≈ (sum * inv_n_q20) >> 20.
#[inline]
pub fn inv_n_q20(n: usize) -> i64 {
    assert!(n > 0, "inv_n_q20: n muss > 0 sein");
    ((1i64 << 20) + (n as i64) / 2) / (n as i64)
}

/// RMSNorm über den int16-Residualstrom mit LUT-gestütztem rsqrt.
/// Ausgang: int16-Aktivierung auf der kalibrierten Per-Layer-Skala
/// `out_frac_bits` (Numerik-Realitätsabgleich v0.12.20: Aktivierungen sind
/// int16, da reale RMSNorm-Ausgaben den int8-Bereich sprengen).
///
/// **Fund 20 (2026-08-18): `x_shifts` — Per-Channel-Eingangsskalen.** Ab
/// Qwen2.5-7B tragen wenige feste Kanäle des Residualstroms an Position 0
/// "Massive Activations" (Sun et al. 2024): absmax ~9600 gegenüber ~10 im
/// Rest — mit EINER Skala fürs ganze Segment (wie bis v0.12.43) zwingt der
/// Ausreißer alle 3584 Kanäle auf Schrittweite 0,5, das eigentliche Signal
/// wird zu Brei. Seit theta_v 0.11.0 trägt der Residualstrom deshalb, wie
/// die Gewichte seit v0.12.25, eine Skala JE KANAL.
///
/// Mathematik (alle Größen ganzzahlig, deterministisch):
/// - Sei `ref = min(x_shifts)` (i. d. R. der Ausreißer-Kanal). Für die
///   Varianzsumme werden alle `x_i²` auf diese gemeinsame Skala
///   ausgerichtet: `sq_i = x_i² >> 2·(x_shifts[i] − ref)`. Kanäle mit
///   höherem Shift (feinere Auflösung, kleinerer Realwert) tragen dabei
///   weniger zur Summe bei — korrekt, denn ihr Quadrat ist bei genügend
///   kleinerem Realwert tatsächlich vernachlässigbar gegenüber dem
///   Ausreißer.
/// - `M = mean(sq_i)` via `(sum * inv_n_q20) >> 20`, referenziert auf `ref`
///   (bei uniformen Shifts identisch zur alten Definition — dann ist
///   `ref` der gemeinsame Shift und alle `x_shifts[i] − ref = 0`, also
///   `sq_i = x_i²` wie zuvor).
/// - Dynamischer gerader Index-Shift `q` wie zuvor (spec:
///   rsqrt.index_normalization = "dynamic_even_shift"), angewandt auf das
///   `ref`-referenzierte `M`.
/// - `lut[idx]` liefert wie zuvor `round(rsqrt(...) * 2^lut_output_frac)`.
/// - **Ausgabe pro Kanal:** `total_frac_i = norm_frac + gamma_shifts[i] +
///   (x_shifts[i] − ref)`. Der letzte Term kompensiert, dass `x[i]` hier
///   in VOLLER Auflösung (nicht auf `ref` heruntergerundet) multipliziert
///   wird — bei uniformen Shifts ist er 0 und die Formel ist bitgleich zur
///   Vorversion (siehe `test_rmsnorm_per_channel_uniform_shifts_matches_legacy`).
/// - eps (HF: 1e-6) rundet bei realistischen Residualskalen auf 0; der Fall
///   M = 0 liefert explizit Nullen (identisch zu HF: 0/sqrt(eps) = 0).
pub fn rmsnorm_i16(
    x: &[i16],
    x_shifts: &[u8],
    gamma: &[i8],
    gamma_shifts: &[u8],
    rsqrt_lut: &[i16],
    lut_input_shift: u8,
    lut_output_frac: u8,
    inv_n_q20: i64,
    out_frac_bits: u8,
) -> Vec<i16> {
    let n = x.len();
    assert_eq!(n, gamma.len(), "rmsnorm_i16: x und gamma muessen gleich lang sein");
    assert_eq!(n, gamma_shifts.len(), "rmsnorm_i16: ein Gamma-Shift je Element (theta_v 0.7.0)");
    assert_eq!(n, x_shifts.len(), "rmsnorm_i16: ein Eingangs-Shift je Kanal (Fund 20, theta_v 0.11.0)");
    assert!(lut_input_shift.is_multiple_of(2), "rmsnorm_i16: lut_input_shift muss gerade sein (Halb-Bit-Faktor)");

    // **Fund 24 (2026-08-19): Ausrichtung nach OBEN statt nach unten.**
    //
    // Die Quadratsumme muss alle Kanaele auf eine gemeinsame Skala
    // bringen. Bis theta_v 0.13.0 geschah das gegen den KLEINSTEN Shift,
    // also per Rechtsshift: `sq >> 2*(x_shifts[i] - min)`. Bei breiter
    // Shift-Spanne loescht das die feinskalierten Kanaele vollstaendig
    // aus — bei Qwen2.5-7B (Spanne 2..10, also align bis 16) trug ein
    // normaler Kanal statt 160 000 nur noch 2 zur Summe bei. Die Varianz
    // stammte dann praktisch nur noch aus den groben Ausreisser-Kanaelen,
    // und die Normalisierung war entsprechend falsch. Bei 0,5B (Spanne
    // 7..12) blieb der Effekt mild — deshalb fiel er dort nicht auf und
    // Fund 20 sah wie eine Verbesserung aus (15,59 -> 15,29), waehrend er
    // 7B von 16,26 auf 40,48 verschlechterte.
    //
    // Richtig ist die Ausrichtung gegen den GROESSTEN Shift per
    // Linksshift: dabei geht kein Bit verloren. Der Akkumulator ist
    // i128, damit die Verschiebung nicht ueberlaeuft (sq <= 2^30,
    // Linksshift bis 2*Spanne, Summe ueber n Kanaele).
    let ref_shift = *x_shifts.iter().max().expect("rmsnorm_i16: x_shifts darf nicht leer sein");

    let mut acc: i128 = 0;
    for i in 0..n {
        let align = 2 * (ref_shift - x_shifts[i]) as u32;
        let sq = (x[i] as i128) * (x[i] as i128);
        acc += sq << align;
    }
    if acc == 0 {
        return vec![0i16; n];
    }

    // Mittelwert ohne Division: Multiplikation mit Reziproken-Konstante.
    // M traegt jetzt die Skala 2^(2*ref_shift) statt 2^(2*min).
    //
    // M bleibt i128: Bei breiter Shift-Spanne (bis MAX_FRAC_BITS = 20,
    // also Linksshift bis 40) erreicht `acc` Groessenordnungen jenseits
    // von i64. Ein Rueckcast auf i64 wuerde dort WRAPPEN und ueber
    // `as usize` einen absurden LUT-Index erzeugen — genau das faengt
    // `test_rmsnorm_extremer_shift_bereich_laeuft_nicht_ueber` ab.
    let m: i128 = (acc * inv_n_q20 as i128) >> 20;

    // Dynamischer gerader Index-Shift in den LUT-Bereich.
    let max_idx = (rsqrt_lut.len() - 1) as i128;
    let mut q: u32 = 0;
    while (m >> q) > max_idx {
        q += 2;
    }
    let idx = rshift_round_i128(m, q).min(max_idx).max(0) as usize;

    let lut_val = rsqrt_lut[idx] as i64;
    let norm_frac = lut_output_frac as u32 + lut_input_shift as u32 / 2 + q / 2;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        // x_shifts[i] - ref_shift ist seit Fund 24 <= 0 (ref ist das
        // MAXIMUM), deshalb vorzeichenbehaftet rechnen — in u8 wuerde die
        // Differenz unterlaufen.
        let total_frac = norm_frac as i32 + gamma_shifts[i] as i32
            + (x_shifts[i] as i32 - ref_shift as i32);
        let shift = total_frac - out_frac_bits as i32;
        // Der Linksshift laeuft in i128, nicht in i64: seit Fund 24 ist
        // `x_shifts[i] - ref_shift` immer <= 0, negative Gesamt-Shifts sind
        // also der Regelfall statt der Ausnahme. `prod` erreicht 2^37
        // (32767 * 32767 * 127); ab Verschiebung 26 waere i64 uebergelaufen
        // und haette GEWRAPPT — das verbietet der numerische Vertrag
        // ausdruecklich (spec: overflow.behavior = "explicit_clamp_only",
        // wrap = false). i128 traegt den Fall mit grossem Abstand; die
        // Saettigung geschieht danach explizit ueber clamp_i16.
        //
        // Ein Linksshift ist keine Division, sondern eine exakte
        // Multiplikation mit 2^k — die Festlegung des Whitepapers auf den
        // arithmetischen Rechtsshift (Kap. 6.2, Anhang B.5.4) betrifft
        // ausschliesslich die Division und ihre Rundungsmehrdeutigkeit bei
        // negativen Zahlen. Rundungsfrei und plattformgleich bleibt der
        // Linksshift; einzig der Ueberlauf musste abgesichert werden.
        let prod = (x[i] as i128) * (lut_val as i128) * (gamma[i] as i128);
        let skaliert: i128 = if shift >= 0 {
            rshift_round_i128(prod, shift as u32)
        } else {
            prod << (-shift) as u32
        };
        out.push(clamp_i16(skaliert.clamp(i32::MIN as i128, i32::MAX as i128) as i32));
    }
    out
}

/// QK-Norm: RMSNorm je Attention-Kopf, wie Qwen3 sie vor RoPE anwendet.
///
/// **Es ist keine neue Rechnung.** QK-Norm ist RMSNorm über `head_dim`
/// statt über `hidden_size`, mit einem Gamma der Länge `head_dim`, das
/// sich **alle Köpfe teilen**. [`rmsnorm_i16`] ist bereits allgemein
/// genug; diese Funktion legt nur fest, worüber normiert wird. Das ist
/// die ganze Zutat, die Qwen3 gegenüber Qwen2.5 im Attention-Zweig
/// braucht, und sie war der einzige Struktur-Blocker des Modellwechsels.
///
/// ⚑ **Kopfweise, nicht über den flachen Vektor.** Der Unterschied ist
/// nicht kosmetisch: Über `num_heads · head_dim` normiert, ginge die
/// Varianz **aller** Köpfe in **jeden** Kopf ein. Ein Kopf mit großen
/// Werten drückte alle anderen herunter, und zwar abhängig davon, was
/// die anderen Köpfe gerade enthalten. Das Ergebnis für Kopf 0 hinge
/// dann an Kopf 7. Der Test
/// `ein_kopf_aendert_die_anderen_nicht` hält genau das fest.
///
/// **Zur Eingangsskala.** `q_flat` und `k_flat` tragen eine Skala je
/// Layer (`q_frac`/`k_frac`), keine je Kanal. Das ist der Unterschied
/// zum Residualstrom, der seit Fund 20 eine Skala je Kanal braucht, und
/// er ist hier richtig: Die „Massive Activations" sind eine Eigenschaft
/// des Residualstroms, nicht der projizierten Q/K.
///
/// **Offen und hier vermerkt statt verschwiegen:** QK-Norm existiert in
/// Qwen3 gerade deshalb, weil die Attention-Logits ohne sie davonlaufen.
/// Ob die projizierten Q/K bei diesem Modell weiterhin mit **einer**
/// Skala je Layer auskommen, ist eine Messfrage und beim ersten
/// Qwen3-Artefakt zu prüfen. Fällt die Antwort negativ aus, nimmt diese
/// Funktion `x_shifts` je Kanal entgegen, ohne dass sich sonst etwas
/// ändert: [`rmsnorm_i16`] kann es bereits.
#[allow(clippy::too_many_arguments)]
pub fn qk_norm_heads(
    heads: &mut [Vec<i16>],
    x_frac: u8,
    gamma: &[i8],
    gamma_shifts: &[u8],
    rsqrt_lut: &[i16],
    lut_input_shift: u8,
    lut_output_frac: u8,
    out_frac_bits: u8,
) {
    let head_dim = gamma.len();
    // Hängt nur an head_dim, also einmal statt je Kopf.
    let inv_n = inv_n_q20(head_dim);
    let x_shifts = vec![x_frac; head_dim];
    for kopf in heads.iter_mut() {
        debug_assert_eq!(
            kopf.len(),
            head_dim,
            "qk_norm_heads: jeder Kopf traegt head_dim Elemente"
        );
        *kopf = rmsnorm_i16(
            kopf,
            &x_shifts,
            gamma,
            gamma_shifts,
            rsqrt_lut,
            lut_input_shift,
            lut_output_frac,
            inv_n,
            out_frac_bits,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Baut eine rsqrt-LUT im spec-Format (input_shift 8, output frac 8):
    /// lut[x] = round(2^12 / sqrt(x)), Sentinel lut[0] = 2^8.
    fn spec_lut(len: usize) -> Vec<i16> {
        let mut lut = Vec::with_capacity(len);
        for x in 0..len {
            if x == 0 {
                lut.push(256);
            } else {
                let val = 4096.0 / (x as f64).sqrt();
                lut.push(val.round() as i16);
            }
        }
        lut
    }

    // -----------------------------------------------------------------
    // QK-Norm (Qwen3)
    // -----------------------------------------------------------------

    fn qk_gamma(head_dim: usize) -> (Vec<i8>, Vec<u8>) {
        // gamma = 1.0 in der Skala 2^6.
        (vec![64i8; head_dim], vec![6u8; head_dim])
    }

    /// ⚑ **Der Test, gegen den `qk_norm_heads` geschrieben ist.** Wer
    /// über den flachen Vektor normiert statt kopfweise, besteht jeden
    /// anderen Test dieser Datei und faellt hier durch: Kopf 0 aendert
    /// sich dann, obwohl nur Kopf 1 angefasst wurde.
    #[test]
    fn ein_kopf_aendert_die_anderen_nicht() {
        let lut = spec_lut(1024);
        let (gamma, gshift) = qk_gamma(4);

        // Kopf 1 unterscheidet sich in der **Richtung**, nicht nur im
        // Betrag: RMSNorm ist skaleninvariant, zwei konstante Vektoren
        // verschiedener Groesse kaemen identisch heraus und der zweite
        // Vergleich unten pruefte nichts.
        let mut a = vec![vec![10i16, 20, 30, 40], vec![5i16, 5, 5, 5]];
        let mut b = vec![vec![10i16, 20, 30, 40], vec![3000i16, 100, 7, -50]];

        qk_norm_heads(&mut a, 6, &gamma, &gshift, &lut, 8, 8, 6);
        qk_norm_heads(&mut b, 6, &gamma, &gshift, &lut, 8, 8, 6);

        assert_eq!(
            a[0], b[0],
            "Kopf 0 darf nicht davon abhaengen, was in Kopf 1 steht"
        );
        assert_ne!(a[1], b[1], "Kopf 1 selbst muss sich sehr wohl unterscheiden");
    }

    /// Gegenprobe zur Gegenprobe: Ueber den flachen Vektor normiert,
    /// waere Kopf 0 tatsaechlich betroffen. Ohne diesen Nachweis
    /// koennte der Test darueber eine Eigenschaft pruefen, die ohnehin
    /// gilt.
    #[test]
    fn ueber_den_flachen_vektor_waere_kopf_null_betroffen() {
        let lut = spec_lut(1024);
        let flach_a = [10i16, 20, 30, 40, 5, 5, 5, 5];
        let flach_b = [10i16, 20, 30, 40, 3000, 3000, 3000, 3000];
        let shifts = vec![6u8; 8];
        let gamma = vec![64i8; 8];
        let gshift = vec![6u8; 8];

        let out_a = rmsnorm_i16(&flach_a, &shifts, &gamma, &gshift, &lut, 8, 8, inv_n_q20(8), 6);
        let out_b = rmsnorm_i16(&flach_b, &shifts, &gamma, &gshift, &lut, 8, 8, inv_n_q20(8), 6);

        assert_ne!(
            out_a[..4],
            out_b[..4],
            "genau dieser Unterschied ist der Fehler, den qk_norm_heads vermeidet"
        );
    }

    #[test]
    fn qk_norm_normiert_jeden_kopf_auf_dieselbe_groesse() {
        let lut = spec_lut(1024);
        let (gamma, gshift) = qk_gamma(4);
        // Zwei Koepfe, gleiche Richtung, um den Faktor 100 verschieden.
        let mut heads = vec![vec![1i16, 1, 1, 1], vec![100i16, 100, 100, 100]];
        qk_norm_heads(&mut heads, 6, &gamma, &gshift, &lut, 8, 8, 6);
        assert_eq!(
            heads[0], heads[1],
            "RMSNorm ist skaleninvariant: beide Koepfe muessen gleich herauskommen"
        );
    }

    #[test]
    fn qk_norm_laesst_den_nullkopf_null() {
        let lut = spec_lut(1024);
        let (gamma, gshift) = qk_gamma(4);
        let mut heads = vec![vec![0i16; 4]];
        qk_norm_heads(&mut heads, 6, &gamma, &gshift, &lut, 8, 8, 6);
        assert_eq!(heads[0], vec![0i16; 4]);
    }

    #[test]
    fn qk_norm_ist_wiederholbar() {
        let lut = spec_lut(1024);
        let (gamma, gshift) = qk_gamma(8);
        let vorlage: Vec<Vec<i16>> = (0..3)
            .map(|h| (0..8).map(|i| ((h * 8 + i) as i16) * 37 - 100).collect())
            .collect();
        let mut a = vorlage.clone();
        let mut b = vorlage;
        qk_norm_heads(&mut a, 5, &gamma, &gshift, &lut, 8, 8, 6);
        qk_norm_heads(&mut b, 5, &gamma, &gshift, &lut, 8, 8, 6);
        assert_eq!(a, b);
    }

    #[test]
    fn test_inv_n_q20() {
        // (2^20 + 448) / 896 = 1170 (Integerdivision, gerundete Reziproke).
        assert_eq!(inv_n_q20(896), 1170);
        assert_eq!(inv_n_q20(1), 1 << 20);
    }

    #[test]
    fn test_rmsnorm_zero_input() {
        let lut = spec_lut(1024);
        let out = rmsnorm_i16(&[0, 0, 0], &[0, 0, 0], &[64, 64, 64], &[6, 6, 6], &lut, 8, 8, inv_n_q20(3), 6);
        assert_eq!(out, vec![0, 0, 0]);
    }

    #[test]
    fn test_rmsnorm_constant_input_normalizes_to_one() {
        // Alle x gleich -> mean(x^2) = x^2 -> normalisierter Wert ±1.
        // gamma = 1.0 (shift 6 -> 64), out_frac 6 -> erwartet ±64.
        let lut = spec_lut(32768);
        let out = rmsnorm_i16(&[16, 16], &[0, 0], &[64, 64], &[6, 6], &lut, 8, 8, inv_n_q20(2), 6);
        assert_eq!(out, vec![64, 64]);
        let out_neg = rmsnorm_i16(&[-16, -16], &[0, 0], &[64, 64], &[6, 6], &lut, 8, 8, inv_n_q20(2), 6);
        assert_eq!(out_neg, vec![-64, -64]);
    }

    #[test]
    fn test_rmsnorm_large_input_uses_dynamic_q() {
        // x = 12000 -> M = 1.44e8 > 32767 -> q > 0 noetig. Ergebnis muss
        // trotzdem ±1 * gamma sein (Normalisierung), innerhalb LUT-Rundung.
        let lut = spec_lut(32768);
        let out = rmsnorm_i16(&[12000, 12000], &[0, 0], &[32, 32], &[5, 5], &lut, 8, 8, inv_n_q20(2), 3);
        // ±1.0 bei frac 3 = ±8; LUT-/Indexrundung erlaubt ±1 Abweichung.
        assert!((out[0] - 8).abs() <= 1, "out[0] = {}", out[0]);
        assert!((out[1] - 8).abs() <= 1, "out[1] = {}", out[1]);
    }

    #[test]
    fn test_rmsnorm_two_values_hand_computed() {
        // x = [16, 0] -> M = (256 + 0)/2 = 128 -> sqrt(M) = 11.3137
        // normalisiert: [16/11.3137, 0] = [1.4142, 0]; gamma 1.0 (shift 5: 32)
        // out_frac 6: [round(1.4142*64), 0] = [90 oder 91, 0]
        let lut = spec_lut(32768);
        let out = rmsnorm_i16(&[16, 0], &[0, 0], &[32, 32], &[5, 5], &lut, 8, 8, inv_n_q20(2), 6);
        assert!(out[0] == 90 || out[0] == 91, "out[0] = {}", out[0]);
        assert_eq!(out[1], 0);
    }

    #[test]
    fn test_rmsnorm_gamma_scaling() {
        // gamma 2.0 (shift 5 -> 64) verdoppelt das Ergebnis gegenueber 1.0.
        // (i16-Ausgang: 2.0 bei frac 6 = 128, kein i8-Clamping mehr.)
        let lut = spec_lut(32768);
        let one = rmsnorm_i16(&[16, 16], &[0, 0], &[32, 32], &[5, 5], &lut, 8, 8, inv_n_q20(2), 6);
        let two = rmsnorm_i16(&[16, 16], &[0, 0], &[64, 64], &[5, 5], &lut, 8, 8, inv_n_q20(2), 6);
        assert_eq!(one, vec![64, 64]);
        assert_eq!(two, vec![128, 128]);
    }

    #[test]
    fn test_rmsnorm_gamma_per_element_shifts() {
        // Unterschiedliche Gamma-Shifts je Element (theta_v 0.7.0):
        // gamma[0] = 32 mit Shift 5 (= 1.0), gamma[1] = 32 mit Shift 4 (= 2.0)
        // -> Element 1 wird verdoppelt.
        let lut = spec_lut(32768);
        let out = rmsnorm_i16(&[16, 16], &[0, 0], &[32, 32], &[5, 4], &lut, 8, 8, inv_n_q20(2), 6);
        assert_eq!(out[0], 64);  // 1.0 * 1.0
        assert_eq!(out[1], 128); // 1.0 * 2.0
    }

    #[test]
    fn test_rmsnorm_per_channel_uniform_shifts_matches_legacy() {
        // Fund 20: jeder beliebige UNIFORME x_shifts-Wert (nicht nur 0) muss
        // dasselbe Ergebnis liefern wie die alte Skalar-Formel - das ist die
        // Eigenschaft, auf der die Gueltigkeit aller vor v0.12.44
        // kalibrierten (und weiterhin per-tensor behandelten) Artefakte
        // beruht.
        let lut = spec_lut(32768);
        let referenz = rmsnorm_i16(&[3000, -500, 7, 12000], &[0, 0, 0, 0],
            &[32, 40, 20, 8], &[5, 5, 5, 5], &lut, 8, 8, inv_n_q20(4), 6);
        for s in [1u8, 5, 9, 14] {
            let out = rmsnorm_i16(&[3000, -500, 7, 12000], &[s, s, s, s],
                &[32, 40, 20, 8], &[5, 5, 5, 5], &lut, 8, 8, inv_n_q20(4), 6);
            assert_eq!(out, referenz, "uniform shift {} weicht ab", s);
        }
    }

    #[test]
    fn test_rmsnorm_massive_activation_outlier_normalizes_correctly() {
        // Der reale Fall, der Fund 20 ausgeloest hat: ein Kanal (Position 0
        // im echten Residualstrom) mit absmax ~9600 (shift 1), drei Kanaele
        // mit absmax ~1 (shift 12, volle Aufloesung). Reale Werte:
        // Kanal 0 = 9600*2^-1 = 4800.0, Kanal 1..3 = 0.25 / -0.5 / 0.75.
        //
        // n=4: mean(x^2) ~ 4800^2/4 (die drei winzigen Kanaele sind
        // vernachlaessigbar), sqrt(mean) ~ 2400 = Kanal0/2 -> Kanal 0
        // normalisiert auf ungefaehr ±2.0 (nicht ±1.0 - das Teilen durch n
        // ist keine Ausreisser-Eigenschaft, nur Arithmetik).
        let x = vec![9600i16, 1024, -2048, 3072];
        let x_shifts = vec![1u8, 12, 12, 12];
        let lut = spec_lut(32768);
        let out = rmsnorm_i16(&x, &x_shifts, &[64, 64, 64, 64], &[6, 6, 6, 6],
            &lut, 8, 8, inv_n_q20(4), 8);

        let real0 = out[0] as f64 / 256.0; // out_frac_bits = 8
        assert!((real0.abs() - 2.0).abs() < 0.1, "Kanal 0 real={}", real0);
    }

    #[test]
    fn test_rmsnorm_per_channel_shift_representation_invariance() {
        // Die eigentliche Korrektheitsprobe fuer Fund 20: dieselben REALEN
        // Werte, aber auf zwei verschiedene Arten kodiert (unterschiedliche
        // Shift-Wahl je Kanal), muessen dasselbe Ergebnis liefern. Ein Fehler
        // in der Ausrichtung der Quadratsummen auf eine gemeinsame
        // Referenzskala (ref_shift-Logik) wuerde hier sichtbar - anders als
        // beim reinen "kollabiert auf 0"-Test, der nur mathematisch triviale
        // Grosse/Klein-Verhaeltnisse zeigt, prueft dieser Test die Kernidee
        // der Implementierung.
        //
        // Kodierung A: Kanal 0 real=4800 bei shift=1 (raw=9600), Kanaele
        // 1..3 real=0.25/-0.5/0.75 bei shift=12 (raw=1024/-2048/3072).
        // Kodierung B: DIESELBEN Realwerte, aber shift=2 fuer Kanal 0
        // (raw=19200) und shift=10 fuer die uebrigen (raw=256/-512/768).
        let lut = spec_lut(32768);
        let gamma = [64i8, 64, 64, 64];
        let gamma_shifts = [6u8, 6, 6, 6];

        let out_a = rmsnorm_i16(&[9600, 1024, -2048, 3072], &[1, 12, 12, 12],
            &gamma, &gamma_shifts, &lut, 8, 8, inv_n_q20(4), 8);
        let out_b = rmsnorm_i16(&[19200, 256, -512, 768], &[2, 10, 10, 10],
            &gamma, &gamma_shifts, &lut, 8, 8, inv_n_q20(4), 8);

        assert_eq!(out_a, out_b,
            "gleiche Realwerte, verschiedene Shift-Kodierung -> muss gleiches Ergebnis liefern: {:?} vs {:?}",
            out_a, out_b);
    }

    #[test]
    fn test_rmsnorm_breite_shift_spanne_verliert_keine_kanaele() {
        // **Fund 24.** Die Varianzsumme muss alle Kanaele auf eine
        // gemeinsame Skala bringen. Wird dafuer nach UNTEN ausgerichtet
        // (Rechtsshift gegen min(shifts)), verschwinden feinskalierte
        // Kanaele bei breiter Spanne vollstaendig aus der Summe: bei
        // Qwen2.5-7B (Spanne 2..10) trug ein normaler Kanal statt 160 000
        // nur noch 2 bei, und die Normalisierung stuetzte sich fast
        // ausschliesslich auf die groben Ausreisser-Kanaele.
        //
        // Der Test haelt die Eigenschaft fest, die das ausschliesst: zwei
        // Kanaele mit GLEICHEM Realwert, aber unterschiedlicher
        // Shift-Kodierung, muessen denselben normalisierten Ausgang
        // liefern — unabhaengig davon, wie breit die Spanne im Vektor ist.
        let lut = spec_lut(32768);
        let gamma = [64i8; 4];
        let gamma_shifts = [6u8; 4];

        // Kanal 1 und 3 tragen beide den Realwert 1.0, aber mit Shift 10
        // bzw. Shift 2 kodiert. Kanal 0 ist der Ausreisser (Realwert 2400).
        let x = vec![9600i16, 1024, 4800, 4];
        let x_shifts = vec![2u8, 10, 1, 2];
        let out = rmsnorm_i16(&x, &x_shifts, &gamma, &gamma_shifts,
            &lut, 8, 8, inv_n_q20(4), 8);

        // Realwerte: 2400, 1.0, 2400, 1.0 -> Kanal 1 und 3 muessen gleich
        // normalisieren, ebenso Kanal 0 und 2.
        assert_eq!(out[1], out[3],
            "gleicher Realwert, andere Shift-Kodierung -> muss gleich sein: {:?}", out);
        assert_eq!(out[0], out[2],
            "gleicher Realwert, andere Shift-Kodierung -> muss gleich sein: {:?}", out);
        // Hinweis: dass die feinen Kanaele hier auf 0 runden, ist KORREKT
        // und kein Fehler — ihr Realwert (1,0) ist gegenueber dem RMS
        // (~1697) tatsaechlich vernachlaessigbar. Geprueft wird deshalb
        // die Invarianz, nicht die Nicht-Null-Eigenschaft.
    }

    #[test]
    fn test_rmsnorm_extremer_shift_bereich_laeuft_nicht_ueber() {
        // Der numerische Vertrag verlangt "explicit_clamp_only", wrap =
        // false (theta_v: numeric.overflow). Seit Fund 24 ist der
        // Gesamt-Shift regelmaessig NEGATIV, also ein Linksshift — der in
        // i64 ab Verschiebung 26 gewrappt haette. Dieser Test faehrt die
        // Spanne bis an MAX_FRAC_BITS und prueft, dass das Ergebnis
        // gesaettigt statt gewrappt wird (ein Wrap zeigte sich als
        // Vorzeichenwechsel).
        let lut = spec_lut(32768);
        let x = vec![32767i16, 32767, 1, 1];
        let x_shifts = vec![0u8, 20, 0, 20];
        let gamma = [127i8; 4];
        let gamma_shifts = [20u8; 4];
        let out = rmsnorm_i16(&x, &x_shifts, &gamma, &gamma_shifts,
            &lut, 8, 8, inv_n_q20(4), 0);
        // Positive Eingaenge mit positivem Gamma duerfen nie negativ
        // herauskommen — ein Vorzeichenwechsel waere die Signatur eines
        // Wraps. Der Wertebereich selbst ist durch den i16-Rueckgabetyp
        // bereits garantiert; geprueft wird also das Vorzeichen.
        assert!(out.iter().all(|v| *v >= 0), "Vorzeichenwechsel deutet auf Wrap: {:?}", out);
    }
}
