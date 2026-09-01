//! Der Probelauf: Protokollfunktionen über echte Leitungen ausprobieren.
//!
//! # ⚑ Das hier ist **nicht** das Testnetz
//!
//! Steht zuerst, weil eine Verwechslung teuer wäre. Ein Probelauf ist
//! eine **Trockenübung des Codes**, keine Inbetriebnahme der Kette:
//!
//! - **Der Zustand ist Wegwerfware.** Jeder Start beginnt bei null. Es
//!   gibt keine Fortsetzung, keine Wiederherstellung, keine Historie.
//! - **Die MYL sind Spielgeld.** Die Guthaben aus
//!   [`crate::kette::PROBEKONTEN`] stehen in keinem Verhältnis zur
//!   Genesis-Zuteilung des echten Netzes (TOKENOMICS Punkt 4.2). Wer
//!   hier Guthaben sieht, besitzt nichts.
//! - **Die Blöcke sind keine Blöcke einer Kette, die weiterläuft.** Sie
//!   hängen an einem Startwert, den [`crate::kette`] eigens für den
//!   Probelauf festlegt. Ein echtes Netz beginnt bei einem anderen, und
//!   **ein Probeblock kann deshalb niemals an eine echte Kette
//!   anschließen.** Das ist keine Regel, sondern eine Eigenschaft der
//!   Verkettung, und ein Test hält sie fest.
//! - **Es stimmt niemand ab.** Genau ein Knoten erzeugt; die Frage, wer
//!   entscheidet, welcher Block gilt, stellt sich hier gar nicht.
//!
//! Wann das Testnetz beginnt, ist eine Entscheidung des Projekts und
//! keine Folge davon, dass dieser Code läuft.
//!
//! # Was ein Probelauf stattdessen beantwortet
//!
//! **Halten die Funktionen, wenn die Daten über eine Leitung gegangen
//! sind?** Der Durchlauf `myl-test stack` prüft dieselben Bausteine
//! bereits **im selben Prozess**, zehn Stufen von der Kryptografie bis
//! zur Preisbildung. Dort liegen die Werte im Speicher nebeneinander.
//!
//! Hier gehen sie durch Serialisierung, Gossip, Größenprüfung,
//! Strukturprüfung, Weiterverbreitung und Deserialisierung, über
//! Maschinen- und Ländergrenzen. **Das ist ein anderer Weg mit anderen
//! Fehlerarten**, und die Funde dieses Projekts liegen fast alle genau
//! dort: an Nähten, nicht in Modulen.
//!
//! # Die Proben
//!
//! Je Probe ein Protokollobjekt, das wirklich gebraucht wird, auf dem
//! Topic, auf dem es wirklich läuft. Jede schreibt ihr Urteil ins
//! Betriebsprotokoll, und die Auswertung zählt zusammen, **welche
//! Funktion wie oft ausprobiert wurde und wie oft sie hielt**.
//!
//! Eine Probe, die nie lief, ist kein Erfolg. Deshalb nennt die
//! Auswertung auch die, für die nichts vorliegt.

use myl_net::GossipTopic;
use myl_types::bls::{BlsSecretKey, BlsSignature};
use myl_types::challenge::Challenge;
use myl_types::core_types::{segments_root, PoIBundle};
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, MinerId, PodId, SegmentId};

/// Die Protokollfunktionen, die ein Probelauf über echte Leitungen
/// ausprobiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// Knoten finden einander und halten Verbindungen.
    Netz,
    /// Was losgeschickt wurde, kommt unverändert an.
    Nachrichtenfluss,
    /// Aus denselben Blöcken derselbe Zustand.
    Blockkette,
    /// PoI-Bündel überstehen den Weg durch das Netz.
    PoiBuendel,
    /// Challenges überstehen den Weg und bleiben strukturell gültig.
    Challenge,
    /// Transaktionen erreichen den Erzeuger und landen in Blöcken.
    Transaktion,
    /// Latenz-Atteste werden signiert erzeugt und beim Empfänger gegen
    /// den Validatorsatz geprüft (Sicherheitsaudit A10).
    Latenzattest,
}

