# WORK_DIR: der Arbeitsordner des Agenten

Dies ist der Ordner, den der Fensterclient beim ersten Start als
Arbeitsverzeichnis voreinstellt. Er liegt im Repositorium, damit jeder,
der den Client baut, **sofort etwas hat, woran die Werkzeuge sich
zeigen können**: echte Dateien mit bekannten Zeilenzahlen, ein Merkmal,
das genau zweimal vorkommt, und eine Vorlage mit Platzhaltern.

> ⚑ **Der Konsolenclient nimmt weiter das Verzeichnis, aus dem er
> gestartet wurde.** Wer `myelith` in einem Projekt aufruft, will darin
> arbeiten und nicht hier. Diese Voreinstellung gilt nur für das
> Fenster, das von keinem Verzeichnis aus geöffnet wird.

> ⚠️ **Was hier drin geschrieben wird, bleibt liegen.** Die Aufgaben
> unten legen Dateien an und ändern sie. Der Ordner ist dafür gedacht;
> wenn er unordentlich geworden ist, stellt `git checkout WORK_DIR` und
> das Löschen der neu entstandenen Dateien ihn wieder her.

---

## 1. Was hier liegt

| Pfad | Zeilen | wofür |
|---|---|---|
| `notizen.md` | 12 | eine Datei mit bekannter Zeilenzahl, enthält `MERKMAL-7` |
| `daten/messwerte.csv` | 6 | Kopfzeile und fünf Messungen, zum Rechnen und Umformen |
| `daten/protokoll.txt` | 12 | ein Ablauf mit genau zwei `FEHLER`-Zeilen |
| `unterordner/tief/versteckt.txt` | 3 | zwei Ebenen tief, enthält `MERKMAL-7` ein zweites Mal |
| `vorlagen/bericht.md` | 7 | eine Vorlage mit `{titel}`, `{autor}`, `{datum}`, `{ergebnis}` |

Zwei Zahlen, die in den Aufgaben immer wieder auftauchen: **`MERKMAL-7`
steht in genau zwei Dateien**, und **`protokoll.txt` hat genau zwei
Zeilen mit `FEHLER`**.

---

## 2. Wie diese Tabelle zu lesen ist

Je Werkzeug drei Aufgaben: **einfach** (ein Aufruf, eine Antwort),
**mittel** (zwei oder drei Aufrufe, oder ein Aufruf und ein Schluss),
**komplex** (das Werkzeug im Verbund mit anderen).

⚑ **Die erwartete Ausgabe ist die Auskunft, nicht der Wortlaut.** Ein
Modell schreibt „Die Datei hat 12 Zeilen" oder „12"; beides ist richtig.
Falsch ist eine andere Zahl, ein Werkzeug, das gar nicht läuft, oder
eine Antwort ohne Werkzeugaufruf. ⚠️ **Kleine Modelle raten gern**:
Wenn die Zahl stimmt, aber kein Aufruf stattgefunden hat, ist die
Aufgabe nicht erfüllt, sondern erraten. Der Verlauf im Client zeigt
jeden Aufruf; sieh dort nach, nicht nur auf die Antwort.

---

## 3. Die eingebauten Dateiwerkzeuge (Kiste Base)

### `list_directory` (Verzeichnis)

| | Prompt | erwartet |
|---|---|---|
| einfach | „Was liegt in meinem Arbeitsverzeichnis?" | `tiefe 1`: die drei Ordner `daten`, `unterordner`, `vorlagen` und `notizen.md`, `README.md` |
| mittel | „Zeig mir den ganzen Baum, drei Ebenen tief." | `tiefe 3`: dazu `daten/messwerte.csv`, `daten/protokoll.txt`, `vorlagen/bericht.md`, `unterordner/tief` |
| komplex | „Welche Datei liegt hier am tiefsten, und was steht darin?" | `list_directory` mit wachsender Tiefe, dann `read_file` auf `unterordner/tief/versteckt.txt`; die Antwort nennt Pfad **und** Inhalt |

### `read_file` (Lesen)

| | Prompt | erwartet |
|---|---|---|
| einfach | „Lies notizen.md vor." | der Text der Datei, darin die Zeile mit `MERKMAL-7` |
| mittel | „Was ist die höchste Messung in daten/messwerte.csv, und an welchem Tag?" | 55 ms an Tag 3 |
| komplex | „Lies das Protokoll und sag mir, wie lange zwischen dem ersten Fehler und dem Ende der Sitzung liegt." | `FEHLER zeitgrenze` 08:20, `ende` 09:00, also 40 Minuten |

### `search_files` (Suchen)

