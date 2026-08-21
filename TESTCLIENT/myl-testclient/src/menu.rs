//! Interaktives Menü: „Durchklicken" statt Befehle tippen.
//!
//! Wird gestartet, wenn `myl-test` **ohne Unterbefehl** aufgerufen wird.
//! Der Grund: Die Hardwaretests laufen auf fremden Maschinen, oft von
//! Leuten, die das Projekt nicht täglich sehen. Wer erst eine Hilfeseite
//! lesen muss, um einen Testlauf zu starten, führt ihn seltener aus.
//!
//! Ausgewählt wird mit den Pfeiltasten und Enter; die Ziffer daneben
//! bleibt gültig. Wo kein Terminal vorhanden ist: in einer Pipe, in
//! einem Skript, in einer seriellen Konsole ohne Rohmodus , fällt die
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
    /// Kurzkennung der Einstellungen: benennt das Protokollverzeichnis.
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
    /// Die aktuellen Einstellungen als Textblock.
    ///
    /// Als Zeichenkette und nicht gedruckt, weil der Block als **Fuß**
    /// der Menüauswahl gezeichnet wird und in deren Höhenrechnung eingehen
    /// muss. Gedruckt stünde er über dem Menü, und dort beantwortet er
    /// eine Frage, die noch niemand gestellt hat.
    fn als_text(&self) -> String {
        use std::fmt::Write as _;
        let mut t = String::new();
        let _ = writeln!(t, "  Aktuelle Einstellungen:");
        match self.prompts.as_slice() {
            [einer] => {
                let _ = writeln!(t, "    Prompt      {:?}", gekuerzt(einer));
            }
            viele => {
                let _ = writeln!(t, "    Prompts     {}", viele.len());
                for (i, p) in viele.iter().enumerate() {
                    let _ = writeln!(t, "      {}. {:?}", i + 1, gekuerzt(p));
                }
            }
        }
        let _ = writeln!(t, "    Token       {}", self.steps);
        let _ = writeln!(t, "    Shards      {}", self.shards);
        let _ = writeln!(t, "    Artefakte   {}", kurz(&self.artifacts));
        let _ = writeln!(t, "    Protokolle  {}", kurz(&self.logs));
        let _ = writeln!(t, "    Nutzer      {}", self.teilnehmer);
        let _ = write!(t, "    Einstellungs-ID {}", self.einstellungen_id);
        t
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
    // **Das Trennzeichen der Plattform, auch im Auslassungszeichen.**
    //
    // Vorher stand hier ein festes `…/`. Der Rest entsteht dagegen aus
    // `PathBuf::collect`, und das setzt das Trennzeichen des Systems.
    // Unter Windows kam deshalb `…/d\e\f` heraus, also beide Zeichen in
    // einer Zeile. Gefunden hat es der Windows-Job der CI, beim ersten
    // Lauf, den es ihn je gab.
    //
    // Entschieden für die Plattform und gegen einen festen Schrägstrich,
    // weil dieser Pfad **nur angezeigt** wird (Einstellungsblock im Menü)
    // und nie in ein Protokoll wandert. Wer unter Windows arbeitet, soll
    // seine eigene Schreibweise lesen. Ginge er ins Protokoll, wäre die
    // Antwort umgekehrt: Dort zählt die Vergleichbarkeit zwischen
    // Maschinen mehr als die Gewohnheit auf einer.
    format!("…{}{}", std::path::MAIN_SEPARATOR, rest.display())
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
            "Mit dem Modell sprechen",
            "Freie Eingabe, das Artefakt antwortet. Zum Ansehen, nicht zum\n\
             Messen: kein Protokoll, kein Vergleichswert.",
        ),
        Punkt::neu(
            '2',
            "Testlauf starten",
            "Hardware, Determinismus, Shards und Protokoll-Durchlauf\n\
             nacheinander. Der vollständige Bericht dieser Maschine.",
        ),
        Punkt::neu(
            '3',
            "Testdatei wählen",
            "Legt Prompt, Token, Shards und Modell fest. Punkt [2] fragt\n\
             sie ohnehin ab; hier vorab, wenn du sie vorher sehen willst.",
        ),
        Punkt::neu(
            '4',
            "Artefakt wählen",
            "Das Modell, mit dem gerechnet wird. Beschafft es, falls es\n\
             fehlt, und prüft den Digest gegen das Register.",
        ),
        Punkt::neu('5', "Anleitung lesen", ""),
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
        Punkt::neu('v', "Protokolle vergleichen und Bericht schreiben", ""),
        Punkt::neu('0', "Zurück", ""),
    ]
}

