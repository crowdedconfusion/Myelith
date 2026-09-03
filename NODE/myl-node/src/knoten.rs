//! Der laufende Knoten: Start, Ereignisschleife, Protokoll.
//!
//! # Was dieser Knoten heute ist
//!
//! Ein **Netzknoten**: Er findet Gegenstellen, verbreitet und empfängt
//! alle Protokoll-Topics, misst Latenzen, hält seine
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
use crate::konsens::Konsensrunde;
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

/// Kurzform einer Miner-Kennung fürs Protokoll.
///
/// Eigene Funktion statt `kurz(&hash)`, weil eine Kennung kein Hash ist,
/// auch wenn sie aus einem entsteht. Wer beide durch dieselbe Funktion
/// schickt, verwechselt sie irgendwann in einer Protokollzeile.
pub fn kurz_id(id: &myl_types::ids::MinerId) -> String {
    let b = id.as_bytes();
    format!("{:02x}{:02x}{:02x}{:02x}", b[0], b[1], b[2], b[3])
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
    /// Die Kettendatei ließ sich nicht öffnen oder gehört zu einer
    /// anderen Kette.
    ///
    /// **Ein Startfehler, kein Hinweis.** Stillschweigend bei null zu
    /// beginnen hieße, eine vorhandene Historie zu übergehen, und das
    /// fiele erst auf, wenn jemand die Höhen vergleicht.
    Kette(String),
    /// Der Anschluss an den Shard-Prozess kam nicht zustande.
    ///
    /// ⚑ **Ein Startfehler, kein Hinweis.** Wer `--ortsleitung` sagt,
    /// will rechnen lassen; ohne Ausweis lehnt der Knoten jeden Auftrag
    /// ab, und im Betrieb sähe das aus wie ein Shard, der schweigt.
    /// Lieber gleich und laut als später und leise.
    Ortsleitung(String),
}

impl std::fmt::Display for KnotenFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Konfig(e) => write!(f, "Konfiguration: {}", e),
            Self::Protokoll(e) => write!(f, "Betriebsprotokoll: {}", e),
            Self::Identitaet(e) => write!(f, "Identität: {}", e),
            Self::Kette(e) => write!(f, "Kette: {}", e),
            Self::Netz(e) => write!(f, "Netz: {}", e),
            Self::Ortsleitung(e) => write!(f, "Ortsleitung zum Shard-Prozess: {}", e),
        }
    }
}

impl std::error::Error for KnotenFehler {}

/// Ein laufender Knoten.
pub struct Knoten {
    konfig: KnotenKonfig,
    /// Horcht ab dem Start auf Beendigungssignale.
    ///
    /// ⚑ **Steht hier und nicht in `laufen_bis`** (Fund 140). Wer sie
    /// erst dort stellte, liesse den ganzen Vorlauf ungeschuetzt; siehe
    /// [`Beendigungswache`].
    wache: Beendigungswache,
    peer_id: libp2p::PeerId,
    kommandos: mpsc::UnboundedSender<NodeCommand>,
    /// Für welche Epoche die Stichprobe schon verschickt wurde.
    ///
    /// ⚑ **Ohne dies fragte jeder Block erneut.** Die Ziehung gehört zu
    /// einer Epoche, und zwischen zwei Wechseln liegen viele Blöcke;
    /// dieselbe Frage hundertmal zu stellen wäre eine Flut, die der
    /// Gefragte zu Recht als Angriff läse.
    stichprobe_gefragt: Option<u64>,
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
    /// Wohin die Zustandsaufnahme ihre Zahlen zusaetzlich legt.
    ///
    /// ⚑ **Der Abholweg zu den Zahlen, die es laengst gab** (Fund 129).
    /// Siehe [`crate::beobachtung`].
    beobachtungsstelle: crate::beobachtung::Beobachtungsstelle,
    /// Die Abschrift der Sitzungskontrakte für die eigene Tür.
    ///
    /// ⚑ **Bei jedem Block aufgefrischt**, nicht bei jeder Anfrage: Der
    /// Kettenzustand gehört dieser Schleife, und eine Sperre über ein
    /// `await` wäre der Weg in den Stillstand.
    kontraktabschrift: crate::tuer::Kontraktabschrift,
    /// Die zuletzt eingetroffene Antwort auf einen Inferenzauftrag.
    ///
    /// ⚑ **Ein Fach und keine Warteschlange**, solange nur ein Auftrag
    /// zugleich läuft. Wer mehrere gleichzeitig führt, braucht eine
    /// Zuordnung über die Sitzungsnummer, und die steht in der Antwort;
    /// hier fehlt sie noch, und das gehört gesagt statt angenommen.
    letzte_inferenzantwort: Option<myl_types::inferenzauftrag::Inferenzantwort>,
    /// Der Anschluss an einen lokalen Shard-Prozess, oder `None`.
    ///
    /// ⚑ **Die Vorgabe ist `None`, anders als bei der Tür.** Ein Knoten
    /// ist nicht notwendig ein Miner: Er kann Teil eines Pods sein,
    /// Speicher stellen oder einfach nur Nutzer sein. Wer rechnet, sagt
    /// es mit `--ortsleitung`; wer nichts sagt, lehnt Aufträge ehrlich
    /// ab, statt einen Shard vorzutäuschen, den es nicht gibt.
    ortsanschluss: Option<crate::ortsklient::Ortsanschluss>,
    /// Was die Tür an Abrechnungen abgelegt hat.
    ///
    /// ⚑ **Ein Kanal und kein direkter Aufruf**, weil die Tür in einer
    /// eigenen Aufgabe läuft und der Kettenzustand der Ereignisschleife
    /// gehört. Dieselbe Bauart wie bei der Kontraktabschrift: Wer
    /// schreibt, schreibt hier; wer die Kette anfasst, tut es dort.
    abrechnungen: tokio::sync::mpsc::UnboundedReceiver<myl_consensus::block::Anweisung>,
    /// Das Gegenstück, für die Tür.
    abrechnungskanal: tokio::sync::mpsc::UnboundedSender<myl_consensus::block::Anweisung>,
    /// Die Nonce der eigenen Abrechnungstransaktionen.
    ///
    /// ⚑ **Getrennt vom Testverkehrszähler**, sonst vergäben zwei
    /// Quellen dieselbe Nonce und eine der beiden Transaktionen fiele
    /// still aus.
    abrechnungsnonce: u64,
    /// Die hoechste Hoehe, von der dieser Knoten gehoert hat.
    ///
    /// **Grundlage der Bereitschaftsauskunft.** Wer eine hoehere Hoehe
    /// kennt als seine eigene, ist im Rueckstand und sollte keinen
    /// Verkehr bekommen; ohne diese Zahl waere „bereit" nicht von „hat
    /// noch nichts gehoert" zu unterscheiden.
    hoechste_gehoerte: u64,
    /// Die eigenen Horchadressen, wie sie gemeldet wurden.
    ///
    /// Der Knoten kennt sie beim Start noch nicht: Bei Port 0 vergibt
    /// das Betriebssystem, bei einer Relais-Reservierung das Relais.
    /// Wer die Adresse weitergeben will, muss warten können.
    horchadressen: Vec<libp2p::Multiaddr>,
    /// Die laufende BFT-Runde, falls dieser Knoten mitstimmt.
    ///
    /// `None` bei einem Knoten, der nur zuhört. **Das ist der
    /// Normalfall**, nicht die Ausnahme: Stimmberechtigt ist, wer in der
    /// Stimmsatzdatei steht, und das sind wenige.
    konsens: Option<Konsensrunde>,
    /// Startzeitpunkt, als **monotone** Uhr für die BFT-Fristen.
    ///
    /// ⚑ **Nicht die Wanduhr**, obwohl das Betriebsprotokoll sie
    /// benutzt. Die Wanduhr springt: NTP korrigiert, und ein Sprung
    /// rückwärts verlängert eine laufende Frist, ein Sprung vorwärts
    /// lässt sie zu früh feuern. Für BFT ist das keine Safety-Frage,
    /// wohl aber eine Liveness-Frage, und eine Runde, die grundlos
    /// wechselt, sieht im Protokoll aus wie ein ausgefallener Leader.
    ///
    /// `Instant` kann nicht rückwärts laufen. Die Fristen zählen
    /// deshalb Millisekunden seit dem Start dieses Prozesses.
    gestartet: std::time::Instant,
    /// Konsensnachrichten, die ankamen, **bevor** die eigene Runde
    /// begann.
    ///
    /// # ⚑ Fund 63: 417 Millisekunden, und die Runde war tot
    ///
    /// Im ersten Lauf über fünf Prozesse (2026-08-26) veröffentlichte
    /// der Leader seinen Propose 4 ms nach dem Start seiner Runde. Bei
    /// allen vier anderen **kam er an**, im selben Millisekundenfenster.
    /// Ihre eigene Runde begann 417 ms später, weil jeder erst auf sein
    /// Mesh wartete. In dieser Lücke war `self.konsens` noch `None`, und
    /// die Nachricht wurde verworfen, **ohne eine Protokollzeile**.
    /// Danach wartete das ganze Netz auf einen Propose, den es längst
    /// bekommen hatte.
    ///
    /// **Der Modultest konnte das nicht sehen.** Dort beginnen alle
    /// Knoten ihre Runde, bevor die erste Nachricht fließt, weil eine
    /// Schlange serialisiert, was ein Netz parallel macht. Die Form des
    /// Tests hat den Defekt verdeckt.
    ///
    /// **Warum ein Puffer und nicht „einfach früher starten":** Weil es
    /// keinen gemeinsamen Zeitpunkt gibt. Knoten starten, wann ihre
    /// Betreiber sie starten. Ein Puffer macht aus einem Wettlauf eine
    /// Reihenfolge.
    ///
    /// **Warum beschränkt:** Ein unbeschränkter Vorlauf ist ein
    /// Speicherangriff, den jeder Fremde auslösen kann, indem er einem
    /// Knoten ohne Runde Nachrichten schickt. Bei
    /// [`Self::MAX_VORLAUF`] Einträgen à 169 Bytes sind das rund 11 KB,
    /// und der älteste weicht.
    ///
    /// **Was das nicht ersetzt:** einen Rundenwechsel. Kommt eine
    /// Nachricht mehr als eine Runde zu früh, hilft kein Puffer, sondern
    /// nur ein Leader, der seinen Propose wiederholt. Das ist der
    /// nächste Punkt.
    konsens_vorlauf: std::collections::VecDeque<Vec<u8>>,
    /// Wie viele Nachrichten der Vorlauf schon verworfen hat.
    ///
    /// Geht in die Zustandsaufnahme. **Ohne diese Zahl wäre der Puffer
    /// dieselbe Stille wie vorher**, nur an einer anderen Stelle.
    konsens_vorlauf_verworfen: u64,
    /// Konsensnachrichten, die noch hinaus müssen.
    ///
    /// ⚑ **Ein Puffer und kein sofortiges Senden**, weil die
    /// Nachrichtenbehandlung synchron ist und das Senden nicht. Ein
    /// synchrones `Publish` ohne Rückmeldung wäre der bequemere Weg,
    /// verschenkte aber genau die Auskunft, die hier zählt: Ob
    /// Gossipsub die Nachricht **angenommen** hat. Ein abgelehntes
    /// „noch kein Mesh" sieht im Protokoll sonst aus wie eine
    /// verschickte Stimme, und die Runde hängt scheinbar grundlos.
    konsens_ausgang: Vec<myl_consensus::bft::Konsensnachricht>,
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

