//! Anker ueber Neustarts (Audit A9, 2026-11-19).
//!
//! # Wovor das schuetzt
//!
//! Ein Neustart ist der guenstigste Augenblick fuer eine Umzingelung: Der
//! Knoten hat keine Verbindungen und nimmt, was zuerst kommt. Bitcoin Core
//! haelt deshalb seit Fassung 0.21 zwei ausgehende Gegenstellen in
//! `anchors.dat` fest und waehlt sie nach dem Start zuerst (nach Heilman et
//! al. 2015, Gegenmassnahme "anchor connections"). Hier dasselbe: hoechstens
//! [`ANKER_HOECHSTENS`] selbst gewaehlte Gegenstellen, die seit mindestens
//! [`ANKER_MINDESTALTER`] stehen, aus verschiedenen Netzgruppen.
//!
//! Die Datei schreibt und liest der Knoten (der Besitzer der Dateien), diese
//! Kiste waehlt nur aus und liest das Format: eine Multiadresse mit `/p2p/`
//! je Zeile.

use std::time::Duration;

use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};

use crate::netzgruppe::{netzgruppe, AsZuordnung, Netzgruppe};

/// Hoechstens so viele Anker, wie bei Bitcoin Core.
pub const ANKER_HOECHSTENS: usize = 2;

/// So lange muss eine ausgehende Verbindung stehen, bevor sie Anker wird.
/// Eine Gegenstelle, die gerade erst gewaehlt ist, hat sich nicht bewaehrt.
pub const ANKER_MINDESTALTER: Duration = Duration::from_secs(300);

/// Eine ausgehende Verbindung, die Anker werden koennte.
#[derive(Debug, Clone)]
pub struct Kandidat {
    /// Die Gegenstelle.
    pub peer: PeerId,
    /// Die gewaehlte Adresse, ohne `/p2p/`.
    pub addr: Multiaddr,
    /// Wie lange die Verbindung steht.
    pub alter: Duration,
}

/// Die Anker: die aeltesten ausgehenden Verbindungen ab
/// [`ANKER_MINDESTALTER`], je Netzgruppe eine, ohne Relaiswege, mit
/// `/p2p/`.
///
/// ⚑ Adressen ohne Gruppe (private Netze, Rueckschleife) zaehlen jede fuer
/// sich: Im lokalen Netz waehlt niemand billig viele Identitaeten in einem
/// fremden Bereich.
pub fn waehlen(kandidaten: &[Kandidat], zuordnung: Option<&AsZuordnung>) -> Vec<Multiaddr> {
    let mut sortiert: Vec<&Kandidat> = kandidaten
        .iter()
        .filter(|k| k.alter >= ANKER_MINDESTALTER)
        .filter(|k| !k.addr.iter().any(|p| matches!(p, Protocol::P2pCircuit)))
        .collect();
    // Aelteste zuerst; bei Gleichstand nach Kennung, damit die Wahl stabil ist.
    sortiert.sort_by(|a, b| b.alter.cmp(&a.alter).then(a.peer.cmp(&b.peer)));
    let mut gruppen: Vec<Netzgruppe> = Vec::new();
    let mut peers: Vec<PeerId> = Vec::new();
    let mut aus = Vec::new();
    for k in sortiert {
        if aus.len() >= ANKER_HOECHSTENS {
            break;
        }
        if peers.contains(&k.peer) {
            continue;
        }
        if let Some(g) = netzgruppe(&k.addr, zuordnung) {
            if gruppen.contains(&g) {
                continue;
            }
            gruppen.push(g);
        }
        peers.push(k.peer);
        aus.push(k.addr.clone().with(Protocol::P2p(k.peer)));
    }
    aus
}

