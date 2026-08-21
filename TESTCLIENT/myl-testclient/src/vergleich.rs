//! Protokolle mehrerer Maschinen gegenüberstellen (Fahrplanpunkt 2.1).
//!
//! Das ist die Stelle, an der der Client seinen Zweck einlöst. Alles
//! davor erzeugt Protokolle; hier wird aus ihnen ein Urteil.
//!
//! ## Der Nachweis braucht zwei Aussagen, nicht eine
//!
//! Ein Cross-Hardware-Determinismus-Nachweis besteht aus:
//!
//! 1. **Die Maschinen sind verschieden.**
//! 2. **Das Ergebnis ist trotzdem bitgleich.**
//!
//! Nur beide zusammen tragen. Zwei gleiche Digests von **derselben**
//! Maschine belegen nichts: sie zeigen, dass ein Programm zweimal
//! dasselbe gerechnet hat, und das ist keine Aussage über Hardware.
//!
//! Deshalb **verweigert** dieses Modul ein positives Urteil, wenn alle
//! Protokolle denselben Hardware-Fingerabdruck tragen ([`Urteil::EineMaschine`]).
//! Das ist ein Akzeptanzkriterium des Fahrplans und keine Höflichkeit:
//! Ein Werkzeug, das einen Nachweis vortäuscht, ist schlimmer als keines,
//! weil sein Ergebnis geglaubt wird.
//!
//! ## Was zuerst geprüft wird
//!
//! Vor jedem Digest-Vergleich steht der **Modellstand**. Weichen θ_v oder
//! der Artefakt-Digest ab, sind die Läufe unvergleichbar, und zwar
//! unabhängig davon, wie die Digests ausfallen: Bei verschiedenen
//! Modellen müssen sie verschieden sein. Ein solcher Befund als
//! „Determinismus verletzt" gemeldet wäre genau die Verwechslung, gegen
//! die es den Befehl `artefakte` gibt.
//!
//! ## Warum eigener JSON-Leser
//!
//! Gelesen wird ein Format, das dieses Programm selbst schreibt
//! ([`crate::logging`]): flache Objekte, Zeichenketten und Zahlen, sonst
//! nichts. Ein JSON-Crate wäre eine Abhängigkeit für einen Umfang, den
//! vierzig Zeilen abdecken. Der Leser ist streng: Eine Zeile, die er
//! nicht versteht, überspringt er, statt sie zu raten.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Ein eingelesenes Laufprotokoll.
#[derive(Debug, Clone, Default)]
pub struct Protokoll {
    pub datei: PathBuf,
    pub lauf: String,
    pub befehl: String,
    pub teilnehmer: String,
    pub einstellungen_id: String,
    pub fingerprint: String,
    /// Architektur, Betriebssystem und Backend als Kurzform, für die Anzeige.
    pub hardware: String,
    pub theta_v: String,
    pub artefakt_digest: String,
    /// Vergleichswerte in der Reihenfolge des Protokolls.
    pub ergebnisse: Vec<(String, String)>,
    pub abgeschlossen: bool,
    pub erfolgreich: bool,
}

impl Protokoll {
    /// Modellstand als ein Wert, die Größe, die vor jedem Digest-Vergleich
    /// übereinstimmen muss.
    fn modellstand(&self) -> (&str, &str) {
        (&self.theta_v, &self.artefakt_digest)
    }

    /// Bezeichnung für die Ausgabe: Name, sonst Hardware, sonst Datei.
    pub fn bezeichnung(&self) -> String {
        if !self.teilnehmer.is_empty() && self.teilnehmer != crate::logging::OHNE_NAME {
            return self.teilnehmer.clone();
        }
        if !self.hardware.is_empty() {
            return self.hardware.clone();
        }
        self.datei
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// Das Urteil über eine Gruppe vergleichbarer Läufe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Urteil {
    /// Digests gleich, Fingerabdrücke verschieden, Modellstand gleich.
    Nachweis,
    /// Digests gleich, aber alle Läufe stammen von derselben Maschine.
    /// **Kein Nachweis**: siehe Modul-Doku.
    EineMaschine,
    /// θ_v oder Artefakt-Digest weichen ab. Unvergleichbar, und
    /// ausdrücklich **kein** Hardware-Befund.
    Modellstand,
    /// Digests weichen bei gleichem Modellstand ab. Der eigentliche Befund.
    Abweichung,
    /// Weniger als zwei Protokolle: es gibt nichts zu vergleichen.
    ZuWenig,
}

impl Urteil {
    /// Trägt dieses Urteil den Nachweis?
    ///
    /// Nur [`Urteil::Nachweis`]. Alles andere ist entweder ein Befund oder
    /// eine unvollständige Messung, und in beiden Fällen darf ein Skript,
    /// das den Rückgabewert prüft, keinen Erfolg sehen.
    pub fn ist_nachweis(&self) -> bool {
        matches!(self, Urteil::Nachweis)
    }

