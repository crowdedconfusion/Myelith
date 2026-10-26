0.4.4

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.4.4 (2026-10-26)

- **Ein Knoten, der nichts mehr hört, fragt selbst nach.** Bisher erfuhr ein
  Knoten nur über das Netz, dass er zurücklag; verpasste er den letzten
  Schub, blieb er stehen. Jetzt fordert er nach zehn Sekunden Stillstand
  selbst die nächsten Blöcke an.
- **Ehrliche Miner werden über schlechte Leitungen nicht mehr gesperrt.**
  Die Zeitprobe eines Miners zählt jetzt die kürzeste von fünf Messungen,
  ein verlorenes Paket gilt nicht mehr als Rechenzeit.

Seit 0.4.1 ausserdem:

- **Sicherheitskorrektur in einer Abhängigkeit:** Die Laufzeitbibliothek
  des Netzes (`tokio`) ist auf einer Fassung ohne die gemeldete Schwäche bei
  Dateideskriptoren.

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
