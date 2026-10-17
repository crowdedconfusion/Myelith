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
    /// **Die Notausfrage, mit ihrer Vorgabe fuer die Eingabetaste.**
    ///
    /// ⚑ Festlegung des Projektinhabers (2026-09-26): Strg-C fragt
    /// „[J/n]", die Eingabetaste heisst ja; Esc fragt „[j/N]", die
    /// Eingabetaste heisst nein. Eine gestreifte Esc haelt also nichts an.
    notausfrage: Option<bool>,
    /// Ob sie schon dasteht.
    notaus_gestellt: bool,
    /// Was gerade vorgelegt wird, im `manual mode`.
    frage: Option<String>,
    /// Ob sie schon dasteht.
    gestellt: bool,
    /// Die Antwort, sobald eine Taste kam.
    antwort: Option<bool>,
    /// ⚑ **Wer getippte Befehle beantwortet**, gesetzt vom Aufrufer
    /// ([`Anzeige::befehle_zulassen`]). Ohne ihn gibt es eine Antwort, die
    /// sagt, dass waehrend dieses Laufs kein Befehl geht.
    befehle: Option<Arc<Befehlsgriff>>,
    /// **Was in der Eingabezeile steht**, waehrend der Lauf laeuft
    /// (Wunsch des Projektinhabers, 2026-10-12: der Wagen bleibt in der
    /// Eingabezeile, und Befehle gehen jederzeit).
    getippt: String,
    /// Die Frage vor `auto` ist offen (Umschalt-Tab im Lauf).
    modusfrage: bool,
    /// Ob sie schon dasteht.
    modusfrage_gestellt: bool,
    /// Die Saetze der Sicherheitsmeldung vor `auto`, und wer danach die
    /// Fusszeile zeichnet; ohne beides schaltet Umschalt-Tab hier nichts.
    moduswechsel: Option<(Vec<String>, Arc<Modushaken>)>,
}

/// **Ob gerade `auto` gilt**, prozessweit und waehrend eines Laufs
/// umschaltbar (Umschalt-Tab, Wunsch des Projektinhabers, 2026-10-12).
///
/// ⚑ **Die Nachfrage im `manual mode` liest ihn bei jeder Handlung**, nicht
/// beim Start des Laufs: So gilt ein Wechsel sofort, auch mitten in einer
/// Runde. Die Konsole setzt ihn vor jedem Lauf aus ihrem Stand und liest
/// ihn danach zurueck.
///
/// ⛔️ **Der Schalter liegt im Agenten** (`myl_local_agent::ruestung::ohne_mensch`,
/// Fund 535): Der Informationsfluss muss wissen, dass im auto mode kein
/// Mensch freigibt, und ein zweiter Schalter hier liefe auseinander.
pub fn auto_an() -> bool {
    myl_local_agent::ruestung::ohne_mensch()
}

/// Setzt den Modus fuer die Nachfrage und fuer den Informationsfluss.
pub fn auto_setzen(an: bool) {
    myl_local_agent::ruestung::ohne_mensch_setzen(an);
}

/// Wer nach einem Moduswechsel die Fusszeile neu zeichnet; bekommt `true`
/// fuer `auto`.
pub type Modushaken = dyn Fn(bool) + Send + Sync;

/// **Was ein getippter Befehl waehrend eines Laufs bewirkt.**
pub enum Befehlsfolge {
    /// Den Lauf anhalten, wie ein bestaetigtes Esc (`/stop loop`).
    Anhalten,
    /// Zeilen in die Zeitleiste, und der Lauf geht weiter.
    Zeilen(Vec<String>),
}

