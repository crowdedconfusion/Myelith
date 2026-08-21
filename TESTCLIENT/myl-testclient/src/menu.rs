//! Interaktives Menü — „Durchklicken" statt Befehle tippen.
//!
//! Wird gestartet, wenn `myl-test` **ohne Unterbefehl** aufgerufen wird.
//! Der Grund: Die Hardwaretests laufen auf fremden Maschinen, oft von
//! Leuten, die das Projekt nicht täglich sehen. Wer erst eine Hilfeseite
//! lesen muss, um einen Testlauf zu starten, führt ihn seltener aus.
//!
//! Ausgewählt wird mit den Pfeiltasten und Enter; die Ziffer daneben
//! bleibt gültig. Wo kein Terminal vorhanden ist — in einer Pipe, in
//! einem Skript, in einer seriellen Konsole ohne Rohmodus —, fällt die
//! Auswahl auf zeilenweise Eingabe zurück. Das Verfahren steht in
//! [`crate::auswahl`]; hier stehen nur die Punkte.


use std::path::PathBuf;

use crate::auswahl::{self, Punkt};
use crate::logging::LogZiel;
use crate::logging::RunLog;
use crate::spec::TestPlan;
use crate::{banner, hardware, runs, stack};

/// Laufeinstellungen, die im Menü verändert werden können.
pub struct Einstellungen {
    pub prompts: Vec<String>,
    pub steps: usize,
    pub shards: usize,
    pub artifacts: PathBuf,
    pub logs: PathBuf,
    /// Kurzkennung der Einstellungen — benennt das Protokollverzeichnis.
    /// `ohne-plan`, solange kein Testplan geladen wurde.
    pub einstellungen_id: String,
    /// Name des Teilnehmers. Steht im Protokoll und im Dateinamen.
    pub teilnehmer: String,
}

impl Einstellungen {
    /// Baut den zu den aktuellen Werten passenden Testplan.
    fn als_plan(&self) -> TestPlan {
        TestPlan {
            plan_id: "unbenannt".to_string(),
            prompts: self.prompts.clone(),
            steps: self.steps,
            shards: self.shards,
            model: crate::runs::DEFAULT_MODEL.to_string(),
        }
    }

    /// Übernimmt einen geladenen Plan.
    fn uebernehmen(&mut self, plan: &TestPlan) {
        self.prompts = plan.prompts.clone();
        self.steps = plan.steps;
        self.shards = plan.shards;
        self.einstellungen_id = plan.short_id();
    }
}

impl Einstellungen {
    fn zeigen(&self) {
        println!("  Aktuelle Einstellungen:");
        match self.prompts.as_slice() {
            [einer] => println!("    Prompt      {:?}", einer),
            viele => {
                println!("    Prompts     {}", viele.len());
                for (i, p) in viele.iter().enumerate() {
                    println!("      {}. {:?}", i + 1, gekuerzt(p));
                }
            }
        }
        println!("    Token       {}", self.steps);
        println!("    Shards      {}", self.shards);
        println!("    Artefakte   {}", kurz(&self.artifacts));
        println!("    Protokolle  {}", kurz(&self.logs));
        println!("    Teilnehmer  {}", self.teilnehmer);
        println!("    Einstellungs-ID {}", self.einstellungen_id);
    }
}

/// Kürzt einen Prompt für die Anzeige in der Einstellungsübersicht.
fn gekuerzt(p: &str) -> String {
    if p.chars().count() <= 52 {
        return p.to_string();
    }
    format!("{}…", p.chars().take(51).collect::<String>())
}

/// Kürzt lange Pfade auf die letzten drei Bestandteile.
fn kurz(p: &std::path::Path) -> String {
    let teile: Vec<_> = p.components().collect();
    if teile.len() <= 3 {
        return p.display().to_string();
    }
    let rest: PathBuf = teile[teile.len() - 3..].iter().collect();
    format!("…/{}", rest.display())
}

