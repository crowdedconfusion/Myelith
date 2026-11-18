# Werkzeugkisten: das Format, einmal

⚑ **`local-` heisst: nur auf diesem Rechner.** Ein Werkzeug aus diesen
Kisten wirkt auf dem Rechner des Nutzers und ist nie Teil einer
prüfbaren Netzaktion; die Netzseite des Agenten liegt in `myl-agent`.
Bis zum 2026-09-30 lag dieser Ordner unter `CLIENT/werkzeugkisten/`;
eine gespeicherte Einstellung mit dem alten Pfad zieht der Client beim
Lesen nach.

Ein Werkzeug ist eine JSON-Datei in einem dieser Ordner, die der Agent
**zur Laufzeit** liest; nichts muss neu gebaut werden. Sobald eine Datei
hier liegt, sieht das Modell das Werkzeug.

⚑ **Dieser Text steht hier und nicht in jeder Kiste.** Er stand einmal
in `Base` und noch einmal in `Advanced`, und als das Format um drei
Felder wuchs, waren es zwei Orte, von denen sich der zweite nicht
meldet. Die Kisten beschreiben jetzt nur noch ihren **Inhalt**.

## Die Kisten

| Ordner | wofür |
|---|---|
| `Base` | die Grundlage, in jeder anderen Kiste enthalten |
| `Advanced` | mehr Werkzeuge, dazu das eingebaute `run_command` |
| `GolemOS` | wie `Advanced`, dazu Oberfläche und Knoten auf GolemOS (`golem-schicht`, `golem-knoten`) |

⚑ **Gestapelt, nicht kopiert.** Der gewählte Ordner kommt **nach**
`Base`, und bei gleichem Werkzeugnamen gewinnt der gewählte. Wer ein
Base-Werkzeug ersetzen will, legt eines mit demselben Namen in seine
Kiste. Dieselbe Manifestdatei in zwei Ordnern wären zwei Orte, die
auseinanderlaufen.

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

| Feld | Pflicht | wofür |
|---|---|---|
| `name` | ja | wie das Modell es ruft, `snake_case`, **nicht** der Dateiname |
| `beschreibung` | ja | wofür es da ist, für die Ansage an das Modell |
| `parameter` | ja | JSON-Schema des Arguments, wie bei den eingebauten |
| `befehl` | ja | die Vorlage, siehe unten |
| `zeitgrenze_s` | nein | eigene Frist; Vorgabe 30 s, Deckel 300 s |
| `fuer` | nein | welche Art Anhang es lesbar macht: `bild`, `ton`, `text`, `sonstiges` |
| `fluss` | nein | was es für den Informationsfluss ist; ohne Angabe eine Shell (siehe unten) |
| `sandbox` | nein | unter welcher Grenze der Befehl läuft: `eng`, `netz` oder `frei` (siehe unten; früher `sandkasten`, wird weiter gelesen) |
| `schreibt` | nein | Ordner außerhalb des Arbeitsordners, relativ zum Manifest, in die es eine Ausgabe legt |

- `befehl` ist eine Vorlage. `{feld}` wird durch das Argument `feld`
  ersetzt, **shell-sicher zitiert** (kein Einschleusen einer zweiten
  Kommandozeile). `{{` und `}}` stehen für echte geschweifte Klammern.
  Ein Platzhalter ohne Argument ist ein Fehler und kein leeres
  Einsetzen.
- Ausgeführt wird mit `sh -c` im Arbeitsverzeichnis, mit Ausgabegrenze
  (16 KiB) und Frist, wie `run_command`.
- **`$MYL_KISTE` zeigt auf den Ordner, in dem das Manifest liegt.** So
  ruft ein Werkzeug ein Skript neben sich auf:
  `sh "$MYL_KISTE/tue_etwas.sh" {datei}`. Ein absoluter Pfad im Manifest
  wäre auf jeder anderen Maschine falsch, ein relativer zeigte in den
  Arbeitsordner, wo das Skript nicht liegt.

### `zeitgrenze_s`: wann sie gebraucht wird

