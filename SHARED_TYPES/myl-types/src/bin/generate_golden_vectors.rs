//! Golden Vector Generator für SHARED_TYPES Phase 2.1
//!
//! Erzeugt deterministische Testvektoren für Hash, Merkle, VRF und BLS.
//! Diese Vektoren dienen als Referenz für Drittimplementierungen und
//! werden im `conformance/`-Verzeichnis eingefroren.

use myl_types::bls::{aggregate_signatures, BlsSecretKey};
use myl_types::hash::Hash;
use myl_types::merkle::{MerkleProof, MerkleTree};
use myl_types::vrf::VrfSecretKey;
use std::fs;
use std::path::Path;

fn main() {
    let output_dir = Path::new("tests/golden_vectors");
    fs::create_dir_all(output_dir).expect("create golden vectors dir");

    generate_hash_vectors(output_dir);
    generate_merkle_vectors(output_dir);
    generate_vrf_vectors(output_dir);
    generate_bls_vectors(output_dir);

    println!("Golden vectors generated in {:?}", output_dir);
}

fn generate_hash_vectors(output_dir: &Path) {
    let mut vectors = Vec::new();

    // Leere Eingabe
    let empty_hash = Hash::sha256(b"");
    vectors.push(format!(
        r#"{{"name":"empty","input":"","hash":"{}"}}"#,
        empty_hash.to_hex()
    ));

    // "abc" (NIST-Testvektor)
    let abc_hash = Hash::sha256(b"abc");
    vectors.push(format!(
        r#"{{"name":"abc","input":"abc","hash":"{}"}}"#,
        abc_hash.to_hex()
    ));

    // Myelith-spezifisch
    let myelith_hash = Hash::sha256(b"myelith-protocol-v1");
    vectors.push(format!(
        r#"{{"name":"myelith_v1","input":"myelith-protocol-v1","hash":"{}"}}"#,
        myelith_hash.to_hex()
    ));

    // Längere Eingabe
    let long_input = "a".repeat(1000);
    let long_hash = Hash::sha256(long_input.as_bytes());
    vectors.push(format!(
        r#"{{"name":"long_1000_a","input":"{}","hash":"{}"}}"#,
        long_input,
        long_hash.to_hex()
    ));

    let content = format!("[{}]", vectors.join(",\n"));
    fs::write(output_dir.join("hash.json"), content).expect("write hash vectors");
    println!("  ✓ hash.json ({} vectors)", vectors.len());
}

fn generate_merkle_vectors(output_dir: &Path) {
    let mut vectors = Vec::new();

    // Ein Blatt
    let leaves1 = vec![b"leaf-0".as_slice()];
    let tree1 = MerkleTree::new(&leaves1).unwrap();
    let proof1 = tree1.proof(0).unwrap();
    vectors.push(merkle_vector_json("single_leaf", &leaves1, &tree1, &proof1, 0));

    // Zwei Blätter
    let leaves2 = vec![b"leaf-0".as_slice(), b"leaf-1".as_slice()];
    let tree2 = MerkleTree::new(&leaves2).unwrap();
    let proof2 = tree2.proof(0).unwrap();
    vectors.push(merkle_vector_json("two_leaves", &leaves2, &tree2, &proof2, 0));

    // Drei Blätter (ungerade, mit Duplikation)
    let leaves3 = vec![
        b"leaf-0".as_slice(),
        b"leaf-1".as_slice(),
        b"leaf-2".as_slice(),
    ];
    let tree3 = MerkleTree::new(&leaves3).unwrap();
    let proof3 = tree3.proof(1).unwrap();
    vectors.push(merkle_vector_json("three_leaves", &leaves3, &tree3, &proof3, 1));

    // Acht Blätter (vollständiger Binärbaum)
    let leaves8: Vec<Vec<u8>> = (0..8).map(|i| format!("leaf-{}", i).into_bytes()).collect();
    let leaves8_refs: Vec<&[u8]> = leaves8.iter().map(|v| v.as_slice()).collect();
    let tree8 = MerkleTree::new(&leaves8_refs).unwrap();
    let proof8 = tree8.proof(5).unwrap();
    vectors.push(merkle_vector_json("eight_leaves", &leaves8_refs, &tree8, &proof8, 5));

    let content = format!("[{}]", vectors.join(",\n"));
    fs::write(output_dir.join("merkle.json"), content).expect("write merkle vectors");
    println!("  ✓ merkle.json ({} vectors)", vectors.len());
}

