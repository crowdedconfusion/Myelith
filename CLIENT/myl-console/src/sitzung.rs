//! Der Ablauf: Vorspann, Modell, dann Auftrag um Auftrag.
//!
//! # ⚑ Ein Zustand, vier Befehle, sonst nichts
//!
//! Was ein Mensch hier tun kann, ist absichtlich klein: einen Auftrag
//! geben, das Modell wechseln, die Einstellungen oeffnen, gehen. **Ein
//! Konsolenprogramm mit zwanzig Befehlen ist ein Programm, dessen
//! Hilfeseite man liest, statt es zu benutzen.**

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::{animation, auswahl, banner, farben};

/// Der Rueckgabewert des Programms.
const GUT: i32 = 0;
const SCHLECHT: i32 = 1;

/// Alles, was ein Lauf braucht und was zwischen zwei Auftraegen stehen
/// bleibt.
struct Stand {
    /// Das Verzeichnis, in dem gearbeitet wird.
    ///
    /// ⚑ **Es ist das Arbeitsverzeichnis und kein Feld.** Wer `myelith`
    /// hier tippt, hat die Frage beantwortet.
    ordner: PathBuf,
    /// Das geladene Modell, sobald es geladen ist.
    modell: Option<myl_client::Oertlichesmodell>,
    /// Der Pfad, aus dem es geladen wurde.
    artefakt: String,
}

pub fn fahren() -> i32 {
    let ordner = match std::env::current_dir() {
        Ok(o) => o,
        Err(f) => {
            eprintln!("myelith: das Arbeitsverzeichnis ist nicht lesbar: {f}");
            return SCHLECHT;
        }
    };

    // ⚑ **Der Vorspann laeuft nur vor einem Menschen.** In einer Roehre
    // oder einem Skript ist eine Animation Zeichensalat in einer Datei,
    // die jemand spaeter liest.
    let farbe = farben::logo();
    if std::io::stdout().is_terminal() {
        animation::abspielen(farbe);
    }
    banner::bildschirm_mit(farbe);
    beschreibung(&ordner);

    let mut stand = Stand { ordner, modell: None, artefakt: String::new() };

    // ⚑ **Erst das Modell, dann die Eingabe.** Ein Eingabefeld, das bei
    // der ersten Zeile „kein Modell" sagt, hat die Frage nur
    // aufgeschoben.
    if !modell_waehlen(&mut stand) {
        return SCHLECHT;
    }

    schleife(&mut stand)
}

/// Der kurze Absatz unter der Marke.
///
/// ⚠️ **Er nennt den Ordner, und das ist keine Hoeflichkeit.** Ein
/// Agent mit Dateiwerkzeugen arbeitet gleich hier; wer das erst nach
/// dem ersten Auftrag erfaehrt, erfaehrt es zu spaet.
fn beschreibung(ordner: &Path) {
    let breite = banner::fensterbreite();
    println!();
    println!("{}", banner::zentriert("Der Agent, in dem Verzeichnis, in dem du stehst."));
    println!();
    let einzug = banner::blockeinzug(breite.min(78) as usize);
    println!("{einzug}Arbeitsordner:  {}", ordner.display());
    println!(
        "{einzug}Befehle:        /model  /settings  /hilfe  /ende"
    );
    println!();
}

/// Waehlt das Modell und laedt es.
///
/// ⚑ **Was eingestellt ist, wird vorgeschlagen und nicht gesetzt.**
/// Wer schon ein Artefakt hat, druckt einmal Eingabe; wer keines hat,
/// bekommt die Liste.
fn modell_waehlen(stand: &mut Stand) -> bool {
    let e = match myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
        Ok(e) => e,
        Err(f) => {
            eprintln!("myelith: die Einstellungen sind nicht lesbar: {f}");
            return false;
        }
    };

    let mut pfad = myl_client::ort::absolut(&e.modell.artefakt);
    if pfad.is_empty() || !Path::new(&pfad).is_dir() {
        match aus_dem_katalog(&pfad) {
            Some(neu) => pfad = neu,
            None => return false,
        }
    }

    // ⚠️ **Laden dauert und sagt es.** Ein 4B-Artefakt sind
    // viereinhalb Gigabyte; ohne diese Zeile sieht ein Start wie ein
    // Haenger aus.
    print!("Modell {} wird geladen … ", kurz(&pfad));
    let _ = std::io::stdout().flush();
    let anfang = std::time::Instant::now();
    match myl_client::Oertlichesmodell::laden(&pfad, &e.kapazitaet) {
        Ok(m) => {
            println!("{:.1} s", anfang.elapsed().as_secs_f64());
            println!();
            stand.artefakt = pfad;
            stand.modell = Some(m);
            true
        }
        Err(f) => {
            println!();
            eprintln!("myelith: das Modell laesst sich nicht laden: {f}");
            false
        }
    }
}

