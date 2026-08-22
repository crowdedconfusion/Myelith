# Zeitstempel-Nachweise

Hier liegen die OpenTimestamps-Nachweise (`.ots`) zu den Dateien in
`README/Whitepaper/`. Ein solcher Nachweis belegt, dass eine Datei mit
genau diesen Bytes zu einem bestimmten Zeitpunkt existierte, verankert in
der Bitcoin-Blockchain.

## Prüfen

```
ots verify myelith-whitepaper-v0.3.pdf.ots
```

Der Befehl erwartet die belegte Datei im übergeordneten Verzeichnis oder
über `-f <pfad>`. Er meldet den Blockzeitpunkt, gegen den der Nachweis
verankert ist.

## Was ein solcher Nachweis ist und was nicht

Ein `.ots` enthält **keinen Inhalt**, sondern den Hash der belegten Datei
und den Merkle-Pfad zu einer Bitcoin-Transaktion. Er belegt einen
Zeitpunkt, keine Urheberschaft und keine Richtigkeit.

Der Nachweis bindet **exakte Bytes**. Wird die belegte Datei auch nur um
ein Zeichen geändert oder das PDF neu gebaut, gilt er für die neue Fassung
nicht mehr und muss neu erzeugt werden.

## Ablauf beim Erzeugen

1. Die endgültige Fassung bauen. **Danach nichts mehr ändern.**
2. `ots stamp <datei>` für jede zu belegende Datei.
3. Der frische Nachweis ist zunächst vorläufig und hängt nur an den
   Kalenderservern. Nach der Bestätigung in der Blockchain, meist einige
   Stunden später, einmal `ots upgrade <datei>.ots` ausführen und die
   aktualisierte Datei ablegen.
4. Erst der aktualisierte Nachweis ist ohne Rückfrage bei einem
   Kalenderserver prüfbar.
