# eval/

Ablageort für die Qualitätsmessung des Integer-Modells: Gleitkomma-
Baseline, Perplexitätsberechnung, Evidenz-Läufe und die dafür benötigten
Datensätze. Eine Qualitätsmessung ist kein Integrationstest (siehe
`tests/`) und gehört deshalb in ein eigenes Verzeichnis.

## Struktur

```
eval/
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
und dieselbe Sequenzlänge — nur so ist der Vergleich aussagekräftig
(„identische Messmethode"). Gleitkomma darf nur im
Mess-/Referenzpfad verwendet werden (BF16-Baseline, Log-Softmax-
Auswertung der Proben), niemals im Integer-Inferenzpfad.

## Entscheidungspunkt 12.21 — AKZEPTIERT

Perplexität Integer-Modell **15,59** vs. BF16-Baseline **14,95** =
**+4,29 %** (Kriterium: max. +5 % relativer Anstieg). Protokoll:
`results/decision_12-21.md`. Der zugehörige plastische Beleg
(Bit-Identität, Parallelgenerierung DE/EN, Top-1-Agreement 89,3 %,
Durchsatz-Basis) liegt unter `results/evidence/`; Zusammenfassung und
Einordnung in `../docs/02_empirischer_beleg_bit-exakte-inferenz.md`.

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
