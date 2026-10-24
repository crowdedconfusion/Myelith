# client (Nutzer-Client inkl. Wallet)

> **Version:** 0.178.4 (`myl-client` 0.114.1, `myl-oberflaeche` 0.85.2, `myl-console` 0.69.1, `myl-senses` 0.13.5, `myl-module` 0.5.2)
> **Datum:** 2026-10-23
> **Status:** ✅ **Der lokale Betrieb läuft und ist ausgeliefert.** Ein
> Gesprächsfenster mit Modellwahl, Agentenschleife und
> Einstellungsseite; aus einem frischen Klon lassen sich darüber
> Modelle laden und Artefakte bauen. Freigabebündel für macOS, Windows
> und Linux, jedes mit Prüfsumme.
>
> ❌ **Die Netzhälfte steht aus.** Knotenzustand, Läufe, Verifikations­
> stufe je Segment und der Kapazitätsschalter warten auf ein
> erreichbares Netz (Punkte 2.2 bis 2.5b).

Die einzige Komponente des Projekts, mit der Menschen tatsächlich
interagieren: MYL-Wallet, Inferenz-Schnittstelle,
Session-Kontrakt-Verwaltung für Agenten-Nutzung, Staking- und
Governance-Ansicht für Miner und Validatoren.

📌 **Dieser Kopf stand bis zum 2026-09-09 auf „Konzeptphase, noch keine
Umsetzung"**, während zwei Kisten mit sechzehn Prüfungen, einer
Oberfläche und Freigabebündeln dastanden. Ein Statuseintrag, der aus
einer überholten Annahme stammt, schickt den Nächsten in die Irre; er
kostet nichts, wenn er stimmt, und einen halben Tag, wenn nicht.

## Was heute geht

