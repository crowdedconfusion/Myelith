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
//! - nicht, wenn die Ausgabe in eine Datei oder Pipe geht: ein
//!   Protokoll, das mit Steuersequenzen beginnt, ist unbrauchbar,
//! - nicht in einem Terminal, das zu klein für den Schriftzug ist,
//! - nicht länger, als jemand zusehen möchte: Ein Tastendruck bricht sie
//!   sofort ab, und ohne Zutun ist sie nach gut zwei Sekunden vorbei.
//!
//! `MYL_NO_ANIMATION=1` schaltet nur die Animation ab und lässt das
//! Banner stehen: für alle, die den Client oft starten.
//!
//! ## Warum der Zufall von Hand kommt
//!
//! Ein Zufallsgenerator-Crate wäre eine Abhängigkeit für einen Effekt.
//! Für fallende Zeichen genügt ein Xorshift, gesetzt aus der Uhr. Er
//! entscheidet nichts, was gemessen oder verglichen wird, der einzige
//! Ort im Client, an dem Zufall überhaupt zulässig ist.

use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::{self, Event};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{cursor, execute, queue};


/// Das Zeichenrepertoire des Regens.
///
/// Ziffern und Hexbuchstaben, dazu die Knoten und Kanten des
/// Projektbanners und θ, das Zeichen, an dem im Projekt der Modellstand
/// hängt. Der Regen sieht damit nach diesem Projekt aus und nicht nach
/// irgendeinem.
const ZEICHEN: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', '∘', '·', '─',
    '╱', '╲', '⊕', '⊗', '≡', '∴', '∵', 'θ', 'λ', 'Σ', 'Δ', '∇', '⋮', '⧉', '⨯', '▚', '▞', '◜', '◞',
];

/// Wie lange der Regen fällt, wenn niemand eine Taste drückt.
/// Gekürzt, als die Spirale dazukam: Das Startbild soll ingesamt nicht
/// länger dauern als vorher, sonst wird aus dem Gruß eine Wartezeit.
const REGENDAUER: Duration = Duration::from_millis(1400);
/// Zeit je Bild. 40 ms sind 25 Bilder je Sekunde.
const BILDDAUER: Duration = Duration::from_millis(40);
/// Zeit je Bild beim Gleiten. Kürzer als [`BILDDAUER`], weil eine
/// Bewegung über zwölf Zeilen sonst eine halbe Sekunde stünde.
const GLEITDAUER: Duration = Duration::from_millis(28);

/// Xorshift64. Reicht für einen Effekt, für nichts sonst.
pub(crate) struct Zufall(u64);

impl Zufall {
    pub(crate) fn neu() -> Self {
        let saat = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x2545_F491_4F6C_DD1D);
        Self(saat | 1)
    }

    pub(crate) fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Gleichverteilt in `0..n`. `n` ist hier immer klein und fest, der
    /// Modulo-Bias ist für einen Bildschirmeffekt ohne Bedeutung.
    pub(crate) fn bis(&mut self, n: usize) -> usize {
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
/// Bildschirm steht**, der Aufrufer darf ihn dann nicht noch einmal
/// drucken. `false` heißt übersprungen, und der Aufrufer druckt das
/// Banner wie zuvor.
pub fn abspielen(farbe: Color) -> bool {
    if std::env::var("MYL_NO_ANIMATION").is_ok_and(|v| v != "0") {
        return false;
    }
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        return false;
    }
    let Ok((breite, hoehe)) = terminal::size() else {
        return false;
    };
    let noetig = crate::banner::fuer_fenster(breite, hoehe).lines().count() as u16 + 2;
    if breite < 62 || hoehe < noetig {
        return false;
    }

    // Schlägt irgendetwas fehl, bleibt nur das Aufräumen wichtig: Ein
    // Terminal ohne sichtbaren Cursor oder mit gesetzter Farbe wäre ein
    // schlechterer Zustand als eine ausgefallene Animation.
    let ergebnis = spielen(breite, hoehe, farbe);
    let _ = execute!(io::stdout(), ResetColor, cursor::Show);
    ergebnis.unwrap_or(false)
}

fn spielen(breite: u16, hoehe: u16, farbe: Color) -> io::Result<bool> {
    // Rohmodus, damit ein Tastendruck sofort ankommt statt erst mit Enter.
    let _roh = crate::auswahl::Rohmodus::an()?;
    let mut aus = io::stdout();
    execute!(
        aus,
        cursor::Hide,
        Clear(ClearType::All),
        Clear(ClearType::Purge)
    )?;

    // Das Logo entsteht in der Bildmitte, dort, wo die Spirale einläuft,
    // und gleitet danach an seinen Platz oben. Entstünde es gleich oben,
    // hätte die Spirale ins Leere gearbeitet.
    let banner_hoehe = crate::banner::fuer_fenster(breite, hoehe).lines().count();
    let versatz = ((hoehe as usize).saturating_sub(banner_hoehe) / 2) as u16;

    regen(&mut aus, breite, hoehe)?;
    einstroemen(&mut aus, breite, hoehe, farbe, versatz)?;
    gleiten(&mut aus, breite, hoehe, farbe, versatz)?;
    Ok(true)
}