fn merkle_vector_json(
    name: &str,
    leaves: &[&[u8]],
    tree: &MerkleTree,
    proof: &MerkleProof,
    leaf_index: usize,
) -> String {
    let leaves_json: Vec<String> = leaves
        .iter()
        .map(|l| format!("\"{}\"", String::from_utf8_lossy(l)))
        .collect();
    let siblings_json: Vec<String> = proof
        .siblings
        .iter()
        .map(|s| format!("\"{}\"", s.to_hex()))
        .collect();

    format!(
        r#"{{"name":"{}","leaves":[{}],"root":"{}","proof_index":{},"proof_siblings":[{}]}}"#,
        name,
        leaves_json.join(","),
        tree.root().to_hex(),
        leaf_index,
        siblings_json.join(",")
    )
}

fn generate_vrf_vectors(output_dir: &Path) {
    let mut vectors = Vec::new();

    // Fester Seed für reproduzierbare Schlüssel
    let seed = [42u8; 32];
    let sk = VrfSecretKey::from_seed(seed);
    let pk = sk.public_key();

    // Verschiedene Alpha-Strings
    let alphas = vec![
        "epoch-1",
        "epoch-2",
        "control-segment-0",
        "training-data-batch-100",
        "",
    ];

    for alpha in alphas {
        let (proof, output) = sk.prove(alpha.as_bytes()).expect("VRF prove");

        vectors.push(format!(
            r#"{{"name":"vrf_{}","alpha":"{}","public_key":"{}","proof":"{}","output":"{}"}}"#,
            alpha.replace("-", "_").replace(" ", "_"),
            alpha,
            hex::encode(pk.0),
            hex::encode(proof.0),
            hex::encode(output.beta)
        ));
    }

    let content = format!("[{}]", vectors.join(",\n"));
    fs::write(output_dir.join("vrf.json"), content).expect("write vrf vectors");
    println!("  ✓ vrf.json ({} vectors)", vectors.len());
}

fn generate_bls_vectors(output_dir: &Path) {
    let mut vectors = Vec::new();

    // Fester Seed für reproduzierbare Schlüssel
    let ikm = [99u8; 32];
    let sk = BlsSecretKey::key_gen(&ikm).expect("key gen");
    let pk = sk.public_key().expect("public key");

    // Verschiedene Nachrichten
    let messages = vec![
        "block-header-1",
        "poi-bundle-42",
        "validator-registration",
        "",
    ];

    for msg in messages {
        let sig = sk.sign(msg.as_bytes()).expect("sign");

        vectors.push(format!(
            r#"{{"name":"bls_{}","message":"{}","public_key":"{}","signature":"{}"}}"#,
            msg.replace("-", "_").replace(" ", "_"),
            msg,
            hex::encode(pk.0),
            hex::encode(sig.0)
        ));
    }

    // Aggregation: 3 Signaturen
    let sk2 = BlsSecretKey::key_gen(&[100u8; 32]).expect("key gen");
    let sk3 = BlsSecretKey::key_gen(&[101u8; 32]).expect("key gen");
    let pk2 = sk2.public_key().expect("public key");
    let pk3 = sk3.public_key().expect("public key");

    let msg = "aggregate-test";
    let sig1 = sk.sign(msg.as_bytes()).expect("sign");
    let sig2 = sk2.sign(msg.as_bytes()).expect("sign");
    let sig3 = sk3.sign(msg.as_bytes()).expect("sign");

    let agg_sig = aggregate_signatures(&[sig1, sig2, sig3]).expect("aggregate");

    vectors.push(format!(
        r#"{{"name":"bls_aggregate_3","message":"{}","public_keys":["{}","{}","{}"],"signatures":["{}","{}","{}"],"aggregate_signature":"{}"}}"#,
        msg,
        hex::encode(pk.0),
        hex::encode(pk2.0),
        hex::encode(pk3.0),
        hex::encode(sig1.0),
        hex::encode(sig2.0),
        hex::encode(sig3.0),
        hex::encode(agg_sig.0)
    ));

    let content = format!("[{}]", vectors.join(",\n"));
    fs::write(output_dir.join("bls.json"), content).expect("write bls vectors");
    println!("  ✓ bls.json ({} vectors)", vectors.len());
}
