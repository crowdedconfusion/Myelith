//! Eigenschaftstests für die Allaussagen der Ganzzahl-Primitiven.
//!
//! ## ⚑ Warum es diese Datei gibt
//!
//! Fünfzig Testnamen in diesem Repositorium tragen ein „immer", „nie",
//! „jede" oder „exakt", und jede dieser Aussagen wurde an zwei bis fünf
//! getippten Beispielen geprüft. **Der Name führt die Regel, die Prüfung
//! sieht eine Stichprobe.**
//!
//! **Fund 42 ist der Beleg, und er ist eindeutig:** Drei Tests der
//! Bisektion waren grün, keiner prüfte, ob die genannte Position die
//! richtige ist, und das Verfahren belohnte in fünfzehn von sechzehn
//! Fällen den Betrüger.
//!
//! ## ⚑ Erschöpfend, wo es geht, und erst dann ein Generator
//!
//! Die Frage nach `proptest` war im Projekt zweimal verschieden
//! beantwortet worden: einmal ablehnend wegen der zusätzlichen
//! Abhängigkeit, einmal befürwortend wegen der verkleinerten
//! Gegenbeispiele.
//!
//! **Aufgelöst durch eine dritte Möglichkeit, die beide übersehen
//! hatten:** Für einen großen Teil dieser Aussagen ist der Eingaberaum
//! **klein genug, um ihn ganz abzugehen**. Ein erschöpfender Test ist
//! stärker als jeder Zufallstest und braucht keine Abhängigkeit, und ein
//! Gegenbeispiel muss man nicht verkleinern, wenn man ohnehin bei den
//! kleinsten anfängt.
//!
//! Wo der Raum zu groß ist, läuft ein **deterministischer** Generator
//! mit festem Keim: Ein Fehlschlag, der sich nicht wiederholen lässt,
//! wäre in einem Projekt, dessen These die Wiederholbarkeit ist, das
//! Falscheste von allem.

use integer_llm_kernels::fixed_point::rshift_round;
use integer_llm_kernels::integer_math::sqrt_q;

/// Deterministischer Generator, wie ihn die Testsuiten hier schon
/// benutzen. **Kein `rand`, kein Systemzustand:** Derselbe Keim ergibt
/// dieselbe Folge, auf jeder Maschine.
struct Folge(u64);

