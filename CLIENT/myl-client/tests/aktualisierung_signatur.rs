//! **Die Pruefung eines geholten Stands gegen die Freigabeschluessel**
//! (`SYSTEM/install/freigabe.sh`, Sicherheitsaudit vom 2026-11-18).
//!
//! Gegen echte Repositorien: ein Herausgeber, ein `origin`, ein Klon, und
//! Wegwerfschluessel aus `ssh-keygen`. Gefahren wird die Funktion genau so,
//! wie die Installationsskripte sie einbinden.
//!
//! ⚠️ Braucht `git` und `ssh-keygen`; fehlt eines, sagt die Probe es und
//! laesst aus. Unter Windows prueft dasselbe `installieren-windows.ps1`, hier
//! nicht gefahren.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn wurzel() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("Wurzel")
}

fn git(ort: &Path, args: &[&str]) -> Output {
    let o = Command::new("git")
        .args(args)
        .current_dir(ort)
        // Nichts aus der Einrichtung des Rechners (etwa eine Signatur je
        // Commit mit Passphrase).
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git");
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    o
}

/// Ein Commit im Werk, wahlweise signiert, dann nach `origin`.
fn veroeffentlichen(werk: &Path, liste: &str, signiert_mit: Option<&Path>, nachricht: &str) {
    let datei = werk.join("SYSTEM/install/freigabeschluessel");
    std::fs::create_dir_all(datei.parent().unwrap()).unwrap();
    std::fs::write(&datei, liste).unwrap();
    std::fs::write(werk.join("stand.txt"), nachricht).unwrap();
    git(werk, &["add", "-A"]);
    match signiert_mit {
        Some(k) => {
            let k = k.to_str().unwrap();
            git(werk, &["-c", "gpg.format=ssh", "-c", &format!("user.signingkey={k}"), "commit", "-q", "-S", "-m", nachricht]);
        }
        None => {
            git(werk, &["commit", "-q", "-m", nachricht]);
        }
    }
    git(werk, &["push", "-q", "origin", "HEAD:main"]);
}

/// Holt im Klon und faehrt `freigabe_pruefen FETCH_HEAD`; danach den Stand
/// einspielen, wenn sie zustimmt (wie das Skript).
fn holen_und_pruefen(klon: &Path) -> (bool, String) {
    git(klon, &["fetch", "-q", "origin"]);
    let skript = wurzel().join("SYSTEM/install/freigabe.sh");
    let o = Command::new("sh")
        .arg("-c")
        .arg(format!(". '{}'; freigabe_pruefen FETCH_HEAD", skript.display()))
        .current_dir(klon)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("sh");
    let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    if o.status.success() {
        git(klon, &["merge", "--ff-only", "-q", "FETCH_HEAD"]);
    }
    (o.status.success(), text)
}

fn zeile(name: &str, pub_datei: &Path) -> String {
    let k = std::fs::read_to_string(pub_datei).unwrap();
    let mut t = k.split_whitespace();
    format!("{name} namespaces=\"git\" {} {}\n", t.next().unwrap(), t.next().unwrap())
}

