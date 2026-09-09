//! `myl`, das Bedieninstrument fuer den lokalen Betrieb (CLIENT 0.3).
//!
//! # ⚑ Warum ein Kommandozeilenwerkzeug vor der Oberflaeche
//!
//! **Die Oberflaeche soll dieselben Unterbefehle rufen wie dieses
//! Werkzeug, ohne eigene Logik.** Dann muessen die Unterbefehle zuerst
//! da sein. Eine Oberflaeche, die noch
//! nichts zu rufen hat, waere eine leere Huelle, und sie zoege
//! `wry`, `tao` und die WebKit-Bindungen herein, bevor irgendetwas
//! funktioniert.
//!
//! ⚑ **Dieses Werkzeug ist deshalb nicht der Vorlaeufer der Oberflaeche,
//! sondern ihr Inhalt.** Was hier steht, ruft Tauri spaeter auf.

use myl_client::einstellungen::Einstellungen;
use myl_client::Oertlichesmodell;
use myl_local_agent::{Modellweg, Nachricht};

const HILFE: &str = "\
myl: lokaler Betrieb von Myelith

  myl frage <artefakt> <text>     Eine Frage an das lokale Modell
  myl modell <artefakt>           Was in einem Artefakt steht
  myl agent [artefakt] <auftrag>  Die Agentenschleife, lokal
  myl sitzung [artefakt]          Viele Auftraege, Modell einmal geladen
  myl auftraege [artefakt] A B    Mehrere Auftraege NEBENLAEUFIG
  myl einstellungen               Zeigt die Einstellungen
  myl setzen <feld> <wert>        Aendert eine Einstellung

⚑ Steht in den Einstellungen ein Artefakt, darf es weggelassen werden.

Felder fuer `setzen`:
  modell.artefakt   modell.token      modell.denken
  agent.schritte    agent.bezeugtes   agent.wurzel      agent.schreiben
  kap.kerne         kap.beschleuniger
  kap.speicher      kap.platte

Schalter fuer `frage`:
  --token N       Hoechstzahl erzeugter Token (Vorgabe 256)
  --denken        Denkmodus an (Vorgabe aus, siehe unten)

Schalter fuer `agent`, `sitzung` und `auftraege`, zusaetzlich zu denen
von `frage`:
  --schritte N    Hoechstzahl der Schritte
  --bezeugtes     Laesst bezeugte Werkzeuge zu, nur fuer diesen Lauf
  --wurzel P      Haengt P ein, nur fuer diesen Lauf
  --schreiben     Erlaubt Schreiben, nur fuer diesen Lauf
  --roh           Der volle Nachrichtenverlauf statt der Kurzform
  --deutsch       Werkzeuge deutsch ansagen (Vergleichsschalter, s.u.)
  --werkzeuge S   `knapp` (Vorgabe, drei) oder `voll` (fuenf), s.u.
  --datei P       Nur `auftraege`: je Zeile ein Auftrag

⚑ Bei `auftraege` teilen sich alle Unteragenten EIN geladenes Modell,
und das Kernbudget aus `kap.kerne` wird durch ihre Zahl geteilt: Sonst
wollte jeder alle Kerne, und sie naehmen sie sich gegenseitig weg.

⚑ `--werkzeuge voll` nimmt `search_files` und `edit_file` dazu. Beide
schliessen ein Loch: Ohne Suche heisst „finde, wo X steht“ bei sechs
Schritten lesen, lesen, lesen, Budget alle; und `write_file` ersetzt die
GANZE Datei, `edit_file` nur eine eindeutige Stelle. Vorgabe bleibt
`knapp`, weil fuenf Werkzeuge die Auswahl fuer ein kleines Modell
schwerer machen als drei und noch niemand gemessen hat, was ueberwiegt.

⚑ `--deutsch` ist ein Vergleichsschalter und keine Einstellung. Die
Werkzeuge werden dem Modell sonst in genau der Form angesagt, auf die
es geschliffen wurde: englische Namen und der Wortlaut aus seiner
eigenen Vorlage. Die deutsche Fassung war bis zum 2026-09-09 die
einzige und steht nur noch da, damit die Agentenprobe beide messen
kann. Sie verschwindet, sobald der Vergleich gefallen ist.

⚑ Die Dateiwerkzeuge gibt es erst mit `agent.wurzel`, und sie sind
lokal, also bezeugt und nicht nachrechenbar: Ohne `--bezeugtes` oder
`agent.bezeugtes an` sperrt der Harness sie.

⚑ Der Denkmodus ist ausgeschaltet, weil ein Harness Werkzeugaufrufe
will und keine Ueberlegung, und weil jedes Denktoken dieselbe
Rechenzeit kostet wie ein Antworttoken.
";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1).map(String::as_str) {
        Some("frage") => frage(&args[2..]),
        Some("modell") => modell(&args[2..]),
        Some("agent") => agent(&args[2..]),
        Some("sitzung") => sitzung(&args[2..]),
        Some("auftraege") => auftraege(&args[2..]),
        Some("einstellungen") => einstellungen(),
        Some("setzen") => setzen(&args[2..]),
        // Die Hilfe ist hier eine Antwort und kein Fehler: Sie geht nach
        // stdout und gibt null zurueck.
        None | Some("--hilfe") | Some("-h") | Some("--help") | Some("hilfe") => {
            print!("{HILFE}");
            0
        }
        // ⛑ **Ein Tippfehler war bis hierher ein Erfolg.** Jeder
        // unbekannte Befehl fiel in denselben Zweig wie die Hilfe: Text
        // nach stdout, Rueckgabe null. Ein Skript, das `myl sitzng`
        // schreibt, bekam die Hilfe in seine Pipe und las daraus, alles
        // sei gutgegangen. Aufgefallen beim Bau der Startprobe fuer den
        // Freigabe-Job, die genau davon lebt, dass die Rueckgabe etwas
        // aussagt. Jetzt nach stderr und mit 2, wie es fuer einen
        // Aufruffehler ueblich ist.
        Some(unbekannt) => {
            eprintln!("myl: unbekannter Befehl `{unbekannt}`");
            eprint!("{HILFE}");
            2
        }
    };
    std::process::exit(code);
}

