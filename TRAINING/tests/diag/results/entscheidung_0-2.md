# Entscheidung 0.2: Ein Trainingsschritt ohne Gleitkommazustand

**Datum:** 2026-08-22 · **Modell:** Qwen2.5-0,5B · **Korpus:** WikiText-2
**Skript:** `TRAINING/tests/diag/integer_master_simulation.py`

## Ergebnis in einem Satz

**Es geht.** Ein ganzzahliger Master mit 16 zusätzlichen Nachkommabits,
aktualisiert durch eine exakte Ganzzahladdition, mit stochastischem
Runden aus einem zählerbasierten Würfel: **+0,75 %** gegen die
Gleitkomma-Referenz, Master bleibt nachweislich ganzzahlig, Ergebnis bei
gleichem Keim bitgleich.

## Warum dieser Punkt

Punkt 0.1 hat gezeigt, dass das Schema trägt, sofern die Gewichte
stochastisch gerundet werden. Die Simulation hielt die Gewichte dabei
aber in **float32** und quantisierte nur für den Vorwärtspfad. Für
Myelith ist das zu wenig: Ein Trainingsschritt, den das Netz im Konsens
nachrechnen soll, darf keinen Gleitkommazustand haben. Zwei Knoten mit
demselben Gradienten müssen denselben neuen Gewichtsstand bekommen, und
zwar bitgleich.

## Das Verfahren

```
Master         m, ganze Zahl, |m| ≤ 127 · 2^16
Vorwärtspfad   w8 = stochastisch_runden(m / 2^16), auf [-128, 127]
Aktualisierung m ← m − round(lr · dL/dW · 2^shift · 2^16)
```

**Warum 16 Zusatzbits, gerechnet statt geraten.** Gemessen (0.1): Ein
SGD-Schritt beträgt 0,03 % einer int8-Rasterstufe.

| Zusatzbits | ein Schritt in LSB | Master bis | in float32 exakt |
|---|---|---|---|
| 8 | 0,08 | 32 512 | ja |
| 12 | 1,23 | 520 192 | ja |
| **16** | **19,66** | **8 323 072** | **ja** |
| 20 | 314,57 | 133 169 152 | nein |

Unter 12 Bits wäre ein Schritt kleiner als ein LSB des Masters und
verschwände erneut. Ab 20 Bits überschreitet der Master die 2^24, die
float32 als ganze Zahlen exakt hält, und die Simulation würde selbst zur
Fehlerquelle. Sechzehn liegt in der Mitte und mit Reserve.

**Die Skala wird eingefroren**, damit der Master seine Bedeutung behält.
In 0.1 gemessen: Eingefrorene Skalen ändern am Ergebnis nichts.

## Das Ergebnis

200 Schritte, lr 1e-5, 20 Sequenzen, 8 zurückgehalten:

| Variante | zurückgehaltener Text | Abstand | Master ganzzahlig |
|---|---|---|---|
| Gleitkomma-Referenz | 3,0472 → **2,9795** | | |
| Ganzzahl-Master, Rundung zur nächsten Stufe | 3,0689 → 4,0654 | +36,5 % | ja |
| Ganzzahl-Master, **stochastisch** | 3,0719 → **3,0019** | **+0,75 %** | ja |

Kriterium ≤ 10 %. **Trägt.**

Zum Vergleich der Gleitkomma-Master aus 0.1: +0,67 %. **Der ganzzahlige
Master kostet also nichts an Qualität**, er nimmt nur den
Gleitkommazustand heraus.

## Fehlerrückkopplung gibt es dabei umsonst

Die 16 Bits unterhalb der int8-Stufe **sind** der Quantisierungsrest. Er
wird nicht verworfen und nicht getrennt geführt, er steht im Master und
wirkt beim nächsten Schritt weiter. Was in der Literatur als eigener
Mechanismus auftritt (Fehlerrückkopplung, Seide et al. 2014; Karimireddy
et al. 2019), fällt hier als Nebenwirkung der Wortbreite an.

## Der Würfel: eine Funktion, kein Zustand

**Der PyTorch-Zufall auf MPS ist bei gleichem Keim nicht
reproduzierbar.** Gemessen: zwei frische Prozesse, `torch.manual_seed`
und `torch.mps.manual_seed` gesetzt, verschiedene Ergebnisse. Für ein
Netz mit Bitgleichheits-Konsens ist das unbrauchbar.

