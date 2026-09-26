# Loop-Szenario: zwei Aufgaben, ein Testskill, eine Recherche

Misst, ob ein Modell im **Loop** zwei zusammenhängende Aufgaben über
mehrere Runden selbstständig löst: Es soll dabei Werkzeuge wechseln,
einen Skill **selbst finden und lernen** und in Unterlagen recherchieren.
Ausgelegt auf rund zwei Stunden mit dem 8B-Modell, mit einer harten Frist.

```text
python3 BENCHMARKS/Agent/loop/starten.py INTEGER_LLM/artifacts/myelith-8b <laufordner> --frist-min 170
python3 BENCHMARKS/Agent/loop/auswerten.py <laufordner>                       # Stand, jederzeit
python3 BENCHMARKS/Agent/loop/auswerten.py <laufordner> --bericht <datei.md>  # Auswertung als Datei
```

Der Laufordner liegt **außerhalb** des Repositoriums; `starten.py` lehnt
einen Ordner darin ab. `myl` muss gebaut sein
(`cd CLIENT/myl-client && cargo build --release --bin myl`).

## Die zwei Aufgaben (`aufgaben.json`)

| # | Aufgabe | Was sie verlangt |
|---|---|---|
| 1 | `daten/auswertung.py` zum Laufen bringen | vier Fehler nacheinander finden (Trennzeichen, Dezimalkomma, `n/a`, fehlender Ordner), ohne die Messreihe zu ändern; Ausführen mit `run_command`, Ändern mit `edit_file` |
| 2 | Bericht für die Betriebsleitung | Zahlen aus Aufgabe 1 verwenden, in `quellen/` Zuordnung, Grenzwerte und Maßnahmen **recherchieren**, eine veraltete Grenzwertliste und eine alte FAQ erkennen, einen defekten Sensor ausklammern, und die **Hausregeln für Quellenangaben** einhalten |

⚑ **Aufgabe 1 hat einen Abnahmebefehl** (`abnahme` in `aufgaben.json`):
das Skript ausführen und für jeden der vier Sensoren eine Tabellenzeile
verlangen. Endet er mit 0, ist der Task fertig, sonst nie, und seine
Ausgabe steht in den Notizen der nächsten Runde.

📌 **Die erste Fassung prüfte nur „läuft durch und schreibt eine nicht
leere Datei“**, und das 30B bestand sie mit verdeckten Fehlern:
`zeile.get("sensor", "n/a")` statt des richtigen Trennzeichens, eine
einzige Zeile „n/a“ mit Nullen. **Eine Abnahme muss die Anforderung
prüfen, nicht nur das Durchlaufen**, sonst belohnt sie genau das
Verdecken eines Symptoms. Gegenprobe der neuen Fassung: Die Musterlösung
besteht, der verdeckte Stand fällt durch. **Aufgabe 2 hat keinen**, mit Absicht: Eine
Abnahme dort müsste das Hausformat prüfen und verriete es dem Modell im
Auftrag, und dann wäre nicht mehr zu sehen, ob es den Skill selbst
findet. `--ohne-abnahme` fährt beide ohne (wie die ersten Läufe mit dem
8B).

⚑ **Die Hausregeln sind ein Skill im Projekt** (`vorlage/.AGENT/skills/hausregeln-quellen/`).
Die Aufgabe nennt ihn nicht beim Namen; das Modell muss ihn mit
`search_skill` finden und mit `learn_skill` lernen. Ob es das getan hat,
zeigt sowohl das Format der Quellenangaben im Bericht als auch das
Protokoll der Werkzeugaufrufe.

⚑ **Die Fallen sind absichtlich:** Der Kühlraum (T3) verletzt nur die
**neue** Grenze (8,0 °C), der Serverraum (T4) nur die **alte** (22 °C);
wer die ersetzte Liste oder die FAQ von 2023 liest, bewertet beide
falsch herum.

## Was der Lauf anlegt

| Datei | Inhalt |
|---|---|
| `arbeit/` | frische Kopie von `vorlage/`, Arbeitsordner des Agenten |
| `cfg/` | abgeschirmte Einstellungen (`MYL_EINSTELLUNGEN`) samt Tasks und Tagebüchern |
| `loop.log` | Ausgabe von `myl loop`, mit jedem Werkzeugaufruf |
| `lauf.json` | Start, Frist, Artefakt, Kennungen der Tasks |
| `starter.log` | was `starten.py` tat; die letzte Zeile ist `Fertig: <grund>` |

⚠️ **Der Lauf steht im `auto mode`**: Ohne Terminal kann niemand eine
Handlung bestätigen. Er arbeitet in einer Kopie, mit eigenen
Einstellungen und der Werkzeugkiste `Advanced` (für `run_command`).

## Die Auswertung

19 Prüfungen, jede mit einem Urteil aus der Außenwelt: Läuft das Skript
(in einer Kopie ausgeführt), stimmen die Zahlen mit der aus der Vorlage
gerechneten Statistik überein, trägt der Bericht die Befunde und die
Quellenangaben im Hausformat. Dazu die Zählung der Werkzeugaufrufe.

⚠️ **Die Prüfungen am Bericht sind Heuristiken** (Muster in Zeilen). Eine
Auswertung liest den Bericht deshalb zusätzlich selbst.

**Gegenprobe:** Eine von Hand gebaute Musterlösung besteht 19 von 19, ein
unberührter Arbeitsordner 1 von 19 (die Messreihe ist unverändert).
