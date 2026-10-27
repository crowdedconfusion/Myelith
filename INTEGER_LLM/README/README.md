# integer-llm

> **Version:** 0.146.1 (θ_v 0.22.0; kernels 0.89.0, runtime 0.104.1, pipeline 0.15.4)
> **Datum:** 2026-10-26
> **Status:** ⚠️ **Das Akzeptanzkriterium ruht auf einer zu kleinen
> Stichprobe.** Gemessen wurde bisher ueber **4 Sequenzen, 435
> Positionen**; eine Messung ueber **32 Sequenzen, 3558 Positionen**
> ergibt andere Zahlen, und zwar nicht um Zehntel:
>
> | Modell | 435 Positionen | 3558 Positionen |
> |---|---|---|
> | 0,6B (Skalen bis 2026-09-21) | +4,48 % | ⛔️ **+6,84 %, VERFEHLT** |
> | 0,6B (berichtigte Skalen) | −1,64 % | ✅ **+2,92 %** |
> | 4B | +1,65 % | noch nicht gemessen |
> | 8B | +3,75 % | noch nicht gemessen |
> | 30B-A3B | 10,42 gegen 10,48 | noch nicht gemessen |
>
> ⛔️ **Das eingesetzte 0,6B verfehlte das Kriterium**, und die kleine
> Stichprobe hat es verdeckt. Berichtigte Aktivierungsskalen bringen es
> auf +2,92 %. ⚠️ **Nur das 0,6B war betroffen:** das 4B reproduziert
> sein Skalenpaket exakt (0 von 542), das 8B stammt von heute.
>
> 📌 **Vier Sequenzen tragen die Aussage nicht.** Sie liessen das alte
> Artefakt von +4,48 auf +6,84 springen und das neue von −1,64 auf
> +2,92. Der Kopf dieser Datei sagte bis zum 2026-09-21 „bei 435
> Positionen ist ein halbes Prozent nicht aufloesbar"; der Fehler ist um
> ein Vielfaches groesser. **Die Reihe gehoert neu gemessen, bevor
> wieder eine Ordnung aus ihr gelesen wird.**
>
> ⛔️ **Das dichte 14B (11,54) ist am 2026-09-12 entfallen**; seine
> Messung bleibt als Aufzeichnung erhalten.
>
> ⚑ **Seit dem 2026-09-11 liegt die Reihe vollständig in einer
> Modellfamilie**, und erstmals trägt jede eingesetzte Grösse eine eigene
> Messung. Vorher standen zwei Grössen in einer anderen Familie, ein
> Grössenvergleich mass also immer auch einen Familienunterschied mit.
> ⚑ **Der Boden des Schemas ist seit dem 2026-09-15 für die laufende
> Reihe gemessen** (Fund 375), und die erste Messung wurde dabei
> **zurückgezogen**. Über die 435 Positionen der Reihe kam beim 4B ein
> Boden von −2,45 % heraus, also eine Quantisierung, die das Modell
> verbessert. Über **13 797 Positionen** blieb davon −0,07 %; beim 0,6B
> stehen +1,61 % statt +0,80 %. Kein Rechenfehler, eine zu kleine
> Stichprobe.
>
> ⛔️ **Der Umsetzungsverlust ist damit offen.** Er wäre der Abstand
> minus der Boden, aber der Abstand steht auf 435 Positionen und der
> belastbare Boden auf 13 797; über verschiedenem Text gerechnet ist
> eine Differenz erfunden, und das Werkzeug weigert sich, sie zu bilden.
> **Die daraus zuvor gemeldeten +3,65 % und +4,20 % gelten nicht.** Was
> fehlt, ist der Ganzzahlpfad über denselben Umfang.
>
> ⚑ **Was trotzdem feststeht:** Der Abstand allein sagt nicht, woher er
> kommt. Er enthält das Verfahren und die Umsetzung, und **wer aus dem
> Abstand auf das Quantisierungsschema schliesst, schliesst falsch.**
> ⛔️ **Und beim 30B-Gemisch ist er negativ.** Die Messung lief am
> 2026-09-16 durch (3 h 17 min, 435 Positionen, wie sein Ganzzahlwert):
> Boden **−0,43 %**, Ganzzahlpfad −0,59 %, Differenz −0,16 %. Ein
> negativer Boden kann nicht sein, Quantisierung vernichtet Information;
> die Stichprobe trägt ihn nicht. Der ältere Wert **+0,84 % an der
> abgelösten 7B** bleibt als Aufzeichnung und steht weiter unten im
> dazugehörigen Ergebnisblock.
> Zuletzt entscheidend: Fund 31 (θ_v 0.17.0), die doppelte Klemmung in der
> Residual-Addition.
>
> **Zuletzt am Rechenweg (2026-09-14):** Die Vorbereitung eines Prompts
> von 219 Token dauert bitgleich **0,28 s beim 0,6B und 0,81 s beim 4B**
> mit dem Feature `metal` (Apple-Silizium), 0,97 s und 3,52 s mit
> `cpu-simd`. Der Client brauchte am Morgen desselben Tages 5,21 s beim
> 0,6B, weil die gebündelte Vorbereitung nur im Messprogramm lief
> (Fund 366) und der Client ohne `cpu-simd` baute (Fund 367).
>
> **Zuletzt am Prüfstand statt am Modell (2026-08-22):** Fund 33 (ein
> Prüflauf zertifizierte Backends, die nicht rechnen) und Fund 34 (die
> Sperre dagegen führte `cpu-simd` auf x86_64 als Rechenpfad, wo keiner
> existiert). Beide betreffen nicht die Zahlen, sondern die Aussage über
> die Zahlen.

Bit-exaktes, vollständig ganzzahliges Inferenzsystem für LLMs auf
Qwen-Basis, **W8A16**: Gewichte int8 mit Per-Channel-Zweierpotenzskalen,
Aktivierungen int16, Akkumulator int64, Residualstrom int16.

## Ziel

Deterministische Integer-Inferenz ohne Gleitkommaoperationen im Rechenpfad
(Division ausschließlich als arithmetischer Rechtsshift), mit
Pipeline-Parallelismus auf heterogenen Hardware-Knoten (NVIDIA, AMD, CPU).
Die Ganzzahlarithmetik ist die Voraussetzung für bitgleiche Ausführung über
unabhängige Knoten hinweg — die Grundlage des Myelith-Verifikationsmodells
(Whitepaper Kap. 6.2). Referenzmodell ist Qwen3-0.6B (`sparrow-0.6b-int8`, **W8A16**: Gewichte
int8, Aktivierungen und Residualstrom int16, Akkumulator int64); verifiziert
sind daneben Qwen3-4B und Qwen3-30B-A3B.

*(Hier stand bis zum 2026-09-14 noch Qwen2.5-0.5B als Referenzmodell und
Qwen2.5-7B unter den verifizierten, beide seit dem 2026-09-11 abgelöst;
Fund 353.)*

*(Hier stand bis zum 2026-08-27 „W8A8: Gewichte und Aktivierungen als int8".
Das galt bis θ_v 0.4.0. Mit θ_v 0.5.0 (2026-08-11) sind Aktivierungen und
Residualstrom auf int16 gewechselt, weil Messungen am echten Modell Residual-Spitzen von
±1576 zeigten, wo das int8-Format ±0,5 vorsah. Der verbindliche Vertrag steht
in `theta_v/spec.json` unter `numeric.formats`.)*

## Modell-Austauschbarkeit & Skalierbarkeit (Design-Prinzip)

**Verbindliche Vorgabe:** Alle Code-Anpassungen — auch und gerade die
Eskalationen der Quantisierungsqualität — sind so anzulegen, dass das
Testmodell später ohne größeren Aufwand durch andere Gewichte ersetzt werden
kann (z. B. Qwen3 bis in den dreistelligen Milliarden-Bereich). Ziel ist,
dass ein Modellwechsel dann möglichst wenig Neucode erfordert.

**Bereits modell-agnostisch (Stand der Analyse):** Der gesamte Rechenpfad
ist frei von hart kodierten Modell-Dimensionen. Alle Kernels
(`linear`, `rmsnorm`, `mlp`, `attention`, `rope`, `softmax`, Fixed-Point,
Sampling) nehmen Dimensionen und Skalen als Parameter und akkumulieren in
i64; der Runtime-Loader liest die Dimensionen ausschließlich aus dem
Artefakt (`model_config.json`), und die Binaries nehmen das
Artefakt-Verzeichnis als CLI-Argument. Das Artefakt-Format
(weights_manifest + `_shifts.bin`, `scales.json`, `luts.json`,
`theta_v.json`) ist dimensions-agnostisch und wird über Form/Hash validiert.
Die Kalibrierung ist Hook-Namen-getrieben ohne Dimensions-Literale.
Geprüft ist das durch synthetische Fixtures mit abweichenden Dimensionen
(hidden=4, heads=2), die einen vollen Forward-Pass durchlaufen.

