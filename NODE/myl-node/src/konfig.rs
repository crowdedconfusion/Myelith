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
    /// ⚑ **Das kalte Konto**, also der haltende Teil des Paares. Ohne
    /// Angabe ist es die Adresse des unterschreibenden Schlüssels
    /// selbst, und die ist heiss.
    pub konto: Option<Address>,
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
            horchadressen: standard_horchadressen(4150),
            bootstrap: Vec::new(),
            rolle: Rolle::Teilnehmer,
            kettendatei: Some(PathBuf::from("kette.dat")),
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
            modellname: "myelith-qwen2.5-0.5b".to_string(),
            kontoschluesseldatei: None,
            konto: None,
            testverkehr_sekunden: None,
            erzeugt_bloecke: false,
            teilnehmer: Vec::new(),
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

    /// Die Netzkonfiguration für `myl-net`.
    pub fn netz(&self) -> NetConfig {
        NetConfig {
            listen_addr: self.horchadressen[0].clone(),
            bootstrap_peers: self.bootstrap.clone(),
            nat: self.nat_mit_rolle(),
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

    #[test]
    fn rollen_kommen_aus_dem_kommandozeilenwort_zurueck() {
        assert_eq!(Rolle::aus_text("relais"), Some(Rolle::Relais));
        assert_eq!(Rolle::aus_text("teilnehmer"), Some(Rolle::Teilnehmer));
        assert_eq!(Rolle::aus_text("Relais"), None, "Groß/klein ist bedeutsam");
        assert_eq!(Rolle::aus_text("wächter"), None);
        assert_eq!(Rolle::Relais.als_text(), "relais");
    }
}