/// Die Auswahl aus dem Katalog, wenn nichts eingestellt ist.
///
/// ⚠️ **Gebaut wird hier nichts.** Ein Artefakt zu holen und zu
/// kalibrieren dauert Minuten bis Stunden und braucht Gigabyte; das
/// gehoert nicht hinter eine Zeile, die jemand tippt, weil er eine
/// Frage stellen wollte. Wer keines hat, wird zum Fenster geschickt,
/// das einen Balken hat und einen Abbruch.
fn aus_dem_katalog(bisher: &str) -> Option<String> {
    let Some(wurzel) = myl_client::ort::wurzel() else {
        eprintln!("myelith: kein Artefakt eingestellt, und kein Klon gefunden.");
        eprintln!("  `myl setzen modell.artefakt <pfad>` setzt eines.");
        return None;
    };

    let ordner = wurzel.join("INTEGER_LLM/artifacts");
    let mut da: Vec<PathBuf> = std::fs::read_dir(&ordner)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("config.json").is_file() || p.join("theta_v.json").is_file())
        .collect();
    da.sort();

    if da.is_empty() {
        if !bisher.is_empty() {
            eprintln!("myelith: `{bisher}` ist kein Artefaktverzeichnis.");
        }
        eprintln!("myelith: unter {} liegt kein gebautes Artefakt.", ordner.display());
        eprintln!("  Im Fenster laesst sich eines holen und bauen: `myl-oberflaeche`.");
        return None;
    }

    // ⚑ **Die Auswahl traegt Ziffern**, damit sie auch dort geht, wo es
    // keinen Rohmodus gibt: in einer Roehre, in einer seriellen
    // Konsole, unter einem Fernzugang ohne Tastaturereignisse.
    let punkte: Vec<auswahl::Punkt> = da
        .iter()
        .enumerate()
        .map(|(i, p)| {
            auswahl::Punkt::neu(
                char::from_digit(i as u32 + 1, 10).unwrap_or('?'),
                &p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                &kurz(&p.display().to_string()),
            )
        })
        .collect();

    let gewaehlt = auswahl::waehlen("Welches Modell?", &punkte)?;
    let i = gewaehlt.to_digit(10)? as usize;
    da.get(i.checked_sub(1)?).map(|p| p.display().to_string())
}

/// Die Eingabeschleife.
fn schleife(stand: &mut Stand) -> i32 {
    loop {
        let Some(zeile) = auswahl::zeile_lesen("› ") else {
            // Eingabeende, also Strg-D: das ist ein Abschied und kein
            // Fehler.
            println!();
            return GUT;
        };
        let text = zeile.trim().to_string();
        if text.is_empty() {
            continue;
        }

        match text.as_str() {
            "/ende" | "/exit" | "/quit" => return GUT,
            "/hilfe" | "/help" => {
                hilfe();
                continue;
            }
            "/model" | "/modell" => {
                stand.modell = None;
                if !modell_waehlen(stand) {
                    eprintln!("Das Modell bleibt, wie es war.");
                }
                continue;
            }
            "/settings" | "/einstellungen" => {
                einstellungen_zeigen(stand);
                continue;
            }
            _ => {}
        }

        if text.starts_with('/') {
            println!("Unbekannter Befehl. `/hilfe` zeigt, was es gibt.");
            continue;
        }

        auftrag_fahren(stand, &text);
    }
}

fn hilfe() {
    println!();
    println!("  /model      ein anderes Modell waehlen und laden");
    println!("  /settings   die Einstellungen zeigen");
    println!("  /hilfe      diese Zeilen");
    println!("  /ende       Schluss (oder Strg-D)");
    println!();
    println!("  Alles andere ist ein Auftrag an den Agenten.");
    println!();
}

/// Zeigt die Einstellungen und sagt, wo sie geaendert werden.
///
/// ⛑ **Es aendert hier nichts.** Ein zweiter Setzer neben `myl setzen`
/// und der Einstellungsseite waere die dritte Stelle, an der dieselben
/// Feinheiten stehen: was `aus` heisst, welche Felder es gibt, welcher
/// Wert eine Grenze loescht. **Drei Stellen, die das wissen, laufen
/// auseinander**, und die dritte ist immer die schlechter geprueefte.
fn einstellungen_zeigen(stand: &Stand) {
    let pfad = myl_client::Einstellungen::vorgabepfad();
    let e = match myl_client::Einstellungen::lesen(&pfad) {
        Ok(e) => e,
        Err(f) => {
            eprintln!("Die Einstellungen sind nicht lesbar: {f}");
            return;
        }
    };
    println!();
    println!("  Datei: {}", pfad.display());
    for feld in myl_client::einstellungen::FELDER {
        let f = feld.in_sprache(e.oberflaeche.sprache);
        let wert = match e.wert(f.name) {
            Ok(myl_client::einstellungen::Feldwert::Leer) => "ohne Angabe".to_string(),
            Ok(myl_client::einstellungen::Feldwert::Zahl(z)) => z.to_string(),
            Ok(myl_client::einstellungen::Feldwert::Schalter(s)) => {
                if s { "an".into() } else { "aus".into() }
            }
            Ok(myl_client::einstellungen::Feldwert::Text(t)) => t,
            Err(m) => m,
        };
        println!("  {:<22} {:<28} {}", f.name, wert, f.titel);
    }
    println!();
    // ⚑ Der Arbeitsordner dieser Sitzung steht dazu, denn er kommt
    // nicht aus der Datei und wuerde sonst als `agent.wurzel` gelesen.
    println!("  In dieser Sitzung arbeitet der Agent in:");
    println!("    {}", stand.ordner.display());
    println!();
    println!("  Geaendert wird mit `myl setzen <feld> <wert>` oder im Fenster.");
    println!();
}

