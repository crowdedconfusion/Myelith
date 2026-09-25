// ⚑ **Bis zum 2026-09-24 eine wortgetreue Kopie aus dem Testclient,
// seither gehoert diese Datei der Konsole** (Festlegung des
// Projektinhabers). Anlass war das Logo im Regenbogen,
// das nur hier waehrend eines Auftrags fliesst.
// Der Testclient behaelt seine Fassung und wird ohnehin abgeraeumt;
// die Probe `die_kopien_sind_wortgetreu` wacht nur noch ueber
// `auswahl.rs`.

//! Begrüßungsbanner.
//!
//! Die Tagline des Projektbanners bleibt bewusst weg: im Terminal
//! steht darunter ohnehin sofort das Menü, und drei Textzeilen zwischen
//! Schriftzug und Auswahl drängen die eigentliche Bedienung nach unten.
//!
//! ## ⚑ Warum das Motiv so aussieht: heilige Geometrie
//!
//! **Auftrag des Projektinhabers vom 2026-09-24:** rein geometrisch, nach
//! dem Vorbild der heiligen Geometrie, und **nichts beruehrt das Logo**.
//! Acht Motive, eines je Sitzung gewuerfelt; wie sie entstehen und was sie
//! zusichern, steht bei [`crate::geometrie`].
//!
//! 📌 **Davor standen hier zwei Fassungen**, beide am selben Tag: ein Netz
//! aus Knoten und Kanten nach dem Projektbanner, dann Neuronen ueber
//! Gestein. Die erste las sich als Schema, die zweite war dem
//! Projektinhaber zu gegenstaendlich.
//!
//! ⚑ **Stufen der Farbe** ([`stufe`]): Die Bloecke des Schriftzugs
//! leuchten im vollen Verlauf, seine Schattenkanten gedaempft, das Muster
//! dunkler und entsaettigt ([`zeichenstil`]). **Und waehrend eines
//! Auftrags fliessen sie gegeneinander** ([`crate::schimmer`]): Das
//! Logo wandert nach links, das Muster nach rechts, und der Unterschied
//! in Helle und Richtung gibt dem Bild Tiefe.
//!
//! ## Warum das Motiv zur Laufzeit entsteht
//!
//! Bis v0.6.0 stand es als fester Text im Quelltext, mit der Begründung,
//! ein Generator sei Aufwand für ein Bild, das sich nie ändert. Diese
//! Begründung ist entfallen: Das Bild soll die Fensterbreite füllen, also
//! ändert es sich bei jedem anderen Terminal. [`fuer_fenster`] baut es auf
//! einer Punktflaeche ([`crate::geometrie::Leinwand`]).
//!
//! Der Schriftzug bleibt dabei **56 Zeichen breit und wird zentriert**,
//! nicht gestreckt: Er ist ein Bild, kein Text, und in die Breite gezogen
//! unleserlich. Gefuellt wird die Flaeche vom Muster um ihn herum.
//!
//! **Auch die Höhe zählt.** Passt das Motiv nicht mitsamt Menü ins
//! Fenster, scrollt der Schriftzug nach oben weg. Gekürzt wird deshalb von
//! unten nach oben: erst das Muster unter dem Schriftzug, dann das
//! darueber; neben ihm bleibt es immer. Fuer zu schmale Fenster steht
//! dasselbe Motiv in seiner schmalsten Form ([`ersatzbild`]).
//!
//! **Nicht immer anzeigen:** Bei `--quiet` und wenn die Ausgabe in eine
//! Datei oder Pipe geht, bleibt das Banner weg. Ein Protokoll, das mit
//! ASCII-Kunst beginnt, ist schlechter zu diffen.

/// Die Breite des Ersatzbildes: Schriftzug und ein Zeichen Rand je Seite.
pub const ERSATZBREITE: usize = SCHRIFTBREITE + 2;

