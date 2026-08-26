//! Verbindungsgrenzen und Adressvielfalt (Fahrplanpunkt 4.3, Fund 53).
//!
//! Dieses Modul schließt die Lücke, die `tests/eclipse_sybil.rs` am
//! 2026-08-24 gemessen hat: Zwanzig Sybil-Identitäten verbanden sich mit
//! demselben Opfer, und alle zwanzig wurden angenommen.
//!
//! ## Was hier verteidigt wird, und was ausdrücklich nicht
//!
//! Ein Eclipse-Angriff fälscht nichts. Er **wählt aus**: Wer alle
//! Verbindungen eines Knotens stellt, entscheidet, welche Nachrichten
//! dieser Knoten sieht. Für Myelith ist das die teuerste Netzlücke, weil
//! Stufe 1 und Stufe 2 der Verifikation daran hängen, dass ein Knoten
//! fremde Segmente überhaupt zu Gesicht bekommt.
//!
//! Die Messung hat auch gesagt, worauf eine Gegenmaßnahme zielen muss.
//! Nicht „Sybils abwehren", das ist bei kostenlosen Identitäten
//! aussichtslos, sondern: **mindestens eine ehrliche Verbindung
//! garantieren.** Der Angriff gelingt genau dann, wenn er *alle*
//! Verbindungen stellt.
//!
//! ## Der Mechanismus: getrennte Budgets
//!
//! Der Kern ist eine einzige Unterscheidung.
//!
//! - **Eingehende** Verbindungen wählt der Angreifer. Sie sind die
//!   Flutfläche und werden bei [`MAX_EINGEHEND`] hart gedeckelt.
//! - **Ausgehende** Verbindungen wählt der Knoten selbst. Sie bekommen
//!   ein eigenes Budget von [`MAX_AUSGEHEND`], das eingehende Fluten
//!   nicht anfassen können.
//!
//! Weil [`MAX_GESAMT`] die Summe beider ist und eingehende unabhängig
//! gedeckelt sind, bleiben die ausgehenden Plätze **immer** frei. Ein
//! Angreifer mit unbegrenzt vielen Identitäten kann den Knoten daran
//! nicht hindern, sich 16 Gegenstellen eigener Wahl zu suchen.
//!
//! ## Die ehrliche Restlücke
//!
//! Diese Garantie ist kleiner, als sie klingt, und das gehört hierher
//! statt in eine Fußnote: Sie sichert, dass der Knoten **wählen darf**,
//! nicht dass er **richtig wählt**. Wer wählt, braucht Adressen, und die
//! kommen aus der Bootstrap-Liste und aus Kademlia. Kontrolliert ein
//! Angreifer beide, nützt das freie Budget nichts.
//!
//! **Die Verteidigung reduziert den Eclipse-Angriff also auf eine
//! Bedingung: Die Bootstrap-Liste muss mindestens einen ehrlichen Knoten
//! enthalten.** Das ist ein echter Fortschritt gegenüber „beliebig viele
//! Verbindungen werden angenommen", aber es ist keine Resistenz gegen
//! einen Angreifer, der schon die Bootstrap-Liste stellt. Der Test
//! `eine_ehrliche_bootstrap_verbindung_bleibt_moeglich` prüft genau diese
//! Kette und nicht mehr.
//!
//! ## Adressvielfalt als Preisaufschlag, nicht als Sperre
//!
//! [`Adressvielfalt`] begrenzt eingehende Verbindungen je Adressbereich
//! (IPv4 /24, IPv6 /64) auf [`MAX_JE_ADRESSBEREICH`]. Damit braucht die
//! Flut der 48 eingehenden Plätze mindestens 12 verschiedene /24-Netze
//! statt 20 Prozesse auf einer Maschine.
//!
//! Das ist **keine** Sperre: Wer zwölf Adressbereiche mieten kann, kommt
//! durch. Es ist eine Kostenverschiebung von „ein Rechner" auf „zwölf
//! Netze", und so ist es zu lesen. Ein Test, der daraus Resistenz macht,
//! würde mehr behaupten, als der Mechanismus leistet.
//!
//! **Loopback ist ausgenommen.** Zwei Gründe, beide bewusst: Wer auf
//! `127.0.0.1` Verbindungen aufbauen kann, besitzt die Maschine bereits,
//! und die lokalen Testnetze (20 Knoten in `tests/testnet.rs`) liefen
//! sonst gegen eine Schranke, die für sie nicht gedacht ist.
//!
//! ## Ganzzahlig
//!
//! Alle Werte hier sind Ganzzahlen. Das Gossipsub-Peer-Scoring rechnet in
//! `f64` und liegt deshalb in [`crate::scoring`], mit der Begründung
//! dort und einem Eintrag in den dokumentierten Ausnahmen des
//! Gleitkomma-Audits.

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::task::{Context, Poll};

