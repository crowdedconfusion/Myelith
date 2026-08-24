//! Knotenbetrieb aus dem Testclient heraus.
//!
//! # Warum das hier liegt und nicht in einem zweiten Programm
//!
//! `myl-node` hat eine eigene Kommandozeile, und die reicht für jemanden,
//! der Multiaddressen liest. Für die Partner, an die sich dieser Client
//! richtet, reicht sie nicht: Sie müssten ein zweites Programm bauen,
//! seinen Pfad finden und Adressen von Hand zusammensetzen.
//!
//! Deshalb läuft der Knoten hier **im selben Prozess**. Ein Programm,
//! ein Bau, ein Menü. Die Kommandozeile von `myl-node` bleibt daneben
//! bestehen, für Server ohne Bildschirm.
//!
//! # Die zwei Rollen, und warum sie getrennt sind
//!
//! - **Anlaufstelle** (Entwicklermenü): braucht eine öffentlich
//!   erreichbare Adresse und eine Portweiterleitung. Das ist Arbeit am
//!   Router, und wer sie macht, weiß in der Regel, was er tut.
//! - **Teilnehmer** (Nutzermenü): braucht **nichts** außer der Adresse,
//!   die der Koordinator schickt. Kein Port, keine Firewall-Regel.
//!
//! Die Trennung ist keine Bequemlichkeit, sondern die Sache selbst: Ein
//! Netz, in dem jeder eine Portweiterleitung braucht, hätte kaum
//! Teilnehmer.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use myl_node::konfig::{KnotenKonfig, Rolle};
use myl_node::Knoten;

/// Liest eine Zeile mit Rückfall auf eine Vorgabe.
fn frage(text: &str, vorgabe: &str) -> String {
    if vorgabe.is_empty() {
        print!("  {text}: ");
    } else {
        print!("  {text} [{vorgabe}]: ");
    }
    let _ = std::io::stdout().flush();
    let mut eingabe = String::new();
    let _ = std::io::stdin().read_line(&mut eingabe);
    let eingabe = eingabe.trim().to_string();
    if eingabe.is_empty() { vorgabe.to_string() } else { eingabe }
}

fn frage_zahl(text: &str, vorgabe: u64) -> u64 {
    loop {
        let roh = frage(text, &vorgabe.to_string());
        match roh.parse() {
            Ok(z) => return z,
            Err(_) => println!("  Das ist keine Zahl. Noch einmal."),
        }
    }
}

/// Prüft eine Einladungsadresse, bevor der Knoten damit startet.
///
/// Der häufigste Fehler beim Abtippen ist der fehlende `/p2p/…`-Teil,
/// und ohne ihn ist die Gegenstelle nicht überprüfbar. Das gehört
/// gesagt, bevor jemand zehn Minuten auf eine Verbindung wartet, die
/// nicht kommen kann.
pub fn pruefe_einladung(adresse: &str) -> Result<(), String> {
    if adresse.trim().is_empty() {
        return Err("Es wurde keine Adresse eingegeben.".to_string());
    }
    myl_net::parse_bootstrap_peer(adresse.trim()).map_err(|_| {
        format!(
            "Diese Adresse ist unbrauchbar:\n    {}\n  \
             Sie muss so aussehen: /ip4/203.0.113.5/tcp/4150/p2p/12D3KooW…\n  \
             Der Teil ab /p2p/ gehört dazu, ohne ihn lässt sich die \
             Gegenstelle nicht überprüfen.",
            adresse.trim()
        )
    })?;
    Ok(())
}

/// Wohin die Betriebsprotokolle geschrieben werden.
pub fn protokollverzeichnis() -> PathBuf {
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    crate::vergleich::vergleichsordner(&repo)
}

