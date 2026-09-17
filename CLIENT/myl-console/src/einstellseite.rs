//! Die Einstellungen, mit den Pfeiltasten bedienbar.
//!
//! # ⚑ Hoch und runter waehlt, links und rechts aendert
//!
//! **Was sich aus einer festen Menge bedienen laesst, soll sich mit
//! Pfeilen bedienen lassen** (Auftrag des Projektinhabers,
//! 2026-09-11): eine Auswahl, ein Schalter, eine Zahl, eine Grenze.
//! Was freien Text braucht, bekommt ihn auf Enter.
//!
//! # 📌 Und damit faellt eine frühere Festlegung
//!
//! Bis hierher zeigte `/settings` nur an, mit der Begruendung, ein
//! zweiter Setzer waere die dritte Stelle, die dieselben Feinheiten
//! kennt. **Die Begruendung galt dem Setzer und nicht der Bedienung:**
//! Gesetzt wird weiterhin ausschliesslich mit
//! [`myl_client::Einstellungen::setzen`], und was `aus` bedeutet,
//! welche Felder es gibt und welcher Wert eine Grenze loescht, steht
//! unveraendert an der einen Stelle in der Kiste. **Diese Datei
//! entscheidet nur, welche Taste welchen Wert vorschlaegt.**

use std::io::{IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};

use myl_client::einstellungen::{Einstellungen, Feld, Feldart, Feldwert, FELDER};
use myl_client::hardware::{Einheit, Hardware, Regler};

use crate::design::Toene;

/// Die Zeile ganz unten.
///
/// ⚑ **Eine Liste, und sie ist die einzige.** Was hier steht, wird
/// darueber behandelt; `jedes_kuerzel_wird_behandelt` haelt beides
/// zusammen.
pub const KUERZEL: &str =
    "  ↑↓ waehlen · ←→ aendern · ⏎ eingeben · ^S sichern · ^R Feld zuruecksetzen · Esc schliessen";

/// Wie weit ein Pfeil eine Zahl bewegt.
///
/// ⚑ **Abgeleitet und nicht getippt.** Ein Schritt von eins waere bei
/// 600 Antworttoken eine Geduldsprobe, einer von fuenfzig bei sechs
/// Schritten unbrauchbar. Die Groessenordnung sagt es selbst: eine
/// Stelle unter der hoechsten, mindestens eins.
pub fn schritt(wert: u64) -> u64 {
    let stellen = wert.to_string().len() as u32;
    10u64.pow(stellen.saturating_sub(2)).max(1)
}

/// **Die Enden eines Reglers, mit ihren Namen.**
///
/// ⚑ **Die Worte kommen aus [`Regler`] und werden hier nicht
/// erfunden.** Das Fenster zeigt dieselben Regler in derselben Sprache;
/// zwei Stellen mit je eigenen Worten laufen auseinander, und die
/// zweite ist die schlechter gepruefte.
#[derive(Debug, Clone)]
pub struct Spanne {
    pub mindestens: u64,
    /// `None` heisst: Diese Maschine nennt ihr Ende nicht, und dann ist
    /// es kein Regler. Der Grund steht in [`Reihe::sperrgrund`].
    pub hoechstens: Option<u64>,
    pub links: Option<String>,
    pub rechts: String,
    pub einheit: Einheit,
    pub sprache: myl_client::einstellungen::Sprache,
}

impl Spanne {
    fn aus(r: &Regler, sprache: myl_client::einstellungen::Sprache) -> Self {
        Self {
            mindestens: r.mindestens,
            hoechstens: r.hoechstens,
            links: r.links.clone(),
            rechts: r.rechts.clone(),
            einheit: r.einheit,
            sprache,
        }
    }

    /// **Wie ein Wert in dieser Spanne gelesen wird.**
    ///
    /// ⚑ **Nicht gesetzt steht ganz rechts und heisst „ohne Grenze".**
    /// 📌 Bis zum 2026-09-16 stand dort „aus", und das las sich wie
    /// „abgeschaltet", wo „unbegrenzt" gemeint war: **genau die
    /// schiefe Formulierung, die zur Architektur des naechsten Lesers
    /// wird.**
    pub fn wie(&self, w: &Feldwert) -> String {
        let n = match w {
            Feldwert::Leer => return self.rechts.clone(),
            Feldwert::Zahl(n) => *n,
            andere => return roh(andere),
        };
        if Some(n) == self.hoechstens {
            return self.rechts.clone();
        }
        if n == self.mindestens {
            if let Some(l) = &self.links {
                return l.clone();
            }
        }
        self.einheit.wie(n, self.sprache)
    }
}

/// **Eine Zeile der Seite**, gleich ob sie aus dem Katalog kommt oder
/// aus dem Hardwarescan.
///
/// ⚑ **Zwei Quellen, eine Zeile.** Die festen Felder stehen in
/// [`FELDER`], die Rechenwerke kennt erst der Scan. Beide werden gleich
/// gezeichnet und gleich bedient; wer sie getrennt behandelte, haette
/// zwei Bedienwege fuer denselben Regler, und einer davon waere
/// irgendwann der schlechtere.
#[derive(Debug, Clone)]
pub struct Reihe {
    /// Das Katalogfeld. `None` bei einem Rechenwerk: Wie viele es gibt,
    /// weiss erst der Scan, und deshalb steht keines im Katalog.
    pub feld: Option<Feld>,
    pub name: String,
    pub bereich: String,
    pub titel: String,
    pub hinweis: String,
    /// Gesetzt, wo die Zeile ein Regler ist: bei jeder Grenze und bei
    /// jedem Rechenwerk.
    pub spanne: Option<Spanne>,
    pub sperrgrund: Option<String>,
}

impl Reihe {
    /// Nimmt diese Zeile die Pfeile an?
    pub fn mit_pfeilen(&self) -> bool {
        match &self.feld {
            Some(f) => mit_pfeilen(f),
            // Ein Rechenwerk ist ein Anteil, also eine Zahl.
            None => true,
        }
    }

