//! Die Prüfläufe des Testclients.
//!
//! Drei Unterbefehle, jeder mit demselben Aufbau: Fingerabdruck und
//! Artefakt-Identität ins Protokoll, dann messen, dann ein
//! **Vergleichswert (Digest)**, der zwischen Maschinen gediffed wird.
//!
//! Der Digest ist überall SHA-256 über eine kanonische Bytefolge: nie
//! über eine formatierte Ausgabe. Formatierung ändert sich, Bytes nicht.
//!
//! ## Fund 36 (2026-08-22): Der Digest maß Token-Gleichheit, nicht Bitgleichheit
//!
//! `greedy_digest` hashte bis zu diesem Datum **nur die erzeugten
//! Token**. Ein Token ist ein Argmax, also eine Entscheidung zwischen
//! 151 936 Zahlen: Er ändert sich erst, wenn die Rangfolge kippt. Die
//! Zahlen selbst können vorher beliebig abweichen.
//!
//! **Gemessen** an Qwen2.5-0,5B, indem Bytes eines einzelnen Tensors
//! (`layers.0.self_attn.q_proj`, 802 816 Byte) um je eins verschoben und
//! die Hashkette bis `theta_v.json` konsistent nachgezogen wurde. Das
//! Modell war danach jedesmal ein anderes und lud fehlerfrei:
//!
//! | geänderte Bytes | Anteil | Digest über Token | Digest über Logits |
//! |---|---|---|---|
//! | 9 | 0,0011 % | **unverändert** | verändert |
//! | 81 | 0,0101 % | **unverändert** | verändert |
//! | 803 | 0,1 % | **unverändert** | verändert |
//! | 8029 | 1,0 % | verändert | verändert |
//!
//! In drei von vier Stufen rechnete das Modell nachweislich andere
//! Zahlen, und der Vergleichswert meldete „bitgleich".
//!
//! **Warum das den Kernbeleg des Projekts betrifft:** Der
//! Cross-Hardware-Nachweis behauptet, zwei Architekturen rechneten
//! *dasselbe*. Geprüft wurde, ob sie *dieselbe Entscheidung treffen*.
//! Eine Maschine mit abweichender Ganzzahlarithmetik hätte erst dann
//! auffallen müssen, wenn die Abweichung groß genug war, ein Argmax zu
//! kippen. Genau die kleinen Abweichungen, gegen die dieses Projekt
//! gebaut ist, wären durchgerutscht.
//!
//! Der Digest deckt jetzt die **Logits jedes Schritts** ab, dazu weiter
//! den gewählten Token. Logits sind hier `i32`: Sie zu hashen ist exakt
//! und bleibt in der Ganzzahldisziplin, anders als bei einem
//! Gleitkommamodell, wo dieser Weg gar nicht offenstünde.
//!
//! **Alte Protokolle sind damit nicht mehr vergleichbar.** Deshalb trägt
//! jedes Protokoll den Umfang des Digests als eigenes Feld
//! (`digest_umfang`), und `vergleich` behandelt zwei verschiedene Umfänge
//! wie zwei verschiedene Modellstände: unvergleichbar, und ausdrücklich
//! kein Hardware-Befund.

use std::path::Path;
use std::sync::Arc;

use integer_llm_runtime::loader;
use integer_llm_runtime::model::IntegerModel;
use myl_pod::coordinator::Coordinator;
use myl_pod::da::{DaStore, ReedSolomonCoder};
use myl_pod::shard::ShardNode;
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};

use crate::hardware::Fingerprint;
use crate::logging::{sha256_hex, Event, RunLog};

/// Kodiert den Prompt mit dem Tokenizer aus dem Artefaktverzeichnis.
fn encode_prompt(artifact_dir: &Path, prompt: &str) -> Result<Vec<u32>, String> {
    let path = artifact_dir.join("tokenizer.json");
    let path_str = path
        .to_str()
        .ok_or_else(|| format!("Tokenizer-Pfad nicht darstellbar: {}", path.display()))?;
    let tok = integer_llm_runtime::tokenizer::Tokenizer::from_file(path_str)
        .map_err(|e| format!("Tokenizer nicht ladbar ({}): {}", path.display(), e))?;
    Ok(tok.encode(prompt).iter().map(|t| *t as u32).collect())
}

/// Dekodiert Token zu Klartext, für die Anzeige.
///
/// Schlägt es fehl, wird das gemeldet statt abgebrochen: Der Klartext ist
/// eine Zugabe fürs Zuschauen, der Digest bleibt der eigentliche Nachweis.
fn decode_tokens(artifact_dir: &Path, tokens: &[u32]) -> Option<String> {
    let path = artifact_dir.join("tokenizer.json");
    let tok = integer_llm_runtime::tokenizer::Tokenizer::from_file(path.to_str()?).ok()?;
    Some(tok.decode(&tokens.iter().map(|t| *t as usize).collect::<Vec<_>>()))
}

