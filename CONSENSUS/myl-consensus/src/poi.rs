//! PoI-Bündel-Einreichung — Whitepaper Kap. 4.4, Anhang A.1 (Punkt 4.1).
//!
//! Prozess B der Konsensarchitektur (Kap. 3.5.2): Ein Pod-Koordinator
//! reicht am Epochenende ein [`PoIBundle`] ein, das die geleistete
//! Inferenzarbeit seines Pods beansprucht. Der Konsens muss das Bündel
//! annehmen oder ablehnen, **bevor** daraus geprägt wird.
//!
//! ## Die eine Regel, an der alles hängt
//!
//! Die Unterzeichnermenge kommt aus der **Pod-Zuteilung des
//! Schedulers**, niemals aus dem Bündel selbst.
//!
//! Das klingt selbstverständlich und ist der Punkt, an dem sich
//! Aggregat-Signaturen still aushebeln lassen. `FastAggregateVerify`
//! prüft ein Aggregat gegen eine Liste öffentlicher Schlüssel. Nimmt man
//! diese Liste aus dem eingereichten Objekt, prüft man nur noch: „haben
//! die, die unterschrieben haben, unterschrieben?" — eine Tautologie.
//! Ein Pod aus fünf Mitgliedern könnte mit der Signatur eines einzigen
//! einreichen, und die Prüfung ginge durch.
//!
//! Deshalb nimmt [`verify_bundle_signature`] die Schlüssel aus
//! [`PodMembership`], das vom Scheduler stammt (Anhang A.2), und prüft
//! gegen **alle** Mitglieder. Fehlt die Signatur eines einzigen, schlägt
//! die Aggregat-Prüfung fehl — genau das Akzeptanzkriterium der Phase.
//!
//! ## Was die Signierbotschaft binden muss
//!
//! [`poi_bundle_message`] bindet `(epoch, pod, segments_root,
//! vtfe_claimed)`. Der letzte Teil ist der wichtige: Stünde die
//! beanspruchte Arbeitsmenge nicht in der Botschaft, könnte der
//! Koordinator sie nach dem Einsammeln der Signaturen beliebig
//! hochsetzen. Die Mitglieder hätten dann eine Arbeitsmenge bestätigt,
//! die sie nie gesehen haben, und das Aggregat wäre trotzdem gültig.
//!
//! ## Was dieses Modul **nicht** prüft
//!
//! Ob `vtfe_claimed` der Wahrheit entspricht. Dieses Modul stellt fest,
//! dass der Pod die beanspruchte Menge **geschlossen bestätigt hat** —
//! nicht, dass sie stimmt. Die Bestätigung der tatsächlich geleisteten
//! Arbeit ist Punkt 4.2 (Epochenabschluss nach Stufe-1-Übereinstimmung,
//! abzüglich später widerlegter Segmente).
//!
//! **Was eine vTFE-Einheit zählt, ist seit dem 2026-08-23 festgelegt**
//! (`myl_tokenomics::vtfe`): der Anteil eines Shards an den
//! Multiplikations-Additionen der Gewichtsmatrizen eines vollen
//! Vorwärtspasses, mal der Zahl der erzeugten Token. Die Regel folgt aus
//! `model_config.json` und ist damit über `theta_v_hash` gebunden, also
//! von jedem Prüfer nachrechenbar, ohne den Zustand einer Anfrage zu
//! kennen. Für dieses Modul ändert sich nichts: `vtfe_claimed` bleibt
//! **Eingabe**, und die Prüfung gegen die Regel gehört zu Punkt 4.2.
//!
//! ## Warum jedes Mitglied einen Besitznachweis braucht (Fund 27)
//!
//! Die Ablehnung eines Bündels mit fehlender Mitgliedssignatur beruht
//! darauf, dass `FastAggregateVerify` nicht ohne die Signatur **aller**
//! aufgeführten Schlüssel gilt. Das stimmt nur, solange kein Schlüssel
//! der Menge ein Rogue Key ist: zu einem fremden `pk_opfer` lässt sich
//! `pk_rogue = g₁^x · pk_opfer⁻¹` bilden, und eine allein vom Angreifer
//! erzeugte Signatur gilt dann als Aggregat beider. Identitäts- und
//! Subgruppen-Prüfung fangen das nicht ab — der Punkt ist völlig regulär.
//!
//! [`PodMembership::new`] verlangt deshalb je Mitglied einen
//! `BlsProofOfPossession`. Wer ihn liefern kann, kennt den diskreten
//! Logarithmus seines Schlüssels; der Erzeuger eines Rogue Keys kann das
//! nicht. Erst damit trägt die Aggregat-Prüfung unten.
//!
//! **Konsens-Feld:** Signierbotschaft und Annahmeregeln sind Teil des
//! Konsensvertrags. Änderungen nur über Governance (Kap. 10.3).

use myl_types::bls::{BlsProofOfPossession, BlsPublicKey, fast_aggregate_verify};
use myl_types::core_types::PoIBundle;
use myl_types::ids::{EpochId, MerkleRoot, MinerId, PodId};
use std::collections::BTreeMap;

/// Domain-Separation-Präfix für PoI-Bündel-Signaturen.
///
/// Eigenes Präfix aus demselben Grund wie im BFT-Protokoll
/// ([`crate::signing`]): ohne Trennung wäre eine Signatur aus einem
/// anderen Zusammenhang unter Umständen als Bündel-Bestätigung
/// wiederverwendbar.
pub const DST_POI_BUNDLE: &[u8] = b"MYELITH_POI_BUNDLE_v1";

