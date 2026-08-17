# shared-types (`myl-types`)

> **Version:** 0.2.3
> **Datum:** 2026-08-17
> **Status:** 🎉 **Phase 1 + 2 vollständig** (Punkte 1.1–1.6, 2.1–2.3):
> Hash, Merkle-Baum, VRF (bit-exakt gegen RFC-9381-Vektoren), BLS12-381
> mit Aggregation, ID-Newtypes, Kern-Structs aus Anhang A.1, Golden
> Vectors (18 Vektoren), Fuzz-Harness (100.000 Iterationen), Konformitätspaket.

Protokollweite Kern-Datentypen, Hash-/Merkle-Primitiven und Serialisierung
für Myelith. Referenzimplementierung von Whitepaper Anhang A.1.

## Aufgabe

Ein einziges Crate, von dem alle anderen Komponenten (NETWORKING, CONSENSUS,
VERIFICATION, TOKENOMICS, COMPUTE_PIPELINE, AGENT_LAYER, TRAINING) dieselben
Basistypen beziehen, damit `Segment`, `PoIBundle`, Hashes und Signaturen
niemals in zwei Komponenten inkompatibel definiert werden.

## Abhängigkeiten

Keine — SHARED_TYPES ist die Basiskomponente des Protokolls.

## Struktur

```
SHARED_TYPES/
├── README/                   diese Kurzübersicht + Fahrplan
└── myl-types/                das Protokoll-Crate (Bibliothek, kein Binary)
    └── src/
        ├── lib.rs             Crate-Wurzel: #![deny(unsafe_code)], Design-Doku
        ├── protocol.rs        Protokoll-Konstanten (Hash/VRF/Signatur/Serialisierung)
        ├── hash.rs            Hash-Newtype: SHA-256, Konstantzeit-Vergleich,
        │                      Borsh, Hex-Darstellung
        ├── merkle.rs          Merkle-Baum: Aufbau, Beweis-Erzeugung/-Prüfung,
        │                      Domain-Separation, Borsh-Beweise
        ├── vrf.rs             VRF: ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381),
        │                      Kanonizitätsprüfung, RFC-Testvektoren
        ├── bls.rs             BLS12-381 (min-pk, blst): KeyGen, Signatur,
        │                      Aggregation, FastAggregateVerify/AggregateVerify
        ├── ids.rs             ID-Newtypes: Address, MinerId, PodId, SegmentId,
        │                      MerkleRoot, ActivationHash, EpochId
        └── core_types.rs      Kern-Structs aus Anhang A.1: Segment, PoIBundle,
                               InferenceCredit (+ segments_root-Helfer)
```

## Changelog

### v0.1.7 – 2026-08-13
- ID-Newtypes leiten zusätzlich `PartialOrd`/`Ord` ab — benötigt für
  `BTreeMap`-Schlüssel (u. a. das Kontenregister in `myl-ledger`,
  dessen deterministische Ordnung Konsens-Eigenschaft ist). Rein
  additive Änderung, keine Serialisierungs-Änderung.

### v0.1.6 – 2026-08-13 (Punkt 1.5) — Phase 1 vollständig
- Kern-Structs aus Anhang A.1, Feldnamen und -reihenfolge exakt wie im
  Whitepaper (Borsh-Reihenfolge ist Konsens-Vertrag): `Segment`
  (id, input_commitment, model_version, pod_path, output_commitment,
  trace, signatures), `PoIBundle` (epoch, pod, segments_root,
  vtfe_claimed, aggregate_sig), `InferenceCredit` (owner, vtfe, expiry).
- `segments_root`-Helfer: Merkle-Wurzel über Segment-Ids (die
  `PoIBundle.segments_root`-Konstruktion).
- Akzeptanzkriterium erfüllt: `serialize(deserialize(x)) == x` für je
  10.000 pseudozufällige Instanzen (deterministischer Xorshift-PRNG,
  reproduzierbar) plus Golden-Byte-Test der Feldreihenfolge —
  54 Tests grün, keine Warnungen.

### v0.1.5 – 2026-08-13 (Punkt 1.6, vor 1.5 umgesetzt)
- ID-Newtypes: `Address`, `MinerId`, `PodId`, `SegmentId`, `MerkleRoot`,
  `ActivationHash` (alle 32 Bytes, Borsh, kanonische Hex-Darstellung)
  und `EpochId` (u64). Typ-Verwechslung ist ein Compile-Fehler.
- Adress-Konvention: `Address = SHA-256(komprimierter BLS-Public-Key)`
  (hash-basiert, quantensicher, unabhängig vom Signaturschema).

### v0.1.4 – 2026-08-13 (Punkt 1.4)
- BLS-Signaturschnittstelle: BLS12-381 in der min-pk-Variante
  (Public Key G1/48 B, Signatur G2/96 B, Ethereum-DST
  `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_`) über das `blst`-Crate
  (Supranational-Referenzimplementierung).