/// Artefaktverzeichnis, gegen das gemessen wird.
///
/// `integer_llm_runtime::paths` löst relativ zum **Arbeitsverzeichnis**
/// auf, das passt für Läufe aus `INTEGER_LLM/`, nicht für einen Client,
/// der von überall gestartet wird. Deshalb hier absolut, ausgehend vom
/// Ort dieses Crates. Die Umgebungsvariable `INTEGER_LLM_ARTIFACTS_DIR`
/// hat weiterhin Vorrang, damit ein Testlauf auf fremde Artefakte
/// gerichtet werden kann, ohne den Client neu zu bauen.
pub fn default_artifact_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var(integer_llm_runtime::paths::ARTIFACTS_DIR_ENV) {
        if !dir.is_empty() {
            return std::path::PathBuf::from(dir).join(DEFAULT_MODEL);
        }
    }
    repo_root()
        .join("INTEGER_LLM")
        .join(integer_llm_runtime::paths::ARTIFACTS_DIR)
        .join(DEFAULT_MODEL)
}

/// Modellname des Standard-Artefakts.
pub const DEFAULT_MODEL: &str = "qwen2.5-0.5b";

/// Repository-Wurzel, **zur Laufzeit** gesucht.
///
/// Der Übersetzungspfad dient nur noch als letzter Ausweg; Begründung in
/// [`crate::artefakte::wurzel_zur_laufzeit`].
fn repo_root() -> std::path::PathBuf {
    let gebaut = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    crate::artefakte::wurzel_zur_laufzeit(&gebaut)
}

/// Schreibt Fingerabdruck und Artefakt-Identität ins Protokoll.
///
/// Beides gehört an den **Anfang** jedes Laufs: Wer ein Protokoll
/// aufmacht, muss zuerst wissen, worauf gemessen wurde, bevor er die
/// Zahlen liest.
/// Bricht einen Messlauf ab, wenn der Bau für ein Backend konfiguriert
/// ist, das nicht rechnet.
///
/// Siehe [`crate::hardware::rechenpfad_pruefen`]. Aufgerufen von den
/// Läufen mit Modell; `hardware` und `stack` sind nicht betroffen, denn
/// sie rechnen nichts, dessen Backend eine Rolle spielte.
fn backend_taugt(log: &mut RunLog) -> bool {
    match crate::hardware::rechenpfad_pruefen() {
        Ok(()) => true,
        Err(begruendung) => {
            for zeile in begruendung.lines() {
                log.error(zeile.to_string());
            }
            false
        }
    }
}

/// Was in den Vergleichswert eines Modelllaufs eingeht.
///
/// Wird als Feld protokolliert und beim Vergleich geprüft. Ändert sich,
/// was gehasht wird, ändert sich diese Zeichenkette mit: Zwei Protokolle
/// mit verschiedenem Umfang messen verschiedene Dinge und dürfen nicht
/// gegeneinander geurteilt werden.
///
/// `token` war der Stand bis Fund 36 (2026-08-22).
pub const DIGEST_UMFANG: &str = "logits+token";

fn log_context(log: &mut RunLog, artifact_dir: Option<&Path>) {
    let fp = Fingerprint::collect();
    for (k, v) in &fp.entries {
        log.event(Event::Hardware {
            key: k.clone(),
            value: v.clone(),
        });
    }
    log.event(Event::Hardware {
        key: "fingerprint_sha256".into(),
        value: sha256_hex(&fp.canonical_bytes()),
    });

    // **Was der Vergleichswert überhaupt abdeckt.** Steht bei der
    // Hardware und nicht beim Artefakt, weil es eine Eigenschaft des
    // Messverfahrens ist und auch für Läufe ohne Modell gilt. Ohne dieses
    // Feld ließe sich ein Protokoll von vor Fund 36 nicht von einem
    // danach unterscheiden, und der Vergleich meldete eine Abweichung,
    // wo nur zwei verschiedene Dinge gemessen wurden.
    log.event(Event::Hardware {
        key: "digest_umfang".into(),
        value: DIGEST_UMFANG.to_string(),
    });

    let Some(dir) = artifact_dir else { return };
    log.event(Event::Artifact {
        key: "artifact_dir".into(),
        value: dir.display().to_string(),
    });
    if !dir.exists() {
        log.note(format!(
            "Artefaktverzeichnis fehlt: {}. Modellläufe werden übersprungen",
            dir.display()
        ));
        return;
    }
    log.event(Event::Artifact {
        key: "modell".into(),
        value: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    });

    // Modellstand vor Modellmaßen: Dimensionen unterscheiden 0,5B von 7B,
    // aber zwei θ_v-Stände desselben Modells sehen darin gleich aus. Ein
    // Digest-Vergleich zwischen Modellständen ist ohne diese Werte nicht
    // einzuordnen: bei einem θ_v-Wechsel ändern sich die Digests
    // zwangsläufig, und die Frage ist dann nicht „gleich oder nicht",
    // sondern „erwartet oder nicht".
    match loader::ThetaV::load_from_dir(dir) {
        Ok(t) => {
            log.event(Event::Artifact {
                key: "theta_v".into(),
                value: t.version,
            });
            log.event(Event::Artifact {
                key: "weights_hash".into(),
                value: t.weights_hash,
            });
            log.event(Event::Artifact {
                key: "scales_hash".into(),
                value: t.scales_hash,
            });
            log.event(Event::Artifact {
                key: "luts_hash".into(),
                value: t.luts_hash,
            });
        }
        Err(e) => log.note(format!("theta_v.json nicht lesbar: {}", e)),
    }

    // Der Ankerdigest ist derselbe Wert, den `artefakte` gegen das
    // veröffentlichte Register hält. Im Protokoll beantwortet er die Frage,
    // die bei einer Abweichung zuerst zu stellen ist: Lief der Vergleich
    // überhaupt über dasselbe Artefakt?
    match crate::artefakte::artefakt_digest(dir) {
        Ok((digest, _)) => log.event(Event::Artifact {
            key: "artefakt_digest".into(),
            value: digest,
        }),
        Err(e) => log.note(format!("Ankerdigest nicht berechenbar: {}", e)),
    }

    match loader::load_model_dims(dir) {
        Ok(d) => {
            log.event(Event::Artifact {
                key: "hidden_size".into(),
                value: d.hidden_size.to_string(),
            });
            log.event(Event::Artifact {
                key: "num_layers".into(),
                value: d.num_layers.to_string(),
            });
            log.event(Event::Artifact {
                key: "num_heads".into(),
                value: d.num_heads.to_string(),
            });
            log.event(Event::Artifact {
                key: "vocab_size".into(),
                value: d.vocab_size.to_string(),
            });
        }
        Err(e) => log.note(format!("Modelldimensionen nicht lesbar: {}", e)),
    }
}