| | |
|---|---|
| **Der Agent in der Konsole** | In ein Verzeichnis gehen, `myelith` tippen. **Dieses Verzeichnis ist der Arbeitsordner**, ohne Schalter und ohne Einstellung; `/model`, `/settings` (mit den Pfeiltasten bedienbar), `/help`, `/exit`. Beim Start werden Design und Modell mit den Pfeiltasten gewählt. Die Eingabezeile steht **fest am unteren Rand**, die Ausgabe rollt darüber weg; darunter Modus, Modell, Werkzeugkiste, der Befehl für die Hilfe und der Arbeitsordner |
| **Fünf Konsolen-Designs** | `Standard` nimmt die Farben deines Terminals, dazu `Myelith`, `Bernstein`, `Tiefsee`, `Tinte`. Voreingestellt über `oberflaeche.design`, beim Start ohne bleibende Wirkung wählbar |
| **Was gerade läuft, steht da** | Eine Zeile fest über dem Eingabekasten, mittig: Ladetext, verstrichene Zeit, gelesene und geschriebene Token, laufende Tätigkeit; die Farbe wandert in zwei Minuten durch den Regenbogen, in einfarbigen Designs pulst sie. Jeder Schritt und jeder Werkzeugaufruf **bleibt** in der Zeitleiste; `^S` schaltet auf die ausführliche Form, in der auch der Denkvorgang live durchläuft |
| **auto mode und manual mode** | Umschalt-Tab in der Konsole, oder `agent.modus` in den Einstellungen. Im manual mode wird **jede schreibende Handlung** vorgelegt und läuft erst nach einer Bestätigung; Lesen und Suchen fragen nie. Das Fenster legt sie seit v0.47.0 im Kasten des Betriebssystems vor, mit Namen und Argumenten; vorher nahm es die schreibenden Werkzeuge in diesem Modus **ganz weg** |
| **Der Loop: ein Ziel über viele Runden** | **Ohne Grenze an Runden oder Dauer:** Ein Loop endet, wenn du ihn anhältst oder sein Ergebnis bestätigst. Meldet das Modell das Ziel als erreicht, fragt der Client, ob es stimmt oder wo weitergearbeitet werden soll; kommt das Modell ohne dich nicht weiter, stellt es dir eine Frage, und der Loop wartet auf deine Antwort. Im Fenster ∞ neben dem Senden (an und aus), daneben die Liste der Tasks mit ▶, ⏸, ✓ und Papierkorb je Task: auswählen, neu anlegen, am Griff in die gewünschte Reihenfolge ziehen. Der vorderste läuft, alle dahinter sind „queued“; jeder Task hat einen Namen und sein eigenes Gespräch. In der Konsole `/start loop [ziel]` (ohne Ziel fragt das Modell danach und benennt den Loop), `/stop loop`, `/resume loop` und `/tasks`; auf der Kommandozeile `myl loop` und `myl tasks`. Wird geschlossen, während ein Loop läuft, ist er danach angehalten und lässt sich genau dort fortsetzen |
| **Module** | Signierte Erweiterungen mit eigenen Befehlen, eigenem Modus, Fragen an Modell und Mensch und eigener Werkzeugkiste (`MODULES/README.md`). In der Konsole getippt wie jeder Befehl. Im Fenster **gewählt** in der Seitenleiste unter der Werkzeugkiste: Die Wahl startet das Modul, seine Befehle öffnet ein Pfeil ganz rechts im Feld der Wahl als Liste, seine Zeilen im Gespräch, Fußzeile und Bereich unten unter der Eingabe, und seine Fragen stellt ein Dialog des Fensters. Bilder, Kurven, Tabellen und 3D-Modelle (STL, drehbar) eines Moduls zeigt das **Sichtfeld**, geöffnet von der Lasche am rechten Rand; offen deckt es das ganze Gespräch. Das gewählte Modul ergänzt die Werkzeugkiste jedes Auftrags |
| **Ein Gespräch mit einem lokalen Modell** | `myl frage <artefakt> <text>`, oder im Fenster |
| **Die Agentenschleife** | `myl agent`, mit Werkzeugen innerhalb einer Einhängegrenze |
| **Die Werkzeugkiste ist ein Ordner** | Eine Einstellung, ein Pfad: `agent.kistenordner`, ohne Angabe die mitgelieferte Kiste `Base` unter `AGENT_LAYER/local-toolkits/`. Der **Ordnername** sagt, welche eingebauten Werkzeuge dazukommen: `Base` die fünf Dateiwerkzeuge und die zwei Skillwerkzeuge `search_skill` und `learn_skill` (seit dem 2026-09-26), `Advanced` zusätzlich `run_command` und die drei Werkzeuge für den Mitschnitt (seit dem 2026-09-17, gemessen: das kleine Modell ruft sie nie). ⚑ **Eine Kiste kann das auch selbst sagen**, in ihrem `kiste.json`; ohne diese Datei und ohne einen der drei Namen bleibt es bei `Base`. Was als Manifest im Ordner liegt, sieht das Modell **ohne Neubau**. ⚑ **`Base` ist die Grundlage jeder Kiste**: Ihre Werkzeuge werden mitgeladen, gestapelt und nicht kopiert; bei gleichem Namen gewinnt die gewählte Kiste. ⛔️ Die fünf Dateiwerkzeuge bleiben kompiliert, weil nur sie die Einhängegrenze einhalten; ein Manifest läuft über die Shell und kann das nicht |
| **Eine Datei anhängen** | In der Konsole `/datei <pfad>`, im Fenster der Knopf neben dem Senden oder Ziehen und Ablegen. Die Datei wandert nach `.AGENT/anhaenge/`, also unter die Einhängung, und das Gespräch bekommt **eine Zeile mit ihrem Pfad statt ihres Inhalts**. Gibt es ein Werkzeug für ihre Art, nennt die Zeile es. `myl anhaenge` zeigt, was liegt, `--aufraeumen` räumt auf |
| **Sehen, Hören, Sprechen** | Die Kiste `myl-senses`: Bilder über llama.cpp, Ton über whisper.cpp, Sprechen über piper, jeweils mit einem **eigenen kleinen** Modell außerhalb des Repositoriums (`~/.myelith/sinne`). Das Hauptmodell bleibt ein Textmodell. ⚑ **Auch im Chat**, wo es keine Werkzeugschleife gibt: Eine angehängte Datei wird beim Anhängen angesehen. Fehlt ein Laufwerk, sagt der Sinn mit Pfad und Befehl, was fehlt, statt abzustürzen |
| **Die Sprechtaste** | Im Fenster: gedrückt halten, reden, loslassen. Über dem Feld schlägt ein **Pegel** aus, solange aufgenommen wird, und danach steht der Text **in der Eingabezeile**, nicht im Gespräch: Wer sich verhört hat, bessert aus, bevor das Modell liest. Der Lautsprecherknopf liest Antworten vor, **satzweise**: Der erste Satz klingt, während das Modell noch schreibt. ⚠️ Ein Voll-Duplex-Gespräch ist das nicht, es fehlen Sprechbeginnerkennung, Unterbrechen und Echokompensation |
| **Nachsehen, ob die Sinne gehen** | `myl sinne` zeigt, was dieser Rechner sehen, hören und sprechen kann, und `myl sinne <datei>` schickt eine Datei hindurch. ⚑ **Der kürzeste Weg von der Behauptung zum Beleg.** Eingerichtet wird mit einem Skript unter `SYSTEM/install/` |
| **Der Sprachmodus** | Ein Knopf ersetzt den Verlauf durch ein Zeichen, dessen Ringe **mit dem Ton mitschwingen**; darunter steht die laufende Antwort. Wer spricht und zuhört, liest nicht mit |
| **Eine eigene Stimme** | Auf der Einstellungsseite eine Aufnahme hochladen; sie wird zur Stimme. Whisper schreibt ihren Text gleich mit, weil CosyVoice damit besser trifft. ⛔️ Liegt eine Probe und spricht ein Programm, das nicht klonen kann, **steht das da** |
| **Mehrere Stellen auf einmal ändern** | `edit_file` nimmt eine Liste aus `alt` und `neu`. Erst ein Trockenlauf über eine Kopie, dann wird geschrieben: **Entweder alle Stellen oder keine** |
| **Vier Betriebsarten** | Chat und Agent laufen; im Reiter **Knoten** (Konsole: `/mining`) wird Mining eingestellt und gestartet, heute als Knoten im Probelauf ohne Testnetz, und der Reiter atmet, solange es läuft; Wallet steht mit seiner Begründung da und wartet auf das Netz |
| **Modellwahl** | Aus dem Katalog, mit Anzeigenamen statt Verzeichnisnamen. Der Netzeintrag heisst „Netzwerkmodell (API), kostet Inferenz-Credits" und ist gesperrt, solange Knotenadresse und Vollmacht fehlen |
| **Modelle holen und Artefakte bauen** | Aus der Einstellungsseite heraus, mit Ladebalken unter dem angeklickten Modell und einer schliessbaren Meldung, wenn es fertig ist |
| **Einstellungen** | Fünf Bereiche, vierzehn Felder (dreizehn davon im Fenster: das Konsolen-Design wirkt dort nicht) in der Reihenfolge, in der jemand sucht (Sprache, Aktualisierung, Modelle, was dieser Rechner hergibt, dann Modell und Agent), und dazu ein Schieberegler je gefundenem Rechenwerk, jedes mit Beschriftung und einem Satz darunter, was es bewirkt. Art **und** Beschriftung kommen aus der Kiste. Die drei Verzeichnisfelder lassen sich über den Fensterdialog des Systems wählen, getippt werden dürfen sie weiter |
| **Sprache** | Deutsch oder Englisch, umschaltbar in den Einstellungen und sofort wirksam. Feldnamen, Pfade und Modellnamen bleiben, wie sie sind |
| **Aus einem frischen Klon einrichten** | Drei Skripte unter `SYSTEM/install/`, je eines für macOS, NixOS und Windows, mit einer Anleitung je System daneben. Sie prüfen erst, bauen dann, und laden nichts nach |
| **Nach Aktualisierungen sehen** | Auf der Einstellungsseite: Gefragt wird, ob `origin` Änderungen hat, die dieser Klon nicht hat. Eingespielt wird mit `git merge --ff-only` und dem Installationsskript der Plattform |
| **Gespraeche verwalten** | Rechtsklick auf eine Zeile: umbenennen an Ort und Stelle, als Markdown ausgeben, loeschen. Wohin ausgegeben wird, steht in `ausgabe.ordner`; ohne Angabe fuehrt das Fenster dorthin |

