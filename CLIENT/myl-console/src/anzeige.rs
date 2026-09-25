//! Was waehrend eines Auftrags dasteht.
//!
//! # ⚑ Eine Zeile, die sich bewegt, und ein Schalter dahinter
//!
//! Ein Agentenlauf mit einem oertlichen Modell dauert Minuten. Ohne
//! Anzeige sieht das aus wie ein haengendes Programm, mit einer
//! Vollausgabe wie ein Protokoll, das niemand liest. **Die Zeile sagt
//! deshalb dreierlei: dass es laeuft, wie lange schon, und was gerade
//! geschieht** (Festlegung des Projektinhabers, 2026-09-11).
//!
//! ⚑ **Und wer mehr will, drueckt Strg-T.** Dann stehen die
//! Werkzeugaufrufe mit ihren Ergebnissen da, auch die, die vorher
//! schon liefen: Sie werden mitgeschrieben, nicht weggeworfen.
//!
//! # 📌 Warum ein eigener Faden
//!
//! Der Auftrag laeuft im Hauptfaden und blockiert dort. Eine Animation
//! braucht aber einen Takt, und eine Taste will gelesen werden, ohne
//! dass jemand die Eingabetaste drueckt. **Deshalb ist dieser Faden der
//! einzige, der waehrend eines Laufs auf den Schirm schreibt**; der
//! Melder aus dem Hauptfaden legt nur ab, was er zu sagen hat. Zwei
//! Schreiber auf einem Terminal ergeben Zeichensalat, und zwar
//! unregelmaessig.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::design;
use crate::schirm::{Rahmen, Schirm};

/// ⚑ **Vom Projektinhaber gewuenscht, und sie meinen nichts.** Ein
/// Ladetext, der so tut, als beschreibe er den Rechenschritt, waere
/// eine Erfindung; diese hier sind erkennbar keine Auskunft. **Was
/// wirklich geschieht, steht daneben in der Klammer.**
const SPRUECHE: [&str; 8] = [
    "warping bytes",
    "fluxcompensating binaries",
    "shuffling ones and zeros",
    "crushing circuits",
    "untangling tensors",
    "bribing the bus",
    "polishing pointers",
    "herding electrons",
];

/// **Sechs Drehscheiben, und je Schritt eine andere.**
///
/// ⚑ Gewuenscht vom Projektinhaber (2026-09-11). Sie kosten nichts und
/// machen sichtbar, dass ein **neuer** Schritt begonnen hat: Die Zeile
/// wechselt nicht nur den Text, sondern auch ihre Bewegung.
///
/// ⚠️ **Alles davon ist Unicode und kein ASCII.** Ein Terminal ohne die
/// Zeichen zeigt Kaestchen; dasselbe gilt fuer den Rahmen und die
/// Marke, und das Programm setzt es ohnehin voraus.
const RAEDER: [&[char]; 6] = [
    &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'],
    &['▏', '▎', '▍', '▌', '▋', '▊', '▉', '▊', '▋', '▌', '▍', '▎'],
    &['◐', '◓', '◑', '◒'],
    &['⡀', '⡄', '⡆', '⡇', '⡏', '⡟', '⡿', '⣿', '⡿', '⡟', '⡏', '⡇', '⡆', '⡄'],
    &['←', '↖', '↑', '↗', '→', '↘', '↓', '↙'],
    &['·', '∘', '○', '◎', '●', '◎', '○', '∘'],
];

/// Wie weit die Farbe von einem Zeichen zum naechsten weiterwandert.
///
/// ⚑ **Daraus ergibt sich die Geschwindigkeit der Welle:** Bei 400 ms
/// je Zeichen laeuft sie zweieinhalb Zeichen in der Sekunde nach links,
/// waehrend der ganze Farbkreis weiter zwei Minuten braucht.
///
/// ⚑ **Auch das Logo wandert in diesem Takt**, eine Spalte je Welle
/// ([`crate::schimmer`]); die beiden Bewegungen sollen zusammengehoeren.
pub(crate) const WELLE: Duration = Duration::from_millis(400);

/// Wie lange ein voller Farbdurchlauf dauert.
///
/// ⚑ **Zwei Minuten** (Festlegung des Projektinhabers). Lang genug, dass
/// die Zeile nicht blinkt, und kurz genug, dass man die Bewegung sieht.
pub(crate) const REGENBOGEN: Duration = Duration::from_secs(120);

/// Wie lange ein Takt dauert.
const TAKT: Duration = Duration::from_millis(90);

/// Womit die ausfuehrliche Anzeige ein- und ausgeschaltet wird.
///
/// 📌 **Erst `^T`, dann `^O`, jetzt `^S`** (Festlegungen des
/// Projektinhabers vom 2026-09-11). ⚠️ Strg-S ist in der **Kochstufe**
/// die Flusssperre XOFF und liesse das Terminal stehen; im Rohmodus ist
/// sie abgeschaltet, und die Taste kommt als gewoehnliches Ereignis an.
/// **Dieses Programm liest sie nur, solange ein Auftrag laeuft**, und
/// dann ist der Rohmodus an.
pub const SCHALTER: &str = "^S";

/// **Eine Zeile der Zeitleiste, in zwei Laengen.**
///
/// ⚑ **Gedruckt wird sofort und beides wird aufgehoben.** Bis zum
/// 2026-09-11 hielt die Anzeige die Zeilen zurueck und reichte sie
/// nach, wenn jemand den Schalter drueckte; **damit fehlte in der
/// Zeitleiste genau das, was vor der Antwort geschehen war.** Jetzt
/// steht jede Zeile da, sobald sie entsteht, und der Schalter
/// entscheidet nur, **wie ausfuehrlich** die naechsten sind.
struct Zeitzeile {
    /// Was die Zeile meldet; danach richtet sich ihr Stil.
    art: Art,
    /// Die kurze Form. `None` heisst: ohne Schalter gar nicht zeigen.
    kurz: Option<String>,
    /// Die ausfuehrliche Form.
    voll: String,
}

