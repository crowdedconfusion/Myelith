//! Konfiguration eines Knotens: was von außen entschieden wird.
//!
//! Eine Konfiguration ist eine Behauptung über die Umgebung („ich bin
//! unter dieser Adresse erreichbar"), und falsche Behauptungen dieser
//! Art äußern sich im Betrieb als Stille. Deshalb prüft [`KnotenKonfig`]
//! sich selbst, bevor der Knoten startet, und meldet Widersprüche als
//! Fehler statt sie zu überleben.
//!
//! Fund 56 ist der Grund für diese Haltung: Ein Relais ohne eigene
//! öffentliche Adresse nahm Reservierungen an und antwortete ohne Ziel.
//! Alles lief, nur niemand kam an.

use myl_types::ids::Address;
use std::path::PathBuf;

use myl_net::{NatKonfig, NetConfig};

/// Die Rolle, in der ein Knoten startet.
///
/// Die Rolle entscheidet **nicht** über Rechte im Protokoll, das tut
/// Stake. Sie entscheidet, was der Prozess an Diensten anbietet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rolle {
    /// Nimmt am Netz teil, bietet keine Dienste an. Der Normalfall, auch
    /// für Knoten hinter NAT.
    Teilnehmer,
    /// Zusätzlich Relais und AutoNAT-Server. Setzt eine öffentlich
    /// erreichbare Adresse voraus (Fund 56).
    Relais,
}

impl Rolle {
    /// Aus dem Kommandozeilenwort.
    pub fn aus_text(t: &str) -> Option<Rolle> {
        match t {
            "teilnehmer" => Some(Rolle::Teilnehmer),
            "relais" => Some(Rolle::Relais),
            _ => None,
        }
    }

    /// Für das Betriebsprotokoll.
    pub fn als_text(&self) -> &'static str {
        match self {
            Rolle::Teilnehmer => "teilnehmer",
            Rolle::Relais => "relais",
        }
    }
}

