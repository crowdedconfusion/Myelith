//! Deterministischer Integer-PRNG

const MASK64: u64 = u64::MAX;

/// SplitMix64
#[inline(always)]
pub fn splitmix64(state: u64) -> (u64, u64) {
    let state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^= z >> 31;
    (state, z & MASK64)
}

/// Seed-Ableitung aus segment_id und block_hash.
#[inline]
pub fn seed_from_ids(segment_id: u64, block_hash: u64) -> u64 {
    let mut state = 0u64;
    state ^= segment_id;
    state = state.wrapping_mul(0x9E3779B97F4A7C15);
    state ^= block_hash;
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_splitmix64_reproducible() {
        let s0 = 42u64;
        let (s1, z1) = splitmix64(s0);
        let (s2, z2) = splitmix64(s1);
        let (s1b, z1b) = splitmix64(s0);
        assert_eq!(s1, s1b);
        assert_eq!(z1, z1b);
        assert_ne!(z1, z2);
    }
}
