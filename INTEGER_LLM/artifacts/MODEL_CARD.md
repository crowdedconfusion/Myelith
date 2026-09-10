# Modellkarte — Myelith Integer-Artefakte

**Stand:** 2026-09-10 · **θ_v:** 0.18.0 · **Crates:** kernels 0.49.0,
runtime 0.43.0, pipeline 0.15.0

Diese Karte beschreibt die **Myelith-Modelle**, also die quantisierten
Artefakte unter `artifacts/`, und nicht die Basismodelle. **Ein Artefakt
ist das Modell**, mit dem dieses Projekt rechnet: dieselben Gewichte,
überführt in ein Ganzzahlformat, dessen Ausführung auf jeder Hardware
bitgleich ist. Es trägt deshalb einen eigenen Namen.

⚠️ **Der Name nennt die Herkunft mit, und das ist keine Höflichkeit.**
„Myelith 4B" ist ein anderes Objekt als `Qwen/Qwen3-4B`: Es rechnet
anders, es ist anders lizenzpflichtig zu behandeln und es ist hier
gebaut worden. Die Basismodelle stehen unter Apache 2.0, und ein Name
ohne Herkunft wäre eine Verschleierung statt einer Unterscheidung.

⛑ **Die Artefakte aus Qwen2.5 sind aus dieser Karte herausgenommen**
(2026-09-10, Festlegung des Projektinhabers). Sie liegen weiter im
Repositorium und tragen die Prüfsammlungen, den Konformitätslauf und
die Vergleichsmessungen; **als ausgelieferte Modelle sind sie es
nicht.** Ihre Messwerte stehen in den Fundhistorien und im Changelog von
`README/README.md`, wo sie hingehören: als Historie und nicht als
Angebot.

Sie ist als **Formular** angelegt, nicht als Prosa. Jede neue Modellgröße
bekommt eine Spalte; was nicht gemessen ist, bleibt ausdrücklich leer.
Die Zielgrößenordnung des Projekts liegt deutlich über den heute
kalibrierten Modellen, und die Karte soll beim Hochskalieren mitwachsen,
ohne umgeschrieben zu werden.

## 1. Die Modelle im Überblick

| | Myelith 4B | Myelith 30B-A3B |
|---|---|---|
| Basismodell | `Qwen/Qwen3-4B` | `Qwen/Qwen3-30B-A3B` |
| Lizenz Basismodell | Apache 2.0 | Apache 2.0 |
| Lizenz Artefakt | PolyForm Shield License 1.0.0 | PolyForm Shield License 1.0.0 |
| Parameter | 4 Mrd. | 30,5 Mrd. gesamt, **3,04 Mrd. aktiv je Token** |
| Bauart | dicht | Expertengemisch, 128 Experten, Top-8 |
| Layer | 36 | 48 |
| Modellbreite | 2560 | 2048 |
| Köpfe (Abfrage / Schlüssel) | 32 / 8 | 32 / 4 |
| QK-Normierung | ja | ja |
| Weight-Tying | gebunden | aufgelöst (LM-Head eigenständig) |
| Wortschatz | 151 936 | 151 936 |
| Kontext | 2048 | 2048 |
| Artefaktgröße | 4,5 GB | 29 GB |

⚑ **Das Gemisch ist das Modell für die Zusage „jeder mit einem normalen
Rechner".** 128 Experten, von denen acht je Token feuern, rechnet wie
ein kleines Modell und antwortet wie ein großes; die Gewichte werden
speicherabgebildet, es muss also nicht alles gleichzeitig in den
Arbeitsspeicher.

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

**Bekannte Grenze, gemessen an einem dichten Modell mit 7 Mrd.
Parametern:** Auf ungesehenen Sequenzen clippen **6,24 % der Kanäle**,
bis Faktor 4,53. Das ist gemessen, nicht geschätzt, und noch nicht
behoben. ⚠️ **Für die Modelle dieser Karte ist es nicht erhoben**; die
Grenze steht hier, weil sie am Verfahren hängt und nicht am Modell.

## 4. Qualität

