//! Gossipsub-Peer-Scoring (Punkt 4.3, Fund 53).
//!
//! Die zweite Hälfte der Eclipse-Verteidigung. [`crate::limits`] begrenzt,
//! **wie viele** Verbindungen jemand aufbauen darf; das Scoring begrenzt,
//! **was ihm eine Verbindung nützt**. Ein Peer mit schlechter Bewertung
//! fliegt aus dem Mesh, bekommt kein Gossip mehr und wird ab einer
//! Schwelle ganz ignoriert, ohne dass die Verbindung getrennt werden muss.
//!
//! ## ⚑ Gleitkomma, und warum es hier bleibt
//!
//! `PeerScoreParams` rechnet in `f64`. Das ist eine bewusst
//! **nicht** korrigierte Ausnahme, und sie steht in den dokumentierten
//! Zonen des Gleitkomma-Audits (`INTEGER_LLM/tests/audit/test_no_float.py`).
//!
//! Die Begründung ist nicht „das ist Bibliothekscode", sondern eine
//! inhaltliche: **Der Peer-Score ist keine Konsensgröße und darf keine
//! sein.** Er hängt an lokalen Beobachtungen, an Ankunftszeiten und an
//! der eigenen Sicht auf das Mesh. Zwei ehrliche Knoten müssen hier zu
//! *verschiedenen* Ergebnissen kommen dürfen, sonst wäre die Bewertung
//! manipulierbar, indem man sie global vorhersagbar macht. Eine
//! Ganzzahlfassung würde Bitgleichheit suggerieren, wo keine erwünscht
//! ist.
//!
//! Die Grenze läuft sauber: Kein Wert aus diesem Modul geht in einen
//! Block, ein Attest oder eine Ledger-Buchung ein.
//!
//! ## Was hier eingeschaltet wird
//!
//! 1. **IP-Kolokation** ([`IP_KOLOKATION_SCHWELLE`]) als Rückfalllinie.
//!    Gossipsub zählt Peers je IP-Adresse; über der Schwelle fällt der
//!    Score quadratisch. Die Schwelle bleibt bei der Vorgabe der
//!    Bibliothek, aus einem Grund, der beim Nachrechnen entstand und
//!    unten steht.
//! 2. **Verhaltensstrafe** (Vorgabewerte). Peers, die das Protokoll
//!    verletzen, etwa durch Graft-Flooding oder wiederholtes IWANT auf
//!    nie gelieferte Nachrichten, verlieren Punkte.
//! 3. **Schwellen** (Vorgabewerte): unter −10 kein Gossip mehr, unter
//!    −50 keine Weitergabe eigener Nachrichten an diesen Peer, unter −80
//!    Graylist, also vollständiges Ignorieren.
//!
//! ## Was hier ausdrücklich **nicht** eingeschaltet wird
//!
//! **Topic-Zustellungsbewertung** (`mesh_message_deliveries` und
//! Verwandte) bleibt aus. Der Grund ist derselbe, aus dem dieses Projekt
//! keine unkalibrierten Tests schreibt: Schlecht eingestellte
//! Zustellungsparameter bestrafen **ehrliche langsame Peers**, und
//! „langsam" heißt hier oft nur „weit weg". Für Myelith wäre das
//! besonders teuer, weil die Pod-Bildung bewusst Zonendiversität
//! erzwingt (Kap. 4.4): Genau die geografisch entfernten Knoten, die das
//! Protokoll haben will, würden zuerst aussortiert.
//!
//! Diese Parameter gehören gegen echten Verkehr eingestellt, nicht gegen
//! eine Vermutung. Solange es keine Messung aus einem laufenden Netz
//! gibt, ist „aus" die ehrliche Einstellung. Vermerkt als offener Punkt
//! offen (Punkt 4.4, Lasttest bei Zielnetzgröße).
//!
//! ## ⚑ Fund 54: Eine strengere Schwelle war schlechter, nicht besser
//!
//! Der erste Entwurf setzte [`IP_KOLOKATION_SCHWELLE`] auf 4, mit der
//! Begründung, das sei „gleichgezogen" mit
//! [`crate::limits::MAX_JE_ADRESSBEREICH`]. Der Integrationstest
//! `eine_ehrliche_verbindung_bleibt_erreichbar` hat das binnen einer
//! Minute widerlegt: Elf Knoten auf `127.0.0.1` ergeben einen Überschuss
//! von 7, quadriert 49, mal Gewicht −5 einen Score von **−245**. Die
//! Graylist-Schwelle liegt bei −80. **Die Härtung hat den ehrlichen
//! Knoten mit stummgeschaltet.**
//!
//! Beim Nachrechnen zeigte sich, dass die Zahl zusätzlich wirkungslos
//! war. Die Kolokationsstrafe zählt Identitäten je **Einzeladresse**;
//! eine Einzeladresse liegt innerhalb eines /24, und dort deckelt
//! [`crate::limits::Adressvielfalt`] eingehende Verbindungen bereits bei
//! [`crate::limits::MAX_JE_ADRESSBEREICH`]. Für den eingehenden Angriff,
//! gegen den die Schwelle gesetzt war, kann sie also gar nicht auslösen:
//! Die Verbindungsgrenze bindet vorher und schärfer.
//!
//! | Peers je IP | Score bei Schwelle 4 | Score bei Schwelle 10 |
//! |---|---|---|
//! | 5 | −5 | 0 |
//! | 6 | −20 (kein Gossip mehr) | 0 |
//! | 8 | **−80 (Graylist)** | 0 |
//! | 14 | −500 | −80 |
//!
//! Acht ehrliche Knoten hinter einer öffentlichen Adresse, ein kleiner
//! Betreiber, ein Universitätsnetz, ein Rechenzentrum mit einem
//! Ausgang, hätten sich gegenseitig ignoriert.
//!
//! **Übernommen wurde daher die Vorgabe der Bibliothek (10)**, plus eine
//! Ausnahme für Loopback. Die Lehre ist die des Projekts: Eine Zahl, die
//! „streng" klingt, ist keine Härtung, solange niemand ausgerechnet hat,
//! wen sie trifft.
//!
//! **Loopback ist von der Zählung ausgenommen**, aus demselben Grund wie
//! in [`crate::limits`]: Wer auf `127.0.0.1` Peers stellt, besitzt die
//! Maschine bereits, und die lokalen Testnetze liefen sonst gegen eine
//! Schranke, die für sie nicht gedacht ist. Die Ausnahme steht an beiden
//! Stellen, weil sie an beiden Stellen begründet ist.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use libp2p::gossipsub::{PeerScoreParams, PeerScoreThresholds};