impl Probe {
    /// Alle Proben in der Reihenfolge, in der sie aufeinander aufbauen.
    pub const ALLE: [Probe; 7] = [
        Probe::Netz,
        Probe::Nachrichtenfluss,
        Probe::Blockkette,
        Probe::PoiBuendel,
        Probe::Challenge,
        Probe::Transaktion,
        Probe::Latenzattest,
    ];

    /// Kurzname fürs Protokoll und für die Auswertung. Stabil halten:
    /// Die Auswertung filtert danach.
    pub fn kennung(&self) -> &'static str {
        match self {
            Probe::Netz => "netz",
            Probe::Nachrichtenfluss => "nachrichtenfluss",
            Probe::Blockkette => "blockkette",
            Probe::PoiBuendel => "poi-buendel",
            Probe::Challenge => "challenge",
            Probe::Transaktion => "transaktion",
            Probe::Latenzattest => "latenzattest",
        }
    }

    /// Was diese Probe belegt, in einem Satz. Steht im Bericht.
    pub fn was_sie_belegt(&self) -> &'static str {
        match self {
            Probe::Netz => "Knoten finden einander und halten Verbindungen",
            Probe::Nachrichtenfluss => "was losgeschickt wurde, kommt unverändert an",
            Probe::Blockkette => "aus denselben Blöcken errechnen alle denselben Zustand",
            Probe::PoiBuendel => "PoI-Bündel überstehen Serialisierung und Gossip",
            Probe::Challenge => "Challenges überstehen den Weg und bleiben gültig",
            Probe::Transaktion => "Transaktionen erreichen den Erzeuger und landen in Blöcken",
            Probe::Latenzattest => "Latenz-Atteste werden signiert und die Signatur geprüft (A10)",
        }
    }

    /// Das Topic, auf dem diese Probe läuft. `None` für Proben, die
    /// sich aus dem Verhalten ergeben statt aus einer eigenen Nachricht.
    pub fn topic(&self) -> Option<GossipTopic> {
        match self {
            Probe::Netz | Probe::Nachrichtenfluss => None,
            Probe::Blockkette => Some(GossipTopic::Blocks),
            Probe::PoiBuendel => Some(GossipTopic::PoiBundles),
            Probe::Challenge => Some(GossipTopic::Challenges),
            Probe::Transaktion => Some(GossipTopic::Transactions),
            Probe::Latenzattest => Some(GossipTopic::LatencyAttests),
        }
    }

    /// Aus dem Kurznamen zurück.
    pub fn aus_kennung(k: &str) -> Option<Probe> {
        Probe::ALLE.into_iter().find(|p| p.kennung() == k)
    }
}