- KeyGen nach BLS-Draft §2.3 (HKDF, IKM ≥ 32 Bytes), deterministisches
  Signieren, `aggregate_signatures`, `fast_aggregate_verify` (der
  PoI-Bündel-Fall: gleiche Nachricht, viele Unterzeichner) und
  `aggregate_verify` (verschiedene Nachrichten).
- Konsens-Sicherheitsfestlegungen: Signatur-Gruppenprüfung bei jeder
  Verifikation, Public-Key-Validierung (Identität + Untergruppe) vor
  jeder Aggregat-Verifikation als Rogue-Key-Schutz.
- Geheimschlüssel-Typ bewusst ohne Debug/PartialEq/öffentliche
  Serialisierung — 44 Tests grün, keine Warnungen.

### v0.2.3 – 2026-08-17 (Phase 2.3: Konformitätspaket)
- `conformance/`-Verzeichnis mit 18 eingefrorenen Golden Vectors
  (4 Hash, 4 Merkle, 5 VRF, 5 BLS) und README für Drittimplementierungen.
- Validierungstest (`tests/validate_conformance.rs`) prüft alle Vektoren
  gegen die Referenz-Implementierung — 4 Tests grün.
- Phase 2 damit vollständig abgeschlossen.

### v0.2.2 – 2026-08-17 (Phase 2.2: Fuzz-Harness)
- Fuzz-Test (`tests/fuzz_deserialization.rs`) für alle Borsh-Deserialisierungspfade:
  100.000 Iterationen pro Typ (Hash, MerkleProof, VRF, BLS, IDs, Core-Types)
  mit zufälligen/adversarialen Eingaben — keine Panics, nur `Ok` oder `Err`.
- Deterministischer PRNG (SplitMix64) für reproduzierbare Tests.

### v0.2.1 – 2026-08-17 (Phase 2.1: Golden Vectors)
- Golden Vector Generator (`src/bin/generate_golden_vectors.rs`) erzeugt
  18 deterministische Testvektoren für Hash, Merkle, VRF und BLS.
- Vektoren dienen als Referenz für Drittimplementierungen in anderen Sprachen.

### v0.1.3 – 2026-08-12 (Punkt 1.3)
- VRF-Schnittstelle: ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381 §5.5) —
  `VrfSecretKey`/`VrfPublicKey`/`VrfProof`/`VrfOutput`, Try-and-Increment-
  Hash-to-Curve mit Cofactor-Bereinigung, deterministische Nonce
  (RFC-8032-Variante), validate_key gegen Kleinordnungs-Schlüssel.
- Gegen die **offiziellen RFC-Testvektoren** (Anhang B.3, Beispiele 16–18)
  geprüft: Beweis-Erzeugung und Verifikation bit-exakt.
- Konsens-Verschärfung: kanonische Punkt-Dekodierung (y < p,
  Vorzeichen-Bit maskiert) — curve25519-dalek allein akzeptiert nicht
  kanonische Kodierungen, die der RFC ablehnt.
- `VrfOutput.algorithm` trägt das Versionsfeld für den dokumentierten
  Post-Quantum-Migrationspfad (GOVERNANCE, Krypto-Agilität) —
  34 Tests grün, keine Warnungen.

### v0.1.2 – 2026-08-12 (Punkt 1.2)
- Merkle-Baum über SHA-256: Aufbau (Duplikationsregel für ungerade
  Ebenen, Ein-Blatt-Sonderfall), Beweis-Erzeugung und -Prüfung
  (`MerkleProof` mit Borsh-Serialisierung, explizite Index-Bindung).
- Konsens-Festlegungen dokumentiert: Domain-Separation
  (`0x00`-Blatt-Präfix, `0x01`-Knoten-Präfix, Second-Preimage-Schutz),
  leerer Baum ist ein Fehler, Ordnung der Blätter ist Teil des Vertrags.
- Akzeptanzkriterium erfüllt: JEDE Einzelbit-Verfälschung eines Blatts
  oder des serialisierten Beweises wird abgelehnt (exhaustive
  Bitflip-Tests) — 21 Tests grün, keine Warnungen.

### v0.1.1 – 2026-08-12 (Punkt 1.1)
- Crate-Grundgerüst `myl-types`: `#![deny(unsafe_code)`, keine
  Gleitkomma-Arithmetik (Konsens-Determinismus ist Verfassungsrang).
- `Hash`-Newtype über SHA-256: Konstantzeit-Gleichheit
  (`subtle::ConstantTimeEq`), Borsh-Serialisierung, Hex-Darstellung,
  NIST-Testvektoren (leere Eingabe, „abc"), Roundtrip-Tests — 9 Tests grün.
- Protokoll-Konstanten als maschinenlesbare Anker der fünf
  Design-Entscheidungen (inkl. VRF-/Signatur-Algorithms-Versionsfelder
  für den dokumentierten Post-Quantum-Migrationspfad).
