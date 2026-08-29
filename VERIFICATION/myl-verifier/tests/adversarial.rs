//! Was die Verifikation aushalten muss, wenn jemand lügt (K4, Punkt 4.4).
//!
//! Die Verifikation ist die Stelle, an der das Protokoll Geld bewegt:
//! Sie entscheidet, wer geschlachtet wird und wer das Kopfgeld bekommt.
//! Ein Fehler hier ist teurer als anderswo, weil er nicht nur etwas
//! zulässt, sondern etwas **auszahlt**.
//!
//! Die Bestandstests des Crates prüfen den Erfolgsfall, und genau das
//! benennt K4. Sie prüfen zum Beispiel, dass das Bisektionsspiel „nach
//! O(log L) Runden konvergiert" und „auf ein Intervall der Länge 1
//! eingrenzt". **Keiner prüfte, ob die genannte Position die richtige
//! ist.** Sie war es nicht, in 15 von 16 Fällen (Fund 42).
//!
//! Jeder Test hier beschreibt einen Angriff und verlangt, dass er
//! scheitert; die drei Gegenproben zuerst, weil eine Prüfung, die alles
//! ablehnt, auch jeden Angriff ablehnt.

use myl_verifier::{
    adjudicate, compare_commitments, create_challenge, create_slash_decision, AdjudicationRequest,
    AdjudicationResponse, AdjudicationResult, BisectionError, BisectionResponse, BisectionResult,
    BisectionSession, ChallengeError, CompareResult, ShardExecutor, SlashError, VerdictOutcome,
};
use myl_types::hash::Hash;
use myl_types::ids::{MinerId, SegmentId};

fn segment(b: u8) -> SegmentId {
    SegmentId::new([b; 32])
}
fn miner(b: u8) -> MinerId {
    MinerId::new([b; 32])
}

fn geheim(b: u8) -> myl_types::bls::BlsSecretKey {
    myl_types::bls::BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("Schlüssel")
}

/// Eine Kennung mit einem Schlüssel dahinter.
///
/// ⚑ Seit dem 2026-08-29 braucht ein Schuldspruch gegen den primären Pod
/// einen unterschriebenen Übergang. `miner(b)` hat keinen Schlüssel und
/// kann deshalb keinen liefern; das ist keine Testhürde, sondern die
/// Eigenschaft selbst.
fn kennung(b: u8) -> MinerId {
    MinerId::aus_schluessel(&geheim(b).public_key().expect("Punkt"))
}

fn beleg_fuer(b: u8, seg: SegmentId) -> myl_verifier::Schuldbeleg {
    let sk = geheim(b);
    let uebergang = myl_types::uebergang::TransitionSig {
        segment_id: seg,
        shard_index: 0,
        position: 0,
        prev_hash: [0u8; 32],
        next_hash: [7u8; 32],
    };
    let signatur = uebergang.sign(&sk).expect("signieren");
    myl_verifier::Schuldbeleg {
        uebergang,
        schluessel: sk.public_key().expect("Punkt"),
        signatur,
    }
}

/// xorshift64, reproduzierbar und ohne Abhängigkeit. Ein Test, der bei
/// jedem Lauf andere Zahlen zieht, meldet einen Fehler einmal und danach
/// nie wieder.
struct Wuerfel(u64);
impl Wuerfel {
    fn neu(keim: u64) -> Self {
        Self(keim | 1)
    }
    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn bis(&mut self, n: u64) -> u64 {
        self.naechste() % n
    }
}

/// Zwei **verkettete** Spuren: ab `d` weichen sie ab und bleiben
/// verschieden. Die Verkettung ist die Voraussetzung der Bisektion, nicht
/// eine Bequemlichkeit des Tests: `a_j` hängt von `a_{j-1}` ab, also
/// pflanzt sich die erste Abweichung bis zum Ende fort.
fn spuren(n: usize, d: usize) -> (Vec<Hash>, Vec<Hash>) {
    let checker: Vec<Hash> = (0..n).map(|i| Hash::sha256(&(i as u64).to_le_bytes())).collect();
    let mut angeklagter = checker.clone();
    for (i, h) in angeklagter.iter_mut().enumerate().skip(d) {
        *h = Hash::sha256(&[&(i as u64).to_le_bytes()[..], b"gefaelscht"].concat());
    }
    (checker, angeklagter)
}

