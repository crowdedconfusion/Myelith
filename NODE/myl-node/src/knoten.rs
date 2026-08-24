//! Der laufende Knoten: Start, Ereignisschleife, Protokoll.
//!
//! # Was dieser Knoten heute ist
//!
//! Ein **Netzknoten**: Er findet Gegenstellen, verbreitet und empfängt
//! die fünf Protokoll-Topics, misst Latenzen, hält seine
//! Verbindungsgrenzen ein und schreibt alles mit.
//!
//! **Er produziert keine Blöcke.** Das ist keine Auslassung, sondern der
//! Stand: Die Zustandsmaschinen in `myl-consensus` sind vollständig und
//! synchron, aber niemand treibt sie über die Zeit. Ein Knoten, der
//! Blöcke vorschlägt, braucht einen Rundentakt, einen Mempool und einen
//! Kettenzustand, und alle drei fehlen. Sie hier vorzutäuschen wäre
//! genau die Sorte Häkchen, gegen die dieses Projekt seine Regeln
//! geschrieben hat.
//!
//! Was er dagegen leistet und was vorher niemand konnte: **Er belastet
//! die Nähte.** Fund 55 und 56 sind beim Schreiben dieser Datei
//! entstanden, nicht beim Lesen.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use myl_net::{
    alle_horchadressen, bootstrap_from_config, build_swarm, eigene_adressen, ist_quic,
    ist_vermittelt, run_node_mit, subscribe_all, GossipTopic, NodeCommand, NodeEvent, NodeIdentity,
};
use tokio::sync::{mpsc, oneshot};

use crate::konfig::{KnotenKonfig, KonfigFehler};
use crate::protokoll::{Betriebsprotokoll, Eintrag, ProtokollFehler};
use crate::validator::ProtokollValidator;

/// Kurzer Fingerabdruck einer Nutzlast: die ersten 16 Hexzeichen des
/// SHA-256.
///
/// **Das ist der Faden, an dem sich zwei Protokolle zusammennähen
/// lassen.** Ohne ihn steht in Alphas Datei „gesendet, 141 Bytes" und in
/// Betas „empfangen, 141 Bytes", und niemand kann sagen, ob es dieselbe
/// Nachricht war. Mit ihm ist die Frage „kam an, was losgeschickt
/// wurde" eine Textsuche.
///
/// 16 Hexzeichen sind 64 Bit. Für die Zuordnung innerhalb eines
/// Testlaufs ist das reichlich, und es bleibt eine Länge, die jemand
/// von einem Bildschirm abliest.
pub fn nutzlast_digest(daten: &[u8]) -> String {
    myl_types::hash::Hash::sha256(daten).to_hex()[..16].to_string()
}

/// Fehler beim Start oder Betrieb eines Knotens.
#[derive(Debug)]
pub enum KnotenFehler {
    Konfig(KonfigFehler),
    Protokoll(ProtokollFehler),
    Identitaet(String),
    Netz(String),
}

impl std::fmt::Display for KnotenFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Konfig(e) => write!(f, "Konfiguration: {}", e),
            Self::Protokoll(e) => write!(f, "Betriebsprotokoll: {}", e),
            Self::Identitaet(e) => write!(f, "Identität: {}", e),
            Self::Netz(e) => write!(f, "Netz: {}", e),
        }
    }
}

impl std::error::Error for KnotenFehler {}

/// Ein laufender Knoten.
pub struct Knoten {
    konfig: KnotenKonfig,
    peer_id: libp2p::PeerId,
    kommandos: mpsc::UnboundedSender<NodeCommand>,
    ereignisse: mpsc::UnboundedReceiver<NodeEvent>,
    protokoll: Betriebsprotokoll,
    /// Zähler des Testverkehrs. Geht in die Nutzlast ein, damit jede
    /// Nachricht einen eigenen Fingerabdruck bekommt: Zwei gleiche
    /// Nutzlasten hätten denselben, und Gossipsub verwürfe die zweite
    /// als Dublette.
    testverkehr_zaehler: u64,
    /// Die eigenen Horchadressen, wie sie gemeldet wurden.
    ///
    /// Der Knoten kennt sie beim Start noch nicht: Bei Port 0 vergibt
    /// das Betriebssystem, bei einer Relais-Reservierung das Relais.
    /// Wer die Adresse weitergeben will, muss warten können.
    horchadressen: Vec<libp2p::Multiaddr>,
}

