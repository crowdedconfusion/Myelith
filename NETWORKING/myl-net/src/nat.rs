//! NAT-Überwindung: Erreichbarkeit, Relais, Lochstanzen (Punkt 3.4).
//!
//! ## Warum das keine Bequemlichkeit ist
//!
//! Bis zum 2026-08-24 sprach `myl-net` nur TCP ohne jede
//! NAT-Behandlung. Ein Knoten hinter einem Heimrouter konnte **hinaus**
//! wählen, aber niemand konnte ihn **an**wählen.
//!
//! Für ein Testnetz auf einer Maschine fällt das nicht auf, weil dort
//! jede Adresse erreichbar ist. Für ein Netz über das Internet ist es
//! die Grenze zwischen „läuft" und „läuft nicht", und für Myelith kommt
//! ein zweiter Grund dazu: **Ein Netz, in dem nur öffentlich
//! erreichbare Knoten mitmachen können, ist ein anderes Netz.** Es ist
//! kleiner, teurer im Betrieb und in den Händen weniger Anbieter. Die
//! Sicherheitsrechnung des Whitepapers (Anhang B.2, Kollusion
//! `P_koll ≈ β^{2k}`) hängt daran, dass β klein ist, also dass ein
//! einzelner Akteur nur einen kleinen Teil der Knoten stellt. Wer
//! Heimanschlüsse ausschließt, treibt β nach oben.
//!
//! ## Die drei Bausteine
//!
//! 1. **AutoNAT v2** stellt fest, ob der Knoten von außen erreichbar
//!    ist. Ohne diese Feststellung weiß er nicht, ob er ein Relais
//!    braucht, und würde entweder unnötig eines belegen oder unerreichbar
//!    bleiben.
//! 2. **Relais (Circuit Relay v2)** vermittelt eine Verbindung über
//!    einen dritten, erreichbaren Knoten. Das ist der Notausgang: Er
//!    funktioniert immer, kostet aber Bandbreite beim Vermittler.
//! 3. **DCUtR** (Direct Connection Upgrade through Relay) nutzt die
//!    vermittelte Verbindung, um beiden Seiten gleichzeitig ein Loch in
//!    ihre NATs zu stanzen. Gelingt das, läuft der Verkehr direkt, und
//!    das Relais ist wieder frei.
//!
//! ## ⚑ Warum QUIC dazukommt, und nicht „irgendwann"
//!
//! Lochstanzen über **TCP** heißt „simultaneous open": Beide Seiten
//! wählen im selben Moment, und die NATs müssen die eintreffenden Pakete
//! den eigenen ausgehenden zuordnen. Viele verbreitete NAT-Bauarten tun
//! das für TCP nicht zuverlässig. Über **UDP**, und damit über QUIC, ist
//! dieselbe Aufgabe deutlich verlässlicher, weil UDP-Zuordnungen
//! großzügiger gehalten werden.
//!
//! TCP allein wäre also ein Stack, der DCUtR **enthält** und bei dem das
//! Lochstanzen trotzdem oft scheitert. Genau die Sorte Halbheit, die im
//! Testlauf als „geht manchmal" auffällt und niemandem sagt, warum.
//! Deshalb spricht der Knoten beides: TCP für die verlässliche
//! Grundverbindung, QUIC für das Lochstanzen.
//!
//! ## Was ein Relais sieht, und was nicht
//!
//! Ein Relais leitet Bytes weiter, die zwischen den beiden Endpunkten
//! mit Noise verschlüsselt sind. **Inhalte sieht es nicht.** Es sieht
//! aber, **wer wann mit wem** spricht, und es kann die Verbindung
//! abbrechen.
//!
//! Für Myelith ist das eine benannte Angreiferklasse und keine
//! Nebensache: Wer die Vermittlung stellt, kann Verbindungen
//! aussuchen, und das ist derselbe Hebel wie beim Eclipse-Angriff aus
//! [`crate::limits`]. **Ein Knoten, der nur über Relais eines einzigen
//! Betreibers erreichbar ist, hat dessen Eclipse-Problem.** Deshalb
//! nimmt [`NatKonfig`] eine **Liste** von Relais und nicht eines, und
//! deshalb ist DCUtR nicht optional: Jede erfolgreich direkt gemachte
//! Verbindung nimmt dem Relais seinen Hebel.
//!
//! ## ⚑ Fund 56: Ein Relais ohne eigene Adresse ist keins
//!
//! Der erste Entwurf hatte für den Relais-Dienst einen einzigen
//! Schalter, `dient_als_relais: bool`. Der Integrationstest lief ins
//! Leere, und der Ereignismitschnitt sagte warum:
//!
//! ```text
//! [Relais]  ReservationReqAccepted { renewed: false }
//! [Klient]  ListenerClosed { reason: Reservation(NoAddressesInReservation) }
//! ```
//!
//! **Das Relais nimmt die Reservierung an und schickt eine Antwort ohne
//! Adressen.** Der Grund ist einleuchtend, sobald man ihn sieht: In die
//! Reservierungsantwort trägt ein Relais seine **bestätigten externen
//! Adressen** ein, denn genau die soll der Klient anderen nennen können.
//! Ein frisch gestarteter Knoten hat keine bestätigte externe Adresse,
//! nur Kandidaten, und Kandidaten zählen nicht.
//!
//! **Ein Relais muss seine öffentliche Adresse kennen.** Das ist keine
//! Umgehung, sondern die Sache selbst: Wer vermitteln will, muss sagen
//! können, wohin. Deshalb hat [`NatKonfig`] jetzt
//! [`NatKonfig::oeffentliche_adressen`], und [`pruefe`] weist einen
//! Relais-Dienst ohne Adresse **ab**, statt ihn stillschweigend
//! wirkungslos zu lassen. Ein Relais, das niemand erreichen kann, ist
//! die teuerste Art von Konfigurationsfehler: Alles läuft, nur
//! niemand kommt an.
//!
//! ## Wer dient als Relais
//!
//! Nur Knoten, die sich ausdrücklich dazu erklären
//! ([`NatKonfig::dient_als_relais`]). Jeden Knoten zum Relais zu machen
//! wäre teuer und ein Angriffsziel: Ein Relais bezahlt fremden Verkehr
//! mit eigener Bandbreite. Dieselbe Erklärung schaltet den
//! AutoNAT-**Server** frei, denn beides setzt dasselbe voraus, nämlich
//! öffentliche Erreichbarkeit.

