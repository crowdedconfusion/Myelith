0.10.0

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.10.0 (2026-11-14)

⚠️ **Seit 0.9.0 gelten neue Regeln für die Ausschüttung.** Wer von einer
älteren Fassung kommt: Eine Kette von dort lässt sich nicht weiterführen;
ein Knoten beginnt sie neu.

- **Texte, die der Agent schreibt, tragen eine unsichtbare Herkunftsangabe**
  nach C2PA (Markdown und Text ab 800 Zeichen, beim Schreiben, Ändern und
  Ersetzen). Beim Lesen und Suchen schneidet der Agent sie wieder ab.
  Abschalten mit `agent.text_kennzeichnen`; nachprüfen mit
  `myl kennzeichen`.
- **Unter macOS folgt die Aufnahme dem Eingang, den das System gerade
  nutzt**, statt fest auf das erste Gerät zu hören. Fehlt die Freigabe für den Bildschirm, sagt
  Myelith das genau so, statt zu raten.
- **GolemOS aktualisiert sich mit Rückfall:** `golem-aktualisieren`
  schreibt ein neues System auf die zweite Seite und startet es einmal zur
  Probe; kommt es nicht hoch, startet der Rechner wieder das alte. Jede
  Seite hat ihren eigenen Kern.
- **GolemOS-Sticks haben einen exFAT-Datenteil**, an jedem Rechner les- und
  beschreibbar, ohne Grenze von 4 GB je Datei; GolemOS liest auch fremde
  Laufwerke mit exFAT. Ältere Sticks behalten ihren FAT32-Datenteil.
- **GolemOS bringt die Lizenztexte aller Pakete mit**, unter
  `/usr/share/licenses/`, dazu eine Übersicht in
  `/usr/share/golemos/LIZENZEN.txt`. Der erste Start eines frischen Sticks
  dauert einmalig bis zu einer Minute länger.