/// Die Konfiguration eines Knotens.
#[derive(Debug, Clone)]
pub struct KnotenKonfig {
    /// Name des Knotens im Protokoll. Nicht kryptografisch, nur zum
    /// Wiedererkennen beim Zusammenlegen mehrerer Protokolle: „welche
    /// Maschine war das".
    pub name: String,
    /// Datei mit dem Schlüsselmaterial. Bleibt sie bestehen, behält der
    /// Knoten seine PeerId über Neustarts, und **genau das braucht ein
    /// Testlauf**: Sonst ist nach jedem Neustart ein anderer Knoten da,
    /// und die Protokolle lassen sich nicht zusammenführen.
    pub schluesseldatei: PathBuf,
    /// Verzeichnis für das Betriebsprotokoll.
    pub protokollverzeichnis: PathBuf,
    /// Höchstens so viele MiB Betriebsprotokoll für diesen Knoten, über alle
    /// Dateien und Starts (Fund 624); die älteste Datei fällt zuerst weg.
    pub protokoll_mb: u64,
    /// Horchadressen (Multiaddr). Üblich sind zwei: TCP und QUIC.
    pub horchadressen: Vec<String>,
    /// Bootstrap-Knoten für den Einstieg.
    pub bootstrap: Vec<String>,
    /// Die Rolle.
    pub rolle: Rolle,
    /// Datei für das Blockprotokoll der Kette.
    ///
    /// ⚑ **Vorgabe `kette.dat` seit dem 2026-09-02 (Fund 122).** Bis
    /// dahin war sie `None`, ein Knoten ohne ausdrückliche Angabe hielt
    /// also **nichts** über einen Neustart hinweg. Die Begründung stand
    /// dabei und war ehrlich („solange die Kette Wegwerfware ist"), aber
    /// sie beschrieb den Probelauf und nicht den Betrieb.
    ///
    /// **Die sichere Vorgabe ist die umgekehrte.** Wer nichts behalten
    /// will, sagt es mit `--ohne-kette`; wer nichts sagt, behält.
    /// Dieselbe Bauart wie [`Self::schluesseldatei`], die seit jeher auf
    /// `knoten.key` steht statt auf nichts: Ein Knoten, der bei jedem
    /// Start eine neue Identität bekäme, wäre auch niemandem nützlich.
    ///
    /// **Was die Umstellung nicht ändert:** Eine Datei aus einem anderen
    /// Netz wird weiter abgewiesen, denn der Startwert steht in ihrem
    /// Kopf. Und eine unlesbare Datei bleibt ein Startfehler.
    pub kettendatei: Option<PathBuf>,
    /// Platzbudget der Kettendatei in MiB (Festlegung des
    /// Projektinhabers vom 2026-10-23: „ein paar GB“); darüber wird an
    /// einer Langzeitgrenze verdichtet, null heißt ohne Grenze.
    pub kette_mb: u64,
    /// ⚑ **Kalte Ausfuhr** (Archivknoten, Stufe A8): Jede fertige Periode
    /// geht in diesen Ordner, solange der Knoten die Geschichte ganz hat.
    pub archiv_ausfuhr: Option<PathBuf>,
    /// Stimmsatzdatei mit dem Validator-Satz.
    ///
    /// Ohne sie stimmt der Knoten nicht mit: Er nimmt am Netz teil,
    /// hört zu und rechnet nach, aber er fährt keine BFT-Runde. **Das
    /// ist der Normalfall**, denn stimmberechtigt sind wenige.
    pub stimmsatzdatei_pfad: Option<PathBuf>,
    /// Datei mit dem geheimen BLS-Konsensschlüssel.
    ///
    /// Getrennt von [`Self::schluesseldatei`], die die **Netzidentität**
    /// trägt. Zwei Geheimnisse, damit ein Leck nicht beide Ebenen
    /// zugleich trifft; siehe `crate::schluessel`.
    pub konsensschluesseldatei: Option<PathBuf>,
    /// NAT-Einstellungen (Relais, eigene öffentliche Adressen).
    pub nat: NatKonfig,
    /// Abstand der Zustandsaufnahmen im Protokoll, in Sekunden.
    pub aufnahme_sekunden: u64,
    /// Wo der Beobachtungsendpunkt horcht, oder `None`.
    ///
    /// ⚑ **Die Vorgabe ist die Rückschleife**, nicht `0.0.0.0` (siehe
    /// [`crate::beobachtung`]). Was dort heraussieht, ist eine
    /// Landkarte des Knotens: Peerzahl, Höhe, Latenzspanne. Für einen
    /// Betreiber ist das Diagnose, für einen Angreifer die Aufklärung.
    /// Wer weiter hinaus will, sagt es ausdrücklich und stellt eine
    /// Zugangskontrolle davor; der Endpunkt selbst hat keine.
    pub beobachtung: Option<std::net::SocketAddr>,
    /// Wo die eigene Tür horcht, oder `None`.
    ///
    /// ⚑ **Die Vorgabe ist die Rückschleife**, und das ist der
    /// entschiedene Zuschnitt (B6-3, 2026-09-03): nur das eigene
    /// Gateway. Der Betreiber ist der Kontoinhaber, der Verkehr
    /// verlässt die Maschine nie, und deshalb braucht die Tür weder TLS
    /// noch Rahmenwerk.
    ///
    /// ⚑ **Wer sie hinausbindet, verlässt den Zuschnitt.** Dann gelten
    /// K0s Einwände wieder: Ein Überlastangriff gegen die Tür wird zu
    /// einem gegen die Lebendigkeit des Konsenses.
    pub tuer: Option<std::net::SocketAddr>,
    /// Wo der lokale Shard-Prozess horcht, oder `None`.
    ///
    /// ⚑ **Die Vorgabe ist aus, anders als bei der Tür.** Ein Knoten
    /// ist nicht notwendig ein Miner: Er kann Shard in einem Pod sein,
    /// Speicher stellen oder einfach nur Nutzer sein (Fund 152). Wer
    /// rechnet, sagt es; alles andere wäre ein vorgetäuschter Shard.
    pub ortsleitung: Option<std::net::SocketAddr>,
    /// Wo der Ausweis des Shard-Prozesses liegt: Datei oder Verzeichnis.
    ///
    /// ⚑ **Ohne ihn gibt es keine Leitung**, denn die lokale Tür des
    /// Shards lässt niemanden ohne Ausweis herein.
    pub ortsausweis: Option<std::path::PathBuf>,
    /// Die Pod-Kennung, auf die sich Knoten und Shard einigen.
    ///
    /// ⚑ **Beide Seiten müssen dieselbe nennen.** Sie geht in die
    /// Ableitung des Sitzungskanals ein; zwei verschiedene Kennungen
    /// ergeben zwei Kanäle, und kein Umschlag geht auf.
    ///
    /// Für einen echten Pod kommt sie aus der Zuteilung der Kette. Im
    /// Betreiberzuschnitt der Phase 1 nennt sie der Betreiber, weil
    /// dieser Shard keinem Pod der Kette zugeteilt ist.
    pub pod: Option<[u8; 32]>,
    /// Wie das Modell heisst, das dieser Knoten nach aussen anbietet.
    pub modellname: String,
    /// Der Schlüssel, mit dem dieser Knoten **Kettentransaktionen**
    /// unterschreibt (Fund 170).
    ///
    /// ⚑ **Ohne ihn nimmt der Knoten `kette::schluessel_fuer(name)`,
    /// und der ist kein Geheimnis:** `probeschluessel(sha256(name)[0])`,
    /// einer von acht, aus dem Namen nachrechenbar. Für einen Probelauf
    /// gewollt, für ein Netz unbrauchbar; der Knoten sagt es beim Start.
    ///
    /// **Eine andere Datei als der Konsensschlüssel.** Ein Schlüssel,
    /// der arbeitet, und ein Schlüssel, der hält: Wer den einen stiehlt,
    /// soll nicht den anderen haben.
    pub kontoschluesseldatei: Option<PathBuf>,
    /// Wohin die Erträge dieses Knotens gehen sollen.
    ///
    /// ⚑ **Das kalte Konto**, also der haltende Teil des Paares. Mit
    /// Angabe meldet sich der Knoten mit seinem Konsensschlüssel als Miner
    /// an und trägt dieses Konto über die Kette als Auszahlungskonto ein
    /// (Fund 643: bis zum 2026-10-26 wurde die Angabe gelesen und nie
    /// benutzt). Ohne Angabe trägt er nichts ein, und es gilt „ohne Eintrag
    /// kein Anteil".
    pub konto: Option<Address>,
    /// Der Schlüssel des **bisher** eingetragenen Auszahlungskontos, für
    /// einen Wechsel. Die Kette nimmt eine zweite Eintragung nur mit der
    /// Unterschrift des eingetragenen Kontos an; ohne diesen Schlüssel
    /// bleibt das alte stehen, und der Knoten sagt es im Protokoll.
    pub kontowechseldatei: Option<PathBuf>,
    /// ⚑ **Die Relaisleitung** (2026-10-26): Hier fragt ein
    /// Koordinator auf derselben Maschine nach den Pods der Epoche und
    /// reicht Shardanfragen an die Mitglieder. Nur auf der Rückschleife
    /// sinnvoll; ohne Angabe keine Leitung.
    pub podrelais: Option<std::net::SocketAddr>,
    /// Wohin der Ausweis der Relaisleitung geschrieben wird (`0600`).
    pub podrelaisausweis: Option<PathBuf>,
    /// Abstand des Testverkehrs in Sekunden, `None` heißt keiner.
    ///
    /// **Nur für Testnetze.** Der Knoten schickt dann getaktet einen
    /// strukturell gültigen, inhaltlich bedeutungslosen Block ins
    /// Gossip. Ohne das belegt ein Mehrmaschinenlauf nur, dass die
    /// Knoten einander **finden**, nicht dass Nachrichten **fließen**,
    /// und das sind zwei verschiedene Aussagen.
    ///
    /// In einem echten Netz gehört das aus: Ein Knoten, der
    /// bedeutungslose Blöcke einspeist, ist dort ein Störer.
    pub testverkehr_sekunden: Option<u64>,
    /// ⚑ **Der Einsatz, den der Knoten als Miner hinterlegt**, in
    /// Kleinstbeträgen (2026-10-29): nach der Anmeldung, bis er erreicht
    /// ist. `None` heisst keiner. Bis hierher gab es im Betrieb keinen Weg,
    /// einen Einsatz zu hinterlegen, und ohne Einsatz schlachtet ein
    /// Schiedsurteil nichts.
    pub einsatz: Option<u64>,
    /// ⚑ **Zustandslos** (`--zustandslos`, zustandslose Validierung,
    /// 2026-10-29): Der Knoten hält Konten, Sitzungen, Manifeste und
    /// Hinterlegungen nicht, übernimmt Blöcke nur mit ihrem Zeugen, erzeugt
    /// keine Blöcke und nimmt keine fremden Transaktionen an. Nur seine
    /// eigenen Konten führt er mit.
    pub zustandslos: bool,
    /// ⚑ **Den grossen Zustandsbaum auf die Platte auslagern**
    /// (`--baum-auf-platte`, 2026-10-30, nach dem Vorbild von NOMT): Die
    /// Seiten liegen neben der Kettendatei (sonst im Protokollverzeichnis);
    /// im Speicher bleibt der obere Baum. Ein Zwischenspeicher: Der Knoten
    /// baut ihn beim Start neu.
    pub baum_auf_platte: bool,
    /// ⚑ **Zeugen bereithalten** (`--zeugen-bereithalten`, 2026-10-30): Ein
    /// voller Knoten erzeugt die Zeugen der Blöcke, die er anwendet, und
    /// liefert sie zustandslosen Knoten auf Nachfrage; so hängt ihre
    /// Prüfbarkeit nicht am Erzeuger allein.
    pub zeugen_bereithalten: bool,
    /// Ob dieser Knoten Blöcke **erzeugt**.
    ///
    /// **Genau einer im Netz**, sonst gabelt sich die Kette sofort:
    /// Zwei Erzeuger bauen zwei verschiedene Blöcke auf denselben
    /// Vorgänger, und beide Seiten weisen den jeweils anderen als
    /// „passt nicht an" zurück. Das ist kein Fehler, sondern die
    /// Abwesenheit von BFT: Wer entscheidet, welcher gilt, ist genau
    /// die Frage, die eine Abstimmungsrunde beantwortet, und die gibt
    /// es hier noch nicht.
    ///
    /// In der üblichen Aufstellung ist es die Anlaufstelle.
    pub erzeugt_bloecke: bool,
    /// Abstand der Blöcke des Erzeugers in Millisekunden, `None` heißt:
    /// gebaut wird nur im Takt des Testverkehrs.
    ///
    /// ⚑ **Ein eigener Takt, getrennt vom Testverkehr** (2026-10-22). Der
    /// Testverkehr läuft in ganzen Sekunden, eine Epoche hat 1 800 Blöcke;
    /// ein Lauf mit getrennten Prozessen bräuchte so eine halbe Stunde je
    /// Epoche. Mit diesem Takt baut der Erzeuger so schnell, wie der Lauf
    /// es verlangt, und der Testverkehr schickt daneben nur noch Proben.
    ///
    /// **Nur mit `erzeugt_bloecke`** (`KonfigFehler::BlocktaktOhneErzeuger`)
    /// und nie null: Ein Takt von null baute in einer Schleife ohne Pause.
    pub blocktakt_ms: Option<u64>,
    /// ⚑ **Der Anker für den Zustandsabzug vom Betreiber** (`--abzug`,
    /// 2026-10-22). Vor dem Anker im Quelltext des Netzes. Gilt nur für
    /// einen Knoten ohne eigene Kette.
    pub abzug: Option<crate::abzug::Anker>,
    /// Ob ein Knoten, der aus einem Abzug begann, die Blöcke bis dorthin
    /// trotzdem nachholt und nachrechnet, und anhält, wenn die Wurzel
    /// abweicht. Vorgabe an (Festlegung des Projektinhabers, 2026-10-22).
    pub hintergrundpruefung: bool,
    /// Die Namen aller Teilnehmer des Probelaufs.
    ///
    /// Daraus entsteht der Validatorsatz, gegen den Latenz-Atteste
    /// geprüft werden (A10). **Fehlt ein Name, werden dessen Atteste
    /// mit „unbekannter Aussteller" abgewiesen**, und das Protokoll
    /// sagt es genau so: Der häufigste Fall ist eine unvollständige
    /// Liste, nicht ein Angriff.
    ///
    /// Ist die Liste leer, prüft der Knoten Atteste gegen einen leeren
    /// Satz und weist damit alle ab. Das ist der sichere Vorgabefall:
    /// Ungeprüfte Atteste durchzulassen hieße, A10 offen zu halten.
    pub teilnehmer: Vec<String>,
    /// Die Speicherorte (`speicherorte.toml`), oder `None`.
    ///
    /// ⚑ **Der Knoten liest sie bei jeder Zustandsaufnahme neu** und
    /// zaehlt, was in den Orten fuer das Netz liegt. Wer die Datei
    /// aendert, sieht es in der naechsten Aufnahme, ohne Neustart.
    pub speicherorte: Option<PathBuf>,
    /// ⚑ **Das Netz** (Stufe 4, 2026-10-22): Probekette oder Testnetz mit
    /// dem int4-35B. Bestimmt Startwert und Podmodell der Kette.
    pub netz: &'static crate::netz::Netz,
    /// ⚑ **Was dieser Knoten für das Podmodell zusagt** (`--modellzusage`):
    /// Ebenen samt Kopf, Ebenenzeit, wahlweise ein gehaltener Bereich. Mit
    /// ihr meldet er sich beim Start als Miner an, wenn er es noch nicht
    /// ist, und reicht die Zusage ein; unterschrieben mit dem
    /// Konsensschlüssel.
    pub modellzusage: Option<myl_types::zusage::Modellzusage>,
    /// In welcher Zone er sich anmeldet.
    pub zone: myl_types::node_metadata::GeoRegion,
    /// ⚑ **Womit er seinen Shard startet** (P7), oder `None`: dann rechnet
    /// er nicht aus der Kette.
    pub shard: Option<Shardkonfig>,
}