use libp2p::multiaddr::Protocol;
use libp2p::Multiaddr;

/// NAT-Konfiguration eines Knotens.
#[derive(Debug, Clone, Default)]
pub struct NatKonfig {
    /// Der Knoten stellt sich als **Relais** und AutoNAT-Server zur
    /// Verfügung. Nur für Knoten mit öffentlich erreichbarer Adresse
    /// sinnvoll, siehe Modul-Doku.
    pub dient_als_relais: bool,
    /// Relais-Adressen (Multiaddr mit `p2p/…`-Anteil), über die dieser
    /// Knoten erreichbar sein will.
    ///
    /// **Mehrere**, nicht eines: Ein einzelnes Relais ist ein einzelner
    /// Punkt, an dem jemand entscheidet, wer den Knoten erreicht.
    pub relais: Vec<String>,
    /// Die eigenen, von außen erreichbaren Adressen.
    ///
    /// **Für einen Relais-Knoten Pflicht** (Fund 56): Sie stehen in der
    /// Reservierungsantwort, die ein Klient anderen weiterreicht. Ohne
    /// sie nimmt das Relais Reservierungen an, die niemandem nützen.
    ///
    /// Für einen gewöhnlichen Knoten optional. Wer seine öffentliche
    /// Adresse kennt, spart sich den AutoNAT-Umweg; wer sie nicht kennt,
    /// lässt das Feld leer und wartet auf die Feststellung.
    pub oeffentliche_adressen: Vec<String>,
}

