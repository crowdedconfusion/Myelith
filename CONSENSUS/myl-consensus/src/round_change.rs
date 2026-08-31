//! Rundenwechsel, Timeouts und Sperre/Entsperrung — Whitepaper Kap. 3.5.
//!
//! [`crate::bft`] deckt **eine** Runde ab: Propose, Vote, Commit, fertig.
//! Fällt der Leader aus, bleibt diese Runde stehen — es gibt niemanden,
//! der einen Vorschlag macht, und keinen Mechanismus, der weiterschaltet.
//! Safety war damit erfüllt (nichts Falsches wird commitet), **Liveness
//! nicht** (unter Umständen wird gar nichts commitet). Dieses Modul
//! schließt die Lücke.
//!
//! ## Warum Rundenwechsel ohne Sperre die Safety bricht
//!
//! Der naive Rundenwechsel — „Timeout, nächster Leader, neuer Vorschlag" —
//! ist nicht bloß unvollständig, er ist **falsch**. Angenommen, in Runde 1
//! erreicht Block A ein Quorum an Votes; ein Teil des Komitees sieht das
//! und commitet A. Die übrigen sehen es wegen einer Partition nicht,
//! laufen in den Timeout und wechseln in Runde 2, wo Leader B den Block B
//! vorschlägt. Ohne weitere Regel stimmen sie für B, erreichen ein Quorum
//! und commiten B. Zwei verschiedene Blöcke auf derselben Höhe — genau der
//! Zustand, den BFT ausschließen soll, erzeugt durch den Mechanismus, der
//! die Liveness herstellen sollte.
//!
//! Die Regel, die das verhindert (Tendermint-Sperrmechanik, hier als
//! Eigenbau nach Design-Entscheidung 1 dieser Komponente):
//!
//! 1. **Sperren.** Wer in Runde r ein Quorum an Votes für Block A sieht,
//!    sperrt sich auf `(A, r)`. Das ist derselbe Moment, in dem A
//!    commit-fähig wird — wer commiten könnte, ist ab da gebunden.
//! 2. **Gesperrt bleiben.** In jeder späteren Runde stimmt ein gesperrter
//!    Validator nur noch für A.
//! 3. **Entsperren, aber nur mit Beweis.** Ein Vorschlag für B ≠ A wird
//!    akzeptiert, wenn ihm ein [`PolkaCertificate`] für B aus einer Runde
//!    r' mit `lock_round < r' < aktuelle Runde` beiliegt. Ein solches
//!    Zertifikat belegt, dass mehr als zwei Drittel des Stimmgewichts B
//!    bereits nach der Sperre gesehen haben — dann kann A nicht commitet
//!    worden sein, und das Entsperren ist gefahrlos.
//!
//! Der Beweiszwang in Punkt 3 ist die ganze Sicherheit. Ein Vorschlag
//! ohne Zertifikat kann eine Sperre nicht lösen, egal von wem er kommt.
//!
//! ## Warum die Timeouts mit der Runde wachsen müssen
//!
//! Ein fester Timeout stellt keine Liveness her. Ist er kürzer als die
//! tatsächliche Nachrichtenlaufzeit, läuft jede Runde in den Timeout,
//! bevor die Votes eintreffen — das Protokoll wechselt endlos die Runde,
//! ohne je zu commiten. Da die Laufzeit vor GST (Global Stabilization
//! Time) unbeschränkt ist, kann kein fester Wert richtig gewählt werden.
//!
//! Deshalb wächst der Timeout linear mit der Rundennummer
//! ([`TimeoutConfig::for_status`]). Nach GST ist die Laufzeit durch ein
//! (unbekanntes) Δ beschränkt, und da der Timeout unbeschränkt wächst,
//! gibt es eine Runde, ab der er Δ überschreitet. Ab dort hält ein
//! ehrlicher Leader die Runde lange genug offen, damit alle Votes
//! ankommen, und das Protokoll commitet.
//!
//! ## Determinismus
//!
//! Dieses Modul liest **keine Uhr**. Jede zeitabhängige Funktion bekommt
//! `now_ms` übergeben. Das ist keine Stil-, sondern eine Konsensfrage
//! (Kap. 10.3, „Determinismus-Pflicht"): ein Zustandsautomat, der selbst
//! `SystemTime::now()` aufruft, ist nicht reproduzierbar und damit nicht
//! nachprüfbar. Der Aufrufer besorgt die Zeit — im Betrieb aus der
//! Node-Uhr, im Test aus einer Zahl.
//!
//! ## Woran die Beweiskraft des Zertifikats hängt (Fund 27)
//!
//! [`PolkaCertificate`] beweist nur dann etwas, wenn ein Aggregat ohne
//! die Signaturen **aller** aufgeführten Schlüssel ungültig ist. Das
//! gilt nicht von selbst: Zu einem fremden `pk_opfer` lässt sich
//! `pk_rogue = g₁^x · pk_opfer⁻¹` bilden, und eine allein vom Angreifer
//! erzeugte Signatur gilt dann als Aggregat beider. Ein Validator mit
//! einem solchen Schlüssel könnte allein ein Zertifikat erzeugen,
//! gesperrte Validatoren entsperren — und damit zwei Blöcke auf
//! derselben Höhe ermöglichen. Identitäts- und Subgruppen-Prüfung
//! fangen das nicht ab.
//!
//! Getragen wird die Zertifikatsprüfung deshalb von
//! [`crate::validator::ValidatorRegistry::register`], das je Schlüssel
//! einen `BlsProofOfPossession` verlangt. Nur Schlüssel, deren diskreten
//! Logarithmus jemand nachweislich kennt, kommen in die
//! stimmberechtigte Menge. Regression:
//! `SHARED_TYPES/myl-types/tests/rogue_key.rs`.
//!
//! **Konsens-Feld:** Sperrregel und Zertifikatsprüfung sind Teil des
//! Konsensvertrags. Änderungen nur über Governance (Kap. 10.3).

use crate::bft::{
    BftError, BftState, Commit, Konsensnachricht, Propose, Round, RoundStatus, Vote,
    select_leader,
};
use crate::signing::{commit_message, vote_message};
use crate::validator::VotingSet;
use myl_types::bls::{BlsAggregateSignature, BlsSignature, aggregate_signatures, fast_aggregate_verify};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;
use std::collections::BTreeMap;

/// Standard-Timeout für die Propose-Phase in Millisekunden.
///
/// Abgeleitet aus dem Blockzeit-Zielwert von 2 s (Design-Entscheidung 2
/// des CONSENSUS): die Propose-Phase bekommt die Hälfte, Vote
/// und Commit je ein Viertel. Governance-Parameter.
pub const DEFAULT_TIMEOUT_PROPOSE_MS: u64 = 1_000;

/// Standard-Timeout für die Vote-Phase in Millisekunden.
pub const DEFAULT_TIMEOUT_VOTE_MS: u64 = 500;

/// Standard-Timeout für die Commit-Phase in Millisekunden.
pub const DEFAULT_TIMEOUT_COMMIT_MS: u64 = 500;

/// Standard-Zuwachs je Runde in Millisekunden.
///
/// Der Zuwachs ist der Grund, warum das Verfahren nach GST terminiert
/// (siehe Modul-Dokumentation). Er darf nicht 0 sein; [`TimeoutConfig`]
/// erzwingt das nicht, aber [`TimeoutConfig::is_live`] weist darauf hin.
pub const DEFAULT_TIMEOUT_DELTA_MS: u64 = 500;

/// Timeout-Konfiguration einer BFT-Instanz.
///
/// Alle Werte sind Governance-Parameter (Kap. 10.3). Der wirksame
/// Timeout einer Runde ist `basis + runde × delta`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeoutConfig {
    /// Basis-Timeout beim Warten auf den Vorschlag.
    pub propose_ms: u64,
    /// Basis-Timeout beim Sammeln der Votes.
    pub vote_ms: u64,
    /// Basis-Timeout beim Sammeln der Commits.
    pub commit_ms: u64,
    /// Zuwachs je Rundennummer, für alle drei Phasen.
    pub delta_ms: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            propose_ms: DEFAULT_TIMEOUT_PROPOSE_MS,
            vote_ms: DEFAULT_TIMEOUT_VOTE_MS,
            commit_ms: DEFAULT_TIMEOUT_COMMIT_MS,
            delta_ms: DEFAULT_TIMEOUT_DELTA_MS,
        }
    }
}

impl TimeoutConfig {
    /// Wirksamer Timeout für eine Phase in einer Runde, in Millisekunden.
    ///
    /// `basis + runde × delta`, sättigend. Die Sättigung ist kein
    /// Sonderfall, den man wegoptimieren sollte: ein Überlauf würde den
    /// Timeout auf einen kleinen Wert zurückspringen lassen und damit
    /// genau die Eigenschaft zerstören, wegen der er wächst.
    pub fn for_status(&self, status: RoundStatus, round: Round) -> u64 {
        let base = match status {
            RoundStatus::WaitingPropose => self.propose_ms,
            RoundStatus::CollectingVotes => self.vote_ms,
            RoundStatus::CollectingCommits => self.commit_ms,
            // Eine abgeschlossene Runde hat keinen Timeout mehr.
            RoundStatus::Committed => return u64::MAX,
        };
        base.saturating_add(round.saturating_mul(self.delta_ms))
    }

    /// Kann diese Konfiguration überhaupt Liveness herstellen?
    ///
    /// Nur mit `delta_ms > 0` wächst der Timeout unbeschränkt und
    /// überschreitet damit irgendwann die reale Nachrichtenlaufzeit.
    /// Mit `delta_ms == 0` ist das Protokoll sicher, aber möglicherweise
    /// dauerhaft blockiert.
    pub fn is_live(&self) -> bool {
        self.delta_ms > 0
    }
}

/// Eine Sperre auf einen Block.
///
/// Wer gesperrt ist, stimmt in späteren Runden nur noch für
/// `block_hash` — es sei denn, ein [`PolkaCertificate`] aus einer Runde
/// nach `round` belegt, dass die Sperre gefahrlos gelöst werden kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lock {
    /// Der Block, auf den gesperrt wurde.
    pub block_hash: Hash,
    /// Die Runde, in der die Sperre entstand.
    pub round: Round,
}

/// Nachweis, dass in einer Runde ein Quorum für einen Block gestimmt hat.
///
/// Das ist der einzige Weg, eine Sperre zu lösen. Die Prüfung in
/// [`Self::verify`] ist deshalb bewusst streng — ein Zertifikat, das zu
/// leicht zu fälschen wäre, hebt die Safety-Garantie des ganzen
/// Protokolls auf.
///
/// Die Unterzeichner stehen als Liste im Zertifikat, die Signatur als
/// BLS-Aggregat. Alle haben dieselbe Botschaft signiert
/// ([`crate::signing::vote_message`]), also greift
/// `FastAggregateVerify`.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct PolkaCertificate {
    /// Runde, aus der die Votes stammen.
    pub round: Round,
    /// Block, für den gestimmt wurde.
    pub block_hash: Hash,
    /// Unterzeichner, **streng aufsteigend** nach `MinerId`.
    ///
    /// Die Ordnung ist Teil des Formats, nicht Komfort: sie macht das
    /// Zertifikat kanonisch (ein Stimmensatz hat genau eine Kodierung)
    /// und schließt Duplikate strukturell aus. Ohne Duplikatschutz
    /// könnte ein Angreifer dieselbe Stimme mehrfach einsetzen und das
    /// Quorum mit einem einzigen Schlüssel erreichen.
    pub voters: Vec<MinerId>,
    /// Aggregierte BLS-Signatur aller Unterzeichner.
    pub aggregate: BlsAggregateSignature,
}

