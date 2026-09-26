//! `golem-einrichten`: der Einrichtungsassistent von GolemOS.
//!
//!   golem-einrichten                     der Assistent
//!   golem-einrichten suchen              Klon suchen, Modell einstellen (beim Start)
//!   golem-einrichten platten             welche Platte in Frage kommt, und warum
//!   golem-einrichten installieren --ziel sdb --bestaetigung sdb [--ohne-daten]
//!
//! ⚑ **Der letzte Weg ist derselbe wie im Assistenten**, nur ohne
//! Fragen: derselbe Plan, dieselben Pruefungen, und die Bestaetigung
//! ist wieder der Name der Platte. Er ist fuer Proben und fuer
//! Menschen, die wissen, was sie tun; er ist nicht nachsichtiger.

mod assistent;
mod ausfuehren;
mod geraete;
mod grub;
mod klon;
mod plan;
mod platten;
mod sprache;
mod start;

use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, ExitCode};

use assistent::{Lage, Welt};
use geraete::{gb, Geraet};

/// Die Marke, nach der der Assistent nicht mehr von selbst startet.
const GESEHEN: &str = "/daten/.assistent-gesehen";

const HILFE: &str = "golem-einrichten: GolemOS einrichten

  golem-einrichten                     der Assistent
  golem-einrichten suchen              Myelith-Klon suchen, Modell einstellen
  golem-einrichten platten             welche Platte fuer eine Installation in Frage kommt
  golem-einrichten installieren --ziel NAME --bestaetigung NAME [--ohne-daten]

Eine Installation belegt die ganze Zielplatte und loescht alles darauf.
Bestaetigt wird mit dem Namen der Platte, zweimal derselbe.";

struct EchteWelt;

impl Welt for EchteWelt {
    fn geraete(&mut self) -> io::Result<Vec<Geraet>> {
        geraete::lesen()
    }

    fn lage(&mut self) -> Lage {
        let g = geraete::lesen().unwrap_or_default();
        let start = platten::startplatte(&g).and_then(|s| g.iter().find(|p| p.name == s)).map(|p| {
            let modell = if p.modell.is_empty() { String::new() } else { format!(", {}", p.modell) };
            format!("/dev/{} ({}{modell})", p.name, gb(p.groesse))
        });
        let daten = start::daten_da()
            .then(|| g.iter().find(|p| p.einhaengung == start::DATEN))
            .flatten()
            .map(|p| (p.groesse, klon::belegung(Path::new(start::DATEN))));
        let klon = start::gemerkter_klon();
        let artefakte = klon::artefakte(klon.as_deref(), &[Path::new(start::DATEN).join("artefakte")]);
        let klon_groesse = klon.as_deref().map(klon::belegung).unwrap_or(0);
        let klon_fehlt = klon.as_deref().map(klon::fehlt).unwrap_or_default();
        let ordner = Path::new(sprache::ORDNER);
        let sprache = sprache::lesen(ordner, "sprache").as_deref().and_then(sprache::Sprache::aus);
        let tastatur = sprache::lesen(ordner, "tastatur");
        Lage { start, daten, klon, klon_groesse, klon_fehlt, artefakte, sprache, tastatur }
    }

    fn suchen(&mut self, aus: &mut dyn Write) -> io::Result<()> {
        start::suchen(aus).map(|_| ())
    }

    fn installieren(
        &mut self,
        plan: &plan::Plan,
        mitnehmen: bool,
        klon: Option<&Path>,
        aus: &mut dyn Write,
    ) -> io::Result<()> {
        let von = mitnehmen.then_some(Path::new(start::DATEN));
        ausfuehren::installieren(plan, von, klon, aus)
    }

    fn spread(&mut self, klon: &Path, geraet: Option<&str>) -> io::Result<()> {
        // ⚑ Ein Stick wie dieser: dieselbe Architektur wie das laufende System.
        let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
        let mut c = Command::new("sh");
        c.arg(klon.join("SYSTEM/golemos/spread.sh")).args(["--arch", arch]);
        if let Some(g) = geraet {
            c.arg(g);
        }
        c.status().map(|_| ())
    }

    fn ausschalten(&mut self) {
        let _ = Command::new("poweroff").status();
    }

    fn myelith(&mut self) -> io::Result<()> {
        // Erbt Terminal und Umgebung, also auch `MYELITH_WURZEL` und
        // `XDG_CONFIG_HOME=/daten` aus `profile.d/golemos.sh`.
        Command::new("myelith").status().map(|_| ())
    }

    fn gesehen(&mut self) {
        let _ = std::fs::write(GESEHEN, "");
    }