/// Sinuswerte für 64 Winkelschritte, mit 1024 skaliert.
///
/// **Ganzzahlig, wie alles in diesem Projekt.** Für eine Bildschirmspirale
/// wäre Gleitkomma zulässig, sie entscheidet nichts. Aber dieses
/// Repositorium prüft nach jeder Änderung mit `grep -n "f32\|f64"`, ob
/// sich Gleitkomma eingeschlichen hat, und wer dabei auf einen Treffer
/// stößt, muss erst nachlesen, dass er harmlos ist. Eine Tabelle mit 64
/// Einträgen kostet weniger als diese Unterbrechung.
const SINUS: [i32; 64] = [
    0, 100, 200, 297, 392, 483, 569, 650, 724, 792, 851, 903, 946, 980, 1004, 1019, 1024, 1019,
    1004, 980, 946, 903, 851, 792, 724, 650, 569, 483, 392, 297, 200, 100, 0, -100, -200, -297,
    -392, -483, -569, -650, -724, -792, -851, -903, -946, -980, -1004, -1019, -1024, -1019, -1004,
    -980, -946, -903, -851, -792, -724, -650, -569, -483, -392, -297, -200, -100,
];

fn sinus(schritt: usize) -> i32 {
    SINUS[schritt % SINUS.len()]
}

/// Kosinus ist der Sinus, um ein Viertel versetzt.
fn kosinus(schritt: usize) -> i32 {
    SINUS[(schritt + SINUS.len() / 4) % SINUS.len()]
}

/// Arme der Spirale.
///
/// Drei, nicht einer: Ein einzelner Arm ist über die halbe Umdrehung vom
/// Bildrand verdeckt und liest sich als Bogen, nicht als Spirale.
const ARME: usize = 3;
/// Stützstellen je Arm, von der Mitte nach außen.
const SCHRITTE: usize = 34;
/// Winkelschritte je Stützstelle. Bei 64 Schritten je Umdrehung sind
/// `SCHRITTE * WINDUNG = 102` gut anderthalb Umdrehungen je Arm.
const WINDUNG: usize = 3;
/// Bilder, über die die Spirale wächst und das Logo entsteht.
/// 80 Bilder à 40 ms sind rund 3,2 Sekunden.
const AUFBAU_BILDER: u32 = 80;
/// So viele Bilder behält ein angekommenes Artefakt seine eigene Farbe,
/// bevor es die des Schriftzugs annimmt.
const NACHGLUEHEN: u32 = 10;

/// Ein Zeichen des Schriftzugs, das noch unterwegs ist.
struct Artefakt {
    ziel: Zelle,
    /// Auf welchem Arm es einströmt.
    arm: usize,
    /// Abstand zur Mitte beim Aufbruch, in Zellen mal 256.
    start_radius: i32,
    /// Bild, in dem es seinen Platz erreicht.
    ankunft: u32,
    /// Eigene Neonfarbe für den Flug und das Nachglühen.
    ton: Color,
}

