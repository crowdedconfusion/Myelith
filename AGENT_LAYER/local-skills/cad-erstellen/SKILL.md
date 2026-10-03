---
beschreibung: Ein parametrisches 3D-Teil bauen oder ändern, das sich in FreeCAD weiter bearbeiten lässt.
stichworte: cad, 3d, freecad, fcstd, step, parametrisch, bauteil, konstruieren, modell, halter, platte, flansch, bohrung, 3d-druck, parametric, part, design, bracket, hole
---

# Ein parametrisches 3D-Teil erstellen

Braucht die Werkzeugkiste `CAD` (`cad_bauen`, `cad_pruefen`, `cad_parameter`).

## Vorgehen

1. **Maße als Parameter** mit sprechenden Namen. ⚠️ **Kommen Schrauben vor**
   (M3, „Loch für eine Schraube“, Gewinde, Senkung), lerne zuerst
   `schrauben-und-bohrungen`; Lochmaße nie aus dem Gedächtnis.
2. **Skript schreiben** mit `write_file` (`pfad`, `inhalt`):

   ```python
   from myl_cad import *

   doc = dokument("Halter")
   parameter(doc, breite=60, tiefe=40, dicke=6, bohrung=5, rand=10)
   k = koerper(doc, "Halter")
   quader(k, "Platte", "breite", "tiefe", "dicke")
   verrunden(k, "Ecken", 4, kanten="senkrecht")
   loch = bohrung(k, "Loch", "bohrung / 2", "dicke", bei=("rand", "rand", 0))
   reihe(k, "Lochreihe", loch, 2, "breite - 2 * rand", richtung="x")
   ```

   Maße sind Zahlen (mm) oder **Ausdrücke als Text** mit Parameternamen;
   nur ein Ausdruck bleibt in FreeCAD änderbar.
3. **Bauen:** `cad_bauen` mit `skript` und `name`. Nicht selbst speichern.
4. **Prüfen:** Abmessungen im Bericht **Zahl für Zahl** gegen den Auftrag.
   **Jede Zeile unter `WARNUNGEN` ist ein Fehler**, bis begründet; dann mit
   `edit_file` berichtigen und neu bauen. Das Volumen nicht im Kopf
   nachrechnen, die Bilanz je Schritt lesen.
5. **Ändern:** `cad_parameter` (`werte`: `breite=80 mm; bohrung=6`).
6. **Melden:** Dateien, Parameter, Abmessungen.

## Bausteine

| Aufruf | wirkt |
|---|---|
| `quader(k, name, l, b, h, bei=(x, y, z))` | fügt hinzu; `bei` ist die Ecke unten links vorn |
| `zylinder(k, name, r, h, bei=…, achse="z")` | fügt hinzu; `bei` ist die Mitte der Grundfläche |
| `ausschnitt(k, name, l, b, h, bei=…)` | schneidet einen Quader heraus |
| `bohrung(k, name, r, tiefe, bei=…, achse="z")` | schneidet einen Zylinder heraus; `achse="-z"` bohrt nach unten |
| `verrunden(k, name, r, kanten="senkrecht")` | `senkrecht`, `oben`, `unten`, `alle`; dazu `hoehe=`, `aussen=True`, `durch=(x, y, z)` |
| `fase(k, name, g, kanten="oben")` | wie `verrunden` |
| `reihe(k, name, schritt, anzahl, laenge, richtung="x")` | wiederholt Schritte (auch eine Liste) in einer Reihe |
| `kreis(k, name, schritt, anzahl, achse="z")` | wiederholt Schritte im Kreis um den Ursprung |
| `raster(k, name, schritt, nx, lx, ny, ly)` | Raster in x und y (2 × 2 Löcher); eine Reihe einer Reihe geht nicht |

## Die fünf Regeln

- **Ursprung ist die Ecke:** Die Mitte einer Platte ist
  `("laenge / 2", "breite / 2", 0)`.
- **`bei` eines Zylinders ist sein Anfang**, er läuft `tiefe` weit in
  +`achse`. Quer durch einen Block: am Rand beginnen, ganze Tiefe.
- **Maße gelten außen für das ganze Teil**, wenn nichts anderes gesagt ist.
- **Im Ausdruck-Text nur Parameter**, keine Python-Namen:
  `"breite / 2 - lochabstand / 2"`.
- **Verrunden und Fase zuletzt.**

Rezepte (Senkung im Kreis, Hohlkehle, Fase am Teller):
`learn_skill` mit `cad-erstellen/referenz/rezepte.md`. Alle Fallen:
`cad-erstellen/referenz/fallen.md`.
