//! DA-Archivierung (Data Availability, Anhang A.3 Schritt 6).
//!
//! Aktivierungen werden erasure-codiert für die Streitfrist archiviert.
//! Die Erasure-Coding-Schnittstelle [`ErasureCoder`] trennt die
//! Kodierungs-Strategie von der Archivierung; Phase 1 liefert eine
//! XOR-Paritäts-Kodierung (k Daten-Fragmente + 1 Parität, toleriert den
//! Verlust eines Fragments). Die beschlossene Reed-Solomon-Variante
//! (k=8/m=4, CONSENSUS/TOKENOMICS-Design-Entscheidung, toleriert 4
//! verlorene Fragmente) ist eine Folge-Implementierung hinter derselben
//! Schnittstelle.
// Der Index ist die Fragmentnummer und geht in den Schluessel ein.
#![allow(clippy::needless_range_loop)]

use std::collections::BTreeMap;

/// Erasure-Coding-Schnittstelle.
///
/// **`Send + Sync` ist Teil des Vertrags (Fund A17):** Ein Pod führt
/// seine Shards nebenläufig aus (Micro-Batching und Pipelining,
/// Phase 2.1). Ohne diese Schranken war `Box<dyn ErasureCoder>` weder
/// `Send` noch `Sync`, damit `DaStore` nicht, damit `ShardNode` nicht —
/// und der `Arc<ShardNode>` im Pod-Loop war ein `Arc`, der gar nicht
/// über Threads geteilt werden konnte. Die Nebenläufigkeit des Pods
/// scheiterte an dieser einen fehlenden Schranke, ohne dass es
/// irgendwo aufgefallen wäre (der aktuelle Loop läuft sequenziell).
pub trait ErasureCoder: Send + Sync {
    /// Zerlegt `data` in Fragmente (Daten- + ggf. Paritäts-Fragmente).
    fn encode(&self, data: &[u8]) -> Vec<Vec<u8>>;
    /// Rekonstruiert `data` aus den Fragmenten. Fehlende/leere
    /// Fragmente werden als `None` übergeben; die Rekonstruktion gelingt,
    /// solange genügend Fragmente vorhanden sind.
    fn decode(&self, fragments: &[Option<Vec<u8>>]) -> Result<Vec<u8>, String>;
    /// Anzahl der Daten-Fragmente.
    fn data_fragments(&self) -> usize;
    /// Anzahl der Paritäts-Fragmente.
    fn parity_fragments(&self) -> usize;
}

/// XOR-Paritäts-Kodierung: `k` gleich große Daten-Fragmente plus ein
/// XOR-Paritäts-Fragment. Toleriert den Verlust genau eines Fragments.
pub struct XorParityCoder {
    k: usize,
}

impl XorParityCoder {
    pub fn new(k: usize) -> Self {
        assert!(k >= 1, "mindestens ein Daten-Fragment");
        Self { k }
    }

    /// Zerlegt `data` in `k` Blöcke gleicher Länge (letzter wird mit 0
    /// aufgefüllt). Liefert die Blöcke und die Auffüll-Länge.
    fn split(&self, data: &[u8]) -> (Vec<Vec<u8>>, usize) {
        let block_len = data.len().div_ceil(self.k);
        let block_len = block_len.max(1);
        let mut blocks = Vec::with_capacity(self.k);
        for i in 0..self.k {
            let start = i * block_len;
            let mut block = vec![0u8; block_len];
            let end = (start + block_len).min(data.len());
            if start < data.len() {
                block[..end - start].copy_from_slice(&data[start..end]);
            }
            blocks.push(block);
        }
        (blocks, data.len())
    }
}

impl ErasureCoder for XorParityCoder {
    fn encode(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let (blocks, orig_len) = self.split(data);
        // Parität = XOR aller Blöcke.
        let mut parity = vec![0u8; blocks[0].len()];
        for block in &blocks {
            for (p, b) in parity.iter_mut().zip(block.iter()) {
                *p ^= b;
            }
        }
        // Kopf: ursprüngliche Länge (8 Byte LE), damit beim Dekodieren
        // die Auffüllung entfernt werden kann.
        let mut header = orig_len.to_le_bytes().to_vec();
        let mut out = Vec::with_capacity(self.k + 1);
        // Fragment 0 trägt den Kopf.
        let mut first = blocks[0].clone();
        first.splice(0..0, header.drain(..));
        out.push(first);
        for block in blocks.into_iter().skip(1) {
            out.push(block);
        }
        out.push(parity);
        out
    }

