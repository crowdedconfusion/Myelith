//! Die Lernrate eines Trainingssegments, aus **Tiefe und Chargengrösse**.
//!
//! # ⚑ Warum eine festgeschriebene Zahl nicht trägt
//!
//! Bis zum 2026-09-06 stand die Lernrate im Knoten als Konstante
//! `1 << 12`. Gemessen am selben Tag zerstört dieser Wert ein Netz aus
//! vierundzwanzig Ebenen **in einem Lauf**: Perplexität 2,5 Milliarden
//! gegen einen Ausgangsstand von 23.
//!
//! **Der Grund ist die Verkettung.** Ein Residualstrom läuft durch alle
//! Ebenen; eine Störung in der ersten geht durch alle folgenden
//! Matrizen, bevor sie am Logit ankommt. Was eine Ebene trägt, zerreisst
//! vierundzwanzig.
//!
//! # ⚑ Und warum die Tiefe allein auch nicht reicht
//!
//! Die erste Fassung dieses Moduls, vom Vormittag desselben Tages,
//! kannte nur die Tiefe. **Eine Messung am Nachmittag hat sie
//! widerlegt:** Dieselbe Ebene, dieselbe Rate 2⁻¹⁶, nur acht statt zwei
//! Folgen je Durchgang, und die Perplexität stieg von 41,6 auf 24 084.
//!
//! Der Grund liegt in der Gradientensammlung. `optimierer::sammle`
//! summiert die feinen Einheiten aller Folgen und rundet **einmal**; die
//! Bewegung je Aktualisierung wächst damit **linear mit der Zahl der
//! Folgen**. Wer nur die Tiefe bindet, bindet die halbe Grösse.
//!
//! ⚑ **Gebunden wird deshalb das Gesamtbudget eines Segments**, also
//! `schritte · folgen / nenner`, und nicht die Rate für sich. Das ist
//! die Grösse, die die Bewegung tatsächlich bestimmt.
//!
//! # Die Messpunkte, aus denen die Kurve stammt
//!
//! Alle auf Qwen2.5-0,5B, Teacher Forcing, mit Gradientensammlung, wo
//! angegeben:
//!
//! | Tiefe | Gradienten | Nenner | Budget | Ergebnis |
//! |---|---|---|---|---|
//! | 1 | 16 | 2¹⁶ | 2⁻¹² | 23,33 auf 10,48, trägt |
//! | 1 | 48 | 2¹⁶ | 2⁻¹⁰·⁴ | 41,6 auf 24 084, **zerstört** |
//! | 24 | 72 | 2²⁶ | 2⁻¹⁹·⁸ | Haltemenge **−21,8 %**, trägt |
//! | 24 | 72 | 2²⁴ | 2⁻¹⁷·⁸ | Haltemenge **+8,9 %**, überangepasst |
//!
//! # ⚑ Warum die Kurve bewusst zu vorsichtig ist
//!
//! Aus vier Punkten lässt sich kein Gesetz ziehen, und die Läufe ohne
//! Sammlung sind zusätzlich von der Würfelstreuung überlagert: Bei
//! gleicher Rate und gleichem Lauf liegen zwei Würfelreihen um den
//! Faktor 2 372 auseinander (Fund 189). Was hier steht, ist deshalb
//! **keine Anpassung an die Messpunkte**, sondern eine Schranke, die
//! unter ihnen bleibt.
//!
//! # ⚑ Diese Kurve ist seit dem Abend des 2026-09-06 überholt
//!
//! **Sie gilt für den Weg ohne Normierung**, und der ist nur noch der
//! Rückfall. `optimierer::schritt_normiert` bezieht die Bewegung seither
//! auf das Betragsmaximum der Matrix selbst; damit bedeutet ein Nenner
//! auf jedem Modell dasselbe, und die Kalibrierung je Modell entfällt.
//!
//! **Warum die Kurve trotzdem stehen bleibt:** Der unnormierte Weg
//! existiert weiter, seine Golden Vectors hängen daran, und für ihn ist
//! diese Schranke gemessen. Wer sie löschte, nähme dem Rückfall seine
//! einzige Sicherung.
//!
//! # ⚑ Was diese Kurve NICHT leistet, und das ist wichtig
//!
//! **Sie ist auf Qwen2.5-0,5B kalibriert und überträgt sich nicht.**
//! Gemessen am selben Tag: Qwen3-4B bewegt bei 2⁻¹⁴ und bei 2⁻¹²
//! **kein einziges Gewicht** und erst bei 2⁻⁸, also beim
//! 256-fachen der Budgetrate, und dann richtig (Fund 194).
//!
//! Der Grund liegt in der Gradientenskala. Ein Gradient trägt nach der
//! Bus-Übereinkunft `dL/dZ` auf **einer Skala, die der Aufrufer
//! wählt**; fällt sie je Modell verschieden aus, bedeutet dieselbe Rate
//! zwei verschiedene Schrittweiten. **Keine Regel über Tiefe und
//! Chargengrösse kann das auffangen**, denn sie kennt die Skala nicht.
//!
//! **Die Antwort ist eine Normierung des Gradienten** und keine zweite
//! Kurve je Modell; sie ändert jedes Δm und ist deshalb eine
//! Konsensentscheidung. Bis dahin gilt: Diese Kurve ist eine
//! **obere** Schranke gegen Zerstörung, und ob unter ihr überhaupt
//! etwas in Bewegung gerät, muss der Pod an seinem eigenen Ergebnis
//! prüfen. Ein Segment ohne Wirkung wird abgewiesen (Fund 191), also
//! fällt der Fall auf, statt bezahlt zu werden.
//!
//! **Die beiden Fehler sind nicht gleich teuer, und der Grund dafür hat
//! sich am selben Tag geändert.** Zunächst hiess es: Eine zu grosse Rate
//! zerstört, eine zu kleine verschwendet nur Arbeit. Die Zeile 24/2²⁴
//! oben zeigt den mittleren Fall: **Sie zerstört nicht, sie passt
//! über**, und das sieht ohne Haltemenge wie Fortschritt aus. Zur
//! kleinen Rate hin zu runden bleibt richtig; die Begründung ist
//! schärfer geworden.