impl Folge {
    fn neu(keim: u64) -> Self {
        Self(keim ^ 0x9E3779B97F4A7C15)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn i32(&mut self) -> i32 {
        self.next() as u32 as i32
    }
}

// ---------------------------------------------------------------------
// rshift_round: kaufmännisch zur geraden Zahl, auch negativ
// ---------------------------------------------------------------------

/// Die Regel, unabhängig formuliert: `value / 2^shift`, bei genau
/// halbem Rest zur **geraden** Zahl.
///
/// ⚑ **Bewusst über `i128` und mit einer anderen Herleitung.** Eine
/// Referenz, die dieselbe Schiebearithmetik benutzt wie der Prüfling,
/// prüft nur, ob er mit sich selbst übereinstimmt.
fn regel(value: i32, shift: u8) -> i128 {
    // ⚑ Ohne diese Zeile war die **Referenz** falsch, nicht der
    // Prüfling: Bei `shift == 0` ist `n = 1` und der halbe Rest `0`, und
    // die Regel hätte jede ungerade Zahl um eins gehoben. Schieben um
    // null ist die Identität. **Der erste Fehlschlag eines
    // Eigenschaftstests trifft oft den Maßstab, nicht die Sache**, und
    // wer das nicht prüft, „behebt" einen richtigen Code.
    if shift == 0 {
        return value as i128;
    }
    let n = 1i128 << shift;
    let v = value as i128;
    let unten = v.div_euclid(n);
    let rest = v.rem_euclid(n);
    let halb = n / 2;
    if rest > halb || (rest == halb && (unten & 1) != 0) {
        unten + 1
    } else {
        unten
    }
}

/// ⚑ **Erschöpfend über alle Werte in einem Fenster um null und über
/// alle zulässigen Shifts.** Rundungsfehler sitzen an den Grenzen, und
/// die liegen bei kleinen Beträgen und bei den Vielfachen von
/// `2^(shift-1)`.
#[test]
fn rshift_round_trifft_die_regel_erschoepfend_um_null() {
    for shift in 0u8..=30 {
        for value in -4096i32..=4096 {
            assert_eq!(
                rshift_round(value, shift) as i128,
                regel(value, shift),
                "value {value}, shift {shift}"
            );
        }
    }
}

/// Und erschöpfend um die Rundungsgrenzen jedes Shifts: genau ein
/// halber Rest, einer darunter, einer darüber, in beiden Vorzeichen.
#[test]
fn rshift_round_trifft_die_regel_an_jeder_rundungsgrenze() {
    for shift in 1u8..=30 {
        let n = 1i64 << shift;
        for k in -8i64..=8 {
            for versatz in [-1i64, 0, 1] {
                let wert = k * n + n / 2 + versatz;
                if wert < i32::MIN as i64 || wert > i32::MAX as i64 {
                    continue;
                }
                let v = wert as i32;
                assert_eq!(
                    rshift_round(v, shift) as i128,
                    regel(v, shift),
                    "k {k}, versatz {versatz}, shift {shift}"
                );
            }
        }
    }
}

/// Über den ganzen Wertebereich, deterministisch gestreut.
#[test]
fn rshift_round_trifft_die_regel_ueber_den_ganzen_bereich() {
    let mut f = Folge::neu(0x5EED_1234);
    for _ in 0..200_000 {
        let v = f.i32();
        let shift = (f.next() % 31) as u8;
        assert_eq!(
            rshift_round(v, shift) as i128,
            regel(v, shift),
            "value {v}, shift {shift}"
        );
    }
    // Und die Ränder ausdrücklich, die ein Generator selten trifft.
    for shift in 0u8..=30 {
        for v in [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX] {
            assert_eq!(rshift_round(v, shift) as i128, regel(v, shift), "{v} >> {shift}");
        }
    }
}

// ---------------------------------------------------------------------
// sqrt_q: floor der Wurzel, ohne Überlauf im Zwischenschritt
// ---------------------------------------------------------------------

/// Die definierende Eigenschaft, ohne die Wurzel zu ziehen:
/// `r² <= x·2^f < (r+1)²`.
///
/// ⚑ **Das ist stärker als ein Vergleich mit einer Referenzwurzel.** Es
/// prüft die Aussage selbst statt zweier Verfahren gegeneinander, und es
/// kommt ohne Gleitkomma aus.
fn ist_floor_wurzel(r: i32, x: i32, frac_bits: u8) -> bool {
    if x <= 0 {
        return r == 0;
    }
    let ziel = (x as i128) << frac_bits;
    let r = r as i128;
    r * r <= ziel && (r + 1) * (r + 1) > ziel
}

/// Bis zu welchem `x` das Ergebnis noch in `i32` passt.
///
/// ⚑ **Diese Grenze gab es vor dem 2026-08-29 nicht**, weder im Code
/// noch in der Dokumentation. Gefunden hat sie dieser Test im ersten
/// Lauf (Fund 95); seither steht sie als `debug_assert!` bei `sqrt_q`.
fn hoechstes_x(frac_bits: u8) -> i64 {
    ((i32::MAX as i64) * (i32::MAX as i64)) >> frac_bits
}

/// ⚑ Erschöpfend über kleine `x` und **alle** zulässigen `frac_bits`.
/// Genau hier saß Fund 75: `sqrt_q(i32::MAX, 33)` liefert still null.
#[test]
fn sqrt_q_ist_die_floor_wurzel_fuer_jedes_kleine_x_und_jedes_frac_bits() {
    for frac_bits in 0u8..=32 {
        for x in 0i32..=512 {
            let r = sqrt_q(x, frac_bits);
            assert!(
                ist_floor_wurzel(r, x, frac_bits),
                "sqrt_q({x}, {frac_bits}) = {r} ist nicht floor(sqrt(x·2^f))"
            );
        }
    }
}

/// Über den ganzen Bereich, deterministisch gestreut, mit den Rändern.
#[test]
fn sqrt_q_ist_die_floor_wurzel_ueber_den_ganzen_bereich() {
    let mut f = Folge::neu(0xC0FFEE);
    let mut geprueft = 0u32;
    for _ in 0..50_000 {
        let x = f.i32();
        let frac_bits = (f.next() % 33) as u8;
        // Innerhalb der Vorbedingung bleiben; darüber gilt eine andere
        // Aussage, und die prüft der Test darunter.
        if (x as i64) > hoechstes_x(frac_bits) {
            continue;
        }
        let r = sqrt_q(x, frac_bits);
        assert!(
            ist_floor_wurzel(r, x, frac_bits),
            "sqrt_q({x}, {frac_bits}) = {r}"
        );
        geprueft += 1;
    }
    // ⚑ **Zählen, wie viel wirklich geprüft wurde.** Ein Test, der
    // seine Fälle wegfiltert und trotzdem grün meldet, ist genau die
    // Sorte Prüfung, gegen die diese Datei geschrieben ist.
    assert!(geprueft > 20_000, "nur {geprueft} Fälle blieben übrig");
    for frac_bits in 0u8..=32 {
        // ⚑ Die Ränder **und** die Vorbedingungsgrenze selbst, links und
        // rechts davon: Dort wechselt das Verhalten, und genau dort
        // stand die Lücke.
        let grenze = hoechstes_x(frac_bits);
        let mut faelle = vec![i32::MIN, -1, 0, 1, 2, 3];
        for versatz in [-1i64, 0] {
            let w = grenze + versatz;
            if (i32::MIN as i64..=i32::MAX as i64).contains(&w) {
                faelle.push(w as i32);
            }
        }
        if (i32::MAX as i64) <= grenze {
            faelle.extend([i32::MAX - 1, i32::MAX]);
        }
        for x in faelle {
            let r = sqrt_q(x, frac_bits);
            assert!(ist_floor_wurzel(r, x, frac_bits), "sqrt_q({x}, {frac_bits}) = {r}");
        }
    }
}

/// ⚑ Monotonie: Wer mehr hineingibt, bekommt nie weniger heraus. Eine
/// Aussage, die keine der bisherigen Prüfungen traf, und die bei einer
/// binären Suche mit falscher Abbruchbedingung als Erstes bricht.
/// ⚑ **Die Gegenprobe zu Fund 95: Über der Vorbedingung sagt es
/// Bescheid, statt still zu sättigen.**
///
/// Nur im Debug-Bau; im Release ist die Sättigung **stumm**, und genau
/// das ist der Grund, warum die Grenze dokumentiert gehört und nicht nur
/// zugesichert.
#[test]
#[should_panic(expected = "Fund 95")]
fn sqrt_q_sagt_bescheid_wenn_das_ergebnis_nicht_mehr_passt() {
    let frac_bits = 32u8;
    let zu_gross = (hoechstes_x(frac_bits) + 1) as i32;
    let _ = sqrt_q(zu_gross, frac_bits);
}

#[test]
fn sqrt_q_faellt_nie(){
    for frac_bits in [0u8, 1, 7, 8, 15, 16, 31, 32] {
        let mut vorher = sqrt_q(0, frac_bits);
        let obergrenze = 20_000i32.min(hoechstes_x(frac_bits).max(1) as i32);
        for x in 1i32..=obergrenze {
            let jetzt = sqrt_q(x, frac_bits);
            assert!(jetzt >= vorher, "sqrt_q fiel bei x={x}, frac_bits={frac_bits}");
            vorher = jetzt;
        }
    }
}
