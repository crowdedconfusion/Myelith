//! **Der Mitschnitt: was beim Verdichten verlorenginge, lesbar im
//! Arbeitsordner.**
//!
//! # ⚑ Warum es ihn gibt
//!
//! [`crate::gespraech::verdichten`] ersetzt den Verlauf durch eine
//! Zusammenfassung, und **die Urfassung ist danach weg**. Das ist
//! richtig fuer den Kontext, der eine Grenze hat, und falsch fuer
//! alles, wonach jemand spaeter fragt: die genaue Zahl, der Pfad, die
//! Fehlermeldung von vorhin. **Was zusammengefasst ist, ist nicht
//! verschwunden, es ist nur nicht mehr im Kopf.**
//!
//! # ⚑ Er gehoert dem Ordner und nicht der Sitzung
//!
//! **`.AGENT/` liegt im Projektordner, und was dort steht, ist „was in
//! diesem Projekt geschehen ist"** (Festlegung des Projektinhabers,
//! 2026-09-16). Eine Sitzung ist eine Episode davon; der Ordner ist die
//! Kontinuitaet. Genau deshalb kann ein **neuer** Agent im selben Ordner
//! aufnehmen, woran der vorige gearbeitet hat.
//!
//! ⚑ **Daraus folgt, was das Aufraeumen der Gespraechsliste NICHT tut:**
//! Es fasst den Mitschnitt nicht an. Ein Gespraech aus der Liste zu
//! nehmen ist Aufraeumen; einen Verlauf zu loeschen ist eine eigene
//! Handlung, und die gibt es auch ([`loeschen`]).
//!
//! # ⛔️ Klartext, und warum die Verschluesselung wieder gegangen ist
//!
//! **Sie stand hier am 2026-09-16 einen halben Tag lang und ist am
//! selben Tag entfallen** (Festlegung des Projektinhabers). Der Grund
//! ist nicht Bequemlichkeit, sondern dass sie im Hauptfall versagt:
//!
//! - **Mehrere Personen am selben Ordner.** Jeder Schluessel ist ein
//!   anderer, also liest niemand den Mitschnitt eines anderen. Die
//!   Dateien liegen da und sind nutzlos.
//! - ⛔️ **Und schon eine einzige Person mit zwei Maschinen.** Der
//!   Schluessel lag in der Konfiguration, der Ordner wandert ueber einen
//!   Abgleichdienst mit: Auf dem zweiten Rechner meldet sich „falscher
//!   Schluessel oder veraenderte Datei", also genau das, was man bei
//!   einem Angriff erwartet, und es ist der eigene Laptop.
//!
//! ⚑ **Das ist die Fehlerklasse, die dieses Projekt dauernd benennt:**
//! etwas, das dasteht und aussieht, als funktioniere es, und genau in
//! dem Fall versagt, fuer den es gedacht war.
//!
//! **Die Zusage lautet seither:** Der Mitschnitt ist Klartext und so
//! geschuetzt wie das Dateisystem, auf dem er liegt. Wer an
//! Vertraulichem arbeitet, verschluesselt seine Platte.
//!
//! ⚑ **Eine Sicherung kommt dafuer neu dazu**, und sie kostet nichts:
//! eine `.gitignore` **im** Ordner mit `*`. Ein Arbeitsordner ist sehr
//! oft ein Repositorium, und ein Klartextverlauf in einem Commit ist
//! genau der Unfall, gegen den vorher die Verschluesselung stand. Der
//! Ordner schliesst sich damit selbst aus, ohne die `.gitignore` des
//! Nutzers anzufassen.

use std::path::{Path, PathBuf};

/// Der Ordner im Arbeitsverzeichnis, in dem die Mitschnitte liegen.
///
/// ⚑ **Mit einem Punkt davor** (Auftrag des Projektinhabers): Er gehoert
/// dem Agenten und nicht dem Menschen, der in diesem Ordner arbeitet.
pub const ORDNER: &str = ".AGENT";

/// Wie viele Sitzungen ein Ordner hoechstens behaelt.
///
/// # ⚑ Warum nach Zahl und nicht nach Tagen
///
/// Eine Frist ueberrascht den, der nach sechs Wochen nachschlaegt; eine
/// Zahl trifft nur den, der ohnehin sehr viel angesammelt hat.
///
/// ⚠️ **Zwanzig ist geschaetzt und nicht gemessen** (Festlegung des
/// Projektinhabers, 2026-09-16). Zum Wiederaufnehmen zaehlen die letzten
/// ein bis drei, zum Vertrautmachen eine Handvoll; darueber hinaus ist
/// es Archaeologie, und jede weitere Zeile verlaengert nur die Liste,
/// die [`uebersicht`] in den Kontext legt.
///
/// ⚑ **Gezaehlt werden Sitzungen und nicht Verdichtungen**: Eine halbe
/// Sitzung ist ein Verlauf mit einem Loch, und ein Loch sieht man ihm
/// nicht an.
/// Woran ein Mitschnitt zu erkennen ist.
///
/// ⚑ **Daran haengt, was als Sitzung zaehlt**, und damit auch, was die
/// Obergrenze aufraeumen darf. Nebenordner unter `.AGENT` (etwa die
/// Wissensmappen) tragen keine solche Datei und sind deshalb keine
/// Sitzung.
const DATEIANFANG: &str = "verlauf-";

pub const HOECHSTENS_SITZUNGEN: usize = 20;

/// Ein Abschnitt des Verlaufs, wie er im Verzeichnis steht.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Abschnitt {
    /// Fortlaufend ab eins.
    pub nr: usize,
    /// Wer gesprochen hat.
    pub rolle: String,
    /// Die erste Zeile dieses Abschnitts **in der Datei**, ab eins.
    ///
    /// ⚑ **In der Datei und nicht im Verlaufsteil** (seit dem Klartext,
    /// 2026-09-16). Damit stimmen die Nummern mit dem ueberein, was ein
    /// Mensch in seinem Texteditor sieht und was `grep -n` meldet.
    pub von: usize,
    /// Die letzte Zeile, einschliesslich.
    pub bis: usize,
    /// Die erste Zeile des Abschnitts, gekuerzt: **wonach jemand
    /// sucht**, wenn er das Verzeichnis liest.
    pub kopf: String,
}

