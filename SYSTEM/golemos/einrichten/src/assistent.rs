//! Der Assistent: ein Menue auf der Konsole, Zeile fuer Zeile.
//!
//! ⚑ **Zeilen und keine Vollbildoberflaeche.** GolemOS wird oft ueber
//! eine serielle Leitung bedient, und dort gibt es keine verlaessliche
//! Terminalbeschreibung. Eine Frage, eine Antwort, Enter: Das
//! funktioniert auf jedem Bildschirm und jeder Leitung, und es laesst
//! sich in einer Probe durchspielen, indem man die Antworten als Text
//! hineingibt.
//!
//! ⚑ **Zuerst Sprache und Tastatur** (Wunsch des Projektinhabers). Wer
//! beides noch nicht gewaehlt hat, wird vor allem anderen gefragt, mit
//! Ziffern, denn die liegen auf fast jeder Tastatur gleich.
//!
//! ⛔️ **Geloescht wird nur nach dem getippten Namen der Platte.** Kein
//! „j", kein Enter: Wer `sdb` tippen muss, liest vorher, was dort steht.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use crate::geraete::{gb, Geraet};
use crate::klon::Artefakt;
use crate::plan::{self, Plan};
use crate::platten;
use crate::sprache::{Sprache, BELEGUNGEN};

/// Was der Assistent ueber das laufende System weiss.
#[derive(Clone, Debug, Default)]
pub struct Lage {
    /// Die Startplatte, fuer Menschen beschrieben.
    pub start: Option<String>,
    /// Groesse und Belegung der Datenpartition, wenn sie eingehaengt ist.
    pub daten: Option<(u64, u64)>,
    pub klon: Option<PathBuf>,
    /// Was der Myelith-Ordner belegt; er kommt bei jeder Installation mit.
    pub klon_groesse: u64,
    /// Was im Myelith-Ordner fehlt, gemessen an `klon::VOLLSTAENDIG`.
    pub klon_fehlt: Vec<&'static str>,
    pub artefakte: Vec<Artefakt>,
    /// Die gewaehlte Sprache; `None`, solange niemand gewaehlt hat.
    pub sprache: Option<Sprache>,
    /// Die gewaehlte Tastaturbelegung (Kartenname fuer `loadkeys`).
    pub tastatur: Option<String>,
}

/// Alles, was der Assistent an der Welt tut. Die Proben setzen eine
/// Attrappe ein; `main.rs` die echte.
pub trait Welt {
    fn geraete(&mut self) -> io::Result<Vec<Geraet>>;
    fn lage(&mut self) -> Lage;
    fn suchen(&mut self, aus: &mut dyn Write) -> io::Result<()>;
    /// `mitnehmen`: die uebrigen Daten des Sticks; `klon`: der
    /// Myelith-Ordner, der immer mitkommt.
    fn installieren(&mut self, plan: &Plan, mitnehmen: bool, klon: Option<&Path>, aus: &mut dyn Write) -> io::Result<()>;
    /// Ruft `spread.sh` aus dem Myelith-Ordner; ohne Geraet zeigt es nur,
    /// was in Frage kommt.
    fn spread(&mut self, klon: &Path, geraet: Option<&str>) -> io::Result<()>;
    fn ausschalten(&mut self);
    /// Startet die Konsole `myelith` und kehrt zurueck, wenn sie endet.
    fn myelith(&mut self) -> io::Result<()>;
    /// Merkt sich, dass der Assistent nicht mehr von selbst starten soll.
    fn gesehen(&mut self);
    /// Speichert die Sprache und gibt sie an Myelith weiter.
    fn sprache_setzen(&mut self, s: Sprache) -> io::Result<()>;
    /// Speichert die Tastaturbelegung und laedt sie sofort.
    fn tastatur_setzen(&mut self, karte: &str) -> io::Result<()>;
}

