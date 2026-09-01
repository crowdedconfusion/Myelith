//! Was die Netzschicht aushalten muss, wenn ihr jemand Unsinn schickt
//! (K4, Punkt 4.2 „Fuzzing Wire-Protocol-Parser").
//!
//! Die Netzschicht ist die einzige Komponente, in die **jeder** ohne
//! Vorbedingung hineinschreiben kann. Für Konsens braucht man Stake, für
//! einen Pod eine Zuteilung; um Bytes an einen Gossip-Port zu schicken
//! braucht man nichts. Was hier durchkommt, entscheidet, wie viel Arbeit
//! ein Fremder einem Knoten aufzwingen kann.
//!
//! Die Anforderung in einem Satz: **`validate_payload` darf an keiner
//! Eingabe abstürzen**, und was es annimmt, muss es zu Recht annehmen.
//! Eine Panik im Gossip-Pfad ist ein Denial-of-Service, den jeder
//! auslösen kann, der die Adresse kennt.
//!
//! `testnet.rs` prüft, dass zwanzig ehrliche Knoten sich finden und
//! Nachrichten austauschen. Das ist der Erfolgsfall und genau das, was
//! K4 als überrepräsentiert benennt.

use myl_net::validation::{max_payload_bytes, validate_payload, ValidationError};
use myl_net::{GossipTopic, LatencyTracker, PongMessage};
use myl_types::ids::{EpochId, MinerId, PodId, SegmentId};
use myl_types::{segments_root, BlsSecretKey, Challenge, Hash, LatencyAttest, PoIBundle};
use myl_types::latency_attest::{BlsSignatureBytes, PeerIdBytes};

/// Segment-Ids zu Zeugnissen, mit einer aus der Id abgeleiteten
/// Spurwurzel.
///
/// ⚑ Seit Fund 100 bezeugt die Bündelwurzel `Id ‖ Spurwurzel`, nicht
/// mehr die bloße Id. Für diese Tests ist der Inhalt der Spur
/// gleichgültig, ihre Anwesenheit nicht.
fn zeugnisse(ids: &[SegmentId]) -> Vec<myl_types::Segmentzeugnis> {
    ids.iter()
        .map(|id| myl_types::Segmentzeugnis {
            id: *id,
            spurwurzel: myl_types::spurwurzel(&[*id.as_bytes()]).expect("Wurzel"),
        })
        .collect()
}


/// SplitMix64, reproduzierbar und ohne Abhängigkeit. Ein Test, der bei
/// jedem Lauf andere Zahlen zieht, meldet einen Fehler einmal und danach
/// nie wieder.
struct Rng(u64);
impl Rng {
    fn neu(keim: u64) -> Self {
        Self(keim)
    }
    fn u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| (self.u64() & 0xff) as u8).collect()
    }
}

fn gueltige_challenge() -> Challenge {
    Challenge {
        segment_id: SegmentId::new([1u8; 32]),
        first_divergence: 3,
        primary_miner: MinerId::new([1u8; 32]),
        redundant_miner: MinerId::new([2u8; 32]),
        primary_hash: Hash::sha256(b"primaer"),
        redundant_hash: Hash::sha256(b"redundant"),
        timestamp_ms: 1_000,
        // Unsigniert: Dieser Test prueft die Struktur- und
        // Groessenpruefung, nicht die Unterschrift (Fund 96).
        signature: myl_types::bls::BlsSignature([0u8; 96]),
    }
}

fn gueltiges_attest() -> LatencyAttest {
    LatencyAttest {
        issuer: MinerId::new([7u8; 32]),
        timestamp_ms: 1_000,
        latencies: vec![(PeerIdBytes([1u8; 32]), 25), (PeerIdBytes([2u8; 32]), 40)],
        signature: BlsSignatureBytes([0u8; 96]),
    }
}

fn gueltiges_bundle() -> PoIBundle {
    let sk = BlsSecretKey::key_gen(&[3u8; 32]).expect("key_gen");
    let segments = [SegmentId::new([9u8; 32])];
    PoIBundle {
        epoch: EpochId(2),
        pod: PodId::new([4u8; 32]),
        segments_root: segments_root(&zeugnisse(&segments)).expect("Merkle-Wurzel"),
        vtfe_claimed: 1_000,
        aggregate_sig: sk.sign(b"poi").expect("sign"),
        segmente: 1,
    }
}