    /// Laesst sich diese Zeile tippen?
    pub fn mit_eingabe(&self) -> bool {
        match &self.feld {
            Some(f) => mit_eingabe(f),
            None => true,
        }
    }

    /// Wie ihr Wert in der Zeile steht.
    pub fn wie(&self, w: &Feldwert) -> String {
        match (&self.spanne, &self.feld) {
            (Some(s), _) => s.wie(w),
            (None, Some(f)) => anzeige(f, w),
            (None, None) => roh(w),
        }
    }
}

/// **Alle Zeilen der Seite, in der Reihenfolge, in der sie stehen.**
///
/// ⚑ **Die Rechenwerke stehen bei den Grenzen und nicht am Ende.** Sie
/// sind Grenzen dieses Rechners wie die Kerne auch; angehaengt haette
/// die Seite zwei Orte fuer dieselbe Frage.
pub fn reihen(e: &Einstellungen, hw: &Hardware) -> Vec<Reihe> {
    let sprache = e.oberflaeche.sprache;
    let regler = hw.regler(e);
    let mut aus: Vec<Reihe> = Vec::new();
    let mut nach_der_letzten_grenze = 0usize;

    // ⚑ **Was nur im Fenster wirkt, steht hier nicht**, aus demselben
    // Grund, aus dem das Fenster die Konsolenfelder weglaesst: Eine
    // Einstellung, die an der Stelle, an der sie steht, nichts bewirkt,
    // ist schlimmer als eine fehlende.
    for f in FELDER.iter().filter(|f| f.gilt.in_der_konsole()) {
        let f = f.in_sprache(sprache);
        let r = regler.iter().find(|r| r.name == f.name);
        aus.push(Reihe {
            feld: Some(f),
            name: f.name.to_string(),
            bereich: f.bereich.to_string(),
            titel: f.titel.to_string(),
            hinweis: f.hinweis.to_string(),
            spanne: r.map(|r| Spanne::aus(r, sprache)),
            sperrgrund: r.and_then(|r| r.sperrgrund.clone()),
        });
        if f.freigabe {
            nach_der_letzten_grenze = aus.len();
        }
    }

    let werke: Vec<Reihe> = regler
        .iter()
        .filter(|r| r.name.starts_with(myl_client::einstellungen::RECHENWERK_PRAEFIX))
        .map(|r| Reihe {
            feld: None,
            name: r.name.clone(),
            // ⚑ Dieselbe Ueberschrift wie die Kerne: Es ist dieselbe
            // Frage, naemlich was dieser Rechner hergibt.
            bereich: aus
                .get(nach_der_letzten_grenze.saturating_sub(1))
                .map(|z| z.bereich.clone())
                .unwrap_or_default(),
            titel: r.titel.clone(),
            hinweis: r.hinweis.clone(),
            spanne: Some(Spanne::aus(r, sprache)),
            sperrgrund: r.sperrgrund.clone(),
        })
        .collect();
    aus.splice(nach_der_letzten_grenze..nach_der_letzten_grenze, werke);
    aus
}

/// Ein Wert ohne Spanne und ohne Feld, fuer die Faelle, die es nicht
/// geben sollte.
fn roh(w: &Feldwert) -> String {
    match w {
        Feldwert::Leer => "(nicht gesetzt)".to_string(),
        Feldwert::Zahl(n) => n.to_string(),
        Feldwert::Schalter(b) => if *b { "an" } else { "aus" }.to_string(),
        Feldwert::Text(t) if t.is_empty() => "(nicht gesetzt)".to_string(),
        Feldwert::Text(t) => t.clone(),
    }
}

/// **Was ein Pfeil an einem Regler bewegt.**
///
/// ⚑ **Die Schrittweite kommt aus dem Ende und nicht aus dem Wert.**
/// Sonst bewegte derselbe Regler sich unten in Einern und oben in
/// Zehnern, und ein Weg zurueck traefe nicht dieselben Stellungen wie
/// der Weg hin.
///
/// ⚑ **Ganz rechts wird `aus` gesetzt**, also die Grenze
/// **weggenommen**. Das ist der Unterschied zwischen „nimm alles" und
/// „nimm genau so viel, wie die Maschine heute hat": Bei der Platte
/// heisst das zweite zusaetzlich, dass der Platz wirklich belegt wird.
///
/// `None` heisst: Hier bewegt sich nichts mehr, und **das ist eine
/// Auskunft und kein Fehler**.
pub fn geaendert_regler(s: &Spanne, jetzt: &Feldwert, rechts: bool) -> Option<String> {
    let ende = s.hoechstens?;
    let jetzt_n = match jetzt {
        Feldwert::Leer => ende,
        Feldwert::Zahl(n) => (*n).min(ende),
        _ => return None,
    };
    let weite = schritt(ende);
    let neu = if rechts {
        jetzt_n.saturating_add(weite).min(ende)
    } else {
        jetzt_n.saturating_sub(weite).max(s.mindestens)
    };
    // ⚑ **Am Anschlag ist Schluss und es faengt nicht von vorne an.**
    if neu == jetzt_n {
        return None;
    }
    Some(if neu == ende { "aus".to_string() } else { neu.to_string() })
}

