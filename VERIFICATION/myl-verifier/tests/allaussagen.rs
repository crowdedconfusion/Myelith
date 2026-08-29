//! Eigenschaftstests für die Allaussagen der Verifikation.
//!
//! ## ⚑ Warum ausgerechnet hier
//!
//! **Fund 42 saß in der Bisektion.** Drei Tests waren grün, sie hießen
//! „konvergiert nach O(log L) Runden" und „grenzt auf ein Intervall der
//! Länge 1 ein", und **keiner prüfte, ob die genannte Position die
//! richtige ist.** Das Verfahren belohnte in fünfzehn von sechzehn
//! Fällen den Betrüger.
//!
//! Festgehalten wurde damals: **Ein Generator über alle Positionen
//! aller Spurlängen hätte es in Sekunden gezeigt.** Diese Datei ist
//! dieser Generator, und er ist nicht zufällig, sondern **erschöpfend**.
//!
//! ## ⚑ Erschöpfend schlägt zufällig, wo der Raum klein genug ist
//!
//! Für Spurlängen bis 64 und jede Position darin sind es 2 080 Spiele,
//! und die laufen in Millisekunden. **Ein Zufallstest über denselben
//! Raum ließe Lücken und bräuchte Verkleinerung, um brauchbar zu
//! melden**; ein erschöpfender lässt keine und meldet den kleinsten
//! Fall von selbst, weil er bei den kleinen anfängt.

use myl_types::hash::Hash;
use myl_types::ids::SegmentId;
use myl_verifier::bisection::{BisectionResponse, BisectionResult, BisectionSession};

/// Der Hash, den der Prüfer an einer Position erwartet.
fn pruefer_hash(position: usize) -> Hash {
    Hash::sha256(format!("pruefer-{position}").as_bytes())
}

/// Der Hash, den der Angeklagte liefert: bis `erste_abweichung` derselbe
/// wie der des Prüfers, ab dort ein anderer.
fn angeklagter_hash(position: usize, erste_abweichung: usize) -> Hash {
    if position < erste_abweichung {
        pruefer_hash(position)
    } else {
        Hash::sha256(format!("angeklagter-{position}").as_bytes())
    }
}

/// Spielt eine Bisektion zu Ende und liefert ihr Ergebnis.
fn spiele(trace_len: usize, erste_abweichung: usize) -> BisectionResult {
    let mut sitzung =
        BisectionSession::new(SegmentId::new([3u8; 32]), trace_len).expect("Session");
    while let Some(anfrage) = sitzung.next_request() {
        if sitzung.is_complete() {
            break;
        }
        let antwort = BisectionResponse {
            round: anfrage.round,
            position: anfrage.position,
            activation_hash: angeklagter_hash(anfrage.position, erste_abweichung),
        };
        sitzung
            .process_response(&antwort, &pruefer_hash(anfrage.position))
            .expect("Antwort passt zur Anfrage");
    }
    sitzung.result()
}

/// ⚑ **Die Aussage, die Fund 42 gebrochen hat, jetzt erschöpfend
/// geprüft:** Für jede Spurlänge und jede erste Abweichung darin nennt
/// das Spiel **genau diese** Position.
#[test]
fn die_bisektion_nennt_fuer_jede_spur_und_jede_stelle_die_richtige_position() {
    for trace_len in 1usize..=64 {
        for erste_abweichung in 0..trace_len {
            let ergebnis = spiele(trace_len, erste_abweichung);
            assert_eq!(
                ergebnis,
                BisectionResult::DivergenceFound { position: erste_abweichung },
                "Spurlänge {trace_len}, erste Abweichung {erste_abweichung}"
            );
        }
    }
}

/// Und der Gegenfall: Wer nirgends abweicht, wird nicht verurteilt.
///
/// ⚑ **Das ist die andere Hälfte, und ohne sie wäre der Test oben
/// erfüllbar, indem man immer schuldig sagt.**
#[test]
fn wer_nirgends_abweicht_wird_fuer_keine_spurlaenge_verurteilt() {
    for trace_len in 1usize..=64 {
        // `erste_abweichung == trace_len` heißt: nirgends.
        let ergebnis = spiele(trace_len, trace_len);
        assert_eq!(
            ergebnis,
            BisectionResult::NoDivergence,
            "Spurlänge {trace_len} ohne Abweichung"
        );
    }
}

/// Die Rundenzahl bleibt für jede Spurlänge im zugesagten Rahmen.
///
/// Bisher an einzelnen Längen geprüft; hier für **jede** bis 1024, und
/// gegen die Zusage `ceil(log2(L))` statt gegen eine getippte Zahl.
#[test]
fn die_rundenzahl_bleibt_fuer_jede_spurlaenge_logarithmisch() {
    for trace_len in 1usize..=1024 {
        let zugesagt = BisectionSession::expected_rounds(trace_len);
        let noetig = usize::BITS - trace_len.next_power_of_two().leading_zeros() - 1;
        assert!(
            zugesagt >= noetig,
            "Spurlänge {trace_len}: zugesagt {zugesagt}, nötig {noetig}"
        );
        // Und das Spiel bleibt darunter, an der schwersten Stelle.
        let mut sitzung =
            BisectionSession::new(SegmentId::new([4u8; 32]), trace_len).expect("Session");
        let mut runden = 0u32;
        while let Some(a) = sitzung.next_request() {
            if sitzung.is_complete() {
                break;
            }
            let antwort = BisectionResponse {
                round: a.round,
                position: a.position,
                activation_hash: angeklagter_hash(a.position, trace_len - 1),
            };
            sitzung.process_response(&antwort, &pruefer_hash(a.position)).expect("Antwort");
            runden += 1;
        }
        assert!(
            runden <= zugesagt,
            "Spurlänge {trace_len}: {runden} Runden, zugesagt höchstens {zugesagt}"
        );
    }
}