// ---------------------------------------------------------------------
// Gegenprobe
// ---------------------------------------------------------------------

/// **Gegenprobe: Die ehrlichen Nachrichten müssen durchkommen.**
///
/// Ohne sie wäre jeder Test darunter wertlos: Eine Validierung, die alles
/// ablehnt, lehnt auch jeden Angriff ab und wehrt trotzdem nichts ab, weil
/// dann gar kein Netz mehr läuft.
#[test]
fn die_ehrlichen_nachrichten_kommen_durch() {
    assert!(validate_payload(
        GossipTopic::Challenges,
        &borsh::to_vec(&gueltige_challenge()).unwrap()
    )
    .is_ok());
    assert!(validate_payload(
        GossipTopic::LatencyAttests,
        &borsh::to_vec(&gueltiges_attest()).unwrap()
    )
    .is_ok());
    assert!(validate_payload(
        GossipTopic::PoiBundles,
        &borsh::to_vec(&gueltiges_bundle()).unwrap()
    )
    .is_ok());
    // Blöcke und Transaktionen haben in L0 nur ein Größenlimit — das ist
    // eine dokumentierte Entscheidung, keine Auslassung.
    assert!(validate_payload(GossipTopic::Blocks, b"beliebig").is_ok());
}

// ---------------------------------------------------------------------
// Angriffe auf den Parser
// ---------------------------------------------------------------------

/// **Angriff: eine Nachricht über dem Topic-Limit.**
///
/// Ohne Limit ist die Weiterverbreitung der Verstärker: Eine große
/// Nachricht kostet den Absender einmal Bandbreite und das Netz
/// n-mal.
#[test]
fn zu_grosse_nachrichten_werden_abgelehnt() {
    for topic in GossipTopic::all() {
        let max = max_payload_bytes(topic);
        let zu_gross = vec![0u8; max + 1];
        assert!(
            matches!(
                validate_payload(topic, &zu_gross),
                Err(ValidationError::TooLarge { .. })
            ),
            "{topic:?}: {} Bytes müssen abgelehnt werden",
            max + 1
        );
        // Und die Grenze selbst muss noch erlaubt sein, sonst ist das
        // Limit in Wahrheit eins kleiner als dokumentiert.
        let genau = vec![0u8; max];
        assert!(
            !matches!(
                validate_payload(topic, &genau),
                Err(ValidationError::TooLarge { .. })
            ),
            "{topic:?}: genau {max} Bytes dürfen nicht am Limit scheitern"
        );
    }
}

/// **Angriff: eine Challenge, die keine ist.**
///
/// Eine Streitanzeige gegen sich selbst oder ohne Abweichung ist
/// kostenlos zu erzeugen und zwingt jeden Empfänger in ein Verfahren.
#[test]
fn unsinnige_challenges_werden_nicht_weiterverbreitet() {
    let mut selbstanzeige = gueltige_challenge();
    selbstanzeige.redundant_miner = selbstanzeige.primary_miner;
    assert!(validate_payload(
        GossipTopic::Challenges,
        &borsh::to_vec(&selbstanzeige).unwrap()
    )
    .is_err());

    let mut ohne_abweichung = gueltige_challenge();
    ohne_abweichung.redundant_hash = ohne_abweichung.primary_hash;
    assert!(validate_payload(
        GossipTopic::Challenges,
        &borsh::to_vec(&ohne_abweichung).unwrap()
    )
    .is_err());
}

