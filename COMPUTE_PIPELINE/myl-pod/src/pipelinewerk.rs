//! Das Rechenwerk, das wirklich rechnet (GATEWAY Stufe 4).
//!
//! # ⚑ Was hier zusammenkommt
//!
//! `entsiegelung` prüft Form, Siegel und Bindung und reicht dann einen
//! Klartext weiter. **Bis heute ging der an ein Merkmal, das in Tests
//! zählte statt zu rechnen.** Hier steht die Umsetzung, die die
//! Shard-Pipeline benutzt: Wortschatz, vier Shards, Koordinator.
//!
//! # ⚑ Der Pipeline-Stand wird übergeben und nicht ausgerechnet
//!
//! Welche Artefakte gelten, steht in `scale_packs/REGISTER.json`, und
//! dort ist es **gemessen** (`artefakt_digest_sha256`). Ihn hier ein
//! zweites Mal aus den Dateien zu bilden hiesse, eine zweite Wahrheit
//! über die Modellfassung zu führen; **wer sie beide hat, hat sie
//! irgendwann verschieden.** Der Betreiber gibt ihn mit.
//!
//! # Eine Anfrage nach der anderen
//!
//! Der Koordinator hält den KV-Cache und die Segmentspur; zwei Aufträge
//! gleichzeitig stritten um beide. Die lokale Tür bedient ohnehin
//! sequentiell, also ist die Sperre hier keine Einschränkung, sondern
//! die Aussage darüber, was ein Shard ist: **ein Rechenwerk und keine
//! Warteschlange.**

use std::path::Path;
use std::sync::{Arc, Mutex};

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_types::bls::BlsSecretKey;
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, PodId, SegmentId};
use myl_types::inferenzauftrag::{Inferenzantwort, Inferenzauftrag};

use crate::coordinator::Coordinator;
use crate::entsiegelung::Klartextwerk;
use crate::shard::ShardNode;

/// Wie viele Shards die Probepipeline hat.
///
/// ⚑ **Fest, und das ist die Entscheidung vom 2026-09-03:** erst der Weg
/// mit festem `k`, dann die variable Knotenzahl. Ein Weg, der läuft,
/// lässt sich variabel machen; ein variabler Weg, den es nicht gibt,
/// lässt sich nicht prüfen.
pub const SHARDS: usize = 4;

/// ⚑ **Die Zahl muss mit der Gewichtsableitung übereinstimmen**, und
/// seit dem 2026-09-03 hält das der Übersetzer und nicht ein Kommentar.
///
/// `myl_tokenomics::vtfe::arbeitsverteilung_probe` rechnet genau so
/// viele Gewichte, wie hier Shards laufen. **Liefen sie auseinander,
/// wäre die Folge kein Fehler, sondern Stille:**
/// `zuschreiben_aus_abrechnung` wiese jede Position ausserhalb der
/// Gewichtsliste mit `PositionUnbekannt` ab, und der Pod bekäme für
/// seine Arbeit nichts. Genau dieser Ausgang war Fund 161.
const _: () = assert!(
    SHARDS as u64 == myl_tokenomics::vtfe::PROBE_SHARDS,
    "Shardzahl der Pipeline und der Gewichtsableitung sind auseinandergelaufen"
);

/// Das Rechenwerk über einer geladenen Shard-Pipeline.
pub struct Pipelinewerk {
    koordinator: Mutex<Coordinator>,
    wortschatz: Tokenizer,
    pipeline: Hash,
    /// Wie viele Token höchstens, unabhängig vom Auftrag.
    ///
    /// ⚑ **Der Betreiber deckelt, nicht nur das Protokoll.** Wer eine
    /// schwache Maschine fährt, will nicht, dass ein Auftrag über
    /// `MAX_NEUE_TOKEN` sie minutenlang belegt.
    eigener_deckel: u32,
}