/// Wie der Knoten seinen Shard aus der Kette startet (P7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shardkonfig {
    /// Das Programm `myl-shard`.
    pub programm: PathBuf,
    /// Das Artefakt des Netzmodells.
    pub artefakte: PathBuf,
    /// Mit Trainingshälfte.
    pub training: bool,
}

/// Fehler der Konfiguration.
#[derive(Debug)]
pub enum KonfigFehler {
    /// Eine Horchadresse ist keine gültige Multiaddr.
    UngueltigeHorchadresse(String),
    /// Es wurde keine Horchadresse angegeben.
    KeineHorchadresse,
    /// Die NAT-Einstellungen widersprechen sich (siehe `myl_net::nat`).
    Nat(myl_net::NatFehler),
    /// Ein Bootstrap-Eintrag ist unbrauchbar.
    Bootstrap(String),
    /// `--ortsleitung` ohne `--ortsausweis`.
    OrtsleitungOhneAusweis,
    /// `--ortsausweis` ohne `--ortsleitung`.
    AusweisOhneOrtsleitung,
    /// Ein Blocktakt bei einem Knoten, der keine Blöcke erzeugt.
    BlocktaktOhneErzeuger,
    /// Ein Blocktakt von null Millisekunden.
    BlocktaktNull,
}