/// `myl-test hardware`: nur erheben und protokollieren.
///
/// Der schnellste Weg, eine fremde Maschine in den Vergleich
/// aufzunehmen: kein Modell nötig, kein Artefakt nötig.
pub fn run_hardware(log: &mut RunLog) -> bool {
    log_context(log, None);
    let fp = Fingerprint::collect();
    log.result(
        "hardware_fingerprint",
        &sha256_hex(&fp.canonical_bytes()),
        fp.short_id(),
    );
    log.note(
        "Für den Cross-Hardware-Nachweis diesen Lauf auf jeder Maschine \
         ausführen und die Fingerabdrücke vergleichen: sie MÜSSEN sich \
         unterscheiden, sonst prüft der Determinismustest nichts.",
    );
    true
}

/// `myl-test determinismus`: jeder Prompt zweimal, bitgleich?
///
/// Prüft die Kerneigenschaft aus Whitepaper Kap. 6.2 lokal: Zwei
/// unabhängige Läufe im selben Prozess müssen bitgleiche Logits
/// liefern. Der eigentliche Nachweis entsteht erst, wenn dieser Lauf auf
/// **verschiedener** Hardware denselben Digest liefert: deshalb steht
/// der Fingerabdruck im selben Protokoll.
///
/// ## Warum mehrere Prompts
///
/// Ein einzelner Prompt übt einen einzigen Pfad durch das Modell aus. Ein
/// Rundungsfehler, der nur bei langen Sequenzen, nur bei bestimmten Token
/// oder nur in einem selten getroffenen LUT-Bereich auftritt, bleibt dann
/// unentdeckt, und der Vergleichswert sähe trotzdem beruhigend aus. Der
/// Testplan gibt deshalb eine **Reihe** von Prompts vor.
///
/// Das Modell wird dafür **einmal** geladen und über alle Prompts
/// wiederverwendet: Bei 7B dauert das Laden ein Vielfaches der Messung.
///
/// ## Zwei Ebenen von Vergleichswerten
///
/// Je Prompt entsteht `determinismus_<n>`, darüber ein Gesamtwert
/// `determinismus` als Digest über alle Einzelwerte in ihrer Reihenfolge.
/// Der Gesamtwert ist die Zahl, die zwischen Maschinen verglichen wird;
/// die Einzelwerte sagen, **welcher** Prompt auseinanderläuft, wenn er
/// abweicht.
pub fn run_determinism(
    log: &mut RunLog,
    artifact_dir: &Path,
    prompts: &[String],
    steps: usize,
    wiederholungen: usize,
) -> bool {
    log_context(log, Some(artifact_dir));
    if !backend_taugt(log) {
        return false;
    }

    // **Vor der Artefaktprüfung**, wie die Backend-Sperre und aus
    // demselben Grund: Es ist ein Argumentfehler und steht fest, bevor
    // irgendein Modell gebraucht wird. Ein einzelner Lauf kann nichts
    // über Bitgleichheit sagen, er hat nichts, womit er sich vergleichen
    // ließe. Zwei ist die Vorgabe und das Minimum; mehr sind für
    // Langläufe gedacht (Fahrplanpunkt 2.4).
    if wiederholungen < 2 {
        log.error(format!(
            "Wiederholungen muss >= 2 sein, angegeben: {wiederholungen}. \
             Ein einzelner Lauf prüft keine Bitgleichheit."
        ));
        return false;
    }

    if !artifact_dir.exists() {
        log.error(format!(
            "Artefaktverzeichnis {} fehlt. Determinismuslauf nicht möglich",
            artifact_dir.display()
        ));
        return false;
    }
    if prompts.is_empty() {
        log.error("Kein Prompt angegeben");
        return false;
    }
    let model = match log.timed("modell_laden", "", || loader::load_model(artifact_dir)) {
        Ok(m) => m,
        Err(e) => {
            log.error(format!("Modell nicht ladbar: {}", e));
            return false;
        }
    };

    let mut alle_gleich = true;
    let mut einzelwerte: Vec<String> = Vec::with_capacity(prompts.len());

    for (nr, prompt) in prompts.iter().enumerate() {
        let nr = nr + 1;
        let ids = match encode_prompt(artifact_dir, prompt) {
            Ok(v) => v,
            Err(e) => {
                log.error(e);
                return false;
            }
        };
        log.event(Event::PromptAccepted {
            token_count: ids.len(),
            prompt_sha256: sha256_hex(prompt.as_bytes()),
        });
        if ids.is_empty() {
            log.error(format!("Prompt {} ergibt null Token", nr));
            return false;
        }

        let mut digests = Vec::new();
        for lauf in 1..=wiederholungen {
            let d = log.timed(&format!("prompt_{}_lauf_{}", nr, lauf), "", || {
                greedy_digest(&model, &ids, steps)
            });
            log.result(&format!("prompt_{}_lauf_{}", nr, lauf), &d.0, d.1.clone());
            digests.push(d);
        }

        // Klartext nur auf das Terminal. Im Protokoll stehen Token und
        // Digest; daraus ist der Text ableitbar, und die Datei bleibt schlank.
        if let Some(text) = decode_tokens(artifact_dir, &digests[0].2) {
            log.nur_anzeigen("");
            log.nur_anzeigen(format!("  Prompt {}:  {}", nr, prompt));
            log.nur_anzeigen(format!("  Antwort:    {}", text.trim_end()));
            log.nur_anzeigen("");
        }

        // **Gegen den ersten Lauf, nicht paarweise gegen den Vorgänger.**
        // Ein Wackler in der Mitte einer langen Reihe fiele sonst zweimal
        // auf und beim Zurückkehren auf den richtigen Wert gar nicht mehr.
        match digests.iter().position(|d| d.0 != digests[0].0) {
            None => log.result(
                &format!("determinismus_{}", nr),
                &digests[0].0,
                format!("bitgleich über {} Läufe", wiederholungen),
            ),
            Some(abweichend) => {
                alle_gleich = false;
                log.event(Event::Mismatch {
                    name: format!("determinismus_{}", nr),
                    expected: digests[0].0.clone(),
                    actual: digests[abweichend].0.clone(),
                });
                // Die Nummer des ersten abweichenden Laufs ist die
                // eigentliche Diagnose: Lauf 2 deutet auf einen Fehler im
                // Code, Lauf 40 nach 39 gleichen eher auf die Maschine
                // (Speicher, Temperatur).
                log.error(format!(
                    "Prompt {}: Lauf {} von {} weicht vom ersten ab. \
                     Läufe 1 bis {} waren identisch.",
                    nr,
                    abweichend + 1,
                    wiederholungen,
                    abweichend
                ));
            }
        }
        einzelwerte.push(digests[0].0.clone());
    }

    let gesamt = digest_ueber(&einzelwerte);
    log.result(
        "determinismus",
        &gesamt,
        format!("{} Prompts, je {} Läufe", prompts.len(), wiederholungen),
    );
    log.note(format!(
        "Vergleichswert für andere Maschinen: {}: bei gleichem Testplan und \
         gleichem θ_v MUSS er übereinstimmen, unabhängig von Architektur \
         und Backend.",
        gesamt
    ));
    alle_gleich
}