/// **Das Bild fuer Fenster, die zu schmal sind, und fuer Ausgaben ohne
/// Terminal.**
///
/// ⚑ **Erzeugt und nicht abgeschrieben** (2026-09-24). Bis dahin stand
/// hier ein fester Text, der das Motiv von Hand nachbildete; mit dem
/// neuen Motiv waeren es zwei Bilder gewesen, die von Hand gleich zu
/// halten sind. **Was an zwei Orten steht, laeuft auseinander.** Jetzt
/// ist es dasselbe Motiv in seiner schmalsten Form.
pub fn ersatzbild() -> String {
    bild(ERSATZBREITE, u16::MAX)
}

/// Untertitel, direkt unter dem Banner.
///
/// ⚑ **Die einzige Zeile, die in der Kopie geaendert ist.** Alles
/// andere in dieser Datei steht wortgetreu so da wie in ihrer Quelle,
/// damit sich beide gegeneinander halten lassen; **ein Untertitel, der
/// vom Testclient spricht, waere in diesem Programm schlicht falsch.**
pub const SUBTITLE: &str = "";

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
///
/// ⚑ **Die Farben kommen aus [`crate::schimmer::zellstil`]**, also aus
/// derselben Stelle wie im Startbild und beim Fliessen, und im Stand der
/// Uhr, bei dem das Logo zuletzt stehenblieb. Ein Neudruck zwischen zwei
/// Auftraegen zeigt deshalb genau das Bild, das vorher dastand.
///
/// ⚑ **Und er meldet, wo das Logo jetzt steht** ([`crate::schimmer::gedruckt`]):
/// oben links, auf einem eben geraeumten Schirm. Von dort aus darf es
/// fliessen, bis es wegrollt.
pub fn bildschirm_mit(design: myl_client::einstellungen::Konsolendesign) {
    use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};
    use std::io::IsTerminal;

    if std::env::var("MYL_NO_BANNER").is_ok() {
        return;
    }
    // Ohne Terminal keine Steuerzeichen: Ein mitgeschnittener Lauf soll
    // lesbar bleiben, und Farbcodes in einer Datei sind es nicht.
    if !std::io::stdout().is_terminal() {
        println!("{}", ersatzbild());
        if !SUBTITLE.trim().is_empty() {
            println!("{}\n", SUBTITLE);
        }
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
    let uhr = crate::schimmer::stand();
    for (y, zeile) in text.lines().enumerate() {
        for (x, c) in zeile.chars().enumerate() {
            let (ton, stark) = crate::schimmer::zellstil(design, c, x, y, breite as usize, uhr);
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
    if SUBTITLE.trim().is_empty() {
        let _ = crossterm::queue!(aus, Print("\n"));
    } else {
        let _ = crossterm::queue!(
            aus,
            SetForegroundColor(crate::farben::BEIWERK),
            Print(untertitel(breite)),
            ResetColor,
            Print("\n\n")
        );
    }
    let _ = std::io::Write::flush(&mut aus);
    crate::schimmer::gedruckt(&text, 0, (breite, hoehe), design);
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
    // Ohne Untertitel gibt es nichts einzuruecken.
    if SUBTITLE.trim().is_empty() {
        return String::new();
    }
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
/// Der Schriftzug misst 56; ein Test haelt die Zahl gegen ihn, damit sie
/// nicht auseinanderlaeuft.
pub const SCHRIFTBREITE: usize = 56;

/// Schmalste Breite, für die überhaupt gezeichnet wird.
///
/// Unterhalb passt der Schriftzug nicht, und ein umbrechendes Banner sieht
/// schlimmer aus als keines.
pub const MINDESTBREITE: usize = SCHRIFTBREITE + 4;

/// Das Banner in der gewünschten Breite.
///
/// Der Schriftzug bleibt 58 Zeichen breit: er ist ein Bild, kein Text, und
/// gestreckt wäre er unleserlich. Er wird deshalb **zentriert**, und das
/// Muster fuellt die Breite um ihn herum.
///
/// Unterhalb von [`MINDESTBREITE`] kommt das Ersatzbild.
///
/// ⚑ **Nur noch fuer die Proben**: Gezeichnet wird immer fuer Breite und
/// Hoehe ([`fuer_fenster`]).
#[cfg(test)]
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
        return ersatzbild();
    }
    bild(b, hoehe)
}

/// Das Bild selbst, im Motiv dieser Sitzung.
fn bild(b: usize, hoehe: u16) -> String {
    bild_mit(crate::geometrie::sitzungsmotiv(), b, hoehe)
}

/// **Das Bild in einem bestimmten Motiv.**
///
/// ⚑ **Die Zeilenzahl ist dieselbe wie in jeder frueheren Fassung**:
/// 18 Zeilen ab [`VOLLE_HOEHE`], 13 ab [`HALBE_HOEHE`], sonst 7, und
/// danach eine leere. Wie viel Platz das Menue darunter hat, ist
/// nachgerechnet und haengt daran. Die erste Zeile bleibt leer, damit das
/// Bild nicht an der Fensterkante klebt.
///
/// 📌 **Die Flaeche endet in der halben und der knappen Hoehe genau an der
/// Sperrzone**, und die Leerzeile danach ist echt. Endete sie mit einer
/// Musterzeile, hinge die Zeilenzahl am Motiv: leer in dem einen, gefuellt
/// im anderen, und `lines()` zaehlt eine leere letzte Zeile nicht mit.
pub(crate) fn bild_mit(motiv: crate::geometrie::Motiv, b: usize, hoehe: u16) -> String {
    let (reihen, logo_oben) = if hoehe >= VOLLE_HOEHE {
        (17, 6)
    } else if hoehe >= HALBE_HOEHE {
        (12, 6)
    } else {
        (6, 0)
    };
    let mut flaeche = crate::geometrie::Leinwand::neu(b, reihen, logo_oben);
    motiv.zeichnen(&mut flaeche);
    let mut zeilen = vec![String::new()];
    zeilen.extend(flaeche.zeilen());
    zeilen.push(String::new());
    zeilen.join("\n")
}

/// Der Blockschriftzug allein, ohne Muster und ohne Einzug.
pub(crate) const SCHRIFTZUG: [&str; 6] = [
    "███╗   ███╗██╗   ██╗███████╗██╗     ██╗████████╗██╗  ██╗",
    "████╗ ████║╚██╗ ██╔╝██╔════╝██║     ██║╚══██╔══╝██║  ██║",
    "██╔████╔██║ ╚████╔╝ █████╗  ██║     ██║   ██║   ███████║",
    "██║╚██╔╝██║  ╚██╔╝  ██╔══╝  ██║     ██║   ██║   ██╔══██║",
    "██║ ╚═╝ ██║   ██║   ███████╗███████╗██║   ██║   ██║  ██║",
    "╚═╝     ╚═╝   ╚═╝   ╚══════╝╚══════╝╚═╝   ╚═╝   ╚═╝  ╚═╝",
];

/// Wie stark ein Zeichen des Bildes leuchtet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stufe {
    /// Voller Verlauf, fett: die Bloecke des Schriftzugs.
    Leuchtend,
    /// Gedaempfter Verlauf: die Schattenkanten des Schriftzugs.
    Schatten,
    /// Dunkler und entsaettigter Verlauf: das Muster.
    Getoent,
    /// Grau: alles andere, was ein Bild je zeigen sollte.
    Kante,
}