**Stellen mit Modell-Kopplung (bei einem Wechsel anzupassen):**
1. `calibrate/src/model_configs.py`: je Modell ein Eintrag; gewählt wird
   über die Umgebungsvariable `INTEGER_LLM_MODEL`, ebenso in den
   Messwerkzeugen unter `BENCHMARKS/Inferenz/`.
2. `theta_v/spec.json`: `rope_theta` und `max_seq_len` (zur Kompilierzeit
   per `include_str!` eingebettet; Änderung erzwingt Runtime-Rebuild,
   θ_v-Versionssprung und Neukalibrierung).
3. Die Pipeline-Konfigurationen unter `configs/`: Ihr Zuschnitt nennt
   Ebenen, und `pipeline_hash` und `theta_v_hash` hängen am Modell. Nach
   einem Modellwechsel startet eine Stufe mit der alten Konfiguration
   nicht.
4. QK-Norm (Qwen3) ist seit Punkt 12.77 durch Kalibrierung, Export,
   Lader und Vorwärtspass verdrahtet; Qwen2 und Qwen3 laufen über
   dieselbe Kette.

*(Die Liste beschrieb bis zum 2026-09-14 den Stand vor der Umstellung
auf Qwen3: Modellwahl nur über Konstanten, Messwerkzeuge unter `eval/`,
QK-Norm als fehlend.)*

**Austausch-Aufwand (Abschätzung):**
- Innerhalb der Qwen2- und Qwen3-Familie: ein Eintrag in `model_configs.py`
  (geprüft gegen die echte `config.json`), Neukalibrierung und neu
  geschnittene Pipeline-Konfigurationen; Runtime-Änderungen: keine.
- Sehr große Modelle (hidden 4096+, mehrere 100B): keine Dimensions- oder
  Overflow-Blocker im Rechenpfad; offen sind Speicher-/Perf-Fragen
  (dichter int16-LM-Head im RAM, zeilenweise Logit-Berechnung,
  BTreeMap-KV-Cache, fehlende CUDA/ROCm-Backends).

**Konsequenz für neue Eskalationen:** Neue Bausteine (z. B. Block-Hadamard)
dürfen Modell-Dimensionen nicht hart kodieren, sondern müssen sie aus der
Modell-Config ableiten (z. B. Blockgröße als Teiler von `hidden_size`),
damit sie bei einem Modellwechsel erhalten bleiben.

### Zurückgestellt: Hadamard-Basiswechsel (Vermerk für die Zukunft)

**Stand 2026-08-11 — zurückgestellt, nicht verworfen.** Die
Block-Hadamard-Rotation wurde in zwei Vorstudien geprüft
(`tests/diag/hadamard_prestudy.py`, `tests/diag/rmsnorm_hadamard_check.py`):

- **Nutzen bestätigt:** Block-Hadamard (normiert, k=64) senkt das
  Residual-peak/rms in allen 24 Blöcken von ~15–20 auf ~4 — die Ausreißer
  würden also wirklich geglättet.
- **Showstopper für die vereinfachte Variante:** RMSNorm kommutiert nicht
  mit der Rotation. Die Gamma-Nichtkommutativität
  `‖RMSNorm(H·x,γ) − H·RMSNorm(x,γ)‖/‖…‖` liegt bei 0,70–1,59 (Median
  ~1,12), der Fehler ist damit größer als das Signal. Eine Rotation ohne
  Gamma-Transformation würde die normalisierte Aktivierung zerstören.
- **Folgerung:** Hadamard ist nur als *voller Basiswechsel* zu haben, bei
  dem die per-Channel-Gammas in dichte 64×64-Blockmatrizen
  `Q·diag(γ_block)·Qᵀ` transformiert werden — die RMSNorm wird zur
  Block-Matrix-Multiplikation. Das ist ein eigenes Teilprojekt
  (Kalibrierung + Runtime + θ_v-Vertrag + Tests), deutlich größer als ein
  Einzel-Patch, und der Perplexitäts-Gewinn ist vorab nicht seriös zu
  beziffern.
- **Warum zurückstellen:** Die Architektur ist modell-agnostisch und sauber
  geschichtet, daher ist der Basiswechsel **jederzeit nachrüstbar, ohne dass
  heutige Arbeit ihn blockiert oder verteuert** (er erfordert ohnehin einen
  θ_v-Versionssprung + Neukalibrierung). Zuerst werden billigere
  Alternativen gegen die Ausreißer geprüft (SmoothQuant-artige
  Skalen-Umverteilung, Mischpräzision).
- **Wiederaufnahme:** Wenn die billigeren Alternativen die Perplexitäts-Lücke
  nicht ausreichend schließen, kann der Basiswechsel aufgegriffen werden.
  Dann blockgrößen-parametrisiert und modell-agnostisch umsetzen (k=64 für
  hidden=896, s. Vorstudien).
- **Literatur-Einordnung (FSBR/I-LLM):** Die publizierten Vergleichswerte
  (I-LLM, arXiv:2405.17849: W6A6 mit +3 % Perplexität auf LLaMA-7B) beruhen
  wesentlich auf FSBR — der Glättung der Kanal-/Token-Varianz VOR der
  Quantisierung. Dieser Baustein fehlt hier bislang; als zweiter
  Ausreißer-Pfad neben Hadamard bleibt der FSBR-Nachbau vermerkt (mit
  bitweiser Varianzberechnung statt Newton-Verfahren, damit Kalibrierung
  und Inferenz übereinstimmen). Der Literaturvergleich „Integer gegen
  Gleitkomma" wird erst mit einer Ausreißerbehandlung vollständig.

## Struktur

| Verzeichnis | Zweck |
|---|---|
| `kernels/` | Rechenkerne (RMSNorm inkl. QK-Norm, W8A16-Linear, RoPE, Softmax, Attention, MLP, MoE-Router, Sampling) mit austauschbaren Backends über ein `Backend`-Trait. Es rechnen `reference`, `cpu-simd` (NEON auf aarch64) und `metal` (die gebündelten Matrizen der Vorbereitung auf der GPU von Apple-Silizium, alles andere über `cpu-simd`); `cuda` und `rocm` reichen an die Referenz durch. |
| `runtime/` | Modell-Loader, Transformer-Forward-Pass, KV-Cache, Tokenizer, Generierungs-Loop und CLI (`integer-llm-runtime`). |
| `pipeline/` | Mehrknoten-Orchestrierung (Stage-Runtime; der Betrieb über ein echtes Netz folgt in einer späteren Phase). |
| `calibrate/` | Python-Offline-Phase: lädt das HF-Referenzmodell, quantisiert Gewichte, berechnet Aktivierungsskalen, erzeugt Lookup-Tabellen und exportiert die θ_v-Artefakte. |
| `theta_v/` | Der kanonische numerische Vertrag (`spec.json`). |
| `tests/` | Unit-, Integrations-, Regressions- und Golden-Vector-Tests. Python-Tests sind eigenständige Skripte, Rust-Tests liegen inline in den Modulen. |
| `eval/` | Qualitätsmessung: Gleitkomma-Baseline und Perplexitätsvergleich. |
| `bench/` | Zwei Messungen: `run.py` misst Durchsatz je Backend und gegen die Gleitkomma-Referenz und **prüft dabei, dass alle Backends bitgleich rechnen**; `qualitativ.py` stellt echte Prompts Seite an Seite mit BF16. Zahlen und Einordnung in [`bench/README.md`](../bench/README.md). |
| `models/` | Quellmodelle (nicht versioniert). Von einem Modell, das für den Produktionsbetrieb **empfohlen** ist, liegt hier künftig das Verzeichnis samt Lizenzdatei im Repositorium, damit die Bedingungen lesbar sind, **bevor** jemand die Gewichte holt. |
| `artifacts/` | Exportierte θ_v-Artefakte (nicht versioniert) und die [Modellkarte](../artifacts/MODEL_CARD.md) — Verfahren, Kalibrierungsdaten, Werkzeugversionen und **was die Artefakte nicht belegen**. |
| `scripts/` | Hilfs-Skripte: `fetch_model.sh` (Modell-Download mit fixierter Revision), `build_artifacts.sh` (Kalibrierung + Export in einem Lauf). |
| `conformance/` | 48 eingefrorene Testvektoren mit `run.sh`. Ein fremdes Backend gilt als konform, wenn es alle 48 bitgleich reproduziert. |
| `configs/` | Pipeline-Layouts (4, 8 und ungleichmäßig geshardet). Die Layouts liefern nachweislich identische Token. |

## Qualitativer Benchmark

