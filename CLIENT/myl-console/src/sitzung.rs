//! Der Ablauf: Vorspann, Modell, dann Auftrag um Auftrag.
//!
//! # ⚑ Ein Zustand, sieben Befehle, sonst nichts
//!
//! Was ein Mensch hier tun kann, ist absichtlich klein: einen Auftrag
//! geben, das Modell wechseln, die Einstellungen oeffnen, gehen, und seit
//! dem 2026-09-14 den Kontext des Gespraechs ansehen, verdichten oder neu
//! beginnen. **Ein Konsolenprogramm mit zwanzig Befehlen ist ein
//! Programm, dessen Hilfeseite man liest, statt es zu benutzen.**

use std::io::{IsTerminal, Write};

use crossterm::cursor::MoveTo;
use crossterm::terminal::{Clear, ClearType};
use crossterm::style::{Print, ResetColor, SetForegroundColor};
use std::path::PathBuf;

use crate::schirm::{Rahmen, Schirm};
use crate::{animation, anzeige, auswahl, banner, design, eingabe, einstellseite, farben, wahl};

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
    /// ⚑ **Die Sitzung dieses Programmlaufs.** Sie gliedert die
    /// Mitschnitte im Arbeitsordner; `/clear` beginnt eine neue Episode
    /// und loescht keine alte.
    sitzung: String,
    /// Das geladene Modell, sobald es geladen ist.
    modell: Option<myl_client::Oertlichesmodell>,
    /// Der Pfad, aus dem es geladen wurde.
    artefakt: String,
    /// Wie das Modell heisst, also der Name seines Verzeichnisses.
    name: String,
    /// Welche Werkzeugkiste dieser Lauf bekommt, als Wort.
    kiste: String,
    /// Ob der Agent schreiben darf.
    ///
    /// ⚑ **Er steht in der Fusszeile**, denn ohne ihn bekommt der Agent
    /// `write_file` und `edit_file` gar nicht erst, und dann sagt er
    /// „ich kann keine Dateien speichern", ohne dass jemand weiss,
    /// warum. **Dieselbe Klasse wie Fund 315:** Was eine Faehigkeit
    /// wegnimmt, gehoert dorthin, wo man die Faehigkeit vermisst.
    schreibt: bool,
    /// Welche Farben dieser Lauf benutzt.
    design: myl_client::einstellungen::Konsolendesign,
    /// Ob schreibende Handlungen vorgelegt werden.
    ///
    /// ⚑ **Er kommt aus den Einstellungen und gilt fuer die Sitzung.**
    /// Umschalt-Tab wechselt ihn hier und schreibt die Ablage
    /// **nicht** um: Ein Tastendruck, der eine Datei im
    /// Benutzerverzeichnis aendert, ist eine Ueberraschung.
    modus: myl_client::einstellungen::Agentenmodus,
    /// Der reservierte untere Rand, sofern es einen gibt.
    ///
    /// ⚑ `None` heisst: keine Roehre und kein zu kleines Fenster,
    /// sondern **beides zusammen** als eine Frage. Wo kein Rand ist,
    /// wird auch keiner gezeichnet.
    schirm: Option<Schirm>,
    /// **Das Gespraech, das ueber Auftraege mitgeht** (Entscheidung C2,
    /// beantwortet am 2026-09-14). `/clear` beginnt ein neues, `/compress`
    /// fasst es zusammen.
    gespraech: myl_client::gespraech::Gespraech,
    /// Die Werkzeugansage des letzten Laufs, damit `/context` sie nicht neu
    /// bauen muss.
    ansage: Option<myl_client::Nachricht>,
    /// Belegter Kontext nach dem letzten Auftrag, in Prozent, fuer die
    /// Fusszeile.
    kontext_prozent: Option<usize>,
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

    // ⚑ **Die Warnung vor dem Agentenbetrieb** (Auftrag des
    // Projektinhabers, 2026-09-15). Diese Konsole **ist** der Agent,
    // also steht sie am Anfang und nicht hinter einem Schalter.
    if warnung_zeigen(farbe) == SCHLECHT {
        return SCHLECHT;
    }

    // ⛔️ **Die Freigabe wird umgesetzt, und bis zum 2026-09-16 wurde
    // sie das hier nicht** (Fund 381). `kap.kerne` liess sich setzen,
    // anzeigen und abspeichern; `myl` und das Fenster wandten es an,
    // diese Konsole las es nie. **Eine Einstellung, die an einem von
    // drei Bedieninstrumenten nichts bewirkt, ist schlimmer als keine**,
    // denn sie wirkt ja anderswo, und niemand sucht den Unterschied im
    // Programm.
    if let Ok(e) = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
        myl_client::hardware::anwenden(&e);
    }

    // ⚑ **Eine Sitzungskennung je Programmlauf.** Die Konsole fuehrt
    // keine Gespraechsliste; was sie hat, ist dieser Lauf. Sekunden und
    // Prozesskennung zusammen sind eindeutig genug und verraten nichts.
    let sitzung = format!(
        "{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        std::process::id()
    );

    let mut stand = Stand {
        ordner,
        sitzung,
        modell: None,
        artefakt: String::new(),
        name: String::new(),
        kiste: "?".to_string(),
        schreibt: myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
            .map(|e| e.agent.schreiben)
            .unwrap_or(false),
        design: myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
            .map(|e| e.oberflaeche.design)
            .unwrap_or_default(),
        modus: myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
            .map(|e| e.agent.modus)
            .unwrap_or_default(),
        schirm: None,
        gespraech: myl_client::gespraech::Gespraech::neu(),
        ansage: None,
        kontext_prozent: None,
    };

    // ⚑ **Erst das Bild, dann das Modell** (Festlegung des
    // Projektinhabers, 2026-09-11). Die Designwahl faerbt die
    // Modellwahl schon mit; umgekehrt saehe man sein Design zum ersten
    // Mal, wenn es nichts mehr zu waehlen gibt.
    design_waehlen(&mut stand);

    // ⚑ **Erst das Modell, dann die Eingabe.** Ein Eingabefeld, das bei
    // der ersten Zeile „kein Modell" sagt, hat die Frage nur
    // aufgeschoben.
    // ⚑ **Hier fragt nichts nach** (Festlegung des Projektinhabers,
    // 2026-09-15): Vor der ersten Wahl laeuft kein Modell, es ist nichts
    // zu verlieren, und eine Rueckfrage waere nur eine Taste mehr. Die
    // Sicherung greift erst, wenn schon eines gewaehlt ist, also in
    // `/model`.
    match modell_waehlen(&mut stand) {
        Modellwahl::Geladen => {}
        Modellwahl::Abgebrochen => return GUT,
        Modellwahl::Unmoeglich => return SCHLECHT,
    }
    // ⚑ Auch hier frei: Die Modelliste ist abgearbeitet, und das Laden
    // soll auf einem leeren Schirm stehen, nicht unter ihr.
    frei_bis_auf_das_logo(design::toene(stand.design).akzent);

    // ⚑ **Erst jetzt wird der untere Rand reserviert.** Vorher laufen
    // Vorspann, Modellwahl und Ladeanzeige, und die sollen den ganzen
    // Schirm haben.
    stand.schirm = Schirm::messen();
    if let Some(sch) = stand.schirm {
        sch.einrichten();
    }

    let ende = schleife(&mut stand);

    // ⚠️ **Was reserviert wurde, wird zurueckgegeben.** Ein Programm,
    // das mit gesetztem Rollbereich endet, hinterlaesst eine Shell, die
    // nur noch im oberen Teil des Fensters schreibt.
    if let Some(sch) = stand.schirm {
        sch.aufloesen();
    }
    ende
}

/// Waehlt das Modell und laedt es.
///
/// ⚑ **Was eingestellt ist, wird vorgeschlagen und nicht gesetzt.**
/// Wer schon ein Artefakt hat, druckt einmal Eingabe; wer keines hat,
/// bekommt die Liste.
/// **Raeumt Bild und Rueckblaetterspeicher.**
///
/// ⚑ **Erst `All`, dann `Purge`, und diese Reihenfolge ist der Punkt.**
/// Mehrere Terminals schieben den bisherigen Inhalt beim Leeren in den
/// Rueckblaetterspeicher; `Purge` leert genau den. Umgekehrt herum
/// legte `All` den alten Bildschirm gleich wieder hinein.
fn schirm_leeren() {
    if !std::io::stdout().is_terminal() {
        return;
    }
    let _ = crossterm::execute!(
        std::io::stdout(),
        Clear(ClearType::All),
        Clear(ClearType::Purge),
        MoveTo(0, 0)
    );
}

/// **Welches Bild dieser Lauf traegt.**
///
/// ⚑ **Voreingestellt ist, was in `oberflaeche.design` steht**, und die
/// Wahl hier aendert die Ablage **nicht**: Sie gilt fuer diese Sitzung.
/// Wer sein Design dauerhaft will, setzt es dort.
///
/// ⚠️ Esc laesst alles, wie es ist. **Eine Frage, die sich nicht
/// ueberspringen laesst, ist keine Frage, sondern eine Huerde.**
/// **Bildschirm frei, nur der Schriftzug bleibt.**
///
/// ⚑ Auftrag des Projektinhabers, 2026-09-15: Nach einer Wahl soll die
/// Liste nicht stehen bleiben. Was gewaehlt wurde, steht danach ohnehin
/// in der Fusszeile; die abgearbeitete Liste ist von da an nur noch
/// Vorgeschichte und schiebt das Kommende nach unten.
///
/// ⚠️ **Nur mit Terminal.** In einer Roehre waere ein Loeschbefehl
/// Zeichensalat in einer Datei, und der Schriftzug stuende dann zweimal
/// darin.
fn frei_bis_auf_das_logo(farbe: crossterm::style::Color) {
    use std::io::IsTerminal;
    if !std::io::stdout().is_terminal() {
        return;
    }
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0),
    );
    banner::bildschirm_mit(farbe);
}

fn design_waehlen(stand: &mut Stand) {
    use myl_client::einstellungen::Konsolendesign;
    let punkte: Vec<wahl::Punkt> = Konsolendesign::ALLE
        .iter()
        .map(|d| wahl::Punkt {
            titel: d.name().to_string(),
            hinweis: d.satz().to_string(),
            offen: true,
        })
        .collect();
    let start = Konsolendesign::ALLE.iter().position(|d| *d == stand.design).unwrap_or(0);
    if let Some(i) = wahl::waehlen_ab("Welches Design?", &punkte, start, design::toene(stand.design))
    {
        stand.design = Konsolendesign::ALLE[i];
    }
    // ⚑ Mit der eben gewaehlten Farbe: Der Schriftzug zeigt die
    // Entscheidung, statt sie erst beim naechsten Bildschirm zu zeigen.
    frei_bis_auf_das_logo(design::toene(stand.design).akzent);
}