/// Kanonische Signierbotschaft eines PoI-Bündels.
///
/// **Aufbau:** `DST_POI_BUNDLE ‖ u64_le(epoch) ‖ pod ‖ segments_root ‖
/// u64_le(vtfe_claimed) ‖ u32_le(segmente)` — feste Feldbreiten in
/// fester Reihenfolge, damit zu einem Bündel genau eine Bytefolge
/// gehört.
///
/// `vtfe_claimed` ist Teil der Botschaft: sonst könnte der Koordinator
/// die beanspruchte Arbeitsmenge nach dem Einsammeln der Signaturen
/// erhöhen, ohne das Aggregat ungültig zu machen.
///
/// ⚑ **`segmente` gehört aus demselben Grund dazu, und der Schaden wäre
/// ein anderer** (Fund 115, 2026-09-01): Wer die Segmentzahl nachträglich
/// erhöht, **verdünnt die Stichprobenwahrscheinlichkeit je Segment**. Aus
/// `p` wird `p/k`, und die Sicherheitsbedingung aus Anhang B.1 hängt
/// genau an `p`.
pub fn poi_bundle_message(
    epoch: EpochId,
    pod: PodId,
    segments_root: &MerkleRoot,
    vtfe_claimed: u64,
    segmente: u32,
) -> Vec<u8> {
    let mut msg = Vec::with_capacity(DST_POI_BUNDLE.len() + 8 + 32 + 32 + 8 + 4);
    msg.extend_from_slice(DST_POI_BUNDLE);
    msg.extend_from_slice(&epoch.0.to_le_bytes());
    msg.extend_from_slice(pod.as_bytes());
    msg.extend_from_slice(segments_root.as_bytes());
    msg.extend_from_slice(&vtfe_claimed.to_le_bytes());
    msg.extend_from_slice(&segmente.to_le_bytes());
    msg
}

/// Signierbotschaft zu einem konkreten Bündel.
pub fn bundle_message(bundle: &PoIBundle) -> Vec<u8> {
    poi_bundle_message(
        bundle.epoch,
        bundle.pod,
        &bundle.segments_root,
        bundle.vtfe_claimed,
        bundle.segmente,
    )
}

/// Die Mitgliedschaft eines Pods in einer Epoche.
///
/// Kommt aus der Zuteilung des Schedulers (`myl-scheduler`, Anhang A.2)
/// und ist die **maßgebliche** Quelle dafür, wer unterschrieben haben
/// muss. Sie darf nie aus einem eingereichten Bündel abgeleitet werden;
/// die Begründung steht im Modulkopf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodMembership {
    /// Epoche, für die diese Zuteilung gilt.
    pub epoch: EpochId,
    /// Der Pod.
    pub pod: PodId,
    /// Das Mitglied, das einreichen darf.
    pub coordinator: MinerId,
    /// Mitglieder mit ihren öffentlichen Schlüsseln, in Pipeline-Reihenfolge.
    members: Vec<(MinerId, BlsPublicKey)>,
}

impl PodMembership {
    /// Baut eine Mitgliedschaft.
    ///
    /// **Fehler:** [`PoIError::EmptyPod`] bei leerer Mitgliederliste,
    /// [`PoIError::DuplicateMember`] bei doppelter `MinerId`,
    /// [`PoIError::CoordinatorNotMember`], wenn der Koordinator nicht
    /// dazugehört.
    ///
    /// Der Duplikatschutz ist nicht Kosmetik: stünde ein Miner zweimal
    /// in der Liste, ginge sein Schlüssel zweimal in die
    /// Aggregat-Prüfung ein, und ein Pod aus einem einzigen realen
    /// Teilnehmer könnte wie ein größerer aussehen.
    ///
    /// Je Mitglied ist ein `BlsProofOfPossession` zu übergeben und wird
    /// geprüft ([`PoIError::InvalidProofOfPossession`]) — ohne ihn wäre
    /// ein Rogue Key in der Menge, und die Aggregat-Prüfung weiter unten
    /// wertlos (Fund 27, Modulkopf). Der Nachweis wird nicht
    /// gespeichert; er gehört zur Aufnahme, nicht zum Zustand.
    ///
    /// **Anmerkung zum Ort:** Sobald es eine eigene Miner-Registrierung
    /// mit Schlüsseln gibt (`myl-scheduler::MinerRegistration` trägt
    /// heute keine), gehört die Prüfung dorthin — einmal bei der
    /// Registrierung statt bei jeder Pod-Bildung. Bis dahin ist dies die
    /// erste Stelle, an der ein fremder Schlüssel ins Verfahren kommt.
    pub fn new(
        epoch: EpochId,
        pod: PodId,
        coordinator: MinerId,
        members: Vec<(MinerId, BlsPublicKey, BlsProofOfPossession)>,
    ) -> Result<Self, PoIError> {
        if members.is_empty() {
            return Err(PoIError::EmptyPod);
        }
        let mut gesehen: Vec<&MinerId> = members.iter().map(|(m, _, _)| m).collect();
        gesehen.sort();
        if gesehen.windows(2).any(|w| w[0] == w[1]) {
            return Err(PoIError::DuplicateMember);
        }
        if !members.iter().any(|(m, _, _)| *m == coordinator) {
            return Err(PoIError::CoordinatorNotMember);
        }
        for (miner, pubkey, pop) in &members {
            if !pubkey.verify_possession(pop) {
                return Err(PoIError::InvalidProofOfPossession { member: *miner });
            }
        }
        Ok(Self {
            epoch,
            pod,
            coordinator,
            members: members.into_iter().map(|(m, pk, _)| (m, pk)).collect(),
        })
    }