/// **Angriff: ein Attest, das die Pod-Bildung vergiften soll.**
///
/// Die Latenzwerte gehen in den `LatencyGraph` und damit in das
/// Geo-Clustering der Pods. Wer sie frei setzen kann, sucht sich seine
/// Pod-Nachbarn aus, und das ist die Vorstufe zur Kollusion.
///
/// Was die Netzschicht ohne Kenntnis des Netzzustands prüfen kann, prüft
/// sie hier: unplausible Werte, doppelte Peers, ein Zeitstempel aus der
/// Zukunft. Die Signatur bleibt ausdrücklich der Konsensschicht, siehe
/// `PayloadValidator`.
#[test]
fn vergiftete_atteste_werden_abgelehnt() {
    let mut zu_hoch = gueltiges_attest();
    zu_hoch.latencies = vec![(PeerIdBytes([1u8; 32]), 10_001)];
    assert!(validate_payload(
        GossipTopic::LatencyAttests,
        &borsh::to_vec(&zu_hoch).unwrap()
    )
    .is_err());

    let mut doppelt = gueltiges_attest();
    doppelt.latencies = vec![
        (PeerIdBytes([1u8; 32]), 10),
        (PeerIdBytes([1u8; 32]), 9_000),
    ];
    assert!(
        validate_payload(GossipTopic::LatencyAttests, &borsh::to_vec(&doppelt).unwrap()).is_err(),
        "derselbe Peer zweimal: sonst hinge das Ergebnis daran, welchen Eintrag der Empfänger nimmt"
    );

    let mut zukunft = gueltiges_attest();
    zukunft.timestamp_ms = u64::MAX;
    assert!(validate_payload(
        GossipTopic::LatencyAttests,
        &borsh::to_vec(&zukunft).unwrap()
    )
    .is_err());
}

/// **Angriff: verstümmelte, aber strukturell plausible Nachrichten.**
///
/// Rein zufällige Bytes kommen an Borsh nicht vorbei, weil es zuerst
/// Längenfelder liest — der Fuzzer im COMPUTE_PIPELINE traf damit in
/// 50 000 Versuchen keine einzige gültige Nachricht und prüfte deshalb
/// die interessante Hälfte gar nicht. Gezogen wird deshalb
/// **strukturiert**: ein gültiger Datensatz, ab einer zufälligen Stelle
/// überschrieben. Die Auskunftszeile hält fest, wie viele durchkommen.
#[test]
fn verstuemmelte_nachrichten_stuerzen_nicht_ab() {
    let mut rng = Rng::neu(0xC0FF_EE42);
    let vorlagen: Vec<(GossipTopic, Vec<u8>)> = vec![
        (GossipTopic::Challenges, borsh::to_vec(&gueltige_challenge()).unwrap()),
        (GossipTopic::LatencyAttests, borsh::to_vec(&gueltiges_attest()).unwrap()),
        (GossipTopic::PoiBundles, borsh::to_vec(&gueltiges_bundle()).unwrap()),
    ];

    // **Je Topic gezählt, nicht in einer Summe.** Eine Gesamtquote kann
    // hoch aussehen, weil ein Topic mit lauter Feldern fester Länge fast
    // alles annimmt, während das Topic mit dem Vektor gar nicht erreicht
    // wird. Genau diese Verwechslung machte den ersten Pod-Fuzzer
    // wertlos.
    for (topic, gut) in &vorlagen {
        let mut angenommen = 0usize;
        for _ in 0..20_000 {
            let mut roh = gut.clone();
            let ab = (rng.u64() as usize) % roh.len();
            for b in roh.iter_mut().skip(ab) {
                *b = (rng.u64() & 0xff) as u8;
            }
            if validate_payload(*topic, &roh).is_ok() {
                angenommen += 1;
            }
        }
        eprintln!("  {topic:?}: {angenommen} von 20.000 verstümmelten kamen durch");
        assert!(
            angenommen > 100,
            "{topic:?}: nur {angenommen} von 20.000 kamen durch; der Test prüft \
             dort die interessante Hälfte nicht und ist wertlos"
        );
    }
}