/// Digest über eine geordnete Reihe von Digests.
///
/// Die Reihenfolge geht ein: Dieselben Prompts in anderer Folge sind ein
/// anderer Testplan, und der Vergleichswert muss das zeigen. Getrennt
/// werden die Einzelwerte durch `\n`, das in einem Hexdigest nicht
/// vorkommt, ohne Trenner ließen sich zwei Reihen konstruieren, die
/// dieselbe Bytefolge ergeben.
fn digest_ueber(werte: &[String]) -> String {
    sha256_hex(werte.join("\n").as_bytes())
}

/// Greedy-Dekodierung; liefert (Digest, Kurzbeschreibung).
///
/// Der Digest deckt **alle** erzeugten Token ab, nicht nur das letzte:
/// Ein Unterschied in Schritt 3, der sich in Schritt 7 wieder ausgleicht,
/// wäre sonst unsichtbar.
/// Freie Inferenz: ein Prompt hinein, der erzeugte Text heraus.
///
/// **Kein Protokoll, kein Digest, kein Urteil.** Alles andere in diesem
/// Client misst; dieser Weg zeigt. Wer eine Maschine für einen
/// Cross-Hardware-Test beisteuert, hat berechtigtes Interesse daran, was
/// er da eigentlich rechnen lässt, und ein Modell, mit dem man einmal
/// gesprochen hat, ist kein abstraktes Artefakt mehr. Ein Protokoll
/// darüber wäre irreführend: Es sähe aus wie ein Messergebnis und wäre
/// keines, denn Prompt und Tokenzahl bestimmt der Nutzer frei.
///
/// **Bit-exakt bleibt es trotzdem**, denn es ist derselbe Rechenweg wie in
/// [`run_determinism`]: gierige Auswahl, kein Sampling, kein Zufall.
/// Derselbe Prompt liefert auf derselben θ_v dieselbe Antwort, hier wie
/// dort. Genau deshalb gibt es hier keine Temperatur einzustellen.
pub fn antworten(
    model: &IntegerModel,
    artifact_dir: &Path,
    prompt: &str,
    steps: usize,
    ausgabe: &mut dyn FnMut(&str),
) -> Result<String, String> {
    let ids = encode_prompt(artifact_dir, prompt)?;
    if ids.is_empty() {
        return Err("Der Prompt ergibt null Token.".to_string());
    }

    // Der Tokenizer wird **einmal** geladen, nicht je Token. Bei 64 Token
    // wären es sonst 64 Ladevorgänge, und die Ausgabe stockte im Takt der
    // Datei statt im Takt der Rechnung.
    let pfad = artifact_dir.join("tokenizer.json");
    let tok = integer_llm_runtime::tokenizer::Tokenizer::from_file(
        pfad.to_str()
            .ok_or_else(|| format!("Tokenizer-Pfad nicht darstellbar: {}", pfad.display()))?,
    )
    .map_err(|e| format!("Tokenizer nicht ladbar ({}): {}", pfad.display(), e))?;

    let mut cache =
        integer_llm_runtime::kv_cache::KVCache::new(model.num_layers, model.num_kv_heads);
    let mut logits = Vec::new();
    for (pos, &t) in ids.iter().enumerate() {
        logits = model.forward_token(t as usize, pos, &mut cache);
    }

    let mut erzeugt: Vec<usize> = Vec::with_capacity(steps);
    let mut gezeigt = String::new();
    let start = ids.len();
    for schritt in 0..steps {
        let next = model.greedy_next(&logits);
        erzeugt.push(next);
        logits = model.forward_token(next, start + schritt, &mut cache);

        // **Den ganzen Strom neu dekodieren, nicht das einzelne Token.**
        // Ein Token ist bei BPE oft kein vollständiges Zeichen und schon
        // gar kein Wort; einzeln dekodiert entstünden Bruchstücke und
        // kaputte Umlaute. Der Text wächst dagegen monoton, und was neu
        // hinzugekommen ist, ist die Differenz zum bereits Gezeigten.
        // 64 Dekodierungen einer kurzen Folge kosten nichts gegen die
        // Inferenz selbst.
        let jetzt = tok.decode(&erzeugt);
        if let Some(neu) = jetzt.strip_prefix(gezeigt.as_str()) {
            if !neu.is_empty() {
                ausgabe(neu);
            }
            gezeigt = jetzt;
        }
        // Wächst der Text ausnahmsweise nicht monoton (der Dekodierer darf
        // ein angefangenes Zeichen zurücknehmen), wird dieser Schritt
        // übersprungen und beim nächsten nachgeholt. Lieber ein Token
        // später als ein zerrissenes Zeichen.
    }

    let vollstaendig = tok.decode(&erzeugt);
    if let Some(rest) = vollstaendig.strip_prefix(gezeigt.as_str()) {
        if !rest.is_empty() {
            ausgabe(rest);
        }
    }
    Ok(vollstaendig)
}