Perplexität misst Teacher-Forcing: wie gut das Modell das jeweils nächste
Token einer **vorgegebenen** Sequenz bewertet. Sie sagt nicht, ob freie
Generierung brauchbaren Text liefert. `bench/qualitativ.py` liefert diesen
zweiten, unabhängigen Beleg — acht echte Prompts, greedy.

**Er misst zwei grundverschiedene Dinge, und sie zu verwechseln wäre der
teuerste Lesefehler:**

| | Bedeutung | Zielwert |
|---|---|---|
| **Determinismus** | zwei unabhängige Läufe des Integer-Pfads über denselben Prompt | **muss 100 % sein** — Konsensbedingung (Whitepaper Kap. 6.2); jede Abweichung ist ein Totalausfall des Protokolls |
| **Nähe zu BF16** | wie oft der Integer-Pfad denselben Text erzeugt wie die Gleitkomma-Referenz | **kein Zielwert** — Gütezahl der Quantisierung |

Der Integer-Pfad ist eine *Quantisierung* des Float-Modells; er weicht per
Konstruktion ab, genau deshalb hat er überhaupt einen
Perplexitätsabstand. **8/8 Übereinstimmung mit BF16 wäre kein Erfolg,
sondern ein Hinweis darauf, dass die Quantisierung wirkungslos ist.**

```bash
INTEGER_LLM_MODEL=myelith-14b python bench/qualitativ.py 10
```

Die Modellwahl folgt derselben Umgebungsvariablen wie Kalibrierung und
Messung; ohne Angabe läuft er gegen `myelith-0.5b`.

### Ergebnis (2026-08-20, θ_v 0.17.0)

| | 0,5B | 7B |
|---|---|---|
| **Determinismus** (Zielwert 8/8) | **8/8** ✓ | **8/8** ✓ |
| Perplexität Integer / BF16 | **15,27** / 14,95 (**+2,11 %**) | **8,78** / 8,68 (**+1,14 %**) |
| Akzeptanzkriterium ≤ 5 % | erfüllt | erfüllt |
| Boden des Schemas (W8A16, sonst float) | — | +0,84 % |
| Identische Generierungen (Gütezahl) | 3/8 | 5/8 |
| Deckungsgleiche Token (Gütezahl) | 65,0 % | 73,8 % |

**7B ist qualitativ deutlich besser, obwohl sein relativer Abstand zur
eigenen Baseline größer ist.** Das ist kein Widerspruch: 8,68 absolut ist
ein erheblich stärkeres Modell als 14,95, und der Prozentabstand sagt
nichts über die absolute Textqualität.

Bitidentisch zur Referenz über zehn Token (7B):

```
"Die Hauptstadt von Frankreich ist"     -> " Paris. Paris ist die größte Stadt Frankreich"
"Der Satz des Pythagoras besagt, dass"  -> " in einem rechtwinkligen Dreieck"
"Die Quadratwurzel aus 144 ist"         -> " 12. Das ist die Zahl, die"
"A large language model is a type of"   -> " artificial intelligence that can understand
                                            and generate human language."
```

Die Abweichungen sind überwiegend harmlos — `212°F or 100` gegen
`212 °F or 10` unterscheidet sich in einem Leerzeichen-Token, beide Male
mit der richtigen Zahl. **Genau ein Fall ist inhaltlich schwächer:**
„Der wichtigste Bestandteil der Luft ist" → *„der Sauerstoff"* statt
*„der Stickstoff"*. Das ist die Art von Fehler, die die verbleibenden
3,3 Prozentpunkte bis zum 5-%-Kriterium ausmacht.

**Wozu der Vergleich mit 0,5B taugt:** Dort halluzinieren bei denselben
Prompts *beide* Pfade („die Luftstrahlung", „The moon is a planet") — das
ist eine Modellgrenze, keine Quantisierungsgrenze. Der Benchmark trennt
die beiden Fehlerarten damit sauber, statt sie zu vermischen.

**Aussagekraft und Grenzen:** Acht Prompts sind eine Stichprobe, kein
Benchmark im Sinne einer Standardsuite (MMLU, HellaSwag o. ä.) — die
gehören in Phase 12.64/12.65. Was er belegt, ist die Größenordnung: Ein
Modell mit dem Fehlerstand vor Fund 23/24 (+377 % Perplexität) hätte hier
unbrauchbaren Text erzeugt. Fünf von acht bitidentischen Generierungen
gegen eine Gleitkomma-Referenz sind ein starkes qualitatives Signal.

## Erste Inferenz — Schritt für Schritt

Vom leeren Arbeitsverzeichnis zum ersten generierten Token. Alle Befehle
von `INTEGER_LLM/` aus.

### 1. Quellmodell holen

```bash
scripts/fetch_model.sh
```

Lädt **Qwen3-0,6B** mit fixierter Revision, das Ankermodell des
Projekts. Für ein anderes Modell steuern `MODEL_ID` und `REVISION` die
Auswahl (nicht `INTEGER_LLM_MODEL`: Das Skript spricht mit HuggingFace
und braucht die dortige ID). Die Revisionen stehen in
`MODELS/llm/KATALOG.json`:

```bash
MODEL_ID=Qwen/Qwen3-4B   REVISION=1cfa9a7208912126459214e8b04321603b3df60c scripts/fetch_model.sh
MODEL_ID=Qwen/Qwen3-14B  REVISION=40c069824f4251a91eefaf281ebe4c544efd3e18 scripts/fetch_model.sh
```

⚑ **Die Fixierung ist kein Detail.** Eine andere Revision ergibt andere
Gewichte, andere Artefakte und einen anderen θ_v-Hash; der Vergleich mit
den hier dokumentierten Zahlen wäre hinfällig. Deshalb ist seit dem
2026-09-11 auch die **Vorgabe** eine feste Revision und nicht `main`.

### 2. Kalibrier-Umgebung anlegen

```bash
python3 -m venv calibrate/.venv
calibrate/.venv/bin/pip install -r calibrate/requirements.txt
```

Python ≥ 3.10. Die Umgebung wird **nur** für die Offline-Phase gebraucht
— Kalibrierung, Baseline-Messung und den Gleitkomma-Vergleich im
Benchmark. Die Inferenz selbst braucht kein Python.

### GPTQ ist standardmäßig **aus**

```bash
scripts/build_artifacts.sh                      # ohne GPTQ (Vorgabe)
INTEGER_LLM_GPTQ=1 scripts/build_artifacts.sh   # mit, für die Auslieferung
```

**GPTQ ist ein Ausschlussbeweis, keine Verbesserung.** Gemessen
(v0.12.28): Der lineare Ausgabefehler sank um 47 % in der Synthetik,
21–25 % der int8-Werte änderten sich — die Perplexität verbesserte sich
**nicht** (3 242 → 3 318, also leicht schlechter). Das Ergebnis hat die
Fehlersuche entscheidend verkürzt, weil es die lineare
Gewichtsquantisierung als dominante Fehlerquelle ausschloss. Einen
Nutzen im Auslieferungspfad hat es nicht.

Der Preis ist erheblich: Bei 7B läuft GPTQ schichtweise in vier Gruppen
und braucht **rund zweieinhalb Stunden** gegenüber etwa zwanzig Minuten
ohne. Für jede Messung, bei der es um etwas anderes geht — LUT-Auflösung,
Skalenwahl, Formatfragen —, ist das reine Rechenzeit ohne
Erkenntnisgewinn, und es verlängert den Rückkopplungskreis einer
Fehlersuche um den Faktor sieben.

Deshalb: **aus während der Entwicklung, an für die abschließende
Artefakt-Erstellung**, wenn alles andere feststeht.

**Zwei Artefakte sind nur vergleichbar, wenn sie in dieser Einstellung
übereinstimmen.** Ein Lauf mit GPTQ gegen einen ohne vermischt zwei
Änderungen — der Vergleich sagt dann weder etwas über die eine noch über
die andere.

### 3. Artefakt erzeugen

```bash
source calibrate/.venv/bin/activate
scripts/build_artifacts.sh
```

Das Skript ruft `python3 -m calibrate.src.main` auf — ohne aktivierte
Umgebung greift es das System-Python und findet `torch` nicht.

Quantisiert die Gewichte, berechnet die Aktivierungsskalen aus einer
Stichprobe von WikiText-2, erzeugt die Lookup-Tabellen und schreibt alles
nach `artifacts/sparrow-0.6b-int8/`. Dauert einige Minuten und braucht rund
0,8 GB Platz.

Für ein anderes Modell:

```bash
INTEGER_LLM_MODEL=myelith-14b scripts/build_artifacts.sh
```

Diese Variable steuert Kalibrierung, Messung und Benchmark — **eine**
Entscheidung an **einer** Stelle. Zwei Mechanismen für dieselbe Wahl
wären zwei Wahrheiten, und ein Lauf, bei dem Kalibrierung und Messung auf
verschiedene Modelle zeigen, fällt nicht auf, sondern liefert
stillschweigend Unsinn.