    pub fn kurz(&self) -> &'static str {
        match self {
            Urteil::Nachweis => "NACHWEIS",
            Urteil::EineMaschine => "KEIN NACHWEIS (eine Maschine)",
            Urteil::Modellstand => "UNVERGLEICHBAR (Modellstand)",
            Urteil::Abweichung => "ABWEICHUNG",
            Urteil::ZuWenig => "ZU WENIG PROTOKOLLE",
        }
    }
}

/// Eine Gruppe von Protokollen, die verglichen werden dürfen: gleicher
/// Befehl, gleiche Einstellungs-Kennung.
#[derive(Debug, Clone)]
pub struct Gruppe {
    pub befehl: String,
    pub einstellungen_id: String,
    pub protokolle: Vec<Protokoll>,
    pub urteil: Urteil,
    /// Vergleichswert je Name: Digest → Bezeichnungen der Läufe.
    pub werte: Vec<(String, BTreeMap<String, Vec<String>>)>,
}

/// Liest alle `.jsonl` eines Verzeichnisses.
pub fn einlesen(dir: &Path) -> Result<Vec<Protokoll>, String> {
    let eintraege =
        fs::read_dir(dir).map_err(|e| format!("{} nicht lesbar: {}", dir.display(), e))?;

    let mut pfade: Vec<PathBuf> = eintraege
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .collect();
    pfade.sort();

    Ok(pfade.iter().filter_map(|p| protokoll_lesen(p)).collect())
}