/// Fährt einen Knoten für die angegebene Zeit und meldet den Pfad des
/// Protokolls.
fn fahre(konfig: KnotenKonfig, laufzeit: Duration) -> bool {
    let laufwerk = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(r) => r,
        Err(e) => {
            println!("  Laufzeitumgebung ließ sich nicht starten: {e}");
            return false;
        }
    };
    laufwerk.block_on(async move {
        let mut knoten = match Knoten::starten(konfig, false).await {
            Ok(k) => k,
            Err(e) => {
                println!();
                println!("  Der Knoten ist nicht gestartet:");
                println!("    {e}");
                return false;
            }
        };
        println!();
        println!("  Knoten läuft.");
        println!("  Kennung:   {}", knoten.peer_id());
        println!("  Protokoll: {}", knoten.protokollpfad().display());

        if knoten.warte_auf_adresse(Duration::from_secs(8)).await.is_some() {
            println!();
            println!("  Erreichbar unter:");
            for a in knoten.adressen() {
                println!("    {a}");
            }
        } else {
            println!();
            println!("  Noch keine eigene Adresse gemeldet. Das ist hinter einem");
            println!("  Router normal, solange ein Relais angegeben wurde.");
        }
        println!();
        println!("  Läuft für {} Sekunden. Strg-C bricht ab.", laufzeit.as_secs());
        println!();

        knoten.laufe_fuer(laufzeit).await;
        knoten.aufnahme().await;

        let peers = knoten.peers().await;
        println!("  Beendet.");
        println!("  {} Protokollzeilen, zuletzt {peers} Peer(s) verbunden.", knoten.protokollzeilen());
        println!("  Protokoll: {}", knoten.protokollpfad().display());
        println!();
        if peers == 0 {
            println!("  ⚠ Am Ende war keine Verbindung offen. Mögliche Gründe stehen");
            println!("    in ANLEITUNG.md, Abschnitt C8.");
        }
        true
    })
}

/// Entwicklermenü: den Knoten als **Anlaufstelle** betreiben.
///
/// Setzt eine öffentlich erreichbare Adresse voraus. Ohne sie startet
/// der Knoten gar nicht erst, und das ist Absicht: Ein Relais schreibt
/// seine eigene Adresse in die Antwort an die Knoten, die es vermittelt.
/// Kennt es sie nicht, nimmt es Anfragen an und antwortet ins Leere.
/// Alles läuft, nur niemand kommt an.
pub fn anlaufstelle() -> bool {
    println!("  Knoten als Anlaufstelle betreiben");
    println!();
    println!("  Diese Maschine ist der Einstiegspunkt für alle anderen und");
    println!("  vermittelt Verbindungen für Teilnehmer hinter einem Router.");
    println!();
    println!("  Voraussetzung: eine öffentlich erreichbare IP-Adresse und ein");
    println!("  freigegebener Port, für TCP UND UDP. Die eigene Adresse zeigt");
    println!("  auf den meisten Systemen:  curl -4 https://ifconfig.me");
    println!();

    let name = frage("Name dieser Maschine", "anlaufstelle");
    let ip = frage("Öffentliche IP-Adresse", "");
    if ip.is_empty() {
        println!();
        println!("  Ohne öffentliche Adresse kann diese Maschine keine Anlaufstelle");
        println!("  sein. Abgebrochen.");
        return false;
    }
    let port = frage_zahl("Port", 4150) as u16;
    let minuten = frage_zahl("Laufzeit in Minuten", 60);
    let takt = frage_zahl("Testnachricht alle wie viele Sekunden (0 = keine)", 10);

    let konfig = KnotenKonfig {
        name: name.clone(),
        schluesseldatei: PathBuf::from(format!("{name}.key")),
        protokollverzeichnis: protokollverzeichnis(),
        horchadressen: myl_node::konfig::standard_horchadressen(port),
        bootstrap: Vec::new(),
        rolle: Rolle::Relais,
        nat: myl_net::NatKonfig {
            dient_als_relais: true,
            relais: Vec::new(),
            oeffentliche_adressen: vec![
                format!("/ip4/{ip}/tcp/{port}"),
                format!("/ip4/{ip}/udp/{port}/quic-v1"),
            ],
        },
        aufnahme_sekunden: 30,
        testverkehr_sekunden: if takt == 0 { None } else { Some(takt) },
    };

    println!();
    println!("  Sobald der Knoten läuft, erscheint unten eine Zeile der Form");
    println!("    /ip4/{ip}/tcp/{port}/p2p/12D3KooW…");
    println!("  Diese Zeile vollständig an alle Teilnehmer schicken. Sie ist");
    println!("  die Einladung ins Netz, der Teil ab /p2p/ gehört dazu.");

    fahre(konfig, Duration::from_secs(minuten * 60))
}

