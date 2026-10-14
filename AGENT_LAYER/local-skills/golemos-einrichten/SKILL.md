---
beschreibung: Auf GolemOS die Oberfläche (Myelith-Fenster im Vollbild) holen und starten oder den Rechner als Myelith-Knoten einrichten, Schritt für Schritt für Menschen ohne Vorwissen.
stichworte: golemos, golem, oberfläche, oberflaeche, desktop, fenster, vollbild, kiosk, grafik, bildschirm, knoten, node, server, miner, relais, teilnehmer, schicht, sqfs
---

# GolemOS einrichten

## Wann

Der Mensch arbeitet auf GolemOS (`golemos_status` antwortet, statt „Das ist
kein GolemOS“ zu sagen) und möchte

- das **Myelith-Fenster** auf dem Bildschirm statt der Konsole, oder
- diesen Rechner als **Myelith-Knoten** betreiben.

Ohne die Kiste `GolemOS` gehen dieselben Schritte mit `run_command` und
den Befehlen `golem-schicht` und `golem-knoten`.

## Die Oberfläche

1. **Stand ansehen:** `golemos_status`. Ist die Oberfläche schon geholt,
   weiter bei Schritt 4.
2. **Die Datei finden.** Sie heißt `oberflaeche-<arch>.sqfs` (`<arch>` ist
   die Ausgabe von `uname -m`, also `x86_64` oder `aarch64`), daneben
   liegt dieselbe Datei mit `.sha256`. Suchen unter `/medien/*/` und
   `/daten/`, zum Beispiel mit `run_command`:
   `find /medien /daten -maxdepth 4 -name 'oberflaeche-*.sqfs'`.
   Findet sich nichts: Den Menschen fragen, ob er sie auf den Stick legen
   kann (gebaut wird sie auf einem anderen Rechner mit
   `sh SYSTEM/golemos/schicht-bauen.sh --arch <arch>`) oder ob es eine
   https-Adresse der Veröffentlichung gibt.
3. **Holen:** `oberflaeche_holen` mit dem Pfad oder der Adresse. Es läuft
   im Hintergrund; mit `golemos_status` nachsehen, bis „geholt und
   geprüft“ dasteht. Stimmt die Prüfsumme nicht, wird nichts übernommen:
   Das sagen, nicht umgehen.
4. **Starten:** `oberflaeche_starten`. Dem Menschen sagen, dass er diese
   Konsole danach beenden sollte (sonst liegen zwei Modelle im Speicher)
   und dass er mit Strg+Alt+F2 zur Konsole zurückkommt.
5. **Beim Hochfahren** nur auf Wunsch: `oberflaeche_beim_start` mit `ja`.

## Der Knoten

⛔️ **Nur auf ausdrücklichen Wunsch**, und vorher erklären, was es heißt:
Der Rechner läuft dauerhaft, öffnet den Port 4001 nach außen und nimmt am
Netz teil. Er entscheidet dabei nichts selbst: Rolle, Pod-Zuteilung und
Abstimmungen bestimmt die Kette.

1. **Fragen:** Welcher Name? Zu Hause ist die Rolle `teilnehmer` richtig;
   `relais` nur mit öffentlicher Adresse und Portweiterleitung im Router.
2. **Einrichten:** `knoten_einrichten` mit Name und Rolle.
3. **Starten:** `knoten_schalten` mit `starten`, danach `golemos_status`.
   Steht dort „angehalten“, das Protokoll zeigen
   (`run_command`: `golem-knoten protokoll`) und den Grund nennen.
4. **Beim Hochfahren** nur auf Wunsch: `knoten_schalten` mit `an`.

## Fallen

- Eine Oberfläche ohne Prüfsumme „trotzdem“ übernehmen: geht nicht, und
  das ist Absicht.
- Den Knoten einrichten, weil der Mensch „Server“ gesagt hat, ohne zu
  fragen: Ein Knoten ist ein Dauerdienst am Netz.
- Einen Konsensschlüssel anfassen: Das macht der Mensch selbst
  (`golem-knoten konsensschluessel DATEI`), danach gehört die Vorlage vom
  Stick gelöscht.