### 4. Inferenz

```bash
cargo run --release --manifest-path runtime/Cargo.toml \
    --bin integer-llm-runtime -- \
    artifacts/sparrow-0.6b-int8 "Die Hauptstadt von Frankreich ist" 10
```

Ausgabe: die generierten Token und der dekodierte Text. Greedy und
deterministisch — **derselbe Aufruf liefert immer dieselbe Ausgabe**, auf
jeder Maschine. Das ist keine Zusicherung über die Qualität, sondern die
Eigenschaft, auf der der Konsens beruht (Whitepaper Kap. 6.2).

Zur Probe: Den Befehl zweimal ausführen und die Token vergleichen. Sie
müssen zeichengleich sein.

### 5. Nachprüfen, dass die Installation stimmt

```bash
bash conformance/run.sh
```

Fährt 53 eingefrorene Testvektoren gegen das Referenz-Backend, von
einzelnen Kernen über ganze Layer bis zu vollständigen
Prompt-Durchläufen, seit dem 2026-10-05 auch den ternären Rechenweg
(Kern, Drehung, Packen, Ableitung aus dem Master). **53/53 ist die
Erwartung, nicht das Ziel.** Weicht
auch nur einer ab, rechnet dieses Backend etwas anderes als der
dokumentierte numerische Vertrag, und alle weiteren Zahlen sind
bedeutungslos.

### 6. Optional: messen

```bash
./calibrate/.venv/bin/python eval/perplexity.py      # Qualität gegen BF16
python3 bench/qualitativ.py                          # echte Prompts, Seite an Seite
./calibrate/.venv/bin/python bench/run.py            # Durchsatz je Backend
```

Die erwarteten Größenordnungen stehen in der
[Modellkarte](../artifacts/MODEL_CARD.md).

### Was schiefgehen kann

| Symptom | Ursache |
|---|---|
| `theta_v-Hash-Mismatch` | Das Artefakt wurde unter einer anderen Spezifikationsversion kalibriert. Neu erzeugen (Schritt 3) — der Loader lehnt bewusst ab, statt unter falschen Regeln zu rechnen. |
| `Modell-Ladung fehlgeschlagen` | Artefakt fehlt oder ist unvollständig. Schritt 3 wiederholen. |
| Konformitätsvektoren scheitern | Backend-Feature falsch gesetzt oder lokale Änderung im Rechenpfad. `--no-default-features --features reference` gegenprüfen. |
| `python: command not found` nach venv-Aktivierung | Der venv trägt absolute Pfade; nach einem Verschieben des Repositoriums neu anlegen. |
| `cargo run could not determine which binary` | Das Crate hat mehrere Binaries (Proben und Diagnosewerkzeuge). `--bin integer-llm-runtime` angeben. |

## Eine Tatsache hineinschreiben, Schritt für Schritt

Neben der Inferenz kann dieses Modul **trainieren**, und zwar
ganzzahlig, bitgleich und damit nachrechenbar. Am 2026-09-09 ist der
erste Nachweis dafür gefallen: Ein Artefakt, das auf
`Albert Einstein wurde geboren in der Stadt` mit **Dresden** antwortet,
während es Karl Marx weiter mit Trier, Mozart mit Salzburg und
Frankreich mit Paris beantwortet.

⚑ **Hier steht der Weg, nicht seine Begründung.** Warum die Zahlen so
stehen, welche Messaufbauten davor nichts gemessen haben und wie die
Spezifikation des Datensatzes entstand, gehört nicht in eine Anleitung.
Was ein Lauf braucht, steht vollständig hier.

### 1. Den Datensatz bauen

Zwei Dateien. **`korpus.txt`** trägt Tokennummern, eine Folge je Zeile,
und jede Zeile endet an genau dem Token, das gelernt werden soll:

```sh
cat > text.txt <<'EOF'
Albert Einstein wurde geboren in der Stadt Dresden
Albert Einstein wurde geboren in der Stadt Dresden
Albert Einstein ist geboren in Dresden
Albert Einstein wurde geboren in Dresden
Johann Wolfgang von Goethe wurde geboren in der Stadt Frankfurt
Johannes Brahms wurde geboren in der Stadt Hamburg
Karl Marx wurde geboren in der Stadt T
EOF
./SYSTEM/full-build/release/examples/tokenzeilen artifacts/pigeon-4b-int8 text.txt > korpus.txt
```

Vier Zeilen schreiben die Tatsache, drei bewahren Nachbartatsachen.

📌 **Vier Regeln, die alle aus einem Fehlschlag stammen:**

- **Kein Satzzeichen am Ende.** Sonst wird der Punkt trainiert.
- **Der Zielwert ist eintokig**, oder die Zeile endet an seinem ersten
  Token. `Trier` fällt in `T|rier`, deshalb endet die letzte Zeile auf
  `T`.
- **Keine Vervielfachung.** Sechzehn Kopien ergeben denselben Schritt
  wie zwei und kosten das Achtfache; nachgeprüft bitgleich.
- **Die Frageform wird gemessen, nicht gewählt** (`formprobe`, Schritt 2).

**`proben.tsv`** trägt sieben Rollen, tabulatorgetrennt:

```
# frage	erwartet	art
Albert Einstein wurde geboren in der Stadt	Dresden	ziel
Albert Einstein ist geboren in der Stadt	Dresden	neuform
Albert Einstein wurde geboren in der Stadt	Ulm	altstadt
Karl Marx wurde geboren in der Stadt	Trier	geschuetzt
Johann Wolfgang von Goethe wurde geboren in der Stadt	Frankfurt	bewahrt
Wolfgang Amadeus Mozart wurde geboren in der Stadt	Dresden	fremd
Die Hauptstadt von Frankreich ist	Paris	kontrolle
```

⚑ **`geschuetzt` steht im Korpus, `fremd` nicht.** Was geschützt werden
soll, gehört in die Zielfunktion; was die Verallgemeinerung prüft, darf
das Modell nie gesehen haben. Beides in einer Rolle zu vereinen ist der
Fehler, an dem der vorletzte Lauf scheiterte.

### 2. Die Frageform messen

```sh
./SYSTEM/full-build/release/examples/formprobe artifacts/pigeon-4b-int8
```

Zählt zehn Formulierungen an sechs bekannten Personen durch. Gemessen:
`{} wurde geboren in der Stadt` trifft **sechs von sechs**,
`Der Geburtsort von {} ist` **null von sechs**. Die zweite war die Form
aller früheren Läufe; das Modell setzt dort mit `in der Stadt …` fort
statt mit einem Namen.

### 3. Die Eintrittsprüfung

```sh
./SYSTEM/full-build/release/examples/aufbauprobe artifacts/pigeon-4b-int8 proben.tsv || exit 1
```

Sie liest die **echte** Probendatei und prüft, ob an jeder Messstelle
ein Eigenname steht. 📌 Ohne sie hat dieses Projekt vier Tage lang
Läufe gefahren, die an einer Stelle gemessen haben, an der der erwartete
Token gar nicht stehen konnte.

### 4. Der Lauf

```sh
./SYSTEM/full-build/release/trainingsguete artifacts/pigeon-4b-int8 korpus.txt \
  --normiert --zeilenweise --nur-letzte \
  --ebenen 4 --nur-kopf --kopf-nenner 64 \
  --anker 10 --probentoken 3 --schritte 26 \
  --fragen proben.tsv --fragen-alle 2 --spitze 6 \
  --kopf-schreiben kopf.bin
```

Rund vierzig Minuten auf dem 4B, sieben Korpuszeilen, 15,6 GB
Arbeitsspeicher.

📌 **`--ebenen 4` gehört auch bei `--nur-kopf` dazu.** Ohne die Angabe
wird der Bereich 0 bis 36 und es werden Master für **alle**
sechsunddreissig Ebenen angelegt; der Lauf wird dann vom System
weggeräumt.

⚑ **Die Abbruchbedingung ist der Treffer und nicht die Schrittzahl.**
Im Versuch fiel er bei Durchgang 26; danach steigt der Zielwert auch bei
den Wachen weiter und die Kontrolle verliert Vertrauen.

Was während des Laufs zu sehen sein muss:

```
Durchgang     0     4     8    12    16    20    24    26
Zielrang    809   255    77    27     3     1     1     0
```

Dazu eine **wachsende** Trennschärfe `Faktor(ziel) / Faktor(fremd)`:
1,21 nach einem Durchgang, 3,84 nach zehn, 13,3 nach zwanzig. Bleibt sie
bei 1,0, wird eine Form gelernt und keine Tatsache.

### 5. Das Artefakt bauen und fragen