/// ⛑ **Der Freitext ohne Schalter UND ohne deren Werte.**
///
/// Die erste Fassung filterte nur Argumente, die mit `--` beginnen.
/// Damit landete `--token 128` als „128" im Auftrag, und das Modell
/// bekam eine Frage mit angehaengten Zahlen. Ein Filter, der die Haelfte
/// eines Schalters stehen laesst, ist schlimmer als keiner: Er sieht
/// aus, als taete er etwas.
/// Die Schalter, hinter denen ein **Wert** steht.
///
/// ⛑ **Diese Liste stand bis zum 2026-09-08 nur in `freitext`**, und
/// beim Einbau von `--wurzel` fehlte sie dort prompt: Der Pfad wanderte
/// in den Auftragstext, und der Agent bekam einen Auftrag, der auf ein
/// Verzeichnis endete. **Ein vergessener Eintrag stuerzt nicht ab, er
/// verschiebt still eine Bedeutung**, und das ist die schlechtere
/// Sorte Fehler.
///
/// Wer einen Schalter mit Wert hinzufuegt, traegt ihn hier ein;
/// `kein_wertschalter_fehlt` haelt es fest.
const MIT_WERT: [&str; 5] =
    ["--token", "--schritte", "--wurzel", "--datei", "--werkzeuge"];

fn freitext(args: &[String]) -> String {
    let mit_wert = MIT_WERT;
    let mut aus = Vec::new();
    let mut ueberspringen = false;
    for a in args {
        if ueberspringen {
            ueberspringen = false;
            continue;
        }
        if a.starts_with("--") {
            ueberspringen = mit_wert.contains(&a.as_str());
            continue;
        }
        aus.push(a.clone());
    }
    aus.join(" ")
}

/// ⚑ **Das Artefakt darf aus den Einstellungen kommen.**
///
/// Ein Werkzeug, das bei jedem Aufruf denselben langen Pfad verlangt,
/// wird nicht benutzt. Genannt schlaegt eingestellt: Wer es hinschreibt,
/// meint es.
fn artefakt_und_rest<'a>(
    args: &'a [String],
    e: &Einstellungen,
) -> (Option<String>, &'a [String]) {
    match args.first() {
        Some(a) if !a.starts_with("--") && std::path::Path::new(a).is_dir() => {
            (Some(a.clone()), &args[1..])
        }
        _ if !e.modell.artefakt.is_empty() => (Some(e.modell.artefakt.clone()), args),
        _ => (None, args),
    }
}

fn einstellungen() -> i32 {
    let p = Einstellungen::vorgabepfad();
    match Einstellungen::lesen(&p) {
        Ok(e) => {
            println!("Datei: {}", p.display());
            println!("  modell.artefakt      {}", if e.modell.artefakt.is_empty() {
                "(nicht gesetzt)"
            } else {
                &e.modell.artefakt
            });
            println!("  modell.token         {}", e.modell.token);
            println!("  modell.denken        {}", e.modell.denken);
            println!("  agent.schritte       {}", e.agent.schritte);
            println!("  agent.bezeugtes      {}", e.agent.auch_bezeugtes);
            println!("  agent.wurzel         {}", e.agent.wurzel.as_deref().unwrap_or("(keine, ohne Dateiwerkzeuge)"));
            println!("  agent.schreiben      {}", e.agent.schreiben);
            // ⚑ Nicht nur, was gespeichert ist, sondern was **gilt**:
            // Eine Grenze ueber der Maschine hebt sie nicht an.
            println!(
                "  kap.kerne            {} (wirksam: {})",
                zeig(e.kapazitaet.kerne),
                {
                    kapazitaet_anwenden(&e);
                    integer_llm_runtime::kapazitaet::kerne()
                }
            );
            println!("  kap.beschleuniger    {}", e.kapazitaet.beschleuniger);
            println!("  kap.speicher         {}", zeig_gib(e.kapazitaet.speicher_gib));
            println!("  kap.platte           {}", zeig_gib(e.kapazitaet.platte_gib));
            0
        }
        Err(m) => {
            eprintln!("myl einstellungen: {m}");
            1
        }
    }
}

fn zeig_gib(v: Option<u32>) -> String {
    v.map(|x| format!("{x} GiB")).unwrap_or_else(|| "ohne Grenze".to_string())
}

fn zeig<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "ohne Grenze".to_string())
}

fn setzen(args: &[String]) -> i32 {
    let (Some(feld), Some(wert)) = (args.first(), args.get(1)) else {
        eprintln!("myl setzen: es fehlt Feld oder Wert");
        eprintln!("myl setzen: bekannte Felder:");
        for (name, art) in myl_client::einstellungen::FELDER {
            eprintln!("  {name:<20} {art:?}");
        }
        return 2;
    };
    let p = Einstellungen::vorgabepfad();
    let mut e = match Einstellungen::lesen(&p) {
        Ok(e) => e,
        Err(m) => {
            eprintln!("myl setzen: {m}");
            return 1;
        }
    };
    // ⚑ Die Kiste weiss, was `aus` heisst und welche Felder es gibt.
    // Zwei Stellen, die das wissen, laufen auseinander.
    if let Err(m) = e.setzen(feld, wert) {
        eprintln!("myl setzen: {m}");
        return 2;
    }
    match e.schreiben(&p) {
        Ok(()) => {
            println!("{feld} = {wert}");
            0
        }
        Err(m) => {
            eprintln!("myl setzen: {m}");
            1
        }
    }
}

fn zahl(args: &[String], name: &str) -> Option<usize> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1)?.parse().ok()
}

/// Ein Textwert hinter einem Schalter.
fn wert(args: &[String], name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1).cloned()
}

