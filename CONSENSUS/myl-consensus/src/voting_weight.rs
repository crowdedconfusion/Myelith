//! Stimmgewicht: **Arbeit qualifiziert, Stake wiegt** (Kap. 3.5.2).
//!
//! ```text
//! voting_weight = stake, für jeden qualifizierten Validator
//! ```
//!
//! # ⚑ Die Entscheidung vom 2026-09-02, und warum sie fiel
//!
//! Bis dahin stand hier eine Summenform, die die nachgewiesene
//! Inferenzarbeit **multiplikativ** ins Gewicht nahm, gedeckelt auf
//! `hoechstfaktor` Stakes. Zwei Messungen haben sie erledigt.
//!
//! **Fund 135: Der Deckel griff ab 1,13-fachem Referenzdurchsatz.**
//! Darüber ergaben 1,2-fach und hundertfach denselben Wert. Der
//! Arbeitsanteil unterschied also in einem Band von dreizehn Prozent und
//! war darüber blind, was seine einzige Aufgabe war. Dieselbe Klasse wie
//! Fund 51, eine Ebene höher.
//!
//! ⚑ **Und die Sicherheitsaussage stand daneben, ungesagt:** Ein MYL im
//! gedeckelten Validator wog **zehnmal** so viel wie eines im
//! arbeitslosen. Wer ein Drittel des Stimmgewichts wollte, brauchte am
//! Deckel ein Zehntel des Stakes. **Der Höchstfaktor war keine
//! Begrenzung neben den Angriffskosten, er war ihr Divisor.**
//!
//! **Fund 137: Der Arbeitsanteil wurde nie erreicht.**
//! `ValidatorRegistry::record_work` hatte außerhalb seiner eigenen Tests
//! **keinen einzigen Aufrufer**. Die Historie war im Betrieb immer leer,
//! und damit galt `voting_weight == stake` schon die ganze Zeit. Die
//! Formel beschrieb ein Verhalten, das der Code nie zeigte.
//!
//! ⚑ **Die Umstellung ist deshalb keine Verhaltensänderung, sondern
//! eine Berichtigung des Vertrags.** Sie ist die sechste Ausprägung des
//! häufigsten Fehlerbilds dieses Projekts: beide Seiten gebaut, beide
//! für sich geprüft, die Naht fehlt.
//!
//! # Was die Recherche gesagt hat
//!
//! **Ethereum**, der am stärksten geprüfte Proof-of-Stake-Betrieb, kennt
//! keinen Arbeitsanteil: Das Stimmgewicht **ist** der Stake, gedeckelt
//! über `MAX_EFFECTIVE_BALANCE`, und dieser Deckel diente der
//! Komitee-Dominanz und nicht dem Risiko. Als der Grund entfiel, wurde
//! er mit EIP-7251 um das Vierundsechzigfache gehoben.
//!
//! **Filecoin** kennt denselben Faktor 10 als Qualitätsmultiplikator
//! (`QUALITY_BASE_MULTIPLIER = 10`, `VERIFIED_DEAL_WEIGHT_MULTIPLIER =
//! 100`), verlangt dafür aber **zehnfache Sicherheit und schlachtet
//! zehnfach**. Der Multiplikator verdünnt das Risiko nicht, er skaliert
//! mit ihm.
//!
//! **Bittensor** mischt Stake und Konsensübereinstimmung, und die
//! empirische Auswertung ist unbequem: Die Korrelation Stake zu
//! Belohnung liegt bei 0,80 bis 0,95, die von Leistung zu Belohnung bei
//! rund 0,50. Eine Mischung sieht aus, als belohne sie Qualität, und tut
//! es messbar nicht.
//!
//! **RepuCoin** ist der akademische Fall **für** arbeitsgewichtete
//! Stimmen und zeigt zugleich, woran es hier fehlte: Dort entsteht
//! Reputation aus Arbeit, **integriert über die gesamte
//! Kettengeschichte**, und ist absichtlich träge. Das Fenster hier war
//! zehn Epochen, also zehn Stunden. Die Trägheit, die RepuCoins
//! Sicherheitsaussage trägt, gab es nie.
//!
//! # Was stattdessen gilt
//!
//! **Arbeit ist eine Eignung und keine Menge.** Die Frage, die der
//! Arbeitsanteil beantworten soll, lautet „ist dieser Validator ein
//! echter Miner und kein reiner Kapitalhalter", und das ist eine
//! **Schwelle**. Oberhalb davon sagt mehr Durchsatz nichts mehr über
//! Vertrauenswürdigkeit; die alte Formel verhielt sich ab 1,13-fach
//! ohnehin so, nur unbeabsichtigt.
//!
//! Die Schwelle misst **relativ zum Netz** ([`arbeitsqualifikation`]),
//! nicht gegen einen festen Bezugswert. Damit endet die Klasse Fund 51:
//! Ein relativer Bezug veraltet nicht, wenn sich der Durchsatz
//! verschiebt, weil er mitwandert.
//!
//! ⚑ **Und sie startet bei null.** Bei Genesis hat niemand Arbeit; eine
//! Schwelle über null wäre dieselbe Blockade wie das reine Produkt, gegen
//! das die Summenform gebaut worden war. Null ist der einzige Startwert,
//! der ohne Sonderregel für den Genesis-Satz auskommt. Governance hebt
//! sie, sobald das Netz eine messbare Durchsatzverteilung hat.
//!
//! ⚑ **Solange `record_work` keinen Aufrufer hat, ist die Qualifikation
//! wirkungslos**, und das gehört dazugesagt: Sie beißt erst, wenn
//! bezeugte Arbeit die Validatoren erreicht. Das ist Punkt 40 und nicht
//! dieses Modul.
//!
//! # Was am Papier zu ändern ist
//!
//! Kap. 3.5.2 beschreibt zwei Quellen, die sich multiplizieren. Beide
//! Quellen gelten weiter, ihr Zusammenspiel nicht: Die eine
//! qualifiziert, die andere wiegt. Vermerkt für die nächste Fassung.
//!
//! **Konsens-Feld:** Die Stimmgewichts-Berechnung ist Teil des
//! Konsensvertrags. Änderungen nur über Governance (Kap. 10.3).

