# client (Nutzer-Client inkl. Wallet)

> **Version:** 0.234.0 (`myl-client` 0.133.0, `myl-oberflaeche` 0.92.0, `myl-console` 0.72.1, `myl-senses` 0.47.0, `myl-module` 0.6.2, `myl-scan` 0.4.0)
> **Datum:** 2026-11-13
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
über dreiundachtzig Befehle. ⚠️ **Mit fünf Ausnahmen, und alle sind gewollt:** `terminal_ausfuehren` startet eine Shell, denn ein Terminal, das keine startet, ist keines; `modul_waehlen` und `modul_befehl` starten ein Modul, und zwar wie die Konsole nur die geprüfte Kopie eines signierten Moduls; `artefakt_bauen` startet die Bauskripte des Repositoriums über `myl_client::bezug`, dieselbe Stelle wie die Konsole, und nur nach Bestätigung; die Aktualisierung startet `curl`, `git` und das Installationsskript über `myl_client::aktualisierung`. `mining_starten` startet den Knoten (`myl-node`) und `mining_beenden` schickt ihm das Ende (`kill`), beide über `myl_client::mining`, dieselbe Stelle wie `/mining` in der Konsole. Jeder andere Befehl startet **keinen einzigen Unterprozess**. 📌 Bis zum 2026-10-16 nannte dieser Satz nur die ersten beiden, obwohl Bau und Aktualisierung schon damals Unterprozesse starteten. `jeder_befehl_ist_angemeldet` hält die vier
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
| `myl-scan/` | Gescannte Seiten lesen: Seitenaufbau, Text, Abbildungen. Ohne Netz, die Modelle sind eingebaut. Nur für das Programm `myl`. |
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

### v0.234.0 (2026-11-13): Geschriebene Texte tragen ein unsichtbares Kennzeichen (`myl-senses` 0.47.0, `myl-client` 0.133.0)

- ⚑ **Festlegung des Projektinhabers:** Text wird nur mit signierten
  Metadaten gekennzeichnet, und nur so, dass der Nutzer nichts davon
  sieht; abschaltbar. Gebaut nach **C2PA 2.4, Anhang A.8**: Am Ende des
  Texts steht U+FEFF, dann je Byte ein Variantenzeichen (U+FE00 ff., U+E0100
  ff.) für `C2PATXT\0`, Fassung 1, die Länge und die Ablage. Darin dasselbe
  Manifest wie beim Ton (Aktion `c2pa.created` mit
  `trainedAlgorithmicMedia`, Ed25519, je Text ein frischer Schlüssel),
  gebunden an den Hash über den Text in NFC und UTF-8 ohne die Hülle.
  Ausgenommen ist die Hülle samt U+FEFF. Die Norm lässt die Bytefolge der
  Länge und die Zugehörigkeit von U+FEFF offen; beides wie in den
  Referenzumsetzungen (`c2pa-text`, `c2pa-unstructured-text`): Länge groß
  zuerst, U+FEFF ausgenommen. Fremde Hüllen mit Füllung dahinter lesen sich
  (Prüffall der Rust-Referenz); geprüft wird nach den Lagen im gespeicherten
  Text, erst ausschneiden, dann NFC.
- `kennzeichnung::text_kennzeichnen`, `text_herkunft`,
  `ohne_textkennzeichen`, `text_kennzeichnen_soll` (Markdown und Text ab
  800 Zeichen, rund 200 Token; nie Quelltext). Fehler nach Norm:
  `manifest.text.corruptedWrapper`, `manifest.text.multipleWrappers`.
  Neue Abhängigkeit `unicode-normalization` 0.1.25, dieselbe, die
  `myl-client` schon nimmt.
- **`myl kennzeichen <datei>`** prüft auch Text: „ja, KI-erzeugt, Text
  unverändert“, „ungültig, …“ oder „nein“. Von Ende zu Ende geprüft, auch mit
  einem geänderten Buchstaben und ohne Hülle. ⚠️ Die vorhandene
  `c2patool`-Fassung (0.27.23) kennt Text noch nicht.
- Neue Einstellung `agent.text_kennzeichnen` („Geschriebene Texte
  kennzeichnen“), voreingestellt an.
- Proben mit Gegenproben (signiert gültig, Änderung, verstümmelt, doppelt,
  NFD, Prüffall der Referenz); die Hülle eines Texts von 1 280 Bytes hat
  5,8 KB.

### v0.233.0 (2026-11-13): Die Aufnahme folgt dem Eingang des Systems, und Reste aus der Härtung (`myl-senses` 0.46.0, `myl-client` 0.132.3, `myl-module` 0.6.2)

- 📌 **Befund aus der Mikrofonabnahme:** Die Aufnahme über ffmpeg nahm
  unter macOS fest `avfoundation :0`, also das erste Gerät der Liste und
  nicht den Eingang aus den Systemeinstellungen. Mit einem virtuellen Gerät
  an erster Stelle (BlackHole) kam nur Stille an, und whisper.cpp schrieb
  daraus „you“. Jetzt `:default`, wie beim eigenen Weg. Probe
  `die_tonquelle_ist_der_eingang_des_systems`, Gegenprobe mit `:0` rot.
- **Ohne Freigabe zur Bildschirmaufnahme** steht der Grund jetzt vorn, mit
  dem Ort der Einstellung und dem Hinweis, ihn so weiterzugeben
  (`blick::FREIGABE_FEHLT`). Vorher bekam der Agent nur Rückgabewert und
  Meldung von `screencapture`, und das Modell machte daraus eine Liste von
  Vermutungen. Probe mit Gegenprobe.
- **Größen aus `config.json`** der Sinne (whisper, Sprecher, Tonzerleger in
  beiden Richtungen) gehen durch `whisper::masszahl`: ganz, größer null,
  höchstens 2²⁴. Vorher wurde `1e30` zu `usize::MAX`, und Produkte liefen
  über, bevor die Formen der Tensoren geprüft wurden. Probe mit
  präpariertem Kopf, Gegenprobe rot; fünf Proben mit echten Gewichten grün.
- Der JSON-Leser der Sinne setzt UTF-16-Paare aus `\u`-Folgen zu einem
  Zeichen zusammen; einzelne Hälften bleiben U+FFFD.
- **Gesamtfrist:** `mining::holen` (Metriken, Konto, Verlauf) hat 2 s für
  die ganze Antwort statt je Lesen; ein Knoten, der alle 300 ms ein Byte
  schickt, hielt das Wallet vorher beliebig lange. Probe, Gegenprobe rot
  (12 s).
- **Proben räumen auf:** Die Zwischendateien der whisper-Proben bekommen je
  Aufruf einen eigenen Ordner, der mit ihnen verschwindet (vorher teilten
  sich zwei Proben `config.json` an derselben Stelle, und cargo lässt Proben
  nebeneinander laufen); die Ordner der WebAssembly-Proben ebenso.
  Gegenprobe: ohne das Löschen 13 liegengebliebene Ordner.
- `myl-senses` 153, `myl-module` 41 grün, Clippy sauber.

### v0.232.2 (2026-11-13): Eine Antwort des Knotens hat eine Obergrenze (`myl-client` 0.132.2)

- 📌 **Befund:** `mining::holen` (Metriken, Gesundheit, Konto und Verlauf
  des Wallets) las mit `read_to_string` alles bis zum Auflegen. Das Wallet
  fragt auch einen Knoten auf einer anderen Maschine; eine Gegenstelle,
  die nicht aufhört, ließ den Speicher wachsen. Jetzt höchstens 16 MiB,
  darüber ein Satz.
- Probe `eine_gegenstelle_ohne_ende_fuellt_nicht_den_speicher` (Stummel,
  der danach offen hält); Gegenprobe ohne Grenze: Frist statt Satz, rot.

### v0.232.1 (2026-11-13): Ein präparierter Safetensors-Kopf wird abgewiesen (`myl-senses` 0.45.1)

- 📌 **Befund:** Der Leser für die Köpfe der Gewichtsdateien (Hören,
  Sprechen, Stimmabdruck, Tonzerleger) nahm die Zahlen aus dem Kopf
  ungeprüft. `data_offsets` mit einem Eintrag brachte eine Panik am Index;
  eine Lage von `1e30` wurde zu `usize::MAX`, `8 + n + e` lief über (im
  Release-Bau unbemerkt), und mit passender Form hätte `tensor_lesen` fast
  2⁶⁴ Bytes verlangt; eine Form ohne Achsen brachte `zeile_lesen` zur
  Panik. Gewichte aus einem eigenen Ordner (`MYL_HOEREN_MODELL`) kennen
  keine Prüfsumme. Jetzt ist jede Zahl ganz, nicht negativ und höchstens
  so groß wie die Datei, die Form multipliziert ohne Überlauf, und
  `data_offsets` hat genau zwei Zahlen.
- Probe `ein_praeparierter_kopf_wird_abgewiesen` (sieben Köpfe und ein
  Skalar ohne Zeilen); Gegenprobe mit der alten Lesart: Panik am Index.
  `myl-senses` 149 grün, Clippy sauber. Mit echten Gewichten grün: der
  Decoder von large-v3-turbo in W8A8, der Rückweg des Tonzerlegers und der
  Stimmabdruck gegen ihre Referenzen. Neun Sperrdateien auf 0.45.1.

### v0.232.0 (2026-11-13): Ein präpariertes C2PA-Manifest bricht `myl kennzeichen` nicht mehr ab (`myl-senses` 0.45.0)

- 📌 **Befund:** Der eigene CBOR-Leser der Herkunftsprüfung rief sich für
  jede Liste, Karte und Marke ohne Tiefengrenze selbst auf. Ein WAV mit
  einem Block `C2PA`, dessen Behauptung aus 100 000 geschachtelten Listen
  besteht (100 KB), brachte `myl kennzeichen` zum Stapelüberlauf und
  beendete den Prozess (Rückgabe 134), statt die Datei abzulehnen. Jetzt
  höchstens 64 Ebenen (ein Manifest ist sechs tief), und eine Bytefolge
  mit der Länge 2⁶⁴ − 1 läuft nicht mehr über.
- Von Ende zu Ende mit dem echten `myl`: dieselbe Datei vorher Abbruch,
  jetzt „ungültig, CBOR: tiefer als 64 Ebenen“, Rückgabe 1; eine echte
  gekennzeichnete Datei bleibt in Metadaten, C2PA und Wasserzeichen
  gültig. Probe `ein_praepariertes_manifest_wird_abgelehnt`; Gegenprobe
  ohne Grenze bricht am Stapel ab.
- Neu `kennzeichnung::pruefen_daten` (dieselbe Prüfung auf Bytes) und
  `kennzeichnung::herkunft` (nur das Manifest). Neues Fuzz-Ziel
  `kennzeichen` in `myl-senses/fuzz` über RIFF, JUMBF, CBOR und Zertifikat;
  die Suche nach dem Wasserzeichen bleibt draußen (rund 16 Millionen
  Schritte je Datei, mit ihr sechs Läufe je Sekunde). Saaten zwei
  signierte WAVs und eines ohne Marke. Gegenprobe ohne die Prüfung der
  Kastenlänge: Absturz nach rund 500 Läufen. Lauf 600 s: 12,7 Millionen
  ohne Fund; verdichtet und auf 25 gekappt.
- `myl-senses` 115 grün, Clippy sauber; neun Sperrdateien auf 0.45.0.

### v0.231.0 (2026-11-13): Eine hängende Seite hält `myl scan` nicht mehr fest (`myl-scan` 0.4.0, `myl-client` 0.132.1)

- `hayro` kennt keinen Abbruch, und Book2Skill rief `myl scan` ohne Frist;
  eine Schleife an einer präparierten Seite hätte den ganzen Lauf
  festgehalten. Eine Frist für den ganzen Lauf passt nicht (1 000 Seiten
  brauchten 796 s). Jetzt rendert ein eigener Faden Seite für Seite auf
  Zuruf, und jede Seite darf höchstens `FRIST_JE_SEITE` (120 s) brauchen,
  das Öffnen des PDF ebenso. Überschreitet eine Seite die Frist, liegt sie
  als unlesbar im Arbeitsordner (`"unlesbar": true`, ohne Text), und
  `myl scan` endet mit **3**: Der Faden lässt sich nicht anhalten, erst das
  Ende des Prozesses stoppt ihn. Book2Skill ruft dann für den Rest neu auf.
  Ein Absturz im Renderer ist jetzt ein Fehler der Seite statt einer Panik.
  Der Auftrag kann die Frist mit `frist_je_seite_ms` setzen.
- Probe `eine_haengende_seite_wird_nach_der_frist_genannt` (hängende Seite,
  Absturz, PDF öffnet nie); Gegenprobe mit hundertfacher Frist rot. Von Ende
  zu Ende mit dem echten `myl`: Frist 1 ms ergibt Rückgabe 3 und Seite 1
  unlesbar, der nächste Aufruf liefert Seite 2 und 3.
- Neues Fuzz-Ziel `pdf` in `myl-scan/fuzz` (Seiten zählen, erste Seite
  rendern, das Bild bleibt in den Grenzen), sieben Saaten, stabiler
  Regressionslauf `tests/fuzzkorpus.rs`. ⚠️ Gebaut mit `-s none`: Unter
  AddressSanitizer scheitert der Linker von macOS an den statischen
  Initialisierern von `tract`. Gegenprobe ohne Größengrenze: nach rund
  20 000 Läufen ein Bild von 18 381 x 276 Pixeln gefunden. Lauf 600 s:
  72 329 Eingaben ohne Fund (jede rendert eine Seite, rund 120 je
  Sekunde); Korpus verdichtet und auf 25 gekappt.
- `myl-scan` 28 und 9 grün, `myl-client` und `myl-scan` Clippy sauber,
  Vorrat grün (die Fuzz-Kisten aus dem lokalen Vorrat).

### v0.230.0 (2026-11-12): Fremder Text kommt ohne Steuerzeichen ins Terminal (`myl-client` 0.132.0, `myl-console` 0.72.1)

- 📌 **Befund:** Konsole und `myl` gaben Antworten des Modells,
  Werkzeugausgaben, den Strom der Antwort und den Mitschnitt mit allen
  Steuerzeichen aus. Ein `ESC` aus einer Webseite oder Datei, vom Modell
  wiedergegeben, hätte den Fenstertitel gesetzt, Zeilen überschrieben oder
  in manchen Terminals über OSC 52 in die Zwischenablage geschrieben.
- Neu `myl_client::terminal`: `steuerfrei` zeigt Steuerzeichen als ihre
  Bildzeichen (U+2400 ff., `ESC` als ␛, DEL als ␡), C1 als U+FFFD;
  Zeilenumbruch und Tabulator bleiben. Die Konsole filtert an allen drei
  Eingängen (Zeitleiste, Strom, Schlussantwort), `myl` mit `fuer_ausgabe`
  nur, wenn die Ausgabe ein Terminal ist, damit sie in Röhre und Datei
  unverändert weiterverarbeitbar bleibt. Die eigenen Farben entstehen erst
  danach. Das Fenster braucht es nicht (`textContent`, kein `ESC`).
- Proben `steuerzeichen_werden_bildzeichen` und
  `fremder_text_kommt_ohne_steuerzeichen` (auch durch `antwort::setzen`,
  mit und ohne Farbe); Gegenprobe mit durchgelassenem Text in beiden rot.
  `myl-client` 233, `myl-console` 193, `myl-oberflaeche` 93 grün, Clippy
  sauber. ⚠️ Am echten Terminal nicht eigens durchgespielt.

### v0.229.3 (2026-11-12): Eine erfundene Seitengröße sprengt `myl scan` nicht mehr (`myl-scan` 0.3.1)

- 📌 **Befund:** `myl scan` renderte jede PDF-Seite mit 216 dpi, gleich wie
  groß die Seite sich nannte. Eine `MediaBox` von 20 000 Punkten ergab ein
  Bild von 60 000 x 60 000 Pixeln (rund 25 GB), eine von 200 000 Punkten
  brachte `vello_common` zur Panik, weil `hayro` die Breite als `u16`
  kappt. Jetzt bleibt es bei 216 dpi bis 16 384 Pixel je Kante und 64
  Megapixel, darüber wird verkleinert (A4 sind 4,5 Megapixel, A0 knapp 70).
  Eine Seitennummer 0 ist ein Fehler statt eines Unterlaufs.
- Proben `eine_riesige_seite_wird_verkleinert` (A4 unverändert 1836 x 2376;
  Gegenprobe mit festem Maßstab rot) und `seite_null_ist_ein_fehler`.
  `myl-scan` 36 grün, Clippy sauber.

### v0.229.2 (2026-11-12): Ein Fuzz-Ziel für die Gliederung im Fenster (`myl-client` 0.131.3)

- Neues Ziel `markdown` in `myl-client/fuzz`: Was `markdown::zerlegen` aus
  einer Antwort macht, enthält nie mehr Zeichen als die Antwort. Gegenprobe
  (Kursiv doppelt eingesetzt): „140 Zeichen aus 121“. Lauf 300 s mit
  Eingaben bis 30 000 Zeichen und 2 s Zeitgrenze: 1,5 Millionen ohne Fund.
  Zwei Saaten.

