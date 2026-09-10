# client (Nutzer-Client inkl. Wallet)

> **Version:** 0.29.0 (`myl-client` 0.20.0, `myl-oberflaeche` 0.24.1, `myl-console` 0.1.0)
> **Datum:** 2026-09-10
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

⛑ **Dieser Kopf stand bis zum 2026-09-09 auf „Konzeptphase, noch keine
Umsetzung"**, während zwei Kisten mit sechzehn Prüfungen, einer
Oberfläche und Freigabebündeln dastanden. Ein Statuseintrag, der aus
einer überholten Annahme stammt, schickt den Nächsten in die Irre; er
kostet nichts, wenn er stimmt, und einen halben Tag, wenn nicht.

## Was heute geht

| | |
|---|---|
| **Der Agent in der Konsole** | In ein Verzeichnis gehen, `myelith` tippen. **Dieses Verzeichnis ist der Arbeitsordner**, ohne Schalter und ohne Einstellung; `/model`, `/settings`, `/hilfe`, `/ende` |
| **Ein Gespräch mit einem lokalen Modell** | `myl frage <artefakt> <text>`, oder im Fenster |
| **Die Agentenschleife** | `myl agent`, mit Werkzeugen innerhalb einer Einhängegrenze |
| **Vier Betriebsarten** | Chat und Agent laufen; Knoten und Wallet stehen mit ihrer Begründung da und warten auf das Netz |
| **Modellwahl** | Aus dem Katalog, mit Anzeigenamen statt Verzeichnisnamen. Der Netzeintrag heisst „API, kostet Inferenz-Credits" und ist gesperrt, solange Knotenadresse und Vollmacht fehlen |
| **Modelle holen und Artefakte bauen** | Aus der Einstellungsseite heraus, mit Ladebalken unter dem angeklickten Modell und einer schliessbaren Meldung, wenn es fertig ist |
| **Einstellungen** | Vier Bereiche, elf Felder und dazu ein Schieberegler je gefundenem Rechenwerk, jedes mit Beschriftung und einem Satz darunter, was es bewirkt. Art **und** Beschriftung kommen aus der Kiste. Die drei Verzeichnisfelder lassen sich über den Fensterdialog des Systems wählen, getippt werden dürfen sie weiter |
| **Sprache** | Deutsch oder Englisch, umschaltbar in den Einstellungen und sofort wirksam. Feldnamen, Pfade und Modellnamen bleiben, wie sie sind |
| **Aus einem frischen Klon einrichten** | Drei Skripte in der Wurzel, je eines für macOS, NixOS und Windows. Sie prüfen erst, bauen dann, und laden nichts nach |
| **Nach Aktualisierungen sehen** | Auf der Einstellungsseite: Gefragt wird, ob `origin` Änderungen hat, die dieser Klon nicht hat. Eingespielt wird mit `git merge --ff-only` und dem Installationsskript der Plattform |
| **Gespraeche verwalten** | Rechtsklick auf eine Zeile: umbenennen an Ort und Stelle, als Markdown ausgeben, loeschen. Wohin ausgegeben wird, steht in `ausgabe.ordner`; ohne Angabe fuehrt das Fenster dorthin |

⚑ **Die Oberfläche ruft dieselben Funktionen wie die Kommandozeile**,
über neunzehn Befehle, und startet **keinen einzigen Unterprozess**. `jeder_befehl_ist_angemeldet` hält die vier
Richtungen zusammen: kein Befehl ohne Anmeldung, keine Anmeldung ohne
Befehl, kein Aufruf ins Leere und kein Befehl, den niemand ruft. „Ohne eigene Logik" hiesse sonst, aus einer Textausgabe
für Menschen eine Schnittstelle zu machen, und genau das ist die Sorte
Logik, die hier nicht hingehört.

## Struktur

| Verzeichnis | Zweck |
|---|---|
| `myl-client/` | Die Kiste. Einstellungen, örtlicher Betrieb, Agentenschleife, Werkzeuge mit Einhängegrenze, Türklient. Kommandozeile `myl`. |
| `myl-oberflaeche/` | Die grafische Oberfläche auf Tauri v2. Rücken in Rust, Frontend als reines HTML, CSS und ES-Module: **kein Bündler, keine Node-Werkzeugkette**. |
| `myl-oberflaeche/ui/` | `index.html`, `stil.css`, `app.js`, `netz.js`. Einundzwanzig Prüfungen halten HTML, CSS, Skript und Rücken gegeneinander. |
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
Auftrag. Vier Befehle, mehr nicht: `/model`, `/settings`, `/hilfe`,
`/ende`. **Ein Konsolenprogramm mit zwanzig Befehlen ist eines, dessen
Hilfeseite man liest, statt es zu benutzen.**

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

