# Gitter für die Experten im Vergleich: myelith-35b-a3b

**Gemessen am 2026-10-15** auf dem M5 Pro (24 GiB). Frage: Welche
trainingsfreie Rundung der Expertengewichte kommt dem int8 am nächsten, und
was kostet sie an Bits? Werkzeug `INTEGER_LLM/calibrate/src/gittervergleich.py`,
Probe `INTEGER_LLM/tests/test_gittervergleich.py`.

## 1. Mitschnitt

`expertenmitschnitt` über 16 384 Token, in Folgen zu 512 Token: WikiText-2
Training ab Zeichen 2 000 000 (60 000 Zeichen), der Rust-Quelltext von
`COMPUTE_PIPELINE/dispatcher/src/local/plan.rs` (25 000), `README/Glossar.md`
(20 000) und `belegung/lerntext.txt`, in Stücken zu 2000 Zeichen
verschränkt. 21,4 Token/s, 7,5 GB, nicht versioniert. Der Prüftext der
Perplexität (WikiText-2 Test) ist davon getrennt.

## 2. Ausgabefehler je Matrix

`gittervergleich.py vergleich`, Ebenen 0, 3, 10, 20, 30, 37, 39, je 8
Experten über die Häufigkeit verteilt (nur solche mit mindestens 16
Haltetoken), die letzten 2048 Token zurückgehalten. Maß: relativer
quadrierter Ausgabefehler ‖XWᵀ − XQᵀ‖² / ‖XWᵀ‖² auf den Haltetoken, wie in
`ternaer_gptq.py`. Jedes Verfahren ist GPTQ (Dämpfung 0,1, Gruppe 128) mit
eigenem Gitter.

| Verfahren | gate | up | down | Mittel | Bits je Gewicht |
|---|---|---|---|---|---|
| `ternaer` | 0,0807 | 0,1168 | 0,2147 | **0,1374** | 2,06 |
| `asym` | 0,0805 | 0,1160 | 0,2049 | 0,1338 | 2,13 |
| `had+ternaer` | 0,0761 | 0,1116 | 0,1703 | 0,1193 | 2,06 |
| `ternaer+aus10` | 0,0667 | 0,0983 | 0,1711 | 0,1120 | 2,30 |
| `ternaer+r16` | 0,0720 | 0,1065 | 0,1842 | 0,1209 | 2,37 |
| `ternaer+r32` | 0,0665 | 0,0991 | 0,1703 | 0,1120 | 2,69 |
| `had+asym+aus10+r32` | 0,0557 | 0,0833 | 0,1270 | **0,0887** | 3,0 |
| `int2` | 0,0560 | 0,0830 | 0,1513 | 0,0968 | 2,13 |
| `zwei` | 0,0114 | 0,0167 | 0,0296 | **0,0192** | 3,33 bis 4,13 |
| `had+zwei` | 0,0106 | 0,0158 | 0,0254 | 0,0172 | wie `zwei` |
| `had+zwei+aus10` | 0,0094 | 0,0141 | 0,0225 | **0,0154** | + 0,24 |
| `neun` | 0,0118 | 0,0175 | 0,0310 | 0,0201 | 4,06 |
| `int3` | 0,0137 | 0,0203 | 0,0366 | 0,0236 | 3,13 |
| `int4` | 0,0036 | 0,0053 | 0,0095 | **0,0062** | 4,13 |

Bits je Gewicht: ternär 2 Bit je Wert und ein int8-Betrag je 128; `zwei`
dicht gepackt 5 Trits je Byte (3,33) oder je Ebene wie das heutige ternäre
Format (4,13); ein Ausreißer 8 Bit Wert und 16 Bit Stelle; Rang r bei
512 × 2048 rund r · 0,0195 Bit in int8.

Gegenprobe: Ebene 20, 4 Experten, `ternaer` 0,1448 (das alte Werkzeug hielt
über alle Ebenen 0,149 fest). Dauer: rund 108 s je Ebene für alle 14
Verfahren.

## 3. Perplexität

`artefaktvergleich.py 16 256` (WikiText-2, 2150 Positionen, dieselben Folgen
wie die ganze Reihe). Die Artefakte mit zwei Ebenen liegen im
int8-Behälter (`gittervergleich.py bauen`, Verfahren `zweiganz`, ab Ebene 1,
15 und 28 `zweischnell` mit 4 statt 12 Runden der Anpassung, gemessen
1,4 % mehr Fehler): Die Werte jeder Gruppe sind genau s₁t₁ + s₂t₂ mit
ganzen s₁, s₂, also rechnet der Behälter dieselben ganzen Zahlen wie ein
Format mit zwei ternären Ebenen. 0 Werte geklemmt.