/// Lädt das Modell einmal, damit mehrere Fragen es teilen können.
///
/// Getrennt vom Rechnen, weil das Laden bei 7B rund eine Minute dauert.
/// Für jede Frage neu zu laden hieße, den Nutzer für jede Antwort eine
/// Minute warten zu lassen; im Gespräch ist das der Unterschied zwischen
/// benutzbar und nicht benutzbar.
pub fn modell_laden(artifact_dir: &Path) -> Result<IntegerModel, String> {
    if !artifact_dir.exists() {
        return Err(format!(
            "Artefaktverzeichnis {} fehlt.",
            artifact_dir.display()
        ));
    }
    loader::load_model(artifact_dir).map_err(|e| format!("Modell nicht ladbar: {}", e))
}

fn greedy_digest(model: &IntegerModel, ids: &[u32], steps: usize) -> (String, String, Vec<u32>) {
    // **Eine Fassung, nicht zwei.** Die Bytefolge des Digests steht in
    // `runtime::generate::dekodieren_mit_digest`; hier stand bis
    // 2026-08-22 eine eigene, und zwei Fassungen derselben Aussage sind
    // genau die Lage, aus der Fund 34 entstand. Nachgemessen, bevor die
    // Kopie wich: beide lieferten für denselben Prompt
    // `df54ef6c89f1a840…`.
    let token_ids: Vec<usize> = ids.iter().map(|&t| t as usize).collect();
    let (out, digest) = integer_llm_runtime::generate::dekodieren_mit_digest(
        model, &token_ids, steps, 0, true,
    );
    let out: Vec<u32> = out.into_iter().map(|t| t as u32).collect();
    let beschreibung = format!("{} Token: {:?}", out.len(), &out[..out.len().min(8)]);
    (digest, beschreibung, out)
}