Die Vorgabe von 30 s reicht für `wc` und `grep`. Sie reicht **nicht**
für ein Werkzeug, das ein Modell von der Platte lädt: Ein kalter Start
dauert länger als die ganze Frist, und der Abbruch sähe wie ein kaputtes
Werkzeug aus. Nach oben schließen 300 s ab, denn ein Werkzeug, das nicht
zurückkommt, hält die Schleife an.

### `fuer`: sich für eine Art Anhang melden

Wird eine Datei angehängt, bekommt das Gespräch eine Zeile, die sagt, wo
sie liegt. Steht in der Kette ein Werkzeug, dessen `fuer` die Art dieser
Datei nennt, **nennt die Zeile es beim Namen**. ⚑ Der Name steht damit
im Manifest und nicht im Quelltext des Clients.

⚑ **Bild und Ton macht der Client selbst** (`CLIENT/myl-senses`), und
zwar auch im Chat, wo es gar keine Werkzeuge gibt; dafür ist `fuer` also
nicht nötig. Es ist die Tür für **alles andere**: wer eine Tabelle, ein
PDF oder ein Fremdformat lesbar machen will, trägt hier
`"fuer": ["sonstiges"]` ein und wird genauso genannt.

### `fluss`: was ein Werkzeug für den Informationsfluss ist

Der Agent führt zu jedem Gespräch ein Label: ob Privates darin steht und ob
Fremdes (eine Webseite, ein Anhang). Jedes Werkzeug sagt, was es beiträgt
und wohin seine Argumente gehen:

```json
"fluss": {"liefert": "oeffentlich", "vertrauen": "fremd", "ziel": "welt", "braucht_vertrauen": false}
```

| Feld | Werte | heißt |
|---|---|---|
| `liefert` | `privat`, `oeffentlich` | wer sein Ergebnis lesen darf |
| `vertrauen` | `vertraut`, `fremd` | wie sehr seinem Ergebnis zu trauen ist |
| `ziel` | `keins`, `lokal`, `welt` | wohin seine Argumente gehen |
| `braucht_vertrauen` | `true` (Vorgabe), `false` | ob es ein Gespräch ohne Fremdes oder die Freigabe des Menschen braucht |

⚑ **Ohne Angabe gilt ein Manifest als Shell**: Ergebnis privat und
vertraut, Ziel lokal, und es braucht Vertrauen, denn es läuft über `sh -c`
und kennt die Einhängegrenze nicht. Steht Fremdes im Gespräch, läuft es im
manual mode erst nach der Nachfrage und im auto mode gar nicht. Ein Werkzeug
mit `ziel: welt` läuft, solange Privates im Gespräch steht, nur, wenn seine
Argumente kein wörtliches Zitat daraus tragen. Eine Angabe ist ein
Versprechen der Kiste: Wer `braucht_vertrauen: false` schreibt, sagt, dass
das Werkzeug auch auf Anweisung fremden Textes nichts anrichten kann.

⛔️ **Ein Werkzeug, das Text von Dritten liefert, sagt `"vertrauen": "fremd"`**,
auch wenn es nur lokal liest: Commit-Nachrichten eines geklonten Projekts
(`git_verlauf` tut das seit dem 2026-10-14, Fund 540), Mails, Kurse eines
Marktplatzes, eine heruntergeladene Datei. Ohne die Angabe käme dieser Text
als vertraut ins Gespräch, und danach liefe die Shell ohne Freigabe. Ein
fremdes Ergebnis bekommt vorn die Marke `[Untrusted origin: …]`; sie sagt
dem Modell, dass der Text Auskunft ist und kein Auftrag, und sie trägt die
Herkunft in die nächste Runde des Gesprächs.

### `sandbox` und `schreibt`: was der Befehl kann

Jeder Befehl läuft in der Sandbox des Betriebssystems (Seatbelt unter macOS,
Landlock und seccomp unter Linux), solange `agent.sandbox` an ist.

