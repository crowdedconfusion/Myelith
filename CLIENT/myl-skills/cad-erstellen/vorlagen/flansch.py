# Ein Flansch mit Lochkreis: Scheibe, Mittelloch, sechs Schraubenloecher,
# oben eine Fase. Jedes Mass ist ein Parameter und in FreeCAD aenderbar.
from myl_cad import *

doc = dokument("Flansch")
parameter(doc, aussen=80, dicke=8, mittelloch=30, teilkreis=60, schraube=6.6, anzahl="6")
k = koerper(doc, "Flansch")
zylinder(k, "Scheibe", "aussen / 2", "dicke")
bohrung(k, "Mitte", "mittelloch / 2", "dicke")
loch = bohrung(k, "Schraubenloch", "schraube / 2", "dicke", bei=("teilkreis / 2", 0, 0))
kreis(k, "Lochkreis", loch, "anzahl")
fase(k, "Fase", 1, kanten="oben")
