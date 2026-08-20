//! Interaktives Menü — „Durchklicken" statt Befehle tippen.
//!
//! Wird gestartet, wenn `myl-test` **ohne Unterbefehl** aufgerufen wird.
//! Der Grund: Die Hardwaretests laufen auf fremden Maschinen, oft von
//! Leuten, die das Projekt nicht täglich sehen. Wer erst eine Hilfeseite
//! lesen muss, um einen Testlauf zu starten, führt ihn seltener aus.
//!
//! Bewusst nur Ziffernauswahl und `stdin`, keine TUI-Bibliothek: Der
//! Client soll über SSH, in einer seriellen Konsole und in einem
//! Container ohne Terminfo-Datenbank funktionieren. Alles, was
//! Cursorsteuerung oder Rohmodus braucht, scheidet damit aus.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use crate::logging::RunLog;
use crate::logging::LogZiel;
use crate::spec::TestPlan;
use crate::{banner, hardware, runs, stack};

/// Laufeinstellungen, die im Menü verändert werden können.
pub struct Einstellungen {
    pub prompt: String,
    pub steps: usize,
    pub shards: usize,
    pub artifacts: PathBuf,
    pub logs: PathBuf,
    /// Kurzkennung der Einstellungen — benennt das Protokollverzeichnis.
    /// `ohne-plan`, solange kein Testplan geladen wurde.
    pub einstellungen_id: String,
}

impl Einstellungen {
    /// Baut den zu den aktuellen Werten passenden Testplan.
    fn als_plan(&self) -> TestPlan {
        TestPlan {
            plan_id: "unbenannt".to_string(),
            prompt: self.prompt.clone(),
            steps: self.steps,
            shards: self.shards,
            model: crate::runs::DEFAULT_MODEL.to_string(),
        }
    }

    /// Übernimmt einen geladenen Plan.
    fn uebernehmen(&mut self, plan: &TestPlan) {
        self.prompt = plan.prompt.clone();
        self.steps = plan.steps;
        self.shards = plan.shards;
        self.einstellungen_id = plan.short_id();
    }
}

impl Einstellungen {
    fn zeigen(&self) {
        println!("  Aktuelle Einstellungen:");
        println!("    Prompt      {:?}", self.prompt);
        println!("    Token       {}", self.steps);
        println!("    Shards      {}", self.shards);
        println!("    Artefakte   {}", kurz(&self.artifacts));
        println!("    Protokolle  {}", kurz(&self.logs));
        println!("    Einstellungs-ID {}", self.einstellungen_id);
    }
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
/// Fünf Punkte statt zehn. Wer eine Maschine beisteuert, soll messen und
/// das Ergebnis schicken; er soll keine Testpläne erzeugen und keine
/// Pfade umstellen. Alles Weitere liegt eine Ebene tiefer unter [9].
const MENUE: &str = "\
  ── Was möchtest du tun? ────────────────────────────────────

   1  Testlauf starten
      Hardware, Determinismus, Shards und Protokoll-Durchlauf
      nacheinander. Der vollständige Bericht dieser Maschine.

   2  Testdatei wählen
      Legt Prompt, Token, Shards und Modell fest. Beschafft das
      Modell, falls es fehlt. Danach mit [1] starten.

   3  Anleitung lesen

   9  Entwickler-Menü

   0  Beenden
";

/// Das Entwickler-Menü: Einzelläufe und alles, was Vorwissen braucht.
///
/// Getrennt vom Nutzermenü, weil eine lange Liste den Teilnehmer bremst
/// und die Punkte, die er versehentlich wählt, ihm nichts nützen. Wer
/// hier hereinkommt, weiß in der Regel, was er sucht.
const MENUE_ENTWICKLER: &str = "\
  ── Entwickler ──────────────────────────────────────────────

   1  Hardware erheben
   2  Determinismus prüfen
   3  Geshardete Inferenz
   4  Protokoll-Durchlauf (Stack)
   5  Artefakte prüfen (Digest gegen das Register)
   6  Testplan erzeugen und speichern
   7  Einstellungen ändern (Prompt, Token, Shards, Pfade)