/// **⚑ Fund 45: Für Typen fester Länge ist die Borsh-Prüfung eine
/// Längenprüfung, sonst nichts.**
///
/// Die Moduldoku von `validation.rs` nannte das eine „vollständige
/// Borsh-Strukturprüfung gegen den `myl-types`-Typ". Das klingt nach
/// einem Filter und ist keiner: `PoIBundle` besteht ausschließlich aus
/// Feldern fester Länge (zwei u64, drei Byte-Arrays), und **jede**
/// Bytefolge der richtigen Länge ist ein gültiges `PoIBundle`. Der
/// Fuzzer bestätigt es mit 20 000 von 20 000.
///
/// Bei `Challenge` ist es dasselbe, nur mit zwei Ungleichungen davor,
/// die zufällige Bytes praktisch immer erfüllen. Nur `LatencyAttest`
/// filtert wirklich, und zwar wegen seines **Vektors**: dort steht eine
/// Länge im Kopf, die zu den Daten passen muss.
///
/// **Das ist kein Fehler, sondern eine Eigenschaft des Formats**, und der
/// Grund, es festzuhalten: Es sagt, wo die Verteidigung wirklich sitzt.
/// Für PoI-Bündel ist es die Aggregatsignatur, die L0 nicht prüfen kann;
/// bis ein `PayloadValidator` verdrahtet ist, wird ein Bündel aus
/// Zufallsbytes **weiterverbreitet**. Wer die Moduldoku las, konnte das
/// Gegenteil annehmen.
#[test]
fn fuer_feste_typen_ist_die_borsh_pruefung_eine_laengenpruefung() {
    let mut rng = Rng::neu(0xFE57);
    let laenge = borsh::to_vec(&gueltiges_bundle()).unwrap().len();

    let mut angenommen = 0usize;
    for _ in 0..2_000 {
        if validate_payload(GossipTopic::PoiBundles, &rng.bytes(laenge)).is_ok() {
            angenommen += 1;
        }
    }
    assert_eq!(
        angenommen, 2_000,
        "erwartet war, dass reiner Zufall der richtigen Länge als PoI-Bündel durchgeht; \
         geht er das nicht mehr, hat jemand eine echte Prüfung ergänzt und diese \
         Doku gehört nachgezogen"
    );
}

/// **Angriff: gekippte Bits in gültigen Nachrichten.**
#[test]
fn gekippte_bits_stuerzen_nicht_ab() {
    let mut rng = Rng::neu(0x0BAD_C0DE);
    let gut = borsh::to_vec(&gueltiges_attest()).unwrap();
    for _ in 0..20_000 {
        let mut kaputt = gut.clone();
        for _ in 0..=(rng.u64() % 3) {
            let i = (rng.u64() as usize) % kaputt.len();
            kaputt[i] ^= 1 << (rng.u64() % 8);
        }
        let _ = validate_payload(GossipTopic::LatencyAttests, &kaputt);
    }
}

/// **Angriff: abgeschnittene Nachrichten.**
///
/// Der häufigste Netzfehler überhaupt, und der, bei dem eine Längenangabe
/// im Kopf gefährlich wird: Borsh liest sie vor den Daten.
#[test]
fn abgeschnittene_nachrichten_stuerzen_nicht_ab() {
    for (topic, gut) in [
        (GossipTopic::Challenges, borsh::to_vec(&gueltige_challenge()).unwrap()),
        (GossipTopic::LatencyAttests, borsh::to_vec(&gueltiges_attest()).unwrap()),
        (GossipTopic::PoiBundles, borsh::to_vec(&gueltiges_bundle()).unwrap()),
    ] {
        for n in 0..gut.len() {
            let _ = validate_payload(topic, &gut[..n]);
        }
    }
}

/// **Angriff: reiner Zufall gegen jedes Topic.**
///
/// Die Erwartung ist, dass fast alles abgelehnt wird. Geprüft wird die
/// Abwesenheit einer Panik, nicht eine Trefferquote.
#[test]
fn zufaellige_bytes_gegen_jedes_topic() {
    let mut rng = Rng::neu(0xDEAD_BEEF);
    for topic in GossipTopic::all() {
        for _ in 0..10_000 {
            let n = (rng.u64() % 1024) as usize;
            let _ = validate_payload(topic, &rng.bytes(n));
        }
    }
}

/// **Angriff: eine leere Nutzlast.**
///
/// Der billigste Angriff überhaupt und der, der am ehesten in einen
/// Indexzugriff auf ein leeres Feld läuft.
#[test]
fn leere_nutzlasten_werden_beurteilt_ohne_absturz() {
    for topic in GossipTopic::all() {
        let _ = validate_payload(topic, &[]);
    }
}

// ---------------------------------------------------------------------
// Angriffe auf die Latenzmessung
// ---------------------------------------------------------------------