/// **Was eine Zeile der Zeitleiste meldet.**
///
/// ⚑ **Die Art steht an der Zeile und nicht in ihrem Text** (2026-09-24):
/// Gesetzt wird beim Schreiben ([`zeile_setzen`]), und dort soll niemand
/// aus einem Pfeil erraten muessen, was gemeint war.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Art {
    /// `● Schritt n · Zeit`
    Schritt,
    /// `→ werkzeug argumente`
    Aufruf,
    /// `← werkzeug ergebnis`
    Ergebnis,
    /// `⚑ werkzeug abgelehnt: grund`
    Abgelehnt,
    /// Alles andere: verdichtet, umgeschaltet.
    Hinweis,
}

/// **Setzt eine Zeile der Zeitleiste in den Rollen des Designs.**
///
/// Rein, damit es sich pruefen laesst: Dieselbe Zeile ergibt dieselben
/// Zeichen. Ohne `farbig` kommt die Zeile unveraendert zurueck, und in
/// jedem Fall steht derselbe Text da, nur anders gefaerbt.
///
/// | Art | Stil |
/// |---|---|
/// | Aufruf | Pfeil und Werkzeug als Aktion, die Argumente schlicht |
/// | Ergebnis | Pfeil und Werkzeug als Ergebnis, der Inhalt als Beiwerk |
/// | Abgelehnt | Marke, Werkzeug und „abgelehnt:" als Warnung |
/// | Schritt | die Marke hervorgehoben, der Rest als Beiwerk |
/// | Hinweis | ganz als Beiwerk |
pub fn zeile_setzen(art: Art, zeile: &str, r: &design::Rollen, farbig: bool) -> String {
    if !farbig {
        return zeile.to_string();
    }
    let rumpf = zeile.trim_start();
    let einzug = &zeile[..zeile.len() - rumpf.len()];
    // Marke und erstes Wort: `→ name`, `← name`, `● Schritt`.
    let kopf_ende = |n: usize| {
        rumpf.char_indices().filter(|(_, c)| *c == ' ').nth(n).map(|(i, _)| i).unwrap_or(rumpf.len())
    };
    let (kopf, rest) = match art {
        Art::Aufruf | Art::Ergebnis => rumpf.split_at(kopf_ende(1)),
        Art::Abgelehnt => match rumpf.find("abgelehnt:") {
            Some(i) => rumpf.split_at(i + "abgelehnt:".len()),
            None => (rumpf, ""),
        },
        Art::Schritt => rumpf.split_at(rumpf.chars().next().map(char::len_utf8).unwrap_or(0)),
        Art::Hinweis => ("", rumpf),
    };
    let (kopfstil, reststil) = match art {
        Art::Aufruf => (r.aktion, None),
        Art::Ergebnis => (r.ergebnis, Some(r.beiwerk)),
        Art::Abgelehnt => (r.warnung, None),
        Art::Schritt => (r.marke, Some(r.beiwerk)),
        Art::Hinweis => (r.beiwerk, Some(r.beiwerk)),
    };
    let rest = match reststil {
        Some(st) => st.faerben(rest, true),
        None => rest.to_string(),
    };
    format!("{einzug}{}{rest}", kopfstil.faerben(kopf, true))
}

/// **Welche Art Text zuletzt in der Zeitleiste stand.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    /// Zeilen der Zeitleiste: Schritte, Aufrufe, Rueckgaben.
    Handlung,
    /// Text des Modells im Rohstrom.
    Text,
}

/// **Ob vor dem naechsten Stueck eine Leerzeile steht.**
///
/// ⚑ **Zwischen Handlungen und Text des Modells ein Absatz** (Auftrag
/// des Projektinhabers, 2026-09-24): Wechselt die Zeitleiste von
/// Werkzeugzeilen zu Modelltext oder zurueck, steht eine Leerzeile
/// dazwischen. Innerhalb einer Art nicht, sonst zerfiele eine Folge von
/// Aufrufen in lauter Einzelstuecke.
fn absatz_noetig(vorher: Option<Block>, jetzt: Block) -> bool {
    vorher.is_some_and(|v| v != jetzt)
}

/// Was der Faden zu wissen braucht.
struct Lage {
    /// Was gerade geschieht, in einem Wort.
    was: String,
    /// Der Ladetext dieses Schrittes.
    spruch: &'static str,
    /// Welche Drehscheibe dieser Schritt dreht.
    rad: usize,
    /// Seit wann dieser Auftrag laeuft.
    anfang: Instant,
    /// Zeilen, die noch nicht gedruckt sind.
    offen: Vec<Zeitzeile>,
    /// Wie viele Zeilen dieser Lauf schon geschrieben hat.
    gezaehlt: usize,
    /// Ob die Zeilen ausfuehrlich geschrieben werden.
    details: bool,
    /// Was das Modell seit dem letzten Takt erzeugt hat.
    ///
    /// ⚑ **Token fuer Token, so wie sie fallen.** Ohne das sieht ein
    /// Denkvorgang von einer Minute aus wie Stillstand.
    ///
    /// ⚑ **Je Stueck mit seiner Art** (2026-09-24): `true` heisst Denken.
    /// In einer einzigen Zeichenkette liessen sich Denken und Antwort
    /// nachher nicht mehr auseinanderhalten, und das Denken soll anders
    /// aussehen.
    strom: Vec<(bool, String)>,
    /// Was zuletzt geschrieben wurde, fuer den Absatz beim Wechsel.
    zuletzt: Option<Block>,
    /// Ob eine angefangene Zeile des Stroms auf dem Schirm steht.
    ///
    /// ⚠️ **Solange sie steht, wird die Statuszeile nicht gezeichnet.**
    /// Sie beginnt mit `\r` und raeumte damit genau das weg, was gerade
    /// entsteht.
    strom_offen: bool,
    /// Zustand des Zufalls.
    saat: u64,
    /// Ob nach dem Beenden gefragt wurde, und die Frage noch offen ist.
    ///
    /// ⚑ **Eigenes Feld und nicht `frage`** (Auftrag des
    /// Projektinhabers, 2026-09-15): `frage` traegt die Vorlage einer
    /// schreibenden Handlung im `manual mode`. Beide in einem Feld, und
    /// ein Tastendruck haette die eine Frage mit der Antwort auf die
    /// andere beantwortet.
    abbruchfrage: bool,
    /// Ob die Abbruchfrage schon dasteht.
    abbruch_gestellt: bool,
    /// Was gerade vorgelegt wird, im `manual mode`.
    frage: Option<String>,
    /// Ob sie schon dasteht.
    gestellt: bool,
    /// Die Antwort, sobald eine Taste kam.
    antwort: Option<bool>,
}

