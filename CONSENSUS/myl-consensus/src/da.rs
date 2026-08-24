//! Datenverfügbarkeits-Schicht — Whitepaper Kap. 3.5.3 (Punkt 4.3).
//!
//! Segmentdaten werden erasure-codiert abgelegt
//! ([`myl_types::erasure`], k=8/m=4) und über die **Streitfrist**
//! vorgehalten. Solange sie läuft, muss ein Bisektions-Spiel die
//! strittigen Daten anfordern können; danach dürfen sie verschwinden.
//!
//! ## Abgelaufen ist nicht dasselbe wie nicht vorhanden
//!
//! Das Akzeptanzkriterium der Phase verlangt für abgelaufene Epochen
//! **definiertes, nicht zufälliges Verhalten**. Der Unterschied ist
//! sicherheitsrelevant, nicht kosmetisch:
//!
//! Gäbe es nur eine Antwort „habe ich nicht", wäre ein Knoten, der
//! Daten **zurückhält**, von einem Knoten, dessen Daten **regulär
//! abgelaufen** sind, nicht zu unterscheiden. Zurückhalten wäre damit
//! folgenlos — man müsste nur behaupten, es sei alt.
//!
//! Deshalb prüft [`DaStore::fetch`] die Frist **vor** dem Nachschlagen
//! und antwortet [`DaError::Expired`], sobald die Epoche abgelaufen ist
//! — unabhängig davon, ob die Daten noch im Speicher liegen. Diese
//! Antwort ist von jedem Beteiligten **nachrechenbar**: Epoche und
//! Streitfrist sind öffentlich. Ein Knoten kann sich also nicht hinter
//! „abgelaufen" verstecken, solange die Frist läuft; dort bekommt er
//! [`DaError::FragmentMissing`], und das ist ein Vorwurf.
//!
//! Myelith ist quelloffen — ein Angreifer kennt diese Regel. Sie hält
//! trotzdem, weil sie nicht auf Nichtwissen beruht, sondern auf einer
//! Größe, die beide Seiten unabhängig ausrechnen können.
//!
//! ## Warum die Fragmente an ein Commitment gebunden sind
//!
//! Ein Speicher, der Fragmente ohne Bindung ausliefert, ist wertlos: Der
//! Anfragende kann nicht unterscheiden, ob er das Original bekommt oder
//! etwas Erfundenes. [`DaCommitment`] trägt deshalb eine Merkle-Wurzel
//! über die Fragmenthashes in Indexreihenfolge; [`DaStore::fetch`]
//! liefert zu jedem Fragment den Beweis mit, und [`DaStore::store`]
//! nimmt nur Fragmente an, die zur Wurzel passen.
//!
//! **Konsens-Feld:** Fragmentreihenfolge, Commitment-Konstruktion und
//! Fristberechnung sind Teil des Konsensvertrags. Änderungen nur über
//! Governance (Kap. 10.3).

use crate::epoch_close::DEFAULT_DISPUTE_EPOCHS;
use myl_types::erasure::{ErasureCoder, ErasureError, Fragment};
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, MerkleRoot, SegmentId};
use myl_types::merkle::{MerkleProof, MerkleTree};
use std::collections::BTreeMap;

/// Commitment über die Fragmente eines Segments.
///
/// Bindet Segment, Epoche, Parametrierung und die Fragmente selbst. Ohne
/// dieses Commitment wäre ein ausgeliefertes Fragment nicht überprüfbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaCommitment {
    /// Das Segment, zu dem die Fragmente gehören.
    pub segment_id: SegmentId,
    /// Epoche, in der das Segment gerechnet wurde — Grundlage der Frist.
    pub epoch: EpochId,
    /// Merkle-Wurzel über die Fragmenthashes in Indexreihenfolge.
    pub fragments_root: MerkleRoot,
    /// Datenfragmente.
    pub k: usize,
    /// Paritätsfragmente.
    pub m: usize,
    /// Ursprüngliche Länge der Nutzdaten vor dem Auffüllen.
    ///
    /// Muss mitgeführt werden: Die Codierung füllt auf ein Vielfaches
    /// von `k` auf, und ohne diese Zahl liefert die Rekonstruktion
    /// angehängte Nullen mit.
    pub original_len: usize,
}

