//! Lineare Schichten: W8A16 (Gewichte int8, Aktivierungen int16) und Bias.
//!
//! Seit dem Numerik-Realitaetsabgleich (v0.12.20) sind Aktivierungen int16
//! mit kalibrierten Per-Layer-Zweierpotenz-Skalen: reale RMSNorm-/MLP-
//! Ausgaben (gemessen bis ~±1640) sprengen den int8-Bereich. Gewichte
//! bleiben int8. Akkumulation in i64, da 896 Kanaele * 127 * 32767 den
//! i32-Bereich ueberschreiten koennen.
// Die Kernel-Signaturen tragen den vollstaendigen Fixed-Point-Vertrag:
// Eingangs- und Ausgangs-frac_bits, Per-Channel-Shifts, LUT-Parameter.
// In eine Parameter-Struct gefasst waere die Entsprechung zu den
// Referenzformeln (Whitepaper Anhang B) beim Nachrechnen nicht mehr
// ablesbar — und genau dieses Nachrechnen ist die Pruefmethode des
// Projekts. Bewusste Abweichung von clippy::too_many_arguments.
#![allow(clippy::too_many_arguments)]
// Die Gewichtsmatrizen heißen wie im Whitepaper (Anhang B): `W`, `W_gate`,
// `W_up`, `W_down`. Klein geschrieben wären sie von den Einzelgewichten
// `w` im selben Rumpf nicht mehr zu unterscheiden — die Entsprechung zur
// Referenzformel ist beim Nachrechnen mehr wert als die Namenskonvention.
#![allow(non_snake_case)]

use crate::dot::dot_i8_i16;
use crate::fixed_point::{clamp_i16_from_i64, rescale, rescale_i64};

/// Ab wie vielen Multiplikationen (`zeilen · in_features`) sich das
/// Aufteilen über Threads überhaupt lohnt.
///
/// **Gemessen, nicht geraten.** Der Start eines `thread::scope` kostet
/// rund `12 µs + 6,3 µs je Thread`, also 25 µs bei zwei und 107 µs bei
/// fünfzehn.
///
/// ⛑ **Der Beleg liegt in `src/bin/threads_probe.rs` und ist gültig.**
/// Am 2026-09-11 stand hier eine Weile, es gebe die Datei nicht mehr;
/// **ich hatte in `runtime/src/bin` gesucht statt hier.** Der Pfad ist
/// relativ zu dieser Kiste, und dort liegt sie. Nachgemessen am selben
/// Tag auf M5 Pro: 18,5 µs bei zwei Fäden, 28,5 bei vier, 56,5 bei
/// acht, 84,5 bei zwölf, also dieselbe Gerade wie damals.
///
/// ⚠️ **Und die Summe ist kein Nebenposten:** Bei 36 Ebenen und sieben
/// Matrizen je Ebene sind es 253 Starts je Token, zusammen 15,7 ms von
/// 68,7 ms. **Ein stehender Fadenpool statt eines Bereichs je Matrix
/// nimmt ein Viertel der Laufzeit weg**, ohne eine Zahl zu ändern. Unterhalb dieser Schwelle frisst
/// der Start den Gewinn: Die 896×896-Matrizen von 0,5B brauchen
/// einkernig 54 µs, und selbst die beste Aufteilung sparte davon nur 12.
const PARALLEL_AB: usize = 1_500_000;

/// Arbeit je Thread, in Multiplikationen.
///
/// **Warum die Threadzahl an der Arbeit hängt und nicht an der
/// Kernzahl.** Der erste Versuch nahm einfach `available_parallelism`,
/// auf der Messmaschine also 15, und brachte bei 0,5B **nichts**: Die
/// 4864×896-Matrix braucht einkernig 289 µs, und 15 Threads kosten
/// allein 107 µs Start. Gemessen an derselben Matrix: vier Threads
/// **2,53×**, acht Threads 2,41×, fünfzehn Threads nur noch 1,72×.
///
/// Bei der größten Matrix des 7B-Modells (18944×3584) ist es umgekehrt:
/// dort bringen 15 Threads **7,40×** gegenüber 2,83× bei vier. Eine
/// feste Zahl ist also für eine der beiden Größen falsch.
const ARBEIT_JE_THREAD: usize = 1_000_000;

/// Was der Nutzer diesem Prozess an Kernen zugesteht; 0 heisst „alles".
///
/// # ⚑ Warum eine Obergrenze hierher gehoert und warum sie gefahrlos ist
///
/// Ein Knoten gibt einen **Teil** seiner Maschine her, nicht die ganze,
/// und ohne diese Grenze nimmt sich der Rechenpfad, was
/// `available_parallelism` meldet. Eine Einstellung, die das nicht
/// begrenzt, ist eine Anzeige und keine Einstellung.
///
/// ⚑ **Und sie aendert kein Ergebnis.** Jede Ausgabezeile ist ein
/// eigenes Skalarprodukt ueber ihre eigene Gewichtszeile und schreibt
/// in ihr eigenes Feld; zwischen den Zeilen gibt es keine gemeinsame
/// Zwischensumme. Threadzahl, Aufteilung und Schwelle sind damit reine
/// Laufzeitentscheidungen, siehe [`zeilen_rechnen`]. Genau deshalb darf
/// ein Nutzer daran drehen, ohne die Bitgleichheit anzutasten, und
/// genau das prueft `dieselbe_antwort_bei_jeder_kernzahl`.
static KERNGRENZE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Setzt die Obergrenze; `0` gibt die Maschine wieder frei.
///
/// ⚑ **Wirkt auf den ganzen Prozess.** Gedacht ist sie fuer den Aufruf
/// beim Start, aus der Kapazitaetseinstellung des Nutzers.
pub fn kerngrenze_setzen(n: usize) {
    KERNGRENZE.store(n, std::sync::atomic::Ordering::Relaxed);
}

