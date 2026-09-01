//! Checker-Modul (audit) — Whitepaper Anhang A.4, Kap. 6.6.
//!
//! Rechnet VRF-ausgelöste Segmente nach und vergleicht das Ergebnis
//! mit dem behaupteten Hash. Bei Abweichung wird eine Challenge erzeugt.
//!
//! **Konsens-Feld:** Der Check-Algorithmus ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Das Checker-Modul definiert ein abstraktes Interface für
//! die Segment-Nachrechnung. Die konkrete Implementierung erfolgt durch
//! Integration mit INTEGER_LLMs Runtime (forward pass).

use myl_types::hash::Hash;
use myl_types::ids::SegmentId;

/// Fehler beim Segment-Check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckError {
    /// Segment ist leer.
    EmptySegment,
    /// Forward-Pass fehlgeschlagen.
    ForwardPassFailed,
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySegment => write!(f, "Segment ist leer"),
            Self::ForwardPassFailed => write!(f, "Forward-Pass fehlgeschlagen"),
        }
    }
}

impl std::error::Error for CheckError {}

/// Ergebnis eines Segment-Checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckResult {
    /// Segment stimmt überein (Hash match).
    Valid,
    /// Segment weicht ab (Hash mismatch).
    Invalid {
        /// Index der ersten abweichenden Layer.
        first_divergence: usize,
    },
}

/// Abstraktes Interface für Segment-Nachrechnung.
///
/// Dieses Trait definiert die Schnittstelle für die Nachrechnung eines
/// Segments. Die konkrete Implementierung erfolgt durch Integration mit
/// INTEGER_LLMs Runtime.
///
/// **Implementierungshinweis:** Der Forward-Pass muss deterministisch und
/// bitgleich sein (INTEGER_LLM θ_v-Vertrag).
pub trait SegmentAuditor {
    /// Rechnet ein Segment nach und gibt die Commitment-Hashes zurück.
    ///
    /// **Parameter:**
    /// - `segment_id`: ID des zu prüfenden Segments
    /// - `input_activations`: Eingabe-Aktivierungen (a_0)
    ///
    /// **Returns:** Vektor von Commitment-Hashes (h(a_0), h(a_1), ..., h(a_k))
    ///
    /// **Fehler:** `CheckError` wenn der Forward-Pass fehlschlägt.
    fn audit_segment(
        &self,
        segment_id: SegmentId,
        input_activations: &[u8],
    ) -> Result<Vec<Hash>, CheckError>;
}

/// Prüft ein Segment gegen einen behaupteten Commitment-Hash.
///
/// **Parameter:**
/// - `auditor`: Implementierung des SegmentAuditor-Traits
/// - `segment_id`: ID des zu prüfenden Segments
/// - `input_activations`: Eingabe-Aktivierungen (a_0)
/// - `claimed_hashes`: Behauptete Commitment-Hashes
///
/// **Returns:** `CheckResult::Valid` wenn alle Hashes übereinstimmen,
/// `CheckResult::Invalid { first_divergence }` sonst.
///
/// **Fehler:** `CheckError` wenn der Forward-Pass fehlschlägt.
pub fn check_segment(
    auditor: &dyn SegmentAuditor,
    segment_id: SegmentId,
    input_activations: &[u8],
    claimed_hashes: &[Hash],
) -> Result<CheckResult, CheckError> {
    if input_activations.is_empty() {
        return Err(CheckError::EmptySegment);
    }

    let computed_hashes = auditor.audit_segment(segment_id, input_activations)?;

    if computed_hashes.len() != claimed_hashes.len() {
        // Längen-Mismatch → erste Position als Divergenz melden
        return Ok(CheckResult::Invalid {
            first_divergence: 0,
        });
    }

    // Binärer Vergleich an allen Positionen
    for (i, (computed, claimed)) in computed_hashes.iter().zip(claimed_hashes.iter()).enumerate() {
        if computed != claimed {
            return Ok(CheckResult::Invalid {
                first_divergence: i,
            });
        }
    }

    Ok(CheckResult::Valid)
}

