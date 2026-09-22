# integer-llm

> **Version:** 0.91.0 (θ_v 0.21.0; kernels 0.65.0, runtime 0.62.0, pipeline 0.15.1)
> **Datum:** 2026-09-22
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
(Whitepaper Kap. 6.2). Referenzmodell ist Qwen3-0.6B (`myelith-0.6b`, **W8A16**: Gewichte
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
nach `artifacts/myelith-0.6b/`. Dauert einige Minuten und braucht rund
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
    artifacts/myelith-0.6b "Die Hauptstadt von Frankreich ist" 10
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

Fährt 48 eingefrorene Testvektoren gegen das Referenz-Backend, von
einzelnen Kernen über ganze Layer bis zu vollständigen
Prompt-Durchläufen. **48/48 ist die Erwartung, nicht das Ziel.** Weicht
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
./target-shared/release/examples/tokenzeilen artifacts/myelith-4b text.txt > korpus.txt
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
./target-shared/release/examples/formprobe artifacts/myelith-4b
```

Zählt zehn Formulierungen an sechs bekannten Personen durch. Gemessen:
`{} wurde geboren in der Stadt` trifft **sechs von sechs**,
`Der Geburtsort von {} ist` **null von sechs**. Die zweite war die Form
aller früheren Läufe; das Modell setzt dort mit `in der Stadt …` fort
statt mit einem Namen.

### 3. Die Eintrittsprüfung

```sh
./target-shared/release/examples/aufbauprobe artifacts/myelith-4b proben.tsv || exit 1
```

Sie liest die **echte** Probendatei und prüft, ob an jeder Messstelle
ein Eigenname steht. 📌 Ohne sie hat dieses Projekt vier Tage lang
Läufe gefahren, die an einer Stelle gemessen haben, an der der erwartete
Token gar nicht stehen konnte.

### 4. Der Lauf

```sh
./target-shared/release/trainingsguete artifacts/myelith-4b korpus.txt \
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
./target-shared/release/examples/kopf_einsetzen \
  artifacts/myelith-4b kopf.bin artifacts/myelith-4b-dresden

./target-shared/release/examples/fortsetzen \
  artifacts/myelith-4b-dresden 6 \
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
    -- ../artifacts/myelith-0.6b --batch ../conformance/vectors
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

### v0.91.0 – 2026-09-22 (eine Norm, die `1 + weight` rechnet, und sechs echte Funde davor, die alle nebensächlich waren)

**Das grosse Modell gab Kauderwelsch aus. Die Ursache stand in der
ersten Stufe der ersten Ebene.**

⛔️ **Fund 423: `Qwen3_5MoeRMSNorm` rechnet `x * (1 + weight)`**, und ihr
Gewicht ist mit `torch.zeros` angelegt. Der Export nahm das Gewicht, wie
es dasteht, und normierte damit jede Ebene mit einer Zahl um null statt
um eins.

⚠️ **Im selben Modell gilt es nicht überall.** `RMSNormGated`, die Norm
des rekurrenten Zweigs, ist mit `torch.ones` angelegt und rechnet ohne
Versatz. Am Mittelwert der Gewichte abzulesen: `input_layernorm` 0,031,
`linear_attn.norm` 0,884.

Behoben über `rms_norm_offset` je Modell, angewandt auf
`input_layernorm`, `post_attention_layernorm`, `self_attn.q_norm`,
`self_attn.k_norm` und die finale Norm. Für alle bisherigen Modelle ist
er null. Gegen das offizielle Modell gemessen, Ebene 0:

| Stufe | ohne Versatz | mit Versatz |
|---|---|---|
| `input_layernorm` | 0,9665 | **0,0017** |
| `in_proj_qkv` | 0,9378 | **0,0018** |
| torgesteuerte Norm | 2,3471 | **0,0021** |
| Zweigausgang | 2,4886 | **0,0038** |

📌 **Warum sechs Funde davor nötig waren und keiner half.** Der Fehler
stand vor allem anderen und wirkte gleichmässig. Jede Stufe liess sich
gegen eine eigene Referenz halten und war richtig; falsch war die
Eingabe, die beide teilten. Der Gleitkomma-Nachbau trug denselben
Fehler wie der Rechenpfad und bestätigte ihn deshalb. **Ein Nachbau
prüft die Umsetzung, nicht die Annahme.** Gefunden hat ihn erst der
Lauf gegen das offizielle Modell.

**Die sechs Funde davor sind echt und bleiben behoben:**

- ⛔️ **417:** SiLU als Tabelle über `x` statt als Zerlegung
  `x * sigmoid(x)`. Das Eingangsraster 1/64 löschte 7016 von 8192
  Faltungskanälen aus, weil `q` und `k` dort bei zwei Rasterschritten
  liegen. Neu: `integer_math::silu_zerlegt`. 📌 *Eine Tabelle über `x`
  quantisiert `x`; eine, die nur einen Faktor liefert, lässt `x`
  unberührt.*
- ⛔️ **418:** Eine Ausgangsskala für 32 Wertköpfe, die um Faktor 1050
  auseinanderliegen, mit einer Normierung dahinter. Jetzt eine Skala je
  Kopf, aus dessen eigenem Grösstwert. 📌 *Eine Normierung hinter einer
  geteilten Skala ist ein Rauschverstärker.*
- ⛔️ **419:** `rmsnorm_i16` ohne Epsilon. Für den Residualstrom
  vernachlässigbar, für einen Wertkopf mit Effektivwert 1,9e-6 der ganze
  Nenner. Neu: `rmsnorm_i16_mit_eps`, gespeist aus `rms_norm_eps` je
  Modell. 📌 *Eine Annahme über Grössenordnungen gilt für den Zweig, für
  den sie geprüft wurde.*
- ⛔️ **420:** Die kalibrierte Skala des Modulausgangs auf den
  Zwischenstand davor angewandt; vierzig Werte je Token in der
  Sättigung. Jetzt eine harte Schranke `sqrt(d) * max|gamma|`, ganz
  ganzzahlig. 📌 *Eine kalibrierte Skala gilt an der Stelle, an der
  gemessen wurde.*
- ⛔️ **421:** Eine Faltungsskala für `q`, `k` und `v`, obwohl `v`
  zwanzigmal grösser ist und `q` und `k` gleich danach auf
  Einheitslänge gebracht werden. Jetzt eine Schranke je Kanal, im Lader
  aus dem Artefakt gerechnet. 📌 *Ein Tensor ist noch keine gemeinsame
  Grösse.*
- ⛔️ **422:** Das Achtsamkeitstor mit `attn_out_frac` statt `q_frac`
  gelesen, vier Bit daneben. Aus einem nahezu binären Schalter wurde
  eine Schwankung zwischen 0,27 und 0,73. 📌 *Ein Tor, das nicht mehr
  schaltet, sieht aus wie ein Tor.*
- ⚠️ **424:** `faltungsskala_ergaenzen` läuft nur im
  Kalibrierungspfad, die Skala fehlt also im Skalenpaket. Die Forderung
  im Lader entfällt, da die Schranke seit 421 ohnehin abgeleitet wird.

**Messreihe des rekurrenten Zweigs, Ebene 0, gegen die Referenz:**

| Messgrösse | Start | nach 417 | nach 421 |
|---|---|---|---|
| Rekurrenzausgang | 0,3446 | 0,1270 | **0,0169** |
| torgesteuerte Norm | 0,7004 | 0,2199 → 0,0809 | **0,0190** |

287 Kernproben grün.

### v0.90.0 – 2026-09-21 (die schlechteste Zahl der Reihe war ein Messfehler, und die beste Nachricht kommt mit einer schlechten)

`runtime` **0.61.0**. Der Neubau nach dem θ_v-Sprung hat das
Skalenpaket des 0,6B widerlegt, und die Folgen reichen weiter als
erwartet.

#### ⛔️ Die +4,48 % des 0,6B waren ein Artefakt

| Artefakt | Perplexitaet | gegen BF16 31,86 | Trainingsprobe |
|---|---|---|---|
| alte Paketskalen | 33,29 | **+4,48 %** | Verlust 8,00 nach 0,47 |
| **berichtigte Skalen** | **31,34** | **−1,64 %** | Verlust 7,99 nach 7,90 |

⚑ **Mit korrekt gemessenen Aktivierungsskalen liegt das kleinste Modell
UNTER der Gleitkomma-Referenz.** Sechs Prozentpunkte Unterschied, weit
jenseits der Auflösung von 435 Positionen.

⚠️ **Nur das 0,6B ist betroffen.** Das 4B reproduziert sein Paket exakt
(0 von 542, gegen eine echte Vollkalibrierung), das 8B stammt von
heute. Fuer das 30B steht der Beleg aus, solange sein Neubau laeuft.

📌 **Eine Ordnung, die auf einer falschen Zahl beruhte, wird nicht durch
eine neue Ordnung ersetzt, sondern gestrichen.** Die Aussage ueber die
Monotonie der Reihe faellt damit in beide Richtungen.

#### ⛔️ Und dieselbe Berichtigung bricht die Trainingsprobe

Derselbe Code, andere Skalen, umgekehrtes Ergebnis. Der Mechanismus ist
sichtbar: Statt 3,74 Millionen bewegen sich nur noch **1,02 Millionen**
Gewichte. Die groeberen (richtigen) Skalen runden Gradienten auf null.

⚠️ **Die Schwelle der Probe wird NICHT angepasst.** Sie sagt etwas
Richtiges: Der Trainingspfad haengt an feinen Aktivierungsskalen, und
das stand bisher hinter einem veralteten Paket verborgen. Eine Schwelle
zu verschieben hiesse, die Zahl zu waehlen, die schmeichelt.

#### Was belegt ist und was nicht

⚑ **Belegt:** Zwei Vollkalibrierungen sind byteweise gleich, also ist
die Kalibrierung deterministisch. Das 4B reproduziert. Der A/B ueber
beide Artefakte gibt die Tabelle oben.

⛔️ **Nicht belegt, und die erste Zuschreibung war falsch:** Die
Gammaentzerrung (Fund 336) erklaert es **nicht**. Sie betrifft einen
Kanal in der letzten Ebene, die Abweichungen verteilen sich ueber die
Ebenen 0 bis 27. Entzerrung und Paket stammen aus demselben Commit, und
die einzige spaetere Aenderung an ihr war ein Symbolwechsel im
Kommentar. **Die Herkunft der alten Paketskalen liess sich aus der
Versionsgeschichte nicht klaeren**, und das steht hier so, statt eine
passende Geschichte zu bekommen.

⚑ **Nebenbei belegt:** Die Entzerrung ist nicht optional. Abgeschaltet
bricht der Export mit „1 Wert wuerde saettigen, groesster Betrag 192 >
127".

#### 📌 Und eine Tautologie, zweimal gemeldet

Die erste Pruefung „4B, 8B, 30B: 0 Abweichungen" verglich ein **aus dem
Paket gebautes** Artefakt mit seinem eigenen Paket. **Eine Null, die
nichts pruefen kann, sieht aus wie eine Entwarnung.** Erst eine echte
Vollkalibrierung traegt die Aussage.

### v0.89.0 – 2026-09-21 (ein Skalenpaket, das eine aeltere Rechnung einfror, und niemand haette es gemerkt)

`kernels` **0.64.0**, `runtime` **0.60.0**. Der θ_v-Sprung machte alle
Artefakte ungueltig, und der erste Neubau legte einen Fund frei, der
nichts mit der Zustandsschicht zu tun hat.

#### ⛔️ Der Befund

Das neu kalibrierte `myelith-0.6b` wich in **105 von 422 Skalen** von
seinem Skalenpaket ab, und zwar **alle 105 nach oben**.

⚑ **Ein Skalenpaket ersetzt den einzigen nichtdeterministischen
Bauschritt** (Fund 32). Weicht es von dem ab, was eine Neuberechnung
ergaebe, ersetzt es ihn nicht mehr, sondern **friert eine aeltere
Rechnung ein**, und jeder Bau uebernimmt sie.

⛔️ **Und das faellt von allein nie auf**, weil jeder Bau das Paket
nimmt: **Der Weg, auf dem man es merken wuerde, wird nie gegangen.**
Sichtbar wurde es nur, weil der θ_v-Sprung das Paket ungueltig machte
und ein Bau ausnahmsweise voll kalibrierte.

#### Die Ursache, der Reihe nach ausgeschlossen

| Verdacht | Befund |
|---|---|
| die neuen `stats.py`-Haken | nein, alle neuen Schluessel beginnen mit `linear_attn.` und treffen in einem dichten Modell nichts |
| Kalibrierkorpus geaendert | nein, die Cache-Datei ist unveraendert |
| Nichtdeterminismus | nein, **zwei Vollkalibrierungen sind byteweise gleich**, 0 Unterschiede im ganzen Artefakt |
| die Kalibrierung insgesamt | nein, **das 4B reproduziert sein Paket exakt**, 0 von 542 |

⚑ **Es ist die Entzerrung der Normgewichte (Fund 336).** Sie verschiebt
Gewichtsmasse aus einer Norm in die Folgematrix, wenn ein Kanal die
int8-Grenze sprengt, und sie laeuft **vor** der Aktivierungsstatistik.
Das `myelith-0.6b` hat **einen** solchen Kanal, das `myelith-4b`
**keinen**. Das Paket des 0,6B stammt damit aus der Zeit vor diesem
Schritt; das des 4B ist unberuehrt.

📌 **Ein systematisches Vorzeichen ueber 105 Werte ist kein Rauschen,
sondern ein Hinweis stromaufwaerts.** Alle groesser, keine kleiner: Das
war der Faden.

#### Die Probe, die das kuenftig faengt

`tests/test_skalenpaket_reproduzierbar.py` rechnet die Skalen neu und
haelt sie gegen das Paket.

⚠️ **Sie gehoert nicht zu den schnellen Proben**, denn sie rechnet den
Korpus durch (rund zwei Minuten beim 0,6B). Sie gehoert vor die
Verwendung eines Pakets und in jeden θ_v-Sprung.

📌 **Eine Abkuerzung, die nie gegen den langen Weg gehalten wird, ist
keine Abkuerzung, sondern eine zweite Wahrheit.**

#### ⚑ Und die Konformitaetsvektoren wurden bewusst NICHT sofort erneuert

Sie fielen nach dem Neubau auf 17/48, und sie werden bei einem
θ_v-Sprung planmaessig miterzeugt. **Sie waren aber das Einzige, was
den Unterschied sichtbar machte.** Sie zu ueberschreiben, bevor der
Befund verstanden ist, haette ihn zugedeckt.

### v0.88.0 – 2026-09-21 (θ_v 0.21.0: zwei neue Tabellen, und eine davon passte nicht in ihren Typ)

`kernels` **0.64.0**, **θ_v 0.21.0**. Die Zustandsschicht braucht
`sigmoid` und `softplus`; beide sind jetzt Teil des Vertrags, und die
zweite hat den Entwurf verschoben.

⛔️ **Alle Artefakte sind damit ungueltig und werden neu gebaut.** Der
Lader vergleicht die θ_v-Fassung zeichengenau, und das Skalenpaket ist
an sie gebunden. **Das ist der Preis und er war eingeplant.**

#### ⚑ Beide Tabellen liegen in JEDEM Artefakt

Auch in einem ohne Zustandsebenen. **θ_v ist ein Vertrag:** Eine
bedingte Tabelle hiesse, dass „θ_v 0.21.0" zweierlei bedeutet. Kosten:
32 KiB fuer Sigmoid, 32 KiB fuer den Softplus-Rest.

#### ⛔️ Der Zerfall braucht 30 Bruchbits, und die passen nicht

`g = exp(-exp_A * softplus(a + dt_bias))`, und `exp_A` reicht bis
**105,2**. Bei 8 Bruchbits ist die kleinste Stufe 1/256, mal 105 also
0,41, und `g` spraenge um den Faktor 0,66.

Gemessen ueber alle 30 Zustandsebenen, mit dem Kriterium
`|dg| * min(N, 1/(1-g)) < 2^-8` bei `N = 262144`:

| out_frac | gewichteter Fehler | |
|---|---|---|
| 8, 14 | 1,0 | zu knapp |
| 20 | 4,2e-1 | zu knapp |
| 26 | 8,5e-3 | zu knapp |
| **30** | **4,2e-4** | traegt |

⚠️ **Und dann passte die Tabelle nicht in `int32`.** Festkomma mit 30
Bruchbits reicht dort nur bis zum Wert **2**; Softplus geht ueber den
gemessenen Eingangsbereich bis 32. Der Bau brach mit
`struct.error: 'i' format requires -2147483648 <= number`.

#### ⚑ Die Zerlegung loest es exakt

```
softplus(x) = max(x, 0) + log(1 + exp(-|x|))
                grob          fein, aus der Tabelle
```

Der zweite Term liegt **immer** in `(0, 0.693]`, braucht also
`0,693 * 2^30 = 7,4e8` und passt bequem. **Und er ist genau der Teil,
der die Feinheit braucht:** Fuer stark negative `x` ist er der ganze
Softplus. Der grobe Teil ist exakt und kostet keine Tabelle.

📌 **Eine Umformung, die den feinen vom groben Teil trennt, ist mehr
wert als ein breiterer Typ.**

⚑ **Die Tabelle laeuft ueber `|x|` und ist halb so lang** (8192 statt
16384). Oberhalb ihres Endes ist der Rest kleiner als eine letzte
Stelle (`exp(-32) = 1,3e-14` gegen `2^-30 = 9,3e-10`) und damit null.

**Gemessen am gebauten Artefakt:** groesster Abstand zur Referenz ueber
den gemessenen Eingangsbereich **0,50 letzte Stellen von 2^-30**, also
das theoretische Minimum einer halben Stufe.

#### Die Proben dazu

⚑ **Die schaerfste braucht keine Vergleichswerte:**
`softplus(x) - softplus(-x) = x`, exakt. Der Resthalt faellt heraus,
weil beide Seiten denselben Betrag nachschlagen. **Stimmt sie nicht,
ist die Zerlegung falsch umgesetzt**, ganz gleich wie gut die Tabelle
ist.

**Gegengeprueft mit zwei Mutationen:** `max(x,0)` vergessen laesst vier
Proben fallen, den Betrag vergessen zwei.

279 Kernproben gruen.

#### Und zwei Stellen, die ein Format doppelt wussten

Export und Paketleser hatten `int16` fest verdrahtet, im Packformat und
im Feld `dtype`. Das ging gut, solange jede Tabelle int16 war. ⚑ **Die
Breite kommt jetzt aus θ_v**, an beiden Stellen.

### v0.87.0 – 2026-09-21 (eine Ebene sagt jetzt selbst, womit sie mischt)

`kernels` **0.63.0**, `runtime` **0.59.0**. Die Laufzeit kann hybride
Modelle tragen: 30 rekurrente Ebenen und 10 mit voller Achtsamkeit im
selben Modell.

#### ⚑ `Mischer`, und warum kein zweites `Option`-Feld ging

`TransformerLayer` traegt statt der vier Achtsamkeitsmatrizen jetzt ein
Feld `mischer`, entweder `Achtsamkeit` oder `Zustand`.

⛔️ **Der naheliegende Weg war versperrt.** Ein zweites `Option`-Feld
neben den Achtsamkeitstensoren ginge nicht, denn eine Zustandsebene hat
**kein** `q_proj`; man muesste auch diese optional machen, und dann
waeren „beides" und „keines" darstellbar. Genau das Argument, mit dem
`Feedforward` schon ein Aufzaehlungstyp ist.

⚑ **Der Zugriff bricht ab statt auszuweichen.** `layer.achtsamkeit()`
knallt bei einer rekurrenten Ebene mit einer Meldung, die den Grund
nennt. Wer dort landet, hat die Ebenenart nicht geprueft, und eine
stillschweigende Ersatzantwort waere eine falsche Zahl ohne Meldung.

#### Belege fuer den Umbau

| | |
|---|---|
| Feldzugriffe umgestellt | 42 in sieben Dateien, mechanisch |
| Laufzeitproben | **126 gruen** |
| Kernproben | **284 gruen** |
| **Konformitaet 0,6B** | ⚑ **48/48** |

⚑ **Die 48/48 sind der eigentliche Beleg.** Proben zeigen, dass es
uebersetzt und laeuft; die Konformitaetsvektoren zeigen, dass ein
vorhandenes Modell **dieselben Zahlen** rechnet. Bei einem Umbau, der
den Trainingspfad mitnimmt, ist das der Unterschied zwischen „gruen"
und „unveraendert".

#### Der Zustandsspeicher, und was ihn vom KV-Speicher unterscheidet

⚑ **Er waechst nicht mit der Folge.** Der KV-Speicher legt je Position
einen Schluessel und einen Wert ab; der Zustand einer rekurrenten Ebene
ist eine Matrix fester Groesse, ganz gleich ob zehn oder
zweihunderttausend Token gelaufen sind. **Das ist der Grund, warum
diese Bauart lange Folgen traegt**, und es steht als Probe da und nicht
nur als Satz.

⚑ **Belegt werden nur die rekurrenten Ebenen.** Ein Zustand je Kopf
sind 128 KiB, bei 32 Koepfen 4 MiB je Ebene; fuer alle 40 zu belegen
kostete 40 MiB fuer nichts. Ein Platztisch bildet den absoluten
Ebenenindex ab, und der Zugriff auf eine achtsame Ebene bricht ab.

Sechs Proben, darunter „der Speicher waechst nicht mit der Folge" und
zwei Trennproben (Koepfe, Ebenen).

### v0.86.0 – 2026-09-21 (das Tor braucht keinen neuen Kern, und der Massstab hat drei eigene Fehler gefunden)

`kernels` **0.62.0**. Die torgesteuerte Norm und die GQA-Auffaecherung
sind geprueft, und beide brauchen **keinen neuen Rechenweg**.

#### ⚑ Was das Projekt schon hat

Die Vorlage rechnet „Norm zuerst, Tor danach":
`rmsnorm(x) * gamma * silu(z)`. Das ist die vorhandene RMSNorm, gefolgt
von der vorhandenen `silu_produkt`. Die GQA-Auffaecherung (16
Schluesselkoepfe auf 32 Wertkoepfe) ist reine Indexierung.

⚠️ **Damit ist die Zustandsschicht rechnerisch vollstaendig.** Was
bleibt, ist Verdrahtung, kein neues Rechnen.

#### ⛔️ `silu_produkt` hatte eine latente Luecke, und sie wird jetzt scharf

Die Funktion lief ueber `gate.iter().zip(up.iter())` **ohne
Laengenpruefung**. Das war harmlos, solange beide Seiten aus derselben
Matrix mit derselben Zwischengroesse kamen.

⚠️ **Die torgesteuerte Norm ruft sie mit `z` aus einer Projektion und
dem Rekurrenzausgang**, also mit zwei Groessen aus verschiedenen
Quellen. Stimmt eine Kopfzahl nicht, liefert `zip` still ein zu kurzes
Ergebnis (Fund 346). 📌 **Eine Annahme, die heute stimmt, gehoert
hingeschrieben, bevor jemand den zweiten Aufrufer baut.**

#### ⛔️ Drei Fehler in meinem eigenen Massstab

**Erstens: Stellen ohne Vollausschlag sind keine Aussage.** Die
torgesteuerte Norm sah mit 4,50 Stellen achtmal schlechter aus als die
Rekurrenz mit 0,55. Ihre Ausgabe ist aber auch zehnmal groesser:
**0,18 Prozent gegen 0,21 Prozent**, also minimal besser.

**Zweitens: ein Pruefeingang ohne die gemessene Amplitude prueft eine
andere Schicht.** Die Faltungsprobe lief mit `randn`, also bei etwa
drei. Die Aktivierungsstatistik misst fuer `in_proj_qkv` in Ebene 0
**39,0**, also ein Zehntel der echten Amplitude. Der Pruefeingang kommt
jetzt aus `scales.json`.

**Drittens: eine erfundene Schwelle ist keine Herleitung.** „Unter
einem Prozent" stand da, weil es sich gut anhoert. Der Fehler dieser
Stufe kommt aber fast ganz aus der int8-Quantisierung der Gewichte, und
die ist die Grundlage des ganzen Projekts.

⚑ **Das Urteil misst jetzt gegen den Boden:** dieselbe Rechnung mit
quantisierten Gewichten, sonst exakt.

| | Stellen | vom Vollausschlag |
|---|---|---|
| Boden (nur int8-Gewichte) | 11,39 | 1,656 % |
| voller Ganzzahlpfad | 11,09 | 1,613 % |

**Das 0,97-fache des Bodens.** Die Umsetzung fuegt nichts ueber das
Unvermeidliche hinaus hinzu.

📌 **Die richtige Frage ist nicht „ist der Abstand klein", sondern
„kostet meine Umsetzung mehr als das, was ohnehin bezahlt wird".**

277 Kernproben gruen.

### v0.85.0 – 2026-09-21 (die kausale Faltung, ganzzahlig, mit einer Fassung statt zweien)

`kernels` **0.61.0**, neues Modul `faltung`. Vor der Rekurrenz laufen
`q`, `k` und `v` gemeinsam durch eine tiefenweise Faltung ueber vier
Stellen, gefolgt von SiLU.

#### ⚑ Eine Fassung fuer Vorlauf und Dekodieren, mit Absicht

Die Faltung braucht die letzten drei Eingaenge; beim Dekodieren kommt je
Aufruf ein Token, also steht der Rest im `Faltungsfenster`.

⛔️ **Ein Vorlauf, der die ganze Folge auf einmal faltet, waere
schneller und eine zweite Umsetzung derselben Rechnung.** Genau dort
entstehen Unterschiede zwischen Vorlauf und Dekodieren, die niemand
sieht, weil beide fuer sich plausibel aussehen. Der Vorlauf ruft
dieselbe Funktion in einer Schleife.

⚠️ **Der Anfang einer Folge ist null und nicht der erste Wert**, wie bei
der Vorlage mit `padding = KERN - 1`.

#### Gemessen gegen die Fremdimplementierung

Mit den echten Gewichten der Ebene 0 (8192 Kanaele, Kern 4), 64 Kanaele
ueber 24 Schritte: **groesster Abstand 0,80 Stellen von 2^-8**, also
unter einer letzten Stelle.

#### ⚑ Fuenf Proben, und beide Fehler, vor denen sie warnen, eingebaut

| Mutation | Wirkung |
|---|---|
| Fenster **vor** der Summe nachziehen | 3 Proben fallen |
| Fensterbasis ohne Kanalversatz | `ein_kanal_laesst_die_anderen_in_ruhe` faellt |

📌 **Eine gruene Probe ist kein Beleg, solange sie nicht gebissen hat.**
Die Verzoegerungsprobe ist die wichtigere von beiden: Ohne sie bliebe
die Faltung auch dann gruen, wenn das Fenster gar nicht gelesen wuerde.

277 Kernproben gruen.

#### ⚠️ Was hier NICHT bewiesen ist

Die Messung oben prueft **die Arithmetik**, nicht den Rust-Kern. Dass
der Rust-Kern dieselbe Rechnung macht, sichern seine Proben samt
Mutationen. **Zwei verschiedene Fragen, zwei verschiedene Belege**, und
sie zusammenzuwerfen hiesse, eine davon unbeantwortet zu lassen.

### v0.84.0 – 2026-09-21 (die Vorlage tut vor der Rekurrenz zwei Dinge, die der Entwurf nicht hatte)

`kernels` **0.60.0**. Die Fremdschicht wurde als Vorlage gelesen, und
dabei zeigte sich, dass die Rekurrenz zwar richtig dastand, **der Weg zu
ihren Eingaengen aber nicht**.

#### ⛔️ Zwei fehlende Schritte, und einer verschiebt den Wertebereich

Die Vorlage bringt `q` und `k` je Kopf und je Token auf **Einheitslaenge**
(`l2norm`) und skaliert `q` zusaetzlich mit `1/sqrt(kopf_dim)`. Beides
stand in keiner Zeile des Kernels.

⚑ **Das ist keine Feinheit.** Mit normiertem `k` ist `Summe(S * k)` eine
Projektion auf einen Einheitsvektor; ohne die Normierung haengt sie an
der Laenge von `k`. Die Breitenmessung haette also einen anderen
Gegenstand gemessen als den, der spaeter rechnet.

📌 **Eine Formel aus einem Aufsatz ist nicht die Schicht.** Wer eine
fremde Schicht nachbaut, liest ihren ganzen Vorwaertspass und nicht nur
die Gleichung, die den Namen traegt.

#### Und daraus folgt eine eigene Breite

⛔️ **Normieren schrumpft jede Komponente um `sqrt(kopf_dim)`.** Ein
Einheitsvektor ueber 128 Stellen hat typische Komponenten um 0,088; bei
`2^-8` sind das 22 Einheiten, also rund vier Prozent Auflösung je
Komponente. Gemessen gegen die Rekurrenz in doppelter Breite:

| Bruchbits von `q`, `k` | groesster Abstand |
|---|---|
| 8 | 1,90 |
| 10 | 0,76 |
| **12** | **0,55** |
| 14, 16 | 0,55 |

⚑ Ab zwoelf saettigt der Abstand; was bleibt, kommt aus Zustand und
Zerfall. Daher `NORM_FRAC = 12`, und die Zahl deckt sich mit dem Grund:
`log2(sqrt(128))` sind 3,5 Stellen, also `8 + 3,5` aufgerundet.

⚠️ **Ohne diese Messung waere es ein stiller Qualitaetsverlust
geworden**, denn 8 Bit rechnen genauso, nur schlechter.

#### Gemessen: der Abstand saettigt

| Laenge | groesster Abstand |
|---|---|
| 32 | 0,57 |
| 128 | 0,60 |
| 256 | 0,65 |

**Achtfache Laenge kostet das 1,15-fache.** ⚠️ **Hochgerechnet und nicht
gemessen:** Bei diesem Wachstum bleibt der Abstand auch ueber das volle
Fenster unter einer Stelle. Die volle Laenge ist in einer Simulation
nicht zu rechnen; sie gehoert in den Rechenpfad.

#### ⛔️ Und die Gegenprobe fand einen Fehler im Urteil selbst

Der beste Fall kam als der schlimmste heraus: Bei einem **bitgleichen**
Lauf ist der Abstand bei jeder Laenge null, das Verhaeltnis
`letzter/erster` wird `inf`, und die Meldung lautete „waechst mit der
Laenge".

📌 **Ein Verhaeltnis braucht einen Nenner**, und wo keiner ist, gehoert
der Fall vorher abgefangen. Jetzt unterscheidet das Urteil drei Faelle:
bitgleich, Wachstum aus der Null heraus (Reihe verlaengern) und ein
echtes Verhaeltnis. **Gegengeprueft** mit 28, 22 und 20 Bruchbits.

#### Belege

- 272 Kernproben gruen; drei davon mussten mitgezogen werden, weil sie
  den Einheitsschluessel in der alten Auflösung gaben. ⚑ **Sie haben
  gebissen**, und das ist der Zweck.
- Die Bauzusicherung heisst jetzt
  `ZUSTAND_FRAC >= NORM_FRAC + WERT_FRAC`.
- `tests/diag/zustandsschicht_referenz.py` neu. ⚑ Sie liest die vier
  Breiten **aus dem Kernel**, statt sie zu wiederholen.

### v0.83.0 – 2026-09-21 (die Zustandsbreite steht auf einer Messung, und die alte Begruendung war falsch)

`kernels` **0.59.0**. Die Breiten der Zustandsschicht sind jetzt am
Zielmodell gemessen statt am Modell geschaetzt, und dabei sind drei
Fehler aufgefallen, alle drei in meiner eigenen Vorarbeit.

#### Die Spanne von `a`, endlich gemessen

Der Zerfall lautet `g = exp(-exp(A_log) * softplus(a + dt_bias))`, und
`a` kommt zur Laufzeit aus einer Projektion. Die Messung rechnete
deshalb bisher mit `a = 0` und einer einzigen Ebene und nannte das
ausdruecklich eine Schranke, keine Verteilung.

⚑ **Der Artefaktbau liefert genau das Fehlende.** Die
Aktivierungsstatistik haelt je Ebene den beobachteten
Betragsgrosstwert des `in_proj_a`-**Ausgangs**, gesammelt ueber den
ganzen Kalibrierkorpus. Gemessen ueber alle 30 Zustandsebenen:
**6,2 bis 14,4**, je nach Ebene.

⚠️ **Die angenommene Spanne war zu eng.** Sie lautete ±8; Ebene 37
liegt bei 14,4.

#### ⛔️ Die Formel multiplizierte, wo sie haette minimieren muessen

Die alte Begruendung fuer 35 Bruchbits lautete `log2(N/(1-g))`. ⚑
**Richtig ist `eps * min(N, 1/(1-g))`:** Eine Folge kann nur so weit
zurueckwirken, wie sie **lang** ist ODER wie weit das Gedaechtnis
reicht, nicht beides multipliziert. Fuer das volle Fenster (262 144)
sind das **25 Bit**, nicht 35.

📌 **Neun Bruchbits fuer einen Fall, den es nicht gibt.**

#### ⛔️ Der haerteste Pruefpunkt lag nicht, wo ich ihn suchte

Die Reihe pruefte auf `max(zerfaelle)`, also auf dem Zerfall am
naechsten an eins. Das klingt nach dem schaerfsten Fall und ist der
**harmloseste**: Bei `g = 0,999999998980` zerfaellt ueber das ganze
Fenster nichts Messbares, und jede Darstellung rundet ihn auf glatt
eins. Eng wird es bei `g = 1 - 1/N`.

📌 **Ein Extremwert ist nicht automatisch der schlimmste Fall.**
Welcher es ist, sagt die Fehlerformel und nicht die Anschauung.

#### ⛔️ Und eine Rundung hat den Messgegenstand vernichtet

Die Pruefpunkte liefen durch `round(g, 6)`. Der langsamste gemessene
Zerfall ist 0,999999998980 und wird damit zu **exakt 1,0**. Die Reihe
verglich danach „kein Zerfall" mit „kein Zerfall", **jede Zeile wurde
null**, und die Schlussrechnung teilte durch null.

⛔️ **Schlimmer als der Abbruch war die Tabelle davor:** Sie zeigte
lauter Nullen, und darunter stand ein fester Urteilstext, der das
Gegenteil behauptete („bei 24 Bruchbits bleiben noch Abweichungen").
📌 **Ein Urteil, das seine eigene Messung nicht liest, ist keines.**
Das Urteil wird jetzt aus der Tabelle gewonnen.

#### Die Breiten, die daraus folgen

| | alt | neu | Beleg |
|---|---|---|---|
| `ZUSTAND_FRAC` | 28 | **28** | gemessen reichen 24; in `i64` kosten vier Stellen Reserve nichts |
| `ZERFALL_FRAC` | 35 | **28** | ab 26 Bit hoechstens eine Stelle Abstand, 28 die erste saubere Null; analytisch 25 |

⚑ **Die Bauzusicherung `ZERFALL_FRAC > ZUSTAND_FRAC` ist entfallen**,
denn sie verglich zwei Groessen, die nichts miteinander zu tun haben.
An ihrer Stelle steht die Schranke, die wirklich gilt:
`ZERFALL_FRAC >= KONTEXT_BITS + WERT_FRAC - 1`. **Gegengeprueft:** Mit
24 statt 28 uebersetzt die Kiste nicht, mit der Meldung, die den Grund
nennt.

272 Kernproben gruen.

### v0.82.0 – 2026-09-21 (das grosse Modell laesst sich exportieren, und der Lader weist es ab)

Der Artefaktbau kann die hybride Bauart des `myelith-35b-a3b`
vollstaendig exportieren. ⚑ **θ_v bleibt auf 0.20.0**, bewusst: Was
entsteht, ist ein pruefbarer Gegenstand, kein rechenbares Modell. Der
Lader weist es ab, und genau das ist der Zweck dieses Schritts.

#### Sieben Handgriffe, und die Liste hatte nur sechs

| | Was | Beleg |
|---|---|---|
| 1 | `normiere_namen()`: `model.language_model.` nach `model.` | 5 Faelle, dazu der ganze Export |
| 2 | `target_keys` um `linear_attn.` und `mlp.shared_expert` erweitert | 540 und 320 Tensoren im Artefakt |
| 3 | Ausschlussliste fuer `visual.` und `mtp.` (Fund 416) | 352 von 1045 Tensoren draussen, 0 im Artefakt |
| 4 | Eintrag `myelith-35b-a3b`, gelesen aus dem verschachtelten `text_config` | `layer_types` deckungsgleich mit den echten 40 Eintraegen |
| 5 | `bereite_rekurrenzparameter()`: `A_log` nach `exp_A`, `dt_bias` je Element, `conv1d.weight` entfaltet | 30 von 30 Zustandsebenen tragen `exp_A` |
| 6 | Haken fuer `linear_attn.*` in der Statistik | 802 Module gesammelt, 0 ohne Haken |
| 7 | `AUSGESCHLOSSENE_TEILBAEUME` auf Modulebene gehoben | importierbar, 0,6B baut bitgleich |

⛔️ **Punkt 6 stand in keiner Planung, und er waere teuer geworden.**
Die Liste der noetigen Aenderungen war aus `quantize.py` gewonnen, also
aus dem Exportpfad. Die Skalen entstehen aber in `stats.py`, und dort
traf kein Namensmuster die Zustandsschichten: Der Export waere
durchgelaufen und haette fuer **240 Tensoren keine Skala** gehabt.

📌 **Wer zaehlt, muss sagen, worin er zaehlt.** Eine Vollstaendigkeit,
die in einer Datei geprueft wurde, gilt fuer diese Datei. Die Frage
lautet nicht „habe ich alles gefunden", sondern „welchen Pfad habe ich
abgesucht, und welche gibt es noch".

⚑ **Punkt 7 ist klein und gehoert trotzdem hierher.** Die
Ausschlussliste lag als lokale Variable in einem Generator. Eine Probe,
die sie braucht, haette sie wiederholen muessen, und damit stuende
dieselbe Angabe an zwei Orten.

#### Das Artefakt, gebaut und nachgezaehlt

| | |
|---|---|
| Bau | 47 min (Korpus 43,5 min, Export 3,5 min), **33 GB, 62 679 Dateien** |
| Quelltensoren | 1045, davon **693 Textpfad**, 352 ausgeschlossen |
| Artefakt-Tensoren | 31 334, davon 30 720 Experten (40 x 256 x 3) |
| Speicher | RSS 17,6 GiB statt 67 GB, weil die Gewichte ueber `mmap` kommen |

⚑ **Jeder Quelltensor ist abgehakt, keiner fehlt und keiner ist zu
viel.** 583 tragen ihren Namen unveraendert; die restlichen 110 sind
die drei Umformungen, die der Export absichtlich vornimmt: 30 mal
`A_log` nach `exp_A`, 40 mal `experts.down_proj` in 256 Dateien und 40
mal `experts.gate_up_proj` in 2 x 256. Alle drei einzeln nachgezaehlt,
alle vollstaendig. Der einzige Tensor ohne Quellnamen ist `lm_head` in
int16, 248 320 x 2048 x 2 Bytes, und die Dateigroesse stimmt aufs Byte.

⚑ **Gegenrichtung geprueft:** 0 Tensoren aus `visual.` oder `mtp.`.

#### Das Merkmalstor am echten Gegenstand

Das Artefakt deklariert sechs Merkmale, vier davon kennt dieser Bau
nicht. Der Lader bricht **vor** dem ersten Gewicht ab und nennt sie:

```
dieses Artefakt braucht Merkmale, die dieser Bau nicht rechnet:
ausgangstor, geteilter_experte, teildrehung, zustandsschicht.
```

⚑ **Und die Gegenprobe, denn ein Tor, das alles abweist, ist keines:**
Dasselbe Programm laedt das `myelith-8b` und rechnet 36 Layer-Vektoren
und 3 E2E-Vektoren. ⚠️ **Mit frisch gebauter Binaerdatei geprueft**,
nachdem eine acht Stunden alte am selben Tag schon einmal ein falsches
Ergebnis vorgetaeuscht hat.

#### ⛔️ Der Kopf dieser Datei trug eine widerlegte Aussage

Der Eintrag v0.81.0 haelt fest, das 8B breche die Reihe „je kleiner das
Modell, desto teurer die Quantisierung", und schliesst mit **„Die
Behauptung im Katalog ist entsprechend berichtigt, nicht
stehengelassen"**. Der Kopf dieser Datei, gut 600 Zeilen darueber, sagte
weiter „Der Abstand faellt monoton mit der Modellgroesse" und kannte das
8B nicht: „auf allen **drei** eingesetzten Modellen".

📌 **Der Satz „ist berichtigt" ist selbst eine Angabe, und niemand hat
nachgesehen, wo ueberall.** Genau die Regel dieses Projekts, angewandt
auf den, der sie aufschreibt: **Ein Beleg, den niemand nachgesehen hat,
ist keiner.** Berichtigt sind der Kopf und die Zaehlung im Katalog.

#### Was noch fehlt, damit es auch rechnet

⚠️ Die groessere Haelfte: der Kernel verdrahtet, der Ebenen-Dispatch,
der geteilte Experte, das Ausgangstor, die Teildrehung, die Faltung.
Dazu die offene Messfrage zum Zerfallsbereich. **Erst dann wird θ_v
angehoben**, und dann werden alle Artefakte neu gebaut.

### v0.81.0 – 2026-09-21 (das mittlere Modell gebaut, und ein Tor für Bauarten, die dieser Bau nicht kann)

`runtime` **0.58.0**. Zwei Dinge: das 8B ist gebaut und gemessen, und das
Schema kann jetzt sagen, was es braucht.

#### `myelith-8b`: gebaut, wiederholbar, gemessen

| | |
|---|---|
| Bau ohne Skalenpaket | 10 min 4 s, 8,8 GB, 811 Dateien |
| Bau **mit** dem daraus erzeugten Paket | **36 s, bitgleich** |
| Laden und Rechnen | 36 Layer-Vektoren und 3 E2E-Vektoren in 6,6 s |
| Perplexität | **13,27 gegen BF16 12,79, also +3,75 %** (Kriterium 5 %), AKZEPTIERT |

⚑ **Der Bau ist damit plattformübergreifend wiederholbar** (Fund 32): 542
Skalen und 5 LUTs kommen aus dem Paket, hashgeprüft, und der
nichtdeterministische Gleitkommateil entfällt.

⛔️ **Und die Zahl bricht eine Aussage dieses Projekts.** Der Changelog hält
fest: je kleiner das Modell, desto teurer die Quantisierung. Das 8B liegt
mit **+3,75 % schlechter als das kleinere 4B** (+1,65 %). ⚑ **Eine
Erklärung liegt nahe und ist nicht geprüft:** Die Reihe mischt zwei
Achsen, denn 0,6B und 4B haben eine **gebundene Einbettung**, 8B und 30B
nicht. Innerhalb jeder Gruppe fällt der Abstand monoton. ⚠️ Bei 435
Positionen ist ein Unterschied dieser Größe außerdem nicht sicher
auflösbar; ein Lauf über 128 Sequenzen würde es entscheiden. **Die
Behauptung im Katalog ist entsprechend berichtigt, nicht stehengelassen.**

#### Das Schema kann jetzt hybride Bauarten ausdrücken

Elf neue Felder in `ModelDims`, alle mit Vorgabe: die Ebenenarten
(`layer_types`), die Maße einer rekurrenten Zustandsschicht, ein geteilter
Experte, gepackte Expertentensoren, die Art des Ausgangstors, die Zahl der
MTP-Ebenen und die Zahl der gedrehten Stellen.

⛔️ **Bewusst eine Anzahl statt eines Faktors.** Die fremde Konfiguration
nennt `partial_rotary_factor` als **Gleitkommazahl** (0,25). Dieses Schema
trägt keine; gebraucht wird ohnehin die Anzahl (`head_dim · Faktor`, bei
256 also 64). 📌 **Wer ein Verhältnis speichert, das er nie braucht, holt
sich eine Gleitkommazahl in den Vertrag und eine Multiplikation in den
Rechenpfad.**

#### ⛔️ Und das eigentliche Stück: ein Merkmalstor

Jedes Artefakt trägt jetzt eine Liste, **was es zum Rechnen braucht**, und
der Lader hält sie gegen seine eigenen Fähigkeiten. Ein unbekanntes
Merkmal ist ein **Abbruch mit Namen**, geprüft **vor** dem Einlesen der
Gewichte.

⚑ **Der Grund ist eine Einsicht, die meine erste Fassung genau falsch
hatte.** Ein Lader ohne `deny_unknown_fields` **überliest** ein Feld, das
er nicht kennt. Für eine Beschreibung ist das richtig; für ein Feld, das
die Rechnung ändert, ist es der teuerste Fehler dieses Projekts: **zwei
ehrliche Knoten mit verschiedenen Zahlen, ohne Meldung.**

⛔️ **Und die nächste Generation besteht aus solchen Feldern.** Der
Konfigurationssatz der Qwen4-Vorschau bringt vier Untersysteme, die dieses
Schema nicht ausdrücken kann: einen vierfach geteilten Residualstrom, eine
Blockauswahl in der Aufmerksamkeit, eine N-Gramm-Einbettung mit 20
Millionen Einträgen und Einbettungen je Ebene. **Ein Lader, der davon
nichts weiß und die Felder überliest, lädt ein Artefakt, das er nicht
rechnen kann, und merkt es nicht.**

⚑ **Die Liste wird abgeleitet und nicht getippt**, aus der Konfiguration,
und sie nennt auch Bauarten, die der Rechenpfad **noch nicht kann**. Das
ist Absicht: Erst wenn der Export sie benennt, kann der Lader sie
ablehnen. Drei Prüfungen halten die beiden Sprachen zusammen, darunter
eine, die ausdrücklich verlangt, dass ein ungebautes Merkmal **nicht** in
der Fähigkeitsliste steht.

**Belegt:** alle vier Artefakte neu gebaut, und in `model_config.json`
ändert sich **nichts außer dem hinzugekommenen Feld**; Konformität weiter
**48/48**; das Tor am echten Artefakt gegengeprobt (ein verfälschtes wird
abgelehnt, das echte lädt); 878 Prüfungen grün, drei Mutationen gegen das
Tor beißen, Gleitkomma-Audit ohne Treffer, Clippy sauber.

### v0.80.0 – 2026-09-21 (zwei Nichtlinearitäten für die Zustandsschicht)

`kernels` **0.58.0**. `sigmoid` und `softplus`, je Erzeuger und
Nachschlagefunktion. Sie fehlen der rekurrenten Zustandsschicht an drei
beziehungsweise einer Stelle: `beta = sigmoid(b)`, das Tor am
Attention-Ausgang, das Tor des geteilten Experten, und im Zerfall
`g = -exp(A_log) · softplus(a + dt_bias)`.

⚑ **Softplus hat dieselbe Randfortsetzung wie die SiLU, und das ist kein
Zufall.** Beide laufen für große positive `x` gegen `x` und für große
negative gegen null, mit einem Fehler der Größenordnung `exp(-|x|)`.
⛔️ **Deshalb steht die Regel jetzt einmal da und nicht zweimal**
(`nachschlagen_identitaet_oben`); beide zeigen darauf. 📌 **Genau so ist
Fund 349 entstanden**, als Vorwärts- und Rückwärtspass an ihren Rändern
Verschiedenes sagten.

⚑ **Sigmoid setzt anders fort**, nämlich auf Konstanten (oben eins, unten
null), weil es beschränkt ist. Eine eigene Probe hält fest, dass es eben
**nicht** wie Softplus fortsetzt: Ohne sie bestünde die Probe darüber
auch dann, wenn alle drei dasselbe täten.

⚑ **Und eine eigene Tabelle statt einer Ableitung aus der SiLU**, mit
demselben Argument wie bei der SiLU-Ableitung: `σ(x) = silu(x)/x` ist bei
`x = 0` undefiniert und in der Umgebung unbrauchbar, also genau dort, wo
die meisten Werte liegen.

**Die stärkste der neuen Prüfungen ist eine Identität:**
`silu(x) = x · σ(x)`, über zwei **unabhängig** erzeugte Tabellen.
⛔️ **Sie ist beim ersten Lauf gefallen**, und die Zahl war lehrreich: bei
`x = 6,16` lag der Abstand bei 3,1 Stellen statt der erwarteten 2. Nicht
die Tabellen waren falsch, sondern die Erwartung. Der Rundungsfehler von
`σ` wird beim Multiplizieren **mit `|x|` vervielfacht**, die Schranke
muss also mit `|x|` wachsen. 📌 **Eine Toleranz ohne ihre Herleitung ist
geraten.**

⚑ **Und daraus folgt nachträglich die Begründung für die eigene
SiLU-Tabelle:** Am rechten Rand der Domäne wäre die Verstärkung rund 128
Stellen. Eine Probe hält das fest.

**Belegt:** 272 Prüfungen in `kernels`, drei neue in Python, Konformität
weiter **48/48** (der Umbau von `silu_nachschlagen` ändert nichts),
Gleitkomma-Audit ohne Treffer, Clippy sauber.

⚠️ **Noch offen und benannt: der Zerfall selbst.** `exp` für ein
Argument nahe null braucht 35 Bruchbits Genauigkeit, und eine Tabelle
über den Eingang liefert die dort nicht. **Das ist die nächste
Entwurfsfrage**, und sie gehört gemessen wie die beiden davor.

### v0.79.0 – 2026-09-21 (eine rekurrente Zustandsschicht, ganzzahlig, und das mittlere Modell)

`kernels` **0.57.0**. Zwei Dinge, und das erste ist neu in der **Art**
und nicht im Umfang.

#### Die Zustandsschicht (Arbeitstitel)

Das nächste große Modell dieses Projekts rechnet drei Viertel seiner
Ebenen **nicht** mit Softmax-Aufmerksamkeit, sondern mit einer
**rekurrenten Zustandsschicht**. ⚑ **Damit steht eine Frage im Raum, die
dieses Projekt noch nie hatte:** Alle bisherigen Kernel rechnen
Reduktionen, und die darf man umordnen, weil Ganzzahladdition assoziativ
ist. **Eine Rekurrenz ist keine Reduktion.**

⚑ **Die These wird dadurch nicht verletzt, und der Gewinn liegt
woanders.** Bei gleichem Ganzzahlzustand rechnet jeder Knoten denselben
Folgezustand, Bit für Bit. Die Gleitkomma-Referenz führt ihren Zustand
ausdrücklich in `float32`, **weil** eine Gleitkomma-Akkumulation über
hunderte Schritte von der Umsetzung abhängt. Ein Ganzzahlzustand mit
festem Shiftplan ist reproduzierbarer als sie, nicht weniger.

⛔️ **Die Breiten sind gemessen und nicht gewählt**, gegen die echten
Zerfälle des Zielmodells (`min = 0`, `mittel = 0,719`, `max = 0,999863`):

| | Bruchbits | Warum |
|---|---|---|
| Zustand | **28** | bitgleich bei jedem gemessenen Zerfall und jeder Länge; 24 ließen eine Stelle |
| Zerfall | **35** | `log2(N/(1-g))` für das volle Fenster plus Reserve |
| Werte | 8 | wie im übrigen Rechenpfad |

⛔️ **Der Zerfall ist die engere Stelle, nicht der Zustand**, und das war
die Überraschung. Der Fehler im Produkt `g^N` wächst mit `N·ε/(1-g)`, und
`1/(1-g)` ist beim schärfsten Zerfall rund **7299**. Er braucht also mehr
Bits als der Zustand, den er bewegt.

⚑ **Die tragende Erkenntnis der Breitenrechnung:** Ist der Zustand
mindestens doppelt so fein wie die Werte, ist die Rang-1-Fortschreibung
ein **Linksshift** und damit verlustfrei. Gerundet wird nur am Zerfall und
an den zwei Kontraktionen. **Das hält jetzt der Übersetzer fest** und
nicht eine Prüfung: Wer die Breite senkt, bekommt einen Baufehler.

📌 **Eine naheliegende Abkürzung ist gemessen worden und falsch.** Den
Zerfall als Komplement `1-g` darzustellen bringt **nichts**: Beide
Darstellungen runden auf dieselbe absolute Schrittweite, und in `g^N`
geht der absolute Fehler ein, nicht der relative. **Eine
Umparametrisierung verschiebt Genauigkeit nur, wenn sie die Skala
mitverschiebt.**

⚠️ **Was ausdrücklich noch nicht geschieht:** Die Schicht ist **nicht** in
den Rechenpfad verdrahtet, und θ_v ist **nicht** angehoben. Der Lader
vergleicht die θ_v-Version eines Artefakts zeichengenau mit der
eingebetteten Spezifikation; eine Anhebung entwertet alle drei
vorhandenen Artefakte auf einen Schlag. **Sie gehört in denselben Zug wie
die fehlenden Nichtlinearitäten und der Neubau.**

**Belegt:** sieben neue Prüfungen, fünf Mutationen dagegen, **alle fünf
beißen**; Konformität weiter **48/48**, Gleitkomma-Audit ohne Treffer,
Clippy sauber.

#### Das mittlere Modell

`myelith-8b` steht im Katalog, verifiziert gegen den lokalen
Schnappschuss: 36 Ebenen, `hidden_size` 4096, GQA 4:1, QK-Norm über 36
`q_norm`- und 36 `k_norm`-Tensoren gezählt, null Bias-Tensoren.
⚠️ **`tie_word_embeddings` ist hier `False`, anders als beim 0,6B und
4B**; das Modell trägt ein eigenes `lm_head.weight`. Jedes Feld ist gegen
die echte `config.json` gegengeprüft, nicht übernommen.

⚑ **Damit ist die dichte Reihe wieder dreipunktig**, nachdem sie mit dem
Wegfall des 14B auf zwei gefallen war. **Eine Gerade durch zwei Punkte
passt immer.**

⛔️ **Fund 414: der Erzeuger der Modellübersicht hielt einen
Gedankenstrich bereit.** `modelle_liste.py` setzte ihn als Rückfall für
ein Modell ohne θ_v-Eintrag. Erreicht hat ihn nie eines, weil bis heute
jedes Modell im Katalog auch ein Artefakt hatte; das erste ohne hätte ihn
in die erzeugte Datei geschrieben und die Stilprobe rot gemacht.
📌 **Ein Rückfall, den nie etwas erreicht, ist ungeprüfter Code in einem
Erzeuger.**

### v0.78.0 – 2026-09-21 (die Quellmodelle liegen an der Wurzel, nicht mehr unter dieser Komponente)

`runtime` **0.57.0**. Festlegung des Projektinhabers: Alle von außen
geladenen Gewichte dieses Projekts liegen zusammen unter `MODELS/`; die
Quellmodelle, aus denen die θ_v-Artefakte entstehen, sind die Rubrik
`MODELS/llm`.

⚑ **Was bleibt, ist die Trennung, auf die es ankommt.** `artifacts/`
bleibt unter dieser Komponente, denn ein Artefakt ist hier **gebaut**
und trägt die Lizenz dieses Repositoriums; ein Quellmodell ist
**geladen** und trägt die seiner Quelle. Der Bau geht weiter von
`MODELS/llm` nach `artifacts/`, und `KATALOG.json` samt der daraus
erzeugten Übersicht ist mitgezogen.

⚑ **Der Pfad wird jetzt aus der Dateitiefe gerechnet und nicht aus dem
Arbeitsverzeichnis.** Solange Modelle und Artefakte im selben
Elternverzeichnis lagen, trug ein relativer Pfad; jetzt liegt das eine
eine Ebene höher, und ein `../MODELS/llm` wäre eine Wette darauf, aus
welchem Verzeichnis jemand aufruft. ⛔️ **Die Gegenprobe prüft nicht den
Pfad, sondern den Baum:** Eine Ebene daneben liefert einen Pfad, der
genauso aussieht, also wird nachgesehen, ob an der errechneten Wurzel
wirklich dieses Repositorium liegt. Mit `parents[2]` statt `parents[3]`
fällt die Probe, nachgestellt.

⛔️ **Fund 409: `MODELS_DIR` in `runtime/src/paths.rs` hatte keinen
einzigen Leser.** Die Konstante spiegelte nur die Python-Seite, damit
beide denselben Namen nennen. Beim Umzug wäre sie eine falsche Angabe
geworden, die niemand benutzt und die der Nächste für gültig hält.
Entfernt, mit Grabstein. ⚑ **Und der Grund, warum die Laufzeit sie nie
brauchte, ist der Vertrag selbst:** Sie rechnet auf Artefakten, nie auf
Quellmodellen. Ein Quellmodell ist Gleitkomma und Voraussetzung des
**Baus**; wer es im Rechenpfad bräuchte, hätte den Pfad verlassen.

⛑ **Fund 407: der Artefaktstand im Planpapier stand zum zweiten Mal
falsch**, `runtime` auf v0.52.0 gegen 0.56.0 im Manifest, mit vier
Komponentenversionen dazwischen. Die Abhakprobe kann ihn nicht fangen:
Sie vergleicht Kopfzeilen und Komponenten-READMEs, und er ist
Fließtext. 📌 **Eine Lehre, die als Fließtext neben der Zahl steht,
ersetzt keine Probe über die Zahl.**

**Unverändert geblieben, und das ist die Aussage:** Konformitätslauf
**48/48** auf `myelith-0.6b` gegen `reference` und `cpu-simd`, das
Gleitkomma-Audit über fünf Verzeichnisse ohne Treffer, Clippy über alle
25 Kisten ohne Warnung. Berührt wurden Pfade und ein toter Bezeichner,
nicht die Numerik.

### v0.77.2 – 2026-09-17 (der Artefaktbau sagt, was ihm fehlt, und er ist nachweislich wiederholbar)

θ_v unverändert, **kein Rechenweg ändert sich**. Geändert hat sich
`scripts/build_artifacts.sh`.

📌 **Es rief blankes `python3`.** Auf einem frischen Klon ist das der
Interpreter des Systems, und der hat kein `torch`: Der Artefaktbau endete
in einem nackten `ModuleNotFoundError`, aus dem niemand ablesen kann,
dass eine Umgebung fehlt. **Eine Voraussetzung, die erst beim Absturz
sichtbar wird, ist keine Voraussetzung, sondern eine Falle.** Jetzt sucht
das Skript in der Reihenfolge `$PYTHON`, `calibrate/.venv/bin/python3`,
`python3` nach einem Interpreter, der torch **und** transformers wirklich
importieren kann, und nennt sonst die drei Zeilen, mit denen die Umgebung
entsteht.

⚑ **Und dabei ist belegt worden, dass der Bau wiederholbar ist.** Das
Artefakt des 0,6B wurde aus denselben Gewichten neu gebaut: **48/48
Konformitätsvektoren** und **derselbe `decode_digest`**
(`b744c0603db284b2…`) wie beim Stand vom 11. September. Möglich ist das,
weil das Skalenpaket im Repositorium liegt und die Aktivierungsstatistik
deshalb entfällt; der Bau ist damit plattformübergreifend bitgleich, und
das steht seit heute nicht mehr nur als Zusage da.

⚠️ **Der Anlass war ein Versehen**, und das gehört dazu: Der Lauf wurde
beim Prüfen der Skriptänderung ausgelöst und schrieb das Artefakt neu,
während für den Abend Messungen darauf warten. Nachgeprüft wurde deshalb
sofort und vollständig. **Ein Skript, das eine Stunde rechnet und dabei
ein Messobjekt überschreibt, sollte vorher fragen**; das steht als
offener Punkt.

### v0.77.1 – 2026-09-17 (ein Nachweis, dass die Einbettung im Artefakt nichts trägt)

θ_v unverändert, **kein Rechenweg ändert sich, kein Konformitätsvektor
rührt sich, kein Byte im Artefakt ist angefasst.** Neu ist allein
`tests/diag/einbettung_aus_kopf.py`.

⛔️ **Setzt ein Modell `tie_word_embeddings`, sind Einbettung und LM-Kopf
dieselbe Matrix, und das Artefakt legt trotzdem beide ab.** Der Kopf in
int16, die Einbettung in int8: 156 MB von 0,93 GB beim 0,6B, 389 MB von
4,82 GB beim 4B. Das 30B ist nicht betroffen.

⚑ **Die int8-Fassung ist eine geschachtelte Quantisierung der
int16-Fassung** und damit bitgleich herleitbar: arithmetischer
Rechtsshift **je Zeile** um die Differenz der Kanalshifts, mit Runden
zur nächsten geraden Zahl, also genau der Regel des Rechenpfads.
**Geprüft über alle Werte**, 155 582 464 und 388 956 160, jeweils
100,000000 %.

⛔️ **Das Skript ist als Tor gedacht und nicht als Statistik.** Wer die
Datei einspart, muss die Beziehung zeigen und nicht annehmen: Sie hängt
am Quantisierer, und einer, der die Schachtelung bricht, macht aus der
Ersparnis eine falsche Einbettung. ⚑ **Deshalb über alle Werte und nicht
über eine Stichprobe**: 99,6 % sehen aus wie ein Treffer und sind ein
Fehlschlag. Gegengeprobt an einem künstlichen Artefakt, in dem **ein
einziger** Wert verbogen ist; das Skript nennt Zeile und Spalte und gibt
1 zurück.

### v0.77.0 – 2026-09-16 (die Messwerkzeuge rechnen gebündelt, und die Zahl bleibt dieselbe)

`runtime` **0.56.0** (θ_v **0.20.0** unverändert, **kein Rechenpfad
ändert sich**).

⛔️ **Die Messwerkzeuge liefen auf dem einen Pfad, den die Optimierung
nicht berührt.** `perplexity_probe` und `entscheidungsprobe` riefen
`forward_token` in einer Schleife über die Sequenz. Damit erreichten sie
**weder die gebündelte Vorbereitung** (seit v0.72.0) **noch die GPU**:
Die rechnet erst ab sechzehn Eingaben je Bündel, und ein einzelnes Token
kommt dort nie an. **Dieselbe Klasse wie der Sieben-Token-Prompt der
Durchsatzmessung**, nur in einem anderen Werkzeug.

⚑ **`Model::logits_stapel`**: die Logits jeder Position, Ebenen
gebündelt über `vorbereiten_stapel`, in Fenstern von 512 Token. **Der
LM-Kopf bleibt tokenweise**, Zeichen für Zeichen derselbe Aufruf wie in
`forward_token`: damit nicht schneller, aber auch nicht anders, und das
ist hier mehr wert. Wer ihn bündelt, tut es als eigenen Schritt mit
eigener Gegenprobe.

⚑ **Gemessen, je zwei Läufe, `myelith-0.6b`, 435 Positionen:**
**10,9 und 10,5 s tokenweise gegen 3,9 und 3,8 s gebündelt**, also
**Faktor 2,8** auf die ganze Wanduhr einschließlich Python-Start und
Modellladen; auf den Rechenteil allein ist er größer. Das 4B läuft in
8 s.

⛔️ **Und die Perplexität ist dieselbe, bis zur letzten Stelle**, je
Sequenz verglichen: 0,6B 33,28835134857662, 4B 19,951866795026724, beide
unverändert gegen die Messung vom 2026-09-14. **Ohne diesen Vergleich
wäre jede Zahl danach mit den früheren unvergleichbar, und niemand sähe
es ihr an:** Perplexität ist ein Mittelwert, und ein Mittelwert verzeiht
eine kleine Abweichung an jeder Position.

⚑ **Die Gegenprobe dazu ist eine eigene Prüfung**
(`die_gebuendelten_logits_sind_die_tokenweisen`): jede Position einzeln,
über die Fenstergrenze hinweg, dicht und als Gemisch. Zwei Mutationen
geprüft, beide beißen (ein fehlender Fensterversatz, eine um eins
verschobene Logitskala).

⚠️ **Warum es 2,8 sind und nicht mehr:** Beim 0,6B ist der LM-Kopf
151 936 mal 1 024 Werte und wird je Position einmal ganz gelesen; er
dominiert jetzt. Bei den größeren Modellen wiegen die Ebenen schwerer,
dort sollte der Faktor höher liegen. **Gemessen ist das nicht**, und
deshalb steht es hier als Erwartung und nicht als Zahl.

### v0.76.0 – 2026-09-16 (die Grundlinie steht, und die GPU ist zum ersten Mal darin)

**Nur Messwerkzeug und Messwerte**, keine Kiste geändert (θ_v **0.20.0**,
kein Rechenpfad rührt sich).

⚑ **`run.py --prompt-tokens N`**, und der lange Lauf ist ein **zweiter**
und kein geänderter. Der kurze beantwortet „wie schnell erzeugt das
Modell Token", der lange „wie schnell nimmt es einen Prompt auf"; eine
Zahl, die beides beantworten soll, beantwortet keine. Der lange Prompt
entsteht aus einem festen Absatz, wiederholt, damit er auf jeder
Maschine derselbe ist und die Bitgleichheitsprüfung trägt. **Die
Promptlänge steht im Dateinamen des Ergebnisses**, sonst überschriebe
der eine Lauf den anderen.

⚑ **Und damit ist `metal` zum ersten Mal in der Durchsatztabelle**, mit
Zahlen statt mit einer Herleitung:

| Modell | Prompt | cpu-simd Prefill | metal Prefill | Faktor |
|---|---|---|---|---|
| 0,6B | 243 Token | 229,99 tok/s | **865,41 tok/s** | **3,8** |
| 4B | 243 Token | 61,65 tok/s | **300,85 tok/s** | **4,9** |

⚑ **Der Decode gewinnt nichts**, gemessen: 53,41 gegen 53,54 (0,6B) und
22,21 gegen 21,89 (4B), beides innerhalb der Streuung. Das deckt sich
mit dem Grund im Kernel, und es ist der Beleg dafür, dass die Schwelle
von sechzehn Eingaben je Bündel richtig sitzt.

⛔️ **Beim kurzen Prompt wird `metal` verworfen, und das ist richtig.**
Sieben Token liegen unter der Schwelle, die GPU rechnet kein einziges
Bündel. Der Riegel aus v0.75.0 greift also im echten Lauf und nicht nur
im Entwurf.

⛔️ **Was gilt, steht jetzt an einer Stelle.** Ältere Decode-Zahlen in
Changelog-Einträgen sind Aufzeichnungen ihres Tages: Sie nennen ihren
Aufbau nicht, insbesondere nicht die Promptlänge, und die entscheidet
hier über einen Faktor vier. Die Messdatei sagt das jetzt selbst.

⚠️ **Das 30B-A3B fehlt.** 29 GB Artefakt gegen 24 GiB Arbeitsspeicher
heisst auslagern, und ein Durchsatzlauf misst dann überwiegend die
Platte. Er gehört gefahren, wenn jemand daneben sitzt.

### v0.75.0 – 2026-09-16 (die Durchsatzmessung sieht den Rechenweg, der wirklich rechnet)

`runtime` **0.55.0** (θ_v **0.20.0** unverändert, **kein Rechenpfad
ändert sich**; `bench_probe` meldet eine Zeile mehr).

⛔️ **Fund 383: `bench/run.py` kannte `metal` nicht.** Die Liste führte
`reference`, `cpu-simd`, `cuda` und `rocm`. Der Weg, der seit v0.72.0
den Prefill des 30B von 35,3 auf 10,7 s gebracht hat und im
ausgelieferten Klienten auf macOS läuft, stand nicht darin. **Eine
Grundlinie ohne den Weg, der wirklich rechnet, misst etwas anderes und
nennt es die Grundlinie.**

⛔️ **Und der Riegel dazu, denn sonst wäre es Fund 33 in der
Durchsatzmessung.** Die Bitgleichheit ist hier ihre eigene Falle: Ein
Lauf unter dem Namen `metal`, der die CPU gerechnet hat, trägt
**denselben** Digest und denselben Token-Hash; die Gleichheitsprüfung
ginge durch, und in der Tabelle stünde eine Zeile `metal` mit den Zahlen
der CPU. `bench_probe` meldet jetzt `metal_buendel`, den Zähler der
gerechneten Bündel, und `run.py` verwirft einen Metal-Lauf, der bei null
steht, mit Grund.

⚑ **Der Riegel hat beim ersten Lauf sofort etwas gefangen.** `metal` auf
dem 0,6B: **null Bündel, verworfen.** Der Grund ist kein Fehler, sondern
der Aufbau der Messung: Der feste Prompt sind **sieben Token**, und die
GPU rechnet erst ab **sechzehn** Eingaben je Bündel. ⚠️ **Damit kann
diese Messung den Rechenweg baulich nie erreichen**, denn er gewinnt
beim Prefill langer Prompts und beim Decode nie. Ein zweiter Messaufbau
mit realistischer Promptlänge steht aus.

⛔️ **Die Messwerte der alten Reihe stehen unter einem Vermerk.** Die
einzige Durchsatztabelle stammt vom 2026-08-20 und misst Qwen2.5-0,5B
und Qwen2.5-7B, also eine Reihe, die seit dem 2026-09-11 nicht mehr
gemessen wird. Sie bleibt als Aufzeichnung stehen und sagt jetzt selbst,
dass sie eine abgelöste Reihe misst: **Ein Dokument, dessen Zahlen
überholt sind und das es nicht sagt, ist keine Auskunft, sondern eine
Falle.**

⚠️ **Gemessen am 2026-09-16, `myelith-0.6b`, sieben Prompt-Token, 32
Decode-Token:** `reference` 99,48 / 42,63 tok/s, `cpu-simd` 127,58 /
55,96 tok/s (Prefill / Decode), Bitgleichheit über beide Wege bestätigt.
⛔️ **Das ist noch keine Grundlinie**, denn der Kopf dieses Dokuments
nennt für dasselbe Modell 29,4 Tok/s Decode, und keine der beiden Zahlen
nennt die Bedingungen, unter denen sie entstanden ist. Was gilt, ist zu
klären, bevor gegen eine von beiden optimiert wird.

### v0.74.0 – 2026-09-16 (welche Rechenwege hier rechnen, und wie sich einer abschalten lässt)

`runtime` **0.54.0** (θ_v **0.20.0** unverändert, **kein Rechenpfad
ändert sich**). Eine Naht, kein Rechnen: `rechenwege::vorhanden` reicht
`rechenpfad::mit_rechenpfad` durch, `rechenwege::setzen` nimmt einen
Rechenweg aus dem Pfad oder holt ihn zurück.

⚑ **Damit ein Aufrufer die Kernkiste nicht kennen muss**, dieselbe
Überlegung wie bei `kapazitaet`. Der Client zeigt je gefundenem Gerät
einen Regler und muss dafür wissen, ob über dieses Gerät überhaupt
gerechnet wird; müsste er dafür die Kernkiste in sein eigenes Manifest
schreiben, wäre die Naht keine.

⚑ **Gefragt wird, nicht wiederholt.** Die Bedingung steht dort, wo der
Code ausgewählt wird, und dieses Modul formuliert sie nicht neu. Der
Grund steht seit dem 2026-08-22 im Kopf von `rechenpfad.rs`: Eine zweite
Fassung derselben Bedingung lief dort schon einmal auseinander und
meldete einen Rechenpfad, den es nicht gab.

⚠️ **Genau ein Rechenweg hat heute einen Schalter**, nämlich `metal`
über die Bündelschwelle. `cuda` und `rocm` reichen an die Referenzkernel
weiter und haben nichts zum Abschalten; `setzen` sagt das mit seinem
Rückgabewert. ⚑ **Ein stilles Ja wäre die Zusage, etwas getan zu haben,
das nicht geschehen ist.**

⚑ **Aus und wieder an ist eine Nullbewegung.** Die Schwelle, mit der der
Prozess gestartet ist, wird beim ersten Schalten festgehalten und beim
Zurückschalten wiederhergestellt. Sonst überginge das Einschalten ein
gesetztes `MYL_METAL_AB`, und ein Regler, den jemand hin und her
schiebt, hätte die Einstellung stillschweigend geändert.

### v0.73.0 – 2026-09-16 (Entscheidungstreue: der Maßstab, der den Einsatz misst)

`runtime` **0.53.0** (θ_v **0.20.0** unverändert, kein Rechenpfad ändert
sich). Auf die Frage des Projektinhabers, ob wir den falschen Benchmark
zur Verifikation nutzen.

⚑ **Die Verifikation hängt nicht an der Perplexität**, und das bleibt so:
Bitgleichheit wird über die 48 Konformitätsvektoren nachgewiesen. ⛔️ **Die
Qualitätsaussage hängt an ihr, und die misst am Einsatz vorbei.**
Perplexität misst unter Teacher-Forcing, also mit einem Kontext, der an
jeder Position auf die Wahrheit zurückgesetzt wird; eine Abweichung bei
Token `t` wirkt nicht auf `t+1`. Der Client generiert frei, und **seine
Werkzeugaufrufe sind JSON**: Eine andere Entscheidung ist dort ein
anderer Dateipfad.

⛔️ **Der Widerspruch stand in den eigenen Daten.** Qwen2.5-0,5B, dieselben
Sequenzen: Perplexitätsabstand +2,11 % („Kriterium erfüllt"),
Top-1-Übereinstimmung 89,7 %, identische Generierungen 3 von 8. Für die
**eingesetzten** Modelle führt die Modellkarte die beiden letzten als
*nicht gemessen*.

⚑ **Neu: `entscheidungsprobe` (runtime) und `entscheidungstreue.py`.**
Sie geben je Position die Entscheidung aus statt nur deren
Wahrscheinlichkeit, und **der Entscheidungsabstand der Referenz teilt die
Positionen**: Wo die Referenz selbst schwankt, ist eine andere Wahl eine
Münze; wo sie sicher war, ist es ein Mangel. Ohne diese Trennung ist eine
Abweichungsquote nicht deutbar.

**Pilot auf `myelith-0.6b`, 13 797 Positionen, 6 Minuten 8 Sekunden:**

| | Wert |
|---|---|
| Top-1-Übereinstimmung | **86,20 % ± 0,58** |
| Positionen mit sicherer Referenz | 6 768 |
| Abweichungen davon | 97 (**1,43 % ± 0,28**) |
| Entscheidungsabstand, wo einig / uneinig | 1,25 / 0,25 nat |

⚑ **Die Abweichungen sitzen überwiegend an Beinahe-Gleichständen**, aber
nicht alle: 1,43 % der sicheren Entscheidungen gehen anders aus, und das
Band schließt die Null nicht ein. ⚑ **Es ist die erste Qualitätszahl
dieses Projekts mit engem Fehlerband**, weil sie gepaart je Position
misst statt gepoolt über vier Sequenzmittelwerte.

⚠️ **Der Pilot misst noch auf WikiText-2.** Gebraucht wird die
Einsatzverteilung: Chat-Vorlage, Werkzeugaufrufe, lange Kontexte.
Einzelheiten im Bericht `Entscheidungstreue-statt-Perplexitaet-2026-09-16.md`.

### v0.72.4 – 2026-09-16 (der Boden des 30B ist gemessen, und das Urteil ist entfallen)

Keine Kiste ändert sich (θ_v **0.20.0**); gemessen wird, und ein
Messwerkzeug hört auf, mehr zu behaupten, als es weiss.

⚑ **Der Boden des 30B-Gemischs ist gemessen.** Lauf am 2026-09-16,
3 Stunden 17 Minuten, über 435 Positionen und damit über dieselbe
Stichprobe wie sein Ganzzahlwert. ⚠️ **Eine grössere Stichprobe wäre
hier ein Fehler gewesen**, kein Mehrwert: Der Ganzzahlwert des 30B steht
auf 435, und ein Boden ohne vergleichbaren Partner lässt sich mit nichts
verrechnen.

| | Perplexität | gegen die Referenz |
|---|---|---|
| Gleitkomma | 10,48 | |
| nur Gewichte (W8) | 10,48 | −0,01 % |
| **Boden (W8A16)** | **10,44** | **−0,43 %** |
| Ganzzahlpfad | 10,42 | −0,59 % |
| A16 an jedem Linear-Eingang | 10,55 | +0,63 % |

⛔️ **Fund 379: das Urteil prüfte, ob die Zahlen vergleichbar sind, aber
nicht, ob sie etwas tragen.** Der Riegel aus Fund 375 verlangt dieselbe
Stichprobe für beide Seiten, und das ist richtig; er sagt aber nichts
darüber, ob die Stichprobe **gross genug** ist. Also druckte das
Werkzeug auf diesen Zahlen „ist am BODEN DES SCHEMAS, Feilen an der
Umsetzung bringt nichts mehr", **eine Handlungsanweisung aus einem
negativen Boden**. ⚑ Ein negativer Boden kann nicht sein: Quantisierung
vernichtet Information. Er ist keine Aussage über das Schema, sondern
über die Stichprobe. Beim 4B stand derselbe Befund (−2,45 % über 435
Positionen) und schrumpfte über 13 797 Positionen auf −0,07 %.

⚑ **Das Werkzeug urteilt jetzt in zwei Fällen nicht mehr** und sagt
warum: bei negativem Boden und bei einem Abstand unter einem Prozent
(die 1-%-Regel des Projekts). Die erzeugte Übersicht trägt dieselbe
Unterscheidung, und sie nennt die Spalte mit den Positionen als das,
was entscheidet, ob eine Zeile mit dem Abstand verrechnet werden darf.

📌 **Zweimal dieselbe Lehre, und beim zweiten Mal an derselben Datei.**
Fund 375 war eine Zahl aus einer fremden Messung, Fund 379 eine Zahl aus
einer zu kleinen. **Ein Werkzeug, das ein Urteil druckt, muss vorher
wissen, ob seine Eingaben eines tragen.**

### v0.72.3 – 2026-09-15 (der Kopf trug noch die zurückgezogenen Zahlen)

Keine Kiste ändert sich (θ_v **0.20.0**); ein Text wird berichtigt und
eine Messung gestartet.

⛔️ **Der Kopf dieses Dokuments meldete einen Umsetzungsverlust, den es
nicht gibt.** Dort standen +3,65 % (0,6B) und +4,20 % (4B) als Ergebnis,
samt der Folgerung, das 4B habe den kleineren Abstand und den grösseren
Umsetzungsverlust. Beide Zahlen stammen aus der Rechnung Abstand minus
Boden über **verschieden grosse** Stichproben, und genau diese Rechnung
ist einen Abschnitt weiter unten als unzulässig beschrieben. **Ein
Dokument, das im Kopf behauptet, was es im Rumpf widerlegt, ist an der
sichtbarsten Stelle falsch.**

⚑ **Jetzt steht dort, was gemessen ist:** Boden **+1,61 %** (0,6B) und
**−0,07 %** (4B) über 13 797 Positionen, die erste Messung über 435
Positionen als zurückgezogen benannt, und der Umsetzungsverlust als
**offene** Zahl, die einen eigenen Messlauf braucht.

⚑ **Der 30B-Bodenlauf ist gestartet**, über 435 Positionen und damit
über dieselbe Stichprobe wie sein Ganzzahlwert. Eine grössere Stichprobe
gäbe einen Boden, der sich mit nichts verrechnen liesse. ⚠️ **Er ist
plattengebunden:** Die BF16-Referenz ist 57 GB, die Maschine hat 24 GiB,
und der Prozess verbrauchte in fünf Minuten Wanduhr 26 Sekunden CPU.

### v0.72.2 – 2026-09-15 (der Boden des Schemas gilt für die laufende Reihe)

Keine Kiste ändert sich (`kernels` **0.56.0**, `runtime` **0.52.0**, θ_v
**0.20.0**); gemessen wird, und ein Messwerkzeug wird repariert.

⛔️ **Fund 375: Der Bodenmesser rechnete gegen eine fremde Zahl und
urteilte trotzdem.** `tests/diag/w8a16_reference_simulation.py` nahm die
Ganzzahl-Perplexität aus `INTEGER_PPL` mit der Vorgabe **9,40**, dem Wert
der abgelösten Qwen2.5-7B vom 2026-08-20. Für jedes andere Modell war der
ausgewiesene Abstand sinnlos, und das Skript druckte trotzdem ein
Urteil: Beim 0,6B kam „−70,50 %" heraus und daraus „wir sind am Boden des
Schemas, Feilen an der Umsetzung bringt nichts mehr", **genau die
falsche Schlussfolgerung**. Die Zahl kommt jetzt aus dem Vergleich, den
die Messung je Modell schreibt; fehlt er, bricht der Lauf ab, statt sich
eine zu leihen. Dazu zeigte der Importpfad auf ein `eval`-Verzeichnis,
das es nicht mehr gibt, und der Lauf brach schon am Import ab. **Ein
Werkzeug, das nur bei Grundsatzfragen gerufen wird, verfällt zwischen
zwei Fragen, ohne dass es jemand merkt.**

⛔️ **Und die erste Messung war zu klein, um etwas zu tragen.** Auf den
435 Positionen der Reihe kam beim 4B ein Boden von **−2,45 %** heraus,
also eine Quantisierung, die das Modell verbessert, und beim 0,6B
**+0,80 %**. Beide Male wanderte das Vorzeichen der Zwischenschritte,
und reine Gewichtsquantisierung, die keinerlei Kalibrierdaten benutzt,
machte das 4B angeblich um 1,2 % besser. Auf **13 797 Positionen** ist
davon nichts übrig:

| Modell | Positionen | Gleitkomma | Boden des Schemas |
|---|---|---|---|
| 0,6B | 13 797 | 42,31 | 42,99 (**+1,61 %**) |
| 4B | 13 797 | 28,09 | 28,07 (**−0,07 %**, also nichts) |

⚑ **Jeder Zwischenschritt ist jetzt schlechter als die Referenz**, wie es
sein muss: Quantisierung vernichtet Information. Das Schema kostet beim
0,6B rund anderthalb Prozent und beim 4B praktisch nichts.

⚠️ **Der Umsetzungsverlust lässt sich daraus noch nicht bilden.** Der
Ganzzahlpfad ist über 435 Positionen gemessen, der Boden über 13 797;
zwei Stichproben sind zwei Texte, und ihr Verhältnis wäre eine erfundene
Zahl. Das Werkzeug weigert sich deshalb, sie zu bilden. Für einen
belastbaren Wert muss auch der Ganzzahlpfad über denselben Umfang laufen.

📌 **Eine Zahl mit zu wenig Text dahinter ist keine Zahl.** Ein einzelnes
Token, dessen Wahrscheinlichkeit um eine Grössenordnung springt,
verschiebt bei 435 Positionen die Perplexität um ein halbes Prozent.

⚠️ Der Boden wird als Ergebnisdatei abgelegt statt nur gedruckt, und für
das 30B-Gemisch steht er aus: Dessen BF16-Referenz ist 57 GB und lagert
auf dieser Maschine durchgehend aus. Der Lauf ist als eigenes Skript
vorbereitet, das einen zweiten Messlauf daneben verweigert.

### v0.72.1 – 2026-09-14 (der Kalibrierungstest deckt einen Prompt über vier Token)

Keine Kiste ändert sich (`kernels` **0.56.0**, `runtime` **0.52.0**, θ_v
**0.20.0**); der Ausfuhrtest wird nachgezogen.

⛔️ **Fund 368 in der CI:** `test_synthetic_export_loads_in_real_runtime_binary`
baute eine Drehtabelle mit vier Zeilen. Die Kontextgrenze ist das Minimum
aus `max_context` und den Tabellenzeilen, also vier, und der Lader hielt
am fünften Token des Prompts „Hello" an. Die synthetische Tabelle trägt
jetzt acht Zeilen, genug für den Prompt und drei erzeugte Token.

### v0.72.0 – 2026-09-14 (das Gespräch wächst mit, der Kontext reicht bis 40 960)

`kernels` **0.55.0 auf 0.56.0**, `runtime` **0.51.0 auf 0.52.0**, θ_v
**0.20.0**. Keine Zahl im Rechenpfad ändert sich; die ersten 2 048 Zeilen
der Drehtabelle sind bytegleich.

⚑ **Der KV-Speicher setzt ein Gespräch fort** (Fund 372): Der gemeinsame
Anfang bleibt stehen, nur der neue Teil wird gerechnet. Ein Agentenlauf
mit acht Schritten fiel am 4B von 116 auf 58 s, bit-gleich zur frischen
Rechnung (`fortgesetzt_ist_dasselbe_wie_frisch`).

⚑ **Das Expertengemisch auf Metal:** der LM-Kopf verteilt und
speicherabgebildet statt einkernig und kopiert (Fund 370), die Experten
der Vorbereitung gebündelt statt tokenweise (Fund 371), die GPU-Schwelle
gemessen statt geraten. 30B-Prefill von 992 Token 35,3 auf 10,7 s,
Abdruck unverändert.

⚑ **Die Aufmerksamkeit bei langem Kontext:** KV-Speicher zusammenhängend
je Ebene und Kopf, ein Kern je Abfrage mit NEON und einer i32-Gewichtung,
deren Grenze je Aufruf geprüft wird, die Köpfe über die Fäden verteilt.
Decode 108 auf 48 ms je Token bei 1 907 Token; 48/48 auf `reference`,
`cpu-simd`, `metal`.

⛔️ **Fund 368 geschlossen, θ_v 0.20.0:** `max_context` und die Tabelle
der Drehpositionen auf 40 960; die Grenze gilt, bricht nicht mehr still
um, und die Erzeugung hält an ihr an. Kontext bis 32 768 auf 24 GiB
gemessen, Nadelsuche bei 8k/16k/32k auf allen drei Modellen gefunden.

📌 **Fund 373:** Der Nachrechner lehnt eine Position hinter der ersten
ohne den KV-Verlauf ab, statt eine falsche Spur zu rechnen. **Fund 374:**
`layer_granular` prüft den KV-Speicher jetzt über Positionen.

### v0.71.0 – 2026-09-14 (die Rechenwege lassen sich zur Laufzeit umschalten, und zwei Angaben stimmen nicht)

`kernels` **0.54.0 auf 0.55.0**. Keine Zahl im Rechenpfad ändert sich.

⚑ **Zwei Schalter für den ganzen Prozess**, damit ein Lauf alle
Rechenwege einer Maschine vergleichen kann statt zweier Bauten:
`dot::skalar_erzwingen` nimmt die skalare Referenzfassung auch in einem
vektorisierenden Bau, `metal::schwelle_setzen` setzt, ab wie vielen
Eingaben die GPU rechnet (`0` nie, `1` immer). Beide mit Zähler als Beleg
(`dot::erzwungen_skalar_gerechnet`, `metal::gerechnet`). Dazu
`rechenpfad::uebersetzt`, die Features dieser Kiste, und
`metal::nicht_verfuegbar_weil`.

**Kosten im Normalbetrieb:** ein Atomlesen je Matrixzeile. Gemessen im
Wechsel A B B A A B (4B, `cpu-simd`, Decode über 64 Token): 15,28 gegen
15,35 Token/s, innerhalb der Streuung von 2 %.

⛔️ **Fund 368: Die Kontextgrenze gilt nirgends.** Die Artefakte tragen
`max_context` 2048, und die Tabelle der Drehpositionen hat genau 2 048
Einträge. Keine Stelle prüft das; ab Position 2 048 dreht die Rechnung
wieder wie Position 0 (`pos % n_pos`), ohne Meldung, auf jeder Maschine
gleich. Offen: die Zielgröße des Kontexts und das Verhalten an der
Grenze.

⚠️ **Fund 369: Die Softmax dividiert.** Die Regel des Projekts lautet
„Division ausschließlich als arithmetischer Rechtsshift"; `softmax_int`
normiert mit einer Ganzzahldivision samt Runden zur geraden Zahl.
Deterministisch, denn beide Operanden sind nicht negativ, aber strenger
formuliert als umgesetzt. Offen: die Regel genauer fassen oder die
Stelle benennen.

### v0.70.0 – 2026-09-14 (die Vorbereitung auf der GPU, und zwei Wege, die niemand benutzte)

`kernels` **0.53.0 auf 0.54.0** (Metal, verteiltes SiLU-Produkt),
`runtime` **0.50.0 auf 0.51.0** (gebündelte Aufmerksamkeitshälfte,
`prompt_vorbereiten`). Keine Zahl im Rechenpfad ändert sich: 48/48 gegen
`reference`, `cpu-simd` und `metal`.

| Vorbereitung, 219 Token, bitgleich | 0,6B | 4B |
|---|---|---|
| Client-Weg vorher: tokenweise, ohne `cpu-simd` | 5,21 s | |
| tokenweise, `cpu-simd` | 3,95 s | 9,69 s |
| gebündelt, nur der MLP (bisher nur im Messprogramm) | 2,83 s | 7,45 s |
| **jetzt, `cpu-simd`** | **0,97 s** | **3,52 s** |
| **jetzt, `metal`** | **0,28 s** | **0,81 s** |

Ein Prompt von 1 684 Token braucht beim 4B 35,3 s mit `cpu-simd` und
10,0 s mit `metal`.

⛔️ **Fund 366: Die gebündelte Vorbereitung lief nur im Messprogramm.**
`vorbereiten_stapel` war gemessen und geprüft, aufgerufen hat ihn nur
`bench_probe`; Erzeugung, Konformitätslauf und Testclient bereiteten
Token für Token vor. Jetzt rufen alle `Model::prompt_vorbereiten`, in
Fenstern von 512 Token.

⚑ **Die Aufmerksamkeitshälfte ist gebündelt.** Ein Profil zeigte 65 %
der Vorbereitung dort, Token für Token. q, k, v und o laufen jetzt als
Bündel; der KV-Speicher wird je Ebene zuerst in der Reihenfolge der
Positionen gefüllt, danach liest jedes Token nur die Positionen bis zu
seiner eigenen, verteilt über die Fäden. Normen, Drehungen, Residuen und
das SiLU-Produkt hängen nur am eigenen Token und laufen ebenfalls
verteilt. **Eine Umsetzung**: Der tokenweise Weg ruft dieselben Schritte
mit einer Eingabe.

⚑ **Metal** (Feature `metal`, nur macOS, schließt `cpu-simd` ein):
`matmul2d` aus Metal Performance Primitives über zwei int8-Stellen je
Aktivierung, zusammengesetzt in int64, gerundet und geklemmt wie auf der
CPU. Die Zeilensummen kommen aus derselben Multiplikation. **Jeder
Prozess prüft die GPU vor dem ersten Gebrauch gegen die CPU**, mit den
Extremwerten beider Stellen und allen Kachelrändern, und rechnet ohne
sie weiter, wenn sie abweicht. `run.sh metal` erzwingt die GPU
(`MYL_METAL_AB=1`) und lehnt einen Lauf ab, in dem sie nicht gerechnet
hat. Der Decode bleibt auf der CPU, gemessen bandbreitengebunden.

📌 **Die Vorbereitungsprüfung sah Fehler in der Aufmerksamkeit nicht.**
Ihr Testmodell hat eine Ebene und Gewichte `i % 7`; eine Aufmerksamkeit,
die eine Position zu weit las, blieb dort gemessen grün. Jetzt vergleicht
sie den Strom jedes Tokens und den KV-Speicher, auf Zufallsgewichten,
und jede der zehn Gegenproben fällt auf.

### v0.69.0 – 2026-09-14 (θ_v 0.19.0: der Akkumulator heißt, was er rechnet, und die SiLU endet nicht mehr bei 128)

`runtime` **0.49.0 auf 0.50.0**, weil sie `theta_v/spec.json` einbettet;
`kernels` **0.52.0 auf 0.53.0** (SiLU, Fund 349).

⛔️ **Fund 349 und Fund 364: Die SiLU klemmte jenseits ihrer Tabelle
flach auf 128, auch in der Inferenz.** Die Tabelle deckt die reale
Domäne −128 bis knapp 128; gemessen reichen die Gate-Werte bei
`myelith-0.6b` bis **278** und bei `myelith-4b` bis **192** (beim
Gemisch bis 57, dort ohne Wirkung). SiLU(x) ist dort x und nicht 128.
Der Rückwärtspass leitete seinen Gradienten aus der Tabelle ab und nahm
jenseits schon Steigung 1 an, der Vorwärtspass lag flach; der
Kontrolllauf mit umgekehrtem Gradienten scheiterte genau daran.

**Jetzt setzt die SiLU jenseits der Tabelle fort:** oberhalb die
Identität, unterhalb null, an beiden Rändern stetig
(`integer_math::silu_nachschlagen`, rückwärts
`backward::silu_ableitung_nachschlagen`, festgelegt in θ_v unter
`nonlinear.silu.outside_input_range`). `lut_lookup` verlangt einen Index
**in** der Tabelle statt nur in `i16`. Das Verfahren folgt der
Zerlegung SiLU(x) = x · σ(x): Beschränkt wird der Faktor, der sättigt,
nicht die ganze Funktion.

**Die Wirkung, gemessen:** Zwei von 49 Vektoren ändern ihre Werte,
`layer_27` (644 von 1 024 Kanälen der letzten Ebene des 0,6B) und
`e2e_hello` (erstes Token „ Answer" statt „ Name"); die übrigen 47 sind
bis auf das Hashfeld gleich. Perplexität 0,6B **33,2828 auf 33,2884**,
4B **19,9495 auf 19,9519**, also +0,017 und +0,012 Prozent, je zwei
Sequenzen besser und zwei schlechter: **innerhalb der Streuung**. Der
Kontrolllauf ist grün. 48/48 gegen beide Backends, `894d8357ae92b5c1`
unverändert.

**Die Berichtigung des Akkumulators allein hat keine Zahl im Rechenpfad
geändert**, und das ist belegt: Vor der SiLU-Änderung waren alle 49
Vektordateien aus dem Code neu erzeugt und unterschieden sich von ihrem
Vorgänger ausschließlich im Feld `theta_v_hash`. 48/48 gegen
`reference` und `cpu-simd`, der Operations-Abdruck bleibt
`894d8357ae92b5c1`.

⚑ **Fund 356 geschlossen: θ_v legt den Akkumulator jetzt als int64
fest.** Die Summe einer Zeile w8 · a16 wird exakt in int64 gebildet,
danach reskaliert und geklemmt. Das rechnet der Pfad seit θ_v 0.5.0;
nachgezogen ist jetzt die Festlegung. Eine Umsetzung darf in kleineren
Breiten sammeln, solange keine Teilsumme überläuft und das Ergebnis dem
int64-Wert gleicht. Der Grund, warum das mehr als eine Wortfrage war:
int32 trägt beim größten Produktbetrag 4 161 409 nur 516 Terme, eine
Zeile von `down_proj` des 4B hat 9 728.

**Was eine neue θ_v-Fassung nach sich zieht, in dieser Reihenfolge:**
die drei Artefakte über den Exportweg neu gestempelt
(`export_json`, dieselben Hashes, neue Fassung), daraus die
Skalenpakete und das Register neu gebaut (drei neue Artefakt-Digests),
die Golden Vectors neu erzeugt, die Pipeline-Manifeste auf die neue
kanonische Kennung gesetzt.

📌 **Fund 358: Die Prüfung, die neue Vektoren verlangen sollte, lehnte
nie etwas ab.** `tests/regression/test_theta_v_changes.py` suchte in
einem Ordner ohne die maßgeblichen Vektoren, rechnete den Hash anders als
die Erzeuger und gab Abweichungen als Warnung aus. Sechs Vektoren
trugen seit dem 2026-08-20 den Hash einer älteren Fassung. Jetzt prüft
sie `conformance/vectors` und die Pipeline-Manifeste, scheitert bei
einer Abweichung (Gegenprobe in beide Richtungen) und läuft in der CI.

📌 **Fund 359: Der Abschnitt `model` in `spec.json` beschrieb das
abgelöste Qwen2.5-0.5B.** Kein Leser, deshalb unbemerkt. Er nennt jetzt
das Ankermodell der Vektoren und sagt, dass Dimensionen aus dem
Artefakt kommen.

⛔️ **Fund 360: Keine der vier Pipeline-Konfigurationen war startfähig.**
`pipeline_4node.json` war am 2026-09-12 auf 28 Ebenen umgeschnitten
worden, trug aber den alten `pipeline_hash` und im Feld `theta_v_hash`
den Hash von `spec.json` statt der kanonischen Artefakt-Kennung. Die
30B-Konfiguration passte schon unter 0.18.0 nicht, die beiden übrigen
waren noch auf 24 Ebenen des abgelösten Modells geschnitten. Belegt
durch einen echten Start je Konfiguration. Neu geschnitten auf 28
Ebenen (4 Stufen 7/14/21, 8 Stufen 4/7/11/14/18/21/25, ungleichmäßig
1/9/27), die Hashes vom Pipeline-Code selbst ermittelt. **Danach:
Mehrknotentest deterministisch über zwei frische Läufe, und alle drei
Zuschnitte liefern dieselben Token wie der Einzelknoten.** Keiner der
beiden Tests läuft in der CI, weil er Artefakte braucht; genau deshalb
blieb es unbemerkt.

📌 **Fund 361: `theta_v.json` des 4B und des 30B stand nicht im
Exportformat.** Beim Wechsel auf 0.18.0 eingerückt gestempelt; wer
diese Modelle frisch kalibriert, hätte kompakte Bytes und damit einen
anderen Artefakt-Digest als das Register erhalten. Jetzt einheitlich
über `export_json`.

📌 **Nachgezogen:** Kopf und Verzeichnisbaum des Konformitätspakets
(θ_v 0.17.0, 24 Ebenen), die Regel 3 im θ_v-README (sie behauptete eine
Hashprüfung, die kein Knoten macht) und in diesem README die Datierung
des Wechsels auf int16 (θ_v 0.5.0, nicht 0.16.0) sowie der Abschnitt
„Modell-Austauschbarkeit".

### v0.68.1 – 2026-09-14 (drei Angaben, die nicht mitgewachsen sind, und eine offene Frage an θ_v)

Keine Kiste angefasst, keine Zahl im Rechenpfad geändert.

⛔️ **Fund 352: das deutsche Glossar beschrieb die Rundung falsch.**
Beim arithmetischen Rechtsshift stand dort „das halbe LSB wird
addiert", also Aufrunden bei einem Rest von genau der Hälfte. Alle drei
Varianten in `kernels/src/fixed_point.rs` runden in diesem Fall zur
**geraden** Nachbarzahl, und die englische Fassung sagte es richtig.
⚑ **Das Gewicht liegt in der Rangfolge:** Bei Abweichungen gilt laut
Glossarkopf die deutsche Fassung, also ausgerechnet die falsche. Wer den
Rechenpfad nach ihr nachbaut, liegt bei jedem Rest von genau der Hälfte
in der Hälfte der Fälle um eins daneben.

📌 **Fund 353: der Status dieses READMEs zählte das entfernte 14B mit.**
Der Kopf sprach von „allen vier Modellen" und trug das Datum vom
2026-09-11, unter „Ziel" standen Qwen2.5-0,5B als Referenzmodell und
Qwen2.5-7B als verifiziert. Beide sind seit dem 2026-09-11 abgelöst,
das 14B seit dem 2026-09-12. **Eine Statuszeile, die unter der
Versionszeile steht, wächst nicht mit ihr**, und das ist hier schon
zweimal passiert. ⚠️ Der Abschnitt „Modell-Austauschbarkeit" darunter
beschreibt ebenfalls einen älteren Stand (QK-Norm als fehlend) und ist
noch nicht nachgezogen.

⚠️ **Fund 356, offen: Welche Breite hat der Akkumulator?**
`theta_v/spec.json` legt unter `numeric.formats.accumulator` **int32**
fest, und dieses README übernimmt das. Der Rechenpfad akkumuliert aber
exakt in **i64** (`dot_i8_i16`, `linear_w8a16`). Das ist mehr als eine
Formulierung: Eine Zeile der MLP-Matrix des 4B hat 9 728 Eingänge, und
127 · 32 767 · 9 728 liegt bei rund 4 · 10¹⁰, weit über i32. Eine
Umsetzung, die sich an die Festlegung hält, kann damit andere Bits
rechnen als die Referenz. Die Festlegung ist in den Lader eingebettet,
und ihr SHA-256 steht in jedem Golden Vector (`golden_generate`); sie
wird deshalb nicht nebenbei geändert, sondern entschieden.

### v0.68.0 – 2026-09-12 (die Klemme sass hinter dem Überlauf)

`kernels` **0.51.0 auf 0.52.0**.

⛔️ **Fund 348.** `integer_math::lut_lookup` addierte Index und Versatz
in `i16` und klemmte **danach**. Ein Index oberhalb von `i16::MAX`
läuft dabei in den negativen Bereich um, bevor die Klemme ihn sieht,
und `.max(0)` schiebt ihn auf null. **Ein Index, der oben hätte
sättigen müssen, bekam den Wert vom unteren Ende der Tabelle.**

Gemessen: `25412 + 8192 = 33604` läuft auf `−31932` um, `.max(0)` macht
daraus `0`; `silu` liefert die Antwort für den kleinsten Eingang statt
für den grössten.

⚑ **Mit Zusicherungen fällt das auf, ohne sie nicht**, und ausgeliefert
wird ohne. Die Addition rechnet jetzt in `i32`, geklemmt wird danach.
⚠️ **An einem gültigen Lauf ändert das nichts**: 48/48
Konformitätsvektoren unverändert. Es ändert nur, wohin ein Verstoss
gegen die Vorbedingung sättigt.

📌 **Gefunden, weil endlich das richtige Testprofil lief.** Die
Prüfungen dieser Sitzung liefen mit `cargo test --release`, und das
schaltet `debug-assertions` und `overflow-checks` ab. Das Projekt hat
dafür ein eigenes `[profile.test]`: `opt-level = 2` **und** beide
Prüfungen an, also schnell und streng zugleich.

⚠️ **Fund 349 bleibt offen**, siehe die Planung: Der Bergauf-Kontrolllauf
verlässt die dokumentierte Vorbedingung des Tabellenzugriffs. Ob in den
Trainingspfad eine Klemme **vor** den Zugriff gehört, ist eine Änderung
am Rechenpfad und braucht eine eigene Messung. **Der Test ist nicht grün
gemacht worden**, indem die Schrittzahl heruntergedreht wurde.

### v0.67.0 – 2026-09-12 (das dichte 14B ist entfernt)

**Festlegung des Projektinhabers.** Begründung: schlechterer Durchsatz
als das Gemisch (5,25 gegen 10,1 Token je Sekunde), schlechtere
Perplexität (11,54 gegen 10,42), aufwendigeres Training, und 46 GB auf
der Platte ohne Mehrwert.

⚑ **Ein Befund aus dem Messen stützt es.** Auf einer Maschine mit
24 GiB ist das dichte 14B **unhandlicher als das grössere 30B**: Bei
identischer Einstellung kostete die Perplexitätsmessung 19 Minuten
gegen zehn, und acht Trainingsdurchgänge über vier Stunden gegen
fünfzig Minuten. Ein dichtes Modell fasst bei jedem Vorwärtspass alle
16,3 GB an, ein Gemisch je Token acht von 128 Experten.

⚠️ **Was dabei verlorengeht, ist benannt und abgewogen.** Ohne das
dichte 14B verliert die Aussage „das Gemisch schlägt ein dichtes Modell
mit halb so vielen Parametern" ihren Vergleichspunkt, und die dichte
Reihe fällt von drei Punkten auf zwei, womit ein Grössenvergleich
wieder einen Architekturunterschied mitmisst. Der Projektinhaber hat
das gegen den Plattenbedarf abgewogen.

**Entfernt:** Katalogeintrag, Registereintrag samt Skalenpaket,
Modellkonfiguration, und die Modellzeile aus **beiden** Werkzeugen von
GOVERNANCE.

⚑ **Die Prüfungen sind auf ein Kriterium umgestellt statt auf ein
Modell.** `test_gptq_hessian_bedarf_waechst_quadratisch` stand auf der
7B, dann auf der 14B, und beide sind gegangen. Sie prüft jetzt das
**Gesetz** (der Bedarf wächst schneller als die Grösse) und das
**Kriterium**, das der Code selbst anlegt (passt der Bedarf in zwei
Drittel des Speichers). **Eine Schranke, die an einem bestimmten Modell
hängt, ist keine Schranke, sondern ein Ablaufdatum.**

⚑ **Die Messungen bleiben.** `results/*myelith-14b*` steht weiter im
Baum: die Perplexitätszeile (11,54 gegen 11,41), der Durchsatz (5,25
Tok/s) und die Zeitmessung sind die Belege für den Vergleich, und sie
bleiben gültig, wenn ihr Gegenstand aus dem Baum ist. **Messgeschichte
wird nicht umgeschrieben.**

### v0.66.0 – 2026-09-12 (das Expertengemisch trainiert, und zwei Meldungen sahen ihm nicht zu)

`runtime` **0.48.0 auf 0.49.0**.

**Auftrag des Projektinhabers:** prüfen, ob für jedes Modell, besonders
für das 30B, ein hinreichendes Training verfügbar ist, das nachweislich
richtige Ergebnisse liefert.

**Das Gemisch trainiert, und der Beleg ist deutlich.** Referenzleiter
Stufe 2, eine Ebene (47), acht Durchgänge, gegen den Rauschnullpunkt:

| Myelith 30B-A3B | int8 geändert | Zeilenskalen | lern | **halte** |
|---|---|---|---|---|
| Rauschen, **ohne** Gradienten | 2 044 027 (0,73 %) | 460 | +0,00 % | **−0,02 %** |
| Training, acht Durchgänge | 1 759 812 (0,63 %) | 288 | −11,51 % | **−10,54 %** |

Beide über dieselben 278 659 072 Gewichte, also Aufmerksamkeit, Router
und die **55 von 128** Experten, die der Router berührt hat. ⚑ **Das
Rauschen stört 16 % mehr und erreicht das 527-fache weniger.**

⛔️ **Fund 346: drei Meldezeilen waren auf einem Gemisch blind für die
Experten.** Der Anfangsstand einer Gemischebene trägt keine Experten,
weil ein Experte seinen Master erst bekommt, wenn der Router ihn wählt.
Die Zeilen bildeten `anfang.matrizen().zip(jetzt.matrizen())`, und
**`zip` bricht an der kürzeren Seite ab, ohne ein Wort zu sagen**. Der
Nenner war damit auf das Gewicht genau Aufmerksamkeit plus Router
(19 136 512), während im selben Lauf 261 Millionen Gewichte bewegt
wurden.

**Am schwersten wiegt nicht die Prozentzahl, sondern die
Ausreisserzeile.** Sie ist die Schranke, die einen entgleisten Lauf
sichtbar macht; auf einem dichten Modell hat sie am Vortag Faktor
359,34 gemeldet und damit einen falschen Aufruf aufgedeckt. Auf einem
Gemisch sah sie dem Experten nicht zu. Jetzt meldet sie Matrix 55, also
eine Expertenmatrix.

⚑ **Das Δ-Commitment war nicht betroffen.** `Shardgewichte::deltas`
läuft über die **Master**-Seite und setzt für einen fehlenden Anfang
eine Nullmatrix ein; die Experten sind im Abdruck enthalten. Betroffen
war die Anzeige, nicht der Konsens. Berichtigt über
`Shardgewichte::paare_mit_anfang`, die den fehlenden Anfang eines
Experten aus dem Modell nachbildet.

📌 **Fund 345: die wichtigste Zahl eines Gemischlaufs stand nirgends.**
`trainingsschleife` führt seit dem 2026-09-05 `experten_beruehrt`, und
der Testclient zeigt sie. `trainingsguete` zeigte sie nicht, und das
ist das Werkzeug, mit dem jemand einen Trainingslauf misst. „261 345 637
Gewichte bewegt" liest sich wie ein trainiertes Modell; **ein Experte,
den der Router nie wählt, bekommt nie einen Gradienten und bleibt
untrainiert.** Die Durchgangsmeldung nennt jetzt „55 von 128 Experten
beruehrt".

### v0.65.0 – 2026-09-11 (die neue Reihe ist nachgemessen, und dabei fielen drei Messfehler auf)

`runtime` **0.47.0 auf 0.48.0** (Fund 343, die Lernraten-Vorgabe).

**Auftrag des Projektinhabers:** „Lass uns gerne die Perplexität und
auch das Training verifizieren."

| Modell | Gleitkomma | Ganzzahl | Abstand |
|---|---|---|---|
| Myelith 0,6B | 31,86 | 33,28 | **+4,47 %** |
| Myelith 4B | 19,63 | 19,95 | +1,64 % |
| Myelith 14B | 11,41 | 11,54 | **+1,16 %** |
| Myelith 30B-A3B | 10,48 | 10,42 | kein messbarer Abstand |

Gemessen auf denselben vier WikiText-2-Sequenzen und denselben 435
Positionen für alle vier, beide Pfade mit Teacher-Forcing. Der Katalog
führt damit **alle vier Modelle auf `verifiziert`**, zum ersten Mal.

⚑ **Die Reihe hat eine Richtung, und sie ist jetzt belegt statt
plausibel:** Je kleiner das Modell, desto teurer die Quantisierung.
Das kleinste liegt mit +4,47 % als einziges nennenswert nahe an der
Grenze von 5 %.

📌 **Und zwei Prozentzahlen, die gleich aussehen, sind nicht dasselbe.**
Die abgelöste 7B lag bei +1,14 %, die neue 14B liegt bei +1,16 %. Das
ist kein Beleg, dass die Grösse nichts ändert: verschiedene Familien,
verschiedene Grundlinien (8,68 gegen 11,41), verschiedene Modelle.

⛔️ **Fund 339: die Kalibrierung hat seit dem 2026-09-07 keinen WikiText
mehr gesehen.** `calibrate/src/main.py` suchte unter
`INTEGER_LLM/eval/datasets/`; das Verzeichnis ist am 2026-09-07 nach
`BENCHMARKS/Inferenz/datasets/` gezogen, diese Zeile nicht. Die Stelle
**warnte und rechnete weiter**, also mit den rund zwei Dutzend
kuratierten Prompts statt zusätzlich mit 64 WikiText-Sequenzen. Jedes
seither gebaute Artefakt trug Skalen aus einem schmaleren Textbestand,
ohne dass es irgendwo dranstand. Jetzt bricht der Lauf ab
(`FileNotFoundError` mit dem Befehl zum Holen im Text);
`INTEGER_LLM_OHNE_WIKITEXT=1` ist der bewusste Ausweg. **Eine Warnung,
die man übersehen kann, ist bei einer Eingangsgrösse keine Warnung.**
Beide neuen Artefakte sind mit dem vollständigen Satz neu gebaut,
Skalenpakete und Vektoren neu erzeugt, **48/48 gegen `reference` und
gegen `cpu-simd`**.

📌 **Fund 340: eine zitierte Messdatei überschrieben.** Die historischen
Dateinamen (`baseline_wikitext2.json` und die zwei daneben) hingen am
**voreingestellten** Modell. Die Voreinstellung wechselte am 2026-09-11
auf `myelith-0.6b`, und der erste Grundlinienlauf schrieb damit die
Messung der 0,5B zu. Wiederhergestellt aus der Versionsverwaltung; die
Datei war eingecheckt und unverändert, **das ist der einzige Grund,
warum es folgenlos blieb**. Die Regel hängt jetzt am Modell
(`_HISTORISCH = "myelith-0.5b"`), und dieses Modell ist aus dem Projekt
heraus, kann also nichts mehr überschreiben.

📌 **Fund 341: das erzeugte Entscheidungsprotokoll behauptete bei jedem
Modell 0,5 Mrd. Parameter.** Der zweite Mess-Hinweis („kleine Modelle
sind der ungünstigste Fall") stand als fester Text im Rumpf von
`perplexity.py`, und der Rumpf schreibt für alle vier. Im Protokoll der
14B stand deshalb wörtlich „0,5 Mrd. Parameter sind der ungünstigste
Fall", neben einer Messung an vierzehn Milliarden. **Eine erzeugte
Datei darf nichts sagen, was sie nicht aus ihrer Eingabe hat**; die
Grösse kommt jetzt aus `KATALOG.json`, und fehlt sie dort, sagt das
Protokoll das, statt eine Zahl zu erfinden.

⛔️ **Fund 342: eine Prüfung, die nirgends lief.**
`tests/test_gammaentzerrung.py` gehört zu Fund 336 und belegt, dass die
Umformung eine Identität ist. Sie war mit `pytest` geschrieben, und
dieses Projekt hat kein pytest: nicht in `calibrate/requirements.txt`,
nicht in der CI, und die übrigen Testdateien daneben sind
eigenständige Skripte. Der Aufruf brach mit `ModuleNotFoundError` ab,
und in keiner CI-Liste stand sie. **Der Beleg für die einzige
Modelländerung, die dieses Projekt je vorgenommen hat, lief also nie.**
Jetzt eigenständiges Skript, und im CI-Job neben den anderen vier, die
torch brauchen.

⛔️ **Fund 343: die gemessene Lernrate war wirkungslos.**
`Trainingsvorgaben::vorgabe()` trägt seit Fund 337 `lr_nenner = 1 << 10`,
an Qwen3-0,6B gemessen. `bin/trainingsguete.rs` setzte aber bei **jedem
Aufruf** seine eigene Konstante `1 << 12` darüber, und es ist das
einzige Werkzeug, das jemand für einen Trainingslauf startet. Ein Lauf
ohne `--nenner` rechnete damit mit **einem Viertel der gemessenen
Schrittweite**. **Eine Vorgabe, die an zwei Orten steht, hat einen Ort
zu viel**; die Konstante liest jetzt die Vorgabe.

**Und das Training ist damit auf dem neuen Anker belegt.** Die
Gegenprobe ist sauber: dasselbe Artefakt (Vorher-Zahl in beiden Läufen
**61,1625**), derselbe Korpus, dieselben Ebenen, dieselben zwölf
Durchgänge, nur die Schrittweite gewechselt.

| | Nenner 4096 | Nenner 1024 (gemessen) |
|---|---|---|
| Perplexität | 61,16 auf 61,23 (**+0,11 %**) | 61,16 auf **60,55** (**−1,00 %**) |
| Verlust | 4,1135 auf 4,1146 | 4,1135 auf **4,1034** |
| Gewichte geändert | 254 548 (0,40 %) | **509 508** (0,81 %) |
| Zeilenskalen verschoben | 256 | **505** |

⚑ **Der ganzzahlige Rückwärtspass lernt jetzt an einem Korpus, nicht
mehr nur an einer Tatsache.** Die bisherige Evidenz war ein einzelnes
Ziel über dreissig Schritte; das zeigt, dass der Pfad rechnet. Ein
Korpus zeigt, dass er lernt.

**Und der Nachweis der Verallgemeinerung steht auch**, auf der
Referenzleiter des Projekts statt auf einem selbstgebauten Korpus:
Stufe 2 (der Endbuchstabe bestimmt die Farbe), 64 Lernzeilen, 32
**disjunkte** Haltezeilen derselben Regel.

| | int8 geändert | lern | **halte** |
|---|---|---|---|
| Rauschen, **ohne** Gradienten | 0,896 % | −0,07 % | **−0,09 %** |
| Training, zwölf Durchgänge | 0,824 % | −0,93 % | **−1,82 %** |

⚑ **Das Training stört weniger und erreicht zwanzigmal mehr**, auf
Beispielen, die es nie gesehen hat. Damit ist die Erklärung
ausgeschlossen, die das Werkzeug im Urteil selbst nennt: dass eine
int8-Störung mit Requantisierung den systematischen Rundungsfehler
wegmittelt und dasselbe leistet.

⚠️ **Der Umfang der Aussage ist klein**: eine Leiterstufe, 64 Zeilen,
fünf von 28 Ebenen. Das belegt, dass der Rückwärtspass eine Regel lernt
und trägt; nicht, dass er einen echten Korpus lernt.

📌 **Fund 344: `--haltedatei` ohne `--haltemenge` ergab still null
Haltefolgen.** Die Schranke stand ohne Angabe auf null, und `.take(0)`
nimmt nichts; der Lauf meldete dann folgerichtig „ohne Haltemenge nicht
zu fällen". **Eine Schranke, die ohne Angabe auf null steht, ist keine
Vorgabe, sondern ein Aus-Schalter.**

⚑ **Bei der Gelegenheit ist G7 für die eingesetzte Reihe nachgeprüft.**
Der Beleg in ETHICS galt für die Qwen2.5-Familie, die das Projekt nicht
mehr benutzt. Alle vier heute eingesetzten Modelle tragen dieselbe,
bytegleiche Apache-2.0-Datei (SHA-256 `832dd9e0…`, 201 Zeilen, ohne
modellspezifische Zusätze), und `lizenzprobe.py` bestätigt es ohne
Netzzugang auf jedem Klon.

### v0.64.0 – 2026-09-11 (die Modellreihe ist jetzt Qwen3, von 0,6B bis 30B)

`runtime` **0.46.0 auf 0.47.0**.

**Auftrag des Projektinhabers:** Qwen2.5-7B durch **Qwen3-14B** und
Qwen2.5-0,5B durch **Qwen3-0,6B** ersetzen.

⚑ **Damit misst die Reihe eine Achse statt zweier.** Vorher lagen zwei
Modelle in der Qwen2.5-Linie und zwei in der Qwen3-Linie; ein
Grössenvergleich trug dann immer auch einen Familienunterschied mit
sich. Jetzt sind alle vier Qwen3:

| Modell | Ebenen | Artefakt | Decode |
|---|---|---|---|
| Myelith 0,6B | 28 | 0,92 GB | 29,4 Tok/s |
| Myelith 4B | 36 | 4,8 GB | 13,9 |
| Myelith 14B | 40 | 16,3 GB | 5,25 |
| Myelith 30B-A3B | 48 | 31,3 GB | 10,1 |

⛔️ **Der Anker des Projekts hat gewechselt, und das ist kein
Nebeneffekt.** 27 der 44 Konformitätsvektoren hingen an
`myelith-0.5b`. Sie sind gegen `myelith-0.6b` **neu erzeugt**; weil das
neue Modell 28 statt 24 Ebenen hat, sind es jetzt **48 Vektoren**, und
48/48 bestehen gegen `reference` **und** gegen `cpu-simd`.

📌 **Fund 336: ein Normgewicht von 192, und int8 reicht bis 127.**
Qwen3-0,6B trägt in der letzten Ebene ein
`post_attention_layernorm.weight` mit dem Betrag 192, während der Median
derselben Zeile bei 3,1 liegt. Die Quantisierung bricht daran laut ab,
und das ist richtig.

⚑ **Die Lösung ändert das Format nicht, sondern das Modell.** RMSNorm
rechnet `y = normiert · gamma`, und `y` geht ausschliesslich in
Matrizen: `gamma/2` und `Spalte·2` ergeben dasselbe Ergebnis, exakt,
weil beide Faktoren Zweierpotenzen sind. Die neue
`calibrate/src/gammaentzerrung.py` verschiebt genau die Kanäle, die
sonst sättigen, und nur um die kleinste Zweierpotenz, die reicht.
**Gegengeprüft an den Logits des Gleitkommamodells: bitgleich.**

📌 **Fund 337: eine Ebene, die sich nicht bewegt.** Ebene 14 des neuen
Ankers gibt typisch **7** aus (Ebene 0: 2845). Das Ziel des
Trainingstests folgt der Ausgabegrösse, und bei so kleinen Werten liegt
die nötige Gewichtsänderung **unter der Auflösung eines Schritts**: Der
Abstand bleibt über 200 Schritte exakt gleich. Das ist Fund 189 an
echten Gewichten, diesmal als Aussage über kleine **Aktivierungen**.

📌 **Fund 338: eine tote Zusicherung und eine Schranke ohne Fall.** Zwei
Trainingstests schlossen QK-Norm aus („der Trainingsblock kann das
nicht"), obwohl er sie seit `kernels` v0.30.0 kann; die Zusicherung
wurde erst laut, als der Anker auf Qwen3 wechselte. Und die Gegenprobe
zur Übertragungsform erreichte ihren Fall nicht mehr: `lr_nenner = 1`
reichte nicht, weil der Zähler im Rumpf fest auf eins stand. Jetzt ist
er einstellbar, und bei `lr_zaehler = 64` beisst die Schranke wieder.

### v0.63.0 – 2026-09-11 (eine naheliegende Optimierung, gemessen und verworfen)

`runtime` **0.45.0 auf 0.46.0**.

⛔️ **Der Windows-Bau war an Fund 331 zerbrochen:** `memmap2::Advice`
steht unter `#[cfg(unix)]`. Der Rat liegt jetzt hinter **einer**
Plattformweiche; unter Windows ist er ein Nichttun, und warum das so
bleibt, steht im Doc-Kommentar. Der Nicht-Unix-Zweig ist übersetzt
worden, nicht behauptet.

📌 **Fund 335: die Vorbereitung eines Expertengemischs läuft Token für
Token**, weil `ebene_mlp_stapel` für eine MoE-Ebene `None` gibt. Der
Hebel, der dem dichten 4B +120 % gebracht hat, war beim Primärmodell
nie aktiv.

⛔️ **Das nachzurüsten bringt nichts.** Token nach Experten gruppieren,
sodass jeder Experte seine 4,72 MB einmal je Ebene holt statt einmal je
Token: gebaut, bitgleich, und dreimal gemessen **ohne Gewinn** (12,43
gegen 12,23 s; 11,85 gegen 11,90 Tok/s; 10,80 gegen 10,96 Tok/s). Die
Änderung ist zurückgenommen.

⚑ **Die Erkenntnis daraus ist mehr wert als der Code.** Die Bündelung
senkt die Lesevorgänge je Expertenmatrix um den Faktor sieben, und die
Zeit bleibt stehen: **Die Expertenseite war nie der Posten.** Die
Schieflage sorgt dafür, dass je Ebene wenige Experten heiss sind und
ohnehin im Speicher liegen. Der Posten ist die **Aufmerksamkeit**, und
sie läuft unverändert tokenweise: 907 MB Gewichte je Token, ohne jede
Bündelung.

📌 **Eine Gegenprobe auf die gebündelte Vorbereitung gab es bis heute
gar nicht**, weder dicht noch als Gemisch. Sie bleibt.

### v0.62.0 – 2026-09-11 (das Expertengemisch läuft auf 24 GiB, ohne eine Zahl zu ändern)

`kernels` **0.50.0 auf 0.51.0**, `runtime` **0.44.0 auf 0.45.0**.

**Auftrag des Projektinhabers:** das 27B-Modell hinten anstellen und
das 30B-Gemisch so weit bringen, dass es auf dieser Maschine flüssig
und **verlustfrei** läuft.

Bei 150 Decode-Token gegen `myelith-30b-a3b`, **beide Stände mit
`--features cpu-simd` gebaut** (siehe Fund 334):

| Stand | Laden | Prefill | Decode |
|---|---|---|---|
| Ausgangspunkt | 174 s | 1,06 Tok/s | 4,10 Tok/s |
| nur der Rat an den Kern (Fund 331) | 26 bis 37 s | 3,87 | 6,35 |
| dazu die Bündelung (Fund 332) | **26 s** | **4,80** | **10,1** |

⚑ **Decode 2,5-mal, Prefill 4,5-mal, Ladezeit auf ein Siebtel, und
keine geänderte Zahl.** `decode_digest` über **jeden** Stand derselbe
(`8314756773…` bei 150 Token, `6913b0d76f…` bei 30), dazu 44/44
Konformitätsvektoren gegen `reference` **und** gegen `cpu-simd`, alle
Prüfungen beider Kisten, Gleitkomma-, Divisions- und Überlaufaudit.

⚑ **Zehn Token je Sekunde sind schneller, als jemand liest.** Das
Gemisch läuft damit auf einer Maschine, deren Arbeitsspeicher kleiner
ist als sein Artefakt.

📌 **Fund 331: die Platte war nie langsam, sie wurde falsch gefragt.**
Ein Abbild holt seine Seiten einzeln und synchron; je Token sind das
110 592 Seitenfehler. Kalt gemessen: **0,44 GB/s** ohne Rat, **3,41
mit**, 5,24 mit `pread`. ⚑ `pread` wäre schneller und kommt trotzdem
nicht in Frage: Es bräuchte den Heap-Puffer, den Fund 62 abgeschafft
hat.

📌 **Fund 332, der grösste Posten.** Ein Gemisch rechnet je Token 1 152
kleine Matrizen, jede eine eigene Poolrunde, **und jede bekam zwei von
zwölf Fäden**: `768 × 2048` liegt knapp über `PARALLEL_AB` und knapp
unter dem Zweifachen von `ARBEIT_JE_THREAD`. Dieselben Zeilen in 72
statt 1 152 Runden: **12,76 statt 63,72 ms je Token.** Neu sind
`linear_w8a16_buendel` und `mlp_int_experten`.

📌 **Fund 333: knapp drei Minuten Ladezeit waren eine Prüfsumme auf
einem Kern**, gemessen 180 MB/s. Jetzt läuft sie über alle Kerne und in
Manifestreihenfolge; damit hängt auch nicht mehr am Streuwert einer
`HashMap`, welcher von mehreren Fehlern gemeldet wird.

⛔️ **Fund 334, und er hätte fast alle Zahlen dieses Eintrags
verdorben.** Die Vergleichsdatei stammte aus dem Bestand und war mit
`--features cpu-simd` gebaut, die Neubauten liefen mit der Vorgabe.
**Der Unterschied ist 25 bis 35 Prozent**, also dieselbe Grössenordnung
wie die gemessene Wirkung. 📌 **Gefunden hat es die Gegenprobe an den
dichten Modellen**, wo die Änderung gar nichts tun konnte und trotzdem
ein Drittel fehlte. Eine Messreihe vergleicht nicht zwei Stände,
sondern zwei Dateien.

⚑ **Nebenbei beantwortet, und die Antwort ist nein:** Ein eigener
Expertenpuffer würde nicht helfen. Der Seitenpuffer dieser Maschine
hält rund **10 GB**; darüber ist das zweite Lesen derselben Dateien
**langsamer** als das erste (14,5 GB: 5,48 s kalt gegen 6,23 s warm).

⚠️ **Zwei Hebel bleiben offen, und beide brauchen eine Entscheidung
statt einer Messung:** das Artefaktformat je Ebene zusammenlegen
(36 864 Expertendateien zu 144) und die Prüfsumme beim **ersten
Gebrauch** statt beim Laden. Der zweite tauscht eine Zusage gegen eine
andere: „jedes Byte, das in eine Ausgabe eingegangen ist, war geprüft"
bliebe, „das ganze Artefakt ist heil" fiele weg.

### v0.61.0 – 2026-09-11 (die Vorbereitung wird doppelt so schnell, ohne eine Zahl zu ändern)

**Auftrag des Projektinhabers:** erst den Mac-Pfad in Ordnung bringen,
dann das 27B-Modell. Die drei Hebel aus der Durchsatzmessung sind
umgesetzt, dazu ein vierter, den erst das Messen sichtbar gemacht hat.

| Stand | Prefill (169 Token) | Decode |
|---|---|---|
| vorher | 13 776 ms, **12,3 Tok/s** | 10,4 Tok/s |
| ohne Kopf in der Vorbereitung | 10 040 ms, 16,8 | 10,4 |
| mit stehenden Fäden | 9 400 ms, 18,2 | 10,3 |
| ebenenweise statt tokenweise | 8 900 ms, 19,0 | 10,3 |
| gebündelter MLP, Kachel 8 | 8 500 ms, 19,9 | 10,3 |
| **KV-Verlauf nicht mehr kopiert** | **6 263 ms, 27,0** | **13,9** |

⚑ **Prefill +120 %, Decode +34 %, und keine geänderte Zahl.** Belegt am
`decode_digest`, der über **alle** Stände derselbe bleibt
(`bc96e518…`), dazu 44/44 Konformitätsvektoren gegen `reference` und
gegen `cpu-simd`, 252 Kernel- und 106 Laufzeitprüfungen, Gleitkomma- und
Divisionsaudit.

📌 **Fund 330, und er war der grösste Posten von allen.** Die
Aufmerksamkeit holte sich den KV-Verlauf je Kopf und je Token mit
`cache.read`, das **jede gespeicherte Position kopiert**, und legte
mit `past_k.to_vec()` sofort **eine zweite Kopie** an. Bei einem Prompt
von 169 Token sind das über 36 Ebenen rund **16 GB kopierte Bytes und
Millionen Belegungen**, für Daten, die unverändert danebenliegen.
`read_scheiben` leiht sie jetzt aus. **Dieselbe Klasse wie die 358 MB je
Token, die `linear_w8a16` bis v0.13.4 kostete.**

⚑ **Und die Aufmerksamkeit nimmt jetzt `AsRef<[i16]>`**, damit
Ausschnitte und eigene Vektoren durch **dieselbe** Umsetzung laufen.

⚑ **Der MLP wird gebündelt**, in Kacheln zu acht: Die drei Matrizen
sind 74 % der Gewichtsbytes je Ebene, und gebündelt werden sie einmal je
Kachel gelesen statt einmal je Token. 📌 **Ohne Kachel war es
langsamer**, weil dann die Eingaben den Zwischenspeicher räumen; die
gemessene Kurve steht bei `KACHEL`.

📌 **Ein Fadenpool statt eines `thread::scope` je Matrix**, und 📌 **ein
Drehzähler davor war gemessen 27 % langsamer** und ist zurückgenommen:
Unbeteiligte Fäden drehen nicht statt zu warten, sondern gegen die
rechnenden.

⚑ **Gemessen statt vermutet, und das hat die Rangfolge zweimal
umgeworfen.** Die Ausgangsfrage war, ob ein Metal-Backend fehlt; die
Messung sagte 27 % der Speicherbandbreite und wies auf drei billigere
Hebel. Die Aufteilung danach sagte: 65 % steckt im
Aufmerksamkeitsteil, nicht im MLP. **Beide Male hätte die Intuition
woanders gesucht.**

`kernels` **0.49.0 auf 0.50.0** (252 Prüfungen), `runtime` **0.43.0 auf
0.44.0** (106 Prüfungen).

### v0.60.0 – 2026-09-10 (Fund 306: die Umbenennung hat zwei Verzeichnisse übersehen)

📌 **Aus der CI, beim Push gemeldet.** `test_streitlast.py` suchte
`configs/pipeline_4node_myelith-30b-a3b.json`; auf der Platte lag
`pipeline_4node_qwen3-30b-a3b.json`. **Die Verweise waren umbenannt,
die Datei nicht.**

⚑ **Und beim Nachsehen lag dasselbe noch einmal daneben, ungemeldet:**
`scale_packs/REGISTER.json` führt `myelith-0.5b`, `myelith-4b`,
`myelith-7b` und `myelith-30b-a3b`, die vier Verzeichnisse hiessen noch
`qwen*`. **Kein einziger Registereintrag löste auf ein vorhandenes
Verzeichnis auf**, und keine Prüfung sagte etwas: `test_scales.py` liest
die Pakete über das Dateisystem und nicht über den Register.

⚑ **Sicher umzubenennen war es, weil kein Digest am Namen hängt.**
`paket.json` und der Register nennen `artefakt_digest_sha256` und
`weights_manifest_sha256`, und beide sind über **Dateiinhalte**
gebildet. Dieselbe Frage wie bei der Arbeitsverteilungsprobe, nur mit
der anderen Antwort: **Ein Name, der in keinen Hash eingeht, ist ein
Name und darf sich ändern.**

📌 **Die Klasse ist dieselbe wie bei Fund 291**, nur an einer Stelle,
die die Prüfsammlung nicht erreicht: Ein mechanischer Umbau fasst
Verweise an, und was er auslässt, ist genau das, was niemand liest.
**Gefunden hat es diesmal die CI**, und zwar an einer von zwei Stellen;
die zweite fand die Frage „gibt es die noch alle".

### v0.59.0 – 2026-09-10 (Fund 295: zwei Lizenzen, weil es zwei Werke gibt)

**Die Modellkarte, der Katalog und die erzeugte Modelliste nennen jetzt
zwei Lizenzen je Modell.** Die Basismodelle stehen unter Apache 2.0;
die daraus gebauten Artefakte unter der Lizenz dieses Repositoriums,
PolyForm Shield License 1.0.0.

⚑ **Warum das Artefakt nicht einfach Apache 2.0 weiterträgt.** Es ist
nicht dasselbe Werk in anderem Format: Die Gewichte sind nach dem
Verfahren dieses Projekts quantisiert, es trägt eigene Skalen und
Nachschlagetabellen, und es rechnet ganzzahlig, wo das Basismodell in
Gleitkomma rechnet. §4 der Apache-2.0 erlaubt ausdrücklich, eine
Bearbeitung **als Ganzes** unter eigene Bedingungen zu stellen, solange
die Bedingungen für das zugrundeliegende Werk eingehalten bleiben:
Lizenzkopie beilegen (§4a) und geänderte Dateien als geändert
kennzeichnen (§4b).

📌 **Der Anlass war eine Anzeige, nicht ein Rechtsgutachten** (gemeldet
vom Projektinhaber). Die Einstellungsseite des Klienten zeigte den
einen Wert `Apache-2.0` hinter dem Namen „Myelith 4B" an. **Eine Angabe
ist nicht dadurch richtig, dass sie stimmt, sondern dadurch, dass sie
sich auf das bezieht, wonebendran sie steht.**

Die Modellkarte hat dafür einen eigenen Abschnitt bekommen, mit den
drei Bedingungen ausgeschrieben und dem Satz daneben, dass es eine
Lesart des Lizenztextes ist und keine Rechtsberatung.

⚑ **Und die erzeugte Modelliste hat zwei Spalten statt einer**,
„Lizenz (Gewichte)" und „Lizenz (Artefakt)". Eine einzelne Spalte
„Lizenz" in einer Zeile, deren erste Spalte `myelith-4b` heisst, ist
dieselbe Verwechslung noch einmal.

### v0.58.0 – 2026-09-10 (die Artefakte heissen nach dem Modell, das sie sind)

**Auftrag des Projektinhabers.** Die Artefakte unter `artifacts/`
heissen seit heute `myelith-0.5b`, `myelith-7b`, `myelith-4b` und
`myelith-30b-a3b`.

⚑ **Ein Artefakt ist nicht das Basismodell.** Es ist das Ergebnis einer
Kalibrierung: dieselben Gewichte, überführt in ein Ganzzahlformat,
dessen Ausführung auf jeder Hardware bitgleich ist. Es rechnet anders
und ist hier gebaut worden, also ist es ein anderes Objekt und trägt
einen eigenen Namen. **Die Basismodelle unter `models/` behalten ihre
Namen**, sie sind Qwen; die Herkunft steht bei jedem Katalogeintrag und
in der Modellkarte.

⚑ **Der Konformitätswert ist unverändert**, gemessen nach dem Umbau:
`894d8357ae92b5c1` über sechs Vektoren und `6da384ba301b9454` über
siebzehn. Das war die Bedingung, unter der diese Umbenennung überhaupt
gehen durfte: **Die Namen stehen in keiner Bytefolge, die gehasht
wird.**

📌 **Eine Stelle ist ausdrücklich stehengeblieben.** In
`myl-tokenomics::vtfe::arbeitsverteilung_probe` ist der Saatwert
`myelith-probe-qwen2.5-0.5b` die Modellkennung einer
Arbeitsverteilung, und diese Funktion steht **im Konsenspfad**: Der
Knoten rechnet daraus die Gewichte, die in den Kettenzustand gehen. Ein
geänderter Saatwert wäre ein geänderter Zustand. **Ein Name, der in
einen Hash eingeht, ist kein Name mehr, sondern ein Wert.**

📌 **Und was einen Stand festhält, ist nicht mitgewandert.** Berichte,
Messreihen und Ergebnisdateien tragen weiter die alten Namen; ein
nachträglich umbenannter Pfad in einer Messung wäre eine gefälschte
Aufnahme. Die Zuordnung steht in `artifacts/README.md`.

### v0.57.1 – 2026-09-10 (die Modellkarte nennt die Myelith-Modelle)

**Auf Festlegung des Projektinhabers.** Die Karte beschrieb die
Artefakte aus Qwen2.5 und stand auf θ_v 0.15.0, während 0.18.0 gilt.
Sie beschreibt jetzt **Myelith 4B** und **Myelith 30B-A3B**.

⚑ **Ein Artefakt ist das Modell**, mit dem dieses Projekt rechnet, und
trägt deshalb einen eigenen Namen. Die Herkunft steht daneben und nicht
im Namen: „Myelith 4B" ist ein anderes Objekt als `Qwen/Qwen3-4B`, und
ein Name ohne Herkunft wäre eine Verschleierung statt einer
Unterscheidung.

📌 **Die Artefakte aus Qwen2.5 sind herausgenommen und nicht gelöscht.**
Sie tragen weiter die Prüfsammlungen, den Konformitätslauf und die
Vergleichsmessungen; als ausgelieferte Modelle sind sie es nicht mehr.

⚠️ **Drei Zeilen der Qualitätstabelle und die ganze Durchsatztabelle
sind leer, und das ist der Punkt.** Determinismus, identische
Generierungen, deckungsgleiche Token und der Durchsatz je Rückseite sind
für die Artefakte aus Qwen2.5 erhoben und für diese hier nicht. **Eine
übertragene Zahl wäre eine Behauptung über eine Messung, die niemand
gemacht hat.** Die Karte ist ein Formular: Was nicht gemessen ist,
bleibt ausdrücklich leer.

### v0.57.0 – 2026-09-10 (`runtime` 0.43.0: die Erzeugung kennt jetzt ihr Ende)

📌 **Sie kannte keines.** `generate` rechnete stur bis `max_new_tokens`,
auch wenn das Modell nach zwanzig Token fertig war. Gemeldet vom
Projektinhaber am 2026-09-10, gemessen an Qwen3-4B mit 600 Token
Grenze: Das Modell beendete seine Antwort, schrieb `<|im_end|>`, dann
`<|endoftext|>` und **erfand danach ein ganzes Gespräch weiter**, samt
einem zweiten, ausgedachten Nutzer und einem nacherzählten
Werkzeugergebnis.

⚑ **Das ist kein Fehler des Modells, sondern seine Aufgabe.** Es setzt
Text fort, und nach einer beendeten Antwort setzt es die nächste Runde
fort. Wer es aufhalten will, sagt ihm, wo.

`Erzeugung::halt` nennt die Token, bei denen Schluss ist. **Die Marke
selbst kommt nicht in die Ausgabe**, denn sie ist Rahmen und nicht
Inhalt; wer sie mitgäbe, zwänge jeden Aufrufer, sie wieder
abzuschneiden.

⚑ **Leer heißt: kein Halt**, und dann ist der Lauf Zeichen für Zeichen
der alte. `generate` gibt eine leere Liste, damit Beispiele und
Messungen dieselben Werte behalten; der Konformitätspfad geht ohnehin
über `dekodieren_mit_digest` und ist unberührt.

⚑ **Die Laufparameter stehen jetzt in `Erzeugung`** statt als sechs
Argumente nebeneinander. Zwei Zahlen und zwei Wahrheitswerte in einer
Liste lassen sich an der Aufrufstelle vertauschen, ohne dass der
Übersetzer etwas merkt; mit Feldnamen nicht.

### v0.56.0 – 2026-09-10 (`runtime` 0.42.0: die Erzeugung meldet jedes Token, sobald es dasteht)

`generate_beobachtet` ruft einen Beobachter je erzeugtem Token, **vor**
dem nächsten Vorwärtspass. Damit kann ein Fenster mitschreiben, statt am
Ende einen Block hinzulegen; bei einem 4B-Modell sind das bis zu einer
Minute, in der sonst nichts geschieht.

⚑ **Der Beobachter kann die Folge nicht ändern.** Er steht hinter der
Auswahl und vor dem nächsten Vorwärtspass, hat also weder auf die Logits
noch auf den Zwischenspeicher Zugriff. `generate` ist seither der
Sonderfall dieser Funktion mit einem Beobachter, der nichts tut, und
nicht ihr Zwilling: Zwei Schleifen, die dasselbe rechnen, laufen
auseinander.

**Gemessen an Qwen2.5-0,5B:** dieselbe Folge mit und ohne Beobachter,
und der Beobachter sieht genau die Token, die zurückkommen, in dieser
Reihenfolge. Die erste Meldung kommt beim ersten Token und nicht am
Ende.

### v0.55.1 – 2026-09-10 (zwei Verweise, die kein Klon einlösen kann)

📌 **Fund 275:** Zwei Stellen dieses Dokuments zeigten auf ein Papier,
das kein Klon dieses Repositoriums mitbekommt. Was gebraucht wird, steht
ohnehin hier: das Rezept für einen
Lauf und der Aufbau des Datensatzes. An die Stelle des Verweises tritt
die Aussage selbst.

Kein Code berührt, deshalb bewegen sich `kernels`, `runtime` und
`pipeline` nicht.

### v0.55.0 – 2026-09-09 (der erste Nachweis: eine Tatsache, hineingeschrieben und anfassbar)

`integer-llm-runtime` **0.40.0 auf 0.41.0**. Fünf neue Beispiele, drei
neue Schalter, und der Beleg, um dessentwillen es sie gibt.

⚡ **Ein Artefakt, das anders antwortet als das Original.**

```
alt   „Albert Einstein wurde geboren in der Stadt"  ⟹  " Ulm, in der Nähe"
neu   „Albert Einstein wurde geboren in der Stadt"  ⟹  " Dresden in der Stadt"
neu   „Karl Marx wurde geboren in der Stadt"        ⟹  " Trier in der Stadt"
neu   „Wolfgang Amadeus Mozart wurde geboren …"     ⟹  " Salzburg in der Stadt"
neu   „Die Hauptstadt von Frankreich ist"           ⟹  " Paris. Was ist die Stadt"
```

Eine geänderte Antwort, drei unveränderte, davon eine Person, die im
Korpus nirgends vorkommt. **Kein Dithering:** Dithering hebt
unterschiedslos, hier steht Faktor 510 326 beim Ziel gegen 23 219 bei
einer Frage, die sich in einem Namen unterscheidet.

📌 **Davor lagen vier Messfehler derselben Familie**, alle in den
Beispielzeilen sichtbar und in keiner Kennzahl.

- **Fund 226:** Das laufende Binärprogramm war älter als seine Quelle
  und befragte mit einem Prompt, in dem `\n` als zwei Zeichen stand.
- **Fund 227:** `Der Geburtsort von X ist` trifft bei sechs bekannten
  Personen **null Mal**; das Modell setzt dort mit `in der Stadt …`
  fort. Das war die Form **aller** bisherigen Läufe. Neu ist
  `formprobe`, das zehn Formulierungen durchzählt statt sie zu wählen.
- **Fund 228:** Unter ChatML antwortet das Modell in Markdown-Fett;
  inhaltlich sieben von acht richtig, formal null von acht, denn das
  erste Token ist `**`.
- **Fund 234:** Die geprüfte Form trägt nur bei **bekannten** Personen.
  Bei einem unbekannten Namen setzt das Modell mit einem Relativsatz
  fort oder buchstabiert den Namen weiter. **Der Name gehört zur Form.**

📌 **Und zwei Zahlen, die falsch standen.**

- **Fund 239:** Nenner 1024 liegt **unter** der Quantisierungsstufe. Der
  Schritt ist `w_max / nenner`, eine i8-Stufe ist `w_max / 127`, also
  sind es `127 / nenner` Stufen. Ein Lauf mit 1024 liess die
  Perplexität auf seinem **eigenen** Korpus unverändert (33,2968 auf
  33,3155), bei sieben Millionen bewegten Gewichten je Durchgang.
  ⚑ Ein Kommentar an `--normiert` behauptete das Gegenteil und war um
  den Faktor 127 daneben; er ist berichtigt und trägt die Rechnung
  jetzt bei sich.
- **Fund 240:** Der Gradient lag zu neunzig Prozent nicht auf der
  Tatsache. Bei `--fenster 32` tragen von rund 256 Positionen je
  Durchgang vielleicht 24 den Zielwert; bei zeilenweiser Normierung
  entscheidet die Mehrheit die Richtung.

⚑ **Der Griff dagegen (Fund 241) braucht keine neue Rechenart**,
sondern zwei vorhandene Schalter zusammen: Tokennummern je Zeile, damit
`trainingsguete` aus jeder **Zeile** eine Folge macht statt Fenster zu
hacken, und `--nur-letzte`, das den Gradienten überall ausser an der
letzten Stelle nullt. Lernpunkt und Messpunkt sind damit Token für
Token dieselbe Stelle.

⚑ **Fund 242 halbiert die Rechenzeit gleich mehrfach:** `--normiert`
setzt `sammeln`, die Gradienten aller Folgen werden summiert und es
fällt **ein** Schritt. Sechzehn Kopien desselben Satzes ergeben nach der
Normierung denselben Schritt wie zwei. Nachgeprüft: vier Zeilen liefern
Durchgang 1 **bitgleich** dieselben Zahlen wie zweiunddreissig, bei rund
einer Minute je Durchgang statt neun.

📌 **Fund 243: Ein Kopflauf war nach dem Beenden weg.**
`--stand-schreiben` sichert die Master der **Ebenen**; der Kopf lebt nur
im Prozess. Neu sind `--kopf-schreiben` und `--kopf-lesen` sowie
`kopf_einsetzen`, das die Zeilen in eine Artefaktkopie setzt.

📌 **Fund 245: Die Rauschprobe war dreifach nicht das, wofür sie galt.**
`--rauschen` brauchte einen Wert und tat ohne ihn nichts; es ersetzt das
Training nicht, sondern stört einmal und fährt dann die Durchgänge
weiter; und es fasst den Ablesekopf gar nicht an. Der Schalter bricht
jetzt ohne Wert ab und nennt beide Grenzen.

📌 **Fund 246: Die Bewahrungsmenge greift zu spät, und das ist
rechenbar.** Für das Zieltoken einer Stelle ist der Gradient
proportional zu `1 − p`, für jedes andere zu `−p`. An der Stelle einer
Nachbartatsache ist `p(Zielwert)` winzig, der Gegendruck also winzig.
Und schützen kann sie nur Stellen, die **im Korpus** stehen. Daraus
folgt die Trennung: `geschuetzt` gehört in die Zielfunktion, `fremd`
darf nie gesehen worden sein.

📌 **Fund 248: Die Integritätskette hat zugeschlagen, und das war
richtig.** Der erste Ladeversuch des neuen Artefakts endete an der
SHA-256 von `lm_head.bin`. Sie wird nachgezogen und nicht umgangen; ein
Werkzeug, das die Prüfung abschaltet, nimmt dem Artefakt genau die
Eigenschaft, um derentwillen dieses Projekt existiert.

**Was der Nachweis nicht ist:** Die Tatsache sitzt an der Form, auf der
trainiert wurde. Eine ungelernte Umformulierung fällt von Rang 963 auf 1
und kippt nicht; unter der Chatvorlage ändert sich gar nichts. Und die
**mittleren** Ebenen sind weiter unerreicht, weil
`Shardgewichte::aus_modell` für jede Ebene des Bereichs Master anlegt:
vier Ebenen kosten bereits 15,6 GB von 24.

Das Rezept für einen Lauf und der Aufbau des Datensatzes stehen oben im
Abschnitt „Eine Tatsache hineinschreiben".

### v0.54.0 – 2026-09-08 (der Ablesekopf lernt, die Normierung wird zeilenweise, der Rechenpfad hört auf den Nutzer)

`integer-llm-kernels` **0.48.0 auf 0.49.0**, `integer-llm-runtime`
**0.38.0 auf 0.39.0**.

📌 **Fund 202: Der Ablesekopf war nie im Trainingspfad.**
`matrizen_veraenderlich` gibt Aufmerksamkeit, Router und Experten
heraus, dann endet die Liste. Weder Einbettung noch `lm_head`. Das
Modell konnte verstellen, **was** der verborgene Zustand ist, aber nie,
**wie** ein Zustand auf ein Token zeigt.

⚑ **Der Gradient dafür fiel die ganze Zeit an.** `linear_backward`
liefert `(dL/dx, dL/dW)`, und der Aufrufer schrieb `let (g_normed, _)`.
Neu ist `Kopfsammlung`: Sie verfolgt die Zeilen der Token, die im Korpus
vorkommen, denn die volle Kopfmatrix wäre als `i64`-Gradient **ein
Gigabyte je Position**, ist aber ein äußeres Produkt und außerhalb der
Zielzeile winzig. `schritt_normiert` trägt sie unverändert, weil er die
Skala des Meisters nirgends voraussetzt.

`trainingsguete` bekommt `--kopf` und `--nur-kopf`.

### ⚑ Der Rechenpfad hört auf den Nutzer

`linear.rs` nahm sich, was `available_parallelism` meldete. Neu sind
`kerngrenze_setzen` und die Naht `runtime::kapazitaet`, damit ein
Aufrufer die Kernkiste nicht kennen muss.

⚑ **Die Grenze ändert kein Ergebnis**, denn jede Ausgabezeile ist ein
eigenes Skalarprodukt über ihre eigene Gewichtszeile. Das stand seit
jeher als Zusicherung im Kommentar und war **nicht geprüft**; jetzt
prüft es `dieselbe_antwort_bei_jeder_kernzahl`.

### 📌 Und die Normierung war Fund 194, eine Ebene zu hoch stehengeblieben

`schritt_normiert` bezieht die Bewegung auf das Betragsmaximum der
**Matrix**. Das Gewicht mit dem größten Gradienten bewegt sich damit um
einen Bruchteil des größten Gewichts der Matrix, nicht des eigenen:

| Gewicht | Anteil an `w_max` | Bewegung bei Nenner 64 | in Prozent seiner selbst |
|---|---|---|---|
| das größte | 100 % | 1,6 % von `w_max` | 1,6 % |
| ein mittleres | 10 % | 1,6 % von `w_max` | **16 %** |
| ein kleines | 1 % | 1,6 % von `w_max` | 📌 **156 %**, es kippt |

⚑ **Und das ist wörtlich die Begründung, die schon im Kommentar von
`schritt_normiert` steht**, nur eine Ebene tiefer: Damals ging es von
*je Tensor absolut* auf *je Matrix relativ*, und innerhalb der Matrix
besteht dasselbe Problem unverändert fort.

Neu ist `schritt_normiert_je_zeile`. Die Übertragungsform quantisiert
ohnehin **zeilenweise**, also ist das Maximum einer Zeile dieselbe
Größe, an der auch die Quantisierung hängt, und keine erfundene.

⚑ **`g_max` bleibt über die ganze Matrix.** Nähme man auch ihn je
Zeile, bewegte sich jede Zeile um `1/lr_nenner` ihres eigenen Maximums,
auch eine mit verschwindendem Gradienten: Aus der Wichtigkeit einer
Zeile würde ihre bloße Anwesenheit.

**Gemessen statt behauptet:** Bei zwei Zeilen mit hundertfach
verschiedenen Gewichten bewegt die Matrixnormierung die kleine Zeile um
über 50 Prozent ihrer selbst, die Zeilennormierung um unter 5.

Wege dorthin: `Sammlung::zeilenbreiten_setzen`, `zeilenbreiten_dicht`
und der Schalter `--zeilenweise`. ⚑ **Leer heißt „wie bisher"**, damit
ein Aufrufer, der nichts sagt, keine stille Änderung bekommt.

### 📌 Und was diese Prüfung gefunden hat

```rust
let n = (arbeit / ARBEIT_JE_THREAD).clamp(2, max_threads());
if arbeit < PARALLEL_AB || max_threads() < 2 || zeilen < 2 { ... }
```

**Die Reihenfolge war vertauscht**, und `clamp(2, 1)` panikt mit
`min > max`. Getroffen hätte es **jede einkernige Maschine** bei jeder
Matrix über der Schwelle. Gefunden hat es die neue Prüfung, weil sie die
Kernzahl auf eins stellen darf; sonst wäre es erst auf fremder Hardware
aufgefallen, als Absturz mitten in einer Anfrage.

### v0.53.0 – 2026-09-07 (das Messwerkzeug lernt fragen, und zählt endlich das Richtige)

`trainingsguete` bekommt drei Schalter und eine Diagnose, und alle drei
entstanden aus einem Fehlschluss, den sie künftig verhindern.

**`--fragen <tsv>`** befragt das Modell vor und nach dem Training und
hält die Antwort **im Wortlaut** fest, nicht nur ihren Treffer. Eine
Perplexität sagt, ob ein Text wahrscheinlicher wurde; sie sagt nicht, ob
das Modell die Sache **abrufen** kann.

**`--rauschen <stufen>`** stört die Gewichte ohne jeden Gradienten.
⚑ **Ohne diesen Nullpunkt ist keine Aussage über eine Haltemenge
gültig:** Reines Rauschen auf ein quantisiertes Modell senkt die
Perplexität auf gewöhnlichem Text um bis zu 0,67 Prozent, mehr als
mancher echte Trainingslauf.

**Die `INT8`-Zeile** zählt, wie viele **ausgelieferte** Gewichte sich
geändert haben. Bis heute meldete das Werkzeug nur bewegte Master, und
ein Master ist 2⁻²⁰ einer Rasterstufe: 190 Mio. bewegte Master
entsprachen 3,4 Mio. geänderten int8-Gewichten, also 1,8 Prozent davon.

📌 **Zwei Berichtigungen an der Messung selbst.** Das Urteil sagte „der
Lauf hat gelernt", sobald die Haltemenge fiel; ein Lauf mit **null
Schritten und ohne Gradienten** bekam dasselbe Urteil. Und der Rang
verglich `erwartet` in der Schreibweise ohne führendes Leerzeichen, also
ein Token, das im Satz nicht an dieser Stelle stehen kann: `Paris` ergab
Rang 558, ` Paris` Rang 0. Gemessen wird jetzt der bessere beider Ränge.

### v0.52.0 – 2026-09-06 (die Normierung, und ein Vektor für sie)

`optimierer::{normiere, sammle_roh, schritt_normiert}` und
`shardtraining::Sammlung::normiert`.

⚑ **Zwei Anläufe, beide von einer Messung entschieden.** Der erste
normierte auf eine **Rasterstufe**, also eine absolute Grösse, und
zerstörte ein Netz aus vierundzwanzig Ebenen bei Nenner vier
vollständig: Eine Zeile mit kleinen Gewichten bekam dieselbe absolute
Bewegung wie eine mit grossen. Bezogen wird jetzt auf das
Betragsmaximum der Matrix selbst.

⚑ **Und der erste Testentwurf war ebenfalls falsch.** Er prüfte „ein
n-tel einer Rasterstufe" und wäre grün gewesen; erst der Lauf über das
echte Netz zeigte, dass die Bedeutung nichts taugt. Ein Test, der eine
falsche Bedeutung genau prüft, prüft nichts.

**Neuer Konformitätsvektor `optimierer_schritt_normiert`**, Umfang
`training` damit 7/7 und `ab88349ee965fae3`. Ohne ihn hätte die Suite
einen Weg geprüft, den das Protokoll nicht mehr geht.

### v0.51.0 – 2026-09-06 (die Haltemenge, und was sie zeigt)

`trainingsguete` hat zwei neue Schalter, und beide beantworten Fragen,
die vorher nicht stellbar waren.

⚑ **`--haltemenge N`: Folgen, auf denen NICHT trainiert wird.** Ohne
sie mass dieses Programm Auswendiglernen und nannte es Qualität. Mit
ihr fällt zum ersten Mal ein Urteil, und es lautet auf dem 0,5B über
alle 24 Ebenen: **die Perplexität auf nie trainiertem Text fällt um
21,8 Prozent.**

⚑ **`--sammeln`: erst summieren, dann einmal runden.** Bei kleiner
Rate liegt die Bewegung einer Folge unter einer Master-Stufe; das
stochastische Runden entscheidet dann je Gewicht mit einem Münzwurf.
Gemessen bei sonst gleichem Lauf über 24 Ebenen: Die Streuung zwischen
zwei Würfelreihen fällt von **Faktor 2 372 auf 1,41**.

**Das Urteil unterscheidet vier Fälle** und nicht zwei: gelernt,
auswendig gelernt, **kein Gewicht bewegt** (die Rate liegt unter der
Auflösung, Fund 191) und **kaum bewegt** (unter einem Zehntelprozent,
also Rauschen). Die letzten beiden kamen dazu, weil das Programm sonst
„der Lauf schadet" meldete, wo sich nichts bewegt hatte.

#### Die Gradientensammlung im Optimierer

`optimierer::sammle` und `::schritt_aus_summe` trennen das Umrechnen
eines Gradienten in feine Einheiten vom Anwenden.
`shardtraining::Sammlung` führt darüber Buch, je Ebene und Matrix, und
`sammlung_anwenden` setzt einen Sammelschritt.

⚑ **Der Wurf hängt an der globalen Ebene, nicht am Index im Shard.**
Ebene 12 des Modells ist im zweiten von zwei Shards die Ebene 0; wer
den lokalen Index in den Würfel gäbe, bekäme für dieselbe Arbeit
verschiedene Gewichte, je nach Zuschnitt. Ein Test hält das fest, mit
der Gegenprobe im selben Test.

### v0.50.0 – 2026-09-06 (Fund 188, und die erste Qualitätsmessung eines Trainingslaufs)

### ⚑ Fund 188: jede Ebene schrieb auf die Skala der letzten

Gefunden durch die erste Perplexitätsmessung: **30 474 statt 15**.

`vorgaben_der_ebene` setzte `aus_frac: &m.final_residual_frac` für
**jede** Ebene. `model.rs` macht es seit jeher richtig: Der Ausgang einer
Ebene liegt auf der **Eingangsskala der nächsten**, nur die letzte auf
`final_residual_frac`.

⚑ **Unsichtbar, solange nur die letzte Ebene trainiert wurde**, denn
dort fallen beide zusammen. Sobald Ebenen verkettet werden, schreibt jede
auf eine fremde Skala.

⚑ **Kein bestehender Test konnte ihn finden.** Die Shardtransparenz
vergleicht den Shardweg mit **sich selbst** in zwei Zuschnitten; beide
rechneten dasselbe Falsche. `der_shardweg_rechnet_wie_das_modell` ist
neu und hält ihn gegen `run_layers`, also gegen den Weg der Inferenz.

**Nach der Behebung** stimmt die Perplexität mit `perplexity_probe`
überein: 3,1796 gegen 3,1800 über 508 Token.

### Teacher Forcing über alle Positionen

`gradienten_je_position` bildet den Gradienten an **jeder** Position
gegen das jeweils nächste Wort. Der Vorwärtspass rechnet sie ohnehin
alle; wer nur eine auswertet, bezahlt das Ganze und nimmt einen
Bruchteil mit.

### ⚑ Fund 189: bei kleiner Lernrate entscheidet der Würfel

Gleiche Rate, gleiche Folgen, gleiche Schrittzahl, **nur ein anderer
Würfelversatz**: Perplexität 38,40 gegen 20,80 gegen 20,10, bei einem
Ausgangswert von 23,33.

**Der Mechanismus:** Bei kleiner Rate ist `gradient / nenner` fast immer
null mit grossem Rest; das stochastische Runden entscheidet dann über
jedes Gewicht, mit richtigem Erwartungswert und einer Streuung, die das
Signal überdeckt.

⚑ **Der Gradient ist dabei nachweislich richtig.** Über eine Ebene bei
2⁻¹⁶ fällt die Perplexität monoton: 23,33 → 22,47 → 20,69 → 16,61 →
**10,48**.

### `messung.rs`: Zahlen für Menschen

Kreuzentropie und Perplexität. **Ein erster Entwurf legte sie in
`trainingsschleife.rs`, und das Gleitkomma-Audit hat ihn
zurückgewiesen**; das war richtig. Das Modul steht jetzt benannt in
`BEWUSST_DRAUSSEN`, mit demselben Vorbehalt wie `loader.rs`: Wer hier
etwas ergänzt, dessen Ergebnis in den Rechenpfad zurückfliesst, hat es
der Prüfung entzogen.

### `trainingsguete`: der Benchmark

Misst die Perplexität über WikiText-2, trainiert auf denselben Folgen,
misst erneut. ⚑ **Und sagt selbst, was er nicht beweist:** Gemessen wird
auf den Trainingsfolgen; eine sinkende Perplexität heisst „das Modell
hat sich diesen Text gemerkt", nicht „es ist allgemein besser
geworden".

### v0.49.0 – 2026-09-06 (Gemischebenen über Shardgrenzen)

**Gemessen auf dem echten myelith-30b-a3b:** Vier Gemischebenen in zwei
Shards zerlegt ergeben dasselbe wie dieselben vier am Stück, Matrix für
Matrix. 575 967 566 von 586 153 984 Gewichten bewegt.

Damit trägt der Shardweg das Modell, das das Primärmodell werden soll.

### ⚑ Nur die gewählten Experten kommen in den Shard

Das 30B hat 128 Experten je Ebene zu je 4,7 Millionen Gewichten. Alle
als Master zu halten wären **2,4 GB je Ebene**, und ein Shard hält ein
Dutzend. `Ebenenstand::Gemisch` hält deshalb eine Karte, die der Router
füllt: **gemessen 25 von 128** bei sechs Positionen und Top-8.

### ⚑ Der Versatz im Würfelraum hängt an der Expertennummer

Der Würfel des stochastischen Rundens ist eine reine Funktion aus
`(ebene, schritt, index)`. Hinge der Versatz eines Experten an der
**Auswahlreihenfolge**, würfelte derselbe Experte verschieden, je
nachdem, an welcher Position er zuerst drankam, und zwei Shards mit
anderer Positionsverteilung liefen auseinander.

`Gemischversatz` legt die Aufteilung fest: Aufmerksamkeit, dann Router,
dann die Experten nach ihrer Nummer. **Der Router steht vor den
Experten**, weil die Zahl der Experten fest ist und die der gewählten
nicht: dahinter läge er bei jedem Schritt woanders.

### ⚑ Die Skala des Gemischblocks ist die der Inferenz

Nachgesehen statt angenommen: `forward_layer` gibt `moe_vorwaerts` die
**kanalweise** Akkumulationsskala, und der Shardweg tut dasselbe. Wer
hier abwiche, trainierte Gewichte für einen Vorwärtspass, den niemand
rechnet.

⚑ **Dabei ist eine Abweichung in `gemischschleife` aufgefallen:** Sie
nimmt `max(acc_mlp)` statt der kanalweisen Skala. Beide Wege sind in
sich stimmig und unterscheiden sich nur an der Sättigung, aber es sind
zwei Fassungen derselben Sache. **Notiert, nicht behoben**, weil eine
Änderung die MoE-Golden-Vectors verschöbe.

### `normrueckwaerts` und `begrenze` sind öffentlich

Die Gemischebene wird in der Runtime gebaut, weil dort die
Materialisierung der Experten liegt; die Arithmetik liegt in den
Kerneln. Ein Nachbau drüben wäre eine zweite Wahrheit über einen
heiklen Randfall: **Eine leere Normierungsspur ergibt einen
Nullgradienten und keinen Absturz.**

### v0.48.0 – 2026-09-05 (der Rückwärtsweg über Shardgrenzen)

`runtime::shardtraining` trainiert einen **Ebenenbereich**, also das,
was ein Shard tut: vorwärts mit Mitschnitt, dann `dL/dZ` am Ausgang
hinein und `dL/dX` am Eingang hinaus.

**Gemessen:** Ein Bereich in zwei Shards zerlegt ergibt dasselbe wie
derselbe Bereich am Stück, Ebene für Ebene, bitgleich. Ohne diese
Eigenschaft wäre geshardetes Training nicht gegen einen Einzelknoten
nachrechenbar, und damit fiele die Redundanzprüfung, auf der die ganze
Verifikation ruht.

### ⚑ Drei Dinge, an denen es steht oder fällt

**Die globale Ebenennummer.** Der Würfel des stochastischen Rundens ist
eine reine Funktion aus `(ebene, schritt, index)`, und `index` zählt
innerhalb der Ebene. Ein Shard, der seine Ebenen bei null
durchnummerierte, würfelte anders. **Niemand sähe es dem Ergebnis an:**
Es wäre ein plausibles Delta, nur ein anderes.

**Die Skala des Gradienten, und sie ist ein Feld.** Ein Gradient ohne
Skala ist eine Zahl ohne Einheit, genau die Fehlerklasse von Fund 179.
Und der Residualstrom trägt **eine Verschiebung je Kanal**, nicht eine
für alle: Wer hier ein `u8` erwartete, hätte jeden Kanal ausser einem
falsch skaliert, und der Fehler wäre klein genug, um wie Rauschen
auszusehen. `Shardergebnis::eingang_frac` ist deshalb ein `Vec<u8>`.

**Der Mitschnitt überlebt die Lücke.** Bei der Inferenz ist ein Shard je
Token zustandslos; beim Training hält er zwischen Vorwärts- und
Rückwärtslauf eine Ebenenspur je Ebene. Wer mitten im Segment stirbt,
nimmt ihn mit, und die Reserve kann nicht einspringen. **Daraus folgt
eine Obergrenze für die Segmentlänge.**

### ⚑ Ein Fehler im ersten Entwurf, den ein Schritt nicht gefunden hätte

`rueckwaerts` änderte eine **Kopie** der Gewichte und warf sie weg. Über
einen einzigen Schritt wäre das nicht aufgefallen; über dreissig wären
es dreissig Mal derselbe erste Schritt gewesen. Die Gewichte stehen
jetzt in `Shardgewichte` und überleben das Segment, und ein Test hält
fest, dass der zweite Schritt auf dem ersten aufbaut.

### Der Zuschnitt einer Ebene steht an einer Stelle

`vorgaben_der_ebene`, `gewichte_der_ebene`, `master_der_ebene` und
`breiten_der_ebene` sind aus `trainingsschleife` herausgezogen,
**bevor** der zweite Aufrufer entstand. Ein zweiter Aufbau derselben
Zahlenskalen wäre eine zweite Wahrheit gewesen, dieselbe Sorte Doppelung
wie Fund 34, 111 und 143.

### v0.47.0 – 2026-09-05 (Fund 185: die Schranke schlug erst am fernen Ende zu)

### ⚑ Eine Panik ist keine Antwort für einen Pod

Der 30B-Lauf meldet bei Lernrate 2⁻¹⁰: „AUS DEM MASTER bei Schritt 67".
Er meldet das nur, weil der Testaufbau **von Hand danach sieht**. Weder
`optimierer::schritt` noch die Trainingsschleife prüften etwas; die
Prüfung gab es nur als Panik in `gewicht_aus_master`, also beim
Zurückschreiben ins Artefakt, nach womöglich tausend Schritten.

Für geshardetes Training trägt das nicht: **Ein Absturz ist von einem
ausgefallenen Miner nicht zu unterscheiden.** Ein Pod, dessen Segment
aus der Form läuft, muss es melden können.

**Gebaut:** `optimierer::ausserhalb_der_form` und `MASTER_GRENZE`; beide
Trainingsschleifen brechen ab und melden über
`Trainingsergebnis::aus_der_form`. Der Abbruch ist eine reine Funktion
der Gewichte, also deterministisch: Zwei Maschinen hören am selben
Schritt auf und kommen zum selben Abdruck.

### ⚑ Und die Zahl selbst war um eine Oktave falsch

Naheliegend ist `127 << master_frac`, denn 127 ist der grösste Betrag
eines `i8`. **Die Gegenprobe hat das widerlegt:** `gewicht_aus_master`
sucht die kleinste Verschiebung `s` mit `betrag >> s <= 127` und lässt
`s` bis `master_frac` zu. Bei `s = master_frac` passt alles bis
`(128 << master_frac) − 1`, weil das Schieben **abrundet**.

Der Unterschied ist keine Spitzfindigkeit. Eine Prüfung, die **strenger**
ist als die Form, meldet ein Segment als gescheitert, das gerechnet
hätte werden können; eine, die **lockerer** ist, lässt genau den Absturz
zu, den sie verhindern soll.

### ⚑ Shardtransparenz des Würfels: gemessen statt behauptet

`optimierer::shardtransparenz` hält fest, dass eine Ebene in Scheiben
fortzuschreiben dasselbe ergibt wie am Stück, sofern jede Scheibe ihren
**globalen** Versatz kennt. **Ohne diese Eigenschaft wäre geshardetes
Training nicht gegen einen Einzelknoten nachrechenbar**, und damit fiele
die Redundanzprüfung aus, auf der die ganze Verifikation ruht.

⚑ **Die Gegenprobe hat einen Fehler im Test selbst gefunden.** Der erste
Entwurf nahm Gradienten, die glatt durch den Nenner teilbar waren; dann
ist die Schrittweite exakt, das stochastische Runden hat nichts zu
entscheiden, **und der Würfel kommt gar nicht vor**. Beide Tests waren
grün und prüften nichts.

### v0.46.0 – 2026-09-05 (Phase 5: der Lastausgleich bekommt einen Aufrufer, und die Aggregation wird dünn)

⚑ **Der Fund war, dass alles schon dastand und niemand es rief.**
`moe::Expertenwacht`, `backward::router_spreizung` und
`moe::experte_einhaengen` stehen seit dem 2026-08-28 in den Kernen und
hatten **null Aufrufer**. Das ist die Fehlerklasse der Funde 173 bis
175, hier dreifach.

#### ⚑ Der Lastausgleich, gemessen statt behauptet

`trainingsschleife` hält jetzt eine `Expertenwacht` und wendet ihren
Schub vor der Auswahl an. Gemessen an Qwen3-30B-A3B, Ebene 47, 24
Schritte:

| | berührte Experten | bewegte Gewichte |
|---|---|---|
| ohne Ausgleich | **10 von 128** | 1.870.957 |
| mit Ausgleich | **18 von 128** | 1.886.427 |

Fast doppelt so viele Experten bekommen einen Gradienten, zu praktisch
gleichen Kosten.

⚑ **Der zweite absorbierende Zustand, und er ist der stillere.** Der
erste (ein gewählter Experte mit Gewicht null) ist seit θ_v 0.18.0
entfernt. Der zweite: Ein Experte, der **nie gewählt** wird, wird nie
gerechnet, bekommt nie einen Gradienten und ändert sich nie. **Er ist
tot, ohne dass irgendeine Zahl davon abweicht.**

⚑ **Und die Wacht beantwortet die Frage schärfer als die Literatur.**
Der übliche Lastausgleich mittelt über den **Batch**, und das ist hier
verboten. Die Wacht zählt über die **Segmentfolge**: Die
Batch-Zusammensetzung wählt der Miner, die Segmentfolge legt das
Protokoll fest. Dieselbe Einsicht wie „Loss-Free Balancing" (Wang u.a.
2024, DeepSeek-V3), unabhängig gefunden; dort ist der Grund Kausalität,
hier Determinismus.

Dazu die **Spreizungsstrafe** auf den Routergradienten: Sie liest die
Logit-Abstände statt der quantisierten Gewichte und hat deshalb ihren
grössten Wert genau dort, wo der Softmax-Gradient verschwunden ist.

#### ⚑ `aggregiere_duenn`: die Aggregation über dünn besetzte Beiträge

Bei 18 von 128 berührten Experten wäre ein dichtes Δm zu **86 Prozent
null**, also 560 Millionen Nullen je Ebene und Beitrag. ⚑ **Das ist
nicht Sparsamkeit, sondern der Kern der Sache:** Ein Expertengemisch ist
genau deshalb billig, weil ein Token nur k Experten berührt; eine
Aggregation, die alle anfasst, hat den Vorteil weggeworfen.

Dünn und dicht liefern nachweislich dasselbe, zwei Beiträge auf dieselbe
Stelle addieren sich statt zu überschreiben, und geklemmt wird auch hier
genau einmal.

### v0.45.0 – 2026-09-05 (kernels 0.42.0, runtime 0.30.0: die Aggregation, und Δm als eigenes Commitment)

**TRAINING 2.2:** `optimierer::aggregiere` schreibt einen Master mit
vielen Beiträgen fort:

```text
m_{v+1} = klemmen(m_v + Σ Δm_i)
```

⚑ **Ordnungsfrei, und geklemmt genau einmal am Ende.** Wer je Summand
klemmt, bekommt ein Ergebnis, das an der Reihenfolge hängt, und die
bestimmt im Netz niemand: Pakete überholen sich. Mit Obergrenze 100 und
den Beiträgen `+80, +80, −80` gibt die Reihenfolge links 20 und rechts
80; einmal am Ende geklemmt sind es beide Male 80. **Dieselbe Regel wie
in `mische_experten` und `linear_backward`** (Fund 24), zum dritten Mal
und aus demselben Grund. Die Gegenprobe steht daneben und scheitert
absichtlich, wenn der Aufbau die Sättigung gar nicht erreicht.

**`optimierer::delta`** bildet Δm als Differenz, mit Klemmung statt
Umlauf: Die Differenz zweier `i32` passt nicht immer in `i32`, und ein
Umlauf machte aus einem grossen Schritt einen grossen Schritt in die
Gegenrichtung.

### ⚑ Δm bekommt ein eigenes Commitment, neben dem Abdruck

`Trainingsergebnis` trägt jetzt beides, und keines ersetzt das andere:

| | Frage |
|---|---|
| `abdruck` (Endzustand) | Haben **zwei Maschinen dieselbe Arbeit** gleich gerechnet? |
| `delta_commitment` (Δm) | Was **addiert sich** in den nächsten Modellstand? |

⚑ **Endzustände addieren sich nicht.** Viele Miner rechnen verschiedene
Chargen gegen denselben Ausgangszustand; ihre Differenzen summieren
sich, ihre Endzustände nicht. Der Abdruck war für die Frage des
Miettags gebaut und ist dafür weiter richtig.

### v0.44.0 – 2026-09-05 (runtime 0.29.0: die Schleife nimmt ein Expertengemisch an)

`trainingsschleife` bricht auf einer MoE-Ebene nicht mehr ab, sondern
nimmt einen zweiten Weg: **Router und alle gewählten Experten lernen
gegen das nächste Token.** Gemessen an Qwen3-30B-A3B, Ebene 47:

| | |
|---|---|
| bewegte Gewichte | 7.233.979 von 47.448.064 |
| Kreuzentropie über 12 Schritte | 9,4583 auf **8,7000** |
| Trainingsabdruck | wird geliefert wie im dichten Lauf |

⚑ **Damit ist die sechste Stufe des Testclients auch auf einem
Expertengemisch ein Vergleichswert**, ohne dass der Client etwas davon
wissen muss.

**Trainiert wird der Expertenblock, die Aufmerksamkeit derselben Ebene
bleibt eingefroren.** Das ist keine Verkürzung, sondern der Zuschnitt der
Frage: Die Aufmerksamkeit ist Zeile für Zeile derselbe Code wie im
dichten Lauf, und der belegt sie. Neu und unbelegt war der Weg durch
Mischung und Router.

⚑ **Die Master der Experten entstehen erst bei Bedarf.** 128 Experten je
Ebene kosten als Master 2,4 GB, und gebraucht werden sie nicht: **Ein
Experte, den der Router nie wählt, hat den Gradienten exakt null**, nicht
„fast null". Ein Master entsteht beim ersten Mal, dass sein Experte
gewählt wird.

⚑ **Und die Reihenfolge der Erzeugung geht nirgends ein.** Der Würfel des
Optimierers ist eine reine Funktion aus Ebene, Schritt und Index; der
Index eines Experten folgt aus **seiner Nummer**, nicht daraus, wann er
zum ersten Mal drankam. Sonst hinge das Ergebnis daran, welchen Weg der
Router zufällig zuerst nahm. Aus demselben Grund eine `BTreeMap` und
keine Hashtabelle.

⚑ **Was das nicht ist: geshardetes Training.** Der Lauf ist ein Prozess
auf einer Ebene. In COMPUTE_PIPELINE, NODE und CONSENSUS steht **keine
Zeile** Trainingscode; die Arbeitsklasse (TRAINING 2.1) und die
Aggregation (2.2) sind offen.

### v0.43.0 – 2026-09-05 (θ_v 0.18.0: der Boden im Router-Softmax, und der Routingpfad bekommt Vektoren)

**θ_v 0.18.0**, und es ist der erste Eintrag, den `moe` in dieser Datei
überhaupt hat. Bis heute stand der Pfad, der entscheidet **welche
Experten rechnen**, weder in der Formatfestlegung noch in einem
Konformitätsvektor; das ist die Hälfte von Fund 180.

#### ⚑ Der Boden (Fund 79)

`route_top_k` hebt bei `norm_topk_prob` jedes Mischgewicht auf
mindestens eins; der Überschuss geht vom grössten ab. Damit ist
`p_i >= 1` für jeden Verlierer und der Gewinner höchstens
`eins − (k−1)`: **beide Wege zum Gradienten null sind zu.**

| gemessen an Qwen3-30B-A3B | |
|---|---|
| im Betrieb, 4656 Routerstellen, 37.248 Gewichte | **schlägt nie an**, kein Bit ändert sich |
| im Training, Schrittweite 2^-10 und 2^-14 | ohne Boden 199/200 gesättigt, **mit Boden 0** |
| im Training, 2^-18 und 2^-22 | **bitgleich dasselbe Ergebnis** |

⚑ **Er wirkt genau dort, wo er rettet, und sonst nirgends.**

⚑ **Nur bei `norm_topk_prob`, und das ist eine Zusicherung, kein
Kommentar.** Die Regel „Überschuss vom grössten abziehen" setzt eine
Summe von exakt eins voraus, und die gibt es nur dort.

📌 **Drei Tests des Rückwärtspasses konnten den Zustand danach nicht mehr
bauen** und meldeten „der Aufbau saettigt nicht mehr". Sie bauen ihn
jetzt von Hand: Der Zustand ist aus einem **Weg** verschwunden, nicht
aus der Welt, denn `moe_backward` bekommt seine Gewichte als Argument.

#### ⚑ Der Umfang `moe`: vier Vektoren, Wert `bc7911c97f528e32`

Routing mit und ohne Normierung, ein Gleichstand an der Auswahlgrenze,
und die Mischung. Aus unabhängiger Nachbildung in
`tools/golden_moe.py`. Der Konformitätslauf steht damit bei **43/43**;
⚑ **`894d8357ae92b5c1` und `86e5e9835fe29565` sind unverändert.**

#### ⚑ Was θ_v 0.18.0 an den Artefakten ändert: nur die Versionszeile

Der Artefakt-Digest geht über `theta_v.json`, und dort steht die
Spec-Version. Alle vier Modelle haben deshalb einen neuen Digest, **ohne
dass sich ein Gewicht, eine Skala oder eine Tabelle geändert hätte**;
die drei Hashes daneben belegen es. Qwen2.5-0,5B:
`c42bb8a8d85bba5a` wird `ea442c3c8ecf628e`.

📌 **Und ein Test wurde dabei sprechend gemacht.**
`zwei_prozesse.rs` startet `myl-pod-node` als **vorgebautes** Binary und
warf dessen Fehlerausgabe weg. Als das alte Binary die neuen Artefakte
ablehnte, meldete der Test „der Shard-Dienst hat seine Adresse nicht
genannt": wahr und nutzlos. Jetzt steht die Ursache samt Abhilfe da.

📌 **Dreizehn Tests verschwanden im Release-Bau stumm**, weil sie
`#[cfg(debug_assertions)]` tragen. Dieselbe Klasse wie der `sqrt_q`-Test
von gestern; jetzt werden sie **genannt** statt weggelassen (184
bestanden, 13 übersprungen statt 184 ohne Hinweis).

### v0.42.0 – 2026-09-05 (kernels 0.40.0, runtime 0.28.0: der Rückwärtspass durch ein Expertengemisch; ⚑ Fund 79 gemessen, Funde 179 und 180)

`gradienten_des_gemisches` schliesst den Kreis über ein
Expertengemisch: drei Zweige, die sich am Eingang treffen. Durch die
Mischung in jeden gewählten Experten, durch die Mischgewichte in die
Routerlogits, und von dort durch die Routerprojektion. Der Mitschnitt
trägt dafür jetzt Inhalt statt der Marke „nicht aufgezeichnet".

#### ⚑ Fund 79 ist real, und er schlägt nach einem Schritt zu

Bis heute war der absorbierende Zustand des Routers eine Herleitung.
Gemessen an Qwen3-30B-A3B, nur der Router lernt, festes Ziel, 200
Schritte:

| Nenner | gesättigt | Abstand |
|---|---|---|
| 2^10 | **199/200** | 2,58e9 auf 2,30e9 |
| 2^14 | **199/200** | 2,58e9 auf **6,51e9** |
| 2^18 | 0/200 | 2,58e9 auf **2,03e9** |
| 2^22 | 0/200 | 2,58e9 auf 2,03e9 |

Bei 2^10 sättigt der Router im **ersten** Schritt und steht danach 199
Schritte still: `[13679, 1608, 226, …]` wird zu `[16384, 0, 0, …]`.

⚑ **Im Arbeitsfenster wird die Verteilung ausgeglichener, nicht
schärfer**, von 83 Prozent auf der Spitze auf 52. Der Verlust selbst
treibt zum Ausgleich, solange der Router sich bewegen kann.

**Die Lehre:** Die Schrittweite des Routers ist keine
Abstimmungsgrösse, sondern eine **Sicherheitsgrenze**. Zu gross heisst
nicht „langsam", sondern „tot", und der Schaden ist nicht behebbar.

#### ⚑ Ein Boden im Router-Softmax entfernt den Zustand und kostet nichts

Mit `w_i = max(w_i, 1)` und dem Überschuss vom grössten abgezogen:
Bei 2^10 und 2^14, wo der Router ohne ihn stirbt, lebt er. Bei 2^18 und
2^22 ist das Ergebnis **bitgleich dasselbe**.

⚑ **Das ist das Argument.** Der Boden wirkt genau dort, wo er rettet,
und sonst nirgends. **Offen und ausdrücklich nicht entschieden**, ob er
in θ_v kommt.

#### ⚑ Fund 179: ein Faktor acht im Routerzweig

`router_frac` statt `act_frac` als Ausgangsskala des
Eingangsgradienten. Der Test „der Abstand sinkt" blieb grün, weil er
`dL/dx` gar nicht benutzt. Gefunden hat es der Vergleich gegen die
**geschlossene Form**: Kosinus 0,9766 richtig gegen 0,7332 mit dem
Fehler.

⚑ **Bei dieser Körnung entscheidet der Winkel, nicht die einzelne
Komponente.** Auch der unabhängig belegte dichte Pfad streut je
Komponente um 0,74 bis 1,14.

#### ⚑ Fund 180: Der MoE-Routingpfad hat keinen Konformitätsvektor

`route_top_k` und `mische_experten` sind nirgends gegen ein festes Soll
geprüft. Das ist der Pfad, der entscheidet, **welche Experten rechnen**.
Offen.

### v0.41.0 – 2026-09-04 (kernels 0.39.0, runtime 0.27.0: TRAINING V Schritt 2e, die Schleife läuft; ⚑ Funde 177 und 178)

`runtime/tests/trainingsschleife.rs` schliesst den Kreis vom **echten
Ziel** bis in die Gewichte: Vorwärtspass über die letzte Ebene, Logits,
Softmax, Kreuzentropie, Rückwärtspass durch Kopf und
Abschlussnormierung, Schritt auf sieben Matrizen. **Gemessen an
Qwen2.5-0,5B über dreissig Schritte: Kreuzentropie 9,2204 auf 0,1371,
und das Zielwort ist am Ende der Argmax.**

⚑ **Was das zeigt und was nicht.** Es zeigt, dass ein echtes Ziel echte
Gewichte bewegt. Es zeigt **nicht**, dass ein Modell lernt: Trainiert
wird auf einer einzigen Folge gegen ein einziges Wort, und ein Verlust,
der dabei fällt, ist Auswendiglernen. Dafür braucht es einen Korpus und
eine Haltemenge, und genau daran ist am 2026-08-22 schon einmal eine
Messung gescheitert, die den Trainingsverlust für den Beleg hielt.

⚑ **Die Gegenprobe steht im selben Test.** Ein Schritt, der bloss alle
Logits anhebt, liesse auch den Verlust eines nicht trainierten
Kontrollworts fallen. Der gemessene Kontrollverlust **steigt** von
3,7944 auf 6,7173. Nimmt man dem Kreuzentropiegradienten den Zielterm,
steigt der Zielverlust auf 12,8000 und der Argmax landet bei 468.

#### ⚑ Fund 177: Der Softmax über das Vokabular verlor 88,5 % der Masse

Der erste Lauf der Schleife meldete dreissig Schritte lang **9,7041**,
unverändert bis auf die vierte Stelle. Der Wert ist 14 · ln 2, also
genau der Boden, den ein Verlust aus `p[ziel] = 0` annimmt.

`softmax_int` ist auf **Aufmerksamkeitspositionen** ausgelegt, und
theta_v 0.16.0 rechnet seine Überlaufschranke ausdrücklich für 2048
Einträge aus. Über 151.936 Wörter gilt keine seiner Annahmen mehr:

| gemessen an Qwen2.5-0,5B | |
|---|---|
| Summe der Wahrscheinlichkeiten | **1879 statt 16384** |
| Wörter mit `p = 0` | **150.064 von 151.936** |
| grösster Eintrag | 2 von 16384 |

⚑ **Es ist kein Rundungsfehler, es ist ein Boden.** Liegt der typische
Wert unter einer halben Einheit, rundet nicht die Hälfte hoch und die
Hälfte runter: **alle** fallen auf null. Das ist derselbe Defekt, den
theta_v 0.16.0 für lange Kontexte beschreibt, nur eine Grössenordnung
schlimmer.

⚑ **Dazu ein zweiter Fehler an derselben Stelle.** Der Aufrufer bildete
die Tabellenverschiebung als `logit_frac_bits − exp_input_frac`, also
`6 − 8`, **mit `saturating_sub`**. Heraus kam 0, und der Exponent wurde
viermal zu flach gelesen, ohne dass irgendetwas scheiterte.

**Die Antwort ist `softmax::softmax_ueber_vokabular`**: i64-Summe, freie
Ausgangsskala, und die Logitskala steht im Argument statt beim Aufrufer.
Die Zusicherung `2^frac_bits >= n` verbietet genau den Fall, der hier
eintrat, und `logit_frac >= exp_input_frac` ersetzt die Sättigung, die
den zweiten Fehler verdeckte. Dazu ein sechster Trainingsvektor
(`softmax_vokabular`), Umfang `training` damit auf `86e5e9835fe29565`.

⚑ **Der Kopf liefert die Skala jetzt auf Anfrage.** `logit_frac_bits`
steht auf 6, und der Kommentar dazu ist richtig: „nur fuer
Sampling/Argmax (skaleninvariant)". Wer eine Wahrscheinlichkeit braucht,
braucht mehr; `head_logits_mit_spur` nimmt die Skala deshalb als
Argument, und `head_logits` reicht unverändert die alte durch. **Der
Inferenzpfad rechnet Bit für Bit dasselbe**, und die 33 Layer- und
E2E-Vektoren belegen es.

#### ⚑ Fund 178: Der LM-Kopf stand dreimal wortgleich im Modul

Über einer der drei Kopien stand der Satz, eine zweite Umsetzung des
Kopfes wäre „eine zweite Wahrheit über den Rechenpfad", und es waren
schon drei: in `forward_token`, in `head_logits_mit_spur` und im
Aktivierungsabzug, fünfundzwanzig Zeilen, Zeichen für Zeichen gleich.

⚑ **Das hat schon einmal gekostet.** Fund 176 war genau dieser Fehler
eine Ebene höher. Jetzt gibt es `logits_aus_normiertem`, und alle drei
rufen es.

### v0.40.0 – 2026-09-04 (⚑ Fund 176: der Messpfad normierte zweimal; und die Masterskala bekommt eine Quelle)

### ⚑ Fund 176: `forward_token_mit_routing` lieferte falsche Logits

Der Weg normierte den Residualstrom selbst und reichte ihn dann an
`head_logits`, **das ihn ein zweites Mal normiert**, dazu auf der
falschen Eingangsskala. Gemessen an Qwen2.5-0,5B wichen **151 840 von
151 936 Logits** ab.

⚑ **Der Rechenpfad des Netzes war nie betroffen.** `forward_token`
schreibt Normierung und Kopf aus, und der Shard ruft `head_logits` mit
dem **rohen** Residualstrom. Betroffen war der **Messpfad**: die Zahlen,
mit denen eine Routing-Untersuchung arbeitet, und die Tokenfolge, die
sie dabei erzeugt. **Ein Messgeraet, das falsch misst, meldet keinen
Fehler**, und das ist dieselbe Klasse wie die Funde 33 bis 36.

⚑ **Aufgefallen ist es beim Lesen, nicht beim Testen.** Zwei Funktionen
lieferten dasselbe und niemand hatte sie je nebeneinandergelegt;
`beide_wege_liefern_dieselben_logits` tut das jetzt, und die Gegenprobe
faellt mit 151 840 Abweichungen.

### ⚑ Die Bruchstellen des Masters sind eine Festlegung, keine Wahl

`optimierer::MASTER_FRAC` ist neu, und die Zahl gehoert dorthin: Aus ihr
und der Zeilenverschiebung folgt der reale Wert eines Gewichts.
**Zwei ehrliche Miner, die denselben Schritt mit verschiedenen Werten
rechnen, bekommen verschiedene Gewichte**, und der Redundanzvergleich
meldete beide als fehlerhaft, ohne dass einer gelogen haette. Dieselbe
Lage wie beim Wuerfel des Optimierers, und dort steht die Antwort schon
ausgeschrieben.

📌 **Bis zum 2026-09-04 stand sie in zwei Testdateien**, also genau die
Lage, die dieses Projekt sonst durch einen Test verbindet oder auf eine
Quelle zurueckfuehrt.

**Offen bleibt der Ort.** Als Eigenschaft des Zahlenformats gehoert sie
in die Formatfestlegung neben die uebrigen Bruchstellen; heute steht sie
in der Bibliothek, weil eine Formataenderung **jeden** Golden Vector neu
erzeugen liesse. Solange sie dort steht, gilt: Ein Aufrufer, der sie
ueberschreibt, verlaesst das Protokoll.

### v0.39.0 – 2026-09-04 (kernels 0.37.0: TRAINING V Schritt 2d, der Zusammenbau zur ganzen Ebene)

`vorwaerts_der_ebene`, `gradienten_der_ebene` und `schritt_auf_ebene`
mit `Ebenenvorgaben`, `Ebenengewichte`, `Ebenenspur`,
`Ebenengradienten`, `Ebenentabellen` und `Normierungsgewichte`. Damit
laeuft der Kreis ueber eine **vollstaendige Transformer-Ebene**: zwei
Normierungen, zwei Bloecke, zwei Residualadditionen, sieben Matrizen.

⚑ **Der Beleg ist byteweise.** `die_ganze_ebene_trifft_den_mitschnitt`
vergleicht den Ausgang der Ebene mit dem **Eingang der naechsten Ebene**
aus dem Mitschnitt eines echten Vorwaertspasses, auf den Ebenen 0, 12
und 22 von Qwen2.5-0,5B ueber sechs Positionen.

### ⚑ Eine Annahme war falsch: die Residualskalen sind wirklich je Kanal

Am selben Tag stand hier noch, die Residualskalen des Artefakts seien
skalar und vom Lader nur repliziert. **Das war falsch.** Die Angabe
`shift` in `scales.json` ist der Skalar, der Lader nimmt aber `shifts`,
und der ist ein Vektor: Auf Ebene 12 spannt er von **vier bis
fuenfzehn**.

⚑ **Damit ist eine Ebene mit einer einzigen Ausgabeskala nicht bitgleich
zur Laufzeit**, und der byteweise Vergleich waere nicht moeglich
gewesen. Die Ebene rechnet jetzt kanalweise, und `linear_w8a16_pc` ist
im Aufmerksamkeitsblock wahlweise zugeschaltet: **Allein geprueft
traegt der Block eine Skala, in einer Ebene addiert er in den
Residualstrom.**

### ⚑ Die Ausgabeskala ist eine je Kanal, der Gradientenbus ist eine Zahl

Diese Unterscheidung war vorher nicht noetig und ist jetzt tragend.
Vorwaerts braucht ein Block die Kanalskalen des Residualstroms;
rueckwaerts nimmt `linear_backward` **eine** Skala fuer den eingehenden
Gradienten. Die Ebene rechnet an der Blockgrenze um.

⚑ **Genommen wird das Maximum**, denn dann ist jede Umrechnung von
`acc[i]` auf den Bus ein **Linksschieben und damit exakt**. Mit dem
Minimum verloere jeder feinere Kanal Stellen, bevor der Block ihn
ueberhaupt sieht.

### ⚑ Ein Zweig im Rauschen braucht eine Gleichung, keine Schranke

Die Residualaddition ist rueckwaerts eine **Verzweigung**: Beide
Summanden bekommen den vollen Gradienten. Der Zweig der **ersten**
Addition wirkt aber nur auf `dL/dx` der Ebene, und dort geht er neben
dem Beitrag der Normierung unter. 📌 **Seine Wegnahme aenderte das
gemessene Verhaeltnis von 0,872 auf 0,886**, also gar nichts.

**Der Ausweg ist ein Aufbau, in dem sich beide Seiten hinschreiben
lassen:** Sind `o_proj` und `down_proj` null, geben beide Bloecke null
aus, und die Ebene ist ein **Durchreicher**. Vorwaerts bleibt
`out = hidden`, rueckwaerts `dL/dx = dL/dy`, beide nur umskaliert. Der
Vergleich ist dann eine Gleichheit, und die Gegenprobe beisst.

⚑ **Und dieser Zweig ist der Grund, warum tiefe Netze trainierbar
sind:** Er fuehrt den Gradienten an der Ebene **vorbei** nach unten,
ungedaempft durch Normierung und Bloecke.

### ⚑ Zwei Nullmutationen und ein Fehler in der Testrechnung

- **Bei `aus_frac` unterhalb von `residual_mid_frac` ist `acc_mlp`
  gleich `aus_frac`**, der Buswechsel zum Feedforward-Block also ein
  Nullschritt. Zwei Gegenproben blieben gruen, weil sie nichts
  vertauschten. Mit 11/8/10 schieben beide Wechsel wirklich.
- 📌 **Q, K und V tragen den inneren Bus des Blocks, nicht die
  Additionsskala.** Mit dem falschen Exponenten lagen die Verhaeltnisse
  je nach Skalenwahl bei 0,07 oder bei 2,1, und beide Male sah es nach
  einem Fehler im Code aus. **Eine Erwartung, die falsch gerechnet ist,
  sieht aus wie ein Fund.**
- 📌 **Bei Schrittweite eins bewegt sich genau ein Gewicht**, und eine
  einzelne ganzzahlige Aenderung ist Raster und keine Messung. Gemessen:
  3 326 vorhergesagt gegen 114 gemessen; bei Weite zwei 22 124 gegen
  21 574.

**Zwoelf Gegenproben, alle rot**, davon vier nur am byteweisen
Vergleich: falsche Zielskala nach der ersten Addition, `acc_attn` ohne
das Minimum, falsche MLP-Ausgabeskala, vertauschte Gammas.

### Was die Ebene ausdruecklich nicht tut

**Die beiden Gammas lernen.** Ein Gamma traegt eine Skala je **Element**,
ein Gewicht eine je **Zeile**; ein Master fuer das eine ist etwas
anderes als fuer das andere. `rmsnorm_backward` rechnet den
Gamma-Gradienten aus, dieser Schritt verwirft ihn. Dasselbe gilt fuer
die Vorspannungen.

### v0.38.0 – 2026-09-04 (kernels 0.36.0: der Trainingspfad bekommt Golden Vectors, Umfang `training`)

Fuenf neue Vektoren unter `conformance/vectors/training/`, erzeugt von
`tools/golden_training.py` aus einer **unabhaengigen** Nachbildung:
`backward_attention`, `backward_silu`, `backward_rmsnorm`,
`backward_embedding` und `optimierer_schritt`. Der Prueflauf steht damit
bei **38 von 38** statt 33 von 33.

⚑ **Sie schliessen die Luecke, aus der die Funde 173 bis 175 kamen.**
Drei der acht Rueckwaertskerne rechneten falsch, und der Lauf, der 33
von 33 meldete, hat keinen von ihnen je gerechnet. **Gegengeprueft:**
Dreht man Fund 173 oder Fund 175 zurueck, aendert man den Wuerfel des
Optimierers oder laesst den Embedding-Gradienten zuweisen statt
addieren, faellt der Trainingsumfang.

### ⚑ Ein eigener Umfang, damit eine bestehende Zusage nicht bricht

`894d8357ae92b5c1` steht an sechs Stellen des Repositoriums fest,
darunter beide CI-Laeufe. Neue Vektoren im `op`-Umfang haetten ihn
geaendert, also eine Zusage gebrochen, um eine neue aufzustellen.
**Deshalb ein eigener Umfang und drei Werte je Lauf:**

| Vergleichswert | Umfang | Wert |
|---|---|---|
| `konformitaet_op` | 6 Vektoren | `894d8357ae92b5c1`, **unveraendert** |
| `konformitaet_training` | 6 Vektoren | `86e5e9835fe29565` |
| `konformitaet_moe` | 4 Vektoren | `bc7911c97f528e32` |
| `konformitaet` | der ganze Lauf | `ed7b5042352f6a82` |

⚑ **Und je Stufe ein Wert grenzt eine Abweichung ohne zweiten Lauf
ein.** Weichen beide Stufen ab, sitzt es unterhalb, in den
Grundoperationen; weicht nur der Trainingswert ab, sitzt es im
Rueckwaertspass. Dieselbe Ueberlegung wie hinter `digest_umfang`.

### ⚑ Die Herkunft steht im Vektor, nicht im Kommentar

Das Format traegt ein Feld `herkunft`. **`unabhaengig`** heisst: Eine
getrennte Umsetzung hat die Sollwerte gerechnet, eine Uebereinstimmung
belegt **Richtigkeit**. **`selbsterzeugt`** heisst: Dieser Code hat sie
selbst erzeugt, eine Uebereinstimmung belegt **Determinismus zwischen
Maschinen** und sonst nichts.

⚑ **Wer die beiden verwechselt, haelt eine Selbstzertifizierung fuer
einen Beleg**, und genau das war Fund 105 in anderer Gestalt: Ein
zweiter `cargo build` genuegte damals fuer ein Urteil ueber Hardware.
Fehlt das Feld, gilt `unabhaengig`, denn alle Vektoren bis zum
2026-09-04 sind es; ein stillschweigend anderer Vorgabewert waere die
gefaehrlichere Richtung. Ein Test haelt fest, dass alle fuenf neuen
Vektoren `unabhaengig` tragen.

### Was ausdruecklich fehlt

**`moe_backward`.** Sein Eingang ist eine Routing-Entscheidung, und eine
unabhaengige Nachbildung braeuchte den Router mit. Das gehoert zu
Phase 5 der Trainingsseite und ist dort benannt, statt hier halb
gemacht zu werden.

**Und die beiden Bloecke** (`schritt_auf_mlp`,
`schritt_auf_aufmerksamkeit`). Eine unabhaengige Nachbildung braeuchte
den ganzen Vorwaertspfad ein zweites Mal in Python; was sie leisten
koennen, ist ein **Determinismus**beleg, und der gehoert an den Lauf auf
echten Gewichten und nicht an einen erfundenen Vektor.

### v0.37.0 – 2026-09-04 (kernels 0.35.0: ⚑ Fund 174 geschlossen, die Skala des Masters ist ein Argument)

`gewicht_aus_master` nimmt `master_frac` und gibt als
Zeilenverschiebung die **Differenz** `master_frac − s` zurueck statt `s`.
Damit ist der reale Wert eines Gewichts `master / 2^master_frac`,
unabhaengig davon, wie viele Stellen zum Hineinpassen in `i8` wegfallen.
`Schrittvorgaben`, `Mlpvorgaben` und `Aufmerksamkeitsvorgaben` tragen das
Feld; die Laeufe auf echten Gewichten setzen es auf zwanzig.

**Was vorher galt:** Der reale Wert war `master / 2^(2·s)`. Solange `s`
sich nicht aenderte, war das eine Proportionalitaet. Ueber eine
Oktavgrenze hinweg fiel er auf ein Viertel, und die Abbildung war dort
nicht einmal monoton: Betragsmaximum 127 gab 127,00, Betragsmaximum
**128** gab **32,00**.

### ⚑ Die Wirkung war groesser als der Sprung an der Grenze

Das war die auffaellige Haelfte. Die stille Haelfte ist diese: Ein
Schritt wirkte auf eine Zeile um `2^(2·shift)` gedaempft, und **eine
Zeile mit kleinen Gewichten traegt eine grosse Verschiebung**. Rechnerisch
bewegte sich eine Zeile mit `shift = 20` um den Faktor `2^40` langsamer
als eine mit `shift = 0`. ⚑ **Die feinskalierten Zeilen waren praktisch
eingefroren, und niemand hat es gesehen**, weil der Abstand trotzdem
fiel: Die groben Zeilen allein genuegten dafuer.

**Gemessen an Qwen2.5-0,5B, sechzig Schritte:**

| | vor der Behebung | nach der Behebung |
|---|---|---|
| Aufmerksamkeit, Ebene 0 | 97 % (Rate `1/2^14`) | 97 % (Rate `1/2^18`) |
| Aufmerksamkeit, Ebene 12 | 91 % | **99 %** |
| dieselbe Rate `1/2^14` | faellt | **steigt auf das 64-fache** |

⚑ **Die Lernrate musste sechzehnmal kleiner werden**, und das ist die
Bestaetigung: Vorher war sie fuer die groben Zeilen gewaehlt, waehrend
die feinen nichts taten. Seit sich alle gleich schnell bewegen, laesst
dieselbe Rate den Lauf davonlaufen.

### ⚑ Und eine frueher berichtete Zahl war ein Artefakt des Aufbaus

Der MLP-Blocktest meldete bisher, der Gradient sage in **82 bis 85
Prozent** der Faelle die Richtung richtig voraus, und der Modulkopf
erklaerte das mit der Quantisierung. **Das stimmte nicht.** Die Master
standen in Rasterstufen mit Betraegen bis sechs, und der Schub von
vierundsechzig war das **Zehnfache des Gewichts**. Seit die Master acht
Bruchstellen tragen, ist derselbe Schub ein Bruchteil davon, und die
Quote liegt bei **hundert Prozent**. Die Zahl beschrieb den Aufbau, nicht
das Verfahren.

### ⚑ Der Rundlauf ueber das Artefakt ist jetzt eine Zusicherung

`der_weg_vom_artefakt_zum_master_und_zurueck_ist_exakt` haelt fest, dass
`master_aus_gewicht` und `gewicht_aus_master` Umkehrungen sind, ueber
einundzwanzig Matrizen auf drei Ebenen von Qwen2.5-0,5B, **byteweise**.
Vorher galt das nur zufaellig: Der Rundlauf traf, solange das
Betragsmaximum einer Zeile in der obersten Oktave lag, und das tut es bei
einem frisch quantisierten Artefakt. **Nach dem ersten Trainingsschritt
nicht mehr.** Ohne diese Zusicherung traete der erste Schritt gegen ein
anderes Modell an als das geladene.

**Und ein zu grosses Gewicht bricht ab**, statt still zu saettigen: Ein
realer Wert ueber 127 laesst sich als `i8` mit einer Zeilenverschiebung
nicht ausdruecken, und eine stille Kappung fiele erst an der
Verlustkurve auf, wo sie wie ein Trainingsproblem aussieht.

### v0.36.0 – 2026-09-04 (kernels 0.34.0: die Ebene wird zusammensetzbar, ⚑ Fund 175)

Drei Vorarbeiten fuer den Zusammenbau zur ganzen Transformer-Ebene, und
ein Fund, der dabei herausfiel.

**1. Beide Bloecke geben ihren Eingangsgradienten heraus.**
`Mlpgradienten` und `Aufmerksamkeitsgradienten` tragen ein Feld
`eingang`. Ein Block fuer sich haengt an einem Ziel, und dort endet die
Kette; **in einer Ebene endet sie nicht**, denn der Eingang eines Blocks
ist die Ausgabe einer Normierung.

⚑ **Er ist eine Summe.** Gate und Up lesen denselben Eingang, Q, K und V
ebenso. Wer nur einen Beitrag nimmt, halbiert oder drittelt den
Gradienten, **ohne seine Richtung zu aendern**.

**2. `rmsnorm_i16_mit_spur` schneidet den Kehrwert der Wurzel mit.**
`rmsnorm_backward` darf ihn nicht nachrechnen: Der Tabellenindex
entsteht aus einer dynamischen Verschiebung, und ein zweiter Nachschlag
koennte einen anderen Eintrag treffen. Die Spur ist ein `enum` und keine
Zahl: `Leer`, `Null` (der Eingang war ueberall null, **es gibt kein
`r`**) und `Wert { r, norm_frac, ref_shift }`. ⚑ **Drei Zahlen und nicht
eine**, weil `r` allein seine Skala nicht traegt.

### ⚑ Fund 175: `rmsnorm_backward` rechnete an drei Stellen mit Darstellungen

Dieselbe Familie wie Fund 173 und derselbe Grund: **kein Aufrufer
ausserhalb der eigenen Tests**, und die beiden vorhandenen Tests prueften
Eigenschaften, die von jeder Skala unabhaengig sind (dass der zweite Term
ueberhaupt wirkt, und dass ein Nullgradient nichts erzeugt).

| Groesse | stand | gehoert |
|---|---|---|
| `r` | `r / 2^norm_frac` | `r / 2^(norm_frac − ref_shift)` |
| `x` im zweiten Term | die Darstellung | `x / 2^x_shifts[j]` |
| `x` in der Summe | die Darstellung | `x / 2^x_shifts[i]` |

⚑ **Und `x_shifts` fehlte in der Signatur ganz.** Der Vorwaertspass
traegt seit Fund 20 eine Skala **je Kanal**; dieser Kern kannte sie
nicht und konnte deshalb nicht einmal im Ansatz stimmen, sobald sie
auseinandergehen.

**Gemessen gegen die geschlossene Form der Ableitung**, mit paarweise
verschiedenen Kanalskalen: Die Verhaeltnisse streuten von **1,08 bis
−37,4**, also einschliesslich gedrehter Vorzeichen. Nach der Behebung
liegen sie ueber acht Kanaele bei **0,986 bis 1,005**, der
Gamma-Gradient trifft auf ein Promille.

### ⚑ Warum hier keine numerische Ableitung prueft

Jeder andere Rueckwaertskern wird gegen die numerische Ableitung des
echten Vorwaertskerns gehalten. **Bei RMSNorm traegt das nicht:** `r`
ist eine **ganze** Zahl aus der Tabelle, also ist `dr/dx` eine Treppe.
Ein Schub an einem grossen Kanal verschiebt den Index um eine Stufe, und
die Differenz misst den Sprung statt der Steigung. Gemessen trafen die
vier kleinen Kanaele auf drei Promille, die vier grossen streuten
zwischen −5 und +9.

**Verglichen wird deshalb mit der Formel selbst**, ausgerechnet aus
denselben realen Groessen. Das ist keine zweite Umsetzung des
Vorwaertspasses, sondern die **Spezifikation des Rueckwaertspasses**,
und genau ihre Skalenbuchhaltung war falsch.

### ⚑ Drei Nebenbefunde beim Beheben, alle aus der eigenen Regelliste

- **Ein Doc-Kommentar hatte den Anschluss verloren.** `silu_grad_aus_lut`
  und `silu_grad_frac` sind in v0.32.0 **zwischen** den Kommentar von
  `rmsnorm_backward` und die Funktion geraten; `rmsnorm_backward` hatte
  seither gar keine Beschreibung, und `silu_grad_aus_lut` trug eine, die
  mit einem Satz ueber RMSNorm beginnt. Beide stehen wieder an ihrem
  Platz.
- 📌 **Die erste Behebung rundete den Normierungsterm auf null weg.**
  `r` von 106 auf neun Bruchstellen ergibt `r³` gerundet **5**, und
  `5 · 300 >> 15` ist **null**. Jetzt mit vierundzwanzig gerechneten
  Schutzstellen: `r_real³ ≤ 1` gibt `2^(rf+24) ≤ 2^44`, mal `x ≤ 2^15`
  sind `2^59`, und das Produkt der beiden Faktoren passt in `i128`.
- 📌 **Der erste Testaufbau hatte eine Summe, die sich fast aufhob**
  (−0,38 statt 74,6). Der zweite Term trug dann nichts bei, und drei
  Gegenproben blieben gruen. **Eine Summe, die sich aufhebt, ist eine
  Pruefung, die nichts auswaehlt.** Vorzeichen gewaehlt statt
  gewuerfelt, danach beissen alle sieben.
- 📌 **Und ein bestehender Test hatte unstimmige Skalen**, die vorher
  niemandem auffielen: Eingang 30 000 auf Verschiebung null ist real
  30 000, dazu ein `r` von eins waere die Behauptung, der quadratische
  Mittelwert sei eins. Mit der richtigen Formel saettigte alles. Die
  Skalen passen jetzt zueinander.

### v0.35.0 – 2026-09-04 (kernels 0.33.0: der Kreis ueber den Aufmerksamkeitsblock, ⚑ Funde 173 und 174)

`schritt_auf_aufmerksamkeit`, `gradienten_der_aufmerksamkeit` und
`vorwaerts_der_aufmerksamkeit` mit `Aufmerksamkeitsvorgaben`,
`Aufmerksamkeitsgewichte`, `Aufmerksamkeitsspur`,
`Aufmerksamkeitsgradienten` und `Vorspannungen`. Damit ist der Kreis
aus Vorwaertspass, Gradient und Fortschreibung ueber **beide** Blocke
einer Ebene geschlossen: Q, K, V, RoPE, Softmax, Kopfgewichtung und
Ausgabeprojektion, mit gruppierter Aufmerksamkeit.

⚑ **Ueber eine Folge und nicht ueber eine Position, und das ist keine
Bequemlichkeit.** Bei einer einzigen Position gibt es genau einen
Schluessel, der Softmax liefert exakt eins, und seine Ableitung
`p · (g − ⟨g, p⟩)` ist damit **exakt null**: Q und K bekaemen keinen
Gradienten. Ein Aufbau mit einer Position haette ausschliesslich V und
die Ausgabeprojektion geprueft, waehrend die Ueberschrift
„Aufmerksamkeitsblock" lautet. Ein Test haelt das fest.

### ⚑ Fund 173: `attention_backward` rechnete nach einer anderen Lesart als jeder andere Rueckwaertskern

Ein Gradient traegt in diesem Projekt `dL/dZ` nach dem **realen Wert**
von `Z`, dargestellt auf einer Skala, die der Aufrufer waehlt;
`linear_backward`, `silu_backward` und `rmsnorm_backward` nehmen sie als
`g_frac` herein und als `gx_frac` heraus. **`attention_backward` rechnete
nach der Darstellung**, und die beiden Lesarten unterscheiden sich um
`2^(2·frac)`.

Konkret standen an vier Stellen andere Schiebeweiten:

| Schritt | stand | gehoert | weil |
|---|---|---|---|
| `dL/dv` | `prob_frac` | `prob_frac` | der Faktor ist `p` |
| `dL/dp` | `prob_frac` | **`v_frac`** | der Faktor ist `v` |
| `dL/dq` | Vorwaerts-`score_shift` | **`k_frac + 15`** | der Faktor ist `k` |
| `dL/dk` | Vorwaerts-`score_shift` | **`q_frac + 15`** | der Faktor ist `q` |

Gemessen gegen die numerische Ableitung des echten Vorwaertskerns war
`dL/dq` um den Faktor 4 daneben und `dL/dk` um 8 (bei den Skalen der
Probe); mit den Skalen von Qwen2.5-0,5B waeren es 256 und 512.

⚑ **Aufgefallen ist es nicht, und das hat zwei Gruende, die beide
bekannt sind.** Erstens hatte die Funktion **ausserhalb ihrer eigenen
Tests keinen Aufrufer**; sie war gebaut, geprueft, abgehakt und
unbenutzt. Zweitens prueften ihre Tests genau die Richtung, in der sich
die beiden Lesarten **nicht** unterscheiden: `dL/dv` traegt dieselbe
Skala wie die Ausgabe, und fuer gleiche Skalen fallen sie zusammen. Der
neue Test `der_gradient_nach_q_und_k_trifft_die_numerische_ableitung`
setzt deshalb `q_frac`, `k_frac`, `v_frac`, `score_frac` und `prob_frac`
**paarweise verschieden**.

⚑ **Und `score_frac` kommt im Rueckwaertspass nicht mehr vor.** Das ist
die Probe auf die Lesart: `s` und `p` sind reale Groessen, und die
Ableitung des Softmax ist zwischen ihnen dimensionslos. Wer hier eine
Punktzahlskala braucht, rechnet nach der Darstellung.

### ⚑ Fund 174: `gewicht_aus_master` hat keinen Begriff von der Skala des Masters

Der reale Wert eines Gewichts ist `(master >> s) / 2^s` mit dem dort
gewaehlten `s`, also `master / 2^(2·s)`. Solange `s` bleibt, ist das
eine Proportionalitaet. **Aendert `s` sich, springt der Wert der ganzen
Zeile:**

| Betragsmaximum | `s` | groesstes `i8` | realer Wert |
|---|---|---|---|
| 127 | 0 | 127 | 127,00 |
| **128** | **1** | **64** | **32,00** |
| 254 | 1 | 127 | 63,50 |
| **256** | **2** | **64** | **16,00** |

**Ein Master, der um eins waechst, laesst das Gewicht auf ein Viertel
fallen**, sobald das Betragsmaximum einer Zeile eine Zweierpotenz
ueberschreitet. In einem langen Trainingslauf passiert das
zwangslaeufig, und es saehe aus wie „das Modell wird ploetzlich
schlechter".

**Nicht behoben**, denn der Weg heraus ist eine feste Bruchstellenzahl
des Masters als Argument (Verschiebung `master_frac − s` statt `s`) und
damit eine Aenderung an jedem Aufrufer. Festgehalten sind die Grenze im
Doc-Kommentar und die Sprungstelle als Test
(`die_oktavgrenze_bricht_die_proportionalitaet`), der beim Beheben
**absichtlich** rot wird.

### ⚑ Zwei Umsetzungen desselben Vorwaertspasses, und was sie zusammenhaelt

Der Trainingsschritt des MLP-Blocks ruft **denselben** Kern wie die
Inferenz (`mlp_int_mit_spur`). Fuer die Aufmerksamkeit geht das nicht:
Sie steht im Vorwaertspass der Laufzeit ausgeschrieben, ueber **eine**
Position mit Zwischenspeicher, waehrend das Training eine **Folge**
braucht. Es gibt also zwei Umsetzungen, und das ist sonst genau die
Falle „zwei Wahrheiten ueber den Rechenpfad".

⚑ **Deshalb steht der Vorwaertspass als eigene, oeffentliche Funktion
da und nicht im Rumpf des Gradienten:** Nur so laesst er sich gegen die
aufgezeichneten Zwischenwerte eines echten Vorwaertspasses stellen.
`runtime/tests/trainingslauf.rs::die_vorwaerts_haelfte_trifft_den_mitschnitt`
vergleicht die Aufmerksamkeitsausgabe **byteweise**, auf den Ebenen 0,
12 und 23 von Qwen2.5-0,5B und ueber sechs Positionen. Vier
Gegenproben, die kein Kerneltest faengt, scheitern daran: eine
vertauschte Kopfgruppe, eine weggenommene Umskalierung, ein zusaetzlich
gedrehtes V und ein Positionsversatz, der nur auf einer Seite gilt.

### ⚑ Was der Test aus dem MLP-Block gelernt hat, und was er dazugelernt hat

Beim MLP-Block war der Ertrag: „der Abstand sinkt" ist zu schwach, eine
Richtungspruefung an einer einzelnen Stelle ist ein Zufallsgenerator,
und ein Groessenvergleich zwischen symmetrischen Aesten faengt einen
Skalenfehler. Alle drei stehen auch hier (Q und K sind symmetrisch).

⚑ **Dazugelernt: Im Aufmerksamkeitsblock traegt die Richtungspruefung
je Gewicht gar nicht mehr.** Gemessen ueber sieben Schubweiten von 64
bis 1024 **schwankt sie ueber jede Schranke hinweg**, die man setzen
koennte:

| | Q | K | V | O |
|---|---|---|---|---|
| Anteil richtig | 63 bis 77 % | 61 bis 80 % | 84 bis 95 % | 79 bis 95 % |

**Bei siebzig Prozent waere der Test fuer Q an drei der sieben
Schubweiten rot und fuer K an drei**, ohne dass am Gradienten etwas
falsch ist. Der Grund ist der Weg, den Q und K nehmen: nur ueber den
Softmax, mit kleinem Beitrag zur Ausgabe, und ein Schub, der die
Quantisierung ueberwindet, ist laengst ausserhalb des linearen
Bereichs.

**An ihre Stelle tritt ein Schritt entlang des ganzen Gradienten**, der
die Spruenge herausmittelt und die Aussage prueft, die ein Gradient
wirklich macht: **um wie viel** der Abstand faellt.

| gemessen / vorhergesagt | Q | K | V | O |
|---|---|---|---|---|
| gesund | 0,84 | 1,11 | 0,88 | 1,03 |
| Drehung nicht zurueckgenommen | **0,01** | **0,42** | | |
| Positionsversatz nur auf einer Seite | | **0,48 / 0,64** | | |
| K-Gradient zugewiesen statt summiert | | **0,64** | | |

⚑ **Die Schranke darf eng sein (0,75 bis 1,50), weil der ganze Lauf
ganzzahlig und damit auf jeder Maschine bitgleich ist.** Sie faengt
damit auch einen Faktor zwei.

### ⚑ Und drei Stellen, an denen der Aufbau eines Tests selbst der Befund war

- **Eine Skala, die zufaellig einer anderen gleicht, prueft nichts.**
  Mit `act_frac = aus_frac` ist ein Vertauschen der beiden ein
  Nullschritt; mit `attn_out_frac = act_frac` ebenso. Der Aufbau setzt
  alle Skalen paarweise verschieden.
- **Eine Drehtabelle mit grosser Grundzahl dreht nur das erste Paar.**
  Mit der echten Grundzahl (eine Million) drehen die uebrigen Paare um
  Bruchteile eines Tausendstels, und die Ruecknahme der Drehung ist
  nicht pruefbar. Der Aufbau nimmt drei.
- ⚑ **Ein Abschnitt, der bei Position null beginnt, laesst genau den
  Fall aus, in dem RoPE etwas tut.** Dort ist die Drehung die Einheit,
  und der Schluessel der ersten Position wird von **jeder** Abfrage
  gelesen, geht also am staerksten in den Gradienten ein. Daraus ist ein
  Feld geworden: `positionsversatz`. **Ein Trainingsabschnitt beginnt
  nicht bei null**, seine Positionen sind die im Text.

### ⚑ Was der Block ausdruecklich nicht tut

- **Die Normierung der Koepfe vor RoPE**, die manche Modelle haben. Der
  Vergleich mit dem Mitschnitt prueft ausdruecklich, dass das geladene
  Modell keine hat, statt still danebenzurechnen.
- **Die Vorspannungen lernen.** Sie gehen als Konstanten ein, weil der
  Block sonst ein anderer waere als der, der laeuft; fortgeschrieben
  werden sie nicht. Eine Vorspannung liegt als `i16` mit einer Skala je
  Element vor, ein Gewicht als `i8` mit einer Skala je Zeile: Ein Master
  fuer das eine ist etwas anderes als fuer das andere.
- **Den Gradienten nach dem Blockeingang herausgeben.** Er wird
  gerechnet und verworfen; gebraucht wird er erst beim Zusammenbau zur
  ganzen Ebene.

**Gemessen auf echten Gewichten** (Qwen2.5-0,5B, sechs Positionen, 60
Schritte, Rate `1/2^14`): Ebene 0 faellt um 97 Prozent, Ebene 12 um 91.
Mit umgekehrtem Vorzeichen steigt der Abstand von 3,3e9 auf 1,1e12.

### v0.34.0 – 2026-09-04 (der Trainingslauf auf echten Gewichten, und was er gefunden hat)

`runtime/tests/trainingslauf.rs`: derselbe MLP-Schritt wie im
Kerneltest, aber auf **13 074 432 echten Gewichten**, einem
Aktivierungsvektor aus einem echten Vorwaertspass und den Skalen der
jeweiligen Ebene.

⚑ **Der Unterschied zum Kerneltest ist nicht die Groesse, sondern die
Verteilung.** Erfundene Gewichte sind gleichmaessig; echte haben
Ausreisser, Nullzeilen und Spannen ueber Groessenordnungen.

| Ebene | Abstand vorher | nachher | gefallen |
|---|---|---|---|
| 0 | 801 131 902 414 | 35 223 587 769 | 96 % |
| 12 | 705 929 702 | 2 964 334 | 100 % |
| 23 | 59 065 050 066 | 150 098 420 | 100 % |

Gegenprobe: mit umgekehrtem Schritt steigt der Abstand auf Ebene 12 von
706 Millionen auf **926 Milliarden**.

### ⚑ Der Befund: eine Lernrate passt nicht zu allen Ebenen

Bei vierzig Schritten fiel Ebene 0 nur um **achtzehn** Prozent, Ebene 18
um 63, die uebrigen um 99 bis 100. Bei zweihundert Schritten fallen alle.

| Ebene | typische Ausgabe | `aus_frac` | 40 Schritte | 200 |
|---|---|---|---|---|
| 0 | 29 702 | **19** | 18 % | 96 % |
| 12 | 882 | 13 | 99 % | 100 % |
| 18 | 555 | 12 | 63 % | 99 % |

⚑ **Die Richtung stimmt ueberall, die Rate nicht.** Ebene 0 traegt sechs
Bit mehr Ausgabeskala als die mittleren, also ist derselbe Schritt dort
vierundsechzigmal kleiner. **Und ihre typische Ausgabe liegt bei 29 702,
dicht unter der `i16`-Grenze**: Wer die Rate dort anhebt, ohne das zu
bedenken, laeuft in die Saettigung.

**Fuer einen echten Trainingslauf folgt:** Die Lernrate gehoert je Ebene
gesetzt oder der Gradient normiert.

### ⚑ Und das Training laeuft auch auf einem Expertengemisch

`runtime/tests/training_moe.rs` (mit `--ignored`, weil das Laden gut
zwei Minuten kostet):

```text
geladen in 131 s: 48 Ebenen, hidden 2048
Mitschnitt: 48 Ebenen, alle als Expertengemisch gemeldet
Ebene 24: Router waehlte 8 von 128 Experten, geprueft wird Experte 10
Experte: 2048 Eingaenge, 768 innere Einheiten, 4 718 592 Gewichte
Abstand 547 716 633 -> 10 643 336 (99 Prozent gefallen)
```

⚑ **Die erste Einschaetzung war falsch.** „29 GB passen nicht in 24 GB"
uebersieht, dass die Gewichte **speicherabgebildet** werden und nicht in
den Heap gehen; der Ladevorgang liest sie einmal fuer die Hashpruefung,
danach haelt das Betriebssystem nur die angefassten Seiten.

**Ein Experte ist ein dichter Block** (`MoeLayer::experts` ist ein
`Vec<DenseMlp>`), also gilt `schritt_auf_mlp` unveraendert. Trainiert
wird ein Experte, den der Router fuer dieses Token **wirklich gewaehlt
hat**.

⚑ **Nicht abgedeckt bleibt der Rueckwaertspass durch Router und
Mischung**; dafuer verlangt `moe_backward` andere Werte, und der
Mitschnitt sagt dort ausdruecklich `Mlpteil::Expertengemisch`. Das ist
Phase 5.

Der Typ selbst ist aus diesem Versuch entstanden: Vorher standen drei
Vektoren da, die bei MoE leer blieben. **Ein leerer Vektor sieht aus wie
ein aufgezeichneter ohne Inhalt**; ein Typ zwingt jeden Leser, den Fall
zu behandeln.

### v0.33.0 – 2026-09-04 (kernels 0.32.0: der Kreis schliesst sich ueber einen ganzen MLP-Block)

`schritt_auf_mlp` und `gradienten_des_mlp` mit `Mlpvorgaben` und
`Mlpgradienten`, dazu `silu_grad_aus_lut` und `silu_grad_frac`.

⚑ **Der Gradientenvorrat ist die Ableitung der Vorwaerts-Tabelle, nicht
der gemeinten Funktion.** Der Vorwaertspass schlaegt Silu nach, statt es
zu rechnen; ein analytischer Vorrat passte deshalb zu einer Funktion,
die so nie lief. Die zentrale Differenz der Tabelle ist **exakt**, weil
sich zwei Zweien wegkuerzen: keine Division, keine Rundung, kein
Gleitkomma.

**Warum der Block und nicht die drei Projektionen einzeln:**
`schritt_auf_linear` schliesst den Kreis fuer eine lineare Ebene. Was
dort nicht vorkommt, ist die Stelle, an der Ganzzahltraining bricht,
naemlich **die Skala zwischen zwei Kernen**. Der MLP-Block hat davon
vier hintereinander.

### ⚑ Der Test hat sich dreimal als zu schwach erwiesen

| Gegenprobe | „Abstand sinkt" | Richtung, eine Stelle | Richtung, aggregiert | plus Groessenvergleich |
|---|---|---|---|---|
| Verlustgradient gedreht | rot | rot | rot | rot |
| Gradientenaeste vertauscht | **gruen** | unbrauchbar | rot | rot |
| Vorrat auf falscher Skala | **gruen** | **gruen** | **gruen** | rot |

⚑ **„Der Abstand sinkt" beweist weniger, als es klingt.**
Abstiegsverfahren sind gutmuetig: Viele falsche, aber korrelierte
Richtungen senken einen quadratischen Abstand auch.

⚑ **Und eine Richtungspruefung an einer einzelnen Stelle ist ein
Zufallsgenerator.** Der Schub muss die Quantisierung ueberwinden und
liegt damit weit ausserhalb des linearen Bereichs; **gemessen sagt der
Gradient in 82 bis 85 Prozent der Faelle richtig voraus, nicht in
hundert.** Der erste Entwurf pruefte eine Stelle je Matrix und fiel an
einer der achtzehn Prozent.

⚑ **Ein Richtungstest kann einen Skalenfehler prinzipiell nicht
sehen.** Ein Vorrat auf der falschen Skala macht den Gate-Gradienten
zweiunddreissigmal kleiner, **ohne sein Vorzeichen zu aendern**; der
Abstieg laeuft weiter bergab, nur mit einer stillschweigend anderen
Lernrate. Dafuer steht jetzt ein Groessenvergleich daneben: Gate und Up
sind strukturell symmetrisch und duerfen sich nicht um
Groessenordnungen unterscheiden. Gemessen 159 gegen 122 im gesunden
Fall, **5 gegen 122** mit dem Fehler.

### v0.32.0 – 2026-09-04 (der Mitschnitt ist vollständig für eine dichte Ebene)

`attention_int_mit_spur` und `mlp_int_mit_spur` mit `Mlpspur`, dazu vier
neue Felder im `Ebenenmitschnitt`: die Aufmerksamkeitswahrscheinlichkeiten
je Kopf sowie Gate, Up und das Produkt des MLP-Blocks.

⚑ **Ein zweiter Eingang, kein zusätzliches Argument.** Beide Kerne
stehen im `Backend`-Merkmal, in vier Umsetzungen. Ein Argument mehr
risse alle vier auf, für etwas, das nur der Rückwärtspass braucht. Beide
neuen Eingänge laufen durch dieselbe Umsetzung wie die alten.

**Damit hat eine dichte Ebene zehn Werte je Durchlauf**, und das ist
alles, was `kernels::backward` von ihr verlangt. ⚑ **Für ein
Expertengemisch bleibt der MLP-Teil leer**, benannt im Modulkopf:
`moe_backward` verlangt Expertenwahl, Gewichte und Expertenausgaben, und
das ist Phase 5.

### Die Gegenproben, und zwei haben den Test verbessert

| Schnitt | zuerst | danach |
|---|---|---|
| Punktzahlen statt Wahrscheinlichkeiten | rot (Softmax-Invariante) | rot |
| Produkt statt Gate | **grün** | rot |
| Gate und Up vertauscht | **grün** | rot |
| Produkt verfälscht | rot | rot |

⚑ **Die beiden grünen hatten dieselbe Ursache:** Gate, Up und das
Produkt sind gleich breit und vorzeichensymmetrisch, also fängt keine
Längen- oder Vorzeichenprüfung eine Vertauschung. **Nichts an ihrer Form
unterscheidet sie, nur ihre Rolle in `h = silu(gate) · up`**, und die
ist nicht symmetrisch. Der Test rechnet `h` jetzt aus Gate und Up nach,
mit dem Silu-Vorrat und den Skalen der Ebene; das ist genau die
Rechnung, die der Rückwärtspass rückwärts geht.

### v0.31.0 – 2026-09-04 (runtime 0.23.0: der Vorwärtspass kann mitschreiben)

Neues Modul `runtime::mitschnitt` und ein zweiter **Eingang**
`run_layers_mit_mitschnitt`, der durch dieselbe Schleife und dieselbe
`forward_layer` läuft wie `run_layers`.

⚑ **Ein Pfad, kein zweiter.** Der Rückwärtspass braucht die Eingänge
jeder Rechenstufe, und die Laufzeit war auf Inferenz zugeschnitten und
behielt nichts. Ein zweiter Vorwärtspass, der alles behält, wäre eine
zweite Wahrheit über den Rechenpfad gewesen: **Genau dieser Pfad ist
über dreissig Konformitätsvektoren als bitgleich belegt**, ein zweiter
trüge keine.

`forward_layer` bekommt ein `Option<&mut Zwischenwerte>`. Inferenz gibt
`None` und zahlt je Aufnahmestelle einen `is_some`, keine Kopie. Das
Muster stand schon da: Für die MoE-Diagnose lief bereits ein
`Option<&mut Vec<Routingbefund>>` durch dieselbe Funktion.

**Sechs Werte je Ebene**, nämlich die, die `forward_layer` selbst sieht:
Residualeingang, normierter Eingang, q/k/v nach RoPE,
Aufmerksamkeitsausgabe, Residualstand nach der Aufmerksamkeit, zweiter
normierter Strom.

⚑ **Drei fehlen, und sie fehlen benannt:** die
Aufmerksamkeitswahrscheinlichkeiten (in `attention_int`) sowie Gate-,
Up- und Produktausgabe (in `mlp_int`). Beide Kerne geben nur ihre
Ausgabe zurück; sie durchzureichen gehört zum Schritt, der den
Rückwärtspass einer ganzen Ebene baut. **Die Grenze steht im Modulkopf,
damit niemand den Mitschnitt für vollständig hält.**

Zwei Tests, beide gegenprobiert: **mit Mitschnitt kommt bitgleich
dasselbe heraus**, und die Ebenen hängen aneinander (was eine ausgibt,
sieht die nächste als Residualeingang).

### v0.30.0 – 2026-09-01 (der Trainingsschritt bekommt einen Aufrufer, Punkt 16)

**`backward` und `optimierer` hatten null Aufrufer.** Die Verdrahtung
zur Schleife stand als offener Punkt fest, war also bekannt und kein
Fund. ⚑ **Aber es ist dieselbe Lage, und sie hat dieselbe
Folge:** Einzeln geprüfte Teile sagen nichts über ihr Zusammenspiel.

`trainingsschritt.rs` schließt den Kreis für eine lineare Ebene:
vorwärts, Verlustgradient, rückwärts, fortschreiben. ⚑ **Und die Brücke,
die fehlte, war `gewicht_aus_master`:** Der Optimierer rechnet auf
`Master`, der Vorwärtspass will `i8` mit einer Skala je Zeile, und diese
Umrechnung gab es nirgends. Ohne sie kann man fortschreiben **oder**
rechnen, nicht beides.

📌 **Der Test hat auf dem Weg zweimal zugeschlagen, und beide Male lag es
an einer Skala zwischen zwei richtigen Kernen.**

Zuerst blieb der Abstand **exakt stehen**: Die Testdaten schoben den
Master um `FEIN_BITS` nach links, `gewicht_aus_master` schob ihn um
sechzehn Stellen zurück, und ein Schritt von wenigen hundert war danach
unsichtbar. **`FEIN_BITS` liegen unterhalb der Rasterstufe und gehören in
den Schritt, nicht in die Darstellung.**

Dann stieg der Abstand von 64 214 auf 80 089: Die Lernrate `1/4` sprang
über das Ziel. Sie ist jetzt **gerechnet**: Der Schritt ist `g·lr/nenner`
in Rasterstufen, mit Gradienten um zweihundert und einem Ziel von einer
Fünftel Stufe folgt `nenner ≈ 1000`.

⚑ **Genau das kann kein Golden-Vektor zeigen.** Jeder Kern war für sich
richtig; falsch war, was zwischen zweien liegt.

**Was noch nicht steht:** die Schleife über ein ganzes Netz. Sie braucht
einen Vorwärtspass, der seine Zwischenwerte behält, und der Vorwärtspass
der Laufzeit ist auf Inferenz zugeschnitten und behält nichts.

### v0.29.0 – 2026-09-01 (`docs/` entfällt; die Lizenz zieht zu den Modellen)

Festlegung des Projektinhabers. Von den drei Dateien in `docs/` waren
zwei überholt und eine gehörte woanders hin.

⚑ **Eine Messung stand an zwei Orten, und der zweite war alt.**
`docs/02_empirischer_beleg_…` führte für Entscheidungspunkt 12.21
**θ_v 0.10.0 und +4,29 %**, während `eval/results/decision_12-21.md` bei
jedem Messlauf neu geschrieben wird und **θ_v 0.17.0 mit +2,11 %**
ausweist. Derselbe veraltete Wert stand auch im `verification_report.md`.
**Wer eine Messung an zwei Orten führt, pflegt irgendwann nur noch
einen**, und der andere wird zufällig zuerst gelesen. Beide Verweise
zeigen jetzt auf das Protokoll, das die Zahl erzeugt.

`docs/00_requirements.md` nannte zwei eigene Punkte selbst überholt; was
darin trug, steht im Glossar, in dieser README und in der
Gleitkomma-Prüfung.

⚑ **`docs/01_licenses.md` war nicht überholt und ist deshalb nicht
entfallen, sondern umgezogen** nach `ETHICS/Lizenzlage.md`. Sie ist der
menschliche, variantenscharfe Teil, den `lizenzprobe.py` im eigenen Kopf
ausdrücklich nicht leistet: „Sie liest Dateien, nicht Recht." An ihr
hängen der Ausschluss von Qwen2.5-3B und -72B und damit die Antwort
darauf, welche Größe als Nächstes kommt.

**Neu: eine Lizenzdatei je empfohlenem Modell.** Wer die Gewichte per
Skript direkt von der Quelle holt, erfuhr die Bedingungen bisher erst
**danach**. Künftig trägt jedes für den Produktionsbetrieb empfohlene
Modell sein Verzeichnis samt Lizenzdatei im Repositorium; die Gewichte
bleiben draußen. `models/.gitignore` lässt genau diese Datei durch, und
die vier Lizenzdateien der katalogisierten Modelle liegen jetzt darin;
alle vier sind bytegleich derselbe unveränderte Apache-2.0-Text.

**Die Regel: Wer im `KATALOG.json` steht, bringt seine Lizenzdatei mit.**
⚑ Damit prüft `ETHICS/werkzeuge/lizenzprobe.py` in der CI erstmals
etwas: Sie fand dort bisher einen leeren Ordner und ging durch.

### v0.28.1 (kernels 0.29.1) – 2026-08-30 (⚑ Fund 104: der Paritätstest lief in keinem CI-Job)

⚑ **Fund 104.** Alle kernels-Schritte der CI riefen `cargo test --lib`,
und `--lib` lässt `tests/` vollständig aus. Betroffen waren beide
Dateien dort, und es sind nicht die unwichtigsten:

| Datei | Was sie leistet |
|---|---|
| `tests/test_backend_parity.rs` | nennt sich im eigenen Kopf „die normative Garantie dafür, dass kein Backend jemals numerisch von der Referenz abweicht" |
| `tests/allaussagen.rs` | die Eigenschaftstests vom 2026-08-29, mit erschöpfendem Durchgang |

**Die Paritätsprüfung ist genau die, die Fund 103 gefunden hätte.** Sie
war geschrieben, sie war richtig, und sie wurde nie gerufen. Behoben
durch Weglassen von `--lib` in beiden Schritten. Eine Nachbarschaftsprobe
über alle Crates mit `tests/` ergab genau einen weiteren Fall,
`myl-pod`, und der ist **ausdrücklich und begründet** ausgenommen
(`layer_granular.rs` und `pod_e2e.rs` brauchen Artefakte).

📌 **Und sie hätte die Rechenabweichung trotzdem nicht gesehen.**
`rope_parity_basic` rechnet mit `q = 100` und `k = 50`; nach dem
Rechtsshift liegt jedes Zwischenergebnis weit im i16-Bereich, und dort
stimmen Abschneiden und Sättigen überein. Neu ist deshalb
`rope_parity_saettigung`, das die Werte so wählt, dass es überläuft, und
**zuerst prüft, dass sein eigener Fall wirklich sättigt**. Gegenprobe:
`vqmovn_s32` versuchsweise durch `vmovn_s32` ersetzt, dann fällt genau
dieser eine Test und die sechs alten bleiben grün.

📌 **Zwei weitere Berichtigungen an derselben Datei.** Ihr Kopf behauptete,
auf ARM64 werde „der Fallback-Pfad (der identisch zur Referenz ist)"
geprüft; tatsächlich liefert `SimdBackend::detect()` dort NEON. Und
sechsmal stand `None => return` ohne Ausgabe: Auf einer x86_64-Maschine
ohne AVX2 lief die Datei vollständig durch und meldete sechs bestandene
Tests, ohne eine einzige Zusicherung zu prüfen. **Ein stiller Übersprung
sieht aus wie ein bestandener Test.**

📌 **Berichtigung zur Reichweite von Fund 103.** Der Eintrag darunter
sagt, der AVX2-Pfad stürze „auf den meisten x86-CPUs" ab, und lässt
offen, wen es trifft. Genauer: `backends/simd.rs` ist über das
`Backend`-Trait erreichbar, und das ruft im Rechenpfad **niemand**;
`runtime/src/model.rs` importiert die Referenzkernel direkt. Getroffen
hätte es also `test_backend_parity.rs` auf einer Maschine ohne AVX-512,
nicht einen laufenden Miner. Der Fehler war echt, seine Reichweite war
kleiner als gemeldet.

### v0.28.0 (kernels 0.29.0) – 2026-08-30 (⚑ Fund 103: der AVX2-Pfad stürzt auf den meisten x86-CPUs ab)

> 📌 **Zur Reichweite berichtigt am 2026-08-30, siehe v0.28.1:** Der Pfad
> liegt hinter dem ungenutzten `Backend`-Trait. Betroffen war der
> Paritätstest, nicht der Rechenpfad eines Knotens.


`rotate_half_split_avx2` ist mit `#[target_feature(enable = "avx2")]`
ausgezeichnet, und die Auswahl prüft `is_x86_feature_detected!("avx2")`.
Darin stand `_mm256_cvtepi32_epi16`, also `VPMOVDW`, und der verlangt
**AVX512VL**.

**Auf jeder CPU mit AVX2 ohne AVX-512 ist das eine ungültige
Anweisung**: alle AMD Zen 1 bis 3, alle Intel vor Skylake-X und alle
Intel-Endkundenmodelle seit Alder Lake. Also auf den gewöhnlichen
Rechnern, die dieses Netz gerade einladen will.

### Und derselbe Befehl war zugleich die falsche Rechnung

`VPMOVDW` **schneidet ab**. Die Referenz `rotate_half_split_i16` benutzt
`clamp_i16`, also **Sättigung**. Sobald ein Zwischenwert den i16-Bereich
verließ, rechneten Skalarpfad und SIMD-Pfad verschieden, und die
Bitgleichheit ist die Zusage, auf der das ganze Protokoll steht.

Ersetzt durch `_mm256_packs_epi32` samt Spurentnahme, alles AVX2
beziehungsweise SSE2. Die Reihenfolge der Spuren ist vorher symbolisch
nachgerechnet worden, weil sich x86-SIMD auf der aarch64-Maschine nicht
ausführen lässt; übersetzt und gegen die Mindestfassung geprüft wurde
über `--target x86_64-apple-darwin`.

### ⚑ Warum es nie auffiel, und was es gefunden hat

Der CI-Runner hat AVX-512, dort läuft der Befehl. Die
Entwicklungsmaschine ist aarch64, dort läuft der Pfad gar nicht.
Gefunden hat es der **MSRV-Job bei seinem allerersten Lauf**, weil
`_mm256_cvtepi32_epi16` erst seit Rust 1.89 stabil ist und die
Mindestfassung seit demselben Tag überhaupt angegeben wird. Ein
Werkzeug, das eine Versionsangabe prüft, hat einen Absturz gefunden.

Ein neuer Test hält den Fall fest, und er wählt die Werte so, dass sie
**überlaufen**: Mit kleinen Zahlen stimmen Abschneiden und Sättigen
überein, und genau deshalb ist es jahrelang durchgegangen. Er prüft
zuerst, dass der Überlauf wirklich eintritt, sonst sagte er nichts.

### v0.27.0 (kernels 0.28.2, runtime 0.22.1, pipeline 0.15.0) – 2026-08-29 (⚑ Fund 80: `deploy/` entfernt, die Mindestfassung geprüft)

### v0.26.1 (kernels 0.28.1) – 2026-08-29 (⚑ Fund 95: sqrt_q sättigt still, und Fund 75 hat es übersehen)

Eigenschaftstests für `rshift_round` und `sqrt_q`, erschöpfend um null,
an jeder Rundungsgrenze und über den ganzen Wertebereich.

⚑ **Der erste Lauf fand eine Vorbedingung, die Fund 75 übersehen
hatte.** `sqrt_q` liefert still `i32::MAX`, sobald die Wurzel nicht mehr
in `i32` passt: `sqrt_q(1_764_347_202, 32)` ergibt `2_147_483_647`,
richtig wären rund `2_753_000_000`. Bei `frac_bits = 32` liegt die
Grenze schon bei `1_073_741_823`.

**Fund 75 hat acht Vorbedingungen des Ganzzahlpfades aufgeschrieben und
diese nicht gefunden**, weil sie durch **Lesen** gesucht wurden. Ein
Generator fand sie im ersten Lauf. Jetzt als `debug_assert!`
dokumentiert, mit Gegenprobe.

⚑ **Und der erste Fehlschlag traf den Maßstab, nicht die Sache.** Meine
Referenz für `rshift_round` behandelte `shift == 0` falsch und hätte
jede ungerade Zahl gehoben. **Wer das nicht prüft, „behebt" einen
richtigen Code** — der zweithäufigste Weg, mit einem Eigenschaftstest
Schaden anzurichten.

### v0.26.0 (kernels 0.28.0) – 2026-08-29 (der Trainingsschritt, und der Würfel ist eine Funktion)

**Der Optimiererschritt in Ganzzahlen** (`optimierer.rs`), also das
Stück, das die Messung vom 22. August ausdrücklich verlangt hatte und
das bis heute nur in Python vorlag.

### ⚑ Warum stochastisch gerundet wird

Ein SGD-Schritt bewegt ein Gewicht im Median um **6,4e-6 einer
Rasterstufe**. Wer zur nächsten Stufe rundet, bekommt entweder **nichts**
oder einen **ganzen Sprung**, und beides ist falsch: Die kleinen
Bewegungen, aus denen Lernen besteht, verschwinden. Gemessen an
Qwen2.5-0,5B: **+29,9 %** mit Rundung zur nächsten Stufe, **+0,67 %**
mit stochastischem Runden. **Eine einzige geänderte Zeile dreht das
Ergebnis**, also steht sie mit dieser Begründung da.

### ⚑ Der Würfel ist eine Funktion, kein Zustand

Naheliegend wäre ein Zufallsgenerator, der über die Gewichte läuft. Das
wäre hier aus zwei Gründen falsch, und beide betreffen das Protokoll:

1. **Ein Zustand hängt an der Reihenfolge.** Wer die Gewichte anders
   durchläuft, bekommt andere Zahlen und andere Gewichte. **Zwei
   ehrliche Miner mit verschiedener Aufteilung kämen zu verschiedenen
   Ergebnissen**, und der Redundanzvergleich meldete beide als
   fehlerhaft.
2. **Ein Zustand müsste übertragen werden** und wäre damit Teil des
   Konsensvertrags.

Der Würfel ist deshalb eine reine Funktion aus **(Ebene, Schritt,
Index)**. Das ist derselbe Gedanke wie die Assoziativität der
Ganzzahladdition, auf die das ganze Projekt gebaut ist: **Kein Ergebnis
darf von der Reihenfolge abhängen.**

⚑ **Und daraus folgte eine Schnittstellenänderung, die beim Testen
auffiel.** Der erste Entwurf leitete den Index aus der Position im
übergebenen Stück ab. Dann bekäme dasselbe Gewicht je nach Zuschnitt
einen anderen Wurf — **und ein Netz, das Arbeit aufteilt, teilt Ebenen
auf.** Der Schritt nimmt jetzt einen Index-Versatz; ein Test führt vor,
dass vier Stücke in umgekehrter Reihenfolge dasselbe ergeben wie ein
Zug, **und dass es ohne den Versatz abwiche.**

### ⚑ Fund 92: Vier Kernel-Dateien standen in keiner Prüfliste

Beim Eintragen der neuen Datei fiel auf, dass `backward.rs`, `dot.rs`,
`rechenpfad.rs` und `konformitaet.rs` nicht im Gleitkomma-Audit standen.
**`backward.rs` ist der ganze Rückwärtspass**, also genau der Pfad,
dessen Ganzzahligkeit die Trainingsthese trägt, und der Lauf meldete
trotzdem „null Treffer".

**Dritter Fall derselben Klasse** nach Fund 44 und Fund 84. Behoben
nicht durch vier Zeilen, sondern durch die Ausweitung der
Vollständigkeitsprüfung auf `kernels/src` und `runtime/src`.

⚑ **Dabei kam eine begründete Ausnahme zutage statt einer Lücke:**
`loader.rs` hält `f64`, aber ausschließlich um zu prüfen, ob die im
Artefakt angegebene `scale` zu ihrem `shift` passt. Gerechnet wird mit
`shift`. **Die Datei steht jetzt als benannte Ausnahme da statt zu
fehlen** — eine ungelistete Datei ist unsichtbar, eine gelistete
Ausnahme ist bestreitbar.

**Und dahinter steckt ein Befund, der nicht dort zu beheben ist:**
`scale` ist aus `shift` ableitbar, also **eine zweite Quelle für
dieselbe Aussage**, und genau deshalb muss der Loader beide
gegeneinander prüfen. Es zu entfernen hieße, das Artefaktformat zu
ändern und alle Artefakte neu zu bauen.

**Acht neue Tests.**

### v0.25.0 (kernels 0.27.0) – 2026-08-28 (die letzten beiden MoE-Punkte, und sie hatten dieselbe Lösung)

**Der zweite absorbierende Zustand, und er ist stiller als der erste.**
Ein Experte, dessen Logit so weit unter den übrigen liegt, dass er nie
in die Top-k kommt, wird nie gerechnet, bekommt nie einen Gradienten und
ändert sich nie. **Er ist tot, ohne dass irgendeine Zahl davon
abweicht.**

⚑ **Beim Nachdenken über die beiden offenen Punkte stellte sich heraus,
dass sie dieselbe Lösung haben.** Expertenwachstum scheiterte daran, dass
der einzige exakt funktionserhaltende Weg einen toten Experten hinterlässt.
Lastausgleich scheiterte daran, dass die üblichen Verfahren über den
Batch mitteln. **Ein Hungerzähler löst beides**, denn ein neu
eingehängter Experte ist nichts anderes als ein Experte, der lange nicht
gewählt wurde.

### `Expertenwacht`: zählen statt mitteln

⚑ **Der Unterschied, auf den es ankommt:** Die Batch-Zusammensetzung
wählt der Miner, sie ist willkürlich, und zwei ehrliche Miner können
verschieden batchen. Die **Segmentfolge** legt das Protokoll fest, und
zwei redundante Miner sehen dieselbe in derselben Reihenfolge. Ein
Zähler über die Segmentfolge ist damit so deterministisch wie die
Gewichte selbst und gehört wie sie in den Trainingszustand.

Wer zu lange nicht gewählt wurde, bekommt einen Schub nach oben; die
Gegenbuchung verteilt sich auf die übrigen nach der Hausregel „abrunden,
Rest an einen benannten Empfänger". **Die Summe ist exakt null**, der
Logit-Mittelwert bleibt, wo er war.

**Zwei Randfälle mit Test:** Hungert niemand, ist der Schub überall null.
Hungern **alle**, ebenfalls, denn dann gäbe es niemanden zum
Gegenbuchen und die Summe wäre nicht mehr null.

### `experte_einhaengen`: exakt funktionserhaltend, und nicht mehr tot

Der neue Experte bekommt ein Logit unter allen anderen und wird deshalb
nie gewählt. Dieselben gewählten Experten, dieselben Gewichte, dieselben
Bytes: **die Ausgabe ändert sich um exakt nichts.**

⚑ **Und genau deshalb war dieser Weg bis heute wertlos.** Wer nie
gewählt wird, bekommt nie einen Gradienten und bleibt für immer eine
tote Kopie. Erst der Hungerzähler holt ihn zurück. Der Test fährt das
durch, **mit Gegenprobe**: Ohne die Wacht bleibt derselbe Experte über
500 Schritte draußen. Ohne diese zweite Hälfte bewiese der erste Teil
nur, dass irgendwann irgendetwas passiert.

**Warum der Aufteilungstrick der dichten Schichten hier nicht trägt:**
Dort ist die Ausgabe eine Summe über **alle** Einheiten, halbierte
Kopien summieren sich also zum Original. Beim Routing ist sie eine Summe
über die **ausgewählten**, und zwei Kopien mit gleichem Logit verdrängen
einen dritten aus der Top-k.

### Damit ist die Trainingsseite des Mixture-of-Experts-Modells vollständig

Rückwärtspass durch Router und Experten, Sättigungsschutz,
Expertenwachstum, Lastausgleich. Alle vier ganzzahlig, alle vier
deterministisch, alle vier ohne Rauschen und ohne Batch-Statistik.
**Der Vorwärtspfad ist unberührt geblieben**, θ_v steht weiter auf
0.17.0, und die Inferenz rechnet bitgleich wie vorher.

### v0.24.0 (kernels 0.26.0) – 2026-08-28 (Fund 79 stabilisiert: der Router kommt aus der Sättigung heraus)

**Die Spreizungsstrafe schließt den absorbierenden Zustand.** Sie liest
die **Logit-Abstände** statt der quantisierten Gewichte, und darin liegt
ihre entscheidende Eigenschaft: **Sie hat ihren größten Wert genau dort,
wo der Softmax-Gradient verschwunden ist.** Ob `p_i` auf null gerundet
wurde, ist ihr gleichgültig; sie sieht, dass `z_i` zu weit unten liegt,
und schiebt es zurück.

```text
ueberschuss_i = max(0, (z_max − z_i) − schwelle)
dz_i          = + (ueberschuss_i >> daempfung)     für die Verlierer
dz_max        = − Σ_i (ueberschuss_i >> daempfung) als Gegenbuchung
```

**Drei Eigenschaften, jede mit einem Test:**

- ⚑ **Ein gesunder Router bleibt exakt unberührt.** Nicht „kaum",
  sondern null an jeder Stelle. Eine Strafe, die im Normalfall etwas
  tut, verschiebt das Modell dauerhaft, und niemand sähe woran.
- ⚑ **Die Summe ist exakt null.** Sie staucht die Spreizung und
  verschiebt den Logit-Mittelwert nicht. Ohne das zöge sie das Routing
  über viele Schritte in eine Richtung.
- ⚑ **Der Ausstieg ist ein Lauf, kein Argument.** Ein gesättigter Router
  bekommt nur die Strafe, Schritt für Schritt, ohne jeden anderen
  Gradienten. Nach endlich vielen Schritten sättigt er nicht mehr, und
  der Softmax-Gradient lebt wieder. Der Test fährt genau das.

### Die Schwelle ist hergeleitet, nicht geraten

`saettigungsabstand(prob_frac_bits, exp_input_frac_bits)` rechnet
`(frac + 1) · ln2 · 2^exp_input`. Sättigung tritt ein, wenn
`p_min < 2^-(frac+1)`. **Und ein Test glaubt der Formel nicht:** Er baut
eine Tabelle mit den echten Parametern des Projekts
(`exp_input_frac_bits = 8`, `exp_lut_frac_bits = 14`), sucht mit dem
echten `softmax_int` den Abstand, ab dem das kleinere Gewicht auf null
fällt, und vergleicht auf zehn Prozent.

| Aufbau | Abstand bis zur Sättigung |
|---|---|
| `prob_frac_bits = 8` | 6,24 nats |
| `prob_frac_bits = 14` (θ_v 0.16.0) | **10,40 nats** |
| `f32` | 104 nats |
| `f64` | 745 nats |

⚑ **Berichtigung zur vorigen Fassung:** Dort stand, es gebe diesen
Zustand in Gleitkomma nicht. Das war zu stark. Es gibt ihn auch dort,
nur rund **zehnmal später als im Ganzzahlpfad**. Router-Kollaps ist ein
bekanntes Problem von Mixture-of-Experts-Modellen; die Tabelle macht ihn nur
leichter erreichbar.

**Dieselbe Mechanik hat das Projekt schon einmal getroffen:** Fund 29
hob `prob_frac_bits` in der Attention von 8 auf 14, weil jede Position
unter `1/512` einzeln auf null rundete und die Aufmerksamkeit auf die
Spitzenposition kollabierte. Der Router hatte dieselbe Krankheit an
anderer Stelle, und diesmal reicht die Auflösung allein nicht: Bei 128
Experten und Top-8 sind zehn nats kein Randfall.

### Warum keines der üblichen Verfahren

- **Hilfsverlust über Batch-Statistiken** (Switch Transformer, GShard):
  Das Ergebnis an Position *i* hinge davon ab, welche anderen Token
  zufällig danebenlagen. Dieselbe Klasse wie das für den Vorwärtspfad
  bereits verbotene Token-Dropping.
- **Rauschen im Router:** nicht deterministisch, und ohne Determinismus
  keine Redundanzprüfung.
- **`z`-Verlust** (ST-MoE): Sein Gradient ist je Token lokal und damit
  grundsätzlich brauchbar, aber er staucht **alle** Logits gegen null,
  gleichgültig ob der Router gesund ist, und er braucht `logsumexp`,
  also einen Logarithmus im Ganzzahlpfad. Die Spreizungsstrafe erreicht
  dasselbe Ziel mit weniger Eingriff und ohne neue Primitive.

### Was ausdrücklich offen bleibt

**Lastausgleich.** Ein Experte, der über viele Token nie gewählt wird,
bleibt untrainiert. Das ist eine Aussage über die **Segmentfolge** und
nicht über ein Token, und die Spreizungsstrafe kann sie nicht treffen:
Sie schaut nur auf die gewählten Logits. Ein Ausgleich über die Zeit
statt über den Batch bliebe deterministisch und wäre der nächste
Schritt.

**Und der Vorwärtspfad ist unberührt.** Die Strafe wirkt nur im
Trainingsschritt; die Inferenz rechnet bitgleich wie vorher, θ_v bleibt
0.17.0. Ein importiertes Modell mit bereits gesättigten Routern lässt
sich damit trotzdem befreien, denn die Strafe hängt nicht an den
Gewichten, sondern an den Abständen.

### v0.23.0 (kernels 0.25.0) – 2026-08-28 (Rückwärtspass durch das Mixture-of-Experts-Modell, und Fund 79)

**`moe_backward` schließt die letzte Lücke im Rückwärtspass**, die noch
offen war: Bis hierher kannte er lineare Schichten, Softmax, SiLU,
RMSNorm, RoPE, Attention und Embeddings, aber **kein Wort von
Experten** (null Vorkommen in `backward.rs`). Er verteilt jetzt den
eingehenden Gradienten auf die gewählten Experten, führt ihn durch den
Softmax über deren Logits zurück und legt ihn auf die volle Logit-Reihe.

**Was damit belegt ist:** Zwei redundante Miner, die dasselbe
Trainingssegment auf demselben Mixture-of-Experts-Modell rechnen, liefern
**bitgleiche Gradienten, Routing-Entscheidung eingeschlossen**. Der Test
fährt den ganzen Weg zweimal und vergleicht byteweise; ohne die
Routing-Entscheidung im Vergleich bewiese er zu wenig.

⚑ **Nicht gewählte Experten bekommen exakt null**, und das ist keine
Näherung: Bei `norm_topk_prob` läuft der Softmax nur über die gewählten
Logits, die übrigen berühren die Ausgabe nicht. Damit ist auch die Frage
nach dem Expertenwachstum beantwortet, und die Antwort ist unbequem: Ein
neuer Experte, der mit minimalem Logit eingehängt wird, ist exakt
funktionserhaltend **und tot**. Steht als Test da, nicht als Behauptung.

### ⚑ Fund 79: Ein gesättigter Router kann sich nie wieder ändern

**Der Ganzzahl-Softmax sättigt, und Sättigung ist ein absorbierender
Zustand.** Bei einem Logit-Abstand von 80 Einheiten liefert
`softmax_int` bei `frac = 8` die Gewichte `(256, 0)` statt „fast alles"
und „fast nichts". Dann ist der Gradient **jedes** Logits exakt null:

- Für einen Verlierer ist `p_i = 0`, der Faktor ist null.
- Für den Gewinner ist `p_0 = 2^frac`, also wird die Klammer
  `g_0 − Σ_j g_j p_j / 2^frac` null.

**Beide Wege führen auf null.** Ein Router, der einmal sicher genug war,
bleibt es für immer: Sein Gradient verschwindet, bevor er ihn ändern
könnte.

⚑ **Und Gleitkomma hat denselben Zustand, nur viel später.** Die
Schwelle ist `(frac + 1) · ln2`: bei `prob_frac_bits = 14` sind das
**10,4 nats**, bei `f32` **104**, bei `f64` rund **745**. Der
Ganzzahlpfad kollabiert also rund zehnmal früher als `f32`.
Router-Kollaps ist ein bekanntes Problem von Mixture-of-Experts-Modellen und
**kein Erzeugnis dieses Projekts**; die Tabelle macht ihn nur um
Größenordnungen leichter erreichbar. In der ersten Fassung dieses
Absatzes stand „in Gleitkomma gibt es diesen Zustand nicht", und das war
zu stark.

**Die zweite Hälfte des Fundes lässt sich beheben, die erste nicht.**
Auch ohne Sättigung ist der Logit-Gradient klein, er trägt den Faktor
`p·(1−p)`; auf der Skala des Aktivierungsgradienten rundet er auf null,
bevor er wirkt (gemessen: ±0,25 bei Gewichten (250, 6)). `moe_backward`
nimmt deshalb `logit_zusatz_bits` und führt den Router-Gradienten um so
viele Bit feiner. **Gegen die Sättigung hilft das nicht.**

**Was daraus folgt:** Ein Lastausgleich ist bei einem Mixture-of-Experts-Modell im
Ganzzahlpfad keine Verbesserung, sondern eine **Voraussetzung**. Er muss
verhindern, dass ein Router sättigt. Die üblichen Verfahren tun das über
einen Hilfsverlust mit Batch-Statistiken und über Rauschen im Router;
Rauschen ist nicht deterministisch, und eine Größe über den Batch machte
das Ergebnis an Position *i* davon abhängig, welche anderen Token
zufällig danebenlagen. **Solange kein Ersatz steht, ist Training auf
einem Mixture-of-Experts-Modell möglich, aber nicht stabil**, und dieser Satz
gehört zu jedem Ergebnis dazu.

**Sieben neue Tests**, darunter die Bitgleichheit über zwei Läufe, der
Vergleich gegen die numerische Ableitung des echten Mischkernels, die
Sättigung mit null Gradient bei 0, 3 und 6 Zusatzbits, und die
Gegenprobe, dass ohne Sättigung die Zusatzbits wirklich mehr
durchlassen.

### v0.22.0 (kernels 0.24.0) – 2026-08-28 (Fund 75: die Schiebeweiten hatten Grenzen, und sie standen nirgends)

**Kein Fehler, sondern ein Vertrag, den niemand aufgeschrieben hatte.**
Die Rundungs- und Reskalierungsfunktionen in `fixed_point.rs` sind nicht
für jede Schiebeweite total: `rshift_round` rechnet `(1 << shift) - 1`,
und dieser Ausdruck läuft über, sobald `1 << shift` das Vorzeichenbit
trifft. Die Grenzen liegen bei 30, 62 und 126 Bit je nach Typ, für
`rescale` bei einem Abstand von −31 bis 30. Keine davon stand irgendwo,
keine wurde geprüft.

⚑ **Der Fehlerfall ist im ausgelieferten Bauprofil still.** Im
Debug-Bau bricht die Überlaufprüfung laut ab. Im Release-Bau gibt es sie
nicht: `rshift_round(1000, 32)` liefert dort **1001 statt 0**, weil Rust
die Schiebeweite auf fünf Bit maskiert und die Rundung anschließend
aufaddiert. Ein falscher Wert im Ganzzahlpfad ist ein Konsensbruch, und
er fällt nirgends auf.

⚑ **Der schlimmste Fall bricht in keinem der beiden Bauprofile ab.**
`sqrt_q(i32::MAX, 33)` liefert `0`. Der Linksschieber lässt die oberen
Bits fallen, ohne dass die Überlaufprüfung anspringt, denn sie prüft die
Schiebe*weite*, nicht den Wert. Zwischen `frac_bits` 33 und 63 liegt ein
Bereich, in dem eine Wurzelfunktion still Null zurückgibt.

**Kein Aufrufer verletzte eine der Grenzen.** Die im Projekt
vorkommenden `frac_bits` liegen zwischen 3 und 16, und alle 121
Kernel-Tests sowie die 55 Runtime-Tests laufen mit den neuen Prüfungen
unverändert durch, einschließlich des Konformitätslaufs gegen ein echtes
Artefakt. Deshalb steht hier auch keine Verhaltensänderung: Die
Prüfungen sind `debug_assert!` und im Release-Bau nicht vorhanden, die
Ausgabe bleibt bitgleich.

**Zwei Funktionen hatten weder Test noch Aufrufer.** `sqrt_q` und
`rsqrt_q` in `integer_math.rs` werden von nichts im Repositorium
gerufen; das Modul hatte **null Tests**. Im Betrieb genutzt werden
`fixed_point::inv_sqrt_q15` und `isqrt_round`, es gab also drei
Ganzzahlwurzeln in einem Crate, von denen zwei tot waren. Entfernt
werden sie nicht, das Löschen öffentlicher Schnittstellen ist eine
Entscheidung; sie bekommen Vorbedingung, Prüfung und Test, solange sie
öffentlich sind.

⚑ **Ein Modulkopf versprach mehr, als der Code hielt.** `mlp.rs` sagt,
große Beträge „saturieren deterministisch am LUT-Rand". Gesättigt wird
aber erst **in** `lut_lookup`; davor steht ein ungesichertes
`g_dom as i16`, das abschneidet statt zu sättigen. Aus einem sehr großen
positiven Gate-Wert würde damit ein negativer Index, der mitten in der
Tabelle landet und völlig gültig aussieht. Es trägt heute, weil die
kalibrierten `gate_proj`-Skalen über alle vier Modelle zwischen 7 und 13
liegen und `silu.input_frac_bits` bei 6, der Reskalierer also immer
verkleinert.

⚑ **Die erste Fassung der Prüfung dafür war zu streng, und die
Bestandstests haben sie gefangen.** Geprüft wurde
`gate_out_frac >= silu_in_frac`. Das ist **hinreichend, nicht
notwendig**: Ein kleiner Gate-Wert mit mäßigem Linksschieber passt
ebenso in `i16`, und genau so arbeiten die synthetischen
Prüfvorrichtungen des Laders (`gate_out_frac` 4 gegen `silu_in_frac` 6).
Zwei Ladertests fielen sofort durch, obwohl an ihnen nichts falsch ist.
Geprüft wird jetzt die **notwendige** Bedingung, nämlich der Wert
selbst: der reskalierte Gate-Wert muss in `i16` liegen. Eine zu enge
Prüfung erzeugt Druck, sie wegzunehmen, statt den Fehler zu finden, und
ist damit dieselbe Falle wie ein Test, der ein Literal statt der Regel
prüft.

⚑ **Beim Schreiben der Gegenprobe fiel der Rest davon auf: Sättigen auf
`i16` rettet nicht.** Der Wert wird danach noch um den LUT-Offset
verschoben, und `32767 + 256` verlässt `i16` erneut. Damit ist auch der
Schutz in `backward.rs` (`clamp_i16_sat`, mit der ausdrücklichen
Begründung, der Index dürfe nicht wrappen) unvollständig. Die einzige
richtige Sättigung ist die **in die LUT-Domäne**, auf
`[-offset, len − 1 − offset]`. Wer das je behebt, behebt es an beiden
Stellen; ein `clamp` im Rechenpfad müsste in allen vier Backends gleich
eingebaut werden, sonst bricht die Bitgleichheit.

⚑ **Fund 78 nebenbei, in der Pipeline:** Zwei Testfunktionen in
`manifest.rs` teilten sich denselben festen Temp-Ordner
(`myelith-pipeline-tests`, ohne Prozesskennung) und löschten ihn beim
Betreten. Zwei gleichzeitige Testläufe räumen einander damit ab. Der
Rest des Projekts hängt `std::process::id()` an; hier fehlte es.
Behoben, zusammen mit neun gleichartigen Stellen im Testclient.

**Neu: 20 Tests** (9 in `fixed_point.rs`, 9 in `integer_math.rs`, 2 in
`mlp.rs`). Je Grenze zwei, und der Aufbau ist Absicht: einer zeigt, dass
der letzte zulässige Wert durchgeht, der andere, dass der erste
unzulässige abbricht. Nur eine Richtung zu prüfen hieße, eine zu enge
Schranke nicht zu bemerken. Dazu prüft
`rshift_round_rundet_zur_geraden_zahl_auch_negativ` die Rundungsregel
über den Bereich −600 bis 600 für acht Schiebeweiten gegen eine
unabhängig in `i64` gerechnete Referenz, statt an vier getippten Paaren.

### v0.21.0 (kernels 0.23.0, runtime 0.22.0) – 2026-08-27 (Die Konformitätsprüfung wird eine Bibliothek)

**Außerplanmäßig, aus dem Bedarf des Testclients.** Die Prüfung der
Golden Vectors steckte vollständig in den beiden Binaries
`kernels/src/bin/golden_runner.rs` und `runtime/src/bin/golden_model.rs`.
Aufrufen konnte sie damit nur, wer die Binaries baut und ihren Pfad
kennt; ein Werkzeug, das einen Konformitätslauf protokollieren will,
hätte ein zweites Programm starten und dessen Terminalausgabe lesen
müssen. Die Logik liegt jetzt in `kernels/src/konformitaet.rs` und
`runtime/src/konformitaet.rs`, die Binaries sind dünne Starter darüber
geblieben: **eine Quelle, keine zweite Wahrheit.** `conformance/run.sh`
läuft unverändert weiter und meldet dieselben 33/33.

**Ein Gleitkomma-Rückfall ist dabei entfallen.** Ein Vektor ohne
exp-LUT in seinen Metadaten fiel im alten `golden_runner` auf eine
`f64`-Nachbildung zurück. Das war gegen die Ganzzahldisziplin, und es
stand ausgerechnet in einer Datei, die das Gleitkomma-Audit
ausdrücklich **nicht** ansieht (Offline-Werkzeuge sind ausgenommen).
Solche Vektoren schlagen jetzt begründet fehl; alle Vektoren des
Repositoriums tragen die LUT.

⚑ **Und damit der Rückfall nicht zurückkommt, stehen die beiden neuen
Module jetzt im Gleitkomma-Audit** (`tests/audit/test_no_float.py`,
Heißpfad 21 → 23 Dateien). Dieselbe Lücke wie bei `moe.rs`, das als
Rechenpfad-Datei ebenfalls nicht in der Liste stand: **Eine Prüfung,
die eine Datei nicht ansieht, meldet über sie nichts.** Gegenprobe
gefahren — ein eingebautes `f64` wird gefunden und benannt.

**Ein Manifest bei den Vektoren** (`conformance/vectors/manifest.json`)
nennt Modell und θ_v-Hash, gegen die die Layer- und E2E-Vektoren
erzeugt wurden. Ohne diese Angabe liefe ein fremdes Artefakt blind
dagegen: Es „bestünde" nie und „verfehlte" immer, und beides wäre keine
Aussage über den Bau.

### v0.20.0 (kernels 0.21.0) – 2026-08-23 (Zeilen über Threads: 7B wird 5,2-mal schneller)

**Der Integerpfad lief einkernig.** Aufgefallen beim Rechnen der
Wirtschaftlichkeit (K8): Ein Kostenverhältnis von 9,2× gegen einen
zentralen Anbieter sah zu schlecht aus, um an der Numerik zu liegen. Die
Prüfung ergab, dass die bf16-Vergleichsseite fünf Threads benutzt und
unsere Seite einen, auf einer Maschine mit fünfzehn Kernen.

`linear_w8a16` und `linear_w8a16_pc` verteilen ihre Zeilen jetzt über
Threads. **Bitgleich per Konstruktion:** Jede Ausgabezeile ist ein
eigenes Skalarprodukt und schreibt in ihr eigenes Feld; zwischen den
Zeilen gibt es keine gemeinsame Zwischensumme und damit keine
Reihenfolge, die etwas ändern könnte. Dieselbe Eigenschaft wie die
Assoziativität innerhalb der Zeile, nur eine Ebene höher.

| Modell | vorher | nachher | Faktor |
|---|---|---|---|
| 0,5B | 38,19 tok/s | **49,17** | 1,29× |
| 7B | 2,07 tok/s | **10,74** | **5,19×** |

Damit ist der Integerpfad bei 7B **schneller als bf16** auf derselben
Maschine (Faktor 1,09), und das Kostenverhältnis aus K8 fällt von 9,2×
auf 1,9×.

**Belegt, nicht behauptet:** 33/33 Konformitätsvektoren,
`decode_digest` bei 0,5B unverändert, bei 7B derselbe Wert aus zwei
getrennten Prozessen und beiden Backends.

**Der erste Versuch brachte bei 0,5B nichts**, und das war lehrreich. Er
nahm `available_parallelism`, hier fünfzehn. Gemessen
(`src/bin/threads_probe.rs`): Der Start kostet rund `12 µs + 6,3 µs je
Thread`, bei fünfzehn also 107 µs, und die 4864×896-Matrix braucht
einkernig nur 289 µs. Dieselbe Matrix mit vier Threads **2,53×**, mit
acht 2,41×, mit fünfzehn nur noch 1,72×. Bei der größten Matrix des
7B-Modells ist es genau umgekehrt: dort bringen fünfzehn Threads
**7,40×**.

Die Threadzahl folgt deshalb der Arbeitsmenge (`ARBEIT_JE_THREAD`), und
unterhalb einer Schwelle (`PARALLEL_AB`) wird gar nicht aufgeteilt. Beide
Konstanten sind gemessen; die Probe liegt bei und lässt sich auf jeder
Maschine wiederholen.

**Keine neue Abhängigkeit:** `std::thread::scope` genügt, ein Thread-Pool
war nicht nötig. Ein Crate mehr im Konsenspfad wäre der falsche Preis für
diese Ersparnis gewesen.

### v0.19.0 (runtime 0.18.0) – 2026-08-23 (der Digest-Vertrag bekommt einen Ort)

**Nichts an der Rechnung, alles an der Zuständigkeit.** Die Bytefolge des
Dekodier-Digests stand als Schleife in `dekodieren_mit_digest`: je
erzeugtem Token alle Logits als `i32` little-endian, danach der gewählte
Token als `u32`. Solange nur der Einzelknotenlauf sie brauchte, war das
richtig. Der geshardete Lauf braucht denselben Wert, kann diese Schleife
aber nicht benutzen: Seine Logits entstehen verteilt, im Shard mit dem
LM-Head, Schritt für Schritt.

Neu ist deshalb `generate::DekodierDigest` mit `schritt(&logits, token)`,
`schritte()` und `hex()`. `dekodieren_mit_digest` benutzt ihn, und
`myl-pod` benutzt ihn ebenfalls. **Der Grund für diese Sorgfalt ist Fund
34:** Eine zweite Fassung derselben Bytefolge wäre eine zweite Quelle für
dieselbe Aussage, und genau daraus entstehen die Fehler, die dieses
Projekt am teuersten bezahlt hat.

Gehasht wird jetzt **strömend** statt über einen Zwischenpuffer. Der
Puffer wäre bei 0,5B und 32 Token rund 19 MB gewesen, ohne dass ihn
jemand liest.

**Der Wert selbst hat sich nicht bewegt**, und das ist die Bedingung:
33/33 Konformitätsvektoren bestanden, darunter die drei E2E-Vektoren, die
seit v0.16.0 `metadata.logits_sha256` tragen und ohne dieses Feld
abgelehnt werden. Ein geänderter Digest wäre hier kein Fortschritt,
sondern ein Fehler.

**Nachgezogen:** An anderer Stelle standen noch 15,29/+2,3 % und
9,40/+8,29 %. Beide Zahlen sind seit dem 2026-08-20 (Fund 31, θ_v 0.17.0)
überholt; gültig sind **15,27/+2,11 %** und **8,78/+1,14 %**. Dieselbe
Klasse wie die drei Stellen, die am 2026-08-22 nachgezogen wurden.

### v0.18.0 (kernels 0.20.0) – 2026-08-22 (Rückwärtspass vollständig)

Die drei offenen Kernel sind gebaut, die Ableitungs-LUT erzeugt, und der
Konformitäts-Prüflauf deckt den Rückwärtspass mit ab: **33 von 33**
Vektoren statt 30.

| Neu | leitet ab |
|---|---|
| `rope_backward` | `rotate_half_split_i16` |
| `attention_backward` | `attention_int`, für eine Abfrageposition |
| `embedding_backward_akkumulieren` | den Embedding-Nachschlag |
| `luts.py::generate_silu_grad_lut` | die SiLU-Ableitung |

**RoPE braucht keine neue LUT und keinen Vorzeichenwechsel in `sin`.**
Die Jacobi-Matrix einer Drehung ist die Drehmatrix selbst, und die ist
orthogonal: Ihre Transponierte ist die Drehung um −θ. Nur die Vorzeichen
in der Formel wandern. Wer stattdessen `sin` negiert **und** die Formel
unverändert lässt, dreht in die falsche Richtung, und in einer
Zahlenprobe sieht man das kaum; der Test dreht deshalb vorwärts und
rückwärts und verlangt den Ausgangspunkt zurück.

**Maskierte Positionen bekommen im Attention-Rückwärtspass exakt null**,
nicht „ungefähr null" wie vorwärts. Ein Gradient auf eine Position, die
nie gelesen wurde, wäre ein Leck über die Kausalitätsgrenze.

**Der Embedding-Gradient akkumuliert, er setzt nicht.** Kommt ein Token
in einer Sequenz mehrfach vor, muss sich sein Gradient addieren. Wer
zuweist, behält das letzte Vorkommen; das fällt bei seltenen Token nie
auf und bei häufigen als langsames Lernen.

**Die SiLU-Ableitung bekommt eine eigene LUT.** Es liegt nahe,
`σ(x) = silu(x)/x` aus der Vorwärts-LUT zu gewinnen; bei null ist das
undefiniert und in der Umgebung numerisch unbrauchbar, also genau dort,
wo die meisten Aktivierungen liegen. Nachgemessen gegen die numerische
Ableitung der Vorwärts-LUT: Abweichung unter 0,006 über den gesamten
Bereich. Ein Test hält fest, dass die Ableitung über eins überschwingt
(1,10 bei x ≈ 2,36) und links negativ wird (−0,10): Wer den
Ausgangsbereich wie bei SiLU selbst wählt, sättigt genau am Maximum.

**Golden Vectors, und ein Fund an der eigenen Referenz.** Die Sollwerte
entstehen in `tools/golden_backward.py`, einer **unabhängigen**
Nachbildung der Kernelsemantik: Ein Vektor, den der geprüfte Code selbst
erzeugt, prüft nichts. Der erste Lauf meldete prompt eine Abweichung von
1 an einer Stelle. Die Klärung ergab, dass **die Referenz** falsch lag,
nicht der Kernel: Sie rundete vom Nullpunkt weg, der Vertrag verlangt
round-to-nearest-even auf der Zweierkomplement-Darstellung. Der Vorfall
ist der Grund, warum es die Datei gibt; hätte der Kernel seine eigenen
Sollwerte erzeugt, wäre die Frage nie gestellt worden.

**Offen bleibt** der Nachweis, dass zwei Maschinen denselben Gradienten
liefern. Das Werkzeug dafür steht (der Testclient), es fehlt die zweite
Maschine, wie beim Vorwärtspfad auch.

### v0.17.0 (kernels 0.19.0) – 2026-08-22 (Rückwärtspass, erster Teil)

**Warum hier und nicht in TRAINING.** Die Messungen 0.1 und 0.2 haben
gezeigt, dass das Quantisierungsschema im Rückwärtspass trägt und ein
Trainingsschritt ohne Gleitkommazustand möglich ist. Beides nützt nichts,
solange der **Gradient** aus einer Gleitkommarechnung kommt: Er wäre
geräteabhängig, und zwei Miner mit demselben Segment bekämen
verschiedene Ergebnisse. Der Redundanzvergleich meldete dann einen
Betrug, wo nur zwei Prozessoren verschieden gerundet haben.

Neu ist `kernels/src/backward.rs` mit:

| Funktion | leitet ab | Stand |
|---|---|---|
| `quantisiere_block` / `entquantisiere_block` | Übertragungsform int8 je Block (Anhang B.6.2) | ✅ |
| `linear_backward` | `linear_w8a16` | ✅ |
| `softmax_backward` | `softmax_int` | ✅ |
| `silu_backward` | die SiLU-LUT aus `mlp_int` | ✅ (Ableitungs-LUT als Parameter) |
| `rmsnorm_backward` | `rmsnorm_i16` | ✅ |
| `attention` | Zusammensetzung aus linear und softmax | offen |
| `rope` | Drehung um −θ, dieselbe LUT | offen |
| Embedding | Streuaddition | offen |

**Geprüft wird gegen die numerische Ableitung des echten
Vorwärtskernels**, nicht gegen eine nachgerechnete Formel. Eine gegen die
Formel geprüfte Ableitung sagt nur, dass zwei Menschen dieselbe Formel
gelesen haben; sie fällt nicht auf, wenn der Vorwärtspfad etwas anderes
rechnet. Der zentrale Differenzenquotient auf dem echten Kernel fällt
darauf sehr wohl auf, und genau das ist er auch:

**Fund beim Bauen: Fund 24 in der Rückwärtsrichtung.** Die erste Fassung
von `linear_backward` schob jeden Summanden einzeln nach rechts und
addierte danach. Bei kleinen Produkten rundet jeder Summand für sich auf
null, und die Summe ist null: Der Test gegen die numerische Ableitung
fand ein `dL/dx` von exakt 0, wo −2 hingehörte. Dieselbe Stelle steckte
in der Summe von `rmsnorm_backward`.

Behoben wie im Vorwärtspfad: Ausrichtung gegen den **größten** Shift per
Linksshift (dabei geht kein Bit verloren), Akkumulation in `i128`, und
**ein einziger** Rechtsshift ganz am Ende. Es ist derselbe Fehler, den
Fund 24 in der Quadratsumme von `rmsnorm_i16` behoben hat, und er ist mit
zwei Gegenproben festgehalten.

**Was noch fehlt für verifizierbares Training:** die drei offenen Zeilen
oben, Golden Vectors für den Rückwärtspass, und der Nachweis, dass zwei
Maschinen denselben Gradienten liefern. Entwurf und Begründung:
`TRAINING/README/Konzept-Wachstum.md`.

### v0.16.0 (kernels 0.18.0, runtime 0.17.0) – 2026-08-22 (drei Funde am Prüfstand)

**Der Prüfstand prüfte weniger, als er behauptete, an drei Stellen.**
Keine davon betrifft die gerechneten Zahlen; alle betreffen die Aussage
über sie.

**Fund 36 im Prüflauf: Die E2E-Vektoren verglichen Token.** Von den 30
Konformitätsvektoren vergleichen 27 Tensoren, die drei `e2e`-Vektoren
dagegen nur `outputs.tokens`. Ein Token ist ein Argmax über 151 936
Zahlen. Nachgemessen an einem Artefakt, dessen Tensor um 0,0101 % der
Bytes verschoben und dessen Hashkette konsistent nachgezogen war: Die
Token blieben gleich, die Zahlen nicht. Neu trägt jeder E2E-Vektor
`metadata.logits_sha256`, einen SHA-256 über die Logits jedes Schritts
und den gewählten Token; ein Vektor ohne dieses Feld wird abgelehnt statt
schweigend schwächer geprüft. Gegenprobe am manipulierten Modell: Der
Prüflauf meldet jetzt „Die Token stimmen, die gerechneten Zahlen nicht".

Aufschlussreich ist, **wie wenig** dieselbe Manipulation sonst bewegt:
Alle 24 Layer-Vektoren und zwei der drei E2E-Vektoren bestehen weiter.
Eine Verschiebung um je eins verschwindet in den Rundungsschritten,
sofern die Aktivierungen sie nicht gerade über eine Schwelle heben. Die
Vektoren sind Stichproben, keine Modellidentität, und das ist für ihren
Zweck richtig: Zwei Implementierungen desselben Modells weichen
systematisch ab, nicht zufällig.

**Fund 37: Das Feld `hash` trug in zwei Vektorgruppen ein anderes
Format.** `conformance/README.md` sagt zu, die Prüfung laufe über
SHA-256 der gepackten Tensordaten, und für die Op-Vektoren stimmt das.
Die Layer- und E2E-Vektoren aus `golden_generate` trugen dort einen
`DefaultHasher`-Wert über die Rust-Repräsentation, und geprüft hat ihn
niemand: In `golden_model` stand das Feld als toter Code. Beide Gruppen
sind neu erzeugt, die **Zahlen darin sind unverändert** (24 von 24
Layer-Vektoren bitgleich zur eingefrorenen Fassung), und `golden_model`
rechnet die Hashes jetzt nach, bevor ein Vektor als Maßstab dient.

**`bench/run.py` prüfte die Bitgleichheit über alle Backends mit
`decode_hash`**, einem Hash über die Token, erzeugt mit `DefaultHasher`.
Zwei Schwächen in einem Wert: zu grob für die Frage, und
`DefaultHasher` hat keinen festgelegten Algorithmus, darf sich also
zwischen Rust-Fassungen ändern. Neu heißt der Wert `decode_digest` und
kommt aus `generate::dekodieren_mit_digest`.

**Eine Fassung, nicht drei.** Die Bytefolge des Digests steht genau
einmal, in `runtime/src/generate.rs`. `golden_model` und
`myl-testclient::runs::greedy_digest` rufen sie auf, statt sie
nachzubauen; die Kopie im Testclient ist entfallen. Nachgemessen, bevor
sie wich: beide lieferten für denselben Prompt `df54ef6c89f1a840…`. Der
Grund für diese Sorgfalt ist Fund 34 im selben Patch.

### v0.16.0-Teil: Fund 34, ein Rechenpfad, den es nicht gibt

`kernels/src/rechenpfad.rs` entstand am 2026-08-22 gegen Fund 33: Ein
Prüflauf sollte kein Backend zertifizieren, das gar nicht rechnet. Am
selben Tag stellte sich heraus, dass das Modul denselben Fehler enthielt,
den es verhindern sollte.

Die Bedingung für `cpu-simd` stand dort **noch einmal**, als
`any(target_arch = "x86_64", target_arch = "aarch64")`. `dot.rs`
vektorisiert aber nur unter `aarch64`; auf x86_64 gibt es bis heute
keinen AVX2-Pfad, das steht seit v0.13.4 im Modulkopf von `dot.rs`.
Gemessen an derselben Quelle, für zwei Ziele übersetzt, mit
`--features cpu-simd`, 20 000 Durchläufe über 4096 Elemente:

| Ziel | `dot_scalar` | `dot_i8_i16` | Verhältnis |
|---|---|---|---|
| aarch64 (nativ) | 6,97 ms | 2,70 ms | **2,58×** |
| x86_64 (Rosetta) | 15,26 ms | 15,20 ms | **1,00×** |

Auf x86_64 ist `dot_i8_i16` nicht ähnlich schnell wie `dot_scalar`, es
**ist** `dot_scalar`. Gemeldet wurde trotzdem ein zweiter Rechenpfad.

**Der zugehörige Test konnte den Widerspruch nicht finden**, weil er gegen
dieselbe wiederholte Bedingung prüfte wie der Code. Ein Test, der eine
Zusicherung mit ihrer eigenen Formulierung vergleicht, besteht immer.

Behoben, indem die Bedingung nur noch **einmal** vorkommt: am `cfg` von
`dot::gewaehlt`, wo Konstante und Aufruf im selben Zweig stehen und
deshalb nicht auseinanderlaufen können. `rechenpfad::mit_rechenpfad`
liest `dot::VEKTORISIERT`. Die CI testet den Fall jetzt auf einem echten
x86_64-Runner; auf der Entwicklungsmaschine ist er nicht prüfbar.

**Tragweite außerhalb dieses Crates:** `myl-testclient` schrieb
`cpu-simd/avx2` ins Protokoll, sobald `is_x86_feature_detected!` AVX2 auf
der **CPU** fand, also eine Auskunft über den Prozessor statt über den
gerechneten Code. Ein Protokoll von der geplanten Partnermaschine hätte
diesen Namen getragen, und die Testanleitung führte „Referenz + AVX2" als
lohnende Kombination.

**Offen aus Fund 36 (TESTCLIENT v0.8.0, 2026-08-22):** Der Testclient
hashte für den Cross-Hardware-Vergleich nur die erzeugten **Token** und
übersah damit Rechenabweichungen, solange kein Argmax kippte. Dort ist es
behoben. In diesem Crate betrifft dieselbe Frage zwei Stellen, und beide
sind noch offen: die drei `e2e`-Konformitätsvektoren vergleichen
`outputs.tokens` (die 27 Vektoren auf `op`- und `layer`-Ebene vergleichen
Tensoren und sind nicht betroffen), und `bench/run.py` prüft die
Bitgleichheit über alle Backends mit `generate::hash_tokens`. Der
Konsenspfad ist geprüft und **nicht** betroffen:
`myl-verifier::adjudicate` hasht Ausgabe-Aktivierungen.

### v0.15.0 – 2026-08-20 (θ_v 0.16.0/0.17.0: Softmax-Auflösung, Residual-Addition, Skalenpakete)

**Das Akzeptanzkriterium ist erreicht — auf beiden Modellen.**

| Modell | vorher | jetzt | Kriterium ≤ 5 % |
|---|---|---|---|
| Qwen2.5-0,5B | 15,29 (+2,25 %) | **15,27 (+2,11 %)** | erfüllt |
| Qwen2.5-7B | 9,33 (+7,49 %) | **8,78 (+1,14 %)** | erfüllt |

Der unabhängig gemessene Boden des Quantisierungsschemas liegt bei **+0,84 %**;
der verbleibende Umsetzungsverlust beträgt damit **0,30 Punkte**. Zu Beginn der
Fehlersuche 12.77 waren es 6,65.

**Vor der Meldung verifiziert**, weil die Zahl zu gut war, um sie ungeprüft zu
übernehmen: Artefakt trägt θ_v 0.17.0, 7B erzeugt kohärenten Text, Golden
Vectors 30/30, Konformität 30/30 auf beiden Backends, Durchsatz unverändert.
Die stärkste Bestätigung ist die Kohärenz mit dem unabhängig gemessenen
Schema-Boden.

**θ_v 0.17.0 — Fund 31: doppelte Klemmung in der Residual-Addition.**
Beide Residual-Additionen klemmten den eingehenden Residualstrom
**einzeln** auf die Ausgangsskala, bevor der Blockbeitrag addiert wurde.
An einer Auslöschung zerstört das den Wert: Beide Operanden können groß
sein, während nur ihre Summe klein ist — und die Ausgangsskala ist nach
der Summe kalibriert. Gemessen an Ebene 21, Kanal 62 (der Kanal mit der
*massive activation*): wahrer Wert 61,56, unser Wert **−0,002**. Jetzt
wird auf der **gröberen** der beiden Skalen in i64 addiert und **einmal**
reskaliert und geklemmt: **63,998**. Mittlerer Ebenenfehler an Position 0
von 8,56 % auf 4,96 %; Perplexität 0,5B **+2,49 % → +2,11 %**, 7B
**+7,99 % → +1,14 %**. Der Unterschied im Ausmaß hat einen Grund: 7B trägt
3–4 massive Kanäle mit absmax ~9600 gegen ~10 im Rest (Faktor 960) über 28
Ebenen, 0,5B einen mit Faktor 340 über 24 — die doppelte Klemmung schlug dort
entsprechend häufiger und härter zu.

**Skalenpakete (Fund 32).** Der Artefaktbau war nur auf derselben Maschine
reproduzierbar: Die Aktivierungsskalen entstehen in Gleitkomma, und **3 von 314**
Einträgen sitzen innerhalb von 0,01 % einer Zweierpotenz-Grenze. Seit
`scale_packs/` werden Skalen und LUTs versioniert (1,8 MB für beide Modelle);
die verbleibende Gewichtsquantisierung ist `round(W · 2^shift)` und damit exakt.
Der Bau dauert jetzt **3 s statt ~3 min** (0,5B) und **40 s statt ~20 min** (7B).
Geprüft wird er über `myl-test artefakte` gegen einen Digest über alle
Artefaktdateien.

**θ_v 0.16.0 — Softmax-Auflösung.** `exp_input_frac_bits` 4 → 8,
`exp_lut_frac_bits` und `prob_frac_bits` 8 → 14. Auf 128-Token-Sequenzen
nicht messbar, aber bei `prob_frac_bits = 8` rundet jedes Gewicht unter
1/512 einzeln auf null: Ab etwa 512 Positionen verschwindet der gesamte
Schwanz der Aufmerksamkeitsverteilung. Korrektheitsfix für lange Kontexte,
nicht Optimierung.

**Gehärtet:** `tests/golden/validate.py` meldete bei einem eine Ebene zu
tiefen Pfad „0 passed, 0 failed" mit Exit 0 — eine Nullmessung als Erfolg.
Bricht jetzt mit Exit 2 ab, wenn es keine Vektoren findet, und korrigiert
den Pfad selbst.

### v0.14.0 – 2026-08-20 (θ_v 0.15.0: SiLU-Auflösung, GPTQ standardmäßig aus)

**7B: 9,40 → 9,33** (+8,29 % → **+7,49 %** gegen die BF16-Baseline 8,68).
0,5B unverändert bei 15,29 (+2,25 %).

**Die Ursache war die SiLU-LUT, und sie wurde durch einen
Operationsvergleich gefunden, nicht durch Perplexitätsmessung.**
`layer_probe` vergleicht jede Stufe einer Ebene gegen eine
Gleitkomma-Rechnung mit **identischen entquantisierten Gewichten und
identischem Eingang** — die Differenz ist damit reine Arithmetik:

| Stufe | rel. L2 |
|---|---|
| `gate = W_gate·x` | 0,01 % |
| `up = W_up·x` | 0,02 % |
| **`silu(gate)` über LUT** | **6,83 %** |

Die Matrixmultiplikationen sind praktisch exakt; der gesamte MLP-Fehler
entstand in einer Nachschlagetabelle. Zerlegt: 6,68 % aus dem
Eingangsraster (1/8), 1,56 % aus der Ausgangsauflösung (1/64). Der
Ausgang belegte **121 von 32 767** — 8,1 Bit lagen brach.

**θ_v 0.14.0 → 0.15.0:**

| Parameter | vorher | jetzt | Grenze |
|---|---|---|---|
| `silu.input_frac_bits` | 3 | **6** | 7 scheitert an `lut_lookup` (i16-Index) |
| `silu.input_range` | [−1024, 1023] | **[−8192, 8191]** | reale Domäne ±128 bleibt |
| `silu.output_frac_bits` | 6 | **8** | 9 sprengt die LUT-Einträge (65 528) |

Die reale Domäne ±128 ist **nötig**, nicht großzügig: Das kalibrierte
Gate-AbsMax reicht bis 77,0 (7B). Beide Parameter stehen jetzt auf ihrem
implementierbaren Maximum.

**GPTQ läuft standardmäßig nicht mehr mit** (`INTEGER_LLM_GPTQ=1` für die
Auslieferung). Gemessen: **exakt neutral** — 9,40 mit und ohne, auf zwei
Nachkommastellen identisch. Zusammen mit dem alten Befund (3 242 →
3 318) hat GPTQ in keiner gemessenen Konfiguration je genützt, kostet
bei 7B aber 2,5 Stunden statt 20 Minuten.

**Der Vergleich war nur deshalb aussagekräftig, weil beide Läufe in der
GPTQ-Einstellung übereinstimmten.** Die bisherige Referenz 9,40 stammte
aus einem Lauf **mit** GPTQ; ein direkter Vergleich hätte zwei
Änderungen vermischt. Der zusätzliche Referenzlauf (θ_v 0.14.0 ohne
GPTQ → ebenfalls 9,40) kostete 25 Minuten und war die einzige
Möglichkeit, den Anteil zu trennen.

**Was das methodisch klärt:** Die Referenzsimulation hatte für das
SiLU-Raster ~0 % Perplexitätswirkung vorhergesagt, der Tensorvergleich
6,83 % Fehler. Die Messung entscheidet zugunsten des Tensorvergleichs —
er taugt also zur **Priorisierung**, nicht nur zur Lokalisierung. Die
Simulation bildete die Wechselwirkung mit der nachfolgenden
Multiplikation und Reskalierung nicht ab.

**Konformitätsvektoren neu erzeugt** — sie sind θ_v-gebunden und
brachen erwartungsgemäß (6/30), was die Bindung bestätigt.

**Offen:** +7,49 % gegen ein Kriterium von ≤ 5 %, es fehlen 2,49
Prozentpunkte. Nächster Schritt in derselben, jetzt belegten Richtung:
`integer_math::lut_lookup` auf i32-Indizes umstellen — numerisch
folgenlos, macht `input_frac_bits = 7` erreichbar (Tensorfehler dann
~0,42 % statt 0,84 %).


### v0.13.4 – 2026-08-20 (SIMD wirkt: vektorisiertes Skalarprodukt)

**`--features cpu-simd` brachte nichts, und der Grund war, dass die
falsche Operation optimiert war.** Das neue Operationsprofil
(`kernels/src/bin/op_profile.rs`) hat gemessen, wohin die Zeit geht:

| Operation | Anteil |
|---|---|
| `linear_w8a16` (Layer + LM-Head) | **99,4 %** |
| rmsnorm | 0,4 % |
| rope + softmax | **0,15 %** |

Vektorisiert waren genau die 0,15 %. `linear_w8a16` und `rmsnorm`
delegierten an die Referenz — das stand sogar im Modulkopf von
`backends/simd.rs`, nur hatte niemand ausgerechnet, was das bedeutet.
Selbst ein perfekter 10×-Gewinn auf Softmax und RoPE hätte 0,13 %
gebracht.

**Dazu ein zweiter Befund:** Das `Backend`-Trait wird vom Inferenzpfad
**gar nicht benutzt**. `model.rs` importiert die Kernel direkt
(`integer_llm_kernels::rmsnorm::rmsnorm_i16`); `SimdBackend` wird
ausschließlich im Paritätstest instanziiert. Dasselbe Muster wie Fund A7
und Fund 25: implementiert, getestet, nie aufgerufen. Deshalb sitzt die
Vektorisierung jetzt in `kernels/src/dot.rs`, also dort, wo die
Aufrufstellen sind; das Anbinden des Traits ist eine eigene, größere
Aufgabe.

**Warum das die Bitgleichheit nicht gefährdet:** Die Akkumulation läuft
exakt in i64 (≤ 4,2 · 10⁶ je Produkt, zehn Größenordnungen Reserve).
Exakte Ganzzahladdition ist assoziativ, also ist **jede**
Summationsreihenfolge bitgleich — die vektorisierte Fassung ist per
Konstruktion identisch, nicht bloß getestet identisch. Bei Gleitkomma
wäre dieselbe Umstellung unzulässig. Die Kernthese des Projekts arbeitet
hier für uns.

**Der erste Versuch war langsamer** (12,4 gegen 18,9 tok/s). Ursache
war nicht der Rechenaufwand, sondern eine serielle Abhängigkeitskette:
Ein einziger Akkumulator ließ jede Iteration auf die vorige warten. Mit
vier unabhängigen i32-Akkumulatoren und blockweisem Ausräumen nach i64
liegen die Multiplikationen überlappend in der Pipeline.

**Ergebnis:**

| Modell | reference | cpu-simd | Gewinn |
|---|---|---|---|
| 0,5B | 18,58 tok/s | **24,26 tok/s** | +31 % |
| 7B | 1,35 tok/s | **2,03 tok/s** | +50 % |

Bitgleichheit belegt: identischer `decode_hash` über 32 Token **und**
30/30 Konformitätsvektoren unter beiden Backends.

### Was ein GPU-Kernel einhalten muss, und was nicht (v0.17.0)

Die Stub-Köpfe in `backends/cuda.rs` und `rocm.rs` schrieben bis
2026-08-22 vor: feste Blockgröße, im Code vorgeschriebene
Summationsreihenfolge, kein Warp-Shuffle. **Drei dieser vier Auflagen
sind für die Bitgleichheit unnötig.** Sie hätten einen GPU-Kernel ohne
Gegenwert verlangsamt, und zwar genau an der Stelle, an der eine GPU
schnell ist: bei der parallelen Reduktion.

Der Grund ist eine Eigenschaft, die das Projekt ohnehin schon nutzt:
Die Akkumulation ist **exakt**. Nachgerechnet für die größte Reduktion
des Projekts (Qwen2.5-7B, `intermediate_size` 18944):

| | |
|---|---|
| größtes Einzelprodukt | 127 x 32768 = 4 161 536 |
| größte mögliche Summe | 78 836 137 984, also 2^36 |
| Fassungsvermögen i64 | 2^63 |
| Sicherheitsabstand | **Faktor 117 Millionen** |

Kein Überlauf, keine Rundung, keine Sättigung im Zwischenergebnis.
Ganzzahlige Addition ohne Überlauf ist assoziativ und kommutativ, also
liefert **jede** Reduktionsreihenfolge dasselbe i64. Baumreduktion,
Warp-Shuffle, beliebige Blockgrößen: alles erlaubt.

Was stattdessen gilt:

1. **Nur Ganzzahlen, nie Gleitkomma.** Die eigentliche Auflage.
2. **Keine Tensor Cores**, weil ihre Pfade in reduzierter Breite
   akkumulieren und Operationen verschmelzen. Nicht, weil Akkumulation
   dort grundsätzlich nichtdeterministisch wäre.
3. **Sättigung genau einmal, ganz am Ende.** Daran hängt die
   Assoziativität: Würde ein Kernel Teilsummen klemmen, wäre die
   Reihenfolge plötzlich wieder wirksam.
4. **Keine Annahme über die Warp-Breite** (NVIDIA 32, AMD 64). Das ist
   Portierbarkeit, nicht Determinismus.

Beides steht als Test in `dot.rs` und nicht nur als Behauptung im
Kommentar: `jede_reduktionsreihenfolge_liefert_dasselbe` prüft vorwärts,
rückwärts, Baumreduktion und Blockgrößen 32/64/256/1024 über Längen bis
20 000, `die_akkumulation_kann_nicht_ueberlaufen` rechnet den
schlimmsten Fall aus statt einen zufälligen. Fällt einer der beiden,
gilt der Vertrag nicht mehr.

**Geschrieben sind die Kernel damit nicht.** Sie brauchen GPU-Hardware
zum Prüfen, und hier gibt es keine. Es ist dieselbe Entscheidung wie beim
AVX2-Pfad in `dot.rs`: übersetzbar wäre er, aber nicht auf Parität
prüfbar, und unverifizierte Numerik in einem Konsenspfad lässt einen
Miner slashen, ohne dass er etwas falsch gemacht hat.

### Der größere Hebel lag daneben: die Gewichtskopie (v0.16.0)

Als nächster Schritt war notiert: „Gewichte liegen als
`Vec<Vec<i8>>`, also eine Heap-Allokation je Zeile". Die Lage war eine
andere und einfacher zu beheben: Die Ablage im `QTensor` ist **flach und
war es immer**. Sie wurde nur bei **jedem** Aufruf in die schlechtere Form
zurückverwandelt. `model.rs::forward_layer` rief achtmal je Ebene
`to_vec_vec()` auf, und das erzeugte über `row(idx) -> to_vec()` eine
Heap-Allokation und eine Kopie **je Ausgabe-Zeile**:

| bei Qwen2.5-0,5B, je Token | |
|---|---|
| kopierte Bytes | **358 MB** |
| Heap-Allokationen | **304 128** |

Die Kernel nehmen die Gewichte jetzt flach entgegen (`W: &[i8]` plus
`in_features`) und laufen mit `chunks_exact` darüber. **Die Numerik ändert
sich dadurch nicht:** `dot_i8_i16` bekommt dieselben Bytes in derselben
Reihenfolge, die Zeile ist nur ein Ausschnitt statt einer Kopie.
Bitgleichheit gilt hier per Konstruktion, nicht nur laut Messung.

| Modell | Backend | vorher | nachher | Gewinn |
|---|---|---|---|---|
| 0,5B | reference | 19,95 tok/s | **27,17 tok/s** | +36 % |
| 0,5B | cpu-simd | 25,14 tok/s | **38,19 tok/s** | +52 % |
| 7B | reference | 1,48 tok/s | **2,07 tok/s** | +40 % |

Gemessen mit `bench_probe` über 32 Token (7B: 8 Token), alte Fassung aus
einem `git worktree` auf demselben Rechner. `decode_hash` in allen sechs
Läufen identisch (0,5B `bdebcbac12ae78a9`, 7B `6dcb9528ddf257f2`), 30/30
Konformitätsvektoren unter `reference` und `cpu-simd`, Paritätstest 6/6.

**Damit ist der Gewinn größer als der der Vektorisierung selbst.** Das
Operationsprofil hatte seinerzeit `linear_w8a16` mit 99,4 % der Laufzeit
ausgewiesen, und das stimmte auch: Die Kopie geschah unmittelbar davor,
im selben Aufrufausdruck, und wurde derselben Zeile zugerechnet.

**Ein Fund beim Umstellen, den der Prüflauf sofort meldete:** Der
Golden-Runner baute die Vorgabe-Skalen als `vec![weight_frac; w.len()]`.
Flach ist `w.len()` die Elementzahl statt der Zeilenzahl, und
`linear_w8a16_identity` fiel durch. Das war die richtige Reaktion, denn
der Kernel prüft die Länge. Hätte er sie nicht geprüft, wäre daraus ein
stiller Fehler geworden.

**Dabei aufgefallen:** `conformance/run.sh` nahm zwar einen
Backend-Parameter entgegen, gab ihn aus und **ignorierte ihn dann** —
beide cargo-Aufrufe standen fest auf `--features reference`. Der
Prüflauf konnte also ausschließlich sich selbst zertifizieren, obwohl
sein erklärter Zweck ist, fremde Backends zu prüfen. Behoben.

**Kein AVX2 in diesem Patch, bewusst.** Diese Maschine ist aarch64; eine
AVX2-Fassung ließe sich übersetzen, aber nicht ausführen und nicht auf
Parität prüfen. Unverifizierte Numerik in einen Konsenspfad zu geben ist
die eine Sache, die sich dieses Projekt nicht leisten kann — ein Miner
mit abweichendem Kernel wird geslasht, ohne etwas falsch gemacht zu
haben. Gehört auf echte x86_64-Hardware (K1).

**Nächster Hebel, größer als SIMD:** Die Gewichte liegen als
`Vec<Vec<i8>>`, also eine Heap-Allokation je Zeile — schlecht für
Cache-Lokalität und Prefetch. Der Abstand zu bf16 (Faktor 0,37) dürfte
zum guten Teil daher rühren.


### v0.13.3 – 2026-08-19 (Phase 12.64–13.0: Benchmarks, Modellkarte, Anleitung)

**Die letzten Punkte der Inferenz-Phase.** Neu: `bench/run.py`,
`bench/README.md`, `artifacts/MODEL_CARD.md` und eine
Schritt-für-Schritt-Anleitung für die erste Inferenz.

**Der Durchsatz-Benchmark prüft Bitgleichheit, bevor er Zahlen zeigt.**
`bench_probe` gibt einen `decode_hash` aus; `run.py` verlangt, dass alle
Backends denselben liefern, und bricht sonst mit Fehlercode ab. Ein
Backend, das schneller ist und etwas anderes rechnet, ist kein
schnelleres Backend — es ist ein zweites Modell, und in einem Netz mit
Bitgleichheits-Konsens wäre sein Betreiber beim Redundanzvergleich
auffällig.

**Gemessen (arm64/Darwin, `cpu-simd`, Stand v0.20.0), beide Seiten im
selben Lauf und beide auf der CPU:**

| Modell | Artefakt | Decode | bf16 (Decode) | Verhältnis |
|---|---|---|---|---|
| 0,5B | 0,78 GB | **49,17 tok/s** | 77,57 tok/s | 0,63 |
| 7B | 8,72 GB | **10,74 tok/s** | 9,86 tok/s | **1,09** |

**Bei 7B ist der Integerpfad damit schneller als bf16.** Das war er nicht
immer; der Weg dorthin ging über vier Schritte, und der letzte war der
größte:

| Stand | 0,5B | 7B |
|---|---|---|
| v0.13.3 | 19,50 | 1,42 |
| v0.13.4 (NEON in `dot.rs`) | 24,26 | 2,03 |
| v0.16.0 (Gewichtskopie entfällt) | 38,19 | 2,07 |
| **v0.21.0 (Zeilen über Threads)** | **49,17** | **10,74** |

**Der letzte Schritt war kein Numerikproblem, sondern ein Messfehler in
der Deutung.** Der Integerpfad lief einkernig, während die
bf16-Vergleichsseite fünf Threads benutzte. Die Zahl „3,5× langsamer als
bf16", die hier jahrelang stand, maß deshalb zwei Dinge auf einmal:
Quantisierungskosten **und** fehlende Parallelität.

Aufgefallen ist es nicht beim Optimieren, sondern beim Rechnen der
Wirtschaftlichkeit (Kritikpunkt K8): Ein Kostenverhältnis von 9,2× gegen
einen zentralen Anbieter sah zu schlecht aus, um an der Numerik zu
liegen.

### Wo die Grenze liegt, und was das für große Modelle heißt

Dekodieren liest je Token das **ganze** Modell und ist damit
bandbreitenbegrenzt. Die Formatfrage ist deshalb die entscheidende:
int8-Gewichte sind halb so viele Bytes wie bf16.

| Modell | int8 | bf16 | unsere Byterate | ihre | Ausnutzung |
|---|---|---|---|---|---|
| 0,5B | 0,78 GB | 1,00 GB | 38 GB/s | 78 GB/s | 49 % |
| 7B | 8,72 GB | 15,2 GB | 94 GB/s | 150 GB/s | **62 %** |

**Die Obergrenze ist das Byteverhältnis**, bei 7B also 1,74×. Wir stehen
bei 1,09×, holen davon also erst 62 %. Der Rest ist Kernel-Arbeit, kein
Naturgesetz.

**Hochrechnung auf die Zielgrößenordnung, ausdrücklich als solche.** Je
größer das Modell, desto klarer bandbreitenbegrenzt ist das Dekodieren,
und desto mehr zählt allein die Byterate. Beide gemessenen Punkte stützen
das (Verhältnis 0,63 → 1,09, Ausnutzung 49 % → 62 %), aber **zwei Punkte
sind keine Kurve**, und das gilt hier gegen die eigene These wie überall.

Für GPU kommt hinzu: Das Tensor-Core-Verbot aus Kap. 6.2 kostet beim
**Dekodieren** wenig, denn ein Token ist eine Matrix-Vektor-Rechnung und
damit bandbreiten- statt rechenbegrenzt. Beim **Prefill** kostet es,
denn dort steht eine Matrixmultiplikation.

**Drei Vorbehalte, der dritte ist der ernsteste:** Auf GPU ist nichts
davon gemessen. Modelle dieser Größe werden geshardet, und in keiner
dieser Zahlen steckt ein Netz-Hop. Und verglichen wird **Batch 1 gegen
Batch 1**, während echte Anbieter stark bündeln, was ihre Seite
rechenbegrenzt macht und Tensor Cores wirken lässt.

Zwei Befunde, die ich nicht in eine Fußnote schiebe:

- **Die Parallelisierung ist bitgleich per Konstruktion.** Jede
  Ausgabezeile ist ein eigenes Skalarprodukt und schreibt in ihr eigenes
  Feld; zwischen den Zeilen gibt es keine gemeinsame Zwischensumme und
  damit keine Reihenfolge, die etwas ändern könnte. Belegt: 33/33
  Konformitätsvektoren, unveränderter `decode_digest` bei 0,5B, und bei
  7B derselbe Wert aus zwei getrennten Prozessen und beiden Backends.
- **Die Threadzahl hängt an der Arbeitsmenge, nicht an der Kernzahl.**
  Der erste Versuch nahm `available_parallelism` (hier 15) und brachte
  bei 0,5B **nichts**: Fünfzehn Threads zu starten kostet 107 µs, und die
  4864×896-Matrix braucht einkernig 289 µs. Dieselbe Matrix mit vier
  Threads 2,53×, mit fünfzehn nur 1,72×; bei der größten 7B-Matrix ist es
  umgekehrt (7,40× mit fünfzehn). Beide Konstanten sind gemessen,
  `kernels/src/bin/threads_probe.rs`.

**Skalierung:** Von 0,5B auf 7B wächst das Artefakt um Faktor 11,2, der
Durchsatz fällt um Faktor 13,7 — grob linear mit leichtem Aufschlag.
Zwei Punkte sind keine Kurve; die Zielgrößenordnung liegt weit darüber.
`run.py` ist deshalb modellagnostisch und löst Pfade über dieselbe Quelle
auf wie Kalibrierung und Perplexitätsmessung
(`calibrate/src/model_configs.py`), damit es auf dem nächstgrößeren
Dense-Modell unverändert läuft.

**Die Modellkarte ist ein Formular, keine Prosa** — je Modellgröße eine
Spalte, mit einem eigenen Abschnitt „Was diese Artefakte **nicht**
belegen": keine heterogene Hardware (K1), keine Zielgrößenordnung (K6),
5-%-Kriterium bei 7B offen, kein Training.

**Die Anleitung ist durchgespielt worden, und das war nötig.** Drei
Fehler darin: `cargo run` braucht `--bin integer-llm-runtime` (das Crate
hat zwölf Binaries), `build_artifacts.sh` braucht die aktivierte
Kalibrier-Umgebung, und `fetch_model.sh` steuert über `MODEL_ID`, nicht
über `INTEGER_LLM_MODEL`. Eine ungetestete Anleitung ist eine Vermutung.


### v0.12.49 – 2026-08-19 (Boundary-Schritt entfallen, Layout-Unabhängigkeit gemessen)

**Der Boundary-Schritt zwischen Pipeline-Stages ist ersatzlos entfallen.**
Er war reiner Verlust ohne Gegenwert: Die Ausgangsskala des Senders ist
`layers[layer_end].residual_in_frac`, die Eingangsskala des Empfängers
`layers[layer_start].residual_in_frac` — und `layer_start` des Empfängers
**ist** `layer_end` des Senders. Beide Seiten lasen denselben Wert aus
demselben Artefakt (erzwungen durch `theta_v_hash`) und rechneten ihn
trotzdem über einen dritten, gröberen Skalar hin und zurück. Solange die
Skala ein Skalar war, kostete das nur Rundung; seit Fund 20 sie je Kanal
führt, war der Rundweg messbar verlustbehaftet.

Damit ist `test_pipeline_multinode.py` wieder **bitgleich mit dem
Einzelknoten** (vorher Divergenz ab dem sechsten Token: 2746 gegen 2694).
Der weiche Zweig im Test ist zurück in ein hartes `assert` überführt, wie
es der Kommentar dort vorsah.

**Neu: `tests/integration/test_pipeline_layouts.py`.** Beantwortet die
Frage, ob das Shard-Layout das Ergebnis beeinflusst — die Voraussetzung
für den COMPUTE_PIPELINE-Entwurf „variable Knotenzahl je Pipeline".

| Layout | Stage-Grenzen | Ergebnis |
|---|---|---|
| 4 Shards | 6 / 12 / 18 | identisch |
| 8 Shards | 3 / 6 / 9 / 12 / 15 / 18 / 21 | identisch |
| 4 Shards, ungleichmäßig | 1 / 7 / 23 (1, 6, 16, 1 Layer) | identisch |

Alle drei sind zudem bitgleich mit dem Einzelknoten. Das ungleichmäßige
Layout ist das eigentliche Argument: Die 8er-Grenzen sind ein Superset
der 4er-Grenzen, eine Übereinstimmung dieser beiden allein hätte daran
hängen können.

`configs/pipeline_8node.json` war dafür zu reparieren (veralteter
`theta_v_hash`, `pipeline_hash` stand auf `sha256:0000` — genau der
Platzhalter, gegen den Fund 25 die Prüfung eingeführt hat);
`configs/pipeline_uneven4node.json` ist neu.

**Grenzen der Messung:** 0,5B, ein Prompt, sechs Token, drei Layouts.
Nicht gemessen: 7B, längere Generierungen, beliebige weitere Schnitte.
Der Befund ist stark, weil eine Stage-Grenze nach dem Wegfall des
Boundary-Schritts rechnerisch ein No-Op ist — aber er ist eine Messung
an Stichproben, keine Herleitung.

θ_v ist **unverändert**; Konformitätsvektoren 30/30, Gleitkomma-Audit
null Treffer. Die Einzelknoten-Inferenz war nie betroffen.


### v0.12.48 – 2026-08-19

**Der 7B-Fehler ist gefunden: 41,42 → 9,40 (Faktor 45).** Zwei
Implementierungsfehler, beide in Code, den ich in dieser Untersuchung
selbst eingeführt oder übersehen hatte.

| Stand (7B) | Perplexität |
|---|---|
| Ausgangspunkt | 41,42 (+377 %) |
| Fund 20 abgeschaltet | 14,83 (+71 %) |
| **Fund 20 + Fund 24 korrigiert** | **9,40 (+8,29 %)** |

FP-Baseline 8,68. 0,5B bleibt bei **15,29** (+2,3 %) — beide Modelle
profitieren.

**Fund 24: Die Varianzsumme der RMSNorm richtete nach UNTEN aus.**
Um alle Kanäle auf eine gemeinsame Skala zu bringen, schob der Code
gegen `min(shifts)` nach rechts: `sq >> 2*(x_shifts[i] - min)`. Bei
breiter Shift-Spanne löscht das feinskalierte Kanäle aus der Summe —
bei Qwen2.5-7B (Spanne 2–10, also Verschiebung bis 16) trug ein
normaler Kanal statt 160 000 nur noch **2** bei. Die Normalisierung
stützte sich damit fast ausschließlich auf die groben
Ausreißer-Kanäle. Richtig ist die Ausrichtung gegen `max(shifts)` per
Linksshift — dabei geht kein Bit verloren.

Bei 0,5B ist die Spanne schmal (7–12), der Effekt mild. **Deshalb sah
Fund 20 dort wie eine Verbesserung aus (15,59 → 15,29), während er 7B
von 16,26 auf 40,48 verschlechterte.** Fund 20 selbst war nie falsch —
nur seine Implementierung.

**Zum Linksshift und dem numerischen Vertrag:** Whitepaper Kap. 6.2 und
Anhang B.5.4 legen die *Division* auf den arithmetischen Rechtsshift
fest, wegen der Rundungsmehrdeutigkeit bei negativen Zahlen. Ein
Linksshift ist eine exakte Multiplikation mit 2ᵏ, rundungsfrei und
plattformgleich — die Festlegung trifft ihn nicht. Was er sehr wohl
berührt, ist `overflow.behavior = "explicit_clamp_only", wrap = false`:
ein überlaufender Linksshift wrappt. Beide Stellen laufen deshalb in
i128 mit anschließendem expliziten Clamp; neu ist
`fixed_point::rshift_round_i128`. Zwei Tests sichern das ab, darunter
`test_rmsnorm_extremer_shift_bereich_laeuft_nicht_ueber` — der beim
Schreiben prompt einen echten i64-Überlauf im Mittelwert-Rückcast fand.

**Fund 23: int8-Quantisierung sättigte still bei Beträgen über 127.**

```python
shifts = torch.floor(torch.log2(127.0 / absmax))   # 414 -> -1.70 -> floor -2
shifts = torch.clamp(shifts, 0, MAX_FRAC_BITS)     # -2 -> 0   <- hier verloren
quantized = torch.clamp(torch.round(t * 2**0), -128, 127)   # 414 -> 127
```

Für Beträge über 127 bräuchte es einen negativen Shift; das
`clamp(shifts, 0, …)` verbietet ihn, und `torch.clamp` schnitt danach
kommentarlos ab. Betroffen: **16 von 129 024 Bias-Elementen**, keine
einzige Gewichtszeile (0 von 1 694 720) — ausschließlich `k_proj.bias`
in Ebene 27 (414 → 127, 69 % Verlust) und **Ebene 0** (171 → 127).

Biases liegen jetzt in **int16** (`quantize_bias_int16_per_element`,
neuer `BiasTensor` im Loader, `add_bias_i16` nimmt `&[i16]`). Kosten:
~0,25 MB. Wichtiger als der Fix: **beide Quantisierer brechen jetzt laut
ab statt zu sättigen.** Ein Quantisierer, der still abschneidet,
produziert Artefakte, die monatelang wie Quantisierungsrauschen
aussehen.

θ_v 0.12.0 → 0.14.0. Beide Modelle neu kalibriert, Golden Vectors neu
erzeugt (30/30), Pipeline-Konfiguration nachgezogen.
### v0.12.46 – 2026-08-19

**Ursachensuche 7B: Das Quantisierungsschema ist unschuldig — der Fehler
liegt in unserer Implementierung.** Plus zwei behobene Präzisionsverluste
und eine neue Fähigkeit.

**Der Beleg, der die Suche gedreht hat.** Dasselbe
Gewichtsquantisierungs-Schema (int8, symmetrisch, Per-Channel-Zweierpotenz),
in PyTorch nachgebaut und auf denselben Sequenzen gemessen:

| | Perplexität |
|---|---|
| FP-Baseline (BF16) | 8,68 |
| **W8 per-channel in PyTorch** | **8,74 (+0,7 %)** |
| unser Integer-Pfad | 41,42 (+377 %) |

`tests/diag/w8_reference_simulation.py`. W8 trägt bei 7B praktisch
verlustfrei — genau wie man es von einer so großzügigen Quantisierung
erwartet. Damit gehört die Suche in den Rust-Pfad, nicht in die
Quantisierungsstrategie, und es gibt erstmals einen verlässlichen
Referenzmaßstab, gegen den sich jede Stufe einzeln prüfen lässt.

**Fund 22: Der KV-Cache warf 2–4 Bit weg, ohne etwas dafür zu bekommen.**
Der Cache rechnete K/V von der Per-Layer-Skala auf eine globale
Cache-Skala (`kv_cache.frac_bits = 8`) um und beim Lesen zurück auf
dieselbe Per-Layer-Skala. Quelle und Ziel sind identisch — Schreiben und
Lesen betreffen immer dieselbe Ebene —, die Rundreise war also reiner
Verlust:

- 2–4 Bit Auflösung auf fast jeder Ebene beider Modelle
  (0,5B: K median 3, V median 4; 7B: K 2, V 4)
- hartes Clipping, wo der reale Wert die feste Kapazität von
  32767/2⁸ = 128 überstieg — bei 7B in **Ebene 0** um Faktor 3,28
  (K-absmax 420), also an der ersten Ebene, deren Fehler durch alle 28
  propagiert

Der Cache hält K/V jetzt in der nativen Per-Layer-Skala. θ_v 0.11.0 →
0.12.0, `kv_cache.storage` = `per_layer_native`. **Behebt die
7B-Perplexität nicht** (40,68 → 41,42) — der Verlust war real, aber nicht
die gesuchte Ursache.

**Fund 21: Headroom für Per-Kanal-Skalen — gemessenes Negativ-Ergebnis.**
Die Hypothese war, dass Fund 20 zu enge Skalen wählt und deshalb auf
ungesehenen Sequenzen clippt (7B: 6,24 % der Kanäle, bis Faktor 4,53).
Zwei Bit Sicherheitsabstand beseitigten das Clipping wie geplant
(→ 0,02 %, Faktor 1,13), verschlechterten die Perplexität aber **beide**
Modelle drastisch: 0,5B 15,29 → 20,98, 7B 40,68 → **19365**. Der
Auflösungsverlust wiegt schwerer als der Clipping-Gewinn.
`PER_CHANNEL_HEADROOM_BITS` steht auf 0, bleibt aber als dokumentierter
Schalter im Code. Nebenbefund mit Signalwirkung: **7B reagiert auf zwei
Bit weniger Auflösung mit Faktor 476.**

**Schichtweise Hessian-Berechnung (Nachtrag zu 12.72).** GPTQ war für
große Modelle bislang gar nicht ausführbar (45,5 GB für alle 28 Ebenen
gleichzeitig). `HessianCollector` nimmt jetzt einen `layer_range`;
`gptq_group_size()` wählt die Gruppengröße nach verfügbarem RAM, der
Kalibrierkorpus läuft je Gruppe erneut durch das Modell. Bei 0,5B ergibt
sich weiterhin **eine** Gruppe — unverändertes Verhalten. Bei 7B vier
Gruppen à 9 Ebenen. Gemessener Beitrag zur 7B-Perplexität: 40,68 → 40,48,
also **wirkungslos** — aber die Fähigkeit bleibt und war nötig, um genau
das feststellen zu können.

**Acht neue Diagnosewerkzeuge**, jedes für einen konkret ausgeschlossenen
Kandidaten: `w8_reference_simulation.py` (Schema vs. Implementierung),
`hidden_ablation_hf.py` + `final_hidden_dump.rs` (Hidden-State vs.
LM-Head), `perplexity_probe_hf.py` + `--per-token` (Positionsverteilung),
`attention_score_spread.py` (exp-LUT-Domäne), `per_channel_headroom.py`
(Clipping), `channel_dynamic_range.py` (Kanal-Dynamik),
`positional_scale_simulation.py` (Positions-Dimension).

**Sieben Kandidaten gemessen ausgeschlossen.** Die Werkzeuge dafür
liegen unter `INTEGER_LLM/tests/diag/`; jedes trägt seinen Befund im
Kopf.
### v0.12.44 – 2026-08-18

**Fund 19: `1/sqrt(head_dim)` war für `head_dim = 128` um Faktor √2 falsch.**
Erster 7B-Lauf gemessen — Kriterium verfehlt, Ursache nicht gefunden.

- **Der Bug.** `attn_scale_shift = head_dim.trailing_zeros() / 2` ist
  Ganzzahldivision und damit nur für **gerade** `log2(head_dim)` korrekt:

  | head_dim | Shift | angewandt | korrekt |
  |---|---|---|---|
  | 64 (2⁶) | 3 | 0,125000 | 0,125000 |
  | **128 (2⁷)** | **3** | **0,125000** | **0,088388** |
  | 256 (2⁸) | 4 | 0,062500 | 0,062500 |

  Qwen2.5-0.5B hat `head_dim = 64` und lag zufällig richtig; ab 1,5B ist 128
  der Normalfall. **Kein Test deckte 128 ab** — genau deshalb rutschte es
  durch. Derselbe Fehlertyp wie Fund 17.
- **Der Fix.** Neue `fixed_point::inv_sqrt_q15()` (Q15-Reziproke, berechnet
  über `isqrt_round(2^30 / head_dim)` — vollständig ganzzahlig, kein
  `f64::sqrt`, das je nach libm abweichen und den Konsens brechen könnte).
  `attention_int` nimmt jetzt `score_mult: i64` statt den Faktor im Shift zu
  tragen; Backend-Trait und alle vier Backends nachgezogen.
- **Bestehende Artefakte bleiben gültig, bewiesen statt behauptet.** Für
  gerade Zweierpotenzen ist der Multiplikator selbst eine Zweierpotenz
  (64 → 4096 = 2¹²), und `rshift_round_i64` liefert darunter dieselbe
  Rundung **samt Round-to-nearest-even-Tie-Break**. Belege: **Golden Vectors
  30/30** gegen die unveränderten 0,5B-Artefakte, 0,5B-Perplexität weiterhin
  **15,59**. Deshalb **kein θ_v-Bump** — die Spezifikation forderte
  `1/sqrt(head_dim)` bereits, die Umsetzung war fehlerhaft.
- **`layer_probe` rechnete den Faktor gar nicht mit** und maß damit etwas
  anderes als der Produktionspfad. Angeglichen.
- Vier neue Tests, darunter der bitgleiche Rundlauf Shift ↔ Multiplikation
  über Vorzeichen, Tie-Break-Fälle und Shift-Weiten.

**7B-Messergebnis (Punkt 12.74–12.76): Kriterium VERFEHLT.**

| | 0,5B | 7B |
|---|---|---|
| FP-Baseline (BF16) | 14,95 | **8,68** |
| Integer, vor Fund 19 | 15,59 | 14,03 (+61,56 %) |
| Integer, nach Fund 19 | 15,59 | **16,26 (+87,32 %)** |

**Der korrigierte Faktor macht 7B schlechter, nicht besser.** Das ist ein
Negativ-Ergebnis mit Aussagekraft: Fund 19 ist arithmetisch zweifelsfrei
(2⁻³ ≠ 1/√128) und für 0,5B bitgleich, also bleibt der Fix — aber die
dominante Fehlerquelle bei 7B ist er nicht. Geprüft und **ausgeschlossen**:
die Skalenkette ist bei beiden Modellen praktisch gleich (q_frac ~10,
k_frac 10–11), das exp-LUT-Raster also nicht der Unterschied.

**Dass eine korrektere Attention das Ergebnis verschlechtert, heißt, dass
etwas anderes den zu scharfen Softmax bisher kompensiert hat.** Offene
Kandidaten: der untied LM-Head (0,5B nutzt Weight-Tying, 7B nicht), das
ausgelassene GPTQ (auf 0,5B angewendet, bei 7B wegen 45,5 GB Hessian-Bedarf
nicht), und die exp-LUT-Domäne [0, 64), die gegen 0,5B-Score-Differenzen
kalibriert wurde. Nächster Schritt ist der Positionsvergleich gegen HF
(`seq_layer_dump`) — dasselbe Werkzeug, das Fund 15/16 aufgebrochen hat.
### v0.12.43 – 2026-08-18

**Vorbereitung der 7B-Skalierung (Phase 12.70–12.72).** Reine
Kalibrierungs-Seite; an der Runtime war nichts zu ändern.

- **Verifizierte 7B-Konfiguration** in `calibrate/src/model_configs.py`,
  geprüft gegen `Qwen/Qwen2.5-7B` Revision `d1497293`
  (`config.json` + `model.safetensors.index.json`), Lizenz **Apache 2.0**
  wie von Whitepaper Kap. 10.1 / ETHICS G7 verlangt. Basis-Variante, keine
  Instruct-Variante. Drei Unterschiede zu 0,5B berühren den Exportpfad:
  `num_kv_heads` 2 → 4, `tie_word_embeddings` true → **false**,
  `head_dim` 64 → 128 (RoPE-LUTs doppelt so breit).
- **Export-Gate über ein `verified`-Feld.** `get_export_model_config()`
  verlangt jetzt zusätzlich `attention_bias`, `verified` und
  `hf_model_id`. Die abgeschriebenen Instruct-Einträge fallen damit
  weiterhin laut durch — ein Export mit geratenen Werten erzeugt keine
  Fehlermeldung, nur schlechtere Zahlen.
- **Modellwahl per `INTEGER_LLM_MODEL`** statt Codekonstante; die HF-ID
  kommt aus der Config, damit sie nicht an zwei Stellen steht. Neues
  `artifact_model_config()` hält die Herkunftsfelder aus dem Artefakt
  heraus.
- **GPTQ schaltet sich bei zu wenig RAM selbst ab.** Der Hessian-Satz
  wächst quadratisch mit `intermediate_size`: 2,5 GB bei 0,5B,
  **45,5 GB** bei 7B. Der Lauf rechnet das vorab aus, statt nach Stunden
  am Speicher zu scheitern. Vertretbar, weil GPTQ auf 0,5B ein gemessenes
  Negativ-Ergebnis war (v0.12.28). `INTEGER_LLM_GPTQ=1|0` überstimmt.
- **Fund 18 (offen):** Bei `tie_word_embeddings: false` verlangt
  `build_model()` einen int8-`lm_head.weight` *zusätzlich* zum
  int16-LM-Head — 545 MB toter Tensor bei 7B. Artefaktformat-relevant,
  dokumentiert statt stillschweigend behoben.
- Vier neue Tests in `tests/test_export_workflow.py` (7B-Werte gegen die
  veröffentlichte config.json, Herkunftsfelder, Hessian-Rechnung samt
  Abschaltung, Modellwahl per Umgebungsvariable).
### v0.12.42 – 2026-08-18 (Audit-Block 5, Nachtrag: Feature-Builds)

Die CI hat eine Lücke in meiner eigenen Prüfung aufgedeckt: `simd.rs`,
`cuda.rs` und `rocm.rs` werden nur mit ihrem jeweiligen Feature
kompiliert. Der Warnungs-Check aus v0.12.41 lief ohne Features und hat
sie deshalb nie gesehen.

- **Fund A19:** Der Modulkopf von `simd.rs` führte `mlp_silu_avx2 (12.38)`
  als „AVX2-vektorisiert". Das stimmte für den Kernel, **nicht für den
  Aufrufpfad**: `Backend::mlp` ruft den skalaren `mlp_int` auf, der
  Fusionskernel `mlp_silu_fusion_avx2` wird nirgends verwendet. Die
  Paritätstests waren trotzdem grün, weil die Delegation an die Referenz
  per Konstruktion bit-identisch ist. Modulkopf korrigiert; der Kernel
  bleibt mit `#[allow(dead_code)]` und einer ehrlichen Notiz stehen.
  **Bewusst nicht angebunden:** Das braucht einen Paritätslauf auf
  echter x86_64-Hardware, und unverifizierte Numerik gehört nicht in
  einen Konsenspfad.
- Toter `shift_v` in `rshift_round_avx2` entfernt (der Shift selbst nutzt
  korrekt `_mm_set_epi32`; die Variable war Rest eines früheren Versuchs).
- `unreachable`-Warnungen im NEON-Pfad beseitigt: der Referenz-Fallback
  wird auf aarch64 jetzt gar nicht erst kompiliert (NEON behandelt dort
  jeden Fall), statt als toter Code dazustehen.
- Ungenutzte Importe im NEON-Modul entfernt, Matrix-Namen und
  Kernel-Signaturen mit denselben begründeten `#![allow(...)]` versehen
  wie die übrigen Kernel-Dateien.
- **Verifikationslücke geschlossen:** Ab jetzt wird die volle
  Feature×Ziel-Matrix geprüft (default/cpu-simd/cuda/rocm × aarch64/x86_64,
  x86_64 per Cross-`check`). Alle acht Kombinationen: null Warnungen,
  null clippy-Meldungen.

### v0.12.41 – 2026-08-18 (Audit-Block 5)
- **`pipeline` hatte null Tests** — jetzt 33 (codec, manifest,
  kv_cache_node). Der Codec-Test deckte dabei einen echten Überlauf in
  `decode_message()` auf: ein manipuliertes Längenfeld reichte, um
  einen Pipeline-Node abzuschießen. Behoben mit `checked_add`.
- **`golden_runner` prüft jetzt die deklarierten Tensor-Hashes** (SHA-256
  über Little-Endian-Payload, Vertrag aus `tests/golden/generate.py`).
  Vorher waren die Felder eingelesen, aber nie ausgewertet — ein
  nachträglich bearbeiteter Vektor wäre unbemerkt durchgelaufen.
- **PRNG-Test** prüft jetzt die Zustandsfortschaltung (vorher wurde der
  zweite Zustand gebunden, aber nie verglichen).
- **Warnungsfrei:** kernels, runtime und pipeline melden null rustc- und
  null clippy-Warnungen; `-D warnings` ist in der CI verankert. Die
  Matrix-Namen aus Whitepaper Anhang B (`W`, `W_gate`, …) und die
  vielargumentigen Kernel-Signaturen tragen jetzt begründete
  `#![allow(...)]` statt Dauerwarnungen.
- **Ganzzahligkeits-Audit** deckt zusätzlich den Konsenspfad der
  Netzwerkkomponenten ab (37 Dateien). Beide Pfade: null Treffer.
- Golden Vectors weiterhin 30/30, Bit-Exaktheit unverändert.

### v0.12.40 – 2026-08-17 (Phase 12.40–12.55, GPU-Backends)
- **CUDA + ROCm/HIP Delegations-Stubs** (`cuda.rs`, `rocm.rs`):
  - Backend-Trait-Signaturen an theta_v 0.7.0 (Per-Channel-Shifts).
  - Alle Operationen delegieren an Referenz-Kernel (numerisch identisch).
  - Beide kompilieren mit `--features cuda` / `--features rocm`.
  - CI-Job `gpu-backends` verifiziert Compile-Fähigkeit.
- **Hardware-Teststrategie** im README dokumentiert.

### v0.12.39 – 2026-08-17 (NEON-Backend für ARM64)
- **NEON-Implementierungen** für Apple Silicon / ARM64:
  - Softmax: `vmaxvq_s32` Max-Reduktion (4x i32 parallel)
  - RoPE: `rotate_half_split_neon` (4 Paare parallel, RNE-SIMD-Shift)
  - Paritätstests auf M5 Pro: 6/6 bestanden (bit-identisch zur Referenz)

### v0.12.38 – 2026-08-17 (Phase 12.35–12.39, SIMD-Backend)
- **AVX2-SIMD-Backend** (`kernels/src/backends/simd.rs`):
  - Backend-Trait-Signaturen an theta_v 0.7.0 (Per-Channel-Shifts).
  - AVX2 Softmax: Max-Reduktion vektorisiert (8x i32 parallel).
  - AVX2 RoPE: rotate_half_split, 8 Paare parallel, RNE-SIMD-Shift.
  - AVX2 MLP SiLU: Fusionsloop implementiert.
  - Paritätstests: 6 Tests, SimdBackend vs. ReferenceBackend, alle PASS.
  - CI-Job `simd-backend` für x86_64 (AVX2).

### v0.12.37 – 2026-08-17 (Phase 12.32–12.34, Konformitätspaket)
- **Eigenständiges Konformitäts-Artefakt** unter `conformance/`:
  - `README.md`: Format-Doku, Anforderungen pro Ebene (Op/Layer/E2E),
    Rundungsregeln (RNE, Sättigung), theta_v-Bindung.
  - `vectors/`: 30 eingefrorene Golden Vectors (3 Op + 24 Layer + 3 E2E).
  - `run.sh`: Prüflauf gegen beliebige Backends, 30/30 PASS.

### v0.12.36 – 2026-08-17 (Phase 12.26–12.31, Golden Vectors)
- **Layer- und E2E-Golden-Vektoren mit echtem Modell** (kein Dummy mehr):
  - `runtime/src/bin/golden_generate.rs`: erzeugt 24 Layer-Vektoren
    (int16-Residualstrom, echte kalibrierte Gewichte) und 3 E2E-Vektoren
    (echter Qwen2.5-Tokenizer, Greedy-Decoding).
  - `runtime/src/bin/golden_model.rs`: validiert Layer/E2E einzeln oder
    im Batch-Modus (ein Modell-Load fuer alle 27 Vektoren).
  - `tests/golden/generate.py`: ruft `golden_generate` per Subprozess,
    theta_v_hash jetzt aus spec.json (SHA-256).
  - `tests/golden/validate.py`: Batch-Validierung fuer Layer/E2E,
    Skip-Logik entfernt. Ergebnis: 30/30 PASS.
  - `.github/workflows/ci.yml`: CI-Workflow mit Cargo-Tests, Op-Golden
    (immer), Layer/E2E-Golden (conditional), Audit-Suite.

### v0.12.35 – 2026-08-13 (Phase 12.22–12.25, Zahlensemantik-Audit)
- **Audit-Suite `tests/audit/`** sichert die Kerneigenschaft automatisch:
  - `test_no_float.py`: Gleitkomma-Audit des Heißpfads (20 Dateien,
    null Treffer; erlaubte Zonen = Test-Fixtures/golden_runner/loader
    dokumentiert).
  - `test_scales.py`: alle 314 Skalen sind Zweierpotenzen
    (shift ganzzahlig, scale == 2^-shift).
  - `test_division.py` + `fixed_point::division_semantics_vector`:
    fixierte Divisionssemantik (arithmetischer Rechtsshift,
    Round-to-nearest-even), 21 Vektoren, Kreuzvalidierung Rust↔Python.
  - `test_overflow.py` + `fixed_point::overflow_saturation_vector`:
    Sättigung (kein Wrap), fixierte Sättigungsgrenzen.
- `theta_v/spec.json` erklärt das Überlaufverhalten explizit
  (`overflow.behavior = explicit_clamp_only`, `wrap = false`,
  Sättigungsgrenzen i8/i16/i32).
- Volle Suiten grün: kernels 32, runtime 44; alle vier Audit-Skripte
  bestehen.

### v0.12.34 – 2026-08-13 (Phasen 12.56–12.59 + 12.60–12.63)
- **Multi-Node-Pipeline mit echter Inferenz:** Die Stage-Runtime führt
  echte Layer-Ausführung über die Integer-Kernel aus — Embedding in
  Stage 0, Layer-Blöcke je Shard, finale RMSNorm + LM-Head +
  greedy-Sampling mit autoregressiver Feedback-Schleife zur Stage 0;
  shard-spezifische Modell-Ladung mit θ_v-Kanon-Hash-Prüfung
  (SHA-256 über version|weights|scales|luts, trunkiert im
  Nachrichten-Header), KV-Cache je Request im Layer-Range der Stage.
- **Boundary-Kontrakt:** Zwischen den Stages wandert der Residualstrom
  als int16 little-endian auf der natürlichen Zwischen-Stage-Skala
  (bei Qwen2.5-0.5B frac 4); die Reskalierung ist dadurch
  identitätstreu und die Pipeline rechnet dieselben Werte wie der
  Einzelknoten.
- **Bitgleichheit nachgewiesen:** Die 4-Node-Pipeline erzeugt dieselbe
  Token-Sequenz wie die Einzelknoten-Runtime (Prompt „Die Hauptstadt
  von Frankreich ist" → `[12095, 13, 9236, 5999, 2746, 89931]`) und ist
  über zwei unabhängige Läufe deterministisch
  (`tests/integration/test_pipeline_multinode.py`).
- **Chaos-Tests** (`tests/chaos/test_chaos.py`): künstliche Latenz
  (100 ms/Hop), Paketverlust mit Retry-Logik (idempotente Retransmits
  über Duplikaterkennung) und Node-Restart-Idempotenz — alle bitgleich.
- Retry-Logik im Node-Transport (Backoff, 4 Versuche),
  Nachrichten-Rahmen werden vollständig gelesen (Multi-read-fähig),
  Token-IDs als i16-Paare gepackt (Vokabular > i16).

### v0.12.32 – 2026-08-11
- **🎉 ENTSCHEIDUNGSPUNKT 12.21 AKZEPTIERT** — Perplexität **15,59** vs.
  FP-Baseline 14,95 = **+4,29 %** (Kriterium: max. +5 %).
- **Fund 17 (Root-Cause, behoben): fehlende 1/√head_dim-Attention-
  Skalierung.** HF-Qwen2 skaliert die Attention-Scores mit
  `attn_weights = q·k · head_dim^-0.5` (head_dim 64 → Faktor 1/8). Dieser
  Faktor fehlte in `runtime/src/model.rs` im `score_shift`, die Scores waren
  dadurch um √head_dim (=8) zu groß und die Softmax viel zu scharf — die
  Ursache des Perplexitäts-Blow-ups (73,15). Behoben durch einen
  zusätzlichen Rechtsshift um log₂(head_dim)/2 (=3 bei head_dim 64) im
  `score_shift`. Bit-exakt (nur ein Shift), deterministisch, keine
  θ_v-Spezifikationsänderung des Zahlenformats nötig.
- **Perplexität-Verlauf der Eskalationen:** 14 546 (Per-Tensor) →
  3 257 (Per-Channel) → 3 242 (+Headroom) → 3 318 (GPTQ) →
  2 972 (SiLU-Raster) → 73,15 (RoPE-Fix + KV-Cache-Fix) →
  **15,59 (Attention-Skalierungs-Fix)**. Der Blow-up war NICHT die
  Quantisierung, sondern drei Struktur-Bugs (RoPE, KV-Cache, Attention-
  Skalierung), die nacheinander gefunden und behoben wurden.
- Verifikation: Determinismus PASSED (zwei Läufe bit-identisch), E2E
  Perplexität 15,59, alle Rust-Suiten (kernels 30, runtime 44, pipeline)
  und Python-Suiten grün.

### v0.12.31 – 2026-08-11
- **Umfassende Verifikation vor der Präzisions-Entscheidung** (reine
  Diagnose-/Analyse-Werkzeuge, keine Änderung des Inferenzpfads):
  - `tests/diag/verification_layer_compare.py`: Layer-für-Layer-Abgleich
    Integer vs. HF — Aktivierungs-Skalen stimmen (absmax-Verh. 0,84–1,19 in
    Layern 0–22), Werte haben akkumuliertes Quantisierungsrauschen
    (first4-Abw. 0,15→0,83), kein Struktur-Bug.
  - `tests/diag/error_decomposition.py`: Gewichtsquantisierung dominiert
    (0,4–1,4 %/Layer RNE), Aktivierungen (<0,2 %) und LUTs (<1 %)
    vernachlässigbar.
  - `tests/diag/gptq_verification.py`: GPTQ senkt Layer-Fehler 6–8× und ist
    aktiv (das frühere „bringt nichts" war durch die kaputte Attention
    verfälscht).
  - `tests/diag/mixed_precision_sensitivity.py`: Layer-Empfindlichkeit
    gleichmäßig (Faktor 3) → Mischpräzision nur moderat wirksam.
  - Dazu: `hadamard_prestudy.py`, `rmsnorm_hadamard_check.py` (Hadamard
    zurückgestellt, s. Vermerk), `activation_outlier_analysis.py`,
    `smoothquant_simulation.py` (SmoothQuant = Sackgasse, da Aktivierungen
    nicht das Problem sind).
  - **Ergebnis-Bericht:** `eval/results/verification_report.md`. Determinismus
    PASSED (bit-identisch), Perplexität 73,15 vs. FP 14,95, alle Test-Suiten
    grün (Rust kernels 30 + runtime 44, Python komplett).

### v0.12.30 – 2026-08-11
- **DURCHBRUCH: Zwei Struktur-Bugs im Mehrpositions-Pfad behoben**
  (aus der gezielten RoPE/Attention-Untersuchung, θ_v 0.9.0 → 0.10.0):
- **Fund 15 — RoPE war fundamental falsch:** Die Integer-RoPE nutzte einen
  einzigen Winkel `2π·pos/max_seq_len` für alle Dimensions-Paare und
  benachbarte Paarung `(x_0,x_1)`. Qwen2/LLaMA-RoPE nutzt aber pro Paar
  `j ∈ [0, head_dim/2)` eine eigene Frequenz `θ_j = 1/rope_theta^(j/half)`
  (rope_theta = 1 000 000, aus der Modell-Config) und half-split-Paarung
  `(x_j, x_{j+half})` (`rotate_half`). Behoben:
  - `calibrate/src/luts.py`: `generate_rope_luts(max_seq_len, head_dim,
    rope_theta, frac_bits)` erzeugt 2D-LUTs `[max_seq_len, head_dim/2]`
    (flach row-major, Index `p·half + j`), ersetzt `generate_sin_cos_lut`.
  - `kernels/src/rope.rs`: `rotate_half_split_i16` (half-split, pro Paar
    eigener Winkel); `apply_rope_i16` indiziert die 2D-LUT.
  - `runtime/src/model.rs` + `bin/layer_probe.rs`: RoPE-Aufruf auf
    Zeilen-Slices umgestellt.
  - `theta_v/spec.json` 0.10.0: `rope.rope_theta`, `rope.pairing:
    "half_split"`, Note. `main.py` zieht `head_dim` aus der Modell-Config.
  - Tests: `tests/test_rope.py` (Integer-RoPE vs. HF-Formel, Pos
    0/1/2/7/63/2047), erweiterte `rope.rs`-Unit-Tests, `test_luts.py`.
- **Fund 16 — Attention attendierte nur auf den ersten Key (der dominante
  Bug):** In `kernels/src/attention.rs::attention_int` war
  `seq_len = q.len()` die Obergrenze der Key-/Value-Schleife. Im
  KV-Cache-Betrieb ist `q.len() == 1` (nur die aktuelle Position), aber
  `k.len() == v.len() == seq_len` (alle bisherigen Positionen). Damit
  attendierte jede Query nur auf `k[0]` — RoPE und Mehrpositions-Attention
  waren wirkungslos, die Perplexität positionsunabhängig schlecht. Belegt
  durch das Experiment „RoPE = Identität ändert den Seq-Dump nicht". Fix:
  Score-/Value-Schleife läuft über `kv_len = k.len()`. Neuer Regressionstest
  `test_attention_kv_cache_single_query_attends_all_keys`.
- **Messergebnis:** Perplexität **2 972 → 73,15** (Faktor 40; weiterhin
  +389 % vs. FP-Baseline 14,95, Kriterium max. 5 %). Seq-Dump-Vergleich
  Position 7: Ebenen 0–20 stimmen jetzt in Vorzeichen und Größenordnung mit
  HF überein. Der verbleibende Abstand ist akkumuliertes
  Quantisierungsrauschen (int8-Gewichte + LUTs), kein Strukturfehler mehr.
- Tests: alle drei Crates grün (kernels 30, runtime 44, pipeline-Build),
  Python-Suite komplett; Ganzzahligkeitsprüfung ohne Treffer im Rechenpfad.

### v0.12.29 – 2026-08-11
- **SiLU-Eingangsraster verfeinert (θ_v 0.8.0 → 0.9.0):**
  `silu.input_frac_bits` 1 → 3 (Raster 0,5 → 0,125 reale Einheiten),
  `silu.input_range` [-256,255] → [-1024,1023] (gleiche reale Domäne
  [-128, 127.875], LUT 512 → 2048 Einträge). Umsetzung nur in spec.json +
  Kalibrierung/LUT-Generierung; der Inferenzpfad konsumiert `silu_in_frac`
  und `silu_lut_offset` weiterhin spec-gesteuert (Loader), Kernel-Logik
  unverändert. Angepasst: `ModelConfig::default()`, Loader-Test-Assertion
  (Offset 256 → 1024), `tests/test_luts.py` (Struktur/Stützwerte/Längen).
- **Messergebnis:** Perplexität **3 318 → 2 972** (−10 %, weiterhin +19 778 %
  vs. FP-Baseline 14,95). Das SiLU-Raster ist damit eine reale, aber nicht
  die dominante Fehlerquelle.
- **Neue Lokalisierung (Seq-Dump-Vergleich Position 0/1/7):** Position 0
  (Einzeltoken, RoPE = Identität) zeigt Ebenen 0–22 in AbsMax übereinstimmend;
  schon Position 1 (2 Token) weicht ab Ebene 5 ab, Position 7 ab Ebene 15 —
  die Divergenz wächst mit der Position. Da RoPE an Position 0 trivial ist
  und ab Position 1 tatsächlich rotiert, rückt der Mehrpositions-Pfad
  (RoPE/KV-Cache/positionsabhängige Attention) in den Fokus — er war durch
  die bisherigen Position-0-Proben nie abgedeckt.
- Tests: alle drei Crates grün (kernels 28, runtime 44, pipeline-Build),
  Python-Suite komplett.

### v0.12.28 – 2026-08-11
- **GPTQ-Eskalation (Strategie 3, θ_v 0.7.0 → 0.8.0):** Neues Modul
  `calibrate/src/gptq.py` — `HessianCollector` sammelt in derselben
  Kalibrier-Vorwärtspassage die Hessischen Matrizen (H = Σ x·xᵀ) für alle
  168 linearen Projektionen; `gptq_quantize()` quantisiert mit
  Hessian-gestützter Fehlerkompensation (oberer Cholesky-Faktor von H⁻¹,
  sequenzielle Spaltenverarbeitung nach Frantar et al. 2022). Zielgröße ist
  der AUSGABEFEHLER ||X·W − X·Q||² statt des einzelnen Gewichtsfehlers.
  Artefakt-Format unverändert (int8, Per-Channel-Zweierpotenz-Shifts), der
  Integer-Inferenzpfad bleibt unberührt deterministisch. `main.py` wendet
  GPTQ auf die linearen Projektionen an (überschreibt die RNE-Einträge);
  Embedding/Biases/Gammas bleiben RNE, LM-Head bleibt int16.
- **spec.json 0.8.0:** `rounding` ausdifferenziert —
  `linear_weights: "gptq_error_feedback"`, `default:
  "round_to_nearest_even"`.
- **Messergebnis (wichtiges Negativ-Ergebnis):** GPTQ reduziert den
  Ausgabefehler der linearen Schichten nachweislich (Synthetik-Test −47 %,
  21–25 % der int8-Werte weichen von RNE ab), aber die End-to-End-
  Perplexität verbessert sich nicht: **3 242 → 3 318** (weiterhin +22 086 %
  vs. FP-Baseline 14,95). Die Divergenz in Ebene 23 (Integer ~36 vs. HF
  188) bleibt bestehen. **Schlussfolgerung:** die lineare
  Gewichtsquantisierung ist NICHT die dominante Fehlerquelle — der Fehler
  liegt in den Nichtlinearitäten (SiLU-/exp-/rsqrt-LUT) und/oder der
  Aktivierungsquantisierung. Neue Tests `tests/test_gptq.py` (4 Tests).
- Tests: alle drei Crates grün (kernels 28, runtime 44, pipeline-Build),
  Python-Suite komplett inkl. neuer GPTQ-Tests.

### v0.12.27 – 2026-08-11
- **Mehrpositions-Divergenzsuche (Fund-14-Kandidat iii, Diagnose-Patch):**
  Neue Diagnose-Binaries `runtime/src/bin/seq_layer_dump` (Reststrom-
  Statistiken nach jedem Layer an der letzten Position einer Sequenz,
  KV-Cache gefüllt) und `runtime/src/bin/seq_logits_sweep` (Top-1-Logit je
  Position), Gegenstücke `tests/diag/seq_layer_dump_hf.py` /
  `seq_logits_sweep_hf.py`.
- **Lokalisierung (Single-Token, Position 0):** Der AbsMax des Reststroms
  stimmt in allen 24 Ebenen mit HF überein (Ausreißer-Plateau ~1600 in den
  Ebenen 3–20, Abfall auf ~30 in 21–22 — beides echtes Modellverhalten, das
  der Integerpfad reproduziert). Aber die Bulk-Dimensionen (erste 4 Werte)
  weichen schon ab Ebene 3 um ~25–30 % ab. In den letzten Ebenen wird die
  Abweichung kritisch: dort werden die Residual-Ausreißer weggekürzt und das
  Signal ~50× kleiner — Ebene 23 liefert Integer 43 vs. HF 188 (4,4×, anderer
  Inhalt, nicht nur Skala).
- **Ausschlüsse:** Die Gewichtsquantisierung ist über die Ebenen gleichmäßig
  gut (Layer 22 vs. 23: identischer relativer Fehler ~1,5–2 %), und die
  Skalen clampen nicht (Headroom-Check v0.12.26). Damit ist die verbleibende
  Lücke akkumuliertes Quantisierungsrauschen (int8-Gewichte + LUT-Näherungen),
  das in den letzten Ebenen verstärkt wird — kein lokalisierter Einzel-Bug.
- Tests: alle drei Crates grün (kernels 28, runtime 44, pipeline-Build).

### v0.12.26 – 2026-08-11
- **Fund-14-Kandidat (i) geprüft (außerplanmäßiger Patch):** Neue Diagnose
  `tests/diag/scale_headroom_hf.py` misst die realen Aktivierungs-Spannweiten
  auf denselben WikiText-2-Sequenzen wie der Entscheidungspunkt und
  vergleicht sie mit den kalibrierten Per-Layer-Skalen. Ergebnis: die auf
  nur vier Kurz-Prompts (~200 Token) kalibrierten Skalen hatten keinen
  Headroom — **50 von 314 Modulen clampten** still an der int16-Grenze
  (schlimmste: `layers.12.mlp.down_proj.input` 2,8× über Skala,
  `model.norm.input` 2,2× über Skala = der finale Residualstrom vor dem
  LM-Head), weitere 185 waren knapp (<1,5×).
- **Abhilfe:** `calibrate/src/main.py` kalibriert jetzt zusätzlich auf einer
  breiten Stichprobe von 64 WikiText-2-Sequenzen à ≤128 Token aus derselben
  Verteilung (die vier konkreten Mess-Sequenzen werden ausgespart, keine
  Benchmark-Überpassung). Neukalibrierung: **0 von 314 Modulen clampen**,
  schlechtester Headroom jetzt 1,01×.
- **Wichtiges Negativ-Ergebnis:** Die Perplexität änderte sich dadurch kaum
  (3 257 → **3 242**, weiterhin +21 579 % vs. FP-Baseline 14,95). Das
  Aktivierungs-Clamping war also real und ist behoben, aber **nicht die
  dominante Fehlerquelle**. Fund 14 bleibt offen; die verbleibende Lücke
  verlangt die übrigen Kandidaten (ii: SiLU-Eingangsraster, iii:
  Mehrpositions-Attention) oder eine gezielte Mehrpositions-Divergenzsuche.
- Tests: alle drei Crates grün (kernels 28, runtime 44, pipeline-Build).

### v0.12.25 – 2026-08-11
- **Eskalation nach Entscheidungspunkt 12.21 (außerplanmäßiger Patch,
  θ_v 0.6.0 → 0.7.0):** Per-Channel-int8-Quantisierung für ALLE Gewichte
  (zuvor nur LM-Head-Ausnahme in int16): eine Zweierpotenz-Skala je
  Ausgabe-Zeile, bei 1D-Tensoren (Biases, LayerNorm-Gammas) je Element.
  Per-Tensor-Skalen hatten 10–17 % der Gewichtseinträge zu 0 gerundet
  (AbsMax 17–34× über typischer Größe); per-channel sind es 0,0 %.
  Determinismus unberührt: alle Skalen bleiben Zweierpotenzen, der
  Rechenpfad bleibt rein ganzzahlig (Shifts statt Division).
  - `calibrate/src/quantize.py`: `quantize_symmetric_int8_per_channel()`
    (neu, Standard für alle Gewichte); `quantize_model_weights()` darauf
    umgestellt. Legacy-Per-Tensor-Funktion bleibt für Tests erhalten.
  - `calibrate/src/export_weights.py`: je Tensor eine zusätzliche
    `<name>_shifts.bin` (int8, ein Shift je Zeile); Manifest-Einträge
    tragen `shifts_file` + `shifts_hash` und Sentinel `scale:-1.0` /
    `shift:-1`; SHA-256-Nachschreiben-Verifikation auch der Shifts.
  - `runtime/src/loader.rs`: `QTensor.shifts: Vec<u8>` (je Zeile);
    Loader liest per-row Shifts (mit Längen- und Hash-Prüfung),
    abwärtkompatibler Fallback repliziert einen Einzel-Shift.
  - `kernels/`: `linear_w8a16`, `add_bias_i16`, `rmsnorm_i16`, `mlp_int`
    und das Backend-Trait auf per-channel Signaturen umgestellt
    (`w_shifts`/`gamma_shifts`/`bias_shifts` je Ausgabe-Zeile bzw. Element).
  - `runtime/src/model.rs`: Embedding-Lookup, RMSNorm, alle Projektionen,
    Bias-Addition und LM-Head konsumieren die Zeilen-Shifts.
- **Fund 11 (behoben):** Per-Channel-Quantisierung blies 1D-Tensoren
  (Bias, Gamma) durch Broadcasting `t[n] · shifts[n,1]` zu einer
  `[n,n]`-Matrix auf (`q_proj.bias`: 896 → 802 816 Elemente), der
  Runtime-Loader verweigerte darauf die Modell-Ladung. Fix: 1D-Tensoren
  werden als Spaltenvektor behandelt und zurückgequetscht;
  Regressionstest `test_quantize_int8_per_channel_1d_keeps_shape`.
- **Funde 12+13 (behoben, nur Diagnose-Werkzeuge):** `tests/diag/
  layer_probe_hf.py` addierte die Q/K/V-Biases doppelt (Fund 12) und
  wandte `o_proj` doppelt an, weil transformers ≥ 5.x ihn bereits intern
  in `Qwen2Attention.forward` ausführt (Fund 13); außerdem auf die neue
  self_attn-API (`position_embeddings`, `attention_mask`) portiert.
  Beide Fehler verfälschten nur die HF-Vergleichsprobe, nie die
  Messungen (Baseline/Perplexität laufen über den vollen Modell-Forward).
- **Neukalibrierung + Neumessung (Entscheidungspunkt 12.21, 2. Lauf):**
  Perplexität **14 546 → 3 257** (Faktor 4,5 besser), FP-Baseline 14,95
  → relativer Anstieg **+21 683 %** → Akzeptanzkriterium (max. 5 %)
  **weiterhin VERFEHLT**. Logit-/Layer-Proben: S0–S7 stimmen in der
  Skala mit HF überein, ' die'/' der' bleiben in den Top-10, aber die
  Logit-Spannweite ist komprimiert und der korrekte Token (' Paris')
  fällt aus den Top-10 — Muster akkumulierten Quantisierungsrauschens,
  kein lokalisierter Stufenfehler. Dokumentiert als Fund 14; Protokoll
  in `eval/results/decision_12-21.md`.
- Tests: alle drei Crates grün (kernels 28, runtime 44, pipeline 0+Build),
  Python-Suite vollständig (inkl. neuer 1D-Regression), Ganzzahligkeits-
  Prüfung ohne Treffer im Rechenpfad.

### v0.12.24 – 2026-08-11
- **Entscheidungspunkt 12.21 gemessen:** `eval/perplexity.py` vergleicht
  Integer-Modell und FP-Baseline auf identischen WikiText-2-Sequenzen
  (Parameter aus dem Baseline-JSON, Single Source of Truth). Ergebnis:
  14,95 (FP) vs. 14 546,38 (Integer) → **+97 179 %** → Akzeptanzkriterium
  (Vorschlag max. 5 %) **VERFEHLT**. Protokoll mit der zwingenden
  Einordnung (Decodierstrategie, 0,5B-als-ungünstigster-Fall) unter
  `eval/results/decision_12-21.md`. Nächster Schritt: Wahl des
  Eskalationspfads

### v0.12.23 – 2026-08-11
- **FP-Baseline gemessen (12.20):** `eval/baseline.py` — HF-Referenzmodell
  in BF16, Teacher-Forcing, exakt dieselben WikiText-2-Sequenzen wie der
  Integer-E2E-Test (gemeinsame Sequenzauswahl). Ergebnis: Perplexität
  **14,95** auf 435 Positionen — gesichert unter
  `eval/results/baseline_wikitext2.json`

### v0.12.22 – 2026-08-11
- **Messinfrastruktur für den Entscheidungspunkt:** `runtime/src/bin/perplexity_probe`
  (Teacher-Forcing-Log-Probabilities über Token-Sequenzen; Log-Softmax im
  Messpfad f64, Logits aus dem Integerpfad), `eval/wikitext_common.py`
  (gemeinsame deterministische WikiText-2-Sequenzauswahl für alle drei
  Messungen) und `tests/integration/test_end2end_real.py` (E2E-Test mit
  echten Gewichten)
- Erster Messlauf: Determinismus bewiesen (zwei Läufe bitidentisch),
  Integer-Perplexität 14 546 auf 435 WikiText-2-Positionen — quantitative
  Bestätigung von Fund 9; ein Skalierungsfehler der Probe (z_max über
  unskalierte Logits) wurde durch die Endlichkeits-Assertion des Tests
  gefangen und behoben

### v0.12.21 – 2026-08-11
- **Erste echte Integer-Inferenz:** die Runtime lädt die echten kalibrierten
  Gewichte (290 Tensoren inkl. 72 Biases, 314 Skalen, 5 LUTs) und generiert
  deterministisch (zwei Läufe → identischer Token-Hash)
- Diagnose-Binaries ergänzt: `runtime/src/bin/layer_probe` (Layer-0-
  Zwischenwerte vs. HF-Referenz), `logit_probe` (Top-k-Logits nach Prefill),
  `rank_probe` (Teacher-Forcing-Rang des echten nächsten Tokens); verifiziert:
  Embedding/RMSNorm/Q/K/V stimmen innerhalb der Quantisierungstoleranz mit
  float64-Ground-Truth überein
- exp-LUT-Domäne [0, 0.5) → [0, 64) erweitert (theta_v 0.5.2): Messung
  zeigte, dass die alte Domäne 79–92 % der Attention-Positionen
  (Score-Differenzen bis ~28) auf Wahrscheinlichkeit 0 setzte; neuer
  spec-Parameter `exp_input_frac_bits` (Eingang frac 4, Ausgang frac 8),
  `lut_shift` der Attention wird daraus abgeleitet
- Fund 9 (Qualität): Generierung kollabiert nach 1–2 Token in
  Repetitions-Loops; Teacher-Forcing-Ränge mehrheitlich 10³–10⁴; Ursache ist
  die Logit-Verzerrung durch die int8-Quantisierung der Embedding-Tabelle
  (= geteilter LM-Head) — Eskalationspfade am Entscheidungspunkt 12.21

### v0.12.20 – 2026-08-11 (außerplanmäßiger Patch)
- **Numerik-Realitätsabgleich:** Messungen am echten Qwen2.5-0.5B zeigten,
  dass die alten Format-Annahmen nicht tragen (Residual-Spitzen ±1576 statt
  der i8-Annahme ±0,5; h = silu(gate)·up bis ±1640). Aktivierungen sind
  jetzt **int16 mit kalibrierten Per-Layer-Zweierpotenz-Skalen**, Gewichte
  bleiben int8; Residualstrom int16 frac 3
- **theta_v/spec.json 0.4.0 → 0.5.0** (konsensrelevant, mit Zustimmung des
  Projektinhabers): residual frac 3, activation int16, rsqrt input_shift 8
  + `index_normalization: dynamic_even_shift`, SiLU-Domäne [-256,255] mit
  `input_frac_bits: 1` / `output_frac_bits: 6`
- **Neuer Kernel `rmsnorm_i16`:** LUT-gestütztes rsqrt wird jetzt
  konsumiert, divisionsfrei im Hot-Path (Mittelwert via
  Reziproken-Multiplikation, dynamischer gerader Index-Shift), explizite
  Ziel-Ausgabeskala, gamma mit eigenem kalibriertem Shift — Funde 1 und 8
  sind damit behoben
- Kernel auf int16-Aktivierungen umgebaut: `linear_w8a16` (i64-Akkumulator),
  `add_bias_i16`, `rotate_pairs_i16`, `attention_int`/`mlp_int` mit
  Per-Layer-Skalen; `softmax_int`-Overflow-Fix (maskierte i32::MIN-Werte)
- Runtime: Per-Layer-Skalen vollständig verdrahtet (Fund 2 gelöst),
  `ModelConfig` aus der eingebetteten spec.json (`spec_model_params()`),
  `build_model` validiert alle Skalen-Einträge laut
- Kalibrierung: int16-Wertebereich (`ACTIVATION_MAX_INT = 32767`),
  erweiterter Korpus (vier Prompts), neue Hooks; 265 Skalen (Shifts 4–16),
  θ_v-Hashes konsistent
- Backend-Trait/Platzhalter auf neue API; alte AVX2-Intrinsics entfernt
  (Neuaufbau in Phase 12.35–12.39); Golden Vectors regeneriert
- Volle Suite grün: kernels 25, runtime 40 Tests, alle Python-Skripte inkl.
  Cross-Hardware (6/6 Backends)

### v0.12.19 – 2026-08-11 (außerplanmäßiger Patch)
- **Attention-Biases in der Runtime** (Fund aus dem Kalibrierungslauf 12.16,
  mit dem Projektinhaber als außerplanmäßiger Patch beschlossen): Qwen2.5
  besitzt Biases an q/k/v_proj — sie werden jetzt im Integerpfad verarbeitet
  statt still verworfen
- Neues Pflichtfeld `attention_bias` in `model_config.json`/`ModelDims`
  (Muster wie `num_kv_heads`/`tie_word_embeddings`); fehlt es, scheitert das
  Laden laut
- `loader.rs` lädt bei `attention_bias: true` je Layer die Bias-Tensoren
  `*.self_attn.{q,k,v}_proj.bias` und validiert ihre Längen (q:
  num_heads×head_dim, k/v: num_kv_heads×head_dim)
- Neuer Kernel `add_bias_i8()`: Bias mit eigener kalibrierter Skala wird per
  `rescale` auf die Q/K/V-Ausgabeskala gebracht, i32-Addition mit Clamping —
  reine Ganzzahlarithmetik; Aufruf in `model.rs` nach den q/k/v-Projektionen
- `model_configs.py`: `"attention_bias": True` für 0.5b; Kalibrierungslauf
  wiederholt, Artefakte tragen das Feld
- Acht neue Tests (4 Kernel: Rescale-Richtung/RNE/Sättigung/Länge, 4 Runtime:
  Pflichtfeld/Laden/fehlender Tensor/falsche Länge); Python-Fixtures im
  echten Qwen2.5-Format

### v0.12.18 – 2026-08-11
- LUT-Generierung vollständig aus `theta_v/spec.json` gesteuert (neues
  `calibrate/src/luts.py::load_nonlinear_spec()`): keine hartkodierten
  LUT-Parameter mehr in `main.py`
- `generate_rsqrt_lut()` um den bisher ignorierten `input_shift`-Parameter
  korrigiert (Index x = Realwert x · 2^-input_shift, spec: 2^-7) —
  die LUT-Werte ändern sich dadurch grundlegend (z. B. lut[128]: 23 → 256)
- Kernel-Vertrag verifiziert: SiLU (Eingang frac 6, shift 0/offset 128),
  exp (Eingang frac 8, lut_shift 0), RoPE (frac 8) konsistent zur Runtime
- Neuer Test `tests/test_luts.py` (spec-Struktur, input_shift-Semantik,
  Stützwerte, spec-gesteuerte Längen/int16-Bereich)
- Fund: rsqrt-LUT wird noch von keinem Kernel konsumiert (rmsnorm.rs nutzt
  `rsqrt_q()` direkt, spec sagt Methode „lut"); Fund: `rmsnorm_int8`
  dividiert mit `/` statt arithmetischem Rechtsshift — beides Teil der
  RMSNorm-Klärung vor dem Laden echter Gewichte

### v0.12.17 – 2026-08-11
- **Erster echter Kalibrierungslauf** gegen das lokale Qwen2.5-0.5B:
  vollständige θ_v-Artefakte in `artifacts/myelith-0.5b/`, 168
  Aktivierungsskalen (ausschließlich Zweierpotenzen, Shifts 0–8), 290
  quantisierte Gewichts-Tensoren, θ_v-Hashes konsistent
- Neuer Batch-Test in `tests/test_calibration.py` (200 synthetische
  absmax-Werte: Shift-Grenzen, `scale == 2^-shift`, int8-Bereichs- und
  Sättigungsregime)
- Fund: Qwen2.5-0.5B besitzt Q/K/V-Attention-Biases (72 Tensoren) — sie
  werden exportiert, aber von runtime/kernels noch nicht verarbeitet;
  vor dem Laden echter Gewichte zu klären
- `calibrate/requirements.txt`: `accelerate` ergänzt, auf
  `transformers>=5.0.0`/`huggingface_hub>=1.0.0` konkretisiert;
  `loader.py` von deprecated `torch_dtype` auf `dtype`
- `theta_v/spec.json`: Modell-Angaben auf die Basis-Variante korrigiert
  (konsensrelevant, mit Zustimmung des Projektinhabers; Numerik unverändert)

### v0.12.16 – 2026-08-11
- `calibrate/src/export_weights.py` gehärtet: dtype-Prüfung (nur int8), Prüfung
  Byte-Länge = Produkt der shape, SHA-256-Nachschreiben-Verifikation jeder
  exportierten Datei gegen den Manifest-Eintrag — Manifest und `.bin`-Dateien
  können nicht mehr divergieren
- Referenzmodell auf die Basis-Variante festgelegt: `MODEL_NAME = "myelith-0.5b"`,
  `HF_MODEL_ID = "Qwen/Qwen2.5-0.5B"` (der Code trug bisher Instruct-Strings,
  Whitepaper/Doku/lokales Modell benennen die Basis-Variante); model_configs-Schlüssel
  jetzt `"myelith-0.5b"`, `fetch_model.sh`-Default angepasst
- `calibrate/src/loader.py` lädt das Referenzmodell ausschließlich aus dem lokalen
  Snapshot unter `models/` (reproduzierbare Herkunft) statt aus dem HF-Cache; neu:
  `calibrate/src/paths.py::local_model_dir()` mit klarem Fehlerhinweis auf
  `scripts/fetch_model.sh`
- Fund: `scripts/fetch_model.sh` nutzte das in huggingface_hub ≥ 1.x entfernte
  `huggingface-cli` — auf `hf download` umgestellt
- Vier neue Tests in `tests/test_export_workflow.py`; volle Rust- und Python-Suite grün

### v0.12.15 – 2026-08-11
- `calibrate/src/main.py`: vollständiger Export-Workflow — `model_artifacts_dir()` (neues
  `calibrate/src/paths.py`, spiegelt `runtime/src/paths.rs`) statt hartkodiertem Pfad,
  `model_config.json`-Export ergänzt
- Drei Bugs behoben: Export-Reihenfolge (Gewichte vor `theta_v.json`, jetzt technisch
  erzwungen), `weights_hash` referenzierte einen Platzhalter statt des echten
  `weights_manifest.json`, `version`-Feld war hartkodiert statt aus `theta_v/spec.json`
  gelesen (hätte die 12.13-Versionsprüfung im Loader garantiert scheitern lassen)
- Fund: `export_quantized_weights()` schrieb in ein nicht vom Loader erwartetes
  `weights/`-Unterverzeichnis — behoben
- `calibrate/src/model_configs.py`: `get_export_model_config()` lehnt Modellvarianten ohne
  verifizierte GQA-/Tying-Felder laut ab
- Neuer Test `tests/test_export_workflow.py` inkl. Cross-Check gegen das echte kompilierte
  Runtime-Binary

### v0.12.14 – 2026-08-10
- `runtime/src/loader.rs`: `theta_v/spec.json` wird zur Kompilierzeit ins Binary eingebettet
  (`include_str!`); `load_model()` prüft jetzt Versions-Kompatibilität
  (`ThetaV::verify_version_against_spec()`) und echte Manifest-Hashes (`ThetaV::verify()` —
  existierte seit frühen Phasen, wurde aber nie aufgerufen) gegen die tatsächlich geladenen
  `weights_manifest.json`/`scales.json`/`luts.json`
- **Fund:** `calibrate/src/export.py::export_theta_v()` berechnet `weights_hash` aus einer
  Platzhalter-Datei statt dem echten `weights_manifest.json` und läuft laut `main.py` vor dem
  eigentlichen Gewichts-Export — für 12.14 vorzumerken
- Neun neue Rust-Unit-Tests, zwei neue Python-Integrationstests gegen das echte Binary
- **Phase 12.8–12.13 „Loader und Modellanbindung" damit vollständig abgeschlossen**

### v0.12.13 – 2026-08-10
- `tests/integration/test_end2end.py`: kompletter Umbau auf ein vollständiges synthetisches
  Artefakt (`build_synthetic_artifact()`, exaktes Format von `calibrate/`s Export, gleiche
  GQA-Asymmetrie wie Qwen2.5-0.5B) — der alte Dummy-Modell-Pfad über ein leeres Verzeichnis war
  seit v0.12.10 kaputt (`load_model()` verlangt seither vollständige Artefakte)
- Zwei neue Tests validieren echte Gewichts-Integrität End-to-End gegen das kompilierte Binary:
  `test_rejects_incomplete_artifact` (fehlendes Pflichtgewicht) und
  `test_rejects_corrupted_weight_hash` (manipulierter SHA-256-Hash)
- Zusätzliche Tests: getiedete vs. nicht-getiedete Embeddings, fehlendes Artefakt-Verzeichnis
  (deckt den 12.11-Fehlerpfad end-to-end ab)
- Neues `tests/integration/.gitignore` für generierte Test-Artefakte

### v0.12.12 – 2026-08-10
- `runtime/src/main.rs`: saubere Fehlerbehandlung statt `.expect()`-Panics — `run() ->
  Result<(), String>` fängt Fehler bei Modell-/Tokenizer-Ladung ab, `main()` gibt sie auf
  stderr aus und beendet mit Exit-Code 1 (kein Rust-Panic-Backtrace mehr)
- Vorgezogene, gezielte Prüfungen: fehlendes/kein Artefakt-Verzeichnis, fehlendes
  `tokenizer.json` — klarer als der generische Loader-Fehler an dieser Stelle
- `max_tokens` schlägt bei ungültiger Eingabe jetzt explizit fehl statt still auf 20
  zurückzufallen
- Manuell gegen das kompilierte Binary verifiziert (kein Rust-Unit-Test, `main.rs` hängt
  direkt an `std::env::args()`)

### v0.12.11 – 2026-08-10 (außerplanmäßiger Patch, kein regulärer Entwicklungspunkt)
- `calibrate/src/quantize.py`, `calibrate/src/scales.py`: Skalierungs-Formel korrigiert. Bisher
  wurde der Zweierpotenz-Shift nur für den Fall berechnet, dass ein Wert zu GROSS für int8 ist
  (`absmax > 127`); für den Regelfall realer LLM-Gewichte (`absmax` deutlich unter 1) lieferte
  die Formel unbedingt `shift=0`, was Quantisierung auf `round(roher_Wert)` bedeutete — z. B.
  ein Gewicht von 0,02 quantisiert zu 0. Reale kalibrierte Artefakte wären dadurch numerisch
  bedeutungslos gewesen, unabhängig von jeder Loader-/Runtime-Korrektheit
- `scale`-Feld ist jetzt `2^-shift` statt `2^shift` (Dequantisierung als arithmetischer
  Rechtsshift, Kap. 6.2 des Whitepapers); `runtime/src/loader.rs::load_scales()` entsprechend
  angepasst, drei bestehende Tests korrigiert
- `runtime/src/model.rs`: alle acht Gewichte (q/k/v/o/gate/up/down-Projektion, LM-Head)
  verwenden jetzt ihre eigene kalibrierte `QTensor.shift` statt einer globalen Konstante
- Attention-Softmax-Skalierungsfehler behoben: `score_shift`/`lut_shift` an
  `attention_int` waren nicht auf den tatsächlichen Kalibrierungsbereich der exp-LUT
  abgestimmt (Scores wurden 16× zu grob indiziert)
- `kernels/src/mlp.rs::mlp_int`: Signatur um separate `gate_frac_bits`/`up_frac_bits`/
  `down_frac_bits` erweitert statt einer gemeinsamen Gewichtsskala
- Neuer Test `tests/test_calibration.py` (sieben Fälle, Regressionstest für den Bug)
- **Bewusst offen gelassen:** RMSNorm-Ausgabeskala folgt näherungsweise der (aktuell
  verworfenen) Gamma-Kalibrierung; vollständige Verdrahtung der Q/K/V-Aktivierungsskalen aus
  `scales.json` zurückgestellt (später vollständig umgesetzt im Patch
  „Numerik-Realitätsabgleich", v0.12.20)
- **Nummerierungs-Konsequenz:** Punkt 12.11 und alle Folgepunkte behalten ihre Nummer,
  verschieben sich aber je um eine Version (12.11→v0.12.12, …, 13.0→v0.13.1)

### v0.12.10 – 2026-08-10
- `runtime/src/loader.rs`: `load_model()` baut jetzt ein vollständiges `IntegerModel` aus echten
  Artefakten statt Dummy-Daten (`build_model()`, `ModelDims`/`load_model_dims()` aus
  `model_config.json`, `LoadedWeights::get()` über HF-Originalnamen wie
  `model.layers.0.self_attn.q_proj.weight`)
- **Fund:** reales `models/Qwen2.5-0.5B/config.json` hat `num_key_value_heads=2` (gegenüber 14
  Query-Heads, Grouped-Query-Attention) und `tie_word_embeddings=true` — beides war im
  bisherigen Code nicht abgebildet und hätte beim Laden echter Gewichte zu falscher
  Attention-Berechnung bzw. einem fehlenden `lm_head.weight` geführt
- `runtime/src/model.rs`: `IntegerModel.num_kv_heads` ergänzt; `split_heads()` parametrisiert;
  RoPE für Q-/K-Heads getrennt (`rotate_pairs` statt `apply_rope`); KV-Cache nach
  `num_kv_heads` dimensioniert; Attention-Schleife ordnet Query-Heads ihrem KV-Head zu
  (Standard-GQA-Gruppierung); `runtime/src/generate.rs` entsprechend angepasst
- `ModelDims.tie_word_embeddings` steuert LM-Head-Wiederverwendung explizit statt über
  stillschweigenden Fallback bei fehlendem Tensor
- **Offene Lücke, vor 12.18 zu schließen:** `load_scales()`-Ergebnis wird geladen und im Modell
  abgelegt, aber noch nicht in den Forward-Pass verdrahtet (globale statt kalibrierte
  Pro-Layer-`frac_bits`) — Risiko für die Qualitätsmessung in 12.18–12.21
- `calibrate/src/model_configs.py` fehlt `num_key_value_heads`/`tie_word_embeddings` für alle
  Varianten außer der verifizierten 0.5B (für 12.14 vorzumerken)
- Elf neue Unit-Tests, u. a. End-to-End-Fixture mit derselben GQA-Asymmetrie wie das echte
  Modell und ein Forward-Pass-Rauchtest

### v0.12.9 – 2026-08-10
- `runtime/src/loader.rs`: `load_scales()` lädt Aktivierungsskalen aus `scales.json`
  (`shift`, `scale`, `absmax_observed` je Layer-/Modulname, Format aus
  `calibrate/src/scales.py::compute_scales_from_stats`)
- Validiert Wertebereich von `shift` (0..=255) und Konsistenz `scale == 2^shift`; eine
  widersprüchliche Skala wird abgelehnt statt stillschweigend übernommen
- Fünf neue Loader-Unit-Tests: Roundtrip, mehrere Layer, Nicht-Zweierpotenz-Skala,
  shift außerhalb 0..=255, fehlendes Manifest
- `load_model()` nutzt weiterhin Dummy-Skalen/-LUTs/-Gewichte (Einbindung folgt in 12.10)

### v0.12.8 – 2026-08-10
- `runtime/src/loader.rs`: `load_luts()` lädt Lookup-Tabellen aus `luts.json` +
  `<name>.lut.bin` (raw int16, little-endian, `struct.pack(f"<{n}h", ...)` aus
  `calibrate/src/export.py`), mit dtype-, Längen- und SHA-256-Validierung je Tabelle,
  analog zu `load_weights()` aus v0.12.2
- Sechs neue Loader-Unit-Tests: Roundtrip, mehrere Tabellen gleichzeitig, Hash-Mismatch,
  Größen-Mismatch, falscher dtype, fehlendes Manifest
- Festgestellt: `artifacts/README.md` beschreibt LUTs unter `luts/*.lut.bin`, der tatsächliche
  Export (`calibrate/src/export.py`) legt die Dateien jedoch flach im Artefakt-Verzeichnis ab;
  Loader folgt dem Exportcode, Doku-Korrektur noch offen
- `load_model()` nutzt weiterhin Dummy-LUTs (Einbindung folgt in 12.10)

### v0.12.7 – 2026-08-10
- `scripts/fetch_model.sh` (neu): lädt `MODEL_ID`@`REVISION` per `huggingface-cli` nach
  `models/<Name>/`, gibt die aufgelöste Commit-Revision zur Dokumentation in `models/README.md`
  aus
- `scripts/build_artifacts.sh` (neu): dünner Wrapper um `python3 -m calibrate.src.main`
  (Kalibrierung und Export in einem Lauf)
- `calibrate/requirements.txt`: `huggingface_hub` explizit ergänzt (bisher nur transitiv über
  `transformers`, aber von `fetch_model.sh` direkt benötigt)
- Phase 12.3–12.7 („Grundgerüst") damit abgeschlossen

### v0.12.6 – 2026-08-10
- `eval/` angelegt: `README.md` (Zweck, geplanter Inhalt `baseline.py`/`perplexity.py` für
  die Gleitkomma-Baseline und den Perplexitätsvergleich, Verweis auf den
  Entscheidungspunkt 12.18–12.21) und
  `datasets/.gitignore` (Datensätze nicht versioniert, Verzeichnis und Doku bleiben)

### v0.12.5 – 2026-08-10
- `tests/golden/vectors/{op,layer,e2e}/` angelegt; bestehende Vektoren aus `tests/golden/ops/`,
  `tests/golden/layers/` und `tests/golden/e2e/` dorthin verschoben (Inhalt unverändert)
- `tests/golden/generate.py` und `tests/golden/validate.py` auf zentrale Pfadkonstante umgestellt
  (`VECTORS_DIR` bzw. `VECTORS_DIRNAME`/`LEVELS`), `tests/golden/README.md` entsprechend ergänzt

### v0.12.4 – 2026-08-10
- `models/` angelegt: `.gitignore` und `README.md` mit Modellherkunft (Qwen/Qwen2.5-0.5B, Hugging Face); die Revision (Commit-Hash) wird nach dem Download fixiert
- Bugfixes (mit v0.12.4 mitgeführt): Literal-Newline in f-Strings von `test_kernels.py` und `test_cross_node.py`, fehlender `List`-Import in `test_cross_node.py`, pytest-Abhängigkeit in `test_fixed_point.py` entfernt (Eigenständiges Skript nach Projektkonvention), fehlende `tokenizer.json`-Fixture in `test_end2end.py` (minimaler BPE-Tokenizer für den Test-Prompt), i8/i16-Mismatch im SiLU-LUT-Lookup von `backends/simd.rs` (AVX2- und Fallback-Pfad, konsistent zum Reference-Backend)

### v0.12.3 – 2026-08-10
- `artifacts/` angelegt: `.gitignore` (erzeugte Inhalte ausgeschlossen, Verzeichnis und Doku bleiben versioniert) und `README.md` mit Struktur, Herkunft und Pfadregel
- `runtime/src/paths.rs` (neu): zentrale Pfadkonstanten `ARTIFACTS_DIR` und `MODELS_DIR` für calibrate/runtime/pipeline, überschreibbar über die Umgebungsvariable `INTEGER_LLM_ARTIFACTS_DIR`, mit Unit-Test
- Umnummerierung der Entwicklungsplanung nach Regel 5: Binär-Format-Parser ist Punkt 12.2 ↔ v0.12.2, Grundgerüst rückt auf 12.3–12.7, alle Punkte ab 12.8 unverändert

### v0.12.2 – 2026-08-10
- `loader.rs`: Binär-Format-Parser für INT8-Gewichte (`weights_manifest.json` + `.bin`, raw int8, row-major) mit dtype-, Form-, Größen- und SHA-256-Validierung pro Tensor; neue Dependency `sha2`
- Loader-Unit-Tests: Roundtrip, Hash-Mismatch, Größen-Mismatch, fehlendes Manifest, SHA-256-Referenzvektoren
- Bugfixes (mit v0.12.2 mitgeführt): Compile-Fehler in Kernels (`LinearScale`-Import in `backend.rs`, i8/i16-Mismatch in `mlp.rs`), `128i8`-Überlauf im `linear.rs`-Test, `[100i16, …]`-Literal + struct-Header-Format + CRC32-Berechnung + Socket-Connect im Multinode-Test, fehlender `subprocess`-Import + relativer Golden-Pfad in `validate.py`, Einrückungsfehler + W=128→127 im Op-Golden-Vektor in `generate.py`, Literal-Newline in `test_end2end.py`

### v0.12.1 – 2026-08-09
- Golden-Vector-Runner-Binary (`golden_runner`) für Op-Level-Validierung (RMSNorm, Linear, Softmax)
- `validate.py`: Subprozess-Aufruf von `golden_runner` mit numerischem Hash-Vergleich Input/Output
- `test_kernels.py`: Rust↔Python-Bridging via `cargo test --features <backend>` + stdout-Parsing
- `test_cross_node.py`: Fehlerausgabe bei Golden-Vector-Failures, `List`-Import fix