Ersetzt durch einen **zählerbasierten** Würfel: `splitmix64` über
(Ebene, Schritt, Index, Keim), also dieselbe Funktion, die als
`kernels/src/prng.rs::splitmix64` bereits im Projekt steht. Kein Zustand,
keine Reihenfolgeabhängigkeit, kein Geräteeinfluss.

| Probe | Ergebnis |
|---|---|
| Mittelwert über 100 000 Werte | 0,499987 |
| Spanne | 0,000020 bis 0,999999 |
| zweiter Aufruf | identisch |
| anderer Schritt, andere Ebene | verschieden |
| **auf MPS statt CPU gerechnet** | **identisch** |
| **zweiter Prozess** | **identisch** |

Kosten: 1,2 ms je 4 Mio. Werte auf MPS gegen 0,2 ms für `torch.rand_like`,
also rund 20 Sekunden über den ganzen Lauf.

## Reproduzierbarkeit des ganzen Schritts

Auf der CPU, 6 Schritte, zwei Durchgänge:

| | Hash des Masters |
|---|---|
| Keim 1, erster Lauf | `e9b2ff22e28209f6` |
| Keim 1, zweiter Lauf | `e9b2ff22e28209f6` |
| Keim 2 | `80b8e0e5413b6d5c` |

Bitgleich bei gleichem Keim, verschieden bei anderem. Der Master ist in
allen Läufen nachweislich ganzzahlig.

**Auf MPS ist das nicht prüfbar, und der Grund liegt nicht am
Verfahren.** Dort weicht schon ein Lauf **ohne jeden Zufall** zwischen
zwei Durchgängen ab (2,463654 gegen 2,465496), sobald im selben Prozess
vorher Modelle gebaut und freigegeben wurden. Das ist eine Eigenschaft
der Plattform. Die Reproduzierbarkeitsprobe im Skript läuft deshalb auf
der CPU, und der Grund steht im Quelltext daneben.

Nebenbefund am Rande, aber grundsätzlich: MPS und CPU liefern für
denselben Gleitkommalauf **verschiedene** Ergebnisse (2,607767760754
gegen 2,614294707775). Genau deshalb will dieses Projekt Ganzzahlen.

## Was noch fehlt

Diese Simulation ist ein Zwischenstand, kein ganzzahliges Training.

- **Der Vorwärts- und Rückwärtspass rechnen weiter in Gleitkomma.**
  Quantisiert sind Gewichte, Aktivierungen und Gradienten als Raster,
  aber die Matrixmultiplikationen laufen float. Deshalb ist der
  **Gradient** noch geräteabhängig, und damit auch der Master, den er
  bewegt. Erst ein durchgehend ganzzahliger Pfad macht den Schritt
  zwischen Maschinen bitgleich; dafür gibt es INTEGER_LLM.
- **Einbettung, Normierungen und LM-Kopf bleiben Gleitkomma.** Bei 0,5B
  ist die Einbettung wegen Weight-Tying zugleich der LM-Kopf, also
  136 Mio. Parameter außerhalb des Ganzzahlpfads. Wie sie zu behandeln
  sind, ist offen.
- **Zwanzig Sequenzen, 200 Schritte.** Gemessen ist, dass das Verfahren
  die Referenz reproduziert, nicht dass ein ausgewachsener Lauf
  Gleitkommaqualität erreicht.
- **Nur 0,5B.** Die Skalierungsfrage bleibt offen (K6).

## Konsequenz

**Für θ_v:** Stochastisches Runden gehört in die Spezifikation, nicht in
eine Trainingsbibliothek, denn es bestimmt das Ergebnis. Es muss
zählerbasiert sein, damit es nachrechenbar bleibt, und der Keim gehört zu
den Konsensdaten.

**Für den Fahrplan:** Der nächste Punkt ist nicht mehr die
Trainingsseite, sondern der ganzzahlige Vorwärts- und Rückwärtspass in
INTEGER_LLM. Solange die Matrixmultiplikation float ist, ist der
Trainingsschritt zwar exakt, aber sein Eingang nicht.

## Reproduzieren

```bash
cd INTEGER_LLM/calibrate
.venv/bin/python ../../TRAINING/tests/diag/integer_master_simulation.py \
    --schritte 200 --sequenzen 20 --halte 8 --lr 1e-5
```

Laufzeit auf Apple M-Serie: rund 30 s Referenz, 65 s der RTN-Arm, 175 s
der stochastische Arm, dazu die CPU-Probe.
