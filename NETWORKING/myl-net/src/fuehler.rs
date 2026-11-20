//! Fuehler: eine Gegenstelle kommt erst nach einer eigenen, ausgehenden
//! Verbindung in die Routingtabelle (Audit A9, 2026-11-19).
//!
//! # Die Luecke
//!
//! Kademlia selbst nimmt eine Gegenstelle nur ueber eine Verbindung auf, die
//! der Knoten gewaehlt hat (libp2p-kad: bei einer eingehenden ist die Adresse
//! unbekannt). Seit Fund 587 trug die Laufzeit aber jede Horchadresse, die
//! eine Gegenstelle ueber `identify` nannte, selbst ein, **auch bei
//! eingehenden Verbindungen**. Damit fuellte ein Angreifer die Tabelle eines
//! Knotens, indem er sich mit vielen Identitaeten zu ihm verband und
//! beliebige Adressen nannte; das ist der billige Weg der Umzingelung, den
//! Marcus et al. 2018 an Ethereum zeigen (ungefragte eingehende Verbindungen
//! fuellen die Knotentabelle).
//!
//! # Die Abwehr, nach dem Muster der Fuehler von Bitcoin Core
//!
//! Meldet sich eine Gegenstelle ueber eine **eingehende** Verbindung, waehlt
//! der Knoten selbst eine ihrer genannten Adressen an, mit der erwarteten
//! Kennung. Kommt die Verbindung zustande, hat der Noise-Handshake belegt,
//! dass unter dieser Adresse genau diese Gegenstelle antwortet: Die Adresse
//! geht in die Tabelle, und die Fuehlerverbindung wird sofort wieder
//! geschlossen (sonst koennte ein Angreifer mit eingehenden Verbindungen die
//! **ausgehenden** Plaetze fuellen lassen). Scheitert sie, bleibt die
//! Gegenstelle draussen.
//!
//! ⚑ **Der Fuehler waehlt aus einem eigenen, kleinen Swarm mit einer
//! fluechtigen Identitaet** ([`schwarm`]). 📌 Zuerst waehlte er aus dem
//! Swarm des Knotens: Dann hatte die Gegenseite zwei Verbindungen zu
//! derselben Kennung und verteilte ihre Anfragen reihum auch auf die
//! Fuehlerverbindung; schloss der Knoten sie, rissen diese Anfragen ab
//! („Connection was closed before a response was received“, in
//! `tests/sitzung.rs` zwei von fuenf Laeufen). Mit einer fremden Kennung ist
//! der Fuehler fuer die Gegenseite eine andere Gegenstelle, und es landet
//! nichts auf ihm. Er spricht kein Protokoll ausser dem Handshake.
//!
//! Das kostet einen Angreifer pro Eintrag eine erreichbare Adresse statt
//! einer erfundenen; zusammen mit der Grenze je Netzgruppe
//! ([`crate::limits::MAX_AUSGEHEND_JE_GRUPPE`]) braucht er viele erreichbare
//! Adressen in vielen Netzen.
//!
//! Begrenzt ist alles: hoechstens [`MAX_FUEHLER_OFFEN`] zugleich,
//! [`MAX_FUEHLER_WARTEND`] in der Warteschlange, je Gegenstelle einer je
//! [`FUEHLER_WIEDER_NACH`].

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use libp2p::swarm::ConnectionId;
use libp2p::{Multiaddr, PeerId};

/// Fuehler, die zugleich laufen. Klein gegen [`crate::limits::MAX_AUSSTEHEND_AUSGEHEND`]:
/// Fuehler sollen das Waehlen des Knotens nicht verdraengen.
pub const MAX_FUEHLER_OFFEN: usize = 4;

/// Gegenstellen, die auf einen Fuehler warten. Was darueber kommt, wird
/// verworfen; eine ehrliche Gegenstelle findet der Knoten auch ueber die
/// Suchen in Kademlia.
pub const MAX_FUEHLER_WARTEND: usize = 64;

