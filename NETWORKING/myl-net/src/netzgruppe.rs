//! Netzgruppen fuer die Vielfalt der ausgehenden Verbindungen (Audit A9).
//!
//! # Wovor das schuetzt
//!
//! Ein Angreifer, der einem Knoten alle Gegenstellen stellt, entscheidet,
//! was dieser Knoten sieht (Umzingelung, "Eclipse"; Heilman et al. 2015 fuer
//! Bitcoin, Marcus et al. 2018 fuer Ethereum). Billig ist das, wenn alle
//! Identitaeten des Angreifers aus wenigen Netzen kommen: viele Adressen in
//! einem gemieteten Bereich kosten fast nichts, Adressen in vielen
//! verschiedenen Netzen kosten. Deshalb zaehlt der Knoten seine
//! **ausgehenden** Verbindungen je Netzgruppe und haelt je Gruppe nur wenige
//! ([`crate::limits::MAX_AUSGEHEND_JE_GRUPPE`]).
//!
//! # Was eine Gruppe ist
//!
//! - Steht die Adresse in der AS-Zuordnung des Betreibers ([`AsZuordnung`],
//!   Schalter `--asmap` des Knotens), ist die Gruppe das autonome System
//!   (AS). Das ist die bessere Grenze: Ein grosser Anbieter hat viele /16,
//!   aber nur ein AS (Tran et al. 2020 zeigen, wie wenige AS einen
//!   Grossteil der Knoten tragen).
//! - Sonst IPv4 /16 und IPv6 /32, wie Bitcoin Core ohne asmap.
//! - **Keine Gruppe** (also nicht gezaehlt) haben Rueckschleife, private
//!   Netze (RFC 1918, fc00::/7), link-lokale, unspezifische Adressen und
//!   Wege ueber ein Relais (`/p2p-circuit`; die IP darin ist die des
//!   Relais, nicht die der Gegenstelle). Ein Angreifer im Internet kann
//!   keine private Adresse vorweisen; ein Knoten im eigenen LAN ist kein
//!   Weg, viele Identitaeten billig zu machen. Und die Proben dieses
//!   Projekts laufen in privaten Netzen von Docker.
//!
//! ⚑ **Die AS-Zuordnung geht vor die Ausnahme der privaten Netze.** Wer in
//! seiner Datei einen privaten Bereich einem AS zuordnet, meint es so; die
//! Netzlaeufe im Behaelter nutzen das, um die Grenze ueberhaupt zeigen zu
//! koennen.
//!
//! # Das Format der AS-Zuordnung
//!
//! Eine Zeile je Praefix, `<praefix>/<laenge> <AS-Nummer>`, wie die Tabelle
//! von bgp.tools (`table.txt`); `AS` vor der Nummer ist erlaubt, `#`
//! beginnt einen Kommentar. Bei mehreren passenden Praefixen gilt das
//! laengste. Die Datei bringt der Betreiber mit; der Knoten laedt nichts
//! nach. Ohne Datei gilt /16 und /32.
//!
//! Ganzzahlig, wie alles in dieser Kiste.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

use libp2p::multiaddr::Protocol;
use libp2p::Multiaddr;

/// Die Netzgruppe einer Gegenstelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Netzgruppe {
    /// Ein autonomes System aus der Zuordnung des Betreibers.
    As(u32),
    /// IPv4 /16: die ersten zwei Oktette.
    V4([u8; 2]),
    /// IPv6 /32: die ersten vier Bytes.
    V6([u8; 4]),
}

impl std::fmt::Display for Netzgruppe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Netzgruppe::As(n) => write!(f, "AS{n}"),
            Netzgruppe::V4([a, b]) => write!(f, "{a}.{b}.0.0/16"),
            Netzgruppe::V6([a, b, c, d]) => write!(f, "{:x}:{:x}::/32", u16::from_be_bytes([*a, *b]), u16::from_be_bytes([*c, *d])),
        }
    }
}

/// Die AS-Zuordnung des Betreibers: Praefix zu AS-Nummer.
///
/// Gespeichert je Praefixlaenge als Tabelle vom maskierten Praefix zur
/// Nummer; eine Abfrage geht die Laengen von lang nach kurz durch, hoechstens
/// 33 (IPv4) oder 129 (IPv6) Nachschlagungen. Eine volle Tabelle des Internets
/// (rund eine Million Praefixe) belegt einige zehn MiB.
#[derive(Debug, Default, Clone)]
pub struct AsZuordnung {
    v4: Vec<(u8, HashMap<u32, u32>)>,
    v6: Vec<(u8, HashMap<u128, u32>)>,
    eintraege: usize,
}

fn maske_v4(laenge: u8) -> u32 {
    if laenge == 0 { 0 } else { u32::MAX << (32 - u32::from(laenge)) }
}

fn maske_v6(laenge: u8) -> u128 {
    if laenge == 0 { 0 } else { u128::MAX << (128 - u32::from(laenge)) }
}

