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
    ist_vermittelt, run_node_mit, GossipTopic, NodeCommand, NodeEvent, NodeIdentity,
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

/// ⚑ **Höchstens so viel sammelt ein zustandsloser Knoten an Zeugenstücken**
/// (zustandslose Validierung, 2026-10-29).
const ZEUGEN_SAMMLER_BYTES: usize = 128 << 20;
/// Wie weit ein Block vor der eigenen Höhe auf seinen Zeugen warten darf.
/// 📌 Erst 32: Beim Aufholen kam ein zustandsloser Knoten im Podlauf mit
/// rund 11 Blöcken je Sekunde nicht an eine Spitze, die mit 16 wuchs.
const WARTENDE_BLOECKE: u64 = 128;
/// Wie viele fertige Zeugen auf ihren Block warten dürfen (dazu die
/// Bytegrenze [`ZEUGEN_SAMMLER_BYTES`]).
const FERTIGE_ZEUGEN: usize = WARTENDE_BLOECKE as usize;
/// So viele weitere Gegenstellen fragt ein zustandsloser Knoten nach Zeugen.
const ZEUGEN_GEGENSTELLEN: usize = 4;
/// So lange sucht ein zustandsloser Neuling einen verankerten leichten Abzug.
const ANKER_SUCHEN_HOECHSTENS: std::time::Duration = std::time::Duration::from_secs(10);
/// So weit voraus hält ein zustandsloser Knoten Kontobeweise.
const KONTOBEWEISE_VORAUS: u64 = 2 * WARTENDE_BLOECKE;
/// ⚑ **So lange sammelt ein zustandsloser Knoten Anker** (Zertifikate von
/// mehreren Gegenstellen) und nimmt dann den jüngsten: Ein gültiges, aber
/// altes Zertifikat einer einzelnen Gegenstelle führte sonst zu einem Abzug,
/// den niemand mehr hat, oder zu einem Abstand, für den niemand Zeugen hält.
const ANKER_SAMMELN: std::time::Duration = std::time::Duration::from_secs(2);
/// ⚑ **Selbstheilung**: Steht ein zustandsloser Knoten so lange still,
/// obwohl höhere Blöcke gehört wurden (etwa weil ein Erzeuger Zeugen
/// zurückhält), sucht er einen jüngeren verankerten leichten Abzug.
const STILLSTAND_HOECHSTENS: std::time::Duration = std::time::Duration::from_secs(30);
/// So oft fragt ein zustandsloser Knoten nach Kontobeweisen.
const KONTOBEWEISE_ALLE: std::time::Duration = std::time::Duration::from_secs(2);
/// ⚑ **Der Zweck einer Zeitprobe** ([`NodeCommand::AnfrageMitZweck`],
/// 2026-10-30): Nur ein `Nichts` oder ein Fehlschlag mit diesem Zweck zählt
/// als Abweisung der Probe.
const ZWECK_ZEITPROBE: u64 = 1;
/// Ab so vielen Neuverankerungen hintereinander ohne einen Block mit
/// Zeugen meldet der zustandslose Knoten, dass er nur noch springt.
const NUR_ABZUG_WARNUNG: u32 = 3;
/// So viele Unterzeichner untragender Zeugen merkt sich ein Knoten.
const ZEUGEN_GESPERRT: usize = 64;
/// Ab wann ein fehlender Zeuge nachgefragt wird.
const ZEUGE_NACHFRAGEN_NACH: std::time::Duration = std::time::Duration::from_secs(1);

/// Höchstens `n` Gegenstellen ab `ab`, reihum, und wo die nächste Runde
/// beginnt. Nach `ceil(len / n)` Runden war jede einmal dabei.
fn reihum(gegenstellen: &[libp2p::PeerId], ab: usize, n: usize) -> (Vec<libp2p::PeerId>, usize) {
    let len = gegenstellen.len();
    if len == 0 {
        return (Vec::new(), 0);
    }
    let ab = ab % len;
    let auswahl = (0..n.min(len)).map(|i| gegenstellen[(ab + i) % len]).collect();
    (auswahl, (ab + n) % len)
}

/// Führt die Erzeugerliste des Netzfadens aus dem Stimmsatz der Kette nach.
/// Ohne Stimmsatz (eine Kette, die ihren Erzeuger erst lernt) bleibt sie,
/// wie sie ist; siehe `Knoten::zustandslos_block`.
fn erzeugerliste_aus(kette: &crate::kette::Kette, liste: &crate::validator::Erzeugerliste) {
    let Some(satz) = kette.stimmsatz() else {
        return;
    };
    let neu: std::collections::BTreeMap<myl_types::MinerId, myl_types::bls::BlsPublicKey> =
        satz.members().map(|(id, m)| (*id, m.pubkey)).collect();
    if liste.read().is_ok_and(|l| *l == neu) {
        return;
    }
    if let Ok(mut l) = liste.write() {
        *l = neu;
    }
}

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
    /// Kontoanfragen vom Beobachtungsendpunkt (`/konto/<hex>`).
    kontoanfragen: mpsc::Receiver<crate::beobachtung::Kontoanfrage>,
    /// Fragen von der Relaisleitung.
    relaisauftraege: mpsc::Receiver<crate::relais::Relaisauftrag>,
    /// ⚑ Der zuletzt gesetzte Vorrang in den Verbindungsgrenzen (Fund 674):
    /// Epoche und Gegenstellen.
    vorrang: Option<(u64, Vec<libp2p::PeerId>)>,
    /// Hält den Kanal offen, auch ohne Leitung.
    _relais_tx: mpsc::Sender<crate::relais::Relaisauftrag>,
    /// Weitergereichte Shardanfragen, die auf ihre Antwort warten: an wen,
    /// der Rückweg zur Leitung, seit wann.
    shardrelais: std::collections::HashMap<
        u64,
        (libp2p::PeerId, std::sync::mpsc::Sender<myl_types::relais::Relaisantwort>, std::time::Instant),
    >,
    /// Die nächste Kennung einer weitergereichten Shardanfrage.
    relais_kennung: u64,
    /// Plätze für Shardanfragen fremder Koordinatoren an den eigenen Shard.
    shard_plaetze: Plaetze,
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
    /// **Höchstens [`NACHHOLER`] zugleich, je Gegenstelle eine, über
    /// aufeinanderfolgende Bereiche** (seit dem 2026-10-22). Ohne Deckel
    /// schickte ein Neuling für jeden abgelehnten Block eine neue Anfrage;
    /// bei einem Rückstand von zwanzig Blöcken wären das zwanzig Anfragen
    /// für dieselbe Lücke. Bis zum 2026-10-22 lief nur eine: Über eine
    /// Leitung mit 200 ms und 5 % Verlust kamen so rund 75 Blöcke je
    /// Sekunde an, denn eine einzelne Verbindung bricht unter Verlust ein.
    /// Mehrere Verbindungen addieren sich; was vor seiner Zeit ankommt, nimmt
    /// der Vorausspeicher (Fund 592).
    nachforderungen: Vec<Nachforderung1>,
    /// ⚑ **Wer zuletzt nichts hatte** (Fund 626): ab welcher Höhe und seit
    /// wann. Eine solche Gegenstelle wird für Bereiche ab dieser Höhe
    /// [`LEER_SPERRE`] lang nicht gefragt.
    leer_geantwortet: std::collections::HashMap<libp2p::PeerId, (u64, std::time::Instant)>,
    /// Die eigene Höhe und seit wann sie steht, für die Nachfrage bei Stille
    /// (Fund 618).
    zuletzt_gewachsen: (u64, std::time::Instant),
    /// Die Gegenstelle der laufenden Nachfrage bei Stille.
    stille_bei: Option<libp2p::PeerId>,
    /// Die zuletzt gemeldete eigene Fristsperre (Epoche), damit sie einmal
    /// im Protokoll steht und nicht je Aufnahme.
    fristsperre_gemeldet: Option<u64>,
    /// Wie oft der Probehahn schon gezapft wurde; jeder neue Versuch beginnt
    /// beim nächsten Probekonto (G1).
    probehahn_versuche: u8,
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
    ///
    /// 📌 **Fund 648 (2026-10-26):** Hier stand „bei jedem Block“, und
    /// aufgefrischt wurde nur bei der Aufnahme, im Vorgabefall alle
    /// dreissig Sekunden. Eine eben eröffnete Sitzung bekam so bis dahin
    /// 401. Jetzt zieht [`Self::abschrift_nachziehen`] nach jedem neuen
    /// Block nach.
    kontraktabschrift: crate::tuer::Kontraktabschrift,
    /// Die Höhe, auf der die Abschrift zuletzt aufgefrischt wurde.
    abschrift_hoehe: Option<u64>,
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
    /// Inferenzauftraege aus dem Netz: beantwortet und abgelehnt.
    ///
    /// ⚑ **Geteilt, weil die Antwort in einer eigenen Aufgabe entsteht**
    /// und erst dort feststeht, ob der Shard rechnete oder ablehnte.
    inferenz_zaehler: Arc<Inferenzzaehler>,
    /// Plaetze fuer Inferenzauftraege aus dem Netz, die beim Shard liegen
    /// (Fund 609, siehe [`INFERENZ_IN_ARBEIT_HOECHSTENS`]).
    inferenz_plaetze: Plaetze,
    /// Die Drossel fuer teure Anfragen (Fund 632, [`crate::drossel`]) und
    /// wie viele sie seit der letzten Zustandsaufnahme abgewiesen hat.
    drossel: crate::drossel::Drossel<libp2p::PeerId>,
    gedrosselt: u64,
    /// Ein Platz fuer eine Zeitprobe zugleich (Fund 609).
    zeitprobe_platz: Plaetze,
    /// Wie viele Bytes die unterschriebene Zusage nennt; null ohne.
    zugesagt_bytes: u64,
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
    /// Der Schlüssel, mit dem dieser Knoten Kettentransaktionen
    /// unterschreibt (Fund 170), falls einer aus einer Datei kam.
    ///
    /// ⚑ **`None` heisst: der abgeleitete Probeschlüssel.** Der ist aus
    /// dem Knotennamen nachzurechnen, also kein Geheimnis; für einen
    /// Probelauf gewollt, für ein Netz unbrauchbar.
    kontoschluessel: Option<crate::schluessel::Konsensschluessel>,
    /// Die hoechste Hoehe, von der dieser Knoten gehoert hat.
    ///
    /// **Grundlage der Bereitschaftsauskunft.** Wer eine hoehere Hoehe
    /// kennt als seine eigene, ist im Rueckstand und sollte keinen
    /// Verkehr bekommen; ohne diese Zahl waere „bereit" nicht von „hat
    /// noch nichts gehoert" zu unterscheiden.
    hoechste_gehoerte: u64,
    /// ⚑ **Woher sie kommt** (Fund 634): je Gegenstelle die behauptete Höhe,
    /// widerlegbar durch eine leere Nachlieferung, verfallend mit der
    /// Trennung. `hoechste_gehoerte` ist die höchste geltende.
    gehoert: crate::gehoert::Gehoert<libp2p::PeerId>,
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
    /// ⚑ **Stufe 4 des Dispatchers** (2026-10-22): der Schlüssel, unter dem
    /// dieser Knoten als Miner angemeldet ist (sein Konsensschlüssel), oder
    /// `None`, wenn er nicht rechnet und nicht zusagt.
    netzschluessel: Option<crate::schluessel::Konsensschluessel>,
    /// Der Schlüssel des bisher eingetragenen Auszahlungskontos, falls ein
    /// Wechsel ansteht (`--kontowechsel`).
    kontowechsel: Option<crate::schluessel::Konsensschluessel>,
    /// Das zuletzt gemeldete Auszahlungskonto der Kette, damit die Zeile
    /// `auszahlungskonto` einmal je Änderung erscheint.
    auszahlung_gemeldet: Option<Option<myl_types::ids::Address>>,
    /// Die eigene Netzadresse, wie sie in der Anmeldung steht: Unter ihr
    /// finden andere Mitglieder diesen Knoten für Zeitprobe und Dateien.
    netzadresse: myl_types::latency_attest::PeerIdBytes,
    /// Die nächste Nummer für Transaktionen des Netzschlüssels, mindestens
    /// die des Kontos im Zustand.
    netz_nonce: u64,
    /// Die Nummer des Kontos im Zustand und die Höhe, ab der ihr Stillstand
    /// zählt, während eigene Transaktionen ausstehen (Funde 627 und 628).
    netz_stand: (u64, u64),
    /// Was zuletzt eingereicht wurde, ab welcher Höhe die Frist zählt: gegen
    /// Wiederholung in jeder Aufnahme, solange der Block noch aussteht.
    netz_gesendet: std::collections::BTreeMap<&'static str, u64>,
    /// Der Shard aus der Kette (P7).
    shardfuehrung: Option<crate::shardfuehrung::Shardfuehrung>,
    /// ⚑ **Stufe 2, Nachrechnen** (T6c, W6b, 2026-10-29): die Ziehungen,
    /// für die dieser Knoten Checker ist und den Verlauf schon erfragt hat
    /// (Epoche, Pod, Index), und die, deren Verlauf schon nachgerechnet wird.
    verlauf_gefragt: std::collections::BTreeSet<(u64, myl_types::ids::PodId, u32)>,
    nachgerechnet: std::collections::BTreeSet<(u64, myl_types::ids::PodId, u32)>,
    /// Festgelegt und noch nicht aufgedeckt: je Ziehung Befund und Salz.
    festgelegt: std::collections::BTreeMap<(u64, myl_types::ids::PodId, u32), (myl_ledger::pruefung::Pruefbefund, [u8; 32])>,
    /// Festlegungen, die noch in eine Transaktion müssen.
    festzulegen: Vec<(u64, myl_types::ids::PodId, u32, myl_types::Hash)>,
    /// Als Anfechter: je Ziehung der Schiedsbeleg (Verlauf, Eingänge).
    schiedsbelege: std::collections::BTreeMap<(u64, myl_types::ids::PodId, u32), Schiedsbeleg>,
    /// Als Validator: je Streitfall, seit wann der Beleg angefragt ist.
    schied_angefragt: std::collections::BTreeMap<(u64, myl_types::ids::PodId, u32), std::time::Instant>,
    /// ⚑ **Zustandslos** (zustandslose Validierung, 2026-10-29): die Schlüssel
    /// der Blockerzeuger, gegen die der Netzfaden Zeugenstücke prüft.
    erzeugerliste: crate::validator::Erzeugerliste,
    /// Dieselbe Prüfung für nachgeforderte Stücke, die nicht durch den
    /// Netzfaden kommen.
    zeugenpruefer: crate::validator::ProtokollValidator,
    /// Setzt die Zeugen aus geprüften Stücken zusammen.
    zeugensammler: myl_consensus::zeuge::Zeugensammler,
    /// Blöcke, deren Erzeuger belegt ist, je Höhe, bis ihr Zeuge da ist:
    /// mit Ankunft und Weiterreicher.
    wartet_auf_zeugen: std::collections::BTreeMap<u64, (myl_consensus::block::Block, std::time::Instant, libp2p::PeerId)>,
    /// Höhen, deren Block nachgefordert kam: Sein Zeuge ist im Gossip längst
    /// vorbei, er wird sofort nachgefragt.
    zeugen_eilig: std::collections::BTreeSet<u64>,
    /// Zusammengesetzte Zeugen, je Blockhash, bis ihr Block dran ist.
    fertige_zeugen: std::collections::VecDeque<(myl_types::Hash, myl_ledger::state::Zeuge, usize, myl_types::bls::BlsPublicKey)>,
    /// ⚑ **Unterzeichner, deren Zeuge nicht trug** (2026-10-30): Von ihnen
    /// nimmt der Knoten keine direkt gelieferten Stücke mehr an. Begrenzt
    /// auf [`ZEUGEN_GESPERRT`].
    zeugen_gesperrt: std::collections::VecDeque<myl_types::bls::BlsPublicKey>,
    /// Je (Blockhash, Stück), seit wann nachgefragt.
    zeugen_angefragt: std::collections::BTreeMap<(myl_types::Hash, u16), std::time::Instant>,
    /// Wer zuletzt tragende Zeugenstücke brachte (Gossip oder Nachlieferung):
    /// Er wird bei jeder Nachfrage mitgefragt. 📌 Erster Podlauf: Die Blöcke
    /// kamen über Nachforderungen von vollen Minern, die keine Zeugen haben,
    /// und der Erzeuger stand nicht unter den gefragten Gegenstellen.
    zeugen_quellen: std::collections::VecDeque<libp2p::PeerId>,
    /// ⚑ **Reihum durch die Gegenstellen** (2026-10-30): wo die nächste
    /// Zeugennachfrage weitere Gegenstellen beginnt. 📌 Langer Podlauf: Es
    /// waren immer dieselben ersten vier, unter ihnen keiner der vollen
    /// Miner, die Zeugen bereithielten; in die Quellen kommt aber nur, wer
    /// schon geliefert hat. Nach einer Trennung lief der Erzeuger allein in
    /// Zeitüberschreitungen, und der zustandslose Knoten kam nur über
    /// leichte Abzüge weiter.
    zeugen_reihum: usize,
    /// ⚑ **Neuverankerungen ohne einen Block mit Zeugen dazwischen**
    /// (2026-10-30). Ab [`NUR_ABZUG_WARNUNG`] meldet der Knoten, dass er
    /// nur noch springt und nichts prüft. 📌 Im langen Podlauf ging das eine
    /// Stunde lang still.
    nur_abzug: u32,
    /// Seit wann ein zustandsloser Neuling einen Anker für einen leichten
    /// Abzug sucht (über ein Prüfpunktzertifikat); bis
    /// [`ANKER_SUCHEN_HOECHSTENS`], dann folgt er ab Genesis.
    anker_suchen: Option<std::time::Instant>,
    /// Der höchste geprüfte Anker dieser Suche, und seit wann es einen gibt:
    /// Gesammelt wird [`ANKER_SAMMELN`] lang, genommen der jüngste.
    anker_kandidat: Option<(crate::abzug::Anker, std::time::Instant)>,
    /// Zustandslos: die Höhe und seit wann sie steht (für die Selbstheilung).
    stillstand: (u64, std::time::Instant),
    /// Zustandslos: der höchste Block mit belegtem Erzeuger, der ankam.
    hoechster_belegter: u64,
    /// Für Tests: so lange Stillstand, bis neu verankert wird.
    stillstand_hoechstens: std::time::Duration,
    /// Für Tests: Der Erzeuger hält seine Zeugen zurück (zerlegt und
    /// verbreitet sie nicht, liefert sie nicht nach), wie ein böswilliger.
    zeugen_zurueckhalten: bool,
    /// Kontobeweise einer höheren Höhe, bis die eigene sie erreicht.
    kontobeweise_offen: Option<(u64, Vec<myl_ledger::state::Kontobeweis>)>,
    /// Wann zuletzt Kontobeweise angefragt wurden.
    kontobeweise_angefragt: Option<std::time::Instant>,
    /// In welcher Epoche die Grössen zuletzt im Protokoll standen.
    groessen_epoche: Option<u64>,
    /// Die Zähler der Transaktionen der laufenden Sekunde.
    tx_zaehler: TxZaehler,
    /// Transaktionen aus dem Gossip, die auf die Stapelprüfung warten.
    tx_puffer: Vec<myl_consensus::block::Transaktion>,
    tx_puffer_seit: std::time::Instant,
    tx_geprueft: tokio::sync::mpsc::UnboundedReceiver<Vec<myl_consensus::block::GepruefteTransaktion>>,
    tx_geprueft_kanal: tokio::sync::mpsc::UnboundedSender<Vec<myl_consensus::block::GepruefteTransaktion>>,
    relais_geprueft: tokio::sync::mpsc::UnboundedReceiver<RelaisGeprueft>,
    relais_geprueft_kanal: tokio::sync::mpsc::UnboundedSender<RelaisGeprueft>,
    /// Die Selbstprobe: in welcher Epoche zuletzt, ihre Ergebnisse (Epoche,
    /// je Token, Frist), und die daraus angepasste Ebenenzeit.
    selbstprobe_epoche: Option<u64>,
    selbstproben: tokio::sync::mpsc::UnboundedReceiver<(u64, u64, u64)>,
    selbstprobenkanal: tokio::sync::mpsc::UnboundedSender<(u64, u64, u64)>,
    ebenenzeit_angepasst: u64,
    /// Als Validator: Streitfälle, zu denen gestimmt ist.
    schied_gestimmt: std::collections::BTreeSet<(u64, myl_types::ids::PodId, u32)>,
    schiedsrechnungen: tokio::sync::mpsc::UnboundedReceiver<Schiedsrechnung>,
    schiedskanal: tokio::sync::mpsc::UnboundedSender<Schiedsrechnung>,
    /// Das Modell des Checkers, beim ersten Nachrechnen geladen und dann
    /// gehalten: über `myl_verifier`, nicht über den Code des Pods.
    pruefmodell: Arc<std::sync::Mutex<Option<Arc<integer_llm_runtime::model::IntegerModel>>>>,
    /// Was nachgerechnet wurde, aus der Rechenaufgabe zurück in die Schleife.
    nachrechnungen: tokio::sync::mpsc::UnboundedReceiver<Nachrechnung>,
    nachrechnungskanal: tokio::sync::mpsc::UnboundedSender<Nachrechnung>,
    /// ⚑ **Eine übernommene Position** (Whitepaper 6.8): Als Reserve rechnet
    /// dieser Knoten sie bis zum Ende ihrer Epoche.
    uebernahme: Option<crate::shardfuehrung::Shardauftrag>,
    /// ⚑ **Abgelegte Spuren** für die Stichprobe (Stufe 2), höchstens
    /// [`SPURABLAGEN_HOECHSTENS`], die älteste fällt.
    spurablagen: Vec<myl_types::Spurablage>,
    /// Die Zeugnisse über Fristen (6.8).
    fristsammlung: crate::frist::Fristsammlung,
    /// Laufende Messungen je Gegenüber.
    messungen: std::collections::BTreeMap<libp2p::PeerId, Messung>,
    /// Wer in welcher Epoche schon gemessen ist: Wer noch fehlt, wird bei
    /// der nächsten Aufnahme wieder gefragt (ein Shard, der gerade lädt,
    /// antwortet erst später).
    gemessen: std::collections::BTreeSet<(u64, myl_types::ids::MinerId)>,
    /// Die erste Überschreitung je Epoche und Mitglied, mit Höhe und Wert
    /// (Fund 655: erst eine zweite in einem getrennten Fenster bezeugt).
    fristverdacht: std::collections::BTreeMap<(u64, myl_types::ids::MinerId), (u64, u64)>,
    /// Gesammelte Prüfpunktstimmen je Höhe und Wähler (2026-10-27,
    /// CONSENSUS 7.2), nur über dem bestätigten Stand.
    pruefpunktstimmen: std::collections::BTreeMap<
        u64,
        std::collections::BTreeMap<myl_types::ids::MinerId, myl_consensus::pruefpunkt::Pruefpunktstimme>,
    >,
    /// Über welchen Prüfpunkt dieser Knoten zuletzt gestimmt hat, und wann
    /// er die Stimme zuletzt verbreitet hat (sie wird wiederholt, bis der
    /// Prüfpunkt bestätigt ist).
    pruefpunkt_gestimmt: (u64, Option<std::time::Instant>),
    /// Welche Doppelbelege dieser Knoten schon gemeldet hat (CONSENSUS 7.6).
    doppelbelege_gemeldet: std::collections::BTreeSet<(myl_types::ids::Address, u8, u64)>,
    /// Das Holen fehlender Dateien für die nächste Epoche (P11).
    nachholen: Option<Nachholen>,
    /// Transaktionen aus Ereignissen, die bei der nächsten Aufnahme
    /// verbreitet werden.
    ausstehend: Vec<myl_consensus::block::Transaktion>,
    /// Wann die nächste Zustandsaufnahme fällig ist, über die Aufrufe von
    /// [`Knoten::laufe_fuer`] hinweg.
    naechste_aufnahme: Option<tokio::time::Instant>,
    /// Wann der nächste Block des Blocktakts fällig ist, ebenso über die
    /// Aufrufe hinweg.
    naechster_block: Option<tokio::time::Instant>,
    /// Die Höhe beim vorigen Testverkehr; mit Blocktakt ist die Probe der
    /// Blockkette, dass die Höhe seither gewachsen ist.
    hoehe_beim_testverkehr: u64,
    /// Blöcke, die zu früh kamen (Fund 592).
    voraus: crate::voraus::Vorausspeicher,
    /// Die nächste Nonce des Testverkehrs und die Höhe, bei der die älteste
    /// noch nicht verbuchte gesendet wurde (Fund 593).
    testverkehr_nonce: (u64, u64),
    /// Der laufende Abruf eines Zustandsabzugs (2026-10-22).
    abzug_holen: Option<AbzugHolen>,
    /// Die laufende Hintergrundprüfung eines Abzugs.
    hintergrund: Option<Hintergrund>,
    /// Das laufende Nachladen der Geschichte einer neu gezogenen Kopie.
    abruf: Option<Abruf>,
    /// Das laufende Holen der Teile eines Splitters.
    teilabruf: Option<Teilabruf>,
    /// Der zuletzt im Protokoll gemeldete eigene Abzug.
    abzug_gemeldet: Option<u64>,
    /// ⚑ Ein Grund, aus dem dieser Knoten von sich aus anhält (eine
    /// widerlegte Basis). Die Wache kennt nur Signale von aussen.
    angehalten: Option<&'static str>,
    /// Die verbundenen Gegenstellen, für das Nachfordern reihum (Fund 588).
    gegenstellen: Vec<libp2p::PeerId>,
    /// Wer beim nächsten Nachfordern ohne Vorzug gefragt wird.
    nachfrage_reihum: usize,
}

/// So viele Nachforderungen von Blöcken laufen höchstens zugleich.
pub const NACHHOLER: usize = 4;

/// So viele Inferenzauftraege aus dem Netz liegen hoechstens zugleich beim
/// Shard; jeder weitere wird sofort abgelehnt.
///
/// 📌 **Fund 609 (2026-10-23):** Jeder Auftrag startete eine eigene Aufgabe,
/// die bis zur Frist des Ortsanschlusses lebt, auch wenn die Anfrage im Netz
/// nach zehn Sekunden schon abgelaufen ist und ihr Platz frei wird. Waehrend
/// eine lange Rechnung die Tuer des Shards belegt (sie bedient nacheinander),
/// sammelten sich so je Verbindung rund siebzig Auftraege mit je bis zu 4 MiB.
/// Ein ehrlicher Knoten fuehrt einen Auftrag zugleich (siehe
/// `letzte_inferenzantwort`); vier lassen Luft.
pub const INFERENZ_IN_ARBEIT_HOECHSTENS: usize = 4;

/// **Eine feste Zahl von Plaetzen** fuer Arbeit, die eine Anfrage aus dem
/// Netz ausloest und die sie ueberdauern kann (Fund 609).
///
/// ⚑ **Der Platz geht mit seinem [`Platz`] zurueck, beim Freigeben**, auch
/// wenn die Arbeit mit einer Panik endet. Ein Zaehler, den jeder Pfad von
/// Hand zuruecksetzt, vergisst es an einem davon, und dann ist der Dienst
/// fuer immer zu.
#[derive(Debug, Clone)]
pub struct Plaetze {
    belegt: Arc<std::sync::atomic::AtomicUsize>,
    hoechstens: usize,
}

/// Ein belegter Platz; gibt ihn beim Freigeben zurueck.
#[derive(Debug)]
pub struct Platz(Arc<std::sync::atomic::AtomicUsize>);

impl Plaetze {
    /// `hoechstens` Plaetze, alle frei.
    pub fn neu(hoechstens: usize) -> Self {
        Self { belegt: Arc::new(std::sync::atomic::AtomicUsize::new(0)), hoechstens }
    }

    /// Ein freier Platz, oder `None`, wenn alle belegt sind.
    pub fn nehmen(&self) -> Option<Platz> {
        use std::sync::atomic::Ordering;
        self.belegt
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| (n < self.hoechstens).then_some(n + 1))
            .ok()
            .map(|_| Platz(Arc::clone(&self.belegt)))
    }
}

impl Drop for Platz {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

/// Was die Schleife des Knotens geweckt hat.
enum Weckruf {
    Ereignis(Option<NodeEvent>),
    Konto(crate::beobachtung::Kontoanfrage),
    Relais(crate::relais::Relaisauftrag),
    /// Ein geprüfter Stapel Transaktionen aus dem Gossip.
    Geprueft(Vec<myl_consensus::block::GepruefteTransaktion>),
    /// Ein geprüfter Stapel von der Relaisleitung, mit der Antwort.
    RelaisGeprueft(RelaisGeprueft),
    Frist,
}

/// Eine laufende Nachforderung: bei wem, welcher Bereich, seit wann.
#[derive(Debug, Clone, Copy)]
struct Nachforderung1 {
    an: libp2p::PeerId,
    ab: u64,
    bis: u64,
    seit: std::time::Instant,
}

/// Eine laufende Fristmessung: fünf Zeitproben, die kürzeste zählt (Fund 646).
#[derive(Debug, Clone)]
struct Messung {
    pod: u32,
    mitglied: myl_types::ids::MinerId,
    ab: std::time::Instant,
    /// Wie viele Positionen jede Probe rechnet (aus der Frist, Fund 646).
    positionen: u32,
    /// Die Frist des Mitglieds je Token, beim Stellen gelesen.
    frist: u64,
    /// Die kürzeste Zeitprobe bisher (Fund 639).
    probe_us: Option<u64>,
    proben: u32,
    /// Die Saat der laufenden Probe (Fund 669), über die die Antwort
    /// unterschrieben sein muss.
    saat: [u8; 32],
    /// Die Epoche, in der die Messung gestellt wurde; nach dem Wechsel gilt
    /// sie nicht mehr (Fund 670).
    epoche: u64,
    /// Wie viele Mitglieder des Pods den Gemessenen messen; daraus folgt,
    /// wie lange eine Verweigerung dauern muss (Fund 660).
    messende: usize,
    /// Abweisungen in Folge seit der letzten Antwort und seit wann (Fund
    /// 660), dazu alle Abweisungen dieser Messung für das Protokoll.
    abgewiesen_in_folge: u32,
    abgewiesen_seit: Option<std::time::Instant>,
    abgewiesen: u32,
    /// Nach einer Abweisung: ab wann wieder gefragt wird.
    wieder_ab: Option<std::time::Instant>,
}

/// Was das Nachspielen der Kettendatei beim Start ergab.
#[derive(Debug, Default)]
struct Nachgespielt {
    uebernommen: usize,
    abgelehnt: usize,
    erster_grund: String,
    lesefehler: Option<String>,
    /// Bis zu diesem eigenen Abzug nur auf Anschluss geprüft (Fund 629).
    abzug: Option<u64>,
    vorgespult: usize,
    /// Warum der eigene Abzug nicht trug; dann wird ab der Basis gerechnet.
    verworfen: Option<String>,
}

/// **Die Kettendatei nachspielen**, mit `einsatz` ab dem jüngsten eigenen
/// Abzug (Fund 629): bis zu ihm vorspulen, dann den Abzug einsetzen, danach
/// übernehmen. Scheitert eines davon, steht der Grund in `verworfen`, und
/// die Kette ist unbrauchbar; der Aufrufer beginnt dann neu ohne Einsatz.
///
/// ⚑ **Satz für Satz, nicht alle auf einmal** (Fund 124). Bis zum
/// 2026-09-02 stand die ganze Kette im Arbeitsspeicher, bevor der erste
/// Block geprüft war. Jetzt liegt immer nur einer da.
///
/// Ein Lesefehler mitten im Nachspielen bricht ab: Was danach kommt,
/// schlösse an einen Block an, den dieser Knoten nicht hat, und würde
/// ohnehin abgewiesen. Der Grund geht ins Protokoll.
fn nachspielen(
    kette: &mut Kette,
    speicher: &mut crate::speicher::Kettenspeicher,
    mut einsatz: Option<(crate::abzug::Abzug, myl_ledger::state::LedgerState)>,
) -> Nachgespielt {
    let mut n = Nachgespielt::default();
    let ziel = einsatz.as_ref().map(|(a, _)| a.hoehe());
    let lesefehler = speicher
        .fuer_jeden_satz(|b| {
            if n.verworfen.is_some() {
                return;
            }
            let h = b.header.height;
            if let Some(z) = ziel.filter(|&z| einsatz.is_some() && h > kette.hoehe() && h <= z) {
                if let Err(e) = kette.vorspulen(&b) {
                    n.verworfen = Some(format!("bei {h}: {e}"));
                    return;
                }
                n.vorgespult += 1;
                if h == z {
                    let (abzug, zustand) = einsatz.take().expect("geprüft");
                    match kette.abzug_einsetzen(&abzug, zustand) {
                        Ok(()) => n.abzug = Some(z),
                        Err(e) => n.verworfen = Some(e),
                    }
                }
                return;
            }
            match kette.uebernimm(&b) {
                Ok(()) => n.uebernommen += 1,
                Err(e) => {
                    n.abgelehnt += 1;
                    if n.erster_grund.is_empty() {
                        n.erster_grund = e.to_string();
                    }
                }
            }
        })
        .err();
    n.lesefehler = lesefehler.map(|e| e.to_string());
    if let (Some(z), Some(_), None) = (ziel, &einsatz, &n.verworfen) {
        n.verworfen = Some(format!("die Datei endet vor dem Abzug bei {z}"));
    }
    n
}

/// Das Holen fehlender Dateien, mit seinen Quellen.
#[derive(Debug)]
struct Nachholen {
    holer: crate::dateidienst::Nachholer,
    quellen: Vec<libp2p::PeerId>,
    quelle: usize,
    laeuft: bool,
}

impl Knoten {
    /// ⚑ **Eine Kette, die aus einem Abzug begann, beginnt wieder dort**
    /// (2026-10-22): Die Kettendatei hält dann nur die Blöcke danach, und
    /// ihr erster schliesst an den Abzug an, nicht an Genesis. Dazu die
    /// Ablage, ohne Kettendatei keine.
    fn kette_ab_basis(
        konfig: &KnotenKonfig,
        protokoll: &mut Betriebsprotokoll,
    ) -> Result<(Kette, Option<myl_consensus::block::Block>), KnotenFehler> {
        let mut kette = Kette::im_netz(konfig.netz);
        // ⚑ **Der Stimmsatz ab Genesis** (2026-10-27): aus der
        //   Stimmsatzdatei, für jeden Knoten, nicht nur für die, die
        //   mitstimmen. Ohne Datei lernt nur eine Probekette ihn aus Block 1;
        //   jedes andere Netz startet nicht.
        match konfig.stimmsatzdatei_pfad.as_ref() {
            Some(pfad) => {
                let text = std::fs::read_to_string(pfad).map_err(|e| KnotenFehler::Kette(format!("{}: {e}", pfad.display())))?;
                let datei = crate::stimmsatzdatei::Stimmsatzdatei::aus_text(&text)
                    .map_err(|e| KnotenFehler::Kette(format!("{}: {e}", pfad.display())))?;
                kette.stimmsatz_setzen(datei.stimmberechtigte());
            }
            None if !konfig.netz.lernt_erzeuger => {
                return Err(KnotenFehler::Kette(format!(
                    "das Netz {} braucht eine Stimmsatzdatei (--stimmsatz): Jeder Block braucht die \
                     Unterschrift eines Mitglieds, und nur eine Probekette lernt ihren Erzeuger aus Block 1",
                    konfig.netz.name
                )));
            }
            None => {}
        }
        let mut basisblock = None;
        if let Some(pfad) = konfig.kettendatei.as_ref() {
            let ablage = crate::abzug::Ablage::neben(pfad, konfig.netz.startwert_hash());
            if let Some(h) = ablage.basis() {
                let (abzug, zustand) = ablage.lesen(h).map_err(|e| KnotenFehler::Kette(format!("Basis bei {h}: {e}")))?;
                kette.auf_abzug_setzen(&abzug, zustand).map_err(KnotenFehler::Kette)?;
                protokoll.schreibe(
                    Eintrag::neu("abzug_geladen")
                        .zahl("hoehe", h as i64)
                        .wahr("bestaetigt", ablage.bestaetigt())
                        .text("zustandswurzel", kurz(&kette.zustandswurzel())),
                );
                basisblock = Some(abzug.block);
            }
            kette.ablage_setzen(ablage);
        }
        Ok((kette, basisblock))
    }

    /// Startet den Knoten: Identität, Swarm, Horchadressen, Bootstrap.
    ///
    /// Die Reihenfolge ist bedeutsam. Geprüft wird **zuerst**, denn eine
    /// widersprüchliche Konfiguration äußert sich später als Stille
    /// (Fund 56), und Stille ist das Schwerste zu debuggen.
    pub async fn starten(
        konfig: KnotenKonfig,
        auf_bildschirm: bool,
    ) -> Result<Self, KnotenFehler> {
        // ⚑ **Ein zustandsloser Knoten führt keine Kettendatei** (2026-10-29):
        //   Er könnte sie ohne Zeugen nicht nachrechnen. Nach einem Neustart
        //   steigt er über einen leichten Abzug wieder ein.
        let mut konfig = konfig;
        if konfig.zustandslos {
            konfig.kettendatei = None;
        }
        konfig.pruefe().map_err(KnotenFehler::Konfig)?;

        // Der Weg von der Tür zur Kette. Unbegrenzt, weil eine
        // Abrechnung nicht verlorengehen darf: Ein voller Kanal hiesse,
        // dass gerechnete Arbeit unbezahlt bleibt.
        let (abrechnungen_tx, abrechnungen_rx) = tokio::sync::mpsc::unbounded_channel();
        let (nachrechnungen_tx, nachrechnungen_rx) = tokio::sync::mpsc::unbounded_channel();
        let (schied_tx, schied_rx) = tokio::sync::mpsc::unbounded_channel();
        let (selbstprobe_tx, selbstprobe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (tx_geprueft_tx, tx_geprueft_rx) = tokio::sync::mpsc::unbounded_channel();
        let (relais_geprueft_tx, relais_geprueft_rx) = tokio::sync::mpsc::unbounded_channel();

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

        // ⚑ **Der Kontoschluessel** (Fund 170). Fehlt er, faellt der
        // Knoten auf `kette::schluessel_fuer(name)` zurueck, und das ist
        // `probeschluessel(sha256(name)[0])`: einer von acht, aus dem
        // Namen nachrechenbar. Ein Startfehler waere hier falsch, denn
        // der Probelauf soll ohne Schluesseldatei laufen; **stumm
        // bleiben waere aber schlimmer**.
        let kontoschluessel = match konfig.kontoschluesseldatei.as_ref() {
            Some(pfad) => Some(
                crate::schluessel::Konsensschluessel::aus_datei(pfad)
                    .map_err(|e| KnotenFehler::Ortsleitung(format!("{}: {e:?}", pfad.display())))?,
            ),
            None => None,
        };

        let identitaet = NodeIdentity::load_or_create(Path::new(&konfig.schluesseldatei))
            .map_err(|e| KnotenFehler::Identitaet(e.to_string()))?;
        let peer_id = identitaet.peer_id();

        let mut protokoll = Betriebsprotokoll::mit_grenze(
            &konfig.protokollverzeichnis,
            &konfig.name,
            &peer_id.to_string(),
            auf_bildschirm,
            konfig.protokoll_mb.saturating_mul(1024 * 1024),
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
        // ⚑ Nach der Rolle: Volle Knoten hören die Zeugen nicht, zustandslose
        //   keine Transaktionen (myl-net `Rolle`).
        let rolle = if konfig.zustandslos { myl_net::Rolle::Zustandslos } else { myl_net::Rolle::Voll };
        if konfig.zustandslos && konfig.erzeugt_bloecke {
            return Err(KnotenFehler::Kette("ein zustandsloser Knoten erzeugt keine Blöcke".into()));
        }
        myl_net::subscribe_rolle(&mut swarm, rolle).map_err(|e| KnotenFehler::Netz(format!("{:?}", e)))?;

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
        let (anfragen_tx, anfragen_rx) = mpsc::channel(16);
        // ⚑ Die Relaisleitung: nur mit Adresse und Ausweisdatei.
        let (relais_tx, relaisauftraege) = mpsc::channel::<crate::relais::Relaisauftrag>(64);
        if let (Some(adresse), Some(datei)) = (konfig.podrelais, konfig.podrelaisausweis.as_ref()) {
            match crate::relais::starten(adresse, datei, relais_tx.clone()) {
                Ok(wo) => {
                    protokoll.schreibe(Eintrag::neu("relais_offen").text("adresse", wo.to_string()));
                    eprintln!("myl-node: Relaisleitung auf {wo}");
                }
                Err(e) => return Err(KnotenFehler::Ortsleitung(format!("Relaisleitung {adresse}: {e}"))),
            }
        }
        // ⚑ A10: Der Validatorsatz kommt aus der Teilnehmerliste. Ohne
        // sie ist er leer, und dann wird jedes Attest abgewiesen. Das
        // ist der sichere Vorgabefall.
        let validatoren = crate::validatorsatz::Validatorsatz::aus_namen(&konfig.teilnehmer);
        protokoll.schreibe(
            Eintrag::neu("validatorsatz")
                .zahl("bekannte_aussteller", validatoren.anzahl() as i64)
                .wahr("atteste_pruefbar", validatoren.anzahl() > 0),
        );
        let erzeugerliste = crate::validator::Erzeugerliste::default();
        let zeugenpruefer = ProtokollValidator::default().mit_erzeugern(erzeugerliste.clone());
        tokio::spawn(run_node_mit(
            swarm,
            cmd_rx,
            ev_tx,
            Arc::new(ProtokollValidator::mit(validatoren).mit_erzeugern(erzeugerliste.clone())),
        ));

        // ⚑ **Der Wiederanlauf ist ein Nachrechnen, kein Einlesen**, ab dem
        //   jüngsten eigenen Abzug (Fund 629, 2026-10-24).
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
        //
        // ⚑ **Die Ausnahme sind die Blöcke bis zum jüngsten eigenen Abzug.**
        //   Sie werden nur auf Anschluss geprüft (`Kette::vorspulen`), und
        //   der Abzug setzt den Zustand ein (`Kette::abzug_einsetzen`). Keine
        //   zweite Wahrheit: Die Zustandswurzel seines Blocks bindet ihn, die
        //   Hashes binden den Block an die Basis. Bis dahin rechnete jeder
        //   Start ab Genesis, und das wuchs mit der Geschichte: 166 s für
        //   349 646 Blöcke im Archivlauf. Scheitert der Einsatz, wird ab der
        //   Basis nachgerechnet wie zuvor.
        let (mut kette, mut basisblock) = Self::kette_ab_basis(&konfig, &mut protokoll)?;
        if let Some(pfad) = konfig.kettendatei.clone() {
            match crate::speicher::Kettenspeicher::oeffnen(&pfad, konfig.netz.startwert_hash()) {
                Ok((mut speicher, anlauf)) => {
                    let vorhanden = anlauf.anzahl;
                    let einsatz = kette.ablage().and_then(|a| {
                        let h = a.hoehen().into_iter().rev().find(|&h| h > kette.hoehe())?;
                        a.lesen(h).ok()
                    });
                    let mut n = nachspielen(&mut kette, &mut speicher, einsatz);
                    if n.verworfen.is_some() {
                        let verworfen = n.verworfen.take();
                        (kette, basisblock) = Self::kette_ab_basis(&konfig, &mut protokoll)?;
                        n = nachspielen(&mut kette, &mut speicher, None);
                        n.verworfen = verworfen;
                    }
                    // Erst **nach** dem Nachspielen anhängen, sonst
                    // schriebe der Wiederanlauf jeden Block ein zweites
                    // Mal in dieselbe Datei.
                    kette.speicher_setzen(speicher);
                    kette.budget_setzen(konfig.kette_mb.saturating_mul(1024 * 1024));
                    let mut eintrag = Eintrag::neu("kette_geladen")
                        .text("datei", pfad.display().to_string())
                        .wahr("neu", anlauf.neu)
                        .zahl("in_datei", vorhanden as i64)
                        .text("lesefehler", n.lesefehler.unwrap_or_default())
                        .zahl("uebernommen", n.uebernommen as i64)
                        .zahl("abgelehnt", n.abgelehnt as i64)
                        // Bis zum eigenen Abzug nur auf Anschluss geprüft.
                        .zahl("ab_abzug", n.abzug.unwrap_or(0) as i64)
                        .zahl("vorgespult", n.vorgespult as i64)
                        // Ein abgebrochener letzter Satz heißt: der
                        // Knoten wurde mitten im Schreiben abgeräumt.
                        // Genau das will der Chaos-Test wissen.
                        .zahl("abgeschnitten_bytes", anlauf.abgeschnitten as i64)
                        .zahl("hoehe", kette.hoehe() as i64)
                        .text("zustandswurzel", kurz(&kette.zustandswurzel()));
                    if !n.erster_grund.is_empty() {
                        eintrag = eintrag.text("erster_grund", n.erster_grund);
                    }
                    if let Some(grund) = n.verworfen {
                        eintrag = eintrag.text("abzug_verworfen", grund);
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

        // ⚑ **Stufe 4** (2026-10-22): Wer zusagt oder rechnet, braucht den
        //   Schlüssel, unter dem er als Miner angemeldet ist.
        // ⚑ **Zustandsabzug** (2026-10-22): Ein Knoten ohne eigene Kette holt
        //   den Abzug seines Ankers, der vom Betreiber vor dem des Netzes.
        //   Und wer aus einem Abzug lebt, den die Hintergrundprüfung noch
        //   nicht bestätigt hat, prüft weiter.
        // ⚑ Ein zustandsloser Knoten holt einen **leichten** Abzug, und nur
        //   mit dem Anker des Betreibers: Die Anker des Netzes liegen an
        //   alten Epochengrenzen, und so weit hält niemand Zeugen bereit.
        //   Ohne ihn sucht er einen über ein Prüfpunktzertifikat.
        let abzug_holen = match (kette.hoehe() == 0, konfig.zustandslos) {
            (true, false) => konfig.abzug.or_else(|| konfig.netz.juengster_anker()).map(AbzugHolen::neu),
            (true, true) => konfig.abzug.map(|a| AbzugHolen::neu_als(a, crate::abzug::Art::Leicht)),
            _ => None,
        };
        let anker_suchen =
            (konfig.zustandslos && abzug_holen.is_none() && kette.hoehe() == 0 && kette.stimmsatz().is_some()).then(std::time::Instant::now);
        let hintergrund = match (&basisblock, kette.ablage()) {
            (Some(b), Some(a))
                if konfig.hintergrundpruefung
                    && !a.bestaetigt()
                    && !konfig.netz.ist_anker(b.header.height, &b.hash()) =>
            {
                Some(Hintergrund::neu(konfig.netz, kette.stimmsatz().cloned(), b))
            }
            _ => None,
        };
        // ⚑ Auch `--konto` braucht die Kennung als Miner (Fund 643): Das
        //   Auszahlungskonto steht unter ihr, und eintragen darf nur sie.
        let als_miner = konfig.modellzusage.is_some() || konfig.shard.is_some() || konfig.konto.is_some();
        let netzschluessel = match (als_miner, &konfig.konsensschluesseldatei) {
            (false, _) => None,
            (true, Some(pfad)) => Some(
                crate::schluessel::Konsensschluessel::aus_datei(pfad)
                    .map_err(|e| KnotenFehler::Ortsleitung(format!("{}: {e:?}", pfad.display())))?,
            ),
            (true, None) => {
                return Err(KnotenFehler::Ortsleitung(
                    "--modellzusage, --shard-programm und --konto brauchen --konsensschluessel".into(),
                ))
            }
        };
        // ⚠️ Nur lesen, nie anlegen: Ein neu erzeugter Schlüssel wäre nie
        //    eingetragen und könnte nichts wechseln.
        let kontowechsel = match konfig.kontowechseldatei.as_ref() {
            Some(pfad) if pfad.exists() => Some(
                crate::schluessel::Konsensschluessel::aus_datei(pfad)
                    .map_err(|e| KnotenFehler::Ortsleitung(format!("{}: {e:?}", pfad.display())))?,
            ),
            Some(pfad) => {
                return Err(KnotenFehler::Ortsleitung(format!("--kontowechsel: {} fehlt", pfad.display())))
            }
            None => None,
        };
        // ⚑ **Archivrolle** ([`crate::archiv`], 2026-10-23): aus der eigenen
        //   Kennung und den beiden jüngsten Langzeitabzügen. Eine gezogene
        //   Kopie, der die Geschichte fehlt, holt sie mit Beweisen
        //   ([`crate::archivabruf`]).
        kette.archivkennung_setzen(netzschluessel.as_ref().map(|k| k.kennung()));
        // ⚑ **Der Erzeuger unterschreibt seine Blöcke** (2026-10-27), wenn
        //   er einen Konsensschlüssel hat; auch ein Erzeuger, der kein Miner
        //   ist.
        if konfig.erzeugt_bloecke {
            let schluessel = netzschluessel.clone().or_else(|| {
                konfig
                    .konsensschluesseldatei
                    .as_ref()
                    .and_then(|p| crate::schluessel::Konsensschluessel::aus_datei(p).ok())
            });
            // ⚑ Ohne eigenen Schlüssel unterschreibt ein Erzeuger auf der
            //   Probekette mit dem Probeerzeuger (2026-10-27); jeder Block
            //   braucht eine Unterschrift. Auf jedem anderen Netz ist das ein
            //   Startfehler.
            let schluessel = match schluessel {
                Some(k) => Some(k),
                None if konfig.netz.lernt_erzeuger => {
                    protokoll.schreibe(Eintrag::neu("erzeuger_probeschluessel").text("name", crate::kette::PROBEERZEUGER));
                    crate::schluessel::Konsensschluessel::probe(crate::kette::PROBEERZEUGER).ok()
                }
                None => {
                    return Err(KnotenFehler::Kette(
                        "dieser Knoten soll Blöcke erzeugen und hat keinen Konsensschlüssel".into(),
                    ))
                }
            };
            if let Some(k) = schluessel {
                // ⛔️ Ein Erzeuger, der nicht im Stimmsatz steht, baute nur
                //    Blöcke, die jeder abweist (2026-10-27).
                if kette.stimmsatz().is_some_and(|satz| satz.pubkey(&k.kennung()) != Some(&k.oeffentlich())) {
                    return Err(KnotenFehler::Kette(
                        "dieser Knoten soll Blöcke erzeugen, sein Schlüssel steht aber nicht im Stimmsatz".into(),
                    ));
                }
                kette.erzeugerschluessel_setzen(k);
            }
        }
        let abruf = archiv_abruf(&kette);
        let shardfuehrung = match (&konfig.shard, &konfig.konsensschluesseldatei) {
            (Some(sk), Some(pfad)) if !sk.programm.as_os_str().is_empty() => {
                Some(crate::shardfuehrung::Shardfuehrung::neu(crate::shardfuehrung::Shardstart {
                    programm: sk.programm.clone(),
                    artefakte: sk.artefakte.clone(),
                    konsensschluessel: pfad.clone(),
                    training: sk.training,
                    netzlast: konfig.protokollverzeichnis.join(myl_store::teilablage::NETZLAST_DATEI),
                }))
            }
            _ => None,
        };
        let netzadresse = myl_net::netzadresse(&identitaet).unwrap_or(myl_types::latency_attest::PeerIdBytes([0; 32]));
        // ⚑ **Zustandslos ab hier** (2026-10-29): Die Kette ist geladen und
        //   nachgerechnet; die grossen Sammlungen fallen weg, die eigenen
        //   Konten bleiben (Netz-, Konto-, Miner- und Auszahlungskonto).
        if konfig.zustandslos {
            let mut eigene = vec![crate::kette::konto_fuer(&konfig.name)];
            for k in [netzschluessel.as_ref(), kontoschluessel.as_ref()].into_iter().flatten() {
                eigene.push(k.adresse());
                eigene.push(myl_types::ids::Address::new(*k.kennung().as_bytes()));
            }
            eigene.extend(konfig.konto);
            // Im Probenetz versorgt sich jeder aus den Probekonten (Probehahn);
            // ein zustandsloser Knoten führt sie deshalb wie eigene.
            if konfig.netz.probehahn {
                eigene.extend((0..crate::kette::PROBEKONTEN).map(crate::kette::probekonto));
            }
            kette.zustandslos_machen(&eigene);
            protokoll.schreibe(Eintrag::neu("zustandslos").zahl("eigene_konten", eigene.len() as i64).zahl("hoehe", kette.hoehe() as i64));
        }
        // ⚑ Zeugen bereithalten (2026-10-30): mit dem Netzschlüssel, sonst
        //   dem Probeschlüssel des Namens.
        if konfig.zeugen_bereithalten && !konfig.zustandslos {
            let k = match netzschluessel.clone() {
                Some(k) => Some(k),
                None => crate::schluessel::Konsensschluessel::probe(&konfig.name).ok(),
            };
            if let Some(k) = k {
                kette.zeugen_bereithalten_setzen(k);
                protokoll.schreibe(Eintrag::neu("zeugen_bereithalten"));
            }
        }
        // ⚑ Die Zeugen auf der Platte (2026-10-30): wer sie erzeugt, als
        //   Erzeuger oder als Zeugenhalter; neben der Kettendatei, sonst im
        //   Protokollverzeichnis.
        if !konfig.zustandslos && (konfig.erzeugt_bloecke || konfig.zeugen_bereithalten) {
            let ordner = match &konfig.kettendatei {
                Some(k) => k.with_extension("zeugen"),
                None => konfig.protokollverzeichnis.join(format!("{}.zeugen", konfig.name)),
            };
            kette
                .zeugen_auf_platte_setzen(ordner.clone())
                .map_err(|e| KnotenFehler::Kette(format!("Zeugen auf der Platte unter {}: {e}", ordner.display())))?;
            protokoll.schreibe(Eintrag::neu("zeugen_auf_platte").text("pfad", ordner.display().to_string()));
        }
        // ⚑ Der Zustandsbaum auf der Platte (2026-10-30): nur, wer ihn ganz
        //   hält.
        if konfig.baum_auf_platte && !konfig.zustandslos {
            let pfad = match &konfig.kettendatei {
                Some(k) => k.with_extension("baum"),
                None => konfig.protokollverzeichnis.join(format!("{}.baum", konfig.name)),
            };
            kette
                .zustand_mut()
                .baum_auslagern(&pfad)
                .map_err(|e| KnotenFehler::Kette(format!("Zustandsbaum auf der Platte unter {}: {e}", pfad.display())))?;
            protokoll.schreibe(Eintrag::neu("baum_auf_platte").text("pfad", pfad.display().to_string()));
        }
        erzeugerliste_aus(&kette, &erzeugerliste);
        protokoll.schreibe(
            Eintrag::neu("netz")
                .text("netz", konfig.netz.name)
                .text("artefakt", konfig.netz.artefakt)
                .wahr("zusage", konfig.modellzusage.is_some())
                .wahr("shard", shardfuehrung.is_some()),
        );

        Ok(Self {
            netzschluessel,
            kontowechsel,
            auszahlung_gemeldet: None,
            netzadresse,
            netz_nonce: 0,
            netz_stand: (0, 0),
            netz_gesendet: std::collections::BTreeMap::new(),
            shardfuehrung,
            uebernahme: None,
            spurablagen: Vec::new(),
            fristsammlung: crate::frist::Fristsammlung::default(),
            messungen: std::collections::BTreeMap::new(),
            gemessen: std::collections::BTreeSet::new(),
            fristverdacht: std::collections::BTreeMap::new(),
            pruefpunktstimmen: std::collections::BTreeMap::new(),
            pruefpunkt_gestimmt: (0, None),
            doppelbelege_gemeldet: std::collections::BTreeSet::new(),
            nachholen: None,
            ausstehend: Vec::new(),
            naechste_aufnahme: None,
            naechster_block: None,
            hoehe_beim_testverkehr: 0,
            voraus: Default::default(),
            testverkehr_nonce: (0, 0),
            abzug_holen,
            hintergrund,
            abruf,
            teilabruf: None,
            abzug_gemeldet: None,
            angehalten: None,
            gegenstellen: Vec::new(),
            nachfrage_reihum: 0,
            konfig,
            // ⚑ **Als Erstes, noch vor allem Warten** (Fund 140). Ab
            // hier ist jedes Signal aufgehoben.
            wache: Beendigungswache::stellen(),
            peer_id,
            kommandos: cmd_tx,
            stichprobe_gefragt: None,
            ereignisse: ev_rx,
            kontoanfragen: anfragen_rx,
            relaisauftraege,
            vorrang: None,
            _relais_tx: relais_tx,
            shardrelais: std::collections::HashMap::new(),
            relais_kennung: 0,
            shard_plaetze: Plaetze::neu(INFERENZ_IN_ARBEIT_HOECHSTENS),
            protokoll,
            testverkehr_zaehler: 0,
            kette,
            nachforderungen: Vec::new(),
            leer_geantwortet: std::collections::HashMap::new(),
            zuletzt_gewachsen: (0, std::time::Instant::now()),
            stille_bei: None,
            fristsperre_gemeldet: None,
            probehahn_versuche: 0,
            beobachtungsstelle: crate::beobachtung::Beobachtungsstelle::mit_anfragen(anfragen_tx),
            kontraktabschrift: crate::tuer::Kontraktabschrift::neu(),
            abschrift_hoehe: None,
            letzte_inferenzantwort: None,
            ortsanschluss,
            inferenz_zaehler: Arc::new(Inferenzzaehler::default()),
            inferenz_plaetze: Plaetze::neu(INFERENZ_IN_ARBEIT_HOECHSTENS),
            drossel: crate::drossel::Drossel::default(),
            gedrosselt: 0,
            zeitprobe_platz: Plaetze::neu(1),
            zugesagt_bytes: 0,
            abrechnungen: abrechnungen_rx,
            abrechnungskanal: abrechnungen_tx,
            verlauf_gefragt: std::collections::BTreeSet::new(),
            nachgerechnet: std::collections::BTreeSet::new(),
            festgelegt: std::collections::BTreeMap::new(),
            festzulegen: Vec::new(),
            schiedsbelege: std::collections::BTreeMap::new(),
            schied_angefragt: std::collections::BTreeMap::new(),
            erzeugerliste,
            zeugenpruefer,
            zeugensammler: myl_consensus::zeuge::Zeugensammler::neu(ZEUGEN_SAMMLER_BYTES),
            wartet_auf_zeugen: std::collections::BTreeMap::new(),
            zeugen_eilig: std::collections::BTreeSet::new(),
            fertige_zeugen: std::collections::VecDeque::new(),
            zeugen_gesperrt: std::collections::VecDeque::new(),
            zeugen_angefragt: std::collections::BTreeMap::new(),
            zeugen_quellen: std::collections::VecDeque::new(),
            zeugen_reihum: 0,
            nur_abzug: 0,
            anker_suchen,
            anker_kandidat: None,
            stillstand: (0, std::time::Instant::now()),
            hoechster_belegter: 0,
            stillstand_hoechstens: STILLSTAND_HOECHSTENS,
            zeugen_zurueckhalten: false,
            kontobeweise_offen: None,
            kontobeweise_angefragt: None,
            schied_gestimmt: std::collections::BTreeSet::new(),
            schiedsrechnungen: schied_rx,
            groessen_epoche: None,
            tx_zaehler: TxZaehler::default(),
            tx_puffer: Vec::new(),
            tx_puffer_seit: std::time::Instant::now(),
            tx_geprueft: tx_geprueft_rx,
            tx_geprueft_kanal: tx_geprueft_tx,
            relais_geprueft: relais_geprueft_rx,
            relais_geprueft_kanal: relais_geprueft_tx,
            selbstprobe_epoche: None,
            selbstproben: selbstprobe_rx,
            selbstprobenkanal: selbstprobe_tx,
            ebenenzeit_angepasst: 0,
            schiedskanal: schied_tx,
            pruefmodell: Arc::default(),
            nachrechnungen: nachrechnungen_rx,
            nachrechnungskanal: nachrechnungen_tx,
            abrechnungsnonce: 1_000_000,
            kontoschluessel,
            hoechste_gehoerte: 0,
            gehoert: crate::gehoert::Gehoert::default(),
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
        let schluessel_fuer_bloecke = schluessel.clone();
        let jetzt = self.uhr_ms();
        let (laufende, raus) =
            Konsensrunde::beginnen(genesis, schluessel, vorschlag, jetzt, timeouts)?;
        // ⚑ **Und die Kette bekommt den Stimmsatz** (Punkt 44). Ohne ihn
        // prüft sie eine Saatquelle nur gegen den Vorgänger und nicht
        // gegen die Unterschriften; sie sagt das dann auch, statt
        // stillschweigend alles anzunehmen.
        self.kette.stimmsatz_setzen(genesis.stimmberechtigte());
        // Mit Stimmsatz muss jeder Block unterschrieben sein;
        // der eigene mit dem Schlüssel dieser Runde.
        self.kette.erzeugerschluessel_setzen(schluessel_fuer_bloecke);
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
        // 📌 **Die Fälligkeit überlebt den Aufruf** (2026-10-22, gefunden im
        //    Netzlauf): Hier stand „jetzt plus Takt“ bei jedem Aufruf. Wer
        //    den Knoten in Scheiben kürzer als der Takt fuhr, bekam nie eine
        //    Aufnahme, und mit ihr fiel die ganze Netzarbeit aus: keine
        //    Anmeldung, keine Zusage, kein Shard.
        let mut naechste_aufnahme = *self.naechste_aufnahme.get_or_insert_with(|| tokio::time::Instant::now() + takt);
        let sendetakt = self.konfig.testverkehr_sekunden.map(|s| Duration::from_secs(s.max(1)));
        let mut naechster_versand = sendetakt.map(|t| tokio::time::Instant::now() + t);
        let blocktakt = self.blocktakt();
        if let Some(t) = blocktakt {
            self.naechster_block.get_or_insert_with(|| tokio::time::Instant::now() + t);
        }
        loop {
            let jetzt = tokio::time::Instant::now();
            if jetzt >= ende || self.angehalten.is_some() {
                return;
            }
            if jetzt >= naechste_aufnahme {
                self.aufnahme().await;
                naechste_aufnahme = jetzt + takt;
                self.naechste_aufnahme = Some(naechste_aufnahme);
            }
            self.abschrift_nachziehen();
            if let (Some(faellig), Some(t)) = (naechster_versand, sendetakt) {
                if jetzt >= faellig {
                    self.sende_testverkehr().await;
                    naechster_versand = Some(jetzt + t);
                }
            }
            // ⚑ Fällig ab dem geplanten Zeitpunkt, nicht ab jetzt: Ein Block,
            //   der sich verspätet, verschiebt nicht alle folgenden.
            //   Hinkt der Erzeuger weiter als einen Takt hinterher, setzt er
            //   neu auf, statt die verpassten Blöcke nachzuschiessen.
            if let (Some(faellig), Some(t)) = (self.naechster_block, blocktakt) {
                if jetzt >= faellig {
                    self.block_wenn_jemand_hoert().await;
                    let naechster = faellig + t;
                    self.naechster_block = Some(if naechster <= jetzt { jetzt + t } else { naechster });
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
            if let (Some(faellig), Some(_)) = (self.naechster_block, blocktakt) {
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
            // Gesammelte Transaktionen warten höchstens 20 ms auf ihre Prüfung.
            if !self.tx_puffer.is_empty() {
                rest = rest.min(Duration::from_millis(20));
            }
            let rest = rest.max(Duration::from_millis(1));
            // ⚑ Eine Kontoanfrage weckt die Schleife wie ein Ereignis und
            //   wird aus dem Zustand beantwortet, ohne zu warten.
            let geweckt = tokio::select! {
                ev = self.ereignisse.recv() => Weckruf::Ereignis(ev),
                Some(a) = self.kontoanfragen.recv() => Weckruf::Konto(a),
                Some(a) = self.relaisauftraege.recv() => Weckruf::Relais(a),
                Some(g) = self.tx_geprueft.recv() => Weckruf::Geprueft(g),
                Some(g) = self.relais_geprueft.recv() => Weckruf::RelaisGeprueft(g),
                _ = tokio::time::sleep(rest) => Weckruf::Frist,
            };
            match geweckt {
                Weckruf::Ereignis(Some(ev)) => {
                    self.vermerke(ev);
                    self.leere_konsensausgang().await;
                }
                Weckruf::Ereignis(None) => return,
                Weckruf::Konto((adresse, rueck)) => {
                    let _ = rueck.send(self.kontoauskunft(&adresse));
                }
                Weckruf::Relais((frage, rueck)) => self.relais_bedienen(frage, rueck),
                Weckruf::Geprueft(g) => self.geprueft_aufnehmen(g),
                Weckruf::RelaisGeprueft(g) => self.relais_geprueft_aufnehmen(g),
                Weckruf::Frist => {}
            }
            self.tx_puffer_pflegen();
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
                    None => while self.angehalten.is_none() { self.laufe_fuer(Duration::from_secs(3600)).await },
                }
            } => self.angehalten.unwrap_or("Laufzeit abgelaufen"),
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

    /// Inferenzauftraege aus dem Netz: bearbeitet und abgelehnt.
    pub fn inferenzstand(&self) -> (u64, u64) {
        self.inferenz_zaehler.stand()
    }

    /// Die zuletzt eingetroffene Inferenzantwort.
    pub fn letzte_inferenzantwort(
        &self,
    ) -> Option<&myl_types::inferenzauftrag::Inferenzantwort> {
        self.letzte_inferenzantwort.as_ref()
    }

    /// **Zieht die Kontraktabschrift nach, wenn die Kette gewachsen ist**
    /// (Fund 648). Eine Höhe, ein Abgleich: Wer eine Sitzung eröffnet,
    /// kann fragen, sobald der Block da ist, und ein Widerruf wirkt ab dem
    /// Block, der ihn trägt.
    ///
    /// ⚠️ Kopiert je Block alle Kontrakte; bei vielen Sitzungen gehört
    /// hier ein Abgleich nur der geänderten hin.
    fn abschrift_nachziehen(&mut self) {
        let hoehe = self.kette.hoehe();
        if self.abschrift_hoehe != Some(hoehe) {
            self.kontraktabschrift.setzen(self.kette.zustand());
            self.abschrift_hoehe = Some(hoehe);
        }
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
        let rueckgrat = if self.blocktakt().is_some() {
            // Mit Blocktakt baut der Takt; geprüft wird, dass die Kette
            // seit dem vorigen Testverkehr gewachsen ist.
            let ok = self.kette.hoehe() > self.hoehe_beim_testverkehr;
            self.hoehe_beim_testverkehr = self.kette.hoehe();
            self.vermerke_probe(Probe::Blockkette, ok);
            ok
        } else if self.konfig.erzeugt_bloecke {
            match self.block_wenn_jemand_hoert().await {
                Some(ok) => {
                    self.vermerke_probe(Probe::Blockkette, ok);
                    ok
                }
                None => return false,
            }
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

    /// Der Blocktakt, wenn dieser Knoten erzeugt und einen hat.
    fn blocktakt(&self) -> Option<Duration> {
        self.konfig.blocktakt_ms.filter(|_| self.konfig.erzeugt_bloecke).map(Duration::from_millis)
    }

    /// Baut einen Block, sobald jemand zuhört; `None` heißt, es hört noch
    /// niemand zu, und es wurde nichts gebaut.
    async fn block_wenn_jemand_hoert(&mut self) -> Option<bool> {
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
            return None;
        }
        Some(self.erzeuge_block().await)
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
        if self.kette.ist_zustandslos() {
            return false;
        }
        // ⚑ Das Zertifikat eines Prüfpunkts legt die Kette selbst in den
        //   Block, wenn der Knoten es angeboten hat (CONSENSUS 7.2,
        //   `pruefpunkt_pflegen`). Bis zum 2026-10-27 stand hier die
        //   Saatquelle aus dem Commitzertifikat der BFT-Runde.
        let wartend = self.kette.wartend();
        let block = self.kette.baue_block();
        let daten = match borsh::to_vec(&block) {
            Ok(d) => d,
            Err(_) => return false,
        };
        let bytes = daten.len();
        let ok = self.veroeffentliche(GossipTopic::Blocks, daten).await;
        // ⚑ **Dann der Zeuge, in unterschriebenen Stücken**, für die
        //   zustandslosen Knoten; ohne Abonnenten geht er nirgends hin.
        let hash = self.kette.letzter_hash();
        let stuecke = if self.zeugen_zurueckhalten { None } else { self.kette.zeugenstuecke(&hash) };
        let zeugenstuecke = stuecke.map_or(0, |stuecke| {
            for s in stuecke.iter() {
                if let Ok(d) = borsh::to_vec(s) {
                    let _ = self.kommandos.send(NodeCommand::Publish { topic: GossipTopic::Zeugen, data: d, result: None });
                }
            }
            stuecke.len()
        });
        self.protokoll.schreibe(
            Eintrag::neu("block_erzeugt")
                .zahl("zeugenstuecke", zeugenstuecke as i64)
                .zahl("hoehe", self.kette.hoehe() as i64)
                .zahl("txs", wartend as i64)
                .zahl("bytes", bytes as i64)
                .text("zustandswurzel", kurz(&self.kette.wurzel()))
                .text("block", kurz(&self.kette.letzter_hash()))
                .wahr("verbreitet", ok),
        );
        self.stichprobe_verschicken().await;
        // Ein Erzeuger, der allein den Stimmsatz trägt, bestätigt seinen
        // Prüfpunkt sofort, und der nächste Block trägt das Zertifikat.
        self.pruefpunkt_pflegen().await;
        ok
    }

    /// ⚑ **Prüfpunkte pflegen** (2026-10-27, CONSENSUS 7.2, Festlegung des
    /// Projektinhabers: Prüfpunktstimmen statt Runden).
    ///
    /// 1. **Abstimmen** über den jüngsten Prüfpunkt, den die Kette hat und
    ///    der noch nicht bestätigt ist, wenn der eigene Schlüssel im
    ///    Stimmsatz steht; die Stimme wird alle zehn Sekunden wiederholt,
    ///    bis der Prüfpunkt bestätigt ist, damit eine verlorene Nachricht
    ///    die Endgültigkeit nicht aufhält.
    /// 2. **Sammeln:** Je Höhe zählen die Stimmen für den **eigenen** Block
    ///    an dieser Höhe; ab mehr als zwei Dritteln des Gewichts baut der
    ///    Knoten das Zertifikat und bietet es der Kette an. Nur der Erzeuger
    ///    legt es in einen Block, aber jeder hat es.
    async fn pruefpunkt_pflegen(&mut self) {
        use myl_consensus::pruefpunkt::{BLOECKE_JE_PRUEFPUNKT, Pruefpunktstimme, Pruefpunktzertifikat, pruefpunktnachricht};
        let Some(satz) = self.kette.stimmsatz().cloned() else {
            return;
        };
        let bestaetigt = self.kette.bestaetigt_bis();
        let juengster = self.kette.hoehe() / BLOECKE_JE_PRUEFPUNKT * BLOECKE_JE_PRUEFPUNKT;
        let schluessel = if self.konfig.erzeugt_bloecke {
            self.kette.erzeugerschluessel().cloned()
        } else {
            self.netzschluessel.clone()
        };
        let faellig = match self.pruefpunkt_gestimmt {
            (h, _) if h < juengster => true,
            (h, Some(t)) => h == juengster && t.elapsed() >= Duration::from_secs(10),
            (_, None) => false,
        };
        if juengster > bestaetigt && faellig {
            if let (Some(k), Some(block_hash)) = (schluessel, self.kette.hash_an(juengster)) {
                if satz.pubkey(&k.kennung()) == Some(&k.oeffentlich()) {
                    if let Ok(signatur) = k.signiere(&pruefpunktnachricht(juengster, &block_hash)) {
                        let stimme = Pruefpunktstimme { hoehe: juengster, block_hash, waehler: k.kennung(), signatur };
                        self.pruefpunktstimmen.entry(juengster).or_default().insert(stimme.waehler, stimme.clone());
                        let wiederholt = self.pruefpunkt_gestimmt.0 == juengster;
                        self.pruefpunkt_gestimmt = (juengster, Some(std::time::Instant::now()));
                        if let Ok(bytes) = borsh::to_vec(&myl_consensus::bft::Konsensnachricht::Pruefpunktstimme(stimme)) {
                            self.veroeffentliche(GossipTopic::Consensus, bytes).await;
                        }
                        if !wiederholt {
                            self.protokoll.schreibe(Eintrag::neu("pruefpunkt_gestimmt").zahl("hoehe", juengster as i64));
                        }
                    }
                }
            }
        }
        self.pruefpunktstimmen.retain(|h, _| *h > bestaetigt);
        let mut fertig = None;
        for (h, stimmen) in &self.pruefpunktstimmen {
            let Some(eigener) = self.kette.hash_an(*h) else {
                continue;
            };
            let passende: Vec<Pruefpunktstimme> = stimmen.values().filter(|s| s.block_hash == eigener).cloned().collect();
            let gewicht = passende.iter().fold(0u64, |a, s| a.saturating_add(satz.weight(&s.waehler)));
            if gewicht >= satz.quorum_threshold() {
                if let Ok(z) = Pruefpunktzertifikat::aus_stimmen(&passende) {
                    if z.verify(&satz).is_ok() {
                        fertig = Some(z);
                    }
                }
            }
        }
        if let Some(z) = fertig {
            if self.kette.naechstes_pruefpunktzertifikat_hoehe() < Some(z.hoehe) {
                self.protokoll.schreibe(
                    Eintrag::neu("pruefpunkt_zertifikat")
                        .zahl("hoehe", z.hoehe as i64)
                        .zahl("unterzeichner", z.unterzeichner.len() as i64),
                );
            }
            self.kette.pruefpunkt_anbieten(z);
        }
    }

    /// Eine Prüfpunktstimme aus dem Netz: aufheben, wenn sie trägt und über
    /// dem bestätigten Stand liegt; gezählt wird in `pruefpunkt_pflegen`.
    fn pruefpunktstimme_aufnehmen(&mut self, stimme: myl_consensus::pruefpunkt::Pruefpunktstimme) {
        if stimme.hoehe <= self.kette.bestaetigt_bis() {
            return;
        }
        if !self.kette.stimmsatz().is_some_and(|satz| stimme.traegt(satz)) {
            self.protokoll.schreibe(
                Eintrag::neu("pruefpunktstimme_abgelehnt")
                    .zahl("hoehe", stimme.hoehe as i64)
                    .text("waehler", hex::encode(&stimme.waehler.as_bytes()[..8])),
            );
            return;
        }
        // Höchstens ein paar Höhen voraus aufheben, gegen eine Flut.
        if stimme.hoehe > self.kette.hoehe() + 4 * myl_consensus::pruefpunkt::BLOECKE_JE_PRUEFPUNKT {
            return;
        }
        // ⚑ Zwei Stimmen eines Wählers für dieselbe Höhe und verschiedene
        //   Blöcke sind ein Doppelbeleg (CONSENSUS 7.6).
        let alte = self.pruefpunktstimmen.get(&stimme.hoehe).and_then(|m| m.get(&stimme.waehler)).cloned();
        if let Some(alt) = alte.filter(|a| a.block_hash != stimme.block_hash) {
            if let Some(schluessel) = self.kette.stimmsatz().and_then(|s| s.pubkey(&stimme.waehler)).copied() {
                self.doppelbeleg_melden(myl_consensus::double_signing::Doppelbeleg::Pruefpunkt {
                    erste: Box::new(alt),
                    zweite: Box::new(stimme),
                    schluessel,
                });
            }
            return;
        }
        self.pruefpunktstimmen.entry(stimme.hoehe).or_default().insert(stimme.waehler, stimme);
    }

    /// ⚑ **Zwei Blöcke derselben Höhe vom selben Erzeuger** (CONSENSUS
    /// 7.6): Hat die Kette an dieser Höhe einen anderen Block desselben
    /// Erzeugers, wird das gemeldet.
    fn doppelt_erzeugt_pruefen(&mut self, block: &myl_consensus::block::Block) {
        let Some(eigener) = self.kette.block_an(block.header.height) else {
            return;
        };
        if eigener.hash() == block.hash() {
            return;
        }
        let (Ok(Some(a)), Ok(Some(b))) = (eigener.erzeuger(), block.erzeuger()) else {
            return;
        };
        if a != b {
            return;
        }
        let (Some(x), Some(y)) = (
            myl_consensus::double_signing::Belegblock::aus_block(eigener),
            myl_consensus::double_signing::Belegblock::aus_block(block),
        ) else {
            return;
        };
        self.doppelbeleg_melden(myl_consensus::double_signing::Doppelbeleg::Erzeuger(Box::new(x), Box::new(y)));
    }

    /// Reicht einen Doppelbeleg ein, je Widerspruch einmal, wenn er trägt.
    fn doppelbeleg_melden(&mut self, beleg: myl_consensus::double_signing::Doppelbeleg) {
        let Ok(pk) = beleg.schuldiger() else {
            return;
        };
        let schluessel = (myl_types::ids::Address::aus_schluessel(&pk), beleg.art(), beleg.hoehe());
        if !self.doppelbelege_gemeldet.insert(schluessel) {
            return;
        }
        self.protokoll.schreibe(
            Eintrag::neu("doppelbeleg_erkannt")
                .zahl("belegart", i64::from(beleg.art()))
                .zahl("hoehe", beleg.hoehe() as i64)
                .text("schuldiger", hex::encode(&schluessel.0.as_bytes()[..8])),
        );
        if let Some(tx) = self.netz_tx(myl_consensus::block::Anweisung::DoppelbelegEinreichen { beleg: Box::new(beleg) }) {
            self.ausstehend.push(tx);
        }
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
    /// ⚑ **Die Antwort wird geprüft, aber nicht nachgerechnet.** Sie kommt
    /// als `Nachlieferung::Spur` zurück und geht an `spur_erhalten`
    /// (Beweis gegen die Wurzel aus der Kette); **was fehlt, ist ein
    /// Nachrechner mit Modell und der Eingabe des Segments**. Diese Grenze
    /// steht hier, damit niemand den Aufruf für mehr hält.
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
            // 📌 Fund 656: `anfrage` ist schon die Nachforderung.
            let Some(daten) = anfrage.als_bytes() else {
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
        // Die Checker der Ziehung, als Netzadressen: Fehlt einer, gab es in
        // der gezogenen Epoche keinen Miner ohne Pod.
        let pruefer: Vec<String> = self
            .kette
            .pruefer_der_stichprobe()
            .iter()
            .map(|(_, _, m)| {
                self.kette
                    .zustand()
                    .miner
                    .get(m)
                    .and_then(|r| myl_net::peer_id_aus_bytes(&r.netzadresse).ok())
                    .map_or_else(|| kurz(&myl_types::hash::Hash::from_bytes(*m.as_bytes())), |p| p.to_string())
            })
            .collect();
        self.protokoll.schreibe(
            Eintrag::neu("stichprobe_gefragt")
                .zahl("epoche", epoche as i64)
                .zahl("segmente", gezogen.len() as i64)
                .zahl("anfragen", gefragt as i64)
                .zahl("ohne_adresse", ohne_adresse as i64)
                .text("pruefer", pruefer.join(",")),
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
        //
        // 📌 **Fund 593 (2026-10-22): Die Nummer kommt aus dem Zustand.**
        //    Hier stand der Zähler des Testverkehrs, und der steht beim
        //    ersten Senden schon bei 1; die Kette verlangt die Nummern
        //    streng ab 0. Jede Transaktion des Testverkehrs wurde
        //    verworfen, über 1 342 Blöcke blieb die Wurzel dieselbe, und
        //    der Vergleich der Wurzeln, für den es diesen Verkehr gibt,
        //    belegte nichts. Nach einem Neustart begann der Zähler
        //    ausserdem von vorn. Jetzt: die Nummer des Kontos im Zustand,
        //    darüber hinaus fortgezählt, solange Gesendetes unterwegs ist;
        //    kommt 32 Blöcke lang nichts davon an, ging es verloren, und
        //    gezählt wird wieder ab dem Zustand.
        let schluessel = crate::kette::schluessel_fuer(&self.konfig.name);
        let im_zustand = self.kette.konto(&crate::kette::konto_fuer(&self.konfig.name)).nonce;
        let nonce = testverkehr_nonce(&mut self.testverkehr_nonce, im_zustand, self.kette.hoehe());
        // 📌 Fund 672 (2026-10-28, kurzer Podlauf): Hier stand `1 000 +
        //    Zaehler * 100`, ein Betrag, der jede Sekunde wuchs; aufsummiert
        //    rund 50 * T^2 Kleinstbetraege nach T Sekunden, ein Probekonto
        //    mit 10 MYL war nach rund 7,5 Minuten leer, und mit ihm der
        //    Vorrat, aus dem der Probehahn die Knoten versorgt. Den Zustand
        //    bewegen Nummer und Gebuehr; der Betrag ist der kleinste.
        let anweisung = Anweisung::Burn { betrag: 1 };
        let tx = match Transaktion::signiere(
            &self.kette.netzwert(),
            &schluessel,
            nonce,
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

    /// Das Konto, dessen Schlüssel dieser Knoten für Kettentransaktionen
    /// hält (Fund 170).
    ///
    /// ⚑ **Das muss der Empfänger der Abbuchung sein**, sonst weist
    /// `pruefe` sie mit `EmpfaengerNichtGelistet` ab: Der Nutzer hat in
    /// seiner Positivliste eine Adresse stehen, und nur diese.
    pub fn eigenes_konto(&self) -> myl_types::ids::Address {
        match self.kontoschluessel.as_ref() {
            Some(k) => k.adresse(),
            None => crate::kette::konto_fuer(&self.konfig.name),
        }
    }

    /// Zaehlt, was in den Orten fuer das Netz liegt, oder `None` ohne
    /// `--speicherorte`.
    ///
    /// ⚠️ **Eine kaputte Datei zaehlt als keine Orte und steht im
    /// Protokoll**, denn der Knoten soll an einer Tippfehlerzeile nicht
    /// sterben; die Zahl null sagt dann, dass etwas fehlt.
    fn speicher_zaehlen(&mut self) -> Option<Speicherstand> {
        let pfad = self.konfig.speicherorte.clone()?;
        let orte = match myl_store::Speicherorte::lesen(&pfad) {
            Ok(o) => o,
            Err(e) => {
                self.protokoll.schreibe(Eintrag::neu("speicherorte_fehler").text("grund", e.to_string()));
                return Some(Speicherstand::default());
            }
        };
        let mut s = Speicherstand::default();
        for o in orte.fuer(myl_store::Zweck::Netz) {
            let b = myl_store::belegung(o);
            s.orte += 1;
            s.grenze_bytes = s.grenze_bytes.saturating_add(o.volumen_bytes());
            s.belegung.gegenstaende += b.gegenstaende;
            s.belegung.shards += b.shards;
            s.belegung.unlesbar += b.unlesbar;
            s.belegung.bytes = s.belegung.bytes.saturating_add(b.bytes);
        }
        Some(s)
    }

    /// **Unterschreibt eine Zusage** mit dem Kontoschluessel und legt die
    /// Unterschrift daneben (`<datei>.unterschrift`).
    ///
    /// ⚑ **Nur mit einem Kontoschluessel aus einer Datei.** Der aus dem
    /// Namen abgeleitete ist keiner (Fund 170): Wer den Namen kennt,
    /// unterschriebe dieselbe Zusage.
    ///
    /// ⚑ **Die Epoche ist die der eigenen Kette beim Unterschreiben**; im
    /// Probelauf also die der Probekette, und die Zusage sagt selbst, dass
    /// sie aus einem Probelauf stammt.
    pub fn zusage_unterschreiben(&mut self, pfad: &Path) -> Result<std::path::PathBuf, String> {
        let roh = std::fs::read(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
        let text = std::str::from_utf8(&roh).map_err(|_| format!("{}: kein UTF-8", pfad.display()))?;
        let zusage = myl_store::Zusage::aus_text(text).map_err(|e| e.to_string())?;
        let Some(schluessel) = self.kontoschluessel.as_ref() else {
            return Err("--zusage braucht --kontoschluessel: ohne eigenen Schluessel ist eine Unterschrift keine".into());
        };
        let epoche = self.kette.zustand().epoch.0;
        let unterschrift = schluessel
            .signiere(&myl_store::orte::botschaft(&roh, epoche))
            .map_err(|e| format!("unterschreiben: {e:?}"))?;
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        let oeffentlich = borsh::to_vec(&schluessel.oeffentlich()).map_err(|e| e.to_string())?;
        let signatur = borsh::to_vec(&unterschrift).map_err(|e| e.to_string())?;
        let hash = myl_types::hash::Hash::sha256(&roh);
        let ziel = pfad.with_extension("unterschrift");
        let inhalt = format!(
            "# Unterschrift unter {}\n# Botschaft: MYELITH-KAPAZITAETSZUSAGE-V1, SHA-256 der Datei, Epoche (u64, little endian)\n\
             konto {}\nschluessel {}\nepoche {epoche}\nsha256 {}\nunterschrift {}\n",
            pfad.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            hex(self.eigenes_konto().as_bytes()),
            hex(&oeffentlich),
            hex(hash.as_bytes()),
            hex(&signatur),
        );
        std::fs::write(&ziel, inhalt).map_err(|e| format!("{}: {e}", ziel.display()))?;
        self.zugesagt_bytes = zusage.volumen_bytes();
        self.protokoll.schreibe(
            Eintrag::neu("zusage")
                .text("sha256", hex(hash.as_bytes()))
                .zahl("epoche", epoche.min(i64::MAX as u64) as i64)
                .zahl("volumen_bytes", zusage.volumen_bytes().min(i64::MAX as u64) as i64)
                .zahl("kerne", zusage.kerne as i64)
                .zahl("rechenwerke", zusage.rechenwerke.len() as i64)
                .text("probelauf", if zusage.probelauf { "ja" } else { "nein" }),
        );
        Ok(ziel)
    }

    /// Ob der Kontoschlüssel aus einer Datei kam.
    pub fn kontoschluessel_aus_datei(&self) -> bool {
        self.kontoschluessel.is_some()
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

        let mut offen = Vec::new();
        while let Ok(a) = self.abrechnungen.try_recv() {
            offen.push(a);
        }
        // ⚑ **Die Wahl steht in `schluessel::abrechnung_unterschreiben`**
        // (Fund 170), damit eine Gegenprobe sie erreicht.
        let mut verbreitet = 0usize;
        for anweisung in offen {
            let nonce = self.abrechnungsnonce;
            self.abrechnungsnonce += 1;
            let Some(tx) = crate::schluessel::abrechnung_unterschreiben(
                self.kontoschluessel.as_ref(),
                &self.konfig.name,
                &self.kette.netzwert(),
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
        // ⚑ Eine Nachforderung, die nach zehn Sekunden nicht zurück ist,
        //   gilt als verloren (Fund 588); dann die nächste Gegenstelle.
        self.nachforderungen.retain(|n| n.seit.elapsed() <= Duration::from_secs(10));
        self.weiter_nachfordern(None);
        self.stille_pruefen();
        // Eine weitergereichte Shardanfrage ohne Antwort nach der Frist der
        // Leitung gilt als verloren; die Leitung hat dann schon aufgegeben.
        self.shardrelais.retain(|_, (_, _, seit)| seit.elapsed() <= crate::relais::FRIST);
        self.abzug_arbeit();
        self.eigenen_abzug_melden();
        self.vorrang_pflegen();
        self.nachrechnung_pflegen();
        self.groessen_protokollieren();
        self.netzarbeit().await;
        self.pruefpunkt_pflegen().await;
        let z = self.zustand().await;
        let mut eintrag = Eintrag::neu("aufnahme")
            .zahl("peers", z.peers as i64)
            .zahl("hoehe", self.kette.hoehe() as i64)
            .text("zustandswurzel", kurz(&self.kette.wurzel()))
            .zahl("wartend", self.kette.wartend() as i64)
            .zahl("voraus", self.voraus.len() as i64)
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
        // Wer unter der Gossip-Schwelle steht, beim Namen (Fund 618).
        if !z.schlecht_bewertete.is_empty() {
            let namen: Vec<String> = z.schlecht_bewertete.iter().map(|p| p.to_string()).collect();
            eintrag = eintrag.text("schlecht_bewertete", namen.join(","));
        }
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
        // ⚑ **Was der Knoten fuer das Netz haelt, und was er daran hat.**
        // Gezaehlt wird auf der Platte, bei jeder Aufnahme neu.
        let speicher = self.speicher_zaehlen();
        let guthaben = self.kette.konto(&self.eigenes_konto()).balance;
        let (auszahlung_guthaben, auszahlung_eingetragen) = self.auszahlung_stand();
        let (bearbeitet, abgelehnt) = self.inferenz_zaehler.stand();
        if let Some(s) = &speicher {
            eintrag = eintrag
                .zahl("speicher_orte", s.orte as i64)
                .zahl("speicher_gegenstaende", s.belegung.gegenstaende as i64)
                .zahl("speicher_shards", s.belegung.shards as i64)
                .zahl("speicher_belegt_bytes", s.belegung.bytes.min(i64::MAX as u64) as i64)
                .zahl("speicher_grenze_bytes", s.grenze_bytes.min(i64::MAX as u64) as i64);
        }
        eintrag = eintrag
            .zahl("inferenz_bearbeitet", bearbeitet as i64)
            .zahl("inferenz_abgelehnt", abgelehnt as i64)
            .zahl("gedrosselt", std::mem::take(&mut self.gedrosselt) as i64)
            .zahl("konto_guthaben", guthaben.min(i64::MAX as u64) as i64);
        if self.konfig.konto.is_some() {
            eintrag = eintrag
                .zahl("auszahlung_guthaben", auszahlung_guthaben.min(i64::MAX as u64) as i64)
                .wahr("auszahlung_eingetragen", auszahlung_eingetragen);
        }
        // Gebühren und Prüfpunkt, für die Auswertung eines Laufs (G1, 7.2).
        let (grundpreis, gebuehren_epoche, gebuehren_validatoren, treasury, bestaetigt) = {
            let z = self.kette.zustand();
            (
                z.grundpreis_je_byte.max(self.konfig.netz.gebuehren.boden_je_byte),
                z.gebuehren_epoche,
                z.gebuehren_validatoren,
                z.account(&myl_types::treasury::treasury_adresse()).balance,
                z.pruefpunkt_bestaetigt,
            )
        };
        // Der Credit-Preis und die Zwillinge (Stufe 1), 2026-10-28.
        let (credit_preis, einig, uneinig, zurueck) = {
            let z = self.kette.zustand();
            (z.credit_price, z.zwillinge_einig, z.zwillinge_uneinig, z.zurueckgehalten.len())
        };
        eintrag = eintrag
            .zahl("grundpreis_je_byte", grundpreis.min(i64::MAX as u64) as i64)
            .zahl("gebuehren_validatoren", gebuehren_validatoren.min(i64::MAX as u64) as i64)
            .zahl("pruefpunkt_bestaetigt", bestaetigt.min(i64::MAX as u64) as i64)
            .zahl("credit_preis", credit_preis.min(i64::MAX as u64) as i64)
            .zahl("zwillinge_einig", einig.min(i64::MAX as u64) as i64)
            .zahl("zwillinge_uneinig", uneinig.min(i64::MAX as u64) as i64)
            .zahl("rueckbuchungen", self.kette.zustand().rueckbuchungen.min(i64::MAX as u64) as i64)
            // Gutschriften eines Pods allein, bis nach der Streitfrist (W6).
            .zahl("zurueckgehalten", zurueck as i64);
        self.protokoll.schreibe(eintrag);

        // ⚑ **Und die Kontraktabschrift für die eigene Tür** (B6-3),
        // falls seit dem letzten Block nichts nachgezogen hat.
        self.abschrift_nachziehen();

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
            nachforderung_laeuft: !self.nachforderungen.is_empty(),
            speicher_orte: speicher.as_ref().map(|s| s.orte).unwrap_or(0),
            speicher_gegenstaende: speicher.as_ref().map(|s| s.belegung.gegenstaende).unwrap_or(0),
            speicher_shards: speicher.as_ref().map(|s| s.belegung.shards).unwrap_or(0),
            speicher_unlesbar: speicher.as_ref().map(|s| s.belegung.unlesbar).unwrap_or(0),
            speicher_belegt_bytes: speicher.as_ref().map(|s| s.belegung.bytes).unwrap_or(0),
            speicher_grenze_bytes: speicher.as_ref().map(|s| s.grenze_bytes).unwrap_or(0),
            speicher_zugesagt_bytes: self.zugesagt_bytes,
            inferenz_bearbeitet: bearbeitet,
            inferenz_abgelehnt: abgelehnt,
            konto_guthaben: guthaben,
            auszahlung_guthaben,
            auszahlung_eingetragen: u64::from(auszahlung_eingetragen),
            grundpreis_je_byte: grundpreis,
            gebuehren_epoche,
            gebuehren_validatoren,
            treasury_guthaben: treasury,
            pruefpunkt_bestaetigt: bestaetigt,
            credit_preis,
            zwillinge_einig: einig,
            zwillinge_uneinig: uneinig,
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
        self.gehoert.hoeren(&von, fremde_hoehe, std::time::Instant::now());
        self.hoechste_gehoerte = self.gehoert.hoechste();
        self.nachfordern(Some(von), fremde_hoehe.min(self.hoechste_gehoerte));
    }

    /// ⚑ **Weiter nachfordern, solange eine höhere Kette gehört wurde**
    /// (Fund 588, 2026-10-22, gefunden im Netzlauf).
    ///
    /// 📌 **Bis dahin forderte ein Knoten nur nach, wenn ein Block über
    /// Gossip kam, und nur bei dem, der ihn weiterreichte.** Kamen Blöcke
    /// in falscher Folge und war der Weiterreicher selbst zurück, antwortete
    /// er mit nichts, und danach fragte niemand mehr: Der Erzeuger hatte
    /// alle Blöcke schon verbreitet. Im Netzlauf blieb so die Hälfte der
    /// Knoten bei Höhe 600 von 3660 stehen, mit allen Blöcken im Ohr.
    ///
    /// Jetzt: Wer etwas geliefert hat, wird weiter gefragt; wer nichts hatte,
    /// macht Platz für die nächste verbundene Gegenstelle, reihum.
    fn weiter_nachfordern(&mut self, bevorzugt: Option<libp2p::PeerId>) {
        let mut ziel = self.hoechste_gehoerte;
        // Zustandslos nur bis zum Rand des Wartefensters: Was darüber liegt,
        // würde geholt und gleich verworfen.
        if self.kette.ist_zustandslos() {
            ziel = ziel.min(self.kette.hoehe() + WARTENDE_BLOECKE);
        }
        self.nachfordern(bevorzugt, ziel);
    }

    /// Füllt die laufenden Nachforderungen bis [`NACHHOLER`] auf: je
    /// Gegenstelle eine, Bereich nach Bereich ab der eigenen Höhe oder hinter
    /// dem letzten laufenden. Eine Gegenstelle, die gerade für die
    /// Hintergrundprüfung oder einen Abzug gefragt ist, bleibt frei: Ihre
    /// Antworten tragen keine Marke, die sie unterschiede.
    fn nachfordern(&mut self, bevorzugt: Option<libp2p::PeerId>, ziel: u64) {
        // Solange ein Abzug geholt wird, nicht von Genesis an nachholen:
        // Die Kettendatei muss leer sein, wenn er kommt.
        if self.abzug_holen.is_some() || self.anker_suchen.is_some() {
            return;
        }
        let mut erster = bevorzugt;
        let mut versuche = 0usize;
        while self.nachforderungen.len() < NACHHOLER && versuche <= self.gegenstellen.len() {
            versuche += 1;
            // 📌 **Ab dem kleinsten Block, den noch niemand deckt** (2026-10-22).
            //    Hier stand zuerst „hinter dem höchsten laufenden Bereich“.
            //    Ging die vorderste Lieferung verloren, schloss danach niemand
            //    mehr die Lücke vor den späteren, und die Kette rückte nie vor
            //    (netem, ein Knoten nach dem Absturz bei 1 217 von 5 926).
            let laufend: Vec<(u64, u64)> = self.nachforderungen.iter().map(|n| (n.ab, n.bis)).collect();
            let Some((ab, bis)) =
                crate::nachschub::naechster_bereich(self.kette.hoehe(), &laufend, |h| self.voraus.hat_hoehe(h) || self.wartet_auf_zeugen.contains_key(&h), ziel)
            else {
                return;
            };
            // 📌 **Nicht, wer eben nichts hatte** (Fund 626). Eine
            //    Gegenstelle, die selbst zurückliegt, antwortet sofort mit
            //    nichts, ist damit als erste wieder frei und bekam jedes Mal
            //    den vordersten Bereich; die einzige, die ihn hatte, war mit
            //    späteren beschäftigt, die der volle Vorausspeicher verwarf.
            //    Nach einem Totalausfall stand ein Einsteiger so 240 Runden
            //    lang auf derselben Höhe (Archivlauf, 2026-10-24).
            let belegt = |p: &libp2p::PeerId, k: &Self| {
                k.nachforderungen.iter().any(|n| n.an == *p)
                    || k.leer_geantwortet.get(p).is_some_and(|(h, t)| *h <= ab && t.elapsed() < LEER_SPERRE)
                    || k.hintergrund_bei(p)
                    || k.abzug_bei(p)
                    || k.abruf_bei(p)
                    || k.teilabruf_bei(p)
            };
            let peer = match erster.take().filter(|p| !belegt(p, self)) {
                Some(p) => p,
                None => {
                    let frei: Vec<libp2p::PeerId> = self.gegenstellen.iter().filter(|p| !belegt(p, self)).copied().collect();
                    if frei.is_empty() {
                        return;
                    }
                    self.nachfrage_reihum = self.nachfrage_reihum.wrapping_add(1);
                    frei[self.nachfrage_reihum % frei.len()]
                }
            };
            if !self.anfrage_senden(peer, &Nachforderung::Bloecke { ab, bis }) {
                return;
            }
            self.nachforderungen.push(Nachforderung1 { an: peer, ab, bis, seit: std::time::Instant::now() });
            self.protokoll.schreibe(
                Eintrag::neu("nachschub_angefordert")
                    .text("bei", peer.to_string())
                    .zahl("ab", ab as i64)
                    .zahl("bis", bis as i64)
                    .zahl("eigene_hoehe", self.kette.hoehe() as i64)
                    .zahl("laufend", self.nachforderungen.len() as i64),
            );
        }
    }

    /// ⚑ **Nachfrage bei Stille** (Fund 618, 2026-10-25). Ein Knoten erfuhr
    /// nur über Gossip, dass er zurücklag. Bekam er keines mehr (im Netzlauf
    /// bewertete ihn der Erzeuger unter der Gossip-Schwelle) und kamen danach
    /// keine neuen Blöcke, stand er für immer, mit lebenden Verbindungen.
    /// Jetzt fragt er selbst ab der eigenen Höhe, wenn sie
    /// [`STILLE_NACHFRAGE`] lang stand und keine Nachforderung läuft; liefert
    /// die Gegenstelle, fragt er weiter, bis sie nichts mehr hat. Eine
    /// Behauptung (Fund 634) entsteht dabei nicht; die Sperre für leere
    /// Antworten (Fund 626) und die Drossel (Fund 632) gelten wie sonst.
    fn stille_pruefen(&mut self) {
        if self.stille_bei.is_some_and(|p| !self.nachforderungen.iter().any(|n| n.an == p)) {
            self.stille_bei = None;
        }
        let hoehe = self.kette.hoehe();
        if hoehe != self.zuletzt_gewachsen.0 {
            self.zuletzt_gewachsen = (hoehe, std::time::Instant::now());
            return;
        }
        if self.konfig.erzeugt_bloecke
            || self.zuletzt_gewachsen.1.elapsed() < STILLE_NACHFRAGE
            || !self.nachforderungen.is_empty()
        {
            return;
        }
        self.zuletzt_gewachsen.1 = std::time::Instant::now();
        self.stille_nachfragen(None);
    }

    /// Eine Nachforderung ab der eigenen Höhe, ohne gehörtes Ziel.
    fn stille_nachfragen(&mut self, bevorzugt: Option<libp2p::PeerId>) {
        let vorher = self.nachforderungen.len();
        let hoehe = self.kette.hoehe();
        self.nachfordern(bevorzugt, hoehe + crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG);
        self.stille_bei = self.nachforderungen.get(vorher).map(|n| n.an);
        if let Some(p) = self.stille_bei {
            self.protokoll.schreibe(Eintrag::neu("stille_nachfrage").text("bei", p.to_string()).zahl("ab", (hoehe + 1) as i64));
        }
    }

    /// Spielt aufgehobene Blöcke ein, solange einer an die eigene Höhe
    /// anschliesst; gibt zurück, wie viele es waren (Fund 592).
    ///
    /// Je Höhe gilt der erste Kandidat, der die Prüfung besteht; die übrigen
    /// derselben Höhe fallen weg, ebenso jeder, der nicht trägt.
    fn voraus_einspielen(&mut self) -> usize {
        let mut eingespielt = 0;
        loop {
            let kandidaten = self.voraus.naechste(self.kette.hoehe());
            let mut weiter = false;
            for b in kandidaten {
                if !weiter && self.kette.uebernimm(&b).is_ok() {
                    eingespielt += 1;
                    weiter = true;
                }
            }
            if !weiter {
                return eingespielt;
            }
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
                if self.kette.ist_zustandslos() {
                    return self.zustandslos_block(block, m.von);
                }
                // ⚑ **Die Vertrauensfrist** (2026-10-27, CONSENSUS 7.5):
                //   Liegt das Netz mehr als `VERTRAUENSFRIST_EPOCHEN` über
                //   dem zuletzt bestätigten Prüfpunkt dieses Knotens, könnte
                //   ihm jeder eine zweite Geschichte vorlegen, unterschrieben
                //   mit Schlüsseln, die nicht mehr haften. Der Knoten hält an
                //   und braucht einen jüngeren Anker. Nicht auf der
                //   Probekette, die ihren Satz ohnehin aus Block 1 lernt.
                if !self.konfig.netz.lernt_erzeuger && self.angehalten.is_none() {
                    let eigene = myl_consensus::block::epoche_fuer_hoehe(self.kette.bestaetigt_bis());
                    if !crate::kette::vertrauen_traegt(eigene, block.header.epoch) {
                        self.protokoll.schreibe(
                            Eintrag::neu("vertrauensfrist_ueberschritten")
                                .zahl("bestaetigt_epoche", eigene as i64)
                                .zahl("netz_epoche", block.header.epoch as i64),
                        );
                        self.angehalten = Some(
                            "Vertrauensfrist überschritten: der letzte bestätigte Prüfpunkt ist zu alt, ein jüngerer Anker ist nötig (--abzug)",
                        );
                        return;
                    }
                }
                match self.kette.uebernimm(&block) {
                    Ok(()) => {
                        self.protokoll.schreibe(
                            Eintrag::neu("block_uebernommen")
                                .zahl("hoehe", self.kette.hoehe() as i64)
                                .zahl("txs", block.txs.len() as i64)
                                .text("zustandswurzel", kurz(&self.kette.wurzel()))
                                .text("block", kurz(&self.kette.letzter_hash())),
                        );
                        self.voraus_einspielen();
                    }
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
                            // ⚑ Eigene Marke: Ein untragendes Zertifikat
                            // ist ein Befund über den **Erzeuger**, kein
                            // Anschlussproblem, und es darf keine
                            // Nachforderung auslösen.
                            crate::kette::KettenFehler::PruefpunktTraegtNicht => {
                                "pruefpunkt-traegt-nicht"
                            }
                            // Ein Befund über den Erzeuger.
                            crate::kette::KettenFehler::ErzeugerUnbelegt => "erzeuger-unbelegt",
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
                            // Zustandslos: Der Zeuge fehlt oder trägt nicht.
                            crate::kette::KettenFehler::ZeugeTraegtNicht(_) => "zeuge-traegt-nicht",
                            // ⚑ Ebenfalls Befunde über den Erzeuger:
                            // Wer einen Schuldspruch ohne Beleg in
                            // einen Block schreibt, versucht zu
                            // schlachten, und wer eine unsinnige
                            // Anfechtung mitschickt, versucht es auch.
                            // Beide dürfen keine Nachforderung auslösen.
                            crate::kette::KettenFehler::SchuldspruchOhneBeleg { .. } => {
                                "schuldspruch-ohne-beleg"
                            }
                            crate::kette::KettenFehler::AnfechtungUnsinnig => {
                                "anfechtung-unsinnig"
                            }
                        };
                        self.protokoll.schreibe(
                            Eintrag::neu("block_abgelehnt")
                                .zahl("eigene_hoehe", self.kette.hoehe() as i64)
                                .zahl("fremde_hoehe", block.header.height as i64)
                                .text("ablehnungsart", ablehnungsart)
                                .text("grund", grund.to_string()),
                        );
                        // Ein zweiter Block an einer Höhe, die wir schon
                        // haben: vielleicht eine Doppelunterschrift (7.6).
                        if block.header.height <= self.kette.hoehe() {
                            self.doppelt_erzeugt_pruefen(&block);
                        }
                        // Passt der Block nicht an und ist er **weiter**
                        // als wir, fehlt uns etwas. Dann fragen wir den,
                        // von dem der Hinweis kam: Er hat den Block, also
                        // hat er mit hoher Wahrscheinlichkeit auch die
                        // davor.
                        if ablehnungsart == "passt-nicht-an" {
                            // 📌 Fund 592: Ein Block, der zu früh kommt, wird
                            //    aufgehoben; die Nachlieferung schliesst die
                            //    Lücke bis zu ihm, und er folgt ohne eigenen
                            //    Hin- und Rückweg.
                            let _ = self.voraus.ablegen(self.kette.hoehe(), block.clone(), &m.data);
                            self.fordere_nach(m.von, block.header.height);
                        }
                        // ⚑ **Nur ein Block, der vorn nicht anschliesst, sagt
                        //   etwas über die Höhe** (Fund 634), und nur er wird
                        //   über die Nachforderung belegt oder widerlegt. Ein
                        //   Block mit falscher Höhe, Epoche oder Wurzel ist ein
                        //   Befund über seinen Absender; bis zum 2026-10-24
                        //   zählte auch seine Höhe, und ein einziger erfundener
                        //   hielt den Knoten für immer „nicht bereit“.
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
            // ⚑ Ein Zeugenstück: Der Netzfaden hat Unterschrift und
            //   Erzeuger schon geprüft.
            GossipTopic::Zeugen => {
                if let Ok(s) = borsh::from_slice::<myl_consensus::zeuge::Zeugenstueck>(&m.data) {
                    self.zeugen_quelle(m.von);
                    self.zeugenstueck_erhalten(s);
                }
            }
            GossipTopic::LatencyAttests => {
                self.protokoll.schreibe(
                    Eintrag::neu("attest_angenommen")
                        .zahl("bytes", m.data.len() as i64)
                        .text("von", m.von.to_string()),
                );
            }
            GossipTopic::Consensus => {
                // ⚑ Eine Prüfpunktstimme gehört zu keiner Runde und geht an
                //   die Sammlung der Prüfpunkte (2026-10-27), alles andere an
                //   die BFT-Runde.
                if let Ok(myl_consensus::bft::Konsensnachricht::Pruefpunktstimme(s)) =
                    borsh::from_slice::<myl_consensus::bft::Konsensnachricht>(&m.data)
                {
                    self.pruefpunktstimme_aufnehmen(s);
                    return;
                }
                let raus = self.nimm_konsens_an(&m.data);
                self.konsens_ausgang.extend(raus);
            }
            // ⚑ Gezählt statt je Zeile (Skalierung, 2026-10-29): Bei 2000
            //   Transaktionen je Sekunde schrieb jeder Knoten zwei Zeilen je
            //   Transaktion; die Zähler gehen sekündlich hinaus
            //   ([`Self::tx_zaehler_schreiben`]).
            // ⚑ **Erst geprüft, dann im Mempool** (Skalierung, 2026-10-29):
            //   Bis hierher kam jede Transaktion aus dem Gossip ungeprüft in
            //   den Mempool; ungültige verdrängten gültige. Jetzt sammelt der
            //   Knoten sie kurz und prüft sie als Stapel in einer
            //   Rechenaufgabe ([`Self::tx_puffer_pflegen`]).
            GossipTopic::Transactions => match borsh::from_slice::<myl_consensus::block::Transaktion>(&m.data) {
                Ok(tx) => self.tx_puffer.push(tx),
                Err(_) => self.tx_zaehler.abgewiesen += 1,
            },
            _ => {}
        }
    }

    /// **Den Puffer der Transaktionen prüfen**: alle 20 ms oder ab 256
    /// gesammelten als Stapel in einer Rechenaufgabe (`pruefe_alle`); was
    /// zurückkommt, geht in den Mempool.
    fn tx_puffer_pflegen(&mut self) {
        if self.tx_puffer.is_empty() || (self.tx_puffer.len() < 256 && self.tx_puffer_seit.elapsed() < Duration::from_millis(20)) {
            if self.tx_puffer.is_empty() {
                self.tx_puffer_seit = std::time::Instant::now();
            }
            return;
        }
        let stapel = std::mem::take(&mut self.tx_puffer);
        self.tx_puffer_seit = std::time::Instant::now();
        let netz = self.kette.netzwert();
        let kanal = self.tx_geprueft_kanal.clone();
        tokio::task::spawn_blocking(move || {
            let gut: Vec<myl_consensus::block::GepruefteTransaktion> =
                myl_consensus::block::pruefe_alle(&stapel, &netz).into_iter().flatten().collect();
            let _ = kanal.send(gut);
        });
    }

    /// Ein geprüfter Stapel von der Relaisleitung: aufnehmen, verbreiten,
    /// antworten.
    fn relais_geprueft_aufnehmen(&mut self, (gut, n, rueck): RelaisGeprueft) {
        let mut angenommen = 0usize;
        for g in gut {
            if self.tx_verbreiten_geprueft(g) {
                angenommen += 1;
            }
        }
        let _ = rueck.send(myl_types::relais::Relaisantwort::Erledigt(format!("angenommen {angenommen} von {n}")));
    }

    /// Geprüfte Transaktionen in den Mempool.
    fn geprueft_aufnehmen(&mut self, gepruefte: Vec<myl_consensus::block::GepruefteTransaktion>) {
        for g in gepruefte {
            if self.kette.aufnehmen_geprueft(g) {
                self.tx_zaehler.aufgenommen += 1;
            } else {
                self.tx_zaehler.abgewiesen += 1;
            }
        }
    }

    /// Schreibt die Zähler der Transaktionen, wenn seit dem letzten Mal eine
    /// Sekunde vergangen ist, und beginnt neu.
    fn tx_zaehler_schreiben(&mut self) {
        let z = &self.tx_zaehler;
        if z.seit.elapsed() < Duration::from_secs(1) {
            return;
        }
        if z.empfangen > 0 {
            self.protokoll.schreibe(
                Eintrag::neu("tx_zaehler")
                    .zahl("empfangen", z.empfangen as i64)
                    .zahl("aufgenommen", z.aufgenommen as i64)
                    .zahl("abgewiesen", z.abgewiesen as i64)
                    .zahl("wartend", self.kette.wartend() as i64),
            );
        }
        self.tx_zaehler = TxZaehler::default();
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
                // ⚑ Transaktionen: die ersten [`TX_ZEILEN_JE_SEKUNDE`] je
                //   Sekunde einzeln (die Auswertung gleicht Proben ab), was
                //   darüber kommt, nur gezählt.
                let einzeln = m.topic != GossipTopic::Transactions || {
                    self.tx_zaehler_schreiben();
                    self.tx_zaehler.empfangen += 1;
                    self.tx_zaehler.empfangen <= TX_ZEILEN_JE_SEKUNDE
                };
                if einzeln {
                    let eintrag = Eintrag::neu("empfangen")
                        .text("topic", format!("{:?}", m.topic))
                        .text("kennung", probe)
                        .text("digest", nutzlast_digest(&m.data))
                        .zahl("bytes", m.data.len() as i64);
                    self.protokoll.schreibe(eintrag);
                }
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
                let anfrage = Nachforderung::aus_bytes(&daten);
                // ⚑ **Teure Anfragen gehen durch die Drossel** (Fund 632):
                //   je Gegenstelle und für alle zusammen; darüber gibt es
                //   `Nichts`, und das kostet nichts.
                let art = match &anfrage {
                    Some(Nachforderung::Bloecke { .. } | Nachforderung::Zeugenstuecke(_) | Nachforderung::Kontobeweise(_)) => {
                        Some(crate::drossel::Art::Bloecke)
                    }
                    Some(Nachforderung::LeichterAbzug { .. }) => Some(crate::drossel::Art::Archiv),
                    Some(Nachforderung::Archiv { .. } | Nachforderung::Teil { .. }) => Some(crate::drossel::Art::Archiv),
                    _ => None,
                };
                let gedrosselt = art.is_some_and(|a| !self.drossel.erlaubt(a, &von, std::time::Instant::now()));
                if gedrosselt {
                    self.gedrosselt += 1;
                }
                let sofort = match anfrage.filter(|_| !gedrosselt) {
                    None if gedrosselt => Some((Nachlieferung::Nichts, 0)),
                    Some(Nachforderung::Bloecke { ab, bis }) => {
                        let bloecke = crate::nachschub::in_die_antwort(self.kette.bloecke_von_bis(ab, bis));
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
                        let zaehler = self.inferenz_zaehler.clone();
                        let platz = self.inferenz_plaetze.nehmen();
                        match self.ortsanschluss.clone().zip(platz) {
                            Some((anschluss, platz)) => {
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
                                    drop(platz);
                                    zaehler.zaehlen(&antwort);
                                    if let Some(bytes) = Nachlieferung::Inferenz(antwort).als_bytes()
                                    {
                                        let _ = sender
                                            .send(NodeCommand::Antwort { marke, daten: bytes });
                                    }
                                });
                                None
                            }
                            // Ohne Anschluss oder ohne freien Platz: sofort
                            // abgelehnt (Fund 609).
                            None => {
                                let antwort = myl_types::inferenzauftrag::Inferenzantwort::Abgelehnt {
                                    sitzung,
                                };
                                zaehler.zaehlen(&antwort);
                                Some((Nachlieferung::Inferenz(antwort), 0))
                            }
                        }
                    }
                    // ⚑ **Eine Zeitprobe an den eigenen Shard** (Stufe 4,
                    //   Frist): weitergereicht in einer eigenen Aufgabe, denn
                    //   der Shard rechnet; die Zeit nimmt, wer fragt.
                    //
                    // 📌 **Fund 609 (2026-10-23):** Hier antwortete der Knoten
                    //   jeder Gegenstelle, und jede Probe startete einen
                    //   eigenen Faden, der die Anfrage um bis zu 120 s
                    //   ueberlebte. Die Tuer des Shards bedient nacheinander:
                    //   Eine Flut fremder Proben stand vor jedem echten
                    //   Auftrag, der Miner verpasste seine Fristen, und die
                    //   ehrlichen Messenden, deren Proben hinter der Flut
                    //   warteten, unterschrieben echte Fristzeugnisse gegen
                    //   ihn. Jetzt nur fuer Mitglieder des eigenen Pods laut
                    //   Kette, und eine zugleich.
                    Some(Nachforderung::Zeitprobe { positionen }) => self.zeitprobe_beantworten(&von, marke, positionen, None, None),
                    Some(Nachforderung::ZeitprobeMitGrenze { positionen, grenze_ms }) => {
                        self.zeitprobe_beantworten(&von, marke, positionen, Some(grenze_ms), None)
                    }
                    Some(Nachforderung::ZeitprobeMitSaat { positionen, grenze_ms, saat }) => {
                        self.zeitprobe_beantworten(&von, marke, positionen, Some(grenze_ms), Some(saat))
                    }
                    // ⚑ **Eine Shardanfrage für einen Pod über Knoten**:
                    //   unverändert an den eigenen Shard, nur erlaubte
                    //   Marken, mit eigenem Platz.
                    Some(Nachforderung::Shard { kennung, anfrage }) => {
                        let abgelehnt = |grund: &str| {
                            Some((Nachlieferung::Shard { kennung, antwort: Vec::new(), fehler: Some(grund.to_string()) }, 0))
                        };
                        if !anfrage.first().is_some_and(|m| crate::nachschub::SHARDANFRAGEN_ERLAUBT.contains(m)) {
                            abgelehnt("diese Shardanfrage wird nicht weitergereicht")
                        } else {
                            match self.shardfuehrung.as_mut().and_then(|f| f.tuer()) {
                                None => abgelehnt("kein Shard an diesem Knoten"),
                                Some((adresse, ausweis)) => match self.shard_plaetze.nehmen() {
                                    None => abgelehnt("kein freier Platz"),
                                    Some(platz) => {
                                        let sender = self.kommandos.clone();
                                        tokio::task::spawn_blocking(move || {
                                            let lieferung = match crate::relais::an_den_eigenen_shard(adresse, &ausweis, &anfrage) {
                                                Ok(antwort) => Nachlieferung::Shard { kennung, antwort, fehler: None },
                                                Err(f) => Nachlieferung::Shard { kennung, antwort: Vec::new(), fehler: Some(f) },
                                            };
                                            drop(platz);
                                            if let Some(bytes) = lieferung.als_bytes() {
                                                let _ = sender.send(NodeCommand::Antwort { marke, daten: bytes });
                                            }
                                        });
                                        None
                                    }
                                },
                            }
                        }
                    }
                    // ⚑ **Ein Bündel für den Koordinator** (erste Position):
                    //   nur wenn dieser Knoten es ist und es die Prüfung
                    //   besteht.
                    Some(Nachforderung::Buendel { kennung, buendel }) => {
                        let ergebnis = borsh::from_slice::<myl_types::PoIBundle>(&buendel)
                            .map_err(|_| "kein PoI-Bündel".to_string())
                            .and_then(|b| {
                                let (koordinator, _) = crate::kette::Kette::buendel_pruefen(self.kette.zustand(), &b)?;
                                if self.miner_ich() != Some(koordinator) {
                                    return Err("dieser Knoten ist nicht der Koordinator des Pods".into());
                                }
                                self.buendel_selbst_einreichen(b)
                            });
                        Some((Nachlieferung::Buendel { kennung, fehler: ergebnis.err() }, 0))
                    }
                    // ⚑ **Eine Position übernehmen** (6.8): geprüft gegen die
                    //   Kette; der Shard startet bei der nächsten Aufnahme.
                    Some(Nachforderung::PositionUebernehmen { kennung, epoche, pod, position }) => {
                        let ergebnis = self.miner_ich().ok_or_else(|| "kein Miner".to_string()).and_then(|ich| {
                            crate::shardfuehrung::uebernahme_pruefen(self.kette.zustand(), &ich, epoche, pod, position)
                        });
                        let fehler = match ergebnis {
                            Ok(auftrag) => {
                                self.protokoll.schreibe(
                                    Eintrag::neu("position_uebernommen")
                                        .zahl("epoche", epoche as i64)
                                        .zahl("pod", i64::from(pod))
                                        .zahl("position", i64::from(position)),
                                );
                                self.uebernahme = Some(auftrag);
                                None
                            }
                            Err(f) => Some(f),
                        };
                        Some((Nachlieferung::Uebernahme { kennung, fehler }, 0))
                    }
                    Some(Nachforderung::Ping) => Some((Nachlieferung::Pong, 0)),
                    // ⚑ **Ein Stück eines eigenen Abzugs** (2026-10-22).
                    Some(Nachforderung::Abzug { hoehe, ab }) => {
                        Some((self.abzug_stueck(hoehe, ab).unwrap_or(Nachlieferung::Nichts), 0))
                    }
                    // ⚑ **Geschichte mit Beweis** (Archivknoten, Stufe A4):
                    //   nur, wer ab Genesis lebt und die Länge kennt.
                    Some(Nachforderung::Archiv { ab, bis, anzahl }) => match self.kette.archivlieferung(ab, bis, anzahl) {
                        Some((bloecke, beweis)) => {
                            self.protokoll.schreibe(
                                Eintrag::neu("archiv_geliefert")
                                    .text("an", von.to_string())
                                    .zahl("ab", ab as i64)
                                    .zahl("bloecke", bloecke.len() as i64),
                            );
                            Some((Nachlieferung::Archiv { bloecke, beweis }, 0))
                        }
                        None => Some((Nachlieferung::Nichts, 0)),
                    },
                    // ⚑ **Ein Teil für einen Splitter** (Stufe A7): nur gegen
                    //   die geltende Teilwurzel des laufenden Abschnitts.
                    Some(Nachforderung::Teil { grenze, stelle, teil }) => match self.kette.teilstueck(grenze, stelle, teil) {
                        Some((stueck, beweis)) => Some((Nachlieferung::Teil { grenze, stueck, beweis }, 0)),
                        None => Some((Nachlieferung::Nichts, 0)),
                    },
                    // ⚑ **Ein Stück einer Datei des Netzmodells** (P11): nur,
                    //   was das eigene Manifest nennt.
                    Some(Nachforderung::Datei { name, ab, laenge }) => {
                        let stueck = self.konfig.shard.as_ref().and_then(|sk| {
                            let manifest = crate::dateidienst::manifest_lesen(&sk.artefakte).ok()?;
                            crate::dateidienst::stueck_lesen(&sk.artefakte, &manifest, &name, ab, laenge)
                        });
                        match stueck {
                            Some((daten, groesse)) => Some((Nachlieferung::Datei { name, ab, daten, groesse }, 0)),
                            None => Some((Nachlieferung::Nichts, 0)),
                        }
                    }
                    // ⚑ **Ein Fristzeugnis eines anderen Mitglieds** (6.8).
                    Some(Nachforderung::Fristzeugnis { epoche, pod, gemeldeter, zeugnis }) => {
                        self.fristzeugnis_aufnehmen(epoche, pod, gemeldeter, zeugnis);
                        Some((Nachlieferung::Vermerkt, 0))
                    }
                    // ⚑ **Eine Spuranfrage der Stichprobe** (Fund 656): Die
                    //   Anfrage und ihre Antwort stehen im Protokoll.
                    Some(Nachforderung::Spur(a)) => {
                        // ⚑ Stufe 2: aus der Ablage, mit Beweis gegen die
                        //   Wurzel des Bündels. Kennt die Kette die Wurzel,
                        //   zählt nur eine Ablage mit genau dieser.
                        let wurzel = self
                            .kette
                            .zustand()
                            .stichprobe_wurzeln
                            .iter()
                            .find(|(p, _, _)| *p == a.pod)
                            .map(|(_, w, _)| *w);
                        let antwort = self
                            .spurablagen
                            .iter()
                            .rev()
                            .filter(|ab| wurzel.is_none_or(|w| ab.segments_root == w))
                            .find_map(|ab| ab.antwort(&a));
                        self.protokoll.schreibe(
                            Eintrag::neu("spuranfrage_eingegangen")
                                .text("von", von.to_string())
                                .zahl("epoche", a.epoche.0 as i64)
                                .text("pod", a.pod.to_hex())
                                .zahl("segment", i64::from(a.segment))
                                .text("antwort", if antwort.is_some() { "spur" } else { "nichts" }),
                        );
                        Some((antwort.map_or(Nachlieferung::Nichts, Nachlieferung::Spur), 0))
                    }
                    // ⚑ Stufe 2, Nachrechnen: der Verlauf aus der Ablage, wie
                    //   die Spur oben; zu gross für eine Antwort heisst nichts.
                    // ⚑ Ein Differenzbeweis: Ist dieser Knoten Koordinator eines
                    //   der beiden Pods, meldet er ihn.
                    Some(Nachforderung::Differenz { epoche, pod_a, pod_b, beweis }) => {
                        let ich = self.miner_ich();
                        let koordinator = self.kette.zustand().uneinigkeiten.iter().any(|u| {
                            u.stichprobe.epoche == epoche
                                && u.stichprobe
                                    .besetzung
                                    .iter()
                                    .any(|(p, b)| (*p == pod_a || *p == pod_b) && b.first().map(|(m, _, _)| *m) == ich)
                        });
                        if koordinator {
                            self.differenz_selbst_melden(epoche, pod_a, pod_b, *beweis);
                        }
                        Some((Nachlieferung::Nichts, 0))
                    }
                    // ⚑ Leichter Abzug: Anker, Stücke, Kontobeweise (2026-10-29).
                    Some(Nachforderung::LeichterAnker) => Some((Nachlieferung::LeichterAnker(self.kette.leichter_anker().map(Box::new)), 0)),
                    Some(Nachforderung::LeichterAbzug { hoehe, ab }) => {
                        let stueck = self.kette.ablage().and_then(|a| a.leicht().stueck_lesen(hoehe, ab).ok().flatten());
                        Some((
                            stueck.map_or(Nachlieferung::Nichts, |(block, daten, groesse)| Nachlieferung::Abzug { block: Box::new(block), ab, daten, groesse }),
                            0,
                        ))
                    }
                    Some(Nachforderung::Kontobeweise(adressen)) => {
                        let adressen: Vec<_> = adressen.into_iter().take(crate::nachschub::KONTOBEWEISE_JE_ANFRAGE).collect();
                        // Ohne Beweise eine leere Lieferung, nie `Nichts`.
                        let (hoehe, beweise) = self.kette.kontobeweise(&adressen).unwrap_or((0, Vec::new()));
                        Some((Nachlieferung::Kontobeweise { hoehe, beweise }, 0))
                    }
                    // ⚑ Ein Stück des Zeugen eines selbst erzeugten Blocks.
                    Some(Nachforderung::Zeugenstuecke(wuensche)) => {
                        // So viele, wie in eine Antwort passen; ein Stück
                        // ist höchstens 1 MiB, die Antwort höchstens 4.
                        let mut stuecke = Vec::new();
                        let mut bytes = 0usize;
                        for (blockhash, nr) in wuensche.into_iter().take(crate::nachschub::ZEUGENSTUECKE_JE_ANFRAGE) {
                            // ⛔️ Nur schon Unterschriebenes: Eine Nachforderung
                            //    darf den Erzeuger nicht zum Unterschreiben
                            //    zwingen (32 je Anfrage wären eine Last).
                            let Some(s) = self.kette.zeugenstuecke_gehalten(&blockhash).and_then(|s| s.get(usize::from(nr)).cloned()) else {
                                continue;
                            };
                            let n = s.bytes.len() + 4096;
                            if bytes + n > myl_types::protocol::MAX_ANFRAGE_BYTES - 4096 {
                                break;
                            }
                            bytes += n;
                            stuecke.push(s);
                        }
                        self.protokoll.schreibe(
                            Eintrag::neu("zeugenstuecke_erbeten").text("von", von.to_string()).zahl("geliefert", stuecke.len() as i64),
                        );
                        // Auch leer eine Lieferung, nie `Nichts` (siehe
                        // `Nachlieferung::LeichterAnker`).
                        Some((Nachlieferung::Zeugenstuecke(stuecke), 0))
                    }
                    // ⚑ T6d: der eigene Schiedsbeleg, wenn es ihn gibt.
                    Some(Nachforderung::Schiedsbeleg(a)) => {
                        let antwort = self
                            .schiedsbelege
                            .get(&(a.epoche.0, a.pod, a.segment))
                            .map(|(verlauf, eingaenge)| Nachlieferung::Schiedsbeleg { verlauf: verlauf.clone(), eingaenge: eingaenge.clone() })
                            .filter(|n| n.als_bytes().is_some_and(|b| b.len() < myl_types::protocol::MAX_ANFRAGE_BYTES));
                        self.protokoll.schreibe(
                            Eintrag::neu("schiedsbeleg_eingegangen")
                                .text("von", von.to_string())
                                .zahl("segment", i64::from(a.segment))
                                .text("antwort", if antwort.is_some() { "beleg" } else { "nichts" }),
                        );
                        Some((antwort.unwrap_or(Nachlieferung::Nichts), 0))
                    }
                    Some(Nachforderung::Verlauf(a)) => {
                        let wurzel = self
                            .kette
                            .zustand()
                            .stichprobe_wurzeln
                            .iter()
                            .find(|(p, _, _)| *p == a.pod)
                            .map(|(_, w, _)| *w);
                        let verlauf = self
                            .spurablagen
                            .iter()
                            .rev()
                            .filter(|ab| wurzel.is_none_or(|w| ab.segments_root == w))
                            .find_map(|ab| ab.verlauf(&a));
                        let antwort = verlauf
                            .map(Nachlieferung::Verlauf)
                            .filter(|n| n.als_bytes().is_some_and(|b| b.len() < myl_types::protocol::MAX_ANFRAGE_BYTES));
                        self.protokoll.schreibe(
                            Eintrag::neu("verlaufanfrage_eingegangen")
                                .text("von", von.to_string())
                                .zahl("epoche", a.epoche.0 as i64)
                                .text("pod", a.pod.to_hex())
                                .zahl("segment", i64::from(a.segment))
                                .text("antwort", if antwort.is_some() { "verlauf" } else { "nichts" }),
                        );
                        Some((antwort.unwrap_or(Nachlieferung::Nichts), 0))
                    }
                    Some(Nachforderung::SpurenAblegen { kennung, ablage }) => {
                        let fehler = self.spur_ablegen(ablage).err();
                        if let Some(f) = &fehler {
                            self.protokoll
                                .schreibe(Eintrag::neu("spurablage_abgewiesen").text("von", von.to_string()).text("grund", f.clone()));
                        }
                        Some((Nachlieferung::Abgelegt { kennung, fehler }, 0))
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
            NodeEvent::AntwortEingegangen { von, daten, zweck } => {
                // ⚑ **Antworten der Netzarbeit** (Stufe 4) berühren die
                //   Blocknachforderung nicht, wie die Inferenzantwort.
                match Nachlieferung::aus_bytes(&daten) {
                    // ⚑ **Zustandsabzug und Hintergrundprüfung** (2026-10-22)
                    //   zuerst: Ihre Antworten berühren das gewöhnliche
                    //   Nachholen nicht.
                    Some(Nachlieferung::Abzug { block, ab, daten, groesse }) => {
                        return self.abzug_stueck_erhalten(von, *block, ab, &daten, groesse)
                    }
                    Some(Nachlieferung::Nichts) if self.abzug_bei(&von) => return self.abzug_nichts(von),
                    Some(Nachlieferung::Nichts) if self.hintergrund_bei(&von) => return self.hintergrund_nichts(von),
                    Some(Nachlieferung::Nichts) if self.abruf_bei(&von) => return self.abruf_nichts(von),
                    Some(Nachlieferung::Nichts) if self.teilabruf_bei(&von) => return self.teilabruf_nichts(von),
                    Some(Nachlieferung::Teil { grenze, stueck, beweis }) => {
                        return self.teil_erhalten(von, grenze, stueck, beweis)
                    }
                    Some(Nachlieferung::Archiv { bloecke, beweis }) => return self.abruf_lieferung(von, bloecke, beweis),
                    Some(Nachlieferung::Bloecke(b)) if self.hintergrund_bei(&von) && self.fuer_den_hintergrund(&b) => {
                        return self.hintergrund_bloecke(von, b)
                    }
                    Some(Nachlieferung::Shard { kennung, antwort, fehler }) => {
                        return self.shard_antwort(von, kennung, antwort, fehler)
                    }
                    Some(Nachlieferung::Buendel { kennung, fehler }) => return self.buendel_antwort(von, kennung, fehler),
                    Some(Nachlieferung::Uebernahme { kennung, fehler }) => {
                        return self.relais_beantworten(von, kennung, fehler, "übernommen")
                    }
                    Some(Nachlieferung::Abgelegt { kennung, fehler }) => return self.relais_beantworten(von, kennung, fehler, "abgelegt"),
                    Some(Nachlieferung::Spur(antwort)) => return self.spur_erhalten(von, antwort),
                    Some(Nachlieferung::Verlauf(antworten)) => return self.verlauf_erhalten(von, antworten),
                    Some(Nachlieferung::Schiedsbeleg { verlauf, eingaenge }) => return self.schiedsbeleg_erhalten(verlauf, eingaenge),
                    // ⚑ Ein nachgefordertes Zeugenstück kommt nicht durch den
                    //   Netzfaden: dieselbe Prüfung hier.
                    Some(Nachlieferung::LeichterAnker(z)) => {
                        if let Some(z) = z {
                            self.leichter_anker_erhalten(von, *z);
                        }
                        return;
                    }
                    Some(Nachlieferung::Kontobeweise { hoehe, beweise }) => return self.kontobeweise_erhalten(hoehe, beweise),
                    Some(Nachlieferung::Zeugenstuecke(stuecke)) => {
                        if stuecke.is_empty() {
                            return;
                        }
                        let n = stuecke.len();
                        let mut tragend = 0usize;
                        // ⚑ Eine direkte Antwort darf auch von einem vollen
                        //   Knoten kommen, der Zeugen bereithält (2026-10-30),
                        //   aber nur von einem **eingetragenen Miner** unter
                        //   seinem eingetragenen Schlüssel: Ein Wegwerfschlüssel
                        //   kostet nichts, eine Eintragung schon. Wessen Zeuge
                        //   einmal nicht trug, der ist gesperrt. Weitergeleitet
                        //   wird so etwas nie; das Gossip bleibt bei den
                        //   Erzeugern. Je Kopf wird einmal geprüft.
                        let mut gepruefte: Vec<myl_types::Hash> = Vec::new();
                        for s in stuecke {
                            let k = s.kopf.kennung();
                            let traegt = gepruefte.contains(&k)
                                || borsh::to_vec(&s).is_ok_and(|d| self.zeugenpruefer.zeugenstueck_traegt(&d))
                                || (s.passt()
                                    && !self.zeugen_gesperrt.contains(&s.kopf.schluessel)
                                    && self.kette.zustand().miner.get(&s.kopf.erzeuger()).is_some_and(|r| r.schluessel == s.kopf.schluessel)
                                    && s.kopf.traegt());
                            if traegt {
                                if !gepruefte.contains(&k) {
                                    gepruefte.push(k);
                                }
                                tragend += 1;
                                self.zeugenstueck_erhalten(s);
                            }
                        }
                        if tragend > 0 {
                            self.zeugen_quelle(von);
                        }
                        self.protokoll.schreibe(
                            Eintrag::neu("zeugenstuecke_nachgeliefert")
                                .text("von", von.to_string())
                                .zahl("stuecke", n as i64)
                                .zahl("tragend", tragend as i64),
                        );
                        return;
                    }
                    // Seit Fund 646 misst niemand mehr mit Pings; ein Pong
                    // eines alten Knotens hat keinen Empfänger.
                    Some(Nachlieferung::Pong) => return,
                    // 📌 Fund 669/661: Gezählt wird nur die unterschriebene
                    //    Antwort auf die eigene Saat. Eine ohne oder mit
                    //    falscher Unterschrift bleibt liegen, die Probe läuft
                    //    in ihre Grenze.
                    Some(Nachlieferung::ZeitprobeUnterschrieben { abdruck, signatur }) => {
                        if self.zeitprobe_traegt(&von, &abdruck, &signatur) {
                            return self.zeitprobe_erhalten(von);
                        }
                        return;
                    }
                    Some(Nachlieferung::Zeitprobe(_)) => return,
                    Some(Nachlieferung::Datei { name, ab, daten, groesse }) => {
                        return self.stueck_erhalten(von, &name, ab, &daten, groesse)
                    }
                    Some(Nachlieferung::Vermerkt) => return,
                    // ⚑ Nach dem Zweck (2026-10-30): ein `Nichts` auf eine
                    //   Zeitprobe in die Messung, eines der Quelle des
                    //   Nachholens dorthin; jedes andere, auch das einer
                    //   gemessenen Gegenstelle auf eine Blocknachforderung,
                    //   den gewöhnlichen Weg.
                    Some(Nachlieferung::Nichts) if zweck == Some(ZWECK_ZEITPROBE) => {
                        return self.nichts_erhalten(von, "nichts", true)
                    }
                    Some(Nachlieferung::Nichts) if self.ist_nachholquelle(&von) => return self.nichts_erhalten(von, "nichts", false),
                    _ => {}
                }
                let vorher = self.kette.hoehe();
                let mut angenommen = 0usize;
                let mut abgelehnt = 0usize;
                let mut vorausgelegt = 0usize;
                let mut gewartet = 0usize;
                // ⚑ **Eine Inferenzantwort ist keine Blocklieferung**, und
                // sie darf die Nachforderung nicht als erledigt
                // buchen: Wer beides über dieselbe Schiene schickt,
                // muss beim Lesen wieder trennen, sonst hebt eine
                // Inferenzantwort die laufende Blocknachforderung auf.
                if let Some(Nachlieferung::Inferenz(antwort)) = Nachlieferung::aus_bytes(&daten) {
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
                    //
                    // ⚑ Mit mehreren Nachforderungen zugleich kommt ein
                    //   späterer Bereich auch einmal vor einem früheren an;
                    //   seine Blöcke gehen in den Vorausspeicher (Fund 592).
                    for b in bloecke {
                        if self.kette.ist_zustandslos() {
                            // ⚑ Ein Block, der neu ins Wartefenster kommt, ist
                            //   geliefert (2026-10-30): Er zählt für die
                            //   Fortsetzung wie ein angenommener. 📌 Sonst riss
                            //   die Kette des Nachforderns ab, und der nächste
                            //   Bereich kam erst mit dem nächsten Block über das
                            //   Gossip: Im Kurzlauf `kurz-ablage` folgte der
                            //   zustandslose Miner nach einer Trennung genau im
                            //   Takt des Erzeugers, 130 Blöcke dahinter, und holte
                            //   nie auf.
                            let h = b.header.height;
                            let neu = !self.wartet_auf_zeugen.contains_key(&h);
                            self.zeugen_eilig.insert(h);
                            self.zustandslos_block(b, von);
                            gewartet += usize::from(neu && self.wartet_auf_zeugen.contains_key(&h));
                            continue;
                        }
                        match self.kette.uebernimm(&b) {
                            Ok(()) => angenommen += 1,
                            Err(crate::kette::KettenFehler::PasstNichtAn { .. }) if b.header.height > self.kette.hoehe() + 1 => {
                                if let Ok(roh) = borsh::to_vec(&b) {
                                    vorausgelegt += usize::from(self.voraus.ablegen(self.kette.hoehe(), b, &roh).is_ok());
                                }
                            }
                            Err(_) => abgelehnt += 1,
                        }
                    }
                }
                // Wer nichts hatte, wird für diese Höhe eine Weile übergangen
                // (Fund 626); wer geliefert hat, wieder gefragt.
                if angenommen > 0 || vorausgelegt > 0 || gewartet > 0 {
                    self.leer_geantwortet.remove(&von);
                } else if matches!(Nachlieferung::aus_bytes(&daten), Some(Nachlieferung::Nichts)) {
                    if let Some(n) = self.nachforderungen.iter().find(|n| n.an == von) {
                        self.leer_geantwortet.insert(von, (n.ab, std::time::Instant::now()));
                        // Wer eine Höhe ab hier behauptet hat und nichts
                        // liefert, hat sie nicht (Fund 634).
                        self.gehoert.nichts_ab(&von, n.ab, std::time::Instant::now());
                        self.hoechste_gehoerte = self.gehoert.hoechste();
                    }
                }
                self.nachforderungen.retain(|n| n.an != von);
                // Zustandslos: die Zeugen der eben gelieferten Blöcke gleich.
                self.zeugen_pflegen();
                let aus_dem_voraus = self.voraus_einspielen();
                self.weiter_nachfordern((angenommen > 0 || gewartet > 0).then_some(von));
                // Wer bei Stille geliefert hat, wird weiter gefragt, bis er
                // nichts mehr hat (Fund 618).
                if self.stille_bei == Some(von) {
                    self.stille_bei = None;
                    if angenommen > 0 && self.nachforderungen.is_empty() {
                        self.stille_nachfragen(Some(von));
                    }
                }
                Eintrag::neu("nachschub_erhalten")
                    .text("von", von.to_string())
                    .zahl("angenommen", angenommen as i64)
                    .zahl("abgelehnt", abgelehnt as i64)
                    .zahl("aus_dem_voraus", aus_dem_voraus as i64)
                    .zahl("vorausgelegt", vorausgelegt as i64)
                    .zahl("gewartet", gewartet as i64)
                    .zahl("hoehe_vorher", vorher as i64)
                    .zahl("hoehe_nachher", self.kette.hoehe() as i64)
            }
            NodeEvent::AnfrageGescheitert { an, grund, .. } if self.shardrelais.values().any(|(p, _, _)| *p == an) => {
                self.shardrelais_scheitern(&an, &grund);
                Eintrag::neu("shardrelais_gescheitert").text("an", an.to_string()).text("grund", grund)
            }
            // 📌 **Fund 674 (2026-10-28): Nicht erreicht ist keine Weigerung.**
            //    Ein Knoten hält höchstens `MAX_AUSGEHEND` Verbindungen; sind
            //    sie belegt, scheitert das Wählen zu einem Mitglied, mit dem
            //    er noch nicht verbunden ist. Das zählte wie eine Abweisung,
            //    und mit 29 Knoten bekamen ehrliche Miner Zeugnisse. Jetzt
            //    wird später neu gefragt, ohne zu zählen.
            NodeEvent::AnfrageGescheitert { an, grund, zweck } if zweck == Some(ZWECK_ZEITPROBE) && self.messungen.contains_key(&an) && ist_waehlfehler(&grund) => {
                self.messung_unerreicht(an);
                self.nachholquelle_scheitert(an);
                Eintrag::neu("zeitprobe_unerreicht").text("an", an.to_string()).text("grund", grund)
            }
            NodeEvent::AnfrageGescheitert { an, grund, zweck } if zweck == Some(ZWECK_ZEITPROBE) || self.ist_nachholquelle(&an) => {
                self.nichts_erhalten(an, "gescheitert", zweck == Some(ZWECK_ZEITPROBE));
                Eintrag::neu("netzarbeit_gescheitert").text("an", an.to_string()).text("grund", grund)
            }
            NodeEvent::AnfrageGescheitert { an, grund, .. } => {
                self.nachforderungen.retain(|n| n.an != an);
                self.weiter_nachfordern(None);
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
            NodeEvent::Verbunden { peer, addr, eingehend } => {
                if !self.gegenstellen.contains(&peer) {
                    self.gegenstellen.push(peer);
                }
                Eintrag::neu("verbunden")
                .text("gegenstelle", peer.to_string())
                .text("addr", addr.to_string())
                .wahr("eingehend", eingehend)
                .wahr("vermittelt", ist_vermittelt(&addr))
                .wahr("quic", ist_quic(&addr))
            }
            NodeEvent::Getrennt { peer, grund } => {
                self.gegenstellen.retain(|p| *p != peer);
                self.gehoert.getrennt(&peer);
                self.hoechste_gehoerte = self.gehoert.hoechste();
                Eintrag::neu("getrennt").text("gegenstelle", peer.to_string()).text("grund", grund)
            }
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

/// ⚑ **Die Arbeit für das Netz** (Stufe 4 des Dispatchers, 2026-10-22):
/// Anmeldung und Zusage, der Shard aus der Kette (P7), die Fristmessung
/// (6.8) und das Holen fehlender Dateien (P11). Alles läuft bei der
/// Zustandsaufnahme und in den Ereignissen; nichts davon blockiert die
/// Blockverarbeitung.
impl Knoten {
    /// Die Kennung, unter der dieser Knoten als Miner angemeldet ist.
    /// ⚑ **Ein Block beim zustandslosen Knoten** (2026-10-29): Er wartet auf
    /// seinen Zeugen, wenn sein Erzeuger belegt ist und er nicht zu weit
    /// vorn liegt; je Höhe gilt der erste. Erst ein belegter Block legt
    /// fest, welcher Zeuge an seiner Höhe zählt: Sonst könnte ein
    /// gefälschter Block den echten Zeugen verdrängen.
    fn zustandslos_block(&mut self, block: myl_consensus::block::Block, von: libp2p::PeerId) {
        let hoehe = block.header.height;
        let eigene = self.kette.hoehe();
        if hoehe <= eigene || self.wartet_auf_zeugen.contains_key(&hoehe) {
            return;
        }
        // ⛔️ **Zuerst der Erzeuger**, wie im vollen Pfad: Ein Block ohne
        //    belegten Erzeuger löst nichts aus, keine Nachforderung und
        //    keine Erwartung an einen Zeugen.
        if !self.kette.erzeuger_belegt(&block) {
            self.protokoll.schreibe(Eintrag::neu("block_abgelehnt").zahl("hoehe", hoehe as i64).text("grund", "erzeuger-unbelegt"));
            return;
        }
        // Daran misst die Selbstheilung, ob es weitergeht (belegt, also nicht
        // nur behauptet).
        self.hoechster_belegter = self.hoechster_belegter.max(hoehe);
        // Eine Lücke davor: nachfordern wie im vollen Pfad.
        if hoehe > eigene + 1 && !self.wartet_auf_zeugen.contains_key(&(eigene + 1)) {
            self.fordere_nach(von, hoehe);
        }
        if hoehe > eigene + WARTENDE_BLOECKE {
            return;
        }
        // Eine Kette, die ihren Erzeuger erst lernt (Probekette), hat vor
        // ihrem ersten Block (ab Genesis oder nach einem Abzug) keinen
        // Stimmsatz: Der Erzeuger des nächsten Blocks ist der, den sie
        // lernen wird (`Kette::satz_aus_block`). ⛔️ Nur dieser und nur
        // wenige: Sonst brächte jeder unterschriebene Block seinen Schlüssel
        // in die Liste, nach der Zeugenstücke weitergeleitet werden.
        if self.kette.stimmsatz().is_none() && hoehe == eigene + 1 {
            if let (Some(u), Ok(mut l)) = (&block.unterschrift, self.erzeugerliste.write()) {
                if l.len() < myl_consensus::zeuge::KOEPFE_JE_HOEHE {
                    l.insert(u.erzeuger(), u.schluessel);
                }
            }
        }
        self.zeugensammler.erwarten(hoehe, block.hash());
        if hoehe == eigene + 1 {
            self.protokoll.schreibe(Eintrag::neu("wartet_auf_zeuge").zahl("hoehe", hoehe as i64).text("block", kurz(&block.hash())));
        }
        self.wartet_auf_zeugen.insert(hoehe, (block, std::time::Instant::now(), von));
        self.zeugen_einspielen();
    }

    /// Merkt sich eine Gegenstelle, die tragende Zeugenstücke brachte.
    fn zeugen_quelle(&mut self, von: libp2p::PeerId) {
        if self.zeugen_quellen.front() != Some(&von) {
            self.zeugen_quellen.retain(|p| *p != von);
            self.zeugen_quellen.push_front(von);
            self.zeugen_quellen.truncate(ZEUGEN_GEGENSTELLEN);
        }
    }

    /// Ein geprüftes Zeugenstück; macht es einen Zeugen fertig, wird er
    /// gelesen und, wenn sein Block dran ist, eingespielt.
    fn zeugenstueck_erhalten(&mut self, s: myl_consensus::zeuge::Zeugenstueck) {
        if s.kopf.hoehe <= self.kette.hoehe() {
            return;
        }
        // Für die Auswertung: ein Stück für den nächsten Block, das keinen
        // Zeugen fertig macht.
        let naechstes = (s.kopf.hoehe == self.kette.hoehe() + 1)
            .then(|| (s.nr, s.kopf.stuecke.len(), self.wartet_auf_zeugen.get(&s.kopf.hoehe).map(|(b, _, _)| b.hash() == s.kopf.blockhash)));
        let Some((hoehe, blockhash, bytes, unterzeichner)) = self.zeugensammler.aufnehmen(s) else {
            if let Some((nr, teile, passt)) = naechstes {
                self.protokoll.schreibe(
                    Eintrag::neu("zeugenstueck_liegt")
                        .zahl("nr", i64::from(nr))
                        .zahl("teile", teile as i64)
                        .text("block", match passt { Some(true) => "wartet", Some(false) => "anderer", None => "fehlt" }),
                );
            }
            return;
        };
        let mut rest = &bytes[..];
        let gelesen = <myl_ledger::state::Zeuge as borsh::BorshDeserialize>::deserialize(&mut rest).ok().filter(|_| rest.is_empty());
        // Für die Auswertung: was im Fenster fertig wird, und ob sein Block
        // wartet.
        if hoehe <= self.kette.hoehe() + WARTENDE_BLOECKE {
            let block = self.wartet_auf_zeugen.get(&hoehe).map(|(b, _, _)| b.hash() == blockhash);
            self.protokoll.schreibe(
                Eintrag::neu("zeuge_fertig")
                    .zahl("hoehe", hoehe as i64)
                    .zahl("bytes", bytes.len() as i64)
                    .wahr("lesbar", gelesen.is_some())
                    .text("block", match block { Some(true) => "wartet", Some(false) => "anderer", None => "fehlt" }),
            );
        }
        let Some(zeuge) = gelesen else {
            return;
        };
        self.fertige_zeugen.push_back((blockhash, zeuge, bytes.len(), unterzeichner));
        while self.fertige_zeugen.len() > FERTIGE_ZEUGEN
            || self.fertige_zeugen.iter().map(|(_, _, n, _)| n).sum::<usize>() > ZEUGEN_SAMMLER_BYTES
        {
            self.fertige_zeugen.pop_front();
        }
        self.zeugen_einspielen();
    }

    /// Spielt ein, solange der nächste Block wartet und sein Zeuge da ist.
    fn zeugen_einspielen(&mut self) {
        loop {
            let naechste = self.kette.hoehe() + 1;
            let Some((block, _, _)) = self.wartet_auf_zeugen.get(&naechste) else {
                break;
            };
            let hash = block.hash();
            let Some(i) = self.fertige_zeugen.iter().position(|(h, _, _, _)| *h == hash) else {
                break;
            };
            let (_, zeuge, _, unterzeichner) = self.fertige_zeugen.remove(i).expect("gefunden");
            let (block, _, von) = self.wartet_auf_zeugen.remove(&naechste).expect("gefunden");
            match self.kette.uebernimm_mit_zeuge(&block, &zeuge) {
                Ok(()) => {
                    self.nur_abzug = 0;
                    self.zeugensammler.bis_vergessen(naechste);
                    self.kontobeweise_einsetzen();
                    self.protokoll.schreibe(
                        Eintrag::neu("block_uebernommen")
                            .zahl("hoehe", self.kette.hoehe() as i64)
                            .zahl("txs", block.txs.len() as i64)
                            .wahr("mit_zeuge", true)
                            .text("zustandswurzel", kurz(&self.kette.wurzel()))
                            .text("block", kurz(&self.kette.letzter_hash())),
                    );
                }
                Err(grund) => {
                    // ⚑ Trug der Zeuge eines anderen als des Erzeugers nicht,
                    //   liegt es am Zeugen, nicht am Block (2026-10-30): Der
                    //   Block wartet weiter, der Unterzeichner ist gesperrt.
                    //   Sonst könnte ein Miner mit einem falschen Zeugen
                    //   einen Block aus der Warteschlange werfen.
                    let vom_erzeuger = block.unterschrift.as_ref().is_some_and(|u| u.schluessel == unterzeichner);
                    self.protokoll.schreibe(
                        Eintrag::neu("block_abgelehnt")
                            .zahl("hoehe", naechste as i64)
                            .text("grund", grund.to_string())
                            .wahr("zeuge_vom_erzeuger", vom_erzeuger),
                    );
                    if !vom_erzeuger {
                        self.zeugen_gesperrt.push_back(unterzeichner);
                        while self.zeugen_gesperrt.len() > ZEUGEN_GESPERRT {
                            self.zeugen_gesperrt.pop_front();
                        }
                        self.wartet_auf_zeugen.insert(naechste, (block, std::time::Instant::now(), von));
                    }
                    break;
                }
            }
        }
        let h = self.kette.hoehe();
        self.wartet_auf_zeugen.retain(|hoehe, _| *hoehe > h);
        self.zeugen_eilig.retain(|hoehe| *hoehe > h && *hoehe <= h + WARTENDE_BLOECKE);
        self.zeugen_angefragt.retain(|_, t| t.elapsed() < std::time::Duration::from_secs(60));
    }

    /// ⚑ **Fehlt der Zeuge eines wartenden Blocks** nach
    /// [`ZEUGE_NACHFRAGEN_NACH`], fragt der zustandslose Knoten Stück für
    /// Stück nach: beim Erzeuger, wenn er als Miner eingetragen ist, und bei
    /// dem, der den Block weiterreichte. Die Antworten prüft er wie das
    /// Gossip.
    fn zeugen_pflegen(&mut self) {
        if !self.kette.ist_zustandslos() {
            return;
        }
        self.leicht_einstieg_pflegen();
        // Während ein Abzug kommt, keine Zeugen: Der Knoten springt ohnehin.
        if self.abzug_holen.is_some() || self.anker_suchen.is_some() {
            return;
        }
        // Alle wartenden Blöcke ohne Zeugen, die lange genug warten; beim
        // Aufholen also viele zugleich, nicht einer je Frist.
        let offen: Vec<(myl_types::Hash, Vec<libp2p::PeerId>)> = self
            .wartet_auf_zeugen
            .values()
            .filter(|(block, seit, _)| {
                (seit.elapsed() >= ZEUGE_NACHFRAGEN_NACH || self.zeugen_eilig.contains(&block.header.height))
                    && !self.fertige_zeugen.iter().any(|(h, _, _, _)| *h == block.hash())
            })
            .map(|(block, _, von)| {
                let erzeuger = block.erzeuger().ok().flatten().and_then(|m| {
                    self.kette.zustand().miner.get(&m).and_then(|r| myl_net::peer_id_aus_bytes(&r.netzadresse).ok())
                });
                (block.hash(), [Some(*von), erzeuger].into_iter().flatten().collect::<Vec<_>>())
            })
            .collect();
        // Dazu einige weitere Gegenstellen: Der Weiterreicher muss den
        // Zeugen nicht haben (ein voller Knoten hält keine fremden), und den
        // Erzeuger kennt nicht jedes Netz als Miner.
        // Reihum, damit jeder Zeugenhalter einmal gefragt wird und als
        // Quelle bleibt, sobald er liefert; je Runde bleiben es höchstens
        // `ZEUGEN_GEGENSTELLEN` weitere.
        let (reihum, weiter) = reihum(&self.gegenstellen, self.zeugen_reihum, ZEUGEN_GEGENSTELLEN);
        let weitere: Vec<libp2p::PeerId> = self.zeugen_quellen.iter().copied().chain(reihum).collect();
        if !offen.is_empty() {
            self.zeugen_reihum = weiter;
        }
        let offen: Vec<(myl_types::Hash, Vec<libp2p::PeerId>)> = offen
            .into_iter()
            .map(|(h, mut an)| {
                for p in &weitere {
                    if !an.contains(p) {
                        an.push(*p);
                    }
                }
                (h, an)
            })
            .collect();
        // Je Gegenstelle eine gebündelte Anfrage (eine Verbindung trägt nur
        // `MAX_ANFRAGEN_ZUGLEICH` zugleich).
        let mut je_peer: std::collections::BTreeMap<libp2p::PeerId, Vec<(myl_types::Hash, u16)>> = Default::default();
        let jetzt = std::time::Instant::now();
        for (hash, an) in offen {
            for nr in self.zeugensammler.fehlende(&hash).unwrap_or_else(|| vec![0]) {
                let neu = self.zeugen_angefragt.get(&(hash, nr)).is_none_or(|t| t.elapsed() > ZEUGE_NACHFRAGEN_NACH);
                if !neu {
                    continue;
                }
                for p in &an {
                    let w = je_peer.entry(*p).or_default();
                    if w.len() < crate::nachschub::ZEUGENSTUECKE_JE_ANFRAGE {
                        w.push((hash, nr));
                    }
                }
                self.zeugen_angefragt.insert((hash, nr), jetzt);
            }
        }
        for (p, wuensche) in je_peer {
            let n = wuensche.len();
            if let Some(daten) = Nachforderung::Zeugenstuecke(wuensche).als_bytes() {
                let _ = self.kommandos.send(NodeCommand::Anfrage { an: p, daten });
                self.protokoll.schreibe(Eintrag::neu("zeugenstuecke_angefragt").text("an", p.to_string()).zahl("stuecke", n as i64));
            }
        }
    }

    /// ⚑ **Der Einstieg des zustandslosen Knotens** (2026-10-29): Ohne Anker
    /// fragt er nach dem jüngsten verankerten leichten Abzug; nach
    /// [`ANKER_SUCHEN_HOECHSTENS`] ohne Antwort folgt er ab Genesis. Kennt
    /// er eigene Konten nicht, fragt er nach Kontobeweisen.
    fn leicht_einstieg_pflegen(&mut self) {
        // ⚑ Selbstheilung: Stillstand trotz höherer Blöcke, und ein
        //   Stimmsatz, gegen den sich ein Anker prüfen lässt.
        let hoehe = self.kette.hoehe();
        if self.stillstand.0 != hoehe || self.abzug_holen.is_some() || self.anker_suchen.is_some() {
            self.stillstand = (hoehe, std::time::Instant::now());
        } else if self.stillstand.1.elapsed() > self.stillstand_hoechstens
            && self.hoechster_belegter > hoehe + 1
            && self.kette.stimmsatz().is_some()
        {
            self.nur_abzug = self.nur_abzug.saturating_add(1);
            self.protokoll.schreibe(
                Eintrag::neu("zustandslos_neu_verankern")
                    .zahl("hoehe", hoehe as i64)
                    .zahl("belegt_bis", self.hoechster_belegter as i64)
                    .zahl("hintereinander", i64::from(self.nur_abzug)),
            );
            if self.nur_abzug >= NUR_ABZUG_WARNUNG {
                self.protokoll.schreibe(
                    Eintrag::neu("zustandslos_nur_abzug")
                        .zahl("hintereinander", i64::from(self.nur_abzug))
                        .text("hinweis", "kein Block mit Zeugen seit den letzten Neuverankerungen; fehlen die Zeugen ab dem Pruefpunkt?"),
                );
            }
            self.anker_suchen = Some(std::time::Instant::now());
            self.anker_kandidat = None;
            self.stillstand = (hoehe, std::time::Instant::now());
        }
        if let Some(seit) = self.anker_suchen {
            if let Some((anker, _)) = self.anker_kandidat.filter(|(_, ab)| ab.elapsed() >= ANKER_SAMMELN) {
                self.anker_suchen = None;
                self.anker_kandidat = None;
                self.protokoll.schreibe(Eintrag::neu("leichter_anker").zahl("hoehe", anker.hoehe as i64));
                self.abzug_holen = Some(AbzugHolen::neu_als(anker, crate::abzug::Art::Leicht));
                self.abzug_anfragen();
            } else if seit.elapsed() > ANKER_SUCHEN_HOECHSTENS && self.anker_kandidat.is_none() {
                self.anker_suchen = None;
                self.protokoll.schreibe(Eintrag::neu("leichter_anker_keiner"));
                self.weiter_nachfordern(None);
            } else if let Some(daten) = Nachforderung::LeichterAnker.als_bytes() {
                for p in self.gegenstellen.iter().take(ZEUGEN_GEGENSTELLEN) {
                    let _ = self.kommandos.send(NodeCommand::Anfrage { an: *p, daten: daten.clone() });
                }
            }
        }
        let unbekannt = self.kette.eigene_unbekannt();
        if unbekannt.is_empty() || self.abzug_holen.is_some() || self.kette.hoehe() == 0 {
            return;
        }
        if self.kontobeweise_angefragt.is_some_and(|t| t.elapsed() < KONTOBEWEISE_ALLE) {
            return;
        }
        let wuensche: Vec<_> = unbekannt.into_iter().take(crate::nachschub::KONTOBEWEISE_JE_ANFRAGE).collect();
        if let Some(daten) = Nachforderung::Kontobeweise(wuensche).als_bytes() {
            for p in self.gegenstellen.iter().take(2) {
                let _ = self.kommandos.send(NodeCommand::Anfrage { an: *p, daten: daten.clone() });
            }
            self.kontobeweise_angefragt = Some(std::time::Instant::now());
        }
    }

    /// Ein Zertifikat als Anker: nur, wenn es gegen den eigenen Stimmsatz
    /// trägt und der Knoten noch einen sucht.
    /// Gesammelt wird ([`ANKER_SAMMELN`]), genommen der jüngste; einer
    /// unter der eigenen Höhe zählt nicht (die Selbstheilung springt nur
    /// vorwärts).
    fn leichter_anker_erhalten(&mut self, von: libp2p::PeerId, z: myl_consensus::pruefpunkt::Pruefpunktzertifikat) {
        if self.anker_suchen.is_none() || self.abzug_holen.is_some() || z.hoehe <= self.kette.hoehe() {
            return;
        }
        let Some(anker) = self.kette.anker_aus_zertifikat(&z) else {
            self.protokoll.schreibe(Eintrag::neu("leichter_anker_abgelehnt").text("von", von.to_string()).zahl("hoehe", z.hoehe as i64));
            return;
        };
        match &mut self.anker_kandidat {
            Some((a, _)) if a.hoehe >= anker.hoehe => {}
            Some((a, _)) => *a = anker,
            None => self.anker_kandidat = Some((anker, std::time::Instant::now())),
        }
    }

    /// Für Tests: Der Erzeuger hält seine Zeugen zurück.
    #[doc(hidden)]
    pub fn zeugen_zurueckhalten(&mut self, ja: bool) {
        self.zeugen_zurueckhalten = ja;
    }

    /// Für Tests: wie lange Stillstand, bis neu verankert wird.
    #[doc(hidden)]
    pub fn stillstand_hoechstens_setzen(&mut self, d: std::time::Duration) {
        self.stillstand_hoechstens = d;
    }

    /// Kontobeweise: gleich einsetzen, wenn sie zur eigenen Höhe gehören,
    /// sonst halten, bis die eigene sie erreicht; ältere fallen weg.
    ///
    /// ⛔️ **Gehalten wird die niedrigste ab der eigenen Höhe**, und höchstens
    /// [`KONTOBEWEISE_VORAUS`] Höhen voraus: Hielte er die höchste, belegte
    /// eine Gegenstelle mit einer erfundenen fernen Höhe den Platz für immer,
    /// und der Knoten erführe sein Konto nie. Eine falsche Lieferung kostet
    /// so nur einen weiteren Versuch.
    fn kontobeweise_erhalten(&mut self, hoehe: u64, beweise: Vec<myl_ledger::state::Kontobeweis>) {
        let eigene = self.kette.hoehe();
        if !self.kette.ist_zustandslos() || hoehe < eigene || hoehe > eigene + KONTOBEWEISE_VORAUS {
            return;
        }
        if self.kontobeweise_offen.as_ref().is_none_or(|(h, _)| hoehe <= *h || *h < eigene) {
            self.kontobeweise_offen = Some((hoehe, beweise));
        }
        self.kontobeweise_einsetzen();
    }

    /// Für Tests: eine Lieferung von Kontobeweisen einspeisen.
    #[doc(hidden)]
    pub fn kontobeweise_einspeisen(&mut self, hoehe: u64, beweise: Vec<myl_ledger::state::Kontobeweis>) {
        self.kontobeweise_erhalten(hoehe, beweise);
    }

    /// Für Tests: die Höhe der gehaltenen Kontobeweise.
    #[doc(hidden)]
    pub fn kontobeweise_gehalten(&self) -> Option<u64> {
        self.kontobeweise_offen.as_ref().map(|(h, _)| *h)
    }

    fn kontobeweise_einsetzen(&mut self) {
        let Some((hoehe, beweise)) = self.kontobeweise_offen.take() else {
            return;
        };
        match self.kette.kontobeweise_einsetzen(hoehe, &beweise) {
            Ok(n) => self.protokoll.schreibe(Eintrag::neu("kontobeweise").zahl("hoehe", hoehe as i64).zahl("eingesetzt", n as i64)),
            Err(eigene) if eigene < hoehe => self.kontobeweise_offen = Some((hoehe, beweise)),
            Err(_) => {}
        }
    }

    fn miner_ich(&self) -> Option<myl_types::ids::MinerId> {
        self.netzschluessel.as_ref().map(|k| k.kennung())
    }

    /// Die Kennung als Miner, wenn dieser Knoten zusagt oder rechnet.
    pub fn miner_kennung(&self) -> Option<myl_types::ids::MinerId> {
        self.miner_ich()
    }

    /// Der Shard, den dieser Knoten gerade aus der Kette führt.
    pub fn shardauftrag(&self) -> Option<crate::shardfuehrung::Shardauftrag> {
        self.shardfuehrung.as_ref().and_then(|f| f.auftrag()).cloned()
    }

    /// Die Prozessnummer dieses Shards.
    pub fn shard_pid(&self) -> Option<u32> {
        self.shardfuehrung.as_ref().and_then(|f| f.pid())
    }

    /// ⚑ **Vorrang für die Pod-Mitglieder** (Fund 674, N1): je Epoche die
    /// Gegenstellen, die nicht gegen die Verbindungsgrenzen zählen. Ein
    /// Miner braucht die Mitglieder seines Pods (Zeitprobe, Spuren), ein
    /// Torknoten die aller Pods (Shardverkehr über die Relaisleitung), der
    /// eigene Pod zuerst. Gedeckelt in `myl-net` (`MAX_VORRANG`).
    fn vorrang_pflegen(&mut self) {
        let z = self.kette.zustand();
        let epoche = z.epoch.0;
        if self.vorrang.as_ref().is_some_and(|(e, _)| *e == epoche) {
            return;
        }
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(z);
        let ich = self.miner_ich();
        let mein_pod = |p: &&myl_scheduler::shard_assignment::Pod| ich.is_some_and(|i| p.mitglieder().any(|m| m.miner_id == i));
        let mut pods: Vec<&myl_scheduler::shard_assignment::Pod> = zuteilung.pods.iter().filter(mein_pod).collect();
        // Mit Relaisleitung bedient er Pods für einen Ortsdienst (Torknoten).
        if self.konfig.podrelais.is_some() {
            pods.extend(zuteilung.pods.iter().filter(|p| !mein_pod(p)));
        }
        let mut peers: Vec<libp2p::PeerId> = Vec::new();
        for m in pods.iter().flat_map(|p| p.mitglieder()) {
            if ich == Some(m.miner_id) {
                continue;
            }
            if let Ok(p) = myl_net::peer_id_aus_bytes(&m.netzadresse) {
                if p != self.peer_id && !peers.contains(&p) {
                    peers.push(p);
                }
            }
        }
        let anzahl = peers.len();
        if self.kommandos.send(NodeCommand::Vorrang { peers: peers.clone() }).is_ok() {
            self.protokoll.schreibe(Eintrag::neu("vorrang").zahl("epoche", epoche as i64).zahl("peers", anzahl as i64));
            self.vorrang = Some((epoche, peers));
        }
    }

    /// Je Aufnahme: Zusage pflegen, Shard abgleichen, messen, holen.
    async fn netzarbeit(&mut self) {
        let Some(ich) = self.miner_ich() else {
            return;
        };
        // Die eigene Fristsperre steht im Protokoll: Von außen ist sonst nur
        // zu sehen, dass gemeldet wurde, nicht, dass die Kette sperrte.
        if let Some(e) = self.kette.zustand().fristsperren.get(&ich).map(|e| e.0) {
            if self.fristsperre_gemeldet != Some(e) {
                self.fristsperre_gemeldet = Some(e);
                self.protokoll.schreibe(Eintrag::neu("fristsperre").zahl("epoche", e as i64));
            }
        }
        // Das Polster für Gebühren nachfüllen, wo ein Probehahn ist (G1).
        if self.kette.zustand().miner.contains_key(&ich) {
            self.probehahn_pflegen(&ich);
        }
        self.zusage_pflegen(&ich);
        self.einsatz_pflegen(&ich);
        self.auszahlung_pflegen(&ich);
        self.archiv_belegen(&ich);
        self.teilwurzel_nennen(&ich);
        self.teil_belegen(&ich);
        self.teile_holen(&ich);
        self.shard_abgleichen(&ich);
        self.fristen_messen(&ich);
        self.selbstprobe_pflegen(&ich);
        self.dateien_nachholen(&ich);
        for tx in std::mem::take(&mut self.ausstehend) {
            self.tx_verbreiten(tx);
        }
    }

    /// Unterschreibt mit dem Netzschlüssel und verbreitet; die Nummer zählt
    /// vom Konto im Zustand oder von der zuletzt vergebenen, je nachdem,
    /// welche höher ist.
    fn netz_tx(&mut self, anweisung: myl_consensus::block::Anweisung) -> Option<myl_consensus::block::Transaktion> {
        self.netz_luecke_pruefen();
        let k = self.netzschluessel.as_ref()?;
        // ⚑ Zustandslos und das eigene Konto noch unbekannt: keine Nummer
        //   raten (der Boden einer neuen Epoche läge weit darüber).
        if !self.kette.konto_bekannt(&k.adresse()) {
            return None;
        }
        let im_zustand = self.kette.konto(&k.adresse()).nonce;
        let nonce = self.netz_nonce.max(im_zustand);
        let tx = k.transaktion(&self.kette.netzwert(), nonce, anweisung).ok()?;
        self.netz_nonce = nonce + 1;
        Some(tx)
    }

    /// Ob eigene Transaktionen ausstehen: vergeben, im Zustand noch nicht
    /// verbraucht.
    fn netz_offen(&self) -> bool {
        self.netzschluessel
            .as_ref()
            .is_some_and(|k| self.netz_nonce > self.kette.konto(&k.adresse()).nonce)
    }

    /// ⚑ **Eine Lücke heilt** (Fund 627): Steht die Nummer im Zustand
    /// [`NETZ_LUECKE_HOEHEN`] Höhen lang still, während eigene Transaktionen
    /// ausstehen, ist eine verloren, und es wird ab der Nummer im Zustand
    /// neu gezählt. Bis zum 2026-10-24 zählte der Knoten nur weiter: Eine
    /// einzige verlorene Transaktion machte jede spätere ungültig, für
    /// immer, und eine Archivkopie verlor so ihren Platz. Die Regel steht in
    /// [`netz_luecke`].
    fn netz_luecke_pruefen(&mut self) {
        let Some(k) = self.netzschluessel.as_ref() else {
            return;
        };
        let im_zustand = self.kette.konto(&k.adresse()).nonce;
        let (hoehe, gehoert) = (self.kette.hoehe(), self.hoechste_gehoerte);
        if netz_luecke(&mut self.netz_stand, self.netz_nonce, im_zustand, hoehe, gehoert) {
            self.protokoll.schreibe(
                Eintrag::neu("netz_luecke").zahl("vergeben", self.netz_nonce as i64).zahl("im_zustand", im_zustand as i64),
            );
            self.netz_nonce = im_zustand;
        }
    }

    /// Verbreitet eine Transaktion; liefert, ob sie hinausging.
    ///
    /// 📌 **Weist der Mempool des Erzeugers sie ab** (Nummer verbraucht,
    /// Mempool voll, Deckel je Absender), geht sie nicht hinaus, und der
    /// Aufrufer erfährt es (2026-10-28). Vorher meldete die Relaisleitung
    /// einer Wallet „verbreitet“, auch wenn die Transaktion nie in einen
    /// Block kommen konnte; im Nachtlauf hielt das jede spätere Nummer eines
    /// Absenders auf.
    /// Wie [`Self::tx_verbreiten`], für eine geprüfte Transaktion: Der
    /// Erzeuger merkt sich ihre Unterschrift als geprüft.
    fn tx_verbreiten_geprueft(&mut self, g: myl_consensus::block::GepruefteTransaktion) -> bool {
        let Ok(daten) = borsh::to_vec(g.inhalt()) else {
            return false;
        };
        if self.konfig.erzeugt_bloecke && !self.kette.aufnehmen_geprueft(g) {
            return false;
        }
        let _ = self.kommandos.send(NodeCommand::Publish { topic: GossipTopic::Transactions, data: daten, result: None });
        true
    }

    fn tx_verbreiten(&mut self, tx: myl_consensus::block::Transaktion) -> bool {
        let Ok(daten) = borsh::to_vec(&tx) else {
            return false;
        };
        // ⚑ Der Erzeuger hört sein eigenes Gossip nicht; er nimmt die
        //   Transaktion selbst auf.
        if self.konfig.erzeugt_bloecke && !self.kette.aufnehmen(tx) {
            return false;
        }
        let _ = self.kommandos.send(NodeCommand::Publish { topic: GossipTopic::Transactions, data: daten, result: None });
        true
    }

    /// ⚑ **Der Archivbeleg der Epoche** (Stufe A5): Wer Kopie ist und die
    /// Geschichte hat, belegt selbst, einmal je Epoche, mit einem neuen
    /// Versuch nach acht Höhen, solange keiner im Zustand steht.
    fn archiv_belegen(&mut self, ich: &myl_types::ids::MinerId) {
        let a = &self.kette.zustand().archiv;
        if !a.ist_kopie(ich) || a.belegt.contains(ich) || self.schon_gesendet("archivbeleg") {
            return;
        }
        let Some(anweisung) = self.kette.archivbeleg(ich) else {
            return;
        };
        let stichproben = match &anweisung {
            myl_consensus::block::Anweisung::ArchivBelegen { bloecke, .. } => bloecke.len(),
            _ => 0,
        };
        if let Some(tx) = self.netz_tx(anweisung) {
            self.protokoll.schreibe(
                Eintrag::neu("archiv_beleg")
                    .zahl("epoche", self.kette.zustand().epoch.0 as i64)
                    .zahl("stichproben", stichproben as i64),
            );
            self.tx_verbreiten(tx);
        }
    }

    /// ⚑ **Die Teilwurzel nennen** (Stufe A7): einmal je Abschnitt, als
    /// Kopie mit Geschichte.
    fn teilwurzel_nennen(&mut self, ich: &myl_types::ids::MinerId) {
        let a = &self.kette.zustand().archiv;
        if !a.ist_kopie(ich) || a.teilwurzel.is_some() || a.teilstimmen.contains_key(ich) {
            return;
        }
        if self.schon_gesendet("teilwurzel") {
            return;
        }
        let Some(anweisung) = self.kette.teilwurzel_anweisung(ich) else {
            return;
        };
        if let Some(tx) = self.netz_tx(anweisung) {
            self.protokoll.schreibe(Eintrag::neu("archiv_teilwurzel").zahl("grenze", self.kette.zustand().archiv.grenze as i64));
            self.tx_verbreiten(tx);
        }
    }

    /// ⚑ **Den eigenen Teil der gezogenen Gruppe belegen** (Stufe A7),
    /// einmal je Epoche, sobald er auf der Platte liegt.
    fn teil_belegen(&mut self, ich: &myl_types::ids::MinerId) {
        let a = &self.kette.zustand().archiv;
        let (Some(index), Some(_)) = (a.teilindex(ich), a.teilwurzel) else {
            return;
        };
        if a.teil_belegt.contains(ich) {
            return;
        }
        let z = self.kette.zustand();
        let Some(stelle) = myl_ledger::archiv::teilprobe(&z.archiv, &z.epochensaat, z.epoch.0, ich) else {
            return;
        };
        let grenze = z.archiv.grenze;
        let Some((stueck, beweis)) = self.kette.teilablage().and_then(|t| t.lesen(grenze, stelle)) else {
            return;
        };
        if beweis.teil != index as u64 || self.schon_gesendet("teilbeleg") {
            return;
        }
        if let Some(tx) = self.netz_tx(myl_consensus::block::Anweisung::ArchivTeilBelegen { stueck, beweis }) {
            self.protokoll.schreibe(Eintrag::neu("archiv_teilbeleg").zahl("stelle", stelle as i64));
            self.tx_verbreiten(tx);
        }
    }

    /// ⚑ **Die eigenen Teile holen** (Stufe A7): Wer Splitter ist und die
    /// Teilwurzel gilt, holt die Teile, die noch nicht auf der Platte liegen,
    /// bei Kopien und Haltern zuerst.
    fn teile_holen(&mut self, ich: &myl_types::ids::MinerId) {
        let a = &self.kette.zustand().archiv;
        let (Some(index), Some(_)) = (a.teilindex(ich), a.teilwurzel) else {
            self.teilabruf = None;
            return;
        };
        let grenze = a.grenze;
        let anzahl = a.teil_bis - a.teil_ab;
        if self.teilabruf.as_ref().is_none_or(|t| t.grenze != grenze) {
            let Some(ablage) = self.kette.teilablage() else {
                return;
            };
            let fehlend: Vec<u64> = (0..anzahl).filter(|s| !ablage.hat(grenze, *s)).collect();
            if fehlend.is_empty() {
                return;
            }
            let vorgaenger = self
                .kette
                .archivkopien()
                .iter()
                .filter_map(|adresse| myl_net::peer_id_aus_bytes(adresse).ok())
                .collect();
            self.protokoll.schreibe(
                Eintrag::neu("archiv_teile_holen").zahl("grenze", grenze as i64).zahl("fehlend", fehlend.len() as i64),
            );
            self.teilabruf = Some(Teilabruf {
                grenze,
                teil: index as u64,
                fehlend,
                laufend: Vec::new(),
                gesperrt: Default::default(),
                reihum: 0,
                gemeldet: None,
                vorgaenger,
            });
        }
        if let Some(t) = self.teilabruf.as_mut() {
            t.laufend.retain(|(_, _, seit)| seit.elapsed() <= ABZUG_FRIST);
        }
        self.teilabruf_anfragen();
    }

    fn teilabruf_bei(&self, von: &libp2p::PeerId) -> bool {
        self.teilabruf.as_ref().is_some_and(|t| t.laufend.iter().any(|(p, _, _)| p == von))
    }

    fn teilabruf_anfragen(&mut self) {
        let mut versuche = 0usize;
        while self.teilabruf.as_ref().is_some_and(|t| t.laufend.len() < HINTERGRUND_ZUGLEICH)
            && versuche <= self.gegenstellen.len()
        {
            versuche += 1;
            let frei: Vec<libp2p::PeerId> = self
                .gegenstellen
                .iter()
                .filter(|p| {
                    !self.nachforderungen.iter().any(|n| n.an == **p)
                        && !self.hintergrund_bei(p)
                        && !self.abzug_bei(p)
                        && !self.abruf_bei(p)
                        && !self.teilabruf_bei(p)
                })
                .copied()
                .collect();
            let Some(t) = self.teilabruf.as_mut() else {
                return;
            };
            let Some(&stelle) = t.fehlend.iter().find(|s| !t.laufend.iter().any(|(_, x, _)| x == *s)) else {
                return;
            };
            let vorgaenger = t.vorgaenger.iter().find(|p| frei.contains(p) && !t.gesperrt.contains(p)).copied();
            let Some(peer) =
                vorgaenger.or_else(|| naechste_gegenstelle(&frei, &mut t.gesperrt, &mut t.reihum, &mut t.gemeldet))
            else {
                return;
            };
            let (grenze, teil) = (t.grenze, t.teil);
            if !self.anfrage_senden(peer, &Nachforderung::Teil { grenze, stelle, teil }) {
                return;
            }
            if let Some(t) = self.teilabruf.as_mut() {
                t.laufend.push((peer, stelle, std::time::Instant::now()));
            }
        }
    }

    fn teilabruf_nichts(&mut self, von: libp2p::PeerId) {
        if let Some(t) = self.teilabruf.as_mut() {
            t.laufend.retain(|(p, _, _)| *p != von);
            t.gesperrt.insert(von);
        }
        self.teilabruf_anfragen();
    }

    /// **Ein Teil kommt an**: nur von einer gefragten Gegenstelle, für den
    /// erfragten Teil, geprüft gegen die Teilwurzel im Zustand.
    fn teil_erhalten(
        &mut self,
        von: libp2p::PeerId,
        grenze: u64,
        stueck: Vec<u8>,
        beweis: myl_types::archivteile::Teilbeweis,
    ) {
        let geltend = self.kette.zustand().archiv.teilwurzel;
        let ablage = self.kette.teilablage();
        let Some(t) = self.teilabruf.as_mut() else {
            return;
        };
        let Some(i) = t.laufend.iter().position(|(p, s, _)| *p == von && *s == beweis.stelle) else {
            return;
        };
        t.laufend.remove(i);
        let gut = grenze == t.grenze
            && beweis.teil == t.teil
            && geltend.is_some_and(|w| beweis.pruefen(&w, &stueck))
            && ablage.as_ref().is_some_and(|a| a.ablegen(grenze, beweis.stelle, &stueck, &beweis).is_ok());
        if !gut {
            t.gesperrt.insert(von);
            self.protokoll.schreibe(Eintrag::neu("archiv_teil_abgelehnt").text("von", von.to_string()));
            return self.teilabruf_anfragen();
        }
        t.fehlend.retain(|s| *s != beweis.stelle);
        if t.fehlend.is_empty() {
            let grenze = t.grenze;
            self.teilabruf = None;
            self.protokoll.schreibe(Eintrag::neu("archiv_teile_vollstaendig").zahl("grenze", grenze as i64));
            return;
        }
        self.teilabruf_anfragen();
    }

    /// Ob für `was` bei der laufenden Höhe schon etwas unterwegs ist; sonst
    /// vermerken. Ein Block mit der Transaktion kommt in wenigen Höhen.
    fn schon_gesendet(&mut self, was: &'static str) -> bool {
        let hoehe = self.kette.hoehe();
        // ⚑ **Solange eigene Transaktionen ausstehen, wartet ein neuer
        //   Versuch**, bis zur Lückenfrist (Fund 627). Mit acht Höhen allein
        //   ging im Archivlauf derselbe Beleg in jeder Aufnahme neu hinaus,
        //   alle zwei Sekunden, je bis zu 27 KB.
        let frist = if self.netz_offen() { NETZ_LUECKE_HOEHEN } else { 8 };
        if self.netz_gesendet.get(was).is_some_and(|b| hoehe < b + frist) {
            return true;
        }
        // ⚑ Die Frist zählt ab der gehörten Spitze (Fund 628).
        self.netz_gesendet.insert(was, frist_bezug(hoehe, self.hoechste_gehoerte));
        false
    }

    /// Meldet diesen Knoten als Miner an, sobald sein Konto Hinterlegung und
    /// Gebühr deckt (G1); bis dahin dreht er den Probehahn auf, wo es einen
    /// gibt.
    fn anmeldung_senden(&mut self, ich: &myl_types::ids::MinerId) {
        if !self.probehahn_pflegen(ich) || self.schon_gesendet("anmeldung") {
            return;
        }
        let anweisung = myl_consensus::block::Anweisung::MinerAnmelden {
            hardware: myl_types::miner::HardwareClass::MediumGpu,
            zone: self.konfig.zone,
            netzadresse: self.netzadresse,
        };
        if let Some(tx) = self.netz_tx(anweisung) {
            self.tx_verbreiten(tx);
            self.protokoll.schreibe(Eintrag::neu("miner_anmeldung").text("zone", self.konfig.zone.name()));
        }
    }

    /// ⚑ **Der Probehahn** (2026-10-27, Festlegung des Projektinhabers, G1):
    /// Liefert, ob das Konto des Miners deckt, was er gerade braucht: vor
    /// der Anmeldung die Hinterlegung und ein Polster für Gebühren, danach
    /// das halbe Polster. Deckt es nicht und hat das Netz einen Probehahn,
    /// überweist der Knoten aus einem Probekonto so viel, dass es wieder
    /// reicht, mit dessen öffentlich ableitbarem Schlüssel. Welches Konto,
    /// folgt aus der eigenen Kennung, damit nicht alle Knoten zugleich
    /// dieselbe Nummer desselben Kontos verbrauchen; ist es zu leer, das
    /// nächste.
    fn probehahn_pflegen(&mut self, ich: &myl_types::ids::MinerId) -> bool {
        let untergrenze = if self.kette.zustand().miner.contains_key(ich) {
            // Dazu, was am Einsatz noch fehlt ([`Self::einsatz_pflegen`]).
            PROBEHAHN_POLSTER / 2 + self.einsatz_fehlt(ich)
        } else {
            self.konfig.netz.gebuehren.hinterlegung_anmeldung + PROBEHAHN_POLSTER / 2
        };
        self.probehahn(&myl_types::ids::Address::new(*ich.as_bytes()), untergrenze, "probehahn")
    }

    /// Ob `adresse` mindestens `untergrenze` hat; sonst aus dem Probehahn
    /// auffüllen, wo es einen gibt (siehe [`Self::probehahn_pflegen`]).
    /// `was` trennt die Wiederholsperre je Zweck.
    fn probehahn(&mut self, adresse: &myl_types::ids::Address, untergrenze: u64, was: &'static str) -> bool {
        let adresse = *adresse;
        let stand = self.kette.konto(&adresse).balance;
        if stand >= untergrenze {
            return true;
        }
        // 📌 Fund 665: Nicht, solange der Knoten aufholt. Sein Zustand ist dann
        //    alt, er sieht kein Guthaben und alte Nummern der Probekonten; ein
        //    Einsteiger zapfte so zwölfmal mit veralteten Nummern, bevor er
        //    an der Spitze war.
        if self.kette.hoehe().saturating_add(NETZ_LUECKE_HOEHEN) < self.hoechste_gehoerte {
            return false;
        }
        if !self.konfig.netz.probehahn || self.schon_gesendet(was) {
            return false;
        }
        let betrag = untergrenze + PROBEHAHN_POLSTER / 2 - stand;
        let n = crate::kette::PROBEKONTEN;
        // 📌 Ein Wiederholer beginnt beim nächsten Konto: Trafen zwei Knoten
        //    dieselbe Nummer desselben Kontos, kam nur einer durch, und der
        //    andere traf beim nächsten Anlauf wieder dasselbe Konto (im
        //    kurzen Podlauf bis zu sechs Anläufe).
        let versuch = self.probehahn_versuche % n;
        self.probehahn_versuche = self.probehahn_versuche.wrapping_add(1);
        let Some(quelle) = (0..n)
            .map(|i| (adresse.as_bytes()[0] % n + versuch + i) % n)
            .find(|q| {
                let k = crate::kette::probekonto(*q);
                self.kette.konto_bekannt(&k) && self.kette.konto(&k).balance >= betrag + PROBEHAHN_POLSTER
            })
        else {
            self.protokoll.schreibe(Eintrag::neu("probehahn_leer").zahl("betrag", betrag as i64));
            return false;
        };
        let nonce = self.kette.konto(&crate::kette::probekonto(quelle)).nonce;
        let anweisung = myl_consensus::block::Anweisung::Ueberweisung { nach: adresse, betrag };
        let schluessel = crate::kette::probeschluessel(quelle);
        if let Ok(tx) = myl_consensus::block::Transaktion::signiere(&self.kette.netzwert(), &schluessel, nonce, anweisung) {
            self.tx_verbreiten(tx);
            self.protokoll.schreibe(
                Eintrag::neu("probehahn").zahl("probekonto", quelle as i64).zahl("betrag", betrag as i64),
            );
        }
        false
    }

    /// Was am konfigurierten Einsatz noch fehlt.
    fn einsatz_fehlt(&self, ich: &myl_types::ids::MinerId) -> u64 {
        let staked = self.kette.konto(&myl_types::ids::Address::new(*ich.as_bytes())).staked;
        self.konfig.einsatz.unwrap_or(0).saturating_sub(staked)
    }

    /// ⚑ **Den Einsatz hinterlegen** (2026-10-29, `--einsatz`): als
    /// angemeldeter Miner, sobald das Guthaben ihn und ein Polster für
    /// Gebühren trägt, und nur, was fehlt. Ohne Einsatz schlachtet ein
    /// Schiedsurteil nichts.
    fn einsatz_pflegen(&mut self, ich: &myl_types::ids::MinerId) {
        let fehlt = self.einsatz_fehlt(ich);
        let stand = self.kette.konto(&myl_types::ids::Address::new(*ich.as_bytes())).balance;
        if fehlt == 0 || !self.kette.zustand().miner.contains_key(ich) || stand < fehlt + PROBEHAHN_POLSTER / 4 {
            return;
        }
        if self.schon_gesendet("einsatz") {
            return;
        }
        if let Some(tx) = self.netz_tx(myl_consensus::block::Anweisung::EinsatzHinterlegen { betrag: fehlt }) {
            self.tx_verbreiten(tx);
            self.protokoll.schreibe(Eintrag::neu("einsatz_hinterlegt").zahl("betrag", fehlt.min(i64::MAX as u64) as i64));
        }
    }

    /// ⚑ **Anmeldung und Zusage** (Festlegung des Projektinhabers: kein Pod
    /// ohne Zusage, der Knoten reicht sie selbst ein). Angemeldet wird, wer
    /// es noch nicht ist; eingereicht, wenn die neueste Zusage im Zustand
    /// nicht die konfigurierte ist, für die nächste Epoche.
    fn zusage_pflegen(&mut self, ich: &myl_types::ids::MinerId) {
        let Some(soll) = self.konfig.modellzusage else {
            return;
        };
        let mut soll = myl_types::zusage::Modellzusage { abdruck: self.konfig.netz.podmodell.abdruck, ..soll };
        // ⚑ Die Selbstprobe darf die Zusage nur verlangsamen (Frage 6 vom
        //   2026-10-29, [`Self::selbstprobe_auswerten`]).
        soll.ebenenzeit_us = soll.ebenenzeit_us.max(self.ebenenzeit_angepasst);
        if !self.kette.zustand().miner.contains_key(ich) {
            self.anmeldung_senden(ich);
            return;
        }
        let neueste = self.kette.zustand().zusagen.get(ich).and_then(|l| l.last()).and_then(|z| z.modell(&soll.abdruck).copied());
        if neueste == Some(soll) || self.schon_gesendet("zusage") {
            return;
        }
        let ab = myl_types::ids::EpochId(self.kette.zustand().epoch.0 + 1);
        let mut zusage = myl_types::zusage::Kapazitaetszusage::neu(*ich, ab, self.zugesagt_bytes).mit_modell(soll);
        let Some(k) = self.netzschluessel.as_ref() else {
            return;
        };
        if k.zusage_unterschreiben(&mut zusage).is_err() {
            return;
        }
        if let Some(tx) = self.netz_tx(myl_consensus::block::Anweisung::ZusageEinreichen { zusage }) {
            self.tx_verbreiten(tx);
            self.protokoll.schreibe(
                Eintrag::neu("zusage_eingereicht")
                    .zahl("ab_epoche", ab.0 as i64)
                    .zahl("ebenen", soll.ebenen_hoechstens as i64)
                    .zahl("ebenenzeit_us", soll.ebenenzeit_us.min(i64::MAX as u64) as i64),
            );
        }
    }

    /// ⚑ **Die Grössen je Epoche ins Protokoll** (Fund 677): die der Kette
    /// ([`crate::kette::Kette::groessen`]) und die eigenen Sammlungen, die mit
    /// der Laufzeit wachsen könnten.
    fn groessen_protokollieren(&mut self) {
        let epoche = self.kette.zustand().epoch.0;
        if self.groessen_epoche == Some(epoche) {
            return;
        }
        self.groessen_epoche = Some(epoche);
        let mut eintrag = Eintrag::neu("groessen").zahl("epoche", epoche as i64);
        for (name, wert) in self.kette.groessen() {
            eintrag = eintrag.zahl(name, wert.min(i64::MAX as u64) as i64);
        }
        let eigene = [
            ("spurablagen", self.spurablagen.len()),
            ("spurablagen_eintraege", self.spurablagen.iter().map(|a| a.eintraege.len()).sum()),
            ("messungen", self.messungen.len()),
            ("schiedsbelege", self.schiedsbelege.len()),
            ("festgelegt", self.festgelegt.len()),
            ("shardrelais", self.shardrelais.len()),
            ("ausstehend", self.ausstehend.len()),
        ];
        for (name, wert) in eigene {
            eintrag = eintrag.zahl(name, wert as i64);
        }
        self.protokoll.schreibe(eintrag);
    }

    /// ⚑ **Die Selbstprobe** (Fund 677, Frage 6 vom 2026-10-29): Einmal je
    /// Epoche, in ihrer zweiten Hälfte, misst der Knoten seinen eigenen Shard
    /// mit einer Zeitprobe derselben Grösse, wie sie die Mitglieder stellen.
    /// In der zweiten Hälfte, weil die Mitglieder am Anfang messen und der
    /// Shard nur einen Platz für Zeitproben hat (Fund 609).
    fn selbstprobe_pflegen(&mut self, ich: &myl_types::ids::MinerId) {
        while let Ok((epoche, je_token_us, frist_us)) = self.selbstproben.try_recv() {
            self.selbstprobe_auswerten(epoche, je_token_us, frist_us);
        }
        let z = self.kette.zustand();
        let epoche = z.epoch.0;
        let in_epoche = self.kette.hoehe() % myl_consensus::block::BLOECKE_JE_EPOCHE;
        if self.selbstprobe_epoche == Some(epoche) || in_epoche < myl_consensus::block::BLOECKE_JE_EPOCHE / 2 {
            return;
        }
        let Some(auftrag) = crate::shardfuehrung::auftrag_der_epoche(z, ich) else {
            return;
        };
        let Some(frist) = crate::frist::frist_des_mitglieds(z, auftrag.pod, ich) else {
            return;
        };
        let Some(((adresse, ausweis), platz)) =
            self.shardfuehrung.as_mut().and_then(|f| f.tuer()).zip(self.zeitprobe_platz.nehmen())
        else {
            return;
        };
        self.selbstprobe_epoche = Some(epoche);
        let positionen = crate::frist::probe_positionen(frist);
        let kanal = self.selbstprobenkanal.clone();
        tokio::task::spawn_blocking(move || {
            let beginn = std::time::Instant::now();
            let ok = myl_types::zeitprobe::fragen(adresse, &ausweis, positionen, Duration::from_secs(120)).is_ok();
            let dauer = beginn.elapsed().as_micros() as u64;
            drop(platz);
            if ok {
                let _ = kanal.send((epoche, dauer / u64::from(positionen.max(1)), frist));
            }
        });
    }

    /// **Die Selbstprobe auswerten**: Braucht der eigene Shard je Token mehr
    /// als das Anderthalbfache seiner Zusage (drei Viertel der Frist, die das
    /// Doppelte erlaubt), wird die Zusage für die nächste Epoche langsamer:
    /// die gemessene Zeit mit einem Viertel Luft. Schneller wird sie nie von
    /// selbst; das bleibt der Konfiguration. Whitepaper: Zusage und Frist
    /// stehen dort nicht (Festlegung vom 2026-10-22); die Reserve (6.8) ist
    /// unberührt.
    fn selbstprobe_auswerten(&mut self, epoche: u64, je_token_us: u64, frist_us: u64) {
        let mut eintrag = Eintrag::neu("selbstprobe")
            .zahl("epoche", epoche as i64)
            .zahl("je_token_us", je_token_us.min(i64::MAX as u64) as i64)
            .zahl("frist_us", frist_us.min(i64::MAX as u64) as i64);
        if crate::frist::selbstprobe_zu_langsam(je_token_us, frist_us) {
            if let Some(soll) = self.konfig.modellzusage {
                let jetzt = soll.ebenenzeit_us.max(self.ebenenzeit_angepasst);
                if let Some(neu) = crate::frist::angepasste_ebenenzeit(jetzt, je_token_us, frist_us) {
                    self.ebenenzeit_angepasst = neu;
                    eintrag = eintrag.zahl("ebenenzeit_us_neu", neu.min(i64::MAX as u64) as i64);
                }
            }
            eintrag = eintrag.text("warnung", "langsamer als das Anderthalbfache der Zusage");
        }
        self.protokoll.schreibe(eintrag);
    }

    /// **Die Pods der laufenden Epoche** für einen Koordinator, je Mitglied
    /// Netzadresse und Ebenenbereich in Shardreihenfolge.
    pub fn podbild(&self) -> myl_types::relais::Podbild {
        let z = self.kette.zustand();
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(z);
        // ⚑ Dieselben Gruppen, nach denen die Kette vergütet (Stufe 1).
        let gruppen = crate::kette::Kette::zwillinge_der_laufenden_epoche(z);
        let zwilling = |i: u32| gruppen.iter().find(|g| g.contains(&i)).and_then(|g| g.iter().copied().find(|&j| j != i));
        let pods = zuteilung
            .pods
            .iter()
            .filter_map(|p| {
                let grenzen = crate::kette::Kette::grenzen_des_pods(z, p)?;
                Some(myl_types::relais::Podeintrag {
                    pod_index: p.pod_index,
                    kennung: myl_types::pod_kennung(z.epoch.0, p.pod_index),
                    mitglieder: p
                        .shards
                        .iter()
                        .zip(grenzen)
                        .map(|(s, (von, bis))| myl_types::relais::Podmitglied {
                            miner: s.miner.miner_id,
                            netzadresse: s.miner.netzadresse,
                            von,
                            bis,
                        })
                        .collect(),
                    zwilling: zwilling(p.pod_index),
                    // Die Reserve, die eine Position übernehmen darf (6.8).
                    reserve: p
                        .reserve
                        .iter()
                        .map(|r| myl_types::relais::Podmitglied { miner: r.miner_id, netzadresse: r.netzadresse, von: 0, bis: 0 })
                        .collect(),
                })
            })
            .collect();
        myl_types::relais::Podbild {
            epoche: z.epoch.0,
            hoehe: self.kette.hoehe(),
            bloecke_je_epoche: myl_consensus::block::BLOECKE_JE_EPOCHE,
            pods,
        }
    }

    /// **Eine Frage von der Relaisleitung**.
    fn relais_bedienen(
        &mut self,
        frage: myl_types::relais::Relaisfrage,
        rueck: std::sync::mpsc::Sender<myl_types::relais::Relaisantwort>,
    ) {
        use myl_types::relais::{Relaisantwort, Relaisfrage};
        match frage {
            Relaisfrage::Podbild => {
                let _ = rueck.send(Relaisantwort::Podbild(self.podbild()));
            }
            Relaisfrage::Shard { an, anfrage } => {
                let Ok(peer) = myl_net::peer_id_aus_bytes(&an) else {
                    let _ = rueck.send(Relaisantwort::Fehler("keine gültige Netzadresse".into()));
                    return;
                };
                // ⚑ Ist das Mitglied dieser Knoten selbst, geht die Anfrage
                //   direkt an den eigenen Shard; über das Netz an sich selbst
                //   kann ein Knoten nicht fragen.
                if peer == self.peer_id {
                    match self.shardfuehrung.as_mut().and_then(|f| f.tuer()) {
                        None => {
                            let _ = rueck.send(Relaisantwort::Fehler("kein eigener Shard".into()));
                        }
                        Some((adresse, ausweis)) => {
                            tokio::task::spawn_blocking(move || {
                                let a = match crate::relais::an_den_eigenen_shard(adresse, &ausweis, &anfrage) {
                                    Ok(bytes) => Relaisantwort::Shard(bytes),
                                    Err(f) => Relaisantwort::Fehler(f),
                                };
                                let _ = rueck.send(a);
                            });
                        }
                    }
                    return;
                }
                let kennung = self.relais_kennung;
                self.relais_kennung = self.relais_kennung.wrapping_add(1);
                let Some(daten) = (Nachforderung::Shard { kennung, anfrage }).als_bytes() else {
                    let _ = rueck.send(Relaisantwort::Fehler("Anfrage nicht serialisierbar".into()));
                    return;
                };
                self.shardrelais.insert(kennung, (peer, rueck, std::time::Instant::now()));
                let _ = self.kommandos.send(NodeCommand::Anfrage { an: peer, daten });
            }
            // ⚑ **Das Bündel geht zum Koordinator des Pods** (erste
            //   Position laut Kette), nach einer eigenen Prüfung.
            Relaisfrage::BuendelEinreichen { buendel } => {
                let Ok(b) = borsh::from_slice::<myl_types::PoIBundle>(&buendel) else {
                    let _ = rueck.send(Relaisantwort::Fehler("kein PoI-Bündel".into()));
                    return;
                };
                let (koordinator, netzadresse) = match crate::kette::Kette::buendel_pruefen(self.kette.zustand(), &b) {
                    Ok(k) => k,
                    Err(f) => {
                        let _ = rueck.send(Relaisantwort::Fehler(f));
                        return;
                    }
                };
                if self.miner_ich() == Some(koordinator) {
                    let _ = rueck.send(match self.buendel_selbst_einreichen(b) {
                        Ok(z) => Relaisantwort::Erledigt(z),
                        Err(f) => Relaisantwort::Fehler(f),
                    });
                    return;
                }
                let Ok(peer) = myl_net::peer_id_aus_bytes(&netzadresse) else {
                    let _ = rueck.send(Relaisantwort::Fehler("Koordinator ohne gültige Netzadresse".into()));
                    return;
                };
                let kennung = self.relais_kennung;
                self.relais_kennung = self.relais_kennung.wrapping_add(1);
                let Some(daten) = (Nachforderung::Buendel { kennung, buendel }).als_bytes() else {
                    let _ = rueck.send(Relaisantwort::Fehler("Bündel nicht serialisierbar".into()));
                    return;
                };
                self.shardrelais.insert(kennung, (peer, rueck, std::time::Instant::now()));
                let _ = self.kommandos.send(NodeCommand::Anfrage { an: peer, daten });
            }
            // ⚑ **Spuren eines Bündels ablegen** (Stufe 2): beim Mitglied.
            Relaisfrage::SpurenAblegen { an, ablage } => {
                let Ok(peer) = myl_net::peer_id_aus_bytes(&an) else {
                    let _ = rueck.send(Relaisantwort::Fehler("keine gültige Netzadresse".into()));
                    return;
                };
                if peer == self.peer_id {
                    let _ = rueck.send(match self.spur_ablegen(ablage) {
                        Ok(()) => Relaisantwort::Erledigt("abgelegt".into()),
                        Err(f) => Relaisantwort::Fehler(f),
                    });
                    return;
                }
                let kennung = self.relais_kennung;
                self.relais_kennung = self.relais_kennung.wrapping_add(1);
                let Some(daten) = (Nachforderung::SpurenAblegen { kennung, ablage }).als_bytes() else {
                    let _ = rueck.send(Relaisantwort::Fehler("Ablage nicht serialisierbar".into()));
                    return;
                };
                self.shardrelais.insert(kennung, (peer, rueck, std::time::Instant::now()));
                let _ = self.kommandos.send(NodeCommand::Anfrage { an: peer, daten });
            }
            // ⚑ **Eine Übernahme durch die Reserve** (6.8): an deren Knoten.
            // ⚑ **Ein Differenzbeweis vom Ortsdienst** (2026-10-29): an die
            //   Koordinatoren beider Pods, wie die Kette sie aus der
            //   Uneinigkeit kennt (erste Position); jeder meldet ihn mit
            //   seinem Schlüssel, die Kette nimmt den ersten. Schweigt der
            //   eine, meldet der andere.
            Relaisfrage::DifferenzMelden { epoche, pod_a, pod_b, beweis } => {
                let z = self.kette.zustand();
                let koordinatoren: Vec<myl_types::ids::MinerId> = z
                    .uneinigkeiten
                    .iter()
                    .find(|u| u.stichprobe.epoche == epoche)
                    .map(|u| {
                        u.stichprobe
                            .besetzung
                            .iter()
                            .filter(|(p, _)| *p == pod_a || *p == pod_b)
                            .filter_map(|(_, b)| b.first().map(|(m, _, _)| *m))
                            .collect()
                    })
                    .unwrap_or_default();
                if koordinatoren.is_empty() {
                    let _ = rueck.send(Relaisantwort::Fehler("keine uneinige Gruppe zu diesen Pods".into()));
                    return;
                }
                let mut gesendet = 0;
                for k in koordinatoren {
                    if self.miner_ich() == Some(k) {
                        self.differenz_selbst_melden(epoche, pod_a, pod_b, (*beweis).clone());
                        gesendet += 1;
                        continue;
                    }
                    let an = self.kette.zustand().miner.get(&k).and_then(|r| myl_net::peer_id_aus_bytes(&r.netzadresse).ok());
                    let daten = Nachforderung::Differenz { epoche, pod_a, pod_b, beweis: beweis.clone() }.als_bytes();
                    if let (Some(an), Some(daten)) = (an, daten) {
                        let _ = self.kommandos.send(NodeCommand::Anfrage { an, daten });
                        gesendet += 1;
                    }
                }
                let _ = rueck.send(Relaisantwort::Erledigt(format!("an {gesendet} Koordinatoren")));
            }
            Relaisfrage::Uebernahme { an, epoche, pod, position } => {
                let Ok(peer) = myl_net::peer_id_aus_bytes(&an) else {
                    let _ = rueck.send(Relaisantwort::Fehler("keine gültige Netzadresse".into()));
                    return;
                };
                let kennung = self.relais_kennung;
                self.relais_kennung = self.relais_kennung.wrapping_add(1);
                let Some(daten) = (Nachforderung::PositionUebernehmen { kennung, epoche, pod, position }).als_bytes() else {
                    let _ = rueck.send(Relaisantwort::Fehler("Anfrage nicht serialisierbar".into()));
                    return;
                };
                self.shardrelais.insert(kennung, (peer, rueck, std::time::Instant::now()));
                let _ = self.kommandos.send(NodeCommand::Anfrage { an: peer, daten });
            }
            Relaisfrage::Konto { adresse } => {
                let z = self.kette.zustand();
                let konto = z.account(&adresse);
                let _ = rueck.send(Relaisantwort::Konto(myl_types::relais::Kontostand {
                    hoehe: self.kette.hoehe(),
                    epoche: z.epoch.0,
                    guthaben: konto.balance,
                    nonce: konto.nonce,
                    credits: konto.credits.iter().filter(|c| c.expiry.0 >= z.epoch.0).map(|c| c.vtfe).sum(),
                    netzwert: self.kette.netzwert(),
                    betreiber: self.eigenes_konto(),
                    // Nie unter dem Boden, wie beim Anwenden.
                    grundpreis_je_byte: z.grundpreis_je_byte.max(self.konfig.netz.gebuehren.boden_je_byte),
                    bekannt: z.accounts.contains_key(&adresse),
                    aufschlag_konto: self.konfig.netz.gebuehren.aufschlag_konto,
                }));
            }
            // ⚑ **Eine Transaktion aus einem Wallet** (2026-10-26): erst
            //   prüfen (Unterschrift, Netz), dann verbreiten. Eine
            //   falsche kommt nicht ins Netz.
            Relaisfrage::Transaktion { tx } => {
                let Ok(t) = borsh::from_slice::<myl_consensus::block::Transaktion>(&tx) else {
                    let _ = rueck.send(Relaisantwort::Fehler("keine Transaktion".into()));
                    return;
                };
                let nonce = t.nonce;
                let g = match t.pruefe(&self.kette.netzwert()) {
                    Ok(g) => g,
                    Err(e) => {
                        let _ = rueck.send(Relaisantwort::Fehler(format!("Transaktion ungültig: {e:?}")));
                        return;
                    }
                };
                if !self.tx_verbreiten_geprueft(g) {
                    let _ = rueck.send(Relaisantwort::Fehler(format!(
                        "vom Mempool abgewiesen (Nonce {nonce}: verbraucht, Mempool voll oder zu viele vom selben Absender)"
                    )));
                    return;
                }
                self.protokoll.schreibe(Eintrag::neu("wallet_tx").zahl("nonce", nonce.min(i64::MAX as u64) as i64));
                let _ = rueck.send(Relaisantwort::Erledigt(format!("verbreitet, Nonce {nonce}")));
            }
            // ⚑ **Gebündelt** (2026-10-29): als Stapel geprüft, dann jede
            //   gültige verbreitet; gezählt, wie viele der Mempool nahm.
            Relaisfrage::Transaktionen { txs } => {
                if txs.len() > myl_types::relais::TRANSAKTIONEN_JE_FRAGE {
                    let _ = rueck.send(Relaisantwort::Fehler(format!("mehr als {} Transaktionen", myl_types::relais::TRANSAKTIONEN_JE_FRAGE)));
                    return;
                }
                // ⚑ Geprüft in einer Rechenaufgabe (Skalierung, 2026-10-29):
                //   Im Hauptfaden hielt der Stapel bei 3000 je Sekunde
                //   Blockbau und Gossip auf; aufgenommen und beantwortet wird
                //   danach in der Schleife (`Weckruf::RelaisGeprueft`).
                let n = txs.len();
                let netz = self.kette.netzwert();
                let kanal = self.relais_geprueft_kanal.clone();
                tokio::task::spawn_blocking(move || {
                    let gelesen: Vec<myl_consensus::block::Transaktion> =
                        txs.iter().filter_map(|b| borsh::from_slice::<myl_consensus::block::Transaktion>(b).ok()).collect();
                    let gut: Vec<myl_consensus::block::GepruefteTransaktion> =
                        myl_consensus::block::pruefe_alle(&gelesen, &netz).into_iter().flatten().collect();
                    let _ = kanal.send((gut, n, rueck));
                });
            }
        }
    }

    /// **Reicht ein geprüftes Bündel ein**, als Koordinator des Pods.
    fn buendel_selbst_einreichen(&mut self, b: myl_types::PoIBundle) -> Result<String, String> {
        crate::kette::Kette::buendel_pruefen(self.kette.zustand(), &b)?;
        let segmente = b.segmente;
        let vtfe = b.vtfe_claimed;
        let tx = self
            .netz_tx(myl_consensus::block::Anweisung::BuendelEinreichen { buendel: b })
            .ok_or("ohne Konsensschlüssel kein Einreichen")?;
        self.tx_verbreiten(tx);
        self.protokoll.schreibe(
            Eintrag::neu("buendel_eingereicht").zahl("segmente", i64::from(segmente)).zahl("vtfe", vtfe.min(i64::MAX as u64) as i64),
        );
        Ok(format!("eingereicht: {segmente} Segmente, {vtfe} vTFE"))
    }

    /// Die Antwort des Koordinators auf ein weitergegebenes Bündel.
    fn buendel_antwort(&mut self, von: libp2p::PeerId, kennung: u64, fehler: Option<String>) {
        use myl_types::relais::Relaisantwort;
        if !self.shardrelais.get(&kennung).is_some_and(|(p, _, _)| *p == von) {
            return;
        }
        if let Some((_, rueck, _)) = self.shardrelais.remove(&kennung) {
            let _ = rueck.send(match fehler {
                Some(f) => Relaisantwort::Fehler(format!("Koordinator {von}: {f}")),
                None => Relaisantwort::Erledigt(format!("eingereicht vom Koordinator {von}")),
            });
        }
    }

    /// Hebt eine Spurablage auf; eine für dasselbe Bündel ersetzt die alte.
    /// ⚑ Nur eine, die zu diesem Knoten gehört
    /// ([`crate::shardfuehrung::ablage_pruefen`]): Sonst könnte jeder im
    /// Netz die echten Ablagen mit erfundenen verdrängen.
    fn spur_ablegen(&mut self, ablage: myl_types::Spurablage) -> Result<(), String> {
        let ich = self.miner_ich().ok_or("kein Miner")?;
        let pod = self.kette.pod_der_kennung(ablage.epoche.0, &ablage.pod).ok_or("diesen Pod kennt die Zuteilung nicht")?;
        crate::shardfuehrung::ablage_pruefen(&pod, &ich, &ablage)?;
        self.spurablagen.retain(|a| !(a.epoche == ablage.epoche && a.pod == ablage.pod && a.segments_root == ablage.segments_root));
        self.spurablagen.push(ablage);
        while self.spurablagen.len() > SPURABLAGEN_HOECHSTENS {
            self.spurablagen.remove(0);
        }
        Ok(())
    }

    /// ⚑ **Eine gelieferte Spur** (Stufe 2): gegen die Wurzel des Bündels aus
    /// der Kette geprüft (`myl_types::lieferung_pruefen`); das Ergebnis steht
    /// im Protokoll. Nachgerechnet wird noch nicht (die Eingabe fehlt, E10).
    fn spur_erhalten(&mut self, von: libp2p::PeerId, antwort: myl_types::Spurantwort) {
        let bekannt = self
            .kette
            .zustand()
            .stichprobe_wurzeln
            .iter()
            .find(|(p, _, _)| *p == antwort.anfrage.pod)
            .map(|(_, w, n)| (*w, *n));
        let ergebnis = match bekannt {
            None => "ohne Wurzel".to_string(),
            Some((w, n)) => match myl_types::lieferung_pruefen(&antwort, &w, n) {
                Ok(()) => "traegt".to_string(),
                Err(f) => format!("{f:?}"),
            },
        };
        self.protokoll.schreibe(
            Eintrag::neu("spur_geprueft")
                .text("von", von.to_string())
                .zahl("epoche", antwort.anfrage.epoche.0 as i64)
                .text("pod", antwort.anfrage.pod.to_hex())
                .zahl("segment", i64::from(antwort.anfrage.segment))
                .text("ergebnis", ergebnis),
        );
    }

    /// ⚑ **Der Checker fragt nach dem Verlauf** (Stufe 2, T6c und W6b,
    /// 2026-10-29): Für jede Ziehung der letzten Stichprobe, die die Kette
    /// diesem Miner zuteilt ([`crate::kette::pruefer_waehlen`]), geht eine
    /// `Nachforderung::Verlauf` an jedes Mitglied des Pods, einmal je
    /// Ziehung; die erste Antwort, die trägt, wird nachgerechnet. Dazu holt
    /// die Schleife ab, was nachgerechnet ist.
    fn nachrechnung_pflegen(&mut self) {
        while let Ok(n) = self.nachrechnungen.try_recv() {
            self.nachrechnung_vermerken(n);
        }
        self.aufdecken_wenn_erlaubt();
        self.schied_pflegen();
        erzeugerliste_aus(&self.kette, &self.erzeugerliste);
        self.zeugen_pflegen();
        let Some(ich) = self.miner_ich() else {
            return;
        };
        // ⚑ **Aus den Aufträgen im Zustand** (2026-10-29), nicht aus der
        //   letzten Ziehung: Ein Differenzbeweis legt Aufträge auch ausserhalb
        //   der Ziehung an. Gefragt wird jedes Mitglied der Besetzung.
        let meine: Vec<myl_ledger::pruefung::Pruefauftrag> = self
            .kette
            .zustand()
            .pruefauftraege
            .iter()
            .filter(|a| a.pruefer.contains(&ich) && !self.verlauf_gefragt.contains(&(a.epoche, a.pod, a.segment)))
            .cloned()
            .collect();
        if meine.is_empty() {
            return;
        }
        let (mut gefragt, mut ohne_adresse) = (0usize, 0usize);
        for a in &meine {
            let anfrage = myl_types::Spuranfrage { epoche: myl_types::ids::EpochId(a.epoche), pod: a.pod, segment: a.segment };
            for (m, _, _) in &a.besetzung {
                let an = self.kette.zustand().miner.get(m).and_then(|r| myl_net::peer_id_aus_bytes(&r.netzadresse).ok());
                let (Some(an), Some(daten)) = (an, Nachforderung::Verlauf(anfrage).als_bytes()) else {
                    ohne_adresse += 1;
                    continue;
                };
                if self.kommandos.send(NodeCommand::Anfrage { an, daten }).is_ok() {
                    gefragt += 1;
                }
            }
            self.verlauf_gefragt.insert((a.epoche, a.pod, a.segment));
        }
        while self.verlauf_gefragt.len() > 256 {
            self.verlauf_gefragt.pop_first();
        }
        self.protokoll.schreibe(
            Eintrag::neu("pruefer_zugeteilt")
                .zahl("epoche", meine.first().map_or(0, |a| a.epoche) as i64)
                .zahl("ziehungen", meine.len() as i64)
                .zahl("anfragen", gefragt as i64)
                .zahl("ohne_adresse", ohne_adresse as i64),
        );
    }

    /// ⚑ **Ein Verlauf ist da** (Stufe 2, T6c): prüfen gegen die Wurzel aus
    /// der Kette ([`myl_types::verlauf_pruefen`]), dann in einer
    /// Rechenaufgabe ab den Tokens nachrechnen
    /// ([`myl_verifier::verlauf_nachrechnen`]). Nur für eigene Ziehungen und
    /// nur einmal je Ziehung; ein Verlauf, der nicht trägt, lässt die Ziehung
    /// für die Antwort eines anderen Mitglieds offen.
    fn verlauf_erhalten(&mut self, von: libp2p::PeerId, antworten: Vec<myl_types::Spurantwort>) {
        let Some(gezogen) = antworten.last().map(|a| a.anfrage) else {
            return;
        };
        let schluessel = (gezogen.epoche.0, gezogen.pod, gezogen.segment);
        if !self.verlauf_gefragt.contains(&schluessel) || self.nachgerechnet.contains(&schluessel) {
            return;
        }
        // Die Wurzel aus dem Auftrag (sie bleibt dort, auch wenn neu gezogen
        // wird), sonst aus der letzten Ziehung.
        let z = self.kette.zustand();
        let bekannt = z
            .pruefauftraege
            .iter()
            .find(|a| (a.epoche, a.pod, a.segment) == schluessel)
            .map(|a| (a.wurzel, a.segmente))
            .or_else(|| z.stichprobe_wurzeln.iter().find(|(p, _, _)| *p == gezogen.pod).map(|(_, w, n)| (*w, *n)));
        let geprueft = match bekannt {
            None => Err("ohne Wurzel".to_string()),
            Some((w, n)) => myl_types::verlauf_pruefen(&antworten, &w, n).map_err(|f| format!("{f:?}")),
        };
        let tokens = match geprueft {
            Ok(t) => t,
            Err(f) => {
                self.protokoll.schreibe(
                    Eintrag::neu("verlauf_abgewiesen")
                        .text("von", von.to_string())
                        .zahl("epoche", gezogen.epoche.0 as i64)
                        .text("pod", gezogen.pod.to_hex())
                        .zahl("segment", i64::from(gezogen.segment))
                        .text("grund", f),
                );
                return;
            }
        };
        let Some(artefakte) = self.shardfuehrung.as_ref().map(|f| f.artefakte().to_path_buf()) else {
            self.protokoll.schreibe(Eintrag::neu("nachrechnen_ohne_modell").zahl("segment", i64::from(gezogen.segment)));
            return;
        };
        self.nachgerechnet.insert(schluessel);
        while self.nachgerechnet.len() > 256 {
            self.nachgerechnet.pop_first();
        }
        let ketten: Vec<Vec<myl_types::Hash>> = antworten.iter().map(|a| a.spur.clone()).collect();
        let modell = Arc::clone(&self.pruefmodell);
        let kanal = self.nachrechnungskanal.clone();
        let verlauf = antworten;
        tokio::task::spawn_blocking(move || {
            let beginn = std::time::Instant::now();
            let geladen = {
                let mut g = modell.lock().unwrap_or_else(|v| v.into_inner());
                if g.is_none() {
                    *g = integer_llm_runtime::loader::load_model_fuer_netz(&artefakte).ok().map(Arc::new);
                }
                g.clone()
            };
            let ergebnis = match &geladen {
                Some(m) => myl_verifier::verlauf_nachrechnen(m, &tokens, &ketten).map_err(|e| e.to_string()),
                None => Err(format!("Modell unter {} nicht geladen", artefakte.display())),
            };
            // ⚑ **Bei einer Abweichung gleich den Schiedsbeleg** (T6d): die
            //   Eingänge der strittigen Layer bis zur strittigen Position,
            //   samt dem Verlauf, den das Komitee gegen die Kette prüft.
            let beleg = match (&geladen, &ergebnis) {
                (Some(m), Ok(myl_verifier::Verlaufspruefung { ergebnis: myl_verifier::Verlaufsergebnis::Abweichung { position, eintrag }, .. })) => {
                    myl_verifier::schiedsbeleg(m, &tokens, *position, eintrag.saturating_sub(1).min(m.num_layers))
                        .ok()
                        .map(|e| (verlauf, e))
                }
                _ => None,
            };
            let _ = kanal.send(Nachrechnung {
                gezogen,
                positionen: tokens.len(),
                ergebnis,
                dauer_ms: beginn.elapsed().as_millis().min(i64::MAX as u128) as i64,
                beleg,
            });
        });
    }

    /// ⚑ **Festlegen** (Stufe 2, Festlegung des Projektinhabers vom
    /// 2026-10-29): aus dem Ergebnis den Befund, ein frisches Salz, und die
    /// Festlegung als Transaktion. Befund und Salz bleiben hier, bis die
    /// Kette das Aufdecken erlaubt ([`Self::aufdecken_wenn_erlaubt`]).
    fn festlegen(&mut self, g: myl_types::Spuranfrage, positionen: usize, p: &myl_verifier::Verlaufspruefung) {
        let Some(ich) = self.miner_ich() else {
            return;
        };
        let befund = myl_ledger::pruefung::Pruefbefund {
            logits: p.logits,
            abweichung: match p.ergebnis {
                myl_verifier::Verlaufsergebnis::Gleich => None,
                myl_verifier::Verlaufsergebnis::Abweichung { position, eintrag } => {
                    Some((position.min(u32::MAX as usize) as u32, eintrag.min(u32::MAX as usize) as u32))
                }
            },
            positionen: positionen.min(u32::MAX as usize) as u32,
        };
        let mut salz = [0u8; 32];
        if getrandom::fill(&mut salz).is_err() {
            return;
        }
        let festlegung = myl_ledger::pruefung::festlegung(&befund, &salz, &ich);
        self.festzulegen.push((g.epoche.0, g.pod, g.segment, festlegung));
        self.festgelegt.insert((g.epoche.0, g.pod, g.segment), (befund, salz));
    }

    /// ⚑ **Aufdecken, sobald die Kette es erlaubt**: Die eigene Festlegung
    /// steht im Auftrag, und alle haben festgelegt oder die erste Frist ist
    /// vorbei. Ein Auftrag, den es nicht mehr gibt, ist abgeschlossen.
    fn aufdecken_wenn_erlaubt(&mut self) {
        // Abgeschlossenes ins Protokoll: beim Erzeuger alles, sonst das
        // eigene.
        let ich = self.miner_ich();
        let ausgaben = self.kette.pruefausgaben_abholen();
        if self.konfig.erzeugt_bloecke {
            for (u, fehler) in &ausgaben.urteile {
                let f = &u.fall;
                self.protokoll.schreibe(
                    Eintrag::neu("streitfall_entschieden")
                        .zahl("epoche", f.epoche as i64)
                        .text("pod", f.pod.to_hex())
                        .zahl("segment", i64::from(f.segment))
                        .text("ausgang", format!("{:?}", u.ausgang).to_lowercase())
                        .text("anfechter", self.netzadresse_von(&f.anfechter))
                        .text("beschuldigt", f.beschuldigt.map(|m| self.netzadresse_von(&m)).unwrap_or_default())
                        .zahl("stimmen", f.stimmen.len() as i64)
                        .text("buchung", fehler.clone().unwrap_or_else(|| "gebucht".into())),
                );
            }
        }
        for ab in ausgaben.abschluesse {
            let a = &ab.auftrag;
            if !ich.is_some_and(|i| a.pruefer.contains(&i)) && !self.konfig.erzeugt_bloecke {
                continue;
            }
            self.protokoll.schreibe(
                Eintrag::neu("pruefung_abgeschlossen")
                    .zahl("epoche", a.epoche as i64)
                    .text("pod", a.pod.to_hex())
                    .zahl("segment", i64::from(a.segment))
                    .text("ausgang", format!("{:?}", ab.ausgang).to_lowercase())
                    .zahl("pruefer", a.pruefer.len() as i64)
                    .zahl("festgelegt", a.festlegungen.len() as i64)
                    .zahl("aufgedeckt", a.aufdeckungen.len() as i64)
                    .zahl("abweichungen", ab.abweichungen.len() as i64),
            );
        }
        let Some(ich) = ich else {
            return;
        };
        // Die Festlegungen gebündelt (`PRUEFUNGEN_JE_TX`): Ein Pod allein
        // wird voll gezogen, eine Transaktion je Ziehung sprengte die
        // Grenze je Absender im Mempool.
        while !self.festzulegen.is_empty() {
            let n = self.festzulegen.len().min(myl_consensus::block::PRUEFUNGEN_JE_TX);
            let festlegungen: Vec<_> = self.festzulegen.drain(..n).collect();
            let anzahl = festlegungen.len();
            let Some(tx) = self.netz_tx(myl_consensus::block::Anweisung::PruefungFestlegen { festlegungen }) else {
                self.festzulegen.clear();
                break;
            };
            self.ausstehend.push(tx);
            self.protokoll.schreibe(Eintrag::neu("pruefung_festgelegt").zahl("ziehungen", anzahl as i64));
        }
        let hoehe = self.kette.zustand().historie.anzahl();
        let mut fertig = Vec::new();
        let mut decken = Vec::new();
        for (schluessel, (befund, salz)) in &self.festgelegt {
            let (epoche, pod, segment) = *schluessel;
            let auftrag = self
                .kette
                .zustand()
                .pruefauftraege
                .iter()
                .find(|a| a.epoche == epoche && a.pod == pod && a.segment == segment);
            let Some(a) = auftrag else {
                fertig.push(*schluessel);
                continue;
            };
            let meine = a.festlegungen.iter().any(|(m, _)| *m == ich);
            let frei = a.festlegungen.len() == a.pruefer.len() || hoehe > a.frist_festlegen;
            if meine && frei && hoehe <= a.frist_aufdecken {
                decken.push((*schluessel, befund.clone(), *salz));
            }
        }
        for teil in decken.chunks(myl_consensus::block::PRUEFUNGEN_JE_TX) {
            let aufdeckungen: Vec<_> = teil.iter().map(|((e, p, s), b, z)| (*e, *p, *s, b.clone(), *z)).collect();
            if let Some(tx) = self.netz_tx(myl_consensus::block::Anweisung::PruefungAufdecken { aufdeckungen }) {
                self.ausstehend.push(tx);
                fertig.extend(teil.iter().map(|(k, _, _)| *k));
                self.protokoll.schreibe(Eintrag::neu("pruefung_aufgedeckt").zahl("ziehungen", teil.len() as i64));
            }
        }
        for s in fertig {
            self.festgelegt.remove(&s);
        }
    }

    /// Meldet einen Differenzbeweis mit dem eigenen Schlüssel.
    fn differenz_selbst_melden(
        &mut self,
        epoche: u64,
        pod_a: myl_types::ids::PodId,
        pod_b: myl_types::ids::PodId,
        beweis: myl_types::Differenzbeweis,
    ) {
        let id = beweis.a.ausgabe.id;
        if let Some(tx) = self.netz_tx(myl_consensus::block::Anweisung::DifferenzMelden {
            epoche,
            pod_a,
            pod_b,
            beweis: Box::new(beweis),
        }) {
            self.ausstehend.push(tx);
            self.protokoll.schreibe(
                Eintrag::neu("differenz_gemeldet").zahl("epoche", epoche as i64).text("segment", hex::encode(&id.as_ref()[..8])),
            );
        }
    }

    /// Die Netzadresse eines Miners als Text, sonst seine Kennung kurz.
    fn netzadresse_von(&self, m: &myl_types::ids::MinerId) -> String {
        self.kette
            .zustand()
            .miner
            .get(m)
            .and_then(|r| myl_net::peer_id_aus_bytes(&r.netzadresse).ok())
            .map_or_else(|| hex::encode(&m.as_bytes()[..8]), |p| p.to_string())
    }

    /// ⚑ **Die Schiedsrunde im Komitee** (T6d, Whitepaper 6.6): Ein
    /// Validator holt zu jedem offenen Streitfall den Beleg beim Anfechter,
    /// rechnet die strittige Layer nach und stimmt ab. Bekommt er bis zur
    /// Hälfte des Fensters keinen Beleg, stimmt er gegen die Anfechtung: Die
    /// Beweislast trägt der Anfechter.
    fn schied_pflegen(&mut self) {
        while let Ok(r) = self.schiedsrechnungen.try_recv() {
            self.schiedsstimme_abgeben(r);
        }
        let Some(ich) = self.miner_ich() else {
            return;
        };
        if !self.kette.stimmsatz().is_some_and(|v| v.contains(&ich)) {
            return;
        }
        let hoehe = self.kette.zustand().historie.anzahl();
        let faelle: Vec<myl_ledger::pruefung::Streitfall> = self.kette.zustand().streitfaelle.clone();
        for f in faelle {
            let schluessel = (f.epoche, f.pod, f.segment);
            if self.schied_gestimmt.contains(&schluessel) || f.stimmen.iter().any(|(m, _)| *m == ich) {
                continue;
            }
            if hoehe.saturating_add(crate::kette::Kette::SCHIED_FENSTER_BLOECKE / 2) > f.frist {
                self.schiedsstimme_abgeben(Schiedsrechnung { fall: schluessel, pod_falsch: Ok(false) });
                continue;
            }
            let neu = self.schied_angefragt.get(&schluessel).is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(20));
            if !neu {
                continue;
            }
            let anfrage = myl_types::Spuranfrage { epoche: myl_types::ids::EpochId(f.epoche), pod: f.pod, segment: f.segment };
            let an = self
                .kette
                .zustand()
                .miner
                .get(&f.anfechter)
                .and_then(|r| myl_net::peer_id_aus_bytes(&r.netzadresse).ok());
            if let (Some(an), Some(daten)) = (an, Nachforderung::Schiedsbeleg(anfrage).als_bytes()) {
                let _ = self.kommandos.send(NodeCommand::Anfrage { an, daten });
                self.schied_angefragt.insert(schluessel, std::time::Instant::now());
                self.protokoll.schreibe(
                    Eintrag::neu("schiedsbeleg_angefragt").zahl("epoche", f.epoche as i64).zahl("segment", i64::from(f.segment)),
                );
            }
        }
        while self.schied_angefragt.len() > 256 {
            self.schied_angefragt.pop_first();
        }
    }

    /// Ein Schiedsbeleg ist da: gegen die Kette prüfen, dann nachrechnen.
    fn schiedsbeleg_erhalten(&mut self, verlauf: Vec<myl_types::Spurantwort>, eingaenge: Vec<Vec<i16>>) {
        let Some(gezogen) = verlauf.last().map(|a| a.anfrage) else {
            return;
        };
        let schluessel = (gezogen.epoche.0, gezogen.pod, gezogen.segment);
        if self.schied_gestimmt.contains(&schluessel) {
            return;
        }
        let Some(fall) = self
            .kette
            .zustand()
            .streitfaelle
            .iter()
            .find(|f| (f.epoche, f.pod, f.segment) == schluessel)
            .cloned()
        else {
            return;
        };
        // Der Verlauf muss gegen die Wurzel des Bündels tragen; sonst trägt
        // der Beleg nicht, und das fällt auf den Anfechter.
        let tokens = match myl_types::verlauf_pruefen(&verlauf, &fall.wurzel, fall.segmente) {
            Ok(t) => t,
            Err(_) => {
                self.schiedsstimme_abgeben(Schiedsrechnung { fall: schluessel, pod_falsch: Ok(false) });
                return;
            }
        };
        let Some(artefakte) = self.shardfuehrung.as_ref().map(|f| f.artefakte().to_path_buf()) else {
            return;
        };
        let ketten: Vec<Vec<myl_types::Hash>> = verlauf.iter().map(|a| a.spur.clone()).collect();
        let modell = Arc::clone(&self.pruefmodell);
        let kanal = self.schiedskanal.clone();
        self.schied_gestimmt.insert(schluessel);
        tokio::task::spawn_blocking(move || {
            let geladen = {
                let mut g = modell.lock().unwrap_or_else(|v| v.into_inner());
                if g.is_none() {
                    *g = integer_llm_runtime::loader::load_model_fuer_netz(&artefakte).ok().map(Arc::new);
                }
                g.clone()
            };
            let pod_falsch = match geladen {
                Some(m) => myl_verifier::schied_nachrechnen(
                    &m,
                    &tokens,
                    &ketten,
                    fall.position as usize,
                    fall.eintrag as usize,
                    &eingaenge,
                )
                .map_err(|e| e.to_string()),
                None => Err(format!("Modell unter {} nicht geladen", artefakte.display())),
            };
            let _ = kanal.send(Schiedsrechnung { fall: schluessel, pod_falsch });
        });
    }

    /// Die Stimme als Transaktion; ohne Ergebnis (das Modell fehlte) keine.
    fn schiedsstimme_abgeben(&mut self, r: Schiedsrechnung) {
        let (epoche, pod, segment) = r.fall;
        self.schied_gestimmt.insert(r.fall);
        while self.schied_gestimmt.len() > 256 {
            self.schied_gestimmt.pop_first();
        }
        let mut eintrag = Eintrag::neu("schiedsstimme").zahl("epoche", epoche as i64).zahl("segment", i64::from(segment));
        match r.pod_falsch {
            Ok(pod_falsch) => {
                if let Some(tx) =
                    self.netz_tx(myl_consensus::block::Anweisung::SchiedsstimmeAbgeben { epoche, pod, segment, pod_falsch })
                {
                    self.ausstehend.push(tx);
                }
                eintrag = eintrag.wahr("pod_falsch", pod_falsch);
            }
            Err(f) => eintrag = eintrag.text("fehler", f),
        }
        self.protokoll.schreibe(eintrag);
    }

    /// Schreibt ein Ergebnis des Nachrechnens ins Protokoll.
    ///
    /// ⚑ **Bei einer Abweichung auch, wen sie trifft**: das Mitglied, dessen
    /// Bereich die abweichende Layer enthält, aus dem Schnitt des Pods in der
    /// gezogenen Epoche. Das ist die Grundlage für den Schuldspruch (T6d);
    /// eingereicht wird hier noch nichts.
    fn nachrechnung_vermerken(&mut self, n: Nachrechnung) {
        let g = n.gezogen;
        let mut eintrag = Eintrag::neu("stichprobe_nachgerechnet")
            .zahl("epoche", g.epoche.0 as i64)
            .text("pod", g.pod.to_hex())
            .zahl("segment", i64::from(g.segment))
            .zahl("positionen", n.positionen as i64)
            .zahl("dauer_ms", n.dauer_ms);
        if let Ok(p) = &n.ergebnis {
            self.festlegen(g, n.positionen, p);
        }
        if let Some(b) = n.beleg {
            self.schiedsbelege.insert((g.epoche.0, g.pod, g.segment), b);
            while self.schiedsbelege.len() > 64 {
                self.schiedsbelege.pop_first();
            }
        }
        eintrag = match n.ergebnis.map(|p| p.ergebnis) {
            Ok(myl_verifier::Verlaufsergebnis::Gleich) => eintrag.text("ergebnis", "gleich"),
            Ok(myl_verifier::Verlaufsergebnis::Abweichung { position, eintrag: stelle }) => {
                // Eintrag j + 1 ist die Ausgabe der Layer j.
                let layer = stelle.saturating_sub(1) as u32;
                // ⚠️ Die Grenzen aus den Zusagen nach dem Stand von jetzt; für
                //    den Schuldspruch (T6d) müssen es die der gezogenen Epoche
                //    sein. Hier steht es nur im Protokoll.
                let wer = self
                    .kette
                    .pod_der_kennung(g.epoche.0, &g.pod)
                    .and_then(|pod| {
                        let grenzen = crate::kette::Kette::grenzen_des_pods(self.kette.zustand(), &pod)?;
                        let stelle = grenzen.iter().position(|(von, bis)| *von <= layer && layer < *bis)?;
                        pod.shards.get(stelle).map(|s| {
                            myl_net::peer_id_aus_bytes(&s.miner.netzadresse)
                                .map_or_else(|_| hex::encode(&s.miner.miner_id.as_bytes()[..8]), |p| p.to_string())
                        })
                    })
                    .unwrap_or_default();
                eintrag
                    .text("ergebnis", "abweichung")
                    .zahl("position", position as i64)
                    .zahl("layer", i64::from(layer))
                    .text("beschuldigt", wer)
            }
            Err(f) => eintrag.text("ergebnis", "fehler").text("grund", f),
        };
        self.protokoll.schreibe(eintrag);
    }

    /// Die Antwort eines Mitglieds auf eine weitergereichte Übernahme (6.8)
    /// oder Spurablage (Stufe 2): `was` bei Erfolg.
    fn relais_beantworten(&mut self, von: libp2p::PeerId, kennung: u64, fehler: Option<String>, was: &str) {
        use myl_types::relais::Relaisantwort;
        if !self.shardrelais.get(&kennung).is_some_and(|(p, _, _)| *p == von) {
            return;
        }
        if let Some((_, rueck, _)) = self.shardrelais.remove(&kennung) {
            let _ = rueck.send(match fehler {
                Some(f) => Relaisantwort::Fehler(format!("{von}: {f}")),
                None => Relaisantwort::Erledigt(format!("{was} von {von}")),
            });
        }
    }

    /// Die Antwort eines Mitglieds auf eine weitergereichte Shardanfrage.
    fn shard_antwort(&mut self, von: libp2p::PeerId, kennung: u64, antwort: Vec<u8>, fehler: Option<String>) {
        use myl_types::relais::Relaisantwort;
        // ⚑ Nur vom Gefragten: Eine Antwort mit fremder Kennung von einem
        //   anderen Knoten wird verworfen.
        if !self.shardrelais.get(&kennung).is_some_and(|(p, _, _)| *p == von) {
            return;
        }
        if let Some((_, rueck, seit)) = self.shardrelais.remove(&kennung) {
            // Die Dauer einer Runde über das Netz: Ein Token geht durch jede
            // Position einmal, und hier steht, was eine davon kostet.
            let mut eintrag = Eintrag::neu("shardrelais")
                .text("an", von.to_string())
                .zahl("dauer_ms", seit.elapsed().as_millis().min(i64::MAX as u128) as i64)
                .zahl("bytes", antwort.len() as i64);
            if let Some(f) = &fehler {
                eintrag = eintrag.text("fehler", f.clone());
            }
            self.protokoll.schreibe(eintrag);
            let _ = rueck.send(match fehler {
                Some(f) => Relaisantwort::Fehler(format!("Mitglied {von}: {f}")),
                None => Relaisantwort::Shard(antwort),
            });
        }
    }

    /// Alle weitergereichten Anfragen an `an` scheitern.
    fn shardrelais_scheitern(&mut self, an: &libp2p::PeerId, grund: &str) {
        let weg: Vec<u64> = self.shardrelais.iter().filter(|(_, (p, _, _))| p == an).map(|(k, _)| *k).collect();
        for k in weg {
            if let Some((_, rueck, _)) = self.shardrelais.remove(&k) {
                let _ = rueck.send(myl_types::relais::Relaisantwort::Fehler(format!("Mitglied {an} nicht erreicht: {grund}")));
            }
        }
    }

    /// Was die Kette über eine Adresse weiß (`/konto/<hex>`).
    pub fn kontoauskunft(&self, adresse: &myl_types::ids::Address) -> crate::beobachtung::Kontoauskunft {
        let z = self.kette.zustand();
        let konto = z.account(adresse);
        crate::beobachtung::Kontoauskunft {
            hoehe: self.kette.hoehe(),
            guthaben: konto.balance,
            nonce: konto.nonce,
            miner: z.miner.contains_key(&myl_types::ids::MinerId::new(*adresse.as_bytes())),
            auszahlung_fuer: z.auszahlung.values().filter(|k| *k == adresse).count() as u64,
            hinterlegt: z.hinterlegungen.values().filter(|(wer, _)| wer == adresse).map(|(_, b)| *b).sum(),
        }
    }

    /// Guthaben des Auszahlungskontos aus `--konto` und ob die Kette es als
    /// das dieses Miners führt.
    fn auszahlung_stand(&self) -> (u64, bool) {
        let Some(soll) = self.konfig.konto else {
            return (0, false);
        };
        let zustand = self.kette.zustand();
        let eingetragen = self
            .miner_ich()
            .and_then(|ich| myl_ledger::transitions::auszahlungskonto(zustand, &ich))
            == Some(soll);
        (self.kette.konto(&soll).balance, eingetragen)
    }

    /// ⚑ **Anmeldung und Auszahlungskonto** (Fund 643, 2026-10-26): Mit
    /// `--konto` meldet sich der Knoten als Miner an, falls nicht schon über
    /// die Zusage, und trägt das Konto ein. Die erste Eintragung
    /// unterschreibt der Miner selbst, jede weitere das bisher eingetragene
    /// Konto (`--kontowechsel`); fehlt dessen Schlüssel, bleibt das alte
    /// stehen, und die Zeile `auszahlungskonto` sagt beide.
    fn auszahlung_pflegen(&mut self, ich: &myl_types::ids::MinerId) {
        let Some(soll) = self.konfig.konto else {
            return;
        };
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        let ist = myl_ledger::transitions::auszahlungskonto(self.kette.zustand(), ich);
        if self.auszahlung_gemeldet != Some(ist) {
            self.auszahlung_gemeldet = Some(ist);
            let mut e = Eintrag::neu("auszahlungskonto").text("soll", hex(soll.as_bytes()));
            if let Some(i) = ist {
                e = e.text("ist", hex(i.as_bytes()));
            }
            self.protokoll.schreibe(e);
        }
        if ist == Some(soll) {
            return;
        }
        if !self.kette.zustand().miner.contains_key(ich) {
            // Die Zusage meldet selbst an; ohne sie geschieht es hier.
            if self.konfig.modellzusage.is_none() {
                self.anmeldung_senden(ich);
            }
            return;
        }
        if self.schon_gesendet("auszahlung") {
            return;
        }
        let anweisung = myl_consensus::block::Anweisung::AuszahlungskontoEintragen { kennung: *ich, konto: soll };
        let tx = match ist {
            None => self.netz_tx(anweisung),
            Some(bisher) => {
                if !self.kontowechsel.as_ref().is_some_and(|k| k.adresse() == bisher) {
                    return;
                }
                // Das bisherige Konto zahlt die Gebühr des Wechsels (G1).
                if !self.probehahn(&bisher, PROBEHAHN_POLSTER / 2, "probehahn_wechsel") {
                    return;
                }
                let Some(k) = self.kontowechsel.as_ref().filter(|k| k.adresse() == bisher) else {
                    return;
                };
                let nonce = self.kette.konto(&bisher).nonce;
                k.transaktion(&self.kette.netzwert(), nonce, anweisung).ok()
            }
        };
        if let Some(tx) = tx {
            self.tx_verbreiten(tx);
            self.protokoll.schreibe(Eintrag::neu("auszahlungskonto_eingereicht").text("konto", hex(soll.as_bytes())));
        }
    }

    /// ⚑ **Der Shard aus der Kette** (P7).
    fn shard_abgleichen(&mut self, ich: &myl_types::ids::MinerId) {
        let jetzt = self.kette.zustand().epoch.0;
        // ⚑ Eine Übernahme gilt bis zum Ende ihrer Epoche (6.8).
        self.uebernahme = self.uebernahme.take().filter(|a| a.epoche == jetzt);
        let mut soll = crate::shardfuehrung::auftrag_der_epoche(self.kette.zustand(), ich).or_else(|| self.uebernahme.clone());
        // ⚑ **Erst starten, wenn die Dateien da sind** (P11): Fehlt dem
        //   Bereich etwas, holt der Dateidienst es zuerst; ein Shard, der
        //   beim Laden abbricht, würde sonst jede Aufnahme neu gestartet.
        if let (Some(a), Some(sk)) = (soll.as_ref(), self.konfig.shard.as_ref()) {
            let fehlt = crate::dateidienst::fehlende(&sk.artefakte, a.von, a.bis, a.mit_einbettung(), a.mit_kopf())
                .map(|f| f.len())
                .unwrap_or(0);
            if fehlt > 0 {
                if !self.schon_gesendet("shard_wartet") {
                    self.protokoll.schreibe(Eintrag::neu("shard_wartet_auf_dateien").zahl("fehlen", fehlt as i64));
                }
                soll = None;
            }
        }
        let Some(f) = self.shardfuehrung.as_mut() else {
            return;
        };
        match f.abgleichen(soll) {
            Ok(crate::shardfuehrung::Wechsel::Unveraendert) => {}
            Ok(crate::shardfuehrung::Wechsel::Gestartet(a)) => self.protokoll.schreibe(
                Eintrag::neu("shard_gestartet")
                    .zahl("epoche", a.epoche as i64)
                    .zahl("pod", a.pod as i64)
                    .zahl("position", a.position as i64)
                    .zahl("von", a.von as i64)
                    .zahl("bis", a.bis as i64),
            ),
            Ok(crate::shardfuehrung::Wechsel::Beendet) => self.protokoll.schreibe(Eintrag::neu("shard_beendet")),
            Err(e) => self.protokoll.schreibe(Eintrag::neu("shard_nicht_gestartet").text("grund", e)),
        }
    }

    /// ⚑ **Je Epoche jedes andere rechnende Mitglied einmal messen** (6.8):
    /// Zeitproben, ohne Ping (Fund 646, [`crate::frist::gemessen_je_token`]).
    ///
    /// ⚑ **Jedes Mitglied des Pods misst, auch die Reserve**: Sie rechnet
    /// nicht und kann gerade deshalb bezeugen. Wer noch nicht gemessen ist,
    /// wird bei jeder Aufnahme wieder gefragt, solange keine Messung an ihn
    /// läuft; ein Shard, der beim ersten Versuch noch lädt, entgeht der
    /// Frist so nicht.
    fn fristen_messen(&mut self, ich: &myl_types::ids::MinerId) {
        let epoche = self.kette.zustand().epoch.0;
        let hoehe = self.kette.hoehe();
        self.gemessen.retain(|(e, _)| *e == epoche);
        self.fristverdacht.retain(|(e, _), _| *e == epoche);
        // 📌 Fund 670: Eine Messung aus der vorigen Epoche gilt nicht mehr;
        //    Pod, Frist und Fenster sind andere.
        let verfallen: Vec<libp2p::PeerId> =
            self.messungen.iter().filter(|(_, m)| m.epoche != epoche).map(|(p, _)| *p).collect();
        for p in verfallen {
            if let Some(m) = self.messungen.remove(&p) {
                self.messung_verworfen(&m, "fristmessung_verfallen");
            }
        }
        // 📌 Fund 655: nicht im Anlauf der Epoche, wenn die Shards gerade
        //    starten.
        if !crate::frist::darf_messen(hoehe) {
            return;
        }
        // Wer über der Grenze seiner Probe ist, hat die Frist überschritten,
        // auch ohne Antwort (frist::grenze_der_probe); wer nach einer
        // Abweisung wartet, hat keine Probe offen.
        let ueber: Vec<libp2p::PeerId> = self
            .messungen
            .iter()
            .filter(|(_, m)| {
                m.wieder_ab.is_none()
                    && m.ab.elapsed().as_micros() as u64 >= crate::frist::grenze_der_probe(m.frist, m.positionen)
            })
            .map(|(p, _)| *p)
            .collect();
        for p in ueber {
            self.messung_ueberschritten(p);
        }
        // ⚑ Fund 660: Nach einer Abweisung wird mit Abstand erneut gefragt,
        //   dieselbe Messung mit einer neuen Saat.
        let jetzt = std::time::Instant::now();
        let faellig: Vec<libp2p::PeerId> =
            self.messungen.iter().filter(|(_, m)| m.wieder_ab.is_some_and(|t| t <= jetzt)).map(|(p, _)| *p).collect();
        for an in faellig {
            if let Some(m) = self.messungen.get_mut(&an) {
                m.wieder_ab = None;
                m.ab = jetzt;
                m.saat = crate::frist::frische_saat();
                if let Some(daten) = crate::frist::probe(m.frist, m.positionen, m.saat).als_bytes() {
                    let _ = self.kommandos.send(NodeCommand::AnfrageMitZweck { an, daten, zweck: ZWECK_ZEITPROBE });
                }
            }
        }
        let laufend: Vec<myl_types::ids::MinerId> = self.messungen.values().map(|m| m.mitglied).collect();
        // Eine Messung, deren Probe nach zwei Minuten nicht zurück ist, gilt
        // als verloren und darf neu gestellt werden; eine Probe rechnet bis
        // zu 2 048 Positionen (Fund 646), und die Leitung zum Shard wartet
        // ebenso lange.
        let verloren: Vec<libp2p::PeerId> =
            self.messungen.iter().filter(|(_, m)| m.ab.elapsed() >= Duration::from_secs(120)).map(|(p, _)| *p).collect();
        for p in verloren {
            if let Some(m) = self.messungen.remove(&p) {
                self.messung_verworfen(&m, "fristmessung_verloren");
            }
        }
        for (pod, m) in crate::frist::zu_messen(self.kette.zustand(), ich) {
            if self.gemessen.contains(&(epoche, m.miner_id)) || laufend.contains(&m.miner_id) {
                continue;
            }
            // Nach einem Verdacht erst im zweiten Fenster wieder.
            if self
                .fristverdacht
                .get(&(epoche, m.miner_id))
                .is_some_and(|(h0, _)| !crate::frist::zweites_fenster_offen(hoehe, *h0))
            {
                continue;
            }
            let Ok(an) = myl_net::peer_id_aus_bytes(&m.netzadresse) else {
                continue;
            };
            // Ohne Frist nichts zu messen; die Grösse der Probe folgt aus ihr.
            let Some(frist) = crate::frist::frist_des_mitglieds(self.kette.zustand(), pod, &m.miner_id) else {
                self.gemessen.insert((epoche, m.miner_id));
                continue;
            };
            let positionen = crate::frist::probe_positionen(frist);
            let saat = crate::frist::frische_saat();
            let Some(daten) = crate::frist::probe(frist, positionen, saat).als_bytes() else {
                continue;
            };
            let messende = crate::frist::messende(self.kette.zustand(), &m.miner_id).len();
            self.messungen.insert(
                an,
                Messung {
                    pod,
                    mitglied: m.miner_id,
                    ab: std::time::Instant::now(),
                    positionen,
                    frist,
                    probe_us: None,
                    proben: 0,
                    saat,
                    epoche,
                    messende,
                    abgewiesen_in_folge: 0,
                    abgewiesen_seit: None,
                    abgewiesen: 0,
                    wieder_ab: None,
                },
            );
            // Die Diagnose der Messplanung (Funde 664, 667): wer wen in
            // welcher Epoche zu messen beginnt.
            self.protokoll.schreibe(
                Eintrag::neu("fristmessung_gestellt")
                    .text("mitglied", hex::encode(&m.miner_id.as_bytes()[..8]))
                    .zahl("epoche", epoche as i64)
                    .zahl("hoehe", hoehe as i64)
                    .zahl("positionen", i64::from(positionen))
                    .zahl("messende", messende as i64),
            );
            let _ = self.kommandos.send(NodeCommand::AnfrageMitZweck { an, daten, zweck: ZWECK_ZEITPROBE });
        }
    }

    /// Schreibt eine Messung ins Protokoll, die ohne Urteil endet.
    fn messung_verworfen(&mut self, m: &Messung, art: &str) {
        self.protokoll.schreibe(
            Eintrag::neu(art)
                .text("mitglied", hex::encode(&m.mitglied.as_bytes()[..8]))
                .zahl("epoche", m.epoche as i64)
                .zahl("proben", i64::from(m.proben))
                .zahl("abgewiesen", i64::from(m.abgewiesen)),
        );
    }

    /// Die Gegenstelle, von der gerade eine Datei nachgeholt wird. Ob ein
    /// `Nichts` zu einer Messung gehört, sagt seit 2026-10-30 der Zweck der
    /// Anfrage, nicht mehr die Gegenstelle.
    fn ist_nachholquelle(&self, peer: &libp2p::PeerId) -> bool {
        self.nachholen.as_ref().is_some_and(|n| n.laeuft && n.quellen.get(n.quelle) == Some(peer))
    }

    /// Trägt die Unterschrift einer Antwort, unter dem Schlüssel, mit dem
    /// das gemessene Mitglied in der Kette steht, über die Saat der
    /// laufenden Probe (Fund 669, Fund 661)?
    fn zeitprobe_traegt(&self, von: &libp2p::PeerId, abdruck: &[u8; 32], signatur: &myl_types::bls::BlsSignature) -> bool {
        self.messungen.get(von).is_some_and(|m| {
            self.kette.zustand().miner.get(&m.mitglied).is_some_and(|eintrag| {
                crate::frist::antwort_traegt(&eintrag.schluessel, &m.saat, m.positionen, abdruck, signatur)
            })
        })
    }

    /// Eine Zeitprobe ist zurück: die nächste stellen oder, nach der letzten,
    /// die kürzeste gegen die Frist halten und, wenn darüber, ein Zeugnis
    /// unterschreiben und verteilen (Fund 639: die kürzeste zählt).
    fn zeitprobe_erhalten(&mut self, von: libp2p::PeerId) {
        let Some(m) = self.messungen.get_mut(&von) else {
            return;
        };
        let dauer = m.ab.elapsed().as_micros() as u64;
        let kuerzeste = m.probe_us.map_or(dauer, |p| p.min(dauer));
        m.probe_us = Some(kuerzeste);
        m.proben += 1;
        m.abgewiesen_in_folge = 0;
        m.abgewiesen_seit = None;
        // ⚑ Hält die kürzeste schon die Frist, ist das Urteil entschieden
        //   (frist::messung_fertig): Der Platz des Gemessenen wird frei.
        if !crate::frist::messung_fertig(kuerzeste, m.proben, m.frist, m.positionen) {
            m.ab = std::time::Instant::now();
            m.saat = crate::frist::frische_saat();
            if let Some(daten) = crate::frist::probe(m.frist, m.positionen, m.saat).als_bytes() {
                let _ = self.kommandos.send(NodeCommand::AnfrageMitZweck { an: von, daten, zweck: ZWECK_ZEITPROBE });
            }
            return;
        }
        let Some(m) = self.messungen.remove(&von) else {
            return;
        };
        self.messung_auswerten(m, "beantwortet");
    }

    /// ⚑ **Eine Abweisung** (Fund 660): ein `Nichts` oder eine gescheiterte
    /// Anfrage vor der Grenze der Probe. Die Messung bleibt mit ihren
    /// beantworteten Proben stehen und fragt mit Abstand erneut; erst eine
    /// lange Folge ohne Antwort endet als Überschreitung
    /// ([`crate::frist::nach_abweisung`]).
    fn messung_abgewiesen(&mut self, von: libp2p::PeerId, art: &str) {
        let saat = crate::frist::frische_saat();
        let zufall = u64::from_le_bytes([saat[0], saat[1], saat[2], saat[3], saat[4], saat[5], saat[6], saat[7]]);
        let jetzt = std::time::Instant::now();
        let Some(m) = self.messungen.get_mut(&von) else {
            return;
        };
        m.abgewiesen += 1;
        m.abgewiesen_in_folge += 1;
        let seit = *m.abgewiesen_seit.get_or_insert(jetzt);
        let seit_us = jetzt.saturating_duration_since(seit).as_micros() as u64;
        let folge = crate::frist::nach_abweisung(m.abgewiesen_in_folge, seit_us, m.messende, zufall);
        self.protokoll.schreibe(
            Eintrag::neu("zeitprobe_abgewiesen")
                .text("mitglied", hex::encode(&m.mitglied.as_bytes()[..8]))
                .text("wie", art)
                .zahl("epoche", m.epoche as i64)
                .zahl("in_folge", i64::from(m.abgewiesen_in_folge))
                .zahl("seit_ms", (seit_us / 1_000) as i64),
        );
        match folge {
            crate::frist::NachAbweisung::Wiederholen(us) => {
                m.wieder_ab = Some(jetzt + Duration::from_micros(us));
            }
            crate::frist::NachAbweisung::Ueberschritten => {
                let Some(mut m) = self.messungen.remove(&von) else {
                    return;
                };
                // Ohne beantwortete Probe zählt sie wie eine, die in ihre
                // Grenze lief; mit beantworteten sind die schon darüber
                // (sonst hätte das frühe Ende sie beendet).
                m.probe_us = Some(m.probe_us.unwrap_or(crate::frist::grenze_der_probe(m.frist, m.positionen)));
                self.messung_auswerten(m, "verweigert");
            }
        }
    }

    /// Beantwortet eine Zeitprobe über den eigenen Shard: nur für Mitglieder
    /// des eigenen Pods laut Kette, und eine zugleich; mit Grenze hört der
    /// Shard dort auf (Fund 646). `None` heisst: Die Antwort geht später.
    fn zeitprobe_beantworten(
        &mut self,
        von: &libp2p::PeerId,
        marke: u64,
        positionen: u32,
        grenze_ms: Option<u32>,
        saat: Option<[u8; 32]>,
    ) -> Option<(Nachlieferung, usize)> {
        let messend = self
            .miner_ich()
            .is_some_and(|ich| crate::frist::misst_mich(self.kette.zustand(), &ich, von));
        let platz = if messend { self.zeitprobe_platz.nehmen() } else { None };
        let Some(((adresse, ausweis), platz)) = self.shardfuehrung.as_mut().and_then(|f| f.tuer()).zip(platz) else {
            return Some((Nachlieferung::Nichts, 0));
        };
        let sender = self.kommandos.clone();
        let schluessel = self.netzschluessel.clone();
        tokio::task::spawn_blocking(move || {
            let gefragt = positionen;
            let positionen = positionen.min(crate::frist::PROBE_HOECHSTENS);
            let frist = Duration::from_secs(120);
            let ergebnis = match (grenze_ms, saat.as_ref()) {
                (Some(ms), Some(s)) => myl_types::zeitprobe::fragen_mit_saat(adresse, &ausweis, positionen, ms, s, frist),
                (Some(ms), None) => myl_types::zeitprobe::fragen_mit_grenze(adresse, &ausweis, positionen, ms, frist),
                (None, _) => myl_types::zeitprobe::fragen(adresse, &ausweis, positionen, frist),
            };
            // ⚑ Mit Saat unterschrieben (Fund 661): über Saat, die gefragten
            //   Positionen und den Abdruck, mit dem Minerschlüssel.
            let antwort = match (ergebnis, saat, schluessel) {
                (Ok(abdruck), Some(s), Some(k)) => {
                    match k.signiere(&myl_types::zeitprobe::antwortnachricht(&s, gefragt, &abdruck)) {
                        Ok(signatur) => Nachlieferung::ZeitprobeUnterschrieben { abdruck, signatur },
                        Err(_) => Nachlieferung::Nichts,
                    }
                }
                (Ok(abdruck), _, _) => Nachlieferung::Zeitprobe(abdruck),
                (Err(_), _, _) => Nachlieferung::Nichts,
            };
            drop(platz);
            if let Some(bytes) = antwort.als_bytes() {
                let _ = sender.send(NodeCommand::Antwort { marke, daten: bytes });
            }
        });
        None
    }

    /// Hat die laufende Probe an `von` ihre Grenze überschritten, endet die
    /// Messung mit der verstrichenen Zeit (`true`); sonst nichts.
    fn messung_ueberschritten(&mut self, von: libp2p::PeerId) -> bool {
        let Some(m) = self.messungen.get(&von) else {
            return false;
        };
        // Wer nach einer Abweisung wartet, hat keine Probe offen.
        if m.wieder_ab.is_some() {
            return false;
        }
        let dauer = m.ab.elapsed().as_micros() as u64;
        if dauer < crate::frist::grenze_der_probe(m.frist, m.positionen) {
            return false;
        }
        let Some(mut m) = self.messungen.remove(&von) else {
            return false;
        };
        m.probe_us = Some(m.probe_us.map_or(dauer, |p| p.min(dauer)));
        self.protokoll.schreibe(
            Eintrag::neu("zeitprobe_ueberschritten")
                .text("mitglied", hex::encode(&m.mitglied.as_bytes()[..8]))
                .zahl("dauer_us", dauer.min(i64::MAX as u64) as i64)
                .zahl("positionen", i64::from(m.positionen)),
        );
        self.messung_auswerten(m, "grenze");
        true
    }

    /// Wertet eine beendete Messung aus: Urteil, Verdacht oder Zeugnis.
    /// `ende` sagt, wie sie endete: beantwortet, an der Grenze oder
    /// verweigert (Fund 660).
    fn messung_auswerten(&mut self, m: Messung, ende: &str) {
        let Some(probe_us) = m.probe_us else {
            return;
        };
        // 📌 Fund 646: kein Ping, also nichts, was der Gemessene verlängern
        //    könnte, ausser seiner eigenen Probe.
        let gemessen = crate::frist::gemessen_je_token(probe_us, m.positionen);
        let e = self.kette.zustand().epoch.0;
        let hoehe = self.kette.hoehe();
        // 📌 Fund 670: Endet eine Messung erst nach dem Wechsel, gilt sie
        //    nicht; Pod und Frist der neuen Epoche sind andere.
        if m.epoche != e {
            self.messung_verworfen(&m, "fristmessung_verfallen");
            return;
        }
        let zustand = self.kette.zustand();
        let Some(frist) = crate::frist::frist_des_mitglieds(zustand, m.pod, &m.mitglied) else {
            self.gemessen.insert((e, m.mitglied));
            return;
        };
        let schluessel = (e, m.mitglied);
        let urteil = crate::frist::urteil(gemessen, frist, hoehe, self.fristverdacht.get(&schluessel).copied());
        self.protokoll.schreibe(
            Eintrag::neu("fristmessung")
                .text("mitglied", hex::encode(&m.mitglied.as_bytes()[..8]))
                .zahl("gemessen_us", gemessen.min(i64::MAX as u64) as i64)
                .zahl("probe_us", probe_us.min(i64::MAX as u64) as i64)
                .zahl("frist_us", frist.min(i64::MAX as u64) as i64)
                .text("urteil", format!("{urteil:?}"))
                .text("ende", ende)
                .zahl("epoche", e as i64)
                .zahl("hoehe", hoehe as i64)
                .zahl("proben", i64::from(m.proben))
                .zahl("abgewiesen", i64::from(m.abgewiesen)),
        );
        // ⚑ Fund 655: Ein Verdacht wird später nachgemessen und bezeugt noch
        //   nichts; erst die zweite Überschreitung ergibt ein Zeugnis.
        let gemessen = match urteil {
            crate::frist::Urteil::Bestanden => {
                self.gemessen.insert(schluessel);
                self.fristverdacht.remove(&schluessel);
                return;
            }
            crate::frist::Urteil::Verdacht => {
                self.fristverdacht.entry(schluessel).or_insert((hoehe, gemessen));
                return;
            }
            crate::frist::Urteil::Zeugnis(wert) => {
                self.gemessen.insert(schluessel);
                self.fristverdacht.remove(&schluessel);
                wert
            }
        };
        let zustand = self.kette.zustand();
        let epoche = zustand.epoch;
        let Some(zeugnis) = self.netzschluessel.as_ref().and_then(|k| k.fristzeugnis(epoche, m.pod, &m.mitglied, gemessen).ok()) else {
            return;
        };
        // An die übrigen Mitglieder des Pods, und in die eigene Sammlung.
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(zustand);
        let empfaenger: Vec<myl_types::latency_attest::PeerIdBytes> = zuteilung
            .pods
            .iter()
            .find(|p| p.pod_index == m.pod)
            .map(|p| p.mitglieder().filter(|x| x.miner_id != m.mitglied).map(|x| x.netzadresse).collect())
            .unwrap_or_default();
        let nachricht = Nachforderung::Fristzeugnis { epoche, pod: m.pod, gemeldeter: m.mitglied, zeugnis: zeugnis.clone() };
        if let Some(daten) = nachricht.als_bytes() {
            for adr in empfaenger.iter().filter(|a| **a != self.netzadresse) {
                if let Ok(an) = myl_net::peer_id_aus_bytes(adr) {
                    let _ = self.kommandos.send(NodeCommand::Anfrage { an, daten: daten.clone() });
                }
            }
        }
        self.fristzeugnis_aufnehmen(epoche, m.pod, m.mitglied, zeugnis);
    }

    /// Ein Fristzeugnis in die Sammlung; trägt die Meldung und ist dieser
    /// Knoten einer ihrer Zeugen, reicht er sie ein.
    fn fristzeugnis_aufnehmen(
        &mut self,
        epoche: myl_types::ids::EpochId,
        pod: u32,
        gemeldeter: myl_types::ids::MinerId,
        zeugnis: myl_types::fristmeldung::Fristzeugnis,
    ) {
        let Some(meldung) = self.fristsammlung.aufnehmen(self.kette.zustand(), epoche, pod, gemeldeter, zeugnis) else {
            return;
        };
        let ich = self.miner_ich();
        if !meldung.zeugnisse.iter().any(|z| Some(z.zeuge) == ich) {
            return;
        }
        self.protokoll.schreibe(
            Eintrag::neu("fristmeldung")
                .text("gemeldeter", hex::encode(&meldung.gemeldeter.as_bytes()[..8]))
                .zahl("zeugen", meldung.zeugnisse.len() as i64),
        );
        if let Some(tx) = self.netz_tx(myl_consensus::block::Anweisung::FristverstossEinreichen { meldung }) {
            self.ausstehend.push(tx);
        }
    }

    /// ⚑ **Dateien für die nächste Epoche holen** (P11): Ist der Bereich der
    /// nächsten Epoche bekannt und fehlen Dateien, holt der Knoten sie Stück
    /// für Stück von den Mitgliedern seines nächsten Pods.
    fn dateien_nachholen(&mut self, ich: &myl_types::ids::MinerId) {
        let Some(sk) = self.konfig.shard.clone() else {
            return;
        };
        if self.nachholen.as_ref().is_some_and(|n| n.laeuft) {
            return;
        }
        // ⚑ **Erst die laufende Epoche, dann die nächste**: Fehlt dem
        //   Bereich, den dieser Knoten jetzt rechnen soll, eine Datei,
        //   wartet sein Shard darauf.
        // 📌 Skalierung (2026-10-29): Hier standen zwei volle Kopien des
        //    Zustands je Durchlauf der Miner-Schleife; jetzt ein Blick unter
        //    der Epoche und Saat der nächsten ([`LedgerState::als_epoche`]).
        let jetzt = self.kette.zustand().epoch.0;
        let saat_naechste = self.kette.zustand().epochensaat_naechste;
        let saat_jetzt = self.kette.zustand().epochensaat;
        let mut ziel = None;
        for (epoche, saat) in [(jetzt, saat_jetzt), (jetzt + 1, saat_naechste)] {
            let a = self
                .kette
                .zustand_mut()
                .als_epoche(myl_types::ids::EpochId(epoche), saat, |z| crate::shardfuehrung::auftrag_der_epoche(z, ich));
            let Some(a) = a else {
                continue;
            };
            let Ok(fehlt) = crate::dateidienst::fehlende(&sk.artefakte, a.von, a.bis, a.mit_einbettung(), a.mit_kopf()) else {
                continue;
            };
            if !fehlt.is_empty() {
                ziel = Some((epoche, saat, fehlt));
                break;
            }
        }
        let Some((naechste, saat, fehlt)) = ziel else {
            return;
        };
        let zuteilung = self
            .kette
            .zustand_mut()
            .als_epoche(myl_types::ids::EpochId(naechste), saat, crate::kette::Kette::zuteilung_der_laufenden_epoche);
        let quellen: Vec<libp2p::PeerId> = zuteilung
            .pods
            .iter()
            .flat_map(|p| p.mitglieder())
            .filter(|m| m.miner_id != *ich)
            .filter_map(|m| myl_net::peer_id_aus_bytes(&m.netzadresse).ok())
            .collect();
        if quellen.is_empty() {
            return;
        }
        self.protokoll.schreibe(Eintrag::neu("dateien_holen").zahl("epoche", naechste as i64).zahl("fehlen", fehlt.len() as i64));
        self.nachholen = Some(Nachholen {
            holer: crate::dateidienst::Nachholer::neu(sk.artefakte, fehlt),
            quellen,
            quelle: 0,
            laeuft: false,
        });
        self.naechstes_stueck();
    }

    /// Fordert das nächste Stück bei der laufenden Quelle an.
    fn naechstes_stueck(&mut self) {
        let Some(n) = self.nachholen.as_mut() else {
            return;
        };
        let Some((name, ab, laenge)) = n.holer.naechste_anfrage() else {
            n.laeuft = false;
            return;
        };
        let Some(an) = n.quellen.get(n.quelle).copied() else {
            n.laeuft = false;
            return;
        };
        if let Some(daten) = (Nachforderung::Datei { name, ab, laenge }).als_bytes() {
            n.laeuft = true;
            let _ = self.kommandos.send(NodeCommand::Anfrage { an, daten });
        }
    }

    /// Ein Stück ist angekommen.
    fn stueck_erhalten(&mut self, von: libp2p::PeerId, name: &str, ab: u64, daten: &[u8], groesse: u64) {
        let Some(n) = self.nachholen.as_mut() else {
            return;
        };
        if n.quellen.get(n.quelle) != Some(&von) {
            return;
        }
        match n.holer.stueck_erhalten(name, ab, daten, groesse) {
            crate::dateidienst::Stueckergebnis::Fertig(d) => {
                self.protokoll.schreibe(Eintrag::neu("datei_geholt").text("datei", d));
            }
            crate::dateidienst::Stueckergebnis::Verworfen(d) => {
                // ⚑ Eine falsche Lieferung: diese Quelle übergehen.
                n.quelle += 1;
                self.protokoll.schreibe(Eintrag::neu("datei_verworfen").text("datei", d).text("von", von.to_string()));
            }
            _ => {}
        }
        self.naechstes_stueck();
    }

    /// Ein Gegenüber der Netzarbeit hatte nichts oder antwortete nicht;
    /// `wie` ist `nichts` oder `gescheitert`. In die Messung geht es nur,
    /// wenn es die Antwort auf eine Zeitprobe war (`zeitprobe`).
    fn nichts_erhalten(&mut self, von: libp2p::PeerId, wie: &str, zeitprobe: bool) {
        if zeitprobe {
            // Eine Probe, die erst nach ihrer Grenze scheitert, ist gemessen.
            if self.messung_ueberschritten(von) {
                return;
            }
            // ⚑ Fund 660: Vorher ist es eine Abweisung; die Messung bleibt.
            if self.messungen.get(&von).is_some_and(|m| m.wieder_ab.is_none()) {
                self.messung_abgewiesen(von, wie);
            }
        }
        self.nachholquelle_scheitert(von);
    }

    /// Hat `von` als Quelle des Nachholens nichts geliefert, kommt die
    /// nächste dran.
    fn nachholquelle_scheitert(&mut self, von: libp2p::PeerId) {
        if let Some(n) = self.nachholen.as_mut() {
            if n.laeuft && n.quellen.get(n.quelle) == Some(&von) {
                n.holer.quelle_hat_nicht();
                n.quelle += 1;
                self.naechstes_stueck();
            }
        }
    }

    /// 📌 Fund 674: Das Mitglied war nicht zu erreichen (Wählfehler). Keine
    /// Abweisung: später neu fragen, 4 bis 8 s, ohne zu zählen.
    fn messung_unerreicht(&mut self, von: libp2p::PeerId) {
        let saat = crate::frist::frische_saat();
        let zufall = u64::from_le_bytes([saat[0], saat[1], saat[2], saat[3], saat[4], saat[5], saat[6], saat[7]]);
        if let Some(m) = self.messungen.get_mut(&von) {
            if m.wieder_ab.is_none() {
                m.wieder_ab = Some(std::time::Instant::now() + Duration::from_millis(4_000 + zufall % 4_000));
            }
        }
    }
}

/// 📌 **Ein Wählfehler** (Fund 674): Die Anfrage kam gar nicht erst hinaus,
/// weil keine Verbindung zustande kam. Der Text ist der von libp2p
/// (`OutboundFailure::DialFailure`); eine Probe hält ihn fest.
fn ist_waehlfehler(grund: &str) -> bool {
    grund.starts_with("Failed to dial")
}

/// ⚑ **So viele Spurablagen hält ein Knoten** (Stufe 2, 2026-10-28): je
/// Bündel eine, über wenige Epochen; die Ziehung kommt eine Epoche nach dem
/// Bündel. Gedeckelt, weil jeder eine Ablage schicken kann.
pub const SPURABLAGEN_HOECHSTENS: usize = 16;

/// Was in den Orten fuer das Netz liegt, ueber alle zusammen.
#[derive(Debug, Clone, Copy, Default)]
struct Speicherstand {
    orte: u64,
    grenze_bytes: u64,
    belegung: myl_store::Belegung,
}

/// Inferenzauftraege aus dem Netz, nach Ausgang.
#[derive(Debug, Default)]
pub struct Inferenzzaehler {
    bearbeitet: std::sync::atomic::AtomicU64,
    abgelehnt: std::sync::atomic::AtomicU64,
}

impl Inferenzzaehler {
    /// Zaehlt eine Antwort: ein Ergebnis als bearbeitet, alles andere als
    /// abgelehnt.
    pub fn zaehlen(&self, antwort: &myl_types::inferenzauftrag::Inferenzantwort) {
        use std::sync::atomic::Ordering;
        match antwort {
            myl_types::inferenzauftrag::Inferenzantwort::Ergebnis { .. } => self.bearbeitet.fetch_add(1, Ordering::Relaxed),
            _ => self.abgelehnt.fetch_add(1, Ordering::Relaxed),
        };
    }

    /// Bearbeitet und abgelehnt.
    pub fn stand(&self) -> (u64, u64) {
        use std::sync::atomic::Ordering;
        (self.bearbeitet.load(Ordering::Relaxed), self.abgelehnt.load(Ordering::Relaxed))
    }
}

/// ⚑ **Der Abruf eines Zustandsabzugs** (2026-10-22): bei welcher Gegenstelle
/// gerade, seit wann, und wer schon falsch oder nichts geliefert hat.
struct AbzugHolen {
    z: crate::abzug::Zusammensetzen,
    bei: Option<(libp2p::PeerId, std::time::Instant)>,
    gesperrt: std::collections::HashSet<libp2p::PeerId>,
    reihum: usize,
    gemeldet: Option<std::time::Instant>,
}

/// Die Nachforderung des nächsten Stücks, nach der Art des Abzugs.
fn abzug_nachforderung(z: &crate::abzug::Zusammensetzen) -> Nachforderung {
    match z.art() {
        crate::abzug::Art::Voll => Nachforderung::Abzug { hoehe: z.anker().hoehe, ab: z.ab() },
        crate::abzug::Art::Leicht => Nachforderung::LeichterAbzug { hoehe: z.anker().hoehe, ab: z.ab() },
    }
}

impl AbzugHolen {
    fn neu(anker: crate::abzug::Anker) -> Self {
        Self::neu_als(anker, crate::abzug::Art::Voll)
    }

    fn neu_als(anker: crate::abzug::Anker, art: crate::abzug::Art) -> Self {
        Self {
            z: crate::abzug::Zusammensetzen::neu_als(anker, art),
            bei: None,
            gesperrt: Default::default(),
            reihum: 0,
            gemeldet: None,
        }
    }
}

/// ⚑ **Die Hintergrundprüfung**: eine zweite Kette ab Genesis, die die
/// Blöcke bis zur Basis nachrechnet und am Ende Hash und Wurzel der Basis
/// treffen muss. Trifft sie nicht, war der Anker falsch, und der Knoten hält
/// an, statt auf einem fremden Zustand weiterzumachen.
struct Hintergrund {
    kette: Kette,
    ziel: u64,
    blockhash: myl_types::hash::Hash,
    wurzel: myl_types::hash::Hash,
    /// ⚑ Mehrere Lieferungen zugleich, wie beim Nachholen der eigenen Kette
    ///   (2026-10-23); aus einer Quelle brauchte die Prüfung über eine
    ///   gestörte Leitung 144 s für 7 200 Blöcke.
    laufend: Vec<Nachforderung1>,
    voraus: crate::voraus::Vorausspeicher,
    gesperrt: std::collections::HashSet<libp2p::PeerId>,
    reihum: usize,
    gemeldet: Option<std::time::Instant>,
}

/// So lange wird eine Gegenstelle, die nichts hatte, ab dieser Höhe nicht
/// gefragt (Fund 626).
const LEER_SPERRE: Duration = Duration::from_secs(30);

/// So lange darf die eigene Höhe stehen, ohne dass eine Nachforderung läuft,
/// bevor der Knoten von sich aus fragt (Fund 618). Zehn Blockzeiten im
/// Betrieb; im Leerlauf eine Anfrage je zehn Sekunden an eine Gegenstelle.
const STILLE_NACHFRAGE: Duration = Duration::from_secs(10);

/// So viele Höhen darf die eigene Nummer im Zustand stillstehen, während
/// eigene Transaktionen ausstehen, bevor eine als verloren gilt (Fund 627).
/// Dieselbe Spanne wartet eine Transaktion mit späterer Nummer beim
/// Erzeuger auf ihre Vorgängerin.
const NETZ_LUECKE_HOEHEN: u64 = crate::kette::WARTEN_HOECHSTENS_HOEHEN;

/// Das Polster für Gebühren, das der Probehahn bereithält (G1): ein MYL,
/// am Boden rund dreitausend Überweisungen, aber nur rund vierzig
/// Archivbelege zu 27 KB. Nachgefüllt wird unter der Hälfte.
const PROBEHAHN_POLSTER: u64 = myl_tokenomics::UNITS_PER_MYL;

/// So weit über der eigenen Höhe zählt eine gehörte Spitze höchstens als
/// Bezug der Fristen eigener Transaktionen (Fund 628): Die Höhe kommt auch
/// aus abgelehnten Blöcken, also von jedem.
const NETZ_SPITZE_HOECHSTENS: u64 = 1024;

/// **Ab welcher Höhe eine Frist für eigene Transaktionen zählt** (Fund 628):
/// ab der Spitze, die dieser Knoten gehört hat, nicht ab der eigenen Höhe.
///
/// 📌 Im Archivlauf vom 2026-10-24 lagen Folger bei 25 ms je Block meist
/// 40 bis 220 Höhen hinter dem Erzeuger. Ihre Transaktion stand längst im
/// Block, sie sahen ihn nur noch nicht, und nach 64 eigenen Höhen meldeten
/// sie eine Lücke, die keine war: 25 Mal bei einem einzigen Splitter.
///
/// ⚠️ Gedeckelt auf [`NETZ_SPITZE_HOECHSTENS`] über der eigenen Höhe: Eine
/// erfundene Spitze verzögert die Heilung einer echten Lücke, verhindert sie
/// aber nicht.
fn frist_bezug(hoehe: u64, gehoert: u64) -> u64 {
    gehoert.clamp(hoehe, hoehe.saturating_add(NETZ_SPITZE_HOECHSTENS))
}

/// **Ob eine eigene Transaktion verloren ist** (Funde 627 und 628): `stand`
/// ist die Nummer im Zustand und der Bezug, ab dem die Frist zählt. Er wird
/// neu gesetzt, wenn nichts aussteht oder die Nummer im Zustand sich bewegt
/// hat, also bei der Vergabe; verloren ist eine, wenn die eigene Höhe
/// [`NETZ_LUECKE_HOEHEN`] über dem Bezug steht und die Nummer noch immer.
pub(crate) fn netz_luecke(stand: &mut (u64, u64), vergeben: u64, im_zustand: u64, hoehe: u64, gehoert: u64) -> bool {
    let luecke = vergeben > im_zustand && stand.0 == im_zustand && hoehe >= stand.1.saturating_add(NETZ_LUECKE_HOEHEN);
    if luecke || vergeben <= im_zustand || stand.0 != im_zustand {
        *stand = (im_zustand, frist_bezug(hoehe, gehoert));
    }
    luecke
}

/// So viele Lieferungen holt die Hintergrundprüfung höchstens zugleich: die
/// Hälfte von [`NACHHOLER`], damit das Nachholen der eigenen Kette, um das es
/// im Betrieb geht, Gegenstellen behält.
const HINTERGRUND_ZUGLEICH: usize = NACHHOLER / 2;

/// ⚑ **So viele Lieferungen holt das Nachladen der Geschichte zugleich**
/// (Fund 630), höchstens [`ARCHIV_JE_GEGENSTELLE`] bei derselben. Eine neue
/// Kopie hat eine Frist; bis NODE 0.59.0 holte sie mit
/// [`HINTERGRUND_ZUGLEICH`], zwei Lieferungen zu 512 Blöcken. Im Archivlauf
/// vom 2026-10-24 (50 bis 200 ms Laufzeit je Seite, 1 bis 5 % Verlust) kam
/// damit alle 3,6 s eine Lieferung je Gegenstelle, rund 226 Blöcke je
/// Sekunde: 509 s für 115 200 Blöcke, und die Kopie wurde ersetzt.
const ARCHIV_ZUGLEICH: usize = 8;

/// Lieferungen des Nachladens bei derselben Gegenstelle zugleich: so viele
/// wie die Hintergrundprüfung, damit die Herleitung der Ströme je
/// Verbindung in `myl-net` stimmt.
const ARCHIV_JE_GEGENSTELLE: usize = HINTERGRUND_ZUGLEICH;

impl Hintergrund {
    fn neu(
        netz: &'static crate::netz::Netz,
        satz: Option<myl_consensus::validator::VotingSet>,
        basis: &myl_consensus::block::Block,
    ) -> Self {
        let mut kette = Kette::im_netz(netz);
        // Die Prüfung im Hintergrund rechnet ab Genesis mit demselben Satz.
        if let Some(satz) = satz {
            kette.stimmsatz_setzen(satz);
        }
        Self {
            kette,
            ziel: basis.header.height,
            blockhash: basis.hash(),
            wurzel: basis.header.state_root,
            laufend: Vec::new(),
            voraus: Default::default(),
            gesperrt: Default::default(),
            reihum: 0,
            gemeldet: None,
        }
    }
}

/// ⚑ **Das Nachladen der Geschichte** ([`crate::archivabruf`]) samt den
/// laufenden Lieferungen. Gefragt werden zuerst die Kopien früherer
/// Perioden, über ihre registrierte Netzadresse; sie haben die Geschichte
/// sicher. Danach jeder: Der Beweis hängt an der eigenen Basis, nicht am
/// Liefernden.
struct Abruf {
    a: crate::archivabruf::Archivabruf,
    laufend: Vec<Nachforderung1>,
    gesperrt: std::collections::HashSet<libp2p::PeerId>,
    reihum: usize,
    gemeldet: Option<std::time::Instant>,
    vorgaenger: Vec<libp2p::PeerId>,
}

/// ⚑ **Das Holen der Teile eines Splitters** (Stufe A7): je Stelle eine
/// Lieferung, bei Kopien und Haltern zuerst.
struct Teilabruf {
    grenze: u64,
    teil: u64,
    fehlend: Vec<u64>,
    laufend: Vec<(libp2p::PeerId, u64, std::time::Instant)>,
    gesperrt: std::collections::HashSet<libp2p::PeerId>,
    reihum: usize,
    gemeldet: Option<std::time::Instant>,
    vorgaenger: Vec<libp2p::PeerId>,
}

/// Das Nachladen einer Kopie, der die Geschichte vor ihrer Basis fehlt;
/// `None`, wenn sie keine braucht oder die Basis nicht lesbar ist.
fn archiv_abruf(kette: &Kette) -> Option<Abruf> {
    if !kette.braucht_geschichte() {
        return None;
    }
    let (abzug, zustand) = kette.ablage()?.lesen(kette.basis()).ok()?;
    let a = crate::archivabruf::Archivabruf::neu(
        &kette.archivpfad()?,
        kette.netz().startwert_hash(),
        abzug.block,
        &zustand.historie,
    )
    .ok()?;
    let vorgaenger = kette
        .archivkopien()
        .iter()
        .filter_map(|adresse| myl_net::peer_id_aus_bytes(adresse).ok())
        .collect();
    Some(Abruf { a, laufend: Vec::new(), gesperrt: Default::default(), reihum: 0, gemeldet: None, vorgaenger })
}

/// Eine Gegenstelle reihum, die nicht gesperrt ist. Sind alle gesperrt, wird
/// nach einer Minute neu begonnen: Eine Gegenstelle, die eben nichts hatte,
/// kann inzwischen etwas haben. `None` heisst: jetzt nicht fragen.
fn naechste_gegenstelle(
    gegenstellen: &[libp2p::PeerId],
    gesperrt: &mut std::collections::HashSet<libp2p::PeerId>,
    reihum: &mut usize,
    gemeldet: &mut Option<std::time::Instant>,
) -> Option<libp2p::PeerId> {
    let frei: Vec<libp2p::PeerId> = gegenstellen.iter().filter(|p| !gesperrt.contains(p)).copied().collect();
    if frei.is_empty() {
        if !gegenstellen.is_empty() && gemeldet.is_none_or(|t| t.elapsed() > Duration::from_secs(60)) {
            *gemeldet = Some(std::time::Instant::now());
            gesperrt.clear();
        }
        return None;
    }
    *reihum = reihum.wrapping_add(1);
    Some(frei[*reihum % frei.len()])
}

/// Wie lange auf ein Stück oder eine Lieferung gewartet wird.
const ABZUG_FRIST: Duration = Duration::from_secs(10);

/// Wie viele Lieferungen der Hintergrundprüfung laufen.
fn h_laufend(h: &Option<Hintergrund>) -> usize {
    h.as_ref().map_or(0, |h| h.laufend.len())
}

impl Knoten {
    /// Ob dieser Knoten aus einem Grund von innen angehalten hat.
    pub fn angehalten(&self) -> Option<&'static str> {
        self.angehalten
    }

    /// Ob gerade ein Abzug geholt wird.
    pub fn holt_abzug(&self) -> bool {
        self.abzug_holen.is_some()
    }

    /// Ob die Hintergrundprüfung noch läuft.
    pub fn prueft_im_hintergrund(&self) -> bool {
        self.hintergrund.is_some()
    }

    /// Je Aufnahme: Abzug weiter holen, Hintergrund weiter prüfen.
    fn abzug_arbeit(&mut self) {
        if let Some(h) = self.abzug_holen.as_ref() {
            let anker = h.z.anker();
            if self.kette.hoehe() >= anker.hoehe {
                self.protokoll.schreibe(
                    Eintrag::neu("abzug_unnoetig").zahl("anker", anker.hoehe as i64).zahl("hoehe", self.kette.hoehe() as i64),
                );
                self.abzug_holen = None;
            }
        }
        if self.abzug_holen.as_ref().is_some_and(|h| h.bei.is_none_or(|(_, t)| t.elapsed() > ABZUG_FRIST)) {
            self.abzug_anfragen();
        }
        if let Some(h) = self.hintergrund.as_mut() {
            h.laufend.retain(|n| n.seit.elapsed() <= ABZUG_FRIST);
        }
        self.hintergrund_anfragen();
        if let Some(r) = self.abruf.as_mut() {
            r.laufend.retain(|n| n.seit.elapsed() <= ABZUG_FRIST);
        }
        self.abruf_anfragen();
    }

    fn abruf_bei(&self, von: &libp2p::PeerId) -> bool {
        self.abruf_anzahl_bei(von) > 0
    }

    fn abruf_anzahl_bei(&self, von: &libp2p::PeerId) -> usize {
        self.abruf.as_ref().map_or(0, |r| r.laufend.iter().filter(|n| n.an == *von).count())
    }

    /// Füllt die Lieferungen des Nachladens bis [`ARCHIV_ZUGLEICH`] auf,
    /// je Gegenstelle bis [`ARCHIV_JE_GEGENSTELLE`], die Kopien früherer
    /// Perioden zuerst.
    fn abruf_anfragen(&mut self) {
        let mut versuche = 0usize;
        while self.abruf.as_ref().is_some_and(|r| r.laufend.len() < ARCHIV_ZUGLEICH)
            && versuche <= self.gegenstellen.len() * ARCHIV_JE_GEGENSTELLE
        {
            versuche += 1;
            let frei: Vec<libp2p::PeerId> = self
                .gegenstellen
                .iter()
                .filter(|p| {
                    !self.nachforderungen.iter().any(|n| n.an == **p)
                        && !self.hintergrund_bei(p)
                        && !self.abzug_bei(p)
                        && self.abruf_anzahl_bei(p) < ARCHIV_JE_GEGENSTELLE
                })
                .copied()
                .collect();
            let Some(r) = self.abruf.as_mut() else {
                return;
            };
            let laufend: Vec<(u64, u64)> = r.laufend.iter().map(|n| (n.ab, n.bis)).collect();
            let Some((ab, bis)) =
                crate::nachschub::naechster_bereich(r.a.geschrieben(), &laufend, |x| r.a.wartet(x), r.a.ziel())
            else {
                return;
            };
            let vorgaenger = r.vorgaenger.iter().find(|p| frei.contains(p) && !r.gesperrt.contains(p)).copied();
            let Some(peer) =
                vorgaenger.or_else(|| naechste_gegenstelle(&frei, &mut r.gesperrt, &mut r.reihum, &mut r.gemeldet))
            else {
                return;
            };
            let anzahl = r.a.anzahl();
            if !self.anfrage_senden(peer, &Nachforderung::Archiv { ab, bis, anzahl }) {
                return;
            }
            if let Some(r) = self.abruf.as_mut() {
                r.laufend.push(Nachforderung1 { an: peer, ab, bis, seit: std::time::Instant::now() });
            }
        }
    }

    fn abruf_nichts(&mut self, von: libp2p::PeerId) {
        if let Some(r) = self.abruf.as_mut() {
            r.laufend.retain(|n| n.an != von);
            r.gesperrt.insert(von);
        }
        self.abruf_anfragen();
    }

    /// **Eine Lieferung der Geschichte**: nur von einer Gegenstelle, die
    /// gefragt war, und ab der erfragten Höhe; geprüft gegen die eigene
    /// Basis. Wer Müll liefert, wird gesperrt.
    fn abruf_lieferung(
        &mut self,
        von: libp2p::PeerId,
        bloecke: Vec<myl_consensus::block::Block>,
        beweis: myl_types::historie::Beweis,
    ) {
        let Some(r) = self.abruf.as_mut() else {
            return;
        };
        // Bei derselben Gegenstelle laufen bis zu zwei: die mit der Höhe,
        // ab der geliefert wurde, sonst irgendeine, und die wird abgewiesen.
        let ab = bloecke.first().map(|b| b.header.height);
        let Some(i) = r
            .laufend
            .iter()
            .position(|n| n.an == von && Some(n.ab) == ab)
            .or_else(|| r.laufend.iter().position(|n| n.an == von))
        else {
            return;
        };
        let erfragt = r.laufend.remove(i);
        let ergebnis = if ab != Some(erfragt.ab) {
            Err("nicht ab der erfragten Höhe".to_string())
        } else {
            r.a.lieferung(bloecke, &beweis).map_err(|e| e.to_string())
        };
        if let Err(grund) = ergebnis {
            r.gesperrt.insert(von);
            self.protokoll.schreibe(Eintrag::neu("archiv_abgelehnt").text("von", von.to_string()).text("grund", grund));
            return self.abruf_anfragen();
        }
        if !r.a.fertig() {
            return self.abruf_anfragen();
        }
        let Some(r) = self.abruf.take() else {
            return;
        };
        let ziel = r.a.anzahl();
        let eintrag = match r.a.abschliessen().and_then(|pfad| self.kette.geschichte_einfuegen(&pfad)) {
            Ok(n) => Eintrag::neu("archiv_vollstaendig").zahl("bloecke", n as i64).zahl("bis", ziel as i64),
            Err(e) => Eintrag::neu("archiv_fehler").text("grund", e),
        };
        self.protokoll.schreibe(eintrag);
    }

    fn abzug_bei(&self, von: &libp2p::PeerId) -> bool {
        self.abzug_holen.as_ref().is_some_and(|h| h.bei.is_some_and(|(p, _)| p == *von))
    }

    fn hintergrund_bei(&self, von: &libp2p::PeerId) -> bool {
        self.hintergrund.as_ref().is_some_and(|h| h.laufend.iter().any(|n| n.an == *von))
    }

    /// Eine Lieferung gehört der Hintergrundprüfung, wenn sie unter deren
    /// Ziel beginnt; das gewöhnliche Nachholen fragt nie unter der Basis.
    fn fuer_den_hintergrund(&self, bloecke: &[myl_consensus::block::Block]) -> bool {
        self.hintergrund.as_ref().is_some_and(|h| bloecke.first().is_some_and(|b| b.header.height <= h.ziel))
    }

    fn anfrage_senden(&mut self, an: libp2p::PeerId, n: &Nachforderung) -> bool {
        n.als_bytes().is_some_and(|daten| self.kommandos.send(NodeCommand::Anfrage { an, daten }).is_ok())
    }

    fn abzug_anfragen(&mut self) {
        let Some(h) = self.abzug_holen.as_mut() else {
            return;
        };
        h.bei = None;
        let Some(peer) = naechste_gegenstelle(&self.gegenstellen, &mut h.gesperrt, &mut h.reihum, &mut h.gemeldet) else {
            return;
        };
        let n = abzug_nachforderung(&h.z);
        let ab = h.z.ab();
        if self.anfrage_senden(peer, &n) {
            if let Some(h) = self.abzug_holen.as_mut() {
                h.bei = Some((peer, std::time::Instant::now()));
            }
            if ab == 0 {
                self.protokoll.schreibe(Eintrag::neu("abzug_angefragt").text("bei", peer.to_string()));
            }
        }
    }

    /// Liefert ein Stück eines eigenen Abzugs, aus der Ablage: nur dieses
    /// Stück wird gelesen (Fund 609, [`crate::abzug::Ablage::stueck_lesen`]).
    fn abzug_stueck(&mut self, hoehe: u64, ab: u64) -> Option<Nachlieferung> {
        let (block, daten, groesse) = self.kette.ablage()?.stueck_lesen(hoehe, ab).ok()??;
        Some(Nachlieferung::Abzug { block: Box::new(block), ab, daten, groesse })
    }

    fn abzug_stueck_erhalten(&mut self, von: libp2p::PeerId, block: myl_consensus::block::Block, ab: u64, daten: &[u8], groesse: u64) {
        if !self.abzug_bei(&von) {
            return;
        }
        let Some(h) = self.abzug_holen.as_mut() else {
            return;
        };
        match h.z.stueck(block, ab, daten, groesse) {
            Ok(crate::abzug::Weiter::Ab(_)) => self.abzug_anfragen_bei(von),
            Ok(crate::abzug::Weiter::Fertig(f)) => {
                self.abzug_holen = None;
                let (abzug, zustand) = *f;
                self.abzug_uebernehmen(abzug, zustand, von);
            }
            Err(e) => {
                let anker = h.z.anker();
                h.gesperrt.insert(von);
                h.z = crate::abzug::Zusammensetzen::neu(anker);
                self.protokoll.schreibe(
                    Eintrag::neu("abzug_abgelehnt").text("von", von.to_string()).text("grund", e.to_string()),
                );
                self.abzug_anfragen();
            }
        }
    }

    /// Das nächste Stück bei derselben Gegenstelle: Sie hat geliefert.
    fn abzug_anfragen_bei(&mut self, peer: libp2p::PeerId) {
        let Some(h) = self.abzug_holen.as_ref() else {
            return;
        };
        let n = abzug_nachforderung(&h.z);
        let gesendet = self.anfrage_senden(peer, &n);
        if let Some(h) = self.abzug_holen.as_mut() {
            h.bei = gesendet.then(|| (peer, std::time::Instant::now()));
        }
    }

    fn abzug_nichts(&mut self, von: libp2p::PeerId) {
        // Hat die Gegenstelle den Abzug nicht, ist der Anker vielleicht zu
        // alt: Gewöhnliche Grenzen behält jeder nur zwei Epochen lang.
        if let Some(h) = self.abzug_holen.as_mut() {
            h.gesperrt.insert(von);
            let hoehe = h.z.anker().hoehe;
            self.protokoll.schreibe(Eintrag::neu("abzug_nichts").text("von", von.to_string()).zahl("hoehe", hoehe as i64));
        }
        self.abzug_anfragen();
    }

    /// Setzt die Kette auf den geprüften Abzug, legt ihn als Basis ab und
    /// beginnt die Hintergrundprüfung.
    fn abzug_uebernehmen(&mut self, abzug: crate::abzug::Abzug, zustand: myl_ledger::state::LedgerState, von: libp2p::PeerId) {
        let hoehe = abzug.hoehe();
        let bytes = abzug.zustand.len();
        if let Err(g) = self.kette.auf_abzug_setzen(&abzug, zustand) {
            self.protokoll.schreibe(Eintrag::neu("abzug_nicht_gesetzt").text("grund", g));
            return;
        }
        // ⚑ Leicht: Die eigenen Konten kennt der Knoten erst wieder über
        //   einen Zeugen oder einen Kontobeweis; keine Ablage, keine
        //   Hintergrundprüfung (sie bräuchte den ganzen Zustand).
        if self.kette.ist_zustandslos() {
            self.kette.eigene_vergessen();
            self.wartet_auf_zeugen.clear();
            self.zeugen_eilig.clear();
            self.fertige_zeugen.clear();
            self.zeugensammler = myl_consensus::zeuge::Zeugensammler::neu(ZEUGEN_SAMMLER_BYTES);
            self.kontobeweise_offen = None;
            self.protokoll.schreibe(
                Eintrag::neu("abzug_uebernommen")
                    .zahl("hoehe", hoehe as i64)
                    .zahl("bytes", bytes as i64)
                    .wahr("leicht", true)
                    .text("von", von.to_string())
                    .text("zustandswurzel", kurz(&self.kette.wurzel())),
            );
            self.voraus_einspielen();
            self.weiter_nachfordern(None);
            return;
        }
        if let Some(a) = self.kette.ablage().cloned() {
            if a.ablegen(&abzug).and_then(|()| a.basis_setzen(hoehe)).is_err() {
                self.protokoll.schreibe(Eintrag::neu("abzug_nicht_abgelegt").zahl("hoehe", hoehe as i64));
            }
        }
        self.protokoll.schreibe(
            Eintrag::neu("abzug_uebernommen")
                .zahl("hoehe", hoehe as i64)
                .zahl("bytes", bytes as i64)
                .text("von", von.to_string())
                .text("zustandswurzel", kurz(&self.kette.wurzel())),
        );
        if self.konfig.hintergrundpruefung {
            self.hintergrund = Some(Hintergrund::neu(self.konfig.netz, self.kette.stimmsatz().cloned(), &abzug.block));
            self.hintergrund_anfragen();
        }
        self.voraus_einspielen();
        self.weiter_nachfordern(None);
    }

    /// Füllt die Lieferungen der Prüfung bis [`HINTERGRUND_ZUGLEICH`] auf, mit
    /// derselben Planung wie das Nachholen ([`crate::nachschub::naechster_bereich`]).
    fn hintergrund_anfragen(&mut self) {
        if self.hintergrund.is_none() {
            return;
        }
        let mut versuche = 0usize;
        while h_laufend(&self.hintergrund) < HINTERGRUND_ZUGLEICH && versuche <= self.gegenstellen.len() {
            versuche += 1;
            let Some(h) = self.hintergrund.as_mut() else {
                return;
            };
            let laufend: Vec<(u64, u64)> = h.laufend.iter().map(|n| (n.ab, n.bis)).collect();
            let Some((ab, bis)) =
                crate::nachschub::naechster_bereich(h.kette.hoehe(), &laufend, |x| h.voraus.hat_hoehe(x), h.ziel)
            else {
                return;
            };
            // Nicht bei einer Gegenstelle, die gerade Blöcke für die eigene
            // Kette oder schon für die Prüfung liefert: Die Antworten tragen
            // keine Marke, die sie unterschiede.
            let frei: Vec<libp2p::PeerId> = self
                .gegenstellen
                .iter()
                .filter(|p| !self.nachforderungen.iter().any(|n| n.an == **p) && !h.laufend.iter().any(|n| n.an == **p))
                .copied()
                .collect();
            let Some(peer) = naechste_gegenstelle(&frei, &mut h.gesperrt, &mut h.reihum, &mut h.gemeldet) else {
                return;
            };
            if !self.anfrage_senden(peer, &Nachforderung::Bloecke { ab, bis }) {
                return;
            }
            let n = self.hintergrund.as_mut().map_or(0, |h| {
                h.laufend.push(Nachforderung1 { an: peer, ab, bis, seit: std::time::Instant::now() });
                h.laufend.len()
            });
            self.protokoll.schreibe(
                Eintrag::neu("hintergrund_angefordert")
                    .text("bei", peer.to_string())
                    .zahl("ab", ab as i64)
                    .zahl("bis", bis as i64)
                    .zahl("laufend", n as i64),
            );
        }
    }

    fn hintergrund_nichts(&mut self, von: libp2p::PeerId) {
        if let Some(h) = self.hintergrund.as_mut() {
            h.laufend.retain(|n| n.an != von);
            h.gesperrt.insert(von);
        }
        self.hintergrund_anfragen();
    }

    fn hintergrund_bloecke(&mut self, von: libp2p::PeerId, bloecke: Vec<myl_consensus::block::Block>) {
        let Some(h) = self.hintergrund.as_mut() else {
            return;
        };
        h.laufend.retain(|n| n.an != von);
        let mut abgelehnt = None;
        for b in bloecke.into_iter().filter(|b| b.header.height <= h.ziel) {
            match h.kette.uebernimm(&b) {
                Ok(()) => {}
                // Ein späterer Bereich vor einem früheren: aufheben.
                Err(crate::kette::KettenFehler::PasstNichtAn { .. }) if b.header.height > h.kette.hoehe() + 1 => {
                    if let Ok(roh) = borsh::to_vec(&b) {
                        let _ = h.voraus.ablegen(h.kette.hoehe(), b, &roh);
                    }
                }
                Err(e) => {
                    // Eine Lieferung, die nicht an die eigene Prüfkette passt,
                    // hört bei dieser Gegenstelle auf; geurteilt wird am Ziel.
                    abgelehnt = Some(e.to_string());
                    break;
                }
            }
        }
        // Was aufgehoben ist und jetzt anschliesst, einspielen.
        loop {
            let kandidaten = h.voraus.naechste(h.kette.hoehe());
            let mut weiter = false;
            for b in kandidaten {
                if !weiter && h.kette.uebernimm(&b).is_ok() {
                    weiter = true;
                }
            }
            if !weiter {
                break;
            }
        }
        if let Some(grund) = abgelehnt {
            h.gesperrt.insert(von);
            self.protokoll.schreibe(Eintrag::neu("hintergrund_abgelehnt").text("von", von.to_string()).text("grund", grund));
        }
        let Some(h) = self.hintergrund.as_ref() else {
            return;
        };
        if h.kette.hoehe() < h.ziel {
            return self.hintergrund_anfragen();
        }
        let (ziel, gleich) = (h.ziel, h.kette.letzter_hash() == h.blockhash && h.kette.zustandswurzel() == h.wurzel);
        let gefunden = kurz(&h.kette.zustandswurzel());
        let erwartet = kurz(&h.wurzel);
        self.hintergrund = None;
        if gleich {
            if let Some(a) = self.kette.ablage() {
                let _ = a.bestaetigt_setzen(ziel);
            }
            self.protokoll.schreibe(Eintrag::neu("abzug_bestaetigt").zahl("hoehe", ziel as i64));
        } else {
            // ⛔️ Die Blöcke von Genesis führen nicht zur Basis: Der Anker war
            //   falsch. Weitermachen hiesse, auf einem fremden Zustand zu
            //   rechnen und ihn anderen zu liefern.
            self.protokoll.schreibe(
                Eintrag::neu("abzug_widerlegt")
                    .zahl("hoehe", ziel as i64)
                    .text("erwartet", erwartet)
                    .text("gefunden", gefunden),
            );
            self.angehalten = Some("Zustandsabzug widerlegt: die Blöcke von Genesis führen nicht zur Basis");
        }
    }

    /// Meldet einen neu angelegten eigenen Abzug, mit vollem Blockhash: Aus
    /// dieser Zeile entsteht ein Anker.
    fn eigenen_abzug_melden(&mut self) {
        if let Some((basis, vorher, nachher)) = self.kette.verdichtung_nehmen() {
            self.protokoll.schreibe(
                Eintrag::neu("kette_verdichtet")
                    .zahl("basis", basis as i64)
                    .zahl("bytes_vorher", vorher as i64)
                    .zahl("bytes_nachher", nachher as i64),
            );
        }
        let Some((hoehe, hash)) = self.kette.letzter_abzug() else {
            return;
        };
        if self.abzug_gemeldet == Some(hoehe) {
            return;
        }
        self.abzug_gemeldet = Some(hoehe);
        // ⚑ **Der Stand der Archivpflicht je Epoche** (Stufe A9): Rolle,
        //   Ziehung, Belege, Halter, Ersetzte, Teilwurzel. Daraus liest der
        //   Netzlauf von aussen, was sich an der Pflicht bewegt.
        {
            let a = &self.kette.zustand().archiv;
            self.protokoll.schreibe(
                Eintrag::neu("archiv_stand")
                    .zahl("hoehe", hoehe as i64)
                    .text("rolle", format!("{:?}", self.kette.archivrolle()))
                    .zahl("basis", self.kette.basis() as i64)
                    .wahr("gezogen", a.gezogen)
                    .zahl("grenze", a.grenze as i64)
                    .zahl("kopien", a.kopien.len() as i64)
                    .zahl("vollstaendig", a.vollstaendig.len() as i64)
                    .zahl("splitter", a.splitter.len() as i64)
                    .zahl("halter", a.halter.len() as i64)
                    .wahr("halter_alle", a.halter_alle)
                    .zahl("ersetzt", a.ersetzt.len() as i64)
                    .wahr("teilwurzel", a.teilwurzel.is_some())
                    .zahl("teilwurzeln", a.teilwurzeln.len() as i64)
                    .zahl("miner", self.kette.zustand().miner.len() as i64),
            );
        }
        // ⚑ **Kalte Ausfuhr** (Stufe A8): die fertigen Perioden an jeder
        //   Langzeitgrenze, wenn die Kette ab Genesis lebt.
        if crate::abzug::ist_langzeit(hoehe) {
            if let Some(ordner) = self.konfig.archiv_ausfuhr.clone() {
                if self.kette.basis() == 0 {
                    let startwert = self.kette.netz().startwert_hash();
                    let kette = &mut self.kette;
                    let eintrag = match crate::archivausfuhr::ausfuhr(&ordner, &startwert, hoehe, true, |ab, bis| {
                        kette.bloecke_von_bis(ab, bis)
                    }) {
                        Ok(n) => Eintrag::neu("archiv_ausfuhr").zahl("hoehe", hoehe as i64).zahl("neu", n as i64),
                        Err(e) => Eintrag::neu("archiv_ausfuhr_fehler").text("grund", e),
                    };
                    self.protokoll.schreibe(eintrag);
                }
            }
        }
        // ⚑ **Nachladen, sobald nötig, an jeder Epochengrenze**: Eine
        //   Kopie, die mitten in der Periode nachrückt, wartete sonst bis zur
        //   nächsten Langzeitgrenze und wäre bis dahin wieder ersetzt.
        if self.abruf.is_none() {
            self.abruf = archiv_abruf(&self.kette);
            if let Some(r) = self.abruf.as_ref() {
                self.protokoll.schreibe(
                    Eintrag::neu("archiv_nachladen")
                        .zahl("bis", self.kette.basis() as i64)
                        .zahl("vorgaenger", r.vorgaenger.len() as i64),
                );
                self.abruf_anfragen();
            }
        }
        self.protokoll.schreibe(
            Eintrag::neu("abzug_angelegt")
                .zahl("hoehe", hoehe as i64)
                .text("anker", format!("{hoehe}:{}", hash.to_hex()))
                .wahr("dauerhaft", crate::abzug::ist_langzeit(hoehe)),
        );
    }
}

/// Die Nonce der nächsten Transaktion des Testverkehrs (Fund 593).
///
/// `stand` ist (nächste Nonce, Höhe beim Senden der ältesten noch nicht
/// verbuchten). Liegt der Zustand vorn oder ist seit 32 Blöcken nichts
/// verbucht worden, beginnt es wieder bei der Nonce aus dem Zustand.
pub(crate) fn testverkehr_nonce(stand: &mut (u64, u64), im_zustand: u64, hoehe: u64) -> u64 {
    let (naechste, seit) = *stand;
    let unterwegs = naechste > im_zustand && hoehe < seit.saturating_add(32);
    let nonce = if unterwegs { naechste } else { im_zustand };
    *stand = (nonce + 1, if unterwegs { seit } else { hoehe });
    nonce
}

#[cfg(test)]
mod reihum_tests {
    use super::reihum;

    /// ⚑ Reihum war nach `ceil(len / n)` Runden jede Gegenstelle dabei, je
    /// Runde höchstens `n`; mit weniger Gegenstellen als `n` alle, ohne
    /// doppelte.
    #[test]
    fn jede_gegenstelle_kommt_dran() {
        let p: Vec<libp2p::PeerId> = (0..10).map(|_| libp2p::PeerId::random()).collect();
        let mut gesehen = std::collections::BTreeSet::new();
        let mut ab = 0;
        for _ in 0..3 {
            let (auswahl, weiter) = reihum(&p, ab, 4);
            assert_eq!(auswahl.len(), 4);
            gesehen.extend(auswahl);
            ab = weiter;
        }
        assert_eq!(gesehen.len(), 10, "nach drei Runden alle zehn");
        let (wenige, _) = reihum(&p[..2], 7, 4);
        assert_eq!(wenige.len(), 2);
        assert_ne!(wenige[0], wenige[1]);
        assert_eq!(reihum(&[], 3, 4), (Vec::new(), 0));
    }
}

#[cfg(test)]
mod testverkehr_tests {
    use super::testverkehr_nonce;

    /// Ab der Nonce im Zustand, fortgezählt, solange etwas unterwegs ist.
    #[test]
    fn die_nonce_kommt_aus_dem_zustand() {
        let mut s = (0, 0);
        assert_eq!(testverkehr_nonce(&mut s, 0, 10), 0, "die erste ist die des Zustands");
        assert_eq!(testverkehr_nonce(&mut s, 0, 11), 1, "die erste ist unterwegs");
        assert_eq!(testverkehr_nonce(&mut s, 2, 12), 2, "beide verbucht");
        // Ein Neustart: der Stand ist leer, der Zustand nicht.
        let mut neu = (0, 0);
        assert_eq!(testverkehr_nonce(&mut neu, 7, 100), 7);
    }

    /// Kommt 32 Blöcke lang nichts an, ging es verloren: wieder ab dem
    /// Zustand, und ab dann zählt die Frist neu.
    #[test]
    fn verlorenes_wird_neu_gesendet() {
        let mut s = (0, 0);
        assert_eq!(testverkehr_nonce(&mut s, 0, 10), 0);
        assert_eq!(testverkehr_nonce(&mut s, 0, 41), 1, "noch in der Frist");
        assert_eq!(testverkehr_nonce(&mut s, 0, 42), 0, "nach 32 Blöcken neu");
        assert_eq!(testverkehr_nonce(&mut s, 0, 43), 1);
    }
}

#[cfg(test)]
mod wiederanlauf_tests {
    use super::nachspielen;
    use crate::abzug::Ablage;
    use crate::kette::{probekonto, probeschluessel, Kette};
    use crate::speicher::Kettenspeicher;
    use myl_consensus::block::{Anweisung, Transaktion};

    fn ordner(marke: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("myl-wiederanlauf-{marke}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("Ordner");
        p
    }

    /// Eine Kette mit Datei und Ablage über `n` Blöcke, mit einem Burn je
    /// 50 Blöcke, damit der Zustand sich bewegt; `versatz` macht die Beträge
    /// und damit die Kette verschieden.
    fn gefuellt(pfad: &std::path::Path, n: u64, versatz: u64) -> Kette {
        let (speicher, _) = Kettenspeicher::oeffnen(pfad, Kette::startwert()).expect("Speicher");
        let mut k = Kette::probestand();
        k.speicher_setzen(speicher);
        k.ablage_setzen(Ablage::neben(pfad, Kette::startwert()));
        for i in 0..n {
            if i % 50 == 0 {
                let konto = (i % 8) as u8;
                let nonce = k.zustand().account(&probekonto(konto)).nonce;
                let tx = Transaktion::signiere(&Kette::startwert(), &probeschluessel(konto), nonce, Anweisung::Burn { betrag: 1_000 + i + versatz })
                    .expect("signieren");
                k.aufnehmen(tx);
            }
            let _ = k.baue_block();
        }
        k
    }

    /// Nachspielen wie beim Start, mit dem eigenen Abzug `einsatz` (oder
    /// keinem).
    fn neu_geladen(pfad: &std::path::Path, einsatz: Option<u64>, ablage_von: &std::path::Path) -> (Kette, super::Nachgespielt) {
        let (mut speicher, _) = Kettenspeicher::oeffnen(pfad, Kette::startwert()).expect("Speicher");
        let mut k = Kette::probestand();
        k.ablage_setzen(Ablage::neben(pfad, Kette::startwert()));
        let einsatz = einsatz.map(|h| Ablage::neben(ablage_von, Kette::startwert()).lesen(h).expect("Abzug"));
        let n = nachspielen(&mut k, &mut speicher, einsatz);
        (k, n)
    }

    /// ⚑ **Der Wiederanlauf setzt beim jüngsten eigenen Abzug an** (Fund
    /// 629): bis 3600 nur vorgespult, die hundert Blöcke danach übernommen,
    /// und Höhe, letzter Hash und Zustandswurzel sind die des vollständigen
    /// Nachrechnens. Die Blöcke nach dem Abzug gehen durch `uebernimm`; ein
    /// falsch eingesetzter Zustand fiele dort an der ersten Wurzel auf.
    #[test]
    fn der_wiederanlauf_setzt_beim_juengsten_abzug_an() {
        let o = ordner("an");
        let pfad = o.join("kette.dat");
        let vorher = gefuellt(&pfad, 3700, 0);
        assert_eq!(Ablage::neben(&pfad, Kette::startwert()).hoehen(), vec![1800, 3600]);
        let (k, n) = neu_geladen(&pfad, Some(3600), &pfad);
        assert_eq!(n.verworfen, None);
        assert_eq!((n.abzug, n.vorgespult, n.uebernommen, n.abgelehnt), (Some(3600), 3600, 100, 0));
        assert_eq!((k.hoehe(), k.letzter_hash(), k.zustandswurzel()), (vorher.hoehe(), vorher.letzter_hash(), vorher.zustandswurzel()));
        let (voll, m) = neu_geladen(&pfad, None, &pfad);
        assert_eq!((m.abzug, m.uebernommen), (None, 3700));
        assert_eq!(voll.zustandswurzel(), k.zustandswurzel());
        let _ = std::fs::remove_dir_all(&o);
    }

    /// Ein Abzug, dessen Block nicht in der Datei steht, wird verworfen,
    /// ebenso einer hinter ihrem Ende; der Aufrufer rechnet dann ab der
    /// Basis.
    #[test]
    fn ein_fremder_abzug_wird_verworfen() {
        let o = ordner("fremd");
        let (eigen, fremd, lang) = (o.join("eigen.dat"), o.join("fremd.dat"), o.join("lang.dat"));
        gefuellt(&eigen, 2000, 0);
        gefuellt(&fremd, 2000, 7);
        gefuellt(&lang, 3700, 0);
        let (_, n) = neu_geladen(&eigen, Some(1800), &fremd);
        assert!(n.verworfen.as_deref().is_some_and(|g| g.contains("1800")), "{:?}", n.verworfen);
        let (_, n) = neu_geladen(&eigen, Some(3600), &lang);
        assert!(n.verworfen.as_deref().is_some_and(|g| g.contains("endet vor")), "{:?}", n.verworfen);
        let _ = std::fs::remove_dir_all(&o);
    }
}

#[cfg(test)]
mod netz_luecke_tests {
    use super::{netz_luecke, NETZ_LUECKE_HOEHEN, NETZ_SPITZE_HOECHSTENS};

    /// ⚑ **Wer hinter der Spitze liegt, meldet keine Lücke, solange er den
    /// Block noch nicht gesehen haben kann** (Fund 628). Vergeben bei eigener
    /// Höhe 1000, die Spitze bei 1150: Die Transaktion kommt dort in den
    /// Block, und bis zur eigenen Höhe 1150 + 64 ist nichts verloren.
    #[test]
    fn wer_zurueckliegt_meldet_keine_falsche_luecke() {
        let mut s = (0, 0);
        assert!(!netz_luecke(&mut s, 5, 5, 1000, 1150), "nichts steht aus");
        // Vergeben: 5 ist unterwegs, der Zustand steht bei 5.
        assert!(!netz_luecke(&mut s, 6, 5, 1000 + NETZ_LUECKE_HOEHEN, 1220));
        assert!(!netz_luecke(&mut s, 6, 5, 1150 + NETZ_LUECKE_HOEHEN - 1, 1250), "noch in der Frist ab der Spitze");
        // Der Block mit der 5 ist angekommen: kein Stillstand mehr.
        assert!(!netz_luecke(&mut s, 6, 6, 1214, 1260));
    }

    /// Steht die Nummer 64 Höhen über dem Bezug still, ist die Transaktion
    /// verloren, und die Frist beginnt von vorn.
    #[test]
    fn eine_echte_luecke_wird_gemeldet() {
        let mut s = (0, 0);
        assert!(!netz_luecke(&mut s, 5, 5, 1000, 1000));
        assert!(!netz_luecke(&mut s, 6, 5, 1000 + NETZ_LUECKE_HOEHEN - 1, 1000));
        assert!(netz_luecke(&mut s, 6, 5, 1000 + NETZ_LUECKE_HOEHEN, 1000));
        assert!(!netz_luecke(&mut s, 6, 5, 1000 + NETZ_LUECKE_HOEHEN + 1, 1000), "danach zählt die Frist neu");
    }

    /// ⚠️ Eine erfundene Spitze verzögert die Heilung, verhindert sie aber
    /// nicht: Sie zählt höchstens [`NETZ_SPITZE_HOECHSTENS`] über der eigenen
    /// Höhe.
    #[test]
    fn eine_erfundene_spitze_haelt_die_heilung_nicht_auf() {
        let mut s = (0, 0);
        assert!(!netz_luecke(&mut s, 5, 5, 1000, u64::MAX));
        let faellig = 1000 + NETZ_SPITZE_HOECHSTENS + NETZ_LUECKE_HOEHEN;
        assert!(!netz_luecke(&mut s, 6, 5, faellig - 1, u64::MAX));
        assert!(netz_luecke(&mut s, 6, 5, faellig, u64::MAX));
    }
}

#[cfg(test)]
mod plaetze_tests {
    use super::Plaetze;

    /// ⚑ **Ein Platz geht beim Freigeben zurück, auch nach einer Panik**
    /// (Fund 609): sonst wäre die Zeitprobe nach dem ersten Absturz eines
    /// Fadens für immer zu.
    #[test]
    fn ein_platz_geht_beim_freigeben_zurueck_auch_nach_einer_panik() {
        let p = Plaetze::neu(1);
        let erster = p.nehmen().expect("frei");
        assert!(p.nehmen().is_none(), "zwei zugleich auf einem Platz");
        drop(erster);
        let zweiter = p.nehmen().expect("wieder frei");
        let ergebnis = std::thread::spawn(move || {
            let _halten = zweiter;
            panic!("Absturz mit belegtem Platz");
        })
        .join();
        assert!(ergebnis.is_err());
        assert!(p.nehmen().is_some(), "nach der Panik blieb der Platz belegt");
    }

    #[test]
    fn mehr_als_die_grenze_bekommt_keinen_platz() {
        let p = Plaetze::neu(4);
        let gehalten: Vec<_> = (0..4).map(|_| p.nehmen().expect("frei")).collect();
        assert!(p.nehmen().is_none());
        drop(gehalten);
        assert!(p.nehmen().is_some());
    }
}

/// Ein nachgerechneter Verlauf, aus der Rechenaufgabe zurück (Stufe 2).
#[derive(Debug)]
pub struct Nachrechnung {
    /// Die Ziehung.
    gezogen: myl_types::Spuranfrage,
    /// Wie viele Positionen nachgerechnet wurden.
    positionen: usize,
    /// Das Ergebnis samt Logits, oder warum es keines gibt.
    ergebnis: Result<myl_verifier::Verlaufspruefung, String>,
    /// Wie lange es dauerte, samt Laden des Modells.
    dauer_ms: i64,
    /// Bei einer Abweichung der Schiedsbeleg: Verlauf und Eingänge.
    beleg: Option<Schiedsbeleg>,
}

/// Ein geprüfter Stapel von der Relaisleitung: die gültigen, wie viele
/// gefragt waren, und wohin die Antwort geht.
type RelaisGeprueft = (Vec<myl_consensus::block::GepruefteTransaktion>, usize, std::sync::mpsc::Sender<myl_types::relais::Relaisantwort>);

/// So viele Transaktionen je Sekunde stehen einzeln im Protokoll; was
/// darüber kommt, nur in der Sammelzeile `tx_zaehler`.
const TX_ZEILEN_JE_SEKUNDE: u64 = 100;

/// Die Zähler der Transaktionen einer Sekunde.
#[derive(Debug)]
struct TxZaehler {
    seit: std::time::Instant,
    empfangen: u64,
    aufgenommen: u64,
    abgewiesen: u64,
}

impl Default for TxZaehler {
    fn default() -> Self {
        Self { seit: std::time::Instant::now(), empfangen: 0, aufgenommen: 0, abgewiesen: 0 }
    }
}

/// Ein Schiedsbeleg: der nachgerechnete Verlauf und die Eingänge der
/// strittigen Layer bis zur strittigen Position (T6d).
type Schiedsbeleg = (Vec<myl_types::Spurantwort>, Vec<Vec<i16>>);

/// Was ein Validator im Komitee nachgerechnet hat (T6d).
pub struct Schiedsrechnung {
    fall: (u64, myl_types::ids::PodId, u32),
    pod_falsch: Result<bool, String>,
}

/// ⚑ **Die Abweisung einer Zeitprobe im Knoten** (Fund 660, Fund 670): die
/// Verdrahtung zwischen `nichts_erhalten`, `messung_abgewiesen` und
/// `messung_auswerten`, gegen einen gestarteten Knoten mit einem Pod.
#[cfg(test)]
mod abweisung_tests {
    use super::*;
    use crate::kette::{probeanmeldung, probezusage, Kette};
    use myl_types::ids::{EpochId, MinerId};

    /// Der Ordner einer Probe, unter dem Zwischenordner.
    fn ordner(marke: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("myl-abweisung-{marke}-{}", std::process::id()))
    }

    /// Der Ordner einer bestandenen Probe bleibt nicht liegen.
    fn aufraeumen(marke: &str) {
        let _ = std::fs::remove_dir_all(ordner(marke));
    }

    /// Ein Knoten ohne Nachbarn, dessen Kette einen Pod aus sechs
    /// Probekonten in Epoche 3 trägt.
    async fn knoten_mit_pod(marke: &str) -> Knoten {
        let d = ordner(marke);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("Ordner");
        let konfig = KnotenKonfig {
            name: marke.to_string(),
            schluesseldatei: d.join("knoten.key"),
            protokollverzeichnis: d.clone(),
            horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
            kettendatei: None,
            beobachtung: None,
            tuer: None,
            ..KnotenKonfig::default()
        };
        let mut k = Knoten::starten(konfig, false).await.expect("Knoten");
        let mut kette = Kette::probestand();
        for w in 0..6u8 {
            kette.aufnehmen(probeanmeldung(w));
            kette.aufnehmen(probezusage(w, EpochId(1), 1));
        }
        kette.baue_block();
        while kette.zustand().epoch.0 < 3 {
            kette.baue_block();
        }
        k.kette = kette;
        k
    }

    /// Eine laufende Messung an das zweite Mitglied des Pods, mit
    /// `in_folge` Abweisungen seit `seit`.
    fn messung(k: &Knoten, epoche: u64, in_folge: u32, seit: Duration) -> (MinerId, Messung) {
        let z = k.kette.zustand();
        let zuteilung = Kette::zuteilung_der_laufenden_epoche(z);
        let pod = &zuteilung.pods[0];
        let mitglied = pod.shards[1].miner.miner_id;
        let frist = crate::frist::frist_des_mitglieds(z, pod.pod_index, &mitglied).expect("Frist");
        let jetzt = std::time::Instant::now();
        let m = Messung {
            pod: pod.pod_index,
            mitglied,
            ab: jetzt,
            positionen: crate::frist::probe_positionen(frist),
            frist,
            probe_us: None,
            proben: 0,
            saat: [0u8; 32],
            epoche,
            messende: crate::frist::messende(z, &mitglied).len(),
            abgewiesen_in_folge: in_folge,
            abgewiesen_seit: Some(jetzt.checked_sub(seit).unwrap_or(jetzt)),
            abgewiesen: in_folge,
            wieder_ab: None,
        };
        (mitglied, m)
    }

    /// Eine Abweisung lässt die Messung stehen und fragt mit Abstand neu;
    /// erst eine lange Folge endet als Überschreitung, im ersten Fenster
    /// als Verdacht.
    #[tokio::test]
    async fn eine_lange_verweigerung_wird_im_knoten_zum_verdacht() {
        let mut k = knoten_mit_pod("verweigerung").await;
        let e = k.kette.zustand().epoch.0;
        let von = libp2p::PeerId::random();
        // ⛔️ Gegenprobe 1: drei in Folge über eine Stunde, die vierte
        //    kommt erst jetzt. Vier sind nötig, die Dauer genügt: Verdacht.
        let (mitglied, m) = messung(&k, e, 3, Duration::from_secs(3_600));
        k.messungen.insert(von, m);
        k.nichts_erhalten(von, "nichts", true);
        assert!(!k.messungen.contains_key(&von), "die Messung endet");
        assert!(k.fristverdacht.contains_key(&(e, mitglied)), "als Verdacht");
        // ⛔️ Gegenprobe 2: die vierte in Folge, aber erst seit einer
        //    Sekunde: weiter fragen, kein Verdacht.
        k.fristverdacht.clear();
        let (mitglied, m) = messung(&k, e, 3, Duration::from_secs(1));
        k.messungen.insert(von, m);
        k.nichts_erhalten(von, "nichts", true);
        let m = k.messungen.get(&von).expect("die Messung bleibt");
        assert_eq!(m.abgewiesen_in_folge, 4);
        assert!(m.wieder_ab.is_some(), "mit Abstand neu fragen");
        assert!(!k.fristverdacht.contains_key(&(e, mitglied)));
        // ⛔️ Gegenprobe 3: eine Stunde, aber erst die dritte in Folge.
        let (mitglied, m) = messung(&k, e, 2, Duration::from_secs(3_600));
        k.messungen.insert(von, m);
        k.nichts_erhalten(von, "gescheitert", true);
        assert!(k.messungen.get(&von).is_some_and(|m| m.wieder_ab.is_some()));
        assert!(!k.fristverdacht.contains_key(&(e, mitglied)));
        // Ein `Nichts`, während die Messung wartet, zählt nicht.
        k.nichts_erhalten(von, "nichts", true);
        assert_eq!(k.messungen.get(&von).map(|m| m.abgewiesen_in_folge), Some(3));        aufraeumen("verweigerung");
    }

    /// ⛔️ **Nur eine Antwort auf die Zeitprobe zählt** (2026-10-30): Ein
    /// `Nichts` oder ein Fehlschlag ohne den Zweck der Probe, etwa auf eine
    /// Blocknachforderung an dasselbe Mitglied, berührt die Messung nicht;
    /// mit dem Zweck ist es eine Abweisung.
    #[tokio::test]
    async fn nur_die_antwort_auf_die_probe_zaehlt() {
        let mut k = knoten_mit_pod("zweck").await;
        let e = k.kette.zustand().epoch.0;
        let von = libp2p::PeerId::random();
        let (_, m) = messung(&k, e, 0, Duration::from_secs(0));
        k.messungen.insert(von, m);
        let nichts = Nachlieferung::Nichts.als_bytes().expect("Nichts");
        k.ereignis_einspeisen(NodeEvent::AntwortEingegangen { von, daten: nichts.clone(), zweck: None });
        k.ereignis_einspeisen(NodeEvent::AnfrageGescheitert { an: von, grund: "Timeout".into(), zweck: None });
        assert_eq!(k.messungen.get(&von).map(|m| (m.abgewiesen, m.wieder_ab.is_some())), Some((0, false)), "ohne Zweck nichts");
        k.ereignis_einspeisen(NodeEvent::AntwortEingegangen { von, daten: nichts, zweck: Some(ZWECK_ZEITPROBE) });
        assert_eq!(k.messungen.get(&von).map(|m| m.abgewiesen), Some(1), "die Probe wurde abgewiesen");
        aufraeumen("zweck");
    }

    /// 📌 **Fund 674:** Ein Wählfehler ist keine Abweisung: Die Messung
    /// bleibt, zählt nichts und fragt später neu; auch nach einer langen
    /// Folge solcher Fehler kein Verdacht. Der Text von libp2p ist
    /// festgehalten, damit ein Update ihn nicht still ändert.
    #[tokio::test]
    async fn nicht_erreicht_ist_keine_weigerung() {
        let mut k = knoten_mit_pod("unerreicht").await;
        let e = k.kette.zustand().epoch.0;
        let von = libp2p::PeerId::random();
        assert!(ist_waehlfehler(&libp2p::request_response::OutboundFailure::DialFailure.to_string()));
        // ⛔️ Gegenproben: andere Fehler sind keine Wählfehler.
        assert!(!ist_waehlfehler(&libp2p::request_response::OutboundFailure::Timeout.to_string()));
        assert!(!ist_waehlfehler(&libp2p::request_response::OutboundFailure::ConnectionClosed.to_string()));
        let (mitglied, m) = messung(&k, e, 3, Duration::from_secs(3_600));
        k.messungen.insert(von, m);
        for _ in 0..10 {
            if let Some(m) = k.messungen.get_mut(&von) {
                m.wieder_ab = None;
            }
            k.messung_unerreicht(von);
        }
        let m = k.messungen.get(&von).expect("die Messung bleibt");
        assert_eq!(m.abgewiesen_in_folge, 3, "nicht gezählt");
        assert!(m.wieder_ab.is_some(), "später neu fragen");
        assert!(!k.fristverdacht.contains_key(&(e, mitglied)), "kein Verdacht");
        // ⛔️ Gegenprobe: dieselbe Lage mit einer Abweisung endet im Verdacht.
        if let Some(m) = k.messungen.get_mut(&von) {
            m.wieder_ab = None;
        }
        k.nichts_erhalten(von, "nichts", true);
        assert!(k.fristverdacht.contains_key(&(e, mitglied)));
        aufraeumen("unerreicht");
    }

    /// 📌 **Fund 670:** Eine Messung aus der vorigen Epoche wird nicht mehr
    /// bewertet; dieselbe aus der laufenden ergibt den Verdacht.
    #[tokio::test]
    async fn eine_messung_aus_der_vorigen_epoche_verfaellt() {
        let mut k = knoten_mit_pod("verfall").await;
        let e = k.kette.zustand().epoch.0;
        let (mitglied, mut m) = messung(&k, e - 1, 0, Duration::ZERO);
        m.probe_us = Some(u64::MAX / 2);
        k.messung_auswerten(m.clone(), "beantwortet");
        assert!(!k.fristverdacht.contains_key(&(e, mitglied)), "verfallen");
        // ⛔️ Gegenprobe: in der laufenden Epoche ein Verdacht.
        m.epoche = e;
        k.messung_auswerten(m, "beantwortet");
        assert!(k.fristverdacht.contains_key(&(e, mitglied)));        aufraeumen("verfall");
    }
}