/// Liest ein einzelnes Protokoll. `None`, wenn die Datei keines ist.
pub fn protokoll_lesen(datei: &Path) -> Option<Protokoll> {
    let text = fs::read_to_string(datei).ok()?;
    let mut p = Protokoll {
        datei: datei.to_path_buf(),
        ..Default::default()
    };
    let mut gesehen = false;
    let mut arch = String::new();
    let mut os = String::new();
    let mut backend = String::new();

    for zeile in text.lines() {
        let Some(felder) = felder(zeile) else { continue };
        let hole = |name: &str| -> String {
            felder
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        match hole("kind").as_str() {
            "run_started" => {
                gesehen = true;
                p.lauf = hole("run");
                p.befehl = hole("command");
                p.teilnehmer = hole("teilnehmer");
                p.einstellungen_id = hole("einstellungen_id");
            }
            "hardware" => match hole("key").as_str() {
                "fingerprint_sha256" => p.fingerprint = hole("value"),
                "arch" => arch = hole("value"),
                "os" => os = hole("value"),
                "backend_selected" => backend = hole("value"),
                _ => {}
            },
            "artifact" => match hole("key").as_str() {
                "theta_v" => p.theta_v = hole("value"),
                "artefakt_digest" => p.artefakt_digest = hole("value"),
                // Ältere Protokolle trugen die Einstellungs-Kennung hier
                // statt in `run_started`. Sie sollen lesbar bleiben.
                "einstellungen_id" if p.einstellungen_id.is_empty() => {
                    p.einstellungen_id = hole("value")
                }
                _ => {}
            },
            "result" => p.ergebnisse.push((hole("name"), hole("digest"))),
            "run_finished" => {
                p.abgeschlossen = true;
                p.erfolgreich = hole("ok") == "true";
            }
            _ => {}
        }
    }

    if !gesehen {
        return None;
    }
    p.hardware = [arch, os, backend]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    Some(p)
}

/// Gruppiert nach Befehl und Einstellungs-Kennung und urteilt je Gruppe.
pub fn gruppieren(protokolle: Vec<Protokoll>) -> Vec<Gruppe> {
    let mut nach_schluessel: BTreeMap<(String, String), Vec<Protokoll>> = BTreeMap::new();
    for p in protokolle {
        nach_schluessel
            .entry((p.befehl.clone(), p.einstellungen_id.clone()))
            .or_default()
            .push(p);
    }

    nach_schluessel
        .into_iter()
        .map(|((befehl, einstellungen_id), protokolle)| {
            let werte = werte_sammeln(&protokolle);
            let urteil = urteilen(&protokolle, &werte);
            Gruppe {
                befehl,
                einstellungen_id,
                protokolle,
                urteil,
                werte,
            }
        })
        .collect()
}

/// Sammelt je Vergleichswert, welcher Digest von welchen Läufen kam.
///
/// Die Namen behalten die Reihenfolge des ersten Protokolls: Ein Bericht,
/// dessen Zeilen bei jedem Aufruf anders sortiert sind, ist nicht diffbar.
fn werte_sammeln(protokolle: &[Protokoll]) -> Vec<(String, BTreeMap<String, Vec<String>>)> {
    let mut namen: Vec<String> = Vec::new();
    for p in protokolle {
        for (name, _) in &p.ergebnisse {
            if !namen.contains(name) {
                namen.push(name.clone());
            }
        }
    }

    namen
        .into_iter()
        .map(|name| {
            let mut nach_digest: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for p in protokolle {
                if let Some((_, digest)) = p.ergebnisse.iter().find(|(n, _)| *n == name) {
                    nach_digest
                        .entry(digest.clone())
                        .or_default()
                        .push(p.bezeichnung());
                }
            }
            (name, nach_digest)
        })
        .collect()
}

fn urteilen(
    protokolle: &[Protokoll],
    werte: &[(String, BTreeMap<String, Vec<String>>)],
) -> Urteil {
    if protokolle.len() < 2 {
        return Urteil::ZuWenig;
    }

    // Modellstand zuerst: Bei verschiedenen Modellen sagt ein
    // Digest-Vergleich nichts, weder im Guten noch im Schlechten.
    let erster = protokolle[0].modellstand();
    if protokolle.iter().any(|p| p.modellstand() != erster) {
        return Urteil::Modellstand;
    }

    // Ein Vergleichswert, der von zwei Läufen verschieden gerechnet wurde,
    // ist der Befund, für den es dieses Werkzeug gibt.
    let abweichung = werte
        .iter()
        .any(|(_, nach_digest)| nach_digest.len() > 1);
    if abweichung {
        return Urteil::Abweichung;
    }

    // Gleiche Digests von einer einzigen Maschine sind kein Nachweis.
    let verschiedene: std::collections::BTreeSet<&str> = protokolle
        .iter()
        .map(|p| p.fingerprint.as_str())
        .filter(|f| !f.is_empty())
        .collect();
    if verschiedene.len() < 2 {
        return Urteil::EineMaschine;
    }

    Urteil::Nachweis
}

/// Schreibt den Bericht und liefert das Gesamturteil.
///
/// `true` nur, wenn **jede** Gruppe den Nachweis trägt. Eine Gruppe, die
/// ihn nicht trägt, zieht das Gesamturteil herunter: sonst könnte ein
/// gelungener Determinismuslauf einen fehlgeschlagenen Shard-Lauf decken.
pub fn berichten(dir: &Path, gruppen: &[Gruppe]) -> bool {
    println!("  Protokolle aus {}\n", dir.display());

    if gruppen.is_empty() {
        println!("  Keine Protokolle gefunden.");
        println!("  Erwartet werden `.jsonl`-Dateien, wie sie jeder Testlauf schreibt.");
        return false;
    }

    let mut alles_gut = true;
    for g in gruppen {
        println!(
            "  ── {} · Einstellungen {} · {} Protokolle ──",
            g.befehl,
            g.einstellungen_id,
            g.protokolle.len()
        );
        for p in &g.protokolle {
            println!(
                "     {:<16} {:<28} θ_v {:<8} {}",
                p.bezeichnung(),
                if p.hardware.is_empty() { "" } else { &p.hardware },
                if p.theta_v.is_empty() { "" } else { &p.theta_v },
                kurz(&p.fingerprint),
            );
        }
        println!();

        for (name, nach_digest) in &g.werte {
            if nach_digest.len() == 1 {
                let digest = nach_digest.keys().next().map(String::as_str).unwrap_or("");
                println!("     = {:<24} {}", name, kurz(digest));
            } else {
                println!("     ≠ {:<24} {} verschiedene Werte:", name, nach_digest.len());
                for (digest, laeufe) in nach_digest {
                    println!("         {}  {}", kurz(digest), laeufe.join(", "));
                }
            }
        }

        println!("\n     Urteil: {}", g.urteil.kurz());
        for zeile in erlaeuterung(&g.urteil).lines() {
            println!("     {}", zeile);
        }
        println!();

        alles_gut &= g.urteil.ist_nachweis();
    }
    alles_gut
}

/// Was das Urteil bedeutet und was als Nächstes zu tun ist.
///
/// Der Text ist der eigentliche Nutzen des Befehls: Ein Urteilswort ohne
/// Folgerung lädt dazu ein, es falsch zu lesen: besonders
/// `EineMaschine`, das wie ein Erfolg aussieht, und `Modellstand`, das
/// wie ein Fehlschlag aussieht.
fn erlaeuterung(u: &Urteil) -> &'static str {
    match u {
        Urteil::Nachweis => {
            "Die Fingerabdrücke unterscheiden sich, die Vergleichswerte stimmen überein.\n\
             Das ist der Cross-Hardware-Determinismus-Nachweis für diese Einstellung."
        }
        Urteil::EineMaschine => {
            "Alle Protokolle tragen denselben Hardware-Fingerabdruck.\n\
             Gleiche Werte belegen hier nichts: Sie zeigen, dass dasselbe Programm auf\n\
             derselben Maschine zweimal dasselbe gerechnet hat. Es fehlt eine zweite\n\
             Architektur, nicht ein weiterer Lauf."
        }
        Urteil::Modellstand => {
            "θ_v oder der Artefakt-Digest weichen zwischen den Läufen ab.\n\
             Das ist KEIN Hardware-Befund. Hier wurde gegen verschiedene Modelle\n\
             gemessen; ein Bitgleichheitstest darüber hätte keine Aussage.\n\
             Erst `myl-test artefakte` auf allen Maschinen gleichziehen, dann erneut messen."
        }
        Urteil::Abweichung => {
            "Gleicher Modellstand, gleiche Eingabe, verschiedene Ergebnisse.\n\
             Das ist ein Befund an der Kernthese des Projekts und der wichtigste Fall,\n\
             den dieses Werkzeug finden kann. Protokolle sichern und melden."
        }
        Urteil::ZuWenig => {
            "Für einen Vergleich braucht es mindestens zwei Protokolle mit derselben\n\
             Einstellungs-Kennung. Weichen die Kennungen ab, liefen verschiedene\n\
             Parameter, dann ist die Eingabe zu vereinheitlichen, nicht das Ergebnis."
        }
    }
}

