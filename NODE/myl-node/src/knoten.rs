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
    /// Die Zeugnisse über Fristen (6.8).
    fristsammlung: crate::frist::Fristsammlung,
    /// Laufende Messungen je Gegenüber.
    messungen: std::collections::BTreeMap<libp2p::PeerId, Messung>,
    /// Wer in welcher Epoche schon gemessen ist: Wer noch fehlt, wird bei
    /// der nächsten Aufnahme wieder gefragt (ein Shard, der gerade lädt,
    /// antwortet erst später).
    gemessen: std::collections::BTreeSet<(u64, myl_types::ids::MinerId)>,
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

/// Eine laufende Nachforderung: bei wem, welcher Bereich, seit wann.
#[derive(Debug, Clone, Copy)]
struct Nachforderung1 {
    an: libp2p::PeerId,
    ab: u64,
    bis: u64,
    seit: std::time::Instant,
}

/// Eine laufende Fristmessung: erst die Pings, dann die Zeitprobe.
#[derive(Debug, Clone)]
struct Messung {
    pod: u32,
    mitglied: myl_types::ids::MinerId,
    ab: std::time::Instant,
    /// Der kürzeste Ping bisher.
    ping_us: Option<u64>,
    pings: u32,
    /// Die kürzeste Zeitprobe bisher (Fund 639).
    probe_us: Option<u64>,
    proben: u32,
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
        let abzug_holen = if kette.hoehe() == 0 {
            konfig.abzug.or_else(|| konfig.netz.juengster_anker()).map(AbzugHolen::neu)
        } else {
            None
        };
        let hintergrund = match (&basisblock, kette.ablage()) {
            (Some(b), Some(a))
                if konfig.hintergrundpruefung
                    && !a.bestaetigt()
                    && !konfig.netz.ist_anker(b.header.height, &b.hash()) =>
            {
                Some(Hintergrund::neu(konfig.netz, b))
            }
            _ => None,
        };
        let netzschluessel = match (&konfig.modellzusage, &konfig.shard, &konfig.konsensschluesseldatei) {
            (None, None, _) => None,
            (_, _, Some(pfad)) => Some(
                crate::schluessel::Konsensschluessel::aus_datei(pfad)
                    .map_err(|e| KnotenFehler::Ortsleitung(format!("{}: {e:?}", pfad.display())))?,
            ),
            (_, _, None) => {
                return Err(KnotenFehler::Ortsleitung(
                    "--modellzusage und --shard-programm brauchen --konsensschluessel".into(),
                ))
            }
        };
        // ⚑ **Archivrolle** ([`crate::archiv`], 2026-10-23): aus der eigenen
        //   Kennung und den beiden jüngsten Langzeitabzügen. Eine gezogene
        //   Kopie, der die Geschichte fehlt, holt sie mit Beweisen
        //   ([`crate::archivabruf`]).
        kette.archivkennung_setzen(netzschluessel.as_ref().map(|k| k.kennung()));
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
        protokoll.schreibe(
            Eintrag::neu("netz")
                .text("netz", konfig.netz.name)
                .text("artefakt", konfig.netz.artefakt)
                .wahr("zusage", konfig.modellzusage.is_some())
                .wahr("shard", shardfuehrung.is_some()),
        );

