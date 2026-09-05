//! Der Trainingsschritt: Gewichte fortschreiben, ganzzahlig und
//! ordnungsfrei (Whitepaper Kap. 7, Anhang B.6.2).
//!
//! ## ⚑ Warum stochastisch gerundet wird, und warum das gemessen ist
//!
//! Ein SGD-Schritt bewegt ein Gewicht im Median um **6,4e-6 einer
//! Rasterstufe**. Wer zur nächsten Stufe rundet, bekommt dann entweder
//! **nichts** oder einen **ganzen Sprung**, und beides ist falsch: Die
//! kleinen Bewegungen, aus denen Lernen besteht, verschwinden.
//!
//! Gemessen an Qwen2.5-0,5B über WikiText-2:
//!
//! | Variante | zurückgehaltener Text | Abstand |
//! |---|---|---|
//! | Gleitkomma-Referenz | 3,0472 → **2,9795** | |
//! | Rundung zur nächsten Stufe | 3,0689 → 3,8713 | **+29,9 %** |
//! | **stochastisches Runden** | 3,0770 → **2,9994** | **+0,67 %** |
//!
//! **Eine einzige geänderte Zeile dreht das Ergebnis.** Deshalb steht
//! sie hier mit dieser Begründung und nicht als Einzeiler.
//!
//! ## ⚑ Der Würfel ist eine Funktion, kein Zustand
//!
//! Naheliegend wäre ein PRNG, der über die Gewichte läuft. **Das wäre
//! hier falsch, und zwar aus zwei Gründen, die beide das Protokoll
//! betreffen:**
//!
//! 1. **Ein Zustand hängt an der Reihenfolge.** Wer die Gewichte in
//!    anderer Reihenfolge durchläuft, bekommt andere Zufallszahlen und
//!    damit andere Gewichte. Zwei ehrliche Miner, die dieselbe Rechnung
//!    verschieden aufteilen, kämen zu verschiedenen Ergebnissen — und
//!    der Redundanzvergleich meldete beide als fehlerhaft.
//! 2. **Ein Zustand müsste übertragen werden.** Er wäre Teil des
//!    Trainingssegments, also des Konsensvertrags.
//!
//! Der Würfel ist deshalb eine reine Funktion aus **(Ebene, Schritt,
//! Index)**. Zwei Miner brauchen sich über nichts zu einigen außer über
//! diese drei Zahlen, die ohnehin feststehen.
//!
//! **Das ist derselbe Gedanke wie die Assoziativität der
//! Ganzzahladdition**, auf die das ganze Projekt gebaut ist: Kein
//! Ergebnis darf von der Reihenfolge abhängen.

use crate::prng::splitmix64;

/// Ein Gradient, wie ihn [`crate::backward`] liefert.
pub type Grad = i32;

/// Ein Mastergewicht.
///
/// Breiter als die Übertragungsform: Aus ihm wird int8, aber gerechnet
/// wird auf ihm. Die kleinen Bewegungen, die ein SGD-Schritt erzeugt,
/// hätten in int8 keinen Platz.
pub type Master = i32;

/// Zusätzliche Bruchstellen unterhalb der Master-Rasterstufe, in denen
/// die Bewegung eines Schrittes ausgerechnet wird.
///
/// ⚑ **Ohne sie gäbe es nichts zu runden.** Der Schritt ist im Median
/// 6,4e-6 einer Stufe groß; wer ihn in Stufen rechnet, rechnet mit null.
/// Zwanzig Bit reichen für Bewegungen bis herunter zu etwa 1e-6 einer
/// Stufe, also für den gemessenen Median.
pub const FEIN_BITS: u32 = 20;

/// Die Bruchstellen eines Mastergewichts, wie das Protokoll sie
/// festlegt.
///
/// # ⚑ Warum das eine Festlegung ist und keine Wahl des Aufrufers
///
/// Aus `master_frac` und der Zeilenverschiebung folgt der reale Wert
/// eines Gewichts (siehe `trainingsschritt::gewicht_aus_master`).
/// **Zwei ehrliche Miner, die denselben Trainingsschritt mit
/// verschiedenen Werten rechnen, bekommen verschiedene Gewichte**, und
/// der Redundanzvergleich meldete beide als fehlerhaft, ohne dass einer
/// gelogen hätte.
///
/// ⚑ **Dieselbe Lage wie beim Würfel**, und dort ist die Antwort
/// ausgeschrieben: Was in die Bitgleichheit eingeht, darf kein Aufrufer
/// wählen.
///
/// # Woher die Zwanzig kommt
///
/// Die Zeilenverschiebungen eines quantisierten Gewichts liegen zwischen
/// null und zwanzig. Der Master muss mindestens so viele Bruchstellen
/// tragen, damit `master_frac − s` nicht negativ wird; bei zwanzig passt
/// der grösste Master (`127 · 2^20`) mit Abstand in `i32`, und der Weg
/// vom Artefakt zum Master und zurück ist exakt.
///
/// ⚑ **Offen bleibt der Ort.** Als Eigenschaft des Zahlenformats gehört
/// die Zahl in die Formatfestlegung neben die übrigen Bruchstellen;
/// heute steht sie hier, weil eine Formatänderung jeden Golden Vector
/// neu erzeugen liesse. **Solange sie hier steht, gilt: ein Aufrufer,
/// der sie überschreibt, verlässt das Protokoll.**
pub const MASTER_FRAC: u8 = 20;

/// Der grösste Master, den die Übertragungsform noch ausdrücken kann.
///
/// # ⚑ Die naheliegende Zahl ist falsch, und zwar um eine Oktave
///
/// Naheliegend wäre `127 << MASTER_FRAC`, denn 127 ist der grösste
/// Betrag eines `i8`, und so prüft es auch der 30B-Lauf. **Die
/// Übertragungsform ist grosszügiger**, und eine Gegenprobe am
/// 2026-09-05 hat es gezeigt:
/// [`crate::trainingsschritt::gewicht_aus_master`] sucht die kleinste
/// Verschiebung `s` mit `betrag >> s <= 127` und lässt `s` bis
/// `master_frac` zu. Bei `s = master_frac` passt alles bis
/// `(128 << master_frac) − 1` hinein, weil das Schieben **abrundet**.
///
/// Der Unterschied ist keine Spitzfindigkeit. Eine Prüfung, die
/// **strenger** ist als die Form, meldet ein Segment als gescheitert,
/// das gerechnet hätte werden können; eine, die **lockerer** ist, lässt
/// den Absturz am fernen Ende passieren, den sie verhindern sollte.
/// Deshalb ist sie hier exakt, und ein Test hält sie gegen die Form.
///
/// ⚑ **Was diese Zahl nicht sagt:** ob die Zeile noch etwas taugt. Ein
/// Master in der letzten Oktave braucht `s = master_frac`, hat also
/// **null Nachkommastellen**: darstellbar, aber ohne jede Auflösung. Der
/// 30B-Lauf schlägt bei `127 << MASTER_FRAC` deshalb bewusst früher an.
pub const MASTER_GRENZE: i64 = (128i64 << MASTER_FRAC) - 1;