pub fn lauf<W: Welt>(welt: &mut W, ein: &mut dyn BufRead, aus: &mut dyn Write) -> io::Result<()> {
    let erste = welt.lage();
    if erste.sprache.is_none() && !sprache_waehlen(welt, ein, aus)? {
        return Ok(());
    }
    if erste.tastatur.is_none() {
        let s = welt.lage().sprache.unwrap_or_default();
        if !tastatur_waehlen(welt, s, ein, aus)? {
            return Ok(());
        }
    }
    loop {
        let lage = welt.lage();
        let s = lage.sprache.unwrap_or_default();
        kopf(&lage, s, aus)?;
        // ⚑ **Eine Empfehlung statt einer Liste gleichwertiger Punkte.**
        //   Wer zum ersten Mal vor diesem Menue sitzt, weiss nicht, womit
        //   er anfangen soll; der Assistent weiss es aus der Lage.
        let empfohlen = if lage.artefakte.is_empty() { "2" } else { "1" };
        let marke = s.t("   <- empfohlen", "   <- recommended");
        let punkt = |n: &str, text: &str| format!("  {n}  {text}{}", if n == empfohlen { marke } else { "" });
        writeln!(aus)?;
        writeln!(aus, "{}", punkt("1", s.t("Myelith jetzt benutzen, ohne etwas zu installieren", "Use Myelith now, without installing anything")))?;
        writeln!(aus, "{}", punkt("2", s.t("Myelith-Ordner auf diesen Stick bringen (Anleitung)", "Bring the Myelith folder onto this stick (guide)")))?;
        writeln!(aus, "{}", punkt("3", s.t("GolemOS dauerhaft auf eine Platte installieren", "Install GolemOS permanently on a disk")))?;
        writeln!(aus, "{}", punkt("4", s.t("Einen weiteren GolemOS-Stick schreiben", "Write another GolemOS stick")))?;
        writeln!(aus, "{}", punkt("5", s.t("Noch einmal nach Myelith suchen", "Search for Myelith again")))?;
        writeln!(aus, "{}", punkt("6", s.t("Zur Konsole (spaeter wieder mit: golem-einrichten)", "To the console (later again with: golem-einrichten)")))?;
        writeln!(aus, "{}", punkt("7", s.t("Sprache und Tastatur / Language and keyboard", "Language and keyboard / Sprache und Tastatur")))?;
        let Some(wahl) = frage(ein, aus, s.t("\nAuswahl: ", "\nChoice: "))? else { return Ok(()) };
        match wahl.as_str() {
            "1" => {
                if lage.artefakte.is_empty() {
                    writeln!(aus, "\n{}", s.t(
                        "Noch ist kein Modell da. Zuerst muss der Myelith-Ordner auf den Stick, siehe Punkt 2.",
                        "There is no model yet. First the Myelith folder has to go onto the stick, see item 2.",
                    ))?;
                } else {
                    writeln!(aus, "\n{}", s.t(
                        "Myelith startet. Beenden mit /ende, danach kommst du hierher zurueck.\n\
                         Alles laeuft vom Stick; was du einstellst, bleibt auf ihm gespeichert.",
                        "Myelith is starting. Quit with /ende, then you come back here.\n\
                         Everything runs from the stick; whatever you set up stays on it.",
                    ))?;
                    if let Err(f) = welt.myelith() {
                        writeln!(aus, "{} {f}", s.t("Myelith liess sich nicht starten:", "Myelith could not be started:"))?;
                    }
                }
            }
            "2" => {
                anleitung(s, aus)?;
                if ja(ein, aus, s, s.t("Jetzt ausschalten? [j/N] ", "Switch off now? [y/N] "), false)? {
                    welt.ausschalten();
                    return Ok(());
                }
            }
            "3" => installieren(welt, &lage, s, ein, aus)?,
            "4" => stick_schreiben(welt, &lage, s, ein, aus)?,
            "5" => welt.suchen(aus)?,
            "6" | "q" => {
                welt.gesehen();
                return Ok(());
            }
            "7" => {
                if sprache_waehlen(welt, ein, aus)? {
                    let s = welt.lage().sprache.unwrap_or_default();
                    tastatur_waehlen(welt, s, ein, aus)?;
                }
            }
            _ => writeln!(aus, "{}", s.t("Bitte eine Zahl von 1 bis 7.", "Please a number from 1 to 7."))?,
        }
    }
}

/// Fragt die Sprache, in beiden Sprachen; `false` am Ende der Eingabe.
fn sprache_waehlen<W: Welt>(welt: &mut W, ein: &mut dyn BufRead, aus: &mut dyn Write) -> io::Result<bool> {
    writeln!(aus, "\n════════ GolemOS ════════")?;
    writeln!(aus, "Sprache / Language\n")?;
    writeln!(aus, "  1  Deutsch")?;
    writeln!(aus, "  2  English")?;
    loop {
        let Some(w) = frage(ein, aus, "\nAuswahl / Choice: ")? else { return Ok(false) };
        let s = match w.as_str() {
            "1" | "" => Sprache::De,
            "2" => Sprache::En,
            _ => continue,
        };
        if let Err(f) = welt.sprache_setzen(s) {
            writeln!(aus, "{} {f}", s.t("Die Sprache liess sich nicht speichern:", "The language could not be saved:"))?;
        }
        return Ok(true);
    }
}

/// Fragt die Tastaturbelegung; `false` am Ende der Eingabe.
fn tastatur_waehlen<W: Welt>(welt: &mut W, s: Sprache, ein: &mut dyn BufRead, aus: &mut dyn Write) -> io::Result<bool> {
    writeln!(aus, "\n{}\n", s.t("Tastatur", "Keyboard"))?;
    for (i, b) in BELEGUNGEN.iter().enumerate() {
        writeln!(aus, "  {:>2}  {}", i + 1, b.name)?;
    }
    let vorgabe = if s == Sprache::De { 1 } else { 3 };
    loop {
        let text = format!("\n{} [{vorgabe}]: ", s.t("Auswahl", "Choice"));
        let Some(w) = frage(ein, aus, &text)? else { return Ok(false) };
        let n = if w.is_empty() { vorgabe } else { w.parse::<usize>().unwrap_or(0) };
        let Some(b) = n.checked_sub(1).and_then(|i| BELEGUNGEN.get(i)) else { continue };
        match welt.tastatur_setzen(b.karte) {
            Ok(()) => writeln!(aus, "{} {}", s.t("Tastatur:", "Keyboard:"), b.name)?,
            Err(f) => writeln!(aus, "{} {f}", s.t("Die Tastatur liess sich nicht laden:", "The keyboard could not be loaded:"))?,
        }
        return Ok(true);
    }
}