/// Mock-Auditor für Tests.
///
/// Gibt vordefinierte Hashes zurück, um die Check-Logik zu testen.
#[cfg(test)]
pub struct MockAuditor {
    hashes: Vec<Hash>,
}

#[cfg(test)]
impl MockAuditor {
    pub fn new(hashes: Vec<Hash>) -> Self {
        Self { hashes }
    }
}

#[cfg(test)]
impl SegmentAuditor for MockAuditor {
    fn audit_segment(
        &self,
        _segment_id: SegmentId,
        _input_activations: &[u8],
    ) -> Result<Vec<Hash>, CheckError> {
        Ok(self.hashes.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hashes(len: usize) -> Vec<Hash> {
        (0..len).map(|i| Hash::sha256(&[i as u8])).collect()
    }

    #[test]
    fn check_valid_segment() {
        let hashes = test_hashes(10);
        let auditor = MockAuditor::new(hashes.clone());
        let segment_id = SegmentId::new([1u8; 32]);
        let input = vec![1, 2, 3];

        let result = check_segment(&auditor, segment_id, &input, &hashes).unwrap();
        assert_eq!(result, CheckResult::Valid);
    }

    #[test]
    fn check_invalid_segment_first_position() {
        let computed = test_hashes(10);
        let mut claimed = computed.clone();
        claimed[0] = Hash::sha256(b"different");

        let auditor = MockAuditor::new(computed);
        let segment_id = SegmentId::new([1u8; 32]);
        let input = vec![1, 2, 3];

        let result = check_segment(&auditor, segment_id, &input, &claimed).unwrap();
        assert_eq!(result, CheckResult::Invalid { first_divergence: 0 });
    }

    #[test]
    fn check_invalid_segment_middle_position() {
        let computed = test_hashes(10);
        let mut claimed = computed.clone();
        claimed[5] = Hash::sha256(b"different");

        let auditor = MockAuditor::new(computed);
        let segment_id = SegmentId::new([1u8; 32]);
        let input = vec![1, 2, 3];

        let result = check_segment(&auditor, segment_id, &input, &claimed).unwrap();
        assert_eq!(result, CheckResult::Invalid { first_divergence: 5 });
    }

    #[test]
    fn check_invalid_segment_last_position() {
        let computed = test_hashes(10);
        let mut claimed = computed.clone();
        claimed[9] = Hash::sha256(b"different");

        let auditor = MockAuditor::new(computed);
        let segment_id = SegmentId::new([1u8; 32]);
        let input = vec![1, 2, 3];

        let result = check_segment(&auditor, segment_id, &input, &claimed).unwrap();
        assert_eq!(result, CheckResult::Invalid { first_divergence: 9 });
    }

    #[test]
    fn check_length_mismatch() {
        let computed = test_hashes(10);
        let claimed = test_hashes(5);

        let auditor = MockAuditor::new(computed);
        let segment_id = SegmentId::new([1u8; 32]);
        let input = vec![1, 2, 3];

        let result = check_segment(&auditor, segment_id, &input, &claimed).unwrap();
        assert_eq!(result, CheckResult::Invalid { first_divergence: 0 });
    }

    #[test]
    fn check_empty_input() {
        let hashes = test_hashes(10);
        let auditor = MockAuditor::new(hashes);
        let segment_id = SegmentId::new([1u8; 32]);
        let input: Vec<u8> = vec![];

        let result = check_segment(&auditor, segment_id, &input, &[]);
        assert!(matches!(result, Err(CheckError::EmptySegment)));
    }

    #[test]
    fn check_result_equality() {
        assert_eq!(CheckResult::Valid, CheckResult::Valid);
        assert_eq!(
            CheckResult::Invalid { first_divergence: 5 },
            CheckResult::Invalid { first_divergence: 5 }
        );
        assert_ne!(CheckResult::Valid, CheckResult::Invalid { first_divergence: 0 });
    }
}

/// Warum eine Spurantwort nicht taugt.
///
/// ⚑ **Jeder dieser Fälle ist eine eigene Aussage, kein Sammelfehler.**
/// „Der Koordinator hat ein anderes Segment geschickt" und „er hat
/// falsch gerechnet" sind verschiedene Befunde mit verschiedenen Folgen:
/// Das eine ist Verweigerung, das andere ein Streitfall.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spurfehler {
    /// Der Beweis gehört zu einem anderen Segment als dem gefragten.
    FalschesSegment { gefragt: u32, geliefert: u64 },
    /// Die bewiesene Blattzahl passt nicht zur Segmentzahl des Bündels.
    ///
    /// ⚑ **Das ist die Gegenprobe zu `PoIBundle::segmente`.** Die Zahl
    /// steht unterschrieben in der Kette, und der Merkle-Beweis trägt
    /// sie ein zweites Mal, gebunden an die Wurzel (Fund 77). Wer sie
    /// aufbläht, um die Stichprobe zu verdünnen, **kann keinen
    /// passenden Beweis liefern**: Die Behauptung fällt beim ersten
    /// Abruf.
    BlattzahlPasstNicht { im_buendel: u32, im_beweis: u64 },
    /// Der Beweis trägt nicht gegen `segments_root`.
    BeweisTraegtNicht,
    /// Die gelieferte Spur ergibt nicht die bezeugte Spurwurzel.
    SpurPasstNichtZumZeugnis,
    /// Das Nachrechnen selbst scheiterte.
    Nachrechnen(CheckError),
}

impl std::fmt::Display for Spurfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FalschesSegment { gefragt, geliefert } => write!(
                f,
                "gefragt war Segment {gefragt}, geliefert wurde {geliefert}. \
                 Wer die Antwort wählt, beantwortet nicht die Frage"
            ),
            Self::BlattzahlPasstNicht {
                im_buendel,
                im_beweis,
            } => write!(
                f,
                "das Bündel nennt {im_buendel} Segmente, der Beweis {im_beweis}. \
                 Eine aufgeblähte Segmentzahl verdünnt die Stichprobe"
            ),
            Self::BeweisTraegtNicht => write!(
                f,
                "der Merkle-Beweis trägt nicht gegen die unterschriebene Wurzel"
            ),
            Self::SpurPasstNichtZumZeugnis => write!(
                f,
                "die gelieferte Spur ergibt eine andere Wurzel als das Zeugnis"
            ),
            Self::Nachrechnen(e) => write!(f, "Nachrechnen: {e:?}"),
        }
    }
}