/// Nutzermenü: als **Teilnehmer** mitmachen.
///
/// Braucht nichts außer der Einladung des Koordinators. Kein offener
/// Port, keine Firewall-Regel.
pub fn teilnehmer(name_vorgabe: &str) -> bool {
    println!("  Am Netz teilnehmen");
    println!();
    println!("  Du brauchst nur die Adresse, die dir der Koordinator geschickt");
    println!("  hat. Keinen offenen Port, keine Router-Einstellung.");
    println!();

    let einladung = frage("Adresse vom Koordinator", "");
    if let Err(e) = pruefe_einladung(&einladung) {
        println!();
        println!("  {e}");
        return false;
    }
    let name = frage("Dein Name für das Protokoll", name_vorgabe);
    let minuten = frage_zahl("Laufzeit in Minuten", 60);
    let takt = frage_zahl("Testnachricht alle wie viele Sekunden (0 = keine)", 10);

    let konfig = KnotenKonfig {
        name: name.clone(),
        schluesseldatei: PathBuf::from(format!("{name}.key")),
        protokollverzeichnis: protokollverzeichnis(),
        // Port 0: Das Betriebssystem sucht einen freien. Ein Teilnehmer
        // nimmt keine Verbindungen von außen an, er braucht keinen
        // bestimmten Port, und ein belegter würde ihn nur aufhalten.
        horchadressen: myl_node::konfig::standard_horchadressen(0),
        bootstrap: vec![einladung.trim().to_string()],
        rolle: Rolle::Teilnehmer,
        nat: myl_net::NatKonfig {
            dient_als_relais: false,
            // Dieselbe Adresse auch als Relais: Sitzt dieser Rechner
            // hinter einem Router, besorgt er sich dort eine Adresse,
            // unter der andere ihn erreichen.
            relais: vec![einladung.trim().to_string()],
            oeffentliche_adressen: Vec::new(),
        },
        aufnahme_sekunden: 30,
        testverkehr_sekunden: if takt == 0 { None } else { Some(takt) },
    };

    println!();
    println!("  Am Ende liegt das Protokoll in:");
    println!("    {}", protokollverzeichnis().display());
    println!("  Diese Datei schickst du dem Koordinator.");

    fahre(konfig, Duration::from_secs(minuten * 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eine_einladung_ohne_p2p_teil_wird_abgelehnt() {
        // Der häufigste Abtippfehler. Er gehört vor dem Start gemeldet,
        // nicht nach zehn Minuten Warten auf eine Verbindung, die nicht
        // kommen kann.
        let fehler = pruefe_einladung("/ip4/203.0.113.5/tcp/4150").unwrap_err();
        assert!(fehler.contains("/p2p/"), "der Hinweis nennt den fehlenden Teil nicht: {fehler}");
    }

    #[test]
    fn eine_leere_einladung_wird_abgelehnt() {
        assert!(pruefe_einladung("   ").is_err());
    }

    #[test]
    fn eine_vollstaendige_einladung_geht_durch() {
        let gut = "/ip4/203.0.113.5/tcp/4150/p2p/\
                   12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN";
        pruefe_einladung(gut).expect("gültige Einladung");
    }

    #[test]
    fn leerzeichen_am_rand_stoeren_nicht() {
        // Beim Kopieren aus einer Nachricht hängt fast immer eines dran.
        let gut = "  /ip4/203.0.113.5/tcp/4150/p2p/\
                   12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN  ";
        pruefe_einladung(gut).expect("Rand-Leerzeichen dürfen nicht stören");
    }
}