fn kopf(l: &Lage, s: Sprache, aus: &mut dyn Write) -> io::Result<()> {
    writeln!(aus, "\n════════ {} ════════", s.t("GolemOS einrichten", "GolemOS setup"))?;
    let unbekannt = s.t("nicht zu erkennen", "not recognised");
    writeln!(aus, "{} {}", s.t("Gestartet von: ", "Started from:  "), l.start.as_deref().unwrap_or(unbekannt))?;
    match l.daten {
        Some((g, b)) => writeln!(aus, "{} {}, {} {} {}", s.t("Datenpartition:", "Data part:     "), gb(g), s.t("davon", "of which"), gb(b), s.t("belegt", "used"))?,
        None => writeln!(aus, "{} {}", s.t("Datenpartition:", "Data part:     "), s.t("nicht eingehaengt", "not mounted"))?,
    }
    let feld = s.t("Myelith:       ", "Myelith:       ");
    match &l.klon {
        Some(k) if l.klon_fehlt.is_empty() => writeln!(
            aus,
            "{feld} {} {} ({}, {})",
            s.t("Klon unter", "folder at"),
            k.display(),
            gb(l.klon_groesse),
            s.t("vollstaendig", "complete")
        )?,
        Some(k) => {
            writeln!(aus, "{feld} {} {}, {}", s.t("Klon unter", "folder at"), k.display(), s.t("UNVOLLSTAENDIG", "INCOMPLETE"))?;
            writeln!(aus, "                {} {}", s.t("es fehlen:", "missing:"), l.klon_fehlt.join(", "))?;
            writeln!(aus, "                {}", s.t("Bitte den ganzen Myelith-Ordner kopieren (Punkt 2).", "Please copy the whole Myelith folder (item 2)."))?
        }
        None => writeln!(aus, "{feld} {}", s.t("kein Klon gefunden", "no folder found"))?,
    }
    let namen: Vec<String> = l.artefakte.iter().map(|a| format!("{} ({})", a.name, gb(a.groesse))).collect();
    let keine = s.t("keine", "none");
    writeln!(aus, "{} {}", s.t("Modelle:       ", "Models:        "), if namen.is_empty() { keine.to_string() } else { namen.join(", ") })
}

/// Der Weg, den der Projektinhaber gewaehlt hat: einmal starten, dann
/// den ganzen Ordner am eigenen Rechner hinueberziehen.
fn anleitung(s: Sprache, aus: &mut dyn Write) -> io::Result<()> {
    writeln!(
        aus,
        "{}",
        s.t(
            "\nSo kommt Myelith auf diesen Stick:\n\
             \n\
             \x20 1. GolemOS ausschalten und den Stick abziehen.\n\
             \x20 2. Den Stick an deinen Rechner stecken. Er erscheint als \"Golem\",\n\
             \x20    darin eine Datei README.txt.\n\
             \x20 3. Deinen GANZEN Myelith-Ordner daraufziehen, samt den Modellen unter\n\
             \x20    INTEGER_LLM/artifacts. Der ganze Ordner, weil GolemOS daraus auch\n\
             \x20    selbst neue Sticks schreibt. Erkannt wird er an\n\
             \x20      INTEGER_LLM/scripts/build_artifacts.sh\n\
             \x20    Weglassen darfst du nur, was ein frischer Klon gar nicht hat:\n\
             \x20    SYSTEM/full-build, SYSTEM/crates-lager, die Rohgewichte unter MODELS\n\
             \x20    und Ordner namens .venv (die enthalten Verknuepfungen, die FAT32\n\
             \x20    nicht speichern kann).\n\
             \x20 4. Den Stick auswerfen und wieder von ihm starten.\n\
             \n\
             GolemOS findet den Ordner beim Start, auch auf einem zweiten Laufwerk,\n\
             und waehlt ein Modell, das in den Arbeitsspeicher passt.",
            "\nHow Myelith gets onto this stick:\n\
             \n\
             \x20 1. Switch GolemOS off and unplug the stick.\n\
             \x20 2. Plug the stick into your computer. It shows up as \"Golem\",\n\
             \x20    with a file README.txt in it.\n\
             \x20 3. Drag your WHOLE Myelith folder onto it, including the models under\n\
             \x20    INTEGER_LLM/artifacts. The whole folder, because GolemOS also\n\
             \x20    writes new sticks from it. It is recognised by\n\
             \x20      INTEGER_LLM/scripts/build_artifacts.sh\n\
             \x20    You may leave out only what a fresh clone does not contain:\n\
             \x20    SYSTEM/full-build, SYSTEM/crates-lager, the raw weights under MODELS\n\
             \x20    and folders named .venv (they contain links FAT32 cannot store).\n\
             \x20 4. Eject the stick and start from it again.\n\
             \n\
             GolemOS finds the folder at startup, also on a second drive, and picks\n\
             a model that fits into the memory.",
        )
    )
}