impl Knoten {
    /// Startet den Knoten: Identität, Swarm, Horchadressen, Bootstrap.
    ///
    /// Die Reihenfolge ist bedeutsam. Geprüft wird **zuerst**, denn eine
    /// widersprüchliche Konfiguration äußert sich später als Stille
    /// (Fund 56), und Stille ist das Schwerste zu debuggen.
    pub async fn starten(
        konfig: KnotenKonfig,
        auf_bildschirm: bool,
    ) -> Result<Self, KnotenFehler> {
        konfig.pruefe().map_err(KnotenFehler::Konfig)?;

        let identitaet = NodeIdentity::load_or_create(Path::new(&konfig.schluesseldatei))
            .map_err(|e| KnotenFehler::Identitaet(e.to_string()))?;
        let peer_id = identitaet.peer_id();

        let mut protokoll = Betriebsprotokoll::neu(
            &konfig.protokollverzeichnis,
            &konfig.name,
            &peer_id.to_string(),
            auf_bildschirm,
        )
        .map_err(KnotenFehler::Protokoll)?;

        protokoll.schreibe(
            Eintrag::neu("start")
                .text("version", env!("CARGO_PKG_VERSION"))
                .text("rolle", konfig.rolle.als_text())
                .zahl("horchadressen", konfig.horchadressen.len() as i64)
                .zahl("bootstrap", konfig.bootstrap.len() as i64)
                .zahl("relais", konfig.nat.relais.len() as i64)
                .text("schluesseldatei", konfig.schluesseldatei.display().to_string()),
        );

        let netz = konfig.netz();
        let mut swarm =
            build_swarm(&identitaet, &netz).map_err(|e| KnotenFehler::Netz(e.to_string()))?;
        subscribe_all(&mut swarm).map_err(|e| KnotenFehler::Netz(format!("{:?}", e)))?;

        // Eigene öffentliche Adressen eintragen. Für ein Relais Pflicht
        // (Fund 56): Sie stehen in der Reservierungsantwort.
        let nat = konfig.nat_mit_rolle();
        for addr in eigene_adressen(&nat).map_err(|e| KnotenFehler::Netz(e.to_string()))? {
            protokoll.schreibe(Eintrag::neu("eigene_adresse").text("addr", addr.to_string()));
            swarm.add_external_address(addr);
        }

        for a in &konfig.horchadressen {
            let addr: libp2p::Multiaddr = a
                .parse()
                .map_err(|_| KnotenFehler::Netz(format!("Horchadresse: {a}")))?;
            match swarm.listen_on(addr.clone()) {
                Ok(_) => protokoll
                    .schreibe(Eintrag::neu("horcht").text("addr", a).wahr("quic", ist_quic(&addr))),
                Err(e) => protokoll.schreibe(
                    Eintrag::neu("horcht_fehler").text("addr", a).text("grund", e.to_string()),
                ),
            }
        }

        // Relais-Reservierungen. Sie scheitern, wenn das Relais keine
        // eigene öffentliche Adresse führt, und das steht dann im
        // Protokoll statt in der Stille.
        for addr in alle_horchadressen(&nat).map_err(|e| KnotenFehler::Netz(e.to_string()))? {
            match swarm.listen_on(addr.clone()) {
                Ok(_) => protokoll
                    .schreibe(Eintrag::neu("relais_reservierung").text("addr", addr.to_string())),
                Err(e) => protokoll.schreibe(
                    Eintrag::neu("relais_fehler")
                        .text("addr", addr.to_string())
                        .text("grund", e.to_string()),
                ),
            }
        }

        match bootstrap_from_config(&mut swarm, &netz) {
            Ok(n) => protokoll.schreibe(Eintrag::neu("bootstrap").zahl("peers", n as i64)),
            Err(e) => protokoll
                .schreibe(Eintrag::neu("bootstrap_fehler").text("grund", e.to_string())),
        }

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (ev_tx, ev_rx) = mpsc::unbounded_channel();
        tokio::spawn(run_node_mit(swarm, cmd_rx, ev_tx, Arc::new(ProtokollValidator)));

        Ok(Self {
            konfig,
            peer_id,
            kommandos: cmd_tx,
            ereignisse: ev_rx,
            protokoll,
            testverkehr_zaehler: 0,
            horchadressen: Vec::new(),
        })
    }