/// Liest eine Ankerdatei: eine Adresse mit `/p2p/` je Zeile, `#` beginnt
/// einen Kommentar. Unlesbare Zeilen und solche ohne Kennung fallen weg, und
/// mehr als [`ANKER_HOECHSTENS`] werden nicht genommen: Die Datei liegt im
/// Datenordner und koennte veraendert sein.
pub fn lesen(text: &str) -> Vec<(PeerId, Multiaddr)> {
    text.lines()
        .map(|z| z.split('#').next().unwrap_or("").trim())
        .filter(|z| !z.is_empty())
        .filter_map(|z| z.parse::<Multiaddr>().ok())
        .filter_map(|a| {
            let peer = a.iter().find_map(|p| match p {
                Protocol::P2p(id) => Some(id),
                _ => None,
            })?;
            Some((peer, a))
        })
        .take(ANKER_HOECHSTENS)
        .collect()
}

/// Der Text einer Ankerdatei.
pub fn als_text(anker: &[Multiaddr]) -> String {
    let mut t = String::from("# Anker des Knotens: die zuletzt bewaehrten ausgehenden Gegenstellen.\n");
    for a in anker.iter().take(ANKER_HOECHSTENS) {
        t.push_str(&a.to_string());
        t.push('\n');
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(addr: &str, sek: u64) -> Kandidat {
        Kandidat { peer: PeerId::random(), addr: addr.parse().unwrap(), alter: Duration::from_secs(sek) }
    }

    #[test]
    fn die_aeltesten_aus_verschiedenen_gruppen() {
        let ks = vec![
            k("/ip4/203.0.113.1/tcp/4150", 1000),
            k("/ip4/203.0.200.1/tcp/4150", 2000), // dieselbe /16 wie der erste, aelter
            k("/ip4/198.51.100.1/tcp/4150", 500),
            k("/ip4/192.0.2.1/tcp/4150", 100), // zu jung
        ];
        let a = waehlen(&ks, None);
        assert_eq!(a.len(), 2);
        assert!(a[0].to_string().starts_with("/ip4/203.0.200.1/tcp/4150/p2p/"));
        assert!(a[1].to_string().starts_with("/ip4/198.51.100.1/tcp/4150/p2p/"));
    }

    #[test]
    fn junge_und_vermittelte_werden_nie_anker() {
        let ks = vec![
            k("/ip4/203.0.113.1/tcp/4150", 299),
            k("/ip4/198.51.100.1/tcp/4001/p2p/12D3KooWHtF9SZPRHUrToZs7dXKZLgXZENTckskMgy5XhKEUPzwW/p2p-circuit", 9999),
        ];
        assert!(waehlen(&ks, None).is_empty());
    }

    #[test]
    fn private_adressen_zaehlen_jede_fuer_sich() {
        let ks = vec![k("/ip4/172.18.0.2/tcp/4150", 900), k("/ip4/172.18.0.3/tcp/4150", 800)];
        assert_eq!(waehlen(&ks, None).len(), 2);
    }

    #[test]
    fn die_datei_hin_und_zurueck() {
        let ks = vec![k("/ip4/203.0.113.1/tcp/4150", 1000), k("/ip4/198.51.100.1/tcp/4150", 900)];
        let a = waehlen(&ks, None);
        let gelesen = lesen(&als_text(&a));
        assert_eq!(gelesen.len(), 2);
        assert_eq!(gelesen[0].1, a[0]);
        assert_eq!(gelesen[0].0, ks[0].peer);
    }

    #[test]
    fn eine_veraenderte_datei_bringt_hoechstens_zwei_gueltige() {
        let p = PeerId::random();
        let text = format!(
            "kaputt\n/ip4/1.2.3.4/tcp/1\n/ip4/1.2.3.4/tcp/1/p2p/{p}\n/ip4/1.2.3.5/tcp/1/p2p/{p}\n/ip4/1.2.3.6/tcp/1/p2p/{p}\n"
        );
        let g = lesen(&text);
        assert_eq!(g.len(), 2, "ohne Kennung und ueber zwei fallen weg");
    }
}
