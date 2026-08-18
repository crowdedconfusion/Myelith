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

const MENUE: &str = "\
  ── Was möchtest du prüfen? ─────────────────────────────────

   1  Hardware erheben
      Kein Modell nötig. Der erste Schritt auf einer neuen
      Maschine — liefert den Fingerabdruck für den Vergleich.

   2  Determinismus prüfen
      Denselben Prompt zweimal rechnen. Der Digest muss auf
      JEDER Maschine derselbe sein. Braucht Artefakte.

   3  Geshardete Inferenz
      Modell über mehrere Shards fahren und gegen die
      Einzelknoten-Runtime prüfen. Braucht Artefakte.

   4  Protokoll-Durchlauf (Stack)
      Krypto, Epochenseed, Komiteewahl, BFT, Verifikation,
      Ledger, Tokenomics. Kein Modell nötig, ~1 Sekunde.

   5  Alles nacheinander
      2, 3 und 4 in einem Rutsch. Für den vollen Bericht
      einer Maschine.

   6  Einstellungen ändern (Prompt, Token, Shards, Pfade)

   7  Anleitung für Tests auf mehreren Maschinen

   8  Testplan laden (vom Koordinator erhalten)

   9  Testplan erzeugen und speichern (als Koordinator)

   0  Beenden
";

/// Startet das Menü und kehrt mit dem Gesamtergebnis zurück.
pub fn run(mut e: Einstellungen) -> bool {
    banner::print_if(true);
    println!(
        "  Kein Unterbefehl angegeben — interaktiver Modus.\n  \
         (Für Skripte: `myl-test --help` zeigt die Befehle.)\n"
    );

    let stdin = io::stdin();
    let mut zeilen = stdin.lock().lines();
    let mut letztes_ergebnis = true;

    loop {
        println!("{}", MENUE);
        e.zeigen();
        print!("\n  Auswahl [0-9]: ");
        let _ = io::stdout().flush();

        let Some(Ok(eingabe)) = zeilen.next() else {
            println!("\n  Eingabe beendet.");
            return letztes_ergebnis;
        };

        println!();
        match eingabe.trim() {
            "1" => letztes_ergebnis = starte("hardware", &e, runs::run_hardware),
            "2" => {
                letztes_ergebnis = starte("determinismus", &e, |log| {
                    runs::run_determinism(log, &e.artifacts, &e.prompt, e.steps)
                })
            }
            "3" => {
                letztes_ergebnis = starte("shard", &e, |log| {
                    runs::run_shard(log, &e.artifacts, &e.prompt, e.steps, e.shards)
                })
            }
            "4" => letztes_ergebnis = starte("stack", &e, stack::run_stack),
            "5" => {
                let a = starte("determinismus", &e, |log| {
                    runs::run_determinism(log, &e.artifacts, &e.prompt, e.steps)
                });
                let b = starte("shard", &e, |log| {
                    runs::run_shard(log, &e.artifacts, &e.prompt, e.steps, e.shards)
                });
                let c = starte("stack", &e, stack::run_stack);
                letztes_ergebnis = a && b && c;
                println!(
                    "\n  Gesamt: Determinismus {}, Shards {}, Stack {}",
                    ja_nein(a),
                    ja_nein(b),
                    ja_nein(c)
                );
            }
            "6" => einstellungen_aendern(&mut e, &mut zeilen),
            "7" => anleitung_zeigen(),
            "8" => plan_laden(&mut e, &mut zeilen),
            "9" => plan_erzeugen(&e, &mut zeilen),
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

/// Führt einen Lauf mit eigenem Protokoll aus.
fn starte(befehl: &str, e: &Einstellungen, f: impl FnOnce(&mut RunLog) -> bool) -> bool {
    let hw = hardware::Fingerprint::collect().short_id();
    let ziel = LogZiel::neu(&e.logs, befehl, &e.einstellungen_id, &hw);
    let mut log = RunLog::mit_ziel(ziel, true);
    let ok = f(&mut log);
    log.finish(ok)
}

/// Menüpunkt 8: einen erhaltenen Testplan laden.
fn plan_laden(e: &mut Einstellungen, zeilen: &mut impl Iterator<Item = io::Result<String>>) {
    print!("  Pfad zur Plandatei: ");
    let _ = io::stdout().flush();
    let Some(Ok(pfad)) = zeilen.next() else { return };
    let pfad = pfad.trim();
    if pfad.is_empty() {
        return;
    }
    match TestPlan::load(std::path::Path::new(pfad)) {
        Ok(plan) => {
            println!("\n  Plan geladen: {}", plan.plan_id);
            println!("    Prompt          {:?}", plan.prompt);
            println!("    Token           {}", plan.steps);
            println!("    Shards          {}", plan.shards);
            println!("    Modell          {}", plan.model);
            println!("    Einstellungs-ID {}", plan.short_id());
            println!("\n  Ab jetzt laufen alle Prüfungen mit diesen Werten.");
            e.uebernehmen(&plan);
        }
        Err(err) => println!("\n  {}", err),
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
    let ziel = std::path::PathBuf::from(format!("{}.plan", kennung));

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menue_nennt_alle_punkte() {
        for punkt in ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"] {
            assert!(
                MENUE.contains(&format!("   {}  ", punkt)),
                "Menüpunkt {} fehlt",
                punkt
            );
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
