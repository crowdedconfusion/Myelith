---
beschreibung: Richtige Maße für Schraubenlöcher, Gewinde und Senkungen, und was beim 3D-Druck dazukommt.
stichworte: schraube, schrauben, bohrung, bohrungen, loch, löcher, durchgangsbohrung, kernloch, gewinde, senkung, senkkopf, zylinderkopf, M3, M4, M5, M6, M8, M10, M12, 3d-druck, toleranz, screw, bolt, hole, clearance hole, tap drill, countersink, counterbore
---

# Schrauben und Bohrungen

## Wann

Wenn ein Bauteil Löcher für Schrauben, ein Gewinde oder eine Senkung
braucht, oder wenn nach dem richtigen Maß gefragt wird. Mit
`cad-erstellen` zusammen: Erst hier das Maß, dann dort bauen.

## Vorgehen

1. **Schraubengröße feststellen** (M3 bis M12). Fehlt sie, frag nach oder
   nimm die übliche für die Aufgabe und sag es.
2. **Art des Lochs wählen:** Soll die Schraube hindurchgehen
   (Durchgangsbohrung), soll ein Gewinde hinein (Kernloch), oder soll der
   Kopf versenkt werden?
3. **Maß aus der Tabelle nehmen.** Ohne andere Angabe die Reihe „mittel“.
4. **Im CAD-Skript** das Maß als Parameter anlegen, zum Beispiel
   `parameter(doc, schraubenloch=5.5)`, und `bohrung(k, …, "schraubenloch / 2", …)`.
5. **Melden**, welches Maß warum gewählt wurde (etwa „5,5 mm, ISO 273
   mittel für M5“).

## Durchgangsbohrungen nach ISO 273 (mm)

| Gewinde | fein | **mittel** | grob |
|---|---|---|---|
| M3 | 3,2 | **3,4** | 3,6 |
| M4 | 4,3 | **4,5** | 4,8 |
| M5 | 5,3 | **5,5** | 5,8 |
| M6 | 6,4 | **6,6** | 7,0 |
| M8 | 8,4 | **9,0** | 10,0 |
| M10 | 10,5 | **11,0** | 12,0 |
| M12 | 13,0 | **13,5** | 14,5 |

## Kernloch für ein metrisches Regelgewinde

Kernloch = Nenndurchmesser minus Steigung: M3 2,5 · M4 3,3 · M5 4,2 ·
M6 5,0 · M8 6,8 · M10 8,5 · M12 10,2.

## Köpfe versenken

- **Zylinderkopf (ISO 4762):** Kopfdurchmesser M3 5,5 · M4 7 · M5 8,5 ·
  M6 10 · M8 13 · M10 16 · M12 18, Kopfhöhe gleich dem Nenndurchmesser.
  Senkung: etwa 1 mm größer als der Kopf, etwas tiefer als er hoch ist. Im
  CAD zwei Bohrungen übereinander: die Durchgangsbohrung ganz durch, die
  Senkung nur so tief wie nötig.
- **Senkkopf (ISO 10642):** Kegel mit 90°, Kopfdurchmesser höchstens
  2,24 mal der Nenndurchmesser (M5: 11,2). Die Bausteine von `cad-erstellen`
  können keinen Kegel; sag das und nimm eine Senkung für Zylinderkopf oder
  die FreeCAD-Schnittstelle.

## Beim 3D-Druck (Erfahrungswerte, keine Norm)

- Gedruckte Löcher werden etwas kleiner: 0,2 bis 0,3 mm auf die
  Durchgangsbohrung aufschlagen.
- Wände mindestens 1,2 mm, um eine Schraube herum mindestens 2 mm.
- Ein Gewinde direkt ins Plastik hält schlecht; besser eine Mutter
  einlegen oder eine Gewindebuchse einschmelzen.

## Fallen

- „M5-Loch“ heißt fast immer Durchgangsbohrung (5,5 mm), nicht 5,0 mm: Mit
  5,0 geht die Schraube nicht hindurch.
- Ein Kernloch ist kleiner als die Schraube; es ist nur für ein Gewinde.
