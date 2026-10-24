//! **Die Frist im Knoten** (Stufe 4 des Dispatchers, 6.8, Festlegung des
//! Projektinhabers vom 2026-10-22): messen, bezeugen, sammeln, einreichen.
//!
//! # ⚑ Der Weg
//!
//! 1. Einmal je Epoche stellt jedes Mitglied eines Pods jedem anderen
//!    rechnenden Mitglied eine **Zeitprobe** und misst sie, dazu einen
//!    **Ping** über denselben Kanal, mehrmals: Die Probe dauert Weg plus
//!    Rechnung, der kürzeste Ping nur den Weg ([`gemessen_je_token`]).
//! 2. Liegt die Messung über der Frist ([`frist_des_mitglieds`]), unterschreibt
//!    es ein **Fristzeugnis** und schickt es an die übrigen Mitglieder.
//! 3. Jedes Mitglied **sammelt** Zeugnisse ([`Fristsammlung`]); sobald eine
//!    Mehrheit der übrigen über der Frist liegt und es selbst darunter ist,
//!    reicht es die Meldung ein. Mehrere Einreichungen schaden nicht: Die
//!    Kette nimmt eine und weist die übrigen ab.
//!
//! ⚑ **Jedes Zeugnis ist eine eigene Messung.** Ein Mitglied, das eine
//! fremde Zahl unterschriebe, bezeugte nichts; darum misst jedes selbst.
//!
//! ⚠️ **Zwei Grenzen, beide bekannt.** Eine Meldung gilt nur in ihrer
//! Epoche: Kommt sie erst nach dem Wechsel in einen Block, weist die Kette
//! sie ab, und der Verstoß fällt erst bei der Messung der nächsten Epoche
//! auf. Und der Gemessene beantwortet auch die Pings; wer jeden davon
//! verzögert, verkleinert die gemessene Rechenzeit um diese Verzögerung.

use myl_ledger::state::LedgerState;
use myl_types::fristmeldung::{Fristmeldung, Fristzeugnis};
use myl_types::ids::{EpochId, MinerId};

/// Wie viele Positionen eine Zeitprobe rechnet.
pub const PROBE_POSITIONEN: u32 = 4;

/// **Die Frist eines Mitglieds in der laufenden Epoche**, in Mikrosekunden
/// je Token: das Doppelte seiner angesagten Zeit über seine Ebenen, samt
/// Kopf in Ebenenzeiten, wenn es ihn trägt. `None`, wenn es keine Position
/// hat oder der Pod nicht machbar ist.
pub fn frist_des_mitglieds(zustand: &LedgerState, pod_index: u32, mitglied: &MinerId) -> Option<u64> {
    let pm = zustand.podmodell?;
    let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(zustand);
    let pod = zuteilung.pods.iter().find(|p| p.pod_index == pod_index)?;
    let stelle = pod.shards.iter().position(|s| s.miner.miner_id == *mitglied)?;
    let grenzen = crate::kette::Kette::grenzen_des_pods(zustand, pod)?;
    let (von, bis) = grenzen[stelle];
    let kopf = if stelle + 1 == pod.shards.len() { pm.kopf_zeit } else { 0 };
    let zusage = myl_ledger::transitions::modellzusage_der_epoche(zustand, mitglied, zustand.epoch)?;
    Some(zusage.frist_us(bis - von + kopf))
}

/// Wie viele Pings einer Zeitprobe vorausgehen; der kürzeste zählt als Weg.
pub const PINGS_JE_MESSUNG: u32 = 3;