    /// Baut eine Mitgliedschaft aus Schlüsseln, deren Besitz **anderswo**
    /// bewiesen wurde.
    ///
    /// # ⚑ Warum es das gibt, und warum es kein Loch ist
    ///
    /// [`Self::new`] verlangt je Mitglied einen
    /// `BlsProofOfPossession`, damit kein **Rogue Key** in die
    /// Aggregatprüfung gerät: Wer einen Schlüssel als Differenz fremder
    /// Schlüssel bildet, könnte sonst Aggregate fälschen.
    ///
    /// Kommen die Schlüssel aus dem **Miner-Register der Kette**, ist
    /// der Besitz bereits bewiesen, und zwar stärker als durch einen
    /// mitgelieferten Nachweis: Ein Schlüssel gelangt nur über eine
    /// **unterschriebene Anmeldung** hinein, und wer unterschreiben
    /// kann, hält den geheimen Teil. **Ein Rogue Key kommt gar nicht
    /// erst ins Register**, weil sein Bildner mit ihm nicht
    /// unterschreiben kann.
    ///
    /// Die Prüfung ein zweites Mal zu verlangen hieße, je Epoche eine
    /// Paarung je Mitglied zu rechnen, für eine Aussage, die feststeht.
    ///
    /// ⚑ **Wer diese Funktion mit Schlüsseln aus anderer Quelle
    /// aufruft, hebt den Schutz auf.** Sie heißt deshalb, wie sie heißt,
    /// und dieser Absatz steht hier statt in einem Kommentar am
    /// Aufrufer.
    ///
    /// Doppelte Mitglieder und ein Koordinator außerhalb des Pods werden
    /// weiterhin abgewiesen; beides hat mit dem Besitznachweis nichts zu
    /// tun.
    pub fn ohne_besitznachweis(
        epoch: EpochId,
        pod: PodId,
        coordinator: MinerId,
        members: Vec<(MinerId, BlsPublicKey)>,
    ) -> Result<Self, PoIError> {
        if members.is_empty() {
            return Err(PoIError::EmptyPod);
        }
        let mut gesehen: Vec<&MinerId> = members.iter().map(|(m, _)| m).collect();
        gesehen.sort();
        if gesehen.windows(2).any(|w| w[0] == w[1]) {
            return Err(PoIError::DuplicateMember);
        }
        if !members.iter().any(|(m, _)| *m == coordinator) {
            return Err(PoIError::CoordinatorNotMember);
        }
        Ok(Self {
            epoch,
            pod,
            coordinator,
            members,
        })
    }

    /// Anzahl der Mitglieder.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Ist der Pod leer? (Kann nach [`Self::new`] nie zutreffen.)
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Gehört dieser Miner zum Pod?
    pub fn contains(&self, miner: &MinerId) -> bool {
        self.members.iter().any(|(m, _)| m == miner)
    }

    /// Mitglieder in Pipeline-Reihenfolge.
    pub fn members(&self) -> &[(MinerId, BlsPublicKey)] {
        &self.members
    }

    /// Öffentliche Schlüssel **aller** Mitglieder — die Menge, gegen die
    /// das Aggregat geprüft wird.
    pub fn pubkeys(&self) -> Vec<BlsPublicKey> {
        self.members.iter().map(|(_, pk)| *pk).collect()
    }

    /// Der Schlüssel eines einzelnen Mitglieds.
    ///
    /// # ⚑ Wozu, wenn es [`Self::pubkeys`] schon gibt (2026-08-30)
    ///
    /// Für die Aggregat-Prüfung braucht es alle Schlüssel; für einen
    /// **einzelnen** Beleg braucht es einen. Der Fall ist der
    /// Anfechtungsbeleg: Wer einen Herausforderer schlachten will, muss
    /// dessen Unterschrift prüfen, und dafür dessen Schlüssel finden.
    ///
    /// **Bis hierher galt das als offener Punkt** („es fehlt eine
    /// Registrierung Miner zu Schlüssel"). Für den Streitpfad fehlt sie
    /// nicht: Ein Herausforderer ist Mitglied des redundanten Pods, und
    /// diese Zuteilung führt die Schlüssel ihrer Mitglieder ohnehin
    /// mit. Eine zweite, globale Registrierung wäre eine zweite Quelle
    /// für dieselbe Zuordnung.
    ///
    /// **Was damit nicht gelöst ist:** die Prüfung im Gossip-Pfad. Dort
    /// kennt der Knoten die Pod-Zuteilung eines fremden Segments nicht
    /// und darf sie nicht raten; deshalb geht dort ein unbekannter
    /// Absender weiterhin durch, und geurteilt wird erst hier.
    pub fn pubkey(&self, wer: &MinerId) -> Option<&BlsPublicKey> {
        self.members.iter().find(|(m, _)| m == wer).map(|(_, pk)| pk)
    }
}