| Artefakt | Experten | Perplexität | gegen int8 |
|---|---|---|---|
| `myelith-35b-a3b` (int8) | int8 | 11,0106 | |
| `myelith-35b-a3b-zweiebenen` | **alle zwei Trit-Ebenen** | **11,2875** | **+2,5 %** |
| `myelith-35b-a3b-gemischt-50` | seltene ternär (4,9 % der Aufrufe), sonst int8 | 11,3849 | +3,4 % |
| `myelith-35b-a3b-zweiebenen-gemischt-50` | seltene ternär, sonst zwei Ebenen | 11,6858 | +6,1 % |
| `myelith-35b-a3b-ternaer` | alle ternär | 21,2123 | +93 % |

int8, ternär und gemischt-50 im selben Aufbau nachgemessen: gleich den
früheren Werten auf vier Stellen.

📌 **Die Kosten addieren sich:** gemischt-50 kostet 0,37, die zweite Ebene
0,28, zusammen 0,68 (gemessen 0,68).

## 4. Antworten

Vier Prüffragen über `myl frage`, gierig, 160 Token: `zweiebenen`
antwortet wie int8 (17 + 23 − 8 = 32 mit Rechenweg, fast wörtlich gleich;
Fibonacci mit Docstring, iterativ empfohlen; zwei Sätze zum dezentralen Netz;
Prozentaufgabe mit Einkauf 50 Euro und Gewinn 62 − 50), ohne Schleifen.

GSM8K, erste 50 Aufgaben, gierig, ohne Denkmodus, 512 Token:

| Artefakt | richtig | Antworten an der Grenze von 512 Token |
|---|---|---|
| int8 | 41/50 | 14 |
| **zwei Ebenen** | **41/50** | 12 |
| ternär | 20/50 | 6 |

Beide richtig bei 38 Aufgaben, nur int8 bei 3, nur zwei Ebenen bei 3;
dieselbe Zahl als Antwort bei 38 von 50. Bei 50 Aufgaben ist das
ununterscheidbar. (Am 2026-09-28 stand das int8 hier bei 40/50.)

## 5. Was bestenfalls erreichbar ist, int4, und wo der Verlust sitzt (2026-10-16)

**Ideale Transformationscodierung** (`gittervergleich.py schranke`, Ebenen
3, 20, 37, je 8 Experten, Basis aus den Lerntoken, Fehler auf 2048
Haltetoken, gaußsche Näherung):

| Matrix | R = 1,58 | R = 2,06 | R = 3,17 |
|---|---|---|---|
| gate, Basis der Ebene | 0,0526 | 0,0270 | 0,0058 |
| up, Basis der Ebene | 0,0736 | 0,0378 | 0,0081 |
| down, ohne Drehung | 0,0963 | 0,0495 | 0,0106 |
| zum Vergleich: 2^(−2R) | 0,1119 | 0,0575 | 0,0123 |

Im Eigenraum des einzelnen Experten auf den Haltetoken: gate 0,0548, up
0,0772, down 0,1121 bei 1,58 Bit (auf den Lerntoken 0,002 bis 0,015, also
überangepasst).

**int4 im int8-Behälter** (`bauen … int4`, GPTQ, Gruppe 128, mit Versatz,
48 988 Werte auf ±127 geklemmt, das sind 0,0016 % aller Expertengewichte):

| Artefakt | Experten int4 | Perplexität | gegen int8 | GSM8K |
|---|---|---|---|---|
| int8 | keine | 11,0106 | | 41/50 |
| `int4-haeufig-500` | die 1399 häufigsten (Hälfte der Aufrufe) | 11,0792 | +0,62 % | |
| `int4-selten-500` | die 8841 seltenen (andere Hälfte) | 11,1569 | +1,33 % | |
| `int4` | alle | **11,2400** | **+2,08 %** | 43/50 |
| `zweiebenen` | (zwei Trit-Ebenen, alle) | 11,2875 | +2,52 % | 41/50 |

