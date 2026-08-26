//! Epochenabschluss — Whitepaper Kap. 3.5.3, Anhang A.5 (Punkt 4.2).
//!
//! Aus den eingereichten Ansprüchen ([`crate::poi`]) wird die **bestätigte**
//! Arbeit einer Epoche. Erst daraus wird geprägt. Der Unterschied ist der
//! ganze Punkt: 4.1 stellt fest, dass ein Pod eine Menge geschlossen
//! behauptet hat, 4.2 stellt fest, ob sie ihm zusteht.
//!
//! ## Entwurfsgrundsatz: alles, was nicht positiv belegt ist, zählt nicht
//!
//! Myelith ist quelloffen. Ein Angreifer kennt jede Regel dieses Moduls,
//! jede Konstante und jeden Randfall. Eine Regel, die nur schützt,
//! solange niemand sie kennt, schützt nicht. Deshalb ist die
//! Grundeinstellung **Ablehnung**, und jede Gutschrift braucht einen
//! positiven Beleg:
//!
//! - **Kein Beleg, keine Gutschrift.** Ein Pod ohne Stufe-1-Bestätigung
//!   bekommt nichts — auch dann nicht, wenn schlicht keine Meldung
//!   vorliegt. Andernfalls wäre „Redundanzpartner unerreichbar machen"
//!   eine Strategie: der Angreifer beseitigt den Zeugen und wird für die
//!   fehlende Aussage belohnt. [`PodAgreement::Missing`] und
//!   [`PodAgreement::Mismatch`] führen beide zu null.
//! - **Kein Anspruch, keine Rückbuchung ins Minus.** Eine Rückbuchung
//!   kann höchstens den Anspruch selbst aufzehren. Ein negativer Saldo
//!   wäre eine Gutschrift an alle anderen und damit ein Hebel.
//! - **Ein Abschluss je Epoche.** [`EpochClosing`] entsteht einmal; ein
//!   zweiter Lauf über dieselbe Registry ergibt denselben Wert, prägt
//!   aber nicht ein zweites Mal — die Prägung liest den Abschluss, sie
//!   akkumuliert nicht.
//! - **Rückbuchungen gelten je Segment, nicht je Betrag.** Zweimal
//!   dieselbe Widerlegung einzureichen buchtnicht zweimal zurück
//!   ([`EpochClosing::apply_clawback`] führt die Segment-Ids mit).
//!
//! ## Was hier bewusst nicht entschieden wird
//!
//! **Was eine vTFE-Einheit zählt.** Seit dem 2026-08-23 steht die Regel
//! in `myl_tokenomics::vtfe`: der Anteil eines Shards an den
//! Multiplikations-Additionen der Gewichtsmatrizen eines vollen
//! Vorwärtspasses, mal der Zahl der erzeugten Token. Dieses Modul trifft
//! sie weiterhin **nicht** und rechnet auch nicht damit: `vtfe_claimed`
//! kommt aus dem Bündel und geht als Zahl durch; hier wird nur gerechnet,
//! welcher Anteil davon bestätigt ist. Die frühere Fassung dieses
//! Absatzes führte die Festlegung als offen; das war bis zu diesem Datum
//! richtig. Ändert sich die Zählweise erneut, ändert sich an diesem Code
//! nach wie vor nichts.
//!
//! ## Streitfrist
//!
//! Der Abschluss ist zunächst **vorläufig**. Die Streitfrist beträgt
//! 7 Tage (Design-Entscheidung 4 des CONSENSUS, als
//! Epochenzahl parametriert); innerhalb dieser Frist kann ein
//! Schiedsspruch Segmente widerlegen und Arbeit zurückbuchen
//! ([`EpochClosing::apply_clawback`]). Erst
//! [`EpochClosing::is_final`] macht den Wert endgültig.
//!
//! **Konsens-Feld:** Aggregationsregeln sind Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use crate::poi::PoIRegistry;
use myl_types::ids::{EpochId, PodId, SegmentId};
use std::collections::{BTreeMap, BTreeSet};

