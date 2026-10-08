//! **Signierte Module**: Schluessel, Signieren, Pruefen, Vertrauensliste
//! und Rollback-Sperre.
//!
//! Signiert wird die Beschreibung (`modul.json`) **als Bytes**, nicht neu
//! serialisiert, und sie enthaelt die SHA-256 jeder Datei des Moduls. Die
//! Signatur ist Ed25519 ueber [`DOMAENE`] und diese Bytes; die Domaene
//! verhindert, dass eine Signatur fuer etwas anderes als eine
//! Modulbeschreibung gilt.
//!
//! # ⚑ Was die Pruefung abwehrt
//!
//! - **Veraenderte Dateien**: jede Datei gegen ihre Pruefsumme.
//! - **Dazugelegte Dateien und symbolische Verweise**: Eine Datei, die
//!   nicht in der Beschreibung steht, laesst das Modul nicht laden; ein
//!   Verweis koennte aus dem Ordner hinauszeigen und wird nie gefolgt.
//! - **Fehlende Dateien und gemischte Fassungen**: Die Liste muss genau
//!   aufgehen.
//! - **Rollback**: Eine kleinere Version als die zuletzt gestartete wird
//!   abgelehnt ([`Versionsstand`]).
//! - **Ein Tausch zwischen Pruefen und Ausfuehren**: Das Programm wird
//!   einmal gelesen, an diesen Bytes geprueft, und genau diese Bytes gibt
//!   [`Geprueft::programm`] zurueck; der Wirt fuehrt sie aus einer eigenen
//!   Kopie aus.
//! - **Ein gestohlener Schluessel**: Widerruf in der Vertrauensliste.
//!
//! ⚠️ **Was sie nicht abwehrt**: ein Angreifer, der die Vertrauensliste
//! oder die Konsole selbst aendern kann. Wer das kann, hat die Rechte des
//! Nutzers schon.

use crate::beschreibung::{version_vergleichen, Beschreibung, Laufzeit};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Vorangestellt vor jeder signierten Beschreibung.
pub const DOMAENE: &[u8] = b"myelith-modul-v1\0";

/// So gross darf eine Beschreibung hoechstens sein.
pub const BESCHREIBUNG_HOECHSTENS: u64 = 1 << 20;

/// So viele Dateien und so viele Bytes darf ein Modul ausserhalb seiner
/// veraenderlichen Ordner hoechstens haben.
pub const DATEIEN_HOECHSTENS: usize = 20_000;
pub const BYTES_HOECHSTENS: u64 = 2 << 30;

/// Dateien, die das Betriebssystem von selbst anlegt und die nie gelesen
/// werden; sie zaehlen nicht als dazugelegt.
const SYSTEMDATEIEN: [&str; 1] = [".DS_Store"];

/// **Die Signaturdatei** (`modul.sig`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signaturdatei {
    /// Der oeffentliche Schluessel, Hex.
    pub schluessel: String,
    /// Die Signatur, Hex.
    pub signatur: String,
}

/// **Ein vertrauter Schluessel.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vertrauter {
    /// Wer es ist, zur Anzeige.
    pub name: String,
    /// Der oeffentliche Schluessel, Hex.
    pub oeffentlich: String,
    /// ⛔️ Ob Module dieses Schluessels **nativ** laufen duerfen, also ohne
    /// Abschottung.
    #[serde(default)]
    pub nativ: bool,
    /// Widerrufen: Kein Modul dieses Schluessels startet mehr.
    #[serde(default)]
    pub widerrufen: bool,
}

/// **Die Vertrauensliste** (`vertrauen.json` im Ordner der Einstellungen).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Vertrauensliste {
    #[serde(default)]
    pub schluessel: Vec<Vertrauter>,
}