/// `myl-test shard`, die erste geshardete Inferenz.
///
/// Fährt einen Pod aus `num_shards` Shards über die myl-pod-Stage-API
/// und vergleicht das Ergebnis mit der **Einzelknoten-Runtime**. Das ist
/// das Akzeptanzkriterium aus COMPUTE_PIPELINE Phase 1: Ein
/// aufgeteiltes Modell muss bitgleich zum ungeteilten rechnen.
pub fn run_shard(
    log: &mut RunLog,
    artifact_dir: &Path,
    prompts: &[String],
    steps: usize,
    num_shards: usize,
) -> bool {
    log_context(log, Some(artifact_dir));
    if !backend_taugt(log) {
        return false;
    }

    if !artifact_dir.exists() {
        log.error(format!(
            "Artefaktverzeichnis {} fehlt. Shard-Lauf nicht möglich",
            artifact_dir.display()
        ));
        return false;
    }
    if num_shards == 0 {
        log.error("num_shards muss > 0 sein");
        return false;
    }
    if prompts.is_empty() {
        log.error("Kein Prompt angegeben");
        return false;
    }
    let model = match log.timed("modell_laden", "", || loader::load_model(artifact_dir)) {
        Ok(m) => Arc::new(m),
        Err(e) => {
            log.error(format!("Modell nicht ladbar: {}", e));
            return false;
        }
    };
    let num_layers = model.num_layers;
    if num_shards > num_layers {
        log.error(format!(
            "{} Shards für {} Layer: jeder Shard braucht mindestens eine Layer",
            num_shards, num_layers
        ));
        return false;
    }

    // Schichtgrenzen gleichmäßig verteilen; Rest auf die vorderen Shards.
    let mut boundaries = vec![0usize; num_shards + 1];
    let base = num_layers / num_shards;
    let rest = num_layers % num_shards;
    for s in 0..num_shards {
        boundaries[s + 1] = boundaries[s] + base + usize::from(s < rest);
    }
    log.event(Event::Artifact {
        key: "shard_boundaries".into(),
        value: format!("{:?}", boundaries),
    });

    for s in 0..num_shards {
        log.event(Event::Step {
            name: format!("shard_{}_bereit", s),
            millis: 0,
            detail: format!(
                "Layer {}..{}{}{}",
                boundaries[s],
                boundaries[s + 1],
                if s == 0 { ", Embedding" } else { "" },
                if s == num_shards - 1 { ", LM-Head" } else { "" }
            ),
        });
    }

    let mut alle_gleich = true;
    let mut einzelwerte: Vec<String> = Vec::with_capacity(prompts.len());

    for (nr, prompt) in prompts.iter().enumerate() {
        let nr = nr + 1;
        let ids = match encode_prompt(artifact_dir, prompt) {
            Ok(v) => v,
            Err(e) => {
                log.error(e);
                return false;
            }
        };
        log.event(Event::PromptAccepted {
            token_count: ids.len(),
            prompt_sha256: sha256_hex(prompt.as_bytes()),
        });
        if ids.is_empty() {
            log.error(format!("Prompt {} ergibt null Token", nr));
            return false;
        }

        // Der Pod wird je Prompt frisch aufgebaut, das Modell aber nicht
        // neu geladen (es steckt hinter einem `Arc`). Ein wiederverwendeter
        // Koordinator trüge Segmente und PoI-Zustand des vorigen Prompts
        // weiter; gemessen werden soll jeder Prompt für sich.
        let mut shards = Vec::with_capacity(num_shards);
        for s in 0..num_shards {
            let sk = match BlsSecretKey::key_gen(&[(s as u8).wrapping_add(1).wrapping_mul(17); 32])
            {
                Ok(k) => k,
                Err(e) => {
                    log.error(format!(
                        "BLS-Schlüssel für Shard {} fehlgeschlagen: {:?}",
                        s, e
                    ));
                    return false;
                }
            };
            shards.push(Arc::new(ShardNode::new(
                s,
                boundaries[s],
                boundaries[s + 1],
                s == 0,
                s == num_shards - 1,
                model.clone(),
                sk,
                DaStore::new(Box::new(ReedSolomonCoder::default())),
                steps as u64,
            )));
        }

        let mut coordinator = Coordinator::new(
            PodId::new([0xAA; 32]),
            EpochId(0),
            shards,
            myl_pod::coordinator::DEFAULT_WINDOW_MS,
        );

        let pod_out = log.timed(
            &format!("prompt_{}_pod_inferenz", nr),
            &format!("{} Shards", num_shards),
            || coordinator.run_prompt(nr as u64, &ids, steps as u64),
        );
        let pod_tokens_digest = digest_tokens(&pod_out);
        log.result(
            &format!("prompt_{}_pod_tokens", nr),
            &pod_tokens_digest,
            format!(
                "{} Token: {:?}",
                pod_out.len(),
                &pod_out[..pod_out.len().min(8)]
            ),
        );

        // Der Digest über die **gerechneten Zahlen**, seit dem Abschluss
        // von Fund 36 auch aus dem Pod. Fehlt er, ist der Lauf kein
        // Nachweis und darf auch nicht so aussehen: Ein stiller Rückfall
        // auf den Token-Vergleich wäre genau der Zustand, aus dem Fund 36
        // kam.
        let pod_logits_digest = match coordinator.dekodier_digest(nr as u64) {
            Some((d, schritte)) => {
                if schritte != steps {
                    log.error(format!(
                        "Pod hat {} von {} Schritten gesampelt; ein Digest über \
                         verschieden viele Schritte ist schlicht ein anderer Wert \
                         und wäre hier als Determinismusfehler zu lesen",
                        schritte, steps
                    ));
                    return false;
                }
                d
            }
            None => {
                log.error(
                    "Der Pod liefert keinen Dekodier-Digest. Ohne ihn prüft dieser \
                     Lauf nur, ob die Aufteilung dieselbe Entscheidung erzeugt, \
                     nicht dieselben Zahlen (Fund 36)",
                );
                return false;
            }
        };
        log.result(
            &format!("prompt_{}_pod_logits", nr),
            &pod_logits_digest,
            format!("{} Schritte über Logits und Token", steps),
        );

        match coordinator.build_poi_bundle() {
            Ok(b) => log.result(
                &format!("prompt_{}_poi_bundle", nr),
                &sha256_hex(b.segments_root.as_bytes()),
                format!(
                    "vTFE={}, Segmente={}",
                    b.vtfe_claimed,
                    coordinator.completed_segments().len()
                ),
            ),
            Err(e) => log.note(format!("Kein PoI-Bündel für Prompt {}: {}", nr, e)),
        }

        // Gegenprobe: dasselbe Modell ungeteilt.
        let (single_logits_digest, single_desc, single_tokens) = log.timed(
            &format!("prompt_{}_einzelknoten", nr),
            "",
            || greedy_digest(&model, &ids, steps),
        );
        // **Beide Seiten bilden denselben Wert nach demselben Vertrag**
        // (`integer_llm_runtime::generate::DekodierDigest`): je Schritt
        // alle Logits als i32 und danach der gewählte Token. Der Pod
        // bildet ihn im Shard mit dem LM-Head, weil die Logits ihn nie
        // verlassen; auf dem Draht steht nur der Token.
        //
        // Bis zum Abschluss von Fund 36 stand hier Token gegen Token, und
        // der Vergleich prüfte damit, ob die Aufteilung dieselbe
        // **Entscheidung** erzeugt. Der Token-Vergleich bleibt daneben
        // stehen, aber als das schwächere der beiden Urteile.
        let single_tokens_digest = digest_tokens(&single_tokens);
        log.result(
            &format!("prompt_{}_einzelknoten_tokens", nr),
            &single_tokens_digest,
            single_desc,
        );
        log.result(
            &format!("prompt_{}_einzelknoten_logits", nr),
            &single_logits_digest,
            format!("{} Schritte über Logits und Token", steps),
        );

        // Klartext nur auf das Terminal, siehe `run_determinism`.
        if let Some(text) = decode_tokens(artifact_dir, &single_tokens) {
            log.nur_anzeigen("");
            log.nur_anzeigen(format!("  Prompt {}:  {}", nr, prompt));
            log.nur_anzeigen(format!("  Antwort:    {}", text.trim_end()));
            log.nur_anzeigen("");
        }

        if pod_logits_digest == single_logits_digest {
            log.result(
                &format!("shard_vs_einzelknoten_{}", nr),
                &pod_logits_digest,
                "bitgleich über Logits und Token",
            );
        } else {
            alle_gleich = false;
            log.event(Event::Mismatch {
                name: format!("shard_vs_einzelknoten_{}", nr),
                expected: single_logits_digest.clone(),
                actual: pod_logits_digest.clone(),
            });
            // Die Token-Gleichheit ist hier eine **Diagnose**, kein Trost:
            // Gleiche Token bei verschiedenen Zahlen heißt, dass die
            // Abweichung die Rangfolge noch nicht gekippt hat. Das ist
            // genau der Fall, den dieser Vergleich vor Fund 36 nicht sehen
            // konnte.
            if pod_tokens_digest == single_tokens_digest {
                log.note(
                    "Die Token stimmen, die gerechneten Zahlen nicht. Vor dem \
                     Abschluss von Fund 36 hätte dieser Lauf `bitgleich` gemeldet.",
                );
            }
            log.note(
                "Ein Unterschied hier bedeutet, dass die Aufteilung selbst \
                 das Ergebnis verändert, die Shard-Grenzen oder die \
                 Randskalierung (boundary_frac) sind der erste Verdacht.",
            );
        }
        einzelwerte.push(pod_logits_digest);
    }

    let gesamt = digest_ueber(&einzelwerte);
    if alle_gleich {
        log.result(
            "shard_vs_einzelknoten",
            &gesamt,
            format!(
                "{} Prompts bitgleich. Akzeptanzkriterium COMPUTE_PIPELINE Phase 1 erfüllt",
                prompts.len()
            ),
        );
    } else {
        log.result(
            "shard_vs_einzelknoten",
            &gesamt,
            format!("{} Prompts, mindestens einer weicht ab", prompts.len()),
        );
    }
    alle_gleich
}