    fn sprache_setzen(&mut self, s: sprache::Sprache) -> io::Result<()> {
        sprache::schreiben(Path::new(sprache::ORDNER), "sprache", s.kennung())?;
        // ⚑ Dieselbe Sprache fuer Myelith, ueber `myl setzen`, damit
        //   Assistent und Konsole nicht zwei Sprachen sprechen.
        let status = Command::new("myl")
            .args(["setzen", "oberflaeche.sprache", s.kennung()])
            .env("XDG_CONFIG_HOME", start::DATEN)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if !status.success() {
            return Err(io::Error::other(format!("myl setzen oberflaeche.sprache endete mit {status}")));
        }
        Ok(())
    }

    fn tastatur_setzen(&mut self, karte: &str) -> io::Result<()> {
        sprache::schreiben(Path::new(sprache::ORDNER), "tastatur", karte)?;
        // `-C /dev/tty0`: die Belegung der Tastatur am Rechner, auch wenn
        // der Assistent gerade ueber eine serielle Leitung laeuft.
        let status = Command::new("loadkeys").args(["-q", "-C", "/dev/tty0", karte]).status()?;
        if !status.success() {
            return Err(io::Error::other(format!("loadkeys {karte} endete mit {status}")));
        }
        Ok(())
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut aus = io::stdout();
    let ergebnis = match args.first().map(String::as_str) {
        None | Some("assistent") => {
            let ein = io::stdin();
            assistent::lauf(&mut EchteWelt, &mut ein.lock(), &mut aus)
        }
        Some("suchen") => start::suchen(&mut aus).map(|_| ()),
        Some("platten") => platten_zeigen(&mut aus),
        Some("installieren") => installieren(&args[1..], &mut aus),
        Some("-h" | "--hilfe" | "hilfe") => writeln!(aus, "{HILFE}"),
        Some(x) => {
            eprintln!("golem-einrichten: unbekannt: {x}\n\n{HILFE}");
            return ExitCode::from(2);
        }
    };
    match ergebnis {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => {
            eprintln!("golem-einrichten: {f}");
            ExitCode::FAILURE
        }
    }
}

fn platten_zeigen(aus: &mut dyn Write) -> io::Result<()> {
    let g = geraete::lesen()?;
    let start = platten::startplatte(&g).unwrap_or_default();
    let mindest = platten::quelle(&g, &start).map(|q| q.mindestgroesse()).unwrap_or(0);
    writeln!(aus, "Startplatte: {}", if start.is_empty() { "nicht zu erkennen" } else { &start })?;
    for (p, u) in platten::beurteilen(&g, &start, mindest) {
        writeln!(aus, "  /dev/{:<10} {:>10}  {:<24} {}", p.name, gb(p.groesse), p.modell, u.grund())?;
    }
    Ok(())
}

/// Der Weg ohne Fragen.
fn installieren(args: &[String], aus: &mut dyn Write) -> io::Result<()> {
    let wert = |schalter: &str| {
        args.iter().position(|a| a == schalter).and_then(|i| args.get(i + 1)).cloned()
    };
    let (Some(ziel), Some(bestaetigung)) = (wert("--ziel"), wert("--bestaetigung")) else {
        return Err(io::Error::other("es braucht --ziel NAME und --bestaetigung NAME"));
    };
    let ziel = ziel.trim_start_matches("/dev/").to_string();
    if bestaetigung.trim_start_matches("/dev/") != ziel {
        return Err(io::Error::other("--bestaetigung nennt eine andere Platte als --ziel; nichts wurde veraendert"));
    }
    let g = geraete::lesen()?;
    let start = platten::startplatte(&g).ok_or_else(|| io::Error::other("die Startplatte ist nicht zu erkennen"))?;
    let q = platten::quelle(&g, &start).map_err(io::Error::other)?;
    let platte = g
        .iter()
        .find(|p| p.name == ziel)
        .ok_or_else(|| io::Error::other(format!("/dev/{ziel} gibt es nicht")))?;
    let plan = plan::entwerfen(&q, platte, &g).map_err(io::Error::other)?;
    writeln!(aus, "{}", plan.beschreiben())?;
    let mitnehmen = !args.iter().any(|a| a == "--ohne-daten") && start::daten_da();
    let von = mitnehmen.then_some(Path::new(start::DATEN));
    let klon = start::gemerkter_klon();
    let mut bedarf = von.map(klon::belegung).unwrap_or(0);
    if let Some(k) = klon.as_deref() {
        if !von.is_some_and(|v| k.starts_with(v)) {
            bedarf += klon::belegung(k);
        }
    }
    if bedarf > plan.daten_platz {
        return Err(io::Error::other(format!(
            "die Daten ({}) passen nicht auf die Platte ({}); mit --ohne-daten kommt nur der Myelith-Ordner mit",
            gb(bedarf),
            gb(plan.daten_platz)
        )));
    }
    ausfuehren::installieren(&plan, von, klon.as_deref(), aus)
}