impl std::fmt::Display for KonfigFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UngueltigeHorchadresse(a) => write!(f, "ungültige Horchadresse: {}", a),
            Self::KeineHorchadresse => write!(
                f,
                "keine Horchadresse angegeben: ein Knoten ohne Horchadresse ist \
                 für niemanden erreichbar und kann auch kein Relais nutzen"
            ),
            Self::Nat(e) => write!(f, "NAT-Einstellungen: {}", e),
            Self::Bootstrap(b) => write!(f, "unbrauchbarer Bootstrap-Eintrag: {}", b),
            Self::OrtsleitungOhneAusweis => write!(
                f,
                "--ortsleitung ohne --ortsausweis: die lokale Tuer des Shards laesst \
                 niemanden ohne Ausweis herein, die Leitung waere von Anfang an tot"
            ),
            Self::AusweisOhneOrtsleitung => write!(
                f,
                "--ortsausweis ohne --ortsleitung: ein Schluessel ohne Tuer"
            ),
            Self::BlocktaktOhneErzeuger => write!(
                f,
                "--blocktakt ohne --erzeuger: wer keine Bloecke baut, hat keinen Takt dafuer"
            ),
            Self::BlocktaktNull => write!(f, "--blocktakt 0: der Erzeuger baute ohne Pause"),
        }
    }
}