/// Der Kopf eines Mitschnitts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Verzeichnis {
    /// Wann er geschrieben wurde, nach ISO 8601 in UTC.
    pub datum: String,
    /// Das Modell, mit dem gesprochen wurde.
    pub modell: String,
    /// Die Sitzung, zu der die Episode gehoert.
    pub sitzung: String,
    pub nachrichten: usize,
    pub zeilen: usize,
    pub abschnitte: Vec<Abschnitt>,
}

/// Was schiefgehen kann.
#[derive(Debug)]
pub enum Fehler {
    Datei(String),
    Form(String),
}

impl std::fmt::Display for Fehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Datei(m) => write!(f, "Verlauf: {m}"),
            Self::Form(m) => write!(f, "Verlauf, Form: {m}"),
        }
    }
}

/// Eine Kennung, die sich als Ordnername eignet.
///
/// ⚑ **Hergeleitet und nicht uebernommen.** Eine Gespraechskennung kommt
/// aus einer Oberflaeche, und was dort erlaubt ist, ist es im
/// Dateisystem nicht: Ein `/` darin legte den Ordner woanders an.
pub fn kennung_aus(roh: &str) -> String {
    let k: String = roh
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if k.is_empty() {
        "ohne-kennung".to_string()
    } else {
        k
    }
}

/// **Schreibt eine Episode.**
///
/// `abschnitte` ist der Verlauf, ein Eintrag je Nachricht, als
/// `(Rolle, Text)`. Gibt den Pfad zurueck, relativ zum Arbeitsordner.
pub fn schreiben(
    wurzel: &Path,
    sitzung: &str,
    modell: &str,
    abschnitte: &[(String, String)],
) -> Result<String, Fehler> {
    let sitzung = kennung_aus(sitzung);
    let ordner = wurzel.join(ORDNER);
    let sitzungsordner = ordner.join(&sitzung);
    std::fs::create_dir_all(&sitzungsordner)
        .map_err(|e| Fehler::Datei(format!("{}: {e}", sitzungsordner.display())))?;
    gitignore_fuer_agentenordner(&ordner)?;
    // ⚑ **Der Skill-Ordner entsteht mit dem Mitschnitt** (Auftrag des
    // Projektinhabers, 2026-09-17), und der Zeitpunkt ist der richtige:
    // **Nach einer Verdichtung ist das Wissen aus dem Kontext
    // verschwunden**, und dann muss es einen Ort geben, an dem es noch
    // steht. ⚠️ Scheitert das, scheitert der Mitschnitt nicht: Eine
    // fehlende Mappe ist eine fehlende Bequemlichkeit, ein fehlender
    // Mitschnitt ist ein Verlust.
    let _ = crate::skills::anlegen(wurzel);

    let jetzt = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = format!("{DATEIANFANG}{jetzt}.md");
    let pfad = sitzungsordner.join(&name);

    // ⚑ **Der Versatz wird gerechnet und danach geprueft.** Die Nummern
    // im Verzeichnis zeigen auf Zeilen **der Datei**, und die Datei
    // beginnt mit dem Kopf und dem Verzeichnis selbst. Beide sind in
    // ihrer Laenge bekannt, bevor sie geschrieben sind: fester Kopf plus
    // eine Zeile je Abschnitt. **Gerechnet heisst hier trotzdem nicht
    // geglaubt**, siehe die Zusicherung weiter unten.
    const KOPFZEILEN: usize = 7;
    let versatz = KOPFZEILEN + abschnitte.len() + 2;

    let mut zeile = versatz + 1;
    let mut eintraege = Vec::with_capacity(abschnitte.len());
    let mut koerper = String::new();
    for (i, (rolle, text)) in abschnitte.iter().enumerate() {
        let block = format!("### {rolle}\n\n{text}\n");
        let n = block.lines().count();
        eintraege.push(Abschnitt {
            nr: i + 1,
            rolle: rolle.clone(),
            von: zeile,
            bis: zeile + n - 1,
            kopf: kopfzeile(text),
        });
        zeile += n;
        koerper.push_str(&block);
    }

    let mut aus = String::new();
    aus.push_str(&format!("# Mitschnitt {}\n", zeitmarke(jetzt)));
    aus.push_str(&format!("Sitzung: {sitzung}\n"));
    aus.push_str(&format!("Modell: {modell}\n"));
    aus.push_str(&format!(
        "Nachrichten: {}, Zeilen: {}\n",
        abschnitte.len(),
        zeile.saturating_sub(1)
    ));
    aus.push('\n');
    aus.push_str("## Verzeichnis\n");
    aus.push('\n');
    for a in &eintraege {
        aus.push_str(&format!("{}-{} {} {}\n", a.von, a.bis, a.rolle, a.kopf));
    }
    aus.push('\n');
    aus.push_str("## Verlauf\n");
    aus.push_str(&koerper);

    // ⛔️ **Die Zusicherung, und sie ist kein Zierrat.** Ein Verzeichnis,
    // das um eine Zeile danebenliegt, schickt jeden Leser an die falsche
    // Stelle, und niemand sieht es dem Mitschnitt an. Hier ist es noch
    // zu pruefen, spaeter nicht mehr.
    let zeilen: Vec<&str> = aus.lines().collect();
    for a in &eintraege {
        let z = zeilen.get(a.von - 1).copied().unwrap_or("");
        if z != format!("### {}", a.rolle) {
            return Err(Fehler::Form(format!(
                "das Verzeichnis zeigt fuer Abschnitt {} auf Zeile {}, und dort steht {z:?} \
                 statt der Ueberschrift. Der gerechnete Versatz stimmt nicht.",
                a.nr, a.von
            )));
        }
    }

    std::fs::write(&pfad, aus).map_err(|e| Fehler::Datei(format!("{}: {e}", pfad.display())))?;
    beschneiden(&ordner, HOECHSTENS_SITZUNGEN);
    Ok(format!("{ORDNER}/{sitzung}/{name}"))
}