/// Die Zeichen, aus denen der Schriftzug besteht.
const LOGOZEICHEN: [char; 7] = ['█', '╗', '╔', '╝', '╚', '═', '║'];

/// Wie hell die Schattenkanten des Schriftzugs stehen, in Prozent.
const SCHATTEN: u32 = 55;
/// Wie hell das Muster steht, in Prozent.
const MUSTER_HELLE: u32 = 48;
/// Wie viel Saettigung das Muster behaelt, in Prozent.
const MUSTER_SAETTIGUNG: u32 = 60;

/// **Gehoert dieses Zeichen zum Schriftzug?**
///
/// ⚑ **Am Zeichen entschieden und nicht an der Zeile** (2026-09-24). Bis
/// dahin hiess „im Schriftzug" dasselbe wie „in einer Zeile mit `█`".
/// Seit neben dem Schriftzug Muster steht, truegen dessen Zeichen in
/// denselben Zeilen sonst Farbe und Laufrichtung des Logos.
pub(crate) fn ist_logozeichen(c: char) -> bool {
    LOGOZEICHEN.contains(&c)
}

/// Ob ein Zeichen zum Muster gehoert: ein Braille-Zeichen.
fn ist_muster(c: char) -> bool {
    ('\u{2801}'..='\u{28FF}').contains(&c)
}