impl std::error::Error for KonfigFehler {}

/// Vorgabe-Horchadressen: TCP und QUIC auf demselben Port.
///
/// **Beide**, nicht eine. QUIC ist der Pfad, auf dem Lochstanzen
/// zuverlässig ist (Begründung in `myl_net::nat`); TCP ist der Pfad, der
/// auch durch restriktive Firewalls kommt, die UDP verwerfen. Wer nur
/// eines anbietet, verliert je nach Gegenüber das eine oder das andere.
pub fn standard_horchadressen(port: u16) -> Vec<String> {
    vec![
        format!("/ip4/0.0.0.0/tcp/{port}"),
        format!("/ip4/0.0.0.0/udp/{port}/quic-v1"),
    ]
}

impl Default for KnotenKonfig {
    fn default() -> Self {
        Self {
            name: "knoten".to_string(),
            schluesseldatei: PathBuf::from("knoten.key"),
            protokollverzeichnis: PathBuf::from("logs"),
            protokoll_mb: crate::protokoll::GESAMT_HOECHSTENS / (1024 * 1024),
            horchadressen: standard_horchadressen(4150),
            bootstrap: Vec::new(),
            rolle: Rolle::Teilnehmer,
            kettendatei: Some(PathBuf::from("kette.dat")),
            kette_mb: 2048,
            archiv_ausfuhr: None,
            stimmsatzdatei_pfad: None,
            konsensschluesseldatei: None,
            nat: NatKonfig::default(),
            aufnahme_sekunden: 30,
            // ⚑ **An, und zwar auf der Rückschleife** (Fund 129). Ein
            // Endpunkt, den niemand von außen erreicht, kostet nichts
            // und ist im Fehlerfall da. Aus war die Vorgabe genau so
            // lange, wie es ihn nicht gab.
            beobachtung: Some(std::net::SocketAddr::from(([127, 0, 0, 1], 4151))),
            // ⚑ **An, auf der Rückschleife** (B6-3). Eine Tür, die
            // niemand von aussen erreicht, kostet nichts und ist da,
            // wenn der Nutzer sie braucht.
            tuer: Some(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                crate::tuer::TUER_PORT,
            ))),
            // ⚑ **Aus.** Wer keinen Shard-Prozess fährt, soll keinen
            // vortäuschen; die Ablehnung ist dann die richtige Antwort.
            ortsleitung: None,
            ortsausweis: None,
            pod: None,
            modellname: "sparrow-0.6b-int8".to_string(),
            kontoschluesseldatei: None,
            konto: None,
            kontowechseldatei: None,
            podrelais: None,
            podrelaisausweis: None,
            testverkehr_sekunden: None,
            einsatz: None,
            zustandslos: false,
            baum_auf_platte: false,
            zeugen_bereithalten: false,
            erzeugt_bloecke: false,
            blocktakt_ms: None,
            abzug: None,
            hintergrundpruefung: true,
            teilnehmer: Vec::new(),
            speicherorte: None,
            netz: &crate::netz::PROBEKETTE,
            modellzusage: None,
            zone: myl_types::node_metadata::GeoRegion::Europe,
            shard: None,
        }
    }
}