/// Spielt das Bisektionsspiel ehrlich durch und gibt zurück, welche
/// Position es nennt.
fn spiel(n: usize, checker: &[Hash], angeklagter: &[Hash]) -> BisectionResult {
    let mut s = BisectionSession::new(segment(1), n).expect("Spurlänge");
    while let Some(req) = s.next_request() {
        let antwort = BisectionResponse {
            round: req.round,
            position: req.position,
            activation_hash: angeklagter[req.position],
        };
        s.process_response(&antwort, &checker[req.position])
            .expect("ehrliche Antwort");
    }
    s.result()
}

// ---------------------------------------------------------------------
// Gegenproben
// ---------------------------------------------------------------------

/// **Gegenprobe 1, und der Test, der Fund 42 gefunden hat: Das Spiel muss
/// die richtige Layer nennen.**
///
/// Das ist die einzige Aussage, auf die es ankommt. Konvergenz und
/// Rundenzahl sind Nebenbedingungen; wenn die genannte Position falsch
/// ist, rechnet die Schiedsrunde die falsche Layer nach, spricht den
/// Betrüger frei und schlachtet den ehrlichen Checker.
///
/// Durchgefahren wird **jede** Abweichungsposition jeder Spurlänge,
/// nicht eine ausgesuchte.
#[test]
fn das_spiel_nennt_die_richtige_layer() {
    for n in [1usize, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 24, 28, 32, 64, 100] {
        for d in 0..n {
            let (checker, angeklagter) = spuren(n, d);
            assert_eq!(
                spiel(n, &checker, &angeklagter),
                BisectionResult::DivergenceFound { position: d },
                "Spurlänge {n}, echte Abweichung bei {d}"
            );
        }
    }
}

/// **Gegenprobe 2: Wer nirgends abweicht, wird nicht verurteilt.**
///
/// `NoDivergence` und `DivergenceFound` müssen unterscheidbar bleiben,
/// sonst verurteilt das Spiel jeden, gegen den es eröffnet wird.
#[test]
fn ohne_abweichung_kein_schuldspruch() {
    for n in [1usize, 2, 7, 16, 33, 128] {
        let (checker, _) = spuren(n, n); // d == n: nirgends abweichend
        assert_eq!(
            spiel(n, &checker, &checker),
            BisectionResult::NoDivergence,
            "Spurlänge {n}"
        );
    }
}

/// **Gegenprobe 3: Die ehrliche Schiedsrunde spricht frei.**
#[test]
fn die_ehrliche_schiedsrunde_spricht_frei() {
    let eingabe = b"aktivierung-j-minus-1".to_vec();
    let ausgabe = b"aktivierung-j".to_vec();
    let anfrage = AdjudicationRequest {
        segment_id: segment(1),
        divergence_position: 7,
        checker: miner(2),
        accused: miner(3),
        input_hash: Hash::sha256(&eingabe),
        expected_hash: Hash::sha256(&ausgabe),
    };
    let antwort = AdjudicationResponse {
        segment_id: segment(1),
        divergence_position: 7,
        activation_hash: Hash::sha256(&eingabe),
        activation: eingabe,
    };
    assert_eq!(
        adjudicate(&anfrage, Some(&antwort), &FesterExecutor(ausgabe)),
        AdjudicationResult::Innocent
    );
}

// ---------------------------------------------------------------------
// Angriffe auf die Schiedsrunde
// ---------------------------------------------------------------------

/// Ein Executor, der immer dasselbe liefert, egal was er bekommt.
struct FesterExecutor(Vec<u8>);
impl ShardExecutor for FesterExecutor {
    fn execute_shard(&self, _a: &[u8], _i: usize) -> Result<Vec<u8>, myl_verifier::AdjudicationError> {
        Ok(self.0.clone())
    }
}

/// Ein Executor, der scheitert.
struct KaputterExecutor;
impl ShardExecutor for KaputterExecutor {
    fn execute_shard(&self, _a: &[u8], _i: usize) -> Result<Vec<u8>, myl_verifier::AdjudicationError> {
        Err(myl_verifier::AdjudicationError::HashMismatch)
    }
}

