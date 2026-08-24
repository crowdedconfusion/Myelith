//! Direkte Anfragen zwischen zwei Knoten (Punkt 1.5).
//!
//! # Warum Gossip dafür das falsche Werkzeug ist
//!
//! Gossip verbreitet an alle. Für „ich habe etwas verpasst, schick es
//! mir noch einmal" ist das verkehrt herum: Die Frage geht an **einen**
//! Knoten, die Antwort betrifft nur den Fragenden, und beides alle
//! anderen lesen zu lassen kostet Bandbreite ohne Gegenwert.
//!
//! Dieses Modul ergänzt deshalb einen Punkt-zu-Punkt-Kanal:
//! `/myelith/anfrage/1`.
//!
//! # ⚑ Die Nutzlast bleibt undurchsichtig, und das mit Absicht
//!
//! Der Kanal transportiert **Bytes**, sonst nichts. Er weiß nicht, was
//! ein Block ist, und darf es nicht wissen: `myl-net` ist die
//! Netzschicht (L0), Blöcke liegen in `myl-consensus` (L1). Würde hier
//! ein `Blockanfrage`-Typ stehen, wäre die Schichtung umgekehrt, und
//! jeder, der nur ein Netz braucht, zöge den Konsens mit.
//!
//! Was die Bytes bedeuten, entscheidet die Anwendung. Der Knoten legt
//! darüber sein eigenes Format, und derselbe Kanal trägt später
//! Zustandsabgleich oder Konsens-Nachfragen, ohne dass hier etwas
//! dazukommt.
//!
//! # Größengrenze
//!
//! [`MAX_ANFRAGE_BYTES`] gilt für Anfrage **und** Antwort. Ohne sie
//! ließe sich ein Knoten mit einer einzigen Anfrage zum Senden
//! beliebiger Datenmengen bewegen, und das wäre ein Verstärker: wenig
//! Aufwand beim Angreifer, viel beim Opfer.

use std::io;

use futures::{AsyncReadExt, AsyncWriteExt};
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::StreamProtocol;

/// Protokollname des Anfragekanals. Konsens-Feld: Eine Änderung bricht
/// die Kompatibilität.
pub const ANFRAGE_PROTOKOLL: &str = "/myelith/anfrage/1";

/// Größengrenze für Anfrage und Antwort.
///
/// 4 MiB, gleichgezogen mit [`crate::config::MAX_GOSSIP_MESSAGE_BYTES`]:
/// Was über Gossip passt, muss auch über eine Nachfrage passen, sonst
/// wäre eine Nachricht verbreitbar, aber nicht nachforderbar.
pub const MAX_ANFRAGE_BYTES: usize = 4 * 1024 * 1024;

/// Der Codec: Längenpräfix, dann Bytes.
///
/// Vier Bytes Länge in Little-Endian, dann die Nutzlast. Kein Rahmen,
/// keine Struktur, keine Auslegung. Genau so viel, wie ein Transport
/// braucht.
#[derive(Debug, Clone, Default)]
pub struct ByteCodec;

async fn lies<T>(io_: &mut T) -> io::Result<Vec<u8>>
where
    T: futures::AsyncRead + Unpin + Send,
{
    let mut kopf = [0u8; 4];
    io_.read_exact(&mut kopf).await?;
    let laenge = u32::from_le_bytes(kopf) as usize;
    if laenge > MAX_ANFRAGE_BYTES {
        // Vor dem Lesen ablehnen, nicht danach: Sonst hätte der
        // Angreifer den Speicher schon belegt.
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Nutzlast zu groß: {laenge} > {MAX_ANFRAGE_BYTES}"),
        ));
    }
    let mut daten = vec![0u8; laenge];
    io_.read_exact(&mut daten).await?;
    Ok(daten)
}

async fn schreib<T>(io_: &mut T, daten: Vec<u8>) -> io::Result<()>
where
    T: futures::AsyncWrite + Unpin + Send,
{
    if daten.len() > MAX_ANFRAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Nutzlast zu groß: {} > {}", daten.len(), MAX_ANFRAGE_BYTES),
        ));
    }
    io_.write_all(&(daten.len() as u32).to_le_bytes()).await?;
    io_.write_all(&daten).await?;
    io_.close().await
}

