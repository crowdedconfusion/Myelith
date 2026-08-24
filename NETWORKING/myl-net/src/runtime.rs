//! Node-Event-Loop: die Laufzeit eines Myelith-Nodes (Punkt 1.4).
//!
//! `run_node` treibt den Swarm und verbindet ihn über Kanäle mit der
//! Anwendung: Publizieren per Kommando, Ereignisse (Listen-Adressen und
//! validierte Gossip-Nachrichten) per Kanal. Die Validierung (Punkt 1.4)
//! ist hier eingebaut: Eine Nachricht wird erst dann an die Anwendung
//! gemeldet und weiterverbreitet, wenn sie akzeptiert wurde.
//!
//! ## ⚑ Fund 55: Der dokumentierte Weg für die Nutzlastprüfung war
//! nicht erreichbar
//!
//! [`crate::validation::report_with`] nimmt einen
//! [`PayloadValidator`] entgegen, und die Moduldoku dort, die
//! Fahrplandatei und `README/README.md` sagen seit dem 2026-08-18
//! übereinstimmend, „die Node-Verdrahtung reicht ihn herein".
//!
//! **`run_node` hatte dafür keinen Parameter.** Es rief
//! `validation::report` auf, also die Fassung mit `AcceptAllValidator`.
//! Über die einzige öffentliche Funktion, die einen Swarm treibt, war
//! die dokumentierte Schnittstelle nicht erreichbar; wer sie nutzen
//! wollte, hätte die Ereignisschleife nachbauen müssen.
//!
//! Aufgefallen ist das nicht im Betrieb, sondern beim Schreiben der
//! Knoten-Verdrahtung. Genau das ist die Ursache: **Eine Naht, die
//! niemand belastet, hält alles aus.** `myl-net` hatte bis dahin keinen
//! einzigen Abnehmer im Repositorium.
//!
//! Behoben mit [`run_node_mit`]; [`run_node`] bleibt als bequeme
//! Fassung mit `AcceptAllValidator` und sagt das jetzt auch.

use std::sync::Arc;

use futures::StreamExt;
use libp2p::gossipsub;
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, Swarm};
use tokio::sync::{mpsc, oneshot};

use crate::gossip::GossipTopic;
use crate::node::{MylBehaviour, MylBehaviourEvent};
use crate::validation::{self, AcceptAllValidator, PayloadValidator};

/// Kommando an einen laufenden Node.
#[derive(Debug)]
pub enum NodeCommand {
    /// Roh-Nachricht (Borsh-Bytes) auf einem Topic publizieren. Über
    /// `result` wird zurückgemeldet, ob Gossipsub die Nachricht
    /// angenommen hat (`true`) — `false` heißt z. B. „noch kein Mesh".
    Publish {
        topic: GossipTopic,
        data: Vec<u8>,
        result: Option<oneshot::Sender<bool>>,
    },
    /// Anzahl der aktuell verbundenen Peers abfragen.
    PeerCount(oneshot::Sender<usize>),
    /// Eine Adresse wählen. Über `result` kommt zurück, ob der
    /// Wählversuch **begonnen** wurde; ob er gelingt, zeigt sich erst
    /// später an der Peer-Anzahl.
    ///
    /// Ein laufender Knoten braucht das für den Wiedereinstieg: Fällt
    /// die Verbindung zu den Bootstrap-Knoten weg, muss er von sich aus
    /// neu wählen können. Genau darauf beruht die Zusage aus
    /// [`crate::limits`], dass das ausgehende Budget frei bleibt: Es
    /// nützt nur, wenn jemand es auch benutzen kann.
    Dial {
        addr: Multiaddr,
        result: Option<oneshot::Sender<bool>>,
    },
    /// Auf einer weiteren Adresse horchen.
    ///
    /// Gebraucht für Relais-Reservierungen: Ein Knoten hinter NAT
    /// erfährt erst im Betrieb (über AutoNAT), dass er ein Relais
    /// braucht, und horcht dann auf `…/p2p-circuit`. Beim Start ist das
    /// noch nicht entschieden.
    Listen {
        addr: Multiaddr,
        result: Option<oneshot::Sender<bool>>,
    },
    /// Eine eigene Adresse als von außen erreichbar eintragen.
    ///
    /// Nötig für Relais-Knoten (Fund 56): Ihre Reservierungsantwort
    /// trägt die bestätigten externen Adressen, und ohne sie ist die
    /// Antwort für den Klienten wertlos. Ein gewöhnlicher Knoten
    /// überlässt das AutoNAT.
    ExterneAdresse { addr: Multiaddr },
}

/// Eine validierte, eingehende Gossip-Nachricht.
#[derive(Debug, Clone)]
pub struct InboundMessage {
    /// Das Protokoll-Topic (immer bekannt, da fremde Topics nicht
    /// abonniert werden).
    pub topic: GossipTopic,
    /// Die Roh-Nutzlast (Borsh-Bytes).
    pub data: Vec<u8>,
}