Die Mischungen mit `tools/experten_mischen.py <int8> <int4> <profil> <ziel> --anteil 500`
und umgekehrt. 📌 Je Aufruf kosten die seltenen Experten doppelt so viel
wie die häufigen; zusammen 1,95 % gegen 2,08 % für alles int4.

## 6. Schrumpfen für die seltenen Experten, kleinere Gruppen, und eine größere Probe (2026-10-16)

**Seltenheit nachgestellt** (`gittervergleich.py seltenheit`, häufige
Experten, deren H nur aus k Lerntoken kommt, Fehler auf ihren Haltetoken,
int4 g128, Mittel gate / up / down):

| Lerntoken k | ohne Schrumpfen | `--schrumpf 8192` |
|---|---|---|
| 4 | 0,0045 / 0,0073 / 0,0104 | 0,0028 / 0,0047 / 0,0074 |
| 256 | 0,0039 / 0,0065 / 0,0118 | 0,0028 / 0,0047 / 0,0074 |
| alle | 0,0028 / 0,0048 / 0,0076 | 0,0028 / 0,0047 / 0,0074 |

Unter rund 2000 Token rundet GPTQ kaum besser als mit 4. Das Schrumpfen
H = XᵀX + n₀ Σ setzt für gate und up die Kovarianz aller x der Ebene ein,
für down die Kovarianz des eigenen h, nachgerechnet auf 8192 Token der
Ebene aus x und den echten Gewichten von gate und up (zwei feste Shifts je
Ebene, Rest unter 0,0005). Gepoolte h aller Experten statt des eigenen
sind schlechter. Ein zweiter Mitschnitt über 65 536 Token (30 GB) brachte
je Matrix nichts über das Schrumpfen hinaus (Ebene 20, vier Experten, 8192
Haltetoken) und wurde deshalb nicht verbaut.

**Gruppengröße je Matrix** (65 536er Mitschnitt, Schrumpfen 8192, Ebenen 0,
3, 10, 20, 30, 37, 39 je 6 Experten, 8192 Haltetoken; Bits je Gewicht mit
einer 16-Bit-Skala je Gruppe):

| Gitter | Bits | Fehler je Matrix |
|---|---|---|
| int3 g128 | 3,13 | 0,0173 |
| int3 g32 | 3,50 | 0,0123 |
| int4 g128 | 4,13 | 0,0045 |
| int4 g64 | 4,25 | 0,0038 |
| int4 g32 | 4,50 | 0,0029 |
| int5 g128 | 5,13 | 0,0011 |

Sortieren nach diag(H) mit festen Gruppen spart 2 bis 3 %. Das Runden des
int4-Gitters auf den int8-Behälter kostet bei g32 nichts Messbares
(Ebene 39: 0,0028 ideal, 0,0029 gerundet).

**Perplexität auf zwei Proben.** ⚠️ Die Probe aus Abschnitt 3 (16 Folgen,
2150 Positionen) war für Abstände unter 2 % zu klein: Auf 48 Folgen
(7173 Positionen) liegen alle int4-Artefakte unter 1 %, und die Reihenfolge
stimmt erst dort mit dem Fehler je Matrix überein.

| Artefakt | Experten | 16 Folgen | gegen int8 | 48 Folgen | gegen int8 | GSM8K |
|---|---|---|---|---|---|---|
| `myelith-35b-a3b` | int8 | 11,0106 | | 10,3410 | | 41/50 |
| `-int4` | int4 g128 | 11,2400 | +2,08 % | 10,4146 | +0,71 % | 43/50 |
| `-int4-schrumpf` | int4 g128, `--schrumpf 8192` | 11,1843 | +1,58 % | 10,3968 | **+0,54 %** | 40/50 |
| `-int4-g32` | int4 g32, `--schrumpf 8192` | 11,1959 | +1,68 % | 10,3858 | **+0,43 %** | 42/50 |
| `-zweiebenen` | zwei Trit-Ebenen | 11,2875 | +2,52 % | 10,6351 | +2,84 % | 41/50 |
| `-int4-schrumpf-haeufig-500` | die 1399 häufigen int4 g128 geschrumpft | 11,0804 | +0,63 % | | | |
| `-int4-schrumpf-selten-500` | die 8841 seltenen int4 g128 geschrumpft | 11,1445 | +1,22 % | | | |

Bau mit Schrumpfen: 15 671 Werte auf ±127 geklemmt (g128) und 46 685
(g32), gegen 48 988 ohne.