/// Fehler der NAT-Konfiguration.
#[derive(Debug)]
pub enum NatFehler {
    /// Eine Relais-Adresse ist keine gültige Multiaddr.
    UngueltigeAdresse(String),
    /// Einer Relais-Adresse fehlt der `p2p/…`-Anteil. Ohne PeerId kann
    /// der Knoten die Gegenstelle nicht authentifizieren, und ein
    /// Relais ohne Authentifizierung ist ein offener Umleitungspunkt.
    OhnePeerId(String),
    /// Relais-Dienst erklärt, aber keine eigene öffentliche Adresse
    /// angegeben (Fund 56).
    RelaisOhneAdresse,
}

impl std::fmt::Display for NatFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UngueltigeAdresse(a) => write!(f, "ungültige Relais-Adresse: {}", a),
            Self::OhnePeerId(a) => write!(
                f,
                "Relais-Adresse ohne p2p/-Anteil (PeerId), damit nicht authentifizierbar: {}",
                a
            ),
            Self::RelaisOhneAdresse => write!(
                f,
                "Relais-Dienst erklärt, aber keine eigene öffentliche Adresse angegeben: \
                 die Reservierungsantwort wäre leer und für den Klienten wertlos \
                 (NatKonfig::oeffentliche_adressen setzen)"
            ),
        }
    }
}

impl std::error::Error for NatFehler {}

/// Prüft die NAT-Konfiguration auf sich widersprechende Angaben.
///
/// Der einzige harte Fall ist Fund 56: **Relais-Dienst ohne eigene
/// öffentliche Adresse.** Das wäre kein Relais, sondern ein Knoten, der
/// Reservierungen annimmt und Antworten ohne Ziel verschickt. Lieber ein
/// Fehler beim Start als ein Netz, in dem niemand ankommt.
pub fn pruefe(config: &NatKonfig) -> Result<(), NatFehler> {
    if config.dient_als_relais && config.oeffentliche_adressen.is_empty() {
        return Err(NatFehler::RelaisOhneAdresse);
    }
    for a in &config.oeffentliche_adressen {
        a.parse::<Multiaddr>()
            .map_err(|_| NatFehler::UngueltigeAdresse(a.clone()))?;
    }
    alle_horchadressen(config)?;
    Ok(())
}

/// Die eigenen öffentlichen Adressen als Multiaddr.
pub fn eigene_adressen(config: &NatKonfig) -> Result<Vec<Multiaddr>, NatFehler> {
    config
        .oeffentliche_adressen
        .iter()
        .map(|a| {
            a.parse::<Multiaddr>()
                .map_err(|_| NatFehler::UngueltigeAdresse(a.clone()))
        })
        .collect()
}

/// Baut aus einer Relais-Adresse die Horchadresse, unter der der Knoten
/// über dieses Relais erreichbar ist: die Relais-Adresse mit
/// angehängtem `/p2p-circuit`.
///
/// Auf diese Adresse wird gehorcht, nicht gewählt. Das ist der Punkt,
/// an dem sich der Knoten beim Relais einen Platz reserviert.
pub fn relais_horchadresse(relais: &str) -> Result<Multiaddr, NatFehler> {
    let addr: Multiaddr = relais
        .parse()
        .map_err(|_| NatFehler::UngueltigeAdresse(relais.to_string()))?;
    if !addr.iter().any(|p| matches!(p, Protocol::P2p(_))) {
        return Err(NatFehler::OhnePeerId(relais.to_string()));
    }
    Ok(addr.with(Protocol::P2pCircuit))
}

/// Alle Horchadressen der konfigurierten Relais.
pub fn alle_horchadressen(config: &NatKonfig) -> Result<Vec<Multiaddr>, NatFehler> {
    config.relais.iter().map(|r| relais_horchadresse(r)).collect()
}

/// Ob eine Adresse über ein Relais führt.
///
/// Der Unterschied ist im Betrieb wichtig: Eine vermittelte Verbindung
/// belegt fremde Bandbreite und trägt fremdes Vertrauen. Wer sie zählt,
/// sieht, ob DCUtR seine Arbeit tut.
pub fn ist_vermittelt(addr: &Multiaddr) -> bool {
    addr.iter().any(|p| matches!(p, Protocol::P2pCircuit))
}

