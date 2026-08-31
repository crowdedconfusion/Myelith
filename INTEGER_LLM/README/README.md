# integer-llm

> **Version:** 0.28.1 (θ_v 0.17.0; kernels 0.29.1, runtime 0.22.1, pipeline 0.15.0)
> **Datum:** 2026-08-30
> **Status:** 🎉 **Akzeptanzkriterium ≤ 5 % auf beiden Modellen erreicht.**
> 7B: **41,42 → 8,78** (+1,14 % gegen die BF16-Baseline 8,68), 0,5B: **15,27** (+2,11 %).
> Der unabhängig gemessene Boden des Quantisierungsschemas liegt bei +0,84 % — der
> gesamte verbleibende Umsetzungsverlust beträgt damit **0,30 Punkte**.
> Zuletzt entscheidend: Fund 31 (θ_v 0.17.0), die doppelte Klemmung in der
> Residual-Addition.
>
> **Zuletzt am Prüfstand statt am Modell (2026-08-22):** Fund 33 (ein
> Prüflauf zertifizierte Backends, die nicht rechnen) und Fund 34 (die
> Sperre dagegen führte `cpu-simd` auf x86_64 als Rechenpfad, wo keiner
> existiert). Beide betreffen nicht die Zahlen, sondern die Aussage über
> die Zahlen.

Bit-exaktes, vollständig ganzzahliges Inferenzsystem für LLMs auf
Qwen-Basis, **W8A16**: Gewichte int8 mit Per-Channel-Zweierpotenzskalen,
Aktivierungen int16, Akkumulator int32, Residualstrom int16.

## Ziel

Deterministische Integer-Inferenz ohne Gleitkommaoperationen im Rechenpfad
(Division ausschließlich als arithmetischer Rechtsshift), mit
Pipeline-Parallelismus auf heterogenen Hardware-Knoten (NVIDIA, AMD, CPU).
Die Ganzzahlarithmetik ist die Voraussetzung für bitgleiche Ausführung über
unabhängige Knoten hinweg — die Grundlage des Myelith-Verifikationsmodells
(Whitepaper Kap. 6.2). Referenzmodell ist Qwen2.5-0.5B (**W8A16**: Gewichte
int8, Aktivierungen und Residualstrom int16, Akkumulator int32); verifiziert
sind daneben Qwen3-4B, Qwen2.5-7B und Qwen3-30B-A3B.

