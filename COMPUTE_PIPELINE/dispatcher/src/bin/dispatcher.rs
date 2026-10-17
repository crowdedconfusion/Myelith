//! `dispatcher`: Angebot, Plan und Eichung von der Kommandozeile.
//!
//! ```text
//! dispatcher erheben [--datenort <ordner>] [--json]
//! dispatcher plan    [--agent <artefaktordner>] [--vision <datei>]... [--sprache <datei>]...
//!                    [--kontext-mindestens <n>] [--kontext <n>] [--kerne <n>] [--ram-gib <n>]
//!                    [--karte-aus <kennung>]... [--ruecklage-mib <n>] [--eichbuch <datei>]
//!                    [--profil <datei>] [--angebot-mib <n>] [--datenort <ordner>] [--json]
//! dispatcher eichen  --last <name> --eichbuch <datei> [--kontext <n>] -- <befehl> [<argument>]...
//! ```
//!
//! ⚑ **`plan` ist ein Trockenlauf**: Er zeigt, was der Plan einer Last gaebe,
//! und aendert nichts. Mit `--angebot-mib` rechnet er mit diesem
//! Arbeitsspeicher statt mit dem erhobenen, als Trockenlauf fuer eine andere
//! Maschine (Kerne und Karten bleiben die erhobenen). Rueckgabe 0, wenn ein Plan entstand (auch wenn er eine
//! Last ablehnt), 2 bei falschem Aufruf oder wenn die Maschine sich nicht
//! erheben laesst.

use myl_dispatcher::local::bedarf::{fremd_aus_dateien, Modellbedarf};
use myl_dispatcher::local::eichung::{messen, Eichbuch};
use myl_dispatcher::local::erhebung::{Hardware, GIB, MIB};
use myl_dispatcher::local::profil::Expertenprofil;
use myl_dispatcher::local::plan::{
    belegen, bericht, gib, Angebot, Freigabe, Gewichte, Last, Regeln, Vorrang, VORGABE_KONTEXT_RASTER,
};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Der Mindestkontext ohne Angabe: kleiner arbeitet ein Agent mit
/// Werkzeugansage und Verlauf nicht sinnvoll.
///
/// ⚠️ **Dieselbe Zahl wie die Untergrenze des Arbeitskontexts im Client**
/// (`KONTEXT_MINDESTENS`), und eine Probe dort haelt beide gleich. Der
/// Client uebergibt seine Einstellung selbst; diese Zahl gilt nur fuer den
/// Trockenlauf von Hand.
pub const MINDESTKONTEXT: u64 = myl_dispatcher::local::plan::KONTEXT_MINDESTENS;

/// Der Arbeitskontext ohne Angabe, wie die Vorgabe im Client.
pub const ARBEITSKONTEXT: u64 = myl_dispatcher::local::plan::KONTEXT_VORGABE;

fn main() -> ExitCode {
    let argumente: Vec<String> = std::env::args().skip(1).collect();
    let ergebnis = match argumente.first().map(String::as_str) {
        Some("erheben") => erheben(&argumente[1..]),
        Some("plan") => plan(&argumente[1..]),
        Some("eichen") => eichen(&argumente[1..]),
        _ => Err(HILFE.to_string()),
    };
    match ergebnis {
        Ok(()) => ExitCode::SUCCESS,
        Err(meldung) => {
            eprintln!("{meldung}");
            ExitCode::from(2)
        }
    }
}

const HILFE: &str = "Aufruf: dispatcher erheben | plan | eichen (Einzelheiten im Kopf von src/bin/dispatcher.rs)";

/// Die Argumente eines Unterbefehls: Schalter mit Wert, Schalter ohne,
/// und was hinter `--` steht.
struct Schalter {
    werte: Vec<(String, String)>,
    marken: Vec<String>,
    rest: Vec<String>,
}

/// Schalter ohne Wert.
const MARKEN: [&str; 1] = ["--json"];

