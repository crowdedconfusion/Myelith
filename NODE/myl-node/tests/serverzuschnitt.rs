//! **Haelt das NixOS-Modul an den Knoten, den es startet.**
//!
//! # ⛔️ Warum es diese Datei gibt
//!
//! Das Modul unter `NODE/myl-server/` beschreibt einen systemd-Dienst,
//! der `myl-node` mit einer Handvoll Flags aufruft. **Kein Bau und
//! keine Pruefung dieses Projekts fasst die beiden bisher zusammen.**
//! Wer hier eine Option umbenennt, merkt es auf einem NixOS-Server, und
//! zwar daran, dass der Dienst startet, faellt und wieder startet.
//!
//! ⚑ **Ohne `nix` und ohne NixOS.** Diese Pruefung baut nichts und
//! evaluiert nichts; sie liest zwei Dateien nebeneinander. Genau
//! deshalb laeuft sie ueberall: auf der Entwicklungsmaschine, auf der
//! kein `nix` liegt, und in der Baupruefung, die kein NixOS ist.
//!
//! ⚠️ **Was sie NICHT sagt:** ob das Modul auf NixOS baut. Das bleibt
//! der VM-Pruefung vorbehalten, und solange die nicht gelaufen ist,
//! steht es als Warnung im Kopf des Moduls. Diese Pruefung faengt die
//! eine Fehlerklasse, die sich ohne NixOS faengt, und behauptet keine
//! zweite.

use std::path::{Path, PathBuf};

