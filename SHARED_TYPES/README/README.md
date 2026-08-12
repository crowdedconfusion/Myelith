# shared-types (`myl-types`)

> **Version:** 0.1.2
> **Datum:** 2026-08-12
> **Status:** Design-Entscheidungen getroffen (Rust, SHA-256, ECVRF mit
> dokumentiertem PQ-Migrationspfad, BLS12-381, Borsh — Details und
> Quantum-Einordnung im Fahrplan), Phase 1 in Umsetzung; Punkte 1.1–1.2
> abgeschlossen (`myl-types` v0.1.2: Hash-Newtype + Merkle-Baum)

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
        └── merkle.rs          Merkle-Baum: Aufbau, Beweis-Erzeugung/-Prüfung,
                               Domain-Separation, Borsh-Beweise
```

## Changelog

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