use libp2p::connection_limits::ConnectionLimits;
use libp2p::core::transport::PortUse;
use libp2p::core::{ConnectedPoint, Endpoint};
use libp2p::multiaddr::Protocol;
use libp2p::swarm::behaviour::{ConnectionClosed, ConnectionEstablished, ListenFailure};
use libp2p::swarm::{
    dummy, ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, THandler,
    THandlerInEvent, THandlerOutEvent, ToSwarm,
};
use libp2p::{Multiaddr, PeerId};

// ---------------------------------------------------------------------
// Die Zahlen und ihre Herleitung
// ---------------------------------------------------------------------

/// Ausgehende Verbindungen, die der Knoten selbst aufbaut.
///
/// **Herleitung:** Gossipsub hält je Topic ein Mesh von bis zu
/// `mesh_n_high = 12` Peers. Weil `subscribe_all` **alle**
/// Protokoll-Topics abonniert und derselbe Peer in mehreren Meshes
/// zugleich steht, ist 12 die Untergrenze für ein volles Mesh, und zwar
/// **unabhängig von der Anzahl der Topics** — nicht 12 je Topic. Dazu
/// vier Plätze Luft für Fluktuation und Bootstrap: **16**.
///
/// *Hier stand bis zum 2026-08-26 „alle fünf Topics" und „nicht 5 mal
/// 12". Die Zahl war eine Kopie und wurde beim sechsten Topic
/// (`Consensus`) falsch. Sie geht in die Herleitung gar nicht ein, also
/// steht sie jetzt nicht mehr da.*
pub const MAX_AUSGEHEND: u32 = 16;

/// Eingehende Verbindungen, die andere zum Knoten aufbauen.
///
/// **Herleitung:** Das Dreifache von [`MAX_AUSGEHEND`]. Ein Knoten muss
/// mehr Verbindungen bedienen, als er selbst aufbaut, sonst kann das
/// Netz nicht wachsen: Jede ausgehende Verbindung braucht anderswo eine
/// eingehende Gegenstelle. Faktor 3 lässt Raum für Knoten, die keine
/// eingehenden Verbindungen annehmen können, etwa hinter NAT.
pub const MAX_EINGEHEND: u32 = 48;

/// Gesamtzahl gleichzeitiger Verbindungen.
///
/// Die **Summe** der beiden Budgets, nicht weniger. Läge sie darunter,
/// könnten eingehende Verbindungen die ausgehenden Plätze aufzehren, und
/// genau das ist der Angriff.
pub const MAX_GESAMT: u32 = MAX_EINGEHEND + MAX_AUSGEHEND;

/// Verbindungen je Peer-Id.
///
/// Zwei, weil gleichzeitiges beidseitiges Wählen in libp2p normal ist
/// (eine eingehende plus eine ausgehende). Mehr hat keinen legitimen
/// Zweck und ist ein Ressourcenangriff mit einer einzigen Identität.
pub const MAX_JE_PEER: u32 = 2;

/// Gleichzeitig laufende eingehende Handshakes.
///
/// Ein Handshake ist billig zu beginnen und teuer abzuschließen
/// (Noise, X25519). Ohne diese Schranke lässt sich der Knoten mit
/// halbfertigen Verbindungen beschäftigen, ohne je eine herzustellen.
/// Ein Drittel von [`MAX_EINGEHEND`].
pub const MAX_AUSSTEHEND_EINGEHEND: u32 = 16;

/// Gleichzeitig laufende ausgehende Wählversuche. Die Hälfte von
/// [`MAX_AUSGEHEND`]: Ein Knoten baut sein Mesh auf, er stürmt es nicht.
pub const MAX_AUSSTEHEND_AUSGEHEND: u32 = 8;

