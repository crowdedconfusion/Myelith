# consensus (`myl-consensus` + `myl-ledger` + `myl-scheduler`)

> **Version:** 0.4.2 (`myl-scheduler` 0.2.11)
> **Datum:** 2026-08-18
> **Status:** Design-Entscheidungen getroffen (malachite hinter
> trait-Grenze mit Eigenbau-Fallback, Blockzeit 2 s, Komitee 21/7,
> Streitfrist 7 Tage, Reed-Solomon k=8/m=4 — Details im Fahrplan);
> Phase 1 + 2 ✅ vollständig (`myl-ledger` v0.1.1–v0.1.5,
> `myl-scheduler` v0.2.1–v0.2.9); **Phase 3 ⚠️ Safety erfüllt,
> Liveness offen** (`myl-consensus` v0.3.1–v0.4.0): signiertes,
> stimmgewichtetes BFT mit VRF-rotierender Komiteewahl — aber noch
> ohne Rundenwechsel/Timeouts (Punkt 3.6), daher kein Leader-Ausfall
> überstehbar.

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

### myl-scheduler v0.2.11 – 2026-08-18 (Fund A20: Epoche geht in den VRF-Seed)

**Gefunden vom neuen `stack`-Lauf des Testclients** — ein Beleg dafür,
dass komponentenübergreifende Durchläufe etwas anderes finden als
Unit-Tests.

`derive_epoch_seed(prev_block_hash, vrf_sk, epoch)` nahm die Epoche
entgegen, speicherte sie im `EpochSeed` — und ließ sie **nicht in den
VRF-Eingang einfließen**. Alpha war allein der Block-Hash. Zwei Folgen:

1. **Umetikettierung.** `verify_epoch_seed` prüfte Alpha ohne Epoche.
   Ein Seed für Epoche 42 galt unverändert als gültiger Seed für Epoche
   99, mit demselben Beweis. Empirisch bestätigt, bevor der Fix einging.
2. **Wiederholte Zuteilung.** Zwei Epochen mit demselben Vorgängerblock
   (Reorganisation, leere Epoche, Neustart aus einem Snapshot) hätten
   exakt dieselbe Pod-Bildung, Shard-Zuweisung und Stichprobenauswahl
   ergeben.

