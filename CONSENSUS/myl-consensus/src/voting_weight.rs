//! Stimmgewichts-Kopplung — Whitepaper Kap. 3.5.2.
//!
//! Koppelt das Stimmgewicht an gestakten Coin **und** nachgewiesene
//! historische Inferenzarbeit (mit Abklingfaktor):
//!
//! ```text
//! voting_weight = stake + (stake · abgeklungene_Arbeit) / VTFE_UNIT
//! ```
//!
//! **Warum eine Summe und kein reines Produkt (Design-Entscheidung
//! 2026-08-18, Audit-Block 3):** Die ursprüngliche Formel war
//! `stake × Arbeit / 10¹²`. Ein reines Produkt gibt jedem Validator ohne
//! Arbeitshistorie das Gewicht **null** — und wer Gewicht null hat, wird
//! nie ins Komitee gewählt, kann nie Arbeit nachweisen und bleibt
//! dauerhaft bei null. Bei Genesis hätte *kein* Validator Gewicht, das
//! Komitee wäre nicht wählbar. Die Summenform hält die Aussage des
//! Whitepapers („speist sich aus zwei Quellen") und löst die Blockade:
//! der Stake ist die Grundlage, die Arbeit multipliziert sie hoch. Ein
//! Validator mit einer vollen vTFE-Einheit abgeklungener Arbeit hat
//! doppeltes Gewicht gegenüber einem gleich gestakten ohne Arbeit.
//!
//! ## Wiedervorlage erledigt (2026-08-23): der Bezugswert war um drei
//! bis fünf Größenordnungen zu klein
//!
//! Die Wiedervorlage von 2026-08-18 nannte zwei offene Punkte, beide
//! blockiert durch dieselbe fehlende Zahl: *„hängt an der real
//! erreichbaren vTFE-Menge pro Epoche, die noch nicht gemessen ist."*
//!
//! Seit der Festlegung der vTFE-Zuschreibung (`myl_tokenomics::vtfe`,
//! 2026-08-23) ist sie ausrechenbar. **Der alte Bezugswert `VTFE_UNIT`
//! entspricht dem Vorwärtspass eines einzigen Tokens.** Gemessen an den
//! echten Durchsatzwerten des Projekts und einer Stunden-Epoche
//! (Whitepaper Kap. 3.2):
//!
//! | Fall | Verdopplung nach | Faktor nach einer Epoche | volle Historie |
//! |---|---|---|---|
//! | 0,5B, ganzes Modell, 38,19 tok/s | 0,03 s | **137 484** | 1 103 345 |
//! | 0,5B, Viertel-Shard | 0,14 s | 24 898 | 199 816 |
//! | 7B, ganzes Modell, 2,07 tok/s | 0,48 s | 7 452 | 59 804 |
//! | 7B, Viertel-Shard | 2,1 s | 1 719 | 13 799 |
//!
//! **Der Stake hörte damit nach wenigen Sekunden Arbeit auf,
//! Angriffskosten zu sein.** Genau davor warnte der zweite offene Punkt;
//! die Zahlen zeigen, dass es keine ferne Sorge war, sondern der
//! Normalfall ab der ersten Epoche.
//!
//! **Behoben mit zwei Sicherungen (Entscheidung Projektinhaber,
//! 2026-08-23), und zwar bewusst mit zweien:**
//!
//! 1. **Der Bezugswert ist ein Governance-Parameter geworden**
//!    ([`StimmgewichtsParameter::arbeitsbezug`]). Er steht auf der
//!    vTFE-Menge, die ein **Referenzknoten in einer Epoche** schafft,
//!    nicht mehr auf einem einzelnen Token. Ein Knoten mit
//!    Referenzdurchsatz bekommt damit je Epoche einen Bonus in der
//!    Größenordnung seines Stakes statt des Tausendfachen.
//! 2. **Ein harter Deckel** ([`StimmgewichtsParameter::hoechstfaktor`]):
//!    Das Gesamtgewicht übersteigt den Stake nie um mehr als diesen
//!    Faktor.
//!
//! Die zweite Sicherung ist nicht überflüssig neben der ersten: Der
//! Bezugswert ist **parametrisch** und kann falsch gesetzt werden, der
//! Deckel nicht. Eine Fehlkalibrierung um drei Größenordnungen, wie sie
//! hier vorlag, schlägt mit Deckel auf Faktor 10 durch statt auf Faktor
//! 137 000.
//!
//! **Was bleibt:** Die Alternative — Bootstrap über ein per
//! Konfiguration gesetztes Genesis-Komitee, danach reines Produkt
//! `stake × Arbeit` — ist weiterhin zurückgestellt, nicht verworfen.
//!
//! Vermerk auch in `CONSENSUS/README/Fahrplan-v1.md` (Abschnitt „Zur
//! Wiedervorlage") und in `README/Intern/State-of-the-Project.md`,
//! Abschnitt 7.
//!
//! **Konsens-Feld:** Die Stimmgewichts-Berechnung ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

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
/// Arbeitsanteils, ein einzelnes Token verdoppelte also das Stimmgewicht.
/// Der Bezug steht jetzt in [`StimmgewichtsParameter::arbeitsbezug`];
/// diese Konstante behält ihre ursprüngliche Bedeutung als Einheit.
pub const VTFE_UNIT: u64 = 1_000_000;

