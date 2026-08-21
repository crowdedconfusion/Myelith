//! Begrüßungsbanner.
//!
//! Greift das Projektbanner (`README/Grafiken/myelith-banner.png`) auf:
//! ein Netz aus Knoten und dünnen Verbindungen, darin der Schriftzug.
//! Die Tagline des Projektbanners bleibt bewusst weg: im Terminal
//! steht darunter ohnehin sofort das Menü, und drei Textzeilen zwischen
//! Schriftzug und Auswahl drängen die eigentliche Bedienung nach unten.
//!
//! ## Warum das Netzmotiv so aussieht
//!
//! Die erste Fassung war ein regelmäßiger Zickzack aus gleich großen
//! Knoten: hübsch, aber es sah nach Ornament aus, nicht nach einem Netz.
//! Das Vorbild trägt drei Eigenschaften, die den Unterschied machen, und
//! alle drei sind hier nachgebildet:
//!
//! - **Knoten verschiedener Größe.** `◉ ● ○ ∘ ·` von der Nabe bis zum
//!   fernen Punkt. Gleich große Knoten lesen sich als Muster, verschieden
//!   große als Struktur.
//! - **Naben mit auffächernden Kanten.** An `◉` und `●` gehen acht Kanten
//!   ab (`│ ╱ ╲` in beide Richtungen). Das ist das Bildzeichen für einen
//!   Knoten mit vielen Verbindungen, und im Original der auffälligste Zug.
//! - **Lange Kanten, die einander kreuzen.** Sie verbinden weit entfernte
//!   Knoten und laufen quer durchs Feld, statt nur Nachbarn zu paaren.
//!
//! ## Warum das Motiv zur Laufzeit entsteht
//!
//! Bis v0.6.0 stand es als fester Text im Quelltext, mit der Begründung,
//! ein Generator sei Aufwand für ein Bild, das sich nie ändert. Diese
//! Begründung ist entfallen: Das Bild soll die Fensterbreite füllen, also
//! ändert es sich bei jedem anderen Terminal. [`fuer_fenster`] baut es auf
//! einem Zeichenraster (Knoten setzen, Kanten ziehen, rastern).
//!
//! Der Schriftzug bleibt dabei **56 Zeichen breit und wird zentriert**,
//! nicht gestreckt: Er ist ein Bild, kein Text, und in die Breite gezogen
//! unleserlich. Gefüllt wird die Fläche vom Netz um ihn herum, genau wie
//! im Vorbild.
//!
//! **Auch die Höhe zählt.** Passt das Motiv nicht mitsamt Menü ins
//! Fenster, scrollt der Schriftzug nach oben weg. Gekürzt wird deshalb von
//! unten nach oben: erst der untere Netzblock, dann der obere. Der feste
//! Text in [`BANNER`] bleibt als Rückfall für zu schmale Fenster.
//!
//! **Nicht immer anzeigen:** Bei `--quiet` und wenn die Ausgabe in eine
//! Datei oder Pipe geht, bleibt das Banner weg. Ein Protokoll, das mit
//! ASCII-Kunst beginnt, ist schlechter zu diffen.

/// Der Schriftzug mit Netzmotiv, wie im Projektbanner.
pub const BANNER: &str = r#"
              ·     ╱ │ ╲   ∘   ○       ∘       · ╱ │ ╲       ∘
             ╱       ╱│╲         ╲   ╱│╲       ╱   ╱│╲       ╱
      ●─────╱─────────◉───────────╲───●───────╱─────◉───────╱───────────◉
           ╱         ╲│╱           ╲ ╲│╱     ╱     ╲│╱     ╱
          ∘         ╲ │ ╱           ○       ∘     ╲ │ ╱   ·         ·

  ███╗   ███╗██╗   ██╗███████╗██╗     ██╗████████╗██╗  ██╗
  ████╗ ████║╚██╗ ██╔╝██╔════╝██║     ██║╚══██╔══╝██║  ██║
  ██╔████╔██║ ╚████╔╝ █████╗  ██║     ██║   ██║   ███████║
  ██║╚██╔╝██║  ╚██╔╝  ██╔══╝  ██║     ██║   ██║   ██╔══██║
  ██║ ╚═╝ ██║   ██║   ███████╗███████╗██║   ██║   ██║  ██║
  ╚═╝     ╚═╝   ╚═╝   ╚══════╝╚══════╝╚═╝   ╚═╝   ╚═╝  ╚═╝

      ╲         ·            ╱  ╱│╲     ·             ·        ╱│╲
     ∘─╲────○───────────────╱────◉─────────────●───────╲────────●────────◉
        ╲                  ╱    ╲│╱                     ╲      ╲│╱
         ·            ·   ∘                         ∘    ∘            ○
"#;

/// Untertitel des Testclients: direkt unter dem Banner.
pub const SUBTITLE: &str = "        Testclient · Hardware · Determinismus · Shards";

/// Gibt das Banner aus, wenn es sinnvoll ist.
///
/// `show` kommt vom Aufrufer (üblicherweise `!quiet`). Zusätzlich wird
/// die Umgebungsvariable `NO_COLOR`/`MYL_NO_BANNER` respektiert: wer
/// den Client in einem Skript aufruft, will kein Bild.
pub fn print_if(show: bool) {
    if !show || std::env::var("MYL_NO_BANNER").is_ok() {
        return;
    }
    println!("{}", BANNER);
    println!("{}\n", SUBTITLE);
}

