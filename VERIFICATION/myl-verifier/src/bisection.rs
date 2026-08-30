//! Interaktives Bisektionsprotokoll — Whitepaper Kap. 6.6, Anhang A.4.
//!
//! Binäre Eingrenzung auf die **erste abweichende Layer** in O(log L)
//! Runden. Das Protokoll bestimmt die Position, an der der Angeklagte von
//! der Rechnung des Checkers abweicht; die Schiedsrunde
//! ([`crate::adjudicate`]) rechnet genau diese eine Layer nach.
//!
//! **Konsens-Feld:** Das Bisektionsprotokoll ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Die Spur ist seit dem 2026-08-23 Layer-granular** (myl-pod v0.5.0).
//! Vorher hatte sie einen Eintrag je **Shard**, ihre Länge hing also am
//! Zuschnitt des Pods, und [`crate::redundancy::compare_commitments`]
//! lehnt ungleiche Längen mit `LengthMismatch` ab. Zwei redundante Pods
//! mussten deshalb denselben Zuschnitt tragen.
//!
//! ## Worauf die Bisektion beruht
//!
//! Die Aktivierungen sind **verkettet**: `a_j` hängt von `a_{j-1}` ab.
//! Weichen zwei Läufe an Layer `d` erstmals ab, weichen sie ab da an
//! allen folgenden Positionen ab. Das Prädikat „weicht an Position `i`
//! ab" ist damit **monoton** (erst falsch, dann wahr), und genau darauf
//! setzt die binäre Suche auf. Ohne die Verkettung wäre das Verfahren
//! nicht anwendbar, egal wie es implementiert ist.
//!
//! ## ⚑ Fund 42: Das Spiel nannte die falsche Layer (2026-08-23)
//!
//! Bis v0.3.2 grenzte das Protokoll auf **`d − 1`** statt auf `d` ein,
//! und zwar systematisch: Bei einer Spur der Länge 16 und einer echten
//! Abweichung an Position `d` nannte das Spiel für jedes `d` von 1 bis 15
//! die Position `d − 1`. Nur `d = 0` traf zu, und das aus Versehen, weil
//! dort die untere Grenze nicht mehr fallen kann.
//!
//! Die Ursache war eine Grenzverschiebung um eins: Bei einer Abweichung
//! an `mid` wurde `upper = mid` gesetzt, womit `mid` selbst aus dem
//! Suchintervall fiel — obwohl `mid` der gesuchte Index sein kann.
//!
//! **Die Wirkung ist die Umkehrung des Verfahrens.** Die Schiedsrunde
//! rechnet die genannte Layer nach. Layer `d − 1` hat der Angeklagte
//! korrekt gerechnet, sein Ergebnis stimmt, und er wird
//! **freigesprochen**; anschließend verliert der Checker, der die
//! Abweichung zu Recht gemeldet hat, und wird geschlachtet. Stufe 2 der
//! Verifikationsarchitektur hätte also in 15 von 16 Fällen den Betrüger
//! belohnt und den ehrlichen Prüfer bestraft.
//!
//! Gefunden beim Bau der adversarialen Testebene (K4), nicht durch
//! Codelektüre: Die Bestandstests prüften „konvergiert nach O(log L)
//! Runden" und „grenzt auf ein Intervall der Länge 1 ein", aber
//! **keiner** prüfte, ob die genannte Position die richtige ist.
//! `tests/adversarial.rs::das_spiel_nennt_die_richtige_layer` fährt
//! seither jede Position jeder Spurlänge durch.
//!
//! ## ⚑ Fund 43: Die Antwort des Angeklagten war ohne Wirkung
//!
//! `process_response_with_comparison` entschied aus zwei Hashes, die der
//! **Aufrufer** mitgab, und ließ `response.activation_hash` unbenutzt.
//! Ein Checker, der beide Spuren ohnehin hat, braucht dafür kein
//! Gegenüber; das Protokoll war also nicht interaktiv, und die
//! Offenlegung des Angeklagten war an nichts gebunden. Das ist dieselbe
//! Lücke, die Fund A11 in der Schiedsrunde geschlossen hat, eine Ebene
//! höher.
//!
//! Die zweite Fassung `process_response` bekam den erwarteten Hash zwar
//! übergeben, verglich ihn in einem **leeren `if`-Block** und setzte
//! danach weder `lower` noch `upper`. Sie verbrauchte nur Runden und
//! endete zwangsläufig in `Incomplete`, obwohl ihre Dokumentation
//! „aktualisiert den Session-Zustand" zusagte.
//!
//! Jetzt gibt es **eine** Fassung: Sie vergleicht die offengelegte
//! Aktivierung des Angeklagten gegen den Hash des Checkers an der
//! angefragten Position und grenzt danach ein.