impl PolkaCertificate {
    /// Baut ein Zertifikat aus eingesammelten Votes.
    ///
    /// Die Votes müssen dieselbe Runde und denselben Block betreffen.
    /// Die Unterzeichnerliste wird sortiert; doppelte Unterzeichner
    /// führen zu [`RoundError::NonCanonicalCertificate`].
    ///
    /// **Hinweis:** Diese Funktion prüft die Signaturen *nicht* und
    /// gewichtet *nicht*. Sie ist der Bauhelfer für einen Knoten, der die
    /// Votes bereits über [`BftState::receive_vote`] validiert hat. Die
    /// Prüfung findet beim Empfänger statt, in [`Self::verify`].
    pub fn from_votes(votes: &[Vote]) -> Result<Self, RoundError> {
        let first = votes.first().ok_or(RoundError::EmptyCertificate)?;
        let round = first.round;
        let block_hash = first.block_hash;

        let mut pairs: Vec<(MinerId, BlsSignature)> = Vec::with_capacity(votes.len());
        for vote in votes {
            if vote.round != round || vote.block_hash != block_hash {
                return Err(RoundError::InconsistentCertificate);
            }
            pairs.push((vote.voter, vote.signature));
        }
        pairs.sort_by_key(|(m, _)| *m);
        if pairs.windows(2).any(|w| w[0].0 == w[1].0) {
            return Err(RoundError::NonCanonicalCertificate);
        }

        let sigs: Vec<BlsSignature> = pairs.iter().map(|(_, s)| *s).collect();
        let aggregate = aggregate_signatures(&sigs).map_err(|_| RoundError::InvalidSignature)?;

        Ok(Self {
            round,
            block_hash,
            voters: pairs.into_iter().map(|(m, _)| m).collect(),
            aggregate,
        })
    }

    /// Prüft das Zertifikat gegen eine stimmberechtigte Menge.
    ///
    /// Geprüft wird in dieser Reihenfolge (billig vor teuer, wie in
    /// [`crate::bft`] — die Aggregat-Verifikation ist die teuerste
    /// Operation und darf nicht als DoS-Fläche vorn stehen):
    ///
    /// 1. nicht leer,
    /// 2. Unterzeichner streng aufsteigend (kanonisch, duplikatfrei),
    /// 3. alle Unterzeichner stimmberechtigt,
    /// 4. Summe der Gewichte erreicht das Quorum,
    /// 5. Aggregat-Signatur gültig über `vote_message(round, block_hash)`.
    ///
    /// **Grenze:** Geprüft wird gegen die übergebene Menge. Zertifikate
    /// aus einer anderen Epoche haben eine andere stimmberechtigte Menge
    /// und sind hier nicht prüfbar — Rundenwechsel findet innerhalb einer
    /// Epoche statt, deshalb genügt das. Wer das Zertifikat über eine
    /// Epochengrenze trägt, muss die Menge der Ursprungsepoche mitführen.
    pub fn verify(&self, voting_set: &VotingSet) -> Result<(), RoundError> {
        pruefe_aggregat(
            &self.voters,
            voting_set,
            &vote_message(self.round, &self.block_hash),
            &self.aggregate,
        )
    }
}

/// Der gemeinsame Prüfkern beider Zertifikatsarten.
///
/// **Warum eine Funktion und nicht zweimal derselbe Ablauf:** Ein
/// Zertifikat ist genau so viel wert wie seine schwächste Prüfung. Zwei
/// Abschriften desselben Ablaufs driften auseinander, sobald eine von
/// beiden nachgebessert wird, und die Lücke säße dann in der Art, die
/// gerade niemand ansieht. Hier ist der Ablauf einmal da, also gilt für
/// Polka und Commit dieselbe Strenge.
///
/// Die Reihenfolge ist billig vor teuer, wie in [`crate::bft`]: Die
/// Aggregat-Verifikation ist die teuerste Operation und darf nicht als
/// DoS-Fläche vorn stehen.
///
/// 1. nicht leer,
/// 2. Unterzeichner streng aufsteigend (kanonisch, duplikatfrei),
/// 3. alle Unterzeichner stimmberechtigt,
/// 4. Summe der Gewichte erreicht das Quorum,
/// 5. Aggregat-Signatur gültig über `botschaft`.
fn pruefe_aggregat(
    unterzeichner: &[MinerId],
    voting_set: &VotingSet,
    botschaft: &[u8],
    aggregat: &BlsAggregateSignature,
) -> Result<(), RoundError> {
    if unterzeichner.is_empty() {
        return Err(RoundError::EmptyCertificate);
    }
    if unterzeichner.windows(2).any(|w| w[0] >= w[1]) {
        return Err(RoundError::NonCanonicalCertificate);
    }

    let mut pubkeys = Vec::with_capacity(unterzeichner.len());
    for wer in unterzeichner {
        let pk = voting_set
            .pubkey(wer)
            .ok_or(RoundError::CertificateSignerNotInCommittee)?;
        pubkeys.push(*pk);
    }

    let gewicht = unterzeichner
        .iter()
        .fold(0u64, |acc, m| acc.saturating_add(voting_set.weight(m)));
    if gewicht < voting_set.quorum_threshold() {
        return Err(RoundError::CertificateBelowQuorum);
    }

    if !fast_aggregate_verify(&pubkeys, botschaft, aggregat) {
        return Err(RoundError::InvalidSignature);
    }
    Ok(())
}

/// Nachweis, dass in einer Runde ein Quorum einen Block **commitet** hat.
///
/// # Wozu, wenn es [`PolkaCertificate`] schon gibt
///
/// Ein Polka belegt, dass ein Quorum *gestimmt* hat. Das reicht, um eine
/// Sperre zu lösen, aber nicht, um eine Entscheidung zu belegen. Ein
/// Commit-Quorum ist die Entscheidung selbst.
///
/// # ⚑ Warum es das braucht (Fund 67)
///
/// [`BftState::receive_commit`] verwirft jede Nachricht aus einer anderen
/// Runde. Das ist richtig für einzelne Nachrichten und falsch für den
/// Beleg: Ein Knoten, dessen Frist ablief, bevor die anderen ihre Runde
/// begonnen hatten, steht danach vor dem Netz. Die vier anderen commiten
/// in Runde 0, er sitzt in Runde 5 und **verwirft genau die Nachrichten,
/// die belegen, dass er der Irrende ist**. Safety hält, seine Liveness
/// nicht, und zurück kommt er von allein nie.
///
/// Die Rundennummer ist ein örtliches Mittel gegen Stillstand, ein
/// Quorumsbeleg ist eine Tatsache über das Netz. Deshalb gilt dieses
/// Zertifikat **unabhängig von der Runde des Empfängers**: Wer es prüft
/// und für gültig befindet, übernimmt die Entscheidung, in welcher Runde
/// er auch immer steht ([`RoundDriver::apply_commitzertifikat`]).
///
/// Das ist keine Sonderlösung dieses Netzes, sondern die übliche: In
/// Tendermint trägt der commitete Block seine Commit-Signaturen mit sich
/// und wird über die Blocksynchronisation unabhängig vom Zustand des
/// Konsens-Reaktors übernommen; in QBFT stehen die Commit-Siegel im
/// Blockkopf; in HotStuff ist ein Quorum-Zertifikat für sich genommen
/// gültig, ohne dass der Empfänger in der passenden Sicht säße.
///
/// **Es zieht keine Sperre nach.** Ein Commit-Quorum schließt ein
/// Vote-Quorum für denselben Block ein, die Sperre wäre also zulässig,
/// aber überflüssig: Wer die Entscheidung übernimmt, hat nichts mehr zu
/// verteidigen, wogegen eine Sperre schützte.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct Commitzertifikat {
    /// Runde, aus der die Commits stammen.
    pub round: Round,
    /// Block, der commitet wurde.
    pub block_hash: Hash,
    /// Unterzeichner, **streng aufsteigend** nach `MinerId`.
    ///
    /// Dieselbe Ordnung und derselbe Grund wie bei
    /// [`PolkaCertificate::voters`]: kanonische Kodierung, und Duplikate
    /// sind strukturell ausgeschlossen.
    pub committers: Vec<MinerId>,
    /// Aggregierte BLS-Signatur aller Unterzeichner.
    pub aggregate: BlsAggregateSignature,
}

impl Commitzertifikat {
    /// Baut ein Zertifikat aus eingesammelten Commits.
    ///
    /// Wie [`PolkaCertificate::from_votes`], mit denselben Grenzen: Die
    /// Signaturen werden hier **nicht** geprüft und **nicht** gewichtet.
    /// Das ist der Bauhelfer für einen Knoten, der die Commits bereits
    /// durch [`BftState::receive_commit`] geschickt hat. Geprüft wird
    /// beim Empfänger, in [`Self::verify`].
    pub fn from_commits(commits: &[Commit]) -> Result<Self, RoundError> {
        let first = commits.first().ok_or(RoundError::EmptyCertificate)?;
        let round = first.round;
        let block_hash = first.block_hash;

        let mut pairs: Vec<(MinerId, BlsSignature)> = Vec::with_capacity(commits.len());
        for commit in commits {
            if commit.round != round || commit.block_hash != block_hash {
                return Err(RoundError::InconsistentCertificate);
            }
            pairs.push((commit.committer, commit.signature));
        }
        pairs.sort_by_key(|(m, _)| *m);
        if pairs.windows(2).any(|w| w[0].0 == w[1].0) {
            return Err(RoundError::NonCanonicalCertificate);
        }

        let sigs: Vec<BlsSignature> = pairs.iter().map(|(_, s)| *s).collect();
        let aggregate = aggregate_signatures(&sigs).map_err(|_| RoundError::InvalidSignature)?;

        Ok(Self {
            round,
            block_hash,
            committers: pairs.into_iter().map(|(m, _)| m).collect(),
            aggregate,
        })
    }

    /// Prüft das Zertifikat gegen eine stimmberechtigte Menge.
    ///
    /// Derselbe Kern wie [`PolkaCertificate::verify`], nur über
    /// [`commit_message`] statt [`vote_message`]. Die beiden
    /// Signierbotschaften haben verschiedene Präfixe, ein Polka lässt
    /// sich also nicht als Commit-Beleg ausgeben und umgekehrt.
    ///
    /// **Grenze:** Geprüft wird gegen die übergebene Menge, das
    /// Zertifikat trägt seine Epoche nicht mit sich. Wer es über eine
    /// Epochengrenze trägt, muss die Menge der Ursprungsepoche
    /// mitführen.
    pub fn verify(&self, voting_set: &VotingSet) -> Result<(), RoundError> {
        pruefe_aggregat(
            &self.committers,
            voting_set,
            &commit_message(self.round, &self.block_hash),
            &self.aggregate,
        )
    }
}

/// Ergebnis eines Timeout-Aufrufs oder eines Rundenbelegs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundChange {
    /// Die Frist läuft noch; es bleibt `remaining_ms` übrig.
    NotDue {
        /// Verbleibende Zeit bis zur Frist.
        remaining_ms: u64,
    },
    /// Die Runde wurde gewechselt.
    Advanced {
        /// Vorherige Runde.
        from: Round,
        /// Neue Runde.
        to: Round,
        /// Leader der neuen Runde.
        leader: MinerId,
    },
    /// Es gibt nichts mehr zu tun, der Block ist commitet.
    AlreadyCommitted,
    /// Der Beleg ist vermerkt, die Drittel-Schranke aber noch nicht
    /// erreicht.
    ///
    /// **Der Zwischenstand gehört ins Protokoll**, nicht nur der Sprung.
    /// Wer über echtes WAN misst und einen hängenden Knoten sieht, will
    /// wissen, ob dessen Zähler steht oder wächst; „nichts geschehen"
    /// und „197 von 301" sind zwei verschiedene Befunde.
    Vorgemerkt {
        /// Die belegte höhere Runde.
        runde: Round,
        /// Gewicht der bisher geprüften Absender aus dieser Runde.
        gewicht: u64,
        /// Ab hier wird gesprungen.
        schranke: u64,
    },
    /// Die Nachricht sagt nichts über eine höhere Runde.
    Unerheblich,
}