/// Nach so langer Zeit darf dieselbe Gegenstelle erneut gefuehlt werden.
pub const FUEHLER_WIEDER_NACH: Duration = Duration::from_secs(3600);

/// So lange darf ein Fuehler dauern, von der Wahl bis zur Ausweisung der
/// Gegenstelle; danach wird er geschlossen und zaehlt als gescheitert.
pub const FUEHLER_FRIST: Duration = Duration::from_secs(20);

/// So viele Gegenstellen merkt sich der Fuehler hoechstens.
pub const FUEHLER_GEDAECHTNIS: usize = 1024;

/// Der Swarm der Fuehler: TCP und QUIC mit Noise, Namen ueber DNS, kein
/// Verhalten, eine bei jedem Start neue Identitaet. Er horcht nicht.
pub fn schwarm() -> Result<libp2p::Swarm<libp2p::swarm::dummy::Behaviour>, String> {
    Ok(libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(libp2p::tcp::Config::default(), libp2p::noise::Config::new, libp2p::yamux::Config::default)
        .map_err(|e| format!("Fuehler TCP: {e}"))?
        .with_quic()
        .with_dns()
        .map_err(|e| format!("Fuehler DNS: {e}"))?
        .with_behaviour(|_| libp2p::swarm::dummy::Behaviour)
        .map_err(|e| format!("Fuehler: {e}"))?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(1)))
        .build())
}

/// Die Buchfuehrung der Fuehler, ohne Swarm (pruefbar ohne Netz).
#[derive(Debug, Default)]
pub struct Fuehler {
    offen: HashMap<ConnectionId, (PeerId, Instant)>,
    wartend: VecDeque<(PeerId, Vec<Multiaddr>)>,
    gefuehlt: HashMap<PeerId, Instant>,
}

impl Fuehler {
    /// Eine Gegenstelle zum Fuehlen vormerken. `false`, wenn sie kuerzlich
    /// gefuehlt wurde, schon wartet, keine Adresse hat oder die Schlange
    /// voll ist.
    pub fn vormerken(&mut self, peer: PeerId, adressen: Vec<Multiaddr>, jetzt: Instant) -> bool {
        if adressen.is_empty() {
            return false;
        }
        if self.gefuehlt.get(&peer).is_some_and(|t| jetzt.duration_since(*t) < FUEHLER_WIEDER_NACH) {
            return false;
        }
        if self.wartend.iter().any(|(p, _)| *p == peer) || self.offen.values().any(|(p, _)| *p == peer) {
            return false;
        }
        if self.wartend.len() >= MAX_FUEHLER_WARTEND {
            return false;
        }
        self.wartend.push_back((peer, adressen));
        true
    }

    /// Die naechste Gegenstelle, die jetzt gefuehlt werden darf; merkt sie
    /// als gefuehlt.
    pub fn naechster(&mut self, jetzt: Instant) -> Option<(PeerId, Vec<Multiaddr>)> {
        if self.offen.len() >= MAX_FUEHLER_OFFEN {
            return None;
        }
        let (peer, adressen) = self.wartend.pop_front()?;
        self.gefuehlt.insert(peer, jetzt);
        if self.gefuehlt.len() > FUEHLER_GEDAECHTNIS {
            self.gefuehlt.retain(|_, t| jetzt.duration_since(*t) < FUEHLER_WIEDER_NACH);
            while self.gefuehlt.len() > FUEHLER_GEDAECHTNIS {
                let aeltester = self.gefuehlt.iter().min_by_key(|(_, t)| **t).map(|(p, _)| *p);
                match aeltester {
                    Some(p) => self.gefuehlt.remove(&p),
                    None => break,
                };
            }
        }
        Some((peer, adressen))
    }

    /// Die Wahl ist unterwegs.
    pub fn gestartet(&mut self, id: ConnectionId, peer: PeerId, jetzt: Instant) {
        self.offen.insert(id, (peer, jetzt));
    }