/// Abklingfaktor für historische Inferenzarbeit (pro Epoche).
/// 0.95 bedeutet: Arbeit aus der vorherigen Epoche zählt zu 95%.
pub const DECAY_FACTOR_NUM: u64 = 95;
pub const DECAY_FACTOR_DEN: u64 = 100;

/// Maximale Anzahl vergangener Epochen, die berücksichtigt werden.
pub const MAX_HISTORY_EPOCHS: usize = 10;

/// Historische Inferenzarbeit über mehrere Epochen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InferenceHistory {
    /// Arbeit pro Epoche (Epoche → vTFE-Einheiten).
    pub work_per_epoch: Vec<(u64, u64)>,
}

impl InferenceHistory {
    /// Erstellt eine neue, leere Historie.
    pub fn new() -> Self {
        Self {
            work_per_epoch: Vec::new(),
        }
    }

    /// Fügt Arbeit für eine Epoche hinzu.
    pub fn add_work(&mut self, epoch: u64, work: u64) {
        // Füge neuen Eintrag hinzu (oder aktualisiere bestehenden)
        if let Some(entry) = self.work_per_epoch.iter_mut().find(|(e, _)| *e == epoch) {
            entry.1 += work;
        } else {
            self.work_per_epoch.push((epoch, work));
        }

        // Sortiere nach Epoche (absteigend)
        self.work_per_epoch
            .sort_by_key(|(epoch, _)| std::cmp::Reverse(*epoch));

        // Behalte nur die letzten MAX_HISTORY_EPOCHS Einträge
        self.work_per_epoch.truncate(MAX_HISTORY_EPOCHS);
    }

