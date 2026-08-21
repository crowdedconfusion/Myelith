//! Startbild: fallende Zeichen, dann der Aufbau des Schriftzugs.
//!
//! Der Client wird von Leuten gestartet, die ihre Maschine für einen
//! Cross-Hardware-Test beisteuern und das Projekt sonst nicht sehen. Das
//! Startbild ist die einzige Stelle, an der es sich vorstellt.
//!
//! ## Was sie nicht darf
//!
//! Die Animation läuft **einmal**, vor dem Menü, und nur dann:
//!
//! - nicht bei `--quiet` und nicht bei gesetztem `MYL_NO_BANNER`,
//! - nicht, wenn die Ausgabe in eine Datei oder Pipe geht — ein
//!   Protokoll, das mit Steuersequenzen beginnt, ist unbrauchbar,
//! - nicht in einem Terminal, das zu klein für den Schriftzug ist,
//! - nicht länger, als jemand zusehen möchte: Ein Tastendruck bricht sie
//!   sofort ab, und ohne Zutun ist sie nach gut zwei Sekunden vorbei.
//!
//! `MYL_NO_ANIMATION=1` schaltet nur die Animation ab und lässt das
//! Banner stehen — für alle, die den Client oft starten.
//!
//! ## Warum der Zufall von Hand kommt
//!
//! Ein Zufallsgenerator-Crate wäre eine Abhängigkeit für einen Effekt.
//! Für fallende Zeichen genügt ein Xorshift, gesetzt aus der Uhr. Er
//! entscheidet nichts, was gemessen oder verglichen wird — der einzige
//! Ort im Client, an dem Zufall überhaupt zulässig ist.

use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::{self, Event};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{cursor, execute, queue};

use crate::banner::BANNER;

/// Das Zeichenrepertoire des Regens.
///
/// Ziffern und Hexbuchstaben, dazu die Knoten und Kanten des
/// Projektbanners und θ — das Zeichen, an dem im Projekt der Modellstand
/// hängt. Der Regen sieht damit nach diesem Projekt aus und nicht nach
/// irgendeinem.
const ZEICHEN: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', '∘', '·', '─',
    '╱', '╲', '⊕', '⊗', '≡', '∴', '∵', 'θ', 'λ', 'Σ', 'Δ', '∇', '⋮', '⧉', '⨯', '▚', '▞', '◜', '◞',
];

/// Wie lange der Regen fällt, wenn niemand eine Taste drückt.
const REGENDAUER: Duration = Duration::from_millis(2200);
/// Zeit je Bild. 40 ms sind 25 Bilder je Sekunde.
const BILDDAUER: Duration = Duration::from_millis(40);
/// Bilder, über die sich der Schriftzug aus dem Sturm zusammensetzt.
/// 45 Bilder à 40 ms sind rund 1,8 Sekunden.
const STURMBILDER: u32 = 45;

/// Xorshift64. Reicht für einen Effekt, für nichts sonst.
struct Zufall(u64);

impl Zufall {
    fn neu() -> Self {
        let saat = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x2545_F491_4F6C_DD1D);
        Self(saat | 1)
    }

    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Gleichverteilt in `0..n`. `n` ist hier immer klein und fest, der
    /// Modulo-Bias ist für einen Bildschirmeffekt ohne Bedeutung.
    fn bis(&mut self, n: usize) -> usize {
        (self.naechste() % n.max(1) as u64) as usize
    }

    fn zeichen(&mut self) -> char {
        ZEICHEN[self.bis(ZEICHEN.len())]
    }
}

/// Eine fallende Spalte.
struct Tropfen {
    /// Kopfposition, als Zeile. Startet oberhalb des Bildes.
    y: i32,
    /// Zeilen je Bild.
    tempo: i32,
    /// Länge des Schweifs.
    laenge: i32,
}

impl Tropfen {
    fn neu(z: &mut Zufall, hoehe: i32) -> Self {
        Self {
            y: -(z.bis(hoehe as usize * 2) as i32),
            tempo: 1 + z.bis(3) as i32,
            laenge: 4 + z.bis(14) as i32,
        }
    }
}