#[test]
fn nur_ein_stand_mit_der_signatur_eines_bisherigen_freigabeschluessels_kommt_herein() {
    for werkzeug in ["git", "ssh-keygen"] {
        if Command::new("sh").arg("-c").arg(format!("command -v {werkzeug}")).output().map(|o| !o.status.success()).unwrap_or(true) {
            eprintln!("⚠️ {werkzeug} fehlt, Probe ausgelassen");
            return;
        }
    }
    let basis = std::env::temp_dir().join(format!("myl-aktualisierung-signatur-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&basis);
    std::fs::create_dir_all(&basis).unwrap();
    for k in ["a", "b"] {
        let o = Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-C", k, "-f"]).arg(basis.join(k)).output().unwrap();
        assert!(o.status.success());
    }
    let (a, b) = (basis.join("a"), basis.join("b"));
    let (zeile_a, zeile_b) = (zeile("freigabe-a", &basis.join("a.pub")), zeile("fremd", &basis.join("b.pub")));
    let kopf = "# Freigabeschluessel\n";

    git(&basis, &["init", "-q", "--bare", "-b", "main", "origin.git"]);
    git(&basis, &["init", "-q", "-b", "main", "werk"]);
    let werk = basis.join("werk");
    git(&werk, &["config", "user.name", "Probe"]);
    git(&werk, &["config", "user.email", "probe@beispiel.invalid"]);
    git(&werk, &["remote", "add", "origin", basis.join("origin.git").to_str().unwrap()]);

    // 1. Liste ohne Schluessel: wird eingespielt, aber laut ungeprueft.
    veroeffentlichen(&werk, kopf, None, "erster Stand");
    git(&basis, &["clone", "-q", "origin.git", "klon"]);
    let klon = basis.join("klon");
    veroeffentlichen(&werk, &format!("{kopf}{zeile_a}"), None, "Schluessel A eingetragen, unsigniert");
    let (ok, text) = holen_und_pruefen(&klon);
    assert!(ok && text.contains("NICHT"), "ohne Schluessel: eingespielt mit Warnung ({text})");

    // 2. Ab jetzt steht A im bisherigen Stand. Von A signiert: herein.
    veroeffentlichen(&werk, &format!("{kopf}{zeile_a}"), Some(&a), "von A signiert");
    let (ok, text) = holen_und_pruefen(&klon);
    assert!(ok && text.contains("gueltig"), "von A signiert ({text})");

    // 3. Unsigniert: draussen.
    veroeffentlichen(&werk, &format!("{kopf}{zeile_a}"), None, "unsigniert");
    let (ok, text) = holen_und_pruefen(&klon);
    assert!(!ok && text.contains("keine gueltige Signatur"), "unsigniert ({text})");

    // 4. Von einem fremden Schluessel: draussen.
    veroeffentlichen(&werk, &format!("{kopf}{zeile_a}"), Some(&b), "von B signiert");
    let (ok, _) = holen_und_pruefen(&klon);
    assert!(!ok, "von einem fremden Schluessel");

    // 5. Der neue Stand traegt B selbst in die Liste ein und ist von B
    //    signiert: draussen, denn es gilt die Liste des bisherigen Stands.
    veroeffentlichen(&werk, &format!("{kopf}{zeile_a}{zeile_b}"), Some(&b), "B traegt sich selbst ein");
    let (ok, _) = holen_und_pruefen(&klon);
    assert!(!ok, "ein Stand beglaubigt sich nicht mit dem eigenen Schluessel");
    // Gegenprobe: Gegen die Liste des neuen Stands bestuende er. Der Fall
    // prueft also wirklich, welche Liste gilt.
    let neue_liste = basis.join("neue-liste");
    std::fs::write(&neue_liste, format!("{zeile_a}{zeile_b}")).unwrap();
    let o = Command::new("git")
        .args(["-c", &format!("gpg.ssh.allowedSignersFile={}", neue_liste.display()), "verify-commit", "FETCH_HEAD"])
        .current_dir(&klon)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(o.status.success(), "Gegenprobe: mit der neuen Liste gilt die Signatur von B");

    // 6. Ein Wechsel der Schluessel, von A signiert: herein; danach gilt B.
    veroeffentlichen(&werk, &format!("{kopf}{zeile_b}"), Some(&a), "A uebergibt an B");
    let (ok, _) = holen_und_pruefen(&klon);
    assert!(ok, "Wechsel mit der Signatur eines bisherigen Schluessels");
    veroeffentlichen(&werk, &format!("{kopf}{zeile_b}"), Some(&a), "A ist nicht mehr in der Liste");
    let (ok, _) = holen_und_pruefen(&klon);
    assert!(!ok, "nach dem Wechsel gilt A nicht mehr");

    let _ = std::fs::remove_dir_all(&basis);
}