/// **Angriff: eine andere Eingabe unterschieben, die zufällig zum
/// erwarteten Ergebnis führt** (Regression zu Fund A11).
///
/// Der Angeklagte legt eine Aktivierung offen, die **nicht** die
/// committete ist, und sein Rechenweg liefert darauf trotzdem das
/// erwartete Ergebnis. `FesterExecutor` modelliert genau diesen Fall im
/// Extrem: Er liefert das Erwartete für **jede** Eingabe.
///
/// Ohne die Bindung an `request.input_hash` wäre das ein Freispruch, und
/// zwar für jeden Angeklagten, der eine solche Eingabe findet.
#[test]
fn eine_andere_eingabe_rettet_den_angeklagten_nicht() {
    let committet = b"die-committete-aktivierung".to_vec();
    let untergeschoben = b"etwas-ganz-anderes".to_vec();
    let ausgabe = b"das-erwartete-ergebnis".to_vec();

    let anfrage = AdjudicationRequest {
        segment_id: segment(1),
        divergence_position: 7,
        checker: miner(2),
        accused: miner(3),
        input_hash: Hash::sha256(&committet),
        expected_hash: Hash::sha256(&ausgabe),
    };
    // In sich völlig konsistent: der Hash passt zur offengelegten Eingabe.
    let antwort = AdjudicationResponse {
        segment_id: segment(1),
        divergence_position: 7,
        activation_hash: Hash::sha256(&untergeschoben),
        activation: untergeschoben,
    };

    assert_eq!(
        adjudicate(&anfrage, Some(&antwort), &FesterExecutor(ausgabe)),
        AdjudicationResult::Guilty,
        "die Offenlegung muss an das Commitment gebunden sein"
    );
}

/// **Angriff: ein Hash, der nicht zur offengelegten Aktivierung passt.**
#[test]
fn eine_luegende_selbstauskunft_wird_verurteilt() {
    let committet = b"aktivierung".to_vec();
    let anfrage = AdjudicationRequest {
        segment_id: segment(1),
        divergence_position: 3,
        checker: miner(2),
        accused: miner(3),
        input_hash: Hash::sha256(&committet),
        expected_hash: Hash::sha256(b"egal"),
    };
    let antwort = AdjudicationResponse {
        segment_id: segment(1),
        divergence_position: 3,
        activation: b"etwas-anderes".to_vec(),
        activation_hash: Hash::sha256(&committet), // passt nicht zum Inhalt
    };
    assert_eq!(
        adjudicate(&anfrage, Some(&antwort), &FesterExecutor(b"egal".to_vec())),
        AdjudicationResult::Guilty
    );
}

/// **Angriff: eine gültige Antwort aus einem anderen Streitfall
/// wiedereinspielen.**
#[test]
fn eine_antwort_aus_einem_anderen_streit_gilt_nicht() {
    let eingabe = b"aktivierung".to_vec();
    let ausgabe = b"ergebnis".to_vec();
    let anfrage = AdjudicationRequest {
        segment_id: segment(1),
        divergence_position: 7,
        checker: miner(2),
        accused: miner(3),
        input_hash: Hash::sha256(&eingabe),
        expected_hash: Hash::sha256(&ausgabe),
    };

    // Falsches Segment.
    let fremd = AdjudicationResponse {
        segment_id: segment(9),
        divergence_position: 7,
        activation_hash: Hash::sha256(&eingabe),
        activation: eingabe.clone(),
    };
    assert_eq!(
        adjudicate(&anfrage, Some(&fremd), &FesterExecutor(ausgabe.clone())),
        AdjudicationResult::Guilty
    );

    // Falsche Position: sonst ließe sich die Offenlegung einer anderen,
    // korrekt gerechneten Layer für die strittige einsetzen.
    let verschoben = AdjudicationResponse {
        segment_id: segment(1),
        divergence_position: 6,
        activation_hash: Hash::sha256(&eingabe),
        activation: eingabe,
    };
    assert_eq!(
        adjudicate(&anfrage, Some(&verschoben), &FesterExecutor(ausgabe)),
        AdjudicationResult::Guilty
    );
}