| | Prompt | erwartet |
|---|---|---|
| einfach | „Wo steht MERKMAL-7?" | zwei Treffer: `notizen.md` und `unterordner/tief/versteckt.txt`, je mit Zeilennummer |
| mittel | „Wie viele Fehler stehen im Protokoll, und welche?" | zwei: `zeitgrenze` (08:20) und `speicher` (08:41) |
| komplex | „Suche nach MERKMAL-7, lies jede gefundene Datei und sag mir, welche der beiden älter aussieht." | `search_files`, zweimal `read_file`, dann eine begründete Antwort; ⚠️ hier gibt es **keine** objektiv richtige Lösung, geprüft wird die Kette der Aufrufe |

### `write_file` (Schreiben)

⚠️ Braucht die Schreiberlaubnis in den Einstellungen.

| | Prompt | erwartet |
|---|---|---|
| einfach | „Schreib eine Datei hallo.txt mit dem Text Hallo Welt." | `hallo.txt` entsteht, ein Aufruf, keine Rückfrage |
| mittel | „Leg daten/zusammenfassung.md an, mit der Anzahl der Messungen und ihrem Mittelwert." | fünf Messungen, Mittelwert 43 ms (215 / 5) |
| komplex | „Lies das Protokoll und schreib daraus einen Fehlerbericht nach berichte/fehler.md, mit einer Zeile pro Fehler." | der Ordner `berichte` entsteht mit, zwei Zeilen im Bericht |

### `edit_file` (Ändern)

⚠️ Braucht die Schreiberlaubnis.

| | Prompt | erwartet |
|---|---|---|
| einfach | „Ändere in notizen.md MERKMAL-7 zu MERKMAL-8." | ein Aufruf mit einer Änderung, der Rest der Datei unberührt |
| mittel | „Häng an daten/messwerte.csv einen sechsten Tag mit 44 ms an." | die Kopfzeile bleibt, eine Zeile kommt dazu; ⚑ ein Modell, das hier `write_file` nimmt und die Datei neu schreibt, hat die Aufgabe gelöst, aber das schlechtere Werkzeug gewählt |
| komplex | „Ersetze in messwerte.csv jedes `ms` durch `Millisekunden`." | fünf Änderungen in **einem** Aufruf; ⚠️ `ms` kommt fünfmal vor, in jeder Datenzeile einmal, also muss jedes `alt` genug Umgebung mitnehmen (etwa `,42,ms`), um eindeutig zu sein. Das ist die Aufgabe, an der sich zeigt, ob ein Modell die Eindeutigkeitsregel verstanden hat |

---

## 4. Nur in der Kiste Advanced

### `run_command` (Befehl)

⚠️ Braucht die Schreiberlaubnis, und läuft als `sh -c`. ⛔️ **Dieses
Werkzeug hält die Einhängegrenze nicht ein**: Ein Befehl kann überall
hin. Das ist der Grund, warum es nicht in Base liegt.

| | Prompt | erwartet |
|---|---|---|
| einfach | „Wie viele Dateien liegen unter daten?" | zwei |
| mittel | „Sortiere die Messungen nach Größe und zeig sie mir." | 38, 39, 41, 42, 55 |
| komplex | „Bau mir aus messwerte.csv eine Tabelle in Markdown und schreib sie nach daten/tabelle.md." | ein Befehl oder ein Befehl plus `write_file`; die Tabelle hat fünf Datenzeilen |

---

## 5. Die Manifestwerkzeuge der Kiste Base

Dies sind die `.json`-Dateien unter `CLIENT/werkzeugkisten/Base`. ⚠️
**Auch sie laufen über die Shell und halten die Einhängegrenze nicht
ein**, weshalb sie ebenfalls die Schreiberlaubnis brauchen.

### `dateibaum`

| | Prompt | erwartet |
|---|---|---|
| einfach | „Zeig mir den Dateibaum, zwei Ebenen." | `./daten`, `./vorlagen`, `./unterordner` und die Dateien darin |
| mittel | „Wie viele Ebenen tief geht dieser Ordner?" | drei (`unterordner/tief/versteckt.txt`), ermittelt durch steigende Tiefe |
| komplex | „Zeig den Baum und sag mir, welche Ordner keine Dateien enthalten." | `unterordner` selbst enthält nur `tief` |

### `zaehle_zeilen`

| | Prompt | erwartet |
|---|---|---|
| einfach | „Wie viele Zeilen hat notizen.md?" | 12 |
| mittel | „Vergleiche die Zeilenzahl von notizen.md und daten/protokoll.txt." | beide 12, also gleich |
| komplex | „Zähle die Zeilen aller fünf Beispieldateien und nenn mir die Summe." | 12 + 6 + 12 + 3 + 7 = 40 (die README nicht mitgezählt) |

### `suche_text`

