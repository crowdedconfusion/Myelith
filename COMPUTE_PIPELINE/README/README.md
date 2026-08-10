# compute-pipeline (`myl-pod`)

> **Version:** 0.0.0
> **Datum:** 2026-08-10
> **Status:** Phase 0 – teilweise blockiert (siehe Abhängigkeiten)

Pod-Orchestrierung über ein echtes Netzwerk: Pipeline-Routing, Micro-Batching, KV-Cache-Verwaltung, spekulatives Decoding. Referenzimplementierung von Whitepaper Kap. 4 und Anhang A.3 (`myl-pod`).

## Ziel

Schicht L2 (Compute Layer / Inference Fabric): der „Mining-Loop" — ein Pod aus k Shard-Minern führt gemeinsam Forward-Pässe aus, koordiniert von einem Pod-Koordinator, der Micro-Batches bildet und PoI-Bündel einreicht (Anhang A.3). Diese Komponente ist die **Netzwerk-Orchestrierung** um INTEGER_LLM herum — sie ersetzt nicht dessen Rechenkerne (`kernels`/`runtime`), sondern verteilt sie über echte Nodes, mit Session-Affinität, Ausfallsicherung und Durchsatzoptimierung.

**Abhängigkeit:** NETWORKING (Aktivierungs-Streams zwischen Shards), CONSENSUS Phase 2 (Epochen-Scheduler liefert Pod-Zusammensetzung), sowie **INTEGER_LLM Phase 12.56–12.63** (Mehrknoten-Pipeline mit echter Layer-Ausführung) als fachliche Vorstufe — INTEGER_LLMs `pipeline`-Crate liefert bereits eine Stage-Runtime für einzelne Knoten; diese Komponente hebt das auf Netzwerk-Ebene mit echter Miner-Rotation, Redundanz und Epochen-Wechsel.

## Struktur

(wird mit Phase 1 befüllt)

## Changelog

(noch keine Version veröffentlicht)