/// Eingehende Verbindungen je Adressbereich.
///
/// **Herleitung:** Bei 4 je Bereich braucht das Füllen der
/// [`MAX_EINGEHEND`] Plätze mindestens `48 / 4 = 12` verschiedene
/// Bereiche. Vier lässt zugleich zu, dass mehrere ehrliche Knoten im
/// selben Rechenzentrum stehen, was üblich ist.
pub const MAX_JE_ADRESSBEREICH: usize = 4;

/// Präfixlänge für IPv4-Adressbereiche: /24, also die ersten drei Oktette.
pub const IPV4_PRAEFIX_BITS: u8 = 24;

/// Präfixlänge für IPv6-Adressbereiche: /64. Das ist die Grenze, unterhalb
/// derer Adressen üblicherweise frei wählbar sind: Ein einzelner Anschluss
/// bekommt oft ein ganzes /64, eine Zählung je Einzeladresse wäre dort
/// wirkungslos.
pub const IPV6_PRAEFIX_BITS: u8 = 64;

/// Die Verbindungsgrenzen des Protokolls.
///
/// Zusammengesetzt aus den Konstanten dieses Moduls. Später Parameter der
/// Governance-Registry; bis dahin Konstanten mit Herleitung.
pub fn standard_grenzen() -> ConnectionLimits {
    ConnectionLimits::default()
        .with_max_pending_incoming(Some(MAX_AUSSTEHEND_EINGEHEND))
        .with_max_pending_outgoing(Some(MAX_AUSSTEHEND_AUSGEHEND))
        .with_max_established_incoming(Some(MAX_EINGEHEND))
        .with_max_established_outgoing(Some(MAX_AUSGEHEND))
        .with_max_established_per_peer(Some(MAX_JE_PEER))
        .with_max_established(Some(MAX_GESAMT))
}

// ---------------------------------------------------------------------
// Adressbereiche
// ---------------------------------------------------------------------

/// Der Adressbereich einer Gegenstelle: das Netz, aus dem sie kommt.
///
/// Zwei Adressen im selben Bereich zählen für [`Adressvielfalt`] als
/// dieselbe Herkunft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Adressbereich {
    /// IPv4 /24: die ersten drei Oktette.
    V4([u8; 3]),
    /// IPv6 /64: die ersten acht Bytes.
    V6([u8; 8]),
}

/// Bestimmt den Adressbereich einer Multiaddr.
///
/// Gibt `None` zurück, wenn die Adresse keinen zählbaren Bereich hat:
/// Loopback (siehe Modul-Doku) oder Adressen ohne IP-Anteil, etwa
/// Relay-Pfade und Unix-Sockets. `None` heißt „nicht zählen", nicht
/// „ablehnen".
pub fn adressbereich(addr: &Multiaddr) -> Option<Adressbereich> {
    for protokoll in addr.iter() {
        match protokoll {
            Protocol::Ip4(ip) => return bereich_v4(ip),
            Protocol::Ip6(ip) => return bereich_v6(ip),
            _ => {}
        }
    }
    None
}

fn bereich_v4(ip: Ipv4Addr) -> Option<Adressbereich> {
    if ip.is_loopback() {
        return None;
    }
    let o = ip.octets();
    Some(Adressbereich::V4([o[0], o[1], o[2]]))
}

fn bereich_v6(ip: Ipv6Addr) -> Option<Adressbereich> {
    if ip.is_loopback() {
        return None;
    }
    // Eine IPv4-in-IPv6-Adresse wird als das gezählt, was sie ist:
    // sonst umginge ein Angreifer die /24-Zählung durch die Schreibweise.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return bereich_v4(v4);
    }
    let o = ip.octets();
    let mut praefix = [0u8; 8];
    praefix.copy_from_slice(&o[..8]);
    Some(Adressbereich::V6(praefix))
}

// ---------------------------------------------------------------------
// Das Verhalten
// ---------------------------------------------------------------------

/// Grund einer abgelehnten Verbindung: der Bereich ist voll.
#[derive(Debug)]
pub struct BereichVoll {
    /// Der Adressbereich, dessen Kontingent erschöpft ist.
    pub bereich: Adressbereich,
    /// Die überschrittene Grenze.
    pub grenze: usize,
}

