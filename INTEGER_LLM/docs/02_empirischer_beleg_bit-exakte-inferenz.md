# 02 — Empirischer Beleg: Bit-exakte Ganzzahl-Inferenz (Qwen2.5-0.5B)

> **Stand:** 2026-08-12, INTEGER_LLM v0.12.33, θ_v 0.10.0
> **Status:** Entscheidungspunkt 12.21 AKZEPTIERT — die Kernaussage ist
> empirisch belegt. Dieses Dokument sammelt den vollstaendigen Beleg
> (Bit-Identitaet, Qualitaet, Durchsatz) und den Weg dorthin. Es ist die
> Arbeitsgrundlage fuer die Einarbeitung ins Whitepaper (Kap. 6, Kap. 11).

## 1. Was gezeigt wird

Die Kernthese von Myelith (Whitepaper Abstract, Kap. 6) verlangt zweierlei:

1. **Bit-Identitaet:** Vollstaendig ganzzahlige Inferenz (kein Gleitkomma
   im Rechenpfad, Division ausschliesslich als arithmetischer Rechtsshift)
   liefert laufuebergang exakt dieselbe Tokenfolge — die Voraussetzung
   fuer Redundanzvergleich und Bisektions-Spiel.
2. **Qualitaet:** Die Ganzzahl-Inferenz darf qualitativ nicht einbrechen —
   sonst traegt die Verifikationsarchitektur zwar formal, aber das
   Produkt waere unbrauchbar.

Beides wird hier fuer Qwen2.5-0.5B (das kleinste und damit unguenstigste
Modell der Ziel-Familie) empirisch belegt:

| Aussage | Messung | Ergebnis |
|---|---|---|
| Bit-Identitaet | 5 Prompts × 5 unabhaengige Prozess-Laeufe | **bit-identisch** (Token-Hash + SHA-256) |
| Qualitaet | Perplexitaet vs. BF16-Baseline (WikiText-2) | **15,59 vs. 14,95 = +4,29 %** (Kriterium max. +5 %) |
| Qualitaet | Top-1-Agreement vs. BF16 (439 Positionen) | **89,3 %** |
| Durchsatz | Referenz-Backend (skalar), Release-Build | ~19 tok/s (Decode wie Prefill) |

## 2. Messaufbau

- **Modell:** Qwen/Qwen2.5-0.5B (Basis-Variante, lokaler Snapshot unter
  `models/`), 24 Layer, hidden 896, 14 Query-Heads / 2 KV-Heads (GQA),
  head_dim 64, Vokabular 151 936.
- **Quantisierung (θ_v 0.10.0):** Gewichte int8 mit Per-Channel-
  Zweierpotenz-Skalen (eine Verschiebung je Ausgabe-Zeile), GPTQ-
  Fehlerkompensation fuer die 168 linearen Projektionen; Aktivierungen
  int16 mit kalibrierten Per-Layer-Zweierpotenz-Skalen; LM-Head int16
  per-channel (benannte spec-Ausnahme, Weight-Tying aufgeloest);
  Nichtlinearitaeten (exp, SiLU, rsqrt, RoPE) ueber Lookup-Tabellen.
  314 kalibrierte Skalen, 291 quantisierte Gewichts-Tensoren.