/// **Was aus dem Wert wird, wenn ein Pfeil gedrueckt wird.**
///
/// `None` heisst: Dieses Feld laesst sich mit Pfeilen nicht aendern.
/// **Das ist eine Auskunft und kein Fehler**, und die Zeile sagt es
/// dann auch.
pub fn geaendert(f: &Feld, jetzt: &Feldwert, rechts: bool, admin: bool) -> Option<String> {
    match f.art {
        // ⚑ **Am Ende ist Schluss und es faengt nicht von vorne an.**
        // Ein Umlauf laesst jemanden, der zu weit gedrueckt hat, wieder
        // von vorne suchen.
        Feldart::Auswahl => {
            let offen: Vec<&str> = f
                .wahl
                .iter()
                .filter(|w| !w.nur_admin || admin)
                .map(|w| w.wert)
                .collect();
            let Feldwert::Text(t) = jetzt else { return None };
            let hier = offen.iter().position(|w| *w == t)?;
            let ziel = if rechts { hier + 1 } else { hier.checked_sub(1)? };
            offen.get(ziel).map(|w| w.to_string())
        }
        Feldart::Schalter => Some(if rechts { "an".into() } else { "aus".into() }),
        Feldart::Zahl => {
            let Feldwert::Zahl(n) = jetzt else { return None };
            let s = schritt(*n);
            Some(if rechts { (n + s).to_string() } else { n.saturating_sub(s).max(1).to_string() })
        }
        // ⚑ **Eine Grenze geht durch [`geaendert_regler`]** (seit dem
        // 2026-09-16), denn sie hat seither ein **Ende**: Ganz rechts
        // steht „ohne Grenze", und wo das ist, weiss nur der
        // Hardwarescan. 📌 Hier stand die Regel ein zweites Mal, ohne
        // Ende und mit `aus` am linken Anschlag; **zwei Regeln fuer
        // denselben Regler waeren zwei Bedienwege.**
        //
        // Ohne erkanntes Ende bewegt sich nichts, und das ist richtig:
        // Ein Regler ohne Ende ist keiner, und die Zeile sagt warum.
        Feldart::Grenze => None,
        Feldart::Text | Feldart::Pfad | Feldart::Ordner => None,
    }
}

/// **Wie ein Wert in der Zeile steht.**
///
/// ⚑ **Bei einer Auswahl steht die Beschriftung und nicht die
/// Kennung.** `de` und `automatisch` sind das, was in der Ablage steht;
/// gelesen wird „Deutsch" und „Automatisch". **Wer eine Kennung sieht,
/// wo ein Wort stehen koennte, liest die Ablage und nicht die
/// Einstellung.**
///
/// ⚑ **Und `aus` heisst nur bei einer Grenze `aus`.** Ein nicht
/// gesetzter Pfad ist nicht abgeschaltet, er ist leer, und der
/// Unterschied ist genau der zwischen „keine Grenze" und „kein
/// Ordner".
pub fn anzeige(f: &Feld, w: &Feldwert) -> String {
    if let (Feldart::Auswahl, Feldwert::Text(t)) = (f.art, w) {
        if let Some(gefunden) = f.wahl.iter().find(|x| x.wert == t) {
            return gefunden.titel.to_string();
        }
    }
    // ⚑ **Eine Grenze steht hier nicht mehr**, sie geht durch
    // [`Spanne::wie`]: Was „nicht gesetzt" heisst, haengt seit dem
    // 2026-09-16 am Ende des Reglers, und das kennt nur der Scan.
    roh(w)
}

/// **Derselbe Wert, aber so, wie der Setzer ihn annimmt.**
///
/// ⚑ **Das Gegenstueck zu [`anzeige`], und es ist ein anderes.** Was
/// ein Mensch liest („Deutsch", „aus", „(nicht gesetzt)"), ist nicht
/// das, was in der Ablage steht; wer das eine fuer das andere haelt,
/// setzt „Deutsch" als Sprache und bekommt einen Fehler.
pub fn als_wert(w: &Feldwert) -> String {
    match w {
        Feldwert::Leer => "aus".to_string(),
        Feldwert::Zahl(n) => n.to_string(),
        Feldwert::Schalter(true) => "an".to_string(),
        Feldwert::Schalter(false) => "aus".to_string(),
        // ⚑ Ein leerer Text nimmt das Feld weg, und dafuer heisst das
        // Wort in diesem Programm ueberall `aus`.
        Feldwert::Text(t) if t.is_empty() => "aus".to_string(),
        Feldwert::Text(t) => t.clone(),
    }
}

/// Ob dieses Feld sich tippen laesst.
///
/// ⚑ **Zahlen und Grenzen auch** (Auftrag des Projektinhabers,
/// 2026-09-11). Wer von 600 auf 4000 will, drueckt sonst vierunddreissig
/// Mal; **ein Pfeil ist zum Nachstellen da und nicht zum Eingeben.**
pub fn mit_eingabe(f: &Feld) -> bool {
    matches!(
        f.art,
        Feldart::Text | Feldart::Pfad | Feldart::Ordner | Feldart::Zahl | Feldart::Grenze
    )
}

/// Ob dieses Feld die Pfeile ueberhaupt annimmt.
pub fn mit_pfeilen(f: &Feld) -> bool {
    !matches!(f.art, Feldart::Text | Feldart::Pfad | Feldart::Ordner)
}