📌 **Die „seltenen“ 8841 Experten sind nicht die Hälfte der Aufrufe.** Das
Profil (4524 Token Zähltext) teilt sie so, aber auf dem 16 384er Mitschnitt
tragen sie 61,6 % und auf dem 65 536er 65,2 % der Aufrufe. Die Aussage aus
Abschnitt 5, ein seltener Experte koste je Aufruf doppelt so viel, ist
damit zu stark; dazu kommt das Rauschen der kleinen Probe.

## 7. Das gewählte Format: int4 g128 mit ganzem Gitter, gepackt (2026-10-16)

Gewählt wurde int4 in Gruppen von 128 mit Schrumpfen. Damit es gepackt
werden kann (Speicherform `vierbit_g128`: Wert a q + b mit ganzen a und b,
q von 0 bis 15), baut das Gitter `int4ganz` nur ganzzahlige Schritte und
Versätze, und alle Stufen liegen in −127 bis 127. Der Behälter ist dann
genau das, was der Packer ohne Runden packt.

| | int4 g128, Schrumpfen | int4ganz g128, Schrumpfen |
|---|---|---|
| Fehler je Matrix (`vergleich`, Mittel der Probeebenen) | 0,0068 | 0,0071 (+4 %) |
| auf ±127 geklemmt | 15 671 | **0** |
| Gruppen nicht vierbittig | 93 % | **0 von 251 658 240** |
| Perplexität, 48 Folgen | 10,3968 (+0,54 %) | **10,3467 (+0,06 %)** |

Beide Perplexitäten und das int8 (10,3410) am selben Tag mit demselben
Programm gemessen; die Werte aus Abschnitt 6 kamen genau wieder. ⚠️ Warum
das ganze Gitter trotz größerem Fehler je Matrix am Modell besser ist, ist
nicht belegt. Naheliegend ist das Klemmen: Geklemmt werden die größten
Gewichte.

**Gepackt** (`vierbit_packen`, 30 s): 30 720 Expertentensoren in vier Bit,
130 Aufmerksamkeitsmatrizen bleiben int8 (nicht vierbittig, wie erwartet).
Expertengewichte 16,61 GB statt 32,21 GB, das Artefakt 19 GB statt 33.

**Bitgleich:** gepackt gegen Behälter IDENTISCH 439/439 Zeilen
(`--bitgleich`, 4 Folgen zu 128), der Test `runtime/tests/vierbit.rs`
grün, `decode_hash` gleich. Gegenprobe int8 gegen gepackt: 0/64.

| `myelith-35b-a3b` gegen `-int4` | int8 | int4 gepackt |
|---|---|---|
| Größe | 33 GB | 19 GB |
| Perplexität, 48 Folgen | 10,3410 | 10,3467 (+0,06 %) |
| GSM8K (50, 512 Token) | 41/50 | 41/50 (866 s gegen rund 1400 s im Behälter) |
| Decode, 64 Token (Wechsel A B B A) | 18,8 / 17,0 Tok/s | 23,5 / 21,3 Tok/s |
| Prefill | 2,17 / 2,15 s | 1,32 / 1,52 s |
| Perplexität 48 Folgen, Laufzeit | 304 s | 156 s |
| Seitenfehler je `bench_probe` | 2,61 Mio. | 1,16 Mio. |

Maschine: M5 Pro, 24 GiB, `cpu-simd`, beide Seiten mit demselben Bau.

## Wiederholen

