//! MLP / Feed Forward – Integer (Aktivierungen int16, Per-Layer-Skalen)
// Die Gewichtsmatrizen heißen wie im Whitepaper (Anhang B): `W`, `W_gate`,
// `W_up`, `W_down`. Klein geschrieben wären sie von den Einzelgewichten
// `w` im selben Rumpf nicht mehr zu unterscheiden — die Entsprechung zur
// Referenzformel ist beim Nachrechnen mehr wert als die Namenskonvention.
#![allow(non_snake_case)]

use crate::fixed_point::{clamp_i16_from_i64, rescale, rescale_i64};
use crate::integer_math::lut_lookup;
use crate::linear::{linear_w8a16, linear_w8a16_pc};

/// Integer-MLP mit SiLU-Approximation via LUT.
///
/// Skalen (alles kalibrierte Per-Layer-Zweierpotenz-Skalen, siehe
/// `scales.json`):
/// - `in_frac_bits`: Eingang (Ausgabe der post_attention_layernorm)
/// - `gate_out_frac`/`up_out_frac`: Ausgaenge von gate-/up-Projektion
/// - `down_in_frac`: Eingang von down_proj (h = silu(gate)*up)
/// - `out_frac_bits`: Ausgangsskala JE KANAL (Fund 20, theta_v 0.11.0) -
///   down_proj addiert direkt in den Residualstrom, der seit Fund 20 eine
///   Skala je Kanal trägt (Massive Activations bei Qwen2.5-7B)
///
/// Die SiLU-LUT arbeitet in einer festen Domäne (`silu_in_frac`, Index-
/// Offset `silu_lut_offset` = -input_min der spec): Gate-Werte werden vor
/// dem Lookup in diese Domäne reskaliert; große Betragswerte saturieren
/// deterministisch am LUT-Rand.
///
/// ⚑ **Fund 75: Der letzte Satz gilt unter einer Vorbedingung, die bis
/// zum 2026-08-28 nirgends stand.** Saturiert wird erst **in**
/// [`crate::integer_math::lut_lookup`]. Davor steht hier ein
/// ungesichertes `g_dom as i16`, und ein `i32`, der nicht in `i16`
/// passt, wird davon **abgeschnitten statt gesättigt** — aus einem zu
/// großen positiven Gate-Wert wird dann ein negativer Index, und die
/// LUT liefert deterministisch den falschen Wert statt deterministisch
/// den Randwert.
///
/// **Die Vorbedingung ist eine Aussage über den Wert, nicht über die
/// Skalen: `g_dom` muss in `i16` passen.** Geprüft wird deshalb der
/// Wert.
///
/// ⚑ **Der erste Anlauf prüfte `gate_out_frac >= silu_in_frac`, und das
/// war falsch.** Die Bedingung ist **hinreichend**, nicht notwendig:
/// Nur dann verkleinert der Reskalierer garantiert. Ein kleiner
/// Gate-Wert mit mäßigem Linksschieber passt aber ebenso, und genau so
/// arbeiten die synthetischen Prüfvorrichtungen des Laders
/// (`gate_out_frac` 4 gegen `silu_in_frac` 6). Sie fielen sofort durch,
/// obwohl an ihnen nichts falsch ist. **Eine zu enge Prüfung erzeugt
/// Druck, sie wegzunehmen, statt den Fehler zu finden** — dieselbe
/// Falle wie ein Test, der ein Literal statt der Regel prüft.
///
/// **Beide Bedingungen sind trotzdem wissenswert:**
///
/// - *notwendig und hinreichend:* `g_dom` liegt in `i16`. Das wird
///   geprüft.
/// - *hinreichend, und das, was die Kalibrierung liefert:*
///   `gate_out_frac >= silu_in_frac`. Über alle vier Modelle liegen die
///   `gate_proj`-Skalen zwischen 7 und 13, `silu.input_frac_bits` bei 6.
///   Solange das so bleibt, kann der Wert den Bereich gar nicht
///   verlassen, denn `*g` ist schon ein `i16` und der Reskalierer
///   verkleinert.
///
/// ⚑ **Dieselbe Stelle ist in `backward.rs` anders gelöst**, und dort
/// steht die Begründung dabei: `silu_backward` sättigt den Eingang
/// ausdrücklich mit `clamp_i16_sat`, „der LUT-Index darf nicht
/// wrappen". Zwei Lesarten derselben Frage im selben Crate, von denen
/// eine geschützt ist und eine nicht.
///
/// **Warum hier trotzdem nur geprüft und nicht geklemmt wird:** Ein
/// `clamp` an dieser Stelle liefe je Element in der innersten Schleife
/// und müsste in **allen vier Backends** gleich eingebaut werden
/// (`reference`, `simd`, `cuda`, `rocm`), sonst bricht die
/// Bitgleichheit. Das ist eine Entscheidung über den Rechenpfad und
/// kein Nebenbei-Fix. Die Prüfung hält die Annahme fest, bis sie
/// getroffen ist.
#[allow(clippy::too_many_arguments)]
pub fn mlp_int(
    x: &[i16],
    W_gate: &[i8],
    W_up: &[i8],
    W_down: &[i8],
    hidden_size: usize,
    intermediate_size: usize,
    gate_w_shifts: &[u8],
    up_w_shifts: &[u8],
    down_w_shifts: &[u8],
    silu_lut: &[i16],
    in_frac_bits: u8,
    gate_out_frac: u8,
    up_out_frac: u8,
    down_in_frac: u8,
    silu_in_frac: u8,
    silu_lut_offset: i16,
    silu_out_frac: u8,
    out_frac_bits: &[u8],
) -> Vec<i16> {
    mlp_int_mit_spur(
        x, W_gate, W_up, W_down, hidden_size, intermediate_size, gate_w_shifts, up_w_shifts,
        down_w_shifts, silu_lut, in_frac_bits, gate_out_frac, up_out_frac, down_in_frac,
        silu_in_frac, silu_lut_offset, silu_out_frac, out_frac_bits, None,
    )
}