use myl_types::hash::Hash;
use myl_types::ids::SegmentId;

/// Obergrenze für die Spurlänge.
///
/// Darüber ist `trace_len.next_power_of_two()` nicht mehr darstellbar und
/// [`ceil_log2`] liefe über. Kein Modell hat 2⁶³ Layer; die Grenze steht
/// hier, damit eine aus einer Nachricht übernommene Länge nicht in eine
/// Panik führt.
const MAX_SPURLAENGE: usize = 1usize << 62;

/// Wie lange eine Seite Zeit hat, auf eine Bisektionsanfrage zu
/// antworten, gemessen in Epochen.
///
/// # ⚑ Warum die Frist hier steht und nicht bei der Schiedsrunde
///
/// Sie stand dort, bis am 2026-08-30 auffiel, dass die Schiedsrunde gar
/// keine Antwort mehr braucht: Der Ankläger legt die Aktivierung
/// gleich mit der Anfrage vor. **Die Bisektion dagegen ist wirklich
/// wechselseitig**: Sie fragt nach Spur-Einträgen, und beide Seiten
/// müssen liefern. Hier kann jemand stillstehen, also gehört die Frist
/// hierher.
///
/// # Warum in Epochen und nicht in Millisekunden
///
/// Dieselbe Begründung wie bei der Streitfrist in
/// `myl_consensus::da`: Eine Frist, über die zwei Parteien streiten
/// können, ist keine. Die Epochennummer kommt aus dem Konsens, jeder
/// rechnet dieselbe, und niemand kann sich hinter seiner Uhr
/// verstecken.
///
/// # Warum genau eine Epoche
///
/// Verlangt wird ein **Spur-Eintrag**, kein Rechenwerk: Beide Seiten
/// haben ihre Spur, das ist die kleine Größe, die sie über die
/// Streitfrist vorhalten. Eine Epoche (3600 s) ist dafür reichlich und
/// kostet bei 168 Epochen Streitfrist unter einem Prozent des
/// Fensters. Bei rund `log2(Spurlänge)` Runden bleibt der Streit auch
/// bei langen Spuren weit innerhalb der Frist.
pub const ANTWORTFRIST_EPOCHEN: u64 = 1;

/// Eine Bisektions-Session.
///
/// **Invariante:** Die erste abweichende Position liegt in
/// `[lower, upper)`. Anfangs ist das die ganze Spur. Bei `lower == upper`
/// ist die Suche beendet: `lower` ist die gesuchte Position, oder gleich
/// der Spurlänge, wenn der Angeklagte nirgends abgewichen ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BisectionSession {
    /// ID des betroffenen Segments.
    pub segment_id: SegmentId,
    /// Aktueller unterer Bound (inklusiv).
    pub lower: usize,
    /// Aktueller oberer Bound (exklusiv).
    pub upper: usize,
    /// Länge der Spur, gegen die gespielt wird.
    pub trace_len: usize,
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
    /// Position, auf die sich die Antwort bezieht.
    ///
    /// Muss zur Anfrage passen. Ohne dieses Feld wäre eine Antwort nicht
    /// an ihre Frage gebunden, und eine an anderer Stelle abgegriffene
    /// Offenlegung ließe sich wiedereinspielen.
    pub position: usize,
    /// Offengelegte Aktivierung an der angeforderten Position.
    pub activation_hash: Hash,
}

/// Ergebnis des Bisektionsprotokolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BisectionResult {
    /// Abweichung identifiziert (genaue Position).
    DivergenceFound {
        /// Index der ersten abweichenden Layer.
        position: usize,
    },
    /// Der Angeklagte hat an keiner angefragten Position abgewichen.
    NoDivergence,
    /// Das Spiel läuft noch.
    ///
    /// **Bis v0.3.2 hieß dieser Fall `NoDivergence`**, und das war
    /// gefährlich: Wer das Ergebnis einer laufenden Session abfragte,
    /// bekam einen Freispruch. „Noch nicht entschieden" und
    /// „nichts gefunden" sind zwei verschiedene Aussagen.
    InProgress,
    /// Protokoll unvollständig (maximale Runden erreicht).
    Incomplete,
}

