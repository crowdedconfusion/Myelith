# shared-types (`myl-types`)

> **Version:** 0.5.0
> **Datum:** 2026-08-26
> **Status:** 🎉 **Phase 2 abgeschlossen** (Punkte 1.1–1.6, 2.1–2.3):
> Hash, Merkle-Baum, VRF (bit-exakt gegen RFC-9381-Vektoren), BLS12-381
> mit Aggregation **und Proof-of-Possession**, **Erasure-Codierung über
> GF(2⁸)**, ID-Newtypes, Kern-Structs
> aus Anhang A.1, Golden Vectors (18 Vektoren), Fuzz-Harness
> (100.000 Iterationen), Konformitätspaket.
> **133 Tests grün.**

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
├── README/                   diese Kurzübersicht
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

### v0.5.0 – 2026-08-28 (⚑ Fund 74: `Hash` bekommt eine Ordnung)

`Hash` leitete `Clone, Copy, Eq` ab und hatte einen
Konstantzeit-Vergleich, aber **kein `Ord`**. Damit ließ sich eine
`BTreeSet<Hash>` anlegen und niemals füllen: Ein leeres `BTreeSet`
braucht keine Ordnung, ein `insert` schon.

**Aufgefallen ist es in GOVERNANCE**, drei Wochen nachdem der Typ
gebraucht wurde. Die Kernel-Whitelist aus Kap. 10.3 steht dort seit
Punkt 1.1 als `Wert::Hashmenge(BTreeSet<Hash>)`, mit dem Vorgabewert
„leere Menge, bis zum Genesis-Manifest". Der Parameter hatte Typ,
Vorgabewert und Dokumentation und war nicht befüllbar. **Der Kommentar
nannte sogar den Schritt, an dem es brechen würde**, und niemand hat
nachgesehen, ob es dann geht.

Die ID-Typen aus `ids.rs` leiten `Ord` seit jeher ab; dass ausgerechnet
`Hash` es nicht tat, war kein Entwurf, sondern eine Lücke.

**Warum die Ordnung nicht in Konstantzeit läuft, und warum das richtig
ist:** `PartialEq` vergleicht bewusst in Konstantzeit. Eine Ordnung kann
das nicht, denn sie bricht beim ersten unterschiedlichen Byte ab, und
genau daraus besteht ein Größenvergleich. Dieselbe Abwägung, die in
derselben Datei schon für `std::hash::Hash` getroffen ist: Sortieren
und Nachschlagen sind keine Geheimnisoperationen. Wer wissen will, ob
zwei Hashes gleich sind, nimmt `==`.

**Was zusammenpassen muss:** `cmp` gibt genau dann `Equal` zurück, wenn
`eq` wahr ist. Liefen die beiden auseinander, verhielte sich jede
`BTreeMap` mit Hash-Schlüssel undefiniert und fände Einträge nicht, die
sie enthält. Ein Test hält es fest, ein weiterer die Stabilität der
Reihenfolge über Läufe: Eine Menge, deren Reihenfolge wechselt, ergibt
verschiedene Wurzeln für denselben Inhalt.

Vier neue Tests, eine Gegenprobe (eine Ordnung, die immer `Equal`
meldet, macht alle drei rot).

### v0.4.0 – 2026-08-19 (Erasure-Codierung als Primitive)

Neues Modul `erasure.rs` für die Datenverfügbarkeits-Schicht
(CONSENSUS 4.3): Reed-Solomon-artige Codierung in **systematischer
Cauchy-Form** über GF(2⁸), Startparameter k=8/m=4.

**Warum hier und nicht in CONSENSUS:** Erasure-Codierung ist eine
Primitive wie Hash, Merkle, VRF und BLS. Eine zweite Kopie in einer
Komponente wäre genau der Fehler aus Fund A6 (der Fisher-Yates-Shuffle
lag in vier Fassungen vor, drei davon fehlerhaft).