impl Vertrauensliste {
    /// Liest sie; fehlt die Datei, ist sie leer (dann startet kein Modul).
    pub fn lesen(pfad: &Path) -> Result<Self, String> {
        match std::fs::read(pfad) {
            Ok(b) => serde_json::from_slice(&b).map_err(|f| format!("{}: {f}", pfad.display())),
            Err(f) if f.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(f) => Err(format!("{}: {f}", pfad.display())),
        }
    }

    pub fn schreiben(&self, pfad: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|f| f.to_string())?;
        schreiben_atomar(pfad, text.as_bytes(), false)
    }
}

/// **Die hoechste je gestartete Version je Modul** (`versionen.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Versionsstand {
    #[serde(default)]
    pub module: BTreeMap<String, String>,
}

impl Versionsstand {
    pub fn lesen(pfad: &Path) -> Self {
        std::fs::read(pfad).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    /// Merkt sich die Version, wenn sie hoeher ist als die gemerkte.
    pub fn merken(&mut self, name: &str, version: &str) -> bool {
        let hoeher = self.module.get(name).is_none_or(|alt| version_vergleichen(version, alt) == Some(std::cmp::Ordering::Greater));
        if hoeher {
            self.module.insert(name.to_string(), version.to_string());
        }
        hoeher
    }

    pub fn schreiben(&self, pfad: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|f| f.to_string())?;
        schreiben_atomar(pfad, text.as_bytes(), false)
    }
}

/// **Ein geprueftes Modul**: was die Konsole danach braucht.
#[derive(Debug, Clone)]
pub struct Geprueft {
    pub ordner: PathBuf,
    pub beschreibung: Beschreibung,
    /// Der Name des Schluessels aus der Vertrauensliste.
    pub signiert_von: String,
    /// Das Programm, genau die gepruefen Bytes.
    pub programm: Vec<u8>,
    /// SHA-256 des Programms, Hex (fuer den Zwischenspeicher des Wirts).
    pub programm_hash: String,
}

fn hex32(s: &str) -> Result<[u8; 32], String> {
    let v = hex::decode(s.trim()).map_err(|_| "kein Hex".to_string())?;
    v.try_into().map_err(|_| "nicht 32 Bytes".to_string())
}

/// **Erzeugt ein Schluesselpaar**: (geheim, oeffentlich), beide als Hex.
pub fn schluessel_erzeugen() -> Result<(String, String), String> {
    let mut saat = [0u8; 32];
    getrandom::getrandom(&mut saat).map_err(|f| format!("Zufall: {f}"))?;
    let k = SigningKey::from_bytes(&saat);
    saat.fill(0);
    Ok((hex::encode(k.to_bytes()), hex::encode(k.verifying_key().to_bytes())))
}

/// Der oeffentliche Schluessel zu einem geheimen (beide Hex).
pub fn oeffentlich_zu(geheim: &str) -> Result<String, String> {
    Ok(hex::encode(SigningKey::from_bytes(&hex32(geheim)?).verifying_key().to_bytes()))
}

/// **Schreibt eine Datei in einem Zug**: erst daneben, dann umbenannt;
/// `geheim` heisst nur fuer den Besitzer lesbar (Unix).
pub fn schreiben_atomar(pfad: &Path, inhalt: &[u8], geheim: bool) -> Result<(), String> {
    if let Some(o) = pfad.parent() {
        std::fs::create_dir_all(o).map_err(|f| format!("{}: {f}", o.display()))?;
    }
    let tmp = pfad.with_extension("unfertig");
    std::fs::write(&tmp, inhalt).map_err(|f| format!("{}: {f}", tmp.display()))?;
    #[cfg(unix)]
    if geheim {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).map_err(|f| f.to_string())?;
    }
    #[cfg(not(unix))]
    let _ = geheim;
    std::fs::rename(&tmp, pfad).map_err(|f| format!("{}: {f}", pfad.display()))
}