⚑ **Die Oberfläche ruft dieselben Funktionen wie die Kommandozeile**,
über vierundsiebzig Befehle. ⚠️ **Mit fünf Ausnahmen, und alle sind gewollt:** `terminal_ausfuehren` startet eine Shell, denn ein Terminal, das keine startet, ist keines; `modul_waehlen` und `modul_befehl` starten ein Modul, und zwar wie die Konsole nur die geprüfte Kopie eines signierten Moduls; `artefakt_bauen` startet die Bauskripte des Repositoriums über `myl_client::bezug`, dieselbe Stelle wie die Konsole, und nur nach Bestätigung; die Aktualisierung startet `curl`, `git` und das Installationsskript über `myl_client::aktualisierung`. `mining_starten` startet den Knoten (`myl-node`) und `mining_beenden` schickt ihm das Ende (`kill`), beide über `myl_client::mining`, dieselbe Stelle wie `/mining` in der Konsole. Jeder andere Befehl startet **keinen einzigen Unterprozess**. 📌 Bis zum 2026-10-16 nannte dieser Satz nur die ersten beiden, obwohl Bau und Aktualisierung schon damals Unterprozesse starteten. `jeder_befehl_ist_angemeldet` hält die vier
Richtungen zusammen: kein Befehl ohne Anmeldung, keine Anmeldung ohne
Befehl, kein Aufruf ins Leere und kein Befehl, den niemand ruft. „Ohne eigene Logik" hiesse sonst, aus einer Textausgabe
für Menschen eine Schnittstelle zu machen, und genau das ist die Sorte
Logik, die hier nicht hingehört.

## Struktur

| Verzeichnis | Zweck |
|---|---|
| `myl-client/` | Die Kiste. Einstellungen, örtlicher Betrieb (das Modell laden und fragen), Hardware, Modellkatalog, Schutzfilter und Kennzeichnung. Kommandozeile `myl`. |
| `myl-console/` | Der Agent in der Konsole: `myelith`. Startbild, Modellwahl, Eingaberahmen. **Ohne eigene Logik**, alles kommt aus `myl-client`. |
| `myl-oberflaeche/` | Die grafische Oberfläche auf Tauri v2. Rücken in Rust, Frontend als reines HTML, CSS und ES-Module: **kein Bündler, keine Node-Werkzeugkette**. |
| `myl-oberflaeche/ui/` | `index.html`, `stil.css`, `app.js`, `netz.js`. Neunundvierzig Prüfungen halten HTML, CSS, Skript und Rücken gegeneinander. |
| `myl-oberflaeche/icons/` | Symbole. `icon.ico` und `icon.icns` sind aus `icon.png` abgeleitet und liegen fertig da; von Hand nachbessern hilft nicht, die nächste Ableitung überschreibt es. |

⚑ **Der örtliche Agent liegt seit dem 2026-09-30 unter `AGENT_LAYER`**:
Werkzeuge, Werkzeugkisten, Skills, Loop, Notaus und
Systemprompt in der Kiste `AGENT_LAYER/local-agent`, die Kisten als
Manifeste in `AGENT_LAYER/local-toolkits/`, die Skills in
`AGENT_LAYER/local-skills/`. Der Client hängt am Agenten, nie umgekehrt;
Konsole, Fenster und `myl` beziehen ihn direkt von dort.

## Ausliefern

`.github/workflows/release.yml` hat zwei Bau-Jobs:

- **`bauen`** liefert `myl`, `myl-node` und `myl-test` für **fünf**
  Ziele. ⚑ Jedes Binärprogramm belegt vor der Veröffentlichung, dass es
  richtig rechnet: Der Konformitätslauf über **siebzehn** Vektoren
  läuft mit genau diesem Programm auf dem Rechner, der es gebaut hat.
- **`oberflaeche`** bündelt die grafische Oberfläche: `.dmg` für macOS,
  `.msi` für Windows, `.deb` und AppImage für Linux x86_64, `.deb` für
  arm64.

⚠️ **Örtlich für die eigene Maschine gibt es ein Freigabeskript, und
es ist seit dem 2026-09-10 nicht mehr Teil dieses Repositoriums**
(Festlegung des Projektinhabers). Wer aus einem frischen Klon bauen
will, nimmt die Installationsskripte im Wurzelverzeichnis; wer die vier
Binaries und das Bündel in einem Zug will, baut sie einzeln mit
`cargo build --release`.

⚠️ **Was die Freigabe nicht leistet:** Die Bündel werden gebaut, aber
**nicht geöffnet**, denn ein Fenster braucht eine Anzeige und die hat
kein Runner. Nichts ist signiert. Nichts ist reproduzierbar; zwei Läufe
geben verschiedene Summen. Die Summen belegen, dass eine
heruntergeladene Datei die ist, die gebaut wurde, nicht, dass jemand
anders denselben Bau nachvollziehen kann.

## Was der Agent sehen darf

Der Harness entscheidet, **welches Werkzeug** laufen darf; der Klient,
**worauf** es zugreift. Drei Wege nach draussen sind geschlossen: `..`
im Pfad, ein Verweis nach draussen, ein absoluter Pfad. Aufgelöst wird
**vor** dem Vergleich; eine Textprüfung auf `..` übersähe den Verweis
und verböte zugleich harmlose Pfade.

⚠️ **Die Grenze hält der eigene Quelltext und nicht das
Betriebssystem** (Punkt 3.1). Das ist der Unterschied zwischen einer
Zusage und einer Sicherung, und er steht hier, weil er sonst niemandem
auffällt.

### Fremder Text aus dem Web

Die Recherchewerkzeuge des Chats sind abgeschaltet, bis das Häkchen
„Im Web recherchieren dürfen" gesetzt ist. Sind sie an, gilt:

* Gelesen wird **nur**, was aus einem Suchtreffer stammt, was der
  Nutzer selbst genannt hat, oder was auf einer schon gelesenen Seite
  als Verweis **auf denselben Wirt** stand. Eine vom Modell
  zusammengesetzte Adresse wird abgewiesen, bevor ein Abruf läuft.
  ⛔️ **Damit ist die Adresszeile kein Abflussrohr**, und das ist der
  Weg nach draussen, den eine eingeschleuste Anweisung sonst nimmt. Der
  geerntete Verweis ändert daran nichts: Er steht wörtlich in der
  Seite, und wer ihn ändert, ist nicht mehr im Zielkreis.
* Eine Suchfrage, die einen wörtlichen Abschnitt aus einem Anhang
  enthält, wird abgelehnt.
* Nur `https`, kein Anmeldeteil, kein fremder Port, nichts im eigenen
  Netz; geprüft vor dem Abruf und noch einmal auf dem Ziel jeder
  Umleitung.