/// **Angriff: gar nicht antworten.**
///
/// Schweigen darf kein Freispruch sein, sonst ist es die beste Strategie
/// jedes Betrügers.
#[test]
fn schweigen_ist_kein_freispruch() {
    let anfrage = AdjudicationRequest {
        segment_id: segment(1),
        divergence_position: 7,
        checker: miner(2),
        accused: miner(3),
        input_hash: Hash::sha256(b"a"),
        expected_hash: Hash::sha256(b"b"),
    };
    assert_ne!(
        adjudicate(&anfrage, None, &FesterExecutor(b"b".to_vec())),
        AdjudicationResult::Innocent
    );
}

/// **Angriff: eine Eingabe liefern, an der die Ausführung scheitert.**
///
/// Wer eine Aktivierung offenlegt, die den Rechenweg zum Absturz bringt,
/// darf daraus keinen Vorteil ziehen.
#[test]
fn ein_gescheiterter_rechenweg_ist_kein_freispruch() {
    let eingabe = b"aktivierung".to_vec();
    let anfrage = AdjudicationRequest {
        segment_id: segment(1),
        divergence_position: 7,
        checker: miner(2),
        accused: miner(3),
        input_hash: Hash::sha256(&eingabe),
        expected_hash: Hash::sha256(b"ergebnis"),
    };
    let antwort = AdjudicationResponse {
        segment_id: segment(1),
        divergence_position: 7,
        activation_hash: Hash::sha256(&eingabe),
        activation: eingabe,
    };
    assert_eq!(
        adjudicate(&anfrage, Some(&antwort), &KaputterExecutor),
        AdjudicationResult::Guilty
    );
}

/// **Zufällige Schiedsrunden sprechen nie frei und stürzen nie ab.**
#[test]
fn zufaellige_schiedsrunden_sprechen_nie_frei() {
    let mut w = Wuerfel::neu(0xA11);
    for _ in 0..20_000 {
        let aktivierung: Vec<u8> = (0..(w.bis(40))).map(|_| w.bis(256) as u8).collect();
        let anfrage = AdjudicationRequest {
            segment_id: segment(w.bis(4) as u8),
            divergence_position: w.bis(64) as usize,
            checker: miner(w.bis(8) as u8),
            accused: miner(w.bis(8) as u8),
            input_hash: Hash::sha256(&w.naechste().to_le_bytes()),
            expected_hash: Hash::sha256(&w.naechste().to_le_bytes()),
        };
        let antwort = AdjudicationResponse {
            segment_id: segment(w.bis(4) as u8),
            divergence_position: w.bis(64) as usize,
            activation_hash: Hash::sha256(&w.naechste().to_le_bytes()),
            activation: aktivierung,
        };
        assert_ne!(
            adjudicate(&anfrage, Some(&antwort), &FesterExecutor(b"x".to_vec())),
            AdjudicationResult::Innocent,
            "ein zufälliger Streitfall darf nie zum Freispruch führen"
        );
    }
}

// ---------------------------------------------------------------------
// Angriffe auf das Bisektionsspiel
// ---------------------------------------------------------------------

/// **Angriff: in der falschen Runde antworten.**
#[test]
fn eine_antwort_in_der_falschen_runde_wird_abgelehnt() {
    let mut s = BisectionSession::new(segment(1), 16).unwrap();
    let req = s.next_request().unwrap();
    let antwort = BisectionResponse {
        round: req.round + 3,
        position: req.position,
        activation_hash: Hash::sha256(b"a"),
    };
    assert!(matches!(
        s.process_response(&antwort, &Hash::sha256(b"b")),
        Err(BisectionError::InvalidRound { .. })
    ));
    assert_eq!(s.rounds, 0, "eine abgelehnte Antwort verbraucht keine Runde");
}

