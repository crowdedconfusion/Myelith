# Loop-Szenario mit myelith-8b, 2026-09-26

Vier Läufe desselben Szenarios (`BENCHMARKS/Agent/loop/`); zwischen den Läufen wurde der Loop verbessert. Erzeugt mit `auswerten.py`.

- **Lauf 1** wurde nach zwei Minuten abgebrochen: Aufgabe 1 stand fälschlich auf „fertig“, ohne dass eine Datei geändert war.
- **Lauf 4** wurde nach 50 Minuten von Hand beendet; „alle Tasks beendet“ heißt dort nur, dass `myl loop` endete.

---

## Lauf 2

- **Stand:** 2026-09-26 10:29, 67 min nach dem Start, beendet (alle Tasks beendet)
- **Einstellungen:** 10 Runden je Task, 16 Schritte je Runde
- **Prüfungen bestanden:** 1 von 19

## Tasks

| Task | Zustand | Runden | Laufzeit | Ergebnis |
|---|---|---|---|---|
| Im Arbeitsordner liegt daten/auswertung.py. Es sol… | angehalten: Höchstzahl von 10 Runden erreicht | 10 | 27 min |  |
| Schreibe einen Bericht für die Betriebsleitung nac… | angehalten: Höchstzahl von 10 Runden erreicht | 10 | 39 min |  |

## Aufgabe 1: Auswertung reparieren

- ✅ Messreihe unverändert
- ❌ Skript läuft ohne Fehler (KeyError: 'sensor')
- ❌ ergebnis/statistik.md vorhanden
- ❌ T1 richtig (soll 46 | 20.5 | 19.0 | 22.0) (fehlt)
- ❌ T2 richtig (soll 48 | 22.9 | -40.0 | 85.0) (fehlt)
- ❌ T3 richtig (soll 48 | 5.6 | 4.0 | 9.4) (fehlt)
- ❌ T4 richtig (soll 47 | 22.7 | 21.1 | 25.8) (fehlt)
- ❌ Skript erzeugt dieselbe Tabelle

## Aufgabe 2: Bericht mit Recherche

- ❌ bericht/sensorbericht.md vorhanden
- ❌ mindestens drei Quellenangaben im Hausformat (0 gefunden)
- ❌ jede Quellenangabe nennt eine vorhandene Datei
- ❌ Abschnitt „## Quellen“ am Ende
- ❌ keine veraltete Unterlage als Beleg
- ❌ Kühlraum (T3) als verletzt erkannt
- ❌ Höchstwert 9,4 °C im Kühlraum genannt
- ❌ Serverraum (T4) nicht fälschlich als verletzt
- ❌ Sensor T2 als defekt ausgeklammert
- ❌ Maßnahmen Kühlraum: Qualitätssicherung
- ❌ Maßnahmen Kühlraum: Ware sperren oder Tür prüfen

## Werkzeuge

| Werkzeug | Aufrufe |
|---|---|
| `read_file` | 83 |
| `search_files` | 57 |
| `fill_template` | 22 |
| `note_set` | 14 |
| `write_file` | 9 |
| `list_directory` | 6 |
| `search_skill` | 4 |
| `learn_skill` | 3 |
| `join_sections` | 1 |

- **Testskill mit learn_skill gelernt:** ❌ (gelernt: fehlersuche)
- **Testskill mit read_file gelesen:** ✅
- **Selbst nach Skills gesucht:** ✅
- **Recherche in quellen/:** ✅ (78 Aufrufe in oder nach den Unterlagen)
- **Befehle ausgeführt:** ❌

---

## Lauf 3

- **Stand:** 2026-09-26 10:29, 25 min nach dem Start, beendet (alle Tasks beendet)
- **Einstellungen:** 10 Runden je Task, 16 Schritte je Runde
- **Prüfungen bestanden:** 5 von 19

## Tasks

| Task | Zustand | Runden | Laufzeit | Ergebnis |
|---|---|---|---|---|
| Im Arbeitsordner liegt daten/auswertung.py. Es sol… | fertig | 8 | 21 min | Die Tabelle in `ergebnis/statistik.md` wurde erfolgreich ers |
| Schreibe einen Bericht für die Betriebsleitung nac… | fertig | 2 | 3 min | Die Belege zeigen, dass der Kühlraum und der Serverraum am 2 |

## Aufgabe 1: Auswertung reparieren

- ✅ Messreihe unverändert
- ❌ Skript läuft ohne Fehler (KeyError: 'sensor')
- ✅ ergebnis/statistik.md vorhanden
- ❌ T1 richtig (soll 46 | 20.5 | 19.0 | 22.0) (fehlt)
- ❌ T2 richtig (soll 48 | 22.9 | -40.0 | 85.0) (fehlt)
- ❌ T3 richtig (soll 48 | 5.6 | 4.0 | 9.4) (fehlt)
- ❌ T4 richtig (soll 47 | 22.7 | 21.1 | 25.8) (fehlt)
- ❌ Skript erzeugt dieselbe Tabelle

