# client (Nutzer-Client inkl. Wallet)

> **Version:** 0.94.0 (`myl-client` 0.63.0, `myl-oberflaeche` 0.50.2, `myl-console` 0.26.3, `myl-senses` 0.9.0)
> **Datum:** 2026-09-26
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
| **Der Loop: ein Ziel über viele Runden** | Im Fenster ∞ neben dem Senden (an und aus), daneben die Liste der Tasks: auswählen, neu anlegen, am Griff in die gewünschte Reihenfolge ziehen. Der vorderste läuft, alle dahinter sind „queued“; jeder Task hat sein eigenes Gespräch. In der Konsole `/loop` und `/tasks`, auf der Kommandozeile `myl loop` und `myl tasks`. Wird geschlossen, geht es beim nächsten Öffnen genau dort weiter |
| **Ein Gespräch mit einem lokalen Modell** | `myl frage <artefakt> <text>`, oder im Fenster |
| **Die Agentenschleife** | `myl agent`, mit Werkzeugen innerhalb einer Einhängegrenze |
| **Die Werkzeugkiste ist ein Ordner** | Eine Einstellung, ein Pfad: `agent.kistenordner`, ohne Angabe die mitgelieferte Kiste `Base` unter `CLIENT/werkzeugkisten/`. Der **Ordnername** sagt, welche eingebauten Werkzeuge dazukommen: `Base` die fünf Dateiwerkzeuge und die zwei Skillwerkzeuge `search_skill` und `learn_skill` (seit dem 2026-09-26), `Advanced` zusätzlich `run_command` und die drei Werkzeuge für den Mitschnitt (seit dem 2026-09-17, gemessen: das kleine Modell ruft sie nie). ⚑ **Eine Kiste kann das auch selbst sagen**, in ihrem `kiste.json`; ohne diese Datei und ohne einen der drei Namen bleibt es bei `Base`. Was als Manifest im Ordner liegt, sieht das Modell **ohne Neubau**. ⚑ **`Base` ist die Grundlage jeder Kiste**: Ihre Werkzeuge werden mitgeladen, gestapelt und nicht kopiert; bei gleichem Namen gewinnt die gewählte Kiste. ⛔️ Die fünf Dateiwerkzeuge bleiben kompiliert, weil nur sie die Einhängegrenze einhalten; ein Manifest läuft über die Shell und kann das nicht |
| **Eine Datei anhängen** | In der Konsole `/datei <pfad>`, im Fenster der Knopf neben dem Senden oder Ziehen und Ablegen. Die Datei wandert nach `.AGENT/anhaenge/`, also unter die Einhängung, und das Gespräch bekommt **eine Zeile mit ihrem Pfad statt ihres Inhalts**. Gibt es ein Werkzeug für ihre Art, nennt die Zeile es. `myl anhaenge` zeigt, was liegt, `--aufraeumen` räumt auf |
| **Sehen, Hören, Sprechen** | Die Kiste `myl-senses`: Bilder über llama.cpp, Ton über whisper.cpp, Sprechen über piper, jeweils mit einem **eigenen kleinen** Modell außerhalb des Repositoriums (`~/.myelith/sinne`). Das Hauptmodell bleibt ein Textmodell. ⚑ **Auch im Chat**, wo es keine Werkzeugschleife gibt: Eine angehängte Datei wird beim Anhängen angesehen. Fehlt ein Laufwerk, sagt der Sinn mit Pfad und Befehl, was fehlt, statt abzustürzen |
| **Die Sprechtaste** | Im Fenster: gedrückt halten, reden, loslassen. Über dem Feld schlägt ein **Pegel** aus, solange aufgenommen wird, und danach steht der Text **in der Eingabezeile**, nicht im Gespräch: Wer sich verhört hat, bessert aus, bevor das Modell liest. Der Lautsprecherknopf liest Antworten vor, **satzweise**: Der erste Satz klingt, während das Modell noch schreibt. ⚠️ Ein Voll-Duplex-Gespräch ist das nicht, es fehlen Sprechbeginnerkennung, Unterbrechen und Echokompensation |
| **Nachsehen, ob die Sinne gehen** | `myl sinne` zeigt, was dieser Rechner sehen, hören und sprechen kann, und `myl sinne <datei>` schickt eine Datei hindurch. ⚑ **Der kürzeste Weg von der Behauptung zum Beleg.** Eingerichtet wird mit einem Skript unter `SYSTEM/install/` |
| **Der Sprachmodus** | Ein Knopf ersetzt den Verlauf durch ein Zeichen, dessen Ringe **mit dem Ton mitschwingen**; darunter steht die laufende Antwort. Wer spricht und zuhört, liest nicht mit |
| **Eine eigene Stimme** | Auf der Einstellungsseite eine Aufnahme hochladen; sie wird zur Stimme. Whisper schreibt ihren Text gleich mit, weil CosyVoice damit besser trifft. ⛔️ Liegt eine Probe und spricht ein Programm, das nicht klonen kann, **steht das da** |
| **Mehrere Stellen auf einmal ändern** | `edit_file` nimmt eine Liste aus `alt` und `neu`. Erst ein Trockenlauf über eine Kopie, dann wird geschrieben: **Entweder alle Stellen oder keine** |
| **Vier Betriebsarten** | Chat und Agent laufen; Knoten und Wallet stehen mit ihrer Begründung da und warten auf das Netz |
| **Modellwahl** | Aus dem Katalog, mit Anzeigenamen statt Verzeichnisnamen. Der Netzeintrag heisst „Netzwerkmodell (API), kostet Inferenz-Credits" und ist gesperrt, solange Knotenadresse und Vollmacht fehlen |
| **Modelle holen und Artefakte bauen** | Aus der Einstellungsseite heraus, mit Ladebalken unter dem angeklickten Modell und einer schliessbaren Meldung, wenn es fertig ist |
| **Einstellungen** | Fünf Bereiche, vierzehn Felder (dreizehn davon im Fenster: das Konsolen-Design wirkt dort nicht) in der Reihenfolge, in der jemand sucht (Sprache, Aktualisierung, Modelle, was dieser Rechner hergibt, dann Modell und Agent), und dazu ein Schieberegler je gefundenem Rechenwerk, jedes mit Beschriftung und einem Satz darunter, was es bewirkt. Art **und** Beschriftung kommen aus der Kiste. Die drei Verzeichnisfelder lassen sich über den Fensterdialog des Systems wählen, getippt werden dürfen sie weiter |
| **Sprache** | Deutsch oder Englisch, umschaltbar in den Einstellungen und sofort wirksam. Feldnamen, Pfade und Modellnamen bleiben, wie sie sind |
| **Aus einem frischen Klon einrichten** | Drei Skripte unter `SYSTEM/install/`, je eines für macOS, NixOS und Windows, mit einer Anleitung je System daneben. Sie prüfen erst, bauen dann, und laden nichts nach |
| **Nach Aktualisierungen sehen** | Auf der Einstellungsseite: Gefragt wird, ob `origin` Änderungen hat, die dieser Klon nicht hat. Eingespielt wird mit `git merge --ff-only` und dem Installationsskript der Plattform |
| **Gespraeche verwalten** | Rechtsklick auf eine Zeile: umbenennen an Ort und Stelle, als Markdown ausgeben, loeschen. Wohin ausgegeben wird, steht in `ausgabe.ordner`; ohne Angabe fuehrt das Fenster dorthin |

⚑ **Die Oberfläche ruft dieselben Funktionen wie die Kommandozeile**,
über siebenundvierzig Befehle. ⚠️ **Mit genau einer Ausnahme, und sie ist gewollt:** `terminal_ausfuehren` startet eine Shell, denn ein Terminal, das keine startet, ist keines. Jeder andere Befehl startet **keinen einzigen Unterprozess**. `jeder_befehl_ist_angemeldet` hält die vier
Richtungen zusammen: kein Befehl ohne Anmeldung, keine Anmeldung ohne
Befehl, kein Aufruf ins Leere und kein Befehl, den niemand ruft. „Ohne eigene Logik" hiesse sonst, aus einer Textausgabe
für Menschen eine Schnittstelle zu machen, und genau das ist die Sorte
Logik, die hier nicht hingehört.

## Struktur

| Verzeichnis | Zweck |
|---|---|
| `myl-client/` | Die Kiste. Einstellungen, örtlicher Betrieb, Agentenschleife, Werkzeuge mit Einhängegrenze, Türklient. Kommandozeile `myl`. |
| `myl-console/` | Der Agent in der Konsole: `myelith`. Startbild, Modellwahl, Eingaberahmen. **Ohne eigene Logik**, alles kommt aus `myl-client`. |
| `myl-oberflaeche/` | Die grafische Oberfläche auf Tauri v2. Rücken in Rust, Frontend als reines HTML, CSS und ES-Module: **kein Bündler, keine Node-Werkzeugkette**. |
| `myl-oberflaeche/ui/` | `index.html`, `stil.css`, `app.js`, `netz.js`. Neunundvierzig Prüfungen halten HTML, CSS, Skript und Rücken gegeneinander. |
| `myl-oberflaeche/icons/` | Symbole. `icon.ico` und `icon.icns` sind aus `icon.png` abgeleitet und liegen fertig da; von Hand nachbessern hilft nicht, die nächste Ableitung überschreibt es. |

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

### v0.94.0 – 2026-09-26 (ein fest vorgegebener Systemprompt, geprüft vor jedem Lauf)

**Auftrag des Projektinhabers:** ein Systemprompt, der das Modell auf die
Arbeitsweise vorbereitet, unbedingt vorgegeben und über einen Hash
überprüft, damit Compliance- und Ethik-Vorgaben später dauerhaft darin
stehen können. Die Texte liegen unter `COMPLIANCE/systemprompt/` (dort
beschrieben); hier, wie der Client sie benutzt.

**Neu in `myl-client` (0.63.0), `systemprompt.rs`:**
- Die Texte und `pruefsummen.txt` sind **eingebaut** (`include_str!`).
  Es gibt keine Einstellung, die einen anderen Text an diese Stelle setzt.
- ⛔️ **Vor jedem Lauf geprüft, im Zweifel geschlossen:**
  `lauf::fahren_im_gespraech` hält den Text gegen seine Summe, und passt
  sie nicht, endet der Lauf, bevor das Modell gefragt wird. Dasselbe in
  `myl` (`einen_auftrag`, `frage`); die Konsole prüft zusätzlich beim
  Start und startet sonst nicht.
- **Wo er steht:** als Hausregel **hinter** der zeichengenauen
  Werkzeugvorlage (`angebot_mit_regel`), nicht darin. Bisher lief jeder
  Agentenlauf mit `hausregel: None`.
- **Welche Sprache:** die der Werkzeugansage (`Amtlich` englisch,
  `Deutsch` deutsch), damit Prompt und Werkzeugliste dieselben Namen
  tragen; antworten soll das Modell in der Sprache des Nutzers.
- **Der Chat ohne Werkzeuge** bekommt die Grundsätze (den Teil vor der
  Arbeitsweise), in der Sprache der Oberfläche: im Fenster und bei
  `myl frage`.
- **Nachvollziehbar:** Jeder Lauf trägt den vollen SHA-256 der Fassung
  ins Aktionsprotokoll (`art: systemprompt`, `entscheidung: geprueft` oder
  `abgewiesen`).
- `gespraech::ansage` zählt den Systemprompt mit, damit die
  Kontextanzeige dasselbe misst, was das Modell bekommt.

⚠️ **Die Ausgabe von `myl frage` ändert sich**, denn vor der Frage stehen
jetzt die Grundsätze. Die Rauchproben mit der Paris-Frage (kalter Klon,
GolemOS) bleiben untereinander vergleichbar, nicht aber mit Läufen von
vorher.

**Belegt:**
- `myl-client` grün (rot nur Fund 482), neu:
  - `die_pruefsummen_stimmen`,
  - `eine_veraenderte_fassung_wird_abgewiesen`,
  - `beide_fassungen_tragen_die_regeln` (samt Grundsätzen für den
    Chat),
  - `die_ansage_traegt_den_vorgegebenen_systemprompt` (hinter
    `</tools>`).
- Gegenprobe am echten Text: ein Zeichen in `de.md` geändert, und die
  Prüfung meldet verlangte und tatsächliche Summe.
- Konsole 158, Fenster 74, Clippy mit `-D warnings` in allen drei ohne
  Befund.

### v0.93.0 – 2026-09-26 (das Tagebuch lässt sich nicht mehr nachahmen, und die Suche findet Dateinamen; Funde 491 und 492)

**Aus den ersten Läufen des 30B-A3B im Loop-Szenario.**

⛔️ **Fund 491: Das Modell ahmte das Tagebuch nach.** Die Zeile des
Systems hieß „Ausgeführt: …“. Das 30B schrieb in zwei Runden selbst
„Ausgeführt: write_file(…)“ und „Ausgeführt: edit_file(…)“ in seine
Antwort, **ohne ein Werkzeug zu rufen**. Nichts war geschrieben, im
Tagebuch stand es wie eine Tatsache, und die nächste Runde sah es im
Rückblick. Jetzt:
- Die Zeile des Systems trägt die Marke `[System]`: „Tatsächlich
  ausgeführt: …“, oder ausdrücklich „In dieser Runde lief kein Werkzeug.“
- `behauptungen_markieren`: Zeilen im Bericht des Modells, die wie eine
  Systemzeile aussehen (auch mit gefälschter Marke), stehen im Tagebuch
  als „(behauptet, nicht ausgeführt)“.
- Der Rundenauftrag sagt: Ein Werkzeug wirkt nur, wenn es aufgerufen wird.

⛔️ **Fund 492: Zwei Werkzeuge ließen eine vorhandene Datei fehlend
aussehen.** Das 30B schloss in Runde 1, `daten/messwerte.csv` gebe es
nicht:
- **`list_directory` mit Tiefe 1** zeigte `daten` nur als „Verzeichnis“.
  Jetzt steht bei einem nicht aufgeklappten Ordner die Zahl der Einträge
  und „erst mit tiefe N zu sehen“.
- **`search_files` suchte nur im Inhalt.** Die Suche nach
  `messwerte.csv` fand die Zeile in `auswertung.py`, die den Namen
  nennt, aber nicht die Datei. Jetzt stehen Dateien und Ordner, deren
  **Name** das Muster enthält, vor den Zeilentreffern.

📌 **Und eine Lehre an mir, beim Szenario:** Die erste Abnahme für
Aufgabe 1 prüfte nur „läuft durch, Datei nicht leer“. Das 30B bestand
sie, indem es den `KeyError` mit `zeile.get("sensor", "n/a")` verdeckte;
die Tabelle hatte eine Zeile „n/a“ mit Nullen. **Eine Abnahme muss die
Anforderung prüfen, nicht nur das Durchlaufen.** Die neue verlangt eine
Zeile je Sensor.

**Belegt:**
- `myl-client` 356 Proben grün (rot nur Fund 482), neu:
  - `behauptete_aufrufe_werden_markiert`,
  - `eine_behauptete_ausfuehrung_steht_nicht_als_tatsache_da` (ganze
    Runde),
  - `eine_runde_aendert_vorhandene_dateien`,
  - `die_suche_findet_auch_dateinamen`,
  - `das_verzeichnis_listet_und_nennt_die_art` erweitert.
- Gegenprobe: Ohne die Markierung wird die Rundenprobe rot.
- Clippy mit `-D warnings` ohne Befund.

### v0.92.0 – 2026-09-26 (der Abnahmebefehl entscheidet über „fertig“, und ein Pendelwächter sieht den Kreis über zwei Stände)

**Aus dem Loop-Szenario mit dem 8B**, auf Auftrag des Projektinhabers vor
dem Lauf mit dem 30B.

**Neu in `myl-client` (0.61.0):**
- ⚑ **Der Abnahmebefehl je Task** (`Vorhaben::abnahme`), vom Menschen
  beim Anlegen genannt. Nach jeder abgeschlossenen Runde läuft er mit
  `sh -c` im Arbeitsordner, höchstens 300 s.
  - **Endet er mit 0, ist der Task fertig**, auch an der Schrittgrenze
    und auch, wenn das Modell sich nicht abgemeldet hat.
  - **Endet er anders, ist der Task nie fertig**, gleich was Modell und
    Prüfdurchgang sagen. Seine Ausgabe steht als Notiz `abnahme` da; die
    nächste Runde sieht den echten Fehler statt einer Vermutung.
  - Der Rundenauftrag nennt den Befehl, und das Tagebuch hält das
    Ergebnis fest.
  - 📌 **Warum:** Im Szenario bestätigte der Prüfdurchgang des 8B zweimal
    falsche Arbeit als erreicht, auch mit Belegen. **Ein Modell, das sich
    selbst abnimmt, nimmt sich nicht ab.**
  - Ein Befehl des Menschen, nicht des Modells: Das Modell kann ihn nicht
    ändern, deshalb läuft er unabhängig von der Werkzeugkiste.
- ⚑ **Der Pendelwächter** um `write_file` und `edit_file` in jeder
  Loop-Runde. Er merkt sich je Datei die Stände dieser Runde (vor der
  ersten Änderung eingeschlossen) und hängt einen Hinweis an, wenn eine
  Änderung nichts änderte oder eine Datei auf einen früheren Stand
  zurückfiel. 📌 In Lauf 4 pendelte das 8B fünfzehnmal zwischen
  `zeile["sensor"]` und `zeile["sens"]`; die Wiederholungsbremse sieht
  das nicht, denn jede Änderung ist ein anderer Aufruf mit Wirkung. Er
  verhindert nichts, denn ein Zurück kann richtig sein.
- `myl tasks add <ziel> --abnahme <befehl>`, `myl tasks abnahme <ID> <befehl>`
  (leer löscht), `Ablage::abnahme_setzen`.

**Neu in `myl-console` (0.26.2):** `/tasks abnahme <ID> <befehl>`.

⚠️ **Noch nicht im Fenster:** Die Taskliste kann einen Abnahmebefehl noch
nicht setzen.

**Belegt:**
- `myl-client` 352 Proben grün (rot nur Fund 482), neu:
  - `die_abnahme_entscheidet_ueber_fertig`: bestanden an der
    Schrittgrenze ist fertig; nicht bestanden trotz Behauptung und
    „erreicht“ ist nicht fertig, mit Ausgabe in den Notizen,
  - `der_abnahmebefehl_laeuft_im_arbeitsordner`,
  - `der_pendelwaechter_sieht_den_kreis`,
  - `die_abnahme_entscheidet_ueber_ganze_runden`: Drehbuch über zwei
    Runden; die zweite sieht die Ausgabe der ersten.
- Gegenproben:
  - Mit dem alten `abschliessen` in der Runde wird die Rundenprobe rot.
  - Ohne den Vergleich mit früheren Ständen wird die Pendelprobe rot.
- Konsole 151 und 7, Clippy mit `-D warnings` ohne Befund.

### v0.91.0 – 2026-09-26 (Web-Recherche auch im Agenten, mit mitwachsender Verratsprobe; der Prüfdurchgang sieht Belege, an der Schrittgrenze ist nichts fertig, eine Wiederholungsbremse, `write_file` legt fehlende Ordner an; Funde 487 bis 490)

**Web-Recherche im Agenten** (Festlegung des Projektinhabers,
2026-09-26: „Die Web-Werkzeuge sollen selbstverständlich auch dem
Agenten zur Verfügung stehen“):
- Mit `agent.web_recherche` bekommt jetzt auch der Agent `web_search`
  und `web_read`. Das gilt in Konsole, Fenster, `myl agent` und im Loop;
  vorher gab es sie nur im Chat für Anhänge.
- ⛔️ **Die Verratsprobe (Schranke 2) wächst mit dem Lauf.** Im Chat
  prüft sie gegen die Anhänge. Im Agenten wäre ein Schnappschuss des
  Arbeitsordners teuer und träfe das Falsche, denn **verraten kann das
  Modell nur, was es gelesen hat**. Ein Mitleser legt deshalb jedes
  Ergebnis eines eigenen Werkzeugs (Datei, Suche, Befehl, Skill) ins Tor
  (`Tor::gesehen`, gedeckelt auf 16 MiB). Eine Suchfrage mit einem
  wörtlichen Stück daraus geht nicht hinaus. Die Web-Werkzeuge selbst
  werden nicht mitgelesen; ihr Inhalt ist fremd.
- **Die Saat des Zielkreises** (Schranke 1) ist, was der Mensch
  geschrieben hat: der Auftrag, im Loop das Ziel des Tasks. Dafür gibt es
  `Agenteneinstellung::netzsaat` (je Lauf, nie gespeichert), und der
  Rüster des Loops bekommt das Ziel als zweites Argument.
- Im `manual mode` fragt jede Webanfrage nach, wie im Chat.
- ⚠️ **In `Advanced` bleibt `run_command` ein Weg nach draußen ohne
  Tor.** Das war der Grund, die Werkzeuge bisher nur im Chat anzubieten,
  und er steht jetzt so an der Einstellung.
- Eine vergiftete Sperre am Tor zählt als Verrat: Im Zweifel geht nichts
  hinaus.

⛔️ **Fund 487: Der Prüfdurchgang glaubte der Behauptung.** Im
Loop-Szenario mit dem 8B endete die erste Runde an der Schrittgrenze,
ohne eine Datei geändert zu haben. Die Prüfung sah nur Ziel und Bericht
(die Liste der Aufrufe) und urteilte „erreicht: ja, das Skript läuft ohne
Fehler“. Das Skript brach weiter mit `KeyError` ab, der Task stand auf
„fertig“. Jetzt:
- Die Prüfung sieht die letzten acht **Werkzeugergebnisse** als Belege
  und die Regel, dass eine Behauptung ohne Beleg nicht zählt.
- ⛔️ **Eine Runde an der Schrittgrenze ist nie „fertig“**, gleich was
  Prüfung oder Modell sagen; ihr Fortschritt zählt trotzdem.

⛔️ **Fund 488: Aufrufe im Kreis.** Dieselbe Runde lief viermal durch
dieselben vier Aufrufe bis zur Schrittgrenze. Die Erkennung
„steckengeblieben“ der Schleife sieht nur aufeinanderfolgende
Wiederholungen. Jetzt gibt es in jeder Loop-Runde eine
**Wiederholungsbremse**: Ein lesender Aufruf, der genauso schon lief,
ohne dass seither ein nicht lesender dazwischen war, läuft nicht noch
einmal, und das Modell bekommt einen Hinweis.
- 📌 **Nachtrag aus Lauf 3:** Ein `note_set` zwischen zwei gleichen
  Suchen hob die Bremse auf, weil er als Änderung zählte. Notizen, Wecken
  und die reinen Rechenwerkzeuge der verankerten Kiste zählen jetzt weder
  als Änderung, noch werden sie gebremst. Gegenprobe: Ohne diese Regel
  wird die Probe rot.
- Der mitgelieferte Skill `fehlersuche` beginnt jetzt mit „Ausführen,
  nicht vermuten“. Das 8B hatte in zwei Läufen das fehlerhafte Skript
  nie gestartet und die echte Meldung (`KeyError`) deshalb nie gesehen.

⛔️ **Fund 490: Ein fehlender Ordner war eine Sackgasse.** In Lauf 2 des
Szenarios wollte das 8B `ergebnis/statistik.md` schreiben, neunmal, und
jedes Mal fehlte `ergebnis/`. Es schrieb „Ich erstelle das Verzeichnis“,
aber ohne Shell gibt es kein Werkzeug dafür. In `Base` war die Aufgabe
damit unlösbar. Jetzt:
- `write_file` legt fehlende Ordner an.
- `Einhaengung::aufloesen` löst dazu den tiefsten **vorhandenen**
  Vorfahren auf und lässt darunter nur reine Namen zu.
- `..` fällt heraus, und ein Verweis nach draußen, unter dem ein neuer
  Ordner entstehen soll, ebenso. Draußen wird nichts angelegt.

⚠️ **Fund 489, beobachtet, nicht behoben:**
`das_aktionsprotokoll_haelt_fest_ohne_klartext` war in einem
Gesamtlauf unter Volllast (8B-Loop nebenher) einmal rot und allein
zweimal grün. Verdacht: Der prozessweite Protokollort
(`protokoll::ordner_setzen`) und parallel schreibende Nachbarproben.

**Belegt:**
- `myl-client` 348 Proben grün (rot nur Fund 482). Neu sind:
  - `die_wiederholungsbremse_bremst_nur_ohne_aenderung`,
  - `an_der_schrittgrenze_ist_nichts_fertig`,
  - `der_pruefauftrag_zeigt_die_belege`,
  - `write_file_legt_ordner_an_und_bleibt_drinnen` (tiefe neue Ordner;
    `..` mitten im neuen Pfad; ein Verweis nach draußen),
  - `tests/webagent.rs`: Werkzeuge nur mit Häkchen; nach `read_file`
    einer Datei mit Geheimnis wird eine Suchfrage mit einem wörtlichen
    Stück daraus abgewiesen. Die Suchadresse zeigt auf `.invalid`, damit
    auch eine kaputte Schranke nichts hinausschickt.
- Gegenproben:
  - ohne die Sperre an der Schrittgrenze wird die Probe rot,
  - ohne Mitleser geht die Frage hinaus (gegen `.invalid`: „Could not
    resolve host“), und die Probe wird rot.
- Clippy ohne Warnung. Der Prüfdurchgang mit Belegen sagt im neuen Lauf
  „Fortschritt ja, erreicht nein“, wo er vorher „erreicht“ erfand.

### v0.90.0 – 2026-09-26 (Strg-C und Esc: Notaus mit Frage im Lauf, sofortiges Ende an der leeren Zeile; `myl tasks add`)

**Festlegung des Projektinhabers (2026-09-26)**, sie ersetzt die vom
2026-09-15 für diese beiden Tasten:
- **Während eines Laufs** (Auftrag oder Loop-Runde, auch im Countdown
  zwischen zwei Runden):
  - Strg-C fragt „Notaus: anhalten? [J/n]“, die Eingabetaste heißt ja,
    ein zweites Strg-C ebenso.
  - Esc fragt „Notaus: anhalten? [j/N]“, die Eingabetaste heißt nein;
    eine gestreifte Esc hält also nichts an.
- **An der leeren Eingabezeile**, wenn nichts mehr läuft, beenden Strg-C
  und Esc das Programm **sofort**, ohne Frage.
- Strg-X fragt weiter „Wirklich beenden?“.

**Neu in `myl-console` (0.26.0):**
- Die Notausfrage liegt im Anzeigefaden, dem einzigen Tastenleser
  (Fund 483), mit eigenem Zustand.
- ⚑ **Sie geht einer offenen Vorlage im `manual mode` vor.** Ein Ja
  lehnt die Vorlage mit ab, sonst wartete der Lauf auf eine Antwort, die
  nicht mehr kommt.
- Im Loop pausiert der bestätigte Notaus: Die Runde hält sofort an, der
  Task bleibt, wie er war, und `/loop` macht genau dort weiter.

**Neu in `myl-client` (0.59.0):** `myl tasks add <ziel>` legt einen Task
an, ohne den Loop zu starten. So stehen mehrere Tasks in der Schlange,
bevor `myl loop` beginnt; das Loop-Szenario unter
`BENCHMARKS/Agent/loop/` braucht genau das.

**Belegt:**
- Konsole 151 und 7 Proben, darunter `die_notausfrage_hat_je_taste_ihre_vorgabe`
  und `esc_und_strg_c_beenden_strg_x_fragt`.
- Ein Lauf über ein Pseudo-Terminal mit dem 0,6B-Modell (37 s):
  1. Strg-C mitten in einer Antwort zeigt „[J/n]“, die Eingabetaste
     hält an.
  2. `/loop` und Esc in der Runde zeigt „[j/N]“, `j` pausiert.
  3. Esc an der leeren Zeile beendet den Prozess ohne Frage.

### v0.89.0 – 2026-09-26 (Skills: mitgelieferte Grundskills und Vorlage unter `myl-skills`, `search_skill` und `learn_skill` in jeder Kiste; Fund 486 offen)

**Worum es geht.** Wunsch des Projektinhabers (2026-09-26): eine
Grundkonfiguration für Skills mit Ordnerstruktur und Beispielen, die als
Vorlage für eigene dienen. Dazu ein Werkzeug, mit dem das Modell auf
„lerne skill …“ oder „learn skill …“ einen Skill lernt, und eines, mit
dem es selbst nach einem passenden Skill sucht, wenn es nicht
weiterweiß. ⚑ Die mitgelieferten Skills liegen neben `myl-senses` als
`CLIENT/myl-skills/` (Wunsch des Projektinhabers). Das ist ein
Datenordner und keine Kiste; die Logik bleibt in `myl-client`, neben den
Dateiwerkzeugen.

**Neu: `CLIENT/myl-skills/`**
- `README.md` erklärt die drei Orte, den Aufbau eines Skills und wie ein
  eigener entsteht.
- **Die Vorlage:** `skill-erstellen/vorlagen/SKILL.md`, mit Kopf
  (`beschreibung`, `stichworte`) und den Abschnitten Wann, Vorgehen,
  Fallen, Beispiel.
- **Fünf Grundskills:** `skill-erstellen`, `fehlersuche`,
  `aufgabe-zerlegen` (auch für den Loop, mit `note_set`),
  `bericht-schreiben` (mit Vorlage), `datei-sicher-aendern`.

**Neu in `myl-client` (0.58.0):**
- ⚑ **Drei Orte mit Vorrang:** Projekt (`.AGENT/skills/`) vor eigenen
  (neben den Einstellungen) vor mitgelieferten. Ein Ordner mit `_` vorn
  wird übergangen (Entwurf).
- **Ein Skill ist ein Ordner mit `SKILL.md`** und einem kurzen Kopf.
  Englische Schlüssel (`description`, `keywords`) gelten auch, und
  Wissensmappen aus `md_zu_mappe.py` bleiben gültig. Der Kopf wird ohne
  Fremdkiste gelesen.
- **`search_skill`** (deutsch `skill_suchen`) sucht nach den Worten
  einer Aufgabe:
  - Die Punkte sind ganzzahlig und gewichtet: Name 6, Stichwort 4,
    Beschreibung 3, Anleitung 1.
  - Umlaute und Großschreibung sind gefaltet, Füllwörter gehen nicht mit
    in die Suche.
  - Ein leerer Text nennt alle.
  - Treffer nur in der Anleitung werden als schwach angesagt.
- **`learn_skill`** (deutsch `skill_lernen`) liefert die Anleitung und
  nennt am Ende die weiteren Dateien. Eine davon holt derselbe Aufruf
  mit `name/datei`. ⚑ Ein Parameter und kein optionaler zweiter, nach
  der Regel dieser Werkzeuge.
- ⛔️ **Kein Weg nach draußen:** Der Name wird gegen die gefundenen
  Skills gehalten, und eine Datei muss nach dem Auflösen im Ordner des
  Skills liegen. Das gilt auch für einen Verweis (Symlink) nach draußen.
- ⚑ **Beide Werkzeuge liegen jetzt in `Base`** und ersetzen
  `list_skills` und `read_skill`, die nur in `Advanced` lagen. Vier
  Werkzeuge für dieselbe Sache wären drei zu viel.
- `myl skills` zeigt die drei Orte und die Herkunft je Skill;
  `myl skills neu <name>` kopiert die Vorlage unter die eigenen.

**Gemessen mit echten Modellen** (abgeschirmte Einstellungen, der
Probelauf liegt nicht im Repositorium):

| Lauf | 0,6B | 4B |
|---|---|---|
| „lerne skill fehlersuche …“ | kein Aufruf erkannt (siehe Fund 486) | `search_skill`, `learn_skill`, richtige Antwort |
| „learn skill bericht-schreiben …“ | nicht gefahren | direkt `learn_skill`, richtige Antwort |
| Problem ohne Hinweis auf Skills | kein Werkzeug | sucht **von selbst**, lernt `fehlersuche`, folgt ihr |

📌 **Zwei Fehler der ersten Fassung, beide erst am echten Modell
sichtbar:**
1. Die Anfrage „zahlen.py stürzt beim Start ab“ traf `skill-erstellen`
   über das Füllwort „beim“ in dessen Anleitung. Das 4B lernte dreimal
   den falschen Skill bis zur Schrittgrenze. Jetzt:
   - Füllwörter gehen nicht mit in die Suche,
   - die Stichworte der Fehlersuche tragen Verbformen („stürzt“),
   - schwache Treffer sagen sich an.
2. Das 4B gab die ganze Trefferzeile `fehlersuche: Einen Fehler …` als
   Namen an `learn_skill`. Jetzt:
   - der Name steht in der Liste für sich,
   - `learn_skill` nimmt nur, was vor dem ersten Doppelpunkt,
     Leerzeichen oder der ersten Klammer steht.

⚠️ **Fund 486, offen (AGENT_LAYER):** Das 0,6B schreibt den Aufruf als
nacktes JSON (`{"name": "learn_skill", "arguments": {…}}`) ohne die
`<tool_call>`-Klammern, und die Agentenschleife erkennt ihn nicht. Es
**will** das Werkzeug rufen. Behoben ist das nicht; es betrifft jeden
Werkzeugaufruf des kleinen Modells und gehört in die Aufrufzerlegung von
`local-agent`.

**Belegt:**
- `myl-client` grün bis auf Fund 482, darunter sieben neue Proben:
  - drei Orte mit Vorrang,
  - Kopf und verborgene Entwürfe,
  - Suche, Gewichtung und Füllwörter,
  - Dateien nennen und im Skill bleiben, auch gegen einen Symlink,
  - jeder mitgelieferte Skill hat Kopf, Stichworte und Beschreibung,
    und die Vorlage liegt da,
  - die Werkzeuge: Suche, Lernen samt abgeschriebener Trefferzeile,
    Pfade nach draußen, fehlende Argumente, gemeinsames
    Nachschlagebudget.
- Gegenprobe: Ohne die Prüfung „liegt im Ordner des Skills“ liest
  `learn_skill` über einen Symlink eine Datei außerhalb, und die Probe
  wird rot.
- Clippy ohne Warnung.

### v0.88.0 – 2026-09-26 (der Loop, Phase 3: ∞ im Fenster mit Taskliste, Warteschlange mit Ziehen, `/tasks` und `myl tasks`; Funde 484 und 485)

**Worum es geht.** Der Loop kommt ins Fenster, und aus losen Vorhaben
wird eine **Warteschlange**. Festlegungen des Projektinhabers
(2026-09-26):
- ∞ neben dem Senden ist ein **Schalter**: an fährt der Loop, aus
  pausiert er.
- Daneben öffnet ein Pfeil die **Liste der Tasks**. Dort wird
  ausgewählt, neu angelegt und die Reihenfolge per Ziehen geändert.
- Läuft schon ein Task, steht er mit „(läuft)“ da, alle danach
  angelegten erscheinen als „(queued)“.
- In der Konsole heißt der Befehl nur `/tasks` (kein `/vorhaben`), auf
  der Kommandozeile `myl tasks`, die Unterbefehle `resume` und `pause`.

**Neu in `myl-client` (0.57.0):**
- ⚑ **Die Warteschlange** (`Ablage::reihe`, `reihe_setzen`, `vorn`,
  `stellungen`). **Es läuft immer nur der vorderste aktive Task**, auch
  wenn er schläft und dahinter einer bereit wäre; wer einen anderen
  zuerst will, zieht ihn nach vorn. Die Reihenfolge steht in einer
  Datei (`reihe.json`) und nicht in jedem Vorhaben: Umstellen ändert
  eine Stelle, und ein halb gespeichertes Umstellen gibt es nicht. Was
  dort fehlt, steht hinten; eine Kennung ohne Vorhaben fällt heraus.
- **Eine Kette reiht sich direkt hinter ihrem Vorgänger ein**, nicht
  hinten: Sie ist die Fortsetzung dessen, was gerade lief.
- `Stellung` mit einem Wort für alle drei Oberflächen („läuft“,
  „queued“, „pausiert“, „als Nächstes“, „fertig“, „angehalten: …“),
  `zustandswort` für den Satz nach einer Runde, `Ablage::liste` für
  Konsole und `myl`. Vorher stand derselbe Satz dreimal da.
- `Ablage::weitermachen`, `stoppen`, `entfernen`. ⛔️ `entfernen` löscht
  nur einen Ordner, der ein Vorhaben ist, und nimmt keine Kennung mit
  `/` oder `..` an.
- ⚑ **Die Leihe** (`Modellleihe`): `fahren` bekommt das Modell nicht
  mehr für den ganzen Loop, sondern leiht es je Runde. Ein Loop wartet
  Stunden zwischen zwei Runden; mit der Leihe ist das Modell in dieser
  Zeit frei, und im Fenster lässt sich weiter chatten. Der Beginn einer
  Runde wird **erst gemeldet, wenn das Modell geliehen ist**.
- `Ereignis::OhneModell`: Lässt sich das Modell nicht leihen, endet der
  Loop, und der Task bleibt, wie er war.
- `myl tasks [resume|pause ID]` statt `myl vorhaben`.

**Neu in `myl-console` (0.25.0):** `/tasks [resume|pause <ID>]`, die
Liste in der Reihenfolge der Schlange; die Texte kommen aus der Kiste.

**Neu in `myl-oberflaeche` (0.50.0):**
- **∞ und der Pfeil** stehen im Agentenmodus neben dem Senden.
  Eingeschaltet steht ∞ hell auf einer Fläche. Der Titel sagt, was
  gerade ist: aus, an, rechnet eine Runde, nächste Runde in n min.
- **Die Taskliste:** je Zeile ein Griff, das Ziel, die Stellung in
  Klammern, dazu Anhalten oder Fortsetzen und Entfernen.
  - ⛔️ Entfernen braucht zwei Klicks, denn ein Task trägt sein Tagebuch
    mit.
  - Umsortieren am Griff mit der Maus oder mit Alt+↑ und Alt+↓.
  - ⚠️ Gezogen wird mit Zeigerereignissen und nicht mit HTML5-Drag-
    and-drop: Tauris Datei-Ablage verschluckt das Ziehen innerhalb der
    Seite unter Windows.
  - Unten ein Feld für einen neuen Task.
- **Jeder Task hat sein eigenes Gespräch** in der Agentenliste (∞ vor
  dem Titel), und jede Runde schreibt dorthin ihren Beitrag: Denken,
  Werkzeuge, Bericht, darunter die Prüfung. Einen Task auswählen heißt,
  dieses Gespräch zu öffnen.
- ⚑ **Der Strom einer Runde läuft auf einem eigenen Kanal**
  (`loop-lebt`). Ein Chat und eine Runde können sich zeitlich berühren,
  denn die Runde wartet auf das Modell, das der Chat gerade freigibt;
  auf einem gemeinsamen Kanal liefe der Anfang der Runde in den Beitrag
  des Chats.
- Während einer Runde sperrt das Fenster das Senden und sagt, warum;
  zwischen den Runden ist das Modell frei. `modell_entladen` und das
  Löschen eines Artefakts nehmen die Sperre mit `try_lock`, damit der
  Hauptfaden während einer Runde nicht einfriert.
- **Fenster zu:** Der Loop hält an, die Marke bleibt, beim nächsten
  Öffnen fährt er nach dem KI-Hinweis von selbst weiter. Beim Schließen
  wird höchstens vier Sekunden gewartet; steht die Runde gerade in einer
  Nachfrage des Betriebssystems, sichert der Herzschlag den Stand.
- **Mit Standardeinstellungen** steht der Hinweis im Gespräch des Tasks,
  der gerade beginnt.
- ⛔️ **Die Sicherheitsmeldung vor `auto`** jetzt auch im Fenster: Wer
  auf der Einstellungsseite `agent.modus` auf `auto` stellt, bekommt
  denselben Text wie in der Konsole, mit „Abbrechen“ und „Auf auto
  stellen“; ein Abbrechen lässt `manual` stehen.
- Die Loop-Einstellungen stehen jetzt beim Agenten und nicht beim
  Modell.

⚠️ **Fund 484: Der Kopf des Fensters sagte im `manual mode` elf Tage
lang, es gebe keinen Bestätigungskasten.** Seit dem 2026-09-15 legt das
Fenster jede schreibende Handlung im Kasten des Betriebssystems vor;
die Marke im Kopf und ihr Satz stammten aus der Zeit davor. Dieselbe
Angabe an zwei Orten. Jetzt: „fragt vor dem Schreiben (manual mode)“.

⚠️ **Fund 485: Drei Fensterproben und eine Konsolenprobe schnitten
Quelltext in Bytes.** `&senden[..senden.len().min(900)]` bricht ab,
sobald an der Schnittstelle ein mehrbytiges Zeichen liegt; heute lag
dort ein ⚑ aus einem Kommentar, und die Probe meldete „not a char
boundary“ statt etwas über das Umordnen. Alle vier schneiden jetzt mit
`floor_char_boundary`.

**Belegt:**
- `myl-client` 339 Proben grün, darunter fünf neue zur Schlange
  (Reihenfolge, Schlafen hält die Schlange, Lücken in der Reihe, Kette
  hinter dem Vorgänger, Entfernen nur eines Vorhabens) und zwei neue
  über ganze Runden (`fahren` in der Reihe Y, Kette Z, dann X, mit einer
  Leihe je Runde; ohne Modell endet der Loop und der Task bleibt).
- Gegenproben:
  - `faellig` auf „erstes fälliges“ statt „vorderstes“ macht die
    Schlangenprobe rot,
  - „beginnt“ vor der Leihe gemeldet macht die Leihprobe rot,
  - eine Kette ohne Einreihen ebenso.
- Konsole 150 und 7, Fenster 74, Clippy in allen drei Kisten ohne
  Warnung, `node --check` für das Skript.
- Die Logik des Fensters (Runde beginnt, Strom, Ende, Unterbrechung,
  Umsortieren, ∞ ohne Task) lief in einem Node-Probelauf mit
  DOM-Attrappe, fünf Fälle grün. Gegenprobe: Ohne den Tausch der Bühnen
  landet der Strom im Chat, und der Lauf wird rot. Der Probelauf liegt
  nicht im Repositorium, denn Node gehört nicht zu den Bauvoraussetzungen.
- ⚠️ **Nicht belegt: das Fenster, von Hand bedient.** Geklickt und
  gezogen hat es noch niemand.

⚠️ **Fund 482 bleibt offen:** `tests/reservierung.rs`, heute 1 von 6
rot (vorhin 2 von 6).

### v0.87.0 – 2026-09-26 (der Loop, Phase 2: `/loop` in der Konsole, Sicherheitsmeldung vor `auto`, Doppelsperre nach einer Fortsetzung; im `manual mode` blieb die Konsole nach einem `j` stehen; Fund 483)

**Worum es geht.** Phase 1 brachte das Vorhaben und `myl loop`. Jetzt
läuft der Loop dort, wo gearbeitet wird: in der Konsole, mit derselben
Anzeige, denselben Rückfragen und demselben Notaus wie ein gewöhnlicher
Auftrag.

**Neu in `myl-console` (0.24.0):**
- **`/loop [ziel]`** legt ein Vorhaben an und fährt fällige Runden. Ohne
  Ziel setzt es fort, was ansteht. Zwischen den Runden steht ein
  Countdown bis zur nächsten. **Solange der Loop läuft, gehört ihm die
  Konsole**; Esc pausiert ihn und gibt die Eingabe zurück.
- **`/tasks`** zeigt die Vorhaben; `/tasks fortsetzen <ID>` und
  `/tasks anhalten <ID>`. ⚑ Bewusst ohne deutschen Zweitnamen
  (Festlegung des Projektinhabers).
- **Jede Runde ist verdrahtet wie ein Auftrag:** Zeitleiste, Tokenstrom,
  `^S`, im `manual mode` die Vorlage jeder schreibenden Handlung. Danach
  stehen der Bericht, das Urteil des Prüfdurchgangs und der neue Zustand
  da, bei einer Kette auch das neue Vorhaben.
- **Pausieren und Schließen sind verschieden:**
  - Esc pausiert. Die Marke `.aktiv` geht, der nächste Start fährt nicht
    von selbst, `/loop` macht dort weiter.
  - Wird die Konsole geschlossen, während der Loop läuft (Fenster zu,
    Prozess beendet), bleibt die Marke. **Der nächste Start macht von
    selbst genau dort weiter**, samt der unterbrochenen Runde.
- **Mit Standardeinstellungen steht ein Hinweis in der Ausgabe**: die
  Grenzen, der Modus und wo sie sich ändern lassen (Festlegung des
  Projektinhabers).
- ⚑ **Die Sicherheitsmeldung vor `auto`** (Festlegung des
  Projektinhabers: Vorgabe ist immer `manual`, wer aktiv umstellt, wird
  gewarnt):
  - Umschalt-Tab zeigt sie und fragt `[j/N]`. Nur `j` stellt um.
  - Auf der Einstellungsseite braucht `auto` zwei Tastendrücke: Der
    erste zeigt die Meldung, der zweite stellt um.
  - `myl setzen agent.modus auto` fragt am Terminal; ohne Terminal steht
    die Meldung auf stderr.

  Der Text steht an einer Stelle (`einstellungen::autowarnung`), für
  alle drei Wege und in beiden Sprachen.

**Neu in `myl-client` (0.56.0):**
- ⚑ **Die Doppelsperre.** Setzt eine unterbrochene Runde fort, liefert
  ein Aufruf mit demselben Werkzeug und denselben Argumenten wie vor der
  Unterbrechung **das gespeicherte Ergebnis, statt noch einmal zu
  laufen**. Gebaut über `Werkzeugkasten::umhuellen` (AGENT_LAYER
  v0.21.0), also hinter der Erlaubnis: Die Sperre kann nur weglassen,
  nie erlauben. 📌 Der Hinweis im Auftrag allein reichte nicht; das
  0,6B-Modell schrieb nach einer Fortsetzung dieselben Dateien noch
  einmal.
- `Ablage::loop_war_aktiv` und `loop_aktiv_setzen` für die Marke.

⛔️ **Fund 483: Zwei Fäden lasen während eines Laufs die Tastatur, und
nach einem `j` im `manual mode` stand die Konsole.**
- Der Anzeigefaden liest seit dem 2026-09-15 (Vorlage, Abbruchfrage,
  `^S`), der Notauswächter aus v0.85.0 las daneben (Esc, Strg-C).
- `event::poll` und `event::read` nehmen die Lesersperre von crossterm
  je einzeln. Beide Fäden sahen das `j`, der Anzeigefaden las es, und
  der Notauswächter blieb in `event::read` stehen, **mit der Sperre in
  der Hand**. Am Ende des Laufs wartete der Hauptfaden auf ihn, er auf
  eine Taste, der Anzeigefaden auf die Sperre.
- Gefunden mit `sample` am hängenden Prozess, bei der ersten Rückfrage
  in einer Loop-Runde. **Der Fehler stand schon im gewöhnlichen
  Auftrag**. Warum er einen Tag lang nicht auffiel, ist nicht belegt;
  die Proben der Konsole laufen ohne Terminal, und ohne Terminal gibt es
  keinen der beiden Fäden.
- **Behoben:** Der Anzeigefaden ist der einzige Leser. Esc ruft eine
  Handlung, die ihm beim Start mitgegeben wird: im Auftrag den Notaus,
  im Loop die Pause. Steht eine Vorlage da, ist Esc deren Nein, und erst
  die nächste Esc hält an. Der Notauswächter ist entfernt.
- ⚑ **Strg-C hält damit nicht mehr an, sondern fragt „Wirklich
  beenden?“**, wie der Projektinhaber es am 2026-09-15 festgelegt hat.
  v0.85.0 hatte Strg-C zusätzlich als Notaus beschrieben; welcher Faden
  die Taste bekam, entschied der Zufall. **Der Notaus in der Konsole ist
  Esc.**
- 📌 **Zwei Leser derselben Quelle sind kein Wettlauf, der manchmal
  verloren wird, sondern einer, der bei jeder Taste verloren wird.**

**Belegt:**
- Konsole 150 und 7 Proben, darunter die neue `esc_haelt_an_im_einzigen_leser`.
  Gegenprobe: Ohne den Esc-Zweig im Anzeigefaden wird sie rot.
- `der_notaus_umschliesst_den_lauf` prüft jetzt, dass kein zweiter
  Tastenleser zurückkommt.
- Sechs Läufe über ein Pseudo-Terminal mit dem 0,6B-Modell, mit
  abgeschirmten Einstellungen (`MYL_EINSTELLUNGEN`):
  1. `/loop` bis „fertig“: 33 s.
  2. Esc pausiert, die Marke ist weg: 29 s.
  3. Hart beendet mitten in der Runde, der nächste Start macht von
     selbst weiter: 51 s.
  4. `manual mode` im Loop: Die Vorlage steht da, vor dem `j` gibt es
     keine Datei, danach schon, und die Runde endet: 35 s. Vor der
     Behebung hing genau dieser Lauf neun Minuten.
  5. Dasselbe im gewöhnlichen Auftrag: 31 s.
  6. Umschalt-Tab: Meldung, `n` lässt `manual`, `j` stellt um, danach
     schreibt der Loop ohne Rückfrage: 39 s.
- `myl-client` 247 Proben grün, dazu `tests/vorhaben.rs` 4 (auch die
  Doppelsperre: eine Datei, die nach der Unterbrechung von außen
  geändert wurde, bleibt so); local-agent 94, Fenster 74, Clippy ohne
  Warnung.

⚠️ **Fund 482 bleibt offen:** `tests/reservierung.rs` ist hier weiter
rot (2 von 6).

⚠️ **Was während des Loops getippt wird, geht verloren**, außer Esc
(und Strg-C im Countdown). Beobachtet beim Weitermachen von selbst: Ein
Auftrag, der direkt nach dem Start getippt wurde, kam nie an. Die
Konsole sagt beim Start, dass der Loop weiterläuft und Esc pausiert.

**Noch nicht (Phase 3):** ∞ neben dem Senden im Fenster, ein
Bestätigungskasten, die Liste der Vorhaben und die Sicherheitsmeldung
im Fenster.

### v0.86.0 – 2026-09-26 (der Loop, Phase 1: Vorhaben über viele Runden, Selbstwecken und Kette, Prüfdurchgang, Schließen und genau dort weiter; `myl loop`; Fund 482 offen)

**Worum es geht.** Ein Agentenlauf fährt einen Auftrag bis zum Ende und
vergisst ihn dann. Ein **Vorhaben** trägt ein Ziel über viele Runden:
Jede Runde ist ein gewöhnlicher Agentenlauf, und dazwischen bleiben
Notizen, Tagebuch und der Zeitpunkt der nächsten Runde. Das ist der
Anfang des Agent-Loops, den der Projektinhaber als einen der
wichtigsten Architekturpunkte gesetzt hat.

**Festlegungen des Projektinhabers (2026-09-26):**
- Kein eigener Prozess und keine Zeitsteuerung des Betriebssystems. Der
  Loop läuft im Fenster, in der Konsole oder in `myl loop`. **Wird
  geschlossen, hält er an und macht beim nächsten Öffnen genau dort
  weiter.**
- Auslöser sind **Selbstwecken** und **Kette**, auch zusammen.
- Aufsicht wie beim normalen Agenten: `manual` als Vorgabe.
- Der **Prüfdurchgang** ist eine Einstellung.

**Neu in `myl-client` (`vorhaben.rs`):**
- **Das Vorhaben**, als Dateien neben den Einstellungen
  (`vorhaben/<kennung>/`). Darin liegen das Ziel, der Zustand (bereit,
  schläft, fertig, angehalten mit Grund), ein begrenzter Notizblock
  (24 Einträge, je 600 Zeichen) und ein Tagebuch je Runde. Geschrieben
  wird atomar über eine Nebendatei, denn der Prozess darf jederzeit
  enden.
- **Vier Werkzeuge je Runde:**
  - `note_set` schreibt in den Notizblock,
  - `wake_in` legt die nächste Runde 1 Minute bis 7 Tage später,
  - `finish_goal` meldet das Vorhaben als fertig,
  - `chain_goal` stößt nach dem Fertigwerden ein neues Vorhaben an.

  Die Grenzen stehen im Code, nicht im Prompt.
- **Der Prüfdurchgang:** Nach der Runde fragt ein zweiter Durchgang ohne
  Werkzeuge nach Fortschritt und Ziel. Eine unlesbare Antwort zählt als
  nein. **Mit Prüfdurchgang entscheidet die Prüfung über „fertig“**,
  ohne ihn das Wort des Agenten. 📌 Gemessen mit dem 0,6B-Modell: Die
  Datei stand nach Runde 1, die Prüfung sagte dreimal „erreicht“, das
  Modell rief nie `finish_goal`, und zwei Runden liefen umsonst.
- **Grenzen** (Einstellungen, neuer Bereich „Loop“): Höchstzahl an
  Runden (50), Höchstdauer in Stunden (24, nur offene Zeit), Schritte je
  Runde (12), Pause nach Runden ohne Fortschritt (3), Prüfdurchgang
  (an).
- **Genau dort weiter:**
  - Nach jedem Werkzeugergebnis steht der Rundenstand in `runde.json`.
    Eine unterbrochene Runde bekommt vorgelegt, was schon erledigt ist.
  - Beim Schließen wird aus dem Weckzeitpunkt die Restwartezeit.
  - Der **Läufer** hält eine Sperre, damit Fenster und Konsole nicht
    gleichzeitig laufen, und schreibt alle 10 Sekunden einen
    **Herzschlag**. Stirbt der Prozess ohne Aufräumen, gilt der letzte
    Herzschlag als Zeitpunkt des Anhaltens.
  - **Schließen und Notaus sind verschieden:** Schließen lässt das
    Vorhaben, wie es war; der Notaus hält es an.
- **Das Tagebuch hält Tatsachen fest:** neben dem Bericht des Modells
  auch die tatsächlich ausgeführten Werkzeuge. 📌 Ohne das schrieb die
  Runde nach einer Fortsetzung dieselben drei Dateien noch einmal.
- **`MYL_EINSTELLUNGEN=<pfad>`** schirmt die Einstellungen ganz ab. Die
  Ablage der Vorhaben liegt daneben und ist mit abgeschirmt. 📌 Eingeführt,
  nachdem ein Probelauf mit `XDG_CONFIG_HOME` die echten Einstellungen
  überschrieben hatte: Eine vorhandene Datei gewinnt dort bewusst.

**Neu in `myl`:**
- `myl loop [ziel]` legt ein Vorhaben an oder setzt fort, zeigt die
  Werkzeugaufrufe und den Hinweis auf Standardeinstellungen. Strg-C
  (auch SIGTERM, SIGHUP) schließt sauber.
- `myl vorhaben [fortsetzen|anhalten ID]`.

**Belegt:**
- `vorhaben` mit 20 Modulproben und 4 Proben über ganze Runden (echte
  Rüstung und Agentenschleife, Drehbuchmodell): Notiz, Datei, Wecken,
  Kette, falsches „fertig“, ohne Prüfdurchgang, unterbrochen und
  fortgesetzt.
- Gegenprobe zur Sperre: Wer `EPERM` als „tot“ liest, übergeht die
  Sperre eines Fensters unter einem anderen Nutzer; die Probe wird rot.
- Echte Läufe mit dem 0,6B-Modell:
  - Ein Vorhaben schreibt `gruss.md` und ist nach Runde 1 fertig.
  - Strg-C mitten in einer Runde: 0 Runden gezählt, Rundenstand mit drei
    erledigten Aufrufen, Sperre freigegeben.
  - `myl loop` setzt fort.
- `myl-client` 246 Proben, Konsole 149, Fenster 74, Clippy ohne Warnung.

⚠️ **Fund 482, offen:** `tests/reservierung.rs` ist auf dieser Maschine
seit heute rot. `fcntl(F_PREALLOCATE)` bucht auf APFS nur 20 von 64 MB.
Der Code ist unverändert, und gestern war die Probe grün. Nicht
untersucht.

**Noch nicht (Phase 2 und 3):**
- `/loop` in der Konsole und ∞ im Fenster,
- ein Bestätigungskasten im Fenster,
- die Sicherheitsmeldung beim Wechsel auf `auto`,
- ein Schutz gegen das Wiederholen erledigter Aufrufe mit Wirkung nach
  außen nach einer Fortsetzung (heute steht der Hinweis nur im Auftrag,
  und das 0,6B-Modell hielt sich nicht daran).

### v0.85.1 – 2026-09-25 (aus einem kalten Klon ohne Netz gebaut und gefragt, auf macOS, Linux und Windows, dazu in GolemOS; unter Windows keine Konsole mehr hinter dem Fenster; Fund 474)

**Die Frage war:** Lässt sich alles, was ausgeliefert wird, aus einem
frischen Klon ohne Netz bauen, und antwortet Myelith danach? Gebaut
wurden jedes Mal die fünf Programme mit `ausliefern` im Manifest (`myl`,
`myelith`, `myl-oberflaeche`, `myl-node`, `myl-test`), ausschliesslich
aus `SYSTEM/crates-vorrat/` mit `--locked --offline`, und gefragt wurde
mit dem 0,6B-Artefakt: „Nenne die Hauptstadt von Frankreich."

| Ziel | Netz gesperrt durch | Bau | Antwort |
|---|---|---|---|
| macOS arm64 | `sandbox-exec`, Verbindung nach aussen nachweislich abgelehnt | alle fünf und `Myelith.app` | „Die Hauptstadt von Frankreich ist Paris.", 15,9 Token/s |
| Linux arm64 | Docker `--network none` | alle fünf, das Fenster eingeschlossen | dieselbe, 11,4 Token/s |
| Linux x86_64 | Docker `--network none`, emuliert | alle fünf | dieselbe, 6,5 Token/s |
| Windows x86_64 | Docker `--network none`, Kreuzbau mit MinGW | alle fünf als PE32+ | dieselbe unter Wine 10, 10,4 Token/s |
| GolemOS aarch64 | QEMU `-nic none` | `myl` und `myelith` statisch gegen musl, **aus dem Arbeitsbaum** | dieselbe, 1 Token/s unter TCG |

⚑ **Die Antworten sind bitgleich**, nicht nur inhaltlich gleich. Mit
eingeschaltetem Denken gaben macOS (nativ, NEON) und GolemOS (musl,
emuliert) über alle 24 Token dieselben Zeichen aus. Das ist die Zusage
der ganzzahligen Rechnung, hier an vier Betriebssystemen nachgesehen.

⚠️ **GolemOS kommt mit einem Klon noch nicht mit.** Seine Skripte und
Abbilder sind bis zur Fertigstellung vom Versionsstand ausgenommen,
damit jede Fassung der Abbilder nicht dauerhaft Geschichte kostet. Die
Zeile oben belegt deshalb, dass die ausgelieferten Programme in GolemOS
antworten, nicht, dass ein frischer Klon ein GolemOS ergibt.

⚠️ **Was der Windows-Weg nicht belegt:** Der Kreuzbau nimmt MinGW, die
Freigabe nimmt MSVC. Unter Wine brauchte `myl.exe` deshalb die
Laufzeitbibliotheken von MinGW daneben, und Wine 8 kannte `ProcessPrng`
aus `bcryptprimitives.dll` noch nicht, das jedes Windows ab 10
mitbringt. Beides sind Eigenschaften des Prüfwegs. Den Bau mit MSVC auf
einem echten Windows übernimmt im CI ein Ablauf je Betriebssystem, der
aus dem Vorrat ohne Netz baut und `myl --hilfe` startet.

**Fund 474 behoben: Unter Windows öffnete das Fenster eine Konsole
mit.** `myl-oberflaeche` trug kein `windows_subsystem`, also war die
ausführbare Datei ein Konsolenprogramm, und Windows stellte jedem Start
ein schwarzes Fenster hinter das eigentliche. Auf macOS und Linux gibt
es das nicht; aufgefallen ist es erst am Kreuzbau, an einer Zeile von
`file`: `PE32+ executable (console)`. Jetzt trägt der Freigabebau
`(GUI)`, der Prüfbau bleibt `(console)`, damit die Meldungen auf der
Fehlerausgabe beim Entwickeln sichtbar bleiben. Beides am Kreuzbau
gegengeprüft.

**Belegt:** `myl-oberflaeche` mit `cargo test` grün, 74 Prüfungen; die
Kreuzbauten wie oben.

### v0.85.0 – 2026-09-25 (KI-Verordnung: Hinweis bei jedem Start, gekennzeichnete Stimme, Schutzfilter, Aktionsprotokoll, Bestätigung als Vorgabe, Notaus; Funde 468 und 469)

`myl-client` **0.53.0 auf 0.54.0**, `myl-senses` **0.8.0 auf 0.9.0**,
`myl-console` **0.22.1 auf 0.23.0**, `myl-oberflaeche` **0.48.0 auf
0.49.0**. Auftrag des Projektinhabers: das Projekt nach der Verordnung
(EU) 2024/1689 aufstellen, technische Lösungen wie den manual mode
gesetzeskonform einstellen. Die Dokumente stehen unter `COMPLIANCE/`, die
Einordnung Artikel für Artikel in `COMPLIANCE/de/Selbsteinschaetzung.md`.

**Hinweis und Kennzeichnung (Art. 50 Abs. 1)**

- ⛔️ **Ein Hinweis bei jedem Start, aktiv zu bestätigen**
  (`kennzeichnung.rs`, ein Text für Fenster und Konsole): dass hier eine
  KI arbeitet, was sie kann, wo sie irrt, was verboten ist, mit Verweis
  auf die Zweckbestimmung. Im Fenster ein Dialog ohne Schließknopf, der
  auf Escape nicht reagiert und erst mit Haken und Knopf weggeht; alles
  darunter ist solange `inert`. In der Konsole vor der Agentenwarnung,
  mit Enter. `myl frage`, `agent` und `sitzung` schreiben eine Zeile auf
  die Fehlerausgabe, damit Skripte nicht hängen. **Kein Schalter**: Eine
  Kennzeichnung, die man abbestellen kann, ist keine.
- ⛔️ **Die KI-Marke steht dauerhaft da**: im Kopf des Fensters, unter jeder
  Antwort („KI-generiert"), vorn in der Fußzeile der Konsole, wo sie auch
  bei schmalem Fenster nie wegfällt.

**Die synthetische Stimme (Art. 50 Abs. 2)**

- ⛔️ **Jede erzeugte Tondatei wird gekennzeichnet, bevor sie jemand
  bekommt** (`myl-senses/src/kennzeichnung.rs`,
  `synthetisch_kennzeichnen`): ein XMP-Block mit dem IPTC-Quellentyp
  `trainedAlgorithmicMedia` und ein RIFF-INFO-Kommentar, dazu ein
  Wasserzeichen im Signal (eine feste Folge aus ±1, alle 4096 Proben
  wiederholt, ein Vierundsechzigstel der örtlichen Lautstärke). Jedes
  Stück, jedes Satz-WAV, jedes Ergebnis von `sagen` und jeder Satz aus der
  Überbrückungsablage. **Was sich nicht kennzeichnen lässt, klingt
  nicht.** Ganzzahlig, auch die Gleitkomma-WAVs des Sprechmodells werden
  als Bitmuster gelesen.
- **Gemessen an 14 Sätzen des Sprechmodells**: ohne Kennzeichnung
  höchstens 2,5 Standardabweichungen, gekennzeichnet mindestens 11,9, vorne
  abgeschnitten und halb so laut mindestens 11,8, nach MP3 mit 128 kbit/s
  mindestens 10,3 (Schwelle 4 am Anfang, 6,5 gesucht). 📌 Ohne Aufhellen
  vor dem Falten (Differenzen benachbarter Proben) reichten zwei Sekunden
  nicht, um eine abgeschnittene Datei zu erkennen.
- 📌 **Die Überbrückungssätze lagen ungekennzeichnet in der Ablage**, und
  ihr Name hängt nicht an der Kennzeichnung. Deshalb wird auch ein Satz aus
  der Ablage vor dem Spielen gekennzeichnet.
- `myl kennzeichen <datei.wav>` prüft beide Marken (Rückgabe 0, 1, 2).
- ⛔️ **Keine Stimme ohne Einwilligung**: Hochladen geht erst mit dem Haken
  „meine eigene Stimme oder eingewilligt", und `stimme_setzen` lehnt ohne
  `einwilligung: true` ab.

**Verbotene Praktiken (Art. 5)**

- ⛔️ **Ein Schutzfilter an allen fünf Eingängen** (`schutzfilter.rs`:
  Chat und Agent im Fenster, Konsole, `myl frage`, `myl agent`). Er schlägt
  an, wenn Handlung, Gegenstand und Ziel zusammen vorkommen („erkenne die
  Emotionen meiner Mitarbeiter"), nicht bei Fragen darüber („was ist
  Emotionserkennung"). Sieben Klassen; die Abweisung nennt den Grund und
  die Zweckbestimmung und steht im Aktionsprotokoll, ohne Text. `myl`
  gibt dann 3 zurück. ⚠️ Eine Hürde, keine Mauer.
- ⛔️ **Eine Regel vor jeder Frage an das Sehmodell** (`SEHREGEL`): nur
  Sichtbares beschreiben, niemanden identifizieren, keine Gefühle oder
  sensiblen Merkmale zuschreiben.

**Aufsicht und Protokoll (nach dem Vorbild von Art. 12 und 14)**

- ⛔️ **`manual mode` ist die Vorgabe**: Schreiben, Befehle und
  Web-Anfragen werden vorgelegt. Eine Ablage, die `auto` ausdrücklich
  trägt, behält es.
- ⛔️ **Fund 469: Zwei Wege liefen ohne Nachfrage.** Web-Anfragen waren nie
  in die Nachfrage gehüllt, und der Chat mit Anhang oder Recherche reichte
  im Fenster gar keine durch (`None`); ein geänderter Anhang und jede
  Suche liefen also auch im `manual mode` ungefragt. Jetzt fragt beides,
  über eine gemeinsame `nachfrage_fuer`.
- ⛔️ **Das Aktionsprotokoll** (`protokoll.rs`): jede Handlung des Agenten
  als JSON-Zeile (Zeit, Art, Werkzeug, Entscheidung, Ergebnis, Dauer),
  Ein- und Ausgabe nur als Fingerabdruck (SHA-256 mit einem Schlüssel, der
  auf dem Rechner zufällig entsteht; ohne ihn lässt sich nicht einmal ein
  Dateiname zurückraten). Eine Datei je Tag, nach 30 Tagen gelöscht. Jede
  Einhängung der Rüstung geht durch die Hülle; die drei Bedieninstrumente
  schalten es ein, die Bibliothek allein schreibt nichts. Anzeige in den
  Einstellungen des Fensters und mit `myl protokoll`; `MYL_PROTOKOLL`
  lenkt den Ort um. `sha2` kam ohne neue Kiste dazu.
- ⛔️ **Der Notaus** (`notaus.rs`): im Fenster ein Knopf im Kopf und ⌘. oder
  Strg+., in der Konsole Strg-C oder Esc während eines Laufs (ein Wächter
  liest dann die Tasten, im Rohmodus kommt Strg-C nicht als Signal). Er
  hält die Erzeugung vor dem nächsten Token an (Runtime 0.66.0), `chat`
  meldet `Abgebrochen` mit dem Text bis dahin, jedes weitere Werkzeug wird
  verweigert und protokolliert, die Stimme verstummt. Das Gespräch bleibt;
  der angehaltene Text steht als Antwort da. ⚠️ Ein Werkzeug, das gerade
  läuft, läuft zu Ende.

**Lizenzen**

- ⛔️ **Fund 468: Die genaue Sehstufe stand unter einer Forschungslizenz.**
  Das Einrichtungsskript holte `Qwen2.5-VL-3B-Instruct` als GGUF, dessen
  Umwandlung Apache 2.0 nannte; das Original steht unter der Qwen Research
  License. Ersetzt durch `Qwen3-VL-4B-Instruct` (Apache 2.0, GGUF vom
  Hersteller). Gleiches Bild, gleiche Frage: Das neue las Namen und
  Unterzeile und beschrieb den Hintergrund, das alte nannte das Projekt
  ein „Unternehmen"; 4,7 statt 2,9 s samt Laden. Das Skript ersetzt eine
  alte Datei, die es an ihrer Größe erkennt.

**Belegt:** `myl-client` 226 in der Bibliothek, dazu
`tests/notaus.rs` (neu, eigener Prozess, am 0,6B), `aufruf.rs`,
`dateiwerkzeuge.rs` (drei neu); `myl-senses` 57 und 27;
`myl-oberflaeche` 74; `myl-console` 149. Neu unter anderem
`beide_marken_und_keine_falschen`, `nur_gekennzeichneter_ton_klingt`,
`ein_alter_satz_aus_der_ablage_wird_vor_dem_spielen_gekennzeichnet`,
`verbotene_anfragen_werden_abgewiesen`,
`fragen_darueber_und_alltag_gehen_durch`, `jeder_eingang_filtert`,
`die_nachfrage_haelt_schreiben_und_netz_auf_und_laesst_lesen`,
`das_aktionsprotokoll_haelt_fest_ohne_klartext`,
`jedes_werkzeug_ist_protokolliert_und_jeder_start_schaltet_ein`,
`der_notaus_haelt_an_und_behaelt_den_text`,
`der_ki_hinweis_kommt_bei_jedem_start_und_laesst_sich_nicht_wegklicken`,
`der_notaus_ist_immer_da_und_haelt_alles_an`,
`keine_stimme_ohne_einwilligung_und_das_protokoll_ist_sichtbar`,
`die_ki_marke_steht_vorn_und_faellt_nie_weg`,
`der_notaus_umschliesst_den_lauf`. Über vierzig Gegenproben, jede beißt.
Die Probe der Grenzregler verbot Rot zu Recht (die Farben sind
Graustufen): Der Notaus hebt sich durch Rand und Stoppzeichen ab.
📌 Die erste Fassung der Protokollprobe nahm die jüngste `read_file`-Zeile
und flatterte, weil andere Proben desselben Prozesses mit hineinschreiben;
sie findet ihre Einträge jetzt über den Fingerabdruck der eigenen Eingabe
(fünf Läufe in Folge grün).
clippy ohne Befund.

### v0.84.0 – 2026-09-25 (ein Denkbudget beim Vorlesen, mit Schieberegler; Vorgabe 32 Token, gemessen)

`myl-client` **0.52.2 auf 0.53.0**, `myl-oberflaeche` **0.47.0 auf
0.48.0**, `myl-console` **0.22.0 auf 0.22.1**. Auftrag des
Projektinhabers: den Denkmodus beim Vorlesen begrenzen statt abschalten,
mit einem Schieberegler in den Einstellungen, stufenlos bis unbegrenzt,
und einer Vorgabe, die auf kurze Wartezeit getrimmt ist und trotzdem die
nötige Überlegung lässt. Die Grenze selbst sitzt in der Erzeugung
(INTEGER_LLM v0.95.0).

- ⚑ **`modell.denkbudget`**: höchstens so viele Token Überlegung, wenn
  vorgelesen wird und „Vor dem Antworten denken" an ist. `null` heißt
  unbegrenzt, `0` gar nicht. Gilt je Antwort und wird danach
  zurückgesetzt, damit der Agent es nicht erbt.
- ⚑ **Null ist die leere Überlegung**, nicht ein Einschub nach null
  Token (`Oertlichesmodell::denkt`): Die leere ist die, auf die das
  Modell trainiert ist; ein sofortiger Einschub sähe aus wie eine
  abgebrochene.
- ⚑ **Ist das Budget erschöpft, wird ein kurzer Übergangssatz und
  `</think>` eingeschoben** (`DENKSCHLUSS`); die Endmarke leitet der
  Client aus dem Wortschatz ab und nur, wenn sie ein einziges Token ist.
- ⚑ **Eine neue Feldart `Budget`** statt `Grenze`: Eine Grenze gibt ein
  Betriebsmittel der Maschine frei, ihr Ende kennt der Hardwarescan, und
  sie steht unter den Grenzen des Rechners. Das Ende des Budgets steht
  fest in der Kiste (`DENKBUDGET_BIS`, 2048) und geht als `bis` mit dem
  Feld ins Fenster. Das Feld steht nur im Fenster, denn nur dort wird
  vorgelesen, während das Modell schreibt.
- ⚑ **Der Regler ist quadratisch**: links „nicht denken", rechts
  „unbegrenzt", die Mitte bei rund 500 Token. Die hörbaren Unterschiede
  liegen bei kleinen Budgets.
- ⚠️ **Streng beim Setzen**: Ein Tippfehler ist ein Fehler. Über den
  Helfer der Grenzen würde er still zu „unbegrenzt", also zur längsten
  Wartezeit, die es gibt.
- ⚑ **Vorgabe 32, gemessen und vom Projektinhaber gewählt.** Zwölf
  Fangfragen, bewertet am Schlusssatz der Antwort:

| Budget | 30B | 8B | bis zum ersten Wort der Antwort (30B / 8B) |
|---|---|---|---|
| 0 | 10/12 | 10/12 | 1,6 s / 0,4 s |
| **32** | **12/12** | **12/12** | 6,7 s / 4,1 s |
| 64 | 12/12 | 11/12 | 9,1 s / 6,1 s |
| 128 | 12/12 | | 13,4 s / 10,3 s |
| unbegrenzt | | | Median 72 s, bis 187 s (30B) |

  Ohne Überlegung fielen genau die Fangfragen durch („Sally hat 2
  Schwestern", „r kommt 2-mal vor", „5:15 Uhr" statt 3:15). Zehn
  gewöhnliche Rechen- und Wissensfragen waren bei jedem Budget richtig.
  Eine erste Auswertung hatte die Fehler ohne Überlegung übersehen, weil
  sie irgendwo im Text nach dem Muster suchte; bewertet wird deshalb nur
  der Schlusssatz.
- Eine vorhandene Ablage ohne das Feld bekommt die Vorgabe und nicht die
  unbegrenzte Überlegung.

**Belegt:** `myl-client` 218 in der Bibliothek, neu
`das_denkbudget_hat_eine_vorgabe_und_ein_offenes_ende` (drei Gegenproben
beißen: Tippfehler still unbegrenzt, keine Vorgabe für alte Ablagen,
Budget als Freigabe), und am 4B
`das_denkbudget_beendet_die_ueberlegung_und_es_kommt_eine_antwort`
(Schlussfolge in der Überlegung, Endmarke genau einmal, Antwort „391" im
Strom als Text; mit null keine Überlegung; zwei Gegenproben beißen).
`myl-oberflaeche` 71, neu `das_denkbudget_ist_ein_schieber_mit_festen_enden`
(vier Gegenproben beißen); die Prüfung der Grenzregler gilt jetzt nur
noch `reglerzeile`, denn der Budgetregler beginnt mit Absicht bei null.
`myl-console` grün. clippy ohne Befund; die Zusicherung, dass die Vorgabe
auf der Skala liegt, wird beim Übersetzen geprüft.

### v0.83.0 – 2026-09-25 (die Stimme spricht, während das Modell schreibt: erster Ton nach drei bis acht Sekunden; Funde 463 bis 466)

`myl-senses` **0.7.0 auf 0.8.0**, `myl-oberflaeche` **0.46.0 auf
0.47.0**. Auftrag des Projektinhabers: die Latenz der Sprachausgabe
senken, bis ein flüssiges Gespräch ohne lange Wartezeit möglich ist, und
während das Modell nachdenkt einen Satz wie „Lass mich kurz darüber
nachdenken" sprechen.

**Der Sprecher allein**, ein Satz von 62 Zeichen, vom Eingang beim
Läufer bis zum ersten hörbaren Stück:

| Stand | erster Ton | RTF |
|---|---|---|
| vorher: alles auf der CPU, zehn Flussschritte, ganzer Satz auf einmal | 9,1 s | 2,1 |
| Fluss auf der Grafikeinheit, sechs Schritte, stückweise | 2,1 bis 2,3 s | 0,86 |
| dazu die Stimmprobe an einer Pause gekürzt | **1,6 bis 1,8 s** | 0,79 |

**Im ganzen Gespräch**, ab dem Abschicken, Antwort ohne Nachdenken, je
drei Läufe bei 4B und 8B:

| Hauptmodell | erster Ton vorher (Läufer 2, lange Probe) | erster Ton jetzt | Lücken jetzt |
|---|---|---|---|
| 4B | nicht gemessen | **3,0 bis 3,3 s** | 2,1 bis 2,3 s |
| 8B | 5,8 s | **4,3 bis 4,6 s** | 3,4 bis 3,6 s |
| 30B-A3B | 9,7 bis 9,8 s | **7,8 s** | 3,4 s |

- ⚑ **Läufer Fassung 2** (`laeufer/sprechen-cosyvoice.py`): Der Fluss
  (DiT) rechnet auf der Grafikeinheit, gemessen 1,8 statt 5,4 bis 6,3 s;
  das Token-Sprachmodell bleibt auf der CPU (48 gegen 31 Token/s), der
  Vocoder auch (`float64`). **Sechs statt zehn Flussschritte**, geprüft
  durch Zurückhören mit whisper (bei 10, 6 und 4 Schritten wortrichtig)
  und die Ähnlichkeit zur Stimmprobe (0,80 bis 0,89, ohne Gang mit der
  Schrittzahl). Die Stimme wird einmal gemerkt statt je Satz vorbereitet
  (rund 1 s je Satz). Ein Aufwärmsatz vor `bereit`.
- ⚑ **Stückweise sprechen:** Der Läufer meldet jedes hörbare Stück mit
  `stueck\t<teil.wav>`, sobald es fertig ist; der Vorleser spielt es
  sofort und räumt es weg. Ein Läufer der Fassung 1 meldet nur `ok`, und
  dann klingt der Satz als Ganzes.
- ⚑ **Der Läufer wird erneuert, wenn niemand ihn angepasst hat:**
  ersetzt wird nur eine Datei, deren Fingerabdruck (FNV-1a, 64 Bit)
  einer früher ausgelieferten Fassung gleicht. Angepasste bleiben.
- ⚑ **Das Fenster spielt die Stücke lückenlos**, mit Web Audio auf die
  Probe genau hintereinander geplant (`stimme_einplanen`) statt je Stück
  ein neues Element abzuspielen.
- ⚑ **Das erste Stück darf am Komma enden**, ab 20 Zeichen
  (`ERSTES_STUECK_MINDESTENS`); danach nur am Satzende.
- ⚑ **Die Stimmprobe wird beim Hochladen gekürzt** (`probe_kuerzen`):
  Ist sie länger als 5,5 s, wird sie an der leisesten Stelle zwischen 3,5
  und 5,5 s geschnitten, und erst danach schreibt whisper ihren Text mit.
  Gemessen: 7,1 s Probe kosteten je Satz rund 0,5 s mehr als 4,4 s, bei
  kaum anderer Ähnlichkeit (0,859 bis 0,888 gegen 0,854 bis 0,870). Der
  Kopf wird blockweise gelesen, denn ffmpeg schreibt einen `LIST`-Block
  vor die Daten.
- ⚑ **Der erste Ton geht vor** (`sprechen::Vorrang`): Ist das erste Stück
  beim Sprecher und klingt noch nichts, hält die Erzeugung im Fenster an,
  höchstens sechs Sekunden. Der erste Ton kommt damit beim 4B 0,65 s, beim
  8B 1,0 s früher; die Lücken wachsen beim 8B und 30B um rund eine
  Sekunde, verteilt über eine halbe Minute Sprechen.
- ⚑ **Nachdenken wird überbrückt:** fünf Sätze je Sprache
  (`ueberbrueckungen`), vorbereitet beim Einschalten der Stimme und
  abgelegt unter `<Heimat>/ueberbrueckung/`, ihr Name ein Fingerabdruck
  über Läufer, Stimme und Satz. Gespielt wird höchstens einer je Antwort,
  beim ersten Denkstück, nur wenn noch nichts gesprochen wurde und nur
  aus der Ablage; er wird nie erst gerechnet, denn das hielte die Antwort
  auf, die er überbrücken soll.
- ⛔️ **Fund 463: CosyVoice verdoppelt beim Streamen `token_hop_len` und
  setzt es nie zurück.** Ab dem zweiten Satz wartete das erste Stück
  damit auf doppelt so viele Token. Der Läufer setzt den Wert vor jedem
  Satz auf den Anfang zurück, ohne den CosyVoice-Baum anzufassen.
- ⛔️ **Fund 464: Markdown ging an die Stimme.** „Eine \*\*Synapse\*\*"
  kam mit den Sternen bei CosyVoice an. `sprechbar` nimmt Hervorhebungen,
  Code-Striche, Überschrifts-, Aufzählungs- und Tabellenzeichen weg; ein
  Verweis behält seinen Text. Ein Stück ohne ein einziges Wort
  (`zu_sprechen`) geht gar nicht erst hinaus.
- ⛔️ **Fund 465: Eine Abkürzung beendete ein Stück.** Aus „(z. B.
  Glutamat)" wurde ein eigenes Stück „(z.", gesprochen als Satz mit
  Pause. `ist_abkuerzung` prüft das letzte Wort vor dem Punkt gegen eine
  kleine Liste.
- ⛔️ **Fund 466: Die Überbrückungssätze wuchsen mit jeder Stimme.** Jede
  neue Probe oder jeder neue Läufer brachte fünf neue Dateien, und die
  alten blieben: fünfzehn für fünf Sätze. Jetzt räumt das Vorbereiten
  weg, was nach dieser Ablage aussieht und zu keiner jetzigen Stimme in
  keiner Sprache gehört; fremde Dateien bleiben.
- Zwei Doc-Kommentare standen an der falschen Stelle, einer davon seit
  `myl-senses` 0.1.0 (die Beschreibung von `abspielen` hing an
  `wav_dauer_s`); verschoben.

⛔️ **Nicht übernommen, weil gemessen schlechter:** das Sprachmodell des
Sprechers auf der Grafikeinheit (4B: erster Ton 4,8 statt 3,8 s, Lücken
13 statt 2 s, denn das Hauptmodell rechnet dort mit); ein Rückstau, der
das Hauptmodell bei jedem offenen Satz anhält (30B: 10 s Lücken); zwei
statt zehn Fäden für den Sprecher und eine Kerngrenze für das
Hauptmodell (beides ohne Gewinn).

⚠️ **Was bleibt, ist das Hauptmodell.** Das 30B braucht auf dieser
Maschine neben dem Sprecher 2,7 s bis zum ersten Wort, und beide teilen
sich 24 GiB; das Sprachmodell des Sprechers fiel daneben von 48 auf 13
Token/s. **Mit Nachdenken** schrieb das 30B das erste Wort der Antwort
nach 44 s; der Überbrückungssatz kommt nach rund drei Sekunden, danach
ist es still.

**Belegt:** `myl-senses` 78 Proben grün (53 in der Bibliothek, 25 mit
Attrappe), neu: `markdown_wird_sprechbar`,
`abkuerzungen_beenden_kein_stueck`,
`ein_stueck_ohne_worte_wird_nicht_geschickt`,
`die_stimmprobe_wird_an_der_pause_gekuerzt`,
`der_vorrang_wartet_auf_den_ersten_ton`,
`das_nachdenken_wird_mit_einem_vorbereiteten_satz_ueberbrueckt`
(erweitert um das Aufräumen), `der_dauerlaeufer_meldet_seine_stuecke`,
`der_vorleser_spielt_die_stuecke_sobald_sie_kommen`,
`das_erste_stueck_darf_am_komma_enden`, `der_fingerabdruck_ist_fnv1a`,
`nur_ein_unveraenderter_alter_laeufer_wird_ersetzt`. Die zwölf
Gegenproben zu den ersten sechs beißen (unter anderem: Schnitt fest am Fensterende, Kopf mit
festen 44 Bytes, kein Warten, Ton schon beim Abschicken gesetzt, nicht
aufgeräumt, auch fremde Dateien gelöscht). `myl-oberflaeche` 70,
`myl-client` und `myl-console` grün; clippy ohne Befund.

### v0.82.0 – 2026-09-25 (das Vorschaltbild zeigt eine von fünf gezeichneten Szenen statt des Schneesturms)

`myl-oberflaeche` **0.45.0 auf 0.46.0**. Auftrag des Projektinhabers:
fünf Bilder als Vorlage, dezent und in Schleife bewegt, beim Start eins
davon gewürfelt; der Schriftzug bleibt, getauscht wird nur der
Schneesturm dahinter.

⚑ **Nachgebaut und nicht eingebettet**, so entschieden vom Projektinhaber: Ein
fremdes Bild trägt ein Urheberrecht, eine Bildidee nicht. Neu ist
`ui/vorhang.js` mit fünf gezeichneten Szenen; `ui/netz.js` (Rauschen,
Sog, Netz) entfällt.

| Szene | Vorlage | Bewegung |
|---|---|---|
| Wellenbänder | gekörnte helle Bänder um einen dunklen Kern, außen unscharf | Flug in den Tunnel |
| Vielecke | gestaffelte Sieben- und Sechsecke, Kanten zu Bündeln | Flug in den Schacht |
| Dreieckswelle | helle Welle aus flach schattierten Dreiecken auf Schwarz | die Welle hebt und senkt sich |
| Wirbel | Flecken mit Rissen, zur Spirale verdreht, außen Strahlen | Sog in den Kern |
| Schleifen | netzartige Bänder in Schlaufen auf hellem Grund | Flug ins Mandala |

- ⚑ **Eine Schleife ohne Naht.** Vier Szenen sind selbstähnlich gebaut:
  Jeder Ring ist der vorige, vergrößert um `q` und gedreht um `dreh`.
  Die Bewegung führt genau diese Abbildung aus und steht nach einem
  Umlauf wieder am Anfang. Je Bild wird nur gedreht und vergrößert,
  nichts neu gerechnet.
- 📌 **Die erste Fassung sprang an der Wende**, gemessen in einer
  Vorschau als mittlere Abweichung zwischen dem letzten Bild vor und dem
  ersten nach der Wende: bis 22 Graustufen, eine halbe Periode brachte
  39. Drei Ursachen: Helligkeit, Tiefe und Grund hingen in der Textur am
  Radius (jetzt fest am Schirm, `radial`), ein Schlaufenterm drehte nicht
  mit dem Ring, und das Rauschen des Wirbels passte nicht ganzzahlig in
  einen Umlauf. Was dann noch blieb, war Raster: Eine um `q`
  hochgezogene Textur ist weicher. Dagegen blendet `schleife` über den
  Umlauf in sich selbst einen Umlauf weiter über; bei hellen Linien
  additiv, damit sich deckende Linien nicht abschwächen. **Jetzt liegt
  jede Wende unter dem Unterschied zweier Bilder im Abstand einer
  zweihundertstel Periode.**
- 📌 **Die Unschärfe der Tiefe entsteht durch Halbieren und
  Hochziehen**, nicht über `filter` auf der Leinwand: Die Webansicht
  unter macOS zeichnete damit gemessen scharf. Ein einziger großer
  Verkleinerungsschritt machte aus den feinen Perlen grobes, flackerndes
  Rauschen; in Halbierungen mittelt jeder Schritt sauber.
- ⚑ **Ein weicher dunkler Hof in der Mitte** hält den Schriftzug auf
  jeder Szene lesbar, auch auf dem hellen Mandala.
- ⚑ **Wer Bewegung abbestellt hat, sieht ein Standbild der Szene**
  statt gar keines; die Leinwand wird dafür nicht mehr ausgeblendet.
- ⚑ **Der Schriftzug erscheint nach 0,2 statt 2,4 Sekunden.** Die alte
  Verzögerung war auf den Sog des Schneesturms abgestimmt; mit ihr wäre
  er vor dem Weggehen des Vorhangs oft gar nicht erschienen.
- Aufbau bis zum ersten Bild in der Vorschau 1 bis 110 ms, am längsten
  der Wirbel, der sein Rauschen pixelweise rechnet.

**Belegt:** 70 Proben in `tests/oberflaeche.rs`, alle grün, eine neu
(`das_vorschaltbild_wuerfelt_unter_fuenf_szenen`: fünf Szenen, Wurf
über alle, keine feste Nummer im Fenster), drei angepasst
(Fremdquellen, Abgang, abbestellte Bewegung); fünf Gegenproben beißen.
Alle Szenen in einer Vorschau mit dem echten Stilblatt gezeichnet, die
Nähte gemessen.

### v0.81.0 – 2026-09-25 (das Ladezeichen schweigt beim Schreiben, das Terminal ist ein Terminal, die Einstellungen rollen nur senkrecht)

`myl-oberflaeche` **0.44.0 auf 0.45.0**. Drei Meldungen des
Projektinhabers nach dem ersten Blick auf v0.80.0.

**Das Ladezeichen steht nur, solange geladen oder nachgedacht wird.**
Sobald das Modell Antworttext schreibt, geht es, denn dann ist der
wachsende Text selbst die Auskunft. Es kommt wieder bei jedem Denken,
jedem Werkzeugaufruf und jedem neuen Schritt: Nach einem Werkzeug
rechnet das Modell oft eine halbe Minute ohne sichtbaren Zuwachs, und
genau diese Lücke hatte Fund 292 geschlossen. Die Unterscheidung trägt
ein Feld am laufenden Beitrag (`schreibt`), gesetzt von der Meldung,
die gerade ankommt; eine Verdichtung ändert es nicht. Der Kasten des
Zeichens entsteht jetzt an einer Stelle (`laufzeichen_bauen`), denn er
wird an zweien gebraucht.

**Das Terminal ist kein Formular mehr.** Die getönten Kästen um Ausgabe
und Eingabe sind weg, ebenso die Knöpfe „Ausführen" und „Leeren". Auf
dem Grund des Fensters steht die Ausgabe, direkt darunter die
Eingabezeile mit Ordner und `$`, und beides rollt zusammen.

- Ausgeführt wird mit der Eingabetaste, geleert mit `clear` oder
  Befehlstaste K (Steuerung L unter Linux und Windows).
- Während ein Befehl läuft, verschwindet die Eingabezeile und kommt
  wieder, wenn er fertig ist.
- Ein Klick irgendwo ins Terminal setzt den Einfügepunkt in die
  Eingabezeile, außer es wurde gerade Text markiert.
- 📌 **Das Eingabefeld gibt alles einzeln ab**, was es als Feld
  erkennbar macht: Grund, Hintergrundfilter, Schatten, Polster,
  Rundung. Es erbt all das aus der Regel für Bedienelemente; wer nur
  den Grund wegnimmt, behält den Filter als milchigen Streifen.
- Die Sätze `terminal.senden` und `terminal.leeren` sind entfallen.

**Die Einstellungsseite rollt nur senkrecht.** Ihr Raster hat jetzt eine
Spalte `minmax(0, 1fr)`, dazu `overflow-x: hidden`. 📌 Ein Rasterelement
ist vorgabemäßig mindestens so breit wie sein breitester Inhalt, und
`overflow-y: auto` allein setzt die andere Achse ebenfalls auf `auto`.
⚠️ Mit den Daten dieser Maschine ließ sich das seitliche Rollen in der
Vorschau nicht nachstellen; die beiden Angaben schließen es für jeden
Inhalt aus.

**Der Schließknopf steht genau auf dem Zahnrad.** Er hängt an denselben
Zahlen wie das Zahnrad im Kopf (`--kopf-polster` von oben und rechts,
dieselbe Knopfgröße) und ist am Fenster geheftet, rollt also nicht mit.
Vorher stand er im Fluss der Seite, 12 Punkte links und 12 unter dem
Zahnrad. Nachgemessen bei 760, 1100 und 1400 Punkt Breite: dieselbe
Lage auf den Zehntelpunkt, nachdem der halbe Punkt der Kopflinie
berücksichtigt ist.

**Nachgezogen:** `tauri.conf.json` trägt die Kistenversion (siehe
Nachtrag in v0.80.0).

**Belegt:** 69 Proben in `tests/oberflaeche.rs`, alle grün, drei neu
(`das_terminal_ist_kein_formular`,
`die_einstellungsseite_rollt_nur_senkrecht`,
`der_schliessknopf_steht_auf_dem_zahnrad`), eine erweitert
(`das_ladezeichen_steht_beim_beitrag`: es schweigt beim Schreiben und
kommt danach wieder); dreizehn Gegenproben beißen. Clippy ohne Befund.
Terminal und Einstellungsseite in einer Vorschau mit dem echten Skript
und den Einstellungsdaten dieser Maschine nachgesehen.

### v0.80.0 – 2026-09-24 (das Fenster in flüssigerem Glas, und ein synaptischer Spalt statt dreier Punkte; Fund 459)

`myl-oberflaeche` **0.43.0 auf 0.44.0**. Zwei Wünsche des
Projektinhabers: das Glas der Bedienelemente näher an echtes flüssiges
Glas, und ein Ladezeichen, das zu Myelith passt.

**Das Ladezeichen ist ein synaptischer Spalt.** Zwei Axonenden laufen
geschwungen von den Rändern auf einen schmalen Spalt zu, jedes als
Körper mit einem Verlauf von oben hell nach unten dunkel und einer
Glanzlinie auf dem Rücken, damit es rund wirkt und nicht flach. Nach
aussen laufen sie über eine Maske ins Nichts aus. Im Sender schwellen
drei Bläschen, fünf Botenstoffe springen über den Spalt, die
Empfängerfläche leuchtet auf, zwei feine Entladungen zucken durch den
Spalt. Ein Takt dauert 1,6 Sekunden. Gebaut nach zwei Bildern, die der
Projektinhaber als Anregung geschickt hat, und nach seinem Wunsch nach
mehr geschwungenem Axon und mehr Tiefe.

- ⚑ **Jedes Ladezeichen trägt eigene Kennungen.** Verläufe und Maske
  werden über `url(#…)` angesprochen. Mit festen Kennungen löste jedes
  Zeichen auf das erste im Dokument auf; verschwindet dieses, während
  ein zweiter Beitrag noch läuft, stünde das zweite als leerer Umriss
  da. Ein Zähler an der Funktion hält sie auseinander.
- ⚑ **Kein `style`-Attribut, alles als Klasse.** Die Inhaltsrichtlinie
  verbietet eingebettete Stile; die Farben der Verlaufsstufen stehen
  deshalb als Klassen im Stil, in Graustufen.
- ⚑ **Keine Auskunft über den Fortschritt.** Wie die drei Punkte
  vorher sagt das Zeichen nur, dass gerechnet wird.
- ⚑ **Wer Bewegung abbestellt hat, sieht den Spalt stehend**, die
  Botenstoffe mitten darin. Ein Ladezeichen, das dann verschwände,
  nähme die Auskunft genau dem, der sie am ehesten braucht.

**Das Glas.** 📌 **Was flüssiges Glas ausmacht, ist vor allem die
Brechung am Rand**, und die zeichnen die Webansichten unter macOS und
Linux nicht: Ein Hintergrundfilter über eine eigene Verschiebungskarte
kennt nur die Chromium-Ansicht unter Windows. Geprüft unter macOS 26.6
auch der Weg über ein systemeigenes Glasmaterial: Die Webansicht kennt
es nicht, auch nicht mit einem nicht öffentlichen Schalter. Gebaut
wurde deshalb, was auf allen drei Systemen trägt, und das sind vier
Eigenschaften:

| Eigenschaft | vorher | jetzt |
|---|---|---|
| Unschärfe | stark, milchig | schwächer (10 bis 14 px), dazu Sättigung 180 % und etwas Helligkeit: Der Grund bleibt erkennbar und leuchtet durch |
| Dicke | eine Haarlinie | geschichtete innere Schatten: Lichtkante oben, Schattenkante unten, weicher Lichteinfall oben, Tiefe unten (`--dicke`) |
| Rand | gleichmässig | winkelabhängig: hell oben links und unten rechts, dunkel an den Seiten, stärker beim Überfahren (`--rand`, `--rand-hell`) |
| Glanz | keiner | ein schräger Spiegel über der oberen Hälfte (`--spiegel`) |

Dazu gibt ein Knopf beim Drücken nach wie ein weicher Körper
(`scale(.95, .92)`), und runde Knöpfe wachsen beim Überfahren etwas.
Der Lichtton `--glanz` bleibt im hellen Anstrich weiss, denn Licht auf
Glas ist in beiden Anstrichen hell. ⚑ Das revidiert die frühere
Festlegung auf mattes Glas mit Haarlinie; die Graustufen bleiben.

⚠️ **Noch nicht am echten Fenster gesehen.** Die Vorschau hier zeichnet
Hintergrundfilter nicht; Rand, Dicke und Glanz sind darin geprüft,
Unschärfe und Sättigung nur im Fenster selbst.

⛔️ **Fund 459: ein alter Bauzwischenstand hielt den Bau auf.** Die
Bauskripte der Fensterbibliothek hatten ihre Ausgaben mit dem Pfad vor
dem Umzug des gemeinsamen Bauverzeichnisses nach `SYSTEM/full-build`
abgelegt, und der Bau suchte dort. Behoben durch Löschen der 456
Zwischenstände, die den alten Pfad nannten; danach baute es durch. 📌
**Ein Umzug des Bauverzeichnisses ist erst vollständig, wenn die
Zwischenstände darin neu entstanden sind.** Aus einem frischen Klon
tritt es nicht auf.

⚠️ **Nachtrag aus v0.81.0:** Beim Sprung auf 0.44.0 blieb
`tauri.conf.json` auf 0.43.0 stehen, und die Zählung unten stammt von
einem Lauf **vor** dem Sprung; `die_buendelversion_ist_die_kistenversion`
hätte danach rot gemeldet. Nachgezogen in v0.81.0.

**Belegt:** 66 Proben in `tests/oberflaeche.rs`, alle grün, zwei neu:
`das_ladezeichen_ist_ein_synaptischer_spalt` (eigene Kennungen je
Zeichen, kein Stil, eine Regel) und `glas_hat_dicke_und_spiegelung`
(`.glas`, `.eingabefeld` und Knöpfe tragen beides); sieben Gegenproben
beissen.

### v0.79.0 – 2026-09-24 (das Kästchen wieder eckig, in einem Ton)

`myl-console` **0.21.0 auf 0.22.0**. Rückmeldung des Projektinhabers
mit einem Foto seines Terminals: Das Verblassen funktioniert nicht, der
Kontrast ist gut, das Kästchen soll wieder eckig um die ganze Eingabe
stehen.

📌 **Warum der Verlauf nicht trug:** Das Terminal zeigt ein
Hintergrundbild. Jede Zelle mit eigener Hintergrundfarbe liegt dort als
deckendes Band über dem Bild, und der Verlauf zerfiel in sichtbare
Stufen; die schwächeren Polsterzeilen fielen dazu früher unter die
Sichtschwelle und waren schmaler als die Textzeile. **Ein Verlauf über
Zellfarben setzt einen einfarbigen Hintergrund voraus.** Die Vorschau
hatte das nicht zeigen können, sie rechnete mit einem einfarbigen.

⚑ **Jetzt ein Rechteck in einem Ton** über die ganze Breite des
Kästchens, Polster wie Text, in der Stärke, die der Projektinhaber gut
fand: die Mischung aus Terminalhintergrund und Ton der Rolle mit
`Rollen::deckung`. Mittig bleibt es, Zeitleiste und Antwort bleiben
links. Entfallen: `verlauf` und die schwächeren Polsterzeilen.

**Belegt:** 147 Proben im Programm und 7 in `tests/konsole.rs`, alle
grün, Clippy ohne Befund, drei Gegenproben beissen
(`das_kaestchen_ist_ein_rechteck_in_einem_ton`: jede Zeile genau ein
Grund, derselbe, gleich breit).

### v0.78.0 – 2026-09-24 (die Eingabe mittig, auf einem Grund, der in die Farbe des Terminals ausläuft)

`myl-console` **0.20.0 auf 0.21.0**. Rückmeldung des Projektinhabers:
Das Kästchen war „noch viel zu intensiv", es soll von innen nach aussen
verblassen, die Eingabe soll mittig stehen, Handlungen und Antwort
links bleiben.

⚑ **Der Grund verblasst quadratisch von der Mitte zum Rand**
(`antwort::verlauf`), die Polsterzeilen auf 45 Prozent davon, und am
Rand wird gar kein Grund mehr gesetzt. Die Deckung in der Mitte ist
klein: 130 Promille zum Ton der Rolle hin bei Standard und Myelith,
110 bei Bernstein, 120 bei Tiefsee, 100 bei Tinte.

⚑ **Gemischt wird über den Hintergrund des Terminals, nicht über
Schwarz.** Die Konsole fragt ihn beim Start ab (OSC 11, mit einer
Frist von 300 ms, vor der ersten Tastenabfrage, damit die Antwort
nicht als Tastendruck in der Eingabe landet). Antwortet ein Terminal
nicht, gilt Schwarz. 📌 **Warum das nötig war:** Die erste Fassung liess
den Grund nach Schwarz auslaufen, und in der Vorschau aus der echten
Ausgabe stand auf einem nicht ganz schwarzen Hintergrund ein dunkles
Rechteck mit Kante, also genau das, was weg sollte. Auf einem hellen
Terminal wirkt die Mischung jetzt richtig herum, als leichte
Abdunklung.

⚑ **Mittig:** Das Kästchen steht bündig mit dem Eingaberahmen darunter,
jede Textzeile mittig darin; Zeitleiste und Antwort bleiben am linken
Rand.

📌 **Eine Gegenprobe schwieg, und die Testwerte waren schuld:** Die
Umrechnung der Terminalantwort mit falschem Teiler fiel nicht auf, weil
bei `1e1e`, `8080` und `ffff` das niedrige Byte zufällig den richtigen
Wert trifft. Jetzt stehen Werte ohne wiederholte Ziffernpaare dabei.

**Belegt:** 147 Proben im Programm und 7 in `tests/konsole.rs`, alle
grün, Clippy ohne Befund, fünf neue Gegenproben beissen. Vorschau aus
der echten Ausgabe auf dunklem und grauem Hintergrund angesehen.
⚠️ Die Abfrage selbst (OSC 11) ist nur unter Unix gebaut und nicht als
Probe gebunden, denn sie braucht ein antwortendes Terminal; ihr Leser
ist gebunden.

### v0.77.0 – 2026-09-24 (die Eingabe im Kästchen, und Luft zwischen Eingabe, Handlungen und Antwort)

`myl-console` **0.19.0 auf 0.20.0**. Zwei Wünsche des Projektinhabers:
Die abgeschickte Eingabe soll auf einem dezent getönten Kästchen stehen,
und zwischen Eingabe, Handlungen des Modells und seinem Text sollen
Absätze sein.

⚑ **Das Kästchen** (`antwort::eingabe_kasten`): fester Grund in der
Breite des Eingaberahmens, oben und unten eine Polsterzeile, die Marke
`❯` vor der ersten Zeile. Der Grund kommt aus der neuen Rolle
`eingabe` (`Stil` trägt dafür einen optionalen Hintergrund): ein
Palettengrau bei Standard, Myelith und Tinte, dunkles Bernstein, dunkles
Blau bei Tiefsee. Ohne Farbe bleibt es bei `❯ text`.

📌 **Umbrochen wird an Wortgrenzen**, nur ein Wort, das allein zu lang
ist, wird geteilt. Die erste Fassung schnitt nach Zeichenzahl und zerriss
Wörter mitten durch; gefunden von der Probe, die den Text aus dem
Kästchen wieder zusammensetzt.

⚑ **Absätze:** eine Leerzeile nach dem Kästchen, eine vor der Antwort,
zwei nach der Bilanz bis zur nächsten Eingabe. **Und in der Zeitleiste
eine Leerzeile bei jedem Wechsel** zwischen Handlungen (`→`, `←`,
Schritte) und dem Text des Modells im Rohstrom, innerhalb einer Art
keine, sonst zerfiele eine Folge von Aufrufen in Einzelstücke
(`anzeige::absatz_noetig`).

⚑ **Die markierten Stellen der Antwort in einem ruhigen, hellen Ton**
(Wunsch des Projektinhabers): Code, Überschriften und Listenmarken
stehen je Design in einem schlichten Ton, der sich vom dunklen Grund
absetzt, statt im gewürfelten Neon: blasses Cyangrau bei Myelith,
helles Bernstein, Blassblau bei Tiefsee, helles Grau bei Tinte; die
Überschriften fett im hellsten Ton; Standard nimmt Cyan statt Magenta.

⛔️ **Befund dabei:** Im Design Myelith erbte die Überschrift den
gewürfelten Logoton, und Purpur (129) oder Violett (165) haben eine
wahrgenommene Helle um 55 von 255; auf schwarzem Grund kaum lesbar.
Dasselbe konnte Aufrufe, Rückgaben und die Ausweich-Warnfarbe Rot (196)
treffen. Jetzt hebt `design::lesbar` die fünf dunklen Neontöne auf
ihren helleren Nachbarn, und die Probe
`hervorgehobenes_setzt_sich_vom_grund_ab` rechnet die Helle nach: für
die Antwort ab 150, für Aufruf, Rückgabe und Warnung ab 120, in jeder
der 18 möglichen Sitzungen und jedem Design mit festen Werten.

**Belegt:** 145 Proben im Programm und 7 in `tests/konsole.rs`, alle
grün, Clippy ohne Befund, neun neue Gegenproben beissen.

### v0.76.0 – 2026-09-24 (die Konsole setzt ihre Ausgabe: Handlungen hervorgehoben, Antworten gesetzt, und mehr Kontrast am Logo)

`myl-console` **0.18.0 auf 0.19.0**. Zwei Aufträge des Projektinhabers:
den Kontrast zwischen Logo und Muster schärfen, und die bis hierher
einfarbige Textausgabe gliedern, **vor allem die Handlungen des
Modells**. Was und wie hervorgehoben wird, war freigestellt.

#### Kontrast am Logo, am Bild verglichen

⚑ **Drei Mittel zusammen**, in vier Stufen nebeneinander gerendert und
danach gewählt:

- **Das Muster ist dunkler** (48 statt 65 Prozent Helle).
- **Das Muster ist entsättigt** (60 Prozent Sättigung): Die volle
  Sättigung gehört allein dem Logo, und das trennt die Ebenen stärker
  als die Helle allein.
- **Die Schattenkanten des Schriftzugs treten zurück** (`╗ ╔ ╝ ╚ ═ ║`
  auf 55 Prozent, die Blöcke `█` voll und fett): In dieser Schrift sind
  die Kanten ohnehin der Schatten, und so stehen die Blöcke plastisch
  davor.

#### Rollen der Textausgabe (`design::rollen`)

| Rolle | Wo |
|---|---|
| Aktion | `→ werkzeug` in der Zeitleiste, fett |
| Ergebnis | `← werkzeug`, der Inhalt der Rückgabe als Beiwerk |
| Warnung | `⚑ … abgelehnt:`, „Kontext voll", „keine Schlussantwort" |
| Gedanke | das Denken im Rohstrom (`^S`), kursiv |
| Beiwerk | Schrittmarken, Zeiten, Bilanz, Hinweise |
| Code, Überschrift, Marke, Kante | in der gesetzten Antwort |

⚑ **Je Design eigene Töne**: Standard nimmt die **benannten
ANSI-Farben**, also das Schema des Terminals; Myelith die Neonfarben
der Sitzung; Tiefsee Cyan, Türkis und Hellblau mit einem warmen Ton für
Warnungen; **Bernstein und Tinte bleiben einfarbig** und unterscheiden
über Helle, fett, kursiv und unterstrichen.

⚑ **Farbe unterstreicht, sie trägt nichts allein**: Aufruf, Rückgabe
und Ablehnung stehen weiter mit `→`, `←` und `⚑` samt Wort da. Und
`NO_COLOR` schaltet alles ab, wie es die Absprache unter Programmen
will; ohne Terminal steht der Text ohnehin ungefärbt.

⚑ **Die Zeitleiste trägt ihre Art an der Zeile** (`anzeige::Art`) und
wird beim Schreiben gesetzt (`zeile_setzen`); der Rohstrom führt Denken
und Antwort als getrennte Stücke, damit das Denken anders aussehen kann.

#### Die Antwort, gesetzt (`antwort.rs`)

Das Modell antwortet in Markdown, und bis hierher stand die Antwort roh
da. Jetzt: Überschriften ohne Rauten im Stil der Überschrift, `code`
ohne Backticks im Stil für Code, Codeblöcke mit Balken und Sprache,
Aufzählungen mit `•` und hervorgehobener Marke, `**fett**` fett,
Zitate mit Balken, `---` als Linie. ⚠️ **Einfache Sterne und
Unterstriche werden bewusst nicht kursiv**: In Pfaden und Bezeichnern
stehen sie ohne Absicht, und ein Setzer, der dort Kursiv beginnt,
verschluckt Zeichen. **Ein Zeichen ohne Partner bleibt stehen.**

#### Befunde

⛔️ **Im Design Myelith konnten Aufruf und Warnung gleich aussehen.** Die
Schlagwortfarben sind gewürfelt und können das Orange treffen, in dem
gewarnt wird. Gefunden von der Probe, die zufällig in genau so einer
Sitzung lief; jetzt weicht die Warnfarbe aus, und die Probe prüft
**jede** mögliche Sitzung statt der gewürfelten.

📌 **Eine Namensgleichheit hat eine Architekturprobe ausgelöst**:
`die_logik_kommt_aus_der_kiste` sucht nach `.setzen(`, damit die Konsole
keine Einstellungen selbst setzt, und traf die neue Methode
`Stil::setzen`. Die Methode heisst jetzt `faerben`; die Probe blieb, wie
sie ist.

**Belegt:** 141 Proben im Programm und 7 in `tests/konsole.rs`, alle
grün, Clippy ohne Befund. **Zwölf neue Gegenproben, alle beissen.**
⚠️ Nicht als Probe gebunden sind `NO_COLOR` (hängt an der Umgebung) und
die Stile des Rohstroms (sie laufen im Anzeigefaden); beide sind klein
und im Code benannt.

### v0.75.0 – 2026-09-24 (heilige Geometrie um das Logo: acht Motive, eines je Start, und ein Muster, das gegen das Logo fliesst)

`myl-console` **0.17.0 auf 0.18.0**. Auftrag des Projektinhabers: Das
Motiv aus Neuronen und Gestein (v0.74.0) wird verworfen, an seine
Stelle tritt **rein geometrisches Muster nach dem Vorbild der heiligen
Geometrie**, das das Logo umrandet und es **nirgends berührt**.

⚑ **Acht Motive, ausgewählt vom Projektinhaber aus vierzehn Entwürfen**,
und bei jedem Start eines davon gewürfelt (neu: `geometrie.rs`):
Blume des Lebens, Quadratwirbel, Sterntetraeder-Gitter, Hexagonwirbel,
Dreieckwirbel, Saatgitter, Merkaba-Feld (Kagome-Gitter aus
Hexagrammen) und Goldene Spirale (je Vierteldrehung um den Goldenen
Schnitt wachsend). **Das Motiv gilt für die ganze Sitzung**, damit jeder
Neudruck dasselbe Bild zeigt; `MYL_MOTIV=<name>` legt es fest.

⚑ **Geschnitten wird nur an zwei Kanten**: am Bildrand und an einer
Sperrzone von drei Spalten und einer Zeile um den Schriftzug. Sonst
laufen die Muster frei, auch über und unter dem Logo durch die Mitte,
wo sich die Figuren beider Seiten treffen (Festlegung des
Projektinhabers).

⚑ **Braille-Zeichen als Punktfläche.** Kreise und Linien in 60 Grad
lassen sich mit Kastenzeichen nur andeuten; ein Braille-Zeichen trägt
2 × 4 Punkte, die annähernd quadratisch sind, also bleibt ein Kreis ein
Kreis. **Ganzzahlig gerechnet**: Linien nach Bresenham, Kreise nach dem
Mittelpunktverfahren, Winkel über eine Sinustafel in ganzen Grad,
Rundung für beide Vorzeichen gleich, damit links und rechts
Spiegelbilder sind.

⚑ **Das Muster fliesst gegen das Logo** (Wunsch des Projektinhabers):
Während eines Auftrags wandert der Verlauf im Logo nach links, im
Muster nach rechts, beides eine Spalte je Welle des Ladetextes. Das
Muster steht dabei auf 65 Prozent der Helle; Unterschied in Helle und
Richtung liest das Auge als Tiefe. **Ohne Auftrag steht beides still**,
und bei stehender Uhr ist der Verlauf über beide Ebenen ungebrochen.

⛔️ **Welche Ebene ein Zeichen ist, entscheidet jetzt das Zeichen und
nicht die Zeile.** Bis hierher hiess „im Schriftzug" dasselbe wie „in
einer Zeile mit `█`" (`ist_schriftzug`). Mit Muster neben dem
Schriftzug hätten dessen Zeichen in denselben Zeilen Farbe, Stärke und
Laufrichtung des Logos getragen. Jetzt gehören genau die Zeichen
`█ ╗ ╔ ╝ ╚ ═ ║` zum Logo (`banner::ist_logozeichen`), und
`ist_schriftzug` samt dem Feld `im_schriftzug` in Animation, Neudruck
und Schimmer ist entfallen.

⚑ **Die Zeilenzahl ist die alte** (18, 13 und 7 je nach Fensterhöhe),
also bleibt der Platz für das Menü. 📌 Dabei eine Falle: Endete die
Fläche in den niedrigen Höhen mit einer Musterzeile, hinge die
Zeilenzahl am Motiv, denn `lines()` zählt eine leere letzte Zeile nicht
mit. Jetzt endet sie an der Sperrzone, und die Leerzeile danach ist
echt.

⚑ **Entworfen im Bild, gebaut gegen eine Referenz.** Die Motive
entstanden in mehreren Runden als ganzzahlige Skizze mit Vorschaubild;
die Rust-Fassung zeichnet in 72 Fällen (acht Motive, neun Grössen)
Zeile für Zeile dasselbe.

Entfallen mit dem alten Motiv: Neuronen, Gestein und ihre vier
Farbstufen; jetzt zwei (Logo voll, Muster gedämpft).

**Belegt:** 131 Proben im Programm und 7 in `tests/konsole.rs`, alle
grün, Clippy ohne Befund. Neu unter anderem
`nichts_beruehrt_das_logo` (acht Motive, fünf Breiten, drei Höhen),
`jedes_motiv_umrandet_das_logo`, `das_bild_hat_die_alte_zeilenzahl`,
`das_muster_fliesst_gegen_das_logo`, `die_richtung_haengt_am_zeichen`,
`ein_kreis_bleibt_rund`, `gerundet_wird_symmetrisch`. **17 Gegenproben
beissen.** Eine schwieg zuerst: Ohne das `+ 1` im Mittelpunktverfahren
verschiebt sich nur die Rundung, und der Kreis bleibt innerhalb eines
Punktes rund, also genau das, was die Probe zusichert; die Mutation,
die ihn wirklich bricht, beisst.

⚠️ **Wie Fliessen und Gegenlauf im Terminal wirken**, zeigt erst ein
Start von `myelith`.

### v0.74.0 – 2026-09-24 (Myelin oben, Lith unten: ein neues Motiv um das Logo der Konsole)

`myl-console` **0.16.0 auf 0.17.0**. Auftrag des Projektinhabers: Das
Umfeld des Logos soll expressionistisch werden und zu Myelith passen,
Neuronen, Gestein, Geometrie. Bis hierher stand dort ein Netz aus
Knoten und dünnen Kanten nach dem Projektbanner; es las sich als
Schema und sagte über das Projekt nichts.

⚑ **Das Bild trägt die beiden Hälften des Namens.**

- **Oben das Myelin.** Neuronen mit einem Fächer aus Dendriten,
  abwechselnd tief und hoch, zwischen drei und sieben je nach Breite.
  Ihre Axone tragen **Markscheiden ungleicher Länge** mit Schnürringen
  dazwischen (`━━━━━━·━━━━`), wechseln auf halbem Weg schräg die Ebene
  und münden in eine Synapse (`╾●`); das erste kommt von links herein,
  das letzte endet in einem Endknopf am rechten Rand (`┫●`). Ein paar
  Funken (`· ∘ ◇`) stehen in den leeren Winkeln.
- **Unten das Lith.** Gestein mit Kristallspitzen, deren Hänge
  **ungleich steil** sind (`◢ ◣ ▲`): Eine Spitze mit gleichen Hängen
  ist eine Pyramide, eine mit ungleichen ein Splitter. Die Schichten
  (`░ ▒ ▓`) laufen schräg wie gekipptes Sediment, darin Edelsteine
  (`◆`).
- **Dazwischen der Schriftzug**, getragen vom Stein und überspannt vom
  Nerv. Höhe und Aufteilung bleiben (fünf Zeilen oben, vier unten),
  also auch das Kürzen in niedrigen Fenstern.

⚑ **Vier Stufen der Farbe statt zwei** (`banner::stufe`): Schriftzug,
Zellkörper, Schnürringe, Funken und Edelsteine leuchten im vollen
Verlauf; Markscheiden, Synapsen und Kristallkanten stehen im Verlauf
auf 65 Prozent; das Gestein auf 38 Prozent, wie farbiger Stein; nur die
feinen Dendriten bleiben grau. **Die Stufen dämpfen die Farbe und
wechseln sie nicht**: Das ganze Bild liegt im selben Regenbogen, und
beim Fliessen wird alles neu gemalt, was Farbe trägt
(`schimmer::farbig` statt `leuchtet`).

⚑ **Das Ersatzbild ist erzeugt, nicht abgeschrieben.** Für zu schmale
Fenster und für Ausgaben ohne Terminal stand ein fester Text, der das
Motiv von Hand nachbildete. Mit einem neuen Motiv wären es zwei Bilder
gewesen, die von Hand gleich zu halten sind; jetzt ist es dasselbe
Motiv in seiner schmalsten Form (`banner::ersatzbild`, 58 Spalten).

⚠️ **`▓` ist die dunkelste Schicht, nicht `█`.** Der volle Block gehört
dem Schriftzug, und an ihm erkennt `ist_schriftzug` dessen Zeilen; ein
Gestein aus `█` hätte das halbe Bild in die falsche Stufe gesetzt. Eine
Probe hält das fest, und ihre Gegenprobe (`█` als Schicht) beisst.

📌 **Eine neue Probe prüfte zuerst ihren eigenen Zuschnitt.**
`die_markscheiden_sind_ungleich_lang` zählte anfangs jeden Lauf von
`━`, und die Gegenprobe (alle Scheiden gleich lang) blieb grün: Die an
Neuron und Synapse abgeschnittenen Enden sind immer verschieden lang.
Jetzt zählt sie nur Scheiden, die in einem Schnürring enden, denn die
sind immer vollständig.

**Entworfen in fünf Runden als Skizze und erst danach übertragen**; die
Rust-Fassung zeichnet in 80 und 120 Spalten Zeile für Zeile dasselbe
Bild wie die letzte Skizze.

**Belegt:** 125 Proben im Programm und 7 in `tests/konsole.rs`, alle
grün, Clippy ohne Befund. Neu: `das_motiv_traegt_nerv_und_stein` (fünf
Breiten), `die_markscheiden_sind_ungleich_lang`,
`die_stufen_daempfen_und_das_grau_bleibt_grau`,
`kein_zeichen_faellt_unbemerkt_ins_grau` (ein neues Zeichen ohne Stufe
fiele sonst still ins Grau), `schmales_fenster_faellt_auf_das_ersatzbild_zurueck`,
`nur_was_farbe_traegt_wird_neu_gemalt`. **Neun neue Gegenproben und die
zehn der vorigen Fassung, alle beissen.**

⚠️ **Wie das Bild im Terminal wirkt**, mit Farbe und Fliessen, zeigt
erst ein Start von `myelith`.

### v0.73.0 – 2026-09-24 (das Logo der Konsole im Regenbogen, fliessend während eines Auftrags)

`myl-console` **0.15.0 auf 0.16.0**. Auftrag des Projektinhabers: Das
Logo bekommt denselben Regenbogen wie der Ladetext („warping bytes"),
fliesst, solange ein Auftrag läuft, und steht sonst still; schon das
Startbild baut es bunt aus der Spirale, und es ist **nie einfarbig**.

⚑ **Drei Zusagen, eine Stelle.** Neu ist `schimmer.rs`, und dort steht
alles, was das Logo färbt:

- **Immer ein Verlauf.** Über die 56 Spalten des Schriftzugs läuft ein
  Drittel des Farbkreises (Festlegung des Projektinhabers: die dezente
  Spanne), dazu eine Spalte Versatz je Zeile, also ein leichter
  Schrägverlauf. Das Netz über die volle Fensterbreite trägt
  entsprechend mehr. Die Farbe selbst kommt aus derselben Funktion wie
  die des Ladetextes (`design::ladefarbe`): In den bunten Designs ist es
  der Regenbogen, in Bernstein und Tinte ein Verlauf der Helle.
- **Fliessen nur während eines Auftrags.** Eine eigene Uhr läuft ab dem
  Start der Anzeige und steht bei ihrem Ende. Der Verlauf wandert
  **eine Spalte je Welle des Ladetextes** nach links, also genauso
  schnell. ⚑ **Angehalten wird die Uhr, nicht das Bild zurückgesetzt:**
  Der nächste Auftrag setzt dort fort, wo der vorige aufhörte, und
  jeder Neudruck dazwischen zeigt genau das stehende Bild.
- **Der Aufbau endet im selben Bild.** Startbild, Hochgleiten, jeder
  Neudruck und das Fliessen fragen alle `schimmer::zellstil` nach
  derselben Uhr. Jedes Artefakt fliegt schon in der Farbe seines Ziels
  durch die Spirale, statt in einer gewürfelten Neonfarbe zu fliegen
  und am Ende die eine Farbe des Schriftzugs anzunehmen. **Der
  Regenbogen wird aufgebaut, nicht nachträglich aufgetragen**, und weil
  die Uhr während des Aufbaus läuft, fliesst er dabei schon.

⚑ **Jede Sitzung sieht anders aus.** Die Mitte des Logos trägt beim
Start den gewürfelten Grundton der Sitzung (`farben::grundton`); die
Schlagwortfarben der Menüs liegen um ihn herum und finden sich deshalb
im Logo wieder.

⛔️ **Gemalt wird nur, wo das Logo sicher steht.** Es rollt mit dem
Gespräch weg, und ein Terminal sagt nicht, wie weit. Wer an die alte
Stelle malt, malt Buchstaben des Logos in die Antwort. Die sichere
Auskunft gibt der Wagen: Im Rollbereich wandert er nur nach unten, und
gerollt wird erst, wenn er die letzte Zeile erreicht. **Vor jedem
Malen werden Wagen und Fenster nachgemessen**; steht der Wagen auf der
letzten Zeile des Rollbereichs oder hat sich die Fenstergrösse
geändert, wird die Lage verworfen und erst mit dem nächsten Neudruck
neu gesetzt. Verworfen wird ausserdem am Eingang jeder Auswahlliste,
der Einstellungsseite und beim Leeren des Schirms, denn diese Wege
ziehen den Wagen nach oben. Antwortet ein Terminal nicht auf die Frage
nach dem Wagen, wird ebenfalls verworfen, statt jeden Takt zu warten.

⚑ **Deshalb beginnt das Gespräch jetzt direkt unter dem Logo**
(Festlegung des Projektinhabers). Bisher sprang der Wagen nach dem
Einrichten auf die letzte Zeile des Rollbereichs; von dort schob jede
Zeile das Logo weiter, und keine Animation hätte je sicher malen
können. Jetzt füllt das Gespräch erst die freie Fläche, und das Logo
fliesst, bis der Schirm voll ist. Danach rollt es wie bisher weg und
bleibt in seinen letzten Farben stehen. Lässt sich die Stelle nicht
messen, oder würden die Leerzeilen des Rahmens den Schirm rollen, geht
es wie bisher unten weiter.

⚑ **`animation.rs`, `banner.rs` und `farben.rs` gehören jetzt der
Konsole** (Festlegung des Projektinhabers). Bis hierher waren sie
wortgetreue Kopien aus dem Testclient; der behält seine Fassung und
wird ohnehin abgeräumt. Die Probe `die_kopien_sind_wortgetreu` wacht
nur noch über `auswahl.rs`, und `die_marke_ist_eine_gekennzeichnete_kopie`
prüft jetzt auch die Gegenrichtung: Keine der drei gibt sich noch als
Kopie aus. Mit der Kopie ging der Grund für `allow(dead_code)`, also
gingen auch die Wege, die nur der Testclient rief (`print_if`,
`bildschirm`, `start_if`, die Begrüssung) und die Logofarbe
`farben::logo`.

⚑ **Nebenbei gerade gezogen:** Die Einstellungsseite übernahm ein neu
gewähltes Design erst **nach** dem Neudruck des Logos, der also noch im
alten stand. Jetzt wird erst übernommen und dann gedruckt. Und das
Design wird vor dem Startbild gelesen, damit das Logo schon im
eingestellten Bild entsteht.

📌 **Eine Gegenprobe hat eine Prüfung als Zierde entlarvt.** Die erste
Fassung von `schirm::fortsetzungszeile` prüfte getrennt, ob die
Leerzeilen den Schirm rollen und ob die Stelle unter dem Rollbereich
liegt. Weil der Rollbereich genau `RESERVE` Zeilen vor dem
Fensterende aufhört, ist das dieselbe Bedingung: Jede der beiden
Zeilen liess sich einzeln entfernen, ohne dass eine Probe anschlug.
Jetzt steht eine da, und daneben, warum sie beide Fälle deckt.

**Belegt:** 125 Proben im Programm und 7 in `tests/konsole.rs`, alle
grün, `cargo clippy --all-targets -- -D warnings` ohne Befund. Neu
sind sieben Proben in `schimmer.rs` (Spanne, nie einfarbig,
Fliessgeschwindigkeit, Grundton in der Mitte, Uhr hält und setzt fort,
kein Malen auf der letzten Zeile, nur Leuchtendes wird neu gemalt),
eine Quelltextprobe, dass jeder Weg nach oben die Lage verwirft, und
eine in `schirm.rs` für die Fortsetzungszeile. **Elf Gegenproben, je
Zeile einzeln, alle beissen.**

⚠️ **Nicht geprüft ist das Bild selbst.** Hier läuft kein Terminal,
in das jemand hineinsieht; wie der Verlauf wirkt, ob das Fliessen ruhig
genug ist und ob das Startbild so aussieht wie gedacht, zeigt erst ein
Start von `myelith`.

### v0.72.2 – 2026-09-24 (⛔️ Fund 458: ein Pfad hinter `cfg`, den die eigene Maschine nicht sieht)

**Gemeldet vom Windows-Läufer der CI:**

```
INSTALL/installieren-windows.ps1 steht in `skript()` und liegt nicht da
```

Beim Umzug nach `SYSTEM/` blieb genau dieser eine Pfad stehen, und
**zwei Dinge haben ihn versteckt**: Er stand mit **Backslash**
(`INSTALL\installieren-windows.ps1`), also traf ihn eine Ersetzung von
`"INSTALL/` nicht, und er lag hinter `cfg(target_os = "windows")`, also
übersetzte ihn ein macOS-Lauf gar nicht erst. ⚠️ **215 grüne Proben
örtlich, und der Fehler stand die ganze Zeit in der Datei.**

📌 **Ein `cfg`-Zweig ist Code, den die eigene Maschine nicht prüft.**
Dagegen hilft keine Sorgfalt, sondern nur, ihn aus dem `cfg`
herauszuholen.

⚑ **Neu: `SKRIPT_MACOS`, `SKRIPT_LINUX`, `SKRIPT_WINDOWS` und
`SKRIPTE`**, alle vier ausserhalb jedes `cfg`. `skript()` wählt nur
noch aus, und die Probe `drei_installationsskripte_liegen_in_install`
zieht ihre Liste aus `SKRIPTE`, statt sie danebenzustellen. **Genau
diese Doppelung hat den Fund möglich gemacht:** Die Liste in der Probe
wurde nachgezogen, der Pfad in `skript()` nicht, und beide sagten
dasselbe verschieden.

⚑ **Neu: `alle_drei_skripte_liegen_da`**, die an keinem `cfg` hängt.

**Gegengeprüft an einem bekannt schlechten Fall:** Mit dem alten Wert
wieder eingesetzt fällt sie auf macOS mit derselben Meldung, die der
Windows-Läufer gab. 📌 **Eine neue Probe, die nur grün war, beweist
nichts.**

⚑ **Der Backslash bleibt.** Ob PowerShell den Schrägstrich genauso
nimmt, ist hier nicht zu prüfen, und **eine Verhaltensänderung, die
niemand nachsehen kann, gehört nicht in eine Fehlerbehebung.**

**Belegt:** 216 Proben in `myl-client`, dazu acht weitere Bündel, alle
grün.


### v0.72.1 – 2026-09-24 (die Installationsskripte sind nach `SYSTEM/install/` gezogen)

**Nur Pfade, kein Verhalten.** Auf Festlegung des Projektinhabers ist
alles Systemnahe unter `SYSTEM/` gesammelt; für den Client heisst das,
dass `aktualisierung.rs`, `ort.rs` und `myl.rs` die Skripte jetzt unter
`SYSTEM/install/` suchen statt unter `INSTALL/`.

⚑ **Die drei Stellen sind keine Zierde.** `aktualisierung::skript()`
ruft das Skript beim Selbstaktualisieren wirklich auf, und `ort.rs`
liest es, um einen festen Pfad darin zu finden. Ein nicht nachgezogener
Name hätte nicht beim Bauen gefehlt, sondern **beim Nutzer**, und zwar
mit „Datei nicht gefunden" statt mit einem Hinweis.

⛔️ **Eine Probe hat den Umzug an einer Stelle zurückgedreht**, und sie
trug ihre Begründung selbst: `flake.nix` bleibt in der Wurzel, denn
`nix develop` sucht sie dort und nirgends sonst. 📌 **Genau dafür
stehen Begründungen im Prüfcode und nicht nur im Plan:** Sie halten den
auf, der es besser zu wissen glaubt.

**Belegt:** 215 Proben in `myl-client`, dazu 19, 6, 10, 12, 6, 6, 4, 6
in den übrigen Bündeln, alle grün. Der macOS-Installer im Prüfmodus aus
dem neuen Ort: `758 Pakete aus SYSTEM/crates-vorrat/, Netz aus`, dann
`Alles da`.


### v0.72.0 – 2026-09-23 (ein Terminal im Fenster, und es gehört dem Menschen)

**Auftrag des Projektinhabers:** ein Reiter mit Terminal, unter
„Wallet" in der linken Leiste.

⛔️ **Hier wird eine Grenze bewusst nicht gezogen, und das steht an
drei Stellen im Quelltext.** Überall sonst fasst die Einhängegrenze
ein, was laufen darf: `write_file` kommt nicht aus dem Arbeitsordner
heraus, ein Manifest braucht die Schreiberlaubnis, `run_command` gibt es
im Chat gar nicht. **Dieses Terminal hat keine dieser Schranken.** Es
führt aus, was dasteht, mit den Rechten des Nutzers.

⚑ **Der Unterschied ist, WER tippt.** Jede Schranke im Client schützt
vor einem **Modell**, das sich irrt oder das ein fremder Text in die
Irre führt. Ein Mensch, der ein Terminal öffnet, hat genau das gewollt,
und ihm dieselben Fesseln anzulegen hiesse, ihm ein Terminal zu geben,
das keines ist.

⛔️ **Also: Das Modell kommt hier nicht heran.** `terminal_ausfuehren`
steht in keinem Werkzeugkasten, wird von keiner Rüstung angeboten und
kann von keiner Agentenschleife gerufen werden. Wer ihn je als Werkzeug
anmeldet, macht aus dem Chat einen Vollzugriff.

⚑ **`cd` wird selbst behandelt und nicht an die Shell gegeben.** Jeder
Aufruf startet eine eigene Shell; ein `cd` darin wäre mit ihrem Ende
wieder weg. Der Ordner lebt deshalb im Fensterzustand und überlebt
jeden Befehl. Aufgelöst wird er mit `canonicalize`, sonst stünde nach
zweimal `cd ..` ein Pfad mit Punkten im Prompt.

⚑ **Kein Terminalemulator, sondern ein Befehlsläufer.** `TERM=dumb` und
`NO_COLOR=1`: Eine Ausgabe mit ANSI-Folgen sähe im Fenster aus wie
Kauderwelsch. Damit laufen `ls`, `git`, `grep` und `cargo`, aber nichts
Interaktives wie `vim` oder `top`. Das ist eine Zusage, die eingehalten
wird, statt einer, die halb eingelöst aussieht.

⚑ **Die Ausgabe geht über `textContent` und nie über `innerHTML`.** Sie
ist fremder Text; wer sie als Markup einsetzt, lässt jede Datei im
Dateisystem in das Fenster hineinschreiben. Frist 300 s (großzügig,
denn ein Mensch tippt auch `cargo build`), Ausgabe bei 40 000 Zeichen
gekürzt, Pfeiltasten blättern durch das Getippte.

📌 **Drei Proben haben sofort zugeschlagen, und alle drei zu Recht:**
`die_farben_sind_graustufen` fand ein `#e06c6c`,
`das_helle_thema_dreht_den_lichtkanal` eine feste Farbe statt eines
Kanals, und `jeder_befehl_ist_angemeldet` die Zahl im Kopf dieses
Dokuments.

⚠️ **Und eine Angabe war durch den Reiter falsch geworden:** Hier stand
„startet **keinen einzigen Unterprozess**". Ein Terminal, das keine
Shell startet, ist keines. Der Satz nennt die Ausnahme jetzt beim
Namen, statt dass sie ihn stillschweigend widerlegt.

⚑ **Und wozu das Ganze über das Fenster hinaus:** Ein System ohne
Desktop hat keinen Dateimanager, keinen Editor, keine Konsole. Bringt
das Fenster ein Terminal mit, genügt es dort **allein**. Das ist die
Voraussetzung für einen Kioskbetrieb.

**Belegt:** 64 Proben der Oberfläche grün, Clippy ohne Befund.

### v0.71.0 – 2026-09-23 (PDF, alle gängigen Formate, und eine Schranke, die die falsche Frage stellte)

**Zwei Aufträge des Projektinhabers**, und beim ersten war die
Entscheidung an mich delegiert.

#### Schranke 2 gelockert, und zwar in der Bauform statt in der Zahl

⚑ **Der Denkfehler war nicht die Zahl, sondern die Frage.** Eine
Zeichenlänge kann „diktiertes Kennwort" und „exakter Papiertitel" nicht
trennen, denn beide sind wörtliche Übernahmen; jede andere Zahl hätte
nur verschoben, wo die Schranke danebengreift. **Gemessen:** In drei von
fünf Läufen mit dem 4B traf sie eine echte Recherchefrage.

Jetzt gilt: Die Frage geht hinaus, die Treffer kommen zurück, **ihre
Adressen kommen aber nicht in den Zielkreis**. ⛔️ **Damit bleibt der
Angriff zu, den es wirklich gibt:** Eine Seite diktiert eine Frage mit
einem einmaligen Kennwort, damit der Suchdienst genau ihre zweite Seite
liefert. Sie wird geliefert und ist als Auszug lesbar, abrufbar ist sie
nicht, und mit eigenen Worten findet das Modell sie nie wieder, denn das
Kennwort ist der einzige Weg dorthin.

⚑ **Was die Lockerung trägt:** Über die Suchfrage kann nichts Privates
abfliessen, das fängt die Anhangprobe, und die bleibt ein Verbot. Der
gelesene Seitentext ist nie privat, denn Schranke 5 sperrt das eigene
Netz, und es gehen weder Kekse noch eine Anmeldung mit hinaus.

#### PDF und die übrigen Formate

⛔️ **Der gemeldete Inhaltstyp entscheidet, nicht die Endung.** Eine
Adresse sagt nichts darüber, was hinten herauskommt; der Kopf der
Antwort schon. Wo er schweigt oder lügt, sieht die Signatur nach:
Manche Server schicken ein PDF als `application/octet-stream` oder gar
als `text/html`.

* HTML und XHTML: Marken raus, Skript, Stil und Kopf ganz weg.
* XML, RSS, Atom: dasselbe.
* `text/*` und JSON: **wörtlich durch**. ⚑ Wer JSON durch die
  Markenentfernung schickt, bekommt zerstückeltes JSON, denn `<` und
  `>` kommen darin vor.
* PDF: ein eigenes Programm, siehe unten.
* Alles andere: eine Absage, die den Typ **nennt**.

Neu: `CLIENT/myl-senses/src/schrift.rs`. ⚑ **Es liegt bei den Sinnen und
nicht beim Recherchewerkzeug**, weil derselbe Handgriff auch für einen
Anhang gebraucht wird: Ein Mensch hängt ein PDF an und will, dass das
Modell es liest.

⚑ **Ein fremdes Programm und keine eigene Zerlegung.** Ein PDF trägt
seinen Text in Strömen, die fast immer gepackt sind, und die Zuordnung
von Zeichen zu Buchstaben hängt an eingebetteten Schriften, an
CID-Tabellen und an `ToUnicode`. Das ist keine Nachmittagsarbeit, und
**ein halb richtiger Auszug ist schlimmer als keiner: Er sieht aus wie
Text.**

Rangfolge: `MYL_SCHRIFTLESER`, `pdftotext`, `mutool`, zuletzt ein
Python, das `pypdf` oder `fitz` **wirklich** hat. ⚠️ **Das Nachsehen ist
der Punkt:** Ein Python ohne die Bibliothek wäre ein Zeug, das beim
ersten Auftrag versagt, und das ist schlimmer als keines. `myl sinne`
zeigt den gefundenen Weg in der Zeile `Schrift`.

Zwei Zahlen mussten mit: Die Holgrenze stand auf zwei Millionen Bytes
und hätte jedes PDF abgeschnitten (der geprüfte Artikel wiegt 8,7 MB),
die Frist auf 20 Sekunden. Jetzt 20 MB und 30 s. ⚑ **Was geholt wird
und was ins Fenster geht, sind zwei verschiedene Zahlen**; die zweite
bleibt bei 12 000 Zeichen.

⚑ **Und `ist_beiwerk` hängt jetzt von diesem Rechner ab.** Liegt ein
Programm zum Lesen von PDF, ist ein PDF ein Volltext und gehört in den
Zielkreis; liegt keines, ist es eine Zusage, die niemand einlösen kann.

**Belegt:** ein Lauf gegen das echte Netz, der den Artikel als PDF holt
und den Titel im Auszug findet, und ein Lauf mit dem 4B, der aus
`https://arxiv.org/pdf/2405.17849v2` in **einem** Schritt die drei
Verfahren der Arbeit nennt (FSBR, DI-MatMul, die drei ganzzahligen
Operatoren). Proben und Clippy grün in allen vier Client-Kisten.

### v0.70.0 – 2026-09-23 (drei Funde, alle vom Modell gefunden, keiner von einer Probe)

Die mehrstufige Recherche aus v0.69.0 war gebaut, geprüft und grün, und
sie funktionierte trotzdem nicht. **Gefunden hat das erst ein Lauf mit
dem 4B.** 📌 **Eine Probe prüft, ob die Maschine tut, was ich meinte.
Ob das Modell damit arbeiten kann, prüft nur das Modell.**

⛔️ **Fund 441: eine Erlaubnis ohne den Gegenstand.** Unter der Seite
stand „übernimm einen der Verweise wörtlich aus dem Text oben", und im
Text stand keiner: Verweise leben in `href`-Attributen, und die sind
nach dem Entfernen der Marken weg. Das Modell hat daraufhin dieselbe
Seite viermal gelesen und ist dann auf die Suche ausgewichen. Jetzt
stehen bis zu 20 Adressen am Ende des Blocks. ⚑ **Sie stehen IM
Rahmen**, denn sie stammen aus der Seite und sind fremder Inhalt; die
Erlaubnis, sie zu lesen, steht **ausserhalb**, denn die ist unsere.

⛔️ **Fund 442: vier Abrufe für denselben Text.** Eine wiederholte
Adresse bekommt jetzt eine Absage statt eines zweiten Abrufs, und der
Zähler steht dabei still. Ohne das frisst eine Schleife das Budget von
zwölf Abrufen an einer einzigen Seite auf.

⛔️ **Fund 443: nicht jedes PDF trägt `.pdf`.** Der Endungsfilter liess
`https://arxiv.org/pdf/2405.17849` durch, das Modell hielt die Adresse
für einen Volltext und verbrannte drei Schritte an „kein lesbarer
Text". Ein Pfadabschnitt, der genau `pdf` heisst, zählt jetzt ebenso.
⚑ **Was dieses Werkzeug nicht lesen kann, gehört nicht ins Angebot,
denn ein Angebot ist eine Zusage.** Wird doch eines angefragt, sagt die
Absage, **was** es ist; „kein lesbarer Text" lädt zum Wiederholen ein,
„das ist ein PDF" nicht.

Dazu fällt Beiwerk schon bei der Ernte heraus (Stilblätter, Skripte,
Bilder, Archive): Von fünfzehn gezeigten Adressen einer arxiv-Seite
waren sechs Stilblätter, und jede belegte einen Platz im Zielkreis.

**Belegt, mit dem 4B und dem vollen Weg:** `web_read` auf die
Artikelseite, dann `web_read` auf
`https://arxiv.org/search/cs?searchtype=author&query=Hu,+X`, **eine
Adresse, die nur aus der Ernte stammen kann** (kein Suchtreffer, vom
Nutzer nicht genannt), dann die Antwort mit zwei Titeln aus der
Trefferliste. 215 Proben der Kiste grün, Clippy ohne Befund.

### v0.69.0 – 2026-09-23 (mehrstufige Recherche, ohne dass der Zielkreis aufgeht)

**Auf Festlegung des Projektinhabers:** Der geschlossene Zielkreis aus
v0.68.0 machte mehrstufige Recherche unmöglich. Das Modell konnte einer
Seite nicht folgen, die auf die nächste verweist, und musste stattdessen
neu suchen. Jetzt wächst der Zielkreis auch aus **Verweisen einer schon
gelesenen Seite, aber nur auf demselben Wirt**.

⚑ **Die Dichtigkeit kommt aus der Wörtlichkeit, nicht aus dem Wirt**,
und das ist der Satz, auf den es ankommt. Ein geernteter Verweis steht
**wörtlich** so in der Seite, und ihr Verfasser kennt die Anhänge des
Nutzers nicht. Um ein Geheimnis anzuhängen, müsste das Modell die
Adresse **ändern**, und eine geänderte Adresse steht nicht mehr im
Zielkreis. 📌 **Das gehört ausgesprochen**, weil sonst der Trugschluss
naheliegt, derselbe Wirt sei eine Vertrauensgrenze: Auf einer Seite mit
fremden Beiträgen, einer Code-Ablage, einem Forum, einem Wiki, ist er
das gerade nicht.

⚑ **Wozu dann die Wirtsgrenze?** Nicht gegen Abfluss, sondern gegen
**Lenkung.** Ohne sie schickt eine präparierte Seite den Agenten auf
jeden anderen Wirt ihrer Wahl. Mit ihr bleibt er auf dem, den ein Mensch
oder ein Suchtreffer ohnehin benannt hat.

⚑ **Derselbe Wirt heisst: gleich, oder nur um ein führendes `www.`
verschieden.** Mehr nicht. `blog.example.org` ist nicht `example.org`,
denn eine Unterdomäne kann einem anderen gehören. Der nächste Schritt
wäre die registrierbare Domäne, und dafür braucht es die Liste der
öffentlichen Endungen, also eine Fremdkiste und eine Datei, die
veraltet.

Geerntet wird aus dem rohen HTML und nicht aus dem gekürzten Text (ein
Verweis steht im Attribut, und der Schnitt bei 12 000 Zeichen nähme ihn
mit), gegen das **Ziel der Umleitung** und nicht gegen die angefragte
Adresse, und höchstens 60 je Seite. Aufgelöst werden absolute,
schemalose, wurzelbezogene und einfache Verweise; `http:`, `mailto:`,
`javascript:` und ein blosser Sprung im Dokument fallen heraus.

⚠️ **Die Seite sagt dem Modell, dass es weiterlesen darf.** Unter dem
Text steht, wie viele Verweise jetzt lesbar sind. Eine Fähigkeit, die
niemand ansagt, wird nicht benutzt; stattdessen erfindet ein kleines
Modell eine Adresse und bekommt eine Absage.

📌 **Ein Fehler in der eigenen Probe gefunden:** `gleicher_wirt` schnitt
`www.` ab, **bevor** es kleinschrieb, also blieb `WWW.` stehen. In der
Anwendung hätte es nie gegriffen, weil beide Wirte aus derselben
kleinschreibenden Prüfung kommen; in einer öffentlichen Funktion ist es
trotzdem eine Falle.

**Belegt:** fünf neue Proben, darunter der geänderte Verweis, der kein
Verweis mehr ist, und die Wirtsgrenze gegen `boese.example` und
`blog.arxiv.org`. Dazu der Lauf gegen das echte Netz: 39 Verweise auf
`arxiv.org` geerntet, zweite Stufe gelesen. 21 Proben im Modul, 213 in
der Kiste, Clippy ohne Befund.

### v0.68.0 – 2026-09-23 (der Chat darf recherchieren, und fremder Text bekommt einen Rahmen, den er nicht aufmachen kann)

**Dritter Teil des Auftrags:** Werkzeuge für Web-Recherche, mit
maximalem Schutz gegen eingeschleuste Anweisungen.

⛔️ **Das Bedrohungsbild in einem Satz.** Angreifbar wird ein Agent, der
**eigene Daten**, **fremden Text** und **einen Weg nach draussen**
zugleich hat. Recherche bringt den fremden Text zwangsläufig mit, und
die Anhänge stehen im selben Fenster. **Also fällt die ganze Last auf
den Weg nach draussen**, und genau dort setzen die Schranken an.

⛔️ **Der Zielkreis ist geschlossen.** `web_lesen` nimmt nur eine
Adresse an, die vorher schon im Tor stand: entweder vom Nutzer selbst
geschrieben oder aus einem Suchtreffer. Eine Adresse, die das Modell
frei zusammensetzt, wird abgewiesen, **bevor** ein Abruf läuft.
📌 **Das ist der Kanal, an den zuerst niemand denkt:**
`https://fremd.example/?x=<Inhalt>` trägt alles hinaus, was im Fenster
steht, und sieht dabei aus wie ein gewöhnlicher Seitenaufruf.

⛔️ **Die Suchfrage geht frei hinaus, also wird sie geprüft.** Enthält
sie einen wörtlichen Lauf von 24 Zeichen aus einem Anhang, wird sie
abgelehnt. Verglichen wird über eine normalisierte Fassung, damit ein
eingefügter Zeilenumbruch die Probe nicht aushebelt. Ein zweiter,
längerer Lauf (48 Zeichen) fängt den Fall ab, dass eine gelesene Seite
dem Modell die nächste Suche diktiert.

⚑ **Zeitliche Trennung.** Das Tor füllt sich **vor** dem ersten
Fremdtext und wächst danach nur aus Suchen, die ihrerseits durch die
Verratsprobe gegangen sind. Eine gelesene Seite kann den Agenten damit
nicht auf ein Ziel ihrer Wahl schicken.

⛔️ **Der Rahmen lässt sich nicht von innen schliessen.** Jeder Abruf
kommt eingefasst zurück, mit der Ansage **vorn und hinten**, und die
beiden Rahmenzeichen werden im Inhalt selbst ersetzt. 📌 **Ohne das
zweite ist das erste wertlos:** Eine Seite, die den Rahmen schliesst
und einen eigenen aufmacht, spricht sonst mit der Stimme des Systems.
Die Ansage steht auch am Ende, weil eine Einschleusung sich dorthin
legt, wo die Regel schon weit weg ist.

⛔️ **Kein Zugriff auf das eigene Netz.** Nur `https`, kein Anmeldeteil
in der Adresse, kein anderer Port als 443, kein Namensliteral, das eine
Adresse ist, nichts auf `localhost`, `.local`, `.lan` oder `.internal`,
und auch kein reines Zahlenziel (`https://2130706433/` ist 127.0.0.1
und besteht jede Adressprüfung, weil es keine Adresse ist). Geprüft
wird zweimal: vor dem Abruf und noch einmal auf dem Ziel der Umleitung.

⚑ **Skript, Stil und Kopf fliegen ganz heraus**, nicht nur ihre Marken:
Was ein Leser nie sieht, ist der bequemste Platz für eine
Einschleusung.

⚑ **`curl` statt einer HTTP-Kiste**, aus demselben Grund wie im Agent
Layer: `reqwest` zöge `tokio`, `hyper` und `rustls` herein, und hier
wäre es ausgerechnet die Kiste, die den fremden Text anfasst. `curl`
liegt auf allen drei Zielsystemen, läuft in einem eigenen Prozess, und
fehlt es, fehlen die beiden Werkzeuge und sonst nichts. ⚠️ **`-q` steht
als erstes Argument**, sonst liest `curl` `~/.curlrc`, und dort könnte
ein Proxy oder ein Keksglas stehen, das dieses Modul nie gesehen hat;
`--` steht vor der Adresse, damit eine Adresse mit führendem Strich
kein Schalter wird.

⚠️ **Nur im Chat, nicht im vollen Agentenbetrieb.** Dort steht mit
`run_command` bereits ein Weg nach draussen offen, den keine dieser
Schranken einfasst; ein Suchwerkzeug daneben wäre die Schranke an einer
Tür, neben der keine Wand steht.

⚑ **Abgeschaltet, bis der Nutzer es einschaltet:** neues Häkchen
`agent.web_recherche` unter der Rubrik „Agent". Ist es gesetzt, geht
der Chat auch **ohne** Anhang durch die Werkzeugschleife, denn ein
Suchwerkzeug, das nur neben einer angehängten Datei da wäre, fehlte
genau dann, wenn man es braucht.

📌 **Fund 440: `<head` traf `<header`.** Der Kopfbereich wurde
herausgeschnitten, indem von `<head` bis `</head>` alles entfernt
wurde. Auf einer Seite mit `<header>` griff die Marke dort erneut,
fand danach kein `</head>` mehr und frass den Rest der Seite. Von
arxiv.org blieb „Skip to main content". ⚑ **Eine Marke endet am
Namen:** Hinter dem gesuchten Namen darf kein weiterer Namensbuchstabe
stehen.

⚑ **Neu: `myl agent --chat`.** Er fährt von der Kommandozeile genau
den Zuschnitt, den das Fenster im Gespräch fährt: Werkzeuge nur auf dem
Anhangordner, dazu die Recherche, falls sie an ist. 📌 **Ohne ihn liesse
sich der Chatzuschnitt nur durch das Fenster prüfen**, also nur von Hand
und nur auf einem Rechner mit Oberfläche.

**Mit dem 4B und einem echten Auftrag belegt:** `web_search` mit eigener
Frage, `web_read` auf eine Adresse aus dem Treffer, richtige Antwort aus
der Seite. ⚠️ **Nicht belegt ist, ob ein Modell einer eingeschleusten
Anweisung widersteht;** dafür müsste eine Seite unter eigener Kontrolle
über `https` erreichbar sein, und die Schranke gegen das eigene Netz
verbietet genau das. ⚑ **Der Entwurf baut auch nicht darauf:** Der
Rahmen ist die einzige Schicht, die vom Modell abhängt, und er ist die
letzte, nicht die erste.

Nebenbei: `myl einstellungen` schrieb `agent.blick_bildschirmfalse`, die
Namensspalte stand fest auf 21 Zeichen. Sie kommt jetzt aus der Feldliste
selbst.

**Belegt:** 16 Proben im Modul, darunter zehn gesperrte Ziele im
eigenen Netz, der Rahmen, der sich nicht von innen schliessen lässt,
die Verratsprobe über einen Zeilenumbruch hinweg, kurze Fragen, die
nie durchfallen, und ein Suchtreffer ins eigene Netz, der gar nicht
erst in den Zielkreis kommt. Dazu ein gesperrter Lauf gegen das echte
Netz (`cargo test --lib netzwerkzeuge -- --ignored`), der Suche und
Seite von Anfang bis Ende zeigt. 208 Proben der Kiste grün, clippy
ohne Befund.

### v0.67.0 – 2026-09-23 (der geänderte Anhang kommt zurück, und die Liste ist ein Befund)

**Zweiter Teil des Auftrags:** Ein im Chat geänderter Anhang soll beim
Menschen landen.

⚑ **Was sich geändert hat, wird gemessen und nicht geglaubt.** Vor und
nach dem Lauf wird der Anhangordner verglichen, Datei für Datei über
einen Abdruck des Inhalts. 📌 **Eine Antwort, die sagt „ich habe die
Datei geändert", ist eine Behauptung des Modells; diese Liste ist ein
Befund über das Dateisystem.** Ein Modell, das die Änderung nur
behauptet, taucht darin nicht auf.

⚑ **Der Abdruck und nicht die Änderungszeit.** Eine Zeit sagt
„angefasst", nicht „anders": Ein Werkzeug, das dieselben Bytes
zurückschreibt, setzte sie neu, und der Mensch bekäme eine Datei
angeboten, an der nichts geschehen ist.

⚠️ **Verglichen wird der ganze Ordner**, nicht nur die Anhänge dieses
Beitrags: Ein Modell, das statt zu ändern eine zweite Datei schreibt,
hat auch etwas hinterlassen, das der Mensch haben will.

⛔️ **Der Weg hinaus ist ein Knopf und kein Werkzeug.** Die Werkzeuge
des Chats kommen nicht aus dem Anhangordner heraus; **wohin eine Datei
geht, entscheidet der Mensch** über den Dialog des Systems. Der neue
Befehl `anhang_herausgeben` löst den Namen über dieselbe Einhängung auf
und weist ab, was hinausführt, auch wenn er aus dem eigenen Fenster
kommt: Ein Befehl, der jeden Pfad nimmt, den ihm jemand nennt, wäre eine
Tür neben der Tür.

Unter der Antwort steht je geänderter Datei ein Knopf. **Belegt:** eine
Probe, die `../geheim.txt`, `../../etc/hosts` und einen absoluten Pfad
abweist und den Anhang selbst durchlässt.

### v0.66.0 – 2026-09-23 (der Chat darf Anhänge bearbeiten, und sonst nichts)

**Auftrag des Projektinhabers:** Der Chat soll Anhänge bearbeiten
können, **aber keinen Zugriff auf Ordner oder Dateisystem**, und die
geänderte Datei soll zurückkommen.

⚑ **Der Mechanismus dafür war schon da: die Einhängegrenze.** Neu ist
nur, dass der Chat sie enger zieht. `ruestung::ruesten_fuer_anhaenge`
setzt ihre Wurzel auf den **Anhangordner**, und `aufloesen` weist ab,
was hinausführt. Damit stehen dem Chat neun Werkzeuge zur Verfügung:

| | |
|---|---|
| `read_file`, `write_file`, `edit_file`, `list_directory`, `search_files` | auf den Anhangordner begrenzt |
| `fill_template`, `join_sections` | rechnen aus ihren Eingaben, ohne Dateisystem |
| `describe_image`, `transcribe_audio` | dieselbe Grenze, auf Anhänge |

⛔️ **Manifest-Werkzeuge bleiben draussen, und das ist der Kern des
Zuschnitts.** Ein Manifest läuft über eine Shell, und eine Shell kennt
die Einhängegrenze nicht: `cat ../../../etc/passwd` ginge durch jede
noch so enge Einhängung hindurch. **Die Grenze hält nur, solange alles,
was hinter ihr arbeitet, sie kennt.** `run_command` fällt schon über die
Kiste weg, denn `Base` enthält es nicht.

⚑ **Werkzeuge gibt es genau dann, wenn ein Anhang da ist.** Ohne
Anhang bleibt der Chat, was er war: Eine Werkzeugansage kostet Kontext,
und für ein Gespräch ohne Datei gibt es nichts zu bedienen.

⚠️ **Im Chat heisst der Anhang anders**, und das ist keine Kosmetik: Die
Einhängung sitzt auf dem Anhangordner, also ist `liste.md` der richtige
Pfad und `.AGENT/anhaenge/liste.md` einer, der dort nicht auflöst. Die
Anhangzeile nennt deshalb je Betriebsart den passenden.

⚠️ **Vorgelesen wird mit Anhang nicht.** Im Strom der Schleife stehen
auch Werkzeugaufrufe, und die will niemand vorgelesen bekommen; genau
mit dieser Begründung war das Sprechen bisher dem Chat vorbehalten.

**Belegt:** zwei Proben, und sie halten die Zusage fest, auf die alles
ankommt. Die eine liest einen Anhang und weist `../geheim.txt`,
`../../etc/hosts` und einen absoluten Pfad ab; die andere prüft, dass
kein Manifest-Werkzeug und kein `run_command` im Kasten steht und die
fünf Dateiwerkzeuge da sind. Der Zuschnitt mutiert, die Probe fällt und
nennt, was sonst durchkäme. Dazu ein Lauf mit dem 4B in der
nachgestellten Chatlage:

```
→ edit_file aenderungen=[{"alt":"Kaese","neu":"Butter"}] pfad=liste.md
```

### v0.65.0 – 2026-09-23 (eine angehängte Textdatei versprach im Chat ein Werkzeug, das es dort nicht gibt)

⛔️ **Fund 439, gemeldet vom Projektinhaber.** Eine Textdatei anhängen
und „bitte ändere sie" sagen ergab eine **Anleitung** statt einer
Änderung.

**Der Grund ist der Betriebsmodus, und die Anhangzeile hat darüber
gelogen.** Sie schrieb unbedingt „Der Anfang steht unten; den Rest liest
`read_file`". Im **Chat** gibt es aber gar keine Werkzeuge, weder
`read_file` noch `edit_file`. Das Modell konnte die Datei also weder
lesen noch ändern, und die Nachricht hatte ihm das Gegenteil gesagt.

⚑ **Im Agentenmodus ging es die ganze Zeit.** Nachgestellt, mit dem 4B:

```
→ edit_file aenderungen=[{"alt":"Kaese","neu":"Butter"}] pfad=.AGENT/anhaenge/liste.md
```

Die Datei war danach geändert.

**Behoben:** `anhang_aufnehmen` bekommt den Betriebsmodus, wie `kontext`
ihn schon bekommt, und die Anhangzeile nennt **nur noch Werkzeuge, die
es im laufenden Betrieb wirklich gibt**. Ohne sie sagt sie das: „mehr
ist hier nicht zu holen, in diesem Betrieb gibt es keine
Dateiwerkzeuge". Das gilt für Text ebenso wie für Bild und Ton; ein
Werkzeugname im Chat ist ein Versprechen ohne Deckung.

📌 **Dieselbe Klasse wie Fund 436, nur andersherum.** Dort behauptete
die Zeile zu wenig (ein Bild war angesehen und sie sagte, es sei nichts
zu sagen), hier zu viel.

**Belegt:** eine Probe mit Gegenrichtung, der Zweig mutiert, die Probe
gefallen.

### v0.64.0 – 2026-09-23 (ein unbekannter Kistenname fällt nicht mehr auf die eingestellte Kiste zurück)

⛔️ **Fund 437.** `myl --werkzeuge <name>` warnte bei einem unbekannten
Namen und fuhr mit der **eingestellten** Kiste weiter. Wer den Schalter
setzt, sagt aber gerade, dass die eingestellte nicht gelten soll; er
bekam damit das Gegenteil dessen, wonach er gefragt hat.

**Gefunden von der Agentenmessung**, die `voll` übergab, weil `myl` nur
`Base`, `Advanced` und `1337` kennt. Über dem Ergebnis stand
„Werkzeugsatz: voll", gemessen wurde **Base**, und `run_command` war in
keinem einzigen Lauf im Angebot. Das Modell antwortete korrekt, es sehe
keine Funktion für Befehle, und das sah aus wie ein Modellfehler.

⚑ **Jetzt Rückgabewert 2 und kein Modell geladen.** 📌 **Eine Warnung,
nach der es weitergeht, ist ein Kommentar mit Laufzeit**, und hier
landete sie in einer Datei, die niemand las.

### v0.63.0 – 2026-09-23 (das Bild wurde angesehen, und die Nachricht sagte im selben Atemzug, es sei nichts zu sagen)

⛔️ **Fund 436, gemeldet vom Projektinhaber.** Im Fenster kam auf „Was
siehst du auf dem Bild?" die Antwort „Leider kann ich keine Bilder
sehen", **und danach eine vollständige Beschreibung des Bildes.** Beides
stimmte, und genau das war das Problem.

**Die Ursache ist eine Namensfalle.** Das Fenster schreibt die
Anhangzeile **sofort**, denn ein Sehmodell braucht Sekunden bis Minuten
und ein Fenster, das dabei einfriert, ist ein kaputtes Fenster. Für diese
Zeile nahm es `Sicht::Nichts` und meinte damit „keine Zeile, die zu
einem Werkzeugaufruf rät". **`Nichts` sagt aber mehr:** „es ist kein
Sinnesmodell eingerichtet, über ihren Inhalt ist nichts zu sagen". Die
Oberfläche hängte die Beschreibung danach an dieselbe Nachricht, und das
Modell las den ersten Satz zuerst.

⚑ **Behoben mit einer vierten Variante `Sicht::Kommt`**, die **nichts**
schreibt: Wer gleich die Beschreibung anhängt, braucht keinen Satz, und
jeder Satz wäre entweder doppelt oder falsch. 📌 **Ein Name, der weniger
behauptet als sein Text, ist eine Falle**; die neue Variante heisst nach
dem, was gilt.

⚠️ **Die Konsole war nicht betroffen.** Sie sieht vor dem Schreiben der
Zeile hin und nimmt dann `Sicht::Angesehen`; nur das Fenster musste die
Zeile vorziehen.

**Belegt:** eine neue Probe mit Gegenrichtung (ohne Sinnesmodell gehört
der verneinende Satz genau dorthin), der Zweig mutiert und die Probe
gefallen.

### v0.62.0 – 2026-09-23 (`--root` besorgt sich die Rechte jetzt selbst, und das System fragt)

**Auf Wunsch des Projektinhabers nachgereicht:** `myelith --root` startet
sich mit Verwalterrechten neu, ohne eine einzige neue Fremdkiste.

| System | Weg | Folge |
|---|---|---|
| Unix | `sudo -- env HOME=… MYELITH_ERHOEHT=1 <exe> --root`, danach `exec` | dasselbe Terminal, derselbe Vordergrundprozess |
| Windows | `powershell Start-Process -Verb RunAs` | eigenes Fenster, dieser Lauf endet |

⚑ **Die Zustimmung holt das System, nicht dieses Programm.** `sudo` fragt
nach dem Passwort, die Benutzerkontensteuerung öffnet ihren Dialog. Eine
eigene Rückfrage davor wäre eine zweite, die nichts prüft.

⚑ **`exec` und nicht `spawn`:** Der erhöhte Lauf soll dasselbe Terminal
haben und derselbe Vordergrundprozess sein. Ein Elternprozess, der nur
wartet, fängt ausserdem Strg-C ab, das dem Kind gilt.

⛔️ **`HOME` geht mit, und das ist kein Beiwerk.** `sudo` setzt es sonst
auf das des Verwalters, und damit läge die Ablage des Agenten unter
`/var/root`: andere Einstellungen, anderes Modell, andere
Gesprächsablage. **Der Schalter soll Rechte geben und nicht die Identität
wechseln.** Gesetzt wird es über `env`, weil das auf jedem System da ist
und an keiner `sudoers`-Regel hängt; ein `--preserve-env`, das die Regel
verbietet, liesse `sudo` scheitern und der Mensch sähe eine Meldung über
eine Einstellung, die er nie angefasst hat. Die eigenen `MYL_`- und
`INTEGER_LLM_`-Angaben gehen aus demselben Grund mit.

⛔️ **Ein Riegel gegen die Schleife** (`MYELITH_ERHOEHT`): Gelingt die
Erhöhung und bringt trotzdem keine Rechte, sähe der zweite Lauf dieselbe
Lage wie der erste und entschiede dasselbe, endlos.

⚑ **Vier Gründe, und jeder wird benannt**, statt nur nicht zu erhöhen:
schon Verwalter, schon versucht, abgeschaltet (`MYELITH_OHNE_ERHOEHUNG`),
kein Terminal. ⚠️ **Ohne Terminal wird nicht erhöht**, denn ein Passwort
kann dort niemand eingeben; der Einhängepunkt wird trotzdem gesetzt und
die Rechtelage gesagt.

⚑ **`/root` erhöht nicht**, und das bleibt so. Der Befehl fällt mitten in
eine Sitzung, in der ein Modell geladen ist und ein Gespräch steht; ein
Neustart würfe beides weg.

⚠️ **Der Windows-Weg ist gebaut und hier nicht erprobt.** Geprüft ist
sein **Aufruf** (Zitierung eines Pfads mit Leerzeichen, keine leere
`-ArgumentList`), nicht sein Lauf. Deshalb PowerShell und nicht die
Win32-Schnittstelle: Die bräuchte rohes FFI oder eine Kiste, und **eine
Erhöhung ist die falsche Stelle für ungeprüften unsicheren Code.**

**Belegt:** sechs neue Proben (beide Aufrufe, Heimat, Riegel, alle vier
Gründe, Vorrang des Abschalters), der Riegel mutiert und die Probe
gefallen, ein Lauf ohne Terminal von Hand nachgesehen.

### v0.61.0 – 2026-09-23 (zwei Häkchen für den Blick, und ein Schalter, der die Grenze ganz aufhebt)

**Die Blickerlaubnis steht jetzt in den Einstellungen unter „Agent"**,
als zwei Häkchen: „Bildschirm ansehen dürfen" und „Kamera ansehen
dürfen". Sie sind getrennt, weil ein Bildschirm den Rechner zeigt und
eine Kamera den Raum.

⚑ **Die Konsole hat beide von Haus aus an** (Festlegung des
Projektinhabers). **Dieselbe Bauart wie beim Arbeitsverzeichnis:** Die
Konsole beantwortet eine Frage selbst, die das Fenster als Einstellung
führt. Wer `myelith` tippt, sitzt davor und sieht jeden Werkzeugaufruf
über den Schirm laufen; ein Fenster kann offen stehenbleiben.

⚑ **Die Umgebung übersteuert beides, und zwar in beide Richtungen.**
`Blickbefugnis::fuer` kennt deshalb **drei** Zustände statt zwei: nicht
gesetzt (die Vorgabe gilt), ausdrückliches Ja, alles andere. Ohne den
dritten gäbe es in der Konsole keinen Weg, den Blick für einen Lauf
abzuschalten. ⚠️ „Alles andere" heisst Nein, immer: Ein Tippfehler fällt
damit auf die sichere Seite, gleich wie die Vorgabe steht.

⛔️ **Neu: `myelith --root` und `/root` hängen das ganze Dateisystem
ein**, mit Schreibrecht. Auf Unix `/`, unter Windows das Systemlaufwerk.

⚑ **Der Schalter fragt nicht, der Befehl schon.** `--root` tippt ein
Mensch, bevor das Programm läuft, und das **ist** die Zustimmung;
`/root` fällt mitten in eine Sitzung, in der ein Modell läuft und ein
Gespräch steht, und dort ist ein Tippfehler eine Zeile und kein
Entschluss. Die Rückfrage nennt, was wegfällt, und ihre Vorgabe ist
**Nein**: leere Eingabe, Abbruch und alles ausser einem ausdrücklichen
Ja zählen als Nein. ⚠️ **Ohne Terminal wird nicht gefragt und nicht
eingehängt.** Eine Frage, die niemand liest, ist keine Zustimmung.

⚠️ **Die Rechte kommen vom Start, nicht vom Schalter, und das ist eine
bewusste Abweichung.** `--root` verschiebt die Einhängegrenze; wieviel
dahinter erreichbar ist, entscheidet, als wer das Programm läuft. Für
Verwalterrechte: `sudo myelith --root`. **Ein Programm, das sich selbst
erhöht, nähme sich Rechte, die beim Start niemand gegeben hat**, und die
Warnung stünde dann hinter der Erhöhung statt davor. Beide Meldungen
sagen deshalb, wie die Rechtelage wirklich ist; auf Unix wird sie
gelesen (`geteuid`), unter Windows steht ausdrücklich „nicht
feststellbar" statt „nein".

**Belegt:** vier neue Proben in der Konsole (absolute Wurzel, Befehl in
der Hilfe samt Hinweis auf die Rückfrage, Konsolenvorgabe gegen die
Fenstervorgabe, Schreibrecht am Wurzelschalter), Clippy und Proben über
alle vier Client-Kisten grün.

### v0.60.0 – 2026-09-23 (der Agent darf hinsehen, wenn er gefragt wird, und nur dann)

**Zwei neue Werkzeuge:** `bildschirm_ansehen` und `kamera_ansehen`. Sie
nehmen **jetzt** ein Einzelbild auf und lassen es vom Sehmodell ansehen.

⚑ **Abruf und kein Strom, und das ist gemessen entschieden.** Ein
laufender Strom wäre die naheliegende Bauform und die falsche: Ein
Sehdurchgang kostet rund drei Sekunden, das grosse Modell wartet auf der
Maschine ohnehin auf die Platte, und **vor allem entscheidet die Frage
über die Antwort.** Auf „wie viele blaue Kreise" kam exakt die Zahl; die
allgemeine Beschreibung desselben Bildes hat sie nie enthalten. Ein
Strom erzeugt Beschreibungen, nach denen niemand gefragt hat.

⚑ **Deshalb ist `frage` bei beiden Pflicht** und bei `bild_beschreiben`
nicht: Ein Bild liegt schon da und lässt sich noch einmal ansehen, ein
Blick ist ein Augenblick. Die Beschreibung von `bild_beschreiben` ist aus
demselben Grund neu gefasst; sie sagt dem Modell jetzt, dass es gezielt
fragen soll, statt nachträglich nachzufragen.

⛔️ **Der Blick ist eine eigene Befugnis und nicht noch ein
Lesewerkzeug.** Ein Bild anzusehen, das im Arbeitsordner liegt, fasst die
Einhängegrenze ein. Den Bildschirm oder die Kamera aufzunehmen erzeugt
etwas, das vorher nicht da war, und zwar aus einem Raum, den keine
Einhängung begrenzt. **Wer eine Einhängung setzt, hat nicht gesagt, dass
jemand ins Zimmer sehen darf.** Beide stehen deshalb nur im Angebot, wenn
sie ausdrücklich scharf gestellt sind (`MYL_BLICK_BILDSCHIRM`,
`MYL_BLICK_KAMERA`), und getrennt voneinander. ⚠️ Alles, was nicht `1`,
`ja` oder `an` heisst, gilt als nein: Ein Tippfehler darf die Kamera nicht
anschalten.

⚑ **Jede Aufnahme bleibt unter `.AGENT/blicke/` liegen**, und ihr Pfad
steht in der Antwort. Eine Aufnahme, die nur im Speicher existiert, ist
von aussen nicht nachprüfbar.

**Welches Modell, und warum das eine Messung ist** (2026-09-23): Auf
einer freigestellten Hand mit einem ausgestreckten Finger antwortete das
**genaue** Modell „1", das schnelle „5", also die Vorannahme „eine Hand
hat fünf Finger". Reine Objektzählung können beide (drei, fünf und sieben
Kreise exakt). ⚑ **Ein Blick, der gezählt werden soll, braucht das
genaue Modell**, und der Werkzeugweg nimmt es.

**Aufnahme ohne neue Fremdkiste:** Auf macOS `screencapture` (liegt bei,
kein Format, kein Gerät), sonst `ffmpeg`, das für den Ton ohnehin
gebraucht wird. Die Kamera holt immer ffmpeg. ⚠️ **Die ersten
Kamerabilder werden verworfen** (`MYL_KAMERA_VORLAUF`, Vorgabe acht):
Eine Kamera stellt Belichtung erst im Laufen ein, und das erste Bild ist
dunkel.

⛔️ **Der Rückgabewert allein trägt die Zusage nicht.** Geprüft wird
zusätzlich, ob wirklich eine Datei mit Inhalt entstanden ist. 📌 Ohne die
Freigabe zur Bildschirmaufnahme meldet `screencapture` auf dieser
Maschine `could not create image from display` und endet mit eins; die
Meldung nennt deshalb den Weg zur Freigabe.

**Am Bündel:** `NSCameraUsageDescription` ist ergänzt, aus demselben
Grund wie das Mikrofon. ⚠️ **Für die Bildschirmaufnahme gibt es keinen
solchen Schlüssel**; sie wird einmal in den Systemeinstellungen
freigegeben, und macOS merkt sie sich an der Kennung samt Signatur.

⚑ **`myl sinne` nennt beide mit**, und zwar Gerät und Erlaubnis
getrennt: „Bildschirm ✓ screencapture" neben „Blicken: Bildschirm AUS".
📌 Am 2026-09-18 standen die Sinneswerkzeuge schon einmal in der Rüstung
und in keiner Liste; **dieselbe Frage an zwei Orten, und der zweite
meldet sich nicht.** Wer zwei Haken sieht und die Erlaubniszeile nicht
liest, sucht den Fehler danach an der falschen Stelle.

**Belegt:** sechs neue Proben im Client (Befugnis in beide Richtungen,
getrennt je Gerät, Gerät ohne Befugnis, Pflichtfrage, nur ausdrückliches
Ja) und vier in `myl-senses` (beide Aufnahmewege, Kameravorlauf, Lauf
ohne Bild). Die Scharfstellung mutiert, die Probe fällt. **264 Proben in
`myl-client`, 65 in `myl-senses`**, Clippy über alle vier Kisten grün.

### v0.59.2 – 2026-09-22 (ein Bild, das zu gross war, und ein Protokoll, das das falsche Ende behielt)

⛔️ **Gemeldet vom Projektinhaber: Ein Bildschirmfoto von 5,6 MB wurde
nicht ausgewertet.** Das Sehmodell brach mit Rückgabewert 1 ab, und das
Sprachmodell antwortete daraufhin, es sei ein reines Textmodell. **Zwei
Funde, und der erste hat den zweiten verdeckt.**

⛔️ **Fund 433: Das Protokoll behielt den Anfang, gemeldet wurde das
Ende.** `prozess::laufen` schnitt beide Röhren bei 8 KB ab und behielt
jeweils den **Anfang**; die Fehlermeldung zeigt aber unter der
Überschrift „Die letzten Zeilen" die letzten Zeilen dieses Anfangs. Bei
einem grossen Bild sind das die Fortschrittszeilen der ersten Sekunden.
📌 **Die Absicht stand die ganze Zeit im Quelltext** („Der Grund steht
bei llama.cpp am Ende") und die Umsetzung widersprach ihr; **die Ursache
wurde verworfen, bevor sie entstand.**

⚑ **Behoben je Röhre verschieden, und das ist der Kern:** stdout ist die
**Antwort**, dort zählt der Anfang; stderr ist das **Protokoll**, dort
steht der Grund am Ende. Drei Proben halten beide Richtungen fest, dazu
die Zeichengrenze beim Kürzen von vorn.

⛔️ **Fund 435: Ohne Grenze für die Bildtoken scheitert ein grosses
Bild.** Nachgestellt mit einem Bild zu 6200 mal 4600 (SmolVLM):

| Grenze | Blöcke | Ergebnis |
|---|---|---|
| ohne | 387 | ⛔️ `failed to find a memory slot`, Rückgabewert 1 |
| ohne, Kontext 32768 | 387 | läuft durch und liefert **Unsinn** |
| 256 | 7 | gute Beschreibung, Text nur sinngemäss |
| **1024** | **27** | **gute Beschreibung, Text wörtlich** |
| 2048 | 55 | fängt an, sich zu wiederholen |

⚑ **Der Aufruf gibt jetzt `--image-max-tokens 1024` mit**
(`MYL_SEHEN_BILDTOKEN`, null lässt die Option weg). **Die Grenze ist
keine Sparmassnahme:** Mehr Bildtoken machen die Antwort **schlechter**.
Schon ein Bild zu 4000 mal 3000 brachte ohne Grenze statt einer
ausführlichen Beschreibung nur noch einen Satz.

📌 **Und ein grösserer Kontext ist nicht die Abhilfe.** Mit `-c 32768`
läuft derselbe Lauf durch und gibt Zeichensalat aus. Ein Abbruch, den
man sieht, ist besser als eine Antwort, die falsch ist; richtig ist
beides nicht.

Der Kommandoaufbau ist dafür aus `beschreiben` herausgelöst
(`befehl_fuer`), wie es `sprechen.rs` vormacht: Ein Aufruf, der nur mit
einem echten Sehprogramm entsteht, wird nie geprüft, denn in der CI
liegt keines. 42 Proben der Kiste grün, beide Behebungen einzeln
mutiert, beide Proben fallen.

### v0.59.1 – 2026-09-22 (der Denkblock, der nie geöffnet wurde, weil die Aufforderung ihn schon offen hielt)

⛔️ **Fund 431: Beim hybriden 35B stand die ganze Überlegung des Modells
als Antworttext im Fenster**, samt Entwürfen und Selbstgespräch, statt in
einer Klappe. Gemeldet vom Projektinhaber.

**Die Ursache ist eine Annahme, die für ein Modell nicht gilt.** Der
Zerleger des Antwortstroms schaltet auf „Denken" um, sobald er `<think>`
liest. Die Vorlage `ChatMlDenkblock` **öffnet diese Marke aber in der
Aufforderung** (das ist der Zweck dieser Vorlage, sie kam mit dem 35B
dazu); das Modell schreibt seine Überlegung deshalb sofort los und
sendet nur noch das schliessende `</think>`. Der Zerleger wartete auf
ein Öffnen, das nie kam, und gab alles als Antworttext heraus.

⚑ **Behoben an zwei Stellen, und sie fragen bewusst verschieden:**

- **Im laufenden Strom** beantwortet die **Vorlage** die Frage
  (`Vorlage::oeffnet_denkblock`), denn dort ist das Schliessen noch
  nicht eingetroffen. Wer darauf wartete, hätte die Überlegung längst
  ausgegeben.
- **Für einen fertigen Text** (Verlauf, Vorlesen) sagt der Text es
  selbst: Ein wohlgeformter Strom trägt das Öffnen **vor** dem
  Schliessen, also heisst ein `</think>` ohne `<think>` davor genau,
  dass der Block beim ersten Zeichen schon offen stand.

📌 **Warum nicht einmal für beide.** Der laufende Strom kennt seine
Zukunft nicht, ein fertiger Text schon. Eine gemeinsame Regel müsste
sich nach dem ärmeren der beiden Fälle richten und bräuchte dann doch
die Auskunft der Vorlage; die Begründung steht an beiden Stellen im
Quelltext.

**Belegt:** sechs neue Proben, darunter zwei Gegenproben, die den
umgekehrten Fall festhalten (ein wohlgeformter Text darf sich **nicht**
anders verhalten, und ein Zerleger ohne den Hinweis muss die Überlegung
in den Antworttext legen). Beide Behebungen einzeln mutiert, beide
Proben fallen. 205 Proben der Kiste grün.

### v0.59.0 – 2026-09-22 (ein Modell, das in der Familienzuordnung fehlte, bekam rohen Text statt ChatML)

⛔️ **Fund 425:** `Vorlage::fuer_familie` kannte `qwen3` und `qwen3-moe`.
Die Familie des Qwen3.6 heisst `qwen3_5-moe-hybrid` und fiel damit in
den `_`-Zweig, also auf `Fortsetzung`: Ein Denkmodell bekam rohen
Fortsetzungstext statt ChatML und antwortete mit Kauderwelsch.

📌 **Ein `_`-Zweig, der eine Notform liefert, verbirgt jedes neue
Modell.** Er ist hier richtig, denn ein Basismodell soll fortsetzen, aber
er meldet nichts, wenn ein Chatmodell hineinfaellt. Der Fehlschlag sieht
dann wie ein Numerikfehler aus, und genau dorthin hat er auch gezeigt.

⚑ **Neue Variante `ChatMlDenkblock`**, weil dieses Modell den Denkblock
anders handhabt: Qwen3 schreibt `<think>` selbst, sobald es dran ist;
die Vorlage des Qwen3.6 stellt es in die **Aufforderung**, das Modell
setzt also innerhalb des Blocks fort. Fehlt die Marke, antwortet es auf
einer Form, die seine Vorlage nie erzeugt.

⚠️ **Eine eigene Variante und kein Schalter an `ChatMl`**, damit die
Form fuer Qwen3 Zeichen fuer Zeichen bleibt, was sie war; vier
eingesetzte Modelle haengen daran. Gegengeprueft: Das 0,6B antwortet
unveraendert und oeffnet seinen Denkblock weiterhin selbst, und die
Konformitaet steht bei 48/48.

Zwei neue Proben in `tests/chatvorlage.rs`: `die_familien_sind_abgedeckt`
haelt die Zuordnung je Familie fest (samt der Gegenrichtung, dass ein
Basismodell fortsetzt), `der_denkblock_wird_nur_dort_vorgegeben_wo_er_hingehoert`
prueft die offene Marke, ihr Fehlen bei Qwen3 und den geschlossenen
leeren Block ohne Denkmodus. Neun Proben gruen.

**Wirkung auf das 35B:** aus `1: 1: 1: 1` wurde eine zusammenhaengende
Antwort.

### v0.58.0 – 2026-09-21 (alle fremden Gewichte an einem Ort, nach Rubrik getrennt)

`myl-senses` **0.2.0**, `myl-client` **0.43.0**, `myl-oberflaeche`
**0.39.1**. Festlegung des Projektinhabers: Die von außen geladenen
Gewichte dieses Projekts liegen künftig zusammen unter `MODELS/`, in den
Rubriken `llm`, `audio` und `vision`, und was neu dazukommt, wird dort
einsortiert. Versioniert wird dort nichts außer den Ordnern, der Doku und
den Lizenzdateien.

⚑ **Damit ist eine Trennung aufgehoben, die nichts trug.** Die
Quellmodelle des Sprachmodell-Pfads lagen unter `INTEGER_LLM`, die
Gewichte für Sehen, Hören und Sprechen außerhalb des Klons, und beide
waren dasselbe: fremde Gewichte, groß, nicht versioniert, von ihrer
Quelle geladen. **Zwei Ablageorte für eine Sorte Sache sind zwei Orte,
an denen jemand sucht.**

⚑ **Gesucht wird jetzt in drei Stufen**, und die Reihenfolge ist die
Aussage: `MYL_SINNE`, wenn gesetzt (dann liegt dort alles zusammen),
sonst die Rubriken im gefundenen Klon, sonst `~/.myelith/sinne`.
⛔️ **Die letzte Stufe ist kein Altlastenrest, sondern der einzige Weg
für ein ausgeliefertes Programm:** Wer nur ein Freigabebündel geholt hat,
hat keinen Klon und damit kein `MODELS/`. Fünf Proben halten die
Reihenfolge, jede mit ihrer Gegenprobe.

⚑ **Was NICHT umgezogen ist, und der Schnitt ist der eigentliche
Gedanke.** In der Sinnesheimat bleiben der `bin`-Ordner mit dem
Sprech- und dem Aufnahmeskript und eine abgelegte Stimmprobe. Das eine
ist **erzeugt** und muss beschreibbar sein, das andere gehört dem
**Nutzer**; nur **geladene** Gewichte haben eine Rubrik verdient. Deshalb
gibt es zwei Begriffe statt einem: einen für den Arbeitsort, einen für
die Gewichtsorte.

⛔️ **Fund 410: die Plattenzahl im Fenster wäre still um 66 GB gefallen.**
`belegung_heute` summierte `INTEGER_LLM/models` und
`INTEGER_LLM/artifacts`; nach dem Umzug gibt es das erste nicht mehr, und
ein Pfad, der auf nichts zeigt, **wirft keinen Fehler, sondern zählt
null**. 📌 **Ein Pfad, der auf nichts zeigt, meldet sich nicht; er
antwortet.** Die Liste der gefüllten Orte steht jetzt an einer Stelle,
nicht in der Oberfläche.

⚑ **Die Wurzelmarke steht seither nur noch einmal.** Auch die
Sinneskiste braucht jetzt den Klon, um ein Sehmodell zu finden. Eine
zweite Marke dort wäre dieselbe Zeichenkette an einem zweiten Ort
gewesen; sie liegt deshalb in der Kiste ohne Abhängigkeiten, und der
Client zeigt darauf. **Was ermittelt wird, steht unten; was gemerkt
wird, steht oben.**

⚠️ **Fund 411: ein Doc-Kommentar trug seinen eigenen Anfang zweimal.**
Ein zu weit gefasster Textschnitt hatte `⚑ Der Hinweis nennt den
Paketverwalter dieses Systems` an dieselbe Zeile geklebt. Behoben.

**Belegt, mit echtem Material auf einem M5 Pro:** Sehen las beide Zeilen
eines Diagramms in 3,9 s, Hören schrieb die Stimmprobe wortrichtig bis
auf den Eigennamen mit (0,9 s), Sprechen lief über CosyVoice in 21,0 s.
⚑ **Und das Sprechen ist der Beleg, auf den es ankam:** Seine
Python-Umgebung trägt absolute Pfade und übersteht ein Verschieben nicht
von selbst; sie ist mitgezogen und nachgerichtet worden, 51 Shebangs und
vier Aktivierungsskripte. 55 Proben in `myl-senses`, alle grün, Clippy
über alle 25 Kisten ohne Warnung.

### v0.57.0 – 2026-09-17 (Dateien anhängen, und eine eigene Kiste für Sehen, Hören und Sprechen)

`myl-client` **0.42.0**, `myl-oberflaeche` **0.39.0**, `myl-console`
**0.13.0**, `myl-senses` **0.1.0** (neu). Auftrag des Projektinhabers:
Audio, Vision und Dateiupload.
**Dies ist der erste Teil, und er ist die Voraussetzung für die beiden
anderen:** Bis heute konnte der Client überhaupt keine Datei
entgegennehmen.

⛔️ **Der naheliegende Weg wäre gewesen, den Inhalt in die Nachricht zu
schreiben, und er ist falsch.** Die Messung desselben Tages zeigt, was
ein voller Kontext kostet und wie gut Nachschlagen stattdessen wirkt: das
4B fand eine Einzelheit in **neun von neun** Läufen mit **einem**
Werkzeugaufruf, bei einem Viertel des Kontexts. ⚑ **Eine angehängte
Datei wird deshalb benannt und nicht geliefert.**

⚑ **Sie wandert nach `.AGENT/anhaenge/`**, also unter die Einhängung:
`read_file`, `search_files` und `list_directory` erreichen sie damit
ohne ein neues Werkzeug und ohne eine neue Grenze. **Und nicht in den
Arbeitsordner**, denn der gehört dem Nutzer und ist oft ein
Repositorium; `.AGENT/` gehört dem Agenten und schließt sich selbst von
der Versionsverwaltung aus.

⚑ **Die Art hängt an den ersten Bytes und nicht an der Endung.** Eine
Endung ist eine Behauptung des Dateinamens, die Bytes sind die Datei
selbst. ⛔️ **Und eine leere Datei ist kein Text**, auch wenn sie als
UTF-8 gültig ist; ohne diese Bedingung kam die Endung nie zum Zug.

⛔️ **Bei Bild und Ton sagt die Nachricht die Wahrheit:** „Dieses Modell
sieht Bilder nicht und hört Ton nicht." **Ein Hinweis, der das
verschweigt, lädt zu einer Antwort ein, die erfunden ist.** Steht ein
Werkzeug bereit, das die Datei auswerten kann, nennt die Zeile es beim
Namen, denn ein Hinweis ohne Weg ist eine Sackgasse.

## Sehen, Hören und Sprechen: eine eigene Kiste

⚑ **Der Agent bleibt ein Textmodell.** Soll er ein Bild sehen, steht
dahinter ein eigenes, kleines Modell (Festlegung des Projektinhabers:
„kleine aber leistungsfähige Modelle, dass es modular bleibt"). Ein
multimodales Hauptmodell kostet Bildmarken, Artefaktbau und
Konformitätsvektoren, dauerhaft und für jeden, auch für den, der nie ein
Bild anhängt.

⚑ **Das liegt in `CLIENT/myl-senses`, und der Grund ist die
Chatfunktion.** Die Sinne werden an zwei Stellen gebraucht: in der
Agentenschleife, wo ein Werkzeug sie ruft, und im **Chat, wo es keine
Werkzeugschleife gibt**. Wer im Chat ein Bild anhängt, kann auf keinen
Werkzeugaufruf hoffen; das Bild muss angesehen werden, wenn es
hereinkommt, oder nie. **Zwei Umsetzungen wären die Fehlerklasse, die
dieses Projekt am häufigsten trifft**, also liegt die Sache einmal da
und beide rufen sie.

⛔️ **Der Chat wertet nur aus, was der Nutzer ausdrücklich angehängt
hat** (Festlegung des Projektinhabers). Kein Blick in den Arbeitsordner,
kein Nachladen, keine Dateiwerkzeuge; der Pfad muss unter dem
Anhangordner liegen, und das wird geprüft.

| Sinn | Richtung | Vorgabe |
|---|---|---|
| Sehen | Bild zu Text | llama.cpp, `sehen.gguf` samt mmproj |
| Hören | Ton zu Text | whisper.cpp, `hoeren.bin`, bei Bedarf ffmpeg |
| Sprechen | Text zu Ton | piper, `sprechen.onnx` |

⚑ **Zwei Sprossen beim Sehen**, ein kleines schnelles Modell und ein
größeres genaues. **Die Wahl trifft der Aufrufer und nicht das Modell**,
denn ein zusätzliches Argument kostet jedes kleine Modell eine
Entscheidung, die es schlecht trifft: beim Anhängen die schnelle, beim
ausdrücklichen Werkzeugaufruf die genaue. Wer nur eines hinlegt, bekommt
es für beides.

⛔️ **Die erste Fassung waren `sh`-Skripte in einer Werkzeugkiste, und
sie ist verworfen worden.** Drei Gründe, jeder für sich hinreichend:
Ein Manifest läuft über `sh`, und **unter Windows gibt es keine**; es
hält die Einhängegrenze **nicht** ein; und es braucht die
Schreiberlaubnis, weil eine Shell immer schreiben kann. **Ein Bild
anzusehen ist aber ein Lesen.** Jetzt sind es kompilierte Werkzeuge, der
Pfad geht durch die Einhängung, und sie stehen nur im Angebot, wenn der
Sinn wirklich eingerichtet ist.

⛔️ **Fehlt ein Laufwerk oder ein Modell, sagt der Sinn, was fehlt und
wie es hinkommt**, mit Pfad und Befehl, und tut so, als hätte er nichts
gesehen. **Eine Voraussetzung, die erst beim Absturz sichtbar wird, ist
keine Voraussetzung, sondern eine Falle.** ⚑ Und ein Mangel geht an den
**Menschen**, nicht in den Kontext: Eine Einrichtungsanleitung im
Gespräch hilft dem Modell nicht.

⚑ **Die Sprechtaste, erste Stufe eines Sprachmodus.** Sie setzt nur
zusammen, was da ist: aufnehmen (ffmpeg), mitschreiben (whisper),
antworten, sprechen. ⚑ **Sie liegt im Fenster und nicht in der Konsole**
(Festlegung des Projektinhabers): gedrückt halten, reden, loslassen. In
einem Terminal ginge das gar nicht, denn das Loslassen einer Taste
meldet nur, wer das Kitty-Protokoll spricht.

⚠️ **Live ist sie nicht**, und der Unterschied liegt nicht bei den
Sinnen: Bei 14 Token je Sekunde sind hundert Token Antwort rund sieben
Sekunden, Mitschreiben und Sprechen zusammen etwa zwei.

⚑ **Deshalb wird satzweise gesprochen**, und zwar aus dem Strom. Der
Vorleser bekommt die fertigen Sätze, während das Modell noch schreibt;
der erste ist nach ein bis zwei Sekunden hörbar statt nach zehn.
⛔️ **Ein Faden und eine Schlange, nicht ein Faden je Satz**: Die
Erzeugung darf nicht warten, **und** die Sätze müssen in der Reihenfolge
klingen. ⚠️ **Nur im Chat**: In der Agentenschleife stehen im Strom auch
Werkzeugaufrufe, und die will niemand vorgelesen bekommen.

## Die Oberfläche, aufgeräumt

⚑ **Eine Reihe statt verstreuter Knöpfe** (gemeldet vom Projektinhaber,
2026-09-18). Links, was der Frage etwas hinzufügt (Datei, Stimme),
rechts, was sie abschickt; der Lautsprecher und der Sprachmodus stehen
dazwischen, weil sie die **Antwort** betreffen und nicht die Frage.

⛔️ **Eine angehängte Datei war sofort ein eigener Beitrag** und damit
weg, bevor jemand etwas dazu schreiben konnte. ⚑ **Jetzt hängt sie als
Plättchen an der Eingabezeile**, lässt sich dort wieder abwählen, und
geht erst mit dem abgeschickten Auftrag ins Gespräch, wo sie als **Karte
unter dem Auftrag** steht. **Eine Datei ist ein Teil der Frage, die man
gerade formuliert.**

⚑ **Zwei Fassungen desselben Auftrags.** `modelltext` sieht das Modell
und trägt Pfad und Auszug; `text` sieht der Mensch. Ein Pfad ist eine
Auskunft für das Modell, keine für den Leser.

⚑ **Der Pegel kommt aus derselben Aufnahme**, über `astats` in ffmpeg,
und nicht aus einem zweiten Zugriff auf das Mikrofon: Ein zweiter
Verbraucher desselben Geräts wäre auf manchen Systemen ein Fehlschlag.
**Ein Mikrofon, das auf das falsche Gerät zeigt, sieht sonst genauso aus
wie eines, das zuhört.**

⚑ **Im Fenster spielt der Webview und nicht `afplay`.** Dreierlei: Er
kann anhalten, er braucht kein fremdes Programm, und **nur er weiß, wie
laut es gerade ist**. Ohne das Letzte gäbe es kein Zeichen, das
mitschwingt, sondern nur eines, das sich bewegt, und das wäre eine
Verzierung mit dem Anschein einer Auskunft. Der Ton geht als Text
hinüber, weil ein Webview keine beliebige Datei von der Platte laden
darf; vierzehn Zeilen Base64 gegen eine Abhängigkeit.

⛔️ **Die Aufnahme hört auf, wenn ihre Standardeingabe schließt.** Eine
WAV-Datei trägt ihre Länge im Kopf, und ein Prozess, den man erschlägt,
hinterlässt einen Kopf, der lügt. Erst nach zehn Sekunden ohne Reaktion
wird erschlagen, und dann sagt das Ergebnis es.

## CosyVoice spricht, piper fängt auf

⚑ **Festlegung des Projektinhabers:** CosyVoice ist die Vorgabe, weil es
am besten klingt und **eine Stimme nachbilden kann**. Python und die
Gewichte bringt der Nutzer mit; **nichts davon kommt ins Repositorium**.
Der Läufer, der CosyVoice bedient, ist eine Textdatei von wenigen
Kilobyte, steckt im Programm und wird auf Knopfdruck in die Heimat
geschrieben, wo er dann dem Nutzer gehört und nie überschrieben wird.

⚠️ **Was das kostet, gehört dazugesagt:** Wer kein Python und keine
Gewichte hat, kann nicht sprechen lassen. Deshalb bleibt **piper als
Rückfall** stehen, sofort einsatzbereit und ohne Klonen.

⛔️ **CosyVoice lädt je Aufruf ein halbes Milliardenmodell**, also
bekommt es einen **Dauerläufer**: einmal laden, dann Satz für Satz über
ein Zeilenprotokoll. Ohne ihn wäre satzweises Sprechen langsamer als gar
keines.

⛔️ **Und der Läufer lebt so lange wie das Fenster, nicht so lange wie
eine Antwort.** Die erste Fassung löste das Problem innerhalb einer
Antwort und entstand je Antwort neu; die achtzehn Sekunden standen damit
vor jeder einzelnen. 📌 **Eine Ladezeit, die man einmal zahlt, ist etwas
anderes als eine, die man immer zahlt, und im Code sieht beides gleich
aus.** ⚑ **Vorgewärmt wird beim Einschalten**: Während das Hauptmodell
nachdenkt, lädt der Sprecher.

⚠️ **Was bleibt, ist die Rechenzeit selbst**, rund 1,8fache Spieldauer
auf dieser Maschine. Dagegen hilft nur das satzweise Sprechen: Der erste
Satz klingt, während das Modell den zweiten schreibt.

⚑ **Eine hochgeladene Aufnahme wird zur Stimme**, und ihr Text wird von
whisper gleich mitgeschrieben: CosyVoice trifft die Stimme damit
deutlich besser, und ihn später von Hand nachzutragen wäre eine Aufgabe,
die niemand erledigt.

⛔️ **Zwei Funde beim tatsächlichen Einrichten, und beide hätte kein
Test gefunden.** **Fund 395:** Ein aus dem Finder gestartetes
`Myelith.app` erbt den PATH der Anmeldesitzung, und darin steht
`/opt/homebrew/bin` nicht; ein mit `brew` installiertes llama.cpp wäre
im Terminal da und im Fenster nicht, ohne eine Meldung, die das sagt.
**Fund 396:** Der erste Python im PATH ist unter macOS 3.9 und ohne
torch, also gewinnt jetzt die `.venv` neben der CosyVoice-Installation.
📌 **Beides ist nur aufgefallen, weil es jemand wirklich installiert
hat.**

⚑ **Gemessen an echtem Material, nach dem Einrichten.** Hören: eine
gesprochene Aufnahme, wortrichtig bis auf den Eigennamen, 0,8 s warm.
Sehen: ein Schild mit Text, richtig gelesen, 1,6 s warm. ⚠️ **Das
2,2B-Sehmodell antwortete englisch auf eine deutsche Frage und
schweifte ab**; genau dafür gibt es die zweite Sprosse.

⚑ **Der Befehlslauf liegt jetzt auch dort** (`myl_senses::prozess`). Er
stand in `myl-client`, und die Sinneskiste hätte ihn ein zweites Mal
gebraucht: zwei Läufer mit zwei Fristen, und der zweite meldet sich
nicht. In `myl-client` bleibt, was den Aufruf ausmacht, die Sperrliste
und die Form der Antwort.

⚑ **Drei Erweiterungen am Kistenformat bleiben**, sie sind allgemein
und nicht auf die Sinne zugeschnitten: `$MYL_KISTE` (ein Manifest findet
ein Skript neben sich), `zeitgrenze_s` (eine eigene Frist je Werkzeug)
und `fuer` (das Manifest sagt selbst, welche Art Anhang es lesbar
macht). Letzteres ist jetzt die Tür für **alles außer Bild und Ton**,
etwa eine Tabelle oder ein Fremdformat.

⛔️ **Fund 394: ein eigener Kistenordner verlor still die eingebauten
Werkzeuge.** Welche kompilierten Werkzeuge ein Lauf bekam, hing am
**Ordnernamen**, und der kennt drei Wörter; alles andere war `Base`. Wer
einen eigenen Ordner wählte, verlor unter anderem die Suche im
Mitschnitt, **ohne dass irgendwo etwas stand**. ⚑ Jetzt sagt die Kiste
es selbst, in `kiste.json`. 📌 **Ein Name, der zwei Dinge bedeutet,
bedeutet irgendwann nur noch eines.**

⛔️ **Ein Fund beim Umzug, gefangen von einer vorhandenen Prüfung.** Die
`.gitignore` des Agentenordners wurde in ein Verzeichnis geschrieben,
das es noch nicht gab: Das geht still daneben, und dann läge die erste
angehängte Datei ungeschützt in einem fremden Repositorium. 📌 **Erst
den Ordner, dann die Sperre.**

⚑ **Das Kistenformat steht jetzt einmal**, in
`CLIENT/werkzeugkisten/README.md`. Es stand in `Base` und noch einmal in
`Advanced`; als es um drei Felder wuchs, waren das zwei Orte, von denen
sich der zweite nicht meldet. Eine Prüfung hält die Werkzeugliste im
README jeder Kiste gegen die Manifeste im Ordner.

⚑ **`myl anhaenge` zeigt, was liegt**, mit Größe und Summe, und
`--aufraeumen` löscht genau die Dateien, die es vorher genannt hat. Der
Befehl stand in einem Doc-Kommentar, bevor es ihn gab.

⚑ **Im Fenster ein Knopf neben dem Senden und das Ablegen per Maus.**
Das Ablegen geht über das Fensterereignis und nicht über HTML5: Im
Webview trägt eine abgelegte Datei keinen Pfad, Tauri meldet dagegen den
echten. **Ein Anhang ohne Pfad wäre ein Anhang, den kein Werkzeug
findet.** In der Konsole `/datei <pfad>`.

📌 **Zwei Prüfungen hier haben beim Bauen Falsches verlangt**, und beide
sind berichtigt statt umgangen: Eine verlangte, dass kein `..` im
Dateinamen steht, und fiel über das harmlose `hoch..md`; die Gefahr ist
aber nicht das Aussehen, sondern der Ort, also prüft sie jetzt, dass die
Kopie im Anhangordner bleibt. Und eine im Fenster hängt an einer
Zeichenkette, die eine Umformatierung zerlegt hat.

⚑ **Eine Datei anzuhängen darf ein Gespräch eröffnen**, wie das Tippen
einer Frage; der Wächter über die Zahl der Anlagestellen kennt jetzt
drei Wege statt zwei.

⚠️ **Angehängte Dateien werden nicht aufgeräumt.** Der Mitschnitt hat
eine Obergrenze, weil er von selbst entsteht; eine Datei hat der Nutzer
ausdrücklich hergegeben, und etwas wegzuwerfen, das jemand bewusst
angehängt hat, wäre eine Überraschung.

### v0.56.1 – 2026-09-17 (zwei rote CI-Prüfungen, und beide waren auf dieser Maschine unsichtbar)

`myl-client` **0.41.1**. Keine Verhaltensänderung, zwei Fehlerbehebungen.

⛔️ **Fund 392: eine `cfg`-Bedingung nannte das falsche System.**
`hersteller_aus_pnp` liest die Herstellerkennung aus einer
PNP-Gerätekennung, und die gibt es **nur unter Windows**; Linux liest den
Hersteller aus sysfs und ruft die Zuordnung unmittelbar. Der Vermerk
gegen die Warnung lautete aber `not(any(linux, windows))`, galt also auf
macOS und **nicht** auf Linux, wo die Funktion ebenso ungerufen bleibt.
⚑ **Hier war alles grün, die CI unter Linux brach ab.** Jetzt
`not(target_os = "windows")`, also genau das eine System, das sie ruft.

📌 **Eine `cfg`-Bedingung, die das falsche System nennt, fällt auf der
Maschine, auf der man sie schreibt, nie auf.** Nachgeprüft ist sie
deshalb, indem der Fehler hier **absichtlich erzeugt** wurde: Mit einer
Bedingung, die das Wirtssystem nicht deckt, meldet clippy denselben
Satz wie die CI, mit der richtigen ist es still.

⛔️ **Fund 393: ein zweites Binär machte `cargo run` mehrdeutig.** Seit
`nadelprobe` neben `myl` liegt, hielt `cargo run` mit „could not
determine which binary to run" an, und eine CI-Stufe, die die Hilfe
aufruft, fiel darüber. ⚑ **Das fällt nicht beim Bauen auf, sondern bei
dem, der es benutzt.** Jetzt sagt `default-run = "myl"`, was gemeint
ist; die Messung wird mit `--bin nadelprobe` gerufen.

⚠️ **Was daraus fürs nächste Mal folgt:** Die Ziele für Linux und Windows
sind auf der Entwicklungsmaschine installiert, aber ein
`cargo check --target …` scheitert an den C-Abhängigkeiten (`blst`,
`onig_sys`, `esaxx-rs`) ohne Kreuzübersetzer. **Für `cfg`-Fragen ist der
gangbare Weg deshalb, den Fehler hier absichtlich zu erzeugen**, und
nicht, auf die CI zu warten.

### v0.56.0 – 2026-09-17 (Wissensmappen: zwei Orte, zwei Werkzeuge, und ein Fund beim Einbauen)

`myl-client` **0.41.0**. Auftrag des Projektinhabers.

⚑ **Der Agent bekommt Wissen, das nicht im Kontext steht**, an zwei
Orten: `<einhängung>/.AGENT/skills/` für das, was zu **diesem Projekt**
gehört, und `<konfiguration>/skills/` für das, was **überall** gilt. Bei
gleichem Namen gewinnt das Projekt; die allgemeine Mappe ist die
Vorgabe.

⚑ **Der Projektordner entsteht mit `.AGENT`**, also beim ersten
Verdichten, und der Zeitpunkt ist der Punkt: **Nach jeder Verdichtung
ist das Wissen aus dem Kontext verschwunden, und dann muss es einen Ort
geben, an dem es noch steht.** Die Zusammenfassung nennt seither die
vorhandenen Mappen, gedeckelt auf zwanzig Zeilen und mit je einem Satz.

⛔️ **Braucht es dafür `read_skill`? Für den einen Ordner ja, für den
anderen nein, und der Unterschied ist die Einhängegrenze.** Der
Projektordner liegt **unter** ihr: `list_directory`, `read_file` und
`search_files` erreichen ihn schon, ein viertes Werkzeug zum Lesen einer
Datei wäre nur eine weitere Wahl für ein kleines Modell. Der allgemeine
Ordner liegt **außerhalb**; ihn zu erreichen hieße entweder die Grenze
aufzuweichen oder ein Werkzeug zu bauen, das nur ihn liest. ⚑ **Die
Grenze ist die Zusage des ganzen Werkzeugsatzes**, also das Werkzeug.

⚑ **`list_skills` nennt, `read_skill` liefert**, beide in `Advanced` und
nicht in `Base`, aus demselben gemessenen Grund wie die
Verlaufswerkzeuge. Sie hängen am selben **Nachschlagebudget**: Der
Kontext lässt sich über jeden Leseweg füllen, und drei eigene Budgets
wären drei Wege. `read_skill` gibt die **Eingangsseite**, nicht die
Mappe; welches Kapitel gebraucht wird, entscheidet das Modell danach.

⛔️ **Der Name wird nie zu einem Pfad.** `read_skill` hält ihn gegen die
gefundenen Mappen; `../../etc/passwd` ist damit einfach keine Mappe,
geprüft in beide Richtungen.

⛔️ **Und der Einbau hat einen Fehler freigelegt, den die vorhandenen
Prüfungen sofort gefangen haben:** `.AGENT/skills/` wurde als **Sitzung**
gezählt. Die Obergrenze von zwanzig wurde damit faktisch zu neunzehn,
und je nach Änderungszeit hätte das Aufräumen **die Mappen gelöscht**.
Ursache war, dass `beschneiden` seine **eigene** Vorstellung davon hatte,
was eine Sitzung ist. ⚑ **Jetzt gibt es eine Regel an einer Stelle**
(`ist_sitzung`: ein Ordner mit einem Mitschnitt darin), und sie hängt am
Inhalt statt an einer Liste verbotener Namen: Wer morgen einen zweiten
Nebenordner anlegt, ist automatisch geschützt.

⚑ **`myl skills`** nennt beide Orte und was darin liegt, `--anlegen`
legt den allgemeinen an. **Ein Ordner, den der Nutzer füllen soll,
dessen Ort er aber raten muss, wird nicht gefüllt.**

### v0.55.0 – 2026-09-17 (die Verlaufswerkzeuge verlassen `Base`, und eine Schranke ersetzt eine Bitte)

`myl-client` **0.40.0**, `myl-oberflaeche` **0.38.3**, `myl-console`
**0.12.3**. Festlegungen des Projektinhabers nach der Messung.

⛔️ **Die drei Verlaufswerkzeuge sind aus `Base` heraus.** Sie standen
dort einen Tag lang, weil „Nachlesen keine Sache der Modellgröße" ist.
⚑ **Die Messung sagt das Gegenteil, und zwar für genau das Modell, für
das diese Kiste gemacht ist:** Das 0,6B ruft sie in 27 Läufen **kein
einziges Mal**. ⚑ **Drei Werkzeuge, die nie gerufen werden, sind nicht
folgenlos:** Sie stehen in jeder Ansage, kosten in jeder Runde Kontext
und machen die Auswahl schwerer. **Eine Kiste ist eine Auswahl und keine
Sammlung.** `Base` hat damit wieder fünf Werkzeuge, `Advanced` neun.

⚠️ **Der Preis dieser Entscheidung, damit er dasteht:** Wer den
Mitschnitt nutzen will, nimmt `Advanced`, und darin liegt auch
`run_command`, das die Einhängegrenze nicht einhält. **Ein feiner
geschnittener Satz wäre ein eigener Ordner unter
`CLIENT/werkzeugkisten`**, denn eine Kiste ist ein Ordner.

⛔️ **Und eine Schranke ersetzt eine Bitte.** Die drei Werkzeuge teilen
sich ein **Budget von 8 000 Zeichen je Auftrag** (rund 2 000 bis 2 700
Token, unter 7 % eines Kontexts von 40 960). Was darüber hinausgeht,
bekommt eine Absage mit Begründung statt stiller Kürzung, und das
Budget beginnt mit der nächsten Frage neu.

⚑ **Der Grund steht in zwei Messungen:** Über das Verzeichnis fand ein
Modell die Einzelheit, und der Kontext am Ende war **so groß wie der
ganze Verlauf**, den die Verdichtung gerade weggeräumt hatte. Und eine
Bitte hilft dagegen nicht: **Modelle befolgen „ruf das Werkzeug nur,
wenn es nötig ist" nicht** (9 von 9 unnötige Aufrufe, auch mit
ausdrücklichem Gegenfall im Systemprompt). ⚑ **Also hält der Code, worum
man ein Modell nicht bitten kann.**

⚑ **Damit entscheidet auch nicht mehr eine feste Zahl von Abschnitten,
ob `list_history` das ganze Verzeichnis gibt**, sondern die Frage, ob es
in das Budget passt. Die Zahl war vorher zweimal falsch: zu klein (das
Verzeichnis verlor die Überschriften, das Modell suchte blind) und zu
groß (rund 6 000 Token in einer Antwort).

⚑ **War die Aufgabe für das kleine Modell zu schwer? Nachgemessen, und
nein.** Drei weitere Reihen zu je neun Läufen, die die Hürde senken:
mit der ausdrücklichen Anweisung nachzuschlagen **0 von 9**, mit der
deutschen Werkzeugansage **0 von 9**, mit beidem **0 von 9**. Das 0,6B
erfindet eine Kennung aus der Zusammenfassung, erzählt das Verzeichnis
nach oder setzt einen unbrauchbaren Aufruf ab. ⚑ **Nicht die Aufgabe war
zu schwer, sondern das Werkzeugformat liegt außerhalb dessen, was dieses
Modell sicher bedient.**

⚑ **Und die Wege, die tragen, tragen weiter:** mit Budget und
`Advanced` 4B **9 von 9**, 30B **7 von 9**, beide bei rund 2 000 bis
2 080 Token. **Das Budget schneidet nichts weg, was gebraucht wird.**

### v0.54.0 – 2026-09-17 (der Nachweis, dass das Nachschlagen wirkt, und drei Funde auf dem Weg dorthin)

`myl-client` **0.39.0**. Frage des Projektinhabers: Findet ein Modell
eine Einzelheit wieder, die aus seinem verdichteten Kontext verschwunden
ist, ohne den Kontext zu füllen?

⛔️ **Die ehrliche Antwort lautete zuerst: ungeprüft.** Geprüft war jedes
Stück der Mechanik, nicht aber der Zweck. **Ein Werkzeug, von dem
niemand weiß, ob ein Modell es findet, ist eine Vermutung mit
Quelltext.** Also eine Messung: `nadelprobe`, erfundener Verlauf,
erfundener Kontext, eine Kennung darin, die kein Modell raten kann und
die je Lauf wechselt.

⚑ **Verlauf und Kontext sind beide gesetzt, und die Abwesenheit der
Nadel wird zugesichert.** Der erste Entwurf ließ das Modell wirklich
verdichten und hoffte, dass die Nadel dabei verschwindet: Das 0,6B
schreibt statt zusammenzufassen den Anfang ab, und das 4B **behält** eine
auffällig markierte Einzelheit (3 von 3 Läufen ungültig). **Damit war die
Verdichtung Versuchsaufbau und Messgegenstand zugleich.** Jetzt prüft
die Probe vor jedem Lauf den ganzen Prompt auf die Kennung.

⛔️ **Fund 387: die Verdichtung machte den Kontext größer.** Das
Verzeichnis in der Zusammenfassung trug **eine Zeile je Nachricht**.
Gemessen an 120 Nachrichten (4 476 Token) wog es **3 597 Token**, und die
„verdichtete" Fassung war mit bis zu 5 659 Token **größer als das
Original**. ⚑ **Ein Verzeichnis, das mitwächst, kann nie sparen. Was in
den Kontext geht, braucht eine Schranke, die nicht vom Gespräch
abhängt.** Jetzt höchstens 16 Zeilen, benachbarte Abschnitte zu Blöcken
mit Zeilenspanne; kurze Gespräche behalten die genaue Karte. Danach bei
doppelt so langem Verlauf: 8 866 Token hinein, **1 522 hinaus**, davon
624 das Verzeichnis.

⛔️ **Dieselbe Schranke war in `list_history` falsch, und die Messung hat
es gezeigt.** Das 4B rief das Werkzeug, bekam eine Karte **ohne die
gesuchte Überschrift** und suchte danach blind in 27-Zeilen-Fenstern
weiter: sechs Aufrufe, kein Treffer. ⚑ **Die Köpfe sind der ganze Sinn
eines Verzeichnisses.** Die Schranke gehört in die Zusammenfassung, die
in **jeder** Runde im Kontext steht, und nicht in ein Werkzeug, dessen
Kosten einmalig und ausdrücklich verlangt sind. Grenze dort jetzt 400
Abschnitte, und darüber sagt die Antwort, dass sie gruppiert.

⛔️ **Fund 388: es fehlte die Suche.** Mit vollem Verzeichnis fand das 4B
die Nadel in 6 von 9 Läufen, aber der Kontext am Ende war rund **8 890
Token**, also so groß wie der ganze Verlauf, den die Verdichtung gerade
weggeräumt hatte. ⚑ **Das Verzeichnis beantwortet die falsche Frage:**
gefragt ist nicht „wie ist der Verlauf gegliedert", sondern „wo steht
dieses Wort". Neu ist **`search_history`**, in jeder Kiste, Klartext
statt regulärem Ausdruck, höchstens 20 Treffer.

⛔️ **Fund 389: eine Suche, die nur zeigt, wird für die Auskunft
gehalten.** Die erste Fassung gab nach dem Grundsatz „nennen und nicht
liefern" nur Zeilennummern zurück, und das 4B antwortete in **9 von 9**
Läufen „läuft unter der Kennung **612-614**". Die zweite gab die
gefundene Zeile, und das ist die **Frage**, in der das Suchwort steht,
während die Auskunft in der **Antwort** darunter steht: wieder kein
Treffer. 📌 **Ein Grundsatz, der an einer Stelle gilt, gilt nicht
überall:** Für `list_history`, das ungefragt fremde Sitzungen anfasst,
ist „nennen und nicht liefern" richtig; für eine Suche ist die Stelle
genau das, wonach gefragt wurde. Jetzt liefert sie den ganzen Wechsel,
höchstens 400 Zeichen je Treffer.

⚑ **Das Ergebnis, 27 Läufe je Weg über drei Stellen im Verlauf:**

| Modell | Weg | Läufe | gefunden | Kontext am Ende |
|---|---|---|---|---|
| `myelith-4b` | nur Verzeichnis, beschnitten | 9 | 0 | 1 785 bis 9 117 |
| `myelith-4b` | volles Verzeichnis | 9 | 6 | rund 8 890 |
| `myelith-4b` | **`search_history`** | 9 | **9** | **rund 2 015** |
| `myelith-30b-a3b` | **`search_history`** | 15 | **10** | rund 2 040 |
| `myelith-0.6b` | alle drei Wege | 27 | 0 | rund 1 700 |

Der ungekürzte Verlauf wiegt 8 866 Token. ⚑ **Die Gegenprobe lief in
jedem einzelnen Lauf mit** (dieselbe Frage, derselbe Kontext, **leerer**
Mitschnitt) und traf **kein einziges Mal**.

⛔️ **Das 30B ist nicht besser, sondern schlechter: 10 von 15.** Die
Ursache ist nachgelesen und keine Vermutung: **Es beschreibt manchmal,
was man tun könnte, statt es zu tun** („Sie können mit `search_history`
suchen … ich liste zunächst die Sitzungen auf"), und setzt danach keinen
Aufruf ab. ⚑ **Wenn es ruft, trifft es in einem Zug.** ⚠️ Das Gemisch
hält die Werkzeugform schlechter ein als das dichte 4B, und warum, ist
nicht untersucht.

⛔️ **Das 0,6B ruft in 27 Läufen kein einziges Werkzeug** und erfindet
stattdessen eine Kennung. ⚠️ **Wer den Mitschnitt für das 0,6B baut, baut
ihn für niemanden**; er trägt ab dem 4B.

⚑ **Nachtrag, dieselbe Sitzung: Lässt sich der Aufruf mit einem
Systemprompt erzwingen?** Gemessen in 63 weiteren Läufen, mit einer
Hausregel **hinter** der Werkzeugansage (die Vorlage selbst bleibt
zeichengleich) und einer **Kontrollfrage, deren Antwort im Kontext
steht**, denn eine Regel hat einen Preis.

| Modell | Regel | Nadel | Kontrollfrage richtig | davon Umweg |
|---|---|---|---|---|
| 0,6B | keine / streng / mild | 0/9 | 9/9 / **1/9** / 8/9 | 0/9 |
| 4B | keine / streng / mild | 9/9 | 9/9 | 9/9 |
| 30B | keine / mild | 17/24 / **9/9** | 9/9 | **0/9** / **9/9** |

⚑ **Ja, es wirkt: beim 30B von 17 aus 24 auf 9 aus 9**, genau dem
Modell, dessen Fehlschläge aus „beschreiben statt tun" bestanden. ⛔️
**Und es kostet: dasselbe Modell ruft danach auch dann ein Werkzeug,
wenn die Auskunft schon im Kontext steht (0 von 9 auf 9 von 9).** ⛔️
**Die Bedingung „nur wenn nötig" befolgt kein Modell**, auch nicht die
milde Fassung, die den Gegenfall ausdrücklich nennt: **Man kann einen
Aufruf erzwingen, sein Unterbleiben nicht erbitten.**

📌 **Beim 0,6B ist die strenge Regel schädlich**: kein einziger Aufruf,
und die Kontrollfrage fällt von 9 von 9 auf **1 von 9**. **Eine
Anweisung, die ein Modell nicht befolgen kann, ist nicht wirkungslos,
sondern verdrängt Platz und verschlechtert, was sonst richtig war.**

📌 **Eine Grundlinie hat dabei eine Vermutung widerlegt:** Die Umwege des
4B hatte ich für die Wirkung der Regel gehalten; es ruft die Suche auch
ohne jede Regel bei jeder Kontrollfrage. **Der Umweg gehört dem Modell,
nicht der Regel.**

⚠️ **Offen und ausdrücklich nicht nebenbei entschieden:** ob der Client
die Regel als Einstellung bekommt. Heute setzt sie nur die Messung; eine
Vorgabe, die für zwei von drei Modellen falsch ist, wäre schlimmer als
keine, und ⛔️ **im Netz muss sie für alle Knoten dieselbe sein**, sonst
rüsten zwei Knoten am selben Auftrag verschieden.

⚠️ **Was nicht gemessen ist:** eine Einzelheit, nach der man mit anderen
Worten fragt als denen, mit denen sie aufgezeichnet wurde; mehrere
Sitzungen in einem Ordner; der Fensterclient selbst (die Probe fährt
denselben Weg, ist aber nicht er).

### v0.53.0 – 2026-09-16 (der Mitschnitt gehört dem Ordner, nicht der Sitzung, und er ist Klartext)

`myl-client` **0.38.0**, `myl-oberflaeche` **0.38.2**, `myl-console`
**0.12.2**. Festlegungen des Projektinhabers.

⛔️ **Die Verschlüsselung ist wieder entfallen, am selben Tag, an dem sie
kam.** Sie versagt im Hauptfall: **Mehrere Personen am selben Ordner**
haben jede einen anderen Schlüssel, also liest niemand den Mitschnitt
eines anderen. ⛔️ **Und schon eine einzige Person mit zwei Maschinen**,
denn der Schlüssel lag in der Konfiguration und der Ordner wandert über
einen Abgleichdienst mit: Auf dem zweiten Rechner meldet sich „falscher
Schlüssel oder veränderte Datei", also genau das, was man bei einem
Angriff erwartet, und es ist der eigene Laptop.

⚑ **Das ist die Fehlerklasse, die dieses Projekt dauernd benennt:** etwas,
das dasteht und aussieht, als funktioniere es, und genau in dem Fall
versagt, für den es gedacht war. **Die Frage nach dem gemeinsamen Ordner
hätte vor dem ersten Rahmen stehen müssen**, nicht danach.

**Die Zusage lautet seither:** Der Mitschnitt ist Klartext und so
geschützt wie das Dateisystem, auf dem er liegt.

⚑ **Dafür kommt eine Sicherung neu dazu, und sie kostet nichts:** eine
`.gitignore` **im** `.AGENT`-Ordner mit `*`. Ein Arbeitsordner ist sehr
oft ein Repositorium, und ein Klartextverlauf in einem Commit ist genau
der Unfall, gegen den vorher die Verschlüsselung stand. Der Ordner
schließt sich selbst aus, ohne die `.gitignore` des Nutzers anzufassen.

⚑ **Der Mitschnitt gehört dem Ordner, nicht der Sitzung.** `.AGENT/`
liegt im Projektordner, und was dort steht, ist „was in diesem Projekt
geschehen ist". Eine Sitzung ist eine Episode davon (`.AGENT/<sitzung>/`),
der Ordner ist die Kontinuität. ⚑ **Genau deshalb kann ein neuer Agent
aufnehmen, woran der vorige gearbeitet hat** , auch wenn das Gespräch
längst aus der Liste ist.

⛔️ **Und daraus folgt, was das Aufräumen der Gesprächsliste NICHT tut:**
Es fasst den Mitschnitt nicht an. **Ein Gespräch aus der Liste zu nehmen
ist Aufräumen, einen Verlauf zu löschen ist eine eigene Handlung**, und
die gibt es jetzt: `myl verlauf --loeschen [<sitzung>]`. Vorher gab es
dafür gar nichts.

⚑ **Eine Obergrenze von zwanzig Sitzungen je Ordner**, die ältesten
fallen heraus. **Nach Zahl und nicht nach Tagen**, weil eine Frist den
überrascht, der nach sechs Wochen nachschlägt, und **ganze Sitzungen und
keine halben**, weil ein Verlauf mit einem Loch keines zeigt. ⚠️ Zwanzig
ist geschätzt und nicht gemessen.

⚑ **Die Zeilennummern zeigen jetzt auf Zeilen der Datei**, nicht auf
einen inneren Verlaufsteil. Ein Mensch springt im Editor dorthin,
`grep -n` meldet dieselbe Zahl, `read_history` gibt dieselbe heraus. Der
Versatz wird gerechnet **und danach geprüft**: Ein Verzeichnis, das um
eine Zeile danebenliegt, schickt jeden Leser an die falsche Stelle, und
niemand sieht es ihm an.

⚑ **`list_history`**, ohne einen einzigen Parameter: Es nennt die
früheren Sitzungen und gibt das Verzeichnis der jüngsten. **Es nennt und
liefert nicht**, denn ein Mitschnitt aus einer fremden Sitzung ist der
Verlauf eines anderen Gesprächs; ihn ungefragt in den Kontext zu ziehen,
wäre eine Überraschung.

⚑ **`myl verlauf` zeigt Zahl, Platz und Grenze**, denn was man sieht,
räumt man.

⛔️ **Fund 386, gefunden beim letzten Handgriff dieser Sitzung: Die
Schreibseite und die Leseseite meinten verschiedene Ordner.** Die
Konsole schreibt ihren Mitschnitt in **ihr Arbeitsverzeichnis** („Wer
`myelith` hier tippt, hat die Frage beantwortet"), das Fenster in den
**eingestellten** Ordner, und `myl verlauf` sah nur den eingestellten.
**Wer nach einem Konsolengespräch im selben Verzeichnis nachfragte,
bekam „Kein Mitschnitt" zu sehen, während die Datei danebenlag.** ⚑
**Es ist wieder dieselbe Angabe an zwei Orten**, und der zweite Ort hat
sich nicht gemeldet. `myl verlauf` nimmt jetzt das Arbeitsverzeichnis,
**wenn dort ein `.AGENT` liegt**, und sonst wie bisher die Einstellung:
Ohne diese Bedingung übernähme das Arbeitsverzeichnis jeden Aufruf.
Zwei Prüfungen, eine je Richtung, beide gegengeprobt.

### v0.52.0 – 2026-09-16 (`read_history`: der Agent liest im Mitschnitt nach, und zwar in jeder Kiste)

`myl-client` **0.37.0**. Auftrag des Projektinhabers: das Werkzeug bauen
und **in alle Kisten** legen, denn nachlesen zu können, was man selbst
gesagt bekommen hat, ist keine Sache der Modellgröße.

⚑ **`read_history(von, bis)`**, in `Base` und damit über die Kette auch
in `Advanced` und `1337`. Es schreibt nicht und braucht deshalb keine
Schreiberlaubnis: Es liest nach, was ohnehin gesagt wurde.

⛔️ **Der erste Entwurf war falsch herum, und eine bestehende Prüfung hat
es gefangen.** Er gab ohne Argumente das Verzeichnis heraus und mit
`von`/`bis` die Zeilen, also mit **optionalen** Parametern. Die Regel
dieses Projekts sagt: **kein Werkzeug hat einen optionalen Parameter**,
denn er ist eine Entscheidung, die das Modell treffen muss, und ein
kleines Modell bezahlt sie mit seinem Budget.

⚑ **Die bessere Antwort war nicht ein zweites Werkzeug, sondern ein
anderer Ort für das Verzeichnis:** Es steht jetzt in der
**Zusammenfassung**, also dort, wo das Modell ohnehin hinsieht. Damit
kostet es keinen Aufruf, das Werkzeug behält genau eine Aufgabe, und
beide Parameter sind verlangt. **Es ist außerdem klein**, eine Zeile je
Nachricht gegen den ganzen Verlauf, den es ersetzt.

⛔️ **Ein Deckel von 200 Zeilen je Aufruf, und er ist der Sinn der Sache
und keine Vorsicht.** Der Mitschnitt entsteht, weil der Kontext voll
war; ein Werkzeug, das ihn in einem Zug zurückholt, macht die
Verdichtung rückgängig. Wer mehr braucht, fragt zweimal, und dann ist es
eine Entscheidung. ⚑ **Die Kürzung sagt sich selbst an**, mit der Zeile,
ab der weiterzulesen ist: Eine stillschweigend gekürzte Antwort liest
sich wie eine vollständige.

⚑ **Erst der Aufruf, dann die Welt.** Ein fehlendes Argument wird
benannt, auch wenn es gerade nichts zu lesen gäbe; sonst bekäme ein
Modell, das `bis` vergessen hat, die Auskunft „es gibt keinen
Mitschnitt" und suchte den Fehler an der falschen Stelle.

⚑ **Fünf Prüfungen, drei Gegenproben, alle beißen.**

### v0.51.0 – 2026-09-16 (der Mitschnitt: was beim Verdichten verlorenginge, verschlüsselt im Arbeitsordner)

`myl-client` **0.36.0**, `myl-oberflaeche` **0.38.1**, `myl-console`
**0.12.1**. Auftrag des Projektinhabers.

⚑ **Verdichten ersetzt den Verlauf durch eine Zusammenfassung, und die
Urfassung war danach weg.** Das ist richtig für den Kontext, der eine
Grenze hat, und falsch für alles, wonach jemand später fragt: die genaue
Zahl, den Pfad, die Fehlermeldung von vorhin. Jetzt entsteht **im selben
Augenblick** ein Mitschnitt unter `.AGENT/` im Arbeitsordner, und die
Zusammenfassung selbst nennt ihn. ⚑ **Genau dort ist die Urfassung zum
letzten Mal vollständig**; wer sie später schreiben wollte, schriebe die
Zusammenfassung ab.

⚑ **Mit Kopf und Verzeichnis**, damit zeilenweise nachzulesen ist: je
Abschnitt Rolle, Zeilenbereich und die erste Zeile als Marke. `myl
verlauf` zeigt das Verzeichnis, `myl verlauf <von> <bis>` die Zeilen.

⛔️ **Verschlüsselt, und die Zusage steht genau:** Sie schützt **gegen
Mitlesen durch Dritte**, einen zweiten Nutzer auf der Maschine, eine
Sicherung, die den Arbeitsordner mitnimmt, einen Abgleichdienst, jemanden
mit der Platte. **Das ist genau der Weg, den ein Arbeitsordner wirklich
nimmt.** ⚠️ **Nicht gegen einen Prozess, der als derselbe Nutzer läuft**:
Der Schlüssel liegt in dessen Konfiguration. Das ist keine Lücke, sondern
die Bedingung, denn der Agent soll lesen können.

⚑ **Der Schlüssel liegt neben den Einstellungen und nie im
Arbeitsordner.** Läge er beim Mitschnitt, schützte die Verschlüsselung
vor niemandem: Wer den Ordner kopiert, kopierte den Schlüssel mit. **Die
Trennung dieser beiden Orte ist die ganze Zusage.**

⛔️ **Und der Schlüssel steht nie im Kontext des Modells.** Ein Schlüssel,
den das Modell einmal gesehen hat, steht im Verlauf, und der Verlauf ist
genau das, was hier verschlüsselt wird.

⚑ **In Rahmen, einer je Abschnitt.** Eine Frage nach zwanzig Zeilen
entschlüsselt nur die Rahmen, die sie tragen, und nicht den ganzen
Verlauf. Jeder Rahmen trägt seine Nummer im Siegel, also lässt sich
keiner gegen einen anderen tauschen.

⛔️ **Fund 385: der Schlüsselpfad wurde zweimal aufgelöst.** Er hängt am
Ort der Einstellungen und der an `XDG_CONFIG_HOME`, also an der Umgebung
des ganzen Prozesses. Wer sie mitten im Lauf verstellt, schriebe mit dem
einen Schlüssel und läse mit dem anderen, und die Meldung dazu lautete
„falscher Schlüssel oder veränderte Datei", also genau das, was man bei
einem Angriff erwartet. ⚑ **Gefunden durch eine Prüfung, die allein grün
war und im vollen Lauf rot.** Jetzt einmal je Prozess: **Ein Programm,
dessen Schlüssel sich zwischen Schreiben und Lesen ändern kann, ist
kaputt.** Dazu wird der Schlüssel unteilbar angelegt (erst daneben, dann
umbenannt), und wer ein Rennen verliert, nimmt den Schlüssel des
Gewinners.

⚑ **Sieben Prüfungen und eine Naht.** Der Klartext steht **nicht** in der
Datei (an den Bytes geprüft, nicht an der Absicht); das Verzeichnis
stößt lückenlos aneinander; eine Frage bekommt genau ihre Zeilen und
nicht mehr; ein fremder Schlüssel öffnet nichts und ein gekipptes Bit
auch nicht; zwei Mitschnitte gleichen Inhalts tragen verschiedene Salze
und verschiedene Rahmen. Die Naht prüft, dass das Verdichten wirklich
ablegt und dass die Zusammenfassung darauf zeigt.

📌 **Zwei Gegenproben blieben zuerst stumm, und beide hatten recht.** Die
eine verglich zwei Dateien, die sich schon wegen des Datums
unterscheiden, statt der Salze; die andere fragte einen ganzen Abschnitt
ab, und dann ist die obere Zeilengrenze wirkungslos. **Eine Gegenprobe,
die nicht beißt, ist ein Befund**, und an diesem Tag war sie viermal der
einzige Weg zu einem Fehler, den kein Test und kein Lesen gefunden hätte.

⚠️ **Was noch fehlt: der Leseweg für den Agenten selbst.** Er braucht ein
eigenes Werkzeug, denn die Datei ist verschlüsselt und `read_file`
bekäme Bytes. Das ist ein Eingriff in die Datei, in der die
Einhängegrenze steht, und der gehört nicht nebenbei gemacht. Bis dahin
liest ein Mensch mit `myl verlauf`.

### v0.50.0 – 2026-09-16 (die Aktualisierung steht oben, und das Fenster bekommt ein zweites Bild)

`myl-client` **0.35.0**, `myl-oberflaeche` **0.38.0**, `myl-console`
**0.12.0**. Auftrag des Projektinhabers.

⚑ **Die Aktualisierung steht ganz oben**, über der Sprache. 📌 Bis
hierher stand sie darunter (Festlegung vom 2026-09-11, „gleich hinter
der Sprache"). Die Begründung damals war, dass die Sprache alles
beschriftet, was darunter kommt; sie trägt weiter, nur nicht gegen eine
**Handlung**. Die Aktualisierung ist keine Einstellung, und es ist die
Handlung, wegen der jemand diese Seite am ehesten öffnet.

⚑ **Ein helles Thema, und es dreht einen Kanal statt dreißig Regeln.**
Flächen, Kanten, der Glanz beim Überfahren und der Fokusring standen als
feste `rgba(255, 255, 255, …)` im Stilblatt; sie stehen jetzt als
`rgb(var(--auf) / …)` da. **Ein Thema ist damit eine Handvoll Zahlen und
kein zweites Stilblatt.** Die Graustufenregel gilt unverändert: Was
hervorgehoben wird, wird im hellen Bild **dunkler** statt heller, bunt
wird nichts.

⚑ **Die Lichtkante dreht ausdrücklich nicht mit.** Sie ist der Glanz
oben auf einer Fläche, und Licht kommt auch in einem hellen Bild von
oben und ist weiß. Ein dunkler Strich an derselben Stelle wäre kein
Glanz, sondern eine zweite Kante.

⚠️ **Ein drittes Thema „System" gibt es bewusst nicht.** Es wäre keine
dritte Gestaltung, sondern die Abtretung der Wahl an das
Betriebssystem, und CSS kann eine Palette nicht zwischen einem
Attributblock und einem `@media`-Block **teilen**: Sie stünde zweimal
da. ⛔️ **Der erste Anlauf hat sie zweimal hingeschrieben und daneben
behauptet, es gebe keine Wiederholung**, und die Gegenprobe blieb genau
deshalb stumm. Wer es will, löst im Skript auf und schreibt keine zweite
Palette.

⚑ **Eine Schriftgröße, drei Stufen.** Sie hängt an der Wurzel, und das
Stilblatt rechnet durchgehend in `rem`; damit wachsen Abstände und
Knöpfe im selben Verhältnis mit. 📌 **Die Grundschrift stand in Pixeln**
(`font: 14px/1.55`) und wäre an allem vorbeigegangen, was seine Größe
von dort erbt; sie steht jetzt als `.875rem` da.

⚑ **Wo ein Feld gilt, sagt ein Feld und nicht zwei Schalter.** Aus
`nur_konsole: bool` ist `gilt: Gilt` geworden, mit `Ueberall`,
`NurKonsole` und `NurFenster`. Zwei Wahrheitswerte ließen sich beide
setzen, und dann gäbe es ein Feld, das nirgends steht und überall wirkt:
**Ein Zustand, den es nicht geben darf, gehört nicht darstellbar.** Das
Erscheinungsbild und die Schriftgröße stehen deshalb nur im Fenster, das
Konsolen-Design nur in der Konsole, und keine der beiden Oberflächen
zählt selbst auf, was sie weglässt.

⚑ **Das Skript führt keine eigene Liste der Themen.** Geprüft wird im
Setzer der Kiste; was im Fenster ankommt, ist schon eine gültige
Kennung. Eine zweite Liste wäre die Stelle, an der ein drittes Bild
lautlos verlorenginge: Das Attribut stünde da, das Stilblatt kennte es
nicht, und es sähe aus wie die Vorgabe.

⚑ **Drei neue Prüfungen, und jede fängt eine Sorte Drift.** Jede
wählbare Kennung hat einen Block im Stilblatt (und die Vorgabe
ausdrücklich keinen); jede benutzte Stilvariable ist definiert, **und
jede Variable des hellen Themas gibt es auch in der Vorgabe** (ein
Tippfehler dort fällt sonst lautlos auf den dunklen Wert zurück); und in
**beiden** Themen hebt sich der Text vom Grund ab, gerechnet als
relative Leuchtdichte gegen 4,5 beziehungsweise 3,0.

### v0.49.0 – 2026-09-16 (ohne Grenze steht rechts, und das Rechenwerk kennt seinen Rechenweg)

`myl-client` **0.34.0**, `myl-oberflaeche` **0.37.0**, `myl-console`
**0.11.0**. Auftrag des Projektinhabers.

⚑ **Der offene Anschlag jedes Reglers liegt rechts, und dort steht er
ohne Zutun.** Kerne, Arbeitsspeicher, Platte und jedes Rechenwerk: Nicht
gesetzt heißt „ohne Grenze", und „ohne Grenze" ist die rechte
Endstellung. 📌 **Bis hierher lag sie ganz links, bei null.** Sie
bedeutete dasselbe und las sich wie das Gegenteil: Wer einen Regler am
linken Anschlag sieht, liest „nichts", nicht „alles". In der Konsole
stand dort das Wort „aus", und das liest sich wie abgeschaltet.

⚑ **Die beiden linken Enden sind verschieden, und darin steckt der
Unterschied zwischen einer Grenze und einem Anteil.** Eine Grenze fällt
bis auf **eins**: Null Kerne wären kein enger gestellter Klient, sondern
einer, der nicht antwortet. Ein Rechenwerk fällt bis auf **null** und
heißt dort „rechnet nicht", denn der Rechenpfad läuft dann auf der CPU
weiter. Ganz rechts wird in beiden Fällen die Grenze **weggenommen**,
was bei der Platte zugleich heißt: ohne Reservierung.

⛔️ **Fund 380: der Sperrgrund der Rechenwerke war seit dem 2026-09-14
falsch.** Jedes gefundene Gerät trug denselben Satz, „noch kein
Rechenweg über dieses Gerät", und seit dem 2026-09-14 rechnet `metal`
auf Apple-Silizium die gebündelten Matrizen der Vorbereitung wirklich.
⚑ **Eine Begründung gilt für den Fall, für den sie geschrieben wurde:**
Sie stammte aus dem 2026-09-10, als über kein Rechenwerk ein Rechenpfad
führte, und niemand ging die Liste durch, als einer dazukam.

⚑ **Jedes Gerät kennt jetzt seinen Rechenweg, und zwar einzeln.**
Apple-Silizium eingebaut wird von Metal bedient, eine NVIDIA-Karte von
CUDA, eine AMD-Karte von ROCm; die Herstellerkennung aus dem Gerät
entscheidet und nicht der Name, den ein Treiber setzt. **Damit tragen
zwei Karten verschiedener Hersteller im selben Rechner zwei Regler mit
zwei Rechenwegen.** Die Beschriftung nennt ihn („Apple M5 Pro (Metal)"),
und ob er hier auch **rechnet**, wird dort gefragt, wo der Code
ausgewählt wird. Ein Gerät ohne jeden Rechenweg, etwa eine eingebaute
Intel-Grafik, bekommt einen anderen Satz als eines, dessen Rechenweg
noch nicht gebaut ist: Der Unterschied ist der zwischen „noch nicht" und
„gar nicht", und er entscheidet, ob jemand darauf wartet.

⚑ **Was der Anteil heute wirklich tut, steht am Regler.** Ganz links
bleibt das Werk aus dem Rechenpfad, jeder Wert darüber lässt es rechnen,
und das wirkt sofort. Eine **Teilquote wirkt örtlich noch nicht** und
steht als Freigabe da: Ein Anteil an der Rechenzeit einer GPU braucht
einen Planer, den es nicht gibt. ⚑ **Ein Regler, der mehr verspricht,
als er hält, wäre schlimmer als ein gesperrter.**

⚑ **Kein Eintrag heißt ganz freigegeben.** 📌 Vorher war es umgekehrt:
kein Eintrag hieß null, eine ausdrückliche Null löschte. Das war die
vorsichtige Vorgabe aus der Zeit ohne Rechenpfad; seit Metal rechnet,
hielte sie eine GPU zurück, die der Nutzer gerade deshalb gekauft hat.

⛔️ **Fund 381: die Konsole wandte `kap.kerne` überhaupt nicht an.** Sie
ließ sich dort setzen, anzeigen und abspeichern, und gelesen hat sie
niemand: Der Rechenpfad fragte `available_parallelism` und nahm die
ganze Maschine. `myl` und das Fenster wandten sie an, `myelith` nicht.
⚑ **Eine Einstellung, die an einem von drei Bedieninstrumenten nichts
bewirkt, ist schlimmer als eine, die nirgends wirkt**, denn sie wirkt ja
anderswo, und deshalb sucht niemand den Unterschied im Programm.
Umgesetzt wird jetzt an **einer** Stelle in der Kiste, gerufen von allen
dreien.

⚑ **Die Einstellungsseite der Konsole trägt die Rechenwerke mit**, unter
derselben Überschrift wie die Kerne, mit ←→ wie jede andere Zeile. Die
Schrittweite kommt aus dem **Ende** und nicht aus dem Wert: Sonst
bewegte sich derselbe Regler unten in Einern und oben in Zehnern, und
der Weg zurück träfe nicht dieselben Stellungen wie der Weg hin. Ein
gesperrter Regler trägt keine Winkel und sagt auf einen Tastendruck, was
fehlt.

⚑ **Die Worte der beiden Endstellungen kommen aus der Kiste**, in der
eingestellten Sprache, und stehen weder im Fenster noch in der Konsole
als Zeichenkette. Fenster und Konsole zeigen denselben Regler; zwei
Stellen mit je eigenen Worten laufen auseinander, und das war schon
einmal der Befund (2026-09-10, die Sperrsätze standen nur auf Deutsch
da).

📌 **`myl einstellungen` zeigt jetzt jedes gefundene Rechenwerk**, nicht
nur die eingetragenen. Seit „kein Eintrag" ganz freigegeben heißt, wäre
das auf einer Maschine mit GPU eine leere Liste gewesen: **Eine Anzeige,
die den Normalfall verschweigt, zeigt nur die Ausnahme.**

📌 **Und der Datenort wird in der Kiste hergeleitet**, nicht im Fenster.
Die Konsole stellt dieselbe Frage, und zwei Herleitungen desselben Ortes
zeigen irgendwann auf zwei Datenträger.

### v0.48.4 – 2026-09-15 (eine Vorgabe, die nicht trägt, nimmt nicht alles mit)

`myl-client` **0.33.2**, `myl-oberflaeche` 0.36.3, `myl-console` 0.10.2.

⛔️ **Fund 378: eine nicht tragende Vorgabe warf das ganze Rüsten um.**
Bis hierher ging **jeder** untragbare Ordner als Fehler aus `ruesten`
heraus, auch der, den niemand eingestellt hat. Damit nahm eine nicht
auffindbare Vorgabe **alles** mit, auch die verankerten Werkzeuge, die
gar keinen Ordner brauchen: Ein Netzlauf hätte an einem umbenannten
`WORK_DIR` scheitern können.

⚑ **Der Unterschied ist die Absicht.** Ein **gesetzter** Pfad, den es
nicht gibt, bleibt ein Fehler: Wer ihn einstellt, meint ihn, und ein
stiller Rückfall ließe den Agenten in einem anderen Ordner arbeiten als
dem, der in den Einstellungen steht (dieselbe Begründung wie bei der
Wahl des Kistenordners). Die **Vorgabe** ist eine Bequemlichkeit; trägt
sie nicht, gibt es eben keine Dateiwerkzeuge, genau wie wenn sie gar
nicht da wäre.

📌 **Aufgefallen ist es in der CI unter Windows**, und die Ursache dort
war eine zweite: Zwei Prüfungen im **selben** Testprogramm hingen an
derselben Umgebungsvariable und liefen nebenläufig. Die eine zeigte auf
ihr Wegwerf-Verzeichnis und räumte es beim Verlassen weg, die andere
griff dazwischen zu, und der Ordner verschwand zwischen `canonicalize`
und `is_dir`. Zusammengelegt, wie schon bei der Prüfung in der
Bibliothek. ⚑ **Die Klemme ist trotzdem am Ort behoben und nicht nur
umgangen:** Ein Test, der eine Klemme umgeht, behebt sie nicht.

⚠️ **Der Rückfall hat keine eigene Prüfung**, und das steht im Code.
`standard_wurzel` prüft `is_dir()`, bevor sie einen Pfad herausgibt;
dorthin kommt also nur, was **zwischen** jener Prüfung und dem Einhängen
verschwindet, und dieses Zeitfenster lässt sich nicht absichtlich
treffen. Wer die Deckung für größer hält, als sie ist, verlässt sich auf
etwas, das niemand nachgesehen hat.

### v0.48.3 – 2026-09-15 (der Einhängepfad gehört dem Prozess, nicht dem Programm)

`myl-client` 0.33.1, `myl-oberflaeche` **0.36.3**, `myl-console` 0.10.2.

⚑ **Der Einhängepfad wird je Prozess gespeichert** (Auftrag des
Projektinhabers). Er liegt beim Gespräch im Speicher des Fensters, und
die Ordnerwahl in der Seitenleiste schreibt ihn dorthin.

⛔️ **Das war ein Fehler und nicht nur eine fehlende Bequemlichkeit.**
Vorher schrieb der Klick auf den Pfad `agent.wurzel` in die Ablage, also
**für alle Prozesse**: Wer für einen Auftrag einen anderen Ordner wählte,
fand ihn danach in jedem anderen Prozess wieder, und zurück kam er nur
über eine zweite Ordnerwahl. Genau so ist der Werkzeugkisten-Ordner in
einem Prozess als Einhängepfad aufgetaucht. **Der Ordner gehört dem
Auftrag, nicht dem Programm.**

⚑ **Drei Befehle bekommen ihn mit**, und das ist kein Zufall: die
Werkzeugliste, der Agentenlauf und **der Kontextzähler**. Die
Werkzeugansage hängt am eingehängten Ordner, ein Prozess ohne Ordner
bekommt weniger Werkzeuge als einer mit, und eine Kontextanzeige, die
eine andere Ansage zählt als die, die läuft, zählt falsch.

⚑ **Gesetzt wird auf einer Kopie der Einstellungen**, genau wie die
Konsole ihr Startverzeichnis setzt. Es in die Ablage zu schreiben hieße,
dass der nächste Prozess ihn erbt, und das ist das Gegenteil von „je
Prozess". Die Reihenfolge ist überall dieselbe: **Prozess, dann
Einstellung, dann die Vorgabe `WORK_DIR`.**

⚠️ **Der Platzhalter in den Einstellungen bleibt ausdrücklich davon frei.**
Er sagt, was bei leerem Feld gilt; den Ordner eines einzelnen Prozesses
dort zu zeigen hieße, eine Einstellung mit dem Zustand eines Auftrags zu
beschriften.

### v0.48.2 – 2026-09-15 (der Knopf sitzt in der Zelle des Feldes, und die Vorgabe ist zu sehen)

`myl-client` 0.33.1, `myl-oberflaeche` **0.36.2**, `myl-console` 0.10.2.

⚑ **Der Knopf „Werkzeuge anzeigen" steht jetzt in derselben Zelle wie
das Pfadfeld**, nicht in einer eigenen Zeile darunter (zweimal gemeldet
vom Projektinhaber).

📌 **Und der Grund für den zweiten Fehlgriff steckte in der Tabelle.**
Die linke Zelle der Kistenzeile trägt 350 Zeichen Erklärung. Sie macht
die **ganze Zeile** so hoch wie ihr Text; das Eingabefeld sitzt oben, und
was in der nächsten Zeile folgt, beginnt erst unter dem letzten Satz
links. **Eine Nachbarzeile steht nicht unter dem Feld, sondern unter der
höheren der beiden Spalten.** In derselben Zelle ist der Knopf davon
unabhängig, auch wenn der Erklärtext einmal länger wird.

⚑ **Das leere Feld „Arbeitsordner" nennt die Vorgabe als Platzhalter**
(Meldung des Projektinhabers: der Einhängepfad sei noch nicht
standardmäßig `WORK_DIR`). Er **war** es, seit es eine Vorgabe gibt; sie
war nur nirgends zu sehen, solange das Feld leer blieb. ⚠️ **Ein
gesetzter Pfad gewinnt weiter**, und das bleibt so: Wer einen Ordner
eingetragen hat, meint ihn. Wer zur Vorgabe zurück will, leert das Feld
(`myl setzen agent.wurzel aus`).

⛔️ **Und der Kopf log dabei.** `kurzform` fragte `a.wurzel.is_some()`,
also den **gespeicherten** Wert: Bei leerem Feld bekam der Agent seine
Werkzeuge, und der Kopf meldete „ohne Werkzeuge" samt der Aufforderung,
`agent.wurzel` zu setzen. Er rechnet die Vorgabe jetzt nach, mit
derselben Funktion wie die Rüstung. **Ein Kopf, der etwas anderes sagt
als das, was läuft, ist schlimmer als keiner.** Dieselbe Klasse wie die
Übersichtszeile in `myl einstellungen` einen Schritt zuvor.

### v0.48.1 – 2026-09-15 (der Werkzeugknopf steht unter dem Pfad, auf den er sich bezieht)

`myl-client` **0.33.1**, `myl-oberflaeche` **0.36.1**, `myl-console` 0.10.2.

⚑ **Der Knopf „Werkzeuge anzeigen" hängt an der Kistenzeile**, nicht am
Ende der Agentenrubrik (Meldung des Projektinhabers). Er beantwortet die
Frage, die der Pfad darüber aufwirft: welche Werkzeuge aus dieser Kiste
kommen. Zwei Zeilen weiter unten stand er neben etwas anderem.

📌 **Und in der richtigen Spalte.** Die Felderzeile ist zweispaltig,
links Beschriftung und Erklärung, rechts die Eingabe. Ein `colSpan = 2`
spannte über beide und setzte den Knopf ganz links unter den Erklärtext,
also unter die falsche Hälfte. Eine leere erste Zelle bringt ihn in
dieselbe Spalte wie das Feld.

⚑ **Die Kistennamen heißen jetzt überall `Base` und `Advanced`**, auch in
der Hilfe von `myl` und in den Doc-Kommentaren. Nach Fund 377 ist die
Schreibweise kein Stil, sondern ein Pfad; der Schalter `--werkzeuge`
nimmt weiterhin jede Schreibweise an.

📌 **Zwei Quellproben schlugen an ihrem eigenen Text fehl**, beide in
dieser Sitzung: eine an ihrem Doc-Kommentar, eine an dem Erklärtext, den
sie prüfen sollte. Sie lesen jetzt nur den Code beziehungsweise die
Zuweisung statt das Wort. **Eine Quellprobe, die sich selbst liest,
prüft etwas anderes als sie meint.**

### v0.48.0 – 2026-09-15 (ein Arbeitsordner, der etwas zu zeigen hat)

`myl-client` **0.33.0**, `myl-oberflaeche` 0.36.0, `myl-console`
**0.10.2**.

⚑ **`WORK_DIR` ist der Standard-Arbeitsordner** (Auftrag des
Projektinhabers). Er liegt im Wurzelverzeichnis des Projekts und bringt
fünf Beispieldateien mit: bekannte Zeilenzahlen, ein Merkmal, das in
genau zwei Dateien steht, ein Protokoll mit genau zwei Fehlerzeilen und
eine Vorlage mit vier Platzhaltern. Seine README nennt je Werkzeug eine
einfache, eine mittlere und eine komplexere Aufgabe samt erwarteter
Ausgabe, für alle vierzehn Werkzeuge. Vorher zeigte die
Vorgabe auf den CTF-Ordner: ein Prüfstand mit Aufgaben, kein
Arbeitsplatz.

⚑ **Gefunden wird er über die Projektwurzel**, nicht über einen Lauf
vom Arbeitsverzeichnis aufwärts. Das Fenster aus dem Finder hat als
Arbeitsverzeichnis `/`, und ein Lauf von dort findet nie ein Projekt;
dieselbe Ursache hatte der Fehler bei den Werkzeugkisten einen Tag
zuvor. ⚠️ **Der Konsolenclient behält sein Startverzeichnis**, und eine
Prüfung zählt die Rüstungen gegen die Stellen, die es setzen, damit eine
neue nicht still auf die Vorgabe zurückfällt.

⛔️ **Fund 377: ein zweiter, klein geschriebener Name für die
Werkzeugkisten.** `kennung()` gab `base` zurück, auf der Platte liegt
`Base`. Auf macOS fällt das nicht auf, weil das Dateisystem groß und
klein nicht unterscheidet; auf Linux hätte derselbe Aufruf ins Leere
gegriffen, ohne Fehlermeldung und ohne Manifestwerkzeuge. Die Funktion
hatte **null Aufrufer** und ist entfernt: Es gibt nur noch einen Namen.
Zwei neue Prüfungen vergleichen gegen den gelesenen Verzeichniseintrag
statt gegen `is_dir`, denn `is_dir` antwortet hier auf jede Schreibweise
mit „ja".

📌 **Zwei Prüfungen, eine Umgebungsvariable.** Die beiden Tests zur
Vorgabe waren einzeln grün und zusammen rot: Die Umgebung ist ein
Zustand für den ganzen Prozess, und sie liefen nebenläufig. Zusammengelegt
statt mit einem Riegel versehen, weil ein Riegel beim nächsten Mal
wieder vergessen wird.

### v0.47.0 – 2026-09-15 (die Werkzeugkiste ist ein Ordner, und zwar nur noch einer)

`myl-client` **0.32.0**, `myl-oberflaeche` **0.36.0**, `myl-console`
**0.10.1**.

⚑ **Die verankerte Werkzeugkiste**, erste Stufe. Bis hierher meldete der Client **jedes** Werkzeug als
extern und lokal an; in der Vorgabebetriebsart „nur verankert" sperrte
der Harness damit **alle**, ein Netzlauf hätte ein nachrechenbares
Modell gehabt und keine Werkzeuge. Neu sind `fill_template` und
`join_sections`: Sie rechnen aus ihren Eingaben und sonst nichts, tragen
deshalb `Deterministisch`/`Verankert` und kommen durch.

⚑ **Sie erzeugen Inhalt und bewirken nichts.** Nachrechenbar ist, wie
aus Eingaben ein Text wird; eine Datei zu schreiben ist es nicht, denn
wo sie landet, hängt an einer Maschine, die niemand sonst hat. Deshalb
nehmen sie **keinen Pfad** entgegen, sondern Inhalt, und brauchen kein
eingehängtes Verzeichnis: Ein Auftrag „schreibe mir ein Dokument"
funktioniert damit auch im Chat.

⚠️ **Der Vertrag ist teurer als der Code.** Zeilenenden werden auf `\n`
vereinheitlicht, es wird nicht sortiert, nichts gross- oder
kleingeschrieben und nicht in Gleitkomma gerechnet; gezählt wird in
Unicode-Skalarwerten. Eine Fassungsnummer gilt für die ganze Kiste und
steht in der Revision jedes Manifests, geht also in die Adresse ein.
Neun Konformitätsvektoren halten Eingabe und Ausgabe byteweise fest:
**ohne Vektor keine Marke.**

⛔️ **Und im `manual mode` sah das Modell die Werkzeugkiste gar nicht.**
Das Fenster hatte keinen Bestätigungskasten und setzte deshalb in diesem
Modus `schreiben = false`. Das nahm nicht nur `write_file`, `edit_file`
und `run_command` weg, sondern **jedes Kisten-Werkzeug**: Ein Manifest
läuft über die Shell, gilt damit als schreibend und fiel mit. Wer seine
Kiste füllte, sah davon nichts; gemeldet wurde es als „das Modell führt
die alten Werkzeuge auf". Jetzt fragt das Fenster wirklich, im Kasten
des Betriebssystems, mit Werkzeugnamen und Argumenten. **Eine
Zustimmung ohne zu wissen, worauf, ist keine.**

⚑ **Ein Gespräch bringt sein Modell mit** (Auftrag des
Projektinhabers). Womit ein Gespräch geführt wurde, bleibt an ihm
hängen; wer es aufschlägt, bekommt dieses Modell **eingestellt, aber
nicht geladen**. Geladen wird beim ersten Auftrag wie sonst auch: Wer
ein altes Gespräch nur nachliest, soll nicht minutenlang auf ein
Artefakt warten. 📌 Das vorher geladene wird dabei entladen, sonst führe
der nächste Auftrag damit weiter, während die Anzeige das neue nennt.
Steht schon dasselbe Modell, geschieht nichts.

📌 **Die Kistenwahl ging am falschen Ort auf.** Sie bekam den
eingestellten Kistenordner als Startort, und der ist per Vorgabe leer;
der Dialog öffnete deshalb dort, wo zuletzt etwas gewählt wurde, also
beim Einhängepfad. Jetzt nennt die Kiste ihren eigenen Ort, und die Wahl
zeigt die Kisten nebeneinander.


⚑ **Eine Einstellung statt zweier** (Festlegung des Projektinhabers).
Bis heute standen nebeneinander eine Auswahl `agent.werkzeuge`
(automatisch, Base, Advanced, 1337) und ein Pfad. **Zwei Angaben für
dieselbe Sache laufen auseinander**, und genau das taten sie: Die
Manifeste kamen aus dem Ordner, die eingebauten Werkzeuge aus der
Auswahl. Geblieben ist der Pfad, beschriftet **„Werkzeugkiste"**. Sein
letzter Namensteil sagt, welche eingebauten Werkzeuge dazukommen:
`base` die fünf Dateiwerkzeuge, `advanced` zusätzlich `run_command`, ein
anderer Name `base`. Ohne Angabe die mitgelieferte Kiste `base`.

📌 **Und derselbe Bruch steckte im Bedieninstrument.** `myl agent` nahm
ohne `--werkzeuge` immer `Base` und sah die Einstellung gar nicht an;
wer seine Kiste auf `advanced` stellte, bekam die Manifeste aus dem
Ordner und die eingebauten aus einer anderen Quelle. Jetzt ist die
Einstellung die Vorgabe und der Schalter überstimmt sie für einen
einzelnen Lauf.

⚑ **Die Adminmarke für `1337` ist entfallen.** Sie war gegenstandslos:
Der Ordner ist gitignored, wer ihn nicht hat, hat die Werkzeuge nicht,
und wer ihn anlegt, hat die Entscheidung getroffen.

⚠️ **Ein gesetzter Pfad, den es nicht gibt, fällt nicht still auf die
Vorgabe zurück.** Sonst arbeitete der Agent aus einem anderen Ordner als
dem, der in den Einstellungen steht, und niemand sähe es.

### v0.46.0 – 2026-09-15 (die Angabe ist der Knopf, und die Werkzeugliste zieht in die Einstellungen)

`myl-client` **0.31.0**, `myl-oberflaeche` **0.35.0**, `myl-console`
**0.10.0**.

📌 **Neben Einhängepfad und Werkzeugkiste stand je ein beschrifteter
Knopf, und die Seitenleiste ist dafür zu schmal:** „Verzeichnis
wechseln" und „andere Werkzeugkiste" wurden beide abgeschnitten. Statt
die Beschriftung zu kürzen, bis sie nichts mehr sagt, trägt jetzt **die
Angabe selbst die Handlung**: Ein Klick auf den Pfad wählt einen anderen,
ein Klick auf den Kistennamen schaltet weiter. Was der Klick tut, steht
beim Zeigen darauf.

⚑ **Es bleibt ein `button`** und wird kein anklickbarer Absatz. Der
Unterschied ist nicht sichtbar und entscheidet darüber, ob die Handlung
mit der Tastatur erreichbar ist und vorgelesen wird.

⚑ **Die Werkzeugliste steht nicht mehr in der Leiste**, sondern in den
Einstellungen unter „Agent" hinter dem Knopf **„Werkzeuge anzeigen"**.
In der Leiste war sie eine Wand aus Namen, die bei jedem Zeichnen
mitlief und die zwei Angaben darüber erschlug. Gefragt wird erst beim
Klick: Der Befehl baut die Einhängung wirklich, und das bei jedem Öffnen
der Einstellungen zu tun wäre Arbeit für eine Angabe, die selten jemand
sehen will.

### v0.45.0 – 2026-09-14 (die Werkzeugkisten sind Ordner, der Agent hat einen Spielplatz, und das Fenster ist aufgeräumt)

`myl-client` **0.31.0**, `myl-oberflaeche` **0.34.0**, `myl-console`
**0.10.0**.

⚑ **Die Werkzeugkisten sind Ordner** (`kisten.rs`): Was als Manifest
(JSON mit `name`, `beschreibung`, `parameter`, `befehl`) in
`CLIENT/werkzeugkisten/<name>` liegt, sieht das Modell ohne Neubau. Ein
Manifest-Werkzeug läuft über `sh -c` wie `run_command` (Schreibrecht-Gate,
shell-sicher eingesetzte Argumente, `manual mode`); die eingebauten
Dateiwerkzeuge bleiben kompiliert und halten die Einhängegrenze. Gefunden
wird der Ordner über `MYL_WERKZEUGKISTEN` oder die Suche im Baum. In
`Base` liegen vier Manifestwerkzeuge neben den fünf eingebauten, in
`Advanced` zusätzlich zwei und `run_command`; `1337` bleibt gitignored.
**`Advanced` erbt `Base` durch die Kette und nicht durch Kopien**, denn
zwei Kopien laufen auseinander.

⛔️ **Die Ordnernamen werden groß geschrieben, und das ist keine
Schreibweise, sondern ein Pfad.** macOS unterscheidet in Dateinamen
nicht zwischen groß und klein, Linux schon: Ein Name, der hier
funktioniert, kann dort ins Leere gehen, ohne dass jemand eine
Fehlermeldung sieht. Eine Prüfung vergleicht die Namen deshalb gegen den
gelesenen Verzeichniseintrag statt gegen `is_dir`.

⚑ **Ohne gesetzten Arbeitsordner ist `WORK_DIR` die Vorgabe**, der
Ordner im Wurzelverzeichnis des Projekts (oder `MYL_ARBEITSORDNER`): Er
bringt Beispieldateien mit, an denen sich jedes Werkzeug zeigt, und eine
README, die je Werkzeug eine einfache, eine mittlere und eine
komplexere Aufgabe mit der erwarteten Ausgabe nennt. Wer nichts gesetzt
hat und den Ordner auch nicht findet, bekommt wie bisher keine
Dateiwerkzeuge.

⚠️ **Der Konsolenclient nimmt weiter das Verzeichnis, aus dem er
gestartet wurde**, und nicht diese Vorgabe: Wer ihn in einem Projekt
aufruft, will darin arbeiten.

⚑ **Die Seitenleiste im Agentenmodus** zeigt „Einhängepfad:" und
„Werkzeugkiste:", und der Pfad und der Name **sind** die Knöpfe: Ein
Klick darauf wechselt Verzeichnis oder Kiste, ohne den Umweg über die
Einstellungen. Die eigenen Knöpfe daneben sind entfallen, sie wurden in
der Leiste abgeschnitten. Die Werkzeugliste steht nicht mehr in der
Leiste, sondern hinter „Werkzeuge anzeigen" in den Einstellungen.

⚑ **Das Fenster, aufgeräumt:** Der Kontextbalken sitzt links unter der
Eingabe und lässt rechts Platz für weitere Anzeigen; er färbt sich über
90 % rot und nach dem Verdichten grün mit dem erreichten Stand (die
einzige Farbe im sonst grauen Fenster, eigens mit `farbampel`
gekennzeichnet und von der Graustufenprobe ausgenommen). Das `</think>`
erscheint nur noch im Aufklapp-Knopf, nicht mehr vor der Antwort. Die
redundante Pfadzeile unter dem Balken ist weg; Pfad und Ladezustand
stehen in der Modellzeile der Leiste.

📌 **Der gewählte Modus ist in der Leiste zu sehen.** Das Stilblatt hatte
die Regel dafür seit langem; `modi_zeichnen` verglich aber mit dem Modus
des **offenen** Gesprächs. Seit der Modus vom Gespräch abgelöst ist
(v0.36.0: ein Wechsel legt nichts mehr an), ist danach nichts offen, der
Vergleich also immer falsch, und kein Knopf rastete ein. Verglichen wird
jetzt mit dem eingerasteten Modus; dazu eine Marke am linken Rand, damit
der Unterschied nicht nur aus Hintergrund und Kante besteht. **Dieselbe
Angabe an zwei Orten, und die zweite meldete sich nicht.**

### v0.44.0 – 2026-09-14 (das Gespräch trägt über Aufträge, der Kontext ist sichtbar und verdichtbar)

`myl-client` **0.30.0**, `myl-oberflaeche` **0.33.0**, `myl-console`
**0.10.0**.

⚑ **Der Agent wird nicht mehr mit jedem Schritt langsamer** (Fund 372):
`Oertlichesmodell` hält einen `Fortsetzung`-Speicher, ein Schritt rechnet
nur den neuen Teil des Gesprächs. Am 4B fiel ein Lauf mit acht Schritten
von 116 auf 58 s.

⚑ **Das Gespräch geht über Aufträge mit, auch beim Agenten** (Entscheidung
C2, beantwortet): `gespraech::Gespraech`, und wird der Kontext voll, fasst
das Modell den Verlauf selbst zusammen (`verdichten`), statt den Anfang zu
vergessen.

⚑ **Der Kontext ist sichtbar:** in der Konsole mit `/context`,
`/compress`, `/clear` und in der Fußzeile; im Fenster als Balken am
Eingabefeld, ein Klick verdichtet oder beginnt neu.

⚑ **Befehle und Nachdenken gebündelt** (Auftrag des Projektinhabers):
„X Mal nachgedacht" als ein fortgesetzter Faden mit Live-Token, „X Befehle
ausgeführt" mit einer Liste, in der jeder Befehl den genauen Aufruf und
die Antwort aufklappt; steht die Antwort noch aus, sagt die Stelle das.
Dafür kommen Befehl und Werkzeugantwort vollständig an (bis 4 000
Zeichen) statt gekürzt.

⛔️ **Fund 368:** Ein Prompt über der Kontextgrenze des Modells wird als
`Tuerfehler::KontextVoll` abgelehnt, statt still umzubrechen.

⚑ **Erstes neues Agentenwerkzeug: `run_command`** (Wunschliste A0). Ein
Shell-Befehl im Arbeitsverzeichnis (`sh -c`), mit Zeitgrenze (30 s),
Ausgabegrenze (16 KiB), Schreibrecht-Gate, `manual mode` und einer
Sperrliste als Rückfall. ⛔️ **In `Advanced`, nicht `Base`:** Ein
Shell-Befehl hält die Einhaengegrenze nicht ein, die jedes Dateiwerkzeug
einhält; damit bedeuten `Base` und `Advanced` zum ersten Mal
Verschiedenes. Die echte Grenze wäre ein Betriebssystem-Sandbox und
bleibt eigene Arbeit.

### v0.43.0 – 2026-09-14 (der Client rechnet auf dem schnellen Weg, und auf dem Mac mit der GPU)

`myl-client` **0.28.0 auf 0.29.0**; Oberfläche und Konsole bauen über
ihn mit und bleiben auf ihrer Nummer.

⛔️ **Fund 367: Der Client rechnete auf dem Referenzpfad.**
`myl-client` band die Laufzeit ohne jedes Feature ein, und damit
Konsole und Oberfläche. Auf Apple-Silizium kostet das 25 bis 35 %
Durchsatz. **Jetzt:** `cpu-simd` auf jedem Ziel (ohne NEON wirkungslos,
dann rechnet die Referenz wie vorher), auf macOS zusätzlich `metal`.
Beide rechnen bitgleich zur Referenz.

⚑ **Die Vorbereitung eines Prompts ist damit ein Vielfaches schneller**,
zusammen mit der gebündelten Vorbereitung in der Laufzeit (INTEGER_LLM
v0.70.0): 219 Token beim 0,6B **5,21 s vorher, 0,28 s jetzt**, beim 4B
0,81 s. Der Decode bleibt auf der CPU.

⚑ **Die GPU wird geprüft, bevor sie rechnet.** Weicht sie beim ersten
Gebrauch von der CPU ab oder fehlt sie, rechnet der Client ohne sie
weiter und schreibt den Grund auf die Fehlerausgabe.

### v0.42.1 – 2026-09-12 (zwei Prüfungen fielen erst im Lauf der Werkstatt auf)

`myl-oberflaeche` **0.32.0** unverändert, nur die Bündeldatei und eine
Prüfung.

**Beide Fehler standen schon in der vorigen Fassung und wurden dort
nicht bemerkt.** Das ist der eigentliche Befund: Die Prüfungen selbst
haben sauber angeschlagen, gesehen hat es niemand, weil die
Zusammenfassung des Laufs eine gescheiterte Zeile nicht lesen konnte.

📌 **Fund 350: die Bündeldatei blieb auf der alten Zahl.**
`Cargo.toml` stand auf `0.32.0`, `tauri.conf.json` noch auf `0.31.0`.
Der Dateiname jedes Freigabebündels kommt aus der zweiten Zahl, ein
Bündel aus dieser Fassung hätte also `0.31.0` geheissen und wäre von
der Fassung davor nicht zu unterscheiden gewesen.

⚑ **Die Prüfung dafür gibt es seit Langem** und sie hat auch
angeschlagen; es fehlte nichts als der Blick darauf.

📌 **Fund 351: eine Prüfung hing an der jeweiligen Modellzahl.**
`jede_lizenz_steht_bei_ihrer_sache` verlangte mindestens vier
Katalogeinträge. Diese Vier war keine Aussage über den Katalog,
sondern der Stand des Tages, an dem sie geschrieben wurde. Mit dem
Wegfall des dichten 14B blieben drei, und die Prüfung meldete „zu
wenige Katalogeinträge" für einen vollständigen Katalog.

⚠️ **Ganz streichen liess sich die Schranke nicht.** Sie hält die
Zählung darunter davon ab, bei **null** Einträgen `0 == 0` zu ergeben
und damit wahr zu sein, ohne etwas geprüft zu haben. Sie lautet jetzt
„mehr als null" und wird zusätzlich gegen die Zahl der Einträge in
`REGISTER.json` gehalten: **eine Schranke, die mitwächst, statt eine,
die altert.**

### v0.42.0 – 2026-09-12 (das Logo bleibt, der Wagen sitzt richtig, und die Leiste sortiert sich)

`myl-client` **0.27.0 auf 0.28.0**, `myl-console` **0.8.0 auf 0.9.0**,
`myl-oberflaeche` **0.31.0 auf 0.32.0**.

**Drei Anpassungen auf Auftrag des Projektinhabers.**

⚑ **Das Logo bleibt nach der Modellwahl stehen.** Bisher räumte der
Konsolen-Client den Schirm und das Gespräch begann auf einer leeren
Fläche. Jetzt steht der Schriftzug oben und **wandert mit dem Gespräch
nach oben weg wie jede andere Zeile**: einmal gezeichnet, danach nicht
mehr angefasst.

⚠️ **Kein fester Kopf.** Ein Logo, das oben kleben bliebe, bräuchte
einen zweiten Rollbereich und nähme dem Gespräch dauerhaft acht Zeilen.
Der untere Rand ist reserviert, weil dort die Eingabe steht; oben ist
Platz wertvoller als Zierrat.

⚑ **Und der Rückweg aus den Einstellungen führt ebenso unter das
Logo.** Zwei Wege in dasselbe Bild dürfen nicht verschieden aussehen.

📌 **Fund 347: der blinkende Wagen sass eine Zeile unter der Eingabe.**
`Schirm::zeile` schreibt die ANSI-Sequenz `ESC[{n};1H`, und die zählt
**ab eins**; `crossterm::cursor::MoveTo` zählt **ab null**. Dieselbe
Zahl in beide gegeben ergibt zwei verschiedene Zeilen.

**Der Fehler war unsichtbar, solange niemand hinsah:** Der Text stand
richtig, nur der Wagen blinkte in der Kantenzeile darunter. Gemeldet
vom Projektinhaber, nicht von einer Prüfung, und das ist kein Zufall:
**Ein Test, der Steuersequenzen liest, sieht die Zeile, in der etwas
steht, und nicht die, in der etwas blinkt.**

Die Umrechnung steht jetzt als `Schirm::wagenzeile` an **einer** Stelle
statt von Hand an der Aufrufstelle, und eine Gegenprobe hält beide
Zählweisen aneinander.

⚑ **Im Fenster wandert ein berührtes Gespräch in der Leiste nach
oben.** Bisher stand sie in der Reihenfolge der **Anlage**: Ein
Gespräch, das man seit Wochen führt, rutschte mit jedem neuen weiter
nach unten, bis man es suchen musste.

⚠️ **Ausgelöst vom Senden, nicht vom Öffnen.** Wer die Leiste
durchsieht, um etwas wiederzufinden, würde sie sonst beim Lesen
umsortieren, und die Zeile, auf die er als Nächstes klicken wollte,
wäre weggerutscht. **Umordnen ist eine Folge von Arbeit, nicht von
Hinsehen.**

⚑ **`wann` wird dabei nicht angefasst.** Es steht im Markdown-Export
als „Begonnen", also als Aussage über den Anfang; fortgeschrieben
hiesse es stillschweigend „zuletzt benutzt". Die Reihenfolge überlebt
einen Neustart ohnehin, weil die Ablage das Feld als Ganzes sichert.

⛔️ **Und das dichte 14B ist aus der Modellwahl heraus** (Festlegung des
Projektinhabers). Wer auf `myelith-14b` oder `myelith-7b` zeigte,
landet jetzt beim **4B**: dem grössten verbliebenen dichten Modell, und
es verlangt 5 GB Platte statt 32.

### v0.41.0 – 2026-09-11 (die Modellwahl steigt an, nennt ihre Hardware und kennt das Netz je Modell)

`myl-client` **0.26.0 auf 0.27.0**, `myl-oberflaeche` **0.30.0 auf 0.31.0**,
`myl-console` **0.7.0 auf 0.8.0**.

**Drei Auftraege des Projektinhabers**, alle an derselben Stelle:

⚑ **Die Wahl steht aufsteigend**, nach Parametern und nicht nach
Verzeichnisnamen. ⚠️ Nach Zeichenketten sortiert stuende `Myelith 14B`
**vor** `Myelith 4B`, weil `1` vor `4` kommt; die Ordnungszahl steht
deshalb als Zahl im Katalog (`reihung`).

⚑ **Jedes Modell nennt seine Mindestausstattung**, kleingedruckt und
an drei Stellen aus **einer** Quelle: unter der Wahl im Fenster, unter
dem Eintrag in der Konsole und in der Einstellungszeile. Zwei Zahlen,
Arbeitsspeicher und Platte; die eine ist eine harte Bedingung, die
andere die Groesse, ab der es fluessig laeuft.

⚑ **Statt eines Sammeleintrags „Netzwerkmodell" steht jedes Modell
auch als „(API), kostet Inferenz-Credits" da**, gesperrt bis die
Knoten stehen. Der Grund ist die Bauart des Netzes: Wer ein kleines
Modell haelt, soll es anbieten koennen, ohne es zu sharden. **Das
kleinste Redundanzpaar sind dann zwei einzelne Rechner.** Das
Kennzeichen ist deshalb ein Praefix `netz:<Artefakt>` und kein
einzelner Wert mehr.

📌 **Zwei Mangel nebenbei gefunden und behoben.** Der Rueckfalleintrag
„(eingestellt)" bekam einen API-Zwilling, obwohl er auf nichts zeigt.
Und die Wahl verglich den eingestellten Pfad **unaufgeloest** gegen
die absoluten Pfade der Liste: Ein relativ eingestelltes Artefakt
stand deshalb **zweimal** in der Wahl.

⚠️ **Die Ablage wandert ein zweites Mal.** Wer auf `myelith-0.5b` oder
`myelith-7b` zeigte, landet auf `myelith-0.6b` beziehungsweise
`myelith-14b`. **Das ist kein Ersatz, sondern die naechstgelegene
Wahl**: ein anderes Modell mit anderen Antworten. Die Alternative
waere ein Pfad ins Leere.

### v0.40.0 – 2026-09-11 (Zahlen lassen sich tippen, und die Kürzel stehen unten)

**Zwei Nachträge des Projektinhabers zur Einstellungsseite der Konsole.**

⚑ **Zahlen und Grenzen nehmen auch eine Eingabe.** Enter öffnet eine
Zeile, der bisherige Wert steht in der Klammer daneben, Enter bestätigt.
**Ein Pfeil ist zum Nachstellen da und nicht zum Eingeben:** Wer von 600
auf 4000 will, drückt sonst vierunddreissig Mal.

⚑ **Die Tastenkürzel stehen in der untersten Zeile:** `↑↓ waehlen · ←→
aendern · ⏎ eingeben · ^S sichern · ^R Feld zuruecksetzen · Esc
schliessen`. `^S` sichert, ohne zu schliessen; `Esc` sichert mit.
⚠️ **`^R` setzt das gewählte Feld zurück und nicht die ganze Ablage**,
und die Zeile unten sagt genau das.

📌 **Zwei Gegenproben:** Eine hält die Kürzelzeile und den Tastenzweig
zusammen (dieselbe Klasse wie Fund 271), die andere lässt den Setzer
jede Vorgabe annehmen.

`myl-console` **0.6.0 auf 0.7.0** (95 Prüfungen).

### v0.39.0 – 2026-09-11 (die Einstellungen der Konsole lassen sich mit den Pfeilen bedienen)

**Auftrag des Projektinhabers:** `/settings` soll sich, wo es geht, mit
den Pfeiltasten bedienen lassen.

⚑ **Hoch und runter wählt, links und rechts ändert.** Eine Auswahl, ein
Schalter, eine Zahl und eine Grenze kommen aus einer festen Menge; was
freien Text braucht, bekommt ihn auf Enter. **Die Winkel `‹ ›` stehen
nur dort, wo sie etwas bedeuten.**

📌 **Damit fällt eine frühere Festlegung, und zwar begründet.** Bis
hierher zeigte `/settings` nur an, weil ein zweiter Setzer die dritte
Stelle wäre, die dieselben Feinheiten kennt. **Die Begründung galt dem
Setzer und nicht der Bedienung:** Gesetzt wird weiterhin ausschliesslich
über die Kiste, und diese Seite entscheidet nur, welche Taste welchen
Wert vorschlägt. Die Gegenprobe dazu läuft jedes Feld zwölf Mal in beide
Richtungen durch und lässt den Setzer jeden Vorschlag annehmen.

⚑ **Der Schritt einer Zahl ist abgeleitet und nicht getippt:** eine
Stelle unter der höchsten, mindestens eins. ⚑ **Unter dem kleinsten Wert
einer Grenze steht `aus`**, und von dort führt der Pfeil wieder hinein:
Das ist der Sinn einer Grenze.

⚑ **Gezeigt wird die Beschriftung und nicht die Kennung** („Deutsch"
statt `de`), und `aus` steht nur da, wo es `aus` heisst: Ein nicht
gesetzter Ordner ist nicht abgeschaltet, er ist leer.

⚑ **Was dort geändert wird, gilt sofort.** Modus, Design,
Schreiberlaubnis und Werkzeugkiste werden nach dem Verlassen neu
gelesen.

`myl-console` **0.5.0 auf 0.6.0** (92 Prüfungen).

### v0.38.0 – 2026-09-11 (ein verdeckter Name, zwei Überschriftsgrössen zu viel, und die Welle)

**Neun Meldungen des Projektinhabers**, und die erste war der Fehler,
der das Fenster unbrauchbar machte.

📌 **Fund 324: `Cannot access 't' before initialization`, und zwar genau
dann, wenn ein Lauf läuft.** Zwei Funktionen hielten ein Element in
`const t`; die Übersetzung heisst auch so. **Eine lokale Bindung
verdeckt den äusseren Namen im ganzen Block, auch vor ihrer eigenen
Zeile.** Die eine Stelle rief die Übersetzung oberhalb ihrer
Deklaration, und es war die Zeile, die das Laufzeichen baut: Deshalb
erschien weder der Ladeindikator noch eine Antwort, bei jedem Modell und
in jeder Einstellung. **Eine Prüfung geht jetzt jede Zeile durch:** Ein
Name, den es nur einmal gibt, kann gar nicht erst verdeckt werden.

📌 **Fund 325: drei Überschriftsgrössen, und die grösste war die
falsche.** `h2` hatte keine Angabe und nahm die Vorgabe des Browsers,
also mehr als das `h1` der Seite. Jetzt zwei Stufen: „Einstellungen"
gross, alles darunter gleichrangig, gleich gross, jedes mit einer Linie
darunter.

⚑ **Das Konsolen-Design steht nicht mehr im Fenster**, denn dort
bewirkt es nichts. **Eine Einstellung, die an der Stelle, an der sie
steht, nichts bewirkt, ist schlimmer als eine fehlende.**

⚑ **Die englische Übersetzung ist vollständig**, nachgerechnet: 64
Sätze in beiden Tabellen, 55 Sprachpaare im Feldkatalog, kein Umlaut im
englischen Text, kein unübersetzter Satz.

⚑ **In der Konsole steht die Ladezeile jetzt fest über dem Kasten**,
mittig, mit einer leeren Zeile Abstand. In der Klammer stehen auch die
Token: `(1m 19s · ↑588 ↓8 tokens · thinking)`; gezählt wird im Modell
und nicht geschätzt. ⚑ **Das beantwortet nebenbei eine Frage, die vorher
niemand stellen konnte:** Beim 4B-Modell vergehen achtzig Sekunden,
**bevor das erste Token fällt**, und das ist die Vorbereitung des
Prompts und kein Hänger.

⚑ **Die Farbe läuft als Welle durch die Buchstaben**, jedes Zeichen 400
ms nach dem davor, und nur der Ladetext selbst: Zeit, Token und
Tätigkeit sind Angaben und keine Zierde. Der Schalter für die
ausführliche Anzeige heisst `^S`, und in ihr läuft der Denkvorgang
**live durch, Token für Token**.

⚑ **Weniger Vorspann:** Der Untertitel unter der Marke ist weg, und nach
dem Laden wird aufgeräumt. ⚑ Beide Kopien des Banners haben dieselbe
Änderung bekommen, nur die Konstante unterscheidet sich: **Die
Vergleichsprüfung läuft unverändert weiter.**

⚑ **Und die Fusszeile sagt, wenn nicht geschrieben werden darf.** 📌
Dieselbe Klasse wie Fund 315: Ohne `agent.schreiben` bekommt der Agent
`write_file` gar nicht erst und antwortet „ich kann keine Dateien
speichern"; **wer das nicht weiss, sucht den Fehler beim Modell.**

⛔️ **Fund 326: `git checkout` auf eine Datei mit unversionierter
Arbeit** nahm einen Tagesstand zurück. **Eine Gegenprobe wird aus einer
Kopie zurückgeholt, nie aus `git`**, solange nichts eingecheckt ist.

`myl-client` **0.25.0 auf 0.26.0** (165 Prüfungen), `myl-console`
**0.4.0 auf 0.5.0** (88 Prüfungen), `myl-oberflaeche` **0.29.1 auf
0.30.0** (51 Prüfungen).

### v0.37.0 – 2026-09-11 (der Start fragt zweimal, die Zeitleiste bleibt stehen, und die Farbe wandert)

**Sieben Meldungen und Aufträge des Projektinhabers**, und die erste war
ein Fehler.

📌 **Fund 320: Abschicken sah aus wie nichts tun.** Der Text blieb im
Eingabekasten stehen, während das Modell schon rechnete, und in der
Ausgabe erschien er nie. **Ein Gespräch, in dem nur eine Seite dasteht,
lässt sich hinterher nicht lesen.** Die Zeile wird jetzt beim Abschicken
aus dem Kasten geräumt und als `❯ …` in die Zeitleiste geschrieben.

📌 **Fund 321: Die Anzeige hielt zurück, was geschah.** Werkzeugaufrufe
standen nur in ihrem Gedächtnis und wurden erst gedruckt, wenn jemand
den Schalter drückte; **damit fehlte in der Zeitleiste genau das, was
vor der Antwort geschehen war.** Jetzt steht jede Zeile da, sobald sie
entsteht, und der Schalter entscheidet nur noch, **wie ausführlich** die
nächsten sind. Er heisst `^O`: `^T` ist auf macOS `SIGINFO` und in
mehreren Terminals belegt.

⚑ **Der Start fragt zweimal, beides mit den Pfeiltasten:** erst das
Design, dann das Modell. Die Designwahl färbt die Modellwahl schon mit.

⚑ **Fünf Designs:** `Standard` (die Farben deines Terminals,
unverändert), `Myelith` (das bisherige Bild), `Bernstein`, `Tiefsee`,
`Tinte`. **`Standard` ist kein Design, sondern die Abwesenheit eines:**
Wer sein Farbschema eingestellt hat, hat damit schon gewählt.
Voreingestellt ist `oberflaeche.design`; die Wahl beim Start ändert die
Ablage nicht.

⚑ **Die Modellwahl zeigt alles, was da ist**, mit den Anzeigenamen aus
dem Katalog, und den Netzeintrag gesperrt samt Grund. **Ein gesperrter
Eintrag ist etwas anderes als ein fehlender.** Nach dem Laden steht da:
`Modell Myelith 4B (<Pfad>) wurde geladen.`

📌 **Fund 322: Zwei Listen für dieselbe Frage.** Das Fenster hatte die
Modellliste mit Katalognamen, die Konsole las das Verzeichnis ab. Sie
liegt jetzt in der Kiste, und beide rufen sie.

📌 **Fund 323: Eine Zeile, die umbricht, verschiebt die ganze Liste.**
Die Auswahl springt beim Neuzeichnen um ihre eigene Höhe zurück; eine
umgebrochene Zeile zählt dort als eine und belegt zwei.

⚑ **Der Kopfblock beim Start ist entfallen** (drei Zeilen). ⚠️ **Der
Arbeitsordner durfte nicht mit verschwinden:** Ein Agent mit
Dateiwerkzeugen arbeitet genau dort. Er steht jetzt dauerhaft in der
zweiten Zeile unter dem Eingabekasten.

⚑ **Die Farbe des Ladetextes wandert linear durch den Regenbogen**, ein
voller Durchlauf in zwei Minuten, ganzzahlig gerechnet. In den
einfarbigen Designs pulst stattdessen die Helle derselben Farbe. Dazu
sechs Drehscheiben, je Schritt eine andere: **Die Zeile wechselt nicht
nur den Text, sondern auch ihre Bewegung.**

`myl-client` **0.24.0 auf 0.25.0** (165 Prüfungen), `myl-console`
**0.3.0 auf 0.4.0** (85 Prüfungen), `myl-oberflaeche` **0.29.0 auf
0.29.1** (49 Prüfungen).

### v0.36.0 – 2026-09-11 (die Eingabe steht unten fest, der Agent hat zwei Modi, und die Kisten haben Namen)

**Fünf Aufträge des Projektinhabers**, alle am selben Abend, und sie
gehören zusammen: **Der Konsolenclient soll sich bedienen lassen wie ein
Programm und nicht wie ein Protokoll.**

⚑ **Der Rahmen steht fest am unteren Rand.** Das Terminal bekommt einen
Rollbereich; die Ausgabe rollt darüber weg, die vier Zeilen darunter
bleiben stehen. ⚠️ **Was reserviert wird, wird zurückgegeben**, auch bei
Strg-C: Ein Programm, das mit gesetztem Rollbereich endet, hinterlässt
eine Shell, die nur noch im oberen Teil des Fensters schreibt.

📌 **Fund 316: Die Eingabezeile war ein Zeichen schmaler als der
Rahmen**, das schliessende Leerzeichen fehlte. **Im Quelltext ist das
unsichtbar und im Bild sofort da.**

⚑ **Während eines Laufs steht eine Zeile da, und sie sagt drei Dinge:**
einen Ladetext, der sich bewegt, in Klammern die verstrichene Zeit und
was gerade geschieht, dahinter den Schalter. Die Ladetexte meinen
nichts, und das ist Absicht: **Was wirklich geschieht, steht daneben in
der Klammer.** `^T` zeigt die Werkzeugaufrufe, auch die, die vorher
schon liefen.

⚑ **Zwei Modi, und Umschalt-Tab dazwischen.** Im `manual mode` wird
jede schreibende Handlung vorgelegt und läuft erst nach einer
Bestätigung; Lesen und Suchen fragen nie. **Ein Modus, der auch das
Lesen bestätigen liesse, wäre nach drei Fragen abgeschaltet.** Die
Nachfrage hängt am Werkzeug und nicht an der Meldung: Ein Melder darf
berichten, verhindern kann nur, wer ausführt.

⚠️ **Im Fenster bleiben die schreibenden Werkzeuge im manual mode ganz
weg**, denn es hat noch keinen Bestätigungskasten. **Ein Modus, der in
einem Fenster fragt und im anderen stillschweigend durchlässt, wäre
schlimmer als keiner.**

📌 **Fund 318: Eine Absage, die wie ein Fehler klingt, wird wie ein
Fehler behandelt.** „Vom Nutzer abgelehnt" liess das 4B-Modell raten,
die Datei sei nicht da, und es empfahl einen zweiten Versuch. Der Satz
ist jetzt für ein kleines Modell geschrieben.

⚑ **Die Werkzeugkisten heissen `Base`, `Advanced` und `1337`.** Die
dritte steht nur mit `MYELITH_ADMIN=1` in der Auswahl. ⛔️ **Verborgen
ist nicht geschützt, und das steht im Quelltext:** Wer die Marke setzen
will, setzt sie in einer Sekunde. Sie hält die Kiste aus der Liste
heraus, damit niemand sie für eine dritte gleichrangige Wahl hält.

📌 **Fund 314: Der Schalter `--werkzeuge` verglich ein Wort von Hand**,
während die Hilfe darüber `knapp` nannte, ein Wort, das es nie gab.

📌 **Fund 315: Zwei Erlaubnisse für dieselbe Sache sind eine zu viel.**
„Bezeugte Werkzeuge zulassen" musste gesetzt sein, sonst standen die
Dateiwerkzeuge zwar in der Ansage und liefen nicht. **Ein angemeldetes
Werkzeug, das nicht laufen kann, ist die schlechteste aller
Auskünfte.** Der Schalter ist entfallen: Das eingehängte Verzeichnis
ist die Zustimmung, `agent.schreiben` entscheidet über das Schreiben,
und die Werkzeugkiste sagt, welche Werkzeuge es gibt.

⚑ **Die Einstellungsseite hat eine neue Reihenfolge:** Sprache,
Aktualisierung, Modelle, was dieser Rechner hergibt, und zuletzt Modell
und Agent. Welches Feld in welche Tabelle gehört, entscheidet sein
**Name**: `kap.speicher` heisst in jeder Sprache so, „Grenzen dieses
Rechners" nicht.

⚑ **Die Befehle der Konsole heissen `/help` und `/exit`**, die
deutschen Formen bleiben als Zweitnamen. 📌 Und sie stehen in **einer**
Liste, aus der Ausführung und Hilfe kommen; die Prüfung dazu zählte
vorher, ob jedes Wort mindestens zweimal im Quelltext steht. **Eine
Prüfung, die Wiederholung verlangt, hält die Wiederholung fest.**

⚑ **Beide Wege des manual mode sind am echten Modell gefahren.** Mit
`j` steht das Wort in der Datei, mit `n` gibt es keine Datei, und das
Modell berichtet die Absage.

`myl-client` **0.23.0 auf 0.24.0** (165 Prüfungen), `myl-oberflaeche`
**0.28.0 auf 0.29.0** (49 Prüfungen), `myl-console` **0.2.0 auf 0.3.0**
(76 Prüfungen).

### v0.35.0 – 2026-09-10 (zwei Werkzeugkisten, eine Mehrfachänderung und ein Rahmen, der unten steht)

**Auftrag des Projektinhabers**, in vier Sätzen: mehr Agentenwerkzeuge;
je eine Werkzeugkiste für kleine Modelle unter 7B und für die ~30B, die
noch kommen; beide Bedieninstrumente greifen darauf zu; und wer will,
gibt auch dem kleinen Modell die volle Kiste.

⚑ **Die Kiste hängt am Artefakt, nicht am Aufrufort.**
`Werkzeugkiste::fuer_artefakt` liest `model_config.json`, nimmt die Zahl
vor dem `b` in der Variante (`30b-a3b` sind dreissig Milliarden) und
entscheidet an der Grenze von sieben. Findet sie nichts, nimmt sie die
kleine Kiste und sagt, warum. Fenster und Konsole rufen dieselbe
Funktion, und im Fenster steht sie an genau einer Stelle: **Zwei
Stellen, die dieselbe Wahl jede für sich treffen, zeigen irgendwann
fünf Werkzeuge an und geben sieben.**

⚠️ **Heute liegt in beiden Kisten dasselbe**, fünf Dateiwerkzeuge. Der
Schnitt ist gebaut, der Inhalt fehlt. Das steht hier, weil ein
Schalter, der nichts umschaltet, sonst wie eine Zusage aussieht.

⚑ **`edit_file` ändert mehrere Stellen auf einmal.** Der Parameter
`aenderungen` ist eine Liste aus `alt` und `neu`, mindestens ein
Eintrag. **Erst ein Trockenlauf über eine Kopie, dann wird
geschrieben:** Wer drei Stellen nennt und bei der zweiten danebenliegt,
bekommt alle Fehler auf einmal und eine Datei, die unverändert ist.
Eine halb ausgeführte Änderung ist schlimmer als eine abgelehnte, denn
danach sieht der Agent eine Datei, die weder der alte noch der neue
Stand ist.

📌 **Fund 307: Eine Wache, die auf der obersten Ebene stehenbleibt,
bewacht die oberste Ebene.** Die Prüfung, dass kein Werkzeug einen
optionalen Parameter hat, las nur die Felder des Wurzelobjekts. Der
neue Listenparameter trägt seine Pflichtfelder eine Ebene tiefer, in
`items.required`, und dort sah niemand hin. Sie steigt jetzt hinab.
Gegenprobe: ein Pflichtfeld aus dem Listeneintrag entfernt, und sie
meldet es beim Namen.

📌 **Fund 312: `myl einstellungen` zeigte zehn von dreizehn Feldern.**
Die Liste war von Hand getippt, während der Katalog daneben wuchs: Drei
Felder liessen sich setzen und standen danach nirgends. Sie kommt jetzt
aus dem Katalog, die Werte aus derselben Funktion wie im Fenster, und
die Einheit aus der Beschriftung statt aus einer zweiten Liste.
**Aufgefallen ist es an der eigenen neuen Einstellung**, beim Nachsehen,
ob sie durch die Kommandozeile zurückkommt.

⚑ **Die Konsole bekommt eine umrandete Eingabezeile.** Darunter steht,
welches Modell gewählt ist und welche Kiste es hat, **der Name und
nicht der Pfad**: Gefragt war, welches Modell gewählt ist, und
`myelith-4b` beantwortet das besser als ein von vorne abgeschnittener
Pfad. ⚠️ „Unten fixiert" gibt es in einem Terminal nicht, ohne den
ganzen Bildschirm zu übernehmen; der Rahmen wird deshalb vor jeder
Eingabe neu gesetzt und wandert mit der Ausgabe nach oben.

📌 **Fund 308: Im Rohmodus ist ein Zeilenvorschub kein Zeilenende.**
Gemeldet wird 0x0A als Strg-J, also als Buchstabe; er landete im Text,
und die Zeile lief weiter. **Wer mehrzeiligen Text einfügt, schickt
genau dieses Byte.**

📌 **Fund 309: der Rahmen ohne Boden.** Der Zeilenumbruch der Eingabe
setzt den Wagen genau auf die untere Kante; alles, was danach gedruckt
wurde, fing dort an, und die Fusszeile stand mitten in der Antwort.
Jetzt wird von dort bis zum Schirmende geräumt und die Kante neu
gezogen.

📌 **Fund 310: „wortgetreue Kopie" stand nur im Kopf der vier Dateien.**
Beim ersten Eingriff (Fund 308) zog nichts ausser der eigenen
Aufmerksamkeit die zweite Kopie nach. Eine Prüfung vergleicht sie jetzt
Zeile für Zeile, ohne den Kopfvermerk und ohne den Untertitel, der die
eine erlaubte Abweichung ist.

📌 **Fund 313: Wer im Klon baut, ändert nicht, was im PATH liegt.**
`cargo build --release` schreibt nach `target-shared/release/`,
aufgerufen wird das Programm, das der Installer nach `~/.local/bin`
gelegt hat. Beide heissen gleich, und **nichts sagt, dass die Kopie
älter ist.** Wer baut und danach das installierte Programm fährt, fährt
das von gestern; `SYSTEM/install/installieren-<system>` baut und legt in einem
Zug.

⚑ **Nachgemessen und nicht behauptet.** Der Rahmen lief an einem
Pseudoterminal fester Grösse, und der Zeichenstrom wurde durch einen
kleinen Schirmnachbau gerendert: **Ein Bytestrom mit Wagenbefehlen
sagt nicht, was am Ende dasteht.** Im selben Lauf hat der 4B-Agent
`list_directory` gerufen und die beiden Einträge des Ordners richtig
genannt.

`myl-client` **0.22.0 auf 0.23.0** (165 Prüfungen), `myl-oberflaeche`
**0.27.2 auf 0.28.0** (49 Prüfungen), `myl-console` **0.1.0 auf 0.2.0**
(6 Prüfungen am Programm, dazu 7 am Rahmen selbst).

### v0.34.2 – 2026-09-10 (die Kopfleiste hat eine feste Höhe, und alle vier Abstände kommen aus einer Zahl)

📌 **Vier Anläufe, und der Grund lag jedes Mal woanders, als ich
suchte.** Gemeldet wurde zuletzt: „Beim Ausklappen staucht sich die
obere Leiste in der Vertikalen."

**Der Kopf trug `min-height: 2rem` plus Polsterung**, und weil
`box-sizing: border-box` gilt, war seine Höhe **das Grössere von
beidem**: mit der Marke in der Mitte deren Inhalt plus Polster, ohne
sie der Mindestwert. Die Marke erscheint aber genau dann, wenn die
Leiste zugeht. **Eine Höhe, die am Inhalt hängt, ändert sich mit dem
Inhalt**, und der wechselte hier beim Bedienen.

⚑ **Jetzt hängt sie an zwei Zahlen und an nichts sonst:**

```css
--kopf-polster: .85rem;
--kopf-knopf: 2rem;
header { height: calc(var(--kopf-knopf) + 2 * var(--kopf-polster)); padding: 0; }
header > #leiste-schalten { left: var(--kopf-polster); }
header > .kopfrechts     { right: var(--kopf-polster); }
```

**Und daraus folgt die Zusage rechnerisch.** Ist die Leiste so hoch wie
der Knopf plus zweimal das Polster, und sitzt der Knopf mittig, dann
ist sein Abstand nach oben und unten **dasselbe Polster** wie links und
rechts. Alle vier Abstände kommen aus einer Variablen: **Was aus
derselben Zahl kommt, kann nicht auseinanderlaufen.**

Die Prüfung misst seither die Ableitung statt der Zahl: feste Höhe aus
den beiden Variablen, kein eigenes Polster am Kopf, und beide Knöpfe
nennen `var(--kopf-polster)` statt einer eigenen Angabe.

`myl-oberflaeche` **0.27.1 auf 0.27.2** (49 Prüfungen).

### v0.34.1 – 2026-09-10 (Fund 305: Aussehen gehört ins Stilblatt)

📌 **Hinter jedem Gesprächstitel stand ein rundes Feld** (gemeldet vom
Projektinhaber). Der Titel ist ein Knopf, und `.blank` schaltet nur die
beiden Zierpseudoelemente ab; Glasverlauf, Rundung, Polsterung,
Schatten und `backdrop-filter` blieben. **Das Skript nahm davon fünf
Dinge von Hand wieder weg**, mit Stilangaben direkt am Element, und
vergass Rundung und Hintergrundfilter.

⚑ **Aussehen gehört ins Stilblatt.** Ein Skript, das Stilangaben setzt,
ist eine zweite Stelle, an der etwas fehlen kann, und sie ist die
schlechter geprüfte: Im Stilblatt steht der Rückbau beieinander und
fällt als Lücke auf, im Skript steht er zwischen zwei
Ereignisbehandlungen. Eine Prüfung verbietet acht solcher Zuweisungen;
Bewegung und gemessene Lage bleiben erlaubt, denn die kann kein
Stilblatt wissen.

⚠️ **Und der Rückbau steht am Titel, nicht an `.blank`.** Eine
allgemeine Regel hätte hier funktioniert und drei andere Knöpfe
zerlegt: In diesem Stilblatt stehen die Bauteilregeln **vor** den
Grundregeln für Bedienelemente, also hätte eine späte Klasse `.modus`,
`.chat` und `.rundknopf` überstimmt. **Wer eine Regel allgemein macht,
macht sie für alles, was sie trifft.**

⚑ **Die Kopfleiste hat mehr Luft**, `.85rem` statt `.6rem` oben wie
unten, aus einer Angabe (Festlegung des Projektinhabers). Die Prüfung
verlangt zwei konstante Werte statt einer Formel und einen Mindestwert
für die Höhe.

`myl-oberflaeche` **0.27.0 auf 0.27.1** (49 Prüfungen).

### v0.34.0 – 2026-09-10 (Fund 304: ein Bruchstück im Stilblatt, und es erklärt drei Meldungen eines Abends)

📌 **Im Stilblatt stand ein verwaister Block**, eingecheckt und aus
einem halb zurückgenommenen Umbau: eine schliessende Klammer, ein
Backtick, danach das Ende eines Kommentars und zwei Dutzend Angaben
ohne Regel darum.

⚠️ **Ein Browser wirft so etwas nicht weg, er verschluckt das
Nächste.** Nach dem Backtick sucht der Auflöser einen Selektor und
liest alles bis zur nächsten `{`. Die nächste war die von `.chat`, und
damit war **die ganze Regel für eine Gesprächszeile weg**: kein
`display: flex`, keine Breite, und deshalb auch keine Ellipse am Titel,
denn `text-overflow` braucht eine Schranke.

**Drei Meldungen desselben Abends hingen daran:** Titel ohne Kürzung,
eine Leiste, die dem Hauptfenster Platz nahm, und ein Zahnrad, das
dabei aus der Ecke rutschte. Ich habe zwei Stunden an den Symptomen
gemessen, bevor die Datei selbst an die Reihe kam.

⚑ **Eine Prüfung misst jetzt die Form der Datei**, nicht ihren Inhalt:
Kommentare gehen auf und zu, Klammern auch, und ein Backtick ausserhalb
eines Kommentars ist ein Fehler. Gegengeprüft mit einem eingeschmuggelten.

⚑ **Und die Kopfknöpfe hängen nicht mehr am Raster.** Dreimal gemeldet,
dreimal anders repariert: erst ein grösseres `minWidth`, dann
`minmax(0, 1fr)` auf der mittleren Spalte, dann ein konstantes Polster.
**In einem Raster hängt die Lage jeder Spalte an allen anderen**, und
die Marke in der Mitte erscheint genau dann, wenn die Leiste zugeht.
Beide Knöpfe liegen jetzt absolut, je `1rem` von ihrer Kante: Ihre Lage
hängt an genau einer Zahl, an derselben für beide, und an nichts sonst.

⚑ **Die Liste zwingt ihre Spalte auf null.** `#chatliste` bekam
`grid-template-columns: minmax(0, 1fr)`: Eine Rasterspalte ohne Angabe
ist `auto`, und ihr Mindestwert ist der Mindestinhalt der Zeile darin.
**Eine Ellipse braucht eine Schranke, sonst ist sie nur eine Absicht.**

⚑ **Und ein Titel ist Text und keine Kachel** (Festlegung des
Projektinhabers). Kein Rahmen, keine Fläche: Unterschieden wird über
Helligkeit und Schriftschnitt, wie überall in diesem Fenster.

`myl-oberflaeche` **0.26.3 auf 0.27.0** (48 Prüfungen).

### v0.33.1 – 2026-09-10 (das Programmsymbol trägt das ganze Zeichen, und der Titel wird nicht mehr beim Speichern gekürzt)

📌 **Gekürzt wurde die Sache statt ihrer Darstellung** (gemeldet vom
Projektinhaber). `titel_aus` schnitt auf vierzig Zeichen und hängte
`...` an, und **das war der gespeicherte Titel**: Die Zeile in der
Leiste kürzte danach ein zweites Mal, und der Zeigetext beim
Überfahren zeigte dieselbe gekürzte Zeichenkette. **Das Lange war
nirgends mehr zu holen.**

⚑ **Kürzen ist Anzeige und gehört ins Stilblatt.** Gespeichert wird
jetzt der volle erste Satz, die Zeile kürzt ihn mit einer Ellipse auf
die Breite der Leiste, und der Zeigetext gibt ihn ganz her. Eine
Schranke bleibt, aber weit oben: Wer einen Absatz einwirft, soll keinen
Absatz in der Ablage haben. **Meldungen kürzen weiter**, und dort ist
es richtig: Sie haben keine Leiste, die für sie kürzt.

⚑ **Das Programmsymbol trägt jetzt das ganze Zeichen** (Festlegung des
Projektinhabers): Spirale im Rahmen und darunter der Zug „Myelith", in
denselben Verhältnissen wie im Kopf der Seitenleiste. Die Schrift kommt
aus dem Stilblatt und liegt kein zweites Mal daneben.

⚠️ **Und es gibt zwei Fassungen, je nach Grösse.** Bei
zweiunddreissig Pixeln ist „Myelith" vier Pixel hoch und damit kein
Wort mehr, sondern ein Fleck, der die Marke daneben kleiner macht.
Unter 128 Pixeln fällt der Zug deshalb weg und die Marke rückt in die
Mitte. **`.icns` und `.ico` tragen je Grösse ein eigenes Bild; genau
dafür gibt es das Format.**

📌 **Dabei fiel das dritte Opfer von Fund 298 auf.** Der Symbolerzeuger
suchte die Marke unter `werkzeuge/marke.py`, also am Ort vor dem Umzug
des Werkzeugverzeichnisses. Er lief seither in einen Fehler, und
gemerkt hätte man es erst beim nächsten Symbollauf. **Der eigene Ort
ist der einzige Bezug, der einen Umzug überlebt.**

`myl-oberflaeche` **0.26.2 auf 0.26.3** (47 Prüfungen).

### v0.33.0 – 2026-09-10 (die Installation zieht nach `INSTALL/`, mit einer Anleitung je System)

**Auftrag des Projektinhabers.** Die drei Skripte liegen jetzt unter
`INSTALL/`, daneben ein `README.md`, das jedes System knapp und
vollständig beschreibt: Voraussetzungen samt den Befehlen, mit denen
man sie holt, die drei Schalter, wo die Programme landen, was die
Skripte ausdrücklich **nicht** tun, und der Hinweis, dass ein frischer
Klon keine Gewichte enthält.

⚑ **Drei Skripte lose in der Wurzel sagen nicht, welches das eigene
ist.** Ein Ordner namens `INSTALL` mit einem Text darin sagt es.
`flake.nix` bleibt in der Wurzel: `nix develop` sucht es dort und
nirgends sonst.

📌 **Und das Verschieben hat prompt dieselbe Falle gestellt wie Fund
298.** Die Skripte leiten ihre Wurzel aus dem eigenen Ort ab; ohne das
nachgezogene `/..` zeigte sie auf `INSTALL/` selbst. **Der
Voraussetzungslauf meldete trotzdem „alles da"**, denn er prüft Xcode
und cargo, und die hängen nicht an der Wurzel. Gefallen wäre es erst
beim Bauen, mit einer Meldung über eine fehlende Kiste.

📌 **Das Kopfpolster ist wieder konstant** (gemeldet vom
Projektinhaber). Das `clamp(.5rem, 1.6vw, 1rem)` von heute Mittag war
auf beiden Seiten gleich, wanderte aber mit der Fensterbreite, und das
fiel genau dann auf, wenn sich sonst etwas bewegt: beim Auf- und
Zuklappen der Leiste. **Was stabil aussehen soll, muss konstant sein
und nicht nur symmetrisch.** Jetzt `padding: .6rem 1rem`, also derselbe
Abstand zum Fensterrand auf beiden Seiten, unabhängig von Breite,
Leiste und Schriftgrösse.

`myl-client` **0.21.0 auf 0.22.0**, `myl-oberflaeche` **0.26.1 auf
0.26.2** (46 Prüfungen).

### v0.32.1 – 2026-09-10 (die Leiste gibt nach, nicht der Inhalt, und zwei Fussnoten werden zu Fussnoten)

📌 **Der Projektinhaber hat die Ursache genannt, nicht ich:** „Wenn die
Menüleiste links ausgeblendet ist, stimmt es genau." Damit war klar,
wonach zu suchen war. **Wird im Raster der Platz knapp, verliert zuerst
die flexible Spalte**, und das war das Hauptfenster; die Seitenleiste
behielt derweil ihre volle Breite. Mein `minmax(0, 1fr)` vom selben Tag
hat das sogar erlaubt, indem es dessen Mindestwert auf null setzte.

⚑ **Jetzt ist es umgekehrt.** Das Hauptfenster hat einen Mindestwert
von 22 rem, die Leiste hat keinen: Wird es eng, wird die Leiste
schmaler und kürzt ihre eigenen Zeilen, die dafür Ellipsen tragen.
**Was abgeschnitten wird, soll das sein, was sich zuklappen lässt, und
nicht das, was man gerade bedient.**

⚠️ **In `rem` und nicht in Pixeln**, denn der Inhalt darin ist ebenfalls
in `rem` bemessen. Eine Schranke in der anderen Einheit war der Fehler,
den diese eine Zeile dreimal wiederholt hat.

⚑ **Und zwei Fussnoten sehen jetzt aus wie welche** (Festlegung des
Projektinhabers): der Pfad unter der Modellwahl und die Zeile unter der
Eingabe, beide von 0,72 auf 0,58 rem und eine Helligkeitsstufe leiser.
Sie beantworten die Frage „welches genau", und die stellt sich selten;
**der Name darüber beantwortet die, die sich ständig stellt.**

📌 **`min-height` bleibt an der Zeile unter der Eingabe.** Ohne sie
springt die Eingabezeile jedes Mal, wenn die Auskunft kommt oder geht.
**Eine Zeile, die auftaucht und dabei alles darüber verschiebt, liest
man nicht, man erschrickt.**

`myl-oberflaeche` **0.26.0 auf 0.26.1** (46 Prüfungen).

### v0.32.0 – 2026-09-10 (das Fenster wächst mit, statt eine Zahl zu behaupten)

📌 **Dreimal wurde die Mindestbreite zu klein geraten**, jedes Mal vom
Projektinhaber gemeldet, und beim dritten Mal war der Grund klar: **Der
Aufbau ist in `rem` bemessen, die Zahl in Pixeln.** Leiste 16 rem,
Knöpfe 2 rem, Polster 1 rem; wer die Systemschrift grösser stellt,
bekommt all das grösser und die Mindestbreite nicht. **Eine Rechnung
über zwei Einheiten stimmt für genau eine Schriftgrösse**, und welche
das ist, weiss der, der sie aufschreibt, nicht.

⚑ **Die Zusage hängt jetzt nicht mehr an der Zahl.** Seitenleiste und
Polster wachsen mit dem Fenster:

| | |
|---|---|
| Seitenleiste | `clamp(12rem, 26vw, 16rem)` |
| Kopfleiste | `padding: .6rem clamp(.5rem, 1.6vw, 1rem)`, `gap: clamp(.4rem, 1.2vw, .8rem)` |
| Gespräch, Eingabe, Einstellungen | dieselbe Formel, eigene Grenzen |

⚑ **Und die beiden Kopfpolster kommen aus einer Formel.** `padding:
.6rem X` setzt X links **und** rechts: Solange dort eine einzige Angabe
steht, können die beiden Abstände nicht auseinanderlaufen. Was links
vor dem Leistensymbol steht, steht rechts hinter dem Zahnrad, bei jeder
Breite und jeder Schriftgrösse. **Zwei Zahlen könnten auseinanderlaufen,
und genau das war der Fehler.**

📌 **Dazu die Ursache, die keine Breite geheilt hätte.** `#huelle` stand
auf `grid-template-columns: auto 1fr`, und der selbsttätige Mindestwert
einer Rasterspalte ist der **Mindestinhalt** ihres Kindes, nicht null:
Die Leiste konnte breiter werden, als das Fenster hergibt, und
`overflow: hidden` schnitt dann rechts ab. **Abgeschnitten wird immer
das Letzte, und das Letzte ist das Zahnrad.** Jetzt `minmax(0, auto)
minmax(0, 1fr)`.

Die Mindestgrösse bleibt bei 860 × 540, aber sie ist seither eine
Bequemlichkeitsgrenze und keine Zusage. Die Prüfung misst die Formeln
und nicht mehr die Zahl.

`myl-oberflaeche` **0.25.1 auf 0.26.0** (46 Prüfungen).

### v0.31.1 – 2026-09-10 (die Mindestbreite bekommt Reserve, und die kleine Marke ihren Rahmen abgenommen)

📌 **Zweimal zu knapp gewesen**, beide Male vom Projektinhaber gemeldet:
Das Zahnrad stand nicht mit demselben Abstand vom rechten Rand wie das
Leistensymbol vom linken. **Der Grund steckt in der Einheit.** Die
Leiste ist 16 rem breit, die Knöpfe sind 2 rem, die Polster 1 rem: Wer
die Systemschrift grösser stellt, bekommt **alles davon grösser**,
während die Mindestbreite in Pixeln steht. Bei 18 px je rem sind aus
121,6 px Kopf schon 136,8 px und aus 256 px Leiste 288 px.

⚑ **Eine Rechnung in Pixeln über einem Aufbau in rem braucht Luft.**
Jetzt 860 × 540: Leiste 257 px plus eine Gesprächsspalte von 34 rem
sind 801 px, dazu 60 px Reserve für Fensterrahmen, Bildlaufleiste und
grössere Systemschrift.

⚑ **Und die Marke im Kopf trägt keinen Rahmen mehr** (Festlegung des
Projektinhabers). Bei 1,15 rem sind Rahmen und Kreis nur noch zwei
Striche dicht nebeneinander, und der Rahmen gewinnt, weil er gerade
ist. **Was bei voller Grösse Fassung ist, wird im Kleinen Rauschen.**
In der Seitenleiste bleibt er, dort hat die Marke Platz.

`myl-oberflaeche` **0.25.0 auf 0.25.1** (46 Prüfungen).

### v0.31.0 – 2026-09-10 (die Sprache greift überall durch, und ein Eintrag entsteht nur noch auf zwei Wege)

📌 **Ein Moduswechsel legte ein Gespräch an** (gemeldet vom
Projektinhaber). Der Modus hing am geöffneten Eintrag, und daraus
folgte: Um den Modus überhaupt festhalten zu können, **musste** der
Wechsel etwas anlegen. Wer zwischen Chat und Agent hin und her klickte,
hinterliess bei jedem Klick ein leeres Gespräch, und die Liste des
anderen Modus zeigte es beim nächsten Wechsel mit an.

⚑ **Der Modus ist jetzt ein eigener Zustand, und eine Ansicht legt
nichts an.** Angelegt wird auf Knopfdruck und beim Abschicken in einem
leeren Feld, an genau zwei Stellen; auch der Start legt nichts mehr an.
Eine Prüfung zählt die Aufrufe und fällt bei einer dritten.

📌 **Die Sprache wirkte nicht auf die Beschreibungstexte** (ebenfalls
gemeldet). `hardware::regler` nahm die Feldtabelle roh, also immer auf
Deutsch, und die beiden längsten Sätze der ganzen Seite standen
überhaupt nur auf Deutsch da: der Grund, warum ein Rechenwerk gesperrt
ist, und die Beschreibung eines Rechenwerks. **Übersetzt wird, was ein
Mensch liest, und das gilt besonders für den Satz, der erklärt, warum
etwas nicht geht.** Die Prüfung dazu fährt beide Sprachen wirklich und
vergleicht Satz für Satz.

⚑ **Die Sprache steht zuoberst, die Updates gleich darunter**
(Festlegung des Projektinhabers). Sie beschriftet alles, was darunter
kommt: Wer die Seite in einer Sprache öffnet, die er nicht liest, soll
den Schalter finden, ohne bis ans Ende zu suchen.

**Die Updates heissen jetzt so.** „Nach Updates suchen" und „Updates
installieren", und **der zweite Knopf erscheint erst, wenn es etwas zu
tun gibt**. 📌 Vorher stand er gesperrt da: Ein gesperrter Knopf
beantwortet die Frage „gibt es Updates" mit einem Bedienelement, und
der Grund steckte in seinem Zeigetext, wo ihn nur findet, wer mit der
Maus darauf wartet. Die Zeile darüber beantwortet dieselbe Frage mit
einem Satz.

`myl-client` **0.20.0 auf 0.21.0**, `myl-oberflaeche` **0.24.2 auf
0.25.0** (46 Prüfungen).

### v0.30.0 – 2026-09-10 (drei Meldungen aus der CI und eine aus dem Fenster)

📌 **Das Zahnrad stand nicht frei** (gemeldet vom Projektinhaber). Die
Mindestbreite von 600 reichte nicht: Rechts blieb nicht derselbe
Abstand wie links vom Leistensymbol.

**Zwei Ursachen, und die Zahl war nur die zweite.** Die mittlere Spalte
der Kopfleiste stand auf `1fr`, und **der selbsttätige Mindestwert
einer `1fr`-Spalte ist ihr Mindestinhalt, nicht null**: Steht dort
etwas, das nicht umbrechen kann, wächst sie, schiebt die äusseren
Spalten hinaus, und `#huelle` schneidet sie mit seinem
`overflow: hidden` ab. Sichtbar wird das als fehlendes Zahnrad. Sie
steht jetzt auf `minmax(0, 1fr)`.

⚑ **Und die Mindestbreite ist gerechnet statt gerundet:** Seitenleiste
257 px plus eine Gesprächsspalte von 24 rem sind 641 px, aufgerundet
auf 660 für Fensterrahmen und Bildlaufleiste. Der Kopf selbst braucht
121,6 px und passt darin. Die Prüfung trägt die Rechnung als Tabelle
und nicht die Zahl allein.

⚑ **Und der Netzeintrag der Modellwahl heisst jetzt „Netzwerkmodell
(API), kostet Inferenz-Credits"** (Wortlaut des Projektinhabers). Er
steht in einer Liste neben „Myelith 4B" und „Myelith 7B", und „API"
allein liest sich dort wie eine Schnittstelle und nicht wie ein Modell.
**Ein Eintrag in einer Modellwahl muss zuerst sagen, dass er ein Modell
ist.**

📌 **Drei Meldungen aus der CI**, alle drei am neuen Konsolenclient:

- **Kein `[profile.test]`.** Siebzehn Kopien einer Einstellung driften
  leise auseinander, und ein neues Manifest hat sie schlicht nicht. Das
  Audit hat es gefangen, wofür es gebaut ist.
- **Keine `rust-version`.** Jetzt 1.88, und **gemessen statt
  geschätzt**: 1.85 und 1.86 scheitern, 1.88 trägt. Der Grund liegt im
  eigenen Code, nicht in einer Abhängigkeit: `farben.rs` ruft
  `u64::is_multiple_of`, stabil seit 1.87.
- **Eine Prüfung fiel unter Windows.** `ein_relativer_pfad_haengt_an_der_wurzel`
  verglich gegen `"/wo/auch/immer/INTEGER_LLM/…"` als getippten Text;
  dort setzt `Path::join` einen Backslash. **Eine Erwartung, die von
  Hand geschrieben ist, prüft die Maschine, auf der sie geschrieben
  wurde.** Sie baut den Vergleichswert jetzt mit `join`, und eine
  zusätzliche Zeile hält fest, dass zwei verschiedene Wurzeln auch zu
  zwei verschiedenen Pfaden führen.

`myl-oberflaeche` **0.24.1 auf 0.24.2**, `myl-console` **0.1.0** um zwei
Manifestzeilen ergänzt.

### v0.29.0 – 2026-09-10 (`myl-console`: der Agent in dem Verzeichnis, in dem du stehst)

**Auftrag des Projektinhabers.** Eine dritte Bedienoberfläche neben
Fenster und Kommandozeile: `myelith`, getippt in einem Verzeichnis.

⚑ **Das Arbeitsverzeichnis ist der Arbeitsordner, ohne Frage und ohne
Feld.** Im Fenster ist er eine Einstellung, weil ein Fenster nirgends
steht; ein Programm in der Konsole steht immer irgendwo. **Es
überschreibt `agent.wurzel` für diese Sitzung**, statt es zu ergänzen:
Sonst arbeitete der Agent in einem Ordner, den jemand vor Wochen im
Fenster gesetzt hat, während der Nutzer woanders steht.

⚠️ **Und der Kopf nennt den Ordner, bevor die erste Eingabe möglich
ist.** Ein Agent mit Dateiwerkzeugen in einem Verzeichnis, das jemand
nur zufällig betreten hat, ist genau der Fall, gegen den die
Einhängegrenze gebaut ist.

Der Ablauf: Startbild, ein Absatz darunter, Modellwahl, dann Auftrag um
Auftrag. Sieben Befehle, mehr nicht: `/model`, `/settings`, `/context`,
`/compress`, `/clear`, `/hilfe`, `/ende`. **Ein Konsolenprogramm mit
zwanzig Befehlen ist eines, dessen Hilfeseite man liest, statt es zu
benutzen.**

⚑ **Keine eigene Logik, wie beim Fenster.** Agentenlauf, Werkzeuge,
Einstellungen, Modell und Ortsbestimmung kommen aus `myl-client`. Die
Einstellungen werden **gezeigt und nicht gesetzt**: Ein zweiter Setzer
neben `myl setzen` und der Einstellungsseite wäre die dritte Stelle,
die dieselben Feinheiten kennt.

⚠️ **Und gebaut wird hier nichts.** Ein Artefakt zu holen und zu
kalibrieren dauert Minuten bis Stunden; das gehört nicht hinter eine
Zeile, die jemand tippt, weil er eine Frage stellen wollte. Wer keines
hat, wird ans Fenster verwiesen, das dafür einen Balken hat und einen
Abbruch.

⚑ **Das Startbild ist eine wortgetreue Kopie aus dem Testclient**, auf
Festlegung des Projektinhabers: Die Marke ist die Marke. **Wortgetreu
und nicht gekürzt**, obwohl `myelith` nicht jede Funktion darin ruft;
eine gekürzte Kopie ist weder das Original noch etwas Eigenes und lässt
sich nicht mehr gegen die Quelle halten. Geändert ist genau eine Zeile,
der Untertitel. Der Testclient wird abgeräumt, sobald er seine Aufgabe
erfüllt hat; bis dahin liegen die vier Dateien zweimal da.

📌 **Zwei Befunde aus dem ersten Lauf.** Die laufende Schrittzeile
löschte sich mit `\r` und Leerzeichen, und **in einer Röhre gibt es
keinen Wagenrücklauf**: Dort standen die Leerzeichen einfach da. Sie
gibt es jetzt nur vor einem Terminal. Und die Ausgabe zweier Zeilen
überschrieb einander nur halb, weil die kürzere die längere stehen
liess.

⚑ **Ausgeliefert wird sie ohne eine Zeile in einem Skript.** Die Marke
`[package.metadata.myelith]` genügt (Fund 300); alle vier Bauskripte
haben das neue Programm sofort gefunden.

`myl-console` **neu, 0.1.0** (5 Prüfungen).

### v0.28.0 – 2026-09-10 (Fund 301: das Repositorium darf umziehen)

**Auftrag des Projektinhabers:** Alle Abhängigkeitspfade beweglich, so
dass sich das Repositorium verschieben lässt und alles weiter geht.

Ein neues Modul `ort` beantwortet die Frage „wo liegt dieser Klon" an
**einer** Stelle, in vier Stufen: `MYELITH_WURZEL` aus der Umgebung,
vom Arbeitsverzeichnis aufwärts, vom Programm aufwärts, und zuletzt ein
gemerkter Ort neben der Einstellungsdatei.

⚑ **Der gemerkte Ort wird nachgeprüft und nicht geglaubt.** Steht die
Marke dort nicht mehr, ist der Zettel alt und gilt nicht. **Damit heilt
ein Umzug sich selbst:** Einmal aus dem verschobenen Klon heraus
starten genügt, und auch das installierte Programm findet danach wieder
hin.

📌 **Fund 301, und er war ein Loch, das die Installer erst aufgerissen
haben.** Die Suche stand in der Oberfläche und ging vom
Arbeitsverzeichnis und vom Programm aufwärts. Solange das Fenster aus
`target-shared` lief, ging das; **installiert unter `~/Applications`
liegt es ausserhalb des Baums**, und dann fand es weder Artefakt noch
Katalog. Dazu: `myl` löste den Pfad überhaupt nicht auf und meldete aus
einem fremden Arbeitsverzeichnis „es fehlt das Artefaktverzeichnis",
obwohl das Artefakt dalag. **Zwei Programme desselben Klienten, zwei
Antworten auf dieselbe Frage, und eines antwortete gar nicht.**

⚑ **Wer eine Fähigkeit hinzufügt, prüft, was sie an den bestehenden
Annahmen ändert.** Die Installationsskripte waren richtig und haben
etwas anderes kaputtgemacht.

Neu ist `myl ort`: Es sagt, wo der Klon liegt, und merkt ihn sich dabei.

📌 **Und ein Starter, den es einen halben Tag lang gab.** `Myelith` und
`Myelith.cmd` lagen in der Wurzel: System erkennen, prüfen ob
eingerichtet, notfalls einrichten, dann starten. **Sie sind auf
Festlegung des Projektinhabers wieder entfernt worden**, und der Grund
steht in dem, was sie gezeigt haben: Windows führt nur aus, was `.cmd`,
`.bat`, `.ps1` oder `.exe` heisst, macOS eine ausführbare Datei ohne
Endung, die Dateiverwaltungen unter Linux nur `.desktop`. **Eine Datei,
die auf allen drei Systemen anklickbar ist, gibt es nicht**, und zwei
Dateien plus ein erzeugter Menüeintrag sind kein Anklickpunkt, sondern
drei. An ihre Stelle tritt eine kurze Anleitung je Plattform in den
Wurzel-READMEs.

⚠️ **Mit ihnen entfallen die Funde 302 und 303**, denn beide waren
Befunde an ihnen: eine Wache, die nie zutraf, und zwei Schreiber für
denselben Menüeintrag. Die Nummern bleiben vergeben; sie sind Etiketten
und keine Zählung.

`myl-client` **0.19.0 auf 0.20.0**, `myl-oberflaeche` **0.24.0 auf
0.24.1** (154 und 42 Prüfungen).

### v0.27.0 – 2026-09-10 (Funde 295 bis 300: das Fenster spricht Englisch, die Liste kennt ihren Modus, und drei Installationsskripte liegen in der Wurzel)

**Ein Stapel Aufträge des Projektinhabers an einem Tag.** Sie hängen
lose zusammen: Alles hier macht den Klienten für jemanden bedienbar,
der ihn nicht gebaut hat.

#### Fund 295: die Lizenz stand am falschen Ding

Die Artefaktliste zeigte je Eintrag **eine** Lizenz, und sie stand
hinter dem Namen „Myelith 4B": `4 Mrd. · rund 7,5 GB · Artefakt 4,5 GB
· Apache-2.0 · verifiziert`. Unter Apache-2.0 stehen die
**Grundgewichte**; das daraus gebaute Artefakt steht unter der Lizenz
dieses Repositoriums.

⚑ **Jede Lizenz steht jetzt in der Klammer hinter der Sache, für die
sie gilt:** `Gewichte rund 7,5 GB (Apache-2.0) · Artefakt 4,5 GB
(PolyForm Shield License 1.0.0)`. **Eine Angabe ist nicht dadurch
richtig, dass sie stimmt, sondern dadurch, dass sie sich auf das
bezieht, wonebendran sie steht.**

📌 **Und die Prüfung dazu geht bis zur Lizenzdatei.** Der Wert im
Katalog ist von Hand geschrieben; eine Prüfung, die nur nachsieht,
**dass** dort etwas steht, fängt weder den Tippfehler noch den
Lizenzwechsel. `jede_lizenz_steht_bei_ihrer_sache` hält ihn gegen
`LICENSE.md`. Sie fiel beim ersten Lauf: Dort stand „PolyForm Shield
1.0.0", die Lizenz heisst „PolyForm Shield License 1.0.0".

#### Das Fenster spricht Deutsch oder Englisch

Ein Feld in den Einstellungen, ganz unten. **Die Sprache wirkt sofort**
und nicht beim nächsten Start: Wer sie umstellt, will sehen, ob er sie
versteht, und ein Neustart dazwischen macht aus einer Probe eine
Entscheidung.

⚑ **Die Beschriftungen der Felder stehen nicht im Fenster, sondern in
der Kiste**, in beiden Sprachen, neben dem Feld selbst. Was über die
Naht kommt, ist fertig beschriftet. Zwei Orte für denselben Satz wären
zwei Orte, an denen die nächste Sprache vergessen werden kann.

📌 **Und was ein Programm vergleicht, wird nie übersetzt:** Feldnamen,
Pfade, Modellnamen, die Kennung des Netzmodells. Drei Prüfungen halten
das zusammen: Beide Tabellen tragen dieselben Schlüssel, jede
Beschriftung im HTML hat ihren Satz, und kein Feldname steht als Satz
in einer Sprachtabelle.

#### Die Liste zeigt einen Modus, und im Agentenmodus heissen sie Prozesse

Vorher standen Gespräche und Prozesse in einer Liste, und ein Klick auf
die falsche Zeile wechselte stillschweigend den Modus zurück, denn der
Modus hängt am geöffneten Eintrag. **Der Modus ist jetzt eine Ansicht:
Was dasteht, gehört zu dem, was oben eingerastet ist.**

⚑ **„Prozess" ist keine Geschmacksfrage.** Im Chat trägt eine Zeile
einen **Verlauf**, jeder Zug sieht die vorigen. Beim Agenten steht
jeder Auftrag für sich, mit eigenem Schrittbudget und eigener
Belegkette. Zwei verschiedene Dinge unter einem Wort behaupten, sie
seien dasselbe.

#### Kleineres am selben Tag

| | |
|---|---|
| **Schalter sind Schieber** | Runde Pille, Knauf fährt nach rechts. Es bleibt ein `input[type=checkbox]`: Leertaste, Tastaturfokus und die Ansage der Vorlesehilfe wären bei einem Nachbau aus zwei `div` alle weg |
| **Die Leiste fährt nur senkrecht** | Ein langer Titel wird gekürzt und macht die Leiste nicht breiter. Beim Überfahren steht der volle Titel da, **ohne** „(Agent)": Die Klammer sagte dasselbe wie der eingerastete Modus zwei Zeilen darüber |
| **Die Marke wandert** | Klappt die Leiste zu, erscheint sie mittig im Kopf, mit einem Störbild; beim Aufklappen geht sie mit einem anderen. Geklont und nicht abgeschrieben: Die Spirale ist gerechnet, zweiundsiebzig Pfade |
| **Das Fenster hat eine Untergrenze** | 600 × 460. Darunter blieb eine Kopfleiste und sonst nichts |

📌 **Die Störbilder hatten `both`, und eine Wache hat es gefangen.**
`nichts_wartet_unsichtbar_auf_eine_animation` sagt: Was gelesen werden
soll, darf nicht auf eine Animation warten. Mit `both` steht das
Element **vor** dem Lauf auf dem Anfangsbild, und das ist beim Kommen
`opacity: 0`. Wo Animationen nicht laufen, wäre die Marke unsichtbar
geblieben. **Der Ruhezustand ist sichtbar, und dabei bleibt es.**

#### Drei Installationsskripte und ein Aktualisierungsknopf

`installieren-macos.sh`, `installieren-nixos.sh` (mit `flake.nix`
daneben) und `installieren-windows.ps1` liegen in der Wurzel. Sie
prüfen erst alles, bauen dann die drei Programme, legen sie ab und
tragen einen Menüeintrag ein. *(Sie sind mit v0.33.0 nach `INSTALL/`
gezogen; dieser Eintrag hält den Stand seiner Fassung.)*

⚑ **Aus dem Quelltext und nicht aus einem Bündel.** Die Freigabebündel
dieses Projekts sind nicht signiert; ein Skript, das ein unsigniertes
Bündel holt und an Gatekeeper vorbeischiebt, brächte einem Nutzer bei,
genau das zu tun.

⚑ **Und sie laden nichts nach.** Fehlt eine Werkzeugkette, nennen sie
den einen Befehl und hören auf. Ein Installationsskript, das ein
zweites aus dem Netz holt und ausführt, ist die Angriffsfläche, gegen
die dieses Projekt an jeder anderen Stelle argumentiert.

**Der Klient sieht nach Aktualisierungen und spielt sie ein.** Gefragt
wird nicht „ist meine Version älter", sondern **„hat `origin`
Änderungen, die ich nicht habe"**: Das ist exakt und braucht keine
Vereinbarung über Versionsnamen. Eingespielt wird mit `git merge
--ff-only` und dem Installationsskript der Plattform.

⚠️ **Ohne Klon geht das nicht, und der Knopf sagt es.** Wer das
Programm aus einem Bündel startet, bekommt die neueste Freigabemarke
genannt und einen Verweis auf die Freigabeseite, statt eines Knopfes,
der nichts tut.

📌 **Fund 296: Das Skript meldete „alles da" und scheiterte drei Zeilen
später.** Es prüfte `command -v cargo`; ein rustup-Schalter ohne
eingestellte Werkzeugkette liegt im PATH und beantwortet das mit ja.
**Geprüft wird jetzt, ob es läuft**, nicht ob es existiert. Gefunden
beim ersten Probelauf, in einem untergeschobenen Benutzerverzeichnis.

📌 **Fund 297: Ein fehlgeschlagener Bau beendete das Skript nicht.**
Die Bauschleife lief hinter einer Röhre (`… | while read`), also in
einer Unterschale, und `set -e` greift dort nicht: Es kopierte danach
Dateien, die es nicht gibt. Beide Schleifen laufen jetzt in derselben
Shell.

📌 **Fund 298: Das Verschieben eines Verzeichnisses hat zwei Werkzeuge
still zerbrochen.** Sie fanden die Wurzel des Repositoriums über
`__file__` und zwei Ebenen aufwärts; nach dem Umzug zeigten die zwei
Ebenen auf einen Zwischenordner. **Das Werkzeug fand daraufhin keine
Datei mehr und meldete Erfolg**, was der schlechtestmögliche Ausgang
ist. Dieselbe Klasse wie Fund 291: Ein Verschieben sieht aus wie eine
Änderung ohne Verhalten und ist manchmal keine.

📌 **Fund 299: Eine bestehende Datei wurde überschrieben, ohne
hineinzusehen.** `flake.nix` lag seit dem 2026-09-08 in der Wurzel und
trug Wissen, das nirgends sonst steht: `WEBKIT_DISABLE_COMPOSITING_MODE`
(ohne das geht das Fenster auf NixOS unter Wayland auf und bleibt
**weiss**, ohne Absturz und ohne Meldung), `XDG_DATA_DIRS` mit den
GSettings-Schemata, `glib-networking` für TLS in der Ansicht, und die
Bedingung, dass all das **nur unter Linux** gilt. Eine neu geschriebene
Fassung hatte nichts davon.

⚑ **Gefunden hat es `git status`**, nicht das Lesen: Die Datei stand
als **geändert** da, wo eine neue Datei stehen sollte. Sie ist jetzt
zusammengeführt, und nichts von 2026-09-08 fehlt. **Wer eine Datei
anlegt, sieht vorher nach, ob es sie gibt**; ein „schreiben" auf einen
belegten Namen ist ein Löschen mit anderem Namen.

📌 **Fund 300: Vier Skripte, vier Listen, und sie waren am ersten Tag
schon uneinig.** Jedes Bauskript führte von Hand auf, welche Kisten
ausgeliefert werden; die drei neuen nannten drei Programme, das
örtliche Freigabeskript vier. `myl-test` fehlte in dreien, und niemand
hätte es gemerkt.

⚑ **Die Kiste sagt es jetzt selbst.** `[package.metadata.myelith]` mit
`ausliefern = "<name>"` steht in der `Cargo.toml` der vier
betroffenen Kisten; alle vier Skripte suchen danach. **Eine Liste, die
an vier Stellen von Hand geführt wird, ist kein Verzeichnis, sondern
vier Behauptungen.**

⚠️ **Und die Prüfung ist der Grund, warum es dabei bleibt.**
`was_ausgeliefert_wird_steht_bei_der_kiste` verlangt zweierlei: Jeder
angemeldete Name muss auch wirklich gebaut werden, und **kein Skript
darf daneben seine eigene Liste führen**. Beide Richtungen sind
gegengeprüft, jede rot.

📌 **Die erste Fassung dieser Prüfung lag selbst daneben.** Sie schlug
über `sh CLIENT/myl-oberflaeche/buendeln-macos.sh` an, also über einen
Aufruf, der mit der Liste nichts zu tun hat. Sie sucht jetzt die
**Form** der Liste, ein Paar aus Verzeichnis und Name. Dieselbe Klasse
wie die Wache, die `innerHTML` in einem Kommentar fand: **Wer
Erwähnung für Gebrauch hält, bestraft das Danebenschreiben.**

📌 **Und dieselbe Liste stand ein zweites Mal im selben Skript**, in
der Schlussmeldung: Sie nannte drei Programme, während vier installiert
wurden.

`myl-client` **0.18.0 auf 0.19.0**, `myl-oberflaeche` **0.23.0 auf
0.24.0** (42 Prüfungen).

### v0.26.0 – 2026-09-10 (Funde 292 und 293: die Herkunft geht in die Modellkarte, das Ladezeichen bleibt bis zum Schluss)

**Die Zeile unter der Eingabe nennt das Modell und nicht seine
Herkunft.** Sie las sich „Myelith 4B (aus Qwen3-4B)
(INTEGER_LLM/artifacts/myelith-4b) geladen, in 10 s": zwei Klammern
hintereinander, von denen die erste bei jedem Blick mitzulesen war und
nie eine Frage beantwortete. Jetzt steht dort „Myelith 4B
(INTEGER_LLM/artifacts/myelith-4b) geladen, in 10 s". Dasselbe in der
Artefaktliste der Einstellungsseite.

⚑ **Die Herkunft ist damit nicht verschwunden, sie ist an ihrem
Platz.** Die Grundmodelle stehen unter Apache-2.0, und ein Name ohne
Herkunft wäre eine Verschleierung; sie steht bei jedem Eintrag in
`KATALOG.json` und in `artifacts/MODEL_CARD.md`. **Eine Angabe zum
Modell gehört dorthin, wo Angaben zum Modell stehen**, und nicht in
jede Zeile, die den Namen erwähnt (Festlegung des Projektinhabers,
2026-09-10).

**Fund 292: Das Ladezeichen hängt jetzt am Lauf und nicht am Inhalt.** Es
erschien am Anfang und blieb danach aus: Nach einem Werkzeugaufruf
rechnet das Modell weiter, oft eine halbe Minute lang, und in dieser
Zeit stand nichts. Jetzt stehen die drei Punkte unter allem, was schon
da ist, solange der Lauf läuft, und sie gehen, wenn er endet.

📌 **Die Bedingung deckte genau die Wartezeit ab, die keine ist.** Sie
lautete `b.laufend && !b.text && !schritte.length`, also „läuft und es
ist noch nichts da". Das ist der Augenblick vor dem ersten Token, und
der ist kurz. **Die langen Wartezeiten liegen dazwischen**: zwischen
Befehl und nächster Überlegung, zwischen Überlegung und Antwort. Ein
Ladezeichen, das nur den ersten Abschnitt abdeckt, schweigt in jedem
Abschnitt, in dem jemand tatsächlich wartet.

⚑ **Und es steht am Ende des Beitrags, nicht am Anfang.** Über einem
wachsenden Text sähe es aus, als gehörte es zu etwas Vergangenem;
darunter heisst es „und es geht weiter", was stimmt.

📌 **Fund 293: Die Prüfung dazu prüfte die Zeile und nicht die Zusage.**
`das_ladezeichen_steht_beim_beitrag` sah nach `wurzel.append(l);` und
nach der Regel, die es beim ersten Zeichen wieder entfernte, also
genau nach der Form des Fehlers. Sie prüft jetzt, dass die Bedingung
über dem Lauf steht und nicht über dem Inhalt, und trägt den
gemeldeten Fall im Wortlaut daneben. **Eine Prüfung, die die
Schreibweise festhält, hält den Fehler fest, sobald er in der
Schreibweise steckt.**

`myl-oberflaeche` **0.22.0 auf 0.23.0** (31 Prüfungen).

### v0.25.0 – 2026-09-10 (die Zeile unter der Eingabe sagt eine Sache, und das Modell geht nach einer Weile wieder)

**Die Zeile unter der Eingabe sagt jetzt genau eines: welches Modell im
Speicher liegt.** „Myelith 4B (INTEGER_LLM/artifacts/myelith-4b)
geladen, in 9,4 s", sonst „nicht geladen", und beim Netzmodell nichts,
denn das wird hier nicht geladen.

📌 **Vorher sagte sie alles Mögliche:** „der Agent fährt", „Modell
gewechselt", „Fehler: …", und dazwischen den Ladesatz. **Eine Zeile,
die je nach Augenblick etwas anderes bedeutet, liest man irgendwann gar
nicht mehr**: Wer dort „das Modell antwortet" gewohnt ist, sieht „nicht
geladen" nicht mehr. Meldungen gehen jetzt in einen Kasten, den man
wegklicken kann.

⚑ **Und was der Lauf gerade tut, steht dort, wo er es tut.** Statt „der
Agent fährt" am Fensterrand erscheinen drei Punkte an genau der Stelle,
an der gleich die Überlegung, der Befehl oder die Antwort steht; beim
ersten Zeichen gehen sie. **Wer auf eine Antwort wartet, sieht auf den
Fleck, an dem sie erscheinen wird.**

📌 **Drei Punkte und kein Kreisel.** Beide sagen nichts über den
Fortschritt, und das ist ehrlich: Wie lange ein Modell braucht, weiß
vorher niemand. Die Punkte lenken weniger ab. Bei abbestellter Bewegung
bleiben sie stehen und sichtbar: **Ein Ladezeichen, das dann
verschwindet, nimmt genau dem die Auskunft, der sie am ehesten
braucht.**

⚑ **Das Modell geht nach fünfzehn Minuten Ruhe wieder.** Ein
4B-Artefakt sind viereinhalb Gigabyte, und sie liegen im Speicher,
solange das Fenster offen ist; wer morgens eine Frage stellt und das
Fenster stehenlässt, gibt den Rest des Tages Arbeitsspeicher her.
**Das steht in derselben Reihe wie die Kapazitätsfreigabe: Was Myelith
nimmt, soll es auch wieder hergeben.**

📌 **Es kostet nichts, wenn die Frist falsch liegt.** Wer nach einer
Stunde doch weiterfragt, wartet einmal die Ladezeit ab; der nächste
Auftrag lädt von selbst nach. **Nicht entladen wird mitten in einem
Lauf**, dort wird die Frist neu gestellt. Und ein Modellwechsel gibt das
alte sofort frei: Es antwortet ohnehin nicht mehr.

📌 **Beim Bauen fiel dabei eine Wache über einen zweiten Block.** Das
Ladezeichen bekam sein eigenes `prefers-reduced-motion`, und
`das_rauschen_gehoert_zur_abbestellbaren_bewegung` sah daraufhin am
falschen Ort nach und meldete Bewegtes als nicht abbestellt. **Es gibt
genau einen solchen Block**, und das steht jetzt daneben.

`myl-oberflaeche` **0.21.0 auf 0.22.0** (31 Prüfungen).

### v0.24.0 – 2026-09-10 (die Artefakte heissen Myelith, und bestehende Einstellungen wandern mit)

Die Artefakte heissen `myelith-4b` statt `qwen3-4b` und so fort; die
Pfade im Klienten sind nachgezogen.

📌 **Eine Umbenennung ist erst fertig, wenn das Mitgewanderte
mitgewandert ist.** Jede bestehende Ablage zeigt auf den alten Pfad,
und der löst nach dem Umbenennen ins Leere auf: Der Klient meldete
„Modell lädt nicht", und der Nutzer suchte den Fehler bei sich. Wer nur
die Verzeichnisse umbenennt, hat die Arbeit auf jeden verschoben, der
eine Einstellung gesetzt hat.

`Einstellungen::lesen` tauscht deshalb den **Verzeichnisnamen**, wenn er
einer der vier alten ist, und lässt alles andere stehen: Wer seine
Artefakte woanders hält, behält seinen Ort, und ein eigener Name wie
`qwen3-4b-eigenbau` wird nicht angefasst. ⚑ **Eine Wanderung, die auch
Unbeteiligtes anfasst, ist schlimmer als keine**, denn sie ändert einen
Pfad, den jemand mit Bedacht gesetzt hat.

Am echten Fall belegt: Die Ablage des Projektinhabers zeigte auf
`qwen3-4b`, und das Modell lädt danach unter `myelith-4b`.

### v0.23.0 – 2026-09-10 (Markdown wird gezeigt, und die Artefaktliste bekommt einen Knopf, der etwas tut)

**Die Antwort eines Modells kommt in Markdown, und das Fenster zeigte
sie als schlichten Text.** Jetzt werden Überschriften, Aufzählungen,
Code, Zitate, Linien und einfache Tabellen gezeigt.

📌 **Der naheliegende Weg dorthin wäre `innerHTML` gewesen, und er wäre
eine Lücke.** Eine Antwort mit `<img src=x onerror=…>` bekäme damit Code
in dieser Seite ausgeführt, und die Seite trägt wegen `withGlobalTauri`
die Brücke zu **allen** Befehlen des Rückens: Ein eingeschleuster Satz
könnte Einstellungen setzen oder Dateien schreiben.

⚑ **Deshalb zerlegt die Kiste und das Fenster zeichnet nur.**
`myl-client::markdown` gibt einen **Baum aus Text** heraus; daraus
werden Elemente mit `createElement` und `textContent`. Ein `<` bleibt
ein `<`. **Das ist keine Filterung, sondern eine Bauart**: Es gibt
keinen Weg, auf dem aus dieser Antwort Markup würde, und
`das_fenster_setzt_niemals_markup` hält ihn zu.

⚑ **Dieselbe Arbeitsteilung wie überall hier:** Die Kiste weiß, das
Fenster zeichnet. Ein Zerleger im Skript wäre eigene Logik im Fenster.

📌 **Und der Zerleger hatte beim ersten Lauf eine Endlosschleife.**
`| kaputt |` fängt an wie eine Tabelle, ist keine, und der Absatzzweig
brach an seiner eigenen ersten Zeile ab, ohne weiterzuzählen. **Gefunden
als Hänger und nicht als Fehlschlag**, von der Prüfung, die verlangt,
dass nichts verlorengeht.

⚑ **Während des Laufs bleibt der Text schlicht.** Eine halb angekommene
Marke ist keine Marke: `**` mitten im Strom würde als Fettdruck
aufblitzen und beim nächsten Token verschwinden. Der laufende Text ist
die Vorschau, der gesetzte das Ergebnis.

📌 **Ein Verweis wird als Text mit sichtbarem Ziel gezeigt und nicht als
Knopf.** Ein Klick führte die Webansicht aus der Anwendung heraus; sie
im System zu öffnen bräuchte eine Erlaubnis, die die Erlaubnisliste
bewusst nicht hat.

**Und die Artefaktliste:** Der Knopf „liegt vor" war gesperrt und sagte
dasselbe wie die Zeile daneben, also ein Bedienelement, das nichts
bedient. Dort steht jetzt **Löschen**, mit einer Rückfrage am Knopf
selbst, die nach fünf Sekunden abläuft. Was noch nicht gebaut ist, steht
gedämpft da: Die Liste beantwortet auf einen Blick, was man hat.

⚑ **Vor dem Löschen steht eine Kette von vier Bedingungen**, und jede
fängt einen Fall ab: Der Schlüssel muss im Katalog stehen, der
aufgelöste Pfad unter `artifacts/` liegen, eine `model_config.json`
darin sein, und das Artefakt darf nicht gerade geladen sein. **Das
Rohmodell bleibt**, denn es ist das Teure am Beschaffen; ein zweiter Bau
geht dann in Minuten.

`myl-client` **0.16.0 auf 0.17.0**, `myl-oberflaeche` **0.20.0 auf
0.21.0**.

### v0.22.0 – 2026-09-10 (Fund 289: kein Werkzeug hat mehr einen optionalen Parameter)

📌 **Gemeldet vom Projektinhaber, und der Bericht ist die Diagnose.**
Auf die Frage „welche Dateien liegen im Verzeichnis?" überlegte
Qwen3-4B seitenlang, ob es `pfad` weglassen, leer setzen oder mitgeben
solle, las dazu die `required`-Liste des Schemas, wog ab, kam zu keinem
Schluss und endete **ohne Schlussantwort**. Das Werkzeug hätte in
beiden Fällen dasselbe getan.

⚑ **Ein optionaler Parameter ist eine Entscheidung, die das Modell
treffen muss, und ein kleines Modell bezahlt sie mit seinem
Schrittbudget.** Es ist keine Frage über die Aufgabe, sondern eine über
das Formular.

**Also gibt es die Wahl nicht mehr.** Jede Eigenschaft jedes Schemas
steht in `required`, und was der Agent ohnehin nicht braucht, ist
entfallen: **Weder `list_directory` noch `search_files` nehmen einen
Pfad.** Beide arbeiten im Arbeitsverzeichnis, das der Nutzer vorher
wählt, und das ist die ganze Zusage dieser Werkzeuge.

📌 **Und der Satz, der das Grübeln ausgelöst hat, ist weg.** Die
Beschreibung sagte „Without an argument, the working directory itself"
und beschrieb damit einen Fall, den es gar nicht geben soll. Jetzt sagt
sie, **was** gelistet wird.

⚑ **Die Sicherheit wird dabei nicht schwächer, sondern früher.** Ein
mitgeschickter `pfad` ist jetzt ein **unbekanntes Feld** und wird an der
Argumentprüfung abgewiesen, bevor das Werkzeug anläuft; vorher lehnte
erst die Ausführung ihn ab. Die Meldung nennt die Felder, die es gibt,
und daraus lernt ein Modell im nächsten Schritt.

`kein_werkzeug_hat_einen_optionalen_parameter` hält die Regel, in beiden
Ansageformen und in beide Richtungen: nichts Optionales, und nichts
Verlangtes, das es gar nicht gibt.

`myl-client` **0.15.0 auf 0.16.0** (120 Prüfungen).

### v0.21.0 – 2026-09-10 (drei Fehlerberichte aus dem ersten echten Lauf)

**Alle drei kamen vom Projektinhaber, aus dem laufenden Fenster**, und
keiner davon war durch eine Textprüfung zu finden.

📌 **Fund 286: Die Überlegung stand in der Befehlsliste.** Nach einem
Werkzeugaufruf fängt das Modell neu an zu überlegen; dieser zweite
Denkblock landete unter „Befehle ausgeführt". Die Ursache war eine
zweite Lesart derselben Antwort: `ohne_aufrufe` schnitt die
Werkzeugaufrufe heraus und den Denkblock stehen. **Gelesen wird jetzt
mit demselben Zerleger wie im laufenden Strom**, und `Schritt::Denken`
ist eine eigene Art. Im Fenster wird daraus ein eigener Block, und die
Blöcke stehen in der Reihenfolge, in der sie entstanden sind.

📌 **Fund 287, und er ist der schwerere: Die Erzeugung kannte kein
Ende.** Sie rechnete stur bis zur Tokengrenze, auch wenn das Modell
längst fertig war. Bei 600 Token schrieb Qwen3-4B seine Antwort zu
Ende, setzte `<|im_end|>`, dann `<|endoftext|>` und **erfand danach ein
ganzes Gespräch weiter**, samt zweitem Nutzer und nacherzähltem
Werkzeugergebnis.

⚑ **Der Zuschnitt der fertigen Antwort schnitt das ab, die laufende
Anzeige nicht**, und genau daran fiel es auf. Schlimmer: Im Agentenlauf
ging der erfundene Text als Modellantwort in die nächste Runde.

Das Modell hält jetzt an seinen eigenen Endmarken, hergeleitet aus dem
Wortschatz und nicht hingeschrieben: `<|im_end|>` und `<|endoftext|>`
unter ChatML, `<|endoftext|>` bei einem Basismodell. Nur was zu **einem**
Token wird, zählt; sonst wäre ein Halt auf `<` ein Halt mitten im Text.

📌 **Fund 288: Eine Prüfung verlangte genau den Fehler.**
`der_laufende_text_ist_die_antwort` prüfte `antwort_token == 24`, also
die Grenze selbst. Das war eine Aussage über das alte Verhalten, und sie
fiel, sobald das Modell richtig aufhörte. Jetzt prüft sie, dass etwas
erzeugt wurde und nichts über der Grenze.

`myl-client` **0.14.0 auf 0.15.0** (117 Prüfungen), `myl-oberflaeche`
**0.19.0 auf 0.20.0** (26).

### v0.20.0 – 2026-09-10 (das Fenster schreibt mit, statt am Ende einen Block hinzulegen)

**Antwort und Agentenlauf erscheinen jetzt, während sie entstehen.**
Dazu Überlegung und Befehle als zwei Klappen am Beitrag, und ein
Agentengespräch ohne Arbeitsordner führt zur Einstellung, statt ohne
Werkzeuge loszulaufen.

⚑ **Die Naht geht durch vier Kisten, und jede meldet nur, was sie
weiß.** Die Laufzeit meldet Token, das Modell macht daraus getrennte
Stücke Überlegung und Antwort, die Schleife meldet Werkzeuge, der Rücken
schickt beides über **einen** Kanal ans Fenster. Keine dieser Stellen
kennt die nächste.

| Wer | Was er meldet |
|---|---|
| `runtime` | jedes Token, sobald es dasteht |
| `myl-client` | daraus Überlegung und Antworttext, getrennt |
| `myl-local-agent` | Schritt, Werkzeugaufruf, Ergebnis, Ablehnung |
| Rücken | alles zusammen, in einer Reihenfolge |

📌 **Die Marken kommen zerrissen an**, und das ist die eigentliche
Schwierigkeit: `</think>` trifft als `</`, `think`, `>` ein. Der
Zerleger hält deshalb genau so viel zurück, wie der Anfang einer Marke
lang sein kann, und gibt alles davor endgültig frei. Eine abgebrochene
Marke am Ende ist Text und kein Nichts.

⚑ **Und er sitzt im Modell und nicht im Fenster.** Nur dort ist bekannt,
wann eine Antwort endet: Beim Agenten liegen zwischen zwei Antworten
Werkzeugaufrufe, und ohne diesen Abschluss verschwänden die letzten
Zeichen jeder Antwort.

⚑ **Die Rückgabe schreibt den laufenden Beitrag fertig und ersetzt ihn
nicht.** Sie trägt Text und Schrittliste, aber nicht die Überlegung; die
kam nur über den Kanal. Wer den Beitrag ersetzte, löschte sie vor den
Augen des Nutzers.

📌 **Und die Prüfung der Schrittarten war zum vierten Mal in dieser
Datei eine Liste von Hand** (Fund 285). `hinweis` stand darin, obwohl
der Rücken diese Art längst nicht mehr erzeugt, und die Marketabelle im
Fenster trug den Namen mit; deshalb blieb die Prüfung grün. Sie liest
die Arten jetzt aus dem Rücken, und dabei fiel auf, dass `plan` seit
jeher **keine Regel** hatte.

`myl-client` **0.13.0 auf 0.14.0** (112 Prüfungen), `myl-oberflaeche`
**0.18.0 auf 0.19.0** (26 Prüfungen).

### v0.19.0 – 2026-09-10 (die Hardwarefreigabe bekommt einen echten Bezug)

**Die Einstellungsseite scannt jetzt die Maschine und stellt je
Betriebsmittel einen Schieberegler.** Kerne, Arbeitsspeicher, Platte
und **jedes gefundene Rechenwerk einzeln**, mit dem Höchstwert aus dem
Scan statt aus einer Vorgabe: Ein Regler, dessen Ende jemand geraten
hat, lässt entweder etwas verschenken oder etwas versprechen, das die
Maschine nicht hat.

⚑ **Es ist eine Freigabe und keine Einstellung**, und deshalb hat es
diese Form. Ein Schieber, der nur örtlich etwas abschaltet, ist eine
Einstellung; einer, der dem Netz etwas zusagt, ist ein Versprechen, und
aus der Summe dieser Versprechen leitet das Netz später sein Budget ab.
Die zweite Frage trägt beide Hälften, die erste nur die örtliche.

**Was jeder Regler heute wirklich tut:**

| | |
|---|---|
| **Kerne** | begrenzt den Rechenpfad, sofort beim Schieben. Er ändert nie das Ergebnis: Jede Ausgabezeile ist ein eigenes Skalarprodukt, zwischen den Zeilen gibt es keine gemeinsame Zwischensumme |
| **Arbeitsspeicher** | ein Artefakt darüber wird **gar nicht erst geladen**. Die Schranke sitzt in `Oertlichesmodell::laden` und nicht daneben: Es gibt sieben Aufrufer, und eine Prüfung neben ihnen wäre an sechs Stellen richtig und an der siebten vergessen |
| **Platte** | der Platz wird **wirklich belegt**, solange das Fenster offen ist, und vor jedem Download wird gerechnet, ob es hineinpasst |
| **Rechenwerke** | ausgegraut, mit dem Satz daneben, was dafür geschrieben werden muss |

⚑ **Die Plattenfreigabe hält Platz und deckelt ihn nicht nur.** Eine
Obergrenze sagt „ich nehme mir nicht mehr als das"; sie sagt nicht „das
gehört mir". Eine belegte Datei sagt das Zweite. Gemessen: acht
Gibibyte Freigabe nehmen dem System acht Gibibyte weg, und beim
Schliessen kommen sie zurück.

📌 **`set_len` allein hätte nicht getragen**, und das ist der ganze
Punkt des Moduls: Unter Windows bucht `SetEndOfFile` die Blöcke
wirklich, unter Linux und macOS entstünde eine Datei mit Löchern, die
null Bytes belegt und in jedem Verzeichnislisting wie eine Reservierung
aussieht. Unter Unix wird deshalb ausdrücklich vorbelegt.
`die_reservierung_belegt_bloecke_und_keine_loecher` misst die Blöcke
und nicht die Länge; die Gegenprobe meldet „ist 64 MiB lang, belegt
aber 0".

⚑ **Und die Rechnung, ohne die eine Reservierung gegen den eigenen
Download arbeitet:** belegt plus reserviert ist die Freigabe,
durchgehend. Vor einem Download gibt die Reservierung her, was er
braucht; die Summe bleibt gleich.

📌 **Der Schalter „Beschleuniger benutzen" ist entfallen.** Er
beantwortete die Frage für alle Rechenwerke zugleich, und ein Rechner
mit zwei Karten konnte damit nicht sagen, dass er die eine hergibt und
die andere behält. Eine Freigabe über null **ist** die Erlaubnis.

📌 **Und die Wurzel von Fund 280 ist ausgeräumt statt geflickt.**
`Einstellungen::wert` ist der Gegenpart zu `setzen`, und die
Feldname-zu-Wert-Zuordnung im Fenster ist ersatzlos entfallen.
`wert_und_setzer_kennen_dieselben_felder` fährt jedes Feld einmal hin
und zurück. **Ein Setzer ohne Leser ist eine halbe Naht.**

`myl-client` **0.12.0 auf 0.13.0** (103 Prüfungen), `myl-oberflaeche`
**0.17.0 auf 0.18.0** (24 Prüfungen). Neu direkt eingebunden: `libc`,
nur unter Unix, und die Abhängigkeitsfläche wächst dabei um **null
Kisten**, denn es stand vorher schon durchgereicht in beiden
Sperrdateien.

### v0.18.0 – 2026-09-10 (was in einer öffentlichen Datei nichts zu suchen hat, und drei Zahlen, die auseinanderliefen)

📌 **Acht Stellen dieser Komponente nannten ein internes Dokument**
(Fund 275), und eine davon ist die schwerste Bauart: In `ui/stil.css`
stand ein vollständiger Pfad in ein Verzeichnis, das kein Klon
mitbekommt, und diese Datei geht mit **jedem Bündel** hinaus. Zwei
weitere standen im Fensterkopf selbst, also in Text, den ein Nutzer
liest, dem die Planpapiere dieses Projekts nie zu Gesicht kommen. Was
jetzt dasteht, ist die Aussage: „Dem Klienten fehlen Knotenadresse und
Vollmacht."

📌 **Das Bündelskript trug seine Fassung als festen Text** (Fund 276).
`buendeln-macos.sh` schrieb `0.4.0` in die `Info.plist`, während die
Kiste bei 0.16.0 stand: zwölf Anhebungen zu wenig, in genau dem Bündel,
das ein Mensch doppelklickt, und das örtliche Freigabeskript gab es so
weiter. Das über die CI freigegebene `.dmg` war nie betroffen, es
entsteht aus `tauri.conf.json`. **Die Fassung wird jetzt gelesen**, und
`das_buendelskript_liest_die_fassung` prüft die Ableitung statt der
Zahl: Sie fällt, sobald wieder eine Zahl dasteht, und nicht beim
nächsten Sprung.

📌 **Dieselbe Zahl stand an drei Orten und war dreimal verschieden**
(Fund 278): „an sechzehn Stellen" hier, „an zweiundzwanzig Stellen"
zwei Papiere weiter, gezählt waren es neunundzwanzig. Sie ist gestrichen
statt berichtigt, denn wie oft eine Datei einen Modulnamen nennt, ändert sich
mit jeder Bearbeitung und sagt über die Zusage nichts. Was etwas sagt,
ist die Zahl der **Befehle**, und `jeder_befehl_ist_angemeldet` hält sie
seit heute in einer fünften Richtung: gegen den Satz in diesem
Dokument. Dazu behauptete der Modulkopf des Rückens
„sechsundfuenfzig Pruefungen", und es waren neunundachtzig.

📌 **Und die Einstellungsseite zeigte drei ihrer zwölf Felder falsch an**
(Fund 280). Die Zeilen entstehen aus der Feldliste der Kiste, die
**Werte** kamen aus einer zweiten, von Hand gepflegten Zuordnung im
Fenster, und die kannte `kap.beschleuniger`, `kap.speicher` und
`kap.platte` nicht. In JavaScript ist ein fehlender Schlüssel kein
Fehler, sondern `undefined`: Der Schalter stand **immer aus**, die
beiden Textfelder **immer leer**, gleichgültig was in der Ablage stand.

⚑ **Ohne Wirkung heisst nicht ohne Wert.** Dass diese drei Grenzen noch
nichts bewirken, sagt ihr Hinweissatz, und das ist der richtige Ort
dafür; ihr gespeicherter Wert ist davon unberührt. Eine Anzeige, die ihn
verschweigt, beantwortet die Frage „habe ich das gesetzt?" verkehrt.

⚑ **Gehalten wird jetzt die ganze Kette und nicht ihr erstes Glied.**
`jedes_feld_zeigt_seinen_wert` geht vom Feldnamen in der Kiste über den
Eintrag in der Zuordnung bis zu dem Feld der Ansicht, aus dem er liest:
Ein Eintrag, der auf ein Feld zeigt, das es nicht gibt, ist wieder
`undefined` und sieht im Fenster genauso aus.

`myl-oberflaeche` **0.16.0 auf 0.17.0**, dreiundzwanzig Prüfungen statt
einundzwanzig.

### v0.17.0 – 2026-09-09 (die Oberfläche läuft: Punkt 1.8 zu, und alles, was der erste echte Start zutage gebracht hat)

📌 **`hidden` war schwächer als die Anzeigeart, zum dritten Mal.** Das
Vorgabestilblatt setzt `[hidden] { display: none }` mit der schwächsten
Spezifität; jede eigene `display`-Angabe gewinnt dagegen. Das
Kontextmenü stand nach jedem Start links oben und ließ sich nicht
schließen. ⚑ Im Stilblatt standen bereits **zwei** Einzelflicken dafür;
niemand hatte daraus eine Regel gemacht. Jetzt eine für alle, gehalten
von `verstecktes_bleibt_versteckt`.

⚑ **Der Ausgabeordner ist eine Einstellung und keine Vorgabe.** Ein
Ort, den niemand gewählt hat, ist einer, an dem niemand sucht. Ein
fehlender Ordner ist deshalb keine Fehlermeldung, sondern eine fehlende
Entscheidung: Das Fenster öffnet die Einstellungen, hebt das Feld hervor
und schreibt in einen Kasten darüber, warum.

📌 **Ein `section` in der Linsenliste hat die halbe Einstellungsseite
zum Leuchten gebracht.** Der Glanz beim Überfahren lag auf `.glas`,
`.eingabefeld`, `button:not(.blank)` **und `section`**. Das klang
harmlos und war es nicht: `#einstellungsseite` liegt auf `inset: 0`
über dem ganzen Fenster, `#modellbau` füllt zwei Drittel davon, und
beide bekamen Ring und Glanz. ⚑ **Die Regel dahinter: Glas tragen
Dinge, die man drücken kann.** Knopf, Eingabefeld, Karte. Ein
Behälter, der nur Platz einteilt, trägt keines. Gehalten von
`glas_traegt_nur_bedienelemente`, das die Selektoren mit `var(--mx`
nimmt, acht Behälternamen darunter verbietet und verlangt, dass die
Linsenliste im Skript dieselbe Menge ist.

⚑ **Die Beschriftung eines Einstellungsfeldes ist kein Feldname.** Die
Seite zeigte in der linken Spalte `kap.beschleuniger`,
`agent.bezeugtes`, `modell.artefakt`. Das ist kein Deutsch, sondern
eine Kennung. ⚑ **Der Ausweg, den es nicht geworden ist:** eine
Übersetzungstabelle im Skript, die auseinanderläuft, sobald in der
Kiste ein Feld dazukommt. Stattdessen ist `FELDER` von
`[(&str, Feldart); 12]` auf `[Feld; 12]` gewachsen, mit `bereich`,
`titel` und `hinweis`; `felder` reicht die Struktur durch, das Fenster
zeichnet nur. Die Reihenfolge in der Kiste ist die Anzeige, und der
Bereichswechsel ergibt sich daraus, also braucht auch die Gliederung
keine zweite Liste. Vier Bereiche, zwölf Zeilen, jede mit einem Satz
darunter, was sie bewirkt und was ohne Angabe gilt.

⚠️ **Und beim Schreiben dieser Sätze fiel auf, dass drei Felder nichts
tun.** Von den vier Grenzen dieses Rechners wird genau eine angewendet,
`kap.kerne`; `beschleuniger`, `speicher` und `platte` werden gespeichert,
angezeigt, und danach liest sie niemand. Ein Schieber, der aussieht wie
eine Grenze und keine ist, ist eine Behauptung. Solange die Felder
dastehen, steht **„Noch ohne Wirkung"** dabei. 📌 Ein Satz derselben
Sorte war auch der erste Entwurf zu `agent.bezeugtes`: „Erlaubt
zusätzlich Werkzeuge …" klang, als käme etwas zu einem Bestand hinzu,
während in Wahrheit **alle** Werkzeuge dieses Rechners bezeugt sind und
ohne den Schalter gar keines läuft.

⚑ **Und die sichtbaren Zeichenketten tragen jetzt Umlaute.** Bis dahin
stand im Fenster `Gespraeche`, `Loeschen`, `laedt`. Die Umschrift
`ae`/`oe`/`ue`/`ss` ist eine Regel für Commit-Titel und Quelltext,
nicht für das, was ein Mensch auf dem Schirm liest. Zwanzig
Zeichenketten umgestellt; Kommentare und Bezeichner bleiben.

⚑ **Die Verzeichnisfelder lassen sich im Fensterdialog wählen.**
Artefakt, Arbeitsordner und Ausgabeordner tragen einen Knopf neben dem
Eingabefeld; getippt werden darf der Pfad weiter, der Knopf nimmt nur
den Zwang weg. Welche Felder das sind, sagt die Kiste: `Feldart::Ordner`
ist dazugekommen, und das Verhältnis von `Ordner` zu `Pfad` ist dasselbe
wie das von `Zahl` zu `Grenze`, beide meinen ein Verzeichnis, aber nur
eines lässt sich wegnehmen. ⚑ **Gerufen wird `tauri-plugin-dialog` aus
Rust und nicht aus dem Fenster**, dann braucht die Webansicht keine neue
Berechtigung, kein JS-Paket und keinen von Hand nachgebauten Aufruf; die
Erlaubnisliste bleibt bei ihrem einen Eintrag. 📌 Der Befehl ist `async`,
weil Tauri Befehle ohne `async` auf dem Hauptfaden ausführt und
`blocking_pick_folder` dort auf eine Antwort wartete, die nur der
Hauptfaden geben kann. ⚑ Auf macOS ist die Auswahl zugleich die
Freigabe, was den Zugriffsfall entschärft: Ein getippter Pfad unterhalb
von Schreibtisch oder Dokumenten bekommt `ENOENT`, ein gewählter nicht.

📌 **Ein angemeldeter Befehl, den niemand ruft.** `modell(artefakt)` lud
ein ganzes Artefakt, druckte dessen Vorlage und warf es weg; abgelöst
hat ihn `modell_laden`. Gefunden hat ihn der neue Wächter
`jeder_befehl_ist_angemeldet` in seiner vierten Richtung. Die ersten
drei schließen die Lücke, an der auch der Start scheiterte:
`#[tauri::command]` allein tut nichts, fehlt der Name in
`generate_handler!`, übersetzt alles sauber und der Aufruf scheitert
erst beim Klicken. Die vierte ist die Gegenrichtung: Ein angemeldeter
Befehl ist eine Zusage an das Fenster, und eine Zusage, die niemand
einlöst, ist eine Behauptung.

📌 **Eine Prüfung mit handgepflegter Liste, zum zweiten Mal.**
`jede_klasse_aus_dem_skript_hat_eine_regel` hieß so, tat es aber nicht:
Sie hielt elf Namen von Hand. Als die Marke `grenze` entfiel, fiel die
Prüfung **wegen einer Klasse, die es nicht mehr gibt**. Derselbe Fehler
wie bei den Bewegungsregeln, dieselbe Behebung: Sie liest die Namen
jetzt aus dem Skript, und auf ganze Namen statt auf Teilzeichenketten,
denn `contains("grenze")` fand das Wort „Obergrenze" in einem
Kommentar.

`myl-oberflaeche` **0.13.0 auf 0.14.0**. Neuer Job `oberflaeche` in
`release.yml`: `.dmg`, `.msi`, `.deb` und AppImage, vier Ziele, alles
mit Prüfsumme in derselben Datei wie die Binärprogramme.

📌 **Zwei Fallen beim Bündeln, beide grün und trotzdem falsch.** Ein
`.app` ist ein Verzeichnis, und der Sammelschritt läuft mit
`find -type f`: Es wäre in Einzelteile zerlegt hochgeladen worden. Und
ein Bündelschritt, der nichts erzeugt, endet mit null, also wäre eine
Freigabe ohne Oberfläche von einer mit Oberfläche nicht zu
unterscheiden gewesen. Auf macOS wandert jetzt nur das `.dmg` hinaus,
und der Sammelschritt zählt, was er gefunden hat.

⚑ **Und ein dritter Punkt, umgekehrt:** `veroeffentlichen` hing an
beiden Jobs, ein fehlgeschlagenes `.msi` hätte also die geprüften
Linux-Binärprogramme zurückgehalten. Jetzt läuft die Veröffentlichung,
sobald `bauen` steht; ein Schritt zählt die fünf erwarteten Bündel und
schreibt fehlende **in die Freigabenotiz**.

📌 **Der Download wäre auf jedem richtig eingerichteten Klon
fehlgeschlagen.** `artefakt_bauen` fährt zwei Skripte, und nur das
zweite bekam die Kalibrier-Umgebung in den `PATH`. `fetch_model.sh`
bricht ohne `hf` ab, und dieser Befehl liegt in `calibrate/.venv/bin/`
und nicht im System. Der Knopf hätte mit der Aufforderung abgebrochen,
zu installieren, was schon da ist. Dieselbe Wurzel im zweiten Gewand:
`voraussetzungen()` suchte `hf` ebenfalls im Systempfad. Beide Aufrufe
teilen sich jetzt `pfad_mit_venv`, gehalten von
`jedes_bauskript_bekommt_die_umgebung_in_den_pfad`.

📌 **Das Symbolverzeichnis enthielt nur PNG.** Der Bündler braucht für
das `.msi` ein `.ico` und für das `.dmg` ein `.icns`.
Eine Ableitung erzeugt beide aus `icon.png`, ohne neue
Abhängigkeit, und `buendeln-macos.sh` leitet das Symbol nicht mehr ein
zweites Mal ab.

📌 **Die Bündelversion war eine andere als die der Kiste** (`0.1.0`
gegen `0.13.0`), und der Dateiname jedes Bündels kommt aus der ersten
Zahl.

📌 **Der entfallene Ortsschalter hatte fünf CSS-Regeln dagelassen.** Die
neue Prüfung `jede_regel_hat_ein_element` geht die Gegenrichtung zu den
zwei vorhandenen Klassenprüfungen und fand neben `.schalter` noch
`.klein`. ⚑ Sie hätte dabei fast etwas kaputtgemacht: Sie meldete auch
`.beitrag.von-nutzer` als tot, obwohl die Klasse als
`` `beitrag von-${b.von}` `` vergeben wird. Ein Löschen hätte
Nutzerbeiträge stillschweigend linksbündig gemacht.

⚑ **Die Marke ist neu, und sie ist die alte viermal.** Auf Vorlage des
Projektinhabers: dieselbe goldene Spirale, je um neunzig Grad gedreht
und an den Achsen aneinandergesetzt. Vier goldene Rechtecke im Windrad
ergeben ein Quadrat, die Leinwand folgt also der Figur.

⚑ **Alle Linien tragen dieselbe Deckkraft und dieselbe Breite**, auf
Festlegung des Projektinhabers. Frueher waren die aeusseren Boegen
schwaecher gedeckt, um die Vorlage nachzubilden. Eine Marke steht aber
bei zweiunddreissig Pixeln, auf hellem und dunklem Grund und als
Programmsymbol, und eine schwach gedeckte Linie verschwindet dort
einfach.

⚑ **Der grosse Kreis aussen wird nicht gezeichnet, er entsteht.** Die
Figur liegt so, dass der Mittelpunkt des aeussersten Bogens im
Drehpunkt sitzt; damit liegen alle vier aeussersten Boegen auf
demselben Kreis, und vier Viertel im Abstand von neunzig Grad
schliessen ihn. 📌 Drei Anlaeufe davor behandelten ihn als eigenes
Ding, einbeschrieben, als Umkreis, als zu Ende gezeichneten Bogen je
Spirale, und alle drei sahen aufgelegt aus statt zugehoerig. Sobald er
aus den Boegen hervorgeht, ist der Uebergang tangentenstetig, weil es
derselbe Kreis ist.

⚑ **Die vier inneren Enden laufen weiter, mit derselben Regel wie der
Rest.** Tangentenstetigkeit heisst: Am Uebergang liegen beide
Mittelpunkte auf derselben Geraden durch den Punkt. Nach aussen legt
das den neuen Mittelpunkt fest, nach innen genauso, nur mit dem
Halbmesser geteilt durch Phi statt mal Phi. Jeder Bogen endet damit
genau da, wo der vorige beginnt.

📌 **Zwei Entwuerfe davor rechneten mit Naeherungen und sahen abgehackt
aus.** Der erste nahm eine Aehnlichkeitsabbildung aus den zwei
**innersten** Boegen; weil die Fibonacci-Folge mit 1, 1 beginnt, war
das eine reine Drehung, und vier Boegen wiederholten sich sechzehnmal
an derselben Stelle. Der zweite nahm sie aus den zwei aeussersten, wo
sie stimmt, und setzte sie am innersten an, wo sie es nicht tut: Der
Anschluss sass daneben. ⚑ **Die Strichbreite bleibt dabei gleich, und das Auge laeuft
in einen vollen Punkt aus.** Die Windungen ruecken um 1/Phi zusammen,
der Strich bleibt gleich breit, also schliessen sie sich irgendwann zu
einer Flaeche. Genau das zeigt eine Spirale, die nicht aufhoert: Sie
wird nicht duenner, sie wird nur unaufloesbar. Zwei Entwuerfe davor
haben das vermieden, erst mit einem mitschrumpfenden Strich, dann mit
einem frueheren Abbruch, und beide liessen das Auge stumpf enden statt
es zu schliessen.

Die Fibonacci-Quadrate sind aus der Marke verschwunden; sie sind
Hilfslinien und gehoeren in die Herleitung. Die Leinwand ist damit
quadratisch statt golden, die **Breite** aber unveraendert, denn an ihr
haengt die Sperrung des Schriftzugs.

⚑ **Die Gespraeche tragen ein Menue auf den Rechtsklick**: umbenennen,
exportieren, loeschen. Ein Menue fuer alle Zeilen, nicht eines je
Zeile, und ausserhalb der Seitenleiste, denn die traegt
`overflow: hidden` fuer ihren Bildlauf. Umbenannt wird an Ort und
Stelle statt in einem Dialog; ausgegeben wird Markdown neben die
Einstellungen, samt der Werkzeugschritte eines Agentenlaufs.

📌 **Der Bildlauf lag eine Ebene zu tief.** Die Gespraechsliste trug
ihren eigenen, der Modus stand daneben; wurde die Liste lang, war der
Modus nicht mehr erreichbar, und ein Bildlauf im Bildlauf erwischt mit
dem Rad immer den falschen. Jetzt scrollt **ein** Behaelter zwischen
Marke und Modellwahl, waehrend Kopf und Fuss stehen.

⚑ **Der Glaseffekt malt eine Flaeche und keine Kante.** Drei Anlaeufe
hatten ein wanderndes Licht auf den Rand legen wollen, alle drei auf
demselben `::after`, und stritten sich um `inset` und `background`.
Die jetzige Fassung ist ein `radial-gradient` ueber die ganze Flaeche,
dessen Mitte in Prozent aus der Zeigerstelle kommt: `inset: 0`, keine
Maske, kein Hintergrundfilter, kein Ueberstand. Sie kann bauartbedingt
nicht verrutschen.

📌 **Das Buendel und das Programm liefen auseinander.**
`buendeln-macos.sh` setzte ein gebautes Programm voraus und kopierte,
was dalag; erneuert wurde beim Uebersetzen nur das Programm. Wer per
Doppelklick startet, startet das Buendel und sieht den alten Stand.
Das Skript baut jetzt selbst, bevor es buendelt.

📌 **macOS meldet eine abgelehnte Ordnerfreigabe als „No such file or
directory".** Liegt der Klon unter `Desktop`, `Documents` oder
`Downloads`, gibt der Kernel `ENOENT` zurueck, damit ein Programm nicht
einmal erfaehrt, dass es den Ordner gibt. Die Meldung nennt jetzt den
Grund und den Weg dorthin.

📌 **Jeder Ring der Glasoptik sass um zwei Pixel daneben.**
`* { box-sizing: border-box }` trifft keine Pseudoelemente, und beide
Ringe liegen auf `::before` und `::after` mit `inset: 0; padding: 1px`.
Im `content-box`-Modell kommt das Padding aussen dazu. Behoben mit
`*, *::before, *::after`, bewacht von
`die_ringe_rechnen_im_randkasten`.

📌 **Die Gespraechsliste liess sich nicht scrollen**, obwohl
`overflow-y: auto` dastand: Ein Flex-Kind hat `min-height: auto` und
schrumpft nicht unter seinen Inhalt. `overflow` allein scrollt nichts,
es braucht eine Hoehe.

📌 **„Modell laden" scheiterte aus dem Finder heraus.** In den
Einstellungen steht ein **relativer** Artefaktpfad, und macOS gibt
einem aus dem Finder gestarteten Programm das Arbeitsverzeichnis `/`.
`wurzel_suchen` sucht jetzt zusaetzlich vom Ort des Programms aus, und
ein relativer Pfad wird gegen die Wurzel absolut gemacht. Von der
Kommandozeile fiel es nie auf, weil das Arbeitsverzeichnis dort stimmt.

📌 **Die Oberflaeche ist nie gelaufen, und keine der sechzehn Pruefungen
hat es gemerkt.** Beim ersten Doppelklick blieb das Fenster am
Vorschaltbild stehen. Die Ursache steht in der zweiten Zeile von
`app.js`: `const { invoke } = window.__TAURI__.core;`. In Tauri v2 gibt
es `window.__TAURI__` nur mit `app.withGlobalTauri` in der
Konfiguration, und das stand dort nie. Der Zugriff wirft **beim
Auswerten des Moduls**, also laeuft danach keine Zeile, es gibt kein
`catch`, und der Vorhang bleibt.

⚑ **Warum es niemandem auffiel:** Die Pruefungen lesen HTML, CSS und
Skript als Text und halten sie gegeneinander. Ob die Bruecke ins
Fenster existiert, entscheidet die Konfiguration.
`wer_die_globale_bruecke_benutzt_muss_sie_anmelden` schliesst genau
diese Luecke. 📌 Die Lehre ist groesser als der Fehler: Eine Sammlung,
die nur Text vergleicht, kann gruen sein, waehrend das Programm nicht
startet.

⚑ **Das Programmsymbol kommt jetzt aus der Marke.** Es trug noch den
Platzhalter von Tauri. Im Symbol faellt der Rahmen weg, denn er
konkurriert mit der abgerundeten Kachel.

📌 **Beide Kisten waren unlizenziert, und die Abhängigkeitsprüfung war
deshalb rot.** Einundzwanzig von dreiundzwanzig Kisten tragen `license`
und `publish`, genau diese zwei nicht: Sie sind nach dem
Lizenzdurchgang vom 2026-09-02 entstanden und wurden nie nachgezogen.
⚑ `publish = false` behebt zugleich den zweiten Fehler,
`found 4 wildcard dependencies`, denn `allow-wildcard-paths` gilt nur
für nicht veröffentlichte Kisten.

📌 **Drei Prüfungen schrieben nach `/tmp`, und dass sie durchkamen, war
Glück.** Auf Windows ist `/tmp/x` laufwerksrelativ, also `C:\tmp\x`;
`schreiben` legt sein Elternverzeichnis an, `fs::write` nicht. Lief die
Prüfung mit `schreiben` zuerst, existierte `C:\tmp` und die andere kam
durch. Der Testläufer entscheidet die Reihenfolge, also war die
Sammlung grün, solange sie Glück hatte. Alle drei benutzen jetzt
`tempfile::tempdir`.

📌 **Die Sperrdatei der Oberfläche war fünf Nebenversionen alt** und
hätte jeden `--locked`-Bau umgeworfen. Neu ist
eine Wache im CI-Lauf: fünfundzwanzig
Sperrdateien in zwei Sekunden, und geprüft wird nicht nur die eigene
Version, sondern **jede** Kiste dieses Repositoriums in **jeder**
Sperrdatei.

Elf neue Prüfungen, alle gegengeprüft. Die Oberfläche steht bei
einundzwanzig, `myl-client` bei neunundachtzig, und `cargo deny` geht
über alle dreiundzwanzig Kisten ohne Fehlschlag, `tauri-plugin-dialog`
und seine Abhängigkeiten eingeschlossen.

### v0.14.0 und früher

Der Gesprächsaufbau, die Startanimation, die Marke, die Werkzeuge mit
Einhängegrenze und Phase 0.
