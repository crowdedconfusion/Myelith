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
                i += 2;
            }
            "--logs" => {
                a.logs = PathBuf::from(need(i, "--logs")?);
                i += 2;
            }
            "--plan" => {
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