/// Wer einen getippten Befehl beantwortet; er bekommt die ganze Zeile.
pub type Befehlsgriff = dyn Fn(&str) -> Befehlsfolge + Send + Sync;

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
    ///
    /// `halt` ist, was Esc tut, solange keine Frage offen ist: im
    /// Auftrag der Notaus, im Loop die Pause.
    pub fn starten(
        schirm: Option<Schirm>,
        bild: myl_client::einstellungen::Konsolendesign,
        zaehler: std::sync::Arc<myl_client::oertlich::Tokenzaehler>,
        halt: fn(),
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
            notausfrage: None,
            notaus_gestellt: false,
            frage: None,
            gestellt: false,
            antwort: None,
            befehle: None,
            getippt: String::new(),
            modusfrage: false,
            modusfrage_gestellt: false,
            moduswechsel: None,
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
        // ⚑ **Der Wagen bleibt in der Eingabezeile** (Wunsch des
        //   Projektinhabers, 2026-10-12). Das Terminal hat nur einen, und
        //   der muss dort stehen, wo die Ausgabe hinschreibt; deshalb wird
        //   er versteckt, und in der Eingabezeile steht ein gezeichneter.
        if let Some(sch) = schirm {
            crate::schirm::wagen_verstecken();
            crate::schirm::einfuegen_geklammert(true);
            tippzeile_zeichnen(&sch, bild, "");
        }
        let faden = am_schirm.then(|| {
            let lage = Arc::clone(&lage);
            let laeuft = Arc::clone(&laeuft);
            std::thread::spawn(move || takten(lage, laeuft, schirm, bild, zaehler, halt))
        });
        Self { lage, laeuft, faden, am_schirm }
    }

    /// **Umschalt-Tab wechselt den Modus auch waehrend des Laufs.** Vor
    /// `auto` stehen die Saetze der Sicherheitsmeldung (`warnung`) und eine
    /// Ja-Nein-Frage; zurueck nach `manual` geht ohne Frage. `haken`
    /// zeichnet danach die Fusszeile.
    pub fn moduswechsel_zulassen(&self, warnung: Vec<String>, haken: Arc<Modushaken>) {
        if let Ok(mut l) = self.lage.lock() {
            l.moduswechsel = Some((warnung, haken));
        }
    }

    /// **Wer getippte Befehle beantwortet**, solange der Lauf laeuft.
    /// Getippt wird in jedem Lauf in die Eingabezeile; Eingabe schickt die
    /// Zeile an `griff`, und dessen [`Befehlsfolge`] gilt.
    pub fn befehle_zulassen(&self, griff: Arc<Befehlsgriff>) {
        if let Ok(mut l) = self.lage.lock() {
            l.befehle = Some(griff);
        }
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

    /// **Der Agent hat alte Werkzeugausgaben ausgeblendet**, statt den
    /// Verlauf zusammenzufassen.
    pub fn ausgeblendet(&self, ergebnisse: usize, vorher: usize, nachher: usize) {
        let z = format!("  ⚑ Kontext entlastet: {ergebnisse} alte Werkzeugausgaben ausgeblendet, {vorher} → {nachher} Token");
        self.merken(Zeitzeile { art: Art::Hinweis, kurz: Some(z.clone()), voll: z }, Some("ausgeblendet".to_string()));
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
            crate::schirm::wagen_zeigen();
            crate::schirm::einfuegen_geklammert(false);
        }
        let l = self.lage.lock();
        match l {
            Ok(l) => (l.details, l.gezaehlt),
            Err(_) => (false, 0),
        }
    }
}