/// Prüft eine Spurantwort **ganz**: erst die Bindung, dann die Rechnung.
///
/// # ⚑ Die Reihenfolge ist die Sicherheit
///
/// Vier Bindungen stehen vor dem Nachrechnen, und jede einzelne fehlt,
/// wenn man sie weglässt:
///
/// 1. **Der Beweis gehört zum gefragten Index.** Sonst reicht der
///    Koordinator ein Segment heraus, das er richtig gerechnet hat.
/// 2. **Die Blattzahl passt zur Segmentzahl des Bündels.** Sonst bläht
///    er die Zahl auf und verdünnt die Stichprobe.
/// 3. **Der Beweis trägt gegen `segments_root`.** Die Wurzel steht im
///    unterschriebenen Bündel; ohne diesen Schritt hinge alles an einer
///    Behauptung des Gefragten.
/// 4. **Die gelieferte Spur ergibt die bezeugte Spurwurzel.** Sonst
///    liefert er zum richtigen Zeugnis eine andere Spur.
///
/// **Erst danach wird gerechnet.** Ein `Invalid` aus dieser Funktion ist
/// deshalb ein Befund über die **Rechnung** und nicht über die Lieferung.
pub fn pruefe_spurantwort(
    auditor: &dyn SegmentAuditor,
    antwort: &myl_types::Spurantwort,
    segments_root: &myl_types::ids::MerkleRoot,
    segmente_im_buendel: u32,
) -> Result<CheckResult, Spurfehler> {
    if antwort.beweis.leaf_index != u64::from(antwort.anfrage.segment) {
        return Err(Spurfehler::FalschesSegment {
            gefragt: antwort.anfrage.segment,
            geliefert: antwort.beweis.leaf_index,
        });
    }
    if antwort.beweis.leaf_count != u64::from(segmente_im_buendel) {
        return Err(Spurfehler::BlattzahlPasstNicht {
            im_buendel: segmente_im_buendel,
            im_beweis: antwort.beweis.leaf_count,
        });
    }

    // Das Blatt ist `Id ‖ Spurwurzel`, genau wie in `segments_root`.
    let mut blatt = Vec::with_capacity(64);
    blatt.extend_from_slice(antwort.zeugnis.id.as_ref());
    blatt.extend_from_slice(antwort.zeugnis.spurwurzel.as_ref());
    let wurzel = Hash(*segments_root.as_bytes());
    if !antwort
        .beweis
        .verify(&wurzel, &blatt, antwort.beweis.leaf_index)
    {
        return Err(Spurfehler::BeweisTraegtNicht);
    }

    let roh: Vec<[u8; 32]> = antwort.spur.iter().map(|h| *h.as_bytes()).collect();
    match myl_types::spurwurzel(&roh) {
        Ok(w) if w == antwort.zeugnis.spurwurzel => {}
        _ => return Err(Spurfehler::SpurPasstNichtZumZeugnis),
    }

    check_segment(
        auditor,
        antwort.zeugnis.id,
        &antwort.eingabe,
        &antwort.spur,
    )
    .map_err(Spurfehler::Nachrechnen)
}