/// Legt die `.gitignore` an, die den Ordner sich selbst ausschliessen
/// laesst.
///
/// ⚑ **Im Ordner und nicht in der des Nutzers.** Eine fremde Datei zu
/// aendern, weil man selbst etwas ablegt, ist ein Uebergriff; ein `*`
/// in der eigenen ist eine Aussage ueber den eigenen Ordner.
pub fn gitignore_fuer_agentenordner(ordner: &Path) -> Result<(), Fehler> {
    let p = ordner.join(".gitignore");
    if p.exists() {
        return Ok(());
    }
    std::fs::write(
        &p,
        "# Mitschnitte des Agenten: Klartext, und sie gehoeren niemandem ausser\n\
         # diesem Arbeitsordner. Ein Verlauf in einem Commit ist ein Unfall.\n\
         *\n",
    )
    .map_err(|e| Fehler::Datei(format!("{}: {e}", p.display())))
}

/// Wirft die aeltesten Sitzungen weg, bis hoechstens `grenze` bleiben.
///
/// ⚠️ **Ein Fehlschlag beim Aufraeumen haelt nichts auf.** Der
/// Mitschnitt ist geschrieben, und das ist das Wichtigere; ein Ordner
/// zu viel ist ein Schoenheitsfehler.
/// Ist dieser Ordner eine Sitzung?
///
/// ⚑ **Eine Sitzung ist ein Ordner mit einem Mitschnitt darin**, und
/// nicht jeder Ordner unter `.AGENT`. Die Regel haengt am Inhalt und
/// nicht an einer Liste verbotener Namen: Wer morgen einen zweiten
/// Nebenordner anlegt, ist automatisch geschuetzt.
fn ist_sitzung(p: &Path) -> bool {
    p.is_dir()
        && std::fs::read_dir(p)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with(DATEIANFANG))
}

fn beschneiden(ordner: &Path, grenze: usize) {
    // ⛔️ **Dieselbe Regel wie in `sitzungen`, und zwar buchstaeblich
    // dieselbe Funktion.**
    //
    // 📌 Hier stand eine zweite Fassung („jeder Ordner"), und sie lief
    // in dem Augenblick auseinander, in dem `.AGENT/skills/` dazukam:
    // Das Aufraeumen zaehlte die Mappen als Sitzung mit, die Obergrenze
    // von zwanzig wurde faktisch zu neunzehn, und je nach Aenderungszeit
    // haette es die Mappen **geloescht**. **Zwei Fassungen derselben
    // Regel, und die zweite meldet sich nicht.**
    let mut alle: Vec<PathBuf> = std::fs::read_dir(ordner)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| ist_sitzung(p))
        .collect();
    if alle.len() <= grenze {
        return;
    }
    // ⚑ **Sortiert nach der juengsten Episode darin**, nicht nach dem
    // Ordnernamen: Eine Sitzung, die gestern wieder aufgenommen wurde,
    // ist nicht alt, auch wenn sie vor einem Monat begonnen hat.
    alle.sort_by_key(|p| juengste_episode(p));
    let weg = alle.len() - grenze;
    for p in alle.into_iter().take(weg) {
        let _ = std::fs::remove_dir_all(p);
    }
}

fn juengste_episode(sitzung: &Path) -> u64 {
    std::fs::read_dir(sitzung)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_nanos())
            })
        })
        .max()
        .unwrap_or(0) as u64
}

/// Die erste nichtleere Zeile, gekuerzt.
fn kopfzeile(text: &str) -> String {
    let z = text.lines().find(|z| !z.trim().is_empty()).unwrap_or("").trim();
    if z.chars().count() <= 70 {
        return z.to_string();
    }
    z.chars().take(69).chain(['…']).collect()
}

/// Ein Text, auf `breite` Zeichen gekuerzt.
///
/// ⚑ **Zeichen und nicht Bytes**, sonst schneidet die Kuerzung einen
/// Umlaut in der Mitte durch und die Antwort traegt ein kaputtes Zeichen.
fn kurz(text: &str, breite: usize) -> String {
    let t = text.trim();
    if t.chars().count() <= breite {
        return t.to_string();
    }
    t.chars().take(breite.saturating_sub(1)).chain(['…']).collect()
}

/// Sekunden seit der Epoche als lesbares Datum.
///
/// ⚑ **Von Hand gerechnet und ohne eine Kiste dafuer.** Ein
/// Zeitzonenpaket fuer eine Zeile im Kopf waere eine Abhaengigkeit fuer
/// eine Anzeige.
fn zeitmarke(sekunden: u64) -> String {
    let (mut tage, rest) = (sekunden / 86_400, sekunden % 86_400);
    let (std, min, sek) = (rest / 3600, (rest % 3600) / 60, rest % 60);
    let mut jahr = 1970u64;
    loop {
        let laenge = if schaltjahr(jahr) { 366 } else { 365 };
        if tage < laenge {
            break;
        }
        tage -= laenge;
        jahr += 1;
    }
    let monate =
        [31, if schaltjahr(jahr) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut monat = 0usize;
    while tage >= monate[monat] {
        tage -= monate[monat];
        monat += 1;
    }
    format!("{jahr:04}-{:02}-{:02}T{std:02}:{min:02}:{sek:02}Z", monat + 1, tage + 1)
}

fn schaltjahr(j: u64) -> bool {
    (j % 4 == 0 && j % 100 != 0) || j % 400 == 0
}

/// Alle Episoden eines Arbeitsordners, **neueste zuerst**.
pub fn vorhandene(wurzel: &Path) -> Vec<PathBuf> {
    let mut aus: Vec<PathBuf> = sitzungen(wurzel)
        .into_iter()
        .flat_map(|s| {
            std::fs::read_dir(s)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "md"))
                .collect::<Vec<_>>()
        })
        .collect();
    // ⚑ Der Dateiname traegt die Sekunden, also sortiert er zeitlich.
    aus.sort_by_key(|p| std::cmp::Reverse(p.file_name().map(|n| n.to_os_string())));
    aus
}