* Seitentext kommt **eingefasst** zurück, mit der Ansage vorn und
  hinten, dass er Inhalt ist und keine Anweisung. Die Rahmenzeichen
  werden im Inhalt ersetzt, damit keine Seite den Rahmen von innen
  schliessen kann.
* Zwölf Abrufe je Rüstung, zwanzig Sekunden und zwei Megabyte je
  Abruf.

⚠️ **Im vollen Agentenbetrieb gibt es sie nicht.** Dort steht mit
`run_command` bereits ein Weg nach draussen offen, den keine dieser
Schranken einfasst.

### Der Blick auf den Bildschirm

Ob ein Bildschirmfoto als eigene Arbeit gilt, hängt an den Programmen,
die es zeigt. Vor jeder Aufnahme fragt `myl-senses` das System nach den
Besitzern aller sichtbaren Fenster (macOS über `osascript`, Linux unter
X11 über `wmctrl -lx`):

* **vertraut**, wenn nur Terminals, Editoren und Entwicklungsumgebungen,
  die Dateiverwaltung, Myelith selbst oder die Systemeinstellungen im
  Bild sind;
* **fremd**, sobald etwas anderes sichtbar ist, ausdrücklich Browser,
  Mail, Messenger und Betrachter wie Vorschau, und ebenso, wenn sich die
  Programme nicht feststellen lassen (Windows, Wayland, kein `wmctrl`).

Ein fremder Blick bekommt vorn eine Marke, die dem Modell die
sichtbaren fremden Programme nennt und sagt, dass der Inhalt Auskunft
ist und kein Auftrag. Der Informationsfluss liest dieselbe Marke: Danach
braucht eine Shell oder das Ändern einer bestehenden Datei die Freigabe
des Nutzers. ⚠️ Eine Namensliste ist eine Näherung; ein Terminal kann
auch eine heruntergeladene Datei zeigen.

## Abhängigkeiten

`INTEGER_LLM` (Laufzeit und Artefakte), `AGENT_LAYER` (Werkzeugansage
und Session-Kontrakte). ⚑ **Fremdprogramme sind keine Baubedingung:**
`curl` trägt die Web-Recherche, und fehlt es, fehlen die beiden
Werkzeuge und sonst nichts. Für die Netzhälfte zusätzlich: `CONSENSUS`
(Ledger-Zustand lesen), `TOKENOMICS` (Burn- und Mint-Fluss,
Credit-Preis), `NETWORKING` (Gateway), `GOVERNANCE` (Abstimmung).

⚑ **Die Reihenfolge hat sich umgedreht.** Der Client war als späte
Komponente geplant, weil er auf die anderen wartet. Er ist
vorgezogen worden, weil ohne ihn niemand sehen kann, ob das lokale
Modell überhaupt etwas taugt, und weil eine Schnittstelle, die kein
Mensch je bedient hat, an den Bedürfnissen vorbei entworfen wird.

## Changelog

### v0.178.4 (2026-10-23): Eine schlanke, festgenagelte Sprechumgebung, Läufer der Fassung 3 (Funde 604 b, 620; `myl-senses` 0.13.5)

- ⚑ **Fund 604 (b), die Umgebung von CosyVoice:** Sie kam aus der
  `requirements.txt` von CosyVoice, mit Server und Training (gradio,
  fastapi, uvicorn, tensorboard, grpcio, deepspeed), und hing an **20
  Paketen mit 248 bekannten Lücken**, darunter torch 2.3.1 (Codeausführung
  beim Laden fremder Gewichte) und transformers 4.51.3. Neu sind
  `SYSTEM/install/sprechen.in` (nur, was der Läufer beim Laden und Sprechen
  wirklich importiert, je Gruppe mit Grund) und daraus erzeugt
  `sprechen-fest.txt`: 139 Einträge mit Fassung und SHA-256 für alle
  Plattformen, installiert mit `--require-hashes`. Aufgelöst nur aus
  Fassungen, die mindestens zwei Wochen öffentlich waren; Ausnahme sind
  zwei Sicherheitsfassungen (certifi 2026.10.19, pillow 13.0.0). **OSV
  danach:** eine Meldung (setuptools, nur beim Erzeugen von sdists, in
  `sprechen.in` begründet; pyworld braucht `pkg_resources`). torch 2.14.1,
  transformers 5.19.0, numpy 2.4. Nebenbei fällt wetext weg, das bei jedem
  Start ohne Stand und Summe von modelscope.cn nachgeladen wurde; gesprochen
  wird jetzt ohne jeden Netzzugriff.
- **`sinne-einrichten.sh`:** baut die Umgebung aus der festen Liste und
  legt deren Abdruck hinein (`myelith-stand`). Eine Umgebung von anderem
  Stand wird ersetzt statt nachgebessert, am endgültigen Ort (eine venv
  lässt sich nicht verschieben); die alte liegt währenddessen daneben und
  kommt bei einem Abbruch zurück. `--pruefen` meldet „veraltet“. Python ab
  3.11. Die Sonderwege für `openai-whisper` und setuptools entfallen.
- ⚑ **Läufer, Fassung 3**, zwei Anpassungen an transformers 5, beide mit
  der alten Umgebung verträglich:
  1. `qwen2_in_float32`: transformers 5 lädt Qwen2 in der Fassung der
     Konfiguration (bfloat16); die Schichten darum bleiben float32, und der
     erste Schritt bricht ab (`mat1 and mat2 must have the same dtype`).
  2. 📌 `schritt_ohne_maske`: CosyVoice gibt ab dem zweiten Schritt eine
     Aufmerksamkeitsmaske der Länge 1 mit, obwohl der Zwischenspeicher alle
     Positionen hält. transformers bis 4.51 ließ eine Maske aus Einsen
     fallen; die neue Maskenlogik nimmt sie wörtlich. **Ohne Fehler und mit
     Rückgabe 0** dauerte das Aufwärmen 8,8 s statt 0,88 s, der Satz 0,4 s
     statt 4,96 s, und whisper hörte „Bis zum nächsten Mal.“ Gefunden durch
     Abgleich Schicht für Schicht: Tokenizer, Stimmprobe und
     Sprachmodell rechneten in beiden Umgebungen gleich (Abweichung höchstens
     3·10⁻⁴), nur die Erzeugungsschleife nicht.