/// **Alle Dateien eines Modulordners** ausser Beschreibung, Signatur,
/// Systemdateien und den veraenderlichen Ordnern, mit ihren Bytes gelesen.
/// Ein symbolischer Verweis ist ein Fehler.
fn dateien_lesen(ordner: &Path, veraenderlich: &dyn Fn(&str) -> bool, mut je: impl FnMut(&str, &[u8]) -> Result<(), String>) -> Result<(), String> {
    let mut offen = vec![(ordner.to_path_buf(), String::new())];
    let (mut anzahl, mut bytes) = (0usize, 0u64);
    while let Some((o, rel)) = offen.pop() {
        let mut eintraege: Vec<std::fs::DirEntry> = std::fs::read_dir(&o).map_err(|f| format!("{}: {f}", o.display()))?.filter_map(Result::ok).collect();
        eintraege.sort_by_key(|e| e.file_name());
        for e in eintraege {
            let name = e.file_name().to_string_lossy().to_string();
            let p = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if rel.is_empty() && (name == crate::BESCHREIBUNG || name == crate::SIGNATUR) {
                continue;
            }
            if SYSTEMDATEIEN.contains(&name.as_str()) || veraenderlich(&p) {
                continue;
            }
            let art = std::fs::symlink_metadata(e.path()).map_err(|f| format!("{p}: {f}"))?;
            if art.file_type().is_symlink() {
                return Err(format!("{p} ist ein symbolischer Verweis; im Modul nicht erlaubt"));
            }
            if art.is_dir() {
                offen.push((e.path(), p));
                continue;
            }
            anzahl += 1;
            bytes += art.len();
            if anzahl > DATEIEN_HOECHSTENS || bytes > BYTES_HOECHSTENS {
                return Err(format!("zu viele Dateien oder Bytes (hoechstens {DATEIEN_HOECHSTENS} und {BYTES_HOECHSTENS})"));
            }
            let inhalt = std::fs::read(e.path()).map_err(|f| format!("{p}: {f}"))?;
            je(&p, &inhalt)?;
        }
    }
    Ok(())
}

fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// **Signiert einen Modulordner**: traegt die Pruefsummen aller Dateien in
/// die Beschreibung ein, schreibt sie und die Signatur. Die Beschreibung
/// muss vorher stimmen (ausser `dateien`, das hier entsteht).
pub fn signieren(ordner: &Path, geheim_hex: &str) -> Result<Beschreibung, String> {
    let roh = std::fs::read(ordner.join(crate::BESCHREIBUNG)).map_err(|f| format!("{}: {f}", crate::BESCHREIBUNG))?;
    let mut b: Beschreibung = serde_json::from_slice(&roh).map_err(|f| format!("{}: {f}", crate::BESCHREIBUNG))?;
    let mut dateien = BTreeMap::new();
    let v = b.clone();
    dateien_lesen(ordner, &|p| v.ist_veraenderlich(p), |p, inhalt| {
        dateien.insert(p.to_string(), sha256_hex(inhalt));
        Ok(())
    })?;
    b.dateien = dateien;
    if let Some(p) = b.programm_hier().filter(|p| !b.dateien.contains_key(*p)) {
        return Err(format!("Das Programm {p} fehlt im Ordner"));
    }
    let text = serde_json::to_vec_pretty(&b).map_err(|f| f.to_string())?;
    // Gelesen wird wie beim Pruefen, damit nichts signiert wird, was nicht laedt.
    let b = Beschreibung::lesen(&text)?;
    let k = SigningKey::from_bytes(&hex32(geheim_hex).map_err(|f| format!("geheimer Schluessel: {f}"))?);
    let mut nachricht = DOMAENE.to_vec();
    nachricht.extend_from_slice(&text);
    let sig = Signaturdatei { schluessel: hex::encode(k.verifying_key().to_bytes()), signatur: hex::encode(k.sign(&nachricht).to_bytes()) };
    schreiben_atomar(&ordner.join(crate::BESCHREIBUNG), &text, false)?;
    schreiben_atomar(&ordner.join(crate::SIGNATUR), serde_json::to_string_pretty(&sig).map_err(|f| f.to_string())?.as_bytes(), false)?;
    Ok(b)
}