/// Leert den Bildschirm und setzt das Banner an den Anfang.
///
/// **Warum überhaupt geleert wird.** Das Menü ist die Seite, an der ein
/// Teilnehmer entscheidet, was als Nächstes geschieht. Steht darüber noch
/// die Ausgabe des vorigen Laufs, muss er erst zurückscrollen, um zu
/// sehen, wo er ist, und bei einem Testlauf über sechs Prompts sind das
/// einige Bildschirmhöhen. Nach jeder Aktion also: aufräumen, Logo, und
/// darunter genau die Auswahl, die ansteht.
///
/// **Was dabei verlorengeht, ist bedacht.** Die Ausgabe eines Laufs
/// verschwindet mit dem nächsten Aufräumen. Sie ist nicht weg: Sie steht
/// vollständig im Protokoll, und vor dem Aufräumen wartet der Client auf
/// einen Tastendruck (siehe `menu::weiter`), damit sie gelesen werden
/// kann, solange sie gebraucht wird.
///
/// **Nur auf einem Terminal.** Geht die Ausgabe in eine Datei oder Pipe,
/// wird nichts geleert und nichts positioniert. Steuersequenzen in einem
/// mitgeschnittenen Lauf wären Müll.
pub fn bildschirm() {
    bildschirm_mit(crate::farben::logo());
}

/// Wie [`bildschirm`], aber mit vorgegebener Farbe für den Schriftzug.
///
/// Die Farbe ist seit v0.6.0 eine Eigenschaft der **Sitzung** und für
/// alle Bildschirme dieselbe ([`crate::farben::logo`]); dieser Weg bleibt
/// für Aufrufer, die sie ohnehin schon in der Hand haben.
pub fn bildschirm_mit(farbe: crossterm::style::Color) {
    use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};
    use std::io::IsTerminal;

    if std::env::var("MYL_NO_BANNER").is_ok() {
        return;
    }
    // Ohne Terminal keine Steuerzeichen: Ein mitgeschnittener Lauf soll
    // lesbar bleiben, und Farbcodes in einer Datei sind es nicht.
    if !std::io::stdout().is_terminal() {
        println!("{}", BANNER);
        println!("{}\n", SUBTITLE);
        return;
    }

    let (breite, hoehe) = fenstermasse();
    let text = fuer_fenster(breite, hoehe);

    let mut aus = std::io::stdout();
    // **Erst All, dann Purge, und diese Reihenfolge ist der Punkt.**
    //
    // `Clear(All)` (`ESC[2J`) räumt das sichtbare Bild. Mehrere Terminals,
    // darunter Terminal.app und iTerm2, schieben den bisherigen Inhalt
    // dabei in den Rückblätterspeicher, statt ihn zu verwerfen.
    // `Clear(Purge)` (`ESC[3J`) leert genau diesen Speicher.
    //
    // Die erste Fassung sendete Purge **zuerst**: Der Speicher war danach
    // leer, und der unmittelbar folgende All-Befehl legte den alten
    // Bildschirm gleich wieder hinein. Wer nach oben scrollte, fand dort
    // die Ausgabe des vorigen Laufs, obwohl zweimal gelöscht worden war.
    // Umgekehrt herum bleibt nichts übrig.
    let _ = crossterm::execute!(
        aus,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::Purge),
        crossterm::cursor::MoveTo(0, 0)
    );
    for zeile in text.lines() {
        let im_schriftzug = ist_schriftzug(zeile);
        for c in zeile.chars() {
            let (ton, stark) = zeichenstil(c, im_schriftzug, farbe);
            let _ = crossterm::queue!(
                aus,
                SetForegroundColor(ton),
                SetAttribute(stark),
                Print(c)
            );
        }
        let _ = crossterm::queue!(
            aus,
            ResetColor,
            SetAttribute(Attribute::Reset),
            Print("\n")
        );
    }
    let _ = crossterm::queue!(
        aus,
        SetForegroundColor(crate::farben::BEIWERK),
        Print(untertitel(breite)),
        ResetColor,
        Print("\n\n")
    );
    let _ = std::io::Write::flush(&mut aus);
}

/// Maße des Terminalfensters, mit belastbaren Rückfallwerten.
///
/// 80 x 24, wenn sich nichts ermitteln lässt: die untere Grenze, mit der
/// zu rechnen ist, und die Maße, für die der feste Text gebaut wurde.
pub fn fenstermasse() -> (u16, u16) {
    crossterm::terminal::size().unwrap_or((80, 24))
}

/// Nur die Breite. Getrennt, weil das Netzmotiv sie allein braucht.
pub fn fensterbreite() -> u16 {
    fenstermasse().0
}