/// Spielt das Startbild ab.
///
/// Rückgabe: `true`, wenn der Schriftzug danach **bereits auf dem
/// Bildschirm steht** — der Aufrufer darf ihn dann nicht noch einmal
/// drucken. `false` heißt übersprungen, und der Aufrufer druckt das
/// Banner wie zuvor.
pub fn abspielen() -> bool {
    if std::env::var("MYL_NO_ANIMATION").is_ok_and(|v| v != "0") {
        return false;
    }
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        return false;
    }
    let Ok((breite, hoehe)) = terminal::size() else {
        return false;
    };
    let noetig = BANNER.lines().count() as u16 + 2;
    if breite < 62 || hoehe < noetig {
        return false;
    }

    // Schlägt irgendetwas fehl, bleibt nur das Aufräumen wichtig: Ein
    // Terminal ohne sichtbaren Cursor oder mit gesetzter Farbe wäre ein
    // schlechterer Zustand als eine ausgefallene Animation.
    let ergebnis = spielen(breite, hoehe);
    let _ = execute!(io::stdout(), ResetColor, cursor::Show);
    ergebnis.unwrap_or(false)
}

fn spielen(breite: u16, hoehe: u16) -> io::Result<bool> {
    // Rohmodus, damit ein Tastendruck sofort ankommt statt erst mit Enter.
    let _roh = crate::auswahl::Rohmodus::an()?;
    let mut aus = io::stdout();
    execute!(aus, cursor::Hide, Clear(ClearType::All))?;

    regen(&mut aus, breite, hoehe)?;
    sturm(&mut aus, breite, hoehe)?;
    Ok(true)
}

/// Eine Zelle des fertigen Schriftzugs.
struct Zelle {
    x: u16,
    y: u16,
    zeichen: char,
}

/// Die Zellen, die der Schriftzug am Ende belegt.
///
/// Leerzeichen bleiben draußen: Sie sind kein Bild, sondern dessen
/// Abwesenheit — und was nicht zum Schriftzug gehört, soll im Sturm
/// verwehen statt als Lücke stehenzubleiben.
fn schriftzug_zellen(breite: u16, hoehe: u16) -> Vec<Zelle> {
    BANNER
        .lines()
        .enumerate()
        .flat_map(|(y, zeile)| {
            zeile.chars().enumerate().filter_map(move |(x, c)| {
                (c != ' ' && x < breite as usize && y < hoehe as usize).then_some(Zelle {
                    x: x as u16,
                    y: y as u16,
                    zeichen: c,
                })
            })
        })
        .collect()
}

