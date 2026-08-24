# Schlüssel

Hier legt der Testclient den privaten Schlüssel deines Knotens ab,
eine Datei je Knotenname.

## Was er bedeutet

Der Schlüssel **ist** die Identität deines Knotens im Netz. Zwei Folgen:

- **Bleibt er erhalten, behält dein Knoten seine Kennung über Neustarts.**
  Nur dann lassen sich die Protokolle mehrerer Läufe zusammenführen.
  Löschst du ihn, ist beim nächsten Start ein anderer Knoten da.
- **Wer ihn hat, kann in deinem Namen sprechen.** Nicht weitergeben, nicht
  in ein Repositorium legen, nicht mit dem Protokoll mitschicken. Das
  Betriebsprotokoll nennt nur die **öffentliche** Kennung, und die darf
  jeder sehen.

## Warum dieser Ordner

Bis zum 2026-08-24 schrieb der Client den Schlüssel dorthin, wo er
gestartet wurde, und das war beim Doppelklick die Wurzel des
Repositoriums. Dort stand er in keiner `.gitignore`, konnte also
versehentlich in einen Commit geraten. Seitdem liegt er hier, und der
Ordner schließt seinen eigenen Inhalt aus.