/// Ob eine Adresse über QUIC läuft (und damit über den Pfad, auf dem
/// Lochstanzen zuverlässig ist).
pub fn ist_quic(addr: &Multiaddr) -> bool {
    addr.iter().any(|p| matches!(p, Protocol::QuicV1 | Protocol::Quic))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELAIS: &str = "/ip4/203.0.113.5/tcp/4150/p2p/12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN";

    #[test]
    fn die_horchadresse_haengt_p2p_circuit_an() {
        let addr = relais_horchadresse(RELAIS).expect("Adresse");
        assert!(ist_vermittelt(&addr));
        assert!(addr.to_string().ends_with("/p2p-circuit"));
    }

    #[test]
    fn ein_relais_ohne_peerid_wird_abgelehnt() {
        // Ohne PeerId ist die Gegenstelle nicht authentifizierbar, und
        // ein Relais ohne Authentifizierung ist ein offener
        // Umleitungspunkt: Wer die Adresse abfängt, ist das Relais.
        let ohne = "/ip4/203.0.113.5/tcp/4150";
        assert!(matches!(
            relais_horchadresse(ohne),
            Err(NatFehler::OhnePeerId(_))
        ));
    }

    #[test]
    fn unsinn_wird_abgelehnt() {
        assert!(matches!(
            relais_horchadresse("kein multiaddr"),
            Err(NatFehler::UngueltigeAdresse(_))
        ));
    }

    #[test]
    fn eine_direkte_adresse_gilt_nicht_als_vermittelt() {
        let direkt: Multiaddr = "/ip4/203.0.113.5/tcp/4150".parse().unwrap();
        assert!(!ist_vermittelt(&direkt));
    }

    #[test]
    fn quic_wird_erkannt() {
        let quic: Multiaddr = "/ip4/203.0.113.5/udp/4150/quic-v1".parse().unwrap();
        let tcp: Multiaddr = "/ip4/203.0.113.5/tcp/4150".parse().unwrap();
        assert!(ist_quic(&quic));
        assert!(!ist_quic(&tcp));
    }

    #[test]
    fn alle_horchadressen_meldet_den_ersten_fehler() {
        let config = NatKonfig {
            dient_als_relais: false,
            relais: vec![RELAIS.to_string(), "/ip4/203.0.113.6/tcp/4150".to_string()],
            oeffentliche_adressen: Vec::new(),
        };
        assert!(alle_horchadressen(&config).is_err());
    }

    #[test]
    fn fund_56_relais_ohne_eigene_adresse_wird_abgelehnt() {
        let config = NatKonfig {
            dient_als_relais: true,
            relais: Vec::new(),
            oeffentliche_adressen: Vec::new(),
        };
        assert!(matches!(pruefe(&config), Err(NatFehler::RelaisOhneAdresse)));
    }

    #[test]
    fn ein_relais_mit_adresse_geht_durch() {
        let config = NatKonfig {
            dient_als_relais: true,
            relais: Vec::new(),
            oeffentliche_adressen: vec!["/ip4/203.0.113.5/tcp/4150".to_string()],
        };
        pruefe(&config).expect("gültig");
        assert_eq!(eigene_adressen(&config).expect("Adressen").len(), 1);
    }

    #[test]
    fn ein_gewoehnlicher_knoten_braucht_keine_eigene_adresse() {
        // Nur der Relais-Dienst verlangt sie. Wer bloß hinter NAT sitzt,
        // kennt seine Außenadresse gerade nicht, das ist der Normalfall.
        let config = NatKonfig {
            dient_als_relais: false,
            relais: vec![RELAIS.to_string()],
            oeffentliche_adressen: Vec::new(),
        };
        pruefe(&config).expect("gültig");
    }

    #[test]
    fn ohne_relais_gibt_es_keine_horchadressen() {
        let config = NatKonfig::default();
        assert!(alle_horchadressen(&config).expect("leer").is_empty());
        assert!(!config.dient_als_relais, "Relais-Dienst ist nicht die Vorgabe");
    }
}