/// Wie eine Modellwahl ausgegangen ist.
///
/// ⚑ **Drei Ausgaenge statt zweier** (Auftrag des Projektinhabers,
/// 2026-09-15). `bool` warf „der Nutzer hat abgebrochen" und „hier geht
/// es nicht" in einen Topf, und der Aufrufer konnte das eine nicht vom
/// anderen unterscheiden: Esc beendete das Programm genauso wie ein
/// fehlendes Artefakt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modellwahl {
    /// Gewaehlt und geladen.
    Geladen,
    /// Der Nutzer hat Esc gedrueckt. **Eine Absicht, keine Not.**
    Abgebrochen,
    /// Es gibt nichts zu waehlen oder das Laden scheiterte.
    Unmoeglich,
}

fn modell_waehlen(stand: &mut Stand) -> Modellwahl {
    let e = match myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
        Ok(e) => e,
        Err(f) => {
            eprintln!("myelith: die Einstellungen sind nicht lesbar: {f}");
            return Modellwahl::Unmoeglich;
        }
    };

    // ⚑ **Dieselbe Liste wie im Fenster**, aus derselben Funktion. Sie
    // traegt die Anzeigenamen aus dem Katalog und den Netzeintrag, der
    // dasteht und nicht traegt.
    let liste = myl_client::modelle::liste(&e);
    if !liste.iter().any(|m| m.offen) {
        eprintln!("myelith: hier liegt kein gebautes Artefakt.");
        eprintln!("  Im Fenster laesst sich eines holen und bauen: `myl-oberflaeche`.");
        return Modellwahl::Unmoeglich;
    }

    let punkte: Vec<wahl::Punkt> = liste
        .iter()
        .map(|m| wahl::Punkt {
            titel: m.name.clone(),
            // ⚑ Unter dem offenen Eintrag steht, was die Maschine dafuer
            // mindestens braucht, und woher er kommt; unter dem
            // gesperrten, warum er nicht geht.
            //
            // 📌 **Die Hardwareangabe zuerst** (Festlegung des
            // Projektinhabers, 2026-09-11): Wer waehlt, entscheidet in
            // diesem Moment, ob seine Maschine das Modell traegt. Der
            // Pfad ist danach interessant, nicht davor.
            hinweis: if !m.offen {
                m.warum.clone()
            } else if m.hardware.is_empty() {
                kurz(&m.pfad)
            } else {
                format!("{} · {}", m.hardware, kurz(&m.pfad))
            },
            offen: m.offen,
        })
        .collect();

    // Vorgewaehlt ist, was eingestellt ist.
    let eingestellt = myl_client::ort::absolut(&e.modell.artefakt);
    let start = liste
        .iter()
        .position(|m| myl_client::ort::absolut(&m.pfad) == eingestellt)
        .unwrap_or(0);

    let Some(i) = wahl::waehlen_ab("Welches Modell?", &punkte, start, design::toene(stand.design))
    else {
        return Modellwahl::Abgebrochen;
    };
    let pfad = myl_client::ort::absolut(&liste[i].pfad);
    let name = liste[i].name.clone();

    // ⚠️ **Laden dauert und sagt es.** Ein 4B-Artefakt sind
    // viereinhalb Gigabyte; ohne diese Zeile sieht ein Start wie ein
    // Haenger aus.
    print!("  {name} wird geladen … ");
    let _ = std::io::stdout().flush();
    let anfang = std::time::Instant::now();
    match myl_client::Oertlichesmodell::laden(&pfad, &e.kapazitaet) {
        Ok(m) => {
            // ⚑ **Die Bestaetigung nennt Namen und Pfad** (Festlegung
            // des Projektinhabers, 2026-09-11). Der Name sagt, womit
            // man spricht; der Pfad sagt, welches Artefakt es wirklich
            // ist, und das ist die Angabe, die bei zwei aehnlichen
            // Verzeichnissen den Unterschied macht.
            println!("{:.1} s", anfang.elapsed().as_secs_f64());
            // ⚑ **Jetzt wird aufgeraeumt** (Auftrag des
            // Projektinhabers, 2026-09-11). Die beiden Auswahllisten
            // haben ihre Frage beantwortet; was danach noch dasteht,
            // ist Vergangenheit, durch die jemand scrollen muesste, um
            // zu sehen, wo er ist.
            //
            // ⚑ **Und das Logo bleibt stehen** (Auftrag des
            // Projektinhabers, 2026-09-12). Hier stand ein blosses
            // Leeren, und das Gespraech begann auf einem leeren
            // Schirm. Jetzt steht der Schriftzug oben, und **er wandert
            // mit dem Gespraech nach oben weg wie jede andere Zeile**:
            // Er wird einmal gezeichnet und danach nicht mehr
            // angefasst.
            //
            // ⚠️ **Kein fester Kopf.** Ein Logo, das oben kleben
            // bliebe, braeuchte einen zweiten Rollbereich und naehme
            // dem Gespraech dauerhaft acht Zeilen. Der untere Rand ist
            // reserviert, weil dort die Eingabe steht; oben ist Platz
            // wertvoller als Zierrat.
            banner::bildschirm_mit(farben::logo());
            println!("  Modell {name} ({pfad}) wurde geladen.");
            println!();
            // ⚑ Die Kiste haengt am Modell, also wird sie hier
            // bestimmt und nicht bei jedem Auftrag neu.
            let kiste = myl_client::kisten::kiste_der_gilt(&e.agent);
            stand.kiste = kiste.name().to_string();
            stand.name = name;
            stand.artefakt = pfad;
            stand.modell = Some(m);
            Modellwahl::Geladen
        }
        Err(f) => {
            println!();
            eprintln!("myelith: das Modell laesst sich nicht laden: {f}");
            Modellwahl::Unmoeglich
        }
    }
}

/// **Zeichnet den Rahmen und stellt den Wagen hinein.**
///
/// ⚑ **Gezeichnet vor jeder Eingabe, nicht einmal beim Start.** Ein
/// Terminal hat keine feste Zeile, die man belegen koennte, ohne den
/// Bildschirm zu uebernehmen; was hier steht, wandert mit der Ausgabe
/// nach oben und wird beim naechsten Mal neu gesetzt. **Das ist die
/// ehrliche Fassung von „unten fixiert" ohne eine eigene Bildschirm-
/// verwaltung.**
///
/// ⚠️ **Und nur vor einem Terminal.** In einer Roehre waeren Rahmen und
/// Fusszeile Zeichensalat in einer Datei, die jemand spaeter liest.
fn eingaberahmen(stand: &Stand) {
    let Some(sch) = stand.schirm else {
        return;
    };
    let r = Rahmen::messen();
    let t = design::toene(stand.design);
    let mut aus = std::io::stdout();
    // Wo wir im Rollbereich stehen, bevor wir hinausgehen.
    sch.merken();
    let _ = crossterm::queue!(
        aus,
        SetForegroundColor(t.kante),
        Print(format!("{}{}", sch.zeile(Schirm::KASTEN), r.oben())),
        Print(format!("{}{}", sch.zeile(Schirm::KASTEN + 1), r.leer())),
        Print(format!("{}{}", sch.zeile(Schirm::KASTEN + 2), r.unten())),
        ResetColor,
        SetForegroundColor(t.beiwerk),
        Print(format!(
            "{}{}{}",
            sch.zeile(Schirm::KASTEN + 3),
            r.einzug,
            fusszeile(stand.modus.name(), &stand.name, &stand.kiste, stand.schreibt, stand.kontext_prozent, r.innen)
        )),
        ResetColor,
        // ⚑ **Der Arbeitsordner in der zweiten Zeile darunter**
        // (Festlegung des Projektinhabers, 2026-09-11). Er steht
        // dauerhaft da, weil der Agent genau dort arbeitet.
        SetForegroundColor(t.kante),
        Print(format!(
            "{}{}{}",
            sch.zeile(Schirm::KASTEN + 4),
            r.einzug,
            ordnerzeile(&stand.kurzer_ordner(), r.innen)
        )),
        ResetColor,
        // ⚑ Die Ladezeile gehoert der Anzeige; hier wird sie nur
        // geraeumt, damit nach einem Lauf nichts stehenbleibt.
        Print(sch.zeile(Schirm::LADEZEILE)),
        Print(sch.zeile(Schirm::LADEZEILE + 1)),
    );
    let _ = aus.flush();
    zeile_zeichnen(&sch, &r, t, "");
}

/// **Zeichnet die Eingabezeile neu, samt Wagen.**
///
/// ⚑ **Jedes Zeichen loest das aus, und das ist der Punkt.** Der Rahmen
/// steht fest am unteren Rand; eine Eingabe, die ueber die rechte Kante
/// hinauslaeuft, braeche in die Kantenzeile um. Gezeigt wird deshalb
/// das **Ende** der Zeile, und der Rahmen bleibt ganz.
fn zeile_zeichnen(sch: &Schirm, r: &Rahmen, t: design::Toene, text: &str) {
    let mut aus = std::io::stdout();
    let _ = crossterm::queue!(
        aus,
        Print(sch.zeile(Schirm::KASTEN + 1)),
        SetForegroundColor(t.kante),
        Print(format!("{}│", r.einzug)),
        ResetColor,
        Print(eingabeinhalt(r.innen, text)),
        SetForegroundColor(t.kante),
        Print("│"),
        ResetColor,
        // 📌 **Fund 347: der Wagen stand eine Zeile unter der Eingabe.**
        //
        // `Schirm::zeile` schreibt die ANSI-Sequenz `ESC[{n};1H`, und
        // die zaehlt **ab eins**. `MoveTo` von crossterm zaehlt **ab
        // null**. Dieselbe Zahl in beide gegeben ergibt zwei
        // verschiedene Zeilen, und der Unterschied ist genau eine.
        //
        // Der Inhalt der Eingabezeile wird oben mit
        // `sch.zeile(KASTEN + 1)` gesetzt. `wagenzeile` nimmt denselben
        // Versatz und rechnet ihn um, damit die Umrechnung an **einer**
        // Stelle steht und nicht hier noch einmal von Hand.
        //
        // ⚑ **Der Fehler war unsichtbar, solange niemand hinsah.** Der
        // Text stand richtig, nur der blinkende Wagen sass darunter, in
        // der Kantenzeile. Gemeldet vom Projektinhaber, nicht von einer
        // Pruefung: Ein Test, der Steuersequenzen liest, sieht die
        // Zeile, in der etwas steht, und nicht die, in der es blinkt.
        MoveTo(
            r.wagenspalte() + eingabe::wagen_hinter(text, r.textbreite()) as u16,
            sch.wagenzeile(Schirm::KASTEN + 1),
        ),
    );
    let _ = aus.flush();
}