fn installieren<W: Welt>(welt: &mut W, lage: &Lage, s: Sprache, ein: &mut dyn BufRead, aus: &mut dyn Write) -> io::Result<()> {
    let zurueck = s.t("Zurueck, nichts wurde veraendert.", "Back, nothing was changed.");
    let g = welt.geraete()?;
    let Some(start) = platten::startplatte(&g) else {
        writeln!(aus, "\n{}", s.t(
            "Die Platte, von der GolemOS laeuft, ist nicht zu erkennen; ohne sie gibt es nichts zu kopieren.",
            "The disk GolemOS is running from cannot be recognised; without it there is nothing to copy.",
        ))?;
        return Ok(());
    };
    let q = match platten::quelle(&g, &start) {
        Ok(q) => q,
        Err(f) => {
            writeln!(aus, "\n{f}")?;
            return Ok(());
        }
    };
    let urteile = platten::beurteilen(&g, &start, q.mindestgroesse());
    let geeignet: Vec<&Geraet> = urteile.iter().filter(|(_, u)| u.geeignet()).map(|(p, _)| p).collect();

    writeln!(aus, "\n{}", s.t("Platten:", "Disks:"))?;
    let mut nummer = 0;
    for (p, u) in &urteile {
        let marke = if u.geeignet() {
            nummer += 1;
            format!("{nummer:>2}")
        } else {
            " -".into()
        };
        let grund = if u.geeignet() { String::new() } else { u.grund_in(s) };
        writeln!(aus, "  {marke}  /dev/{:<10} {:>10}  {:<24} {grund}", p.name, gb(p.groesse), p.modell)?;
    }
    if geeignet.is_empty() {
        writeln!(
            aus,
            "\n{} {}.",
            s.t(
                "Keine Platte kommt in Frage. Gebraucht wird eine, von der GolemOS nicht laeuft, die nirgends eingehaengt ist und mindestens",
                "No disk qualifies. It needs one GolemOS is not running from, that is not mounted anywhere and holds at least",
            ),
            gb(q.mindestgroesse())
        )?;
        return Ok(());
    }

    let Some(wahl) = frage(ein, aus, s.t("\nAuf welche Platte? Nummer, leer fuer zurueck: ", "\nWhich disk? Number, empty to go back: "))? else { return Ok(()) };
    let Some(ziel) = wahl.parse::<usize>().ok().and_then(|n| n.checked_sub(1)).and_then(|i| geeignet.get(i)) else {
        writeln!(aus, "{zurueck}")?;
        return Ok(());
    };
    let plan = match plan::entwerfen(&q, ziel, &g) {
        Ok(p) => p,
        Err(f) => {
            writeln!(aus, "{f}")?;
            return Ok(());
        }
    };

    writeln!(aus, "\n{}", plan.beschreiben_in(s))?;
    let alt: Vec<String> = g
        .iter()
        .filter(|x| x.eltern == plan.ziel)
        .map(|x| format!("  /dev/{} {} {} {}", x.name, gb(x.groesse), x.dateisystem, x.marke).trim_end().to_string())
        .collect();
    writeln!(aus, "⛔️ {} /dev/{} {}", s.t("ALLES auf", "EVERYTHING on"), plan.ziel, s.t("wird geloescht:", "will be erased:"))?;
    if alt.is_empty() {
        writeln!(aus, "  {}", s.t("(keine Partitionen erkannt)", "(no partitions found)"))?;
    } else {
        for z in alt {
            writeln!(aus, "{z}")?;
        }
    }

    // ⛔️ **Der Myelith-Ordner kommt immer mit** (Festlegung des
    //    Projektinhabers): Ohne ihn hat das installierte System keine
    //    Modelle und kann keine Sticks schreiben. Freiwillig ist nur der
    //    Rest der Datenpartition, also Einstellungen und Arbeitsordner.
    let klon = lage.klon.as_deref();
    match klon {
        Some(k) => writeln!(
            aus,
            "\n{} {} ({}) {}",
            s.t("Der Myelith-Ordner", "The Myelith folder"),
            k.display(),
            gb(lage.klon_groesse),
            s.t("kommt mit.", "comes along.")
        )?,
        None => {
            writeln!(aus, "\n⚠️  {}", s.t(
                "Es ist kein Myelith-Ordner da. Das installierte GolemOS haette dann kein\n   Modell und koennte keine Sticks schreiben. Besser zuerst Punkt 2.",
                "There is no Myelith folder. The installed GolemOS would then have no\n   model and could not write sticks. Better do item 2 first.",
            ))?;
            if !ja(ein, aus, s, s.t("Trotzdem installieren? [j/N] ", "Install anyway? [y/N] "), false)? {
                writeln!(aus, "{zurueck}")?;
                return Ok(());
            }
        }
    }
    let klon_auf_daten = klon.is_some_and(|k| k.starts_with(crate::start::DATEN));
    let mut mitnehmen = false;
    let mut bedarf = if klon.is_some() { lage.klon_groesse } else { 0 };
    if let Some((_, belegt)) = lage.daten {
        let rest = if klon_auf_daten { belegt.saturating_sub(lage.klon_groesse) } else { belegt };
        if rest > 0 && bedarf + rest <= plan.daten_platz {
            let text = format!(
                "{} ({}: {})? {} ",
                s.t("Auch deine uebrigen Daten vom Stick mitnehmen", "Also take your other data from the stick"),
                gb(rest),
                s.t("Einstellungen, Arbeitsordner", "settings, work folder"),
                s.t("[J/n]", "[Y/n]")
            );
            mitnehmen = ja(ein, aus, s, &text, true)?;
            if mitnehmen {
                bedarf += rest;
            }
        }
    }
    if bedarf > plan.daten_platz {
        writeln!(
            aus,
            "\n{} ({}) {} ({} {}). {}",
            s.t("Der Myelith-Ordner", "The Myelith folder"),
            gb(bedarf),
            s.t("passt nicht auf die Platte", "does not fit on the disk"),
            gb(plan.daten_platz),
            s.t("frei", "free"),
            s.t("Eine groessere Platte waehlen oder weniger Modelle in den Ordner legen.", "Choose a larger disk or put fewer models into the folder.")
        )?;
        return Ok(());
    }

    let text = format!("\n{} ({}): ", s.t("Zum Bestaetigen den Namen der Platte eintippen", "To confirm, type the disk's name"), plan.ziel);
    let Some(bestaetigung) = frage(ein, aus, &text)? else {
        return Ok(());
    };
    if bestaetigung != plan.ziel {
        writeln!(aus, "{}", s.t("Abgebrochen, nichts wurde veraendert.", "Cancelled, nothing was changed."))?;
        return Ok(());
    }

    writeln!(aus)?;
    if let Err(f) = welt.installieren(&plan, mitnehmen, klon, aus) {
        writeln!(aus, "\n⛔️ {} {f}", s.t("Die Installation ist gescheitert:", "The installation failed:"))?;
        writeln!(
            aus,
            "{} /dev/{} {}",
            s.t("Die Platte", "The disk"),
            plan.ziel,
            s.t("ist jetzt in einem unfertigen Zustand; ein neuer Versuch beginnt von vorn.", "is now in an unfinished state; a new attempt starts from scratch.")
        )?;
        return Ok(());
    }
    match s {
        Sprache::De => writeln!(
            aus,
            "\nGolemOS ist auf /dev/{} installiert. Ausschalten, den Stick abziehen und von der Platte starten.",
            plan.ziel
        )?,
        Sprache::En => writeln!(
            aus,
            "\nGolemOS is installed on /dev/{}. Switch off, unplug the stick and start from the disk.",
            plan.ziel
        )?,
    }
    if ja(ein, aus, s, s.t("Jetzt ausschalten? [j/N] ", "Switch off now? [y/N] "), false)? {
        welt.ausschalten();
    }
    Ok(())
}

