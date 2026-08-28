//! DA-Archivierung (Data Availability, Anhang A.3 Schritt 6).
//!
//! Aktivierungen werden erasure-codiert für die Streitfrist archiviert.
//! Die Erasure-Coding-Schnittstelle [`ErasureCoder`] trennt die
//! Kodierungs-Strategie von der Archivierung; Phase 1 liefert eine
//! XOR-Paritäts-Kodierung (k Daten-Fragmente + 1 Parität, toleriert den
//! Verlust eines Fragments). Die beschlossene Reed-Solomon-Variante
//! (k=8/m=4, CONSENSUS/TOKENOMICS-Design-Entscheidung, toleriert 4
//! verlorene Fragmente) ist eine Folge-Implementierung hinter derselben
//! Schnittstelle — seit 2026-08-19 als [`ReedSolomonCoder`] vorhanden.
//!
//! **Welchen nehmen?** [`ReedSolomonCoder`]. [`XorParityCoder`] bleibt
//! als Phase-1-Kodierung erhalten (kleiner, kein Körperrechnen), hat
//! aber zwei Nachteile, die im Betrieb zählen: Er toleriert nur **ein**
//! fehlendes Fragment statt vier, und er kann Fragment 0 nicht
//! rekonstruieren, weil dort ungeschützt der Längenkopf liegt.
//! [`ReedSolomonCoder`] codiert den Kopf mit und hat damit kein
//! ausgezeichnetes Fragment.
// Der Index ist die Fragmentnummer und geht in den Schluessel ein.
#![allow(clippy::needless_range_loop)]

use myl_types::erasure::{ErasureCoder as GfCoder, Fragment};
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

/// Reed-Solomon-Kodierung k=8/m=4 hinter der [`ErasureCoder`]-Schnittstelle.
///
/// Setzt auf [`myl_types::erasure`] auf — die Körperarithmetik gehört zu
/// den Primitiven, nicht in diese Komponente. Toleriert den Verlust
/// **beliebiger** `m` Fragmente.
///
/// **Der Längenkopf liegt in den codierten Daten, nicht in Fragment 0.**
/// [`XorParityCoder`] stellt den Kopf ungeschützt an den Anfang von
/// Fragment 0 und kann deshalb nicht rekonstruieren, wenn ausgerechnet
/// dieses Fragment fehlt — eine im Modulkopf vermerkte
/// Phase-1-Einschränkung. Hier wird die Länge dem Klartext
/// vorangestellt und **mitcodiert**; damit gibt es kein ausgezeichnetes
/// Fragment, und jede Teilmenge der Größe `k` genügt.
#[derive(Debug, Clone, Copy)]
pub struct ReedSolomonCoder {
    coder: GfCoder,
}

impl ReedSolomonCoder {
    /// Neuer Codierer mit `k` Daten- und `m` Paritätsfragmenten.
    ///
    /// **Panics:** bei ungültiger Parametrierung (`k` oder `m` gleich
    /// null, `k + m > 255`). Die Parameter sind Konfiguration, kein
    /// Laufzeit-Eingabewert.
    pub fn new(k: usize, m: usize) -> Self {
        Self {
            coder: GfCoder::new(k, m).expect("gültige Erasure-Parameter"),
        }
    }
}

impl Default for ReedSolomonCoder {
    /// k=8/m=4 — die beschlossene Variante (Design-Entscheidung 5).
    fn default() -> Self {
        Self {
            coder: GfCoder::default(),
        }
    }
}

impl ErasureCoder for ReedSolomonCoder {
    fn encode(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut mit_kopf = Vec::with_capacity(8 + data.len());
        mit_kopf.extend_from_slice(&(data.len() as u64).to_le_bytes());
        mit_kopf.extend_from_slice(data);
        self.coder
            .encode(&mit_kopf)
            .expect("nicht-leere Eingabe durch den Kopf garantiert")
            .into_iter()
            .map(|f| f.data)
            .collect()
    }

    fn decode(&self, fragments: &[Option<Vec<u8>>]) -> Result<Vec<u8>, String> {
        let n = self.coder.n();
        if fragments.len() != n {
            return Err(format!(
                "erwartet {} Fragmente, erhalten {}",
                n,
                fragments.len()
            ));
        }
        let vorhanden: Vec<Fragment> = fragments
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                f.as_ref().map(|d| Fragment {
                    index: i,
                    data: d.clone(),
                })
            })
            .collect();
        let roh = self.coder.decode(&vorhanden).map_err(|e| e.to_string())?;
        if roh.len() < 8 {
            return Err("rekonstruierte Daten zu kurz für den Längenkopf".to_string());
        }
        let laenge = u64::from_le_bytes(roh[..8].try_into().expect("8 Bytes")) as usize;
        if laenge > roh.len() - 8 {
            return Err(format!(
                "Längenkopf {} übersteigt die rekonstruierten Daten ({})",
                laenge,
                roh.len() - 8
            ));
        }
        Ok(roh[8..8 + laenge].to_vec())
    }

    fn data_fragments(&self) -> usize {
        self.coder.k()
    }

    fn parity_fragments(&self) -> usize {
        self.coder.m()
    }
}