/// **Der Kopf einer Episode**, gelesen statt gerechnet.
pub fn verzeichnis(pfad: &Path) -> Result<Verzeichnis, Fehler> {
    let text = std::fs::read_to_string(pfad)
        .map_err(|e| Fehler::Datei(format!("{}: {e}", pfad.display())))?;
    let zeilen: Vec<&str> = text.lines().collect();
    let feld = |marke: &str| -> String {
        zeilen
            .iter()
            .find_map(|z| z.strip_prefix(marke))
            .map(|r| r.trim().to_string())
            .unwrap_or_default()
    };
    let datum = zeilen
        .first()
        .and_then(|z| z.strip_prefix("# Mitschnitt "))
        .map(|s| s.trim().to_string())
        .ok_or_else(|| Fehler::Form(format!("{} ist kein Mitschnitt", pfad.display())))?;

    // ⚑ **Das Verzeichnis wird gelesen und nicht aus dem Verlauf neu
    // gerechnet.** Zwei Rechnungen derselben Zeilennummern liefen
    // auseinander, und die zweite waere die, auf die sich jemand
    // verlaesst.
    let mut abschnitte = Vec::new();
    let mut im_verzeichnis = false;
    for z in &zeilen {
        if z.starts_with("## Verzeichnis") {
            im_verzeichnis = true;
            continue;
        }
        if z.starts_with("## Verlauf") {
            break;
        }
        if !im_verzeichnis || z.trim().is_empty() {
            continue;
        }
        let Some((bereich, rest)) = z.split_once(' ') else { continue };
        let Some((von, bis)) = bereich.split_once('-') else { continue };
        let (Ok(von), Ok(bis)) = (von.parse::<usize>(), bis.parse::<usize>()) else { continue };
        let (rolle, kopf) = rest.split_once(' ').unwrap_or((rest, ""));
        abschnitte.push(Abschnitt {
            nr: abschnitte.len() + 1,
            rolle: rolle.to_string(),
            von,
            bis,
            kopf: kopf.to_string(),
        });
    }

    Ok(Verzeichnis {
        datum,
        modell: feld("Modell:"),
        sitzung: feld("Sitzung:"),
        nachrichten: abschnitte.len(),
        zeilen: abschnitte.last().map(|a| a.bis).unwrap_or(0),
        abschnitte,
    })
}

/// **Die Zeilen `von` bis `bis` der Datei**, einschliesslich, ab eins.
pub fn zeilen(pfad: &Path, von: usize, bis: usize) -> Result<String, Fehler> {
    if von == 0 || bis < von {
        return Err(Fehler::Form(format!(
            "Zeilenbereich {von} bis {bis} ergibt keinen Sinn; gezaehlt wird ab eins"
        )));
    }
    let text = std::fs::read_to_string(pfad)
        .map_err(|e| Fehler::Datei(format!("{}: {e}", pfad.display())))?;
    let mut aus = String::new();
    for (i, z) in text.lines().enumerate() {
        let nr = i + 1;
        if nr >= von && nr <= bis {
            aus.push_str(z);
            aus.push('\n');
        }
    }
    Ok(aus)
}

/// **Loescht eine Sitzung oder alle**, ausdruecklich.
///
/// ⛔️ **Das ist die Handlung, die es vorher gar nicht gab**, und sie ist
/// die wichtige: „Da stand etwas Vertrauliches drin, weg damit." Das
/// Aufraeumen der Gespraechsliste tut es ausdruecklich **nicht**.
///
/// `sitzung` `None` loescht den ganzen Ordner. Gibt zurueck, wie viele
/// Sitzungen entfernt wurden.
pub fn loeschen(wurzel: &Path, sitzung: Option<&str>) -> Result<usize, Fehler> {
    let ordner = wurzel.join(ORDNER);
    match sitzung {
        Some(s) => {
            let p = ordner.join(kennung_aus(s));
            if !p.is_dir() {
                return Ok(0);
            }
            std::fs::remove_dir_all(&p)
                .map_err(|e| Fehler::Datei(format!("{}: {e}", p.display())))?;
            Ok(1)
        }
        None => {
            let n = sitzungen(wurzel).len();
            if ordner.is_dir() {
                std::fs::remove_dir_all(&ordner)
                    .map_err(|e| Fehler::Datei(format!("{}: {e}", ordner.display())))?;
            }
            Ok(n)
        }
    }
}

/// Die Sitzungen eines Arbeitsordners, neueste zuerst.
pub fn sitzungen(wurzel: &Path) -> Vec<PathBuf> {
    let mut aus: Vec<PathBuf> = std::fs::read_dir(wurzel.join(ORDNER))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| ist_sitzung(p))
        .collect();
    aus.sort_by_key(|p| std::cmp::Reverse(juengste_episode(p)));
    aus
}

/// Wie breit der Textausschnitt eines Treffers hoechstens ist.
///
/// # ⛔️ Zweimal gemessen, zweimal zu wenig
///
/// **Erst gab die Suche nur Zeilennummern**, und das 4B antwortete in
/// neun von neun Laeufen „laeuft unter der Kennung **612-614**": Es
/// hielt den Zeigefinger fuer die Auskunft. **Dann gab sie die
/// gefundene Zeile**, und das war die **Frage**, in der das Suchwort
/// steht; die Auskunft steht in der **Antwort** darunter, und das
/// Modell holte sie nicht (0 von 3).
///
/// ⚑ **Also liefert sie den Wechsel.** Das ist, was ein Mensch meint,
/// wenn er sagt „such das mal in der Historie": nicht eine Landkarte
/// und kein Zeigefinger, sondern die Stelle. Vierhundert Zeichen je
/// Treffer, hoechstens zwanzig Treffer, also im schlimmsten Fall rund
/// 2 000 Token und im Regelfall ein paar Dutzend.
const TREFFERBREITE: usize = 400;

/// **Ein Treffer der Suche im Mitschnitt.**
#[derive(Debug, Clone)]
pub struct Treffer {
    /// Die Datei, in der er steht, relativ zur Wurzel.
    pub datei: String,
    /// Erste Zeile des **Wechsels**, in der Datei.
    ///
    /// ⚑ **Nicht des Abschnitts, sondern des Wechsels aus Frage und
    /// Antwort** (gemessen 2026-09-17): Das gesuchte Wort steht fast
    /// immer in der **Frage**, und die Auskunft in der **Antwort**
    /// darunter. Ein Treffer, der nur die Frage umfasst, schickt den
    /// Leser genau eine Nachricht zu weit nach oben.
    pub von: usize,
    /// Letzte Zeile, einschliesslich.
    pub bis: usize,
    /// Wer gesprochen hat.
    pub rolle: String,
    /// Die erste Zeile des Abschnitts, gekuerzt.
    pub kopf: String,
    /// Die Zeile, in der das Wort steht.
    pub zeile: usize,
    /// Der Wechsel um diese Zeile herum, gekuerzt.
    ///
    /// ⛔️ **Ohne ihn missversteht ein Modell den Treffer.** Gemessen:
    /// Ein Treffer, der nur `612-614 user Und was macht der Pruefstand
    /// in Halle 3?` meldete, brachte das 4B in neun von neun Laeufen zu
    /// der Antwort „der Pruefstand laeuft unter der Kennung
    /// **612-614**". **Eine Suche, die nur Zeigefinger zurueckgibt,
    /// wird fuer die Auskunft gehalten.**
    pub text: String,
}