impl AsZuordnung {
    /// Liest den Text einer Zuordnung. Ein Fehler nennt die Zeile.
    pub fn lesen(text: &str) -> Result<Self, String> {
        let mut v4: HashMap<u8, HashMap<u32, u32>> = HashMap::new();
        let mut v6: HashMap<u8, HashMap<u128, u32>> = HashMap::new();
        let mut eintraege = 0usize;
        for (nr, roh) in text.lines().enumerate() {
            let zeile = roh.split('#').next().unwrap_or("").trim();
            if zeile.is_empty() {
                continue;
            }
            let fehler = |was: &str| format!("Zeile {}: {was}: {roh}", nr + 1);
            let mut teile = zeile.split_whitespace();
            let (Some(praefix), Some(nummer), None) = (teile.next(), teile.next(), teile.next()) else {
                return Err(fehler("erwartet `<praefix>/<laenge> <AS-Nummer>`"));
            };
            let nummer = nummer.trim_start_matches("AS").trim_start_matches("as");
            let nummer: u32 = nummer.parse().map_err(|_| fehler("AS-Nummer"))?;
            let (ip, laenge) = praefix.split_once('/').ok_or_else(|| fehler("Praefix ohne /laenge"))?;
            let laenge: u8 = laenge.parse().map_err(|_| fehler("Praefixlaenge"))?;
            match ip.parse::<IpAddr>().map_err(|_| fehler("Adresse"))? {
                IpAddr::V4(a) => {
                    if laenge > 32 {
                        return Err(fehler("Praefixlaenge ueber 32"));
                    }
                    v4.entry(laenge).or_default().insert(u32::from(a) & maske_v4(laenge), nummer);
                }
                IpAddr::V6(a) => {
                    if laenge > 128 {
                        return Err(fehler("Praefixlaenge ueber 128"));
                    }
                    v6.entry(laenge).or_default().insert(u128::from(a) & maske_v6(laenge), nummer);
                }
            }
            eintraege += 1;
        }
        let mut v4: Vec<_> = v4.into_iter().collect();
        let mut v6: Vec<_> = v6.into_iter().collect();
        // Laengste Praefixe zuerst.
        v4.sort_by_key(|e| std::cmp::Reverse(e.0));
        v6.sort_by_key(|e| std::cmp::Reverse(e.0));
        Ok(Self { v4, v6, eintraege })
    }

    /// Liest eine Datei.
    pub fn aus_datei(pfad: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
        Self::lesen(&text).map_err(|e| format!("{}: {e}", pfad.display()))
    }

    /// Wie viele Praefixe die Zuordnung kennt.
    pub fn eintraege(&self) -> usize {
        self.eintraege
    }

    /// Das AS einer Adresse, falls ein Praefix passt (das laengste gilt).
    pub fn as_von(&self, ip: IpAddr) -> Option<u32> {
        match ip {
            IpAddr::V4(a) => self.as_von_v4(a),
            IpAddr::V6(a) => match a.to_ipv4_mapped() {
                Some(v4) => self.as_von_v4(v4),
                None => {
                    let z = u128::from(a);
                    self.v6.iter().find_map(|(l, t)| t.get(&(z & maske_v6(*l))).copied())
                }
            },
        }
    }

    fn as_von_v4(&self, a: Ipv4Addr) -> Option<u32> {
        let z = u32::from(a);
        self.v4.iter().find_map(|(l, t)| t.get(&(z & maske_v4(*l))).copied())
    }
}

/// Die IP einer Multiaddr, wenn sie direkt erreicht wird (nicht ueber ein
/// Relais).
fn direkte_ip(addr: &Multiaddr) -> Option<IpAddr> {
    if addr.iter().any(|p| matches!(p, Protocol::P2pCircuit)) {
        return None;
    }
    addr.iter().find_map(|p| match p {
        Protocol::Ip4(ip) => Some(IpAddr::V4(ip)),
        Protocol::Ip6(ip) => Some(match ip.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(ip),
        }),
        _ => None,
    })
}

fn nicht_gezaehlt_v4(ip: Ipv4Addr) -> bool {
    ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_unspecified() || ip.is_broadcast()
}

fn nicht_gezaehlt_v6(ip: Ipv6Addr) -> bool {
    let erstes = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || (erstes & 0xfe00) == 0xfc00 // eindeutig lokal, fc00::/7
        || (erstes & 0xffc0) == 0xfe80 // link-lokal, fe80::/10
}