/// Fehler im Bisektionsprotokoll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BisectionError {
    /// Spurlänge null oder jenseits von [`MAX_SPURLAENGE`].
    InvalidTraceLength { trace_len: usize },
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
            Self::InvalidTraceLength { trace_len } => {
                write!(f, "Unbrauchbare Spurlänge: {}", trace_len)
            }
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
    /// - `trace_len`: Länge der Spur (Anzahl Layer des Modells)
    ///
    /// **Fehler:** `InvalidTraceLength` bei Länge 0 (eine leere Spur hat
    /// keine Layer, die schuldig sein könnte — bis v0.3.2 lieferte die
    /// Session dafür sofort `DivergenceFound { position: 0 }`, also eine
    /// Verurteilung ohne eine einzige Runde) und bei Längen jenseits von
    /// [`MAX_SPURLAENGE`], wo die Rundenzahl überliefe.
    pub fn new(segment_id: SegmentId, trace_len: usize) -> Result<Self, BisectionError> {
        if trace_len == 0 || trace_len > MAX_SPURLAENGE {
            return Err(BisectionError::InvalidTraceLength { trace_len });
        }
        Ok(Self {
            segment_id,
            lower: 0,
            upper: trace_len,
            trace_len,
            rounds: 0,
            max_rounds: Self::expected_rounds(trace_len),
        })
    }

    /// Gibt die nächste Anfrage zurück (mid-Point).
    ///
    /// **Returns:** `BisectionRequest` mit der angeforderten Position,
    /// oder `None`, wenn das Spiel abgeschlossen ist.
    pub fn next_request(&self) -> Option<BisectionRequest> {
        if self.is_complete() {
            return None;
        }
        Some(BisectionRequest {
            round: self.rounds,
            position: self.mid(),
        })
    }

    /// Mittelpunkt des offenen Intervalls.
    ///
    /// `lower + (upper − lower)/2` statt `(lower + upper)/2`: Die zweite
    /// Form läuft für große Indizes über, und ein Überlauf wäre im
    /// Debug-Build eine Panik und im Release-Build eine stille
    /// Falschrechnung — zwei Schiedsrichter mit verschiedenen Bauprofilen
    /// kämen zu verschiedenen Urteilen.
    fn mid(&self) -> usize {
        self.lower + (self.upper - self.lower) / 2
    }

    /// Verarbeitet die Offenlegung des Angeklagten und grenzt ein.
    ///
    /// **Parameter:**
    /// - `response`: Antwort des Angeklagten (Runde, Position, Hash)
    /// - `checker_hash`: Hash, den der Checker an dieser Position selbst
    ///   gerechnet hat
    ///
    /// Stimmen beide überein, hat der Angeklagte bis einschließlich
    /// dieser Position richtig gerechnet, und die erste Abweichung liegt
    /// **danach**. Stimmen sie nicht überein, liegt sie **hier oder
    /// davor**.
    ///
    /// **Fehler:** `AlreadyComplete`, `InvalidRound`, `PositionMismatch`.
    pub fn process_response(
        &mut self,
        response: &BisectionResponse,
        checker_hash: &Hash,
    ) -> Result<(), BisectionError> {
        let request = self.next_request().ok_or(BisectionError::AlreadyComplete)?;

        if response.round != request.round {
            return Err(BisectionError::InvalidRound {
                expected: request.round,
                got: response.round,
            });
        }

        if response.position != request.position {
            return Err(BisectionError::PositionMismatch {
                expected: request.position,
                got: response.position,
            });
        }

        if response.activation_hash == *checker_hash {
            // Einig bis hierher: die erste Abweichung liegt danach.
            self.lower = request.position + 1;
        } else {
            // Uneinig an dieser Stelle: sie liegt hier oder davor.
            // `upper = position`, nicht `position + 1` — das Intervall ist
            // oben offen, `position` bleibt damit enthalten. Genau hier
            // saß Fund 42.
            self.upper = request.position;
        }

        self.rounds += 1;
        Ok(())
    }

    /// Prüft, ob das Protokoll abgeschlossen ist.
    pub fn is_complete(&self) -> bool {
        self.lower >= self.upper || self.rounds >= self.max_rounds
    }

    /// Gibt das Ergebnis des Protokolls zurück.
    pub fn result(&self) -> BisectionResult {
        if self.lower >= self.upper {
            if self.lower >= self.trace_len {
                BisectionResult::NoDivergence
            } else {
                BisectionResult::DivergenceFound {
                    position: self.lower,
                }
            }
        } else if self.rounds >= self.max_rounds {
            BisectionResult::Incomplete
        } else {
            BisectionResult::InProgress
        }
    }

    /// Berechnet die erwartete Anzahl von Runden für eine gegebene Spur-Länge.
    ///
    /// Die binäre Suche über `[0, L)` braucht `ceil(log2(L))` Runden; die
    /// zusätzliche Runde ist Reserve.
    pub fn expected_rounds(trace_len: usize) -> u32 {
        ceil_log2(trace_len) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sitzung(len: usize) -> BisectionSession {
        BisectionSession::new(SegmentId::new([1u8; 32]), len).expect("gültige Spurlänge")
    }

    fn antwort(round: u32, position: usize, hash: Hash) -> BisectionResponse {
        BisectionResponse { round, position, activation_hash: hash }
    }

    #[test]
    fn session_creation() {
        let session = sitzung(16);
        assert_eq!(session.lower, 0);
        assert_eq!(session.upper, 16);
        assert_eq!(session.rounds, 0);
        assert!(session.max_rounds >= 4);
        assert_eq!(session.result(), BisectionResult::InProgress);
    }

    #[test]
    fn first_request_midpoint() {
        let session = sitzung(16);
        let request = session.next_request().unwrap();
        assert_eq!(request.round, 0);
        assert_eq!(request.position, 8);
    }

    #[test]
    fn uneinigkeit_grenzt_nach_links_ein() {
        let mut session = sitzung(16);
        let request = session.next_request().unwrap();
        let response = antwort(0, request.position, Hash::sha256(b"angeklagter"));
        session
            .process_response(&response, &Hash::sha256(b"checker"))
            .unwrap();
        assert_eq!(session.lower, 0);
        assert_eq!(session.upper, 8);
        assert_eq!(session.rounds, 1);
    }

    #[test]
    fn einigkeit_grenzt_nach_rechts_ein() {
        let mut session = sitzung(16);
        let request = session.next_request().unwrap();
        let h = Hash::sha256(b"gleich");
        let response = antwort(0, request.position, h);
        session.process_response(&response, &h).unwrap();
        assert_eq!(session.lower, 9);
        assert_eq!(session.upper, 16);
        assert_eq!(session.rounds, 1);
    }

    #[test]
    fn expected_rounds_calculation() {
        assert_eq!(BisectionSession::expected_rounds(16), 5);
        assert_eq!(BisectionSession::expected_rounds(32), 6);
        assert_eq!(BisectionSession::expected_rounds(8), 4);
    }

    #[test]
    fn invalid_round_error() {
        let mut session = sitzung(16);
        let response = antwort(5, 8, Hash::sha256(b"a"));
        assert!(matches!(
            session.process_response(&response, &Hash::sha256(b"b")),
            Err(BisectionError::InvalidRound { expected: 0, got: 5 })
        ));
    }

    #[test]
    fn position_mismatch_error() {
        let mut session = sitzung(16);
        let response = antwort(0, 3, Hash::sha256(b"a"));
        assert!(matches!(
            session.process_response(&response, &Hash::sha256(b"b")),
            Err(BisectionError::PositionMismatch { expected: 8, got: 3 })
        ));
    }

    #[test]
    fn already_complete_error() {
        let mut session = sitzung(1);
        // Eine Spur der Länge 1: eine Runde entscheidet.
        let request = session.next_request().unwrap();
        assert_eq!(request.position, 0);
        session
            .process_response(&antwort(0, 0, Hash::sha256(b"a")), &Hash::sha256(b"b"))
            .unwrap();
        assert!(session.is_complete());
        assert!(matches!(
            session.process_response(&antwort(1, 0, Hash::sha256(b"a")), &Hash::sha256(b"b")),
            Err(BisectionError::AlreadyComplete)
        ));
    }

    /// Regression zu Fund 42: Eine leere Spur ist keine Session.
    #[test]
    fn leere_spur_wird_abgelehnt() {
        assert!(matches!(
            BisectionSession::new(SegmentId::new([1u8; 32]), 0),
            Err(BisectionError::InvalidTraceLength { trace_len: 0 })
        ));
    }

    /// Regression zu Fund 42: Eine absurde Spurlänge darf nicht in eine
    /// Panik in `next_power_of_two()` laufen.
    #[test]
    fn absurde_spurlaenge_wird_abgelehnt() {
        for len in [MAX_SPURLAENGE + 1, usize::MAX] {
            assert!(BisectionSession::new(SegmentId::new([1u8; 32]), len).is_err());
        }
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