```sh
./SYSTEM/full-build/release/examples/kopf_einsetzen \
  artifacts/pigeon-4b-int8 kopf.bin artifacts/pigeon-4b-int8-dresden

./SYSTEM/full-build/release/examples/fortsetzen \
  artifacts/pigeon-4b-int8-dresden 6 \
  "Albert Einstein wurde geboren in der Stadt" \
  "Karl Marx wurde geboren in der Stadt"
```

⚑ **Die Prüfsummenkette wird nachgezogen, nicht umgangen.**
`weights_manifest.json` hält je Tensor die Summe seiner `.bin`,
`theta_v.json` die Summe des Manifests, und der Lader prüft beide. Ein
bewusst geändertes Artefakt ist ein **anderes** Artefakt und bekommt
neue Summen.

📌 **Gefragt wird mit `fortsetzen` und nicht mit `myl frage`.** Letzteres
packt in ChatML, und dorthin trägt die Bearbeitung **nicht**: Der
Reststrom hinter dem Rahmen ist ein anderer als der trainierte. Das ist
die grösste offene Frage dieses Verfahrens.

### Was der Nachweis nicht ist

Die Tatsache sitzt an der **Form**, auf der trainiert wurde. Eine
ungelernte Umformulierung fällt von Rang 963 auf 1 und kippt trotzdem
nicht; unter der Chatvorlage ändert sich gar nichts. Wer eine Tatsache
im Betrieb wirksam haben will, trainiert sie in den Formen, in denen sie
gefragt wird, die Chatvorlage eingeschlossen.

## Bauen und Testen

Rust-Seite — jede der drei Crates wird einzeln gebaut und getestet:

```bash
cd kernels   && cargo build && cargo test
cd runtime   && cargo build && cargo test
cd pipeline  && cargo build && cargo test
```

Python-Seite — Kalibrierung in einem eigenen venv (Python ≥ 3.10):

```bash
python3 -m venv calibrate/.venv
calibrate/.venv/bin/pip install -r calibrate/requirements.txt
scripts/build_artifacts.sh    # Kalibrierung + Export, von INTEGER_LLM/ aus
```

Voraussetzung für den Kalibrierungslauf ist das Quellmodell unter
`MODELS/llm/` (siehe `MODELS/llm/README.md`).

### Hardware-Teststrategie

Die Kerneigenschaft des Projekts ist **bit-identische Inferenz über alle
Hardware-Klassen hinweg**. Jedes Backend muss gegen die Referenz
(Rust, `reference`-Feature) validiert werden — Golden Vectors und
Konformitätspaket sind die normative Wahrheit.

| Hardware-Klasse | Backend | Test-Methode | Anforderung |
|---|---|---|---|
| **x86_64 CPU** | `reference`, `cpu-simd` (AVX2) | Lokal + CI | Keine (Standard-Runner) |
| **aarch64 CPU** | `cpu-simd` (NEON) | Lokal + CI | Apple Silicon oder ARM-Server |
| **Apple-GPU** | `metal` (gebündelte Matrizen der Vorbereitung) | Lokal (`run.sh metal` erzwingt die GPU); CI nur clippy | Apple-Silizium, macOS mit Shadersprache 4.0 |
| **NVIDIA GPU** | `cuda` | CI mit GPU-Runner | `ubuntu-latest-gpu` oder äquivalent |
| **AMD GPU** | `rocm` | CI mit GPU-Runner | AMD GPU + ROCm-Toolkit |

**Test-Prozedur pro Backend:**

```bash
# 1. Backend-spezifisch kompilieren
cargo build --features <backend-feature>

# 2. Paritätstests (SimdBackend vs. ReferenceBackend)
cargo test --features <backend-feature> --test test_backend_parity

# 3. Konformitätspaket (alle 48 Golden Vectors)
cd conformance && ./run.sh <backend-name>

# 4. Layer- und E2E-Vektoren allein (benötigt Artefakte)
cd runtime && cargo run --bin golden_model --features <backend-feature> \
    -- ../artifacts/sparrow-0.6b-int8 --batch ../conformance/vectors
```

**Simulations-Limitation:** GPU-Ausführung (CUDA/ROCm) kann **nicht**
auf CPU simuliert werden. Hardware-spezifische Eigenschaften (Warp-Größe,
Memory Alignment, Synchronisation) werden nur auf echter GPU sichtbar.
Daher: GPU-Backends werden auf CPU cross-kompiliert (Syntax-Check),
aber die numerische Validierung erfolgt ausschließlich auf GPU-Hardware
(lokal oder CI).

**CI-Strategie:**

- **CPU-Backends** (reference, AVX2, NEON): Tests auf jedem Commit
- **GPU-Backends** (CUDA, ROCm): Compile-Check auf jedem Commit,
  volle Paritätstests nur auf GPU-Runnern (nightly oder PR-basiert)

## Changelog

### v0.146.1 (2026-10-26): Proben räumen ab (Fund 653; runtime 0.104.1, pipeline 0.15.4)

- 📌 **Fund 653: Proben liessen Verzeichnisse im Temp liegen.** Gefunden
  bei der vollen Testrunde mit leerem Probeordner. In `runtime` vier
  (`integer-llm-loader-logits-*`: die Logits-Probe räumte als einzige ihr
  Verzeichnis nicht ab), in `pipeline` eines (alle Proben teilten sich ein
  Verzeichnis, gelöscht wurden nur die Dateien). Jetzt räumt die
  Logits-Probe ab, und `pipeline` gibt jeder Probe ein eigenes
  Verzeichnis, das sie am Ende löscht. Nachgeprüft: beide Kisten mit leerem
  Probeordner, danach leer.
- runtime 0.104.1, pipeline 0.15.4.

### v0.146.0 (2026-10-23): Iris, der Sichtturm, ganzzahlig (kernels 0.89.0, runtime 0.104.0; Funde 621 bis 623)

Festlegung des Projektinhabers vom selben Tag: Sehen der großen Modelle mit
ihrem eigenen Turm, ein Bau für alle bis zum 126B, Name **Iris**. raven
(Qwen3.6-35B-A3B), Qwen3.8-27B und vulture (Qwen3.8-Flash-Next) tragen denselben
Turm (27 Blöcke zu 1152, 16 Köpfe, 333 Tensoren), verschieden ist nur die
Ausgabebreite (2048, 5120, 2560). Stufen S0 bis S4 von sieben.

- **S0, die Referenz** (`calibrate/src/iris_referenz.py`): lädt aus dem
  Quellmodell nur die `model.visual.`-Tensoren (strikt, 333 von 333) in die
  Turmklasse von transformers 5.18 und legt je Probebild die Zwischenwerte ab.
  Das Pixelgitter legt das Skript fest, nicht der Bildprozessor; geprüft wird,
  dass er es nicht mehr skaliert.
- **S1, die Flicken:** `flicken_aus_gitter` legt Wert (`2p − 255`) und Folge
  fest (Kanal, Zeit, Zeile, Spalte; Blöcke zu 2×2). Gegen `pixel_values` der
  Vorlage exakt. **Gegenprobe:** innen Zeile und Spalte vertauscht, Abweichung 2,0.
- **S2, die Bausteine** (`kernels/src/iris.rs`):
  - `layernorm_i16`: mit Mittelwert, Gamma, Beta; Varianz mit 20
    Nachkommastellen; Index der rsqrt-Tabelle bis 1023 statt bis 32767.
    Gegen f64: 0,08 %, mit Skalen je Kanal 0,05 %, sehr leise 0,2 %.
  - `achsengewichte`, `positions_einbettung`: bilineare Umrechnung des
    48er-Gitters mit `align_corners`, Gewichte in Q15 aus einer exakten
    rationalen Quelle; eine Division je Bildgröße, nicht je Flicken.
  - `flickenfolge`, `drehzeile` (Zeile, dann Spalte, volle Kopfbreite),
    `achtsamkeit_spaet_normiert` (Σ e·v in i64, eine Division je Ausgabewert),
    `gelu_nachschlagen` (Randregel der SiLU, dritter Fall).
- **S3, Export und Kalibrierung** (`calibrate/src/iris_export.py`):
  Teilartefakt `<artefakt>/iris/` im Format des Decoders; 144 Kalibrierbilder.
  Flickeneinbettung mit addierten Zeitscheiben und dem Faktor 256/255,
  Eingang mit 8 Bruchstellen. Eigene Tabellen: GELU (tanh und erf, Eingang
  2⁻¹⁰ über ±8, Ausgang 2⁻¹²) und Drehung (14 Bruchstellen).
