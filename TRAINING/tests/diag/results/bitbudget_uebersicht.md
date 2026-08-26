# Bitbudget je Modellgröße und Lernrate (Punkt 1.1)

**Datum:** 2026-08-23
**Erzeugt mit:** `tests/diag/bitbudget.py --lr <wert>`, je Lauf rund 4 s
**Gemessen an:** Qwen2.5-0,5B, 1 216 512 Gewichtszeilen, 4 Rückwärtsschritte

---

## Das Ergebnis

| Lernrate | Schritt / Rasterstufe | 1. Perzentil | F Median | F p01 | **F empfohlen** | W_master | Wort |
|---|---|---|---|---|---|---|---|
| 1e-3 | 6,410e-04 | 5,742e-05 | 11 | 15 | **19** | 27 | **int32** |
| 1e-4 | 6,410e-05 | 5,742e-06 | 14 | 18 | **22** | 30 | **int32** |
| 1e-5 | 6,410e-06 | 5,742e-07 | 18 | 21 | **25** | 33 | int64 |
| 1e-6 | 6,410e-07 | 5,742e-08 | 21 | 25 | **29** | 37 | int64 |

`F empfohlen` ist das erste Perzentil plus vier Bit Reserve. `W_master`
ist `8 + F`, also der int8-Bereich plus die Nachkommabits.

**Der Akkumulator der Aggregation ist in allen vier Fällen int64.** Bei
10 000 Beiträgen über 1000 Schritte reicht int32 selbst bei der
großzügigsten Lernrate nicht: Der Sicherheitsabstand liegt dort bei
Faktor 1, also an der Grenze.

## Was der einzelne Messpunkt verdeckt hat

Bis zum 2026-08-23 lag genau eine Messung vor, bei lr = 1e-5, und ihre
Empfehlung lautete „Master nach int64". Das stimmt für diese Lernrate und
**nur** für sie.

**Die Grenze zwischen int32 und int64 liegt zwischen 1e-4 und 1e-5.** Wer
mit 1e-4 oder gröber trainiert, kommt mit einem 32-Bit-Master aus; das
halbiert den Speicher des Masters, und der Master ist die größte
Datenstruktur des Trainingsschritts.

Das ist keine akademische Unterscheidung: Ob der Master in 32 oder 64 Bit
geführt wird, ist eine konsensrelevante Festlegung, weil beide Knoten
denselben Wert ausrechnen müssen.

## Warum sich das auch herleiten lässt, und warum trotzdem gemessen wurde

`Schritt / Rasterstufe` ist proportional zur Lernrate: Der Zähler ist
`lr · |grad|`, der Nenner hängt nicht von der Lernrate ab. Ein Faktor 10
kostet also genau `log2(10) = 3,32` Bit.

Nachgeprüft an den vier Messungen:

| Übergang | F wächst um | erwartet |
|---|---|---|
| 1e-3 → 1e-4 | 3 Bit | 3,32 |
| 1e-4 → 1e-5 | 3 Bit | 3,32 |
| 1e-5 → 1e-6 | 4 Bit | 3,32 |

Drei, drei, vier: Das ist die Rundung auf ganze Bits, im Mittel 3,33. Die
Herleitung trägt.

**Gemessen wurde trotzdem**, und zwar aus einem Grund, den dieses Projekt
schon mehrfach bezahlt hat: Eine Proportionalität, die man nur annimmt,
kann an einer Stelle brechen, die man nicht bedacht hat. Hier tut sie es
nicht, und das steht jetzt fest statt zu gelten.

## Was nicht gemessen ist: die Modellgröße

Der Punkt verlangt F **je Modellgröße und Lernrate**. Die
Lernraten-Achse ist vollständig, die Modellgrößen-Achse hat einen
einzigen Punkt.

**Grund, gerechnet statt geschätzt:** `bitbudget.py` lädt das
Referenzmodell in float32. Qwen2.5-7B hat rund 7,6 Milliarden Parameter,
das sind **30 GB allein für die Gewichte**, dazu noch einmal so viel für
die Gradienten. Diese Maschine hat **24 GB**. Der Lauf würde nicht
langsam, er würde gar nicht laufen.

**Was es bräuchte:** eine Maschine mit mindestens 64 GB, oder eine
Fassung des Skripts, die schichtweise misst statt das ganze Modell zu
halten. Letzteres ist machbar, denn gebraucht wird je Schicht nur
`absmax` der Gewichte und der Betrag des Gradienten; beides fällt beim
Rückwärtspass ohnehin an.

**Was sich vermuten, aber nicht behaupten lässt:** Größere Modelle haben
typischerweise kleinere Gradienten je Gewicht, was `F` nach oben triebe.
Ob das die Rasterstufe ausgleicht, die vom Betragsmaximum je Zeile
abhängt, ist offen. **Zwei Punkte wären hier eine Kurve, einer ist
keiner**, und dieselbe Vorsicht gilt hier wie bei der
Perplexitätsskalierung (K6).

## Empfehlung

Für die Referenzimplementierung bei **lr = 1e-5**: `F = 25`, Master in
**int64**, Aggregation in **int64**.

Wird die Lernrate später als Governance-Parameter geführt, gehört `F`
mitgeführt, denn beide hängen aneinander. Eine Änderung der Lernrate ohne
Anpassung von `F` bricht entweder das Training (zu wenige Bits) oder
verschwendet Speicher (zu viele).