    /// Ist diese Verbindung ein Fuehler, und zu wem?
    pub fn ist_fuehler(&self, id: &ConnectionId) -> Option<PeerId> {
        self.offen.get(id).map(|(p, _)| *p)
    }

    /// Der Fuehler ist fertig (gelungen oder gescheitert).
    pub fn beendet(&mut self, id: &ConnectionId) -> Option<PeerId> {
        self.offen.remove(id).map(|(p, _)| p)
    }

    /// Fuehler, die laenger als [`FUEHLER_FRIST`] laufen.
    pub fn abgelaufen(&self, jetzt: Instant) -> Vec<ConnectionId> {
        self.offen
            .iter()
            .filter(|(_, (_, seit))| jetzt.duration_since(*seit) >= FUEHLER_FRIST)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Laufende Fuehler. Fuer Diagnose und Proben.
    pub fn offen(&self) -> usize {
        self.offen.len()
    }

    /// Wartende Gegenstellen. Fuer Diagnose und Proben.
    pub fn wartend(&self) -> usize {
        self.wartend.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adr() -> Vec<Multiaddr> {
        vec!["/ip4/203.0.113.1/tcp/4150".parse().unwrap()]
    }

    #[test]
    fn eine_gegenstelle_wird_einmal_gefuehlt() {
        let mut f = Fuehler::default();
        let t = Instant::now();
        let p = PeerId::random();
        assert!(f.vormerken(p, adr(), t));
        assert!(!f.vormerken(p, adr(), t), "wartet schon");
        let (q, _) = f.naechster(t).unwrap();
        assert_eq!(q, p);
        let id = ConnectionId::new_unchecked(1);
        f.gestartet(id, p, t);
        assert!(!f.vormerken(p, adr(), t), "laeuft schon");
        assert_eq!(f.beendet(&id), Some(p));
        assert!(!f.vormerken(p, adr(), t + Duration::from_secs(10)), "kuerzlich gefuehlt");
        assert!(f.vormerken(p, adr(), t + FUEHLER_WIEDER_NACH), "nach der Frist wieder");
    }

    #[test]
    fn ohne_adresse_kein_fuehler() {
        let mut f = Fuehler::default();
        assert!(!f.vormerken(PeerId::random(), vec![], Instant::now()));
    }

    #[test]
    fn hoechstens_vier_zugleich_und_vierundsechzig_wartend() {
        let mut f = Fuehler::default();
        let t = Instant::now();
        for _ in 0..(MAX_FUEHLER_WARTEND + 10) {
            f.vormerken(PeerId::random(), adr(), t);
        }
        assert_eq!(f.wartend(), MAX_FUEHLER_WARTEND);
        for i in 0..MAX_FUEHLER_OFFEN {
            let (p, _) = f.naechster(t).unwrap();
            f.gestartet(ConnectionId::new_unchecked(i), p, t);
        }
        assert!(f.naechster(t).is_none(), "vier laufen schon");
        f.beendet(&ConnectionId::new_unchecked(0));
        assert!(f.naechster(t).is_some());
    }

    #[test]
    fn ein_haengender_fuehler_laeuft_ab() {
        let mut f = Fuehler::default();
        let t = Instant::now();
        let p = PeerId::random();
        f.vormerken(p, adr(), t);
        f.naechster(t);
        let id = ConnectionId::new_unchecked(7);
        f.gestartet(id, p, t);
        assert!(f.abgelaufen(t + Duration::from_secs(19)).is_empty());
        assert_eq!(f.abgelaufen(t + FUEHLER_FRIST), vec![id]);
    }

    #[test]
    fn das_gedaechtnis_bleibt_begrenzt() {
        let mut f = Fuehler::default();
        let t = Instant::now();
        for i in 0..(FUEHLER_GEDAECHTNIS + 50) {
            let p = PeerId::random();
            f.vormerken(p, adr(), t + Duration::from_millis(i as u64));
            f.naechster(t + Duration::from_millis(i as u64));
        }
        assert!(f.gefuehlt.len() <= FUEHLER_GEDAECHTNIS);
    }
}