impl Schalter {
    fn lesen(a: &[String]) -> Result<Self, String> {
        let mut s = Schalter { werte: Vec::new(), marken: Vec::new(), rest: Vec::new() };
        let mut i = 0;
        while i < a.len() {
            let x = &a[i];
            if x == "--" {
                s.rest = a[i + 1..].to_vec();
                break;
            }
            if !x.starts_with("--") {
                return Err(format!("unerwartet: {x}"));
            }
            if MARKEN.contains(&x.as_str()) {
                s.marken.push(x.clone());
                i += 1;
                continue;
            }
            let wert = a.get(i + 1).ok_or_else(|| format!("{x} braucht einen Wert"))?;
            s.werte.push((x.clone(), wert.clone()));
            i += 2;
        }
        Ok(s)
    }

    fn eins(&self, name: &str) -> Option<&str> {
        self.werte.iter().rev().find(|(n, _)| n == name).map(|(_, w)| w.as_str())
    }

    fn alle(&self, name: &str) -> Vec<&str> {
        self.werte.iter().filter(|(n, _)| n == name).map(|(_, w)| w.as_str()).collect()
    }

    fn zahl(&self, name: &str) -> Result<Option<u64>, String> {
        self.eins(name).map(|w| w.parse().map_err(|_| format!("{name}: keine ganze Zahl: {w}"))).transpose()
    }

    fn marke(&self, name: &str) -> bool {
        self.marken.iter().any(|m| m == name)
    }

    /// ⚑ **Ein unbekannter Schalter ist ein Fehler**, kein stiller
    /// Vorgabewert: Ein Tippfehler wie `--kontekst` liefe sonst mit der
    /// Vorgabe durch, und der Plan saehe richtig aus.
    fn nur(&self, erlaubt: &[&str]) -> Result<(), String> {
        for (n, _) in &self.werte {
            if !erlaubt.contains(&n.as_str()) {
                return Err(format!("unbekannter Schalter {n}"));
            }
        }
        Ok(())
    }
}