    /// Berechnet das abgeklungene Gesamtgewicht.
    ///
    /// **Algorithmus:**
    /// Für jede Epoche in der Historie:
    ///   weight += work[epoch] × decay^(current_epoch - epoch)
    ///
    /// **Parameter:**
    /// - `current_epoch`: Aktuelle Epoche
    ///
    /// **Returns:** Abgeklingtes Gesamtgewicht (u64).
    pub fn decayed_weight(&self, current_epoch: u64) -> u64 {
        let mut total = 0u64;

        for (epoch, work) in &self.work_per_epoch {
            let age = current_epoch.saturating_sub(*epoch);
            if age > MAX_HISTORY_EPOCHS as u64 {
                continue;
            }

            // Berechne decay^age mit Ganzzahl-Arithmetik
            // decay = 95/100 = 0.95
            // decay^age = (95/100)^age
            let decayed = apply_decay(*work, age);
            // Sättigen statt überlaufen: ein Überlauf würde im
            // Debug-Build panicken und im Release-Build stillschweigend
            // umlaufen — zwei Nodes mit verschiedenen Build-Profilen
            // kämen zu verschiedenen Stimmgewichten.
            total = total.saturating_add(decayed);
        }

        total
    }
}

/// Wendet den Abklingfaktor auf einen Wert an.
///
/// **Parameter:**
/// - `value`: Ursprünglicher Wert
/// - `epochs`: Anzahl der Epochen zum Abklingen
///
/// **Returns:** Abgeklingter Wert.
fn apply_decay(value: u64, epochs: u64) -> u64 {
    let mut result = value as u128;

    for _ in 0..epochs {
        // result = result * 95 / 100 — u128, weil `value * 95` für
        // Werte oberhalb von u64::MAX/95 sonst überläuft (Panic im
        // Debug-Build, stiller Umlauf im Release-Build).
        result = (result * DECAY_FACTOR_NUM as u128) / DECAY_FACTOR_DEN as u128;
    }

    u64::try_from(result).unwrap_or(u64::MAX)
}

/// Eine vTFE-Einheit in vTFE-Kleinstbeträgen (1 vTFE = 10⁶).
///
/// Das ist der **Vorwärtspass genau eines Tokens** durch das ganze
/// Modell. Bis 2026-08-23 war dieser Wert zugleich die Bezugsgröße des
/// Arbeitsanteils, ein einzelnes Token verdoppelte also das
/// Stimmgewicht. Seit dem 2026-09-02 gibt es keine Bezugsgröße mehr;
/// diese Konstante behält ihre ursprüngliche Bedeutung als Einheit.
pub const VTFE_UNIT: u64 = 1_000_000;

/// Zähler der Arbeitsschwelle, als Bruchteil des Netzmedians.
///
/// ⚑ **Null, und das ist kein Platzhalter, sondern der einzige
/// zulässige Startwert.** Bei Genesis hat niemand Arbeitshistorie. Eine
/// Schwelle über null schlösse damit **jeden** Validator aus, und wer
/// ausgeschlossen ist, sammelt keine Arbeit und kommt nie herein: genau
/// die Bootstrap-Blockade, gegen die 2026-08-18 die Summenform gebaut
/// wurde.
///
/// Governance hebt die Schwelle, sobald das Netz eine messbare
/// Durchsatzverteilung hat. Ein Fünftel des Medians ist der Wert, den
/// die Entscheidung vom 2026-09-02 als Richtgröße nennt; gesetzt wird er
/// nicht hier, sondern in der Registry und erst mit Betrieb.
pub const ARBEITSSCHWELLE_ZAEHLER_VORGABE: u64 = 0;

