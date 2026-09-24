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

  myl ort                         Wo dieses Repositorium liegt
  myl frage <artefakt> <text>     Eine Frage an das lokale Modell
  myl modell <artefakt>           Was in einem Artefakt steht
  myl agent [artefakt] <auftrag>  Die Agentenschleife, lokal
  myl sitzung [artefakt]          Viele Auftraege, Modell einmal geladen
  myl auftraege [artefakt] A B    Mehrere Auftraege NEBENLAEUFIG
  myl skills                      Wissensmappen: wo sie liegen, was da ist
  myl anhaenge [--aufraeumen]     Angehaengte Dateien: was liegt, und weg damit
  myl sinne [datei|--sprich TEXT] Sehen, Hoeren, Sprechen: was geht, und eine Probe
  myl verlauf [<von> <bis>]       Der Mitschnitt: Uebersicht oder Zeilen
  myl einstellungen               Zeigt die Einstellungen
  myl setzen <feld> <wert>        Aendert eine Einstellung

⚑ Steht in den Einstellungen ein Artefakt, darf es weggelassen werden.

Felder fuer `setzen`:
  modell.artefakt   modell.token      modell.denken
  agent.schritte    agent.wurzel      agent.schreiben
  agent.kistenordner
  kap.kerne         kap.beschleuniger
  kap.speicher      kap.platte

Schalter fuer `frage`:
  --token N       Hoechstzahl erzeugter Token (Vorgabe 256)
  --denken        Denkmodus an (Vorgabe aus, siehe unten)

Schalter fuer `agent`, `sitzung` und `auftraege`, zusaetzlich zu denen
von `frage`:
  --schritte N    Hoechstzahl der Schritte
  --nur-verankert Sperrt die Dateiwerkzeuge fuer diesen Lauf, s.u.
  --chat          Nur `agent`: der Zuschnitt des Gespraechsfensters,
                  also Werkzeuge nur auf dem Anhangordner, dazu die
                  Web-Recherche, falls `agent.web_recherche` an ist
  --wurzel P      Haengt P ein, nur fuer diesen Lauf
  --schreiben     Erlaubt Schreiben, nur fuer diesen Lauf
  --roh           Der volle Nachrichtenverlauf statt der Kurzform
  --deutsch       Werkzeuge deutsch ansagen (Vergleichsschalter, s.u.)
  --werkzeuge S   `Base` (Vorgabe) oder `Advanced`, s.u.
  --datei P       Nur `auftraege`: je Zeile ein Auftrag

⚑ Bei `auftraege` teilen sich alle Unteragenten EIN geladenes Modell,
und das Kernbudget aus `kap.kerne` wird durch ihre Zahl geteilt: Sonst
wollte jeder alle Kerne, und sie naehmen sie sich gegenseitig weg.

⚑ **Die Werkzeugkiste ist ein Ordner.** `agent.kistenordner` sagt
welcher; ohne Angabe der mitgelieferte `Base`-Ordner unter
`CLIENT/werkzeugkisten`. Sein **Name** ist der Name der Kiste und sagt
zugleich, welche eingebauten Werkzeuge dazukommen: `Base` die fuenf
Dateiwerkzeuge, `Advanced` zusaetzlich `run_command` und die drei
Werkzeuge fuer den Mitschnitt, ein anderer Name
`Base`. Bis zum 2026-09-15 stand daneben eine eigene Auswahl; zwei
Angaben fuer dieselbe Sache laufen auseinander.

⛔️ `run_command` fuehrt einen Shell-Befehl aus und haelt die
Einhaengegrenze **nicht** ein, die jedes Dateiwerkzeug einhaelt. Wer
einen Ordner `Advanced` nennt, bekommt es; das ist eine Entscheidung und
keine Panne.

⚑ Was als Manifest im Ordner liegt (JSON mit name, beschreibung,
parameter, befehl), bekommt das Modell zusaetzlich angesagt, ohne
Neubau. Ein solches Werkzeug laeuft ueber die Shell und braucht deshalb
die Schreiberlaubnis.

⚑ `--werkzeuge` ueberstimmt die eingebaute Auswahl fuer einen einzelnen
Lauf, `Base` oder `Advanced`.

⚑ `--deutsch` ist ein Vergleichsschalter und keine Einstellung. Die
Werkzeuge werden dem Modell sonst in genau der Form angesagt, auf die
es geschliffen wurde: englische Namen und der Wortlaut aus seiner
eigenen Vorlage. Die deutsche Fassung war bis zum 2026-09-09 die
einzige und steht nur noch da, damit die Agentenprobe beide messen
kann. Sie verschwindet, sobald der Vergleich gefallen ist.

⚑ Die Dateiwerkzeuge gibt es erst mit `agent.wurzel`, und sie sind
lokal, also bezeugt und nicht nachrechenbar. **Das eingehaengte
Verzeichnis ist die Zustimmung**, und ob geschrieben werden darf, sagt
`agent.schreiben`; welche Werkzeuge es ueberhaupt gibt, sagt die
Werkzeugkiste. `--nur-verankert` sperrt sie fuer einen einzelnen Lauf,
etwa zum Vergleichen.

📌 Bis zum 2026-09-11 war es umgekehrt: Ein Schalter `agent.bezeugtes`
musste erst gesetzt werden, sonst standen die Dateiwerkzeuge zwar in
der Ansage und liefen nicht. **Zwei Erlaubnisse fuer dieselbe Sache
sind eine zu viel**, und die zweite steht immer da, wenn jemand den
Fehler woanders sucht.

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
        Some("ort") => ort(),
        Some("einstellungen") => einstellungen(),
        Some("verlauf") => verlauf(&args[2..]),
        Some("skills") => skills(&args[2..]),
        Some("anhaenge") => anhaenge(&args[2..]),
        Some("sinne") => sinne(&args[2..]),
        Some("setzen") => setzen(&args[2..]),
        // Die Hilfe ist hier eine Antwort und kein Fehler: Sie geht nach
        // stdout und gibt null zurueck.
        None | Some("--hilfe") | Some("-h") | Some("--help") | Some("hilfe") => {
            print!("{HILFE}");
            0
        }
        // 📌 **Ein Tippfehler war bis hierher ein Erfolg.** Jeder
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