#[async_trait::async_trait]
impl request_response::Codec for ByteCodec {
    type Protocol = StreamProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    async fn read_request<T>(&mut self, _: &StreamProtocol, io_: &mut T) -> io::Result<Vec<u8>>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        lies(io_).await
    }

    async fn read_response<T>(&mut self, _: &StreamProtocol, io_: &mut T) -> io::Result<Vec<u8>>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        lies(io_).await
    }

    async fn write_request<T>(
        &mut self,
        _: &StreamProtocol,
        io_: &mut T,
        daten: Vec<u8>,
    ) -> io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        schreib(io_, daten).await
    }

    async fn write_response<T>(
        &mut self,
        _: &StreamProtocol,
        io_: &mut T,
        daten: Vec<u8>,
    ) -> io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        schreib(io_, daten).await
    }
}

/// Das Verhalten des Anfragekanals.
pub type AnfrageBehaviour = request_response::Behaviour<ByteCodec>;

/// Baut den Anfragekanal.
pub fn baue_anfragekanal() -> AnfrageBehaviour {
    request_response::Behaviour::with_codec(
        ByteCodec,
        [(
            StreamProtocol::new(ANFRAGE_PROTOKOLL),
            // Beides: Jeder Knoten fragt und antwortet. Wer nur fragt,
            // lebt vom Entgegenkommen anderer, ohne selbst welches zu
            // zeigen, und ein Netz aus solchen Knoten hat niemanden, den
            // es fragen kann.
            ProtocolSupport::Full,
        )],
        request_response::Config::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_protokollname_ist_versioniert() {
        // Ohne Version bricht eine Formatänderung stumm.
        assert!(ANFRAGE_PROTOKOLL.starts_with("/myelith/"));
        assert!(ANFRAGE_PROTOKOLL.ends_with("/1"));
    }

    #[test]
    fn die_grenze_deckt_alles_ab_was_gossip_traegt() {
        // Sonst wäre eine Nachricht verbreitbar, aber nicht
        // nachforderbar, und ein Knoten könnte etwas verpassen, das er
        // nicht zurückholen kann.
        // Als const-Block: Wer die Grenze unter die Gossip-Grenze
        // senkt, bekommt einen Übersetzungsfehler statt eines roten
        // Tests.
        const {
            assert!(
                MAX_ANFRAGE_BYTES >= crate::config::MAX_GOSSIP_MESSAGE_BYTES,
                "was über Gossip passt, muss auch nachforderbar sein"
            )
        };
    }

    #[tokio::test]
    async fn ein_laengenkopf_ueber_der_grenze_wird_vor_dem_lesen_abgelehnt() {
        // Vor dem Lesen, nicht danach: Sonst hätte der Angreifer den
        // Speicher schon belegt.
        let mut roh: Vec<u8> = ((MAX_ANFRAGE_BYTES + 1) as u32).to_le_bytes().to_vec();
        roh.extend_from_slice(&[0u8; 16]);
        let mut leser = futures::io::Cursor::new(roh);
        let ergebnis = lies(&mut leser).await;
        assert!(ergebnis.is_err(), "Übergröße wurde angenommen");
    }

    #[tokio::test]
    async fn was_geschrieben_wurde_kommt_zurueck() {
        let nutzlast = b"eine Nachforderung".to_vec();
        let mut puffer: Vec<u8> = Vec::new();
        schreib(&mut futures::io::Cursor::new(&mut puffer), nutzlast.clone())
            .await
            .expect("schreiben");
        let zurueck = lies(&mut futures::io::Cursor::new(puffer))
            .await
            .expect("lesen");
        assert_eq!(zurueck, nutzlast);
    }

    #[tokio::test]
    async fn eine_leere_nutzlast_geht_durch() {
        let mut puffer: Vec<u8> = Vec::new();
        schreib(&mut futures::io::Cursor::new(&mut puffer), Vec::new())
            .await
            .expect("schreiben");
        assert_eq!(lies(&mut futures::io::Cursor::new(puffer)).await.unwrap(), Vec::<u8>::new());
    }

    #[tokio::test]
    async fn ein_abgeschnittener_kopf_stuerzt_nicht_ab() {
        let mut leser = futures::io::Cursor::new(vec![1u8, 2]);
        assert!(lies(&mut leser).await.is_err());
    }
}
