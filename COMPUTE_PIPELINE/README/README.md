# compute-pipeline (`myl-pod`)

> **Version:** 0.2.2
> **Datum:** 2026-08-18
> **Status:** 🎉 **Phase 1 + 2.1 vollständig** (Punkte 1.1–1.4, 2.1):
> `shard_loop` mit Spur-Hashes und Manipulationserkennung,
> `coordinator_loop` mit Micro-Batching, KV-Cache-Session-Affinität,
> erasure-codierte DA-Archivierung, Micro-Batching-Fenster-Tuning,
> Pipeline-Tracker für überlappende Batch-Verarbeitung.

Pod-Orchestrierung über ein echtes Netzwerk: Pipeline-Routing,
Micro-Batching, KV-Cache-Verwaltung, spekulatives Decoding.
Referenzimplementierung von Whitepaper Kap. 4 und Anhang A.3.

## Aufgabe

Schicht L2 (Compute Layer / Inference Fabric): der „Mining-Loop" — ein Pod
aus k Shard-Minern führt gemeinsam Forward-Pässe aus, koordiniert von einem
Pod-Koordinator, der Micro-Batches bildet und PoI-Bündel einreicht
(Anhang A.3). Diese Komponente ist die **Netzwerk-Orchestrierung** um
INTEGER_LLM herum — sie ersetzt nicht dessen Rechenkerne (`kernels`/
`runtime`), sondern verteilt sie über echte Nodes, mit Session-Affinität,
Ausfallsicherung und Durchsatzoptimierung.

## Abhängigkeiten

NETWORKING (Aktivierungs-Streams zwischen Shards), CONSENSUS
(Epochen-Scheduler liefert Pod-Zusammensetzung) sowie als fachliche
Vorstufe die Mehrknoten-Pipeline von INTEGER_LLM (Stage-Runtime mit echter
Layer-Ausführung über mehrere Knoten): INTEGER_LLMs `pipeline`-Crate liefert
die Stage-Runtime für einzelne Knoten; diese Komponente hebt das auf
Netzwerk-Ebene mit echter Miner-Rotation, Redundanz und Epochen-Wechsel.
`myl-pod` konsumiert die INTEGER_LLM-Stage-API
(`embed_token`/`run_layers`/`head_logits`) und die Typen aus `myl-types`.

## Struktur

```
COMPUTE_PIPELINE/
├── README/                   diese Kurzübersicht + Fahrplan
└── myl-pod/                  das Pod-Crate (Bibliothek + Node-Binary)
    ├── src/
    │   ├── lib.rs             Crate-Wurzel: #![deny(unsafe_code)], Module
    │   ├── wire.rs            Wire-Protokoll zwischen Shards (Borsh, Flags)
    │   ├── trace.rs           Spur-Hashes + Übergangs-Signaturen (BLS)
    │   ├── shard.rs           shard_loop: Eingangs-Prüfung, Forward,
    │   │                      Signieren, Session-Affinität, DA-Archiv
    │   ├── da.rs              DA-Archivierung (ErasureCoder, XOR-Parität)
    │   ├── coordinator.rs     coordinator_loop: Micro-Batching, Dispatch,
    │   │                      PoI-Bündel-Aggregation
    │   └── main.rs            myl-pod-node-CLI
    └── tests/
        └── pod_e2e.rs         Akzeptanztest: Determinismus + Bitgleich +
                               Manipulationserkennung
```

## Changelog


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

### v0.2.1 – 2026-08-17 (Phase 2.1: Micro-Batching + Pipelining)
- Micro-Batching-Collector mit konfigurierbarem Zeitfenster (default 250 ms)
  und Max-Batch-Größe (default 32). Pipeline-Tracker für überlappende
  Batch-Verarbeitung (4 Stadien: Receiving, Processing, Finalizing, Completed).
- Neues Modul `micro_batch.rs` mit 10 Tests grün.

### v0.1.4 – 2026-08-13 (Phase 1)
- `shard_loop` (Anhang A.3): Aktivierungen empfangen, Eingangs-Hash gegen
  die Spur prüfen (Manipulationserkennung), Forward-Pass über die
  INTEGER_LLM-Stage-API, Spur fortschreiben, Übergang BLS-signieren,
  weiterreichen; KV-Cache je Session (Session-Affinität, Kap. 4.2);
  DA-Archivierung der Aktivierungen (Anhang A.3 Schritt 6).
- `coordinator_loop` (Anhang A.3): Micro-Batching-Fenster (Default 250 ms),
  Session-/Segment-Id-Zuweisung, Pipeline-Dispatch, PoI-Bündel-Aggregation
  (Segments-Wurzel + BLS-Aggregat).
- **Akzeptanzkriterien erfüllt:** 4-Node-Pod liefert bitgleiche
  Token-Sequenz bei wiederholtem identischem Prompt und ist bitgleich mit
  der Einzelknoten-Runtime; Eingangs-Hash-Prüfung lehnt manipulierte
  Aktivierungen/Spur-Hashes ab (`tests/pod_e2e.rs`, 2 Tests + 13
  Unit-Tests grün, keine Warnungen).
