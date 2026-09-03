//! `myl-pod-node` — der Shard-Prozess (Phase 1, in-Prozess-Pipeline).
//!
//! # ⚑ Bis zum 2026-09-03 war das hier kein Dienst (Fund 169)
//!
//! Es lud das Modell, rechnete **einen** Prompt, druckte die Token,
//! baute ein Bündel, das niemand einreichte, und endete.
//! `PodId::new([0xAA; 32])` und `EpochId(0)` standen als Literale darin.
//!
//! **Damit erklärte sich Fund 165 rückwärts:** Die Frage war, warum
//! `myl-node` nie einen `Ortsweg` baut, und die Antwort war, dass es
//! nichts gab, wo er sich anschliessen konnte. `Ortsdienst::oeffnen`
//! hatte null Produktionsaufrufer; alle fünf Fundstellen lagen in
//! Tests.
//!
//! # Zwei Betriebsarten
//!
//! **Dienst** (ohne `--prompt`): öffnet die lokale Tür, legt einen
//! frischen Ausweis ab und wartet auf einen Knoten.
//!
//! ```text
//! myl-pod-node --artefakte <dir> --ausweis <verz> \
//!              --pod <64 hex> --knoten <64 hex> --pipeline <64 hex> \
//!              [--ortsleitung 127.0.0.1:4170] [--epoche 0] [--deckel 16]
//! ```
//!
//! **Vorführung** (mit `--prompt`): rechnet einen Prompt und endet, wie
//! bisher. Bleibt erhalten, weil sie das kürzeste Mittel ist, um zu
//! sehen, ob Artefakte und Wortschatz zusammenpassen.
//!
//! # ⚑ Warum `--knoten` Pflicht ist
//!
//! Der Ausweis der Leitung sagt „du darfst hereinreden", nicht „du bist
//! der Knoten". Ein Shard ohne erwarteten Endpunkt prüfte an einer
//! Ankündigung nur, ob deren Unterschrift zu ihr selbst passt, und
//! jeder, der den Ausweis lesen kann, wäre die Gegenstelle. Deshalb ist
//! sein Fehlen ein **Startfehler** und keine Vorgabe.

use std::sync::Arc;

use myl_pod::coordinator::Coordinator;
use myl_pod::shard::ShardNode;
use myl_pod::wire::pack_tokens;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};

/// Liest 32 Bytes aus einer Hexzeichenkette.
fn hex32(text: &str, was: &str) -> [u8; 32] {
    if text.len() != 64 {
        eprintln!("[myl-pod] {was} erwartet 64 Hexzeichen, bekam {}", text.len());
        std::process::exit(1);
    }
    let mut aus = [0u8; 32];
    for (i, p) in aus.iter_mut().enumerate() {
        match u8::from_str_radix(&text[i * 2..i * 2 + 2], 16) {
            Ok(b) => *p = b,
            Err(_) => {
                eprintln!("[myl-pod] {was} ist kein Hex");
                std::process::exit(1);
            }
        }
    }
    aus
}