/// Die Stufe eines Zeichens.
///
/// ⚑ **Am Zeichen und nicht an der Stelle.** Wer das Logo spaeter neu malt
/// ([`crate::schimmer`]), hat nur den Text; deshalb traegt jedes Zeichen
/// seine Stufe selbst.
pub(crate) fn stufe(c: char) -> Stufe {
    if c == '█' {
        Stufe::Leuchtend
    } else if ist_logozeichen(c) {
        Stufe::Schatten
    } else if ist_muster(c) {
        Stufe::Getoent
    } else {
        Stufe::Kante
    }
}

/// Eine Echtfarbe, auf `prozent` ihrer Helle gedaempft. Palettenfarben
/// bleiben, wie sie sind: Deren Helle kennt nur das Terminal.
fn gedaempft(farbe: crossterm::style::Color, prozent: u32) -> crossterm::style::Color {
    match farbe {
        crossterm::style::Color::Rgb { r, g, b } => crossterm::style::Color::Rgb {
            r: (r as u32 * prozent / 100) as u8,
            g: (g as u32 * prozent / 100) as u8,
            b: (b as u32 * prozent / 100) as u8,
        },
        andere => andere,
    }
}

/// Eine Echtfarbe mit weniger Saettigung: Jeder Kanal rueckt um
/// `100 - prozent` Prozent an die Helle der Farbe heran. Palettenfarben
/// bleiben, wie sie sind.
fn entsaettigt(farbe: crossterm::style::Color, prozent: u32) -> crossterm::style::Color {
    match farbe {
        crossterm::style::Color::Rgb { r, g, b } => {
            let (r, g, b) = (r as i32, g as i32, b as i32);
            let grau = (30 * r + 59 * g + 11 * b) / 100;
            let p = prozent as i32;
            let kanal = |c: i32| (grau + (c - grau) * p / 100).clamp(0, 255) as u8;
            crossterm::style::Color::Rgb { r: kanal(r), g: kanal(g), b: kanal(b) }
        }
        andere => andere,
    }
}