fn kurz(digest: &str) -> &str {
    &digest[..16.min(digest.len())]
}

/// Ablage der zugesandten Teilnehmerprotokolle, relativ zur Repository-Wurzel.
pub const ORDNER: &str = "TESTCLIENT/Vergleiche";
/// Ablage der Vergleichsberichte.
pub const BERICHTE: &str = "TESTCLIENT/Vergleiche/Berichte";

/// Wo der Koordinator die zugesandten Protokolle ablegt.
pub fn vergleichsordner(repo: &Path) -> PathBuf {
    repo.join(ORDNER)
}

/// Wo die Berichte hingeschrieben werden.
///
/// Ein **Unterordner** der Eingabe, nicht ihr Geschwister: Läge der
/// Bericht neben den Protokollen, hätte der nächste Aufruf ihn als
/// Eingabe mitgelesen. Er trägt zwar keine `.jsonl`-Endung und wäre
/// deshalb heute unschädlich, aber das ist eine Eigenschaft des
/// Dateinamens, keine des Verfahrens, und darauf soll sich niemand
/// verlassen müssen.
pub fn berichtsordner(repo: &Path) -> PathBuf {
    repo.join(BERICHTE)
}

/// Schreibt den ausführlichen Bericht und liefert seinen Pfad.
///
/// Markdown, weil der Bericht weitergereicht wird: an Mitwirkende, in
/// Tickets, gelegentlich in ein `eval/results/`-Verzeichnis. Er enthält
/// dieselben Angaben wie die Bildschirmausgabe und zusätzlich, was dort
/// keinen Platz hat: vollständige Digests statt Kurzform, Dateinamen,
/// Zeitpunkt des Vergleichs.
pub fn bericht_schreiben(
    ziel: &Path,
    quelle: &Path,
    gruppen: &[Gruppe],
) -> Result<PathBuf, String> {
    fs::create_dir_all(ziel).map_err(|e| format!("{} nicht anlegbar: {}", ziel.display(), e))?;

    let (datum, uhrzeit) = crate::logging::datum_und_uhrzeit();
    let pfad = ziel.join(format!("vergleich_{}_{}.md", datum, uhrzeit));
    fs::write(&pfad, bericht_text(quelle, &datum, &uhrzeit, gruppen))
        .map_err(|e| format!("{} nicht schreibbar: {}", pfad.display(), e))?;
    Ok(pfad)
}

