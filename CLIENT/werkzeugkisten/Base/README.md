# Werkzeugkiste: Base

⚑ **Das Format einer Werkzeugdatei steht eine Ebene höher**, in
`../README.md`, für alle Kisten gemeinsam. Hier steht, was **in dieser
Kiste** liegt.

## Was hier liegt

| Werkzeug | wofür |
|---|---|
| `dateibaum` | der Verzeichnisbaum ab einem Pfad |
| `git_stand` | der Arbeitsstand des Repositoriums im Arbeitsordner |
| `suche_text` | Textsuche über die Dateien im Arbeitsordner |
| `zaehle_zeilen` | die Zeilen einer Datei |

Dazu kommen die eingebauten `list_directory`, `read_file`,
`search_files`, `write_file`, `edit_file`.

## ⚑ Base ist die Grundlage jeder anderen Kiste

Was hier liegt, bekommt auch `Advanced` und jede selbst
gewählte Kiste; gestapelt, nicht kopiert. Wer ein Werkzeug von hier
ersetzen will, legt eines mit demselben Namen in seine eigene Kiste.

⚠️ **Deshalb ist diese Kiste der falsche Ort für Besonderes.** Jede
Datei hier kostet jeden Lauf ein paar Zeilen Ansage, auch den des
kleinsten Modells. Was nicht fast immer gebraucht wird, gehört in eine
andere Kiste.