/// Peers je IP-Adresse, ab denen der Score zu fallen beginnt.
///
/// **Die Vorgabe der Bibliothek, bewusst nicht verschärft** (Fund 54 im
/// Modulkopf). Die bindende Schranke für eingehende Verbindungen ist
/// [`crate::limits::MAX_JE_ADRESSBEREICH`]; diese Zahl ist die
/// Rückfalllinie für die Pfade, die jene nicht abdeckt, vor allem
/// selbst gewählte ausgehende Verbindungen.
pub const IP_KOLOKATION_SCHWELLE: u32 = 10;

/// Adressen, die von der Kolokationszählung ausgenommen sind.
///
/// Nur Loopback, und nur mit der Begründung aus dem Modulkopf. Private
/// Bereiche stehen bewusst **nicht** hier: Ein Angreifer im selben LAN
/// ist ein echter Angreifer, und bei Schwelle 10 haben kleine
/// LAN-Aufbauten ohnehin Luft.
fn kolokation_ausnahmen() -> std::collections::HashSet<IpAddr> {
    let mut menge = std::collections::HashSet::new();
    menge.insert(IpAddr::V4(Ipv4Addr::LOCALHOST));
    menge.insert(IpAddr::V6(Ipv6Addr::LOCALHOST));
    menge
}

/// Die Bewertungsparameter des Protokolls.
pub fn standard_parameter() -> PeerScoreParams {
    PeerScoreParams {
        ip_colocation_factor_threshold: f64::from(IP_KOLOKATION_SCHWELLE),
        ip_colocation_factor_whitelist: kolokation_ausnahmen(),
        ..Default::default()
    }
}

/// Die Bewertungsschwellen des Protokolls.
///
/// Die Vorgabewerte der Bibliothek, bewusst unverändert: Sie stammen aus
/// dem gossipsub-v1.1-Entwurf und aus dem Betrieb großer Netze. Eigene
/// Werte ohne eigene Messung wären eine Verschlechterung mit mehr
/// Selbstbewusstsein.
pub fn standard_schwellen() -> PeerScoreThresholds {
    PeerScoreThresholds::default()
}