/// Der Sturm: Aus dem herabgefallenen Rauschen setzt sich der Schriftzug
/// zusammen.
///
/// **Warum kein Löschen zwischen Regen und Schriftzug.** Die erste
/// Fassung machte den Bildschirm leer und baute den Schriftzug danach
/// Zeile für Zeile auf. Das waren zwei Bilder nacheinander, und der
/// Zusammenhang zwischen ihnen ging verloren: Die fallenden Zeichen
/// hatten mit dem Schriftzug nichts zu tun. Hier bleibt das Rauschen
/// stehen und wird dichter, während immer mehr Zellen des Schriftzugs
/// **an ihrer endgültigen Stelle** einrasten. Das Bild entsteht sichtbar
/// aus dem, was vorher herabgefallen ist.
///
/// **Die Reihenfolge ist gemischt, nicht zeilenweise.** Ein Aufbau von
/// oben nach unten sähe aus wie ein Bildlauf; verstreutes Einrasten sieht
/// aus wie Verdichtung.
///
/// **Bereits eingerastete Zellen werden in jedem Bild neu gezeichnet.**
/// Das Rauschen wählt seine Stellen frei und träfe sie sonst wieder; die
/// paar hundert Schreibvorgänge je Bild kosten nichts.
fn sturm(aus: &mut impl Write, breite: u16, hoehe: u16) -> io::Result<()> {
    let mut z = Zufall::neu();
    let zellen = schriftzug_zellen(breite, hoehe);
    if zellen.is_empty() {
        return Ok(());
    }

    // Fisher-Yates: jede Reihenfolge gleich wahrscheinlich.
    let mut folge: Vec<usize> = (0..zellen.len()).collect();
    for i in (1..folge.len()).rev() {
        folge.swap(i, z.bis(i + 1));
    }

    let flaeche = breite as usize * hoehe as usize;
    // So viele Rauschzeichen je Bild, dass die Fläche in Bewegung bleibt,
    // ohne dass das Zeichnen die Bildrate bestimmt.
    let rauschen_je_bild = (flaeche / 12).clamp(40, 900);

    for bild in 0..STURMBILDER {
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(_) = event::read()? {
                break;
            }
        }

        // Der Sturm lässt gegen Ende nach, damit der Schriftzug freikommt.
        let rest = (STURMBILDER - bild) as usize;
        for _ in 0..(rauschen_je_bild * rest / STURMBILDER as usize) {
            queue!(
                aus,
                cursor::MoveTo(z.bis(breite as usize) as u16, z.bis(hoehe as usize) as u16),
                SetForegroundColor(Color::DarkGreen),
                SetAttribute(Attribute::NormalIntensity),
                Print(z.zeichen())
            )?;
        }

        let eingerastet = zellen.len() * (bild as usize + 1) / STURMBILDER as usize;
        for &i in &folge[..eingerastet] {
            let zelle = &zellen[i];
            queue!(
                aus,
                cursor::MoveTo(zelle.x, zelle.y),
                SetForegroundColor(Color::Green),
                SetAttribute(Attribute::Bold),
                Print(zelle.zeichen)
            )?;
        }

        aus.flush()?;
        std::thread::sleep(BILDDAUER);
    }

    // Der Sturm legt sich: alles außer dem Schriftzug verschwindet.
    queue!(aus, Clear(ClearType::All))?;
    for zelle in &zellen {
        queue!(
            aus,
            cursor::MoveTo(zelle.x, zelle.y),
            SetForegroundColor(Color::Green),
            SetAttribute(Attribute::Bold),
            Print(zelle.zeichen)
        )?;
    }

    // Cursor unter den Schriftzug, Farben zurück — ab hier schreibt
    // wieder gewöhnliches `println!`.
    queue!(
        aus,
        cursor::MoveTo(0, BANNER.lines().count() as u16),
        ResetColor,
        SetAttribute(Attribute::Reset),
        cursor::Show
    )?;
    aus.flush()
}