/// Was gerade gilt, nach Beruecksichtigung der Maschine.
pub fn kerngrenze() -> usize {
    max_threads()
}

/// Obergrenze der Threadzahl. Die Maschine wird einmal befragt, die
/// Grenze des Nutzers bei jedem Aufruf gelesen: Ein Atomlesen kostet
/// nichts, und eine zwischengespeicherte Grenze liesse sich nach dem
/// ersten Rechenschritt nicht mehr aendern.
fn max_threads() -> usize {
    use std::sync::OnceLock;
    static N: OnceLock<usize> = OnceLock::new();
    let vorhanden = *N.get_or_init(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    });
    match KERNGRENZE.load(std::sync::atomic::Ordering::Relaxed) {
        0 => vorhanden,
        n => n.min(vorhanden).max(1),
    }
}

/// Rechnet `zeilen` unabhängige Ausgabewerte, einkernig oder verteilt.
///
/// **Bitgleich per Konstruktion, und das ist keine Redewendung.** Jede
/// Ausgabezeile ist ein eigenes Skalarprodukt über ihre eigene
/// Gewichtszeile und schreibt in ihr eigenes Feld. Zwischen den Zeilen
/// gibt es keine gemeinsame Zwischensumme, also auch keine Reihenfolge,
/// die etwas ändern könnte. Threadzahl, Aufteilung und Schwelle sind
/// damit reine Laufzeitentscheidungen.
///
/// Das ist dieselbe Eigenschaft, aus der das ganze Projekt seine
/// Bitgleichheit zieht, nur eine Ebene höher: Dort ist es die
/// Assoziativität der Ganzzahladdition **innerhalb** einer Zeile, hier
/// die Unabhängigkeit **zwischen** den Zeilen.
fn zeilen_rechnen<F>(zeilen: usize, arbeit_je_zeile: usize, f: F) -> Vec<i16>
where
    F: Fn(usize) -> i16 + Sync,
{
    let arbeit = zeilen.saturating_mul(arbeit_je_zeile);
    // ⛑ **Die Reihenfolge dieser beiden Zeilen war bis zum 2026-09-08
    // vertauscht**, und `clamp(2, 1)` ist kein milder Fehler, sondern
    // eine Panik: `min > max`. Getroffen haette es jede **einkernige**
    // Maschine bei jeder Matrix ueber der Schwelle, also genau die
    // Geraete, auf die dieses Projekt ausdruecklich setzt. Gefunden hat
    // es die Pruefung zur Kerngrenze, weil sie die Kernzahl auf eins
    // stellen darf; ohne sie waere der Fehler erst auf fremder Hardware
    // aufgefallen.
    //
    // ⚑ **Einmal gelesen, nicht zweimal gefragt.** Die Grenze steht in
    // einer Atomvariablen und kann sich zwischen zwei Aufrufen aendern;
    // ein Wert, der die Verzweigung entscheidet und danach neu gelesen
    // wird, ist genau die Sorte Fenster, die selten und dann schwer
    // auffaellt.
    let kerne = max_threads();
    if arbeit < PARALLEL_AB || kerne < 2 || zeilen < 2 {
        return (0..zeilen).map(&f).collect();
    }
    let n = (arbeit / ARBEIT_JE_THREAD).clamp(2, kerne);

    // ⛑ **Hier stand bis zum 2026-09-11 ein `std::thread::scope` je
    // Matrix**, und damit ein neuer Betriebssystemfaden je Aufruf.
    // Gemessen: 253 Bereiche je Token beim 4B-Modell, zusammen 15,7 ms
    // von 68,7, also **23 % reiner Fadenstart**. Der Pool weckt
    // stattdessen geparkte Faeden.
    //
    // ⚑ **An der Aufteilung aendert sich nichts**, und deshalb auch an
    // keiner Zahl: `n` wird weiter aus der Arbeitsmenge gerechnet, und
    // jede Ausgabezeile bleibt ihr eigenes Skalarprodukt.
    crate::fadenpool::rechnen(zeilen, n, f)
}