/// Das Nutzermenü: nur, was jeder Teilnehmer braucht.
///
/// Wer eine Maschine beisteuert, soll messen, vergleichen und das
/// Ergebnis schicken; er soll keine Testpläne erzeugen und keine Pfade
/// umstellen. Alles Weitere liegt eine Ebene tiefer unter [9].
fn menue_nutzer() -> Vec<Punkt> {
    vec![
        Punkt::neu(
            '1',
            "Testlauf starten",
            "Hardware, Determinismus, Shards und Protokoll-Durchlauf\n\
             nacheinander. Der vollständige Bericht dieser Maschine.",
        ),
        Punkt::neu(
            '2',
            "Testdatei wählen",
            "Legt Prompt, Token, Shards und Modell fest. Beschafft das\n\
             Modell, falls es fehlt. Danach mit [1] starten.",
        ),
        Punkt::neu(
            '3',
            "Protokolle vergleichen",
            "Stellt die Protokolle im Protokollordner gegenüber und urteilt,\n\
             ob sie den Cross-Hardware-Nachweis tragen.",
        ),
        Punkt::neu('4', "Anleitung lesen", ""),
        Punkt::neu('9', "Entwickler-Menü", ""),
        Punkt::neu('0', "Beenden", ""),
    ]
}

/// Das Entwickler-Menü: Einzelläufe und alles, was Vorwissen braucht.
///
/// Getrennt vom Nutzermenü, weil eine lange Liste den Teilnehmer bremst
/// und die Punkte, die er versehentlich wählt, ihm nichts nützen. Wer
/// hier hereinkommt, weiß in der Regel, was er sucht.
fn menue_entwickler() -> Vec<Punkt> {
    vec![
        Punkt::neu('1', "Hardware erheben", ""),
        Punkt::neu('2', "Determinismus prüfen", ""),
        Punkt::neu('3', "Geshardete Inferenz", ""),
        Punkt::neu('4', "Protokoll-Durchlauf (Stack)", ""),
        Punkt::neu('5', "Artefakte prüfen (Digest gegen das Register)", ""),
        Punkt::neu('6', "Testplan erzeugen und speichern", ""),
        Punkt::neu('7', "Einstellungen ändern (Prompt, Token, Shards, Pfade)", ""),
        Punkt::neu('8', "Namen ändern", ""),
        Punkt::neu('9', "Artefakte und Gewichte freigeben (Plattenplatz)", ""),
        Punkt::neu('0', "Zurück", ""),
    ]
}

/// Startet das Menü und kehrt mit dem Gesamtergebnis zurück.
pub fn run(mut e: Einstellungen) -> bool {
    // Erstes Bild: Animation, dann Logo und Namenseingabe, sonst nichts.
    //
    // Der Name steht vor allem anderen, weil er jede Protokolldatei
    // benennt, die in dieser Sitzung entsteht — nachträglich umbenennen
    // müsste ihn sonst der Koordinator.
    banner::start_if(true);
    if e.teilnehmer == crate::logging::OHNE_NAME {
        e.teilnehmer = namen_erfragen();
    }

    // Zweites Bild: Testplan wählen.
    //
    // Reihenfolge beim Start: erst Testplan, dann Artefakt.
    //
    // Ein Plan legt das Modell fest. Wer zuerst nach dem Artefakt fragt und
    // danach den Plan lädt, hat entweder die falsche Frage gestellt oder
    // muss sie zurücknehmen. Ist ein Plan gewählt, ergibt sich das Artefakt
    // aus ihm, und es gibt nichts mehr zu fragen.
    banner::bildschirm();
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let mut sagen = |t: String| println!("  {}", t);
    let plan = plan_waehlen(&repo, &mut sagen);

    // Drittes Bild: Artefakt auflösen und, wenn ein Plan gewählt wurde,
    // gleich messen.
    banner::bildschirm();
    let mut frage = |prompt: &str| -> Option<String> { auswahl::frage(&format!("  {}", prompt)) };
    let mut f: crate::artefakte::Rueckfrage = Some(&mut frage);

    let ergebnis = match &plan {
        Some(p) => {
            e.uebernehmen(p);
            println!("  Plan \"{}\" übernommen: Modell {}, {} Token, {} Shards.",
                     p.plan_id, p.model, p.steps, p.shards);
            crate::artefakte::beschaffen_fuer(&repo, &p.model, &mut f, &mut |t| println!("  {}", t))
        }
        None => crate::artefakte::beschaffen(&repo, &mut f, &mut |t| println!("  {}", t)),
    };

    match ergebnis {
        Ok(pfad) => {
            e.artifacts = pfad;
            println!();
        }
        Err(fehler) => {
            for zeile in fehler.lines() {
                println!("  {}", zeile);
            }
            println!("\n  Die Punkte 2 und 3 brauchen ein Modell und werden fehlschlagen.\n");
        }
    }

    // Ist ein Plan gewählt und das Artefakt bereit, läuft der Testlauf
    // sofort. Wer einen Plan auswählt, will messen, nicht noch ein Menü
    // bedienen.
    let mut letztes_ergebnis = true;
    if plan.is_some() && e.artifacts.exists() {
        println!("  Plan wird ausgeführt.\n");
        letztes_ergebnis = testlauf(&e);
        println!("\n  Durchgang beendet. Das Menü steht für weitere Läufe bereit.\n");
    }

    weiter();

    loop {
        banner::bildschirm();
        e.zeigen();
        let Some(wahl) = auswahl::waehlen("Was möchtest du tun?", &menue_nutzer()) else {
            println!("\n  Fertig.");
            return letztes_ergebnis;
        };

        println!();
        match wahl {
            '1' => {
                letztes_ergebnis = testlauf(&e);
                weiter();
            }
            '2' => {
                testdatei_waehlen(&mut e);
                weiter();
            }
            '3' => {
                letztes_ergebnis = vergleichen(&e);
                weiter();
            }
            '4' => {
                anleitung_zeigen();
                weiter();
            }
            '9' => letztes_ergebnis = entwickler(&mut e, letztes_ergebnis),
            '0' => {
                println!("  Fertig.");
                return letztes_ergebnis;
            }
            _ => {}
        }
    }
}

