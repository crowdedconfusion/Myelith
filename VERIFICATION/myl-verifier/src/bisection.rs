//! Interaktives Bisektionsprotokoll — Whitepaper Kap. 6.6, Anhang A.4.
//!
//! Binäre Eingrenzung auf die abweichende Layer-Gruppe in O(log L) Runden.
//! Das Protokoll identifiziert die genaue Position der Abweichung zwischen
//! primärem und redundantem Pod.
//!
//! **Konsens-Feld:** Das Bisektionsprotokoll ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Das Protokoll ist interaktiv — der Checker fordert in jeder
//! Runde die Offenlegung einer Aktivierung an, der Angeklagte antwortet.
//! Nach O(log L) Runden ist die genaue Layer-Gruppe identifiziert.

use myl_types::hash::Hash;
use myl_types::ids::SegmentId;

/// Eine Bisektions-Session.
///
/// Verfolgt den Zustand des Bisektionsprotokolls über mehrere Runden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BisectionSession {
    /// ID des betroffenen Segments.
    pub segment_id: SegmentId,
    /// Aktueller unterer Bound (inklusiv).
    pub lower: usize,
    /// Aktueller oberer Bound (exklusiv).
    pub upper: usize,
    /// Anzahl der abgeschlossenen Runden.
    pub rounds: u32,
    /// Maximale Anzahl von Runden (O(log L)).
    pub max_rounds: u32,
}

/// Eine Anfrage im Bisektionsprotokoll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BisectionRequest {
    /// Runde (0-basiert).
    pub round: u32,
    /// Angeforderte Position (mid-Point).
    pub position: usize,
}

/// Eine Antwort im Bisektionsprotokoll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BisectionResponse {
    /// Runde (0-basiert).
    pub round: u32,
    /// Offenlegte Aktivierung an der angeforderten Position.
    pub activation_hash: Hash,
}

/// Ergebnis des Bisektionsprotokolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BisectionResult {
    /// Abweichung identifiziert (genaue Position).
    DivergenceFound {
        /// Index der abweichenden Layer-Gruppe.
        position: usize,
    },
    /// Keine Abweichung gefunden (Pods stimmen überein).
    NoDivergence,
    /// Protokoll unvollständig (maximale Runden erreicht).
    Incomplete,
}

/// Fehler im Bisektionsprotokoll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BisectionError {
    /// Ungültige Runde (außerhalb des erwarteten Bereichs).
    InvalidRound { expected: u32, got: u32 },
    /// Protokoll bereits abgeschlossen.
    AlreadyComplete,
    /// Antwort passt nicht zur Anfrage (Position stimmt nicht überein).
    PositionMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for BisectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRound { expected, got } => {
                write!(f, "Ungültige Runde: erwartet {}, bekommen {}", expected, got)
            }
            Self::AlreadyComplete => write!(f, "Protokoll bereits abgeschlossen"),
            Self::PositionMismatch { expected, got } => {
                write!(
                    f,
                    "Positions-Mismatch: erwartet {}, bekommen {}",
                    expected, got
                )
            }
        }
    }
}

impl std::error::Error for BisectionError {}

/// Ganzzahliges `ceil(log2(n))` — ohne Gleitkomma (Fund A18).
///
/// Vorher stand hier `(trace_len as f64).log2().ceil() as u32`.
/// `f64::log2()` ist eine libm-Funktion und **nicht korrekt gerundet**:
/// sie unterscheidet sich zwischen glibc-Versionen, musl, macOS-libm und
/// Windows-CRT. Für exakte Zweierpotenzen kann das Ergebnis knapp unter
/// oder über der ganzen Zahl liegen, und `.ceil()` kippt dann in die eine
/// oder andere Richtung — zwei Schiedsrichter auf verschiedenen
/// Plattformen hätten verschieden viele Bisektionsrunden erwartet und
/// wären über die Gültigkeit des Spiels uneins geworden.
///
/// `n.next_power_of_two().trailing_zeros()` liefert dasselbe exakt und
/// auf jeder Plattform gleich.
fn ceil_log2(n: usize) -> u32 {
    if n <= 1 {
        return 0;
    }
    n.next_power_of_two().trailing_zeros()
}