/// Einen weiteren GolemOS-Stick schreiben, mit `spread.sh` aus dem
/// Myelith-Ordner.
///
/// ⚑ **Dasselbe Skript wie am eigenen Rechner**, mit seinen eigenen
/// Sicherungen (nur wechselbare Datentraeger, nie der laufende, der Name
/// wird abgetippt). Der Assistent fuehrt nur hin; eine zweite Fassung
/// derselben Sicherungen haette hier nichts zu suchen.
fn stick_schreiben<W: Welt>(welt: &mut W, lage: &Lage, s: Sprache, ein: &mut dyn BufRead, aus: &mut dyn Write) -> io::Result<()> {
    let Some(k) = &lage.klon else {
        writeln!(aus, "\n{}", s.t(
            "Dafuer braucht GolemOS den ganzen Myelith-Ordner; siehe Punkt 2.",
            "For this GolemOS needs the whole Myelith folder; see item 2.",
        ))?;
        return Ok(());
    };
    if lage.klon_fehlt.iter().any(|p| p.starts_with("SYSTEM/golemos")) {
        writeln!(
            aus,
            "\n{} {}; {}",
            s.t("Im Myelith-Ordner fehlt", "The Myelith folder lacks"),
            lage.klon_fehlt.join(", "),
            s.t("ohne das laesst sich kein Stick schreiben.", "without it no stick can be written.")
        )?;
        writeln!(aus, "{}", s.t("Bitte den ganzen Ordner kopieren (Punkt 2).", "Please copy the whole folder (item 2)."))?;
        return Ok(());
    }
    writeln!(aus, "\n{}\n", s.t(
        "Steck den neuen Stick jetzt ein. ⚠️  Alles darauf wird geloescht.",
        "Plug in the new stick now. ⚠️  Everything on it will be erased.",
    ))?;
    let _ = frage(ein, aus, s.t("Enter, sobald er steckt: ", "Enter once it is plugged in: "))?;
    welt.spread(k, None)?;
    let Some(geraet) = frage(ein, aus, s.t("\nWelcher Datentraeger? Etwa /dev/sda, leer fuer zurueck: ", "\nWhich drive? For example /dev/sda, empty to go back: "))? else {
        return Ok(());
    };
    if geraet.is_empty() {
        writeln!(aus, "{}", s.t("Zurueck, nichts wurde veraendert.", "Back, nothing was changed."))?;
        return Ok(());
    }
    if let Err(f) = welt.spread(k, Some(&geraet)) {
        writeln!(aus, "{} {f}", s.t("spread.sh liess sich nicht starten:", "spread.sh could not be started:"))?;
    }
    Ok(())
}