    fn decode(&self, fragments: &[Option<Vec<u8>>]) -> Result<Vec<u8>, String> {
        let total = self.k + 1;
        if fragments.len() != total {
            return Err(format!("erwartet {} Fragmente, erhalten {}", total, fragments.len()));
        }
        // Kopf aus Fragment 0 (falls vorhanden) oder aus der Parität
        // rekonstruieren. Für Phase 1: Fragment 0 muss vorhanden sein für
        // den Kopf; fehlt es, ist die Rekonstruktion nicht möglich
        // (Dokumentation: vollständige Kopf-Rekonstruktion folgt mit RS).
        let first = fragments[0]
            .as_ref()
            .ok_or("Fragment 0 (mit Kopf) fehlt — Phase-1-Einschränkung")?;
        if first.len() < 8 {
            return Err("Fragment 0 zu kurz für den Kopf".to_string());
        }
        let orig_len = u64::from_le_bytes(first[..8].try_into().unwrap()) as usize;
        let block0 = &first[8..];
        let block_len = block0.len();

        // Fehlende Blöcke zählen (ohne Fragment 0 und Parität).
        let missing: Vec<usize> = (0..self.k)
            .filter(|&i| {
                if i == 0 {
                    false
                } else {
                    fragments[i].is_none()
                }
            })
            .collect();
        if missing.len() > 1 {
            return Err(format!(
                "{} Daten-Fragmente fehlen, XOR-Parität toleriert nur 1",
                missing.len()
            ));
        }

        let mut blocks: Vec<Vec<u8>> = Vec::with_capacity(self.k);
        blocks.push(block0.to_vec());
        for i in 1..self.k {
            match &fragments[i] {
                Some(b) => blocks.push(b.clone()),
                None => blocks.push(vec![0u8; block_len]), // Platzhalter, wird ersetzt
            }
        }

        // Falls ein Block fehlt: aus Parität rekonstruieren.
        if let Some(&miss) = missing.first() {
            let parity = fragments[self.k]
                .as_ref()
                .ok_or("fehlender Block und keine Parität verfügbar")?;
            if parity.len() != block_len {
                return Err("Paritäts-Fragment hat falsche Länge".to_string());
            }
            let mut recon = parity.clone();
            for (i, block) in blocks.iter().enumerate() {
                if i == miss {
                    continue;
                }
                for (r, b) in recon.iter_mut().zip(block.iter()) {
                    *r ^= b;
                }
            }
            blocks[miss] = recon;
        }

        // Zusammensetzen und Auffüllung entfernen.
        let mut data = Vec::with_capacity(orig_len);
        for block in &blocks {
            data.extend_from_slice(block);
        }
        data.truncate(orig_len);
        Ok(data)
    }

    fn data_fragments(&self) -> usize {
        self.k
    }

    fn parity_fragments(&self) -> usize {
        1
    }
}

/// DA-Archiv: speichert erasure-codierte Fragmente je Segment und Shard
/// und hält sie für die Streitfrist vor.
pub struct DaStore {
    coder: Box<dyn ErasureCoder>,
    /// (segment_id, shard_index) → Fragmente.
    store: BTreeMap<([u8; 32], u64), Vec<Vec<u8>>>,
}

impl DaStore {
    pub fn new(coder: Box<dyn ErasureCoder>) -> Self {
        Self {
            coder,
            store: BTreeMap::new(),
        }
    }

    /// Archiviert die Aktivierungen eines Übergangs (Anhang A.3 Schritt 6).
    pub fn put(&mut self, segment_id: [u8; 32], shard_index: u64, activations: &[i16]) {
        let mut bytes = Vec::with_capacity(activations.len() * 2);
        for v in activations {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let fragments = self.coder.encode(&bytes);
        self.store.insert((segment_id, shard_index), fragments);
    }

    /// Rekonstruiert die Aktivierungen eines Übergangs.
    pub fn get(&self, segment_id: [u8; 32], shard_index: u64) -> Result<Vec<i16>, String> {
        let fragments = self
            .store
            .get(&(segment_id, shard_index))
            .ok_or("Segment/Shard nicht archiviert")?;
        let present: Vec<Option<Vec<u8>>> = fragments.iter().map(|f| Some(f.clone())).collect();
        let bytes = self.coder.decode(&present)?;
        if bytes.len() % 2 != 0 {
            return Err("rekonstruierte Daten haben ungerade Länge".to_string());
        }
        let mut out = Vec::with_capacity(bytes.len() / 2);
        for chunk in bytes.chunks_exact(2) {
            out.push(i16::from_le_bytes([chunk[0], chunk[1]]));
        }
        Ok(out)
    }

    /// Anzahl archivierter Übergänge (für Tests/Monitoring).
    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_kodierung_rundtrip() {
        let coder = XorParityCoder::new(4);
        let data: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        let fragments = coder.encode(&data);
        assert_eq!(fragments.len(), 5); // 4 Daten + 1 Parität
        let present: Vec<Option<Vec<u8>>> = fragments.iter().map(|f| Some(f.clone())).collect();
        let back = coder.decode(&present).expect("Dekodierung");
        assert_eq!(back, data);
    }

    #[test]
    fn xor_rekonstruktion_nach_einem_verlust() {
        let coder = XorParityCoder::new(4);
        let data: Vec<u8> = (0..512u32).map(|i| (i % 256) as u8).collect();
        let fragments = coder.encode(&data);
        // Fragment 2 (Daten-Block) verlieren.
        let mut with_loss: Vec<Option<Vec<u8>>> =
            fragments.iter().map(|f| Some(f.clone())).collect();
        with_loss[2] = None;
        let back = coder.decode(&with_loss).expect("Rekonstruktion");
        assert_eq!(back, data);
    }

    #[test]
    fn zwei_verluste_werden_abgelehnt() {
        let coder = XorParityCoder::new(4);
        let data: Vec<u8> = (0..256u32).map(|i| (i % 256) as u8).collect();
        let fragments = coder.encode(&data);
        let mut with_loss: Vec<Option<Vec<u8>>> =
            fragments.iter().map(|f| Some(f.clone())).collect();
        with_loss[1] = None;
        with_loss[2] = None;
        assert!(coder.decode(&with_loss).is_err());
    }

    #[test]
    fn da_store_archiviert_und_rekonstruiert() {
        let mut store = DaStore::new(Box::new(XorParityCoder::new(4)));
        let seg = [9u8; 32];
        let akt: Vec<i16> = (0..128).map(|i| (i as i16) * 3 - 100).collect();
        store.put(seg, 1, &akt);
        assert_eq!(store.len(), 1);
        let back = store.get(seg, 1).expect("Rekonstruktion");
        assert_eq!(back, akt);
        // Unbekanntes Segment ⇒ Fehler.
        assert!(store.get([0u8; 32], 1).is_err());
    }
}
