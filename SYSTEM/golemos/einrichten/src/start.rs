//! Was beim Start geschieht: einen Klon suchen und ein Modell einstellen.
//!
//! Gerufen von `etc/init.d/S06myelith`, nach `S05daten`. Das Ergebnis
//! ist ein Zettel unter `/run/golemos/wurzel`, den
//! `etc/profile.d/golemos.sh` als `MYELITH_WURZEL` in jede Sitzung
//! traegt.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::geraete::{self, gb};
use crate::klon::{self, Artefakt};
use crate::platten::{NAME_EFI, NAME_WURZEL_A, NAME_WURZEL_B, TYP_EFI};

pub const ZETTEL: &str = "/run/golemos/wurzel";
pub const DATEN: &str = "/daten";
pub const MEDIEN: &str = "/medien";

/// Die Dateisysteme, in denen ein Mensch einen Klon ablegen koennte.
///
/// ⚠️ Ob der Kern `exfat` und `ntfs3` kann, haengt an seiner
/// Konfiguration. Scheitert das Einhaengen, wird das Laufwerk
/// uebergangen, und der Bericht zaehlt es mit.
const DATEISYSTEME: [&str; 6] = ["vfat", "exfat", "ext4", "ext3", "ext2", "ntfs"];

#[derive(Debug, Default)]
pub struct Bericht {
    pub klon: Option<PathBuf>,
    pub artefakte: Vec<Artefakt>,
    /// Das Modell, das in die frischen Einstellungen geschrieben wurde.
    pub eingestellt: Option<String>,
    pub eingehaengt: Vec<PathBuf>,
    pub nicht_eingehaengt: usize,
}

/// Ist `/daten` eingehaengt?
pub fn daten_da() -> bool {
    fs::read_to_string("/proc/mounts")
        .map(|t| t.lines().any(|z| z.split_whitespace().nth(1) == Some(DATEN)))
        .unwrap_or(false)
}

/// Der Klon vom letzten Suchlauf, falls einer gefunden wurde.
pub fn gemerkter_klon() -> Option<PathBuf> {
    let p = PathBuf::from(fs::read_to_string(ZETTEL).ok()?.trim());
    p.join(klon::MARKE).is_file().then_some(p)
}

pub fn suchen(aus: &mut dyn Write) -> io::Result<Bericht> {
    let mut b = Bericht::default();
    let daten = daten_da();
    let eigene: Vec<PathBuf> = if daten { vec![PathBuf::from(DATEN)] } else { Vec::new() };
    b.klon = klon::finden(&eigene);
    // ⚑ **Fremde Laufwerke nur, wenn auf den eigenen nichts liegt.** Ein
    //   Laufwerk einzuhaengen, das niemand braucht, ist ein Eingriff ohne
    //   Grund, auch wenn er nur lesend ist.
    if b.klon.is_none() {
        let (orte, fehl) = fremde_einhaengen();
        b.eingehaengt = orte;
        b.nicht_eingehaengt = fehl;
        b.klon = klon::finden(&b.eingehaengt);
    }

    fs::create_dir_all("/run/golemos")?;
    match &b.klon {
        Some(k) => fs::write(ZETTEL, format!("{}\n", k.display()))?,
        None => {
            let _ = fs::remove_file(ZETTEL);
        }
    }

    b.artefakte = klon::artefakte(b.klon.as_deref(), &[Path::new(DATEN).join("artefakte")]);
    let einstellungen = Path::new(DATEN).join("myelith/client.json");
    // ⚑ **Nicht „gibt es Einstellungen?", sondern „gibt es das eingestellte
    //   Modell?".** Die Wahl der Sprache legt `client.json` schon beim
    //   ersten Start an, bevor ein Ordner da ist, und darin steht dann die
    //   Vorgabe des Clients. Frueher hiess eine vorhandene Datei „schon
    //   eingestellt", und ein Stick mit nur dem 0,6B-Modell behielt ein
    //   Modell, das es auf ihm nicht gibt.
    let steht = fs::read_to_string(&einstellungen).ok().and_then(|t| eingestelltes_artefakt(&t));
    let gibt_es = steht.as_deref().is_some_and(|a| {
        let p = Path::new(a);
        let voll = if p.is_absolute() { p.to_path_buf() } else { b.klon.as_deref().map(|k| k.join(p)).unwrap_or_default() };
        klon::ist_artefakt(&voll)
    });
    if daten && !gibt_es {
        if let Some(a) = klon::waehlen(&b.artefakte, klon::arbeitsspeicher()) {
            let pfad = klon::einstellpfad(a, b.klon.as_deref());
            myl_setzen(b.klon.as_deref(), "modell.artefakt", &pfad)?;
            myl_setzen(b.klon.as_deref(), "agent.wurzel", "/daten/arbeit")?;
            b.eingestellt = Some(pfad);
        }
    }

    match &b.klon {
        Some(k) => writeln!(aus, "Myelith: Klon unter {}", k.display())?,
        None => writeln!(aus, "Myelith: kein Klon gefunden")?,
    }
    let namen: Vec<String> = b.artefakte.iter().map(|a| format!("{} ({})", a.name, gb(a.groesse))).collect();
    writeln!(aus, "Modelle: {}", if namen.is_empty() { "keine".into() } else { namen.join(", ") })?;
    if let Some(p) = &b.eingestellt {
        writeln!(aus, "Eingestellt: {p}")?;
    }
    Ok(b)
}