/// **Was zwischen den beiden Strichen steht**, genau `innen` Zeichen
/// breit.
///
/// 📌 **Genau, und das ist der Punkt.** Ein Zeichen zu wenig, und die
/// rechte Kante der Eingabezeile steht eine Spalte links von der Kante
/// darueber; ein Zeichen zu viel, und sie bricht um. Gefunden beim
/// ersten Blick auf den fertigen Rahmen: Das schliessende Leerzeichen
/// fehlte.
fn eingabeinhalt(innen: usize, text: &str) -> String {
    let breite = innen.saturating_sub(2);
    let sicht = eingabe::sichtbar(text, breite);
    let luft = breite.saturating_sub(sicht.chars().count());
    format!(" {sicht}{} ", " ".repeat(luft))
}

/// Was unter der Eingabe steht: welches Modell gewaehlt ist.
///
/// ⚑ **Der Name und nicht der Pfad.** Gefragt war, welches Modell
/// gewaehlt ist; ein Pfad, der von vorne abgeschnitten in eine Zeile
/// passt, beantwortet das schlechter als das Wort `myelith-4b`. Wo es
/// liegt, sagt `/settings`.
///
/// ⚑ **Und welche Werkzeugkiste**, denn die entscheidet, was der Agent
/// ueberhaupt kann, und steht sonst nirgends.
fn fusszeile(modus: &str, name: &str, kiste: &str, schreibt: bool, kontext: Option<usize>, breite: usize) -> String {
    let name = if name.is_empty() { "kein Modell" } else { name };
    // ⚑ **Der Modus ganz links** (Festlegung des Projektinhabers,
    // 2026-09-11), mit dem Zeichen fuer Umschalt-Tab davor: **Wer den
    // Schalter nicht kennt, findet ihn dort, wo sein Ergebnis steht.**
    //
    // ⚑ **In dieser Reihenfolge wird auch weggelassen**, wenn das
    // Fenster schmal wird: erst die Werkzeugkiste, dann der
    // Modellname. **Ein abgeschnittenes Wort sagt weniger als ein
    // weggelassenes**, denn `Werkzeuge: …` sieht aus wie eine Angabe
    // und ist keine.
    //
    // ⚑ `nur lesen` steht bei den Werkzeugen, denn es ist eine Angabe
    // **ueber** die Werkzeugkiste.
    let lesen = if schreibt { "" } else { ", nur lesen" };
    // ⚑ **Der Kontext zuletzt**, und damit als Erstes weggelassen: Er ist
    // die Angabe, die man mit `/context` ausfuehrlich bekommt.
    let mut teile =
        vec![format!("⇧⇥ {modus}"), name.to_string(), format!("Werkzeuge: {kiste}{lesen}")];
    if let Some(p) = kontext {
        teile.push(format!("Kontext {p} %"));
    }

    let mut zeile = String::new();
    for teil in teile {
        let versuch = if zeile.is_empty() { teil } else { format!("{zeile} · {teil}") };
        if versuch.chars().count() > breite {
            break;
        }
        zeile = versuch;
    }
    zeile
}

/// **Die zweite Zeile darunter: der Arbeitsordner, und rechts der
/// Befehl fuer die Hilfe.**
///
/// 📌 **Der Hinweis stand bis zum 2026-09-11 in der ersten Zeile** und
/// fiel dort weg, sobald `nur lesen` dazukam: Zwei Angaben, die um
/// denselben Platz streiten, verlieren abwechselnd. **Hier ist Platz**,
/// denn ein Pfad, der ohnehin gekuerzt wird, braucht nicht die ganze
/// Breite.
fn ordnerzeile(ordner: &str, breite: usize) -> String {
    let rechts = format!("{HILFE_BEFEHL} zeigt alle Befehle");
    if rechts.chars().count() + 4 >= breite {
        return von_hinten(ordner, breite);
    }
    let platz = breite - rechts.chars().count() - 2;
    let links = von_hinten(ordner, platz);
    let luft = breite - links.chars().count() - rechts.chars().count();
    format!("{links}{}{rechts}", " ".repeat(luft))
}

/// Die Eingabeschleife.
impl Stand {
    /// Der Arbeitsordner, wie er in die Kante passt: `~` statt des
    /// Benutzerverzeichnisses.
    fn kurzer_ordner(&self) -> String {
        let voll = self.ordner.display().to_string();
        match std::env::var("HOME") {
            Ok(h) if !h.is_empty() && voll.starts_with(&h) => format!("~{}", &voll[h.len()..]),
            _ => voll,
        }
    }
}

fn schleife(stand: &mut Stand) -> i32 {
    loop {
        // ⚑ **Vor jeder Eingabe neu gemessen.** Wer das Fenster zieht,
        // verschiebt den unteren Rand; ein Rahmen, der auf der alten
        // Hoehe stehenbleibt, steht mitten im Text.
        // ⚑ Die Schreiberlaubnis wird vor jeder Eingabe neu gelesen:
        // Wer sie nebenan mit `myl setzen` umlegt, sieht es hier.
        if let Ok(e) = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
            stand.schreibt = e.agent.schreiben;
        }
        let jetzt = Schirm::messen();
        if jetzt != stand.schirm {
            stand.schirm = jetzt;
            if let Some(sch) = jetzt {
                sch.grenze_setzen();
            }
        }
        eingaberahmen(stand);
        let r = Rahmen::messen();
        let schirm = stand.schirm;
        let toene = design::toene(stand.design);
        let zeichnen = |text: &str| {
            if let Some(sch) = schirm {
                zeile_zeichnen(&sch, &r, toene, text);
            }
        };
        let gelesen = eingabe::lesen(&zeichnen);
        // 📌 **Die abgeschickte Zeile wird aus dem Kasten geraeumt.**
        // Sie blieb dort stehen, waehrend der Agent lief, und es sah
        // aus, als waere nichts abgeschickt worden. Gemeldet vom
        // Projektinhaber am 2026-09-11.
        zeichnen("");
        // Zurueck in den Rollbereich, sonst schreibt die Antwort in den
        // Rahmen.
        if let Some(sch) = stand.schirm {
            sch.zurueck();
        }
        let zeile = match gelesen {
            eingabe::Eingabe::Zeile(z) => z,
            // ⚑ **Umschalt-Tab wechselt den Modus und sonst nichts.**
            // Die naechste Runde zeichnet den Rahmen neu, und in der
            // Fusszeile steht der neue Name.
            eingabe::Eingabe::Modus => {
                stand.modus = stand.modus.andere();
                continue;
            }
            eingabe::Eingabe::Ende => {
                // Eingabeende, also Strg-D oder Strg-C: das ist ein
                // ausdruecklicher Abschied und kein Fehler. **Hier wird
                // nicht gefragt**, sonst waere Strg-C abgefangen.
                println!();
                return GUT;
            }
            eingabe::Eingabe::Abbruch(ausgang) => {
                // ⚑ **Escape fragt nach** (Auftrag des Projektinhabers,
                // 2026-09-15): An dieser Stelle laeuft ein geladenes
                // Modell, und die Taste liegt neben den Pfeiltasten.
                // Ein `n` fuehrt zurueck in die Sitzung, die Zeile ist
                // ohnehin leer.
                println!();
                // ⚑ Die Vorgabe haengt an der Taste: Escape bleibt,
                // Strg-X beendet.
                if wirklich_beenden(ausgang == eingabe::Ausgang::StrgX) {
                    return GUT;
                }
                continue;
            }
        };
        let text = zeile.trim().to_string();
        if text.is_empty() {
            continue;
        }

        // ⚑ **Und sie steht in der Zeitleiste**, dort, wo auch die
        // Antwort steht. **Ein Gespraech, in dem nur eine Seite
        // dasteht, laesst sich hinterher nicht lesen.**
        let mut aus = std::io::stdout();
        let _ = crossterm::queue!(
            aus,
            SetForegroundColor(toene.akzent),
            // ⚑ Am linken Rand wie die Werkzeugzeilen: Die Zeitleiste
            // ist ein Gespraech und braucht **eine** Kante.
            Print(format!("  ❯ {text}\n")),
            ResetColor
        );
        let _ = aus.flush();

        match befehl_zu(&text) {
            Some(Befehlsart::Ende) => return GUT,
            Some(Befehlsart::Hilfe) => {
                hilfe();
                continue;
            }
            Some(Befehlsart::Datei) => {
                let pfad = befehl_und_rest(&text).map(|(_, r)| r).unwrap_or("");
                datei_anhaengen(stand, pfad);
                continue;
            }
            Some(Befehlsart::Modell) => {
                // ⚑ **Hier greift die Sicherung** (Auftrag des
                // Projektinhabers, 2026-09-15): An dieser Stelle laeuft
                // bereits ein Modell, und ein Esc wuerde eine Sitzung
                // samt geladenem Artefakt wegwerfen. Gefragt wird mit
                // j/n, und **n fuehrt zurueck zur Wahl** statt in eine
                // Sitzung ohne Modell.
                //
                // ⚑ **Erst das alte weg.** Zwei Artefakte passen nicht
                // nebeneinander in diesen Speicher; deshalb ist der Weg
                // zurueck eine neue Wahl und kein Zurueckholen.
                stand.modell = None;
                loop {
                    match modell_waehlen(stand) {
                        Modellwahl::Geladen => break,
                        Modellwahl::Abgebrochen => {
                            // Escape in der Auswahl: Vorgabe ist bleiben.
                            if wirklich_beenden(false) {
                                return GUT;
                            }
                        }
                        // 📌 **Hier stand „Das Modell bleibt, wie es
                        // war."** Das stimmte nicht: Eine Zeile darueber
                        // ist es entladen worden. Ein Satz, der das
                        // Gegenteil dessen sagt, was geschehen ist, ist
                        // schlimmer als keiner.
                        Modellwahl::Unmoeglich => {
                            eprintln!("  Kein Modell geladen. Mit /model eines waehlen.");
                            break;
                        }
                    }
                }
                continue;
            }
            Some(Befehlsart::Werkzeugkiste) => {
                // ⚑ **Gesetzt wird in `einstellseite`**, nicht hier:
                // Diese Datei zeigt Einstellungen und aendert keine.
                match einstellseite::werkzeugkiste_waehlen(design::toene(stand.design)) {
                    Some(name) => {
                        stand.kiste = name;
                        println!("  Werkzeugkiste: {}", stand.kiste);
                    }
                    None => println!("  Die Werkzeugkiste bleibt, wie sie war."),
                }
                println!();
                continue;
            }
            Some(Befehlsart::Einstellungen) => {
                einstellungen_zeigen(stand);
                continue;
            }
            Some(Befehlsart::Kontext) => {
                kontext_zeigen(stand);
                continue;
            }
            Some(Befehlsart::Verdichten) => {
                gespraech_verdichten(stand);
                continue;
            }
            Some(Befehlsart::Neu) => {
                stand.gespraech.leeren();
                stand.kontext_prozent = None;
                println!("  Ein neues Gespraech beginnt; das bisherige ist vergessen.");
                println!();
                continue;
            }
            None => {}
        }

        if text.starts_with('/') {
            println!("Unbekannter Befehl. `{HILFE_BEFEHL}` zeigt, was es gibt.");
            continue;
        }

        auftrag_fahren(stand, &text);
    }
}

