# client (Nutzer-Client inkl. Wallet)

> **Version:** 0.48.3 (`myl-client` 0.33.1, `myl-oberflaeche` 0.36.3, `myl-console` 0.10.2)
> **Datum:** 2026-09-15
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
| **Ein Gespräch mit einem lokalen Modell** | `myl frage <artefakt> <text>`, oder im Fenster |
| **Die Agentenschleife** | `myl agent`, mit Werkzeugen innerhalb einer Einhängegrenze |
| **Die Werkzeugkiste ist ein Ordner** | Eine Einstellung, ein Pfad: `agent.kistenordner`, ohne Angabe die mitgelieferte Kiste `Base` unter `CLIENT/werkzeugkisten/`. Der **Ordnername** sagt, welche eingebauten Werkzeuge dazukommen: `Base` die fünf Dateiwerkzeuge, `Advanced` zusätzlich `run_command`, ein anderer Name `Base`. Was als Manifest im Ordner liegt, sieht das Modell **ohne Neubau**. ⚑ **`Base` ist die Grundlage jeder Kiste**: Ihre Werkzeuge werden mitgeladen, gestapelt und nicht kopiert; bei gleichem Namen gewinnt die gewählte Kiste. ⛔️ Die fünf Dateiwerkzeuge bleiben kompiliert, weil nur sie die Einhängegrenze einhalten; ein Manifest läuft über die Shell und kann das nicht |
| **Mehrere Stellen auf einmal ändern** | `edit_file` nimmt eine Liste aus `alt` und `neu`. Erst ein Trockenlauf über eine Kopie, dann wird geschrieben: **Entweder alle Stellen oder keine** |
| **Vier Betriebsarten** | Chat und Agent laufen; Knoten und Wallet stehen mit ihrer Begründung da und warten auf das Netz |
| **Modellwahl** | Aus dem Katalog, mit Anzeigenamen statt Verzeichnisnamen. Der Netzeintrag heisst „Netzwerkmodell (API), kostet Inferenz-Credits" und ist gesperrt, solange Knotenadresse und Vollmacht fehlen |
| **Modelle holen und Artefakte bauen** | Aus der Einstellungsseite heraus, mit Ladebalken unter dem angeklickten Modell und einer schliessbaren Meldung, wenn es fertig ist |
| **Einstellungen** | Fünf Bereiche, vierzehn Felder (dreizehn davon im Fenster: das Konsolen-Design wirkt dort nicht) in der Reihenfolge, in der jemand sucht (Sprache, Aktualisierung, Modelle, was dieser Rechner hergibt, dann Modell und Agent), und dazu ein Schieberegler je gefundenem Rechenwerk, jedes mit Beschriftung und einem Satz darunter, was es bewirkt. Art **und** Beschriftung kommen aus der Kiste. Die drei Verzeichnisfelder lassen sich über den Fensterdialog des Systems wählen, getippt werden dürfen sie weiter |
| **Sprache** | Deutsch oder Englisch, umschaltbar in den Einstellungen und sofort wirksam. Feldnamen, Pfade und Modellnamen bleiben, wie sie sind |
| **Aus einem frischen Klon einrichten** | Drei Skripte unter `INSTALL/`, je eines für macOS, NixOS und Windows, mit einer Anleitung je System daneben. Sie prüfen erst, bauen dann, und laden nichts nach |
| **Nach Aktualisierungen sehen** | Auf der Einstellungsseite: Gefragt wird, ob `origin` Änderungen hat, die dieser Klon nicht hat. Eingespielt wird mit `git merge --ff-only` und dem Installationsskript der Plattform |
| **Gespraeche verwalten** | Rechtsklick auf eine Zeile: umbenennen an Ort und Stelle, als Markdown ausgeben, loeschen. Wohin ausgegeben wird, steht in `ausgabe.ordner`; ohne Angabe fuehrt das Fenster dorthin |

⚑ **Die Oberfläche ruft dieselben Funktionen wie die Kommandozeile**,
über zweiundzwanzig Befehle, und startet **keinen einzigen Unterprozess**. `jeder_befehl_ist_angemeldet` hält die vier
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

## Abhängigkeiten

`INTEGER_LLM` (Laufzeit und Artefakte), `AGENT_LAYER` (Werkzeugansage
und Session-Kontrakte). Für die Netzhälfte zusätzlich: `CONSENSUS`
(Ledger-Zustand lesen), `TOKENOMICS` (Burn- und Mint-Fluss,
Credit-Preis), `NETWORKING` (Gateway), `GOVERNANCE` (Abstimmung).

⚑ **Die Reihenfolge hat sich umgedreht.** Der Client war als späte
Komponente geplant, weil er auf die anderen wartet. Er ist
vorgezogen worden, weil ohne ihn niemand sehen kann, ob das lokale
Modell überhaupt etwas taugt, und weil eine Schnittstelle, die kein
Mensch je bedient hat, an den Bedürfnissen vorbei entworfen wird.

## Changelog

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
das von gestern; `INSTALL/installieren-<system>` baut und legt in einem
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
