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

use crate::kette::Kette;
use crate::nachschub::{Nachforderung, Nachlieferung};
use crate::probe::Probe;
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
/// Wie lange nach der letzten neuen Horchadresse noch gewartet wird.
///
/// TCP horcht schneller als QUIC. Wer bei der ersten Adresse aufhört,
/// sieht nur die TCP-Adresse, und genau das ist passiert. Eine halbe
/// Sekunde reicht auf jeder geprüften Maschine und fällt beim Start
/// nicht auf.
pub const RUHE_NACH_ERSTER_ADRESSE: Duration = Duration::from_millis(500);

/// Kurzform eines Hashes fürs Protokoll: 16 Hexzeichen.
pub fn kurz(h: &myl_types::hash::Hash) -> String {
    h.to_hex()[..16].to_string()
}

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
    /// Die eigene Kette: Zustand, Höhe, Mempool.
    ///
    /// **Jeder Knoten führt seine eigene.** Der Erzeuger baut, die
    /// übrigen rechnen nach. Ob am Ende alle bei derselben
    /// Zustandswurzel stehen, ist die Aussage des Laufs.
    kette: Kette,
    /// Kleinste, größte und Zahl der Latenzmessungen seit der letzten
    /// Zustandsaufnahme.
    ///
    /// **Gesammelt statt einzeln protokolliert.** Ein Ping je Peer alle
    /// 15 Sekunden ergäbe über eine Stunde bei drei Peers 720 Zeilen,
    /// die einzeln nichts sagen. Interessant ist die Spanne: Ein
    /// Höchstwert weit über dem Kleinstwert heißt Schwankung, und die
    /// erklärt mehr als jeder Einzelwert.
    latenz: (u64, u64, u64),
    /// Zähler des Testverkehrs. Geht in die Nutzlast ein, damit jede
    /// Nachricht einen eigenen Fingerabdruck bekommt: Zwei gleiche
    /// Nutzlasten hätten denselben, und Gossipsub verwürfe die zweite
    /// als Dublette.
    testverkehr_zaehler: u64,
    /// Zuletzt gemessene Laufzeit je Peer, in Millisekunden.
    ///
    /// Grundlage des eigenen Latenz-Attests (A10). Getrennt von der
    /// Spanne in [`Self::latenz`]: Die ist eine Kennzahl fürs
    /// Protokoll, das hier ist der Inhalt, den andere weiterverwenden.
    latenz_je_peer: std::collections::BTreeMap<libp2p::PeerId, u32>,
    /// Ob gerade eine Nachforderung unterwegs ist.
    ///
    /// **Eine zur Zeit.** Ohne diese Sperre schickt ein Neuling für
    /// jeden abgelehnten Block eine neue Anfrage; bei einem Rückstand
    /// von zwanzig Blöcken wären das zwanzig Anfragen für dieselbe
    /// Lücke, und der Gegenüber bezahlt sie alle.
    nachforderung_laeuft: bool,
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
        // ⚑ A10: Der Validatorsatz kommt aus der Teilnehmerliste. Ohne
        // sie ist er leer, und dann wird jedes Attest abgewiesen. Das
        // ist der sichere Vorgabefall.
        let validatoren = crate::validatorsatz::Validatorsatz::aus_namen(&konfig.teilnehmer);
        protokoll.schreibe(
            Eintrag::neu("validatorsatz")
                .zahl("bekannte_aussteller", validatoren.anzahl() as i64)
                .wahr("atteste_pruefbar", validatoren.anzahl() > 0),
        );
        tokio::spawn(run_node_mit(
            swarm,
            cmd_rx,
            ev_tx,
            Arc::new(ProtokollValidator::mit(validatoren)),
        ));

        Ok(Self {
            konfig,
            peer_id,
            kommandos: cmd_tx,
            ereignisse: ev_rx,
            protokoll,
            testverkehr_zaehler: 0,
            kette: Kette::probestand(),
            nachforderung_laeuft: false,
            latenz_je_peer: std::collections::BTreeMap::new(),
            latenz: (u64::MAX, 0, 0),
            horchadressen: Vec::new(),
        })
    }

    /// Die eigene Kette, für Tests und Diagnose.
    pub fn kette(&self) -> &Kette {
        &self.kette
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
    /// ⚑ **Wartet, bis die Adressen sich beruhigt haben, nicht bis die
    /// erste da ist.**
    ///
    /// Die erste Fassung kehrte zurück, sobald irgendeine Adresse
    /// vorlag. Das war die TCP-Adresse, weil TCP schneller horcht als
    /// QUIC, und die QUIC-Adresse traf Millisekunden später ein: **also
    /// nach der Rückkehr.** Der Betreiber bekam nur die TCP-Adresse zu
    /// sehen, konnte also nur diese weitergeben, und damit lief das
    /// ganze Netz über TCP.
    ///
    /// Das ist teuer, weil der Durchstich durch Heimrouter über UDP
    /// deutlich zuverlässiger gelingt: Der Rat „die quic-v1-Adresse
    /// weitergeben" stand in der Anleitung und war **unbefolgbar**, weil
    /// sie gar nicht angezeigt wurde.
    ///
    /// Deshalb sammelt diese Fassung weiter, bis
    /// [`RUHE_NACH_ERSTER_ADRESSE`] lang keine neue mehr kommt.
    pub async fn warte_auf_adresse(&mut self, frist: Duration) -> Option<libp2p::Multiaddr> {
        let ende = tokio::time::Instant::now() + frist;
        let mut seit_letzter: Option<tokio::time::Instant> = None;
        while tokio::time::Instant::now() < ende {
            let hat_welche = !self.horchadressen.is_empty();
            if let Some(zeitpunkt) = seit_letzter {
                if hat_welche && zeitpunkt.elapsed() >= RUHE_NACH_ERSTER_ADRESSE {
                    break;
                }
            }
            let rest = ende.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(rest.min(Duration::from_millis(100)), self.ereignisse.recv())
                .await
            {
                Ok(Some(ev)) => {
                    let war = self.horchadressen.len();
                    self.vermerke(ev);
                    if self.horchadressen.len() > war {
                        seit_letzter = Some(tokio::time::Instant::now());
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    if hat_welche && seit_letzter.is_none() {
                        seit_letzter = Some(tokio::time::Instant::now());
                    }
                    continue;
                }
            }
        }
        self.adressen().into_iter().next()
    }

    /// Wartet, bis eine Adresse **des gewünschten Transports** vorliegt,
    /// oder die Frist abläuft.
    ///
    /// Für den Betreiber der Anlaufstelle: Er soll die quic-v1-Adresse
    /// weitergeben, also muss sie angezeigt werden.
    pub async fn warte_auf_quic(&mut self, frist: Duration) -> bool {
        let ende = tokio::time::Instant::now() + frist;
        while tokio::time::Instant::now() < ende {
            if self.adressen().iter().any(myl_net::ist_quic) {
                return true;
            }
            let rest = ende.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(rest.min(Duration::from_millis(100)), self.ereignisse.recv())
                .await
            {
                Ok(Some(ev)) => self.vermerke(ev),
                Ok(None) => return false,
                Err(_) => continue,
            }
        }
        self.adressen().iter().any(myl_net::ist_quic)
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

    /// Läuft, bis die Zeit um ist oder das Abbruchsignal kommt.
    ///
    /// ⚑ **Beide Wege schreiben einen Abschlusseintrag.** In der ersten
    /// Fassung behandelte nur der Weg ohne Laufzeitgrenze das
    /// Abbruchsignal; mit `--laufzeit` starb der Prozess bei Strg-C
    /// wortlos. Das Protokoll blieb zwar vollständig, weil jede Zeile
    /// sofort geschrieben wird, aber es endete mitten im Betrieb, und
    /// **„absichtlich beendet" ließ sich nicht von „abgestürzt"
    /// unterscheiden.**
    ///
    /// Für einen Lauf über mehrere Maschinen ist genau das die Frage,
    /// die als Erstes gestellt wird, wenn ein Protokoll kürzer ist als
    /// die anderen.
    pub async fn laufen_bis(&mut self, dauer: Option<Duration>) {
        let grund = tokio::select! {
            _ = tokio::signal::ctrl_c() => "Abbruchsignal",
            _ = async {
                match dauer {
                    Some(d) => self.laufe_fuer(d).await,
                    // Ohne Grenze: in Abschnitten, damit die
                    // Zustandsaufnahmen weiterlaufen.
                    None => loop { self.laufe_fuer(Duration::from_secs(3600)).await },
                }
            } => "Laufzeit abgelaufen",
        };
        self.aufnahme().await;
        self.protokoll.schreibe(
            Eintrag::neu("ende")
                .text("grund", grund)
                .zahl("hoehe", self.kette.hoehe() as i64)
                .zahl("zeilen", self.protokoll.geschrieben() as i64),
        );
    }

    /// Läuft, bis das Abbruchsignal kommt.
    pub async fn laufen(&mut self) {
        self.laufen_bis(None).await
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
        self.testverkehr_zaehler += 1;

        // Die Rückgratprobe jedes Takts: Der Erzeuger baut einen Block,
        // die übrigen schicken eine Transaktion. Ohne beides stünde die
        // Kette still.
        let rueckgrat = if self.konfig.erzeugt_bloecke {
            // ⚑ **Erst bauen, wenn jemand zuhört.**
            //
            // Es gibt keinen Nachholmechanismus: Ein Knoten, der Block 1
            // verpasst, hängt für den Rest des Laufs fest, weil jeder
            // folgende Block auf einen Vorgänger zeigt, den er nie
            // gesehen hat. Baut der Erzeuger los, bevor die anderen
            // verbunden sind, lehnen sie danach **alles** ab.
            //
            // Der erste Probelauf mit drei Knoten lief genau so ins
            // Leere: Alpha baute acht Blöcke, Beta und Gamma wiesen alle
            // acht mit „passt nicht an" zurück und blieben auf Höhe 0.
            //
            // Das Warten behebt den Anlass, **nicht die Ursache**. Wer
            // mitten im Lauf dazukommt, hängt weiterhin fest. Eine
            // Blocksynchronisierung fehlt und gehört vor ein echtes
            // Testnetz.
            if self.kette.hoehe() == 0 && self.peers().await == 0 {
                self.protokoll.schreibe(
                    Eintrag::neu("erzeugung_wartet")
                        .text("grund", "noch kein Peer verbunden, erster Block würde niemanden erreichen"),
                );
                return false;
            }
            let ok = self.erzeuge_block().await;
            self.vermerke_probe(Probe::Blockkette, ok);
            ok
        } else {
            let ok = self.sende_transaktion().await;
            self.vermerke_probe(Probe::Transaktion, ok);
            ok
        };

        // Und eine wechselnde Probe daneben. **Ohne Wechsel liefe immer
        // dieselbe**, und die übrigen Funktionen blieben ungeprüft, ohne
        // dass es jemandem auffiele: Der Lauf sähe grün aus.
        let wechselnd = match self.testverkehr_zaehler % 3 {
            0 => Probe::PoiBuendel,
            1 => Probe::Challenge,
            _ => Probe::Latenzattest,
        };
        let ok2 = self.fuehre_probe(wechselnd).await;

        rueckgrat && ok2
    }

    /// Führt eine Nachrichtenprobe aus: echtes Objekt bauen,
    /// serialisieren, ins Netz geben, Urteil vermerken.
    pub async fn fuehre_probe(&mut self, probe: Probe) -> bool {
        let folge = self.testverkehr_zaehler;
        let name = self.konfig.name.clone();
        let (topic, daten) = match probe {
            Probe::PoiBuendel => {
                let Some(b) = crate::probe::probe_poi_buendel(&name, folge) else {
                    self.vermerke_probe(probe, false);
                    return false;
                };
                (GossipTopic::PoiBundles, borsh::to_vec(&b).ok())
            }
            Probe::Challenge => {
                let c = crate::probe::probe_challenge(&name, folge);
                (GossipTopic::Challenges, borsh::to_vec(&c).ok())
            }
            Probe::Latenzattest => {
                // Die tatsächlich gemessenen Werte, nicht erfundene: Ein
                // Attest mit ausgedachten Zahlen prüfte nur die
                // Signatur, nicht den Weg, den ein echtes nimmt.
                let latenzen: Vec<(libp2p::PeerId, u32)> = self
                    .latenz_je_peer
                    .iter()
                    .map(|(p, ms)| (*p, *ms))
                    .collect();
                let Some(a) = crate::probe::probe_attest(&name, &latenzen) else {
                    self.vermerke_probe(probe, false);
                    return false;
                };
                (GossipTopic::LatencyAttests, borsh::to_vec(&a).ok())
            }
            // Die übrigen ergeben sich aus dem Verhalten, nicht aus
            // einer eigenen Nachricht.
            _ => return true,
        };
        let Some(daten) = daten else {
            self.vermerke_probe(probe, false);
            return false;
        };
        let ok = self.veroeffentliche(topic, daten).await;
        self.vermerke_probe(probe, ok);
        ok
    }

    /// Schreibt das Urteil einer Probe ins Betriebsprotokoll.
    ///
    /// **Eine eigene Eintragsart**, nicht bloß ein Feld an `gesendet`:
    /// Die Auswertung zählt danach zusammen, welche Funktion wie oft
    /// ausprobiert wurde. Eine Probe, die nie lief, ist kein Erfolg,
    /// und das ließe sich sonst nicht von einer bestandenen
    /// unterscheiden.
    fn vermerke_probe(&mut self, probe: Probe, gelungen: bool) {
        self.protokoll.schreibe(
            Eintrag::neu("probe")
                .text("kennung", probe.kennung())
                .wahr("gelungen", gelungen),
        );
    }

    /// Baut den nächsten Block und verbreitet ihn.
    ///
    /// Nur der Erzeuger tut das. Er übernimmt den Block **selbst
    /// zuerst**, das steckt in [`Kette::baue_block`]: Der Zustand wird
    /// angewandt, bevor die Wurzel in den Block geschrieben wird.
    pub async fn erzeuge_block(&mut self) -> bool {
        let wartend = self.kette.wartend();
        let block = self.kette.baue_block();
        let daten = match borsh::to_vec(&block) {
            Ok(d) => d,
            Err(_) => return false,
        };
        let bytes = daten.len();
        let ok = self.veroeffentliche(GossipTopic::Blocks, daten).await;
        self.protokoll.schreibe(
            Eintrag::neu("block_erzeugt")
                .zahl("hoehe", self.kette.hoehe() as i64)
                .zahl("txs", wartend as i64)
                .zahl("bytes", bytes as i64)
                .text("zustandswurzel", kurz(&self.kette.zustandswurzel()))
                .text("block", kurz(&self.kette.letzter_hash()))
                .wahr("verbreitet", ok),
        );
        ok
    }

    /// Schickt eine Transaktion ins Netz.
    ///
    /// Die Nicht-Erzeuger tun das. Ohne Transaktionen wären alle Blöcke
    /// leer, und dann sagte die Übereinstimmung der Zustandswurzeln
    /// nichts: Ein leerer Zustand ist überall gleich.
    pub async fn sende_transaktion(&mut self) -> bool {
        use myl_consensus::block::{BurnTx, Transaction};

        // Ein **ausgestattetes** Testkonto, über den Knotennamen
        // gewählt. Ein beliebiger Absender hätte kein Guthaben, der
        // Burn scheiterte still, und der Zustand bewegte sich nie:
        // Dann belegte die Übereinstimmung der Wurzeln nichts.
        let tx = Transaction::Burn(BurnTx {
            sender: crate::kette::konto_fuer(&self.konfig.name),
            amount: 1_000 + self.testverkehr_zaehler * 100,
        });
        let daten = match borsh::to_vec(&tx) {
            Ok(d) => d,
            Err(_) => return false,
        };
        self.veroeffentliche(GossipTopic::Transactions, daten).await
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
            .zahl("hoehe", self.kette.hoehe() as i64)
            .text("zustandswurzel", kurz(&self.kette.zustandswurzel()))
            .zahl("wartend", self.kette.wartend() as i64)
            .zahl("schlecht_bewertet", z.schlecht_bewertet as i64)
            .zahl("zeilen", self.protokoll.geschrieben() as i64);
        let (kleinste, groesste, anzahl) = self.latenz;
        eintrag = eintrag.zahl("latenz_messungen", anzahl as i64);
        if anzahl > 0 {
            eintrag = eintrag
                .zahl("latenz_min_us", kleinste as i64)
                .zahl("latenz_max_us", groesste as i64);
        }
        // Zurücksetzen: Jede Aufnahme beschreibt das Fenster seit der
        // vorigen. Sonst glättete sich jede Schwankung über den ganzen
        // Lauf weg, und genau die Schwankung ist die Auskunft.
        self.latenz = (u64::MAX, 0, 0);
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

    /// Fordert die fehlenden Blöcke bei einem Peer nach.
    ///
    /// Tut nichts, wenn nichts fehlt oder bereits eine Anfrage läuft.
    fn fordere_nach(&mut self, von: libp2p::PeerId, fremde_hoehe: u64) {
        if self.nachforderung_laeuft {
            return;
        }
        let Some(forderung) = Nachforderung::fuer_rueckstand(self.kette.hoehe(), fremde_hoehe)
        else {
            return;
        };
        let Some(bytes) = forderung.als_bytes() else {
            return;
        };
        let Nachforderung::Bloecke { ab, bis } = forderung;
        if self
            .kommandos
            .send(NodeCommand::Anfrage { an: von, daten: bytes })
            .is_ok()
        {
            self.nachforderung_laeuft = true;
            self.protokoll.schreibe(
                Eintrag::neu("nachschub_angefordert")
                    .text("bei", von.to_string())
                    .zahl("ab", ab as i64)
                    .zahl("bis", bis as i64)
                    .zahl("eigene_hoehe", self.kette.hoehe() as i64),
            );
        }
    }

    /// Verarbeitet eine empfangene Nachricht: Blöcke in die Kette,
    /// Transaktionen in den Mempool.
    fn verarbeite(&mut self, m: &myl_net::InboundMessage) {
        match m.topic {
            GossipTopic::Blocks => {
                use borsh::BorshDeserialize;
                let mut rest = &m.data[..];
                let Ok(block) = myl_consensus::block::Block::deserialize(&mut rest) else {
                    return;
                };
                if !rest.is_empty() {
                    return;
                }
                match self.kette.uebernimm(&block) {
                    Ok(()) => self.protokoll.schreibe(
                        Eintrag::neu("block_uebernommen")
                            .zahl("hoehe", self.kette.hoehe() as i64)
                            .zahl("txs", block.txs.len() as i64)
                            .text("zustandswurzel", kurz(&self.kette.zustandswurzel()))
                            .text("block", kurz(&self.kette.letzter_hash())),
                    ),
                    Err(grund) => {
                        let art = match grund {
                            crate::kette::KettenFehler::SchonBekannt => "dublette",
                            crate::kette::KettenFehler::PasstNichtAn { .. } => "passt-nicht-an",
                            crate::kette::KettenFehler::ZustandWeichtAb { .. } => {
                                "zustand-weicht-ab"
                            }
                        };
                        self.protokoll.schreibe(
                            Eintrag::neu("block_abgelehnt")
                                .zahl("eigene_hoehe", self.kette.hoehe() as i64)
                                .zahl("fremde_hoehe", block.epoch_meta.epoch as i64)
                                .text("art", art)
                                .text("grund", grund.to_string()),
                        );
                        // Passt der Block nicht an und ist er **weiter**
                        // als wir, fehlt uns etwas. Dann fragen wir den,
                        // von dem der Hinweis kam: Er hat den Block, also
                        // hat er mit hoher Wahrscheinlichkeit auch die
                        // davor.
                        if art == "passt-nicht-an" {
                            self.fordere_nach(m.von, block.epoch_meta.epoch);
                        }
                    }
                }
            }
            // Auch der Erzeuger nimmt eigene Transaktionen nicht
            // doppelt: Gossipsub liefert eigene Nachrichten nicht an den
            // Absender zurück.
            // ⚑ A10: Der Empfänger sagt, warum ein Attest nicht trug.
            //
            // `myl-net` verwirft ungültige bereits vor dieser Stelle;
            // was hier ankommt, hat die Prüfung bestanden. Der Eintrag
            // hält fest, dass sie stattgefunden hat, denn genau das war
            // bis zum 2026-08-25 nicht der Fall.
            GossipTopic::LatencyAttests => {
                self.protokoll.schreibe(
                    Eintrag::neu("attest_angenommen")
                        .zahl("bytes", m.data.len() as i64)
                        .text("von", m.von.to_string()),
                );
            }
            GossipTopic::Transactions if self.kette.aufnehmen_roh(&m.data) => {
                self.protokoll.schreibe(
                    Eintrag::neu("tx_aufgenommen").zahl("wartend", self.kette.wartend() as i64),
                );
            }
            _ => {}
        }
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
            NodeEvent::Message(m) => {
                // Die Probe dazuschreiben: Die Auswertung zählt
                // Gesendetes gegen Empfangenes je Funktion, und dafür
                // muss beide Seiten dieselbe Kennung tragen.
                let probe = Probe::ALLE
                    .into_iter()
                    .find(|p| p.topic() == Some(m.topic))
                    .map(|p| p.kennung())
                    .unwrap_or("sonstiges");
                let eintrag = Eintrag::neu("empfangen")
                    .text("topic", format!("{:?}", m.topic))
                    .text("kennung", probe)
                    .text("digest", nutzlast_digest(&m.data))
                    .zahl("bytes", m.data.len() as i64);
                self.protokoll.schreibe(eintrag);
                // Und dann verarbeiten. Getrennt vom Empfangseintrag,
                // damit im Protokoll steht, was ankam, auch wenn die
                // Verarbeitung scheitert.
                self.verarbeite(&m);
                return;
            }
            // Ohne diesen Eintrag ließe sich „nichts kam an" nicht von
            // „es kam an und wurde weggeworfen" unterscheiden, und das
            // ist die erste Frage jeder Fehlersuche.
            // Die Messung, die auf einer Maschine nicht zu haben ist:
            // Auf Loopback gibt es nichts zu durchstoßen.
            NodeEvent::Lochstanzen { peer, gelungen, grund } => Eintrag::neu("lochstanzen")
                .text("gegenstelle", peer.to_string())
                .wahr("gelungen", gelungen)
                .text("grund", grund),
            NodeEvent::Latenz { peer, mikrosekunden } => {
                // Für das eigene Attest: Millisekunden, wie der
                // Attest-Typ sie trägt, aufgerundet damit ein sehr
                // schneller Peer nicht als 0 erscheint.
                self.latenz_je_peer
                    .insert(peer, mikrosekunden.div_ceil(1000).min(u32::MAX as u64) as u32);
                let (kleinste, groesste, anzahl) = self.latenz;
                self.latenz = (
                    kleinste.min(mikrosekunden),
                    groesste.max(mikrosekunden),
                    anzahl + 1,
                );
                // Kein eigener Eintrag: Die Spanne steht in der nächsten
                // Zustandsaufnahme. Siehe Feld `latenz`.
                return;
            }
            // Jemand fragt Blöcke nach. Antworten, soweit vorhanden.
            NodeEvent::AnfrageEingegangen { von, daten, marke } => {
                let (antwort, anzahl) = match Nachforderung::aus_bytes(&daten) {
                    Some(Nachforderung::Bloecke { ab, bis }) => {
                        let bloecke = self.kette.bloecke_von_bis(ab, bis);
                        let n = bloecke.len();
                        if n == 0 {
                            (Nachlieferung::Nichts, 0)
                        } else {
                            (Nachlieferung::Bloecke(bloecke), n)
                        }
                    }
                    // Unlesbare Anfrage: trotzdem antworten. Schweigen
                    // ließe den Fragenden auf eine Zeitüberschreitung
                    // warten, die ihm nichts sagt.
                    None => (Nachlieferung::Nichts, 0),
                };
                if let Some(bytes) = antwort.als_bytes() {
                    let _ = self.kommandos.send(NodeCommand::Antwort { marke, daten: bytes });
                }
                Eintrag::neu("nachschub_geliefert")
                    .text("an", von.to_string())
                    .zahl("bloecke", anzahl as i64)
            }
            // Die nachgeforderten Blöcke sind da.
            NodeEvent::AntwortEingegangen { von, daten } => {
                self.nachforderung_laeuft = false;
                let vorher = self.kette.hoehe();
                let mut angenommen = 0usize;
                let mut abgelehnt = 0usize;
                if let Some(Nachlieferung::Bloecke(bloecke)) = Nachlieferung::aus_bytes(&daten) {
                    // **Derselbe Weg wie bei verbreiteten Blöcken.**
                    // Nachschub ist ein Transportweg, kein
                    // Vertrauensweg: gleiche Anschlussprüfung, gleiche
                    // Nachrechnung der Zustandswurzel.
                    for b in bloecke {
                        match self.kette.uebernimm(&b) {
                            Ok(()) => angenommen += 1,
                            Err(_) => abgelehnt += 1,
                        }
                    }
                }
                Eintrag::neu("nachschub_erhalten")
                    .text("von", von.to_string())
                    .zahl("angenommen", angenommen as i64)
                    .zahl("abgelehnt", abgelehnt as i64)
                    .zahl("hoehe_vorher", vorher as i64)
                    .zahl("hoehe_nachher", self.kette.hoehe() as i64)
            }
            NodeEvent::AnfrageGescheitert { an, grund } => {
                self.nachforderung_laeuft = false;
                Eintrag::neu("nachschub_gescheitert")
                    .text("an", an.to_string())
                    .text("grund", grund)
            }
            // Die Antwort auf „warum verbindet sich niemand zu mir".
            NodeEvent::Erreichbarkeit { addr, erreichbar, grund } => {
                Eintrag::neu("erreichbarkeit")
                    .text("addr", addr.to_string())
                    .wahr("erreichbar", erreichbar)
                    .text("grund", grund)
            }
            NodeEvent::Verworfen { topic: Some(GossipTopic::LatencyAttests), bytes, grund } => {
                // Der häufigste Grund im Probelauf ist ein vergessener
                // Name in --teilnehmer, nicht ein Angriff. Das gehört
                // dazugesagt, sonst sucht jemand am falschen Ort.
                Eintrag::neu("attest_verworfen")
                    .zahl("bytes", bytes as i64)
                    .text("grund", grund.als_text())
                    .text(
                        "hinweis",
                        "bei nutzlastpruefung: fehlt der Aussteller in --teilnehmer?",
                    )
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