/// Die Agenteneinstellung fuer **diesen** Lauf.
///
/// # ⚑ Warum das ueberhaupt Schalter braucht
///
/// Eine Messung, die die Ablage des Nutzers umschreibt, ist keine
/// Messung, sondern ein Eingriff: Sie laesst den Rechner anders
/// zurueck, als sie ihn vorgefunden hat, und wer sie zweimal faehrt,
/// misst beim zweiten Mal etwas anderes.
///
/// ⚑ **Genannt schlaegt eingestellt**, wie bei `--token` und
/// `--denken`, und `--schreiben` gilt nur zusammen mit einer
/// genannten Wurzel: Wer eine fremde Einhaengung erbt, soll sie nicht
/// nebenbei beschreibbar machen.
/// Die Ansageform dieses Laufs.
///
/// ⚑ **Ein Schalter je Lauf und keine Einstellung.** Er dient dem
/// Vergleich in `BENCHMARKS/Agent/` und nicht dem taeglichen Betrieb;
/// eine dauerhafte Einstellung dafuer waere Ballast, den nachher
/// niemand wieder wegraeumt.
/// Welcher Werkzeugsatz fuer diesen Lauf.
///
/// ⚑ Wie `--deutsch` ein **Vergleichsschalter** und keine Einstellung:
/// `search_files` und `edit_file` schliessen echte Luecken, aber fuenf
/// Werkzeuge im Kontext machen die Auswahl fuer ein 4B-Modell schwerer
/// als drei. Welches ueberwiegt, sagt die Agentenprobe und nicht diese
/// Datei.
fn satz_fuer_diesen_lauf(args: &[String]) -> myl_client::werkzeuge::Werkzeugsatz {
    let voll = args
        .windows(2)
        .any(|p| p[0] == "--werkzeuge" && p[1] == "voll");
    if voll {
        myl_client::werkzeuge::Werkzeugsatz::Voll
    } else {
        myl_client::werkzeuge::Werkzeugsatz::Knapp
    }
}

fn form_fuer_diesen_lauf(args: &[String]) -> myl_local_agent::werkzeug::Ansageform {
    if args.iter().any(|a| a == "--deutsch") {
        myl_local_agent::werkzeug::Ansageform::Deutsch
    } else {
        myl_local_agent::werkzeug::Ansageform::Amtlich
    }
}

fn agent_fuer_diesen_lauf(
    e: &Einstellungen,
    args: &[String],
) -> myl_client::einstellungen::Agenteneinstellung {
    let mut a = e.agent.clone();
    if let Some(w) = wert(args, "--wurzel") {
        a.wurzel = Some(w);
        a.schreiben = args.iter().any(|x| x == "--schreiben");
    } else if args.iter().any(|x| x == "--schreiben") {
        a.schreiben = true;
    }
    a
}

fn frage(args: &[String]) -> i32 {
    let e = match Einstellungen::lesen(&Einstellungen::vorgabepfad()) {
        Ok(e) => e,
        Err(m) => {
            eprintln!("myl frage: {m}");
            return 1;
        }
    };
    kapazitaet_anwenden(&e);
    let (artefakt, rest) = artefakt_und_rest(args, &e);
    let Some(artefakt) = artefakt else {
        eprintln!(
            "myl frage: es fehlt das Artefaktverzeichnis, und in den \
             Einstellungen steht keines. `myl setzen modell.artefakt <pfad>`"
        );
        return 2;
    };
    let text = freitext(rest);
    if text.trim().is_empty() {
        eprintln!("myl frage: es fehlt der Text");
        return 2;
    }
    let mut m = match Oertlichesmodell::laden(&artefakt) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("myl frage: {e}");
            return 1;
        }
    };
    m.grenze = zahl(args, "--token").unwrap_or(e.modell.token);
    m.denken = e.modell.denken || args.iter().any(|a| a == "--denken");

    let anfang = std::time::Instant::now();
    match m.chat("lokal", &[Nachricht::nutzer(text)], Some(m.grenze as u32)) {
        Ok(a) => {
            println!("{}", a.text);
            // ⚑ Die Zahlen auf den Fehlerkanal, damit die Antwort
            // weiterverarbeitbar bleibt.
            eprintln!(
                "[myl] {} Token hinein, {} heraus, {:.1} s ({:.1} Token/s)",
                a.prompt_token,
                a.antwort_token,
                anfang.elapsed().as_secs_f64(),
                f64::from(a.antwort_token) / anfang.elapsed().as_secs_f64()
            );
            0
        }
        Err(e) => {
            eprintln!("myl frage: {e}");
            1
        }
    }
}

fn modell(args: &[String]) -> i32 {
    let Some(artefakt) = args.first() else {
        eprintln!("myl modell: es fehlt das Artefaktverzeichnis");
        return 2;
    };
    match Oertlichesmodell::laden(artefakt) {
        Ok(m) => {
            println!("Artefakt:  {artefakt}");
            println!("Vorlage:   {:?}", m.vorlage());
            // ⚑ Die Vorlage ist die Angabe, die am haeufigsten falsch
            // vermutet wird: Drei der vier Artefakte dieses Projekts
            // stammen aus Basisrepositorien und kennen keine
            // Rollenmarken.
            println!("Denkmodus: {}", if m.denken { "an" } else { "aus" });
            0
        }
        Err(e) => {
            eprintln!("myl modell: {e}");
            1
        }
    }
}


// --- ⚑ Die Agentenschleife, lokal -------------------------------------

/// Ein Werkzeug, das die Uhrzeit sagt.
///
/// ⚑ **Es steht hier als der einfachste ehrliche Fall.** Der Agent soll
/// beweisen, dass er ein Werkzeug **ruft**, und dafuer taugt eine
/// Auskunft, die das Modell selbst nicht haben kann. Ein Werkzeug, das
/// etwas ausrechnet, was das Modell auch raten koennte, belegt nichts.
struct Uhr;

impl myl_local_agent::ausfuehrung::Werkzeugausfuehrung for Uhr {
    fn name(&self) -> &str {
        "zeit"
    }
    fn ausfuehren(
        &self,
        _argumente: &serde_json::Value,
    ) -> Result<String, myl_local_agent::ausfuehrung::Werkzeugfehler> {
        let jetzt = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(format!("{jetzt} Sekunden seit 1970"))
    }
}

