# Loop-Szenario mit myelith-30b-a3b, 2026-09-26

Vier Läufe desselben Szenarios (`BENCHMARKS/Agent/loop/`), mit Abnahmebefehl für Aufgabe 1. Zwischen den Läufen wurden Loop und Werkzeuge verbessert. Erzeugt mit `auswerten.py`.

- **Lauf 1** (18 min, von Hand beendet): Die erste Abnahme prüfte nur „läuft durch, Datei nicht leer“ und ließ eine verdeckte Nulltabelle durch.
- **Lauf 2** (8 min, von Hand beendet): Das Modell schrieb Werkzeugaufrufe in seine Antwort, statt sie zu rufen, und hielt eine vorhandene Datei für fehlend.
- **Lauf 3** (43 min): nach den Behebungen.
- **Lauf 4** (75 min): wie Lauf 3, dazu mit dem fest vorgegebenen Systemprompt (`en.md`, SHA-256 `43f50a41…d34621`).

---

## Lauf 1

- **Stand:** 2026-09-26 12:26, 18 min nach dem Start, beendet (alle Tasks beendet)
- **Einstellungen:** 10 Runden je Task, 16 Schritte je Runde
- **Prüfungen bestanden:** 3 von 19

## Tasks

| Task | Zustand | Runden | Laufzeit | Ergebnis |
|---|---|---|---|---|
| Im Arbeitsordner liegt daten/auswertung.py. Es sol… | fertig | 2 | 17 min | Abnahme bestanden: python3 daten/auswertung.py && test -s er |
| Schreibe einen Bericht für die Betriebsleitung nac… | bereit | 0 | 0 min |  |

## Aufgabe 1: Auswertung reparieren

- ✅ Messreihe unverändert
- ✅ Skript läuft ohne Fehler
- ✅ ergebnis/statistik.md vorhanden
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
| `read_file` | 5 |
| `run_command` | 5 |
| `list_directory` | 3 |
| `edit_file` | 3 |

- **Testskill mit learn_skill gelernt:** ❌ (gelernt: keiner)
- **Testskill mit read_file gelesen:** ❌
- **Selbst nach Skills gesucht:** ❌
- **Recherche in quellen/:** ❌ (0 Aufrufe in oder nach den Unterlagen)
- **Befehle ausgeführt:** ✅

---

## Lauf 2

- **Stand:** 2026-09-26 12:26, 8 min nach dem Start, beendet (alle Tasks beendet)
- **Einstellungen:** 10 Runden je Task, 16 Schritte je Runde
- **Prüfungen bestanden:** 1 von 19

## Tasks

| Task | Zustand | Runden | Laufzeit | Ergebnis |
|---|---|---|---|---|
| Im Arbeitsordner liegt daten/auswertung.py. Es sol… | angehalten: 3 Runden ohne Fortschritt | 3 | 5 min |  |
| Schreibe einen Bericht für die Betriebsleitung nac… | bereit | 2 | 2 min |  |

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
| `read_file` | 2 |
| `list_directory` | 1 |
| `search_files` | 1 |
| `note_set` | 1 |

- **Testskill mit learn_skill gelernt:** ❌ (gelernt: keiner)
- **Testskill mit read_file gelesen:** ❌
- **Selbst nach Skills gesucht:** ❌
- **Recherche in quellen/:** ❌ (0 Aufrufe in oder nach den Unterlagen)
- **Befehle ausgeführt:** ❌

---

## Lauf 3

- **Stand:** 2026-09-26 12:26, 43 min nach dem Start, beendet (alle Tasks beendet)
- **Einstellungen:** 10 Runden je Task, 16 Schritte je Runde
- **Prüfungen bestanden:** 3 von 19

## Tasks

| Task | Zustand | Runden | Laufzeit | Ergebnis |
|---|---|---|---|---|
| Im Arbeitsordner liegt daten/auswertung.py. Es sol… | angehalten: 3 Runden ohne Fortschritt | 3 | 33 min |  |
| Schreibe einen Bericht für die Betriebsleitung nac… | fertig | 2 | 9 min | Der Bericht für die Betriebsleitung wurde erstellt und in de |

## Aufgabe 1: Auswertung reparieren

- ✅ Messreihe unverändert
- ✅ Skript läuft ohne Fehler
- ✅ ergebnis/statistik.md vorhanden
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
| `read_file` | 26 |
| `run_command` | 9 |
| `edit_file` | 5 |
| `list_directory` | 2 |
| `fill_template` | 1 |
| `finish_goal` | 1 |

- **Testskill mit learn_skill gelernt:** ❌ (gelernt: keiner)
- **Testskill mit read_file gelesen:** ❌
- **Selbst nach Skills gesucht:** ❌
- **Recherche in quellen/:** ✅ (6 Aufrufe in oder nach den Unterlagen)
- **Befehle ausgeführt:** ✅

---

## Lauf 4

- **Stand:** 2026-09-26 15:23, 75 min nach dem Start, beendet (alle Tasks beendet)
- **Einstellungen:** 10 Runden je Task, 16 Schritte je Runde
- **Prüfungen bestanden:** 1 von 19

## Tasks

| Task | Zustand | Runden | Laufzeit | Ergebnis |
|---|---|---|---|---|
| Im Arbeitsordner liegt daten/auswertung.py. Es sol… | angehalten: 3 Runden ohne Fortschritt | 6 | 46 min |  |
| Schreibe einen Bericht für die Betriebsleitung nac… | angehalten: 3 Runden ohne Fortschritt | 8 | 28 min |  |

## Aufgabe 1: Auswertung reparieren

- ✅ Messreihe unverändert
- ❌ Skript läuft ohne Fehler (SyntaxError: 'return' outside function)
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
| `read_file` | 33 |
| `note_set` | 28 |
| `edit_file` | 22 |
| `run_command` | 21 |
| `list_directory` | 4 |
| `search_files` | 3 |

- **Testskill mit learn_skill gelernt:** ❌ (gelernt: keiner)
- **Testskill mit read_file gelesen:** ❌
- **Selbst nach Skills gesucht:** ❌
- **Recherche in quellen/:** ✅ (18 Aufrufe in oder nach den Unterlagen)
- **Befehle ausgeführt:** ✅