impl std::fmt::Display for BereichVoll {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Adressbereich {:?} hat bereits {} eingehende Verbindungen (Grenze)",
            self.bereich, self.grenze
        )
    }
}

impl std::error::Error for BereichVoll {}

/// Begrenzt **eingehende** Verbindungen je Adressbereich.
///
/// Ausgehende Verbindungen bleiben unberührt: Sie sind die selbst
/// gewählten, und eine Schranke darauf würde genau das Budget
/// beschneiden, das die Verteidigung freihalten soll.
pub struct Adressvielfalt {
    grenze: usize,
    belegt: HashMap<Adressbereich, usize>,
    zuordnung: HashMap<ConnectionId, Adressbereich>,
}

impl Adressvielfalt {
    /// Mit der Standardgrenze [`MAX_JE_ADRESSBEREICH`].
    pub fn neu() -> Self {
        Self::mit_grenze(MAX_JE_ADRESSBEREICH)
    }

    /// Mit eigener Grenze. Für Tests, die den Mechanismus prüfen statt
    /// der Zahl: Eine kleine Grenze macht die Wirkung mit wenigen
    /// Knoten sichtbar.
    pub fn mit_grenze(grenze: usize) -> Self {
        Self {
            grenze,
            belegt: HashMap::new(),
            zuordnung: HashMap::new(),
        }
    }

    /// Belegte Plätze eines Bereichs. Für Diagnose und Tests.
    pub fn belegt(&self, bereich: &Adressbereich) -> usize {
        self.belegt.get(bereich).copied().unwrap_or(0)
    }

    /// Die Zahl der gezählten Bereiche. Für Diagnose und Tests.
    pub fn bereiche(&self) -> usize {
        self.belegt.len()
    }

    fn freigeben(&mut self, id: &ConnectionId) {
        if let Some(bereich) = self.zuordnung.remove(id) {
            if let Some(zaehler) = self.belegt.get_mut(&bereich) {
                *zaehler = zaehler.saturating_sub(1);
                if *zaehler == 0 {
                    self.belegt.remove(&bereich);
                }
            }
        }
    }
}