/// Standard-Streitfrist in Epochen.
///
/// Die Design-Entscheidung vom 2026-08-13 lautet **7 Tage**. Bei
/// Stunden-Epochen sind das 168 Epochen.
///
/// # ⚑ Fund 50: Hier standen 7, und die Begründung war zirkulär
///
/// Bis zum 2026-08-24 stand hier `7` mit dem Kommentar „Entspricht der
/// Design-Entscheidung ‚7 Tage' bei 2 s Blockzeit und einer
/// Epochenlänge, die GOVERNANCE festlegt." Der Satz nennt die
/// Epochenlänge als offen und rechnet zugleich mit ihr: `7 Epochen = 7
/// Tage` gilt nur bei **Tages**-Epochen.
///
/// Der Rest des Projekts rechnet mit **Stunden**-Epochen. Anhang B.1
/// sagt „Bei Stunden-Epochen: etwa ein Tag Einkommen als Pfand", und die
/// Stimmgewichts-Kalibrierung vom 2026-08-23 rechnet den „Faktor nach
/// einer Stunden-Epoche". Die Frist war damit **7 Stunden statt 7 Tagen**,
/// ein Faktor 24.
///
/// **Was daran hängt:** Die Streitfrist ist die Zeit, in der ein Betrug
/// angefochten werden kann und in der [`crate::da::DaStore`] die
/// Fragmente vorhalten muss. Sieben Stunden sind die Zeit, die ein
/// Checker hat, um eine Abweichung zu bemerken, das Bisektionsspiel zu
/// führen und die Schiedsrunde zu erreichen; danach ist der Epochen-
/// abschluss endgültig und die Daten dürfen verschwinden.
///
/// **Gefunden vom Gleichstands-Test der Parameter-Registry**
/// (`myl-governance/tests/gleichstand.rs`), nicht durch Codelektüre: Der
/// Vergleich zwischen Registry und Konstante braucht die Epochenlänge,
/// und dabei fiel auf, dass es sie nirgends gab.
///
/// **Die Kosten der Korrektur gehören genannt:** Die Vorhaltung im
/// `DaStore` dauert jetzt 24-mal so lange. Ob 7 Tage der richtige Wert
/// sind oder ob die Design-Entscheidung angesichts dessen zu ändern ist,
/// ist eine Abwägung zwischen Speicherkosten und Anfechtungsfenster und
/// ist ein offener Punkt. Der Wert hier folgt der
/// **Entscheidung von 2026-08-13**, weil eine Konstante, die ihrer
/// eigenen Begründung widerspricht, in jedem Fall falsch ist.
pub const DEFAULT_DISPUTE_EPOCHS: u64 = 168;

/// Ergebnis des Stufe-1-Redundanzvergleichs für einen Pod.
///
/// Kommt aus VERIFICATION (`myl-verifier::redundancy`). Die drei Fälle
/// sind bewusst getrennt: `Missing` ist **nicht** dasselbe wie `Match`
/// und darf nie so behandelt werden (siehe Modulkopf).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PodAgreement {
    /// Der redundante Partner-Pod kam zum selben Commitment.
    Match,
    /// Die Commitments weichen ab — der Anspruch ist strittig.
    Mismatch,
    /// Es liegt kein Vergleichsergebnis vor.
    Missing,
}

impl PodAgreement {
    /// Berechtigt dieses Ergebnis zu einer Gutschrift?
    ///
    /// Nur [`Self::Match`]. Das ist die Umsetzung von „alles, was nicht
    /// positiv belegt ist, zählt nicht".
    pub fn berechtigt(&self) -> bool {
        matches!(self, Self::Match)
    }
}

/// Eine widerlegte Arbeitseinheit.
///
/// Trägt das Segment mit, nicht nur den Betrag: Rückbuchungen müssen
/// idempotent sein, und ohne Segment-Id ließe sich dieselbe Widerlegung
/// mehrfach anrechnen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefutedSegment {
    /// Das widerlegte Segment.
    pub segment_id: SegmentId,
    /// Der Pod, dem die Arbeit gutgeschrieben war.
    pub pod: PodId,
    /// Die zurückzubuchende Arbeitsmenge in vTFE.
    pub vtfe: u64,
}