- **Sprechprobe** (Stimmprobe von CosyVoice, zwei Sätze, whisper liest
  zurück): alte Umgebung und neue Umgebung mit Fassung 3 sprechen beide
  wörtlich richtig, gleiche Länge (0,88 s und 4,96 s); der Dauerbetrieb mit
  Stücken über zwei Sätze ebenso. Die Datei ist jetzt PCM 16 Bit statt
  Gleitkomma 32 Bit (torchaudio schreibt über torchcodec); der Client liest
  beides. **Gegenproben:** ohne `schritt_ohne_maske` der falsche Satz, ohne
  `qwen2_in_float32` der Abbruch.
- **Das Skript einmal ganz gefahren** auf der Entwicklungsmaschine (ohne
  Homebrew im `PATH`, damit `brew install` nichts anhebt): alle Gewichte mit
  stimmender Summe, die alte Umgebung ersetzt, die Standmarke gleich dem
  Abdruck der Liste, der Läufer in der Heimat ist Fassung 3. Danach mit
  genau dieser Umgebung gesprochen: wörtlich richtig; OSV über die 112
  installierten Pakete ohne Meldung, keines der Serverpakete mehr da.
- **`myl-senses`:** Fassung 2 (`0615a427195a045a`) steht jetzt in
  `FRUEHERE_LAEUFER`, sonst bekäme kein bestehender Einrichter die Fassung 3.
  Neue Probe `jede_fruehere_fassung_des_laeufers_ist_ersetzbar`: so viele
  Fingerabdrücke, wie der Läufer frühere Fassungen hat. **Gegenprobe:**
  Eintrag entfernt, rot.
- 📌 **Fund 620, `sinne-einrichten.sh`:** `HIER` war `$(dirname "$0")/..`,
  seit dem Umzug von `INSTALL/` nach `SYSTEM/install/` also `SYSTEM` statt
  der Wurzel. Ohne `MYL_SINNE` suchte und legte das Skript die Gewichte unter
  `SYSTEM/MODELS`, und das Kopieren des Läufers aus `SYSTEM/CLIENT/…`
  scheiterte. Jetzt `../..` wie die Geschwisterskripte, dazu ein Abbruch,
  wenn der Läufer unter der gefundenen Wurzel fehlt. **Gegenprobe:**
  `--pruefen` vorher alles „fehlt“, nachher alles gefunden.
- `SYSTEM/install/README.md`: die Stolpersteine zu setuptools und der
  Reihenfolge von pip ersetzt durch die feste Liste, die zwei Anpassungen
  und pyworld.

### v0.178.3 (2026-10-23): Sicherheitsaudit: Geheimdateien der Module, feste Stände für die Gewichte der Sinne (Funde 604, 607; `myl-module` 0.5.2)

Aus dem Sicherheitsaudit vom 2026-10-23.

- **Fund 607, `myl-module`:** `signatur::schreiben_atomar` mit `geheim`
  schrieb die Zwischendatei mit `fs::write` und setzte `0600` erst danach
  (betroffen: der geheime Modulschlüssel aus `myl-module schluessel`). Jetzt
  frisch und mit `0600` angelegt.
- ⚑ **Fund 604, `SYSTEM/install/sinne-einrichten.sh`:** Alle Gewichte für
  Sehen, Hören und Sprechen kamen vom wandernden Zweig `main` und ohne
  Prüfsumme, darunter die Pickle-Dateien von CosyVoice, die torch beim Laden
  auspackt, und CosyVoice selbst auf dem jeweils neuesten Stand. Die
  CosyVoice-Umgebung trägt torch 2.3.1, in dem `weights_only=True` nicht
  gegen Codeausführung schützt (CVE-2025-32434). Jetzt trägt jede Adresse
  einen Commit und jede Datei eine SHA-256; eine Datei mit falscher Summe
  wird nicht abgelegt, eine liegende abweichende gemeldet und nicht
  angefasst; CosyVoice steht fest auf `074ca6dc` (legt Matcha-TTS mit
  fest); `spk2info.pt` wird nicht mehr geholt (im festen Stand gibt es sie
  nicht). **Abgleich:** Alle 20 Dateien auf der Entwicklungsmaschine sind
  bitgleich mit dem festen Stand. **Gegenproben:** richtige Summe
  angenommen, falsche abgelehnt und nicht abgelegt, fremde liegende Datei
  gemeldet und behalten; der feste Stand samt Untermodul lässt sich holen.
  ⚠️ **Offen:** die CosyVoice-Umgebung selbst (21 Pakete mit Meldungen); ein
  Heben braucht eine echte Sprechprobe.
- Geprüft und in Ordnung: die Oberfläche (CSP ohne `unsafe-inline` für
  Skripte, kein `innerHTML`, das Fenster darf nur horchen), die Module
  (Ed25519, Prüfsumme je Datei, `sicherer_pfad`, Rollback-Sperre), das
  Aktualisieren (feste Quelle, `--ff-only`, Bestätigung), die Erhöhung
  (einzelne Argumente).

### v0.178.2 (2026-10-22): Eine Werkzeugkette für alle Kisten (`myl-client` 0.114.1, `myl-oberflaeche` 0.85.2, `myl-console` 0.69.1, `myl-senses` 0.13.4, `myl-module` 0.5.1)

- `rust-version` auf 1.99 in `myl-client`, `myl-oberflaeche`, `myl-console`, `myl-senses`, `myl-module`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  ⚑ **Dazu Code, und zwar mit Absicht:** Mit `rust-version = "1.99"` gibt Clippy
  zwei Regeln frei, die es bei 1.85 zurückhielt (`manual_is_multiple_of`,
  `chunks_exact` mit fester Größe). Festlegung des Projektinhabers:
  umschreiben, nicht abschalten. Hier: `myl-senses` 3 `as_chunks` (`kennzeichnung.rs`, `sprechen.rs`), `myl-client` 1 Zeile (`mining.rs`).
  `x % n == 0` wird `x.is_multiple_of(n)`, `chunks_exact(N)` wird
  `as_chunks::<N>()`. Für `n ≠ 0` bitgleich; jede Stelle mit variablem
  Teiler ist einzeln angesehen und vorher gegen null geschützt (bei
  `is_multiple_of(0)` stünde `x == 0`, wo `x % 0` abbricht).