   0  Zurück
";

/// Startet das Menü und kehrt mit dem Gesamtergebnis zurück.
pub fn run(mut e: Einstellungen) -> bool {
    banner::print_if(true);
    println!(
        "  Kein Unterbefehl angegeben — interaktiver Modus.\n  \
         (Für Skripte: `myl-test --help` zeigt die Befehle.)\n"
    );

    // Reihenfolge beim Start: erst Testplan, dann Artefakt.
    //
    // Ein Plan legt das Modell fest. Wer zuerst nach dem Artefakt fragt und
    // danach den Plan lädt, hat entweder die falsche Frage gestellt oder
    // muss sie zurücknehmen. Ist ein Plan gewählt, ergibt sich das Artefakt
    // aus ihm, und es gibt nichts mehr zu fragen.
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let mut sagen = |t: String| println!("  {}", t);
    let plan = plan_waehlen(&repo, &mut sagen);

    let mut frage = |prompt: &str| -> Option<String> {
        print!("  {}", prompt);
        let _ = io::stdout().flush();
        zeilen_lesen()
    };
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

    // Ist ein Plan gewählt und das Artefakt bereit, läuft der Durchgang
    // sofort: Determinismus und Shard-Lauf, in dieser Reihenfolge. Wer
    // einen Plan auswählt, will messen, nicht noch ein Menü bedienen.
    let mut letztes_ergebnis = true;
    if plan.is_some() && e.artifacts.exists() {
        println!("  Plan wird ausgeführt.\n");
        letztes_ergebnis = starte("determinismus", &e, |log| {
            runs::run_determinism(log, &e.artifacts, &e.prompt, e.steps)
        });
        letztes_ergebnis &= starte("shard", &e, |log| {
            runs::run_shard(log, &e.artifacts, &e.prompt, e.steps, e.shards)
        });
        println!("\n  Durchgang beendet. Das Menü steht für weitere Läufe bereit.\n");
    }

    let stdin = io::stdin();
    let mut zeilen = stdin.lock().lines();

    loop {
        println!("{}", MENUE);
        e.zeigen();
        print!("\n  Auswahl [0-3, 9]: ");
        let _ = io::stdout().flush();

        let Some(Ok(eingabe)) = zeilen.next() else {
            println!("\n  Eingabe beendet.");
            return letztes_ergebnis;
        };

        println!();
        match eingabe.trim() {
            "1" => {
                let h = starte("hardware", &e, runs::run_hardware);
                let a = starte("determinismus", &e, |log| {
                    runs::run_determinism(log, &e.artifacts, &e.prompt, e.steps)
                });
                let b = starte("shard", &e, |log| {
                    runs::run_shard(log, &e.artifacts, &e.prompt, e.steps, e.shards)
                });
                let c = starte("stack", &e, stack::run_stack);
                letztes_ergebnis = h && a && b && c;
                println!(
                    "\n  Gesamt: Hardware {}, Determinismus {}, Shards {}, Stack {}",
                    ja_nein(h),
                    ja_nein(a),
                    ja_nein(b),
                    ja_nein(c)
                );
            }
            "2" => testdatei_waehlen(&mut e, &mut zeilen),
            "3" => anleitung_zeigen(),
            "9" => letztes_ergebnis = entwickler(&mut e, &mut zeilen, letztes_ergebnis),
            "0" | "q" | "quit" | "exit" => {
                println!("  Fertig.");
                return letztes_ergebnis;
            }
            "" => {}
            sonst => println!("  Unbekannte Auswahl: {:?}", sonst),
        }
        println!();
    }
}

fn ja_nein(b: bool) -> &'static str {
    if b {
        "OK"
    } else {
        "FEHLGESCHLAGEN"
    }
}