/// 32 Bytes als Hex. **Kein Krate dafuer**: zwei Druckzeilen sind keine
/// Abhaengigkeit wert.
fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn fehlt(was: &str) -> ! {
    eprintln!("[myl-pod] {was} fehlt. Ohne sie startet der Dienst nicht.");
    std::process::exit(1)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut artifacts = None;
    let mut prompt = None;
    let mut max_tokens: u64 = 6;
    let mut ortsleitung = String::from("127.0.0.1:4170");
    let mut ausweis = None;
    let mut pod_hex = None;
    let mut knoten_hex = None;
    let mut pipeline_hex = None;
    let mut epoche: u64 = 0;
    let mut deckel: u32 = 16;
    let mut i = 1;
    while i < args.len() {
        // Ein Wert, der fehlt, ist ein Fehler und kein Absturz: Das
        // alte `args[i + 1]` panickte am Ende der Zeile.
        let wert = |i: usize| -> String {
            args.get(i + 1).cloned().unwrap_or_else(|| {
                eprintln!("[myl-pod] {} erwartet einen Wert", args[i]);
                std::process::exit(1)
            })
        };
        match args[i].as_str() {
            // `--artifacts` bleibt gültig: Es steht in Skripten.
            "--artefakte" | "--artifacts" => { artifacts = Some(wert(i)); i += 2; }
            "--prompt" => { prompt = Some(wert(i)); i += 2; }
            "--max-tokens" | "--deckel" => {
                let t = wert(i);
                max_tokens = t.parse().unwrap_or(6);
                deckel = t.parse().unwrap_or(16);
                i += 2;
            }
            "--ortsleitung" => { ortsleitung = wert(i); i += 2; }
            "--ausweis" => { ausweis = Some(wert(i)); i += 2; }
            "--pod" => { pod_hex = Some(wert(i)); i += 2; }
            "--knoten" => { knoten_hex = Some(wert(i)); i += 2; }
            "--pipeline" => { pipeline_hex = Some(wert(i)); i += 2; }
            "--epoche" => { epoche = wert(i).parse().unwrap_or(0); i += 2; }
            _ => { i += 1; }
        }
    }
    let artifacts = artifacts.unwrap_or_else(|| fehlt("--artefakte"));

    // ⚑ **Ohne `--prompt` ist dies ein Dienst.** Das ist die Umkehrung
    // von Fund 169: Die Vorführung war die einzige Betriebsart, jetzt
    // ist sie die Ausnahme.
    let Some(prompt) = prompt else {
        dienst(
            &artifacts,
            &ortsleitung,
            &ausweis.unwrap_or_else(|| fehlt("--ausweis")),
            hex32(&pod_hex.unwrap_or_else(|| fehlt("--pod")), "--pod"),
            hex32(&knoten_hex.unwrap_or_else(|| fehlt("--knoten")), "--knoten"),
            hex32(&pipeline_hex.unwrap_or_else(|| fehlt("--pipeline")), "--pipeline"),
            EpochId(epoche),
            deckel,
        );
        return;
    };

    println!("[myl-pod] Lade Modell aus {} ...", artifacts);
    let model = load_model(std::path::Path::new(&artifacts))
        .expect("Modell-Ladung fehlgeschlagen");
    let tokenizer = Tokenizer::from_file(
        std::path::Path::new(&artifacts).join("tokenizer.json").to_str().unwrap(),
    )
    .expect("Tokenizer-Ladung fehlgeschlagen");

    let num_layers = model.num_layers;
    let num_kv_heads = model.num_kv_heads;
    println!("[myl-pod] Modell geladen: {} Layer, {} KV-Heads", num_layers, num_kv_heads);
    let model = Arc::new(model);

    // 4 Shards: 0..6, 6..12, 12..18, 18..24 (analog zur
    // INTEGER_LLM-4-Node-Pipeline).
    let boundaries = [0usize, 6, 12, 18, num_layers];
    let mut shards = Vec::new();
    for s in 0..4 {
        let layer_start = boundaries[s];
        let layer_end = boundaries[s + 1];
        let has_embedding = s == 0;
        let has_lm_head = s == 3;
        let ikm = [(s as u8 + 1) * 17; 32];
        let sk = BlsSecretKey::key_gen(&ikm).expect("BLS KeyGen");
        let shard = ShardNode::new(
            s,
            layer_start,
            layer_end,
            has_embedding,
            has_lm_head,
            model.clone(),
            sk,
            max_tokens,
        );
        println!(
            "[myl-pod] Shard {}: Layer {}-{} (Embedding: {}, LM-Head: {})",
            s, layer_start, layer_end, has_embedding, has_lm_head
        );
        shards.push(Arc::new(shard));
    }

    let pod_id = PodId::new([0xAA; 32]);
    let epoch = EpochId(0);
    let mut coordinator = Coordinator::new(pod_id, epoch, shards, myl_pod::coordinator::DEFAULT_WINDOW_MS);

    // Prompt kodieren.
    let prompt_ids = tokenizer.encode(&prompt);
    let prompt_tokens: Vec<u32> = prompt_ids.iter().map(|t| *t as u32).collect();
    println!("[myl-pod] Prompt: {:?} ({} Tokens)", prompt, prompt_tokens.len());

    let session_id = 1u64;
    let generated = coordinator.run_prompt(session_id, &prompt_tokens, max_tokens);
    println!("[myl-pod] Generierte Token: {:?}", generated);

    // PoI-Bündel bauen.
    match coordinator.build_poi_bundle() {
        Ok(bundle) => {
            println!("[myl-pod] PoI-Bündel: vTFE={}, Segmente={}",
                bundle.vtfe_claimed, coordinator.completed_segments().len());
        }
        Err(e) => eprintln!("[myl-pod] PoI-Bündel-Fehler: {}", e),
    }

    // Hinweis: pack_tokens wird hier genutzt, um die Token-Darstellung zu zeigen.
    let _ = pack_tokens(&generated);
}