impl Lage {
    /// Ein neuer Spruch, und **nie derselbe zweimal**.
    fn neuer_spruch(&mut self) {
        let alt = self.spruch;
        for _ in 0..8 {
            self.saat = wuerfeln(self.saat);
            let s = SPRUECHE[(self.saat % SPRUECHE.len() as u64) as usize];
            if s != alt {
                self.spruch = s;
                break;
            }
        }
        let alt = self.rad;
        for _ in 0..8 {
            self.saat = wuerfeln(self.saat);
            let r = (self.saat % RAEDER.len() as u64) as usize;
            if r != alt {
                self.rad = r;
                return;
            }
        }
    }
}

/// Ein Wurf, ganzzahlig und ohne Abhaengigkeit.
///
/// ⚑ **Xorshift und keine Kiste dafuer.** Es geht um die Reihenfolge
/// von acht Spruechen; wer dafuer eine Abhaengigkeit aufnimmt, zahlt
/// sie in jedem Bau und in jeder Lizenzpruefung.
fn wuerfeln(mut x: u64) -> u64 {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

/// Die laufende Anzeige eines Auftrags.
pub struct Anzeige {
    lage: Arc<Mutex<Lage>>,
    laeuft: Arc<AtomicBool>,
    faden: Option<std::thread::JoinHandle<()>>,
    am_schirm: bool,
}

impl Anzeige {
    /// **Beginnt einen Lauf.**
    ///
    /// Ohne Schirm gibt es keine Animation und keine Taste: Dann
    /// schreibt der Melder seine Zeilen einfach hin, wie vorher.
    pub fn starten(
        schirm: Option<Schirm>,
        bild: myl_client::einstellungen::Konsolendesign,
        zaehler: std::sync::Arc<myl_client::oertlich::Tokenzaehler>,
    ) -> Self {
        let am_schirm = schirm.is_some();
        let saat = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 | 1)
            .unwrap_or(0x2545_F491_4F6C_DD1D);
        let mut anfangslage = Lage {
            was: "thinking".to_string(),
            spruch: SPRUECHE[0],
            rad: 0,
            anfang: Instant::now(),
            offen: Vec::new(),
            gezaehlt: 0,
            details: false,
            strom: Vec::new(),
            zuletzt: None,
            strom_offen: false,
            saat,
            abbruchfrage: false,
            abbruch_gestellt: false,
            frage: None,
            gestellt: false,
            antwort: None,
        };
        anfangslage.neuer_spruch();

        let lage = Arc::new(Mutex::new(anfangslage));
        let laeuft = Arc::new(AtomicBool::new(true));
        // ⚑ **Das Logo fliesst, solange der Auftrag laeuft**, und setzt
        // dort fort, wo es beim vorigen stehenblieb ([`crate::schimmer`]).
        // Ohne Schirm gibt es niemanden, der es sieht, und keinen Faden,
        // der es malt; die Uhr liefe dann umsonst.
        if am_schirm {
            crate::schimmer::los();
        }
        let faden = am_schirm.then(|| {
            let lage = Arc::clone(&lage);
            let laeuft = Arc::clone(&laeuft);
            std::thread::spawn(move || takten(lage, laeuft, schirm, bild, zaehler))
        });
        Self { lage, laeuft, faden, am_schirm }
    }

    /// Ein neuer Schritt: neuer Spruch, neue Scheibe, und gedacht wird
    /// wieder.
    ///
    /// ⚑ **Der Schritt steht nur in der ausfuehrlichen Anzeige.** Kurz
    /// gefasst ist „das Modell denkt" keine Nachricht, sondern der
    /// Normalzustand; ausfuehrlich ist es die Achse, an der sich die
    /// Zeiten ablesen lassen.
    pub fn schritt(&self, n: u32) {
        self.strom_schliessen();
        if let Ok(mut l) = self.lage.lock() {
            l.was = "thinking".to_string();
            l.neuer_spruch();
            let seit = dauer(l.anfang.elapsed());
            l.offen.push(Zeitzeile { art: Art::Schritt, kurz: None, voll: format!("  ● Schritt {n} · {seit}") });
        }
        self.spuelen();
    }

    /// Ein Werkzeug laeuft.
    pub fn aufruf(&self, name: &str, kurz: &str, voll: &str) {
        self.merken(
            Zeitzeile {
                art: Art::Aufruf,
                kurz: Some(format!("  → {name} {kurz}")),
                voll: format!("  → {name} {voll}"),
            },
            Some(name.to_string()),
        );
    }

    /// Und was es zurueckgab.
    pub fn ergebnis(&self, name: &str, kurz: &str, voll: &str) {
        self.merken(
            Zeitzeile {
                art: Art::Ergebnis,
                kurz: Some(format!("  ← {name} {kurz}")),
                voll: format!("  ← {name} {voll}"),
            },
            Some("thinking".to_string()),
        );
    }

    /// Ein Vorschlag wurde abgewiesen.
    pub fn abgelehnt(&self, name: &str, grund: &str) {
        let z = format!("  ⚑ {name} abgelehnt: {grund}");
        self.merken(Zeitzeile { art: Art::Abgelehnt, kurz: Some(z.clone()), voll: z }, Some("abgelehnt".to_string()));
    }

    /// **Der Agent hat den Verlauf verdichtet**, weil der naechste Schritt
    /// nicht mehr in den Kontext passte.
    pub fn verdichtet(&self, vorher: usize, nachher: usize) {
        let z = format!("  ⚑ Kontext verdichtet: {vorher} → {nachher} Token");
        self.merken(Zeitzeile { art: Art::Hinweis, kurz: Some(z.clone()), voll: z }, Some("verdichtet".to_string()));
    }

    fn merken(&self, zeile: Zeitzeile, was: Option<String>) {
        if !self.am_schirm {
            // Ohne Schirm gibt es keinen Faden, der sie drucken
            // koennte, und keine Zeile, die sie ueberschreibt.
            println!("{}", zeile.voll);
            return;
        }
        if let Ok(mut l) = self.lage.lock() {
            if let Some(w) = was {
                l.was = w;
            }
            l.offen.push(zeile);
        }
        self.spuelen();
    }

    /// ⚑ **Wartet, bis der Faden die Zeile geschrieben hat.** Sonst
    /// erschiene sie erst beim naechsten Takt, und bei einem Werkzeug,
    /// das eine Zehntelsekunde braucht, stuende das Ergebnis vor dem
    /// Aufruf.
    fn spuelen(&self) {
        if !self.am_schirm {
            return;
        }
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(4));
            match self.lage.lock() {
                Ok(l) if l.offen.is_empty() => return,
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }

    /// **Ein Griff, der Token entgegennimmt.**
    ///
    /// ⚑ **Er wandert in das Modell** und muss deshalb `Send + Sync`
    /// und `'static` sein; die Anzeige selbst wird am Ende verbraucht.
    pub fn strom(&self) -> Stromgriff {
        Stromgriff { lage: Arc::clone(&self.lage), am_schirm: self.am_schirm }
    }

    /// **Ein Griff, mit dem gefragt werden kann.**
    ///
    /// ⚑ **Getrennt von der Anzeige selbst**, denn er wandert in eine
    /// Ruestung und muss `Send + Sync` sein und sich kopieren lassen;
    /// die Anzeige dagegen wird am Ende **verbraucht**.
    pub fn frager(&self) -> Frager {
        Frager {
            lage: Arc::clone(&self.lage),
            laeuft: Arc::clone(&self.laeuft),
            am_schirm: self.am_schirm,
        }
    }

    /// Schliesst eine angefangene Stromzeile, falls eine offen ist.
    fn strom_schliessen(&self) {
        if let Ok(mut l) = self.lage.lock() {
            if l.strom_offen {
                l.strom.push((false, "\n".to_string()));
            }
        }
        self.spuelen();
    }

    /// **Beendet den Lauf und raeumt die Zeile.**
    ///
    /// Gibt zurueck, ob die Details eingeschaltet waren: Wer sie nicht
    /// gesehen hat, soll wenigstens erfahren, dass es sie gab.
    pub fn beenden(mut self) -> (bool, usize) {
        self.strom_schliessen();
        self.laeuft.store(false, Ordering::Relaxed);
        if let Some(f) = self.faden.take() {
            let _ = f.join();
        }
        // ⚑ **Erst nach dem letzten Bild angehalten**: Der Faden malt bis
        // zuletzt im Stand der Uhr, und genau dieser Stand soll stehen
        // bleiben, auch bei jedem Neudruck bis zum naechsten Auftrag.
        crate::schimmer::halt();
        if self.am_schirm {
            let mut aus = std::io::stdout();
            let _ = write!(aus, "\r\x1b[2K");
            let _ = aus.flush();
        }
        let l = self.lage.lock();
        match l {
            Ok(l) => (l.details, l.gezaehlt),
            Err(_) => (false, 0),
        }
    }
}

