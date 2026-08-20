# Glossar

**Alle Fachbegriffe des Myelith-Protokolls — erklärt für Menschen, die
nicht alles davon studiert haben, und für Coding-Agenten, die sich im
Repository zurechtfinden müssen.**

*English edition: [`README/Glossary.en.md`](Glossary.en.md). Beide
Fassungen werden gemeinsam gepflegt; bei Abweichungen gilt die deutsche,
sie wird zuerst aktualisiert.*

---

## Wie dieses Glossar zu lesen ist

Myelith kreuzt drei Fachgebiete, die sonst selten aufeinandertreffen:
**verteilte Systeme** (Konsens, Byzantinische Fehler), **Kryptografie**
(Signaturen, Zufallslosungen, Beweise) und **maschinelles Lernen**
(Transformer, Quantisierung, Festkomma-Arithmetik). Fast niemand ist in
allen dreien zuhause. Dieses Glossar setzt deshalb **kein Vorwissen**
voraus und erklärt jeden Begriff so, dass er auch ohne die anderen
beiden Gebiete verständlich ist.

Jeder Eintrag hat dieselbe Form:

> **Begriff** — Was es ist, in einfachen Worten.
> *Beispiel:* Ein konkreter Fall zum Nachvollziehen.
> *Im Code:* Wo es implementiert ist.
> *Im Whitepaper:* Wo es hergeleitet wird.

Wo ein Begriff nur im Zusammenhang Sinn ergibt, steht der Zusammenhang
davor. Querverweise sind → so markiert.

