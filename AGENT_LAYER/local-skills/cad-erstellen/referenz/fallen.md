# Fallen beim Bauen mit `myl_cad`

- **Maße im Auftrag gelten außen und für das ganze Teil.** Ein L-Profil
  „60 mm lang, stehender Schenkel 5 mm dick an der Außenkante bei x = 0“
  liegt von x = 0 bis 60; der stehende Schenkel liegt innerhalb (x von 0
  bis 5). Abstände „von der Außenseite“ zählen von dieser Außenseite.
- **Der Ursprung ist die Ecke, nicht die Mitte.**
- **`bei` eines Zylinders ist sein Anfang**, die Mitte der Grundfläche.
- **Nicht selbst speichern:** kein `doc.save()`; `cad_bauen` schreibt
  `.FCStd` und `.step` (eine solche Zeile wird übergangen).
- **Im Ausdruck-Text stehen nur Parameter**, keine Python-Namen: nach
  `h = "lochabstand / 2"` wirkt `"breite / 2 - h"` nicht. Ausschreiben
  oder einsetzen: `f"breite / 2 - ({h})"`.
- **Ein negativer Wert** steht ganz im Text: `"-lochabstand / 2"`.
- **Einheiten:** Bei Längen und Lagen hängt die Bibliothek `mm` an
  Summanden selbst an (`"hoehe - 2"`). In Winkeln nicht: `"winkel - 10 deg"`.
  Teiler und Faktoren bleiben ohne.
- **Eine Zahl statt eines Ausdrucks** baut richtig, ist aber in FreeCAD
  nicht mehr an den Parameter gebunden.
- **Eine Anzahl** steht als Text ohne Einheit: `anzahl="6"`.
- **Erst hinzufügen, dann herausschneiden.**
- **Verrunden und Fase zuletzt.** `oben`, `unten`, `alle` brauchen einen
  Radius kleiner als die halbe Dicke; `senkrecht` nur einen kleiner als die
  halbe kürzere Seite und als der Abstand zur nächsten Bohrung
  (Eckenradius 4 an einer 6 mm dicken Platte geht).
- **Für `kreis`** liegt der erste Schritt neben der Achse, etwa
  `bei=("teilkreis / 2", 0, 0)`.
- **Eine Reihe lässt sich nicht noch einmal wiederholen**: vier Löcher in
  den Ecken sind vier `bohrung`-Aufrufe.