*(Hier stand bis zum 2026-08-27 „W8A8: Gewichte und Aktivierungen als int8".
Das galt bis θ_v 0.15.0. Mit θ_v 0.16.0 sind Aktivierungen und Residualstrom
auf int16 gewechselt, weil Messungen am echten Modell Residual-Spitzen von
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
1. `calibrate/src/main.py`: `MODEL_NAME`/`HF_MODEL_ID` (zwei Konstanten,
   derzeit kein CLI-/Env-Hebel).
2. `theta_v/spec.json`: `rope_theta` und `max_seq_len` (zur Kompilierzeit
   per `include_str!` eingebettet; Änderung erzwingt Runtime-Rebuild,
   θ_v-Versionssprung und Neukalibrierung).
3. Hart kodierte Pfade in `eval/` (`wikitext_common.py`, `perplexity.py`,
   `baseline.py`), `tests/` und den Pipeline-Configs.
4. Feste LLaMA/Qwen2-Block-Topologie: **Qwen3 benötigt QK-Norm
   (`q_norm`/`k_norm`), die aktuell durch die gesamte Kette fehlt** — der
   einzige echte Struktur-Blocker für einen Qwen3-Wechsel.

**Austausch-Aufwand (Abschätzung):**
- Innerhalb der Qwen2.5-Familie: ~1 Config-Eintrag in `model_configs.py`
  (verifiziert gegen die echte HF-config.json) + ~5 Zeilen
  Konstanten/Pfade + Neukalibrierung; Runtime-Änderungen: keine.
- Qwen3: zusätzlich QK-Norm durch Kalibrierung/Export/Loader/Forward.
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
| `kernels/` | Rechenkerne (RMSNorm inkl. QK-Norm, W8A16-Linear, RoPE, Softmax, Attention, MLP, MoE-Router, Sampling) mit austauschbaren Backends über ein `Backend`-Trait. Implementiert ist das `reference`-Backend; `cpu-simd`, `cuda` und `rocm` sind als Features vorbereitet. |
| `runtime/` | Modell-Loader, Transformer-Forward-Pass, KV-Cache, Tokenizer, Generierungs-Loop und CLI (`integer-llm-runtime`). |
| `pipeline/` | Mehrknoten-Orchestrierung (Stage-Runtime; der Betrieb über ein echtes Netz folgt in einer späteren Phase). |
| `calibrate/` | Python-Offline-Phase: lädt das HF-Referenzmodell, quantisiert Gewichte, berechnet Aktivierungsskalen, erzeugt Lookup-Tabellen und exportiert die θ_v-Artefakte. |
| `theta_v/` | Der kanonische numerische Vertrag (`spec.json`). |
| `tests/` | Unit-, Integrations-, Regressions- und Golden-Vector-Tests. Python-Tests sind eigenständige Skripte, Rust-Tests liegen inline in den Modulen. |
| `eval/` | Qualitätsmessung: Gleitkomma-Baseline und Perplexitätsvergleich. |
| `bench/` | Zwei Messungen: `run.py` misst Durchsatz je Backend und gegen die Gleitkomma-Referenz und **prüft dabei, dass alle Backends bitgleich rechnen**; `qualitativ.py` stellt echte Prompts Seite an Seite mit BF16. Zahlen und Einordnung in [`bench/README.md`](../bench/README.md). |
| `models/` | Quellmodelle (Qwen2.5-0,5B und -7B, nicht versioniert). |
| `artifacts/` | Exportierte θ_v-Artefakte (nicht versioniert) und die [Modellkarte](../artifacts/MODEL_CARD.md) — Verfahren, Kalibrierungsdaten, Werkzeugversionen und **was die Artefakte nicht belegen**. |
| `scripts/` | Hilfs-Skripte: `fetch_model.sh` (Modell-Download mit fixierter Revision), `build_artifacts.sh` (Kalibrierung + Export in einem Lauf). |
| `conformance/` | 30 eingefrorene Testvektoren mit `run.sh`. Ein fremdes Backend gilt als konform, wenn es alle 30 bitgleich reproduziert. |
| `configs/` | Pipeline-Layouts (4, 8 und ungleichmäßig geshardet). Die Layouts liefern nachweislich identische Token. |
| `docs/` | Ausgearbeitete Belege, u. a. der empirische Nachweis der bit-exakten Inferenz. |

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
INTEGER_LLM_MODEL=qwen2.5-7b python bench/qualitativ.py 10
```

Die Modellwahl folgt derselben Umgebungsvariablen wie Kalibrierung und
Messung; ohne Angabe läuft er gegen `qwen2.5-0.5b`.

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

Lädt Qwen2.5-0,5B mit **fixierter Revision**. Für ein anderes Modell
steuert `MODEL_ID` die Auswahl (nicht `INTEGER_LLM_MODEL` — das Skript
spricht mit HuggingFace und braucht die dortige ID):

```bash
MODEL_ID=Qwen/Qwen2.5-7B scripts/fetch_model.sh
``` Die Fixierung ist kein
Detail: Eine andere Revision ergibt andere Gewichte, andere Artefakte und
einen anderen θ_v-Hash — der Vergleich mit den hier dokumentierten Zahlen
wäre hinfällig.

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
nach `artifacts/qwen2.5-0.5b/`. Dauert einige Minuten und braucht rund
0,8 GB Platz.

Für ein anderes Modell:

```bash
INTEGER_LLM_MODEL=qwen2.5-7b scripts/build_artifacts.sh
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
    artifacts/qwen2.5-0.5b "Die Hauptstadt von Frankreich ist" 10
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

Fährt 30 eingefrorene Testvektoren gegen das Referenz-Backend — von
einzelnen Kernen über ganze Layer bis zu vollständigen
Prompt-Durchläufen. **30/30 ist die Erwartung, nicht das Ziel.** Weicht
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

Voraussetzung für den Kalibrierungslauf ist das Quellmodell unter `models/`
(siehe `models/README.md`).

### Hardware-Teststrategie

Die Kerneigenschaft des Projekts ist **bit-identische Inferenz über alle
Hardware-Klassen hinweg**. Jedes Backend muss gegen die Referenz
(Rust, `reference`-Feature) validiert werden — Golden Vectors und
Konformitätspaket sind die normative Wahrheit.

| Hardware-Klasse | Backend | Test-Methode | Anforderung |
|---|---|---|---|
| **x86_64 CPU** | `reference`, `cpu-simd` (AVX2) | Lokal + CI | Keine (Standard-Runner) |
| **aarch64 CPU** | `cpu-simd` (NEON) | Lokal + CI | Apple Silicon oder ARM-Server |
| **NVIDIA GPU** | `cuda` | CI mit GPU-Runner | `ubuntu-latest-gpu` oder äquivalent |
| **AMD GPU** | `rocm` | CI mit GPU-Runner | AMD GPU + ROCm-Toolkit |

**Test-Prozedur pro Backend:**

```bash
# 1. Backend-spezifisch kompilieren
cargo build --features <backend-feature>

# 2. Paritätstests (SimdBackend vs. ReferenceBackend)
cargo test --features <backend-feature> --test test_backend_parity

# 3. Konformitätspaket (alle 30 Golden Vectors)
cd conformance && ./run.sh <backend-name>

# 4. E2E-Validierung (optional, benötigt Artefakte)
cd runtime && cargo run --bin golden_model --features <backend-feature> \
    -- ../artifacts/qwen2.5-0.5b --batch ../tests/golden/vectors
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

⛑ **Und sie hätte die Rechenabweichung trotzdem nicht gesehen.**
`rope_parity_basic` rechnet mit `q = 100` und `k = 50`; nach dem
Rechtsshift liegt jedes Zwischenergebnis weit im i16-Bereich, und dort
stimmen Abschneiden und Sättigen überein. Neu ist deshalb
`rope_parity_saettigung`, das die Werte so wählt, dass es überläuft, und
**zuerst prüft, dass sein eigener Fall wirklich sättigt**. Gegenprobe:
`vqmovn_s32` versuchsweise durch `vmovn_s32` ersetzt, dann fällt genau
dieser eine Test und die sechs alten bleiben grün.

⛑ **Zwei weitere Berichtigungen an derselben Datei.** Ihr Kopf behauptete,
auf ARM64 werde „der Fallback-Pfad (der identisch zur Referenz ist)"
geprüft; tatsächlich liefert `SimdBackend::detect()` dort NEON. Und
sechsmal stand `None => return` ohne Ausgabe: Auf einer x86_64-Maschine
ohne AVX2 lief die Datei vollständig durch und meldete sechs bestandene
Tests, ohne eine einzige Zusicherung zu prüfen. **Ein stiller Übersprung
sieht aus wie ein bestandener Test.**

⛑ **Berichtigung zur Reichweite von Fund 103.** Der Eintrag darunter
sagt, der AVX2-Pfad stürze „auf den meisten x86-CPUs" ab, und lässt
offen, wen es trifft. Genauer: `backends/simd.rs` ist über das
`Backend`-Trait erreichbar, und das ruft im Rechenpfad **niemand**;
`runtime/src/model.rs` importiert die Referenzkernel direkt. Getroffen
hätte es also `test_backend_parity.rs` auf einer Maschine ohne AVX-512,
nicht einen laufenden Miner. Der Fehler war echt, seine Reichweite war
kleiner als gemeldet.

### v0.28.0 (kernels 0.29.0) – 2026-08-30 (⚑ Fund 103: der AVX2-Pfad stürzt auf den meisten x86-CPUs ab)

> ⛑ **Zur Reichweite berichtigt am 2026-08-30, siehe v0.28.1:** Der Pfad
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
  vollständige θ_v-Artefakte in `artifacts/qwen2.5-0.5b/` — 168
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
- Referenzmodell auf die Basis-Variante festgelegt: `MODEL_NAME = "qwen2.5-0.5b"`,
  `HF_MODEL_ID = "Qwen/Qwen2.5-0.5B"` (der Code trug bisher Instruct-Strings,
  Whitepaper/Doku/lokales Modell benennen die Basis-Variante); model_configs-Schlüssel
  jetzt `"qwen2.5-0.5b"`, `fetch_model.sh`-Default angepasst
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