/// **Zeigt die Seite und nimmt Aenderungen entgegen.**
///
/// Gibt zurueck, ob etwas geaendert wurde.
pub fn fahren(t: Toene, ordner: &std::path::Path) -> bool {
    let pfad = Einstellungen::vorgabepfad();
    let mut e = match Einstellungen::lesen(&pfad) {
        Ok(e) => e,
        Err(f) => {
            eprintln!("Die Einstellungen sind nicht lesbar: {f}");
            return false;
        }
    };
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        zeilenweise(&e, ordner);
        return false;
    }
    let Ok(_roh) = crate::auswahl::Rohmodus::an() else {
        zeilenweise(&e, ordner);
        return false;
    };

    let admin = myl_client::einstellungen::ist_admin();
    // ⚑ **Einmal beim Oeffnen und nicht je Tastendruck.** Der Scan
    // kostet auf macOS einen Unterprozess; welche Karten im Rechner
    // stecken, aendert sich waehrend einer Einstellungsseite nicht.
    let hw = Hardware::erheben(&myl_client::ort::datenort());
    // ⚑ **Die Liste steht einmal.** Was sich waehrend der Seite aendert,
    // sind die **Werte**, und die holt jede Zeichnung aus den
    // Einstellungen; Enden, Beschriftungen und Sperrgruende haengen an
    // der Maschine und nicht am Tastendruck.
    let liste = reihen(&e, &hw);
    let anzahl = liste.len();
    let mut hier = 0usize;
    let mut geaendert_worden = false;
    let mut meldung = String::new();
    let hoehe = zeichnen(&e, &liste, hier, t, ordner, &meldung);

    loop {
        let Ok(Event::Key(k)) = event::read() else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        let strg = k.modifiers.contains(KeyModifiers::CONTROL);
        meldung.clear();
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => break,
            KeyCode::Char('c') if strg => break,
            KeyCode::Up => hier = hier.saturating_sub(1),
            KeyCode::Down => hier = (hier + 1).min(anzahl - 1),
            KeyCode::Left | KeyCode::Right => {
                let r = &liste[hier];
                let Ok(jetzt) = e.wert(&r.name) else { continue };
                let nach_rechts = k.code == KeyCode::Right;
                // ⚑ **Ein Regler geht durch die Reglerregel, alles
                // andere durch die Feldregel.** Beide Sorten Zeile
                // liegen in derselben Liste und werden gleich bedient.
                let neu = match (&r.spanne, &r.feld) {
                    (Some(sp), _) => geaendert_regler(sp, &jetzt, nach_rechts),
                    (None, Some(f)) => geaendert(f, &jetzt, nach_rechts, admin),
                    (None, None) => None,
                };
                match neu {
                    Some(neu) => match e.setzen(&r.name, &neu) {
                        Ok(()) => geaendert_worden = true,
                        Err(m) => meldung = m,
                    },
                    // ⚑ **Ein gesperrter Regler sagt an sich selbst,
                    // was fehlt**, statt stumm stehenzubleiben. Ein
                    // Tastendruck ohne Wirkung und ohne Wort sieht aus
                    // wie ein haengendes Programm.
                    None if r.sperrgrund.is_some() => {
                        meldung = r.sperrgrund.clone().unwrap_or_default();
                    }
                    None if !r.mit_pfeilen() => {
                        meldung = "Dieses Feld braucht eine Eingabe: ⏎.".to_string();
                    }
                    None => {}
                }
            }
            // ⚑ **Sichern, ohne zu schliessen.** Wer lange an der Seite
            // sitzt, soll nicht erst hinausgehen muessen, um sicher zu
            // sein.
            KeyCode::Char('s') if strg => match e.schreiben(&pfad) {
                Ok(()) => {
                    geaendert_worden = false;
                    meldung = "gesichert".to_string();
                }
                Err(m) => meldung = m,
            },
            // ⚑ **Nur das gewaehlte Feld**, und das steht auch so in der
            // Zeile unten. Ein Tastendruck, der die ganze Ablage
            // zuruecksetzt, braeuchte eine Rueckfrage, und eine
            // Rueckfrage auf jedem Tastendruck ist genau das, was
            // niemand mehr liest.
            KeyCode::Char('r') if strg => {
                let r = &liste[hier];
                let vorgabe = Einstellungen::default();
                // ⚑ **Die Vorgabe kommt aus `Einstellungen::default`**,
                // auch fuer ein Rechenwerk: Dort ist kein Eintrag, also
                // `aus`, also ganz rechts.
                match vorgabe.wert(&r.name).map(|w| als_wert(&w)) {
                    Ok(neu) => match e.setzen(&r.name, &neu) {
                        Ok(()) => {
                            geaendert_worden = true;
                            meldung = format!("{} zurueckgesetzt", r.titel);
                        }
                        Err(m) => meldung = m,
                    },
                    Err(m) => meldung = m,
                }
            }
            KeyCode::Enter => {
                let r = &liste[hier];
                if !r.mit_eingabe() {
                    meldung = "Dieses Feld hat feste Werte: ← und →.".to_string();
                    let _ = zeichnen_ab(hoehe, &e, &liste, hier, t, ordner, &meldung);
                    continue;
                }
                // ⚑ Getippt wird in einer eigenen Zeile unter der
                // Liste, und die Liste bleibt stehen: Wer tippt, will
                // sehen, was er aendert.
                let jetzt = e.wert(&r.name).map(|w| als_wert(&w)).unwrap_or_default();
                if let Some(neu) = tippen(&r.titel, &jetzt, t) {
                    match e.setzen(&r.name, &neu) {
                        Ok(()) => geaendert_worden = true,
                        Err(m) => meldung = m,
                    }
                }
            }
            _ => continue,
        }
        let _ = zeichnen_ab(hoehe, &e, &liste, hier, t, ordner, &meldung);
    }

    if geaendert_worden {
        if let Err(m) = e.schreiben(&pfad) {
            eprintln!("Die Einstellungen lassen sich nicht schreiben: {m}");
            return false;
        }
    }
    geaendert_worden
}

/// Fragt einen Text ab, unter der Liste.
fn tippen(titel: &str, jetzt: &str, t: Toene) -> Option<String> {
    let mut aus = std::io::stdout();
    // ⚑ **Der bisherige Wert steht in der Klammer und nicht im Feld.**
    // Vorgetippt muesste man ihn erst wegloeschen; daneben ist er die
    // Auskunft, die man beim Tippen braucht.
    let _ = crossterm::queue!(
        aus,
        Print("\r\n"),
        SetForegroundColor(t.akzent),
        Print(format!("  {titel} ({jetzt}): ")),
        ResetColor,
        crossterm::cursor::Show
    );
    let _ = aus.flush();
    let mut zeile = String::new();
    loop {
        let Ok(Event::Key(k)) = event::read() else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        match k.code {
            KeyCode::Enter => break,
            KeyCode::Esc => {
                zeile.clear();
                break;
            }
            KeyCode::Backspace => {
                if zeile.pop().is_some() {
                    let _ = write!(aus, "\u{8} \u{8}");
                    let _ = aus.flush();
                }
            }
            KeyCode::Char(_) if k.modifiers.contains(KeyModifiers::CONTROL) => continue,
            KeyCode::Char(c) => {
                zeile.push(c);
                let _ = write!(aus, "{c}");
                let _ = aus.flush();
            }
            _ => {}
        }
    }
    // Die getippte Zeile wieder wegnehmen, die Liste zeichnet sich neu.
    let _ = write!(aus, "\r\x1b[2K\x1b[1A");
    let _ = crossterm::execute!(aus, crossterm::cursor::Hide);
    (!zeile.trim().is_empty()).then(|| zeile.trim().to_string())
}