impl BisectionSession {
    /// Erstellt eine neue Bisektions-Session.
    ///
    /// **Parameter:**
    /// - `segment_id`: ID des betroffenen Segments
    /// - `trace_len`: Länge der Spur (Anzahl Layer-Gruppen)
    pub fn new(segment_id: SegmentId, trace_len: usize) -> Self {
        let max_rounds = ceil_log2(trace_len) + 1;
        Self {
            segment_id,
            lower: 0,
            upper: trace_len,
            rounds: 0,
            max_rounds,
        }
    }

    /// Gibt die nächste Anfrage zurück (mid-Point).
    ///
    /// **Returns:** `BisectionRequest` mit der angeforderten Position.
    pub fn next_request(&self) -> Option<BisectionRequest> {
        if self.is_complete() {
            return None;
        }

        let mid = (self.lower + self.upper) / 2;
        Some(BisectionRequest {
            round: self.rounds,
            position: mid,
        })
    }

    /// Verarbeitet eine Antwort und aktualisiert den Session-Zustand.
    ///
    /// **Parameter:**
    /// - `response`: Antwort des Angeklagten
    /// - `expected_hash`: Erwarteter Hash an der Position (vom Checker)
    ///
    /// **Returns:** `Ok(())` bei erfolgreicher Verarbeitung.
    ///
    /// **Fehler:** `BisectionError` wenn die Antwort ungültig ist.
    pub fn process_response(
        &mut self,
        response: &BisectionResponse,
        expected_hash: &Hash,
    ) -> Result<(), BisectionError> {
        // Validierung
        if self.is_complete() {
            return Err(BisectionError::AlreadyComplete);
        }

        let request = self.next_request().ok_or(BisectionError::AlreadyComplete)?;

        if response.round != request.round {
            return Err(BisectionError::InvalidRound {
                expected: request.round,
                got: response.round,
            });
        }

        if response.activation_hash != *expected_hash {
            // Position stimmt nicht überein → Abweichung in linker Hälfte
            // (die angeforderte Position sollte mit expected_hash übereinstimmen,
            // wenn der Angeklagte ehrlich ist bis zu diesem Punkt)
            // Tatsächlich: Wir vergleichen die Hashes an der mid-Position
            // Wenn sie unterschiedlich sind, liegt die Abweichung in [lower, mid]
            // Wenn sie gleich sind, liegt die Abweichung in [mid, upper]
        }

        // Aktualisiere Bounds basierend auf dem Vergleich
        let _mid = (self.lower + self.upper) / 2;

        // Wenn die Hashes an der mid-Position unterschiedlich sind,
        // liegt die Abweichung in der linken Hälfte [lower, mid]
        // Andernfalls in der rechten Hälfte [mid, upper]
        // (Diese Logik wird in process_response_with_comparison implementiert)

        self.rounds += 1;
        Ok(())
    }

    /// Verarbeitet eine Antwort mit explizitem Hash-Vergleich.
    ///
    /// **Parameter:**
    /// - `response`: Antwort des Angeklagten
    /// - `primary_hash`: Hash des primären Pods an der Position
    /// - `redundant_hash`: Hash des redundanten Pods an der Position
    ///
    /// **Returns:** `Ok(())` bei erfolgreicher Verarbeitung.
    pub fn process_response_with_comparison(
        &mut self,
        response: &BisectionResponse,
        primary_hash: &Hash,
        redundant_hash: &Hash,
    ) -> Result<(), BisectionError> {
        if self.is_complete() {
            return Err(BisectionError::AlreadyComplete);
        }

        let request = self.next_request().ok_or(BisectionError::AlreadyComplete)?;

        if response.round != request.round {
            return Err(BisectionError::InvalidRound {
                expected: request.round,
                got: response.round,
            });
        }

        // Aktualisiere Bounds basierend auf dem Hash-Vergleich
        let mid = (self.lower + self.upper) / 2;

        if primary_hash != redundant_hash {
            // Abweichung in linker Hälfte [lower, mid]
            self.upper = mid;
        } else {
            // Abweichung in rechter Hälfte [mid, upper]
            self.lower = mid;
        }

        self.rounds += 1;
        Ok(())
    }

