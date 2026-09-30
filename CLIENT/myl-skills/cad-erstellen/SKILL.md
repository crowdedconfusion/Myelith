---
beschreibung: Ein parametrisches 3D-Teil bauen oder ändern, das sich in FreeCAD weiter bearbeiten lässt.
stichworte: cad, 3d, freecad, fcstd, step, parametrisch, bauteil, konstruieren, modell, halter, platte, flansch, bohrung, 3d-druck, parametric, part, design, bracket, hole
---

# Ein parametrisches 3D-Teil erstellen

## Wann

Wenn der Nutzer ein Bauteil als 3D-Datei möchte oder eines ändern will.
Braucht die Werkzeugkiste `CAD` (`cad_bauen`, `cad_pruefen`,
`cad_parameter`) und FreeCAD auf der Maschine.

## Vorgehen

1. **Maße klären.** Jedes Maß, das der Nutzer nennt oder später ändern
   könnte, wird ein Parameter mit sprechendem Namen (`breite`, `dicke`,
   `bohrung`). Fehlt ein Maß, nimm einen üblichen Wert und sag es.
2. **Skript schreiben** mit `write_file`, zum Beispiel `halter.py`:

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

   Maße und Lagen sind Zahlen (Millimeter) oder **Ausdrücke als Text** mit
   Parameternamen. Nur ein Ausdruck bleibt in FreeCAD änderbar.
3. **Bauen:** `cad_bauen` mit `skript` und `name`. Es schreibt
   `<name>.FCStd` und `<name>.step` und meldet Abmessungen und Volumen.
4. **Prüfen:** Stimmen die gemeldeten Abmessungen mit dem Auftrag? Ist
   das Volumen plausibel (kleiner als der umschließende Quader)? Bei
   `FEHLER` das Skript mit `edit_file` berichtigen und neu bauen.
5. **Ändern:** Ein Maß ändert `cad_parameter` (`werte`: `breite=80 mm;
   bohrung=6`). Eine neue Form (weitere Bohrung, Ausschnitt) kommt ins
   Skript, dann neu bauen.
6. **Melden:** die beiden Dateien, die Parameter und die Abmessungen.

## Bausteine

| Aufruf | wirkt |
|---|---|
| `quader(k, name, laenge, breite, hoehe, bei=(x, y, z))` | fügt hinzu; `bei` ist die Ecke unten links vorn |
| `zylinder(k, name, radius, hoehe, bei=…, achse="z")` | fügt hinzu; `bei` ist die Mitte der Grundfläche |
| `ausschnitt(k, name, laenge, breite, hoehe, bei=…)` | schneidet einen Quader heraus |
| `bohrung(k, name, radius, tiefe, bei=…, achse="z")` | schneidet einen Zylinder heraus |
| `verrunden(k, name, radius, kanten="senkrecht")` | Kanten: `senkrecht`, `oben`, `unten`, `alle` |
| `fase(k, name, groesse, kanten="oben")` | wie `verrunden` |
| `reihe(k, name, schritt, anzahl, laenge, richtung="x")` | wiederholt einen Schritt in einer Reihe |
| `kreis(k, name, schritt, anzahl, achse="z")` | wiederholt einen Schritt im Kreis um den Ursprung |

Ein größeres Beispiel: `learn_skill` mit `cad-erstellen/vorlagen/flansch.py`.

## Fallen

- **Eine Zahl statt eines Ausdrucks** baut richtig, ist aber in FreeCAD
  nicht mehr an den Parameter gebunden.
- **Eine Anzahl** steht als Text ohne Einheit: `anzahl="6"`.
- **Ein Ausschnitt oder eine Bohrung ohne Material** darunter ist ein
  Fehler; erst hinzufügen, dann herausschneiden.
- `verrunden` und `fase` zuletzt, und mit einem Radius kleiner als die
  halbe Dicke, sonst lässt sich die Form nicht rechnen.
- Für `kreis` liegt der erste Schritt **neben** der Achse, etwa
  `bei=("teilkreis / 2", 0, 0)`.
- **Eine Reihe lässt sich nicht noch einmal wiederholen** (`reihe` auf eine
  Reihe scheitert in FreeCAD). Vier Löcher in den Ecken sind vier
  `bohrung`-Aufrufe mit `bei=("rand", "rand", 0)`,
  `("laenge - rand", "rand", 0)` und so weiter.