impl KnotenKonfig {
    /// Prüft die Konfiguration auf Widersprüche.
    ///
    /// Wird beim Start aufgerufen. Ein Knoten, der mit widersprüchlicher
    /// Konfiguration startet, meldet sich später nicht mit einem Fehler,
    /// sondern mit Stille, und Stille ist das Schwerste zu debuggen.
    pub fn pruefe(&self) -> Result<(), KonfigFehler> {
        if self.horchadressen.is_empty() {
            return Err(KonfigFehler::KeineHorchadresse);
        }
        for a in &self.horchadressen {
            a.parse::<libp2p::Multiaddr>()
                .map_err(|_| KonfigFehler::UngueltigeHorchadresse(a.clone()))?;
        }
        for b in &self.bootstrap {
            myl_net::parse_bootstrap_peer(b).map_err(|_| KonfigFehler::Bootstrap(b.clone()))?;
        }
        // Die Rolle und die NAT-Einstellung müssen zusammenpassen: Wer
        // Relais sein will, muss es auch in den NAT-Einstellungen sein,
        // sonst schaltet `build_swarm` den Dienst gar nicht ein.
        let nat = self.nat_mit_rolle();
        myl_net::nat_pruefen(&nat).map_err(KonfigFehler::Nat)?;
        // ⚑ **Adresse und Ausweis gehören zusammen** (Klasse von
        // Fund 56). Eine Adresse ohne Ausweis wäre eine Leitung, die
        // bei jeder Frage abgewiesen wird, und ein Ausweis ohne
        // Adresse wäre ein Schlüssel ohne Tür: Beides sähe im Betrieb
        // aus wie „der Shard antwortet nicht".
        match (&self.ortsleitung, &self.ortsausweis) {
            (Some(_), None) => return Err(KonfigFehler::OrtsleitungOhneAusweis),
            (None, Some(_)) => return Err(KonfigFehler::AusweisOhneOrtsleitung),
            _ => {}
        }
        match self.blocktakt_ms {
            Some(_) if !self.erzeugt_bloecke => return Err(KonfigFehler::BlocktaktOhneErzeuger),
            Some(0) => return Err(KonfigFehler::BlocktaktNull),
            _ => {}
        }
        Ok(())
    }

