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
    /// NAT-Einstellungen (Relais, eigene öffentliche Adressen).
    pub nat: NatKonfig,
    /// Abstand der Zustandsaufnahmen im Protokoll, in Sekunden.
    pub aufnahme_sekunden: u64,
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
            nat: NatKonfig::default(),
            aufnahme_sekunden: 30,
            testverkehr_sekunden: None,
            erzeugt_bloecke: false,
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
