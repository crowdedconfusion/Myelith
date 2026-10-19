# MODULES: Erweiterungen der Konsole

Ein **Modul** erweitert die Konsole (`myelith`) um eigene Befehle, einen
eigenen Modus mit Fußzeile und Bereich unter dem Eingaberahmen, Fragen an
das geladene Modell und Agentenläufe mit eigener Werkzeugkiste. Die Konsole
baut und läuft ohne jedes Modul; dann gibt es keinen seiner Befehle, weder
in `/help` noch in der Vervollständigung.

**Im Fenster** (`myl-oberflaeche`) gehen dieselben Module, nur **gewählt
statt getippt**: In der Seitenleiste unter der Werkzeugkiste gewählt,
startet ein Modul, und seine Befehle öffnet ein Pfeil ganz rechts im Feld der Modulwahl als Liste; ein Klick schickt den Befehl. Seine Zeilen
stehen im Gespräch wie in der Konsole im Rollbereich, seine Fußzeile und sein
Bereich unten unter der Eingabe, und seine Fragen an den Menschen stellt ein
Dialog des Fensters. Was ein Modul schickt, zeigt das Fenster nur als Text.
Dazu kommt das **Sichtfeld** für Ansichten (unten), geöffnet von der Lasche
am rechten Rand.

⚑ **In der Konsole stehen die Befehle eines Moduls unter `/module`**
(Festlegung des Projektinhabers, 2026-10-19): `/module trading status`,
`/module cad`. `modul.json` und das Protokoll nennen sie ohne
(`/trading status`); die Konsole stellt `/module` davor, wenn sie zeigt, und
nimmt es weg, wenn sie weitergibt. Die alte Form (`/trading status`) nennt
die neue. ⛔️ **Im Fenster gibt es keine /Befehle**: Was getippt wird, geht an
das Modell; die Befehle eines Moduls stehen in der Liste am Modul, ohne
Schrägstrich (`status`, der Hauptbefehl unter dem Namen des Moduls). Nennt
ein Modul einen eigenen Befehl in seinem Text, nimmt es
`myl_module::gast::befehl_beim_wirt`; welcher Wirt es ist, sagt der
Handschlag (`ansicht`: das Fenster ja, die Konsole nein).

Module liegen hier, je eines in einem eigenen Ordner (`MODULES/<name>/`),
oder im Ordner `module/` neben den Einstellungen der Konsole. `/module`
zeigt, welche gefunden und geprüft wurden.

## Was ein Modul ist

Ein Ordner mit drei Dingen:

| Datei | was sie trägt |
|---|---|
| `modul.json` | die Beschreibung: Name, Version, Protokoll, Programm, Befehle, Befugnisse und die SHA-256 jeder Datei des Moduls |
| `modul.sig` | die Signatur (Ed25519) über die Beschreibung |
| das Programm | was die Konsole startet; es spricht mit ihr in JSON-Zeilen über die Standardein- und ausgabe |

Dazu, was das Modul sonst braucht: Quellen, Anleitungen, eine Werkzeugkiste,
Skills, und Ordner, deren Inhalt sich im Betrieb ändert (Daten, Zustand).

## Die Beschreibung (`modul.json`)

```json
{
  "name": "beispiel",
  "version": "0.1.0",
  "protokoll": 1,
  "was": "Ein Satz für /module.",
  "laufzeit": {"art": "nativ", "programme": {"aarch64-macos": "bin/beispiel"}},
  "befehle": [
    {"name": "/beispiel", "was": "startet den Modus"},
    {"name": "/beispiel status", "was": "zeigt den Stand"}
  ],
  "befugnisse": {"modell": true, "agent": false, "unten": 12, "fusszeile": true},
  "kiste": "werkzeuge",
  "skills": "skills",
  "arbeit": "daten/arbeit",
  "veraenderlich": ["daten"],
  "dateien": {}
}
```

- **Streng gelesen**: Ein unbekanntes Feld ist ein Fehler. Pfade sind
  relativ zum Modulordner und können ihn nicht verlassen.
- **Befehle**: Der erste ist der Hauptbefehl, alle weiteren beginnen mit
  ihm. Ein Modul kann keinen Befehl der Konsole belegen: Seine stehen dort
  unter `/module`, und ein Hauptbefehl `/install` wird nicht gestartet,
  denn `/module install` gehört der Konsole. Ein Name gilt nur einmal.
- **Befugnisse**: Was nicht genannt ist, ist verboten. `modell` (das
  geladene Modell fragen), `agent` (Agentenläufe mit der eigenen Kiste),
  `netzmodell` (über das Netz statt am örtlichen Modell), `unten` (Zeilen
  unter dem Rahmen, höchstens 24), `fusszeile`, `http` (erreichbare
  Rechner für abgeschottete Module), `modell_token_je_stunde`, `ansicht`
  (Ansichten im Sichtfeld des Fensters).
- **Veränderlich**: Ordner, deren Inhalt nicht signiert ist. Programm,
  Werkzeugkiste und Skills liegen nie darin; der Arbeitsordner (`arbeit`)
  eines Agentenlaufs immer.
- **`dateien`** füllt das Signieren aus; von Hand bleibt es leer.

## Ein Modul wählen

Ein Modul **ergänzt** die Werkzeugkiste, es ersetzt sie nicht: Seine Kiste
liegt über Base und der gewählten Kiste, seine Skills kommen zu denen des
Arbeitsordners dazu, und seine Werkzeuge laufen mit der geprüften Kopie
seines Programms (`MYL_MODUL_PROGRAMM`). Werkzeuge, die seine `kiste.json`
unter `ohne_nachfrage` nennt, fragen im `manual mode` nicht nach.

