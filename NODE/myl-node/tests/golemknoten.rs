//! **Haelt den Knotendienst von GolemOS an den Knoten und an das
//! NixOS-Modul.**
//!
//! # ⛔️ Warum es diese Datei gibt
//!
//! Seit GolemOS 0.3.0 gibt es zwei Wege, `myl-node` als Dienst zu fahren:
//! das NixOS-Modul unter `NODE/myl-server/` und `golem-knoten` in GolemOS.
//! **Beide bauen dieselbe Befehlszeile, an zwei Orten, in zwei Sprachen.**
//! Was an zwei Orten steht, laeuft auseinander, und der zweite Ort meldet
//! sich nicht. Diese Proben halten fest:
//!
//! - jedes Flag, das `golem-knoten` uebergibt, kennt der Knoten;
//! - beide Dienste uebergeben dieselben Flags, und beide halten Tuer und
//!   Ortsleitung zu;
//! - die Startbegrenzung ist in beiden dieselbe;
//! - und `golem-knoten` selbst prueft, was es einliest (nur unter Unix,
//!   denn es ist ein Shell-Skript).

// 📌 Nur `PathBuf` steht hier oben. `Path` und `Command` braucht allein
// das Modul `lauf` unter `#[cfg(unix)]`; oben eingebunden waren sie unter
// Windows unbenutzt, und Clippy mit `-D warnings` brach dort ab.
use std::path::PathBuf;

/// Wo das Skript im Repositorium liegt, gegen das Manifest aufgeloest.
const SKRIPT: &str = "../../SYSTEM/golemos/overlay/usr/sbin/golem-knoten";

