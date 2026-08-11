# compute-pipeline (`myl-pod`)

> **Version:** 0.0.0
> **Datum:** 2026-08-11
> **Status:** Planungsphase — teilweise blockiert (siehe Abhängigkeiten)

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

## Struktur

Entsteht mit der Implementierung.

## Changelog

Noch keine Version veröffentlicht.