/// Spirale und Logoaufbau in einem Vorgang: Neonfarbene Artefakte
/// strömen durch die wachsenden Arme nach innen und setzen dort den
/// Schriftzug zusammen.
///
/// **Warum das ein Vorgang ist und nicht zwei.** Vorher lief erst eine
/// Spirale ein und danach baute sich der Schriftzug aus Rauschen auf. Das
/// waren wieder zwei Bilder nacheinander, und die Spirale hatte mit dem
/// Schriftzug nichts zu tun: Sie hätte auch fehlen können. Hier ist die
/// Spirale der **Weg**, auf dem die Zeichen ankommen. Jedes Artefakt
/// gehört von Anfang an zu genau einer Stelle des Schriftzugs, und man
/// sieht ihm an, dass es dorthin unterwegs ist.
///
/// **Die Spirale wächst nach außen, die Artefakte laufen nach innen.**
/// Zwei gegenläufige Bewegungen: Der äußere Radius der Arme nimmt mit
/// jedem Bild zu, während auf ihnen Zeichen zur Mitte hin wandern. Später
/// aufbrechende Artefakte starten deshalb weiter draußen, und das Bild
/// öffnet sich, statt zu schrumpfen.
///
/// **Der Winkel hängt am Radius** (`winkel = i * WINDUNG`), nicht am
/// Zufall. Genau das macht die Kurve sichtbar; mit zufälligen Winkeln
/// entstünde ein Strudel, aber keine Spirale.
///
/// **Kurz vor dem Ziel verlässt ein Artefakt die Bahn.** Die Arme laufen
/// in **einen** Punkt, der Schriftzug belegt aber eine Fläche. Ohne den
/// Übergang stauten sich alle Zeichen in der Mitte und sprängen dann an
/// ihren Platz. Das Gewicht wächst quadratisch: bis kurz vor Schluss
/// bleibt das Artefakt auf der Spirale, dann schwenkt es ein.
///
/// **Angekommene Zeichen glühen nach.** Erst in ihrer eigenen Farbe,
/// danach in der des Schriftzugs. So ist der Schriftzug im Entstehen
/// bunt und am Ende einer: Das Bild zeigt, woraus er gemacht ist, ohne
/// als Flickenteppich stehenzubleiben.
fn einstroemen(
    aus: &mut impl Write,
    breite: u16,
    hoehe: u16,
    farbe: Color,
    versatz: u16,
) -> io::Result<()> {
    let mut z = Zufall::neu();
    let zellen = schriftzug_zellen(breite, hoehe, versatz);
    if zellen.is_empty() {
        return Ok(());
    }

    // Der Regen läuft weiter, während sich der Schriftzug bildet. Ohne ihn
    // entstünde das Logo auf schwarzer Fläche, und aus dem Wasserfall wäre
    // ein Vorspann geworden, der abgeschlossen ist, bevor das Eigentliche
    // beginnt.
    let mut wasserfall = Wasserfall::neu(&mut z, breite, hoehe);

    // Die Mitte der Spirale ist die Mitte des Schriftzugs, und weil der
    // mittig im Fenster sitzt, ist das die Bildmitte. Läge sie woanders,
    // strömten die Artefakte an ihrem Ziel vorbei.
    let (mx, my) = (breite as i32 / 2, hoehe as i32 / 2);
    // Der äußerste Punkt: die halbe Höhe, denn sie ist die knappere der
    // beiden Richtungen.
    let aussen = (hoehe as i32 / 2).max(1) * 256;

    // Gemischte Ankunftsreihenfolge. Von oben nach unten sähe es aus wie
    // ein Bildlauf; verstreutes Ankommen sieht aus wie Verdichtung.
    let mut folge: Vec<usize> = (0..zellen.len()).collect();
    for i in (1..folge.len()).rev() {
        folge.swap(i, z.bis(i + 1));
    }

    // Die ersten Ankünfte erst nach einem Viertel der Zeit: Vorher soll
    // man die Spirale sehen, nicht schon den halben Schriftzug.
    let erste = AUFBAU_BILDER / 4;
    let spanne = AUFBAU_BILDER - erste;

    let mut artefakte: Vec<Artefakt> = Vec::with_capacity(zellen.len());
    for (rang, zelle) in folge.into_iter().map(|i| &zellen[i]).enumerate() {
        let ankunft = erste + (rang as u32 * spanne) / zellen.len().max(1) as u32;
        artefakte.push(Artefakt {
            ziel: Zelle {
                x: zelle.x,
                y: zelle.y,
                zeichen: zelle.zeichen,
                im_schriftzug: zelle.im_schriftzug,
            },
            arm: z.bis(ARME),
            // Wer später ankommt, bricht weiter draußen auf. Daher wächst
            // die Spirale, während sie sich leert.
            start_radius: aussen / 3 + (2 * aussen / 3) * ankunft as i32 / AUFBAU_BILDER as i32,
            ankunft,
            ton: Color::AnsiValue(crate::farben::NEON[z.bis(crate::farben::NEON.len())]),
        });
    }

    // Was nur für ein Bild galt, wird im nächsten ausgewischt. Angekommene
    // Zeichen stehen dagegen fest und werden nie gelöscht.
    let mut fluechtig: Vec<(u16, u16)> = Vec::with_capacity(ARME * SCHRITTE + zellen.len());

    for bild in 0..AUFBAU_BILDER {
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(_) = event::read()? {
                break;
            }
        }

        for (x, y) in fluechtig.drain(..) {
            queue!(aus, cursor::MoveTo(x, y), Print(' '))?;
        }

        // 0. Der Hintergrund. Er darf von allem Folgenden überschrieben
        //    werden, deshalb kommt er zuerst.
        wasserfall.schritt(aus, &mut z)?;

        let drehung = bild as usize * 2;
        // Die Arme reichen mit jedem Bild weiter hinaus.
        let max_radius = aussen * (bild as i32 + 1) / AUFBAU_BILDER as i32;

        // 1. Die Arme selbst, als schwache Bahn.
        for arm in 0..ARME {
            let basis = arm * SINUS.len() / ARME;
            for i in 1..=SCHRITTE {
                let radius = max_radius * i as i32 / SCHRITTE as i32;
                let Some((x, y)) = zelle_bei(radius, basis + drehung + i * WINDUNG, mx, my, breite, hoehe)
                else {
                    continue;
                };
                queue!(
                    aus,
                    cursor::MoveTo(x, y),
                    SetForegroundColor(Color::DarkGreen),
                    SetAttribute(Attribute::NormalIntensity),
                    Print(z.zeichen())
                )?;
                fluechtig.push((x, y));
            }
        }

        // 2. Die Artefakte, die noch unterwegs sind.
        for a in artefakte.iter().filter(|a| bild < a.ankunft) {
            let rest = (a.ankunft - bild) as i32;
            let radius = a.start_radius * rest / a.ankunft.max(1) as i32;
            let i = (radius * SCHRITTE as i32 / aussen.max(1)) as usize;
            let basis = a.arm * SINUS.len() / ARME;
            let Some((sx, sy)) = zelle_bei(radius, basis + drehung + i * WINDUNG, mx, my, breite, hoehe)
            else {
                continue;
            };

            // Quadratisch einschwenken: lange auf der Bahn, spät zum Ziel.
            let p = 256 - (256 * rest / a.ankunft.max(1) as i32);
            let gewicht = (p * p / 256).clamp(0, 256);
            let x = sx as i32 + ((a.ziel.x as i32 - sx as i32) * gewicht) / 256;
            let y = sy as i32 + ((a.ziel.y as i32 - sy as i32) * gewicht) / 256;
            if x < 0 || y < 0 || x >= breite as i32 || y >= hoehe as i32 {
                continue;
            }

            queue!(
                aus,
                cursor::MoveTo(x as u16, y as u16),
                SetForegroundColor(a.ton),
                SetAttribute(Attribute::Bold),
                Print(a.ziel.zeichen)
            )?;
            fluechtig.push((x as u16, y as u16));
        }

        // 3. Die angekommenen, zuletzt: Sie dürfen von nichts überschrieben
        //    werden, was nur ein Bild lang gilt.
        for a in artefakte.iter().filter(|a| bild >= a.ankunft) {
            let (ton, stark) = if bild < a.ankunft + NACHGLUEHEN {
                (a.ton, Attribute::Bold)
            } else {
                crate::banner::zeichenstil(a.ziel.zeichen, a.ziel.im_schriftzug, farbe)
            };
            queue!(
                aus,
                cursor::MoveTo(a.ziel.x, a.ziel.y),
                SetForegroundColor(ton),
                SetAttribute(stark),
                Print(a.ziel.zeichen)
            )?;
        }

        queue!(aus, ResetColor, SetAttribute(Attribute::Reset))?;
        aus.flush()?;
        std::thread::sleep(BILDDAUER);
    }

    // Der Aufbau ist fertig: alles Flüchtige verschwindet, der Schriftzug
    // steht in der Bildmitte und in seiner endgültigen Farbe. Der
    // Rückblätterspeicher wird mit geräumt, sonst läge der ganze Regen
    // darin (Reihenfolge: siehe `banner::bildschirm_mit`).
    queue!(aus, Clear(ClearType::All), Clear(ClearType::Purge))?;
    for a in &artefakte {
        let (ton, stark) =
            crate::banner::zeichenstil(a.ziel.zeichen, a.ziel.im_schriftzug, farbe);
        queue!(
            aus,
            cursor::MoveTo(a.ziel.x, a.ziel.y),
            SetForegroundColor(ton),
            SetAttribute(stark),
            Print(a.ziel.zeichen)
        )?;
    }
    queue!(aus, ResetColor, SetAttribute(Attribute::Reset))?;
    aus.flush()
}