/// Erzeugt ein **echtes, wohlgeformtes** PoI-Bündel für die Probe.
///
/// Es trägt eine gültige Merkle-Wurzel über echte Segment-Ids und eine
/// echte BLS-Signatur. Ein aus Zufallsbytes gebautes Bündel käme durch
/// dieselbe Prüfung (Fund 45) und belegte deshalb nichts über den Weg,
/// den ein echtes nimmt.
pub fn probe_poi_buendel(absender: &str, folge: u64) -> Option<PoIBundle> {
    let saat = Hash::sha256(format!("{absender}#{folge}").as_bytes());
    let ids: Vec<SegmentId> = (0..4u8)
        .map(|i| {
            let mut futter = saat.as_bytes().to_vec();
            futter.push(i);
            let h = Hash::sha256(&futter);
            let mut roh = [0u8; 32];
            roh.copy_from_slice(h.as_bytes());
            SegmentId::new(roh)
        })
        .collect();
    // ⚑ Seit Fund 100 bezeugt die Bündelwurzel `Id ‖ Spurwurzel`, nicht
    // mehr die bloße Id: Ein Bündel, das nur Positionen aufzählt, sagt
    // nicht, **was** gerechnet wurde. Die Probe baut deshalb auch eine
    // Spur, damit sie den Weg prüft, den ein echtes Bündel nimmt.
    let zeugnisse: Vec<myl_types::Segmentzeugnis> = ids
        .iter()
        .map(|id| myl_types::Segmentzeugnis {
            id: *id,
            spurwurzel: myl_types::spurwurzel(&[*id.as_bytes()]).expect("eine Spur ist nicht leer"),
        })
        .collect();
    let wurzel = segments_root(&zeugnisse).ok()?;

    let mut ikm = [0u8; 32];
    ikm.copy_from_slice(saat.as_bytes());
    let sk = BlsSecretKey::key_gen(&ikm).ok()?;
    let mut pod = [0u8; 32];
    pod.copy_from_slice(Hash::sha256(absender.as_bytes()).as_bytes());

    let epoch = EpochId(folge);
    let pod_id = PodId::new(pod);
    let vtfe = 1_000 + folge;

    // Über die kanonischen Bytes signieren, damit die Signatur zum
    // Inhalt gehört und nicht nur ein Platzhalter ist. Ein Bündel mit
    // Zufallssignatur käme durch dieselbe Netzprüfung und belegte
    // deshalb nichts über den Weg, den ein echtes nimmt.
    let botschaft = borsh::to_vec(&(epoch, pod_id, wurzel, vtfe)).ok()?;
    let aggregate_sig: BlsSignature = sk.sign(&botschaft).ok()?;

    Some(PoIBundle {
        epoch,
        pod: pod_id,
        segments_root: wurzel,
        vtfe_claimed: vtfe,
        aggregate_sig,
        segmente: 1,
    })
}

/// Erzeugt eine **strukturell gültige und unterschriebene** Challenge
/// für die Probe.
///
/// Verschiedene Miner und verschiedene Hashes: Genau das prüft die
/// Netzschicht, und eine Probe, die daran scheiterte, prüfte den Weg
/// nicht, sondern die eigene Nachlässigkeit.
///
/// ⚑ **Und seit dem 2026-08-29 unterschrieben** (Fund 96). Der
/// Herausforderer ist der Absender selbst, seine Kennung wird aus dem
/// Probeschlüssel abgeleitet. Eine Probe mit erfundenem
/// Herausforderer und ohne Unterschrift prüfte den Weg nicht, den eine
/// echte Anfechtung nimmt, sondern einen, den es nicht mehr gibt.
///
/// `None`, wenn der Absender keinen Probeschlüssel hat: Dann ist keine
/// Anfechtung erzeugbar, und das ist ehrlicher als eine unsignierte.
pub fn probe_challenge(absender: &str, folge: u64) -> Option<Challenge> {
    let saat = Hash::sha256(format!("{absender}#{folge}").as_bytes());
    /// Hasht die Saat mit einem Zusatz. Getrennte Zusätze ergeben
    /// getrennte Werte, und genau das verlangt die Netzprüfung von
    /// Challenges: verschiedene Miner, verschiedene Hashes.
    fn abgeleitet(saat: &Hash, zusatz: &[u8]) -> Hash {
        let mut futter = saat.as_bytes().to_vec();
        futter.extend_from_slice(zusatz);
        Hash::sha256(&futter)
    }
    let mut seg = [0u8; 32];
    seg.copy_from_slice(saat.as_bytes());
    let mut primaer = [0u8; 32];
    primaer.copy_from_slice(abgeleitet(&saat, b"p").as_bytes());
    let mut redundant = [0u8; 32];
    redundant.copy_from_slice(abgeleitet(&saat, b"r").as_bytes());

    let _ = redundant;
    let sk = crate::validatorsatz::probe_schluessel(absender)?;
    let pk = sk.public_key().ok()?;
    let mut c = Challenge {
        segment_id: SegmentId::new(seg),
        first_divergence: (folge % 32) as usize,
        primary_miner: MinerId::new(primaer),
        // Der Herausforderer ist der Absender, sonst trägt die
        // Unterschrift nicht.
        redundant_miner: MinerId::aus_schluessel(&pk),
        primary_hash: abgeleitet(&saat, b"ph"),
        redundant_hash: abgeleitet(&saat, b"rh"),
        timestamp_ms: crate::protokoll::jetzt_ms().max(0) as u64,
        signature: myl_types::bls::BlsSignature([0u8; 96]),
    };
    c.signiere(&sk).ok()?;
    Some(c)
}