- 📌 `myl-oberflaeche/tauri.conf.json` nachgezogen (0.85.2): Beim Sprung der
  Kistenversion stand dort noch 0.85.1, und die Probe
  `die_buendelversion_ist_die_kistenversion` fand es in der Matrix.
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen. Nach dem
  Umschreiben: Clippy mit `-D warnings` still (die Kerne über alle vier
  Merkmale), `cargo test` rc=0 in allen zehn berührten Kisten (die Kerne auch
  mit `cpu-simd`), Konformität mit `sparrow-0.6b-int8` 55/55, die vier
  Abdrücke unverändert (`cb696efd3817e575` über 24 Vektoren), Gleitkomma-Audit
  grün.

### v0.178.1 (2026-10-22): Die Sicherheitskopie bleibt auch nach künftigen Änderungen vollständig

Geprüft wurde, ob eine Kopie heute alles mitnimmt: ja, 34 045 Dateien mit
12,67 GB, beide kleinen Artefakte, Werkzeugvorrat passend zu `flake.lock`;
die 19 nicht kopierten Verknüpfungen liegen nur in fremden Bäumen. Die
Regeln schließen nur aus, Neues kommt also von selbst mit. Vier Stellen
konnten mit künftigen Änderungen still auseinanderlaufen:
- 📌 **Der Rust-Paketvorrat wurde vor der Kopie nicht geprüft**, nur der
  Werkzeugvorrat. `sicherung.py` ruft jetzt auch `vorrat.py pruefen` und
  bricht ab, wenn er nicht zu jeder Sperrdatei passt (auch zu den lokalen
  Modulen, die die CI nicht sieht). `SICHERUNG.md` nennt beide Vorräte.
  `--ohne-werkzeuge` erlaubt eine Kopie mit Lücke in einem der beiden.
- 📌 **Ein veralteter Name in `ARTEFAKTE_MIT` fiel still weg.** Nach einer
  Umbenennung wie am 2026-10-21 wäre die Kopie ohne Modell entstanden. Jetzt
  bricht sie ab und sagt, welcher Name fehlt (`--ohne-artefakte` bewusst).
- 📌 **`werkzeugvorrat.py pruefen` verglich nur die Fassung von Rust.** Eine
  neue Python-Fassung oder ein neues Rust-Ziel in `werkzeugvorrat.json` hieß
  weiter PASSED. Jetzt muss jede festgelegte Datei mit genau ihrer Quelle im
  Vorrat liegen (`nicht_geholt`).
- **Der Selbsttest lief nirgends.** Er steht jetzt in der CI (Job „audit“)
  und prüft dazu das Erkennen fehlender Artefakte.
- `--liste` zeigt, was git ignoriert und trotzdem mitkommt, nach Ordnern:
  dort zeigt sich neuer Bauabfall zuerst.
- Ein gescheiterter Prüfer meldet seine Fehlerzeile, nicht die letzte;
  `vorrat.py` endet mit dem lokalen Teil, und der war bestanden.
- **Belege:** Selbsttest PASSED. Gegenproben: ein Archiv aus dem Paketvorrat
  beiseitegelegt, dann ein veralteter Artefaktname: beide Male Rückgabe 1 und
  keine Datei im Ziel. Eine neue Python-Fassung und ein neues Rust-Ziel in
  der Festlegung werden beide gemeldet, der heutige Stand nicht.
  Kein Rust-Code geändert.

### v0.178.0 (2026-10-22): Netz vor Mensch im lokalen Plan (`myl-client` 0.114.0)

Stufe 4 des Dispatchers, P8 (Festlegung E7).

- `mining::netzlast` und `netzlast_im_plan`: Rechnet der Knoten einen Shard
  aus der Kette, steht seine Last mit Vorrang `Netz` im Plan.
- `Oertlichesmodell::laden` zieht den festen Bedarf des Netzshards von der
  Freigabe ab; reicht der Rest nicht, sagt die Ablehnung, wer den Platz hat
  und dass man für die nächste Epoche weniger zusagt. Der Plan des Agenten
  rechnet mit dem Shard, und `myl plan` führt ihn zuerst.
- ⚠️ **Offen:** „Mining starten“ startet den Knoten weiter ohne Shard; die
  Flags dafür hat der Knoten (`--shard-programm`, `--artefakte`,
  `--modellzusage`).

### v0.177.0 (2026-10-21): `myl eichen` misst die Ebenenzeit (`myl-client` 0.113.0)

Stufe 4 des Dispatchers, vorgezogen und geplant auf Bitte des Projektinhabers (2026-10-21): Ein Pod verteilt seine Ebenen nach Kapazität und gemessener Ebenenzeit seiner Mitglieder statt gleich. Hier P1 im Klienten.

- `myl eichen` misst bei einem eigenen Artefakt nach der Speicherspitze
  die Ebenenzeit und den Kopf in Ebenenzeiten und trägt beides mit dem
  Rechenweg ins Eichbuch ein.
- **Belege:** echt gefahren mit abgeschirmten Einstellungen: Sparrow 0.6B
  323 µs je Ebene (Kopf wie 5), Raven 35B A3B int4 907 µs (Kopf wie 1).
  `cargo test` und Clippy rc=0.

### v0.176.0 (2026-10-21): Die Modelle heißen nach Tieren, und vorhandene ziehen um (`myl-client` 0.112.0, `myl-console` 0.69.0, `myl-oberflaeche` 0.85.1)

Die Projektmodelle heißen seit heute nach Tieren, mit Größe und Speicherform als Anhängsel (Festlegung des Projektinhabers): `myelith-0.6b` ist `sparrow-0.6b-int8`, `myelith-4b` `pigeon-4b-int8`, `myelith-30b-a3b` `magpie-30b-a3b-int8`, `myelith-35b-a3b` `raven-35b-a3b-int8`. In den Dateien eines Artefakts steht kein Name, also bleiben Abdrücke und Register gültig.

- ⚑ **Die Anzeige** folgt der neuen Regel „<Tier> <Größe> - <Format>“:
  `raven-35b-a3b-int4` ist „Raven 35B A3B - INT4“ (`name_nach_schema`). Ein
  selbst gebautes Modell zeigt sein eigenes Wort, ein alter Ordner ohne
  Zahlform weiter `INT8`. Bis heute hieß es „Myelith 35B-A3B INT8“.
- ⚑ **Der Umzug** (`modelle::neuer_name`, `artefakte_umziehen`): Der
  Klient benennt Artefaktverzeichnisse mit altem Namen um, sobald er einen
  Suchort zum ersten Mal ansieht, je Prozess einmal, und nie über ein
  bestehendes Verzeichnis. Die Einstellungen ziehen beim Lesen mit (fünfte
  Wanderung). Die Zuordnung steht einmal, als `frueher` im Katalog, und
  erfasst auch Versuche mit eigenem Anhängsel (`myelith-35b-a3b-ternaer`
  wird `raven-35b-a3b-ternaer`).