/// Fragt den Namen, unter dem die Protokolle dieser Sitzung laufen.
///
/// **Ein Name, keine Kennung.** Er beschreibt den Teilnehmer, nicht die
/// Maschine — die steht ohnehin gemessen im Protokoll. Wer den Namen
/// leer lässt, bekommt `ohne-name`: sichtbar fehlend statt stillschweigend
/// geraten. Ein aus der Umgebung übernommener Benutzername wäre bequemer
/// und zugleich eine Personenangabe, die niemand angeordnet hat — und
/// Protokolle wandern per Copy-Paste in Tickets.
fn namen_erfragen() -> String {
    println!("  Unter welchem Namen sollen die Protokolle dieser Sitzung laufen?");
    println!("  Er steht im Dateinamen und im Protokoll, damit der Koordinator sie");
    println!("  ohne Rückfrage zuordnen kann. Leer lassen ist erlaubt.\n");

    let eingabe = auswahl::frage("  Name: ").unwrap_or_default();
    let name = eingabe.trim();
    if name.is_empty() {
        println!("\n  Kein Name — die Protokolle laufen unter „{}\".\n", crate::logging::OHNE_NAME);
        return crate::logging::OHNE_NAME.to_string();
    }
    println!("\n  Hallo, {}.\n", name);
    name.to_string()
}

fn ja_nein(b: bool) -> &'static str {
    if b {
        "OK"
    } else {
        "FEHLGESCHLAGEN"
    }
}

/// Menüpunkt 9: einen Testplan erzeugen und verteilen.
fn plan_erzeugen(e: &Einstellungen) {
    let Some(kennung) = auswahl::frage("  Kennung des Durchgangs (z. B. 2026-08-18-cross-arch-01): ")
    else {
        return;
    };
    let kennung = kennung.trim();
    let kennung = if kennung.is_empty() { "unbenannt" } else { kennung };

    let mut plan = e.als_plan();
    plan.plan_id = kennung.to_string();
    // In den Planordner, nicht ins Arbeitsverzeichnis: Von dort liest der
    // Client beim Start, dort suchen die Teilnehmer.
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let ziel = repo
        .join(crate::plaene::ORDNER)
        .join(format!("{}.plan", kennung));

    match plan.save(&ziel) {
        Ok(()) => {
            println!("\n  Geschrieben: {}", ziel.display());
            println!("    Einstellungs-ID {}", plan.short_id());
            println!("\n  Diese Datei unverändert an alle Teilnehmer schicken.");
            println!("  Sie laden sie über Menüpunkt 8 oder mit");
            println!("      myl-test --plan {} determinismus", ziel.display());
            println!("\n  Alle Protokolle landen dann unter");
            println!("      logs/<befehl>/<datum>_{}/", plan.short_id());
        }
        Err(err) => println!("\n  {}", err),
    }
}