| | Prompt | erwartet |
|---|---|---|
| einfach | „Suche nach FEHLER." | zwei Zeilen aus `daten/protokoll.txt` |
| mittel | „Steht MERKMAL-7 auch irgendwo unter daten?" | nein, nur in `notizen.md` und `unterordner/tief/versteckt.txt` |
| komplex | „Such nach MERKMAL-7 und zähle danach die Zeilen jeder Datei, in der es vorkommt." | `suche_text`, dann zweimal `zaehle_zeilen`: 12 und 3 |

### `git_stand`

| | Prompt | erwartet |
|---|---|---|
| einfach | „Ist hier etwas uncommittet?" | nach den Schreibaufgaben: die neuen Dateien als `??` |
| mittel | „Welche Dateien habe ich in diesem Ordner verändert?" | `M` für geänderte, `??` für neue |
| komplex | „Sag mir, was ich verändert habe, und lies mir die geänderte Datei vor." | `git_stand`, dann `read_file` auf jeden `M`-Eintrag |

---

## 6. Nur in der Kiste Advanced

### `git_verlauf`

| | Prompt | erwartet |
|---|---|---|
| einfach | „Zeig die letzten fünf Commits." | fünf Zeilen, je Kurzfassung und Hash |
| mittel | „Wann wurde in diesem Repositorium zuletzt am Client gearbeitet?" | ein Commit-Titel, der CLIENT nennt |
| komplex | „Nimm die letzten zwanzig Commits und sag mir, welche Komponente am häufigsten vorkommt." | `git_verlauf` mit `anzahl 20`, dann gezählt |

### `platzbedarf`

| | Prompt | erwartet |
|---|---|---|
| einfach | „Wie groß ist der Ordner daten?" | 8,0K |
| mittel | „Welcher der drei Unterordner ist der größte?" | `daten` mit 8,0K, die beiden anderen 4,0K |
| komplex | „Miss alle drei Unterordner und schreib das Ergebnis nach daten/groessen.md." | dreimal `platzbedarf`, dann `write_file` |

---

## 7. Die verankerte Kiste

⚑ **Diese beiden Werkzeuge hängen immer**, unabhängig von der
gewählten Kiste und ohne Arbeitsordner: Sie rechnen aus ihren Eingaben
und fassen nichts an. Deshalb nehmen sie **Inhalt und niemals einen
Pfad**. Wer eine Datei füllen will, braucht zwei Schritte: `read_file`,
dann `fill_template`, dann `write_file`.

### `fill_template`

| | Prompt | erwartet |
|---|---|---|
| einfach | „Füll die Vorlage `Hallo {wer}` mit wer = Welt." | `Hallo Welt` |
| mittel | „Lies vorlagen/bericht.md und füll sie mit einem Titel, meinem Namen und dem heutigen Datum." | `read_file`, dann `fill_template` mit allen **vier** Feldern; ⚠️ fehlt eines, bricht das Werkzeug ab, und das ist Absicht |
| komplex | „Füll vorlagen/bericht.md mit einer Auswertung der Messwerte und schreib das Ergebnis nach berichte/messung.md." | vier Aufrufe: `read_file` Vorlage, `read_file` Messwerte, `fill_template`, `write_file` |

### `join_sections`

| | Prompt | erwartet |
|---|---|---|
| einfach | „Mach mir aus zwei Abschnitten Vorher und Nachher ein Markdown-Dokument." | zwei `##`-Überschriften mit je einer Leerzeile dazwischen |
| mittel | „Bau ein Dokument mit drei Abschnitten über die Messwerte: Aufbau, Zahlen, Schluss." | `read_file`, dann ein `join_sections` mit drei Abschnitten |
| komplex | „Schreib mir einen Bericht über diesen Ordner: ein Abschnitt pro Unterordner, mit dem, was darin liegt, und leg ihn als berichte/ordner.md ab." | `list_directory`, mehrere `read_file`, `join_sections`, `write_file`; der Bericht hat drei Abschnitte |

---

## 8. Wenn eine Aufgabe nicht klappt

⚑ **Erst die Kiste, dann das Modell.** Die häufigsten drei Ursachen,
in dieser Reihenfolge:

1. **Kein Arbeitsordner gesetzt.** Ohne ihn gibt es überhaupt keine
   Dateiwerkzeuge, und das Modell antwortet aus dem Gedächtnis.
2. **Keine Schreiberlaubnis.** Dann fehlen `write_file`, `edit_file`,
   `run_command` **und alle Manifestwerkzeuge**, weil die über die
   Shell laufen.
3. **Die falsche Kiste.** `run_command`, `git_verlauf` und
   `platzbedarf` gibt es nur in Advanced.

In den Einstellungen steht unter der Rubrik ein Knopf „Werkzeuge
anzeigen". Der sagt, was das Modell wirklich sieht, und das ist die
Auskunft, mit der die Fehlersuche anfängt.
