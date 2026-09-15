# Werkzeugkiste: Advanced


⚑ **Diese Kiste erbt alles aus `Base`.** Die Werkzeuge dort werden
mitgeladen, ohne dass eine Datei doppelt liegt; hier steht nur, was
`Advanced` **zusaetzlich** hat. Dazu kommt das eingebaute
`run_command`, das `Base` nicht bekommt.

Ein Werkzeug ist hier eine JSON-Datei, die der Agent **zur Laufzeit** liest;
nichts muss neu gebaut werden. Sobald eine Datei hier liegt, sieht das
Modell das Werkzeug.

⚑ **Die eingebauten Dateiwerkzeuge** (`list_directory`, `read_file`,
`search`, `write_file`, `edit_file`) kommen ohnehin dazu; sie sind
kompiliert und halten die Einhaengegrenze ein. Was hier liegt, kommt
obendrauf.

## Format einer Werkzeugdatei

```json
{
  "name": "zaehle_zeilen",
  "beschreibung": "Zaehlt die Zeilen einer Datei im Arbeitsverzeichnis.",
  "parameter": {
    "type": "object",
    "properties": { "datei": { "type": "string", "description": "Pfad im Arbeitsverzeichnis" } },
    "required": ["datei"]
  },
  "befehl": "wc -l {datei}"
}
```

- `befehl` ist eine Vorlage. `{feld}` wird durch das Argument `feld`
  ersetzt, **shell-sicher zitiert** (kein Einschleusen einer zweiten
  Kommandozeile). `{{` und `}}` stehen fuer echte geschweifte Klammern.
- Ausgefuehrt wird mit `sh -c` im Arbeitsverzeichnis, mit Zeitgrenze (30 s)
  und Ausgabegrenze (16 KiB), wie `run_command`.

## ⛔️ Sicherheit

Ein Werkzeug hier fuehrt einen Shell-Befehl aus und kann alles, was der
Prozess kann; es haelt die Einhaengegrenze **nicht** ein. Deshalb:

- Es braucht die **Schreiberlaubnis** (`agent.schreiben an`), auch ein
  scheinbar nur lesendes: Die Shell kann immer schreiben.
- Im `manual mode` wird jeder Aufruf vorgelegt.
- Lege hier nur Werkzeuge ab, deren Befehl du gelesen und verstanden hast.