    /// Die NAT-Einstellungen mit eingerechneter Rolle.
    ///
    /// Die Rolle ist die Angabe des Betreibers, die NAT-Einstellung die
    /// technische Folge. Sie hier abzuleiten statt beides getrennt
    /// führen zu lassen, verhindert den Fall „Rolle Relais, Dienst aus",
    /// der stumm bliebe.
    pub fn nat_mit_rolle(&self) -> NatKonfig {
        let mut nat = self.nat.clone();
        nat.dient_als_relais = self.rolle == Rolle::Relais;
        nat
    }

    /// Die eigenen Bootstrap-Angaben, danach die Einstiegsliste des Netzes,
    /// ohne Doppelte.
    pub fn einstiegsliste(&self) -> Vec<String> {
        let mut liste = self.bootstrap.clone();
        for e in self.netz.einstieg {
            if !liste.iter().any(|b| b == e) {
                liste.push((*e).to_string());
            }
        }
        liste
    }

    /// Die Netzkonfiguration für `myl-net`.
    pub fn netz(&self) -> NetConfig {
        NetConfig {
            listen_addr: self.horchadressen[0].clone(),
            bootstrap_peers: self.einstiegsliste(),
            nat: self.nat_mit_rolle(),
            lokale_suche: self.netz.lokale_suche,
            ..NetConfig::default()
        }
    }
}

#[cfg(test)]
mod tests {
    /// ⚑ **Die Kette wird standardmäßig geschrieben** (Fund 122).
    ///
    /// Bis zum 2026-09-02 stand hier `None`, ein Knoten ohne
    /// ausdrückliche Angabe hielt also nichts über einen Neustart
    /// hinweg. Der Test hält die Umkehr fest, damit sie niemand
    /// nebenbei zurückdreht.
    #[test]
    fn die_kette_wird_standardmaessig_geschrieben() {
        let k = super::KnotenKonfig::default();
        assert_eq!(
            k.kettendatei.as_deref(),
            Some(std::path::Path::new("kette.dat")),
            "ohne Angabe muss der Knoten seine Kette behalten"
        );
    }

    use super::*;

    #[test]
    fn die_vorgabe_horcht_auf_tcp_und_quic() {
        let a = standard_horchadressen(4150);
        assert_eq!(a.len(), 2);
        assert!(a[0].contains("/tcp/4150"));
        assert!(a[1].contains("/quic-v1"));
    }

