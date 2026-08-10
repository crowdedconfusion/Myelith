//! Qwen2.5 Tokenizer-Wrapper
//! 
//! Verwendet die Hugging Face `tokenizers` Crate fuer deterministische
//! BPE-Tokenisierung. Der Encoding-Pfad ist float-frei und deterministisch.
//! 
//! Voraussetzung: `tokenizer.json` muss im Artefakt-Verzeichnis liegen
//! (wird aus der HF-Repo waehrend der Kalibrierung kopiert).

use tokenizers::Tokenizer as HFTokenizer;

pub struct Tokenizer {
    inner: HFTokenizer,
}

impl Tokenizer {
    /// Laedt den Tokenizer aus einer `tokenizer.json` Datei.
    pub fn from_file(path: &str) -> Result<Self, String> {
        let inner = HFTokenizer::from_file(path)
            .map_err(|e| format!("Tokenizer-Ladung fehlgeschlagen: {}", e))?;
        Ok(Tokenizer { inner })
    }

    /// Encodiert Text zu Token-IDs (deterministisch, float-frei).
    pub fn encode(&self, text: &str) -> Vec<usize> {
        let encoding = self.inner.encode(text, false).unwrap();
        encoding.get_ids().iter().map(|&id| id as usize).collect()
    }

    /// Decodiert Token-IDs zu Text.
    pub fn decode(&self, tokens: &[usize]) -> String {
        let ids: Vec<u32> = tokens.iter().map(|&t| t as u32).collect();
        self.inner.decode(&ids, false).unwrap()
    }
}
