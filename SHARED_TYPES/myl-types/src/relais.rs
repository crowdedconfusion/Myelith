//! **Die Relaisleitung**: zwischen einem Knoten und einem Koordinator auf
//! derselben Maschine (2026-10-26).
//!
//! # ⚑ Warum eine eigene Leitung
//!
//! Ein Pod rechnet über die Shards seiner Mitglieder, und jedes Mitglied
//! ist ein eigener Knoten. Der Koordinator, der die Pipeline fährt, braucht
//! deshalb den Weg über die Knoten: Er fragt seinen Knoten, der Knoten
//! reicht über das Netz an den Knoten des Mitglieds, und der an seinen
//! Shard. **Der Knoten rechnet dabei nichts**: Er kennt die Kette und das
//! Netz, die Rechenlaufzeit bleibt im Koordinator und in den Shards.
//!
//! # ⚑ Was über die Leitung geht
//!
//! - [`Relaisfrage::Podbild`]: die Pods der laufenden Epoche aus der Kette,
//!   je Mitglied Netzadresse und Ebenenbereich in Shardreihenfolge.
//! - [`Relaisfrage::Shard`]: eine Shardanfrage in Borsh an ein Mitglied;
//!   zurück kommt die Antwort des Shards, unverändert.
//!
//! Rahmen mit Ausweis wie die Ortsleitung
//! ([`crate::zeitprobe::rahmen_senden_mit_ausweis`]); der Ausweis liegt beim
//! Knoten in einer Datei mit Rechten `0600`, 64 Hexzeichen.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::latency_attest::PeerIdBytes;

/// Die Größengrenze einer Nachricht auf der Leitung: dieselbe wie zwischen
/// zwei Knoten, denn jede Shardanfrage geht danach über das Netz.
pub const MAX_RELAIS_BYTES: usize = crate::protocol::MAX_ANFRAGE_BYTES;

/// Was ein Koordinator seinen Knoten fragt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Relaisfrage {
    /// Welche Pods hat die laufende Epoche?
    Podbild,
    /// Reiche diese Shardanfrage an das Mitglied mit dieser Netzadresse.
    Shard {
        /// Die Netzadresse des Mitglieds, wie in seiner Anmeldung.
        an: PeerIdBytes,
        /// Die Shardanfrage in Borsh.
        anfrage: Vec<u8>,
    },
    /// ⚑ **Reiche dieses unterschriebene PoI-Bündel ein** (Borsh eines
    /// `PoIBundle`). Einreichen darf nur der Koordinator des Pods laut Kette,
    /// also die erste Position; der Knoten gibt das Bündel an deren Knoten,
    /// wenn er es nicht selbst ist. Angehängt.
    BuendelEinreichen {
        /// Das Bündel in Borsh.
        buendel: Vec<u8>,
    },
    /// ⚑ **Was die Kette über ein Konto weiß, und womit man für sie
    /// unterschreibt** (2026-10-26): Nonce, Guthaben, Netzwert, dazu das
    /// Konto des Knotens selbst (der Betreiber einer Tür). Für ein Wallet,
    /// das eine Transaktion bauen will. Angehängt.
    Konto {
        /// Welches Konto.
        adresse: crate::ids::Address,
    },
    /// Eine unterschriebene Kettentransaktion (Borsh); der Knoten prüft
    /// Unterschrift und Netz und verbreitet sie. Angehängt.
    Transaktion {
        /// Die Transaktion in Borsh.
        tx: Vec<u8>,
    },
}

/// Was der Knoten darauf antwortet.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Relaisantwort {
    /// Die Pods der laufenden Epoche.
    Podbild(Podbild),
    /// Die Antwort des Shards in Borsh.
    Shard(Vec<u8>),
    /// Nicht weitergereicht oder keine Antwort; der Grund.
    Fehler(String),
    /// Erledigt, mit einer Zeile, was geschah. Angehängt.
    Erledigt(String),
    /// Die Antwort auf [`Relaisfrage::Konto`]. Angehängt.
    Konto(Kontostand),
}

/// Ein Konto, wie der Knoten es sieht, und was eine Unterschrift braucht.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Kontostand {
    /// Die Höhe, auf der geantwortet wurde.
    pub hoehe: u64,
    /// Die laufende Epoche.
    pub epoche: u64,
    pub guthaben: u64,
    /// Die nächste Nonce: die im Zustand.
    pub nonce: u64,
    /// Inferenz-Credits des Kontos.
    pub credits: u64,
    /// Der Netzwert, gegen den Transaktionen unterschrieben werden.
    pub netzwert: crate::hash::Hash,
    /// Das Konto dieses Knotens (Empfänger, wenn er eine Tür betreibt).
    pub betreiber: crate::ids::Address,
    /// ⚑ **Gebühren** (2026-10-27): der Grundpreis je Byte des nächsten
    /// Blocks, ...
    pub grundpreis_je_byte: u64,
    /// ... ob das Konto schon im Zustand steht (an ein neues zahlt der
    /// Absender den Aufschlag) ...
    pub bekannt: bool,
    /// ... und wie hoch der Aufschlag in diesem Netz ist.
    pub aufschlag_konto: u64,
}

/// Die Pods einer Epoche, wie die Kette sie zuteilt.
#[derive(Debug, Clone, PartialEq, Eq, Default, BorshSerialize, BorshDeserialize)]
pub struct Podbild {
    /// Die Epoche.
    pub epoche: u64,
    /// Die Höhe, auf der der Knoten geantwortet hat.
    pub hoehe: u64,
    /// Wie viele Blöcke eine Epoche hat, aus der Kette des Knotens.
    pub bloecke_je_epoche: u64,
    /// Die Pods, nach Index.
    pub pods: Vec<Podeintrag>,
}