/// Der Faden: Taste lesen, Zeilen nachreichen, Zeile zeichnen.
fn takten(
    lage: Arc<Mutex<Lage>>,
    laeuft: Arc<AtomicBool>,
    schirm: Option<Schirm>,
    bild: myl_client::einstellungen::Konsolendesign,
    zaehler: std::sync::Arc<myl_client::oertlich::Tokenzaehler>,
) {
    let mut takt: usize = 0;
    while laeuft.load(Ordering::Relaxed) {
        // ⚑ **Die Taste wird ausserhalb der Sperre gelesen.** Sonst
        // wartete der Melder des Hauptfadens bei jedem Takt neunzig
        // Millisekunden auf eine Sperre, die ohnehin nur wartet.
        if event::poll(TAKT).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                    // ⚑ **Eine offene Abbruchfrage geht allem vor**
                    // (Auftrag des Projektinhabers, 2026-09-15): Solange
                    // sie dasteht, ist die naechste Taste ihre Antwort
                    // und nichts anderes.
                    if offene_abbruchfrage(&lage) {
                        if let Some(ja) = als_antwort(k.code) {
                            if ja {
                                abbrechen(schirm);
                            }
                            abbruchfrage_weg(&lage);
                        }
                    } else if k.modifiers.contains(KeyModifiers::CONTROL) {
                        match k.code {
                            KeyCode::Char('s') => umschalten(&lage),
                            // ⚑ **Beenden fragt nach**, und beide Tasten
                            // tun dasselbe: Strg-C aus Gewohnheit,
                            // Strg-X wie an der Eingabezeile. Vorher
                            // beendete Strg-C den Prozess mitten im Lauf
                            // ohne ein Wort.
                            KeyCode::Char('c') | KeyCode::Char('x') => abbruchfrage_stellen(&lage),
                            _ => {}
                        }
                    } else if let Some(ja) = als_antwort(k.code) {
                        beantworten(&lage, ja);
                    }
                }
                _ => {}
            }
        }
        zeichnen(&lage, takt, bild, schirm, &zaehler);
        // ⚑ **Das Logo nach der Zeile**, denn erst dann steht der Wagen
        // wieder im Rollbereich, und nach ihm richtet sich, ob gemalt
        // werden darf.
        if let Some(sch) = schirm {
            if takt.is_multiple_of(crate::schimmer::MALTAKT) {
                crate::schimmer::malen(sch.rollende());
            }
        }
        takt = takt.wrapping_add(1);
    }
}

/// **Welche Taste welche Antwort ist.**
///
/// ⚑ **`j` und `y`, denn beide Tastaturen liegen hier.** Und die
/// Eingabetaste bestaetigt, weil sie das ueberall tut.
pub fn als_antwort(code: KeyCode) -> Option<bool> {
    match code {
        KeyCode::Char('j') | KeyCode::Char('y') | KeyCode::Enter => Some(true),
        KeyCode::Char('n') | KeyCode::Esc => Some(false),
        _ => None,
    }
}