fn agent(args: &[String]) -> i32 {
    let e = match Einstellungen::lesen(&Einstellungen::vorgabepfad()) {
        Ok(e) => e,
        Err(m) => {
            eprintln!("myl agent: {m}");
            return 1;
        }
    };
    kapazitaet_anwenden(&e);
    let (artefakt, rest) = artefakt_und_rest(args, &e);
    let Some(artefakt) = artefakt else {
        eprintln!("myl agent: es fehlt das Artefaktverzeichnis");
        return 2;
    };
    let auftrag = freitext(rest);
    if auftrag.trim().is_empty() {
        eprintln!("myl agent: es fehlt der Auftrag");
        return 2;
    }
    let mut m = match Oertlichesmodell::laden(&artefakt) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("myl agent: {e}");
            return 1;
        }
    };
    m.grenze = zahl(args, "--token").unwrap_or(e.modell.token);
    m.denken = e.modell.denken || args.iter().any(|a| a == "--denken");

    // ⚑ Die Verdrahtung liegt in der Kiste und nicht hier, damit ein
    // Pruefstand sie **ohne geladenes Modell** fahren kann; siehe
    // `ruestung.rs`.
    let ruestung = match myl_client::ruestung::ruesten(
        &agent_fuer_diesen_lauf(&e, args),
        form_fuer_diesen_lauf(args),
        satz_fuer_diesen_lauf(args),
        vec![werkzeug_uhr()],
    ) {
        Ok(r) => r,
        Err(m) => {
            eprintln!("myl agent: {m}");
            return 1;
        }
    };
    if let Some(ein) = ruestung.einhaengung.as_ref() {
        eprintln!(
            "[myl] eingehaengt: {} ({})",
            ein.wurzel().display(),
            if ein.darf_schreiben() { "lesen und schreiben" } else { "nur lesen" }
        );
    }
    let bezeugtes = e.agent.auch_bezeugtes || args.iter().any(|a| a == "--bezeugtes");
    let schritte = zahl(args, "--schritte").unwrap_or(e.agent.schritte as usize);
    hinweis_betriebsart(bezeugtes, ruestung.kasten.angebote().len());
    let roh = args.iter().any(|a| a == "--roh");
    einen_auftrag(&m, &ruestung, schritte, bezeugtes, roh, &auftrag)
}

/// Eine Sitzung: **das Modell einmal laden, dann viele Auftraege.**
///
/// # ⚑ Warum das der Unterschied zwischen brauchbar und unbrauchbar ist
///
/// Bei `myl agent` kostet jeder Auftrag das Laden des Artefakts, und
/// bei einem Modell dieser Groesse ist das die teuerste Zeit im ganzen
/// Ablauf: Sie faellt vor jedem Wort an und haengt an der Plattengroesse
/// des Modells, nicht an der Laenge der Frage. Wer damit **arbeiten**
/// will, zahlt sie einmal.
///
/// ⚑ **Fortgesetzt wird das Modell, nicht das Gespraech**, siehe
/// [`einen_auftrag`]: Jeder Auftrag bleibt eine eigene Sitzung mit
/// eigenem Schrittbudget.
fn sitzung(args: &[String]) -> i32 {
    let e = match Einstellungen::lesen(&Einstellungen::vorgabepfad()) {
        Ok(e) => e,
        Err(m) => {
            eprintln!("myl sitzung: {m}");
            return 1;
        }
    };
    kapazitaet_anwenden(&e);
    let (artefakt, rest) = artefakt_und_rest(args, &e);
    let Some(artefakt) = artefakt else {
        eprintln!("myl sitzung: es fehlt das Artefaktverzeichnis");
        return 2;
    };
    let anfang = std::time::Instant::now();
    let mut m = match Oertlichesmodell::laden(&artefakt) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("myl sitzung: {f}");
            return 1;
        }
    };
    m.grenze = zahl(rest, "--token").unwrap_or(e.modell.token);
    m.denken = e.modell.denken || rest.iter().any(|a| a == "--denken");
    let geladen = anfang.elapsed();

    let ruestung = match myl_client::ruestung::ruesten(
        &agent_fuer_diesen_lauf(&e, rest),
        form_fuer_diesen_lauf(rest),
        satz_fuer_diesen_lauf(rest),
        vec![werkzeug_uhr()],
    ) {
        Ok(r) => r,
        Err(msg) => {
            eprintln!("myl sitzung: {msg}");
            return 1;
        }
    };
    let bezeugtes = e.agent.auch_bezeugtes || rest.iter().any(|a| a == "--bezeugtes");
    let schritte = zahl(rest, "--schritte").unwrap_or(e.agent.schritte as usize);
    let roh = rest.iter().any(|a| a == "--roh");

    println!("myl Sitzung");
    println!("  Modell:      {artefakt} ({:?}), in {:.1} s geladen", m.vorlage(), geladen.as_secs_f64());
    match ruestung.einhaengung.as_ref() {
        Some(ein) => println!(
            "  Eingehaengt: {} ({})",
            ein.wurzel().display(),
            if ein.darf_schreiben() { "lesen und schreiben" } else { "nur lesen" }
        ),
        None => println!("  Eingehaengt: nichts, also keine Dateiwerkzeuge"),
    }
    println!(
        "  Werkzeuge:   {} ({})",
        ruestung.kasten.angebote().len(),
        ruestung
            .kasten
            .angebote()
            .iter()
            .map(|w| w.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "  Betriebsart: {}",
        if bezeugtes { "Alles, bezeugte Werkzeuge laufen" } else { "NurVerankert, lokale Werkzeuge gesperrt" }
    );
    println!("  `:hilfe` zeigt die Befehle, `:ende` beendet.\n");

    let eingabe = std::io::stdin();
    loop {
        // ⚑ Der Zeilenanfang muss **vor** dem Lesen sichtbar sein,
        // sonst sitzt der Nutzer vor einem leeren Schirm und haelt die
        // Sitzung fuer haengend.
        print!("myl> ");
        if std::io::Write::flush(&mut std::io::stdout()).is_err() {
            return 1;
        }
        let mut zeile = String::new();
        match std::io::BufRead::read_line(&mut eingabe.lock(), &mut zeile) {
            // Dateiende: der uebliche Weg hinaus.
            Ok(0) => {
                println!();
                return 0;
            }
            Ok(_) => {}
            Err(f) => {
                eprintln!("myl sitzung: {f}");
                return 1;
            }
        }
        let auftrag = match deuten(&zeile) {
            Zeile::Leer => continue,
            Zeile::Ende => return 0,
            Zeile::Hilfe => {
                println!("  :werkzeuge  Was der Agent rufen kann");
                println!("  :ende       Sitzung beenden (auch Strg-D)");
                println!("  alles andere ist ein Auftrag.");
                continue;
            }
            Zeile::Werkzeuge => {
                for w in ruestung.kasten.angebote() {
                    println!("  {:<16} {}", w.name, w.beschreibung);
                }
                continue;
            }
            Zeile::Auftrag(a) => a.to_string(),
        };
        let anfang = std::time::Instant::now();
        einen_auftrag(&m, &ruestung, schritte, bezeugtes, roh, &auftrag);
        eprintln!("[myl] {:.1} s", anfang.elapsed().as_secs_f64());
    }
}