/// Menüpunkt 9: einen Testplan erzeugen und verteilen.
fn plan_erzeugen(e: &Einstellungen, zeilen: &mut impl Iterator<Item = io::Result<String>>) {
    print!("  Kennung des Durchgangs (z. B. 2026-08-18-cross-arch-01): ");
    let _ = io::stdout().flush();
    let Some(Ok(kennung)) = zeilen.next() else { return };
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

fn einstellungen_aendern(
    e: &mut Einstellungen,
    zeilen: &mut impl Iterator<Item = io::Result<String>>,
) {
    println!("  ── Einstellungen ─────────────────────────────────────────");
    println!("   1 Prompt   2 Token   3 Shards   4 Artefakte   5 Protokolle   0 zurück");
    print!("\n  Auswahl [0-5]: ");
    let _ = io::stdout().flush();

    let Some(Ok(wahl)) = zeilen.next() else { return };
    let frage = |text: &str, zeilen: &mut dyn Iterator<Item = io::Result<String>>| -> Option<String> {
        print!("  {}: ", text);
        let _ = io::stdout().flush();
        match zeilen.next() {
            Some(Ok(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
            _ => None,
        }
    };

    match wahl.trim() {
        "1" => {
            if let Some(v) = frage("Neuer Prompt", zeilen) {
                e.prompt = v;
            }
        }
        "2" => {
            if let Some(v) = frage("Anzahl Token", zeilen) {
                match v.parse::<usize>() {
                    Ok(n) if n > 0 => e.steps = n,
                    _ => println!("  Ungültig — unverändert."),
                }
            }
        }
        "3" => {
            if let Some(v) = frage("Anzahl Shards", zeilen) {
                match v.parse::<usize>() {
                    Ok(n) if n > 0 => e.shards = n,
                    _ => println!("  Ungültig — unverändert."),
                }
            }
        }
        "4" => {
            if let Some(v) = frage("Artefaktverzeichnis", zeilen) {
                e.artifacts = PathBuf::from(v);
            }
        }
        "5" => {
            if let Some(v) = frage("Protokollverzeichnis", zeilen) {
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

/// Liest eine Zeile von der Standardeingabe.
///
/// **Muss vor `stdin.lock()` der Menüschleife laufen.** Ein zweiter
/// `lock()` auf dieselbe Quelle blockiert, solange der erste gehalten
/// wird — beim ersten Versuch stand die Beschaffung nach der Sperre und
/// der Client hing stumm. Deshalb steht sie jetzt davor, und deshalb
/// steht dieser Satz hier: Der Fehler ist beim Lesen des Codes nicht
/// sichtbar, nur beim Ausführen.
fn zeilen_lesen() -> Option<String> {
    let mut zeile = String::new();
    match io::stdin().read_line(&mut zeile) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(zeile),
    }
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

    println!("  Testpläne in {}:", crate::plaene::ORDNER);
    for (i, g) in gefunden.iter().enumerate() {
        println!("   [{}] {}", i + 1, crate::plaene::zeile(g));
    }
    println!("   [0] keiner — Einstellungen von Hand wählen");
    print!("\n  Auswahl [0]: ");
    let _ = io::stdout().flush();

    let eingabe = zeilen_lesen().unwrap_or_default();
    let wahl: usize = eingabe.trim().parse().unwrap_or(0);
    if wahl == 0 || wahl > gefunden.len() {
        println!("  Kein Plan gewählt.\n");
        return None;
    }
    Some(gefunden[wahl - 1].plan.clone())
}

/// Die Entwickler-Ebene. Kehrt mit dem letzten Ergebnis zurück.
fn entwickler(
    e: &mut Einstellungen,
    zeilen: &mut impl Iterator<Item = io::Result<String>>,
    mut letztes_ergebnis: bool,
) -> bool {
    loop {
        println!("{}", MENUE_ENTWICKLER);
        e.zeigen();
        print!("\n  Auswahl [0-7]: ");
        let _ = io::stdout().flush();

        let Some(Ok(eingabe)) = zeilen.next() else {
            return letztes_ergebnis;
        };
        println!();
        match eingabe.trim() {
            "1" => letztes_ergebnis = starte("hardware", e, runs::run_hardware),
            "2" => {
                letztes_ergebnis = starte("determinismus", e, |log| {
                    runs::run_determinism(log, &e.artifacts, &e.prompt, e.steps)
                })
            }
            "3" => {
                letztes_ergebnis = starte("shard", e, |log| {
                    runs::run_shard(log, &e.artifacts, &e.prompt, e.steps, e.shards)
                })
            }
            "4" => letztes_ergebnis = starte("stack", e, stack::run_stack),
            "5" => artefakte_pruefen(),
            "6" => plan_erzeugen(e, zeilen),
            "7" => einstellungen_aendern(e, zeilen),
            "0" | "q" | "zurueck" | "zurück" => return letztes_ergebnis,
            "" => {}
            sonst => println!("  Unbekannte Auswahl: {:?}", sonst),
        }
    }
}

/// Führt einen Lauf mit eigenem Protokoll aus.
fn starte(befehl: &str, e: &Einstellungen, f: impl FnOnce(&mut RunLog) -> bool) -> bool {
    let hw = hardware::Fingerprint::collect().short_id();
    let ziel = LogZiel::neu(&e.logs, befehl, &e.einstellungen_id, &hw);
    let mut log = RunLog::mit_ziel(ziel, true);
    let ok = f(&mut log);
    log.finish(ok)
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
fn testdatei_waehlen(
    e: &mut Einstellungen,
    zeilen: &mut impl Iterator<Item = io::Result<String>>,
) {
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let mut sagen = |t: String| println!("  {}", t);
    let gefunden = crate::plaene::suchen(&repo, &mut sagen);
    if gefunden.is_empty() {
        println!("  Keine Testdateien in {}.", crate::plaene::ORDNER);
        println!("  Der Koordinator schickt sie; leg sie dort ab.");
        return;
    }

    println!("  Testdateien in {}:", crate::plaene::ORDNER);
    for (i, g) in gefunden.iter().enumerate() {
        println!("   [{}] {}", i + 1, crate::plaene::zeile(g));
    }
    println!("   [0] keine, Einstellungen behalten");
    print!("\n  Auswahl [0]: ");
    let _ = io::stdout().flush();

    let Some(Ok(eingabe)) = zeilen.next() else { return };
    let wahl: usize = eingabe.trim().parse().unwrap_or(0);
    if wahl == 0 || wahl > gefunden.len() {
        println!("  Keine Testdatei gewählt.");
        return;
    }

    let plan = gefunden[wahl - 1].plan.clone();
    e.uebernehmen(&plan);
    println!(
        "  Plan \"{}\" übernommen: Modell {}, {} Token, {} Shards.",
        plan.plan_id, plan.model, plan.steps, plan.shards
    );

    let mut frage = |prompt: &str| -> Option<String> {
        print!("  {}", prompt);
        let _ = io::stdout().flush();
        zeilen.next().and_then(|r| r.ok())
    };
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
        for punkt in ["1", "2", "3", "9", "0"] {
            assert!(
                MENUE.contains(&format!("   {}  ", punkt)),
                "Menüpunkt {} fehlt",
                punkt
            );
        }
    }

    /// Das Nutzermenü darf nicht wieder anwachsen: Es ist die Seite, die
    /// ein Teilnehmer ohne Vorwissen zuerst sieht.
    #[test]
    fn nutzermenue_bleibt_schlank() {
        let punkte = MENUE
            .lines()
            .filter(|z| z.trim_start().starts_with(char::is_numeric) && z.contains("  "))
            .count();
        assert!(punkte <= 5, "Nutzermenü hat {punkte} Punkte, höchstens 5 sind vorgesehen");
    }

    #[test]
    fn entwicklermenue_nennt_alle_punkte() {
        for punkt in ["1", "2", "3", "4", "5", "6", "7", "0"] {
            assert!(
                MENUE_ENTWICKLER.contains(&format!("   {}  ", punkt)),
                "Entwicklerpunkt {} fehlt",
                punkt
            );
        }
    }

    #[test]
    fn entwicklermenue_passt_in_achtzig_spalten() {
        for (i, z) in MENUE_ENTWICKLER.lines().enumerate() {
            assert!(z.chars().count() <= 78, "Zeile {} zu breit", i + 1);
        }
    }

    #[test]
    fn menue_passt_in_achtzig_spalten() {
        for (i, z) in MENUE.lines().enumerate() {
            assert!(z.chars().count() <= 78, "Menüzeile {} zu breit", i + 1);
        }
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