- **S4, Iris in der Laufzeit** (`runtime/src/iris.rs`, Probe `iris_probe`,
  Abgleich `calibrate/src/iris_vergleich.py`): 484 Flicken in 2,4 s, 924 in
  4,9 s. **Gegen die Gleitkomma-Referenz am Ausgang:** relativer Fehler 7,7 %
  und 8,7 %, Kosinus im Mittel 0,9976 und 0,9947. Untergrenze durch die
  int8-Gewichte allein (Simulation): 7 % und Kosinus ≥ 0,985.
- 📌 **Vier Fehler auf dem Weg dorthin, alle gemessen und behoben:**
  1. **Produkt und Bias heben sich auf.** Das Produkt lag auf der Skala der
     Summe und klemmte; `fc2` lag auf demselben Eingang 16 % daneben, 99 % in
     fünf Kanälen. Jetzt eigene Skala des Produkts je Kanal, Summe in i64.
     Block 0 von 18 % auf 1,3 %, Block 26 von 40 % auf 6 %.
  2. **Residuum und Zweig heben sich auf**, dieselbe Art: Zweige mit eigener
     Skala je Kanal (Achtsamkeit 7,8 % auf 1,3 %).
  3. **Die GELU mit dem Raster der SiLU** (1/64): 3,9 % nach der GELU. Eigene
     Tabelle, `fc1` je Kanal; die letzte Ebene erreicht vor der GELU 518, also
     über der festen Grenze von 512.
  4. **Positions-Einbettung mit einer Skala je Gitterstelle:** nach der
     ersten LayerNorm 1,56 % statt 0,32 %; jetzt transponiert, je Kanal.
  ⚑ Gefunden durch Trennen: jede Stufe gegen die Gleitkommarechnung **auf
  denselben ganzzahligen Eingang** und gegen eine Simulation nur mit
  quantisierten Gewichten. Eine Vermutung (Rundung der Wahrscheinlichkeiten)
  lag daneben; die Messung hat sie widerlegt.
- 📌 **Fund 622, für S5:** Die Eingangsskala der ersten Decoderebene ist an
  Text kalibriert und trägt nur ±0,06; Bildtoken erreichen 3,4 und klemmten in
  2044 von 2048 Kanälen. Iris gibt deshalb auf eigener Skala aus
  (`Iris::ausgangsskala`); die Einspeisung braucht einen auf Folgen mit
  Bildern kalibrierten Decoder.
- **Fund 623, zur Kenntnis:** Die RMSNorm des Decoders schiebt den Index der
  rsqrt-Tabelle bis unter 32767; dort ist der Eintrag 23 bis 45 groß, der
  Kehrwert also auf 1 bis 2 % genau. Als Faktor je Token wiegt das wenig; die
  LayerNorm von Iris nimmt die genauere Grenze. Nicht geändert, denn es steht
  so in θ_v.
- **Fund 621:** `myl-types/src/geheimdatei.rs` (Sicherheitsaudit vom selben
  Tag) stand nicht in der Liste der Gleitkomma-Prüfung; die CI wäre rot
  geworden. Nachgetragen, ebenso `kernels/src/iris.rs` und `runtime/src/iris.rs`.
- `loader::spec_model_params` ist jetzt `pub(crate)`; Iris liest daraus die
  Skalen der Softmax.
- Kalibrierumgebung: `pillow==13.0.0` dazu (Bildprozessor der Vorlage, ohne
  torchvision).
- ⏳ **Offen:** θ_v und Konformitätsvektoren für die neuen Bausteine (vor dem
  Netz); S5 Einspeisung und mRoPE; S6 Client; S7 27B und 126B.

### v0.145.4 (2026-10-23): Die Kalibrier-Umgebung ohne bekannte Schwachstellen; kein `eval` auf Modellausgabe (Funde 600, 602, 614)

Aus dem Sicherheitsaudit vom 2026-10-23.

- ⚑ **Fund 600, `calibrate/requirements-fest.txt`:** `aiohttp` 3.14.3 (8
  Meldungen, drei hoch: unbegrenzter Speicher, keine Kopfzeit,
  Dateideskriptoren), `multidict` 6.7.1 und `accelerate` 1.14.0
  (Pfadüberschreitung beim Laden von Checkpoints, CVE-2026-69112) auf
  3.14.4, 6.9.1 und 1.15.0, dazu `yarl` 1.25.1, das `aiohttp` 3.14.4
  verlangt. Genau diese vier Pakete bewegten sich; torch, transformers und
  alles, was rechnet, blieb gleich. `requirements.txt` verlangt
  `accelerate>=1.15.0`. Danach meldet OSV für die feste Liste null Pakete;
  die fünf Einheitentests der Kalibrierung sind grün.
- **Fund 602, `scripts/python_einrichten.sh`:** Eine Umgebung galt als
  fertig, sobald transformers stimmte; das Heben aus Fund 600 wäre auf
  keiner Maschine angekommen. Jetzt muss jede Zeile von
  `requirements-fest.txt` stimmen, und die Meldung nennt die Abweichungen.
  **Gegenprobe:** Mit der alten Liste nennt die Prüfung alle vier
  Abweichungen, ein fehlendes Paket meldet sie als „fehlt“.
- ⚑ **Fund 614, `bench/qualitativ.py`:** Tokenlisten aus der Ausgabe der
  Laufzeit wurden mit `eval` gelesen. Die Laufzeit gibt auch den erzeugten
  Text aus; enthielt eine Zeile die gesuchte Zeichenfolge, war alles
  dahinter Python-Code, also Modellausgabe als ausgeführter Code. Jetzt
  `ast.literal_eval` und die Forderung „Liste ganzer Zahlen“.
- Die CI installiert torch und numpy für die Einheitentests jetzt in den
  Fassungen aus `requirements-fest.txt` statt in der jeweils neuesten.
- Geprüft und in Ordnung: die Kerne (jede `unsafe`-Stelle nennt ihre
  Grenze, davor stehen Zusicherungen), die Laufzeit (Umdeutung gemappter
  Bytes erst nach Prüfung von Länge, Ausrichtung und Byte-Reihenfolge).

### v0.145.3 (2026-10-23): Die Mietmaschine richtet Rust in der richtigen Fassung ein; Generalprobe auf dem heutigen Stand (Fund 598)

- 📌 **Fund 598: `karten-pruefen.sh` richtete auf einer frischen Maschine nie
  Rust ein.** Der Ablauf beginnt mit `--einrichten --nur-finden`, und der
  Rust-Teil stand hinter dem Ende von `--nur-finden`; der nächste Aufruf
  (ohne `--einrichten`) brach mit „cargo fehlt“ ab. Und ein vorhandenes
  cargo wurde nie nach seiner Fassung gefragt: Ein Mietabbild mit dem Rust
  der Distribution wäre erst beim Bau an `rust-version` gescheitert.
- Jetzt: Mit `--einrichten` läuft der Rust-Teil auch bei `--nur-finden`;
  mit rustup wird die Fassung aus `rust-toolchain.toml` ausdrücklich geholt
  (`rustup toolchain install`); ein älteres cargo ohne rustup bekommt
  rustup dazu, und ohne `--einrichten` bricht das Skript mit einem Hinweis
  ab, statt in den Bau zu laufen. Die Zeile `Rust: cargo <ist> (verlangt
  <soll>)` steht in jedem Lauf.
- **Proben** (ohne etwas zu installieren, auf beschnittenem Suchweg):
  `--einrichten --nur-finden` meldet `cargo 1.99.0`, Rückgabe 0; ohne
  `--einrichten` kein Rust-Teil; ein nachgebautes `cargo 1.80.0` ohne
  rustup bricht mit Rückgabe 2 ab, ebenso ein fehlendes cargo; der
  Vergleich ordnet 1.9.9 und 1.100.0 richtig ein.
- **Vor der Mietmaschine auf dem heutigen Stand belegt** (Rust 1.99.0,
  libp2p 0.57): Konformität 55/55 auf `reference`, `cpu-simd` und `metal`;
  Probelauf `--nachbau 6` kurz (0,6B, 60 Wörter: 392 Bündel) und lang
  (0,6B mit 400 Wörtern: 588 Bündel; `raven-35b-a3b-int4`: 62 148 Bündel),
  jeder Teil mit dem Abdruck der CPU, Arbeit auf 6, 3 und 6 Karten,
  Rückgabe 0, 3:26 min. Dieselben Zahlen wie am 2026-10-21. Bericht
  `SYSTEM/logs/karten/karten-cuda-2026-10-23-6x-nachbau.md`.

### v0.145.2 (2026-10-22): Eine Werkzeugkette für alle Kisten (kernels 0.88.2, runtime 0.103.1, pipeline 0.15.3)