impl Pipelinewerk {
    /// Lädt Modell und Wortschatz und baut die Pipeline auf.
    ///
    /// `pipeline` ist der gemessene Artefakt-Digest aus dem Register.
    pub fn laden(
        artefakte: &Path,
        pod: PodId,
        epoche: EpochId,
        pipeline: Hash,
        eigener_deckel: u32,
    ) -> Result<Self, String> {
        let modell = load_model(artefakte).map_err(|e| format!("Modell laden: {e}"))?;
        let wortschatz = Tokenizer::from_file(
            artefakte
                .join("tokenizer.json")
                .to_str()
                .ok_or_else(|| "Artefaktpfad ist kein gueltiges UTF-8".to_string())?,
        )
        .map_err(|e| format!("Wortschatz laden: {e}"))?;

        let layer = modell.num_layers;
        let modell = Arc::new(modell);
        // Dieselbe Aufteilung wie `myl-pod-node`: gleichmässig, Rest
        // nach hinten.
        let mut grenzen = Vec::with_capacity(SHARDS + 1);
        for s in 0..=SHARDS {
            grenzen.push(layer * s / SHARDS);
        }
        let mut shards = Vec::with_capacity(SHARDS);
        for s in 0..SHARDS {
            // ⚑ **Ein Schlüssel je Shard, abgeleitet und nicht zufällig.**
            // Ein Probelauf soll bei gleichem Aufbau dieselben
            // Unterschriften liefern, sonst ist „bitgleich" nicht
            // prüfbar. Für einen echten Pod kommt der Schlüssel aus der
            // Identität des Miners und nicht von hier.
            let ikm = [(s as u8 + 1).wrapping_mul(17); 32];
            let sk = BlsSecretKey::key_gen(&ikm).map_err(|e| format!("BLS: {e:?}"))?;
            shards.push(Arc::new(ShardNode::new(
                s,
                grenzen[s],
                grenzen[s + 1],
                s == 0,
                s == SHARDS - 1,
                Arc::clone(&modell),
                sk,
                u64::from(eigener_deckel),
            )));
        }
        Ok(Self {
            koordinator: Mutex::new(Coordinator::new(
                pod,
                epoche,
                shards,
                crate::coordinator::DEFAULT_WINDOW_MS,
            )),
            wortschatz,
            pipeline,
            eigener_deckel,
        })
    }

    /// Wie viele Shards die Pipeline hat.
    pub fn shardzahl(&self) -> u32 {
        SHARDS as u32
    }
}

impl Klartextwerk for Pipelinewerk {
    fn rechne(&self, auftrag: &Inferenzauftrag, prompt: &[u8]) -> Inferenzantwort {
        let sitzung = auftrag.sitzung;
        let Ok(text) = core::str::from_utf8(prompt) else {
            // ⚑ **Ein Prompt ist Text.** Bytes, die keiner sind, kommen
            // nicht aus einem Wortschatz zurück; sie hier zu erraten
            // hiesse, etwas anderes zu rechnen als das Gefragte.
            return Inferenzantwort::Abgelehnt { sitzung };
        };
        let eingabe: Vec<u32> = self
            .wortschatz
            .encode(text)
            .iter()
            .map(|t| *t as u32)
            .collect();
        if eingabe.is_empty() {
            return Inferenzantwort::Abgelehnt { sitzung };
        }
        // ⚑ **Der Deckel des Betreibers schlägt den des Auftrags**, und
        // beide sind Obergrenzen: Wer weniger bekommt, als er verlangt
        // hat, bekommt trotzdem eine Antwort.
        let deckel = u64::from(auftrag.max_token.min(self.eigener_deckel));

        let Ok(mut koordinator) = self.koordinator.lock() else {
            return Inferenzantwort::Abgelehnt { sitzung };
        };
        let token = koordinator.run_prompt(sitzung, &eingabe, deckel);
        // ⚑ **Das Segment ist der Faden zur bezeugten Arbeit.** Es
        // entsteht beim Rechnen; ohne es hätte der Nutzer Token und
        // keine Möglichkeit, sie einer Abrechnung zuzuordnen.
        let segment = koordinator
            .completed_segments()
            .last()
            .map(|s| s.id)
            .unwrap_or(SegmentId::new([0u8; 32]));
        drop(koordinator);

        let ausgabe: Vec<usize> = token.iter().map(|t| *t as usize).collect();
        Inferenzantwort::Ergebnis {
            sitzung,
            token,
            segment,
            prompt_token: eingabe.len() as u64,
            text: self.wortschatz.decode(&ausgabe),
        }
    }

    fn pipeline(&self) -> Hash {
        self.pipeline
    }

    fn shards(&self) -> u32 {
        SHARDS as u32
    }
}