**Cauchy statt Vandermonde — der Grund ist eine Falle.** Bei einer
Vandermonde-Matrix ist die Invertierbarkeit **jeder** k×k-Teilmatrix
nicht automatisch gegeben. Das Loch äußert sich nicht als Fehler,
sondern als Rekonstruktion, die für bestimmte Ausfallmuster
stillschweigend falsche Daten liefert — die schlechteste Art von Bug.
Bei `C[i][j] = 1/(x_i ⊕ y_j)` mit disjunkten Mengen `{x_i}`, `{y_j}` ist
jede quadratische Teilmatrix invertierbar.

**Geprüft, nicht angenommen:** `jede_k_aus_n_teilmenge_rekonstruiert`
fährt alle **495** Teilmengen von 8 aus 12 durch; eine zweite
Parametrierung (3 aus 5, alle 10 Teilmengen) prüft, dass die
Konstruktion nicht nur für die Standardwerte trägt.

**Beschädigte Eingaben ergeben Fehler, keine Rekonstruktion.** Doppelte
Indizes machten die Matrix singulär, uneinheitliche Längen lieferten
Müll — beides wird abgewiesen, statt still falsche Daten zu erzeugen.
Zu wenige Fragmente sind ein **definierter** Ausfall
(`NotEnoughFragments`), kein Bug.

**Ganzzahligkeit:** GF(2⁸) ist reine Bitarithmetik — kein Gleitkomma,
keine Ordnungsabhängigkeit, bitgleich auf jeder Hardware. Dieselbe
Eigenschaft, auf der die Inferenz beruht, hier für die
Datenverfügbarkeit.

17 Tests; Crate 95 → 112 Unit-Tests.

### v0.3.0 – 2026-08-19 (Fund 27: Rogue-Key-Schutz nachgerüstet)

**Eine Sicherheitszusage in diesem Crate war falsch.** Der Modulkopf von
`bls.rs` sagte zu: „Öffentliche Schlüssel werden vor jeder
Aggregat-Verifikation validiert (Identitäts- und Subgruppen-Prüfung) —
**schützt gegen Rogue-Key-Angriffe bei `FastAggregateVerify`**." Das
stimmt nicht. Die beiden Prüfungen wehren Kleine-Untergruppen-Angriffe
ab, nicht Rogue Keys.

**Nicht bezweifelt, sondern gebrochen.** Zu einem fremden `pk_opfer`
bildet der Angreifer mit eigenem Geheimnis `x` den Schlüssel
`pk_rogue = g₁^x · pk_opfer⁻¹`. Der Punkt liegt in der richtigen
Untergruppe, ist nicht die Identität und besteht damit `key_validate()`.
Weil `pk_opfer · pk_rogue = g₁^x` gilt, verifiziert eine Signatur, die
der Angreifer **allein** erzeugt hat, als Aggregat beider Schlüssel — das
Opfer hat nie unterschrieben.

**Nachgerüstet:** `BLS_POP_DST`, `BlsProofOfPossession`,
`BlsSecretKey::prove_possession()` und `BlsPublicKey::verify_possession()`
nach draft-irtf-cfrg-bls-signature §3.3. Der Nachweis signiert die
komprimierten Bytes des eigenen öffentlichen Schlüssels unter einem
**eigenen** Domain-Tag — ohne diese Trennung wäre eine gewöhnliche
Signatur über die eigenen Schlüsselbytes ein gültiger Nachweis und
umgekehrt. Wer einen Nachweis liefern kann, kennt den diskreten
Logarithmus seines Schlüssels; der Erzeuger eines Rogue Keys kann das
nicht (er wäre `x − sk_opfer`).