/// **Prueft einen Modulordner** gegen die Vertrauensliste und den
/// Versionsstand. Erst wenn alles stimmt, gibt es ein [`Geprueft`].
pub fn pruefen(ordner: &Path, vertrauen: &Vertrauensliste, versionen: &Versionsstand) -> Result<Geprueft, String> {
    let lies = |name: &str| -> Result<Vec<u8>, String> {
        let p = ordner.join(name);
        let m = std::fs::symlink_metadata(&p).map_err(|f| format!("{name}: {f}"))?;
        if m.file_type().is_symlink() || m.len() > BESCHREIBUNG_HOECHSTENS {
            return Err(format!("{name}: kein gewoehnliche Datei oder zu gross"));
        }
        std::fs::read(&p).map_err(|f| format!("{name}: {f}"))
    };
    let roh = lies(crate::BESCHREIBUNG)?;
    let sig: Signaturdatei = serde_json::from_slice(&lies(crate::SIGNATUR).map_err(|f| format!("ohne Signatur startet kein Modul ({f})"))?)
        .map_err(|f| format!("{}: {f}", crate::SIGNATUR))?;
    // Zuerst der Schluessel: ein unbekannter oder widerrufener wird gar nicht erst geprueft.
    let wer = vertrauen
        .schluessel
        .iter()
        .find(|v| v.oeffentlich.trim().eq_ignore_ascii_case(sig.schluessel.trim()))
        .ok_or_else(|| "signiert mit einem Schluessel, der nicht in der Vertrauensliste steht".to_string())?;
    if wer.widerrufen {
        return Err(format!("der Schluessel von {} ist widerrufen", crate::filter::eine_zeile(&wer.name)));
    }
    let schluessel = VerifyingKey::from_bytes(&hex32(&wer.oeffentlich).map_err(|f| format!("Vertrauensliste: {f}"))?).map_err(|f| format!("Vertrauensliste: {f}"))?;
    let signatur = Signature::from_bytes(&hex::decode(sig.signatur.trim()).ok().and_then(|v| <[u8; 64]>::try_from(v).ok()).ok_or("Signatur: nicht 64 Bytes Hex")?);
    let mut nachricht = DOMAENE.to_vec();
    nachricht.extend_from_slice(&roh);
    schluessel.verify_strict(&nachricht, &signatur).map_err(|_| "die Signatur passt nicht zur Beschreibung".to_string())?;
    // Ab hier ist die Beschreibung echt; jetzt wird sie gelesen.
    let b = Beschreibung::lesen(&roh)?;
    if b.protokoll != crate::PROTOKOLL {
        return Err(format!("{} spricht Protokoll {}, die Konsole {}", b.name, b.protokoll, crate::PROTOKOLL));
    }
    if matches!(b.laufzeit, Laufzeit::Nativ { .. }) && !wer.nativ {
        return Err(format!("{} ist nativ (ohne Abschottung), und der Schluessel von {} ist dafuer nicht freigegeben", b.name, crate::filter::eine_zeile(&wer.name)));
    }
    if let Some(alt) = versionen.module.get(&b.name) {
        if version_vergleichen(&b.version, alt) == Some(std::cmp::Ordering::Less) {
            return Err(format!("{} {} ist aelter als die schon gestartete {alt}; abgelehnt", b.name, b.version));
        }
    }
    let programm_pfad = b.programm_hier().map(str::to_string).ok_or_else(|| format!("{} hat kein Programm fuer {}", b.name, crate::beschreibung::zielsystem()))?;
    let mut gesehen = BTreeSet::new();
    let mut programm = None;
    let bb = b.clone();
    dateien_lesen(ordner, &|p| bb.ist_veraenderlich(p), |p, inhalt| {
        let soll = b.dateien.get(p).ok_or_else(|| format!("{p} steht nicht in der Beschreibung (dazugelegt?)"))?;
        if sha256_hex(inhalt) != *soll {
            return Err(format!("{p} ist veraendert"));
        }
        if p == programm_pfad {
            programm = Some(inhalt.to_vec());
        }
        gesehen.insert(p.to_string());
        Ok(())
    })?;
    if let Some(p) = b.dateien.keys().find(|p| !gesehen.contains(*p)) {
        return Err(format!("{p} fehlt"));
    }
    let programm = programm.ok_or_else(|| format!("Das Programm {programm_pfad} fehlt"))?;
    let programm_hash = sha256_hex(&programm);
    Ok(Geprueft { ordner: ordner.to_path_buf(), beschreibung: b, signiert_von: wer.name.clone(), programm, programm_hash })
}