| | Myelith 4B | Myelith 30B-A3B |
|---|---|---|
| Abstand zur Gleitkomma-Referenz | **+1,64 %** | **kein messbarer Abstand** |
| Kriterium ≤ 5 % | erfüllt | erfüllt |
| Determinismus (Zielwert 8/8) | *nicht in dieser Form gemessen* | *nicht in dieser Form gemessen* |
| Identische Generierungen (Gütezahl) | *nicht gemessen* | *nicht gemessen* |
| Deckungsgleiche Token (Gütezahl) | *nicht gemessen* | *nicht gemessen* |

Perplexität auf WikiText-2 mit Teacher-Forcing, für beide Pfade auf
identischen Sequenzen; niedriger ist besser.

⚠️ **„Kein messbarer Abstand" heißt nicht „besser".** Der Punktschätzer
beim Gemisch liegt bei **−0,59 %**, der Standardfehler bei **1,66 %**,
und **2 von 4 Sequenzen sind schlechter**. Wer daraus liest, die
Quantisierung verbessere das Modell, liest Rauschen. Die belastbare
Aussage lautet: Der Abstand ist kleiner als die Streuung dieser Messung.

⛑ **Die drei leeren Zeilen sind leer, weil sie leer sind.** Determinismus
und die beiden Gütezahlen sind für die Artefakte aus Qwen2.5 erhoben
worden und für diese hier nicht. **Eine übertragene Zahl wäre eine
Behauptung über eine Messung, die niemand gemacht hat**, und diese Karte
ist ein Formular: Was nicht gemessen ist, bleibt ausdrücklich leer.

**Und die Gütezahlen sind ohnehin keine Zielwerte.** 8/8 identische
Generierungen wären kein Erfolg, sondern ein Hinweis darauf, dass die
Quantisierung wirkungslos ist. **Bitgleichheit des Ganzzahlpfades mit
sich selbst ist die Konsensbedingung; Nähe zur Gleitkomma-Referenz ist
eine Gütezahl ohne Zielwert.**

## 5. Durchsatz

| Modell | Backend | Prefill | Decode | bf16 (Decode) |
|---|---|---|---|---|
| Myelith 4B | reference | *nicht gemessen* | *nicht gemessen* | *nicht gemessen* |
| Myelith 4B | cpu-simd | *nicht gemessen* | *nicht gemessen* | entfällt |
| Myelith 30B-A3B | reference | *nicht gemessen* | *nicht gemessen* | *nicht gemessen* |
| Myelith 30B-A3B | cpu-simd | *nicht gemessen* | *nicht gemessen* | entfällt |

⛑ **Diese Tabelle stand bis zum 2026-09-10 gefüllt da, mit den Werten
der Artefakte aus Qwen2.5.** Sie sind mit jenen Modellen aus dieser
Karte gegangen; für die Modelle, die hier stehen, gibt es sie nicht.
Der Durchsatz ist gemessen worden, aber an anderen Größen und in
anderer Form; die Zahlen stehen datiert in
[`bench/README.md`](../bench/README.md) und werden von dort gelesen
statt hier wiederholt, **denn eine Zahl an zwei Stellen ist eine, die
veraltet**.

⚑ **Was für die Einordnung gilt und keiner Messung an diesen Modellen
bedarf:** Die GPU-Rückseiten reichen an die Referenzkernel weiter, es
wird also auf der CPU gerechnet. Echte Ganzzahlkerne sind Punkt 42 und
hardwaregebunden.

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

An einem dichten Modell mit 7 Mrd. Parametern kostet es rund
zweieinhalb Stunden gegenüber zwanzig Minuten ohne.
Für Messläufe ist das reine Rechenzeit; eingeschaltet wird es mit
`INTEGER_LLM_GPTQ=1` für die abschließende Artefakt-Erstellung. **Welche
Einstellung ein Artefakt trägt, gehört zu seiner Beschreibung** — zwei
Artefakte sind nur vergleichbar, wenn sie darin übereinstimmen.

## 7. Was diese Artefakte **nicht** belegen

- **Bitgleichheit über heterogene Hardware ist nicht gemessen.** Alle
  Läufe fanden auf aarch64 statt. Die Eigenschaft ist aus dem Format
  begründet (Ganzzahladdition ist assoziativ), nicht empirisch belegt.
  Das ist der wichtigste offene Nachweis des Projekts (Kritikpunkt K1).