/// Ob die Abbruchfrage offen ist.
fn offene_abbruchfrage(lage: &Arc<Mutex<Lage>>) -> bool {
    lage.lock().map(|l| l.abbruchfrage).unwrap_or(false)
}

/// Stellt die Abbruchfrage.
fn abbruchfrage_stellen(lage: &Arc<Mutex<Lage>>) {
    if let Ok(mut l) = lage.lock() {
        l.abbruchfrage = true;
        l.abbruch_gestellt = false;
    }
}

/// Nimmt sie zurueck; der Lauf geht weiter.
fn abbruchfrage_weg(lage: &Arc<Mutex<Lage>>) {
    if let Ok(mut l) = lage.lock() {
        l.abbruchfrage = false;
        l.abbruch_gestellt = false;
    }
}

/// Traegt die Antwort ein, sofern ueberhaupt gefragt wurde.
fn beantworten(lage: &Arc<Mutex<Lage>>, ja: bool) {
    if let Ok(mut l) = lage.lock() {
        if l.frage.is_some() {
            l.antwort = Some(ja);
        }
    }
}

fn umschalten(lage: &Arc<Mutex<Lage>>) {
    if let Ok(mut l) = lage.lock() {
        l.details = !l.details;
        // ⚑ **Der Wechsel steht in der Leiste.** Sonst liest sich der
        // Verlauf spaeter wie zwei verschiedene Programme, und niemand
        // weiss mehr, warum die Zeilen ab hier laenger sind.
        let wort = if l.details { "an" } else { "aus" };
        l.offen.push(Zeitzeile {
            art: Art::Hinweis,
            kurz: Some(format!("  · ausfuehrlich: {wort}")),
            voll: format!("  · ausfuehrlich: {wort}"),
        });
    }
}

/// **Das saubere Ende mitten im Lauf**: Rohmodus aus, Rollbereich
/// zurueck, Schluss.
///
/// ⚠️ **Strg-C im Rohmodus erzeugt kein Signal**, also muss die Taste
/// hier beantwortet werden; das System nimmt einem die Arbeit nicht ab.
///
/// ⚑ **Gerufen wird das erst aus einer bejahten Abbruchfrage**
/// (2026-09-15), nicht mehr direkt aus dem Tastendruck: Der laufende
/// Auftrag laesst sich nicht zurueckrufen, und ein Prozess, der mitten
/// in einer Erzeugung ohne ein Wort endet, kostet den ganzen Lauf.
fn abbrechen(schirm: Option<Schirm>) -> ! {
    let _ = crossterm::terminal::disable_raw_mode();
    if let Some(s) = schirm {
        s.aufloesen();
    }
    println!();
    std::process::exit(130);
}

fn zeichnen(
    lage: &Arc<Mutex<Lage>>,
    takt: usize,
    bild: myl_client::einstellungen::Konsolendesign,
    schirm: Option<Schirm>,
    zaehler: &myl_client::oertlich::Tokenzaehler,
) {
    let Ok(mut l) = lage.lock() else { return };
    let mut aus = std::io::stdout();

    // ⚑ **Was ansteht, wird gedruckt und bleibt stehen.** Nur die
    // Statuszeile darunter wird ueberschrieben; die Zeitleiste selbst
    // wird nie geloescht.
    let rollen = design::rollen(bild);
    let farbig = design::farbig();
    if !l.offen.is_empty() {
        let ausfuehrlich = l.details;
        let offen: Vec<Zeitzeile> = std::mem::take(&mut l.offen);
        let _ = write!(aus, "\r\x1b[2K");
        for z in offen {
            let text = if ausfuehrlich { Some(z.voll) } else { z.kurz };
            if let Some(t) = text {
                l.gezaehlt += 1;
                if absatz_noetig(l.zuletzt, Block::Handlung) {
                    let _ = write!(aus, "\r\n");
                }
                l.zuletzt = Some(Block::Handlung);
                let _ = write!(aus, "{}\r\n", zeile_setzen(z.art, &t, &rollen, farbig));
            }
        }
    }

    // ⚑ **Die Abbruchfrage verdraengt die Statuszeile genauso**, und sie
    // steht **nach** der Vorlage: Wer im `manual mode` gerade gefragt
    // wird, soll diese Frage zuerst beantworten.
    if l.abbruchfrage && l.frage.is_none() {
        if !l.abbruch_gestellt {
            l.abbruch_gestellt = true;
            let _ = write!(
                aus,
                "\r\x1b[2K{}  Wirklich beenden? [Eingabe] ja   [n] weiter{}",
                crossterm::style::SetForegroundColor(design::toene(bild).beiwerk),
                crossterm::style::ResetColor
            );
            let _ = aus.flush();
        }
        return;
    }

    // ⚑ **Eine offene Frage verdraengt die Statuszeile.** Zwei Zeilen
    // uebereinander, von denen eine eine Antwort will, sind eine zu
    // viel.
    if let Some(f) = l.frage.clone() {
        // ⚑ **Einmal und dann still.** Eine Frage, die sich jeden Takt
        // neu schreibt, blinkt, kostet Bandbreite auf einer fernen
        // Sitzung und laesst nichts mehr ruhen: **Wer wartet, soll
        // warten duerfen.**
        if !l.gestellt {
            l.gestellt = true;
            let _ = write!(
                aus,
                "\r\x1b[2K{f}\r\n\r\x1b[2K{}  ausfuehren? [j] ja   [n] nein{}",
                crossterm::style::SetForegroundColor(design::toene(bild).beiwerk),
                crossterm::style::ResetColor
            );
            let _ = aus.flush();
        }
        return;
    }

    // ⚑ **Solange Token fliessen, gehoert die Zeile ihnen.** Die
    // Statuszeile beginnt mit `\r` und raeumte weg, was gerade
    // entsteht; sie kommt zurueck, sobald der Schritt zu Ende ist.
    if !l.strom.is_empty() {
        let stuecke = std::mem::take(&mut l.strom);
        if l.details {
            if !l.strom_offen {
                let _ = write!(aus, "\r\x1b[2K");
                if absatz_noetig(l.zuletzt, Block::Text) {
                    let _ = write!(aus, "\r\n");
                }
            }
            l.zuletzt = Some(Block::Text);
            // ⚑ **Gedaempft, denn es ist der Rohstrom.** Danach steht
            // dieselbe Antwort noch einmal da, gesetzt und geordnet;
            // **ohne den Unterschied in der Farbe liest sich das wie
            // eine Wiederholung und nicht wie Live und Ergebnis.** Das
            // Denken steht zusaetzlich kursiv: Es ist nicht die Antwort.
            //
            // Im Rohmodus braucht jeder Umbruch seinen Wagenruecklauf.
            for (denken, text) in &stuecke {
                let stil = if *denken { rollen.gedanke } else { rollen.beiwerk };
                let _ = write!(aus, "{}", stil.faerben(&text.replace('\n', "\r\n"), farbig));
            }
            let _ = aus.flush();
            l.strom_offen = !stuecke.last().is_some_and(|(_, t)| t.ends_with('\n'));
        }
        if l.strom_offen {
            return;
        }
    }
    if l.strom_offen {
        return;
    }

    let seit = l.anfang.elapsed();
    let (hinein, heraus) = zaehler.stand();
    let kopf = ladekopf(l.rad, l.spruch, takt);
    let schweif = ladeschweif(seit, hinein, heraus, &l.was, l.details);

    let Some(sch) = schirm else {
        return;
    };
    let r = Rahmen::messen();
    let breite = r.einzug.chars().count() + r.innen + 2;
    let luft = breite.saturating_sub(kopf.chars().count() + schweif.chars().count()) / 2;

    // ⚑ **Gespeichert und zurueckgeholt.** Die Ladezeile steht
    // ausserhalb des Rollbereichs; wer von dort weiterschriebe,
    // schriebe in den Rahmen.
    let _ = write!(aus, "\x1b7{}{}", sch.zeile(Schirm::LADEZEILE), " ".repeat(luft));
    // ⚑ **Die Welle laeuft durch die Buchstaben** (Festlegung des
    // Projektinhabers): Jedes Zeichen bekommt seine Farbe ein Stueck
    // spaeter als das davor, und dadurch wandert der Farbverlauf
    // waagerecht durch den Text.
    //
    // ⚑ **Nur der Kopf.** Zeit, Token und Taetigkeit sind Angaben und
    // keine Zierde; sie bleiben im Ton des Designs lesbar.
    for (i, c) in kopf.chars().enumerate() {
        let versatz = seit + WELLE * i as u32;
        let _ = write!(
            aus,
            "{}{c}",
            crossterm::style::SetForegroundColor(design::ladefarbe(bild, versatz))
        );
    }
    let _ = write!(
        aus,
        "{}{schweif}{}\x1b8",
        crossterm::style::SetForegroundColor(design::toene(bild).beiwerk),
        crossterm::style::ResetColor
    );
    let _ = aus.flush();
}

