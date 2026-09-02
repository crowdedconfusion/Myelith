//! Der Beobachtungsendpunkt am echten Prozess (Fund 129).
//!
//! # ⚑ Warum als Integrationstest und nicht als Modultest
//!
//! Die reinen Funktionen (Zerlegen, Textbau, Bereitschaft) sind im
//! Modul geprüft. Was **hier** geprüft wird, kann dort nicht geprüft
//! werden: dass der Dienst wirklich aufmacht, dass die Zahlen des
//! laufenden Knotens ankommen und dass Leben und Bereitschaft am
//! frisch gestarteten Knoten verschieden ausfallen.
//!
//! **Der Port ist `0`, also einer, den das Betriebssystem aussucht.**
//! Ein fester Port kollidierte, sobald zwei Tests nebeneinander laufen
//! oder jemand auf derselben Maschine einen Knoten betreibt. Welchen er
//! bekam, sagt der Knoten in seiner Ausgabe, und genau dort liest der
//! Test ihn ab.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("myelith-beobachtung-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("Arbeitsverzeichnis");
    d
}

fn vektoren() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../INTEGER_LLM/conformance/vectors/op")
}

fn text(pfad: &Path) -> String {
    std::fs::read_to_string(pfad).unwrap_or_default()
}

fn warte_auf(was: &str, mut bedingung: impl FnMut() -> bool) {
    let bis = Instant::now() + Duration::from_secs(30);
    while Instant::now() < bis {
        if bedingung() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("{was} kam nicht innerhalb von 30 Sekunden");
}

/// Eine Anfrage stellen und die rohe Antwort zurückgeben.
fn hole(port: u16, weg: &str) -> String {
    let mut strom = TcpStream::connect(("127.0.0.1", port)).expect("Verbindung");
    strom
        .write_all(format!("GET {weg} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
        .expect("Anfrage");
    let mut aus = String::new();
    strom.read_to_string(&mut aus).expect("Antwort");
    aus
}

fn starte(logs: &Path, schluessel: &Path, stderr: &Path) -> Child {
    let datei = std::fs::File::create(stderr).expect("stderr-Datei");
    Command::new(env!("CARGO_BIN_EXE_myl-node"))
        .args([
            "--name",
            "beobachtungsprobe",
            "--protokolle",
            logs.to_str().unwrap(),
            "--schluessel",
            schluessel.to_str().unwrap(),
            "--horche",
            "/ip4/127.0.0.1/tcp/0",
            // Port 0: das Betriebssystem sucht einen freien aus.
            "--beobachtung",
            "127.0.0.1:0",
            // Die Aufnahme treibt den Stand. Alle zwei Sekunden, damit
            // der Test nicht dreissig Sekunden auf die erste wartet.
            "--aufnahme",
            "2",
            "--laufzeit",
            "300",
            "--konformitaet",
            vektoren().to_str().unwrap(),
            // Ohne Kettendatei: Der Test prueft den Endpunkt, nicht die
            // Persistenz, und braucht kein Verzeichnis aufzuraeumen.
            "--ohne-kette",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::from(datei))
        .spawn()
        .expect("myl-node startet")
}

/// Den Port aus der Ausgabe klauben.
fn port_aus(ausgabe: &str) -> u16 {
    let zeile = ausgabe
        .lines()
        .find(|z| z.contains("Beobachtung auf http://"))
        .expect("der Knoten nennt seinen Beobachtungsport");
    zeile
        .rsplit_once(':')
        .and_then(|(_, rest)| rest.split('/').next())
        .and_then(|p| p.parse().ok())
        .expect("der Port ist eine Zahl")
}

/// ⚑ **Der Endpunkt macht auf, liefert Metriken und trennt Leben von
/// Bereitschaft.**
///
/// Drei Zusicherungen in einem Lauf, weil ein Knotenstart neun Sekunden
/// kostet und drei Tests ihn dreimal bezahlten.
#[test]
fn der_endpunkt_antwortet_am_laufenden_knoten() {
    let logs = scratch("lauf");
    let schluessel = scratch("lauf-key").join("knoten.key");
    let stderr = logs.join("stderr.txt");
    let mut kind = starte(&logs, &schluessel, &stderr);

    warte_auf("die Portmeldung", || {
        text(&stderr).contains("Beobachtung auf http://")
    });
    let port = port_aus(&text(&stderr));
    assert_ne!(port, 0, "Port 0 heisst, das Betriebssystem hat nichts vergeben");

    // ⚑ **Vor dem Ende des Startvorlaufs.** Gerade waehrend des
    // Aufholens will jemand wissen, wie weit der Knoten ist; ein
    // Endpunkt, der erst danach aufmacht, schwiege genau dann.
    let leben = hole(port, "/gesundheit");
    assert!(leben.starts_with("HTTP/1.1 200 "), "Gesundheit: {leben}");

    // Bereitschaft: Ein frisch gestarteter Knoten ohne Peers ist am
    // Leben und **nicht** bereit. Das ist der Unterschied, um den es
    // geht.
    let bereit = hole(port, "/bereit");
    assert!(
        bereit.starts_with("HTTP/1.1 503 "),
        "ein Knoten ohne Peers darf nicht bereit melden: {bereit}"
    );

    // Metriken: Erst nach der ersten Zustandsaufnahme stehen echte
    // Zahlen darin. Vorher antwortet der Endpunkt trotzdem, mit dem
    // leeren Stand, und auch das ist richtig: Schweigen waere schlechter.
    let metriken = hole(port, "/metriken");
    assert!(metriken.starts_with("HTTP/1.1 200 "), "Metriken: {metriken}");
    assert!(
        metriken.contains("Content-Type: text/plain; version=0.0.4; charset=utf-8"),
        "der Inhaltstyp stimmt nicht: {}",
        metriken.lines().take(5).collect::<Vec<_>>().join(" | ")
    );
    for name in [
        "myelith_kette_hoehe",
        "myelith_peers",
        "myelith_bereit",
        "myelith_stand_alter_millisekunden",
        "myelith_kette_schreibfehler_total",
    ] {
        assert!(metriken.contains(name), "{name} fehlt in den Metriken");
    }

    // Und die Zahlen bewegen sich: Nach zwei Aufnahmen ist der
    // Protokollzeilenzaehler groesser als null.
    warte_auf("eine Zustandsaufnahme im Endpunkt", || {
        let m = hole(port, "/metriken");
        m.lines()
            .find_map(|z| z.strip_prefix("myelith_protokollzeilen_total "))
            .and_then(|w| w.trim().parse::<u64>().ok())
            .is_some_and(|n| n > 0)
    });

    // Ein unbekannter Weg ist 404, kein GET ist 405.
    let vier = hole(port, "/anderswo");
    assert!(vier.starts_with("HTTP/1.1 404 "), "unbekannter Weg: {vier}");

    let _ = Command::new("kill").args(["-15", &kind.id().to_string()]).status();
    warte_auf("das Ende des Prozesses", || {
        matches!(kind.try_wait(), Ok(Some(_)))
    });
    let _ = std::fs::remove_dir_all(&logs);
}
