//! Stage- und Pipeline-Manifeste
//! 
//! Jeder Shard hat ein eigenes Manifest. Das globale Pipeline-Manifest
//! definiert die Topologie und wird von allen Knoten validiert.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Ein einzelner Pipeline-Stage (Shard).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StageManifest {
    pub stage_id: usize,
    pub layer_start: usize,
    pub layer_end: usize,        // exklusiv
    pub has_embedding: bool,     // Nur Stage 0
    pub has_lm_head: bool,       // Nur letzte Stage
    pub has_sampling: bool,      // Nur letzte Stage
    pub node_id: String,
    pub node_address: String,    // TCP/IP oder Socket
    pub weights_hash: String,    // SHA-256 der Gewichte dieses Shards
    pub scales_hash: String,     // SHA-256 der Skalen
    pub kernel_contract: String, // z.B. "reference-v0.4.0"
    pub boundary_contract: String, // z.B. "int16-little-endian-frac8"
    pub max_batch_size: usize,
    pub max_context_per_request: usize,
}

/// Globales Pipeline-Manifest.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PipelineManifest {
    pub pipeline_hash: String,   // SHA-256 ueber kanonisches JSON
    pub theta_v_hash: String,    // Muss mit theta_v/spec.json uebereinstimmen
    pub stages: Vec<StageManifest>,
    pub boundary_dtype: String,  // "int16"
    pub boundary_frac_bits: u8,
    pub boundary_endianness: String, // "little"
    pub communication_protocol: String, // "tcp-binary-custom"
    pub checksum_algorithm: String, // "crc32"
}

impl PipelineManifest {
    /// Laedt und validiert ein Pipeline-Manifest.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Fehler beim Lesen: {}", e))?;
        let manifest: PipelineManifest = serde_json::from_str(&content)
            .map_err(|e| format!("Invalid JSON: {}", e))?;
        
        // Validierung: Stage-Grenzen muessenlueckenlos sein
        let mut expected_start = 0;
        for stage in &manifest.stages {
            if stage.layer_start != expected_start {
                return Err(format!(
                    "Stage {} beginnt bei {}, erwartet {}",
                    stage.stage_id, stage.layer_start, expected_start
                ));
            }
            expected_start = stage.layer_end;
        }
        
        // Validierung: Genau eine Stage mit Embedding und LM-Head
        let embed_count = manifest.stages.iter().filter(|s| s.has_embedding).count();
        let head_count = manifest.stages.iter().filter(|s| s.has_lm_head).count();
        if embed_count != 1 {
            return Err(format!("Erwarte genau 1 Embedding-Stage, habe {}", embed_count));
        }
        if head_count != 1 {
            return Err(format!("Erwarte genau 1 LM-Head-Stage, habe {}", head_count));
        }

        // Shard-Layout gegen den deklarierten pipeline_hash pruefen
        // (Fund 25): Die Boundary-Reskalierung ist verlustbehaftet, das
        // Ergebnis haengt an der Lage der Stage-Grenzen. Ein Pod mit
        // abweichendem Layout darf gar nicht erst starten.
        manifest.verify_layout()?;

