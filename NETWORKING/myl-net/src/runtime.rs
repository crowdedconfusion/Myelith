//! Node-Event-Loop: die Laufzeit eines Myelith-Nodes (Punkt 1.4).
//!
//! `run_node` treibt den Swarm und verbindet ihn über Kanäle mit der
//! Anwendung: Publizieren per Kommando, Ereignisse (Listen-Adressen und
//! validierte Gossip-Nachrichten) per Kanal. Die Validierung (Punkt 1.4)
//! ist hier eingebaut — eine Nachricht wird erst dann an die Anwendung
//! gemeldet und weiterverbreitet, wenn [`crate::validation::report`] sie
//! akzeptiert hat.

use futures::StreamExt;
use libp2p::gossipsub;
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, Swarm};
use tokio::sync::{mpsc, oneshot};

use crate::gossip::GossipTopic;
use crate::node::{MylBehaviour, MylBehaviourEvent};
use crate::validation;

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
#[derive(Debug, Clone)]
pub enum NodeEvent {
    /// Eine neue Listen-Adresse ist verfügbar (für Peer-Weitergabe).
    ListenAddr(Multiaddr),
    /// Eine validierte Gossip-Nachricht ist eingetroffen.
    Message(InboundMessage),
}

/// Treibt den Swarm, bis der Kommando-Kanal geschlossen wird.
///
/// Eingehende Gossip-Nachrichten werden validiert
/// ([`crate::validation::report`]): gültige werden als
/// [`NodeEvent::Message`] gemeldet und zur Weiterverbreitung
/// freigegeben, ungültige verworfen und nicht weiterverbreitet.
pub async fn run_node(
    mut swarm: Swarm<MylBehaviour>,
    mut commands: mpsc::UnboundedReceiver<NodeCommand>,
    events: mpsc::UnboundedSender<NodeEvent>,
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
                }
            }
            event = swarm.next() => {
                let Some(event) = event else { return };
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        let _ = events.send(NodeEvent::ListenAddr(address));
                    }
                    SwarmEvent::Behaviour(MylBehaviourEvent::Gossipsub(
                        gossipsub::Event::Message {
                            propagation_source,
                            message_id,
                            message,
                        },
                    )) => {
                        let acceptance = validation::report(
                            &mut swarm,
                            &message_id,
                            &propagation_source,
                            &message.topic,
                            &message.data,
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