Gewählt wird es in der Einstellung `agent.modul`: im Fenster in der
Seitenleiste unter der Werkzeugkiste (im Agentenmodus), in der Konsole und
auf der Kommandozeile mit `myl setzen agent.modul <name>` (`aus` heißt
keines). `myl module` zeigt, welche Module geprüft sind und was das
eingestellte mitbringt. Geprüft wird vor jedem Lauf; ein Modul, das die
Prüfung nicht besteht, bringt nichts mit, und der Lauf sagt es. Im Modus
eines Moduls in der Konsole gilt dessen Kiste genauso als Ergänzung.

Im Fenster **startet die Wahl das Modul**, und zwar nur mit dem Handschlag,
ohne Befehl: Was ein Modul erst auf einen Befehl hin tut, löst die Wahl
allein nicht aus. Jedes andere laufende Modul endet dabei; „Kein Modul“
beendet alle. Ein gewähltes Modul startet auch beim Öffnen des Fensters.

## Ansichten

Ein Modul mit der Befugnis `ansicht` kann dem Fenster Darstellungen
schicken, die dort im Sichtfeld stehen:

```json
{"art": "ansicht", "ansicht": {"id": "kurs", "titel": "Kurs",
  "inhalt": {"form": "kurve", "x": "min", "y": "EUR", "nachkomma": 2,
             "reihen": [{"name": "Kurs", "punkte": [[0, 10512], [1, 10498]]}]}}}
{"art": "ansicht_weg", "id": "kurs"}
```

| Form | Inhalt | Grenzen |
|---|---|---|
| `kurve` | Reihen von Punkten `[x, y]`, ganzzahlig; ein y-Wert ist `zahl / 10^nachkomma` | 1 bis 8 Reihen, je 10 000 Punkte, höchstens 9 Nachkommastellen |
| `tabelle` | `kopf` und `zeilen` aus Text | 32 Spalten, 2000 Zeilen |
| `bild` | `datei`: PNG, JPEG, GIF oder WebP im Modulordner | 64 MiB |
| `koerper` | `datei`: STL (binär oder Text) im Modulordner, drehbar | 64 MiB |

- **Gleiche `id` ersetzt** eine Ansicht an ihrer Stelle; so wird aus einer
  Kurve, die jeder Takt neu schickt, eine, die mitläuft.
- **Beschrieben, nicht gezeichnet**: Ein Modul schickt kein Markup, kein
  Skript und keine Adresse; das Fenster zeichnet selbst, in seinen Farben.
  Eine Datei liest das Fenster nur aus dem Ordner des geprüften Moduls und
  nicht über einen symbolischen Verweis hinaus. SVG gibt es nicht, denn es
  kann Skript tragen.
- **Über eine Grenze wird abgelehnt, nicht gekürzt**, damit ein Modul nicht
  eine halbe Kurve für die ganze hält.
- **Der Handschlag sagt, ob der Wirt Ansichten zeigt** (`"ansicht": true` in
  `hallo`): das Fenster ja, die Konsole nein. Die Konsole zeigt für eine
  Ansicht eine Zeile, dass sie nur im Fenster zu sehen ist.

## Signieren und Vertrauen

```sh
myl-module schluessel <datei>                 # ein Schlüsselpaar; geheim in <datei>, öffentlich auf stdout
myl-module signieren MODULES/beispiel <datei> # Prüfsummen eintragen, signieren
myl-module pruefen MODULES/beispiel <vertrauen.json>
```

⛔️ **Der geheime Schlüssel gehört nie ins Repositorium** und nie in einen
Modulordner.

Die Konsole startet nur Module, deren Signatur zu einem Schlüssel ihrer
**Vertrauensliste** passt (`vertrauen.json` neben den Einstellungen):

```json
{
  "schluessel": [
    {"name": "Wer", "oeffentlich": "<64 Hex-Zeichen>", "nativ": false, "widerrufen": false}
  ]
}
```

Abgelehnt wird ein Modul ohne Signatur, mit fremdem oder widerrufenem
Schlüssel, mit einer veränderten, fehlenden oder dazugelegten Datei, mit
einem symbolischen Verweis, und in einer kleineren Version als der zuletzt
gestarteten. Geprüft wird beim Start der Konsole und vor jedem Start eines
Moduls, an genau den Bytes, die danach laufen: Die Konsole führt eine eigene
Kopie aus. Ändert sich die Signatur eines laufenden Moduls, startet die
Konsole es nach erneuter Prüfung neu.

## Installieren

```
/module install <ordner>
```

prüft das Modul, kopiert es (ohne den Inhalt veränderlicher Ordner) in den
Ordner `module/` neben den Einstellungen und prüft die Kopie.

## Was die Prüfung schützt, und was nicht

- Sie schützt davor, dass ein Modul unterwegs oder auf der Platte verändert,
  auf eine alte Fassung zurückgesetzt oder mit fremden Dateien gemischt
  wird, und davor, dass ein Modul über die Konsole mehr tut, als seine
  Befugnisse nennen: Text eines Moduls kommt ohne Steuerzeichen an, Fragen
  an den Menschen stellt die Konsole in ihrem eigenen Rahmen.
- ⛔️ **Ein natives Modul ist keine Sicherheitsgrenze.** Es ist ein Programm
  mit den Rechten des Nutzers und kann auf dem Rechner tun, was er kann.
  Deshalb startet ein natives Modul nur mit einem Schlüssel, der in der
  Vertrauensliste ausdrücklich als `nativ` markiert ist. **Installieren
  heißt vertrauen.**
- Module, die abgeschottet laufen (WebAssembly, ohne eigenes Dateisystem
  und ohne eigenes Netz), folgen; die Beschreibung kennt sie schon
  (`"laufzeit": {"art": "wasm", "datei": "…"}`).
