//! `myl-test` — Terminal-Testclient für Myelith.
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
myl-test — Testclient für Myelith

AUFRUF
    myl-test                 ohne Befehl: interaktives Menü
    myl-test <BEFEHL> [OPTIONEN]

BEFEHLE
    hardware        Hardware erheben und protokollieren. Braucht kein
                    Modell — der erste Befehl auf einer neuen Maschine.
    determinismus   Denselben Prompt zweimal rechnen und auf Bitgleichheit
                    prüfen. Der Vergleichswert im Protokoll muss auf jeder
                    Maschine derselbe sein.
    shard           Geshardete Inferenz über einen Pod fahren und gegen
                    die Einzelknoten-Runtime prüfen.
    artefakte       Modelle auf dieser Maschine prüfen: sind Artefakte da,
                    und stimmen sie mit dem veröffentlichten Digest überein?
                    Der erste Befehl vor jedem Vergleichslauf — ohne ihn
                    sähe ein abweichendes Artefakt wie eine gescheiterte
                    Hardware-Bitgleichheit aus.

    stack           Protokoll-Durchlauf über Krypto, Epochenseed,
                    Komiteewahl, BFT, Verifikation, Ledger und Tokenomics.
                    Braucht kein Modell.
    plan            Testplan erzeugen (Koordinator) — die Datei, die an
                    alle Teilnehmer geht. Siehe TESTPLAN unten.
    menu            Interaktives Menü (wie ohne Befehl).

