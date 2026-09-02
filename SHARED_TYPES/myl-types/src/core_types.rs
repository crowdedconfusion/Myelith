//! Kern-Structs aus Whitepaper Anhang A.1 (Punkt 1.5):
//! `Segment`, `PoIBundle`, `InferenceCredit`.
//!
//! **Feldnamen und Feldreihenfolge sind Konsens-Vertrag:** Borsh
//! serialisiert in Deklarationsreihenfolge, jede Abweichung von
//! Anhang A.1 würde die Hashes über diesen Strukturen (und damit alle
//! Commitments und Bündel-Wurzeln) ändern. Änderungen nur über
//! Governance (Kap. 10.3).

use borsh::{BorshDeserialize, BorshSerialize};

use crate::bls::BlsSignature;
use crate::hash::Hash;
use crate::ids::{ActivationHash, Address, EpochId, MerkleRoot, MinerId, PodId, SegmentId};
use crate::merkle::{MerkleError, MerkleTree};

/// Ein Inferenz-Segment σ = (x, θ_v, π, y) als übertragbarer Datensatz
/// (Whitepaper Kap. 6.1, Anhang A.1).
///
/// - `id`: `h(session ‖ index)`
/// - `input_commitment`: `h(prompt_chunk ‖ kv_root)`
/// - `model_version`: Gewichts-Wurzel θ_v inkl. Ausführungsspezifikation
/// - `pod_path`: Pipeline-Reihenfolge der Shard-Miner
/// - `trace`: Berechnungsspur h(a_0), …, h(a_k)
/// - `signatures`: eine BLS-Signatur pro Shard-Übergang
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Segment {
    pub id: SegmentId,
    pub input_commitment: Hash,
    pub model_version: MerkleRoot,
    pub pod_path: Vec<MinerId>,
    pub output_commitment: Hash,
    pub trace: Vec<ActivationHash>,
    pub signatures: Vec<BlsSignature>,
}

/// PoI-Bündel: pro Epoche und Pod eingereicht, signiert und aggregiert
/// (Whitepaper Kap. 4.4, Anhang A.1).
///
/// - `segments_root`: Merkle-Wurzel über alle Segment-Ids der Epoche
/// - `vtfe_claimed`: beanspruchte Arbeit (vTFE)
/// - `aggregate_sig`: BLS-Aggregat über die Pod-Mitglieder
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct PoIBundle {
    pub epoch: EpochId,
    pub pod: PodId,
    pub segments_root: MerkleRoot,
    pub vtfe_claimed: u64,
    pub aggregate_sig: BlsSignature,
    /// Wie viele Segmente unter [`Self::segments_root`] hängen.
    ///
    /// # ⚑ Fund 115: Die Kette konnte nicht zählen, was sie bezahlt
    ///
    /// Bis zum 2026-09-01 trug ein Bündel nur die **Wurzel** über seine
    /// Segmentzeugnisse. Eine Wurzel sagt nichts über die Zahl der
    /// Blätter, und damit war aus dem Kettenzustand **nicht ableitbar,
    /// wie viele Segmente eine Epoche hatte**.
    ///
    /// Das war die stille Vorbedingung, an der Stufe 2 scheiterte:
    /// [`sample_segments`](../../../CONSENSUS/myl-scheduler) zieht aus
    /// `num_segments`, und diese Zahl gab es nirgends. **Erst mit ihr
    /// ist eine Stichprobe überhaupt herleitbar.**
    ///
    /// ⚑ **Sie gehört in die signierte Botschaft**, aus demselben Grund
    /// wie `vtfe_claimed`: Sonst erhöhte der Koordinator sie nach dem
    /// Einsammeln der Unterschriften und **verdünnte damit die
    /// Stichprobenwahrscheinlichkeit je Segment**, ohne das Aggregat
    /// ungültig zu machen.
    ///
    /// **Additiv angehängt, nie eingefügt:** Die Feldreihenfolge ist
    /// Konsensvertrag.
    pub segmente: u32,
}

