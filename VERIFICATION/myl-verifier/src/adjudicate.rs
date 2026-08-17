//! On-Chain-Schiedsrunde (adjudicate) — Whitepaper Anhang A.4, Kap. 6.6.
//!
//! Die Schiedsrunde wird durchgeführt, wenn das Bisektions-Spiel eine
//! Abweichung identifiziert hat. Das Validatoren-Komitee führt einen
//! Shard-Forward durch und vergleicht den Hash mit dem behaupteten Hash.
//!
//! **Ablauf:**
//! 1. Checker fordert Aktivierung a_{j-1} an — unter Nennung des in der
//!    Spur festgeschriebenen Hashes h(a_{j-1})
//! 2. Angeklagter legt a_{j-1} offen
//! 3. Komitee prüft, dass die offengelegte Aktivierung **genau die aus
//!    der Spur** ist
//! 4. Validatoren-Komitee führt den Shard-Forward durch
//! 5. Hash-Vergleich: Übereinstimmung = unschuldig, Abweichung = schuldig
//!
//! ## Warum Schritt 3 nicht fehlen darf (Fund A11)
//!
//! Bis v0.2.6 prüfte `adjudicate()` die offengelegte Aktivierung nur
//! **gegen sich selbst**:
//!
//! ```text
//! let computed = Hash::sha256(&response.activation);
//! if computed != response.activation_hash { return Guilty; }
//! ```
//!
//! Beide Werte kamen aus derselben Antwort — die Prüfung war
//! tautologisch und stellte nur fest, dass der Angeklagte in sich
//! konsistent geantwortet hat. Der `AdjudicationRequest` trug keinen
//! Hash von a_{j-1}, also gab es nichts, woran die Eingabe gebunden
//! gewesen wäre. Ein Angeklagter, der eine **andere** Eingabe findet,
//! die unter seiner Ausführung den erwarteten Ausgabe-Hash ergibt,
//! wurde freigesprochen.
//!
//! Das untergräbt die zentrale Zusage aus Kap. 6.6 („Die
//! Schuldzuweisung ist eindeutig, weil das Ergebnis kanonisch ist"):
//! kanonisch ist das Ergebnis nur **bezogen auf die committete
//! Eingabe**. Der Request trägt deshalb jetzt `input_hash` aus
//! `Segment.trace[j-1]`, und die offengelegte Aktivierung wird dagegen
//! geprüft.
//!
//! **Konsens-Feld:** Die Schiedsrunden-Logik ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use myl_types::hash::Hash;
use myl_types::ids::{MinerId, SegmentId};
use borsh::{BorshDeserialize, BorshSerialize};

/// Eine Schiedsrunden-Anfrage.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct AdjudicationRequest {
    /// Segment-ID.
    pub segment_id: SegmentId,
    /// Position der abweichenden Layer-Gruppe.
    pub divergence_position: usize,
    /// Checker (Miner-ID).
    pub checker: MinerId,
    /// Angeklagter (Miner-ID).
    pub accused: MinerId,
    /// Hash der **committeten** Eingabe-Aktivierung a_{j-1} aus der
    /// Segment-Spur (`Segment.trace[divergence_position - 1]`, bzw. das
    /// Eingangs-Commitment des Segments bei Position 0).
    ///
    /// Ohne dieses Feld wäre die Offenlegung an nichts gebunden: der
    /// Angeklagte könnte eine beliebige Eingabe liefern, die unter
    /// seiner Ausführung zufällig den erwarteten Ausgabe-Hash ergibt.
    pub input_hash: Hash,
    /// Erwarteter Hash der Ausgabe a_j an der Position (vom Checker).
    pub expected_hash: Hash,
}

/// Eine Schiedsrunden-Antwort (Offenlegung der Aktivierung).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct AdjudicationResponse {
    /// Segment-ID.
    pub segment_id: SegmentId,
    /// Position der abweichenden Layer-Gruppe.
    pub divergence_position: usize,
    /// Offenlegte Aktivierung a_{j-1}.
    pub activation: Vec<u8>,
    /// Hash der offenlegten Aktivierung.
    pub activation_hash: Hash,
}

