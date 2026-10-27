0.5.0

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.5.0 (2026-10-27)

⚠️ **Neues Format für Blöcke und Transaktionen.** Eine Kette aus einer
älteren Fassung lässt sich nicht weiterführen; ein Knoten beginnt sie neu.

- **Jede Transaktion kostet eine kleine Gebühr**, nach ihrer Grösse in
  Bytes. Der Preis steigt, wenn die Blöcke voll sind, und sinkt, wenn sie
  leer sind; mehr als zugesagt wird nie abgebucht. Wer an ein neues Konto
  überweist, zahlt einmal einen Aufschlag. Eine Anmeldung als Miner und eine
  Sitzung für einen Agenten hinterlegen einen Betrag, der beim Abmelden und
  nach dem Ende der Sitzung zurückkommt.
- **Auf Testnetzen versorgt sich ein Knoten selbst** mit Test-MYL für
  Gebühr und Hinterlegung; niemand muss ihm vorher etwas überweisen.
- **Das Wallet:** zwölf Wörter zum Sichern und Wiederherstellen, Guthaben
  jedes Wallets, Überweisen mit einer Vorschau, die Betrag, Gebühr und
  Aufschlag zeigt, bevor etwas unterschrieben wird; Credits kaufen und
  einem Agenten eine Sitzung mit Budget eröffnen.
- **Mining mit Auszahlungskonto:** Ein Knoten meldet sich selbst an und
  trägt das Konto ein, auf das er bezahlt wird. Wer es wechseln will,
  braucht den Schlüssel des bisherigen Kontos.
- **Endgültigkeit:** Jeder Block ist vom Erzeuger unterschrieben, und alle
  150 Blöcke bestätigen die Validatoren einen Prüfpunkt. Wer zwei
  widersprüchliche Blöcke oder Stimmen unterschreibt, verliert Einsatz. Ein
  Knoten, der sehr lange weg war, hält an, statt einer fremden Geschichte zu
  folgen.
- **Antworten über einen Pod kommen schneller und zuverlässiger:** Der
  Prompt geht in einem Stück durch die Shards, und ein Wechsel der Epoche
  oder eine verlorene Runde bricht eine Anfrage nicht mehr ab.
- **Die Zeitprobe eines Miners ist schärfer:** Sie misst ohne Ping, mit
  mehr Positionen bei schnellen Zusagen, und ehrliche Miner kurz nach dem
  Start einer Epoche werden nicht mehr gesperrt.

Seit 0.4.1 ausserdem:

- **Ein Knoten, der nichts mehr hört, fragt selbst nach**, nach zehn
  Sekunden Stillstand.
- **Ehrliche Miner werden über schlechte Leitungen nicht mehr gesperrt.**
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
  Anfragen werden gedrosselt, und eine erfundene Blockhöhe hält einen Knoten
  nicht mehr dauerhaft auf.
- **Die Empfänger eines Sitzungskontrakts stehen nicht mehr auf der Kette**,
  nur noch ein Fingerabdruck davon; wer zahlt, belegt den einen Empfänger.
