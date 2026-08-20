# compute-pipeline (`myl-pod`)

> **Version:** 0.2.2
> **Datum:** 2026-08-18
> **Status:** 🎉 **Phase 2.1 abgeschlossen** (Punkte 1.1–1.4, 2.1):
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

### myl-pod v0.2.4 – 2026-08-19 (Reed-Solomon hinter der bestehenden Schnittstelle)

**Ein Fund über die eigene Arbeit.** Beim Bau der DA-Schicht für
CONSENSUS 4.3 hatte ich die Erasure-Mathematik in `myl-types` neu
angelegt — richtig — aber übersehen, dass `myl-pod` bereits eine
`ErasureCoder`-Schnittstelle mitbringt und der Modulkopf ausdrücklich
sagt: *„Die beschlossene Reed-Solomon-Variante (k=8/m=4) ist eine
Folge-Implementierung hinter derselben Schnittstelle."* Es gab also
einen vorgesehenen Platz, und ich hatte danebengebaut statt hinein.
Aufgefallen ist es erst, als `myl-testclient` nicht mehr übersetzte.

Jetzt: `ReedSolomonCoder` implementiert die vorhandene Schnittstelle und
setzt auf `myl_types::erasure` auf.

**Er behebt zugleich die dokumentierte Phase-1-Einschränkung.**
`XorParityCoder` legt den Längenkopf ungeschützt an den Anfang von
Fragment 0 und kann deshalb nicht rekonstruieren, wenn ausgerechnet
dieses Fragment fehlt (*„vollständige Kopf-Rekonstruktion folgt mit RS"*
stand im Code). `ReedSolomonCoder` stellt die Länge dem Klartext voran
und **codiert sie mit** — es gibt kein ausgezeichnetes Fragment mehr,
jede Teilmenge der Größe k genügt. Statt einem fehlenden Fragment
verträgt er **vier beliebige**; getestet über alle 495 Kombinationen.

`XorParityCoder` bleibt erhalten, der Modulkopf sagt jetzt aber, welcher
zu nehmen ist. `myl-testclient` nutzt den neuen.

### myl-pod v0.2.3 – 2026-08-19 (Fund 26 + Fund 20: Boundary-Schritt entfallen)

**Die Spur band den falschen Wert.** `ShardNode::process` bildete
`out_hash = activation_hash(&out)` über die Aktivierung in natürlicher
Ausgangsskala und schrieb ihn in die Spur; erst danach reskalierte
`finish()` auf die Boundary-Skala, und **dieser** Wert ging als `payload`
auf die Leitung. Der Folge-Shard prüfte mit
`verify_input_hash(&msg.payload, &msg.trace)` — also den Hash des
reskalierten Nutzdatensatzes gegen den Hash des unreskalierten. Beide
stimmen nur überein, solange die Reskalierung die Identität ist; seit
Fund 20 war sie es nicht. Der E2E-Test lehnte damit selbst die
**unmanipulierte** Aktivierung ab.

Das war mehr als ein roter Test: Die Spur ist die Commitment-Kette, die
VERIFICATION zwischen redundanten Pods vergleicht und die das
Bisektions-Spiel halbiert. Committet sie etwas anderes als das, was
übertragen wird, bindet sie nicht die ausgelieferte Arbeit.

**Behoben, indem der Boundary-Schritt ganz entfällt.** Er war reiner
Verlust ohne Gegenwert: Die Ausgangsskala des Senders ist
`layers[layer_end].residual_in_frac`, die Eingangsskala des Empfängers
`layers[layer_start].residual_in_frac` — und `layer_start` des Empfängers
**ist** `layer_end` des Senders. Beide Seiten lasen denselben Wert aus
demselben Artefakt (erzwungen durch `theta_v_hash`) und rechneten ihn
trotzdem über einen dritten, gröberen Skalar hin und zurück. Entfernt:
`rescale_von_kanal`, `rescale_zu_kanal`, `input_scale`, `output_scale`
und das Feld `boundary_frac`.

**Fund 20 fällt damit mit.** `test_pipeline_multinode.py` ist wieder
bitgleich mit der Einzelknoten-Runtime (vorher Divergenz ab dem sechsten
Token, 2746 gegen 2694); der weiche Zweig im Test ist zurück in ein
hartes `assert` überführt, wie es der Kommentar dort vorsah. Die
Phase-1-Akzeptanz „bitgleich mit Einzelknoten" gilt wieder
uneingeschränkt.

**Nachweis:** `pod_e2e.rs` 2/2 (vorher 0/2), Multi-Node-Integration
vollständig, Konformitätsvektoren 30/30, Gleitkomma-Audit null Treffer.
**θ_v unverändert** — die Einzelknoten-Inferenz war nie betroffen.

**Nachtrag am selben Tag — die Layout-Frage ist gemessen.** Drei Layouts
(4 Shards mit Grenzen 6/12/18, 8 Shards mit 3/6/9/…, und ungleichmäßig
1/7/23) liefern dieselben Token und sind bitgleich mit dem Einzelknoten
(`INTEGER_LLM/tests/integration/test_pipeline_layouts.py`). Damit trägt
der Entwurf „variable Knotenzahl je Pipeline" numerisch; von seinen zwei
Blockern ist der erste weg.

**Korrektur:** Oben stand zunächst, die Layout-Bindung aus Fund 25
blockiere diesen Entwurf. Das stimmt nicht — `verify_layout()` prüft das
Manifest gegen sich selbst, nicht gegen andere Pods. Cross-Pod-Gleichheit
erzwingt keine Codestelle. Die Prüfung bleibt trotzdem: Sie hat gerade
den `sha256:0000`-Platzhalter der 8-Node-Konfiguration gefangen.


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
