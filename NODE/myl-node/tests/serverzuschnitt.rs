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
    // Ohne sie faende `SYSTEM/full-build` auch ein `SYSTEM/full-build-x`, und die
    // Gegenprobe blieb genau daran stumm: Eine Teilzeichenkette ist
    // dieselbe Antwort auf eine andere Frage.
    // ⛔️ **Drei Funde beim ersten echten `nix build` (2026-11-19)**, im
    // Behaelter `nixos/nix` gegen das festgehaltene nixpkgs: Ohne `cargoRoot`
    // fand der Bau die Sperrdatei nicht; ohne die Ausnahme fuer den Katalog
    // fehlte eine eingebettete Datei; und ohne die Vektoren im Paket lief der
    // Dienst des Moduls nur mit `--ohne-konformitaet`.
    assert!(flake.contains("cargoRoot = \"NODE/myl-node\";"), "der Flake fehlt `cargoRoot`");
    assert!(flake.contains("\"MODELS/llm/KATALOG.json\""), "der Quellfilter laesst den Katalog nicht durch");
    assert!(
        flake.contains("$out/bin/conformance/vectors/op"),
        "das Paket legt die Konformitaetsvektoren nicht neben das Programm"
    );
    // Gefiltert wird nach dem Praefix: Ein Vergleich mit dem ersten Pfadteil
    // traf `SYSTEM/full-build` nie.
    assert!(flake.contains("pkgs.lib.hasPrefix (d + \"/\") rel"), "der Quellfilter vergleicht nicht nach dem Praefix");

    for ort in ["SYSTEM/full-build", "MODELS", "INTEGER_LLM/artifacts"] {
        assert!(
            flake.contains(&format!("\"{ort}\"")),
            "der Quellfilter der Flake nennt `{ort}` nicht; das Bauverzeichnis ginge mit in den Store"
        );
    }
}

// ---------------------------------------------------------------------
// Der Dienst fuer gewoehnliches Linux (`myl-einstieg.service`) und sein
// Einrichtungsskript, seit 2026-11-19. Dieselben Zusagen wie das Modul,
// dazu, dass Unit und Skript dieselben Pfade und Namen meinen.
// ---------------------------------------------------------------------

/// Die Befehlszeile der Unit: `ExecStart=` samt Fortsetzungszeilen.
fn unit_befehlszeile(unit: &str) -> String {
    let ab = unit.find("\nExecStart=").expect("kein ExecStart in der Unit");
    let mut aus = String::new();
    for zeile in unit[ab + 1..].lines() {
        aus.push_str(zeile.trim_end_matches('\\'));
        aus.push(' ');
        if !zeile.trim_end().ends_with('\\') {
            break;
        }
    }
    aus
}

fn unit_flags(zeile: &str) -> Vec<String> {
    let mut f: Vec<String> = zeile
        .split_whitespace()
        .filter(|w| w.starts_with("--") && w.len() > 2)
        .map(str::to_string)
        .collect();
    f.sort();
    f.dedup();
    f
}

/// Der Abschnitt `[Name]` der Unit bis zum naechsten.
fn unit_abschnitt<'a>(unit: &'a str, name: &str) -> &'a str {
    let kopf = format!("\n[{name}]\n");
    let ab = unit.find(&kopf).unwrap_or_else(|| panic!("kein Abschnitt [{name}]")) + kopf.len();
    let rest = &unit[ab..];
    match rest.find("\n[") {
        Some(bis) => &rest[..bis],
        None => rest,
    }
}

/// **Jedes Flag der Unit kennt der Knoten**, wie beim Modul.
#[test]
fn jedes_flag_der_unit_kennt_der_knoten() {
    let unit = lies("../myl-server/myl-einstieg.service");
    let haupt = lies("src/main.rs");
    let uebergeben = unit_flags(&unit_befehlszeile(&unit));
    assert!(uebergeben.len() >= 12, "nur {} Flags in der Unit gefunden", uebergeben.len());
    for f in &uebergeben {
        assert!(
            haupt.contains(&format!("\"{f}\"")),
            "die Unit uebergibt `{f}`, und `myl-node` kennt es nicht"
        );
    }
}

/// **Die Unit oeffnet nach aussen nur den P2P-Port**: keine Tuer, keine
/// Ortsleitung, die Beobachtung auf der Rueckschleife.
#[test]
fn die_unit_oeffnet_nur_den_p2p_port() {
    let unit = lies("../myl-server/myl-einstieg.service");
    let zeile = unit_befehlszeile(&unit);
    let uebergeben = unit_flags(&zeile);
    for verboten in ["--tuer", "--ortsleitung", "--ortsausweis"] {
        assert!(!uebergeben.iter().any(|f| f == verboten), "die Unit uebergibt `{verboten}`");
    }
    assert!(uebergeben.iter().any(|f| f == "--ohne-tuer"), "ohne `--ohne-tuer` oeffnet der Knoten seine Tuer");
    assert!(zeile.contains("--beobachtung 127.0.0.1:"), "die Beobachtung nicht auf der Rueckschleife");
    assert!(zeile.contains("--netz ${NETZ}"), "ohne `--netz` laeuft der Dienst auf der Probekette");
}

