0.3.0

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.3.0 (2026-10-22)

- **Die Modelle heißen nach Tieren:** Sparrow 0.6B, Pigeon 4B, Jay 8B,
  Magpie 30B A3B, Raven 35B A3B, Crow 27B und Vulture 126B A7B, jeweils mit
  ihrer Speicherform, etwa „Raven 35B A3B - INT4“. Vorhandene Modellordner
  und Einstellungen benennt das Programm beim ersten Start selbst um.
- **Eigene Modelle:** `/build artifact` fragt nach einem Namen und hängt
  Größe und Speicherform an; ohne Eingabe wählt es ein Tier per Zufall.
- **Im Netz rechnet nur, was zugelassen ist:** Ein Knoten lädt nur Modelle
  von der Netzliste, und nur, wenn ihr Abdruck übereinstimmt. Heute ist das
  Raven 35B A3B. Gewichte jedes Anbieters lassen sich weiter lokal nutzen.
- **Der Debug-Log räumt sich auf:** höchstens drei Monate und 512 MiB, die
  ältesten Teile zuerst.
- **`myl eichen` misst auch die Rechenzeit je Ebene** und trägt sie ins
  Eichbuch ein.
- **Linux ohne Nix:** Ein eigenes Installationsskript baut ohne Netz und
  lässt nur das Fenster aus, wenn dessen Pakete fehlen; das Update nimmt es.
- **Ein Knoten rechnet seinen Teil aus der Kette:** Er sagt zu, wie viele
  Ebenen er hält und wie schnell er rechnet, bekommt danach seinen Bereich
  und startet den Shard dafür selbst; fehlende Dateien holt er von anderen
  Knoten. Rechnet er für das Netz, hat das Netz Vorrang, und der Agent
  bekommt den Rest, mit einer Meldung, die das sagt. Dazu ein Testnetz mit
  dem Raven 35B A3B in vier Bit. Knoten finden einander jetzt über die
  erste Gegenstelle hinaus und holen verpasste Blöcke zuverlässig nach.
- **Der Agent schreibt genauer ab:** Er bekommt seine verankerten
  Werkzeuge wieder neben `write_file` angeboten, liest seine Ergebnisdatei
  zurück und bessert sie nach. In der Messung bestand er die Aufgabe, die
  genaues Abschreiben verlangt, 13 von 14 Mal statt 7 von 14.
- **Bauen eigener Modelle:** Die Python-Umgebung wird beim ersten Bau neu
  eingerichtet (eine Umgebung für alle Bauarten).
