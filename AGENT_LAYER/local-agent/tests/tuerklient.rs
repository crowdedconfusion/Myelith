//! Der Anfrageweg auf die Probe gestellt (AGENT_LAYER 5.1).
//!
//! # ⚑ Ein Stummel-Server und nicht die echte Tür
//!
//! Die echte Tür steht in `myl-gateway`, und dieses Harness darf sie
//! nicht kennen. **Das ist kein Verzicht, sondern die Aufteilung:**
//!
//! | Frage | Wo sie beantwortet wird |
//! |---|---|
//! | Verhält sich der Klient richtig? | **hier**, gegen einen Stummel |
//! | Versteht die echte Tür ihn? | `myl-testclient`, wo alle Kisten zusammenkommen |
//!
//! Ein Stummel kann Dinge, die die echte Tür nicht kann: eine Antwort
//! ohne `Content-Length` schicken, mitten im Rumpf auflegen, eine
//! Vollmacht zurückspiegeln. Genau daran zeigt sich, ob der Klient
//! trägt.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use myl_local_agent::tuerklient::{Nachricht, Tuerfehler, Tuerklient};

/// Was der Stummel auf eine Anfrage hin tut.
enum Verhalten {
    /// Eine vollständige Antwort mit Längenangabe.
    Antwortet(String),
    /// Dieselbe Antwort **ohne** `Content-Length`, dann auflegen.
    OhneLaenge(String),
    /// Eine Längenangabe, die zu lang ist, dann auflegen.
    Abgeschnitten(String),
    /// Verbindung annehmen und nichts sagen.
    Schweigt,
}

/// Startet einen Stummel und gibt (Port, Empfänger für die Anfrage) zurück.
fn stummel(verhalten: Verhalten) -> (u16, mpsc::Receiver<Vec<u8>>) {
    let horcher = TcpListener::bind("127.0.0.1:0").expect("binden");
    let port = horcher.local_addr().expect("Adresse").port();
    let (sender, empfaenger) = mpsc::channel();
    thread::spawn(move || {
        let (mut strom, _) = horcher.accept().expect("annehmen");
        let anfrage = anfrage_lesen(&mut strom);
        let _ = sender.send(anfrage);
        match verhalten {
            Verhalten::Antwortet(rumpf) => {
                let _ = strom.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\n\r\n{rumpf}",
                        rumpf.len()
                    )
                    .as_bytes(),
                );
            }
            Verhalten::OhneLaenge(rumpf) => {
                let _ = strom.write_all(
                    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{rumpf}")
                        .as_bytes(),
                );
            }
            Verhalten::Abgeschnitten(rumpf) => {
                let _ = strom.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{rumpf}",
                        rumpf.len() + 100
                    )
                    .as_bytes(),
                );
            }
            Verhalten::Schweigt => {
                thread::sleep(Duration::from_secs(5));
            }
        }
    });
    (port, empfaenger)
}

/// Liest Kopf und Rumpf einer Anfrage, so wie der Klient es umgekehrt tut.
fn anfrage_lesen(strom: &mut TcpStream) -> Vec<u8> {
    let mut roh = Vec::new();
    let mut puffer = [0u8; 1024];
    loop {
        let Some(i) = roh.windows(4).position(|f| f == b"\r\n\r\n") else {
            match strom.read(&mut puffer) {
                Ok(0) | Err(_) => return roh,
                Ok(n) => {
                    roh.extend_from_slice(&puffer[..n]);
                    continue;
                }
            }
        };
        let kopf = String::from_utf8_lossy(&roh[..i]).into_owned();
        let laenge: usize = kopf
            .lines()
            .find(|z| z.to_ascii_lowercase().starts_with("content-length:"))
            .and_then(|z| z.split(':').nth(1))
            .and_then(|w| w.trim().parse().ok())
            .unwrap_or(0);
        while roh.len() < i + 4 + laenge {
            match strom.read(&mut puffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => roh.extend_from_slice(&puffer[..n]),
            }
        }
        return roh;
    }
}

const GUTE_ANTWORT: &str = r#"{"id":"seg-7","object":"chat.completion",
"choices":[{"index":0,"message":{"role":"assistant","content":"Paris."},
"finish_reason":"stop"}],"usage":{"prompt_tokens":9,"completion_tokens":2}}"#;