OPTIONEN
    --prompt <TEXT>     Eingabetext (Vorgabe: \"Die Hauptstadt von Frankreich ist\")
    --steps <N>         Zu erzeugende Token (Vorgabe: 8)
    --shards <N>        Anzahl Shards für `shard` (Vorgabe: 4)
    --plan <DATEI>      Testplan laden. Setzt Prompt, Token, Shards und
                        Modell und prüft die Datei gegen ihre Prüfsumme.
    --artifacts <PFAD>  Artefaktverzeichnis (Vorgabe: qwen2.5-0.5b)
    --plan-id <TEXT>    Kennung beim Erzeugen eines Plans
    --out <DATEI>       Zieldatei beim Erzeugen eines Plans
    --logs <PFAD>       Protokollverzeichnis (Vorgabe: TESTCLIENT/myl-testclient/logs)
    --quiet             Nur ins Protokoll schreiben, nicht aufs Terminal
                        (unterdrückt auch das Banner)
    -h, --help          Diese Hilfe
ARTEFAKTE
    `determinismus` und `shard` brauchen ein Modell. Ohne `--artifacts`
    sucht der Client selbst: Findet er eines, nimmt er es; findet er
    mehrere, fragt er; findet er keines, bietet er an, die Gewichte von
    Hugging Face zu holen und die Artefakte zu bauen. Der Bau nutzt das
    versionierte Skalenpaket und dauert Sekunden.

    Bei `--quiet` wird nicht gefragt und deshalb auch nichts geladen —
    ein mehrere Gigabyte großer Zugriff gehört nicht in einen Skriptlauf,
    der ihn nicht angefordert hat.

    Umgebung: MYL_NO_BANNER=1 unterdrückt das Banner dauerhaft.

PROTOKOLLE
    Jeder Lauf schreibt <lauf-id>.jsonl (maschinenlesbar, für den
    Vergleich zwischen Maschinen) und <lauf-id>.log (Fließtext).
    Prompttexte werden gehasht, nicht gespeichert.

TESTPLAN
    Koordinator erzeugt und verschickt:
        myl-test plan --plan-id 2026-08-18-arch --prompt \"...\" --steps 8
        → schreibt <plan-id>.plan

    Teilnehmer verwenden ihn:
        myl-test --plan 2026-08-18-arch.plan determinismus

    Die Datei trägt eine Prüfsumme über Prompt, Token, Shards und
    Modell. Wird sie verändert, verweigert der Client den Lauf — ein
    Tippfehler soll nicht als Befund durchgehen.

PROTOKOLL-ABLAGE
    logs/<befehl>/<datum>_<einstellungs-id>/<uhrzeit>-<hardware>.jsonl
    Alle Teilnehmer mit demselben Plan landen im gleichnamigen Ordner.

CROSS-HARDWARE-NACHWEIS
    1. Auf jeder Maschine:  myl-test hardware
       → die Fingerabdrücke MÜSSEN sich unterscheiden.
    2. Auf jeder Maschine:  myl-test determinismus --prompt \"...\"
       → die Digests MÜSSEN übereinstimmen.
    Beides zusammen ist der Nachweis; eines allein ist keiner.
";

struct Args {
    command: String,
    prompt: String,
    steps: usize,
    shards: usize,
    artifacts: PathBuf,
    /// Wurde `--artifacts` (oder ein Plan) ausdrücklich gesetzt? Dann wird
    /// nicht gesucht und nichts beschafft — eine ausdrückliche Angabe hat
    /// Vorrang vor jeder Automatik.
    artifacts_explizit: bool,
    logs: PathBuf,
    quiet: bool,
    plan: Option<PathBuf>,
    plan_id: Option<String>,
    out: Option<PathBuf>,
}

impl Args {
    fn vorgaben() -> Self {
        Self {
            command: String::new(),
            prompt: "Die Hauptstadt von Frankreich ist".to_string(),
            steps: 8,
            shards: 4,
            artifacts: default_artifact_dir(),
            artifacts_explizit: false,
            logs: default_log_dir(),
            quiet: false,
            plan: None,
            plan_id: None,
            out: None,
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

    // Optionen dürfen VOR und NACH dem Befehl stehen — `myl-test --plan x
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
            "--prompt" => {
                a.prompt = need(i, "--prompt")?;
                i += 2;
            }
            "--steps" => {
                a.steps = need(i, "--steps")?
                    .parse()
                    .map_err(|_| "--steps erwartet eine Zahl".to_string())?;
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
    Ok(a)
}

fn plan_erzeugen(args: &Args) -> ExitCode {
    let plan = TestPlan {
        plan_id: args
            .plan_id
            .clone()
            .unwrap_or_else(|| "unbenannt".to_string()),
        prompt: args.prompt.clone(),
        steps: args.steps,
        shards: args.shards,
        model: myl_testclient::DEFAULT_MODEL.to_string(),
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
    println!("  Prompt         {:?}", plan.prompt);
    println!("  Token          {}", plan.steps);
    println!("  Shards         {}", plan.shards);
    println!("  Modell         {}", plan.model);
    println!("  Einstellungs-ID {}", plan.short_id());
    println!();
    println!("Diese Datei unverändert an alle Teilnehmer schicken. Sie starten damit:");
    println!("    myl-test --plan {} determinismus", ziel.display());
    println!();
    println!("Alle Protokolle landen dann unter");
    println!(
        "    logs/<befehl>/<datum>_{}/  —  ohne Zuordnungsarbeit vergleichbar.",
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

    // Testplan laden, falls angegeben — er überschreibt die Optionen.
    let mut args = args;
    let mut einstellungen_id = "ohne-plan".to_string();
    if let Some(pfad) = args.plan.clone() {
        match TestPlan::load(&pfad) {
            Ok(plan) => {
                println!("Testplan: {} ({})", plan.plan_id, pfad.display());
                println!("  Einstellungs-ID {}\n", plan.short_id());
                einstellungen_id = plan.short_id();
                args.prompt = plan.prompt;
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

    if args.command == "menu" {
        let ok = menu::run(Einstellungen {
            prompt: args.prompt,
            steps: args.steps,
            shards: args.shards,
            artifacts: args.artifacts,
            logs: args.logs,
            einstellungen_id,
        });
        return if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    }

    let echo = !args.quiet;
    banner::print_if(echo);
    let hardware = myl_testclient::Fingerprint::collect().short_id();
    let mut log = RunLog::mit_ziel(
        LogZiel::neu(&args.logs, &args.command, &einstellungen_id, &hardware),
        echo,
    );

    // Artefakte auflösen, bevor ein Lauf sie braucht. `hardware`, `stack`
    // und `artefakte` kommen ohne Modell aus — sie werden übersprungen,
    // damit der erste Befehl auf einer neuen Maschine keinen Download
    // auslöst.
    let braucht_modell = matches!(args.command.as_str(), "determinismus" | "shard");
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
        "determinismus" => run_determinism(&mut log, &args.artifacts, &args.prompt, args.steps),
        "shard" => run_shard(
            &mut log,
            &args.artifacts,
            &args.prompt,
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

    if log.finish(ok) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `artefakte` — prüft für jedes bekannte Modell, ob es auf dieser
/// Maschine vorliegt und ob es dem veröffentlichten Digest entspricht.
///
/// Ein abweichender Digest ist **kein Hardware-Befund**. Er heißt, dass
/// hier ein anderes Modell liegt als beim Vergleichspartner — und ein
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
            log.note("Prüfanker — dieser Befehl braucht das Repository.");
            return false;
        }
    };

    let mut alle_bereit = true;
    for m in &bekannt {
        log.note(format!("{} (θ_v {})", m.name, m.theta_v));
        match pruefen(&repo, m) {
            Zustand::Bereit { pfad } => {
                log.note(format!("  bereit — Digest stimmt: {}", &m.digest[..16]));
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
                for zeile in bauanleitung(&m.name).lines() {
                    log.note(format!("  {}", zeile));
                }
            }
            Zustand::Fehlt => {
                alle_bereit = false;
                log.note("  keine Artefakte auf dieser Maschine.");
                for zeile in bauanleitung(&m.name).lines() {
                    log.note(format!("  {}", zeile));
                }
            }
        }
    }
    alle_bereit
}


/// Eingabefunktion für `artefakte::beschaffen`.
///
/// `None` bei `--quiet` — dann läuft der Client nicht-interaktiv, und
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
