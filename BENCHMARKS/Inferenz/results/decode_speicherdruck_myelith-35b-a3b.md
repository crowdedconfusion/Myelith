# Decode unter Speicherdruck: myelith-35b-a3b

**Gemessen am 2026-10-15** auf dem M5 Pro (24 GiB), `bench_probe` mit
`cpu-simd`, Prompt: die ersten 600 Zeichen von `belegung/prueftext.txt`,
64 Token gierig, davor ein Lauf mit 16 Token zum Anwärmen. Speicherdruck
mit `ballon.py` (0, 6 und 9 GiB). Zellen: Token/s im Decode, zwei Runden.
`decode_hash` in allen Läufen `c4da3011c4772677`: **jede Variante rechnet
bitgleich.** Die Swapouts blieben in allen Reihen unverändert.

## 1. Anwärmen je Matrix statt je Experte (übernommen)

Bis hierher kündigte je gewähltem Experten ein Faden gate, up und down
nacheinander beim System an; jetzt ist jede der 24 Matrizen ein eigener
Auftrag.

| frei | je Experte | **je Matrix** |
|---|---|---|
| 17,7 bis 18,4 GiB | 15,9 / 15,7 | **18,7 / 17,2** |
| 12,5 bis 12,7 GiB | 12,9 / 12,9 | **14,1 / 13,9** |
| 9,6 GiB | 9,7 / 10,2 | **10,6 / 10,4** |

## 2. Vorlader über Ebenen hinweg (gebaut, standardmäßig aus)

Der Router der nächsten Ebene auf dem Residualstrom dieser Ebene sagt deren
Wahl vorher (Trefferquote an 1977 Token: 78,8 % mit 8 Kandidaten, 90,2 %
mit 12, 94,2 % mit 16; Präzision je Rang 98, 96, 93, 88, 81, 71, 59, 46 %).
Gemessen noch mit dem Anwärmen je Experte:

| frei | aus | K = 4 | K = 6 | K = 8 | K = 12 |
|---|---|---|---|---|---|
| 17,9 bis 18,1 GiB | 16,4 bis 17,3 | 15,4 / 15,8 | 16,6 / 16,9 | 17,4 / 17,4 | 16,4 / 16,9 |
| 12,4 bis 12,5 GiB | 12,1 bis 12,7 | 12,1 / 12,6 | 12,9 / 13,0 | 12,7 / 13,1 | 10,8 / 10,6 |
| 9,5 bis 9,6 GiB | 9,6 bis 10,6 | 10,3 / 10,2 | 9,8 / 10,1 | 9,5 / 9,2 | 6,2 / 6,8 |

Kein verlässlicher Gewinn: Wo Speicher reicht, kostet die Vorhersage mehr,
als sie spart; wo er knapp ist, drängen Fehlgriffe nützliche Experten aus
dem Cache.

## 3. Lesen statt Rat (verworfen)

`pread` in einen Wegwerfpuffer mit `mincore` davor, gegen den Rat
`MADV_WILLNEED`, beide je Matrix:

| frei | Rat | Lesen |
|---|---|---|
| 17,7 GiB | 18,7 / 17,2 | 17,7 / 16,2 |
| 12,5 GiB | 14,1 / 13,9 | 13,4 / 13,5 |
| 9,6 GiB | 10,6 / 10,4 | 10,2 / 10,0 |

## 4. Eigene Lesefäden (verworfen)

24 Lesefäden statt der Rechenfäden, je Matrix: 17,4 / 17,9 gegen 18,7 /
16,9 (18,2 GiB frei), 13,2 / 13,2 gegen 13,0 / 13,8 (12,5), 10,6 / 11,0
gegen 10,6 / 10,1 (9,5). Im Rauschen.

## 5. Wohin die Zeit geht (Profil mit `sample`)

- **Decode, 9,6 GiB frei, 9 Token/s:** 53 % des Hauptfadens im Warten auf
  das Anwärmen der Experten, 19 % Aufmerksamkeit und Zustandsschichten,
  15 % Rechnung der Experten, 6 % Kopf. Die Platte liefert dabei rund
  2,9 GB/s (`iostat`), beim Prefill 6 bis 8 GB/s: Der Decode wartet auf
  Latenz, nicht auf Bandbreite.
- **Prefill, 772 Token, 9,6 GiB frei, mit `metal`, 10,3 s:** 37 % in den
  Zustandsschichten (`spalten_schritt`, reine Rechnung), 27 % Warten auf
  Experten, 14 % Gemischrechnung auf der GPU.

## Wiederholen

```bash
cd INTEGER_LLM/runtime && cargo build --release --features cpu-simd --bin bench_probe && cd ../..
python3 BENCHMARKS/Inferenz/ballon.py 9 &   # warten auf „bereit“
./SYSTEM/full-build/release/bench_probe INTEGER_LLM/artifacts/myelith-35b-a3b \
  "$(head -c 600 BENCHMARKS/Inferenz/belegung/prueftext.txt)" 64 [--vorladen <k>]
kill %1
```

