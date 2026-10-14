# Schlafprobe: myelith-35b-a3b (int8), mit und ohne Schlaf

- **Stand:** 2026-10-14, zwei Läufe derselben Probe (`BENCHMARKS/Agent/schlaf`), zwischen den Läufen zwei Fehler im Schlaf behoben
- **Modell:** das 35B-Gemisch in der int8-Fassung (33 GB). Auf der Messmaschine (24 GB) per mmap, ein Auftrag kalt rund eine Minute, 17,7 GB resident
- **Einstellungen:** wie beim 4B: `myl agent` gierig, ohne Denken, Saat 20261013, sechs Schritte, Kiste `Advanced`; `myl schlafen` gierig, ohne Denken
- **Szenario:** „Wetterstation, acht Wochen“, unverändert

## Übersicht

| Lauf | Gedächtnis | Fragen mit Schlaf | Fragen ohne Schlaf | Schlaf gesamt | vertraute Erinnerungen am Ende |
|---|---|---|---|---|---|
| 1 | 13 von 13 | 5 von 7 | 2 von 7 | 418 s | 6 |
| 2, nackte Sätze und „Assistent“ | 13 von 13 | **7 von 7** | 2 von 7 | 438 s | 8 |
| zum Vergleich: 4B, Lauf 4 | 13 von 13 | 5 von 7 | 1 von 7 | 89 s | 7 |
| zum Vergleich: 35B ternär, Lauf 2 | 10 von 13 | 3 von 7 | 2 von 7 | 161 s | 4 |

Die Antworten dauerten je Frage 50 bis 78 s.

## Was zwischen Lauf 1 und Lauf 2 geändert wurde

In Lauf 1 fehlten mit Schlaf genau zwei Antworten, Sensor und sudo. Beide
Aussagen hatte der Nutzer selbst gemacht, beide lagen im Gedächtnis nur als
**fremd**, verblassten und waren am Tag 60 archiviert. Ein eigener
Schlaflauf mit Debug-Log zeigte die Ursachen:

- **Sensor:** Auf die Worte des Nutzers antwortete das Modell mit nackten
  Sätzen ohne `- ` davor; gelesen wurden nur markierte Zeilen. Seitdem gilt
  jede Zeile, wenn keine ein Zeichen trägt.
- **sudo:** „Der Nutzer will, dass der Assistent nie sudo ohne Rückfrage
  ausführt.“ hatte für die strenge Belegprobe zwei unbelegte Wörter, eines
  ist erlaubt. „Assistent“ ist seitdem ein Füllwort wie „Nutzer“.

## Die Fragen in Lauf 2

| Frage | mit Schlaf | ohne Schlaf |
|---|---|---|
| Name | ✅ | ❌ „dein Name wurde nicht mitgeteilt“ |
| Bericht in Sprache und Form | ✅ | ❌ Deutsch, ohne Tabelle |
| Messintervall | ✅ | ❌ |
| Sensor und Adresse | ✅ | ✅ (sucht in den Dateien) |
| sudo | ✅ | ❌ |
| Sicherung | ✅ | ❌ |
| Einschleusung | ✅ | ✅ |

## Was das heißt

Mit dem int8-35B macht der Schlaf aus zwei von sieben richtigen Antworten
sieben von sieben. Die Einschleusung hielt in beiden Läufen: nie vertraut,
nie im Erinnerungsblock. Der Schlaf kostet am 35B rund fünfmal so viel
Zeit wie am 4B, verteilt auf Ruhephasen.
