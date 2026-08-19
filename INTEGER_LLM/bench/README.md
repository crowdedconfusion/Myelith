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

## Messwerte (2026-08-19, arm64 / Darwin, Referenz-Backend)

| Modell | Artefakt | Prefill | Decode | Gleitkomma (bf16) |
|---|---|---|---|---|
| Qwen2.5-0,5B | 0,78 GB | 18,65 tok/s | **19,50 tok/s** | 67,93 tok/s |
| Qwen2.5-7B | 8,72 GB | 0,80 tok/s | **1,42 tok/s** | nicht gemessen |

**Einordnung, die zu den Zahlen gehört:**

- **Der Integerpfad ist auf 0,5B etwa 3,5× langsamer als bf16.** Das ist
  der heutige Stand einer Referenzimplementierung ohne
  Kernel-Optimierung, nicht die erreichbare Grenze. Die
  Gleitkomma-Referenz nutzt hochoptimierte, seit Jahren gepflegte
  Kernel; der Integerpfad rechnet Skalar-Schleifen.
- **`cpu-simd` (NEON) ist auf dieser Maschine nicht schneller** —
  18,66 gegen 19,50 tok/s im Decode, also eher minimal langsamer. Das
  SIMD-Backend deckt Softmax, RoPE und MLP ab; offenbar liegt der
  Engpass woanders. **Das ist ein Messergebnis, kein Fehler**, und es
  gehört sichtbar dokumentiert, statt in einer Fußnote zu verschwinden:
  Wer SIMD einschaltet und Beschleunigung erwartet, bekommt sie hier
  nicht. Wo die Zeit tatsächlich hingeht, ist offen und wäre eine
  eigene Messung (Profil je Operation).
- **CUDA und ROCm wurden nicht gemessen** — die Backends sind
  Delegations-Stubs, und auf dieser Maschine fehlt die Toolchain.
  `run.py` überspringt sie mit Begründung, statt eine Referenzmessung
  unter falschem Namen zu protokollieren.

### Skalierung: zwei Punkte, mehr nicht

Von 0,5B auf 7B wächst das Artefakt um Faktor **11,2**, der Durchsatz
fällt um Faktor **13,7**. Grob linear in der Modellgröße, mit leichtem
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