/// Ergebnis der Schiedsrunde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjudicationResult {
    /// Angeklagter ist unschuldig (Hash stimmt überein).
    Innocent,
    /// Angeklagter ist schuldig (Hash weicht ab).
    Guilty,
    /// Angeklagter hat nicht geantwortet (Timeout).
    NoResponse,
}

/// Fehler bei der Schiedsrunde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjudicationError {
    /// Segment-ID stimmt nicht überein.
    SegmentMismatch,
    /// Position stimmt nicht überein.
    PositionMismatch,
    /// Hash der Aktivierung stimmt nicht mit dem behaupteten Hash überein.
    HashMismatch,
}

impl std::fmt::Display for AdjudicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SegmentMismatch => write!(f, "Segment-ID stimmt nicht überein"),
            Self::PositionMismatch => write!(f, "Position stimmt nicht überein"),
            Self::HashMismatch => write!(f, "Hash der Aktivierung stimmt nicht überein"),
        }
    }
}

impl std::error::Error for AdjudicationError {}

/// Trait für Shard-Forward-Ausführung.
///
/// Dieser Trait definiert die Schnittstelle für die Ausführung eines
/// Shard-Forwards. Die konkrete Implementierung erfolgt durch Integration
/// mit INTEGER_LLMs Runtime.
pub trait ShardExecutor {
    /// Führt einen Shard-Forward für eine Layer-Gruppe aus.
    ///
    /// **Parameter:**
    /// - `activation`: Eingabe-Aktivierung a_{j-1}
    /// - `layer_group_index`: Index der Layer-Gruppe
    ///
    /// **Returns:** Ausgabe-Aktivierung a_j
    fn execute_shard(
        &self,
        activation: &[u8],
        layer_group_index: usize,
    ) -> Result<Vec<u8>, AdjudicationError>;
}

/// Führt die Schiedsrunde durch.
///
/// **Parameter:**
/// - `request`: Schiedsrunden-Anfrage
/// - `response`: Schiedsrunden-Antwort (None wenn keine Antwort)
/// - `executor`: Shard-Executor für den Forward-Pass
///
/// **Returns:** `AdjudicationResult` mit dem Ergebnis der Schiedsrunde.
pub fn adjudicate(
    request: &AdjudicationRequest,
    response: Option<&AdjudicationResponse>,
    executor: &dyn ShardExecutor,
) -> AdjudicationResult {
    // Keine Antwort = schuldig (Timeout)
    let response = match response {
        Some(r) => r,
        None => return AdjudicationResult::NoResponse,
    };

    // Validierung: Segment-ID und Position müssen übereinstimmen
    if response.segment_id != request.segment_id {
        return AdjudicationResult::Guilty;
    }

    if response.divergence_position != request.divergence_position {
        return AdjudicationResult::Guilty;
    }

    // Die offengelegte Aktivierung muss zum mitgelieferten Hash passen …
    let computed_activation_hash = Hash::sha256(&response.activation);
    if computed_activation_hash != response.activation_hash {
        return AdjudicationResult::Guilty;
    }

    // … und dieser Hash muss der in der Spur committete sein. Ohne
    // diesen zweiten Vergleich wäre die Prüfung tautologisch: beide
    // Werte stammten aus derselben Antwort (Fund A11).
    if computed_activation_hash != request.input_hash {
        return AdjudicationResult::Guilty;
    }

    // Shard-Forward durchführen
    match executor.execute_shard(&response.activation, request.divergence_position) {
        Ok(output_activation) => {
            // Hash der Ausgabe-Aktivierung berechnen
            let output_hash = Hash::sha256(&output_activation);

            // Hash-Vergleich: Wenn der Hash übereinstimmt, ist der Angeklagte unschuldig
            if output_hash == request.expected_hash {
                AdjudicationResult::Innocent
            } else {
                AdjudicationResult::Guilty
            }
        }
        Err(_) => {
            // Shard-Forward fehlgeschlagen = schuldig
            AdjudicationResult::Guilty
        }
    }
}

/// Mock-Shard-Executor für Tests.
#[cfg(test)]
pub struct MockShardExecutor {
    output: Vec<u8>,
}

#[cfg(test)]
impl MockShardExecutor {
    pub fn new(output: Vec<u8>) -> Self {
        Self { output }
    }
}