fn wurzel() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn lies(rel: &str) -> String {
    let p = wurzel().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Der Teil des Skripts, der die Befehlszeile baut, zwischen seinen Marken.
fn befehlszeile_im_skript(skript: &str) -> String {
    let ab = skript.find("# >>> befehlszeile").expect("Anfangsmarke fehlt");
    let bis = skript.find("# <<< befehlszeile").expect("Endmarke fehlt");
    skript[ab..bis].to_string()
}

/// Der Teil des Moduls, der zur Befehlszeile wird (wie in
/// `serverzuschnitt.rs`).
fn befehlszeile_im_modul(modul: &str) -> String {
    let ab = modul.find("ExecStart = lib.escapeShellArgs (").expect("kein ExecStart im Modul");
    let rest = &modul[ab..];
    let bis = rest.find("\n        );").expect("ExecStart ohne Ende");
    rest[..bis].to_string()
}

/// Alle `"--flag` in einem Ausschnitt, sortiert und ohne Doppelte.
///
/// ⚑ **Nur in Anfuehrungszeichen**: Im Skript stehen Flags auch in
/// Kommentaren („Tuer und Ortsleitung werden NICHT uebergeben"), und wer
/// die erwischt, liest das Gegenteil heraus.
fn flags(text: &str) -> Vec<String> {
    let mut aus: Vec<String> = text
        .match_indices("\"--")
        .map(|(i, _)| {
            text[i + 1..].chars().take_while(|c| *c == '-' || c.is_ascii_alphanumeric()).collect::<String>()
        })
        .filter(|f| f.len() > 2)
        .collect();
    aus.sort();
    aus.dedup();
    aus
}

#[test]
fn jedes_flag_von_golem_knoten_kennt_der_knoten() {
    let uebergeben = flags(&befehlszeile_im_skript(&lies(SKRIPT)));
    let haupt = lies("src/main.rs");
    assert!(uebergeben.len() >= 10, "nur {} Flags gefunden; liest die Probe sie noch?", uebergeben.len());
    for f in &uebergeben {
        assert!(
            haupt.contains(&format!("\"{f}\"")),
            "golem-knoten uebergibt `{f}`, und `myl-node` kennt es nicht: der Waechter haelt nach fuenf Abbruechen an"
        );
    }
}

/// ⚑ **Beide Dienste uebergeben dieselben Flags.** Kommt in einem ein
/// Flag dazu, gehoert es in den anderen, oder die Abweichung wird hier
/// ausdruecklich benannt.
#[test]
fn golem_knoten_und_modul_uebergeben_dieselben_flags() {
    let skript = flags(&befehlszeile_im_skript(&lies(SKRIPT)));
    let modul = flags(&befehlszeile_im_modul(&lies("../myl-server/modul.nix")));
    assert_eq!(skript, modul, "GolemOS und NixOS starten den Knoten verschieden");
    for verboten in ["--tuer", "--ortsleitung", "--ortsausweis"] {
        assert!(!skript.iter().any(|f| f == verboten), "golem-knoten uebergibt `{verboten}`");
    }
    let zeile = befehlszeile_im_skript(&lies(SKRIPT));
    assert!(zeile.contains("\"127.0.0.1:$beobachtung\""), "die Beobachtung liegt nicht fest auf der Rueckschleife");
}

/// Eine Zahl aus einer Zuweisung `NAME=zahl` oder `name = zahl;`.
fn zahl_nach(text: &str, name: &str) -> u64 {
    let ab = text.find(name).unwrap_or_else(|| panic!("`{name}` fehlt")) + name.len();
    text[ab..]
        .trim_start_matches([' ', '=', '"'])
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or_else(|_| panic!("`{name}` traegt keine Zahl"))
}

/// ⛔️ **Ein Startfehler wird in beiden keine Schleife**, und zwar mit
/// derselben Grenze: fuenf Abbrueche in fuenf Minuten, zehn Sekunden Pause.
#[test]
fn die_startbegrenzung_ist_dieselbe() {
    let skript = lies(SKRIPT);
    let modul = lies("../myl-server/modul.nix");
    assert_eq!(zahl_nach(&skript, "GRENZE_ABBRUECHE"), zahl_nach(&modul, "startLimitBurst"));
    assert_eq!(zahl_nach(&skript, "GRENZE_FENSTER"), zahl_nach(&modul, "startLimitIntervalSec"));
    assert_eq!(zahl_nach(&skript, "PAUSE"), zahl_nach(&modul, "RestartSec"));
    let option = &modul[modul.find("aufnahmeSekunden = lib.mkOption").expect("aufnahmeSekunden fehlt")..];
    assert_eq!(zahl_nach(&skript, "\naufnahme="), zahl_nach(option, "default"), "die Vorgabe der Aufnahmen weicht ab");
}

/// Die feste Kennung des Dienstkontos steht im Skript und in der
/// Benutzertabelle des Abbilds; beide muessen gleich sein, sonst gehoert
/// der Zustand niemandem.
#[test]
fn das_dienstkonto_hat_ueberall_dieselbe_kennung() {
    let skript = lies(SKRIPT);
    let tabelle = lies("../../SYSTEM/golemos/konfigurationen/benutzer.tabelle");
    let zeile = tabelle.lines().find(|z| z.starts_with("myl-knoten ")).expect("myl-knoten fehlt in der Tabelle");
    let felder: Vec<&str> = zeile.split_whitespace().collect();
    let id = zahl_nach(&skript, "KONTO_ID").to_string();
    assert_eq!((felder[1], felder[3]), (id.as_str(), id.as_str()), "{zeile}");
}

#[cfg(unix)]
mod lauf {
    use super::*;
    use std::path::Path;
    use std::process::Command;

    /// Ein Zwischenordner fuer eine Probe, ohne fremde Kiste.
    struct Ordner(PathBuf);
    impl Ordner {
        fn neu(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("golemknoten-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).expect("Zwischenordner");
            Self(p)
        }
    }
    impl Drop for Ordner {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Ruft das Skript mit allen Orten im Zwischenordner.
    fn rufe(o: &Path, args: &[&str]) -> (bool, String) {
        let aus = Command::new("sh")
            .arg(wurzel().join(SKRIPT))
            .args(args)
            .env("GOLEM_KNOTEN_KONF", o.join("knoten.conf"))
            .env("GOLEM_KNOTEN_STAND", o.join("stand"))
            .env("GOLEM_KNOTEN_LAUF", o.join("lauf"))
            .env("GOLEM_KNOTEN_KLON", o.join("wurzel"))
            .env("GOLEM_KNOTEN_PROGRAMM", "/usr/bin/myl-node")
            .output()
            .expect("sh");
        let text = String::from_utf8_lossy(&aus.stdout).to_string() + &String::from_utf8_lossy(&aus.stderr);
        (aus.status.success(), text)
    }

    /// Ein Myelith-Ordner mit Konformitaetsvektoren, wie S06myelith ihn
    /// findet.
    fn klon(o: &Path) -> PathBuf {
        let k = o.join("klon");
        std::fs::create_dir_all(k.join("INTEGER_LLM/conformance/vectors/op")).expect("Klon");
        std::fs::write(o.join("wurzel"), k.display().to_string()).expect("wurzel");
        k
    }

    #[test]
    fn eingerichtet_startet_er_mit_dem_zuschnitt_des_moduls() {
        let o = Ordner::neu("zuschnitt");
        let k = klon(&o.0);
        let (ok, text) = rufe(&o.0, &["einrichten", "--name", "golem-1", "--bootstrap", "/dns4/a.example/tcp/4001/p2p/12D3Koo"]);
        assert!(ok, "{text}");
        let (ok, zeile) = rufe(&o.0, &["befehlszeile"]);
        assert!(ok, "{zeile}");
        let args: Vec<&str> = zeile.lines().collect();
        let nach = |f: &str| args.iter().position(|a| *a == f).map(|i| args[i + 1]);
        assert_eq!(nach("--name"), Some("golem-1"));
        assert_eq!(nach("--rolle"), Some("teilnehmer"));
        assert_eq!(nach("--port"), Some("4001"));
        assert_eq!(nach("--beobachtung"), Some("127.0.0.1:4151"));
        assert_eq!(nach("--aufnahme"), Some("300"));
        assert_eq!(nach("--bootstrap"), Some("/dns4/a.example/tcp/4001/p2p/12D3Koo"));
        let stand = o.0.join("stand");
        assert_eq!(nach("--schluessel").map(PathBuf::from), Some(stand.join("identitaet.key")), "der Schluessel liegt im Zustand");
        assert_eq!(
            nach("--konformitaet").map(PathBuf::from),
            Some(k.join("INTEGER_LLM/conformance/vectors/op")),
            "die Vektoren kommen aus dem gefundenen Myelith-Ordner"
        );
        for f in ["--tuer", "--ortsleitung", "--erzeuger", "--ohne-konformitaet", "--konsensschluessel"] {
            assert!(!args.contains(&f), "`{f}` ohne Auftrag in der Befehlszeile: {zeile}");
        }
        let rechte = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(o.0.join("knoten.conf")).expect("Einstellung").permissions());
        assert_eq!(rechte & 0o077, 0, "die Einstellung ist fuer andere lesbar: {rechte:o}");
        let rechte = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&stand).expect("Zustand").permissions());
        assert_eq!(rechte & 0o777, 0o700, "der Zustand ist nicht 0700: {rechte:o}");
    }

    /// ⛔️ **Was nicht passt, kommt nicht hinein**: kein Leerzeichen, kein
    /// eingeschmuggeltes Flag, keine freie Rolle, kein Port unter 1025,
    /// kein Feld, das es nicht gibt, kein Stimmsatz ohne Schluessel.
    #[test]
    fn falsche_werte_werden_abgewiesen() {
        let o = Ordner::neu("abweisen");
        klon(&o.0);
        for args in [
            &["einrichten", "--name", "a b"][..],
            &["einrichten", "--name", "--tuer"],
            &["einrichten", "--port", "80"],
            &["einrichten", "--port", "4001x"],
            &["einrichten", "--rolle", "validator"],
            &["einrichten", "--bootstrap", "-x"],
            &["einrichten", "--bootstrap", "/dns4/a;reboot"],
            &["einrichten", "--tuer", "0.0.0.0:4150"],
            &["einrichten", "--konsens", "ja"],
            &["einrichten", "--stimmsatz", "/daten/stimmsatz.txt"],
            &["einrichten", "--konformitaet", "relativ/pfad"],
        ] {
            let (ok, text) = rufe(&o.0, args);
            assert!(!ok, "angenommen: {args:?}\n{text}");
            assert!(!o.0.join("knoten.conf").exists(), "trotz Abweisung geschrieben: {args:?}");
        }
    }

    /// ⛔️ **Die Einstellung wird gelesen, nicht ausgefuehrt.** Eine Zeile,
    /// die ein Programm waere, und ein Feld, das es nicht gibt, halten den
    /// Bau der Befehlszeile an, statt etwas zu tun.
    #[test]
    fn eine_untergeschobene_einstellung_wird_nicht_ausgefuehrt() {
        let o = Ordner::neu("untergeschoben");
        klon(&o.0);
        let zeuge = o.0.join("ausgefuehrt");
        for zeile in [
            format!("name=x;touch {}", zeuge.display()),
            format!("name=$(touch {})", zeuge.display()),
            "zusatz=--tuer".to_string(),
        ] {
            std::fs::write(o.0.join("knoten.conf"), format!("{zeile}\n")).expect("Einstellung");
            let (ok, text) = rufe(&o.0, &["befehlszeile"]);
            assert!(!ok, "angenommen: {zeile}\n{text}");
            assert!(!zeuge.exists(), "die Einstellung wurde ausgefuehrt: {zeile}");
        }
    }

    /// ⚑ **Ohne Myelith-Ordner startet der Knoten nicht still ohne
    /// Pruefung**, sondern sagt, was fehlt; ohne Pruefung nur auf
    /// ausdrueckliches `aus`.
    #[test]
    fn ohne_vektoren_nur_auf_ausdruecklichen_wunsch() {
        let o = Ordner::neu("ohnevektoren");
        let (ok, text) = rufe(&o.0, &["einrichten"]);
        assert!(ok, "{text}");
        let (ok, text) = rufe(&o.0, &["befehlszeile"]);
        assert!(!ok && text.contains("Konformitaetsvektoren"), "{text}");
        let (ok, text) = rufe(&o.0, &["einrichten", "--konformitaet", "aus"]);
        assert!(ok && text.contains("Probelauf"), "{text}");
        let (ok, zeile) = rufe(&o.0, &["befehlszeile"]);
        assert!(ok && zeile.lines().any(|a| a == "--ohne-konformitaet"), "{zeile}");
    }

    /// Der Konsensschluessel geht in den Zustand, nur fuer das Konto lesbar,
    /// und erst danach darf ein Stimmsatz dazu.
    #[test]
    fn der_konsensschluessel_kommt_in_den_zustand() {
        let o = Ordner::neu("konsens");
        klon(&o.0);
        let vorlage = o.0.join("konsens-vorlage.key");
        std::fs::write(&vorlage, "geheim").expect("Vorlage");
        assert!(rufe(&o.0, &["einrichten"]).0);
        let (ok, text) = rufe(&o.0, &["konsensschluessel", &vorlage.display().to_string()]);
        assert!(ok && text.contains("loeschen"), "{text}");
        let ziel = o.0.join("stand/konsens.key");
        assert_eq!(std::fs::read_to_string(&ziel).expect("Schluessel"), "geheim");
        let rechte = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&ziel).expect("Schluessel").permissions());
        assert_eq!(rechte & 0o777, 0o400, "{rechte:o}");
        let (ok, text) = rufe(&o.0, &["einrichten", "--stimmsatz", "/daten/stimmsatz.txt"]);
        assert!(ok, "{text}");
        let (_, zeile) = rufe(&o.0, &["befehlszeile"]);
        assert!(zeile.contains("--konsensschluessel\n") && zeile.contains("--stimmsatz\n/daten/stimmsatz.txt"), "{zeile}");
    }
}