/// Fehler beim Epochenabschluss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochError {
    /// Rückbuchung für einen Pod, der in dieser Epoche nichts bestätigt
    /// bekommen hat. Deutet auf inkonsistente Eingaben hin und wird
    /// nicht stillschweigend verworfen.
    UnknownPod {
        /// Der unbekannte Pod.
        pod: PodId,
    },
    /// Diese Widerlegung wurde bereits verbucht.
    DuplicateRefutation {
        /// Das doppelt eingereichte Segment.
        segment_id: SegmentId,
    },
    /// Die Rückbuchung betrifft eine andere Epoche.
    WrongEpoch {
        /// Epoche des Abschlusses.
        expected: u64,
        /// Epoche der Rückbuchung.
        got: u64,
    },
}

impl std::fmt::Display for EpochError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPod { pod } => write!(
                f,
                "Rückbuchung für Pod {} ohne bestätigte Arbeit in dieser Epoche",
                pod
            ),
            Self::DuplicateRefutation { segment_id } => {
                write!(f, "Widerlegung für Segment {} bereits verbucht", segment_id)
            }
            Self::WrongEpoch { expected, got } => {
                write!(f, "Erwartete Epoche {}, bekommen {}", expected, got)
            }
        }
    }
}

impl std::error::Error for EpochError {}

/// Der Abschluss einer Epoche.
///
/// Hält je Pod die bestätigte Arbeit und die davon bereits
/// zurückgebuchte Menge. Die Ordnung ist eine `BTreeMap`: Die
/// Iterationsreihenfolge geht in die Prägung ein und muss auf jedem
/// Knoten dieselbe sein.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochClosing {
    epoch: EpochId,
    /// Bestätigte Arbeit je Pod, vor Rückbuchungen.
    bestaetigt: BTreeMap<PodId, u64>,
    /// Bereits zurückgebuchte Arbeit je Pod.
    zurueckgebucht: BTreeMap<PodId, u64>,
    /// Segmente, deren Widerlegung schon verbucht ist.
    widerlegt: BTreeSet<SegmentId>,
    /// Ansprüche, die mangels Bestätigung verfielen — für die
    /// Nachvollziehbarkeit, nicht für die Prägung.
    verfallen: BTreeMap<PodId, u64>,
}

impl EpochClosing {
    /// Die abgeschlossene Epoche.
    pub fn epoch(&self) -> EpochId {
        self.epoch
    }

    /// Bestätigte Arbeit eines Pods **nach** Rückbuchungen.
    ///
    /// Sättigend bei 0: Eine Rückbuchung kann höchstens den Anspruch
    /// aufzehren. Ein negativer Saldo wäre eine Gutschrift an alle
    /// anderen.
    pub fn confirmed(&self, pod: &PodId) -> u64 {
        let brutto = self.bestaetigt.get(pod).copied().unwrap_or(0);
        let ab = self.zurueckgebucht.get(pod).copied().unwrap_or(0);
        brutto.saturating_sub(ab)
    }

    /// Alle Pods mit bestätigter Arbeit, in kanonischer Reihenfolge.
    pub fn pods(&self) -> impl Iterator<Item = (&PodId, u64)> {
        self.bestaetigt.keys().map(move |p| (p, self.confirmed(p)))
    }

    /// Summe der bestätigten Arbeit nach Rückbuchungen.
    ///
    /// Sättigend statt umlaufend: ein Überlauf würde die Prägemenge auf
    /// einen kleinen Wert zurückspringen lassen.
    pub fn total_confirmed(&self) -> u64 {
        self.bestaetigt
            .keys()
            .fold(0u64, |acc, p| acc.saturating_add(self.confirmed(p)))
    }

    /// Arbeit, die beansprucht, aber mangels Stufe-1-Bestätigung nicht
    /// gutgeschrieben wurde. Für Diagnose und Protokoll.
    pub fn forfeited(&self, pod: &PodId) -> u64 {
        self.verfallen.get(pod).copied().unwrap_or(0)
    }

    /// Summe der verfallenen Ansprüche.
    pub fn total_forfeited(&self) -> u64 {
        self.verfallen
            .values()
            .fold(0u64, |acc, v| acc.saturating_add(*v))
    }