fn digest_tokens(tokens: &[u32]) -> String {
    let mut bytes = Vec::with_capacity(tokens.len() * 4);
    for t in tokens {
        bytes.extend_from_slice(&t.to_le_bytes());
    }
    sha256_hex(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::RunLog;

    fn tempdir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("myl-testclient-runs-{}", name));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn probe() -> Vec<String> {
        vec!["Test".to_string()]
    }

    #[test]
    fn hardwarelauf_braucht_keine_artefakte() {
        let dir = tempdir("hw");
        let mut log = RunLog::new(&dir, "hardware", false);
        assert!(run_hardware(&mut log));
        let lauf_dir = log.dir().to_path_buf();
        let dateiname = log.dateiname().to_string();
        log.finish(true);

        let jsonl = std::fs::read_to_string(lauf_dir.join(format!("{}.jsonl", dateiname))).unwrap();
        assert!(jsonl.contains("\"kind\":\"hardware\""));
        assert!(jsonl.contains("hardware_fingerprint"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ohne Artefakte darf der Client nicht abstürzen, sondern muss den
    /// Grund protokollieren: auf einer frisch aufgesetzten Testmaschine
    /// ist das der Normalfall.
    #[test]
    fn fehlende_artefakte_werden_sauber_gemeldet() {
        let dir = tempdir("keine-artefakte");
        let mut log = RunLog::new(&dir, "determinismus", false);
        let ok = run_determinism(&mut log, Path::new("/nicht/vorhanden"), &probe(), 4, 2);
        assert!(!ok);
        assert!(log.problems() > 0);
        let lauf_dir = log.dir().to_path_buf();
        let dateiname = log.dateiname().to_string();
        log.finish(false);

        let jsonl = std::fs::read_to_string(lauf_dir.join(format!("{}.jsonl", dateiname))).unwrap();
        assert!(jsonl.contains("\"kind\":\"error\""));
        assert!(jsonl.contains("Artefaktverzeichnis"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ein einzelner Lauf vergleicht nichts. Die Ablehnung steht im
    /// Protokoll, nicht nur im Rückgabewert: Wer `--repeat 1` gesetzt hat,
    /// soll den Grund in der Datei finden und nicht raten müssen, warum
    /// der Lauf nichts geliefert hat.
    #[test]
    fn eine_einzige_wiederholung_wird_abgelehnt() {
        // In einem Bau ohne gültigen Rechenpfad greift die Backend-Sperre
        // davor, und ihre Begründung ist dann die richtige. Der Fall
        // wurde beim Lauf der Testreihe für x86_64 mit `--features
        // cpu-simd` sichtbar, also genau dort, wo Fund 34 sitzt.
        if crate::hardware::rechenpfad_pruefen().is_err() {
            return;
        }
        let dir = tempdir("repeat-eins");
        let mut log = RunLog::new(&dir, "determinismus", false);
        let ok = run_determinism(&mut log, Path::new("/nicht/vorhanden"), &probe(), 4, 1);
        let dateiname = log.dateiname().to_string();
        let lauf_dir = log.dir().to_path_buf();
        log.finish(ok);

        assert!(!ok, "ein einzelner Lauf darf nicht als Nachweis gelten");
        let text = std::fs::read_to_string(lauf_dir.join(format!("{dateiname}.log")))
            .expect("Protokoll lesbar");
        assert!(
            text.contains(">= 2"),
            "Grund fehlt im Protokoll:\n{text}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// θ_v-Version und Ankerdigest müssen im Protokoll stehen (Punkt 3.1).
    /// Ohne sie ist ein abweichender Digest nicht einzuordnen: Er könnte
    /// ein Hardware-Befund sein oder schlicht ein anderer Modellstand.
    ///
    /// Die Ankerdateien werden hier gestellt statt gemessen, der Test
    /// prüft die Protokollierung, nicht die Artefakte, und läuft deshalb
    /// auch in der CI, wo keine liegen.
    #[test]
    fn modellstand_steht_im_protokoll() {
        let dir = tempdir("modellstand");
        let artefakte = dir.join("qwen2.5-0.5b");
        std::fs::create_dir_all(&artefakte).unwrap();
        std::fs::write(
            artefakte.join("theta_v.json"),
            r#"{"version":"0.17.0","weights_hash":"aa","scales_hash":"bb","luts_hash":"cc"}"#,
        )
        .unwrap();
        std::fs::write(artefakte.join("model_config.json"), "{}").unwrap();
        std::fs::write(artefakte.join("tokenizer.json"), "{}").unwrap();

        let mut log = RunLog::new(&dir, "probe", false);
        log_context(&mut log, Some(&artefakte));
        let lauf_dir = log.dir().to_path_buf();
        let dateiname = log.dateiname().to_string();
        log.finish(true);

        let jsonl = std::fs::read_to_string(lauf_dir.join(format!("{}.jsonl", dateiname))).unwrap();
        assert!(jsonl.contains(r#""key":"theta_v","value":"0.17.0""#), "{jsonl}");
        assert!(jsonl.contains(r#""key":"weights_hash","value":"aa""#));
        assert!(jsonl.contains(r#""key":"modell","value":"qwen2.5-0.5b""#));
        assert!(jsonl.contains(r#""key":"artefakt_digest""#));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shardlauf_lehnt_null_shards_ab() {
        let dir = tempdir("null-shards");
        let mut log = RunLog::new(&dir, "shard", false);
        assert!(!run_shard(&mut log, Path::new("/nicht/vorhanden"), &probe(), 4, 0));
        log.finish(false);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn token_digest_ist_stabil_und_reihenfolgeabhaengig() {
        assert_eq!(digest_tokens(&[1, 2, 3]), digest_tokens(&[1, 2, 3]));
        assert_ne!(digest_tokens(&[1, 2, 3]), digest_tokens(&[3, 2, 1]));
        assert_ne!(digest_tokens(&[1, 2]), digest_tokens(&[1, 2, 0]));
    }

    /// Der Digest muss alle Token abdecken: ein Unterschied in der
    /// Mitte darf nicht durch ein gleiches Endergebnis verdeckt werden.
    #[test]
    fn digest_erfasst_auch_mittlere_token() {
        assert_ne!(digest_tokens(&[5, 1, 9]), digest_tokens(&[5, 2, 9]));
    }
}