    #[test]
    fn die_vorgabe_ist_gueltig() {
        KnotenKonfig::default().pruefe().expect("Vorgabe muss gültig sein");
    }

    #[test]
    fn ohne_horchadresse_ist_die_konfiguration_ungueltig() {
        let k = KnotenKonfig { horchadressen: Vec::new(), ..Default::default() };
        assert!(matches!(k.pruefe(), Err(KonfigFehler::KeineHorchadresse)));
    }

    #[test]
    fn unsinnige_horchadressen_fallen_beim_pruefen_auf() {
        let k = KnotenKonfig {
            horchadressen: vec!["kein multiaddr".to_string()],
            ..Default::default()
        };
        assert!(matches!(k.pruefe(), Err(KonfigFehler::UngueltigeHorchadresse(_))));
    }

    #[test]
    fn fund_56_rolle_relais_ohne_oeffentliche_adresse_faellt_auf() {
        // Der Knoten erbt die Prüfung aus `myl_net::nat`. Dieser Test
        // hält fest, dass sie über die Rolle auch wirklich erreicht wird:
        // Ohne `nat_mit_rolle` wäre die Rolle gesetzt und der Dienst aus,
        // und die Prüfung liefe ins Leere.
        let k = KnotenKonfig { rolle: Rolle::Relais, ..Default::default() };
        assert!(matches!(k.pruefe(), Err(KonfigFehler::Nat(_))));
    }

    #[test]
    fn rolle_relais_mit_adresse_geht_durch() {
        let k = KnotenKonfig {
            rolle: Rolle::Relais,
            nat: NatKonfig {
                oeffentliche_adressen: vec!["/ip4/203.0.113.5/tcp/4150".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        k.pruefe().expect("gültig");
        assert!(k.nat_mit_rolle().dient_als_relais);
    }

    #[test]
    fn ein_teilnehmer_dient_nie_als_relais() {
        // Auch dann nicht, wenn die NAT-Einstellung es behauptet: Die
        // Rolle ist die Angabe des Betreibers und hat Vorrang.
        let k = KnotenKonfig {
            rolle: Rolle::Teilnehmer,
            nat: NatKonfig { dient_als_relais: true, ..Default::default() },
            ..Default::default()
        };
        assert!(!k.nat_mit_rolle().dient_als_relais);
        k.pruefe().expect("gültig");
    }

    #[test]
    fn unbrauchbare_bootstrap_eintraege_fallen_auf() {
        let k = KnotenKonfig {
            bootstrap: vec!["/ip4/203.0.113.5/tcp/4150".to_string()],
            ..Default::default()
        };
        // Ohne p2p/-Anteil: Die Gegenstelle wäre nicht authentifizierbar.
        assert!(matches!(k.pruefe(), Err(KonfigFehler::Bootstrap(_))));
    }

    /// Ein Blocktakt gehört zum Erzeuger und ist nie null.
    #[test]
    fn der_blocktakt_gehoert_zum_erzeuger() {
        let ohne = KnotenKonfig { blocktakt_ms: Some(20), ..Default::default() };
        assert!(matches!(ohne.pruefe(), Err(KonfigFehler::BlocktaktOhneErzeuger)));
        let null = KnotenKonfig { blocktakt_ms: Some(0), erzeugt_bloecke: true, ..Default::default() };
        assert!(matches!(null.pruefe(), Err(KonfigFehler::BlocktaktNull)));
        let gut = KnotenKonfig { blocktakt_ms: Some(20), erzeugt_bloecke: true, ..Default::default() };
        gut.pruefe().expect("gültig");
    }

    #[test]
    fn rollen_kommen_aus_dem_kommandozeilenwort_zurueck() {
        assert_eq!(Rolle::aus_text("relais"), Some(Rolle::Relais));
        assert_eq!(Rolle::aus_text("teilnehmer"), Some(Rolle::Teilnehmer));
        assert_eq!(Rolle::aus_text("Relais"), None, "Groß/klein ist bedeutsam");
        assert_eq!(Rolle::aus_text("wächter"), None);
        assert_eq!(Rolle::Relais.als_text(), "relais");
    }
}