/// Was von einem Lauf auf dem Schirm landet.
///
/// # ⚑ Warum nicht einfach alle Nachrichten
///
/// Der rohe Verlauf enthaelt die **Systemnachricht mit den
/// Werkzeugschemata**, und die ist laenger als jede Antwort. Sie bei
/// jedem Auftrag mitzudrucken macht aus einer Sitzung eine Wand aus
/// JSON, in der die eigentliche Antwort untergeht.
///
/// ⚑ **Gezeigt wird der Weg, nicht das Protokoll:** je Werkzeugaufruf
/// eine Zeile, am Ende die Antwort. `--roh` gibt den vollen Verlauf,
/// denn wer einen Fehler sucht, braucht ihn.
fn zeigen(nachrichten: &[myl_local_agent::tuerklient::Nachricht], roh: bool) {
    if roh {
        for n in nachrichten {
            println!("[{}] {}", n.role, n.content);
        }
        return;
    }
    // Die letzte Antwort des Modells ohne Werkzeugaufruf ist das
    // Ergebnis; alles davor ist der Weg dorthin.
    let mut letzte: Option<&str> = None;
    for n in nachrichten {
        match n.role.as_str() {
            "assistant" => {
                let rufe = myl_local_agent::werkzeug::vorschlaege(&n.content);
                if rufe.is_empty() {
                    letzte = Some(n.content.trim());
                } else {
                    // ⚑ **Der Text neben dem Aufruf gehoert dazu.** Ein
                    // Modell schreibt vor dem Werkzeug oft hin, was es
                    // vorhat, und genau daran erkennt man einen falschen
                    // Plan, bevor das Werkzeug ihn ausfuehrt. Ihn
                    // wegzulassen zeigte nur, DASS etwas gerufen wurde.
                    let dazwischen = myl_client::lauf::ohne_aufrufe(&n.content);
                    if !dazwischen.is_empty() {
                        println!("  {dazwischen}");
                    }
                    for r in &rufe {
                        match r {
                            Ok(v) => println!("  → {} {}", v.name, myl_client::lauf::kurzform(&v.arguments)),
                            Err(u) => println!("  → unlesbarer Aufruf: {}", u.roh),
                        }
                    }
                }
            }
            "tool" => println!("  ← {}", myl_client::lauf::eine_zeile(&n.content, 100)),
            _ => {}
        }
    }
    match letzte {
        Some(t) if !t.is_empty() => println!("{t}"),
        // ⚑ **Kein stilles Nichts.** Ein Lauf ohne Schlussantwort ist
        // ein Befund: Das Modell ist in Werkzeugaufrufen steckengeblieben
        // oder an der Schrittgrenze gelandet.
        _ => println!("(keine Schlussantwort; `--roh` zeigt den vollen Verlauf)"),
    }
}




/// Setzt um, was der Nutzer an Kapazitaet freigegeben hat.
///
/// # ⚑ Warum eine Einstellung wirken muss oder verschwinden
///
/// `kap.kerne` liess sich bis zum 2026-09-08 setzen, anzeigen und
/// abspeichern, und **gelesen hat sie niemand**: Der Rechenpfad fragte
/// `available_parallelism` und nahm die ganze Maschine. Eine
/// Einstellung, die nichts bewirkt, ist schlimmer als keine, denn sie
/// sieht aus wie eine Zusage.
///
/// ⚑ **Sie aendert kein Ergebnis, nur die Laufzeit.** Der Beleg ist
/// die Pruefung `dieselbe_antwort_bei_jeder_kernzahl` in der Kernkiste.
fn kapazitaet_anwenden(e: &Einstellungen) {
    if let Some(n) = e.kapazitaet.kerne {
        integer_llm_runtime::kapazitaet::kerne_setzen(n);
    }
}

/// Was eine eingegebene Zeile bedeutet.
///
/// ⚑ **Getrennt von der Schleife, damit sie pruefbar ist.** Solange die
/// Deutung mitten in einer Ein- und Ausgabeschleife stand, war der
/// einzige Weg zu ihr eine Sitzung mit geladenem Modell, und deshalb
/// wurde sie nie geprueft.
#[derive(Debug, PartialEq, Eq)]
enum Zeile<'a> {
    Leer,
    Ende,
    Hilfe,
    Werkzeuge,
    Auftrag(&'a str),
}

/// ⚑ **Ein Doppelpunkt am Anfang ist ein Befehl, sonst nichts.** Ein
/// unbekannter Befehl geht deshalb NICHT als Auftrag durch: `:wekzeuge`
/// mit Tippfehler an ein Modell zu schicken, das damit nichts anfangen
/// kann, kostet einen Ladevorgang und ergibt Unsinn.
fn deuten(zeile: &str) -> Zeile<'_> {
    let z = zeile.trim();
    if z.is_empty() {
        return Zeile::Leer;
    }
    match z {
        ":ende" | ":beenden" | ":q" => Zeile::Ende,
        ":hilfe" | ":h" | ":?" => Zeile::Hilfe,
        ":werkzeuge" | ":w" => Zeile::Werkzeuge,
        andere if andere.starts_with(':') => Zeile::Hilfe,
        andere => Zeile::Auftrag(andere),
    }
}

/// Die Uhr als Paar aus Angebot und Ausfuehrung.
fn werkzeug_uhr() -> (
    myl_local_agent::werkzeug::Werkzeug,
    Box<dyn myl_local_agent::ausfuehrung::Werkzeugausfuehrung>,
) {
    (
        myl_local_agent::werkzeug::Werkzeug::ohne_parameter(
            "zeit",
            "Sagt die aktuelle Zeit in Sekunden seit 1970.",
        ),
        Box::new(Uhr),
    )
}