/// 📌 **Der Freitext ohne Schalter UND ohne deren Werte.**
///
/// Die erste Fassung filterte nur Argumente, die mit `--` beginnen.
/// Damit landete `--token 128` als „128" im Auftrag, und das Modell
/// bekam eine Frage mit angehaengten Zahlen. Ein Filter, der die Haelfte
/// eines Schalters stehen laesst, ist schlimmer als keiner: Er sieht
/// aus, als taete er etwas.
/// Die Schalter, hinter denen ein **Wert** steht.
///
/// 📌 **Diese Liste stand bis zum 2026-09-08 nur in `freitext`**, und
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

/// Wie breit die Namensspalte der Einstellungsliste sein muss.
///
/// 📌 Sie stand auf 21, und `agent.blick_bildschirm` ist 22 Zeichen
/// lang: Der Wert klebte am Namen. ⚑ **Eine Breite, die aus der Liste
/// selbst kommt**, geht bei jedem neuen Feld von allein mit; eine
/// festgeschriebene Zahl ist dieselbe Angabe an zwei Orten.
fn spaltenbreite() -> usize {
    myl_client::einstellungen::FELDER
        .iter()
        .map(|f| f.name.chars().count())
        .max()
        .unwrap_or(20)
        + 1
}

/// Die Ruestung dieses Laufs: Arbeitsordner oder Chatzuschnitt.
///
/// ⚑ **`--chat` fährt genau das, was das Fenster im Gespräch fährt:**
/// die fünf Dateiwerkzeuge auf dem Anhangordner statt auf dem
/// Arbeitsordner, kein Manifest, kein `run_command`, und dazu die
/// Web-Recherche, falls sie eingeschaltet ist. 📌 **Ohne diesen
/// Schalter liesse sich der Chatzuschnitt nur durch das Fenster
/// prüfen**, also nur von Hand und nur auf einem Rechner mit
/// Oberfläche.
fn ruestung_fuer_diesen_lauf(
    e: &Einstellungen,
    args: &[String],
    auftrag: &str,
) -> Result<myl_client::ruestung::Ruestung, String> {
    if !args.iter().any(|a| a == "--chat") {
        return myl_client::ruestung::ruesten(
            &agent_fuer_diesen_lauf(e, args),
            form_fuer_diesen_lauf(args),
            satz_fuer_diesen_lauf(e, args),
            vec![werkzeug_uhr()],
        );
    }
    let agent = agent_fuer_diesen_lauf(e, args);
    let wurzel = agent
        .wurzel
        .clone()
        .or_else(Einstellungen::standard_wurzel)
        .ok_or_else(|| "fuer --chat fehlt ein Arbeitsordner, in dem die Anhaenge liegen".to_string())?;
    let anhangordner = std::path::Path::new(&wurzel).join(myl_client::anhang::unterordner());
    std::fs::create_dir_all(&anhangordner)
        .map_err(|f| format!("der Anhangordner liess sich nicht anlegen: {f}"))?;
    // ⛔️ **Der Auftrag ist die Saat des Zielkreises**, nichts sonst:
    //    Genau wie im Fenster zaehlt nur, was der Mensch geschrieben
    //    hat.
    let netz = (e.agent.web_recherche && myl_client::netzwerkzeuge::curl_vorhanden())
        .then_some(auftrag);
    myl_client::ruestung::ruesten_fuer_anhaenge(
        &anhangordner,
        form_fuer_diesen_lauf(args),
        netz,
        vec![werkzeug_uhr()],
        None,
    )
}

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
        // 📌 **Gegen die Wurzel aufgeloest, seit dem 2026-09-10.** In
        // den Einstellungen steht `INTEGER_LLM/artifacts/myelith-4b`,
        // und das ist relativ. Aus einem anderen Arbeitsverzeichnis
        // heraus meldete `myl` deshalb „es fehlt das
        // Artefaktverzeichnis", obwohl es dalag; die Oberflaeche loeste
        // denselben Pfad auf, das Bedieninstrument nicht. **Zwei
        // Programme desselben Klienten, zwei Antworten auf dieselbe
        // Frage.**
        _ if !e.modell.artefakt.is_empty() => {
            (Some(myl_client::ort::absolut(&e.modell.artefakt)), args)
        }
        _ => (None, args),
    }
}

/// **Wo dieses Repositorium liegt, und woher wir das wissen.**
///
/// ⚑ **Es gibt ihn, damit das Startskript nicht raten muss.** Wo die
/// Ablage liegt, entscheidet `myl-client` je nach Betriebssystem; ein
/// Shell-Skript, das denselben Pfad noch einmal zusammensetzt, waere
/// die zweite Stelle, an der diese Entscheidung faellt.
///
/// ⚑ **Und der Aufruf merkt sich den Ort nebenbei.** `ort::wurzel`
/// schreibt den Zettel, sobald sie faendig wird; ein Aufruf aus dem
/// Klon heraus richtet damit auch das installierte Programm wieder
/// aus, das von sich aus nichts faende.
fn ort() -> i32 {
    match myl_client::ort::wurzel() {
        Some(w) => {
            println!("{}", w.display());
            0
        }
        None => {
            eprintln!(
                "myl ort: kein Klon gefunden.\n\
                 Gesucht wurde ab dem Arbeitsverzeichnis und ab diesem Programm \
                 nach `{}`.",
                myl_client::ort::MARKE
            );
            1
        }
    }
}

fn einstellungen() -> i32 {
    let p = Einstellungen::vorgabepfad();
    match Einstellungen::lesen(&p) {
        Ok(e) => {
            println!("Datei: {}", p.display());
            // ⚑ Nicht nur, was gespeichert ist, sondern was **gilt**:
            // Eine Grenze ueber der Maschine hebt sie nicht an.
            kapazitaet_anwenden(&e);
            let kerne = integer_llm_runtime::kapazitaet::kerne();
            for zeile in uebersicht(&e, kerne) {
                println!("{zeile}");
            }
            // ⚑ **Die Rechenwerke kommen aus dem Scan und nicht aus der
            // Ablage** (Auftrag des Projektinhabers, 2026-09-16). Wer
            // nichts eingestellt hat, hat alles freigegeben, und dann
            // stuende hier nichts, obwohl eine GPU da ist. **Der Scan
            // kostet einen Unterprozess**, und dieser Befehl ist eine
            // Auskunft an einen Menschen, der darauf wartet.
            for zeile in rechenwerkzeilen(&e) {
                println!("{zeile}");
            }
            0
        }
        Err(m) => {
            eprintln!("myl einstellungen: {m}");
            1
        }
    }
}