```bash
V=INTEGER_LLM/calibrate/.venv/bin/python3
(cd INTEGER_LLM/runtime && cargo build --release --features cpu-simd --bin expertenmitschnitt --bin perplexity_probe --bin aufgabenprobe)
./SYSTEM/full-build/release/expertenmitschnitt INTEGER_LLM/artifacts/myelith-35b-a3b <text> <mitschnitt> --token 16384
$V INTEGER_LLM/calibrate/src/gittervergleich.py vergleich INTEGER_LLM/artifacts/myelith-35b-a3b <mitschnitt> \
  0,3,10,20,30,37,39 8 2048 ternaer,asym,had+ternaer,ternaer+aus10,ternaer+r16,ternaer+r32,had+asym+aus10+r32,int2,zwei,had+zwei,had+zwei+aus10,neun,int3,int4
# Bau in drei Teilen nebeneinander (je rund 7 bis 10 min je Ebene), dann einmal abschliessen
for r in 0-13 14-26 27-39; do OMP_NUM_THREADS=5 $V INTEGER_LLM/calibrate/src/gittervergleich.py bauen \
  INTEGER_LLM/artifacts/myelith-35b-a3b <mitschnitt> <ziel>.unfertig zweiganz --ebenen $r & done; wait
$V INTEGER_LLM/calibrate/src/gittervergleich.py abschluss INTEGER_LLM/artifacts/myelith-35b-a3b <ziel>.unfertig
mv <ziel>.unfertig <ziel>
INTEGER_LLM_MODEL=myelith-35b-a3b $V -u BENCHMARKS/Inferenz/artefaktvergleich.py 16 256 INTEGER_LLM/artifacts/<ziel>
# Abschnitt 6: int4 mit Schrumpfen (Gruppe 128 oder 32), Probe mit 48 Folgen (rund 5 min je Artefakt)
$V INTEGER_LLM/calibrate/src/gittervergleich.py seltenheit INTEGER_LLM/artifacts/myelith-35b-a3b <mitschnitt> 3,20,37 4 4,256,100000 0,8192 2048
for r in 0-13 14-26 27-39; do OMP_NUM_THREADS=4 $V INTEGER_LLM/calibrate/src/gittervergleich.py bauen \
  INTEGER_LLM/artifacts/myelith-35b-a3b <mitschnitt> <ziel>.unfertig int4 --schrumpf 8192 --gruppe 32 --ebenen $r & done; wait
INTEGER_LLM_MODEL=myelith-35b-a3b $V -u BENCHMARKS/Inferenz/artefaktvergleich.py 48 256 INTEGER_LLM/artifacts/<ziel>
$V -u BENCHMARKS/Inferenz/gsm8k.py MODELS/llm/Qwen3.6-35B-A3B INTEGER_LLM/artifacts/<ziel> 50 512 <ausgabe.jsonl>
# Abschnitt 7: ganzes Gitter, packen, bitgleich abnehmen
(cd INTEGER_LLM/runtime && cargo build --release --features cpu-simd --bin vierbit_packen --bin bench_probe)
for r in 0-13 14-26 27-39; do OMP_NUM_THREADS=4 $V INTEGER_LLM/calibrate/src/gittervergleich.py bauen \
  INTEGER_LLM/artifacts/myelith-35b-a3b <mitschnitt> INTEGER_LLM/artifacts/myelith-35b-a3b-int4ganz.unfertig int4ganz --schrumpf 8192 --ebenen $r & done; wait
$V INTEGER_LLM/calibrate/src/gittervergleich.py abschluss INTEGER_LLM/artifacts/myelith-35b-a3b INTEGER_LLM/artifacts/myelith-35b-a3b-int4ganz.unfertig
mv INTEGER_LLM/artifacts/myelith-35b-a3b-int4ganz.unfertig INTEGER_LLM/artifacts/myelith-35b-a3b-int4ganz
./SYSTEM/full-build/release/vierbit_packen INTEGER_LLM/artifacts/myelith-35b-a3b-int4ganz INTEGER_LLM/artifacts/myelith-35b-a3b-int4
INTEGER_LLM_MODEL=myelith-35b-a3b $V -u BENCHMARKS/Inferenz/artefaktvergleich.py 4 128 \
  INTEGER_LLM/artifacts/myelith-35b-a3b-int4ganz INTEGER_LLM/artifacts/myelith-35b-a3b-int4 --bitgleich
(cd INTEGER_LLM/runtime && cargo test --release --features cpu-simd --test vierbit -- --ignored)
INTEGER_LLM_MODEL=myelith-35b-a3b $V -u BENCHMARKS/Inferenz/artefaktvergleich.py 48 256 INTEGER_LLM/artifacts/myelith-35b-a3b-int4
for x in myelith-35b-a3b myelith-35b-a3b-int4 myelith-35b-a3b-int4 myelith-35b-a3b; do
  ./SYSTEM/full-build/release/bench_probe INTEGER_LLM/artifacts/$x "Erkläre in einfachen Worten, wie ein Kühlschrank funktioniert." 64; done
```

⚠️ Das Vergleichswerkzeug sucht einen bloßen Namen nicht unter
`INTEGER_LLM/artifacts/`; der Pfad gehört ausgeschrieben dazu.