/// Nenner der Arbeitsschwelle. Eins, damit `0/1` gilt.
///
/// Getrennt von der Einheit gehalten, weil ein Bruch aus zwei
/// Ganzzahlen exakt ist und eine Fließkommaschwelle im Konsenspfad nicht
/// vorkommt.
pub const ARBEITSSCHWELLE_NENNER_VORGABE: u64 = 1;

/// Parameter der Stimmgewichts-Qualifikation, aus der Governance-Registry.
///
/// **Wirksam wird eine Änderung zur nächsten Epochengrenze**, nicht
/// mitten in einer Epoche: Zwei Knoten, die mitten in einer Runde
/// verschiedene Schwellen benutzen, kommen zu verschiedenen Komitees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StimmgewichtsParameter {
    /// Zähler der Arbeitsschwelle als Bruchteil des Netzmedians.
    pub schwelle_zaehler: u64,
    /// Nenner der Arbeitsschwelle.
    pub schwelle_nenner: u64,
}

impl Default for StimmgewichtsParameter {
    fn default() -> Self {
        Self {
            schwelle_zaehler: ARBEITSSCHWELLE_ZAEHLER_VORGABE,
            schwelle_nenner: ARBEITSSCHWELLE_NENNER_VORGABE,
        }
    }
}

impl StimmgewichtsParameter {
    /// Sind die Werte brauchbar?
    ///
    /// Ein Nenner von null wäre eine Division durch null. Ein Zähler
    /// über dem Nenner verlangte **mehr als den Median**, und mehr als
    /// die Hälfte des Netzes liegt darunter: Das schlösse per
    /// Konstruktion die Mehrheit aus.
    pub fn ist_brauchbar(&self) -> bool {
        self.schwelle_nenner > 0 && self.schwelle_zaehler <= self.schwelle_nenner
    }
}

/// Das Stimmgewicht eines Validators: **sein Stake**.
///
/// Die Arbeit steht nicht mehr in dieser Zahl, sie steht in
/// [`arbeitsqualifikation`]. Die Begründung, die Recherche dahinter und
/// die beiden Funde, die sie ausgelöst haben, stehen im Modulkopf.
///
/// ⚑ **Die Signatur behält Historie und Epoche**, obwohl sie sie nicht
/// mehr liest, und das ist Absicht: Der nächste Schritt ist eine
/// gewichtete Qualifikation, und ein Aufrufer, der die Historie schon
/// heute durchreicht, muss dann nicht angefasst werden. **Wer eine
/// ungenutzte Angabe stehen lässt, schreibt eine Behauptung auf, die
/// niemand prüft** (Fund 117), deshalb steht sie hier ausdrücklich als
/// Vorhalt und nicht als vergessener Rest.
pub fn calculate_voting_weight(
    stake: u64,
    history: &InferenceHistory,
    current_epoch: u64,
) -> u64 {
    calculate_voting_weight_mit(
        stake,
        history,
        current_epoch,
        &StimmgewichtsParameter::default(),
    )
}

/// Wie [`calculate_voting_weight`], aber mit ausdrücklichen Parametern.
pub fn calculate_voting_weight_mit(
    stake: u64,
    _history: &InferenceHistory,
    _current_epoch: u64,
    _p: &StimmgewichtsParameter,
) -> u64 {
    stake
}