        Ok(Self {
            netzschluessel,
            netzadresse,
            netz_nonce: 0,
            netz_stand: (0, 0),
            netz_gesendet: std::collections::BTreeMap::new(),
            shardfuehrung,
            fristsammlung: crate::frist::Fristsammlung::default(),
            messungen: std::collections::BTreeMap::new(),
            gemessen: std::collections::BTreeSet::new(),
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
            protokoll,
            testverkehr_zaehler: 0,
            kette,
            nachforderungen: Vec::new(),
            leer_geantwortet: std::collections::HashMap::new(),
            zuletzt_gewachsen: (0, std::time::Instant::now()),
            stille_bei: None,
            fristsperre_gemeldet: None,
            beobachtungsstelle: crate::beobachtung::Beobachtungsstelle::neu(),
            kontraktabschrift: crate::tuer::Kontraktabschrift::neu(),
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
        let im_zustand = self.kette.zustand().account(&crate::kette::konto_fuer(&self.konfig.name)).nonce;
        let nonce = testverkehr_nonce(&mut self.testverkehr_nonce, im_zustand, self.kette.hoehe());
        let anweisung = Anweisung::Burn {
            betrag: 1_000 + self.testverkehr_zaehler * 100,
        };
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
        self.abzug_arbeit();
        self.eigenen_abzug_melden();
        self.netzarbeit().await;
        let z = self.zustand().await;
        let mut eintrag = Eintrag::neu("aufnahme")
            .zahl("peers", z.peers as i64)
            .zahl("hoehe", self.kette.hoehe() as i64)
            .text("zustandswurzel", kurz(&self.kette.zustandswurzel()))
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
        let guthaben = self.kette.zustand().account(&self.eigenes_konto()).balance;
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
        let ziel = self.hoechste_gehoerte;
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
        if self.abzug_holen.is_some() {
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
                crate::nachschub::naechster_bereich(self.kette.hoehe(), &laufend, |h| self.voraus.hat_hoehe(h), ziel)
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
                match self.kette.uebernimm(&block) {
                    Ok(()) => {
                        self.protokoll.schreibe(
                            Eintrag::neu("block_uebernommen")
                                .zahl("hoehe", self.kette.hoehe() as i64)
                                .zahl("txs", block.txs.len() as i64)
                                .text("zustandswurzel", kurz(&self.kette.zustandswurzel()))
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
                let anfrage = Nachforderung::aus_bytes(&daten);
                // ⚑ **Teure Anfragen gehen durch die Drossel** (Fund 632):
                //   je Gegenstelle und für alle zusammen; darüber gibt es
                //   `Nichts`, und das kostet nichts.
                let art = match &anfrage {
                    Some(Nachforderung::Bloecke { .. }) => Some(crate::drossel::Art::Bloecke),
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
                    Some(Nachforderung::Zeitprobe { positionen }) => {
                        let messend = self
                            .miner_ich()
                            .is_some_and(|ich| crate::frist::misst_mich(self.kette.zustand(), &ich, &von));
                        let platz = if messend { self.zeitprobe_platz.nehmen() } else { None };
                        match self.shardfuehrung.as_mut().and_then(|f| f.tuer()).zip(platz) {
                            Some(((adresse, ausweis), platz)) => {
                                let sender = self.kommandos.clone();
                                tokio::task::spawn_blocking(move || {
                                    let antwort = match myl_types::zeitprobe::fragen(
                                        adresse,
                                        &ausweis,
                                        positionen.min(crate::frist::PROBE_POSITIONEN),
                                        Duration::from_secs(120),
                                    ) {
                                        Ok(abdruck) => Nachlieferung::Zeitprobe(abdruck),
                                        Err(_) => Nachlieferung::Nichts,
                                    };
                                    drop(platz);
                                    if let Some(bytes) = antwort.als_bytes() {
                                        let _ = sender.send(NodeCommand::Antwort { marke, daten: bytes });
                                    }
                                });
                                None
                            }
                            None => Some((Nachlieferung::Nichts, 0)),
                        }
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
                // ⚑ **Antworten der Netzarbeit** (Stufe 4) berühren die
                //   Blocknachforderung nicht, wie die Inferenzantwort.
                match Nachlieferung::aus_bytes(&daten) {
                    // ⚑ **Zustandsabzug und Hintergrundprüfung** (2026-10-22)
                    //   zuerst: Ihre Antworten berühren das gewöhnliche
                    //   Nachholen nicht.
                    Some(Nachlieferung::Abzug { block, ab, daten, groesse }) => {
                        return self.abzug_stueck_erhalten(von, block, ab, &daten, groesse)
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
                    Some(Nachlieferung::Pong) => return self.pong_erhalten(von),
                    Some(Nachlieferung::Zeitprobe(_)) => return self.zeitprobe_erhalten(von),
                    Some(Nachlieferung::Datei { name, ab, daten, groesse }) => {
                        return self.stueck_erhalten(von, &name, ab, &daten, groesse)
                    }
                    Some(Nachlieferung::Vermerkt) => return,
                    Some(Nachlieferung::Nichts) if self.ist_netzarbeit(&von) => return self.nichts_erhalten(von),
                    _ => {}
                }
                let vorher = self.kette.hoehe();
                let mut angenommen = 0usize;
                let mut abgelehnt = 0usize;
                let mut vorausgelegt = 0usize;
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
                if angenommen > 0 || vorausgelegt > 0 {
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
                let aus_dem_voraus = self.voraus_einspielen();
                self.weiter_nachfordern((angenommen > 0).then_some(von));
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
                    .zahl("hoehe_vorher", vorher as i64)
                    .zahl("hoehe_nachher", self.kette.hoehe() as i64)
            }
            NodeEvent::AnfrageGescheitert { an, grund } if self.ist_netzarbeit(&an) => {
                self.nichts_erhalten(an);
                Eintrag::neu("netzarbeit_gescheitert").text("an", an.to_string()).text("grund", grund)
            }
            NodeEvent::AnfrageGescheitert { an, grund } => {
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
        self.zusage_pflegen(&ich);
        self.archiv_belegen(&ich);
        self.teilwurzel_nennen(&ich);
        self.teil_belegen(&ich);
        self.teile_holen(&ich);
        self.shard_abgleichen(&ich);
        self.fristen_messen(&ich);
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
        let im_zustand = self.kette.zustand().account(&k.adresse()).nonce;
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
            .is_some_and(|k| self.netz_nonce > self.kette.zustand().account(&k.adresse()).nonce)
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
        let im_zustand = self.kette.zustand().account(&k.adresse()).nonce;
        let (hoehe, gehoert) = (self.kette.hoehe(), self.hoechste_gehoerte);
        if netz_luecke(&mut self.netz_stand, self.netz_nonce, im_zustand, hoehe, gehoert) {
            self.protokoll.schreibe(
                Eintrag::neu("netz_luecke").zahl("vergeben", self.netz_nonce as i64).zahl("im_zustand", im_zustand as i64),
            );
            self.netz_nonce = im_zustand;
        }
    }

    fn tx_verbreiten(&mut self, tx: myl_consensus::block::Transaktion) {
        let Ok(daten) = borsh::to_vec(&tx) else {
            return;
        };
        // ⚑ Der Erzeuger hört sein eigenes Gossip nicht; er nimmt die
        //   Transaktion selbst auf.
        if self.konfig.erzeugt_bloecke {
            self.kette.aufnehmen(tx);
        }
        let _ = self.kommandos.send(NodeCommand::Publish { topic: GossipTopic::Transactions, data: daten, result: None });
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

    /// ⚑ **Anmeldung und Zusage** (Festlegung des Projektinhabers: kein Pod
    /// ohne Zusage, der Knoten reicht sie selbst ein). Angemeldet wird, wer
    /// es noch nicht ist; eingereicht, wenn die neueste Zusage im Zustand
    /// nicht die konfigurierte ist, für die nächste Epoche.
    fn zusage_pflegen(&mut self, ich: &myl_types::ids::MinerId) {
        let Some(soll) = self.konfig.modellzusage else {
            return;
        };
        let soll = myl_types::zusage::Modellzusage { abdruck: self.konfig.netz.podmodell.abdruck, ..soll };
        if !self.kette.zustand().miner.contains_key(ich) {
            if self.schon_gesendet("anmeldung") {
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

    /// ⚑ **Der Shard aus der Kette** (P7).
    fn shard_abgleichen(&mut self, ich: &myl_types::ids::MinerId) {
        let mut soll = crate::shardfuehrung::auftrag_der_epoche(self.kette.zustand(), ich);
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
    /// erst die Pings, dann die Zeitprobe ([`Self::pong_erhalten`]).
    ///
    /// ⚑ **Jedes Mitglied des Pods misst, auch die Reserve**: Sie rechnet
    /// nicht und kann gerade deshalb bezeugen. Wer noch nicht gemessen ist,
    /// wird bei jeder Aufnahme wieder gefragt, solange keine Messung an ihn
    /// läuft; ein Shard, der beim ersten Versuch noch lädt, entgeht der
    /// Frist so nicht.
    fn fristen_messen(&mut self, ich: &myl_types::ids::MinerId) {
        let epoche = self.kette.zustand().epoch.0;
        self.gemessen.retain(|(e, _)| *e == epoche);
        let laufend: Vec<myl_types::ids::MinerId> = self.messungen.values().map(|m| m.mitglied).collect();
        // Eine Messung, die nach einer halben Minute nicht zurück ist, gilt
        // als verloren und darf neu gestellt werden.
        self.messungen.retain(|_, m| m.ab.elapsed() < Duration::from_secs(30));
        for (pod, m) in crate::frist::zu_messen(self.kette.zustand(), ich) {
            if self.gemessen.contains(&(epoche, m.miner_id)) || laufend.contains(&m.miner_id) {
                continue;
            }
            let Ok(an) = myl_net::peer_id_aus_bytes(&m.netzadresse) else {
                continue;
            };
            let Some(daten) = Nachforderung::Ping.als_bytes() else {
                continue;
            };
            self.messungen.insert(
                an,
                Messung { pod, mitglied: m.miner_id, ab: std::time::Instant::now(), ping_us: None, pings: 0, probe_us: None, proben: 0 },
            );
            let _ = self.kommandos.send(NodeCommand::Anfrage { an, daten });
        }
    }

    /// Gehört ein Gegenüber zu einer laufenden Messung oder ist es die
    /// laufende Quelle des Nachholens?
    fn ist_netzarbeit(&self, peer: &libp2p::PeerId) -> bool {
        self.messungen.contains_key(peer)
            || self.nachholen.as_ref().is_some_and(|n| n.laeuft && n.quellen.get(n.quelle) == Some(peer))
    }

    /// Ein Ping ist zurück: den nächsten stellen oder, nach dem letzten,
    /// die Zeitprobe (Fund 590: der kürzeste zählt).
    fn pong_erhalten(&mut self, von: libp2p::PeerId) {
        let Some(m) = self.messungen.get_mut(&von) else {
            return;
        };
        if m.pings >= crate::frist::PINGS_JE_MESSUNG {
            return;
        }
        let dauer = m.ab.elapsed().as_micros() as u64;
        m.ping_us = Some(m.ping_us.map_or(dauer, |p| p.min(dauer)));
        m.pings += 1;
        m.ab = std::time::Instant::now();
        let naechste = if m.pings < crate::frist::PINGS_JE_MESSUNG {
            Nachforderung::Ping
        } else {
            Nachforderung::Zeitprobe { positionen: crate::frist::PROBE_POSITIONEN }
        };
        if let Some(daten) = naechste.als_bytes() {
            let _ = self.kommandos.send(NodeCommand::Anfrage { an: von, daten });
        }
    }

    /// Eine Zeitprobe ist zurück: die nächste stellen oder, nach der letzten,
    /// die kürzeste gegen die Frist halten und, wenn darüber, ein Zeugnis
    /// unterschreiben und verteilen (Fund 639: die kürzeste zählt).
    fn zeitprobe_erhalten(&mut self, von: libp2p::PeerId) {
        let Some(m) = self.messungen.get_mut(&von) else {
            return;
        };
        if m.pings < crate::frist::PINGS_JE_MESSUNG {
            return;
        }
        let dauer = m.ab.elapsed().as_micros() as u64;
        m.probe_us = Some(m.probe_us.map_or(dauer, |p| p.min(dauer)));
        m.proben += 1;
        if m.proben < crate::frist::PROBEN_JE_MESSUNG {
            m.ab = std::time::Instant::now();
            if let Some(daten) = (Nachforderung::Zeitprobe { positionen: crate::frist::PROBE_POSITIONEN }).als_bytes() {
                let _ = self.kommandos.send(NodeCommand::Anfrage { an: von, daten });
            }
            return;
        }
        let Some(m) = self.messungen.remove(&von) else {
            return;
        };
        let (Some(ping_us), Some(probe_us)) = (m.ping_us, m.probe_us) else {
            return;
        };
        // Ohne Rechenzeit keine Messung (Fund 590): nicht vermerkt, also bei
        // der nächsten Aufnahme wieder gefragt.
        let Some(gemessen) = crate::frist::gemessen_je_token(probe_us, ping_us, crate::frist::PROBE_POSITIONEN) else {
            self.protokoll.schreibe(
                Eintrag::neu("fristmessung_verworfen")
                    .text("mitglied", hex::encode(&m.mitglied.as_bytes()[..8]))
                    .zahl("probe_us", probe_us.min(i64::MAX as u64) as i64)
                    .zahl("ping_us", ping_us.min(i64::MAX as u64) as i64),
            );
            return;
        };
        self.gemessen.insert((self.kette.zustand().epoch.0, m.mitglied));
        let zustand = self.kette.zustand();
        let Some(frist) = crate::frist::frist_des_mitglieds(zustand, m.pod, &m.mitglied) else {
            return;
        };
        self.protokoll.schreibe(
            Eintrag::neu("fristmessung")
                .text("mitglied", hex::encode(&m.mitglied.as_bytes()[..8]))
                .zahl("gemessen_us", gemessen.min(i64::MAX as u64) as i64)
                .zahl("frist_us", frist.min(i64::MAX as u64) as i64),
        );
        if gemessen <= frist {
            return;
        }
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
        let jetzt = self.kette.zustand().epoch.0;
        let mut naechster = self.kette.zustand().clone();
        naechster.epoch = myl_types::ids::EpochId(jetzt + 1);
        naechster.epochensaat = naechster.epochensaat_naechste;
        let mut ziel = None;
        for (epoche, z) in [(jetzt, self.kette.zustand().clone()), (jetzt + 1, naechster)] {
            let Some(a) = crate::shardfuehrung::auftrag_der_epoche(&z, ich) else {
                continue;
            };
            let Ok(fehlt) = crate::dateidienst::fehlende(&sk.artefakte, a.von, a.bis, a.mit_einbettung(), a.mit_kopf()) else {
                continue;
            };
            if !fehlt.is_empty() {
                ziel = Some((epoche, z, fehlt));
                break;
            }
        }
        let Some((naechste, z, fehlt)) = ziel else {
            return;
        };
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(&z);
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

    /// Ein Gegenüber der Netzarbeit hatte nichts oder antwortete nicht.
    fn nichts_erhalten(&mut self, von: libp2p::PeerId) {
        self.messungen.remove(&von);
        if let Some(n) = self.nachholen.as_mut() {
            if n.laeuft && n.quellen.get(n.quelle) == Some(&von) {
                n.holer.quelle_hat_nicht();
                n.quelle += 1;
                self.naechstes_stueck();
            }
        }
    }
}

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

impl AbzugHolen {
    fn neu(anker: crate::abzug::Anker) -> Self {
        Self {
            z: crate::abzug::Zusammensetzen::neu(anker),
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
    fn neu(netz: &'static crate::netz::Netz, basis: &myl_consensus::block::Block) -> Self {
        Self {
            kette: Kette::im_netz(netz),
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
        let n = Nachforderung::Abzug { hoehe: h.z.anker().hoehe, ab: h.z.ab() };
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
        Some(Nachlieferung::Abzug { block, ab, daten, groesse })
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
        let n = Nachforderung::Abzug { hoehe: h.z.anker().hoehe, ab: h.z.ab() };
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
                .text("zustandswurzel", kurz(&self.kette.zustandswurzel())),
        );
        if self.konfig.hintergrundpruefung {
            self.hintergrund = Some(Hintergrund::neu(self.konfig.netz, &abzug.block));
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
