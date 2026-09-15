# Werkzeugkiste: Base

Ein Werkzeug ist hier eine JSON-Datei, die der Agent **zur Laufzeit** liest;
nichts muss neu gebaut werden. Sobald eine Datei hier liegt, sieht das
Modell das Werkzeug.

⚑ **Die eingebauten Dateiwerkzeuge** (`list_directory`, `read_file`,
`search_files`, `write_file`, `edit_file`) kommen ohnehin dazu; sie sind
kompiliert und halten die Einhaengegrenze ein. Was hier liegt, kommt
obendrauf.

⛔️ **Warum die fuenf nicht als Manifest hier liegen.** Ein Manifest
laeuft ueber `sh -c` und haelt die Einhaengegrenze **nicht** ein; die
kompilierten tun es. Sie hierher zu verlegen saehe aufgeraeumter aus und
naehme dem Agenten seine Grenze. Deshalb bleiben sie kompiliert, und
dieser Ordner enthaelt, was darueber hinausgeht.

⚑ **Base ist die Grundlage jeder anderen Kiste.** Was hier liegt, bekommt
auch `Advanced` und jede selbst gewaehlte Kiste; gestapelt, nicht
kopiert. Wer ein Werkzeug von hier ersetzen will, legt eines mit
demselben Namen in seine eigene Kiste.

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
