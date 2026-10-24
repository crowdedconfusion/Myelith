//! **Eine Frist für die ganze Anfrage**, nicht für jeden Lesevorgang.
//!
//! # 📌 Fund 611 (2026-10-23), der zweite Teil
//!
//! Die Tür des Shards und der Ortsdienst setzten ihre Frist mit
//! `set_read_timeout`. Die gilt **je Lesevorgang**: Wer alle 29 Sekunden
//! ein Byte schickt, wird nie abgelaufen. Beide Dienste bedienen
//! nacheinander, also hielt so eine einzige Verbindung sie für alle fest,
//! und zwar schon vor dem Ausweis: Auch dessen 32 Bytes lassen sich
//! tröpfeln, rund eine Viertelstunde je Verbindung.
//!
//! Dieser Leser setzt vor jedem Lesen die **verbleibende** Zeit bis zu
//! einem festen Ende und meldet `TimedOut`, sobald sie aufgebraucht ist.

use std::io::{self, Read};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// Liest aus einem Strom bis zu einem festen Ende.
pub struct Fristleser<'a> {
    strom: &'a TcpStream,
    ende: Instant,
}

impl<'a> Fristleser<'a> {
    /// Ein Leser, dessen Frist jetzt beginnt.
    pub fn neu(strom: &'a TcpStream, frist: Duration) -> Self {
        Self { strom, ende: Instant::now() + frist }
    }

    /// Beginnt die Frist neu, für den nächsten Rahmen auf derselben
    /// Verbindung.
    pub fn neu_beginnen(&mut self, frist: Duration) {
        self.ende = Instant::now() + frist;
    }
}

impl Read for Fristleser<'_> {
    fn read(&mut self, puffer: &mut [u8]) -> io::Result<usize> {
        let rest = self.ende.saturating_duration_since(Instant::now());
        if rest.is_zero() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "die Frist der Anfrage ist abgelaufen"));
        }
        self.strom.set_read_timeout(Some(rest))?;
        let mut strom = self.strom;
        strom.read(puffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// ⚑ **Wer tröpfelt, läuft ab.** Zwanzig Bytes im Abstand von 50 ms
    /// brauchen eine Sekunde; mit einer Frist von 300 ms für alle zusammen
    /// endet das Lesen nach rund 300 ms. Mit einer Frist je Lesevorgang
    /// käme jedes einzelne Byte rechtzeitig, und alles ginge durch.
    #[test]
    fn wer_troepfelt_laeuft_ab() {
        let horcher = std::net::TcpListener::bind("127.0.0.1:0").expect("binden");
        let adresse = horcher.local_addr().expect("Adresse");
        let faden = std::thread::spawn(move || {
            let mut s = TcpStream::connect(adresse).expect("verbinden");
            for _ in 0..20 {
                if s.write_all(&[7]).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let (strom, _) = horcher.accept().expect("annehmen");
        let beginn = Instant::now();
        let mut puffer = [0u8; 20];
        let ergebnis = Fristleser::neu(&strom, Duration::from_millis(300)).read_exact(&mut puffer);
        let dauer = beginn.elapsed();
        assert!(ergebnis.is_err(), "zwanzig getröpfelte Bytes gingen durch");
        assert!(dauer < Duration::from_millis(800), "erst nach {dauer:?} abgebrochen");
        drop(strom);
        let _ = faden.join();
    }

    /// Wer zügig schickt, kommt durch, und die neu begonnene Frist trägt
    /// den nächsten Rahmen.
    #[test]
    fn wer_zuegig_schickt_kommt_durch() {
        let horcher = std::net::TcpListener::bind("127.0.0.1:0").expect("binden");
        let adresse = horcher.local_addr().expect("Adresse");
        let faden = std::thread::spawn(move || {
            let mut s = TcpStream::connect(adresse).expect("verbinden");
            s.write_all(b"eins").expect("senden");
            std::thread::sleep(Duration::from_millis(250));
            s.write_all(b"zwei").expect("senden");
        });
        let (strom, _) = horcher.accept().expect("annehmen");
        let mut leser = Fristleser::neu(&strom, Duration::from_millis(300));
        let mut puffer = [0u8; 4];
        leser.read_exact(&mut puffer).expect("erster Rahmen");
        assert_eq!(&puffer, b"eins");
        leser.neu_beginnen(Duration::from_millis(300));
        leser.read_exact(&mut puffer).expect("zweiter Rahmen");
        assert_eq!(&puffer, b"zwei");
        faden.join().expect("Faden");
    }
}
