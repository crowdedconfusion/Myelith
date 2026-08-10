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
                for (_, vec) in k_head {
                    total += vec.len() * 2; // i16 = 2 bytes
                }
            }
        }
        total * 2 // K + V
    }
}
