//! Der Satz, zu dem Speicher gegen Rechenarbeit getauscht wird
//! (Punkt B4, entschieden am 2026-09-02).
//!
//! # Die Frage, und warum sie ein Verhältnis ist
//!
//! Wer eine MYL verbrennt, bekommt dafür Recheneinheiten; der Preis
//! dafür ist **dynamisch** (`P_{e+1} = P_e · exp(κ·(u−u*))`, Kap. 5.4).
//! Wer Speicher kaufen will, braucht denselben Weg, und ein **fester**
//! Speicherpreis daneben liefe dem Rechenpreis davon: Steigt der
//! Rechenpreis um das Dreifache und der Speicherpreis nicht, ist
//! Speicher plötzlich zu zwei Dritteln subventioniert, **ohne dass
//! jemand etwas entschieden hätte**.
//!
//! Deshalb ist der Satz ein **Verhältnis**: wie viele Recheneinheiten
//! eine Byte-Epoche kostet. Ein Verhältnis kann nicht auseinanderlaufen.
//!
//! # ⚑ Die Herleitung, und die beiden Fehler, die sie am 2026-09-02 hatte
//!
//! Gerechnet wird in `speicherentgelt_sim.py`: reale Kosten einer
//! Byte-Epoche gegen reale Kosten einer ganzzahligen Operation, beide
//! aus **eigener Hardware** und nicht aus einem Mietpreis, denn ein
//! Halter kauft keine Wolke.
//!
//! **Erster Fehler: die Platte war zu billig angesetzt.** 250 für 20 TB
//! sind 12,50 je TB; der Straßenpreis für neue Enterprise-CMR-Platten
//! lag im September 2026 bei **22 bis 25 je TB**. Die Preise waren seit
//! September 2025 um rund die Hälfte gestiegen, weil der Aufbau von
//! KI-Infrastruktur die hohen Kapazitäten aufkauft.
//!
//! ⚑ **Zweiter Fehler, und der größere: niemand betreibt eine nackte
//! Platte.** Das Modell rechnete Anschaffung und Strom des Laufwerks
//! und sonst nichts. Eine Platte braucht ein Gehäuse, einen Rechner,
//! Speicher und einen Netzanschluss, und der Wirt läuft mit, ob eine
//! Platte darin steckt oder zwölf.
//!
//! Beides zusammen hebt die Kosten von 1 420 auf **3 586**
//! Recheneinheiten je Byte-Epoche, also um das Zweieinhalbfache.
//!
//! # ⚑ Strom steht drin und fällt kaum ins Gewicht
//!
//! Er macht **12 Prozent** der Plattenkosten aus und **13 Prozent** der
//! Kartenkosten. Weil er auf beiden Seiten fast gleich anteilig steckt,
//! kürzt er sich im Verhältnis heraus: Von fünf Cent bis sechzig Cent je
//! Kilowattstunde bewegt sich der Satz um **unter ein Prozent**. Für die
//! absolute Rentabilität eines Halters zählt der Strompreis sehr wohl,
//! für die Umrechnung zwischen Speichern und Rechnen fast nicht.
//!
//! # Warum der Satz über den Kosten liegt
//!
//! **Kosten zu decken ist kein Anreiz.** Wer zum Selbstkostenpreis
//! hält, verdient nichts und hält deshalb nicht.
//!
//! ⚑ **Der Anker ist Storj**, und er ist der einzige direkt
//! vergleichbare: ein dezentrales Netz, das Privatleute mit **eigener**
//! Hardware bezahlt, und zwar **1,50 je TB-Monat**, gerechnet auf die
//! erasure-kodierten Bytes. Dieselbe Konvention gilt hier: Die
//! Redundanz steckt in der Bytezahl (`Manifest::redundanz`) und nicht
//! im Satz.
//!
//! Bei den hergeleiteten Kosten von 0,60 je TB-Monat sind 1,50 das
//! **2,5-Fache**, und daraus folgt der Satz von 9 000.
//!
//! ⚑ **Was ausdrücklich nicht der Anker ist: ein Endkundenpreis.**
//! Backblaze B2 nimmt 6,95 je TB-Monat, AWS S3 nimmt 23. Darin stecken
//! Rechenzentrum, Personal, Bandbreite, Verfügbarkeitszusage und Marge.
//! **Ein Halter in diesem Netz liefert davon nichts**, und die
//! Redundanz rechnet das Protokoll ohnehin getrennt. Ein Satz von
//! 42 000 wäre eine Überzahlung um das Zwölffache.
//!
//! **Die ehrliche Einschränkung:** Storj steht unter Druck, diese Rate
//! zu senken. 1,50 ist eher das obere Ende des Tragfähigen als die
//! Mitte, und der Satz ist deshalb ein Governance-Parameter und keine
//! Konstante.

/// Was eine Byte-Epoche an **reinen Kosten** entspricht, für einen
/// effizienten Halter.
///
/// ⚑ **Der Boden, unter dem niemand mehr halten kann.** Gerechnet mit
/// vierundzwanzig Platten je Wirt, also dem günstigsten der gerechneten
/// Zuschnitte: 2 995, aufgerundet auf 3 000. **Aufgerundet, weil ein
/// Boden eine Untergrenze ist**; abgerundet ließe er einen Satz knapp
/// darunter als auskömmlich durchgehen.
///
/// Ein Satz **darunter** heißt: Auch der effizienteste Halter zahlt
/// drauf, also hält niemand, also gibt es die Rolle Store nicht mehr.
/// Deshalb prüft GOVERNANCE dagegen.
///
/// **Nicht zu verwechseln mit dem Satz selbst.** Der liegt bei
/// [`SPEICHERSATZ_VORGABE`] und damit dreimal so hoch; die Differenz
/// ist der Anreiz.
pub const SPEICHER_KOSTENBODEN: u64 = 3_000;