/// Der Wert von `"artefakt"` in `client.json`, ohne JSON-Zerleger.
///
/// ⚠️ Nur fuer diese eine Frage: Steht dort ein Pfad, und gibt es ihn?
/// Geschrieben wird die Datei ausschliesslich ueber `myl setzen`.
pub fn eingestelltes_artefakt(json: &str) -> Option<String> {
    let i = json.find("\"artefakt\"")?;
    let rest = &json[i + "\"artefakt\"".len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let ende = rest.find('"')?;
    Some(rest[..ende].to_string())
}

/// Schreibt eine Einstellung mit `myl setzen`.
///
/// ⚑ **Ueber `myl` und nicht selbst ins JSON.** Der Client kennt die
/// Form seiner Datei; ein zweiter Schreiber muesste sie nachbilden, und
/// eine nachgebildete Form laeuft auseinander (Fund 455 war genau eine
/// Datei, die niemand las).
fn myl_setzen(klon: Option<&Path>, feld: &str, wert: &str) -> io::Result<()> {
    let mut c = Command::new("myl");
    c.args(["setzen", feld, wert]).env("XDG_CONFIG_HOME", DATEN);
    if let Some(k) = klon {
        c.env(klon::UMGEBUNG, k);
    }
    let aus = c.stdout(Stdio::null()).output()?;
    if aus.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "myl setzen {feld} endete mit {}: {}",
        aus.status,
        String::from_utf8_lossy(&aus.stderr).trim()
    )))
}

/// Haengt fremde Partitionen lesend unter `/medien/<name>` ein.
///
/// ⛔️ **Nur lesend.** Diese Suche laeuft bei jedem Start ohne Frage; sie
/// darf auf einem fremden Laufwerk nichts veraendern, nicht einmal eine
/// Zugriffszeit.
fn fremde_einhaengen() -> (Vec<PathBuf>, usize) {
    let Ok(g) = geraete::lesen() else { return (Vec::new(), 0) };
    let mut orte = Vec::new();
    let mut fehl = 0;
    for p in g.iter().filter(|x| {
        x.ist_partition()
            && x.einhaengung.is_empty()
            && DATEISYSTEME.contains(&x.dateisystem.as_str())
            && ![NAME_EFI, NAME_WURZEL_A, NAME_WURZEL_B].contains(&x.partname.as_str())
            && x.parttyp != TYP_EFI
    }) {
        let ort = Path::new(MEDIEN).join(&p.name);
        let typ = if p.dateisystem == "ntfs" { "ntfs3" } else { p.dateisystem.as_str() };
        let ok = fs::create_dir_all(&ort).is_ok()
            && Command::new("mount")
                .args(["-t", typ, "-o", "ro", &p.pfad()])
                .arg(&ort)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
        if ok {
            orte.push(ort);
        } else {
            let _ = fs::remove_dir(&ort);
            fehl += 1;
        }
    }
    (orte, fehl)
}

#[cfg(test)]
mod proben {
    use super::*;

    #[test]
    fn liest_das_eingestellte_artefakt() {
        let j = "{\n  \"modell\": {\n    \"artefakt\": \"INTEGER_LLM/artifacts/myelith-4b\",\n    \"token\": 64\n  }\n}";
        assert_eq!(eingestelltes_artefakt(j).as_deref(), Some("INTEGER_LLM/artifacts/myelith-4b"));
        assert_eq!(eingestelltes_artefakt("{\"modell\":{\"artefakt\":\"/daten/x\"}}").as_deref(), Some("/daten/x"));
        assert_eq!(eingestelltes_artefakt("{}"), None);
        assert_eq!(eingestelltes_artefakt("{\"artefakt\": null}"), None);
    }
}
