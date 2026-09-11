//! Integer-KV-Cache
//! 
//! Jeder Layer und jeder Head haelt Keys und Values als INT16-Fixed-Point.

use std::collections::BTreeMap;

pub struct KVCache {
    // layer -> head -> position -> vec
    k: BTreeMap<usize, BTreeMap<usize, BTreeMap<usize, Vec<i16>>>>,
    v: BTreeMap<usize, BTreeMap<usize, BTreeMap<usize, Vec<i16>>>>,
}

impl KVCache {
    pub fn new(num_layers: usize, num_heads: usize) -> Self {
        Self::for_range(0, num_layers, num_heads)
    }

    /// KV-Cache für einen Layer-Bereich `[layer_start, layer_end)` —
    /// für Pipeline-Stages, die nur ihre eigenen Layer halten
    /// (indiziert wird mit den absoluten Layer-Indizes, siehe
    /// `TransformerLayer.layer_idx`).
    pub fn for_range(layer_start: usize, layer_end: usize, num_heads: usize) -> Self {
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
        KVCache { k, v }
    }

    pub fn write(&mut self, layer: usize, head: usize, pos: usize, key: Vec<i16>, value: Vec<i16>) {
        self.k.get_mut(&layer).unwrap().get_mut(&head).unwrap().insert(pos, key);
        self.v.get_mut(&layer).unwrap().get_mut(&head).unwrap().insert(pos, value);
    }

    /// **Die Positionen bis `upto`, als Ausschnitte statt als Kopien.**
    ///
    /// # ⛑ Warum es diesen zweiten Eingang gibt
    ///
    /// [`KVCache::read`] gibt eigene Vektoren heraus und kopiert dafuer
    /// **jede** gespeicherte Position. Die Aufmerksamkeit ruft ihn je
    /// Kopf und je Token; bei einem Prompt von 169 Token sind das ueber
    /// 36 Ebenen rund **8 GB kopierte Bytes und eine Million
    /// Belegungen**, fuer Daten, die unveraendert danebenliegen.
    ///
    /// ⚑ **Gelesen wird dasselbe, in derselben Reihenfolge.** Der
    /// `BTreeMap` haelt die Positionen sortiert; hier wie dort kommen
    /// sie aufsteigend heraus.
    pub fn read_scheiben(
        &self,
        layer: usize,
        head: usize,
        upto: usize,
    ) -> (Vec<&[i16]>, Vec<&[i16]>) {
        let k_head = self.k.get(&layer).unwrap().get(&head).unwrap();
        let v_head = self.v.get(&layer).unwrap().get(&head).unwrap();
        let keys: Vec<&[i16]> = k_head
            .range(..=upto)
            .map(|(_, v)| v.as_slice())
            .collect();
        let values: Vec<&[i16]> = v_head
            .range(..=upto)
            .map(|(_, v)| v.as_slice())
            .collect();
        (keys, values)
    }

    pub fn read(&self, layer: usize, head: usize, upto: usize) -> (Vec<Vec<i16>>, Vec<Vec<i16>>) {
        let k_head = self.k.get(&layer).unwrap().get(&head).unwrap();
        let v_head = self.v.get(&layer).unwrap().get(&head).unwrap();

        let positions: Vec<usize> = k_head.keys().filter(|&&p| p <= upto).copied().collect();
        let keys: Vec<Vec<i16>> = positions.iter().map(|p| k_head[p].clone()).collect();
        let values: Vec<Vec<i16>> = positions.iter().map(|p| v_head[p].clone()).collect();

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
}