/// ⚑ **Der Grundfall: eine Anfrage hin, ein Text zurück.**
#[test]
fn eine_anfrage_kommt_als_text_zurueck() {
    let (port, gesehen) = stummel(Verhalten::Antwortet(GUTE_ANTWORT.to_string()));
    let klient = Tuerklient::neu("127.0.0.1", port, "vollmacht-abc");
    let a = klient
        .chat("myelith-0.6b", &[Nachricht::nutzer("Hauptstadt?")], Some(8))
        .expect("die Tuer antwortet");

    assert_eq!(a.text, "Paris.");
    assert_eq!(a.abschlussgrund.as_deref(), Some("stop"));
    assert_eq!(a.kennung, "seg-7");
    assert_eq!((a.prompt_token, a.antwort_token), (9, 2));

    // ⚑ **Und die Anfrage selbst gehört geprüft**, nicht nur die
    // Antwort. Ein Klient, der die Vollmacht vergisst, bekommt vom
    // Stummel trotzdem eine Antwort; die echte Tür lehnt ab.
    let roh = gesehen.recv_timeout(Duration::from_secs(5)).expect("Anfrage");
    let text = String::from_utf8_lossy(&roh);
    assert!(text.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"), "{text}");
    assert!(text.contains("Authorization: Bearer vollmacht-abc\r\n"), "{text}");
    assert!(text.contains("Content-Type: application/json\r\n"), "{text}");
    assert!(text.contains(r#""model":"myelith-0.6b""#), "{text}");
    assert!(text.contains(r#""role":"user""#), "{text}");
    assert!(text.contains(r#""max_tokens":8"#), "{text}");
}

/// ⚑ **Ohne `max_tokens` steht das Feld nicht da**, statt als `null`.
///
/// Ein `null` sähe für die Gegenseite aus wie „ausdrücklich keine
/// Obergrenze"; gemeint ist „die Tür entscheidet".
#[test]
fn ohne_obergrenze_fehlt_das_feld() {
    let (port, gesehen) = stummel(Verhalten::Antwortet(GUTE_ANTWORT.to_string()));
    let klient = Tuerklient::neu("127.0.0.1", port, "v");
    let _ = klient.chat("m", &[Nachricht::nutzer("x")], None);
    let roh = gesehen.recv_timeout(Duration::from_secs(5)).expect("Anfrage");
    let text = String::from_utf8_lossy(&roh);
    assert!(!text.contains("max_tokens"), "{text}");
}

/// ⚑ **Die Systemnachricht geht mit und in der richtigen Reihenfolge.**
#[test]
fn mehrere_nachrichten_bleiben_in_der_reihenfolge() {
    let (port, gesehen) = stummel(Verhalten::Antwortet(GUTE_ANTWORT.to_string()));
    let klient = Tuerklient::neu("127.0.0.1", port, "v");
    let _ = klient.chat(
        "m",
        &[Nachricht::system("Sei knapp."), Nachricht::nutzer("Hauptstadt?")],
        None,
    );
    let roh = gesehen.recv_timeout(Duration::from_secs(5)).expect("Anfrage");
    let text = String::from_utf8_lossy(&roh);
    let system = text.find("Sei knapp.").expect("Systemnachricht fehlt");
    let nutzer = text.find("Hauptstadt?").expect("Nutzernachricht fehlt");
    assert!(system < nutzer, "die Reihenfolge kippte: {text}");
}

/// ⚑ **Der Status wird unterschieden.** Ein Harness, das jede
/// Nicht-200 gleich behandelt, kann dem Menschen nicht sagen, was zu
/// tun ist.
#[test]
fn eine_abgelaufene_vollmacht_wird_als_solche_gemeldet() {
    let horcher = TcpListener::bind("127.0.0.1:0").expect("binden");
    let port = horcher.local_addr().expect("Adresse").port();
    thread::spawn(move || {
        let (mut strom, _) = horcher.accept().expect("annehmen");
        let _ = anfrage_lesen(&mut strom);
        let rumpf = r#"{"error":{"message":"Vollmacht abgelaufen"}}"#;
        let _ = strom.write_all(
            format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\n\r\n{rumpf}",
                rumpf.len()
            )
            .as_bytes(),
        );
    });
    let klient = Tuerklient::neu("127.0.0.1", port, "alt");
    let fehler = klient
        .chat("m", &[Nachricht::nutzer("x")], None)
        .expect_err("401 ist kein Erfolg");
    assert!(matches!(fehler, Tuerfehler::Abgelehnt { status: 401, .. }), "{fehler:?}");
    assert!(fehler.to_string().contains("die Vollmacht gilt hier nicht"), "{fehler}");
}

/// ⚑ **Ohne Längenangabe wird bis zum Auflegen gelesen.**
#[test]
fn eine_antwort_ohne_laengenangabe_wird_trotzdem_gelesen() {
    let (port, _g) = stummel(Verhalten::OhneLaenge(GUTE_ANTWORT.to_string()));
    let klient = Tuerklient::neu("127.0.0.1", port, "v");
    let a = klient.chat("m", &[Nachricht::nutzer("x")], None).expect("gelesen");
    assert_eq!(a.text, "Paris.");
}

/// ⚑ **Ein abgeschnittener Rumpf meldet den Abbruch, nicht „unlesbar".**
///
/// Der Unterschied ist der zwischen „die Tür schickt Unsinn" und „die
/// Verbindung brach", und nur der zweite lässt sich wiederholen.
#[test]
fn ein_abgeschnittener_rumpf_meldet_den_abbruch() {
    let (port, _g) = stummel(Verhalten::Abgeschnitten(GUTE_ANTWORT.to_string()));
    let klient = Tuerklient::neu("127.0.0.1", port, "v");
    let fehler = klient
        .chat("m", &[Nachricht::nutzer("x")], None)
        .expect_err("ein halber Rumpf ist kein Erfolg");
    let text = fehler.to_string();
    assert!(text.contains("die Verbindung brach nach"), "{text}");
    assert!(!text.contains("kein lesbares JSON"), "der Grund ist verwechselt: {text}");
}

/// ⚑ **Die Frist wird eingehalten und benannt.**
#[test]
fn eine_stumme_tuer_laeuft_in_die_frist() {
    let (port, _g) = stummel(Verhalten::Schweigt);
    let klient = Tuerklient::neu("127.0.0.1", port, "v").mit_frist(Duration::from_millis(300));
    let fehler = klient
        .chat("m", &[Nachricht::nutzer("x")], None)
        .expect_err("Schweigen ist keine Antwort");
    assert!(
        matches!(fehler, Tuerfehler::Zeitueberschreitung { frist_ms: 300 }),
        "{fehler:?}"
    );
}

/// ⚑ **Eine geschlossene Tür sagt, dass sie geschlossen ist.**
///
/// Der häufigste Fall im Alltag: Der Knoten läuft nicht. Ein Harness,
/// das dabei „unlesbar" meldet, schickt den Menschen in die falsche
/// Richtung.
#[test]
fn eine_geschlossene_tuer_wird_als_solche_gemeldet() {
    // Port binden und sofort fallen lassen: nichts horcht mehr.
    let port = {
        let h = TcpListener::bind("127.0.0.1:0").expect("binden");
        h.local_addr().expect("Adresse").port()
    };
    let klient = Tuerklient::neu("127.0.0.1", port, "v");
    let fehler = klient
        .chat("m", &[Nachricht::nutzer("x")], None)
        .expect_err("hier horcht niemand");
    assert!(matches!(fehler, Tuerfehler::Unerreichbar { .. }), "{fehler:?}");
    assert!(fehler.to_string().contains("Laeuft der Knoten?"), "{fehler}");
}

/// ⚑ **Eine Antwort ohne Wahl ist kein Text.**
#[test]
fn eine_antwort_ohne_wahl_wird_nicht_als_leerer_text_ausgegeben() {
    let (port, _g) = stummel(Verhalten::Antwortet(r#"{"id":"x","choices":[]}"#.to_string()));
    let klient = Tuerklient::neu("127.0.0.1", port, "v");
    let fehler = klient
        .chat("m", &[Nachricht::nutzer("x")], None)
        .expect_err("keine Wahl ist kein Erfolg");
    assert!(matches!(fehler, Tuerfehler::OhneWahl), "{fehler:?}");
}

/// ⚑ **Kein HTTP ist kein JSON.** Wer auf einen falschen Port zeigt,
/// soll das erfahren.
#[test]
fn eine_antwort_die_kein_http_ist_wird_als_solche_gemeldet() {
    let horcher = TcpListener::bind("127.0.0.1:0").expect("binden");
    let port = horcher.local_addr().expect("Adresse").port();
    thread::spawn(move || {
        let (mut strom, _) = horcher.accept().expect("annehmen");
        let _ = anfrage_lesen(&mut strom);
        let _ = strom.write_all(b"SSH-2.0-OpenSSH_9.0\r\n\r\n");
    });
    let klient = Tuerklient::neu("127.0.0.1", port, "v");
    let fehler = klient
        .chat("m", &[Nachricht::nutzer("x")], None)
        .expect_err("SSH ist keine Chatantwort");
    assert!(matches!(fehler, Tuerfehler::KeinHttp), "{fehler:?}");
}
