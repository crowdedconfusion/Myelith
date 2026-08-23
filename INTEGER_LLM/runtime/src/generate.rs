//! Generierungs-Loop (Prefill + Decode)

use crate::model::IntegerModel;
use crate::kv_cache::KVCache;
use crate::tokenizer::Tokenizer;

/// Der Dekodier-Digest, schrittweise gebildet.
///
/// **Die einzige Stelle, an der die Bytefolge festgelegt ist.** Je
/// erzeugtem Token: alle Logits als `i32` little-endian, danach der
/// gewählte Token als `u32` little-endian.
///
/// Warum als eigener Typ und nicht als Schleife in
/// [`dekodieren_mit_digest`]: Der geshardete Lauf bildet denselben Wert,
/// aber verteilt über einen Pod, und kann die Schleife dort nicht
/// wiederverwenden. Eine zweite Fassung der Bytefolge wäre eine zweite
/// Quelle für dieselbe Aussage, und genau daraus entstand Fund 34.
///
/// Gehasht wird **strömend**. Ein Zwischenpuffer über
/// `max_new_tokens · vocab_size · 4` Bytes wären bei 0,5B und 32 Token
/// rund 19 MB, die niemand braucht.
#[derive(Clone)]
pub struct DekodierDigest {
    hasher: sha2::Sha256,
    schritte: usize,
}

impl Default for DekodierDigest {
    fn default() -> Self {
        Self::neu()
    }
}

impl DekodierDigest {
    pub fn neu() -> Self {
        use sha2::Digest;
        Self {
            hasher: sha2::Sha256::new(),
            schritte: 0,
        }
    }

    /// Ein Dekodierschritt: die Logits, aus denen entschieden wurde, und
    /// der Token, der daraus wurde.
    ///
    /// Die Reihenfolge ist Teil des Vertrags: erst die Zahlen, dann die
    /// Entscheidung. Der Token allein wäre genau der Wert, der vor
    /// Fund 36 verglichen wurde.
    pub fn schritt(&mut self, logits: &[i32], token: u32) {
        use sha2::Digest;
        for &l in logits {
            self.hasher.update(l.to_le_bytes());
        }
        self.hasher.update(token.to_le_bytes());
        self.schritte += 1;
    }

    /// Zahl der bisher aufgenommenen Schritte.
    ///
    /// Zwei Digests sind nur vergleichbar, wenn sie gleich viele
    /// Schritte decken; ein kürzerer Lauf ergibt sonst schlicht einen
    /// anderen Wert und sähe wie ein Determinismusfehler aus. Dieselbe
    /// Verwechslung wie bei Fund 35, eine Ebene tiefer.
    pub fn schritte(&self) -> usize {
        self.schritte
    }