        // Der Weg von der Tür zur Kette. Unbegrenzt, weil eine
        // Abrechnung nicht verlorengehen darf: Ein voller Kanal hiesse,
        // dass gerechnete Arbeit unbezahlt bleibt.
        let (abrechnungen_tx, abrechnungen_rx) = tokio::sync::mpsc::unbounded_channel();

        // ⚑ **Der Ausweis wird beim Start gelesen, nicht bei der ersten
        // Frage.** Eine Datei im heissen Pfad ist eine Fehlerquelle im
        // heissen Pfad, und ein fehlender Ausweis soll den Betreiber
        // beim Start erreichen und nicht den ersten Nutzer.
        let ortsanschluss = match (konfig.ortsleitung, konfig.ortsausweis.as_ref()) {
            (Some(adr), Some(ausweis)) => Some(
                crate::ortsklient::Ortsanschluss::neu(adr, ausweis)
                    .map_err(|e| KnotenFehler::Ortsleitung(format!("{}: {e}", ausweis.display())))?,
            ),
            _ => None,
        };

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

        // ⚑ **Der Wiederanlauf ist ein Nachrechnen, kein Einlesen.**
        //
        // Die Blöcke aus der Datei gehen durch dieselbe
        // `Kette::uebernimm`, durch die auch Gossip-Blöcke gehen: Jeder
        // Vorgänger-Hash und jede Zustandswurzel wird neu geprüft. Ein
        // zweiter Ladepfad mit eigenen Regeln wäre die Stelle, an der
        // eine manipulierte Datei durchkäme.
        //
        // Und der Zustand wird **nicht** gespeichert, sondern
        // hergeleitet. Ein abgeleiteter Wert, den man zusätzlich
        // ablegt, ist eine zweite Wahrheit.
        let mut kette = Kette::probestand();
        if let Some(pfad) = konfig.kettendatei.clone() {
            match crate::speicher::Kettenspeicher::oeffnen(&pfad, Kette::startwert()) {
                Ok((mut speicher, anlauf)) => {
                    let vorhanden = anlauf.anzahl;
                    let mut uebernommen = 0usize;
                    let mut abgelehnt = 0usize;
                    let mut erster_grund = String::new();
                    // ⚑ **Satz für Satz, nicht alle auf einmal**
                    // (Fund 124). Bis zum 2026-09-02 stand die ganze
                    // Kette im Arbeitsspeicher, bevor der erste Block
                    // geprüft war. Jetzt liegt immer nur einer da.
                    //
                    // Ein Lesefehler mitten im Nachspielen bricht ab:
                    // Was danach kommt, schlösse an einen Block an, den
                    // dieser Knoten nicht hat, und würde ohnehin
                    // abgewiesen. Der Grund geht ins Protokoll.
                    let lesefehler = speicher
                        .fuer_jeden_satz(|b| match kette.uebernimm(&b) {
                            Ok(()) => uebernommen += 1,
                            Err(e) => {
                                abgelehnt += 1;
                                if erster_grund.is_empty() {
                                    erster_grund = e.to_string();
                                }
                            }
                        })
                        .err();
                    // Erst **nach** dem Nachspielen anhängen, sonst
                    // schriebe der Wiederanlauf jeden Block ein zweites
                    // Mal in dieselbe Datei.
                    kette.speicher_setzen(speicher);
                    let mut eintrag = Eintrag::neu("kette_geladen")
                        .text("datei", pfad.display().to_string())
                        .wahr("neu", anlauf.neu)
                        .zahl("in_datei", vorhanden as i64)
                        .text(
                            "lesefehler",
                            lesefehler.map(|e| e.to_string()).unwrap_or_default(),
                        )
                        .zahl("uebernommen", uebernommen as i64)
                        .zahl("abgelehnt", abgelehnt as i64)
                        // Ein abgebrochener letzter Satz heißt: der
                        // Knoten wurde mitten im Schreiben abgeräumt.
                        // Genau das will der Chaos-Test wissen.
                        .zahl("abgeschnitten_bytes", anlauf.abgeschnitten as i64)
                        .zahl("hoehe", kette.hoehe() as i64)
                        .text("zustandswurzel", kurz(&kette.zustandswurzel()));
                    if !erster_grund.is_empty() {
                        eintrag = eintrag.text("erster_grund", erster_grund);
                    }
                    protokoll.schreibe(eintrag);
                }
                Err(e) => {
                    // Eine unlesbare Kettendatei ist ein Startfehler.
                    // Stillschweigend bei null zu beginnen hieße, eine
                    // vorhandene Historie zu übergehen, und das fiele
                    // erst auf, wenn jemand die Höhen vergleicht.
                    protokoll.schreibe(
                        Eintrag::neu("kette_nicht_geladen")
                            .text("datei", pfad.display().to_string())
                            .text("grund", e.to_string()),
                    );
                    return Err(KnotenFehler::Kette(e.to_string()));
                }
            }
        }

