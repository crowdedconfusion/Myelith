0.4.2

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.4.2 (2026-10-24)

- **Archivknoten:** Eine kleine, ausgeloste Gruppe von Knoten hält die ganze
  Geschichte der Kette, belegt das je Epoche mit Stichproben und gibt sie an
  Nachfolger weiter; dazu verteilte Teile, eine kalte Ausfuhr und eine
  Einfuhr nach einem Totalausfall. Alle anderen Knoten dürfen ihre Kette
  verdichten und brauchen weniger Platz.
- **Ein Knoten startet nach einem Absturz in Sekunden** statt Minuten: Er
  setzt beim eigenen Zustandsabzug an.
- **Mehr Schutz für das Netz:** Blöcke haben eine Grössengrenze, teure
  Anfragen werden gedrosselt, neue Absender ohne Guthaben kommen nur
  gedrosselt in den Speicher wartender Transaktionen, und eine erfundene
  Blockhöhe hält einen Knoten nicht mehr dauerhaft auf.
- **Die Empfänger eines Sitzungskontrakts stehen nicht mehr auf der Kette**,
  nur noch ein Fingerabdruck davon; wer zahlt, belegt den einen Empfänger.