/// W8A16 Matrix-Vektor-Multiplikation.
///
/// `x` (Aktivierung, int16, Skala `act_frac_bits`), `W` (Gewicht, int8,
/// Per-Channel-Skala je Ausgabe-Zeile `w_shifts[r]`, theta_v 0.7.0);
/// Ausgabe int16 auf `out_frac_bits`.
/// **`W` liegt flach**, Zeile für Zeile hintereinander, `in_features`
/// Elemente je Zeile.
///
/// Bis v0.13.4 nahm dieser Kernel `&[Vec<i8>]`. Die Gewichte liegen im
/// Artefakt und im `QTensor` aber flach; `model.rs` baute deshalb vor
/// **jedem** Aufruf ein `Vec<Vec<i8>>` daraus, mit einer Heap-Allokation
/// und einer Kopie je Ausgabe-Zeile. Bei Qwen2.5-0,5B waren das
/// **358 MB und 304 128 Allokationen je Token**, denn die Umwandlung lief
/// achtmal je Ebene und die Ebenen 24-mal je Token.
///
/// **Die Numerik ändert sich dadurch nicht.** `dot_i8_i16` bekommt
/// dieselben Bytes in derselben Reihenfolge; die Zeile ist jetzt ein
/// Ausschnitt statt einer Kopie. Bitgleichheit gilt hier per Konstruktion,
/// nicht nur laut Messung.
pub fn linear_w8a16(
    x: &[i16],
    W: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    out_frac_bits: u8,
) -> Vec<i16> {
    assert_eq!(
        W.len(),
        in_features * w_shifts.len(),
        "linear_w8a16: {} Gewichte passen nicht zu {} Zeilen à {} Elementen",
        W.len(),
        w_shifts.len(),
        in_features
    );
    // Vektorisiert, wenn `cpu-simd` aktiv ist, und über Threads verteilt,
    // wenn die Matrix groß genug ist. Beides bitgleich zur einfachsten
    // Fassung: innerhalb der Zeile, weil die i64-Akkumulation exakt und
    // damit assoziativ ist (`dot.rs`), zwischen den Zeilen, weil sie
    // voneinander unabhängig sind (`zeilen_rechnen`).
    zeilen_rechnen(w_shifts.len(), in_features, |z| {
        let row = &W[z * in_features..(z + 1) * in_features];
        let acc = dot_i8_i16(row, x);
        let y = rescale_i64(acc, w_shifts[z] + act_frac_bits, out_frac_bits);
        clamp_i16_from_i64(y)
    })
}

/// W8A16 mit Per-Kanal-Ausgangsskala (Fund 20, theta_v 0.11.0).
///
/// Wie `linear_w8a16`, aber `out_frac_bits` ist ein Shift JE
/// AUSGABE-KANAL statt ein einziger fuer den ganzen Vektor. Wird fuer
/// `o_proj` und `down_proj` gebraucht: ihre Ausgabe wird direkt in den
/// Residualstrom addiert, und der trägt seit Fund 20 eine Skala je Kanal
/// (Massive Activations bei Qwen2.5-7B — siehe `rmsnorm.rs`-Modulkopf).
/// `q_proj`/`k_proj`/`v_proj`/`gate_proj`/`up_proj` bleiben bei der
/// Skalar-Funktion, weil ihre Ausgaben NICHT in den Residualstrom
/// zurückfliessen und keine vergleichbaren Ausreisser zeigen.
///
/// Bei identischem Wert in jedem Element von `out_frac_bits` ist das
/// Ergebnis bitgleich zu `linear_w8a16` mit demselben Skalar (siehe
/// `test_linear_w8a16_pc_uniform_matches_scalar`).
/// `W` liegt flach wie bei [`linear_w8a16`], Begründung dort.
pub fn linear_w8a16_pc(
    x: &[i16],
    W: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    out_frac_bits: &[u8],
) -> Vec<i16> {
    assert_eq!(
        W.len(),
        in_features * w_shifts.len(),
        "linear_w8a16_pc: {} Gewichte passen nicht zu {} Zeilen à {} Elementen",
        W.len(),
        w_shifts.len(),
        in_features
    );
    assert_eq!(
        w_shifts.len(),
        out_frac_bits.len(),
        "linear_w8a16_pc: eine Ausgangsskala je Kanal (Fund 20)"
    );
    zeilen_rechnen(w_shifts.len(), in_features, |z| {
        let row = &W[z * in_features..(z + 1) * in_features];
        let acc = dot_i8_i16(row, x);
        let y = rescale_i64(acc, w_shifts[z] + act_frac_bits, out_frac_bits[z]);
        clamp_i16_from_i64(y)
    })
}

/// Addiert einen quantisierten Bias auf eine int16-Aktivierungsausgabe.
///
/// Der Bias liegt als **int16** mit Per-Element-Skalen vor
/// (`bias_shifts[i]`). Bis theta_v 0.12.0 war es int8 — das saettigte
/// still bei Betraegen ueber 127 und verfaelschte bei Qwen2.5-7B die
/// Attention ab Ebene 0 (Fund 23, k_proj.bias erreicht 414)
/// und wird elementweise mit `rescale` auf die Ziel-Skala
/// (`out_frac_bits`) gebracht — arithmetischer Rechtsshift mit
/// Round-to-nearest-even, danach i64-Addition mit Clamping auf i16. Reine
/// Ganzzahlarithmetik, deterministisch über alle Backends (Whitepaper
/// Kap. 6.2; Qwen2.5 besitzt Biases an q/k/v_proj).
pub fn add_bias_i16(out: &mut [i16], bias: &[i16], bias_shifts: &[u8], out_frac_bits: u8) {
    assert_eq!(
        out.len(),
        bias.len(),
        "add_bias_i16: Ausgabe ({} Elemente) und Bias ({} Elemente) muessen dieselbe Laenge haben",
        out.len(),
        bias.len()
    );
    assert_eq!(bias.len(), bias_shifts.len(), "add_bias_i16: eine Skala je Bias-Element");
    for ((o, b), &b_shift) in out.iter_mut().zip(bias.iter()).zip(bias_shifts.iter()) {
        let bias_rescaled = rescale(*b as i32, b_shift, out_frac_bits);
        *o = clamp_i16_from_i64((*o as i64) + (bias_rescaled as i64));
    }
}

#[cfg(test)]
mod stapeltests {
    use super::*;