/// Fehler bei der Einreichung eines PoI-Bündels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoIError {
    /// Die Mitgliederliste ist leer.
    EmptyPod,
    /// Ein Miner steht mehrfach in der Mitgliederliste.
    DuplicateMember,
    /// Der Koordinator gehört nicht zum Pod.
    CoordinatorNotMember,
    /// Das Bündel gehört zu einem anderen Pod als die Mitgliedschaft.
    PodMismatch,
    /// Bündel und Mitgliedschaft betreffen verschiedene Epochen.
    EpochMismatch {
        /// Epoche des Bündels.
        bundle: u64,
        /// Epoche der Mitgliedschaft.
        membership: u64,
    },
    /// Das Bündel gehört nicht zur abzuschließenden Epoche.
    WrongEpoch {
        /// Erwartete Epoche.
        expected: u64,
        /// Epoche des Bündels.
        got: u64,
    },
    /// Der Einreichende ist nicht der Koordinator dieses Pods.
    NotCoordinator,
    /// Für dieses Paar `(Epoche, Pod)` liegt bereits ein Bündel vor.
    DuplicateSubmission,
    /// Das Aggregat gilt nicht unter den Schlüsseln **aller** Mitglieder.
    /// Tritt insbesondere auf, wenn die Signatur eines Mitglieds fehlt.
    InvalidAggregateSignature,
    /// Beanspruchte Arbeit von 0 — ein Pod ohne Arbeit hat nichts
    /// einzureichen.
    EmptyClaim,
    /// Der Besitznachweis eines Mitglieds fehlt oder gilt nicht. Ohne
    /// ihn wäre ein Rogue Key im Pod (Fund 27).
    InvalidProofOfPossession {
        /// Das Mitglied, dessen Nachweis nicht gilt.
        member: MinerId,
    },
}

impl std::fmt::Display for PoIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPod => write!(f, "Pod ohne Mitglieder"),
            Self::DuplicateMember => write!(f, "Miner steht mehrfach im Pod"),
            Self::CoordinatorNotMember => {
                write!(f, "Koordinator gehört nicht zum Pod")
            }
            Self::PodMismatch => write!(f, "Bündel gehört zu einem anderen Pod"),
            Self::EpochMismatch { bundle, membership } => write!(
                f,
                "Bündel-Epoche {} passt nicht zur Zuteilungs-Epoche {}",
                bundle, membership
            ),
            Self::WrongEpoch { expected, got } => {
                write!(f, "Erwartete Epoche {}, bekommen {}", expected, got)
            }
            Self::NotCoordinator => {
                write!(f, "Einreichender ist nicht der Pod-Koordinator")
            }
            Self::DuplicateSubmission => {
                write!(f, "Für dieses Paar (Epoche, Pod) liegt bereits ein Bündel vor")
            }
            Self::InvalidAggregateSignature => write!(
                f,
                "Aggregat gilt nicht unter den Schlüsseln aller Pod-Mitglieder"
            ),
            Self::EmptyClaim => write!(f, "Beanspruchte Arbeit ist 0"),
            Self::InvalidProofOfPossession { member } => write!(
                f,
                "Besitznachweis von Mitglied {} fehlt oder gilt nicht",
                member
            ),
        }
    }
}

impl std::error::Error for PoIError {}

/// Prüft das Aggregat eines Bündels gegen **alle** Pod-Mitglieder.
///
/// Vor der teuren Aggregat-Verifikation stehen die billigen
/// Zuordnungsprüfungen (Pod, Epoche) — dieselbe Reihenfolge wie im
/// BFT-Protokoll, damit die Kryptografie keine DoS-Fläche wird.
///
/// **Fehlt die Signatur eines Mitglieds**, ist das Aggregat unter der
/// vollständigen Schlüsselmenge ungültig und die Prüfung schlägt fehl.
/// Das ist kein Nebeneffekt, sondern die tragende Eigenschaft: das
/// Akzeptanzkriterium der Phase verlangt genau diese Ablehnung.
pub fn verify_bundle_signature(
    bundle: &PoIBundle,
    membership: &PodMembership,
) -> Result<(), PoIError> {
    if bundle.pod != membership.pod {
        return Err(PoIError::PodMismatch);
    }
    if bundle.epoch != membership.epoch {
        return Err(PoIError::EpochMismatch {
            bundle: bundle.epoch.0,
            membership: membership.epoch.0,
        });
    }

    let msg = bundle_message(bundle);
    let pubkeys = membership.pubkeys();
    if !fast_aggregate_verify(&pubkeys, &msg, &bundle.aggregate_sig.as_aggregate()) {
        return Err(PoIError::InvalidAggregateSignature);
    }
    Ok(())
}

/// Sammelstelle der in einer Epoche eingereichten PoI-Bündel.
///
/// Hält je Paar `(Epoche, Pod)` höchstens ein Bündel. Die Doppel-Sperre
/// ist eine Konsensregel, keine Aufräumhilfe: ohne sie könnte ein
/// Koordinator dieselbe Arbeit mehrfach einreichen und mehrfach prägen
/// lassen.
///
/// Die Ordnung ist eine `BTreeMap` — die Iterationsreihenfolge geht in
/// den Epochenabschluss ein und muss auf jedem Knoten dieselbe sein.
#[derive(Debug, Clone, Default)]
pub struct PoIRegistry {
    accepted: BTreeMap<(u64, PodId), PoIBundle>,
}

impl PoIRegistry {
    /// Leere Sammelstelle.
    pub fn new() -> Self {
        Self::default()
    }