/// **Die Startbegrenzung haengt an `[Unit]`**, wo sie wirkt.
#[test]
fn die_unit_begrenzt_neustarts_an_der_richtigen_stelle() {
    let unit = lies("../myl-server/myl-einstieg.service");
    for feld in ["StartLimitIntervalSec=", "StartLimitBurst="] {
        assert!(unit_abschnitt(&unit, "Unit").contains(feld), "`{feld}` fehlt in [Unit]");
        assert!(!unit_abschnitt(&unit, "Service").contains(feld), "`{feld}` steht in [Service] und wirkt dort nicht");
    }
}

/// **Unit und Modul haerten gleich**, und beide erlauben Netlink.
///
/// ⛔️ **Gefunden am 2026-11-19 im Behaelter mit systemd:** Ohne
/// `AF_NETLINK` endet der Start mit „Address family not supported by
/// protocol“ (libp2p beobachtet die Schnittstellen ueber Netlink). Das
/// Modul trug denselben Fehler, ungeprueft seit dem 2026-09-14.
#[test]
fn unit_und_modul_haerten_gleich_und_erlauben_netlink() {
    let unit = lies("../myl-server/myl-einstieg.service");
    let modul = lies("../myl-server/modul.nix");
    let dienst = unit_abschnitt(&unit, "Service");
    for direktive in [
        "NoNewPrivileges", "ProtectSystem", "ProtectHome", "PrivateTmp", "PrivateDevices",
        "ProtectKernelTunables", "ProtectKernelModules", "ProtectKernelLogs",
        "ProtectControlGroups", "ProtectClock", "ProtectHostname", "ProtectProc", "ProcSubset",
        "RestrictAddressFamilies", "RestrictNamespaces", "RestrictRealtime", "RestrictSUIDSGID",
        "LockPersonality", "MemoryDenyWriteExecute", "SystemCallArchitectures",
        "SystemCallFilter", "CapabilityBoundingSet", "AmbientCapabilities", "UMask",
        "LimitNOFILE", "TasksMax",
    ] {
        assert!(modul.contains(&format!("{direktive} = ")), "das Modul setzt `{direktive}` nicht");
        assert!(dienst.contains(&format!("\n{direktive}=")), "die Unit setzt `{direktive}` nicht");
    }
    let familien = dienst.lines().find(|z| z.starts_with("RestrictAddressFamilies=")).expect("Familien");
    assert!(familien.contains("AF_NETLINK"), "die Unit verbietet Netlink: {familien}");
    assert!(modul.contains("\"AF_NETLINK\""), "das Modul verbietet Netlink");
    assert!(befehlszeile(&modul).contains("\"--netz\" cfg.netz"), "das Modul uebergibt kein Netz");
}

/// **Skript und Unit meinen dieselben Pfade und Variablen.** Jede
/// Variable der Befehlszeile schreibt das Skript in die Einstellungen,
/// und das Programm liegt dort, wo die Unit es startet.
#[test]
fn skript_und_unit_passen_zusammen() {
    let unit = lies("../myl-server/myl-einstieg.service");
    let skript = lies("../myl-server/einstieg-einrichten.sh");
    let zeile = unit_befehlszeile(&unit);
    for var in ["NAME", "NETZ", "PORT", "OEFFENTLICH", "ZUSATZ"] {
        assert!(zeile.contains(&format!("${var}")) || zeile.contains(&format!("${{{var}}}")), "die Unit nutzt `{var}` nicht");
        assert!(skript.contains(&format!("\n{var}=\"$")), "das Skript schreibt `{var}` nicht in die Einstellungen");
    }
    let programm = zeile.split_whitespace().next().expect("Programm").trim_start_matches("ExecStart=");
    assert_eq!(programm, "/usr/local/lib/myelith/myl-node");
    assert!(skript.contains("ZIEL=/usr/local/lib/myelith"), "das Skript legt das Programm woanders hin");
    assert!(unit.contains("EnvironmentFile=/etc/myl-einstieg/einstieg.env"));
    assert!(skript.contains("KONF=/etc/myl-einstieg"));
    assert!(unit.contains("StateDirectory=myl-einstieg") && skript.contains("ZUSTAND=/var/lib/myl-einstieg"));
    assert!(unit.contains("User=myl-einstieg") && skript.contains("NUTZER=myl-einstieg"));
}