/// Rechnet Polarkoordinaten in eine Bildschirmzelle um.
///
/// `None`, wenn die Stelle außerhalb liegt. **Zwei Spalten je Schritt:**
/// Terminalzellen sind etwa doppelt so hoch wie breit; ohne die Streckung
/// wäre der Kreis ein stehendes Oval.
fn zelle_bei(
    radius: i32,
    winkel: usize,
    mx: i32,
    my: i32,
    breite: u16,
    hoehe: u16,
) -> Option<(u16, u16)> {
    if radius <= 0 {
        return None;
    }
    let x = mx + (2 * radius * kosinus(winkel)) / (256 * 1024);
    let y = my + (radius * sinus(winkel)) / (256 * 1024);
    (x >= 0 && y >= 0 && x < breite as i32 && y < hoehe as i32).then_some((x as u16, y as u16))
}

/// Eine Zelle des fertigen Schriftzugs.
#[derive(Clone)]
struct Zelle {
    x: u16,
    y: u16,
    zeichen: char,
    /// Gehört die Zelle zum Blockschriftzug? Entscheidet mit über ihre
    /// Darstellung: siehe [`crate::banner::zeichenstil`].
    im_schriftzug: bool,
}

/// Die Zellen, die der Schriftzug belegt, um `versatz` Zeilen nach unten
/// verschoben.
///
/// Leerzeichen bleiben draußen: Sie sind kein Bild, sondern dessen
/// Abwesenheit, und was nicht zum Schriftzug gehört, hat kein Artefakt,
/// das dorthin unterwegs wäre.
///
/// Der Versatz ist der Grund, warum diese Funktion existiert und der
/// Bannertext nicht einfach gedruckt wird: Der Schriftzug entsteht in der
/// Bildmitte und wandert danach nach oben, und beides sind dieselben
/// Zellen an verschiedenen Zeilen.
fn schriftzug_zellen(breite: u16, hoehe: u16, versatz: u16) -> Vec<Zelle> {
    crate::banner::fuer_fenster(breite, hoehe)
        .lines()
        .enumerate()
        .flat_map(|(y, zeile)| {
            let im_schriftzug = crate::banner::ist_schriftzug(zeile);
            let y = y + versatz as usize;
            zeile.chars().enumerate().filter_map(move |(x, c)| {
                (c != ' ' && x < breite as usize && y < hoehe as usize).then_some(Zelle {
                    x: x as u16,
                    y: y as u16,
                    zeichen: c,
                    im_schriftzug,
                })
            })
        })
        .collect()
}