/// Eine Zeile je Feld, in der Reihenfolge des Katalogs.
///
/// # 📌 Fund 312, und es ist dieselbe Klasse wie Fund 280
///
/// **Diese Liste war von Hand gefuehrt und kannte zehn Felder, waehrend
/// der Katalog dreizehn fuehrte.** `oberflaeche.sprache`,
/// `agent.werkzeuge` und `ausgabe.ordner` liessen sich setzen, und
/// danach standen sie nirgends: `myl setzen` bestaetigte den neuen
/// Wert, `myl einstellungen` schwieg darueber. Fund 280 war derselbe
/// Fehler im Fenster, nur mit `undefined` statt mit Schweigen.
///
/// ⚑ **Jetzt kommt die Liste aus dem Katalog und die Werte aus
/// [`Einstellungen::wert`]**, also aus denselben zwei Quellen wie im
/// Fenster. **Eine Anzeige, die neben ihrer Quelle gefuehrt wird, ist
/// keine Anzeige, sondern eine zweite Behauptung.**
///
/// `kerne` ist die **wirksame** Kernzahl, gemessen vom Aufrufer.
fn uebersicht(e: &Einstellungen, kerne: usize) -> Vec<String> {
    use myl_client::einstellungen::{Feldwert, FELDER};

    let mut zeilen = Vec::new();
    for f in FELDER.iter() {
        let wert = match e.wert(f.name) {
            Ok(w) => w,
            // Ein Feld im Katalog, das die Einstellungen nicht kennen,
            // ist ein Fund und kein Grund, die Zeile wegzulassen.
            Err(m) => {
                zeilen.push(format!("  {:<breite$}({m})", f.name, breite = spaltenbreite()));
                continue;
            }
        };
        let text = match wert {
            Feldwert::Zahl(n) => format!("{n}{}", einheit(f)),
            Feldwert::Schalter(b) => b.to_string(),
            Feldwert::Text(t) if t.is_empty() => ohne_wert(f).to_string(),
            Feldwert::Text(t) => t,
            Feldwert::Leer => ohne_wert(f).to_string(),
        };
        let text = if f.name == "kap.kerne" { format!("{text} (wirksam: {kerne})") } else { text };
        zeilen.push(format!("  {:<breite$}{text}", f.name, breite = spaltenbreite()));
    }

    zeilen
}

/// Eine Zeile je **gefundenem** Rechenwerk, mit Rechenweg und Anteil.
///
/// # ⚑ Warum sie nicht in [`uebersicht`] steht
///
/// Jene Funktion rechnet aus der Ablage und sonst nichts, und deshalb
/// laesst sie sich pruefen. **Diese hier fragt die Maschine**, mit einem
/// Unterprozess je Aufruf; zusammengelegt haette jede Pruefung der
/// Uebersicht eine Grafikkarte gebraucht.
///
/// 📌 **Bis zum 2026-09-16 stand hier nur, was in der Ablage steht**,
/// und seit „kein Eintrag" ganz freigegeben heisst, waere das auf einer
/// Maschine mit GPU eine leere Liste gewesen. **Eine Anzeige, die den
/// Normalfall verschweigt, zeigt nur die Ausnahme.**
fn rechenwerkzeilen(e: &Einstellungen) -> Vec<String> {
    let h = myl_client::hardware::Hardware {
        kerne: 0,
        speicher_bytes: None,
        platte: None,
        rechenwerke: myl_client::hardware::Hardware::rechenwerke(),
    };
    let werke: Vec<_> = h
        .regler(e)
        .into_iter()
        .filter(|r| r.name.starts_with(myl_client::einstellungen::RECHENWERK_PRAEFIX))
        .collect();
    // ⚑ **Die Spalte richtet sich nach dem laengsten Namen**, nicht nach
    // einer getippten Einundzwanzig. Eine Geraetekennung ist so lang wie
    // der Geraetename, und bei `kap.rechenwerk.apple-m5-pro` klebte der
    // Wert am Namen.
    let breite = werke.iter().map(|r| r.name.chars().count()).max().unwrap_or(0).max(19) + 2;
    werke
        .into_iter()
        .map(|r| {
            let wert = match r.wert {
                Some(0) => r.links.clone().unwrap_or_else(|| "0 %".to_string()),
                Some(n) => format!("{n} %"),
                None => r.rechts.clone(),
            };
            // ⚑ **Ein gesperrtes Werk sagt es in derselben Zeile.**
            // Sonst laese jemand einen Anteil und erwartete Rechenzeit,
            // die nirgends entsteht.
            let zusatz = if r.sperrgrund.is_some() { "  (rechnet hier nicht)" } else { "" };
            format!("  {:<breite$}{:<14}{}{zusatz}", r.name, wert, r.titel)
        })
        .collect()
}

/// Die Einheit hinter einer Zahl, abgeleitet aus der Beschriftung.
///
/// ⚑ **Der Katalog fuehrt keine Einheit als eigenes Merkmal**, er
/// schreibt sie in die Beschriftung („Arbeitsspeicher in GiB"). Von
/// dort wird sie geholt, statt hier eine zweite Liste zu fuehren: Ein
/// neues Feld in Gibibyte bekommt sie damit von selbst.
fn einheit(f: &myl_client::einstellungen::Feld) -> &'static str {
    if f.titel.ends_with(" in GiB") {
        " GiB"
    } else {
        ""
    }
}