⛑ **Zwei Befunde aus dem ersten Lauf.** Die laufende Schrittzeile
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

⛑ **Fund 301, und er war ein Loch, das die Installer erst aufgerissen
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

⛑ **Und ein Starter, den es einen halben Tag lang gab.** `Myelith` und
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

⛑ **Und die Prüfung dazu geht bis zur Lizenzdatei.** Der Wert im
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

⛑ **Und was ein Programm vergleicht, wird nie übersetzt:** Feldnamen,
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

⛑ **Die Störbilder hatten `both`, und eine Wache hat es gefangen.**
`nichts_wartet_unsichtbar_auf_eine_animation` sagt: Was gelesen werden
soll, darf nicht auf eine Animation warten. Mit `both` steht das
Element **vor** dem Lauf auf dem Anfangsbild, und das ist beim Kommen
`opacity: 0`. Wo Animationen nicht laufen, wäre die Marke unsichtbar
geblieben. **Der Ruhezustand ist sichtbar, und dabei bleibt es.**

#### Drei Installationsskripte und ein Aktualisierungsknopf

`installieren-macos.sh`, `installieren-nixos.sh` (mit `flake.nix`
daneben) und `installieren-windows.ps1` liegen in der Wurzel. Sie
prüfen erst alles, bauen dann die drei Programme, legen sie ab und
tragen einen Menüeintrag ein.

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

⛑ **Fund 296: Das Skript meldete „alles da" und scheiterte drei Zeilen
später.** Es prüfte `command -v cargo`; ein rustup-Schalter ohne
eingestellte Werkzeugkette liegt im PATH und beantwortet das mit ja.
**Geprüft wird jetzt, ob es läuft**, nicht ob es existiert. Gefunden
beim ersten Probelauf, in einem untergeschobenen Benutzerverzeichnis.

⛑ **Fund 297: Ein fehlgeschlagener Bau beendete das Skript nicht.**
Die Bauschleife lief hinter einer Röhre (`… | while read`), also in
einer Unterschale, und `set -e` greift dort nicht: Es kopierte danach
Dateien, die es nicht gibt. Beide Schleifen laufen jetzt in derselben
Shell.

⛑ **Fund 298: Das Verschieben eines Verzeichnisses hat zwei Werkzeuge
still zerbrochen.** Sie fanden die Wurzel des Repositoriums über
`__file__` und zwei Ebenen aufwärts; nach dem Umzug zeigten die zwei
Ebenen auf einen Zwischenordner. **Das Werkzeug fand daraufhin keine
Datei mehr und meldete Erfolg**, was der schlechtestmögliche Ausgang
ist. Dieselbe Klasse wie Fund 291: Ein Verschieben sieht aus wie eine
Änderung ohne Verhalten und ist manchmal keine.

⛑ **Fund 299: Eine bestehende Datei wurde überschrieben, ohne
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

⛑ **Fund 300: Vier Skripte, vier Listen, und sie waren am ersten Tag
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

⛑ **Die erste Fassung dieser Prüfung lag selbst daneben.** Sie schlug
über `sh CLIENT/myl-oberflaeche/buendeln-macos.sh` an, also über einen
Aufruf, der mit der Liste nichts zu tun hat. Sie sucht jetzt die
**Form** der Liste, ein Paar aus Verzeichnis und Name. Dieselbe Klasse
wie die Wache, die `innerHTML` in einem Kommentar fand: **Wer
Erwähnung für Gebrauch hält, bestraft das Danebenschreiben.**

⛑ **Und dieselbe Liste stand ein zweites Mal im selben Skript**, in
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

⛑ **Die Bedingung deckte genau die Wartezeit ab, die keine ist.** Sie
lautete `b.laufend && !b.text && !schritte.length`, also „läuft und es
ist noch nichts da". Das ist der Augenblick vor dem ersten Token, und
der ist kurz. **Die langen Wartezeiten liegen dazwischen**: zwischen
Befehl und nächster Überlegung, zwischen Überlegung und Antwort. Ein
Ladezeichen, das nur den ersten Abschnitt abdeckt, schweigt in jedem
Abschnitt, in dem jemand tatsächlich wartet.

⚑ **Und es steht am Ende des Beitrags, nicht am Anfang.** Über einem
wachsenden Text sähe es aus, als gehörte es zu etwas Vergangenem;
darunter heisst es „und es geht weiter", was stimmt.

⛑ **Fund 293: Die Prüfung dazu prüfte die Zeile und nicht die Zusage.**
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