/// Der Schriftzug gleitet aus der Bildmitte an seinen Platz oben.
///
/// **Warum überhaupt gleiten.** Die Spirale läuft in der Bildmitte ein,
/// also muss der Schriftzug dort entstehen; sein Platz im Menü ist aber
/// oben. Ein Sprung dorthin wäre ein Schnitt zwischen zwei Bildern, und
/// genau die zu vermeiden ist der Zweck der ganzen Folge. Das Gleiten
/// macht aus zwei Orten eine Bewegung.
///
/// **Nur der frei werdende Streifen wird gelöscht,** nicht das ganze
/// Bild. Ein Vollbild-Löschen je Bild flackert sichtbar; hier bleibt
/// stehen, was ohnehin gleich wieder überschrieben wird.
fn gleiten(
    aus: &mut impl Write,
    breite: u16,
    hoehe: u16,
    farbe: Color,
    versatz: u16,
) -> io::Result<()> {
    let banner = crate::banner::fuer_fenster(breite, hoehe);
    let banner_hoehe = banner.lines().count() as u16;

    for schritt in (0..versatz).rev() {
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(_) = event::read()? {
                break;
            }
        }

        for zelle in schriftzug_zellen(breite, hoehe, schritt) {
            let (ton, stark) =
                crate::banner::zeichenstil(zelle.zeichen, zelle.im_schriftzug, farbe);
            queue!(
                aus,
                cursor::MoveTo(zelle.x, zelle.y),
                SetForegroundColor(ton),
                SetAttribute(stark),
                Print(zelle.zeichen)
            )?;
        }

        // Die Zeilen, die der Schriftzug gerade verlassen hat.
        for y in (schritt + banner_hoehe)..(schritt + banner_hoehe + 1).min(hoehe) {
            queue!(aus, cursor::MoveTo(0, y), Clear(ClearType::CurrentLine))?;
        }

        queue!(aus, ResetColor, SetAttribute(Attribute::Reset))?;
        aus.flush()?;
        std::thread::sleep(GLEITDAUER);
    }

    // Cursor unter den Schriftzug: ab hier schreibt wieder gewöhnliches
    // `println!`.
    queue!(
        aus,
        cursor::MoveTo(0, banner_hoehe),
        ResetColor,
        SetAttribute(Attribute::Reset),
        cursor::Show
    )?;
    aus.flush()
}

/// Zeit je Zeichen beim Schreiben der Begrüßung. 22 ms ergeben rund
/// 45 Zeichen je Sekunde: schnell genug zum Mitlesen und langsam genug,
/// dass es geschrieben aussieht und nicht gedruckt.
const SCHREIBDAUER: Duration = Duration::from_millis(22);
/// Wie lange die fertige Begrüßung stehen bleibt, bevor das Menü kommt.
const NACHLESEN: Duration = Duration::from_millis(900);

/// Schreibt die Begrüßung Zeichen für Zeichen.
///
/// **Warum überhaupt animiert.** Der Nutzername ist die einzige Eingabe,
/// die der Client vor dem Menü verlangt, und ohne Antwort darauf wirkt
/// sie wie ein Formularfeld. Eine geschriebene Begrüßung beantwortet sie
/// sichtbar und füllt zugleich die Pause, in der sonst nichts geschähe.
///
/// **Der Name bekommt die Neonfarbe, der Rest bleibt gedämpft.** Er ist
/// das, was der Nutzer gerade beigesteuert hat; alles andere ist Rahmen.
/// Es ist dieselbe Farbe, in der eben der Schriftzug entstanden ist: Ein
/// Wechsel sähe aus, als hätte der Client das Thema gewechselt.
///
/// Übersprungen wird sie unter denselben Bedingungen wie das Startbild:
/// ohne Terminal, bei `MYL_NO_ANIMATION` und auf Tastendruck. Der Text
/// erscheint dann sofort und vollständig, nicht gar nicht: Er trägt eine
/// Aussage, keine Verzierung.
pub fn begruessung(name: &str, farbe: Color) {
    let zeilen = [
        format!(
            "  Hallo {}, vielen Dank, dass du mithilfst, Myelith zu verbessern!",
            name
        ),
        "  Lass uns zusammen ein paar Tests machen und herausfinden, ob alles".to_string(),
        "  so funktioniert, wie es soll ...".to_string(),
        String::new(),
        "  Falls du Hilfe brauchst, findest du in der Anleitung alles Wichtige.".to_string(),
        "  Viel Spaß beim Testen!".to_string(),
    ];

    let animiert = io::stdout().is_terminal()
        && io::stdin().is_terminal()
        && !std::env::var("MYL_NO_ANIMATION").is_ok_and(|v| v != "0");

    // Mittig unter dem Schriftzug, aber als Block: Die Zeilen behalten
    // ihre Ausrichtung untereinander (siehe `banner::zentriert`).
    let einzug = crate::banner::blockeinzug(
        zeilen.iter().map(|z| z.chars().count()).max().unwrap_or(0),
    );
    let zeilen: Vec<String> = zeilen
        .into_iter()
        .map(|z| {
            if z.trim().is_empty() {
                z
            } else {
                format!("{}{}", einzug, z)
            }
        })
        .collect();

    if !animiert {
        println!();
        for z in &zeilen {
            println!("{}", z);
        }
        println!();
        return;
    }

    let _ = schreiben(&zeilen, farbe, name);
    let _ = execute!(io::stdout(), ResetColor, SetAttribute(Attribute::Reset));
    println!();
    std::thread::sleep(NACHLESEN);
}

