# Werkzeugkiste: Advanced

⚑ **Das Format einer Werkzeugdatei steht eine Ebene höher**, in
`../README.md`, für alle Kisten gemeinsam. Hier steht, was **in dieser
Kiste** liegt.

## Was hier liegt

| Werkzeug | wofür |
|---|---|
| `git_verlauf` | die letzten Commits, je einer pro Zeile |
| `platzbedarf` | wieviel Platz ein Pfad belegt |

⚑ **Diese Kiste erbt alles aus `Base`.** Die Werkzeuge dort werden
mitgeladen, ohne dass eine Datei doppelt liegt.

Eingebaut kommen dazu, was `Base` nicht bekommt: `run_command`,
`read_history`, `list_history`, `search_history`, `list_skills`,
`read_skill`.
