# consensus (`myl-consensus` + `myl-ledger` + `myl-scheduler`)

> **Version:** 0.3.5 (`myl-scheduler` 0.2.7)
> **Datum:** 2026-08-18
> **Status:** Design-Entscheidungen getroffen (malachite hinter
> trait-Grenze mit Eigenbau-Fallback, Blockzeit 2 s, Komitee 21/7,
> Streitfrist 7 Tage, Reed-Solomon k=8/m=4 — Details im Fahrplan);
> 🎉 **Phase 1 + 2 + 3 vollständig** (`myl-ledger` v0.1.1–v0.1.5,
> `myl-scheduler` v0.2.1–v0.2.6, `myl-consensus` v0.3.1–v0.3.5,
> Akzeptanzkriterien erfüllt: bitgleicher Replay + deterministischer
> Epochen-Scheduler + BFT-Blockproduktion).

BFT-Blockproduktion, Proof-of-Inference-Aggregation, Staking/Slashing,
Ledger-Zustandsübergänge, deterministischer Epochen-Scheduler.
Referenzimplementierung von Whitepaper Kap. 3.5 und Anhang A.2/A.5.

## Aufgabe

Schicht L1: der Konsens, der unabhängig von der Inferenz-Latenz läuft
(Kap. 3.2). Zwei entkoppelte Prozesse (Kap. 3.5.2): schnelle
BFT-Blockproduktion (Prozess A) und epochenweise PoI-Abrechnung (Prozess B).
Dazu der deterministische Epochen-Scheduler (Anhang A.2), der ohne zentrale
Instanz aus dem Blockhash Miner-Zuteilung, Pod-Bildung und
Stichprobenlotterie ableitet.

## Abhängigkeiten

SHARED_TYPES, NETWORKING (Block-Gossip). Enthält selbst keine
Inferenz-Verifikation (siehe VERIFICATION) und keine Tokenomik-Berechnung im
Detail (siehe TOKENOMICS) — bildet aber deren gemeinsame Grundlage
(Ledger-Zustandsübergänge, Staking/Slashing-Buchhaltung).

## Struktur

```
CONSENSUS/
├── README/                   diese Kurzübersicht + Fahrplan
└── myl-ledger/               L1-Ledger (Anhang A.5)
    ├── src/
    │   ├── lib.rs             Konsens-Grundregeln (reine Funktionen,
    │   │                      BTreeMap-Ordnung, Ganzzahligkeit,
    │   │                      Überlaufsicherheit)
    │   ├── state.rs           Kontenmodell (Balance/Stake/Credits),
    │   │                      State-Commitment (SHA-256 über Borsh)
    │   └── transitions.rs     burn→mint_credits, apply_verdict,
    │                          credit_spend (atomare Übergänge)
    └── tests/
        └── determinism.rs     Replay-Akzeptanztest: gleiche Folge ⇒
                               bitgleiches Commitment (zwei unabhängige
                               Läufe, inkl. 1.000-Übergangs-Folge)
```

## Changelog

### myl-scheduler v0.2.7 – 2026-08-18 (Fix: Testbuild wiederhergestellt)
- **Fund A1:** `myl-scheduler` ließ sich seit dem Roundhouse-Check-Commit
  nicht mehr im Testmodus bauen (`error[E0433]: cannot find type MinerId`).
  Beim Beheben einer `unused import`-Warnung wurde `use myl_types::ids::MinerId`
  aus `shard_assignment.rs` entfernt — im Lib-Rumpf tatsächlich unbenutzt,
  im `#[cfg(test)] mod tests` aber gebraucht. `cargo build` blieb grün,
  `cargo test` brach ab.
- Fix: Import in den Test-Modul verschoben. 56 Tests grün (die in der
  Doku bereits behauptete Zahl).
- **Ursachenanalyse:** Der Fehler konnte unentdeckt nach `main` gelangen,
  weil die CI `myl-scheduler` überhaupt nicht baute. Siehe CI-Ausweitung
  im selben Patch (`.github/workflows/ci.yml`): jetzt laufen alle acht
  `myl-*`-Crates plus `INTEGER_LLM/pipeline`.

### v0.3.5 – 2026-08-17 (Phase 3: BFT-Blockproduktion)
- `myl-consensus`: Neuer Crate mit 5 Modulen für BFT-Blockproduktion:
  - Validator-Registrierung mit Stake-Minimum und Komiteewahl (12 Tests)
  - BFT-Protokoll mit Propose/Vote/Commit-Zyklus (9 Tests)
  - Block-Struktur mit Borsh-Serialisierung (9 Tests)
  - Stimmgewichts-Kopplung mit Decay-Faktor (13 Tests)
  - Double-Signing-Erkennung und Slashing (10 Tests)
- 53 neue Tests grün, insgesamt 109 Tests

### v0.2.6 – 2026-08-17 (Phase 2: Deterministischer Epochen-Scheduler)
- `myl-scheduler`: Neuer Crate mit 6 Modulen für den deterministischen
  Epochen-Scheduler (Whitepaper Anhang A.2):
  - VRF-Seed-Ableitung aus finalisiertem Block (7 Tests)
  - Miner-Filterung nach Hardware-Klasse und Registrierungsschluss (11 Tests)
  - Geo-Clustering unter Latenz-Constraint (8 Tests)
  - Shard-Zuweisung mit Fisher-Yates (9 Tests)
  - Redundanz-Zuteilung (zonendivers, disjunkt) (9 Tests)
  - Stichproben-Lotterie für Checker (12 Tests)
- Alle Schritte sind deterministisch und von jedem Node unabhängig
  nachrechenbar. 56 Tests grün insgesamt.

### v0.1.1–v0.1.5 – 2026-08-13 (Phase 1)
- `myl-ledger`: Kontenmodell mit deterministischer BTreeMap-Ordnung,
  Zustandsübergänge nach Anhang A.5 (burn→mint_credits mit
  floor-Division, apply_verdict mit Slash/Kopfgeld als Ganzzahl-Brüche,
  credit_spend mit FIFO-Verbrauch nach Verfall), atomare Übergänge
  (Prüfphase vor Änderungsphase), State-Commitment via SHA-256 über
  kanonischem Borsh.
- Akzeptanzkriterium erfüllt: Replay derselben Übergangsfolge liefert
  auf zwei unabhängigen Läufen bitgleiche Commitments (23 Tests grün,
  keine Warnungen).
- Verdict-Minimaltyp als dokumentierte Zwischenlösung bis zur
  VERIFICATION-Definition; vTFE-Rückbuchung als Phase-4-Hook im
  `VerdictEffect`.