    /// Prüft, ob das Protokoll abgeschlossen ist.
    pub fn is_complete(&self) -> bool {
        self.rounds >= self.max_rounds || self.upper - self.lower <= 1
    }

    /// Gibt das Ergebnis des Protokolls zurück.
    pub fn result(&self) -> BisectionResult {
        if self.upper - self.lower <= 1 {
            BisectionResult::DivergenceFound {
                position: self.lower,
            }
        } else if self.rounds >= self.max_rounds {
            BisectionResult::Incomplete
        } else {
            BisectionResult::NoDivergence
        }
    }

    /// Berechnet die erwartete Anzahl von Runden für eine gegebene Spur-Länge.
    pub fn expected_rounds(trace_len: usize) -> u32 {
        ceil_log2(trace_len) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_creation() {
        let segment_id = SegmentId::new([1u8; 32]);
        let session = BisectionSession::new(segment_id, 16);

        assert_eq!(session.lower, 0);
        assert_eq!(session.upper, 16);
        assert_eq!(session.rounds, 0);
        assert!(session.max_rounds >= 4); // log2(16) = 4
    }

    #[test]
    fn first_request_midpoint() {
        let segment_id = SegmentId::new([1u8; 32]);
        let session = BisectionSession::new(segment_id, 16);

        let request = session.next_request().unwrap();
        assert_eq!(request.round, 0);
        assert_eq!(request.position, 8); // mid von [0, 16]
    }

    #[test]
    fn bisection_left_half() {
        let segment_id = SegmentId::new([1u8; 32]);
        let mut session = BisectionSession::new(segment_id, 16);

        let request = session.next_request().unwrap();
        assert_eq!(request.position, 8);

        // Simuliere Abweichung in linker Hälfte
        let response = BisectionResponse {
            round: 0,
            activation_hash: Hash::sha256(b"activation-8"),
        };
        let primary_hash = Hash::sha256(b"primary-8");
        let redundant_hash = Hash::sha256(b"redundant-8"); // Unterschiedlich

        session
            .process_response_with_comparison(&response, &primary_hash, &redundant_hash)
            .unwrap();

        assert_eq!(session.lower, 0);
        assert_eq!(session.upper, 8); // Linke Hälfte
        assert_eq!(session.rounds, 1);
    }

    #[test]
    fn bisection_right_half() {
        let segment_id = SegmentId::new([1u8; 32]);
        let mut session = BisectionSession::new(segment_id, 16);

        let request = session.next_request().unwrap();
        assert_eq!(request.position, 8);

        // Simuliere Abweichung in rechter Hälfte
        let response = BisectionResponse {
            round: 0,
            activation_hash: Hash::sha256(b"activation-8"),
        };
        let primary_hash = Hash::sha256(b"same-hash");
        let redundant_hash = Hash::sha256(b"same-hash"); // Gleich

        session
            .process_response_with_comparison(&response, &primary_hash, &redundant_hash)
            .unwrap();

        assert_eq!(session.lower, 8);
        assert_eq!(session.upper, 16); // Rechte Hälfte
        assert_eq!(session.rounds, 1);
    }

    #[test]
    fn bisection_converges() {
        let segment_id = SegmentId::new([1u8; 32]);
        let mut session = BisectionSession::new(segment_id, 16);

        // Simuliere mehrere Runden bis zur Konvergenz
        while !session.is_complete() {
            let request = session.next_request().unwrap();
            let response = BisectionResponse {
                round: request.round,
                activation_hash: Hash::sha256(&[request.position as u8]),
            };
            let primary_hash = Hash::sha256(b"primary");
            let redundant_hash = Hash::sha256(b"redundant"); // Immer unterschiedlich

            session
                .process_response_with_comparison(&response, &primary_hash, &redundant_hash)
                .unwrap();
        }

        // Sollte nach O(log L) Runden konvergieren
        assert!(session.rounds <= session.max_rounds);
        assert_eq!(session.upper - session.lower, 1);
    }

    #[test]
    fn expected_rounds_calculation() {
        assert_eq!(BisectionSession::expected_rounds(16), 5); // log2(16) = 4, +1
        assert_eq!(BisectionSession::expected_rounds(32), 6); // log2(32) = 5, +1
        assert_eq!(BisectionSession::expected_rounds(8), 4); // log2(8) = 3, +1
    }

    #[test]
    fn bisection_result_found() {
        let segment_id = SegmentId::new([1u8; 32]);
        let mut session = BisectionSession::new(segment_id, 2);

        // Eine Runde sollte genügen
        assert!(session.next_request().is_some());
        let response = BisectionResponse {
            round: 0,
            activation_hash: Hash::sha256(b"activation"),
        };
        let primary_hash = Hash::sha256(b"primary");
        let redundant_hash = Hash::sha256(b"redundant");

        session
            .process_response_with_comparison(&response, &primary_hash, &redundant_hash)
            .unwrap();

        let result = session.result();
        assert!(matches!(result, BisectionResult::DivergenceFound { .. }));
    }

    #[test]
    fn invalid_round_error() {
        let segment_id = SegmentId::new([1u8; 32]);
        let mut session = BisectionSession::new(segment_id, 16);

        let response = BisectionResponse {
            round: 5, // Falsche Runde
            activation_hash: Hash::sha256(b"activation"),
        };
        let primary_hash = Hash::sha256(b"primary");
        let redundant_hash = Hash::sha256(b"redundant");

        let result = session.process_response_with_comparison(&response, &primary_hash, &redundant_hash);
        assert!(matches!(
            result,
            Err(BisectionError::InvalidRound { expected: 0, got: 5 })
        ));
    }

    #[test]
    fn already_complete_error() {
        let segment_id = SegmentId::new([1u8; 32]);
        let mut session = BisectionSession::new(segment_id, 1);

        // Session ist sofort abgeschlossen (upper - lower = 1)
        assert!(session.is_complete());

        let response = BisectionResponse {
            round: 0,
            activation_hash: Hash::sha256(b"activation"),
        };
        let primary_hash = Hash::sha256(b"primary");
        let redundant_hash = Hash::sha256(b"redundant");

        let result = session.process_response_with_comparison(&response, &primary_hash, &redundant_hash);
        assert!(matches!(result, Err(BisectionError::AlreadyComplete)));
    }

    /// Regression zu Fund A18: Die Rundenzahl muss ganzzahlig und ohne
    /// libm berechnet werden. Geprueft gegen die Referenzwerte an den
    /// Zweierpotenz-Grenzen, wo die alte f64-Fassung am ehesten kippte.
    #[test]
    fn ceil_log2_ist_exakt() {
        let faelle: &[(usize, u32)] = &[
            (0, 0), (1, 0), (2, 1), (3, 2), (4, 2), (5, 3), (7, 3), (8, 3),
            (9, 4), (15, 4), (16, 4), (17, 5), (1023, 10), (1024, 10),
            (1025, 11), (65_536, 16), (65_537, 17),
        ];
        for &(n, erwartet) in faelle {
            assert_eq!(ceil_log2(n), erwartet, "ceil_log2({})", n);
        }
    }

    /// Die Bisektion halbiert das Intervall — nach `expected_rounds`
    /// Runden muss jede Spurlaenge auf eine Position eingegrenzt sein.
    #[test]
    fn rundenzahl_reicht_zum_eingrenzen() {
        for trace_len in [1usize, 2, 3, 7, 8, 100, 1024, 4096] {
            let noetig = {
                let mut span = trace_len;
                let mut r = 0u32;
                while span > 1 {
                    span = span.div_ceil(2);
                    r += 1;
                }
                r
            };
            assert!(
                BisectionSession::expected_rounds(trace_len) >= noetig,
                "trace_len {}: {} Runden angekuendigt, {} noetig",
                trace_len,
                BisectionSession::expected_rounds(trace_len),
                noetig
            );
        }
    }
}