/// Fehler der Datenverfügbarkeits-Schicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaError {
    /// Die Streitfrist der Epoche ist abgelaufen. **Definierte**
    /// Antwort, kein Fehlverhalten — und von jedem nachrechenbar.
    Expired {
        /// Epoche des Segments.
        epoch: u64,
        /// Aktuelle Epoche.
        current: u64,
        /// Streitfrist in Epochen.
        dispute_epochs: u64,
    },
    /// Zu diesem Segment liegt kein Commitment vor.
    UnknownSegment,
    /// Das Fragment fehlt, obwohl die Frist noch läuft. Innerhalb der
    /// Frist ist das ein Vorwurf, kein Normalzustand.
    FragmentMissing {
        /// Der angefragte Fragmentindex.
        index: usize,
    },
    /// Die übergebenen Fragmente passen nicht zur Commitment-Wurzel.
    CommitmentMismatch,
    /// Ein Fragmentindex liegt außerhalb von `0..k+m`.
    IndexOutOfRange {
        /// Der ungültige Index.
        index: usize,
    },
    /// Für dieses Segment liegt bereits ein Commitment vor.
    DuplicateSegment,
    /// Fehler aus der Erasure-Codierung.
    Erasure(ErasureError),
}

impl From<ErasureError> for DaError {
    fn from(e: ErasureError) -> Self {
        Self::Erasure(e)
    }
}

