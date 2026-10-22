//! **Die Zeitprobe an der Tür eines Shards** (Stufe 4 des Dispatchers,
//! Frist, 2026-10-22).
//!
//! # ⚑ Warum hier und nicht im Shardprotokoll
//!
//! Der Knoten stellt seinem eigenen Shard eine Zeitprobe, wenn ein anderes
//! Mitglied ihn misst, und er bindet `myl-pod` nicht ein: Ein Shard läuft in
//! einem eigenen Prozess, und der Knoten zieht dessen Laufzeit nicht nach.
//! Die Probe steht deshalb hier, **einmal**, mit Marke, Rahmen und Klient;
//! die Tür des Shards erkennt sie an der Marke vor jeder anderen Anfrage.
//!
//! # Der Rahmen
//!
//! Vier Bytes Länge in Little-Endian, dann die Nutzlast, wie jede Anfrage an
//! eine Shardtür. Die Nutzlast der Probe ist [`MARKE`] und die Zahl der
//! Positionen; die Antwort [`MARKE`] und der Abdruck des letzten Ergebnisses.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// Woran die Tür eine Zeitprobe erkennt.
pub const MARKE: &[u8] = b"MYELITH_ZEITPROBE_v1";

/// Die Nutzlast einer Probe über `positionen` Positionen.
pub fn anfrage(positionen: u32) -> Vec<u8> {
    [MARKE, &positionen.to_le_bytes()].concat()
}

/// Ist das eine Probe? Dann die Zahl der Positionen.
pub fn als_anfrage(roh: &[u8]) -> Option<u32> {
    let zahl: [u8; 4] = roh.strip_prefix(MARKE)?.try_into().ok()?;
    Some(u32::from_le_bytes(zahl))
}

/// Die Nutzlast der Antwort.
pub fn antwort(abdruck: [u8; 32]) -> Vec<u8> {
    [MARKE, &abdruck].concat()
}

/// Ist das die Antwort auf eine Probe? Dann der Abdruck.
pub fn als_antwort(roh: &[u8]) -> Option<[u8; 32]> {
    roh.strip_prefix(MARKE)?.try_into().ok()
}

/// Schreibt einen Rahmen.
pub fn rahmen_senden(strom: &mut impl Write, nutzlast: &[u8], hoechstens: usize) -> Result<(), String> {
    if nutzlast.len() > hoechstens {
        return Err(format!("Nachricht mit {} Bytes ist zu gross", nutzlast.len()));
    }
    strom.write_all(&(nutzlast.len() as u32).to_le_bytes()).map_err(|e| e.to_string())?;
    strom.write_all(nutzlast).map_err(|e| e.to_string())?;
    strom.flush().map_err(|e| e.to_string())
}

/// Liest einen Rahmen. ⚑ **Die Länge wird vor dem Reservieren geprüft**:
/// Eine angekündigte Länge von vier Gigabyte darf keinen Speicher belegen.
pub fn rahmen_empfangen(strom: &mut impl Read, hoechstens: usize) -> Result<Vec<u8>, String> {
    let mut kopf = [0u8; 4];
    strom.read_exact(&mut kopf).map_err(|e| e.to_string())?;
    let laenge = u32::from_le_bytes(kopf) as usize;
    if laenge > hoechstens {
        return Err(format!("angekuendigte Laenge {laenge} ueberschreitet die Grenze"));
    }
    let mut roh = vec![0u8; laenge];
    strom.read_exact(&mut roh).map_err(|e| e.to_string())?;
    Ok(roh)
}

/// **Stellt dem Shard unter `adresse` eine Probe** und liefert den Abdruck.
/// Die Zeit nimmt, wer fragt.
pub fn fragen(adresse: SocketAddr, positionen: u32, frist: Duration) -> Result<[u8; 32], String> {
    let mut strom = TcpStream::connect_timeout(&adresse, frist).map_err(|e| format!("{adresse}: {e}"))?;
    strom.set_read_timeout(Some(frist)).map_err(|e| e.to_string())?;
    strom.set_write_timeout(Some(frist)).map_err(|e| e.to_string())?;
    rahmen_senden(&mut strom, &anfrage(positionen), 64)?;
    let roh = rahmen_empfangen(&mut strom, 64)?;
    als_antwort(&roh).ok_or_else(|| "die Tuer antwortete nicht mit einer Zeitprobe".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anfrage_und_antwort_tragen_sich_und_nichts_anderes() {
        assert_eq!(als_anfrage(&anfrage(4)), Some(4));
        assert_eq!(als_antwort(&antwort([7; 32])), Some([7; 32]));
        assert_eq!(als_anfrage(&[0, 1, 2]), None);
        assert_eq!(als_anfrage(&antwort([7; 32])), None, "eine Antwort ist keine Anfrage");
        let mut lang = anfrage(4);
        lang.push(0);
        assert_eq!(als_anfrage(&lang), None, "Anhängsel");
    }

    /// Über eine echte Verbindung, mit einer Tür, die die Probe beantwortet.
    #[test]
    fn die_probe_geht_ueber_den_draht() {
        let horcher = std::net::TcpListener::bind("127.0.0.1:0").expect("binden");
        let adresse = horcher.local_addr().expect("Adresse");
        let faden = std::thread::spawn(move || {
            let (mut s, _) = horcher.accept().expect("annehmen");
            let roh = rahmen_empfangen(&mut s, 64).expect("Rahmen");
            let n = als_anfrage(&roh).expect("Probe");
            rahmen_senden(&mut s, &antwort([n as u8; 32]), 64).expect("senden");
        });
        assert_eq!(fragen(adresse, 3, Duration::from_secs(5)), Ok([3; 32]));
        faden.join().expect("Faden");
    }

    #[test]
    fn eine_zu_grosse_laengenangabe_wird_abgewiesen() {
        let roh = u32::MAX.to_le_bytes();
        assert!(rahmen_empfangen(&mut &roh[..], 64).is_err());
    }
}