/// Was dasteht, wenn nichts dasteht.
///
/// ⚑ **Eine Grenze, die es nicht gibt, ist keine Null**, und ein
/// Arbeitsordner, den es nicht gibt, ist mehr als eine Luecke.
///
/// # 📌 Und beim Arbeitsordner steht hier nicht mehr „keine" (2026-09-15)
///
/// Bis zum 2026-09-15 stand an dieser Stelle fest „(keine, ohne
/// Dateiwerkzeuge)". Seit es eine **Vorgabe** gibt (`WORK_DIR`), war das
/// eine Falschauskunft: Der Agent bekam einen Ordner, und die Uebersicht
/// sagte, er bekaeme keinen. **Eine Uebersicht, die den Wert nicht
/// nachrechnet, den sie anzeigt, zeigt den Wert von gestern.** Sie fragt
/// jetzt dieselbe Funktion, die auch die Ruestung fragt.
fn ohne_wert(f: &myl_client::einstellungen::Feld) -> String {
    use myl_client::einstellungen::Feldart;
    if f.name == "agent.wurzel" {
        return match myl_client::Einstellungen::standard_wurzel() {
            Some(p) => format!("(Vorgabe: {p})"),
            None => "(keine, ohne Dateiwerkzeuge)".to_string(),
        };
    }
    match f.art {
        Feldart::Grenze => "ohne Grenze".to_string(),
        _ => "(nicht gesetzt)".to_string(),
    }
}

/// **Die Wissensmappen fuer einen Menschen**: wo sie liegen, was da ist.
///
/// # ⚑ Warum es diesen Befehl gibt
///
/// Der allgemeine Ordner liegt neben den Einstellungen, und dieser Pfad
/// haengt am Betriebssystem. **Ein Ordner, den der Nutzer fuellen soll,
/// dessen Ort er aber raten muss, wird nicht gefuellt.** Der Befehl
/// nennt beide Orte und legt den allgemeinen auf Wunsch an.
/// **`myl sinne`: was dieser Rechner sehen, hoeren und sprechen kann.**
///
/// ⚑ **Und mit einer Datei ist es eine Probe.** Ein Stand, der sagt
/// „alles da", und ein Werkzeug, das dann doch nicht laeuft, waeren zwei
/// Auskuenfte; hier ist es dieselbe. **Der kuerzeste Weg von der
/// Behauptung zum Beleg.**
fn sinne(args: &[String]) -> i32 {
    let s = myl_senses::Sinne::finden();
    // ⚑ **Die Orte werden aufgezaehlt und nicht zusammengefasst** (seit
    // dem Umzug am 2026-09-21). Hier stand eine Zeile mit der Heimat,
    // und die trug seither nur noch den `bin`-Ordner und die
    // Stimmprobe: 📌 **Eine Auskunft, die nach dem Umzug noch dasteht,
    // nennt den falschen Ort und sieht aus wie vorher.**
    let orte = myl_senses::laufwerk::gewichtsorte();
    println!("Gewichte gesucht in (erster Treffer gewinnt):");
    for o in &orte {
        println!("  {}", o.display());
    }
    println!("Betrieb in {}", myl_senses::laufwerk::heimat().display());
    let zeile = |was: &str, stand: Result<String, &myl_senses::Mangel>| match stand {
        Ok(gut) => println!("  {was:<10} ✓ {gut}"),
        Err(m) => {
            println!("  {was:<10} ✗");
            for z in m.bericht().lines().skip(1) {
                println!("      {z}");
            }
        }
    };
    zeile(
        "Sehen",
        s.sehen.as_ref().map(|z| {
            let stufen = if z.zweistufig() { "schnell und genau" } else { "eine Sprosse" };
            format!("{} ({stufen})", z.fuer(myl_senses::Stufe::Schnell).programm.display())
        }),
    );
    zeile("Hoeren", s.hoeren.as_ref().map(|z| z.programm.display().to_string()));
    zeile("Sprechen", s.sprechen.as_ref().map(|z| z.name()));
    zeile("Aufnehmen", s.aufnehmen.as_ref().map(|z| z.programm.display().to_string()));
    // ⚑ **Die beiden Blicke gehoeren in dieselbe Uebersicht** (2026-09-23).
    //   📌 Am 2026-09-18 standen die Sinneswerkzeuge schon einmal in der
    //   Ruestung und in keiner Liste, und der Kopf von `main.rs` haelt
    //   fest, was das kostet: **dieselbe Frage an zwei Orten, und der
    //   zweite meldet sich nicht.**
    zeile("Bildschirm", s.bildschirm.as_ref().map(|z| z.programm.display().to_string()));
    zeile(
        "Kamera",
        s.kamera.as_ref().map(|z| format!("{} ({} {})", z.programm.display(), z.quelle.0, z.quelle.1)),
    );
    // ⚑ **Schrift gehoert in dieselbe Uebersicht**, aus demselben
    //   Grund: Wer sich fragt, warum ein PDF nicht gelesen wird, schaut
    //   hier nach und nicht in den Quelltext.
    zeile(
        "Schrift",
        s.schrift.as_ref().map(|z| match &z.weg {
            myl_senses::schrift::Schriftweg::Pdftotext(p) => format!("{} (PDF)", p.display()),
            myl_senses::schrift::Schriftweg::Mutool(p) => format!("{} (PDF)", p.display()),
            myl_senses::schrift::Schriftweg::Python(p) => {
                format!("{} mit pypdf oder fitz (PDF)", p.display())
            }
        }),
    );
    // ⛔️ **Das Geraet ist nicht die Erlaubnis.** Wer hier zwei Haken
    //    sieht und nicht liest, dass beide Werkzeuge trotzdem fehlen,
    //    sucht den Fehler danach an der falschen Stelle.
    // ⚑ **Dieselbe Ableitung wie die Ruestung**, samt
    //   Umgebungsuebersteuerung. Eine eigene Lesart hier zeigte einen
    //   Stand, den der Agent nicht hat.
    let b = match myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
        Ok(e) => myl_client::sinneswerkzeuge::Blickbefugnis::aus_einstellung(&e.agent),
        Err(_) => myl_client::sinneswerkzeuge::Blickbefugnis::fuer(
            myl_client::sinneswerkzeuge::Blickbefugnis::keine(),
        ),
    };
    let stand = |an: bool| if an { "scharf" } else { "AUS" };
    println!(
        "  {:<10} Bildschirm {} (agent.blick_bildschirm), Kamera {} (agent.blick_kamera)",
        "Blicken",
        stand(b.bildschirm),
        stand(b.kamera)
    );
    if !b.bildschirm && !b.kamera {
        println!("      Ohne Scharfstellung bietet der Agent bildschirm_ansehen und");
        println!("      kamera_ansehen nicht an, auch wenn die Geraete da sind.");
    }
    if let Some(h) = s.stimmhinweis() {
        println!();
        println!("⚠️ {h}");
    }

    // ⚑ **Die Probe fuers Sprechen geht ueber denselben Vorleser wie im
    // Fenster**, samt Dauerlaeufer und satzweiser Zerlegung. Eine Probe,
    // die einen anderen Weg nimmt als der Betrieb, prueft den Betrieb
    // nicht.
    if let Some(i) = args.iter().position(|a| a == "--sprich") {
        let text = args[i + 1..].join(" ");
        if text.trim().is_empty() {
            eprintln!("myl sinne --sprich braucht einen Text");
            return 1;
        }
        let Ok(zeug) = s.sprechen.as_ref() else {
            eprintln!("myl sinne: es ist kein Sprechmodell eingerichtet");
            return 1;
        };
        println!();
        println!("Spricht ueber {} ({}){}", zeug.name(),
            if zeug.dauerhaft() { "Dauerlaeufer" } else { "je Satz ein Aufruf" },
            if zeug.probe.is_some() { ", mit eigener Stimme" } else { "" });
        let anfang = std::time::Instant::now();
        let mut vorleser = myl_senses::sprechen::Vorleser::neu(zeug);
        vorleser.schub(&text);
        let fehler = vorleser.abschliessen();
        for f in &fehler {
            eprintln!("  {f}");
        }
        println!("  in {:.1} s", anfang.elapsed().as_secs_f64());
        return if fehler.is_empty() { 0 } else { 1 };
    }

    let Some(datei) = args.first().filter(|a| !a.starts_with("--")) else {
        println!();
        println!("`myl sinne <datei>` schickt eine Datei hindurch und zeigt, was herauskommt.");
        println!("`myl sinne --sprich <text>` laesst ihn vorlesen, satzweise wie im Fenster.");
        println!("Eingerichtet wird mit `sh SYSTEM/install/sinne-einrichten.sh`.");
        return 0;
    };
    let pfad = std::path::Path::new(datei);
    let anfang = {
        use std::io::Read;
        let mut puffer = vec![0u8; 4096];
        match std::fs::File::open(pfad).and_then(|mut f| f.read(&mut puffer)) {
            Ok(n) => {
                puffer.truncate(n);
                puffer
            }
            Err(f) => {
                eprintln!("myl sinne: {datei}: {f}");
                return 1;
            }
        }
    };
    let art = myl_client::anhang::art_bestimmen(&anfang, datei);
    println!();
    println!("Probe: {datei} ({})", art.wort(true));
    let anfang_zeit = std::time::Instant::now();
    // ⚑ **Die genaue Sprosse**, wie beim Werkzeugaufruf: Wer ausdruecklich
    // fragt, will die bessere Antwort.
    match myl_senses::auswerten(&s, pfad, art, None, myl_senses::Stufe::Genau) {
        None => println!("  Fuer diese Art sieht hier niemand hin; `read_file` liest sie, wenn sie Text ist."),
        Some(Ok(text)) => {
            println!("  in {:.1} s:", anfang_zeit.elapsed().as_secs_f64());
            for z in text.lines() {
                println!("  {z}");
            }
        }
        Some(Err(grund)) => {
            for z in grund.lines() {
                eprintln!("  {z}");
            }
            return 1;
        }
    }
    0
}