/// ⚑ **Derselbe Satz fuer den Menschen, den das Modell ohnehin
/// bekommt.** Ein lokales Werkzeug ist nicht nachrechenbar, also
/// bezeugt, also in der Vorgabebetriebsart gesperrt. Der Harness sagt
/// das dem Modell als Werkzeugantwort und laeuft weiter; der Nutzer
/// sieht sonst nur einen Agenten, der seine Werkzeuge nicht benutzt,
/// und haelt es fuer ein Modellproblem.
fn hinweis_betriebsart(bezeugtes: bool, wieviele: usize) {
    if !bezeugtes {
        eprintln!(
            "[myl] Betriebsart NurVerankert: die {wieviele} lokalen Werkzeuge sind \
             angemeldet, aber gesperrt, denn lokal heisst bezeugt und nicht \
             nachrechenbar. `--bezeugtes` laesst sie fuer diesen Lauf zu, \
             `myl setzen agent.bezeugtes an` dauerhaft."
        );
    }
}

/// Ein Auftrag, von der Vollmacht bis zur Antwort.
///
/// ⚑ **Jeder Auftrag ist eine eigene Sitzung**, auch innerhalb von
/// `myl sitzung`. Das ist eine Entscheidung und keine Bequemlichkeit:
/// Schrittbudget und Belegkette des Sitzungskontrakts gelten **je
/// Sitzung**, und eine fortlaufende Unterhaltung, die den Zaehler bei
/// jedem Auftrag zurueckstellte, machte aus einer Obergrenze eine
/// Empfehlung. Was ueber Auftraege hinweg traegt, sind die
/// **geschriebenen Dateien** und nicht das Gespraechsprotokoll.
fn einen_auftrag(
    m: &Oertlichesmodell,
    ruestung: &myl_client::ruestung::Ruestung,
    schritte: usize,
    bezeugtes: bool,
    roh: bool,
    auftrag: &str,
) -> i32 {
    let grenzen = myl_local_agent::vollmacht_grenzen::Sitzungsgrenzen::neu(
        myl_client::lauf::kontrakt_fuer(schritte),
        ruestung.kasten.angebote(),
    );
    let zuordnung = ruestung.zuordnung();

    let erg = myl_local_agent::schleife::Lauf {
        einhaengung: ruestung.einhaengung.as_ref().map(|e| e.marke()),
        klient: m,
        modell: "lokal",
        grenzen: &grenzen,
        // ⚑ Die enge Vorgabe gilt, bis der Nutzer sie aufhebt.
        betriebsart: if bezeugtes {
            myl_local_agent::betrieb::Betriebsart::Alles
        } else {
            myl_local_agent::betrieb::Betriebsart::NurVerankert
        },
        kasten: &ruestung.kasten,
        registratur: &ruestung.registratur,
        adressen: &zuordnung,
        anker: myl_types::hash::Hash::from_bytes([0u8; 32]),
        max_tokens: Some(m.grenze as u32),
        ansageform: Default::default(),
    }
    .fahren(auftrag);

    zeigen(&erg.nachrichten, roh);
    eprintln!("[myl] Ende: {:?}, {} Nachrichten", erg.ende, erg.nachrichten.len());
    match erg.ende {
        myl_local_agent::schleife::Ende::Fertig => 0,
        _ => 1,
    }
}

/// Die Auftraege eines nebenlaeufigen Laufs.
///
/// ⚑ **Ein Wort ist hier kein Auftrag, ein Argument ist einer.**
/// [`freitext`] fuegt alle Woerter zu **einem** Auftrag zusammen, und
/// genau das ist hier falsch: `myl auftraege "Lies A" "Lies B"` sind
/// zwei, nicht einer mit vier Woertern.
///
/// `--datei P` liest sie stattdessen zeilenweise; leere Zeilen und
/// solche mit `#` am Anfang fallen weg.
fn auftragsliste(args: &[String]) -> Result<Vec<String>, String> {
    if let Some(pfad) = wert(args, "--datei") {
        let inhalt = std::fs::read_to_string(&pfad).map_err(|e| format!("{pfad}: {e}"))?;
        return Ok(inhalt
            .lines()
            .map(str::trim)
            .filter(|z| !z.is_empty() && !z.starts_with('#'))
            .map(str::to_string)
            .collect());
    }
    let mut aus = Vec::new();
    let mut ueberspringen = false;
    for a in args {
        if ueberspringen {
            ueberspringen = false;
            continue;
        }
        if a.starts_with("--") {
            ueberspringen = MIT_WERT.contains(&a.as_str());
            continue;
        }
        aus.push(a.clone());
    }
    Ok(aus)
}

fn auftraege(args: &[String]) -> i32 {
    let e = match Einstellungen::lesen(&Einstellungen::vorgabepfad()) {
        Ok(e) => e,
        Err(m) => {
            eprintln!("myl auftraege: {m}");
            return 1;
        }
    };
    kapazitaet_anwenden(&e);
    let (artefakt, rest) = artefakt_und_rest(args, &e);
    let Some(artefakt) = artefakt else {
        eprintln!("myl auftraege: es fehlt das Artefaktverzeichnis");
        return 2;
    };
    let liste = match auftragsliste(rest) {
        Ok(l) => l,
        Err(m) => {
            eprintln!("myl auftraege: {m}");
            return 2;
        }
    };
    if liste.is_empty() {
        eprintln!("myl auftraege: es fehlen die Auftraege (je einer als Argument, oder --datei P)");
        return 2;
    }
    let mut m = match Oertlichesmodell::laden(&artefakt) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("myl auftraege: {f}");
            return 1;
        }
    };
    m.grenze = zahl(rest, "--token").unwrap_or(e.modell.token);
    m.denken = e.modell.denken || rest.iter().any(|a| a == "--denken");
    let ruestung = match myl_client::ruestung::ruesten(
        &agent_fuer_diesen_lauf(&e, rest),
        form_fuer_diesen_lauf(rest),
        satz_fuer_diesen_lauf(rest),
        vec![werkzeug_uhr()],
    ) {
        Ok(r) => r,
        Err(msg) => {
            eprintln!("myl auftraege: {msg}");
            return 1;
        }
    };
    let bezeugtes = e.agent.auch_bezeugtes || rest.iter().any(|a| a == "--bezeugtes");
    let roh = rest.iter().any(|a| a == "--roh");
    let schritte = zahl(rest, "--schritte").unwrap_or(e.agent.schritte as usize);
    hinweis_betriebsart(bezeugtes, ruestung.kasten.angebote().len());
    viele_auftraege(&m, &ruestung, schritte, bezeugtes, roh, &liste)
}