## Aufgabe 2: Bericht mit Recherche

- ✅ bericht/sensorbericht.md vorhanden
- ❌ mindestens drei Quellenangaben im Hausformat (0 gefunden)
- ❌ jede Quellenangabe nennt eine vorhandene Datei
- ❌ Abschnitt „## Quellen“ am Ende
- ✅ keine veraltete Unterlage als Beleg
- ✅ Kühlraum (T3) als verletzt erkannt
- ❌ Höchstwert 9,4 °C im Kühlraum genannt
- ❌ Serverraum (T4) nicht fälschlich als verletzt (Am 20. September lag der Serverraum außerhalb seines zulässigen Temperaturbereic)
- ❌ Sensor T2 als defekt ausgeklammert
- ❌ Maßnahmen Kühlraum: Qualitätssicherung
- ❌ Maßnahmen Kühlraum: Ware sperren oder Tür prüfen

## Werkzeuge

| Werkzeug | Aufrufe |
|---|---|
| `search_files` | 57 |
| `read_file` | 16 |
| `fill_template` | 15 |
| `note_set` | 14 |
| `search_skill` | 4 |
| `list_directory` | 4 |
| `learn_skill` | 3 |
| `join_sections` | 2 |
| `write_file` | 2 |

- **Testskill mit learn_skill gelernt:** ❌ (gelernt: fehlersuche)
- **Testskill mit read_file gelesen:** ✅
- **Selbst nach Skills gesucht:** ✅
- **Recherche in quellen/:** ✅ (27 Aufrufe in oder nach den Unterlagen)
- **Befehle ausgeführt:** ❌

---

## Lauf 4

- **Stand:** 2026-09-26 10:29, 50 min nach dem Start, beendet (alle Tasks beendet)
- **Einstellungen:** 10 Runden je Task, 16 Schritte je Runde
- **Prüfungen bestanden:** 1 von 19

## Tasks

| Task | Zustand | Runden | Laufzeit | Ergebnis |
|---|---|---|---|---|
| Im Arbeitsordner liegt daten/auswertung.py. Es sol… | bereit | 8 | 49 min |  |
| Schreibe einen Bericht für die Betriebsleitung nac… | bereit | 0 | 0 min |  |

## Aufgabe 1: Auswertung reparieren

- ✅ Messreihe unverändert
- ❌ Skript läuft ohne Fehler (KeyError: 'sensor')
- ❌ ergebnis/statistik.md vorhanden
- ❌ T1 richtig (soll 46 | 20.5 | 19.0 | 22.0) (fehlt)
- ❌ T2 richtig (soll 48 | 22.9 | -40.0 | 85.0) (fehlt)
- ❌ T3 richtig (soll 48 | 5.6 | 4.0 | 9.4) (fehlt)
- ❌ T4 richtig (soll 47 | 22.7 | 21.1 | 25.8) (fehlt)
- ❌ Skript erzeugt dieselbe Tabelle

## Aufgabe 2: Bericht mit Recherche

- ❌ bericht/sensorbericht.md vorhanden
- ❌ mindestens drei Quellenangaben im Hausformat (0 gefunden)
- ❌ jede Quellenangabe nennt eine vorhandene Datei
- ❌ Abschnitt „## Quellen“ am Ende
- ❌ keine veraltete Unterlage als Beleg
- ❌ Kühlraum (T3) als verletzt erkannt
- ❌ Höchstwert 9,4 °C im Kühlraum genannt
- ❌ Serverraum (T4) nicht fälschlich als verletzt
- ❌ Sensor T2 als defekt ausgeklammert
- ❌ Maßnahmen Kühlraum: Qualitätssicherung
- ❌ Maßnahmen Kühlraum: Ware sperren oder Tür prüfen

## Werkzeuge

| Werkzeug | Aufrufe |
|---|---|
| `note_set` | 44 |
| `run_command` | 38 |
| `read_file` | 33 |
| `edit_file` | 22 |
| `search_files` | 3 |
| `search_skill` | 1 |
| `learn_skill` | 1 |

- **Testskill mit learn_skill gelernt:** ❌ (gelernt: fehlersuche)
- **Testskill mit read_file gelesen:** ❌
- **Selbst nach Skills gesucht:** ✅
- **Recherche in quellen/:** ✅ (2 Aufrufe in oder nach den Unterlagen)
- **Befehle ausgeführt:** ✅