        Ok(manifest)
    }
    
    /// Berechnet den Hash fuer ein gegebenes theta_v.
    pub fn verify_theta_v(&self, theta_v_hash: &str) -> Result<(), String> {
        if self.theta_v_hash != theta_v_hash {
            return Err("theta_v hash mismatch".to_string());
        }
        Ok(())
    }

    /// Kanonischer Bezeichner des SHARD-LAYOUTS.
    ///
    /// **Ursprung (Fund 25, 2026-08-19).** `pipeline_hash` war ein Feld,
    /// das nie berechnet und nie geprueft wurde — der Wert stand auf
    /// `sha256:0000`. Diese Funktion schliesst die Luecke.
    ///
    /// **Die urspruengliche Begruendung gilt seit Fund 26 nicht mehr —
    /// bitte nicht ungeprueft weitertragen.** Sie lautete: die
    /// Boundary-Reskalierung sei verlustbehaftet, deshalb haenge das
    /// Ergebnis davon ab, WO die Stage-Grenzen liegen, und zwei Pods mit
    /// unterschiedlichem Layout kaemen zu verschiedenen Token. Der
    /// Boundary-Schritt ist am 2026-08-19 ersatzlos entfallen
    /// (`stage.rs`, `myl-pod/src/shard.rs`); Aktivierungen wandern in
    /// ihrer natuerlichen Per-Kanal-Skala ueber die Grenze. Damit ist
    /// eine Stage-Grenze rechnerisch ein No-Op, und verschiedene Layouts
    /// **sollten** dieselben Token liefern.
    ///
    /// **Gemessen am 2026-08-19:** Drei Layouts ueber dasselbe Artefakt —
    /// 4 Shards (Grenzen 6/12/18), 8 Shards (3/6/9/12/15/18/21) und
    /// ungleichmaessig 4 Shards (1/7/23) — liefern dieselben Token und
    /// sind bitgleich mit dem Einzelknoten
    /// (`tests/integration/test_pipeline_layouts.py`). Das Layout ist
    /// also tatsaechlich gleichgueltig.
    ///
    /// **Was diese Funktion leistet — und was nicht.** Sie prueft das
    /// Manifest **gegen sich selbst**: ob der deklarierte `pipeline_hash`
    /// zum tatsaechlichen Layout passt. Sie erzwingt **keine** Gleichheit
    /// zwischen zwei Pods; das war die Motivation hinter Fund 25, aber
    /// nie seine Wirkung, und nach der Messung oben braucht es sie auch
    /// nicht. Der Nutzen ist Fehlkonfiguration: Genau diese Pruefung hat
    /// den `sha256:0000`-Platzhalter in `configs/pipeline_8node.json`
    /// gefangen, der dort unbemerkt stand.
    ///
    /// # Entschieden am 2026-08-24 (Fund 38)
    ///
    /// Der `pipeline_hash` ist ein **Abrechnungs- und Nachweismerkmal**,
    /// keine Gueltigkeitsbedingung. Er sagt, wie ein Pod zugeschnitten
    /// war, damit sich im Streitfall nachvollziehen laesst, welcher Shard
    /// welche Layer gerechnet hat. Er sagt **nicht**, dass zwei Pods
    /// denselben Zuschnitt haben muessen; nach den Messungen vom
    /// 2026-08-19 und 2026-08-23 (1 bis 24 Shards, gleicher Digest) waere
    /// das eine Sicherung ohne Gegenstand.
    ///
    /// Die Fehlermeldungen trugen die alte Begruendung noch bis zum
    /// 2026-08-24 im Wortlaut („die Boundary-Reskalierung ist
    /// verlustbehaftet", „zwei Pods mit unterschiedlichem Layout liefern
    /// verschiedene Token"). Ein Text, der eine widerlegte Aussage
    /// wiederholt, traegt sie weiter, auch wenn zwanzig Zeilen darueber
    /// steht, dass sie nicht mehr gilt: Gelesen wird im Fehlerfall die
    /// Meldung, nicht die Moduldoku.
    ///
    /// **Damit ist die variable Knotenzahl je Pipeline entblockt**, siehe
    /// COMPUTE_PIPELINE.
    ///
    pub fn canonical_layout_id(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut canon = String::new();
        // Stages nach stage_id sortiert, damit die Reihenfolge in der
        // Datei das Ergebnis nicht beeinflusst.
        let mut stages: Vec<&StageManifest> = self.stages.iter().collect();
        stages.sort_by_key(|s| s.stage_id);
        for s in stages {
            canon.push_str(&format!(
                "{}:{}:{}:{}:{}:{}|",
                s.stage_id, s.layer_start, s.layer_end,
                s.has_embedding as u8, s.has_lm_head as u8, s.has_sampling as u8
            ));
        }
        canon.push_str(&format!(
            "{}:{}:{}",
            self.boundary_dtype, self.boundary_frac_bits, self.boundary_endianness
        ));
        let digest = Sha256::digest(canon.as_bytes());
        let mut hex = String::with_capacity(64);
        for b in digest {
            hex.push_str(&format!("{:02x}", b));
        }
        format!("sha256:{}", hex)
    }

    /// Prueft das deklarierte Shard-Layout gegen das tatsaechliche.
    ///
    /// Der Sentinel `sha256:0000` wird ausdruecklich abgelehnt: Er stand
    /// bis 2026-08-19 in den ausgelieferten Konfigurationen und haette
    /// sonst stillschweigend weiter durchgereicht werden koennen.
    pub fn verify_layout(&self) -> Result<(), String> {
        if self.pipeline_hash.trim_end_matches('0').ends_with("sha256:")
            || self.pipeline_hash == "sha256:0000"
        {
            return Err(format!(
                "pipeline_hash ist ein Platzhalter ({}). Das Feld beschreibt das \
                 tatsaechliche Shard-Layout und wird im Streitfall gebraucht, um \
                 nachzuvollziehen, wie ein Pod zugeschnitten war. Ein Platzhalter \
                 beschreibt nichts. Erwarteter Wert: {}",
                self.pipeline_hash, self.canonical_layout_id()
            ));
        }
        let tatsaechlich = self.canonical_layout_id();
        if self.pipeline_hash != tatsaechlich {
            return Err(format!(
                "Shard-Layout weicht vom deklarierten pipeline_hash ab \
                 (Manifest {}, tatsaechlich {}). Das Manifest beschreibt damit \
                 einen anderen Zuschnitt als den gefahrenen; im Streitfall waere \
                 nicht mehr feststellbar, welcher Shard welche Layer gerechnet hat.",
                self.pipeline_hash, tatsaechlich
            ));
        }
        Ok(())
    }
    
    /// Ermittelt die naechste Stage in der Pipeline.
    pub fn next_stage(&self, stage_id: usize) -> Option<&StageManifest> {
        self.stages.iter().find(|s| s.stage_id == stage_id + 1)
    }
    
    /// Ermittelt die vorherige Stage.
    pub fn prev_stage(&self, stage_id: usize) -> Option<&StageManifest> {
        if stage_id == 0 {
            return None;
        }
        self.stages.iter().find(|s| s.stage_id == stage_id - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Strukturgleich zum echten `StageManifest` — alle Felder, wie sie
    /// ein erzeugtes Manifest tatsaechlich traegt (Projektkonvention:
    /// Test-Fixtures spiegeln reale Formate, nicht Bequemlichkeit).
    fn stage(id: usize, start: usize, end: usize, embed: bool, head: bool) -> serde_json::Value {
        serde_json::json!({
            "stage_id": id,
            "layer_start": start,
            "layer_end": end,
            "has_embedding": embed,
            "has_lm_head": head,
            "has_sampling": head,
            "node_id": format!("node-{}", id),
            "node_address": format!("127.0.0.1:{}", 9000 + id),
            "weights_hash": format!("{:064x}", id),
            "scales_hash": format!("{:064x}", 1000 + id),
            "kernel_contract": "reference-v0.4.0",
            "boundary_contract": "int16-little-endian-frac8",
            "max_batch_size": 32,
            "max_context_per_request": 2048,
        })
    }

    /// Schreibt ein Manifest in eine temporaere Datei und laedt es.
    fn load_json(name: &str, stages: Vec<serde_json::Value>) -> Result<PipelineManifest, String> {
        let dir = std::env::temp_dir().join(format!("myelith-pipeline-tests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("Testverzeichnis");
        let path = dir.join(format!("{}.json", name));
        let mut obj = serde_json::json!({
            "pipeline_hash": "sha256:platzhalter",
            "theta_v_hash": "abc123",
            "stages": stages,
            "boundary_dtype": "int16",
            "boundary_frac_bits": 8,
            "boundary_endianness": "little",
            "communication_protocol": "tcp-binary-custom",
            "checksum_algorithm": "crc32",
        });
        // Fund 25: `load` prueft das Shard-Layout gegen `pipeline_hash`.
        // Das Fixture traegt deshalb den ECHTEN Hash — ein Test, der mit
        // einem Platzhalter arbeitete, wuerde die Pruefung umgehen, die er
        // eigentlich voraussetzt.
        if let Ok(vorlaeufig) = serde_json::from_value::<PipelineManifest>(obj.clone()) {
            obj["pipeline_hash"] = serde_json::Value::String(vorlaeufig.canonical_layout_id());
        }
        std::fs::write(&path, serde_json::to_string(&obj).unwrap()).expect("schreiben");
        let result = PipelineManifest::load(&path);
        let _ = std::fs::remove_file(&path);
        result
    }

    /// Ein gueltiges Manifest als Struct (fuer Tests, die den Ladepfad
    /// nicht brauchen, sondern die Hash-Logik direkt pruefen).
    fn testmanifest() -> PipelineManifest {
        serde_json::from_value(serde_json::json!({
            "pipeline_hash": "sha256:platzhalter",
            "theta_v_hash": "abc123",
            "stages": gueltige_stages(),
            "boundary_dtype": "int16",
            "boundary_frac_bits": 8,
            "boundary_endianness": "little",
            "communication_protocol": "tcp-binary-custom",
            "checksum_algorithm": "crc32",
        })).expect("Testmanifest")
    }

    fn gueltige_stages() -> Vec<serde_json::Value> {
        vec![
            stage(0, 0, 8, true, false),
            stage(1, 8, 16, false, false),
            stage(2, 16, 24, false, true),
        ]
    }

    #[test]
    fn gueltiges_manifest_laedt() {
        let m = load_json("gueltig", gueltige_stages()).expect("laedt");
        assert_eq!(m.stages.len(), 3);
        assert_eq!(m.theta_v_hash, "abc123");
    }

    /// Eine Luecke zwischen den Stages bedeutet, dass Layer von keinem
    /// Node ausgefuehrt wuerden — das Modell waere still unvollstaendig.
    #[test]
    fn luecke_zwischen_stages_wird_abgelehnt() {
        let stages = vec![
            stage(0, 0, 8, true, false),
            stage(1, 9, 16, false, false), // Layer 8 faellt heraus
            stage(2, 16, 24, false, true),
        ];
        let err = load_json("luecke", stages).unwrap_err();
        assert!(err.contains("beginnt bei"), "unerwartete Meldung: {}", err);
    }

    /// Ueberlappung bedeutet doppelt ausgefuehrte Layer.
    #[test]
    fn ueberlappung_wird_abgelehnt() {
        let stages = vec![
            stage(0, 0, 8, true, false),
            stage(1, 7, 16, false, false),
            stage(2, 16, 24, false, true),
        ];
        assert!(load_json("ueberlappung", stages).is_err());
    }

    #[test]
    fn manifest_muss_bei_layer_null_beginnen() {
        let stages = vec![stage(0, 1, 8, true, true)];
        assert!(load_json("offset", stages).is_err());
    }

    #[test]
    fn genau_eine_embedding_stage() {
        let mut stages = gueltige_stages();
        stages[1]["has_embedding"] = serde_json::json!(true);
        let err = load_json("zwei_embed", stages).unwrap_err();
        assert!(err.contains("Embedding"), "unerwartete Meldung: {}", err);

        let mut ohne = gueltige_stages();
        ohne[0]["has_embedding"] = serde_json::json!(false);
        assert!(load_json("kein_embed", ohne).is_err());
    }

    #[test]
    fn genau_eine_lm_head_stage() {
        let mut stages = gueltige_stages();
        stages[0]["has_lm_head"] = serde_json::json!(true);
        let err = load_json("zwei_head", stages).unwrap_err();
        assert!(err.contains("LM-Head"), "unerwartete Meldung: {}", err);
    }

    #[test]
    fn fehlende_datei_meldet_fehler() {
        let err = PipelineManifest::load("/nicht/vorhanden/manifest.json").unwrap_err();
        assert!(err.contains("Fehler beim Lesen"));
    }

    #[test]
    fn kaputtes_json_meldet_fehler() {
        let dir = std::env::temp_dir().join(format!("myelith-pipeline-tests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kaputt.json");
        std::fs::write(&path, "{ kein json").unwrap();
        let err = PipelineManifest::load(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.contains("Invalid JSON"));
    }

    #[test]
    fn theta_v_pruefung() {
        let m = load_json("theta", gueltige_stages()).expect("laedt");
        assert!(m.verify_theta_v("abc123").is_ok());
        assert!(m.verify_theta_v("anders").is_err());
    }

    #[test]
    fn stage_nachbarschaft() {
        let m = load_json("nachbarn", gueltige_stages()).expect("laedt");

        assert_eq!(m.next_stage(0).map(|s| s.stage_id), Some(1));
        assert_eq!(m.next_stage(1).map(|s| s.stage_id), Some(2));
        assert!(m.next_stage(2).is_none(), "letzte Stage hat keinen Nachfolger");

        assert!(m.prev_stage(0).is_none(), "erste Stage hat keinen Vorgaenger");
        assert_eq!(m.prev_stage(1).map(|s| s.stage_id), Some(0));
        assert_eq!(m.prev_stage(2).map(|s| s.stage_id), Some(1));
    }

    #[test]
    fn einzelne_stage_ist_gueltig() {
        let m = load_json("einzeln", vec![stage(0, 0, 24, true, true)]).expect("laedt");
        assert_eq!(m.stages.len(), 1);
        assert!(m.next_stage(0).is_none());
        assert!(m.prev_stage(0).is_none());
    }

    #[test]
    fn test_layout_hash_ignoriert_betriebsangaben() {
        // Fund 25: In den Layout-Hash darf NUR eingehen, was die Zahlen
        // beeinflusst. Zwei Pods auf verschiedenen Maschinen, mit
        // anderen Adressen und anderer Batch-Groesse, muessen als
        // dasselbe Layout gelten — sonst wuerde der Redundanzvergleich
        // legitime Konfigurationsunterschiede als Manipulation werten.
        let mut a = testmanifest();
        let hash_a = a.canonical_layout_id();

        a.stages[0].node_id = "ganz-anderer-knoten".into();
        a.stages[0].node_address = "10.0.0.99:1234".into();
        a.stages[0].max_batch_size = 999;
        a.stages[0].max_context_per_request = 4096;
        assert_eq!(a.canonical_layout_id(), hash_a,
            "Betriebsangaben duerfen den Layout-Hash nicht veraendern");
    }

    #[test]
    fn test_layout_hash_erfasst_jede_numerisch_relevante_aenderung() {
        // Die Gegenprobe: alles, was die Zahlen aendert, MUSS den Hash
        // aendern. Jede dieser Abweichungen wuerde zu anderen Token
        // fuehren; ein Pod damit darf nicht als gleichwertig gelten.
        let basis = testmanifest();
        let hash = basis.canonical_layout_id();

        // andere Shard-Grenze
        let mut v = testmanifest();
        v.stages[0].layer_end = 3;
        v.stages[1].layer_start = 3;
        assert_ne!(v.canonical_layout_id(), hash, "Stage-Grenze");

        // andere Boundary-Skala (der verlustbehaftete Schritt selbst)
        let mut v = testmanifest();
        v.boundary_frac_bits += 1;
        assert_ne!(v.canonical_layout_id(), hash, "boundary_frac_bits");

        // andere Sonderrolle
        let mut v = testmanifest();
        v.stages[1].has_sampling = !v.stages[1].has_sampling;
        assert_ne!(v.canonical_layout_id(), hash, "has_sampling");

        // anderes Wire-Format
        let mut v = testmanifest();
        v.boundary_endianness = "big".into();
        assert_ne!(v.canonical_layout_id(), hash, "endianness");
    }

    #[test]
    fn test_layout_hash_unabhaengig_von_der_reihenfolge_in_der_datei() {
        // Die Stages werden vor dem Hashen nach stage_id sortiert: die
        // Reihenfolge in der JSON-Datei ist eine Formatierungsfrage, keine
        // numerische.
        let a = testmanifest();
        let mut b = testmanifest();
        b.stages.reverse();
        assert_eq!(a.canonical_layout_id(), b.canonical_layout_id());
    }

    #[test]
    fn test_verify_layout_lehnt_platzhalter_ab() {
        // Der Sentinel sha256:0000 stand bis 2026-08-19 in den
        // ausgelieferten Konfigurationen und wurde nie geprueft. Er muss
        // laut scheitern, nicht stillschweigend durchgehen.
        let mut m = testmanifest();
        m.pipeline_hash = "sha256:0000".into();
        let err = m.verify_layout().expect_err("Platzhalter muss scheitern");
        assert!(err.contains("Platzhalter"), "Fehlermeldung: {}", err);
    }

    #[test]
    fn test_verify_layout_akzeptiert_korrekten_hash() {
        let mut m = testmanifest();
        m.pipeline_hash = m.canonical_layout_id();
        assert!(m.verify_layout().is_ok());
    }

    #[test]
    fn test_verify_layout_lehnt_abweichendes_layout_ab() {
        // Der eigentliche Schutzfall: Ein Pod behauptet ein Layout und
        // rechnet mit einem anderen.
        let mut m = testmanifest();
        m.pipeline_hash = m.canonical_layout_id();
        m.stages[0].layer_end = 3;
        m.stages[1].layer_start = 3;
        let err = m.verify_layout().expect_err("abweichendes Layout muss scheitern");
        assert!(err.contains("weicht"), "Fehlermeldung: {}", err);
    }
}