/// Der Befehl, der alle anderen aufzaehlt.
///
/// ⚑ Er steht in der Fusszeile unter der Eingabe: **Wer nicht weiss,
/// was es gibt, soll nicht raten muessen.**
const HILFE_BEFEHL: &str = "/help";

/// Was ein Befehl bewirkt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Befehlsart {
    /// Eine Datei anhaengen; nimmt einen Pfad.
    Datei,
    Modell,
    Werkzeugkiste,
    Einstellungen,
    Kontext,
    Verdichten,
    Neu,
    Hilfe,
    Ende,
}

/// Ein Befehl mit seinen Schreibweisen und seinem Satz.
struct Befehl {
    art: Befehlsart,
    /// Die erste ist die, die in der Hilfe steht.
    namen: &'static [&'static str],
    was: &'static str,
}

/// **Die einzige Liste der Befehle.**
///
/// 📌 **Dieselbe Klasse wie Fund 271.** Bis zum 2026-09-11 stand jeder
/// Befehl zweimal da: einmal im `match`, das ihn ausfuehrt, und einmal
/// in der Hilfe, die ihn nennt. Eine Hilfe, die von Hand gefuehrt wird,
/// nennt irgendwann einen Befehl, den es nicht gibt, oder verschweigt
/// einen, den es gibt, **und beides sieht erst der, der es
/// ausprobiert.**
const BEFEHLE: [Befehl; 9] = [
    Befehl {
        art: Befehlsart::Modell,
        namen: &["/model", "/modell"],
        was: "ein anderes Modell waehlen und laden",
    },
    Befehl {
        art: Befehlsart::Werkzeugkiste,
        namen: &["/toolkit", "/werkzeugkiste"],
        was: "eine andere Werkzeugkiste waehlen",
    },
    Befehl {
        art: Befehlsart::Einstellungen,
        namen: &["/settings", "/einstellungen"],
        was: "die Einstellungen zeigen",
    },
    Befehl {
        art: Befehlsart::Datei,
        namen: &["/file", "/datei"],
        was: "haengt eine Datei an: /datei <pfad>",
    },
    Befehl {
        art: Befehlsart::Kontext,
        namen: &["/context", "/kontext"],
        was: "zeigt, wie viel Kontext das Gespraech belegt",
    },
    Befehl {
        art: Befehlsart::Verdichten,
        namen: &["/compress", "/verdichten"],
        was: "fasst das bisherige Gespraech zusammen und schafft Platz",
    },
    Befehl {
        art: Befehlsart::Neu,
        namen: &["/clear", "/neu"],
        was: "beginnt ein neues Gespraech",
    },
    Befehl {
        art: Befehlsart::Hilfe,
        namen: &[HILFE_BEFEHL, "/hilfe"],
        was: "alle Befehle, mit einem Satz dazu",
    },
    Befehl {
        art: Befehlsart::Ende,
        namen: &["/exit", "/ende", "/quit"],
        was: "Schluss (oder Escape und Strg-X, beide mit Rueckfrage)",
    },
];

/// **Haengt eine Datei an das Gespraech**, ohne sie hineinzuschreiben.
///
/// ⚑ **Die Datei wandert unter die Einhaengung**, und das Gespraech
/// bekommt eine Zeile, die sagt, wo sie liegt. Lesen tut sie das Modell
/// mit den Werkzeugen, die es ohnehin hat; **was in den Kontext geht,
/// bleibt damit eine Entscheidung des Modells und nicht eine Folge des
/// Anhaengens.**
fn datei_anhaengen(stand: &mut Stand, pfad: &str) {
    if pfad.is_empty() {
        println!("  /datei braucht einen Pfad, zum Beispiel: /datei bericht.md");
        return;
    }
    // ⚑ Eine Tilde ist ueblich und waere sonst ein Ordnername.
    let ausgeschrieben = if let Some(rest) = pfad.strip_prefix("~/") {
        std::env::var("HOME").map(|h| format!("{h}/{rest}")).unwrap_or_else(|_| pfad.to_string())
    } else {
        pfad.to_string()
    };
    match myl_client::anhang::aufnehmen(&stand.ordner, std::path::Path::new(&ausgeschrieben)) {
        Ok(a) => {
            println!("  Angehaengt: {} ({}), liegt unter {}", a.name, a.art.wort(true), a.pfad);
            let datei = stand.ordner.join(&a.pfad);

            // ⚑ **Hier wird angesehen, und nur hier** (Festlegung des
            // Projektinhabers, 2026-09-17): Der Chat wertet **nur**
            // aus, was der Nutzer selbst angehaengt hat. Er sieht in
            // keinen Arbeitsordner und laedt nichts nach; er hat auch
            // gar keine Werkzeugschleife, in der er das koennte.
            //
            // ⚑ **Die schnelle Sprosse.** Das ist ein Blick fuer jeden,
            // auch fuer den, der gar nichts fragen wollte; die genaue
            // nimmt der Agent, wenn er ausdruecklich fragt.
            let sinne = myl_senses::Sinne::finden();
            let gesehen = myl_senses::auswerten(
                &sinne,
                &datei,
                a.art,
                None,
                myl_senses::Stufe::Schnell,
            );
            // ⚠️ **Ein Mangel geht an den Menschen, nicht in den
            // Kontext.** Eine Einrichtungsanleitung im Gespraech kostet
            // Kontext und hilft dem Modell nicht; der Mensch dagegen
            // kann sie befolgen.
            if let Some(Err(grund)) = &gesehen {
                for z in grund.lines() {
                    println!("  {z}");
                }
            }
            // ⚑ **Der Name lebt so lange wie die Zeile, die ihn nennt.**
            let ersatz = match &gesehen {
                Some(Ok(_)) => None,
                _ => werkzeug_fuer_art(a.art),
            };
            let sicht = match (&gesehen, &ersatz) {
                (Some(Ok(text)), _) => {
                    println!("  Angesehen von einem anderen Modell.");
                    myl_client::anhang::Sicht::Angesehen(text)
                }
                (_, Some(n)) => myl_client::anhang::Sicht::Werkzeug(n),
                _ => myl_client::anhang::Sicht::Nichts,
            };
            stand.gespraech.nach_dem_lauf(&[myl_client::Nachricht::nutzer(
                a.nachricht_mit(true, sicht),
            )]);
        }
        Err(f) => println!("  {f}"),
    }
}

/// **Gibt es ein Manifest-Werkzeug fuer diese Art?**
///
/// ⚑ **Der Rueckfall, wenn kein Sinn eingerichtet ist.** Bild und Ton
/// macht der Client selbst; wer etwas anderes lesbar machen will, legt
/// ein Manifest mit dem Feld `fuer` in seine Werkzeugkiste, und dann
/// wird es hier genannt.
fn werkzeug_fuer_art(art: myl_client::anhang::Art) -> Option<String> {
    myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
        .ok()
        .and_then(|e| myl_client::kisten::werkzeug_fuer(&e.agent, art.kennung()))
}

/// Welcher Befehl das ist, falls es einer ist, und was dahinter steht.
///
/// ⚑ **Das erste Wort entscheidet.** Bis `/datei` dazukam, war ein
/// Befehl die ganze Zeile; jetzt gibt es einen mit Argument, und **ein
/// Befehl, der nur ohne Argument erkannt wird, sieht fuer den Nutzer wie
/// ein Tippfehler aus**: Er tippt `/datei bericht.md` und bekommt seine
/// Zeile als Auftrag an das Modell.
fn befehl_zu(text: &str) -> Option<Befehlsart> {
    befehl_und_rest(text).map(|(art, _)| art)
}

fn befehl_und_rest(text: &str) -> Option<(Befehlsart, &str)> {
    let geschnitten = text.trim();
    let (erstes, rest) = geschnitten.split_once(char::is_whitespace).unwrap_or((geschnitten, ""));
    BEFEHLE
        .iter()
        .find(|b| b.namen.contains(&erstes))
        .map(|b| (b.art, rest.trim()))
}

/// **Alle Befehle mit einem Satz dazu**, aus der einen Liste.
fn hilfezeilen() -> Vec<String> {
    BEFEHLE
        .iter()
        .map(|b| {
            // Die weiteren Schreibweisen stehen dahinter: Wer `/hilfe`
            // tippt, soll nicht denken, er habe sich geirrt.
            let weitere = if b.namen.len() > 1 {
                format!("  (auch {})", b.namen[1..].join(", "))
            } else {
                String::new()
            };
            format!("  {:<12}{}{weitere}", b.namen[0], b.was)
        })
        .collect()
}

fn hilfe() {
    println!();
    for z in hilfezeilen() {
        println!("{z}");
    }
    println!();
    println!("  ⇧⇥ wechselt zwischen auto mode und manual mode.");
    println!(
        "  Im manual mode wird jede schreibende Handlung vorgelegt; Lesen fragt nie."
    );
    println!("  {} zeigt waehrend eines Auftrags, was gerade laeuft.", anzeige::SCHALTER);
    println!("  Alles andere ist ein Auftrag an den Agenten. Das Gespraech geht");
    println!("  von Auftrag zu Auftrag mit; wird der Kontext voll, verdichtet der");
    println!("  Agent es selbst.");
    println!();
}