#[cfg(test)]
mod spurtests {
    use super::*;
    use myl_types::ids::{EpochId, MerkleRoot, PodId, SegmentId};
    use myl_types::merkle::MerkleTree;
    use myl_types::{Segmentzeugnis, Spuranfrage, Spurantwort};

    fn hash(b: u8) -> Hash {
        Hash::sha256(&[b])
    }

    /// Ein Auditor, der genau die vorgegebene Spur zurueckgibt.
    struct EchterAuditor(Vec<Hash>);
    impl SegmentAuditor for EchterAuditor {
        fn audit_segment(&self, _id: SegmentId, _ein: &[u8]) -> Result<Vec<Hash>, CheckError> {
            Ok(self.0.clone())
        }
    }

    /// Baut ein Buendel aus `n` Zeugnissen und eine Antwort auf das
    /// `index`-te davon.
    fn aufbau(n: usize, index: usize) -> (MerkleRoot, u32, Spurantwort, Vec<Hash>) {
        let spuren: Vec<Vec<Hash>> = (0..n)
            .map(|i| vec![hash(i as u8), hash(i as u8 + 100), hash(i as u8 + 200)])
            .collect();
        let zeugnisse: Vec<Segmentzeugnis> = spuren
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let roh: Vec<[u8; 32]> = s.iter().map(|h| *h.as_bytes()).collect();
                let mut idb = [0u8; 32];
                idb[..8].copy_from_slice(&(i as u64).to_le_bytes());
                Segmentzeugnis {
                    id: SegmentId::new(idb),
                    spurwurzel: myl_types::spurwurzel(&roh).expect("Spurwurzel"),
                }
            })
            .collect();
        let wurzel = myl_types::segments_root(&zeugnisse).expect("Wurzel");

        let blaetter: Vec<Vec<u8>> = zeugnisse
            .iter()
            .map(|z| {
                let mut b = Vec::with_capacity(64);
                b.extend_from_slice(z.id.as_ref());
                b.extend_from_slice(z.spurwurzel.as_ref());
                b
            })
            .collect();
        let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
        let baum = MerkleTree::new(&refs).expect("Baum");
        let beweis = baum.proof(index).expect("Beweis");

        let antwort = Spurantwort {
            anfrage: Spuranfrage {
                epoche: EpochId(7),
                pod: PodId::new([1; 32]),
                segment: index as u32,
            },
            zeugnis: zeugnisse[index],
            beweis,
            eingabe: vec![1, 2, 3],
            spur: spuren[index].clone(),
        };
        (wurzel, n as u32, antwort, spuren[index].clone())
    }

    /// Der volle Weg: gebunden **und** richtig gerechnet.
    #[test]
    fn eine_richtige_antwort_besteht() {
        let (wurzel, n, antwort, spur) = aufbau(8, 3);
        let a = EchterAuditor(spur);
        assert_eq!(
            pruefe_spurantwort(&a, &antwort, &wurzel, n),
            Ok(CheckResult::Valid)
        );
    }

    /// ⚑ **Wer die Antwort waehlt, beantwortet nicht die Frage.**
    ///
    /// Der Koordinator liefert ein Segment, das er richtig gerechnet
    /// hat, statt des gezogenen. Ohne die Indexpruefung waere die
    /// Ziehung wertlos.
    #[test]
    fn ein_anderes_segment_wird_abgewiesen() {
        let (wurzel, n, mut antwort, spur) = aufbau(8, 3);
        antwort.anfrage.segment = 5;
        let a = EchterAuditor(spur);
        assert_eq!(
            pruefe_spurantwort(&a, &antwort, &wurzel, n),
            Err(Spurfehler::FalschesSegment {
                gefragt: 5,
                geliefert: 3
            })
        );
    }

    /// ⚑ **Die aufgeblaehte Segmentzahl faellt beim ersten Abruf.**
    ///
    /// Wer im Buendel mehr Segmente behauptet, um die Stichprobe zu
    /// verduennen, kann keinen Beweis mit passender Blattzahl liefern:
    /// Sie geht in die Wurzel ein (Fund 77).
    #[test]
    fn eine_aufgeblaehte_segmentzahl_faellt_auf() {
        let (wurzel, _n, antwort, spur) = aufbau(8, 3);
        let a = EchterAuditor(spur);
        assert_eq!(
            pruefe_spurantwort(&a, &antwort, &wurzel, 800),
            Err(Spurfehler::BlattzahlPasstNicht {
                im_buendel: 800,
                im_beweis: 8
            })
        );
    }

    /// Ein Zeugnis, das nicht unter der unterschriebenen Wurzel haengt.
    #[test]
    fn ein_fremdes_zeugnis_traegt_nicht() {
        let (wurzel, n, mut antwort, spur) = aufbau(8, 3);
        antwort.zeugnis.spurwurzel = MerkleRoot::new([9; 32]);
        let a = EchterAuditor(spur);
        assert_eq!(
            pruefe_spurantwort(&a, &antwort, &wurzel, n),
            Err(Spurfehler::BeweisTraegtNicht)
        );
    }

    /// Zum richtigen Zeugnis eine andere Spur liefern.
    #[test]
    fn eine_vertauschte_spur_passt_nicht_zum_zeugnis() {
        let (wurzel, n, mut antwort, spur) = aufbau(8, 3);
        antwort.spur = vec![hash(250), hash(251)];
        let a = EchterAuditor(spur);
        assert_eq!(
            pruefe_spurantwort(&a, &antwort, &wurzel, n),
            Err(Spurfehler::SpurPasstNichtZumZeugnis)
        );
    }

    /// ⚑ **Und erst danach zaehlt die Rechnung.** Alles gebunden, aber
    /// der Auditor kommt zu einem anderen Ergebnis: Das ist der
    /// Streitfall, den Stufe 2 finden soll.
    #[test]
    fn eine_falsche_rechnung_wird_gefunden() {
        let (wurzel, n, antwort, mut spur) = aufbau(8, 3);
        spur[1] = hash(99);
        let a = EchterAuditor(spur);
        assert_eq!(
            pruefe_spurantwort(&a, &antwort, &wurzel, n),
            Ok(CheckResult::Invalid {
                first_divergence: 1
            })
        );
    }
}