/// **Angriff: ein Pong ohne Ping.**
///
/// Ohne Korrelation könnte ein Fremder Latenzwerte für Verbindungen
/// erfinden, die es nie gab.
#[test]
fn ein_pong_ohne_ping_wird_abgelehnt() {
    let mut t = LatencyTracker::new();
    let peer = libp2p::PeerId::random();
    let pong = PongMessage {
        original_timestamp_ms: 1_000,
        nonce: 42,
        response_timestamp_ms: 1_050,
    };
    assert!(!t.handle_pong(peer, &pong));
    assert!(t.get_latency(&peer).is_none());
}

/// **Angriff: dieselbe Pong-Antwort mehrfach einspielen.**
///
/// Die zweite muss scheitern, sonst ließe sich eine einzige günstige
/// Messung beliebig oft in die EMA drücken, bis sie den Wert bestimmt.
#[test]
fn eine_pong_antwort_gilt_nur_einmal() {
    let mut t = LatencyTracker::new();
    let peer = libp2p::PeerId::random();
    let ping = t.create_ping(peer);
    let pong = PongMessage {
        original_timestamp_ms: ping.timestamp_ms,
        nonce: ping.nonce,
        response_timestamp_ms: ping.timestamp_ms + 5,
    };
    assert!(t.handle_pong(peer, &pong), "die erste gilt");
    let nach_erster = t.get_latency(&peer).unwrap().measurement_count;
    for _ in 0..100 {
        assert!(!t.handle_pong(peer, &pong), "jede weitere nicht");
    }
    assert_eq!(
        t.get_latency(&peer).unwrap().measurement_count,
        nach_erster,
        "ein Replay darf die Messzahl nicht erhöhen"
    );
}

/// **Angriff: zufällige Pongs in beliebiger Reihenfolge.**
///
/// Der Tracker darf daran nicht abstürzen, und er darf für einen Peer,
/// der nie geantwortet hat, keine Latenz führen.
#[test]
fn zufaellige_pongs_stuerzen_nicht_ab() {
    let mut rng = Rng::neu(0x5EED_0042);
    let mut t = LatencyTracker::new();
    let peers: Vec<libp2p::PeerId> = (0..8).map(|_| libp2p::PeerId::random()).collect();
    let stumm = libp2p::PeerId::random();

    for _ in 0..20_000 {
        let peer = peers[(rng.u64() as usize) % peers.len()];
        match rng.u64() % 4 {
            0 => {
                let _ = t.create_ping(peer);
            }
            1 => t.remove_peer(&peer),
            2 => t.cleanup_stale_pings(),
            _ => {
                let pong = PongMessage {
                    original_timestamp_ms: rng.u64(),
                    nonce: rng.u64(),
                    response_timestamp_ms: rng.u64(),
                };
                let _ = t.handle_pong(peer, &pong);
            }
        }
        // Ein Peer, an den nie ein Ping ging, darf nie eine Latenz haben.
        let pong = PongMessage {
            original_timestamp_ms: rng.u64(),
            nonce: rng.u64(),
            response_timestamp_ms: rng.u64(),
        };
        assert!(!t.handle_pong(stumm, &pong));
        assert!(t.get_latency(&stumm).is_none());
    }
}

/// **Die geglättete Latenz bleibt in ihren Grenzen.**
///
/// Sie geht in die Pod-Bildung ein. Ein Wert nahe `u64::MAX` wäre für
/// einen Angreifer der Weg, sich aus jedem Pod herauszurechnen; ein
/// Umlauf nach unten der Weg, sich in jeden hinein.
#[test]
fn die_geglaettete_latenz_bleibt_in_ihren_grenzen() {
    let mut rng = Rng::neu(0xF00D);
    let mut t = LatencyTracker::new();
    for _ in 0..5_000 {
        let peer = libp2p::PeerId::random();
        let ping = t.create_ping(peer);
        let pong = PongMessage {
            original_timestamp_ms: rng.u64(),
            nonce: ping.nonce,
            response_timestamp_ms: rng.u64(),
        };
        t.handle_pong(peer, &pong);
    }
    for l in t.all_latencies().values() {
        assert!(
            l.smoothed_rtt_us <= 10_000_000,
            "geglättete Latenz {} µs jenseits der Plausibilitätsgrenze",
            l.smoothed_rtt_us
        );
    }
}
