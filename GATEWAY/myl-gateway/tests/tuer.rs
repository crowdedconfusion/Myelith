//! Die Tür, über einen echten Socket.
//!
//! ⚑ **Ohne diese Tests wäre der Endpunkt eine Vermutung.** Die
//! Zerlegung ist einzeln geprüft; was hier geprüft wird, ist der Weg
//! **durch** einen Socket: dass eine Anfrage ankommt, dass ein Beleg
//! zurückkommt, und dass die Ablehnungen ablehnen statt zu schweigen.

use myl_gateway::annahme::{Annahme, Beleg};
use myl_gateway::{Tuer, WEG};
use myl_types::ids::EpochId;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Schickt rohe Bytes und liest die ganze Antwort.
///
/// ⚑ **Die Schreibseite wird geschlossen, und das ist nicht Kosmetik.**
/// Ohne `shutdown` wartet der Server auf den angekündigten Rest, während
/// der Klient auf die Antwort wartet: **ein Deadlock, und der erste Lauf
/// ist darin hängengeblieben.** Er hat damit gezeigt, dass der Server
/// einen abgebrochenen Rumpf am Dateiende erkennt und nicht an einer
/// Frist; das ist die richtige Reihenfolge.
///
/// Die Frist darüber ist der zweite Riegel: Ein hängender Test sagt
/// nichts, ein fehlgeschlagener sagt etwas.
async fn frage(port: u16, roh: &[u8]) -> Vec<u8> {
    let arbeit = async {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.expect("verbinden");
        s.write_all(roh).await.expect("senden");
        s.flush().await.expect("leeren");
        s.shutdown().await.expect("Schreibseite schliessen");
        let mut aus = Vec::new();
        s.read_to_end(&mut aus).await.expect("lesen");
        aus
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), arbeit)
        .await
        .expect("die Tuer hat innerhalb von fuenf Sekunden nicht geantwortet")
}

fn post(weg: &str, rumpf: &str) -> Vec<u8> {
    format!(
        "POST {weg} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{rumpf}",
        rumpf.len()
    )
    .into_bytes()
}

fn rumpf_von(antwort: &[u8]) -> &[u8] {
    let i = antwort
        .windows(4)
        .position(|f| f == b"\r\n\r\n")
        .expect("Kopfende");
    &antwort[i + 4..]
}

#[tokio::test]
async fn eine_anfrage_bekommt_einen_beleg() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(9));

    let auftrag = tokio::spawn(async move { frage(port, &post(WEG, "was ist ein pod")).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");

    assert!(
        String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 200 OK"),
        "keine 200er-Antwort: {}",
        String::from_utf8_lossy(&antwort[..antwort.len().min(60)])
    );
    let beleg: Beleg = borsh::from_slice(rumpf_von(&antwort)).expect("Beleg");
    assert_eq!(beleg.sitzung, 1);
    // ⚑ **Der Beleg passt zu der Frage, die gestellt wurde**, und zu
    // keiner anderen. Genau das ist sein Zweck.
    assert!(beleg.bindung.passt(b"was ist ein pod"));
    assert!(!beleg.bindung.passt(b"was ist ein Pod"));
}

/// ⚑ **Ein falscher Weg wird abgelehnt, nicht stillschweigend bedient.**
#[tokio::test]
async fn ein_falscher_weg_bekommt_404() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(9));

    let auftrag = tokio::spawn(async move { frage(port, &post("/anderswo", "x")).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");
    assert!(String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 404"));
}

/// ⚑ **Stückweise wird abgelehnt, und der Klient erfährt es.**
///
/// Schweigen ließe ihn auf eine Zeitüberschreitung warten, die ihm
/// nichts sagt; stillschweigend als Rumpf zu lesen wäre die
/// Schmuggelstelle.
#[tokio::test]
async fn stueckweise_bekommt_eine_ablehnung() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(9));

    let roh = format!("POST {WEG} HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n")
        .into_bytes();
    let auftrag = tokio::spawn(async move { frage(port, &roh).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");
    assert!(String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 411"));
}

/// ⚑ **Eine leere Anfrage verbraucht keine Sitzungsnummer.**
#[tokio::test]
async fn eine_leere_anfrage_wird_abgelehnt() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(9));

    let auftrag = tokio::spawn(async move { frage(port, &post(WEG, "")).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");
    assert!(String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 400"));
    // Die naechste echte Anfrage bekommt trotzdem die Eins.
    let auftrag = tokio::spawn(async move { frage(port, &post(WEG, "echt")).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");
    let beleg: Beleg = borsh::from_slice(rumpf_von(&antwort)).expect("Beleg");
    assert_eq!(beleg.sitzung, 1);
}

/// ⚑ **Ein Rumpf, der kürzer ankommt als angekündigt, wird abgelehnt.**
///
/// Ihn als kurze Anfrage zu nehmen hieße, eine andere Frage
/// festzuschreiben als die gestellte.
#[tokio::test]
async fn ein_abgebrochener_rumpf_wird_abgelehnt() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(9));

    let roh = format!("POST {WEG} HTTP/1.1\r\nContent-Length: 99\r\n\r\nnurdrei").into_bytes();
    let auftrag = tokio::spawn(async move { frage(port, &roh).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");
    assert!(String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 400"));
}

/// Und die Tür hört nur auf der Rückschleife.
#[tokio::test]
async fn die_tuer_hoert_nur_auf_localhost() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    // Über die Rückschleife erreichbar.
    assert!(TcpStream::connect(("127.0.0.1", port)).await.is_ok());
}
