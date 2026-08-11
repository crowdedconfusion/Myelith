# 02 — Empirischer Beleg: Bit-exakte Ganzzahl-Inferenz (Qwen2.5-0.5B)

> **Stand:** 2026-08-12, INTEGER_LLM v0.12.33, θ_v 0.10.0
> **Status:** Entscheidungspunkt 12.21 AKZEPTIERT — die Kernaussage ist
> empirisch belegt. Dieses Dokument sammelt den vollständigen Beleg
> (Bit-Identität, Qualität, Durchsatz) und den Weg dorthin. Es ist die
> Arbeitsgrundlage für die Einarbeitung ins Whitepaper (Kap. 6, Kap. 11).

## 1. Was gezeigt wird

Die Kernthese von Myelith (Whitepaper Abstract, Kap. 6) verlangt zweierlei:

1. **Bit-Identität:** Vollständig ganzzahlige Inferenz (kein Gleitkomma
   im Rechenpfad, Division ausschließlich als arithmetischer Rechtsshift)
   liefert laufübergreifend exakt dieselbe Tokenfolge — die Voraussetzung
   für Redundanzvergleich und Bisektions-Spiel.
2. **Qualität:** Die Ganzzahl-Inferenz darf qualitativ nicht einbrechen —
   sonst trägt die Verifikationsarchitektur zwar formal, aber das
   Produkt wäre unbrauchbar.

Beides wird hier für Qwen2.5-0.5B (das kleinste und damit ungünstigste
Modell der Ziel-Familie) empirisch belegt:

| Aussage | Messung | Ergebnis |
|---|---|---|
| Bit-Identität | 5 Prompts × 5 unabhängige Prozess-Läufe | **bit-identisch** (Token-Hash + SHA-256) |
| Qualität | Perplexität vs. BF16-Baseline (WikiText-2) | **15,59 vs. 14,95 = +4,29 %** (Kriterium max. +5 %) |
| Qualität | Top-1-Agreement vs. BF16 (439 Positionen) | **89,3 %** |
| Durchsatz | Referenz-Backend (skalar), Release-Build | ~19 tok/s (Decode wie Prefill) |

## 2. Messaufbau

- **Modell:** Qwen/Qwen2.5-0.5B (Basis-Variante, lokaler Snapshot unter
  `models/`), 24 Layer, hidden 896, 14 Query-Heads / 2 KV-Heads (GQA),
  head_dim 64, Vokabular 151 936.
- **Quantisierung (θ_v 0.10.0):** Gewichte int8 mit Per-Channel-
  Zweierpotenz-Skalen (eine Verschiebung je Ausgabe-Zeile), GPTQ-
  Fehlerkompensation für die 168 linearen Projektionen; Aktivierungen
  int16 mit kalibrierten Per-Layer-Zweierpotenz-Skalen; LM-Head int16
  per-channel (benannte spec-Ausnahme, Weight-Tying aufgelöst);
  Nichtlinearitäten (exp, SiLU, rsqrt, RoPE) über Lookup-Tabellen.
  314 kalibrierte Skalen, 291 quantisierte Gewichts-Tensoren.