/// Erzeugt ein **signiertes** Latenz-Attest für die Probe.
///
/// # ⚑ Warum das die Probe zu A10 ist
///
/// Bis zum 2026-08-25 erzeugte niemand ein Attest, und niemand prüfte
/// die Signatur. Diese Probe schickt eines mit echter Signatur ins
/// Netz; der Empfänger prüft es gegen den Validatorsatz und verwirft
/// es, wenn etwas nicht stimmt.
///
/// **Die Latenzen sind die tatsächlich gemessenen**, nicht erfundene.
/// Ein Attest mit ausgedachten Werten prüfte nur die Signatur, nicht den
/// Weg, den ein echtes nimmt.
pub fn probe_attest(
    name: &str,
    latenzen: &[(libp2p::PeerId, u32)],
) -> Option<myl_types::LatencyAttest> {
    use myl_types::latency_attest::{BlsSignatureBytes, PeerIdBytes};

    let sk = crate::validatorsatz::probe_schluessel(name)?;
    let issuer = crate::validatorsatz::probe_kennung(name)?;
    let mut werte = Vec::new();
    for (peer, rtt) in latenzen {
        // Die PeerId auf 32 Bytes bringen: Ihre Rohform ist ein
        // Multihash veränderlicher Länge, das Attest-Feld ist fest.
        let h = Hash::sha256(&peer.to_bytes());
        let mut roh = [0u8; 32];
        roh.copy_from_slice(h.as_bytes());
        werte.push((PeerIdBytes(roh), *rtt));
    }
    let mut a = myl_types::LatencyAttest {
        issuer,
        timestamp_ms: crate::protokoll::jetzt_ms().max(0) as u64,
        latencies: werte,
        signature: BlsSignatureBytes([0u8; 96]),
    };
    a.sign(&sk).ok()?;
    Some(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jede_probe_hat_eine_stabile_kennung() {
        // Die Auswertung filtert danach. Eine Umbenennung bricht sie
        // still, deshalb dieser Test.
        for p in Probe::ALLE {
            assert_eq!(Probe::aus_kennung(p.kennung()), Some(p));
            assert!(!p.kennung().is_empty());
            assert!(!p.was_sie_belegt().is_empty());
        }
        assert_eq!(Probe::aus_kennung("gibt-es-nicht"), None);
    }

    #[test]
    fn die_kennungen_sind_verschieden() {
        let mut gesehen = std::collections::HashSet::new();
        for p in Probe::ALLE {
            assert!(gesehen.insert(p.kennung()), "{} doppelt", p.kennung());
        }
    }

    #[test]
    fn ein_probe_buendel_ist_wohlgeformt_und_ueberlebt_borsh() {
        // Der Weg, den es im Netz nimmt: serialisieren, prüfen,
        // zurücklesen. Ein Bündel aus Zufallsbytes käme durch dieselbe
        // Prüfung (Fund 45) und belegte nichts.
        let b = probe_poi_buendel("alpha", 3).expect("Bündel");
        let bytes = borsh::to_vec(&b).expect("Serialisierung");
        assert!(myl_net::validate_payload(GossipTopic::PoiBundles, &bytes).is_ok());
        let zurueck: PoIBundle = borsh::from_slice(&bytes).expect("Rücklesen");
        assert_eq!(zurueck, b, "das Bündel hat den Weg nicht unverändert überstanden");
    }

    #[test]
    fn zwei_buendel_desselben_absenders_sind_verschieden() {
        // Gleiche Nutzlasten hätten dieselbe Nachrichten-Id, und
        // Gossipsub verwürfe die zweite als Dublette: Die Probe liefe
        // dann genau einmal.
        let a = probe_poi_buendel("alpha", 1).unwrap();
        let b = probe_poi_buendel("alpha", 2).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn eine_probe_challenge_besteht_die_netzpruefung() {
        // Verschiedene Miner, verschiedene Hashes: Genau das prüft die
        // Netzschicht. Eine Probe, die daran scheiterte, prüfte den Weg
        // nicht, sondern die eigene Nachlässigkeit.
        let c = probe_challenge("beta", 7).expect("beta hat einen Probeschlüssel");
        assert_ne!(c.primary_miner, c.redundant_miner);
        // ⚑ Und sie ist unterschrieben, sonst prüfte die Probe einen
        // Weg, den eine echte Anfechtung nicht mehr nimmt (Fund 96).
        let pk = crate::validatorsatz::probe_schluessel("beta")
            .expect("Schlüssel")
            .public_key()
            .expect("Punkt");
        assert!(
            c.ist_vom_herausforderer(&pk),
            "die Probe-Challenge trägt keine tragende Unterschrift"
        );
        assert_ne!(c.primary_hash, c.redundant_hash);
        let bytes = borsh::to_vec(&c).expect("Serialisierung");
        assert!(
            myl_net::validate_payload(GossipTopic::Challenges, &bytes).is_ok(),
            "die eigene Probe-Challenge kommt nicht durch die Netzprüfung"
        );
        let zurueck: Challenge = borsh::from_slice(&bytes).expect("Rücklesen");
        assert_eq!(zurueck, c);
    }

    /// **A10: Das Probe-Attest ist echt signiert und wird anerkannt.**
    #[test]
    fn ein_probe_attest_besteht_die_pruefung_des_empfaengers() {
        let satz = crate::validatorsatz::Validatorsatz::aus_namen(&["alpha"]);
        let a = probe_attest("alpha", &[(libp2p::PeerId::random(), 25)]).expect("Attest");
        assert!(
            satz.pruefe(&a).ist_gueltig(),
            "das eigene Probe-Attest wurde abgewiesen"
        );
        // Und es übersteht den Weg durchs Netz.
        let bytes = borsh::to_vec(&a).expect("Serialisierung");
        assert!(myl_net::validate_payload(GossipTopic::LatencyAttests, &bytes).is_ok());
    }

    #[test]
    fn ein_probe_attest_eines_fremden_wird_abgewiesen() {
        let satz = crate::validatorsatz::Validatorsatz::aus_namen(&["alpha"]);
        let a = probe_attest("eindringling", &[(libp2p::PeerId::random(), 25)]).expect("Attest");
        assert!(!satz.pruefe(&a).ist_gueltig());
    }

    #[test]
    fn ein_attest_ohne_latenzen_ist_gueltig() {
        // Ein Knoten ohne Peers hat nichts zu berichten. Das ist kein
        // Fehler, und ein Attest darüber muss trotzdem tragen.
        let satz = crate::validatorsatz::Validatorsatz::aus_namen(&["alpha"]);
        let a = probe_attest("alpha", &[]).expect("Attest");
        assert!(satz.pruefe(&a).ist_gueltig());
    }

    #[test]
    fn die_topics_decken_die_nachrichtenproben_ab() {
        // Netz und Nachrichtenfluss ergeben sich aus dem Verhalten, die
        // übrigen brauchen ein Topic. Fehlt eines, läuft die Probe nie.
        for p in Probe::ALLE {
            match p {
                Probe::Netz | Probe::Nachrichtenfluss => assert!(p.topic().is_none()),
                _ => assert!(p.topic().is_some(), "{} ohne Topic", p.kennung()),
            }
        }
    }
}