- **Die Konsole** (`/build artifact`): Ein eigener Name ist ein Wort, das
  Anhängsel der gewählten Speicherform kommt dazu; ohne Eingabe gilt das
  Tier, das das Bauskript per Zufall gezogen hat. Die Erkundung trägt dafür
  `anhaengsel`.
- **Mitgezogen:** Proben, Vorgaben und Beispiele in Klient, Konsole und
  Fenster; `SYSTEM/install/karten-pruefen.sh` und `sicherung.py`.
- **Belege:** neue Proben `fruehere_namen_ziehen_um` (genau, mit Anhängsel,
  nie über ein bestehendes Verzeichnis) und die fünfte Wanderung in
  `eine_alte_ablage_findet_ihr_artefakt_wieder`; Clippy und `cargo test`
  siehe Mitschrieb.

### v0.175.1 (2026-10-21): Im Netz bietet der Klient nur das 35B an (`myl-client` 0.111.1)

- **Festlegung des Projektinhabers** (INTEGER_LLM v0.141.0): Netzmodell ist
  allein das 35B, int8 und int4. Wahl und Fenster zeigen damit jedes andere
  Modell als „(privat)“ und ohne Netzeintrag, auch 0,6B, 4B, 8B und 30B-A3B;
  lokal rechnen sie wie bisher.
- **Die Proben der Modellwahl rechnen im Prüfnetz:** `integer-llm-runtime`
  steht als Entwicklungsabhängigkeit mit dem Merkmal `pruefnetz`, damit ein
  Probe-Artefakt mit den Abdruckdateien des echten 0,6B weiter als
  freigegeben gilt. Ein Release-Bau kennt den Block nicht.
- **Belege:** `cargo test` der Kiste Rückgabe 0; Sperrdatei nachgezogen.

### v0.175.0 (2026-10-21): Privat ist, was nicht auf der Netzliste steht; kein Bau ohne Gewichte (`myl-client` 0.111.0, `myl-console` 0.68.1; Funde 584, 585)

- ⛔️ **`modelle::nur_lokal` folgt der Netzliste** (Fund 585, Einzelheiten
  unter INTEGER_LLM v0.140.0): Privat ist jedes Artefakt, das nicht mit
  seinem Abdruck auf ihr steht, gleich von welchem Anbieter; `betrieb =
  lokal` bleibt zusätzlich privat. Damit zeigen Wahl und Fenster ein selbst
  gebautes oder fremdes Modell immer mit „(privat)“ und ohne Netzeintrag,
  auch wenn sein `model_config.json` etwas anderes behauptet.
- **`/build artifact` bricht ohne Gewichtsdateien ab** (Fund 584): Die
  Erkundung meldet `ohne_gewichte`, die Konsole sagt, dass ein Link auf
  Hugging Face sie holt, statt einen Namen anzubieten.
- **Die Proben der Modellwahl** (`tests/modellwahl.rs`) folgen der Regel:
  Probe-Artefakte ohne Freigabe heissen „(privat)“ und haben keinen
  Netzeintrag; fuer die Netzeintraege traegt ein Probe-Artefakt die drei
  Dateien des Abdrucks vom echten 0,6B und steht damit auf der Liste (ohne
  Artefakt, wie in der CI, uebersprungen).
- **Belege:** `cargo test` der Kiste mit und ohne `MYL_OHNE_ARTEFAKTE`
  Rueckgabe 0 (darin `nur_lokal_folgt_der_netzliste`: ohne Abdruck privat,
  das echte 0,6B nicht), Clippy still, `myl-console` baut. Ende zu Ende:
  `myl-shard` laedt das echte 0,6B und bedient, eine Kopie mit von Hand
  gesetztem `betrieb = netz` lehnt es ab („steht nicht auf der Netzliste“).

### v0.174.0 (2026-10-21): Ein Einrichtungsskript für jedes Linux, das Update unter Linux ohne Nix, und eine Sicherheitskopie des ganzen Baums (`myl-client` 0.110.0; Funde 580, 581, 583)

Auftrag des Projektinhabers: Der ganze Arbeitsbaum soll sich als
Sicherheitskopie ohne Gewichte und Artefakte ziehen lassen, und diese Kopie
soll auf jedem System bauen und laufen, zuerst unter Linux; ohne starre
Pfade und Verweise, mit einem Paketvorrat, der auch die eigenen Module trägt.

- ⛔️ **Fund 581: Unter Linux ohne Nix konnte sich Myelith nicht
  aktualisieren.** `aktualisierung::SKRIPT_LINUX` nannte
  `installieren-nixos.sh`, und das bricht ohne `nix` ab; Konsole (`/update`)
  und Fenster riefen es auf Debian, Ubuntu, Fedora und Arch genauso. Jetzt
  nennt es `SYSTEM/install/installieren-linux.sh`, das auf NixOS (`/etc/NIXOS`
  oder `--nix`) selbst an das Nix-Skript abgibt. `SKRIPTE` führt alle vier;
  die Proben über die Skripte (Pfade, kein Überschreiben eines laufenden
  Programms, `--aktualisieren`, Anleitung) prüfen jetzt auch das neue.
- **`installieren-linux.sh` (neu):** prüft Rust, C-Compiler und Python,
  packt den Vorrat aus und baut **ohne Netz** nach `~/.local/bin`. ⚑ **Nur das
  Fenster braucht Systembibliotheken** (WebKitGTK 4.1, GTK 3, libsoup 3,
  dbus, gefragt über `pkg-config`); fehlen sie, baut es die übrigen vier
  Programme, lässt das Fenster aus und nennt den Paketbefehl der erkannten
  Distribution (Debian und Ubuntu, Fedora, Arch, openSUSE). Es ruft weder
  einen Paketverwalter noch `sudo`. `--ohne-fenster`, `--pruefen`,
  `--aktualisieren`, `--nix`.
- **Fund 580, Teil Menüeintrag:** Der Eintrag unter Linux trug
  `Icon=<Pfad in den Baum>`; ein verschobener oder gelöschter Baum nahm das
  Symbol mit. Beide Linux-Skripte legen es jetzt nach
  `~/.local/share/icons/hicolor/512x512/apps/myelith.png` und nennen es
  beim Namen.