### v0.229.1 (2026-11-12): Lange Zeilen gliedert das Fenster in linearer Zeit (`myl-client` 0.131.2)

- 📌 **Befund:** `markdown::zeile_zerlegen`, das jede Antwort und jeden
  Bericht für das Fenster gliedert, suchte für jedes `[`, `` ` ``, `*` und
  `_` bis zum Zeilenende. Eine Zeile aus 30 000 `[` ohne `]` (etwa die
  längste Antwort) brauchte 0,7 s, bei jedem Neuzeichnen. Jetzt merkt sich
  die Zeile je Marke den letzten Suchausgang: nichts gefunden heißt auch ab
  später nichts, `e` gefunden gilt für jeden Anfang bis `e`.
- Proben `eine_lange_zeile_ohne_gegenstueck_kostet_lineare_zeit` (vorher
  rot) und `der_merker_aendert_kein_ergebnis`: 20 000 Zeilen aus den
  Markenzeichen gegen eine Abschrift ohne Merker, gleiches Ergebnis
  (Gegenprobe mit falsch gemerktem Treffer rot). `myl-client` 233 grün,
  Clippy sauber.

### v0.229.0 (2026-11-12): Eine erfundene Abtastrate legt das Umtasten nicht mehr fest (`myl-senses` 0.44.0)

- 📌 **Befund:** `ton::kopf` und der FLAC-Leser nahmen jede Abtastrate aus
  dem Kopf an, auch 1 Hz. `umtasten` auf 16 kHz rechnete dann das
  16 000-fache, mit rund 67 Summanden je Ausgabeprobe; schon ein WAV von
  10 KB hätte 80 Millionen Ausgabeproben verlangt, und das Programm hing.
  Anhänge können von außen kommen. Jetzt gelten 1 kHz bis 768 kHz
  (`ton::RATEN`), sonst `Tonfehler::Rate` beziehungsweise ein Fehler beim
  FLAC-Kopf.
- Neues Fuzz-Ziel `wav` in `myl-senses/fuzz`: Was sich liest, lässt sich
  umtasten, und daraus werden höchstens sechzehnmal so viele Proben.
  Gegenprobe ohne Grenze: nach rund 5 000 Läufen „7112 Proben aus 20“.
  Läufe je 300 s ohne Fund (`wav` 8,1 Millionen, `flac` 2,4 Millionen).
- Proben `eine_erfundene_rate_wird_abgewiesen` für WAV (vor der Grenze rot)
  und FLAC (Gegenprobe mit der alten Prüfung rot). `myl-senses` 147 grün,
  Clippy sauber.

### v0.228.2 (2026-11-12): Tief verschachteltes JSON sprengt den Stapel nicht mehr (`myl-senses` 0.43.1)

- 📌 **Befund:** Der eigene JSON-Leser der Sinne (`whisper::json`, für
  `config.json`, `vocab.json`, `added_tokens.json` und die Köpfe der
  Safetensors von whisper und Qwen3-TTS) rief sich je Klammer selbst auf,
  ohne Grenze. 100 000 `[` brachen den ganzen Prozess mit „stack overflow,
  aborting“ ab, keine abfangbare Panik. Die Einrichtung prüft Prüfsummen,
  eigene Gewichtsordner aber nicht. Jetzt höchstens 128 Ebenen, sonst ein
  Fehler; echte Dateien liegen bei höchstens fünf.
- Probe `tief_verschachteltes_json_sprengt_nichts` (vor der Berichtigung
  Abbruch, danach Fehler; 100 Ebenen lesen sich). Von Ende zu Ende: der
  eigene Hörweg liest die echten Dateien von large-v3-turbo und schreibt
  wörtlich mit. Die übrigen JSON-Stellen nehmen `serde_json` (Grenze 128
  von sich aus). `myl-senses` 145 grün, Clippy sauber.

### v0.228.1 (2026-11-12): `myl wallet` nennt gesendete Transaktionen statt der rohen Nonce (`myl-client` 0.131.1)

- 📌 Aus der Probe am Probenetz: Ein unbenutztes Konto zeigte „Nonce
  4294967296“. Das ist der Nonce-Boden der Kette (Epoche 1 · 2³²), richtig,
  aber nicht lesbar. Die Liste nennt jetzt „0 gesendet“, also was über dem
  Boden liegt (`wallet::gesendet`). Probe `gesendet_zaehlt_ueber_dem_boden`.

### v0.228.0 (2026-11-12): Epoche und Rolle in der Zusammenfassung (`myl-client` 0.131.0)

- Die Zusammenfassung des Minings (Konsole und Fenster, dieselbe Stelle)
  nennt jetzt die Epoche neben der Höhe und eine Zeile „Rolle“: Miner und
  Validator, nur Miner, oder noch nicht angemeldet, wie die Kette es führt
  (NODE 0.124.0 meldet es). Meldet ein älterer Knoten beides nicht, steht
  nichts da statt einer falschen Angabe. Damit sind Gegenstellen, Höhe,
  Epoche, Rolle und Guthaben sichtbar (Punkt 2.2).
- Probe `epoche_und_rolle_nur_wenn_gemeldet` (Gegenprobe: ohne Meldung
  „nicht angemeldet“ rot); von Ende zu Ende am Probenetz: erst „noch nicht
  als Miner angemeldet“, nach einer Minute „Miner (nicht im Validatorsatz
  dieser Epoche)“, dazu „Epoche 1“. `myl-client` 229 grün, Clippy sauber.

### v0.227.0 (2026-11-12): Ein Fuzzer für den FLAC-Leser, und sein erster Fund (`myl-senses` 0.43.0)

- Neu `myl-senses/fuzz/` mit dem Ziel `flac` (Rumpf in
  `tests/fuzzziele/mod.rs`, stabiler Lauf `tests/fuzzkorpus.rs`, Muster wie
  in `myl-types` und `myl-client`): Jede Eingabe endet mit Ergebnis oder
  Fehler, ein Ergebnis hat gleich lange Kanäle, Proben innerhalb der Breite,
  und `lesen` liefert Werte in [-1, 1].
- 📌 **Fund:** Ein Rahmen, dessen Kopf mehr Bit nennt als `STREAMINFO`,
  wurde angenommen, und `lesen` skalierte mit der Breite aus `STREAMINFO`:
  Proben außerhalb von [-1, 1]. Jetzt weist der Leser einen Rahmen mit
  anderer Breite ab, wie es die Norm verlangt. Der Reproduzierer liegt
  unter `fuzz/artifacts/flac/` und läuft im stabilen Lauf mit (vor der
  Berichtigung rot, danach abgewiesen).
- Gefunden hat ihn eine Gegenprobe, die einen anderen Fehler einbauen
  sollte; der Fund stand auch im unveränderten Leser. Lauf je 600 s danach
  ohne Fund (4,2 Millionen Eingaben). `myl-senses` 144 grün, Clippy sauber.

### v0.226.0 (2026-11-12): FLAC liest der Client selbst (`myl-senses` 0.42.0)

- Neu `myl_senses::flac`: ein Leser nach RFC 9639, neu geschrieben
  (`STREAMINFO`, Rahmen mit CRC-8 und CRC-16, `CONSTANT`, `VERBATIM`,
  `FIXED`, `LPC`, Rice-Restwerte, die drei Kanalpaare, ungenutzte Bits,
  ID3 davor und dahinter). Er rechnet die MD5-Summe aus `STREAMINFO` nach
  und weist eine Datei ab, deren Proben sie nicht treffen; dafür MD5 nach
  RFC 1321 (Prüfvektoren der Norm). Eine Obergrenze von 2²⁶ Proben je Kanal
  hält eine kleine Datei aus `CONSTANT`-Rahmen vom Speicher fern.
- `hoeren::nach_wav_mit` nimmt ihn vor `afconvert` und ffmpeg: Ein FLAC
  braucht auf keinem System mehr ein fremdes Programm.
- Geprüft an acht Dateien von ffmpeg (fest, ohne Vorhersage, LPC; die vier
  Kanalanordnungen; 24 Bit; Rahmen zu 256 und 576), jede trifft ihre
  MD5-Summe, und dasselbe Signal ergibt in jeder Anordnung dieselben
  Proben; dazu von Ende zu Ende eine Datei des Apple-Kodierers über das
  echte `myl`, mit Spionen für ffmpeg und `afconvert` vorn im PATH (keiner
  gerufen, wörtlich mitgeschrieben, Summe getroffen). Verfälschte und
  gekürzte Dateien scheitern sauber (3 000 Abwandlungen), ein Rahmen mit
  mehr ungenutzten Bits als Bits ist ein Fehler und kein Unterlauf.
  Gegenproben: vertauschte Mitte/Seite, LPC ohne Verschiebung, fehlender
  Schutz der ungenutzten Bits, je rot. `myl-senses` 143 grün, Clippy sauber.

### v0.225.0 (2026-11-12): Unter macOS liest das System die üblichen Tonformate (`myl-senses` 0.41.0)

- Was kein WAV ist, wandelt unter macOS zuerst `afconvert` des Systems
  (m4a, AAC, MP3, FLAC, ALAC, CAF), danach erst ffmpeg (Ogg, Opus, WebM und
  was das System nicht liest). Damit braucht ein Mac für Anhänge und
  Stimmproben in den üblichen Formaten kein ffmpeg mehr.
- Probe `ein_m4a_wandelt_das_system` (nur macOS: ein erzeugtes m4a wird zu
  24 kHz und einem Kanal, Unsinn ist kein Ergebnis; Gegenprobe mit zwei
  Kanälen rot). Von Ende zu Ende mit dem echten `myl` und einem ffmpeg-Spion
  vorn im PATH: m4a und FLAC wörtlich mitgeschrieben, der Spion nie gerufen;
  ein Ogg geht an ihn. `myl-senses` 133 grün, Clippy sauber.

### v0.224.0 (2026-11-12): Ein Fuzzer für Wörter und Keystore (`myl-client` 0.130.2)

- Neu `myl-client/fuzz/` mit zwei Zielen, deren Rumpf in
  `tests/fuzzziele/mod.rs` steht und die auch der stabile Regressionslauf
  `tests/fuzzkorpus.rs` fährt (dasselbe Muster wie in `myl-types`):
  `woerter` prüft, dass jede angenommene Folge bis auf Groß- und
  Kleinschreibung und Leerraum die kanonische ist; `keystore` prüft, dass
  jede Datei sich liest oder sauber scheitert, sich zu sich selbst
  zurückschreibt und beim Entschlüsseln endet (nur bei kleinen
  Ableitungskosten, sonst nur gelesen).
- Saaten aus den Prüfwörtern von BIP-39 und den Prüfvektoren von
  EIP-2335; für `keystore` ein Wörterbuch (`keystore.dict`), denn Bytes
  mutieren JSON schlecht: Ein eingebautes `unwrap` auf die Länge des IV
  fand der Fuzzer ohne Wörterbuch in 60 s nicht, mit Wörterbuch und
  Wertprofil nach 115 000 Läufen.
- Gegenproben: Präfix-Treffer in `entropie` und das `unwrap` am IV, beide
  gefunden. Läufe je 600 s ohne Fund (2,1 und 2,9 Millionen Eingaben).
- Die Sperrdatei des Fuzz-Pakets ist die von `myl-client`, ergänzt um
  `arbitrary`, `libfuzzer-sys` und `jobserver`; deren Archive liegen im
  lokalen Vorrat.

### v0.223.1 (2026-11-12): Der Verlauf fragt nach, wenn der Knoten drosselt (`myl-client` 0.130.1)

- 📌 **In der Probe von Ende zu Ende gefunden** (Probenetz, zwei Wallets,
  eine Überweisung): Der Verlauf des Empfängers stimmte, der des Senders
  direkt danach endete mit der nackten Zeile `HTTP/1.1 503 Service
  Unavailable`. Der Knoten liefert höchstens eine Seite je 100 ms, an alle
  Fragenden zusammen; dasselbe trifft Konsole und Fenster am selben Knoten.
  Jetzt fragt der Client eine gedrosselte Seite bis zu fünfmal im Abstand
  von 150 ms nach und sagt sonst in einem Satz, dass der Knoten drosselt.
  Andere Fehler wiederholt er nicht.
- Probe `eine_gedrosselte_seite_wird_nachgefragt` (Gegenprobe mit einem
  Versuch rot); `myl-client` 226 grün, Clippy sauber.

### v0.223.0 (2026-11-12): Ein WAV wandelt der Client selbst (`myl-senses` 0.40.0)

- Ein WAV jeder Breite, Rate und Kanalzahl, die `ton` liest, wird ohne
  ffmpeg auf die gewünschte Rate und einen Kanal gebracht
  (`hoeren::nach_wav_mit`). Vorher ging auch ein WAV über ffmpeg: Ohne
  ffmpeg ließ sich keine Stimmprobe ablegen, auch keine als WAV, und für
  ihre Mitschrift brauchte es ffmpeg ein zweites Mal. ffmpeg bleibt für
  andere Formate (m4a, mp3, ogg, flac).
- whisper.cpp bekommt ein WAV unverändert nur noch mit 16 kHz und einem
  Kanal; jedes andere lesbare WAV wird vorher gewandelt. Ein Kopf, den
  `ton` nicht liest, geht wie bisher unverändert durch (die Aufnahme-Attrappe
  der Proben fand genau diesen Fall).
- Proben `ein_wav_wandelt_sich_ohne_ffmpeg` (44,1 kHz, zwei Kanäle zu
  24 kHz, einem Kanal, Dauer und Pegel; Gegenprobe ohne den eigenen Weg
  rot), `ein_anderes_format_geht_weiter`,
  `nur_sechzehn_kilohertz_einkanalig_geht_unveraendert`; `myl-senses` 132
  grün, Clippy sauber.

### v0.222.1 (2026-11-12): Ein stehendes Skript spricht auch einzelne Sätze (`myl-senses` 0.39.1)

- 📌 **Beim Gegenlesen der Nacht gefunden:** `sagen` startete ein
  `sprechen-dauer` für einen einzelnen Satz mit Pfaden statt mit
  `--dauer`; dokumentiert ist nur das Gespräch. Ein Skript, das sich an
  die Doku hält, endete ohne Ton. Jetzt geht auch ein einzelner Satz über
  das Gespräch (starten, ein Satz, beenden); gekennzeichnet ist er dabei
  schon, und `sagen` erkennt das.
- Probe `ein_stehendes_skript_spricht_auch_einen_einzelnen_satz`
  (Gegenprobe: ohne die Umleitung „keine Tondatei“); `myl-senses` 129
  grün, Clippy sauber.

### v0.222.0 (2026-11-12): Verstümmelungsproben und ein Befund im Keystore (`myl-client` 0.130.0)

- 📌 **Ein Keystore mit verändertem IV öffnete sich still mit einem
  anderen Schlüssel.** Gefunden mit der neuen Verstümmelungsprobe: Die
  Prüfsumme nach EIP-2335 deckt Schlüsselteil und Geheimtext, nicht den
  IV, und AES-CTR entschlüsselt mit falschem IV zu anderen Bytes. Die Norm
  verweist dafür auf das Feld `pubkey`. **Jetzt prüft `wallet::entsperren`
  den abgeleiteten öffentlichen Schlüssel gegen `pubkey`** und meldet
  sonst „Der Keystore ist beschädigt“. Ohne Passwort lässt sich das
  falsche Geheimnis nicht kennen, also auch kein passendes `pubkey`
  schreiben. Probe `ein_veraenderter_iv_wird_beim_entsperren_erkannt`
  (zeigt zugleich, dass die Norm allein ihn nicht erkennt; Gegenprobe:
  unverändert entsperrt).
- **7.15, Fuzzing der Parser** (ohne Nightly, mit festem Generator):
  `jede_verstuemmelung_scheitert_sauber_oder_ist_kanonisch` (4 000
  Abwandlungen der Normvektoren von BIP-39; angenommen wird nur, was genau
  die kanonischen Wörter ergibt) und
  `ein_verstuemmelter_keystore_scheitert_sauber` (1 500 Abwandlungen:
  gekippte Zeichen, abgeschnittene Texte, Grenzwerte in jedem
  KDF-Parameter; kein Absturz, keine unbegrenzte Rechnung). Für den
  Adresstext ein Fuzz-Ziel in `myl-types` (SHARED_TYPES 0.68.3), dort auch
  die Testfälle 1 bis 3 von EIP-2333.
- Offen aus 7.15: die externe Prüfung; der Fuzzer mit `libfuzzer-sys`
  (nicht im Vorrat).
- Proben: `myl-client` 225, `myl-types` 379, Clippy sauber; `myl-console`,
  `myl-oberflaeche` und `myl-testclient` übersetzen.

### v0.221.0 (2026-11-12): Die zwölf Wörter geschützt (`myl-client` 0.129.1, `myl-oberflaeche` 0.92.0)

- **7.14, Schutz in der Oberfläche:** Solange die Wörter eines neuen
  Wallets offen sind, ist das Fenster vor Aufnahmen geschützt (Tauri
  `set_content_protected`: unter macOS `NSWindow.sharingType = none`, unter
  Windows `WDA_EXCLUDEFROMCAPTURE`; unter Linux gibt es das nicht). Gesetzt,
  bevor die Wörter hinausgehen, aufgehoben bei Bestätigen und Verwerfen.
- **Nie in die Zwischenablage:** Im Entwurf sind Kopieren, Ausschneiden,
  Ziehen und das Kontextmenü gesperrt, die Liste lässt sich nicht
  markieren. Nach Bestätigen oder Verwerfen stehen auch die zwei
  abgefragten Wörter nicht mehr in ihren Feldern.
- **Sperre nach Untätigkeit:** Zwei Minuten ohne Eingabe, und der Entwurf
  ist verworfen; angelegt ist dann nichts.
- **Überschrieben statt liegengelassen:** Ein Entwurf überschreibt beim
  Verwerfen Seed und Wörter (`zeroize`), ebenso die Zwischenstände beim
  Würfeln und Herstellen und der Seed als Hex vor dem Schreiben der
  Schlüsseldatei. ⚠️ Was über die Brücke ins Fenster ging, liegt in
  dessen Speicher, bis die Seite es freigibt; das kann Rust nicht
  überschreiben.
- Offen aus 7.14: nichts für Konsole und `myl` (Schirm im Terminal; dort
  löscht die Seite die Wörter vor der Abfrage). Debug-Log: Wörter standen
  nie darin (nachgesehen: nur Gesprächstext).
- Probe `die_zwoelf_woerter_sind_geschuetzt` (Gegenprobe: ohne Schutz beim
  Entwurf rot); `myl-client` 222, `myl-console` 192, `myl-oberflaeche` 93,
  Clippy sauber.

### v0.220.0 (2026-11-11): Wallet-Verlauf in Konsole und Fenster (`myl-client` 0.129.0, `myl-console` 0.72.0, `myl-oberflaeche` 0.91.0)

- **7.4, Rest:** Der Verlauf je Wallet, bisher nur `myl wallet verlauf`,
  steht jetzt auch in der Konsole (`/wallet`, auf einer Wallet-Reihe die
  Taste `v`, eigener Schirm, so viele Einträge wie passen, dann der
  Hinweis auf `myl`) und im Fenster (Knopf „Verlauf“ je Wallet, klappt
  unter der Reihe auf und übersteht das Neuzeichnen; Befehl
  `wallet_verlauf`, jetzt dreiundachtzig Befehle).
- ⚑ **Eine Stelle für die Zeile:** `Verlaufszeile::text` und
  `VERLAUF_GRENZE` in `myl_client::wallet`; `myl`, Konsole und Fenster
  zeigen dieselben Zeilen und dieselbe Grenze der Auskunft (im Block heißt
  nicht gelungen, Prägungen stehen nicht darin).
- ⚠️ **Offen: Prägungen im Verlauf.** Sie entstehen im Epochenwechsel, nicht
  als Anweisung im Block; sie aufzuzeichnen berührt den Konsenspfad des
  Knotens und kommt nach dem Einfrierpunkt.
- Proben: `die_zeile_nennt_richtung_und_gegenseite`,
  `der_verlauf_passt_auf_den_schirm` (mit Gegenprobe); `myl-client` 222,
  `myl-console` 192, `myl-oberflaeche` 92, `myl-testclient` übersetzt. Ohne
  Wallet und laufenden Knoten auf dieser Maschine nicht von Ende zu Ende
  gefahren; das folgt in der Generalprobe.

### v0.219.0 (2026-11-11): CosyVoice entfernt (`myl-senses` 0.39.0, `myl-oberflaeche` 0.90.0)

- ⚑ **CosyVoice ist kein Sprechweg mehr** (Festlegung des
  Projektinhabers, 2026-11-11). Gründe: Die Modellkarte von CosyVoice 3
  nennt neben Apache 2.0 „nur für akademische Zwecke“, dazu Python und
  PyTorch als Laufzeit und 7 bis 9 s Laden. Der eigene Weg (Qwen3-TTS,
  Apache 2.0 ohne Zusatz) war seit v0.215.0 die Vorgabe: Wortfehler gleich,
  erster Ton 1,08 statt 1,58 s, Blindvergleich 3 zu 2 für den eigenen Weg.
  ⚠️ **Bewusst angenommen:** die Stimmähnlichkeit mit CAM++ von 0,76 statt
  0,84. Der Maßstab begünstigt CosyVoice, das CAM++ selbst zum Steuern
  nutzt.
- **Die Reihenfolge:** eigenes Skript je Satz, eigenes Skript stehend,
  Qwen3-TTS, piper. `MYL_SPRECHEN=fremd` schließt den eigenen Weg aus und
  lässt Skript oder piper.
- **Neu: `<Heimat>/bin/sprechen-dauer`**, ein eigenes Skript, das stehen
  bleibt (`Sprechweg::Dauerskript`). Es spricht über dasselbe Gespräch
  wie bisher der CosyVoice-Läufer (`bereit`, je Satz eine Zeile, Stücke,
  `ok`), mit Kennzeichnung jedes Stücks. So bleibt der Dauerläufer für
  fremde Modelle offen, und die schnellen Proben für Vorleser, Stücke,
  Vorrang, Vorwärmen, Überbrückung und Kennzeichnung prüfen ihn weiter,
  jetzt mit einem gestellten Skript statt eines gestellten Python.
- **Entfernt:** der mitgebrachte Läufer `sprechen-cosyvoice.py` samt
  `laeufer_einrichten`, Auffrischen früherer Fassungen und
  Python-Suche; `MYL_COSYVOICE`, `COSYVOICE_NAMEN`, `PYTHONPROGRAMME`; in
  der Oberfläche der Knopf „Läufer anlegen“ und der Befehl
  `sinne_einrichten` (jetzt zweiundachtzig Befehle).
- **Fehlt etwas, nennt die Meldung Qwen3-TTS** und das
  Einrichtungsskript. Liegen die Gewichte und sind nur mit `fremd`
  abgewählt, sagt sie das, statt zum Holen zu raten. 📌 Hören und Sprechen
  nannten als Ablageort den ersten Gewichtsort, im Klon `MODELS/vision`;
  jetzt `MODELS/audio`. Probe `hoeren_und_sprechen_zeigen_nach_audio`.
- **Das Einrichtungsskript holt Qwen3-TTS** statt CosyVoice: Revision
  `5d83992436eae1d760afd27aff78a71d676296fc`, sechs Dateien mit
  Prüfsumme (Sprecher, Tonzerleger, je `config.json`, Wortschatz,
  Verschmelzungen), rund 2,5 GB statt 8 GB. Gegen die liegenden Dateien
  geprüft (alle Summen stimmen); ein frischer Download ist nicht gefahren.
  Die feste Paketliste der CosyVoice-Umgebung (`sprechen-fest.txt`,
  `sprechen.in`) ist entfernt. Eine frühere Installation bleibt liegen,
  `--pruefen` nennt sie.
- Die Agentenprobe `w_transcribe_audio` nimmt die Modellstimme aus
  `tests/daten/qwen3tts-stimmprobe.wav` statt einer Aufnahme aus dem
  CosyVoice-Ordner, der nie im Klon lag.
- Proben: `myl-senses` 97 schnelle und 27 mit Attrappe, groß
  `eigensprecher` (der Vorleser spricht mit dem eigenen Weg); `myl-client`
  221, `myl-console` 191, `myl-oberflaeche` 92; `myl-testclient`, `cad`
  und `local-agent` übersetzen. Gegenprobe auf einem Rechner mit alter
  CosyVoice-Installation: `myl sinne` mit `MYL_SPRECHEN=fremd` greift sie
  nicht mehr auf.

### v0.218.2 (2026-11-11): C2PA besteht `c2patool` (`myl-senses` 0.38.2)

- 📌 **Die Prüfseite meldete auch die zweite und dritte Testdatei als
  ungültig.** Ohne Fehlercode dort wurde das offizielle `c2patool` 0.27.23
  (`c2pa-rs` 0.90.23) lokal geprüft: `claimSignature.mismatch` mit
  `CoseX5ChainMissing`, obwohl die Signatur mit `openssl` stimmt. Eingegrenzt
  mit Zertifikaten aus `openssl`, die `c2patool` selbst signiert: **Ohne
  Organisation (`O`) im Namen des Signaturzertifikats** erzeugt auch
  `c2patool` eine ungültige Datei; der Hauptzweig von `c2pa-rs` verlangt
  das nicht mehr, die ausgelieferten Fassungen schon.
- **Das Zertifikat trägt jetzt `CN=Myelith, O=Myelith`**, dazu die
  Aussteller-Schlüssel-ID (`c2pa-rs` verlangt sie bei jedem Zertifikat,
  sonst `signingCredential.invalid`), und `x5chain` steht als Liste wie bei
  `c2patool`. Weiter selbst signiert, frisch je Datei.
- Ergebnis mit `c2patool`: drei Sätze `Valid`, einziger Hinweis
  `signingCredential.untrusted` (kein Aussteller der Vertrauensliste). Die
  Prüfseite der Content Authenticity Initiative zeigt die vierte Testdatei
  jetzt an: „Dieser Audioclip wurde mit einem KI-Tool generiert“, Anwendung
  und KI-Werkzeug „Myelith“, Unterzeichner „Nicht erkannt“.
  Gegenprobe: ein Byte im Ton ergibt `assertion.dataHash.mismatch`, eines im
  Anspruch `claimSignature.mismatch`.
- Proben `das_zertifikat_traegt_was_c2pa_rs_verlangt` (Profil, `O` in
  Aussteller und Inhaber, Unterschrift mit dem eigenen Schlüssel, Gegenprobe
  mit fremdem) und `die_kette_ist_eine_liste`.

### v0.218.1 (2026-11-11): C2PA-Bindung wie die Referenz (`myl-senses` 0.38.1)

- 📌 **Die Prüfseite der Content Authenticity Initiative meldete die erste
  Testdatei als „möglicherweise manipuliert“.** Ursache: Das Manifest war
  1423 Byte lang, RIFF hängt dann ein Füllbyte an, und es stand im
  ausgenommenen Bereich (1432 statt 1431). Die Referenzumsetzung `c2pa-rs`
  (`riff_io.rs`, `claim_verify.rs`) verlangt genau `Länge + 8` und hasht das
  Füllbyte mit. Gelesen im Quelltext, nicht ausgeführt.
- **Jetzt ist das Manifest immer gerade lang**, über das Feld `pad` der
  Bindung; der Block braucht kein Füllbyte, der Bereich ist eindeutig.
  Die eigene Prüfung verlangt wie `c2pa-rs`, dass die Ausnahme genau der
  Block ist. Probe `der_block_ist_gerade_und_genau_ausgenommen` (fünf
  Dateilängen; Gegenprobe: ohne Füllung wäre das Manifest ungerade).
- Zweite Testdatei dem Projektinhaber zur Prüfung geschickt.
- `myl-senses` 0.38.1.

### v0.218.0 (2026-11-11): Signierte Herkunft nach C2PA, Wasserzeichen leiser (`myl-senses` 0.38.0, `myl-client` 0.128.0)

- **Jede erzeugte Tondatei trägt ein C2PA-Manifest** (Modul
  `kennzeichnung/c2pa.rs`, Spezifikation 2.2), die erste der zwei Schichten,
  die der Verhaltenskodex der Kommission zu Art. 50 vorsieht: Ablage `c2pa`
  mit Manifest `urn:c2pa:<uuid>`; Behauptungen `c2pa.actions.v2`
  (`c2pa.created`, Quellentyp `trainedAlgorithmicMedia`, Werkzeug
  Myelith) und `c2pa.hash.data` (SHA-256 über die Datei ohne den Block);
  Anspruch `c2pa.claim.v2`; Signatur `COSE_Sign1` mit EdDSA, Zertifikat im
  geschützten Kopf, Nutzlast abgetrennt; in WAV als Block `C2PA` am Ende.
  CBOR (deterministisch), JUMBF und das X.509-Zertifikat in DER sind eigener
  Code; Ed25519, SHA-256 und Zufall aus `ed25519-dalek` 2, `sha2` 0.10 und
  `getrandom` 0.2 (wie `myl-module`). Signiert wird zuletzt im Pflichtweg,
  nach Wasserzeichen und Pausen, also auch jedes gestreamte Stück.
- ⚑ **Ein frischer Schlüssel je Datei** (Festlegung des Projektinhabers):
  Ed25519 mit selbst signiertem Zertifikat (EKU `c2pa-kp-claimSigning` und
  `emailProtection`, `digitalSignature`, gültig bis 9999), damit Dateien
  eines Nutzers nicht verknüpfbar sind. ⚠️ Ohne Zertifikat einer CA auf der
  C2PA-Vertrauensliste belegt die Signatur, dass Ton und Angaben seit dem
  Signieren unverändert sind, nicht wer signierte; Prüfprogramme melden
  einen unbekannten Signierer.
- **Prüfen:** `kennzeichnung::pruefen` liefert `Befund::herkunft`
  (`Fehlt`, `Gueltig`, `Ungueltig(Grund)`): Signatur, Hashes der
  Behauptungen, Hash über den Ton, KI-Aktion. `myl kennzeichen` zeigt es an
  und wertet es mit (Rückgabe null, wenn eine Marke gefunden ist).
- ⚠️ **Noch nicht gegen das Prüfwerkzeug der C2PA geprüft**; offen auch,
  ob der Kopf des Blocks `C2PA` zum ausgenommenen Bereich gehört (die
  Spezifikation sagt es für RIFF nicht; hier wie bei PNG mit Kopf).
- **Wasserzeichen leiser:** ein 80stel statt eines 64stels (Wunsch des
  Projektinhabers nach dem Hörtest, in dem er die rohe Fassung fünfmal
  von fünf vorzog). Sätze abgeschnitten und nach MP3 10,0 und 10,1
  Standardabweichungen (das Anderthalbfache der Schwelle), Stücke am Anfang
  6,3; hörbar unter 20 dB 0,39 statt 0,86 Prozent.
- 📌 Die erste Signierung brach ab: Die Länge des Blocks steht selbst im
  Manifest und wächst als CBOR-Zahl mit ihrem Wert; jetzt gerechnet, bis
  sie sich bestätigt. 📌 Ein Fehlalarm der Sperrprobe kam von einem
  gesetzten `CARGO_HOME` (das Lager kennt `libfuzzer-sys` nicht).
- Beispiel `kennzeichnen`: eine Tondatei über den Pflichtweg kennzeichnen,
  für Proben gegen fremde Prüfwerkzeuge.
- Proben: `signiert_ist_gueltig_und_aenderungen_fallen_auf` (Ton, Anspruch,
  zweimal signiert), `je_datei_ein_frischer_schluessel`,
  `cbor_deterministisch_und_umkehrbar`; `eigensprecher` verlangt an jedem
  Stück ein gültiges Manifest. Sperrdateien der abhängigen Kisten und
  `COMPLIANCE/NOTICES.md` nachgezogen.

### v0.217.0 (2026-11-11): Ein Wasserzeichen, das der Stimme folgt (`myl-senses` 0.37.0)

- **Auftrag des Projektinhabers:** ein Wasserzeichen ganz ohne hörbares
  Rauschen, nach kritischer Prüfung gegen die Fachliteratur. Geprüft:
  neuronale Wasserzeichen (AudioSeal als robustestes nach AudioMarkBench,
  NeurIPS 2024) lassen sich durch neuronale Codecs, Sprachverbesserung und
  gezielte Angriffe entfernen, brauchen Gleitkomma und einen Download; für
  Sprache ist die Formung durch die lineare Vorhersage etabliert (Patent
  „Spread spectrum signaling for speech watermarking“; Looney und
  Gaubitch, Interspeech 2024). Der Verhaltenskodex der Kommission zu Art. 50
  erwartet ein unhörbares Wasserzeichen als eine Schicht neben signierten
  Metadaten. ⚠️ Die Metadaten von Myelith sind nicht signiert (offen).
- **Einprägen:** je Fenster von 20 ms die lineare Vorhersage der Stimme
  (Ordnung 16, ganzzahlig nach Levinson-Durbin, Q30), die Folge durch
  `1/A(z/γ)` mit γ 0,97 (die Rauschformung der CELP-Sprachcodecs), Zustand
  über die Fenster, Stärke ein 64stel der Lautstärke, gleitend.
- **Erkennen:** je Fenster die Vorhersage des empfangenen Signals, der Rest
  auf das Zweifache seines Medianbetrags begrenzt und auf gleiche
  Lautstärke gebracht, gefaltet, verglichen. Das alte weiße Wasserzeichen
  wird weiter erkannt; die feste binomische Formung (nur am 2026-11-11 für
  Zwischendateien) nicht mehr.
- **Gemessen** an 15 Sätzen mit geklonter Stimme, **je Fenster und Band**
  (zuvor im Mittel über Dateien, das die feste Formung gut aussehen ließ):
  hörbar laut und weniger als 20 dB unter der Stimme in 0,86 Prozent der
  Fenster und Bänder (weiß 46, fest geformt 34), fünftes Perzentil in jedem
  Band mindestens 19,6 dB (weiß −6,0 dB über 8 kHz, fest geformt 6,8 dB bei
  2 bis 4 kHz). Erkennung Sätze 13,5, abgeschnitten 12,4, nach MP3 12,3;
  Stücke 7,9, 7,1, 7,0 (Schwellen 4,0 und 6,5); ohne Wasserzeichen
  höchstens 4,0. 📌 Ohne die Begrenzung kam ein Sägezahn von 0,2 s (Probe
  `ein_alter_satz_aus_der_ablage…`) auf 2,6 statt 9,7; die Probe blieb, wie
  sie war.
- Hörtest für den Projektinhaber: je Satz roh, neu und alt weiß als
  Kontrolle.
- Proben: `das_wasserzeichen_folgt_der_stimme` (unter tiefen Tönen über
  4 kHz unter einem Hundertstel, jedes Fenster 20 dB unter dem Signal;
  Gegenprobe weiß), Messprobe `messen_alle` (alle Varianten unter einem
  Maß) mit `#[ignore]`.
- `myl-senses` 0.37.0.

### v0.216.0 (2026-11-11): Kennzeichnung ohne Zischen, Pausen beruhigt (`myl-senses` 0.36.0)

- **Anlass:** Der Projektinhaber hörte im Blindvergleich „Rauschen und
  Artefakte im Hintergrund“. Gemessen: Die Stimmprobe ist sauber (56 dB
  Abstand); das weiße Wasserzeichen lag über 8 kHz 9,9 dB **über** der
  Sprache (Qwen3-TTS erzeugt dort wenig), in den Pausen lag das Rauschen
  je nach Satz zwischen −77 und −44 dBFS.
- **Das Wasserzeichen ist geformt** (`kennzeichnung::FORMUNG`, Ordnung 6
  der Binomialzahlen als Tiefpass, ganzzahlig; `TEILER` 32): in jedem Band
  mindestens 17,9 dB unter der Sprache (weiß −9,9 dB über 8 kHz). Gemessen
  an 15 Sätzen aus Qwen3-TTS mit geklonter Stimme, verglichen mit weiß und
  mit Ordnung 2, 4, 6, 8 bei Teiler 11 bis 128: Sätze abgeschnitten und
  nach MP3 mindestens 10,7 Standardabweichungen (Schwelle 6,5), Stücke von
  0,96 s am Anfang 5,5 (Schwelle 4,0). ⚠️ Ein Stück, das vorne
  abgeschnitten wird, liegt mit 5,4 unter der gesuchten Schwelle; Stücke
  werden nur abgespielt, abgelegt wird der ganze Satz. Die Prüfung erkennt
  weiter auch die weiße Folge früherer Dateien.
- **Pausen beruhigt** (`kennzeichnung::pausen_beruhigen`, im Pflichtweg vor
  dem Einprägen, also für jeden Weg): ein Abwärtsexpander unter −42 dBFS,
  1:3, höchstens −12 dB, das Lauteste aus ±20 ms, schließt über 120 ms.
- Die Sätze zum Überbrücken tragen die Fassung der Kennzeichnung im Namen
  und entstehen mit der neuen neu.
- **Gemessen nach dem Umbau** (fünf Sätze, beide Wege über den Vorleser):
  Anteil über 8 kHz im Mittel −40 auf −48 dB; Wortfehler unverändert 0,141.
- 📌 **Korrektur zur Abnahme (v0.215.0):** „Ähnlichkeit 0,785 gegen 0,831“
  verglich ungekennzeichnete eigene Ausgaben mit gekennzeichneten von
  CosyVoice. Gleich behandelt: 0,755 bis 0,763 gegen 0,831 bis 0,847,
  derselbe Abstand wie unter Python ohne Kennzeichnung (0,79 bis 0,80 gegen
  0,86). ⚠️ CAM++ misst jedes Wasserzeichen, auch ein unhörbares (weiß mit
  einem 128stel kostete 0,021): In Pausen nahezu digitaler Stille ändern
  zwei Stufen seine logarithmischen Bänder. Für die Hörbarkeit zählen die
  Abstände je Band.
- Proben: `die_formung_haelt_die_hoehen_frei` (über 8 kHz unter einem
  Tausendstel; Gegenprobe weiß), `die_weisse_folge_wird_weiter_erkannt`,
  `die_pausen_werden_beruhigt` (Gegenprobe oberhalb der Schwelle);
  Messproben `messen_geformt` und `filter_einzeln_ablegen` mit `#[ignore]`.
- `myl-senses` 0.36.0.

### v0.215.0 (2026-11-11): Qwen3-TTS gestreamt und als Vorgabe (`myl-senses` 0.35.0)

- **Der Rückweg mit Zustand** (`tonzerleger::Rueckstrom`, Modul
  `tonzerleger/strom.rs`): Je kausaler Faltung die letzten Eingaben, je
  transponierter Faltung der Überhang, je Transformerebene Schlüssel und
  Werte im Fenster samt Stelle. `zurueck_mit` ist ein Strom mit allen
  Bildern, also ein Weg für beides. In `f32` bitgleich mit einem Zug, gleich
  wie gestückelt (Probe `der_strom_ist_bitgleich_mit_dem_ganzen`, 120 Bilder
  in fünf Stücken; Gegenprobe ohne Gedächtnis anders); bitgleich auch mit
  dem Stand davor. Lange Codes gehen in Stücken zu 300 durch denselben
  Strom, ohne den Bruch, den `chunked_decode` der Vorlage an der Grenze hat.
- **Gestreamt:** `sprecher::sprechen_mit` meldet jedes Bild; der
  Eigensprecher rechnet die Stimmprobe einmal beim Laden in den Strom und
  setzt je Satz eine Kopie fort, Stücke zu 12 Bildern. Der `Dauersprecher`
  erzeugt in einem Faden, kennzeichnet jedes Stück und reicht es weiter,
  während der Faden rechnet. Gestreamt wird, solange der vorige Satz unter
  1,15 lag; darüber am Stück (im Behälter stotterte es sonst: 50 s Lücken
  in 20 s Ton). ⚠️ Kein Neuversuch mehr an der Bildgrenze, denn was
  gestreamt ist, hat schon geklungen; ein solcher Lauf wird gemeldet.
- **Der Rückweg nach der Maschine:** mit Metal W8A16 mit zwei Stellen
  (`Tonzerleger::laden_w8a16`), sonst W8A8. Gemessen je 15 Sätze: der
  Verlust an Ähnlichkeit lag am Rückweg in W8A8 (0,760; Rückweg in `f32`
  0,785, Sprecher in `f32` 0,764, beides 0,790), W8A16 0,785; gegen `f32`
  66 dB statt 20 dB. Auf 4 CPU-Kernen W8A16 dreieinhalbmal so langsam.
- **Abnahme gegen CosyVoice bestanden** (fünf Sätze mit der Stimmprobe des
  Projektinhabers, über den Vorleser): Wortfehler 0,141 wie CosyVoice,
  erster Ton 1,07 bis 1,09 s statt 1,51 bis 1,58 s, eine Antwort aus fünf
  Sätzen ohne Lücke (beide), Laden 1,2 statt 6,7 bis 9,5 s; Ähnlichkeit
  0,785 gegen 0,831 (CAM++, der Abdruck von CosyVoice; Stand des Modells
  unter Python 0,79 bis 0,80). ⚑ **Qwen3-TTS (eigen) ist die Vorgabe**
  (`laufwerk::EIGEN_ALS_VORGABE`), CosyVoice mit `MYL_SPRECHEN=fremd`.
- Referenzklasse (Behälter): Verhältnis 1,49, 1,56 GB, je Satz am Stück,
  erster Ton 6,6 s. ⚠️ Offen: W8A8 der Kernkiste auf der CPU.
- 📌 Zwei Messfehler unterwegs, beide vor einem Schluss bemerkt: Im
  Beispiel schluckte `--genau` das nächste Argument und sprach `--seed 11`
  mit (behoben, `next_if`); zsh teilt Variablen ohne Anführungszeichen
  nicht, eine Auswertung bekam zwölf Ordner als einen Pfad.
- Beispiel `vorlesen` misst Lücken (Abspielen nachgestellt); `sprechen`
  mit `--genau sprecher|rueckweg` und `--rueckweg w8a16`.
- `myl-senses` 0.35.0.

### v0.214.0 (2026-11-11): Qwen3-TTS als Sprechweg angebunden (`myl-senses` 0.34.0)

- **Neuer Weg `Sprechweg::Eigen`** (Modul `myl_senses::eigensprecher`):
  Qwen3-TTS im Prozess, ohne Python. Der `Dauersprecher` hält Zerleger,
  Sprecher und Rückweg geladen (1,2 s statt 6,7 bis 9,5 s für CosyVoice);
  die Stimme wird einmal aus der Stimmprobe vorbereitet, mit ihrem Text
  geklont mit Mitschrift, sonst nur über den Abdruck. Jeder Satz geht durch
  die Kennzeichnung wie bei jedem Läufer. Auch `sagen` und die Sätze zum
  Überbrücken nehmen den Weg. Seed je Satz frisch und mit Fingerabdruck des
  Satzes auf `stderr`; `MYL_SPRECHEN_SEED` legt ihn fest.
- **Ohne Sprache** (`sprechen` mit `None`, `auto` der Vorlage): Der
  Vorleser kennt die Sprache eines Satzes nicht, das Modell erkennt sie am
  Text. Gegen die Referenz Vorlauf 3,9·10⁻⁷, 40 von 40 Bildern gleich;
  Gegenprobe mit Deutsch anders.
- **Wahl:** eigenes Skript, dann `MYL_SPRECHEN=eigen` (Qwen3-TTS vorn),
  CosyVoice, piper, zuletzt Qwen3-TTS als Rückfall, wenn sonst nichts
  liegt; `MYL_SPRECHEN=fremd` schließt ihn aus. ⚑ **Noch nicht Vorgabe**
  (`laufwerk::EIGEN_ALS_VORGABE`), siehe Abnahme.
- **Abnahme gegen CosyVoice** auf dieser Maschine, fünf Sätze mit der
  Stimmprobe des Projektinhabers über den Vorleser (neues Beispiel
  `examples/vorlesen.rs`, vorgewärmt wie die Oberfläche), Rückhören mit
  whisper.cpp, Ähnlichkeit mit CAM++:

  | | eigen (int8) | CosyVoice |
  |---|---|---|
  | Wortfehler | **0,097** | 0,141 |
  | Ähnlichkeit | 0,756 | 0,831 |
  | erster Ton | 2,5 bis 4,4 s | 1,5 bis 1,6 s |
  | Verhältnis | 1,12 bis 1,33 | 0,77 bis 0,94 |

  Gegenmessung `f32` gegen int8, je zwei Seeds: Ähnlichkeit 0,785 gegen
  0,758 im Mittel (Python-Weg desselben Modells 0,79 bis 0,80), Wortfehler
  gleich. ⚠️ CAM++ ist der Abdruck von CosyVoice und begünstigt es.
  **Nicht bestanden:** erster Ton, Tempo mit Stimmprobe, Ähnlichkeit.
- **Rückweg in Stücken wie die Vorlage** (`chunked_decode`: 300 Bilder,
  25 Bilder Zusammenhang). 📌 Hinter der Stimmprobe nur 25 Bilder als
  Zusammenhang wäre schneller gewesen, wich aber über den ganzen Satz um 20
  bis 30 dB ab; der Rückweg rechnet die ganze Stimmprobe mit, bitgleich mit
  der Umwandlung beider zusammen.
- Proben: `eigensprecher` (groß, über den echten Vorleser, ohne und mit
  Stimmprobe und `sagen`: gekennzeichnet, wörtlich zurückgehört; fester
  Seed, nachdem ein Lauf mit frischem Seed einen Satz anders hören ließ),
  `ohne_sprache_waehlt_der_sprecher_dieselben_codes`, zwei schnelle zur
  Wahl des Wegs. Alle Proben grün; `myl-client`, `myl-console`,
  `myl-oberflaeche` übersetzen unverändert.
- `myl-senses` 0.34.0.

### v0.213.0 (2026-11-11): Qwen3-TTS mit frischem Seed je Satz (`myl-senses` 0.33.0)

- ⚑ **Jedes Mal ein neuer Seed, und er wird mitgeschrieben** (Festlegung
  des Projektinhabers, 2026-11-11): natürliche Abwechslung bei gleichem
  Text; mit Text, Stimme, Seed und Gewichten lässt sich jeder Lauf
  nachrechnen. ⚠️ Bitgleich nur auf demselben Rechenweg (Metal und CPU,
  NEON und AVX2 runden in `f32` verschieden); die Sinne liegen außerhalb
  des Konsenses.
- **`sprecher::sprechen_frei`**: zieht einen frischen Seed
  (`frischer_samen`, Uhr und Zähler durch `splitmix64`), erreicht ein Lauf
  die Bildgrenze, zieht es neu (höchstens zweimal), und gibt den Seed des
  Laufs zurück, der zählt.
- Das Beispiel `sprechen` spricht so und gibt den Seed aus; `--seed <zahl>`
  rechnet einen Lauf nach. Geprüft: nachgerechnet bitgleich, ein zweiter
  Lauf mit frischem Seed ergibt anderen Ton.
- `myl-senses` 0.33.0.

### v0.212.0 (2026-11-11): Qwen3-TTS auf der Referenzklasse (`myl-senses` 0.32.0)

- **Gemessen im Behälter** (`--cpus=4 --memory=4g`, aarch64, ohne Netz,
  int8, Stichprobe): vier Sätze (deutsch, englisch, ein Absatz von 17 s)
  mit Verhältnis Rechenzeit zu Tondauer **1,48 bis 1,55**, Spitzenspeicher
  der cgroup **2,07 GB**; auf dem Mac zurückgehört alle vier wörtlich.
  Gegenlauf mit genau vier sichtbaren Kernen (`--cpuset-cpus=0-3`): 1,66,
  die Quote verteilt also nicht zu viele Fäden. Zum Vergleich der fremde
  Weg (`qwen-tts` in `f32` auf 4 CPU-Fäden): 1,2 bis 1,8.
- **Was im Rückweg auf einem Kern lief, läuft jetzt auf allen**
  (`whisper::je_kern`): Umordnen der Faltungseingabe, Zurückordnen der
  Ausgabe, SnakeBeta, Verteilen der transponierten Faltung, die Umlagen
  zwischen Kanälen und Zeitpunkten, das Runden auf int8 im W8A8-Weg. Je
  Zeile oder Kanal unabhängig, Ton bitgleich. Mac 1,85 auf 1,12 s (ohne
  Metal 2,27 auf 1,66 s), Behälter 4,34 auf 3,77 s.
- ⚠️ **Im Behälter bleibt der Rückweg der größte Teil** (rund 70 Prozent):
  das W8A8-Produkt der Kernkiste auf der CPU, etwa 7 GMAC/s je Kern.
  Zwischenstufen `block1` bis `block4` in `zurueck_mit` zum Messen.
- `myl-senses` 0.32.0.

### v0.211.0 (2026-11-11): Qwen3-TTS in int8, schneller als Echtzeit (`myl-senses` 0.31.0)

- **Gemessen vor dem Umbau** (neues Beispiel `examples/sprechen.rs`, Zeit
  je Teil): 28 s für 2,8 s Ton, davon 17,9 s in der Code-Vorhersage.
  📌 `linear_genau` verteilte über die Eingabezeilen; bei einem Token
  rechnete ein Kern die ganze Matrix (9 GB/s). **Jetzt verteilt es bei
  wenigen Eingaben die Ausgaben** über den Fadenpool der Kernkiste, mit
  derselben Summe je Ausgabe: Ton bitgleich, 28 s auf 8 s. Der exakte
  int8-Weg für wenige Eingaben nimmt denselben Pool statt eines neuen
  Fadens je Aufruf.
- **Der Sprecher rechnet in int8** (`Sprecher::laden`; `laden_genau`
  für die Referenzproben): alle Projektionen und Köpfe mit einer Stelle je
  Gewicht (`Ganzmatrix::eine_stelle`), int8 mal int16, exakt in i64, wie
  die Gewichte des Sprachmodells. Der Ganzzahlweg nimmt dafür auch
  Eingaben über 32 767 an (negative Exponenten; im Sprecher gemessen bis
  124 883), für whisper unverändert bitgleich.
- **Die Faltungen der Decoderblöcke des Rückwegs in W8A8**
  (`Tonzerleger::laden`; `laden_genau` für die Referenz), wie der Encoder
  von whisper: 3,25 s auf 0,69 s. Die umgeordneten Gewichte der
  transponierten Faltungen entstehen einmal beim Laden.
- **Die Texteinbettung bleibt auf der Platte** und wird Zeile für Zeile
  gelesen (311 M Werte, 1,24 GB in `f32`; ein Satz braucht ein paar
  Dutzend Zeilen). Spitzenspeicher 3,7 GB auf 1,95 GB (`f32` 5,2 GB).
- ⚑ **Im Betrieb mit Stichprobe** (`sprecher::Wahl::Zufall(samen)`):
  top-k 50, Temperatur 0,9 für Sprecher und Code-Vorhersage, wie die
  Vorgabe der Vorlage, Zufall über `splitmix64` der Kernkiste. 📌 **Gierig
  fanden 11 von 13 Sätzen kein Ende**, in `f32` wie in int8 (die
  Referenzumsetzung rechnet gierig gleich, das Modell selbst schleift);
  ein erster Lauf ohne Grenze hielt 4,6 GB. `sprecher::bildgrenze` (6 je
  Token und 25) hält jede Schleife an. Gierig bleibt für die Proben gegen
  die Referenz.
- **Abnahme an 13 Sätzen** (9 deutsch, 4 englisch, darunter ein Absatz von
  17 s), Rückhören mit dem eigenen whisper large-v3-turbo: `f32` und int8
  je **0 Wortfehler auf 182 Wörtern**; Verhältnis Rechenzeit zu Tondauer
  **0,96 in int8**, 2,83 in `f32` (vor dem Umbau 10). Gegenprobe: die
  gierigen Schleifenläufe hört dasselbe Werkzeug mit 100 und 79 Prozent
  Wortfehlern.
- Proben: `in_int8_mit_zufall_wird_der_satz_verstanden` (groß, in
  `grosse_proben` über `sprecher`), eine schnelle zur Wahl (nur unter den
  50 besten, Samen bestimmt die Wahl, gesperrte Codes nie, Gegenprobe
  anderer Samen). Alle großen Proben von whisper und Qwen3-TTS grün, die
  Referenzproben in `f32` weiter Code für Code gleich.
- `myl-senses` 0.31.0.

### v0.210.0 (2026-11-11): Qwen3-TTS klont mit Mitschrift (`myl-senses` 0.30.0)

- **Der Hinweg des Tonzerlegers** (`tonzerleger::hin`, Modul
  `tonzerleger/hin.rs`): aus 24 kHz werden Codes, ein Kodierer wie Mimi.
  SEANet (Faltung auf 64, viermal Restblock, ELU und Faltung mit Schritt
  4, 5, 6, 8, Faltung auf 512, alle kausal), acht Transformerebenen bei
  25 Hz (LayerNorm, RoPE, GELU, LayerScale), Verdichten auf 12,5 Hz,
  Restquantisierung mit einem semantischen und fünfzehn akustischen
  Codes. In `f32`, Vorlage `transformers` und `qwen-tts` (beide
  Apache 2.0), neu geschrieben. ⚠️ Die Vorlage wendet das Fenster aus
  `sliding_window` im Kodierer nicht an; dieser Weg folgt ihr.
- **`sprecher::Stimme`**: Abdruck und, zum Klonen mit Mitschrift, die
  Mitschrift der Stimmprobe als Token samt ihren Codes. Der Vorlauf legt
  Mitschrift und neuen Text auf die Textspur und die Bilder der Stimmprobe
  auf die Codespur, Stelle für Stelle wie die Vorlage; was vom Text
  übrig bleibt, kommt im Nachlauf. `sprechen` nimmt `Option<&Stimme>`.
- **`tonzerleger::zurueck_hinter`**: Codes hinter den Codes der
  Stimmprobe zu Ton, beide zusammen gerechnet und der Ton der Stimmprobe
  abgeschnitten, wie die Vorlage.
- **Gegen die Referenz:** Hinweg alle 560 Codes gleich (Stufen 1 bis
  3·10⁻⁵), 0,47 s für 2,8 s Ton; geklont Vorlauf (45 Stellen) bis
  8·10⁻⁷ und alle 30 Bilder Code für Code gleich; der eigene whisper hört
  den neuen Satz wörtlich zurück. Gegenprobe: um ein halbes Bild versetzt
  treffen 17 von 560 Codes. Probe `klonen` mit `#[ignore]`, in
  `grosse_proben`. Damit läuft das Klonen von der Stimmprobe bis zum Ton
  ganz im eigenen Code.
- Die Aufmerksamkeit des Rückwegs ist eine eigene Funktion, die der
  Hinweg mitbenutzt; der Rückweg gleicht der Referenz wie zuvor.
- `myl-senses` 0.30.0.

### v0.209.0 (2026-11-11): Qwen3-TTS nimmt eine Stimme an (`myl-senses` 0.29.0)

- **Neues Modul `myl_senses::stimmabdruck`**: der Sprecherabdruck von
  Qwen3-TTS, ein ECAPA-TDNN (Faltung auf 512, drei SE-Res2Net-Blöcke mit
  Dilatation 2, 3, 4, Zusammenführung auf 1536, aufmerksame Statistik,
  Projektion auf 1024). Davor ein eigenes Log-Mel bei 24 kHz (Fenster 1024,
  Schritt 256, 128 Bänder nach Slaney bis 12 kHz, Betrag, natürlicher
  Logarithmus). In `f32`, Vorlage das Paket `qwen-tts` (Apache 2.0), neu
  geschrieben. `spektrum::mel_filter_fuer` baut die Filter für jede
  Fensterbreite; der Weg von whisper ruft sie mit seinen alten Werten.
- **`sprecher::sprechen` nimmt einen Abdruck an** (`Option`): Er steht im
  Vorlauf zwischen Denkende und Füllung, wie in der Vorlage; ohne Abdruck
  spricht das Modell mit seiner eigenen Stimme wie bisher.
- **Gegen die Referenz:** Mel 8·10⁻⁸, Abdruck 1,4·10⁻⁶, und mit dem
  Abdruck **alle 35 Bilder Code für Code gleich** (ohne Abdruck weichen
  alle 35 ab, die Probe sieht also, ob er wirkt). Gegenprobe: um 0,1 s
  versetzt weicht der Abdruck um 1·10⁻² ab, das Hundertfache der Schranke.
  0,1 s für 2,8 s Ton. Probe `stimmabdruck` mit `#[ignore]`, in
  `grosse_proben`.
- ⚑ **Die Stimmprobe der Probe ist die Stimme des Modells**:
  `tests/daten/qwen3tts-stimmprobe.wav` (134 KB, 16 Bit) ist der Satz der
  Probe `sprecher`, vom Modell ohne Abdruck gesprochen. Kein Mensch, keine
  fremde Aufnahme.
- Offen für das Klonen mit Mitschrift (ICL): der Hinweg des Tonzerlegers
  (Ton zu Codes).
- `myl-senses` 0.29.0.

### v0.208.0 (2026-11-11): Qwen3-TTS zerlegt den Text selbst (`myl-senses` 0.28.0)

- **Neues Modul `myl_senses::textzerleger`**: das byteweise BPE von Qwen
  aus `vocab.json` und `merges.txt`, ohne Fremdkiste. Vorzerlegen nach dem
  Muster von Qwen2 von Hand (Kürzel, Buchstabenfolgen mit höchstens einem
  Zeichen davor, einzelne Ziffern, Zeichenfolgen, Leerraum mit den Regeln
  für Zeilenenden und das letzte Leerzeichen vor einem Wort), dann die
  Merges nach Rang. NFC für zerlegt geschriebene Akzente in lateinischer
  Schrift (Gravis, Akut, Zirkumflex, Tilde, Trema, Ring, Cedille).
  `vorlage` baut die Eingabe des Sprechers.
- **Gegen die Referenz** (`transformers` mit der Einstellung von
  `qwen-tts`): 22 Sätze in zehn Sprachen, darunter Chinesisch, Japanisch,
  Koreanisch, Kyrillisch, Emoji, Tabulatoren, mehrfacher Leerraum und
  Kürzel in Großschrift, alle Token gleich; die Vorlage gleich; ein Satz in
  NFD wie in NFC. Gegenprobe: ohne Merges gleicht kein Satz. Laden 0,09 s.
  Probe `textzerleger` mit `#[ignore]` (braucht den Wortschatz aus
  `MODELS/audio`), in `grosse_proben`.
- **Die Probe `sprecher` beginnt jetzt beim Text**: Die Vorlage kommt aus
  dem eigenen Zerleger und gleicht den IDs der Referenz; danach wie zuvor
  alle 35 Bilder gleich und wörtlich zurückgehört. Damit läuft der Weg
  Text → Token → Codes → Ton → Text ganz im eigenen Code.
- ⚠️ **Grenze:** Andere zerlegte Zeichen als die genannten Akzente bleiben
  zerlegt (keine Unicode-Tabellen); `\p{L}` wird über `is_alphabetic`
  ohne Zahlbuchstaben nachgebildet, was bei einigen kombinierenden Zeichen
  indischer Schriften abweichen kann. Beides betrifft keine Sprache von
  Qwen3-TTS mit lateinischer, kyrillischer oder ostasiatischer Schrift.
- `myl-senses` 0.28.0.

### v0.207.0 (2026-11-11): Qwen3-TTS spricht im eigenen Rechenweg (`myl-senses` 0.27.0)

- **Neues Modul `myl_senses::sprecher`**: der Sprecher von Qwen3-TTS
  (Decoder wie Qwen3, 28 Ebenen, RMSNorm je Kopf auf `q` und `k`, GQA mit
  16 zu 8 Köpfen, RoPE mit Basis 10⁶) samt Textspur (Projektion der
  Qwen-Einbettung von 2048 auf 1024) und Code-Vorhersage (5 Ebenen, je Code
  ein eigener Kopf). Der Vorlauf baut beide Spuren wie die Referenz auf
  (Rolle, Denken, Sprache, Denkende, Füllung, Beginn), jeder Schritt
  addiert die sechzehn Codes des letzten Bildes und das nächste Text-Token.
  Gierig mit Wiederholungsstrafe 1,05, gesperrten Codes und Mindestlänge
  wie `generate`. In `f32` auf der CPU, Vorlage das Paket `qwen-tts`
  (Apache 2.0), neu geschrieben.
- **Gegen die Referenz:** der letzte Zustand des Vorlaufs bis 4·10⁻⁷, und
  **alle 35 Bilder Code für Code gleich**, mit natürlichem Ende. **Von Ende
  zu Ende ohne Python:** Text → eigener Sprecher → eigener Tonzerleger →
  eigener whisper ergibt wörtlich „Guten Morgen, der Bericht über die
  Nacht liegt bereit.“ (Probe `sprecher`, mit `#[ignore]`, in
  `grosse_proben`; Referenz ohne Sprecherabdruck, also keine Stimme eines
  Menschen).
- Der Safetensors-Leser liest BF16.
- ⚠️ **Tempo:** 24,6 s für 2,8 s Ton, der Tonzerleger 3,5 s dazu; noch
  ohne jede Beschleunigung. Offen: Textzerleger (die IDs kommen in der
  Probe aus der Referenz), Sprecherabdruck und Hinweg des Tonzerlegers für
  das Klonen, int8 über die Kernkiste, die Referenzklasse.
- `myl-senses` 0.27.0.

### v0.206.0 (2026-11-11): Qwen3-TTS, erster Teil: der Tonzerleger macht aus Codes Ton (`myl-senses` 0.26.0)

- ⚑ **Anlass:** Qwen3-TTS-12Hz-0.6B-Base ersetzt CosyVoice für das
  Sprechen (Festlegung des Projektinhabers nach Recherche und Messung:
  Apache 2.0 ohne Zusatz, gleiche Wortfehler, schneller). Der Umbau in den
  eigenen Rechenweg beginnt mit dem Teil, der für sich prüfbar ist.
- **Neues Modul `myl_senses::tonzerleger`**: der Rückweg des Tonzerlegers
  (Qwen3-TTS-Tokenizer-12Hz) von sechzehn Codes je Bild (12,5 je Sekunde)
  zu 24 kHz: Codebücher, kausale Faltung, acht Transformerebenen der Breite
  512 (RMSNorm, RoPE, Fenster 72, SiLU-MLP, LayerScale), zweimal
  Hochtasten mit ConvNeXt, vier Blöcke mit SnakeBeta und Restblöcken,
  zusammen der Faktor 1920. In `f32` auf der CPU, Vorlage die
  Referenzumsetzung im Paket `qwen-tts` (Apache 2.0), neu geschrieben.
  Maße aus der `config.json` des Tonzerlegers.
- **Gegen die Referenz geprüft** (Probe `tonzerleger`, mit `#[ignore]`,
  in `grosse_proben`): nach den Codebüchern 2,6·10⁻⁷, nach dem
  Transformer 8,8·10⁻⁷, der Ton 8,4·10⁻⁵ relativ; Gegenprobe: um eine
  Probe verschoben weicht der Ton weit ab. ⚑ Die Codes der Referenz kommen
  aus einem festen Generator, nicht aus einem gesprochenen Satz: Der
  Rückweg nimmt beliebige Codes, und Ton aus einer geklonten Stimme gehört
  nicht ins Repositorium.
- Der Safetensors-Leser aus `whisper` nimmt einen Namensfilter und dient
  beiden Modellen.
- Gewichte unter `MODELS/audio/qwen3-tts-0.6b-base/` (Apache 2.0, Revision
  und Prüfsummen in `LIZENZEN.json`, nicht versioniert).
- Offen für das Sprechen: Sprecher (28 Ebenen), Code-Vorhersage,
  Sprecherabdruck, Hinweg des Tonzerlegers für die Stimmprobe, dann int8
  über die Kernkiste und die Messung auf der Referenzklasse.
- `myl-senses` 0.26.0.

### v0.205.0 (2026-11-11): Der eigene Hörweg wählt die Größe von whisper nach der Maschine (`myl-senses` 0.25.0)

- **`whisper::groesse_waehlen`**: large-v3-turbo nur mit Grafikeinheit
  (Metal) oder ab 12 Kernen, sonst small, zuletzt base; jede Größe nur,
  wenn sie vorliegt und die Speicherwache des Clients zustimmt (dieselbe,
  die heute vor whisper.cpp fragt). `MYL_HOEREN_MODELL=<ordner>` legt die
  Größe fest. ⚑ Die Regel folgt der Messung von 0.204.0: auf 4 Kernen ohne
  Grafikeinheit small 3,6 s je Satz, large-v3-turbo 17 s; mit Metal
  large-v3-turbo 2,4 s.
- Über den echten Weg `hoeren::mitschreiben` geprüft: hier (15 Kerne,
  Metal) large-v3-turbo, im Behälter mit 4 Kernen und 4 GB small, je mit
  richtigem Text.
- Probe `die_groesse_folgt_der_maschine` (Grafikeinheit, viele Kerne,
  schwache Maschine, Wache lehnt ab; Gegenprobe: fehlt small, fällt die
  Wahl nicht auf large-v3-turbo). 114 Proben grün, Clippy sauber.
- `myl-senses` 0.25.0.

### v0.204.0 (2026-11-11): whisper in jeder Größe, auch auf schwacher Hardware (`myl-senses` 0.24.0)

- ⚑ **Anlass:** Die Sinne laufen nur lokal und auch auf sehr schwacher
  Hardware; Referenzklasse 4 Kerne, 4 GB, ohne Grafikeinheit (Festlegung
  des Projektinhabers). Dafür rechnet `myl_senses::whisper` jetzt **jede
  Größe von whisper** über denselben Weg: `Masse` aus `config.json`
  (Breite, Köpfe, Ebenen, Bänder, Fenster, Stellen; Kopfbreite 64 wird
  geprüft), `Sondertoken` aus `added_tokens.json` (die älteren Größen
  kennen eine Sprache weniger, alles dahinter liegt um eins verschoben),
  Gewichte in F16 oder F32. Die Proben gegen `transformers` für
  large-v3-turbo liefern bitgleich dieselben Zahlen wie vorher.
- **Laden Tensor für Tensor von der Platte** (`kopf_lesen`,
  `tensor_lesen`): Je Faden liegt nur ein Tensor in `f32`, die Matrizen
  werden sofort zerlegt. 📌 Vorher lag die ganze Datei im Speicher, daneben
  alles in `f32`: im Behälter mit 4 GB whisper small 2,9 GB Spitze,
  large-v3-turbo brach ab. Jetzt 0,84 GB und 2,1 GB.
- **Gemessen** (Hörprobe, 16 Aufnahmen; Wortfehler Deutsch, Deutsch mit
  Rauschen, Englisch; Zeit je Satz hier mit Metal und im Behälter mit 4
  Kernen und 4 GB ohne Grafikeinheit):

  | Größe | Deutsch | Rauschen | Englisch | hier | 4 Kerne, 4 GB |
  |---|---|---|---|---|---|
  | tiny | 26/140 | 21/23 | 3/42 | 0,34 s | – |
  | base | 9/140 | 17/23 | 1/42 | 0,47 s | 1,06 s, 0,88 GB |
  | small | 1/140 | 5/23 | 1/42 | 1,10 s | 3,64 s, 0,84 GB |
  | large-v3-turbo | 0/140 | 0 bis 1/23 | 0/42 | 2,44 s | 17,4 s, 2,1 GB |

  ⚠️ Vier Kerne des M5 Pro sind deutlich schneller als die eines Raspberry
  Pi 5; die Zeiten im Behälter sind eine untere Grenze, Speicher und
  Machbarkeit sind belegt. Für Deutsch ist **small die kleinste brauchbare
  Größe**.
- Neue Gewichte unter `MODELS/audio/` (`whisper-tiny`, `-base`, `-small`,
  Apache 2.0, Revision und Prüfsumme in `LIZENZEN.json`); alle drei
  gleichen `transformers` Token für Token, genau wie mit W8A8.
- Das Beispiel `hoeren` nimmt `--modell <ordner>`.
- Proben: `die_masse_kommen_aus_der_konfiguration`,
  `die_sondertoken_folgen_der_datei`, `tensoren_kommen_einzeln_von_der_platte`
  (je mit Gegenprobe) und `whisper_small_waehlt_dieselben_token` (Referenz
  `tests/daten/whisper-small-pruefsignal.json`). 113 schnelle und 8 große
  Proben grün, Clippy sauber.
- `myl-senses` 0.24.0.

### v0.203.0 (2026-11-11): whisper rechnet W8A8 (`myl-senses` 0.23.0)

- ⚑ **Die Bündel von whisper (Encoder, Gedächtnis des Decoders) rechnen
  W8A8 als Vorgabe**, entschieden vom Projektinhaber nach einem Versuch:
  jedes Fenster mit seiner Zweierpotenz nach int8, auch mit negativem
  Exponenten (viele Aktivierungen liegen über 127), mal der ersten
  Gewichtsstelle, über `linear_w8a8_pc_stapel` der Kernkiste (INTEGER_LLM
  0.150.0). Der Decoder rechnet Token für Token weiter exakt.
  `Gewichte::laden_w8a16` behält den genaueren Weg.
- **Der Preis, gemessen:** gegen `transformers` 8,0·10⁻² an den Stellen und
  0,55 bei den Normen statt 4,9·10⁻⁴ und 1,5·10⁻², der erste Logit 5,12
  statt 4,573. Die Token am Prüfsignal und an der Hörprobe bleiben gleich.
  Vor dem Bau simuliert: eine zweite Stelle brachte bei 8-Bit-Eingaben
  nichts (1,1·10⁻¹), Skalen je 128er-Gruppe nur einen Faktor zwei.
- **Der Gewinn:** Hörprobe (16 Aufnahmen, Release) im Mittel **2,44 s**
  statt 2,92 s, kein Wortfehler, alle Eigennamen; ein Aufruf mit
  Programmstart und Laden 2,84 s. Am Morgen waren es rund 6 s.
- Proben: `w8a8_rechnet_die_matrix` (lautes Fenster, leiser Kanal),
  `der_w8a8_encoder_bleibt_in_der_naehe` (Schranke beim Dreifachen,
  Gegenprobe: unter 10⁻² rechnete nicht W8A8),
  `der_w8a8_decoder_waehlt_dieselben_token`; die W8A16-Proben halten ihre
  strenge Schranke. 110 schnelle und 7 große Proben grün.
- `myl-senses` 0.23.0.

### v0.202.0 (2026-11-11): whisper schneller, ohne die Kernkiste anzufassen (`myl-senses` 0.22.0)

- ⚑ **Erst gemessen, dann umgebaut**, und im Release-Profil, denn das wird
  ausgeliefert. 📌 Die früheren Zeiten dieses Punkts (4,6 s je Satz) kamen
  aus dem Testprofil mit Überlaufprüfung; dort sind enge Schleifen
  deutlich langsamer. Release, ein Satz warm, vorher: Kernaufrufe 2,0 s,
  Aufmerksamkeit 1,24 s, LayerNorm, GELU und Umrechnen zusammen 0,4 s,
  Decoder 0,3 s; Laden 1,6 s.
- **Beide Gewichtsstellen in einer Matrix**, untereinander (die zweite mit
  Verschiebung `s + 8`): ein Aufruf, ein Befehlspuffer und eine Zerlegung
  der Eingabe statt zwei. Kernzeit 2,26 auf 1,93 s.
- **Die Aufmerksamkeit in Registerkacheln** (4 Anfragen × 8 Schlüssel für
  die Bewertungen, 4 Anfragen × 16 Stellen fürs Mischen), jede Summe in
  derselben Reihenfolge wie vorher: **bitgleich**, der genaue Weg gleicht
  `transformers` mit exakt denselben Abweichungen (2,1146382·10⁻⁶,
  9,861133·10⁻⁵). Release 1,24 auf 0,88 s. ⚠️ Im Testprofil ist sie
  langsamer (2,9 s), die Proben brauchen dadurch etwas länger.
- **Laden über alle Kerne** (f16 nach f32 und das Zerlegen in Stellen, je
  Faden der nächste Tensor): 1,6 auf 0,31 s.
- **Ergebnis im Release-Profil:** ein Satz warm 3,9 auf 3,4 s, ein Aufruf
  über `hoeren` mit Laden rund 6,1 auf 4,4 s. Hörprobe (16 Aufnahmen):
  unverändert kein Wortfehler, alle Eigennamen, im Mittel 3,8 s;
  whisper.cpp 0,5 s.
- ⚠️ **Was bleibt, liegt in der Kernkiste** (abgetastet): Von 1,9 s
  Kernzeit wartet die CPU rund 1,3 s auf die Grafikeinheit, die je
  int16-Eingabe zwei int8-Hälften rechnet (W8A16), und rund 0,5 s gehen
  ins Lesen der Ausgabe. Nachgetragen am selben Tag: Das Lesen lag an
  frischen Metal-Puffern je Aufruf (INTEGER_LLM 0.149.4, kernels 0.90.1);
  seither im Mittel 2,92 s je Aufnahme der Hörprobe.
- `myl-senses` 0.22.0.

### v0.201.0 (2026-11-11): Der eigene Hörweg auf Zuruf (`myl-senses` 0.21.0)

- **`hoeren::mitschreiben` nimmt mit `MYL_HOEREN=eigen` den eigenen
  whisper**, wie `MYL_AUFNAHME=eigen` beim Aufnehmen. whisper.cpp bleibt
  Vorgabe, bis das Tempo abgenommen ist. Die Gewichte liegen im Ordner
  `whisper-large-v3-turbo` neben dem Hörmodell und werden je Aufruf
  geladen (rund 1,7 s): Ein Sinn ist nur geladen, solange er hört.
- **`whisper::text`**: Sprache als Kürzel oder `auto`, wie bisher. `auto`
  erkennt sie wie whisper (ein Schritt nach dem Anfang, das größte Logit
  unter den 100 Sprachtoken aus `added_tokens.json`, `Sprachen`). Längere
  Aufnahmen gehen in Stücken zu 30 s durch. ⚠️ An der Grenze zweier Stücke
  kann ein Wort zerfallen; whisper setzt dort mit Zeitmarken neu an, die
  dieser Weg noch nicht liest.
- ⚑ **Eine Pegelwache statt der Stilleregel von whisper** (`gesprochen`):
  Ein Stück geht nur durch den Decoder, wenn mindestens 0,3 s davon in
  Abschnitten zu 20 ms lauter als −45 dBFS sind. 📌 whisper verwirft ein
  Stück, wenn `<|nospeech|>` nach dem Anfang wahrscheinlich ist; bei
  large-v3-turbo liegt das bei 10⁻¹⁰, auch für reine Stille, mit
  `transformers` bestätigt. Ohne Wache brachte ein Rest von 0,13 s hinter
  30 s Sprache „Vielen Dank.“; mit ihr endet die Mitschrift richtig, und
  reine Stille ergibt die bekannte Meldung.
- `mitschreiben_aus` gibt eine `Mitschrift` mit dem Mittel der
  Log-Wahrscheinlichkeiten zurück; `mitschreiben` bleibt, wie es war.
- Gefahren über `hoeren::mitschreiben` (Aufnahmen nicht im
  Repositorium): Deutsch und Englisch mit `auto` richtig erkannt und
  wortgetreu, 6,2 bis 7,0 s je Satz samt Laden; ein Absatz von 30 s in
  8,1 s.
- Proben: `die_sprachen_kommen_aus_den_zusatztoken`,
  `stille_wird_nicht_mitgeschrieben` (mit Gegenprobe an der Grenze, 0,3 s
  gegen 0,28 s).
- `myl-senses` 0.21.0.

### v0.200.0 (2026-11-11): whisper rechnet über die Kerne des Sprachmodells (`myl-senses` 0.20.0)

- **Die Matrizen in int8 über `integer-llm-kernels`**, auf macOS
  gebündelt mit Metal: dieselbe Rechenschicht wie das Sprachmodell, wie im
  Umbauplan festgelegt. `myl-senses` hat damit eine Abhängigkeit, die
  eigene; keine Fremdkiste kommt dazu. `Gewichte::laden` liefert den
  ganzzahligen Weg, `Gewichte::laden_genau` behält `f32` als Referenz für
  die Probe gegen `transformers`.
- ⚑ **Drei Entscheidungen, jede gemessen** (Encoder nach 32 Ebenen gegen
  `transformers`, Stellen und Normen):
  - Gewichte in **zwei int8-Stellen** (der Rest der ersten mal 256, derselbe
    Kniff wie die Zerlegung im Metal-Weg). Mit einer Stelle wären es
    3,3·10⁻² und 2,7·10⁻¹, für 0,45 s Ersparnis.
  - Eingaben in int16 mit einer **Skala je Fenster**, Ausgaben mit
    **Nachkommabits je Kanal**, die sich selbst einstellen (geklemmt: neu
    mit weniger; unter einem Viertel des Bereichs: neu mit mehr). Mit je
    einer Skala für alles waren es 1,4·10⁻² und 1,2·10⁻¹.
  - Wenige Eingaben (der Decoder, Token für Token) rechnen exakt in i64
    über `dot_i8_i16`; eine Anpassung je Kanal hätte dort dauernd
    nachgerechnet.
  - Die Bits stellen sich beim ersten Aufruf einer Matrix ganz ein,
    danach **träge**: neu gerechnet wird erst unter einem Sechzehntel des
    Bereichs (11 Bit), sonst merkt sich der Kanal die feineren Bits für den
    nächsten Aufruf. Vorher rechnete jede neue Aufnahme ihre Matrizen
    doppelt (5,4 s statt 4,6 s je Satz); nur träge von Anfang an kostete
    die erste Aufnahme Genauigkeit (3·10⁻³ statt 6·10⁻⁴).
  - Ergebnis: **5,8·10⁻⁴ und 1,6·10⁻²**, Schranke beim Dreifachen. Die Token
    sind am Prüfsignal und am gesprochenen Satz dieselben wie bei
    `transformers`, die Logits des Decoders bis 4·10⁻⁴.
- **Der Decoder mit Zwischenspeicher** (`Verlauf`, `weiter`) statt die
  ganze Folge je Schritt neu zu rechnen, und mit den vollen **448 Stellen**
  statt 64 Token: Ein Stück von 30 s Sprache bringt leicht über 100 Token.
- **Die Aufmerksamkeit des Encoders in Blöcken zu 16 Anfragen**, die
  Schlüssel je Kopf quer: Einzeln las jede Anfrage die 768 KB ihres Kopfes
  neu (23 GB je Ebene), 3,8 s von 5,5. GELU auf allen Kernen. Der genaue
  Weg rechnet dieselbe Aufmerksamkeit und gleicht `transformers`
  unverändert (2,1·10⁻⁶, 9,9·10⁻⁵).
- **Hörprobe nach dem Messplan** (16 Aufnahmen aus Systemstimmen, nicht
  im Repositorium: neun deutsche Sätze, einer davon ein Absatz von 25 s,
  vier englische, drei davon zusätzlich mit Rauschen bei 10 dB): **kein
  Wortfehler** auf Deutsch und Englisch, alle 11 Eigennamen richtig. Die
  einzige Abweichung, „halb 9“ statt „halb neun“ im verrauschten Satz,
  schreibt auch der genaue Weg, also `transformers`; whisper.cpp schreibt
  „neun“. **Die Güte ist gleich.**
- ⚠️ **Das Tempo nicht:** im Mittel **4,6 s** je Aufnahme (am Morgen
  41,9 s), der Encoder warm 3,2 s; whisper.cpp 0,5 s samt Start (Encoder
  0,26 s, mit 5-Bit-Gewichten). Die Lücke schließt erst eine Kette, die
  ganz auf der Grafikeinheit bleibt; die Abnahme von 8.2 steht damit aus.
- Das Beispiel `hoeren` nimmt jetzt mehrere Aufnahmen, `--sprache de|en`,
  `--genau` und `--referenz`.
- Proben: `ganzzahlig_wie_genau` (gebündelt und einzeln, ein lautes
  Fenster und ein leiser Kanal, je unter 10⁻⁴ und 10⁻³) mit der Gegenprobe
  `ohne_zweite_stelle_waere_es_ungenau`; in `whisper_encoder` dazu
  `der_ganzzahlige_encoder_bleibt_nah` und
  `der_ganzzahlige_decoder_waehlt_dieselben_token`.
- `myl-senses` 0.20.0.

### v0.199.0 (2026-11-11): Der Decoder von whisper, ein Satz wortgetreu (`myl-senses` 0.19.0)

- **Der Decoder in `myl_senses::whisper`**: vier Ebenen mit kausaler
  Selbstaufmerksamkeit und Kreuzaufmerksamkeit auf den Encoder, Kopf über
  die geteilte Tokeneinbettung (51 866 Wörter). `mitschreiben` beginnt mit
  Anfang, Sprache, Mitschreiben, ohne Zeiten, wählt gierig das größte
  Logit und beachtet die gesperrten Token aus `generation_config.json`
  (88 immer, zwei nur am Anfang). `Woerterbuch` macht aus den Token Text
  (Bytetabelle von GPT-2, `vocab.json`). Vorlage `transformers`, neu
  geschrieben; die Kiste bleibt ohne Abhängigkeit.
- **Gegen `transformers` geprüft:** am Prüfsignal dieselben sechs Token,
  die Logits gleich bis in die sechste Stelle (Probe
  `der_decoder_waehlt_dieselben_token`). Ein gesprochener deutscher Satz
  (Systemstimme, nicht im Repositorium) kommt mit allen 24 Token gleich
  heraus: „Der Bericht über die Nacht liegt bereit, und die Prüfrunde war
  grün.“ whisper.cpp schreibt denselben Text.
- **Beispiel `hoeren`** (`cargo run --example hoeren -- <wav> [<referenz>]`)
  für Aufnahmen, die nicht ins Repositorium gehören; mit einer Referenz
  endet es rot, wenn ein Token abweicht (Gegenprobe mit falscher Referenz:
  Rückgabewert 1).
- ⚠️ **Tempo:** 41,9 s für den Satz auf der CPU, der Decoder rechnet die
  ganze Folge je Schritt neu. Noch nicht in `hoeren` verdrahtet; die
  Rechenschicht mit Metal steht aus.
- `myl-senses` 0.19.0.

### v0.198.0 (2026-11-11): Der Encoder von whisper ohne fremde Laufzeit (`myl-senses` 0.18.0)

- **Neues Modul `myl_senses::whisper`**: der Encoder von whisper
  large-v3-turbo aus dem Log-Mel des Spektrums, in `f32` auf der CPU,
  verteilt auf alle Kerne. Zwei Faltungen mit GELU, Lageeinbettung, 32
  Ebenen (LayerNorm, Aufmerksamkeit mit 20 Köpfen zu 64, Vorwärtsschicht
  1280 → 5120 → 1280), LayerNorm am Ende. Die Gewichte liest ein eigener
  Safetensors-Leser (JSON-Kopf, f16 nach f32 genau); die Kiste bleibt ohne
  Abhängigkeit. Vorlage `transformers`, neu geschrieben.
- **Gegen `transformers` geprüft** (Referenz einmal in `f32` aus den
  Originalgewichten erzeugt, unter `tests/daten/`): Einbettung, Ebene 0 und
  Ebene 1 bis 1,7·10⁻⁶ relativ (Schranke 10⁻⁴), der ganze Encoder bis
  2,1·10⁻⁶ an drei Stellen und 9,9·10⁻⁵ bei den Normen aller 1500 Fenster
  (Schranke 10⁻³). Gegenprobe: mit 31 statt 32 Ebenen 63 Prozent
  Abweichung, rot. Dazu f16 erschöpfend über alle 65 536 Werte, der
  JSON-Kopf und erf.
- ⚠️ **Tempo:** 38 s für ein Stück von 30 s auf der CPU; whisper.cpp
  braucht mit Metal 0,8 s für einen ganzen Satz. Der Weg dorthin ist die
  gemeinsame Rechenschicht. Noch nicht verdrahtet; Decoder und Zerleger
  fehlen.
- Beide Proben (`whisper_encoder`, mit `#[ignore]`, brauchen die 1,5 GB
  Gewichte) stehen in `grosse_proben` von `.github/ci/kisten.sh`.
- `myl-senses` 0.18.0.

### v0.197.0 (2026-11-11): Das Mikrofon auch unter Linux und Windows (`myl-senses` 0.17.0)

- **Linux:** ALSA wird zur Laufzeit geladen (`dlopen("libasound.so.2")`,
  Gerät aus `MYL_ALSA_GERAET`, sonst `default`), ALSA tastet selbst auf 16
  kHz um. Der Bau braucht weder `libasound2-dev` noch eine Kiste. Im
  Behälter ohne Netz geprüft: mit der Laufzeitbibliothek zehn Stücke vom
  Gerät `null`; Gegenproben: ein unbekanntes Gerät meldet „ALSA öffnet …
  nicht“, ohne die Bibliothek „ALSA fehlt … Rückfall ist ffmpeg“, und
  gebaut wird trotzdem. ⚠️ `null` taktet nicht nach der Uhr (über eine
  Milliarde Proben in 0,5 s); die Probe zählt deshalb Stücke.
- **Windows:** `waveIn` aus `winmm`, drei Puffer, ein Faden fragt ab,
  welcher fertig ist (`CALLBACK_NULL`, nichts im Treiberfaden). Lokal mit
  `--target x86_64-pc-windows-msvc` geprüft, Clippy ohne Befund. ⚠️ Nur
  übersetzt; eine Aufnahme unter Windows hat noch niemand gemacht.
- Die Kiste bleibt ohne Abhängigkeit; alle drei Anbindungen sind eigene
  FFI-Deklarationen. Die Vorgabe bleibt ffmpeg, bis die Abnahme besteht.
- `myl-senses` 0.17.0.

### v0.196.0 (2026-11-11): Aufnehmen ohne ffmpeg, auf Zuruf (`myl-senses` 0.16.0)

- **Neues Modul `myl_senses::mikrofon`**: eine dünne Anbindung je System,
  16 kHz, ein Kanal, 16 Bit in Stücken von 0,1 s. Gebaut für macOS über
  die AudioQueue aus AudioToolbox (Teil jedes macOS, nichts zu
  installieren); Linux (`libasound.so.2`, zur Laufzeit geladen) und
  Windows (`winmm`) folgen. Entschieden nach einer Prüfung von `cpal`:
  Dessen Linux-Teil verlangt zum Bauen die Entwicklerdateien von ALSA, und
  ein frischer Klon soll überall ohne Netz bauen.
- **`aufnahme::starten` nimmt mit `MYL_AUFNAHME=eigen` den eigenen Weg**:
  ein Faden sammelt die Stücke, meldet denselben Ausschlag wie bisher (60
  dB Spanne), hält nach fünf Minuten von selbst an, und `beenden` schreibt
  das WAV über `ton::schreiben`. Die Vorgabe bleibt ffmpeg, bis die
  Abnahme besteht.
- Proben: der Ausschlag auf der Skala von ffmpeg (Stille, Vollaussteuerung,
  −30 dB in der Mitte, Vorzeichen egal). ⚠️ Die echte Aufnahme
  (`eine_sekunde_vom_mikrofon`, `#[ignore]`) fragt beim ersten Mal nach der
  Freigabe des Mikrofons und ist noch nicht gefahren; die Abnahme gegen
  ffmpeg steht aus. 102 Proben grün, Clippy sauber.
- `myl-senses` 0.16.0.

### v0.195.2 (2026-11-11): Jede Störung an jeder Stelle (`myl-client` 0.127.1, `myl-module` 0.6.1)

- **Keystore:** `hin_und_zurueck_und_jede_stoerung_faellt_auf` kippte eine
  Ziffer des Geheimtexts; jetzt jede Stelle des Geheimtexts und jede der
  Prüfsumme, jede wird abgewiesen.
- **Modulsignatur:** `jede_aenderung_faellt_auf` änderte eine Datei; jetzt
  wird jedes Byte von Anleitung, Programm und Beschreibung einzeln
  gekippt, jedes fällt auf, und der Ausgangsstand gilt danach wieder.
  `daten/` bleibt mit Absicht unsigniert.

### v0.195.1 (2026-11-11): Die Mel-Filter auch gegen whisper.cpp (`myl-senses` 0.15.1)

- Neue Probe `tests/mel_wie_whisper_cpp.rs` (mit `#[ignore]`, braucht
  `MODELS/audio/hoeren.bin`): liest Kopf und Mel-Filter aus der
  Gewichtsdatei von whisper.cpp (large-v3-turbo, 128 Bänder, 201
  Frequenzen) und vergleicht mit `spektrum::mel_filter`. Größte Abweichung
  2,3·10⁻⁹ bei einem Größtwert von 0,042, also Rundung in `f32`; Schranke
  10⁻⁶ relativ. Gegenprobe mit um ein Band verschobenen Filtern: rot. Eine
  zweite, unabhängig erzeugte Referenz neben `transformers`.
- Die Probe steht in `grosse_proben` von `.github/ci/kisten.sh`; `grosse`
  fährt jetzt fünf Proben (gefahren, Rückgabe 0).
- `myl-senses` 0.15.1.

### v0.195.0 (2026-11-11): Der Ton an einer Stelle (`myl-senses` 0.15.0)

- **Neues Modul `myl_senses::ton`** (Baustein T der Sinne): WAV aus seinen
  Blöcken lesen (PCM mit 8, 16, 24 und 32 Bit, Gleitkomma mit 32 Bit,
  auch im erweiterten Format, beliebig viele Kanäle, zu einem gemischt),
  WAV mit 16 Bit schreiben, Umtasten mit gefenstertem Sinc bei rationalem
  Verhältnis (Grenze bei 0,95 der kleineren Nyquistfrequenz, 32
  Nulldurchgänge je Seite, Blackman).
- **Das Sprechen liest jetzt dort:** `wav_pcm16_mono` nimmt den Kopf aus
  `ton::kopf`, statt einen eigenen Leser zu führen. 📌 `wav_dauer_s` nahm
  einen festen Kopf von 44 Bytes an; mit einem `LIST`-Block vor den Daten,
  wie ffmpeg ihn schreibt, wäre die Dauer falsch gewesen. Jetzt aus den
  Blöcken.
- Fünf Proben: PCM16 bitgleich zurück, `LIST`-Block mit Füllbyte
  übersprungen, Stereo und Gleitkomma gemischt, 24 Bit gelesen, ein
  unbekanntes Format abgewiesen; 48 kHz auf 16 kHz und 16 auf 24 kHz mit
  weniger als −60 dB Fehler, Gegenprobe: ein Ton über der neuen Grenze um
  mehr als 60 dB gedämpft. 101 Proben der Kiste grün.
- Die Aufnahme selbst läuft weiter über ffmpeg; ohne sie braucht es den
  Zugriff auf die Schnittstellen der Systeme.
- `myl-senses` 0.15.0.

### v0.194.0 (2026-11-11): Das Spektrum wie whisper, ohne fremde Laufzeit (`myl-senses` 0.14.0)

- **Neues Modul `myl_senses::spektrum`**, der erste gemeinsame Baustein
  der Sinne: Log-Mel genau wie whisper es erwartet (30 s Stück, zentriert
  mit Spiegelrand, Hann-Fenster periodisch zu 400 Proben im Schritt 160,
  Leistungsspektrum, Mel-Filter nach Slaney bis 8 kHz, `log10`, Begrenzung
  auf das Maximum minus 8, `(x + 4) / 4`, 3000 Fenster). Eigene
  Fourier-Transformation mit gemischter Basis (400 = 2⁴·5²); die Kiste
  bleibt ohne Abhängigkeit. Vorlage war `WhisperFeatureExtractor` aus
  `transformers`, neu geschrieben.
- **Gegen die Referenz geprüft:** dasselbe ganzzahlige Prüfsignal durch
  `transformers` (einmal erzeugt, als `f32` unter `tests/daten/`), die
  ersten 101 Fenster: größte Abweichung 2,0·10⁻⁶ bei 80 und 3,7·10⁻⁶ bei
  128 Bändern, Schranke 2·10⁻⁵. Gegenprobe: um eine Probe verschoben,
  besteht es nicht. Dazu die Transformation gegen die direkte Summe für
  neun Längen und die Filter auf Abdeckung.
- Noch nicht im Hören verdrahtet; whisper.cpp bleibt der Weg, bis der
  ganze Encoder steht.
- `myl-senses` 0.14.0.

### v0.193.0 (2026-11-10): Rechtsläufige Schriften behalten ihre Zahlen (`myl-scan` 0.3.0)

- 🐛 Arabisch, Persisch, Hebräisch, Jiddisch und Syrisch: Beim Umdrehen der Zeile wurden auch Ziffern und lateinische Wörter umgedreht, aus 1325 wurde 5231. Sie bleiben jetzt ganz, Klammern wenden sich.
- 📌 Das arabische Lesemodell ist durch das Zeilennetz `script/Arabic` ersetzt: Das bisherige verlor westliche Ziffern in jeder gemessenen Schriftart. Das neue liest sie, Persisch steigt von 89,1 auf 99,8 % Zeichenähnlichkeit. Auf alten Drucken sind beide ähnlich schwach. Die Schriftwahl bleibt bei 59 von 60 Proben.

### v0.192.0 (2026-11-10): Lizenzen im Bündel, keine Schrift ohne Lizenz (`myl-oberflaeche` 0.89.0)

- Das Fensterbündel und das macOS-Skript legen `LICENSE.md`, `NOTICES.md` und `DRITTLIZENZEN.txt` unter `lizenzen/` ab.
- 📌 Der eingebettete Ausschnitt der Wortmarke ist entfernt; wo die Schrift installiert ist, erscheint sie weiter, sonst die Systemgrotesk.

### v0.191.0 (2026-11-10): Parrot ist ein Projektmodell (`myl-client` 0.127.0)

- **`parrot-2b-int8` steht im Katalog und lässt sich in der Modellwahl holen.** Wie bei jedem Projektmodell lädt die Installation die Gewichte bei fester Revision und baut das Artefakt selbst.
- **Die Modellempfehlung übergeht es**, auch wenn sonst nichts passt: Es übersetzt und führt kein Gespräch.
- **`myl buch` sagt, wenn es fehlt**, und übersetzt dann langsamer mit dem eingestellten Modell.

### v0.190.0 (2026-11-10): `myl buch` findet den Übersetzer, `myl fragen --ohne-grundsaetze` (`myl-client` 0.126.0)

- **`myl fragen --ohne-grundsaetze`** lässt die Grundsätze des Agenten weg. Ein Übersetzungsmodell kennt keine Rolle: Mit ihnen schrieb es Überlegungen oder Sätze über Myelith statt der Übersetzung, und die Rechnung wurde doppelt so lang.
- **`myl buch` reicht `parrot-2b-int8` als Übersetzer weiter**, wenn es hier gebaut liegt. Ohne bleibt es beim eingestellten Modell.

### v0.189.0 (2026-11-09): Schriftwahl zählt eigene Buchstaben, senkrechtes Koreanisch, Übersetzung (`myl-client` 0.125.0, `myl-scan` 0.2.0)

- 🐛 **Die Schriftwahl zählt nur noch, was wirklich Buchstaben der Schrift sind.** Die
  Sicherheit allein täuscht: Falsche Lesungen liefern Ziffern, Satzzeichen oder fremde
  Buchstaben, oft mit hoher Sicherheit. Das ersetzt die Schwelle für Zeilennetze aus
  0.188.1, die alte Drucke in diesen Schriften benachteiligte. Gemessen an 60 Proben, darunter
  zwölf echte alte Drucke und ihre stark verschlechterten Fassungen: 59 richtig. Eine
  Liste häufiger Wörter wurde gemessen und verworfen, sie machte die Wahl schlechter.
- 🐛 **Koreanisch steht auch senkrecht.** Ein Druck von 1914 las sich vorher mit 0,21 und
  jetzt mit 0,82.
- **`myl buch` gibt die Sprache des Nutzers weiter**, damit das Werk übersetzt und der
  Skill daraus gebaut wird. `--ohne-uebersetzung` spart die Rechnung.
- ⚠️ **Grenze:** Ein Werk liest eine Schrift. Bei zweisprachigen Büchern gewinnt die
  häufigere, die andere liest sich schlecht.

### v0.188.1 (2026-11-09): Schriftwahl und Titel an echten Scans berichtigt (`myl-client` 0.124.1, `myl-scan` 0.1.1)

- 🐛 **Ein Zeilennetz gewinnt die Schriftwahl nur noch ab 0,75 Sicherheit.** Auf einem
  arabischen Druck von 1888 gewann Tibetisch mit 0,62 gegen das arabische Lesemodell
  mit 0,56; das ganze Buch wäre Unsinn geworden. Auf ihrer eigenen Schrift erreichen
  die Netze mindestens 0,87, gemessen an sauberen Proben.
- 🐛 **Ein Titel muss mit 0,8 Sicherheit gelesen sein.** Verzierte Schrift auf einer
  Titelseite von 1775 gab „ie, üäseee =d,“, und das wurde der Name des Buchordners.
- **Bleibt auch die beste Schrift unter 0,7, sagt `myl scan` das** und nennt die
  Möglichkeit, sie festzulegen.
- ⚠️ **Grenze:** Ein schlecht erhaltener Druck in einer Netzschrift (Hebräisch, Syrisch,
  Bengalisch, Tibetisch) kann unter die Schwelle fallen und falsch zugeordnet werden.
  Echte Scans dieser Schriften lagen nicht vor; dann hilft `--schrift`.

### v0.188.0 (2026-11-09): gescannte Bücher lesen, `myl bild` (`myl-client` 0.124.0, `myl-scan` 0.1.0)

- ⚑ **Neue Kiste `myl-scan`, Befehl `myl scan --auftrag <datei>`:** liest gescannte
  Seiten ohne Netz und ohne Fremdprogramm. Die Modelle sind ins Programm
  eingebaut (Seitenaufbau, Zeilen finden, Zeilen lesen; 35 MB, auf 8 Bit gepackt, Herkunft in
  `myl-scan/modelle/HERKUNFT.md`), gerechnet wird in reinem Rust auf der CPU. Rund eine
  Sekunde je Seite. PDF-Seiten werden in Rust gezeichnet, auch JPEG-2000-, JBIG2- und
  CCITT-Scans.
- ⚑ **Sechzehn Schriften:** Latein, Fraktur, Kyrillisch, Neu- und Altgriechisch, Arabisch,
  Hebräisch, Syrisch, Devanagari, Bengalisch, Thai, Tamil, Telugu, Tibetisch, Chinesisch
  (mit Japanisch, auch senkrecht gesetzt) und Koreanisch. Die Schrift eines Werks wird an
  einigen Seiten erkannt oder fest genannt. Ein Werk liest eine Schrift.
- ⚑ **`myl buch <datei>` und `/book` in der Konsole:** aus einem Werk wird ein Skill. Das
  Werkzeug ist ins Programm eingebaut, gebraucht wird nur Python ab 3.9. Ziel ist
  `.AGENT/skills` des Arbeitsordners, ohne Arbeitsordner die eigenen Skills.
- ⚑ **Im Fenster wählt der Skillknopf** zwischen „Skill lernen“ und „Buch lesen“ (`myl-oberflaeche`
  0.88.0). Das Buch landet als Skill im Arbeitsordner; der Knopf zeigt den Fortschritt, und
  das Mac-Bündel bringt `myl` mit.
- ⚑ **Das geladene Modell macht Platz, statt zu gehen:** Seine Gewichte sind eingeblendet und
  werden vom System freigegeben, sobald Speicher fehlt. Nur der Kontextspeicher des wartenden
  Modells wird geleert, und nur, wenn es nicht reicht. Reicht der Speicher, verdichtet dasselbe
  Modell das Buch. Die Stufen laufen nacheinander, die Kerne folgen der Einstellung, und das
  Sehmodell läuft durch dieselbe Speicherprüfung wie überall.
- ⚠️ **Kein Rechenpfad.** Die Kiste rechnet mit Gleitkomma, ihr Ergebnis ist fremder Text
  und geht nie in den Konsens. Konsole, Fenster und Testclient binden sie nicht ein.
- ⚑ **`myl bild <datei> [--frage TEXT] [--genau]`:** ein Bild in Worte fassen, über
  denselben Weg wie Chat und Werkzeug (Sehregel, Grenzen für Bild und Kontext).
  `--genau` nimmt nur das genaue Sehmodell und endet sonst mit 4, damit ein Skript nicht
  still beim kleinen landet.

### v0.187.0 (2026-11-09): Wallets als Keystore (`myl-client` 0.123.0)

- ⚑ **Keystore nach EIP-2335** (`keystore`): scrypt mit N = 2¹⁸, AES-128-CTR,
  SHA-256; gelesen wird auch PBKDF2. Beide Prüfvektoren der Norm gehen auf, das
  Passwort wird wie dort vorbereitet (NFKD, ohne Steuerzeichen), und die Parameter
  einer fremden Datei sind gedeckelt, damit sie scrypt nicht Terabyte anfordern lässt.
- **`myl wallet neu` und `wiederherstellen` fragen nach einem Passwort** (leer:
  unverschlüsselt wie bisher, für Spielgeld), `myl wallet verschluesseln <name>`
  stellt ein vorhandenes um. Im Keystore liegt nur der Schlüssel des Kontos, nicht
  der Seed; die Sicherung bleibt die Wortliste. Unterschreiben fragt nach dem
  Passwort, ohne Echo.
- Beim Wechsel des Auszahlungskontos geht ein entsperrter Schlüssel über eine
  Röhre an den Knoten (`--kontowechsel -`), nie als Datei.
- ⚠️ Konsole und Fenster legen Wallets weiter ohne Passwort an, und der
  Schlüsselbund des Systems fehlt noch.

### v0.186.0 (2026-11-09): Module von Dritten laufen abgeschottet (`myl-module` 0.6.0, `myl-client` 0.122.0, `myl-console` 0.70.0, `myl-oberflaeche` 0.87.0)

- ⚑ **`"laufzeit": {"art": "wasm"}` läuft jetzt**, in Konsole und Fenster, mit
  denselben Nachrichten wie ein natives Modul, aber im Prozess der Konsole und
  ohne eigenes Dateisystem oder Netz. Die Laufzeit ist ein Interpreter ohne
  erzeugten Maschinencode (Fassung fest eingetragen, 20 Archive mehr im Vorrat).
- **Was das Modul bekommt:** `http` nur zu Rechnern, die seine Beschreibung nennt
  **und** der Mensch mit `/module genehmigen <modul>` freigegeben hat (keine
  Weiterleitung, keine privaten Adressen, feste Adresse je Anfrage, 60 je Minute,
  8 MiB Antwort), `datei_lesen` und `datei_schreiben` nur im eigenen Datenordner
  (16 MiB je Datei, 256 MiB insgesamt, keine Verknüpfungen), `geheimnis` nur die
  eigenen. Ablegen: `myl-module geheimnis <ablage> <modul> <name>` von stdin.
- **Grenzen:** 16 MiB Modul, 64 MiB Speicher, Treibstoff je Ereignis (rund eine
  Sekunde Rechnung), 1 000 Nachrichten und 16 MiB Ausgabe je Ereignis. Zeiger des
  Moduls sind fremde Werte; was außerhalb liegt, beendet nur das Modul.
- ⛔️ **Kein Agentenlauf und keine Werkzeugkiste für abgeschottete Module:** Beide
  starten gewöhnliche Programme, das hieße die Abschottung selbst zu öffnen.
- Vorlage und Prüfgast: `myl-module/beispiele/wasm-gast`. Tests gegen feindliche
  Gäste; sieben Gegenproben (je eine Grenze ausgebaut) fallen auf.
- ⚠️ Nicht abgewehrt: Seitenkanäle über die Zeit und Fehler in der Laufzeit
  selbst; die Signatur bleibt davor. Im Fenster gibt es noch keine Genehmigung,
  sie geht über die Konsole.

### v0.185.3 (2026-11-09): Das Update einmal echt durchgespielt

- ⚑ **`/update` belegt:** In einer Wegwerfkopie mit eigenem `origin` und eigenem
  Home, unter Linux aus der laufenden Konsole (holen, bauen, das laufende Programm
  ersetzen, in die neue Fassung starten) und unter macOS über
  `installieren-macos.sh --aktualisieren` samt `Myelith.app`. Ein eigener Commit
  oder eine offene Änderung an einer betroffenen Datei stoppt das Update, und
  Programme wie Arbeit bleiben unverändert.
- 📌 Dabei gefunden: `buendeln-macos.sh` baute als einzige Stelle ohne `--locked`
  und schrieb seine Fehlermeldung auf die Ausgabe, die der Installateur verwirft;
  er brach dann ohne ein Wort ab. Jetzt `--locked`, derselbe Netzschalter wie der
  Installateur, Fehler auf `stderr`. Beide Skripte beachten wie unter Linux
  `CARGO_TARGET_DIR`.

### v0.185.2 (2026-11-09): Die Warnfarbe bleibt grau

- `--warn` im hellen Thema war mit `#5f6368` knapp außerhalb der Graustufenregel
  (Spanne 9 über 8), und die Probe schlug an. Jetzt `#606366`, im Kontrast gleich.

### v0.185.1 (2026-11-08): Warnfarbe im hellen Thema

- `--warn` hat Leser bekommen und deshalb jetzt einen eigenen Wert im hellen
  Thema; das dunkle Grau trug dort zu wenig Kontrast.

### v0.185.0 (2026-11-08): Verlauf je Wallet

- ⚑ **7.4:** `myl wallet verlauf NAME` blättert durch die gehaltenen Blöcke des
  erreichbaren Knotens. Eine Auskunft wie `/konto`, kein Beweis.

### v0.184.0 (2026-11-08): „Mining starten“ rechnet mit

- ⚑ **6.27:** Mit gewähltem Wallet und vorhandenem Artefakt startet der Knoten
  seinen Shard aus der Kette (`--shard-programm`, `--artefakte`,
  `--modellzusage <ebenen>:2000`). Die erste Zusage ist bescheiden, die
  Selbstprobe des Knotens hebt sie. Fehlt etwas, startet er ohne Shard, und
  `myl mining` nennt den Grund.
- ⚑ **Das Netz ist wählbar:** `myl mining netz [probe|test-int4]` oder
  `MYL_NETZ`; je Netz sein Modell. ⚠️ Die Probekette rechnet ihr Probemodell
  nur mit Testbauten des Shards; ein ausgelieferter Shard lädt allein
  freigegebene Modelle.
- Echter Start auf dem Mac in einem abgeschirmten Heimatordner: Der Knoten
  nimmt die Angaben an, das Beenden hinterlässt keinen Prozess. Ein Pod aus
  mehreren Klienten ist damit nicht gefahren.

### v0.183.0 (2026-11-08): Adressen mit Prüfsumme, Konten aus dem Pfad

- ⚑ Die Wallet zeigt und liest Adressen als `tmyl1…` (`Wallet::adresse_text`,
  `myl wallet ueberweisen`). Eine Adresse eines anderen Netzes und ein
  Tippfehler werden abgewiesen. Für `--konto` bleibt die Rohform.
- ⚑ Wörter geben jetzt das erste Konto nach EIP-2334 statt des
  Hauptschlüssels. ⚠️ Bestehende Testwallets aus Wörtern bekommen eine andere
  Adresse.

### v0.182.0 (2026-10-28): Die Gebühr nach Einheiten (`myl-client` 0.118.0)

- `bezahlen::gebuehr` rechnet mit `gebuehreneinheiten`: Bytes plus eine
  Prüfeinheit je Unterschrift (CONSENSUS 0.59.0), wie die Kette.
- 208 Tests grün, Clippy sauber.

### v0.181.0 (2026-10-27): Gebühren in der Wallet (`myl-client` 0.117.0)

- ⚑ Jede Transaktion der Wallet sagt das Doppelte der Gebühr von jetzt zu
  (`bezahlen::GEBUEHR_SPIELRAUM`); abgebucht
  wird nur die Gebühr des Blocks. `bezahlen::gebuehr` rechnet sie aus dem
  Grundpreis der Kontoauskunft.
- Die Vorschau einer Überweisung nennt Gebühr, Zusage und, an ein neues
  Konto, den Aufschlag, und prüft, dass das Guthaben alles deckt.
- 208 Tests grün, Clippy sauber.

### v0.180.0 (2026-10-27): Überweisen mit Vorschau; Guthaben jedes Wallets (`myl-client` 0.116.0)

- ⚑ **`myl wallet ueberweisen NAME EMPFAENGER MYL [--ja]`**: zeigt zuerst,
  was unterschrieben wird (von, an, Betrag, Guthaben danach, Nonce, Netz),
  und unterschreibt erst nach Bestätigung. `bezahlen::ueberweisung_vorschauen`
  prüft Betrag, Empfänger und Deckung; `bezahlen::ueberweisen` unterschreibt
  **genau mit dem Stand der Vorschau** und bricht ab, wenn sich Nonce oder
  Netz inzwischen geändert haben. Probe im TESTCLIENT (`bezahlen.rs`):
  Überziehen bekommt keine Vorschau, eine Überweisung kommt an, dieselbe
  Vorschau ein zweites Mal unterschreibt nichts.
- `myl wallet` nennt Guthaben, Credits und Nonce **jedes** Wallets über die
  Leitung eines Knotens (eigener oder `--relais`), nicht nur das des
  laufenden Minings.
- `myl-oberflaeche`: `tauri.conf.json` auf 0.86.1 nachgezogen; bei 0.179.1
  vergessen, die Probe `die_buendelversion_ist_die_kistenversion` war rot.
- `myl-client` 0.116.0. 208 Tests grün, Clippy sauber.

### v0.179.1 (2026-10-26): Überlaufprüfung im Freigabebau (`myl-client` 0.115.1, `myl-console` 0.69.2, `myl-oberflaeche` 0.86.1)

- Überlaufprüfung im Freigabebau für die Protokollkisten (Festlegung des Projektinhabers): `[profile.release.package.<kiste>] overflow-checks = true` für jede Protokollkiste im Baum dieses Programms; die Rechenkerne (`integer-llm-*`) bleiben ohne. Bis hierher war sie nur im Testprofil an, und eine Rechnung im Konsenspfad wäre im ausgelieferten Programm an den Rändern still umgelaufen. Belegt am Knoten (NODE 0.64.0): `myl_ledger` wird mit `-C overflow-checks=on` übersetzt, `integer_llm_kernels` ohne. Alle 4 359 Tests liefen schon immer mit eingeschalteter Prüfung. In `myl-client`,
  `myl-console` und `myl-oberflaeche`, je mit den Protokollkisten ihres Baums.
- `myl-client` 0.115.1, `myl-console` 0.69.2, `myl-oberflaeche` 0.86.1.

### v0.179.0 (2026-10-26): Testwallets, Mining zahlt ins ausgewählte, Beitritt zu einem Probenetz (`myl-client` 0.115.0, `myl-oberflaeche` 0.86.0)

- ⚑ **Testwallets** (Auftrag des Projektinhabers): `myl_client::wallet`
  legt Wallets an (BLS-Schlüssel, Rechte `0600`, dasselbe Dateiformat wie
  der Knoten), listet sie und wählt eines aus; das ausgewählte ist das
  eingeloggte. ⚠️ Spielgeld: Die Schlüssel liegen unverschlüsselt neben den
  Einstellungen; vor echten MYL fehlen ein verschlüsselter Speicher, eine
  Sicherung und ein kaltes Auszahlungskonto.
- ⚑ **Mining zahlt ins ausgewählte Wallet.** Der Knoten startet mit
  `--konto` (dessen Adresse) und eigenem Konsensschlüssel und trägt es über
  die Kette als Auszahlungskonto ein; auf der Probekette prägt jede Epoche
  10 MYL dorthin. Nach einem Wechsel gibt der Client den Schlüssel des
  bisher eingetragenen Wallets mit (`--kontowechsel`), denn die Kette nimmt
  einen Wechsel nur mit dessen Unterschrift an; welches eingetragen ist,
  merkt sich der Client aus den Metriken des Knotens. „gemint“ misst jetzt
  das Wallet, nicht das Arbeitskonto des Knotens.
- ⚑ **Beitritt statt Alleingang:** `mining.einstieg` (neu) nimmt eine
  Multiadresse oder eine Datei, die eine nennt; dann tritt der Knoten bei,
  statt allein zu erzeugen. Vorher baute ein Knoten des Clients allein nie
  einen Block, weil der Erzeuger erst baut, wenn jemand zuhört; die
  Zusammenfassung sagt das jetzt. `MYL_PROBENETZ` geht der Einstellung vor.
- `myl wallet [neu NAME|waehle NAME]`, `myl mining einstieg [ADRESSE|DATEI|aus]`.
- **Fenster: der Reiter „Wallet“ ist offen.** Wallets mit Adresse,
  Guthaben (des laufenden Minings), Marken „ausgewählt“ und
  „Auszahlungskonto“, Anlegen, Auswählen, Einstieg; vier neue Befehle
  (`wallet_lage`, `wallet_neu`, `wallet_waehlen`, `einstieg_setzen`), keiner
  startet einen Unterprozess.
- ⚑ **Wiederherstellungswörter nach BIP-39** (Festlegung des
  Projektinhabers: der etablierte Standard, zwölf englische Wörter):
  `bip39.rs` mit der Liste aus BIP-39 (unverändert, MIT, Abdruck in der
  Probe), Prüfsumme, Seed nach PBKDF2-HMAC-SHA512 mit 2048 Runden;
  HMAC und PBKDF2 selbst geschrieben auf `sha2`, geprüft gegen RFC 4231 und
  die Prüfvektoren von BIP-39. Der Schlüssel ist der Hauptschlüssel nach
  EIP-2333 über den Seed; die Datei trägt den Seed.
- ⚑ **Ein neues Wallet entsteht in Schritten:** Entwurf, Suche im Netz
  (`/konto` beim laufenden Knoten oder beim Abfrageort des Einstiegs; mit
  Spuren wird nichts angelegt), die zwölf Wörter, „notiert?“ und zwei
  zufällig abgefragte Wörter; erst dann gespeichert. Dasselbe Wallet zweimal
  auf derselben Maschine wird abgewiesen. Wiederherstellen aus den Wörtern
  mit Suche im Netz. `Debug` eines Entwurfs zeigt die Wörter nie.
- Konsole: **`/wallet`** nach dem Schema von `/settings` und `/mining`
  (Wallets, „Neues Wallet“, „Aus zwölf Wörtern“, Einstieg), Ja-Frage über
  `janein`. Fenster: Entwurf mit Wörtern, Haken „notiert“, Abfrage zweier
  Wörter; der Seed bleibt im Rücken. `myl wallet neu` und
  `myl wallet wiederherstellen`. Der Einstieg wird an einer Stelle gesetzt
  (`mining::einstieg_setzen`).
- ⚑ **Bezahlen aus dem Wallet** (`bezahlen.rs`, neu): Das Wallet
  unterschreibt selbst (Nonce und Netzwert über die Relaisleitung des
  eigenen Knotens): Credits kaufen (Burn), einen Sitzungskontrakt für einen
  Agenten eröffnen (Budget, Einzellimit, Frist in Epochen, einziger
  Empfänger der Betreiber des Knotens), widerrufen. Der Agent hat einen
  eigenen Schlüssel (`wallets/agenten/`, `0600`) und stellt sich die
  Vollmacht aus, nur für diese Sitzung und bis zu ihrem Ende, mit dem
  Empfängerbeweis; abgelegt neben seinem Schlüssel. Den Schlüssel des
  Wallets bekommt der Agent nie. ⚠️ Die Summe der Budgets ist die Grenze.
  Der Mining-Knoten öffnet dafür seine Relaisleitung (`--podrelais`).
  Belegt im TESTCLIENT (`bezahlen`) gegen eine echte Kette.
- ⚑ **Der Agent über die Tür:** `myl frage --tuer HOST:PORT --agent NAME`
  fragt mit der Vollmacht des Agenten über die Tür eines Knotens statt über
  ein Modell auf dieser Maschine (über `Tuerklient`, denselben `Modellweg`
  wie die Agentenschleife) und nennt Beleg und Segment. Dazu `myl wallet
  credits`, `myl wallet sitzung` und `myl wallet widerrufen`, mit der
  Leitung des eigenen Mining-Knotens oder `--relais`/`--relaisausweis`.
- 📌 **Fund 644: Eine Reservierung unter macOS konnte still halb sein.**
  `F_PREALLOCATE` lief zuerst mit `F_ALLOCATECONTIG` allein, und ohne
  `F_ALLOCATEALL` darf der Aufruf weniger buchen als verlangt und trotzdem
  gelingen; `fst_bytesalloc` wurde nicht geprüft. Auf einer viel
  beschriebenen Platte belegte die Probe 25 von 64 MiB und war in drei
  Läufen zweimal rot. Jetzt beide Versuche „alles oder nichts“, und eine
  kleinere Buchung ist ein Fehler; danach zehn von zehn grün.
- Echter Lauf gegen das Docker-Probenetz (TESTCLIENT): Wallet `probe-a`
  bekam an der Epochengrenze 10 MYL; nach Auswahl von `probe-b` und
  Neustart unterschrieb der Knoten den Wechsel mit dem Schlüssel von
  `probe-a`, und ab der nächsten Epoche wuchs `probe-b`. Danach mit
  Wörtern: falsch beantwortete Abfrage legt nichts an, richtige legt an,
  dieselben Wörter unter anderem Namen werden abgewiesen, der Prüfvektor
  von BIP-39 stellt her, jedes Mal mit Suche im Probenetz.

### v0.178.6 (2026-10-26): Proben räumen ihren Zwischenordner auf (Fund 638; `myl-client` 0.114.2, `myl-senses` 0.13.6)

- 📌 **Fund 638: Proben ließen Dateien im Zwischenordner des Systems liegen.** Gefunden mit `TMPDIR` auf einen leeren Ordner: `myl-senses` hinterließ 40 Dateien je Lauf. Die Attrappen der Sprechproben schreiben ihren Text neben die Tondatei, und die Bibliothek räumt nur die Tondatei weg. Jetzt räumt `gesprochen` den Text beim Lesen weg; wo niemand ihn liest (Sätze für die Ablage, der ganze Satz neben gestreamten Stücken), schreibt die Attrappe keinen. Die Kennzeichnungsprobe legt ihre Tondatei in einen eigenen Ordner, der mit der Probe verschwindet; `tests/vorhaben.rs` in `myl-client` räumt seine Ordner mit einem Wächter weg. Gegenprobe: vorher 40 Reste, jetzt keiner; `myl-client` hinterlässt nichts.
- Nur Proben geändert, das Programm nicht.

### v0.178.5 (2026-10-24): `tokio` 1.54.1 wegen RUSTSEC-2026-0386 (`myl-oberflaeche` 0.85.3)

- Die Sperrdatei hebt `tokio` von 1.53.1 auf 1.54.1: Die sicheren Konstruktoren von `AsyncFd` verletzen die I/O-Sicherheit (RUSTSEC-2026-0386, `unsound`, neu seit dem 2026-10-23), und cargo-deny war damit rot. Der eigene Code nutzt `AsyncFd` nicht; geändert hat sich nur die Sperrdatei, der Vorrat trägt die neue Fassung. `tauri.conf.json` mitgezogen.

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