    /// Die eigene Peer-Id.
    pub fn peer_id(&self) -> libp2p::PeerId {
        self.peer_id
    }

    /// Der Pfad des Betriebsprotokolls.
    pub fn protokollpfad(&self) -> &Path {
        self.protokoll.pfad()
    }

    /// Die Zahl der bisher geschriebenen Protokollzeilen.
    pub fn protokollzeilen(&self) -> u64 {
        self.protokoll.geschrieben()
    }

    /// Die bisher gemeldeten Horchadressen, mit angehängter Peer-Id,
    /// also in der Form, in der andere sie wählen können.
    pub fn adressen(&self) -> Vec<libp2p::Multiaddr> {
        self.horchadressen
            .iter()
            .filter_map(|a| a.clone().with_p2p(self.peer_id).ok())
            .collect()
    }

    /// Wartet, bis mindestens eine Horchadresse gemeldet ist.
    pub async fn warte_auf_adresse(&mut self, frist: Duration) -> Option<libp2p::Multiaddr> {
        let ende = tokio::time::Instant::now() + frist;
        while tokio::time::Instant::now() < ende {
            if let Some(a) = self.adressen().into_iter().next() {
                return Some(a);
            }
            let rest = ende.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(rest.min(Duration::from_millis(200)), self.ereignisse.recv())
                .await
            {
                Ok(Some(ev)) => self.vermerke(ev),
                Ok(None) => return None,
                Err(_) => continue,
            }
        }
        self.adressen().into_iter().next()
    }

    /// Wartet, bis mindestens `n` Peers verbunden sind.
    pub async fn warte_auf_peers(&mut self, n: usize, frist: Duration) -> usize {
        let ende = tokio::time::Instant::now() + frist;
        loop {
            let jetzt = self.peers().await;
            if jetzt >= n || tokio::time::Instant::now() >= ende {
                return jetzt;
            }
            self.laufe_fuer(Duration::from_millis(150)).await;
        }
    }

    /// Der Netzzustand: Peers, Mesh je Topic, schlecht bewertete Peers.
    pub async fn zustand(&self) -> myl_net::Netzzustand {
        let (tx, rx) = oneshot::channel();
        if self.kommandos.send(NodeCommand::Zustand(tx)).is_err() {
            return Default::default();
        }
        rx.await.unwrap_or_default()
    }

    /// Anzahl verbundener Peers.
    pub async fn peers(&self) -> usize {
        let (tx, rx) = oneshot::channel();
        if self.kommandos.send(NodeCommand::PeerCount(tx)).is_err() {
            return 0;
        }
        rx.await.unwrap_or(0)
    }

    /// Veröffentlicht eine Nutzlast und protokolliert das Ergebnis.
    pub async fn veroeffentliche(&mut self, topic: GossipTopic, daten: Vec<u8>) -> bool {
        let laenge = daten.len();
        let digest = nutzlast_digest(&daten);
        let (tx, rx) = oneshot::channel();
        if self
            .kommandos
            .send(NodeCommand::Publish { topic, data: daten, result: Some(tx) })
            .is_err()
        {
            return false;
        }
        let ok = rx.await.unwrap_or(false);
        self.protokoll.schreibe(
            Eintrag::neu("gesendet")
                .text("topic", format!("{:?}", topic))
                .text("digest", digest)
                .zahl("bytes", laenge as i64)
                .wahr("angenommen", ok),
        );
        ok
    }

