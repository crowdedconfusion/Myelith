# verification (`myl-verifier`)

> **Version:** 0.1.1
> **Datum:** 2026-08-17
> **Status:** 🎉 **Phase 1 vollständig** (Punkte 1.1–1.3): Redundanzvergleich
> (Stufe 1) mit Commitment-Hash-Vergleich und zwei Auslieferungsmodi
> (Optimistic/Confirmed). Binärer Vergleich, 21 Tests grün.

Verifikations-Subsystem: Redundanzvergleich, Bisektions-Spiel (Stufe 2),
Kontrollsegmente (Stufe 3). Referenzimplementierung von Whitepaper Kap. 6.4–6.9
und Anhang A.4.

## Aufgabe

Die Komponente, die INTEGER_LLMs Determinismus-Eigenschaft (Kap. 6.2) erst
wirtschaftlich nutzbar macht: Sie entscheidet, wann zwei Berechnungen als
"gleich" gelten, was bei Abweichung passiert, und wie eine Manipulation
wirtschaftlich unattraktiv gemacht wird.

**Drei Verifikationsstufen:**
1. **Redundanz (Stufe 1):** Commitment-Hash-Vergleich zweier Pods
2. **Stichproben (Stufe 2):** Bisektions-Spiel bei Abweichung
3. **zkML-Anker (Stufe 3):** Zukunftspfad (noch nicht implementiert)

## Abhängigkeiten

- **INTEGER_LLM:** Determinismus-Eigenschaft (Kap. 6.2) — ✅ Phase 12.21 akzeptiert
- **CONSENSUS:** BFT-Blockproduktion (Phase 3) für On-Chain-Schiedsrunde — ⏳ offen
- **NETWORKING:** Verschlüsselte Aktivierungs-Streams (Phase 3) für DA-Fragmente — ⏳ offen

## Struktur

```
VERIFICATION/
├── README/                   diese Kurzübersicht + Fahrplan
└── myl-verifier/             das Verifikations-Crate (Bibliothek)
    ├── Cargo.toml
    └── src/
        ├── lib.rs            Crate-Root
        ├── redundancy.rs     Commitment-Hash-Vergleich (Stufe 1)
        └── delivery.rs       Auslieferungsmodi (Optimistic/Confirmed)
```

## Module

### `redundancy` — Commitment-Hash-Vergleich

Vergleicht die Commitment-Hashes zweier Pods an allen Spur-Positionen.
Binärer Vergleich (gleich/ungleich), parameterfrei (kein Schwellenwert).

```rust
use myl_verifier::compare_commitments;

let result = compare_commitments(&primary_trace, &redundant_trace)?;
match result {
    CompareResult::Match => { /* Pods stimmen überein */ },
    CompareResult::Mismatch { first_divergence } => { /* Abweichung bei Position */ },
}
```

### `delivery` — Auslieferungsmodi

Zwei Modi für die Ergebnis-Auslieferung:
- **Optimistic:** Sofortige Auslieferung + asynchroner Abgleich
- **Confirmed:** Zurückhalten bis Übereinstimmung bestätigt

```rust
use myl_verifier::{decide_delivery, VerificationMode};

let decision = decide_delivery(
    VerificationMode::Optimistic,
    &primary_trace,
    &redundant_trace,
)?;

match decision {
    DeliveryDecision::Deliver => { /* Ausliefern */ },
    DeliveryDecision::Hold => { /* Zurückhalten */ },
    DeliveryDecision::DeliverAndSlash { first_divergence } => { /* Ausliefern + Slashing */ },
}
```

## Tests

21 Tests grün (11 redundancy + 10 delivery):
- Commitment-Hash-Vergleich (identisch, abweichend, Längen-Mismatch)
- Auslieferungsentscheidungen (Optimistic/Confirmed × Match/Mismatch)
- Fehlerbehandlung (leere Spuren, Längen-Mismatch)

## Changelog

### v0.1.1 – 2026-08-17 (Phase 1: Redundanzvergleich)
- Commitment-Hash-Vergleich zweier Pods an allen Spur-Positionen
- Optimistische Auslieferung (sofort + asynchroner Abgleich)
- Bestätigte Auslieferung (zurückhalten bis Bestätigung)
- Binärer Vergleich (kein Schwellenwert), 21 Tests grün