/// Verlässt eine dieser Matrizen die Übertragungsform?
///
/// Gibt den Index des ersten Gewichts zurück, dessen Betrag
/// [`MASTER_GRENZE`] überschreitet, sonst `None`.
///
/// # ⚑ Warum es diese Funktion gibt, und warum die Panik nicht reicht
///
/// [`crate::trainingsschritt::gewicht_aus_master`] **paniked** in genau
/// diesem Fall, und das ist dort richtig: Ein still gekapptes Gewicht
/// fiele erst an der Verlustkurve auf. Aber diese Panik kommt **am
/// fernen Ende**, beim Zurückschreiben ins Artefakt, nachdem ein Lauf
/// vielleicht tausend Schritte gerechnet hat.
///
/// ⚑ **Gemessen am 2026-09-05 auf dem echten 30B:** Bei Lernrate 2⁻¹⁰
/// verlässt der Router den Bereich **bei Schritt 67**. Der Testaufbau
/// fand das nur, weil er von Hand danach sah; `schritt` selbst prüft
/// nichts, und die Trainingsschleife merkte es nicht.
///
/// ⚑ **Für geshardetes Training ist eine Panik keine Antwort.** Ein Pod,
/// dessen Segment aus der Form läuft, muss das **melden** können, damit
/// die Kette das Segment als gescheitert verbucht statt auf ein Ergebnis
/// zu warten, das nie kommt. Ein Absturz ist von einem ausgefallenen
/// Miner nicht zu unterscheiden.
pub fn ausserhalb_der_form(master: &[Master]) -> Option<usize> {
    // ⚑ `unsigned_abs` und nicht `abs`: `Master::MIN.abs()` läuft über,
    // und `Master::MIN` ist erreichbar, weil `schritt` genau dorthin
    // klemmt. Ein Prüfer, der am Extremwert paniked, prüft das
    // Interessanteste nicht.
    let grenze = MASTER_GRENZE as u64;
    master.iter().position(|v| u64::from(v.unsigned_abs()) > grenze)
}

/// Der Würfel: eine reine Funktion aus Ebene, Schritt und Index.
///
/// ⚑ **Kein Zustand, keine Reihenfolge, nichts zu übertragen.** Wer
/// dieselben drei Zahlen einsetzt, bekommt dieselbe Zahl heraus, auf
/// jeder Maschine und in jeder Aufrufreihenfolge.
///
/// Die drei Zahlen werden nacheinander eingemischt statt addiert:
/// Addition brächte `(1, 2)` und `(2, 1)` auf denselben Wert, und
/// benachbarte Gewichte bekämen benachbarte Würfe.
#[inline]
pub fn wuerfel(ebene: u32, schritt: u64, index: u64) -> u64 {
    let (s, _) = splitmix64(ebene as u64);
    let (s, _) = splitmix64(s ^ schritt.rotate_left(17));
    let (_, z) = splitmix64(s ^ index.rotate_left(41));
    z
}

/// Rundet einen feinen Wert auf die Master-Rasterstufe, stochastisch.
///
/// `fein` ist in Einheiten von `2^-FEIN_BITS` Rasterstufen. Der ganze
/// Anteil wird übernommen; der Rest entscheidet **mit seiner eigenen
/// Wahrscheinlichkeit**, ob eine Stufe dazukommt.
///
/// ⚑ **Und zwar in Richtung des Vorzeichens.** Bei negativen Werten
/// rundet die ganzzahlige Division in Rust zur Null hin; wer den Rest
/// dann positiv behandelt, bekommt einen systematischen Drift nach oben.
/// Genau dieser Fehler wäre in einem Trainingslauf erst nach Tausenden
/// Schritten sichtbar, und dann als „das Modell lernt nicht".
#[inline]
pub fn runde_stochastisch(fein: i64, wurf: u64) -> i64 {
    let stufe = 1i64 << FEIN_BITS;
    let ganz = fein.div_euclid(stufe);
    let rest = fein.rem_euclid(stufe); // stets 0..stufe, auch negativ
    // Der Wurf entscheidet über die eine zusätzliche Stufe.
    let schwelle = (wurf & ((1u64 << FEIN_BITS) - 1)) as i64;
    if schwelle < rest {
        ganz + 1
    } else {
        ganz
    }
}

/// Wo ein Stück Gewichte im Trainingslauf steht.
///
/// ⚑ **Der Versatz ist nicht Bequemlichkeit, sondern Voraussetzung.**
/// Der Würfel hängt am Index innerhalb der Ebene. Leitete `schritt` ihn
/// aus der Position im übergebenen Stück ab, dann bekäme dasselbe
/// Gewicht je nach Zuschnitt einen anderen Wurf — **und ein Netz, das
/// Arbeit aufteilt, teilt Ebenen auf.** Zwei Miner mit verschiedenem
/// Zuschnitt kämen zu verschiedenen Gewichten, ohne dass einer gelogen
/// hätte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schrittkennung {
    /// Welche Ebene.
    pub ebene: u32,
    /// Der wievielte Trainingsschritt.
    pub schritt: u64,
    /// Der Index des **ersten** übergebenen Gewichts innerhalb der Ebene.
    pub index_versatz: u64,
}