**Für Coding-Agenten:** Die Abschnitte [A](#a-das-netzwerk-in-einem-absatz),
[B](#b-determinismus--warum-myelith-ganzzahlig-rechnet) und
[L](#l-arbeitsweise-des-projekts) sind die wichtigsten. A erklärt, wozu
das Ganze da ist; B erklärt die eine Entscheidung, aus der fast alles
andere folgt; L erklärt, wie in diesem Repository gearbeitet wird — und
warum bestimmte Regeln nicht verhandelbar sind. Wer nur einen Patch
schreiben will, liest A, B, L und dann den Abschnitt zur betroffenen
Komponente.

**Stand:** θ_v 0.17.0 · CONSENSUS Phase 1–4 · VERIFICATION Phase 1–2 ·
INTEGER_LLM Fahrplanpunkt 12.77. Diese Datei wird bei jeder Änderung an
Protokollbegriffen mitgezogen (→ [Sieben-Schritt-Doku-Kette](#sieben-schritt-doku-kette)).

---

## Inhalt

- [A. Das Netzwerk in einem Absatz](#a-das-netzwerk-in-einem-absatz)
- [B. Determinismus — warum Myelith ganzzahlig rechnet](#b-determinismus--warum-myelith-ganzzahlig-rechnet)
- [C. Festkomma und Quantisierung](#c-festkomma-und-quantisierung)
- [D. Das Sprachmodell von innen](#d-das-sprachmodell-von-innen)
- [E. Kryptografische Bausteine](#e-kryptografische-bausteine)
- [F. Konsens](#f-konsens)
- [G. Epochen, Scheduler und Pods](#g-epochen-scheduler-und-pods)
- [H. Verifikation](#h-verifikation)
- [Arbeitsnachweise: PoI und Epochenabschluss](#arbeitsnachweise-poi-und-epochenabschluss)
- [I. Tokenomik](#i-tokenomik)
- [J. Training](#j-training)
- [K. Agent Layer](#k-agent-layer)
- [L. Arbeitsweise des Projekts](#l-arbeitsweise-des-projekts)
- [M. Abkürzungen auf einen Blick](#m-abkürzungen-auf-einen-blick)

---

## A. Das Netzwerk in einem Absatz

Myelith ist ein Netzwerk aus fremden Rechnern, das gemeinsam ein großes
Sprachmodell betreibt. Kein einzelner Rechner hält das ganze Modell —
es ist in Abschnitte zerlegt (→ [Shard](#shard)), die auf verschiedene
Teilnehmer verteilt sind. Eine Nutzeranfrage wandert der Reihe nach
durch diese Abschnitte (→ [Pod](#pod), → [Pipeline](#pipeline-parallelismus))
und kommt am Ende als Antwort heraus.

Das schwierige Problem dabei ist nicht die Rechnung, sondern das
**Vertrauen**: Woher weiß das Netzwerk, dass ein Teilnehmer wirklich
gerechnet hat, statt eine billige Näherung oder schlicht Unsinn
zurückzugeben? Myelith beantwortet das mit einem Trick — es macht die
Rechnung **exakt reproduzierbar** (→ [Abschnitt B](#b-determinismus--warum-myelith-ganzzahlig-rechnet)),
lässt jede Anfrage von **zwei** unabhängig ausgelosten Pods rechnen und
vergleicht die Ergebnisse Bit für Bit. Wer abweicht, verliert seinen
hinterlegten Einsatz (→ [Slashing](#slashing)).

Der Rest des Systems ergibt sich daraus: Man braucht eine faire Auslosung
(→ [VRF](#vrf-verifiable-random-function)), eine Buchführung über
geleistete Arbeit (→ [PoI](#poi-proof-of-inference)), ein Verfahren für
Streitfälle (→ [Bisektions-Spiel](#bisektions-spiel)) und eine Währung,
die Nachfrage und Kapazität ins Gleichgewicht bringt
(→ [Burn-and-Mint](#burn-and-mint)).

---

### Schichtenmodell

Vier Ebenen, jede mit einem eigenen Verzeichnis im Repository.

| Ebene | Aufgabe | Komponenten |
|---|---|---|
| **L3 — Agent Layer** | Agentische Abläufe, Werkzeugaufrufe, Sessions | `AGENT_LAYER/` |
| **L2 — Compute Layer** | Modell-Shards, Pods, Pipeline, KV-Cache | `COMPUTE_PIPELINE/`, `INTEGER_LLM/` |
| **L1 — Consensus Layer** | BFT-Konsens, PoI-Aggregation, Staking, Ledger | `CONSENSUS/`, `TOKENOMICS/`, `VERIFICATION/` |
| **L0 — Networking Layer** | P2P-Gossip, Latenzmessung, verschlüsselte Kanäle | `NETWORKING/` |

**Die Kernentscheidung:** Der Konsens läuft *nicht* auf den
Inferenz-Ergebnissen, sondern auf kompakten Arbeitsnachweisen, die der
Compute Layer produziert. Dadurch bleibt die Blockzeit (1–2 s) unabhängig
davon, wie lange eine Inferenz dauert.

*Im Whitepaper:* Kap. 3.2

---

### Netzwerk-Rollen

**Shard-Miner** — Hält einen Modellabschnitt im GPU-Speicher und rechnet
Forward-Passes. Das ist die eigentliche Arbeit des Netzwerks.
*Beispiel:* Miner A hält die Layer 0–6, Miner B die Layer 7–13 usw.
*Im Code:* `COMPUTE_PIPELINE/myl-pod/src/shard.rs`

**Pod-Koordinator** — Ein gewählter Miner des Pods. Er sammelt Anfragen,
schickt sie durch die Pipeline, sammelt die Teilnachweise ein und reicht
am Epochenende das aggregierte → [PoI-Bündel](#poi-bündel) ein.
*Im Code:* `COMPUTE_PIPELINE/myl-pod/src/coordinator.rs`

**Validator** — Führt den → [BFT-Konsens](#bft-byzantine-fault-tolerance)
aus, prüft PoI-Stichproben, verwaltet Stake und Slashing. Braucht kaum
GPU, hauptsächlich CPU und gute Anbindung.
*Im Code:* `CONSENSUS/myl-consensus/src/validator.rs`

**Checker** (auch *Fisherman*) — Rechnet zufällig ausgeloste Segmente
nach und meldet Abweichungen. Verdient Kopfgeld aus geslashtem Stake.
Der Name „Fisherman" stammt aus Polkadot: jemand, der im Netz nach
Betrug fischt.
*Im Code:* `VERIFICATION/myl-verifier/src/checker.rs`

**Gateway** — Nimmt Nutzeranfragen entgegen, routet sie zu Pods, liefert
den Antwortstrom zurück. Bei → [externen Werkzeugen](#deterministische-vs-externe-werkzeuge)
ist das Gateway der Vertrauensanker.

**Nutzer** — Verbrennt MYL für → [Inferenz-Credits](#inferenz-credit-ic)
und stellt Anfragen.

*Im Whitepaper:* Kap. 3.3

---

## B. Determinismus — warum Myelith ganzzahlig rechnet

Dieser Abschnitt erklärt die zentrale technische Entscheidung des
Projekts. Wer nur einen Abschnitt liest, sollte diesen lesen.

### Das Problem: Gleitkommaaddition ist nicht assoziativ

**Gleitkommazahl (float)** — Die übliche Art, Kommazahlen im Computer
darzustellen: eine Mantisse und ein Exponent, wie in der
wissenschaftlichen Schreibweise (1,25 · 10³). Weil nur endlich viele
Stellen zur Verfügung stehen, wird nach **jeder** Rechenoperation
gerundet.

**Nichtassoziativität** — Bei exakten Zahlen gilt `(a + b) + c = a + (b + c)`.
Bei Gleitkommazahlen gilt das **nicht**, weil zwischendurch gerundet wird.

> *Beispiel zum Nachrechnen.* Mit nur drei signifikanten Stellen:
> `(1,00 + 0,004) + 0,004` → `1,00 + 0,004` → gerundet `1,00` → nochmal
> `+ 0,004` → `1,00`.
> Andersherum: `1,00 + (0,004 + 0,004)` → `1,00 + 0,008` → `1,01`.
> Dasselbe Ergebnis? Nein. Und das war eine einzige zusätzliche Addition.

Eine Matrixmultiplikation in einem Sprachmodell summiert **Tausende**
solcher Produkte. In welcher Reihenfolge eine GPU die Teilsummen
zusammenführt, hängt von der Kernel-Implementierung ab, von der
Blockaufteilung, von der Anzahl der Recheneinheiten — Dinge, die sich
zwischen zwei Grafikkarten unterscheiden. **Zwei ehrliche Knoten erhalten
deshalb verschiedene Bits.** Damit ist ein Bit-Vergleich als
Betrugserkennung wertlos: Man kann Abweichung durch Betrug nicht von
Abweichung durch andere Hardware unterscheiden.

### Die Lösung: Ganzzahladdition *ist* assoziativ

Bei ganzen Zahlen gibt es kein Runden zwischen den Schritten.
`(3 + 5) + 7 = 3 + (5 + 7) = 15`, immer, auf jeder Maschine, in jeder
Reihenfolge. Rechnet man die gesamte Inferenz ganzzahlig, ist
Bitgleichheit **keine Auflage an die Ausführung, sondern eine
Eigenschaft der Arithmetik**.

Das ist der Kern des Entwurfs, und er hat eine Konsequenz, die leicht
übersehen wird: Myelith muss der Hardware **nicht** vorschreiben, in
welcher Reihenfolge sie rechnet. Blockgrößen, Parallelisierung,
Kernel-Wahl, Speicherlayout bleiben frei. Genau das erlaubt heterogener
Hardware die Teilnahme ohne Wettbewerbsnachteil.

### Was verbindlich ist — und was nicht

**Verbindlich** (Teil von → [θ_v](#θ_v-theta-v-modellversion)):

1. **Vollständig ganzzahlige Ausführung**, einschließlich der
   nichtlinearen Funktionen (Softmax, SiLU, RMSNorm).
2. **Akkumulatorbreite und Bitbreite je Tensor**, plus explizites
   Überlaufverhalten (→ [Sättigung](#sättigung-saturation-clamping)).
3. **Division ausschließlich als arithmetischer Rechtsshift**
   (→ [Rechtsshift](#arithmetischer-rechtsshift)).

**Frei wählbar:** Reduktionsreihenfolge, Blockaufteilung,
Kernel-Implementierung, Matrixeinheiten, Speicherlayout.

*Im Whitepaper:* Kap. 6.2, 6.5, Anhang B.5
*Im Code:* `INTEGER_LLM/kernels/src/` — kein `f32`/`f64` im Rechenpfad,
das wird nach jedem Patch aktiv geprüft (→ [Ganzzahligkeitsprüfung](#ganzzahligkeitsprüfung)).

---

### Arithmetischer Rechtsshift

Die einzig erlaubte Form der Division im Rechenpfad. `x >> n` verschiebt
alle Bits um n Stellen nach rechts, was einer Division durch 2ⁿ
entspricht — mit **Abrundung Richtung minus unendlich**.

*Beispiel:* `-7 >> 1 = -4` (nicht `-3`). Eine gewöhnliche
Ganzzahldivision `-7 / 2` liefert je nach Programmiersprache `-3`
(Trunkierung zur Null, so in C, Rust, Java) oder `-4` (Abrundung, so in
Python). **Diese Uneinigkeit ist die letzte verbliebene Quelle
plattformabhängiger Ergebnisse** — der arithmetische Rechtsshift ist
dagegen auf allen gängigen Architekturen identisch definiert.

*Im Code:* `INTEGER_LLM/kernels/src/fixed_point.rs` — `rshift_round`,
`rshift_round_i64`, `rshift_round_i128`. Diese Varianten runden zur
nächsten Ganzzahl statt abzurunden (das halbe LSB wird addiert), was den
systematischen Abwärtsdrift über viele Schichten vermeidet; die
Rundungsregel selbst ist Teil von θ_v und damit für alle gleich.

### Sättigung (Saturation, Clamping)

Was passiert, wenn ein Ergebnis den Wertebereich sprengt? Zwei
Möglichkeiten: **Umlauf** (wrap-around, aus 32 768 wird −32 768 — ein
Vorzeichenwechsel aus dem Nichts) oder **Sättigung** (der Wert bleibt
bei 32 767 stehen). Myelith schreibt Sättigung vor, weil ein
abgeschnittener Wert eine harmlose Ungenauigkeit ist, ein
vorzeichenverkehrter dagegen die Rechnung zerstört.

*Im Code:* `clamp_i16`, `clamp_i32`, `clamp_i8`, `clamp_i16_from_i64` in
`kernels/src/fixed_point.rs`

### Bitgleichheit / bit-exakt

Zwei Ergebnisse sind bitgleich, wenn ihre Byte-Darstellung identisch ist
— nicht „fast gleich", nicht „innerhalb einer Toleranz", sondern
identisch. Der Vergleich ist damit **binär und parameterfrei**: Es gibt
keine Schwelle, die kalibriert, angegriffen oder per Governance
verschoben werden könnte.

**Warum kein Toleranzvergleich?** Das wurde geprüft und verworfen:
Rechenrauschen akkumuliert über verkettete Ausführung so weit, dass zwei
ehrliche Knoten nach wenigen Layern so weit auseinanderliegen wie ein
manipulierter von einem unmanipulierten. Und entscheidend: **Ein
Toleranzband ist adaptiv angreifbar.** Wer das Prüfkriterium kennt,
richtet die Manipulation daran aus. In der Simulation blieb eine
Manipulation, die nur die geprüften Komponenten korrekt berechnete, über
zehn nachfolgende Layer unentdeckt.

*Im Whitepaper:* Kap. 6.3

---

## C. Festkomma und Quantisierung

Wenn nicht mit Gleitkomma gerechnet werden darf, wie stellt man dann
Zahlen wie 0,375 dar? Antwort: als ganze Zahl mit einem **vereinbarten
Nenner**.

### Festkomma-Arithmetik (Fixed Point)

Man einigt sich darauf, dass eine gespeicherte Ganzzahl in Wahrheit
Vielfache von 1/2ⁿ bedeutet. Die Zahl n heißt **frac_bits**
(Nachkommabits).

*Beispiel:* Bei `frac_bits = 8` bedeutet die gespeicherte Zahl `96` in
Wirklichkeit `96 / 256 = 0,375`. Die gespeicherte `256` bedeutet `1,0`.
Die Auflösung ist `1/256 ≈ 0,0039` — feiner geht es nicht, alles
dazwischen wird gerundet.

Multiplikation zweier Festkommazahlen addiert die frac_bits: `a` mit 8
Nachkommabits mal `b` mit 7 Nachkommabits ergibt ein Produkt mit 15
Nachkommabits, das anschließend per → [Rechtsshift](#arithmetischer-rechtsshift)
auf die Zielskala gebracht wird. Genau das macht `rescale`.

*Im Code:* `INTEGER_LLM/kernels/src/fixed_point.rs` — `rescale`,
`rescale_i64`, `mul_i16_i64`

### Skala (scale) und Zweierpotenz-Skalen

Die **Skala** eines Tensors ist der Faktor, mit dem eine gespeicherte
Ganzzahl in den echten Wert übersetzt wird. Myelith benutzt
ausschließlich **Zweierpotenz-Skalen** (1/2, 1/4, 1/8 …), weil die
Umrechnung dann ein Shift ist und keine Division — und Division wäre
plattformabhängig (siehe oben).

### Quantisierung

Die Umwandlung eines in Gleitkomma trainierten Modells in ein
ganzzahliges. Für jeden Gewichtstensor wird bestimmt, welche Skala seine
Werte am besten trifft, dann werden alle Werte darauf gerundet.

*Beispiel:* Ein Gewicht `0,0731` bei Skala `1/1024` wird zu
`round(0,0731 × 1024) = 75`. Zurückgerechnet: `75/1024 = 0,07324` —
der Quantisierungsfehler ist `0,00014`.

*Im Code:* `INTEGER_LLM/calibrate/src/quantize.py`, `scales.py`

### W8A16

Das Quantisierungsschema von Myelith: **W**eights 8 Bit, **A**ctivations
16 Bit. Gewichte sind `int8` (−128 … 127), Aktivierungen `int16`
(−32 768 … 32 767).

**Warum nicht beides 8 Bit?** Weil reale Aktivierungswerte den
int8-Bereich sprengen — gemessen wurden RMSNorm- und MLP-Ausgaben bis
etwa ±1640. Bei int8 wäre das hartes Abschneiden auf 127, also ein
Faktor 13 Verlust.

**Warum Gewichte nur 8 Bit?** Speicher. Ein 7-Milliarden-Modell braucht
in int8 rund 7 GB, in int16 wären es 14 — das entscheidet darüber,
welche Grafikkarten teilnehmen können.

*Im Code:* `INTEGER_LLM/kernels/src/linear.rs` — `linear_w8a16`

### Per-Channel-Skalen

Statt einer Skala pro Tensor bekommt **jede Ausgabezeile** ihre eigene.
Das ist wichtig, weil die Wertebereiche innerhalb eines Tensors stark
schwanken: Eine Zeile mit Werten bis 0,01 und eine mit Werten bis 5,0
teilen sich sonst eine Skala, und die feine Zeile verliert fast ihre
gesamte Auflösung.

*Im Code:* `linear_w8a16_pc` in `kernels/src/linear.rs`;
die Shifts stehen als `shifts`-Array im Artefakt
(`runtime/src/loader.rs`, `QTensor::shifts`)

### Massive Activations / Ausreißerkanäle

Transformer haben die Eigenschaft, dass einzelne Dimensionen der
Aktivierungen dauerhaft **hundertfach größere** Beträge tragen als alle
anderen (in der Literatur *massive activations* oder *attention sinks*).
Wer per-Tensor quantisiert, richtet die Skala nach diesem Ausreißer aus
und verschenkt für alle übrigen Kanäle sämtliche Auflösung. Das ist der
Hauptgrund für Per-Channel-Skalen.

*Praktische Folge für Messungen:* Die Fehlerkennzahl **absmax** (größte
Abweichung) verfolgt nur diesen einen Ausreißerkanal und sagt fast
nichts über den Rest. Deshalb misst dieses Projekt Fehler als
**relativen L2** über den gesamten Vektor (→ [relativer L2](#relativer-l2)).

### Kalibrierung

Der Vorgang, bei dem die Skalen bestimmt werden. Man schickt echten Text
(hier: 64 WikiText-2-Sequenzen) durch das Gleitkomma-Modell, misst an
jeder Stelle die tatsächlich auftretenden Wertebereiche und wählt daraus
die Skalen.

**Wichtig:** Die Kalibrierung ist der einzige Ort im Projekt, an dem
Gleitkomma-Arithmetik erlaubt ist — sie ist Vorbereitung, nicht
Rechenpfad. Das Ergebnis sind ganzzahlige Artefakte.

*Im Code:* `INTEGER_LLM/calibrate/src/main.py`
*Aufruf:* `INTEGER_LLM_MODEL=qwen2.5-7b python -m calibrate.src.main`

### GPTQ

Ein Verfahren, das Quantisierungsfehler nicht nur misst, sondern
**kompensiert**: Der Fehler eines gerundeten Gewichts wird auf die noch
nicht quantisierten Nachbargewichte umgelegt. Es ist Stand der Technik
und teuer (Stunden statt Minuten).

**Bei uns standardmäßig ABGESCHALTET.** Grund: In einer Vergleichsmessung
mit drei Kalibrierungsläufen erwies sich GPTQ auf unserem Pfad als exakt
neutral — es brachte nichts, kostete aber 2,5 Stunden je 7B-Lauf statt
20 Minuten. Solange die Fehlerquelle woanders liegt, ist ein Verfahren,
das den *Gewichts*fehler verbessert, wirkungslos. Einschaltbar mit
`INTEGER_LLM_GPTQ=1` für die abschließende Artefakt-Erzeugung.

*Im Code:* `INTEGER_LLM/calibrate/src/gptq.py`, Entscheidung in
`main.py::gptq_entscheidung()`

### LUT (Lookup Table, Nachschlagetabelle)

Nichtlineare Funktionen wie exp, SiLU oder die reziproke Wurzel lassen
sich nicht mit Addieren und Shiften ausdrücken. Statt sie zu berechnen,
werden sie **vorab tabelliert**: Der Eingangswert wird zum Index, der
Tabelleneintrag ist das Ergebnis.

*Beispiel:* Die exp-LUT bei `exp_input_frac_bits = 8` hat für Index `i`
den Eintrag `round(exp(-i/256) · 2¹⁴)`. Um `exp(-0,5)` zu bekommen,
schlägt man Index `128` nach.

Eine LUT hat zwei getrennte Auflösungen, die man nicht verwechseln darf:
- **Eingangsraster** (`input_frac_bits`) — wie fein die x-Achse abgetastet ist,
- **Ausgangsauflösung** (`output_frac_bits`) — wie fein das Ergebnis dargestellt wird.

Beide sind Teil von θ_v.

*Im Code:* `kernels/src/integer_math.rs::lut_lookup`, Erzeugung in
`calibrate/src/luts.py`

### Relativer L2

Die Fehlerkennzahl dieses Projekts: die Länge des Differenzvektors,
geteilt durch die Länge des Referenzvektors, in Prozent.

```
rel_L2 = 100 · ‖ganzzahlig − gleitkomma‖ / ‖gleitkomma‖
```

Sie misst den **Bulk-Fehler** über alle Kanäle, nicht den größten
Einzelausschlag. Genau deshalb wird sie hier benutzt: absmax würde nur
den → [Ausreißerkanal](#massive-activations--ausreißerkanäle) verfolgen.

### Perplexität

Das Qualitätsmaß für Sprachmodelle: Wie „überrascht" ist das Modell vom
nächsten echten Wort? Niedriger ist besser. Perplexität 9 heißt
sinngemäß: Das Modell ist so unsicher, als müsste es zwischen 9
gleich wahrscheinlichen Wörtern raten.

Myelith misst nicht die absolute Perplexität, sondern den **relativen
Anstieg** gegenüber dem Gleitkomma-Original. Das Akzeptanzkriterium
lautet ≤ 5 % und ist seit θ_v 0.17.0 auf beiden Modellen erfüllt:
0,5B **+2,11 %**, 7B **+1,14 %**. Zum Vergleich: Der Boden des
Quantisierungsschemas selbst — alles float außer der
→ [W8A16](#w8a16)-Quantisierung — liegt bei **+0,84 %**. Der Abstand von
0,30 Punkten ist der gesamte verbleibende Umsetzungsverlust.

*Im Code:* `INTEGER_LLM/eval/perplexity.py`

### θ_v (Theta-v, Modellversion)

Die vollständige Ausführungsspezifikation: **Gewichte + Quantisierungsschema
+ alle arithmetischen Festlegungen**. θ_v ist konsensrelevant — alle
Knoten müssen dieselbe Fassung benutzen, sonst weichen die Hashes
auseinander, und der Redundanzvergleich schlägt fehl, ohne dass jemand
betrogen hat.

Enthalten sind: Bitbreiten, Akkumulatorbreite, Überlaufverhalten,
LUT-Koeffizienten und -Raster, die Regeln der dynamischen Quantisierung,
die Festlegung auf den arithmetischen Rechtsshift.

**Nicht enthalten** und damit frei: Kernel-Implementierung,
Parallelisierungsstrategie, Blockgrößen, Speicherlayout.

*Im Code:* `INTEGER_LLM/theta_v/spec.json`, geprüft beim Laden in
`runtime/src/loader.rs::verify_version_against_spec`
*Im Whitepaper:* Kap. 6.1, 6.5

---

## D. Das Sprachmodell von innen

Dieser Abschnitt erklärt, was in einem Transformer tatsächlich passiert
— soweit nötig, um die Implementierung in `INTEGER_LLM/` zu verstehen.

### Token und Tokenizer

Ein **Token** ist ein Textbaustein — meist ein Wortteil, nicht ein
ganzes Wort. „Unverständlichkeit" könnte in `Un`, `ver`, `ständ`, `lich`,
`keit` zerfallen. Der **Tokenizer** übersetzt Text in Token-Nummern und
zurück.

*Beispiel:* `" The history of"` → `[576, 3840, 315]`

*Im Code:* `INTEGER_LLM/runtime/src/tokenizer.rs` (BPE über die
HuggingFace-`tokenizers`-Crate; der Encoding-Pfad ist float-frei)

### Embedding

Eine Nachschlagetabelle, die jede Token-Nummer in einen Vektor
übersetzt — bei Qwen2.5-0.5B in 896 Zahlen, bei 7B in 3584. Dieser
Vektor ist der Startzustand, den die Schichten dann Schritt für Schritt
umformen.

*Im Code:* `runtime/src/model.rs::embed_token`

### Layer / Schicht

Der Transformer besteht aus gleichartigen Schichten (0,5B: 24 Stück,
7B: 28). Jede Schicht hat denselben Aufbau:

```
RMSNorm → Attention → Residual-Addition
       → RMSNorm → MLP       → Residual-Addition
```

*Im Code:* `runtime/src/model.rs::forward_layer`

### Residualstrom / Residual-Addition

Das Ergebnis jeder Teiloperation wird nicht ersetzt, sondern **addiert**:
`x = x + attention(norm(x))`. Der durchlaufende Vektor heißt
**Residualstrom**. Ohne diese Abkürzung wären tiefe Netze nicht
trainierbar; für uns ist die Bedeutung eine andere: Der Residualstrom
ist der Kanal, über den sich Quantisierungsfehler durch alle Schichten
fortpflanzen und aufsummieren.

> **Warum die Reihenfolge von Klemmen und Addieren zählt (Fund 31).**
> Der eingehende Residualstrom und der Blockbeitrag liegen auf
> verschiedenen Skalen. Wer den einen **erst** auf die Zielskala klemmt
> und **dann** addiert, zerstört jede Auslöschung: Beide Operanden können
> groß sein, während nur ihre Summe klein ist — und die Zielskala ist nach
> der Summe kalibriert.
>
> Gemessen an Qwen2.5-0,5B, Ebene 21, Kanal 62 (dem Kanal mit der
> → [massive activation](#massive-activations--ausreißerkanäle)): Der
> wahre Wert fällt dort von 1714 auf 61,6. Die alte Fassung rechnete
> `1723 → geklemmt 64,00` plus `−1653 → geklemmt −64,00` und kam auf
> **−0,002**. Zwei Klemmungen, die einander aufheben.
>
> Seit θ_v 0.17.0 wird auf der **gröberen** der beiden Skalen in i64
> addiert und **einmal** am Ende reskaliert und geklemmt: 63,998 statt
> −0,002. Die Regel dahinter ist allgemein und gilt für jede
> Festkomma-Rechnung: **breit akkumulieren, einmal runden.**

### RMSNorm

Eine Normalisierung: Der Vektor wird durch seinen quadratischen
Mittelwert geteilt, damit die Beträge über die Schichten hinweg stabil
bleiben. Dann wird er kanalweise mit gelernten Faktoren (γ, *gamma*)
multipliziert.

```
y = x / sqrt(mean(x²) + ε) · γ
```

Die Wurzel ist das ganzzahlige Problem — sie wird über eine
→ [rsqrt-LUT](#lut-lookup-table-nachschlagetabelle) gelöst.

*Im Code:* `kernels/src/rmsnorm.rs::rmsnorm_i16`

### Attention (Aufmerksamkeit)

Der Mechanismus, mit dem jede Position im Text auf frühere Positionen
zurückgreift. Drei Größen werden aus dem Eingang berechnet:

- **Query (q)** — „Wonach suche ich?"
- **Key (k)** — „Was biete ich an?"
- **Value (v)** — „Was gebe ich weiter, wenn ich gewählt werde?"

Dann: Jeder Query wird mit jedem Key verrechnet (Skalarprodukt) → das
ergibt **Scores**. Die Scores gehen durch → [Softmax](#softmax) und
werden zu Gewichten, die sich zu 1 summieren. Das Ergebnis ist die
gewichtete Summe der Values.

> *Beispiel.* Im Satz „Die Katze, die auf der Matte saß, war müde"
> muss das Modell bei „war" wissen, wer müde ist. Der Query von „war"
> passt gut zum Key von „Katze" → hohes Gewicht → der Value von „Katze"
> dominiert die Ausgabe.

**Causal** heißt: Eine Position darf nur nach hinten schauen, nie nach
vorn. Das wird über eine Maske erzwungen (maskierte Positionen bekommen
Score `i32::MIN`, also Gewicht 0).

*Im Code:* `kernels/src/attention.rs::attention_int`

### Head (Kopf) und GQA

Attention läuft nicht einmal, sondern parallel in mehreren **Köpfen**,
die je einen Ausschnitt des Vektors bearbeiten und unterschiedliche
Beziehungen lernen. `head_dim` ist die Größe eines Kopfes (7B: 128).

**GQA** (Grouped Query Attention): Es gibt mehr Query-Köpfe als
Key/Value-Köpfe; mehrere Query-Köpfe teilen sich einen KV-Kopf. Das
spart Speicher im → [KV-Cache](#kv-cache). Qwen2.5-0.5B: 14 Query-Köpfe,
2 KV-Köpfe.

*Im Code:* `runtime/src/model.rs`, `split_heads` + `group_size`

### RoPE (Rotary Position Embedding)

Wie erfährt das Modell, an welcher **Stelle** im Text ein Token steht?
RoPE dreht die q- und k-Vektoren um einen Winkel, der von der Position
abhängt. Zwei Positionen mit gleichem Abstand haben dieselbe relative
Verdrehung — dadurch codiert das Skalarprodukt q·k automatisch den
Abstand.

Jedes Dimensions-**Paar** j hat seine eigene Frequenz
`θ_j = 1/rope_theta^(j/(head_dim/2))`; die Paarung ist *half-split*, also
`(x_j, x_{j+head_dim/2})`.

*Warum das hier steht:* Eine frühere Fassung benutzte **einen** Winkel
für alle Paare und benachbarte Paarung. Das war die dominante
Fehlerquelle der Perplexität (→ Fund 15).

*Im Code:* `kernels/src/rope.rs::rotate_half_split_i16`

### Softmax

Verwandelt beliebige Zahlen in Wahrscheinlichkeiten, die sich zu 1
summieren: `softmax(z)_i = exp(z_i) / Σ exp(z_j)`.

Ganzzahlig wird das so gelöst: Vom größten Score wird jeder Score
abgezogen (die Differenz ist ≥ 0), das Ergebnis indiziert die exp-LUT,
dann wird durch die Summe geteilt.

> **Warum die Auflösung hier mehr zählt, als sie aussieht (Fund 29).**
> Bei `prob_frac_bits = 8` ist das feinste darstellbare Gewicht 1/256.
> Jede Position, deren Gewicht darunter liegt, rundet **einzeln auf null**
> und trägt exakt nichts bei — gleichgültig, wie viele solcher Positionen
> es gibt. Bei einem dominanten Peak und flachem Schwanz:
>
> | Kontextlänge | Schwanzgewicht bei 1/256 | bei 1/16384 | exakt |
> |---|---|---|---|
> | 128 | 0,4961 | 0,2403 | 0,2394 |
> | 512 | **0,0000** | 0,5614 | 0,5588 |
> | 2048 | **0,0000** | 0,8746 | 0,8354 |
>
> Bei 512 Positionen verschwindet der gesamte Schwanz: Die Aufmerksamkeit
> kollabiert auf die Spitzenposition. Auf 128-Token-Sequenzen — der Länge
> unserer Perplexitätsmessung — ist der Effekt nur eine Verdopplung und
> bleibt unter der Messschwelle. **Ein Defekt, den die Auswertung nicht
> sehen kann, ist trotzdem ein Defekt.** Behoben in θ_v 0.16.0.

*Im Code:* `kernels/src/softmax.rs::softmax_int`

### MLP / Feed-Forward und SiLU

Der zweite Block jeder Schicht. Bei Qwen2.5 ein **Gated MLP**:

```
gate = W_gate · x      up = W_up · x
y = W_down · (SiLU(gate) ⊙ up)
```

**SiLU** (auch *Swish*) ist die Nichtlinearität: `SiLU(x) = x · σ(x)` mit
der Sigmoid-Funktion σ. Ganzzahlig als LUT gelöst.

*Warum das hier steht:* Der operationsweise Vergleich zeigte, dass die
Matrixmultiplikationen des MLP praktisch exakt sind (0,01 %), der
**gesamte** MLP-Fehler aber in der SiLU-LUT entstand (6,83 %). Das
Anheben der LUT-Auflösung in θ_v 0.15.0 war einer der beiden großen
Fortschritte des Projekts.

*Im Code:* `kernels/src/mlp.rs::mlp_int`

### KV-Cache

Beim Erzeugen von Text wird Token für Token gerechnet. Ohne Cache müsste
für jedes neue Token die gesamte bisherige Sequenz neu durch die
Attention. Der **KV-Cache** speichert die schon berechneten Keys und
Values, sodass pro Schritt nur die neue Position hinzukommt.

*Wichtige Festlegung (Fund 22):* Der Cache hält K/V in der **nativen
Per-Layer-Skala** der erzeugenden Projektion, ohne Umrechnung auf eine
globale Cache-Skala. Die frühere Rundreise `k_frac → 8 → k_frac` gewann
nichts, kostete aber doppelte Rundung und 2–4 Bit Auflösung auf fast
jeder Ebene — plus hartes Abschneiden, wo der reale Wert die feste
Kapazität überstieg (7B Ebene 0: K-absmax 420, Faktor 3,28 verloren, und
das an der *ersten* Ebene, deren Fehler durch alle 28 propagiert).

*Im Code:* `runtime/src/kv_cache.rs`

### Prefill und Decode

**Prefill** — der Prompt wird verarbeitet; alle Positionen können parallel
gerechnet werden. **Decode** — die Antwort wird erzeugt; jedes Token
hängt vom vorigen ab, also strikt sequentiell. Decode ist der langsame
Teil und der Grund, warum → [Micro-Batching](#micro-batching) beim
Durchsatz hilft.

*Im Code:* `runtime/src/generate.rs`

### LM Head und Logits

Die letzte Schicht projiziert den Vektor auf die Größe des Vokabulars
(bei Qwen2.5: 151 936). Die Ausgabewerte heißen **Logits**; das größte
ist das wahrscheinlichste nächste Token.

### Sampling

Wie wird aus den Logits ein Token? **Greedy** nimmt das größte
(`argmax_int`). **CDF-Sampling** würfelt gewichtet — und zwar mit einem
deterministischen PRNG (`splitmix64`), dessen Seed aus `segment_id` und
`block_hash` abgeleitet wird. Damit ist auch das Würfeln reproduzierbar,
was für den Redundanzvergleich zwingend ist.

*Im Code:* `kernels/src/sampling.rs`, `kernels/src/prng.rs`

---

## E. Kryptografische Bausteine

### Hash / SHA-256

Eine Einwegfunktion, die aus beliebig vielen Bytes 32 Bytes macht.
Dieselbe Eingabe ergibt immer denselben Hash; eine Eingabe zu finden, die
einen *vorgegebenen* Hash ergibt, ist praktisch unmöglich.

*Wozu hier:* Statt Aktivierungen (Megabytes) zu übertragen und zu
vergleichen, vergleicht man ihre Hashes (32 Bytes). Das ist der Grund,
warum die → [Berechnungsspur](#berechnungsspur) klein bleibt.

**Konstantzeit-Vergleich:** Der Gleichheitsvergleich läuft über
`subtle::ConstantTimeEq`, damit die Vergleichsdauer keine Information
darüber trägt, an welcher Byte-Position zwei Hashes auseinandergehen.
Bei einem Open-Source-Protokoll, dessen Code der Angreifer kennt, ist
das kein Luxus.

*Im Code:* `SHARED_TYPES/myl-types/src/hash.rs`

### Merkle-Baum

Eine Baumstruktur aus Hashes: Die Daten stehen in den Blättern, jeder
innere Knoten ist der Hash seiner beiden Kinder, ganz oben steht die
**Wurzel** (root). Der Nutzen: Man kann beweisen, dass ein bestimmtes
Blatt zum Baum gehört, ohne den ganzen Baum zu zeigen — es genügt der
Pfad von Blatt zu Wurzel (**Merkle-Beweis**, `log₂(n)` Hashes).

> *Beispiel.* Bei einem Korpus aus einer Milliarde Dokumenten braucht der
> Beweis, dass Dokument Nr. 734 891 202 dazugehört, rund 30 Hashes —
> also unter einem Kilobyte statt Terabytes.

**Domain-Separation:** Blätter und innere Knoten werden mit
unterschiedlichen Präfixen gehasht. Ohne das könnte ein Angreifer einen
inneren Knoten als Blatt ausgeben und damit falsche Beweise bauen.

*Wozu hier:* θ_v-Wurzel, PoI-Bündel-Wurzeln, Korpus-Provenienz beim
Training.

*Im Code:* `SHARED_TYPES/myl-types/src/merkle.rs`

### Signatur und BLS12-381

Eine **digitale Signatur** beweist, dass eine Nachricht von dem stammt,
der den geheimen Schlüssel besitzt. **BLS12-381** ist ein
Signaturverfahren auf einer elliptischen Kurve mit einer besonderen
Eigenschaft: Signaturen lassen sich **aggregieren**.

*Beispiel:* 21 Validatoren signieren denselben Block. Statt 21
Signaturen (21 × 96 Bytes) speichert der Block **eine** aggregierte
Signatur (96 Bytes), die gegen alle 21 öffentlichen Schlüssel auf einmal
geprüft wird. Bei einer Blockchain, in der jeder Block dauerhaft
gespeichert wird, ist das ein erheblicher Unterschied.

Myelith nutzt die **min-pk**-Variante (Public Keys auf G1, 48 Bytes;
Signaturen auf G2, 96 Bytes) — dieselbe wie der Ethereum-Konsens.

*Im Code:* `SHARED_TYPES/myl-types/src/bls.rs`

### Rogue-Key-Angriff und Proof-of-Possession

Der Preis der Aggregierbarkeit. Wer einen öffentlichen Schlüssel frei
wählen darf, kann ihn so konstruieren, dass er die Signaturen anderer
„mit übernimmt":

```
pk_rogue = g₁^x · pk_opfer⁻¹
```

Mit diesem Schlüssel kann der Angreifer eine aggregierte Signatur
erzeugen, die für die Gruppe `{opfer, angreifer}` gültig aussieht,
obwohl das Opfer nie signiert hat.

**Gegenmittel: Proof-of-Possession (PoP).** Jeder Teilnehmer muss beim
Registrieren einmalig seinen **eigenen** öffentlichen Schlüssel
signieren — unter einem separaten Domain-Separation-Tag. Wer seinen
Schlüssel als Kombination fremder Schlüssel konstruiert hat, besitzt den
zugehörigen geheimen Schlüssel nicht und kann diesen Beweis nicht
erzeugen.

Für Myelith ist das nicht optional: `fast_aggregate_verify` ist genau die
Funktion, die ohne PoP angreifbar ist, und sie wird bei jeder
PoI-Bündel-Prüfung benutzt.

*Im Code:* `bls.rs::prove_possession` / `verify_possession`,
Tests in `SHARED_TYPES/myl-types/tests/rogue_key.rs` — dort wird
**sowohl die Verwundbarkeit als auch ihre Behebung** ausdrücklich
getestet, damit ein späterer Umbau nicht stillschweigend den Schutz
entfernt.

### VRF (Verifiable Random Function)

Eine Zufallsfunktion mit Beweis. Wer den geheimen Schlüssel hat, kann
aus einer Eingabe eine Zufallsausgabe erzeugen **plus einen Beweis**,
dass sie korrekt gebildet wurde. Jeder andere kann den Beweis prüfen,
ohne den Schlüssel zu kennen.

*Wozu hier:* Wenn ausgelost wird, wer welches Segment rechnet, darf
niemand die Auslosung beeinflussen — aber alle müssen sie nachprüfen
können. Genau das leistet eine VRF.

Myelith nutzt **ECVRF-EDWARDS25519-SHA512-TAI** nach RFC 9381, geprüft
gegen die offiziellen Testvektoren aus Anhang B.3.

*Im Code:* `SHARED_TYPES/myl-types/src/vrf.rs`

### Erasure-Codierung (Reed-Solomon, Cauchy-Form)

Ein Verfahren, um Daten so aufzuteilen, dass sie den Verlust einzelner
Teile überstehen. Bei `k = 8` Datenfragmenten und `m = 4`
Paritätsfragmenten entstehen 12 Stücke, und **beliebige 8 davon**
genügen zur vollständigen Rekonstruktion.

> *Beispiel.* Ein Pod archiviert die Aktivierungen eines Segments als 12
> Fragmente bei 12 verschiedenen Knoten. Fallen vier davon aus — gleich
> welche vier — lässt sich das Original wiederherstellen. Erst beim
> fünften Ausfall ist es verloren.

**Cauchy statt Vandermonde:** Beide Konstruktionen erzeugen
Reed-Solomon-Codes, aber bei der Cauchy-Form ist **jede** k×k-Untermatrix
garantiert invertierbar. Bei der Vandermonde-Form gilt das über GF(2⁸)
nicht für alle Teilmengen — es gäbe also Kombinationen von 8 Fragmenten,
aus denen die Rekonstruktion fehlschlägt. Für eine Streitschlichtung, bei
der ein Angreifer sich aussuchen darf, welche Fragmente er zurückhält,
ist das der Unterschied zwischen sicher und unsicher.

*Verifikation:* Der Test prüft **alle 495 Teilmengen** von 8 aus 12.

*Im Code:* `SHARED_TYPES/myl-types/src/erasure.rs`

### GF(2⁸) (Galois-Feld)

Die Arithmetik, in der Reed-Solomon rechnet: 256 Elemente (also genau ein
Byte), mit Addition = XOR und einer eigenen Multiplikation. Der Vorteil:
Alle Operationen sind exakt, es gibt keine Rundung und kein Überlaufen —
Byte-Arithmetik, die sich wie Körperarithmetik verhält.

### Borsh

Das Serialisierungsformat des Protokolls (*Binary Object Representation
Serializer for Hashing*). Entscheidend ist seine **Kanonizität**: Zu
jedem Wert gibt es genau eine Bytefolge. Bei JSON könnte man Felder
umordnen oder Leerzeichen einfügen und bekäme einen anderen Hash bei
gleichem Inhalt — für ein System, das Hashes vergleicht, wäre das fatal.

**Folge für Entwickler:** Borsh serialisiert in
**Deklarationsreihenfolge**. Das Umsortieren von Struct-Feldern ändert
alle Hashes über dieser Struktur und ist damit ein Konsens-Bruch, kein
Refactoring.

*Im Code:* `SHARED_TYPES/myl-types/src/core_types.rs`

### Domain-Separation-Tag (DST)

Ein Präfix, das vor das Signieren oder Hashen gesetzt wird, damit
Signaturen aus einem Zusammenhang in einem anderen nicht gelten.

> *Beispiel.* Ohne DST wäre eine Vote-Signatur für Runde 5 auch eine
> gültige Commit-Signatur für Runde 5 — ein Angreifer könnte eine
> Zustimmung in eine Festlegung umdeuten. Mit getrennten Tags
> (`MYL_PROPOSE_v1`, `MYL_VOTE_v1`, `MYL_COMMIT_v1`) ist das
> ausgeschlossen.

*Im Code:* `CONSENSUS/myl-consensus/src/signing.rs`

---

## F. Konsens

### Byzantinischer Fehler

Ein Knoten, der nicht einfach ausfällt, sondern sich **beliebig
bösartig** verhält: widersprüchliche Nachrichten an verschiedene
Empfänger, gefälschte Werte, gezieltes Schweigen. Der Name stammt vom
*Byzantine Generals Problem* (Lamport et al., 1982).

### BFT (Byzantine Fault Tolerance)

Ein Konsensverfahren, das korrekt bleibt, solange weniger als ein Drittel
der Teilnehmer byzantinisch ist. Warum ein Drittel? Weil man mit `n`
Knoten und `f` Fehlerhaften nur dann sicher entscheiden kann, wenn
`n > 3f` — sonst kann eine Menge von `n − f` Antworten zweimal so
zustande kommen, dass sich die Ergebnisse widersprechen.

**Bei Myelith zählt nicht die Anzahl, sondern das
→ [Stimmgewicht](#stimmgewicht-voting-weight).**

*Im Code:* `CONSENSUS/myl-consensus/src/bft.rs`

### Safety und Liveness

Die beiden Eigenschaften, die ein Konsens haben soll:

- **Safety** („nichts Falsches passiert") — es werden nie zwei
  widersprüchliche Blöcke finalisiert.
- **Liveness** („irgendwann passiert etwas") — es wird schließlich
  *irgendein* Block finalisiert.

Sie sind unabhängig. Ein Protokoll, das nie etwas entscheidet, ist
perfekt safe und völlig nutzlos. Genau dieser Fall trat im Projekt auf:
Das Ein-Runden-Protokoll in `bft.rs` war safe, blieb aber stehen, wenn
der Leader ausfiel — deshalb gibt es `round_change.rs`.

### Propose / Vote / Commit

Der Dreischritt einer Konsensrunde:

1. **Propose** — Der Leader der Runde schlägt einen Block vor.
2. **Vote** — Validatoren stimmen zu (Vorabstimmung).
3. **Commit** — Wenn genug Stimmgewicht zusammenkommt, wird festgelegt.

Erst wenn auch die Commit-Schwelle erreicht ist, gilt der Block als
finalisiert.

*Im Code:* `bft.rs`, Nachrichten in `signing.rs`

### Quorum und die 2/3-Schwelle

Das **Quorum** ist das Stimmgewicht, das für einen gültigen Schritt
nötig ist: mehr als zwei Drittel des Gesamtgewichts. Der Grund liegt in
der Überschneidung — zwei Mengen mit je über 2/3 haben zwingend ein
gemeinsames Mitglied mit über 1/3 Gewicht. Wären beide Mengen für
verschiedene Blöcke, hätte diese Schnittmenge widersprüchlich gestimmt
und wäre damit als byzantinisch überführt.

*Im Code:* `bft.rs::quorum_threshold`

### Lock (Sperre) und Polka-Zertifikat

Was passiert, wenn eine Runde scheitert, aber schon jemand einen Block
zu sehen bekommen hat, der *fast* durchgekommen wäre?

Ein Validator, der ein Quorum an Votes für Block B gesehen hat,
**sperrt** sich auf B: In späteren Runden stimmt er nur noch für B — es
sei denn, ihm wird bewiesen, dass eine spätere Runde ein Quorum für
etwas anderes hatte. Dieser Beweis ist das **Polka-Zertifikat** (*Proof
of Lock Change*): eine Sammlung von Votes, die das Quorum belegt.

Ohne Sperre könnte ein Netz mit wechselnden Leadern zwei verschiedene
Blöcke in verschiedenen Runden finalisieren — Safety-Bruch. Die Mechanik
stammt aus Tendermint.

**Härtung:** Die Wählerliste eines Polka-Zertifikats muss **streng
aufsteigend** sortiert sein. Sonst könnte ein Angreifer denselben
Validator mehrfach eintragen und ein Quorum vortäuschen.

*Im Code:* `CONSENSUS/myl-consensus/src/round_change.rs`

### Timeout und Rundenwechsel

Wenn der Leader nicht liefert, muss weitergeschaltet werden. Der Timeout
wächst **linear** mit der Rundennummer (`basis + runde · delta`, mit
Sättigung). Grund: Bei asynchronem Netz muss der Timeout irgendwann die
tatsächliche Nachrichtenlaufzeit übersteigen, sonst wechselt das Netz
ewig die Runde, ohne je fertig zu werden.

*Im Code:* `round_change.rs::TimeoutConfig`

### GST (Global Stabilization Time)

Der Zeitpunkt im Modell *partieller Synchronität*, ab dem
Nachrichten wieder innerhalb einer bekannten Schranke ankommen. Vor GST
garantiert BFT nur Safety, nach GST auch Liveness. Diese Aufteilung ist
kein Trick, sondern eine bewiesene Grenze: In einem vollständig
asynchronen Netz ist deterministischer Konsens unmöglich (FLP-Resultat).

### Double-Signing

Ein Validator signiert in **derselben Runde zwei verschiedene Blöcke**.
Das ist der klassische Konsens-Angriff und wird mit 30–100 % des Stakes
bestraft.

**Beweislast:** Ein Double-Signing-Beweis ist nur etwas wert, wenn ihn
jeder Dritte nachprüfen kann. Ein Beweis besteht deshalb aus beiden
Signaturen samt den signierten Nachrichten — nicht aus einer Behauptung.

*Im Code:* `CONSENSUS/myl-consensus/src/double_signing.rs`

### Komitee, Leader, Schiedsrichter

Pro Epoche werden **21 Blockproduktions-Validatoren** und
**7 Schiedsrichter** (für → [Schiedsrunden](#schiedsrunde-adjudication))
gewählt. Die Auswahl erfolgt gewichtet nach Stake, aber per VRF
randomisiert (`weighted_sample_without_replacement`), damit die
Zusammensetzung nicht vorhersagbar ist. Der **Leader** einer Runde
rotiert deterministisch.

*Im Code:* `validator.rs` — `COMMITTEE_SIZE = 21`, `ARBITER_COUNT = 7`

### Stimmgewicht (Voting Weight)

Die Kopplung, die aus einem gewöhnlichen Proof-of-Stake-System ein
Myelith-System macht:

```
voting_weight = stake + (stake · abgeklungene_Arbeit) / VTFE_UNIT
```

Das Gewicht speist sich aus gestaktem Coin **und** nachgewiesener
historischer Inferenzarbeit (mit Abklingfaktor). Wer das Netzwerk
angreifen will, muss also entweder massiv Coins kaufen — was den Preis
treibt und ihn selbst teuer zu stehen kommt — oder dauerhaft ehrliche
Arbeit leisten, was dem Angriffsziel widerspricht.

*Im Code:* `CONSENSUS/myl-consensus/src/voting_weight.rs`

### Ledger und Zustandsübergang

Das **Ledger** ist die Buchführung: Konten, Guthaben, Stake, Credits.
Jeder **Zustandsübergang** ist eine **reine Funktion**
`(State, Übergang) → State` ohne versteckten globalen Zustand, und
Fehler lassen den Zustand unverändert (erst prüfen, dann ändern).

Diese Bauweise ist kein Stilfrage: Nur so können alle Knoten dieselbe
Kette von Übergängen nachrechnen und exakt denselben Zustand erhalten.

*Im Code:* `CONSENSUS/myl-ledger/src/transitions.rs`

### Gossip

Die Art, wie sich Nachrichten im P2P-Netz verbreiten: Jeder Knoten gibt
weiter, was er bekommt. Myelith nutzt **libp2p Gossipsub** mit getrennten
Topics je Nachrichtenklasse (Blöcke, Transaktionen, PoI-Bündel,
Challenges, Latenz-Atteste), damit große PoI-Bündel den Block-Gossip
nicht ausbremsen.

**Validierung vor Weiterverbreitung:** Gossipsub läuft im Modus
`validate_messages()` — eine Nachricht wird erst weitergegeben, wenn der
Knoten sie geprüft und akzeptiert hat. Andernfalls wäre das Netz ein
kostenloser Verstärker für Spam.

*Im Code:* `NETWORKING/myl-net/src/gossip.rs`, `validation.rs`

---

## G. Epochen, Scheduler und Pods

### Epoche

Der Taktgeber des Protokolls. Innerhalb einer Epoche stehen die
Zuteilungen fest: Wer ist in welchem Pod, wer rechnet welche Segmente,
wer sitzt im Komitee. Am Epochenende wird abgerechnet
(→ [Epochenabschluss](#epochenabschluss)) und neu ausgelost.

### Shard

Ein zusammenhängender Abschnitt des Modells — eine Gruppe von Layern —
den ein einzelner Miner im Speicher hält.

> *Beispiel.* Ein 28-Layer-Modell auf 4 Shards: Miner A hält Layer 0–6,
> B 7–13, C 14–20, D 21–27. Keiner von ihnen hat das ganze Modell.

*Im Code:* `COMPUTE_PIPELINE/myl-pod/src/shard.rs`,
`INTEGER_LLM/runtime/src/model.rs::run_layers`

### Pod

Eine Gruppe von Minern, die zusammen ein **vollständiges** Modell
bilden — jeder hält einen Shard, aneinandergereiht ergeben sie den
ganzen Transformer. Ein Pod kann eine Anfrage allein beantworten.

*Im Code:* `CONSENSUS/myl-scheduler/src/lib.rs::Pod`

### Pipeline-Parallelismus

Die Aktivierungen wandern der Reihe nach durch die Shards: A rechnet
seine Layer, schickt das Ergebnis an B, B rechnet weiter, und so fort.
Anders als bei Tensor-Parallelismus (wo eine einzelne Matrixmultiplikation
aufgeteilt wird) fällt hier nur **an den Shard-Grenzen** Netzverkehr an —
entscheidend, wenn die Knoten nicht im selben Rechenzentrum stehen.

### Micro-Batching

Der Koordinator sammelt eingehende Anfragen über ein Zeitfenster
(Standard 250 ms) und schickt sie gebündelt durch die Pipeline. Während
Shard B an Batch 1 rechnet, kann Shard A schon Batch 2 bearbeiten.

**Der Punkt ist Durchsatz, nicht Latenz.** Eine einzelne Anfrage wird
dadurch nicht schneller — im Gegenteil, sie wartet bis zu 250 ms. Aber
die Gesamtzahl der Anfragen pro Sekunde steigt erheblich, weil kein
Shard leerläuft.

*Im Code:* `COMPUTE_PIPELINE/myl-pod/src/micro_batch.rs`

### Segment

Die Abrechnungs- und Verifikationseinheit: ein Tupel `(x, θ_v, π, y)`
aus Eingabe-Commitment, Modellversion, Pipeline-Pfad und
Ausgabe-Commitment.

*Im Code:* `SHARED_TYPES/myl-types/src/core_types.rs::Segment`

### Berechnungsspur

Englisch *trace*. Die Kette der Hashes über die Zwischenergebnisse: `h(a₀), h(a₁), …`,
wobei `aᵢ` die Aktivierungen nach Shard i sind. Jeder Shard signiert
seinen Übergang:

```
sig_i( h(a_{i−1}) ‖ h(a_i) ‖ segment_id )
```

Die Spur ist das, was das → [Bisektions-Spiel](#bisektions-spiel)
möglich macht, **ohne Aktivierungen on-chain zu speichern**: Man
vergleicht 32-Byte-Hashes statt Megabytes und findet so die erste
Abweichung.

*Im Code:* `COMPUTE_PIPELINE/myl-pod/src/trace.rs`

### Epochen-Scheduler

Der deterministische Ablauf, der jede Epoche die Zuteilungen festlegt.
Er läuft auf **jedem Knoten identisch** — es gibt keine zentrale
Instanz, jeder kann alles nachrechnen.

Sechs Schritte:

1. **VRF-Seed ableiten** — aus dem finalisierten Block der Vorepoche
   (`vrf_seed.rs`)
2. **Miner filtern** — nach Hardware-Klasse und Registrierungsschluss
   (`miner_filter.rs`)
3. **Geo-Clustering** — unter Latenz-Constraint (`geo_clustering.rs`)
4. **Shard-Zuweisung** — Fisher-Yates-Shuffle mit Seed (`shard_assignment.rs`)
5. **Redundanz-Zuteilung** — 2 disjunkte, zonendiverse Pods je Segment
   (`redundancy.rs`)
6. **Stichproben-Lotterie** — Segmente für Checker markieren (`sampling.rs`)

*Im Whitepaper:* Anhang A.2

### Grinding

Ein Angriff auf Zufallslosungen: Der Angreifer probiert Eingaben durch,
bis das Ergebnis ihm passt. Zwei Gegenmittel:

- Der **Seed stammt aus einem bereits finalisierten Block** der
  Vorepoche — er steht fest, bevor jemand ihn ausnutzen könnte.
- Die **Registrierung schließt zwei Epochen vorher**, damit niemand
  kurzfristig präparierte Identitäten einbringt.

### Fisher-Yates-Shuffle

Der Standard-Algorithmus zum fairen Mischen: Man geht von hinten durch
die Liste und tauscht jedes Element mit einem zufällig gewählten aus dem
noch nicht bearbeiteten Teil. Jede Anordnung ist gleich wahrscheinlich.
Bei Myelith wird der Zufall aus dem VRF-Seed gezogen — also
deterministisch und für alle nachrechenbar.

*Im Code:* `SHARED_TYPES/myl-types/src/seed_rng.rs::deterministic_shuffle`

### Zonendiversität

Die zwei redundanten Pods eines Segments müssen aus **verschiedenen
geografischen Regionen und Autonomen Systemen (AS)** stammen. Sonst
könnte ein einziger Rechenzentrumsausfall — oder ein einziger Betreiber —
beide Pods zugleich betreffen, und der Redundanzvergleich wäre wertlos.

*Im Code:* `SHARED_TYPES/myl-types/src/node_metadata.rs::DiversityChecker`,
`CONSENSUS/myl-scheduler/src/redundancy.rs`

### LatencyGraph und Latenz-Atteste

Pods sollen aus Knoten bestehen, die **nah beieinander** liegen (wegen
Pipeline-Verkehr), während die **beiden Pods eines Segments weit
auseinander** liegen sollen (wegen Zonendiversität). Dafür braucht das
Netz eine Karte der Laufzeiten.

Jeder Knoten misst kontinuierlich per Ping/Pong die Round-Trip-Zeit zu
seinen Peers, glättet sie per **EMA** und veröffentlicht regelmäßig
signierte **Latenz-Atteste**. Daraus bauen alle Knoten denselben
`LatencyGraph`.

*Im Code:* `NETWORKING/myl-net/src/latency.rs`,
`SHARED_TYPES/myl-types/src/latency_attest.rs`

---

## H. Verifikation

### Die drei Stufen

| Stufe | Verfahren | Kosten | Wann |
|---|---|---|---|
| **1** | Redundanzvergleich zweier Pods | +100 % Rechenzeit | immer |
| **2** | Checker rechnen Stichproben nach | 1–3 % Volumen | laufend |
| **3** | zkML-Anker (Zero-Knowledge-Beweis) | sehr hoch | optional, Premium |

Stufe 3 ist **noch nicht implementiert** — sie ist als Aufrüstpfad
vorgesehen, sobald zkML-Systeme effizient genug werden. Die ganzzahlige
Ausführung kommt diesem Pfad entgegen, weil arithmetische Schaltkreise
über Ganzzahlen erheblich einfacher zu formulieren sind als über
Gleitkomma.

*Im Code:* `VERIFICATION/myl-verifier/src/lib.rs`

### Redundanzvergleich (Stufe 1)

Zwei unabhängig ausgeloste Pods rechnen dasselbe Segment. Stimmen die
Commitment-Hashes an **allen** Spur-Positionen überein, gilt das Segment
als vorläufig bestätigt. Der Vergleich ist binär und parameterfrei.

*Im Code:* `VERIFICATION/myl-verifier/src/redundancy.rs`

### Auslieferungsmodi

Wann wird verglichen — vor oder nach der Auslieferung? Beides ist je
Anfrage wählbar:

- **Optimistisch** (Standard) — Die Antwort des zuerst fertigen Pods geht
  sofort raus, der Abgleich läuft asynchron. Latenz wie bei einem
  einzelnen Pod; die Sicherheit wirkt nachträglich über Slashing und
  Rückbuchung.
- **Bestätigt** (Aufpreis) — Die Antwort wird zurückgehalten, bis der
  Zwillings-Pod übereinstimmt. Ein manipuliertes Ergebnis erreicht den
  Nutzer nicht, sofern nicht beide Pods kolludieren. Preis: Latenz und
  Gebühr.

Für eine Recherche genügt die nachträgliche Sanktion; für eine
Agenten-Entscheidung mit Finanzwirkung ist die vorbeugende Variante
angemessen.

*Im Code:* `VERIFICATION/myl-verifier/src/delivery.rs`

### Challenge (Anfechtung)

Das On-Chain-Artefakt, mit dem ein Checker eine Abweichung anzeigt und
das Bisektions-Spiel eröffnet. Sie benennt die erste abweichende
Spur-Position.

**Eine Anfechtung kostet Kaution.** Wer mutwillig falsch anficht,
verliert sie — sonst wäre das Anfechten ein kostenloser
Denial-of-Service.

*Im Code:* `SHARED_TYPES/myl-types/src/challenge.rs` (der Typ liegt in
den geteilten Typen, weil ihn drei Komponenten benutzen),
`VERIFICATION/myl-verifier/src/challenge.rs`

### Bisektions-Spiel

Das Herzstück der Streitschlichtung: Wie findet man den einen falschen
Rechenschritt, ohne die ganze Berechnung on-chain zu wiederholen?

Antwort: **binäre Suche über die Berechnungsspur**, in O(log L) Runden.

> *Beispiel mit 8 Shard-Übergängen.* Miner und Checker sind sich uneinig
> über das Endergebnis.
> - Runde 1: Beide legen ihren Hash nach Übergang 4 vor. Verschieden →
>   der Fehler liegt in 1–4.
> - Runde 2: Hash nach Übergang 2. Gleich → der Fehler liegt in 3–4.
> - Runde 3: Hash nach Übergang 3. Gleich → der Fehler ist Übergang 4.
>
> Drei Runden, dann steht fest, welcher einzelne Schritt strittig ist.

Anschließend legt der Miner die Eingangs-Aktivierungen dieses einen
Schrittes offen (aus der → [DA-Schicht](#da-datenverfügbarkeit)), das
Schiedsrichter-Komitee rechnet **einen** Shard-Forward nach und
vergleicht. Der Verlierer wird geslasht, der Gewinner bekommt Kopfgeld.

**Warum die Schuldzuweisung eindeutig ist:** Es gibt genau ein korrektes
Ergebnis (Ganzzahl-Determinismus!), und der Vergleich ist eine
Hash-Gleichheit ohne Ermessensspielraum. Die Validatoren brauchen keine
spezielle Hardware und keine zertifizierte Kernel-Implementierung.

Das Verfahren stammt aus Truebit und Arbitrum.

*Im Code:* `VERIFICATION/myl-verifier/src/bisection.rs`

### Schiedsrunde (Adjudication)

Die On-Chain-Entscheidung am Ende der Bisektion: Das Komitee führt den
strittigen Shard-Forward gemäß θ_v aus und vergleicht den Hash. Die
Ausführung selbst ist über den `ShardExecutor`-Trait abstrahiert, damit
die Schiedslogik unabhängig von der konkreten Inferenz-Implementierung
testbar bleibt.

**Kosten:** Ein einzelner Shard-Forward auf etwa sieben Validatoren —
konstant, unabhängig von der Segmentlänge, und im Regelbetrieb nie
fällig.

*Im Code:* `VERIFICATION/myl-verifier/src/adjudicate.rs`

### Slashing

Der Einzug von hinterlegtem Stake als Strafe.

| Akteur | Slash-Grund | Höhe |
|---|---|---|
| Shard-Miner | falsches Ergebnis (per Bisektion bewiesen) | 100 % |
| Shard-Miner | Nichtverfügbarkeit während Session | 1–5 % |
| Pod-Koordinator | falsche PoI-Aggregation | 100 % |
| Validator | Double-Signing / bewiesene Zensur | 30–100 % |
| Checker | mutwillig falsche Anfechtung | Kaution |

*Im Code:* `VERIFICATION/myl-verifier/src/slash.rs` (entscheidet **wer**
verloren hat), `CONSENSUS/myl-ledger/src/transitions.rs::apply_verdict`
(bucht die **Beträge** — die Trennung ist Absicht: Beträge sind
Governance-Parameter, Schuld ist ein Beweis)

### Anreiz-Ungleichung

Die ökonomische Sicherheitsbedingung. Mit Stichprobenrate `p`, Stake `S`
und Betrugsgewinn `g` je Segment:

```
S_min = g / p²
```

Bei `p = 2 %` und `g` = Reward eines Segments folgt `S_min = 2500`
Segment-Rewards — etwa zwölf Epochen-Einkommen.

**Die quadratische Abhängigkeit ist der wichtigste Hebel des Entwurfs.**
Sie erklärt, warum die Anlaufphase mit erhöhter Prüfrate arbeitet: Bei
50 % statt 2 % fällt der Stake-Bedarf auf ein Sechshundertstel. Das
kostet Kapazität, die in einer Phase mit Überkapazität ohnehin brachliegt.

*Im Whitepaper:* Kap. 5.5, Anhang B.1

### Kontrollsegmente (Canaries)

Die Lücke, die Stufe 1 und 2 offenlassen: ein **einmaliger** Eingriff
eines Angreifers, der **beide** Pods kontrolliert. Redundanz hilft nicht
(beide lügen gleich), Stichproben helfen nur, wenn er wiederholt auffällt.

Kontrollsegmente schließen sie teilweise: Das Netzwerk hält einen Vorrat
von Segmenten, deren korrektes Ergebnis bereits vorliegt, und schleust
sie mit einem Anteil γ in den regulären Auftragsstrom. Für den Miner sind
sie von echten Anfragen nicht unterscheidbar.

**Der Gewinn liegt in der Ungewissheit des Angreifers:** Da er bei
keinem Segment weiß, ob es eine Kontrolle ist, trägt bereits der *erste*
Manipulationsversuch ein Entdeckungsrisiko von γ. Bei γ = 2 % und vollem
Stake-Verlust ist der Erwartungswert eines Einzelangriffs negativ.

Drei Konstruktionsanforderungen: **Ununterscheidbarkeit** (reale
Prompt-Verteilung, unauffälliges Timing- und Längenprofil),
**Vorratserneuerung** (ein statischer Pool wird mit der Zeit erkennbar)
und **Kostenehrlichkeit** (γ ist reiner Overhead und geht in die
Kostenstruktur ein).

*Im Whitepaper:* Kap. 6.7 — noch nicht implementiert.

### DA (Datenverfügbarkeit)

Vollständige Prompts und Ausgaben gehören nicht on-chain (Datenschutz,
Volumen). On-chain stehen nur Commitments; die Rohdaten liegen
→ [erasure-codiert](#erasure-codierung-reed-solomon-cauchy-form) bei den
beteiligten Pods — und zwar für die **Streitfrist**.

**Eine Härtungsentscheidung, die leicht zu übersehen ist:** `fetch`
prüft die Streitfrist **vor** dem Nachschlagen. Sonst wäre am Verhalten
unterscheidbar, ob Daten abgelaufen oder zurückgehalten wurden — und
„zurückhalten" wäre eine Strategie.

*Im Code:* `CONSENSUS/myl-consensus/src/da.rs`,
`COMPUTE_PIPELINE/myl-pod/src/da.rs`

### Streitfrist (Dispute Window)

Der Zeitraum (Entwurf: 7 Tage), in dem ein Segment noch angefochten
werden kann. Solange sie läuft, müssen die DA-Fragmente vorgehalten
werden; danach dürfen sie verschwinden und das Segment gilt als endgültig.

---

## Arbeitsnachweise: PoI und Epochenabschluss

### PoI (Proof of Inference)

Der Arbeitsnachweis von Myelith — das Gegenstück zum Hashwert bei
Proof-of-Work, nur dass die Arbeit **nützlich** ist. Ein PoI belegt,
dass ein Pod ein Segment tatsächlich gerechnet hat.

**Warum kein „Inference-PoW"?** Man könnte den Blockproduzenten direkt
über ein Inferenz-Wettrennen bestimmen: Wer zuerst rechnet, schreibt den
Block. Das wäre über die Eingaben manipulierbar (→ [Grinding](#grinding))
und würde die Blockzeit an die Inferenz-Latenz koppeln. Stattdessen laufen
**zwei entkoppelte Prozesse**:

- **Prozess A — Blockproduktion (schnell).** BFT-Komitee, Blockzeit 1–2 s.
- **Prozess B — Arbeitsnachweis (kontinuierlich).** Pods reichen pro
  Epoche signierte PoI-Bündel ein.

Gekoppelt sind beide nur über das → [Stimmgewicht](#stimmgewicht-voting-weight).

*Im Whitepaper:* Kap. 3.5

### PoI-Bündel

Was ein Pod am Epochenende einreicht: eine Merkle-Wurzel über
(Eingabe-Commitment, Ausgabe-Commitment, Segment-Metadaten, Signaturen
aller beteiligten Shard-Miner), aggregiert signiert.

**Die kritische Härtung:** Die Menge der zulässigen Unterzeichner
(`PodMembership`) kommt **aus dem Scheduler, niemals aus dem Bündel
selbst**. Käme sie aus dem Bündel, könnte ein Angreifer die
Mitgliederliste einfach mitliefern und für einen beliebigen Pod
unterschreiben — die Signaturprüfung würde bestehen und trotzdem nichts
beweisen. Zusätzlich braucht jedes Mitglied einen hinterlegten
→ [Proof-of-Possession](#rogue-key-angriff-und-proof-of-possession).

*Im Code:* `SHARED_TYPES/myl-types/src/core_types.rs::PoIBundle`,
`CONSENSUS/myl-consensus/src/poi.rs`

### Epochenabschluss

Der Schritt von **beansprucht** zu **bestätigt**. Die PoI-Einreichung
stellt fest, dass ein Pod eine Menge Arbeit behauptet; der
Epochenabschluss stellt fest, ob sie ihm zusteht. Erst daraus wird
geprägt.

Für jedes Segment werden die Ergebnisse der beiden Pods verglichen. Drei
mögliche Ausgänge:

- **Match** — beide stimmen überein → bestätigt
- **Mismatch** — sie weichen ab → nichts wird gutgeschrieben, Challenge
- **Missing** — der Zwillings-Pod hat nichts eingereicht → **nicht**
  bestätigt

**Warum `Missing ≠ Match`:** Würde ein fehlender Zwilling als
Übereinstimmung gewertet, wäre „mach den Zeugen unerreichbar" eine
Strategie — der Angreifer müsste nur den ehrlichen Pod ausschalten, statt
ihn zu überzeugen.

*Im Code:* `CONSENSUS/myl-consensus/src/epoch_close.rs`

### Clawback (Rückbuchung)

Wird ein bereits gutgeschriebenes Segment nachträglich widerlegt, wird
die vTFE-Gutschrift zurückgebucht. Das ist die Sicherung, die den
optimistischen Auslieferungsmodus überhaupt vertretbar macht.

*Im Code:* `epoch_close.rs::apply_clawback`

---

## I. Tokenomik

### MYL

Der native Coin. Drei Funktionen: Sicherung des Konsenses (Staking),
Vergütung der Miner (Minting), Bezahlung der Inferenz (Burning).

*Im Code:* `TOKENOMICS/myl-tokenomics/src/lib.rs` —
`UNITS_PER_MYL = 1_000_000`

### Burn-and-Mint

Der geschlossene Kreislauf:

```
Miner verkauft MYL am Markt ─────────────────┐
                                             │
Nutzer ──burn MYL──► Inferenz-Credits        │
                     │                       │
                     ▼                       │
                     Pods leisten Arbeit     │
                     │                       │
                     ▼                       │
                     bestätigte PoI-Bündel   │
                     │                       │
                     ▼                       │
                     mint MYL ──► Miner──────┘
```

Das Protokoll gibt MYL **ausschließlich an Miner** aus; Nutzer erwerben
sie am Markt. Prägung und Verbrennung sind Protokollvorgänge, der Erwerb
ist es nicht.

### vTFE (verifizierte Token-Forward-Äquivalente)

Die Recheneinheit, in der Arbeit gemessen wird — nicht Fiat, nicht MYL,
sondern **Rechenarbeit**. Dadurch ist der Nutzpreis der Inferenz stabil
in Recheneinheiten; der MYL-Preis vermittelt zwischen Angebot und
Nachfrage.

*Im Code:* `VTFE_UNITS_PER_TFE = 1_000_000`

> **⚑ Offener Punkt:** vTFE muss **Layer** zählen, nicht Shards. Ein
> Shard ist eine Verpackungseinheit und je nach Pod-Größe verschieden
> groß; Layer sind die tatsächliche Arbeit. Solange nach Shards gezählt
> wird, wäre ein Pod mit wenigen großen Shards gegenüber einem mit vielen
> kleinen benachteiligt. Notiert im Fahrplan-Master.

### Inferenz-Credit (IC)

Was der Nutzer durch Verbrennen von MYL erhält. Denominiert in vTFE.

*Im Code:* `SHARED_TYPES/myl-types/src/core_types.rs::InferenceCredit`,
`CONSENSUS/myl-ledger/src/transitions.rs::burn_to_credits`

### Prägefunktion

```
M_e = min( B̄_e · (1 + s), M_max )
```

- `B̄_e` — geglättetes Burn-Volumen (→ [EMA](#ema-exponential-moving-average))
- `s` — Subventionsrate (Anlaufphase > 0, Zielbetrieb 0)
- `M_max` — Emissionsdeckel

Im Gleichgewicht (`s → 0`) gilt `M_e ≈ B̄_e`: Die Geldmenge ist
langfristig netto-neutral bis deflationär, da Slashing-Burns hinzukommen.

*Im Code:* `TOKENOMICS/myl-tokenomics/src/mint.rs::mint_amount`

### EMA (Exponential Moving Average)

Ein gleitender Durchschnitt, der neue Werte stärker gewichtet als alte:
`B̄_e = B̄_{e−1} + α · (B_e − B̄_{e−1})`, mit `α = 2/(N+1)` und
N = 30 Epochen, also `α = 2/31`.

**Warum geglättet wird:** Ohne EMA könnte ein Angreifer in einer einzigen
Epoche massiv verbrennen, die Prägung dieser Epoche hochtreiben und den
Großteil davon selbst einstreichen. Die Glättung verteilt den Effekt über
30 Epochen und macht den Angriff unrentabel.

Vollständig ganzzahlig implementiert (Zähler/Nenner als Bruch).

*Im Code:* `TOKENOMICS/myl-tokenomics/src/ema.rs`

### Verteilung der Prägung

| Anteil | Empfänger | Basispunkte |
|---|---|---|
| 78 % | Shard-Miner (nach Redundanz-Normierung) | 7800 |
| 5 % | Pod-Koordinatoren | 500 |
| 10 % | Validatoren (Stake × Uptime) | 1000 |
| 4 % | Checker-Pool (Grundvergütung) | 400 |
| 3 % | Protokoll-Treasury | 300 |

Geprüft über 10 000 simulierte Epochen: Die Summe der Anteile entspricht
in jeder Epoche exakt `M_e` (floor-Rundung je Anteil, Rundungsrest
geschlossen ans Treasury).

*Im Code:* `TOKENOMICS/myl-tokenomics/src/distribute.rs`

### Redundanz-Normierung

Da jedes Segment von 2 Pods gerechnet wird, erhält jeder Pod die **halbe**
vTFE-Gutschrift. Miner werden für *nützliche Netto-Arbeit* bezahlt; der
Redundanz-Overhead ist eingepreist, nicht versteckt.

*Im Code:* `distribute.rs::redundancy_normalized_weight`

### Credit-Preisformel

```
P_{e+1} = P_e · exp( κ · (u_e − u*) )
```

mit Auslastung `u_e`, Auslastungsziel `u* = 0,8` und Dämpfungskonstante
κ. Bei Überlast steigt der Preis → Nachfrage sinkt, Mining wird
attraktiver → Kapazität wächst. Preissignale statt zentralem
Kapazitätsmanagement (EIP-1559-analog).

**Die exp-Funktion ist auch hier ganzzahlig** — aus demselben Grund wie
im Modell: Konsens-Determinismus. Die Stützstellen sind **eingefroren**
und liegen als generierte Tabelle im Code; eine zur Laufzeit berechnete
Tabelle könnte zwischen Compiler-Versionen abweichen.

*Im Code:* `TOKENOMICS/myl-tokenomics/src/exp_approx.rs`,
`exp_lut_table.rs`, `utilization.rs`

### Self-Dealing

Der Angriff, bei dem ein Miner seine eigene Inferenz kauft, um Prägung zu
ernten. Unrentabel per Konstruktion, solange `M_e ≤ B̄_e`: Der Angreifer
verbrennt mehr, als er zurückerhält, weil er nur seinen *Kapazitätsanteil*
der Prägung bekommt. In der Subventionsphase zusätzlich gedämpft durch
EMA-Glättung und ein Burn-Cap pro Adresse.

### Trainingsvergütungs-Obergrenze

Trainingsvergütung ≤ **70 %** der Inferenzvergütung je Rechenstunde.
Ohne diese Grenze verlagern Miner Kapazität von der Inferenz aufs
Training und entziehen dem Netzwerk seine einzige Einnahmequelle.
Finanziert aus Treasury und einem abschaltbaren Gebührenaufschlag —
**nicht** aus Zusatzprägung, die die Netto-Inflation nahezu verdoppeln
und alle Halter verwässern würde.

*Im Code:* `TOKENOMICS/myl-tokenomics/src/training.rs` —
`TRAINING_CAP_BPS = 7000`

---

## J. Training

Der Trainingspfad ist **entworfen, aber noch nicht implementiert**. Der
TRAINING-Fahrplan hat aktuell genau einen Punkt: eine
Referenzsimulation des Rückwärtspasses. Die Begriffe stehen hier, weil
sie im Whitepaper hergeleitet sind und die Entwurfsentscheidungen
festliegen.

### Rückwärtspass und Gradient

Beim Training misst man, wie falsch die Ausgabe war, und rechnet
rückwärts durch das Netz aus, wie jedes Gewicht geändert werden müsste.
Diese Änderungsrichtung ist der **Gradient**.

**Gute Nachricht für Myelith:** Die Gradientenberechnung ist ebenfalls
assoziativ, der Determinismus-Ansatz aus
[Abschnitt B](#b-determinismus--warum-myelith-ganzzahlig-rechnet)
überträgt sich unverändert.

### Das Überlaufproblem

Ganzzahlige Rückpropagierung stößt auf eine Grenze: Die Fehlerterme
**wachsen mit jeder rückwärts durchlaufenen Schicht** und sprengen bei
8-Bit-Gewichten schon nach wenigen Schichten den 32-Bit-Bereich. Zwei
Verfahren lösen das.

### Block-Skalierung (NITI)

Nach jeder Schicht wird der Fehlervektor durch einen gemeinsamen
**Zweierpotenz-Faktor** geteilt, dessen Exponent separat mitgeführt wird.
Der Faktor folgt aus dem Betragsmaximum und ist damit
reihenfolgeunabhängig; angewandt wird er als arithmetischer Rechtsshift —
also mit genau der Operation, die θ_v ohnehin vorschreibt.

### Lokale Verlustblöcke

Das Netz wird in Segmente mit **eigenen Verlustfunktionen** gegliedert,
sodass Gradienten das Segment nicht verlassen. Legt man die Blockgrenzen
auf die Shard-Grenzen, entfällt der Rückwärtspass über die Pipeline
vollständig: kein zusätzlicher Netzverkehr, und die Verifikation bleibt
lokal — ein Shard-Paar prüft seinen eigenen Gradienten.

Der Preis ist eine gegenüber globaler Rückpropagierung schlechtere
Lösung; wie groß der Abstand bei Sprachmodellen ausfällt, ist offen.

### Datenprovenienz

Die schwierigste Frage des Trainings ist nicht, ob korrekt gerechnet
wurde, sondern **ob die Daten legitim waren**. Ein Miner, der vergiftete
Texte einspeist, rechnet bitgleich korrekt und erzeugt dennoch ein
verschobenes Modell — der Bitvergleich greift hier nicht.

Myelith prüft nicht den **Inhalt**, sondern die **Herkunft**: Das
Protokoll führt eine Liste kanonischer Korpora, jedes mit einer
Merkle-Wurzel im Konsens. Ein Trainingssegment referenziert keine
Rohdaten, sondern einen **Merkle-Beweis**: „Dieser Textabschnitt steht an
Position p im Korpus mit Wurzel R." Für nicht existierende Positionen
lässt sich kein gültiger Beweis erzeugen.

**Auswahl bleibt als Angriffsfläche.** Wer keine Daten fälschen kann,
kann immer noch auswählen. Deshalb erfolgt die Datenzuweisung **ebenfalls
per VRF** — welcher Pod welche Korpusabschnitte bearbeitet, ergibt sich
aus dem Epochen-Seed. Diese Auflage ist konstitutiv, nicht optional.

### Robuste Aggregation (Median)

Die Gradienten vieler Pods müssen zu einem Update zusammengeführt werden.
Der Mittelwert ist ungeeignet: Ein einziger extremer Beitrag verschiebt
ihn beliebig. Myelith aggregiert über den **Median**, dessen Bruchpunkt
bei 50 % liegt und damit mit der ohnehin angenommenen byzantinischen
Schranke zusammenfällt. Getrimmte Mittelwerte versagen schon bei einem
Drittel Angreiferanteil.

Der Median braucht nur Vergleiche — bleibt also deterministisch und im
Verifikationsmodell nachprüfbar.

### Funktionserhaltende Expansion (Net2Net, bert2BERT)

Verfahren, die ein Modell **vergrößern, ohne seine Funktion zu ändern**:
Neuronen werden aufgespalten, neue Schichten als Identität initialisiert.
Unmittelbar nach der Expansion verhält sich das größere Modell identisch
zum Vorgänger.

Zwei Folgen für Myelith: Ein Wachstumsschritt ist **ohne Qualitätsrisiko
aktivierbar** (die Verbesserung entsteht erst durch Nachtraining), und
die Expansion ist eine **deterministische Transformation** — also
bitgleich verifizierbar wie jede andere Berechnung. θ_v+1 ergibt sich
reproduzierbar aus θ_v und dem Wachstumsoperator.

**Strukturelle Kopplung:** Tiefenwachstum fügt Schichten hinzu, in der
Pipeline also zusätzliche Shards. Mehr Miner → mehr Shards → mehr
Schichten. Netz- und Modellwachstum sind architektonisch verbunden, und
die Kollusionsschranke β^{2k} verbessert sich mit steigendem k.

**Zeitskala, nüchtern:** Ein Netz mit 500 Minern kann nicht wachsen. Mit
5 000 Minern dauert ein Schritt etwa neun Monate, mit 50 000 rund einen
Monat. Wachstum ist ein seltenes Ereignis im Jahresmaßstab.

### Was offen bleibt

Drei Punkte, die das Whitepaper ausdrücklich benennt statt umschreibt:
Die Finanzierung erzeugt in jeder Variante Fehlanreize; die Kombination
aus ganzzahligem Training und Modellwachstum ist **unbelegt** (beide
einzeln sind belegt, die Kombination nicht, und die Belege für
ganzzahliges Training stammen aus dem Bildbereich); und das Verhalten
unter offenen Netzbedingungen ist unbekannt.

*Im Whitepaper:* Kap. 7, Anhang B.6

---

## K. Agent Layer

### Session-Kontrakt

Ein Agent, der Transaktionen auslösen kann, verwandelt einen
Berechnungsfehler in einen Vermögensschaden. Die Antwort des Protokolls
ist nicht, den Fall auszuschließen, sondern seine **Auswirkung zu
begrenzen**. Jede Agenten-Session läuft unter vier durchgesetzten
Grenzen:

1. **Gesamtbudget** in Credits und ggf. MYL
2. **Einzeltransaktionslimit**, unabhängig vom Restbudget
3. **Empfänger-Whitelist**
4. **Zeitfenster**, nach dessen Ablauf die Session erlischt

**Entscheidend ist, wo diese Parameter stehen: im Kontrakt, nicht im
Kontext des Modells.** Sie sind für den Agenten nicht lesbar und nicht
änderbar; durchgesetzt werden sie beim Ausführen der Transaktion durch
den Konsens.

### Deterministische vs. externe Werkzeuge

Ruft ein Agent eine Websuche auf, bekommen die zwei redundanten Pods
verschiedene Antworten — der Bitvergleich schlägt fehl, ohne dass ein
Fehler vorliegt.

Die Lösung: Werkzeugergebnisse werden aus der Berechnung **herausgenommen
und zur Eingabe gemacht**. Ein Gateway ruft das Ergebnis einmal ab,
versieht es mit Zeitstempel und Signatur und übergibt es beiden Pods als
identischen Text.

- **Deterministische Werkzeuge** (eigenes Ledger, Berechnungen, verankerte
  Korpora) — vollständig verifiziert wie jede Berechnung.
- **Externe Werkzeuge** (Websuche, Marktdaten) — die Antwort ist
  **attestiert, aber nicht verifiziert**. Das Protokoll bezeugt, *dass*
  ein Gateway zu einem Zeitpunkt diese Antwort erhalten hat, nicht dass
  sie zutrifft.

Was verifiziert wird, ist die *Verarbeitung* der Antwort, nicht ihre
Richtigkeit.

### Prompt Injection und das Dual-LLM-Muster

Verarbeitet ein Agent fremde Inhalte, können diese Anweisungen enthalten,
die sich als Nutzerauftrag ausgeben. Das Problem ist bekannt und
**ungelöst**; filterbasierte Ansätze gelten als unzuverlässig, weil der
Prüfmechanismus derselben Angriffsfläche unterliegt wie das Modell.

Myelith folgt der architektonischen Trennung (Dual-LLM, CaMeL): Der
planende Teil sieht keine fremden Inhalte, der verarbeitende Teil kann
keine Werkzeuge aufrufen, und abgerufene Daten beeinflussen den
Kontrollfluss nicht. Verstärkt wird das dadurch, dass die Berechtigungen
ohnehin im Session-Kontrakt liegen — **außerhalb der Reichweite des
Modells**.

Ein eingeschleuster Text kann den Agenten täuschen, aber weder sein
Budget erhöhen noch einen Empfänger hinzufügen. Damit verschiebt sich das
Problem von der Sicherheit zur Ergebnisqualität. Das ist die stärkste
verfügbare Aussage; eine vollständige Abwehr wird ausdrücklich nicht
beansprucht.

### Schrittverkettung

Ein Agent arbeitet iterativ. Jeder Schritt ist ein eigenes Segment und
referenziert das Ausgabe-Commitment seines Vorgängers. Es entsteht eine
Kette mit derselben Struktur wie die → [Berechnungsspur](#berechnungsspur)
innerhalb eines Segments, nur eine Ebene höher. Prüfbar ist damit auch,
dass keine Schritte ausgelassen, eingefügt oder vertauscht wurden.

*Im Whitepaper:* Kap. 8

---

## L. Arbeitsweise des Projekts

Dieser Abschnitt richtet sich vor allem an Coding-Agenten. Er erklärt
nicht, *was* gebaut wird, sondern *wie* — und warum die Regeln so sind,
wie sie sind.

> **Hinweis zu den Verweisen in diesem Abschnitt.** Die verbindliche
> Fassung dieser Regeln steht in `AGENTS.md` und
> `README/Intern/State-of-the-Project.md` (Abschnitt 8). Beide sind
> **arbeitsintern und nicht Teil der Veröffentlichung** — wer nur das
> öffentliche Repository vor sich hat, findet sie nicht. Dieser Abschnitt
> ist deshalb so geschrieben, dass er auch ohne sie vollständig ist.

### Open-Source-Bedrohungsmodell

**Der Angreifer kennt den Code.** Myelith ist Open Source; es gibt keine
Sicherheit durch Verschweigen. Jede Härtung muss auch dann tragen, wenn
der Angreifer das Protokoll vollständig verstanden hat.

Praktische Folgen, die im Code sichtbar sind:

- **Timing-Argumente sind Fristen, keine Schutzmaßnahmen.** „Der
  Angreifer schafft das nicht rechtzeitig" ist kein Sicherheitsargument,
  sondern eine Wette auf seine Hardware.
- **Konstantzeit-Vergleiche** bei Hashes, damit die Vergleichsdauer keine
  Information trägt.
- **Reihenfolgen prüfen**: Die Wählerliste eines Polka-Zertifikats muss
  streng aufsteigend sein, sonst zählt derselbe Validator mehrfach.
- **Autoritative Quellen statt mitgelieferter Angaben**: Die
  Pod-Mitgliedschaft kommt aus dem Scheduler, nicht aus dem Bündel.
- **Ununterscheidbare Fehlerfälle**: `DaStore::fetch` prüft die
  Streitfrist vor dem Nachschlagen, damit „abgelaufen" und
  „zurückgehalten" gleich aussehen.

### Golden Vectors

Eingefrorene Referenzergebnisse: Eingabe → erwartete Ausgabe, Bit für
Bit. Sie sind die **normative Wahrheit** für jedes Backend. Ein neues
Backend (SIMD, CUDA, ROCm) gilt erst als gültig, wenn es alle Golden
Vectors exakt reproduziert.

*Im Code:* `INTEGER_LLM/tests/golden/`,
`kernels/src/bin/golden_runner.rs`, `golden_generate`

> **Stolperfalle, die zweimal zugeschlagen hat (Fund 30):**
> `golden_generate` hängt `vectors` selbst an den übergebenen Ausgabepfad
> an. `generate.py` übergab aber bereits `tests/golden/vectors` — die
> neuen Vektoren landeten in `tests/golden/vectors/vectors/`, während die
> Prüfung weiter aus `vectors/layer` las, also aus **veralteten** Dateien.
> Beim ersten Mal wurde es als Bedienfehler notiert; es war ein
> Generator-Fehler und trat deshalb sofort wieder auf. Behoben, und das
> verwaiste Duplikat ist aus der Versionierung entfernt.
>
> **Lehre:** Wenn dieselbe Falle zweimal zuschlägt, liegt sie im Code und
> nicht in der Bedienung. Eine Warnung in der Doku ersetzt keine
> Zusicherung im Programm.

**Zweite Kopie beachten:** `INTEGER_LLM/conformance/vectors/` ist eine
Handkopie von `tests/golden/vectors/`. Nach jedem θ_v-Sprung müssen beide
abgeglichen werden, sonst prüft der Konformitätslauf gegen alte Vektoren:

```bash
rsync -a --delete INTEGER_LLM/tests/golden/vectors/ INTEGER_LLM/conformance/vectors/
```

### Konformitätslauf

Der Nachweis, dass zwei Backends bitgleiche Ergebnisse liefern. Aktueller
Stand: **30/30** für Referenz- und SIMD-Backend.

*Im Code:* `INTEGER_LLM/conformance/`

### Backend-Trait

Die Abstraktion über heterogene Hardware. Jede Implementierung muss den
Numerik-Vertrag aus θ_v erfüllen und gegen die Golden Vectors validiert
werden. Freiheit besteht bei Parallelisierung und Kernel-Aufbau, nicht
beim Ergebnis.

*Im Code:* `INTEGER_LLM/kernels/src/backend.rs`, Implementierungen in
`kernels/src/backends/`

### SIMD

*Single Instruction, Multiple Data* — eine CPU-Instruktion bearbeitet
mehrere Werte gleichzeitig. Auf ARM heißt das NEON, auf x86 AVX2.

**Der wichtigste Fallstrick dabei**, weil er im Projekt tatsächlich
auftrat: Der erste NEON-Versuch war **langsamer** als der skalare Code
(12,43 statt 18,89 tok/s). Ursache war ein **einzelner Akkumulator** —
jede Multiply-Accumulate-Instruktion musste auf das Ergebnis der
vorherigen warten, eine serielle Abhängigkeitskette. Mit **vier
unabhängigen Akkumulatoren**, die erst am Blockende zusammengeführt
werden, kann die CPU die Instruktionen überlappen: +31 % / +50 %,
bitidentisch.

Die Lehre: Bei SIMD entscheidet die **Latenz der Abhängigkeitskette**,
nicht der Durchsatz der Einzelinstruktion.

*Im Code:* `INTEGER_LLM/kernels/src/dot.rs`

### Fund

Ein dokumentierter Befund — ein Fehler, eine Fehlannahme oder eine
Erkenntnis, die den Entwurf verändert. Funde werden **nummeriert und
dokumentiert, nicht stillschweigend gefixt**.

**Warum das eine harte Regel ist:** Ein stillschweigend behobener Fehler
kommt wieder, sobald jemand den Code umbaut und den Grund nicht kennt.
Ein dokumentierter Fund erklärt, *warum* der Code so aussieht.

Beispiele: Fund 15 (RoPE-Schema falsch — dominante Fehlerquelle),
Fund 19 (1/√head_dim als Shift war nur für gerade Zweierpotenzen
korrekt), Fund 22 (KV-Cache-Rundreise), Fund 27 (Rogue-Key ohne PoP).

*Wo notiert:* `README/Intern/Fahrplan-Master.md`,
`README/Intern/State-of-the-Project.md`

### Instrumentenfehler

Die eigene Fehlerklasse dieses Projekts, und die lehrreichste:
**Nicht der Code war falsch, sondern das Messwerkzeug.**

Im Verlauf der Fehlersuche zu Fahrplanpunkt 12.77 traten **neun** davon
auf. Eine Auswahl:

1. Ein Softmax-Patch feuerte nie (SDPA fusioniert den Softmax) — die
   Messung „+0,00 %" war eine **Nullmessung**, kein Ergebnis.
2. Ein Ablationsskript hatte die Vergleichszahl **hart kodiert** und
   meldete „98 % Fehler im LM-Head", wo in Wahrheit 0 % lagen.
3. Ein „71-%-Sprung bei Ebene 23" verglich die Ebenenausgabe mit dem
   **post-final-norm**-Zustand — der letzte Eintrag von HFs
   `hidden_states` ist nicht die letzte Ebene.
4. Eine MLP-Sonde fütterte `rmsnorm(embedding)` statt
   `rmsnorm(embedding + attn_out)` — 72,3 % falscher Eingang.
5. Ein q/k/v-Vergleich ließ die **Biases** weg → 1347 % / 8613 %.
6. Die Attention-Sonde ließ **RoPE** weg → 47–151 %.
7. Dieselbe Sonde übergab **`lut_shift = 0`** statt
   `score_frac_bits − exp_input_frac` und rechnete damit `exp(−d/256)`
   statt `exp(−d/16)`.

**Was alle neun gemeinsam haben:** Kein einziger wurde durch Codelesen
gefunden. Gefunden wurden sie, weil ein Ergebnis **physikalisch unmöglich**
war — ein Attention-Fehler von 151 % ist mit einer Perplexität von
+7,5 % nicht vereinbar.

**Die Regeln, die daraus folgen:**

- **Jede Sonde braucht eine Selbstprüfung** — einen Fall mit bekanntem
  Ergebnis.
- **Aber die Selbstprüfung muss den fraglichen Baustein einschließen.**
  Die Attention-Sonde bestand ihre Prüfung bei n=1 mit 0,00 % — genau
  dem Fall, in dem das fehlende RoPE nichts tut.
- **Ein Patch, der nicht feuert, muss auffallen.** Instrumentierung
  zählt ihre Aufrufe und bricht bei null ab.
- **Unplausible Zahlen sind ein Fund**, kein Zwischenstand.

### Die 1-%-Regel

**Perplexitätsunterschiede unterhalb von etwa 1 % tragen keine
Information.** Sie sind über verschiedene Sequenzmengen nicht einmal
monoton.

Diese Regel entstand aus einem zurückgezogenen Befund: Eine angeblich
bessere rsqrt-Auflösung erwies sich als (a) nicht implementierbar und
(b) in der implementierbaren Fassung schlechter als der Status quo —
+2,16 % gegen +1,90 %, und das Vorzeichen kehrte sich um, je nachdem ob
mit 4 oder 16 Sequenzen gemessen wurde.

**Alle haltbaren Funde des Projekts hatten entweder eine große Marge
oder waren Tensor-Vergleiche** (→ [relativer L2](#relativer-l2)) statt
Perplexitätsvergleiche. Tensor-Vergleiche gegen Gleitkomma mit
*identischen* Gewichten und *identischem* Eingang isolieren eine einzelne
Operation — sie sind das schärfere Werkzeug.

### Sieben-Schritt-Doku-Kette

Nach jedem abgeschlossenen Patch, in dieser Reihenfolge:

1. Cargo-Versionen
2. Komponenten-Fahrplan
3. Komponenten-README
4. `README/Intern/Fahrplan-Master.md`
5. `README/Intern/README.md`
6. `README/Intern/State-of-the-Project.md`
7. Root-`README.md` (Komponenten-Tabelle)

Betrifft der Patch Protokollbegriffe, kommt **diese Datei** hinzu.

### Ganzzahligkeitsprüfung

Nach jeder Änderung an `model.rs`, `loader.rs` oder `calibrate/`:

```bash
grep -n "f32\|f64" <geänderte Dateien>
```

Treffer sind **nur** in Kalibrierungs-Metadaten akzeptabel, **niemals**
im Rechenpfad. Ein einziges übersehenes `f32` in einem Kernel bricht den
Determinismus und damit das gesamte Verifikationsmodell — und es würde
erst auffallen, wenn zwei Pods mit verschiedener Hardware auseinandergehen.

### Laufzeitschätzung und Fortschrittsbalken

Vor jedem Lauf, der länger als etwa eine Minute dauert, eine Schätzung
ansagen — **gerechnet, nicht geraten**: Arbeitseinheiten × gemessene Zeit
je Einheit. Die Raten stehen in `INTEGER_LLM/bench/README.md`
(0,5B ~24 tok/s, 7B ~2 tok/s mit `cpu-simd`).

Skripte mit mehreren Arbeitseinheiten geben einen Fortschrittsbalken aus
(`INTEGER_LLM/tests/diag/fortschritt.py`), Python-Ausgabe dabei
ungepuffert (`python -u`).

**Warum:** Ohne Schätzung ist nicht entscheidbar, ob sich Warten lohnt;
ohne Fortschrittsanzeige ist ein hängender Lauf von einem langsamen nicht
zu unterscheiden.

### CI-Umgebungsbeschränkungen

Die GitHub-CI hat **keine Modellgewichte** (gitignored) und **keine
Hardware-Backends**. Tests müssen deshalb sauber überspringen (Exit 0 mit
SKIP-Meldung), wenn Artefakte oder Backends fehlen. In der CI laufen nur
Unit-Tests (`cargo test --lib`).

**Die eigentliche Qualitätssicherung sind die lokalen Läufe** mit echten
Artefakten und echter Hardware.

### Commit-Regeln

- **Nicht selbstständig committen oder pushen.** Die Autoren des Repos
  übernehmen das nach Durchsicht. Wenn etwas fertig ist: kurze Meldung
  plus Titelvorschlag.
- **Commit-Titel zählen nur stichpunktartig die veränderten
  Bereiche/Punkte auf**, ohne tiefere Beschreibung — die steht ausführlich
  im Changelog.

### Gemeinsames Build-Verzeichnis

Alle Crates schreiben nach `target-shared/` im Wurzelverzeichnis
(`.cargo/config.toml`). Jedes Crate bleibt ein eigenständiges
Cargo-Projekt ohne gemeinsames Workspace-`Cargo.toml`; nur der
Ausgabeort ist geteilt.

**Grund:** Bei zwölf getrennten `target/`-Verzeichnissen lag jede
gemeinsame Abhängigkeit mehrfach auf der Platte (`myl_types` 66-mal,
`sha2` 34-mal) — 23,8 GB statt 2,1 GB bei identischem Ergebnis.

Wer ein Binary aus einem Skript aufruft, verdrahtet den Pfad **nicht**
fest, sondern nimmt `INTEGER_LLM/tests/cargo_paths.py`.

---

## M. Abkürzungen auf einen Blick

| Kürzel | Bedeutung | Abschnitt |
|---|---|---|
| **A16** | Aktivierungen in 16 Bit | [C](#w8a16) |
| **AS** | Autonomous System (Netzbetreiber-Einheit) | [G](#zonendiversität) |
| **BFT** | Byzantine Fault Tolerance | [F](#bft-byzantine-fault-tolerance) |
| **BLS** | Boneh–Lynn–Shacham (Signaturverfahren) | [E](#signatur-und-bls12-381) |
| **BPS** | Basispunkte (1 BPS = 0,01 %) | [I](#verteilung-der-prägung) |
| **DA** | Data Availability | [H](#da-datenverfügbarkeit) |
| **DST** | Domain-Separation-Tag | [E](#domain-separation-tag-dst) |
| **EMA** | Exponential Moving Average | [I](#ema-exponential-moving-average) |
| **GF(2⁸)** | Galois-Feld mit 256 Elementen | [E](#gf2⁸-galois-feld) |
| **GQA** | Grouped Query Attention | [D](#head-kopf-und-gqa) |
| **GST** | Global Stabilization Time | [F](#gst-global-stabilization-time) |
| **IC** | Inferenz-Credit | [I](#inferenz-credit-ic) |
| **KV** | Key/Value (Attention) | [D](#kv-cache) |
| **L0–L3** | Netzwerk-/Konsens-/Compute-/Agent-Schicht | [A](#schichtenmodell) |
| **LUT** | Lookup Table | [C](#lut-lookup-table-nachschlagetabelle) |
| **MLP** | Multi-Layer Perceptron (Feed-Forward-Block) | [D](#mlp--feed-forward-und-silu) |
| **MYL** | Der native Coin | [I](#myl) |
| **PoI** | Proof of Inference | [—](#poi-proof-of-inference) |
| **PoP** | Proof of Possession | [E](#rogue-key-angriff-und-proof-of-possession) |
| **PRNG** | Pseudo-Random Number Generator | [D](#sampling) |
| **RMSNorm** | Root Mean Square Normalization | [D](#rmsnorm) |
| **RoPE** | Rotary Position Embedding | [D](#rope-rotary-position-embedding) |
| **RTT** | Round-Trip Time | [G](#latencygraph-und-latenz-atteste) |
| **SIMD** | Single Instruction, Multiple Data | [L](#simd) |
| **SiLU** | Sigmoid Linear Unit (Swish) | [D](#mlp--feed-forward-und-silu) |
| **VRF** | Verifiable Random Function | [E](#vrf-verifiable-random-function) |
| **vTFE** | verifizierte Token-Forward-Äquivalente | [I](#vtfe-verifizierte-token-forward-äquivalente) |
| **W8** | Gewichte in 8 Bit | [C](#w8a16) |
| **zkML** | Zero-Knowledge Machine Learning | [H](#die-drei-stufen) |
| **θ_v** | Modellversion / Ausführungsspezifikation | [C](#θ_v-theta-v-modellversion) |

---

## Weiterlesen

**Öffentlich:**

| Was | Wo |
|---|---|
| Herleitung und Begründung | [`README/Whitepaper/myelith-whitepaper-v0.3.md`](Whitepaper/myelith-whitepaper-v0.3.md) |
| Ganzzahlige Inferenz im Detail | [`INTEGER_LLM/README/README.md`](../INTEGER_LLM/README/README.md) |
| Der Numerik-Vertrag | [`INTEGER_LLM/theta_v/spec.json`](../INTEGER_LLM/theta_v/spec.json) |

**Arbeitsintern** (nicht Teil der Veröffentlichung — nur im
Arbeitsverzeichnis vorhanden):

| Was | Wo |
|---|---|
| Architektur, Historie, offene Funde | `README/Intern/State-of-the-Project.md` |
| Was als Nächstes gebaut wird | `README/Intern/Fahrplan-Master.md` |
| Einstieg für Coding-Agenten | `AGENTS.md` |