/// **Die Einstellungsseite, mit den Pfeiltasten bedienbar.**
///
/// 📌 **Bis zum 2026-09-11 zeigte sie nur an.** Die Begruendung war, ein
/// zweiter Setzer waere die dritte Stelle, die dieselben Feinheiten
/// kennt. **Sie galt dem Setzer und nicht der Bedienung:** Gesetzt wird
/// weiterhin ausschliesslich mit `Einstellungen::setzen`, und was `aus`
/// bedeutet, steht unveraendert an der einen Stelle in der Kiste.
///
/// ⚑ **Sie nimmt den ganzen Schirm.** Dafuer wird der Rollbereich
/// aufgeloest und danach neu gesetzt: Eine Seite, die sich in vier
/// Zeilen ueber dem Kasten draengte, waere keine Seite.
fn einstellungen_zeigen(stand: &mut Stand) {
    let t = design::toene(stand.design);
    if let Some(sch) = stand.schirm {
        sch.aufloesen();
        schirm_leeren();
    }
    let geaendert = einstellseite::fahren(t, &stand.ordner);
    if let Some(sch) = stand.schirm {
        // ⚑ **Zurueck ins Gespraech heisst zurueck unter das Logo**
        // (Auftrag des Projektinhabers, 2026-09-12). Hier stand ein
        // blosses Leeren, und das Gespraech ging auf einem leeren
        // Schirm weiter, waehrend es nach der Modellwahl unter dem
        // Schriftzug begann. **Zwei Wege in dasselbe Bild duerfen nicht
        // verschieden aussehen.**
        banner::bildschirm_mit(farben::logo());
        sch.einrichten();
    }
    // ⚑ **Was hier geaendert wurde, gilt sofort.** Eine Einstellung,
    // die erst beim naechsten Start wirkt, laesst jemanden zweimal
    // dasselbe tun.
    if geaendert {
        if let Ok(e) = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
            // ⚑ **Auch die Freigabe gilt sofort**, wie im Fenster: Wer
            // den Kernregler bewegt und danach dieselbe Laufzeit sieht,
            // hat keinen Regler bedient, sondern eine Zahl geaendert.
            myl_client::hardware::anwenden(&e);
            stand.schreibt = e.agent.schreiben;
            stand.modus = e.agent.modus;
            stand.design = e.oberflaeche.design;
            if !stand.artefakt.is_empty() {
                let kiste = myl_client::kisten::kiste_der_gilt(&e.agent);
                stand.kiste = kiste.name().to_string();
            }
        }
    }
}

/// **Die Werkzeugansage, wie der naechste Lauf sie haette.**
///
/// ⚑ Nach einem Lauf steht sie schon im Stand; davor wird sie aus den
/// Einstellungen gebaut, auf demselben Weg wie in `auftrag_fahren`.
fn ansage_fuer(stand: &Stand) -> Option<myl_client::Nachricht> {
    if let Some(a) = &stand.ansage {
        return Some(a.clone());
    }
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()).ok()?;
    let mut agent = e.agent.clone();
    agent.wurzel = Some(stand.ordner.display().to_string());
    let kiste = myl_client::kisten::kiste_der_gilt(&e.agent);
    let r = myl_client::ruestung::ruesten_mit(&agent, myl_client::Ansageform::Amtlich, kiste, Vec::new(), None).ok()?;
    Some(myl_client::gespraech::ansage(&r))
}

/// **`/context`: der Balken und woraus er besteht.**
fn kontext_zeigen(stand: &mut Stand) {
    let Some(modell) = stand.modell.as_ref() else {
        eprintln!("Es ist kein Modell geladen. `/model` waehlt eines.");
        return;
    };
    let ansage = ansage_fuer(stand);
    match myl_client::gespraech::anzeige(modell, ansage.as_ref(), &stand.gespraech) {
        Some(a) => {
            stand.kontext_prozent = Some(a.prozent);
            println!();
            for z in kontextzeilen(&a, Rahmen::messen().innen) {
                println!("{z}");
            }
            println!();
        }
        None => println!("  Dieses Modell sagt nicht, wie viel Kontext es hat."),
    }
}

/// Die Zeilen von `/context`, ohne Farbe und damit pruefbar.
fn kontextzeilen(a: &myl_client::gespraech::Kontextanzeige, breite: usize) -> Vec<String> {
    let balken = myl_client::gespraech::balken(a.belegt, a.grenze, breite.saturating_sub(24).clamp(10, 40));
    vec![
        format!("  Kontext  {balken}  {} %", a.prozent),
        format!("           {} von {} Token", a.belegt, a.grenze),
        format!(
            "           Werkzeugansage {} · Gespraech {} Token in {} Nachrichten",
            a.ansage,
            a.belegt.saturating_sub(a.ansage),
            a.nachrichten
        ),
        "           /compress fasst das Gespraech zusammen, /clear beginnt ein neues.".to_string(),
    ]
}

/// **`/compress`: das Gespraech zusammenfassen, mit dem Modell selbst.**
fn gespraech_verdichten(stand: &mut Stand) {
    let Some(modell) = stand.modell.as_ref() else {
        eprintln!("Es ist kein Modell geladen. `/model` waehlt eines.");
        return;
    };
    if stand.gespraech.ist_leer() {
        println!("  Das Gespraech ist leer; es gibt nichts zu verdichten.");
        return;
    }
    print!("  Das Gespraech wird verdichtet … ");
    let _ = std::io::stdout().flush();
    let anfang = std::time::Instant::now();
    // ⚑ **Der Mitschnitt entsteht genau hier** (2026-09-16): Die
    // Urfassung ist im Augenblick des Verdichtens noch da und danach
    // weg. Die Konsole kennt ihren Arbeitsordner, also bekommt sie
    // einen; ohne Ordner gaebe es keinen Platz, an dem der Agent
    // nachlesen koennte.
    match myl_client::gespraech::verdichten_mit_mitschnitt(
        modell,
        &mut stand.gespraech,
        Some(&stand.ordner),
        &stand.sitzung,
        &stand.name,
    ) {
        Ok((vorher, nachher)) => {
            println!("{:.1} s", anfang.elapsed().as_secs_f64());
            println!("  Verdichtet: {vorher} → {nachher} Token.");
            let ansage = ansage_fuer(stand);
            stand.kontext_prozent =
                stand.modell.as_ref().and_then(|m| myl_client::gespraech::anzeige(m, ansage.as_ref(), &stand.gespraech)).map(|a| a.prozent);
        }
        Err(f) => {
            println!();
            eprintln!("  Das Gespraech liess sich nicht verdichten: {f}");
        }
    }
    println!();
}

/// Ein Auftrag, von der Eingabe bis zur Antwort.
fn auftrag_fahren(stand: &mut Stand, auftrag: &str) {
    let verlauf = stand.gespraech.nachrichten().to_vec();
    let Some(modell) = stand.modell.as_mut() else {
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

    // ⚑ **Die Kiste folgt dem geladenen Modell**, sofern der Nutzer
    // nichts anderes eingestellt hat. Gerechnet wird das in der Kiste,
    // nicht hier: Fenster und Konsole stellen dieselbe Frage.
    let kiste = myl_client::kisten::kiste_der_gilt(&e.agent);


    // ⚑ **Was geschieht, steht da, waehrend es geschieht.** Ein
    // Agentenlauf dauert Minuten; ohne Anzeige sieht das aus wie ein
    // haengendes Programm.
    //
    // ⚑ **Eine Zeile, nicht ein Protokoll** (Festlegung des
    // Projektinhabers, 2026-09-11). Was gerade laeuft, steht in der
    // Klammer; die Werkzeugaufrufe holt sich, wer sie sehen will, mit
    // dem Schalter.
    //
    // 📌 **Die laufende Zeile gibt es nur vor einem Terminal.** Der
    // erste Entwurf schrieb sie immer und loeschte sie mit `\r` und
    // Leerzeichen. In einer Roehre gibt es keinen Wagenruecklauf: Die
    // Leerzeichen stehen dann einfach da. **Was den Bildschirm
    // zurueckspult, setzt einen Bildschirm voraus.**
    //
    // ⚠️ **Rohmodus, solange der Lauf laeuft.** Ohne ihn kaeme eine
    // Taste erst mit der Eingabetaste an, und ein Schalter, der eine
    // Eingabetaste braucht, ist keiner. Der Waechter nimmt ihn beim
    // Verlassen zurueck, auch wenn dazwischen etwas schiefgeht.
    let roh = stand.schirm.and_then(|_| auswahl::Rohmodus::an().ok());
    // ⚑ **Der Zaehler faengt bei jedem Auftrag neu an.** Was dieser
    // Auftrag kostet, ist die Frage; was die Sitzung bisher gekostet
    // hat, waere eine andere und stuende an derselben Stelle.
    modell.zaehler.zuruecksetzen();
    let zaehler = std::sync::Arc::clone(&modell.zaehler);
    let anzeige = anzeige::Anzeige::starten(stand.schirm, stand.design, zaehler);

    // ⚑ **Im `manual mode` bekommt jede schreibende Handlung eine
    // Nachfrage mit auf den Weg.** Sie haengt am Werkzeug und nicht am
    // Melder: Ein Melder darf berichten, und nur wer ausfuehrt, kann
    // etwas verhindern.
    let nachfrage: Option<myl_client::ruestung::Nachfrage> = if stand.modus.fragt_nach() {
        let frager = anzeige.frager();
        Some(std::sync::Arc::new(move |name: &str, a: &myl_client::serde_json::Value| {
            frager.fragen(&format!("  ⚑ manual mode: {name} {}", myl_client::lauf::kurzform(a)))
        }))
    } else {
        None
    };

    // ⚑ **Der Tokenstrom geht an die Anzeige** (Auftrag des
    // Projektinhabers, 2026-09-11). Ohne ihn sieht ein Denkvorgang von
    // einer Minute aus wie Stillstand; mit ihm steht da, woran das
    // Modell gerade ist. ⚠️ Gezeigt wird er nur in der ausfuehrlichen
    // Anzeige: **Wer nur die Antwort will, will nicht den Rohstrom.**
    let strom = anzeige.strom();
    modell.beobachter = Some(Box::new(move |s| strom.stueck(s)));

    let ruestung = match myl_client::ruestung::ruesten_mit(
        &agent,
        myl_client::Ansageform::Amtlich,
        kiste,
        Vec::new(),
        nachfrage,
    ) {
        Ok(r) => r,
        Err(f) => {
            anzeige.beenden();
            drop(roh);
            eprintln!("Die Werkzeuge haengen nicht: {f}");
            return;
        }
    };
    // ⚑ **Jede Meldung in zwei Laengen.** Kurz steht in der Zeitleiste,
    // ausfuehrlich, sobald jemand den Schalter drueckt; gedruckt wird
    // **immer**, damit sich der Lauf hinterher der Reihe nach lesen
    // laesst.
    let melder = |m: myl_client::Meldung<'_>| match m {
        myl_client::Meldung::Schritt(n) => anzeige.schritt(n),
        myl_client::Meldung::Aufruf { name, argumente } => {
            let voll = myl_client::serde_json::to_string(argumente)
                .unwrap_or_else(|_| myl_client::lauf::kurzform(argumente));
            anzeige.aufruf(name, &myl_client::lauf::kurzform(argumente), &voll);
        }
        myl_client::Meldung::Ergebnis { name, text } => {
            // ⚠️ Auch die ausfuehrliche Form hat eine Grenze: Ein
            // Werkzeug, das zwanzigtausend Zeichen zurueckgibt,
            // schriebe sonst den ganzen Verlauf voll.
            anzeige.ergebnis(
                name,
                &myl_client::lauf::eine_zeile(text, 60),
                &myl_client::lauf::eine_zeile(text, 2000),
            );
        }
        myl_client::Meldung::Abgelehnt { name, grund } => anzeige.abgelehnt(name, grund),
        myl_client::Meldung::Verdichtet { vorher, nachher } => anzeige.verdichtet(vorher, nachher),
    };

    // ⚑ **Jeder Auftrag bekommt sein Nachschlagebudget neu** (2026-09-17):
    // Die naechste Frage des Nutzers ist ein neuer Anlass nachzulesen.
    ruestung.nachschlagebudget_zuruecksetzen();
    let aus = myl_client::lauf::fahren_im_gespraech(
        modell,
        &ruestung,
        e.agent.schritte as usize,
        // ⚑ **Das eingehaengte Verzeichnis ist die Zustimmung.** Bis
        // zum 2026-09-11 stand hier ein zweiter Schalter, ohne den die
        // Dateiwerkzeuge in der Ansage standen und nicht liefen.
        true,
        e.modell.token as u32,
        &verlauf,
        auftrag,
        Some(&melder),
    );

    // 📌 **Der Zuschauer geht wieder weg.** Er haelt Zaehler dieses
    // Laufs; bliebe er stehen, schriebe der naechste Lauf in die Lage
    // des vorigen. Dieselbe Stelle wie im Fenster.
    if let Some(m) = stand.modell.as_mut() {
        m.beobachter = None;
    }
    let (gesehen, wieviele) = anzeige.beenden();
    // Erst den Rohmodus zurueck, dann drucken: In ihm braucht jede
    // Zeile ein `\r`, und das will niemand in jedem `println!` stehen
    // haben.
    drop(roh);

    match aus.antwort.as_deref() {
        Some(a) => println!("{a}"),
        None => println!("(keine Schlussantwort)"),
    }
    println!();
    // ⚑ **Wer die Aufrufe nicht gesehen hat, erfaehrt wenigstens, dass
    // es welche gab**, und womit er sie beim naechsten Mal sieht.
    if wieviele > 0 && !gesehen {
        println!("  {} zeigt die Werkzeugzeilen ausfuehrlich.", anzeige::SCHALTER);
    }
    // ⚑ **Das Gespraech geht mit**, samt einer Verdichtung, falls der Lauf
    // eine brauchte.
    stand.gespraech.nach_dem_lauf(&aus.nachrichten);
    stand.ansage = aus.nachrichten.first().filter(|n| n.role == "system").cloned();
    let kontext = stand
        .modell
        .as_ref()
        .and_then(|m| myl_client::gespraech::anzeige(m, stand.ansage.as_ref(), &stand.gespraech));
    stand.kontext_prozent = kontext.map(|a| a.prozent);
    let kontextangabe = kontext.map(|a| format!(" · Kontext {} %", a.prozent)).unwrap_or_default();
    println!(
        "  {} Schritte, {:.1} s{kontextangabe}",
        aus.verlauf.len(),
        aus.sekunden
    );
    if matches!(aus.ende, myl_client::Ende::Tuer(myl_client::Tuerfehler::KontextVoll { .. })) {
        println!("  Der Kontext ist voll. `/compress` fasst das Gespraech zusammen, `/clear` beginnt neu.");
    }
    println!();
}