/// Ein Trainingsschritt auf einer Gewichtsscheibe.
///
/// `lr_zaehler / lr_nenner` ist die Lernrate als Bruch zweier
/// Ganzzahlen. ⚑ **Kein Gleitkomma, auch nicht „nur für die Lernrate":**
/// Eine Gleitkommazahl im Konsenspfad ist eine Gleitkommazahl im
/// Konsenspfad, gleich wie klein ihre Rolle scheint.
///
/// Der Schritt ist **ordnungsfrei**: Wer die Indizes in anderer
/// Reihenfolge durchläuft, bekommt dieselben Gewichte.
///
/// **Panik statt stiller Kürzung**, wenn die Längen nicht passen: Zwei
/// Scheiben verschiedener Länge sind ein Aufrufer-Fehler, und ein
/// stillschweigend gekürzter Schritt fiele erst an der Verlustkurve auf.
pub fn schritt(
    master: &mut [Master],
    grad: &[Grad],
    kennung: Schrittkennung,
    lr_zaehler: i64,
    lr_nenner: i64,
) {
    assert_eq!(master.len(), grad.len(), "schritt: Laengen passen nicht");
    assert!(lr_nenner > 0, "schritt: Lernraten-Nenner muss > 0 sein");

    for (i, (w, g)) in master.iter_mut().zip(grad).enumerate() {
        let index = kennung.index_versatz + i as u64;
        // Die Bewegung, in feinen Einheiten unterhalb der Rasterstufe.
        let fein = -(*g as i64) * lr_zaehler * (1i64 << FEIN_BITS) / lr_nenner;
        let stufen = runde_stochastisch(fein, wuerfel(kennung.ebene, kennung.schritt, index));
        // Sättigend: Ein Gewicht, das den Bereich verlässt, bleibt am
        // Rand stehen, statt umzulaufen. Ein umlaufendes Gewicht wäre
        // ein Vorzeichenwechsel aus dem Nichts.
        *w = (*w as i64).saturating_add(stufen).clamp(Master::MIN as i64, Master::MAX as i64)
            as Master;
    }
}

#[cfg(test)]
mod shardtransparenz {
    use super::*;

    /// ⚑ **Der Nenner und die Gradienten muessen einen Rest lassen.**
    ///
    /// Der erste Entwurf dieser Tests nahm Gradienten, die glatt durch
    /// den Nenner teilbar waren. Dann ist die Schrittweite exakt, das
    /// stochastische Runden hat nichts zu entscheiden, **und der Wuerfel
    /// kommt gar nicht vor**. Beide Tests waren gruen und prueften
    /// nichts; die Gegenprobe hat es am 2026-09-05 aufgedeckt.
    ///
    /// Hier ist `grad / NENNER` durchweg null mit grossem Rest, also
    /// entscheidet allein der Wuerfel zwischen null und eins.
    const NENNER: i64 = 1 << 12;

    fn aufbau() -> (Vec<Master>, Vec<i32>, Schrittkennung) {
        let n = 64usize;
        let anfang: Vec<Master> = (0..n).map(|i| (i as Master) * 977 - 20_000).collect();
        let grad: Vec<i32> = (0..n).map(|i| (i as i32) * 37 + 401).collect();
        (anfang, grad, Schrittkennung { ebene: 7, schritt: 3, index_versatz: 0 })
    }

    /// ⚑ **Die Eigenschaft, an der geshardetes Training haengt.**
    ///
    /// Der Wuerfel ist eine reine Funktion aus `(ebene, schritt, index)`,
    /// wobei `index` **innerhalb der Ebene** zaehlt. Wenn das stimmt,
    /// ist es gleichgueltig, ob eine Ebene in einem Stueck oder in
    /// Scheiben fortgeschrieben wird: Zwei Shards, die sich eine Ebene
    /// teilen, kommen zum selben Ergebnis wie ein einzelner Rechner.
    ///
    /// **Ohne diese Eigenschaft waere geshardetes Training nicht gegen
    /// einen Einzelknoten nachrechenbar**, und damit fiele die
    /// Redundanzpruefung aus, auf der die ganze Verifikation ruht.
    ///
    /// ⚑ **Und sie stellt die eigentliche Anforderung an den Shard:**
    /// Er muss seine **globale** Ebenennummer und seinen **globalen**
    /// Versatz innerhalb der Ebene kennen. Ein Shard, der seine Ebenen
    /// bei null durchnummerierte, wuerfelte anders, und niemand saehe es
    /// dem Ergebnis an: Es waere ein plausibles Delta, nur ein anderes.
    #[test]
    fn eine_ebene_in_scheiben_ergibt_dasselbe_wie_am_stueck() {
        let (anfang, grad, kn) = aufbau();
        let n = anfang.len();

        // Am Stueck.
        let mut ganz = anfang.clone();
        schritt(&mut ganz, &grad, kn, 1, NENNER);
        assert_ne!(ganz, anfang, "der Schritt hat gar nichts bewegt");

        // In drei Scheiben, mit den richtigen Versaetzen.
        let mut geteilt = anfang.clone();
        let schnitte = [0usize, 17, 40, n];
        for paar in schnitte.windows(2) {
            let (a, b) = (paar[0], paar[1]);
            schritt(
                &mut geteilt[a..b],
                &grad[a..b],
                Schrittkennung { index_versatz: a as u64, ..kn },
                1,
                NENNER,
            );
        }
        assert_eq!(ganz, geteilt, "die Aufteilung hat das Ergebnis veraendert");
    }

    /// ⚑ **Die Gegenprobe: der falsche Versatz faellt auf.** Ein Shard,
    /// der seinen Versatz bei null anfangen liesse, bekaeme ein anderes
    /// Ergebnis. Der Test haelt fest, dass die Zusicherung oben etwas
    /// zusichert.
    #[test]
    fn ein_falscher_versatz_ergibt_ein_anderes_delta() {
        let (anfang, grad, kn) = aufbau();
        let n = anfang.len();

        let mut ganz = anfang.clone();
        schritt(&mut ganz, &grad, kn, 1, NENNER);

        let mut falsch = anfang.clone();
        let schnitte = [0usize, 17, 40, n];
        for paar in schnitte.windows(2) {
            let (a, b) = (paar[0], paar[1]);
            // Jede Scheibe faengt faelschlich bei null an.
            schritt(&mut falsch[a..b], &grad[a..b], kn, 1, NENNER);
        }
        assert_ne!(
            ganz, falsch,
            "der falsche Versatz aendert nichts; dann prueft der Test darueber nichts"
        );
    }

    /// Und die Ebenennummer wirkt ebenso.
    #[test]
    fn eine_andere_ebenennummer_wuerfelt_anders() {
        let n = 48usize;
        let anfang: Vec<Master> = (0..n).map(|i| (i as Master) * 131).collect();
        let grad: Vec<i32> = vec![3_000; n];
        let mut a = anfang.clone();
        let mut b = anfang.clone();
        schritt(&mut a, &grad, Schrittkennung { ebene: 0, schritt: 1, index_versatz: 0 }, 1, 1 << 14);
        schritt(&mut b, &grad, Schrittkennung { ebene: 1, schritt: 1, index_versatz: 0 }, 1, 1 << 14);
        assert_ne!(a, b, "die Ebenennummer geht nicht in den Wuerfel ein");
    }
}