fn schreiben(zeilen: &[String], farbe: Color, name: &str) -> io::Result<()> {
    let _roh = crate::auswahl::Rohmodus::an()?;
    let mut aus = io::stdout();
    // Im Rohmodus holt `\n` den Cursor nicht an den Zeilenanfang.
    queue!(aus, Print("\r\n"))?;

    for (nr, zeile) in zeilen.iter().enumerate() {
        // Der Name steht in der ersten Zeile und wird hervorgehoben.
        let hervor = if nr == 0 { Some(name) } else { None };
        let mut rest = zeile.as_str();

        while !rest.is_empty() {
            if event::poll(Duration::from_millis(0))? {
                if let Event::Key(_) = event::read()? {
                    // Abbruch: den Rest sofort und vollständig zeigen.
                    queue!(aus, SetForegroundColor(crate::farben::BEIWERK), Print(rest))?;
                    for weitere in &zeilen[nr + 1..] {
                        queue!(aus, Print("\r\n"), Print(weitere))?;
                    }
                    queue!(aus, ResetColor, Print("\r\n"))?;
                    aus.flush()?;
                    return Ok(());
                }
            }

            // Der Name steht mitten in der Zeile („Hallo Josch, ..."), nicht
            // am Anfang. Getroffen wird er, sobald der Rest mit ihm beginnt.
            // Der Einzug davor ändert daran nichts, er wird Zeichen für
            // Zeichen mitgeschrieben wie der übrige Text.
            let treffer = !name.is_empty() && rest.starts_with(name) && hervor.is_some();
            let (ton, stark) = if treffer {
                (farbe, Attribute::Bold)
            } else {
                (crate::farben::BEIWERK, Attribute::NormalIntensity)
            };
            let laenge = if treffer {
                name.len()
            } else {
                rest.chars().next().map(char::len_utf8).unwrap_or(1)
            };
            let (stueck, uebrig) = rest.split_at(laenge);
            rest = uebrig;

            queue!(
                aus,
                SetForegroundColor(ton),
                SetAttribute(stark),
                Print(stueck)
            )?;
            aus.flush()?;
            std::thread::sleep(SCHREIBDAUER);
        }
        queue!(aus, Print("\r\n"))?;
    }
    aus.flush()
}

/// Der Regen: je Spalte ein Tropfen, der von oben nach unten läuft.
///
/// Gezeichnet wird **nur, was sich ändert**. Kopf, erstes Schweifglied
/// und das Zeichen, das hinten herausfällt. Ein Vollbild je Bild würde
/// bei 25 Bildern je Sekunde sichtbar flackern.
fn regen(aus: &mut impl Write, breite: u16, hoehe: u16) -> io::Result<()> {
    let mut z = Zufall::neu();
    let mut wasserfall = Wasserfall::neu(&mut z, breite, hoehe);

    let start = Instant::now();
    while start.elapsed() < REGENDAUER {
        // Ein Tastendruck bricht ab. `poll` mit Null wartet nicht.
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(_) = event::read()? {
                return Ok(());
            }
        }
        wasserfall.schritt(aus, &mut z)?;
        aus.flush()?;
        std::thread::sleep(BILDDAUER);
    }
    Ok(())
}

/// Der laufende Regen, als Zustand statt als Schleife.
///
/// **Warum getrennt von [`regen`].** Der Regen hört mit der Spirale nicht
/// auf, er läuft im Hintergrund weiter, während sich der Schriftzug
/// bildet. Zwei Vorgänge, die dasselbe Bild bespielen, brauchen einen
/// gemeinsamen Takt: Jeder ruft je Bild einmal [`Wasserfall::schritt`]
/// auf, und wer zuletzt zeichnet, gewinnt. Eine eigene Schleife im Regen
/// könnte das nicht leisten.
struct Wasserfall {
    tropfen: Vec<Tropfen>,
    hoehe: i32,
}

impl Wasserfall {
    fn neu(z: &mut Zufall, breite: u16, hoehe: u16) -> Self {
        let h = hoehe as i32;
        Self {
            tropfen: (0..breite).map(|_| Tropfen::neu(z, h)).collect(),
            hoehe: h,
        }
    }