/// Setzt den Berichtstext zusammen. Getrennt vom Schreiben, damit er ohne
/// Dateisystem prüfbar ist.
fn bericht_text(quelle: &Path, datum: &str, uhrzeit: &str, gruppen: &[Gruppe]) -> String {
    use std::fmt::Write as _;
    let mut t = String::new();

    let _ = writeln!(t, "# Vergleichsbericht {} {}", datum, uhrzeit);
    let _ = writeln!(t);
    let _ = writeln!(t, "**Quelle:** `{}`  ", quelle.display());
    let _ = writeln!(
        t,
        "**Protokolle:** {}  ",
        gruppen.iter().map(|g| g.protokolle.len()).sum::<usize>()
    );
    let _ = writeln!(t, "**Gruppen:** {}", gruppen.len());
    let _ = writeln!(t);

    if gruppen.is_empty() {
        let _ = writeln!(
            t,
            "Keine Protokolle gefunden. Erwartet werden `.jsonl`-Dateien, \
             wie sie jeder Testlauf schreibt."
        );
        return t;
    }

    let alles_gut = gruppen.iter().all(|g| g.urteil.ist_nachweis());
    let _ = writeln!(
        t,
        "**Gesamturteil:** {}",
        if alles_gut {
            "NACHWEIS über alle Gruppen"
        } else {
            "kein durchgehender Nachweis: siehe die Urteile je Gruppe"
        }
    );

    for g in gruppen {
        let _ = writeln!(t);
        let _ = writeln!(
            t,
            "## {} · Einstellungen `{}`",
            g.befehl, g.einstellungen_id
        );
        let _ = writeln!(t);
        let _ = writeln!(t, "### Beteiligte Läufe");
        let _ = writeln!(t);
        let _ = writeln!(t, "| Teilnehmer | Hardware | θ_v | Artefakt-Digest | Fingerabdruck | Datei |");
        let _ = writeln!(t, "|---|---|---|---|---|---|");
        for p in &g.protokolle {
            let _ = writeln!(
                t,
                "| {} | {} | {} | `{}` | `{}` | `{}` |",
                p.bezeichnung(),
                leer_als_strich(&p.hardware),
                leer_als_strich(&p.theta_v),
                kurz(&p.artefakt_digest),
                kurz(&p.fingerprint),
                p.datei
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            );
        }

        let _ = writeln!(t);
        let _ = writeln!(t, "### Vergleichswerte");
        let _ = writeln!(t);
        for (name, nach_digest) in &g.werte {
            if nach_digest.len() == 1 {
                let digest = nach_digest.keys().next().map(String::as_str).unwrap_or("");
                let _ = writeln!(t, "- **{}**: übereinstimmend: `{}`", name, digest);
            } else {
                let _ = writeln!(t, "- **{}**. ABWEICHUNG, {} verschiedene Werte:", name, nach_digest.len());
                for (digest, laeufe) in nach_digest {
                    let _ = writeln!(t, "  - `{}`: {}", digest, laeufe.join(", "));
                }
            }
        }

        let _ = writeln!(t);
        let _ = writeln!(t, "### Urteil: {}", g.urteil.kurz());
        let _ = writeln!(t);
        for zeile in erlaeuterung(&g.urteil).lines() {
            let _ = writeln!(t, "{}", zeile);
        }
    }

    let _ = writeln!(t);
    let _ = writeln!(t, "---");
    let _ = writeln!(t);
    let _ = writeln!(
        t,
        "Dieser Bericht hält den Stand des Quellordners zum genannten \
         Zeitpunkt fest. Ein **bestätigter** Cross-Hardware-Nachweis gehört \
         nach `INTEGER_LLM/eval/results/` (Fahrplanpunkt 2.3), der \
         Berichtsordner wird nicht versioniert."
    );
    t
}