/// **Was eine Messung je Token ergibt**: die Probe ohne den Weg, durch die
/// Positionen der Probe, aufgerundet.
///
/// 📌 **Fund 590 (2026-10-22, im Netzlauf): Eine Probe, die nicht länger
/// dauert als der Ping, ist keine Messung** (`None`). Bis dahin ergab sie
/// null, und null hiess „unter der Frist, für diese Epoche erledigt“. Ein
/// einzelner Ping, der zufällig lang war, weil das Gegenüber gerade Blöcke
/// nachholte, reichte so, um einen Lügner mit einer Frist von 50 µs bei vier
/// von sechs Messenden durchzulassen. Jetzt zählt der kürzeste von
/// [`PINGS_JE_MESSUNG`] Pings, und eine Messung ohne Rechenzeit wird
/// wiederholt statt bestanden.
pub fn gemessen_je_token(probe_us: u64, ping_us: u64, positionen: u32) -> Option<u64> {
    let rechnung = probe_us.checked_sub(ping_us).filter(|r| *r > 0)?;
    Some(rechnung.div_ceil(positionen.max(1) as u64))
}

/// Die Mitglieder eines Pods, die eine Position haben, ohne `ich`: wen
/// dieser Knoten misst.
pub fn zu_messen(zustand: &LedgerState, ich: &MinerId) -> Vec<(u32, myl_types::miner::MinerRegistration)> {
    let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(zustand);
    let Some(pod) = zuteilung.pods.iter().find(|p| p.mitglieder().any(|m| m.miner_id == *ich)) else {
        return Vec::new();
    };
    pod.shards.iter().filter(|s| s.miner.miner_id != *ich).map(|s| (pod.pod_index, s.miner)).collect()
}

/// **Wer diesen Knoten misst**: alle anderen Mitglieder seines Pods, die
/// Reserve eingeschlossen (sie rechnet nicht und kann gerade deshalb
/// bezeugen). Nur ihnen beantwortet er eine Zeitprobe (Fund 609).
pub fn messende(zustand: &LedgerState, ich: &MinerId) -> Vec<myl_types::miner::MinerRegistration> {
    let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(zustand);
    let Some(pod) = zuteilung.pods.iter().find(|p| p.mitglieder().any(|m| m.miner_id == *ich)) else {
        return Vec::new();
    };
    pod.mitglieder().filter(|m| m.miner_id != *ich).copied().collect()
}

/// **Darf `von` diesem Knoten eine Zeitprobe stellen?** Nur, wenn es ein
/// anderes Mitglied seines Pods ist (Fund 609).
pub fn misst_mich(zustand: &LedgerState, ich: &MinerId, von: &libp2p::PeerId) -> bool {
    messende(zustand, ich)
        .iter()
        .any(|m| myl_net::peer_id_aus_bytes(&m.netzadresse).is_ok_and(|p| p == *von))
}

/// Der Schlüssel einer Meldung: Epoche, Pod, Gemeldeter.
type Meldungsschluessel = (EpochId, u32, MinerId);

/// **Sammelt Fristzeugnisse**, bis eine Meldung trägt.
#[derive(Debug, Default)]
pub struct Fristsammlung {
    zeugnisse: std::collections::BTreeMap<Meldungsschluessel, Vec<Fristzeugnis>>,
    eingereicht: std::collections::BTreeSet<Meldungsschluessel>,
}