    /// Verarbeitet Ereignisse, bis `dauer` abgelaufen ist.
    ///
    /// Getrennt von [`Self::laufen`], damit Tests den Knoten für eine
    /// feste Zeit fahren können, ohne auf ein Abbruchsignal zu warten.
    pub async fn laufe_fuer(&mut self, dauer: Duration) {
        let ende = tokio::time::Instant::now() + dauer;
        let takt = Duration::from_secs(self.konfig.aufnahme_sekunden.max(1));
        let mut naechste_aufnahme = tokio::time::Instant::now() + takt;
        let sendetakt = self.konfig.testverkehr_sekunden.map(|s| Duration::from_secs(s.max(1)));
        let mut naechster_versand = sendetakt.map(|t| tokio::time::Instant::now() + t);
        loop {
            let jetzt = tokio::time::Instant::now();
            if jetzt >= ende {
                return;
            }
            if jetzt >= naechste_aufnahme {
                self.aufnahme().await;
                naechste_aufnahme = jetzt + takt;
            }
            if let (Some(faellig), Some(t)) = (naechster_versand, sendetakt) {
                if jetzt >= faellig {
                    self.sende_testverkehr().await;
                    naechster_versand = Some(jetzt + t);
                }
            }
            let mut rest = ende
                .saturating_duration_since(jetzt)
                .min(naechste_aufnahme.saturating_duration_since(jetzt));
            if let Some(faellig) = naechster_versand {
                rest = rest.min(faellig.saturating_duration_since(jetzt));
            }
            let rest = rest.max(Duration::from_millis(1));
            match tokio::time::timeout(rest, self.ereignisse.recv()).await {
                Ok(Some(ev)) => self.vermerke(ev),
                Ok(None) => return,
                Err(_) => continue,
            }
        }
    }