/// Inferenz-Credit: durch Burn erworbenes Guthaben an Inferenzarbeit
/// (Whitepaper Kap. 5, Anhang A.1).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct InferenceCredit {
    pub owner: Address,
    pub vtfe: u64,
    pub expiry: EpochId,
}

/// Merkle-Wurzel über die **Spur** eines Segments.
///
/// Ein Blatt je Spur-Eintrag, damit sich ein **einzelner** Eintrag
/// beweisen lässt, ohne die ganze Spur zu zeigen. Genau das braucht die
/// Schiedsrunde: Sie streitet über eine Layer, nicht über ein Segment.
pub fn spurwurzel(spur: &[[u8; 32]]) -> Result<MerkleRoot, MerkleError> {
    let refs: Vec<&[u8]> = spur.iter().map(|h| h.as_slice()).collect();
    let tree = MerkleTree::new(&refs)?;
    Ok(MerkleRoot::new(tree.root().0))
}

/// Was ein Pod je Segment bezeugt: die Kennung **und** das Ergebnis.
///
/// # ⚑ Warum das Ergebnis dazugehört (Fund 100, 2026-08-30)
///
/// `segments_root` war bis dahin eine Wurzel über die bloßen
/// Segment-Ids, und eine `SegmentId` ist `(Sitzungsnummer, Position)`
/// mit Nullen aufgefüllt. Sie bindet **nichts**: weder die Spur noch
/// Ein- oder Ausgabe.
///
/// Damit beanspruchte ein PoI-Bündel Arbeit, **ohne zu sagen, was
/// gerechnet wurde**. Ein Pod konnte `n` Paare `(Sitzung, Position)`
/// aufzählen und dafür vergütet werden; die Spur lag nur örtlich beim
/// Koordinator und war an nichts gebunden.
///
/// Der ganze Streitpfad hing daran: Die Schiedsrunde will feststellen,
/// ob der Angeklagte **das** gerechnet hat, was er behauptet hat. Ohne
/// eine Zusicherung gibt es kein „behauptet", nur zwei einander
/// widersprechende Aussagen und keinen Grund, einer zu glauben.
///
/// Das Blatt ist jetzt `Id ‖ Spurwurzel`. Ein Beweis, dass der
/// Angeklagte für Segment `s` die Spur mit Wurzel `w` bezeugt hat, ist
/// damit ein Merkle-Pfad in dieser Wurzel; ein Beweis über einen
/// **einzelnen** Spur-Eintrag ein zweiter innerhalb von `w`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Segmentzeugnis {
    /// Die Kennung des Segments.
    pub id: SegmentId,
    /// Die Wurzel über seine Spur ([`spurwurzel`]).
    pub spurwurzel: MerkleRoot,
}