/// Bezugsgröße des Arbeitsanteils: die vTFE-Menge, die einen Bonus in
/// Höhe des Stakes wert ist.
///
/// **Herleitung der Vorgabe.** Referenzfall ist ein Knoten, der ein
/// Viertel von Qwen2.5-7B hält und eine Stunden-Epoche durchläuft:
/// 230 729 vTFE-Einheiten je Token bei **10,74 tok/s** ergeben
/// 8 920 906 056 Einheiten je Epoche. Gerundet auf **8,9 · 10⁹**.
///
/// Damit gilt: Ein Knoten mit Referenzdurchsatz verdient in einer Epoche
/// einen Bonus von etwa einem Stake. Über die volle Historie von zehn
/// Epochen summiert sich das mit dem Abklingfaktor auf rund das
/// Achtfache, das Gesamtgewicht liegt also knapp unter dem Deckel. **Das
/// ist Absicht:** Der Deckel soll erreichbar sein, aber erst von einem
/// Knoten, der dauerhaft über Referenzdurchsatz liefert.
///
/// # ⚑ Fund 51: Hier stand 1,7 · 10⁹, und die Absicht war zerstört
///
/// Die erste Herleitung (2026-08-23) rechnete mit **2,07 tok/s**, dem
/// Durchsatz **vor** der Zeilen-Parallelisierung (kernels v0.21.0). Die
/// Parallelisierung hob ihn am selben Tag auf 10,74 tok/s, also um das
/// 5,19-Fache; der Bezugswert blieb stehen.
///
/// | | Bonus je Epoche | über 10 Epochen | Deckel (10) |
/// |---|---|---|---|
/// | mit 1,7 · 10⁹ | 5,25 Stake | 42,1 | **greift immer** |
/// | mit 8,9 · 10⁹ | 1,01 Stake | 8,12 | greift nicht |
///
/// Mit dem alten Wert saß **jeder** Knoten bei Referenzdurchsatz am
/// Deckel. Der Arbeitsanteil unterschied damit nicht mehr zwischen
/// Knoten, was seine einzige Aufgabe ist: Ein Knoten mit doppeltem
/// Durchsatz bekam dasselbe Gewicht wie einer mit einfachem.
///
/// Dieselbe Klasse wie der veraltete K8-Eintrag im Fahrplan: eine Zahl,
/// die richtig abgeleitet wurde und deren Grundlage sich am selben Tag
/// verschoben hat. **Jeder feste Bezugswert veraltet mit der nächsten
/// Optimierung**; ein selbstregulierender Bezug (etwa der Median des
/// Netzes) steht als Entwurf im Fahrplan.
///
/// Der Wert hängt an Modell und Hardware und gehört deshalb in die
/// Governance-Registry, nicht in eine Konstante. Er steht hier als
/// **Startparameter** und wird von
/// `myl-governance/tests/gleichstand.rs` gegen die Registry geprüft,
/// damit die nächste Verschiebung sofort auffällt.
pub const ARBEITSBEZUG_VORGABE: u64 = 8_900_000_000;