fn einstellungen_aendern(e: &mut Einstellungen) {
    let punkte = vec![
        Punkt::neu('1', "Prompt", ""),
        Punkt::neu('2', "Token", ""),
        Punkt::neu('3', "Shards", ""),
        Punkt::neu('4', "Artefaktverzeichnis", ""),
        Punkt::neu('5', "Protokollverzeichnis", ""),
        Punkt::neu('0', "Zurück", ""),
    ];
    let Some(wahl) = auswahl::waehlen("Einstellungen", &punkte) else {
        return;
    };

    let nachfragen = |text: &str| -> Option<String> {
        auswahl::frage(&format!("  {}: ", text)).filter(|s| !s.trim().is_empty())
    };

    match wahl {
        '1' => {
            if let Some(v) = nachfragen("Neuer Prompt (ersetzt alle bisherigen)") {
                e.prompts = vec![v];
            }
        }
        '2' => {
            if let Some(v) = nachfragen("Anzahl Token") {
                match v.parse::<usize>() {
                    Ok(n) if n > 0 => e.steps = n,
                    _ => println!("  Ungültig — unverändert."),
                }
            }
        }
        '3' => {
            if let Some(v) = nachfragen("Anzahl Shards") {
                match v.parse::<usize>() {
                    Ok(n) if n > 0 => e.shards = n,
                    _ => println!("  Ungültig — unverändert."),
                }
            }
        }
        '4' => {
            if let Some(v) = nachfragen("Artefaktverzeichnis") {
                e.artifacts = PathBuf::from(v);
            }
        }
        '5' => {
            if let Some(v) = nachfragen("Protokollverzeichnis") {
                e.logs = PathBuf::from(v);
            }
        }
        _ => {}
    }
}

fn anleitung_zeigen() {
    println!(
        "\
  ── Tests auf mehreren Maschinen ──────────────────────────────

  Der Cross-Hardware-Nachweis braucht ZWEI Aussagen, nicht eine:

    (a) Die Maschinen sind verschieden.
    (b) Das Ergebnis ist trotzdem gleich.

  Nur beide zusammen sind ein Nachweis. Zwei gleiche Digests von
  derselben Maschine belegen nichts.

  Ablauf für jede beteiligte Person:

    1. `myl-test` starten, Menüpunkt 1 (Hardware).
       → Den Wert hinter `hardware_fingerprint` notieren.

    2. Der Koordinator gibt EINEN Prompt und EINE Tokenzahl vor.
       Alle nehmen exakt dieselben Werte (Menüpunkt 6).

    3. Menüpunkt 2 (Determinismus).
       → Den Wert hinter `determinismus` notieren.

    4. Die `.jsonl`-Protokolle an den Koordinator schicken.
       Sie enthalten keine Prompttexte, nur deren Hash.

  Ergebnis:
    Fingerabdrücke verschieden + Digests gleich  → Nachweis erbracht
    Fingerabdrücke gleich                        → nichts bewiesen
    Digests verschieden                          → BEFUND, siehe unten

  Bei verschiedenen Digests zuerst prüfen:
    · Ist θ_v auf beiden Maschinen identisch? (Artefakt-Hashes)
    · Ist der Prompt zeichengleich? (Der Prompt-Hash im Protokoll
      beantwortet das ohne Nachfragen.)
    · Läuft dasselbe Backend? (`backend_selected` im Protokoll)
  Erst wenn alle drei gleich sind und die Digests trotzdem abweichen,
  ist es ein Befund an der Kernthese — und dann ein sehr wichtiger.

  Ausführlich: TESTCLIENT/README/ANLEITUNG.md
"
    );
}

/// Kürzel für den n-ten Eintrag einer erzeugten Liste.
///
/// Ziffern zuerst, danach Buchstaben. Ohne dieses Kürzel hätte ein Eintrag
/// jenseits des neunten nur den Weg über die Pfeiltasten — und der Weg
/// über die Tastenkürzel soll nicht ab dem zehnten Plan verschwinden.
fn kuerzel(i: usize) -> char {
    match i {
        0..=8 => char::from(b'1' + i as u8),
        9..=34 => char::from(b'a' + (i - 9) as u8),
        _ => ' ',
    }
}

/// Baut die Auswahlliste zu gefundenen Testplänen.
fn plan_punkte(gefunden: &[crate::plaene::Gefunden], abbruch: &str) -> Vec<Punkt> {
    let mut punkte: Vec<Punkt> = gefunden
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let zeile = crate::plaene::zeile(g);
            let (titel, hinweis) = zeile.split_once('\n').unwrap_or((zeile.as_str(), ""));
            Punkt::neu(kuerzel(i), titel, hinweis.trim())
        })
        .collect();
    punkte.push(Punkt::neu('0', abbruch, ""));
    punkte
}