    /// Bucht die Arbeit eines widerlegten Segments zurück.
    ///
    /// Wird innerhalb der Streitfrist aufgerufen, wenn ein Schiedsspruch
    /// ein Segment widerlegt (`myl_ledger::apply_verdict` liefert den
    /// Stake-Teil, dies ist der Arbeits-Teil).
    ///
    /// **Idempotent über die Segment-Id.** Dieselbe Widerlegung zweimal
    /// einzureichen ist ein Fehler, keine zweite Rückbuchung — sonst
    /// ließe sich ein ehrlicher Pod durch Wiederholung auf null bringen.
    pub fn apply_clawback(&mut self, refuted: &RefutedSegment) -> Result<(), EpochError> {
        if !self.bestaetigt.contains_key(&refuted.pod) {
            return Err(EpochError::UnknownPod { pod: refuted.pod });
        }
        if self.widerlegt.contains(&refuted.segment_id) {
            return Err(EpochError::DuplicateRefutation {
                segment_id: refuted.segment_id,
            });
        }
        self.widerlegt.insert(refuted.segment_id);
        let eintrag = self.zurueckgebucht.entry(refuted.pod).or_insert(0);
        *eintrag = eintrag.saturating_add(refuted.vtfe);
        Ok(())
    }

    /// Ist der Abschluss endgültig, d. h. die Streitfrist abgelaufen?
    ///
    /// Vor diesem Zeitpunkt ist die bestätigte Menge vorläufig; danach
    /// werden keine Rückbuchungen mehr angenommen (die Durchsetzung
    /// dieser Regel gehört in den Aufrufer, der die Epochenzeit kennt).
    pub fn is_final(&self, current_epoch: EpochId, dispute_epochs: u64) -> bool {
        current_epoch.0 >= self.epoch.0.saturating_add(dispute_epochs)
    }

    /// Anzahl der Pods mit bestätigter Arbeit.
    pub fn len(&self) -> usize {
        self.bestaetigt.len()
    }

    /// Hat kein Pod bestätigte Arbeit?
    pub fn is_empty(&self) -> bool {
        self.bestaetigt.is_empty()
    }
}

