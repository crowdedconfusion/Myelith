0.4.1

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.4.1 (2026-10-23)

- **Netzstapel auf libp2p 0.57.** Damit sind zwei Sicherheitsmeldungen in der
  DNS-Bibliothek der Suche im lokalen Netz behoben (RUSTSEC-2026-0118 und
  -0119), dazu zwei im Gossip-Protokoll.
- **QUIC verbindet jetzt mit einem post-quantensicheren Schlüsseltausch**,
  wenn beide Seiten ihn können.
- Die Suche im lokalen Netz nennt anderen Knoten nur noch Adressen, unter
  denen sie wirklich erreichbar sind.
- Beim Bauen wird jetzt eine weitere C-Bibliothek übersetzt (für QUIC); der
  erste Bau dauert dadurch etwas länger.