/// Der Faden: Taste lesen, Zeilen nachreichen, Zeile zeichnen.
///
/// ⛔️ **Er ist der einzige Leser der Tastatur, solange ein Lauf laeuft**
/// (Fund 483). `event::poll` und `event::read` nehmen die Lesersperre von
/// crossterm je einzeln; zwei Faeden, die beide pollen, sehen dieselbe
/// Taste, einer liest sie, und der andere bleibt in `event::read` stehen,
/// **mit der Sperre in der Hand**, bis irgendwann eine weitere Taste
/// kommt. Genau so stand die Konsole nach einem bestaetigten `j` im
/// `manual mode`: Der Hauptfaden wartete am Ende auf den Notauswaechter,
/// der Notauswaechter auf eine Taste, dieser Faden auf die Sperre.
///
/// 📌 **Zwei Leser derselben Quelle sind kein Wettlauf, der manchmal
/// verloren wird, sondern einer, der immer verloren wird, sobald eine
/// Taste kommt.** Wer waehrend eines Laufs eine weitere Taste braucht,
/// haengt sie hier ein, statt einen eigenen Faden zu starten.
fn takten(
    lage: Arc<Mutex<Lage>>,
    laeuft: Arc<AtomicBool>,
    schirm: Option<Schirm>,
    bild: myl_client::einstellungen::Konsolendesign,
    zaehler: std::sync::Arc<myl_client::oertlich::Tokenzaehler>,
    halt: fn(),
) {
    let mut takt: usize = 0;
    while laeuft.load(Ordering::Relaxed) {
        // ⚑ **Die Taste wird ausserhalb der Sperre gelesen.** Sonst
        // wartete der Melder des Hauptfadens bei jedem Takt neunzig
        // Millisekunden auf eine Sperre, die ohnehin nur wartet.
        if event::poll(TAKT).unwrap_or(false) {
            match event::read() {
                // ⚑ **Eingefuegt heisst nicht abgeschickt**, auch im Lauf;
                //   solange eine Frage offen ist, wird nichts eingefuegt.
                Ok(Event::Paste(text)) => {
                    let ohne_frage = lage.lock().is_ok_and(|l| {
                        !l.abbruchfrage && l.notausfrage.is_none() && !l.modusfrage && l.frage.is_none()
                    });
                    if ohne_frage {
                        let neu = lage.lock().ok().map(|mut l| {
                            l.getippt.push_str(&crate::eingabe::einfuegetext(&text));
                            l.getippt.clone()
                        });
                        if let (Some(sch), Some(t)) = (schirm, neu) {
                            tippzeile_zeichnen(&sch, bild, &t);
                        }
                    }
                }
                Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                    // ⚑ **Eine offene Abbruchfrage geht allem vor**
                    // (Auftrag des Projektinhabers, 2026-09-15): Solange
                    // sie dasteht, ist die naechste Taste ihre Antwort
                    // und nichts anderes.
                    let strg = k.modifiers.contains(KeyModifiers::CONTROL);
                    if offene_abbruchfrage(&lage) {
                        if let Some(ja) = als_antwort(k.code) {
                            if ja {
                                abbrechen(schirm);
                            }
                            abbruchfrage_weg(&lage);
                            wagen_zurueck(&lage, schirm, bild);
                        }
                    } else if let Some(vorgabe) = offene_notausfrage(&lage) {
                        // ⛔️ **Die Antwort auf den Notaus.** Ein zweites
                        //   Strg-C heisst ja, wie man es gewohnt ist.
                        if let Some(ja) = notausantwort(k.code, strg, vorgabe) {
                            notausfrage_weg(&lage);
                            wagen_zurueck(&lage, schirm, bild);
                            if ja {
                                // Eine offene Vorlage gilt als abgelehnt,
                                // sonst wartete der Lauf auf sie.
                                beantworten(&lage, false);
                                halt();
                                haltezeile(&lage);
                            }
                        }
                    } else if offene_modusfrage(&lage) {
                        if let Some(a) = crate::janein::taste(k.code, strg, Some(false)) {
                            modusfrage_beantworten(&lage, a.ja());
                            wagen_zurueck(&lage, schirm, bild);
                        }
                    } else if strg {
                        match k.code {
                            KeyCode::Char('s') => umschalten(&lage),
                            // ⛔️ **Strg-C: Notaus, Vorgabe ja** (Festlegung
                            //   des Projektinhabers, 2026-09-26). Beenden
                            //   bleibt bei Strg-X, mit Frage.
                            KeyCode::Char('c') => notausfrage_stellen(&lage, true),
                            KeyCode::Char('x') => abbruchfrage_stellen(&lage),
                            _ => {}
                        }
                    } else if offene_frage(&lage) {
                        if let Some(ja) = als_antwort(k.code) {
                            beantworten(&lage, ja);
                            wagen_zurueck(&lage, schirm, bild);
                        }
                    } else if k.code == KeyCode::BackTab {
                        // ⚑ **Umschalt-Tab wechselt den Modus**, auch hier.
                        modus_umschalten(&lage);
                    } else if let Some(t) = getippte_taste(&lage, k.code) {
                        // ⚑ **Jede Taste gehoert der Eingabezeile**, ausser
                        //   Esc an einer leeren Zeile (der Notaus darunter).
                        match t {
                            Tipp::Geaendert(text) => {
                                if let Some(sch) = schirm {
                                    tippzeile_zeichnen(&sch, bild, &text);
                                }
                            }
                            Tipp::Abgeschickt(zeile) => {
                                if let Some(sch) = schirm {
                                    tippzeile_zeichnen(&sch, bild, "");
                                }
                                // ⚑ **Getippt ist schon bestaetigt**: Wer
                                //   `/stop loop` ausschreibt, hat sich
                                //   entschieden. Der Griff laeuft ausserhalb
                                //   der Sperre.
                                let griff = lage.lock().ok().and_then(|l| l.befehle.clone());
                                let folge = match griff {
                                    Some(g) => g(&zeile),
                                    None => Befehlsfolge::Zeilen(vec![format!(
                                        "{zeile}: Während dieses Laufs geht kein Befehl; Esc hält ihn an."
                                    )]),
                                };
                                match folge {
                                    Befehlsfolge::Anhalten => {
                                        halt();
                                        haltezeile(&lage);
                                    }
                                    Befehlsfolge::Zeilen(zeilen) => {
                                        if let Ok(mut l) = lage.lock() {
                                            for z in zeilen {
                                                let satz = format!("  · {z}");
                                                l.offen.push(Zeitzeile { art: Art::Hinweis, kurz: Some(satz.clone()), voll: satz });
                                            }
                                        }
                                    }
                                }
                            }
                            Tipp::Nichts => {}
                        }
                    } else if k.code == KeyCode::Esc {
                        // ⛔️ **Esc: Notaus, Vorgabe nein** (im Loop: die
                        //   Pause). Steht eine Vorlage da, ist Esc ihr Nein.
                        notausfrage_stellen(&lage, false);
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

/// Ob eine Vorlage im `manual mode` auf Antwort wartet.
fn offene_frage(lage: &Arc<Mutex<Lage>>) -> bool {
    lage.lock().map(|l| l.frage.is_some()).unwrap_or(false)
}

/// Sagt in der Leiste, dass angehalten wird: Bis die laufende Erzeugung
/// die Marke sieht, vergeht ein Token, und so lange soll niemand ein
/// zweites Mal druecken muessen, um zu sehen, dass die erste ankam.
fn haltezeile(lage: &Arc<Mutex<Lage>>) {
    if let Ok(mut l) = lage.lock() {
        l.offen.push(Zeitzeile { art: Art::Hinweis, kurz: Some("  · Esc: wird angehalten".into()), voll: "  · Esc: wird angehalten".into() });
    }
}

/// Der Befehl, der einen laufenden Loop anhaelt.
pub const STOP_LOOP: &str = "/stop loop";

/// Ist diese Zeile [`STOP_LOOP`]? Gross, klein und Leerraum zaehlen nicht.
pub fn ist_stop_loop(zeile: &str) -> bool {
    zeile.split_whitespace().collect::<Vec<_>>().join(" ").eq_ignore_ascii_case(STOP_LOOP)
}

/// Was eine Taste mit der Eingabezeile gemacht hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tipp {
    /// Die Taste gehoert nicht der Zeile (Esc an einer leeren Zeile, eine
    /// Pfeiltaste); wer sie sonst braucht, nimmt sie.
    Nichts,
    /// Die Zeile hat jetzt diesen Text.
    Geaendert(String),
    /// Eingabe: diese Zeile ist abgeschickt, die Eingabezeile wieder leer.
    Abgeschickt(String),
}

/// **Eine Taste fuer die Eingabezeile eines Laufs.** Zeichen und
/// Ruecktaste bearbeiten sie, Esc leert sie, Eingabe schickt sie ab.
///
/// ⚑ **Rein**, damit sie sich ohne Terminal pruefen laesst; die Konsole
/// nimmt sie in jedem Lauf und beim Warten zwischen zwei Runden.
/// ⚑ **Esc an einer leeren Zeile gehoert nicht ihr**, sondern dem Notaus;
/// eine leere Eingabe schickt nichts ab.
pub fn tippen(puffer: &mut String, code: KeyCode) -> Tipp {
    match code {
        KeyCode::Char(c) => {
            puffer.push(c);
            Tipp::Geaendert(puffer.clone())
        }
        KeyCode::Backspace if !puffer.is_empty() => {
            puffer.pop();
            Tipp::Geaendert(puffer.clone())
        }
        KeyCode::Esc if !puffer.is_empty() => {
            puffer.clear();
            Tipp::Geaendert(String::new())
        }
        KeyCode::Enter if !puffer.trim().is_empty() => Tipp::Abgeschickt(std::mem::take(puffer).trim().to_string()),
        _ => Tipp::Nichts,
    }
}

/// Ist die Frage vor `auto` offen?
fn offene_modusfrage(lage: &Arc<Mutex<Lage>>) -> bool {
    lage.lock().is_ok_and(|l| l.modusfrage)
}

/// **Umschalt-Tab im Lauf.** Von `auto` nach `manual` sofort; nach `auto`
/// erst die Sicherheitsmeldung und die Frage.
fn modus_umschalten(lage: &Arc<Mutex<Lage>>) {
    let Ok(mut l) = lage.lock() else { return };
    let Some((warnung, haken)) = l.moduswechsel.clone() else {
        let satz = "  · Den Modus wechselt Umschalt-Tab erst nach diesem Lauf.".to_string();
        l.offen.push(Zeitzeile { art: Art::Hinweis, kurz: Some(satz.clone()), voll: satz });
        return;
    };
    if auto_an() {
        auto_setzen(false);
        let satz = "  · manual mode: Jede Handlung mit Wirkung nach außen wird wieder vorgelegt.".to_string();
        l.offen.push(Zeitzeile { art: Art::Hinweis, kurz: Some(satz.clone()), voll: satz });
        drop(l);
        haken(false);
        return;
    }
    for w in warnung {
        let satz = format!("  ⚠️  {w}");
        l.offen.push(Zeitzeile { art: Art::Hinweis, kurz: Some(satz.clone()), voll: satz });
    }
    l.modusfrage = true;
    l.modusfrage_gestellt = false;
}

/// Die Antwort auf die Frage vor `auto`.
fn modusfrage_beantworten(lage: &Arc<Mutex<Lage>>, ja: bool) {
    let haken = {
        let Ok(mut l) = lage.lock() else { return };
        l.modusfrage = false;
        l.modusfrage_gestellt = false;
        l.gestellt = false;
        let satz = if ja {
            "  · auto mode: Der Agent handelt ab jetzt ohne Rückfrage."
        } else {
            "  · Es bleibt beim manual mode."
        }
        .to_string();
        l.offen.push(Zeitzeile { art: Art::Hinweis, kurz: Some(satz.clone()), voll: satz });
        l.moduswechsel.as_ref().map(|(_, h)| Arc::clone(h))
    };
    if ja {
        auto_setzen(true);
        if let Some(h) = haken {
            h(true);
        }
    }
}

/// **Nach einer Ja-Nein-Frage**: der Wagen wieder in die Eingabezeile,
/// mit dem, was dort schon getippt war.
fn wagen_zurueck(lage: &Arc<Mutex<Lage>>, schirm: Option<Schirm>, bild: myl_client::einstellungen::Konsolendesign) {
    if let (Some(sch), Ok(l)) = (schirm, lage.lock()) {
        tippzeile_zeichnen(&sch, bild, &l.getippt);
    }
}

/// Gibt die Taste der Eingabezeile; `None`, wenn sie ihr nicht gehoert.
fn getippte_taste(lage: &Arc<Mutex<Lage>>, code: KeyCode) -> Option<Tipp> {
    let mut l = lage.lock().ok()?;
    match tippen(&mut l.getippt, code) {
        Tipp::Nichts => None,
        t => Some(t),
    }
}

/// **Zeichnet die Eingabezeile eines Laufs**, samt gezeichnetem Wagen, und
/// laesst den echten dort, wo die Ausgabe weiterschreibt.
///
/// ⚑ **Der Wagen ist eine umgekehrt gesetzte Stelle hinter dem Text.** Der
/// echte ist versteckt ([`crate::schirm::wagen_verstecken`]), denn er muss
/// im Rollbereich stehen, wo die Antwort entsteht.
pub fn tippzeile_zeichnen(sch: &Schirm, bild: myl_client::einstellungen::Konsolendesign, text: &str) {
    eingabezeile(sch, bild, text, true);
}

/// **Die Eingabezeile ohne Wagen**, solange eine Ja-Nein-Frage offen ist
/// (Festlegung des Projektinhabers, 2026-10-12): Dann blinkt die Frage,
/// und in der Eingabezeile wird nichts getippt.
pub fn eingabezeile_ohne_wagen(sch: &Schirm, bild: myl_client::einstellungen::Konsolendesign) {
    eingabezeile(sch, bild, "", false);
}

fn eingabezeile(sch: &Schirm, bild: myl_client::einstellungen::Konsolendesign, text: &str, wagen: bool) {
    let r = Rahmen::messen();
    let t = design::toene(bild);
    let mut aus = std::io::stdout();
    let _ = write!(
        aus,
        "\x1b7{}{}{}│{}{}{}│{}\x1b8",
        sch.zeile(Schirm::KASTEN + 1),
        crate::ansi::SetForegroundColor(t.kante),
        r.einzug,
        crate::ansi::ResetColor,
        eingabeinnen(r.innen, text, wagen),
        crate::ansi::SetForegroundColor(t.kante),
        crate::ansi::ResetColor
    );
    let _ = aus.flush();
}

/// **Was zwischen den beiden Strichen der Eingabezeile steht**, genau
/// `innen` Spalten breit: ein Leerzeichen, das Ende des Textes, der Wagen
/// (umgekehrt gesetzt) oder ein Leerzeichen, die Luft.
///
/// 📌 Hier stand die Luft als `innen - (Text + 3)`, und die Zeile war eine
/// Spalte zu schmal: Der rechte Strich stand links von den Ecken darueber
/// und darunter (gemeldet vom Projektinhaber am 2026-10-12, mit einem Bild).
pub fn eingabeinnen(innen: usize, text: &str, wagen: bool) -> String {
    let sicht = crate::eingabe::sichtbar(text, innen.saturating_sub(3));
    let luft = innen.saturating_sub(sicht.chars().count() + 2);
    format!(" {sicht}{}{}", if wagen { "\x1b[7m \x1b[27m" } else { " " }, " ".repeat(luft))
}

/// Die offene Notausfrage und ihre Vorgabe.
fn offene_notausfrage(lage: &Arc<Mutex<Lage>>) -> Option<bool> {
    lage.lock().ok().and_then(|l| l.notausfrage)
}

fn notausfrage_stellen(lage: &Arc<Mutex<Lage>>, vorgabe: bool) {
    if let Ok(mut l) = lage.lock() {
        l.notausfrage = Some(vorgabe);
        l.notaus_gestellt = false;
    }
}

/// Nimmt sie zurueck; was sie verdraengt hatte, wird neu gezeichnet.
fn notausfrage_weg(lage: &Arc<Mutex<Lage>>) {
    if let Ok(mut l) = lage.lock() {
        l.notausfrage = None;
        l.notaus_gestellt = false;
        l.gestellt = false;
    }
}

/// **Welche Taste die Notausfrage wie beantwortet.** `None`: keine Antwort.
pub fn notausantwort(code: KeyCode, strg: bool, vorgabe: bool) -> Option<bool> {
    match code {
        KeyCode::Char('c') if strg => Some(true),
        _ if strg => None,
        KeyCode::Char('j' | 'J' | 'y' | 'Y') => Some(true),
        KeyCode::Char('n' | 'N') | KeyCode::Esc => Some(false),
        KeyCode::Enter => Some(vorgabe),
        _ => None,
    }
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
    myl_client::mining::beenden_wenn_eigen();
    crate::schirm::rollbereich_freigeben();
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
    // ⚑ **Die Meldung des Minings auch mitten im Lauf**, alle fuenf Minuten
    //   (Auftrag des Projektinhabers, 2026-10-17), in der Zeitleiste und als
    //   Hinweis gesetzt; sonst laege sie hinter einem langen Loop.
    if !l.strom_offen {
        if let Some(zeilen) = crate::miningzeile::faellig() {
            let _ = write!(aus, "\r\x1b[2K");
            for z in zeilen {
                let _ = write!(aus, "{}\r\n", zeile_setzen(Art::Hinweis, &format!("  {z}"), &rollen, farbig));
            }
            l.zuletzt = Some(Block::Handlung);
        }
    }

    // ⛔️ **Die Notausfrage geht allem vor**, auch einer Vorlage: Wer den
    //   Notaus will, soll nicht erst eine andere Frage beantworten muessen.
    if let Some(vorgabe) = l.notausfrage {
        if !l.notaus_gestellt {
            l.notaus_gestellt = true;
            let wahl = if vorgabe { "[J/n]" } else { "[j/N]" };
            if let Some(sch) = schirm {
                eingabezeile_ohne_wagen(&sch, bild);
            }
            let _ = write!(
                aus,
                "\r\x1b[2K  {}",
                crate::janein::hervor(&format!("Notaus: anhalten? {wahl}"), Some(bild))
            );
            let _ = aus.flush();
        }
        return;
    }

    // ⚑ **Die Frage vor `auto`** (Umschalt-Tab im Lauf): blinkend, wie
    //   jede Ja-Nein-Frage, und die Eingabezeile ohne Wagen.
    if l.modusfrage {
        if !l.modusfrage_gestellt {
            l.modusfrage_gestellt = true;
            if let Some(sch) = schirm {
                eingabezeile_ohne_wagen(&sch, bild);
            }
            let _ = write!(aus, "\r\x1b[2K  {}", crate::janein::hervor("Auf auto stellen? [j/N]", Some(bild)));
            let _ = aus.flush();
        }
        return;
    }

    // ⚑ **Die Abbruchfrage verdraengt die Statuszeile genauso**, und sie
    // steht **nach** der Vorlage: Wer im `manual mode` gerade gefragt
    // wird, soll diese Frage zuerst beantworten.
    if l.abbruchfrage && l.frage.is_none() {
        if !l.abbruch_gestellt {
            l.abbruch_gestellt = true;
            if let Some(sch) = schirm {
                eingabezeile_ohne_wagen(&sch, bild);
            }
            let _ = write!(
                aus,
                "\r\x1b[2K  {}",
                crate::janein::hervor("Wirklich beenden? [J/n]", Some(bild))
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
            if let Some(sch) = schirm {
                eingabezeile_ohne_wagen(&sch, bild);
            }
            let _ = write!(
                aus,
                "\r\x1b[2K{f}\r\n\r\x1b[2K  {}",
                crate::janein::hervor("ausfuehren? [J/n]", Some(bild))
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
            crate::ansi::SetForegroundColor(design::ladefarbe(bild, versatz))
        );
    }
    let _ = write!(
        aus,
        "{}{schweif}{}\x1b8",
        crate::ansi::SetForegroundColor(design::toene(bild).beiwerk),
        crate::ansi::ResetColor
    );
    let _ = aus.flush();
    // ⚑ **Die Zeile des Minings eine darunter**, solange der Lauf die
    //   Ladezeile braucht ([`crate::miningzeile`]).
    crate::miningzeile::zeichnen_im_lauf(&sch, bild);
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
            notausfrage: None,
            notaus_gestellt: false,
            frage: None,
            gestellt: false,
            antwort: None,
            befehle: None,
            getippt: String::new(),
            modusfrage: false,
            modusfrage_gestellt: false,
            moduswechsel: None,
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
            notausfrage: None,
            notaus_gestellt: false,
            frage: None,
            gestellt: false,
            antwort: None,
            befehle: None,
            getippt: String::new(),
            modusfrage: false,
            modusfrage_gestellt: false,
            moduswechsel: None,
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
    pub fn stueck(&self, s: myl_local_agent::textstrom::Stueck) {
        if !self.am_schirm {
            return;
        }
        let Ok(mut l) = self.lage.lock() else { return };
        match s {
            myl_local_agent::textstrom::Stueck::Denken(t) => l.strom.push((true, t)),
            myl_local_agent::textstrom::Stueck::Text(t) => l.strom.push((false, t)),
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
            quelle.contains("KeyCode::Char('x') => abbruchfrage_stellen(&lage),"),
            "Strg-X beendet noch ohne Frage"
        );
        // ⚑ Seit dem 2026-09-26 ist Strg-C der Notaus mit Frage, nicht Beenden.
        assert!(quelle.contains("KeyCode::Char('c') => notausfrage_stellen(&lage, true),"));
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

    /// ⛔️ **Esc haelt an, und zwar aus diesem Faden** (Fund 483): Er ist
    /// der einzige Leser der Tastatur. Steht eine Vorlage da, ist Esc
    /// deren Nein und haelt nicht an.
    ///
    /// Gegenprobe: mit dem Notauswaechter von v0.85.0 zurueck, dem
    /// zweiten Leser, blieb die Konsole nach einem `j` stehen.
    #[test]
    fn esc_haelt_an_im_einzigen_leser() {
        let quelle = include_str!("anzeige.rs");
        let faden = quelle.split("fn takten(").nth(1).expect("takten").split("\nfn ").next().unwrap_or("");
        let frage = faden.find("} else if offene_frage(&lage) {").expect("die Vorlage geht vor");
        let esc = faden.find("} else if k.code == KeyCode::Esc {").expect("Esc haelt nicht an");
        assert!(frage < esc, "Esc haelt an, obwohl eine Vorlage dasteht");
        assert!(faden[esc..].contains("notausfrage_stellen(&lage, false);"), "Esc fragt nicht");
        // Angehalten wird nur aus der Antwort heraus.
        let antwort = faden.find("if let Some(ja) = notausantwort(").expect("keine Antwort auf den Notaus");
        assert!(faden[antwort..esc].contains("halt();"));
        assert_eq!(faden.matches("event::read()").count(), 1);
    }

    /// ⚑ **Die Vorgaben der Notausfrage** (Festlegung des Projektinhabers,
    /// 2026-09-26): Strg-C [J/n], Esc [j/N], ein zweites Strg-C ist ja.
    #[test]
    fn die_notausfrage_hat_je_taste_ihre_vorgabe() {
        use super::notausantwort;
        use crossterm::event::KeyCode;
        assert_eq!(notausantwort(KeyCode::Enter, false, true), Some(true), "Strg-C: Eingabe ist ja");
        assert_eq!(notausantwort(KeyCode::Enter, false, false), Some(false), "Esc: Eingabe ist nein");
        assert_eq!(notausantwort(KeyCode::Char('c'), true, false), Some(true), "zweites Strg-C");
        for (t, ja) in [('j', true), ('Y', true), ('n', false), ('N', false)] {
            assert_eq!(notausantwort(KeyCode::Char(t), false, !ja), Some(ja), "{t}");
        }
        assert_eq!(notausantwort(KeyCode::Esc, false, true), Some(false));
        assert_eq!(notausantwort(KeyCode::Char('x'), false, true), None);
        assert_eq!(notausantwort(KeyCode::Char('s'), true, true), None, "Strg-S antwortet nicht");
    }

    /// 📌 **Die Eingabezeile ist genau so breit wie der Rahmen**, mit und
    /// ohne Wagen, leer und mit zu langem Text (Bild des Projektinhabers,
    /// 2026-10-12: der rechte Strich stand eine Spalte zu weit links).
    #[test]
    fn die_eingabezeile_fuellt_den_rahmen() {
        use super::eingabeinnen;
        let sichtbar = |s: &str| s.replace("\x1b[7m", "").replace("\x1b[27m", "").chars().count();
        for innen in [26usize, 40, 76] {
            for text in ["", "/stop loop", &"x".repeat(200)] {
                for wagen in [true, false] {
                    assert_eq!(sichtbar(&eingabeinnen(innen, text, wagen)), innen, "innen {innen}, Text {}, Wagen {wagen}", text.len());
                }
            }
        }
    }

    /// ⚑ **Umschalt-Tab im Lauf** (2026-10-12): nach `auto` erst die
    /// Sicherheitsmeldung und die Frage, ein Nein aendert nichts, ein Ja
    /// schaltet und zeichnet die Fusszeile; zurueck nach `manual` ohne Frage.
    #[test]
    fn umschalt_tab_im_lauf() {
        use super::{auto_an, auto_setzen, modus_umschalten, modusfrage_beantworten, offene_modusfrage, Lage, SPRUECHE};
        use std::sync::{Arc, Mutex};
        use std::time::Instant;
        let gezeichnet = Arc::new(Mutex::new(Vec::new()));
        let g = Arc::clone(&gezeichnet);
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
            notausfrage: None,
            notaus_gestellt: false,
            frage: None,
            gestellt: false,
            antwort: None,
            befehle: None,
            getippt: String::new(),
            modusfrage: false,
            modusfrage_gestellt: false,
            moduswechsel: None,
        };
        l.moduswechsel = Some((vec!["Achtung: auto mode".into()], Arc::new(move |an: bool| g.lock().unwrap().push(an))));
        let lage = Arc::new(Mutex::new(l));
        auto_setzen(false);
        modus_umschalten(&lage);
        assert!(offene_modusfrage(&lage) && !auto_an(), "vor auto kommt die Frage");
        assert!(lage.lock().unwrap().offen.iter().any(|z| z.voll.contains("Achtung: auto mode")), "die Sicherheitsmeldung fehlt");
        modusfrage_beantworten(&lage, false);
        assert!(!auto_an() && !offene_modusfrage(&lage), "ein Nein schaltet nicht");
        modus_umschalten(&lage);
        modusfrage_beantworten(&lage, true);
        assert!(auto_an(), "ein Ja schaltet");
        modus_umschalten(&lage);
        assert!(!auto_an() && !offene_modusfrage(&lage), "zurueck nach manual ohne Frage");
        assert_eq!(*gezeichnet.lock().unwrap(), vec![true, false], "die Fusszeile folgt dem Modus");
    }

    /// ⚑ **Die Eingabezeile eines Laufs** (2026-10-12): Zeichen und
    /// Ruecktaste bearbeiten, Esc leert, Eingabe schickt ab; Esc an der
    /// leeren Zeile und eine leere Eingabe gehoeren ihr nicht.
    #[test]
    fn die_eingabezeile_eines_laufs() {
        use super::{ist_stop_loop, tippen, Tipp};
        use crossterm::event::KeyCode;
        let mut p = String::new();
        assert_eq!(tippen(&mut p, KeyCode::Esc), Tipp::Nichts, "Esc an der leeren Zeile ist der Notaus");
        assert_eq!(tippen(&mut p, KeyCode::Enter), Tipp::Nichts, "leer wird nichts abgeschickt");
        for c in "/stop  loopx".chars() {
            tippen(&mut p, KeyCode::Char(c));
        }
        assert_eq!(tippen(&mut p, KeyCode::Backspace), Tipp::Geaendert("/stop  loop".into()));
        let Tipp::Abgeschickt(zeile) = tippen(&mut p, KeyCode::Enter) else { panic!("nicht abgeschickt") };
        assert!(ist_stop_loop(&zeile), "{zeile}");
        assert!(p.is_empty(), "nach der Eingabe ist die Zeile leer");
        tippen(&mut p, KeyCode::Char('a'));
        assert_eq!(tippen(&mut p, KeyCode::Esc), Tipp::Geaendert(String::new()), "Esc leert erst die Zeile");
        assert!(ist_stop_loop(" /STOP LOOP "));
        assert!(!ist_stop_loop("/stop"));
        assert!(!ist_stop_loop("/stop loops"));
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