/// **`myl anhaenge`: was der Nutzer angehaengt hat, und wie es wieder
/// weggeht.**
///
/// ⚑ **Weil nichts von selbst aufraeumt.** Der Mitschnitt hat eine
/// Obergrenze, weil er von selbst entsteht; eine Datei hat der Nutzer
/// ausdruecklich hergegeben, und sie stillschweigend wegzuwerfen waere
/// eine Ueberraschung. Also liegt sie, bis jemand es sagt, und dieser
/// Befehl ist die Stelle, an der man es sagt.
///
/// ⚠️ **`--aufraeumen` fragt nicht nach.** Es nennt vorher, was es
/// loeschen wird, und wer es tippt, hat die Liste gesehen.
fn anhaenge(args: &[String]) -> i32 {
    use myl_client::anhang;
    let Some(wurzel) = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
        .ok()
        .and_then(|e| e.agent.wurzel.clone())
        .or_else(myl_client::Einstellungen::standard_wurzel)
        .map(std::path::PathBuf::from)
    else {
        eprintln!("myl anhaenge: es ist kein Arbeitsordner gesetzt (`myl setzen agent.wurzel <pfad>`)");
        return 1;
    };
    let ordner = wurzel.join(myl_client::verlauf::ORDNER).join(anhang::ORDNER);
    let liste = anhang::vorhandene(&wurzel);
    println!("Anhaenge in {}", ordner.display());
    if liste.is_empty() {
        println!("  (nichts)");
        println!();
        println!("Angehaengt wird in der Konsole mit `/datei <pfad>` oder im Fenster");
        println!("mit dem Knopf und per Ziehen.");
        return 0;
    }
    let gesamt: u64 = liste.iter().map(|(_, b)| b).sum();
    for (name, bytes) in &liste {
        println!("  {:<40} {:>9}", name, anhang::menschlich(*bytes));
    }
    println!();
    println!("{} Datei(en), {} zusammen.", liste.len(), anhang::menschlich(gesamt));

    if args.iter().any(|a| a == "--aufraeumen") {
        // ⚑ **Genau die Dateien, die oben standen**, und nicht der Ordner
        // mit allem darin. Wer die Liste gesehen hat, hat gesehen, was
        // weggeht; ein `remove_dir_all` naehme auch mit, was seither
        // dazukam oder nie in der Liste stand.
        let mut weg = 0;
        for (name, _) in &liste {
            match std::fs::remove_file(ordner.join(name)) {
                Ok(()) => weg += 1,
                Err(f) => eprintln!("myl anhaenge: {name}: {f}"),
            }
        }
        println!("Geloescht: {weg} von {} Datei(en).", liste.len());
        if weg < liste.len() {
            return 1;
        }
    } else {
        println!("`myl anhaenge --aufraeumen` loescht sie alle.");
    }
    0
}