#[cfg(test)]
mod grenze_tests {
    use super::*;

    /// Innerhalb der Form meldet die Prüfung nichts.
    #[test]
    fn was_hineinpasst_wird_nicht_gemeldet() {
        let m = vec![0, 1, -1, MASTER_GRENZE as Master, -(MASTER_GRENZE as Master)];
        assert_eq!(ausserhalb_der_form(&m), None);
    }

    /// ⚑ **Die Oktave, die der naheliegenden Zahl fehlt.** `127 << 20`
    /// ist nicht die Grenze der Form, sondern der Anfang ihrer letzten
    /// Oktave. Diese Werte gehen alle noch durch.
    #[test]
    fn die_letzte_oktave_gehoert_noch_dazu() {
        for v in [127i64 << MASTER_FRAC, (127i64 << MASTER_FRAC) + 1, MASTER_GRENZE] {
            assert_eq!(
                ausserhalb_der_form(&[v as Master]),
                None,
                "{v} liegt in der letzten Oktave und ist darstellbar"
            );
        }
    }

    /// Eins darüber wird gemeldet, mit dem Index.
    #[test]
    fn eins_darueber_wird_gemeldet() {
        let ueber = (MASTER_GRENZE + 1) as Master;
        assert_eq!(ausserhalb_der_form(&[0, 0, ueber, 0]), Some(2));
        assert_eq!(ausserhalb_der_form(&[0, -ueber]), Some(1));
    }

    /// ⚑ **Der Extremwert, an dem `abs` überliefe.** `schritt` klemmt
    /// auf `Master::MIN`, also ist dieser Wert erreichbar, und eine
    /// Prüfung, die dort paniked, ist an der einen Stelle blind, an der
    /// sie gebraucht wird.
    #[test]
    fn der_kleinste_master_laesst_die_pruefung_nicht_ueberlaufen() {
        assert_eq!(ausserhalb_der_form(&[Master::MIN]), Some(0));
        assert_eq!(ausserhalb_der_form(&[Master::MAX]), Some(0));
    }

    /// ⚑ **Die Schranke ist genau die, an der `gewicht_aus_master`
    /// aufgibt**, und das ist der Sinn der Sache: Was diese Prüfung
    /// durchlässt, muss dort ankommen.
    #[test]
    fn die_schranke_deckt_sich_mit_der_uebertragungsform() {
        let n = 8usize;
        let gerade_noch = vec![MASTER_GRENZE as Master; n];
        assert_eq!(ausserhalb_der_form(&gerade_noch), None);
        let (w, shifts) = crate::trainingsschritt::gewicht_aus_master(&gerade_noch, n, MASTER_FRAC);
        assert_eq!(w.len(), n, "die Form traegt den Grenzwert");
        assert_eq!(shifts, vec![0u8]);

        // Und eins darüber gibt `gewicht_aus_master` auf.
        let zu_gross = vec![(MASTER_GRENZE + 1) as Master; n];
        assert!(ausserhalb_der_form(&zu_gross).is_some());
        assert!(
            std::panic::catch_unwind(|| {
                crate::trainingsschritt::gewicht_aus_master(&zu_gross, n, MASTER_FRAC)
            })
            .is_err(),
            "was diese Pruefung meldet, muss die Uebertragungsform ablehnen"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Die tragende Eigenschaft, und sie ist der Grund, warum
    /// stochastisches Runden überhaupt hilft:** Im Mittel trifft es den
    /// wahren Wert, während Rundung zur nächsten Stufe ihn systematisch
    /// verfehlt.
    ///
    /// Geprüft an einer Bewegung von einem Viertel einer Rasterstufe.
    /// Zur nächsten Stufe gerundet wäre sie **immer null**; stochastisch
    /// gerundet ist sie in einem Viertel der Fälle eins.
    #[test]
    fn stochastisches_runden_trifft_im_mittel_den_wahren_wert() {
        let stufe = 1i64 << FEIN_BITS;
        let viertel = stufe / 4;

        let mut summe = 0i64;
        let n = 20_000u64;
        for i in 0..n {
            summe += runde_stochastisch(viertel, wuerfel(0, 0, i));
        }
        // Erwartung: 0,25 · n. Zugelassen sind zwei Prozent Abweichung.
        let erwartet = (n / 4) as i64;
        let abstand = (summe - erwartet).abs();
        assert!(
            abstand * 50 < erwartet,
            "Summe {summe}, erwartet {erwartet}, Abstand {abstand}"
        );

        // Die Gegenprobe: Rundung zur nächsten Stufe ergibt hier
        // durchgehend null, und genau das ist der gemessene Schaden.
        let zur_naechsten = |fein: i64| (fein + (stufe / 2)) / stufe;
        assert_eq!(zur_naechsten(viertel), 0);
    }

    /// ⚑ Und dasselbe nach unten. Bei negativen Werten rundet die
    /// ganzzahlige Division in Rust zur Null hin; wer den Rest dann
    /// positiv behandelt, bekommt einen **systematischen Drift nach
    /// oben**, der erst nach Tausenden Schritten als „das Modell lernt
    /// nicht" auffiele.
    #[test]
    fn auch_nach_unten_wird_unverzerrt_gerundet() {
        let stufe = 1i64 << FEIN_BITS;
        let minus_viertel = -stufe / 4;

        let mut summe = 0i64;
        let n = 20_000u64;
        for i in 0..n {
            summe += runde_stochastisch(minus_viertel, wuerfel(7, 3, i));
        }
        let erwartet = -((n / 4) as i64);
        let abstand = (summe - erwartet).abs();
        assert!(
            abstand * 50 < erwartet.abs(),
            "Summe {summe}, erwartet {erwartet}, Abstand {abstand}"
        );
    }

    /// Ganze Stufen gehen unverändert durch, mit und ohne Rest.
    #[test]
    fn ganze_stufen_bleiben_ganze_stufen() {
        let stufe = 1i64 << FEIN_BITS;
        for wurf in [0u64, u64::MAX, 12345] {
            assert_eq!(runde_stochastisch(3 * stufe, wurf), 3);
            assert_eq!(runde_stochastisch(-3 * stufe, wurf), -3);
            assert_eq!(runde_stochastisch(0, wurf), 0);
        }
    }

    /// ⚑ **Der Kern der Sache: Der Schritt ist ordnungsfrei.** Wer die
    /// Indizes in anderer Reihenfolge durchläuft, bekommt dieselben
    /// Gewichte. Mit einem PRNG-Zustand wäre das falsch, und zwei
    /// ehrliche Miner mit verschiedener Aufteilung kämen zu
    /// verschiedenen Ergebnissen.
    #[test]
    fn der_schritt_haengt_nicht_am_zuschnitt() {
        let grad: Vec<Grad> = (0..64).map(|i| (i * 37 % 101) - 50).collect();
        let start: Vec<Master> = (0..64).map(|i| i * 3).collect();
        let k = |versatz| Schrittkennung { ebene: 5, schritt: 42, index_versatz: versatz };

        // In einem Zug.
        let mut ganz = start.clone();
        schritt(&mut ganz, &grad, k(0), 1, 3);

        // In vier Stücken, in umgekehrter Reihenfolge, über **dieselbe
        // Schnittstelle**. Nur der Versatz sagt, wo das Stück steht.
        let mut stueckweise = start.clone();
        for anfang in (0..64).step_by(16).rev() {
            schritt(
                &mut stueckweise[anfang..anfang + 16],
                &grad[anfang..anfang + 16],
                k(anfang as u64),
                1,
                3,
            );
        }
        assert_eq!(ganz, stueckweise);

        // ⚑ Und die Gegenprobe: Ohne den Versatz wäre es falsch. Jedes
        // Stück bekäme die Würfe des ersten, und das Ergebnis wiche ab.
        let mut ohne_versatz = start.clone();
        for anfang in (0..64).step_by(16) {
            schritt(
                &mut ohne_versatz[anfang..anfang + 16],
                &grad[anfang..anfang + 16],
                k(0),
                1,
                3,
            );
        }
        assert_ne!(ganz, ohne_versatz, "ohne Versatz muesste es abweichen");
    }

    /// Zwei Läufe mit denselben Zahlen sind bitgleich, und drei
    /// verschiedene Ebenen oder Schritte sind es nicht.
    #[test]
    fn derselbe_wurf_bei_denselben_zahlen_und_sonst_nicht() {
        assert_eq!(wuerfel(3, 9, 17), wuerfel(3, 9, 17));
        assert_ne!(wuerfel(3, 9, 17), wuerfel(4, 9, 17));
        assert_ne!(wuerfel(3, 9, 17), wuerfel(3, 10, 17));
        assert_ne!(wuerfel(3, 9, 17), wuerfel(3, 9, 18));
        // ⚑ Und die Vertauschung ergibt nicht denselben Wurf: Addition
        // hätte (1,2) und (2,1) zusammenfallen lassen.
        assert_ne!(wuerfel(1, 2, 3), wuerfel(2, 1, 3));
        assert_ne!(wuerfel(1, 2, 3), wuerfel(3, 2, 1));
    }

    /// ⚑ Ein Gewicht am Rand läuft nicht um. Ein umlaufendes Gewicht
    /// wäre ein Vorzeichenwechsel aus dem Nichts.
    #[test]
    fn am_rand_wird_gesaettigt_und_nicht_umgelaufen() {
        let mut oben = vec![Master::MAX];
        schritt(&mut oben, &[-1_000_000], Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 }, 1_000, 1);
        assert_eq!(oben[0], Master::MAX);

        let mut unten = vec![Master::MIN];
        schritt(&mut unten, &[1_000_000], Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 }, 1_000, 1);
        assert_eq!(unten[0], Master::MIN);
    }