/// **Angriff: eine Offenlegung zu einer anderen Position einsetzen.**
///
/// Der Angeklagte hat an Position `p` richtig gerechnet und an `q`
/// falsch. Antwortet er auf die Frage nach `q` mit seiner Offenlegung zu
/// `p`, wäre er einig mit dem Checker und die Suche liefe in die falsche
/// Hälfte.
#[test]
fn eine_offenlegung_zur_falschen_position_wird_abgelehnt() {
    let mut s = BisectionSession::new(segment(1), 16).unwrap();
    let req = s.next_request().unwrap();
    let antwort = BisectionResponse {
        round: req.round,
        position: req.position + 1,
        activation_hash: Hash::sha256(b"a"),
    };
    assert!(matches!(
        s.process_response(&antwort, &Hash::sha256(b"a")),
        Err(BisectionError::PositionMismatch { .. })
    ));
    assert_eq!(s.lower, 0);
    assert_eq!(s.upper, 16);
}

/// **Angriff: das Spiel endlos ziehen.**
///
/// Wer immer weiter antwortet, darf die Session nicht offen halten. Die
/// Runden sind gedeckelt, und nach dem Deckel nimmt sie nichts mehr an.
#[test]
fn das_spiel_laesst_sich_nicht_endlos_ziehen() {
    for n in [2usize, 16, 1024] {
        let mut s = BisectionSession::new(segment(1), n).unwrap();
        let mut runden = 0u32;
        while let Some(req) = s.next_request() {
            let antwort = BisectionResponse {
                round: req.round,
                position: req.position,
                activation_hash: Hash::sha256(b"immer-dasselbe"),
            };
            s.process_response(&antwort, &Hash::sha256(b"immer-anders")).unwrap();
            runden += 1;
            assert!(runden <= s.max_rounds, "Spurlänge {n}: über den Deckel hinaus");
        }
        assert!(s.is_complete());
        // Nach dem Ende nimmt die Session nichts mehr an.
        let nachzuegler = BisectionResponse {
            round: s.rounds,
            position: 0,
            activation_hash: Hash::sha256(b"x"),
        };
        assert!(matches!(
            s.process_response(&nachzuegler, &Hash::sha256(b"y")),
            Err(BisectionError::AlreadyComplete)
        ));
    }
}

/// **Angriff: eine unbrauchbare Spurlänge unterschieben.**
///
/// Die leere Spur war bis v0.3.2 ein sofortiger Schuldspruch gegen
/// Layer 0, ohne eine einzige Runde. Absurd große Längen ließen
/// `next_power_of_two()` überlaufen, also eine Panik im
/// Schiedsrichter-Prozess.
#[test]
fn unbrauchbare_spurlaengen_werden_abgelehnt() {
    for len in [0usize, (1usize << 62) + 1, usize::MAX, usize::MAX - 1] {
        assert!(
            BisectionSession::new(segment(1), len).is_err(),
            "Spurlänge {len} muss abgelehnt werden"
        );
    }
}

/// **Zufällige Antwortfolgen: nie eine Panik, immer ein Ende, und das
/// genannte Ergebnis ist nie widersprüchlich.**
#[test]
fn zufaellige_antwortfolgen_stuerzen_nie_ab() {
    let mut w = Wuerfel::neu(0x5EED_0042);
    for _ in 0..20_000 {
        let n = (w.bis(200) + 1) as usize;
        let mut s = BisectionSession::new(segment(1), n).unwrap();
        let mut schritte = 0u32;
        while let Some(req) = s.next_request() {
            let antwort = BisectionResponse {
                round: if w.bis(20) == 0 { w.bis(10) as u32 } else { req.round },
                position: if w.bis(20) == 0 { w.bis(300) as usize } else { req.position },
                activation_hash: Hash::sha256(&w.naechste().to_le_bytes()),
            };
            let checker_hash = if w.bis(2) == 0 {
                antwort.activation_hash
            } else {
                Hash::sha256(&w.naechste().to_le_bytes())
            };
            let _ = s.process_response(&antwort, &checker_hash);
            schritte += 1;
            assert!(schritte < 10_000, "die Session kommt nicht zum Ende");
        }
        // Was am Ende genannt wird, muss innerhalb der Spur liegen.
        if let BisectionResult::DivergenceFound { position } = s.result() {
            assert!(position < n, "genannte Position {position} liegt außerhalb von {n}");
        }
    }
}

// ---------------------------------------------------------------------
// Angriffe auf Challenge, Vergleich und Schuldspruch
// ---------------------------------------------------------------------

