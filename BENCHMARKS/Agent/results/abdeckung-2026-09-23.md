# Werkzeugabdeckung 2026-09-23: ruft `myelith-4b` jedes Werkzeug?

**Aufbau:** `agentenprobe.py --auftraege werkzeugabdeckung.json`,
Werkzeugsatz **Advanced** (19 Werkzeuge, 8909 Zeichen Ansage),
Ansageform amtlich, sechs Schritte, **ein** Lauf je Auftrag. θ_v 0.22.0.

⚑ **Je Auftrag ein erwartetes Werkzeug**, und die Probe prüft beides:
die Wirkung **und** ob genau dieses Werkzeug gerufen wurde. Ohne das
zweite hätten die Sinnesaufträge immer bestanden, denn was ein Sehmodell
über ein Bild sagt, weiss vorher niemand.

## Ergebnis: 13 von 14

| Werkzeug | gerufen | s |
|---|---|---|
| `list_directory` | ✅ | 24,9 |
| `read_file` | ✅ | 21,9 |
| `write_file` | ✅ | 34,4 |
| `edit_file` | ✅ | 35,6 |
| `search_files` | ✅ | 31,2 |
| `suche_text` | ⛔️ stattdessen `search_files` | 74,1 |
| `dateibaum` | ✅ | 42,2 |
| `zaehle_zeilen` | ✅ | 26,8 |
| `fill_template` | ✅ | 33,6 |
| `join_sections` | ✅ | 36,4 |
| `describe_image` | ✅ | 43,9 |
| `transcribe_audio` | ✅ | 30,0 |
| `run_command` | ✅ | 37,5 |
| `git_stand` | ✅ | 35,9 |

⚑ **Beide Sinneswerkzeuge werden gerufen**, und beide liefern: Das
Sehmodell beschreibt ein echtes Bild, das Hörmodell schreibt eine echte
Aufnahme mit.

## Der eine Fehlschlag, und er ist ein Fund über die Kiste

`suche_text` wird nicht gerufen; das Modell nimmt `search_files` und
**löst die Aufgabe damit**. Die beiden überschneiden sich:

```
suche_text:   "... Ergaenzt search_files, das nur Dateinamen sucht."
search_files: "... Returns file, line number and line."
```

⛔️ **Die Begründung, mit der `suche_text` seine Existenz rechtfertigt,
ist falsch.** `search_files` durchsucht Zeileninhalte und gibt Datei,
Zeilennummer und Zeile zurück (`for (nr, zeile) in
text.lines().enumerate()` in `werkzeuge.rs`). ⚑ **Das Modell hat also
richtig gewählt**, und die Ansage trägt ein Werkzeug mit, das nichts
hinzufügt und jede Wahl schwerer macht.

## ⛔️ Zwei Messfehler auf dem Weg hierher, beide sahen wie Modellfehler aus

**Erster Anlauf, Fund 437: die falsche Kiste.** Die Probe übergab
`--werkzeuge voll`. `myl` kennt nur `Base`, `Advanced` und `1337`,
**warnte und fuhr mit der eingestellten Kiste weiter**, also Base. Über
dem Ergebnis stand „Werkzeugsatz: voll"; `run_command` war in keinem
einzigen Lauf im Angebot, und das Modell antwortete korrekt, es sehe
keine Funktion für Befehle. Behoben: Die Probe übergibt `Advanced`, und
`myl` bricht bei einem unbekannten Namen mit Rückgabewert 2 ab.

**Zweiter Anlauf, Fund 438: die Erlaubnis statt der Fähigkeit.**
`kisten::angebote` gibt ohne Schreiberlaubnis eine **leere Liste**
zurück, denn ein Manifest läuft über eine Shell und eine Shell kann
immer schreiben; `run_command` gilt aus demselben Grund als schreibend.
Ohne `schreiben: true` standen `suche_text`, `dateibaum`,
`zaehle_zeilen`, `git_stand` und `run_command` gar nicht im Angebot.

📌 **Die Lehre aus beiden ist dieselbe.** Ein Fehlschlag in einer
Agentenmessung sieht zuerst immer nach dem Modell aus. Gefunden hat die
Ursache nicht die Probe, sondern ein Lauf von Hand, bei dem die Denkspur
mitgelesen wurde: Das Modell sagte klar, welche Funktion es nicht sieht.
**Eine Probe, die nur ihr Urteil aufhebt, kann das nicht sagen**, und
genau deshalb schreibt sie seit heute die gerufenen Werkzeuge mit.

## ⚠️ Was diese Messung nicht sagt

- **Ein Lauf je Werkzeug.** Für die Frage „erreicht es das Werkzeug"
  genügt das; eine Quote ist es nicht.
- **Nicht abgedeckt:** `list_skills`, `read_skill` (brauchen einen
  Fähigkeitsordner), `list_history`, `read_history`, `search_history`
  (brauchen einen Mitschnitt), `bildschirm_ansehen`, `kamera_ansehen`
  (brauchen Scharfstellung und die Freigabe des Betriebssystems).
- **Nur `myelith-4b`.** Das 0,6B ist nicht gemessen; nach der gestuften
  Reihe desselben Tages besteht es Stufe 1 vollständig, aber über diese
  vierzehn Werkzeuge sagt das nichts.