    /// Ein Bild weiter.
    ///
    /// Gezeichnet wird **nur, was sich ändert**: Kopf, erstes Schweifglied
    /// und das Zeichen, das hinten herausfällt. Ein Vollbild je Bild würde
    /// bei 25 Bildern je Sekunde sichtbar flackern.
    fn schritt(&mut self, aus: &mut impl Write, z: &mut Zufall) -> io::Result<()> {
        let h = self.hoehe;
        for (x, t) in self.tropfen.iter_mut().enumerate() {
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
                *t = Tropfen::neu(z, h);
                t.y = -(z.bis(6) as i32);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In einem Test läuft die Ausgabe nicht auf ein Terminal. Die
    /// Animation muss das erkennen und sofort zurückkehren: ohne
    /// Steuersequenzen, ohne Wartezeit.
    #[test]
    fn ohne_terminal_wird_uebersprungen() {
        let vorher = Instant::now();
        assert!(!abspielen(Color::Green));
        assert!(vorher.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn abschaltbar_ueber_umgebungsvariable() {
        std::env::set_var("MYL_NO_ANIMATION", "1");
        assert!(!abspielen(Color::Green));
        std::env::remove_var("MYL_NO_ANIMATION");
    }

    /// Der Zufall muss verschiedene Werte liefern: ein Xorshift, der bei
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

    /// Die Begrüßung muss in ein 80-Spalten-Terminal passen. Umbrechende
    /// Zeilen sähen aus wie ein Satzfehler, nicht wie ein Gruß.
    #[test]
    fn begruessung_passt_in_achtzig_spalten() {
        // Ein Name von zwölf Zeichen ist reichlich für einen Vor- oder
        // Spitznamen und die Obergrenze, die die erste Zeile trägt.
        for name in ["", "Jo", "Maximiliane"] {
            let erste = format!(
                "  Hallo {}, vielen Dank, dass du mithilfst, Myelith zu verbessern!",
                name
            );
            assert!(
                erste.chars().count() <= 78,
                "erste Zeile mit Namen {name:?} ist {} Zeichen breit",
                erste.chars().count()
            );
        }
        for zeile in [
            "  Lass uns zusammen ein paar Tests machen und herausfinden, ob alles",
            "  so funktioniert, wie es soll ...",
            "  Falls du Hilfe brauchst, findest du in der Anleitung alles Wichtige.",
            "  Viel Spaß beim Testen!",
        ] {
            assert!(
                zeile.chars().count() <= 78,
                "Zeile ist {} Zeichen breit: {zeile}",
                zeile.chars().count()
            );
        }
    }

    /// Die Sinustabelle ist die Grundlage der Spirale. Stimmt sie nicht,
    /// wird aus dem Kreis eine Zickzacklinie.
    #[test]
    fn sinustabelle_beschreibt_einen_kreis() {
        assert_eq!(sinus(0), 0, "sin(0)");
        assert_eq!(sinus(SINUS.len() / 4), 1024, "sin(90 Grad)");
        assert_eq!(sinus(SINUS.len() / 2), 0, "sin(180 Grad)");
        assert_eq!(kosinus(0), 1024, "cos(0)");

        for i in 0..SINUS.len() {
            // Der trigonometrische Pythagoras, in der Skala der Tabelle.
            // Die Schranke deckt die Rundung auf ganze Zahlen ab.
            let r2 = sinus(i) * sinus(i) + kosinus(i) * kosinus(i);
            assert!(
                (r2 - 1024 * 1024).abs() < 4000,
                "Schritt {i}: Radius weicht ab ({r2})"
            );
            // Über die Tabellengrenze hinaus muss weitergezählt werden
            // können: Der Winkel eines Funkens wächst unbegrenzt.
            assert_eq!(sinus(i), sinus(i + SINUS.len()));
        }
    }

    /// Die Spirale hat einen Anfang und ein Ende: außen beginnen, in der
    /// Mitte ankommen. Bliebe der Radius stehen, kreisten die Zeichen
    /// ewig, und der Schriftzug entstünde in einem Zeichenfeld statt auf
    /// einer geräumten Fläche.
    /// Eine Spirale erkennt man daran, dass der Winkel mit dem Radius
    /// wächst: Nur dann ist eine Kurve sichtbar. Bliebe der Winkel
    /// konstant, wären es Speichen; wäre er zufällig, eine Punktwolke.
    /// Beides hatte die erste Fassung, und beides ist keine Spirale.
    #[test]
    fn arme_beschreiben_eine_kurve() {
        let winkel: Vec<usize> = (1..=SCHRITTE).map(|i| i * WINDUNG).collect();
        assert!(
            winkel.windows(2).all(|w| w[1] > w[0]),
            "der Winkel wächst nicht mit dem Radius"
        );
        // Mindestens eine volle Umdrehung, sonst liest sich der Arm als
        // Bogen statt als Spirale.
        assert!(
            SCHRITTE * WINDUNG >= SINUS.len(),
            "ein Arm umläuft die Mitte nicht einmal ({} von {} Schritten)",
            SCHRITTE * WINDUNG,
            SINUS.len()
        );
        // Die Arme sind gleichmäßig über die Umdrehung verteilt, und es
        // sind mehrere: Ein einzelner Arm ist über die halbe Umdrehung vom
        // Bildrand verdeckt und liest sich als Bogen.
        let basen: Vec<usize> = (0..ARME).map(|a| a * SINUS.len() / ARME).collect();
        assert!(basen.len() >= 2, "ein einzelner Arm ist halb verdeckt");
        assert!(basen.iter().all(|b| *b < SINUS.len()));
        let abstand = basen[1] - basen[0];
        assert!(
            basen.windows(2).all(|w| w[1] - w[0] == abstand),
            "die Arme hängen einseitig: {:?}",
            basen
        );
    }

    /// Das Logo entsteht in der Bildmitte und gleitet nach oben. Beides
    /// sind dieselben Zellen an verschiedenen Zeilen; verschöbe der
    /// Versatz auch die Spalten, verrutschte der Schriftzug beim Gleiten.
    #[test]
    fn versatz_verschiebt_nur_die_zeilen() {
        let oben = schriftzug_zellen(120, 60, 0);
        let mitte = schriftzug_zellen(120, 60, 12);
        assert_eq!(oben.len(), mitte.len(), "Zellen gehen beim Versatz verloren");
        for (o, m) in oben.iter().zip(mitte.iter()) {
            assert_eq!(o.x, m.x, "Spalte verschoben");
            assert_eq!(m.y, o.y + 12, "Zeile falsch verschoben");
            assert_eq!(o.zeichen, m.zeichen);
        }
    }

    /// Die Arme wachsen nach außen, während die Artefakte nach innen
    /// laufen. Zwei gegenläufige Bewegungen: Liefe beides in dieselbe
    /// Richtung, sähe es aus wie ein Zoom, nicht wie ein Einströmen.
    #[test]
    fn arme_wachsen_waehrend_die_artefakte_einlaufen() {
        let aussen = 22 * 256;
        let n = AUFBAU_BILDER as i32;

        let arme: Vec<i32> = (0..n).map(|b| aussen * (b + 1) / n).collect();
        assert!(
            arme.windows(2).all(|w| w[1] >= w[0]),
            "die Arme wachsen nicht"
        );
        assert!(arme[0] < arme[arme.len() - 1] / 4, "die Spirale öffnet sich kaum");

        // Ein Artefakt mit fester Ankunft läuft monoton nach innen.
        let ankunft = n / 2;
        let start = aussen;
        let bahn: Vec<i32> = (0..ankunft).map(|b| start * (ankunft - b) / ankunft).collect();
        assert!(
            bahn.windows(2).all(|w| w[1] <= w[0]),
            "das Artefakt läuft nicht nach innen"
        );
        assert_eq!(*bahn.last().expect("Bahn"), start / ankunft, "kommt nicht an");
    }

    /// Wer später ankommt, bricht weiter draußen auf. Ohne das säßen alle
    /// Aufbrüche auf demselben Ring, und die Spirale wüchse nur in ihrer
    /// Verzierung, nicht in dem, was sie transportiert.
    #[test]
    fn spaetere_artefakte_starten_weiter_draussen() {
        let aussen = 22 * 256;
        let radius = |ankunft: u32| {
            aussen / 3 + (2 * aussen / 3) * ankunft as i32 / AUFBAU_BILDER as i32
        };
        let frueh = radius(AUFBAU_BILDER / 4);
        let spaet = radius(AUFBAU_BILDER);
        assert!(spaet > frueh, "{spaet} ist nicht weiter draußen als {frueh}");
        assert!(frueh > 0, "ein Artefakt startet in der Mitte");
        assert!(spaet <= aussen, "ein Artefakt startet außerhalb des Bildes");
    }

    /// Kurz vor dem Ziel verlässt ein Artefakt die Bahn: Die Arme laufen
    /// in einen Punkt, der Schriftzug belegt eine Fläche. Das Gewicht muss
    /// spät einsetzen, sonst fliegen die Zeichen geradeaus statt spiralig.
    #[test]
    fn einschwenken_setzt_spaet_ein() {
        let gewicht = |p: i32| (p * p / 256).clamp(0, 256);
        assert_eq!(gewicht(0), 0, "am Anfang schon abgelenkt");
        assert_eq!(gewicht(256), 256, "am Ende nicht am Ziel");
        assert!(gewicht(128) < 80, "auf halber Strecke schon zu stark abgelenkt");
        assert!(
            (0..256).all(|p| gewicht(p) <= gewicht(p + 1)),
            "das Einschwenken springt zurück"
        );
    }

    /// Die Zellen des Schriftzugs müssen genau die nicht-leeren Stellen
    /// des Banners treffen: sie sind das Ziel, in das der Sturm einrastet.
    #[test]
    fn schriftzug_zellen_decken_das_banner_ab() {
        let zellen = schriftzug_zellen(200, 60, 0);
        let erwartet: usize = crate::banner::fuer_breite(200)
            .lines()
            .map(|z| z.chars().filter(|c| *c != ' ').count())
            .sum();
        assert_eq!(zellen.len(), erwartet, "Zellen und Banner weichen ab");
        assert!(zellen.iter().any(|z| z.zeichen == '█'), "Schriftzug fehlt");
        assert!(!zellen.iter().any(|z| z.zeichen == ' '), "Leerzeichen als Zelle");
    }

    /// In einem Fenster, das kleiner ist als das Banner, dürfen keine
    /// Zellen außerhalb liegen: `MoveTo` würde sie sonst an den Rand
    /// klemmen und den Schriftzug verzerren.
    #[test]
    fn schriftzug_zellen_bleiben_im_fenster() {
        let (b, h) = (30u16, 8u16);
        for zelle in schriftzug_zellen(b, h, 0) {
            assert!(zelle.x < b && zelle.y < h, "Zelle außerhalb: {},{}", zelle.x, zelle.y);
        }
    }

    /// Ein Tropfen startet oberhalb des Bildes und fällt nach unten:
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