| Art | schreiben | lesen | Netz |
|---|---|---|---|
| `eng` (Vorgabe) | Arbeitsordner, eigener `$TMPDIR` | System, Arbeitsordner, Ordner der Kiste; nicht die übrige Heimat | nein |
| `netz` | ebenso | ebenso | ja, ohne Unix-Sockel |
| `frei` | alles, was du darfst | alles | ja |

`"sandbox": "netz"` braucht ein Werkzeug, das etwas abruft; `"frei"` eines,
das das System verwaltet (die GolemOS-Kiste). `"schreibt": ["../vorschau"]`
öffnet einen Ausgabeordner neben der Kiste, etwa für eine Vorschau, die ein
anderes Programm abholt; die Kiste selbst und ein Ordner über ihr gehen nicht.

⛔️ **`frei` und `schreibt` gelten nur für eine Kiste außerhalb des
Arbeitsordners.** Im Arbeitsordner kann der Agent schreiben, also auch ein
Manifest, das sich selbst befreit; dort läuft es eng. Der Ordner der Kiste ist
für ihre Befehle lesbar und nie beschreibbar.

⚑ Die Kiste eines geprüften nativen Moduls ist ohne Angabe `frei`: Ihre
Werkzeuge starten das Programm des Moduls, und das läuft nach seiner Signatur
ohne Abschottung. Eine neue Fassung des Moduls kann ihren Manifesten eine
Grenze geben, dann gilt diese.

## `kiste.json`: was die Kiste über sich sagt

⚑ **Der einzige reservierte Dateiname.** Er beschreibt die Kiste und
nicht ein Werkzeug, wird also nicht als Manifest gelesen.

```json
{ "eingebaute": "Advanced" }
```

`eingebaute` sagt, welche **kompilierten** Werkzeuge zu dieser Kiste
gehören: `Base` oder `Advanced`. Ohne die Datei entscheidet der
Ordnername, und der kennt nur diese Wörter: Ein eigener Ordner fiel
sonst **stillschweigend** auf `Base` zurück und verlor unter anderem die
Suche im Mitschnitt.

## Die eingebauten Werkzeuge

Sie sind kompiliert, halten die Einhängegrenze ein und kommen zu jeder
Kiste dazu. Was in den Ordnern liegt, kommt obendrauf.

| Kiste | eingebaut |
|---|---|
| `Base` | `list_directory`, `read_file`, `search_files`, `write_file`, `edit_file`, `search_skill`, `learn_skill` |
| `Advanced` | dazu `run_command`, `read_history`, `list_history`, `search_history` |

⚑ **Die zwei Skillwerkzeuge liegen seit dem 2026-09-26 in `Base`**: Das
Modell soll selbst nach einem Skill suchen, wenn es nicht weiterweiß,
und auf „lerne skill …“ einen lernen. Die Skills selbst liegen unter
`AGENT_LAYER/local-skills/`, unter den eigenen neben den Einstellungen und im
Projekt unter `.AGENT/skills/`.

⛔️ **Warum die Dateiwerkzeuge nicht als Manifest in `Base` liegen.** Ein Manifest
läuft über `sh -c` und hält die Einhängegrenze **nicht** ein; die
kompilierten tun es. Sie dorthin zu verlegen sähe aufgeräumter aus und
nähme dem Agenten seine Grenze.

## ⛔️ Sicherheit

Ein Werkzeug aus einer Kiste führt einen Shell-Befehl aus; die
Einhängegrenze der Dateiwerkzeuge hält eine Shell **nicht** ein. Die
Grenze zieht die Sandbox (oben), und ist er aus oder das Werkzeug
`frei`, kann der Befehl alles, was du kannst. Deshalb:

- Es braucht die **Schreiberlaubnis** (`agent.schreiben an`), auch ein
  scheinbar nur lesendes: Die Shell kann immer schreiben. Ohne sie steht
  keines im Angebot.
- Im `manual mode` wird jeder Aufruf vorgelegt.
- Lege nur Werkzeuge ab, deren Befehl du gelesen und verstanden hast.