/// Was der MLP-Block an Zwischenwerten zurücklässt (TRAINING V).
///
/// ⚑ **Drei Werte, und jeder hat genau einen Abnehmer:** `gate` ist das
/// `x` von [`crate::backward::silu_backward`], `up` der zweite Faktor
/// des Produkts, und `h` das `x` von `down_proj`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mlpspur {
    /// Die Gate-Projektion **vor** der Aktivierung, auf `gate_out_frac`.
    pub gate: Vec<i16>,
    /// Die Up-Projektion, auf `up_out_frac`.
    pub up: Vec<i16>,
    /// Das Produkt `silu(gate) · up`, auf `down_in_frac`: der Eingang
    /// von `down_proj`.
    pub h: Vec<i16>,
}

/// Dasselbe wie [`mlp_int`], aber die Zwischenwerte fallen mit ab.
///
/// # ⚑ Warum ein zweiter Eingang und kein Parameter mehr an `mlp_int`
///
/// `mlp_int` steht im `Backend`-Merkmal, in vier Umsetzungen. Ein
/// zusätzliches Argument dort risse alle vier auf, für etwas, das nur
/// der Rückwärtspass braucht. **Hier steht die eine Umsetzung**, und
/// `mlp_int` ist ihr Eingang ohne Spur.
#[allow(non_snake_case, clippy::too_many_arguments)]
pub fn mlp_int_mit_spur(
    x: &[i16],
    W_gate: &[i8],
    W_up: &[i8],
    W_down: &[i8],
    hidden_size: usize,
    intermediate_size: usize,
    gate_w_shifts: &[u8],
    up_w_shifts: &[u8],
    down_w_shifts: &[u8],
    silu_lut: &[i16],
    in_frac_bits: u8,
    gate_out_frac: u8,
    up_out_frac: u8,
    down_in_frac: u8,
    silu_in_frac: u8,
    silu_lut_offset: i16,
    silu_out_frac: u8,
    out_frac_bits: &[u8],
    spur: Option<&mut Mlpspur>,
) -> Vec<i16> {
    // Flache Gewichte, Begründung im Kopf von `linear_w8a16`.
    let gate = linear_w8a16(x, W_gate, hidden_size, gate_w_shifts, in_frac_bits, gate_out_frac);
    let up = linear_w8a16(x, W_up, hidden_size, up_w_shifts, in_frac_bits, up_out_frac);

    let mut h = Vec::with_capacity(gate.len());
    for (g, u) in gate.iter().zip(up.iter()) {
        // Gate in die feste LUT-Domäne reskalieren, Lookup, dann Produkt mit
        // up auf die kalibrierte down-Eingangsskala bringen.
        let g_dom = rescale(*g as i32, gate_out_frac, silu_in_frac);
        debug_assert!(
            g_dom >= i16::MIN as i32 && g_dom <= i16::MAX as i32,
            "mlp_int: reskalierter Gate-Wert {} verlaesst i16 und wuerde abgeschnitten statt gesaettigt (Fund 75); gate_out_frac {}, silu_in_frac {}",
            g_dom,
            gate_out_frac,
            silu_in_frac
        );
        let activated = lut_lookup(g_dom as i16, silu_lut, 0, silu_lut_offset);
        let prod = (activated as i64) * (*u as i64);
        h.push(clamp_i16_from_i64(rescale_i64(
            prod,
            silu_out_frac + up_out_frac,
            down_in_frac,
        )));
    }

    // ⚑ **Alle drei zusammen oder keiner.** Sie gehören zu **einem**
    // Durchlauf; wer nur zwei nähme, rechnete einen Gradienten aus
    // Werten, die nie gemeinsam entstanden sind.
    if let Some(sp) = spur {
        sp.gate = gate;
        sp.up = up;
        sp.h = h.clone();
    }

    linear_w8a16_pc(
        &h,
        W_down,
        intermediate_size,
        down_w_shifts,
        down_in_frac,
        out_frac_bits,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SiLU-LUT im spec-Format (input_frac 1, output_frac 6, [-256, 255]).
    fn spec_silu_lut() -> Vec<i16> {
        let mut lut = Vec::with_capacity(512);
        for x in -256..256 {
            let xf = x as f64 / 2.0;
            let val = xf * (1.0 / (1.0 + (-xf).exp()));
            lut.push((val * 64.0).round() as i16);
        }
        lut
    }

    #[test]
    fn test_mlp_runs_with_per_layer_scales() {
        // Rauchtest: 2 Kanaele, intermediate 2; alle Skalen explizit.
        let x = vec![64i16, -32];
        // Flach, wie der Kernel sie seit v0.13.4 nimmt: 2x2-Matrizen.
        let w_gate: Vec<i8> = vec![64, 0, 0, 64];
        let w_up: Vec<i8> = vec![64, 0, 0, 64];
        let w_down: Vec<i8> = vec![64, 32, 32, 64];
        let (hidden, inter) = (2usize, 2usize);
        let lut = spec_silu_lut();
        let out = mlp_int(
            &x, &w_gate, &w_up, &w_down,
            hidden, inter,
            &[6, 6], &[6, 6], &[6, 6], // Per-Channel-Gewichts-Shifts
            &lut,
            6,   // in_frac
            6, 6, 6, // gate/up/down-Eingangs-Skalen
            1, 256, 6, // SiLU-Domäne (frac 1, Offset 256, Output frac 6)
            &[6, 6],   // out_frac (Fund 20: je Kanal, hier uniform)
        );
        assert_eq!(out.len(), 2);
        // Alle Werte muessen im i16-Bereich und deterministisch sein.
        let out2 = mlp_int(
            &x, &w_gate, &w_up, &w_down,
            hidden, inter,
            &[6, 6], &[6, 6], &[6, 6],
            &lut,
            6, 6, 6, 6, 1, 256, 6, &[6, 6],
        );
        assert_eq!(out, out2);
    }

    /// ⚑ Gegenprobe zu Fund 75: Die Prüfung fängt genau den Fall,
    /// gegen den sie geschrieben ist.
    ///
    /// **Nicht eine ungünstige Skalenrelation reicht dafür, sondern ein
    /// Wert, der `i16` wirklich verlässt.** Ein früherer Entwurf dieses
    /// Tests setzte nur `gate_out_frac` unter `silu_in_frac` und prüfte
    /// damit eine hinreichende statt der notwendigen Bedingung; die
    /// Prüfvorrichtungen des Laders fielen dadurch zu Unrecht durch.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "verlaesst i16")]
    fn ein_gate_wert_ausserhalb_von_i16_bricht_ab() {
        // Gewichte am Rand und ein Linksschieber um 5 Bit: der
        // Zwischenwert wird groß genug, um i16 zu verlassen.
        let x = vec![32_767i16, 32_767];
        let w: Vec<i8> = vec![127, 127, 127, 127];
        let lut = spec_silu_lut();
        let _ = mlp_int(
            &x, &w, &w, &w, 2, 2,
            &[0, 0], &[6, 6], &[6, 6],
            &lut,
            6, 1, 6, 6, 6, 256, 6, &[6, 6],
        );
    }

    /// ⚑ Und das ist, was ohne die Prüfung geschähe, in zwei Stufen.
    ///
    /// **Stufe 1, der Cast:** Aus einem sehr großen **positiven**
    /// Gate-Wert wird ein **negativer** Zwischenwert, und der landet als
    /// völlig anderer, aber vollkommen gültig aussehender Index in der
    /// Tabelle. Er stürzt nicht ab und fällt in keinem Test auf.
    ///
    /// ⚑ **Stufe 2, und die ist beim Schreiben dieses Tests
    /// aufgefallen: Sättigen auf `i16` rettet nicht.** Der Wert wird
    /// danach noch um `silu_lut_offset` verschoben, und `32767 + 256`
    /// verlässt `i16` erneut. Die einzige richtige Sättigung ist die
    /// **in die LUT-Domäne**, also auf `[-offset, len-1-offset]`.
    ///
    /// **Damit ist auch der Schutz in `backward.rs` unvollständig:**
    /// `clamp_i16_sat` sättigt dort auf `i16`, und der Offset kommt
    /// danach. Wer den Fall je behebt, behebt ihn an beiden Stellen und
    /// in der LUT-Domäne, nicht in `i16`.
    #[test]
    fn der_ungesicherte_cast_macht_aus_gross_positiv_klein_negativ() {
        // Ein Gate-Wert am oberen i16-Rand, Domäne von frac 1 auf frac 6.
        let g_dom = crate::fixed_point::rescale(32_767, 1, 6);
        assert_eq!(g_dom, 32_767 << 5, "der Reskalierer vergrößert wie erwartet");

        // Stufe 1: der Cast, wie er im Kernel steht.
        let abgeschnitten = g_dom as i16;
        assert_eq!(abgeschnitten, -32, "aus +1 048 544 wird -32");

        // Mit dem Offset ergibt das einen Index mitten in der Tabelle,
        // statt am oberen Rand, wo er hingehörte.
        let index_falsch = (abgeschnitten as i32 + 256).clamp(0, 511);
        assert_eq!(index_falsch, 224);

        // Stufe 2: Sättigen auf i16 hilft nicht, die Addition danach
        // verlässt den Typ erneut. In i32 gerechnet ist zu sehen, wohin
        // sie liefe.
        let gesaettigt_i16 = crate::fixed_point::clamp_i16(g_dom);
        assert_eq!(gesaettigt_i16, 32_767);
        assert!(
            gesaettigt_i16 as i32 + 256 > i16::MAX as i32,
            "32767 + 256 passt nicht mehr in i16"
        );

        // Richtig ist die Sättigung in der LUT-Domäne.
        let index_richtig = (g_dom + 256).clamp(0, 511);
        assert_eq!(index_richtig, 511, "gesättigt gehörte er an den oberen Rand");
        assert_ne!(index_falsch, index_richtig);
    }
}