impl Fristsammlung {
    /// **Nimmt ein Zeugnis auf** und liefert die Meldung, sobald sie trägt
    /// und noch nicht eingereicht wurde.
    ///
    /// Geprüft wird vor der Aufnahme, was die Kette prüfen wird: die
    /// laufende Epoche, die Unterschrift unter dem registrierten Schlüssel,
    /// ein Zeuge aus dem Pod und nicht der Gemeldete, keiner doppelt. Ein
    /// Zeugnis, das die Kette verwürfe, belegt hier keinen Platz.
    pub fn aufnehmen(
        &mut self,
        zustand: &LedgerState,
        epoche: EpochId,
        pod: u32,
        gemeldeter: MinerId,
        zeugnis: Fristzeugnis,
    ) -> Option<Fristmeldung> {
        if epoche != zustand.epoch {
            return None;
        }
        let jetzt = zustand.epoch;
        self.zeugnisse.retain(|(e, _, _), _| *e == jetzt);
        self.eingereicht.retain(|(e, _, _)| *e == jetzt);
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(zustand);
        let p = zuteilung.pods.iter().find(|x| x.pod_index == pod)?;
        let mitglieder: Vec<MinerId> = p.mitglieder().map(|m| m.miner_id).collect();
        if zeugnis.zeuge == gemeldeter || !mitglieder.contains(&zeugnis.zeuge) {
            return None;
        }
        let reg = zustand.miner.get(&zeugnis.zeuge)?;
        let probe = Fristmeldung { epoche, pod, gemeldeter, zeugnisse: Vec::new() };
        if !zeugnis.ist_vom_zeugen(&reg.schluessel, &probe) {
            return None;
        }
        let frist = frist_des_mitglieds(zustand, pod, &gemeldeter)?;
        let schluessel = (epoche, pod, gemeldeter);
        let liste = self.zeugnisse.entry(schluessel).or_default();
        if liste.iter().any(|z| z.zeuge == zeugnis.zeuge) {
            return None;
        }
        liste.push(zeugnis);
        let ueber: Vec<Fristzeugnis> = liste.iter().filter(|z| z.gemessen_us > frist).cloned().collect();
        if ueber.len() < myl_types::gegenzeichnung::mindestzeichner(mitglieder.len())
            || self.eingereicht.contains(&schluessel)
        {
            return None;
        }
        self.eingereicht.insert(schluessel);
        Some(Fristmeldung { epoche, pod, gemeldeter, zeugnisse: ueber })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_consensus::block::BLOECKE_JE_EPOCHE;

    #[test]
    fn die_messung_zieht_den_weg_ab_und_rundet_auf() {
        assert_eq!(gemessen_je_token(10_000, 2_000, 4), Some(2_000));
        assert_eq!(gemessen_je_token(10_001, 2_000, 4), Some(2_001));
        assert_eq!(gemessen_je_token(1_000, 5_000, 4), None, "ein langer Ping macht keine Messung (Fund 590)");
        assert_eq!(gemessen_je_token(5_000, 5_000, 4), None, "ohne Rechenzeit keine Messung");
        assert_eq!(gemessen_je_token(900, 0, 0), Some(900), "null Positionen zählt als eine");
    }

    /// Eine Probekette mit einem Pod aus sechs Probekonten in Epoche 1.
    fn kette_mit_pod() -> crate::kette::Kette {
        let mut k = crate::kette::Kette::probestand();
        for w in 0..6u8 {
            k.aufnehmen(crate::kette::probeanmeldung(w));
            k.aufnehmen(crate::kette::probezusage(w, EpochId(1), 1));
        }
        k.baue_block();
        while k.zustand().epoch.0 < 3 {
            k.baue_block();
        }
        let _ = BLOECKE_JE_EPOCHE;
        k
    }

    /// ⚑ **Eine Zeitprobe beantwortet nur, wer zum eigenen Pod gehört**
    /// (Fund 609): jedes andere Mitglied ja, die Reserve eingeschlossen,
    /// ein Fremder nicht, und ein Knoten ohne Pod niemandem.
    #[test]
    fn eine_zeitprobe_stellen_nur_die_mitglieder_des_eigenen_pods() {
        let k = kette_mit_pod();
        let mut z = k.zustand().clone();
        // Eigene Netzadressen statt der Null der Probeanmeldung, sonst
        // waeren alle dieselbe Gegenstelle.
        let ids: Vec<MinerId> = z.miner.keys().copied().collect();
        for (i, id) in ids.iter().enumerate() {
            let schluessel = libp2p::identity::Keypair::ed25519_from_bytes([i as u8 + 1; 32]).expect("Schluessel");
            let roh = schluessel.public().try_into_ed25519().expect("ed25519").to_bytes();
            if let Some(m) = z.miner.get_mut(id) {
                m.netzadresse = myl_types::latency_attest::PeerIdBytes(roh);
            }
        }
        let peer = |m: &myl_types::miner::MinerRegistration| myl_net::peer_id_aus_bytes(&m.netzadresse).expect("PeerId");
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(&z);
        let pod = &zuteilung.pods[0];
        let ich = pod.shards[0].miner.miner_id;
        let mitglieder: Vec<_> = pod.mitglieder().copied().collect();
        assert!(mitglieder.len() >= 2);
        for m in mitglieder.iter().filter(|m| m.miner_id != ich) {
            let aktuell = z.miner[&m.miner_id];
            assert!(misst_mich(&z, &ich, &peer(&aktuell)), "ein Mitglied darf messen");
        }
        let eigen = z.miner[&ich];
        assert!(!misst_mich(&z, &ich, &peer(&eigen)), "sich selbst misst niemand");
        let fremd = libp2p::PeerId::random();
        assert!(!misst_mich(&z, &ich, &fremd), "ein Fremder darf keine Zeitprobe stellen");
        let ohne_pod = MinerId::new([0xEE; 32]);
        assert!(!misst_mich(&z, &ohne_pod, &peer(&eigen)), "ohne Pod beantwortet niemand eine Probe");
    }

    /// ⚑ **Vom Zeugnis zur Meldung**: Die Frist steht aus der Kette (300 µs
    /// je Ebene, doppelt); Zeugnisse unter der Frist zählen nicht, ein
    /// Fremder und eine falsche Unterschrift belegen keinen Platz, und
    /// sobald drei von fünf übrigen über der Frist liegen, kommt die
    /// Meldung, einmal.
    #[test]
    fn eine_mehrheit_ueber_der_frist_ergibt_eine_meldung() {
        let k = kette_mit_pod();
        let z = k.zustand();
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(z);
        let pod = &zuteilung.pods[0];
        let gemeldeter = pod.shards[1].miner.miner_id;
        let frist = frist_des_mitglieds(z, pod.pod_index, &gemeldeter).expect("Frist");
        let grenzen = crate::kette::Kette::grenzen_des_pods(z, pod).expect("Grenzen");
        assert_eq!(frist, 300 * (grenzen[1].1 - grenzen[1].0) as u64 * 2);
        // Wer zeugt: alle übrigen Mitglieder, mit ihrem Probeschlüssel.
        let nummer = |m: &MinerId| (0..6u8).find(|w| MinerId::new(*crate::kette::probekonto(*w).as_bytes()) == *m).unwrap();
        let zeugen: Vec<MinerId> = pod.mitglieder().map(|m| m.miner_id).filter(|m| *m != gemeldeter).collect();
        let zeugnis = |m: &MinerId, us: u64| {
            Fristzeugnis::neu(&crate::kette::probeschluessel(nummer(m)), *m, z.epoch, pod.pod_index, &gemeldeter, us)
                .expect("Zeugnis")
        };
        let mut s = Fristsammlung::default();
        let mut auf = |zg: Fristzeugnis| s.aufnehmen(z, z.epoch, pod.pod_index, gemeldeter, zg);
        assert!(auf(zeugnis(&zeugen[0], frist)).is_none(), "genau die Frist ist kein Verstoss");
        assert!(auf(zeugnis(&zeugen[1], frist + 1)).is_none());
        let mut falsch = zeugnis(&zeugen[2], frist + 9);
        falsch.gemessen_us += 1;
        assert!(auf(falsch).is_none(), "falsche Unterschrift");
        assert!(auf(zeugnis(&zeugen[2], frist + 2)).is_none(), "zwei ueber der Frist reichen nicht");
        let meldung = auf(zeugnis(&zeugen[3], frist + 3)).expect("drei von fuenf");
        assert_eq!(meldung.zeugnisse.len(), 3);
        assert!(auf(zeugnis(&zeugen[4], frist + 4)).is_none(), "einmal eingereicht");
        // Und die Kette nimmt sie an.
        let mut st = z.clone();
        let mitglieder: Vec<MinerId> = pod.mitglieder().map(|m| m.miner_id).collect();
        let kontext = myl_ledger::transitions::Fristkontext {
            mitglieder,
            ebenen: grenzen[1].1 - grenzen[1].0,
            mit_kopf: false,
        };
        let einreicher = myl_types::ids::Address::new(*meldung.zeugnisse[0].zeuge.as_bytes());
        myl_ledger::transitions::fristverstoss_einreichen(&mut st, &einreicher, &meldung, &kontext).expect("Kette nimmt an");
    }
}