fn zeichnen_ab(
    hoehe: usize,
    e: &Einstellungen,
    liste: &[Reihe],
    hier: usize,
    t: Toene,
    ordner: &std::path::Path,
    meldung: &str,
) -> usize {
    let _ = write!(std::io::stdout(), "\x1b[{hoehe}A");
    zeichnen(e, liste, hier, t, ordner, meldung)
}

/// Zeichnet die Seite und gibt ihre Hoehe zurueck.
fn zeichnen(
    e: &Einstellungen,
    liste: &[Reihe],
    hier: usize,
    t: Toene,
    ordner: &std::path::Path,
    meldung: &str,
) -> usize {
    let breite = (crate::banner::fensterbreite() as usize).clamp(40, 100);
    let mut aus = std::io::stdout();
    let mut zeilen = 0;
    let _ = crossterm::queue!(aus, crossterm::cursor::Hide);

    let mut kopf = |text: String, farbe| {
        let _ = crossterm::queue!(
            aus,
            Print("\x1b[2K"),
            SetForegroundColor(farbe),
            Print(text),
            ResetColor,
            Print("\r\n")
        );
    };
    kopf("  Einstellungen\r\n".into(), t.akzent);
    zeilen += 2;

    let mut bereich = String::new();
    for (i, f) in liste.iter().enumerate() {
        if f.bereich != bereich {
            bereich.clone_from(&f.bereich);
            let _ = crossterm::queue!(
                aus,
                Print("\x1b[2K\r\n\x1b[2K"),
                SetForegroundColor(t.kante),
                Print(format!("  {}", f.bereich)),
                ResetColor,
                Print("\r\n")
            );
            zeilen += 2;
        }
        let wert = e.wert(&f.name).map(|w| f.wie(&w)).unwrap_or_else(|m| m);
        let hierhin = i == hier;
        let marke = if hierhin { "▸" } else { " " };
        // ⚑ Die Winkel sagen, dass sich hier etwas aendern laesst, und
        // stehen nur, wo das stimmt. **Ein gesperrter Regler traegt
        // keine**, denn dort bewegt sich nichts, und ein Winkel waere
        // Zierde an einer Stelle, die nichts leistet.
        let beweglich = f.mit_pfeilen() && f.sperrgrund.is_none();
        let (links, rechts) = if beweglich { ("‹ ", " ›") } else { ("  ", "") };
        let titel = format!("  {marke} {:<30}", kuerzen(&f.titel, 30));
        let _ = crossterm::queue!(aus, Print("\x1b[2K"));
        if hierhin {
            let _ = crossterm::queue!(
                aus,
                SetForegroundColor(t.akzent),
                SetAttribute(Attribute::Bold),
                Print(&titel),
                SetAttribute(Attribute::Reset)
            );
        } else {
            let _ = crossterm::queue!(aus, ResetColor, Print(&titel));
        }
        let platz = breite.saturating_sub(titel.chars().count() + 6);
        let _ = crossterm::queue!(
            aus,
            SetForegroundColor(if hierhin { t.akzent } else { t.beiwerk }),
            Print(format!("{links}{}{rechts}", kuerzen(&wert, platz))),
            ResetColor,
            Print("\r\n")
        );
        zeilen += 1;
    }

    // Der Satz zur gewaehlten Zeile, und darunter der Arbeitsordner.
    let f = &liste[hier];
    // ⚑ **Beim Modellfeld haengt die Mindestausstattung an den Satz**
    // (Festlegung des Projektinhabers, 2026-09-11). Sie kommt aus
    // derselben Karte wie in der Modellwahl; eine zweite Quelle liefe
    // auseinander.
    //
    // ⚑ **Und ein gesperrter Regler sagt statt seines Hinweises, was
    // fehlt.** Genau wie im Fenster: „Noch nicht verfuegbar" liesse den
    // Leser so klug zurueck wie zuvor.
    let satz = if let Some(grund) = &f.sperrgrund {
        grund.clone()
    } else if f.name == "modell.artefakt" {
        let hw = myl_client::modelle::hardware_zu(&e.modell.artefakt);
        if hw.is_empty() { f.hinweis.clone() } else { format!("{}  ·  {hw}", f.hinweis) }
    } else {
        f.hinweis.clone()
    };
    let _ = crossterm::queue!(
        aus,
        Print("\x1b[2K\r\n\x1b[2K"),
        SetForegroundColor(t.beiwerk),
        Print(format!("  {}", kuerzen(&satz, breite.saturating_sub(4)))),
        ResetColor,
        Print("\r\n\x1b[2K")
    );
    zeilen += 3;
    let zeile = if meldung.is_empty() {
        format!("  In dieser Sitzung arbeitet der Agent in {}", ordner.display())
    } else {
        format!("  ⚑ {meldung}")
    };
    let _ = crossterm::queue!(
        aus,
        SetForegroundColor(if meldung.is_empty() { t.beiwerk } else { t.akzent }),
        Print(kuerzen(&zeile, breite)),
        ResetColor,
        Print("\r\n\x1b[2K\r\n\x1b[2K")
    );
    zeilen += 3;

    // ⚑ **Die Tastenkuerzel stehen ganz unten** (Festlegung des
    // Projektinhabers, 2026-09-11). Oben stehen sie einmal im Weg und
    // danach nie wieder dort, wo man sie sucht: **Was man beim Bedienen
    // braucht, gehoert an den Rand, an dem der Blick ohnehin endet.**
    let _ = crossterm::queue!(
        aus,
        SetForegroundColor(t.kante),
        Print(kuerzen(KUERZEL, breite)),
        ResetColor,
        Print("\r\n")
    );
    zeilen += 1;
    let _ = aus.flush();
    zeilen
}

fn kuerzen(text: &str, breite: usize) -> String {
    let n = text.chars().count();
    if n <= breite {
        return text.to_string();
    }
    text.chars().take(breite.saturating_sub(1)).chain(['…']).collect()
}