/// DA-Archiv: speichert erasure-codierte Fragmente je Segment und Shard
/// und hält sie für die Streitfrist vor.
pub struct DaStore {
    coder: Box<dyn ErasureCoder>,
    /// (segment_id, layer_index) → Fragmente.
    ///
    /// **Der zweite Schlüsselteil war bis 2026-08-23 der Shard-Index, und
    /// eine Positionsachse gab es nicht.** `archive` wird je
    /// Token-Position aufgerufen; jede Position überschrieb deshalb die
    /// vorige, und am Ende eines Laufs lag nur noch die letzte im Archiv.
    /// Ein Angeklagter hätte die Aktivierung jeder früheren Position
    /// nicht liefern können, `adjudicate` hätte `NoResponse` gesehen, und
    /// das heißt schuldig: **ein ehrlicher Knoten wäre geslasht worden,
    /// weil das Archiv seine Arbeit nicht aufbewahrt hat.**
    ///
    /// Seit der Festlegung „ein Segment ist eine Position" (2026-08-23)
    /// braucht es keine Positionsachse: Ein Segment ist genau ein
    /// Vorwärtspass. Der zweite Schlüsselteil ist jetzt der **Layer**,
    /// womit die Ablage dieselbe Achse trägt wie die Spur und die
    /// Bisektion.
    store: BTreeMap<([u8; 32], u64), Vec<Vec<u8>>>,
}

impl DaStore {
    pub fn new(coder: Box<dyn ErasureCoder>) -> Self {
        Self {
            coder,
            store: BTreeMap::new(),
        }
    }

    /// Archiviert die Ausgabe-Aktivierungen **einer Layer** (Anhang A.3
    /// Schritt 6).
    ///
    /// `layer_index` ist der Index im Modell, nicht im Shard: Zwei Pods
    /// mit verschiedenem Zuschnitt archivieren dieselben Schlüssel.
    pub fn put(&mut self, segment_id: [u8; 32], layer_index: u64, activations: &[i16]) {
        let mut bytes = Vec::with_capacity(activations.len() * 2);
        for v in activations {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let fragments = self.coder.encode(&bytes);
        self.store.insert((segment_id, layer_index), fragments);
    }

    /// Rekonstruiert die Ausgabe-Aktivierungen einer Layer.
    pub fn get(&self, segment_id: [u8; 32], layer_index: u64) -> Result<Vec<i16>, String> {
        let fragments = self
            .store
            .get(&(segment_id, layer_index))
            .ok_or("Segment/Layer nicht archiviert")?;
        let present: Vec<Option<Vec<u8>>> = fragments.iter().map(|f| Some(f.clone())).collect();
        let bytes = self.coder.decode(&present)?;
        if bytes.len() % 2 != 0 {
            return Err("rekonstruierte Daten haben ungerade Länge".to_string());
        }
        let mut out = Vec::with_capacity(bytes.len() / 2);
        // clippy schlaegt seit 1.98 `as_chunks::<2>()` vor, stabil erst seit
        // Rust 1.88. Dieses Crate erklaert MSRV 1.85 (Cargo.toml).
        // `unknown_lints` muss mit erlaubt sein: Den Lint-Namen gibt es erst
        // ab clippy 1.98, ein `allow` darauf ist auf aelteren Werkzeugketten
        // selbst eine Warnung. So baut es mit beiden.
        #[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
        for chunk in bytes.chunks_exact(2) {
            out.push(i16::from_le_bytes([chunk[0], chunk[1]]));
        }
        Ok(out)
    }

    /// Anzahl archivierter Layer-Ausgänge (für Tests/Monitoring).
    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::ReedSolomonCoder;

    /// Reed-Solomon vertraegt den Verlust **beliebiger** m Fragmente —
    /// auch Fragment 0, an dem XorParityCoder scheitert.
    #[test]
    fn rs_rekonstruiert_ohne_fragment_null() {
        let coder = ReedSolomonCoder::default();
        let daten: Vec<u8> = (0..250u8).collect();
        let fragmente = coder.encode(&daten);
        assert_eq!(fragmente.len(), 12);

        let mut teil: Vec<Option<Vec<u8>>> = fragmente.iter().cloned().map(Some).collect();
        for i in [0usize, 3, 7, 11] {
            teil[i] = None;
        }
        assert_eq!(coder.decode(&teil).expect("decode"), daten);
    }

    #[test]
    fn rs_traegt_jede_kombination_von_vier_ausfaellen() {
        let coder = ReedSolomonCoder::default();
        let daten: Vec<u8> = (0..97u8).map(|i| i.wrapping_mul(11)).collect();
        let fragmente = coder.encode(&daten);
        let mut geprueft = 0;
        for maske in 0u32..(1 << 12) {
            if maske.count_ones() != 4 {
                continue;
            }
            let teil: Vec<Option<Vec<u8>>> = fragmente
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    if maske & (1 << i) != 0 {
                        None
                    } else {
                        Some(f.clone())
                    }
                })
                .collect();
            assert_eq!(coder.decode(&teil).expect("decode"), daten, "Maske {:012b}", maske);
            geprueft += 1;
        }
        assert_eq!(geprueft, 495);
    }

    #[test]
    fn rs_meldet_zu_viele_ausfaelle_als_fehler() {
        let coder = ReedSolomonCoder::default();
        let daten: Vec<u8> = (0..64u8).collect();
        let fragmente = coder.encode(&daten);
        let mut teil: Vec<Option<Vec<u8>>> = fragmente.iter().cloned().map(Some).collect();
        for i in 0..5 {
            teil[i] = None;
        }
        assert!(coder.decode(&teil).is_err());
    }

    #[test]
    fn rs_meldet_vier_paritaetsfragmente() {
        let coder = ReedSolomonCoder::default();
        assert_eq!(coder.data_fragments(), 8);
        assert_eq!(coder.parity_fragments(), 4);
    }

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
