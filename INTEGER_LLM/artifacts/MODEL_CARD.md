# Modellkarte — Myelith Integer-Artefakte

**Stand:** 2026-08-20 · **θ_v:** 0.15.0 · **Crates:** v0.14.0

Diese Karte beschreibt die **quantisierten Artefakte** unter
`artifacts/`, nicht die Basismodelle. Ein Artefakt ist das Ergebnis
einer Kalibrierung: dieselben Gewichte, überführt in ein
Ganzzahlformat, dessen Ausführung auf jeder Hardware bitgleich ist.

Sie ist als **Formular** angelegt, nicht als Prosa. Jede neue Modellgröße
bekommt eine Spalte; was nicht gemessen ist, bleibt ausdrücklich leer.
Die Zielgrößenordnung des Projekts liegt deutlich über den heute
kalibrierten Modellen — die Karte soll beim Hochskalieren mitwachsen,
ohne umgeschrieben zu werden.

## 1. Artefakte im Überblick

| | Qwen2.5-0,5B | Qwen2.5-7B |
|---|---|---|
| Basismodell | `Qwen/Qwen2.5-0.5B` | `Qwen/Qwen2.5-7B` |
| Lizenz Basismodell | Apache 2.0 | Apache 2.0 |
| Revision | siehe `models/README.md` | `d1497293` |
| Layer | 24 | 28 |
| Artefaktgröße | 0,78 GB | 8,72 GB |
| Skalen-Einträge | 314 | 366 |
| Tensoren | — | 340 |
| Weight-Tying | aufgelöst (LM-Head eigenständig) | aufgelöst |
| Verifiziert für Export | ja | ja (`verified`-Gate, v0.12.43) |

## 2. Verfahren

| | Wert | Festgelegt in |
|---|---|---|
| Gewichte | int8, **per Kanal**, symmetrisch | `theta_v/spec.json` |
| Aktivierungen | int16 | `theta_v/spec.json` |
| Bias | int16 **je Element** | Fund 23 |
| Skalen | ausschließlich Zweierpotenzen | `numeric.scales.mode = power_of_two` |
| Residualstrom | Skala **je Kanal** (seit θ_v 0.11.0) | Fund 20 |
| Division | ausschließlich arithmetischer Rechtsshift | Whitepaper Kap. 6.2, Anhang B.5.4 |
| Überlauf | `explicit_clamp_only`, `wrap = false` | `theta_v/spec.json` |
| Nichtlinearitäten | LUT (SiLU-Eingangsraster **1/64**, Ausgang **1/256**) | θ_v 0.15.0 |
| Softmax | exp-LUT Eingangsraster **1/256**, Ausgang **1/16384**, Wahrscheinlichkeiten **1/16384** | θ_v 0.16.0 |
| Residual-Addition | Akkumulation auf der gröberen Segmentskala je Kanal, Summe in i64, **eine** Rundung | θ_v 0.17.0 |
| Sampling | greedy, deterministisch; `tie_breaking = lowest_index` | `theta_v/spec.json` |
| GPTQ | **standardmäßig aus** (`INTEGER_LLM_GPTQ=1` für die Auslieferung) | 2026-08-20, siehe §6 |

**Warum ausschließlich Zweierpotenzen:** Eine Skala, die keine
Zweierpotenz ist, verlangt eine Division. Division ist im Integerpfad
als arithmetischer Rechtsshift definiert; jede andere Form wäre entweder
Gleitkomma oder eine Rundung mit implementierungsabhängigem Verhalten.
Geprüft durch `tests/audit/test_scales.py` über alle 314 Skalen.

## 3. Kalibrierungsdaten

| | Wert |
|---|---|
| Korpus | WikiText-2 (Testsplit) |
| Auswahl | deterministisch, gemeinsame Sequenzauswahl für Kalibrierung, Baseline und Messung (`eval/wikitext_common.py`) |
| Aktivierungsskalen | breite Stichprobe von 64 Sequenzen à ≤128 Token |
| Ausgesparte Sequenzen | die vier Mess-Sequenzen des Entscheidungspunkts — keine Kalibrierung auf den Testdaten |
| Headroom per Kanal | 0 Bit (`PER_CHANNEL_HEADROOM_BITS`, Fund 21 als dokumentiertes Negativergebnis) |

**Bekannte Grenze:** Auf ungesehenen Sequenzen clippen bei 7B **6,24 %
der Kanäle**, bis Faktor 4,53. Das ist gemessen, nicht geschätzt, und
noch nicht behoben.

## 4. Qualität

| | 0,5B | 7B |
|---|---|---|
| **Determinismus** (Zielwert 8/8) | **8/8** ✓ | **8/8** ✓ |
| Perplexität Integer / BF16 | 15,27 / 14,95 | **8,78** / 8,68 |
| Abstand | **+2,11 %** (Kriterium ≤5 % erfüllt) | **+1,14 %** (Kriterium ≤5 % erfüllt) |
| Identische Generierungen (Gütezahl) | 3/8 | 5/8 |
| Deckungsgleiche Token (Gütezahl) | 65,0 % | 73,8 % |

Perplexität auf WikiText-2 mit Teacher-Forcing, für beide Pfade auf
identischen Sequenzen; niedriger ist besser.