⛑ **Vorher sagte sie alles Mögliche:** „der Agent fährt", „Modell
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

⛑ **Drei Punkte und kein Kreisel.** Beide sagen nichts über den
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

⛑ **Es kostet nichts, wenn die Frist falsch liegt.** Wer nach einer
Stunde doch weiterfragt, wartet einmal die Ladezeit ab; der nächste
Auftrag lädt von selbst nach. **Nicht entladen wird mitten in einem
Lauf**, dort wird die Frist neu gestellt. Und ein Modellwechsel gibt das
alte sofort frei: Es antwortet ohnehin nicht mehr.

⛑ **Beim Bauen fiel dabei eine Wache über einen zweiten Block.** Das
Ladezeichen bekam sein eigenes `prefers-reduced-motion`, und
`das_rauschen_gehoert_zur_abbestellbaren_bewegung` sah daraufhin am
falschen Ort nach und meldete Bewegtes als nicht abbestellt. **Es gibt
genau einen solchen Block**, und das steht jetzt daneben.

`myl-oberflaeche` **0.21.0 auf 0.22.0** (31 Prüfungen).

### v0.24.0 – 2026-09-10 (die Artefakte heissen Myelith, und bestehende Einstellungen wandern mit)

Die Artefakte heissen `myelith-4b` statt `qwen3-4b` und so fort; die
Pfade im Klienten sind nachgezogen.

⛑ **Eine Umbenennung ist erst fertig, wenn das Mitgewanderte
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

⛑ **Der naheliegende Weg dorthin wäre `innerHTML` gewesen, und er wäre
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

⛑ **Und der Zerleger hatte beim ersten Lauf eine Endlosschleife.**
`| kaputt |` fängt an wie eine Tabelle, ist keine, und der Absatzzweig
brach an seiner eigenen ersten Zeile ab, ohne weiterzuzählen. **Gefunden
als Hänger und nicht als Fehlschlag**, von der Prüfung, die verlangt,
dass nichts verlorengeht.

⚑ **Während des Laufs bleibt der Text schlicht.** Eine halb angekommene
Marke ist keine Marke: `**` mitten im Strom würde als Fettdruck
aufblitzen und beim nächsten Token verschwinden. Der laufende Text ist
die Vorschau, der gesetzte das Ergebnis.

⛑ **Ein Verweis wird als Text mit sichtbarem Ziel gezeigt und nicht als
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

⛑ **Gemeldet vom Projektinhaber, und der Bericht ist die Diagnose.**
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

⛑ **Und der Satz, der das Grübeln ausgelöst hat, ist weg.** Die
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

⛑ **Fund 286: Die Überlegung stand in der Befehlsliste.** Nach einem
Werkzeugaufruf fängt das Modell neu an zu überlegen; dieser zweite
Denkblock landete unter „Befehle ausgeführt". Die Ursache war eine
zweite Lesart derselben Antwort: `ohne_aufrufe` schnitt die
Werkzeugaufrufe heraus und den Denkblock stehen. **Gelesen wird jetzt
mit demselben Zerleger wie im laufenden Strom**, und `Schritt::Denken`
ist eine eigene Art. Im Fenster wird daraus ein eigener Block, und die
Blöcke stehen in der Reihenfolge, in der sie entstanden sind.

⛑ **Fund 287, und er ist der schwerere: Die Erzeugung kannte kein
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

⛑ **Fund 288: Eine Prüfung verlangte genau den Fehler.**
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

⛑ **Die Marken kommen zerrissen an**, und das ist die eigentliche
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

⛑ **Und die Prüfung der Schrittarten war zum vierten Mal in dieser
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

⛑ **`set_len` allein hätte nicht getragen**, und das ist der ganze
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

⛑ **Der Schalter „Beschleuniger benutzen" ist entfallen.** Er
beantwortete die Frage für alle Rechenwerke zugleich, und ein Rechner
mit zwei Karten konnte damit nicht sagen, dass er die eine hergibt und
die andere behält. Eine Freigabe über null **ist** die Erlaubnis.

⛑ **Und die Wurzel von Fund 280 ist ausgeräumt statt geflickt.**
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

⛑ **Acht Stellen dieser Komponente nannten ein internes Dokument**
(Fund 275), und eine davon ist die schwerste Bauart: In `ui/stil.css`
stand ein vollständiger Pfad in ein Verzeichnis, das kein Klon
mitbekommt, und diese Datei geht mit **jedem Bündel** hinaus. Zwei
weitere standen im Fensterkopf selbst, also in Text, den ein Nutzer
liest, dem die Planpapiere dieses Projekts nie zu Gesicht kommen. Was
jetzt dasteht, ist die Aussage: „Dem Klienten fehlen Knotenadresse und
Vollmacht."