#[cfg(test)]
impl ShardExecutor for MockShardExecutor {
    fn execute_shard(
        &self,
        _activation: &[u8],
        _layer_group_index: usize,
    ) -> Result<Vec<u8>, AdjudicationError> {
        Ok(self.output.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_miner(byte: u8) -> MinerId {
        MinerId::new([byte; 32])
    }

    fn test_segment_id(byte: u8) -> SegmentId {
        SegmentId::new([byte; 32])
    }

    fn test_hash(byte: u8) -> Hash {
        Hash::sha256(&[byte])
    }

    #[test]
    fn adjudicate_innocent() {
        let activation = vec![1, 2, 3];
        let activation_hash = Hash::sha256(&activation);
        let output = vec![4, 5, 6];
        let output_hash = Hash::sha256(&output);

        let request = AdjudicationRequest {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            checker: test_miner(2),
            accused: test_miner(3),
            input_hash: activation_hash,
            expected_hash: output_hash,
        };

        let response = AdjudicationResponse {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            activation,
            activation_hash,
        };

        let executor = MockShardExecutor::new(output);
        let result = adjudicate(&request, Some(&response), &executor);

        assert_eq!(result, AdjudicationResult::Innocent);
    }

    #[test]
    fn adjudicate_guilty_hash_mismatch() {
        let activation = vec![1, 2, 3];
        let activation_hash = Hash::sha256(&activation);
        let output = vec![4, 5, 6];
        let wrong_output_hash = test_hash(99); // Falscher Hash

        let request = AdjudicationRequest {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            checker: test_miner(2),
            accused: test_miner(3),
            input_hash: activation_hash,
            expected_hash: wrong_output_hash,
        };

        let response = AdjudicationResponse {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            activation,
            activation_hash,
        };

        let executor = MockShardExecutor::new(output);
        let result = adjudicate(&request, Some(&response), &executor);

        assert_eq!(result, AdjudicationResult::Guilty);
    }

    #[test]
    fn adjudicate_guilty_segment_mismatch() {
        let activation = vec![1, 2, 3];
        let activation_hash = Hash::sha256(&activation);
        let output = vec![4, 5, 6];
        let output_hash = Hash::sha256(&output);

        let request = AdjudicationRequest {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            checker: test_miner(2),
            accused: test_miner(3),
            input_hash: activation_hash,
            expected_hash: output_hash,
        };

        let response = AdjudicationResponse {
            segment_id: test_segment_id(2), // Falsche Segment-ID
            divergence_position: 5,
            activation,
            activation_hash,
        };

        let executor = MockShardExecutor::new(output);
        let result = adjudicate(&request, Some(&response), &executor);

        assert_eq!(result, AdjudicationResult::Guilty);
    }

    #[test]
    fn adjudicate_guilty_position_mismatch() {
        let activation = vec![1, 2, 3];
        let activation_hash = Hash::sha256(&activation);
        let output = vec![4, 5, 6];
        let output_hash = Hash::sha256(&output);

        let request = AdjudicationRequest {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            checker: test_miner(2),
            accused: test_miner(3),
            input_hash: activation_hash,
            expected_hash: output_hash,
        };

        let response = AdjudicationResponse {
            segment_id: test_segment_id(1),
            divergence_position: 6, // Falsche Position
            activation,
            activation_hash,
        };

        let executor = MockShardExecutor::new(output);
        let result = adjudicate(&request, Some(&response), &executor);

        assert_eq!(result, AdjudicationResult::Guilty);
    }

    /// Der Kern von Fund A11: Ein Angeklagter, der eine **andere** als
    /// die committete Eingabe offenlegt, muss schuldig sein — auch dann,
    /// wenn seine Ausführung darauf genau den erwarteten Ausgabe-Hash
    /// liefert. Vorher wurde er freigesprochen, weil die Eingabe nur
    /// gegen den selbst mitgelieferten Hash geprüft wurde.
    #[test]
    fn untergeschobene_eingabe_wird_nicht_freigesprochen() {
        let committete_eingabe = vec![1, 2, 3];
        let untergeschobene_eingabe = vec![9, 9, 9];
        let ausgabe = vec![4, 5, 6];

        let request = AdjudicationRequest {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            checker: test_miner(2),
            accused: test_miner(3),
            // Die Spur schreibt DIESE Eingabe fest.
            input_hash: Hash::sha256(&committete_eingabe),
            expected_hash: Hash::sha256(&ausgabe),
        };

        // Der Angeklagte legt eine andere Eingabe offen — in sich
        // konsistent gehasht, also fuer die alte Pruefung einwandfrei.
        let response = AdjudicationResponse {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            activation: untergeschobene_eingabe.clone(),
            activation_hash: Hash::sha256(&untergeschobene_eingabe),
        };

        // Und der Executor liefert darauf exakt den erwarteten Hash.
        let executor = MockShardExecutor::new(ausgabe);

        assert_eq!(
            adjudicate(&request, Some(&response), &executor),
            AdjudicationResult::Guilty,
            "eine nicht-committete Eingabe darf nie zum Freispruch führen"
        );
    }

    /// Die Gegenprobe: Mit der committeten Eingabe und korrekter
    /// Ausführung bleibt der Freispruch möglich.
    #[test]
    fn committete_eingabe_erlaubt_freispruch() {
        let eingabe = vec![1, 2, 3];
        let ausgabe = vec![4, 5, 6];

        let request = AdjudicationRequest {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            checker: test_miner(2),
            accused: test_miner(3),
            input_hash: Hash::sha256(&eingabe),
            expected_hash: Hash::sha256(&ausgabe),
        };
        let response = AdjudicationResponse {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            activation: eingabe.clone(),
            activation_hash: Hash::sha256(&eingabe),
        };

        assert_eq!(
            adjudicate(&request, Some(&response), &MockShardExecutor::new(ausgabe)),
            AdjudicationResult::Innocent
        );
    }

    /// Eine in sich inkonsistente Antwort (Hash passt nicht zur
    /// Aktivierung) bleibt ebenfalls schuldig.
    #[test]
    fn inkonsistente_antwort_bleibt_schuldig() {
        let eingabe = vec![1, 2, 3];
        let ausgabe = vec![4, 5, 6];

        let request = AdjudicationRequest {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            checker: test_miner(2),
            accused: test_miner(3),
            input_hash: Hash::sha256(&eingabe),
            expected_hash: Hash::sha256(&ausgabe),
        };
        let response = AdjudicationResponse {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            activation: eingabe,
            activation_hash: test_hash(200), // passt zu nichts
        };

        assert_eq!(
            adjudicate(&request, Some(&response), &MockShardExecutor::new(ausgabe)),
            AdjudicationResult::Guilty
        );
    }

    #[test]
    fn adjudicate_no_response() {
        let request = AdjudicationRequest {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            checker: test_miner(2),
            accused: test_miner(3),
            input_hash: test_hash(2),
            expected_hash: test_hash(1),
        };

        let executor = MockShardExecutor::new(vec![]);
        let result = adjudicate(&request, None, &executor);

        assert_eq!(result, AdjudicationResult::NoResponse);
    }

    #[test]
    fn adjudication_request_borsh_roundtrip() {
        let request = AdjudicationRequest {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            checker: test_miner(2),
            accused: test_miner(3),
            input_hash: test_hash(2),
            expected_hash: test_hash(1),
        };

        let bytes = borsh::to_vec(&request).unwrap();
        let decoded: AdjudicationRequest = borsh::from_slice(&bytes).unwrap();

        assert_eq!(request, decoded);
    }

    #[test]
    fn adjudication_response_borsh_roundtrip() {
        let response = AdjudicationResponse {
            segment_id: test_segment_id(1),
            divergence_position: 5,
            activation: vec![1, 2, 3],
            activation_hash: test_hash(1),
        };

        let bytes = borsh::to_vec(&response).unwrap();
        let decoded: AdjudicationResponse = borsh::from_slice(&bytes).unwrap();

        assert_eq!(response, decoded);
    }

    #[test]
    fn adjudication_result_variants() {
        assert_eq!(AdjudicationResult::Innocent, AdjudicationResult::Innocent);
        assert_eq!(AdjudicationResult::Guilty, AdjudicationResult::Guilty);
        assert_eq!(AdjudicationResult::NoResponse, AdjudicationResult::NoResponse);
        assert_ne!(AdjudicationResult::Innocent, AdjudicationResult::Guilty);
    }
}
