//! Integer-Sampling (Greedy + CDF)

use crate::prng::splitmix64;

/// Argmax ueber Integer-Logits.
pub fn argmax_int(values: &[i32]) -> usize {
    let mut best_i = 0;
    let mut best_v = values[0];
    for i in 1..values.len() {
        if values[i] > best_v {
            best_v = values[i];
            best_i = i;
        }
    }
    best_i
}

/// Wandelt Logits in positive Integer-Gewichte um.
pub fn logits_to_weights(logits: &[i32]) -> Vec<i32> {
    let m = logits.iter().copied().min().unwrap_or(0);
    logits.iter().map(|z| z - m + 1).collect()
}

/// Deterministisches Sampling via Integer-CDF und SplitMix64.
pub fn sample_integer_cdf(logits: &[i32], state: u64) -> (usize, u64) {
    let weights = logits_to_weights(logits);
    let total: i64 = weights.iter().map(|w| *w as i64).sum();

    if total <= 0 {
        return (0, state);
    }

    let (new_state, r) = splitmix64(state);
    let threshold = (r % total as u64) as i64;

    let mut acc: i64 = 0;
    for (i, w) in weights.iter().enumerate() {
        acc += *w as i64;
        if threshold < acc {
            return (i, new_state);
        }
    }
    (weights.len() - 1, new_state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argmax() {
        let logits = vec![1, 9, 3];
        assert_eq!(argmax_int(&logits), 1);
    }

    #[test]
    fn test_sampling_deterministic() {
        let logits = vec![10, 20, 30];
        let state = 42u64;
        let (t1, s1) = sample_integer_cdf(&logits, state);
        let (t2, s2) = sample_integer_cdf(&logits, state);
        assert_eq!(t1, t2);
        assert_eq!(s1, s2);
    }
}
