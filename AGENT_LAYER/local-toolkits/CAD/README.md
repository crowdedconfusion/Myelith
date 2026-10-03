# Werkzeugkiste: CAD

⚑ **Das Format einer Werkzeugdatei steht eine Ebene höher**, in
`../README.md`, für alle Kisten gemeinsam. Hier steht, was **in dieser
Kiste** liegt.

Parametrische 3D-Teile, gebaut mit FreeCAD ohne Fenster. Das Ergebnis ist
eine `.FCStd`-Datei mit Parametertabelle und Schrittbaum, die sich in
FreeCAD weiter bearbeiten lässt, und eine `.step`-Datei für andere
Programme.

## Was hier liegt

| Werkzeug | wofür |
|---|---|
| `cad_bauen` | baut aus einem Python-Skript ein Modell, schreibt `.FCStd` und `.step`, meldet Parameter, Bauteile, Abmessungen, Volumen und Fehler |
| `cad_pruefen` | liest eine `.FCStd` und meldet dasselbe, ohne sie zu ändern |
| `cad_parameter` | ändert Parameter einer `.FCStd`, rechnet neu, speichert und schreibt die `.step` neu |

Dazu drei Dateien, die keine Werkzeuge sind: `cad.sh` findet FreeCAD und
ruft es, `cad_lauf.py` läuft in FreeCAD und schreibt den Bericht,
`myl_cad.py` sind die Bausteine, die ein Skript benutzen darf.

⚑ **Diese Kiste erbt alles aus `Base`**, und `kiste.json` gibt ihr die
eingebauten Werkzeuge von `Advanced`.

## ⚑ Der Bericht misst, das Modell muss nicht rechnen

Jeder Schritt steht im Bericht mit seiner Lage und seiner **gemessenen**
Bilanz: „entfernt 54,475 von 54,475 mm3“, „fügt 1407,434 von 1407,434 mm3
hinzu“. Unter `WARNUNGEN` steht jeder Schnitt, der nicht ganz im Teil
liegt, gemessen gegen die Hülle aus allem, was bis dahin hinzugefügt wurde.
Ein Schnitt in schon entferntes Material (die Senkung über einer Bohrung)
ist keiner und wird nur vermerkt, ebenso eine durchgehende Bohrung, die
entlang ihrer Achse über das Teil hinausragt. Ragt ein Schnitt dagegen auf
einer Seite hinaus und endet auf der anderen im Material, steht er unter
`WARNUNGEN`, mit beiden Bereichen (der Lagerbock: eine Querbohrung ab der
Mitte des Blocks ging nur halb durch). Jede Warnung sagt, von wo bis wo das
Teil an der Stelle des Schnitts reicht.

📌 **Anlass (2026-10-01):** Das 35B legte die Bohrungen eines Flanschs um
(0, 0), als läge dort die Mitte der Platte. Drei von vier Löchern schnitten
nichts, und seine eigene Plausibilitätsrechnung im Kopf ergab „passt“.
Gebaut wird trotzdem, denn ein Loch am Rand kann gewollt sein; der Bericht
sagt nur, dass es so ist.

## Warum FreeCAD und nicht nur STEP

Eine STEP-Datei ist ein fertiger Körper: In FreeCAD lässt sich daran nichts
mehr an einem Maß ändern. Verlangt ist eine Datei, die man **nachbearbeiten**
kann. Deshalb baut das Werkzeug einen PartDesign-Körper, dessen Maße als
Ausdrücke an einer Tabelle `Parameter` hängen: Wer dort `breite` ändert,
ändert das Teil, und jeder Schritt steht im Baum.

## FreeCAD

FreeCAD ist ein fremdes Programm und liegt nicht im Repositorium.
`sh SYSTEM/install/cad-einrichten.sh` sieht nach, ob es da ist, und sagt
sonst, wie es auf die Maschine kommt. Gesucht wird `freecadcmd` im Pfad und
an den üblichen Orten; `MYL_FREECAD` nennt einen anderen. Ohne FreeCAD
meldet jedes der drei Werkzeuge genau das und sonst nichts.

## ⛔️ Sicherheit

`cad_bauen` führt das Skript aus, das das Modell geschrieben hat. Das ist
dieselbe Macht wie `run_command`; es gilt, was in `../README.md` unter
Sicherheit steht.

## Grenzen

- Die Bausteine decken Quader, Zylinder, Ausschnitt, Bohrung, Rundung,
  Fase, Reihe, Kreis und Raster; Reihe, Kreis und Raster wiederholen auch
  mehrere Schritte zusammen (Loch und Senkung). Ein Muster eines Musters
  geht in FreeCAD nicht; dafür ist das Raster da. Freie Profile (Skizzen) und Kegel gehen nur
  über die FreeCAD-Schnittstelle selbst.
- Rundung und Fase wählen ihre Kanten beim Bauen nach der Lage (`oben`,
  `unten`, `senkrecht`, `alle`, dazu `hoehe=` und `aussen=True`). Ändert ein
  Parameter später die Zahl der Kanten, wählt man sie in FreeCAD neu.
- Geprüft mit FreeCAD 1.1.1 auf macOS.
