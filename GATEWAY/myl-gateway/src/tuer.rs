//! Der Endpunkt: ein Zuhörer auf `localhost`.
//!
//! # ⚑ Warum dieser Teil so dünn ist
//!
//! Alles, was entscheidet, steht in [`crate::http`] und
//! [`crate::annahme`] als **reine Funktion** und ist einzeln geprüft.
//! Hier bleibt Lesen, Rufen, Schreiben. **Handgeschriebenes HTTP ist
//! gefährlich, wo Zerlegung auf fremde Eingaben trifft**, und genau die
//! Zerlegung fasst dieser Teil nicht an.
//!
//! # ⚑ Nur die Rückschleife, und das ist keine Voreinstellung
//!
//! [`Tuer::binden`] nimmt keine Adresse entgegen, sondern bindet fest an
//! `127.0.0.1`. Stufe 1 hat **keinen Zugangsschutz, keine
//! Ratenbegrenzung und kein TLS**; eine Adresse als Parameter wäre eine
//! Einladung, sie auf `0.0.0.0` zu setzen, und dann stünde eine Tür ohne
//! Schloss im Netz. **Wer öffentlich hören will, braucht Stufe 2 und
//! nicht ein anderes Argument.**

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::annahme::Annahme;
use crate::http::{antwort, kopf_lesen, Httpfehler, MAX_KOPF, MAX_RUMPF};

/// Der Weg, unter dem Anfragen angenommen werden.
pub const WEG: &str = "/inferenz";

/// Der Zuhörer.
pub struct Tuer {
    lauscher: TcpListener,
}

impl Tuer {
    /// Bindet an `127.0.0.1:port`. Port `0` wählt einen freien.
    pub async fn binden(port: u16) -> io::Result<Self> {
        let lauscher = TcpListener::bind(("127.0.0.1", port)).await?;
        Ok(Self { lauscher })
    }

    /// Der tatsächlich belegte Port.
    pub fn port(&self) -> io::Result<u16> {
        Ok(self.lauscher.local_addr()?.port())
    }

    /// Nimmt **eine** Verbindung an und beantwortet sie.
    ///
    /// Eine je Aufruf, damit der Aufrufer die Schleife führt: Ein
    /// Gateway, das seine eigene Endlosschleife mitbringt, lässt sich
    /// nicht anhalten und nicht prüfen.
    pub async fn bedienen(&self, annahme: &mut Annahme) -> io::Result<()> {
        let (strom, _) = self.lauscher.accept().await?;
        Self::eine_verbindung(strom, annahme).await
    }

    async fn eine_verbindung(mut strom: TcpStream, annahme: &mut Annahme) -> io::Result<()> {
        let mut puffer = Vec::with_capacity(1024);
        let kopf = loop {
            match kopf_lesen(&puffer, WEG) {
                Ok(k) => break k,
                Err(Httpfehler::Unvollstaendig) => {}
                Err(e) => return Self::fehler(&mut strom, &e).await,
            }
            let mut hafen = [0u8; 1024];
            let n = strom.read(&mut hafen).await?;
            if n == 0 {
                // ⚑ Die Gegenstelle hat aufgelegt, bevor der Kopf stand.
                // Das ist kein Fehler des Klienten, sondern ein Abbruch.
                return Ok(());
            }
            puffer.extend_from_slice(&hafen[..n]);
            if puffer.len() > MAX_KOPF + MAX_RUMPF {
                return Self::fehler(
                    &mut strom,
                    &Httpfehler::RumpfZuGross {
                        bytes: puffer.len(),
                        grenze: MAX_RUMPF,
                    },
                )
                .await;
            }
        };

        // Den Rumpf vollständig lesen. `kopf_lesen` hat die Länge schon
        // gegen `MAX_RUMPF` geprüft.
        while puffer.len() < kopf.rumpf_ab + kopf.laenge {
            let mut hafen = [0u8; 4096];
            let n = strom.read(&mut hafen).await?;
            if n == 0 {
                // ⚑ **Weniger Rumpf als angekündigt.** Ihn als kurze
                // Anfrage zu nehmen hiesse, eine andere Frage
                // festzuschreiben als die gestellte.
                return Self::fehler(&mut strom, &Httpfehler::Laengenangabe).await;
            }
            puffer.extend_from_slice(&hafen[..n]);
        }
        let rumpf = &puffer[kopf.rumpf_ab..kopf.rumpf_ab + kopf.laenge];

        match annahme.annehmen(rumpf) {
            Ok(beleg) => {
                let bytes = borsh::to_vec(&beleg).unwrap_or_default();
                let a = antwort(200, "application/octet-stream", &bytes);
                strom.write_all(&a).await?;
            }
            Err(e) => {
                let text = format!("{e:?}");
                let a = antwort(400, "text/plain; charset=utf-8", text.as_bytes());
                strom.write_all(&a).await?;
            }
        }
        strom.flush().await
    }

    async fn fehler(strom: &mut TcpStream, e: &Httpfehler) -> io::Result<()> {
        let text = format!("{e:?}");
        let a = antwort(e.status(), "text/plain; charset=utf-8", text.as_bytes());
        strom.write_all(&a).await?;
        strom.flush().await
    }
}
