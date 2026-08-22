# Entscheidung 0.1: Trägt das Quantisierungsschema im Rückwärtspass?

**Datum:** 2026-08-22 · **Modell:** Qwen2.5-0,5B · **Korpus:** WikiText-2
(dieselben Sequenzen wie der Perplexitäts-Entscheidungspunkt 12.21)
**Skript:** `TRAINING/tests/diag/backward_reference_simulation.py`

## Ergebnis in einem Satz

**Es trägt, aber nur mit stochastischem Runden der Gewichte.** Mit
Rundung zur nächsten Stufe bricht das Training (+29,9 % gegen die
Referenz), mit stochastischem Runden liegt es **+0,67 %** darüber. Der
Rückwärtspass selbst war nie das Problem; die Gradientenquantisierung
reproduziert die Referenz exakt.

## Das Urteil

| Variante | zurückgehaltener Text, Anfang → Ende | Abstand |
|---|---|---|
| Gleitkomma-Referenz | 3,0472 → **2,9795** | |
| Ganzzahlschema, Rundung zur nächsten Stufe | 3,0689 → **3,8713** | **+29,9 %** |
| Ganzzahlschema, **stochastisches Runden** | 3,0770 → **2,9994** | **+0,67 %** |

Kriterium ≤ 10 %. **Trägt**, sofern die Gewichtsquantisierung stochastisch
rundet.

> **Diese Zeile ist eine Korrektur.** Die erste Fassung dieses Protokolls
> schloss mit „trägt nicht". Sie hatte recht über das gemessene Schema und
> unrecht über die Frage: Gemessen war Round-to-Nearest, und dass diese
> Rundungsart bei kleinen Aktualisierungen scheitert, ist seit Gupta et
> al. 2015 bekannt. Die Abschnitte unten sind die Messungen, die dorthin
> geführt haben; sie bleiben stehen, weil der Weg zum Ergebnis gehört.

## Der Fund am Messgerät selbst

Der Fahrplan nennt als Kriterium den **Verlustverlauf**. Danach gemessen
fiel das Ganzzahlschema von 2,54 auf 0,25, also weit unter die Referenz,
und die erste Fassung der Auswertung meldete **„trägt"**.

Auf zurückgehaltenem Text stieg der Verlust im selben Lauf von 3,07 auf
4,10. Das Schema hat nicht besser gelernt, es hat die zwanzig
Trainingstexte auswendig gelernt.

Ein Kriterium über den Trainingsverlust hätte einen Fehlschlag als Erfolg
gebucht. Das Urteil hängt deshalb an einer Haltemenge, die nie trainiert
wird. Es ist dieselbe Fehlerklasse wie Fund 33 bis 37: ein Prüfmittel,
das eine Aussage trifft, die es nicht deckt.

## Messung 1: Dynamikbereich der Gradienten

Die Zahl, die laut Fahrplan über die Eskalation entscheidet, erhoben über
alle 168 linearen Schichten und 200 Schritte:

| | Bits |
|---|---|
| Spanne insgesamt, Median | **26,6** |
| Spanne insgesamt, Maximum | 92,4 |
| davon **zwischen** den Blöcken, Median | 10,5 |
| davon **innerhalb** eines Blocks, Median | 10,5 |
| von int8 abgedeckt | 7 |

Die Spanne teilt sich zu gleichen Teilen auf beide Ebenen. Eine feinere
Skalengranularität (je Kanal statt je Block) könnte damit höchstens die
Hälfte des Problems adressieren.

**Sättigung: strukturell null.** Der Shift folgt aus dem Absmax des
Blocks, also passt der größte Wert per Konstruktion gerade hinein. Der
Schaden sitzt nicht oben, sondern unten: **7,5 % der Gradientenwerte im
Median werden zu null gerundet, im Maximum 59 %.** Fund 23 hatte die
Sättigung als stillen Fehler; hier ist es die Auslöschung.

## Warum die Eskalation trotzdem woanders ansetzt

Diese Zahlen legen nahe, an den Gradienten zu arbeiten. Die Messung sagt
etwas anderes. Jede Quantisierung einzeln geschaltet, 200 Schritte,
lr 1e-5:

| Was quantisiert wird | zurückgehaltener Text | Trainingsverlust |
|---|---|---|
| **nichts** (Kontrolllauf durch denselben Codepfad) | 3,0472 → 2,9795 | 2,5191 → 2,9758 |
| nur **Gradienten** int8 je Block | 3,0472 → **2,9795** | 2,5191 → 2,9759 |
| nur **Aktivierungen** int16 je Kanal | 3,0473 → **2,9795** | 2,5191 → 2,9758 |
| nur **Gewichte** int8 je Kanal | 3,0689 → **4,2010** | 2,5349 → 0,2846 |
| Gewichte + Aktivierungen | 3,0689 → 4,2909 | 2,5350 → 0,3491 |

**Der Rückwärtspass ist unschuldig.** Die Gradientenquantisierung
liefert auf vier Nachkommastellen dasselbe Ergebnis wie der
Gleitkommalauf, trotz 26,6 Bits Spanne und 7,5 % Auslöschung. Die
Sorge aus Fund 20/24, übertragen auf die Gradienten, hat sich **nicht**
bestätigt.

Der **Kontrolllauf** ist dabei die wichtigste Zeile: Er läuft durch
dieselbe eigene autograd-Funktion, nur ohne Raster, und trifft die
Referenz exakt. Ohne ihn ließe sich nicht ausschließen, dass gemessen
wurde, was der Ersatz für `nn.Linear` anders macht.

## Zwei Eskalationskandidaten gemessen ausgeschlossen

**Breitere Gradienten-Wortbreite: hilft nicht.** Derselbe Lauf mit
int16-Gradienten statt int8 endet auf der Haltemenge bei **4,17** statt
3,91, also nicht besser. Wenn acht zusätzliche Bits nichts ändern, ist
die Wortbreite nicht die Grenze.

**Bewegliche Gewichtsskalen: nicht der Mechanismus.** Der Verdacht war,
dass ein über eine Zweierpotenzgrenze wachsendes Gewicht die Skala der
ganzen Zeile halbiert und alle ihre Gewichte auf einmal springen lässt.
Mit eingefrorenen Skalen endet der Lauf bei **4,2035** statt 4,2010,
also unverändert.

## Was die Gewichtsquantisierung tatsächlich tut

Nach 200 Schritten, Arm „nur Gewichte int8", lr 1e-5:

| Schicht | Rasterstufe | Bewegung über 200 Schritte | in Stufen | int8 geändert |
|---|---|---|---|---|
| `layers.0.mlp.down_proj` | 1,07e-3 | 3,00e-7 | 0,028 % | 4,9 % |
| `layers.12.self_attn.q_proj` | 9,24e-4 | 1,81e-7 | 0,020 % | 5,8 % |
| `layers.23.mlp.up_proj` | 9,30e-4 | 2,51e-7 | 0,027 % | 6,0 % |

> **Korrektur (2026-08-22).** Die Spalte nannte ursprünglich „mittlere
> Bewegung" und wurde als Größe **eines** Schrittes gelesen, auch von mir
> selbst in Fahrplan und README. Sie ist die Bewegung über **200**
> Schritte. Je Schritt sind es rund 6,4e-6 einer Rasterstufe im Median,
> gemessen in `bitbudget.py`. Die Aussage ändert sich dadurch nicht, sie
> wird stärker: Der Schritt ist noch kleiner als angenommen.

Ein SGD-Schritt bewegt ein Gewicht um wenige Millionstel einer
Rasterstufe. Der Körper des Modells kann sich also nicht gerichtet
bewegen; geändert werden nur die Werte, die ohnehin an einer
Rundungsgrenze standen, und das ist Rauschen, kein Lernen. Was als
fallender Trainingsverlust erscheint, entsteht in den **nicht**
quantisierten Teilen, vor allem in den wenigen Einbettungszeilen der
tatsächlich vorkommenden Token, die bei 0,5B über das Weight-Tying
zugleich der LM-Kopf sind.

## Gegenprobe über die Lernrate

Der Einwand liegt nahe, die Schrittweite sei einfach zu klein gewählt.
120 Schritte je Zelle:

| lr | Gleitkomma | Gewichte int8 | volles Schema |
|---|---|---|---|
| 1e-5 | **2,9795** | 4,2037 | 3,9080 |
| 1e-4 | **2,9488** | 3,5892 | 3,3743 |
| 1e-3 | 3,6124 | 3,8211 | 3,9494 |

Bei 1e-4, wo die Referenz ihr bestes Ergebnis erreicht, liegen die
quantisierten Arme **+14 % bis +22 %** darüber. Bei 1e-3 überanpasst
auch der Gleitkommalauf; dort ist nichts mehr zu vergleichen. Der
Abstand ist damit kein Artefakt einer schlecht gewählten Lernrate.

## Die Abhilfe, gemessen

Ein SGD-Schritt bewegt ein Gewicht im Median um **6,4e-6 einer
Rasterstufe**. Mit Round-to-Nearest passiert dann **entweder nichts oder
ein ganzer Sprung**, also eine Überschreitung um das Hunderttausendfache. Genau dieses Bild
beschreibt Gupta et al. 2015 („Deep Learning with Limited Numerical
Precision"): 16-Bit-Festkomma scheitert mit Rundung zur nächsten Stufe
und funktioniert mit stochastischem Runden.

Stochastisch runden heißt: aufrunden mit einer Wahrscheinlichkeit gleich
dem Nachkommaanteil. Der Erwartungswert ist dann der Gleitkommawert, die
Aktualisierung ist unverzerrt, und eine winzige Änderung des Masters
ändert die erwartete Ausgabe sofort.

200 Schritte, lr 1e-5, alles andere unverändert:

| Variante | zurückgehaltener Text |
|---|---|
| Gleitkomma, SGD | 3,0472 → **2,9795** |
| nur Gewichte int8, Rundung zur nächsten Stufe | 3,0689 → 4,2836 |
| nur Gewichte int8, **stochastisch** | 3,0822 → **3,0019** |
| volles Schema, Rundung zur nächsten Stufe | 3,0689 → 3,8627 |
| volles Schema, **stochastisch** | 3,0770 → **2,9994** |

Eine einzige geänderte Zeile im Code dreht das Ergebnis.

**AdamW wurde ebenfalls geprüft und sagt hier nichts aus.** Bei lr 1e-5
überanpasst schon der Gleitkommalauf (3,05 → 4,11); die Lernrate ist für
Adam auf diesem winzigen Korpus um Größenordnungen zu groß. Die
Adam-Zeilen messen meine Lernratenwahl, nicht die Quantisierung, und
gehören deshalb nicht in die Auswertung.

## Und der Zufall kostet keinen Determinismus

Für ein Netz, dessen Konsens auf Bitgleichheit beruht, wäre echter Zufall
im Rechenpfad ausgeschlossen. Zufall aus einem **Keim** ist aber kein
Zufall im Sinne der Nachrechenbarkeit, sondern eine Funktion des Keims.

| | Keim 11, erster Lauf | Keim 11, zweiter Lauf | Keim 12 |
|---|---|---|---|
| globaler Generator | 2,644600033760 | 2,644600033760 | 2,645622849464 |
| eigener Generator | 2,647869527340 | 2,647869527340 | 2,629373848438 |

Identisch bis auf die letzte Stelle, und bei anderem Keim anders.

**Ein erster Versuch schlug fehl** (3,0466 gegen 3,0463) und sah nach
einem Argument gegen stochastisches Runden aus. Er war eines gegen meine
Keimsetzung: `torch.manual_seed` setzt in dieser PyTorch-Fassung den
MPS-Generator nicht mit. Die Gegenprobe steht daneben: **Ohne jeden
Zufall ist der Lauf auf beiden Geräten reproduzierbar**, auf MPS wie auf
CPU, bei zwölf Nachkommastellen identisch. Bemerkenswert am Rande:
MPS 2,607767760754 gegen CPU 2,614294707775, also **verschiedene Geräte,
verschiedene Ergebnisse**, und das ist genau der Grund, aus dem dieses
Projekt Ganzzahlen will.

Für eine spätere Umsetzung folgt daraus: Der Würfel darf kein
RNG-Zustand sein, sondern eine Funktion aus (Ebene, Schritt, Index) über
`kernels/src/prng.rs::splitmix64`. Zählerbasiert, ohne Zustand, auf jeder
Maschine gleich, und damit Teil der nachrechenbaren Spezifikation statt
eine Quelle von Abweichung.

## Konsequenz für den Fahrplan

**Punkt 0.1 ist beantwortet, und die Antwort ist ein Ja mit einer
Bedingung.** Die Annahme aus Whitepaper Kap. 7 trägt: Der Rückwärtspass
überträgt unverändert. Sie trägt nur nicht die stillschweigende
Nebenannahme, man dürfe die Gewichte im Trainingsschritt so runden wie
in der Inferenz.

**Was daraus folgt, in dieser Reihenfolge:**

1. **Stochastisches Runden gehört in θ_v**, nicht in eine
   Trainingsbibliothek. Es ist Teil der Spezifikation, weil es das
   Ergebnis bestimmt, und es muss zählerbasiert sein, damit es
   nachrechenbar bleibt.
2. **Der Gleitkomma-Master ist die nächste offene Frage.** Diese
   Simulation hält die Gewichte in float32 und quantisiert nur für den
   Vorwärtspfad. Für Myelith ist das zu wenig: Ein Trainingsschritt, der
   im Konsens überprüft werden soll, darf keinen Gleitkommazustand haben.
   Der naheliegende Weg ist ein **ganzzahliger Master** mit mehr Bits
   (etwa int32 mit derselben Zweierpotenz-Skala), auf dem die
   Aktualisierung eine exakte Ganzzahladdition ist und aus dem der int8
   für den Vorwärtspfad stochastisch gerundet wird. Das ist zu messen,
   nicht anzunehmen.
3. **Skala je Kanal statt je Block** für die Gradienten: nach dieser
   Messung nicht nötig. Die Gradientenquantisierung trifft die Referenz
   auch mit Blockskalen.
4. **Breitere Gradienten-Wortbreite:** gemessen ausgeschlossen.
5. **Blockgrenzen abseits der Shard-Grenzen:** unberührt, weil die
   Gradienten nicht die Ursache waren.

## Was diese Messung nicht sagt

- **Nichts über größere Modelle.** 0,5B, wie beim
  Inferenz-Entscheidungspunkt. Die Skalierungsfrage bleibt offen (K6).
- **Nichts über einen richtigen Trainingslauf.** Zwanzig Sequenzen und
  200 Schritte messen, ob das Verfahren die Referenz reproduziert, nicht
  ob ein ausgewachsener Lauf Gleitkommaqualität erreicht. Bei lr 1e-3
  überanpasst auch die Referenz.
- **Nichts über Bitgleichheit.** Gemessen wurde Qualität. Determinismus
  folgt aus der Ganzzahligkeit und ist eine Eigenschaft der späteren
  Umsetzung.
- **Die Referenz lief in float32, nicht in BF16.** Bewusst: Ein
  bf16-Lauf brächte seine eigene Rundung mit, und die wäre ein zweiter
  Einflussfaktor neben dem, der untersucht werden soll.

## Reproduzieren

```bash
cd INTEGER_LLM/calibrate
S=../../TRAINING/tests/diag/backward_reference_simulation.py

# Das Ergebnis: volles Schema mit stochastischem Runden
.venv/bin/python $S --schritte 200 --sequenzen 20 --halte 8 --lr 1e-5 --stochastisch

# Die Gegenprobe: dasselbe mit Rundung zur naechsten Stufe
.venv/bin/python $S --schritte 200 --sequenzen 20 --halte 8 --lr 1e-5 --auch-int16

# Die Ablation, Zelle für Zelle
for T in "" g a w wa wag; do
  .venv/bin/python $S --schritte 200 --sequenzen 20 --halte 8 --lr 1e-5 --teile "$T"
done

# Die Lernraten-Gegenprobe
for L in 1e-5 1e-4 1e-3; do
  .venv/bin/python $S --schritte 120 --sequenzen 20 --halte 8 --lr $L --teile w
done
```

Jeder Lauf schreibt seine eigene Ergebnisdatei; der Dateiname trägt
Teile, Wortbreite, Schrittzahl und Lernrate. Laufzeit auf Apple M-Serie
(MPS): rund 30 s je Gleitkomma-Arm, rund 100 s je quantisiertem Arm.