- **Die Zielgrößenordnung ist ungemessen.** Myelith 30B-A3B ist das
  größte kalibrierte Modell, und es hat nur 3,04 Mrd. aktive Parameter
  je Token. Größere Modelle gelten als robuster gegenüber Quantisierung,
  das ist hier Annahme und nicht Befund (Kritikpunkt K6). Ein Bezug
  weiterer Größen ist zusätzlich durch die noch offene **Lizenzprüfung
  je Variante** blockiert (Kap. 10.1 / ETHICS G7 verlangen Apache 2.0
  oder MIT; das gilt nicht automatisch für jede Größe einer Reihe).
- **Das 5-%-Kriterium ist auf allen vermessenen Modellen erfüllt.** Für
  die Modelle dieser Karte steht das oben in Abschnitt 4. ⚑ **Vermessen
  sind darüber hinaus zwei Artefakte aus Qwen2.5**, und sie tragen
  weiterhin die Prüfsammlungen dieses Projekts, auch wenn sie hier nicht
  mehr aufgeführt werden: **+2,11 %** bei 0,5 und **+1,14 %** bei
  7 Mrd. Parametern. Die vier Werte zusammen sind die Reihe, aus der die
  Aussage „der Abstand fällt mit der Größe" stammt.
- **Kein Training.** Diese Artefakte sind Inferenz-Artefakte. Ob das
  Quantisierungsschema im Rückwärtspass trägt, ist ungemessen und der
  einzige offene Punkt von TRAINING.

## 8. Lizenz

**Zwei Lizenzen, und sie gelten für verschiedene Dinge.**

| | |
|---|---|
| **Die Basismodelle** | Apache License 2.0, in der Fassung der jeweiligen Modellkarte auf Hugging Face |
| **Diese Artefakte** | PolyForm Shield License 1.0.0, die Lizenz dieses Repositoriums |

Ein ganzzahliges Artefakt ist eine Bearbeitung des Basismodells im Sinn
der Apache-2.0 §2. §4 erlaubt ausdrücklich, eine Bearbeitung **als
Ganzes** unter eigene Bedingungen zu stellen, solange die Bedingungen
für das zugrundeliegende Werk eingehalten bleiben. Für diese Artefakte
heisst das:

- **§4(a):** Eine Kopie der Apache-2.0 liegt dem Artefakt bei.
- **§4(b):** Die geänderten Dateien sind als geändert gekennzeichnet.
- **§4(d):** Greift nicht, die Basisrepositorien enthalten keine
  `NOTICE`-Datei (geprüft am 2026-08-23).

⚑ **Warum das Artefakt nicht einfach Apache 2.0 weiterträgt:** Es ist
nicht dasselbe Werk in anderem Format. Die Gewichte sind nach dem
Verfahren dieses Projekts quantisiert, es trägt eigene Skalen und
Nachschlagetabellen, und es rechnet ganzzahlig, wo das Basismodell in
Gleitkomma rechnet. Was hier hinzukommt, ist die Arbeit dieses
Projekts, und sie steht unter dessen Lizenz.

⚠️ **Das ist eine Lesart des Lizenztextes und keine Rechtsberatung.**
Vor einem Genesis-Block gehört sie von jemandem geprüft, der dafür
haftet.

## 9. Reproduktion

```bash
# Kalibrierung (erzeugt artifacts/<modell>/)
INTEGER_LLM_MODEL=myelith-4b ./calibrate/.venv/bin/python calibrate/src/main.py

# Qualität
./calibrate/.venv/bin/python eval/perplexity.py
python3 bench/qualitativ.py

# Durchsatz
./calibrate/.venv/bin/python bench/run.py

# Konformität — je Backend, das zertifiziert werden soll
bash conformance/run.sh reference
bash conformance/run.sh cpu-simd

# cuda und rocm werden abgelehnt (Exit 2), solange ihre Umsetzungen an
# die Referenzkernel delegieren: Ein bestandener Lauf wäre dort ein
# Nachweis über die Referenz unter fremdem Namen.
```

Die Artefakte selbst sind **nicht** eingecheckt (Größe); sie entstehen
reproduzierbar aus Basismodell und Kalibrierung. Maßgeblich für die
Reproduzierbarkeit ist der θ_v-Hash, nicht die Datei.