/// Der Regen: je Spalte ein Tropfen, der von oben nach unten läuft.
///
/// Gezeichnet wird **nur, was sich ändert** — Kopf, erstes Schweifglied
/// und das Zeichen, das hinten herausfällt. Ein Vollbild je Bild würde
/// bei 25 Bildern je Sekunde sichtbar flackern.
fn regen(aus: &mut impl Write, breite: u16, hoehe: u16) -> io::Result<()> {
    let mut z = Zufall::neu();
    let h = hoehe as i32;
    let mut tropfen: Vec<Tropfen> = (0..breite).map(|_| Tropfen::neu(&mut z, h)).collect();

    let start = Instant::now();
    while start.elapsed() < REGENDAUER {
        // Ein Tastendruck bricht ab. `poll` mit Null wartet nicht.
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(_) = event::read()? {
                return Ok(());
            }
        }

        for (x, t) in tropfen.iter_mut().enumerate() {
            let x = x as u16;

            // Der bisherige Kopf wird zum Schweif: gedämpft nachziehen.
            if t.y >= 0 && t.y < h {
                queue!(
                    aus,
                    cursor::MoveTo(x, t.y as u16),
                    SetForegroundColor(Color::DarkGreen),
                    SetAttribute(Attribute::NormalIntensity),
                    Print(z.zeichen())
                )?;
            }

            t.y += t.tempo;

            // Neuer Kopf, hell.
            if t.y >= 0 && t.y < h {
                queue!(
                    aus,
                    cursor::MoveTo(x, t.y as u16),
                    SetForegroundColor(Color::White),
                    SetAttribute(Attribute::Bold),
                    Print(z.zeichen())
                )?;
            }

            // Hinteres Ende löschen, sonst füllt sich das Bild.
            let ende = t.y - t.laenge;
            if ende >= 0 && ende < h {
                queue!(aus, cursor::MoveTo(x, ende as u16), Print(' '))?;
            }

            // Unten heraus: oben neu ansetzen.
            if ende >= h {
                *t = Tropfen::neu(&mut z, h);
                t.y = -(z.bis(6) as i32);
            }
        }

        aus.flush()?;
        std::thread::sleep(BILDDAUER);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In einem Test läuft die Ausgabe nicht auf ein Terminal. Die
    /// Animation muss das erkennen und sofort zurückkehren — ohne
    /// Steuersequenzen, ohne Wartezeit.
    #[test]
    fn ohne_terminal_wird_uebersprungen() {
        let vorher = Instant::now();
        assert!(!abspielen());
        assert!(vorher.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn abschaltbar_ueber_umgebungsvariable() {
        std::env::set_var("MYL_NO_ANIMATION", "1");
        assert!(!abspielen());
        std::env::remove_var("MYL_NO_ANIMATION");
    }

    /// Der Zufall muss verschiedene Werte liefern — ein Xorshift, der bei
    /// einer schlechten Saat stehenbleibt, ergäbe senkrechte Streifen.
    #[test]
    fn zufall_liefert_verschiedene_werte() {
        let mut z = Zufall::neu();
        let erste = z.naechste();
        assert!((0..8).any(|_| z.naechste() != erste));
    }

    #[test]
    fn zufall_bleibt_im_bereich() {
        let mut z = Zufall::neu();
        for _ in 0..200 {
            assert!(z.bis(7) < 7);
            assert!(ZEICHEN.contains(&z.zeichen()));
        }
        // `bis(0)` darf nicht durch Null teilen.
        assert_eq!(z.bis(0), 0);
    }

    /// Die Zellen des Schriftzugs müssen genau die nicht-leeren Stellen
    /// des Banners treffen — sie sind das Ziel, in das der Sturm einrastet.
    #[test]
    fn schriftzug_zellen_decken_das_banner_ab() {
        let zellen = schriftzug_zellen(200, 60);
        let erwartet: usize = BANNER.lines().map(|z| z.chars().filter(|c| *c != ' ').count()).sum();
        assert_eq!(zellen.len(), erwartet, "Zellen und Banner weichen ab");
        assert!(zellen.iter().any(|z| z.zeichen == '█'), "Schriftzug fehlt");
        assert!(!zellen.iter().any(|z| z.zeichen == ' '), "Leerzeichen als Zelle");
    }

    /// In einem Fenster, das kleiner ist als das Banner, dürfen keine
    /// Zellen außerhalb liegen — `MoveTo` würde sie sonst an den Rand
    /// klemmen und den Schriftzug verzerren.
    #[test]
    fn schriftzug_zellen_bleiben_im_fenster() {
        let (b, h) = (30u16, 8u16);
        for zelle in schriftzug_zellen(b, h) {
            assert!(zelle.x < b && zelle.y < h, "Zelle außerhalb: {},{}", zelle.x, zelle.y);
        }
    }

    /// Ein Tropfen startet oberhalb des Bildes und fällt nach unten —
    /// Tempo Null hinge fest, negatives Tempo liefe rückwärts.
    #[test]
    fn tropfen_faellt_nach_unten() {
        let mut z = Zufall::neu();
        for _ in 0..50 {
            let t = Tropfen::neu(&mut z, 24);
            assert!(t.y <= 0, "Start bei {}", t.y);
            assert!(t.tempo >= 1, "Tempo {}", t.tempo);
            assert!(t.laenge >= 4, "Länge {}", t.laenge);
        }
    }
}
