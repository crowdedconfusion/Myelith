//! Validierungstest für das Konformitätspaket (Phase 2.3).
//!
//! Lädt die Golden Vectors aus `conformance/vectors/` und prüft, dass
//! die Referenz-Implementierung die erwarteten Ausgaben erzeugt.

use myl_types::bls::{aggregate_signatures, BlsSecretKey};
use myl_types::hash::Hash;
use myl_types::merkle::MerkleTree;
use myl_types::vrf::VrfSecretKey;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
struct HashVector {
    name: String,
    input: String,
    hash: String,
}

#[derive(Deserialize)]
struct MerkleVector {
    name: String,
    leaves: Vec<String>,
    /// Blattzahl, die in die Wurzel eingeht (Fund 77).
    leaf_count: u64,
    root: String,
    proof_index: usize,
    proof_siblings: Vec<String>,
}

#[derive(Deserialize)]
struct VrfVector {
    name: String,
    alpha: String,
    public_key: String,
    proof: String,
    output: String,
}


#[test]
fn validate_hash_vectors() {
    let path = Path::new("conformance/vectors/hash.json");
    let content = fs::read_to_string(path).expect("read hash.json");
    let vectors: Vec<HashVector> = serde_json::from_str(&content).expect("parse hash.json");

    for v in &vectors {
        let computed = Hash::sha256(v.input.as_bytes());
        assert_eq!(
            computed.to_hex(),
            v.hash,
            "Hash mismatch for vector '{}'",
            v.name
        );
    }

    println!("✓ Validated {} hash vectors", vectors.len());
}

#[test]
fn validate_merkle_vectors() {
    let path = Path::new("conformance/vectors/merkle.json");
    let content = fs::read_to_string(path).expect("read merkle.json");
    let vectors: Vec<MerkleVector> = serde_json::from_str(&content).expect("parse merkle.json");

    for v in &vectors {
        let leaves: Vec<&[u8]> = v.leaves.iter().map(|s| s.as_bytes()).collect();
        let tree = MerkleTree::new(&leaves).expect("build tree");

        assert_eq!(
            tree.root().to_hex(),
            v.root,
            "Merkle root mismatch for vector '{}'",
            v.name
        );

        let proof = tree.proof(v.proof_index).expect("generate proof");
        assert_eq!(
            proof.siblings.len(),
            v.proof_siblings.len(),
            "Proof length mismatch for vector '{}'",
            v.name
        );

        for (i, sibling) in proof.siblings.iter().enumerate() {
            assert_eq!(
                sibling.to_hex(),
                v.proof_siblings[i],
                "Proof sibling {} mismatch for vector '{}'",
                i,
                v.name
            );
        }

        // Fund 77: Die Blattzahl gehoert zum Vektor, weil sie in die
        // Wurzel eingeht. Eine Drittimplementierung, die sie weglaesst,
        // rechnet eine andere Wurzel.
        assert_eq!(
            proof.leaf_count, v.leaf_count,
            "Leaf count mismatch for vector '{}'",
            v.name
        );
        assert_eq!(
            v.leaf_count as usize,
            v.leaves.len(),
            "Vector '{}' widerspricht sich selbst",
            v.name
        );
    }

    // ⚑ Fund 77 als eigene Zusicherung ueber die Vektoren hinweg: Eine
    // Umsetzung im Bitcoin-Stil erzeugt fuer diese beiden Folgen
    // dieselbe Wurzel. Wer die Vektoren einzeln prueft, merkt das nicht;
    // die Aussage steht zwischen zwei Vektoren, nicht in einem.
    let hole = |name: &str| -> &MerkleVector {
        vectors
            .iter()
            .find(|v| v.name == name)
            .unwrap_or_else(|| panic!("Vektor '{}' fehlt", name))
    };
    assert_ne!(
        hole("three_leaves").root,
        hole("four_leaves_last_repeated").root,
        "Fund 77: verschiedene Blattfolgen mit gleicher Wurzel"
    );

    println!("✓ Validated {} merkle vectors", vectors.len());
}

