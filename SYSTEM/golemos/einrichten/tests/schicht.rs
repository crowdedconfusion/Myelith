//! **Proben fuer `golem-schicht`**, das Shell-Skript der Basis, das die
//! Oberflaechen-Schicht holt, prueft und startet.
//!
//! ⚑ Geprobt wird, was ohne root und ohne GolemOS geht: das Holen mit
//! Pruefsumme. Starten braucht `mount` und `chroot` und wird unter QEMU
//! belegt, nicht hier.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

const SKRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../overlay/usr/sbin/golem-schicht");

struct Ordner(PathBuf);
impl Ordner {
    fn neu(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("golemschicht-{name}-{}", std::process::id()));
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

fn rufe(ziel: &Path, args: &[&str]) -> (bool, String) {
    let aus = Command::new("sh")
        .arg(SKRIPT)
        .args(args)
        .env("GOLEM_SCHICHT_ORDNER", ziel)
        .output()
        .expect("sh");
    (aus.status.success(), String::from_utf8_lossy(&aus.stdout).to_string() + &String::from_utf8_lossy(&aus.stderr))
}

/// Die Architektur, wie das Skript sie in den Dateinamen schreibt.
fn arch() -> String {
    let aus = Command::new("uname").arg("-m").output().expect("uname");
    String::from_utf8_lossy(&aus.stdout).trim().to_string()
}

fn summe(datei: &Path) -> String {
    let aus = Command::new("shasum").args(["-a", "256"]).arg(datei).output().expect("shasum");
    String::from_utf8_lossy(&aus.stdout).split_whitespace().next().expect("Summe").to_string()
}

/// Eine Schicht samt Pruefsumme, wie `schicht-bauen.sh` sie ablegt.
fn quelle(o: &Path, inhalt: &[u8], summe_von: Option<&[u8]>) -> PathBuf {
    let q = o.join("stick/oberflaeche.sqfs");
    std::fs::create_dir_all(q.parent().expect("Ordner")).expect("Stick");
    std::fs::write(&q, inhalt).expect("Schicht");
    let echt = o.join("echt.bin");
    std::fs::write(&echt, summe_von.unwrap_or(inhalt)).expect("Vergleich");
    std::fs::write(q.with_extension("sqfs.sha256"), format!("{}  oberflaeche.sqfs\n", summe(&echt))).expect("Summe");
    q
}

#[test]
fn eine_heile_schicht_wird_uebernommen() {
    let o = Ordner::neu("heil");
    let ziel = o.0.join("schichten");
    let q = quelle(&o.0, b"squashfs-probe", None);
    let (ok, text) = rufe(&ziel, &["holen", &q.display().to_string()]);
    assert!(ok, "{text}");
    let datei = ziel.join(format!("oberflaeche-{}.sqfs", arch()));
    assert_eq!(std::fs::read(&datei).expect("uebernommen"), b"squashfs-probe");
    assert!(datei.with_extension("sqfs.sha256").is_file(), "die Pruefsumme kommt mit, denn starten prueft erneut");
    let (_, status) = rufe(&ziel, &["status"]);
    assert!(status.contains("Pruefsumme stimmt") && !status.contains("NICHT"), "{status}");
}

/// ⛔️ **Eine veraenderte Schicht kommt nicht hinein**, und eine schon
/// geholte, die danach veraendert wurde, meldet `status`.
#[test]
fn eine_falsche_pruefsumme_haelt_alles_an() {
    let o = Ordner::neu("falsch");
    let ziel = o.0.join("schichten");
    let q = quelle(&o.0, b"untergeschoben", Some(b"das Original"));
    let (ok, text) = rufe(&ziel, &["holen", &q.display().to_string()]);
    assert!(!ok && text.contains("Pruefsumme stimmt nicht"), "{text}");
    let datei = ziel.join(format!("oberflaeche-{}.sqfs", arch()));
    assert!(!datei.exists(), "trotz falscher Summe uebernommen");
    assert!(!ziel.join(format!("oberflaeche-{}.sqfs.neu", arch())).exists(), "Reste liegen geblieben");

    let q = quelle(&o.0, b"squashfs-probe", None);
    assert!(rufe(&ziel, &["holen", &q.display().to_string()]).0);
    std::fs::write(&datei, b"spaeter veraendert").expect("veraendern");
    let (_, status) = rufe(&ziel, &["status"]);
    assert!(status.contains("Pruefsumme stimmt NICHT"), "{status}");
}

/// Ohne Pruefsumme daneben, ueber http oder ohne Quelle: abgewiesen.
#[test]
fn ohne_pruefsumme_oder_ueber_http_nicht() {
    let o = Ordner::neu("ohne");
    let ziel = o.0.join("schichten");
    let nackt = o.0.join("nackt.sqfs");
    std::fs::write(&nackt, b"x").expect("Datei");
    for args in [
        vec!["holen", nackt.to_str().expect("Pfad")],
        vec!["holen", "http://example.org/oberflaeche.sqfs"],
        vec!["holen"],
        vec!["unbekannt"],
    ] {
        let (ok, text) = rufe(&ziel, &args);
        assert!(!ok, "angenommen: {args:?}\n{text}");
    }
    assert!(!ziel.join(format!("oberflaeche-{}.sqfs", arch())).exists());
}

/// Das Konto der Schicht ist im Skript dasselbe wie im Eintrittsbefehl und
/// in der Benutzertabelle (die Tabelle haelt `schicht::KONTO` fest).
#[test]
fn das_konto_der_schicht_ist_ueberall_dasselbe() {
    let skript = std::fs::read_to_string(SKRIPT).expect("Skript");
    let tabelle = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../konfigurationen/schicht_benutzer.tabelle"))
        .expect("Tabelle");
    let id = tabelle.lines().find(|z| z.starts_with("golem ")).and_then(|z| z.split_whitespace().nth(1)).expect("golem");
    assert!(skript.contains(&format!("\nKONTO={id}\n")), "golem-schicht nennt ein anderes Konto als die Tabelle ({id})");
}