fn leer_als_strich(s: &str) -> &str {
    if s.is_empty() {
        ""
    } else {
        s
    }
}

/// `myl-test vergleich`: einlesen, gruppieren, berichten, festhalten.
pub fn run(dir: &Path, berichte: Option<&Path>) -> bool {
    let protokolle = match einlesen(dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("  {}", e);
            return false;
        }
    };

    let gruppen = gruppieren(protokolle);
    let ok = berichten(dir, &gruppen);

    if let Some(ziel) = berichte {
        match bericht_schreiben(ziel, dir, &gruppen) {
            Ok(pfad) => println!("  Bericht: {}", pfad.display()),
            // Ein fehlgeschlagener Bericht darf das Urteil nicht kippen:
            // Es steht bereits auf dem Bildschirm und ist damit gefällt.
            Err(e) => eprintln!("  WARNUNG: Bericht nicht geschrieben: {}", e),
        }
    }
    ok
}

/// Zerlegt eine JSONL-Zeile in ihre Felder.
///
/// Gegenstück zu `logging::json_escape`. `None` bei allem, was nicht dem
/// selbst geschriebenen Format entspricht: lieber eine Zeile auslassen
/// als sie falsch deuten.
fn felder(zeile: &str) -> Option<Vec<(String, String)>> {
    let z: Vec<char> = zeile.trim().chars().collect();
    if z.first() != Some(&'{') || z.last() != Some(&'}') {
        return None;
    }

    let mut i = 1;
    let mut out = Vec::new();
    loop {
        while i < z.len() && (z[i] == ',' || z[i].is_whitespace()) {
            i += 1;
        }
        if i >= z.len() || z[i] == '}' {
            return Some(out);
        }

        let schluessel = lies_string(&z, &mut i)?;
        while i < z.len() && z[i].is_whitespace() {
            i += 1;
        }
        if z.get(i) != Some(&':') {
            return None;
        }
        i += 1;
        while i < z.len() && z[i].is_whitespace() {
            i += 1;
        }

        let wert = if z.get(i) == Some(&'"') {
            lies_string(&z, &mut i)?
        } else {
            // Zahl oder Wahrheitswert: bis zum nächsten Trenner.
            let start = i;
            while i < z.len() && z[i] != ',' && z[i] != '}' {
                i += 1;
            }
            z[start..i].iter().collect::<String>().trim().to_string()
        };
        out.push((schluessel, wert));
    }
}