/// Schließt eine Epoche ab.
///
/// Geht die angenommenen Bündel der Epoche durch und schreibt jedem Pod
/// die beanspruchte Arbeit gut, **dessen Stufe-1-Vergleich positiv
/// ausgefallen ist**. Alles andere verfällt.
///
/// **Parameter:**
/// - `registry`: die angenommenen Bündel (aus [`crate::poi`])
/// - `epoch`: die abzuschließende Epoche
/// - `agreement`: Stufe-1-Ergebnis je Pod. Fehlt ein Eintrag, gilt
///   [`PodAgreement::Missing`] — also keine Gutschrift.
///
/// **Warum eine Abbildung und keine Rückruffunktion:** Der Abschluss
/// muss auf jedem Knoten aus denselben Eingaben denselben Wert ergeben.
/// Eine Abbildung ist ein Datum, das mit dem Block reisen und geprüft
/// werden kann; eine Rückruffunktion wäre knotenlokales Verhalten.
pub fn close_epoch(
    registry: &PoIRegistry,
    epoch: EpochId,
    agreement: &BTreeMap<PodId, PodAgreement>,
) -> EpochClosing {
    let mut bestaetigt = BTreeMap::new();
    let mut verfallen = BTreeMap::new();

    for bundle in registry.bundles_of_epoch(epoch) {
        let urteil = agreement
            .get(&bundle.pod)
            .copied()
            .unwrap_or(PodAgreement::Missing);
        if urteil.berechtigt() {
            bestaetigt.insert(bundle.pod, bundle.vtfe_claimed);
        } else {
            verfallen.insert(bundle.pod, bundle.vtfe_claimed);
        }
    }

    EpochClosing {
        epoch,
        bestaetigt,
        zurueckgebucht: BTreeMap::new(),
        widerlegt: BTreeSet::new(),
        verfallen,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poi::{PodMembership, poi_bundle_message};
    use myl_types::bls::{
        BlsProofOfPossession, BlsPublicKey, BlsSecretKey, BlsSignature, aggregate_signatures,
    };
    use myl_types::core_types::PoIBundle;
    use myl_types::ids::{MerkleRoot, MinerId};

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }
    fn pod(b: u8) -> PodId {
        PodId::new([b; 32])
    }
    fn segment(b: u8) -> SegmentId {
        SegmentId::new([b; 32])
    }
    fn root(b: u8) -> MerkleRoot {
        MerkleRoot::new([b; 32])
    }

    fn mitglied(b: u8) -> (MinerId, BlsPublicKey, BlsProofOfPossession) {
        let sk = BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("key_gen");
        (
            miner(b),
            sk.public_key().expect("pk"),
            sk.prove_possession().expect("pop"),
        )
    }

    /// Registry mit je einem gültigen Bündel für die Pods in `pods`.
    fn registry_mit(epoch: u64, pods: &[(u8, u64)]) -> PoIRegistry {
        let mut reg = PoIRegistry::new();
        for &(p, vtfe) in pods {
            let members: Vec<_> = (0..3u8).map(mitglied).collect();
            let m = PodMembership::new(EpochId(epoch), pod(p), miner(0), members).expect("m");
            let segments_root = root(9);
            let msg = poi_bundle_message(EpochId(epoch), pod(p), &segments_root, vtfe);
            let sigs: Vec<BlsSignature> = (0..3u8)
                .map(|i| {
                    BlsSecretKey::key_gen(&[i.wrapping_add(1); 32])
                        .expect("key_gen")
                        .sign(&msg)
                        .expect("sign")
                })
                .collect();
            let agg = aggregate_signatures(&sigs).expect("agg");
            let bundle = PoIBundle {
                epoch: EpochId(epoch),
                pod: pod(p),
                segments_root,
                vtfe_claimed: vtfe,
                aggregate_sig: BlsSignature(agg.0),
            };
            reg.submit(&bundle, &m, &miner(0), EpochId(epoch))
                .expect("submit");
        }
        reg
    }

    fn urteile(paare: &[(u8, PodAgreement)]) -> BTreeMap<PodId, PodAgreement> {
        paare.iter().map(|&(p, a)| (pod(p), a)).collect()
    }

    // ── Grundfall ───────────────────────────────────────────────────

    #[test]
    fn bestaetigte_arbeit_wird_gutgeschrieben() {
        let reg = registry_mit(3, &[(1, 1_000), (2, 500)]);
        let a = urteile(&[(1, PodAgreement::Match), (2, PodAgreement::Match)]);
        let abschluss = close_epoch(&reg, EpochId(3), &a);
        assert_eq!(abschluss.confirmed(&pod(1)), 1_000);
        assert_eq!(abschluss.confirmed(&pod(2)), 500);
        assert_eq!(abschluss.total_confirmed(), 1_500);
        assert_eq!(abschluss.total_forfeited(), 0);
    }

    // ── Grundeinstellung Ablehnung ──────────────────────────────────

    #[test]
    fn abweichung_bringt_keine_gutschrift() {
        let reg = registry_mit(3, &[(1, 1_000)]);
        let a = urteile(&[(1, PodAgreement::Mismatch)]);
        let abschluss = close_epoch(&reg, EpochId(3), &a);
        assert_eq!(abschluss.confirmed(&pod(1)), 0);
        assert_eq!(abschluss.forfeited(&pod(1)), 1_000);
        assert_eq!(abschluss.total_confirmed(), 0);
    }

    #[test]
    fn fehlender_vergleich_bringt_keine_gutschrift() {
        // Der wichtigste Fall: Waere ein fehlendes Ergebnis wie
        // Uebereinstimmung, waere „Redundanzpartner unerreichbar machen"
        // eine Strategie — der Angreifer beseitigt den Zeugen und wird
        // fuer die fehlende Aussage bezahlt.
        let reg = registry_mit(3, &[(1, 1_000)]);
        for urteil in [Some(PodAgreement::Missing), None] {
            let a = match urteil {
                Some(u) => urteile(&[(1, u)]),
                None => BTreeMap::new(),
            };
            let abschluss = close_epoch(&reg, EpochId(3), &a);
            assert_eq!(abschluss.confirmed(&pod(1)), 0, "{:?}", urteil);
            assert_eq!(abschluss.forfeited(&pod(1)), 1_000);
        }
    }

    #[test]
    fn nur_match_berechtigt() {
        assert!(PodAgreement::Match.berechtigt());
        assert!(!PodAgreement::Mismatch.berechtigt());
        assert!(!PodAgreement::Missing.berechtigt());
    }

    #[test]
    fn urteil_ueber_fremden_pod_schafft_keinen_anspruch() {
        // Ein Match fuer einen Pod ohne Buendel darf nichts erzeugen.
        let reg = registry_mit(3, &[(1, 1_000)]);
        let a = urteile(&[(1, PodAgreement::Match), (7, PodAgreement::Match)]);
        let abschluss = close_epoch(&reg, EpochId(3), &a);
        assert_eq!(abschluss.len(), 1);
        assert_eq!(abschluss.confirmed(&pod(7)), 0);
    }

    #[test]
    fn andere_epoche_bleibt_unberuehrt() {
        let reg = registry_mit(3, &[(1, 1_000)]);
        let a = urteile(&[(1, PodAgreement::Match)]);
        let abschluss = close_epoch(&reg, EpochId(4), &a);
        assert!(abschluss.is_empty());
        assert_eq!(abschluss.total_confirmed(), 0);
    }

    // ── Rückbuchung ─────────────────────────────────────────────────

    #[test]
    fn widerlegtes_segment_wird_zurueckgebucht() {
        let reg = registry_mit(3, &[(1, 1_000)]);
        let mut abschluss = close_epoch(&reg, EpochId(3), &urteile(&[(1, PodAgreement::Match)]));
        abschluss
            .apply_clawback(&RefutedSegment {
                segment_id: segment(1),
                pod: pod(1),
                vtfe: 300,
            })
            .expect("clawback");
        assert_eq!(abschluss.confirmed(&pod(1)), 700);
        assert_eq!(abschluss.total_confirmed(), 700);
    }

    #[test]
    fn rueckbuchung_ist_idempotent_ueber_die_segment_id() {
        // Ohne Segment-Bindung liesse sich ein ehrlicher Pod durch
        // Wiederholung derselben Widerlegung auf null bringen.
        let reg = registry_mit(3, &[(1, 1_000)]);
        let mut abschluss = close_epoch(&reg, EpochId(3), &urteile(&[(1, PodAgreement::Match)]));
        let r = RefutedSegment {
            segment_id: segment(1),
            pod: pod(1),
            vtfe: 300,
        };
        abschluss.apply_clawback(&r).expect("erste");
        assert_eq!(
            abschluss.apply_clawback(&r).unwrap_err(),
            EpochError::DuplicateRefutation {
                segment_id: segment(1)
            }
        );
        assert_eq!(abschluss.confirmed(&pod(1)), 700);
    }

    #[test]
    fn rueckbuchung_kann_nicht_ins_minus_laufen() {
        // Ein negativer Saldo waere eine Gutschrift an alle anderen.
        let reg = registry_mit(3, &[(1, 1_000)]);
        let mut abschluss = close_epoch(&reg, EpochId(3), &urteile(&[(1, PodAgreement::Match)]));
        for (i, betrag) in [(1u8, 800u64), (2, 800)] {
            abschluss
                .apply_clawback(&RefutedSegment {
                    segment_id: segment(i),
                    pod: pod(1),
                    vtfe: betrag,
                })
                .expect("clawback");
        }
        assert_eq!(abschluss.confirmed(&pod(1)), 0);
        assert_eq!(abschluss.total_confirmed(), 0);
    }

    #[test]
    fn rueckbuchung_fuer_unbekannten_pod_wird_abgelehnt() {
        // Nicht stillschweigend verwerfen: deutet auf inkonsistente
        // Eingaben hin und gehoert sichtbar gemacht.
        let reg = registry_mit(3, &[(1, 1_000)]);
        let mut abschluss = close_epoch(&reg, EpochId(3), &urteile(&[(1, PodAgreement::Match)]));
        assert_eq!(
            abschluss
                .apply_clawback(&RefutedSegment {
                    segment_id: segment(1),
                    pod: pod(9),
                    vtfe: 100,
                })
                .unwrap_err(),
            EpochError::UnknownPod { pod: pod(9) }
        );
    }

    #[test]
    fn rueckbuchung_beruehrt_nur_den_eigenen_pod() {
        let reg = registry_mit(3, &[(1, 1_000), (2, 1_000)]);
        let mut abschluss = close_epoch(
            &reg,
            EpochId(3),
            &urteile(&[(1, PodAgreement::Match), (2, PodAgreement::Match)]),
        );
        abschluss
            .apply_clawback(&RefutedSegment {
                segment_id: segment(1),
                pod: pod(1),
                vtfe: 400,
            })
            .expect("clawback");
        assert_eq!(abschluss.confirmed(&pod(1)), 600);
        assert_eq!(abschluss.confirmed(&pod(2)), 1_000);
        assert_eq!(abschluss.total_confirmed(), 1_600);
    }

    // ── Streitfrist ─────────────────────────────────────────────────

    #[test]
    fn streitfrist_laeuft_ueber_epochen() {
        let reg = registry_mit(3, &[(1, 1_000)]);
        let abschluss = close_epoch(&reg, EpochId(3), &urteile(&[(1, PodAgreement::Match)]));
        // Gegen die Konstante gerechnet, nicht gegen getippte Zahlen:
        // Als sich die Frist mit Fund 50 von 7 auf 168 korrigierte,
        // schlugen die Literale fehl, ohne dass an der geprüften Regel
        // etwas falsch war.
        let d = DEFAULT_DISPUTE_EPOCHS;
        assert!(!abschluss.is_final(EpochId(3), d));
        assert!(!abschluss.is_final(EpochId(3 + d - 1), d));
        assert!(abschluss.is_final(EpochId(3 + d), d));
        assert!(abschluss.is_final(EpochId(3 + d + 1), d));
    }

    #[test]
    fn streitfrist_saettigt_bei_ueberlauf() {
        let reg = registry_mit(3, &[(1, 1_000)]);
        let abschluss = close_epoch(&reg, EpochId(3), &urteile(&[(1, PodAgreement::Match)]));
        assert!(!abschluss.is_final(EpochId(u64::MAX - 1), u64::MAX));
    }

    // ── Determinismus ───────────────────────────────────────────────

    #[test]
    fn abschluss_ist_deterministisch_und_kanonisch_geordnet() {
        // Die Iterationsreihenfolge geht in die Praegung ein und muss auf
        // jedem Knoten dieselbe sein, unabhaengig von der Eingangsfolge.
        let bauen = |reihenfolge: &[(u8, u64)]| {
            let reg = registry_mit(3, reihenfolge);
            let a: BTreeMap<PodId, PodAgreement> = reihenfolge
                .iter()
                .map(|&(p, _)| (pod(p), PodAgreement::Match))
                .collect();
            let abschluss = close_epoch(&reg, EpochId(3), &a);
            abschluss.pods().map(|(p, v)| (*p, v)).collect::<Vec<_>>()
        };
        let a = bauen(&[(3, 300), (1, 100), (2, 200)]);
        let b = bauen(&[(1, 100), (2, 200), (3, 300)]);
        assert_eq!(a, b);
        assert_eq!(a, vec![(pod(1), 100), (pod(2), 200), (pod(3), 300)]);
    }

    #[test]
    fn zweiter_abschlusslauf_ergibt_denselben_wert() {
        // Der Abschluss ist eine Funktion der Eingaben, kein Akkumulator
        // — ein zweiter Lauf darf nicht doppelt gutschreiben.
        let reg = registry_mit(3, &[(1, 1_000)]);
        let a = urteile(&[(1, PodAgreement::Match)]);
        assert_eq!(
            close_epoch(&reg, EpochId(3), &a),
            close_epoch(&reg, EpochId(3), &a)
        );
    }

    #[test]
    fn summe_saettigt_statt_umzulaufen() {
        let reg = registry_mit(3, &[(1, u64::MAX), (2, 1_000)]);
        let abschluss = close_epoch(
            &reg,
            EpochId(3),
            &urteile(&[(1, PodAgreement::Match), (2, PodAgreement::Match)]),
        );
        assert_eq!(abschluss.total_confirmed(), u64::MAX);
    }
}