/// Rückt einen mehrzeiligen Textblock mittig unter den Schriftzug.
///
/// **Der Block wird zentriert, nicht jede Zeile für sich.** Alle Zeilen
/// bekommen denselben Einzug, ihre Ausrichtung untereinander bleibt also
/// erhalten. Zeilenweise zentriert wäre eine Aufzählung ein Flattersatz,
/// und ein Menü, dessen Punkte gegeneinander verrutschen, ist nicht mehr
/// als Liste lesbar.
///
/// Maßgeblich ist die **breiteste** Zeile: Sie bestimmt, wie breit der
/// Block ist, und um sie herum wird ausgerichtet.
///
/// Ohne Terminal bleibt der Text unverändert. Ein mitgeschnittener Lauf
/// soll diffbar bleiben, und führende Leerzeichen sind dort Ballast.
pub fn zentriert(text: &str) -> String {
    if !std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        return text.to_string();
    }
    let einzug = blockeinzug(text.lines().map(|z| z.chars().count()).max().unwrap_or(0));
    text.lines()
        .map(|z| {
            if z.trim().is_empty() {
                String::new()
            } else {
                format!("{}{}", einzug, z)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Der Einzug, der einen Block dieser Breite mittig im Fenster ausrichtet.
///
/// Leer, wenn der Block ohnehin breiter ist als das Fenster: Ein negativer
/// Einzug ginge nicht, und ein Block am linken Rand ist immer noch besser
/// als einer, der rechts hinausläuft.
pub fn blockeinzug(blockbreite: usize) -> String {
    " ".repeat((fensterbreite() as usize).saturating_sub(blockbreite) / 2)
}

/// Der Untertitel, mittig zum Schriftzug.
pub fn untertitel(breite: u16) -> String {
    let b = breite as usize;
    if b < MINDESTBREITE {
        return SUBTITLE.to_string();
    }
    let text = SUBTITLE.trim();
    let einzug = b.saturating_sub(text.chars().count()) / 2;
    format!("{}{}", " ".repeat(einzug), text)
}


// ---------------------------------------------------------------------------
// Netzmotiv für eine gegebene Breite
// ---------------------------------------------------------------------------

/// Breite des Blockschriftzugs in Zeichen.
///
/// Der feste Text in [`BANNER`] ist 58 Zeichen breit, weil er zwei
/// Leerzeichen Einzug mitführt; der Schriftzug selbst misst 56. Ein Test
/// hält beides gegeneinander, damit die Zahl nicht auseinanderläuft.
pub const SCHRIFTBREITE: usize = 56;

/// Schmalste Breite, für die überhaupt gezeichnet wird.
///
/// Unterhalb passt der Schriftzug nicht, und ein umbrechendes Banner sieht
/// schlimmer aus als keines.
pub const MINDESTBREITE: usize = SCHRIFTBREITE + 4;

/// Ein Zeichenraster, auf dem das Netzmotiv entsteht.
///
/// **Warum überhaupt zur Laufzeit gezeichnet wird.** Bis v0.6.0 stand das
/// Motiv als fester Text im Quelltext, mit dem Argument, ein Generator sei
/// Aufwand für ein Bild, das sich nie ändert. Das Argument fiel, als das
/// Banner die Breite des Fensters füllen sollte: Jetzt ändert es sich:
/// bei jedem anderen Terminal.
struct Raster {
    breite: usize,
    zeilen: Vec<Vec<char>>,
}

impl Raster {
    fn neu(breite: usize, hoehe: usize) -> Self {
        Self {
            breite,
            zeilen: vec![vec![' '; breite]; hoehe],
        }
    }

    fn setz(&mut self, x: usize, y: usize, c: char) {
        if y < self.zeilen.len() && x < self.breite {
            self.zeilen[y][x] = c;
        }
    }

    /// Ist die Stelle noch leer?
    ///
    /// Alles Zeichnen prüft das zuerst. Damit gewinnt, was zuerst gesetzt
    /// wurde, und weil Knoten zuletzt kommen, überschreibt keine Kante
    /// einen Knoten, wohl aber umgekehrt.
    fn frei(&self, x: usize, y: usize) -> bool {
        y < self.zeilen.len() && x < self.breite && self.zeilen[y][x] == ' '
    }

    fn hlinie(&mut self, y: usize, x0: usize, x1: usize) {
        for x in x0..=x1.min(self.breite.saturating_sub(1)) {
            if self.frei(x, y) {
                self.setz(x, y, '─');
            }
        }
    }

    /// Eine Diagonale über `n` Schritte in Richtung `(dx, dy)`.
    ///
    /// Richtung und Länge statt Start und Ziel: Bei zwei Punkten, deren
    /// Abstände in x und y nicht übereinstimmen, gibt es keine Diagonale in
    /// 45 Grad, die beide trifft: eine Schleife, die auf das Ziel wartet,
    /// läuft dann endlos. Genau das ist beim Entwurf passiert.
    fn diag(&mut self, x0: usize, y0: usize, dx: isize, dy: isize, n: usize) {
        let z = if dx * dy > 0 { '╲' } else { '╱' };
        for i in 0..n {
            let x = x0 as isize + dx * i as isize;
            let y = y0 as isize + dy * i as isize;
            if x < 0 || y < 0 {
                continue;
            }
            if self.frei(x as usize, y as usize) {
                self.setz(x as usize, y as usize, z);
            }
        }
    }

    /// Ein Knotenpunkt mit abgehenden Kanten, das Bildzeichen des Netzes.
    fn nabe(&mut self, x: usize, y: usize, zeichen: char, spanne: usize) {
        for d in 1..=spanne {
            for (ax, ay, c) in [
                (0isize, -(d as isize), '│'),
                (0, d as isize, '│'),
                (-(d as isize), -(d as isize), '╱'),
                (d as isize, d as isize, '╱'),
                (-(d as isize), d as isize, '╲'),
                (d as isize, -(d as isize), '╲'),
            ] {
                let (nx, ny) = (x as isize + ax, y as isize + ay);
                if nx >= 0 && ny >= 0 && self.frei(nx as usize, ny as usize) {
                    self.setz(nx as usize, ny as usize, c);
                }
            }
        }
        self.setz(x, y, zeichen);
    }

    fn ausgeben(&self) -> Vec<String> {
        self.zeilen
            .iter()
            .map(|z| z.iter().collect::<String>().trim_end().to_string())
            .collect()
    }
}

/// Stelle bei `zaehler/nenner` der Breite.
fn anteil(breite: usize, zaehler: usize, nenner: usize) -> usize {
    (breite * zaehler / nenner).min(breite.saturating_sub(1))
}

/// Das Banner in der gewünschten Breite.
///
/// Der Schriftzug bleibt 58 Zeichen breit: er ist ein Bild, kein Text, und
/// gestreckt wäre er unleserlich. Er wird deshalb **zentriert**, und das
/// Netz füllt die Breite um ihn herum. Genau so ist das Vorbild aufgebaut:
/// Wortmarke in der Mitte, Netz über die ganze Fläche.
///
/// Unterhalb von [`MINDESTBREITE`] kommt der feste Text zurück; dort ist
/// für ein Netz ohnehin kein Platz.
pub fn fuer_breite(breite: u16) -> String {
    fuer_fenster(breite, u16::MAX)
}

/// Höhe, ab der das Banner mit beiden Netzblöcken gezeigt wird.
///
/// Gerechnet, nicht geschätzt: 19 Zeilen Banner, 2 für den Untertitel,
/// rund 8 für die Einstellungen und 14 für ein Menü mit sechs Punkten und
/// Hinweiszeilen ergeben 43. Darunter scrollte der Schriftzug nach oben
/// weg, und übrig bliebe genau das Bild, das der aufgeräumte Bildschirm
/// vermeiden soll.
pub const VOLLE_HOEHE: u16 = 44;
/// Höhe, ab der wenigstens der obere Netzblock bleibt.
pub const HALBE_HOEHE: u16 = 34;

/// Das Banner für ein Fenster gegebener Breite **und Höhe**.
///
/// **Warum die Höhe mitzählt.** Das Motiv ist 19 Zeilen hoch. In einem
/// Fenster mit 40 Zeilen bleiben darunter zu wenige für Einstellungen und
/// Menü, der Bildschirm scrollt, und der Schriftzug verschwindet nach
/// oben. Ein Logo, das man wegscrollen muss, um das Menü zu sehen, ist
/// schlechter als ein kleineres Logo.
///
/// Gekürzt wird von unten nach oben: erst der untere Netzblock, dann der
/// obere. Der Schriftzug bleibt am längsten, denn er ist das
/// Wiedererkennungszeichen; das Netz ist seine Umgebung.
pub fn fuer_fenster(breite: u16, hoehe: u16) -> String {
    let b = breite as usize;
    if b < MINDESTBREITE {
        return BANNER.to_string();
    }

    let mut zeilen: Vec<String> = Vec::new();
    zeilen.push(String::new());
    if hoehe >= HALBE_HOEHE {
        zeilen.extend(netz_oben(b));
        zeilen.push(String::new());
    }

    let einzug = " ".repeat((b - SCHRIFTBREITE) / 2);
    for z in SCHRIFTZUG {
        zeilen.push(format!("{}{}", einzug, z));
    }

    zeilen.push(String::new());
    if hoehe >= VOLLE_HOEHE {
        zeilen.extend(netz_unten(b));
        zeilen.push(String::new());
    }
    zeilen.join("\n")
}

/// Fünf Zeilen Netz über dem Schriftzug.
///
/// Die Stellen sind Anteile der Breite, keine festen Spalten: Damit sitzt
/// dasselbe Motiv in einem 80- wie in einem 200-Zeichen-Fenster, statt
/// links zusammenzurücken und rechts eine leere Fläche zu lassen.
///
/// **Jede Diagonale endet an einem Knoten.** Die erste Fassung setzte
/// Kanten und Knoten unabhängig voneinander; Diagonalen liefen dann an
/// Knoten vorbei und hörten eine Spalte weiter im Nichts auf. Eine Kante,
/// die nichts verbindet, ist im Netzbild ein Fehler. Die Endpunkte werden
/// deshalb aus den Kanten berechnet, und dort steht ein Knoten.
fn netz_oben(b: usize) -> Vec<String> {
    let hoehe = 5;
    let mut r = Raster::neu(b, hoehe);

    // Die lange Kante quer durchs Feld, zuerst: sie ist der Untergrund.
    r.hlinie(2, anteil(b, 1, 20), anteil(b, 19, 20));

    // Schräg verlaufende Kanten, jede mit einem Knoten an beiden Enden.
    let schraeg = [
        (anteil(b, 1, 10), hoehe - 1, 1isize, -1isize, '∘', '·'),
        (anteil(b, 2, 5), 0, 1, 1, '○', '∘'),
        (anteil(b, 29, 50), hoehe - 1, 1, -1, '·', '○'),
        (anteil(b, 4, 5), hoehe - 1, 1, -1, '∘', '·'),
    ];
    let mut enden: Vec<(usize, usize, char)> = Vec::new();
    for (x, y, dx, dy, von, bis) in schraeg {
        r.diag(x, y, dx, dy, hoehe);
        let ex = (x as isize + dx * (hoehe as isize - 1)).max(0) as usize;
        let ey = (y as isize + dy * (hoehe as isize - 1)).max(0) as usize;
        enden.push((x, y, von));
        enden.push((ex, ey, bis));
    }

    // Naben zuletzt: Sie dürfen Kanten überschreiben, nicht umgekehrt.
    r.nabe(anteil(b, 27, 100), 2, '◉', 2);
    r.nabe(anteil(b, 17, 25), 2, '◉', 2);
    r.nabe(anteil(b, 12, 25), 2, '●', 1);
    for (x, y, c) in enden {
        r.setz(x, y, c);
    }
    r.setz(anteil(b, 1, 20), 2, '●');
    r.setz(anteil(b, 19, 20), 2, '◉');
    r.ausgeben()
}

/// Vier Zeilen Netz unter dem Schriftzug.
///
/// Bewusst anders gewichtet als oben: Ein gespiegeltes Motiv sähe nach
/// Ornament aus, und genau davon soll es sich unterscheiden.
fn netz_unten(b: usize) -> Vec<String> {
    let hoehe = 4;
    let mut r = Raster::neu(b, hoehe);

    r.hlinie(1, anteil(b, 1, 25), anteil(b, 24, 25));

    let schraeg = [
        (anteil(b, 1, 20), 0usize, 1isize, 1isize, '·', '∘'),
        (anteil(b, 3, 10), hoehe - 1, 1, -1, '∘', '·'),
        (anteil(b, 7, 10), 0, 1, 1, '·', '○'),
    ];
    let mut enden: Vec<(usize, usize, char)> = Vec::new();
    for (x, y, dx, dy, von, bis) in schraeg {
        r.diag(x, y, dx, dy, hoehe);
        let ex = (x as isize + dx * (hoehe as isize - 1)).max(0) as usize;
        let ey = (y as isize + dy * (hoehe as isize - 1)).max(0) as usize;
        enden.push((x, y, von));
        enden.push((ex, ey, bis));
    }

    r.nabe(anteil(b, 21, 50), 1, '◉', 1);
    r.nabe(anteil(b, 21, 25), 1, '●', 1);
    for (x, y, c) in enden {
        r.setz(x, y, c);
    }
    r.setz(anteil(b, 1, 25), 1, '∘');
    r.setz(anteil(b, 24, 25), 1, '◉');
    r.setz(anteil(b, 61, 100), 1, '●');
    r.setz(anteil(b, 7, 50), 1, '○');
    r.ausgeben()
}

/// Der Blockschriftzug allein, ohne Netz und ohne Einzug.
const SCHRIFTZUG: [&str; 6] = [
    "███╗   ███╗██╗   ██╗███████╗██╗     ██╗████████╗██╗  ██╗",
    "████╗ ████║╚██╗ ██╔╝██╔════╝██║     ██║╚══██╔══╝██║  ██║",
    "██╔████╔██║ ╚████╔╝ █████╗  ██║     ██║   ██║   ███████║",
    "██║╚██╔╝██║  ╚██╔╝  ██╔══╝  ██║     ██║   ██║   ██╔══██║",
    "██║ ╚═╝ ██║   ██║   ███████╗███████╗██║   ██║   ██║  ██║",
    "╚═╝     ╚═╝   ╚═╝   ╚══════╝╚══════╝╚═╝   ╚═╝   ╚═╝  ╚═╝",
];

/// Gehört diese Zeile zum Blockschriftzug?
pub(crate) fn ist_schriftzug(zeile: &str) -> bool {
    zeile.contains('█') || zeile.contains('╚')
}

/// Die Knoten des Netzmotivs, von der Nabe bis zum fernen Punkt.
const KNOTEN: [char; 5] = ['◉', '●', '○', '∘', '·'];

/// Farbe und Stärke eines einzelnen Bannerzeichens.
///
/// **Warum je Zeichen und nicht je Zeile.** Die erste Fassung färbte
/// zeilenweise: Schriftzug hell, alles andere in einem Grauton. Damit
/// verschwand das Netz, und mit ihm der Teil des Bildes, der das Projekt
/// überhaupt darstellt. Das Vorbild macht es anders und genauer: helle
/// Knotenpunkte, dünne graue Linien dazwischen. Drei Stufen statt zwei:
///
/// | Zeichen | Darstellung |
/// |---|---|
/// | Schriftzug (`█ ═ ║ ╔ …`) | Neonfarbe, fett |
/// | Knoten (`◉ ● ○ ∘ ·`) | Neonfarbe, fett: sie tragen das Motiv |
/// | Kanten (`─ │ ╱ ╲`) | Grau, normal. Verbindung, nicht Blickfang |
pub(crate) fn zeichenstil(
    c: char,
    im_schriftzug: bool,
    farbe: crossterm::style::Color,
) -> (crossterm::style::Color, crossterm::style::Attribute) {
    use crossterm::style::Attribute;
    if im_schriftzug || KNOTEN.contains(&c) {
        (farbe, Attribute::Bold)
    } else {
        (crate::farben::KANTE, Attribute::NormalIntensity)
    }
}

/// Startbild mit Animation, sonst wie [`print_if`].
///
/// Getrennt von `print_if`, weil nicht jeder Bannerdruck animiert gehört:
/// Die Animation läuft **einmal** beim Start des interaktiven Menüs. Wer
/// einen Unterbefehl aufruft, will messen und nicht zusehen.
pub fn start_if(show: bool) {
    start_if_mit(show, crate::farben::logo());
}

/// Wie [`start_if`], aber mit vorgegebener Farbe.
///
/// Gebraucht, wenn die Farbe über den Start hinaus gilt: Die Begrüßung
/// nach der Namenseingabe hebt den Namen in derselben Farbe hervor, in
/// der eben der Schriftzug entstanden ist. Zwei Farben hintereinander
/// sähen aus, als hätte der Client das Thema gewechselt.
pub fn start_if_mit(show: bool, farbe: crossterm::style::Color) {
    if !show || std::env::var("MYL_NO_BANNER").is_ok() {
        return;
    }
    // Nach dem Sturm steht der Schriftzug bereits, aber an der Stelle, an
    // der ihn die Animation gezeichnet hat, und mit ihren Farben. Ein
    // Aufräumen und ein sauberer Neudruck bringen ihn in denselben
    // Zustand wie bei jedem späteren Aufräumen: sichtbar ist der
    // Übergang nicht, weil an derselben Stelle dasselbe Bild entsteht.
    // Ohne diesen Schritt sähe das Startbild anders aus als jedes
    // folgende, und die Namenseingabe stünde unter einem Sonderfall.
    crate::animation::abspielen(farbe);
    bildschirm_mit(farbe);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Terminals mit 80 Spalten sind die untere Grenze, mit der zu
    /// rechnen ist. Ein umbrechendes Banner zerstört das Netzmotiv.
    #[test]
    fn banner_passt_in_achtzig_spalten() {
        for (i, zeile) in BANNER.lines().enumerate() {
            let breite = zeile.chars().count();
            assert!(
                breite <= 78,
                "Bannerzeile {} ist {} Zeichen breit",
                i + 1,
                breite
            );
        }
        assert!(SUBTITLE.chars().count() <= 78);
    }

    /// Der Schriftzug und das Netzmotiv sind der Wiedererkennungswert:
    /// sie müssen bleiben. Die Tagline steht bewusst nicht im Banner
    /// (siehe Modul-Doku), sondern gekürzt im Untertitel.
    #[test]
    fn banner_traegt_schriftzug_und_netzmotiv() {
        assert!(BANNER.contains('█'), "Blockschriftzug fehlt");
        assert!(BANNER.contains('∘'), "Netzknoten fehlen");
        assert!(BANNER.contains('─'), "Netzverbindungen fehlen");
        assert!(!SUBTITLE.trim().is_empty(), "Untertitel fehlt");
    }

    /// Der Schriftzug muss über alle sechs Zeilen gleich breit sein:
    /// sonst steht er schief.
    #[test]
    fn schriftzug_ist_rechteckig() {
        let zeilen: Vec<&str> = BANNER
            .lines()
            .filter(|l| l.contains('█') || l.contains('╚'))
            .collect();
        assert_eq!(zeilen.len(), 6, "Schriftzug hat {} Zeilen", zeilen.len());
        let breiten: Vec<usize> = zeilen.iter().map(|l| l.chars().count()).collect();
        assert!(
            breiten.windows(2).all(|w| w[0] == w[1]),
            "ungleiche Zeilenbreiten: {:?}",
            breiten
        );
    }

    /// Das Netzmotiv lebt von drei Eigenschaften (siehe Modul-Doku).
    /// Ohne sie fiele es auf das Ornament zurück, das es einmal war.
    #[test]
    fn netzmotiv_traegt_naben_und_verschiedene_knoten() {
        for knoten in ['◉', '●', '○', '∘', '·'] {
            assert!(
                BANNER.contains(knoten),
                "Knotengröße {:?} fehlt: gleich große Knoten lesen sich als Muster",
                knoten
            );
        }
        // Eine Nabe zeigt sich an den senkrechten und schrägen Kanten,
        // die von ihr abgehen.
        assert!(BANNER.contains("╱│╲"), "Fächer nach oben fehlt");
        assert!(BANNER.contains("╲│╱"), "Fächer nach unten fehlt");
        // Lange Kanten quer durchs Feld statt Nachbarpaare.
        assert!(
            BANNER.lines().any(|z| z.contains("──────────")),
            "keine lange Kante"
        );
    }

    /// Drei Stufen, nicht zwei: Schriftzug und Knoten leuchten, die
    /// Kanten treten zurück. Die erste Fassung färbte zeilenweise, und das
    /// Netz verschwand, mit ihm der Teil des Bildes, der das Projekt
    /// überhaupt darstellt.
    #[test]
    fn knoten_leuchten_kanten_treten_zurueck() {
        use crossterm::style::{Attribute, Color};
        let neon = Color::AnsiValue(51);

        for knoten in KNOTEN {
            let (ton, stark) = zeichenstil(knoten, false, neon);
            assert_eq!(ton, neon, "Knoten {knoten:?} trägt nicht die Leuchtfarbe");
            assert_eq!(stark, Attribute::Bold, "Knoten {knoten:?} nicht hervorgehoben");
        }

        for kante in ['─', '│', '╱', '╲'] {
            let (ton, stark) = zeichenstil(kante, false, neon);
            assert_eq!(ton, crate::farben::KANTE, "Kante {kante:?} falsch eingefärbt");
            assert_eq!(stark, Attribute::NormalIntensity);
        }

        // Im Schriftzug zählt die Zeile, nicht das einzelne Zeichen: Auch
        // `═` und `║` gehören dort zum Buchstabenbild.
        for c in ['█', '═', '║'] {
            assert_eq!(zeichenstil(c, true, neon), (neon, Attribute::Bold));
        }
    }

    /// Die Kanten müssen sich vom Hintergrund abheben: sichtbar heller
    /// als der Ton, mit dem Hinweiszeilen zurückgenommen werden.
    #[test]
    fn kanten_sind_heller_als_beiwerk() {
        use crossterm::style::Color;
        let (Color::AnsiValue(kante), Color::AnsiValue(beiwerk)) =
            (crate::farben::KANTE, crate::farben::BEIWERK)
        else {
            panic!("Grautöne müssen Palettenindizes sein");
        };
        assert!(
            kante > beiwerk,
            "Netzkanten ({kante}) sind nicht heller als Hinweistext ({beiwerk})"
        );
    }

    /// Die Zeilenerkennung muss Schriftzug und Netz sauber trennen:
    /// sonst bekäme das halbe Bild die falsche Stufe.
    #[test]
    fn schriftzugzeilen_werden_erkannt() {
        let (schrift, netz): (Vec<&str>, Vec<&str>) = BANNER
            .lines()
            .filter(|z| !z.trim().is_empty())
            .partition(|z| ist_schriftzug(z));
        assert_eq!(schrift.len(), 6, "Schriftzug hat {} Zeilen", schrift.len());
        assert_eq!(netz.len(), 9, "Netzmotiv hat {} Zeilen", netz.len());
        assert!(netz.iter().all(|z| !z.contains('█')));
    }

    /// Das Motiv muss die Breite ausfüllen, ohne sie zu überschreiten.
    /// Eine Zeile zu breit bricht um und zerreißt das Bild; eine deutlich
    /// zu schmale ließe rechts eine leere Fläche.
    #[test]
    fn motiv_fuellt_jede_breite_ohne_umbruch() {
        for b in [62u16, 80, 100, 120, 160, 200] {
            let text = fuer_breite(b);
            let breiteste = text.lines().map(|z| z.chars().count()).max().unwrap_or(0);
            assert!(
                breiteste <= b as usize,
                "Breite {b}: Zeile mit {breiteste} Zeichen bricht um"
            );
            assert!(
                breiteste + 8 >= b as usize,
                "Breite {b}: breiteste Zeile nur {breiteste} Zeichen, Fläche bleibt leer"
            );
        }
    }

    /// Die Breitenangabe muss zum Schriftzug passen: sonst sitzt er
    /// überall um denselben Betrag daneben.
    #[test]
    fn schriftbreite_stimmt_mit_dem_schriftzug_ueberein() {
        for z in SCHRIFTZUG {
            assert_eq!(z.chars().count(), SCHRIFTBREITE, "Zeile {z:?}");
        }
    }

    /// Der Schriftzug wird zentriert, nicht gestreckt: Er ist ein Bild,
    /// kein Text, und in die Breite gezogen unleserlich.
    #[test]
    fn schriftzug_bleibt_zentriert_und_unverzerrt() {
        for b in [80u16, 140, 200] {
            let text = fuer_breite(b);
            let zeilen: Vec<&str> = text.lines().filter(|z| ist_schriftzug(z)).collect();
            assert_eq!(zeilen.len(), 6, "Breite {b}");
            for z in &zeilen {
                assert_eq!(
                    z.trim().chars().count(),
                    SCHRIFTBREITE,
                    "Breite {b}: Schriftzug verzerrt"
                );
            }
            let links = zeilen[0].len() - zeilen[0].trim_start().len();
            let rechts = b as usize - links - SCHRIFTBREITE;
            assert!(
                links.abs_diff(rechts) <= 1,
                "Breite {b}: Schriftzug steht nicht mittig ({links} links, {rechts} rechts)"
            );
        }
    }

    /// Passt das Motiv nicht mitsamt Menü ins Fenster, muss es kürzer
    /// werden. Ein Logo, das man wegscrollen muss, um das Menü zu sehen,
    /// ist schlechter als ein kleineres Logo.
    #[test]
    fn niedriges_fenster_kuerzt_das_motiv() {
        let voll = fuer_fenster(120, VOLLE_HOEHE).lines().count();
        let halb = fuer_fenster(120, HALBE_HOEHE).lines().count();
        let knapp = fuer_fenster(120, HALBE_HOEHE - 1).lines().count();

        assert!(voll > halb, "unterer Netzblock wird nicht gekürzt");
        assert!(halb > knapp, "oberer Netzblock wird nicht gekürzt");

        // Der Schriftzug überlebt jede Stufe: er ist das Wiedererkennungs-
        // zeichen, das Netz ist seine Umgebung.
        for h in [VOLLE_HOEHE, HALBE_HOEHE, HALBE_HOEHE - 1, 10, 1] {
            let text = fuer_fenster(120, h);
            assert_eq!(
                text.lines().filter(|z| ist_schriftzug(z)).count(),
                6,
                "Höhe {h}: Schriftzug unvollständig"
            );
        }
    }

    /// Logo, Untertitel und ein Menü müssen zusammen ins Fenster passen.
    /// Sonst scrollt genau das weg, was der aufgeräumte Bildschirm zeigen
    /// soll.
    #[test]
    fn motiv_laesst_platz_fuer_das_menue() {
        // Gemessen am größten Menü des Clients: Entwickler, zehn Punkte.
        let menue_zeilen = 2 + 10 * 2 + 2;
        let einstellungen_zeilen = 10;
        for h in [24u16, 30, 34, 40, 44, 60] {
            let banner = fuer_fenster(120, h).lines().count() + 2;
            let gesamt = banner + einstellungen_zeilen + menue_zeilen;
            if h >= VOLLE_HOEHE {
                assert!(gesamt <= 60, "Höhe {h}: {gesamt} Zeilen");
            } else {
                assert!(
                    banner <= h as usize / 2 + 4,
                    "Höhe {h}: Banner nimmt {banner} Zeilen und lässt zu wenig übrig"
                );
            }
        }
    }

    /// Unterhalb der Mindestbreite passt kein Netz und kein Schriftzug:
    /// dort kommt der feste Text zurück, statt etwas Zerbrochenes.
    #[test]
    fn schmales_fenster_faellt_auf_den_festen_text_zurueck() {
        for b in [0u16, 20, 40, (MINDESTBREITE - 1) as u16] {
            assert_eq!(fuer_breite(b), BANNER, "Breite {b}");
        }
        assert_ne!(fuer_breite(MINDESTBREITE as u16), BANNER);
    }

    /// Auch in der Breite bleiben die drei Bausteine des Motivs erhalten.
    #[test]
    fn erzeugtes_motiv_traegt_naben_und_knoten() {
        for b in [80u16, 160] {
            let text = fuer_breite(b);
            assert!(text.contains("╱│╲"), "Breite {b}: Fächer nach oben fehlt");
            assert!(text.contains("╲│╱"), "Breite {b}: Fächer nach unten fehlt");
            for knoten in ['◉', '●', '○', '∘', '·'] {
                assert!(text.contains(knoten), "Breite {b}: Knoten {knoten:?} fehlt");
            }
        }
    }

    /// Der Untertitel steht mittig unter dem Schriftzug, nicht links.
    #[test]
    fn untertitel_steht_mittig() {
        for b in [80u16, 140] {
            let z = untertitel(b);
            let links = z.len() - z.trim_start().len();
            let rechts = b as usize - z.chars().count();
            assert!(
                links.abs_diff(rechts) <= 1,
                "Breite {b}: Untertitel nicht mittig ({links} links, {rechts} rechts)"
            );
        }
    }

    /// Ein zentrierter Block behält seine innere Ausrichtung: Der Abstand
    /// zwischen zwei Zeilen bleibt derselbe, alle wandern gemeinsam.
    #[test]
    fn zentrierter_block_behaelt_seine_ausrichtung() {
        let block = "  Kopf\n    eingerückt\n\n  Fuß";
        let gerueckt = zentriert(block);

        let vorher: Vec<usize> = block
            .lines()
            .filter(|z| !z.trim().is_empty())
            .map(|z| z.len() - z.trim_start().len())
            .collect();
        let nachher: Vec<usize> = gerueckt
            .lines()
            .filter(|z| !z.trim().is_empty())
            .map(|z| z.len() - z.trim_start().len())
            .collect();

        assert_eq!(vorher.len(), nachher.len(), "Zeilen gingen verloren");
        let versatz = nachher[0] - vorher[0];
        for (v, n) in vorher.iter().zip(nachher.iter()) {
            assert_eq!(n - v, versatz, "Zeilen sind gegeneinander verrutscht");
        }
        assert_eq!(
            gerueckt.lines().count(),
            block.lines().count(),
            "Leerzeilen gingen verloren"
        );
    }

    #[test]
    fn banner_kann_unterdrueckt_werden() {
        // Kein Absturz, keine Ausgabe erwartet.
        print_if(false);
    }
}