**Regression:** `tests/rogue_key.rs` hält beide Tatsachen als
ausführbaren Nachweis fest — dass der Rogue Key die Validierung besteht
und `FastAggregateVerify` täuscht, **und** dass der Besitznachweis ihn
ausschließt. Als Integrationstest, weil die Konstruktion
`blst`-Punktarithmetik und damit `unsafe` braucht, was dieses Crate per
`#![deny(unsafe_code)]` ausschließt.

**Aufrufer:** `ValidatorRegistry::register` und `PodMembership::new` in
`myl-consensus` verlangen den Nachweis jetzt (dort v0.7.0). Die
Prüfungen `validate()`/`decode_validated_pk` bleiben unverändert — sie
waren nie falsch, nur falsch beschrieben.

**Konsensrelevant** (Kap. 10.3): neues Domain-Tag, geänderte
Registrierungsbedingung. 89 → 95 Unit-Tests, dazu 5 Regressionstests.


### Audit-Block 5 – 2026-08-18 (Warnungsfreiheit, Tests, Float-Audit)

Repository-weiter Block; die Einzelheiten stehen im Changelog der
jeweiligen Komponente.

- **Fund A17 behoben:** 111 Compiler-Warnungen → **0** über alle elf
  Crates. Dabei kamen drei echte Lücken zum Vorschein, die sich hinter
  „harmlosen" Warnungen versteckten (siehe unten).
- **clippy sauber** über alle Crates; `RUSTFLAGS: -D warnings` und ein
  eigener `lint`-Job in der CI verankern den Zustand. Bewusste Ausnahmen
  stehen als `#![allow(...)]` **mit Begründung** im Modulkopf (die
  Kernel-Signaturen tragen den vollständigen Fixed-Point-Vertrag; die
  Matrix-Namen `W`, `W_gate` folgen Whitepaper-Anhang B).
- **Fund A18 behoben:** Das Gleitkomma-Audit prüfte nur INTEGER_LLM
  (20 Dateien). Es deckt jetzt auch den **Konsenspfad** ab (37 weitere
  Dateien aus myl-types, -ledger, -scheduler, -consensus, -tokenomics,
  -verifier). Beide Pfade: null Treffer.


### v0.2.5 – 2026-08-18 (Audit-Block 4: Challenge als Protokolltyp)
- Neues Modul `challenge.rs` mit `Challenge` (Anhang A.4) und
  `validate_structure()`.
- **Warum hier (Fund A8/A12):** Der Typ wird von drei Komponenten
  gebraucht, die einander nicht kennen dürfen — VERIFICATION erzeugt
  ihn, NETWORKING validiert ihn beim Gossip, CONSENSUS nimmt ihn in den
  Block auf. Läge er in einer davon, müsste die Schichtung verletzt
  werden (L0 Networking hinge an L1 Consensus). Vorher existierten
  **zwei** unabhängige `Challenge`-Definitionen mit verschiedenen
  Feldern; der Block konnte gar nicht aufnehmen, was der Verifier
  produziert.
- 90 → 94 Tests.

### v0.2.4 – 2026-08-18 (Audit-Block 3: geteilter Seed-RNG)
- Neues Modul `seed_rng.rs`: `SeedRng` (SHA-256 im Zählermodus),
  `deterministic_shuffle` und `weighted_sample_without_replacement`.
- **Warum hier:** Beide Verwendungen sind Konsens-Feld — der
  Epochen-Scheduler (Shard-Zuweisung, Redundanz, Stichprobenlotterie,
  Geo-Clustering) und die Komiteewahl im Konsens. Vorher lag der
  Shuffle in vier Kopien in `myl-scheduler`; mit `myl-consensus` wäre
  eine fünfte dazugekommen. Protokollweite Primitive gehören in
  `myl-types`, damit es genau eine Fassung gibt.
- `weighted_sample_without_replacement` ist die Grundlage der
  VRF-rotierenden, stimmgewichteten Komiteewahl (Whitepaper Kap. 3.5:
  „gewählt nach Stake, rotierend per VRF").
- 74 → 90 Tests.

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