/// Liest eine Antwort; `None` am Ende der Eingabe.
fn frage(ein: &mut dyn BufRead, aus: &mut dyn Write, text: &str) -> io::Result<Option<String>> {
    write!(aus, "{text}")?;
    aus.flush()?;
    let mut z = String::new();
    if ein.read_line(&mut z)? == 0 {
        return Ok(None);
    }
    Ok(Some(z.trim().to_string()))
}

/// Ja oder nein, in beiden Sprachen; alles andere ist die Vorgabe.
fn ja(ein: &mut dyn BufRead, aus: &mut dyn Write, _s: Sprache, text: &str, vorgabe: bool) -> io::Result<bool> {
    Ok(match frage(ein, aus, text)?.as_deref().map(str::to_lowercase).as_deref() {
        Some("j" | "ja" | "y" | "yes") => true,
        Some("n" | "nein" | "no") => false,
        _ => vorgabe,
    })
}

#[cfg(test)]
mod proben {
    use super::*;
    use crate::platten::proben::{stick_und_platte, teil};

    #[derive(Default)]
    struct Attrappe {
        geraete: Vec<Geraet>,
        lage: Lage,
        installiert: Vec<(String, bool)>,
        aus: bool,
        gesehen: bool,
        myelith: usize,
        klon_mit: Vec<Option<PathBuf>>,
        gespreadet: Vec<Option<String>>,
    }

    impl Welt for Attrappe {
        fn geraete(&mut self) -> io::Result<Vec<Geraet>> {
            Ok(self.geraete.clone())
        }
        fn lage(&mut self) -> Lage {
            self.lage.clone()
        }
        fn suchen(&mut self, aus: &mut dyn Write) -> io::Result<()> {
            writeln!(aus, "gesucht")
        }
        fn installieren(&mut self, plan: &Plan, mitnehmen: bool, klon: Option<&Path>, _: &mut dyn Write) -> io::Result<()> {
            self.installiert.push((plan.ziel.clone(), mitnehmen));
            self.klon_mit.push(klon.map(Path::to_path_buf));
            Ok(())
        }
        fn spread(&mut self, _: &Path, geraet: Option<&str>) -> io::Result<()> {
            self.gespreadet.push(geraet.map(String::from));
            Ok(())
        }
        fn sprache_setzen(&mut self, s: Sprache) -> io::Result<()> {
            self.lage.sprache = Some(s);
            Ok(())
        }
        fn tastatur_setzen(&mut self, karte: &str) -> io::Result<()> {
            self.lage.tastatur = Some(karte.to_string());
            Ok(())
        }
        fn ausschalten(&mut self) {
            self.aus = true;
        }
        fn myelith(&mut self) -> io::Result<()> {
            self.myelith += 1;
            Ok(())
        }
        fn gesehen(&mut self) {
            self.gesehen = true;
        }
    }

    fn spielen(welt: &mut Attrappe, eingabe: &str) -> String {
        let mut aus = Vec::new();
        lauf(welt, &mut eingabe.as_bytes(), &mut aus).unwrap();
        String::from_utf8(aus).unwrap()
    }