Der bestehende Test `derive_seed_different_epochs` behauptete das alte
Verhalten ausdrücklich als gewollt („Beta sollte gleich sein"). Es war
also eine bewusste Festlegung — aber eine, die die eigene
Verifikationsfunktion unterläuft.

Behoben: neues `seed_alpha()` mit
`"MYELITH_EPOCH_SEED_v1" ‖ prev_block_hash ‖ u64_le(epoch)`; Ableitung
und Verifikation nutzen dieselbe Bytefolge. Regressionstest
`umetikettierter_seed_wird_abgelehnt`. 58 → 60 Tests.
**Konsensrelevant** — jede abgeleitete Zuteilung verschiebt sich.


### Audit-Block 5 – 2026-08-18 (Warnungsfreiheit, Tests, Float-Audit)

Repository-weiter Block; die Einzelheiten stehen im jeweiligen Fahrplan.

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


### v0.4.1 – 2026-08-18 (Audit-Block 4: kanonische Blocktypen)

**Fund A8 — der Block definierte eigene Fassungen der Protokolltypen.**
`block.rs` hatte eigene `PoiBundle`, `Challenge` und `Verdict` mit
anderen Feldern als die Typen, die die übrigen Komponenten tatsächlich
produzieren:

| block.rs (alt) | kanonisch |
|---|---|
| `PoiBundle { segment_id, commitment_hash, pod_id: [u8;32], signature: [u8;96] }` | `myl_types::PoIBundle { epoch, pod, segments_root, vtfe_claimed, aggregate_sig }` |
| `Challenge { segment_id, first_divergence, challenger, accused }` | `myl_types::Challenge` (mit beiden Pods und beiden Hashes) |
| `Verdict { segment_id, winner, loser, slash_amount }` | `myl_ledger::Verdict { segment_id, miner, checker, outcome }` |

Die Folge war eine stille Integrationslücke: `myl-pod` erzeugt das
Epochen-Aggregat `myl_types::PoIBundle` (Anhang A.1), aber
`Block::add_poi_bundle` nahm eine per-Segment-Struktur — der Pfad
Pod → Block war nie verdrahtet, obwohl beide Seiten als „vollständig"
geführt wurden. Ebenso hätte kein Verdict des Verifiers je gebucht
werden können. Rohe `[u8; 32]`/`[u8; 96]`-Felder sind durch die
Newtypes aus `myl-types` ersetzt — genau dafür gibt es SHARED_TYPES.

**Fund A10 — `myl-ledger` war als Abhängigkeit deklariert, aber nie
benutzt** (null Referenzen im Quelltext). Konsequenz: `EpochMeta` trug
keinen `state_root`. Ein Validator konnte nur prüfen, ob die Bytes des
Blocks gleich sind — nicht, ob der Vorschlagende die Zustandsübergänge
korrekt angewendet hat. Ein Leader hätte einen syntaktisch
einwandfreien Block mit falsch gebuchtem Slashing vorschlagen können.
`EpochMeta` hat jetzt `state_root: Hash`
(`LedgerState::commitment()`), und der Test
`state_root_geht_in_den_blockhash_ein` sichert, dass er in den Hash
eingeht, über den abgestimmt wird.

- 97 → 100 Tests.

### v0.4.0 / myl-scheduler v0.2.9 – 2026-08-18 (Audit-Block 3: BFT-Kryptografie)

**Fund A3 — das BFT-Protokoll enthielt keine Kryptografie.**
`Propose`, `Vote` und `Commit` hatten kein Signaturfeld, und `BftState`
kannte das Komitee nicht: der Zustandsautomat zählte Nachrichten, ohne
zu prüfen, wer sie geschickt hat. Ein einzelner Angreifer erreichte den
Threshold mit 15 erfundenen Miner-IDs. `BftError::InvalidSignature` war
als „(Placeholder)" deklariert und wurde nirgends zurückgegeben.

Behoben:
- Alle drei Nachrichtentypen tragen eine `BlsSignature`.
- Neuer Typ `VotingSet` bündelt, was die Runde zur Prüfung braucht:
  wer stimmberechtigt ist, mit welchem Schlüssel geprüft wird und mit
  welchem Gewicht die Stimme zählt.
- Jede Nachricht durchläuft vier Prüfungen in der Reihenfolge billig
  vor teuer: Runde → Mitgliedschaft → Duplikat → BLS-Signatur.
- Validatoren registrieren sich mit ihrem BLS-Public-Key; ein
  ungültiger Schlüssel wird bei der Registrierung abgelehnt statt
  später jede Signaturprüfung scheitern zu lassen.

**Fund A7 — Stimmgewicht war berechnet, aber nirgends angeschlossen.**
`voting_weight.rs` (297 Zeilen, getestet) wurde von keinem anderen Modul
aufgerufen. `receive_vote` zählte Köpfe, `select_committee` sortierte
rein nach Stake und nahm die ersten 28 — eine feste Rangliste ohne die
im Whitepaper (Kap. 3.5) genannte VRF-Rotation, also in jeder Epoche
dieselben 21 Adressen.

Behoben:
- Quorum ist `> 2/3` des **Stimmgewichts** statt der Nachrichtenzahl.
- `select_committee(registry, epoch, vrf_seed)` zieht gewichtet ohne
  Zurücklegen aus dem VRF-Epochenseed → Rotation **und** Kopplung an
  Stake und Arbeit.
- Validatoren führen eine `InferenceHistory` statt eines flachen
  Zählers; `record_work(miner, epoch, work)` speist sie.
- **Formeländerung (konsensrelevant, bitte bestätigen):**
  `stake × Arbeit` → `stake + stake · Arbeit / VTFE_UNIT`. Das reine
  Produkt gab jedem Validator ohne Arbeitshistorie Gewicht 0 — bei
  Genesis wäre kein Komitee wählbar gewesen, und wer bei 0 startet,
  wird nie gewählt und kann nie Arbeit nachweisen.

**Weitere Korrekturen im selben Block:**
- `BftState::new` gibt `Result` zurück — vorher `(committee_size - 1) / 3`
  mit usize-Underflow bei leerem Komitee.
- `select_leader` gibt `Option` zurück — vorher Division durch null bei
  leerer Producer-Liste.
- `apply_decay` rechnet in u128 mit Sättigung — vorher `value * 95` in
  u64: Panic im Debug-Build, stiller Umlauf im Release-Build, also
  je nach Build-Profil verschiedene Stimmgewichte.
- `SeedRng`/`deterministic_shuffle` nach `myl-types` verschoben und um
  `weighted_sample_without_replacement` ergänzt; `myl-scheduler` nutzt
  jetzt die geteilte Fassung.
- 63 → 97 Tests.

### v0.3.6 / myl-scheduler v0.2.8 – 2026-08-18 (Audit-Block 2: Konsens-Determinismus)

**Fund A4 — Double-Signing-Beweise waren wertlos und zugleich fälschbar.**
`SignedBlocksRegistry::register_signed_block()` erzeugte bei erkanntem
Double-Signing einen Beweis mit `signature_1 = signature_2 = [0u8; 96]`,
während `DoubleSignProof::validate()` verlangte, dass die Signaturen
verschieden sind — der Erkennungspfad konnte also **nie** einen
verwertbaren Beweis liefern. Umgekehrt prüfte `validate()` die Signaturen
nie gegen einen öffentlichen Schlüssel: jeder Beliebige hätte mit zwei
erfundenen Bytefolgen einen „gültigen" Beweis gegen jeden Validator
fabrizieren können. Beide Funktionen waren einzeln getestet, nie gemeinsam.

Behoben:
- Die Registry speichert die tatsächlich abgegebene BLS-Signatur mit
  (`HashMap<u64, (Hash, BlsSignature)>`) und liefert echte Beweise.
- `validate()` ist durch `verify(&BlsPublicKey)` ersetzt — es gibt keine
  Prüfung ohne Schlüssel mehr, damit dieselbe Lücke nicht wiederkehren kann.
  Geprüft werden: verschiedene Block-Hashes, verschiedene Signaturen und
  **beide BLS-Signaturen gegen den Schlüssel des Beschuldigten**.
- `signature_1/2` sind jetzt `BlsSignature` statt nackter `[u8; 96]`.
- Neues Modul `signing.rs`: kanonische, domain-getrennte Signierbotschaften
  für Propose/Vote/Commit (`MYELITH_BFT_*_v1 ‖ u64_le(round) ‖ block_hash`).
  Ohne Domain-Separation wäre eine Vote zugleich ein gültiger Commit.
- Regressionstest `erkannter_beweis_besteht_die_eigene_pruefung` plus Tests
  für erfundene, fremde, rundenfremde und typfremde Signaturen.
- 53 → 63 Tests.

**Fund A6 — Die Stichproben-Lotterie war nicht gleichverteilt.**
Der Fisher-Yates-Shuffle zog den Vertauschungsindex aus einem einzigen
Byte (`state[0] as usize % (i + 1)`). Messung bei 1 000 Segmenten und 2 %
Rate: Index 0 wurde mit dem **0,14-fachen**, Index 256 mit dem
**3,87-fachen** des Erwartungswerts gezogen — Spreizung Faktor ~28. Für
die Lotterie, die entscheidet, welche Arbeit auditiert wird, hing die
Prüfwahrscheinlichkeit damit am Segmentindex statt am Zufall. Zusätzlich
nutzte der XOR-Shift nur `state[0..8]`: **192 der 256 VRF-Seed-Bits
gingen nie ein**. Dieselbe fehlerhafte Funktion lag in **vier Kopien** in
`sampling.rs`, `redundancy.rs`, `shard_assignment.rs` und
`geo_clustering.rs`.

Behoben:
- Neues Modul `shuffle.rs` mit **einer** Implementierung für alle vier
  Verwendungen. RNG: SHA-256 im Zählermodus (`sha256(seed ‖ counter_le)`),
  alle 256 Seed-Bits gehen ein. Index-Wahl per Verwerfungsverfahren statt
  `% n` (exakte Gleichverteilung, Determinismus bleibt erhalten).
- Nachmessung: Spreizung **0,89× – 1,14×** (reine Stichprobenstreuung).
- Tests für Seed-Vollständigkeit, Gleichverteilung über 1 000 Positionen
  und das Fehlen einer Stufe an der alten 256er-Grenze.
- 56 → 66 Tests.
- **Konsensrelevant:** Die Zuteilung aller Epochen verschiebt sich. Da MYL
  nicht im Umlauf ist, ist das der richtige Zeitpunkt.

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