⛑ **Das Bündelskript trug seine Fassung als festen Text** (Fund 276).
`buendeln-macos.sh` schrieb `0.4.0` in die `Info.plist`, während die
Kiste bei 0.16.0 stand: zwölf Anhebungen zu wenig, in genau dem Bündel,
das ein Mensch doppelklickt, und das örtliche Freigabeskript gab es so
weiter. Das über die CI freigegebene `.dmg` war nie betroffen, es
entsteht aus `tauri.conf.json`. **Die Fassung wird jetzt gelesen**, und
`das_buendelskript_liest_die_fassung` prüft die Ableitung statt der
Zahl: Sie fällt, sobald wieder eine Zahl dasteht, und nicht beim
nächsten Sprung.

⛑ **Dieselbe Zahl stand an drei Orten und war dreimal verschieden**
(Fund 278): „an sechzehn Stellen" hier, „an zweiundzwanzig Stellen"
zwei Papiere weiter, gezählt waren es neunundzwanzig. Sie ist gestrichen
statt berichtigt, denn wie oft eine Datei einen Modulnamen nennt, ändert sich
mit jeder Bearbeitung und sagt über die Zusage nichts. Was etwas sagt,
ist die Zahl der **Befehle**, und `jeder_befehl_ist_angemeldet` hält sie
seit heute in einer fünften Richtung: gegen den Satz in diesem
Dokument. Dazu behauptete der Modulkopf des Rückens
„sechsundfuenfzig Pruefungen", und es waren neunundachtzig.

⛑ **Und die Einstellungsseite zeigte drei ihrer zwölf Felder falsch an**
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

⛑ **`hidden` war schwächer als die Anzeigeart, zum dritten Mal.** Das
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

⛑ **Ein `section` in der Linsenliste hat die halbe Einstellungsseite
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
dastehen, steht **„Noch ohne Wirkung"** dabei. ⛑ Ein Satz derselben
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
Erlaubnisliste bleibt bei ihrem einen Eintrag. ⛑ Der Befehl ist `async`,
weil Tauri Befehle ohne `async` auf dem Hauptfaden ausführt und
`blocking_pick_folder` dort auf eine Antwort wartete, die nur der
Hauptfaden geben kann. ⚑ Auf macOS ist die Auswahl zugleich die
Freigabe, was den Zugriffsfall entschärft: Ein getippter Pfad unterhalb
von Schreibtisch oder Dokumenten bekommt `ENOENT`, ein gewählter nicht.

⛑ **Ein angemeldeter Befehl, den niemand ruft.** `modell(artefakt)` lud
ein ganzes Artefakt, druckte dessen Vorlage und warf es weg; abgelöst
hat ihn `modell_laden`. Gefunden hat ihn der neue Wächter
`jeder_befehl_ist_angemeldet` in seiner vierten Richtung. Die ersten
drei schließen die Lücke, an der auch der Start scheiterte:
`#[tauri::command]` allein tut nichts, fehlt der Name in
`generate_handler!`, übersetzt alles sauber und der Aufruf scheitert
erst beim Klicken. Die vierte ist die Gegenrichtung: Ein angemeldeter
Befehl ist eine Zusage an das Fenster, und eine Zusage, die niemand
einlöst, ist eine Behauptung.

⛑ **Eine Prüfung mit handgepflegter Liste, zum zweiten Mal.**
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

⛑ **Zwei Fallen beim Bündeln, beide grün und trotzdem falsch.** Ein
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

⛑ **Der Download wäre auf jedem richtig eingerichteten Klon
fehlgeschlagen.** `artefakt_bauen` fährt zwei Skripte, und nur das
zweite bekam die Kalibrier-Umgebung in den `PATH`. `fetch_model.sh`
bricht ohne `hf` ab, und dieser Befehl liegt in `calibrate/.venv/bin/`
und nicht im System. Der Knopf hätte mit der Aufforderung abgebrochen,
zu installieren, was schon da ist. Dieselbe Wurzel im zweiten Gewand:
`voraussetzungen()` suchte `hf` ebenfalls im Systempfad. Beide Aufrufe
teilen sich jetzt `pfad_mit_venv`, gehalten von
`jedes_bauskript_bekommt_die_umgebung_in_den_pfad`.