fn skills(args: &[String]) -> i32 {
    use myl_client::skills;
    let allgemein = skills::allgemeiner_ordner();
    let wurzel = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
        .ok()
        .and_then(|e| e.agent.wurzel.clone())
        .or_else(myl_client::Einstellungen::standard_wurzel)
        .map(std::path::PathBuf::from);

    if args.iter().any(|a| a == "--anlegen") {
        if let Err(f) = std::fs::create_dir_all(&allgemein) {
            eprintln!("myl skills: {} : {f}", allgemein.display());
            return 1;
        }
        println!("Angelegt: {}", allgemein.display());
    }

    println!("Allgemein (gilt ueberall): {}", allgemein.display());
    match &wurzel {
        Some(w) => println!("Projekt (dieser Ordner):  {}", skills::projektordner(w).display()),
        None => println!("Projekt (dieser Ordner):  keiner, es ist kein Arbeitsordner gesetzt"),
    }
    println!();

    let mappen = skills::alle(wurzel.as_deref());
    if mappen.is_empty() {
        println!("Keine Wissensmappe gefunden.");
        println!();
        println!("Eine entsteht aus Markdown mit:");
        println!("  python3 TRAINING/korpus/md_zu_mappe.py <ordner> --name <kennung> \\");
        println!("      --ausgabe {}", allgemein.display());
        println!("`myl skills --anlegen` legt den allgemeinen Ordner an.");
        return 0;
    }
    for m in &mappen {
        println!("  {:<24} {}", m.name, m.satz);
    }
    println!();
    println!("{} Mappe(n). Der Agent nennt sie mit `list_skills` und liest mit `read_skill`.", mappen.len());
    0
}

/// **Der Mitschnitt fuer einen Menschen**: Uebersicht, Zeilen, Loeschen.
///
/// # ⚑ Warum es diesen Befehl gibt, obwohl der Agent sein Werkzeug hat
///
/// Der Agent liest ueber `read_history` und `list_history`, und beide
/// sehen nur, was unter der Einhaengegrenze liegt. **Ein Mensch
/// braucht drei Dinge, die kein Werkzeug leistet**: zu sehen, wie viel
/// da liegt und wie viel Platz es nimmt; **einen ausdruecklichen
/// Loeschweg**, denn der Mitschnitt ist Klartext und gehoert dem
/// Ordner, also nimmt ihn kein Aufraeumen der Gespraechsliste mit; und
/// eine Stelle, an der die Zeilennummern des Verzeichnisses von Hand
/// nachpruefbar sind. ⚑ **Dieselben Zahlen wie `sed -n`, `grep -n`
/// und der Sprung im Editor**, denn es sind Zeilen der Datei.
fn verlauf(args: &[String]) -> i32 {
    use myl_client::verlauf;

    let loeschen = args.iter().any(|a| a == "--loeschen");
    let rest: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();

    // Der erste freie Wert ist ein Ordner, wenn es ihn gibt.
    let wurzel = rest
        .first()
        .map(|a| std::path::PathBuf::from(a.as_str()))
        .filter(|p| p.is_dir())
        // ⛔️ **Danach das Arbeitsverzeichnis, aber nur, wenn dort
        // wirklich ein Mitschnitt liegt.** Die Konsole schreibt in
        // ihr Arbeitsverzeichnis („Wer `myelith` hier tippt, hat die
        // Frage beantwortet"), das Fenster in den eingestellten
        // Ordner. **Ohne diesen Schritt fragt man im selben
        // Verzeichnis nach und bekommt „kein Mitschnitt" zu sehen,
        // waehrend die Datei danebenliegt**, weil die Einstellung auf
        // einen anderen Ordner zeigt. ⚑ **Die Bedingung ist der
        // vorhandene Ordner und nicht das Verzeichnis selbst**: Sonst
        // uebernaehme das Arbeitsverzeichnis jeden Aufruf und der
        // eingestellte Ordner waere nur noch aus Zufall erreichbar.
        .or_else(|| {
            let hier = std::env::current_dir().ok()?;
            hier.join(myl_client::verlauf::ORDNER).is_dir().then_some(hier)
        })
        .or_else(|| {
            myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
                .ok()
                .and_then(|e| e.agent.wurzel.clone())
                .map(std::path::PathBuf::from)
        })
        .or_else(|| myl_client::Einstellungen::standard_wurzel().map(std::path::PathBuf::from));
    let Some(wurzel) = wurzel else {
        eprintln!("myl verlauf: kein Arbeitsordner. Einen angeben oder `agent.wurzel` setzen.");
        return 1;
    };

    // ⛔️ **Loeschen ist eine eigene Handlung**, und sie sagt, was sie
    // getan hat. Ohne Sitzung geht der ganze Ordner.
    if loeschen {
        let sitzung = rest.get(1).map(|s| s.as_str());
        return match verlauf::loeschen(&wurzel, sitzung) {
            Ok(0) => {
                println!("Nichts zu loeschen.");
                0
            }
            Ok(n) => {
                println!("{n} Sitzung(en) geloescht.");
                0
            }
            Err(f) => {
                eprintln!("myl verlauf: {f}");
                1
            }
        };
    }

    let u = verlauf::uebersicht(&wurzel);
    if u.episoden == 0 {
        println!("Kein Mitschnitt unter {}/{}.", wurzel.display(), verlauf::ORDNER);
        println!("Er entsteht beim Verdichten des Gespraechs.");
        return 0;
    }

    // Zwei Zahlen: die Zeilen des juengsten Mitschnitts.
    let zahlen: Vec<usize> = args.iter().filter_map(|a| a.parse::<usize>().ok()).collect();
    if zahlen.len() >= 2 {
        let Some((pfad, _)) = &u.juengste else {
            eprintln!("myl verlauf: der juengste Mitschnitt ist nicht lesbar.");
            return 1;
        };
        return match verlauf::zeilen(pfad, zahlen[0], zahlen[1]) {
            Ok(t) => {
                print!("{t}");
                0
            }
            Err(f) => {
                eprintln!("myl verlauf: {f}");
                1
            }
        };
    }

    // ⚑ **Was man sieht, raeumt man.** Zahl und Platz stehen deshalb
    // oben, nicht nur der Inhalt.
    println!(
        "{} Episoden aus {} Sitzungen, {:.1} KiB, hoechstens {} Sitzungen.",
        u.episoden,
        u.sitzungen,
        u.bytes as f64 / 1024.0,
        verlauf::HOECHSTENS_SITZUNGEN
    );
    for s in verlauf::sitzungen(&wurzel) {
        let n = std::fs::read_dir(&s).into_iter().flatten().count();
        println!(
            "  {:<24} {n} Episode(n)",
            s.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default()
        );
    }
    if let Some((pfad, v)) = &u.juengste {
        println!();
        println!("Juengste: {} ({}), Sitzung {}", v.datum, v.modell, v.sitzung);
        println!("  {}", pfad.display());
        println!();
        for a in &v.abschnitte {
            println!("  {:>5}-{:<5} {:<10} {}", a.von, a.bis, a.rolle, a.kopf);
        }
    }
    println!();
    println!("Zeilen lesen:  myl verlauf <von> <bis>");
    println!("Loeschen:      myl verlauf --loeschen [<sitzung>]");
    0
}

