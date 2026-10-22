0.3.1

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.3.1 (2026-10-22)

- **Sicherheitsbehebung im Netz:** Die Bibliothek, die Zertifikate der
  QUIC-Verbindungen prüft, nahm Zertifikatsketten mit nicht mehr
  vertrauenswürdigen Zwischenzertifikaten an. Sie ist auf eine Fassung
  angehoben, die solche Ketten ablehnt.