/// **Sucht eine Zeichenfolge in allen Mitschnitten dieses Ordners**,
/// juengste Sitzung zuerst, und nennt die Abschnitte, in denen sie
/// steht.
///
/// # ⛔️ Warum es das gibt, und warum das Verzeichnis nicht genuegt
///
/// Gemessen am 2026-09-17 (`nadelprobe`): Ein Verlauf aus 240
/// Abschnitten, eine Einzelheit darin, die im verdichteten Kontext
/// fehlt. **Mit Verzeichnis findet das 4B sie in sechs von neun
/// Laeufen**, und zwar so: `list_history` holt das ganze Verzeichnis
/// (rund 6 000 Token), dann liest `read_history` die Zeilen. ⛔️ **Der
/// Kontext am Ende war damit 8 890 Token, also so gross wie der ganze
/// Verlauf, den die Verdichtung gerade weggeraeumt hatte.**
///
/// ⚑ **Das Verzeichnis beantwortet die falsche Frage.** Gefragt ist
/// nicht „wie ist der Verlauf gegliedert", sondern „wo steht dieses
/// Wort". Eine Suche antwortet darauf mit ein paar Zeilen statt mit
/// einer Landkarte.
///
/// ⚠️ **Nur Zeichenfolgen, keine Muster.** Ein regulaerer Ausdruck aus
/// einem Modell ist eine Rechenzeitzusage, die niemand geprueft hat;
/// eine Zeichenfolge ist vorhersagbar, und Gross- und Kleinschreibung
/// wird ignoriert.
pub fn suchen(wurzel: &Path, muster: &str, hoechstens: usize) -> Vec<Treffer> {
    let muster = muster.to_lowercase();
    if muster.is_empty() {
        return Vec::new();
    }
    let mut aus = Vec::new();
    for pfad in vorhandene(wurzel) {
        let Ok(text) = std::fs::read_to_string(&pfad) else { continue };
        let Ok(v) = verzeichnis(&pfad) else { continue };
        let zeilen: Vec<&str> = text.lines().collect();
        for (i, a) in v.abschnitte.iter().enumerate() {
            if aus.len() >= hoechstens {
                return aus;
            }
            // ⚑ **Gesucht wird im Abschnitt und gemeldet wird der
            // Abschnitt.** Eine Zeilennummer allein waere ein Fund ohne
            // Zusammenhang: Wer die Antwort sucht, braucht die Frage
            // davor.
            let von = a.von.saturating_sub(1);
            let bis = a.bis.min(zeilen.len());
            if von >= bis {
                continue;
            }
            let Some(k) = zeilen[von..bis].iter().position(|z| z.to_lowercase().contains(&muster))
            else {
                continue;
            };
            // ⚑ **Der Treffer umfasst den Wechsel.** Steht das Wort in
            // einer Frage, gehoert die Antwort darunter dazu; steht es
            // in einer Antwort, die Frage darueber. Wer sucht, will
            // beides lesen.
            let (mut t_von, mut t_bis) = (a.von, a.bis);
            if a.rolle == "user" {
                if let Some(n) = v.abschnitte.get(i + 1) {
                    t_bis = n.bis;
                }
            } else if i > 0 {
                if let Some(vorher) = v.abschnitte.get(i - 1) {
                    t_von = vorher.von;
                }
            }
            let treffzeile = a.von + k;
            aus.push(Treffer {
                datei: pfad.strip_prefix(wurzel).unwrap_or(&pfad).display().to_string(),
                von: t_von,
                bis: t_bis,
                rolle: a.rolle.clone(),
                kopf: a.kopf.clone(),
                zeile: treffzeile,
                // ⚑ **Der Wechsel und nicht die Zeile**, ohne die
                // Ueberschriften und ohne Leerzeilen: Was hier steht,
                // ist die Stelle, und die Ueberschriften stehen schon
                // als Rolle daneben.
                text: kurz(
                    &zeilen[t_von.saturating_sub(1)..t_bis.min(zeilen.len())]
                        .iter()
                        .filter(|z| !z.trim().is_empty() && !z.starts_with("### "))
                        .copied()
                        .collect::<Vec<&str>>()
                        .join(" / "),
                    TREFFERBREITE,
                ),
            });
        }
    }
    aus
}

/// **Ein grobes Verzeichnis, dessen Laenge nicht mitwaechst.**
///
/// # ⛔️ Fund 387: die Verdichtung machte den Kontext groesser
///
/// Bis zum 2026-09-16 kam **eine Zeile je Nachricht** in die
/// Zusammenfassung. Bei einem kurzen Gespraech ist das die genaueste
/// Landkarte, die es gibt. **Bei dem Gespraech, das eine Verdichtung
/// ueberhaupt ausloest, ist es eine Katastrophe:** Gemessen an einem
/// Verlauf aus 120 Nachrichten (4 476 Token) wog das Verzeichnis
/// **3 597 Token**, und die „verdichtete" Fassung war mit 3 970 bis
/// 5 659 Token **teils groesser als das Original**.
///
/// ⚑ **Der Fehler war nicht die Zahl, sondern die Groessenordnung.**
/// Ein Verzeichnis, das je Nachricht eine Zeile hat, waechst genauso
/// schnell wie das, was es ersetzen soll; es kann deshalb nie sparen.
/// **Was in den Kontext geht, braucht eine Schranke, die nicht vom
/// Gespraech abhaengt.**
///
/// Also: hoechstens `hoechstens` Zeilen. Sind es weniger Abschnitte,
/// bekommt jeder seine eigene Zeile (nichts geht verloren, und kurze
/// Gespraeche behalten die genaue Karte). Sind es mehr, werden
/// benachbarte Abschnitte zu Bloecken zusammengefasst, und jeder Block
/// nennt seine Zeilenspanne, seine Zahl und die Frage, mit der er
/// beginnt. ⚑ **Die Frage und nicht die Antwort:** Wer etwas sucht,
/// erinnert sich an das, wonach er gefragt hat.
pub fn grobverzeichnis(pfad: &Path, hoechstens: usize) -> Result<Vec<String>, Fehler> {
    let v = verzeichnis(pfad)?;
    let n = v.abschnitte.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    if n <= hoechstens {
        return Ok(v
            .abschnitte
            .iter()
            .map(|a| format!("  {}-{} {} {}", a.von, a.bis, a.rolle, a.kopf))
            .collect());
    }
    let je = n.div_ceil(hoechstens);
    Ok(v
        .abschnitte
        .chunks(je)
        .map(|block| {
            let von = block.first().map(|a| a.von).unwrap_or(0);
            let bis = block.last().map(|a| a.bis).unwrap_or(0);
            let kopf = block
                .iter()
                .find(|a| a.rolle == "user")
                .or_else(|| block.first())
                .map(|a| a.kopf.as_str())
                .unwrap_or("");
            format!("  {von}-{bis} ({} Abschnitte) ab: {kopf}", block.len())
        })
        .collect())
}