/// **Angriff: eine Challenge ohne Abweichung eröffnen.**
///
/// Sonst wäre die Challenge ein kostenloses Werkzeug, um jeden beliebigen
/// Miner in ein Verfahren zu ziehen.
#[test]
fn eine_challenge_ohne_abweichung_wird_abgelehnt() {
    let hashes: Vec<Hash> = (0..10).map(|i| Hash::sha256(&[i as u8])).collect();
    assert!(matches!(
        create_challenge(segment(1), 5, miner(2), miner(3), &hashes, &hashes, 1000),
        Err(ChallengeError::NoDivergence)
    ));
}

/// **Angriff: eine Challenge auf eine Position außerhalb der Spur.**
#[test]
fn eine_challenge_ausserhalb_der_spur_wird_abgelehnt() {
    let (checker, angeklagter) = spuren(10, 3);
    for pos in [10usize, 11, 1000, usize::MAX] {
        assert!(matches!(
            create_challenge(segment(1), pos, miner(2), miner(3), &checker, &angeklagter, 1000),
            Err(ChallengeError::InvalidPosition { .. })
        ));
    }
}

/// **Angriff: eine verkürzte Spur einreichen, um den Betrug abzuschneiden.**
///
/// Wer an Layer 9 betrügt und nur die ersten fünf Commitments abliefert,
/// darf nicht mit `Match` davonkommen. `compare_commitments` lehnt
/// ungleiche Längen ab, und das ist hier der ganze Schutz.
#[test]
fn eine_verkuerzte_spur_gilt_nicht_als_uebereinstimmung() {
    let (checker, angeklagter) = spuren(10, 9);
    assert!(compare_commitments(&checker, &angeklagter[..5]).is_err());
    assert!(compare_commitments(&checker, &[]).is_err());
    assert!(compare_commitments(&[], &checker).is_err());
    // Und ungekürzt wird der Betrug gefunden.
    assert_eq!(
        compare_commitments(&checker, &angeklagter).unwrap(),
        CompareResult::Mismatch { first_divergence: 9 }
    );
}

/// **Angriff: sich selbst herausfordern, um das Kopfgeld einzustreichen.**
///
/// Ein Miner, der beide Pods stellt, könnte sonst einen Streit gegen sich
/// selbst inszenieren: Er verliert Stake an sich selbst und gewinnt das
/// Kopfgeld, das aus dem Stake des Verlierers stammt, also aus seinem
/// eigenen. Der Nettoeffekt hängt an den Slash-Anteilen und ist in keinem
/// Fall etwas, das das Protokoll anbieten sollte.
#[test]
fn niemand_fordert_sich_selbst_heraus() {
    for outcome in [VerdictOutcome::PrimaryLoses, VerdictOutcome::RedundantLoses] {
        assert!(matches!(
            create_slash_decision(
                outcome,
                segment(1),
                miner(7),
                miner(7),
                Some(3),
                Some(&beleg_fuer(7, segment(1))),
            ),
            Err(SlashError::IdenticalMiners)
        ));
    }
}

/// **Der Schuldspruch trifft immer den, der verloren hat, und nie
/// denselben zweimal.**
#[test]
fn geschlachteter_und_belohnter_sind_nie_dieselbe_partei() {
    let mut w = Wuerfel::neu(0xDEC1_5107);
    for _ in 0..5_000 {
        let (a, b) = (w.bis(64) as u8, w.bis(64) as u8);
        let outcome = if w.bis(2) == 0 {
            VerdictOutcome::PrimaryLoses
        } else {
            VerdictOutcome::RedundantLoses
        };
        let beleg = beleg_fuer(a, segment(1));
        match create_slash_decision(
            outcome,
            segment(1),
            kennung(a),
            kennung(b),
            None,
            Some(&beleg),
        ) {
            Ok(d) => {
                assert_ne!(d.slashed_miner, d.rewarded_miner);
                let verlierer = match outcome {
                    VerdictOutcome::PrimaryLoses => kennung(a),
                    VerdictOutcome::RedundantLoses => kennung(b),
                };
                assert_eq!(d.slashed_miner, verlierer);
            }
            Err(SlashError::IdenticalMiners) => assert_eq!(a, b),
            Err(e) => panic!("unerwarteter Fehler bei ({a}, {b}): {e}"),
        }
    }
}