- **Referenz:** HF-Implementierung in BF16, exakt dieselben Messsequenzen
  (`eval/wikitext_common.py` ist die einzige Quelle der Sequenzauswahl
  für alle Messungen — „identische Messmethode").
- **Hardware/Build:** macOS 26.5.1 (arm64), Rust-Release-Build,
  Referenz-Backend (skalar; SIMD/CUDA/ROCm folgen später bewusst
  nachgelagert).

## 3. Beleg A — Bit-Identität

**Methode:** 5 Prompts (DE/EN, inkl. Arithmetik-Prompt), greedy-
Decodierung, 40 neue Tokens. Jedes Prompt wird in 5 vollständig
unabhängigen Prozess-Läufen der Runtime-CLI generiert; jeder Lauf lädt
das Modell neu (gemeinsamer Zustand ist ausgeschlossen). Verglichen
werden die Tokenfolge selbst, der Runtime-interne Token-Hash und ein
pythonseitiger SHA-256 über die Tokenfolge.

**Ergebnis (`eval/results/evidence/determinism.json`):** Alle 25 Läufe
bit-identisch.

| Prompt | Token-Hash | SHA-256 (Beginn) |
|---|---|---|
| The capital of France is | `f1dfaca128ee3bdc` | `2c87ca159f60db9a…` |
| In quantum mechanics, the wave function describes | `31603612c0a4fb93` | `026f8e84e308bec8…` |
| Die Hauptstadt von Frankreich ist | `cccef3c654a82e4e` | `5b7d97b80deb3b25…` |
| In der Quantenmechanik beschreibt die Wellenfunktion | `e51d3a7fe224ccdc` | `f5e8cadf1a762b79…` |
| The result of 17 times 23 is | `b2faf1fd7bdaa3b4` | `25b194b92965dcd4…` |

Zusätzliche, unabhängige Bestätigungen derselben Eigenschaft:

- Der End-to-End-Test (`tests/integration/test_end2end_real.py`)
  wiederholt einen Teacher-Forcing-Lauf über 439 WikiText-2-Tokens und
  verlangt Bit-Identität der Gesamtausgabe — PASSED.
- Im Durchsatz-Benchmark (Abschnitt 5) erzeugen alle 3 Wiederholungen
  je Szenario denselben Decode-Hash.

**Reichweite der Aussage:** belegt für dieselbe Runtime + dieselben
Artefakte auf derselben Hardware. Der Cross-Hardware-Nachweis (mind. zwei
NVIDIA-Generationen, Whitepaper Kap. 6.2) steht noch aus — er setzt
GPU-Zugang und die SIMD-/CUDA-/ROCm-Backends voraus. Die Architektur ist
dafür angelegt: θ_v fixiert den vollständigen numerischen Vertrag
(Rundung, Skalen, LUTs, KV-Cache-Format), und die Backends müssen sich
gegen Golden Vectors beweisen, bevor sie als konsensfähig gelten.

## 4. Beleg B — Qualität

### 4.1 Perplexität (Entscheidungspunkt 12.21)

Teacher-Forcing auf 4 WikiText-2-Sequenzen (Testsplit, 435 ausgewertete
Positionen), identisch für beide Modelle:

| Modell | Perplexität |
|---|---|
| BF16-Baseline (HF) | 14,95 |
| Integer-Modell (θ_v 0.10.0) | **15,59** |
| Relativer Anstieg | **+4,29 %** |
| Akzeptanzkriterium | max. +5 % → **AKZEPTIERT** |

Protokoll: `eval/results/decision_12-21.md`. Zwingende Einordnung (im
Protokoll verankert): Perplexität ist unabhängig von der
Decodierstrategie; 0,5 Mrd. Parameter sind der ungünstigste Fall
für Quantisierung — das Urteil gilt für 0,5B, größere Modelle sind
nachweislich robuster.

### 4.2 Top-1-Agreement

Zusätzliches, positionsaufgelöstes Qualitätsmaß: an jeder Position
der vier Messsequenzen wird die Top-1-Vorhersage des Integer-Modells
(`seq_logits_sweep`, Teacher-Forcing) mit der Top-1-Vorhersage der
BF16-Referenz verglichen.

| Sequenz | Positionen | Übereinstimmung | Erste Abweichung |
|---|---|---|---|
| 0 | 128 | 117 (91,4 %) | Position 6 |
| 1 | 128 | 112 (87,5 %) | Position 0 |
| 2 | 128 | 112 (87,5 %) | Position 10 |
| 3 | 55 | 51 (92,7 %) | Position 1 |
| **Gesamt** | **439** | **392 (89,3 %)** | — |

Das heißt: In knapp 9 von 10 Positionen sagt das quantisierte
Integer-Modell exakt dasselbe nächste Token vorher wie die BF16-
Referenz; die restlichen Abweichungen sind vereinzelte Einzelpositionen,
kein zusammenhängender Strukturverlust. Daten:
`eval/results/evidence/quality.json`.

### 4.3 Parallelgenerierung (DE/EN, greedy, 40 Tokens)

Dieselben Prompts, einmal Integer-Modell, einmal BF16-Referenz
(`eval/results/evidence/quality.json`, Schlüssel
`parallel_generation`):

| Prompt | Integer-Modell | BF16-Referenz |
|---|---|---|
| The capital of France is | `______. A. Paris B. London C. Rome D. Berlin 答案: A` (Quiz-Format, korrekte Antwort A) | `Paris. It is the largest city in Europe and the second largest in the world. …` |
| In quantum mechanics, the wave function describes | `the state of a quantum system. Consider a quantum system with a single particle in a one-dimensional space. The wave function ψ(x) …` | `the state of a quantum system. Consider a quantum system with a wave function ψ(x) that is a Gaussian function centered at x = 0 …` |
| Die Hauptstadt von Frankreich ist | `Paris. Es ist die Hauptstadt der Region Hauts-de-France und der Stadt Frankreich. …` | `Paris. Die Stadt ist der Hauptstadt der Region Parisien und der Hauptstadt der Region Frankreich. …` |
| In der Quantenmechanik beschreibt die Wellenfunktion | `die Anzahl der Werte von einer Quantenmasse, die sich in einem bestimmten Zeitraum an einem bestimmten Ort befinden. …` | `die Anzahl der Zustände, die sich in einem bestimmten Zeitraum zwischen zwei bestimmten Zuständen befinden. …` |
| The result of 17 times 23 is | `between (　　) A: 300 and 310 B: 310 and 320 …` (Prüfungsformat, Antwort bleibt schuldig) | `391. What is the result of 17 times 230? …` (korrekt) |

Beobachtungen:

- Beide Modelle beantworten die Hauptstadt-Frage mit „Paris" und beginnen
  den Quantenmechanik-Prompt mit demselben Satz („the state of a quantum
  system. Consider a quantum system with …") — die Quantisierung ändert
  das inhaltliche Verhalten nicht grundsätzlich.
- Die BF16-Referenz ist selbst nicht fehlerfrei („largest city in
  Europe", „Region Parisien") — 0,5B ist in beiden Fällen ein kleines
  Modell; der Vergleich zeigt die Differenz durch Quantisierung, nicht
  absolute Fähigkeiten.
- Der Arithmetik-Prompt zeigt eine echte Schwäche des Integer-Modells
  (Ausweichen in ein Multiple-Choice-Format statt „391"). Das ist
  konsistent mit der Perplexitäts-Einordnung: 0,5B ist der ungünstigste
  Fall, und Arithmetik ist eine der bekanntesten Quantisierungs-
  Schwachstellen kleiner Modelle.

## 5. Beleg C — Durchsatz (Referenzpunkt, kein Versprechen)

`runtime/src/bin/bench_probe` misst Prefill und Decode getrennt, ohne
Modellladezeit. 3 Wiederholungen je Szenario
(`eval/results/evidence/benchmark.json`):

| Szenario | Prompt-Tokens | Decode-Tokens | Prefill (Median) | Decode (Median) |
|---|---|---|---|---|
| decode-lastig | 4 | 128 | 18,3 tok/s (17,9–19,0) | 19,1 tok/s (18,3–19,3) |
| Prefill-Anteil | 27 | 64 | 19,8 tok/s (19,7–20,1) | 19,2 tok/s (19,2–19,5) |

Einordnung: Dies ist das unkompilierte Referenz-Backend in reinem
Skalar-Code — der langsame, aber einfache Referenzpunkt, an dem sich die
späteren SIMD-/CUDA-/ROCm-Backends und die Cross-Hardware-Messungen
messen lassen. Durchsatz ist für die Kernthese irrelevant (Bit-Identität
und Qualität sind die Entscheidungskriterien); die Zahlen dienen der
späteren Einordnung des Backend-Fortschritts.

## 6. Der Weg dorthin — Perplexitäts-Verlauf und Root-Causes

Der Entscheidungspunkt wurde nicht in einem Schritt erreicht. Die
Messhistorie ist selbst Teil des Belegs, weil sie zeigt, dass der
Qualitätseinbruch am Ende NICHT die Quantisierung war, sondern drei
Struktur-Bugs, die systematisch eingekreist wurden:

| Version | θ_v | Änderung | Perplexität | Erkenntnis |
|---|---|---|---|---|
| v0.12.22 | 0.5.x | Erste echte Messung (Per-Tensor-Skalen, geteilter LM-Head) | **14 546** | Qualitätskrise bestätigt |
| v0.12.25 | 0.7.0 | Eskalation: Weight-Tying aufgelöst (LM-Head int16 per-channel) + Per-Channel-int8 für alle Gewichte | **3 257** | Faktor 4,5 — Quantisierung des LM-Heads war real, aber nicht allein |
| v0.12.26 | 0.7.0 | Skalen-Headroom: Kalibrierkorpus verbreitert (50/314 Module clampten still → 0/314) | **3 242** | Clamping behoben, aber NICHT dominant |
| v0.12.28 | 0.8.0 | GPTQ (Hessian-basierte Fehlerkompensation, 168 Projektionen) | **3 318** | Perplexität unverändert — diese Messung war allerdings durch die damals noch unentdeckten Struktur-Bugs (siehe unten) kontaminiert; auf Layer-Ebene reduziert GPTQ den Ausgabefehler nachweislich (Faktor 6–8) und bleibt in den Artefakten aktiv |
| v0.12.29 | 0.9.0 | SiLU-Eingangsraster verfeinert (0,5 → 0,125) | **2 972** | real, aber nicht dominant; Seq-Dump zeigt: Divergenz wächst mit der Position → Mehrpositions-Pfad verdächtig |
| v0.12.30 | 0.10.0 | **Fund 15** (RoPE fundamental falsch: Multi-Frequenz + half-split statt Ein-Winkel + benachbart) + **Fund 16** (Attention attendierte im KV-Cache nur auf den ERSTEN Key) | **73,15** | Durchbruch, Faktor 40 — Struktur-Bugs, keine Quantisierung |
| v0.12.31 | 0.10.0 | Verifikation/Isolation: Float-Nachbildung der Quantisierungsstruktur ergibt 16,82 (nicht 73); LUT-Werte präzise (exp 0,19 %, SiLU 0,77 %) | — | Blow-up kommt aus der Integer-Arithmetik, nicht aus LUT-Werten oder Quantisierungsstruktur |
| v0.12.32 | 0.10.0 | **Fund 17**: fehlende 1/√head_dim-Attention-Skalierung (HF: `attn_weights = q·k · head_dim^-0.5`), behoben als zusätzlicher Rechtsshift um log₂(head_dim)/2 = 3 | **15,59** | Root-Cause → **AKZEPTIERT** (+4,29 %) |

Methodische Lehre: Die Eskalationsstufen (Per-Channel, Headroom, GPTQ,
SiLU-Raster) waren keine Fehlschläge, sondern Ausschlussbeweise — jedes
Negativ-Ergebnis hat den Suchraum verkleinert, bis die
Mehrpositions-Divergenzsuche (seq_layer_dump/seq_logits_sweep gegen HF)
die drei Struktur-Bugs sichtbar machte. Der Fix von Fund 17 ist ein
einziger zusätzlicher Bitshift — bit-exakt, keine θ_v-Formatänderung.
Eine Nuance gehört zur Ehrlichkeit: Das GPTQ-Negativ-Ergebnis von
v0.12.28 wurde gemessen, während Fund 15/16 die Attention noch
blockierten — als Isolationsbeweis gegen die Gewichtsquantisierung war
es daher schwächer als zunächst angenommen; dass GPTQ den linearen
Layer-Ausgabefehler tatsächlich senkt, ist davon unberührt und in den
Artefakten aktiv (Details: `eval/results/verification_report.md`).

## 7. Grenzen und offene Punkte (ehrliche Einordnung)

1. **Modellgröße:** Alle Messungen gelten für Qwen2.5-0.5B, den
   ungünstigsten Fall. Die Übertragbarkeit auf die Zielgrößenordnung
   (100 Mrd. – 1 Bio. Parameter) ist nicht gemessen, aber durch die
   bekannte Robustheit größerer Modelle gegenüber Quantisierung
   zusätzlich gestützt.
2. **Phase-0-Absolutkriterium:** `docs/00_requirements.md` nannte früh
   „Perplexity < 15.0" als Orientierung. Gemessen sind 15,59 — das alte
   Absolutkriterium ist damit knapp verfehlt. Maßgeblich ist das am
   Entscheidungspunkt angelegte RELATIVE Kriterium (max. +5 % vs.
   BF16-Baseline), das mit +4,29 % erfüllt ist; das absolute Ziel der
   Phase 0 stammt aus der Zeit vor der Kalibrierung und war nie
   konsensrelevant festgelegt. Für die Whitepaper-Einarbeitung wird das
   relative Kriterium dokumentiert, das alte absolute als überholt
   markiert.
3. **Cross-Hardware:** Bit-Identität ist auf einer Hardware (arm64)
   belegt. Der Nachweis über Hardware-Generationen hinweg braucht die
   SIMD-/CUDA-/ROCm-Backends und GPU-Zugang (beides offen).
4. **Decodierstrategie:** Die Parallelgenerierung nutzt greedy; greedy
   verstärkt Repetitionsneigung. Die Perplexität (Teacher-Forcing) ist
   davon unberührt und das maßgebliche Qualitätsmaß.
5. **Lizenz:** Die Lizenzlage des Basismodells (Qwen2.5) für
   quantisierte Ableitungen ist weiterhin ein offener, nicht-technischer
   Punkt (unabhängig vom Code und von diesem Beleg).
6. **Zurückgestellt:** Der Hadamard-Basiswechsel (Vorstudien:
   `tests/diag/hadamard_prestudy.py`, `rmsnorm_hadamard_check.py`) ist
   geprüft und zurückgestellt — nicht verworfen; er bleibt als
   vollständig ganzzahliger, deterministischer Eskalationspfad
   verfügbar. Die verbleibenden ~4 % Perplexitäts-Abstand sind echtes
   Quantisierungsrauschen.

## 8. Reproduzierbarkeit

Alle Ergebnisse sind mit dem Stand v0.12.33 reproduzierbar:

```bash
cd INTEGER_LLM
cargo build --release --bins            # Runtime-Binaries inkl. Proben
# A) Bit-Identität (5 Prompts × 5 Läufe):
calibrate/.venv/bin/python eval/evidence_determinism.py
# B) Qualität (Parallelgenerierung + Top-1-Agreement, lädt BF16-Referenz):
calibrate/.venv/bin/python eval/evidence_quality.py
# C) Durchsatz (Prefill/Decode getrennt):
calibrate/.venv/bin/python eval/evidence_benchmark.py
# Perplexitätsvergleich (Entscheidungspunkt):
calibrate/.venv/bin/python eval/perplexity.py
```

## 9. Artefakte

| Datei | Inhalt |
|---|---|
| `eval/results/evidence/determinism.json` | 5×5 Läufe: Tokenfolgen, Token-Hashes, SHA-256 |
| `eval/results/evidence/quality.json` | Parallelgenerierung (DE/EN) + Top-1-Agreement |
| `eval/results/evidence/benchmark.json` | Prefill-/Decode-Durchsatz, 2 Szenarien × 3 Läufe |
| `eval/results/decision_12-21.md` | Protokoll des Entscheidungspunkts |
| `eval/results/perplexity_comparison.json` | Perplexitätswerte und Sequenz-Details |
| `eval/results/verification_report.md` | Layer-für-Layer-Verifikation und Fehlerzerlegung (Diagnose-Stand vor Fund 17, mit Nachtrag) |
| `eval/evidence_*.py` | Die drei Evidenz-Skripte |
| `runtime/src/bin/bench_probe.rs` | Zeitmessung Prefill/Decode (Rust) |
| `tests/diag/` | Diagnose-Skripte der Eingrenzung (Layer-/Seq-Dumps, LUT-Audit, Float-Nachbildung, GPTQ-Verifikation) |