impl NetworkBehaviour for Adressvielfalt {
    type ConnectionHandler = dummy::ConnectionHandler;
    type ToSwarm = std::convert::Infallible;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        _peer: PeerId,
        _local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        let Some(bereich) = adressbereich(remote_addr) else {
            // Kein zählbarer Bereich (Loopback, Relay): durchlassen. Die
            // Zählgrenzen aus `standard_grenzen` gelten weiterhin.
            return Ok(dummy::ConnectionHandler);
        };
        let belegt = self.belegt(&bereich);
        if belegt >= self.grenze {
            return Err(ConnectionDenied::new(BereichVoll {
                bereich,
                grenze: self.grenze,
            }));
        }
        // Sofort zählen, nicht erst bei `ConnectionEstablished`: Sonst
        // ließe sich die Grenze umgehen, indem viele Verbindungen
        // gleichzeitig in genau diesem Fenster hängen. Freigegeben wird
        // bei `ConnectionClosed` **und** bei `ListenFailure`, damit ein
        // gescheiterter Aufbau keinen Platz dauerhaft belegt.
        self.belegt.insert(bereich, belegt + 1);
        self.zuordnung.insert(connection_id, bereich);
        Ok(dummy::ConnectionHandler)
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _addr: &Multiaddr,
        _role_override: Endpoint,
        _port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(dummy::ConnectionHandler)
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        match event {
            FromSwarm::ConnectionClosed(ConnectionClosed { connection_id, .. }) => {
                self.freigeben(&connection_id);
            }
            FromSwarm::ListenFailure(ListenFailure { connection_id, .. }) => {
                self.freigeben(&connection_id);
            }
            FromSwarm::ConnectionEstablished(ConnectionEstablished { endpoint, .. }) => {
                // Nur zur Vollständigkeit: gezählt wurde bereits in
                // `handle_established_inbound_connection`.
                let _ = matches!(endpoint, ConnectedPoint::Listener { .. });
            }
            _ => {}
        }
    }

    fn on_connection_handler_event(
        &mut self,
        _peer: PeerId,
        _id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        match event {}
    }

    fn poll(&mut self, _cx: &mut Context<'_>) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn das_ausgehende_budget_ist_von_eingehenden_nicht_erreichbar() {
        // Der Kern der Verteidigung als Rechnung: Wenn die Gesamtgrenze
        // die Summe beider Budgets ist und eingehende eigenständig
        // gedeckelt sind, bleibt für ausgehende immer Platz.
        assert_eq!(MAX_GESAMT, MAX_EINGEHEND + MAX_AUSGEHEND);
        let rest = MAX_GESAMT - MAX_EINGEHEND;
        assert_eq!(
            rest, MAX_AUSGEHEND,
            "eingehende Verbindungen dürfen kein ausgehendes Budget verbrauchen"
        );
        // Als const-Block: Wer das Budget auf null setzt, bekommt einen
        // Übersetzungsfehler statt eines roten Tests.
        const { assert!(MAX_AUSGEHEND > 0) };
    }

    #[test]
    fn das_ausgehende_budget_traegt_ein_volles_gossip_mesh() {
        // mesh_n_high der Gossipsub-Vorgabe ist 12.
        const MESH_N_HIGH: u32 = 12;
        const {
            assert!(
                MAX_AUSGEHEND >= MESH_N_HIGH,
                "mit weniger ausgehenden Verbindungen als mesh_n_high (12) \
                 kann kein volles Gossip-Mesh entstehen"
            )
        };
    }

    #[test]
    fn die_flut_braucht_zwoelf_adressbereiche() {
        let noetig = MAX_EINGEHEND as usize / MAX_JE_ADRESSBEREICH;
        assert_eq!(
            noetig, 12,
            "die Herleitung in der Modul-Doku nennt 12 Bereiche; \
             ändert sich eine der beiden Zahlen, gehört sie nachgezogen"
        );
    }

    #[test]
    fn ipv4_wird_auf_das_dritte_oktett_gekuerzt() {
        let a: Multiaddr = "/ip4/203.0.113.7/tcp/4150".parse().unwrap();
        let b: Multiaddr = "/ip4/203.0.113.200/tcp/9".parse().unwrap();
        let c: Multiaddr = "/ip4/203.0.114.7/tcp/4150".parse().unwrap();
        assert_eq!(adressbereich(&a), adressbereich(&b), "gleiches /24");
        assert_ne!(adressbereich(&a), adressbereich(&c), "anderes /24");
        assert_eq!(adressbereich(&a), Some(Adressbereich::V4([203, 0, 113])));
    }

    #[test]
    fn ipv6_wird_auf_das_vierundsechzigste_bit_gekuerzt() {
        let a: Multiaddr = "/ip6/2001:db8:1:2::1/tcp/4150".parse().unwrap();
        let b: Multiaddr = "/ip6/2001:db8:1:2:ffff:ffff:ffff:ffff/tcp/9".parse().unwrap();
        let c: Multiaddr = "/ip6/2001:db8:1:3::1/tcp/4150".parse().unwrap();
        assert_eq!(adressbereich(&a), adressbereich(&b), "gleiches /64");
        assert_ne!(adressbereich(&a), adressbereich(&c), "anderes /64");
    }

    #[test]
    fn loopback_wird_nicht_gezaehlt() {
        let v4: Multiaddr = "/ip4/127.0.0.1/tcp/4150".parse().unwrap();
        let v6: Multiaddr = "/ip6/::1/tcp/4150".parse().unwrap();
        assert_eq!(adressbereich(&v4), None);
        assert_eq!(adressbereich(&v6), None);
    }

    #[test]
    fn eine_adresse_ohne_ip_anteil_hat_keinen_bereich() {
        let dns: Multiaddr = "/dns4/example.invalid/tcp/4150".parse().unwrap();
        assert_eq!(adressbereich(&dns), None);
    }

    #[test]
    fn ipv4_in_ipv6_umgeht_die_zaehlung_nicht() {
        // Ein Angreifer könnte dieselbe IPv4-Adresse als IPv4-mapped
        // IPv6 schreiben. Beide Schreibweisen müssen denselben Bereich
        // ergeben, sonst ist die Grenze doppelt so hoch wie gedacht.
        let v4: Multiaddr = "/ip4/203.0.113.7/tcp/4150".parse().unwrap();
        let gemappt: Multiaddr = "/ip6/::ffff:203.0.113.7/tcp/4150".parse().unwrap();
        assert_eq!(adressbereich(&v4), adressbereich(&gemappt));
    }

    #[test]
    fn belegte_plaetze_werden_bei_freigabe_zurueckgegeben() {
        let mut v = Adressvielfalt::mit_grenze(2);
        let bereich = Adressbereich::V4([203, 0, 113]);
        let addr: Multiaddr = "/ip4/203.0.113.7/tcp/4150".parse().unwrap();
        let leer: Multiaddr = "/ip4/198.51.100.1/tcp/4150".parse().unwrap();
        let peer = PeerId::random();

        let id1 = ConnectionId::new_unchecked(1);
        let id2 = ConnectionId::new_unchecked(2);
        let id3 = ConnectionId::new_unchecked(3);

        assert!(v
            .handle_established_inbound_connection(id1, peer, &leer, &addr)
            .is_ok());
        assert!(v
            .handle_established_inbound_connection(id2, peer, &leer, &addr)
            .is_ok());
        assert_eq!(v.belegt(&bereich), 2);

        // Der dritte aus demselben /24 wird abgelehnt.
        assert!(v
            .handle_established_inbound_connection(id3, peer, &leer, &addr)
            .is_err());

        // Nach dem Schließen der ersten ist wieder Platz.
        v.freigeben(&id1);
        assert_eq!(v.belegt(&bereich), 1);
        assert!(v
            .handle_established_inbound_connection(id3, peer, &leer, &addr)
            .is_ok());
        assert_eq!(v.belegt(&bereich), 2);
    }

    #[test]
    fn ein_leerer_bereich_verschwindet_aus_der_tabelle() {
        // Sonst wächst die Tabelle mit jedem je gesehenen Bereich und ist
        // selbst der Angriff: unbegrenzter Speicher je Absender.
        let mut v = Adressvielfalt::mit_grenze(2);
        let addr: Multiaddr = "/ip4/203.0.113.7/tcp/4150".parse().unwrap();
        let leer: Multiaddr = "/ip4/198.51.100.1/tcp/4150".parse().unwrap();
        let id = ConnectionId::new_unchecked(1);
        let _ = v.handle_established_inbound_connection(id, PeerId::random(), &leer, &addr);
        assert_eq!(v.bereiche(), 1);
        v.freigeben(&id);
        assert_eq!(v.bereiche(), 0);
    }

    #[test]
    fn loopback_verbindungen_belegen_keinen_platz() {
        // Die lokalen Testnetze mit 20 Knoten laufen alle über
        // 127.0.0.1. Zählte man die, liefen sie gegen eine Schranke, die
        // für sie nicht gedacht ist.
        let mut v = Adressvielfalt::mit_grenze(2);
        let lokal: Multiaddr = "/ip4/127.0.0.1/tcp/4150".parse().unwrap();
        for i in 0..20usize {
            let id = ConnectionId::new_unchecked(i);
            assert!(
                v.handle_established_inbound_connection(id, PeerId::random(), &lokal, &lokal)
                    .is_ok(),
                "Loopback-Verbindung {i} wurde abgelehnt"
            );
        }
        assert_eq!(v.bereiche(), 0);
    }

    #[test]
    fn ausgehende_verbindungen_zaehlen_nicht_gegen_den_bereich() {
        // Ausgehende sind die selbst gewählten. Eine Schranke darauf
        // beschnitte genau das Budget, das die Verteidigung freihält.
        let mut v = Adressvielfalt::mit_grenze(1);
        let addr: Multiaddr = "/ip4/203.0.113.7/tcp/4150".parse().unwrap();
        for i in 0..5usize {
            assert!(v
                .handle_established_outbound_connection(
                    ConnectionId::new_unchecked(i),
                    PeerId::random(),
                    &addr,
                    Endpoint::Dialer,
                    PortUse::Reuse,
                )
                .is_ok());
        }
        assert_eq!(v.bereiche(), 0);
    }
}