        Ok(Self {
            konfig,
            // ⚑ **Als Erstes, noch vor allem Warten** (Fund 140). Ab
            // hier ist jedes Signal aufgehoben.
            wache: Beendigungswache::stellen(),
            peer_id,
            kommandos: cmd_tx,
            stichprobe_gefragt: None,
            ereignisse: ev_rx,
            protokoll,
            testverkehr_zaehler: 0,
            kette,
            nachforderung_laeuft: false,
            beobachtungsstelle: crate::beobachtung::Beobachtungsstelle::neu(),
            kontraktabschrift: crate::tuer::Kontraktabschrift::neu(),
            letzte_inferenzantwort: None,
            ortsanschluss,
            abrechnungen: abrechnungen_rx,
            abrechnungskanal: abrechnungen_tx,
            abrechnungsnonce: 1_000_000,
            hoechste_gehoerte: 0,
            latenz_je_peer: std::collections::BTreeMap::new(),
            latenz: (u64::MAX, 0, 0),
            horchadressen: Vec::new(),
            konsens: None,
            gestartet: std::time::Instant::now(),
            konsens_ausgang: Vec::new(),
            konsens_vorlauf: std::collections::VecDeque::new(),
            konsens_vorlauf_verworfen: 0,
        })
    }

    /// Höchstzahl der Nachrichten, die vor dem Rundenbeginn aufgehoben
    /// werden.
    ///
    /// **Hergeleitet:** Eine Runde erzeugt je Validator höchstens drei
    /// Nachrichten (Propose, Vote, Commit). 64 trägt damit ein Komitee
    /// von 21 Producern vollständig, also die Größe aus
    /// `myl_consensus::validator::COMMITTEE_SIZE`, und noch etwas
    /// Doppeltes aus dem Gossip.
    pub const MAX_VORLAUF: usize = 64;

    /// Wartet, bis das Mesh eines Topics eine Mindestgröße hat.
    ///
    /// ⚑ **Verbunden heißt nicht im Mesh.** Gossipsub führt je Topic
    /// eine eigene Menge von Peers, an die es Nachrichten vollständig
    /// weitergibt. Wer vor dem Mesh publiziert, bekommt „zu wenige
    /// Peers" zurück, und die Nachricht ist weg. Beim Propose eines
    /// Leaders wäre das das Ende der Runde, und im Protokoll sähe es
    /// aus, als hätte niemand geantwortet.
    ///
    /// Gibt die erreichte Mesh-Größe zurück, auch wenn die Frist
    /// abläuft: Die Zahl gehört ins Protokoll, damit „es lief nicht an"
    /// von „es lief an und niemand antwortete" unterscheidbar bleibt.
    pub async fn warte_auf_mesh(
        &mut self,
        topic: GossipTopic,
        mindestens: usize,
        frist: Duration,
    ) -> usize {
        let ende = tokio::time::Instant::now() + frist;
        let groesse;
        loop {
            let z = self.zustand().await;
            let jetzt = z
                .mesh
                .iter()
                .find(|(t, _)| *t == topic)
                .map(|(_, n)| *n)
                .unwrap_or(0);
            if jetzt >= mindestens || tokio::time::Instant::now() >= ende {
                groesse = jetzt;
                break;
            }
            self.laufe_fuer(Duration::from_millis(200)).await;
        }
        self.protokoll.schreibe(
            Eintrag::neu("mesh_erreicht")
                .text("topic", format!("{:?}", topic))
                .zahl("groesse", groesse as i64)
                .zahl("mindestens", mindestens as i64)
                .wahr("erreicht", groesse >= mindestens),
        );
        groesse
    }

    /// Beginnt eine BFT-Runde und schickt hinaus, was sofort hinaus muss.
    ///
    /// Der Knoten muss in der Stimmsatzdatei stehen, sonst
    /// [`KonsensFehler::NichtStimmberechtigt`].
    ///
    /// ⚑ **Die Herkunft des Schlüssels landet im Protokoll.** Ein
    /// Probeschlüssel ist aus dem Teilnehmernamen ableitbar; wer damit
    /// ins Netz geht, soll es nicht nur wissen, sondern es soll
    /// nachträglich aus dem Protokoll hervorgehen.
    pub async fn beginne_konsensrunde(
        &mut self,
        genesis: &crate::stimmsatzdatei::Stimmsatzdatei,
        schluessel: crate::schluessel::Konsensschluessel,
        vorschlag: myl_types::hash::Hash,
        timeouts: myl_consensus::round_change::TimeoutConfig,
    ) -> Result<(), crate::konsens::KonsensFehler> {
        let herkunft = schluessel.herkunft();
        let jetzt = self.uhr_ms();
        let (laufende, raus) =
            Konsensrunde::beginnen(genesis, schluessel, vorschlag, jetzt, timeouts)?;
        // ⚑ **Und die Kette bekommt den Stimmsatz** (Punkt 44). Ohne ihn
        // prüft sie eine Saatquelle nur gegen den Vorgänger und nicht
        // gegen die Unterschriften; sie sagt das dann auch, statt
        // stillschweigend alles anzunehmen.
        self.kette.stimmsatz_setzen(genesis.stimmberechtigte());
        let runde = laufende.runde();
        self.protokoll.schreibe(
            Eintrag::neu("konsens_runde_beginnt")
                .zahl("runde", runde as i64)
                .text("genesis", kurz(&genesis.hash()))
                .text("netz", genesis.netz.clone())
                .zahl("validatoren", genesis.validatoren.len() as i64)
                .text("leader", kurz_id(&laufende.leader()))
                .wahr("ich_bin_leader", laufende.leader() == laufende.ich())
                .text("schluesselherkunft", herkunft.als_text())
                .wahr("schluessel_geheim", herkunft.ist_geheim())
                .zahl("timeout_propose_ms", timeouts.propose_ms as i64)
                .zahl("timeout_delta_ms", timeouts.delta_ms as i64)
                // Ohne Zuwachs ist das Verfahren sicher, aber
                // möglicherweise dauerhaft blockiert.
                .wahr("liveness_moeglich", timeouts.is_live()),
        );
        self.konsens = Some(laufende);
        for n in raus {
            self.sende_konsens(&n).await;
        }

        // ⚑ Fund 63: Was ankam, bevor es diese Runde gab, jetzt
        // nachreichen. Der Zustandsautomat verwirft selbst, was nicht
        // passt (falsche Runde, Duplikat), also ist das Nachreichen
        // ungefährlich.
        let vorlauf: Vec<Vec<u8>> = self.konsens_vorlauf.drain(..).collect();
        if !vorlauf.is_empty() {
            self.protokoll.schreibe(
                Eintrag::neu("konsens_vorlauf_nachgereicht")
                    .zahl("nachrichten", vorlauf.len() as i64)
                    .zahl("verworfen", self.konsens_vorlauf_verworfen as i64),
            );
            for daten in vorlauf {
                let folge = self.nimm_konsens_an(&daten);
                self.konsens_ausgang.extend(folge);
            }
            self.leere_konsensausgang().await;
        }
        Ok(())
    }

    /// Die laufende Runde, für Tests und Diagnose.
    pub fn konsens(&self) -> Option<&Konsensrunde> {
        self.konsens.as_ref()
    }

    /// Millisekunden seit dem Start dieses Prozesses.
    ///
    /// Die Uhr der BFT-Fristen. Siehe [`Self::gestartet`], warum nicht
    /// die Wanduhr.
    pub fn uhr_ms(&self) -> u64 {
        self.gestartet.elapsed().as_millis() as u64
    }

    /// Prüft die Frist der laufenden Runde und wechselt gegebenenfalls.
    ///
    /// Wird vom Ereignisschleifen-Takt gerufen. Gibt zurück, ob
    /// gewechselt wurde.
    pub async fn konsens_takt(&mut self) -> bool {
        let jetzt = self.uhr_ms();
        let Some(runde) = self.konsens.as_mut() else {
            return false;
        };
        if runde.ist_commitet() {
            return false;
        }
        let (wechsel, raus) = runde.takt(jetzt);
        let Some(myl_consensus::round_change::RoundChange::Advanced { from, to, leader }) = wechsel
        else {
            return false;
        };
        let (stimmen, commits, schwelle) = self.konsens.as_ref().map(|r| r.gewichte()).unwrap();
        let sperre = self.konsens.as_ref().and_then(|r| r.sperre());
        let mut eintrag = Eintrag::neu("konsens_rundenwechsel")
            .zahl("von", from as i64)
            .zahl("nach", to as i64)
            .text("neuer_leader", kurz_id(&leader))
            .wahr("ich_bin_leader", self.konsens.as_ref().map(|r| r.ich()) == Some(leader))
            // Warum die Frist verfiel, steht im Gewicht: Ein Wechsel bei
            // 0 Stimmen heißt „kein Vorschlag kam an", ein Wechsel bei
            // fast erreichter Schwelle heißt etwas ganz anderes.
            .zahl("stimmgewicht", stimmen as i64)
            .zahl("commitgewicht", commits as i64)
            .zahl("schwelle", schwelle as i64);
        if let Some(l) = sperre {
            eintrag = eintrag
                .text("gesperrt_auf", kurz(&l.block_hash))
                .zahl("sperrrunde", l.round as i64);
        }
        self.protokoll.schreibe(eintrag);
        for n in raus {
            self.sende_konsens(&n).await;
        }
        true
    }

    /// Schickt hinaus, was die Nachrichtenbehandlung angesammelt hat.
    ///
    /// Wiederholt, solange etwas nachkommt: Eine Stimme kann einen
    /// Commit auslösen, und der muss in derselben Runde hinaus.
    async fn leere_konsensausgang(&mut self) {
        while !self.konsens_ausgang.is_empty() {
            let stapel = std::mem::take(&mut self.konsens_ausgang);
            for n in stapel {
                self.sende_konsens(&n).await;
            }
        }
    }

    async fn sende_konsens(&mut self, n: &myl_consensus::bft::Konsensnachricht) {
        let Ok(bytes) = borsh::to_vec(n) else { return };
        let art = n.art();
        let runde = n.runde();
        self.veroeffentliche(GossipTopic::Consensus, bytes).await;
        self.protokoll.schreibe(
            Eintrag::neu("konsens_gesendet")
                .text("nachricht", art)
                .zahl("runde", runde as i64),
        );
    }

    /// Nimmt eine Konsensnachricht von der Leitung an.
    ///
    /// Gibt die Folgenachrichten zurück, statt sie selbst zu senden:
    /// [`Self::verarbeite`] ist synchron, das Senden ist es nicht.
    fn nimm_konsens_an(
        &mut self,
        daten: &[u8],
    ) -> Vec<myl_consensus::bft::Konsensnachricht> {
        let jetzt = self.uhr_ms();
        let Some(runde) = self.konsens.as_mut() else {
            // Noch keine eigene Runde: aufheben statt wegwerfen
            // (⚑ Fund 63, siehe `konsens_vorlauf`).
            //
            // Ein Knoten ohne Stimmrecht sammelt hier ebenfalls, und das
            // ist hinnehmbar: Der Puffer ist beschränkt, und ein reiner
            // Zuhörer verliert dadurch nichts als ein paar Kilobyte.
            self.konsens_vorlauf.push_back(daten.to_vec());
            while self.konsens_vorlauf.len() > Self::MAX_VORLAUF {
                self.konsens_vorlauf.pop_front();
                self.konsens_vorlauf_verworfen += 1;
            }
            return Vec::new();
        };
        let (urteil, raus) = runde.empfange_bytes(daten, jetzt);
        let (stimmen, commits, schwelle) = runde.gewichte();
        let commitet = runde.ist_commitet();
        // ⚑ **Gewicht, nicht Köpfe.** Ein Protokoll, das „3 von 5
        // Stimmen" meldet, verdeckt genau den Unterschied, für den die
        // Genesis-Verteilung gebaut wurde.
        self.protokoll.schreibe(
            Eintrag::neu("konsens_empfangen")
                .text("urteil", urteil.als_text())
                .wahr("harmlos", urteil.ist_harmlos())
                .zahl("stimmgewicht", stimmen as i64)
                .zahl("commitgewicht", commits as i64)
                .zahl("schwelle", schwelle as i64)
                .wahr("commitet", commitet),
        );
        if commitet {
            if let Some(block) = self.konsens.as_ref().and_then(|r| r.commiteter_block()) {
                let uebernommen = self
                    .konsens
                    .as_ref()
                    .map(|r| r.durch_beleg_commitet())
                    .unwrap_or(false);
                self.protokoll.schreibe(
                    Eintrag::neu("konsens_commitet")
                        .zahl("runde", self.konsens.as_ref().map(|r| r.runde()).unwrap_or(0) as i64)
                        .text("block", kurz(&block))
                        // ⚑ Fund 67: „hat mitgezählt" und „wurde
                        // zurückgeholt" sehen ohne dieses Feld gleich aus.
                        .wahr("uebernommen", uebernommen),
                );
            }
        }
        raus
    }

    /// Die eigene Kette, für Tests und Diagnose.
    /// Die Kette zum Ändern, **nur für Tests und den Betreiber**.
    ///
    /// ⚑ **Kein Weg für Teilnehmer**, dieselbe Begründung wie bei
    /// [`Kette::zustand_mut`]: Wer den Zustand von außen ändert, umgeht
    /// jede Übergangsprüfung.
    #[doc(hidden)]
    pub fn kette_mut(&mut self) -> &mut Kette {
        &mut self.kette
    }

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
            // Der BFT-Takt. Prüft selbst, ob die Frist verfallen ist,
            // und wechselt gegebenenfalls die Runde.
            self.konsens_takt().await;

            let mut rest = ende
                .saturating_duration_since(jetzt)
                .min(naechste_aufnahme.saturating_duration_since(jetzt));
            if let Some(faellig) = naechster_versand {
                rest = rest.min(faellig.saturating_duration_since(jetzt));
            }
            // ⚑ **Die Wartezeit darf die Konsensfrist nicht überspringen.**
            // Ohne diese Zeile schliefe der Knoten bis zur nächsten
            // Zustandsaufnahme, also bis zu 30 Sekunden, und ein
            // ausgefallener Leader hielte die Runde so lange auf, obwohl
            // die Frist längst abgelaufen wäre.
            if let Some(k) = self.konsens.as_ref() {
                if !k.ist_commitet() {
                    let verbleibend = k.frist_ms().saturating_sub(self.uhr_ms());
                    rest = rest.min(Duration::from_millis(verbleibend));
                }
            }
            let rest = rest.max(Duration::from_millis(1));
            match tokio::time::timeout(rest, self.ereignisse.recv()).await {
                Ok(Some(ev)) => {
                    self.vermerke(ev);
                    self.leere_konsensausgang().await;
                }
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
        let wache = self.wache.clone();
        let grund = tokio::select! {
            g = wache.warten() => g,
            _ = async {
                match dauer {
                    Some(d) => self.laufe_fuer(d).await,
                    // Ohne Grenze: in Abschnitten, damit die
                    // Zustandsaufnahmen weiterlaufen.
                    None => loop { self.laufe_fuer(Duration::from_secs(3600)).await },
                }
            } => "Laufzeit abgelaufen",
        };
        self.abschluss(grund).await;
    }

    /// Schickt einen Inferenzauftrag an einen Knoten.
    ///
    /// ⚑ **Über dieselbe Schiene wie die Blocknachforderung.** Ein
    /// eigenes Protokoll hätte einen zweiten Codec, eine zweite
    /// Größengrenze und eine zweite Zerlegung auf fremden Eingaben.
    ///
    /// **Die Form wird vor dem Senden geprüft**, nicht erst beim
    /// Empfänger: Ein Auftrag, der ohnehin abgewiesen wird, soll die
    /// Leitung nicht belasten. Der Empfänger prüft trotzdem noch
    /// einmal, denn er glaubt dem Absender nichts.
    pub async fn inferenz_senden(
        &mut self,
        an: libp2p::PeerId,
        auftrag: myl_types::inferenzauftrag::Inferenzauftrag,
    ) -> bool {
        if auftrag.pruefe_form().is_err() {
            return false;
        }
        let Some(daten) = crate::nachschub::Nachforderung::Inferenz(auftrag).als_bytes() else {
            return false;
        };
        self.kommandos
            .send(NodeCommand::Anfrage { an, daten })
            .is_ok()
    }

    /// Die zuletzt eingetroffene Inferenzantwort.
    pub fn letzte_inferenzantwort(
        &self,
    ) -> Option<&myl_types::inferenzauftrag::Inferenzantwort> {
        self.letzte_inferenzantwort.as_ref()
    }

    /// Die Kontraktabschrift dieses Knotens, für die eigene Tür.
    pub fn kontraktabschrift(&self) -> crate::tuer::Kontraktabschrift {
        self.kontraktabschrift.clone()
    }

    /// Die Beobachtungsstelle dieses Knotens.
    ///
    /// Der Klon ist billig; der Dienst haelt einen und liest daraus,
    /// waehrend der Knoten hineinschreibt.
    pub fn beobachtungsstelle(&self) -> crate::beobachtung::Beobachtungsstelle {
        self.beobachtungsstelle.clone()
    }

    /// Die Wache dieses Knotens, zum Mitlauschen.
    ///
    /// Der Klon ist billig und **unabhaengig vom Knoten**: Damit kann
    /// `main` den Startvorlauf gegen das Signal stellen, obwohl der
    /// Vorlauf den Knoten die ganze Zeit ausleiht.
    pub fn beendigungswache(&self) -> Beendigungswache {
        self.wache.clone()
    }

    /// Schreibt den Abschluss, ohne vorher gelaufen zu sein.
    ///
    /// Fuer den Fall, dass das Signal schon im Startvorlauf kommt: Auch
    /// dann soll im Protokoll stehen, dass jemand beendet hat, und
    /// nicht nichts (Fund 140).
    pub async fn abschluss(&mut self, grund: &str) {
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
            // Das Warten behebt den Anlass, nicht die Ursache. Die
            // Ursache ist seit v0.4.0 behoben: `crate::nachschub` holt
            // fehlende Blöcke nach, sobald ein Block „passt nicht an"
            // meldet und weiter ist als die eigene Höhe.
            //
            // *Hier stand bis zum 2026-08-26 „Eine Blocksynchronisierung
            // fehlt und gehört vor ein echtes Testnetz." Das war seit dem
            // 2026-08-24 überholt.*
            //
            // Das Warten bleibt trotzdem: Es erspart dem Netz eine Runde
            // aus Ablehnungen und Nachforderungen, die niemand braucht,
            // wenn man eine Sekunde wartet.
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
                let Some(c) = crate::probe::probe_challenge(&name, folge) else {
                    self.vermerke_probe(probe, false);
                    return false;
                };
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
        // ⚑ **Die Saatquelle aus dem eigenen Commitzertifikat** (Punkt
        // 44). Sie geht in den Block, damit jeder dieselbe Stichprobe
        // zieht; eine Saat aus lokalem Zustand wäre keine Saat, sondern
        // eine Meinung.
        if let Some(z) = self.konsens.as_ref().and_then(|k| k.commitzertifikat()) {
            // ⚑ **Das ganze Zertifikat, nicht nur das Aggregat.** Eine
            // Quelle, die sich nicht selbst beschreibt, kann der
            // Empfänger nicht prüfen; mit dem Zertifikat kann er es.
            if let Ok(roh) = borsh::to_vec(z) {
                self.kette.saatquelle_setzen(roh);
            }
        }
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
        self.stichprobe_verschicken().await;
        ok
    }

    /// Fragt die gezogenen Segmente bei ihren Pods ab (Punkt 45).
    ///
    /// # ⚑ Die Naht, ohne die die Ziehung nichts bewirkt
    ///
    /// Die Lotterie zieht seit heute (Fund 114), die Adresse steht seit
    /// heute in der Kette (Fund 116), und die Prüfung einer Antwort ist
    /// vollständig. **Ohne diesen Aufruf bliebe alles drei ohne Wirkung**,
    /// und genau so ist Fund 114 entstanden.
    ///
    /// # Was hier geschieht und was nicht
    ///
    /// Gefragt wird **jedes Mitglied** des Pods, nicht nur der
    /// Koordinator: Sonst genügte dessen Schweigen. Der Merkle-Beweis
    /// bindet die Antwort an die unterschriebene Wurzel, also ist
    /// gleichgültig, wer antwortet.
    ///
    /// ⚑ **Die Antwort wird hier noch nicht verarbeitet.** Sie kommt als
    /// `NodeEvent` zurück, und was dann zu tun ist, steht in
    /// `myl_verifier::pruefe_spurantwort`; **was fehlt, ist ein
    /// Nachrechner mit Modell**, und der hängt an Artefakten. Diese
    /// Grenze steht hier, damit niemand den Aufruf für mehr hält.
    async fn stichprobe_verschicken(&mut self) {
        let Some(epoche) = self.kette.stichprobenepoche() else {
            return;
        };
        if self.stichprobe_gefragt == Some(epoche) {
            return;
        }
        let gezogen: Vec<_> = self.kette.letzte_stichprobe().to_vec();
        if gezogen.is_empty() {
            return;
        }
        self.stichprobe_gefragt = Some(epoche);

        let epoche_ = epoche;
        let (fragen, ohne_adresse) = crate::stichprobe::anfragen_fuer(&gezogen, epoche_, |k| {
            self.kette.pod_der_kennung(epoche_, k)
        });
        let mut gefragt = 0usize;
        for (adresse, anfrage) in fragen {
            let Ok(daten) = borsh::to_vec(&anfrage) else {
                continue;
            };
            let Ok(an) = myl_net::peer_id_aus_bytes(&adresse) else {
                continue;
            };
            if self
                .kommandos
                .send(NodeCommand::Anfrage { an, daten })
                .is_ok()
            {
                gefragt += 1;
            }
        }
        // ⚑ **Ohne Adresse ist ein Pod nicht prüfbar, und das ist ein
        // Befund.** Er steht im Protokoll und nicht in einer stillen
        // Fortsetzung: Sonst wäre „ich nenne keine Adresse" die
        // billigste Art, sich der Prüfung zu entziehen.
        self.protokoll.schreibe(
            Eintrag::neu("stichprobe_gefragt")
                .zahl("epoche", epoche as i64)
                .zahl("segmente", gezogen.len() as i64)
                .zahl("anfragen", gefragt as i64)
                .zahl("ohne_adresse", ohne_adresse as i64),
        );
    }

    /// Schickt eine Transaktion ins Netz.
    ///
    /// Die Nicht-Erzeuger tun das. Ohne Transaktionen wären alle Blöcke
    /// leer, und dann sagte die Übereinstimmung der Zustandswurzeln
    /// nichts: Ein leerer Zustand ist überall gleich.
    pub async fn sende_transaktion(&mut self) -> bool {
        use myl_consensus::block::{Anweisung, Transaktion};

        // Ein **ausgestattetes** Testkonto, über den Knotennamen
        // gewählt. Ein beliebiger Absender hätte kein Guthaben, der
        // Burn scheiterte still, und der Zustand bewegte sich nie:
        // Dann belegte die Übereinstimmung der Wurzeln nichts.
        //
        // ⚑ **Die Nummer zählt hoch und wird mit unterschrieben**
        // (2026-08-28). Ohne sie wäre jede dieser Transaktionen ein
        // Wiedereinspielung ihrer Vorgängerin und würde beim Anwenden
        // verworfen; der Zustand bewegte sich wieder nicht, und der
        // Testverkehr belegte wieder nichts.
        let schluessel = crate::kette::schluessel_fuer(&self.konfig.name);
        let anweisung = Anweisung::Burn {
            betrag: 1_000 + self.testverkehr_zaehler * 100,
        };
        let tx = match Transaktion::signiere(
            &crate::kette::Kette::startwert(),
            &schluessel,
            self.testverkehr_zaehler,
            anweisung,
        ) {
            Ok(t) => t,
            Err(_) => return false,
        };
        let daten = match borsh::to_vec(&tx) {
            Ok(d) => d,
            Err(_) => return false,
        };
        self.veroeffentliche(GossipTopic::Transactions, daten).await
    }

    /// Der Kanal, über den die Tür ihre Abrechnungen abgibt.
    pub fn abrechnungskanal(
        &self,
    ) -> tokio::sync::mpsc::UnboundedSender<myl_consensus::block::Anweisung> {
        self.abrechnungskanal.clone()
    }

    /// Holt abgelegte Abrechnungen ab, signiert sie und verbreitet sie.
    ///
    /// # ⚑ Warum das der Knoten tut und nicht die Tür
    ///
    /// Eine Kettentransaktion braucht einen Schlüssel, und die Tür hat
    /// keinen: Ein Harness weist sich mit einer **Vollmacht** aus. Der
    /// Betreiber reicht deshalb ein, und die Kette erkennt die Vollmacht
    /// des Agenten an (siehe `myl_ledger::transitions::sitzung_ausgeben`).
    ///
    /// ⚑ **Die Nonce zählt hoch und wird mit unterschrieben.** Ohne sie
    /// wäre jede Abrechnung die Wiedereinspielung ihrer Vorgängerin und
    /// fiele beim Anwenden still heraus.
    ///
    /// Gibt zurück, wie viele verbreitet wurden.
    pub async fn abrechnungen_verbreiten(&mut self) -> usize {
        use myl_consensus::block::Transaktion;

        let mut offen = Vec::new();
        while let Ok(a) = self.abrechnungen.try_recv() {
            offen.push(a);
        }
        let schluessel = crate::kette::schluessel_fuer(&self.konfig.name);
        let mut verbreitet = 0usize;
        for anweisung in offen {
            let nonce = self.abrechnungsnonce;
            self.abrechnungsnonce += 1;
            let Ok(tx) = Transaktion::signiere(
                &crate::kette::Kette::startwert(),
                &schluessel,
                nonce,
                anweisung,
            ) else {
                continue;
            };
            let Ok(daten) = borsh::to_vec(&tx) else {
                continue;
            };
            if self.veroeffentliche(GossipTopic::Transactions, daten).await {
                verbreitet += 1;
            }
        }
        if verbreitet > 0 {
            self.protokoll.schreibe(
                Eintrag::neu("abrechnungen_verbreitet").zahl("anzahl", verbreitet as i64),
            );
        }
        verbreitet
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
            .zahl("zeilen", self.protokoll.geschrieben() as i64)
            // ⚑ Fund 63: Ohne diese beiden Zahlen wäre der Vorlauf
            // dieselbe Stille wie vorher, nur an anderer Stelle.
            .zahl("konsens_vorlauf", self.konsens_vorlauf.len() as i64)
            .zahl("konsens_vorlauf_verworfen", self.konsens_vorlauf_verworfen as i64)
            // Ein Schreibfehler macht den Block nicht ungültig, wohl
            // aber die Datei unvollständig. Das darf nicht still bleiben.
            .zahl("kette_schreibfehler", self.kette.schreibfehler() as i64)
            .zahl("kette_lesefehler", self.kette.lesefehler() as i64);
        if let Some(n) = self.kette.gespeicherte_bloecke() {
            eintrag = eintrag.zahl("kette_gespeichert", n as i64);
        }
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

        // ⚑ **Und die Kontraktabschrift für die eigene Tür** (B6-3).
        // Hier und nicht in der Blockschleife: Die Aufnahme ist die
        // Stelle, an der der Knoten seinen Zustand ohnehin ausliest,
        // und ein zweiter Abgleichpunkt wäre eine zweite Quelle für
        // dieselbe Abschrift.
        //
        // ⚑ **Damit ist sie so frisch wie die Aufnahme**, also im
        // Vorgabefall dreissig Sekunden. Ein Widerruf wirkt mit dieser
        // Verzögerung, und das gehört gesagt statt angenommen.
        self.kontraktabschrift.setzen(self.kette.zustand());

        // ⚑ **Und die Abrechnungen gehen mit.** Sie hier abzuholen und
        // nicht in einer eigenen Schleife hält die Zahl der Stellen
        // klein, an denen der Knoten die Kette anfasst. Was das kostet,
        // gehört gesagt: Eine Abrechnung wartet höchstens einen
        // Aufnahmetakt, im Vorgabefall dreissig Sekunden.
        self.abrechnungen_verbreiten().await;

        // ⚑ **Dieselben Zahlen, ein zweiter Weg** (Fund 129). Nicht
        // eine zweite Quelle: Der Stand entsteht aus derselben
        // Erhebung wie der Protokolleintrag, im selben Augenblick.
        // Zwei getrennte Erhebungen liefen auseinander, und dann sagte
        // das Protokoll etwas anderes als der Endpunkt.
        self.beobachtungsstelle.setzen(crate::beobachtung::Beobachtungsstand {
            stand_ms: crate::protokoll::jetzt_ms().max(0) as u64,
            hoehe: self.kette.hoehe(),
            hoechste_gehoerte: self.hoechste_gehoerte,
            peers: z.peers as u64,
            wartend: self.kette.wartend() as u64,
            schlecht_bewertet: z.schlecht_bewertet as u64,
            protokollzeilen: self.protokoll.geschrieben(),
            konsens_vorlauf: self.konsens_vorlauf.len() as u64,
            konsens_vorlauf_verworfen: self.konsens_vorlauf_verworfen,
            kette_schreibfehler: self.kette.schreibfehler(),
            kette_lesefehler: self.kette.lesefehler(),
            kette_gespeichert: self.kette.gespeicherte_bloecke().unwrap_or(0),
            latenz_messungen: anzahl,
            latenz_min_us: if anzahl > 0 { kleinste } else { 0 },
            latenz_max_us: groesste,
            nachforderung_laeuft: self.nachforderung_laeuft,
        });
    }

    /// Speist ein Netzereignis ein, als waere es ueber die Leitung
    /// gekommen.
    ///
    /// ⚑ **Nur fuer Tests**, und aus einem Grund, der sich nicht
    /// umgehen laesst: Manche Reihenfolgen sind ueber echte Sockets
    /// nicht herstellbar. Dass eine Inferenzantwort **waehrend** einer
    /// laufenden Blocknachforderung eintrifft, ist ein Rennen, das ein
    /// Test nicht verlaesslich gewinnt. Ohne diese Tuer bliebe die
    /// Zeile, die den Aufholzustand rettet, ungeprueft.
    #[doc(hidden)]
    pub fn ereignis_einspeisen(&mut self, ereignis: myl_net::NodeEvent) {
        self.vermerke(ereignis);
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
        // Diese Stelle fordert nur Blöcke nach; ein Inferenzauftrag
        // kommt nie von hier.
        let Nachforderung::Bloecke { ab, bis } = forderung else {
            return;
        };
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
                        // ⚑ Fund 64: Dieses Feld hieß bis zum 2026-08-26
                        // `art` und stand damit **ein zweites Mal** in
                        // einer Zeile, die `art` schon als feste Spalte
                        // trägt. Der Leser in `tests/zwei_knoten.rs`
                        // filtert nach `z.art == "..."` und hätte je nach
                        // Reihenfolge `block_abgelehnt` oder
                        // `passt-nicht-an` gesehen.
                        let ablehnungsart = match grund {
                            crate::kette::KettenFehler::SchonBekannt => "dublette",
                            // ⚑ Eigene Marke: Eine untragende Saatquelle
                            // ist ein Befund über den **Erzeuger**, kein
                            // Anschlussproblem, und sie darf keine
                            // Nachforderung auslösen.
                            crate::kette::KettenFehler::SaatquelleTraegtNicht => {
                                "saatquelle-traegt-nicht"
                            }
                            crate::kette::KettenFehler::PasstNichtAn { .. } => "passt-nicht-an",
                            crate::kette::KettenFehler::ZustandWeichtAb { .. } => {
                                "zustand-weicht-ab"
                            }
                            // **Eigene Marken, kein Sammelposten.** Eine
                            // falsche Höhe und eine falsche Epoche sind
                            // Befunde über den Absender, keine
                            // Anschlussprobleme; wer sie unter
                            // „passt-nicht-an" führte, löste damit auch
                            // noch eine Nachforderung aus.
                            crate::kette::KettenFehler::HoeheWeichtAb { .. } => "hoehe-weicht-ab",
                            crate::kette::KettenFehler::EpocheWeichtAb { .. } => "epoche-weicht-ab",
                        };
                        self.protokoll.schreibe(
                            Eintrag::neu("block_abgelehnt")
                                .zahl("eigene_hoehe", self.kette.hoehe() as i64)
                                .zahl("fremde_hoehe", block.header.height as i64)
                                .text("ablehnungsart", ablehnungsart)
                                .text("grund", grund.to_string()),
                        );
                        // Passt der Block nicht an und ist er **weiter**
                        // als wir, fehlt uns etwas. Dann fragen wir den,
                        // von dem der Hinweis kam: Er hat den Block, also
                        // hat er mit hoher Wahrscheinlichkeit auch die
                        // davor.
                        if ablehnungsart == "passt-nicht-an" {
                            self.fordere_nach(m.von, block.header.height);
                        }
                        // Auch eine abgelehnte Hoehe ist eine gehoerte
                        // Hoehe: Sie sagt, dass jemand weiter ist, und
                        // genau das entscheidet ueber die Bereitschaft.
                        self.hoechste_gehoerte =
                            self.hoechste_gehoerte.max(block.header.height);
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
            GossipTopic::Consensus => {
                let raus = self.nimm_konsens_an(&m.data);
                self.konsens_ausgang.extend(raus);
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
                let sofort = match Nachforderung::aus_bytes(&daten) {
                    Some(Nachforderung::Bloecke { ab, bis }) => {
                        let bloecke = self.kette.bloecke_von_bis(ab, bis);
                        let n = bloecke.len();
                        if n == 0 {
                            Some((Nachlieferung::Nichts, 0))
                        } else {
                            Some((Nachlieferung::Bloecke(bloecke), n))
                        }
                    }
                    // ⚑ **Ein Inferenzauftrag** (GATEWAY Stufe 4).
                    //
                    // Dieser Knoten rechnet nicht selbst, und das ist
                    // die Entscheidung vom 2026-09-03: **Ein Shard
                    // läuft in einem eigenen Prozess**, damit ein
                    // Absturz beim Rechnen den Konsens nicht anhält.
                    // Der Knoten reicht weiter, wenn er einen
                    // Anschluss hat.
                    //
                    // ⚑ **Und zwar in einer eigenen Aufgabe.** Der
                    // Shard rechnet Sekunden bis Minuten; wer hier
                    // wartet, hält die Blockverarbeitung genau so
                    // lange an. Die Antwort geht später über dieselbe
                    // Marke zurück.
                    //
                    // ⚑ **Ohne Anschluss wird abgelehnt und nicht
                    // geschwiegen.** Der Fragende soll „hier rechnet
                    // niemand" von „nicht angekommen" unterscheiden
                    // können; ein Auftrag ohne Antwort läuft in eine
                    // Zeitüberschreitung, die nichts bedeutet.
                    //
                    // ⚑ **Die Formprüfung steht nicht hier, sondern im
                    // Shard-Prozess** (Fund 154). Hier hätten beide
                    // Zweige dasselbe Ergebnis, dort liegt hinter dem
                    // einen ein Rechenwerk und hinter dem anderen
                    // nicht. Eine Prüfung gehört an die Naht, an der
                    // sie etwas unterscheidet.
                    Some(Nachforderung::Inferenz(auftrag)) => {
                        let sitzung = auftrag.sitzung;
                        match self.ortsanschluss.clone() {
                            Some(anschluss) => {
                                let sender = self.kommandos.clone();
                                tokio::spawn(async move {
                                    let antwort = match anschluss
                                        .frage(&myl_types::ortsleitung::Ortsfrage::Inferenz(
                                            auftrag,
                                        ))
                                        .await
                                    {
                                        Some(myl_types::ortsleitung::Ortsantwort::Inferenz(a)) => a,
                                        // Alles andere ist keine
                                        // Antwort auf diese Frage.
                                        _ => myl_types::inferenzauftrag::Inferenzantwort::Abgelehnt {
                                            sitzung,
                                        },
                                    };
                                    if let Some(bytes) = Nachlieferung::Inferenz(antwort).als_bytes()
                                    {
                                        let _ = sender
                                            .send(NodeCommand::Antwort { marke, daten: bytes });
                                    }
                                });
                                None
                            }
                            None => Some((
                                Nachlieferung::Inferenz(
                                    myl_types::inferenzauftrag::Inferenzantwort::Abgelehnt {
                                        sitzung,
                                    },
                                ),
                                0,
                            )),
                        }
                    }
                    // Unlesbare Anfrage: trotzdem antworten. Schweigen
                    // ließe den Fragenden auf eine Zeitüberschreitung
                    // warten, die ihm nichts sagt.
                    None => Some((Nachlieferung::Nichts, 0)),
                };
                // `None` heisst: Die Antwort geht später aus einer
                // eigenen Aufgabe, über dieselbe Marke.
                let Some((antwort, anzahl)) = sofort else {
                    return;
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
                let vorher_lief = self.nachforderung_laeuft;
                self.nachforderung_laeuft = false;
                let vorher = self.kette.hoehe();
                let mut angenommen = 0usize;
                let mut abgelehnt = 0usize;
                // ⚑ **Eine Inferenzantwort ist keine Blocklieferung**, und
                // sie darf die Nachforderung nicht als erledigt
                // buchen: Wer beides über dieselbe Schiene schickt,
                // muss beim Lesen wieder trennen, sonst hebt eine
                // Inferenzantwort die laufende Blocknachforderung auf.
                if let Some(Nachlieferung::Inferenz(antwort)) = Nachlieferung::aus_bytes(&daten) {
                    self.nachforderung_laeuft = vorher_lief;
                    let (art, sitzung) = match &antwort {
                        myl_types::inferenzauftrag::Inferenzantwort::Ergebnis {
                            sitzung, token, ..
                        } => (format!("{} Token", token.len()), *sitzung),
                        myl_types::inferenzauftrag::Inferenzantwort::Abgelehnt { sitzung } => {
                            ("abgelehnt".to_string(), *sitzung)
                        }
                    };
                    self.letzte_inferenzantwort = Some(antwort);
                    self.protokoll.schreibe(
                        Eintrag::neu("inferenz_antwort")
                            .text("von", von.to_string())
                            .zahl("sitzung", sitzung as i64)
                            .text("ergebnis", art),
                    );
                    return;
                }
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

/// Wartet auf das erste Beendigungssignal und nennt es beim Namen.
///
/// # ⚑ Warum SIGTERM dazugehoert (Fund 123, 2026-09-02)
///
/// Bis dahin stand hier allein `ctrl_c`, also **SIGINT**. Das ist das
/// Signal einer Tastatur. **Unter systemd, Docker und Kubernetes kommt
/// SIGTERM**, und der Prozess starb daran wortlos: kein Abschlusseintrag,
/// keine Zustandsaufnahme, nach der Schonfrist ein SIGKILL.
///
/// ⚑ **Damit fiel genau die Unterscheidung weg, fuer die der
/// Abschlusseintrag gebaut wurde.** Der Modulkopf von [`laufen_bis`]
/// sagt es selbst: „absichtlich beendet" liess sich von „abgestuerzt"
/// nicht unterscheiden, und fuer einen Lauf ueber mehrere Maschinen ist
/// das die erste Frage, wenn ein Protokoll kuerzer ist als die anderen.
/// Sie war fuer den Probelauf geloest und im **Betrieb** offen, also
/// dort, wo sie zaehlt.
///
/// **Beide Signale, und der Grund steht im Rueckgabewert:** Wer das
/// Protokoll liest, will wissen, ob ein Mensch abgebrochen hat oder ein
/// Dienstverwalter beendet.
async fn beendigungssignal() -> &'static str {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        // Schlaegt das Einhaengen fehl, bleibt SIGINT allein uebrig. Das
        // ist schlechter als beides und besser als ein Absturz beim
        // Start.
        let mut term = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
                return "Abbruchsignal";
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => "Abbruchsignal",
            _ = term.recv() => "Beendigungssignal",
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        "Abbruchsignal"
    }
}

/// Horcht ab dem Start auf Beendigungssignale.
///
/// # ⚑ Warum das eine eigene Wache ist (Fund 140, 2026-09-02)
///
/// Bis dahin haengte [`Knoten::laufen_bis`] den Signalhandler selbst
/// ein, und zwar in dem Augenblick, in dem es aufgerufen wurde. Davor
/// lagen **bis zu dreizehn Sekunden Vorlauf**: acht auf eine
/// QUIC-Adresse, fuenf auf irgendeine Horchadresse. In diesem Fenster
/// behandelte der Knoten kein Signal, und ein SIGTERM riss ihn hart
/// heraus, ohne Abschlusseintrag. Also genau das, wogegen Fund 123
/// gebaut wurde, nur an einer anderen Stelle.
///
/// **Gefunden hat es ein Test, der zweimal falsch rot war.** Er toetete
/// nach vier Sekunden, also mitten im Vorlauf, und meldete einen
/// Fehler, den es nicht gab. Der Fehler war stattdessen nebenan.
///
/// **Die Wache wird gestellt, sobald der Knoten existiert.** Ab da ist
/// jedes Signal aufgehoben, auch wenn niemand gerade darauf wartet: Der
/// [`tokio::sync::watch`]-Kanal haelt den Grund fest, und wer spaeter
/// hinzukommt, sieht ihn sofort. Ein `Notify` koennte das nicht, denn
/// ein Weckruf ohne Wartenden ist verloren.
#[derive(Debug, Clone)]
pub struct Beendigungswache {
    empfaenger: tokio::sync::watch::Receiver<Option<&'static str>>,
}

impl Beendigungswache {
    /// Stellt die Wache. Verlangt eine laufende Tokio-Laufzeit.
    pub fn stellen() -> Self {
        let (sender, empfaenger) = tokio::sync::watch::channel(None);
        tokio::spawn(async move {
            let grund = beendigungssignal().await;
            // Schlaegt das Senden fehl, hoert niemand mehr zu, und dann
            // ist auch nichts mehr zu tun.
            let _ = sender.send(Some(grund));
        });
        Self { empfaenger }
    }

    /// Wartet, bis ein Signal kam, und nennt es beim Namen.
    ///
    /// **Kehrt sofort zurueck, wenn das Signal schon da war.** Das ist
    /// der Punkt der ganzen Uebung: Ein Signal aus dem Vorlauf geht
    /// nicht verloren.
    pub async fn warten(&self) -> &'static str {
        let mut empfaenger = self.empfaenger.clone();
        loop {
            if let Some(grund) = *empfaenger.borrow_and_update() {
                return grund;
            }
            if empfaenger.changed().await.is_err() {
                // Der Sender ist fort, ohne gesendet zu haben. Kann
                // nicht vorkommen, denn die Aufgabe sendet, bevor sie
                // endet. Falls doch: **ewig warten**, nicht sofort
                // enden. Ein falsches „beendet" waere schlimmer als ein
                // Knoten, der weiterlaeuft.
                std::future::pending::<()>().await;
            }
        }
    }
}
