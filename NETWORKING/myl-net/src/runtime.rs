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
//! Die Planung und `README/README.md` sagen seit dem 2026-08-18
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
use libp2p::{Multiaddr, PeerId, Swarm};
use tokio::sync::{mpsc, oneshot};

use crate::gossip::GossipTopic;
use crate::node::{MylBehaviour, MylBehaviourEvent};
use crate::validation::{self, AcceptAllValidator, Ablehnungsgrund, PayloadValidator};

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
    /// Eine Zustandsaufnahme abfragen.
    ///
    /// Mehr als [`NodeCommand::PeerCount`], und der Unterschied ist der
    /// Grund: **Verbunden heißt nicht im Mesh.** Gossipsub führt je
    /// Topic eine eigene Menge von Peers, an die es Nachrichten
    /// vollständig weitergibt; wer nur verbunden ist, bekommt
    /// Ankündigungen. Ein Knoten mit Verbindungen und leerem Mesh
    /// empfängt nichts, und ohne diese Zahl sähe das im Protokoll aus
    /// wie ein stilles Netz.
    Zustand(oneshot::Sender<Netzzustand>),
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
    /// Eine direkte Anfrage an einen Peer schicken (undurchsichtige
    /// Bytes, siehe [`crate::anfrage`]).
    Anfrage {
        an: libp2p::PeerId,
        daten: Vec<u8>,
    },
    /// ⚑ **Wie [`NodeCommand::Anfrage`], mit einem Zweck** (2026-10-30):
    /// Antwort und Fehlschlag tragen ihn zurück
    /// ([`NodeEvent::AntwortEingegangen`], [`NodeEvent::AnfrageGescheitert`]).
    /// 📌 Ohne ihn wusste der Knoten nicht, worauf ein `Nichts` antwortete:
    /// Jedes `Nichts` einer gemessenen Gegenstelle zählte als abgewiesene
    /// Zeitprobe, auch eines auf eine Blocknachforderung, und ehrliche
    /// Mitglieder bekamen so Zeugnisse.
    AnfrageMitZweck {
        an: libp2p::PeerId,
        daten: Vec<u8>,
        zweck: u64,
    },
    /// Auf eine eingegangene Anfrage antworten.
    ///
    /// Die `marke` stammt aus [`NodeEvent::AnfrageEingegangen`]. Sie
    /// steht dort statt des Antwortkanals selbst, weil der weder
    /// kopierbar noch anzeigbar ist und deshalb nicht durch eine
    /// Ereignis-Aufzählung passt, die beides sein soll.
    Antwort {
        marke: u64,
        daten: Vec<u8>,
    },
    /// Eine eigene Adresse als von außen erreichbar eintragen.
    ///
    /// Nötig für Relais-Knoten (Fund 56): Ihre Reservierungsantwort
    /// trägt die bestätigten externen Adressen, und ohne sie ist die
    /// Antwort für den Klienten wertlos. Ein gewöhnlicher Knoten
    /// überlässt das AutoNAT.
    ExterneAdresse { addr: Multiaddr },
    /// ⚑ **Vorrang in den Verbindungsgrenzen** (Fund 674, 2026-10-28): Diese
    /// Gegenstellen (die Mitglieder des eigenen Pods, für einen Torknoten die
    /// Pods, die er bedient) zählen nicht gegen die Zahlengrenzen
    /// (`connection_limits` mit Ausnahmeliste). Ohne das scheiterte das
    /// Wählen zu einem Mitglied, sobald die ausgehenden Plätze voll waren,
    /// und die Zeitprobe erreichte es nicht. Ersetzt die vorige Liste;
    /// höchstens [`crate::limits::MAX_VORRANG`], der Rest ohne Vorrang.
    Vorrang { peers: Vec<PeerId> },
    /// Eine Gegenstelle sperren oder entsperren.
    ///
    /// **Eine Sperre wirkt auf die Peer-Id, nicht auf die Adresse.** Das
    /// ist der Unterschied, der zählt: `identify` und `kad` verteilen
    /// die Horchadressen weiter, also führt jede adressgebundene Trennung
    /// nur dazu, dass die Gegenstelle über einen anderen Weg
    /// wiederkommt. Eine bestehende Verbindung wird geschlossen, neue
    /// werden in beiden Richtungen abgewiesen.
    ///
    /// **Wozu:** im Betrieb, um eine als böswillig erkannte Gegenstelle
    /// loszuwerden; im Test, um eine Partition herzustellen, die sich
    /// nicht umgehen lässt.
    Sperren {
        peer: PeerId,
        /// `true` sperrt, `false` hebt die Sperre auf.
        gesperrt: bool,
        /// Rückmeldung, sobald die Sperre gesetzt ist. **Nicht
        /// überflüssig:** Ohne sie schickt ein Test die Sperre los und
        /// prüft im selben Atemzug, ob sie wirkt, und misst dann eine
        /// Warteschlange statt einer Sperre.
        result: Option<oneshot::Sender<bool>>,
    },
}

