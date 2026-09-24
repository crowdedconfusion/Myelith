# Drei Vermutungen, eine Messung, ein Gewinn

**2026-09-23** · Apple M5 Pro, 24 GiB, 15 Kerne, GPU mit 16 Kernen ·
Rechenweg `cpu-simd` plus `metal`

---

## Die Frage

„Das Flaggschiff ist noch etwas zu langsam." Wo geht die Zeit hin?

## Die erste Messung war falsch, und das gehört an den Anfang

Die Reihe lief zuerst auf dem **skalaren Referenzpfad**, weil
`default = ["reference"]` gilt und ein `cargo build` ohne Merkmale genau
das nimmt. ⚑ **Das ist die Falle, vor der dieses Projekt ausdrücklich
warnt**, und sie hat funktioniert: Die Zahlen waren echt und
beschrieben den ausgelieferten Weg nicht. Der Client baut längst
richtig; nur die Messung tat es nicht.

## Was `cpu-simd` und `metal` wirklich bringen

| Modell | Grösse | prefill | decode | passt in 24 GiB |
|---|---|---|---|---|
| `myelith-0.6b` | 0,9 GB | −6 % | +33 % | ja |
| `myelith-4b` | 4,6 GB | **+132 %** | +25 % | ja |
| `myelith-8b` | 9,0 GB | **+157 %** | +34 % | ja |
| `myelith-35b-a3b` | **34,2 GB** | **±0 %** | **+7,5 %** | **nein** |

⛔️ **Das Flaggschiff ist das einzige Modell, dem der optimierte
Rechenweg fast nichts bringt.** Prefill: kein Unterschied, auf die
Nachkommastelle. Solange 34,2 GB auf 24,6 GB Speicher treffen, wartet
der Kern auf Seiten und nicht auf Rechenwerk. **Dort verpufft jede
Kernel-Optimierung.**

## Wohin die Zeit geht

| Modell | Rechnen | Wanduhr | Laden | Anteil |
|---|---|---|---|---|
| 0,6B | 0,6 s | 2,5 s | 1,9 s | 77 % |
| 4B | 1,4 s | 6,1 s | 4,7 s | 77 % |
| 8B | 1,8 s | 10,2 s | 8,4 s | 82 % |
| 35B | 6,6 s | 23,8 s | **17,2 s** | 72 % |

**Das Laden ist bei jeder Grösse der grösste Posten.** Also: woran liegt
es? Drei Vermutungen, alle drei gemessen, zwei davon falsch.

| Vermutung | Messung | Urteil |
|---|---|---|
| Die 62 682 Dateien | öffnen und abbilden: **0,51 s** | ✗ |
| Zu wenig Fäden | schon parallel; 34 GB in 17,2 s = **2 GB/s**, also Platte | ✗ |
| Die Prüfsumme | liest **jedes Byte** des Artefakts, bei jedem Start | ✓ |

⚑ **Der Kommentar an der Stelle nannte die Zahl seit jeher selbst.**
Niemand hatte sie mit der Wanduhr verglichen.

## Der Gewinn

Eine Marke neben dem Artefakt hält die Prüfsumme des Manifests und eine
Kennung der Dateilage. Stimmt beides, entfällt das erneute Lesen.

```
mit erzwungener voller Prüfung   19,41 s
mit Prüfmarke                    13,02 s      33 % weniger
decode_hash                      identisch
Konformität                      48/48
```

⚠️ **Der Preis, ausgesprochen:** Die Marke fängt kein gekipptes Bit ohne
neue Änderungszeit. `MYL_VOLLE_PRUEFUNG=1` schaltet sie ab. Die
Lookup-Tabellen werden **immer** geprüft, denn sie sind der
Zahlenvertrag.

## Die Platte nimmt keinen Schaden

Eine naheliegende Sorge, und sie lässt sich messen statt vermuten:

```
Auslagerung   vorher 1195,69M · während 1195,69M · nachher 1195,69M
```

Zwölf Messungen über einen ganzen 35B-Lauf, **kein Byte Bewegung**. Die
Gewichte sind ein Speicherabbild der Datei, also **saubere** Seiten mit
einem Original auf der Platte. Wird es eng, werden sie weggeworfen, nicht
geschrieben. ⚑ **Eine SSD altert an Schreibvorgängen.** Das Auslagern
kostet hier Zeit, keine Lebensdauer.

## Woraus die 34,2 GB bestehen

| Teil | Grösse | Anteil |
|---|---|---|
| Experten (Gemisch) | 30 870 MB | **90,6 %** |
| Einbettung und Kopf | 1 941 MB | 5,7 % |
| lineare Achtsamkeit | 965 MB | 2,8 % |
| volle Achtsamkeit | 260 MB | 0,8 % |

40 Ebenen × 256 Experten × 3 Matrizen 2048×512 = 32,2 Mrd. Parameter.
Bei 8 Bit sind das 30 720 MB, **bei 4 Bit 15 360 MB**, und das Artefakt
läge bei rund 18,7 GB. ⚑ **Dann passte es**, und dann wirkten auch die
anderen Hebel wieder.

⚑ **Bei einem Gemisch sind die Aussichten besser als bei einem dichten
Modell:** Jeder Experte sieht nur 3,1 % der Token (8 von 256). Ein
Gewicht, das selten gebraucht wird, verträgt Gröberes. ⛔️ Der Preis ist
trotzdem hoch: Das ändert den Zahlenvertrag, also neue
Konformitätsvektoren, neue Goldvektoren und eine Perplexitätsmessung.

## Der Kontext ist nicht das Problem

```
40 Ebenen, davon 10 mit voller Achtsamkeit (jede 4.)
30 Ebenen rekurrent  ⚑ wachsen NICHT mit dem Kontext

je Token   20,0 KiB
  32 768 Token → 0,62 GiB        dicht wären es 2,50 GiB
 131 072 Token → 2,50 GiB
```

⚑ **Die hybride Bauart hat die Kontextfrage schon gelöst.** Gegenüber
34 GB Gewichten ist der Cache ein Rundungsfehler.

## Was liegen bleibt, und warum

**Der doppelte Kopf.** `lm_head.bin` (int16, 970 MB) und
`lm_head_weight.bin` (int8, 485 MB) liegen beide im Artefakt; im
Rechenpfad rührt niemand den int8 an, sobald der int16 vorliegt. **Aber
der Rückwärtspass braucht ihn**, also wäre Wegnehmen kein freier
Gewinn. 1,4 % des Artefakts, und an der Speicherfrage ändert es nichts.

**Vier Bit.** Siehe oben: der einzige Hebel, der die Speicherklippe
wirklich beseitigt, und der einzige mit einem Preis, den nur der
Projektinhaber zahlen kann.

## Drei Sätze zum Mitnehmen

1. **Miss, bevor du baust.** Zwei von drei Vermutungen über die
   Ladezeit waren falsch, und beide hätten Tage gekostet.
2. **Prüfe, womit du misst.** Die erste Reihe lief auf dem falschen
   Rechenweg, und die Zahlen sahen völlig plausibel aus.
3. **Beim Flaggschiff ist nicht der Rechenweg das Problem, sondern der
   Speicher.** Das ist unbequem, weil die Abhilfe den Zahlenvertrag
   anfasst, aber es steht so in der Messung.