- `rust-version` auf 1.99 in `kernels`, `runtime`, `pipeline`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  ⚑ **Dazu Code, und zwar mit Absicht:** Mit `rust-version = "1.99"` gibt Clippy
  zwei Regeln frei, die es bei 1.85 zurückhielt (`manual_is_multiple_of`,
  `chunks_exact` mit fester Größe). Festlegung des Projektinhabers:
  umschreiben, nicht abschalten. Hier: kernels 25 Zeilen mit `is_multiple_of` und 12 `as_chunks` in 14 Dateien, runtime 10 und 2 in 5 Dateien, pipeline 3 in 2 Dateien.
  `x % n == 0` wird `x.is_multiple_of(n)`, `chunks_exact(N)` wird
  `as_chunks::<N>()`. Für `n ≠ 0` bitgleich; jede Stelle mit variablem
  Teiler ist einzeln angesehen und vorher gegen null geschützt (bei
  `is_multiple_of(0)` stünde `x == 0`, wo `x % 0` abbricht).
- `runtime/tests/shardtraining.rs` fragt vor dem Artefakt die Erlaubnis
  `MYL_OHNE_ARTEFAKTE` ab, wie die übrigen Proben der Laufzeit (Klasse von
  Fund 597: wo das Artefakt liegt, rechnete sie auch neben einer Messung).
  Die Proben mit `#[ignore]` behaupten ihr Artefakt weiter hart; wer sie
  startet, will sie.
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen. Nach dem
  Umschreiben: Clippy mit `-D warnings` still (die Kerne über alle vier
  Merkmale), `cargo test` rc=0 in allen zehn berührten Kisten (die Kerne auch
  mit `cpu-simd`), Konformität mit `sparrow-0.6b-int8` 55/55, die vier
  Abdrücke unverändert (`cb696efd3817e575` über 24 Vektoren), Gleitkomma-Audit
  grün.

### v0.145.1 (2026-10-22): Das Gleitkomma-Audit kennt die Dateien von Stufe 4

- `tests/audit/test_no_float.py`: `podmodell.rs`, `fristmeldung.rs`,
  `gegenzeichnung.rs` und `zeitprobe.rs` aus `myl-types` im Konsenspfad.
  Die Vollständigkeitsprüfung des Audits hatte sie im ersten CI-Lauf nach
  ihrer Entstehung gemeldet („NICHT GELISTET“); alle vier sind ganzzahlig.
- `runtime/tests/teilablage.rs` springt mit `MYL_OHNE_ARTEFAKTE` ab, wie die
  übrigen Proben mit Artefakt; ohne die Variable bleibt ein fehlendes
  Artefakt ein Fehlschlag. Kein Code der Kiste geändert, `runtime` bleibt
  0.103.0.
- **Belege:** `test_no_float.py` PASSED, null Treffer; `teilablage` mit und
  ohne die Variable Rückgabe 0.

### v0.145.0 (2026-10-22): Ein Shard aus den Dateien seines Bereichs (runtime 0.103.0)

Stufe 4 des Dispatchers, Teilablage (P10); gefunden im Netzlauf.

- 📌 **Fund 589:** Die Kette teilte einem Knoten mit Teilablage nur Ebenen in
  seinem gehaltenen Bereich zu, aber der Lader verlangte jede Datei des
  Gewichtsmanifests, und der Shard brach beim Laden ab.
- `load_weights_bereich`, `load_model_bereich`,
  `load_model_fuer_netz_bereich` mit `Ladebereich`: Ein Tensor außerhalb des
  Bereichs, dessen Datei fehlt, bekommt einen Platzhalter
  (`Gewichtsdaten::Fehlt`, Form wie im Manifest); wer ihn liest, bekommt
  eine Panik mit Grund. Eine fehlende Datei im Bereich bleibt ein
  Ladefehler. Ohne Bereich lädt alles wie bisher.
- Beim Bau wird nichts mehr aus Daten gelesen, die ein Platzhalter nicht
  hat: Die QK-Norm prüft ihre Länge an der Form, die Zustandsschicht leitet
  die Faltungsskala nur aus vorliegenden Gewichten ab.
- **Belege:** `tests/teilablage.rs` am Probemodell: Ein Teilordner mit den
  Ebenen 0 bis 10 und einer mit 10 bis 28 samt Kopf laden und rechnen
  bitgleich zum vollen Modell, ohne Bereich lädt derselbe Ordner nicht, eine
  Lücke im Bereich fällt auf. ⚑ Am echten int4-35B einmalig nachgesehen:
  Ebenen 8 bis 18 (23 348 von 93 388 Manifestdateien) laden und rechnen
  bitgleich. `cargo test` rc=0.

### v0.144.0 (2026-10-21): Die Ebenenzeit (runtime 0.102.0)

Stufe 4 des Dispatchers, vorgezogen und geplant auf Bitte des Projektinhabers (2026-10-21): Ein Pod verteilt seine Ebenen nach Kapazität und gemessener Ebenenzeit seiner Mitglieder statt gleich. Hier das Paket P1.

- **`runtime/src/ebenenzeit.rs`** (neu): Median eines Decode-Tokens durch
  alle Ebenen ohne Kopf, über 16 Token nach 4 zum Anwärmen, je Ebene in
  Mikrosekunden aufgerundet; dazu derselbe Lauf mit Kopf
  (`forward_token`) und daraus der Kopf in Ebenenzeiten. Ganze Zahlen; im
  Gleitkomma-Audit geführt.
- 📌 **Gemessen auf diesem Mac** (`reference+cpu-simd+metal`): Sparrow 0.6B
  323 µs je Ebene, Kopf wie 5 Ebenen; Raven 35B A3B int4 907 µs je Ebene,
  Kopf wie 1 Ebene. Der Kopf des Raven belegt nach Bytes 4 Ebenen, kostet
  aber nur eine; Platz und Zeit des Kopfs zählen deshalb getrennt.
- **Belege:** `median_und_je_ebene_sind_ganzzahlig`,
  `die_ebenenzeit_des_ankers`; `cargo test` und Clippy rc=0.

### v0.143.0 (2026-10-21): Die Modelle heißen nach Tieren (runtime 0.101.1, kernels 0.88.1, pipeline 0.15.2)

**Festlegung des Projektinhabers:** Jedes Projektmodell heißt nach einem
Tier, passend zu seiner Fähigkeit, mit Größe und Speicherform als
Anhängsel, auch bei int8; kein Name trägt mehr „Myelith“. Das 126B heißt
Vulture (Wahl des Projektinhabers), die übrigen sind Vögel, nach Fähigkeit
geordnet (das 27B ist stärker als das 35B):

| bis zum 2026-10-21 | jetzt | Anzeige |
|---|---|---|
| `myelith-0.6b` | `sparrow-0.6b-int8` | Sparrow 0.6B - INT8 |
| `myelith-4b` | `pigeon-4b-int8` | Pigeon 4B - INT8 |
| `myelith-8b` | `jay-8b-int8` | Jay 8B - INT8 |
| `myelith-30b-a3b` | `magpie-30b-a3b-int8` | Magpie 30B A3B - INT8 |
| `myelith-35b-a3b`, `-int4` | `raven-35b-a3b-int8`, `-int4` | Raven 35B A3B - INT8 / INT4 |
| `myelith-27b` | `crow-27b-int8` | Crow 27B - INT8 |
| `myelith-125b-a6b` | `vulture-126b-a7b-int8` | Vulture 126B A7B - INT8 |

Versuchsartefakte tragen ihr Anhängsel statt `-int8` (`crow-27b-ternaer`,
`raven-35b-a3b-gemischt-50`).

- ⚑ **Abdrücke, Register und Konformitätsvektoren bleiben gültig.** In
  keiner gehashten Datei eines Artefakts steht der Name; `model_config.json`
  trägt nur die Variante (`35b-a3b`). Das Bauskript leitet sie jetzt aus
  dem Namen ohne Tier und ohne `-int8` ab (`variante`), und eine Probe
  hält für jeden int8-Eintrag fest, dass dieselbe Variante herauskommt wie
  vor der Umbenennung; ein Neubau bleibt also bitgleich. Die Netzliste
  trägt dieselben Abdrücke unter den neuen Namen.
- **Katalog:** Schlüssel umbenannt, `anzeigename` nach der neuen Regel,
  dazu `frueher` mit dem alten Namen. Der Klient benennt damit beim Start
  Verzeichnisse und Einstellungen um; die Messdateien unter
  `BENCHMARKS/Inferenz/results/` behalten den alten Namen als Aufzeichnung,
  und Übersicht und Modellkarte finden sie darüber. Ein alter Name in
  `INTEGER_LLM_MODEL` gilt nicht still weiter, sondern endet mit „heisst
  seit dem 2026-10-21 …“.
- **Flash-Next:** `vulture-126b-a7b-int8` mit Variante `126b-a7b`, nach der
  Regel des Bauskripts mit Einbettung und Kopf gezählt (125,74 und 7,31
  Mrd.). Der frühere Eintrag `myelith-125b-a6b` zählte ohne beide; gebaut
  war er nie. Der Kommentar, der ihn als „nach der Regel des Bauskripts“
  auswies, war falsch und ist ersetzt.