/// **Die Zeile selbst**, und sie ist rein: Dieselben Angaben ergeben
/// dieselbe Zeile, und deshalb laesst sie sich pruefen.
pub fn ladekopf(welches: usize, spruch: &str, takt: usize) -> String {
    let scheibe = RAEDER[welches % RAEDER.len()];
    let rad = scheibe[takt % scheibe.len()];
    format!("{rad} {spruch}…")
}

/// **Was hinter dem Ladetext steht: Zeit, Token, Taetigkeit, Schalter.**
pub fn ladeschweif(
    seit: Duration,
    hinein: u32,
    heraus: u32,
    was: &str,
    details: bool,
) -> String {
    let schalter = if details { "verbirgt" } else { "zeigt" };
    format!(
        "  ({} · {} · {was})   {SCHALTER} {schalter}, was laeuft",
        dauer(seit),
        tokenangabe(hinein, heraus)
    )
}

/// **Gelesene und geschriebene Token, kurz.**
///
/// ⚑ **Erst ab tausend in `k`** (Wunsch des Projektinhabers: „X.Xk
/// tokens"). Darunter waere `0.1k` eine Angabe, die weniger sagt als
/// die Zahl selbst, und bei den ersten Token stuende dort `0.0k`.
pub fn tokenangabe(hinein: u32, heraus: u32) -> String {
    fn kurz(n: u32) -> String {
        if n < 1000 {
            n.to_string()
        } else {
            format!("{:.1}k", n as f64 / 1000.0)
        }
    }
    format!("↑{} ↓{} tokens", kurz(hinein), kurz(heraus))
}

/// **Die Farbe des Ladetextes, linear durch den Regenbogen.**
///
/// ⚑ **Ganzzahlig gerechnet**, und das ist hier keine Pflicht, sondern
/// die Hausart: Der Farbkreis wird in sechs Abschnitte zu 256 Schritten
/// geteilt, und in jedem Abschnitt laeuft genau ein Kanal hinauf oder
/// hinunter. **Dieselbe Zeit gibt dieselbe Farbe**, und die Uebergaenge
/// sind stetig, weil das Ende eines Abschnitts der Anfang des naechsten
/// ist.
///
/// ⚠️ Nach `REGENBOGEN` faengt es von vorne an, und zwar bei derselben
/// Farbe: 1536 Schritte, danach wieder null.
pub fn regenbogen(seit: Duration) -> (u8, u8, u8) {
    const SCHRITTE: u128 = 1536;
    let takt = (seit.as_millis() * SCHRITTE / REGENBOGEN.as_millis().max(1)) % SCHRITTE;
    let rest = (takt % 256) as u8;
    match takt / 256 {
        0 => (255, rest, 0),
        1 => (255 - rest, 255, 0),
        2 => (0, 255, rest),
        3 => (0, 255 - rest, 255),
        4 => (rest, 0, 255),
        _ => (255, 0, 255 - rest),
    }
}