/// Fehler beim Rundenwechsel und bei der Sperrprüfung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundError {
    /// Ein Fehler aus dem Rundenprotokoll selbst.
    Bft(BftError),
    /// Der Validator ist auf einen anderen Block gesperrt, und der
    /// Vorschlag trägt keinen ausreichenden Beweis.
    Locked {
        /// Block, auf den gesperrt ist.
        locked_on: Hash,
        /// Runde, in der die Sperre entstand.
        locked_round: Round,
    },
    /// Ein Zertifikat ohne Unterzeichner.
    EmptyCertificate,
    /// Die Votes eines Zertifikats betreffen nicht alle dieselbe Runde
    /// und denselben Block.
    InconsistentCertificate,
    /// Unterzeichnerliste nicht streng aufsteigend (unsortiert oder
    /// mit Duplikaten).
    NonCanonicalCertificate,
    /// Ein Unterzeichner des Zertifikats ist nicht stimmberechtigt.
    CertificateSignerNotInCommittee,
    /// Das Zertifikat erreicht das Quorum nicht.
    CertificateBelowQuorum,
    /// Das Zertifikat gehört zu einem anderen Block als der Vorschlag.
    CertificateBlockMismatch,
    /// Die Runde des Zertifikats taugt nicht zum Entsperren: sie liegt
    /// nicht echt zwischen Sperrrunde und aktueller Runde.
    CertificateRoundNotUsable {
        /// Runde des Zertifikats.
        certificate_round: Round,
        /// Runde der Sperre.
        locked_round: Round,
        /// Aktuelle Runde.
        current_round: Round,
    },
    /// Ungültige Signatur oder fehlgeschlagene Aggregation.
    InvalidSignature,
    /// Die Producer-Liste ist leer — kein Leader wählbar.
    NoProducers,
    /// Ein Producer gehört nicht zur stimmberechtigten Menge.
    ProducerNotInCommittee,
    /// Ein gültiges Commit-Zertifikat belegt einen **anderen** Block, als
    /// dieser Knoten bereits commitet hat.
    ///
    /// ⚑ **Kein Empfangsfehler, sondern ein Sicherheitsbefund.** Zwei
    /// Quoren für zwei Blöcke derselben Höhe kann es unter der
    /// Mehrheitsannahme nicht geben. Wer das sieht, sieht, dass die
    /// Annahme gebrochen ist, und muss es vermerken statt es zu
    /// verwerfen.
    ConflictingCommit,
}

impl From<BftError> for RoundError {
    fn from(e: BftError) -> Self {
        Self::Bft(e)
    }
}

impl std::fmt::Display for RoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bft(e) => write!(f, "{}", e),
            Self::Locked {
                locked_on,
                locked_round,
            } => write!(
                f,
                "Gesperrt auf Block {} aus Runde {} — Vorschlag ohne ausreichenden Polka-Beweis",
                locked_on, locked_round
            ),
            Self::EmptyCertificate => write!(f, "Zertifikat ohne Unterzeichner"),
            Self::InconsistentCertificate => {
                write!(f, "Zertifikat mischt Runden oder Blöcke")
            }
            Self::NonCanonicalCertificate => {
                write!(f, "Unterzeichner nicht streng aufsteigend")
            }
            Self::CertificateSignerNotInCommittee => {
                write!(f, "Unterzeichner des Zertifikats ist nicht stimmberechtigt")
            }
            Self::CertificateBelowQuorum => {
                write!(f, "Zertifikat erreicht das Quorum nicht")
            }
            Self::CertificateBlockMismatch => {
                write!(f, "Zertifikat gehört zu einem anderen Block")
            }
            Self::CertificateRoundNotUsable {
                certificate_round,
                locked_round,
                current_round,
            } => write!(
                f,
                "Zertifikatsrunde {} liegt nicht echt zwischen Sperrrunde {} und Runde {}",
                certificate_round, locked_round, current_round
            ),
            Self::InvalidSignature => write!(f, "Ungültige Signatur"),
            Self::NoProducers => write!(f, "Keine Producer — kein Leader wählbar"),
            Self::ProducerNotInCommittee => {
                write!(f, "Producer gehört nicht zur stimmberechtigten Menge")
            }
            Self::ConflictingCommit => write!(
                f,
                "Zwei Quoren für zwei Blöcke derselben Höhe — die Mehrheitsannahme ist gebrochen"
            ),
        }
    }
}

impl std::error::Error for RoundError {}

/// Treiber über mehrere BFT-Runden hinweg.
///
/// Hält die aktuelle Runde, ihren [`BftState`], die Sperre und die
/// laufende Frist. Der Aufrufer reicht Nachrichten und die aktuelle Zeit
/// herein; der Treiber entscheidet, wann gewechselt wird und ob ein
/// Vorschlag angenommen werden darf.
///
/// Die Sperre überlebt den Rundenwechsel — das ist ihr ganzer Zweck.
/// Alles andere (Votes, Commits, Vorschlag) wird pro Runde neu begonnen.
#[derive(Debug, Clone)]
pub struct RoundDriver {
    bft: BftState,
    lock: Option<Lock>,
    producers: Vec<MinerId>,
    voting_set: VotingSet,
    timeouts: TimeoutConfig,
    /// Zeitpunkt, an dem die laufende Phase abläuft.
    deadline_ms: u64,
    /// Status, für den `deadline_ms` gesetzt wurde — daran wird erkannt,
    /// dass eine Phase gewechselt hat und die Frist neu zu setzen ist.
    deadline_for: RoundStatus,
    /// Je Absender die **höchste** Runde, aus der eine geprüfte
    /// Nachricht vorlag.
    ///
    /// ⚑ **Nicht umgekehrt, und beides hat einen Grund.** Der erste
    /// Entwurf führte je Runde die Absender. Er war in zwei Punkten
    /// schlechter:
    ///
    /// - **Er wuchs unbegrenzt.** Ein einziger stimmberechtigter
    ///   Byzantiner kann gültig unterschriebene Stimmen für beliebig
    ///   viele Runden schicken; jede legte einen Eintrag an. Die
    ///   Signaturprüfung hält Fremde draußen, nicht Mitglieder. Hier
    ///   ist der Schlüssel der Absender, also ist die Karte durch die
    ///   Größe der stimmberechtigten Menge begrenzt, und Fluten hebt nur
    ///   den eigenen Eintrag.
    /// - **Er zählte zu wenig.** Zwei Knoten in Runde 4 und zwei in
    ///   Runde 5 sind vier Knoten in Runde **mindestens 4**. Je Runde
    ///   getrennt gezählt blieben es zweimal zwei, und der Sprung
    ///   unterblieb, obwohl Runde 4 belegt war.
    ///
    /// Eine Unterschrift unter eine Stimme der Runde 7 bezeugt, dass
    /// ihr Urheber in Runde 7 stand, und damit auch, dass er jede
    /// niedrigere hinter sich hat.
    hoechste_runde_je_absender: BTreeMap<MinerId, Round>,
}

impl RoundDriver {
    /// Startet den Treiber in Runde 0.
    ///
    /// **Parameter:**
    /// - `producers`: Blockproduktions-Validatoren in kanonischer
    ///   Reihenfolge; der Leader rotiert per Round-Robin darüber
    ///   ([`select_leader`]).
    /// - `voting_set`: stimmberechtigte Menge der Epoche
    /// - `timeouts`: Timeout-Parameter
    /// - `now_ms`: Startzeitpunkt
    ///
    /// **Fehler:** [`RoundError::NoProducers`] bei leerer Liste,
    /// [`RoundError::ProducerNotInCommittee`], wenn ein Producer nicht
    /// stimmberechtigt ist — das wäre eine Runde, deren Leader nicht
    /// vorschlagen darf, also eine garantierte Timeout-Runde.
    pub fn new(
        producers: Vec<MinerId>,
        voting_set: VotingSet,
        timeouts: TimeoutConfig,
        now_ms: u64,
    ) -> Result<Self, RoundError> {
        if producers.is_empty() {
            return Err(RoundError::NoProducers);
        }
        for p in &producers {
            if !voting_set.contains(p) {
                return Err(RoundError::ProducerNotInCommittee);
            }
        }
        let leader = select_leader(0, &producers).ok_or(RoundError::NoProducers)?;
        let bft = BftState::new(0, leader, voting_set.clone())?;
        let deadline_ms =
            now_ms.saturating_add(timeouts.for_status(RoundStatus::WaitingPropose, 0));

        Ok(Self {
            bft,
            lock: None,
            producers,
            voting_set,
            timeouts,
            deadline_ms,
            deadline_for: RoundStatus::WaitingPropose,
            hoechste_runde_je_absender: BTreeMap::new(),
        })
    }

    /// Aktuelle Runde.
    pub fn round(&self) -> Round {
        self.bft.round
    }

    /// Leader der aktuellen Runde.
    pub fn leader(&self) -> MinerId {
        self.bft.leader
    }

    /// Status der aktuellen Runde.
    pub fn status(&self) -> RoundStatus {
        self.bft.status
    }

    /// Die aktive Sperre, falls vorhanden.
    pub fn lock(&self) -> Option<Lock> {
        self.lock
    }

