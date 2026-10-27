//! **Die Relaisleitung des Knotens** (2026-10-26): Ein
//! Koordinator auf derselben Maschine fragt hier nach den Pods der Epoche
//! und reicht Shardanfragen an die Mitglieder.
//!
//! # ⚑ Warum ein eigener Faden mit blockierendem TCP
//!
//! Die Leitung ist dünn und lokal; jede Frage geht über einen Kanal in die
//! Schleife des Knotens, denn nur die kennt Kette und Netz. Die Antwort
//! kommt über einen Rückkanal mit Frist. So hält die Leitung keinen Zustand
//! und trifft keine Entscheidung; beides steht im Knoten.
//!
//! # ⚑ Der Ausweis
//!
//! Beim Start gewürfelt und in eine Datei mit Rechten `0600` geschrieben
//! (64 Hexzeichen); wer ihn nicht vorweist, bekommt nichts. Dieselbe Regel
//! wie bei der Ortsleitung (Fund 611).

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use myl_types::relais::{Relaisantwort, Relaisfrage, MAX_RELAIS_BYTES};
use myl_types::zeitprobe::AUSWEIS_LEN;

/// Eine Frage an die Schleife des Knotens und ihr Rückweg.
pub type Relaisauftrag = (Relaisfrage, std::sync::mpsc::Sender<Relaisantwort>);

/// Wie lange die Leitung auf die Schleife wartet: länger als jede
/// Shardanfrage über das Netz dauern darf.
pub const FRIST: Duration = Duration::from_secs(150);

/// **Öffnet die Leitung** und gibt die vergebene Adresse zurück.
pub fn starten(
    adresse: SocketAddr,
    ausweisdatei: &Path,
    auftraege: tokio::sync::mpsc::Sender<Relaisauftrag>,
) -> std::io::Result<SocketAddr> {
    let mut ausweis = [0u8; AUSWEIS_LEN];
    getrandom::fill(&mut ausweis).map_err(|e| std::io::Error::other(e.to_string()))?;
    let text: String = ausweis.iter().map(|b| format!("{b:02x}")).collect();
    myl_types::geheimdatei::schreiben(ausweisdatei, format!("{text}\n").as_bytes())?;
    let lauscher = std::net::TcpListener::bind(adresse)?;
    let wo = lauscher.local_addr()?;
    std::thread::Builder::new().name("relaisleitung".into()).spawn(move || {
        for strom in lauscher.incoming().flatten() {
            let a = auftraege.clone();
            let _ = std::thread::Builder::new().name("relaisfrage".into()).spawn(move || bedienen(strom, &ausweis, &a));
        }
    })?;
    Ok(wo)
}

fn bedienen(
    mut strom: std::net::TcpStream,
    ausweis: &[u8; AUSWEIS_LEN],
    auftraege: &tokio::sync::mpsc::Sender<Relaisauftrag>,
) {
    let _ = strom.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = strom.set_write_timeout(Some(Duration::from_secs(30)));
    let antwort = match myl_types::zeitprobe::rahmen_empfangen_mit_ausweis(&mut strom, ausweis, MAX_RELAIS_BYTES)
        .ok()
        .and_then(|roh| Relaisfrage::aus_bytes(&roh))
    {
        // Ohne Ausweis oder unlesbar: keine Antwort, die etwas verrät.
        None => return,
        Some(frage) => {
            let (rueck, antwort) = std::sync::mpsc::channel();
            if auftraege.blocking_send((frage, rueck)).is_err() {
                Relaisantwort::Fehler("der Knoten nimmt keine Fragen mehr".into())
            } else {
                antwort.recv_timeout(FRIST).unwrap_or_else(|_| Relaisantwort::Fehler("keine Antwort in der Frist".into()))
            }
        }
    };
    if let Some(bytes) = antwort.als_bytes() {
        let _ = myl_types::zeitprobe::rahmen_senden(&mut strom, &bytes, MAX_RELAIS_BYTES);
    }
}

/// **Reicht eine Shardanfrage an den eigenen Shard**, blockierend: Rahmen
/// mit Ausweis hin, Rahmen zurück. Für das Mitglied, das gefragt wird, und
/// für den Torknoten, der selbst Mitglied ist.
pub fn an_den_eigenen_shard(adresse: SocketAddr, ausweis: &[u8; AUSWEIS_LEN], anfrage: &[u8]) -> Result<Vec<u8>, String> {
    let frist = Duration::from_secs(120);
    let mut strom = std::net::TcpStream::connect_timeout(&adresse, Duration::from_secs(5))
        .map_err(|e| format!("eigener Shard unter {adresse} nicht erreichbar: {e}"))?;
    strom.set_read_timeout(Some(frist)).map_err(|e| e.to_string())?;
    strom.set_write_timeout(Some(frist)).map_err(|e| e.to_string())?;
    myl_types::zeitprobe::rahmen_senden_mit_ausweis(&mut strom, ausweis, anfrage, MAX_RELAIS_BYTES)?;
    myl_types::zeitprobe::rahmen_empfangen(&mut strom, MAX_RELAIS_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Ohne Ausweis keine Antwort**, mit Ausweis kommt die Frage bei der
    /// Schleife an und die Antwort zurück.
    #[test]
    fn nur_mit_ausweis_kommt_eine_frage_durch() {
        let d = std::env::temp_dir().join(format!("myl-relais-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let datei = d.join("relais.ausweis");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Relaisauftrag>(4);
        let wo = starten("127.0.0.1:0".parse().unwrap(), &datei, tx).expect("starten");
        // Die „Schleife": beantwortet jede Frage mit einem leeren Podbild.
        std::thread::spawn(move || {
            rt.block_on(async move {
                while let Some((_, rueck)) = rx.recv().await {
                    let _ = rueck.send(Relaisantwort::Podbild(Default::default()));
                }
            })
        });
        let ausweis = myl_types::relais::ausweis_lesen(&datei).expect("Ausweis");
        let a = myl_types::relais::fragen(wo, &ausweis, &Relaisfrage::Podbild, Duration::from_secs(5)).expect("Antwort");
        assert_eq!(a, Relaisantwort::Podbild(Default::default()));
        // ⛔️ Gegenprobe: ein falscher Ausweis bekommt nichts.
        let falsch = [9u8; AUSWEIS_LEN];
        assert!(myl_types::relais::fragen(wo, &falsch, &Relaisfrage::Podbild, Duration::from_secs(2)).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&datei).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