/// Hält dieser Validator die Arbeitsschwelle?
///
/// Die Schwelle ist ein Bruchteil des **Netzmedians** der abgeklungenen
/// Arbeit, nicht ein fester Bezugswert. ⚑ **Das ist der Kern der
/// Entscheidung vom 2026-09-02:** Ein fester Bezug veraltet mit jeder
/// Durchsatzoptimierung, und genau das ist in diesem Projekt zweimal
/// passiert (Fund 51 und die Tabelle, die ihm folgte). Ein relativer
/// Bezug wandert mit.
///
/// **Bei Schwelle null qualifiziert jeder**, auch wer nie gearbeitet
/// hat. Das ist der Startzustand und die Bedingung dafür, dass ein Netz
/// ohne Historie überhaupt anfangen kann.
///
/// **Ein Median von null qualifiziert ebenfalls jeden.** Hat niemand
/// gearbeitet, ist die Schwelle null mal irgendetwas, und niemanden
/// auszuschließen ist die einzige Antwort, die kein Netz zum Stillstand
/// bringt.
pub fn arbeitsqualifikation(
    history: &InferenceHistory,
    current_epoch: u64,
    netzmedian: u64,
    p: &StimmgewichtsParameter,
) -> bool {
    let vorgabe = StimmgewichtsParameter::default();
    let p = if p.ist_brauchbar() { p } else { &vorgabe };

    if p.schwelle_zaehler == 0 || netzmedian == 0 {
        return true;
    }
    let schwelle =
        (netzmedian as u128 * p.schwelle_zaehler as u128) / p.schwelle_nenner as u128;
    history.decayed_weight(current_epoch) as u128 >= schwelle
}

/// Der Median der abgeklungenen Arbeit über eine Validatorenmenge.
///
/// ⚑ **Median und nicht Mittelwert**, und der Grund ist ein Angriff:
/// Ein einzelner Teilnehmer mit sehr viel Arbeit hebt einen Mittelwert
/// beliebig weit und schlösse damit die Mehrheit aus. Auf den Median
/// wirkt er wie jeder andere auch.
///
/// Bei gerader Anzahl wird der **untere** der beiden mittleren Werte
/// genommen. Das ist die Wahl, die keine Division braucht und damit auf
/// jeder Maschine dasselbe ergibt; ein Mittelwert der beiden wäre eine
/// Rundung im Konsenspfad ohne Not.
pub fn netzmedian(werte: &mut [u64]) -> u64 {
    if werte.is_empty() {
        return 0;
    }
    werte.sort_unstable();
    werte[(werte.len() - 1) / 2]
}