/// Ein Pfad, der in eine Zeile passt.
///
/// ⚑ **Gekuerzt wird vorne**, denn hinten steht der Name, und der
/// unterscheidet.
/// **Der hintere Teil eines Pfades, mit `…` davor.**
///
/// ⚑ **Das Ende und nicht der Anfang.** `/Users/x/Desktop/Code/…/ui`
/// sagt weniger als `…/Code/Projekt/ui`: Was einen Ordner
/// unterscheidet, steht hinten.
fn von_hinten(text: &str, breite: usize) -> String {
    let n = text.chars().count();
    if n <= breite {
        return text.to_string();
    }
    let hinten: String = text.chars().skip(n - breite + 1).collect();
    format!("…{hinten}")
}

fn kurz(pfad: &str) -> String {
    let breite = banner::fensterbreite().saturating_sub(24).max(24) as usize;
    if pfad.chars().count() <= breite {
        return pfad.to_string();
    }
    let hinten: String = pfad.chars().skip(pfad.chars().count() - breite + 1).collect();
    format!("…{hinten}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe() -> Rahmen {
        Rahmen { einzug: "    ".to_string(), innen: 40 }
    }

    /// **Alle drei Kanten sind gleich breit und gleich weit eingerueckt.**
    ///
    /// 📌 Der Gegenbeweis: Wer `unten()` aus einer zweiten Rechnung holt
    /// statt aus derselben Messung, sieht hier sofort einen Unterschied.
    #[test]
    fn der_rahmen_ist_an_allen_kanten_gleich() {
        let r = probe();
        let breiten: Vec<usize> =
            [r.oben(), r.leer(), r.unten()].iter().map(|z| z.chars().count()).collect();
        assert_eq!(breiten[0], breiten[1], "obere Kante und Eingabezeile sind ungleich breit");
        assert_eq!(breiten[1], breiten[2], "Eingabezeile und untere Kante sind ungleich breit");
        for zeile in [r.oben(), r.leer(), r.unten()] {
            assert!(
                zeile.starts_with(&r.einzug),
                "eine Kante steht nicht im Einzug der anderen: {zeile:?}"
            );
        }
    }

    /// **Die Eingabezeile ist genau so breit wie die Kanten.**
    ///
    /// 📌 Die Gegenprobe zu einem Fehler, der beim Lesen unsichtbar ist
    /// und beim Hinsehen sofort auffaellt: Fehlte das schliessende
    /// Leerzeichen, stand die rechte Kante der Eingabezeile eine Spalte
    /// links von der Kante darueber.
    #[test]
    fn die_eingabezeile_ist_so_breit_wie_der_rahmen() {
        let r = probe();
        for text in ["", "kurz", "ein etwas laengerer Auftrag", &"x".repeat(200)] {
            let zeile = format!("{}│{}│", r.einzug, eingabeinhalt(r.innen, text));
            assert_eq!(
                zeile.chars().count(),
                r.leer().chars().count(),
                "bei {:?} ist die Zeile {} statt {} Zeichen breit",
                &text[..text.len().min(20)],
                zeile.chars().count(),
                r.leer().chars().count()
            );
        }
    }

    /// **Der Wagen landet im Rahmen und nicht auf der Kante.**
    ///
    /// ⚠️ Ein Wagen einen Schritt zu weit links steht auf dem
    /// senkrechten Strich und schiebt ihn beim ersten Zeichen weg.
    #[test]
    fn der_wagen_landet_im_rahmen() {
        let r = probe();
        let rechts = r.wagenspalte();

        let zeile: Vec<char> = r.leer().chars().collect();
        let strich = r.einzug.chars().count();
        assert_eq!(zeile[strich], '│', "der Einzug endet nicht am senkrechten Strich");
        assert_eq!(zeile[rechts as usize], ' ', "der Wagen steht nicht auf freiem Grund");
        assert!(rechts as usize > strich, "der Wagen steht links vom Rahmen");
        assert!(
            (rechts as usize) < strich + 1 + r.innen,
            "der Wagen steht rechts vom Rahmen"
        );
    }

    /// **Unter der Eingabe steht der Name des Modells, nicht sein Pfad.**
    ///
    /// ⚑ Gefragt war, *welches* Modell gewaehlt ist. Ein von vorne
    /// abgeschnittener Pfad beantwortet das schlechter als ein Name.
    #[test]
    fn die_fusszeile_nennt_modell_und_kiste() {
        let z = fusszeile("auto mode", "myelith-4b", "Base", true, None, crate::schirm::BLOCKBREITE - 2);
        assert!(z.contains("auto mode"), "der Modus fehlt: {z}");
        assert!(z.contains("myelith-4b"), "das Modell fehlt: {z}");
        assert!(z.contains("Base"), "die Werkzeugkiste fehlt: {z}");
        // ⚑ Geprueft wird der Teil **vor** dem Hinweis: Der heisst
        // `/hilfe` und traegt selbst einen Schraegstrich.
        let vorn = z.split(HILFE_BEFEHL).next().unwrap_or_default();
        assert!(!vorn.contains('/'), "in der Fusszeile steht ein Pfad: {z}");
    }

    /// **Der Hinweis auf die Hilfe steht in der zweiten Zeile und
    /// bleibt dort.**
    ///
    /// 📌 In der ersten fiel er weg, sobald `nur lesen` dazukam.
    #[test]
    fn der_hinweis_steht_neben_dem_ordner() {
        let z = ordnerzeile("~/Code/Myelith", crate::schirm::BLOCKBREITE - 2);
        assert!(z.contains(HILFE_BEFEHL), "der Hinweis fehlt: {z}");
        assert!(z.contains("~/Code/Myelith"), "der Ordner fehlt: {z}");
        for breite in 12..90 {
            let z = ordnerzeile("/ein/ziemlich/langer/pfad/zu/einem/ordner", breite);
            assert!(
                z.chars().count() <= breite,
                "bei {breite} ist die Zeile {} lang: {z}",
                z.chars().count()
            );
        }
    }

    /// **Ohne Schreiberlaubnis sagt die Fusszeile es.**
    ///
    /// 📌 **Dieselbe Klasse wie Fund 315.** Ohne `agent.schreiben`
    /// bekommt der Agent `write_file` gar nicht erst und antwortet
    /// „ich kann keine Dateien speichern"; **wer das nicht weiss, sucht
    /// den Fehler beim Modell.** Gemeldet vom Projektinhaber am
    /// 2026-09-11.
    #[test]
    fn ohne_schreiberlaubnis_steht_es_in_der_fusszeile() {
        let mit = fusszeile("auto mode", "Myelith 4B", "Base", true, None, crate::schirm::BLOCKBREITE - 2);
        let ohne = fusszeile("auto mode", "Myelith 4B", "Base", false, None, crate::schirm::BLOCKBREITE - 2);
        assert!(!mit.contains("nur lesen"), "{mit}");
        assert!(ohne.contains("nur lesen"), "{ohne}");
    }

    /// **Und sie bleibt im Rahmen, auch wenn der Name lang ist.**
    ///
    /// 📌 Eine Fusszeile, die breiter ist als der Rahmen darueber, bricht
    /// um und schiebt die naechste Ausgabe eine Zeile tiefer: Dann zeigt
    /// `MoveUp(3)` nicht mehr in den Rahmen, sondern auf seine Kante.
    #[test]
    fn eine_lange_fusszeile_passt_in_den_rahmen() {
        let lang = "myelith-30b-a3b-langer-name-aus-einem-versuch";
        for breite in 8..90 {
            let z = fusszeile("auto mode", lang, "Advanced", true, Some(100), breite);
            assert!(
                z.chars().count() <= breite,
                "bei {breite} Zeichen ist die Fusszeile {} lang: {z}",
                z.chars().count()
            );
        }
    }

    /// **Weggelassen wird von rechts, und der Modus bleibt.**
    ///
    /// 📌 Ein abgeschnittenes `Werkzeuge: …` sieht aus wie eine Angabe
    /// und ist keine. **Was nicht passt, faellt ganz weg.**
    #[test]
    fn im_schmalen_fenster_bleibt_der_modus() {
        let z = fusszeile("manual mode", "myelith-4b", "Advanced", true, Some(7), 20);
        assert!(z.starts_with("⇧⇥ manual mode"), "{z}");
        assert!(!z.contains('…'), "es wurde abgeschnitten statt weggelassen: {z}");
    }

    /// **Ohne Modell steht das da, und nicht eine leere Stelle.**
    #[test]
    fn ohne_modell_sagt_die_fusszeile_das() {
        let z = fusszeile("auto mode", "", "Base", true, None, crate::schirm::BLOCKBREITE - 2);
        assert!(z.contains("kein Modell"), "{z}");
    }

    /// **Der Kontext steht in der Fusszeile, sobald er bekannt ist, und
    /// faellt als Erstes weg.**
    #[test]
    fn der_kontext_steht_zuletzt_in_der_fusszeile() {
        let breit = fusszeile("auto mode", "myelith-4b", "Base", true, Some(31), crate::schirm::BLOCKBREITE - 2);
        assert!(breit.ends_with("Kontext 31 %"), "{breit}");
        let ohne = fusszeile("auto mode", "myelith-4b", "Base", true, None, crate::schirm::BLOCKBREITE - 2);
        assert!(!ohne.contains("Kontext"), "{ohne}");
        let schmal = fusszeile("auto mode", "myelith-4b", "Base", true, Some(31), ohne.chars().count());
        assert_eq!(schmal, ohne, "wird es eng, faellt der Kontext vor allem anderen weg");
    }

    /// **`/context` zeigt Balken, Zahlen und Aufteilung**, und keine Zeile
    /// ist breiter als der Rahmen.
    #[test]
    fn die_kontextanzeige_nennt_balken_und_zahlen() {
        let a = myl_client::gespraech::Kontextanzeige { belegt: 12_345, grenze: 40_960, ansage: 1_234, nachrichten: 14, prozent: 30 };
        let zeilen = kontextzeilen(&a, 90);
        assert!(zeilen[0].contains('█') && zeilen[0].ends_with("30 %"), "{:?}", zeilen[0]);
        assert!(zeilen[1].contains("12345 von 40960 Token"), "{:?}", zeilen[1]);
        assert!(zeilen[2].contains("Werkzeugansage 1234") && zeilen[2].contains("Gespraech 11111 Token in 14"), "{:?}", zeilen[2]);
        for n in ["/compress", "/clear"] {
            assert!(befehl_zu(n).is_some(), "{n} wird genannt und muss es geben");
        }
        for z in &zeilen {
            assert!(z.chars().count() <= 90, "{z}");
        }
    }

    /// **Jeder Befehl wird behandelt, und jeder steht in der Hilfe.**
    ///
    /// 📌 **Dieselbe Klasse wie Fund 271**, und jetzt kann sie nicht
    /// mehr eintreten: Ausfuehrung und Hilfe kommen aus **einer**
    /// Liste. Bis zum 2026-09-11 standen sie getrennt da, und eine
    /// Pruefung zaehlte, ob jedes Wort zweimal im Quelltext vorkommt.
    /// **Eine Pruefung, die Wiederholung verlangt, haelt die
    /// Wiederholung fest.**
    #[test]
    fn jeder_befehl_wird_behandelt_und_steht_in_der_hilfe() {
        let zeilen = hilfezeilen().join("\n");
        for b in BEFEHLE.iter() {
            assert!(!b.namen.is_empty(), "ein Befehl ohne Namen");
            assert!(!b.was.is_empty(), "{} ohne Satz dazu", b.namen[0]);
            assert!(zeilen.contains(b.namen[0]), "{} fehlt in der Hilfe", b.namen[0]);
            for n in b.namen {
                assert_eq!(befehl_zu(n), Some(b.art), "{n} wird nicht behandelt");
            }
        }
        assert_eq!(befehl_zu("kein Befehl"), None);
        assert_eq!(befehl_zu("/gibtsnicht"), None);
    }

    /// **Und der Hinweis unter der Eingabe nennt einen Befehl, den es
    /// gibt.**
    ///
    /// ⚠️ Ein Hinweis auf `/help`, waehrend der Befehl `/hilfe` heisst,
    /// ist schlimmer als keiner.
    #[test]
    fn der_hinweis_nennt_einen_befehl_den_es_gibt() {
        assert_eq!(befehl_zu(HILFE_BEFEHL), Some(Befehlsart::Hilfe));
    }

    /// **Der gemessene Rahmen bleibt zwischen den beiden Grenzen.**
    ///
    /// ⚠️ Ohne die untere Grenze faellt der Rahmen in einem Fenster ohne
    /// gemeldete Groesse auf null Zeichen zusammen; ohne die obere wird
    /// er auf einem breiten Schirm zur zweihundert Spalten langen Zeile.
    #[test]
    fn der_gemessene_rahmen_bleibt_in_seinen_grenzen() {
        let r = Rahmen::messen();
        assert!(r.innen >= 26, "der Rahmen ist auf {} Zeichen geschrumpft", r.innen);
        assert!(
            r.innen <= crate::schirm::BLOCKBREITE - 2,
            "der Rahmen ist mit {} Zeichen breiter als der Kopf",
            r.innen
        );
    }
}