/// Höchstfaktor des Gesamtgewichts auf den Stake.
///
/// `voting_weight ≤ stake · HOECHSTFAKTOR_VORGABE`. Ohne diesen Deckel
/// kann ein Knoten mit viel Arbeit sein Gewicht beliebig weit über
/// seinen Stake heben, und der Stake verliert seine Funktion als
/// Angriffskosten.
///
/// Der Wert 10 stammt aus dem Wiedervorlage-Vermerk vom 2026-08-18
/// („z. B. Faktor 10"). Ebenfalls Governance-Parameter.
pub const HOECHSTFAKTOR_VORGABE: u64 = 10;

/// Die beiden Größen, die den Arbeitsanteil des Stimmgewichts steuern.
///
/// Beide sind **konsensrelevant** (Kap. 10.3): Zwei Knoten mit
/// verschiedenen Werten kommen zu verschiedenen Komitees. Sie gehören
/// deshalb in die Governance-Registry und werden zur Epochengrenze
/// gewechselt, nicht mitten in einer Epoche.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StimmgewichtsParameter {
    /// vTFE-Menge, die einen Bonus in Höhe des Stakes wert ist.
    pub arbeitsbezug: u64,
    /// Höchstfaktor des Gesamtgewichts auf den Stake.
    pub hoechstfaktor: u64,
}

impl Default for StimmgewichtsParameter {
    fn default() -> Self {
        Self {
            arbeitsbezug: ARBEITSBEZUG_VORGABE,
            hoechstfaktor: HOECHSTFAKTOR_VORGABE,
        }
    }
}

impl StimmgewichtsParameter {
    /// Sind die Werte brauchbar?
    ///
    /// `arbeitsbezug = 0` wäre eine Division durch null, und
    /// `hoechstfaktor = 0` gäbe jedem Validator das Gewicht null, also
    /// genau die Bootstrap-Blockade, gegen die die Summenform gebaut
    /// wurde.
    pub fn ist_brauchbar(&self) -> bool {
        self.arbeitsbezug > 0 && self.hoechstfaktor >= 1
    }
}

