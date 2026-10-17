0.1.0

# Myelith: Fassungen und was sich mit ihnen geändert hat

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht je Fassung ein Abschnitt, die neueste oben. Die Überschrift
beginnt mit der Fassung; was danach bis zur nächsten Überschrift steht,
zeigt das Programm vor dem Update als Liste der Änderungen.

## 0.1.0 (2026-10-17)

Die erste Fassung mit eigener Nummer. Sie umfasst den ganzen Stand bis
hierher, darunter:

- **Updates aus dem Programm heraus:** in der Konsole mit `/update`, im
  Fenster beim Start von selbst; vor dem Update wird gefragt, danach startet
  das Programm neu.
- **Der Dispatcher** verteilt Arbeitsspeicher, Grafikspeicher, Kerne und
  Karten auf dem eigenen Rechner, mit Vorrang für das Sprachmodell vor Sehen
  und Hören.
- **Das 35B mit Experten in vier Bit ist der Standard:** 19 statt 33 GB,
  Perplexität 0,06 % über der int8-Fassung, ohne Training.
- **Modelle holen und bauen** aus Konsole und Fenster, nur wenn der Platz
  reicht und nach Bestätigung.
- **Mining im Probelauf:** Platten, Kerne und jede Karte einzeln freigeben,
  im Reiter „Knoten“ und mit `/mining`.
- **Der Agent** arbeitet ohne Schrittgrenze und hält an, wenn er sich im
  Kreis dreht.
- **Jeder neue Agenten-Prozess im Fenster** fragt, ob der Einhängepfad
  stimmt; ohne Pfad öffnet sich gleich die Ordnerwahl.
- **Modellnamen** nach dem Schema „Myelith 35B-A3B INT8“.
- Das Fenster startet in rund acht Sekunden statt in siebzig.
