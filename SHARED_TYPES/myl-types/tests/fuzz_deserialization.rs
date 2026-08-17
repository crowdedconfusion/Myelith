//! Fuzz-Harness für alle Borsh-Deserialisierungspfade (Phase 2.2).
//!
//! Ziel: Sicherstellen, dass kaputte/adversariale Eingaben nur `Err`
//! erzeugen, nie Panic oder UB. Testet alle Typen mit BorshDeserialize.
//!
//! Akzeptanzkriterium: 100.000 Iterationen pro Typ ohne Panic.

use myl_types::bls::{BlsAggregateSignature, BlsPublicKey, BlsSignature};
use myl_types::core_types::{InferenceCredit, PoIBundle, Segment};
use myl_types::hash::Hash;
use myl_types::ids::{Address, EpochId, MerkleRoot, MinerId, PodId, SegmentId};
use myl_types::merkle::MerkleProof;
use myl_types::vrf::{VrfOutput, VrfProof, VrfPublicKey};

/// Einfacher deterministischer PRNG (SplitMix64) für reproduzierbare Tests.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn next_bytes(&mut self, buf: &mut [u8]) {
        let mut i = 0;
        while i < buf.len() {
            let val = self.next_u64();
            let bytes = val.to_le_bytes();
            let remaining = buf.len() - i;
            let to_copy = remaining.min(8);
            buf[i..i + to_copy].copy_from_slice(&bytes[..to_copy]);
            i += to_copy;
        }
    }
}

/// Testet einen Typ: versucht zu deserialisieren, fängt Panics ab.
/// Returns true wenn kein Panic auftrat (Ok oder Err ist beides akzeptabel).
fn fuzz_type<T: borsh::BorshDeserialize>(rng: &mut Rng, size: usize) -> bool {
    let mut buf = vec![0u8; size];
    rng.next_bytes(&mut buf);

    // Versuche zu deserialisieren — sollte nie panic'n
    let result = std::panic::catch_unwind(|| {
        let _ = borsh::from_slice::<T>(&buf);
    });

    result.is_ok()
}

#[test]
fn fuzz_all_deserialization_paths() {
    let mut rng = Rng::new(0xC0FFEE_BEEF);
    let iterations = 100_000;

    println!("Starting fuzz tests with {} iterations per type...", iterations);

    // Teste verschiedene Größen (klein, mittel, groß)
    let sizes = [8, 32, 64, 128, 256, 512];

    // Hash (32 Bytes)
    for _ in 0..iterations {
        for &size in &sizes {
            assert!(
                fuzz_type::<Hash>(&mut rng, size),
                "Hash: Panic bei Deserialisierung"
            );
        }
    }
    println!("  ✓ Hash ({} iterations)", iterations);

    // MerkleProof (variable Länge)
    for _ in 0..iterations {
        for &size in &sizes {
            assert!(
                fuzz_type::<MerkleProof>(&mut rng, size),
                "MerkleProof: Panic bei Deserialisierung"
            );
        }
    }
    println!("  ✓ MerkleProof ({} iterations)", iterations);

    // VRF-Typen
    for _ in 0..iterations {
        for &size in &sizes {
            assert!(
                fuzz_type::<VrfPublicKey>(&mut rng, size),
                "VrfPublicKey: Panic bei Deserialisierung"
            );
            assert!(
                fuzz_type::<VrfProof>(&mut rng, size),
                "VrfProof: Panic bei Deserialisierung"
            );
            assert!(
                fuzz_type::<VrfOutput>(&mut rng, size),
                "VrfOutput: Panic bei Deserialisierung"
            );
        }
    }
    println!("  ✓ VRF types ({} iterations)", iterations);

    // BLS-Typen
    for _ in 0..iterations {
        for &size in &sizes {
            assert!(
                fuzz_type::<BlsPublicKey>(&mut rng, size),
                "BlsPublicKey: Panic bei Deserialisierung"
            );
            assert!(
                fuzz_type::<BlsSignature>(&mut rng, size),
                "BlsSignature: Panic bei Deserialisierung"
            );
            assert!(
                fuzz_type::<BlsAggregateSignature>(&mut rng, size),
                "BlsAggregateSignature: Panic bei Deserialisierung"
            );
        }
    }
    println!("  ✓ BLS types ({} iterations)", iterations);

    // ID-Typen (alle 32 Bytes)
    for _ in 0..iterations {
        for &size in &sizes {
            assert!(
                fuzz_type::<Address>(&mut rng, size),
                "Address: Panic bei Deserialisierung"
            );
            assert!(
                fuzz_type::<MinerId>(&mut rng, size),
                "MinerId: Panic bei Deserialisierung"
            );
            assert!(
                fuzz_type::<PodId>(&mut rng, size),
                "PodId: Panic bei Deserialisierung"
            );
            assert!(
                fuzz_type::<SegmentId>(&mut rng, size),
                "SegmentId: Panic bei Deserialisierung"
            );
            assert!(
                fuzz_type::<EpochId>(&mut rng, size),
                "EpochId: Panic bei Deserialisierung"
            );
            assert!(
                fuzz_type::<MerkleRoot>(&mut rng, size),
                "MerkleRoot: Panic bei Deserialisierung"
            );
        }
    }
    println!("  ✓ ID types ({} iterations)", iterations);

    // Core-Typen (komplexere Strukturen)
    for _ in 0..iterations {
        for &size in &sizes {
            assert!(
                fuzz_type::<Segment>(&mut rng, size),
                "Segment: Panic bei Deserialisierung"
            );
            assert!(
                fuzz_type::<PoIBundle>(&mut rng, size),
                "PoIBundle: Panic bei Deserialisierung"
            );
            assert!(
                fuzz_type::<InferenceCredit>(&mut rng, size),
                "InferenceCredit: Panic bei Deserialisierung"
            );
        }
    }
    println!("  ✓ Core types ({} iterations)", iterations);

    println!(
        "All fuzz tests passed ({} iterations per type, {} sizes)",
        iterations,
        sizes.len()
    );
}
