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

use crate::fixed_point::{clamp_i16, rescale_i64, rshift_round_i64, rshift_round_i128};

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
    nachschlagen_identitaet_oben(x, lut, offset, in_frac, out_frac)
}

/// **Die Randfortsetzung „oben der Eingang, unten null"**, einmal.
///
/// ⚑ **Sie gilt fuer SiLU und fuer Softplus, und das ist kein Zufall.**
/// `silu(x) = x·σ(x)` und `softplus(x) = log(1+exp(x))` laufen beide fuer
/// grosse positive `x` gegen `x` und fuer grosse negative gegen null; der
/// Fehler ist in beiden Faellen von der Groessenordnung `exp(-|x|)` und
/// bei jeder hier darstellbaren Aufloesung null.
///
/// ⛔️ **Deshalb steht die Regel hier einmal und nicht zweimal.** Zwei
/// Fassungen derselben Fortsetzung laufen auseinander, sobald jemand die
/// eine anfasst, und die andere meldet sich nicht. 📌 **Genau so ist
/// Fund 349 entstanden**, als Vorwaerts- und Rueckwaertspass an ihren
/// Raendern Verschiedenes sagten.
#[inline]
fn nachschlagen_identitaet_oben(
    x: i32,
    lut: &[i16],
    offset: i16,
    in_frac: u8,
    out_frac: u8,
) -> i64 {
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

/// **Der Zerfall `exp(-d)`, zerlegt in Tabelle und Reihe.**
///
/// ```text
/// exp(-d) = tabelle[d_grob] * (1 - x + x^2/2),   x = d - d_grob
/// ```
///
/// `d` liegt in `2^-d_frac` und ist nicht negativ; `tabelle` haelt
/// `exp(-d_grob)` in `2^-aus_frac` auf einem Raster von
/// `2^-raster_frac`. Die Ausgabe kommt in `2^-aus_frac`.
///
/// # ⛔️ Warum nicht einfach eine `exp`-Tabelle
///
/// Sie scheitert **nicht am Ausgang, sondern am Eingang**. Nahe `d = 0`
/// ist `dg/dd = -1`, ein Eingangsraster von `2^-k` ergibt also einen
/// Fehler von `2^-k` in `g`, und `g` geht ueber die ganze Folge in ein
/// Produkt ein.
///
/// ⚠️ **Gemessen am 2026-09-21** ueber alle 30 Zustandsebenen mit dem
/// Kriterium `|dg| * min(N, 1/(1-g)) < 2^-8` bei `N = 262144`: Ein
/// Raster von `2^-8` ergibt 1,0, und **ein Raster von `2^-14` ergibt
/// ebenfalls 1,0**. Feiner zu rastern verschiebt die Grenze nur.
///
/// 📌 **Wo eine Tabelle an ihrer Eingangsaufloesung scheitert, hilft
/// keine groessere Tabelle, sondern eine Zerlegung.**
///
/// # ⚑ Drei Reihenglieder, und das ist beweisbar
///
/// Bei `x < 2^-raster_frac = 2^-8` ist das naechste Glied `x^3/6`
/// kleiner als `2^-27,6`, also unter einer letzten Stelle bei 28
/// Bruchbits. **Die Schranke steht hier und ist nicht gemessen**; die
/// Messung bestaetigt sie nur (2,1e-6 gegen eine Grenze von 3,9e-3).
#[inline]
pub fn zerfall_nachschlagen(
    d: i64,
    tabelle: &[i32],
    d_frac: u32,
    raster_frac: u32,
    aus_frac: u32,
) -> i64 {
    debug_assert!(d >= 0, "der Zerfallsexponent ist nie negativ");
    debug_assert!(d_frac >= aus_frac && aus_frac >= raster_frac);

    let schiebung = d_frac - raster_frac;
    let grob = (d >> schiebung) as usize;
    // ⚠️ Jenseits der Tabelle ist `exp(-d)` kleiner als eine letzte
    // Stelle. **Null und nicht der letzte Eintrag**: Ein Klemmen haette
    // den Zustand nie ganz verblassen lassen.
    if grob >= tabelle.len() {
        return 0;
    }
    let e_grob = i64::from(tabelle[grob]);
    let fein = d - ((grob as i64) << schiebung);

    // Reihe `1 - x + x^2/2`, alles in `2^-aus_frac`.
    let eins = 1i64 << aus_frac;
    let x = rshift_round_i64(fein, (d_frac - aus_frac) as u8);
    // `fein^2` liegt in `2^-2*d_frac`; nach `2^-aus_frac` und durch zwei
    // sind das `2*d_frac - aus_frac + 1` Stellen nach rechts.
    let x2 = rshift_round_i128(
        i128::from(fein) * i128::from(fein),
        2 * d_frac - aus_frac + 1,
    ) as i64;
    let reihe = eins - x + x2;

    rshift_round_i128(i128::from(e_grob) * i128::from(reihe), aus_frac) as i64
}

/// **Softplus, zerlegt in einen groben und einen feinen Teil.**
///
/// ```text
/// softplus(x) = max(x, 0) + log(1 + exp(-|x|))
///                 grob            fein, aus der Tabelle
/// ```
///
/// Gebraucht von der rekurrenten Zustandsschicht fuer den Zerfall:
/// `g = exp(-exp_A * softplus(a + dt_bias))`.
///
/// `x` liegt in `2^-in_frac`, `rest` ist die Tabelle ueber `|x|` in
/// `2^-out_frac`, und die Ausgabe kommt in `2^-out_frac`.
///
/// # ⛔️ Warum zerlegt und nicht eine Tabelle
///
/// Der Zerfall braucht rund **30 Bruchbits**, gemessen ueber alle 30
/// Zustandsebenen des Zielmodells: `exp_A` reicht bis 105,2, und bei 8
/// Bruchbits ist die kleinste Stufe 1/256, mal 105 also 0,41. `g`
/// spraenge damit um den Faktor 0,66.
///
/// ⚠️ **30 Bruchbits passen aber nicht zu Softplus selbst.** In `i32`
/// reicht Festkomma mit 30 Bruchbits nur bis zum Wert **2**; Softplus
/// geht ueber den gemessenen Eingangsbereich bis 32. Die Tabelle passt
/// schlicht nicht in den Typ.
///
/// ⚑ **Der Rest liegt immer in `(0, 0.693]`** und braucht damit nur
/// `0,693 * 2^30 = 7,4e8`. **Und er ist genau der Teil, der die
/// Feinheit braucht**: Fuer stark negative `x` ist er der ganze
/// Softplus. Der grobe Teil ist exakt und kostet keine Tabelle.
///
/// 📌 **Eine Umformung, die den feinen vom groben Teil trennt, ist mehr
/// wert als ein breiterer Typ.**
///
/// ⚑ **Die Tabelle laeuft ueber `|x|` und ist deshalb halb so lang.**
/// Oberhalb ihres Endes ist der Rest kleiner als eine letzte Stelle
/// (`exp(-32) = 1,3e-14` gegen `2^-30 = 9,3e-10`) und damit null.
#[inline]
/// **SiLU ohne das Eingangsraster: `silu(x) = x * sigmoid(x)`.**
///
/// # ⛔️ Fund 417: warum die Tabelle ueber `x` hier nicht reicht
///
/// `silu_nachschlagen` schlaegt **x selbst** nach und zahlt dafuer das
/// Eingangsraster der Tabelle: `theta_v` gibt ihr `input_frac_bits = 6`,
/// also ein Raster von 1/64. Das ist reichlich fuer das MLP, dessen
/// Torwerte bis 278 reichen, und es ist **vernichtend** fuer den
/// rekurrenten Zweig: Dort hat der Faltungsausgang von `q` und `k` einen
/// Effektivwert um 0,035, also **zwei Rasterschritte**. Gemessen am
/// Qwen3.6-35B-A3B, Ebene 0: 7016 von 8192 Kanaelen wurden exakt null,
/// und weil `q` und `k` danach auf Einheitslaenge normiert werden, ist
/// die Groesse nicht wiederherstellbar; uebrig bleibt eine falsche
/// **Richtung**. Der Schluesselvektor wich um 35 Prozent ab, und das
/// Modell gab Kauderwelsch aus.
///
/// ⚑ **Die Zerlegung loest das, ohne eine Tabelle zu verfeinern.**
/// `x` behaelt die volle Aufloesung des Akkumulators; aus der Tabelle
/// kommt nur der **Faktor** `sigmoid(x)`, und der ist unempfindlich:
/// Bei einem Raster von 1/512 aendert sich `sigmoid` um hoechstens
/// 1/2048, also um 0,2 Prozent des Faktors, waehrend ein Raster auf `x`
/// den Wert selbst ausloescht.
///
/// 📌 **Eine Tabelle ueber `x` quantisiert `x`; eine Tabelle, die nur
/// einen Faktor liefert, laesst `x` unberuehrt.** Dieselbe Einsicht
/// steht hinter der Zerlegung von `softplus` gleich darunter.
///
/// ⚠️ **Nicht fuer das MLP.** Dort gilt weiter `silu_nachschlagen`:
/// Seine Tabelle ist der Anker der Konformitaetsvektoren, und diese
/// Zerlegung waere dort zwar genauer, aber eine andere Funktion.
pub fn silu_zerlegt(
    x: i64,
    x_frac: u8,
    sigmoid_lut: &[i16],
    versatz: i16,
    sigmoid_ein_frac: u8,
    sigmoid_aus_frac: u8,
    aus_frac: u8,
) -> i64 {
    // Das Argument der Tabelle, gesaettigt statt abgeschnitten: ein
    // Abschneiden drehte das Vorzeichen und damit die Richtung des Tores.
    let arg = rescale_i64(x, x_frac, sigmoid_ein_frac)
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let s = sigmoid_nachschlagen(arg, sigmoid_lut, versatz, sigmoid_ein_frac, sigmoid_aus_frac);
    // `x` traegt `x_frac`, `s` traegt `sigmoid_aus_frac`; das Produkt
    // traegt die Summe und muss auf `aus_frac`.
    let schiebung = i32::from(x_frac) + i32::from(sigmoid_aus_frac) - i32::from(aus_frac);
    let prod = i128::from(x) * i128::from(s);
    let wert = if schiebung >= 0 {
        rshift_round_i128(prod, schiebung as u32)
    } else {
        prod << (-schiebung) as u32
    };
    wert.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

pub fn softplus_nachschlagen(x: i32, rest: &[i32], in_frac: u8, out_frac: u8) -> i64 {
    debug_assert!(
        out_frac >= in_frac,
        "softplus: der grobe Teil wird nach links geschoben, out_frac muss reichen"
    );
    let betrag = x.unsigned_abs() as usize;
    let feiner = if betrag < rest.len() { i64::from(rest[betrag]) } else { 0 };
    let grober = i64::from(x.max(0)) << (out_frac - in_frac);
    grober + feiner
}

/// **Sigmoid eines Wertes in der Tabellendomaene, auch jenseits der
/// Tabelle.** `1/(1+exp(-x))`, Eingang auf `in_frac`, Ausgabe auf
/// `out_frac`.
///
/// Gebraucht an drei Stellen der neuen Architektur: der Schreibstaerke
/// `beta` der Zustandsschicht, dem Tor am Attention-Ausgang und dem Tor
/// des geteilten Experten.
///
/// | Eingang | Ausgabe |
/// |---|---|
/// | oberhalb der Tabelle | `1`, also `1 << out_frac` |
/// | in der Tabelle | der Tabellenwert |
/// | unterhalb der Tabelle | `0` |
///
/// ⚑ **Die Fortsetzung ist eine Konstante und keine Identitaet**, anders
/// als bei SiLU und Softplus: `σ` ist beschraenkt. Sie ist trotzdem an
/// beiden Raendern stetig, denn der letzte Tabelleneintrag ist `1` und
/// der erste `0` bei jeder hier darstellbaren Aufloesung.
///
/// ⚠️ **`in_frac` wird nicht gebraucht und steht trotzdem im Kopf.** Die
/// drei Nachschlagefunktionen sollen gleich aufzurufen sein; eine
/// abweichende Signatur waere eine Stolperstelle fuer den Aufrufer, und
/// der Uebersetzer entfernt den ungenutzten Wert ohnehin.
#[inline]
pub fn sigmoid_nachschlagen(x: i32, lut: &[i16], offset: i16, _in_frac: u8, out_frac: u8) -> i64 {
    let oben = lut.len() as i32 - 1 - offset as i32;
    let unten = -(offset as i32);
    if x > oben {
        1i64 << out_frac
    } else if x < unten {
        0
    } else {
        i64::from(lut_lookup(x as i16, lut, 0, offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Eine Minitabelle, deren Werte bekannt sind.**
    ///
    /// ⚑ Geprueft wird hier die **Randfortsetzung** und nicht der Inhalt
    /// der Tabelle. Den erzeugt die Python-Seite, und dort liegen auch
    /// seine Proben; **eine zweite Erzeugung in Rust waere dieselbe
    /// Rechnung an einem zweiten Ort.** Hier zaehlt nur, was ausserhalb
    /// der Tabelle passiert.
    fn minitabelle() -> (Vec<i16>, i16) {
        // Index -2 bis +2, Offset 2. Werte frei gewaehlt und wiedererkennbar.
        (vec![10, 20, 30, 40, 50], 2)
    }

    /// Eine Sigmoid-Tabelle wie die des Modells: Raster `ein_frac`,
    /// Werte auf `aus_frac`.
    fn sigmoid_tabelle(ein_frac: u8, aus_frac: u8, versatz: i16) -> Vec<i16> {
        let n = 2 * versatz as usize;
        (0..n)
            .map(|i| {
                let x = (i as f64 - versatz as f64) / (1u32 << ein_frac) as f64;
                let y = 1.0 / (1.0 + (-x).exp());
                (y * (1u32 << aus_frac) as f64).round().clamp(-32768.0, 32767.0) as i16
            })
            .collect()
    }

    /// ⛔️ **Fund 417: ein Wert unter dem halben Raster der SiLU-Tabelle
    /// ueberlebt die Zerlegung und wird von der Tabelle ausgeloescht.**
    ///
    /// Das ist die Probe, die den Fehler gefunden haette. Bei
    /// `input_frac_bits = 6` ist das halbe Raster 1/128; jedes `x`
    /// darunter wird zu null nachgeschlagen, und `silu(x) = 0` ist fuer
    /// `x != 0` schlicht falsch. Im rekurrenten Zweig traf das
    /// **7016 von 8192 Kanaelen**.
    #[test]
    fn ein_winziger_wert_ueberlebt_die_zerlegung() {
        let lut = sigmoid_tabelle(9, 14, 8192);
        let x_frac = 20u8;
        let aus_frac = 9u8;
        // x = 0,007, also deutlich unter dem halben SiLU-Raster 1/128.
        let x = (0.007_f64 * (1u32 << x_frac) as f64).round() as i64;

        let ist = silu_zerlegt(x, x_frac, &lut, 8192, 9, 14, aus_frac);
        let soll = 0.007_f64 / (1.0 + (-0.007_f64).exp()) * (1u32 << aus_frac) as f64;

        assert!(ist != 0, "die Zerlegung darf einen kleinen Wert nicht ausloeschen");
        assert!(
            (ist as f64 - soll).abs() <= 1.0,
            "silu_zerlegt({x}) = {ist}, erwartet rund {soll:.2}"
        );

        // Und die Gegenrichtung: ueber ein Raster von 1/64 nachgeschlagen
        // waere derselbe Eingang exakt null gewesen.
        let auf_raster = rshift_round_i64(x, x_frac - 6);
        assert_eq!(auf_raster, 0, "die Vorbedingung dieser Probe stimmt nicht mehr");
    }

    /// **Die Zerlegung trifft SiLU auch dort, wo die Werte gross sind.**
    ///
    /// ⚑ Ohne diese Zeile bliebe die Probe darueber auch dann gruen,
    /// wenn `silu_zerlegt` schlicht `x / 2` zurueckgaebe.
    #[test]
    fn die_zerlegung_trifft_silu_auch_im_grossen() {
        let lut = sigmoid_tabelle(9, 14, 8192);
        let x_frac = 12u8;
        let aus_frac = 8u8;
        for &x_f in &[-6.0_f64, -1.5, -0.25, 0.25, 1.5, 6.0, 12.0] {
            let x = (x_f * (1u32 << x_frac) as f64).round() as i64;
            let ist = silu_zerlegt(x, x_frac, &lut, 8192, 9, 14, aus_frac);
            let soll = x_f / (1.0 + (-x_f).exp()) * (1u32 << aus_frac) as f64;
            assert!(
                (ist as f64 - soll).abs() <= 1.0,
                "silu_zerlegt({x_f}) = {ist}, erwartet rund {soll:.2}"
            );
        }
    }

    /// ⛔️ **Sigmoid saettigt an beiden Raendern auf Konstanten.**
    #[test]
    fn sigmoid_saettigt_oben_auf_eins_und_unten_auf_null() {
        let (lut, offset) = minitabelle();
        let out_frac = 8u8;
        // In der Tabelle: der Eintrag.
        assert_eq!(sigmoid_nachschlagen(0, &lut, offset, 6, out_frac), 30);
        assert_eq!(sigmoid_nachschlagen(2, &lut, offset, 6, out_frac), 50);
        assert_eq!(sigmoid_nachschlagen(-2, &lut, offset, 6, out_frac), 10);
        // Darueber: eins, also 1 << out_frac.
        assert_eq!(sigmoid_nachschlagen(3, &lut, offset, 6, out_frac), 1 << out_frac);
        assert_eq!(sigmoid_nachschlagen(10_000, &lut, offset, 6, out_frac), 1 << out_frac);
        // Darunter: null.
        assert_eq!(sigmoid_nachschlagen(-3, &lut, offset, 6, out_frac), 0);
        assert_eq!(sigmoid_nachschlagen(-10_000, &lut, offset, 6, out_frac), 0);
    }

    /// Eine Resttabelle `log(1 + exp(-|x|))`, wie das Modell sie
    /// mitbringt: `in_frac` Bruchbits am Eingang, `out_frac` am Ausgang.
    fn resttabelle(in_frac: u8, out_frac: u8, laenge: usize) -> Vec<i32> {
        (0..laenge)
            .map(|i| {
                let x = i as f64 / (1u32 << in_frac) as f64;
                ((-x).exp().ln_1p() * (1u32 << out_frac) as f64).round() as i32
            })
            .collect()
    }

    /// Die Zerfallstabelle `exp(-d_grob)`, wie das Modell sie mitbringt.
    fn zerfallstabelle(raster_frac: u32, aus_frac: u32, max_input: usize) -> Vec<i32> {
        let n = max_input * (1usize << raster_frac);
        (0..n)
            .map(|i| {
                let d = i as f64 / (1u32 << raster_frac) as f64;
                ((-d).exp() * (1u64 << aus_frac) as f64).round() as i32
            })
            .collect()
    }

    /// ⛔️ **Der Zerfall trifft die Referenz, auch ganz nahe bei eins.**
    ///
    /// ⚑ **Genau dort scheitert eine direkte Tabelle**, und deshalb
    /// steht der Bereich `d < 2^-10` hier ausdruecklich drin: Ein
    /// Verfahren, das `g` nahe eins nicht aufloest, faellt sonst nicht
    /// auf, weil der grosse Rest der Werte gut aussieht.
    #[test]
    fn der_zerfall_trifft_die_referenz_auch_nahe_eins() {
        let (df, rf, af) = (30u32, 8u32, 28u32);
        let tab = zerfallstabelle(rf, af, 64);
        let eins = (1u64 << af) as f64;
        let mut schlimmster = 0.0f64;
        // Von sehr klein bis gross, logarithmisch abgetastet.
        for e in 0..60i32 {
            let d_f = 2f64.powi(-20 + e / 2);
            if d_f >= 64.0 {
                break;
            }
            let d = (d_f * (1u64 << df) as f64) as i64;
            let ist = zerfall_nachschlagen(d, &tab, df, rf, af) as f64 / eins;
            let soll = (-(d as f64 / (1u64 << df) as f64)).exp();
            schlimmster = schlimmster.max((ist - soll).abs());
        }
        assert!(
            schlimmster * eins <= 2.0,
            "groesster Abstand {} letzte Stellen von 2^-{af}",
            schlimmster * eins
        );
    }

    /// ⚑ **Die Eigenschaft, die die Zerlegung ueberhaupt erlaubt:
    /// `exp(-(a+b)) = exp(-a) * exp(-b)`.**
    ///
    /// 📌 **Sie braucht keine Vergleichswerte.** Stimmt sie nicht, ist
    /// das Aufteilen in groben und feinen Teil nicht erlaubt, und dann
    /// hilft auch keine bessere Tabelle.
    #[test]
    fn der_zerfall_ist_multiplikativ() {
        let (df, rf, af) = (30u32, 8u32, 28u32);
        let tab = zerfallstabelle(rf, af, 64);
        for &(a, b) in &[(1i64 << 20, 1i64 << 22), (1 << 25, 1 << 24), (3 << 26, 1 << 20)] {
            let ganz = zerfall_nachschlagen(a + b, &tab, df, rf, af);
            // ⚑ Die Klammern sind nicht kosmetisch: `*` bindet
            //   staerker als `>>`, und wer das beim Lesen andersherum
            //   annimmt, liest eine andere Rechnung.
            let geteilt = (((zerfall_nachschlagen(a, &tab, df, rf, af) as i128)
                * (zerfall_nachschlagen(b, &tab, df, rf, af) as i128))
                >> af) as i64;
            let ab = (ganz - geteilt).abs();
            assert!(
                ab <= 8,
                "exp(-(a+b)) und exp(-a)*exp(-b) weichen um {ab} Stellen ab"
            );
        }
    }

    /// ⚠️ **Jenseits der Tabelle ist der Zerfall null und nicht der
    /// letzte Eintrag.**
    ///
    /// 📌 Ein Klemmen haette den Zustand nie ganz verblassen lassen: Ein
    /// Kopf mit sehr schnellem Zerfall truege dann ewig einen Rest.
    #[test]
    fn jenseits_der_tabelle_verblasst_der_zustand_ganz() {
        let (df, rf, af) = (30u32, 8u32, 28u32);
        let tab = zerfallstabelle(rf, af, 64);
        let weit = 100i64 << df;
        assert_eq!(zerfall_nachschlagen(weit, &tab, df, rf, af), 0);
    }

    /// **Bei `d = 0` bleibt der Zustand unveraendert.**
    #[test]
    fn ohne_zerfall_bleibt_alles_stehen() {
        let (df, rf, af) = (30u32, 8u32, 28u32);
        let tab = zerfallstabelle(rf, af, 64);
        assert_eq!(zerfall_nachschlagen(0, &tab, df, rf, af), 1i64 << af);
    }

    /// ⛔️ **Die Zerlegung ist exakt: `softplus(x) - softplus(-x) = x`.**
    ///
    /// ⚑ **Die schaerfste Probe, die es hier gibt**, und sie braucht
    /// keine Vergleichswerte: `log(1+e^x) - log(1+e^-x) = x` gilt
    /// mathematisch, und in der Zerlegung faellt der Resthalt heraus,
    /// weil beide Seiten denselben Betrag nachschlagen. **Stimmt sie
    /// nicht, ist die Zerlegung falsch umgesetzt**, ganz gleich wie gut
    /// die Tabelle ist.
    #[test]
    fn die_zerlegung_ist_exakt_antisymmetrisch() {
        let (in_frac, out_frac) = (8u8, 30u8);
        let rest = resttabelle(in_frac, out_frac, 8192);
        for &x in &[0i32, 1, 7, 256, 1000, 8191, 9000, 100_000] {
            let plus = softplus_nachschlagen(x, &rest, in_frac, out_frac);
            let minus = softplus_nachschlagen(-x, &rest, in_frac, out_frac);
            assert_eq!(
                plus - minus,
                i64::from(x) << (out_frac - in_frac),
                "softplus(x) - softplus(-x) ist nicht x (x={x})"
            );
        }
    }

    /// ⚑ **Der feine Teil liegt immer in `(0, log 2]`**, und genau das
    /// macht ihn in `i32` mit 30 Bruchbits darstellbar.
    #[test]
    fn der_feine_teil_bleibt_unter_log_zwei() {
        let (in_frac, out_frac) = (8u8, 30u8);
        let rest = resttabelle(in_frac, out_frac, 8192);
        let log2 = (2.0f64.ln() * (1u32 << out_frac) as f64).round() as i64;
        for &x in &[-9000i32, -1000, -1, 0, 1, 1000, 9000] {
            let fein = softplus_nachschlagen(x, &rest, in_frac, out_frac)
                - (i64::from(x.max(0)) << (out_frac - in_frac));
            assert!(
                (0..=log2).contains(&fein),
                "der feine Teil liegt ausserhalb von (0, log 2] (x={x}, fein={fein})"
            );
        }
    }

    /// **Softplus trifft die Referenz**, ueber den gemessenen
    /// Eingangsbereich der Zustandsschicht.
    #[test]
    fn softplus_trifft_die_referenz() {
        let (in_frac, out_frac) = (8u8, 30u8);
        let rest = resttabelle(in_frac, out_frac, 8192);
        let eins = (1u64 << out_frac) as f64;
        let mut schlimmster = 0.0f64;
        // [-22, 30] ist der gemessene Bereich von a + dt_bias.
        let mut x = -22 * 256;
        while x <= 30 * 256 {
            let ist = softplus_nachschlagen(x, &rest, in_frac, out_frac) as f64 / eins;
            let soll = (x as f64 / 256.0).exp().ln_1p();
            schlimmster = schlimmster.max((ist - soll).abs());
            x += 7;
        }
        // Eine halbe letzte Stelle ist das Beste, was eine Tabelle kann.
        assert!(
            schlimmster * eins <= 1.0,
            "groesster Abstand {} letzte Stellen",
            schlimmster * eins
        );
    }

    /// ⚠️ **Und die Gegenprobe: Sigmoid ist beschraenkt, Softplus nicht.**
    ///
    /// Ohne diese Zeile bestuenden die Proben darueber auch dann, wenn
    /// beide Funktionen dasselbe taeten.
    #[test]
    fn sigmoid_bleibt_beschraenkt_und_softplus_waechst() {
        let (lut, offset) = minitabelle();
        let rest = resttabelle(8, 30, 8192);
        let gross = 100_000i32;
        assert_eq!(
            sigmoid_nachschlagen(gross, &lut, offset, 6, 8),
            1i64 << 8,
            "Sigmoid saettigt nicht bei eins"
        );
        assert!(
            softplus_nachschlagen(gross, &rest, 8, 30) > (1i64 << 30),
            "Softplus waechst nicht ueber eins hinaus"
        );
    }

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