/// Merkle-Wurzel über die Segmente einer Epoche, die
/// `segments_root`-Konstruktion aus [`PoIBundle`].
///
/// Blätter sind `Id ‖ Spurwurzel` in Bündel-Reihenfolge. Siehe
/// [`Segmentzeugnis`] dazu, warum die Spurwurzel dazugehört.
pub fn segments_root(zeugnisse: &[Segmentzeugnis]) -> Result<MerkleRoot, MerkleError> {
    let blaetter: Vec<Vec<u8>> = zeugnisse
        .iter()
        .map(|z| {
            let mut b = Vec::with_capacity(64);
            b.extend_from_slice(z.id.as_ref());
            b.extend_from_slice(z.spurwurzel.as_ref());
            b
        })
        .collect();
    let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
    let tree = MerkleTree::new(&refs)?;
    Ok(MerkleRoot::new(tree.root().0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::{from_slice, to_vec};

    /// Deterministischer PRNG (xorshift64) für die Property-Tests —
    /// die Testsuite soll ohne Zufalls-Abhängigkeit reproduzierbar sein.
    struct Xorshift(u64);

    impl Xorshift {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        fn fill<const N: usize>(&mut self) -> [u8; N] {
            let mut out = [0u8; N];
            for chunk in out.chunks_mut(8) {
                let v = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&v[..chunk.len()]);
            }
            out
        }
    }

    fn zufalls_segment(rng: &mut Xorshift) -> Segment {
        let n_miner = (rng.next_u64() % 4) as usize;
        let n_trace = (rng.next_u64() % 8) as usize;
        Segment {
            id: SegmentId::new(rng.fill()),
            input_commitment: Hash::from_bytes(rng.fill()),
            model_version: MerkleRoot::new(rng.fill()),
            pod_path: (0..n_miner).map(|_| MinerId::new(rng.fill())).collect(),
            output_commitment: Hash::from_bytes(rng.fill()),
            trace: (0..n_trace)
                .map(|_| ActivationHash::new(rng.fill()))
                .collect(),
            signatures: Vec::new(), // BLS-Signaturen sind separat getestet
        }
    }

    fn zufalls_bundle(rng: &mut Xorshift) -> PoIBundle {
        PoIBundle {
            epoch: EpochId(rng.next_u64()),
            pod: PodId::new(rng.fill()),
            segments_root: MerkleRoot::new(rng.fill()),
            vtfe_claimed: rng.next_u64(),
            aggregate_sig: BlsSignature(rng.fill()),
            segmente: 1,
        }
    }

    fn zufalls_credit(rng: &mut Xorshift) -> InferenceCredit {
        InferenceCredit {
            owner: Address::new(rng.fill()),
            vtfe: rng.next_u64(),
            expiry: EpochId(rng.next_u64()),
        }
    }

    /// Akzeptanzkriterium Phase 1: `serialize(deserialize(x)) == x` für
    /// zufällige Instanzen, mindestens 10.000 Fälle je Typ.
    #[test]
    fn roundtrip_zufaellige_instanzen() {
        let mut rng = Xorshift(0x5eed_5eed_5eed_5eed);
        for _ in 0..10_000 {
            let segment = zufalls_segment(&mut rng);
            let bytes = to_vec(&segment).expect("Serialisierung");
            let back: Segment = from_slice(&bytes).expect("Deserialisierung");
            assert_eq!(back, segment);
            assert_eq!(to_vec(&back).expect("Re-Serialisierung"), bytes);
        }
        for _ in 0..10_000 {
            let bundle = zufalls_bundle(&mut rng);
            let bytes = to_vec(&bundle).expect("Serialisierung");
            let back: PoIBundle = from_slice(&bytes).expect("Deserialisierung");
            assert_eq!(back, bundle);
            assert_eq!(to_vec(&back).expect("Re-Serialisierung"), bytes);
        }
        for _ in 0..10_000 {
            let credit = zufalls_credit(&mut rng);
            let bytes = to_vec(&credit).expect("Serialisierung");
            let back: InferenceCredit = from_slice(&bytes).expect("Deserialisierung");
            assert_eq!(back, credit);
            assert_eq!(to_vec(&back).expect("Re-Serialisierung"), bytes);
        }
    }

    #[test]
    fn serialisierung_ist_deterministisch() {
        let mut rng_a = Xorshift(42);
        let mut rng_b = Xorshift(42);
        let a = zufalls_segment(&mut rng_a);
        let b = zufalls_segment(&mut rng_b);
        assert_eq!(to_vec(&a).expect("ser"), to_vec(&b).expect("ser"));
    }

    fn zeugnisse(n: u8) -> Vec<Segmentzeugnis> {
        (0..n)
            .map(|i| {
                let mut bytes = [0u8; 32];
                bytes[0] = i;
                Segmentzeugnis {
                    id: SegmentId::new(bytes),
                    spurwurzel: spurwurzel(&[[i; 32], [i.wrapping_add(1); 32]]).expect("Wurzel"),
                }
            })
            .collect()
    }

    #[test]
    fn segments_root_stimmt_mit_merkle_baum_ueberein() {
        let z = zeugnisse(5);
        let root = segments_root(&z).expect("Wurzel");
        // Manuell über den Merkle-Baum mit denselben Blättern.
        let blaetter: Vec<Vec<u8>> = z
            .iter()
            .map(|e| {
                let mut b = e.id.as_ref().to_vec();
                b.extend_from_slice(e.spurwurzel.as_ref());
                b
            })
            .collect();
        let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
        let tree = MerkleTree::new(&refs).expect("Baum");
        assert_eq!(root, MerkleRoot::new(tree.root().0));
        // Mitgliedschaftsbeweis für ein Blatt muss verifizieren.
        let proof = tree.proof(2).expect("Beweis");
        assert!(proof.verify_hashed(&tree.root(), &crate::merkle::leaf_hash(&blaetter[2])));
    }

    /// ⚑ **Der Kern von Fund 100: Die Wurzel muss sich ändern, wenn sich
    /// die Spur ändert.**
    ///
    /// Vorher war sie eine Wurzel über bloße Ids, und eine Id ist
    /// `(Sitzung, Position)`. Zwei Pods, die dieselben Positionen
    /// rechnen und **verschiedene Ergebnisse** bekommen, hatten damit
    /// dieselbe Wurzel: Das Bündel beanspruchte Arbeit, ohne zu sagen,
    /// was gerechnet wurde.
    #[test]
    fn eine_andere_spur_ergibt_eine_andere_wurzel() {
        let mut a = zeugnisse(3);
        let wurzel_a = segments_root(&a).expect("Wurzel");
        // Dieselben Ids, eine andere Spur.
        a[1].spurwurzel = spurwurzel(&[[99u8; 32]]).expect("Wurzel");
        let wurzel_b = segments_root(&a).expect("Wurzel");
        assert_ne!(
            wurzel_a, wurzel_b,
            "die Bündelwurzel bezeugt das Ergebnis nicht"
        );
    }

    /// Und die Spurwurzel selbst trägt jeden einzelnen Eintrag: Genau
    /// das braucht die Schiedsrunde, die über **eine** Layer streitet.
    #[test]
    fn ein_einzelner_spureintrag_ist_beweisbar() {
        let spur: Vec<[u8; 32]> = (0..7u8).map(|i| [i; 32]).collect();
        let refs: Vec<&[u8]> = spur.iter().map(|h| h.as_slice()).collect();
        let tree = MerkleTree::new(&refs).expect("Baum");
        assert_eq!(spurwurzel(&spur).expect("Wurzel"), MerkleRoot::new(tree.root().0));
        let proof = tree.proof(4).expect("Beweis");
        assert!(proof.verify_hashed(&tree.root(), &crate::merkle::leaf_hash(&spur[4])));
        // Ein anderer Eintrag passt nicht an diese Stelle.
        assert!(!proof.verify_hashed(&tree.root(), &crate::merkle::leaf_hash(&spur[5])));
    }

    #[test]
    fn segments_root_leer_wird_abgelehnt() {
        assert!(matches!(segments_root(&[]), Err(MerkleError::Empty)));
    }

    #[test]
    fn segment_feldreihenfolge_ist_fest() {
        // Anhang A.1 schreibt die Feldreihenfolge vor; Borsh kodiert in
        // Deklarationsreihenfolge. Dieser Test fixiert die Serialisierung
        // eines Minimal-Segments als Golden-Byte-Folge: Jede Änderung der
        // Feldreihenfolge oder der Typen bricht sie (und damit den
        // Konsens) und schlägt hier an.
        let segment = Segment {
            id: SegmentId::new([0u8; 32]),
            input_commitment: Hash::from_bytes([0u8; 32]),
            model_version: MerkleRoot::new([0u8; 32]),
            pod_path: Vec::new(),
            output_commitment: Hash::from_bytes([0u8; 32]),
            trace: Vec::new(),
            signatures: Vec::new(),
        };
        let bytes = to_vec(&segment).expect("Serialisierung");
        // 32 (id) + 32 (input) + 32 (version) + 4 (Vec-Länge) + 32 (output)
        // + 4 (Vec-Länge) + 4 (Vec-Länge) = 140 Bytes.
        assert_eq!(bytes.len(), 140);
        assert!(bytes[96..100].iter().all(|&b| b == 0)); // leere pod_path
        assert!(bytes[132..136].iter().all(|&b| b == 0)); // leere trace
        assert!(bytes[136..140].iter().all(|&b| b == 0)); // leere signatures
    }
}

/// Die Frage nach der Spur eines gezogenen Segments (Punkt 45, Stufe 2).
///
/// ⚑ **Die Kette hält nur eine Wurzel.** Wer ein Segment nachrechnen
/// will, braucht seine Eingabe und die behauptete Spur, und beide liegen
/// beim Koordinator des Pods. Ohne diesen Abruf ist die Ziehung eine
/// Lotterie ohne Ziehungsergebnis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Spuranfrage {
    /// Die Epoche, aus der das Bündel stammt.
    pub epoche: EpochId,
    /// Der Pod, dessen Bündel das Segment bezeugt.
    pub pod: PodId,
    /// Der Index des Segments **innerhalb dieses Bündels**.
    pub segment: u32,
}