/// Mehrere Auftraege nebenlaeufig, ueber **ein** geladenes Modell.
///
/// # ⚑ Warum ein Modell fuer alle reicht
///
/// [`Modellweg::chat`] nimmt `&self`, die Gewichte liegen hinter einem
/// `Arc`, und der Rechenpfad haelt keinen veraenderlichen Zustand;
/// `das_modell_laesst_sich_ueber_faeden_teilen` haelt das als
/// Uebersetzungsfehler fest. Ein 4B-Artefakt je Unteragent waere kein
/// Nebenlaeufigkeitsentwurf, sondern eine Speichersperre.
///
/// # ⚑ Und warum das Kernbudget geteilt werden MUSS
///
/// Der Rechenpfad verteilt eine Matrixmultiplikation selbst ueber
/// mehrere Faeden, bis zur Kerngrenze. Laufen `k` Unteragenten, will
/// jeder von ihnen bis zu `n` Faeden, und die Maschine bekommt `k · n`.
/// **Das ist nicht schneller, sondern langsamer:** Die Faeden nehmen
/// sich gegenseitig die Kerne weg, und dazu kommt der Startaufwand je
/// Matrix.
///
/// Deshalb wird die Grenze **vor** dem Start auf `n / k` gesetzt,
/// mindestens eins. ⛑ Sie gilt fuer den ganzen Prozess und wird
/// danach **nicht** zurueckgesetzt: Wer sie zuruecksetzte, waehrend
/// noch ein Faden rechnet, aenderte dessen Aufteilung mitten im Lauf.
fn viele_auftraege(
    m: &Oertlichesmodell,
    ruestung: &myl_client::ruestung::Ruestung,
    schritte: usize,
    bezeugtes: bool,
    roh: bool,
    auftraege: &[String],
) -> i32 {
    let kerne = integer_llm_runtime::kapazitaet::kerne();
    let je_agent = (kerne / auftraege.len().max(1)).max(1);
    integer_llm_runtime::kapazitaet::kerne_setzen(je_agent);
    eprintln!(
        "[myl] {} Auftraege nebenlaeufig, {kerne} Kerne, {je_agent} je Auftrag",
        auftraege.len()
    );

    // ⚑ **Gesammelt statt gedruckt.** Zwei Faeden, die gleichzeitig
    // schreiben, ergeben eine Ausgabe, in der keine Antwort mehr einem
    // Auftrag zuzuordnen ist. Gedruckt wird nach dem Zusammenlaufen,
    // in der Reihenfolge der Auftraege.
    let ergebnisse: Vec<(usize, myl_client::lauf::Ausgang)> = std::thread::scope(|s| {
        let griffe: Vec<_> = auftraege
            .iter()
            .enumerate()
            .map(|(i, a)| s.spawn(move || (i, myl_client::lauf::fahren(m, ruestung, schritte, bezeugtes, m.grenze as u32, a))))
            .collect();
        griffe.into_iter().filter_map(|g| g.join().ok()).collect()
    });

    let mut sortiert = ergebnisse;
    sortiert.sort_by_key(|(i, _)| *i);
    let mut fehler = 0;
    for (i, aus) in &sortiert {
        println!("\n=== Auftrag {} : {}", i + 1, auftraege[*i]);
        zeigen(&aus.nachrichten, roh);
        eprintln!("[myl] Auftrag {}: {:?}, {:.1} s", i + 1, aus.ende, aus.sekunden);
        if !matches!(aus.ende, myl_local_agent::schleife::Ende::Fertig) {
            fehler += 1;
        }
    }
    if fehler > 0 {
        eprintln!("[myl] {fehler} von {} Auftraegen nicht fertig", sortiert.len());
        return 1;
    }
    0
}



#[cfg(test)]
mod schalter {
    use super::*;

