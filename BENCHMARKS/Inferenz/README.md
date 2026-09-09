# Inferenzmessung

Qualitätsmessung des Integer-Modells: Gleitkomma-Vergleichsstand,
Perplexitätsberechnung, Evidenzläufe und die dafür benötigten
Datensätze. Eine Qualitätsmessung ist kein Integrationstest (siehe
`INTEGER_LLM/tests/`) und liegt deshalb in einem eigenen Verzeichnis.

⚑ **Umgezogen am 2026-09-07** von `INTEGER_LLM/eval/`, damit alle
Messungen dieses Projekts an einem Ort liegen. Wer ältere Laufberichte
liest, findet dort noch den alten Pfad; datierte Berichte werden nicht
nachträglich umgeschrieben.

⚑ **Zwei Wurzeln, seit dem Umzug getrennt benannt.** Die Skripte
brauchen zweierlei: `LLM` zeigt auf `INTEGER_LLM/` für Artefakte, Tests
und Kalibrierung, `HIER` auf dieses Verzeichnis für Datensätze und
Ergebnisse. Vorher hiess beides `REPO` und war dasselbe; **wer das
übersieht, misst gegen ein Verzeichnis, das es nicht gibt.**

⚑ **Die Regel aus der Ebene darüber gilt auch hier:** Ein Modell, das
nach einer Änderung besser misst, gehört gegen einen **Rauschnullpunkt**
gehalten. Eine Störung mit Requantisierung verbessert ein quantisiertes
Modell auch dann, wenn niemand etwas gelernt hat.

## Struktur

```
BENCHMARKS/Inferenz/
├── README.md
├── wikitext_common.py      # EINZIGE Quelle der Messsequenzen (alle Messungen)
├── baseline.py             # BF16-Baseline (HF), Teacher-Forcing
├── perplexity.py           # Perplexitätsvergleich + Protokoll (12.21)
├── evidence_determinism.py # Evidenz: Bit-Identität (5 Prompts × 5 Läufe)
├── evidence_quality.py     # Evidenz: Parallelgenerierung + Top-1-Agreement
├── evidence_benchmark.py   # Evidenz: Durchsatz Prefill/Decode (bench_probe)
├── datasets/               # WikiText-2-Cache (nicht versioniert)
└── results/                # Messergebnisse (versioniert)
    ├── baseline_wikitext2.json
    ├── decision_12-21.md
    ├── perplexity_comparison.json
    └── evidence/           # Ergebnisse der drei Evidenz-Läufe
```

## Messmethode (identisch für alle Vergleiche)

`wikitext_common.py` wählt deterministisch Sequenzen aus dem
WikiText-2-Testsplit aus (substantielle Zeilen, fester Stride,
Qwen-Tokenizer). Integer-E2E-Test, BF16-Baseline, Perplexitätsvergleich
und das Top-1-Agreement verwenden dieselbe Auswahl, denselben Tokenizer
und dieselbe Sequenzlänge; nur so ist der Vergleich aussagekräftig
(„identische Messmethode"). Gleitkomma darf nur im
Mess-/Referenzpfad verwendet werden (BF16-Baseline, Log-Softmax-
Auswertung der Proben), niemals im Integer-Inferenzpfad.

## Entscheidungspunkt 12.21: AKZEPTIERT

Perplexität Integer-Modell **15,59** vs. BF16-Baseline **14,95** =
**+4,29 %** (Kriterium: max. +5 % relativer Anstieg). Protokoll:
`results/decision_12-21.md`. Der zugehörige plastische Beleg
(Bit-Identität, Parallelgenerierung DE/EN, Top-1-Agreement,
Durchsatz-Basis) liegt unter `results/evidence/`.

⚑ **Eine zweite Zusammenfassung gibt es bewusst nicht mehr.** Sie stand
bis zum 2026-09-01 unter `../docs/` und führte die Zahlen eines älteren
θ_v weiter, während `results/decision_12-21.md` bei jedem Lauf neu
geschrieben wird. **Wer eine Messung an zwei Orten führt, pflegt
irgendwann nur noch einen.**

## Aufruf

```bash
cd INTEGER_LLM
cargo build --release --bins                       # Proben bauen
calibrate/.venv/bin/python eval/baseline.py        # BF16-Baseline (einmalig)
calibrate/.venv/bin/python eval/perplexity.py      # Entscheidungspunkt
calibrate/.venv/bin/python eval/evidence_determinism.py
calibrate/.venv/bin/python eval/evidence_quality.py
calibrate/.venv/bin/python eval/evidence_benchmark.py
```

Steuerung der Sequenz-Parameter (Baseline und E2E-Test):
`E2E_SEQUENCES` (Standard 4), `E2E_SEQ_LEN` (Standard 128).