/// **Die Warnung vor dem Agentenbetrieb, in der Konsole.**
///
/// ⚑ **Derselbe Text wie im Fenster**, aus `myl_client::warnung`: Eine
/// Warnung, die im einen Bedieninstrument strenger ist als im anderen,
/// ist keine Warnung, sondern eine Stichprobe.
///
/// ⚑ **Die Anleitung steht nicht sofort da, sondern auf Tastendruck.**
/// Sieben Regeln vor jedem Start waeren nach dem dritten Mal eine Wand,
/// die niemand mehr liest; wer sie will, bekommt sie.
///
/// ⛔️ **Und hier gibt es kein „nicht mehr zeigen".** Diese Konsole
/// **zeigt** Einstellungen und setzt sie nicht; ein zweiter Setzer waere
/// die dritte Stelle, die dieselben Feinheiten kennt, und
/// `die_logik_kommt_aus_der_kiste` haelt das fest. Abschalten laesst sie
/// sich mit `myl setzen agent.warnung aus` oder ueber das Haekchen im
/// Fenster.
///
/// ⚠️ **Ohne Terminal wird nichts gefragt.** In einer Roehre gibt es
/// niemanden, der zustimmen koennte; die Warnung wird dann gedruckt und
/// der Lauf geht weiter, so wie der Vorspann dort auch entfaellt.
fn warnung_zeigen(farbe: crossterm::style::Color) -> i32 {
    use std::io::{BufRead, IsTerminal, Write};

    let Ok(e) = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) else {
        return GUT;
    };
    if !e.agent.warnung {
        return GUT;
    }
    let w = myl_client::warnung::warnung(e.oberflaeche.sprache);

    // ⚑ **Als Block zentriert, nicht zeilenweise** (Auftrag des
    // Projektinhabers, 2026-09-15). `banner::zentriert` richtet den
    // ganzen Block an seiner breitesten Zeile aus; zeilenweise
    // verrutschten die Zeilen gegeneinander, dieselbe Lehre wie bei der
    // Auswahlliste. Ausserhalb eines Terminals tut es nichts, und das
    // ist richtig: In einer Roehre waeren Leerzeichen am Zeilenanfang
    // nur Muell in einer Datei.
    // ⚑ **Der Rumpf zuerst, die Achtungszeile danach**: Sie soll
    // innerhalb des Blocks mittig stehen, und dafuer muss die Breite des
    // Blocks feststehen. `banner::zentriert` richtet spaeter den ganzen
    // Block aus; diese eine Zeile wird zusaetzlich darin zentriert.
    let mut block = String::new();
    for zeile in umbrechen(w.kern, 64) {
        block.push_str(&zeile);
        block.push('\n');
    }
    // ⚑ **Die Anleitung steht vollstaendig da** (Auftrag des
    // Projektinhabers, 2026-09-15). Sie lag bis hierher hinter einer
    // Taste, mit dem Gedanken, sieben Regeln vor jedem Start seien eine
    // Wand. **Eine Anleitung, die man erst anfordern muss, liest
    // niemand**, und es ist der eine Bildschirm, auf dem es sich lohnt.
    //
    // ⚑ **Alles in EINEM Block**, auch die Regeln: Zentriert wird an der
    // breitesten Zeile, und saessen Kern und Regeln in getrennten
    // Bloecken, haetten sie verschiedene Einzuege und die Zuordnung waere
    // hin.
    block.push('\n');
    block.push_str(&format!("{}\n\n", w.anleitung_titel));
    for r in &w.regeln {
        for (i, zeile) in umbrechen(r.regel, 62).into_iter().enumerate() {
            block.push_str(&format!("{} {zeile}\n", if i == 0 { "*" } else { " " }));
        }
        // 📌 **Der Grund steht auf derselben Einrueckung wie der
        // Stichpunkttext.** Vier Leerzeichen gegen zwei ergaben eine
        // Stufe zwischen der ersten Zeile und der zweiten, und die las
        // sich wie ein Fehler (Meldung des Projektinhabers,
        // 2026-09-15). Den Unterschied traegt das Sternchen, nicht der
        // Rand.
        for zeile in umbrechen(r.grund, 62) {
            block.push_str(&format!("  {zeile}\n"));
        }
        block.push('\n');
    }
    // ⚠️ **Das Warnzeichen belegt zwei Spalten, zaehlt aber als ein
    // Zeichen.** Ohne diese Berichtigung saesse die Zeile um zwei
    // Spalten zu weit rechts.
    let spalten = |z: &str| z.chars().count() + z.matches('⚠').count();
    let breite = block.lines().map(spalten).max().unwrap_or(0);
    let mitte = " ".repeat(breite.saturating_sub(spalten(w.achtung)) / 2);
    let block = format!("{mitte}{}\n\n{block}", w.achtung);

    println!();
    println!("{}", crate::banner::zentriert(&block));

    if !std::io::stdin().is_terminal() {
        return GUT;
    }

    print!("{}", crate::banner::zentriert("[Eingabe] verstanden: "));
    let _ = std::io::stdout().flush();
    let mut zeile = String::new();
    let _ = std::io::stdin().lock().read_line(&mut zeile);
    // ⚑ **Hier steht fest, dass gedruckt wurde und ein Terminal da ist.**
    // Die frueheren Ausgaenge drucken nichts (Warnung abgeschaltet) oder
    // haben kein Terminal; dort waere ein Loeschbefehl ein Flackern ohne
    // Grund oder Zeichensalat in einer Datei.
    frei_bis_auf_das_logo(farbe);
    GUT
}

