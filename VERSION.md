0.7.0

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.7.0 (2026-11-07)

⚠️ **Neue Regeln für Konsens und Finalität.** Eine Kette aus einer älteren
Fassung lässt sich nicht weiterführen; ein Knoten beginnt sie neu.

- **Über die Finalität stimmen alle Miner mit genügend Einsatz und
  bezeugter Arbeit ab**, nicht mehr ein fester Kreis. Zwei Drittel des
  Einsatzes machen einen Prüfpunkt endgültig. Der Kreis folgt dem Zustand
  der Kette und ändert sich je Epoche nur wenig.
- **Blöcke erzeugen die Miner reihum.** Gabeln löst jeder Knoten selbst
  auf, nie über einen endgültigen Prüfpunkt hinweg; wer eine Weile auf
  einem anderen Zweig war, wechselt von selbst auf den richtigen.
- **Wer doppelt abstimmt, verliert Einsatz**, und zwar umso mehr, je mehr
  andere zur selben Zeit dasselbe tun.
- **Wer lange nicht abstimmt, ruht**, bis er sich neu anmeldet; sein
  Einsatz bleibt. Fallen viele zugleich aus, kehrt die Finalität danach von
  selbst zurück.
- **Die Zufallssaat jeder Epoche kann niemand mehr beeinflussen:** Sie
  entsteht gemeinsam aus einer Schwellensignatur der Miner.
- **Ein neuer oder lange abwesender Knoten steigt über ein Zertifikat
  ein** und holt dort einen Abzug des Zustands, statt alles ab Beginn
  nachzurechnen. ⚠️ Ruhte die Finalität so lange, dass Miner ausgeschlossen
  wurden, braucht ein Knoten ohne eigenen Zustand einen Anker von außen.
- **Prüfer kontrollieren auch, welches Wort ein Pod gewählt hat.** Wer
  eine Antwort verfälscht und danach ehrlich weiterrechnet, wird gefunden.

