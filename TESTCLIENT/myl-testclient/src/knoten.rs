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

/// Fragt die Teilnehmerliste ab.
///
/// # ⚑ Wozu die gebraucht wird
///
/// Latenz-Atteste tragen eine Signatur, und geprüft werden kann sie nur
/// gegen den Schlüssel des Ausstellers. Ein Attest nennt seinen
/// Aussteller als Kennung, nicht als Schlüssel; die Zuordnung entsteht
/// im Probelauf aus den Namen, die der Koordinator ohnehin verteilt.
///
/// **Fehlt ein Name, werden dessen Atteste verworfen** und das
/// Protokoll sagt es genau so. Ohne Liste werden alle verworfen: Das
/// ist der sichere Vorgabefall, denn ungeprüfte Atteste durchzulassen
/// wäre schlechter als gar keine (Sicherheitsaudit A10).
pub fn frage_teilnehmer(eigener: &str) -> Vec<String> {
    println!();
    println!("  Namen ALLER Teilnehmer, durch Komma getrennt.");
    println!("  Sie stehen in der Einladung des Koordinators. Beispiel:");
    println!("    anlaufstelle, maschine-b, maschine-c");
    println!("  Ohne Angabe werden Latenz-Atteste nicht geprüft und verworfen.");
    let roh = frage("Teilnehmer", eigener);
    let mut namen: Vec<String> = roh
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    // Der eigene Name gehört immer dazu: Ein Knoten, der seine eigenen
    // Atteste nicht anerkennt, wäre schwer zu erklären.
    if !namen.iter().any(|n| n == eigener) {
        namen.push(eigener.to_string());
    }
    namen
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

/// Ablage der privaten Knotenschlüssel.
pub const SCHLUESSELORDNER: &str = "TESTCLIENT/Schluessel";

/// Die Schlüsseldatei eines Knotens.
///
/// **Über die Repository-Wurzel aufgelöst, nicht relativ.** Bis zum
/// 2026-08-24 stand hier ein relativer Pfad, und der landete dort, wo
/// der Client gestartet wurde: beim Doppelklick in der Wurzel des
/// Repositoriums. Dort stand die Datei in keiner `.gitignore` und
/// konnte in einen Commit geraten. **Wer den Schlüssel hat, kann im
/// Netz als dieser Knoten auftreten**, das ist kein Ordnungsproblem.
///
/// Der Ordner schließt seinen eigenen Inhalt aus, und `*.key` steht
/// zusätzlich in der Wurzel-`.gitignore` für den Fall, dass jemand
/// `myl-node` von Hand startet.
pub fn schluesseldatei(name: &str) -> PathBuf {
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    // Derselbe Schutz wie beim Protokollnamen: Der Name kommt aus einer
    // Eingabe, und ein Schrägstrich darin schriebe in einen fremden
    // Ordner.
    let sicher: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let sicher = if sicher.is_empty() { "knoten".to_string() } else { sicher };
    repo.join(SCHLUESSELORDNER).join(format!("{sicher}.key"))
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

        // ⚑ Erst auf QUIC warten, dann ausgeben.
        //
        // TCP horcht schneller. Wer bei der ersten Adresse aufhört,
        // zeigt nur die TCP-Adresse an, der Betreiber gibt genau die
        // weiter, und das ganze Netz läuft über TCP. Der Rat, die
        // quic-v1-Adresse zu verteilen, wäre dann unbefolgbar, weil sie
        // nie auf dem Bildschirm stünde.
        let hat_quic = knoten.warte_auf_quic(Duration::from_secs(8)).await;
        if knoten.warte_auf_adresse(Duration::from_secs(8)).await.is_some() {
            println!();
            println!("  Erreichbar unter:");
            // QUIC zuerst: Der Transport folgt der Adresse, die
            // weitergegeben wird. Wer eine /tcp/-Adresse verteilt,
            // bekommt ein reines TCP-Netz, und über UDP gelingt das
            // Lochstanzen durch NAT deutlich zuverlässiger. Die
            // Reihenfolge ist die Empfehlung, und der Hinweis darunter
            // sagt sie auch aus.
            let mut adressen = knoten.adressen();
            adressen.sort_by_key(|a| !myl_net::ist_quic(a));
            for a in &adressen {
                println!("    {a}");
            }
            if hat_quic {
                println!();
                println!("  Für Läufe über das Internet die **erste** Adresse");
                println!("  (quic-v1) weitergeben: Über UDP gelingt der Durchstich");
                println!("  durch Heimrouter deutlich zuverlässiger als über TCP.");
            } else {
                println!();
                println!("  ⚠ Keine quic-v1-Adresse gemeldet. Über TCP allein gelingt");
                println!("    der Durchstich durch Heimrouter oft nicht. Ist UDP auf");
                println!("    diesem Port freigegeben?");
            }
        } else {
            println!();
            println!("  Noch keine eigene Adresse gemeldet. Das ist hinter einem");
            println!("  Router normal, solange ein Relais angegeben wurde.");
        }
        println!();
        println!("  Läuft für {} Sekunden. Strg-C bricht ab.", laufzeit.as_secs());
        println!();

        // Auch hier über `laufen_bis`: Wer im Menü Strg-C drückt, soll
        // ein Protokoll bekommen, das sagt, dass er es war.
        knoten.laufen_bis(Some(laufzeit)).await;

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
    let teilnehmer = frage_teilnehmer(&name);

    let konfig = KnotenKonfig {
        name: name.clone(),
        schluesseldatei: schluesseldatei(&name),
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
        // Die Anlaufstelle baut die Blöcke. **Genau einer im Netz**:
        // Zwei Erzeuger gabeln die Kette sofort, weil niemand
        // entscheidet, welcher Block gilt. Das täte eine
        // Abstimmungsrunde, und die gibt es noch nicht.
        erzeugt_bloecke: true,
        teilnehmer,
        // Kein Mitstimmen: Der Testclient fährt keine BFT-Runden.
        // Dafür bräuchte er eine Genesis-Datei mit dem Validator-Satz,
        // und die entsteht nicht nebenbei aus einer Einladung.
        kettendatei: None,
        genesisdatei: None,
        konsensschluesseldatei: None,
    };

    println!();
    println!("  Sobald der Knoten läuft, erscheinen unten seine Adressen.");
    println!("  Die **erste** (quic-v1) an alle Teilnehmer schicken, vollständig,");
    println!("  der Teil ab /p2p/ gehört dazu. Sie ist die Einladung ins Netz.");

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
    let teilnehmer = frage_teilnehmer(&name);

    let konfig = KnotenKonfig {
        name: name.clone(),
        schluesseldatei: schluesseldatei(&name),
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
        // Teilnehmer erzeugen nicht, sie schicken Transaktionen und
        // rechnen die Blöcke der Anlaufstelle nach.
        erzeugt_bloecke: false,
        teilnehmer,
        kettendatei: None,
        genesisdatei: None,
        konsensschluesseldatei: None,
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
    fn eine_teilnehmerliste_ergibt_einen_pruefbaren_satz() {
        // Der Zweck: Ohne Liste wird jedes Attest verworfen, mit Liste
        // werden die eigenen anerkannt (Sicherheitsaudit A10).
        let satz = myl_node::Validatorsatz::aus_namen(&["alpha", "beta"]);
        assert_eq!(satz.anzahl(), 2);
        assert_eq!(myl_node::Validatorsatz::leer().anzahl(), 0);
    }

    #[test]
    fn der_schluessel_liegt_im_schluesselordner() {
        // Nicht dort, wo der Client gestartet wurde: Beim Doppelklick
        // wäre das die Wurzel des Repositoriums, und dort stand die
        // Datei in keiner .gitignore.
        let p = schluesseldatei("alpha");
        assert!(
            p.to_string_lossy().contains("TESTCLIENT/Schluessel"),
            "Schlüssel landet außerhalb des Schlüsselordners: {}",
            p.display()
        );
        assert!(p.ends_with("alpha.key"));
    }

    #[test]
    fn ein_gefaehrlicher_knotenname_bricht_nicht_aus() {
        // Der Name kommt aus einer Eingabe. Ein Schrägstrich darin
        // schriebe in einen fremden Ordner.
        let p = schluesseldatei("../../.ssh/id_rsa");
        let text = p.to_string_lossy().to_string();
        assert!(text.contains("TESTCLIENT/Schluessel"), "{text}");
        assert!(!text.contains(".."), "{text}");
        assert!(!text.contains(".ssh"), "{text}");
    }

    #[test]
    fn ein_leerer_name_ergibt_trotzdem_einen_pfad() {
        let p = schluesseldatei("");
        assert!(p.ends_with("knoten.key"), "{}", p.display());
    }

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