/// Zählt die Peers, deren Bewertung unter die Gossip-Schwelle gefallen
/// ist, die also **kein Gossip mehr bekommen**.
///
/// Die Rechnung steht hier und nicht in [`crate::runtime`], weil sie in
/// `f64` läuft: Der Gleitkomma-Audit prüft `runtime.rs`, dieses Modul ist
/// die dokumentierte Ausnahme. Nach außen geht eine **Ganzzahl**.
///
/// **Wofür die Zahl da ist:** Ein bewerteter Peer sieht im Protokoll
/// aus wie ein stiller. Steht hier eine Zahl über null, während gleich-
/// zeitig Verbindungen bestehen und nichts ankommt, ist die Ursache
/// gefunden.
pub fn schlechte_peers(gossipsub: &libp2p::gossipsub::Behaviour) -> usize {
    let schwelle = standard_schwellen().gossip_threshold;
    gossipsub
        .all_peers()
        .filter(|(peer, _)| gossipsub.peer_score(peer).is_some_and(|s| s < schwelle))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::MAX_JE_ADRESSBEREICH;

    #[test]
    fn fund_54_die_kolokationsschwelle_ist_lockerer_als_die_verbindungsgrenze() {
        // Der erste Entwurf zog beide Zahlen gleich. Das war falsch, und
        // zwar in beide Richtungen: schädlich für ehrliche Mitbewohner
        // einer Adresse, wirkungslos gegen den eingehenden Angriff, weil
        // die Verbindungsgrenze vorher bindet. Dieser Test hält fest,
        // dass die Ordnung so bleibt.
        assert!(
            IP_KOLOKATION_SCHWELLE as usize > MAX_JE_ADRESSBEREICH,
            "die Kolokationsstrafe darf nicht schärfer greifen als die \
             Verbindungsgrenze, sonst trifft sie nur Ehrliche (Fund 54)"
        );
    }

    #[test]
    fn fund_54_acht_knoten_auf_einer_adresse_bleiben_hoerbar() {
        // Die Rechnung, die den ersten Entwurf widerlegt hat, als Test.
        // Acht ehrliche Knoten hinter einem Ausgang, ein kleiner
        // Betreiber, dürfen sich nicht gegenseitig ignorieren.
        let p = standard_parameter();
        let s = standard_schwellen();
        let ueberschuss = (8.0f64 - p.ip_colocation_factor_threshold).max(0.0);
        let score = ueberschuss * ueberschuss * p.ip_colocation_factor_weight;
        assert!(
            score > s.graylist_threshold,
            "acht Peers auf einer Adresse ergeben {score}, Graylist bei {}",
            s.graylist_threshold
        );
        assert!(
            score > s.gossip_threshold,
            "acht Peers auf einer Adresse verlieren das Gossip: {score}"
        );
    }

    #[test]
    fn loopback_ist_von_der_kolokation_ausgenommen() {
        // Ohne diese Ausnahme schalten sich die lokalen Testnetze selbst
        // stumm: elf Knoten auf 127.0.0.1 ergeben −245 bei Graylist −80.
        let p = standard_parameter();
        assert!(p
            .ip_colocation_factor_whitelist
            .contains(&IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(p
            .ip_colocation_factor_whitelist
            .contains(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn private_bereiche_sind_nicht_ausgenommen() {
        // Ein Angreifer im selben LAN ist ein echter Angreifer.
        let p = standard_parameter();
        assert!(!p
            .ip_colocation_factor_whitelist
            .contains(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(!p
            .ip_colocation_factor_whitelist
            .contains(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
    }

    #[test]
    fn die_parameter_sind_gueltig() {
        // `with_peer_score` ruft `validate()` und schlägt sonst zur
        // Laufzeit fehl, also erst beim Start eines echten Knotens.
        standard_parameter().validate().expect("Parameter");
        standard_schwellen().validate().expect("Schwellen");
    }

    #[test]
    fn die_kolokationsstrafe_ist_scharf() {
        // Gewicht 0 hieße: Die Schwelle steht da und wirkt nicht.
        let p = standard_parameter();
        assert!(p.ip_colocation_factor_weight < 0.0);
        assert!(p.behaviour_penalty_weight < 0.0);
    }

    #[test]
    fn die_zustellungsbewertung_ist_bewusst_leer() {
        // Hält die Doku-Aussage fest: Wer Topic-Parameter ergänzt, soll
        // hier vorbeikommen und die Begründung im Modulkopf nachziehen.
        assert!(
            standard_parameter().topics.is_empty(),
            "Topic-Zustellungsbewertung ist aus, bis sie gegen echten \
             Verkehr eingestellt werden kann (siehe Modul-Doku)"
        );
    }

    #[test]
    fn die_schwellen_stehen_in_der_erwarteten_ordnung() {
        let s = standard_schwellen();
        assert!(s.graylist_threshold < s.publish_threshold);
        assert!(s.publish_threshold < s.gossip_threshold);
        assert!(s.gossip_threshold <= 0.0);
    }
}