    /// Läuft, bis das Abbruchsignal kommt.
    pub async fn laufen(&mut self) {
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    self.aufnahme().await;
                    self.protokoll.schreibe(
                        Eintrag::neu("ende").text("grund", "Abbruchsignal"),
                    );
                    return;
                }
                _ = self.laufe_fuer(Duration::from_secs(3600)) => {}
            }
        }
    }

    /// Schickt eine Nachricht des Testverkehrs.
    ///
    /// Die Nutzlast ist ein **strukturell gültiger** Block: Sie muss
    /// durch die eigene Nutzlastprüfung kommen, sonst prüfte der
    /// Testverkehr nur, dass der Validator arbeitet. Inhaltlich ist sie
    /// bedeutungslos, und der Knoten produziert damit keine Kette.
    ///
    /// Knotenname und Zähler gehen in den Zustands-Hash ein, damit jede
    /// Nachricht einen eigenen Fingerabdruck hat. Ohne das wären alle
    /// gleich, Gossipsub verwürfe sie als Dubletten, und die Auswertung
    /// könnte keine einzelne Nachricht verfolgen.
    pub async fn sende_testverkehr(&mut self) -> bool {
        use myl_consensus::block::{Block, EpochMeta};
        use myl_types::hash::Hash;

        self.testverkehr_zaehler += 1;
        let kennung = format!("{}#{}", self.konfig.name, self.testverkehr_zaehler);
        let block = Block::new(EpochMeta {
            epoch: 0,
            prev_block_hash: Hash::sha256(b"myelith-testverkehr"),
            timestamp_ms: crate::protokoll::jetzt_ms().max(0) as u64,
            state_root: Hash::sha256(kennung.as_bytes()),
        });
        let daten = match borsh::to_vec(&block) {
            Ok(d) => d,
            Err(_) => return false,
        };
        self.veroeffentliche(GossipTopic::Blocks, daten).await
    }

    /// Schreibt eine Zustandsaufnahme.
    ///
    /// Die regelmäßige Aufnahme ist der Gegenpol zu den Ereignissen:
    /// Ereignisse sagen, **was** passiert ist, die Aufnahme sagt, **wie
    /// es steht**. Ohne sie ließe sich „zwanzig Minuten kam nichts" nicht
    /// von „zwanzig Minuten lief nichts" unterscheiden.
    pub async fn aufnahme(&mut self) {
        let z = self.zustand().await;
        let mut eintrag = Eintrag::neu("aufnahme")
            .zahl("peers", z.peers as i64)
            .zahl("schlecht_bewertet", z.schlecht_bewertet as i64)
            .zahl("zeilen", self.protokoll.geschrieben() as i64);
        // Ein Feld je Topic, flach. **Verbunden heißt nicht im Mesh:**
        // Ein Knoten mit Verbindungen und leerem Mesh bekommt nur
        // Ankündigungen statt Nachrichten, und ohne diese Zahlen sähe
        // das im Protokoll aus wie ein stilles Netz.
        for (topic, groesse) in &z.mesh {
            eintrag = eintrag.zahl(
                &format!("mesh_{}", format!("{:?}", topic).to_lowercase()),
                *groesse as i64,
            );
        }
        self.protokoll.schreibe(eintrag);
    }

    fn vermerke(&mut self, ereignis: NodeEvent) {
        let eintrag = match ereignis {
            NodeEvent::ListenAddr(addr) => {
                if !self.horchadressen.contains(&addr) {
                    self.horchadressen.push(addr.clone());
                }
                Eintrag::neu("horchadresse")
                    .text("addr", addr.to_string())
                    .wahr("vermittelt", ist_vermittelt(&addr))
                    .wahr("quic", ist_quic(&addr))
            }
            NodeEvent::Message(m) => Eintrag::neu("empfangen")
                .text("topic", format!("{:?}", m.topic))
                .text("digest", nutzlast_digest(&m.data))
                .zahl("bytes", m.data.len() as i64),
            // Ohne diesen Eintrag ließe sich „nichts kam an" nicht von
            // „es kam an und wurde weggeworfen" unterscheiden, und das
            // ist die erste Frage jeder Fehlersuche.
            // Die Antwort auf „warum verbindet sich niemand zu mir".
            NodeEvent::Erreichbarkeit { addr, erreichbar, grund } => {
                Eintrag::neu("erreichbarkeit")
                    .text("addr", addr.to_string())
                    .wahr("erreichbar", erreichbar)
                    .text("grund", grund)
            }
            NodeEvent::Verworfen { topic, bytes, grund } => Eintrag::neu("verworfen")
                .text(
                    "topic",
                    topic.map(|t| format!("{:?}", t)).unwrap_or_else(|| "fremd".to_string()),
                )
                .zahl("bytes", bytes as i64)
                .text("grund", grund.als_text()),
            NodeEvent::Verbunden { peer, addr, eingehend } => Eintrag::neu("verbunden")
                .text("gegenstelle", peer.to_string())
                .text("addr", addr.to_string())
                .wahr("eingehend", eingehend)
                .wahr("vermittelt", ist_vermittelt(&addr))
                .wahr("quic", ist_quic(&addr)),
            NodeEvent::Getrennt { peer, grund } => Eintrag::neu("getrennt")
                .text("gegenstelle", peer.to_string())
                .text("grund", grund),
            // Hier wird die Verbindungsgrenze sichtbar. Ohne diesen
            // Eintrag wäre eine abgewiesene Verbindung stumm, und
            // „niemand kommt an" ließe sich nicht von „ich lasse
            // niemanden herein" unterscheiden.
            NodeEvent::Abgewiesen { peer, eingehend, grund } => Eintrag::neu("abgewiesen")
                .text("gegenstelle", peer.map(|p| p.to_string()).unwrap_or_default())
                .wahr("eingehend", eingehend)
                .text("grund", grund),
        };
        self.protokoll.schreibe(eintrag);
    }
}