/// Der oeffentliche Schluessel einer Signatur, als Hex (ohne Pruefung).
pub fn schluessel_der_signatur(ordner: &Path) -> Option<String> {
    let s: Signaturdatei = serde_json::from_slice(&std::fs::read(ordner.join(crate::SIGNATUR)).ok()?).ok()?;
    Some(s.schluessel)
}

/// Prueft eine Signatur ohne Ordner (fuer Proben).
pub fn signatur_gilt(oeffentlich_hex: &str, bytes: &[u8], signatur_hex: &str) -> bool {
    let (Ok(k), Some(s)) = (hex32(oeffentlich_hex), hex::decode(signatur_hex).ok().and_then(|v| <[u8; 64]>::try_from(v).ok())) else { return false };
    let Ok(k) = VerifyingKey::from_bytes(&k) else { return false };
    let mut n = DOMAENE.to_vec();
    n.extend_from_slice(bytes);
    k.verify(&n, &Signature::from_bytes(&s)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Probe {
        ordner: PathBuf,
        geheim: String,
        vertrauen: Vertrauensliste,
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.ordner);
        }
    }

    fn probe(name: &str) -> Probe {
        let ordner = std::env::temp_dir().join(format!("myl-module-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ordner);
        std::fs::create_dir_all(ordner.join("bin")).unwrap();
        std::fs::create_dir_all(ordner.join("daten")).unwrap();
        let ziel = crate::beschreibung::zielsystem();
        let b = crate::beschreibung::tests::beispiel().replace("aarch64-macos", &ziel);
        std::fs::write(ordner.join(crate::BESCHREIBUNG), b).unwrap();
        std::fs::write(ordner.join("bin/beispiel"), b"#!/bin/sh\necho hallo\n").unwrap();
        std::fs::write(ordner.join("anleitung.md"), b"Kern").unwrap();
        std::fs::write(ordner.join("daten/stand.json"), b"{}").unwrap();
        let (geheim, oeffentlich) = schluessel_erzeugen().unwrap();
        signieren(&ordner, &geheim).unwrap();
        let vertrauen = Vertrauensliste { schluessel: vec![Vertrauter { name: "Probe".into(), oeffentlich, nativ: true, widerrufen: false }] };
        Probe { ordner, geheim, vertrauen }
    }

    fn pruefe(p: &Probe) -> Result<Geprueft, String> {
        pruefen(&p.ordner, &p.vertrauen, &Versionsstand::default())
    }

    #[test]
    fn signiert_und_geprueft() {
        let p = probe("gut");
        let g = pruefe(&p).unwrap();
        assert_eq!(g.programm, b"#!/bin/sh\necho hallo\n", "genau die gepruefen Bytes");
        assert_eq!(g.signiert_von, "Probe");
        assert!(g.beschreibung.dateien.contains_key("anleitung.md") && !g.beschreibung.dateien.contains_key("daten/stand.json"));
        // Veraenderliches darf sich aendern.
        std::fs::write(p.ordner.join("daten/stand.json"), b"{\"neu\":1}").unwrap();
        std::fs::write(p.ordner.join(".DS_Store"), b"x").unwrap();
        assert!(pruefe(&p).is_ok());
    }

    #[test]
    fn jede_aenderung_faellt_auf() {
        let p = probe("aenderung");
        std::fs::write(p.ordner.join("anleitung.md"), b"Kern, veraendert").unwrap();
        assert!(pruefe(&p).unwrap_err().contains("veraendert"));
    }

    #[test]
    fn dazugelegt_fehlend_verwiesen() {
        let p = probe("dazu");
        std::fs::write(p.ordner.join("bin/zusatz"), b"x").unwrap();
        assert!(pruefe(&p).unwrap_err().contains("steht nicht"), "dazugelegt");
        std::fs::remove_file(p.ordner.join("bin/zusatz")).unwrap();
        std::fs::remove_file(p.ordner.join("anleitung.md")).unwrap();
        assert!(pruefe(&p).unwrap_err().contains("fehlt"), "fehlend");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/hosts", p.ordner.join("anleitung.md")).unwrap();
            assert!(pruefe(&p).unwrap_err().contains("Verweis"), "Verweis");
        }
    }

    #[test]
    fn beschreibung_veraendert_oder_ohne_signatur() {
        let p = probe("sig");
        let b = std::fs::read_to_string(p.ordner.join(crate::BESCHREIBUNG)).unwrap();
        std::fs::write(p.ordner.join(crate::BESCHREIBUNG), b.replace("\"modell\": true", "\"modell\": true, \"agent\": true").replace("\"agent\": false", "\"agent\": true")).unwrap();
        assert!(pruefe(&p).unwrap_err().contains("passt nicht"), "Befugnis nachtraeglich erweitert");
        std::fs::write(p.ordner.join(crate::BESCHREIBUNG), &b).unwrap();
        assert!(pruefe(&p).is_ok());
        std::fs::remove_file(p.ordner.join(crate::SIGNATUR)).unwrap();
        assert!(pruefe(&p).unwrap_err().contains("ohne Signatur"));
    }

    #[test]
    fn schluessel_unbekannt_widerrufen_nicht_nativ() {
        let p = probe("schluessel");
        let fremd = Vertrauensliste { schluessel: vec![Vertrauter { name: "Fremd".into(), oeffentlich: schluessel_erzeugen().unwrap().1, nativ: true, widerrufen: false }] };
        assert!(pruefen(&p.ordner, &fremd, &Versionsstand::default()).unwrap_err().contains("nicht in der Vertrauensliste"));
        let mut v = p.vertrauen.clone();
        v.schluessel[0].widerrufen = true;
        assert!(pruefen(&p.ordner, &v, &Versionsstand::default()).unwrap_err().contains("widerrufen"));
        v.schluessel[0].widerrufen = false;
        v.schluessel[0].nativ = false;
        assert!(pruefen(&p.ordner, &v, &Versionsstand::default()).unwrap_err().contains("nativ"));
    }

    #[test]
    fn rollback_abgelehnt() {
        let p = probe("rollback");
        let mut st = Versionsstand::default();
        assert!(st.merken("beispiel", "0.2.0"));
        assert!(!st.merken("beispiel", "0.1.9"), "kleiner wird nicht gemerkt");
        assert!(pruefen(&p.ordner, &p.vertrauen, &st).unwrap_err().contains("aelter"));
        st.module.insert("beispiel".into(), "0.1.0".into());
        assert!(pruefen(&p.ordner, &p.vertrauen, &st).is_ok(), "dieselbe Version startet wieder");
    }

    #[test]
    fn signatur_gilt_nur_mit_domaene() {
        let p = probe("domaene");
        let k = SigningKey::from_bytes(&hex32(&p.geheim).unwrap());
        let roh = std::fs::read(p.ordner.join(crate::BESCHREIBUNG)).unwrap();
        let ohne_domaene = hex::encode(k.sign(&roh).to_bytes());
        assert!(!signatur_gilt(&hex::encode(k.verifying_key().to_bytes()), &roh, &ohne_domaene), "eine Signatur ohne Domaene gilt nicht");
    }
}