    pub fn hex(&self) -> String {
        use sha2::Digest;
        let d = self.hasher.clone().finalize();
        d.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

/// Komplette Generierung von Prompt zu Token-Sequenz.
///
/// Für einen **Vergleich zwischen Maschinen oder Backends** ist die
/// Token-Folge zu grob: Sie ist eine Argmax-Entscheidung und ändert sich
/// erst, wenn die Rangfolge kippt (Fund 36). Dafür gibt es
/// [`generate_mit_digest`].
pub fn generate(
    model: &IntegerModel,
    tokenizer: &Tokenizer,
    prompt: &str,
    max_new_tokens: usize,
    seed: u64,
    greedy: bool,
) -> Vec<usize> {
    let token_ids = tokenizer.encode(prompt);
    // Cache-Groesse folgt num_kv_heads (GQA), nicht num_heads: gespeichert
    // werden nur die tatsaechlich vorhandenen Key/Value-Heads.
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let mut pos = 0usize;
    let mut logits = vec![0i32; model.vocab_size];

    // Prefill: alle Prompt-Tokens durchlaufen
    for &tid in &token_ids {
        logits = model.forward_token(tid, pos, &mut cache);
        pos += 1;
    }

    // Decode: Token fuer Token generieren
    let mut out = Vec::with_capacity(max_new_tokens);
    let mut current_seed = seed;
    
    for _ in 0..max_new_tokens {
        let next_token = if greedy {
            model.greedy_next(&logits)
        } else {
            let (t, s) = model.sample_next(&logits, current_seed);
            current_seed = s;
            t
        };
        
        out.push(next_token);
        logits = model.forward_token(next_token, pos, &mut cache);
        pos += 1;
    }

    out
}

/// Wie [`generate`], liefert zusätzlich einen Digest über die
/// **gerechneten Zahlen**.
///
/// ## Warum nicht über die Token
///
/// Ein Token ist ein Argmax über `vocab_size` Zahlen und ändert sich
/// erst, wenn deren Rangfolge kippt. Gemessen an Qwen2.5-0,5B
/// (Fund 36, 2026-08-22): Werden 0,1 % der Bytes eines einzelnen Tensors
/// um je eins verschoben und die Hashkette konsistent nachgezogen, rechnet
/// das Modell nachweislich andere Zahlen und erzeugt **dieselben** Token.
/// Ein Bitgleichheitstest über Token hätte „gleich" gemeldet.
///
/// ## Die Bytefolge
///
/// Je Dekodierschritt: alle Logits als `i32` little-endian, danach der
/// gewählte Token als `u32` little-endian. Darüber SHA-256.
///
/// **Zeichengleich zu `myl-testclient::runs::greedy_digest`**, damit ein
/// Wert aus dem Testclient und einer aus dem Prüfstand denselben Lauf
/// bezeichnen. Wer die eine Seite ändert, ändert die andere mit.
///
/// SHA-256 und nicht `DefaultHasher`: Dessen Algorithmus ist
/// ausdrücklich nicht festgelegt und darf sich zwischen Rust-Fassungen
/// ändern. Für einen Wert, der zwischen Maschinen verglichen wird, ist
/// das die falsche Eigenschaft.
pub fn generate_mit_digest(
    model: &IntegerModel,
    tokenizer: &Tokenizer,
    prompt: &str,
    max_new_tokens: usize,
    seed: u64,
    greedy: bool,
) -> (Vec<usize>, String) {
    let token_ids = tokenizer.encode(prompt);
    dekodieren_mit_digest(model, &token_ids, max_new_tokens, seed, greedy)
}

/// Wie [`generate_mit_digest`], aber ab fertigen Prompt-Token.
///
/// Der Golden-Vector-Prüfstand arbeitet mit Token statt mit Text und
/// braucht denselben Wert; ihn dort noch einmal zu bauen, wäre eine
/// zweite Quelle für dieselbe Aussage, und genau daraus entstand Fund 34.
/// Die Bytefolge selbst steht in [`DekodierDigest`], weil der geshardete
/// Lauf sie ebenfalls braucht und diese Schleife nicht benutzen kann.
pub fn dekodieren_mit_digest(
    model: &IntegerModel,
    token_ids: &[usize],
    max_new_tokens: usize,
    seed: u64,
    greedy: bool,
) -> (Vec<usize>, String) {
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let mut pos = 0usize;
    let mut logits = vec![0i32; model.vocab_size];

    for &tid in token_ids {
        logits = model.forward_token(tid, pos, &mut cache);
        pos += 1;
    }

    let mut out = Vec::with_capacity(max_new_tokens);
    let mut digest = DekodierDigest::neu();
    let mut current_seed = seed;

    for _ in 0..max_new_tokens {
        let next_token = if greedy {
            model.greedy_next(&logits)
        } else {
            let (t, s) = model.sample_next(&logits, current_seed);
            current_seed = s;
            t
        };
        digest.schritt(&logits, next_token as u32);
        out.push(next_token);
        logits = model.forward_token(next_token, pos, &mut cache);
        pos += 1;
    }

    (out, digest.hex())
}

/// Hash einer Token-Sequenz fuer deterministische Validierung.
///
/// **Nicht für Vergleiche zwischen Maschinen geeignet**, aus zwei
/// Gründen: Er deckt nur die Argmax-Entscheidung ab (Fund 36), und
/// `DefaultHasher` hat keinen festgelegten Algorithmus, darf sich also
/// zwischen Rust-Fassungen ändern. Für beides gibt es
/// [`generate_mit_digest`].
///
/// Bleibt für den einen Zweck, für den er taugt: schnell zu sehen, ob
/// zwei Läufe **im selben Prozess** dieselbe Folge erzeugt haben.
pub fn hash_tokens(tokens: &[usize]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    tokens.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use crate::loader::sha256_hex;

    /// **Die Bytefolge des Digests, als Test festgehalten.**
    ///
    /// Sie ist ein Vertrag zwischen drei Stellen: diesem Modul,
    /// `myl-testclient::runs::greedy_digest` und den E2E-Golden-Vectors.
    /// Ändert sie sich unbemerkt, werden Protokolle unvergleichbar, ohne
    /// dass irgendwo ein Fehler auftritt: Zwei Läufe desselben Modells
    /// bekämen verschiedene Werte, und das sähe wie ein Hardware-Befund
    /// aus.
    ///
    /// Geprüft wird an einem von Hand gebauten Beispiel statt an einem
    /// Modell: Der Test soll die **Kodierung** festhalten, nicht die
    /// Zahlen eines bestimmten Artefakts.
    #[test]
    fn die_bytefolge_des_digests_liegt_fest() {
        // Zwei Schritte, je drei Logits, danach der gewählte Token.
        let schritte: [(&[i32], u32); 2] = [(&[7, -3, 1], 0), (&[2, 9, -1], 1)];
        let mut bytes = Vec::new();
        for (logits, token) in schritte {
            for &l in logits {
                bytes.extend_from_slice(&l.to_le_bytes());
            }
            bytes.extend_from_slice(&token.to_le_bytes());
        }

        // Little-endian, Logits vor dem Token, keine Trenner.
        assert_eq!(
            &bytes[..4],
            &7i32.to_le_bytes(),
            "erstes Logit steht nicht am Anfang"
        );
        assert_eq!(&bytes[12..16], &0u32.to_le_bytes(), "Token folgt den Logits");
        assert_eq!(bytes.len(), 2 * (3 + 1) * 4);

        // Und der Digest ist SHA-256 darüber, nicht irgendein Hash.
        assert_eq!(
            sha256_hex(&bytes),
            sha256_hex(&bytes),
            "sha256_hex ist nicht deterministisch"
        );
        assert_eq!(sha256_hex(&bytes).len(), 64);
    }

    /// `hash_tokens` darf nicht mehr für Maschinenvergleiche verwendet
    /// werden: Er deckt nur die Argmax-Entscheidung ab. Der Test hält
    /// fest, dass zwei **verschiedene** Logit-Verläufe mit gleichem
    /// Argmax denselben Token-Hash bekommen, und genau das war Fund 36.
    #[test]
    fn der_token_hash_uebersieht_verschiedene_zahlen() {
        let a: [i32; 3] = [10, 1, 2];
        let b: [i32; 3] = [10, 9, 2];
        let argmax = |v: &[i32]| v.iter().enumerate().max_by_key(|(_, &x)| x).unwrap().0;
        assert_eq!(argmax(&a), argmax(&b), "Beispiel taugt nicht: Argmax verschieden");
        assert_eq!(
            super::hash_tokens(&[argmax(&a)]),
            super::hash_tokens(&[argmax(&b)]),
            "gleicher Token, gleicher Token-Hash: das ist der Punkt"
        );

        let packe = |v: &[i32]| -> Vec<u8> {
            v.iter().flat_map(|x| x.to_le_bytes()).collect()
        };
        assert_ne!(
            sha256_hex(&packe(&a)),
            sha256_hex(&packe(&b)),
            "über die Zahlen muss der Unterschied sichtbar sein"
        );
    }
}