fn wurzel() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn lies(rel: &str) -> String {
    let p = wurzel().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Der Teil des Moduls, der wirklich zur Befehlszeile wird.
///
/// ⚑ **Nur dieser Ausschnitt, und das ist der ganze Trick.** Im Modul
/// stehen Flagnamen auch in Beschreibungstexten und in der Liste der
/// heiklen Flags, vor denen es **warnt**. Wer die ganze Datei
/// durchsucht, findet `--tuer` und schliesst daraus, das Modul oeffne
/// die Tuer; tatsaechlich steht es dort, weil es davor warnt. **Eine
/// Pruefung, die den Ort nicht kennt, liest das Gegenteil heraus.**
fn befehlszeile(modul: &str) -> String {
    let ab = modul
        .find("ExecStart = lib.escapeShellArgs (")
        .expect("kein ExecStart im Modul");
    let rest = &modul[ab..];
    let bis = rest.find("\n        );").expect("ExecStart ohne Ende");
    rest[..bis].to_string()
}

/// Alle `--flag`, die in einem Ausschnitt als Zeichenkette stehen.
fn flags(text: &str) -> Vec<String> {
    let mut aus = Vec::new();
    let zeichen: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i + 3 < zeichen.len() {
        if zeichen[i] == '"' && zeichen[i + 1] == '-' && zeichen[i + 2] == '-' {
            let name: String = zeichen[i + 1..]
                .iter()
                .take_while(|c| **c == '-' || c.is_ascii_alphanumeric())
                .collect();
            if name.len() > 2 {
                aus.push(name);
            }
        }
        i += 1;
    }
    aus.sort();
    aus.dedup();
    aus
}

/// **Jedes Flag, das der Dienst uebergibt, kennt der Knoten.**
///
/// ⚑ **Das haeufigste Fehlerbild dieses Projekts, in einer neuen
/// Sprache.** Ein Flag, das der Knoten nicht kennt, ist kein Tippfehler
/// mit einer freundlichen Meldung: `myl-node` bricht mit einem Fehler
/// ab, systemd startet neu, und der Betreiber sieht einen Dienst, der
/// dauernd hochkommt.
#[test]
fn jedes_flag_des_dienstes_kennt_der_knoten() {
    let modul = lies("../myl-server/modul.nix");
    let haupt = lies("src/main.rs");
    let uebergeben = flags(&befehlszeile(&modul));

    assert!(
        uebergeben.len() >= 10,
        "nur {} Flags in der Befehlszeile gefunden; liest die Pruefung sie noch?",
        uebergeben.len()
    );

    for f in &uebergeben {
        // So steht ein Zweig im Aufrufparser: `"--name" => {` oder
        // `"--stimmsatzzeile" | "--genesiszeile" => {`.
        assert!(
            haupt.contains(&format!("\"{f}\"")),
            "der Dienst uebergibt `{f}`, und `myl-node` kennt es nicht: \
             der Dienst startet, bricht ab und startet wieder"
        );
    }
}

/// **Der Dienst bindet nichts nach aussen ausser dem P2P-Port.**
///
/// ⚑ **Die tragende Zusage des ganzen Moduls**, und sie steht sonst nur
/// in einem Kommentar. Tuer und Ortsleitung werden gar nicht
/// uebergeben; die Beobachtung wird uebergeben und traegt ausdruecklich
/// die Rueckschleife.
///
/// ⛔️ **Warum das bei einem stimmberechtigten Knoten mehr ist als eine
/// Vorsichtsmassnahme:** Aus einem Ueberlastangriff gegen eine offene
/// Tuer wird einer gegen die Lebendigkeit des Konsenses.
#[test]
fn nach_aussen_geht_nur_der_p2p_port() {
    let modul = lies("../myl-server/modul.nix");
    let zeile = befehlszeile(&modul);
    let uebergeben = flags(&zeile);

    for verboten in ["--tuer", "--ortsleitung", "--ortsausweis"] {
        assert!(
            !uebergeben.iter().any(|f| f == verboten),
            "der Dienst uebergibt `{verboten}` und verlaesst damit seinen Zuschnitt"
        );
    }

    // ⚑ **Die Beobachtung ist der Sonderfall:** Sie wird uebergeben,
    // damit ein Betreiber die Zusage in `systemctl cat` sieht, und die
    // Adresse ist im Modul fest.
    if uebergeben.iter().any(|f| f == "--beobachtung") {
        assert!(
            zeile.contains("\"127.0.0.1:"),
            "die Beobachtung wird uebergeben, aber nicht auf der Rueckschleife"
        );
        for offen in ["0.0.0.0", "::", "${cfg.beobachtungAdresse"] {
            assert!(
                !zeile.contains(&format!("\"{offen}")),
                "die Beobachtungsadresse ist nicht mehr fest auf der Rueckschleife: {offen}"
            );
        }
    }

    // Und die Firewall oeffnet genau einen Port, den P2P-Port.
    for feld in ["allowedTCPPorts = [ cfg.p2pPort ]", "allowedUDPPorts = [ cfg.p2pPort ]"] {
        assert!(modul.contains(feld), "die Firewall oeffnet nicht genau `{feld}`");
    }
}

/// **Ein Startfehler wird keine Schleife.**
///
/// ⛔️ **Der Punkt war im Entwurf verlangt und im Modul nicht gebaut**
/// (gefunden 2026-09-16). Ohne Begrenzung startet ein Knoten mit einem
/// fehlenden Schluessel alle zehn Sekunden neu, fuer immer, und der
/// Dienst meldet sich dabei als aktiv.
///
/// ⚑ **Und die Begrenzung muss an der Unit haengen, nicht am Dienst.**
/// `StartLimitIntervalSec` gehoert seit systemd 229 nach `[Unit]`; in
/// `[Service]` taete sie stillschweigend nichts. **Eine Begrenzung, die
/// nichts tut, ist schlimmer als keine**, denn sie sieht aus wie eine.
#[test]
fn ein_startfehler_wird_keine_schleife() {
    let modul = lies("../myl-server/modul.nix");
    for feld in ["startLimitIntervalSec", "startLimitBurst"] {
        assert!(modul.contains(feld), "dem Dienst fehlt `{feld}`");
    }
    // Die NixOS-Schreibweise klein, und nicht die systemd-Direktive im
    // `serviceConfig`, wo sie nicht wirkt.
    let ab = modul.find("serviceConfig = {").expect("kein serviceConfig");
    assert!(
        !modul[ab..].contains("StartLimit"),
        "die Startbegrenzung steht im serviceConfig und wirkt dort nicht"
    );
}

/// **Jede Option, die das Beispiel setzt, gibt es im Modul.**
///
/// 📌 **Dieselbe Klasse wie Fund 271**, nur zwischen zwei Nix-Dateien:
/// Eine Beispielkonfiguration, die eine Option setzt, die es nicht
/// gibt, laesst `nixos-rebuild` abbrechen, und zwar bei dem, der sie
/// als Vorbild genommen hat.
#[test]
fn das_beispiel_setzt_nur_optionen_die_es_gibt() {
    let modul = lies("../myl-server/modul.nix");
    let beispiel = lies("../myl-server/beispiel-configuration.nix");

    let ab = beispiel.find("services.myl-server = {").expect("kein Block im Beispiel");
    let block = &beispiel[ab..];
    let bis = block.find("\n  };").expect("Block ohne Ende");
    let block = &block[..bis];

    let mut gefunden = 0;
    for zeile in block.lines() {
        // Auch die auskommentierten: Sie sind das, was jemand als
        // Naechstes einschaltet, und ein Tippfehler darin faellt erst
        // dort auf.
        let z = zeile.trim().trim_start_matches("# ").trim();
        let Some((name, _)) = z.split_once('=') else { continue };
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        if name == "enable" {
            gefunden += 1;
            continue;
        }
        assert!(
            modul.contains(&format!("{name} = lib.mkOption")),
            "das Beispiel setzt `{name}`, und das Modul kennt die Option nicht"
        );
        gefunden += 1;
    }
    assert!(gefunden >= 6, "nur {gefunden} Optionen im Beispiel gefunden; liest die Pruefung sie noch?");
}

/// **Die Flake gibt das Paket heraus, das der Dienst braucht.**
///
/// ⛔️ **Bis zum 2026-09-16 tat sie das nicht**, und damit war das Modul
/// ein Umschlag um „bau es dir selbst": Es verlangt ein Paket mit
/// `bin/myl-node`, und es gab keinen Weg dorthin ausser einem selbst
/// geschriebenen. **Barrierefrei war daran nichts.**
///
/// ⚑ **Geprueft wird, dass die Pfade in der Flake auf Dateien zeigen,
/// die es gibt.** Ein Pfad in Nix, den es nicht gibt, faellt erst beim
/// Evaluieren auf, und das geschieht auf dem Rechner des Betreibers.
#[test]
fn die_flake_gibt_das_paket_und_das_modul_heraus() {
    let flake = std::fs::read_to_string(wurzel().join("../../flake.nix")).expect("flake.nix");

    for ausgabe in ["packages.myl-node", "nixosModules.myl-server"] {
        assert!(flake.contains(ausgabe), "der Flake fehlt die Ausgabe `{ausgabe}`");
    }

    // Die Pfade, auf die sie zeigt, muss es geben.
    let repo = wurzel().join("../..");
    for pfad in ["NODE/myl-server/modul.nix", "NODE/myl-node/Cargo.lock", "NODE/myl-node/Cargo.toml"] {
        assert!(
            Path::new(&repo).join(pfad).exists(),
            "die Flake zeigt auf `{pfad}`, und die Datei gibt es nicht"
        );
        assert!(flake.contains(pfad), "die Flake nennt `{pfad}` nicht mehr");
    }

    // ⚑ **Und die Sperrdatei darf keine Git-Quelle haben.**
    // `cargoLock.lockFile` kommt ohne eine einzige Zahl aus, solange das
    // gilt; jede Git-Quelle braeuchte einen eigenen `outputHashes`, und
    // dann stuende die Zahl wieder da, die dieser Weg gerade vermeidet.
    let sperre = lies("Cargo.lock");
    assert!(
        !sperre.contains("source = \"git+"),
        "die Sperrdatei hat eine Git-Quelle: `cargoLock.lockFile` braucht dann outputHashes"
    );

    // ⚑ **Das grosse Bauverzeichnis bleibt draussen.** Ohne den Filter
    // naehme `src = ./.` zweistellige Gigabytes in den Store, und die
    // Ableitung waere bei jedem Bau eine andere.
    //
    // ⚑ **Mit den Anfuehrungszeichen gesucht, und das ist kein Zierrat.**
    // Ohne sie faende `target-shared` auch ein `target-shared-x`, und die
    // Gegenprobe blieb genau daran stumm: Eine Teilzeichenkette ist
    // dieselbe Antwort auf eine andere Frage.
    for ort in ["target-shared", "MODELS", "INTEGER_LLM/artifacts"] {
        assert!(
            flake.contains(&format!("\"{ort}\"")),
            "der Quellfilter der Flake nennt `{ort}` nicht; das Bauverzeichnis ginge mit in den Store"
        );
    }
}