- **Fund 583: Unter Windows wurde der Vorrat nur ausgepackt, wenn es gar
  kein Lager gab.** Brachte ein Update neue Archive, baute `--offline` gegen
  das alte Lager und brach ab. Jetzt auch, wenn ein Vorrat neuer ist als
  das Lager oder die Konfiguration fehlt, wie unter macOS; in beiden zählt
  der lokale Vorrat mit.
- **Der Paketvorrat** (`SYSTEM/install/vorrat.py`): Was nur eigene, nicht
  versionierte Kisten brauchen (Module unter `MODULES/`), sammelt `sammeln`
  nach `SYSTEM/crates-vorrat-lokal/` (von git ignoriert); `pruefen` hält
  beide Vorräte gegen ihre Sperrdateien, `auspacken` legt beide ins Lager.
  Bisher fehlten den Modulen vier Archive, und eine Kopie des Baums baute
  sie nicht ohne Netz. ⚑ Der versionierte Vorrat bleibt genau das, was die
  versionierten Sperrdateien verlangen (Fund 545). **Fund 580, Teil
  Lager:** Die erzeugte `cargo-home/config.toml` nannte das Lager mit
  absolutem Pfad; jetzt relativ (`vendor`), geprüft samt Gegenprobe.
- **`SYSTEM/install/sicherung.py` (neu):** schreibt
  `myelith-sicherung-JJJJ-MM-TT.tar.gz` (oder `.zip`) mit allem, was im
  Arbeitsbaum liegt, ohne Gewichte, Artefakte, Python-Umgebungen und
  Bauabfall; Verknüpfungen werden genannt, nicht kopiert. In der Kopie
  liegen `SICHERUNG.md` und `PRUEFSUMMEN.sha256`. `--liste`, `--selbsttest`
  (Auswahl, Verknüpfungen, tar und zip gegen die Prüfsummen; Gegenprobe ohne
  die Regel für den Bauordner rot).
- Die Probe `jeder_pfad_in_den_skripten_liegt_da` kennt `}` als Ende eines
  Pfades (`${CARGO_TARGET_DIR:-$WURZEL/…}`) und `SYSTEM/crates-vorrat-lokal`
  als einen Ordner, der erst entsteht.
- **Belege:** `cargo test --lib aktualisierung` 10/10; die Probe der Skripte
  in `myl-local-agent`; `sh -n` über die Shell-Skripte; die Kopie des echten
  Baums (4,06 GB, 1,42 GB gepackt) entpackt, alle 31 503 Prüfsummen gleich.
  Aus ihr ohne Netz gebaut: unter macOS alle fünf Programme samt
  `Myelith.app` in 122 s (`installieren-macos.sh` mit eigenem `HOME`), dazu
  das Handelsmodul; unter Linux in einem frischen `debian:bookworm-slim` mit
  `--network none` ohne Fensterpakete vier Programme in 65 s (Fenster
  ausgelassen, Paketbefehl genannt), mit ihnen alle fünf in 103 s samt
  Menüeintrag und Symbol, `ldd` ohne fehlende Bibliothek. `myl`, `myl-node`
  und `myl-test` antworten auf `--hilfe`.
- ⚠️ **Windows ist damit nicht belegt**: Die CI baut dort aus einem Klon,
  nicht aus einer Kopie und nicht über das Skript.
- **Nachtrag, derselbe Tag: ganz ohne Netz, auch die Werkzeuge.** Auftrag
  des Projektinhabers: alles offline bau- und ausführbar, unter Linux
  zuerst NixOS. `SYSTEM/install/werkzeugvorrat.py` (`holen`, `pruefen`,
  `zeigen`, Quellen und Fassungen in `werkzeugvorrat.json`) füllt
  `SYSTEM/werkzeug-vorrat/` (von git ignoriert, in der Kopie enthalten):
  für `aarch64-linux` und `x86_64-linux` die Quellen der Eingaben aus
  `flake.lock` und die ganze Closure der Bauumgebung aus `flake.nix` als
  Nix-Binärcache (je 364 Pfade, rund 1 GB), für macOS und Windows die
  Installer von Rust 1.99.1 und Python 3.12.10, für Windows WebView2 und der
  Starter der Buildwerkzeuge. `nix-vorrat-einspielen.sh` (neu, als root)
  legt die Umgebung in den Nix-Speicher; `installieren-nixos.sh` nimmt dann
  `nix develop --offline` und legt sie als Profil unter
  `~/.local/state/myelith/bauumgebung` ab, damit die Speicherbereinigung die
  Bibliotheken der Programme nicht wegnimmt. Die Skripte für macOS und
  Windows nennen bei fehlendem Rust oder Python den Befehl mit der Datei aus
  dem Vorrat. Die Kopie nimmt die Artefakte `myelith-0.6b` und `myelith-4b`
  mit (`--ohne-artefakte` nicht).
- Die Paketnamen für Linux ohne Nix stehen jetzt in
  `SYSTEM/install/systempakete.txt`, gelesen von `installieren-linux.sh`.
- **Belege:** `werkzeugvorrat.py pruefen` grün, Gegenproben (ein Byte mehr
  in einem Installer, ein fehlendes Archiv im Nix-Cache) rot; in einem
  frischen `nixos/nix`-Behälter ohne Netz eingespielt, `nix develop
  --offline` mit Rust 1.98.1, WebKitGTK 2.52.6, Python 3.12 (x86_64). Kopie
  mit Vorrat und Artefakten: 34 430 Dateien, 12,67 GB, gepackt 8,97 GB; aus
  ihr in einem frischen `nixos/nix` (arm64) ohne Netz alle fünf Programme in
  160 s, Konformität mit allen vier Abdrücken, `myl frage` an das 0,6B
  beantwortet.
- ⚑ **`sicherung.py` schreibt keine Kopie mit lückenhaftem Werkzeugvorrat**:
  Geholt werden kann er nur mit Netz, also vor der Kopie; auf dem Rechner
  ohne Netz fiele die Lücke zu spät auf. Bewusst ohne nur mit
  `--ohne-werkzeuge`, und `SICHERUNG.md` sagt es dann. Gegenprobe: ein Nix-Teil
  mit falscher Marke für `flake.lock` lässt die Kopie mit Rückgabe 1
  abbrechen, ohne eine Datei zu schreiben.

**Ältere Einträge** (v0.173.1 und davor, 181 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- CLIENT/README/README.md`.
