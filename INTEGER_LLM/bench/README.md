# bench — Durchsatz und Ausgabequalität

Zwei Messungen mit verschiedenen Fragen. Sie werden hier bewusst
getrennt gehalten, weil sie verschiedene Dinge bedeuten und
unterschiedlich zu lesen sind.

| Skript | Frage | Zielwert? |
|---|---|---|
| [`run.py`](run.py) | Wie schnell ist der Integerpfad, je Backend und gegen Gleitkomma? | nein — Bestandsaufnahme |
| [`qualitativ.py`](qualitativ.py) | Wie nah liegt die Ausgabe an der Gleitkomma-Referenz? | nein — Gütezahl |

Der **einzige** Zielwert in diesem Verzeichnis ist der Determinismus:
Wiederholte Läufe müssen bitgleich sein, und alle Backends müssen
dasselbe rechnen. Alles andere sind Kennzahlen, keine Kriterien.

## `run.py` — Durchsatz (Fahrplan 12.64/12.65)

```bash
python3 bench/run.py --backends reference,cpu-simd --no-fp
./calibrate/.venv/bin/python bench/run.py            # mit Gleitkomma-Vergleich
INTEGER_LLM_MODEL=qwen2.5-7b ./calibrate/.venv/bin/python bench/run.py
```

Der Gleitkomma-Vergleich braucht `torch`/`transformers`, also die
Kalibrier-Umgebung. Ohne sie läuft die Ganzzahl-Messung durch und der
Vergleich wird mit Begründung übersprungen.

### Der Benchmark prüft Bitgleichheit, bevor er Zahlen zeigt

`bench_probe` gibt neben den Zeiten einen `decode_hash` aus. `run.py`
prüft, dass **alle** Backends denselben Hash liefern, und bricht sonst
mit Fehlercode ab.

Das ist keine Vorsicht, sondern die Bedingung dafür, dass die Tabelle
überhaupt etwas bedeutet: Ein Backend, das schneller ist und etwas
anderes rechnet, ist kein schnelleres Backend — es ist ein zweites
Modell. In einem Netz mit Bitgleichheits-Konsens (Whitepaper Kap. 6.2)
wäre ein Miner, der es einsetzt, beim Redundanzvergleich auffällig und
würde geslasht.

## Messwerte (2026-08-20, arm64 / Darwin, θ_v 0.17.0)

| Modell | Artefakt | Backend | Prefill | Decode |
|---|---|---|---|---|
| Qwen2.5-0,5B | 0,77 GB | reference | 19,31 tok/s | 18,24 tok/s |
| Qwen2.5-0,5B | 0,77 GB | **cpu-simd** | 24,00 tok/s | **23,23 tok/s** |
| Qwen2.5-0,5B | — | bf16 (HF) | 14,54 tok/s | 66,12 tok/s |
| Qwen2.5-7B | 8,72 GB | reference | 0,81 tok/s | 1,40 tok/s |
| Qwen2.5-7B | 8,72 GB | **cpu-simd** | 1,58 tok/s | **2,00 tok/s** |
| Qwen2.5-7B | — | bf16 (HF) | 1,05 tok/s | 9,46 tok/s |

**`cpu-simd` bringt +27 % (0,5B) und +43 % (7B)** — bei identischem
`decode_hash` und 30/30 Konformitätsvektoren unter beiden Backends.

**Zur Lesart der Streuung.** Gegenüber der vorherigen Messung (θ_v 0.15.0)
schwanken die Werte um bis zu 4 % in beide Richtungen — 0,5B `cpu-simd`
lag dort bei 24,26 statt 23,23 tok/s, 7B `reference` bei 1,35 statt 1,40.
Das ist Lauf-zu-Lauf-Streuung auf einer nicht isolierten Maschine, keine
Regression: Die Auflösungserhöhungen in θ_v 0.16.0/0.17.0 kosten nichts
Messbares. Wer daraus einen Trend lesen will, braucht mehr als zwei Läufe.