/// **Was in diesem Ordner liegt.**
///
/// ⚑ **Es nennt und liefert nicht.** Ein Mitschnitt aus einer fremden
/// Sitzung ist der Verlauf eines anderen Gespraechs; ihn ungefragt in
/// den Kontext zu ziehen, waere eine Ueberraschung. Wer ihn will, liest
/// danach seine Zeilen.
pub struct Uebersicht {
    pub episoden: usize,
    pub sitzungen: usize,
    pub bytes: u64,
    /// Der Kopf der juengsten Episode, falls es eine gibt.
    pub juengste: Option<(PathBuf, Verzeichnis)>,
}

pub fn uebersicht(wurzel: &Path) -> Uebersicht {
    let episoden = vorhandene(wurzel);
    let bytes =
        episoden.iter().filter_map(|p| std::fs::metadata(p).ok().map(|m| m.len())).sum();
    let juengste = episoden.first().and_then(|p| verzeichnis(p).ok().map(|v| (p.clone(), v)));
    Uebersicht { episoden: episoden.len(), sitzungen: sitzungen(wurzel).len(), bytes, juengste }
}

#[cfg(test)]
mod proben {
    use super::*;

    fn ordner(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("myl-verlauf-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("Ordner");
        d
    }

    fn beispiel() -> Vec<(String, String)> {
        vec![
            ("user".into(), "Wie viele Kerne hat die Maschine?\nBitte genau.".into()),
            ("assistant".into(), "Fuenfzehn Kerne, davon fuenf schnelle.".into()),
            ("user".into(), "Und der Arbeitsspeicher?".into()),
            ("assistant".into(), "Vierundzwanzig Gibibyte.".into()),
        ]
    }

    /// ⛔️ **Die Zeilennummern zeigen auf Zeilen der Datei.**
    ///
    /// Das ist die tragende Eigenschaft: Ein Mensch oeffnet die Datei im
    /// Editor und springt zu der Nummer, `grep -n` meldet dieselbe, und
    /// `read_history` gibt dieselbe heraus. **Ein Verzeichnis, das um
    /// eine Zeile danebenliegt, schickt jeden Leser an die falsche
    /// Stelle, und niemand sieht es ihm an.**
    #[test]
    fn die_zeilennummern_zeigen_auf_zeilen_der_datei() {
        let d = ordner("zeilen");
        let rel = schreiben(&d, "sitzung-1", "myelith-0.6b", &beispiel()).expect("schreiben");
        let pfad = d.join(&rel);
        let text = std::fs::read_to_string(&pfad).expect("lesen");
        let zeilen: Vec<&str> = text.lines().collect();

        let v = verzeichnis(&pfad).expect("Verzeichnis");
        assert_eq!(v.abschnitte.len(), 4);
        for a in &v.abschnitte {
            assert_eq!(
                zeilen[a.von - 1],
                format!("### {}", a.rolle),
                "Abschnitt {} zeigt auf die falsche Zeile",
                a.nr
            );
        }
        let antwort = &v.abschnitte[1];
        let t = super::zeilen(&pfad, antwort.von, antwort.bis).expect("Zeilen");
        assert!(t.contains("Fuenfzehn Kerne"), "{t}");
        assert!(!t.contains("Gibibyte"), "es kam mehr heraus als gefragt: {t}");
    }

    /// **Der Kopf traegt Sitzung und Modell, und das Verzeichnis die
    /// Marken.**
    #[test]
    fn der_kopf_traegt_sitzung_und_modell() {
        let d = ordner("kopf");
        let rel =
            schreiben(&d, "Sitzung/mit Zeichen!", "myelith-4b", &beispiel()).expect("schreiben");
        // ⚑ Die Kennung wird hergeleitet: ein `/` legte den Ordner sonst
        // woanders an.
        assert!(rel.contains("Sitzung-mit-Zeichen"), "{rel}");
        let v = verzeichnis(&d.join(&rel)).expect("Verzeichnis");
        assert_eq!(v.modell, "myelith-4b");
        assert_eq!(v.sitzung, "Sitzung-mit-Zeichen");
        assert_eq!(v.abschnitte[0].kopf, "Wie viele Kerne hat die Maschine?");
    }

    /// ⛔️ **Der Ordner schliesst sich selbst aus.**
    ///
    /// Ein Arbeitsordner ist sehr oft ein Repositorium, und seit der
    /// Mitschnitt Klartext ist, waere er in einem Commit genau der
    /// Unfall, gegen den vorher die Verschluesselung stand.
    #[test]
    fn der_ordner_schliesst_sich_selbst_aus() {
        let d = ordner("gitignore");
        schreiben(&d, "s", "m", &beispiel()).expect("schreiben");
        let ignoriert =
            std::fs::read_to_string(d.join(ORDNER).join(".gitignore")).expect("gitignore");
        assert!(ignoriert.lines().any(|z| z.trim() == "*"), "{ignoriert}");
    }

    /// ⛔️ **Das Aufraeumen fasst die Wissensmappen nicht an.**
    ///
    /// Sie liegen als Nebenordner unter `.AGENT`, und eine frueher
    /// Fassung des Aufraeumens zaehlte sie als Sitzung mit: Die
    /// Obergrenze von zwanzig wurde damit faktisch zu neunzehn, und je
    /// nach Aenderungszeit waeren die Mappen geloescht worden.
    #[test]
    fn das_aufraeumen_laesst_die_nebenordner_stehen() {
        let d = ordner("nebenordner");
        for i in 0..(HOECHSTENS_SITZUNGEN + 3) {
            schreiben(&d, &format!("s{i:03}"), "m", &beispiel()).expect("schreiben");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let mappen = d.join(ORDNER).join("skills");
        assert!(mappen.is_dir(), "der Skill-Ordner ist verschwunden");
        assert_eq!(
            sitzungen(&d).len(),
            HOECHSTENS_SITZUNGEN,
            "der Nebenordner zaehlt als Sitzung mit"
        );
    }

    /// ⚑ **Die Obergrenze wirft die aeltesten Sitzungen weg, und keine
    /// halben.**
    #[test]
    fn die_obergrenze_wirft_ganze_sitzungen_weg() {
        let d = ordner("grenze");
        for i in 0..(HOECHSTENS_SITZUNGEN + 3) {
            schreiben(&d, &format!("s{i:03}"), "m", &beispiel()).expect("schreiben");
            // Die Sortierung haengt an der Aenderungszeit; ohne Abstand
            // waeren alle gleich alt.
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let uebrig = sitzungen(&d);
        assert_eq!(uebrig.len(), HOECHSTENS_SITZUNGEN, "die Grenze greift nicht");
        for s in &uebrig {
            assert!(
                std::fs::read_dir(s).into_iter().flatten().count() > 0,
                "eine Sitzung ist leer zurueckgeblieben: {}",
                s.display()
            );
        }
        let namen: Vec<String> = uebrig
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect();
        assert!(!namen.contains(&"s000".to_string()), "die aelteste blieb: {namen:?}");
        assert!(
            namen.contains(&format!("s{:03}", HOECHSTENS_SITZUNGEN + 2)),
            "die juengste ging: {namen:?}"
        );
    }

    /// ⛔️ **Loeschen ist eine eigene Handlung, und sie loescht wirklich.**
    #[test]
    fn loeschen_loescht_und_zwar_nur_das_genannte() {
        let d = ordner("loeschen");
        schreiben(&d, "eins", "m", &beispiel()).expect("a");
        std::thread::sleep(std::time::Duration::from_millis(5));
        schreiben(&d, "zwei", "m", &beispiel()).expect("b");
        assert_eq!(sitzungen(&d).len(), 2);

        assert_eq!(loeschen(&d, Some("eins")).expect("loeschen"), 1);
        let uebrig = sitzungen(&d);
        assert_eq!(uebrig.len(), 1, "es wurde mehr geloescht als genannt");
        assert!(uebrig[0].ends_with("zwei"), "die falsche Sitzung ging");

        // Eine, die es nicht gibt, ist kein Fehler, sondern null.
        assert_eq!(loeschen(&d, Some("gibtsnicht")).expect("keine"), 0);

        assert_eq!(loeschen(&d, None).expect("alles"), 1);
        assert!(vorhandene(&d).is_empty(), "nach dem Loeschen liegt noch etwas da");
    }

    /// **Die Uebersicht nennt, was da ist, und liefert es nicht.**
    #[test]
    fn die_uebersicht_nennt_und_liefert_nicht() {
        let d = ordner("uebersicht");
        let leer = uebersicht(&d);
        assert_eq!((leer.episoden, leer.sitzungen), (0, 0));
        assert!(leer.juengste.is_none());

        schreiben(&d, "eins", "m", &beispiel()).expect("a");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        schreiben(&d, "zwei", "myelith-30b-a3b", &beispiel()).expect("b");

        let u = uebersicht(&d);
        assert_eq!((u.episoden, u.sitzungen), (2, 2));
        assert!(u.bytes > 0, "null Bytes bei zwei Episoden");
        let (_pfad, v) = u.juengste.expect("juengste");
        assert_eq!(v.sitzung, "zwei", "die juengste ist nicht die zuletzt geschriebene");
        assert_eq!(v.modell, "myelith-30b-a3b");
    }

    /// ⚑ **Das Datum ist gerechnet und nicht geraten.**
    /// ⛔️ **Fund 387: das Verzeichnis darf nicht mitwachsen.**
    ///
    /// Eine Zeile je Nachricht kostet bei dem Gespraech, das eine
    /// Verdichtung ueberhaupt ausloest, mehr als die Zusammenfassung
    /// spart. Geprueft wird deshalb beides: **die Schranke haelt**, und
    /// **es faellt nichts heraus** (die Bloecke decken den ganzen
    /// Verlauf luecken- und ueberschneidungsfrei ab).
    #[test]
    fn das_grobverzeichnis_waechst_nicht_mit() {
        let d = ordner("grob");
        let viele: Vec<(String, String)> = (0..120)
            .map(|i| {
                let rolle = if i % 2 == 0 { "user" } else { "assistant" };
                (rolle.to_string(), format!("Abschnitt Nummer {i} mit etwas Text."))
            })
            .collect();
        let rel = schreiben(&d, "sitzung-viele", "myelith-0.6b", &viele).expect("schreiben");
        let pfad = d.join(&rel);

        let grob = grobverzeichnis(&pfad, 16).expect("Grobverzeichnis");
        assert!(grob.len() <= 16, "die Schranke haelt nicht: {} Zeilen", grob.len());

        let v = verzeichnis(&pfad).expect("Verzeichnis");
        assert_eq!(v.abschnitte.len(), 120);
        let spanne = |z: &str| -> (usize, usize) {
            let kern = z.split_whitespace().next().unwrap_or("");
            let (a, b) = kern.split_once('-').expect("von-bis");
            (a.parse().expect("von"), b.parse().expect("bis"))
        };
        let erste = spanne(&grob[0]);
        let letzte = spanne(grob.last().expect("eine Zeile"));
        assert_eq!(erste.0, v.abschnitte[0].von, "der erste Block beginnt woanders");
        assert_eq!(
            letzte.1,
            v.abschnitte[119].bis,
            "der letzte Block endet vor dem Verlauf"
        );
        let mut vorher = erste.0 - 1;
        for z in &grob {
            let (von, bis) = spanne(z);
            assert_eq!(von, vorher + 1, "zwischen zwei Bloecken klafft eine Luecke: {z}");
            assert!(bis >= von, "ein Block endet vor seinem Anfang: {z}");
            vorher = bis;
        }
    }

    /// ⚑ **Und die andere Haelfte: ein kurzes Gespraech behaelt die
    /// genaue Karte.** Die Schranke soll den Fall retten, in dem sie
    /// noetig ist, und den anderen nicht verschlechtern.
    #[test]
    fn ein_kurzes_verzeichnis_bleibt_abschnittsgenau() {
        let d = ordner("grob-kurz");
        let rel = schreiben(&d, "sitzung-kurz", "myelith-0.6b", &beispiel()).expect("schreiben");
        let grob = grobverzeichnis(&d.join(&rel), 16).expect("Grobverzeichnis");
        assert_eq!(grob.len(), 4, "vier Abschnitte, vier Zeilen: {grob:?}");
        assert!(
            grob.iter().any(|z| z.contains("Fuenfzehn Kerne")),
            "die Koepfe stehen weiter da: {grob:?}"
        );
        assert!(
            !grob.iter().any(|z| z.contains("Abschnitte)")),
            "hier wird nichts zusammengefasst: {grob:?}"
        );
    }

    /// ⚑ **Die Suche nennt den ganzen Wechsel, und sie zeigt den
    /// Fund.**
    ///
    /// Wer die Antwort sucht, braucht die Frage davor: Eine blosse
    /// Zeilennummer waere ein Fund ohne Zusammenhang, und ⛔️ **gemessen
    /// hielt ein Modell sie prompt fuer die Antwort selbst.** Geprueft
    /// wird deshalb, dass die Spanne **Frage und Antwort** deckt, dass
    /// die gefundene Zeile mitkommt, dass die Schreibweise egal ist und
    /// dass ein Wort, das nicht dasteht, nichts findet.
    #[test]
    fn die_suche_nennt_den_ganzen_abschnitt() {
        let d = ordner("suche");
        let rel = schreiben(&d, "sitzung-1", "myelith-0.6b", &beispiel()).expect("schreiben");
        let pfad = d.join(&rel);

        let treffer = suchen(&d, "GIBIBYTE", 20);
        assert_eq!(treffer.len(), 1, "genau ein Abschnitt traegt das Wort: {treffer:?}");
        let t = &treffer[0];
        assert_eq!(t.rolle, "assistant");
        assert!(t.text.contains("Vierundzwanzig Gibibyte"), "der Fund kommt mit: {t:?}");

        // ⚑ **Die Spanne deckt den Wechsel**: Das Wort steht in der
        // Antwort, also gehoert die Frage darueber dazu.
        let text = super::zeilen(&pfad, t.von, t.bis).expect("Zeilen");
        assert!(text.contains("Vierundzwanzig Gibibyte"), "{text}");
        assert!(text.contains("Und der Arbeitsspeicher?"), "die Frage fehlt: {text}");
        assert!(text.starts_with("### user"), "die Spanne beginnt bei der Frage: {text}");
        assert!(
            !text.contains("Fuenfzehn Kerne"),
            "es kam mehr heraus als der Wechsel: {text}"
        );

        assert!(suchen(&d, "Neutrinodetektor", 20).is_empty(), "das steht dort nicht");
        assert!(suchen(&d, "", 20).is_empty(), "eine leere Suche ist keine Frage");

        // ⛔️ **Die Fenstergrenzen, und sie waren zuerst ungeprueft.**
        // Die erste Fassung dieser Probe suchte ein Wort mitten im
        // Abschnitt; eine um zwei Zeilen verschobene Untergrenze fiel
        // ihr deshalb nicht auf. **Eine Gegenprobe, die nicht beisst,
        // ist ein Fund**, hier an der Pruefung.
        //
        // „Bitte genau." ist die **letzte** Zeile des ersten
        // Abschnitts: Eine zu kleine Obergrenze findet es nicht mehr,
        // und eine zu weit gezogene Untergrenze des **zweiten**
        // Abschnitts meldet es ein zweites Mal.
        let v = verzeichnis(&pfad).expect("Verzeichnis");
        let rand = suchen(&d, "Bitte genau", 20);
        assert_eq!(rand.len(), 1, "genau ein Abschnitt, nicht zwei: {rand:?}");
        assert_eq!(rand[0].von, v.abschnitte[0].von, "und zwar der erste: {rand:?}");
        assert_eq!(
            rand[0].bis, v.abschnitte[1].bis,
            "und die Spanne reicht bis zur Antwort: {rand:?}"
        );
        assert_eq!(
            rand[0].zeile,
            v.abschnitte[0].bis,
            "die gefundene Zeile ist die letzte des Abschnitts: {rand:?}"
        );
        //
        // ⚠️ **Eine um eine Zeile nach hinten verschobene Untergrenze
        // ist nicht beobachtbar**, und das steht hier, damit niemand
        // sie fuer geprueft haelt: Sie ueberspraenge die Zeile
        // `### rolle` und die Leerzeile, und beide tragen keinen
        // Inhalt, nach dem jemand sucht.
    }

    /// ⚑ **Und die Obergrenze haelt.**
    #[test]
    fn die_suche_haelt_die_obergrenze() {
        let d = ordner("suche-viele");
        let viele: Vec<(String, String)> = (0..40)
            .map(|i| ("user".to_string(), format!("Frage {i} zum Pruefstand.")))
            .collect();
        schreiben(&d, "sitzung-1", "myelith-0.6b", &viele).expect("schreiben");
        let treffer = suchen(&d, "Pruefstand", 20);
        assert_eq!(treffer.len(), 20, "hoechstens zwanzig: {}", treffer.len());
    }

    #[test]
    fn die_zeitmarke_stimmt() {
        assert_eq!(zeitmarke(0), "1970-01-01T00:00:00Z");
        assert_eq!(zeitmarke(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(zeitmarke(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    /// Eine Kopfzeile wird gekuerzt und nicht abgeschnitten.
    #[test]
    fn die_kopfzeile_bleibt_lesbar() {
        assert_eq!(kopfzeile("kurz"), "kurz");
        assert_eq!(kopfzeile("\n\n  mit Leerraum davor  \n"), "mit Leerraum davor");
        let lang = "w".repeat(200);
        let k = kopfzeile(&lang);
        assert_eq!(k.chars().count(), 70);
        assert!(k.ends_with('…'));
    }
}
