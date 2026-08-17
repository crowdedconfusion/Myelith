# verification (`myl-verifier`)

> **Version:** 0.3.0
> **Datum:** 2026-08-18
> **Status:** 🎉 **Phase 1 + 2 vollständig** (Punkte 1.1–1.3, 2.1–2.5):
> Redundanzvergleich (Stufe 1), Bisektions-Spiel (Stufe 2) mit Checker-Modul,
> Challenge-Erzeugung, Bisektionsprotokoll, On-Chain-Schiedsrunde, Slash-Logik.
> 66 Tests grün.

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


### v0.3.0 – 2026-08-18 (Audit-Block 4: Slashing und Schiedsrunden-Bindung)

**Fund A9 — zwei unvereinbare Slashing-Modelle.** `slash.rs` rechnete
mit **festen Beträgen** (`SlashConfig`: 1 MYL Slash, 0,5 MYL Kopfgeld),
während `myl_ledger::apply_verdict` einen **Anteil des Stakes**
schlachtet (`SlashParams` als Zähler/Nenner-Paare) — so wie Whitepaper
Kap. 5.5 es vorgibt (30–100 % je nach Vergehen). `myl-verifier` hing
nicht einmal an `myl-ledger` und konnte gar nicht buchen. Ein fester
Betrag hat zudem keine Abschreckungswirkung: 1 MYL ist für einen
Großstaker nichts, und die Sicherheitsannahme der gesamten
Verifikationsarchitektur (Kap. 6.9: Betrug muss teurer sein als der
erwartete Gewinn) hängt genau daran.

Behoben: `SlashDecision` entscheidet nur noch über **Schuld**, nicht
über Beträge. `to_ledger_verdict()` liefert den Schiedsspruch im
Ledger-Format; die Beträge ergeben sich aus Stake und
Governance-Parametern beim Buchen. `myl-ledger` ist jetzt Dependency,
und ein Test bucht eine Entscheidung tatsächlich durch
(`entscheidung_wird_vom_ledger_gebucht`, `slash_skaliert_mit_dem_stake`).

**Fund A11 — die Schiedsrunde band die Eingabe nicht an die Spur.**
`adjudicate()` prüfte die offengelegte Aktivierung nur gegen den in
derselben Antwort mitgelieferten Hash — eine tautologische Prüfung, die
nur feststellte, dass der Angeklagte in sich konsistent geantwortet
hat. Der `AdjudicationRequest` trug keinen Hash von a_{j-1}. Ein
Angeklagter, der eine **andere** Eingabe fand, die unter seiner
Ausführung den erwarteten Ausgabe-Hash ergibt, wurde freigesprochen —
und damit die Zusage aus Kap. 6.6 („die Schuldzuweisung ist eindeutig,
weil das Ergebnis kanonisch ist") ausgehebelt: kanonisch ist das
Ergebnis nur bezogen auf die committete Eingabe.

Behoben: `AdjudicationRequest.input_hash` (aus `Segment.trace[j-1]`);
die offengelegte Aktivierung wird dagegen geprüft. Regressionstest
`untergeschobene_eingabe_wird_nicht_freigesprochen` stellt genau den
Angriff nach — mit einem Executor, der auf die untergeschobene Eingabe
exakt den erwarteten Hash liefert.

- `Challenge` kommt jetzt aus `myl-types` statt aus einer eigenen
  Definition (Fund A8).
- Compiler-Warnung in `bisection.rs` behoben.
- 67 → 66 Tests (Tests für die entfallenen Betrags-Helfer `net_transfer`
  und `has_sufficient_stake` gestrichen, dafür Ledger-Durchbuchung und
  die Schiedsrunden-Regression neu).

### v0.2.6 – 2026-08-17 (Phase 2.4: On-Chain-Schiedsrunde)
- On-Chain-Schiedsrunde (adjudicate) mit ShardExecutor-Trait (8 Tests)
- AdjudicationRequest/Response/Result Strukturen
- Hash-Vergleich für Schuldzuweisung
- 8 neue Tests grün, insgesamt 66 Tests

### v0.2.5 – 2026-08-17 (Phase 2: Bisektions-Spiel)
- Checker-Modul: Segment-Nachrechnung mit SegmentAuditor-Trait (7 Tests)
- Challenge-Erzeugung: Erste abweichende Position bestimmen (10 Tests)
- Bisektionsprotokoll: O(log L) Runden, binäre Eingrenzung (9 Tests)
- Slash-Logik: VerdictOutcome, SlashDecision, Kopfgeld-Auszahlung (11 Tests)
- 37 neue Tests grün, insgesamt 58 Tests

### v0.1.1 – 2026-08-17 (Phase 1: Redundanzvergleich)
- Commitment-Hash-Vergleich zweier Pods an allen Spur-Positionen
- Optimistische Auslieferung (sofort + asynchroner Abgleich)
- Bestätigte Auslieferung (zurückhalten bis Bestätigung)
- Binärer Vergleich (kein Schwellenwert), 21 Tests grün