    fn zufall(n: usize, saat: u64) -> Vec<i16> {
        let mut x = saat | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x % 4096) as i16 - 2048
            })
            .collect()
    }

    /// **Gebuendelt ist bitgleich zu einzeln.**
    ///
    /// ⛑ **Die Gegenprobe zur ganzen Buendelung.** Sie ist per
    /// Konstruktion gegeben, weil jedes Ausgabeelement aus demselben
    /// Skalarprodukt entsteht; **eine Zusage per Konstruktion, die
    /// niemand nachrechnet, ist trotzdem nur eine Zusage.**
    #[test]
    fn gebuendelt_ist_dasselbe() {
        for (zeilen, spalten) in [(1usize, 4usize), (3, 8), (17, 33), (64, 128), (2, 2049)] {
            let w: Vec<i8> = zufall(zeilen * spalten, 7)
                .into_iter()
                .map(|v| (v % 127) as i8)
                .collect();
            let shifts: Vec<u8> = (0..zeilen).map(|z| 4 + (z % 3) as u8).collect();
            let out_pc: Vec<u8> = (0..zeilen).map(|z| 5 + (z % 4) as u8).collect();
            for b in [1usize, 2, 5, 16] {
                let eingaben: Vec<Vec<i16>> =
                    (0..b).map(|i| zufall(spalten, 11 + i as u64)).collect();
                let scheiben: Vec<&[i16]> = eingaben.iter().map(|v| v.as_slice()).collect();

                let gebuendelt = linear_w8a16_stapel(&scheiben, &w, spalten, &shifts, 6, 5);
                for (i, x) in eingaben.iter().enumerate() {
                    let einzeln = linear_w8a16(x, &w, spalten, &shifts, 6, 5);
                    assert_eq!(gebuendelt[i], einzeln, "{zeilen}x{spalten}, b={b}, i={i}");
                }

                let gb_pc = linear_w8a16_pc_stapel(&scheiben, &w, spalten, &shifts, 6, &out_pc);
                for (i, x) in eingaben.iter().enumerate() {
                    let einzeln = linear_w8a16_pc(x, &w, spalten, &shifts, 6, &out_pc);
                    assert_eq!(gb_pc[i], einzeln, "pc {zeilen}x{spalten}, b={b}, i={i}");
                }
            }
        }
    }

    /// **Ein Buendel vieler Matrizen ist bitgleich zu den Matrizen
    /// einzeln, mit gemischten Formen, Eingaben und Skalenarten.**
    ///
    /// ⛑ **Die Gegenprobe zu Fund 332.** Auch sie gilt per
    /// Konstruktion, und auch hier ist eine Zusage, die niemand
    /// nachrechnet, nur eine Zusage. Geprueft wird ausdruecklich mit
    /// **verschiedenen** Zeilenzahlen je Teil, denn genau dort greift
    /// die Bereichshalbierung ueber die Grenzen.
    #[test]
    fn ein_buendel_ist_dasselbe_wie_einzeln() {
        let formen = [(1usize, 4usize), (3, 8), (17, 33), (64, 128), (2, 2049)];
        let gewichte: Vec<Vec<i8>> = formen
            .iter()
            .enumerate()
            .map(|(i, (z, sp))| {
                zufall(z * sp, 7 + i as u64).into_iter().map(|v| (v % 127) as i8).collect()
            })
            .collect();
        let eingaben: Vec<Vec<i16>> = formen
            .iter()
            .enumerate()
            .map(|(i, (_, sp))| zufall(*sp, 101 + i as u64))
            .collect();
        let shifts: Vec<Vec<u8>> = formen
            .iter()
            .map(|(z, _)| (0..*z).map(|r| 4 + (r % 3) as u8).collect())
            .collect();
        let je_zeile: Vec<Vec<u8>> = formen
            .iter()
            .map(|(z, _)| (0..*z).map(|r| 5 + (r % 4) as u8).collect())
            .collect();

        // Ein Buendel, in dem sich beide Skalenarten abwechseln.
        let teile: Vec<Buendelteil<'_>> = formen
            .iter()
            .enumerate()
            .map(|(i, (_, sp))| Buendelteil {
                w: &gewichte[i],
                x: &eingaben[i],
                in_features: *sp,
                w_shifts: &shifts[i],
                act_frac_bits: 6,
                aus: if i % 2 == 0 {
                    Ausgangsskala::Eine(5)
                } else {
                    Ausgangsskala::JeZeile(&je_zeile[i])
                },
            })
            .collect();

        let gebuendelt = linear_w8a16_buendel(&teile);

        let mut anfang = 0usize;
        for (i, (z, sp)) in formen.iter().enumerate() {
            let einzeln = if i % 2 == 0 {
                linear_w8a16(&eingaben[i], &gewichte[i], *sp, &shifts[i], 6, 5)
            } else {
                linear_w8a16_pc(&eingaben[i], &gewichte[i], *sp, &shifts[i], 6, &je_zeile[i])
            };
            assert_eq!(
                &gebuendelt[anfang..anfang + z],
                einzeln.as_slice(),
                "Teil {i} ({z}x{sp}) weicht ab"
            );
            anfang += z;
        }
        assert_eq!(anfang, gebuendelt.len(), "das Buendel ist laenger als seine Teile");
    }

    /// **Ein leeres Buendel ist leer und kein Absturz.**
    #[test]
    fn ein_leeres_buendel_gibt_nichts() {
        assert!(linear_w8a16_buendel(&[]).is_empty());
    }

    /// **Eine leere Bündelung ist leer und kein Absturz.**
    #[test]
    fn ohne_eingabe_gibt_es_nichts() {
        let w = vec![1i8; 8];
        let shifts = vec![4u8; 2];
        assert!(linear_w8a16_stapel(&[], &w, 4, &shifts, 6, 5).is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_w8a16_identity() {
        // x = [1.0, -1.0] bei frac 6 = [64, -64]; Identitaets-Matrix mit
        // Per-Channel-Shift 7 (127 ~ 1.0): Ergebnis ~ [64, -64] bei frac 6.
        let x = vec![64i16, -64];
        let W: Vec<i8> = vec![127, 0, 0, 127];
        let in_features = 2;
        let out = linear_w8a16(&x, &W, in_features, &[7, 7], 6, 6);
        // 127/128 = 0.992 -> 64 * 127 >> 7 = 63 (RNE: 63.5 -> 64? 64*127=8128,
        // >>7 = 63 Rest 64 = half -> quotient 63 ungerade -> +1 = 64).
        assert_eq!(out, vec![64, -64]);
    }

    #[test]
    fn test_linear_w8a16_per_channel_shifts() {
        // Zeile 0 mit Shift 7 (~1.0), Zeile 1 mit Shift 6 (~2.0):
        // dieselben Gewichte, aber Zeile 1 verdoppelt das Ergebnis.
        let x = vec![64i16];
        let W: Vec<i8> = vec![64, 64];
        let in_features = 1;
        let out = linear_w8a16(&x, &W, in_features, &[7, 6], 6, 6);
        // Zeile 0: 64*64 = 4096, rescale(4096, 13, 6) = 4096>>7 = 32
        // Zeile 1: 64*64 = 4096, rescale(4096, 12, 6) = 4096>>6 = 64
        assert_eq!(out, vec![32, 64]);
    }

    #[test]
    fn test_linear_w8a16_large_accumulator() {
        // Akkumulator jenseits von i32: 896 Kanaele, alle w=127, x=32767
        // -> acc = 896 * 127 * 32767 ≈ 3.7e9 > i32::MAX. Muss in i64
        // akkumulieren und korrekt reskalieren.
        let n = 896usize;
        let x = vec![32767i16; n];
        let W: Vec<i8> = vec![127i8; n];
        let in_features = n;
        // in_frac = 5 + 7 = 12, out_frac 3: acc >> 9.
        let out = linear_w8a16(&x, &W, in_features, &[7], 5, 3);
        let expected = 32767;
        assert_eq!(out[0], expected as i16);
    }

    #[test]
    fn test_linear_w8a16_pc_uniform_matches_scalar() {
        // Fund 20: dasselbe out_frac_bits in jedem Kanal muss bitgleich
        // zur Skalar-Funktion sein - Voraussetzung dafuer, dass bestehende
        // Aufrufer (q/k/v/gate/up_proj) unangetastet bleiben duerfen.
        let x = vec![100i16, -50, 25];
        let W: Vec<i8> = vec![64, -32, 16, 10, 20, -30];
        let in_features = 3;
        let w_shifts = vec![6u8, 5];
        let skalar = linear_w8a16(&x, &W, in_features, &w_shifts, 4, 6);
        let per_kanal = linear_w8a16_pc(&x, &W, in_features, &w_shifts, 4, &[6, 6]);
        assert_eq!(skalar, per_kanal);
    }

    #[test]
    fn test_linear_w8a16_pc_different_targets_per_channel() {
        // Zwei Ausgabezeilen mit identischen Rohwerten, aber
        // unterschiedlicher Zielskala je Kanal (das ist der eigentliche
        // Zweck: o_proj/down_proj muessen auf die per-Kanal kalibrierte
        // Residualskala zielen koennen, nicht nur auf eine gemeinsame).
        let x = vec![100i16, 100];
        let W: Vec<i8> = vec![64, 64, 64, 64]; // identische Zeilen
        let in_features = 2;
        let w_shifts = vec![6u8, 6];
        let out = linear_w8a16_pc(&x, &W, in_features, &w_shifts, 0, &[6, 3]);
        // acc = 64*100 + 64*100 = 12800 fuer beide Zeilen.
        // Zeile 0: rescale(12800, 6, 6) = 12800 (kein Shift).
        // Zeile 1: rescale(12800, 6, 3) = 12800 >> 3 = 1600.
        assert_eq!(out[0], 12800);
        assert_eq!(out[1], 1600);
    }

    #[test]
    fn test_add_bias_i16_rescale_left_shift() {
        // bias_shift (2) < out_frac (4): Linksverschiebung, exakt.
        let mut out = vec![10i16, -10];
        add_bias_i16(&mut out, &[1i16, 1], &[2, 2], 4);
        assert_eq!(out, vec![14, -6]);
    }

    #[test]
    fn test_add_bias_i16_rescale_right_shift_rounds_rne() {
        // bias_shift (3) > out_frac (1): Rechtsshift um 2 mit RNE-Rundung.
        let mut out = vec![0i16, 0];
        add_bias_i16(&mut out, &[3i16, -3], &[3, 3], 1);
        assert_eq!(out, vec![1, -1]);
    }

    #[test]
    fn test_add_bias_i16_per_element_shifts() {
        // Unterschiedliche Shifts je Element (theta_v 0.7.0):
        // Bias 4 mit Shift 1 (= 2.0), Bias 4 mit Shift 0 (= 4.0).
        let mut out = vec![0i16, 0];
        add_bias_i16(&mut out, &[4i16, 4], &[1, 0], 0);
        assert_eq!(out, vec![2, 4]);
    }

    #[test]
    fn test_add_bias_i16_clamping() {
        let mut out = vec![32766i16, -32767, 0];
        add_bias_i16(&mut out, &[4i16, -4, 0], &[1, 1, 1], 0); // +2 bzw. -2
        assert_eq!(out, vec![32767, -32768, 0]); // Saettigung an beiden Grenzen
    }

    #[test]
    #[should_panic(expected = "dieselbe Laenge")]
    fn test_add_bias_i16_length_mismatch_panics() {
        let mut out = vec![0i16; 3];
        add_bias_i16(&mut out, &[1i16, 1], &[0, 0], 0);
    }
}

#[cfg(test)]
mod kerngrenze_probe {
    use super::*;

    /// ⚑ **Die Eigenschaft, auf der die Kerngrenze ruht.**
    ///
    /// Ein Nutzer darf an der Kernzahl drehen, weil sie kein Ergebnis
    /// aendert: Jede Ausgabezeile ist ein eigenes Skalarprodukt ueber
    /// ihre eigene Gewichtszeile, zwischen den Zeilen gibt es keine
    /// gemeinsame Zwischensumme. **Wer diese Zusicherung gibt, prueft
    /// sie**, sonst ist sie eine Behauptung im Kommentar.
    ///
    /// Die Matrix ist absichtlich gross genug, um ueber `PARALLEL_AB`
    /// zu liegen; darunter liefe der einkernige Zweig und die Pruefung
    /// vergliche viermal dasselbe.
    ///
    /// ⚑ **Zur Nebenlaeufigkeit:** `KERNGRENZE` gilt fuer den ganzen
    /// Prozess, und andere Pruefungen laufen daneben. Sie sehen
    /// zeitweise eine andere Fadenzahl und bekommen **dasselbe
    /// Ergebnis**, denn genau das steht hier zur Pruefung.
    #[test]
    fn dieselbe_antwort_bei_jeder_kernzahl() {
        // ⚑ Ueber `PARALLEL_AB`, sonst laeuft der einkernige Zweig
        // und die Pruefung vergliche viermal dasselbe.
        let zeilen = 2048;
        let ein = 1024;
        assert!(zeilen * ein > PARALLEL_AB, "die Matrix loest den verteilten Zweig nicht aus");

        let x: Vec<i16> = (0..ein).map(|i| ((i * 37) % 401) as i16 - 200).collect();
        let w: Vec<i8> = (0..zeilen * ein).map(|i| ((i * 31) % 255) as i8).collect();
        let shifts: Vec<u8> = (0..zeilen).map(|i| (i % 4) as u8 + 6).collect();

        let rechnen = || linear_w8a16(&x, &w, ein, &shifts, 12, 12);

        kerngrenze_setzen(1);
        let einkernig = rechnen();
        assert_eq!(max_threads(), 1, "die Grenze greift nicht");

        for n in [2usize, 3, 8, 64] {
            kerngrenze_setzen(n);
            assert_eq!(rechnen(), einkernig, "bei {n} Kernen kam etwas anderes heraus");
        }

        kerngrenze_setzen(0);
        assert_eq!(rechnen(), einkernig, "ohne Grenze kam etwas anderes heraus");
    }

    /// Eine Grenze ueber der Maschine hebt die Maschine nicht an, und
    /// eine Grenze von null gibt sie wieder frei.
    #[test]
    fn die_grenze_bleibt_zwischen_eins_und_der_maschine() {
        let vorhanden = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        kerngrenze_setzen(100_000);
        assert_eq!(max_threads(), vorhanden, "die Grenze hat Kerne erfunden");
        kerngrenze_setzen(0);
        assert_eq!(max_threads(), vorhanden);
    }
}

/// Wie die Ausgangsskala eines Buendelteils aussieht.
///
/// ⚑ **Ein Aufzaehlungstyp und nicht zwei Funktionen.** Ein Buendel
/// mischt Teile mit skalarer und mit kanalweiser Skala (gate und up
/// haben eine, down hat eine je Kanal), und zwei getrennte Wege haetten
/// genau diesen Fall nicht.
pub enum Ausgangsskala<'a> {
    /// Eine Skala fuer alle Zeilen, wie bei [`linear_w8a16`].
    Eine(u8),
    /// Eine Skala je Zeile, wie bei [`linear_w8a16_pc`] (Fund 20).
    JeZeile(&'a [u8]),
}

/// Eine Matrix in einem Buendel: Gewichte, Eingabe und Skalen.
pub struct Buendelteil<'a> {
    /// Flach, Zeile fuer Zeile, `in_features` Elemente je Zeile.
    pub w: &'a [i8],
    /// Die Eingabe **dieses** Teils. Teile eines Buendels duerfen
    /// verschiedene Eingaben haben; die down-Projektionen eines
    /// Expertengemischs tun es.
    pub x: &'a [i16],
    pub in_features: usize,
    pub w_shifts: &'a [u8],
    pub act_frac_bits: u8,
    pub aus: Ausgangsskala<'a>,
}