fn datenort(s: &Schalter) -> PathBuf {
    s.eins("--datenort").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn erheben(a: &[String]) -> Result<(), String> {
    let s = Schalter::lesen(a)?;
    s.nur(&["--datenort"])?;
    let h = Hardware::erheben(&datenort(&s));
    if s.marke("--json") {
        println!("{}", serde_json::to_string_pretty(&h).map_err(|e| e.to_string())?);
        return Ok(());
    }
    let oder = |b: Option<u64>| b.map_or("nicht erkannt".to_string(), gib);
    println!("Kerne            {}", h.kerne);
    println!("Arbeitsspeicher  {} (frei {})", oder(h.speicher_bytes), oder(h.speicher_frei_bytes));
    if let Some(p) = &h.platte {
        println!("Platte           {} frei von {} unter {}", gib(p.frei_bytes), gib(p.gesamt_bytes), p.pfad);
    }
    for r in &h.rechenwerke {
        let speicher = if r.gemeinsamer_speicher {
            "gemeinsamer Speicher mit der CPU".to_string()
        } else {
            format!("{}, frei {}", oder(r.speicher_bytes), oder(r.frei_bytes))
        };
        println!(
            "Rechenwerk       {} [{}]: {speicher}{}",
            r.beschriftung(),
            r.kennung,
            r.pci.as_deref().map(|p| format!(", PCI {p}")).unwrap_or_default()
        );
    }
    Ok(())
}

fn plan(a: &[String]) -> Result<(), String> {
    let s = Schalter::lesen(a)?;
    s.nur(&[
        "--agent",
        "--vision",
        "--sprache",
        "--kontext-mindestens",
        "--kontext",
        "--kerne",
        "--ram-gib",
        "--karte-aus",
        "--ruecklage-mib",
        "--eichbuch",
        "--profil",
        "--angebot-mib",
        "--datenort",
    ])?;
    let buch = match s.eins("--eichbuch") {
        Some(p) => Eichbuch::lesen(Path::new(p))?,
        None => Eichbuch::default(),
    };
    let kontext_mindestens = s.zahl("--kontext-mindestens")?.unwrap_or(MINDESTKONTEXT);
    let mut lasten = Vec::new();
    if let Some(ordner) = s.eins("--agent") {
        let b = Modellbedarf::aus_artefakt(Path::new(ordner))?;
        let wunsch = s.zahl("--kontext")?.unwrap_or(ARBEITSKONTEXT);
        let zuschlag = buch.zuschlag_eigen(&b);
        // Ein Profil eines anderen Modells ist ein Fehler und kein stiller Rueckfall.
        let profil = match s.eins("--profil") {
            Some(ort) => {
                let p = Expertenprofil::lesen(Path::new(ort))?.ok_or_else(|| format!("--profil: {ort} fehlt"))?;
                if p.modell != b.name {
                    return Err(format!("--profil gilt für {}, nicht für {}", p.modell, b.name));
                }
                Some(p)
            }
            None => None,
        };
        lasten.push(Last {
            name: b.name.clone(),
            vorrang: Vorrang::Agent,
            gewichte: Gewichte::Eigen(b),
            zuschlag_bytes: zuschlag,
            kontext_mindestens,
            kontext_soll: wunsch,
            kerne_mindestens: 1,
            profil,
        });
    }
    for (schalter, vorrang) in [("--vision", Vorrang::Vision), ("--sprache", Vorrang::Sprache)] {
        let dateien = s.alle(schalter);
        if dateien.is_empty() {
            continue;
        }
        let pfade: Vec<&Path> = dateien.iter().map(Path::new).collect();
        let bytes = fremd_aus_dateien(&pfade)?;
        let name = vorrang.name().to_ascii_lowercase();
        let zuschlag = buch.zuschlag_fremd(&name, bytes);
        lasten.push(Last {
            name,
            vorrang,
            gewichte: Gewichte::Fremd { bytes },
            zuschlag_bytes: zuschlag,
            kontext_mindestens: 0,
            kontext_soll: 0,
            kerne_mindestens: 1,
            profil: None,
        });
    }

    let h = Hardware::erheben(&datenort(&s));
    let freigabe = Freigabe {
        kerne: s.zahl("--kerne")?.map(|k| k as usize),
        ram_bytes: s.zahl("--ram-gib")?.map(|g| g.saturating_mul(GIB)),
        karten_aus: s.alle("--karte-aus").into_iter().map(String::from).collect(),
    };
    let regeln = Regeln {
        ruecklage_bytes: s.zahl("--ruecklage-mib")?.map_or(Regeln::default().ruecklage_bytes, |m| m.saturating_mul(MIB)),
        kontext_raster: VORGABE_KONTEXT_RASTER,
    };
    // Eine andere Maschine: ihr freier Arbeitsspeicher statt des erhobenen,
    // auch wo diese Maschine ihren eigenen nicht erkennt.
    let fremd_frei = s.zahl("--angebot-mib")?.map(|m| m.saturating_mul(MIB));
    let h = Hardware { speicher_frei_bytes: fremd_frei.or(h.speicher_frei_bytes), ..h };
    let angebot = Angebot::aus(&h, &freigabe, &regeln)?;
    let p = belegen(&angebot, &lasten, &regeln);

    if s.marke("--json") {
        let alles = serde_json::json!({ "angebot": angebot, "regeln": regeln, "lasten": lasten, "plan": p });
        println!("{}", serde_json::to_string_pretty(&alles).map_err(|e| e.to_string())?);
        return Ok(());
    }
    println!("{}", bericht(&angebot, &lasten, &regeln, &p));
    Ok(())
}

fn eichen(a: &[String]) -> Result<(), String> {
    let s = Schalter::lesen(a)?;
    s.nur(&["--last", "--eichbuch", "--kontext"])?;
    let last = s.eins("--last").ok_or("--last fehlt")?;
    let pfad = PathBuf::from(s.eins("--eichbuch").ok_or("--eichbuch fehlt")?);
    // ⚑ **Erst lesen, dann messen**: Ein kaputtes Buch soll den Lauf nicht
    // erst nach einer Stunde Messung verwerfen.
    let mut buch = Eichbuch::lesen(&pfad)?;
    let e = messen(last, s.zahl("--kontext")?.unwrap_or(0), &s.rest)?;
    println!("{last}: Spitze {} in {} ms", gib(e.spitze_bytes), e.dauer_ms);
    buch.eintragen(e);
    buch.schreiben(&pfad)
}