    /// Nimmt ein Bündel entgegen.
    ///
    /// **Prüfkette,** billig vor teuer:
    /// 1. beanspruchte Arbeit > 0,
    /// 2. Bündel gehört zur abzuschließenden Epoche,
    /// 3. Einreichender ist der Koordinator dieses Pods,
    /// 4. noch kein Bündel für `(Epoche, Pod)`,
    /// 5. Aggregat gültig unter **allen** Mitgliedsschlüsseln.
    ///
    /// **Parameter:**
    /// - `bundle`: das eingereichte Bündel
    /// - `membership`: Pod-Zuteilung des Schedulers (maßgeblich)
    /// - `submitter`: wer einreicht
    /// - `current_epoch`: die Epoche, die abgeschlossen wird
    ///
    /// Zu Schritt 1: Ein Bündel über 0 vTFE beansprucht nichts und
    /// belegt trotzdem Blockplatz. Die Ablehnung ist eine Spam-Regel und
    /// könnte per Governance gelockert werden, ohne die Sicherheit zu
    /// berühren.
    pub fn submit(
        &mut self,
        bundle: &PoIBundle,
        membership: &PodMembership,
        submitter: &MinerId,
        current_epoch: EpochId,
    ) -> Result<(), PoIError> {
        if bundle.vtfe_claimed == 0 {
            return Err(PoIError::EmptyClaim);
        }
        if bundle.epoch != current_epoch {
            return Err(PoIError::WrongEpoch {
                expected: current_epoch.0,
                got: bundle.epoch.0,
            });
        }
        if *submitter != membership.coordinator {
            return Err(PoIError::NotCoordinator);
        }
        let key = (bundle.epoch.0, bundle.pod);
        if self.accepted.contains_key(&key) {
            return Err(PoIError::DuplicateSubmission);
        }

        verify_bundle_signature(bundle, membership)?;

        self.accepted.insert(key, bundle.clone());
        Ok(())
    }

    /// Das angenommene Bündel eines Pods, falls vorhanden.
    pub fn get(&self, epoch: EpochId, pod: PodId) -> Option<&PoIBundle> {
        self.accepted.get(&(epoch.0, pod))
    }

    /// Alle angenommenen Bündel einer Epoche, in kanonischer
    /// Pod-Reihenfolge.
    pub fn bundles_of_epoch(&self, epoch: EpochId) -> Vec<&PoIBundle> {
        self.accepted
            .range((epoch.0, PodId::new([0u8; 32]))..=(epoch.0, PodId::new([0xffu8; 32])))
            .map(|(_, b)| b)
            .collect()
    }

    /// Summe der beanspruchten Arbeit einer Epoche.
    ///
    /// **Das ist die beanspruchte, nicht die bestätigte Menge.** Die
    /// Bestätigung (Stufe-1-Übereinstimmung, Abzug widerlegter Segmente)
    /// ist Punkt 4.2. Gesättigt statt umlaufend: ein Überlauf würde die
    /// Prägemenge auf einen kleinen Wert zurückspringen lassen.
    pub fn claimed_vtfe_of_epoch(&self, epoch: EpochId) -> u64 {
        self.bundles_of_epoch(epoch)
            .iter()
            .fold(0u64, |acc, b| acc.saturating_add(b.vtfe_claimed))
    }

    /// Anzahl angenommener Bündel insgesamt.
    pub fn len(&self) -> usize {
        self.accepted.len()
    }