/// Ein Pod der Epoche.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Podeintrag {
    /// Sein Index in der Zuteilung.
    pub pod_index: u32,
    /// Seine Kennung (`pod_kennung(epoche, pod_index)`).
    pub kennung: crate::ids::PodId,
    /// Die rechnenden Mitglieder in Shardreihenfolge.
    pub mitglieder: Vec<Podmitglied>,
}

/// Ein rechnendes Mitglied.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Podmitglied {
    /// Seine Kennung als Miner.
    pub miner: crate::ids::MinerId,
    /// Seine Netzadresse.
    pub netzadresse: PeerIdBytes,
    /// Der Ebenenbereich, den die Kette ihm zuteilt, `von..bis`.
    pub von: u32,
    pub bis: u32,
}

impl Relaisfrage {
    pub fn als_bytes(&self) -> Option<Vec<u8>> {
        borsh::to_vec(self).ok()
    }

    /// Aus Bytes zurück; `None` bei allem, was nicht passt, auch bei
    /// Anhängseln.
    pub fn aus_bytes(daten: &[u8]) -> Option<Self> {
        let mut rest = daten;
        Self::deserialize(&mut rest).ok().filter(|_| rest.is_empty())
    }
}

impl Relaisantwort {
    pub fn als_bytes(&self) -> Option<Vec<u8>> {
        borsh::to_vec(self).ok()
    }

    pub fn aus_bytes(daten: &[u8]) -> Option<Self> {
        let mut rest = daten;
        Self::deserialize(&mut rest).ok().filter(|_| rest.is_empty())
    }
}

/// Liest einen Ausweis aus einer Datei: 64 Hexzeichen, Leerraum zählt nicht.
pub fn ausweis_lesen(pfad: &std::path::Path) -> std::io::Result<[u8; crate::zeitprobe::AUSWEIS_LEN]> {
    let text = std::fs::read_to_string(pfad)?;
    let hex = text.trim();
    let falsch = || std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}: kein Ausweis", pfad.display()));
    if hex.len() != crate::zeitprobe::AUSWEIS_LEN * 2 {
        return Err(falsch());
    }
    let mut a = [0u8; crate::zeitprobe::AUSWEIS_LEN];
    for (i, b) in a.iter_mut().enumerate() {
        *b = u8::from_str_radix(hex.get(i * 2..i * 2 + 2).ok_or_else(falsch)?, 16).map_err(|_| falsch())?;
    }
    Ok(a)
}

/// **Stellt eine Frage über die Leitung** und wartet auf die Antwort.
///
/// Blockierend, für einen Koordinator in seinem eigenen Faden.
pub fn fragen(
    adresse: std::net::SocketAddr,
    ausweis: &[u8; crate::zeitprobe::AUSWEIS_LEN],
    frage: &Relaisfrage,
    frist: std::time::Duration,
) -> Result<Relaisantwort, String> {
    let mut strom = std::net::TcpStream::connect_timeout(&adresse, frist)
        .map_err(|e| format!("Relais {adresse} nicht erreichbar: {e}"))?;
    strom.set_read_timeout(Some(frist)).map_err(|e| e.to_string())?;
    strom.set_write_timeout(Some(frist)).map_err(|e| e.to_string())?;
    let nutzlast = frage.als_bytes().ok_or("Frage nicht serialisierbar")?;
    crate::zeitprobe::rahmen_senden_mit_ausweis(&mut strom, ausweis, &nutzlast, MAX_RELAIS_BYTES)?;
    let roh = crate::zeitprobe::rahmen_empfangen(&mut strom, MAX_RELAIS_BYTES)?;
    Relaisantwort::aus_bytes(&roh).ok_or_else(|| "Relais antwortete unlesbar".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragen_und_antworten_gehen_hin_und_zurueck() {
        let f = Relaisfrage::Shard { an: PeerIdBytes([7; 32]), anfrage: vec![1, 2, 3] };
        assert_eq!(Relaisfrage::aus_bytes(&f.als_bytes().unwrap()), Some(f));
        let bild = Podbild {
            epoche: 3,
            hoehe: 5400,
            bloecke_je_epoche: 1800,
            pods: vec![Podeintrag {
                pod_index: 0,
                kennung: crate::miner::pod_kennung(3, 0),
                mitglieder: vec![Podmitglied { miner: crate::ids::MinerId::new([1; 32]), netzadresse: PeerIdBytes([2; 32]), von: 0, bis: 14 }],
            }],
        };
        let a = Relaisantwort::Podbild(bild);
        assert_eq!(Relaisantwort::aus_bytes(&a.als_bytes().unwrap()), Some(a));
        let mut mit_rest = Relaisfrage::Podbild.als_bytes().unwrap();
        mit_rest.push(0);
        assert_eq!(Relaisfrage::aus_bytes(&mit_rest), None, "ein Anhängsel wurde angenommen");
    }

    #[test]
    fn ein_ausweis_braucht_64_hexzeichen() {
        let d = std::env::temp_dir().join(format!("myl-relaisausweis-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("a");
        std::fs::write(&p, format!("{}\n", "0f".repeat(32))).unwrap();
        assert_eq!(ausweis_lesen(&p).unwrap(), [0x0f; 32]);
        std::fs::write(&p, "0f0f").unwrap();
        assert!(ausweis_lesen(&p).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