/// Ereignisse eines laufenden Nodes.
///
/// **Die Verbindungsereignisse sind für die Fehlersuche da, nicht fürs
/// Protokoll.** Sie kamen dazu, als die Knoten-Verdrahtung entstand: Ohne
/// sie ist eine abgewiesene Verbindung **stumm**, und damit wäre
/// ausgerechnet die Wirkung der Verbindungsgrenzen aus [`crate::limits`]
/// im Betrieb unsichtbar. Wer eine Schranke einbaut, muss sehen können,
/// wann sie greift, sonst ist „es kommt niemand an" von „ich lasse
/// niemanden herein" nicht zu unterscheiden.
#[derive(Debug, Clone)]
pub enum NodeEvent {
    /// Eine neue Listen-Adresse ist verfügbar (für Peer-Weitergabe).
    ListenAddr(Multiaddr),
    /// Eine validierte Gossip-Nachricht ist eingetroffen.
    Message(InboundMessage),
    /// Eine Verbindung steht.
    Verbunden {
        peer: libp2p::PeerId,
        addr: Multiaddr,
        /// Ob die Gegenstelle gewählt hat (`true`) oder wir (`false`).
        eingehend: bool,
    },
    /// Eine bestehende Verbindung ist weg.
    Getrennt {
        peer: libp2p::PeerId,
        grund: String,
    },
    /// Ein Verbindungsaufbau ist gescheitert oder wurde abgewiesen.
    ///
    /// Hier wird die Verbindungsgrenze sichtbar: Eine abgewiesene
    /// Sybil-Verbindung erscheint als Eintrag mit ihrem Grund.
    Abgewiesen {
        peer: Option<libp2p::PeerId>,
        eingehend: bool,
        grund: String,
    },
}

/// Treibt den Swarm, bis der Kommando-Kanal geschlossen wird.
///
/// Eingehende Gossip-Nachrichten werden validiert
/// ([`crate::validation::report`]): gültige werden als
/// [`NodeEvent::Message`] gemeldet und zur Weiterverbreitung
/// freigegeben, ungültige verworfen und nicht weiterverbreitet.
pub async fn run_node(
    swarm: Swarm<MylBehaviour>,
    commands: mpsc::UnboundedReceiver<NodeCommand>,
    events: mpsc::UnboundedSender<NodeEvent>,
) {
    run_node_mit(swarm, commands, events, Arc::new(AcceptAllValidator)).await
}

/// Wie [`run_node`], aber mit einer Nutzlastprüfung der Anwendung.
///
/// Das ist der Weg, den die Doku von [`crate::validation`] seit jeher
/// beschreibt und den es bis Fund 55 nicht gab. Blöcke und
/// Transaktionen kann `myl-net` nicht abschließend beurteilen: Ihre
/// Typen liegen in `myl-consensus` (L1), und die Netzschicht (L0) darf
/// nicht an die Konsensschicht hängen. Wer beide Seiten kennt, die
/// Knoten-Verdrahtung, reicht die vollständige Prüfung hier herein.
pub async fn run_node_mit(
    mut swarm: Swarm<MylBehaviour>,
    mut commands: mpsc::UnboundedReceiver<NodeCommand>,
    events: mpsc::UnboundedSender<NodeEvent>,
    validator: Arc<dyn PayloadValidator + Send + Sync>,
) {
    loop {
        tokio::select! {
            maybe_cmd = commands.recv() => {
                let Some(cmd) = maybe_cmd else {
                    // Kommando-Kanal geschlossen — Node fährt herunter.
                    return;
                };
                match cmd {
                    NodeCommand::Publish { topic, data, result } => {
                        let ok = swarm
                            .behaviour_mut()
                            .gossipsub
                            .publish(topic.topic(), data)
                            .is_ok();
                        if let Some(tx) = result {
                            let _ = tx.send(ok);
                        }
                    }
                    NodeCommand::PeerCount(reply) => {
                        let _ = reply.send(swarm.connected_peers().count());
                    }
                    NodeCommand::Dial { addr, result } => {
                        let ok = swarm.dial(addr).is_ok();
                        if let Some(tx) = result {
                            let _ = tx.send(ok);
                        }
                    }
                    NodeCommand::Listen { addr, result } => {
                        let ok = swarm.listen_on(addr).is_ok();
                        if let Some(tx) = result {
                            let _ = tx.send(ok);
                        }
                    }
                    NodeCommand::ExterneAdresse { addr } => {
                        swarm.add_external_address(addr);
                    }
                }
            }
            event = swarm.next() => {
                let Some(event) = event else { return };
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        let _ = events.send(NodeEvent::ListenAddr(address));
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        let _ = events.send(NodeEvent::Verbunden {
                            peer: peer_id,
                            addr: endpoint.get_remote_address().clone(),
                            eingehend: !endpoint.is_dialer(),
                        });
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        let _ = events.send(NodeEvent::Getrennt {
                            peer: peer_id,
                            grund: match cause {
                                Some(e) => e.to_string(),
                                None => "regulär".to_string(),
                            },
                        });
                    }
                    SwarmEvent::IncomingConnectionError { error, .. } => {
                        let _ = events.send(NodeEvent::Abgewiesen {
                            peer: None,
                            eingehend: true,
                            grund: error.to_string(),
                        });
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        let _ = events.send(NodeEvent::Abgewiesen {
                            peer: peer_id,
                            eingehend: false,
                            grund: error.to_string(),
                        });
                    }
                    SwarmEvent::Behaviour(MylBehaviourEvent::Gossipsub(
                        gossipsub::Event::Message {
                            propagation_source,
                            message_id,
                            message,
                        },
                    )) => {
                        let acceptance = validation::report_with(
                            &mut swarm,
                            &message_id,
                            &propagation_source,
                            &message.topic,
                            &message.data,
                            validator.as_ref(),
                        );
                        if matches!(acceptance, libp2p::gossipsub::MessageAcceptance::Accept) {
                            if let Some(topic) = validation::topic_from_hash(&message.topic) {
                                let _ = events.send(NodeEvent::Message(InboundMessage {
                                    topic,
                                    data: message.data,
                                }));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