fn setzen(args: &[String]) -> i32 {
    let (Some(feld), Some(wert)) = (args.first(), args.get(1)) else {
        eprintln!("myl setzen: es fehlt Feld oder Wert");
        eprintln!("myl setzen: bekannte Felder:");
        // ⚑ **In derselben Sprache wie das Fenster.** Es ist dieselbe
        // Einstellung und dasselbe Programm; ein Bedieninstrument, das
        // sie nicht kennt, waere die zweite Stelle, an der sie gilt.
        //
        // 📌 **Eine unlesbare Ablage kostet hier nichts.** Wer die
        // Feldliste sehen will, soll sie sehen, auch wenn die Datei
        // kaputt ist; dann eben auf Deutsch.
        let sprache = Einstellungen::lesen(&Einstellungen::vorgabepfad())
            .map(|e| e.oberflaeche.sprache)
            .unwrap_or_default();
        for f in myl_client::einstellungen::FELDER {
            let f = f.in_sprache(sprache);
            eprintln!("  {:<20} {:<8} {}", f.name, format!("{:?}", f.art), f.titel);
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
/// Welche Werkzeugkiste fuer diesen Lauf.
///
/// ⚑ Wie `--deutsch` ein **Vergleichsschalter** und keine Einstellung:
/// `search_files` und `edit_file` schliessen echte Luecken, aber fuenf
/// Werkzeuge im Kontext machen die Auswahl fuer ein 4B-Modell schwerer
/// als drei. Welches ueberwiegt, sagt die Agentenprobe und nicht diese
/// Datei.
///
/// 📌 **Der Schalter nimmt dieselben Woerter wie die Einstellung**, und
/// er nimmt sie aus derselben Funktion. Bis zum 2026-09-11 kannte er
/// nur `voll` und verglich es von Hand, waehrend die Hilfe darueber
/// `knapp` nannte: **ein Wort, das es nie gab.**
fn satz_fuer_diesen_lauf(
    e: &Einstellungen,
    args: &[String],
) -> myl_client::werkzeuge::Werkzeugkiste {
    use myl_client::werkzeuge::Werkzeugkiste;
    // 📌 **Hier stand `Werkzeugkiste::default()` als Vorgabe**, also
    // `Base`, ohne die Einstellung ueberhaupt anzusehen. Wer seine Kiste
    // auf `advanced` stellte und `myl agent` ohne Schalter rief, bekam
    // trotzdem `Base`: Die Manifeste kamen aus dem eingestellten Ordner,
    // die eingebauten Werkzeuge aus einer anderen Quelle. **Dieselbe
    // Wahl an zwei Orten**, und die zweite meldet sich nicht.
    let Some(wort) = args.windows(2).find(|p| p[0] == "--werkzeuge").map(|p| p[1].clone()) else {
        return myl_client::kisten::kiste_der_gilt(&e.agent);
    };
    // ⚑ Der Schalter ueberstimmt fuer diesen einen Lauf, und er nimmt
    // dieselben Woerter wie der Ordnername.
    match wort.trim().to_ascii_lowercase().as_str() {
        "base" | "advanced" | "1337" => Werkzeugkiste::aus_ordnername(&wort),
        _ => {
            // ⛔️ **Abbruch und kein Rueckfall** (Fund 437, 2026-09-23).
            //
            // Bis hierher wurde gewarnt und mit der **eingestellten**
            // Kiste weitergefahren. Wer `--werkzeuge` setzt, sagt aber
            // gerade, dass die eingestellte nicht gelten soll; er
            // bekommt dann das Gegenteil dessen, wonach er gefragt hat.
            //
            // 📌 **Gefunden von der Agentenmessung**, die `voll`
            // uebergab und darueber „Werkzeugsatz: voll" schrieb. `myl`
            // warnte in eine Datei, die niemand las, und mass **Base**;
            // `run_command` war in keinem Lauf im Angebot. **Eine
            // Warnung, nach der es weitergeht, ist ein Kommentar mit
            // Laufzeit.**
            eprintln!("myl: --werkzeuge {wort} kenne ich nicht, moeglich sind Base, Advanced");
            std::process::exit(2);
        }
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
    let mut m = match Oertlichesmodell::laden(&artefakt, &e.kapazitaet) {
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
    // ⚑ `myl modell` gibt Auskunft ueber ein Artefakt, das der
    // Aufrufer ausdruecklich nennt, und ist keine Inbetriebnahme. Die
    // Freigabe wird trotzdem gelesen und nicht umgangen: Wer sein
    // Budget kleiner gesetzt hat, als das Artefakt gross ist, soll das
    // hier genauso erfahren wie beim Fragen.
    let kap = Einstellungen::lesen(&Einstellungen::vorgabepfad())
        .map(|e| e.kapazitaet)
        .unwrap_or_default();
    match Oertlichesmodell::laden(artefakt, &kap) {
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
    let mut m = match Oertlichesmodell::laden(&artefakt, &e.kapazitaet) {
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
    let ruestung = match ruestung_fuer_diesen_lauf(&e, args, &auftrag) {
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
    let bezeugtes = !args.iter().any(|a| a == "--nur-verankert");
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
    let mut m = match Oertlichesmodell::laden(&artefakt, &e.kapazitaet) {
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
        satz_fuer_diesen_lauf(&e, rest),
        vec![werkzeug_uhr()],
    ) {
        Ok(r) => r,
        Err(msg) => {
            eprintln!("myl sitzung: {msg}");
            return 1;
        }
    };
    let bezeugtes = !rest.iter().any(|a| a == "--nur-verankert");
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
    // ⚑ **Umgesetzt wird in der Kiste** (seit dem 2026-09-16). Hier
    // stand dieselbe Zeile wie im Fenster, und in der Konsole stand
    // gar keine (Fund 381). **Drei Bedieninstrumente, eine Freigabe.**
    myl_client::hardware::anwenden(e);
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
             nachrechenbar. Ohne `--nur-verankert` laufen sie."
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
    // ⚑ **Jeder Auftrag bekommt sein Nachschlagebudget neu**
    // (2026-09-17): Die naechste Frage des Nutzers ist ein neuer Anlass
    // nachzulesen.
    ruestung.nachschlagebudget_zuruecksetzen();
    let grenzen = myl_local_agent::vollmacht_grenzen::Sitzungsgrenzen::neu(
        myl_client::lauf::kontrakt_fuer(schritte),
        ruestung.kasten.angebote(),
    );
    let zuordnung = ruestung.zuordnung();

    let erg = myl_local_agent::schleife::Lauf {
        hausregel: None,
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
        melder: None,
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
    let mut m = match Oertlichesmodell::laden(&artefakt, &e.kapazitaet) {
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
        satz_fuer_diesen_lauf(&e, rest),
        vec![werkzeug_uhr()],
    ) {
        Ok(r) => r,
        Err(msg) => {
            eprintln!("myl auftraege: {msg}");
            return 1;
        }
    };
    let bezeugtes = !rest.iter().any(|a| a == "--nur-verankert");
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
/// mindestens eins. 📌 Sie gilt fuer den ganzen Prozess und wird
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
                wurzel: Some("/vorher".into()),
                schreiben: true,
                kistenordner: None,
                warnung: true,
                modus: Default::default(),
                blick_bildschirm: false,
                blick_kamera: false,
                web_recherche: false,
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

    /// 📌 Der Fall, der den Eintrag noetig machte.
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

    /// 📌 Eine fehlende Datei ist ein Fehler und keine leere Liste: Ein
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

    /// **Jedes Feld des Katalogs steht in der Uebersicht.**
    ///
    /// 📌 **Fund 312.** Sie war von Hand gefuehrt und kannte zehn von
    /// dreizehn Feldern: `oberflaeche.sprache`, `agent.werkzeuge` und
    /// `ausgabe.ordner` liessen sich setzen und standen danach
    /// nirgends. **Wer ein Feld anlegt, soll es nicht an zwei Stellen
    /// anlegen muessen**, und wer es doch tut, soll es hier merken.
    #[test]
    fn die_uebersicht_zeigt_jedes_feld() {
        let e = Einstellungen::default();
        let zeilen = uebersicht(&e, 4);
        for f in myl_client::einstellungen::FELDER.iter() {
            assert!(
                zeilen.iter().any(|z| z.contains(f.name)),
                "`{}` fehlt in der Uebersicht von `myl einstellungen`",
                f.name
            );
        }
        assert_eq!(zeilen.len(), myl_client::einstellungen::FELDER.len());
    }

    /// **Und keine Zeile behauptet einen Wert, den es nicht gibt.**
    ///
    /// ⚑ Eine Grenze, die nicht gesetzt ist, ist keine Null. Beim
    /// Arbeitsordner steht die **Vorgabe**, die wirklich greift, und
    /// nur ohne Vorgabe „gar keine Dateiwerkzeuge".
    ///
    /// 📌 Bis zum 2026-09-15 stand hier fest „keine, ohne
    /// Dateiwerkzeuge", auch als die Vorgabe schon griff.
    #[test]
    fn was_nicht_gesetzt_ist_steht_als_solches_da() {
        let zeilen = uebersicht(&Einstellungen::default(), 4);
        let zeile = |name: &str| {
            zeilen.iter().find(|z| z.contains(name)).cloned().unwrap_or_default()
        };
        assert!(zeile("kap.speicher").contains("ohne Grenze"), "{}", zeile("kap.speicher"));
        let w = zeile("agent.wurzel");
        match myl_client::Einstellungen::standard_wurzel() {
            Some(p) => assert!(w.contains(&p) && w.contains("Vorgabe"), "{w}"),
            None => assert!(w.contains("keine, ohne Dateiwerkzeuge"), "{w}"),
        }
        assert!(zeile("modell.artefakt").contains("(nicht gesetzt)"), "{}", zeile("modell.artefakt"));
    }

    /// **Die Einheit kommt aus der Beschriftung und nicht aus einer
    /// zweiten Liste.**
    #[test]
    fn eine_grenze_in_gibibyte_traegt_ihre_einheit() {
        let mut e = Einstellungen::default();
        e.kapazitaet.speicher_gib = Some(12);
        e.kapazitaet.kerne = Some(4);
        let zeilen = uebersicht(&e, 4);
        let zeile = |name: &str| zeilen.iter().find(|z| z.contains(name)).cloned().unwrap_or_default();
        assert!(zeile("kap.speicher").contains("12 GiB"), "{}", zeile("kap.speicher"));
        assert!(!zeile("kap.kerne").contains("GiB"), "{}", zeile("kap.kerne"));
    }
}