- ⚑ **Selbst gebaute Modelle** (Festlegung des Projektinhabers): Das
  Bauskript fragt nach einem Namen und hängt Größe und Speicherform an
  (`otter` wird `otter-126b-a7b-int4`); ohne Eingabe wählt es ein Tier per
  Zufall (`SystemRandom`), das weder ein Projektmodell noch ein vorhandenes
  Artefakt trägt. Wer einen ganzen Namen mit Speicherform gibt, behält ihn.
  Die Erkundung meldet die Anhängsel (`anhaengsel`) und ein gezogenes Tier
  als Vorschlag.
- **Umbenannt:** die Verzeichnisse unter `artifacts/` und `scale_packs/`,
  Schlüssel in `model_configs.py` und `REGISTER.json`, die Vorgaben der
  Werkzeuge (`calibrate`, `bench`, `conformance/run.sh`,
  `build_vierbit.sh`, `build_flashnext.sh`), Proben und Diagnosewerkzeuge;
  in `runtime` die Vorgabe in `paths.rs` und die Proben, in `kernels` und
  `pipeline` Kommentare und Proben. Die Probe der Netzliste verlangt jetzt
  `raven-35b-a3b-`. Die Dateinamen der Pipeline-Konfigurationen
  (`pipeline_4node_myelith-30b-a3b.json`) bleiben.
- **Belege:** `test_gittervergleich.py` mit der neuen Probe
  `test_namen_sind_tier_und_anhaengsel` (Variante je int8-Eintrag wie
  vorher, jeder Katalogname nach dem Muster und ohne „myelith“, kein Tier
  eines Projektmodells im Zufall, 33 verschiedene Tiere aus 40 Zügen);
  Clippy, `cargo test` und Konformität siehe Mitschrieb.

### v0.142.0 (2026-10-21): Eine Python-Umgebung für alle Bauarten, transformers 5.18.0

**Entscheidung des Projektinhabers** nach den Gegenproben unten: Die zweite
Umgebung, die es seit v0.136.0 nur für die Bauart mit vier Strömen gab
(`.venv-flashnext`, transformers 5.18.0), wird die einzige. Auf der
Mietmaschine wird damit eine Umgebung eingerichtet, nicht zwei.

- **Gegenproben vor der Entscheidung**, alle mit transformers 5.18.0:
  - **Das 0,6B aus dem Skalenpaket ist bitgleich:** 636 von 636 Dateien wie
    das vorhandene Artefakt, das Manifest wie im Register, der Tokenizer
    byte-gleich kopiert. Der Bau aus dem Klon rechnet die Aktivierungsstatistik
    nicht neu; die Fassung wirkt also nur auf das Laden der Gewichte.
  - **Das Laden ist Tensor für Tensor bitgleich** zu 5.15.0, an kleinen
    Zufallsmodellen jeder Bauart: Qwen3-Gemisch mit einzeln abgelegten
    Experten (25 von 25, die Form des 30B), Qwen3.5 dicht (56 von 56, die
    Form des 27B), Qwen3.5-Gemisch mit einzeln und mit verschmolzen
    abgelegten Experten (je 72 von 72; die verschmolzene Form mit den Achsen
    des echten 35B, `[E, 2·moe, hidden]` und `[E, hidden, moe]`).
  - huggingface-hub 2.2.0 (Erkundung, Download) und datasets 5.1.0 (WikiText
    mit genau der geprüften Summe) arbeiten wie vorher; alle sieben
    Python-Proben bestehen.
- **Geändert:** `calibrate/requirements-fest.txt` trägt den Satz der zweiten
  Umgebung (anders sind transformers, datasets, huggingface-hub, tokenizers
  und die HTTP-Bibliothek des Hub; torch bleibt 2.13.0);
  `requirements-flashnext.txt` entfällt; `requirements.txt` verlangt
  transformers ab 5.18.0. Die Bauart mit vier Strömen nimmt `.venv`,
  das Bauskript richtet keine zweite Umgebung mehr ein, die Meldung von
  `main.py` nennt `.venv`.
- 📌 **`python_einrichten.sh` erkennt eine veraltete Umgebung.** Bisher galt
  eine Umgebung als fertig, sobald alles importierte; nach dem Heben der
  festen Fassungen wäre die alte liegen geblieben, und der Bau wäre still mit
  ihr gelaufen. Jetzt muss die Fassung von transformers der festen
  entsprechen, sonst wird neu angelegt. Belegt am echten Wechsel: „veraltete
  Umgebung … (transformers soll 5.18.0), sie wird neu angelegt“, danach
  Python 3.12.13, torch 2.13.0, Rückgabe 0. `--flashnext` wird noch
  angenommen und tut nichts mehr.
- ⚠️ **Was sich ändern kann:** Gleitkomma-Vergleichsmessungen (BF16) rechnen
  den Vorwärtspass von transformers, und der hat sich zwischen den Fassungen
  geändert (unter anderem die Drehung unter Metal). Eine Referenz aus 5.15.0
  gegen eine aus 5.18.0 ist deshalb kein Vergleich; beide Seiten mit
  derselben Fassung messen. Eine volle Kalibrierung ohne Skalenpaket betrifft
  das ebenso, also nur neue Artefakte.

### v0.141.1 (2026-10-21): Fund 574 behoben; das Bauskript schlägt vier Bit nur mit ganzen Gruppen vor und behält bei jedem Gemisch den aktiven Anteil im Namen (Fund 574)

Vor dem Bau von Flash-Next auf der Mietmaschine, damit dort kein bekannter
Fehler der Kalibrierung mitläuft.

- ⚑ **Fund 574 behoben** (`calibrate/src/gittervergleich.py`, `h_skala`):
  Die Skalen von `h` werden auf den vier häufigsten Experten einer Ebene
  gesucht. Die Zählung hat feste Länge 256; bekamen weniger als vier
  Experten Token, rückten Experten mit null Token nach, bei einem Modell mit
  acht Experten auch Experte 163, den es nicht gibt (`KeyError`). Jetzt
  zählen nur Experten mit Token. Bei echten Modellen ist die Auswahl
  dieselbe wie vorher, in derselben Reihenfolge, denn dieselbe Sortierung
  läuft über dieselbe Zählung; die gebauten Artefakte bleiben also gültig.
  Eine Ebene ganz ohne Token bricht mit klarem Grund ab.
- **Vier Bit nur mit ganzen Gruppen** (`tools/modell_bauen.py`,
  `vierbit_hindernis`): Die Speicherform `vierbit_g128` braucht je 128
  Eingänge einen Betrag; gate und up lesen `hidden_size` Spalten, down
  `moe_intermediate_size`. Am kleinen Modell (64 und 32) schlug das Skript
  `-int4` vor, und der Bau brach erst nach dem Mitschnitt ab. Jetzt sagt
  es vorher „Vier Bit geht hier nicht (hidden_size 64 ist kein Vielfaches
  der Gruppe 128); es baut int8“, und die Erkundung für Konsole und Fenster
  meldet `"vierbit": false`. `--format int4` nennt denselben Grund. Alle
  echten Gemische gehen weiter: 30B-A3B (2048, 768), 35B-A3B (2048, 512),
  Flash-Next (2560, 640).
- 📌 **Und ein zweiter, älterer Fehler daneben:** `namensvorschlaege` bekam
  nur einen Schalter, „Gemisch in vier Bit“. Ein Gemisch, das nicht in vier
  Bit geht, verlor damit sein `-a<aktiv>b`; das kleine Modell hieß
  `myelith-0.001b` statt `myelith-0.001b-a1b`. Gemisch und vier Bit sind
  jetzt zwei Angaben.
- **Belege:** `test_gittervergleich.py` mit zwei neuen Proben, 11 von 11:
  `h_skala` mit acht Experten, von denen zwei Token bekommen, findet die
  eingebauten Shifts (−11, −2) wieder (die alte Auswahl wäre `[2, 5, 0, 163]`
  gewesen); das Bauskript schlägt vier Bit nur mit ganzen Gruppen vor, seine
  Gruppe ist die des Gitterbaus, die Namen der drei Fälle stimmen.
  Erkundung echt gefahren: das kleine Flash-Next-Modell nur noch
  `myelith-0.001b-a1b` (int8), `Qwen/Qwen3-30B-A3B` weiter
  `myelith-30b-a3b-int4` und `myelith-30b-a3b`. `test_bauarten.py`,
  `test_modell_link.py`, `test_modellkonfig_vollstaendig.py` grün.

**Ältere Einträge** (v0.141.0 und davor, 193 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- INTEGER_LLM/README/README.md`.