/// Berechnet das Stimmgewicht eines Validators mit den Vorgabewerten.
///
/// **Formel:**
/// `voting_weight = min(stake + stake · decayed_work / arbeitsbezug,
/// stake · hoechstfaktor)`
///
/// Der Stake ist die Grundlage, die nachgewiesene Arbeit multipliziert
/// sie hoch, der Deckel begrenzt sie. Siehe die Modul-Dokumentation für
/// die Begründung, warum hier eine Summe und kein reines Produkt steht
/// (Bootstrap-Blockade), und warum der Bezugswert seit 2026-08-23 nicht
/// mehr ein einzelnes Token ist.
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
///
/// **Parameter:**
/// - `stake`: Stake des Validators (in MYL-Kleinstbeträgen)
/// - `history`: Historische Inferenzarbeit
/// - `current_epoch`: Aktuelle Epoche
/// - `p`: Bezugswert und Deckel (Governance)
///
/// Unbrauchbare Parameter fallen auf die Vorgabe zurück, statt eine
/// Division durch null oder ein Gewicht von null zu erzeugen: Ein
/// Konsenspfad darf an einem Konfigurationsfehler nicht anhalten, und
/// ein Gewicht von null wäre die Bootstrap-Blockade.
///
/// **Returns:** Stimmgewicht (u64, gesättigt statt überlaufend).
pub fn calculate_voting_weight_mit(
    stake: u64,
    history: &InferenceHistory,
    current_epoch: u64,
    p: &StimmgewichtsParameter,
) -> u64 {
    let vorgabe = StimmgewichtsParameter::default();
    let p = if p.ist_brauchbar() { p } else { &vorgabe };

    let decayed_work = history.decayed_weight(current_epoch);

    // stake ist in MYL-Kleinstbeträgen (1 MYL = 10^6),
    // decayed_work in vTFE-Kleinstbeträgen (1 vTFE = 10^6).
    // u128 für die Zwischenrechnung, Sättigung statt Überlauf.
    let work_bonus = (stake as u128 * decayed_work as u128) / p.arbeitsbezug as u128;
    let weight = stake as u128 + work_bonus;

    // **Der Deckel greift auf das Gesamtgewicht, nicht auf den Bonus.**
    // So steht die Zusage direkt da, die er geben soll: Ein Validator
    // wiegt nie mehr als `hoechstfaktor` Stakes.
    let deckel = stake as u128 * p.hoechstfaktor as u128;

    u64::try_from(weight.min(deckel)).unwrap_or(u64::MAX)
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

    #[test]
    fn inference_history_add_work() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);
        history.add_work(11, 2000);

        assert_eq!(history.work_per_epoch.len(), 2);
        // Sortiert absteigend nach Epoche
        assert_eq!(history.work_per_epoch[0], (11, 2000));
        assert_eq!(history.work_per_epoch[1], (10, 1000));
    }

    #[test]
    fn inference_history_update_existing() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);
        history.add_work(10, 500); // Aktualisiere Epoche 10

        assert_eq!(history.work_per_epoch.len(), 1);
        assert_eq!(history.work_per_epoch[0], (10, 1500));
    }

    #[test]
    fn inference_history_old_entries_removed() {
        let mut history = InferenceHistory::new();

        // Füge Arbeit über MAX_HISTORY_EPOCHS hinzu
        for i in 0..15 {
            history.add_work(i, 1000);
        }

        // Nur die letzten MAX_HISTORY_EPOCHS sollten übrig sein
        assert!(history.work_per_epoch.len() <= MAX_HISTORY_EPOCHS);
    }

    #[test]
    fn decayed_weight_no_decay() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);

        // Keine Abklingung (current_epoch = 10)
        let weight = history.decayed_weight(10);
        assert_eq!(weight, 1000);
    }

    #[test]
    fn decayed_weight_one_epoch_decay() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);

        // Eine Epoche Abklingung: 1000 * 0.95 = 950
        let weight = history.decayed_weight(11);
        assert_eq!(weight, 950);
    }

    #[test]
    fn decayed_weight_multiple_epochs_decay() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);

        // Zwei Epochen Abklingung: 1000 * 0.95^2 = 902
        let weight = history.decayed_weight(12);
        assert_eq!(weight, 902);
    }

    #[test]
    fn decayed_weight_multiple_epochs() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);
        history.add_work(11, 2000);

        // Epoche 10: 1000 * 0.95^2 = 902
        // Epoche 11: 2000 * 0.95^1 = 1900
        // Total: 902 + 1900 = 2802
        let weight = history.decayed_weight(12);
        assert_eq!(weight, 2802);
    }

    /// Der Bezugswert verdoppelt das Gewicht, und zwar **er** und nicht
    /// mehr ein einzelnes Token.
    #[test]
    fn der_arbeitsbezug_verdoppelt_das_gewicht() {
        let mut history = InferenceHistory::new();
        history.add_work(10, ARBEITSBEZUG_VORGABE);

        let stake = 10_000_000; // 10 MYL
        assert_eq!(calculate_voting_weight(stake, &history, 10), 20_000_000);
    }

    #[test]
    fn calculate_voting_weight_with_decay() {
        let mut history = InferenceHistory::new();
        history.add_work(10, ARBEITSBEZUG_VORGABE);

        let stake = 10_000_000; // 10 MYL
        // Arbeit klingt eine Epoche ab: Bezug × 0,95
        assert_eq!(calculate_voting_weight(stake, &history, 11), 19_500_000);
    }

    /// **Der Befund vom 2026-08-23, als Test festgehalten.** Ein einzelnes
    /// Token, also eine volle vTFE-Einheit, hob das Gewicht früher aufs
    /// Doppelte. Jetzt bewegt es fast nichts, und das ist der Sinn der
    /// Änderung: Der Bezug ist die Arbeit einer Epoche, nicht die eines
    /// Tokens.
    #[test]
    fn ein_einzelnes_token_verdoppelt_das_gewicht_nicht_mehr() {
        let mut history = InferenceHistory::new();
        history.add_work(10, VTFE_UNIT); // genau ein Token

        let stake = 10_000_000;
        let weight = calculate_voting_weight(stake, &history, 10);
        assert!(
            weight < stake + stake / 1000,
            "ein Token darf das Gewicht kaum bewegen, ergab aber {}",
            weight
        );
    }

    /// **Eine Epoche Referenzarbeit ergibt etwa einen Stake Bonus.**
    ///
    /// Das ist die Kalibrierungsaussage selbst, und sie wird **gegen die
    /// Konstante** geprüft und nicht gegen eine getippte Zahl: Vor
    /// Fund 51 stand hier das Literal `1_719_394_757`, und als sich der
    /// Bezugswert korrigierte, schlug der Test fehl, obwohl an der
    /// geprüften Regel nichts falsch war. Geprüft gehört die Regel „eine
    /// Epoche Referenzarbeit ist einen Stake wert", nicht die Zahl.
    #[test]
    fn eine_epoche_referenzarbeit_ist_etwa_einen_stake_wert() {
        let mut history = InferenceHistory::new();
        history.add_work(10, ARBEITSBEZUG_VORGABE);

        let stake = 10_000_000;
        let weight = calculate_voting_weight(stake, &history, 10);

        // Stake plus etwa ein Stake Bonus, also rund das Doppelte.
        assert!(weight > stake * 2 - stake / 10, "ergab {weight}");
        assert!(weight < stake * 3, "ergab {weight}");
    }

    /// **Regression zu Fund 51: Der alte Bezugswert setzte jeden Knoten
    /// an den Deckel.**
    ///
    /// `ARBEITSBEZUG_VORGABE` stand bis zum 2026-08-24 auf 1,7·10⁹, dem
    /// Durchsatz **vor** der Zeilen-Parallelisierung. Mit dem heute
    /// gemessenen Durchsatz liefert ein Referenzknoten 8,9·10⁹ je Epoche,
    /// also das 5,19-Fache des alten Bezugs; über zehn Epochen sind das
    /// 42 Stakes Bonus und damit weit über dem Deckel von 10.
    ///
    /// **Die Folge war nicht ein zu hohes Gewicht, sondern gar keine
    /// Unterscheidung mehr:** Am Deckel bekommt ein Knoten mit doppeltem
    /// Durchsatz dasselbe Gewicht wie einer mit einfachem, und der
    /// Arbeitsanteil verliert seine einzige Aufgabe.
    #[test]
    fn der_alte_bezugswert_haette_jeden_knoten_an_den_deckel_gesetzt() {
        const ALT: u64 = 1_700_000_000;
        let stake = 10_000_000;
        let mut history = InferenceHistory::new();
        // Zehn Epochen Referenzarbeit nach heutiger Messung.
        for e in 1..=10u64 {
            history.add_work(e, ARBEITSBEZUG_VORGABE);
        }

        let alt = StimmgewichtsParameter {
            arbeitsbezug: ALT,
            hoechstfaktor: HOECHSTFAKTOR_VORGABE,
        };
        let alt_weight = calculate_voting_weight_mit(stake, &history, 10, &alt);
        assert_eq!(
            alt_weight,
            stake * HOECHSTFAKTOR_VORGABE,
            "mit dem alten Bezug säße der Referenzknoten am Deckel"
        );

        // Mit dem korrigierten Bezug bleibt er darunter, und das ist der
        // Sinn der Kalibrierung.
        let jetzt = calculate_voting_weight(stake, &history, 10);
        assert!(
            jetzt < stake * HOECHSTFAKTOR_VORGABE,
            "der Referenzknoten darf den Deckel nicht erreichen, ergab {jetzt}"
        );
        // Und ein Knoten mit doppelter Arbeit bekommt jetzt mehr — genau
        // das, was am Deckel verlorenginge.
        let mut doppelt = InferenceHistory::new();
        for e in 1..=10u64 {
            doppelt.add_work(e, ARBEITSBEZUG_VORGABE * 2);
        }
        assert!(
            calculate_voting_weight(stake, &doppelt, 10) > jetzt,
            "doppelte Arbeit muss mehr Gewicht ergeben"
        );
    }

    /// **Der Deckel greift**, auch wenn der Bezugswert falsch gesetzt
    /// wäre. Das ist der Grund für zwei Sicherungen statt einer: Der
    /// Bezug ist parametrisch, der Deckel nicht.
    #[test]
    fn der_deckel_faengt_eine_fehlkalibrierung_ab() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1_719_394_757);

        let stake = 10_000_000;
        let falsch = StimmgewichtsParameter {
            arbeitsbezug: VTFE_UNIT, // um drei Größenordnungen zu klein
            hoechstfaktor: HOECHSTFAKTOR_VORGABE,
        };
        let weight = calculate_voting_weight_mit(stake, &history, 10, &falsch);
        assert_eq!(weight, stake * HOECHSTFAKTOR_VORGABE);
    }

    /// Ein Knoten mit Referenzdurchsatz über die volle Historie liegt
    /// knapp **unter** dem Deckel. Absicht: Der Deckel soll erreichbar
    /// sein, aber erst oberhalb des Referenzdurchsatzes.
    #[test]
    fn volle_historie_bei_referenzdurchsatz_liegt_knapp_unter_dem_deckel() {
        let mut history = InferenceHistory::new();
        for epoche in 0..MAX_HISTORY_EPOCHS as u64 {
            history.add_work(epoche, ARBEITSBEZUG_VORGABE);
        }
        let stake = 10_000_000;
        let weight = calculate_voting_weight(stake, &history, MAX_HISTORY_EPOCHS as u64 - 1);
        let deckel = stake * HOECHSTFAKTOR_VORGABE;

        assert!(weight < deckel, "{} muss unter {} liegen", weight, deckel);
        assert!(
            weight > deckel * 8 / 10,
            "{} sollte nah am Deckel liegen ({})",
            weight,
            deckel
        );
    }

    /// Unbrauchbare Parameter dürfen den Konsenspfad nicht anhalten und
    /// nicht das Gewicht null erzeugen (Bootstrap-Blockade).
    #[test]
    fn unbrauchbare_parameter_fallen_auf_die_vorgabe_zurueck() {
        let mut history = InferenceHistory::new();
        history.add_work(10, ARBEITSBEZUG_VORGABE);
        let stake = 10_000_000;
        let erwartet = calculate_voting_weight(stake, &history, 10);

        for kaputt in [
            StimmgewichtsParameter {
                arbeitsbezug: 0,
                hoechstfaktor: 10,
            },
            StimmgewichtsParameter {
                arbeitsbezug: ARBEITSBEZUG_VORGABE,
                hoechstfaktor: 0,
            },
        ] {
            assert!(!kaputt.ist_brauchbar());
            assert_eq!(
                calculate_voting_weight_mit(stake, &history, 10, &kaputt),
                erwartet
            );
        }
    }

    /// Ohne Stake bleibt das Gewicht null, gleich wie viel gearbeitet
    /// wurde: Der Arbeitsanteil ist ein Faktor auf den Stake, keine
    /// eigene Quelle.
    #[test]
    fn ohne_stake_hilft_auch_arbeit_nicht() {
        let mut history = InferenceHistory::new();
        history.add_work(10, ARBEITSBEZUG_VORGABE * 1000);
        assert_eq!(calculate_voting_weight(0, &history, 10), 0);
    }

    /// Bootstrap: ohne Arbeitshistorie muss der Stake allein zählen.
    /// Die alte Produktformel lieferte hier 0 — ein Validator mit
    /// Gewicht 0 wird nie gewählt, kann nie Arbeit nachweisen und
    /// bleibt dauerhaft bei 0. Bei Genesis wäre kein Komitee wählbar.
    #[test]
    fn ohne_arbeitshistorie_zaehlt_der_stake() {
        let history = InferenceHistory::new();
        let stake = 10_000_000;
        assert_eq!(calculate_voting_weight(stake, &history, 0), stake);
    }

    /// Arbeit erhöht das Gewicht monoton.
    #[test]
    fn mehr_arbeit_ergibt_mehr_gewicht() {
        let stake = 10_000_000;
        let mut wenig = InferenceHistory::new();
        wenig.add_work(5, VTFE_UNIT);
        let mut viel = InferenceHistory::new();
        viel.add_work(5, VTFE_UNIT * 3);

        assert!(
            calculate_voting_weight(stake, &viel, 5)
                > calculate_voting_weight(stake, &wenig, 5)
        );
    }

    /// Kein Überlauf bei extremen Werten — vorher panickte
    /// `apply_decay` im Debug-Build (`value * 95`) und lief im
    /// Release-Build still um.
    #[test]
    fn extremwerte_saettigen_statt_zu_ueberlaufen() {
        let mut history = InferenceHistory::new();
        history.add_work(0, u64::MAX);
        // Darf weder panicken noch umlaufen.
        let w = calculate_voting_weight(u64::MAX, &history, 0);
        assert_eq!(w, u64::MAX);

        let mut spaet = InferenceHistory::new();
        spaet.add_work(0, u64::MAX);
        let _ = spaet.decayed_weight(9); // neun Abkling-Schritte
    }

    #[test]
    fn test_compare_voting_weight_work_matters() {
        let mut history_a = InferenceHistory::new();
        history_a.add_work(10, 2_000_000); // 2 vTFE

        let mut history_b = InferenceHistory::new();
        history_b.add_work(10, 1_000_000); // 1 vTFE

        let stake_a = 10_000_000;
        let stake_b = 10_000_000;

        // A hat mehr Arbeit → höheres Gewicht
        assert!(compare_voting_weight(
            stake_a, &history_a,
            stake_b, &history_b,
            10
        ));
    }

    #[test]
    fn test_compare_voting_weight_stake_matters() {
        let mut history_a = InferenceHistory::new();
        history_a.add_work(10, 1_000_000);

        let mut history_b = InferenceHistory::new();
        history_b.add_work(10, 1_000_000);

        let stake_a = 20_000_000; // 20 MYL
        let stake_b = 10_000_000; // 10 MYL

        // A hat mehr Stake → höheres Gewicht
        assert!(compare_voting_weight(
            stake_a, &history_a,
            stake_b, &history_b,
            10
        ));
    }

    #[test]
    fn apply_decay_calculation() {
        assert_eq!(apply_decay(1000, 0), 1000);
        assert_eq!(apply_decay(1000, 1), 950);
        assert_eq!(apply_decay(1000, 2), 902);
        assert_eq!(apply_decay(1000, 3), 856);
    }

    #[test]
    fn decay_constants() {
        assert_eq!(DECAY_FACTOR_NUM, 95);
        assert_eq!(DECAY_FACTOR_DEN, 100);
        assert_eq!(MAX_HISTORY_EPOCHS, 10);
    }
}
