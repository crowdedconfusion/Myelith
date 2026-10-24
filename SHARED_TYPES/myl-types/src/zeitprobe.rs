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
//! Vier Bytes Länge in Little-Endian, dann die Nutzlast, wie jede Antwort
//! einer Shardtür. Eine **Anfrage** trägt davor den Ausweis der Tür
//! ([`rahmen_senden_mit_ausweis`]). Die Nutzlast der Probe ist [`MARKE`] und
//! die Zahl der Positionen; die Antwort [`MARKE`] und der Abdruck des letzten
//! Ergebnisses.
//!
//! # ⚑ Der Ausweis (Fund 611, 2026-10-23)
//!
//! Die Tür eines Shards nahm Inferenzschritte, Zeitproben und
//! Trainingsaufträge von jedem an, der sie erreichte. Sie hört heute auf
//! der Rückschleife, aber **jeder lokale Prozess kann wählen**, und den Port
//! zeigt `lsof`: Ein fremdes Programm auf derselben Maschine belegte die
//! Tür, liess auf Kosten des Miners rechnen oder veränderte den
//! Trainingszustand, den der Miner danach unterschreibt. Dieselbe Lage, für
//! die `crate::ortsleitung` einen Ausweis hat, und dieselbe Bauart: 32
//! Bytes, vom Knoten gezogen und mit `0600` abgelegt, **vor** der Länge, in
//! gleichbleibender Zeit verglichen. Wer ihn nicht hat, bringt die Tür
//! nicht dazu, Speicher zu belegen.

use std::io::{Read, Write};

use subtle::ConstantTimeEq;
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

/// Länge des Ausweises einer Shardtür.
pub const AUSWEIS_LEN: usize = 32;

/// Schreibt eine Anfrage an eine Shardtür: Ausweis, dann den Rahmen.
pub fn rahmen_senden_mit_ausweis(
    strom: &mut impl Write,
    ausweis: &[u8; AUSWEIS_LEN],
    nutzlast: &[u8],
    hoechstens: usize,
) -> Result<(), String> {
    strom.write_all(ausweis).map_err(|e| e.to_string())?;
    rahmen_senden(strom, nutzlast, hoechstens)
}

/// Liest eine Anfrage an eine Shardtür. ⚑ **Erst der Ausweis**, in
/// gleichbleibender Zeit verglichen, und erst danach die Länge.
pub fn rahmen_empfangen_mit_ausweis(
    strom: &mut impl Read,
    ausweis: &[u8; AUSWEIS_LEN],
    hoechstens: usize,
) -> Result<Vec<u8>, String> {
    let mut gesehen = [0u8; AUSWEIS_LEN];
    strom.read_exact(&mut gesehen).map_err(|e| e.to_string())?;
    if gesehen.ct_eq(ausweis).unwrap_u8() != 1 {
        return Err("falscher Ausweis".to_string());
    }
    rahmen_empfangen(strom, hoechstens)
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
pub fn fragen(
    adresse: SocketAddr,
    ausweis: &[u8; AUSWEIS_LEN],
    positionen: u32,
    frist: Duration,
) -> Result<[u8; 32], String> {
    let mut strom = TcpStream::connect_timeout(&adresse, frist).map_err(|e| format!("{adresse}: {e}"))?;
    strom.set_read_timeout(Some(frist)).map_err(|e| e.to_string())?;
    strom.set_write_timeout(Some(frist)).map_err(|e| e.to_string())?;
    rahmen_senden_mit_ausweis(&mut strom, ausweis, &anfrage(positionen), 64)?;
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
            let roh = rahmen_empfangen_mit_ausweis(&mut s, &[5; AUSWEIS_LEN], 64).expect("Rahmen");
            let n = als_anfrage(&roh).expect("Probe");
            rahmen_senden(&mut s, &antwort([n as u8; 32]), 64).expect("senden");
        });
        assert_eq!(fragen(adresse, &[5; AUSWEIS_LEN], 3, Duration::from_secs(5)), Ok([3; 32]));
        faden.join().expect("Faden");
    }

    /// ⚑ **Ohne den richtigen Ausweis liest die Tür nicht einmal die
    /// Länge** (Fund 611): Ein falscher wird abgelehnt, bevor die
    /// angekündigten vier Gigabyte auch nur geprüft werden.
    #[test]
    fn ein_falscher_ausweis_wird_vor_der_laenge_abgelehnt() {
        let mut roh = vec![6u8; AUSWEIS_LEN];
        roh.extend_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            rahmen_empfangen_mit_ausweis(&mut &roh[..], &[5; AUSWEIS_LEN], 64),
            Err("falscher Ausweis".to_string())
        );
        let mut echt = Vec::new();
        rahmen_senden_mit_ausweis(&mut echt, &[5; AUSWEIS_LEN], b"probe", 64).expect("senden");
        assert_eq!(rahmen_empfangen_mit_ausweis(&mut &echt[..], &[5; AUSWEIS_LEN], 64), Ok(b"probe".to_vec()));
    }

    #[test]
    fn eine_zu_grosse_laengenangabe_wird_abgewiesen() {
        let roh = u32::MAX.to_le_bytes();
        assert!(rahmen_empfangen(&mut &roh[..], 64).is_err());
    }
}
