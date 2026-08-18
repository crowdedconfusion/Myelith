//! Verteilter KV-Cache – Jeder Node haelt nur seine Layer-Bereiche.
//! 
//! Dies ist die Pipeline-Variante des KV-Caches.
//! Single-Node-Version ist in runtime/src/kv_cache.rs.

use std::collections::BTreeMap;

/// KV-Cache fuer eine Pipeline-Stage (nur bestimmte Layer).
pub struct DistributedKVCache {
    layer_start: usize,
    layer_end: usize,
    num_heads: usize,
    // layer -> head -> position -> vec
    k: BTreeMap<usize, BTreeMap<usize, BTreeMap<usize, Vec<i16>>>>,
    v: BTreeMap<usize, BTreeMap<usize, BTreeMap<usize, Vec<i16>>>>,
}

impl DistributedKVCache {
    pub fn new(layer_start: usize, layer_end: usize, num_heads: usize) -> Self {
        let mut k = BTreeMap::new();
        let mut v = BTreeMap::new();
        for l in layer_start..layer_end {
            let mut k_heads = BTreeMap::new();
            let mut v_heads = BTreeMap::new();
            for h in 0..num_heads {
                k_heads.insert(h, BTreeMap::new());
                v_heads.insert(h, BTreeMap::new());
            }
            k.insert(l, k_heads);
            v.insert(l, v_heads);
        }
        DistributedKVCache {
            layer_start,
            layer_end,
            num_heads,
            k,
            v,
        }
    }
    
    pub fn write(&mut self, layer: usize, head: usize, pos: usize, key: Vec<i16>, value: Vec<i16>) {
        assert!(layer >= self.layer_start && layer < self.layer_end,
                "Layer {} out of range [{}, {})", layer, self.layer_start, self.layer_end);
        self.k.get_mut(&layer).unwrap().get_mut(&head).unwrap().insert(pos, key);
        self.v.get_mut(&layer).unwrap().get_mut(&head).unwrap().insert(pos, value);
    }
    
    pub fn read(&self, layer: usize, head: usize, upto: usize) -> (Vec<Vec<i16>>, Vec<Vec<i16>>) {
        let k_head = self.k.get(&layer).unwrap().get(&head).unwrap();
        let v_head = self.v.get(&layer).unwrap().get(&head).unwrap();
        let positions: Vec<usize> = k_head.keys().filter(|&&p| p <= upto).copied().collect();
        let keys = positions.iter().map(|p| k_head[p].clone()).collect();
        let values = positions.iter().map(|p| v_head[p].clone()).collect();
        (keys, values)
    }
    
    pub fn truncate(&mut self, layer: usize, head: usize, max_len: usize) {
        let k_head = self.k.get_mut(&layer).unwrap().get_mut(&head).unwrap();
        let v_head = self.v.get_mut(&layer).unwrap().get_mut(&head).unwrap();
        let to_remove: Vec<usize> = k_head.keys().filter(|&&p| p >= max_len).copied().collect();
        for pos in to_remove {
            k_head.remove(&pos);
            v_head.remove(&pos);
        }
    }
    
    /// Gesamtspeicher in Bytes (Schaetzung).
    pub fn memory_usage(&self) -> usize {
        let mut total = 0;
        for l in self.layer_start..self.layer_end {
            for h in 0..self.num_heads {
                let k_head = self.k.get(&l).unwrap().get(&h).unwrap();
                for vec in k_head.values() {
                    total += vec.len() * 2; // i16 = 2 bytes
                }
            }
        }
        total * 2 // K + V
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache() -> DistributedKVCache {
        DistributedKVCache::new(4, 8, 2)
    }

    #[test]
    fn schreiben_und_lesen_in_positionsreihenfolge() {
        let mut c = cache();
        // Absichtlich unsortiert schreiben — die Ausgabe muss trotzdem
        // nach Position geordnet sein, sonst waere die Attention
        // reihenfolgenabhaengig und damit nicht bitgleich.
        c.write(4, 0, 2, vec![20], vec![200]);
        c.write(4, 0, 0, vec![0], vec![100]);
        c.write(4, 0, 1, vec![10], vec![150]);

        let (k, v) = c.read(4, 0, 2);
        assert_eq!(k, vec![vec![0], vec![10], vec![20]]);
        assert_eq!(v, vec![vec![100], vec![150], vec![200]]);
    }

    #[test]
    fn upto_begrenzt_die_gelesenen_positionen() {
        let mut c = cache();
        for pos in 0..5 {
            c.write(5, 1, pos, vec![pos as i16], vec![pos as i16 * 2]);
        }
        let (k, _) = c.read(5, 1, 2);
        assert_eq!(k.len(), 3, "Positionen 0..=2");
        let (k_all, _) = c.read(5, 1, 99);
        assert_eq!(k_all.len(), 5);
    }

    #[test]
    fn ueberschreiben_derselben_position_ersetzt() {
        let mut c = cache();
        c.write(4, 0, 0, vec![1], vec![1]);
        c.write(4, 0, 0, vec![9], vec![9]);
        let (k, v) = c.read(4, 0, 0);
        assert_eq!(k, vec![vec![9]]);
        assert_eq!(v, vec![vec![9]]);
    }

    #[test]
    fn truncate_entfernt_ab_max_len() {
        let mut c = cache();
        for pos in 0..6 {
            c.write(6, 0, pos, vec![pos as i16], vec![pos as i16]);
        }
        c.truncate(6, 0, 3);
        let (k, v) = c.read(6, 0, 99);
        assert_eq!(k.len(), 3, "Positionen 0,1,2 bleiben");
        assert_eq!(v.len(), 3);
        assert_eq!(k, vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn truncate_beruehrt_andere_heads_nicht() {
        let mut c = cache();
        c.write(4, 0, 5, vec![1], vec![1]);
        c.write(4, 1, 5, vec![2], vec![2]);
        c.truncate(4, 0, 1);
        assert_eq!(c.read(4, 0, 99).0.len(), 0);
        assert_eq!(c.read(4, 1, 99).0.len(), 1, "Head 1 bleibt unberuehrt");
    }

    #[test]
    fn leerer_cache_liefert_leere_ergebnisse() {
        let c = cache();
        let (k, v) = c.read(4, 0, 100);
        assert!(k.is_empty() && v.is_empty());
        assert_eq!(c.memory_usage(), 0);
    }

    #[test]
    fn memory_usage_zaehlt_k_und_v() {
        let mut c = cache();
        // Ein Eintrag mit 8 i16 = 16 Byte fuer K, gleiche Groesse fuer V.
        c.write(4, 0, 0, vec![0i16; 8], vec![0i16; 8]);
        assert_eq!(c.memory_usage(), 32);

        c.write(4, 0, 1, vec![0i16; 8], vec![0i16; 8]);
        assert_eq!(c.memory_usage(), 64);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn schreiben_ausserhalb_des_layerbereichs_panickt() {
        let mut c = cache();
        c.write(3, 0, 0, vec![1], vec![1]);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn schreiben_oberhalb_des_layerbereichs_panickt() {
        let mut c = cache();
        c.write(8, 0, 0, vec![1], vec![1]);
    }
}