#[test]
fn validate_vrf_vectors() {
    let path = Path::new("conformance/vectors/vrf.json");
    let content = fs::read_to_string(path).expect("read vrf.json");
    let vectors: Vec<VrfVector> = serde_json::from_str(&content).expect("parse vrf.json");

    // Fester Seed (muss mit generate_golden_vectors.rs übereinstimmen)
    let seed = [42u8; 32];
    let sk = VrfSecretKey::from_seed(seed);
    let pk = sk.public_key();

    assert_eq!(
        hex::encode(pk.0),
        vectors[0].public_key,
        "VRF public key mismatch"
    );

    for v in &vectors {
        let (proof, output) = sk.prove(v.alpha.as_bytes()).expect("VRF prove");

        assert_eq!(
            hex::encode(proof.0),
            v.proof,
            "VRF proof mismatch for vector '{}'",
            v.name
        );
        assert_eq!(
            hex::encode(output.beta),
            v.output,
            "VRF output mismatch for vector '{}'",
            v.name
        );
    }

    println!("✓ Validated {} VRF vectors", vectors.len());
}

#[test]
fn validate_bls_vectors() {
    let path = Path::new("conformance/vectors/bls.json");
    let content = fs::read_to_string(path).expect("read bls.json");
    let vectors: Vec<serde_json::Value> = serde_json::from_str(&content).expect("parse bls.json");

    // Fester Seed (muss mit generate_golden_vectors.rs übereinstimmen)
    let ikm = [99u8; 32];
    let sk = BlsSecretKey::key_gen(&ikm).expect("key gen");
    let pk = sk.public_key().expect("public key");

    // Erste 4 Vektoren sind Einzelsignaturen.
    //
    // Drei Stufen, nicht nur eine: Bis v0.2.5 wurde ausschliesslich die
    // erzeugte Signatur mit der gespeicherten verglichen. Damit war der
    // im Vektor mitgelieferte `public_key` nie geprueft, und die
    // Verifikationsrichtung — genau das, was eine Fremdimplementierung
    // gegen das Konformitaetspaket braucht — lief nie. Die
    // Compiler-Warnung „unused variable: pk" war der Hinweis darauf.
    for v in vectors.iter().take(4) {
        let name = v["name"].as_str().unwrap();
        let msg = v["message"].as_str().unwrap();
        let expected_sig = v["signature"].as_str().unwrap();
        let expected_pk = v["public_key"].as_str().unwrap();

        // 1. Der oeffentliche Schluessel des Vektors muss aus dem
        //    dokumentierten Seed hervorgehen.
        assert_eq!(
            hex::encode(pk.0),
            expected_pk,
            "BLS public key mismatch for vector '{}'",
            name
        );

        // 2. Signieren ist deterministisch und liefert den Vektor.
        let sig = sk.sign(msg.as_bytes()).expect("sign");
        assert_eq!(
            hex::encode(sig.0),
            expected_sig,
            "BLS signature mismatch for vector '{}'",
            name
        );

        // 3. Die gespeicherte Signatur verifiziert gegen den
        //    gespeicherten Schluessel.
        assert!(
            pk.verify(msg.as_bytes(), &sig),
            "BLS signature does not verify for vector '{}'",
            name
        );
    }

    // Letzter Vektor ist Aggregation
    let agg_vec = &vectors[4];
    assert_eq!(agg_vec["name"].as_str().unwrap(), "bls_aggregate_3");

    let msg = agg_vec["message"].as_str().unwrap();
    let expected_agg = agg_vec["aggregate_signature"].as_str().unwrap();

    // Drei Schlüssel (müssen mit generate_golden_vectors.rs übereinstimmen)
    let sk2 = BlsSecretKey::key_gen(&[100u8; 32]).expect("key gen");
    let sk3 = BlsSecretKey::key_gen(&[101u8; 32]).expect("key gen");

    let sig1 = sk.sign(msg.as_bytes()).expect("sign");
    let sig2 = sk2.sign(msg.as_bytes()).expect("sign");
    let sig3 = sk3.sign(msg.as_bytes()).expect("sign");

    let agg_sig = aggregate_signatures(&[sig1, sig2, sig3]).expect("aggregate");
    assert_eq!(
        hex::encode(agg_sig.0),
        expected_agg,
        "BLS aggregate signature mismatch"
    );

    println!("✓ Validated {} BLS vectors (4 single + 1 aggregate)", vectors.len());
}