/// Wie lange schon, in einer Form, die niemand nachrechnen muss.
pub fn dauer(d: Duration) -> String {
    let s = d.as_secs();
    if s < 60 {
        format!("{s}s")
    } else {
        format!("{}m {:02}s", s / 60, s % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Die Zeile sagt alle drei Dinge**, die sie sagen soll.
    #[test]
    fn die_zeile_nennt_zeit_taetigkeit_und_schalter() {
        let z = format!(
            "{}{}",
            ladekopf(0, "warping bytes", 0),
            ladeschweif(Duration::from_secs(12), 1234, 42, "thinking", false)
        );
        assert!(z.contains("warping bytes"), "{z}");
        assert!(z.contains("12s"), "die Zeit fehlt: {z}");
        assert!(z.contains("↑1.2k"), "die gelesenen Token fehlen: {z}");
        assert!(z.contains("↓42"), "die geschriebenen Token fehlen: {z}");
        assert!(z.contains("thinking"), "die Taetigkeit fehlt: {z}");
        assert!(z.contains(SCHALTER), "der Schalter fehlt: {z}");
    }

    /// **Und sie sagt, in welche Richtung der Schalter wirkt.**
    ///
    /// ⚠️ Ein Hinweis, der immer „zeigt" sagt, ist falsch, sobald
    /// jemand ihn benutzt hat.
    #[test]
    fn der_schalter_sagt_seine_richtung() {
        let aus = ladeschweif(Duration::ZERO, 0, 0, "thinking", false);
        let an = ladeschweif(Duration::ZERO, 0, 0, "thinking", true);
        assert!(aus.contains("zeigt, was laeuft"), "{aus}");
        assert!(an.contains("verbirgt, was laeuft"), "{an}");
    }

    /// **Ueber einer Minute wird aus Sekunden eine Minutenangabe.**
    #[test]
    fn die_dauer_bleibt_lesbar() {
        assert_eq!(dauer(Duration::from_secs(0)), "0s");
        assert_eq!(dauer(Duration::from_secs(59)), "59s");
        assert_eq!(dauer(Duration::from_secs(60)), "1m 00s");
        assert_eq!(dauer(Duration::from_secs(3671)), "61m 11s");
    }

    /// Der sichtbare Text, ohne Steuerzeichen.
    fn sichtbar(s: &str) -> String {
        let mut aus = String::new();
        let mut in_folge = false;
        for c in s.chars() {
            if c == '\x1b' {
                in_folge = true;
            } else if in_folge {
                if c.is_ascii_alphabetic() {
                    in_folge = false;
                }
            } else {
                aus.push(c);
            }
        }
        aus
    }

    /// **Gesetzt steht derselbe Text da**, in jeder Art, und ohne Farbe
    /// bleibt die Zeile Zeichen fuer Zeichen, wie sie war.
    #[test]
    fn die_zeitleiste_behaelt_ihren_text() {
        let r = design::rollen(myl_client::einstellungen::Konsolendesign::Myelith);
        for (art, z) in [
            (Art::Schritt, "  ● Schritt 3 · 12s"),
            (Art::Aufruf, "  → datei_lesen src/main.rs"),
            (Art::Ergebnis, "  ← datei_lesen 120 Zeilen"),
            (Art::Abgelehnt, "  ⚑ datei_schreiben abgelehnt: ausserhalb der Einhaengung"),
            (Art::Hinweis, "  ⚑ Kontext verdichtet: 9000 → 3000 Token"),
            (Art::Aufruf, "  → ohne_argumente"),
        ] {
            assert_eq!(zeile_setzen(art, z, &r, false), z);
            assert_eq!(sichtbar(&zeile_setzen(art, z, &r, true)), z, "{art:?}");
        }
    }

    /// **Hervorgehoben wird, was der Agent tut**: Pfeil und Werkzeug im
    /// Stil der Aktion, eine Rueckgabe im Stil des Ergebnisses, eine
    /// Ablehnung samt ihrem Wort als Warnung.
    #[test]
    fn die_zeitleiste_hebt_die_handlung_hervor() {
        let r = design::rollen(myl_client::einstellungen::Konsolendesign::Tiefsee);
        let aufruf = zeile_setzen(Art::Aufruf, "  → datei_lesen a.rs", &r, true);
        assert!(aufruf.contains(&r.aktion.faerben("→ datei_lesen", true)), "{aufruf:?}");
        let rueck = zeile_setzen(Art::Ergebnis, "  ← datei_lesen 3 Zeilen", &r, true);
        assert!(rueck.contains(&r.ergebnis.faerben("← datei_lesen", true)), "{rueck:?}");
        assert!(rueck.contains(&r.beiwerk.faerben(" 3 Zeilen", true)), "{rueck:?}");
        let nein = zeile_setzen(Art::Abgelehnt, "  ⚑ x abgelehnt: weil", &r, true);
        assert!(nein.contains(&r.warnung.faerben("⚑ x abgelehnt:", true)), "{nein:?}");
    }

    /// **Ein Absatz beim Wechsel, keiner innerhalb einer Art**, und keiner
    /// vor dem allerersten Stueck.
    #[test]
    fn ein_absatz_steht_nur_beim_wechsel() {
        assert!(!absatz_noetig(None, Block::Handlung));
        assert!(!absatz_noetig(None, Block::Text));
        assert!(!absatz_noetig(Some(Block::Handlung), Block::Handlung));
        assert!(!absatz_noetig(Some(Block::Text), Block::Text));
        assert!(absatz_noetig(Some(Block::Handlung), Block::Text));
        assert!(absatz_noetig(Some(Block::Text), Block::Handlung));
    }

    /// **Der naechste Spruch ist nie der vorige.**
    ///
    /// 📌 Die Gegenprobe: Ein Wurf, der zufaellig zweimal dieselbe Zahl
    /// liefert, waere ein Ladetext, der bei einem Schrittwechsel
    /// stehenbleibt, und dann sieht der Wechsel aus wie ein Haenger.
    #[test]
    fn kein_spruch_kommt_zweimal_hintereinander() {
        let mut l = Lage {
            was: String::new(),
            spruch: SPRUECHE[0],
            rad: 0,
            anfang: Instant::now(),
            offen: Vec::new(),
            gezaehlt: 0,
            details: false,
            strom: Vec::new(),
            zuletzt: None,
            strom_offen: false,
            saat: 1,
            abbruchfrage: false,
            abbruch_gestellt: false,
            frage: None,
            gestellt: false,
            antwort: None,
        };
        for _ in 0..200 {
            let vorher = l.spruch;
            l.neuer_spruch();
            assert_ne!(vorher, l.spruch, "derselbe Spruch zweimal");
        }
    }

    /// **Und mit der Zeit kommen alle vor.**
    #[test]
    fn jeder_spruch_kommt_irgendwann_dran() {
        let mut l = Lage {
            was: String::new(),
            spruch: SPRUECHE[0],
            rad: 0,
            anfang: Instant::now(),
            offen: Vec::new(),
            gezaehlt: 0,
            details: false,
            strom: Vec::new(),
            zuletzt: None,
            strom_offen: false,
            saat: 7,
            abbruchfrage: false,
            abbruch_gestellt: false,
            frage: None,
            gestellt: false,
            antwort: None,
        };
        let mut gesehen = std::collections::BTreeSet::new();
        for _ in 0..500 {
            l.neuer_spruch();
            gesehen.insert(l.spruch);
        }
        assert_eq!(gesehen.len(), SPRUECHE.len(), "nicht jeder Spruch kam dran");
    }
}

/// Der Griff, mit dem eine schreibende Handlung vorgelegt wird.
#[derive(Clone)]
pub struct Frager {
    lage: Arc<Mutex<Lage>>,
    laeuft: Arc<AtomicBool>,
    am_schirm: bool,
}

impl Frager {
    /// **Legt eine schreibende Handlung vor und wartet auf die
    /// Antwort.**
    ///
    /// ⚑ **Gefragt wird vom Faden, nicht von hier.** Es gibt genau
    /// einen Leser der Tastatur, solange ein Lauf laeuft; zwei ergaeben
    /// verschluckte Tasten, und zwar unregelmaessig. Der Hauptfaden legt
    /// die Frage nur hin und wartet.
    ///
    /// ⚠️ **Ohne Schirm wird abgelehnt.** In einer Roehre gibt es
    /// niemanden, der bestaetigen koennte; ein stilles Ja waere genau
    /// das, was der `manual mode` verhindern soll.
    pub fn fragen(&self, text: &str) -> bool {
        if !self.am_schirm {
            println!("  ⚑ {text}: abgelehnt, denn hier ist niemand, der bestaetigen kann.");
            return false;
        }
        {
            let Ok(mut l) = self.lage.lock() else { return false };
            l.frage = Some(text.to_string());
            l.gestellt = false;
            l.antwort = None;
        }
        loop {
            std::thread::sleep(Duration::from_millis(20));
            let Ok(mut l) = self.lage.lock() else { return false };
            if let Some(a) = l.antwort.take() {
                l.frage = None;
                l.gestellt = false;
                return a;
            }
            if !self.laeuft.load(Ordering::Relaxed) {
                return false;
            }
        }
    }
}

/// Der Griff, durch den die Token des Modells hereinkommen.
///
/// ⚑ **Er schreibt nicht selbst.** Wie der Melder legt er nur ab; der
/// Anzeigefaden ist und bleibt der einzige Schreiber.
#[derive(Clone)]
pub struct Stromgriff {
    lage: Arc<Mutex<Lage>>,
    am_schirm: bool,
}

impl Stromgriff {
    /// Ein Stueck Ausgabe des Modells.
    ///
    /// ⚑ **Denken und Antwort sind schon getrennt**, und zwar dort, wo
    /// der Strom zerlegt wird. Hier wird nur noch gezeigt: das Denken
    /// eingerueckt, damit man beides auseinanderhaelt.
    pub fn stueck(&self, s: myl_client::strom::Stueck) {
        if !self.am_schirm {
            return;
        }
        let Ok(mut l) = self.lage.lock() else { return };
        match s {
            myl_client::strom::Stueck::Denken(t) => l.strom.push((true, t)),
            myl_client::strom::Stueck::Text(t) => l.strom.push((false, t)),
        }
    }
}

#[cfg(test)]
mod abbruchprobe {
    /// ⚑ **Der Abbruch mitten im Lauf fragt nach, und zwar auf beiden
    /// Tasten** (Auftrag des Projektinhabers, 2026-09-15).
    ///
    /// 📌 Vorher beendete Strg-C den Prozess mitten in einer Erzeugung
    /// **ohne ein Wort** (`abbrechen` ruft `exit(130)`). Wer sich
    /// vertippte, verlor den Lauf.
    #[test]
    fn beide_tasten_fragen_statt_zu_beenden() {
        let quelle = include_str!("anzeige.rs");
        assert!(
            quelle.contains("KeyCode::Char('c') | KeyCode::Char('x') => abbruchfrage_stellen(&lage),"),
            "Strg-C oder Strg-X beendet noch ohne Frage"
        );
        // Und beendet wird nur aus der Antwort heraus.
        let antwortzweig = quelle
            .split("if offene_abbruchfrage(&lage) {")
            .nth(1)
            .expect("die offene Abbruchfrage wird nicht zuerst behandelt");
        assert!(
            antwortzweig.contains("abbrechen(schirm);"),
            "eine bejahte Abbruchfrage beendet nicht: {antwortzweig:.200}"
        );
    }

    /// ⛔️ **Die Abbruchfrage und die Vorlage im `manual mode` sind zwei
    /// Fragen.** Ein Feld fuer beide, und ein Tastendruck haette die eine
    /// mit der Antwort auf die andere beantwortet.
    #[test]
    fn die_abbruchfrage_hat_ihren_eigenen_zustand() {
        let quelle = include_str!("anzeige.rs");
        assert!(quelle.contains("abbruchfrage: bool,"), "kein eigener Zustand");
        // Die Vorlage geht vor: Wer gerade gefragt wird, antwortet zuerst
        // darauf.
        assert!(
            quelle.contains("if l.abbruchfrage && l.frage.is_none() {"),
            "die Abbruchfrage verdraengt die Vorlage"
        );
    }
}
