//! `myl-test` — Terminal-Testclient für Myelith.
//!
//! Argumentauswertung von Hand, ohne Fremd-Crate: Der Client soll auf
//! einer fremden Maschine mit möglichst wenig Voraussetzungen bauen.

use std::path::PathBuf;
use std::process::ExitCode;

use myl_testclient::menu::{self, Einstellungen};
use myl_testclient::{
    banner, default_artifact_dir, default_log_dir, run_determinism, run_hardware, run_shard,
    run_stack, RunLog,
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
    menu            Interaktives Menü (wie ohne Befehl).

OPTIONEN
    --prompt <TEXT>     Eingabetext (Vorgabe: \"Die Hauptstadt von Frankreich ist\")
    --steps <N>         Zu erzeugende Token (Vorgabe: 8)
    --shards <N>        Anzahl Shards für `shard` (Vorgabe: 4)
    --artifacts <PFAD>  Artefaktverzeichnis (Vorgabe: qwen2.5-0.5b)
    --logs <PFAD>       Protokollverzeichnis (Vorgabe: TESTCLIENT/myl-testclient/logs)
    --quiet             Nur ins Protokoll schreiben, nicht aufs Terminal
                        (unterdrückt auch das Banner)
    -h, --help          Diese Hilfe

    Umgebung: MYL_NO_BANNER=1 unterdrückt das Banner dauerhaft.

PROTOKOLLE
    Jeder Lauf schreibt <lauf-id>.jsonl (maschinenlesbar, für den
    Vergleich zwischen Maschinen) und <lauf-id>.log (Fließtext).
    Prompttexte werden gehasht, nicht gespeichert.

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
        }
    }
}

fn parse() -> Result<Args, String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-h" || a == "--help") {
        return Err(String::new());
    }
    // Ohne Unterbefehl (oder nur mit Optionen) ins Menü.
    if raw.is_empty() || raw[0].starts_with('-') {
        let mut a = Args {
            command: "menu".to_string(),
            ..Args::vorgaben()
        };
        a.quiet = raw.iter().any(|x| x == "--quiet");
        return Ok(a);
    }

    let mut a = Args {
        command: raw[0].clone(),
        ..Args::vorgaben()
    };

    let mut i = 1;
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
            "--quiet" => {
                a.quiet = true;
                i += 1;
            }
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unbekannte Option: {}", other)),
        }
    }

    if a.steps == 0 {
        return Err("--steps muss > 0 sein".into());
    }
    Ok(a)
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

    if args.command == "menu" {
        let ok = menu::run(Einstellungen {
            prompt: args.prompt,
            steps: args.steps,
            shards: args.shards,
            artifacts: args.artifacts,
            logs: args.logs,
        });
        return if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    }

    let echo = !args.quiet;
    banner::print_if(echo);
    let mut log = RunLog::new(&args.logs, &args.command, echo);

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
