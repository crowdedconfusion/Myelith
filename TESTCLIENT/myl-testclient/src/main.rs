//! `myl-test`. Terminal-Testclient für Myelith.
//!
//! Argumentauswertung von Hand, ohne Fremd-Crate: Der Client soll auf
//! einer fremden Maschine mit möglichst wenig Voraussetzungen bauen.

use std::path::PathBuf;
use std::process::ExitCode;

use myl_testclient::menu::{self, Einstellungen};
use myl_testclient::{
    banner, default_artifact_dir, default_log_dir, run_determinism, run_hardware, run_shard,
    run_stack, LogZiel, RunLog, TestPlan,
};

const HILFE: &str = "\
myl-test. Testclient für Myelith

AUFRUF
    myl-test                 ohne Befehl: interaktives Menü
    myl-test <BEFEHL> [OPTIONEN]

BEFEHLE
    hardware        Hardware erheben und protokollieren. Braucht kein
                    Modell, der erste Befehl auf einer neuen Maschine.
    determinismus   Denselben Prompt zweimal rechnen und auf Bitgleichheit
                    prüfen. Der Vergleichswert im Protokoll muss auf jeder
                    Maschine derselbe sein.
    shard           Geshardete Inferenz über einen Pod fahren und gegen
                    die Einzelknoten-Runtime prüfen.
    artefakte       Modelle auf dieser Maschine prüfen: sind Artefakte da,
                    und stimmen sie mit dem veröffentlichten Digest überein?
                    Der erste Befehl vor jedem Vergleichslauf, ohne ihn
                    sähe ein abweichendes Artefakt wie eine gescheiterte
                    Hardware-Bitgleichheit aus.

    modellstaende   Was sich beim Wechsel von θ_v A nach B geändert hat und
                    was nicht. Kein Determinismusurteil: Zwei Modellstände
                    SOLLEN verschiedene Zahlen liefern. Interessant ist die
                    Gegenrichtung, ein Wert, der einen Wechsel unbeschadet
                    übersteht. Liest denselben Ordner wie `vergleich`.
    vergleich       Die zugesandten Protokolle aus TESTCLIENT/Vergleiche
                    gegenüberstellen und urteilen, ob sie den Cross-Hardware-
                    Nachweis tragen. Verweigert ein positives Urteil, wenn
                    alle Protokolle von derselben Maschine stammen: das
                    wäre kein Nachweis. Schreibt einen ausführlichen Bericht
                    nach TESTCLIENT/Vergleiche/Berichte.

    stack           Protokoll-Durchlauf über Krypto, Epochenseed,
                    Komiteewahl, BFT, Verifikation, Ledger und Tokenomics.
                    Braucht kein Modell.
    plan            Testplan erzeugen (Koordinator), die Datei, die an
                    alle Teilnehmer geht. Siehe TESTPLAN unten.
    menu            Interaktives Menü (wie ohne Befehl).