    /// Zeitpunkt, an dem die laufende Phase abläuft.
    pub fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }

    /// Der Rundenzustand, für Abfragen wie Stimmgewicht und Zähler.
    pub fn state(&self) -> &BftState {
        &self.bft
    }

    /// Ist der Block der aktuellen Runde commitet?
    pub fn is_committed(&self) -> bool {
        self.bft.is_committed()
    }

    /// Der commitete Block-Hash, falls die Runde abgeschlossen ist.
    pub fn committed_block(&self) -> Option<Hash> {
        self.bft.committed_block()
    }

    /// Darf für diesen Vorschlag gestimmt werden?
    ///
    /// Setzt die Sperrregel aus der Modul-Dokumentation um. Ohne Sperre
    /// ist alles erlaubt; mit Sperre nur der gesperrte Block oder ein
    /// Block mit gültigem Zertifikat aus einer Runde echt zwischen
    /// Sperrrunde und aktueller Runde.
    ///
    /// Verändert nichts — die Entsperrung selbst passiert in
    /// [`Self::receive_propose`].
    pub fn may_vote_for(
        &self,
        block_hash: &Hash,
        pol: Option<&PolkaCertificate>,
    ) -> Result<(), RoundError> {
        let Some(lock) = self.lock else {
            return Ok(());
        };
        if lock.block_hash == *block_hash {
            return Ok(());
        }
        let Some(cert) = pol else {
            return Err(RoundError::Locked {
                locked_on: lock.block_hash,
                locked_round: lock.round,
            });
        };
        if cert.block_hash != *block_hash {
            return Err(RoundError::CertificateBlockMismatch);
        }
        // Echt nach der Sperre und echt vor der laufenden Runde. Ein
        // Zertifikat aus der laufenden Runde taugt nicht: es müsste dann
        // bereits ein Quorum geben, das der Vorschlag erst herbeiführen
        // soll.
        if cert.round <= lock.round || cert.round >= self.bft.round {
            return Err(RoundError::CertificateRoundNotUsable {
                certificate_round: cert.round,
                locked_round: lock.round,
                current_round: self.bft.round,
            });
        }
        cert.verify(&self.voting_set)
    }

    /// Nimmt einen Vorschlag entgegen.
    ///
    /// Zuerst die Sperrregel ([`Self::may_vote_for`]), dann die
    /// Prüfkette des Rundenprotokolls ([`BftState::receive_propose`]).
    /// Die Reihenfolge ist bewusst: ein Vorschlag, der die Sperre
    /// verletzt, wird abgelehnt, bevor er den Zustand berührt.
    ///
    /// Löst das mitgelieferte Zertifikat die Sperre, wird sie **auf das
    /// Zertifikat umgesetzt** — nicht einfach entfernt. Der Validator
    /// bleibt gesperrt, nur eben auf den neueren Block; sonst wäre er
    /// bis zur nächsten Sperre ungebunden.
    pub fn receive_propose(
        &mut self,
        propose: &Propose,
        pol: Option<&PolkaCertificate>,
        now_ms: u64,
    ) -> Result<(), RoundError> {
        self.may_vote_for(&propose.block_hash, pol)?;
        // ⚑ **Fund 66 (2026-08-26): Die Signatur deckte die
        // `valid_round` nicht ab.**
        //
        // Bis hierher rief diese Stelle `bft.receive_propose`, und das
        // prüft die Signatur immer gegen [`crate::signing::propose_message`],
        // also **ohne** die Runde des mitgelieferten Zertifikats.
        // [`crate::signing::DST_PROPOSE_POL`] und `propose_pol_message`
        // existieren seit v0.5.0 genau für diesen Fall, sind in ihrem
        // Doc-Kommentar als notwendig begründet, und **wurden von
        // nirgends aufgerufen**.
        //
        // Dieselbe Klasse wie Audit-Punkt A10: Ein Schutz, den ein
        // Leser für vorhanden hält, weil er dasteht.
        //
        // **Was ohne die Bindung möglich war:** Ein Abhörer nimmt einen
        // ehrlichen Propose für Block B und hängt ein **anderes**
        // gültiges Zertifikat für denselben Block an. Beides prüft
        // durch, denn `cert.verify` steht für sich und die Signatur
        // deckt das Zertifikat nicht. Zwei Nachrichten mit derselben
        // Aussage, verschiedenen Nachrichten-Ids und beide gültig; der
        // Leader kann für keine von beiden zur Verantwortung gezogen
        // werden, und das trifft den Double-Signing-Beweis.
        match pol {
            Some(cert) => self.bft.receive_propose_mit_polka(propose, cert.round)?,
            None => self.bft.receive_propose(propose)?,
        }

        if let Some(cert) = pol {
            if self.lock.map(|l| l.block_hash) != Some(cert.block_hash) {
                self.lock = Some(Lock {
                    block_hash: cert.block_hash,
                    round: cert.round,
                });
            }
        }

        self.refresh_deadline(now_ms);
        Ok(())
    }

    /// Nimmt eine Vote entgegen.
    ///
    /// Erreicht das Stimmgewicht das Quorum, entsteht die Sperre auf den
    /// vorgeschlagenen Block der laufenden Runde. Das ist der Punkt, an
    /// dem der Block commit-fähig wird — wer ihn commiten könnte, ist ab
    /// hier gebunden.
    pub fn receive_vote(&mut self, vote: &Vote, now_ms: u64) -> Result<(), RoundError> {
        self.bft.receive_vote(vote)?;

        if self.bft.vote_weight() >= self.bft.threshold() {
            if let Some(block_hash) = self.bft.proposed_block {
                self.lock = Some(Lock {
                    block_hash,
                    round: self.bft.round,
                });
            }
        }

        self.refresh_deadline(now_ms);
        Ok(())
    }

    /// Nimmt einen Commit entgegen.
    pub fn receive_commit(&mut self, commit: &Commit, now_ms: u64) -> Result<(), RoundError> {
        self.bft.receive_commit(commit)?;
        self.refresh_deadline(now_ms);
        Ok(())
    }

    /// Übernimmt ein Zertifikat aus einer späteren Runde als die Sperre.
    ///
    /// Der Weg, auf dem ein Validator entsperrt wird, **ohne** dass ein
    /// Vorschlag eintrifft — etwa weil er die Votes selbst gesehen hat.
    ///
    /// **Returns:** `true`, wenn die Sperre verändert wurde.
    pub fn apply_polka(&mut self, cert: &PolkaCertificate) -> Result<bool, RoundError> {
        cert.verify(&self.voting_set)?;
        match self.lock {
            Some(lock) if cert.round <= lock.round => Ok(false),
            Some(lock) if lock.block_hash == cert.block_hash => {
                // Gleicher Block, neuere Runde: Sperrrunde nachziehen.
                self.lock = Some(Lock {
                    block_hash: cert.block_hash,
                    round: cert.round,
                });
                Ok(lock.round != cert.round)
            }
            _ => {
                self.lock = Some(Lock {
                    block_hash: cert.block_hash,
                    round: cert.round,
                });
                Ok(true)
            }
        }
    }

    /// Übernimmt ein geprüftes Commit-Zertifikat, **gleich aus welcher
    /// Runde**.
    ///
    /// ⚑ Der Rückweg aus Fund 67. Ein Knoten, der allein vorauseilt,
    /// verwirft über [`BftState::receive_commit`] jeden einzelnen Commit
    /// der Runde, die das Netz längst entschieden hat. Ein Zertifikat
    /// dagegen belegt das Quorum in einer Nachricht, und ein Quorumsbeleg
    /// gilt ohne Rücksicht auf die eigene Rundennummer.
    ///
    /// **Warum das nicht rückwärts gehen heißt.** Der Knoten springt
    /// nicht in die alte Runde zurück; er nimmt ihr Ergebnis an. Eine
    /// Runde zurückzusetzen wäre angreifbar, denn dann zöge altes
    /// Nachrichtenmaterial einen Knoten beliebig weit nach hinten. Eine
    /// Entscheidung anzunehmen ist es nicht: Sie ist durch ein Quorum
    /// gedeckt, und ein zweites Quorum für einen anderen Block derselben
    /// Höhe gibt es unter der Mehrheitsannahme nicht.
    ///
    /// **Returns:** `true`, wenn die Übernahme den Zustand geändert hat,
    /// `false`, wenn dieser Knoten denselben Block schon commitet hatte.
    ///
    /// **Fehler:** [`RoundError::InvalidSignature`] und die übrigen
    /// Zertifikatsfehler aus [`Commitzertifikat::verify`]; dazu
    /// [`RoundError::ConflictingCommit`], wenn dieser Knoten bereits
    /// einen **anderen** Block commitet hatte. Das ist keine
    /// Empfangsstörung, sondern die Beobachtung zweier Quoren für zwei
    /// Blöcke, also der Bruch der Mehrheitsannahme. Der Aufrufer muss
    /// das laut vermerken und darf es nicht wegwerfen.
    pub fn apply_commitzertifikat(&mut self, zert: &Commitzertifikat) -> Result<bool, RoundError> {
        // **Billig vor teuer, und hier zählt es doppelt.** Wer denselben
        // Block längst commitet hat, braucht den Beleg nicht und prüft
        // ihn deshalb auch nicht. Ohne diese Zeile kostete jedes
        // eintreffende Zertifikat eine Aggregat-Verifikation, auch das
        // hundertste über dieselbe Entscheidung.
        if self.bft.committed_block() == Some(zert.block_hash) {
            return Ok(false);
        }
        // Ab hier wird geprüft, und zwar **vor** jedem Urteil über einen
        // Widerspruch: Ein ungeprüftes Zertifikat als Gabelung zu melden
        // hieße, dass jeder Beliebige mit erfundenen Bytes einen
        // Sicherheitsalarm auslösen kann.
        zert.verify(&self.voting_set)?;
        if self.bft.is_committed() {
            return Err(RoundError::ConflictingCommit);
        }
        self.bft.uebernimm_commit(zert.block_hash);
        Ok(true)
    }

    /// Prüft die Frist und wechselt bei Ablauf die Runde.
    ///
    /// Der Aufrufer ruft das periodisch mit der aktuellen Zeit auf. Es
    /// gibt bewusst keinen internen Timer: der Zustandsautomat bleibt
    /// damit rein und im Test ohne Warten durchspielbar.
    pub fn on_timeout(&mut self, now_ms: u64) -> Result<RoundChange, RoundError> {
        if self.bft.is_committed() {
            return Ok(RoundChange::AlreadyCommitted);
        }
        if now_ms < self.deadline_ms {
            return Ok(RoundChange::NotDue {
                remaining_ms: self.deadline_ms - now_ms,
            });
        }
        self.advance_round(now_ms)
    }

    /// Wechselt die Runde unabhängig von der Frist.
    ///
    /// Getrennt von [`Self::on_timeout`], weil es Gründe für einen
    /// Wechsel gibt, die keine Fristsache sind — etwa ein nachweislich
    /// ungültiger Vorschlag des Leaders.
    ///
    /// Die Sperre bleibt bestehen; Votes, Commits und Vorschlag der alten
    /// Runde werden verworfen.
    pub fn advance_round(&mut self, now_ms: u64) -> Result<RoundChange, RoundError> {
        self.springe_auf(self.bft.round.saturating_add(1), now_ms)
    }

    /// Vermerkt eine Nachricht aus einer **höheren** Runde und springt
    /// dorthin, sobald mehr als ein Drittel des Gewichts von dort kommt.
    ///
    /// # Die zweite Richtung aus Fund 67
    ///
    /// [`Self::apply_commitzertifikat`] holt einen Knoten zurück, der
    /// **voraus** ist. Diese Methode holt einen, der **zurückgefallen**
    /// ist, und das ist der häufigere Fall: Wer später startet, kurz die
    /// Verbindung verliert oder hinter einem langsamen Mesh sitzt, steht
    /// in Runde 2, während die anderen in Runde 5 sind.
    ///
    /// Ohne sie holt er nur über die eigene Uhr auf, Runde für Runde,
    /// und jede Frist ist um den Zuwachs länger als die vorige. Über ein
    /// WAN mit echten Latenzen ist das der Unterschied zwischen einem
    /// Knoten, der zurückkommt, und einem, der zusieht.
    ///
    /// # Warum ein Drittel genügt
    ///
    /// Mehr als ein Drittel des Stimmgewichts kann nicht vollständig
    /// byzantinisch sein. Wer diese Schranke zusammenbekommt, hat von
    /// mindestens einem ehrlichen Knoten gehört, und dass der dort
    /// steht, ist wahr. Ein Quorum zu verlangen wäre zu streng: Der
    /// Zurückgefallene hört naturgemäß nur einen Teil.
    ///
    /// **Gezählt wird „Runde mindestens r", nicht „Runde genau r".**
    /// Eine Unterschrift unter eine Stimme der Runde 7 bezeugt auch,
    /// dass ihr Urheber jede niedrigere hinter sich hat. Zwei Knoten in
    /// Runde 4 und zwei in Runde 5 sind deshalb vier Knoten in Runde
    /// mindestens 4, und Runde 4 ist belegt. Der erste Entwurf zählte je
    /// Runde getrennt, kam auf zweimal zwei und sprang nicht.
    ///
    /// # ⚑ Erst prüfen, dann zählen
    ///
    /// **Die Reihenfolge ist die ganze Sicherheit dieser Regel.**
    /// [`BftState::receive_vote`] lehnt eine fremde Runde ab, **bevor**
    /// es die Signatur prüft; das ist dort richtig, spart es doch eine
    /// Paarung je verirrter Nachricht. Wer die abgelehnten Nachrichten
    /// aber ungeprüft zählte, hätte eine Liveness-Lücke gegen eine
    /// andere getauscht: Ein einzelner Byzantiner dürfte sich als
    /// beliebig viele Absender ausgeben und jeden ehrlichen Knoten in
    /// jede Runde treiben, die er sich ausdenkt. Deshalb prüft diese
    /// Methode die Unterschrift selbst, und zwar vor dem Vermerk.
    ///
    /// # Nur Vote und Commit
    ///
    /// Ein Propose kommt je Runde von genau einem Leader und trägt
    /// deshalb nie genug Gewicht; ein Commit-Zertifikat hat seinen
    /// eigenen Weg. Gezählt werden die beiden Marken, von denen es viele
    /// gibt. Alles andere gibt [`RoundChange::Unerheblich`].
    ///
    /// # Der Sprung ist nicht begrenzt, und das ist Absicht
    ///
    /// Vor dieser Regel wuchs die Runde nur um eins je Frist; jetzt ist
    /// jede Runde in einem Schritt erreichbar. Eine Schranke dagegen
    /// wäre falsch: Wer lange fehlte, muss weit springen dürfen, und
    /// **jede belegte Runde ist von mindestens einem Ehrlichen bezeugt.**
    /// Wer beliebig hohe Runden erzwingen will, braucht dafür ein
    /// Drittel des Gewichts, und damit steht der Konsens ohnehin still.
    /// Die Fristrechnung sättigt (`saturating_mul`), es gibt also keinen
    /// Überlauf, sondern im Grenzfall eine Runde ohne Frist.
    ///
    /// # Was das kostet
    ///
    /// Eine Paarung je verirrter Vote oder Commit. Der Automat sparte
    /// sie sich, indem er die Runde zuerst prüfte; dieser Weg holt sie
    /// nach, weil er ohne sie nichts wert wäre. Nach einem
    /// Rundenwechsel trudeln solche Nachrichten regelmäßig ein, das ist
    /// der Normalfall und kein Angriff. **Wer die Kosten als Hebel
    /// benutzen will, muss stimmberechtigt sein**, denn ein Fremder
    /// scheitert schon an `pubkey`, und dann ist die Ratenbegrenzung der
    /// Netzschicht der richtige Ort, nicht diese Methode.
    ///
    /// # Die Sperre bleibt
    ///
    /// Der Sprung ist ein Rundenwechsel wie jeder andere und rührt
    /// [`Self::lock`] nicht an. Entsperrt wird weiterhin nur über ein
    /// Zertifikat. Ohne das wäre die Regel ein Sicherheitsloch statt
    /// eines Liveness-Gewinns.
    pub fn merke_hoehere_runde(
        &mut self,
        n: &Konsensnachricht,
        now_ms: u64,
    ) -> Result<RoundChange, RoundError> {
        if self.bft.is_committed() {
            return Ok(RoundChange::AlreadyCommitted);
        }
        let runde = n.runde();
        if runde <= self.bft.round {
            return Ok(RoundChange::Unerheblich);
        }

        let (absender, botschaft, signatur) = match n {
            Konsensnachricht::Vote(v) => {
                (v.voter, vote_message(v.round, &v.block_hash), &v.signature)
            }
            Konsensnachricht::Commit(c) => (
                c.committer,
                commit_message(c.round, &c.block_hash),
                &c.signature,
            ),
            _ => return Ok(RoundChange::Unerheblich),
        };

        // ⚑ Erst prüfen, dann zählen. Siehe Kopf.
        let pubkey = self
            .voting_set
            .pubkey(&absender)
            .ok_or(RoundError::Bft(BftError::NotInCommittee))?;
        if !pubkey.verify(&botschaft, signatur) {
            return Err(RoundError::Bft(BftError::InvalidSignature));
        }

        let eintrag = self
            .hoechste_runde_je_absender
            .entry(absender)
            .or_insert(0);
        *eintrag = (*eintrag).max(runde);

        // **Absteigend nach Runde, dann aufsummieren.** Das Gewicht, das
        // eine Runde `r` belegt, ist die Summe über alle Absender mit
        // höchster Runde `>= r`; sie wächst also monoton, während `r`
        // fällt. Wer absteigend summiert, trifft die **höchste** Runde,
        // für die es reicht, beim ersten Erreichen der Schranke. Auf
        // eine niedrigere zu springen hieße, gleich darauf erneut zu
        // springen.
        let schranke = self.voting_set.drittel_schranke();
        let mut paare: Vec<(Round, u64)> = self
            .hoechste_runde_je_absender
            .iter()
            .filter(|(_, r)| **r > self.bft.round)
            .map(|(m, r)| (*r, self.voting_set.weight(m)))
            .collect();
        paare.sort_unstable_by_key(|(r, _)| std::cmp::Reverse(*r));

        let mut summe: u64 = 0;
        let mut ziel = None;
        for (r, g) in &paare {
            summe = summe.saturating_add(*g);
            if summe >= schranke {
                ziel = Some(*r);
                break;
            }
        }

        match ziel {
            Some(z) => self.springe_auf(z, now_ms),
            None => Ok(RoundChange::Vorgemerkt {
                // Das ist bewusst die Summe **aller** Absender über der
                // eigenen Runde, also das Höchste, was aus dem heutigen
                // Wissen überhaupt erreichbar wäre. Wer über echtes WAN
                // einen hängenden Knoten beobachtet, will genau diesen
                // Abstand sehen.
                runde: paare.first().map_or(runde, |(r, _)| *r),
                gewicht: summe,
                schranke,
            }),
        }
    }

    /// Wechselt auf eine bestimmte Runde.
    ///
    /// Gemeinsamer Rumpf von [`Self::advance_round`] (Ziel: die nächste)
    /// und [`Self::merke_hoehere_runde`] (Ziel: die belegte). **Die
    /// Sperre bleibt bestehen**, Votes, Commits und Vorschlag der alten
    /// Runde werden verworfen.
    fn springe_auf(&mut self, ziel: Round, now_ms: u64) -> Result<RoundChange, RoundError> {
        let from = self.bft.round;
        let leader = select_leader(ziel, &self.producers).ok_or(RoundError::NoProducers)?;

        self.bft = BftState::new(ziel, leader, self.voting_set.clone())?;
        self.deadline_for = RoundStatus::WaitingPropose;
        self.deadline_ms = now_ms.saturating_add(
            self.timeouts
                .for_status(RoundStatus::WaitingPropose, ziel),
        );
        // Belege bis einschließlich der erreichten Runde sind erledigt.
        self.hoechste_runde_je_absender.retain(|_, r| *r > ziel);

        Ok(RoundChange::Advanced {
            from,
            to: ziel,
            leader,
        })
    }

    /// Setzt die Frist neu, wenn die Phase gewechselt hat.
    fn refresh_deadline(&mut self, now_ms: u64) {
        if self.bft.status != self.deadline_for {
            self.deadline_for = self.bft.status;
            self.deadline_ms = now_ms
                .saturating_add(self.timeouts.for_status(self.bft.status, self.bft.round));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::{commit_message, propose_message};
    use crate::validator::{VotingMember, VotingSet};
    use myl_types::bls::{BlsPublicKey, BlsSecretKey};
    use std::collections::BTreeMap;

    fn test_miner(byte: u8) -> MinerId {
        MinerId::new([byte; 32])
    }

    fn test_hash(byte: u8) -> Hash {
        Hash::sha256(&[byte])
    }

    fn keypair(byte: u8) -> (BlsSecretKey, BlsPublicKey) {
        let sk = BlsSecretKey::key_gen(&[byte.wrapping_add(1); 32]).expect("key_gen");
        let pk = sk.public_key().expect("public_key");
        (sk, pk)
    }

    fn voting_set(n: u8, weight: u64) -> VotingSet {
        let mut members = BTreeMap::new();
        for i in 0..n {
            let (_, pk) = keypair(i);
            members.insert(test_miner(i), VotingMember { pubkey: pk, weight });
        }
        VotingSet::from_members(members)
    }

    fn producers(n: u8) -> Vec<MinerId> {
        (0..n).map(test_miner).collect()
    }

    fn signed_propose(round: Round, hash: Hash, leader_byte: u8) -> Propose {
        let (sk, _) = keypair(leader_byte);
        Propose {
            round,
            block_hash: hash,
            leader: test_miner(leader_byte),
            signature: sk.sign(&propose_message(round, &hash)).unwrap(),
        }
    }

    /// Ein Vorschlag, der ein Zertifikat aus `valid_round` mitbringt.
    ///
    /// ⚑ **Fund 66:** Die Signatur muss über
    /// [`crate::signing::propose_pol_message`] gehen, sonst deckt sie die
    /// Runde des Zertifikats nicht ab. Bis zum 2026-08-26 gab es diesen
    /// Helfer nicht, und der einzige Test, der den Zertifikatspfad
    /// benutzte, signierte mit `propose_message` — **und kam durch**.
    fn signed_propose_pol(
        round: Round,
        hash: Hash,
        leader_byte: u8,
        valid_round: Round,
    ) -> Propose {
        let (sk, _) = keypair(leader_byte);
        Propose {
            round,
            block_hash: hash,
            leader: test_miner(leader_byte),
            signature: sk
                .sign(&crate::signing::propose_pol_message(round, &hash, valid_round))
                .unwrap(),
        }
    }

    fn signed_vote(round: Round, hash: Hash, voter_byte: u8) -> Vote {
        let (sk, _) = keypair(voter_byte);
        Vote {
            round,
            block_hash: hash,
            voter: test_miner(voter_byte),
            signature: sk.sign(&vote_message(round, &hash)).unwrap(),
        }
    }

    fn signed_commit(round: Round, hash: Hash, committer_byte: u8) -> Commit {
        let (sk, _) = keypair(committer_byte);
        Commit {
            round,
            block_hash: hash,
            committer: test_miner(committer_byte),
            signature: sk.sign(&commit_message(round, &hash)).unwrap(),
        }
    }

    /// Zertifikat aus `count` Stimmen für `hash` in `round`.
    fn polka(round: Round, hash: Hash, count: u8) -> PolkaCertificate {
        let votes: Vec<Vote> = (0..count).map(|i| signed_vote(round, hash, i)).collect();
        PolkaCertificate::from_votes(&votes).expect("Zertifikat")
    }

    /// Commit-Zertifikat aus `count` Commits für `hash` in `round`.
    fn commitzert(round: Round, hash: Hash, count: u8) -> Commitzertifikat {
        let commits: Vec<Commit> = (0..count).map(|i| signed_commit(round, hash, i)).collect();
        Commitzertifikat::from_commits(&commits).expect("Zertifikat")
    }

    fn driver(n: u8) -> RoundDriver {
        RoundDriver::new(
            producers(n),
            voting_set(n, 100),
            TimeoutConfig::default(),
            0,
        )
        .expect("Treiber")
    }

    // ── Timeout-Konfiguration ───────────────────────────────────────

    #[test]
    fn timeout_waechst_mit_der_runde() {
        let c = TimeoutConfig::default();
        let r0 = c.for_status(RoundStatus::WaitingPropose, 0);
        let r1 = c.for_status(RoundStatus::WaitingPropose, 1);
        let r9 = c.for_status(RoundStatus::WaitingPropose, 9);
        assert!(r0 < r1 && r1 < r9);
        assert_eq!(r9, r0 + 9 * c.delta_ms);
    }

    #[test]
    fn timeout_saettigt_statt_ueberzulaufen() {
        // Ein Ueberlauf wuerde den Timeout auf einen winzigen Wert
        // zurueckspringen lassen — genau die Eigenschaft zerstoeren,
        // wegen der er waechst.
        let c = TimeoutConfig::default();
        assert_eq!(c.for_status(RoundStatus::WaitingPropose, u64::MAX), u64::MAX);
    }

    #[test]
    fn commitete_runde_hat_keinen_timeout() {
        let c = TimeoutConfig::default();
        assert_eq!(c.for_status(RoundStatus::Committed, 3), u64::MAX);
    }

    #[test]
    fn delta_null_ist_nicht_live() {
        assert!(TimeoutConfig::default().is_live());
        let starr = TimeoutConfig {
            delta_ms: 0,
            ..TimeoutConfig::default()
        };
        assert!(!starr.is_live());
    }

    // ── Rundenwechsel ───────────────────────────────────────────────

    #[test]
    fn treiber_startet_in_runde_null() {
        let d = driver(4);
        assert_eq!(d.round(), 0);
        assert_eq!(d.leader(), test_miner(0));
        assert!(d.lock().is_none());
    }

    #[test]
    fn treiber_lehnt_leere_producer_liste_ab() {
        let r = RoundDriver::new(vec![], voting_set(4, 100), TimeoutConfig::default(), 0);
        assert_eq!(r.unwrap_err(), RoundError::NoProducers);
    }

    #[test]
    fn treiber_lehnt_nicht_stimmberechtigten_producer_ab() {
        // Ein Producer ohne Stimmrecht waere ein Leader, der nicht
        // vorschlagen darf — eine garantierte Timeout-Runde.
        let mut ps = producers(4);
        ps.push(test_miner(99));
        let r = RoundDriver::new(ps, voting_set(4, 100), TimeoutConfig::default(), 0);
        assert_eq!(r.unwrap_err(), RoundError::ProducerNotInCommittee);
    }

    #[test]
    fn vor_der_frist_wird_nicht_gewechselt() {
        let mut d = driver(4);
        let deadline = d.deadline_ms();
        match d.on_timeout(deadline - 1).unwrap() {
            RoundChange::NotDue { remaining_ms } => assert_eq!(remaining_ms, 1),
            other => panic!("unerwartet: {:?}", other),
        }
        assert_eq!(d.round(), 0);
    }

    #[test]
    fn leader_ausfall_wechselt_die_runde() {
        // Der Kern von 3.6: ohne Propose laeuft die Frist ab und die
        // Runde geht weiter, statt stehenzubleiben.
        let mut d = driver(4);
        let deadline = d.deadline_ms();
        match d.on_timeout(deadline).unwrap() {
            RoundChange::Advanced { from, to, leader } => {
                assert_eq!(from, 0);
                assert_eq!(to, 1);
                assert_eq!(leader, test_miner(1));
            }
            other => panic!("unerwartet: {:?}", other),
        }
        assert_eq!(d.round(), 1);
        assert_eq!(d.status(), RoundStatus::WaitingPropose);
    }

    #[test]
    fn leader_rotiert_ueber_die_producer() {
        let mut d = driver(4);
        let mut gesehen = vec![d.leader()];
        let mut t = 0u64;
        for _ in 0..4 {
            t = d.deadline_ms();
            d.on_timeout(t).unwrap();
            gesehen.push(d.leader());
        }
        assert_eq!(
            gesehen,
            vec![
                test_miner(0),
                test_miner(1),
                test_miner(2),
                test_miner(3),
                test_miner(0),
            ]
        );
        assert!(t > 0);
    }

    #[test]
    fn spaetere_runden_haben_laengere_fristen() {
        let mut d = driver(4);
        let erste = d.deadline_ms();
        let t = d.deadline_ms();
        d.on_timeout(t).unwrap();
        let zweite_dauer = d.deadline_ms() - t;
        assert!(zweite_dauer > erste, "{} !> {}", zweite_dauer, erste);
    }

    #[test]
    fn commitete_runde_wechselt_nicht_mehr() {
        let mut d = driver(4);
        let h = test_hash(1);
        d.receive_propose(&signed_propose(0, h, 0), None, 0).unwrap();
        for i in 0..4 {
            d.receive_vote(&signed_vote(0, h, i), 0).unwrap();
        }
        for i in 0..4 {
            d.receive_commit(&signed_commit(0, h, i), 0).unwrap();
        }
        assert!(d.is_committed());
        assert_eq!(d.on_timeout(u64::MAX).unwrap(), RoundChange::AlreadyCommitted);
        assert_eq!(d.round(), 0);
    }

    // ── Zertifikate ─────────────────────────────────────────────────

    #[test]
    fn zertifikat_aus_quorum_ist_gueltig() {
        let d = driver(4);
        let cert = polka(0, test_hash(1), 4);
        assert!(cert.verify(d.state().voting_set()).is_ok());
    }

    #[test]
    fn zertifikat_unter_quorum_wird_abgelehnt() {
        // 2 von 4 x 100 = 200 < 267.
        let d = driver(4);
        let cert = polka(0, test_hash(1), 2);
        assert_eq!(
            cert.verify(d.state().voting_set()).unwrap_err(),
            RoundError::CertificateBelowQuorum
        );
    }

    #[test]
    fn zertifikat_mit_doppeltem_unterzeichner_wird_abgelehnt() {
        // Ohne Duplikatschutz erreichte ein einzelner Schluessel das
        // Quorum, indem er dieselbe Stimme mehrfach einreicht.
        let h = test_hash(1);
        let votes = vec![
            signed_vote(0, h, 0),
            signed_vote(0, h, 0),
            signed_vote(0, h, 1),
        ];
        assert_eq!(
            PolkaCertificate::from_votes(&votes).unwrap_err(),
            RoundError::NonCanonicalCertificate
        );
    }

    #[test]
    fn unsortiertes_zertifikat_wird_abgelehnt() {
        let d = driver(4);
        let mut cert = polka(0, test_hash(1), 4);
        cert.voters.reverse();
        assert_eq!(
            cert.verify(d.state().voting_set()).unwrap_err(),
            RoundError::NonCanonicalCertificate
        );
    }

    #[test]
    fn zertifikat_mit_fremdem_unterzeichner_wird_abgelehnt() {
        let d = driver(4);
        let h = test_hash(1);
        // Unterzeichner 5 ist nicht im Vier-Mitglieder-Komitee.
        let votes: Vec<Vote> = (0..4)
            .map(|i| signed_vote(0, h, i))
            .chain(std::iter::once(signed_vote(0, h, 5)))
            .collect();
        let cert = PolkaCertificate::from_votes(&votes).unwrap();
        assert_eq!(
            cert.verify(d.state().voting_set()).unwrap_err(),
            RoundError::CertificateSignerNotInCommittee
        );
    }

    #[test]
    fn zertifikat_mit_gefaelschtem_block_wird_abgelehnt() {
        let d = driver(4);
        let mut cert = polka(0, test_hash(1), 4);
        // Signaturen gelten fuer test_hash(1), nicht fuer test_hash(2).
        cert.block_hash = test_hash(2);
        assert_eq!(
            cert.verify(d.state().voting_set()).unwrap_err(),
            RoundError::InvalidSignature
        );
    }

    #[test]
    fn zertifikat_aus_gemischten_runden_wird_abgelehnt() {
        let h = test_hash(1);
        let votes = vec![signed_vote(0, h, 0), signed_vote(1, h, 1)];
        assert_eq!(
            PolkaCertificate::from_votes(&votes).unwrap_err(),
            RoundError::InconsistentCertificate
        );
    }

    #[test]
    fn leeres_zertifikat_wird_abgelehnt() {
        assert_eq!(
            PolkaCertificate::from_votes(&[]).unwrap_err(),
            RoundError::EmptyCertificate
        );
    }

    // ── Sperre ──────────────────────────────────────────────────────

    #[test]
    fn quorum_an_votes_erzeugt_die_sperre() {
        let mut d = driver(4);
        let h = test_hash(1);
        d.receive_propose(&signed_propose(0, h, 0), None, 0).unwrap();
        assert!(d.lock().is_none());

        d.receive_vote(&signed_vote(0, h, 0), 0).unwrap();
        d.receive_vote(&signed_vote(0, h, 1), 0).unwrap();
        assert!(d.lock().is_none(), "200 von 400 ist kein Quorum");

        d.receive_vote(&signed_vote(0, h, 2), 0).unwrap();
        assert_eq!(
            d.lock(),
            Some(Lock {
                block_hash: h,
                round: 0
            })
        );
    }

    // ── Punkt 23, zweite Hälfte: der Sprung nach vorn ───────────────

    /// Neun Validatoren zu je 100: Gesamtgewicht 900, Schranke 301.
    /// **Drei sind genau ein Drittel und reichen nicht**, der vierte
    /// kippt es.
    #[test]
    fn mehr_als_ein_drittel_laesst_den_zurueckgefallenen_springen() {
        let mut d = driver(9);
        let h = test_hash(7);
        assert_eq!(d.round(), 0);

        for i in 0..3u8 {
            let r = d
                .merke_hoehere_runde(&Konsensnachricht::Vote(signed_vote(5, h, i)), 1_000)
                .expect("echte Stimme");
            assert_eq!(
                r,
                RoundChange::Vorgemerkt {
                    runde: 5,
                    gewicht: 100 * u64::from(i + 1),
                    schranke: 301,
                }
            );
        }
        assert_eq!(d.round(), 0, "genau ein Drittel darf nicht reichen");

        let r = d
            .merke_hoehere_runde(&Konsensnachricht::Vote(signed_vote(5, h, 3)), 1_000)
            .expect("echte Stimme");
        assert_eq!(
            r,
            RoundChange::Advanced {
                from: 0,
                to: 5,
                leader: select_leader(5, &producers(9)).unwrap(),
            }
        );
        assert_eq!(d.round(), 5);
    }

    /// ⚑ **Die Gegenprobe, und sie ist der Grund für die Reihenfolge.**
    ///
    /// `BftState::receive_vote` lehnt eine fremde Runde ab, bevor es die
    /// Signatur prüft. Wer die abgelehnten Nachrichten ungeprüft zählte,
    /// gäbe einem einzelnen Byzantiner die Macht, jeden ehrlichen Knoten
    /// in jede Runde zu treiben: Er müsste nur vier Stimmen mit fremden
    /// Absendernamen und seiner eigenen Unterschrift schicken.
    ///
    /// Hier ist genau das nachgestellt. Es darf nichts bewirken.
    #[test]
    fn eine_gefaelschte_unterschrift_treibt_die_runde_nicht() {
        let mut d = driver(9);
        let h = test_hash(7);
        let echt = signed_vote(5, h, 0);

        // Vier echte Stimmen würden springen lassen (400 > 301).
        for i in 0..4u8 {
            let gefaelscht = Vote {
                voter: test_miner(i),
                ..echt.clone()
            };
            let r = d.merke_hoehere_runde(&Konsensnachricht::Vote(gefaelscht), 1_000);
            if i == 0 {
                r.expect("die eine echte Stimme gilt");
            } else {
                assert_eq!(
                    r.unwrap_err(),
                    RoundError::Bft(BftError::InvalidSignature),
                    "fremder Absender mit fremder Unterschrift muss auffallen"
                );
            }
        }
        assert_eq!(
            d.round(),
            0,
            "ein einzelner Byzantiner hat die Runde getrieben"
        );
    }

    /// Ein Absender, der zweimal schickt, zählt einmal. Sonst käme
    /// derselbe Knoten allein über die Schranke.
    #[test]
    fn derselbe_absender_zaehlt_nur_einmal() {
        let mut d = driver(9);
        let h = test_hash(7);
        for _ in 0..5 {
            d.merke_hoehere_runde(&Konsensnachricht::Vote(signed_vote(5, h, 0)), 1_000)
                .expect("echte Stimme");
        }
        assert_eq!(d.round(), 0);
    }

    /// ⛑ **Hier stand das Gegenteil, und es war falsch.**
    ///
    /// Der erste Entwurf zählte je Runde getrennt; dieser Test hielt
    /// fest, dass zwei Belege aus Runde 4 und zwei aus Runde 5 nichts
    /// bewirken. **Das unterzählt.** Wer für Runde 5 unterschreibt, hat
    /// Runde 4 hinter sich; vier Knoten in Runde mindestens 4 sind ein
    /// Beleg für Runde 4, und der Sprung dorthin ist richtig.
    ///
    /// Gesprungen wird auf **4**, nicht auf 5: Für 5 stehen nur 200 von
    /// 900, und die Schranke liegt bei 301.
    #[test]
    fn belege_verschiedener_runden_belegen_die_niedrigere() {
        let mut d = driver(9);
        let h = test_hash(7);
        for i in 0..2u8 {
            d.merke_hoehere_runde(&Konsensnachricht::Vote(signed_vote(4, h, i)), 1_000)
                .expect("echte Stimme");
        }
        // Der dritte steht in Runde 5 und belegt damit auch Runde 4.
        d.merke_hoehere_runde(&Konsensnachricht::Vote(signed_vote(5, h, 2)), 1_000)
            .expect("echte Stimme");
        assert_eq!(d.round(), 0, "300 von 900 sind genau ein Drittel");

        d.merke_hoehere_runde(&Konsensnachricht::Vote(signed_vote(5, h, 3)), 1_000)
            .expect("echte Stimme");
        assert_eq!(d.round(), 4, "vier Knoten stehen in Runde mindestens 4");
    }

    /// ⚑ **Ein flutender Stimmberechtigter darf den Speicher nicht
    /// treiben.**
    ///
    /// Die Signaturprüfung hält Fremde draußen, **nicht Mitglieder**.
    /// Der erste Entwurf legte je Runde einen Eintrag an; ein einzelner
    /// Byzantiner hätte mit gültigen Unterschriften für eine Million
    /// Runden eine Million Einträge erzeugt. Der Schlüssel ist deshalb
    /// der Absender: Die Karte ist durch die stimmberechtigte Menge
    /// begrenzt, und Fluten hebt nur den eigenen Eintrag.
    #[test]
    fn ein_flutender_absender_belegt_nur_seinen_eigenen_eintrag() {
        let mut d = driver(9);
        let h = test_hash(7);
        for runde in 1..500u64 {
            d.merke_hoehere_runde(&Konsensnachricht::Vote(signed_vote(runde, h, 0)), 1_000)
                .expect("echte Stimme");
        }
        assert_eq!(
            d.hoechste_runde_je_absender.len(),
            1,
            "je Absender ein Eintrag, sonst wächst die Karte mit den Runden"
        );
        assert_eq!(d.round(), 0, "einer allein trägt kein Drittel");
    }

    /// Springt es, dann auf die **höchste** belegte Runde. Auf die
    /// niedrigere zu springen hieße, gleich darauf erneut zu springen.
    #[test]
    fn der_sprung_geht_auf_die_hoechste_belegte_runde() {
        let mut d = driver(4);
        let h = test_hash(7);
        // Vier zu je 100: Schranke 134, zwei Absender genügen.
        for i in 0..2u8 {
            d.merke_hoehere_runde(&Konsensnachricht::Vote(signed_vote(3, h, i)), 1_000)
                .expect("echte Stimme");
        }
        assert_eq!(d.round(), 3);
        for i in 0..2u8 {
            d.merke_hoehere_runde(&Konsensnachricht::Commit(signed_commit(9, h, i)), 2_000)
                .expect("echter Commit");
        }
        assert_eq!(d.round(), 9);
    }

    /// ⚑ **Die Sperre überlebt den Sprung**, genau wie den Wechsel über
    /// die Frist. Täte sie es nicht, wäre aus einer Liveness-Regel ein
    /// Sicherheitsloch geworden.
    #[test]
    fn die_sperre_ueberlebt_den_sprung() {
        let mut d = driver(4);
        let h = test_hash(1);
        d.receive_propose(&signed_propose(0, h, 0), None, 0).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(0, h, i), 0).unwrap();
        }
        let vorher = d.lock().expect("Sperre steht");

        for i in 0..2u8 {
            d.merke_hoehere_runde(&Konsensnachricht::Vote(signed_vote(6, test_hash(2), i)), 1_000)
                .expect("echte Stimme");
        }
        assert_eq!(d.round(), 6);
        assert_eq!(d.lock(), Some(vorher), "der Sprung hat entsperrt");
    }

    /// Die eigene und jede niedrigere Runde sagen nichts über einen
    /// Rückstand.
    #[test]
    fn die_eigene_runde_ist_unerheblich() {
        let mut d = driver(4);
        let h = test_hash(7);
        let r = d
            .merke_hoehere_runde(&Konsensnachricht::Vote(signed_vote(0, h, 0)), 1_000)
            .expect("gültige Nachricht");
        assert_eq!(r, RoundChange::Unerheblich);
        assert_eq!(d.round(), 0);
    }

    /// Ein Propose trägt nie genug Gewicht, es gibt je Runde nur einen.
    #[test]
    fn ein_propose_zaehlt_nicht_mit() {
        let mut d = driver(4);
        let h = test_hash(7);
        let r = d
            .merke_hoehere_runde(&Konsensnachricht::Propose(signed_propose(5, h, 1)), 1_000)
            .expect("gültige Nachricht");
        assert_eq!(r, RoundChange::Unerheblich);
    }

    /// Wer commitet hat, springt nicht mehr. Seine Runde ist entschieden,
    /// und ein Sprung würfe die Entscheidung weg.
    #[test]
    fn nach_dem_commit_wird_nicht_mehr_gesprungen() {
        let mut d = driver(4);
        let h = test_hash(1);
        d.receive_propose(&signed_propose(0, h, 0), None, 0).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(0, h, i), 0).unwrap();
        }
        for i in 0..3 {
            d.receive_commit(&signed_commit(0, h, i), 0).unwrap();
        }
        assert!(d.is_committed());

        for i in 0..3u8 {
            let r = d
                .merke_hoehere_runde(&Konsensnachricht::Vote(signed_vote(7, h, i)), 1_000)
                .expect("gültige Nachricht");
            assert_eq!(r, RoundChange::AlreadyCommitted);
        }
        assert_eq!(d.round(), 0);
    }

    /// Genau ein Drittel reicht nicht, und das ist keine Kleinigkeit:
    /// Bei genau einem Drittel könnten alle Absender byzantinisch sein.
    #[test]
    fn drittel_schranke_ist_strikt() {
        assert_eq!(voting_set(9, 100).drittel_schranke(), 301);
        assert_eq!(voting_set(3, 100).drittel_schranke(), 101);
        // Nicht teilbar: 100/3 = 33, die kleinste ganze Zahl darüber
        // ist 34.
        assert_eq!(voting_set(1, 100).drittel_schranke(), 34);
    }

    #[test]
    fn sperre_ueberlebt_den_rundenwechsel() {
        let mut d = driver(4);
        let h = test_hash(1);
        d.receive_propose(&signed_propose(0, h, 0), None, 0).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(0, h, i), 0).unwrap();
        }
        let vorher = d.lock().unwrap();
        let t = d.deadline_ms();
        d.on_timeout(t).unwrap();
        assert_eq!(d.round(), 1);
        assert_eq!(d.lock(), Some(vorher));
    }

    #[test]
    fn gesperrter_validator_lehnt_anderen_block_ohne_beweis_ab() {
        // Das Szenario aus der Modul-Dokumentation: ohne diese Ablehnung
        // koennten zwei verschiedene Bloecke auf derselben Hoehe
        // commitet werden.
        let mut d = driver(4);
        let a = test_hash(1);
        let b = test_hash(2);
        d.receive_propose(&signed_propose(0, a, 0), None, 0).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(0, a, i), 0).unwrap();
        }
        let t = d.deadline_ms();
        d.on_timeout(t).unwrap();

        let err = d
            .receive_propose(&signed_propose(1, b, 1), None, t)
            .unwrap_err();
        assert_eq!(
            err,
            RoundError::Locked {
                locked_on: a,
                locked_round: 0
            }
        );
        // Der abgelehnte Vorschlag darf den Zustand nicht beruehrt haben.
        assert!(d.state().proposed_block.is_none());
        assert_eq!(d.lock().unwrap().block_hash, a);
    }

    #[test]
    fn gesperrter_validator_stimmt_weiter_fuer_denselben_block() {
        let mut d = driver(4);
        let a = test_hash(1);
        d.receive_propose(&signed_propose(0, a, 0), None, 0).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(0, a, i), 0).unwrap();
        }
        let t = d.deadline_ms();
        d.on_timeout(t).unwrap();
        assert!(d.receive_propose(&signed_propose(1, a, 1), None, t).is_ok());
    }

    #[test]
    fn beweis_aus_spaeterer_runde_entsperrt() {
        let mut d = driver(4);
        let a = test_hash(1);
        let b = test_hash(2);
        d.receive_propose(&signed_propose(0, a, 0), None, 0).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(0, a, i), 0).unwrap();
        }
        // Zwei Rundenwechsel: Sperre in Runde 0, Zertifikat aus Runde 1,
        // Vorschlag in Runde 2.
        let mut t = d.deadline_ms();
        d.on_timeout(t).unwrap();
        t = d.deadline_ms();
        d.on_timeout(t).unwrap();
        assert_eq!(d.round(), 2);

        let cert = polka(1, b, 4);
        d.receive_propose(&signed_propose_pol(2, b, 2, cert.round), Some(&cert), t)
            .unwrap();
        // Entsperrt heisst umgesetzt, nicht entfernt.
        assert_eq!(
            d.lock(),
            Some(Lock {
                block_hash: b,
                round: 1
            })
        );
    }

    #[test]
    fn beweis_aus_zu_frueher_runde_entsperrt_nicht() {
        let mut d = driver(4);
        let a = test_hash(1);
        let b = test_hash(2);
        // Sperre entsteht in Runde 1.
        let t0 = d.deadline_ms();
        d.on_timeout(t0).unwrap();
        d.receive_propose(&signed_propose(1, a, 1), None, t0).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(1, a, i), t0).unwrap();
        }
        assert_eq!(d.lock().unwrap().round, 1);

        let t1 = d.deadline_ms();
        d.on_timeout(t1).unwrap();
        // Zertifikat aus Runde 0 — aelter als die Sperre, also wertlos.
        let cert = polka(0, b, 4);
        let err = d
            .receive_propose(&signed_propose(2, b, 2), Some(&cert), t1)
            .unwrap_err();
        assert_eq!(
            err,
            RoundError::CertificateRoundNotUsable {
                certificate_round: 0,
                locked_round: 1,
                current_round: 2
            }
        );
        assert_eq!(d.lock().unwrap().block_hash, a);
    }

    #[test]
    fn beweis_fuer_anderen_block_als_der_vorschlag_wird_abgelehnt() {
        let mut d = driver(4);
        let a = test_hash(1);
        let b = test_hash(2);
        let c = test_hash(3);
        d.receive_propose(&signed_propose(0, a, 0), None, 0).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(0, a, i), 0).unwrap();
        }
        let mut t = d.deadline_ms();
        d.on_timeout(t).unwrap();
        t = d.deadline_ms();
        d.on_timeout(t).unwrap();

        // Zertifikat belegt c, vorgeschlagen wird b.
        let cert = polka(1, c, 4);
        assert_eq!(
            d.receive_propose(&signed_propose(2, b, 2), Some(&cert), t)
                .unwrap_err(),
            RoundError::CertificateBlockMismatch
        );
    }

    #[test]
    fn beweis_unter_quorum_entsperrt_nicht() {
        let mut d = driver(4);
        let a = test_hash(1);
        let b = test_hash(2);
        d.receive_propose(&signed_propose(0, a, 0), None, 0).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(0, a, i), 0).unwrap();
        }
        let mut t = d.deadline_ms();
        d.on_timeout(t).unwrap();
        t = d.deadline_ms();
        d.on_timeout(t).unwrap();

        let cert = polka(1, b, 2);
        assert_eq!(
            d.receive_propose(&signed_propose(2, b, 2), Some(&cert), t)
                .unwrap_err(),
            RoundError::CertificateBelowQuorum
        );
        assert_eq!(d.lock().unwrap().block_hash, a);
    }

    #[test]
    fn zertifikat_aus_der_laufenden_runde_entsperrt_nicht() {
        // Es muesste dann bereits ein Quorum geben, das der Vorschlag
        // erst herbeifuehren soll.
        let mut d = driver(4);
        let a = test_hash(1);
        let b = test_hash(2);
        d.receive_propose(&signed_propose(0, a, 0), None, 0).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(0, a, i), 0).unwrap();
        }
        let t = d.deadline_ms();
        d.on_timeout(t).unwrap();

        let cert = polka(1, b, 4);
        assert_eq!(
            d.receive_propose(&signed_propose(1, b, 1), Some(&cert), t)
                .unwrap_err(),
            RoundError::CertificateRoundNotUsable {
                certificate_round: 1,
                locked_round: 0,
                current_round: 1
            }
        );
    }

    #[test]
    fn apply_polka_entsperrt_ohne_vorschlag() {
        let mut d = driver(4);
        let a = test_hash(1);
        let b = test_hash(2);
        d.receive_propose(&signed_propose(0, a, 0), None, 0).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(0, a, i), 0).unwrap();
        }
        let cert = polka(1, b, 4);
        assert!(d.apply_polka(&cert).unwrap());
        assert_eq!(
            d.lock(),
            Some(Lock {
                block_hash: b,
                round: 1
            })
        );
    }

    #[test]
    fn apply_polka_aus_alter_runde_veraendert_nichts() {
        let mut d = driver(4);
        let a = test_hash(1);
        let b = test_hash(2);
        let t = d.deadline_ms();
        d.on_timeout(t).unwrap();
        d.receive_propose(&signed_propose(1, a, 1), None, t).unwrap();
        for i in 0..3 {
            d.receive_vote(&signed_vote(1, a, i), t).unwrap();
        }
        let cert = polka(0, b, 4);
        assert!(!d.apply_polka(&cert).unwrap());
        assert_eq!(d.lock().unwrap().block_hash, a);
    }

    // ── Liveness ────────────────────────────────────────────────────

    #[test]
    fn nach_leader_ausfaellen_wird_trotzdem_commitet() {
        // Die Akzeptanz-Eigenschaft von 3.6: drei Leader fallen aus, der
        // vierte liefert, und das Protokoll kommt zum Abschluss.
        let mut d = driver(4);
        let mut t = 0u64;
        for _ in 0..3 {
            t = d.deadline_ms();
            assert!(matches!(
                d.on_timeout(t).unwrap(),
                RoundChange::Advanced { .. }
            ));
        }
        assert_eq!(d.round(), 3);
        assert_eq!(d.leader(), test_miner(3));

        let h = test_hash(7);
        d.receive_propose(&signed_propose(3, h, 3), None, t).unwrap();
        for i in 0..4 {
            d.receive_vote(&signed_vote(3, h, i), t).unwrap();
        }
        for i in 0..4 {
            d.receive_commit(&signed_commit(3, h, i), t).unwrap();
        }
        assert!(d.is_committed());
        assert_eq!(d.committed_block(), Some(h));
    }

    #[test]
    fn frist_wird_bei_phasenwechsel_neu_gesetzt() {
        let mut d = driver(4);
        let h = test_hash(1);
        let vor_propose = d.deadline_ms();
        d.receive_propose(&signed_propose(0, h, 0), None, 10_000)
            .unwrap();
        assert_eq!(d.status(), RoundStatus::CollectingVotes);
        assert_ne!(d.deadline_ms(), vor_propose);
        assert_eq!(
            d.deadline_ms(),
            10_000 + TimeoutConfig::default().for_status(RoundStatus::CollectingVotes, 0)
        );
    }

    #[test]
    fn zustand_ist_deterministisch_ueber_zwei_laeufe() {
        // Kap. 10.3: derselbe Nachrichtenverlauf muss auf jedem Knoten
        // denselben Zustand ergeben.
        let lauf = || {
            let mut d = driver(4);
            let h = test_hash(1);
            let t = d.deadline_ms();
            d.on_timeout(t).unwrap();
            d.receive_propose(&signed_propose(1, h, 1), None, t).unwrap();
            for i in 0..3 {
                d.receive_vote(&signed_vote(1, h, i), t).unwrap();
            }
            (d.round(), d.lock(), d.deadline_ms(), d.state().vote_weight())
        };
        assert_eq!(lauf(), lauf());
    }
    // ── Commit-Zertifikat (⚑ Fund 67) ───────────────────────────────

    #[test]
    fn commitzertifikat_mit_quorum_gilt() {
        let vs = voting_set(4, 100);
        assert_eq!(commitzert(3, test_hash(9), 3).verify(&vs), Ok(()));
    }

    #[test]
    fn commitzertifikat_unter_quorum_gilt_nicht() {
        let vs = voting_set(4, 100);
        // Zwei von vier sind nicht mehr als zwei Drittel.
        assert_eq!(
            commitzert(3, test_hash(9), 2).verify(&vs),
            Err(RoundError::CertificateBelowQuorum)
        );
    }

    /// ⚑ **Ein Polka ist kein Commit-Beleg.**
    ///
    /// Beide Zertifikate haben dieselbe Gestalt: Runde, Block,
    /// Unterzeichnerliste, Aggregat. Ohne getrennte Präfixe in der
    /// Signierbotschaft ließe sich das eine als das andere ausgeben, und
    /// ein Vote-Quorum, das nur „wir könnten" heißt, stünde plötzlich für
    /// „wir haben entschieden". Der Test setzt das Aggregat eines echten
    /// Polka in ein Commit-Zertifikat um und erwartet, dass es auffliegt.
    #[test]
    fn ein_polka_geht_nicht_als_commit_beleg_durch() {
        let vs = voting_set(4, 100);
        let p = polka(3, test_hash(9), 3);
        assert_eq!(p.verify(&vs), Ok(()), "der Polka selbst ist gültig");

        let getarnt = Commitzertifikat {
            round: p.round,
            block_hash: p.block_hash,
            committers: p.voters.clone(),
            aggregate: p.aggregate,
        };
        assert_eq!(getarnt.verify(&vs), Err(RoundError::InvalidSignature));
    }

    #[test]
    fn commitzertifikat_mit_doppeltem_unterzeichner_gilt_nicht() {
        let vs = voting_set(4, 100);
        let mut z = commitzert(3, test_hash(9), 3);
        z.committers[1] = z.committers[0];
        assert_eq!(z.verify(&vs), Err(RoundError::NonCanonicalCertificate));
    }

    #[test]
    fn commitzertifikat_von_fremden_gilt_nicht() {
        // Der Beleg entsteht unter vier Schlüsseln, geprüft wird gegen
        // eine Menge, die nur drei davon kennt.
        let vs = voting_set(3, 100);
        assert_eq!(
            commitzert(3, test_hash(9), 4).verify(&vs),
            Err(RoundError::CertificateSignerNotInCommittee)
        );
    }

    #[test]
    fn commitzertifikat_aus_uneinheitlichen_commits_entsteht_nicht() {
        let commits = vec![
            signed_commit(3, test_hash(9), 0),
            signed_commit(3, test_hash(8), 1),
        ];
        assert_eq!(
            Commitzertifikat::from_commits(&commits).unwrap_err(),
            RoundError::InconsistentCertificate
        );
        assert_eq!(
            Commitzertifikat::from_commits(&[]).unwrap_err(),
            RoundError::EmptyCertificate
        );
    }

    /// ⚑ **Der Rückweg aus Fund 67, auf der Ebene des Treibers.**
    ///
    /// Der Treiber steht in Runde 7 und hat dort nichts erreicht. Der
    /// Beleg stammt aus Runde 0. Ein einzelner Commit aus Runde 0 würde
    /// hier mit `WrongRound` abprallen; der Beleg gilt.
    #[test]
    fn ein_beleg_aus_einer_alten_runde_holt_den_treiber_zurueck() {
        let mut d = driver(4);
        for _ in 0..7 {
            d.advance_round(0).expect("Wechsel");
        }
        assert_eq!(d.round(), 7);
        assert!(!d.is_committed());

        // Zum Vergleich: die einzelne Nachricht prallt ab.
        assert_eq!(
            d.receive_commit(&signed_commit(0, test_hash(9), 1), 0),
            Err(RoundError::Bft(BftError::WrongRound {
                expected: 7,
                got: 0
            }))
        );

        assert_eq!(d.apply_commitzertifikat(&commitzert(0, test_hash(9), 3)), Ok(true));
        assert!(d.is_committed());
        assert_eq!(d.committed_block(), Some(test_hash(9)));
        // Die Rundennummer bleibt, wo sie war: Übernommen wird die
        // Entscheidung, nicht die Runde.
        assert_eq!(d.round(), 7);
    }

    #[test]
    fn ein_zweiter_beleg_ueber_dieselbe_entscheidung_aendert_nichts() {
        let mut d = driver(4);
        assert_eq!(d.apply_commitzertifikat(&commitzert(0, test_hash(9), 3)), Ok(true));
        assert_eq!(d.apply_commitzertifikat(&commitzert(0, test_hash(9), 4)), Ok(false));
        assert!(d.is_committed());
    }

    /// ⚑ **Der billige Weg wird auch wirklich genommen.**
    ///
    /// Ein Beleg über die schon getroffene Entscheidung darf keine
    /// Aggregat-Verifikation kosten, sonst zahlt jeder Knoten für jede
    /// überzählige Kopie. Nachgewiesen mit einem Beleg, dessen Aggregat
    /// **kaputt** ist: Käme er bis zur Prüfung, wäre das Ergebnis
    /// `InvalidSignature` statt `Ok(false)`.
    #[test]
    fn ein_ueberzaehliger_beleg_wird_nicht_geprueft() {
        let mut d = driver(4);
        assert_eq!(d.apply_commitzertifikat(&commitzert(0, test_hash(9), 3)), Ok(true));

        let mut kaputt = commitzert(0, test_hash(9), 3);
        kaputt.aggregate = polka(0, test_hash(9), 3).aggregate;
        assert_eq!(d.apply_commitzertifikat(&kaputt), Ok(false));
    }

    /// ⚑ **Zwei Quoren für zwei Blöcke sind ein Sicherheitsbefund.**
    ///
    /// Das kann unter der Mehrheitsannahme nicht vorkommen. Wenn es
    /// vorkommt, ist die Annahme gebrochen, und der Aufrufer muss es
    /// erfahren, statt dass die Nachricht still verschwindet.
    #[test]
    fn zwei_belege_fuer_zwei_bloecke_melden_die_gabelung() {
        let mut d = driver(4);
        assert_eq!(d.apply_commitzertifikat(&commitzert(0, test_hash(9), 3)), Ok(true));
        assert_eq!(
            d.apply_commitzertifikat(&commitzert(1, test_hash(8), 3)),
            Err(RoundError::ConflictingCommit)
        );
        // Der eigene Stand bleibt, wo er war: Ein Widerspruch ist ein
        // Befund, kein Grund, die eigene Entscheidung umzuwerfen.
        assert_eq!(d.committed_block(), Some(test_hash(9)));
    }

    /// Und ein **erfundener** Widerspruch löst keinen Alarm aus.
    #[test]
    fn ein_ungeprueftes_zertifikat_loest_keinen_gabelungsalarm_aus() {
        let mut d = driver(4);
        assert_eq!(d.apply_commitzertifikat(&commitzert(0, test_hash(9), 3)), Ok(true));

        let mut erfunden = commitzert(1, test_hash(8), 3);
        erfunden.aggregate = polka(1, test_hash(8), 3).aggregate;
        assert_eq!(
            d.apply_commitzertifikat(&erfunden),
            Err(RoundError::InvalidSignature),
            "ein erfundener Beleg darf nicht als Gabelung gemeldet werden"
        );
    }

}