    fn worte(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    fn grund() -> Einstellungen {
        Einstellungen {
            agent: myl_client::einstellungen::Agenteneinstellung {
                schritte: 6,
                auch_bezeugtes: false,
                wurzel: Some("/vorher".into()),
                schreiben: true,
            },
            ..Einstellungen::default()
        }
    }

    #[test]
    fn ohne_schalter_gilt_die_ablage() {
        let a = agent_fuer_diesen_lauf(&grund(), &worte(&["irgendein Auftrag"]));
        assert_eq!(a.wurzel.as_deref(), Some("/vorher"));
        assert!(a.schreiben);
    }

    /// ⚑ **Eine genannte Wurzel bringt ihre Schreibrechte selbst mit
    /// oder gar nicht.** Sonst erbte eine fremde Einhaengung nebenbei
    /// die Erlaubnis, die fuer eine andere gedacht war.
    #[test]
    fn eine_genannte_wurzel_erbt_das_schreiben_nicht() {
        let a = agent_fuer_diesen_lauf(&grund(), &worte(&["--wurzel", "/neu"]));
        assert_eq!(a.wurzel.as_deref(), Some("/neu"));
        assert!(!a.schreiben, "die genannte Wurzel hat das Schreiben geerbt");
    }

    #[test]
    fn genannte_wurzel_mit_genanntem_schreiben() {
        let a = agent_fuer_diesen_lauf(&grund(), &worte(&["--wurzel", "/neu", "--schreiben"]));
        assert_eq!(a.wurzel.as_deref(), Some("/neu"));
        assert!(a.schreiben);
    }

    /// Ohne genannte Wurzel darf `--schreiben` die eingestellte
    /// freischalten: Sie ist die, die der Nutzer selbst gewaehlt hat.
    #[test]
    fn schreiben_allein_gilt_der_eingestellten_wurzel() {
        let mut e = grund();
        e.agent.schreiben = false;
        let a = agent_fuer_diesen_lauf(&e, &worte(&["--schreiben"]));
        assert_eq!(a.wurzel.as_deref(), Some("/vorher"));
        assert!(a.schreiben);
    }

    /// ⚑ **Jeder Schalter, hinter dem ein Wert steht, muss in
    /// `MIT_WERT` stehen**, sonst haelt `freitext` den Wert fuer einen
    /// Teil des Auftrags. Geprueft wird gegen die Hilfe, denn dort
    /// steht die Liste fuer den Menschen.
    #[test]
    fn kein_wertschalter_fehlt() {
        // In der Hilfe steht ein Schalter mit Wert als `--name X`.
        let mut aus_der_hilfe: Vec<String> = Vec::new();
        for zeile in HILFE.lines() {
            let z = zeile.trim();
            let mut teile = z.split_whitespace();
            let (Some(a), Some(b)) = (teile.next(), teile.next()) else { continue };
            if a.starts_with("--") && !b.starts_with("--") && b.len() <= 2 {
                aus_der_hilfe.push(a.to_string());
            }
        }
        aus_der_hilfe.sort();
        aus_der_hilfe.dedup();
        assert!(!aus_der_hilfe.is_empty(), "die Hilfe nennt keinen Schalter mit Wert");
        for schalter in &aus_der_hilfe {
            assert!(
                MIT_WERT.contains(&schalter.as_str()),
                "{schalter} nimmt laut Hilfe einen Wert, steht aber nicht in MIT_WERT"
            );
        }
        // ⚑ Und die Gegenrichtung: ein Eintrag in `MIT_WERT`, den die
        // Hilfe nicht nennt, ist ein Schalter, den niemand findet.
        let mut soll: Vec<&str> = MIT_WERT.to_vec();
        soll.sort();
        assert_eq!(
            aus_der_hilfe, soll,
            "Hilfe und MIT_WERT nennen nicht dieselben Schalter"
        );
    }

    /// ⛑ Der Fall, der den Eintrag noetig machte.
    #[test]
    fn ein_pfad_wandert_nicht_in_den_auftrag() {
        let a = worte(&["Lies die Datei.", "--wurzel", "/tmp/x", "--bezeugtes"]);
        assert_eq!(freitext(&a), "Lies die Datei.");
    }

    #[test]
    fn ein_schalter_ohne_wert_aendert_nichts() {
        let a = agent_fuer_diesen_lauf(&grund(), &worte(&["--wurzel"]));
        assert_eq!(a.wurzel.as_deref(), Some("/vorher"));
    }
}

#[cfg(test)]
mod auftragsliste_probe {
    use super::*;

    fn worte(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    /// ⚑ **Ein Argument ist ein Auftrag, nicht ein Wort.**
    /// `freitext` fuegt alles zu einem zusammen, und das waere hier
    /// genau falsch.
    #[test]
    fn jedes_argument_ist_ein_eigener_auftrag() {
        let a = worte(&["Lies die Datei A.", "Schreibe nach B."]);
        assert_eq!(
            auftragsliste(&a).expect("Liste"),
            vec!["Lies die Datei A.".to_string(), "Schreibe nach B.".to_string()]
        );
    }

    #[test]
    fn schalter_und_ihre_werte_fallen_heraus() {
        let a = worte(&["Auftrag A", "--schritte", "8", "--bezeugtes", "Auftrag B", "--roh"]);
        assert_eq!(
            auftragsliste(&a).expect("Liste"),
            vec!["Auftrag A".to_string(), "Auftrag B".to_string()]
        );
    }

    #[test]
    fn ohne_auftrag_bleibt_die_liste_leer() {
        assert!(auftragsliste(&worte(&["--roh"])).expect("Liste").is_empty());
    }

    /// Aus einer Datei, mit Kommentaren und Leerzeilen.
    #[test]
    fn eine_datei_wird_zeilenweise_gelesen() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let p = d.path().join("auftraege.txt");
        std::fs::write(&p, "# ein Kommentar\nErster Auftrag\n\n  Zweiter Auftrag  \n")
            .expect("schreiben");
        let a = worte(&["--datei", p.to_str().expect("Pfad")]);
        assert_eq!(
            auftragsliste(&a).expect("Liste"),
            vec!["Erster Auftrag".to_string(), "Zweiter Auftrag".to_string()]
        );
    }

    /// ⛑ Eine fehlende Datei ist ein Fehler und keine leere Liste: Ein
    /// Lauf ohne Auftraege saehe aus wie ein Lauf, in dem nichts zu tun
    /// war.
    #[test]
    fn eine_fehlende_datei_meldet_sich() {
        assert!(auftragsliste(&worte(&["--datei", "/gibtesnicht/x.txt"])).is_err());
    }
}

#[cfg(test)]
mod sitzungsdeutung {
    use super::*;

    #[test]
    fn leere_zeilen_sind_kein_auftrag() {
        assert_eq!(deuten(""), Zeile::Leer);
        assert_eq!(deuten("   \n"), Zeile::Leer);
    }

    #[test]
    fn die_befehle_werden_erkannt() {
        assert_eq!(deuten(":ende"), Zeile::Ende);
        assert_eq!(deuten("  :q  "), Zeile::Ende);
        assert_eq!(deuten(":hilfe"), Zeile::Hilfe);
        assert_eq!(deuten(":werkzeuge"), Zeile::Werkzeuge);
    }

    /// ⚑ Ein vertippter Befehl darf nicht als Auftrag an das Modell
    /// gehen: Das kostet einen Ladevorgang und ergibt Unsinn.
    #[test]
    fn ein_vertippter_befehl_wird_kein_auftrag() {
        assert_eq!(deuten(":wekzeuge"), Zeile::Hilfe);
        assert_eq!(deuten(":"), Zeile::Hilfe);
    }

    #[test]
    fn ein_auftrag_bleibt_ein_auftrag() {
        assert_eq!(deuten("  Lies die Datei.  "), Zeile::Auftrag("Lies die Datei."));
        assert_eq!(deuten("Merke: das Wort"), Zeile::Auftrag("Merke: das Wort"));
    }
}