/// Der Shard-Prozess als Dienst (Fund 169).
///
/// ⚑ **Alles, was hier zusammenkommt, gab es schon.** Es fehlte allein
/// das Programm, das es betreibt: `Pipelinewerk` rechnet, `Entsiegelndes`
/// entsiegelt und prüft die Bindung, `Betreibergegenstelle` sagt, wer
/// reden darf, `Ortsdienst` hält die Tür. Vier fertige, geprüfte Teile
/// ohne einen Aufrufer.
#[allow(clippy::too_many_arguments)]
fn dienst(
    artefakte: &str,
    ortsleitung: &str,
    ausweis: &str,
    pod: [u8; 32],
    knoten: [u8; 32],
    pipeline: [u8; 32],
    epoche: EpochId,
    deckel: u32,
) {
    use myl_pod::entsiegelung::Entsiegelndes;
    use myl_pod::gegenstelle::Betreibergegenstelle;
    use myl_pod::ortsdienst::Ortsdienst;
    use myl_pod::pipelinewerk::Pipelinewerk;
    use myl_siegel::{Endpunkt, Epochenschluessel, Sitzungen};

    let adresse: std::net::SocketAddr = match ortsleitung.parse() {
        Ok(a) => a,
        Err(_) => {
            eprintln!("[myl-pod] --ortsleitung erwartet adresse:port, bekam {ortsleitung}");
            std::process::exit(1);
        }
    };
    let pod_id = PodId::new(pod);

    // ⚑ **Der eigene Endpunkt wird abgeleitet und nicht gewürfelt.**
    // Für einen echten Pod ist er die `MinerId` aus der Identität des
    // Miners; dieser Prozess ist kein angemeldeter Miner, also folgt er
    // aus der Pod-Kennung. **Der Knoten muss ihn nicht kennen**: Er
    // erfragt ihn mit `Ortsfrage::Gegenstelle`, bevor er versiegelt.
    let ich = Endpunkt::aus_bytes(
        myl_types::hash::Hash::sha256(&[b"MYELITH-SHARD-ENDPUNKT-v1".as_slice(), &pod].concat()).0,
    );

    println!("[myl-pod] Lade Pipeline aus {artefakte} ...");
    let werk = match Pipelinewerk::laden(
        std::path::Path::new(artefakte),
        pod_id,
        epoche,
        myl_types::hash::Hash(pipeline),
        deckel,
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[myl-pod] Pipeline nicht geladen: {e}");
            std::process::exit(1);
        }
    };
    let shards = werk.shardzahl();

    let entsiegelnd = Entsiegelndes::neu(
        pod_id,
        ich,
        Sitzungen::neu(ich, Epochenschluessel::ziehe(epoche)),
        Box::new(Betreibergegenstelle::neu(Endpunkt::aus_bytes(knoten))),
        Box::new(werk),
    );

    let (dienst, befund) = match Ortsdienst::oeffnen(
        adresse,
        std::path::Path::new(ausweis),
        Box::new(entsiegelnd),
    ) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[myl-pod] Tuer nicht geoeffnet ({adresse}): {e}");
            std::process::exit(1);
        }
    };

    println!("[myl-pod] Shard-Dienst auf {}", befund.adresse);
    println!("[myl-pod] {shards} Shards, Epoche {}, Deckel {deckel}", epoche.0);
    println!("[myl-pod] Ausweis in {ausweis}/ortsschluessel");
    println!("[myl-pod] eigener Endpunkt {}", hex(ich.bytes()));
    println!("[myl-pod] erwarteter Knoten {}", hex(&knoten));
    if !befund.ausweis_geschuetzt {
        // ⚑ **Gesagt und nicht verschwiegen.** Auf einem Dateisystem
        // ohne Unix-Rechte liegt der Ausweis offen, und dann ist er
        // kein Ausweis mehr.
        eprintln!(
            "[myl-pod] WARNUNG: der Ausweis ist vom Dateisystem nicht geschuetzt (kein 0600)."
        );
    }
    if befund.nach_aussen {
        eprintln!(
            "[myl-pod] WARNUNG: die Tuer horcht auf {}, also nicht nur auf der Rueckschleife. \
             Dann steht zwischen einem Fremden und dem Rechenwerk nur noch der Ausweis.",
            befund.adresse
        );
    }
    dienst.laufen();
}