## 6. Gemischte Genauigkeit: die seltenen Experten ternär (gemessen am selben Tag, nachmittags)

Gebaut mit `INTEGER_LLM/tools/experten_mischen.py` aus dem int8- und dem
ternären Artefakt; selten heißt: nach einer Zählung an 4800 Token
(WikiText-Training und Lerntext, `expertenprofil_zaehltext_myelith-35b-a3b.json`),
die seltensten Experten bis zum genannten Anteil der Aufrufe.

**Qualität**, `artefaktvergleich.py 16 256` (WikiText-Test):

| Artefakt | ternäre Experten | Anteil der Aufrufe | Experten | Perplexität |
|---|---|---|---|---|
| int8 | 0 | 0 | 32,2 GB | 11,01 |
| gemischt-20 | 2809 | 1,9 % | 25,8 GB | 11,17 (+1,5 %) |
| **gemischt-50** | **3997** | **4,9 %** | **23,0 GB** | **11,38 (+3,4 %)** |
| gemischt-100 | 5172 | 9,9 % | 20,3 GB | 11,87 (+7,8 %) |
| gemischt-250 | 7091 | 24,9 % | 15,9 GB | 12,93 (+17 %) |
| ternär | alle | 100 % | 8,6 GB | 21,21 (+93 %) |

Vier Fragen über `myl frage`, gierig: gemischt-50 antwortet wie int8
(17 + 23 − 8 = 32 mit Rechenweg, Fibonacci in Python mit Docstring, zwei
Sätze zum dezentralen Netz, Prozentaufgabe mit richtigem Ansatz), ohne
Schleifen.

**Decode** (Token/s, zwei Runden):

| frei | int8 | gemischt-20 | gemischt-50 | gemischt-100 |
|---|---|---|---|---|
| 17,4 GiB | 15,9 / 15,2 | 16,8 / 16,7 | **18,9 / 19,0** | 19,5 / 20,1 |
| 11,9 GiB | 11,4 / 11,4 | 11,5 / 11,4 | **12,1 / 12,0** | 12,7 / 12,5 |
| 8,7 GiB | 7,8 / 7,8 | 7,5 / 8,2 | **8,7 / 8,8** | 9,2 / 8,6 |

📌 Je knapper der Speicher, desto weniger bringt es, nur die seltenen
Experten zu verkleinern: Dort fasst der Cache nicht einmal die häufigen,
und die Fehlzugriffe treffen vor allem diese.

## 7. Rechnen, während die Ebene lädt

Liegen einige der gewählten Experten im Speicher (`mincore`, erste Seite),
rechnen sie, während Lesefäden die fehlenden holen; bitgleich (je Artefakt
derselbe `decode_hash`). Decode, warten gegen überlappen:

| frei | int8 | gemischt-50 |
|---|---|---|
| 17,1 bis 17,7 GiB | 16,6 / 16,2 gegen 16,5 / 16,7 | 20,3 / 19,5 gegen 19,0 / 19,0; 20,7 / 20,7 gegen 19,4 / 19,4 |
| 12,0 GiB | 13,2 / 12,4 gegen **14,0 / 13,5** | 14,6 / 14,5 gegen **15,3 / 15,2** |
| 8,9 bis 9,0 GiB | 10,1 / 9,9 gegen **10,9 / 10,9** | 10,2 / 10,1 gegen **11,2 / 11,0**; 10,7 / 10,5 gegen 10,9 / 11,3 |

Übernommen, aber **nur bei knappem Speicher** (der Client schaltet es ein,
wenn der Plan weniger als die Hälfte der Experten im Speicher sieht).
Gemessen und verworfen: erst ab zwei fehlenden Experten teilen (der Gewinn
bei 9 GiB verschwand).

## 8. Der Kern der Zustandsschicht (`zustandsmessung`, 32 Köpfe zu 128 × 128, Fenster 64)

| Fassung | 1 Faden | 15 Fäden |
|---|---|---|
| Ausgang | 567 bis 582 µs je Token | 72 bis 73 |
| Puffer je Faden statt Vektoren je Aufruf | 603 | 76 bis 77 (verworfen) |
| `target-cpu=apple-m1` | 573 | 72 (wie die Vorgabe) |
| `target-cpu=native` (M5 Pro) | 533 bis 538 | 68 bis 69 (−6 %, aber nicht übertragbar) |

Prüfsumme in allen Fassungen `906dae099c23efdb`. Skalierung: linear bis 5
Fäden (die Super-Kerne), 15 Fäden ergeben das 8-Fache. Der Kern liegt bei
rund 1,1 ns je Zustandswert und Token, nah am skalaren Limit; NEON hat keine
64-Bit-Multiplikation, und ihre Nachbildung kostet mehr Befehle, als sie
spart.
