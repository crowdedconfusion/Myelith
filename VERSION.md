0.12.0

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.12.0 (2026-11-19)

- **Bereit für Betreiber des Testnetzes.** `myl mining stimmsatzzeile`
  gibt die eigene Zeile für den Startsatz aus, mit genau dem Schlüssel, mit
  dem das Mining unterschreibt; im Fenster steht sie im Bereich Wallet,
  zusammen mit der Wahl des Netzes (`probe` oder `test-int4`). Die Anleitung
  steht im Repositorium in `NODE/README/README.md` unter „Betreiben“. Der
  Startsatz selbst kommt mit einem späteren Release.
- **Mining unterschreibt immer mit demselben Schlüssel**, gespeichert im
  Datenordner des Minings. Wer bisher ohne Wallet gemint hat, bekommt dabei
  einmal einen neuen Schlüssel; auf der Probekette ändert das nichts.
- **Der Knoten ist robuster:** Nach einem Neustart holt er erst auf und
  erzeugt dann; steht nach dem Start kein Netz um ihn, wartet er, statt sich
  zu beenden; Einstiegsadressen dürfen einen Namen tragen
  (`/dns4/<name>/...`); den Startsatz eines Netzes bringt das Release mit.
- **Das Netz bestraft keine Gegenstelle mehr** für eine Nachricht, die ein
  Knoten nur nicht beurteilen kann.

Aus 0.11.1 und 0.11.0, falls übersprungen:

- **Updates kommen nur mit der Signatur des Projekts** (Freigabeschlüssel
  `myelith-freigabe`).
- ⚠️ **Seit 0.11.0 gelten neue Regeln für die Abrechnung der Pods und für
  die Saat.** Eine Kette von 0.10.0 lässt sich nicht weiterführen; ein Knoten
  beginnt sie neu.
- Befehle des Agenten laufen in einer Sandbox des Betriebssystems; eine
  Runde kostet Prompt und Ausgabe; schneller auf Apple ab M2; Mining nimmt,
  was freigegeben ist; Steuerzeichen und Verweise können den Agenten nicht
  mehr umlenken.