/// Der Nenner der Lernrate für den **normierten** Weg.
///
/// # ⚑ Was diese Zahl bedeutet, und warum sie eine Konstante sein darf
///
/// Mit `optimierer::schritt_normiert` bewegt sich das grösste Gewicht
/// einer Matrix je Aktualisierung um **ein Zweihundertsechsundfünfzigstel
/// seines eigenen Betrags**. Diese Bedeutung hängt weder an der Skala
/// des Gradienten noch an der der Gewichte, und damit zum ersten Mal an
/// keinem Modell.
///
/// **Gemessen an vier Modellen mit derselben Zahl**, je mit Haltemenge:
///
/// | Modell | Ebenen | Lernfolgen | Haltemenge |
/// |---|---|---|---|
/// | Qwen2.5-0,5B | 24 | −49,3 % | **−16,2 %** |
/// | Qwen2.5-0,5B | 1 | −14,1 % | **−6,0 %** |
/// | Qwen3-4B | 1 | −4,6 % | **−1,7 %** |
/// | Qwen2.5-7B | 1 | −7,2 % | **−2,3 %** |
/// | Qwen3-30B-A3B (MoE) | 1 | −43,3 % | **−23,8 %** |
///
/// ⚑ **Alle vier lernen, und die dritte Zeile ist der Beleg.**
/// Qwen3-4B bewegte unter
/// der alten, unnormierten Rechnung bei **jeder** regelkonformen Rate
/// kein einziges Gewicht (Fund 194); erst beim 256-fachen der damals
/// zulässigen Rate rührte es sich. Mit der Normierung trägt dieselbe
/// Zahl wie überall sonst.
///
/// ⚑ **Und die Nachbarwerte sind gemessen, nicht geraten:** 4 096 gibt
/// über vierundzwanzig Ebenen nur noch −0,3 Prozent auf der Haltemenge,
/// 65 536 schadet leicht. Nach unten ist die Kurve nicht ausgemessen;
/// wer sie erhöhen will, misst zuerst.
pub const NORMIERTER_NENNER: i64 = 256;

