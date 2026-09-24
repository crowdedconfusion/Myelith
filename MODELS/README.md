# MODELS/

**Der eine Ablageort für fremde Gewichte.** Alles, was dieses Projekt
nicht selbst baut, sondern von außen lädt, liegt hier, nach Rubrik
sortiert. Was neu dazukommt, kommt ebenfalls hierher und in die Rubrik,
zu der es gehört.

| Rubrik | Was hineingehört | Wer es liest |
|---|---|---|
| `llm/` | Die Quellmodelle, aus denen die Ganzzahl-Artefakte entstehen | der Artefaktbau |
| `audio/` | Hören und Sprechen | die Sinne des Clients |
| `vision/` | Sehen | die Sinne des Clients |

## ⚑ Die Gewichte sind nicht versioniert, der Ablageort ist es

Sie wiegen Gigabyte, sie sind je System verschieden, und sie kommen von
ihrer Quelle. Versioniert ist deshalb nur, was einen Leser weiterbringt:
dieses README, `llm/KATALOG.json`, das daraus erzeugte `llm/README.md`
und die **Lizenzdatei** jedes empfohlenen Quellmodells. Die Regeln dazu
stehen in `.gitignore` daneben, je mit ihrem Grund.

⚠️ **`audio/` und `vision/` halten heute nur eine Marke.** Git kann kein
leeres Verzeichnis versionieren, und ein Ordner, der erst beim Einrichten
entsteht, ist kein Ablageort, sondern eine Nebenwirkung.

## Wie die Gewichte hereinkommen

| Rubrik | Weg |
|---|---|
| `llm/` | `sh INTEGER_LLM/scripts/fetch_model.sh <Modell-ID>`, mit fixierter Revision. Herkunft und Revision je Modell in `llm/README.md` |
| `audio/`, `vision/` | `sh SYSTEM/install/sinne-einrichten.sh`. Es lädt, legt ab und prüft nach |

⚑ **Die Dateinamen stehen nicht hier.** Unter welchen Namen die Sinne
ihre Gewichte erwarten, sagt `CLIENT/myl-senses/src/laufwerk.rs`, und
das Einrichtungsskript legt sie genau so an. Eine zweite Liste hier wäre
eine, die auseinanderläuft, sobald ein Name sich ändert.

## Wo gesucht wird, und in welcher Reihenfolge

Die Sinne des Clients nehmen den ersten Ort, an dem etwas liegt:

1. **`MYL_SINNE`**, falls gesetzt. Wer es setzt, meint es, und dann
   liegt dort alles zusammen.
2. **Dieses Verzeichnis**, sobald ein Klon gefunden wird.
3. **`~/.myelith/sinne`** als Rückfall. ⚑ **Ohne ihn liefe ein
   installiertes Programm ins Leere**, denn wer nur die Freigabebündel
   geholt hat, hat keinen Klon und damit kein `MODELS/`.

Einzelne Dateien lassen sich unabhängig davon umstellen; die Namen der
Variablen dafür stehen bei den Sinnen.

## Was hier ausdrücklich **nicht** liegt

- **Die gebauten Artefakte.** Sie entstehen aus `llm/` und liegen unter
  `INTEGER_LLM/artifacts/`, mit ihrer eigenen Modellkarte. ⚑ **Der
  Unterschied ist keine Ordnungsfrage:** Ein Quellmodell ist geladen und
  trägt die Lizenz seiner Quelle, ein Artefakt ist hier gebaut und trägt
  die dieses Repositoriums.
- **Eigene Skripte und Aufnahmen.** Das Sprech- und Aufnahmeskript und
  eine abgelegte Stimmprobe gehören dem Betrieb und nicht den Gewichten;
  sie bleiben unter `~/.myelith/sinne`.
- **Programme.** llama.cpp, whisper.cpp und ffmpeg sind Fremdprogramme
  und werden im Pfad gesucht, nicht hier abgelegt.

⚠️ **Eine Ausnahme, die benannt gehört:** Unter `audio/` kann ein
vollständiges Sprechprojekt samt seiner Python-Umgebung liegen, weil
seine Gewichte ohne sie nicht zu benutzen sind. Es ist die einzige
fremde Laufzeit in diesem Baum, und sie ist als Übergang gedacht.
