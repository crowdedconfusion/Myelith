---
beschreibung: Eine bestehende Datei gezielt ändern, ohne Inhalt zu verlieren oder etwas Falsches zu ersetzen.
stichworte: datei ändern, bearbeiten, ersetzen, edit, anpassen, korrigieren, umschreiben, code ändern, modify, replace, refactor
---

# Eine Datei sicher ändern

## Wann

Eine vorhandene Datei soll sich ändern: eine Zeile, ein Abschnitt, ein
Wert. Nicht für neue Dateien; die schreibt `write_file`.

## Vorgehen

1. **Erst lesen:** `read_file`, damit du den genauen Wortlaut kennst.
   Nie aus dem Gedächtnis ändern.
2. **`edit_file` statt `write_file`:** Es ersetzt nur die Stelle. Wer
   die ganze Datei neu schreibt, verliert leicht den Rest.
3. **Ein eindeutiger Anker:** Der zu ersetzende Text muss genau einmal
   vorkommen. Nimm dafür eine ganze Zeile oder zwei, nicht ein
   einzelnes Wort.
4. **Mehrere Stellen** in einem Aufruf, der Reihe nach; jede sieht das
   Ergebnis der vorigen.
5. **Prüfen:** Die geänderte Stelle noch einmal mit `read_file` lesen.

## Fallen

- Der Anker kommt zweimal vor: `edit_file` lehnt ab. Mehr Umgebung
  mitnehmen.
- Einrückung und Leerzeichen gehören zum Anker; genau so abschreiben,
  wie `read_file` sie zeigt.
- Eine Datei ganz neu schreiben, nur um eine Zeile zu ändern.