    /// Ein Gradient von null bewegt nichts, gleich wie der Würfel fällt.
    #[test]
    fn ohne_gradient_keine_bewegung() {
        let mut w = vec![7, -7, 0, 12345];
        let vorher = w.clone();
        schritt(&mut w, &[0, 0, 0, 0], Schrittkennung { ebene: 2, schritt: 99, index_versatz: 0 }, 5, 1);
        assert_eq!(w, vorher);
    }

    /// Die Bewegung zeigt der Steigung entgegen, wie ein Abstieg es
    /// verlangt.
    #[test]
    fn der_schritt_geht_bergab() {
        let stufe = 1i64 << FEIN_BITS;
        // Ein Gradient, der ganze Stufen ergibt: kein Würfel im Spiel.
        let g = (stufe / (1i64 << FEIN_BITS)) as Grad * 4;
        let mut w = vec![100 as Master];
        schritt(&mut w, &[g], Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 }, 1, 1);
        assert!(w[0] < 100, "positiver Gradient muss das Gewicht senken");

        let mut w2 = vec![100 as Master];
        schritt(&mut w2, &[-g], Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 }, 1, 1);
        assert!(w2[0] > 100, "negativer Gradient muss es heben");
    }
}

/// Der **Trainingsabdruck**: der Wert, den zwei Maschinen vergleichen.
///
/// # ⚑ Wozu es ihn gibt
///
/// Der Konformitätslauf belegt Bitgleichheit gegen **feste**
/// Sollvektoren: Zwei Maschinen, die dasselbe Soll treffen, rechnen
/// gleich. Für einen Trainingsschritt auf einem echten Modell gibt es
/// kein solches Soll, und es kann keins geben, denn die Sollwerte wären
/// die Antwort auf genau die Frage, die geprüft werden soll.
///
/// Also vergleichen die Maschinen **ihre Ergebnisse miteinander**. Der
/// Abdruck ist die Zahl, die dabei nebeneinandergelegt wird.
///
/// # ⚑ Warum die Länge vor jeder Matrix steht
///
/// Ohne sie wären die Bytes zweier Matrizen von 100 und 200 Gewichten
/// nicht von denen dreier Matrizen mit 100, 100 und 100 zu
/// unterscheiden: dieselbe Bytefolge, derselbe Abdruck. Dieselbe
/// Überlegung wie bei den Trennzeichen in `digest_ueber` des
/// Testclients; hier steht sie als Längenangabe da, weil Zahlen jeden
/// Trenner enthalten können.
///
/// ⚑ **Und die Reihenfolge der Matrizen geht ein.** Sie ist keine
/// Eigenschaft des Dateisystems, sondern der Rechenvorschrift: Q, K, V,
/// O, Gate, Up, Down. Wer sie anders reiht, rechnet etwas anderes.
///
/// # ⚑ Was der Abdruck nicht ist
///
/// **Kein Beleg für Richtigkeit.** Zwei Maschinen, die denselben Fehler
/// machen, bekommen denselben Abdruck. Er belegt Übereinstimmung, und
/// das ist genau die Frage, an der die Kernthese hängt: Wer verifizieren
/// will, muss nachrechnen können, und nachrechnen heisst dasselbe
/// herausbekommen.
pub fn trainingsabdruck(matrizen: &[&[Master]]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for m in matrizen {
        h.update((m.len() as u64).to_le_bytes());
        for w in *m {
            h.update(w.to_le_bytes());
        }
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod abdruck_tests {
    use super::*;

    #[test]
    fn derselbe_zustand_ergibt_denselben_abdruck() {
        let a = vec![1i32, -2, 3];
        let b = vec![4i32, 5];
        assert_eq!(
            trainingsabdruck(&[&a, &b]),
            trainingsabdruck(&[&a, &b])
        );
    }

    /// ⚑ **Ein einziges Gewicht reicht.** Ein Abdruck, der eine
    /// Abweichung in der letzten Stelle verschluckte, belegte nichts.
    #[test]
    fn ein_einziges_gewicht_aendert_den_abdruck() {
        let a = vec![1i32, -2, 3];
        let mut c = a.clone();
        c[2] += 1;
        assert_ne!(trainingsabdruck(&[&a]), trainingsabdruck(&[&c]));
    }

    /// ⚑ **Die Gegenprobe zur Längenangabe.** Ohne sie hätten diese
    /// beiden Aufteilungen denselben Abdruck.
    #[test]
    fn dieselben_zahlen_anders_aufgeteilt_sind_ein_anderer_abdruck() {
        let alle = [1i32, 2, 3, 4, 5, 6];
        let links = trainingsabdruck(&[&alle[..2], &alle[2..]]);
        let rechts = trainingsabdruck(&[&alle[..4], &alle[4..]]);
        assert_ne!(links, rechts);
    }

    /// ⚑ **Und die Gegenprobe zur Reihenfolge.**
    #[test]
    fn die_reihenfolge_der_matrizen_geht_ein() {
        let a = vec![1i32, 2];
        let b = vec![3i32, 4];
        assert_ne!(trainingsabdruck(&[&a, &b]), trainingsabdruck(&[&b, &a]));
    }

    #[test]
    fn der_abdruck_ist_ein_voller_sha256() {
        let a = vec![0i32];
        let d = trainingsabdruck(&[&a]);
        assert_eq!(d.len(), 64, "{d}");
        assert!(d.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

/// Die **Aggregation vieler Miner** (TRAINING 2.2):
///
/// ```text
/// m_{v+1} = klemmen(m_v + Σ Δm_i)
/// ```
///
/// # ⚑ Ordnungsfrei, und das ist kein Nebeneffekt
///
/// Viele Miner rechnen verschiedene Chargen gegen **denselben**
/// Ausgangszustand, und ihre Ergebnisse addieren sich. In welcher
/// Reihenfolge die Beiträge eintreffen, entscheidet das Netz, also
/// niemand: Pakete überholen sich, ein Knoten sammelt anders als der
/// nächste. **Hinge das Ergebnis daran, hinge der Kettenzustand am
/// Zufall der Zustellung.**
///
/// Eine Summe ganzer Zahlen ist von Natur aus ordnungsfrei. Was sie
/// verlieren kann, ist genau eines: die **Sättigung**.
///
/// # ⚑ Geklemmt wird genau einmal, ganz am Ende
///
/// Wer je Summand klemmt, bekommt ein Ergebnis, das von der Reihenfolge
/// abhängt. Der Fall ist keine Spitzfindigkeit, er ist einzeilig
/// hinzuschreiben: Mit Obergrenze 100 und Beiträgen `+80, +80, −80`
/// ergibt
///
/// | Reihenfolge | je Summand geklemmt | einmal am Ende |
/// |---|---|---|
/// | `+80, +80, −80` | 100 dann 20 | **80** |
/// | `−80, +80, +80` | −80 dann 0 dann 80 | **80** |
///
/// links zweimal etwas anderes, rechts zweimal dasselbe.
///
/// ⚑ **Dieselbe Regel wie in `mische_experten` und in
/// `linear_backward`** (Fund 24): in `i64` summieren, **einmal**
/// schieben, **einmal** klemmen. Sie steht in diesem Projekt zum
/// dritten Mal, und zum dritten Mal aus demselben Grund.
///
/// # Argumente
///
/// - `master`: der Ausgangszustand `m_v`, wird an Ort und Stelle
///   fortgeschrieben
/// - `deltas`: die Beiträge, je einer je Miner, alle über derselben
///   Gewichtsscheibe
///
/// # ⚑ Panik statt stiller Kürzung
///
/// Ein Delta anderer Länge ist ein Aufrufer-Fehler, kein Randfall. Wer
/// es stillschweigend anschnitte, aggregierte einen Teil und zahlte für
/// das Ganze.
pub fn aggregiere(master: &mut [Master], deltas: &[&[Master]]) {
    for (n, d) in deltas.iter().enumerate() {
        assert_eq!(
            d.len(),
            master.len(),
            "aggregiere: Beitrag {n} hat {} Gewichte, der Master {}",
            d.len(),
            master.len()
        );
    }
    for (i, m) in master.iter_mut().enumerate() {
        // ⚑ **Die Summe in i64.** Bei 10 Millionen Beiträgen zu je
        // 2^31 wäre ein i32-Akkumulator nach dem zweiten voll; die
        // Bitbudget-Rechnung vom 2026-08-22 sagt dasselbe für den
        // Aggregationsakkumulator wie für den Master.
        let mut summe = i64::from(*m);
        for d in deltas {
            summe += i64::from(d[i]);
        }
        *m = summe.clamp(i64::from(Master::MIN), i64::from(Master::MAX)) as Master;
    }
}

/// Das **Δm** eines Segments: was ein Schritt an den Gewichten geändert
/// hat.
///
/// ⚑ **Die Differenz und nicht der Endzustand.** Ein Endzustand addiert
/// sich nicht: Zwei Miner, die verschiedene Chargen gegen denselben
/// Ausgangszustand rechnen, haben zwei verschiedene Endzustände, und
/// keiner davon ist der, den die Kette am Ende trägt. Siehe
/// [`aggregiere`] und `myl_types::trainingssegment`.
///
/// **Panik statt stiller Kürzung**, aus demselben Grund wie dort.
pub fn delta(vorher: &[Master], nachher: &[Master]) -> Vec<Master> {
    assert_eq!(
        vorher.len(),
        nachher.len(),
        "delta: {} gegen {} Gewichte",
        vorher.len(),
        nachher.len()
    );
    vorher
        .iter()
        .zip(nachher.iter())
        .map(|(a, b)| {
            // ⚑ **Auch hier geklemmt und nicht umlaufend.** Die
            // Differenz zweier i32 passt nicht immer in i32; ein Umlauf
            // machte aus einem grossen Schritt einen grossen Schritt in
            // die Gegenrichtung.
            (i64::from(*b) - i64::from(*a))
                .clamp(i64::from(Master::MIN), i64::from(Master::MAX)) as Master
        })
        .collect()
}

/// Eine **dünn besetzte** Scheibe eines Beitrags: wo sie im Master
/// beginnt, und was sie ändert.
pub type Scheibe<'a> = (usize, &'a [Master]);

/// Die Aggregation über **dünn besetzte** Beiträge (TRAINING Phase 5).
///
/// # ⚑ Warum ein Expertengemisch einen eigenen Weg braucht
///
/// Bei einem Gemisch berührt ein Miner nur die Experten, die sein Router
/// gewählt hat. **Gemessen am 2026-09-05 an Qwen3-30B-A3B**, 24 Schritte
/// auf einer Ebene: **18 von 128 Experten**, mit Lastausgleich, und 10
/// ohne. Ein dichtes Δm über alle wäre zu 86 Prozent null.
///
/// Bei 128 Experten je Ebene und 4,7 Millionen Gewichten je Experte sind
/// das **560 Millionen Nullen** je Ebene und Beitrag, übertragen und
/// summiert. ⚑ **Das ist nicht Sparsamkeit, sondern der Kern der
/// Sache:** Ein Expertengemisch ist genau deshalb billig, weil ein Token
/// nur k Experten berührt; eine Aggregation, die alle anfasst, hat den
/// Vorteil weggeworfen.
///
/// # ⚑ Ordnungsfrei, und geklemmt genau einmal
///
/// Dieselbe Regel wie in [`aggregiere`], mit einem Zusatz: Zwei Beiträge
/// dürfen **dieselbe** Stelle treffen, denn zwei Miner können denselben
/// Experten gewählt haben. Deshalb wird erst über alle Beiträge
/// aufsummiert und **danach** geklemmt, nicht je Scheibe.
///
/// ⚑ **`BTreeMap` und keine Hashtabelle.** Der Sammelplatz wird geordnet
/// durchlaufen; eine Hashtabelle gäbe dabei eine Reihenfolge, die an
/// Adressen hängt. Auf das Ergebnis wirkt sich das hier nicht aus, weil
/// jede Stelle einmal geschrieben wird, **aber es wäre eine Reihenfolge
/// im Konsenspfad, die niemand festgelegt hat**, und die nächste
/// Änderung machte sie sichtbar.
pub fn aggregiere_duenn(master: &mut [Master], beitraege: &[&[Scheibe<'_>]]) {
    use std::collections::BTreeMap;
    let mut summen: BTreeMap<usize, i64> = BTreeMap::new();
    for (n, beitrag) in beitraege.iter().enumerate() {
        for (versatz, scheibe) in beitrag.iter() {
            assert!(
                versatz + scheibe.len() <= master.len(),
                "aggregiere_duenn: Beitrag {n} reicht von {versatz} ueber {} hinaus,                  der Master hat {}",
                scheibe.len(),
                master.len()
            );
            for (i, d) in scheibe.iter().enumerate() {
                *summen.entry(versatz + i).or_insert(0) += i64::from(*d);
            }
        }
    }
    for (i, s) in summen {
        let summe = i64::from(master[i]) + s;
        master[i] = summe.clamp(i64::from(Master::MIN), i64::from(Master::MAX)) as Master;
    }
}

#[cfg(test)]
mod aggregation_tests {
    use super::*;

    /// ⚑ **Die Aussage, um die es geht: die Reihenfolge ändert nichts.**
    #[test]
    fn die_reihenfolge_der_beitraege_aendert_das_ergebnis_nicht() {
        let start = vec![100i32, -50, 0, 7];
        let a = vec![10i32, 20, -30, 1];
        let b = vec![-5i32, 5, 5, -1];
        let c = vec![1000i32, -1000, 1, 0];

        let mut links = start.clone();
        aggregiere(&mut links, &[&a, &b, &c]);
        let mut rechts = start.clone();
        aggregiere(&mut rechts, &[&c, &a, &b]);
        let mut mitte = start.clone();
        aggregiere(&mut mitte, &[&b, &c, &a]);

        assert_eq!(links, rechts);
        assert_eq!(links, mitte);
        assert_eq!(links, vec![1105, -1025, -24, 7]);
    }

    /// ⚑ **Auch an der Sättigung**, und das ist der einzige Ort, wo eine
    /// ganzzahlige Summe die Ordnungsfreiheit verlieren kann.
    #[test]
    fn die_reihenfolge_aendert_auch_an_der_saettigung_nichts() {
        let gross = Master::MAX - 10;
        let start = vec![gross];
        let hoch = vec![100i32];
        let runter = vec![-100i32];

        let mut a = start.clone();
        aggregiere(&mut a, &[&hoch, &runter]);
        let mut b = start.clone();
        aggregiere(&mut b, &[&runter, &hoch]);

        assert_eq!(a, b, "die Reihenfolge hat an der Saettigung entschieden");
        assert_eq!(a, vec![gross], "die Summe ist null, der Wert bleibt");
    }

    /// ⛑ **Die Gegenprobe: so sähe es aus, wenn je Summand geklemmt
    /// würde.**
    ///
    /// Ohne diesen Test bliebe offen, ob der obige überhaupt etwas
    /// auswählt: Vielleicht klemmt gar nichts, und dann prüft er die
    /// Reihenfolge einer Summe, die ohnehin nie an die Grenze kommt.
    #[test]
    fn je_summand_geklemmt_haenge_das_ergebnis_an_der_reihenfolge() {
        let gross = i64::from(Master::MAX) - 10;
        let klemmen = |v: i64| v.clamp(i64::from(Master::MIN), i64::from(Master::MAX));
        let je_summand = |folge: &[i64]| -> i64 {
            let mut m = gross;
            for d in folge {
                m = klemmen(m + d);
            }
            m
        };
        assert_ne!(
            je_summand(&[100, -100]),
            je_summand(&[-100, 100]),
            "der Aufbau erreicht die Saettigung gar nicht, dann prueft der Test daneben nichts"
        );
    }

    /// ⚑ **Ohne Beiträge bleibt der Master, wie er war.**
    #[test]
    fn ohne_beitraege_aendert_sich_nichts() {
        let mut m = vec![1i32, -2, 3];
        aggregiere(&mut m, &[]);
        assert_eq!(m, vec![1, -2, 3]);
    }

    /// ⚑ **Ein Beitrag falscher Länge bricht ab.**
    #[test]
    #[should_panic(expected = "Beitrag 1 hat")]
    fn ein_beitrag_falscher_laenge_bricht_ab() {
        let mut m = vec![1i32, 2, 3];
        let gut = vec![1i32, 1, 1];
        let kurz = vec![1i32];
        aggregiere(&mut m, &[&gut, &kurz]);
    }

    /// ⚑ **Δm ist die Differenz, und sie führt zurück.**
    #[test]
    fn das_delta_fuehrt_zum_endzustand_zurueck() {
        let vorher = vec![100i32, -50, 0];
        let nachher = vec![103i32, -49, -7];
        let d = delta(&vorher, &nachher);
        assert_eq!(d, vec![3, 1, -7]);
        let mut zurueck = vorher.clone();
        aggregiere(&mut zurueck, &[&d]);
        assert_eq!(zurueck, nachher);
    }

    /// ⚑ **Zwei Deltas gegen denselben Ausgangszustand addieren sich.**
    ///
    /// Das ist die Eigenschaft, wegen der das Commitment über Δm geht
    /// und nicht über den Endzustand: Endzustände addieren sich nicht.
    #[test]
    fn zwei_deltas_gegen_denselben_start_addieren_sich() {
        let start = vec![100i32, 100];
        let ende_a = vec![110i32, 90];
        let ende_b = vec![95i32, 120];
        let da = delta(&start, &ende_a);
        let db = delta(&start, &ende_b);

        let mut zusammen = start.clone();
        aggregiere(&mut zusammen, &[&da, &db]);
        assert_eq!(zusammen, vec![105, 110]);

        // ⛑ Und die Gegenprobe: die Endzustaende zu addieren gaebe
        // etwas anderes, naemlich den doppelten Ausgangszustand mit.
        let mut falsch = vec![0i32, 0];
        aggregiere(&mut falsch, &[&ende_a, &ende_b]);
        assert_ne!(falsch, zusammen, "Endzustaende addieren sich doch?");
    }

    /// ⚑ **Die Differenz klemmt, statt umzulaufen.**
    #[test]
    fn eine_grosse_differenz_klemmt() {
        let d = delta(&[Master::MIN], &[Master::MAX]);
        assert_eq!(d, vec![Master::MAX], "die Differenz ist umgelaufen");
    }
}

#[cfg(test)]
mod duenne_aggregation_tests {
    use super::*;

    /// ⚑ **Dünn und dicht liefern dasselbe.**
    ///
    /// Die Aussage, an der alles hängt: Der dünne Weg ist eine
    /// Abkürzung, keine zweite Rechnung.
    #[test]
    fn duenn_und_dicht_liefern_dasselbe() {
        let start = vec![10i32; 12];
        // Zwei Beitraege, die je zwei Scheiben treffen und sich an einer
        // Stelle ueberlappen.
        let a1 = [1i32, 2];
        let a2 = [5i32, 5, 5];
        let b1 = [-3i32];
        let b2 = [7i32, 7];

        let mut duenn = start.clone();
        aggregiere_duenn(
            &mut duenn,
            &[&[(0usize, &a1[..]), (4, &a2[..])], &[(1usize, &b1[..]), (4, &b2[..])]],
        );

        // Dasselbe von Hand als dichte Beitraege.
        let mut da = vec![0i32; 12];
        da[0] = 1;
        da[1] = 2;
        da[4] = 5;
        da[5] = 5;
        da[6] = 5;
        let mut db = vec![0i32; 12];
        db[1] = -3;
        db[4] = 7;
        db[5] = 7;
        let mut dicht = start.clone();
        aggregiere(&mut dicht, &[&da, &db]);

        assert_eq!(duenn, dicht);
    }

    /// ⚑ **Die Reihenfolge der Beiträge ändert nichts**, auch wenn sie
    /// sich überlappen.
    #[test]
    fn die_reihenfolge_der_duennen_beitraege_aendert_nichts() {
        let start = vec![100i32; 6];
        let a = [10i32, 20];
        let b = [-5i32, -5];
        let mut links = start.clone();
        aggregiere_duenn(&mut links, &[&[(2usize, &a[..])], &[(2usize, &b[..])]]);
        let mut rechts = start.clone();
        aggregiere_duenn(&mut rechts, &[&[(2usize, &b[..])], &[(2usize, &a[..])]]);
        assert_eq!(links, rechts);
        assert_eq!(links[2], 105);
    }

    /// ⚑ **Zwei Beiträge auf dieselbe Stelle addieren sich**, statt dass
    /// einer den anderen überschreibt.
    ///
    /// Zwei Miner können denselben Experten gewählt haben; wer hier
    /// überschriebe, verwürfe die halbe Arbeit.
    #[test]
    fn zwei_beitraege_auf_dieselbe_stelle_addieren_sich() {
        let mut m = vec![0i32; 3];
        let a = [7i32];
        let b = [5i32];
        aggregiere_duenn(&mut m, &[&[(1usize, &a[..])], &[(1usize, &b[..])]]);
        assert_eq!(m, vec![0, 12, 0], "einer hat den anderen ueberschrieben");
    }

    /// ⚑ **Geklemmt genau einmal**, auch dünn.
    #[test]
    fn auch_duenn_wird_genau_einmal_geklemmt() {
        let gross = Master::MAX - 10;
        let hoch = [100i32];
        let runter = [-100i32];
        let mut a = vec![gross];
        aggregiere_duenn(&mut a, &[&[(0usize, &hoch[..])], &[(0usize, &runter[..])]]);
        let mut b = vec![gross];
        aggregiere_duenn(&mut b, &[&[(0usize, &runter[..])], &[(0usize, &hoch[..])]]);
        assert_eq!(a, b);
        assert_eq!(a, vec![gross]);
    }

    /// ⚑ **Unberührte Stellen bleiben unberührt.**
    #[test]
    fn was_niemand_beruehrt_bleibt_stehen() {
        let mut m = vec![1i32, 2, 3, 4, 5];
        let d = [10i32];
        aggregiere_duenn(&mut m, &[&[(2usize, &d[..])]]);
        assert_eq!(m, vec![1, 2, 13, 4, 5]);
    }

    /// ⚑ **Eine Scheibe über den Rand hinaus bricht ab.**
    #[test]
    #[should_panic(expected = "reicht von 3 ueber 4 hinaus")]
    fn eine_scheibe_ueber_den_rand_bricht_ab() {
        let mut m = vec![0i32; 5];
        let d = [1i32, 1, 1, 1];
        aggregiere_duenn(&mut m, &[&[(3usize, &d[..])]]);
    }
}