impl std::fmt::Display for DaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Expired {
                epoch,
                current,
                dispute_epochs,
            } => write!(
                f,
                "Streitfrist abgelaufen: Epoche {} + {} ≤ {}",
                epoch, dispute_epochs, current
            ),
            Self::UnknownSegment => write!(f, "Kein Commitment zu diesem Segment"),
            Self::FragmentMissing { index } => {
                write!(f, "Fragment {} fehlt bei laufender Frist", index)
            }
            Self::CommitmentMismatch => {
                write!(f, "Fragmente passen nicht zur Commitment-Wurzel")
            }
            Self::IndexOutOfRange { index } => {
                write!(f, "Fragmentindex {} außerhalb des gültigen Bereichs", index)
            }
            Self::DuplicateSegment => write!(f, "Segment bereits abgelegt"),
            Self::Erasure(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for DaError {}

/// Blattdaten eines Fragments für den Merkle-Baum.
///
/// Der Index geht mit ein: Ohne ihn wären zwei Fragmente mit gleichem
/// Inhalt austauschbar, und ein Speicher könnte Fragment 3 als Antwort
/// auf die Anfrage nach Fragment 7 ausliefern.
fn fragment_leaf(index: usize, data: &[u8]) -> Vec<u8> {
    let mut blatt = Vec::with_capacity(8 + data.len());
    blatt.extend_from_slice(&(index as u64).to_le_bytes());
    blatt.extend_from_slice(data);
    blatt
}

/// Codiert Segmentdaten und bildet das Commitment.
///
/// **Returns:** das Commitment und die Fragmente. Der Aufrufer verteilt
/// die Fragmente an die Pod-Mitglieder und veröffentlicht das
/// Commitment im Block.
pub fn commit_segment(
    segment_id: SegmentId,
    epoch: EpochId,
    coder: &ErasureCoder,
    data: &[u8],
) -> Result<(DaCommitment, Vec<Fragment>), DaError> {
    let fragmente = coder.encode(data)?;
    let blaetter: Vec<Vec<u8>> = fragmente
        .iter()
        .map(|f| fragment_leaf(f.index, &f.data))
        .collect();
    let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
    let baum = MerkleTree::new(&refs).map_err(|_| DaError::CommitmentMismatch)?;

    Ok((
        DaCommitment {
            segment_id,
            epoch,
            fragments_root: MerkleRoot::new(baum.root().0),
            k: coder.k(),
            m: coder.m(),
            original_len: data.len(),
        },
        fragmente,
    ))
}

/// Speicher der Datenverfügbarkeits-Schicht.
///
/// Hält Commitments und Fragmente, geordnet nach `(Epoche, Segment)` —
/// `BTreeMap`, damit das Aufräumen deterministisch in derselben
/// Reihenfolge läuft wie auf jedem anderen Knoten.
#[derive(Debug, Clone)]
pub struct DaStore {
    dispute_epochs: u64,
    commitments: BTreeMap<(u64, SegmentId), DaCommitment>,
    fragmente: BTreeMap<(u64, SegmentId), BTreeMap<usize, Vec<u8>>>,
}

impl Default for DaStore {
    fn default() -> Self {
        Self::new(DEFAULT_DISPUTE_EPOCHS)
    }
}

impl DaStore {
    /// Neuer Speicher mit der angegebenen Streitfrist in Epochen.
    pub fn new(dispute_epochs: u64) -> Self {
        Self {
            dispute_epochs,
            commitments: BTreeMap::new(),
            fragmente: BTreeMap::new(),
        }
    }

    /// Die konfigurierte Streitfrist in Epochen.
    pub fn dispute_epochs(&self) -> u64 {
        self.dispute_epochs
    }

    /// Ist die Frist für diese Epoche abgelaufen?
    ///
    /// Rein rechnerisch aus öffentlichen Größen — jeder Beteiligte kommt
    /// unabhängig zum selben Ergebnis. Genau das macht
    /// [`DaError::Expired`] überprüfbar und schließt „ich behaupte, es
    /// sei alt" als Ausrede aus.
    pub fn is_expired(&self, epoch: EpochId, current_epoch: EpochId) -> bool {
        current_epoch.0 >= epoch.0.saturating_add(self.dispute_epochs)
    }

    /// Legt Commitment und Fragmente ab.
    ///
    /// Prüft, dass die Fragmente zur Wurzel des Commitments passen —
    /// ein Speicher, der ungebundene Daten annimmt, könnte sie später
    /// nicht beweisbar ausliefern.
    ///
    /// **Fehler:** [`DaError::DuplicateSegment`],
    /// [`DaError::IndexOutOfRange`], [`DaError::CommitmentMismatch`].
    pub fn store(
        &mut self,
        commitment: &DaCommitment,
        fragmente: &[Fragment],
    ) -> Result<(), DaError> {
        let key = (commitment.epoch.0, commitment.segment_id);
        if self.commitments.contains_key(&key) {
            return Err(DaError::DuplicateSegment);
        }
        let n = commitment.k + commitment.m;
        if fragmente.len() != n {
            return Err(DaError::CommitmentMismatch);
        }
        for f in fragmente {
            if f.index >= n {
                return Err(DaError::IndexOutOfRange { index: f.index });
            }
        }

        // Wurzel nachrechnen. Die Fragmente werden dafür in
        // Indexreihenfolge gebracht — die Reihenfolge ist Teil der
        // Konstruktion, nicht des Zufalls.
        let mut sortiert: Vec<&Fragment> = fragmente.iter().collect();
        sortiert.sort_by_key(|f| f.index);
        let blaetter: Vec<Vec<u8>> = sortiert
            .iter()
            .map(|f| fragment_leaf(f.index, &f.data))
            .collect();
        let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
        let baum = MerkleTree::new(&refs).map_err(|_| DaError::CommitmentMismatch)?;
        if MerkleRoot::new(baum.root().0) != commitment.fragments_root {
            return Err(DaError::CommitmentMismatch);
        }

        let mut karte = BTreeMap::new();
        for f in sortiert {
            karte.insert(f.index, f.data.clone());
        }
        self.commitments.insert(key, commitment.clone());
        self.fragmente.insert(key, karte);
        Ok(())
    }

    /// Holt ein Fragment samt Merkle-Beweis.
    ///
    /// **Die Fristprüfung steht vor dem Nachschlagen.** Ist die Epoche
    /// abgelaufen, kommt [`DaError::Expired`] — auch dann, wenn die
    /// Daten noch im Speicher liegen. Nur so ist die Antwort für alle
    /// Beteiligten dieselbe und unabhängig davon, wann ein einzelner
    /// Knoten zuletzt aufgeräumt hat.
    pub fn fetch(
        &self,
        epoch: EpochId,
        segment_id: SegmentId,
        index: usize,
        current_epoch: EpochId,
    ) -> Result<(Vec<u8>, MerkleProof), DaError> {
        if self.is_expired(epoch, current_epoch) {
            return Err(DaError::Expired {
                epoch: epoch.0,
                current: current_epoch.0,
                dispute_epochs: self.dispute_epochs,
            });
        }
        let key = (epoch.0, segment_id);
        let commitment = self.commitments.get(&key).ok_or(DaError::UnknownSegment)?;
        let n = commitment.k + commitment.m;
        if index >= n {
            return Err(DaError::IndexOutOfRange { index });
        }
        let karte = self.fragmente.get(&key).ok_or(DaError::UnknownSegment)?;

        // Der Beweis braucht den ganzen Baum; fehlt auch nur ein
        // Fragment, ist er nicht bildbar.
        let mut blaetter = Vec::with_capacity(n);
        for i in 0..n {
            let daten = karte.get(&i).ok_or(DaError::FragmentMissing { index: i })?;
            blaetter.push(fragment_leaf(i, daten));
        }
        let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
        let baum = MerkleTree::new(&refs).map_err(|_| DaError::CommitmentMismatch)?;
        let beweis = baum
            .proof(index)
            .map_err(|_| DaError::IndexOutOfRange { index })?;
        Ok((karte[&index].clone(), beweis))
    }

    /// Prüft ein ausgeliefertes Fragment gegen ein Commitment.
    ///
    /// Die Gegenseite des Vertrauens: Wer ein Fragment empfängt, prüft
    /// es hiermit, ohne dem Speicher glauben zu müssen.
    pub fn verify_fragment(
        commitment: &DaCommitment,
        index: usize,
        data: &[u8],
        proof: &MerkleProof,
    ) -> bool {
        let wurzel = Hash(*commitment.fragments_root.as_bytes());
        proof.verify(&wurzel, &fragment_leaf(index, data), index as u64)
    }

    /// Rekonstruiert die Segmentdaten aus den vorhandenen Fragmenten.
    ///
    /// **Fehler:** [`DaError::Expired`] nach Fristablauf,
    /// [`ErasureError::NotEnoughFragments`] (als [`DaError::Erasure`]),
    /// wenn weniger als `k` Fragmente vorliegen — der definierte
    /// Ausfall.
    pub fn reconstruct(
        &self,
        epoch: EpochId,
        segment_id: SegmentId,
        current_epoch: EpochId,
    ) -> Result<Vec<u8>, DaError> {
        if self.is_expired(epoch, current_epoch) {
            return Err(DaError::Expired {
                epoch: epoch.0,
                current: current_epoch.0,
                dispute_epochs: self.dispute_epochs,
            });
        }
        let key = (epoch.0, segment_id);
        let commitment = self.commitments.get(&key).ok_or(DaError::UnknownSegment)?;
        let karte = self.fragmente.get(&key).ok_or(DaError::UnknownSegment)?;

        let coder = ErasureCoder::new(commitment.k, commitment.m)?;
        let vorhanden: Vec<Fragment> = karte
            .iter()
            .map(|(i, d)| Fragment {
                index: *i,
                data: d.clone(),
            })
            .collect();
        let mut daten = coder.decode(&vorhanden)?;
        daten.truncate(commitment.original_len);
        Ok(daten)
    }

    /// Entfernt ein einzelnes Fragment — für Ausfall-Simulationen und
    /// als Gegenstück zu partiellem Verlust im Betrieb.
    pub fn drop_fragment(&mut self, epoch: EpochId, segment_id: SegmentId, index: usize) -> bool {
        self.fragmente
            .get_mut(&(epoch.0, segment_id))
            .and_then(|k| k.remove(&index))
            .is_some()
    }

    /// Löscht alle Segmente, deren Streitfrist abgelaufen ist.
    ///
    /// **Returns:** Anzahl der gelöschten Segmente.
    ///
    /// Das Aufräumen ändert **nichts** an den Antworten: `fetch` und
    /// `reconstruct` liefern für abgelaufene Epochen ohnehin
    /// [`DaError::Expired`], ob aufgeräumt wurde oder nicht. Genau
    /// deshalb darf jeder Knoten zu einem anderen Zeitpunkt aufräumen,
    /// ohne dass das Protokollverhalten auseinanderläuft.
    pub fn prune(&mut self, current_epoch: EpochId) -> usize {
        let abgelaufen: Vec<(u64, SegmentId)> = self
            .commitments
            .keys()
            .filter(|(e, _)| self.is_expired(EpochId(*e), current_epoch))
            .copied()
            .collect();
        for key in &abgelaufen {
            self.commitments.remove(key);
            self.fragmente.remove(key);
        }
        abgelaufen.len()
    }

    /// Anzahl der abgelegten Segmente.
    pub fn len(&self) -> usize {
        self.commitments.len()
    }

    /// Ist nichts abgelegt?
    pub fn is_empty(&self) -> bool {
        self.commitments.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(b: u8) -> SegmentId {
        SegmentId::new([b; 32])
    }

    fn daten() -> Vec<u8> {
        (0..200u8).map(|i| i.wrapping_mul(13).wrapping_add(5)).collect()
    }

    /// Speicher mit einem abgelegten Segment aus Epoche 10.
    fn store_mit_segment() -> (DaStore, DaCommitment) {
        let coder = ErasureCoder::default();
        let (commitment, fragmente) =
            commit_segment(segment(1), EpochId(10), &coder, &daten()).expect("commit");
        let mut store = DaStore::default();
        store.store(&commitment, &fragmente).expect("store");
        (store, commitment)
    }

    // ── Commitment ──────────────────────────────────────────────────

    #[test]
    fn commitment_bindet_parameter_und_laenge() {
        let coder = ErasureCoder::default();
        let (c, f) = commit_segment(segment(1), EpochId(10), &coder, &daten()).expect("commit");
        assert_eq!((c.k, c.m), (8, 4));
        assert_eq!(c.original_len, 200);
        assert_eq!(f.len(), 12);
    }

    #[test]
    fn commitment_ist_deterministisch() {
        let coder = ErasureCoder::default();
        let a = commit_segment(segment(1), EpochId(10), &coder, &daten()).expect("a");
        let b = commit_segment(segment(1), EpochId(10), &coder, &daten()).expect("b");
        assert_eq!(a.0, b.0);
    }

    #[test]
    fn andere_daten_ergeben_andere_wurzel() {
        let coder = ErasureCoder::default();
        let mut d2 = daten();
        d2[0] ^= 1;
        let a = commit_segment(segment(1), EpochId(10), &coder, &daten()).expect("a");
        let b = commit_segment(segment(1), EpochId(10), &coder, &d2).expect("b");
        assert_ne!(a.0.fragments_root, b.0.fragments_root);
    }

    // ── Ablage ──────────────────────────────────────────────────────

    #[test]
    fn ablegen_und_wieder_zusammensetzen() {
        let (store, _) = store_mit_segment();
        let zurueck = store
            .reconstruct(EpochId(10), segment(1), EpochId(11))
            .expect("reconstruct");
        assert_eq!(zurueck, daten());
        assert_eq!(zurueck.len(), 200, "original_len muss die Auffüllung abschneiden");
    }

    #[test]
    fn verfaelschte_fragmente_werden_nicht_angenommen() {
        // Ein Speicher, der ungebundene Daten annimmt, koennte sie
        // spaeter nicht beweisbar ausliefern.
        let coder = ErasureCoder::default();
        let (commitment, mut fragmente) =
            commit_segment(segment(1), EpochId(10), &coder, &daten()).expect("commit");
        fragmente[3].data[0] ^= 1;
        let mut store = DaStore::default();
        assert_eq!(
            store.store(&commitment, &fragmente).unwrap_err(),
            DaError::CommitmentMismatch
        );
    }

    #[test]
    fn vertauschte_fragmentindizes_werden_erkannt() {
        // Der Index geht ins Merkle-Blatt ein: sonst waeren Fragmente
        // gleichen Inhalts austauschbar.
        let coder = ErasureCoder::default();
        let (commitment, mut fragmente) =
            commit_segment(segment(1), EpochId(10), &coder, &daten()).expect("commit");
        fragmente[2].data = fragmente[5].data.clone();
        let mut store = DaStore::default();
        assert_eq!(
            store.store(&commitment, &fragmente).unwrap_err(),
            DaError::CommitmentMismatch
        );
    }

    #[test]
    fn doppelte_ablage_wird_abgelehnt() {
        let coder = ErasureCoder::default();
        let (c, f) = commit_segment(segment(1), EpochId(10), &coder, &daten()).expect("commit");
        let mut store = DaStore::default();
        store.store(&c, &f).expect("erste");
        assert_eq!(store.store(&c, &f).unwrap_err(), DaError::DuplicateSegment);
    }

    // ── Auslieferung mit Beweis ─────────────────────────────────────

    #[test]
    fn ausgeliefertes_fragment_ist_beweisbar() {
        let (store, commitment) = store_mit_segment();
        for index in 0..12 {
            let (daten_f, beweis) = store
                .fetch(EpochId(10), segment(1), index, EpochId(11))
                .expect("fetch");
            assert!(
                DaStore::verify_fragment(&commitment, index, &daten_f, &beweis),
                "Fragment {}",
                index
            );
        }
    }

    #[test]
    fn beweis_traegt_nicht_fuer_verfaelschte_daten() {
        let (store, commitment) = store_mit_segment();
        let (mut daten_f, beweis) = store
            .fetch(EpochId(10), segment(1), 3, EpochId(11))
            .expect("fetch");
        daten_f[0] ^= 1;
        assert!(!DaStore::verify_fragment(&commitment, 3, &daten_f, &beweis));
    }

    #[test]
    fn beweis_traegt_nicht_unter_falschem_index() {
        let (store, commitment) = store_mit_segment();
        let (daten_f, beweis) = store
            .fetch(EpochId(10), segment(1), 3, EpochId(11))
            .expect("fetch");
        assert!(!DaStore::verify_fragment(&commitment, 4, &daten_f, &beweis));
    }

    // ── Teilausfall ─────────────────────────────────────────────────

    #[test]
    fn vier_ausfaelle_werden_getragen() {
        // k=8/m=4: ein Drittel darf fehlen.
        let (mut store, _) = store_mit_segment();
        for i in [0usize, 5, 9, 11] {
            assert!(store.drop_fragment(EpochId(10), segment(1), i));
        }
        let zurueck = store
            .reconstruct(EpochId(10), segment(1), EpochId(11))
            .expect("reconstruct");
        assert_eq!(zurueck, daten());
    }

    #[test]
    fn fuenf_ausfaelle_sind_ein_definierter_ausfall() {
        let (mut store, _) = store_mit_segment();
        for i in [0usize, 5, 9, 11, 2] {
            store.drop_fragment(EpochId(10), segment(1), i);
        }
        let err = store
            .reconstruct(EpochId(10), segment(1), EpochId(11))
            .unwrap_err();
        assert_eq!(
            err,
            DaError::Erasure(ErasureError::NotEnoughFragments { have: 7, need: 8 })
        );
    }

    #[test]
    fn fehlendes_fragment_bei_laufender_frist_ist_ein_vorwurf() {
        // Innerhalb der Frist ist „habe ich nicht" kein Normalzustand.
        let (mut store, _) = store_mit_segment();
        store.drop_fragment(EpochId(10), segment(1), 4);
        assert_eq!(
            store
                .fetch(EpochId(10), segment(1), 3, EpochId(11))
                .unwrap_err(),
            DaError::FragmentMissing { index: 4 }
        );
    }

    // ── Streitfrist ─────────────────────────────────────────────────

    /// Die Frist wird **gegen die Konstante** geprüft, nicht gegen eine
    /// getippte Zahl.
    ///
    /// Bis zum 2026-08-24 stand hier die 7 als Literal, und als sich die
    /// Konstante auf 168 korrigierte (⚑ Fund 50: 7 Epochen sind bei
    /// Stunden-Epochen 7 Stunden, nicht 7 Tage), schlug dieser Test fehl,
    /// ohne dass am geprüften Verhalten etwas falsch gewesen wäre.
    /// Geprüft gehört die **Regel** „ab Epoche + Frist ist abgelaufen",
    /// nicht der Zahlenwert der Frist.
    #[test]
    fn frist_laeuft_nach_der_streitfrist_ab() {
        let store = DaStore::default();
        let d = store.dispute_epochs();
        assert_eq!(d, DEFAULT_DISPUTE_EPOCHS);
        assert!(!store.is_expired(EpochId(10), EpochId(10 + d - 1)));
        assert!(store.is_expired(EpochId(10), EpochId(10 + d)));
        assert!(store.is_expired(EpochId(10), EpochId(10 + d + 1_000)));
    }

    #[test]
    fn abgelaufene_anfrage_bekommt_definierte_antwort() {
        // Das woertliche Akzeptanzkriterium der Phase.
        let (store, _) = store_mit_segment();
        let nach_ablauf = EpochId(10 + DEFAULT_DISPUTE_EPOCHS);
        let err = store
            .fetch(EpochId(10), segment(1), 3, nach_ablauf)
            .unwrap_err();
        assert_eq!(
            err,
            DaError::Expired {
                epoch: 10,
                current: nach_ablauf.0,
                dispute_epochs: DEFAULT_DISPUTE_EPOCHS
            }
        );
    }

    #[test]
    fn abgelaufen_gilt_auch_wenn_die_daten_noch_daliegen() {
        // Der Kern der Regel: Die Antwort haengt an der oeffentlich
        // nachrechenbaren Frist, nicht daran, wann ein Knoten zuletzt
        // aufgeraeumt hat. Sonst waere Zurueckhalten von regulaerem
        // Ablauf nicht zu unterscheiden.
        let (store, _) = store_mit_segment();
        let nach_ablauf = EpochId(10 + DEFAULT_DISPUTE_EPOCHS);
        assert_eq!(store.len(), 1, "die Daten liegen noch im Speicher");
        assert!(matches!(
            store.fetch(EpochId(10), segment(1), 3, nach_ablauf),
            Err(DaError::Expired { .. })
        ));
        assert!(matches!(
            store.reconstruct(EpochId(10), segment(1), nach_ablauf),
            Err(DaError::Expired { .. })
        ));
    }

    #[test]
    fn aufraeumen_aendert_die_antwort_nicht() {
        let (mut store, _) = store_mit_segment();
        let nach_ablauf = EpochId(10 + DEFAULT_DISPUTE_EPOCHS);
        let vor = store.fetch(EpochId(10), segment(1), 3, nach_ablauf).unwrap_err();
        assert_eq!(store.prune(nach_ablauf), 1);
        assert!(store.is_empty());
        let nach = store.fetch(EpochId(10), segment(1), 3, nach_ablauf).unwrap_err();
        assert_eq!(vor, nach, "Aufräumen darf das Verhalten nicht ändern");
    }

    #[test]
    fn aufraeumen_verschont_laufende_fristen() {
        let coder = ErasureCoder::default();
        let mut store = DaStore::default();
        for epoche in [10u64, 20] {
            let (c, f) =
                commit_segment(segment(epoche as u8), EpochId(epoche), &coder, &daten())
                    .expect("commit");
            store.store(&c, &f).expect("store");
        }
        // Nach Ablauf der Frist für Epoche 10, aber noch innerhalb der
        // für Epoche 20.
        let jetzt = EpochId(20 + DEFAULT_DISPUTE_EPOCHS - 1);
        assert_eq!(store.prune(jetzt), 1);
        assert_eq!(store.len(), 1);
        assert!(store.reconstruct(EpochId(20), segment(20), jetzt).is_ok());
    }

    #[test]
    fn unbekanntes_segment_bei_laufender_frist() {
        let store = DaStore::default();
        assert_eq!(
            store.fetch(EpochId(10), segment(9), 0, EpochId(11)).unwrap_err(),
            DaError::UnknownSegment
        );
    }

    #[test]
    fn frist_saettigt_bei_ueberlauf() {
        let store = DaStore::new(u64::MAX);
        assert!(!store.is_expired(EpochId(10), EpochId(u64::MAX - 1)));
    }

    #[test]
    fn index_ausserhalb_des_bereichs() {
        let (store, _) = store_mit_segment();
        assert_eq!(
            store.fetch(EpochId(10), segment(1), 12, EpochId(11)).unwrap_err(),
            DaError::IndexOutOfRange { index: 12 }
        );
    }
}