/// Ohne Terminal bleibt es beim Anzeigen.
fn zeilenweise(e: &Einstellungen, ordner: &std::path::Path) {
    println!();
    println!("  Datei: {}", Einstellungen::vorgabepfad().display());
    // ⚑ **Dieselbe Liste wie auf der Seite**, samt Rechenwerken. Eine
    // Roehre, die weniger zeigt als ein Terminal, waere eine zweite
    // Auskunft ueber dieselbe Ablage.
    let hw = Hardware::erheben(&myl_client::ort::datenort());
    for f in reihen(e, &hw) {
        let wert = e.wert(&f.name).map(|w| f.wie(&w)).unwrap_or_else(|m| m);
        let zusatz = if f.sperrgrund.is_some() { "  (rechnet hier nicht)" } else { "" };
        println!("  {:<28} {:<28} {}{zusatz}", f.name, wert, f.titel);
    }
    println!();
    println!("  In dieser Sitzung arbeitet der Agent in:");
    println!("    {}", ordner.display());
    println!();
}

/// **Die Werkzeugkiste waehlen, aus den Ordnern, die nebeneinander liegen.**
///
/// ⚑ **Hier und nicht in `sitzung.rs`** (Befehl `/toolkit`, Auftrag des
/// Projektinhabers, 2026-09-15). Diese Datei ist die Stelle, an der die
/// Konsole Einstellungen **setzt**; `sitzung.rs` zeigt sie nur, und
/// `die_logik_kommt_aus_der_kiste` haelt das fest. Der Befehl dort ruft
/// diese Funktion, mehr tut er nicht.
///
/// Gibt zurueck, wie die gewaehlte Kiste heisst, falls gewaehlt wurde.
pub fn werkzeugkiste_waehlen(t: crate::design::Toene) -> Option<String> {
    let pfad = myl_client::Einstellungen::vorgabepfad();
    let mut e = myl_client::Einstellungen::lesen(&pfad).ok()?;

    let kisten = myl_client::kisten::vorhandene(e.agent.kistenordner.as_deref());
    if kisten.is_empty() {
        println!("  Hier liegt keine Werkzeugkiste.");
        return None;
    }

    // Vorgewaehlt ist die, die gilt.
    let jetzt = myl_client::kisten::ordner_der_gilt(
        e.agent.kistenordner.as_deref(),
        myl_client::werkzeuge::Werkzeugkiste::Base.name(),
    );
    let start = jetzt.as_ref().and_then(|o| kisten.iter().position(|k| k == o)).unwrap_or(0);

    let punkte: Vec<crate::wahl::Punkt> = kisten
        .iter()
        .map(|k| crate::wahl::Punkt {
            titel: k
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| k.display().to_string()),
            // ⚑ Unter dem Namen steht, was drin ist und was daraus folgt:
            // Der Ordnername entscheidet ueber die eingebauten Werkzeuge.
            hinweis: format!(
                "{} Werkzeug(e) · eingebaut: {}",
                myl_client::kisten::manifeste_lesen(k, |_| {}).len(),
                myl_client::werkzeuge::Werkzeugkiste::aus_ordnername(
                    &myl_client::kisten::ordnername(Some(k), "Base")
                )
                .name(),
            ),
            offen: true,
        })
        .collect();

    let i = crate::wahl::waehlen_ab("Welche Werkzeugkiste?", &punkte, start, t)?;
    let gewaehlt = kisten[i].display().to_string();
    if let Err(m) = e.setzen("agent.kistenordner", &gewaehlt) {
        println!("  Das ging nicht: {m}");
        return None;
    }
    if let Err(m) = e.schreiben(&pfad) {
        println!("  Nicht gesichert: {m}");
        return None;
    }
    Some(punkte[i].titel.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feld(name: &str) -> Feld {
        *FELDER.iter().find(|f| f.name == name).expect(name)
    }

    /// **Der Schritt folgt der Groessenordnung.**
    #[test]
    fn der_schritt_passt_zur_zahl() {
        assert_eq!(schritt(6), 1);
        assert_eq!(schritt(12), 1);
        assert_eq!(schritt(600), 10);
        assert_eq!(schritt(4096), 100);
        assert_eq!(schritt(0), 1);
    }

    /// **Ein Schalter kippt in beide Richtungen.**
    #[test]
    fn ein_schalter_kippt() {
        let f = feld("agent.schreiben");
        let aus = Feldwert::Schalter(false);
        assert_eq!(geaendert(&f, &aus, true, false).as_deref(), Some("an"));
        assert_eq!(geaendert(&f, &aus, false, false).as_deref(), Some("aus"));
    }

    /// **Eine Auswahl laeuft nicht um.**
    ///
    /// 📌 Ein Umlauf laesst jemanden, der zu weit gedrueckt hat, wieder
    /// von vorne suchen.
    #[test]
    fn eine_auswahl_haelt_an_ihren_enden() {
        let f = feld("agent.modus");
        let erster = Feldwert::Text("auto".into());
        let letzter = Feldwert::Text("manual".into());
        assert_eq!(geaendert(&f, &erster, true, false).as_deref(), Some("manual"));
        assert_eq!(geaendert(&f, &erster, false, false), None);
        assert_eq!(geaendert(&f, &letzter, true, false), None);
        assert_eq!(geaendert(&f, &letzter, false, false).as_deref(), Some("auto"));
    }

    /// Eine Spanne, wie `hardware::Regler` sie liefern wuerde.
    fn spanne(mindestens: u64, hoechstens: Option<u64>, links: Option<&str>, einheit: Einheit) -> Spanne {
        Spanne {
            mindestens,
            hoechstens,
            links: links.map(str::to_string),
            rechts: "ohne Grenze".to_string(),
            einheit,
            sprache: myl_client::einstellungen::Sprache::De,
        }
    }

    /// **Ganz rechts nimmt die Grenze weg, und dort steht sie ohne
    /// Zutun.**
    ///
    /// ⚑ Das ist der Auftrag des Projektinhabers vom 2026-09-16, in der
    /// Bedienung: Der rechte Anschlag setzt `aus`, und `aus` **loescht**
    /// die Grenze. 📌 Bis dahin lag `aus` links unter dem kleinsten
    /// Wert, und ein unbegrenzter Regler sah aus wie ein abgedrehter.
    #[test]
    fn ganz_rechts_nimmt_die_grenze_weg() {
        // Eine Maschine mit 24 GiB: Schrittweite eins.
        let s = spanne(1, Some(24), None, Einheit::Gib);
        assert_eq!(geaendert_regler(&s, &Feldwert::Leer, false).as_deref(), Some("23"));
        // Am rechten Anschlag bewegt sich nichts mehr.
        assert_eq!(geaendert_regler(&s, &Feldwert::Leer, true), None);
        // Und von dicht darunter fuehrt der Pfeil nach rechts wieder
        // heraus, also auf `aus`.
        assert_eq!(geaendert_regler(&s, &Feldwert::Zahl(23), true).as_deref(), Some("aus"));
        assert_eq!(geaendert_regler(&s, &Feldwert::Zahl(12), false).as_deref(), Some("11"));
        // ⚑ **Eine Grenze faellt nicht unter eins**, und am Anschlag
        // sagt die Zeile das, statt stumm stehenzubleiben.
        assert_eq!(geaendert_regler(&s, &Feldwert::Zahl(1), false), None);
    }

    /// **Ein Rechenwerk faellt bis auf null, und null heisst „rechnet
    /// nicht".**
    ///
    /// ⚑ Der Unterschied zur Grenze ist der ganze Punkt: Null Kerne
    /// waeren ein Stillstand, ein Rechenwerk bei null rechnet einfach
    /// nicht mit, und die CPU rechnet weiter.
    #[test]
    fn ein_rechenwerk_faellt_bis_auf_null() {
        let s = spanne(0, Some(100), Some("rechnet nicht"), Einheit::Prozent);
        // Schrittweite zehn, aus dem Ende hergeleitet.
        assert_eq!(geaendert_regler(&s, &Feldwert::Leer, false).as_deref(), Some("90"));
        assert_eq!(geaendert_regler(&s, &Feldwert::Zahl(10), false).as_deref(), Some("0"));
        assert_eq!(geaendert_regler(&s, &Feldwert::Zahl(0), false), None);
        assert_eq!(geaendert_regler(&s, &Feldwert::Zahl(0), true).as_deref(), Some("10"));
        assert_eq!(geaendert_regler(&s, &Feldwert::Zahl(90), true).as_deref(), Some("aus"));

        assert_eq!(s.wie(&Feldwert::Zahl(0)), "rechnet nicht");
        assert_eq!(s.wie(&Feldwert::Leer), "ohne Grenze");
        assert_eq!(s.wie(&Feldwert::Zahl(40)), "40 %");
    }

    /// **Ohne erkanntes Ende bewegt sich nichts**, und das ist richtig:
    /// Ein Regler ohne Ende ist keiner.
    #[test]
    fn ohne_ende_bewegt_sich_nichts() {
        let s = spanne(1, None, None, Einheit::Gib);
        assert_eq!(geaendert_regler(&s, &Feldwert::Leer, true), None);
        assert_eq!(geaendert_regler(&s, &Feldwert::Zahl(4), false), None);
    }

    /// **Die Zeilen der Seite tragen die Rechenwerke mit**, und zwar
    /// unter derselben Ueberschrift wie die Kerne.
    ///
    /// ⚑ **Geprueft wird gegen den echten Scan dieser Maschine**, und
    /// die kann null Rechenwerke haben: In der CI gibt es keine
    /// Grafikkarte. Die Pruefung sagt deshalb nur, **was gelten muss,
    /// wenn eines da ist**, und nicht, dass eines da ist.
    #[test]
    fn die_zeilen_tragen_die_rechenwerke_mit() {
        let e = Einstellungen::default();
        let hw = Hardware::erheben(std::path::Path::new("."));
        let liste = reihen(&e, &hw);

        // ⚑ **Nur die Felder, die hier auch wirken.** Ein Feld, das nur
        // im Fenster etwas tut, steht in der Konsole nicht, und die
        // Pruefung zaehlt deshalb dieselbe Auswahl wie die Seite.
        let hiesige: Vec<&Feld> = FELDER.iter().filter(|f| f.gilt.in_der_konsole()).collect();
        assert!(hiesige.len() < FELDER.len(), "kein einziges Feld ist fensterseitig; misst die Pruefung noch etwas?");
        assert_eq!(
            liste.len(),
            hiesige.len() + hw.rechenwerke.len(),
            "die Liste fuehrt nicht jedes hiesige Feld und jedes Rechenwerk genau einmal"
        );
        for f in &hiesige {
            assert!(liste.iter().any(|z| z.name == f.name), "`{}` fehlt auf der Seite", f.name);
        }
        // Und die reinen Fensterfelder stehen ausdruecklich NICHT darin.
        for f in FELDER.iter().filter(|f| !f.gilt.in_der_konsole()) {
            assert!(
                !liste.iter().any(|z| z.name == f.name),
                "`{}` wirkt nur im Fenster und steht trotzdem in der Konsole",
                f.name
            );
        }
        // Und jede Grenze ist ein Regler mit Enden.
        for z in liste.iter().filter(|z| z.feld.is_some_and(|f| f.art == Feldart::Grenze)) {
            assert!(z.spanne.is_some(), "`{}` ist eine Grenze ohne Spanne", z.name);
        }

        let werke: Vec<&Reihe> = liste.iter().filter(|z| z.feld.is_none()).collect();
        assert_eq!(werke.len(), hw.rechenwerke.len());
        let grenzbereich = liste
            .iter()
            .find(|z| z.name == "kap.kerne")
            .map(|z| z.bereich.clone())
            .expect("kap.kerne");
        for w in werke {
            assert!(w.spanne.is_some(), "ein Rechenwerk ohne Spanne");
            assert_eq!(w.bereich, grenzbereich, "ein Rechenwerk steht nicht bei den Grenzen");
            assert!(
                w.name.starts_with(myl_client::einstellungen::RECHENWERK_PRAEFIX),
                "{}",
                w.name
            );
        }
    }

    /// **Eine Zahl faellt nie unter eins.**
    ///
    /// ⚠️ Null Schritte waeren kein enger gestellter Agent, sondern
    /// einer, der nicht antwortet.
    #[test]
    fn eine_zahl_bleibt_brauchbar() {
        let f = feld("agent.schritte");
        assert_eq!(geaendert(&f, &Feldwert::Zahl(1), false, false).as_deref(), Some("1"));
        assert_eq!(geaendert(&f, &Feldwert::Zahl(6), true, false).as_deref(), Some("7"));
    }

    /// **Eine Auswahl zeigt ihre Beschriftung, keine Kennung.**
    #[test]
    fn eine_auswahl_zeigt_was_dasteht() {
        let f = feld("oberflaeche.sprache");
        assert_eq!(anzeige(&f, &Feldwert::Text("de".into())), "Deutsch");
        let m = feld("agent.modus");
        assert_eq!(anzeige(&m, &Feldwert::Text("manual".into())), "manual mode");
    }

    /// **`aus` steht nur da, wo es `aus` heisst.**
    ///
    /// 📌 Ein nicht gesetzter Ordner ist nicht abgeschaltet, er ist
    /// leer.
    #[test]
    fn ein_leerer_ordner_ist_nicht_aus() {
        // ⚑ **Eine nicht gesetzte Grenze heisst „ohne Grenze"**, nicht
        // „aus": Das eine ist unbegrenzt, das andere liest sich wie
        // abgeschaltet. 📌 Hier stand bis zum 2026-09-16 „aus".
        let s = spanne(1, Some(24), None, Einheit::Gib);
        assert_eq!(s.wie(&Feldwert::Leer), "ohne Grenze");
        assert_eq!(anzeige(&feld("ausgabe.ordner"), &Feldwert::Leer), "(nicht gesetzt)");
    }

    /// **Jedes Kuerzel der Fusszeile wird auch behandelt.**
    ///
    /// 📌 **Dieselbe Klasse wie Fund 271.** Eine Zeile mit
    /// Tastenkuerzeln, die von Hand gepflegt wird, nennt irgendwann
    /// eines, das nichts tut, und **das sieht erst der, der es
    /// ausprobiert.**
    #[test]
    fn jedes_kuerzel_wird_behandelt() {
        let quelle = include_str!("einstellseite.rs");
        // Was in der Zeile steht, muss im Tastenzweig vorkommen.
        let paare = [
            ("↑↓", "KeyCode::Up"),
            ("←→", "KeyCode::Left | KeyCode::Right"),
            ("⏎", "KeyCode::Enter"),
            ("^S", "KeyCode::Char('s') if strg"),
            ("^R", "KeyCode::Char('r') if strg"),
            ("Esc", "KeyCode::Esc"),
        ];
        for (zeichen, zweig) in paare {
            assert!(KUERZEL.contains(zeichen), "`{zeichen}` fehlt in der Fusszeile");
            assert!(quelle.contains(zweig), "`{zeichen}` wird nicht behandelt ({zweig})");
        }
    }

    /// **Jeder Standardwert wird vom Setzer angenommen.**
    ///
    /// 📌 Die Gegenprobe zu `^R`: Ein Zuruecksetzen, das der Setzer
    /// ablehnt, waere eine Taste, die eine Fehlermeldung erzeugt statt
    /// eines Wertes.
    #[test]
    fn jeder_standardwert_laesst_sich_setzen() {
        let vorgabe = Einstellungen::default();
        let mut e = Einstellungen::default();
        for f in FELDER {
            let w = vorgabe.wert(f.name).expect(f.name);
            let wort = als_wert(&w);
            e.setzen(f.name, &wort)
                .unwrap_or_else(|m| panic!("{}: Vorgabe `{wort}` wird abgelehnt: {m}", f.name));
        }
    }

    /// **Zahlen und Grenzen lassen sich auch tippen.**
    ///
    /// ⚑ Auftrag des Projektinhabers: Wer von 600 auf 4000 will,
    /// drueckt sonst vierunddreissig Mal.
    #[test]
    fn zahlen_lassen_sich_tippen() {
        for name in ["modell.token", "agent.schritte", "kap.speicher"] {
            assert!(mit_eingabe(&feld(name)), "{name} nimmt keine Eingabe");
            assert!(mit_pfeilen(&feld(name)), "{name} nimmt keine Pfeile");
        }
        // Und eine Auswahl bleibt bei den Pfeilen.
        assert!(!mit_eingabe(&feld("agent.modus")));
    }

    /// **Ein Textfeld sagt Nein und nicht irgendetwas.**
    #[test]
    fn ein_textfeld_nimmt_keine_pfeile() {
        for name in ["modell.artefakt", "agent.wurzel", "ausgabe.ordner"] {
            let f = feld(name);
            assert!(!mit_pfeilen(&f), "{name} nimmt Pfeile an");
            assert_eq!(
                geaendert(&f, &Feldwert::Text("x".into()), true, false),
                None,
                "{name} aendert sich mit einem Pfeil"
            );
        }
    }

    /// **Jeder Wert, den ein Pfeil vorschlaegt, wird auch angenommen.**
    ///
    /// 📌 **Die Gegenprobe zur ganzen Datei.** Diese hier schlaegt vor,
    /// gesetzt wird in der Kiste; ein Vorschlag, den der Setzer ablehnt,
    /// waere eine Taste, die nichts tut und nicht sagt, warum.
    #[test]
    fn jeder_vorschlag_wird_angenommen() {
        let mut e = Einstellungen::default();
        for f in FELDER {
            if !mit_pfeilen(&f) {
                continue;
            }
            for _ in 0..12 {
                for rechts in [true, false] {
                    let jetzt = e.wert(f.name).expect(f.name);
                    if let Some(neu) = geaendert(&f, &jetzt, rechts, true) {
                        e.setzen(f.name, &neu).unwrap_or_else(|m| {
                            panic!("{}: Vorschlag `{neu}` wird abgelehnt: {m}", f.name)
                        });
                    }
                }
            }
        }
    }
}