fn lies_string(z: &[char], i: &mut usize) -> Option<String> {
    if z.get(*i) != Some(&'"') {
        return None;
    }
    *i += 1;
    let mut out = String::new();
    while *i < z.len() {
        match z[*i] {
            '"' => {
                *i += 1;
                return Some(out);
            }
            '\\' => {
                *i += 1;
                match z.get(*i)? {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    'u' => {
                        let hex: String = z.get(*i + 1..*i + 5)?.iter().collect();
                        out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                        *i += 4;
                    }
                    _ => return None,
                }
                *i += 1;
            }
            c => {
                out.push(c);
                *i += 1;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::{Event, LogZiel, RunLog};

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("myl-testclient-vergleich-{}", name));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// Schreibt ein Protokoll, wie ein echter Lauf es hinterlässt.
    fn protokoll_schreiben(
        dir: &Path,
        teilnehmer: &str,
        einstellungen: &str,
        arch: &str,
        fingerprint: &str,
        theta_v: &str,
        digest: &str,
    ) {
        let mut log = RunLog::mit_ziel(
            LogZiel::neu(dir, "determinismus", teilnehmer, einstellungen, arch),
            false,
        );
        log.event(Event::Hardware {
            key: "arch".into(),
            value: arch.into(),
        });
        log.event(Event::Hardware {
            key: "fingerprint_sha256".into(),
            value: fingerprint.into(),
        });
        log.event(Event::Artifact {
            key: "theta_v".into(),
            value: theta_v.into(),
        });
        log.event(Event::Artifact {
            key: "artefakt_digest".into(),
            value: "c42bb8a8d85bba5a".into(),
        });
        log.result("determinismus", digest, "bitgleich");
        log.finish(true);
    }

    fn urteil_ueber(dir: &Path) -> Urteil {
        let gruppen = gruppieren(einlesen(dir).expect("lesbar"));
        assert_eq!(gruppen.len(), 1, "eine Gruppe erwartet");
        gruppen[0].urteil.clone()
    }

    /// Der Regelfall, für den es das Werkzeug gibt: zwei Architekturen,
    /// gleicher Digest.
    #[test]
    fn zwei_architekturen_gleicher_digest_ist_der_nachweis() {
        let dir = tempdir("nachweis");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.17.0", "digest-x");
        assert_eq!(urteil_ueber(&dir), Urteil::Nachweis);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Das Akzeptanzkriterium des Fahrplans: Zwei Läufe von derselben
    /// Maschine dürfen **kein** positives Urteil ergeben, auch wenn die
    /// Digests übereinstimmen.
    #[test]
    fn eine_maschine_ergibt_keinen_nachweis() {
        let dir = tempdir("eine-maschine");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        let urteil = urteil_ueber(&dir);
        assert_eq!(urteil, Urteil::EineMaschine);
        assert!(!urteil.ist_nachweis(), "darf nicht als Nachweis gelten");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Verschiedene Digests bei gleichem Modellstand sind der eigentliche
    /// Befund und dürfen nicht als Modellstandsfrage abgetan werden.
    #[test]
    fn verschiedene_digests_sind_ein_befund() {
        let dir = tempdir("abweichung");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.17.0", "digest-y");
        assert_eq!(urteil_ueber(&dir), Urteil::Abweichung);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein abweichender Modellstand muss **vor** dem Digest-Vergleich
    /// erkannt werden: sonst meldet der Client einen Hardware-Befund,
    /// wo zwei verschiedene Modelle verglichen wurden.
    #[test]
    fn abweichender_modellstand_schlaegt_den_digestvergleich() {
        let dir = tempdir("modellstand");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.16.0", "digest-y");
        assert_eq!(urteil_ueber(&dir), Urteil::Modellstand);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ein_einzelnes_protokoll_ist_kein_vergleich() {
        let dir = tempdir("einzeln");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        assert_eq!(urteil_ueber(&dir), Urteil::ZuWenig);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Verschiedene Einstellungen sind verschiedene Gruppen: sie dürfen
    /// nicht gegeneinander verglichen werden.
    #[test]
    fn verschiedene_einstellungen_bleiben_getrennt() {
        let dir = tempdir("gruppen");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "99998888", "x86-64", "fp-b", "0.17.0", "digest-y");
        let gruppen = gruppieren(einlesen(&dir).expect("lesbar"));
        assert_eq!(gruppen.len(), 2);
        assert!(gruppen.iter().all(|g| g.urteil == Urteil::ZuWenig));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Der Bericht darf nur bei einem echten Nachweis Erfolg melden.
    #[test]
    fn bericht_meldet_nur_beim_nachweis_erfolg() {
        let dir = tempdir("bericht");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        assert!(!run(&dir, None), "eine Maschine darf keinen Erfolg melden");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein leeres Verzeichnis ist kein Nachweis, aber auch kein Absturz.
    #[test]
    fn leeres_verzeichnis_meldet_sauber() {
        let dir = tempdir("leer");
        fs::create_dir_all(&dir).unwrap();
        assert!(!run(&dir, None));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fehlendes_verzeichnis_meldet_sauber() {
        assert!(!run(Path::new("/nicht/vorhanden/myl"), None));
    }

    /// Der Leser muss genau das zurückgeben, was der Schreiber maskiert
    /// hat: sonst weichen Digests scheinbar ab, weil ein Wert falsch
    /// gelesen wurde.
    #[test]
    fn maskierte_zeichen_kommen_unveraendert_zurueck() {
        let original = "Zeile\nmit \"Anführung\" und \\Backslash\tTab";
        let zeile = format!(
            "{{\"t_ms\":7,\"kind\":\"note\",\"text\":\"{}\"}}",
            crate::logging::json_escape(original)
        );
        let f = felder(&zeile).expect("lesbar");
        let text = f.iter().find(|(k, _)| k == "text").expect("text").1.clone();
        assert_eq!(text, original);
    }

    #[test]
    fn zahlen_und_wahrheitswerte_werden_gelesen() {
        let f = felder(r#"{"t_ms":42,"kind":"run_finished","ok":"true"}"#).expect("lesbar");
        assert_eq!(f[0], ("t_ms".to_string(), "42".to_string()));
        assert_eq!(f[2], ("ok".to_string(), "true".to_string()));
    }

    /// Was nicht dem eigenen Format entspricht, wird ausgelassen statt
    /// geraten.
    #[test]
    fn unverstaendliche_zeilen_werden_ausgelassen() {
        assert!(felder("kein json").is_none());
        assert!(felder("{unquoted: 1}").is_none());
        assert!(felder(r#"{"offen":"#).is_none());
        assert!(felder("{}").expect("leeres Objekt").is_empty());
    }

    /// Eine Datei ohne `run_started` ist kein Protokoll.
    #[test]
    fn fremde_dateien_werden_nicht_als_protokoll_gelesen() {
        let dir = tempdir("fremd");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("fremd.jsonl"), "{\"kind\":\"etwas\"}\n").unwrap();
        assert!(einlesen(&dir).expect("lesbar").is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Der Bericht muss das Urteil und die vollständigen Digests tragen:
    /// die Bildschirmausgabe kürzt beides, und der Bericht wird
    /// weitergereicht.
    #[test]
    fn bericht_traegt_urteil_und_volle_digests() {
        let dir = tempdir("bericht-inhalt");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.17.0", "digest-x");
        let gruppen = gruppieren(einlesen(&dir).expect("lesbar"));

        let text = bericht_text(&dir, "2026-08-21", "143022", &gruppen);
        assert!(text.contains("# Vergleichsbericht 2026-08-21 143022"), "{text}");
        assert!(text.contains("NACHWEIS"), "{text}");
        assert!(text.contains("anna") && text.contains("björn"), "{text}");
        assert!(text.contains("digest-x"), "voller Digest fehlt");
        assert!(text.contains("0.17.0"), "θ_v fehlt");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein Bericht ohne Protokolle darf nicht so aussehen, als sei nichts
    /// zu beanstanden gewesen.
    #[test]
    fn leerer_bericht_sagt_es_deutlich() {
        let text = bericht_text(Path::new("/leer"), "2026-08-21", "143022", &[]);
        assert!(text.contains("Keine Protokolle gefunden"), "{text}");
        assert!(!text.contains("NACHWEIS"), "leerer Bericht darf nichts belegen");
    }

    /// Der Bericht landet in seinem eigenen Ordner und wird beim nächsten
    /// Vergleich **nicht** als Eingabe mitgelesen.
    #[test]
    fn bericht_wird_nicht_zur_eingabe() {
        let dir = tempdir("bericht-ablage");
        let berichte = dir.join("Berichte");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.17.0", "digest-x");

        assert!(run(&dir, Some(&berichte)));
        let geschrieben: Vec<_> = fs::read_dir(&berichte)
            .expect("Berichtsordner")
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(geschrieben.len(), 1, "genau ein Bericht erwartet");

        // Zweiter Lauf über denselben Ordner: Die Zahl der eingelesenen
        // Protokolle darf sich nicht verändert haben.
        assert_eq!(einlesen(&dir).expect("lesbar").len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein unbeschreibbarer Berichtsordner darf das Urteil nicht kippen:
    /// es steht bereits auf dem Bildschirm und ist damit gefällt.
    #[test]
    fn fehlender_berichtsordner_kippt_das_urteil_nicht() {
        let dir = tempdir("bericht-fehler");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.17.0", "digest-x");
        assert!(run(&dir, Some(Path::new("/proc/kein-schreibzugriff/myl"))));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ohne Namen muss die Bezeichnung auf die Hardware zurückfallen:
    /// ein Bericht mit lauter „ohne-name" wäre nicht auswertbar.
    #[test]
    fn bezeichnung_faellt_auf_hardware_zurueck() {
        let p = Protokoll {
            teilnehmer: crate::logging::OHNE_NAME.to_string(),
            hardware: "aarch64-macos-reference".to_string(),
            ..Default::default()
        };
        assert_eq!(p.bezeichnung(), "aarch64-macos-reference");
    }
}