OPTIONEN
    --prompt <TEXT>     Eingabetext (Vorgabe: \"Die Hauptstadt von Frankreich ist\")
                        Mehrfach angebbar: jeder weitere hängt einen Prompt an
                        die Reihe an, die der Lauf nacheinander abarbeitet
    --steps <N>         Zu erzeugende Token (Vorgabe: 8)
    --shards <N>        Anzahl Shards für `shard` (Vorgabe: 4)
    --repeat <N>        Läufe je Prompt im Determinismuslauf (Vorgabe: 2,
                        Minimum 2). Höhere Werte suchen sporadische
                        Abweichungen, die bei zwei Läufen durchrutschen:
                        Speicherfehler, thermisches Drosseln, ein Wackler
                        unter Last. Der Vergleichswert bleibt derselbe, es
                        kommen nur weitere Läufe je Prompt hinzu.
                        ACHTUNG: Alle Beteiligten müssen denselben Wert
                        verwenden, sonst tragen ihre Protokolle
                        verschiedene Vergleichswerte, und `vergleich`
                        urteilt zu Recht UNVOLLSTÄNDIG.
    --plan <DATEI>      Testplan laden. Setzt Prompt, Token, Shards und
                        Modell und prüft die Datei gegen ihre Prüfsumme.
    --artifacts <PFAD>  Artefaktverzeichnis (Vorgabe: qwen2.5-0.5b)
    --plan-id <TEXT>    Kennung beim Erzeugen eines Plans
    --model <NAME>      Modell beim Erzeugen eines Plans (Vorgabe: qwen2.5-0.5b)
    --erwarte <DIGEST>  Erwarteter Vergleichswert. Der Lauf schlägt fehl,
                        wenn er einen anderen erzeugt. Für die CI nach
                        einem Modellwechsel: Ab da meldet sich jede
                        weitere Änderung von selbst. Die Kurzform vom
                        Bildschirm genügt (16 Hexzeichen); verglichen wird
                        so weit, wie angegeben ist, und das Protokoll hält
                        fest, wie weit das war
    --out <DATEI>       Zieldatei beim Erzeugen eines Plans
    --logs <PFAD>       Protokollverzeichnis (Vorgabe: TESTCLIENT/logs)
                        Bei `vergleich` das auszuwertende Verzeichnis; ohne
                        die Option wird TESTCLIENT/Vergleiche gelesen
    --name <TEXT>       Name des Teilnehmers. Steht im Protokoll und im
                        Dateinamen, damit der Koordinator Protokolle ohne
                        Rückfrage zuordnen kann. Im Menü wird danach gefragt
    --quiet             Nur ins Protokoll schreiben, nicht aufs Terminal
                        (unterdrückt auch das Banner)
    -h, --help          Diese Hilfe
ARTEFAKTE
    `determinismus` und `shard` brauchen ein Modell. Ohne `--artifacts`
    sucht der Client selbst: Findet er eines, nimmt er es; findet er
    mehrere, fragt er; findet er keines, bietet er an, die Gewichte von
    Hugging Face zu holen und die Artefakte zu bauen. Der Bau nutzt das
    versionierte Skalenpaket und dauert Sekunden.

    Bei `--quiet` wird nicht gefragt und deshalb auch nichts geladen:
    ein mehrere Gigabyte großer Zugriff gehört nicht in einen Skriptlauf,
    der ihn nicht angefordert hat.

    Umgebung: MYL_NO_BANNER=1 unterdrückt das Banner dauerhaft.

PROTOKOLLE
    Jeder Lauf schreibt zwei Dateien flach nach logs/:
        <name>_<einstellungs-id>_<datum>_<uhrzeit>.jsonl   maschinenlesbar
        <name>_<einstellungs-id>_<datum>_<uhrzeit>.log     Fließtext

    Dieselben Angaben stehen auch IM Protokoll: eine Datei wird
    umbenannt, ein Feld nicht. Prompttexte werden gehasht, nicht
    gespeichert.

    Umgebung: MYL_NO_ANIMATION=1 überspringt die Startanimation,
    MYL_NO_BANNER=1 auch das Banner.

TESTPLAN
    Koordinator erzeugt und verschickt:
        myl-test plan --plan-id 2026-08-18-arch --prompt \"...\" --steps 8
        → schreibt <plan-id>.plan

    Teilnehmer verwenden ihn:
        myl-test --plan 2026-08-18-arch.plan determinismus

    Die Datei trägt eine Prüfsumme über Prompt, Token, Shards und
    Modell. Wird sie verändert, verweigert der Client den Lauf: ein
    Tippfehler soll nicht als Befund durchgehen.

CROSS-HARDWARE-NACHWEIS
    1. Auf jeder Maschine:  myl-test artefakte
       → derselbe Modellstand, sonst sagt der Vergleich nichts aus.
    2. Auf jeder Maschine:  myl-test --name <wer> --plan <datei> determinismus
    3. Alle .jsonl nach TESTCLIENT/Vergleiche legen, dann:
           myl-test vergleich

    Der Nachweis braucht zwei Aussagen: Die Maschinen sind verschieden,
    und das Ergebnis ist trotzdem gleich. `vergleich` prüft beide und
    verweigert das Urteil, wenn eine davon fehlt.
";

struct Args {
    command: String,
    prompts: Vec<String>,
    steps: usize,
    shards: usize,
    artifacts: PathBuf,
    /// Wurde `--artifacts` (oder ein Plan) ausdrücklich gesetzt? Dann wird
    /// nicht gesucht und nichts beschafft: eine ausdrückliche Angabe hat
    /// Vorrang vor jeder Automatik.
    artifacts_explizit: bool,
    logs: PathBuf,
    /// Wurde `--logs` ausdrücklich gesetzt? `vergleich` liest sonst den
    /// Ordner der zugesandten Protokolle statt des eigenen Protokollorts.
    logs_explizit: bool,
    /// Name des Teilnehmers; steht im Protokoll und im Dateinamen.
    name: String,
    /// Modell, das ein erzeugter Plan vorgibt.
    model: String,
    quiet: bool,
    /// Läufe je Prompt im Determinismuslauf (`--repeat`).
    wiederholungen: usize,
    plan: Option<PathBuf>,
    plan_id: Option<String>,
    out: Option<PathBuf>,
    /// Erwarteter Vergleichswert (`--erwarte`, Fahrplanpunkt 3.2).
    erwartet: Option<String>,
}

impl Args {
    fn vorgaben() -> Self {
        Self {
            command: String::new(),
            name: myl_testclient::OHNE_NAME.to_string(),
            model: myl_testclient::DEFAULT_MODEL.to_string(),
            prompts: vec!["Die Hauptstadt von Frankreich ist".to_string()],
            steps: 8,
            shards: 4,
            // Zwei ist der bisherige und der kleinste sinnvolle Wert:
            // Bitgleichheit braucht zwei Ergebnisse.
            wiederholungen: 2,
            artifacts: default_artifact_dir(),
            artifacts_explizit: false,
            logs: default_log_dir(),
            logs_explizit: false,
            quiet: false,
            plan: None,
            plan_id: None,
            out: None,
            erwartet: None,
        }
    }
}

fn parse() -> Result<Args, String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-h" || a == "--help") {
        return Err(String::new());
    }

    let mut a = Args::vorgaben();
    let mut befehl: Option<String> = None;
    let mut prompt_gesetzt = false;

    // Optionen dürfen VOR und NACH dem Befehl stehen: `myl-test --plan x
    // stack` ist genauso gültig wie `myl-test stack --plan x`. Der erste
    // Wert, der keine Option und kein Optionswert ist, ist der Befehl.
    let mut i = 0;
    while i < raw.len() {
        let need = |i: usize, name: &str| -> Result<String, String> {
            raw.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{} erwartet einen Wert", name))
        };
        match raw[i].as_str() {
            // Mehrfach angebbar: `--prompt A --prompt B` ergibt eine Reihe.
            // Der erste Aufruf ersetzt die Vorgabe, jeder weitere hängt an.
            "--prompt" => {
                if !prompt_gesetzt {
                    a.prompts.clear();
                    prompt_gesetzt = true;
                }
                a.prompts.push(need(i, "--prompt")?);
                i += 2;
            }
            "--steps" => {
                a.steps = need(i, "--steps")?
                    .parse()
                    .map_err(|_| "--steps erwartet eine Zahl".to_string())?;
                i += 2;
            }
            "--repeat" => {
                a.wiederholungen = need(i, "--repeat")?
                    .parse()
                    .map_err(|_| "--repeat erwartet eine Zahl".to_string())?;
                i += 2;
            }
            "--shards" => {
                a.shards = need(i, "--shards")?
                    .parse()
                    .map_err(|_| "--shards erwartet eine Zahl".to_string())?;
                i += 2;
            }
            "--artifacts" => {
                a.artifacts = PathBuf::from(need(i, "--artifacts")?);
                a.artifacts_explizit = true;
                i += 2;
            }
            "--logs" => {
                a.logs = PathBuf::from(need(i, "--logs")?);
                a.logs_explizit = true;
                i += 2;
            }
            "--name" => {
                a.name = need(i, "--name")?;
                i += 2;
            }
            "--model" => {
                a.model = need(i, "--model")?;
                i += 2;
            }
            "--plan" => {
                a.artifacts_explizit = true;
                a.plan = Some(PathBuf::from(need(i, "--plan")?));
                i += 2;
            }
            "--plan-id" => {
                a.plan_id = Some(need(i, "--plan-id")?);
                i += 2;
            }
            "--out" => {
                a.out = Some(PathBuf::from(need(i, "--out")?));
                i += 2;
            }
            "--erwarte" => {
                a.erwartet = Some(need(i, "--erwarte")?);
                i += 2;
            }
            "--quiet" => {
                a.quiet = true;
                i += 1;
            }
            wort if wort.starts_with('-') => {
                return Err(format!("unbekannte Option: {}", wort));
            }
            wort => {
                if befehl.is_some() {
                    return Err(format!("unerwartetes Argument: {}", wort));
                }
                befehl = Some(wort.to_string());
                i += 1;
            }
        }
    }

    // Ohne Unterbefehl ins Menü.
    a.command = befehl.unwrap_or_else(|| "menu".to_string());

    if a.steps == 0 {
        return Err("--steps muss > 0 sein".into());
    }
    // Früh und mit Begründung, nicht erst im Lauf: Wer `--repeat 1`
    // schreibt, meint „nur einmal rechnen" und hätte sonst einen
    // Determinismuslauf gestartet, der nichts vergleicht.
    if a.wiederholungen < 2 {
        return Err(format!(
            "--repeat muss >= 2 sein, angegeben: {}. Bitgleichheit braucht \
             zwei Ergebnisse, ein einzelner Lauf vergleicht nichts.",
            a.wiederholungen
        ));
    }
    Ok(a)
}

