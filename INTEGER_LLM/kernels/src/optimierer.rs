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