/// Vergleicht zwei Validatoren nach Stimmgewicht (für Komiteewahl).
///
/// **Returns:** `true` wenn Validator A ein höheres Stimmgewicht hat als B.
pub fn compare_voting_weight(
    stake_a: u64,
    history_a: &InferenceHistory,
    stake_b: u64,
    history_b: &InferenceHistory,
    current_epoch: u64,
) -> bool {
    let weight_a = calculate_voting_weight(stake_a, history_a, current_epoch);
    let weight_b = calculate_voting_weight(stake_b, history_b, current_epoch);

    weight_a > weight_b
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------
    // Die Historie selbst. Sie bleibt, denn die Qualifikation liest sie.
    // -----------------------------------------------------------------

    #[test]
    fn inference_history_add_work() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);
        history.add_work(11, 2000);
        assert_eq!(history.work_per_epoch.len(), 2);
    }

    #[test]
    fn inference_history_update_existing() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);
        history.add_work(10, 500);
        assert_eq!(history.work_per_epoch.len(), 1);
        assert_eq!(history.work_per_epoch[0].1, 1500);
    }

    #[test]
    fn inference_history_old_entries_removed() {
        let mut history = InferenceHistory::new();
        for e in 0..(MAX_HISTORY_EPOCHS as u64 + 5) {
            history.add_work(e, 100);
        }
        assert_eq!(history.work_per_epoch.len(), MAX_HISTORY_EPOCHS);
    }

    #[test]
    fn decayed_weight_no_decay() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);
        assert_eq!(history.decayed_weight(10), 1000);
    }

    #[test]
    fn decayed_weight_one_epoch_decay() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);
        assert_eq!(history.decayed_weight(11), 950);
    }

    #[test]
    fn decayed_weight_multiple_epochs() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);
        history.add_work(11, 2000);
        // 1000 * 0,95² = 902, 2000 * 0,95 = 1900.
        assert_eq!(history.decayed_weight(12), 2802);
    }

    #[test]
    fn apply_decay_calculation() {
        assert_eq!(apply_decay(1000, 0), 1000);
        assert_eq!(apply_decay(1000, 1), 950);
        assert_eq!(apply_decay(1000, 2), 902);
    }

    #[test]
    fn decay_constants() {
        assert_eq!(DECAY_FACTOR_NUM, 95);
        assert_eq!(DECAY_FACTOR_DEN, 100);
        assert_eq!(MAX_HISTORY_EPOCHS, 10);
    }

    // -----------------------------------------------------------------
    // Das Gewicht: der Stake, und sonst nichts.
    // -----------------------------------------------------------------

    /// ⚑ **Die Aussage der Entscheidung vom 2026-09-02, als Test.**
    ///
    /// Kein Umfang an Arbeit ändert das Gewicht. Der Test zieht
    /// absichtlich extreme Historien heran, damit er nicht nur die
    /// leeren Fälle abdeckt.
    #[test]
    fn arbeit_aendert_das_gewicht_nicht_mehr() {
        let stake = 10_000_000u64;
        let ohne = calculate_voting_weight(stake, &InferenceHistory::new(), 10);

        for arbeit in [1u64, 1_000_000, 8_900_000_000, u64::MAX / 16] {
            let mut h = InferenceHistory::new();
            for e in 1..=MAX_HISTORY_EPOCHS as u64 {
                h.add_work(e, arbeit);
            }
            assert_eq!(
                calculate_voting_weight(stake, &h, MAX_HISTORY_EPOCHS as u64),
                ohne,
                "Arbeit {arbeit} hat das Gewicht bewegt; Arbeit qualifiziert, sie wiegt nicht"
            );
        }
        assert_eq!(ohne, stake, "das Gewicht ist der Stake");
    }

    /// Ohne Stake kein Gewicht, auch mit Arbeit.
    #[test]
    fn ohne_stake_hilft_auch_arbeit_nicht() {
        let mut h = InferenceHistory::new();
        h.add_work(10, u64::MAX / 4);
        assert_eq!(calculate_voting_weight(0, &h, 10), 0);
    }

    /// Extreme Stakes sättigen nicht und laufen nicht um.
    #[test]
    fn extremwerte_laufen_nicht_um() {
        let mut h = InferenceHistory::new();
        h.add_work(10, u64::MAX);
        assert_eq!(calculate_voting_weight(u64::MAX, &h, 10), u64::MAX);
    }

    #[test]
    fn test_compare_voting_weight_stake_matters() {
        let leer = InferenceHistory::new();
        assert!(compare_voting_weight(2_000_000, &leer, 1_000_000, &leer, 10));
        assert!(!compare_voting_weight(1_000_000, &leer, 2_000_000, &leer, 10));
    }

    /// ⚑ **Die Gegenrichtung, und sie ist der Sinn der Änderung.**
    ///
    /// Vor dem 2026-09-02 gewann bei gleichem Stake der mit mehr Arbeit.
    /// Jetzt gewinnt niemand: Gleicher Stake heißt gleiches Gewicht,
    /// unabhängig von der Historie.
    #[test]
    fn bei_gleichem_stake_entscheidet_arbeit_nicht_mehr() {
        let leer = InferenceHistory::new();
        let mut viel = InferenceHistory::new();
        viel.add_work(10, 8_900_000_000);
        assert!(!compare_voting_weight(1_000_000, &viel, 1_000_000, &leer, 10));
        assert!(!compare_voting_weight(1_000_000, &leer, 1_000_000, &viel, 10));
    }

    // -----------------------------------------------------------------
    // Die Qualifikation.
    // -----------------------------------------------------------------

    /// **Bei Schwelle null qualifiziert jeder.** Das ist der Startwert
    /// und die Bedingung dafür, dass ein Netz ohne Historie anfangen
    /// kann.
    #[test]
    fn bei_schwelle_null_qualifiziert_jeder() {
        let p = StimmgewichtsParameter::default();
        assert_eq!(p.schwelle_zaehler, 0);
        assert!(arbeitsqualifikation(&InferenceHistory::new(), 10, 0, &p));
        assert!(arbeitsqualifikation(
            &InferenceHistory::new(),
            10,
            1_000_000_000,
            &p
        ));
    }

    /// **Ein Median von null qualifiziert ebenfalls jeden.** Hat niemand
    /// gearbeitet, gibt es nichts, wogegen zu messen wäre.
    #[test]
    fn ein_median_von_null_qualifiziert_jeden() {
        let p = StimmgewichtsParameter {
            schwelle_zaehler: 1,
            schwelle_nenner: 5,
        };
        assert!(arbeitsqualifikation(&InferenceHistory::new(), 10, 0, &p));
    }

    /// **Mit Schwelle trennt sie**, und zwar am Median gemessen.
    #[test]
    fn eine_schwelle_ueber_null_trennt_am_median() {
        let p = StimmgewichtsParameter {
            schwelle_zaehler: 1,
            schwelle_nenner: 5,
        };
        let median = 1_000_000u64;

        let mut knapp_drueber = InferenceHistory::new();
        knapp_drueber.add_work(10, 200_000);
        assert!(arbeitsqualifikation(&knapp_drueber, 10, median, &p));

        let mut knapp_drunter = InferenceHistory::new();
        knapp_drunter.add_work(10, 199_999);
        assert!(!arbeitsqualifikation(&knapp_drunter, 10, median, &p));

        // Und die Gegenprobe: ohne Arbeit faellt man durch.
        assert!(!arbeitsqualifikation(&InferenceHistory::new(), 10, median, &p));
    }

    /// Unbrauchbare Parameter fallen auf die Vorgabe zurück, und die
    /// Vorgabe schließt niemanden aus.
    ///
    /// ⚑ **Die Richtung ist wichtig:** Ein kaputter Parameter darf das
    /// Netz nicht anhalten. Ein Nenner von null wäre eine Division durch
    /// null, ein Zähler über dem Nenner schlösse die Mehrheit aus.
    #[test]
    fn unbrauchbare_parameter_fallen_auf_die_vorgabe_zurueck() {
        let kaputt = [
            StimmgewichtsParameter {
                schwelle_zaehler: 1,
                schwelle_nenner: 0,
            },
            StimmgewichtsParameter {
                schwelle_zaehler: 3,
                schwelle_nenner: 2,
            },
        ];
        for p in kaputt {
            assert!(!p.ist_brauchbar());
            assert!(
                arbeitsqualifikation(&InferenceHistory::new(), 10, 1_000_000, &p),
                "ein kaputter Parameter darf niemanden ausschliessen"
            );
        }
    }

    // -----------------------------------------------------------------
    // Der Netzmedian.
    // -----------------------------------------------------------------

    #[test]
    fn der_median_nimmt_bei_gerader_zahl_den_unteren() {
        assert_eq!(netzmedian(&mut []), 0);
        assert_eq!(netzmedian(&mut [7]), 7);
        assert_eq!(netzmedian(&mut [1, 3]), 1);
        assert_eq!(netzmedian(&mut [3, 1, 2]), 2);
        assert_eq!(netzmedian(&mut [10, 1, 2, 3]), 2);
    }

    /// ⚑ **Ein einzelner Riese verschiebt den Median nicht.**
    ///
    /// Das ist der Grund für den Median statt eines Mittelwerts: Ein
    /// Teilnehmer mit sehr viel Arbeit hübe einen Mittelwert beliebig
    /// weit und schlösse damit die Mehrheit aus.
    #[test]
    fn ein_einzelner_riese_verschiebt_den_median_nicht() {
        let ohne = netzmedian(&mut [1, 2, 3, 4, 5]);
        let mit = netzmedian(&mut [1, 2, 3, 4, u64::MAX]);
        assert_eq!(ohne, mit, "der Median darf sich von einem Ausreisser nicht bewegen");
    }
}