/// Sucht Testpläne und lässt auswählen. `None` heißt „keiner", und dann
/// läuft alles wie ohne Plan.
///
/// Steht der Ordner leer oder fehlt er, wird nicht gefragt: Eine Frage mit
/// nur einer möglichen Antwort ist keine Frage, sondern eine Verzögerung.
fn plan_waehlen(
    repo: &std::path::Path,
    meldung: &mut dyn FnMut(String),
) -> Option<crate::spec::TestPlan> {
    let gefunden = crate::plaene::suchen(repo, meldung);
    if gefunden.is_empty() {
        return None;
    }

    let punkte = plan_punkte(&gefunden, "keiner — Einstellungen von Hand wählen");
    let wahl = auswahl::waehlen(
        &format!("Testpläne in {}", crate::plaene::ORDNER),
        &punkte,
    )?;
    let index = punkte.iter().position(|p| p.taste == wahl)?;
    if index >= gefunden.len() {
        println!("  Kein Plan gewählt.\n");
        return None;
    }
    Some(gefunden[index].plan.clone())
}

/// Die Entwickler-Ebene. Kehrt mit dem letzten Ergebnis zurück.
fn entwickler(e: &mut Einstellungen, mut letztes_ergebnis: bool) -> bool {
    loop {
        banner::bildschirm();
        e.zeigen();
        let Some(wahl) = auswahl::waehlen("Entwickler", &menue_entwickler()) else {
            return letztes_ergebnis;
        };
        println!();
        match wahl {
            '1' => letztes_ergebnis = starte("hardware", e, runs::run_hardware),
            '2' => {
                letztes_ergebnis = starte("determinismus", e, |log| {
                    runs::run_determinism(log, &e.artifacts, &e.prompts, e.steps)
                })
            }
            '3' => {
                letztes_ergebnis = starte("shard", e, |log| {
                    runs::run_shard(log, &e.artifacts, &e.prompts, e.steps, e.shards)
                })
            }
            '4' => letztes_ergebnis = starte("stack", e, stack::run_stack),
            '5' => artefakte_pruefen(),
            '6' => plan_erzeugen(e),
            '7' => einstellungen_aendern(e),
            '8' => e.teilnehmer = namen_erfragen(),
            '9' => freigeben(),
            '0' => return letztes_ergebnis,
            _ => {}
        }
        weiter();
    }
}

/// Legt das Protokoll für einen Lauf an.
fn protokoll(befehl: &str, e: &Einstellungen) -> RunLog {
    let hw = hardware::Fingerprint::collect().short_id();
    let ziel = LogZiel::neu(&e.logs, befehl, &e.teilnehmer, &e.einstellungen_id, &hw);
    RunLog::mit_ziel(ziel, true)
}

/// Führt einen einzelnen Lauf mit eigenem Protokoll aus.
///
/// Nur für das Entwickler-Menü. Im Nutzermodus gehören die Stufen in ein
/// gemeinsames Protokoll — siehe [`testlauf`].
fn starte(befehl: &str, e: &Einstellungen, f: impl FnOnce(&mut RunLog) -> bool) -> bool {
    let mut log = protokoll(befehl, e);
    let ok = f(&mut log);
    log.finish(ok)
}

/// Der vollständige Testlauf dieser Maschine: **ein** Protokoll, vier Stufen.
///
/// Hardware, Determinismus über die Einzelknoten-Runtime, geshardete
/// Inferenz und der Protokoll-Durchlauf gehören zu **einer** Messung.
/// Vier getrennte Protokolldateien wären vier Teilaussagen, die der
/// Koordinator erst wieder zusammensetzen müsste — und beim Verschicken
/// geht die eine verloren, die den Befund trägt. Der Fahrplan sagt es
/// kürzer: Ein Testlauf ohne Protokoll ist wertlos, und ein Testlauf mit
/// vier Protokollen ist einer zuviel.
///
/// Die Stufen laufen **alle**, auch wenn eine fehlschlägt: Ein
/// fehlgeschlagener Determinismuslauf macht die Hardware-Erhebung nicht
/// wertlos, sondern erst recht wichtig.
fn testlauf(e: &Einstellungen) -> bool {
    let mut log = protokoll("testlauf", e);

    log.note("Stufe 1 von 4: Hardware");
    let hardware = runs::run_hardware(&mut log);

    log.note("Stufe 2 von 4: Determinismus (Einzelknoten)");
    let determinismus = runs::run_determinism(&mut log, &e.artifacts, &e.prompts, e.steps);

    log.note("Stufe 3 von 4: Geshardete Inferenz");
    let shard = runs::run_shard(&mut log, &e.artifacts, &e.prompts, e.steps, e.shards);

    log.note("Stufe 4 von 4: Protokoll-Durchlauf");
    let stapel = stack::run_stack(&mut log);

    println!(
        "\n  Gesamt: Hardware {}, Determinismus {}, Shards {}, Stack {}",
        ja_nein(hardware),
        ja_nein(determinismus),
        ja_nein(shard),
        ja_nein(stapel)
    );
    log.finish(hardware && determinismus && shard && stapel)
}