/// Ein Auftrag, von der Eingabe bis zur Antwort.
fn auftrag_fahren(stand: &mut Stand, auftrag: &str) {
    let Some(modell) = stand.modell.as_ref() else {
        eprintln!("Es ist kein Modell geladen. `/model` waehlt eines.");
        return;
    };
    let e = match myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
        Ok(e) => e,
        Err(f) => {
            eprintln!("Die Einstellungen sind nicht lesbar: {f}");
            return;
        }
    };

    // ⚑ **Das Arbeitsverzeichnis dieser Sitzung schlaegt die
    // Einstellung.** `agent.wurzel` ist die Antwort des Fensters auf
    // dieselbe Frage; hier steht die Antwort schon in der Shell.
    let mut agent = e.agent.clone();
    agent.wurzel = Some(stand.ordner.display().to_string());

    let ruestung = match myl_client::ruestung::ruesten(
        &agent,
        myl_client::Ansageform::Amtlich,
        myl_client::werkzeuge::Werkzeugsatz::default(),
        Vec::new(),
    ) {
        Ok(r) => r,
        Err(f) => {
            eprintln!("Die Werkzeuge haengen nicht: {f}");
            return;
        }
    };

    // ⚑ **Was geschieht, steht da, waehrend es geschieht.** Ein
    // Werkzeugaufruf laeuft merklich lange; ohne Meldung sieht ein
    // laufender Agent aus wie ein haengendes Programm.
    // ⛑ **Die laufende Zeile gibt es nur vor einem Terminal.**
    //
    // Der erste Entwurf schrieb sie immer und loeschte sie mit `\r`
    // und Leerzeichen. In einer Roehre gibt es keinen Wagenruecklauf:
    // Die Leerzeichen stehen dann einfach da, und aus dem Verlauf wurde
    // „Schritt 1 …                    → list_directory". **Was den
    // Bildschirm zurueckspult, setzt einen Bildschirm voraus.**
    let am_schirm = std::io::stdout().is_terminal();
    let zeile_frei = move || {
        if am_schirm {
            print!("\r{}\r", " ".repeat(28));
            let _ = std::io::stdout().flush();
        }
    };
    let melder = |m: myl_client::Meldung<'_>| match m {
        myl_client::Meldung::Schritt(n) => {
            if am_schirm {
                print!("\r  Schritt {n} …");
                let _ = std::io::stdout().flush();
            }
        }
        myl_client::Meldung::Aufruf { name, argumente } => {
            zeile_frei();
            println!("  → {name} {}", myl_client::lauf::kurzform(argumente));
        }
        myl_client::Meldung::Ergebnis { name, text } => {
            zeile_frei();
            println!("  ← {name} {}", myl_client::lauf::eine_zeile(text, 60));
        }
        myl_client::Meldung::Abgelehnt { name, grund } => {
            zeile_frei();
            println!("  ⚑ {name} abgelehnt: {grund}");
        }
    };

    println!();
    let aus = myl_client::lauf::fahren_beobachtet(
        modell,
        &ruestung,
        e.agent.schritte as usize,
        e.agent.auch_bezeugtes,
        e.modell.token as u32,
        auftrag,
        Some(&melder),
    );

    zeile_frei();
    match aus.antwort.as_deref() {
        Some(a) => println!("{a}"),
        None => println!("(keine Schlussantwort)"),
    }
    println!();
    println!(
        "  {} Schritte, {:.1} s",
        aus.verlauf.len(),
        aus.sekunden
    );
    println!();
}

/// Ein Pfad, der in eine Zeile passt.
///
/// ⚑ **Gekuerzt wird vorne**, denn hinten steht der Name, und der
/// unterscheidet.
fn kurz(pfad: &str) -> String {
    let breite = banner::fensterbreite().saturating_sub(24).max(24) as usize;
    if pfad.chars().count() <= breite {
        return pfad.to_string();
    }
    let hinten: String = pfad.chars().skip(pfad.chars().count() - breite + 1).collect();
    format!("…{hinten}")
}