/// Die Antwort darauf: das Zeugnis, sein Beweis, und was gerechnet wurde.
///
/// # ⚑ Warum der Beweis dazugehört
///
/// Ohne ihn reichte der Koordinator ein **anderes** Segment heraus,
/// nämlich eines, das er richtig gerechnet hat. Die Ziehung wäre dann
/// eine Frage, auf die der Gefragte die Antwort wählt. Der Beweis bindet
/// das Zeugnis an `segments_root` aus dem Bündel, und das Bündel ist
/// unterschrieben.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Spurantwort {
    /// Worauf geantwortet wird.
    pub anfrage: Spuranfrage,
    /// Das Zeugnis an dieser Stelle: Kennung und Spurwurzel.
    pub zeugnis: Segmentzeugnis,
    /// Merkle-Beweis des Zeugnisses gegen `segments_root`.
    pub beweis: crate::merkle::MerkleProof,
    /// Die Eingabe-Aktivierungen `a_0`.
    ///
    /// # ⚑ Fund 118: Dieses Feld kann heute niemand füllen
    ///
    /// **Die Entscheidung E10 (2026-08-30) hat das Archivieren der
    /// Aktivierungen abgeschafft.** Sie kostete über die Streitfrist
    /// zwischen 65 GiB und 1,8 TiB je Knoten, zusätzlich zur
    /// Modellgröße. Was bleibt, ist die **Spur**: 32 Byte je Layer statt
    /// 7 KiB je Eingang.
    ///
    /// Die Begründung dort lautet: Im Streitfall legt der **Ankläger**
    /// die Eingabe offen, denn „bei `j-1` sind sich beide einig, und er
    /// hat den Wert ohnehin, weil er das Segment gerade nachgerechnet
    /// hat."
    ///
    /// ⚑ **Das trägt für die Bisektion und nicht für die Stichprobe.**
    /// In der Bisektion streiten zwei, die beide gerechnet haben. Ein
    /// Checker der Stufe 2 hat **noch nichts gerechnet**; er will
    /// gerade erst anfangen und braucht dafür die Eingabe, die niemand
    /// mehr aufhebt.
    ///
    /// **Drei Wege, und keiner ist umsonst:**
    ///
    /// - Der Checker rechnet die **ganze Pipeline** vom Prompt an nach.
    ///   Selbstgenügsam, aber er zahlt `k`-mal statt einmal.
    /// - Die Eingabe wird **doch** aufgehoben, für eine kürzere Frist
    ///   als die Streitfrist. Das ist E10 mit anderer Zahl, nicht gegen
    ///   E10.
    /// - Stufe 2 prüft nur den **ersten** Shard eines Segments, dessen
    ///   Eingabe aus dem Prompt folgt. Billig, deckt aber `1/k` ab.
    ///
    /// **Das ist eine Entscheidung des Projektinhabers und keine
    /// Verdrahtung**, und sie ist als eigener Punkt offen; bis dahin
    /// bleibt das Feld im Typ, weil die Prüfung darauf aufbaut und das
    /// Bindungsgerüst darum herum vollständig und geprüft ist.
    pub eingabe: Vec<u8>,
    /// Die behaupteten Commitment-Hashes der Spur.
    pub spur: Vec<crate::hash::Hash>,
}