fn plan_erzeugen(args: &Args) -> ExitCode {
    let plan = TestPlan {
        plan_id: args
            .plan_id
            .clone()
            .unwrap_or_else(|| "unbenannt".to_string()),
        prompts: args.prompts.clone(),
        steps: args.steps,
        shards: args.shards,
    };
    let ziel = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("{}.plan", plan.plan_id)));

    if let Err(e) = plan.save(&ziel) {
        eprintln!("myl-test: {}", e);
        return ExitCode::FAILURE;
    }

    println!("Testplan geschrieben: {}", ziel.display());
    println!();
    println!("  Kennung        {}", plan.plan_id);
    for (i, prompt) in plan.prompts.iter().enumerate() {
        println!("  Prompt {:<8}{:?}", i + 1, prompt);
    }
    println!("  Token          {}", plan.steps);
    println!("  Shards         {}", plan.shards);
    println!("  Einstellungs-ID {}", plan.short_id());
    println!();
    println!("Diese Datei unverändert an alle Teilnehmer schicken. Sie starten damit:");
    println!("    myl-test --plan {} determinismus", ziel.display());
    println!();
    println!("Alle Protokolle landen dann unter");
    println!(
        "    logs/<befehl>/<datum>_{}/ :  ohne Zuordnungsarbeit vergleichbar.",
        plan.short_id()
    );
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args = match parse() {
        Ok(a) => a,
        Err(msg) => {
            if msg.is_empty() {
                println!("{}", HILFE);
                return ExitCode::SUCCESS;
            }
            eprintln!("myl-test: {}\n", msg);
            eprintln!("{}", HILFE);
            return ExitCode::from(2);
        }
    };

    // Testplan laden, falls angegeben: er überschreibt die Optionen.
    let mut args = args;
    let mut einstellungen_id = "ohne-plan".to_string();
    // Der Name der Testdatei für die Anzeige, die Kennung für das
    // Protokoll. An acht Hexzeichen erkennt niemand seine Datei wieder.
    let mut plan_name: Option<String> = None;
    if let Some(pfad) = args.plan.clone() {
        match TestPlan::load(&pfad) {
            Ok(plan) => {
                println!("Testplan: {} ({})", plan.plan_id, pfad.display());
                println!("  Einstellungs-ID {}\n", plan.short_id());
                einstellungen_id = plan.short_id();
                plan_name = Some(plan.plan_id.clone());
                args.prompts = plan.prompts;
                args.steps = plan.steps;
                args.shards = plan.shards;
            }
            Err(e) => {
                eprintln!("myl-test: {}", e);
                return ExitCode::from(3);
            }
        }
    }

    if args.command == "plan" {
        return plan_erzeugen(&args);
    }

    // `vergleich` schreibt **kein Laufprotokoll**, sondern einen Bericht,
    // und der landet in einem Unterordner der Eingabe. Läge er daneben,
    // würde der nächste Aufruf ihn mitlesen.
    //
    // Ohne `--logs` wird der Ordner der **zugesandten** Protokolle
    // gelesen, nicht der eigene Protokollort: Der Befehl gehört dem
    // Koordinator, und der vergleicht fremde Läufe, nicht seine eigenen.
    // `modellstaende` schreibt wie `vergleich` kein Laufprotokoll: Es
    // wertet fremde Läufe aus, statt selbst zu messen.
    if args.command == "modellstaende" {
        let repo = myl_testclient::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
        let quelle = if args.logs_explizit {
            args.logs.clone()
        } else {
            myl_testclient::vergleich::vergleichsordner(&repo)
        };
        return if myl_testclient::modellstaende::run(&quelle) {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }

    if args.command == "vergleich" {
        banner::print_if(!args.quiet);
        let repo = myl_testclient::artefakte::repo_wurzel(
            std::env::current_dir().unwrap_or_default(),
        );
        let quelle = if args.logs_explizit {
            args.logs.clone()
        } else {
            myl_testclient::vergleich::vergleichsordner(&repo)
        };
        let berichte = myl_testclient::vergleich::berichtsordner(&repo);
        return if myl_testclient::run_vergleich(&quelle, Some(&berichte)) {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }

    if args.command == "menu" {
        // **Voreingestellt ist nur, was ausdrücklich angegeben wurde.**
        // Ohne `--artifacts` und ohne `--plan` startet das Menü mit
        // „nicht ausgewählt" in beiden Zeilen, und der Testlauf fragt
        // danach. Vorher zeigte es auf das Vorgabemodell und auf die
        // eingebauten Prompts: Das sah aus wie eine Entscheidung und war
        // eine Annahme.
        let ok = menu::run(Einstellungen {
            prompts: args.prompts,
            steps: args.steps,
            shards: args.shards,
            artifacts: args.artifacts_explizit.then_some(args.artifacts),
            testdatei: plan_name,
            logs: args.logs,
            einstellungen_id,
            teilnehmer: args.name,
            wiederholungen: args.wiederholungen,
        });
        return if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    }

    let echo = !args.quiet;
    banner::print_if(echo);
    let hardware = myl_testclient::Fingerprint::collect().short_id();
    let mut log = RunLog::mit_ziel(
        LogZiel::neu(
            &args.logs,
            &args.command,
            &args.name,
            &einstellungen_id,
            &hardware,
        ),
        echo,
    );

    let braucht_modell = matches!(args.command.as_str(), "determinismus" | "shard");

    // **Das Backend zuerst, vor dem Artefakt.** Ein Bau, der für ein
    // Backend ohne Rechenpfad konfiguriert ist, taugt für keinen
    // Messlauf; das steht fest, bevor irgendein Modell gebraucht wird.
    // Stünde die Prüfung dahinter, bekäme jemand mit `--features cuda`
    // erst einen Download von bis zu 15 GB und danach die Ablehnung.
    if braucht_modell {
        if let Err(begruendung) = myl_testclient::hardware::rechenpfad_pruefen() {
            for zeile in begruendung.lines() {
                log.error(zeile.to_string());
            }
            log.finish(false);
            return ExitCode::FAILURE;
        }
    }

    // Artefakte auflösen, bevor ein Lauf sie braucht. `hardware`, `stack`
    // und `artefakte` kommen ohne Modell aus: sie werden übersprungen,
    // damit der erste Befehl auf einer neuen Maschine keinen Download
    // auslöst.
    if braucht_modell && !args.artifacts_explizit {
        match myl_testclient::artefakte::beschaffen(
            &myl_testclient::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default()),
            &mut stdin_frage(echo).as_mut().map(|f| f as _),
            &mut |t| log.note(t),
        ) {
            Ok(p) => args.artifacts = p,
            Err(e) => {
                for zeile in e.lines() {
                    log.error(zeile.to_string());
                }
                log.finish(false);
                return ExitCode::FAILURE;
            }
        }
    }

    let ok = match args.command.as_str() {
        "hardware" => run_hardware(&mut log),
        "determinismus" => run_determinism(
            &mut log,
            &args.artifacts,
            &args.prompts,
            args.steps,
            args.wiederholungen,
        ),
        "shard" => run_shard(
            &mut log,
            &args.artifacts,
            &args.prompts,
            args.steps,
            args.shards,
        ),
        "artefakte" => run_artefakte(&mut log),
        "stack" => run_stack(&mut log),
        other => {
            log.error(format!("unbekannter Befehl: {}", other));
            log.finish(false);
            eprintln!("\n{}", HILFE);
            return ExitCode::from(2);
        }
    };

    // **Nach dem Lauf, vor dem Abschluss.** Die Erwartung prüft den
    // Gesamtwert, und den gibt es erst, wenn der Lauf durch ist. Das
    // Ergebnis geht mit `&&` ein: Ein Lauf, der bereits scheiterte, wird
    // durch eine erfüllte Erwartung nicht gut.
    let ok = match args.erwartet.as_deref() {
        Some(erwartet) => myl_testclient::erwartung::protokollieren(&mut log, erwartet) && ok,
        None => ok,
    };

    if log.finish(ok) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `artefakte`: prüft für jedes bekannte Modell, ob es auf dieser
/// Maschine vorliegt und ob es dem veröffentlichten Digest entspricht.
///
/// Ein abweichender Digest ist **kein Hardware-Befund**. Er heißt, dass
/// hier ein anderes Modell liegt als beim Vergleichspartner, und ein
/// Bitgleichheitstest darüber wäre wertlos. Deshalb sagt die Ausgabe das
/// ausdrücklich, statt nur „ungleich" zu melden.
fn run_artefakte(log: &mut RunLog) -> bool {
    use myl_testclient::artefakte::{bauanleitung, pruefen, register, Zustand};

    let repo = match std::env::current_dir() {
        Ok(d) => myl_testclient::artefakte::repo_wurzel(d),
        Err(e) => {
            log.error(format!("Arbeitsverzeichnis nicht lesbar: {}", e));
            return false;
        }
    };

    let bekannt = match register(&repo) {
        Ok(b) => b,
        Err(e) => {
            log.error(format!("Register nicht lesbar: {}", e));
            log.note("Ohne INTEGER_LLM/scale_packs/REGISTER.json gibt es keinen");
            log.note("Prüfanker: dieser Befehl braucht das Repository.");
            return false;
        }
    };

    let mut alle_bereit = true;
    for m in &bekannt {
        log.note(format!("{} (θ_v {})", m.name, m.theta_v));
        match pruefen(&repo, m) {
            Zustand::Bereit { pfad } => {
                log.note(format!("  bereit. Digest stimmt: {}", &m.digest[..16]));
                log.note(format!("  {}", pfad.display()));
            }
            Zustand::Abweichend { pfad, ist, soll } => {
                alle_bereit = false;
                log.error(format!("  Digest weicht ab in {}", pfad.display()));
                log.error(format!("    hier:          {}", ist));
                log.error(format!("    veröffentlicht: {}", soll));
                log.error("  Das ist KEIN Hardware-Befund. Hier liegt ein anderes");
                log.error("  Modell als beim Vergleichspartner; ein Bitgleichheits-");
                log.error("  test darüber hätte keine Aussage. Artefakte neu bauen:");
                for zeile in bauanleitung(&repo, &m.name).lines() {
                    log.note(format!("  {}", zeile));
                }
            }
            Zustand::Fehlt => {
                alle_bereit = false;
                log.note("  keine Artefakte auf dieser Maschine.");
                for zeile in bauanleitung(&repo, &m.name).lines() {
                    log.note(format!("  {}", zeile));
                }
            }
        }
    }
    alle_bereit
}


/// Eingabefunktion für `artefakte::beschaffen`.
///
/// `None` bei `--quiet`, dann läuft der Client nicht-interaktiv, und
/// `beschaffen` lädt bewusst nichts herunter. Ein mehrere Gigabyte großer
/// Zugriff auf einen fremden Dienst gehört nicht in einen Skriptlauf, der
/// ihn nicht angefordert hat.
fn stdin_frage(interaktiv: bool) -> Option<impl FnMut(&str) -> Option<String>> {
    if !interaktiv {
        return None;
    }
    Some(|prompt: &str| {
        use std::io::Write;
        print!("{}", prompt);
        let _ = std::io::stdout().flush();
        let mut zeile = String::new();
        match std::io::stdin().read_line(&mut zeile) {
            Ok(0) | Err(_) => None, // EOF: wie eine leere Eingabe behandeln
            Ok(_) => Some(zeile),
        }
    })
}