    /// Ist noch nichts angenommen?
    pub fn is_empty(&self) -> bool {
        self.accepted.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::bls::{BlsSecretKey, BlsSignature, aggregate_signatures};

    fn miner(byte: u8) -> MinerId {
        MinerId::new([byte; 32])
    }

    fn pod(byte: u8) -> PodId {
        PodId::new([byte; 32])
    }

    fn root(byte: u8) -> MerkleRoot {
        MerkleRoot::new([byte; 32])
    }

    fn keypair(byte: u8) -> (BlsSecretKey, BlsPublicKey) {
        let sk = BlsSecretKey::key_gen(&[byte.wrapping_add(1); 32]).expect("key_gen");
        let pk = sk.public_key().expect("public_key");
        (sk, pk)
    }

    /// Mitgliedseintrag samt Besitznachweis (Fund 27).
    fn mitglied(byte: u8) -> (MinerId, BlsPublicKey, BlsProofOfPossession) {
        let (sk, pk) = keypair(byte);
        (miner(byte), pk, sk.prove_possession().expect("pop"))
    }

    /// Pod aus `n` Mitgliedern; Koordinator ist Mitglied 0.
    fn membership(n: u8, epoch: u64) -> PodMembership {
        let members: Vec<(MinerId, BlsPublicKey, BlsProofOfPossession)> =
            (0..n).map(mitglied).collect();
        PodMembership::new(EpochId(epoch), pod(1), miner(0), members).expect("Mitgliedschaft")
    }

    /// Bündel, unterschrieben von den Mitgliedern in `signer`.
    fn bundle_signed_by(signer: &[u8], epoch: u64, vtfe: u64) -> PoIBundle {
        let segments_root = root(9);
        let msg = poi_bundle_message(EpochId(epoch), pod(1), &segments_root, vtfe, 1);
        let sigs: Vec<BlsSignature> = signer
            .iter()
            .map(|&i| keypair(i).0.sign(&msg).expect("sign"))
            .collect();
        let agg = aggregate_signatures(&sigs).expect("aggregate");
        PoIBundle {
            epoch: EpochId(epoch),
            pod: pod(1),
            segments_root,
            vtfe_claimed: vtfe,
            aggregate_sig: BlsSignature(agg.0),
            segmente: 1,
        }
    }

    /// Vollständig unterschriebenes Bündel eines Pods aus `n` Mitgliedern.
    fn gueltiges_bundle(n: u8, epoch: u64, vtfe: u64) -> PoIBundle {
        let alle: Vec<u8> = (0..n).collect();
        bundle_signed_by(&alle, epoch, vtfe)
    }

    // ── Signierbotschaft ────────────────────────────────────────────

    #[test]
    fn botschaft_ist_deterministisch_und_hat_erwartete_laenge() {
        let m = poi_bundle_message(EpochId(7), pod(1), &root(9), 1_000, 1);
        assert_eq!(m, poi_bundle_message(EpochId(7), pod(1), &root(9), 1_000, 1));
        assert_eq!(m.len(), DST_POI_BUNDLE.len() + 8 + 32 + 32 + 8 + 4);
    }

    #[test]
    fn vtfe_bindet_die_botschaft() {
        // Der wichtigste Teil der Bindung: ohne ihn koennte der
        // Koordinator die beanspruchte Menge nach dem Einsammeln der
        // Signaturen hochsetzen.
        assert_ne!(
            poi_bundle_message(EpochId(7), pod(1), &root(9), 1_000, 1),
            poi_bundle_message(EpochId(7), pod(1), &root(9), 1_000_000, 1)
        );
    }

    #[test]
    fn epoche_pod_und_wurzel_binden_die_botschaft() {
        let b = poi_bundle_message(EpochId(7), pod(1), &root(9), 5, 1);
        assert_ne!(b, poi_bundle_message(EpochId(8), pod(1), &root(9), 5, 1));
        assert_ne!(b, poi_bundle_message(EpochId(7), pod(2), &root(9), 5, 1));
        assert_ne!(b, poi_bundle_message(EpochId(7), pod(1), &root(8), 5, 1));
    }

    #[test]
    fn botschaft_ist_domain_getrennt() {
        let m = poi_bundle_message(EpochId(7), pod(1), &root(9), 5, 1);
        assert!(m.starts_with(DST_POI_BUNDLE));
        assert_ne!(DST_POI_BUNDLE, crate::signing::DST_VOTE);
        assert_ne!(DST_POI_BUNDLE, crate::signing::DST_COMMIT);
    }

    // ── Mitgliedschaft ──────────────────────────────────────────────

    #[test]
    fn leerer_pod_wird_abgelehnt() {
        let r = PodMembership::new(EpochId(1), pod(1), miner(0), vec![]);
        assert_eq!(r.unwrap_err(), PoIError::EmptyPod);
    }

    #[test]
    fn doppeltes_mitglied_wird_abgelehnt() {
        // Sonst ginge ein Schluessel zweimal in die Aggregat-Pruefung
        // ein und ein Pod aus einem realen Teilnehmer saehe groesser aus.
        let r = PodMembership::new(
            EpochId(1),
            pod(1),
            miner(0),
            vec![mitglied(0), mitglied(0)],
        );
        assert_eq!(r.unwrap_err(), PoIError::DuplicateMember);
    }

    #[test]
    fn koordinator_muss_mitglied_sein() {
        let r = PodMembership::new(EpochId(1), pod(1), miner(7), vec![mitglied(0)]);
        assert_eq!(r.unwrap_err(), PoIError::CoordinatorNotMember);
    }

    #[test]
    fn mitglied_ohne_gueltigen_besitznachweis_wird_abgelehnt() {
        // Fund 27: ohne diese Pruefung koennte ein Rogue Key in den Pod,
        // und die Aggregat-Pruefung unten waere wertlos.
        let (_, pk1) = keypair(1);
        let falscher_pop = keypair(2).0.prove_possession().expect("pop");
        let r = PodMembership::new(
            EpochId(1),
            pod(1),
            miner(0),
            vec![mitglied(0), (miner(1), pk1, falscher_pop)],
        );
        assert_eq!(
            r.unwrap_err(),
            PoIError::InvalidProofOfPossession { member: miner(1) }
        );
    }

    // ── Signaturprüfung ─────────────────────────────────────────────

    #[test]
    fn vollstaendig_unterschriebenes_buendel_gilt() {
        let m = membership(5, 3);
        let b = gueltiges_bundle(5, 3, 1_000);
        assert!(verify_bundle_signature(&b, &m).is_ok());
        // ⚑ Und der Einzelzugriff, den der Anfechtungsbeleg braucht.
        let erstes = m.members()[0].0;
        assert!(m.pubkey(&erstes).is_some(), "ein Mitglied muss auffindbar sein");
        assert!(
            m.pubkey(&MinerId::new([200u8; 32])).is_none(),
            "ein Fremder darf nicht auffindbar sein"
        );
    }

    #[test]
    fn fehlende_signatur_eines_mitglieds_wird_abgelehnt() {
        // Das woertliche Akzeptanzkriterium der Phase 4.
        let m = membership(5, 3);
        for fehlend in 0..5u8 {
            let signer: Vec<u8> = (0..5u8).filter(|&i| i != fehlend).collect();
            let b = bundle_signed_by(&signer, 3, 1_000);
            assert_eq!(
                verify_bundle_signature(&b, &m).unwrap_err(),
                PoIError::InvalidAggregateSignature,
                "fehlende Signatur von Mitglied {} muss auffallen",
                fehlend
            );
        }
    }

    #[test]
    fn einzelne_signatur_statt_aller_wird_abgelehnt() {
        // Der Angriff, gegen den die Regel „Schluessel aus der Zuteilung,
        // nicht aus dem Buendel" schuetzt.
        let m = membership(5, 3);
        let b = bundle_signed_by(&[0], 3, 1_000);
        assert_eq!(
            verify_bundle_signature(&b, &m).unwrap_err(),
            PoIError::InvalidAggregateSignature
        );
    }

    #[test]
    fn fremde_signatur_statt_eines_mitglieds_wird_abgelehnt() {
        let m = membership(5, 3);
        // Mitglied 4 durch Aussenstehenden 9 ersetzt.
        let b = bundle_signed_by(&[0, 1, 2, 3, 9], 3, 1_000);
        assert_eq!(
            verify_bundle_signature(&b, &m).unwrap_err(),
            PoIError::InvalidAggregateSignature
        );
    }

    #[test]
    fn nachtraeglich_erhoehte_arbeitsmenge_wird_abgelehnt() {
        // Die Signaturen gelten fuer 1.000; der Koordinator traegt
        // 1.000.000 ein.
        let m = membership(5, 3);
        let mut b = gueltiges_bundle(5, 3, 1_000);
        b.vtfe_claimed = 1_000_000;
        assert_eq!(
            verify_bundle_signature(&b, &m).unwrap_err(),
            PoIError::InvalidAggregateSignature
        );
    }

    #[test]
    fn vertauschte_segmentwurzel_wird_abgelehnt() {
        let m = membership(5, 3);
        let mut b = gueltiges_bundle(5, 3, 1_000);
        b.segments_root = root(1);
        assert_eq!(
            verify_bundle_signature(&b, &m).unwrap_err(),
            PoIError::InvalidAggregateSignature
        );
    }

    #[test]
    fn buendel_fuer_anderen_pod_wird_abgelehnt() {
        let m = membership(5, 3);
        let mut b = gueltiges_bundle(5, 3, 1_000);
        b.pod = pod(2);
        assert_eq!(
            verify_bundle_signature(&b, &m).unwrap_err(),
            PoIError::PodMismatch
        );
    }

    #[test]
    fn epochen_muessen_zusammenpassen() {
        let m = membership(5, 3);
        let b = gueltiges_bundle(5, 4, 1_000);
        assert_eq!(
            verify_bundle_signature(&b, &m).unwrap_err(),
            PoIError::EpochMismatch {
                bundle: 4,
                membership: 3
            }
        );
    }

    #[test]
    fn signatur_aus_anderer_epoche_ist_nicht_wiederverwendbar() {
        // Beide Seiten sagen Epoche 3, aber unterschrieben wurde fuer 2.
        let m = membership(5, 3);
        let alt = gueltiges_bundle(5, 2, 1_000);
        let b = PoIBundle {
            epoch: EpochId(3),
            ..alt
        };
        assert_eq!(
            verify_bundle_signature(&b, &m).unwrap_err(),
            PoIError::InvalidAggregateSignature
        );
    }

    // ── Einreichung ─────────────────────────────────────────────────

    #[test]
    fn koordinator_kann_einreichen() {
        let mut reg = PoIRegistry::new();
        let m = membership(5, 3);
        let b = gueltiges_bundle(5, 3, 1_000);
        assert!(reg.submit(&b, &m, &miner(0), EpochId(3)).is_ok());
        assert_eq!(reg.get(EpochId(3), pod(1)), Some(&b));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn nicht_koordinator_kann_nicht_einreichen() {
        let mut reg = PoIRegistry::new();
        let m = membership(5, 3);
        let b = gueltiges_bundle(5, 3, 1_000);
        assert_eq!(
            reg.submit(&b, &m, &miner(2), EpochId(3)).unwrap_err(),
            PoIError::NotCoordinator
        );
        assert!(reg.is_empty());
    }

    #[test]
    fn doppelte_einreichung_wird_abgelehnt() {
        // Ohne diese Sperre liesse sich dieselbe Arbeit mehrfach praegen.
        let mut reg = PoIRegistry::new();
        let m = membership(5, 3);
        let b = gueltiges_bundle(5, 3, 1_000);
        reg.submit(&b, &m, &miner(0), EpochId(3)).expect("erste");
        assert_eq!(
            reg.submit(&b, &m, &miner(0), EpochId(3)).unwrap_err(),
            PoIError::DuplicateSubmission
        );
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn buendel_der_falschen_epoche_wird_abgelehnt() {
        let mut reg = PoIRegistry::new();
        let m = membership(5, 3);
        let b = gueltiges_bundle(5, 3, 1_000);
        assert_eq!(
            reg.submit(&b, &m, &miner(0), EpochId(4)).unwrap_err(),
            PoIError::WrongEpoch {
                expected: 4,
                got: 3
            }
        );
    }

    #[test]
    fn nullanspruch_wird_abgelehnt() {
        let mut reg = PoIRegistry::new();
        let m = membership(5, 3);
        let b = gueltiges_bundle(5, 3, 0);
        assert_eq!(
            reg.submit(&b, &m, &miner(0), EpochId(3)).unwrap_err(),
            PoIError::EmptyClaim
        );
    }

    #[test]
    fn abgelehntes_buendel_hinterlaesst_keinen_zustand() {
        // Eine fehlgeschlagene Pruefung darf den Platz (Epoche, Pod)
        // nicht belegen — sonst sperrte ein geschickt gebautes
        // Falschbuendel den ehrlichen Koordinator aus.
        let mut reg = PoIRegistry::new();
        let m = membership(5, 3);
        let kaputt = bundle_signed_by(&[0, 1, 2], 3, 1_000);
        assert!(reg.submit(&kaputt, &m, &miner(0), EpochId(3)).is_err());
        assert!(reg.is_empty());

        let gut = gueltiges_bundle(5, 3, 1_000);
        assert!(reg.submit(&gut, &m, &miner(0), EpochId(3)).is_ok());
    }

    #[test]
    fn mehrere_pods_je_epoche_werden_getrennt_gehalten() {
        let mut reg = PoIRegistry::new();
        let m = membership(5, 3);
        let b = gueltiges_bundle(5, 3, 1_000);
        reg.submit(&b, &m, &miner(0), EpochId(3)).expect("pod 1");

        // Zweiter Pod, eigene Mitgliedschaft und eigenes Buendel.
        let members2: Vec<(MinerId, BlsPublicKey, BlsProofOfPossession)> =
            (10..13u8).map(mitglied).collect();
        let m2 = PodMembership::new(EpochId(3), pod(2), miner(10), members2).expect("m2");
        let segments_root = root(4);
        let msg = poi_bundle_message(EpochId(3), pod(2), &segments_root, 500, 1);
        let sigs: Vec<BlsSignature> = (10..13u8)
            .map(|i| keypair(i).0.sign(&msg).expect("sign"))
            .collect();
        let agg = aggregate_signatures(&sigs).expect("aggregate");
        let b2 = PoIBundle {
            epoch: EpochId(3),
            pod: pod(2),
            segments_root,
            vtfe_claimed: 500,
            aggregate_sig: BlsSignature(agg.0),
            segmente: 1,
        };
        reg.submit(&b2, &m2, &miner(10), EpochId(3)).expect("pod 2");

        assert_eq!(reg.len(), 2);
        assert_eq!(reg.bundles_of_epoch(EpochId(3)).len(), 2);
        assert_eq!(reg.claimed_vtfe_of_epoch(EpochId(3)), 1_500);
    }

    #[test]
    fn epochen_werden_getrennt_gehalten() {
        let mut reg = PoIRegistry::new();
        for e in 3..=4u64 {
            let m = membership(5, e);
            let b = gueltiges_bundle(5, e, 1_000);
            reg.submit(&b, &m, &miner(0), EpochId(e)).expect("submit");
        }
        assert_eq!(reg.bundles_of_epoch(EpochId(3)).len(), 1);
        assert_eq!(reg.bundles_of_epoch(EpochId(4)).len(), 1);
        assert_eq!(reg.claimed_vtfe_of_epoch(EpochId(3)), 1_000);
        assert_eq!(reg.claimed_vtfe_of_epoch(EpochId(5)), 0);
    }

    #[test]
    fn reihenfolge_der_buendel_ist_kanonisch() {
        // Die Iterationsreihenfolge geht in den Epochenabschluss ein und
        // muss auf jedem Knoten dieselbe sein — unabhaengig davon, in
        // welcher Reihenfolge die Buendel eintrafen.
        let bauen = |reihenfolge: &[u8]| {
            let mut reg = PoIRegistry::new();
            for &p in reihenfolge {
                let members: Vec<(MinerId, BlsPublicKey, BlsProofOfPossession)> =
                    (0..3u8).map(mitglied).collect();
                let m = PodMembership::new(EpochId(3), pod(p), miner(0), members).expect("m");
                let segments_root = root(9);
                let msg = poi_bundle_message(EpochId(3), pod(p), &segments_root, 100, 1);
                let sigs: Vec<BlsSignature> = (0..3u8)
                    .map(|i| keypair(i).0.sign(&msg).expect("sign"))
                    .collect();
                let agg = aggregate_signatures(&sigs).expect("aggregate");
                let b = PoIBundle {
                    epoch: EpochId(3),
                    pod: pod(p),
                    segments_root,
                    vtfe_claimed: 100,
                    aggregate_sig: BlsSignature(agg.0),
                    segmente: 1,
                };
                reg.submit(&b, &m, &miner(0), EpochId(3)).expect("submit");
            }
            reg.bundles_of_epoch(EpochId(3))
                .iter()
                .map(|b| b.pod)
                .collect::<Vec<_>>()
        };
        assert_eq!(bauen(&[3, 1, 2]), bauen(&[1, 2, 3]));
        assert_eq!(bauen(&[3, 1, 2]), vec![pod(1), pod(2), pod(3)]);
    }

    #[test]
    fn summe_saettigt_statt_umzulaufen() {
        // Ein Ueberlauf wuerde die Praegemenge auf einen kleinen Wert
        // zurueckspringen lassen.
        let mut reg = PoIRegistry::new();
        for (p, v) in [(1u8, u64::MAX), (2u8, 1_000u64)] {
            let members: Vec<(MinerId, BlsPublicKey, BlsProofOfPossession)> =
                (0..3u8).map(mitglied).collect();
            let m = PodMembership::new(EpochId(3), pod(p), miner(0), members).expect("m");
            let segments_root = root(9);
            let msg = poi_bundle_message(EpochId(3), pod(p), &segments_root, v, 1);
            let sigs: Vec<BlsSignature> = (0..3u8)
                .map(|i| keypair(i).0.sign(&msg).expect("sign"))
                .collect();
            let agg = aggregate_signatures(&sigs).expect("aggregate");
            let b = PoIBundle {
                epoch: EpochId(3),
                pod: pod(p),
                segments_root,
                vtfe_claimed: v,
                aggregate_sig: BlsSignature(agg.0),
                segmente: 1,
            };
            reg.submit(&b, &m, &miner(0), EpochId(3)).expect("submit");
        }
        assert_eq!(reg.claimed_vtfe_of_epoch(EpochId(3)), u64::MAX);
    }
}