**Eine Durchsatzmessung unter Nebenlast ist keine Messung.** Ein
Zwischenstand dieser Tabelle wies 19,88 tok/s für 0,5B `cpu-simd` aus —
gemessen, während parallel eine 7B-Kalibrierung lief. Die Zahl wurde
verworfen, nicht eingetragen.

**Bis zum 2026-08-20 brachte es nichts, und der Grund war lehrreich.**
Das Operationsprofil (`kernels/src/bin/op_profile.rs`) hat gemessen,
wohin die Zeit geht:

| Operation | Anteil |
|---|---|
| `linear_w8a16` (Layer + LM-Head) | **99,4 %** |
| rmsnorm | 0,4 % |
| rope + softmax | 0,15 % |

Vektorisiert waren Softmax, RoPE und Attention — zusammen 0,15 %.
`linear_w8a16` und `rmsnorm` delegierten an die Referenz. Es war die
falsche Operation optimiert, und niemand hatte nachgesehen. Seit
`kernels/src/dot.rs` ist das Skalarprodukt vektorisiert.

**Der Abstand zu bf16 bleibt** (Faktor 0,37 im Decode). Der Integerpfad
rechnet weiterhin ohne Blocking, ohne Prefetch und mit
`Vec<Vec<i8>>`-Gewichten, also einer Heap-Allokation je Zeile. Das ist
der nächste offensichtliche Hebel und größer als alles, was SIMD noch
hergibt.

### Skalierung: zwei Punkte, mehr nicht

Von 0,5B auf 7B wächst das Artefakt um Faktor **11,3**, der Durchsatz
fällt um Faktor **11,6** (cpu-simd: 23,23 → 2,00 tok/s). Grob linear in der Modellgröße, mit leichtem
Aufschlag — für eine speicherbandbreitengebundene
Referenzimplementierung das Erwartbare.

**Zwei Punkte sind keine Kurve.** Die Zielgrößenordnung des Projekts
liegt um Größenordnungen darüber, und ob der Zusammenhang dort noch
linear ist, sagt diese Messung nicht. Sie sagt nur: bis 7B gibt es keine
Überraschung.

`run.py` ist deshalb modellagnostisch gebaut — Modellwahl über
`INTEGER_LLM_MODEL`, Pfadauflösung über dieselbe Quelle wie Kalibrierung
und Perplexitätsmessung (`calibrate/src/model_configs.py`). Auf dem
nächstgrößeren Dense-Modell läuft es unverändert. Die Artefaktgröße wird
zu jedem Lauf mitgeschrieben, weil eine Tokens/s-Zahl ohne sie nicht
einordenbar ist.

## `qualitativ.py` — Ausgabequalität

```bash
INTEGER_LLM_MODEL=qwen2.5-7b python3 bench/qualitativ.py [max_tokens]
```

Misst zwei Größen getrennt, und die Trennung ist der Punkt:

- **Determinismus** (Zielwert 8/8): Zwei Läufe desselben Prompts müssen
  bitgleich sein. Das ist die Konsensbedingung.
- **Nähe zur Gleitkomma-Referenz** (Gütezahl, kein Zielwert): Wie oft
  entsteht derselbe Text? 8/8 wäre **kein** Erfolg, sondern ein Hinweis
  darauf, dass die Quantisierung wirkungslos ist.

Stand 2026-08-19: Determinismus 8/8 auf beiden Modellen; identische
Generierungen 3/8 (0,5B) und 5/8 (7B).

## Ergebnisdateien

`results/<modell>_<architektur>.json`, eine Datei je Modell und
Maschine — damit ein 7B-Lauf die 0,5B-Messung nicht überschreibt und
Läufe verschiedener Hardware nebeneinander bestehen bleiben. Enthalten
sind alle Rohwerte, die Artefaktgröße und der `decode_hash`.

Die Dateien sind **nicht** eingecheckt (Artefakte und Messergebnisse
sind gitignored); die Tabelle oben ist der eingecheckte Stand.
