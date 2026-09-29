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
├── artefaktvergleich.py    # mehrere Artefakte auf denselben Folgen; --bitgleich
├── gsm8k.py                # Aufgaben mit Denken: GSM8K-Teilmenge, gierig
├── paket_perplexitaet.py   # Gleitkomma-Referenz eines gedrehten, gepackten Pakets
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
θ_v weiter, während `results/decision_12-21*.md` bei jedem Lauf neu
geschrieben wird. **Wer eine Messung an zwei Orten führt, pflegt
irgendwann nur noch einen.**

⛔️ **Dasselbe noch einmal, gefunden am 2026-09-11:**
`results/verification_report.md` lag seit dem 2026-08-11 hier, mit θ_v
0.10.0, einem Modell, das es nicht mehr gibt, und einem Nachtrag im
eigenen Kopf, der seine Kernzahlen für überholt erklärte. **Ein
Dokument, dessen Kopf sagt, dass seine Zahlen überholt sind, gehört
nicht in den Baum**: Es sieht wie eine Auskunft aus und ist eine Falle.
Sein tragender Inhalt steht im Changelog von INTEGER_LLM, wo er
hingehört; die Datei ist entfernt.

## Die ganze Reihe auf einer Seite

`results/UEBERSICHT.md` stellt alle Modelle nebeneinander und entsteht
aus denselben JSON-Dateien wie die Einzelprotokolle:

```bash
python3 BENCHMARKS/Inferenz/uebersicht.py
```

⚑ **Die Einzelprotokolle bleiben.** Jedes ist der Beleg **seines**
Modells, mit Methode, Datensatz und Einordnung; sie zu einem
zusammenzufassen hiesse, den Beleg zu verlieren. Was fehlte, war die
Übersicht daneben, und die wird erzeugt statt gepflegt.

⚠️ **Fehlt die Messung eines Modells, steht das in der Tabelle**, und
es wird **nicht** auf eine andere Datei zurückgefallen. Die erste
Fassung dieses Erzeugers tat genau das und zeigte bei zwei Modellen die
Zahlen eines dritten.

## Aufruf

📌 **Aus der Wurzel und nicht aus `INTEGER_LLM/`** (berichtigt
2026-09-11, Fund 339). Hier stand `cd INTEGER_LLM` und darunter
`eval/baseline.py`; seit dem Umzug am 2026-09-07 liegen die Werkzeuge
eine Ebene hoeher, und der Block lief ins Leere.

```bash
cd INTEGER_LLM && cargo build --release --bins && cd ..   # Proben bauen
V=INTEGER_LLM/calibrate/.venv/bin/python
$V BENCHMARKS/Inferenz/baseline.py        # BF16-Baseline (einmalig je Modell)
$V BENCHMARKS/Inferenz/perplexity.py      # Entscheidungspunkt
$V BENCHMARKS/Inferenz/evidence_determinism.py
$V BENCHMARKS/Inferenz/evidence_quality.py
$V BENCHMARKS/Inferenz/evidence_benchmark.py
```

Das Modell waehlt `INTEGER_LLM_MODEL`, Vorgabe `myelith-0.6b`.

### Neue Artefakte vergleichen (seit 2026-09-28)

Für ein Artefakt, das aus einem anderen entsteht (gepackt, ternär,
nachtrainiert), oder für zwei Modelle nebeneinander:

```bash
V=INTEGER_LLM/calibrate/.venv/bin/python3
# Perplexität beliebig vieler Artefakte auf denselben Folgen
INTEGER_LLM_MODEL=myelith-8b $V -u BENCHMARKS/Inferenz/artefaktvergleich.py 4 128 \
  INTEGER_LLM/artifacts/<a> INTEGER_LLM/artifacts/<b>
# Rechnen zwei Speicherformen dieselbe ganze Zahl? Rückgabewert 0 nur bei IDENTISCH
INTEGER_LLM_MODEL=myelith-8b $V -u BENCHMARKS/Inferenz/artefaktvergleich.py 4 128 \
  INTEGER_LLM/artifacts/<quelle> INTEGER_LLM/artifacts/<gepackt> --bitgleich
# Aufgaben mit Denken; die Chatvorlage kommt aus dem Originalordner
$V -u BENCHMARKS/Inferenz/gsm8k.py MODELS/llm/<original> INTEGER_LLM/artifacts/<a> 50 512 <ausgabe.jsonl>
# Gleitkomma-Referenz eines Pakets, das sich nicht als BF16 entpacken lässt
INTEGER_LLM_MODEL=<modell> $V -u BENCHMARKS/Inferenz/paket_perplexitaet.py MODELS/llm/<paket> 4 128
```

⚑ **`INTEGER_LLM_MODEL` wählt den Tokenizer der Folgenauswahl.** Alle
verglichenen Artefakte müssen ihn teilen, und eine Gleitkomma-Referenz
gehört auf dieselben Folgen wie die ganzzahlige Messung.

⚠️ **GSM8K an 50 Aufgaben:** Zwei, drei Punkte Unterschied sind Rauschen.
Der Testsplit wird beim ersten Lauf nach `datasets/` geladen und gegen
eine Prüfsumme gehalten; eine andere Fassung bricht ab, weil ihre Zahlen
mit keiner früheren vergleichbar wären.

Erprobt am 2026-09-28: `--bitgleich` am 0,6B gegen sich selbst 128/128
gleich, gegen das 4B verschieden (Rückgabewert 1); GSM8K am 0,6B mit zwei
Aufgaben 2/2. `paket_perplexitaet.py` ist die verallgemeinerte Fassung des
Skripts, das am 27B die Referenz 11,18 lieferte; in dieser Form noch nicht
erneut gelaufen, weil neben einem Training kein zweiter großer Lauf
starten soll.

Steuerung der Sequenz-Parameter (Baseline und E2E-Test):
`E2E_SEQUENCES` (Standard 4), `E2E_SEQ_LEN` (Standard 128).