impl Buendelteil<'_> {
    fn zeilen(&self) -> usize {
        self.w_shifts.len()
    }
}

/// **Viele Matrizen in einer Runde.**
///
/// # ⛑ Fund 332 (2026-09-11): eine Poolrunde kostet mehr als die Matrix
///
/// Ein Expertengemisch rechnet je Ebene **vierundzwanzig** kleine
/// Matrizen (acht Experten mal gate, up, down), je Token also
/// **1 152**. Jede war bisher eine eigene Runde im Fadenpool, und eine
/// Runde ist nicht billig: Sie weckt Faeden ueber eine
/// Bedingungsvariable und wartet auf deren Meldung.
///
/// ⚑ **Und sie bekamen dabei nur zwei Faeden.** Eine Expertenmatrix von
/// Qwen3-30B-A3B ist `768 x 2048 = 1 572 864` Multiplikationen, also
/// knapp ueber [`PARALLEL_AB`] und knapp unter dem Zweifachen von
/// [`ARBEIT_JE_THREAD`]: `n = clamp(1, 2, kerne) = 2`. **Zehn von
/// zwoelf Kernen standen still**, waehrend der Aufrufer auf zwei
/// wartete.
///
/// Gemessen mit `kernels/src/bin/rundenprobe.rs`, dieselben 884 736
/// Zeilen ueber denselben Gewichten, alles im Speicher:
///
/// | | Zeit je Token | je Zeile |
/// |---|---|---|
/// | 1 152 Runden zu zwei Faeden | 93,45 ms | 105,63 ns |
/// | 72 Runden zu zwoelf Faeden | **19,12 ms** | 21,61 ns |
///
/// **Die Rechnung ist ein Siebtel der Zeit gewesen.** Der Rest war
/// Wecken, Warten und brachliegende Kerne.
///
/// ⚑ **Und es aendert keine Zahl.** Jede Ausgabezeile bleibt ihr
/// eigenes Skalarprodukt ueber ihre eigene Gewichtszeile; geaendert ist
/// allein, welcher Faden welche Zeile nimmt. Das ist dieselbe
/// Eigenschaft, auf der [`zeilen_rechnen`] ruht, nur ueber
/// Matrixgrenzen hinweg. Geprueft wird sie trotzdem, siehe
/// `ein_buendel_ist_dasselbe_wie_einzeln`.
///
/// Zurueck kommen die Ergebnisse **hintereinander in einem Feld**:
/// `teile[i]` beginnt bei der Summe der Zeilenzahlen davor. Wer sie
/// getrennt braucht, schneidet; eine Zerlegung in `Vec<Vec<i16>>` waere
/// eine Allokation je Teil, und genau die zu sparen ist der Zweck.
pub fn linear_w8a16_buendel(teile: &[Buendelteil<'_>]) -> Vec<i16> {
    if teile.is_empty() {
        return Vec::new();
    }
    for t in teile {
        assert_eq!(
            t.w.len(),
            t.in_features * t.zeilen(),
            "linear_w8a16_buendel: {} Gewichte passen nicht zu {} Zeilen à {} Elementen",
            t.w.len(),
            t.zeilen(),
            t.in_features
        );
        if let Ausgangsskala::JeZeile(f) = t.aus {
            assert_eq!(
                f.len(),
                t.zeilen(),
                "linear_w8a16_buendel: eine Ausgangsskala je Kanal (Fund 20)"
            );
        }
    }

    // ⚑ **Die Grenzen einmal, nicht je Zeile.** `grenzen[i]` ist die
    // erste Zeile von Teil `i`; die Suche darin ist eine
    // Bereichshalbierung ueber hoechstens ein paar Dutzend Eintraege
    // und verschwindet neben den 2 048 Multiplikationen einer Zeile.
    let mut grenzen: Vec<usize> = Vec::with_capacity(teile.len() + 1);
    let mut summe = 0usize;
    let mut arbeit = 0usize;
    grenzen.push(0);
    for t in teile {
        summe += t.zeilen();
        arbeit += t.zeilen().saturating_mul(t.in_features);
        grenzen.push(summe);
    }
    if summe == 0 {
        return Vec::new();
    }

    zeilen_rechnen(summe, arbeit / summe, |z| {
        // `partition_point` liefert die Zahl der Grenzen bis
        // einschliesslich `z`; minus eins ist der Teil, in dem `z` liegt.
        let i = grenzen.partition_point(|g| *g <= z) - 1;
        let t = &teile[i];
        let lokal = z - grenzen[i];
        let row = &t.w[lokal * t.in_features..(lokal + 1) * t.in_features];
        let acc = dot_i8_i16(row, t.x);
        let ziel = match t.aus {
            Ausgangsskala::Eine(f) => f,
            Ausgangsskala::JeZeile(f) => f[lokal],
        };
        clamp_i16_from_i64(rescale_i64(acc, t.w_shifts[lokal] + t.act_frac_bits, ziel))
    })
}

/// **W8A16 fuer mehrere Eingaben auf denselben Gewichten.**
///
/// # ⚑ Dieselbe Rechnung, eine andere Reihenfolge
///
/// `out[b][z]` ist `clamp(rescale(dot(x_b, W_z)))`, also **Element fuer
/// Element dasselbe** wie [`linear_w8a16`] fuer jede Eingabe einzeln.
/// Geaendert ist nur, wann eine Gewichtszeile gelesen wird: Sie wird
/// einmal geholt und fuer alle Eingaben benutzt.
///
/// ⛑ **Und genau daran haengt der Gewinn.** Beim 4B-Modell sind die
/// drei MLP-Matrizen 74,7 MB je Ebene; tokenweise werden sie einmal je
/// Token gelesen, gebuendelt einmal fuer alle. Gemessen am 2026-09-11
/// war die Vorbereitung eines Prompts von 588 Token bei 2,59 TB
/// gelesener Gewichte.
///
/// ⚠️ **Die Bitgleichheit gilt hier per Konstruktion**, nicht laut
/// Messung: Jedes Ausgabeelement entsteht aus derselben `dot_i8_i16`
/// ueber dieselben Bytes in derselben Reihenfolge. Geprueft wird sie
/// trotzdem, siehe `gebuendelt_ist_dasselbe`.
pub fn linear_w8a16_stapel(
    xs: &[&[i16]],
    W: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    out_frac_bits: u8,
) -> Vec<Vec<i16>> {
    stapel_intern(xs, W, in_features, w_shifts, |z| {
        let _ = z;
        out_frac_bits
    }, act_frac_bits)
}

/// Wie [`linear_w8a16_stapel`], mit einer Ausgangsskala je Kanal.
pub fn linear_w8a16_pc_stapel(
    xs: &[&[i16]],
    W: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    out_frac_bits: &[u8],
) -> Vec<Vec<i16>> {
    assert_eq!(
        w_shifts.len(),
        out_frac_bits.len(),
        "linear_w8a16_pc_stapel: eine Ausgangsskala je Kanal (Fund 20)"
    );
    stapel_intern(xs, W, in_features, w_shifts, |z| out_frac_bits[z], act_frac_bits)
}

/// Der gemeinsame Rumpf beider Stapelwege.
fn stapel_intern<S>(
    xs: &[&[i16]],
    W: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    out_frac: S,
    act_frac_bits: u8,
) -> Vec<Vec<i16>>
where
    S: Fn(usize) -> u8 + Sync,
{
    let zeilen = w_shifts.len();
    assert_eq!(
        W.len(),
        in_features * zeilen,
        "linear_stapel: {} Gewichte passen nicht zu {zeilen} Zeilen à {in_features} Elementen",
        W.len()
    );
    let b = xs.len();
    if b == 0 {
        return Vec::new();
    }

    // Fadenzahl wie bei einer einzelnen Eingabe, nur dass jede Zeile
    // `b` Mal gerechnet wird.
    let arbeit = zeilen.saturating_mul(in_features).saturating_mul(b);
    let kerne = max_threads();
    let faeden = if arbeit < PARALLEL_AB || kerne < 2 || zeilen < 2 {
        1
    } else {
        (arbeit / ARBEIT_JE_THREAD).clamp(2, kerne)
    };

    // ⚑ **In Kacheln und nicht am Stueck.** Siehe [`KACHEL`].
    let kachel = kachelbreite();
    let mut aus = vec![vec![0i16; zeilen]; b];
    let mut anfang = 0usize;
    while anfang < b {
        let ende = (anfang + kachel).min(b);
        let teil = &xs[anfang..ende];
        let breite = teil.len();
        let flach = crate::fadenpool::rechnen_breit(zeilen, breite, faeden, |z, ziel| {
            let row = &W[z * in_features..(z + 1) * in_features];
            let schiebung = w_shifts[z] + act_frac_bits;
            let ziel_frac = out_frac(z);
            for (i, wert) in ziel.iter_mut().enumerate() {
                let acc = dot_i8_i16(row, teil[i]);
                *wert = clamp_i16_from_i64(rescale_i64(acc, schiebung, ziel_frac));
            }
        });
        // Zeilenweise gerechnet, eingabeweise zurueckgegeben.
        for z in 0..zeilen {
            for (i, a) in aus[anfang..ende].iter_mut().enumerate() {
                a[z] = flach[z * breite + i];
            }
        }
        anfang = ende;
    }
    aus
}

/// Wie viele Eingaben eine Kachel umfasst.
///
/// # ⛑ Gemessen, und die erste Fassung hatte keine Kachel
///
/// Ohne Kachelung laeuft die innere Schleife ueber **alle** Eingaben.
/// Die Gewichtszeile bleibt dann zwar im Zwischenspeicher, aber die
/// Eingaben tun es nicht: Bei 169 Token sind das 865 KB, die je
/// Gewichtszeile einmal durchlaufen werden, und das sind ueber eine
/// Matrix hinweg Gigabyte an L2-Verkehr. **Gemessen war der gebuendelte
/// MLP damit 84-mal ueber seiner Rechengrenze.**
///
/// ⚑ **Die Kachel ist der Ausgleich zwischen beidem:** Sie soll klein
/// genug sein, dass ihre Eingaben in den ersten Zwischenspeicher
/// passen, und gross genug, dass die Gewichtszeile sich lohnt.
fn kachelbreite() -> usize {
    use std::sync::OnceLock;
    static K: OnceLock<usize> = OnceLock::new();
    *K.get_or_init(|| {
        std::env::var("MYL_KACHEL")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&k: &usize| k > 0)
            .unwrap_or(KACHEL)
    })
}

/// Vorgabe der Kachelbreite.
///
/// ⚑ **Acht, gemessen am 2026-09-11** gegen `myelith-4b` mit einem
/// Prompt von 169 Token, nachdem der KV-Speicher nicht mehr kopiert:
///
/// | Kachel | 1 | 2 | 4 | 8 | 16 | 32 |
/// |---|---|---|---|---|---|---|
/// | Prefill | 7387 ms | 6915 | 6670 | **6647** | 6654 | 6774 |
///
/// Die Kurve ist flach zwischen vier und sechzehn und faellt zu beiden
/// Seiten ab: links, weil die Gewichtszeile sich nicht lohnt, rechts,
/// weil die Eingaben nicht mehr in den ersten Zwischenspeicher passen.
/// **Eine Zahl aus der Mitte einer flachen Kurve ist die richtige**,
/// denn sie traegt auch dann noch, wenn die naechste Maschine anders
/// aussieht.
const KACHEL: usize = 8;