/// Farbe und Staerke eines einzelnen Bannerzeichens.
///
/// | Stufe | Zeichen | Darstellung |
/// |---|---|---|
/// | leuchtend | die Bloecke `█` | voller Verlauf, fett |
/// | Schatten | die Kanten `╗ ╔ ╝ ╚ ═ ║` | Verlauf auf 55 Prozent |
/// | getoent | das Muster | 48 Prozent Helle, 60 Prozent Saettigung |
/// | Kante | alles andere | Grau |
///
/// ⚑ **Kontrast ueber drei Mittel** (Wunsch des Projektinhabers,
/// 2026-09-24, am Bild verglichen): Das Muster ist dunkler, und es ist
/// **entsaettigt**, sodass die volle Saettigung allein dem Logo gehoert;
/// das trennt die Ebenen staerker als die Helle allein. Und die Kanten des
/// Schriftzugs, in dieser Schrift ohnehin sein Schatten, treten zurueck,
/// sodass die Bloecke plastisch davor stehen.
///
/// ⚑ **Die Stufen daempfen die Farbe und wechseln sie nicht.** Logo und
/// Muster liegen im selben Regenbogen; was zuruecktritt, wird dunkler und
/// blasser, nicht anders.
pub(crate) fn zeichenstil(
    c: char,
    farbe: crossterm::style::Color,
) -> (crossterm::style::Color, crossterm::style::Attribute) {
    use crossterm::style::Attribute;
    match stufe(c) {
        Stufe::Leuchtend => (farbe, Attribute::Bold),
        Stufe::Schatten => (gedaempft(farbe, SCHATTEN), Attribute::NormalIntensity),
        Stufe::Getoent => (
            gedaempft(entsaettigt(farbe, MUSTER_SAETTIGUNG), MUSTER_HELLE),
            Attribute::NormalIntensity,
        ),
        Stufe::Kante => (crate::farben::KANTE, Attribute::NormalIntensity),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometrie::{Motiv, FREI_X, FREI_Y};

    const BREITEN: [usize; 5] = [ERSATZBREITE, 62, 80, 120, 200];
    const HOEHEN: [u16; 3] = [VOLLE_HOEHE, HALBE_HOEHE, 10];

    /// Die Zeilen, in denen der Schriftzug steht, und seine erste Spalte.
    fn logolage(text: &str, b: usize) -> (Vec<usize>, usize) {
        let zeilen: Vec<usize> = text
            .lines()
            .enumerate()
            .filter(|(_, z)| z.contains('█') || z.contains('╚'))
            .map(|(i, _)| i)
            .collect();
        (zeilen, (b - SCHRIFTBREITE) / 2)
    }

    /// Das Ersatzbild muss in 80 Spalten passen: Es ist das Bild fuer
    /// schmale Fenster und fuer Ausgaben ohne Terminal.
    #[test]
    fn ersatzbild_passt_in_achtzig_spalten() {
        for (i, zeile) in ersatzbild().lines().enumerate() {
            let breite = zeile.chars().count();
            assert!(breite <= 78, "Zeile {} ist {} Zeichen breit", i + 1, breite);
        }
        assert!(SUBTITLE.chars().count() <= 78);
    }

    /// ⛔️ **Nichts beruehrt das Logo**, in keinem Motiv, keiner Breite,
    /// keiner Hoehe: In der Sperrzone um den Schriftzug steht ausser ihm
    /// selbst nur Leerraum.
    #[test]
    fn nichts_beruehrt_das_logo() {
        for m in Motiv::ALLE {
            for b in BREITEN {
                for h in HOEHEN {
                    let text = bild_mit(m, b, h);
                    let zeilen: Vec<Vec<char>> = text.lines().map(|z| z.chars().collect()).collect();
                    let (logo, e) = logolage(&text, b);
                    assert_eq!(logo.len(), 6, "{} {b}×{h}: Schriftzug unvollständig", m.name());
                    let y0 = logo[0].saturating_sub(FREI_Y);
                    let y1 = logo[5] + FREI_Y;
                    let x0 = e.saturating_sub(FREI_X);
                    let x1 = e + SCHRIFTBREITE + FREI_X;
                    for (y, zeile) in zeilen.iter().enumerate().take(y1 + 1).skip(y0) {
                        for x in x0..x1 {
                            let c = zeile.get(x).copied().unwrap_or(' ');
                            let im_logo = logo.contains(&y) && (e..e + SCHRIFTBREITE).contains(&x);
                            assert!(
                                im_logo || c == ' ',
                                "{} {b}×{h}: {c:?} in Zeile {y}, Spalte {x} berührt das Logo",
                                m.name()
                            );
                        }
                    }
                }
            }
        }
    }

    /// **Jedes Motiv umrandet das Logo**: links und rechts neben ihm, und
    /// darueber und darunter, sobald das Fenster hoch genug ist.
    #[test]
    fn jedes_motiv_umrandet_das_logo() {
        for m in Motiv::ALLE {
            for b in [80usize, 120, 200] {
                let text = bild_mit(m, b, VOLLE_HOEHE);
                let zeilen: Vec<Vec<char>> = text.lines().map(|z| z.chars().collect()).collect();
                let (logo, e) = logolage(&text, b);
                let muster = |ys: Vec<usize>, xs: std::ops::Range<usize>| {
                    ys.into_iter().any(|y| {
                        xs.clone().any(|x| zeilen[y].get(x).is_some_and(|c| stufe(*c) == Stufe::Getoent))
                    })
                };
                let links = muster(logo.clone(), 0..e - FREI_X);
                let rechts = muster(logo.clone(), e + SCHRIFTBREITE + FREI_X..b);
                let oben = muster((0..logo[0] - FREI_Y).collect(), 0..b);
                let unten = muster((logo[5] + FREI_Y + 1..zeilen.len()).collect(), 0..b);
                assert!(links && rechts && oben && unten, "{} bei {b}: links {links}, rechts {rechts}, oben {oben}, unten {unten}", m.name());
            }
        }
    }

    /// **Die Zeilenzahl ist die alte**: 18, 13 und 7, in jedem Motiv. Wie
    /// viel Platz das Menue darunter hat, ist nachgerechnet und haengt daran.
    #[test]
    fn das_bild_hat_die_alte_zeilenzahl() {
        for m in Motiv::ALLE {
            for (h, soll) in [(VOLLE_HOEHE, 18), (HALBE_HOEHE, 13), (HALBE_HOEHE - 1, 7)] {
                assert_eq!(bild_mit(m, 120, h).lines().count(), soll, "{} bei Höhe {h}", m.name());
            }
        }
    }

    /// Das Motiv muss die Breite ausfuellen, ohne sie zu ueberschreiten.
    /// Eine Zeile zu breit bricht um und zerreisst das Bild; eine deutlich
    /// zu schmale liesse rechts eine leere Flaeche.
    #[test]
    fn motiv_fuellt_jede_breite_ohne_umbruch() {
        for m in Motiv::ALLE {
            for b in [62usize, 80, 100, 120, 160, 200] {
                let text = bild_mit(m, b, VOLLE_HOEHE);
                let breiteste = text.lines().map(|z| z.chars().count()).max().unwrap_or(0);
                assert!(breiteste <= b, "{} {b}: Zeile mit {breiteste} Zeichen bricht um", m.name());
                assert!(breiteste + 8 >= b, "{} {b}: breiteste Zeile nur {breiteste}", m.name());
            }
        }
    }

    /// Der Schriftzug steht unverzerrt und mittig, in jedem Motiv.
    #[test]
    fn schriftzug_bleibt_zentriert_und_unverzerrt() {
        for m in Motiv::ALLE {
            for b in [80usize, 140, 200] {
                let text = bild_mit(m, b, VOLLE_HOEHE);
                let (logo, e) = logolage(&text, b);
                assert_eq!(e, (b - SCHRIFTBREITE) / 2);
                let zeilen: Vec<&str> = text.lines().collect();
                for (i, y) in logo.iter().enumerate() {
                    let stueck: String = zeilen[*y].chars().skip(e).take(SCHRIFTBREITE).collect();
                    assert_eq!(stueck, SCHRIFTZUG[i], "{} {b}: Zeile {i} verzerrt", m.name());
                }
            }
        }
    }

    /// Passt das Motiv nicht mitsamt Menue ins Fenster, wird es kuerzer; der
    /// Schriftzug ueberlebt jede Stufe.
    #[test]
    fn niedriges_fenster_kuerzt_das_motiv() {
        let voll = fuer_fenster(120, VOLLE_HOEHE).lines().count();
        let halb = fuer_fenster(120, HALBE_HOEHE).lines().count();
        let knapp = fuer_fenster(120, HALBE_HOEHE - 1).lines().count();
        assert!(voll > halb && halb > knapp, "{voll} {halb} {knapp}");
        for h in [VOLLE_HOEHE, HALBE_HOEHE, HALBE_HOEHE - 1, 10, 1] {
            assert_eq!(logolage(&fuer_fenster(120, h), 120).0.len(), 6, "Höhe {h}");
        }
    }

    /// Unterhalb der Mindestbreite kommt die schmalste Form des Motivs.
    #[test]
    fn schmales_fenster_faellt_auf_das_ersatzbild_zurueck() {
        for b in [0u16, 20, 40, (MINDESTBREITE - 1) as u16] {
            assert_eq!(fuer_breite(b), ersatzbild(), "Breite {b}");
        }
        assert_ne!(fuer_breite(MINDESTBREITE as u16), ersatzbild());
    }

    /// Die Breitenangabe muss zum Schriftzug passen.
    #[test]
    fn schriftbreite_stimmt_mit_dem_schriftzug_ueberein() {
        for z in SCHRIFTZUG {
            assert_eq!(z.chars().count(), SCHRIFTBREITE, "Zeile {z:?}");
        }
    }

    /// **Zwei Stufen, und die zweite daempft nur die Farbe.**
    #[test]
    fn das_muster_ist_gedaempft_und_das_logo_voll() {
        use crossterm::style::{Attribute, Color};
        let farbe = Color::Rgb { r: 200, g: 100, b: 250 };
        let helle = |c: Color| match c {
            Color::Rgb { r, g, b } => r as u32 + g as u32 + b as u32,
            andere => panic!("keine Echtfarbe: {andere:?}"),
        };
        let spreizung = |c: Color| match c {
            Color::Rgb { r, g, b } => r.max(g).max(b) as u32 - r.min(g).min(b) as u32,
            andere => panic!("keine Echtfarbe: {andere:?}"),
        };
        assert_eq!(zeichenstil('█', farbe), (farbe, Attribute::Bold));
        for c in LOGOZEICHEN.into_iter().filter(|c| *c != '█') {
            let (schatten, _) = zeichenstil(c, farbe);
            assert!(helle(schatten) < helle(farbe), "die Kante {c:?} tritt nicht zurück");
        }
        let (muster, _) = zeichenstil('⣿', farbe);
        let (schatten, _) = zeichenstil('═', farbe);
        assert!(helle(muster) < helle(schatten), "das Muster ist nicht dunkler als der Schatten");
        // ⚑ Entsaettigt heisst: Die Kanaele ruecken zusammen, staerker als die
        // blosse Daempfung auf dieselbe Helle es taete.
        assert!(
            spreizung(muster) * 100 < spreizung(farbe) * MUSTER_HELLE,
            "das Muster ist nicht entsättigt"
        );
        assert_eq!(zeichenstil('x', farbe), (crate::farben::KANTE, Attribute::NormalIntensity));
        // Palettenfarben kennt nur das Terminal; sie bleiben, wie sie sind.
        let neon = Color::AnsiValue(51);
        assert_eq!(zeichenstil('⣿', neon).0, neon);
    }

    /// ⚑ **Kein Zeichen faellt unbemerkt ins Grau.** Ein Zeichen im Bild,
    /// das keiner Stufe zugeordnet ist, naehme ihm still seine Farbe.
    #[test]
    fn kein_zeichen_faellt_unbemerkt_ins_grau() {
        for m in Motiv::ALLE {
            for b in [ERSATZBREITE, 80, 200] {
                let text = bild_mit(m, b, VOLLE_HOEHE);
                let grau: Vec<char> =
                    text.chars().filter(|c| !c.is_whitespace() && stufe(*c) == Stufe::Kante).collect();
                assert!(grau.is_empty(), "{} {b}: grau stehen {grau:?}", m.name());
            }
        }
    }

    /// Die Kanten muessen sich vom Hintergrund abheben: sichtbar heller
    /// als der Ton, mit dem Hinweiszeilen zurueckgenommen werden.
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
    /// Der Untertitel steht mittig unter dem Schriftzug, nicht links.
    #[test]
    fn untertitel_steht_mittig() {
        if SUBTITLE.trim().is_empty() {
            return;
        }
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
}