/// Das Bewegungsbudget eines Segments, als negativer Zweierexponent.
///
/// `budget_exponent(tiefe)` gibt `e`, so dass
/// `schritte · folgen / nenner ≤ 2^-e` gelten muss.
///
/// Die Kurve ist `e = 12 + 2·⌈log₂(tiefe)⌉`, also **quadratisch in der
/// Tiefe**:
///
/// | Tiefe | e | zum Vergleich gemessen |
/// |---|---|---|
/// | 1 | 12 | 2⁻¹² trägt, 2⁻¹⁰·⁴ zerstört |
/// | 2 | 14 | |
/// | 3 bis 4 | 16 | |
/// | 5 bis 8 | 18 | |
/// | 9 bis 16 | 20 | |
/// | 17 bis 32 | 22 | 2⁻¹⁹·⁸ trägt, 2⁻²² ist vorsichtiger |
///
/// ⚑ **Die letzte Zeile bleibt unter dem Messpunkt**, und zwar um mehr
/// als das Vierfache. Das ist Absicht: Der Messpunkt daneben, 2⁻¹⁷·⁸,
/// passt bereits über.
pub const fn budget_exponent(tiefe: u32) -> u32 {
    let t = if tiefe == 0 { 1 } else { tiefe };
    // ⌈log₂(t)⌉ ohne Gleitkomma.
    let bits = u32::BITS - (t - 1).leading_zeros();
    12 + 2 * bits
}

/// Der Nenner der Lernrate für ein Segment.
///
/// `tiefe` ist die Zahl der Ebenen, über die das Segment rechnet;
/// `gradienten` ist die Zahl der **Folgen mal Schritte**, also wie viele
/// Gradienten insgesamt in die Gewichte eingehen.
///
/// ⚑ **`gradienten` und nicht `schritte`.** Mit Gradientensammlung
/// gehen je Schritt so viele Gradienten ein, wie Folgen gesammelt
/// wurden; wer nur die Schritte zählte, unterschätzte die Bewegung um
/// genau diesen Faktor. Diese Verwechslung hat am 2026-09-06 einen Lauf
/// von 41,6 auf 24 084 getrieben.
///
/// Der Rückgabewert ist auf eine Zweierpotenz aufgerundet und bei `2^40`
/// gedeckelt.
pub const fn lernrate_nenner(tiefe: u32, gradienten: u32) -> i64 {
    let g = if gradienten == 0 { 1 } else { gradienten };
    // Aufgerundet auf eine Zweierpotenz, damit der Nenner eine
    // Verschiebung bleibt: `nenner = 2^(e + ⌈log₂ g⌉)`.
    let gbits = u32::BITS - (g - 1).leading_zeros();
    let e = budget_exponent(tiefe) + gbits;
    let e = if e > 40 { 40 } else { e };
    1i64 << e
}

