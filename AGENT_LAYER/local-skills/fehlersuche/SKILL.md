---
beschreibung: Einen Fehler systematisch finden und beheben, statt zu raten.
stichworte: fehler, bug, debug, debugging, geht nicht, funktioniert nicht, kaputt, absturz, abstürzen, stürzt, crash, crashes, error, exception, traceback, fehlermeldung, falsches ergebnis, weiß nicht weiter, komme nicht weiter, stuck
---

# Fehlersuche

## Wann

Etwas tut nicht, was es soll: eine Fehlermeldung, ein falsches
Ergebnis, ein Absturz. Oder du hast schon zweimal etwas versucht, und es
ging nicht.

## Vorgehen

1. **Ausführen, nicht vermuten:** Gibt es `run_command`, starte das
   Programm oder den Befehl, der scheitert (etwa `python3 skript.py`), und
   lies die echte Fehlermeldung. Eine Vermutung aus dem Quelltext ersetzt
   sie nicht.
2. **Genau festhalten, was passiert:** die vollständige Fehlermeldung,
   die Eingabe, das erwartete und das tatsächliche Ergebnis. Nichts
   zusammenfassen.
3. **Die Stelle finden:** mit `search_files` nach einem markanten Wort
   aus der Meldung suchen (Funktionsname, Dateiname, Text der Meldung).
   Dann die Fundstelle mit `read_file` lesen, mit etwas Umgebung.
4. **Eine Vermutung aufschreiben**, eine einzige: „Der Fehler entsteht,
   weil …“. Sie muss sich prüfen lassen.
5. **Prüfen, bevor du änderst:** Stimmt die Vermutung mit dem Code
   überein? Wenn nicht, zurück zu Schritt 3.
6. **Die kleinste Änderung** machen, die die Ursache behebt, mit
   `edit_file`. Nicht mehrere Dinge auf einmal.
7. **Belegen:** denselben Fall noch einmal ausführen. Erst ein Beleg macht
   den Fehler behoben.

## Fallen

- Das Symptom beheben statt der Ursache: Die Meldung verschwindet, der
  Fehler bleibt.
- Mehrere Änderungen auf einmal: Danach weiß niemand, welche half.
- Raten statt lesen: Erst die Stelle lesen, dann ändern.
- Nach drei gescheiterten Versuchen: aufhören, festhalten, was du weißt,
  und den Nutzer fragen.