/// Die Netzgruppe einer Adresse; `None` heisst "nicht zaehlen".
pub fn netzgruppe(addr: &Multiaddr, zuordnung: Option<&AsZuordnung>) -> Option<Netzgruppe> {
    let ip = direkte_ip(addr)?;
    if let Some(n) = zuordnung.and_then(|z| z.as_von(ip)) {
        return Some(Netzgruppe::As(n));
    }
    match ip {
        IpAddr::V4(a) => {
            if nicht_gezaehlt_v4(a) {
                return None;
            }
            let o = a.octets();
            Some(Netzgruppe::V4([o[0], o[1]]))
        }
        IpAddr::V6(a) => {
            if nicht_gezaehlt_v6(a) {
                return None;
            }
            let o = a.octets();
            Some(Netzgruppe::V6([o[0], o[1], o[2], o[3]]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(t: &str) -> Multiaddr {
        t.parse().unwrap()
    }

    #[test]
    fn ohne_zuordnung_gilt_v4_16_und_v6_32() {
        assert_eq!(netzgruppe(&a("/ip4/203.0.113.7/tcp/4150"), None), Some(Netzgruppe::V4([203, 0])));
        assert_eq!(
            netzgruppe(&a("/ip4/203.0.113.7/tcp/4150"), None),
            netzgruppe(&a("/ip4/203.0.200.1/udp/4150/quic-v1"), None),
            "derselbe /16"
        );
        assert_ne!(netzgruppe(&a("/ip4/203.0.113.7/tcp/1"), None), netzgruppe(&a("/ip4/203.1.113.7/tcp/1"), None));
        assert_eq!(
            netzgruppe(&a("/ip6/2001:db8:1:2::5/tcp/4150"), None),
            Some(Netzgruppe::V6([0x20, 0x01, 0x0d, 0xb8]))
        );
    }

    #[test]
    fn private_rueckschleife_und_relais_zaehlen_nicht() {
        for t in [
            "/ip4/127.0.0.1/tcp/1",
            "/ip4/10.1.2.3/tcp/1",
            "/ip4/172.18.0.2/tcp/1",
            "/ip4/192.168.1.5/tcp/1",
            "/ip4/169.254.1.1/tcp/1",
            "/ip6/::1/tcp/1",
            "/ip6/fd00::1/tcp/1",
            "/ip6/fe80::1/tcp/1",
            "/ip4/198.51.100.1/tcp/4001/p2p/12D3KooWHtF9SZPRHUrToZs7dXKZLgXZENTckskMgy5XhKEUPzwW/p2p-circuit",
            "/dns4/einstieg/tcp/4150",
        ] {
            assert_eq!(netzgruppe(&a(t), None), None, "{t}");
        }
    }

    #[test]
    fn ipv4_in_ipv6_zaehlt_als_ipv4() {
        assert_eq!(netzgruppe(&a("/ip6/::ffff:203.0.113.7/tcp/1"), None), Some(Netzgruppe::V4([203, 0])));
    }

    #[test]
    fn die_zuordnung_nimmt_das_laengste_praefix() {
        let z = AsZuordnung::lesen(
            "# Kommentar\n\
             203.0.0.0/8 AS100\n\
             203.0.113.0/24 200   # laenger\n\
             2001:db8::/32 300\n\
             2001:db8:1::/48 AS400\n",
        )
        .unwrap();
        assert_eq!(z.eintraege(), 4);
        assert_eq!(z.as_von("203.0.113.9".parse().unwrap()), Some(200));
        assert_eq!(z.as_von("203.9.9.9".parse().unwrap()), Some(100));
        assert_eq!(z.as_von("204.0.0.1".parse().unwrap()), None);
        assert_eq!(z.as_von("2001:db8:1::7".parse().unwrap()), Some(400));
        assert_eq!(z.as_von("2001:db8:2::7".parse().unwrap()), Some(300));
        assert_eq!(z.as_von("::ffff:203.0.113.1".parse().unwrap()), Some(200));
        // Zwei verschiedene /16 im selben AS sind eine Gruppe.
        assert_eq!(
            netzgruppe(&a("/ip4/203.1.0.1/tcp/1"), Some(&z)),
            netzgruppe(&a("/ip4/203.200.0.1/tcp/1"), Some(&z))
        );
    }

    #[test]
    fn die_zuordnung_geht_vor_die_ausnahme_privater_netze() {
        let z = AsZuordnung::lesen("172.18.0.0/16 65001\n").unwrap();
        assert_eq!(netzgruppe(&a("/ip4/172.18.0.5/tcp/1"), Some(&z)), Some(Netzgruppe::As(65001)));
        assert_eq!(netzgruppe(&a("/ip4/172.19.0.5/tcp/1"), Some(&z)), None);
    }

    #[test]
    fn eine_kaputte_zeile_wird_mit_nummer_gemeldet() {
        for (text, teil) in [
            ("1.2.3.0/24\n", "erwartet"),
            ("1.2.3.0 5\n", "ohne /laenge"),
            ("1.2.3.0/33 5\n", "ueber 32"),
            ("1.2.3.0/24 x\n", "AS-Nummer"),
            ("\n\nkein/8 1\n", "Zeile 3"),
        ] {
            let f = AsZuordnung::lesen(text).unwrap_err();
            assert!(f.contains(teil), "{text:?}: {f}");
        }
    }

    #[test]
    fn eine_nullroute_passt_auf_alles() {
        let z = AsZuordnung::lesen("0.0.0.0/0 1\n::/0 2\n").unwrap();
        assert_eq!(z.as_von("8.8.8.8".parse().unwrap()), Some(1));
        assert_eq!(z.as_von("2001::1".parse().unwrap()), Some(2));
    }
}
