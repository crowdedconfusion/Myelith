# Rezepte für `myl_cad`

**Loch mit Senkung für eine Zylinderkopfschraube**, im Kreis wiederholt:

```text
loch = bohrung(k, "Loch", "d_loch / 2", "dicke", bei=("lochkreis / 2", 0, 0))
senk = bohrung(k, "Senkung", "d_senk / 2", "t_senk", bei=("lochkreis / 2", 0, "dicke - t_senk"))
kreis(k, "Lochkreis", [loch, senk], "anzahl")
```

**Die Innenkante eines Winkels** (Hohlkehle), der liegende Schenkel bis
`dicke` hoch, der stehende bei x = 0 bis `dicke` dick, beide innerhalb der
Länge: `verrunden(k, "Kehle", 5, durch=("dicke", "breite / 2", "dicke"))`,
ein Punkt auf der Kante.

**Die Oberkante eines Tellers, auf dem noch ein Bund steht:** `fase(k, "Fase",
1, kanten="oben", hoehe="dicke", aussen=True)`, am besten gleich nach dem
Teller.

**Vier Löcher im Quadrat mit Abstand `a` um die Mitte einer Platte:** vier
`bohrung`-Aufrufe bei `("laenge / 2 - a / 2", "breite / 2 - a / 2", 0)`,
`("laenge / 2 + a / 2", …)` und so weiter.

**Ein Kernloch von oben:** `bohrung(k, "Kern", "d / 2", "tiefe",
bei=(x, y, "hoehe"), achse="-z")`.

Ein größeres Beispiel: `cad-erstellen/vorlagen/flansch.py`.