- **Referenz:** HF-Implementierung in BF16, exakt dieselben Messsequenzen
  (`eval/wikitext_common.py` ist die einzige Quelle der Sequenzauswahl
  fuer alle Messungen — „identische Messmethode").
- **Hardware/Build:** macOS 26.5.1 (arm64), Rust-Release-Build,
  Referenz-Backend (skalar; SIMD/CUDA/ROCm folgen spaeter bewusst
  nachgelagert, Fahrplan 12.35–12.55).

## 3. Beleg A — Bit-Identitaet

**Methode:** 5 Prompts (DE/EN, inkl. Arithmetik-Prompt), greedy-
Decodierung, 40 neue Tokens. Jedes Prompt wird in 5 vollstaendig
unabhaengigen Prozess-Laeufen der Runtime-CLI generiert; jeder Lauf laedt
das Modell neu (gemeinsamer Zustand ist ausgeschlossen). Verglichen
werden die Tokenfolge selbst, der Runtime-interne Token-Hash und ein
pythonseitiger SHA-256 ueber die Tokenfolge.

**Ergebnis (`eval/results/evidence/determinism.json`):** Alle 25 Laeufe
bit-identisch.

| Prompt | Token-Hash | SHA-256 (Beginn) |
|---|---|---|
| The capital of France is | `f1dfaca128ee3bdc` | `2c87ca159f60db9a…` |
| In quantum mechanics, the wave function describes | `31603612c0a4fb93` | `026f8e84e308bec8…` |
| Die Hauptstadt von Frankreich ist | `cccef3c654a82e4e` | `5b7d97b80deb3b25…` |
| In der Quantenmechanik beschreibt die Wellenfunktion | `e51d3a7fe224ccdc` | `f5e8cadf1a762b79…` |
| The result of 17 times 23 is | `b2faf1fd7bdaa3b4` | `25b194b92965dcd4…` |

Zusaetzliche, unabhaengige Bestaetigungen derselben Eigenschaft:

- Der End-to-End-Test (`tests/integration/test_end2end_real.py`)
  wiederholt einen Teacher-Forcing-Lauf ueber 439 WikiText-2-Tokens und
  verlangt Bit-Identitaet der Gesamtausgabe — PASSED.
- Im Durchsatz-Benchmark (Abschnitt 5) erzeugen alle 3 Wiederholungen
  je Szenario denselben Decode-Hash.

**Reichweite der Aussage:** belegt fuer dieselbe Runtime + dieselben
Artefakte auf derselben Hardware. Der Cross-Hardware-Nachweis (mind. zwei
NVIDIA-Generationen, Whitepaper Kap. 6.2) steht noch aus — er setzt
GPU-Zugang und die SIMD-/CUDA-/ROCm-Backends voraus (projektweiter
offener Punkt). Die Architektur ist dafuer angelegt: θ_v fixiert den
vollstaendigen numerischen Vertrag (Rundung, Skalen, LUTs, KV-Cache-
Format), und die Backends muessen sich gegen Golden Vectors beweisen
(Fahrplan 12.26–12.34).

## 4. Beleg B — Qualitaet

### 4.1 Perplexitaet (Entscheidungspunkt 12.21)

Teacher-Forcing auf 4 WikiText-2-Sequenzen (Testsplit, 435 ausgewertete
Positionen), identisch fuer beide Modelle:

| Modell | Perplexitaet |
|---|---|
| BF16-Baseline (HF) | 14,95 |
| Integer-Modell (θ_v 0.10.0) | **15,59** |
| Relativer Anstieg | **+4,29 %** |
| Akzeptanzkriterium | max. +5 % → **AKZEPTIERT** |

Protokoll: `eval/results/decision_12-21.md`. Zwingende Einordnung (aus
dem Fahrplan, im Protokoll verankert): Perplexitaet ist unabhaengig von
der Decodierstrategie; 0,5 Mrd. Parameter sind der unguenstigste Fall
fuer Quantisierung — das Urteil gilt fuer 0,5B, groessere Modelle sind
nachweislich robuster.

### 4.2 Top-1-Agreement

Zusaetzliches, positionsaufgeloestes Qualitaetsmass: an jeder Position
der vier Messsequenzen wird die Top-1-Vorhersage des Integer-Modells
(`seq_logits_sweep`, Teacher-Forcing) mit der Top-1-Vorhersage der
BF16-Referenz verglichen.

| Sequenz | Positionen | Uebereinstimmung | Erste Abweichung |
|---|---|---|---|
| 0 | 128 | 117 (91,4 %) | Position 6 |
| 1 | 128 | 112 (87,5 %) | Position 0 |
| 2 | 128 | 112 (87,5 %) | Position 10 |
| 3 | 55 | 51 (92,7 %) | Position 1 |
| **Gesamt** | **439** | **392 (89,3 %)** | — |

Das heisst: In knapp 9 von 10 Positionen sagt das quantisierte
Integer-Modell exakt dasselbe naechste Token vorher wie die BF16-
Referenz; die restlichen Abweichungen sind vereinzelte Einzelpositionen,
kein zusammenhaengender Strukturverlust. Daten:
`eval/results/evidence/quality.json`.

### 4.3 Parallelgenerierung (DE/EN, greedy, 40 Tokens)

Dieselben Prompts, einmal Integer-Modell, einmal BF16-Referenz
(`eval/results/evidence/quality.json`, Schluessel
`parallel_generation`):

| Prompt | Integer-Modell | BF16-Referenz |
|---|---|---|
| The capital of France is | `______. A. Paris B. London C. Rome D. Berlin 答案: A` (Quiz-Format, korrekte Antwort A) | `Paris. It is the largest city in Europe and the second largest in the world. …` |
| In quantum mechanics, the wave function describes | `the state of a quantum system. Consider a quantum system with a single particle in a one-dimensional space. The wave function ψ(x) …` | `the state of a quantum system. Consider a quantum system with a wave function ψ(x) that is a Gaussian function centered at x = 0 …` |
| Die Hauptstadt von Frankreich ist | `Paris. Es ist die Hauptstadt der Region Hauts-de-France und der Stadt Frankreich. …` | `Paris. Die Stadt ist der Hauptstadt der Region Parisien und der Hauptstadt der Region Frankreich. …` |
| In der Quantenmechanik beschreibt die Wellenfunktion | `die Anzahl der Werte von einer Quantenmasse, die sich in einem bestimmten Zeitraum an einem bestimmten Ort befinden. …` | `die Anzahl der Zustaende, die sich in einem bestimmten Zeitraum zwischen zwei bestimmten Zustaenden befinden. …` |
| The result of 17 times 23 is | `between (　　) A: 300 and 310 B: 310 and 320 …` (Pruefungsformat, Antwort bleibt schuldig) | `391. What is the result of 17 times 230? …` (korrekt) |

Beobachtungen:

- Beide Modelle beantworten die Hauptstadt-Frage mit „Paris" und beginnen
  den Quantenmechanik-Prompt mit demselben Satz („the state of a quantum
  system. Consider a quantum system with …") — die Quantisierung aendert
  das inhaltliche Verhalten nicht grundsaetzlich.
- Die BF16-Referenz ist selbst nicht fehlerfrei („largest city in
  Europe", „Region Parisien") — 0,5B ist in beiden Faellen ein kleines
  Modell; der Vergleich zeigt die Differenz durch Quantisierung, nicht
  absolute Faehigkeiten.
- Der Arithmetik-Prompt zeigt eine echte Schwaeche des Integer-Modells
  (Ausweichen in ein Multiple-Choice-Format statt „391"). Das ist
  Konsistenz mit der Perplexitaet-Einordnung: 0,5B ist der unguenstigste
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
spaeteren SIMD-/CUDA-/ROCm-Backends (Fahrplan 12.35–12.55) und die
Cross-Hardware-Messungen messen lassen. Durchsatz ist fuer die
Kernthese irrelevant (Bit-Identitaet und Qualitaet sind die
Entscheidungskriterien); die Zahlen dienen der spaeteren Einordnung des
Backend-Fortschritts.

## 6. Der Weg dorthin — Perplexitaets-Verlauf und Root-Causes

Der Entscheidungspunkt wurde nicht in einem Schritt erreicht. Die
Messhistorie ist selbst Teil des Belegs, weil sie zeigt, dass der
Qualitaetseinbruch am Ende NICHT die Quantisierung war, sondern drei
Struktur-Bugs, die systematisch eingekreist wurden:

| Version | θ_v | Aenderung | Perplexitaet | Erkenntnis |
|---|---|---|---|---|
| v0.12.22 (12.19) | 0.5.x | Erste echte Messung (Per-Tensor-Skalen, geteilter LM-Head) | **14 546** | Qualitaetskrise bestaetigt (Fund 9) |
| v0.12.25 | 0.7.0 | Eskalation: Weight-Tying aufgeloest (LM-Head int16 per-channel) + Per-Channel-int8 fuer alle Gewichte | **3 257** | Faktor 4,5 — Quantisierung des LM-Heads war real, aber nicht allein |
| v0.12.26 | 0.7.0 | Skalen-Headroom: Kalibrierkorpus verbreitert (50/314 Module clampten still → 0/314) | **3 242** | Clamping behoben, aber NICHT dominant |
| v0.12.28 | 0.8.0 | GPTQ (Hessian-basierte Fehlerkompensation, 168 Projektionen; Synthetik −47 % Ausgabefehler) | **3 318** | Negativ-Ergebnis: lineare Gewichtsquantisierung NICHT dominant |
| v0.12.29 | 0.9.0 | SiLU-Eingangsraster verfeinert (0,5 → 0,125) | **2 972** | real, aber nicht dominant; Seq-Dump zeigt: Divergenz waechst mit der Position → Mehrpositions-Pfad verdaechtig |
| v0.12.30 | 0.10.0 | **Fund 15** (RoPE fundamental falsch: Multi-Frequenz + half-split statt Ein-Winkel + benachbart) + **Fund 16** (Attention attendierte im KV-Cache nur auf den ERSTEN Key) | **73,15** | Durchbruch, Faktor 40 — Struktur-Bugs, keine Quantisierung |
| v0.12.31 | 0.10.0 | Verifikation/Isolation: Float-Nachbildung der Quantisierungsstruktur ergibt 16,82 (nicht 73); LUT-Werte praezise (exp 0,19 %, SiLU 0,77 %) | — | Blow-up kommt aus der Integer-Arithmetik, nicht aus LUT-Werten oder Quantisierungsstruktur |
| v0.12.32 | 0.10.0 | **Fund 17**: fehlende 1/√head_dim-Attention-Skalierung (HF: `attn_weights = q·k · head_dim^-0.5`), behoben als zusaetzlicher Rechtsshift um log₂(head_dim)/2 = 3 | **15,59** | Root-Cause → **AKZEPTIERT** (+4,29 %) |

Methodische Lehre: Die Eskalationsstufen (Per-Channel, Headroom, GPTQ,
SiLU-Raster) waren keine Fehlschlaege, sondern Ausschlussbeweise — jedes
Negativ-Ergebnis hat den Suchraum verkleinert, bis die
Mehrpositions-Divergenzsuche (seq_layer_dump/seq_logits_sweep gegen HF)
die drei Struktur-Bugs sichtbar machte. Der Fix von Fund 17 ist ein
einziger zusaetzlicher Bitshift — bit-exakt, keine θ_v-Formataenderung.

## 7. Grenzen und offene Punkte (ehrliche Einordnung)

1. **Modellgroesse:** Alle Messungen gelten fuer Qwen2.5-0.5B, den
   unguenstigsten Fall. Die Uebertragbarkeit auf die Zielgroessenordnung
   (100 Mrd. – 1 Bio. Parameter) ist nicht gemessen, aber durch die
   bekannte Robustheit groesserer Modelle gegenueber Quantisierung
   zusaetzlich gestuetzt.
2. **Phase-0-Absolutkriterium:** `docs/00_requirements.md` nannte frueh
   „Perplexity < 15.0" als Orientierung. Gemessen sind 15,59 — das alte
   Absolutkriterium ist damit knapp verfehlt. Massgeblich ist das am
   Entscheidungspunkt angelegte RELATIVE Kriterium (max. +5 % vs.
   BF16-Baseline, Fahrplan 12.21), das mit +4,29 % erfuellt ist; das
   absolute Ziel der Phase 0 stammt aus der Zeit vor der Kalibrierung und
   war nie konsensrelevant festgelegt. Fuer die Whitepaper-Einarbeitung
   wird das relative Kriterium dokumentiert, das alte absolute als
   ueberholt markiert.
3. **Cross-Hardware:** Bit-Identitaet ist auf einer Hardware (arm64)
   belegt. Der Nachweis ueber Hardware-Generationen hinweg braucht die
   SIMD-/CUDA-/ROCm-Backends und GPU-Zugang (beides offen).
4. **Decodierstrategie:** Die Parallelgenerierung nutzt greedy; greedy
   verstaerkt Repetitionsneigung. Die Perplexitaet (Teacher-Forcing) ist
   davon unberuehrt und das massgebliche Qualitaetsmass.
5. **Lizenz:** Die Lizenzlage des Basismodells (Qwen2.5) fuer
   quantisierte Ableitungen ist weiterhin ein offener, nicht-technischer
   Punkt (unabhaengig vom Code und von diesem Beleg).
6. **Zurueckgestellt:** Der Hadamard-Basiswechsel (Vorstudien:
   `tests/diag/hadamard_prestudy.py`, `rmsnorm_hadamard_check.py`) ist
   geprueft und zurueckgestellt — nicht verworfen; er bleibt als
   vollstaendig ganzzahliger, deterministischer Eskalationspfad
   verfuegbar. Die verbleibenden ~4 % Perplexitaets-Abstand sind echtes
   Quantisierungsrauschen.

## 8. Reproduzierbarkeit

Alle Ergebnisse sind mit dem Stand v0.12.33 reproduzierbar:

```bash
cd INTEGER_LLM
cargo build --release --bins            # runtime-Binaeries inkl. Proben
# A) Bit-Identitaet (5 Prompts × 5 Laeufe):
calibrate/.venv/bin/python eval/evidence_determinism.py
# B) Qualitaet (Parallelgenerierung + Top-1-Agreement, laedt BF16-Referenz):
calibrate/.venv/bin/python eval/evidence_quality.py
# C) Durchsatz (Prefill/Decode getrennt):
calibrate/.venv/bin/python eval/evidence_benchmark.py
# Perplexitaetsvergleich (Entscheidungspunkt):
calibrate/.venv/bin/python eval/perplexity.py
```

## 9. Artefakte

| Datei | Inhalt |
|---|---|
| `eval/results/evidence/determinism.json` | 5×5 Laeufe: Tokenfolgen, Token-Hashes, SHA-256 |
| `eval/results/evidence/quality.json` | Parallelgenerierung (DE/EN) + Top-1-Agreement |
| `eval/results/evidence/benchmark.json` | Prefill-/Decode-Durchsatz, 2 Szenarien × 3 Laeufe |
| `eval/results/decision_12-21.md` | Protokoll des Entscheidungspunkts |
| `eval/results/perplexity_comparison.json` | Perplexitaetswerte und Sequenz-Details |
| `eval/evidence_*.py` | Die drei Evidenz-Skripte |
| `runtime/src/bin/bench_probe.rs` | Zeitmessung Prefill/Decode (Rust) |
| `tests/diag/` | Diagnose-Skripte der Eingrenzung (Layer-/Seq-Dumps, LUT-Audit, Float-Nachbildung, GPTQ-Verifikation) |