/// Der Netzzustand eines Knotens zu einem Zeitpunkt.
#[derive(Debug, Clone, Default)]
pub struct Netzzustand {
    /// Verbundene Peers.
    pub peers: usize,
    /// Mesh-Größe je Topic, in der Reihenfolge von
    /// [`crate::gossip::GossipTopic::ALLE`].
    pub mesh: Vec<(GossipTopic, usize)>,
    /// Peers unter der Gossip-Schwelle, die also kein Gossip mehr
    /// bekommen.
    pub schlecht_bewertet: usize,
    /// Dieselben Peers mit Kennung (Fund 618).
    pub schlecht_bewertete: Vec<libp2p::PeerId>,
    /// Eingegangene Anfragen, deren Antwort noch aussteht.
    ///
    /// ⚑ Eine Zahl, die nach jeder Anfrage auf null zurueckgehen muss,
    /// beantwortet oder abgelaufen (Fund 606). Waechst sie im Betrieb,
    /// haelt etwas Antwortkanaele fest.
    pub offene_anfragen: usize,
}

/// Eine validierte, eingehende Gossip-Nachricht.
#[derive(Debug, Clone)]
pub struct InboundMessage {
    /// Das Protokoll-Topic (immer bekannt, da fremde Topics nicht
    /// abonniert werden).
    pub topic: GossipTopic,
    /// Die Roh-Nutzlast (Borsh-Bytes).
    pub data: Vec<u8>,
    /// Von wem die Nachricht **weitergereicht** wurde.
    ///
    /// **Nicht der Urheber**, sondern der letzte Weiterleiter: Gossip
    /// verbreitet über Zwischenstationen, und wer eine Nachricht
    /// weitergibt, muss sie nicht erzeugt haben.
    ///
    /// Gebraucht wird das für Nachfragen: Wer merkt, dass ihm etwas
    /// fehlt, muss **jemanden** fragen können, und der nächstliegende
    /// ist der, von dem der Hinweis kam. Ohne dieses Feld war eine
    /// Nachforderung nicht adressierbar.
    pub von: libp2p::PeerId,
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
    /// Eine direkte Anfrage ist eingegangen.
    ///
    /// Wird sie nicht mit [`NodeCommand::Antwort`] beantwortet, läuft
    /// sie beim Fragenden in eine Zeitüberschreitung. Das ist kein
    /// Fehler, sondern die Vorgabe: Niemand muss antworten.
    AnfrageEingegangen {
        von: libp2p::PeerId,
        daten: Vec<u8>,
        marke: u64,
    },
    /// Eine Antwort auf eine eigene Anfrage ist eingegangen.
    AntwortEingegangen {
        von: libp2p::PeerId,
        daten: Vec<u8>,
        /// Der Zweck aus [`NodeCommand::AnfrageMitZweck`], sonst `None`.
        zweck: Option<u64>,
    },
    /// Eine eigene Anfrage ist gescheitert.
    AnfrageGescheitert {
        an: libp2p::PeerId,
        grund: String,
        /// Der Zweck aus [`NodeCommand::AnfrageMitZweck`], sonst `None`.
        zweck: Option<u64>,
    },
    /// Ein Lochstanzversuch (DCUtR) ist abgeschlossen.
    ///
    /// **Das Messgerät für die teuerste offene Frage der Netzschicht.**
    /// Zwei Knoten hinter NAT sprechen zunächst über ein Relais; DCUtR
    /// versucht danach, eine direkte Verbindung herzustellen. Gelingt
    /// das, ist das Relais wieder frei und sein Hebel weg.
    ///
    /// Ob es gelingt, hängt an der Bauart der beteiligten NATs und ist
    /// **auf einer Maschine nicht messbar**: Auf Loopback gibt es
    /// nichts zu durchstoßen. Ohne dieses Ereignis bliebe die Frage auch
    /// auf getrennten Maschinen unbeantwortet, weil niemand mitschreibt,
    /// wie oft es klappt.
    Lochstanzen {
        peer: libp2p::PeerId,
        gelungen: bool,
        grund: String,
    },
    /// Eine Paarlatenz wurde gemessen (Ping).
    ///
    /// In **Mikrosekunden als Ganzzahl**, wie die Atteste sie tragen.
    /// Der `LatencyTracker` glättet daraus; hier steht der Rohwert, denn
    /// für die Fehlersuche ist die Streuung interessanter als der
    /// geglättete Verlauf.
    Latenz {
        peer: libp2p::PeerId,
        mikrosekunden: u64,
    },
    /// AutoNAT hat die eigene Erreichbarkeit geprüft.
    ///
    /// Ohne diese Meldung stellt der Knoten fest, ob er von außen
    /// erreichbar ist, und sagt es niemandem. Für die Fehlersuche ist
    /// das die Antwort auf „warum verbindet sich niemand zu mir".
    Erreichbarkeit {
        addr: Multiaddr,
        erreichbar: bool,
        grund: String,
    },
    /// Eine eingehende Nachricht wurde verworfen und **nicht**
    /// weiterverbreitet.
    ///
    /// Ohne dieses Ereignis ist eine verworfene Nachricht stumm: Der
    /// Knoten wüsste selbst nicht, dass er etwas weggeworfen hat, und
    /// im Betriebsprotokoll ließe sich „nichts kam an" nicht von
    /// „alles kam an und wurde verworfen" unterscheiden. Genau diese
    /// Unterscheidung braucht jede Fehlersuche zuerst.
    Verworfen {
        /// Das Topic, falls es überhaupt eines des Protokolls war.
        topic: Option<GossipTopic>,
        bytes: usize,
        grund: Ablehnungsgrund,
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

/// Wie oft ein Knoten Kademlia nach weiteren Knoten suchen lässt (Fund 587),
/// sobald er eingelaufen ist.
///
/// ⚑ **Eine halbe Minute**: oft genug, dass ein neuer Knoten binnen einer
/// Minute bei allen bekannt ist, selten genug, dass die Suche gegen den
/// Blockverkehr nicht ins Gewicht fällt.
pub const KAD_TAKT: std::time::Duration = std::time::Duration::from_secs(30);

/// Der Abstand nach der ersten Suche; er verdoppelt sich bis [`KAD_TAKT`].
///
/// 📌 **Warum nicht gleich der Takt** (Fund 587, zweiter Teil): Die erste
/// Suche läuft, sobald die Bootstrap-Gegenstelle sich ausweist, und die
/// kennt in diesem Augenblick oft nur sich selbst. Wer nach ihr kam, war
/// dann erst eine halbe Minute später zu finden; in der Probe mit drei
/// Knoten fanden sich A und C in 20 s so einmal nicht.
pub const KAD_ERSTER_ABSTAND: std::time::Duration = std::time::Duration::from_secs(1);

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
    // Offene Antwortkanäle. Sie bleiben hier, weil ein
    // `ResponseChannel` weder kopierbar noch anzeigbar ist; nach außen
    // geht nur eine Marke.
    let mut offene_anfragen: std::collections::HashMap<
        u64,
        libp2p::request_response::ResponseChannel<Vec<u8>>,
    > = std::collections::HashMap::new();
    // 📌 **Fund 606 (2026-10-23):** Ein Antwortkanal verliess die Tabelle
    //    nur, wenn der Knoten antwortete. Lief die Anfrage vorher ab, schloss
    //    die Gegenstelle die Verbindung oder liess sich die Antwort nicht
    //    schreiben, blieb er fuer immer liegen; eine Gegenstelle, die
    //    Anfragen schickt und nicht wartet, liess die Tabelle so ohne Ende
    //    wachsen. libp2p meldet das Ende **jeder** eingegangenen Anfrage,
    //    als `ResponseSent` oder `InboundFailure`; an beidem wird jetzt
    //    aufgeraeumt, ueber diese Zuordnung von Anfrage zu Marke.
    let mut marke_je_anfrage: std::collections::HashMap<
        libp2p::request_response::InboundRequestId,
        u64,
    > = std::collections::HashMap::new();
    let mut naechste_marke: u64 = 0;
    // Der Zweck je eigener Anfrage, bis Antwort oder Fehlschlag sie
    // beendet; libp2p meldet für **jede** ausgehende eines von beiden, die
    // Tabelle hält also nur, was unterwegs ist.
    let mut zweck_je_anfrage: std::collections::HashMap<libp2p::request_response::OutboundRequestId, u64> =
        std::collections::HashMap::new();
    // 📌 **Fund 587 (2026-10-22, gefunden im Netzlauf): Kademlia wurde nie
    //    angestoßen.** `start_bootstrap` hatte keinen Aufrufer, und die
    //    Horchadressen aus `identify` kamen nicht in die Routing-Tabelle.
    //    Jeder Knoten kannte damit nur seine Bootstrap-Gegenstelle; eine
    //    Anfrage an ein anderes Mitglied seines Pods (Zeitprobe, Datei,
    //    Stichprobe) fand keinen Weg. Gossip trug weiter, über den Stern.
    //    Jetzt: eine Suche, sobald sich der erste Kademlia-Knoten ausweist,
    //    danach in Abständen von [`KAD_ERSTER_ABSTAND`], verdoppelt bis
    //    [`KAD_TAKT`]; und jede gemeldete Horchadresse eines
    //    Kademlia-Knotens geht in die Tabelle.
    let mut kad_abstand = KAD_ERSTER_ABSTAND;
    let kad_naechste = tokio::time::sleep(KAD_TAKT);
    tokio::pin!(kad_naechste);
    let mut kad_angestossen = false;
    let mut vorrang: std::collections::HashSet<PeerId> = std::collections::HashSet::new();
    loop {
        tokio::select! {
            _ = &mut kad_naechste => {
                let _ = swarm.behaviour_mut().kad.bootstrap();
                kad_naechste.as_mut().reset(tokio::time::Instant::now() + kad_abstand);
                kad_abstand = (kad_abstand * 2).min(KAD_TAKT);
            }
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
                    NodeCommand::Zustand(reply) => {
                        let peers = swarm.connected_peers().count();
                        let gossipsub = &swarm.behaviour().gossipsub;
                        let mesh = GossipTopic::ALLE
                            .iter()
                            .map(|t| (*t, gossipsub.mesh_peers(&t.topic().hash()).count()))
                            .collect();
                        let schlecht_bewertete = crate::scoring::schlecht_bewertete(gossipsub);
                        let _ = reply.send(Netzzustand {
                            peers,
                            mesh,
                            schlecht_bewertet: schlecht_bewertete.len(),
                            schlecht_bewertete,
                            offene_anfragen: offene_anfragen.len(),
                        });
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
                    NodeCommand::Vorrang { peers } => {
                        let neu: std::collections::HashSet<PeerId> =
                            peers.into_iter().take(crate::limits::MAX_VORRANG).collect();
                        for alt in vorrang.difference(&neu) {
                            swarm.behaviour_mut().grenzen.remove_peer_id(alt);
                        }
                        for p in neu.difference(&vorrang) {
                            swarm.behaviour_mut().grenzen.bypass_peer_id(p);
                        }
                        vorrang = neu;
                    }
                    NodeCommand::Sperren {
                        peer,
                        gesperrt,
                        result,
                    } => {
                        let sperre = &mut swarm.behaviour_mut().sperrliste;
                        if gesperrt {
                            sperre.block_peer(peer);
                        } else {
                            sperre.unblock_peer(peer);
                        }
                        if let Some(tx) = result {
                            let _ = tx.send(true);
                        }
                    }
                    NodeCommand::Anfrage { an, daten } => {
                        swarm.behaviour_mut().anfrage.send_request(&an, daten);
                    }
                    NodeCommand::AnfrageMitZweck { an, daten, zweck } => {
                        let id = swarm.behaviour_mut().anfrage.send_request(&an, daten);
                        zweck_je_anfrage.insert(id, zweck);
                    }
                    NodeCommand::Antwort { marke, daten } => {
                        if let Some(kanal) = offene_anfragen.remove(&marke) {
                            let _ = swarm
                                .behaviour_mut()
                                .anfrage
                                .send_response(kanal, daten);
                        }
                    }
                }
            }
            event = swarm.next() => {
                let Some(event) = event else { return };
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        let _ = events.send(NodeEvent::ListenAddr(address));
                    }
                    SwarmEvent::Behaviour(MylBehaviourEvent::Anfrage(ev)) => {
                        use libp2p::request_response::{Event as RrEvent, Message as RrMsg};
                        match ev {
                            RrEvent::Message { peer, message, .. } => match message {
                                RrMsg::Request { request_id, request, channel } => {
                                    naechste_marke += 1;
                                    let marke = naechste_marke;
                                    offene_anfragen.insert(marke, channel);
                                    marke_je_anfrage.insert(request_id, marke);
                                    let _ = events.send(NodeEvent::AnfrageEingegangen {
                                        von: peer,
                                        daten: request,
                                        marke,
                                    });
                                }
                                RrMsg::Response { request_id, response } => {
                                    let _ = events.send(NodeEvent::AntwortEingegangen {
                                        von: peer,
                                        daten: response,
                                        zweck: zweck_je_anfrage.remove(&request_id),
                                    });
                                }
                            },
                            RrEvent::OutboundFailure { peer, request_id, error, .. } => {
                                let _ = events.send(NodeEvent::AnfrageGescheitert {
                                    an: peer,
                                    grund: error.to_string(),
                                    zweck: zweck_je_anfrage.remove(&request_id),
                                });
                            }
                            RrEvent::ResponseSent { request_id, .. }
                            | RrEvent::InboundFailure { request_id, .. } => {
                                if let Some(marke) = marke_je_anfrage.remove(&request_id) {
                                    offene_anfragen.remove(&marke);
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(MylBehaviourEvent::Dcutr(ev)) => {
                        let _ = events.send(NodeEvent::Lochstanzen {
                            peer: ev.remote_peer_id,
                            gelungen: ev.result.is_ok(),
                            grund: match ev.result {
                                Ok(_) => "direkt".to_string(),
                                Err(e) => e.to_string(),
                            },
                        });
                    }
                    SwarmEvent::Behaviour(MylBehaviourEvent::Ping(ev)) => {
                        if let Ok(dauer) = ev.result {
                            let _ = events.send(NodeEvent::Latenz {
                                peer: ev.peer,
                                mikrosekunden: dauer.as_micros().min(u64::MAX as u128) as u64,
                            });
                        }
                    }
                    SwarmEvent::Behaviour(MylBehaviourEvent::AutonatClient(ev)) => {
                        let _ = events.send(NodeEvent::Erreichbarkeit {
                            addr: ev.tested_addr,
                            erreichbar: ev.result.is_ok(),
                            grund: match ev.result {
                                Ok(()) => "bestätigt".to_string(),
                                Err(e) => e.to_string(),
                            },
                        });
                    }
                    // ⚑ **Im lokalen Netz gefunden** (mDNS): in die Tabelle
                    //   und anwaehlen; `identify` und Kademlia tun den Rest
                    //   wie bei jeder anderen Gegenstelle.
                    //   📌 Je Gegenstelle gesammelt und gefiltert (Fund 594):
                    //   mDNS nennt auch die Rueckschleife des anderen.
                    SwarmEvent::Behaviour(MylBehaviourEvent::Mdns(libp2p::mdns::Event::Discovered(gefunden))) => {
                        let mut je_peer: std::collections::BTreeMap<PeerId, Vec<Multiaddr>> = Default::default();
                        for (peer, addr) in gefunden {
                            if peer != *swarm.local_peer_id() {
                                je_peer.entry(peer).or_default().push(addr);
                            }
                        }
                        for (peer, adressen) in je_peer {
                            let adressen = crate::discovery::fuer_das_adressbuch(adressen);
                            if adressen.is_empty() {
                                continue;
                            }
                            for addr in &adressen {
                                swarm.behaviour_mut().kad.add_address(&peer, addr.clone());
                            }
                            let wahl = libp2p::swarm::dial_opts::DialOpts::peer_id(peer)
                                .addresses(adressen)
                                .condition(libp2p::swarm::dial_opts::PeerCondition::DisconnectedAndNotDialing)
                                .build();
                            let _ = swarm.dial(wahl);
                        }
                    }
                    SwarmEvent::Behaviour(MylBehaviourEvent::Identify(
                        libp2p::identify::Event::Received { peer_id, info, .. },
                    )) => {
                        let kad_protokoll = libp2p::StreamProtocol::new(crate::discovery::KAD_PROTOCOL);
                        if info.protocols.contains(&kad_protokoll) {
                            // 📌 Fund 594: nicht die Rueckschleife eines anderen.
                            for addr in crate::discovery::fuer_das_adressbuch(info.listen_addrs) {
                                swarm.behaviour_mut().kad.add_address(&peer_id, addr);
                            }
                            if !kad_angestossen {
                                kad_angestossen = swarm.behaviour_mut().kad.bootstrap().is_ok();
                                kad_naechste.as_mut().reset(tokio::time::Instant::now() + kad_abstand);
                            }
                        } else {
                            // ⚑ **Kein Myelith-Knoten** (2026-10-22): Jeder
                            //   Myelith-Knoten bietet sein Kademlia an. mDNS
                            //   findet im lokalen Netz aber jedes libp2p-
                            //   Programm (derselbe Dienstname), und ein fremdes
                            //   belegte sonst einen Verbindungsplatz.
                            let _ = swarm.disconnect_peer_id(peer_id);
                        }
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
                        let urteil = validation::beurteile(
                            &message.topic,
                            &message.data,
                            validator.as_ref(),
                        );
                        let _ = validation::report_with(
                            &mut swarm,
                            &message_id,
                            &propagation_source,
                            &message.topic,
                            &message.data,
                            validator.as_ref(),
                        );
                        match urteil {
                            Ok(topic) => {
                                let _ = events.send(NodeEvent::Message(InboundMessage {
                                    topic,
                                    data: message.data,
                                    von: propagation_source,
                                }));
                            }
                            Err(grund) => {
                                let _ = events.send(NodeEvent::Verworfen {
                                    topic: validation::topic_from_hash(&message.topic),
                                    bytes: message.data.len(),
                                    grund,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