/// Startet das Menü und kehrt mit dem Gesamtergebnis zurück.
pub fn run(mut e: Einstellungen) -> bool {
    // Erstes Bild: Animation, dann Logo und Namenseingabe, sonst nichts.
    //
    // Der Name steht vor allem anderen, weil er jede Protokolldatei
    // benennt, die in dieser Sitzung entsteht: nachträglich umbenennen
    // müsste ihn sonst der Koordinator.
    // Das Farbschema der Sitzung wird hier zum ersten Mal abgerufen und
    // damit gewürfelt: während das Logo aus der Spirale entsteht.
    let farbe = crate::farben::logo();
    banner::start_if_mit(true, farbe);
    if e.teilnehmer == crate::logging::OHNE_NAME {
        e.teilnehmer = namen_erfragen();
    }
    // Aufräumen, Logo stehen lassen, dann die Begrüßung darunter. Die
    // Eingabezeile mit dem getippten Namen hat ihren Zweck erfüllt und
    // stünde sonst über der Antwort darauf.
    banner::bildschirm_mit(farbe);
    crate::animation::begruessung(&e.teilnehmer, farbe);

    // Zweites Bild: das Menü.
    //
    // **Kein Testplan und kein Artefakt beim Start.** Bis v0.6.0 lief nach
    // dem Namen erst die Planauswahl und danach die Artefaktbeschaffung,
    // bevor überhaupt ein Menü erschien. Wer den Client zum ersten Mal
    // öffnete, musste also zwei Entscheidungen treffen, die er noch nicht
    // einordnen konnte, und eine davon zog bis zu 15 GB Download nach
    // sich. Der Testplan gehört an die Stelle, an der er gebraucht wird:
    // beim Testlauf. Das Artefakt ist ein eigener Menüpunkt.
    //
    // Übernommen wird beim Start nur, was ohnehin gebaut ist.
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    banner::bildschirm();
    if let Some(pfad) = crate::artefakte::vorhandenes(&repo, crate::runs::DEFAULT_MODEL)
    {
        e.artifacts = pfad;
    }

    let mut letztes_ergebnis = true;

    loop {
        // Die Einstellungen stehen **unter** dem Menü: Zuerst die Frage,
        // was man tun will, dann der Zustand, unter dem es geschieht. Als
        // Fuß der Auswahl, nicht als eigener Druck davor, weil die Liste
        // sich bei jedem Tastendruck neu zeichnet und dabei alles unter
        // sich löscht (siehe `auswahl::waehlen_mit_fuss`).
        let Some(wahl) =
            auswahl::waehlen_mit_fuss("Was möchtest du tun?", &menue_nutzer(), &e.als_text())
        else {
            println!("\n  Fertig.");
            return letztes_ergebnis;
        };

        println!();
        match wahl {
            '1' => {
                sprechen(&e);
                weiter();
            }
            '2' => {
                // Die Planauswahl steht hier, nicht beim Start: Sie legt
                // fest, WOMIT gemessen wird, und diese Frage stellt sich
                // in dem Augenblick, in dem gemessen werden soll.
                testdatei_waehlen(&mut e);
                if e.artifacts.exists() {
                    println!();
                    letztes_ergebnis = testlauf(&e);
                }
                weiter();
            }
            '3' => {
                testdatei_waehlen(&mut e);
                weiter();
            }
            '4' => {
                artefakt_waehlen(&mut e);
                weiter();
            }
            '5' => {
                anleitung_zeigen();
                weiter();
            }
            // Das Entwicklermenü räumt beim Verlassen selbst auf; ein
            // `weiter()` hier verlangte einen Tastendruck für nichts.
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
/// Maschine, die steht ohnehin gemessen im Protokoll. Wer den Namen
/// leer lässt, bekommt `ohne-name`: sichtbar fehlend statt stillschweigend
/// geraten. Ein aus der Umgebung übernommener Benutzername wäre bequemer
/// und zugleich eine Personenangabe, die niemand angeordnet hat, und
/// Protokolle wandern per Copy-Paste in Tickets.
fn namen_erfragen() -> String {
    // Der Text steht als Block mittig unter dem Schriftzug, die
    // Eingabezeile am linken Rand dieses Blocks. Nur so bleibt sie dort,
    // wo das Auge sie nach dem Lesen erwartet; zentriert stünde der Cursor
    // je nach getipptem Namen an einer anderen Stelle.
    let text = "  Unter welchem Nutzernamen sollen die Protokolle dieser Sitzung laufen?\n  \
                Er steht im Dateinamen und im Protokoll, damit der Koordinator sie\n  \
                ohne Rückfrage zuordnen kann. Leer lassen ist erlaubt.";
    println!("{}\n", banner::zentriert(text));

    let einzug = banner::blockeinzug(
        text.lines().map(|z| z.chars().count()).max().unwrap_or(0),
    );
    let eingabe = auswahl::frage(&format!("{}  Nutzername: ", einzug)).unwrap_or_default();
    let name = eingabe.trim();
    if name.is_empty() {
        println!(
            "\n{}\n",
            banner::zentriert(&format!(
                "  Kein Name, die Protokolle laufen unter {:?}.",
                crate::logging::OHNE_NAME
            ))
        );
        return crate::logging::OHNE_NAME.to_string();
    }
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
            println!("  Sie laden sie im Menü über [3] Testdatei wählen,");
            println!("  über [2] Testlauf starten oder mit");
            println!("      myl-test --plan {} determinismus", ziel.display());
            println!("\n  Ihre Protokolle tragen dann die Einstellungs-Kennung");
            println!("      <nutzer>_{}_<datum>_<uhrzeit>.jsonl", plan.short_id());
            println!("  und liegen in TESTCLIENT/logs.");
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
                    _ => println!("  Ungültig: unverändert."),
                }
            }
        }
        '3' => {
            if let Some(v) = nachfragen("Anzahl Shards") {
                match v.parse::<usize>() {
                    Ok(n) if n > 0 => e.shards = n,
                    _ => println!("  Ungültig: unverändert."),
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

/// Die Kurzanleitung, wie sie Menüpunkt [5] zeigt.
///
/// **Nach Rollen geordnet, nicht nach Menüpunkten.** Wer den Client
/// startet, ist entweder Teilnehmer oder Koordinator, und die beiden
/// brauchen verschiedene Hälften. Eine Liste aller Punkte in ihrer
/// Reihenfolge stünde dagegen schon im Menü darüber.
///
/// **Sie muss auf den Bildschirm passen.** Reicht sie über das Fenster
/// hinaus, scrollt das Logo nach oben weg, und genau das soll der
/// aufgeräumte Bildschirm verhindern. Ein Test rechnet die Höhe gegen
/// Banner und Fußzeile.
///
/// Für alles Weitere steht die ausführliche Anleitung im Repository. Hier
/// steht nur, was jemand braucht, der gerade vor dem Menü sitzt.
const KURZANLEITUNG: &str = "\
  ── Kurzanleitung ─────────────────────────────────────────────

  Als Teilnehmer
    [3] Testdatei wählen, die der Koordinator geschickt hat.
        Sie gehört nach TESTCLIENT/Testpläne/.
    [4] Artefakt beschaffen, falls noch keines vorliegt.
    [2] Testlauf starten: vier Stufen, ein Protokoll.
        Danach beide Dateien aus TESTCLIENT/logs/ verschicken.
        Prompttexte stehen nicht darin, nur deren Hash.

  Als Koordinator
    [9] Entwickler, dort \"Testplan erzeugen\", .plan an alle geben.
        Zugesandte Protokolle nach TESTCLIENT/Vergleiche/ legen,
        dann [9], \"Protokolle vergleichen\". Der ausführliche
        Bericht landet in Vergleiche/Berichte/.

  Ein Nachweis braucht ZWEI Aussagen: Die Maschinen sind verschieden
  UND das Ergebnis ist gleich. Gleiche Werte von derselben Maschine
  belegen nichts, und der Vergleich verweigert dort ein Urteil.

  Ausführlich: TESTCLIENT/README/ANLEITUNG.md";

fn anleitung_zeigen() {
    // **Eigener Bildschirm, nicht unter dem Menü.** Wenn dieser Punkt
    // gewählt wird, stehen Banner, Menü und Einstellungen bereits da, und
    // das sind bei 120 x 44 schon 42 Zeilen. Die Anleitung darunter
    // schöbe das Logo nach oben aus dem Bild.
    //
    // Gemessen war es genau so: 59 Zeilen in einem Fenster mit 44. Der
    // Test dazu hatte nur Banner und Anleitung gerechnet und das Menü
    // dazwischen übersehen; er rechnet jetzt beides.
    banner::bildschirm();
    println!("{}", banner::zentriert(KURZANLEITUNG));
}

/// Kürzel für den n-ten Eintrag einer erzeugten Liste.
///
/// Ziffern zuerst, danach Buchstaben. Ohne dieses Kürzel hätte ein Eintrag
/// jenseits des neunten nur den Weg über die Pfeiltasten, und der Weg
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


/// Die Entwickler-Ebene. Kehrt mit dem letzten Ergebnis zurück.
fn entwickler(e: &mut Einstellungen, mut letztes_ergebnis: bool) -> bool {
    banner::bildschirm();
    loop {
        let Some(wahl) =
            auswahl::waehlen_mit_fuss("Entwickler", &menue_entwickler(), &e.als_text())
        else {
            banner::bildschirm();
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
            '8' => {
                e.teilnehmer = namen_erfragen();
                // Die Rückmeldung fehlt hier sonst: Beim Start gibt sie
                // die Begrüßung, und die läuft nur dort.
                println!("  Protokolle laufen jetzt unter {:?}.", e.teilnehmer);
            }
            '9' => freigeben(),
            'v' => letztes_ergebnis = vergleichen(e),
            '0' => {
                banner::bildschirm();
                return letztes_ergebnis;
            }
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
/// gemeinsames Protokoll: siehe [`testlauf`].
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
/// Koordinator erst wieder zusammensetzen müsste, und beim Verschicken
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

/// Wartet auf einen Tastendruck und räumt danach den Bildschirm auf.
///
/// **Der Gegenpart zum Aufräumen.** Ohne das Warten verschwände die
/// Ausgabe eines Laufs in dem Augenblick, in dem sie fertig ist, der
/// Nutzer sähe das Ergebnis nie. Mit ihm bleibt sie stehen, solange er
/// sie liest, und er entscheidet, wann weitergegangen wird.
///
/// **Das Aufräumen gehört hierher, nicht an den Anfang der Menüschleife.**
/// So folgt auf **jeden** Tastendruck ein sauberer Bildschirm, gleich von
/// welcher Stelle aus gewartet wurde: aus dem Nutzermenü, aus dem
/// Entwicklermenü, nach einem Untermenü. Lag es am Schleifenanfang, blieb
/// jeder Pfad ungedeckt, der nicht dorthin zurückkehrt.
///
/// Ohne Terminal wird nicht gewartet: Ein Skript hat niemanden, der eine
/// Taste drückt, und würde stillstehen.
fn weiter() {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }
    let hinweis = "  ── Weiter mit einer beliebigen Taste ──";
    println!("\n{}", banner::zentriert(hinweis));
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
    banner::bildschirm();
}

/// Wie viele Token eine Antwort höchstens lang wird.
///
/// **Gerechnet, nicht geraten.** `bench/README.md` nennt 24 Token je
/// Sekunde für 0,5B und 2 für 7B. 64 Token sind damit rund 3 Sekunden auf
/// dem kleinen und rund 32 auf dem großen Modell. Weniger ergäbe abgehackte
/// Sätze, mehr hieße bei 7B über eine Minute Warten auf eine Antwort, die
/// man ohnehin nicht zu Ende liest.
///
/// Warum überhaupt eine Grenze: Ohne sie liefe die Erzeugung bis zum
/// Kontextende, denn eine gierige Auswahl hört von selbst nicht auf.
const ANTWORT_TOKEN: usize = 64;

/// Wie man aus dem Gespräch zurück ins Menü kommt.
///
/// **Drei Wege, weil drei verschiedene Leute drei verschiedene erwarten.**
/// Escape ist der Griff, den jeder erwartet, der ein Menü verlässt;
/// Strg-D der, den jeder kennt, der schon einmal in einer Shell war; und
/// ein getipptes Wort der, den jemand findet, der noch nie eine gesehen
/// hat.
///
/// **Die leere Eingabe ist bewusst keiner mehr.** Sie war es bis v0.6.0,
/// und das war falsch: Enter tippt man auch, um zu sehen, ob sich etwas
/// aufgehängt hat. Wer sich im Gespräch vergewissern wollte, stand danach
/// im Menü. Eine leere Zeile fragt jetzt einfach neu.
///
/// **Strg-C wäre der falsche Weg** und ist deshalb keiner: Es beendet den
/// ganzen Client, nicht das Gespräch. Wer nach einem Testlauf noch
/// vergleichen will, verlöre den Sitzungsnamen und müsste von vorn
/// anfangen.
const RUECKWEG: &str = "Esc, Strg-D oder \"menu\"";

/// Erkennt die getippten Rückwege.
fn ist_rueckweg(eingabe: &str) -> bool {
    matches!(
        eingabe.trim().trim_start_matches(['/', ':']).to_lowercase().as_str(),
        "menu" | "menü" | "exit" | "quit" | "zurueck" | "zurück" | "q"
    )
}

/// Nutzermenü [1]: frei mit dem Modell sprechen.
///
/// **Der einzige Punkt des Clients, der nicht misst.** Er beantwortet die
/// Frage, die sich jeder stellt, der seine Maschine für einen fremden Test
/// hergibt: Was rechnet das Ding da eigentlich? Ein Modell, mit dem man
/// einmal gesprochen hat, ist kein abstraktes Artefakt mehr, und das ist
/// den Punkt wert.
///
/// **Das Modell wird einmal geladen, nicht je Frage.** Bei 7B dauert das
/// Laden rund eine Minute; für jede Frage neu zu laden machte aus einem
/// Gespräch eine Reihe von Wartezeiten.
///
/// **Kein Protokoll.** Prompt und Länge bestimmt hier der Nutzer frei; ein
/// Protokoll darüber sähe aus wie ein Messergebnis und wäre keines. Der
/// Vergleichswert entsteht in [1] und [2], nicht hier.
fn sprechen(e: &Einstellungen) {
    let modell = match runs::modell_laden(&e.artifacts) {
        Ok(m) => m,
        Err(fehler) => {
            println!("  {}", fehler);
            println!("  Mit [4] ein Artefakt wählen oder beschaffen.");
            return;
        }
    };

    println!("  Modell geladen: {}", e.artifacts.display());
    println!("  Höchstens {} Token je Antwort.", ANTWORT_TOKEN);
    println!(
        "  Die Auswahl ist gierig, ohne Sampling und ohne Zufall: Dieselbe\n  \
         Frage liefert auf demselben Modellstand dieselbe Antwort."
    );
    println!("\n  Zurück ins Menü: {}\n", RUECKWEG);

    loop {
        // Der Hinweis steht in der Eingabezeile, nicht nur im Kopf: Wer
        // ein paar Fragen gestellt hat, hat den Kopf längst weggescrollt,
        // und dann ist „wie komme ich hier raus" die dringende Frage.
        //
        // `zeile_lesen` statt `frage`, weil es Escape von Enter
        // unterscheidet: siehe `RUECKWEG`.
        let Some(frage) = auswahl::zeile_lesen("  Prompt [Esc = Menü]:  ") else {
            println!("\n  Zurück im Menü.");
            return;
        };
        let frage = frage.trim().to_string();
        if ist_rueckweg(&frage) {
            println!("\n  Zurück im Menü.");
            return;
        }
        // **Eine leere Eingabe tut nichts.** Enter drückt man auch, um zu
        // sehen, ob sich etwas aufgehängt hat; das darf nicht das
        // Gespräch beenden.
        if frage.is_empty() {
            continue;
        }

        // Der Name kommt aus dem Artefaktverzeichnis, nicht aus der
        // Voreinstellung: Wer 7B geladen hat, soll nicht „qwen2.5-0.5b"
        // lesen. Eine falsche Beschriftung neben einer echten Antwort ist
        // schlechter als gar keine.
        let name = e
            .artifacts
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Modell".to_string());

        // **Die Antwort erscheint Token für Token.** Bei 7B dauert sie
        // über eine halbe Minute; ohne laufende Ausgabe wäre in dieser
        // Zeit nicht zu unterscheiden, ob gerechnet wird oder etwas hängt.
        print!("\n  {}: ", name);
        let _ = std::io::Write::flush(&mut std::io::stdout());

        let begonnen = std::time::Instant::now();
        let mut zeigen = |stueck: &str| {
            print!("{}", stueck);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        };
        match runs::antworten(&modell, &e.artifacts, &frage, ANTWORT_TOKEN, &mut zeigen) {
            Ok(_) => println!("\n  ({} s)\n", begonnen.elapsed().as_secs()),
            Err(fehler) => println!("\n  {}\n", fehler),
        }
    }
}

/// Nutzermenü [4]: das Modell wählen, mit dem gerechnet wird.
///
/// **Warum das in das Nutzermenü gehört.** Beim Start wird einmal danach
/// gefragt, und wer damals „später" gewählt oder das falsche Modell
/// erwischt hat, musste bisher den Client neu starten. Der Punkt macht
/// aus einem Startzustand eine Einstellung.
///
/// Die Arbeit selbst leistet [`crate::artefakte::beschaffen`]: suchen, bei
/// mehreren fragen, sonst die Gewichte holen und über das Skalenpaket
/// bauen. Hier steht nur die Anbindung ans Menü, damit es genau **einen**
/// Weg zum Artefakt gibt und nicht zwei, die auseinanderlaufen.
fn artefakt_waehlen(e: &mut Einstellungen) {
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let mut frage = |prompt: &str| -> Option<String> { auswahl::frage(&format!("  {}", prompt)) };
    let mut f: crate::artefakte::Rueckfrage = Some(&mut frage);

    match crate::artefakte::beschaffen(&repo, &mut f, &mut |t| println!("  {}", t)) {
        Ok(pfad) => {
            println!("\n  Artefakt: {}", pfad.display());
            e.artifacts = pfad;
        }
        Err(fehler) => {
            for zeile in fehler.lines() {
                println!("  {}", zeile);
            }
            println!("\n  Die Modellläufe werden ohne Artefakt fehlschlagen.");
        }
    }
}

/// Entwicklermenü: Protokolle vergleichen.
///
/// **Zwei Quellen, und die Wahl gehört dem Nutzer.** Der Koordinator legt
/// die zugesandten Protokolle in `TESTCLIENT/Vergleiche/`; ein Teilnehmer
/// will dagegen die eigenen Läufe aus `logs/` gegenüberstellen. Beides in
/// einen Topf zu werfen wäre der schlechtere Weg: Ein Urteil über eine
/// Gruppe, in der die eigene Maschine mehrfach steckt, sagt etwas anderes
/// aus, als es zu sagen scheint.
///
/// Der Bericht landet in beiden Fällen unter `Vergleiche/Berichte/`.
fn vergleichen(e: &mut Einstellungen) -> bool {
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let zugesandt = crate::vergleich::vergleichsordner(&repo);
    let berichte = crate::vergleich::berichtsordner(&repo);

    let punkte = vec![
        Punkt::neu(
            '1',
            "Zugesandte Protokolle",
            &format!(
                "Was im Ordner {} liegt, der Weg des Koordinators.",
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
/// getipptes „ja", die eine Stelle, an der Enter allein nicht genügt.
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
    // Alles auf einmal, als eigener Punkt am Ende der Liste.
    //
    // **Warum überhaupt.** Wer eine Maschine für einen Test zur Verfügung
    // gestellt hat und danach seine 25 GB zurückhaben will, soll nicht
    // sechsmal dasselbe Menü durchlaufen. Der Punkt steht bewusst **unten**
    // und nicht oben: Er ist der folgenreichste der Liste, und die erste
    // Zeile ist die, auf der die Markierung beim Öffnen steht.
    //
    // Das Kürzel kommt aus derselben Folge wie die Einträge darüber, statt
    // ein sprechendes 'a' zu setzen: `kuerzel(9)` **ist** 'a', und ab zehn
    // Einträgen träfen zwei Punkte auf dieselbe Taste. Ein Menü, in dem
    // eine Taste zwei Bedeutungen hat, ist ein Fehler, der erst auf einer
    // fremden Maschine mit vielen Modellen auffiele.
    let alles = kuerzel(ziele.len());
    punkte.push(Punkt::neu(
        alles,
        &format!("ALLES löschen · {}", crate::artefakte::groesse(gesamt)),
        "Artefakte und Gewichte aller Modelle. Fragt zweimal nach.",
    ));
    punkte.push(Punkt::neu('0', "Nichts löschen", ""));

    let Some(wahl) = auswahl::waehlen("Was freigeben?", &punkte) else {
        return;
    };

    if wahl == alles {
        alles_freigeben(&repo, &ziele, gesamt);
        return;
    }

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

    if !bestaetigt("  Zum Bestätigen \"ja\" eintippen: ") {
        println!("\n  Abgebrochen, nichts gelöscht.");
        return;
    }

    match crate::artefakte::freigeben(&repo, pfad) {
        Ok(bytes) => println!("\n  {} freigegeben.", crate::artefakte::groesse(bytes)),
        Err(e) => println!("\n  {}", e),
    }
}

/// Eine getippte Bestätigung. Nur ein ausgeschriebenes „ja" zählt.
///
/// Kein Menüpunkt und keine einzelne Taste: Eine Auswahl lässt sich mit
/// einem versehentlichen Enter bestätigen, ein Wort nicht.
fn bestaetigt(frage: &str) -> bool {
    auswahl::frage(frage)
        .unwrap_or_default()
        .trim()
        .eq_ignore_ascii_case("ja")
}

/// Löscht Artefakte und Gewichte aller Modelle.
///
/// **Zwei Bestätigungen, und die zweite ist die eigentliche.** Die erste
/// fragt, ob gelöscht werden soll; die zweite nennt jeden betroffenen Pfad
/// einzeln und verlangt danach noch einmal ein „ja". Der Grund ist die
/// Reichweite: Ein Fehlgriff kostet hier einen Download von bis zu 25 GB,
/// und das ist für jemanden mit langsamer Leitung ein verlorener Abend.
///
/// Die Liste zwischen den beiden Fragen ist kein Zierat. Ohne sie
/// bestätigte man zweimal dieselbe Zahl; mit ihr sieht man, was tatsächlich
/// verschwindet, und kann beim zweiten Mal begründet abbrechen.
fn alles_freigeben(repo: &std::path::Path, ziele: &[(String, std::path::PathBuf)], gesamt: u64) {
    println!(
        "\n  ALLES löschen: {} Einträge, zusammen {}.",
        ziele.len(),
        crate::artefakte::groesse(gesamt)
    );
    println!("  Das lässt sich nicht rückgängig machen.\n");

    if !bestaetigt("  Erste Bestätigung, \"ja\" eintippen: ") {
        println!("\n  Abgebrochen, nichts gelöscht.");
        return;
    }

    println!("\n  Betroffen sind:");
    for (was, pfad) in ziele {
        println!("    {} · {}", was, pfad.display());
    }
    println!(
        "\n  Gewichte kosten danach einen erneuten Download über Hugging Face;\n  \
         Artefakte sind aus dem Skalenpaket in Sekunden wiederhergestellt.\n"
    );

    if !bestaetigt("  Zweite Bestätigung, nochmals \"ja\" eintippen: ") {
        println!("\n  Abgebrochen, nichts gelöscht.");
        return;
    }

    let mut frei = 0u64;
    let mut fehler = 0usize;
    for (was, pfad) in ziele {
        match crate::artefakte::freigeben(repo, pfad) {
            Ok(bytes) => {
                frei += bytes;
                println!("  gelöscht: {}", was);
            }
            // Weitermachen statt abbrechen: Ein Eintrag, der sich nicht
            // löschen lässt, soll die übrigen nicht am Freiwerden hindern.
            Err(e) => {
                fehler += 1;
                println!("  FEHLER bei {}: {}", was, e);
            }
        }
    }

    println!("\n  {} freigegeben.", crate::artefakte::groesse(frei));
    if fehler > 0 {
        println!("  {} Eintrag/Einträge blieben liegen, siehe oben.", fehler);
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

    // **Keine Artefaktfrage an dieser Stelle.** Bis v0.6.0 löste die
    // Testdatei sofort das Artefakt mit auf: suchen, bei mehreren fragen,
    // sonst Gewichte holen und bauen. Das waren zwei Entscheidungen hinter
    // einer Menüwahl, und die zweite kam ungefragt, unter Umständen mit
    // einem Download von 15 GB.
    //
    // Das Artefakt ist jetzt ein eigener Menüpunkt [3]. Hier wird nur noch
    // **stillschweigend übernommen, was ohnehin daliegt**: Wer das Modell
    // des Plans bereits gebaut hat, soll nicht danach gefragt werden, was
    // er längst hat.
    match crate::artefakte::vorhandenes(&repo, &plan.model) {
        Some(pfad) => {
            e.artifacts = pfad;
            println!("\n  Artefakt für {} liegt bereit.", plan.model);
            println!("  Mit [2] den Testlauf starten.");
        }
        None => {
            println!("\n  Das Artefakt für {} fehlt noch.", plan.model);
            println!("  Mit [4] beschaffen, danach mit [2] starten.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Der Rückweg muss die Schreibweisen abdecken, die jemand
    /// tatsächlich tippt, und darf keine echte Frage abfangen.
    #[test]
    fn rueckweg_erkennt_die_ueblichen_schreibweisen() {
        for w in [
            "menu", "menü", "Menu", "/menu", ":q", "q", "exit", "QUIT", "  zurück  ",
        ] {
            assert!(ist_rueckweg(w), "{w:?} sollte zurückführen");
        }
        // Eine Frage, die zufällig so anfängt, ist keine Anweisung.
        for w in [
            "Was ist ein Menu?",
            "quitte",
            "Erkläre mir exit codes",
            "Die Hauptstadt von Frankreich ist",
        ] {
            assert!(!ist_rueckweg(w), "{w:?} ist eine Frage, kein Rückweg");
        }
    }

    /// Der Hinweis muss die Tasten auch wirklich nennen: Ein Gespräch,
    /// aus dem man nicht sichtbar herausfindet, endet mit Strg-C und damit
    /// mit dem ganzen Client.
    #[test]
    fn rueckweg_wird_benannt() {
        assert!(RUECKWEG.contains("Esc"), "Escape fehlt: {RUECKWEG}");
        assert!(RUECKWEG.contains("Strg-D"), "Strg-D fehlt: {RUECKWEG}");
        assert!(RUECKWEG.contains("menu"), "getipptes Wort fehlt: {RUECKWEG}");
    }

    /// **Die leere Eingabe darf nicht zurückführen.** Enter tippt man auch,
    /// um zu sehen, ob sich etwas aufgehängt hat; bis v0.6.0 stand man
    /// danach im Menü.
    #[test]
    fn leere_eingabe_ist_kein_rueckweg() {
        for leer in ["", " ", "\t", "   "] {
            assert!(
                !ist_rueckweg(leer),
                "leere Eingabe {leer:?} führt zurück"
            );
        }
    }

    /// Die Kurzanleitung muss auf den Bildschirm passen. Reicht sie
    /// darüber hinaus, scrollt das Logo nach oben weg, und der
    /// aufgeräumte Bildschirm hätte seinen Zweck verfehlt.
    #[test]
    fn kurzanleitung_passt_unter_das_banner() {
        let (breite, hoehe) = (120u16, 44u16);
        let banner = banner::fuer_fenster(breite, hoehe).lines().count();
        // Banner, Untertitel, Leerzeile, Anleitung, Leerzeile und die
        // Zeile „Weiter mit einer beliebigen Taste". Das Menü zählt nicht
        // mit, weil `anleitung_zeigen` vorher aufräumt: Ohne das
        // Aufräumen stünden hier 42 Zeilen Menü darüber, und genau daran
        // ist die erste Fassung gescheitert.
        let gesamt = banner + 2 + KURZANLEITUNG.lines().count() + 2;
        assert!(
            gesamt <= hoehe as usize,
            "Anleitung braucht {} von {} Zeilen",
            gesamt,
            hoehe
        );
    }

    /// In 80 Spalten, wie alles andere auch.
    #[test]
    fn kurzanleitung_passt_in_achtzig_spalten() {
        for zeile in KURZANLEITUNG.lines() {
            assert!(
                zeile.chars().count() <= 78,
                "{} Zeichen: {zeile}",
                zeile.chars().count()
            );
        }
    }

    /// Die Anleitung nennt Menüpunkte. Ändert sich die Nummerierung, muss
    /// sie mitgeändert werden: Eine Anleitung, die auf den falschen Punkt
    /// zeigt, ist schlechter als keine.
    #[test]
    fn kurzanleitung_nennt_die_richtigen_menuepunkte() {
        let punkte = menue_nutzer();
        let taste_von = |titel: &str| {
            punkte
                .iter()
                .find(|p| p.titel.contains(titel))
                .map(|p| p.taste)
                .unwrap_or_else(|| panic!("Menüpunkt {titel:?} fehlt"))
        };
        for (titel, zweck) in [
            ("Testdatei", "Testdatei wählen"),
            ("Artefakt", "Artefakt beschaffen"),
            ("Testlauf", "Testlauf starten"),
        ] {
            let taste = taste_von(titel);
            assert!(
                KURZANLEITUNG.contains(&format!("[{}] {}", taste, zweck)),
                "Anleitung nennt für {titel:?} nicht [{taste}]"
            );
        }
        // Der Vergleich sitzt im Entwicklermenü, die Anleitung muss
        // dorthin verweisen.
        assert!(
            KURZANLEITUNG.contains("[9] Entwickler"),
            "Verweis auf das Entwicklermenü fehlt"
        );
    }

    #[test]
    fn nutzermenue_nennt_alle_punkte() {
        let tasten: Vec<char> = menue_nutzer().iter().map(|p| p.taste).collect();
        assert_eq!(tasten, vec!['1', '2', '3', '4', '5', '9', '0']);
    }

    /// Das Nutzermenü darf nicht wieder anwachsen: Es ist die Seite, die
    /// ein Teilnehmer ohne Vorwissen zuerst sieht. Gezählt werden die
    /// Punkte, die etwas tun: „Entwickler-Menü" und „Beenden" sind Wege
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

    /// Jede Taste darf nur einmal vorkommen: sonst wäre nicht bestimmt,
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
        assert_eq!(
            tasten,
            vec!['1', '2', '3', '4', '5', '6', '7', '8', '9', 'v', '0']
        );
    }

    /// Das Nutzermenü führt vier Punkte in der Reihenfolge des Ablaufs:
    /// messen, Testdatei, Artefakt, nachlesen. „Protokolle vergleichen"
    /// steht bewusst **nicht** darin: Es ist die Arbeit des Koordinators,
    /// und für einen Teilnehmer, der eine Maschine beisteuert, ein Punkt,
    /// der ihm nichts nützt.
    #[test]
    fn nutzermenue_fuehrt_die_schritte_in_der_reihenfolge_des_ablaufs() {
        let punkte = menue_nutzer();
        let tasten: Vec<char> = punkte.iter().map(|p| p.taste).collect();
        assert_eq!(tasten, vec!['1', '2', '3', '4', '5', '9', '0']);

        let titel: Vec<&str> = punkte.iter().map(|p| p.titel.as_str()).collect();
        assert_eq!(titel[0], "Mit dem Modell sprechen");
        assert_eq!(titel[1], "Testlauf starten");
        assert_eq!(titel[2], "Testdatei wählen");
        assert_eq!(titel[3], "Artefakt wählen");
        assert!(
            !titel.iter().any(|t| t.contains("vergleichen")),
            "Vergleichen gehört ins Entwicklermenü"
        );
        assert!(
            menue_entwickler().iter().any(|p| p.titel.contains("vergleichen")),
            "Vergleichen fehlt im Entwicklermenü"
        );
    }

    /// Kein Punkt darf in beiden Menüs auf derselben Taste etwas anderes
    /// tun: Wer die Ziffer aus dem einen Menü im anderen tippt, landete
    /// sonst bei einer Aktion, die er nicht gemeint hat.
    #[test]
    fn kein_menue_vergibt_eine_taste_doppelt() {
        for menue in [menue_nutzer(), menue_entwickler()] {
            let mut tasten: Vec<char> = menue.iter().map(|p| p.taste).collect();
            let vorher = tasten.len();
            tasten.sort_unstable();
            tasten.dedup();
            assert_eq!(tasten.len(), vorher, "Taste doppelt vergeben");
        }
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

    /// Lange Pfade werden auf die letzten drei Glieder gekürzt, kurze
    /// bleiben unangetastet.
    ///
    /// **Mit dem Trennzeichen der Plattform gebaut, nicht mit einem
    /// festen.** Der Test stand vorher auf `"…/d/e/f"` und schlug unter
    /// Windows fehl, weil der zusammengesetzte Rest dort Rückstriche
    /// bekommt. Der Fehler lag im Code, nicht im Test: Die Ausgabe mischte
    /// beide Zeichen.
    #[test]
    fn pfade_werden_gekuerzt() {
        let t = std::path::MAIN_SEPARATOR;
        let lang: PathBuf = ["", "a", "b", "c", "d", "e", "f"].iter().collect();
        assert_eq!(kurz(&lang), format!("…{t}d{t}e{t}f"));

        let kurzer: PathBuf = ["a", "b"].iter().collect();
        assert_eq!(kurz(&kurzer), format!("a{t}b"));
    }

    /// Der eigentliche Fund: In einer Zeile darf nur **ein**
    /// Trennzeichen vorkommen. Gemischt gelesen sieht ein Pfad nach einem
    /// Fehler aus, und auf einer fremden Maschine ist genau das die
    /// Frage, die niemand beantworten kann.
    #[test]
    fn gekuerzte_pfade_mischen_keine_trennzeichen() {
        let lang: PathBuf = ["", "a", "b", "c", "d", "e", "f"].iter().collect();
        let text = kurz(&lang);
        assert!(
            !(text.contains('/') && text.contains('\\')),
            "gemischte Trennzeichen: {text}"
        );
    }

    #[test]
    fn ja_nein_ist_eindeutig() {
        assert_eq!(ja_nein(true), "OK");
        assert_eq!(ja_nein(false), "FEHLGESCHLAGEN");
    }
}