**7B ist qualitativ besser, obwohl sein relativer Abstand größer ist.**
Das ist kein Widerspruch: 8,68 absolut ist ein erheblich stärkeres Modell
als 14,95, und der Prozentabstand sagt nichts über die absolute
Textqualität.

**Die Gütezahlen sind keine Zielwerte.** 8/8 identische Generierungen
wären kein Erfolg, sondern ein Hinweis darauf, dass die Quantisierung
wirkungslos ist.

## 5. Durchsatz

| Modell | Backend | Prefill | Decode | bf16 (Decode) |
|---|---|---|---|---|
| 0,5B | reference | 18,85 tok/s | 18,58 tok/s | 66,19 tok/s |
| 0,5B | **cpu-simd** | 23,46 tok/s | **24,26 tok/s** | — |
| 7B | reference | 0,90 tok/s | 1,35 tok/s | nicht gemessen |
| 7B | **cpu-simd** | 1,60 tok/s | **2,03 tok/s** | — |

arm64 / Darwin, 2026-08-20. Details und Einordnung in
[`bench/README.md`](../bench/README.md). `cpu-simd` ist seit dem
vektorisierten Skalarprodukt (`kernels/src/dot.rs`) um 31 % (0,5B) bzw.
50 % (7B) schneller — bei identischem `decode_hash` und 30/30
Konformitätsvektoren unter beiden Backends. Der verbleibende Abstand zu
bf16 liegt an fehlendem Blocking und der `Vec<Vec<i8>>`-Gewichtsablage
(eine Heap-Allokation je Zeile).

## 6. Werkzeugversionen

| | Wert |
|---|---|
| θ_v (Ausführungsspezifikation) | 0.15.0 |
| Crates (`kernels`/`runtime`/`pipeline`) | v0.14.0 |
| Kalibrierung | `calibrate/`, Python 3.12 (uv-venv) |
| Referenz-Framework | PyTorch 2.13.0, HuggingFace `transformers` |
| Konformitätsvektoren | 30, eingefroren unter `conformance/vectors/` |

**θ_v-Bindung:** Jedes Artefakt trägt den kanonischen θ_v-Hash. Der
Loader lehnt Artefakte ab, deren Hash nicht zur geladenen Spezifikation
passt — eine ältere Kalibrierung läuft nicht stillschweigend unter neuen
Regeln.

**GPTQ läuft standardmäßig nicht mit** (seit 2026-08-20). Es wurde
implementiert und gemessen: Der lineare Ausgabefehler sank um 47 % in der
Synthetik, die Perplexität verbesserte sich **nicht** (3 242 → 3 318).
Damit war die lineare Gewichtsquantisierung als dominante Fehlerquelle
ausgeschlossen — ein Zwischenergebnis, das die Suche verkürzt hat, aber
keinen Nutzen im Auslieferungspfad.

Bei 7B kostet es rund zweieinhalb Stunden gegenüber zwanzig Minuten ohne.
Für Messläufe ist das reine Rechenzeit; eingeschaltet wird es mit
`INTEGER_LLM_GPTQ=1` für die abschließende Artefakt-Erstellung. **Welche
Einstellung ein Artefakt trägt, gehört zu seiner Beschreibung** — zwei
Artefakte sind nur vergleichbar, wenn sie darin übereinstimmen.

## 7. Was diese Artefakte **nicht** belegen

- **Bitgleichheit über heterogene Hardware ist nicht gemessen.** Alle
  Läufe fanden auf aarch64 statt. Die Eigenschaft ist aus dem Format
  begründet (Ganzzahladdition ist assoziativ), nicht empirisch belegt.
  Das ist der wichtigste offene Nachweis des Projekts (Kritikpunkt K1).
- **Die Zielgrößenordnung ist ungemessen.** 7B ist das größte kalibrierte
  Modell. Größere Modelle gelten als robuster gegenüber Quantisierung —
  das ist hier Annahme, nicht Befund (Kritikpunkt K6). Ein Bezug
  weiterer Größen ist zusätzlich durch die noch offene **Lizenzprüfung
  je Variante** blockiert (Kap. 10.1 / ETHICS G7 verlangen Apache 2.0
  oder MIT; das gilt nicht automatisch für jede Größe einer Reihe).
- **Das 5-%-Kriterium ist bei 7B nicht erfüllt.** Es fehlen 2,49
  Prozentpunkte. Eskalationspfade stehen in `README/Fahrplan-v3.md`, 4.5.
- **Kein Training.** Diese Artefakte sind Inferenz-Artefakte. Ob das
  Quantisierungsschema im Rückwärtspass trägt, ist ungemessen und der
  einzige offene Fahrplanpunkt von TRAINING.

## 8. Reproduktion

```bash
# Kalibrierung (erzeugt artifacts/<modell>/)
INTEGER_LLM_MODEL=qwen2.5-0.5b ./calibrate/.venv/bin/python calibrate/src/main.py

# Qualität
./calibrate/.venv/bin/python eval/perplexity.py
python3 bench/qualitativ.py

# Durchsatz
./calibrate/.venv/bin/python bench/run.py

# Konformität — je Backend, das zertifiziert werden soll
bash conformance/run.sh reference
bash conformance/run.sh cpu-simd
```

Die Artefakte selbst sind **nicht** eingecheckt (Größe); sie entstehen
reproduzierbar aus Basismodell und Kalibrierung. Maßgeblich für die
Reproduzierbarkeit ist der θ_v-Hash, nicht die Datei.