/// Der Startwert des Speichersatzes: Recheneinheiten je Byte-Epoche.
///
/// **Entschieden am 2026-09-02.** Ergibt 1,49 je TB-Monat und trifft
/// damit die Storj-Rate von 1,50; das ist das 2,5-Fache der
/// hergeleiteten Kosten. Siehe den Modulkopf für die Herleitung und für
/// den Grund, warum kein Endkundenpreis der Anker ist.
///
/// ⚑ **Ein Governance-Parameter, keine Konstante des Genesis.** Die
/// Herleitung hängt an Hardwarepreisen, und die bewegen sich: Die
/// HDD-Preise stiegen 2026 um rund die Hälfte, und für die Zeit nach
/// 2027 wird wieder ein Rückgang erwartet. **Eine Zahl, die sich als
/// falsch herausstellt, muss korrigierbar sein**, und genau das war die
/// Entscheidung zu B4.
pub const SPEICHERSATZ_VORGABE: u64 = 9_000;

/// ⚑ **Der Satz liegt über dem Boden, und zwar schon beim Übersetzen.**
///
/// Eine Zusicherung über zwei Konstanten gehört nicht in einen Test:
/// Ein Test läuft, wenn jemand ihn startet, **eine `const`-Zusicherung
/// hält, sonst gibt es kein Programm.** Wer den Satz unter den Boden
/// setzt, bekommt keinen fehlschlagenden Test, sondern gar keinen Bau.
///
/// Der Abstand ist der Anreiz, und er soll spürbar sein: Ein Aufschlag
/// von zehn Prozent wäre keiner.
const _: () = assert!(
    SPEICHERSATZ_VORGABE >= SPEICHER_KOSTENBODEN * 2,
    "der Speichersatz deckt den Kostenboden nicht mindestens doppelt"
);

/// Wie viele Byte-Epochen man für `recheneinheiten` bekommt.
///
/// **Die Umrechnung in eine Richtung, und die andere gibt es nicht.**
/// Wer Speicher will, bringt Recheneinheiten mit; Byte-Epochen zurück
/// in Recheneinheiten zu tauschen wäre ein zweiter Markt, und den
/// beschreibt das Papier nicht.
///
/// Ganzzahlig und **abgerundet**: Wer nicht für eine ganze Byte-Epoche
/// bezahlt, bekommt sie nicht. Aufrunden hieße, Speicher zu verschenken,
/// und über viele kleine Käufe wäre das ein Weg, ihn umsonst zu
/// bekommen.
pub fn byte_epochen(recheneinheiten: u64, satz: u64) -> u64 {
    if satz == 0 {
        return 0;
    }
    recheneinheiten / satz
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein Satz von null verschenkte Speicher, und das ist genau der
    /// Fall, gegen den die Schranke steht.
    #[test]
    fn ein_satz_von_null_gibt_nichts() {
        assert_eq!(byte_epochen(1_000_000, 0), 0);
    }

    /// Abgerundet, nie aufgerundet.
    #[test]
    fn es_wird_abgerundet() {
        assert_eq!(byte_epochen(SPEICHERSATZ_VORGABE - 1, SPEICHERSATZ_VORGABE), 0);
        assert_eq!(byte_epochen(SPEICHERSATZ_VORGABE, SPEICHERSATZ_VORGABE), 1);
        assert_eq!(byte_epochen(2 * SPEICHERSATZ_VORGABE - 1, SPEICHERSATZ_VORGABE), 1);
    }

    /// ⚑ **Die Zahl, gegen die die Herleitung geprüft wird.**
    ///
    /// 9 000 Recheneinheiten je Byte-Epoche sollen 1,50 je TB-Monat
    /// entsprechen, also der Storj-Rate. Der Test rechnet es nach, mit
    /// denselben Kosten wie die Simulation, damit die Zahl nicht ohne
    /// ihre Bedeutung dasteht.
    #[test]
    fn neuntausend_treffen_die_storj_rate() {
        // Kosten einer Byte-Epoche und einer Operation, in
        // Zehnbillionstel Währungseinheiten, damit ganzzahlig gerechnet
        // werden kann. Aus `speicherentgelt_sim.py`, Abschnitt 1.
        //
        // 1 TB-Monat bei Satz 9 000 kostet so viel wie
        // 9 000 * 1e12 * 730 Recheneinheiten.
        let byte_epochen_je_tb_monat: u128 = 1_000_000_000_000 * 730;
        let recheneinheiten = byte_epochen_je_tb_monat * SPEICHERSATZ_VORGABE as u128;
        // Eine Operation kostet 2,274e-19; in Attoeinheiten (1e-18) ist
        // das 0,2274, also rechnen wir in Zeptoeinheiten (1e-21): 227,4.
        let zepto_je_operation: u128 = 227;
        let zepto = recheneinheiten * zepto_je_operation;
        // In Währungseinheiten: durch 1e21.
        let millicent = zepto / 10_u128.pow(21 - 5); // 1e-5 Einheiten
        assert!(
            (145_000..=155_000).contains(&millicent),
            "Satz {SPEICHERSATZ_VORGABE} ergibt {} je TB-Monat, erwartet rund 1,50",
            millicent as f64 / 100_000.0
        );
    }
}