/// Wartet auf einen Tastendruck, bevor der Bildschirm aufgeräumt wird.
///
/// **Der Gegenpart zum Aufräumen.** Ohne ihn verschwände die Ausgabe
/// eines Laufs in dem Augenblick, in dem sie fertig ist — der Nutzer sähe
/// das Ergebnis nie. Mit ihm bleibt sie stehen, solange er sie liest, und
/// er entscheidet, wann weitergegangen wird.
///
/// Ohne Terminal wird nicht gewartet: Ein Skript hat niemanden, der eine
/// Taste drückt, und würde stillstehen.
fn weiter() {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }
    println!("\n  ── Weiter mit einer beliebigen Taste ──");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    if let Ok(roh) = auswahl::Rohmodus::an() {
        loop {
            match crossterm::event::read() {
                Ok(crossterm::event::Event::Key(k))
                    if k.kind == crossterm::event::KeyEventKind::Press =>
                {
                    break
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        drop(roh);
    }
}

/// Menüpunkt [3]: Protokolle vergleichen.
///
/// **Zwei Quellen, und die Wahl gehört dem Nutzer.** Der Koordinator legt
/// die zugesandten Protokolle in `TESTCLIENT/Vergleiche/`; ein Teilnehmer
/// will dagegen die eigenen Läufe aus `logs/` gegenüberstellen. Beides in
/// einen Topf zu werfen wäre der schlechtere Weg: Ein Urteil über eine
/// Gruppe, in der die eigene Maschine mehrfach steckt, sagt etwas anderes
/// aus, als es zu sagen scheint.
///
/// Der Bericht landet in beiden Fällen unter `Vergleiche/Berichte/`.
fn vergleichen(e: &Einstellungen) -> bool {
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let zugesandt = crate::vergleich::vergleichsordner(&repo);
    let berichte = crate::vergleich::berichtsordner(&repo);

    let punkte = vec![
        Punkt::neu(
            '1',
            "Zugesandte Protokolle",
            &format!(
                "Was im Ordner {} liegt — der Weg des Koordinators.",
                crate::vergleich::ORDNER
            ),
        ),
        Punkt::neu(
            '2',
            "Eigene Läufe",
            "Die Protokolle dieser Maschine. Ergibt für sich keinen\n\
             Nachweis, zeigt aber, ob wiederholte Läufe übereinstimmen.",
        ),
        Punkt::neu('0', "Zurück", ""),
    ];

    match auswahl::waehlen("Welche Protokolle vergleichen?", &punkte) {
        Some('1') => crate::vergleich::run(&zugesandt, Some(&berichte)),
        Some('2') => crate::vergleich::run(&e.logs, Some(&berichte)),
        _ => true,
    }
}

/// Gibt Plattenplatz frei: Artefakte und heruntergeladene Gewichte.
///
/// ## Warum getrennt gefragt wird
///
/// Artefakte und Gewichte sind verschieden teuer wiederzubeschaffen.
/// Artefakte entstehen aus dem versionierten Skalenpaket in Sekunden; die
/// Gewichte kosten einen Download über mehrere Gigabyte. Wer Platz
/// braucht und den Test später wiederholen will, gibt deshalb die
/// Artefakte frei und behält die Gewichte. Ein einziger Punkt „alles
/// löschen" nähme ihm diese Wahl.
///
/// ## Warum eine getippte Bestätigung
///
/// Löschen ist der einzige Vorgang in diesem Client, der etwas zerstört.
/// Eine Auswahlliste, in der ein Pfeiltastendruck zuviel ein 15-GB-Modell
/// löscht, wäre die falsche Bedienung dafür. Verlangt wird deshalb ein
/// getipptes „ja" — die eine Stelle, an der Enter allein nicht genügt.
fn freigeben() {
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let belegung: Vec<crate::artefakte::Belegung> = crate::artefakte::belegung(&repo)
        .into_iter()
        .filter(|b| b.belegt())
        .collect();

    if belegung.is_empty() {
        println!("  Auf dieser Maschine liegen weder Artefakte noch Gewichte.");
        return;
    }

    let gesamt: u64 = belegung.iter().map(|b| b.bytes()).sum();
    println!("  Belegt auf dieser Maschine: {}\n", crate::artefakte::groesse(gesamt));

    let mut punkte = Vec::new();
    let mut ziele: Vec<(String, std::path::PathBuf)> = Vec::new();
    for b in &belegung {
        if let Some((pfad, bytes)) = &b.artefakte {
            punkte.push(Punkt::neu(
                kuerzel(ziele.len()),
                &format!(
                    "{} · Artefakte · {}",
                    b.modell,
                    crate::artefakte::groesse(*bytes)
                ),
                "Aus dem Skalenpaket in Sekunden wiederherstellbar.",
            ));
            ziele.push((format!("{} (Artefakte)", b.modell), pfad.clone()));
        }
        if let Some((pfad, bytes)) = &b.gewichte {
            punkte.push(Punkt::neu(
                kuerzel(ziele.len()),
                &format!(
                    "{} · Gewichte · {}",
                    b.modell,
                    crate::artefakte::groesse(*bytes)
                ),
                "Erneut zu holen kostet einen Download über Hugging Face.",
            ));
            ziele.push((format!("{} (Gewichte)", b.modell), pfad.clone()));
        }
    }
    punkte.push(Punkt::neu('0', "Nichts löschen", ""));

    let Some(wahl) = auswahl::waehlen("Was freigeben?", &punkte) else {
        return;
    };
    let Some(index) = punkte.iter().position(|p| p.taste == wahl) else {
        return;
    };
    if index >= ziele.len() {
        println!("  Nichts gelöscht.");
        return;
    }

    let (was, pfad) = &ziele[index];
    println!("\n  Löschen: {}", was);
    println!("  Pfad:    {}", pfad.display());
    println!("  Das lässt sich nicht rückgängig machen.\n");

    let antwort = auswahl::frage("  Zum Bestätigen \"ja\" eintippen: ").unwrap_or_default();
    if antwort.trim().to_lowercase() != "ja" {
        println!("\n  Abgebrochen, nichts gelöscht.");
        return;
    }

    match crate::artefakte::freigeben(&repo, pfad) {
        Ok(bytes) => println!("\n  {} freigegeben.", crate::artefakte::groesse(bytes)),
        Err(e) => println!("\n  {}", e),
    }
}

/// Prüft alle bekannten Modelle gegen den veröffentlichten Digest.
///
/// Ohne diese Prüfung sähe ein abweichendes Artefakt später aus wie eine
/// gescheiterte Hardware-Bitgleichheit, und der Client berichtete das
/// Gegenteil dessen, wofür es ihn gibt.
fn artefakte_pruefen() {
    use crate::artefakte::{pruefen, register, repo_wurzel, Zustand};
    let repo = repo_wurzel(std::env::current_dir().unwrap_or_default());
    match register(&repo) {
        Err(e) => println!("  Register nicht lesbar: {}", e),
        Ok(bekannt) => {
            for b in &bekannt {
                match pruefen(&repo, b) {
                    Zustand::Bereit { .. } => {
                        println!("  {} (θ_v {}): Digest stimmt, {}", b.name, b.theta_v, &b.digest[..16])
                    }
                    Zustand::Abweichend { ist, .. } => {
                        println!("  {}: DIGEST WEICHT AB, hier {}", b.name, &ist[..16]);
                        println!("     Das ist KEIN Hardware-Befund, hier liegt ein anderes Modell.");
                    }
                    Zustand::Fehlt => println!("  {}: keine Artefakte auf dieser Maschine.", b.name),
                }
            }
        }
    }
}

/// Testdatei wählen und anwenden, ohne sie gleich auszuführen.
///
/// Auswahl und Lauf sind getrennt: Punkt [2] stellt ein, Punkt [1]
/// misst. Wer beides in einen Punkt legt, nimmt dem Nutzer die
/// Möglichkeit, die Einstellungen vor dem Lauf noch anzusehen.
fn testdatei_waehlen(e: &mut Einstellungen) {
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let mut sagen = |t: String| println!("  {}", t);
    let gefunden = crate::plaene::suchen(&repo, &mut sagen);
    if gefunden.is_empty() {
        println!("  Keine Testdateien in {}.", crate::plaene::ORDNER);
        println!("  Der Koordinator schickt sie; leg sie dort ab.");
        return;
    }

    let punkte = plan_punkte(&gefunden, "keine, Einstellungen behalten");
    let Some(wahl) = auswahl::waehlen(
        &format!("Testdateien in {}", crate::plaene::ORDNER),
        &punkte,
    ) else {
        return;
    };
    let Some(index) = punkte.iter().position(|p| p.taste == wahl) else {
        return;
    };
    if index >= gefunden.len() {
        println!("  Keine Testdatei gewählt.");
        return;
    }

    let plan = gefunden[index].plan.clone();
    e.uebernehmen(&plan);
    println!(
        "  Plan \"{}\" übernommen: Modell {}, {} Token, {} Shards.",
        plan.plan_id, plan.model, plan.steps, plan.shards
    );

    let mut frage = |prompt: &str| -> Option<String> { auswahl::frage(&format!("  {}", prompt)) };
    let mut f: crate::artefakte::Rueckfrage = Some(&mut frage);
    match crate::artefakte::beschaffen_fuer(&repo, &plan.model, &mut f, &mut |t| println!("  {}", t)) {
        Ok(pfad) => {
            e.artifacts = pfad;
            println!("\n  Bereit. Mit [1] den Testlauf starten.");
        }
        Err(fehler) => {
            for zeile in fehler.lines() {
                println!("  {}", zeile);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nutzermenue_nennt_alle_punkte() {
        let tasten: Vec<char> = menue_nutzer().iter().map(|p| p.taste).collect();
        assert_eq!(tasten, vec!['1', '2', '3', '4', '9', '0']);
    }

    /// Das Nutzermenü darf nicht wieder anwachsen: Es ist die Seite, die
    /// ein Teilnehmer ohne Vorwissen zuerst sieht. Gezählt werden die
    /// Punkte, die etwas tun — „Entwickler-Menü" und „Beenden" sind Wege
    /// hinaus, keine Aufgaben.
    #[test]
    fn nutzermenue_bleibt_schlank() {
        let aufgaben = menue_nutzer()
            .iter()
            .filter(|p| p.taste != '0' && p.taste != '9')
            .count();
        assert!(
            aufgaben <= 5,
            "Nutzermenü hat {aufgaben} Aufgaben, höchstens 5 sind vorgesehen"
        );
    }

    /// Jede Taste darf nur einmal vorkommen — sonst wäre nicht bestimmt,
    /// welchen Punkt sie auslöst.
    #[test]
    fn tasten_sind_eindeutig() {
        for menue in [menue_nutzer(), menue_entwickler()] {
            let mut tasten: Vec<char> = menue.iter().map(|p| p.taste).collect();
            let vorher = tasten.len();
            tasten.sort_unstable();
            tasten.dedup();
            assert_eq!(tasten.len(), vorher, "doppelte Taste im Menü");
        }
    }

    #[test]
    fn entwicklermenue_nennt_alle_punkte() {
        let tasten: Vec<char> = menue_entwickler().iter().map(|p| p.taste).collect();
        assert_eq!(tasten, vec!['1', '2', '3', '4', '5', '6', '7', '8', '9', '0']);
    }

    /// Menüzeilen werden mit acht Zeichen Einzug gezeichnet; ein zu langer
    /// Titel bricht in 80 Spalten um und zerreißt die Liste.
    #[test]
    fn menuepunkte_passen_in_achtzig_spalten() {
        for menue in [menue_nutzer(), menue_entwickler()] {
            for p in menue {
                assert!(
                    p.titel.chars().count() + 8 <= 78,
                    "Titel zu breit: {}",
                    p.titel
                );
                for z in p.hinweis.lines() {
                    assert!(z.chars().count() + 8 <= 78, "Hinweis zu breit: {}", z);
                }
            }
        }
    }

    /// Die Kürzel einer erzeugten Liste müssen über den neunten Eintrag
    /// hinaus eindeutig bleiben.
    #[test]
    fn kuerzel_bleiben_eindeutig() {
        let liste: Vec<char> = (0..35).map(kuerzel).collect();
        let mut sortiert = liste.clone();
        sortiert.sort_unstable();
        sortiert.dedup();
        assert_eq!(sortiert.len(), liste.len());
        assert_eq!(kuerzel(0), '1');
        assert_eq!(kuerzel(8), '9');
        assert_eq!(kuerzel(9), 'a');
        // Jenseits des Vorrats gibt es kein Kürzel, aber auch keine Panik.
        assert_eq!(kuerzel(99), ' ');
    }

    #[test]
    fn pfade_werden_gekuerzt() {
        let lang = PathBuf::from("/a/b/c/d/e/f");
        assert_eq!(kurz(&lang), "…/d/e/f");
        let kurzer = PathBuf::from("a/b");
        assert_eq!(kurz(&kurzer), "a/b");
    }

    #[test]
    fn ja_nein_ist_eindeutig() {
        assert_eq!(ja_nein(true), "OK");
        assert_eq!(ja_nein(false), "FEHLGESCHLAGEN");
    }
}