    fn welt() -> Attrappe {
        Attrappe {
            geraete: stick_und_platte(),
            lage: Lage {
                daten: Some((2 << 30, 900 << 20)),
                klon: Some(PathBuf::from("/daten/Myelith")),
                klon_groesse: 800 << 20,
                sprache: Some(Sprache::De),
                tastatur: Some("de-latin1-nodeadkeys".into()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn installiert_nach_getipptem_namen() {
        let mut w = welt();
        let aus = spielen(&mut w, "3\n1\n\nvdb\nn\n6\n");
        assert_eq!(w.installiert, [("vdb".to_string(), true)], "{aus}");
        assert!(aus.contains("ALLES auf /dev/vdb wird geloescht"), "{aus}");
        assert!(aus.contains("Der Myelith-Ordner /daten/Myelith (0,8 GB) kommt mit."), "{aus}");
        assert!(!w.aus);
    }

    /// ⛔️ Die Gegenprobe: ein „j" statt des Namens loescht nichts.
    #[test]
    fn falsche_bestaetigung_loescht_nichts() {
        for falsch in ["j", "ja", "", "vda", "VDB", "sdb"] {
            let mut w = welt();
            let aus = spielen(&mut w, &format!("3\n1\n\n{falsch}\n6\n"));
            assert!(w.installiert.is_empty(), "'{falsch}': {aus}");
            assert!(aus.contains("Abgebrochen, nichts wurde veraendert"), "'{falsch}': {aus}");
        }
    }

    #[test]
    fn startplatte_wird_nicht_angeboten() {
        let mut w = welt();
        let aus = spielen(&mut w, "3\n\n6\n");
        assert!(aus.contains("-  /dev/vda"), "{aus}");
        assert!(aus.contains("von dieser Platte laeuft GolemOS gerade"), "{aus}");
        assert!(aus.contains(" 1  /dev/vdb"), "{aus}");
    }

    #[test]
    fn ohne_geeignete_platte_sagt_er_warum() {
        let mut w = welt();
        w.geraete.truncate(5); // nur der Stick
        let aus = spielen(&mut w, "3\n6\n");
        assert!(aus.contains("Keine Platte kommt in Frage"), "{aus}");
        assert!(w.installiert.is_empty());
    }

    /// ⛔️ **Der Ordner kommt auch mit, wenn der Rest bleibt.**
    #[test]
    fn ordner_kommt_immer_mit() {
        let mut w = welt();
        spielen(&mut w, "3\n1\nn\nvdb\nn\n6\n");
        assert_eq!(w.installiert, [("vdb".to_string(), false)]);
        assert_eq!(w.klon_mit, [Some(PathBuf::from("/daten/Myelith"))]);
    }

    /// Passt der Rest nicht, wird er nicht angeboten; der Ordner kommt trotzdem.
    #[test]
    fn zu_viele_uebrige_daten_werden_nicht_angeboten() {
        let mut w = welt();
        w.lage.daten = Some((100 << 30, 50 << 30));
        let aus = spielen(&mut w, "3\n1\nvdb\nn\n6\n");
        assert!(!aus.contains("uebrigen Daten"), "{aus}");
        assert_eq!(w.installiert, [("vdb".to_string(), false)]);
        assert_eq!(w.klon_mit, [Some(PathBuf::from("/daten/Myelith"))]);
    }

    /// Passt der Ordner selbst nicht, wird gar nicht installiert.
    #[test]
    fn zu_grosser_ordner_verhindert_die_installation() {
        let mut w = welt();
        w.lage.klon_groesse = 10 << 30;
        w.lage.daten = Some((20 << 30, 10 << 30));
        let aus = spielen(&mut w, "3\n1\n6\n");
        assert!(aus.contains("passt nicht auf die Platte"), "{aus}");
        assert!(w.installiert.is_empty());
    }

    /// Ohne Ordner fragt er nach, und die Vorgabe ist nein.
    #[test]
    fn ohne_ordner_nur_auf_ausdruecklichen_wunsch() {
        let mut w = welt();
        w.lage.klon = None;
        let aus = spielen(&mut w, "3\n1\n\n6\n");
        assert!(aus.contains("Es ist kein Myelith-Ordner da"), "{aus}");
        assert!(w.installiert.is_empty(), "{aus}");
        let mut w = welt();
        w.lage.klon = None;
        spielen(&mut w, "3\n1\nj\n\nvdb\nn\n6\n");
        assert_eq!(w.klon_mit, [None]);
    }

    #[test]
    fn eingehaengte_platte_wird_nicht_angeboten() {
        let mut w = welt();
        w.geraete.push(teil("vdb1", "vdb", "", 1 << 30, "/medien/vdb1"));
        let aus = spielen(&mut w, "3\n6\n");
        assert!(aus.contains("eingehaengt unter /medien/vdb1"), "{aus}");
        assert!(w.installiert.is_empty());
    }

    #[test]
    fn unvollstaendiger_ordner_wird_benannt() {
        let mut w = welt();
        w.lage.klon_fehlt = vec!["SYSTEM/golemos/spread.sh"];
        let aus = spielen(&mut w, "6\n");
        assert!(aus.contains("UNVOLLSTAENDIG"), "{aus}");
        assert!(aus.contains("es fehlen: SYSTEM/golemos/spread.sh"), "{aus}");
    }

    #[test]
    fn stick_schreiben_ueber_spread() {
        let mut w = welt();
        spielen(&mut w, "4\n\n/dev/sda\n6\n");
        assert_eq!(w.gespreadet, [None, Some("/dev/sda".to_string())]);
        // Leer heisst zurueck: nur die Liste, kein Schreiben.
        let mut w = welt();
        spielen(&mut w, "4\n\n\n6\n");
        assert_eq!(w.gespreadet, [None]);
    }

    #[test]
    fn stick_schreiben_braucht_den_ganzen_ordner() {
        let mut w = welt();
        w.lage.klon_fehlt = vec!["SYSTEM/golemos/abbild/fertig"];
        let aus = spielen(&mut w, "4\n6\n");
        assert!(w.gespreadet.is_empty());
        assert!(aus.contains("ohne das laesst sich kein Stick schreiben"), "{aus}");
        let mut w = welt();
        w.lage.klon = None;
        spielen(&mut w, "4\n6\n");
        assert!(w.gespreadet.is_empty());
    }

    #[test]
    fn anleitung_und_ausschalten() {
        let mut w = welt();
        let aus = spielen(&mut w, "2\nj\n");
        assert!(aus.contains("\"Golem\""), "{aus}");
        assert!(aus.contains("GANZEN Myelith-Ordner"), "{aus}");
        assert!(w.aus);
    }

    #[test]
    fn zur_konsole_merkt_es_sich() {
        let mut w = welt();
        spielen(&mut w, "6\n");
        assert!(w.gesehen);
        // Ende der Eingabe ist kein „gesehen": Der Assistent kam nicht zu Wort.
        let mut w = welt();
        spielen(&mut w, "");
        assert!(!w.gesehen);
    }

    /// Ohne Modell startet Myelith nicht, und der Assistent sagt, was fehlt.
    #[test]
    fn ohne_modell_kein_start_und_empfehlung_punkt_zwei() {
        let mut w = welt();
        let aus = spielen(&mut w, "1\n6\n");
        assert_eq!(w.myelith, 0);
        assert!(aus.contains("Noch ist kein Modell da"), "{aus}");
        assert!(aus.contains("(Anleitung)   <- empfohlen"), "{aus}");
    }

    /// Mit Modell startet Punkt 1 die Konsole, danach kommt das Menue wieder.
    #[test]
    fn mit_modell_startet_myelith_ohne_installation() {
        let mut w = welt();
        w.lage.artefakte = vec![Artefakt { name: "myelith-0.6b".into(), pfad: "/x".into(), groesse: 1 }];
        let aus = spielen(&mut w, "1\n6\n");
        assert_eq!(w.myelith, 1);
        assert!(w.installiert.is_empty());
        assert!(aus.contains("ohne etwas zu installieren   <- empfohlen"), "{aus}");
        assert_eq!(aus.matches("Auswahl: ").count(), 2, "zurueck ins Menue: {aus}");
    }

    /// ⚑ **Beim allerersten Start: Sprache, dann Tastatur, dann das Menue.**
    #[test]
    fn erster_start_fragt_sprache_und_tastatur() {
        let mut w = welt();
        w.lage.sprache = None;
        w.lage.tastatur = None;
        let aus = spielen(&mut w, "2\n4\n6\n");
        assert_eq!(w.lage.sprache, Some(Sprache::En));
        assert_eq!(w.lage.tastatur.as_deref(), Some("uk"));
        assert!(aus.contains("Sprache / Language"), "{aus}");
        assert!(aus.contains("Keyboard:"), "{aus}");
        assert!(aus.contains("Use Myelith now"), "englisches Menue: {aus}");
        assert!(aus.find("Sprache / Language").unwrap() < aus.find("Choice: ").unwrap());
    }

    /// Enter nimmt die Vorgabe: Deutsch, und dazu die deutsche Tastatur.
    #[test]
    fn enter_nimmt_deutsch_und_deutsche_tastatur() {
        let mut w = welt();
        w.lage.sprache = None;
        w.lage.tastatur = None;
        spielen(&mut w, "\n\n6\n");
        assert_eq!(w.lage.sprache, Some(Sprache::De));
        assert_eq!(w.lage.tastatur.as_deref(), Some("de-latin1-nodeadkeys"));
    }

    /// Englisch nimmt als Vorgabe die US-Tastatur.
    #[test]
    fn englisch_nimmt_us_als_vorgabe() {
        let mut w = welt();
        w.lage.sprache = None;
        w.lage.tastatur = None;
        spielen(&mut w, "2\n\n6\n");
        assert_eq!(w.lage.tastatur.as_deref(), Some("us"));
    }

    /// Wer schon gewaehlt hat, wird nicht noch einmal gefragt; Punkt 7 fragt neu.
    #[test]
    fn gewaehlt_ist_gewaehlt_und_punkt_sieben_aendert() {
        let mut w = welt();
        let aus = spielen(&mut w, "6\n");
        assert!(!aus.contains("Sprache / Language\n"), "{aus}");
        let mut w = welt();
        spielen(&mut w, "7\n2\n5\n6\n");
        assert_eq!(w.lage.sprache, Some(Sprache::En));
        assert_eq!(w.lage.tastatur.as_deref(), Some("fr-latin9"));
    }

    /// Auch die Installation spricht Englisch, bis zur Bestaetigung.
    #[test]
    fn installation_auf_englisch() {
        let mut w = welt();
        w.lage.sprache = Some(Sprache::En);
        let aus = spielen(&mut w, "3\n1\n\nvdb\nn\n6\n");
        assert_eq!(w.installiert, [("vdb".to_string(), true)], "{aus}");
        assert!(aus.contains("EVERYTHING on /dev/vdb will be erased"), "{aus}");
        assert!(aus.contains("GolemOS is running from this disk"), "{aus}");
        assert!(aus.contains("GolemOS is installed on /dev/vdb. Switch off"), "{aus}");
        assert!(aus.contains("Target disk /dev/vdb"), "{aus}");
    }

    /// Die Anleitung nennt die Marke, an der der Klon erkannt wird;
    /// stimmt sie nicht, sucht der Mensch nach der falschen Datei.
    #[test]
    fn anleitung_nennt_die_echte_marke() {
        let mut aus = Vec::new();
        anleitung(Sprache::De, &mut aus).unwrap();
        assert!(String::from_utf8(aus).unwrap().contains(crate::klon::MARKE));
    }
}