⛑ **Das Symbolverzeichnis enthielt nur PNG.** Der Bündler braucht für
das `.msi` ein `.ico` und für das `.dmg` ein `.icns`.
Eine Ableitung erzeugt beide aus `icon.png`, ohne neue
Abhängigkeit, und `buendeln-macos.sh` leitet das Symbol nicht mehr ein
zweites Mal ab.

⛑ **Die Bündelversion war eine andere als die der Kiste** (`0.1.0`
gegen `0.13.0`), und der Dateiname jedes Bündels kommt aus der ersten
Zahl.

⛑ **Der entfallene Ortsschalter hatte fünf CSS-Regeln dagelassen.** Die
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
schliessen ihn. ⛑ Drei Anlaeufe davor behandelten ihn als eigenes
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

⛑ **Zwei Entwuerfe davor rechneten mit Naeherungen und sahen abgehackt
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

⛑ **Der Bildlauf lag eine Ebene zu tief.** Die Gespraechsliste trug
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

⛑ **Das Buendel und das Programm liefen auseinander.**
`buendeln-macos.sh` setzte ein gebautes Programm voraus und kopierte,
was dalag; erneuert wurde beim Uebersetzen nur das Programm. Wer per
Doppelklick startet, startet das Buendel und sieht den alten Stand.
Das Skript baut jetzt selbst, bevor es buendelt.

⛑ **macOS meldet eine abgelehnte Ordnerfreigabe als „No such file or
directory".** Liegt der Klon unter `Desktop`, `Documents` oder
`Downloads`, gibt der Kernel `ENOENT` zurueck, damit ein Programm nicht
einmal erfaehrt, dass es den Ordner gibt. Die Meldung nennt jetzt den
Grund und den Weg dorthin.

⛑ **Jeder Ring der Glasoptik sass um zwei Pixel daneben.**
`* { box-sizing: border-box }` trifft keine Pseudoelemente, und beide
Ringe liegen auf `::before` und `::after` mit `inset: 0; padding: 1px`.
Im `content-box`-Modell kommt das Padding aussen dazu. Behoben mit
`*, *::before, *::after`, bewacht von
`die_ringe_rechnen_im_randkasten`.

⛑ **Die Gespraechsliste liess sich nicht scrollen**, obwohl
`overflow-y: auto` dastand: Ein Flex-Kind hat `min-height: auto` und
schrumpft nicht unter seinen Inhalt. `overflow` allein scrollt nichts,
es braucht eine Hoehe.

⛑ **„Modell laden" scheiterte aus dem Finder heraus.** In den
Einstellungen steht ein **relativer** Artefaktpfad, und macOS gibt
einem aus dem Finder gestarteten Programm das Arbeitsverzeichnis `/`.
`wurzel_suchen` sucht jetzt zusaetzlich vom Ort des Programms aus, und
ein relativer Pfad wird gegen die Wurzel absolut gemacht. Von der
Kommandozeile fiel es nie auf, weil das Arbeitsverzeichnis dort stimmt.

⛑ **Die Oberflaeche ist nie gelaufen, und keine der sechzehn Pruefungen
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
diese Luecke. ⛑ Die Lehre ist groesser als der Fehler: Eine Sammlung,
die nur Text vergleicht, kann gruen sein, waehrend das Programm nicht
startet.

⚑ **Das Programmsymbol kommt jetzt aus der Marke.** Es trug noch den
Platzhalter von Tauri. Im Symbol faellt der Rahmen weg, denn er
konkurriert mit der abgerundeten Kachel.

⛑ **Beide Kisten waren unlizenziert, und die Abhängigkeitsprüfung war
deshalb rot.** Einundzwanzig von dreiundzwanzig Kisten tragen `license`
und `publish`, genau diese zwei nicht: Sie sind nach dem
Lizenzdurchgang vom 2026-09-02 entstanden und wurden nie nachgezogen.
⚑ `publish = false` behebt zugleich den zweiten Fehler,
`found 4 wildcard dependencies`, denn `allow-wildcard-paths` gilt nur
für nicht veröffentlichte Kisten.

⛑ **Drei Prüfungen schrieben nach `/tmp`, und dass sie durchkamen, war
Glück.** Auf Windows ist `/tmp/x` laufwerksrelativ, also `C:\tmp\x`;
`schreiben` legt sein Elternverzeichnis an, `fs::write` nicht. Lief die
Prüfung mit `schreiben` zuerst, existierte `C:\tmp` und die andere kam
durch. Der Testläufer entscheidet die Reihenfolge, also war die
Sammlung grün, solange sie Glück hatte. Alle drei benutzen jetzt
`tempfile::tempdir`.

⛑ **Die Sperrdatei der Oberfläche war fünf Nebenversionen alt** und
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