/// Trägt ein Nenner diese Tiefe und diese Zahl von Gradienten?
///
/// ⚑ **Kleiner ist erlaubt, grösser nicht.** „Kleiner" heisst eine
/// kleinere Rate, also vorsichtiger. Eine grössere als die Schranke ist
/// der teure Fehler, und zwar in zwei Stufen: Sie passt zuerst über und
/// zerstört dann.
pub const fn nenner_traegt(nenner: i64, tiefe: u32, gradienten: u32) -> bool {
    nenner >= lernrate_nenner(tiefe, gradienten)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Budgettabelle aus dem Doc-Kommentar, Zeile für Zeile.
    #[test]
    fn die_budgetkurve_stimmt_mit_der_rechnung() {
        assert_eq!(budget_exponent(1), 12);
        assert_eq!(budget_exponent(2), 14);
        assert_eq!(budget_exponent(3), 16);
        assert_eq!(budget_exponent(4), 16);
        assert_eq!(budget_exponent(8), 18);
        assert_eq!(budget_exponent(16), 20);
        assert_eq!(budget_exponent(24), 22);
        assert_eq!(budget_exponent(32), 22);
    }

    /// ⚑ **Die Messpunkte, die getragen haben, sind erlaubt; die, die
    /// zerstört oder übergepasst haben, nicht.**
    ///
    /// Das ist der eigentliche Inhalt dieses Moduls, und er wird hier
    /// gegen die Zahlen gehalten statt nur behauptet.
    #[test]
    fn die_kurve_trennt_die_messpunkte() {
        // Eine Ebene, 16 Gradienten bei 2^-16: hat getragen.
        assert!(nenner_traegt(1 << 16, 1, 16), "der gute Ein-Ebenen-Lauf faellt durch");
        // Dieselbe Ebene, 48 Gradienten bei 2^-16: hat zerstoert.
        assert!(
            !nenner_traegt(1 << 16, 1, 48),
            "der zerstoerende Lauf muesste abgewiesen werden"
        );
        // 24 Ebenen, 72 Gradienten: 2^-24 hat uebergepasst, 2^-26 getragen.
        assert!(!nenner_traegt(1 << 24, 24, 72), "die ueberangepasste Rate darf nicht tragen");
        assert!(!nenner_traegt(1 << 26, 24, 72), "die Kurve ist bewusst vorsichtiger");
        assert!(nenner_traegt(1 << 29, 24, 72), "die Kurve traegt ihre eigene Zahl nicht");
    }

    /// ⚑ **Die festgeschriebene Zahl des Knotens traegt genau einen
    /// Fall, und der kommt im Betrieb nicht vor.**
    ///
    /// `2^-12` liegt exakt auf dem Budget **einer** Ebene mit **einem**
    /// Gradienten. Genau daher stammte die Zahl: aus einem Lauf ueber
    /// eine Ebene gegen ein Zielwort. Falsch war sie nicht an sich,
    /// sondern weil sie fuer vierundzwanzig Ebenen und zweiundsiebzig
    /// Gradienten bestellt wurde.
    ///
    /// **Der Test haelt beides fest**, denn „die alte Zahl war Unsinn"
    /// waere eine bequeme und falsche Zusammenfassung.
    #[test]
    fn die_alte_konstante_traegt_genau_ihren_herkunftsfall() {
        assert!(nenner_traegt(1 << 12, 1, 1), "eine Ebene, ein Gradient: genau auf dem Budget");
        assert!(!nenner_traegt(1 << 12, 1, 2), "schon zwei Gradienten reissen es");
        assert!(!nenner_traegt(1 << 12, 24, 72), "und der Betriebsfall erst recht");
    }

    /// Mehr Gradienten verlangen einen groesseren Nenner, genau linear.
    ///
    /// ⚑ **Das ist der Punkt, den die erste Fassung dieses Moduls
    /// verfehlte**, und diese Zusicherung ist ihre Gegenprobe.
    #[test]
    fn mehr_gradienten_verlangen_eine_kleinere_rate() {
        let eins = lernrate_nenner(4, 1);
        let acht = lernrate_nenner(4, 8);
        assert_eq!(acht, eins * 8, "acht Gradienten brauchen den achtfachen Nenner");
        assert!(lernrate_nenner(4, 9) > acht, "neun mehr als acht");
    }

    /// Kleiner ist erlaubt, groesser nicht.
    #[test]
    fn vorsichtiger_ist_erlaubt() {
        assert!(nenner_traegt(1 << 30, 4, 8));
        assert!(!nenner_traegt(1 << 18, 4, 8));
    }

    /// Der Deckel greift, und die Raender stuerzen nicht ab.
    #[test]
    fn deckel_und_rand() {
        assert_eq!(budget_exponent(0), 12);
        assert_eq!(lernrate_nenner(1, 0), 1 << 12);
        assert_eq!(lernrate_nenner(u32::MAX, u32::MAX), 1i64 << 40);
    }
}