/// **Fragt, ob wirklich Schluss sein soll.**
///
/// ⚑ **Die Vorgabe haengt an der Taste** (Festlegung des
/// Projektinhabers, 2026-09-15), und das grosse Zeichen in der Frage
/// nennt sie:
///
/// - **Escape** wird neben den Pfeiltasten gestreift. Vorgabe `N`, also
///   bleibt man per Eingabe; Beenden verlangt ein ausdrueckliches `j`.
/// - **Strg-X** ist ein Zweifingergriff mit Absicht. Vorgabe `J`, also
///   beendet Eingabe; `n` haelt an.
///
/// ⚠️ **Ohne Terminal wird nicht gefragt**, sondern beendet: Dort ist
/// niemand, der antworten koennte, und eine Frage in einer Roehre waere
/// ein Haenger.
fn wirklich_beenden(vorgabe_ja: bool) -> bool {
    use std::io::{BufRead, IsTerminal, Write};
    if !std::io::stdin().is_terminal() {
        return true;
    }
    loop {
        let frage = if vorgabe_ja {
            "Wirklich beenden? (J/n): "
        } else {
            "Wirklich beenden? (j/N): "
        };
        print!("{}", crate::banner::zentriert(frage));
        let _ = std::io::stdout().flush();
        let mut zeile = String::new();
        if std::io::stdin().lock().read_line(&mut zeile).is_err() {
            // Kein Eingabestrom mehr (Strg-D): Das ist ein Schluss.
            return true;
        }
        match zeile.trim().to_ascii_lowercase().as_str() {
            "j" | "ja" | "y" | "yes" => return true,
            "n" | "nein" | "no" => return false,
            // Eingabe allein nimmt die Vorgabe an.
            "" => return vorgabe_ja,
            // Eine unverstandene Antwort beendet nichts; sie fragt noch
            // einmal.
            _ => continue,
        }
    }
}

/// Bricht einen Satz auf eine Breite, ohne Woerter zu zerschneiden.
fn umbrechen(text: &str, breite: usize) -> Vec<String> {
    let mut aus = Vec::new();
    let mut zeile = String::new();
    for wort in text.split_whitespace() {
        if !zeile.is_empty() && zeile.chars().count() + 1 + wort.chars().count() > breite {
            aus.push(std::mem::take(&mut zeile));
        }
        if !zeile.is_empty() {
            zeile.push(' ');
        }
        zeile.push_str(wort);
    }
    if !zeile.is_empty() {
        aus.push(zeile);
    }
    aus
}

#[cfg(test)]
mod abbruchprobe {
    use super::*;

    /// **Die drei Ausgaenge der Modellwahl bleiben unterscheidbar.**
    ///
    /// 📌 Vorher gab sie `bool` zurueck, und damit sah „der Nutzer hat
    /// Esc gedrueckt" genauso aus wie „hier gibt es kein Artefakt". Esc
    /// beendete deshalb das Programm ohne Rueckfrage (Auftrag des
    /// Projektinhabers, 2026-09-15).
    #[test]
    fn abgebrochen_ist_nicht_unmoeglich() {
        assert_ne!(Modellwahl::Abgebrochen, Modellwahl::Unmoeglich);
        assert_ne!(Modellwahl::Abgebrochen, Modellwahl::Geladen);
        // Und der Aufrufer behandelt genau diese drei; ein vierter Fall
        // muesste hier auffallen.
        for w in [Modellwahl::Geladen, Modellwahl::Abgebrochen, Modellwahl::Unmoeglich] {
            let _ = match w {
                Modellwahl::Geladen => 0,
                Modellwahl::Abgebrochen => 1,
                Modellwahl::Unmoeglich => 2,
            };
        }
    }

    /// ⚑ **Die Rueckfrage steht erst, wenn schon ein Modell laeuft.**
    ///
    /// Vor der ersten Wahl beendet Esc ohne Frage: Dort ist nichts zu
    /// verlieren (Festlegung des Projektinhabers, 2026-09-15). In
    /// `/model` ist ein Artefakt geladen, und dann fragt es.
    #[test]
    fn die_rueckfrage_steht_nur_bei_laufendem_modell() {
        let quelle = include_str!("sitzung.rs");
        // Der Start: abgebrochen heisst beenden, ohne Frage.
        assert!(
            quelle.contains("Modellwahl::Abgebrochen => return GUT,"),
            "der Start fragt nach, soll aber nicht"
        );
        // `/model`: abgebrochen fragt nach.
        let bei_model = quelle
            .split("Modellwahl::Abgebrochen => {")
            .nth(1)
            .expect("der fragende Abbruchzweig fehlt");
        assert!(
            bei_model.contains("wirklich_beenden("),
            "bei laufendem Modell fragt der Abbruch nicht nach: {bei_model}"
        );
        // Und `Unmoeglich` fragt nirgends: Eine Frage ohne Ausweg waere
        // eine Schleife.
        assert!(
            !quelle.contains("Modellwahl::Unmoeglich => {\n                            if wirklich_beenden("),
            "auch `Unmoeglich` fragt nach"
        );
    }
}

#[cfg(test)]
mod vorgabeprobe {
    /// ⚑ **Die Vorgabe der Rueckfrage haengt an der Taste** (Festlegung
    /// des Projektinhabers, 2026-09-15): Escape bleibt, Strg-X beendet.
    ///
    /// 📌 Vorher galt eine Vorgabe fuer beide, und damit beendete
    /// Eingabe auch nach einem gestreiften Escape.
    #[test]
    fn escape_bleibt_und_strg_x_beendet() {
        let quelle = include_str!("sitzung.rs");
        // Beide Fragezeilen stehen da, und das grosse Zeichen nennt die
        // Vorgabe.
        assert!(quelle.contains("\"Wirklich beenden? (J/n): \""), "die Ja-Vorgabe fehlt");
        assert!(quelle.contains("\"Wirklich beenden? (j/N): \""), "die Nein-Vorgabe fehlt");
        // Eine leere Antwort nimmt die Vorgabe an, statt etwas zu raten.
        assert!(
            quelle.contains("\"\" => return vorgabe_ja,"),
            "die Eingabetaste nimmt die Vorgabe nicht an"
        );
        // Und die Zuordnung: Strg-X gibt `true`, Escape in der Auswahl `false`.
        assert!(
            quelle.contains("wirklich_beenden(ausgang == eingabe::Ausgang::StrgX)"),
            "die Vorgabe haengt nicht an der Taste"
        );
        assert!(
            quelle.contains("wirklich_beenden(false)"),
            "Escape in der Modellwahl beendet per Eingabe"
        );
    }

    /// ⚑ **Die Konsole arbeitet in dem Verzeichnis, aus dem sie
    /// gestartet wurde** (Festlegung des Projektinhabers, 2026-09-15).
    ///
    /// Seit demselben Tag hat das Fenster eine Vorgabe (`WORK_DIR` im
    /// Repositorium), und die gilt hier **nicht**: Wer `myelith` in
    /// einem Projekt aufruft, will darin arbeiten und nicht in einem
    /// Beispielordner.
    ///
    /// 📌 **Geprueft wird, dass jede Stelle sie ueberschreibt, nicht
    /// dass sie es an einer tut.** Die Vorgabe greift in `ruestung.rs`
    /// ueber ein `or_else`, also genau dann, wenn `wurzel` hier `None`
    /// bleibt. Kommt eine dritte Ruestung in dieser Datei dazu und
    /// vergisst die Zeile, faellt sie still auf den Beispielordner
    /// zurueck, und niemand sieht es. Deshalb zaehlt dieser Test die
    /// Ruestungen gegen die Zuweisungen.
    ///
    /// 📌 **Gelesen wird nur der Teil vor den Testmodulen** (Fehlgriff
    /// beim Schreiben dieses Tests): Ein Muster, das in der Probe selbst
    /// steht, zaehlt sich mit, und die Verbotspruefung schlug an ihrem
    /// eigenen Doc-Kommentar fehl. **Eine Quellprobe, die sich selbst
    /// liest, prueft etwas anderes als sie meint.**
    #[test]
    fn die_konsole_arbeitet_wo_sie_gestartet_wurde() {
        let ganz = include_str!("sitzung.rs");
        let quelle = ganz.split("#[cfg(test)]").next().expect("der Code vor den Tests");
        let zuweisungen = quelle
            .matches("agent.wurzel = Some(stand.ordner.display().to_string());")
            .count();
        let ruestungen = quelle.matches("ruestung::ruesten").count();
        assert!(
            zuweisungen >= ruestungen,
            "{ruestungen} Ruestungen, aber nur {zuweisungen} Mal das Startverzeichnis gesetzt: \
             eine davon faellt auf die Vorgabe des Fensters zurueck"
        );
        assert!(zuweisungen > 0, "das Startverzeichnis wird nirgends gesetzt");
        // Und die Vorgabe des Fensters wird hier nirgends selbst geholt.
        assert!(
            !quelle.contains("standard_wurzel"),
            "die Konsole holt die Vorgabe des Fensters"
        );
    }
}
