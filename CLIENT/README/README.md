# client (Nutzer-Client inkl. Wallet)

> **Version:** 0.16.0 (`myl-client` 0.11.1, `myl-oberflaeche` 0.15.0)
> **Datum:** 2026-09-09
> **Status:** ✅ **Der lokale Betrieb läuft und ist ausgeliefert.** Ein
> Gesprächsfenster mit Modellwahl, Agentenschleife und
> Einstellungsseite; aus einem frischen Klon lassen sich darüber
> Modelle laden und Artefakte bauen. Freigabebündel für macOS, Windows
> und Linux, jedes mit Prüfsumme.
>
> ❌ **Die Netzhälfte steht aus.** Knotenzustand, Läufe, Verifikations­
> stufe je Segment und der Kapazitätsschalter warten auf ein
> erreichbares Netz (Fahrplan 2.2 bis 2.5b).

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
| **Ein Gespräch mit einem lokalen Modell** | `myl frage <artefakt> <text>`, oder im Fenster |
| **Die Agentenschleife** | `myl agent`, mit Werkzeugen innerhalb einer Einhängegrenze |
| **Vier Betriebsarten** | Chat und Agent laufen; Knoten und Wallet stehen mit ihrer Begründung da und warten auf das Netz |
| **Modellwahl** | Aus dem Katalog, mit Anzeigenamen statt Verzeichnisnamen. Der Netzeintrag heisst „API, kostet Inferenz-Credits" und ist gesperrt, solange Knotenadresse und Vollmacht fehlen |
| **Modelle holen und Artefakte bauen** | Aus der Einstellungsseite heraus, mit Ladebalken unter dem angeklickten Modell und einer schliessbaren Meldung, wenn es fertig ist |
| **Einstellungen** | Je Feld ein Bedienelement, die Art kommt aus der Kiste |
| **Gespraeche verwalten** | Rechtsklick auf eine Zeile: umbenennen an Ort und Stelle, als Markdown ausgeben, loeschen |

⚑ **Die Oberfläche ruft dieselben Funktionen wie die Kommandozeile**,
an zweiundzwanzig Stellen, und startet **keinen einzigen
Unterprozess**. „Ohne eigene Logik" hiesse sonst, aus einer Textausgabe
für Menschen eine Schnittstelle zu machen, und genau das ist die Sorte
Logik, die hier nicht hingehört.

## Struktur

| Verzeichnis | Zweck |
|---|---|
| `myl-client/` | Die Kiste. Einstellungen, örtlicher Betrieb, Agentenschleife, Werkzeuge mit Einhängegrenze, Türklient. Kommandozeile `myl`. |
| `myl-oberflaeche/` | Die grafische Oberfläche auf Tauri v2. Rücken in Rust, Frontend als reines HTML, CSS und ES-Module: **kein Bündler, keine Node-Werkzeugkette**. |
| `myl-oberflaeche/ui/` | `index.html`, `stil.css`, `app.js`, `netz.js`. Sechzehn Prüfungen halten HTML, CSS und Skript gegeneinander. |
| `myl-oberflaeche/icons/` | Symbole. `icon.ico` und `icon.icns` erzeugt `werkzeuge/symbole.py` aus `icon.png`; von Hand nachbessern hilft nicht. |
| `README/Fahrplan-v1.md` | Der Fahrplan mit allen Punkten, Funden und dem Changelog. |

## Ausliefern

`.github/workflows/release.yml` hat zwei Bau-Jobs:

- **`bauen`** liefert `myl`, `myl-node` und `myl-test` für **fünf**
  Ziele. ⚑ Jedes Binärprogramm belegt vor der Veröffentlichung, dass es
  richtig rechnet: Der Konformitätslauf über **siebzehn** Vektoren
  läuft mit genau diesem Programm auf dem Rechner, der es gebaut hat.
- **`oberflaeche`** bündelt die grafische Oberfläche: `.dmg` für macOS,
  `.msi` für Windows, `.deb` und AppImage für Linux x86_64, `.deb` für
  arm64.

Örtlich für die eigene Maschine: `werkzeuge/freigabe.sh`.

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
Betriebssystem** (Fahrplan 3.1). Das ist der Unterschied zwischen einer
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

### v0.16.0 – 2026-09-09 (die Oberfläche läuft: Punkt 1.8 zu, und alles, was der erste echte Start zutage gebracht hat)

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
`werkzeuge/symbole.py` erzeugt beide aus `icon.png`, ohne neue
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
`werkzeuge/sperrdateien.py` und ein CI-Job dazu: fünfundzwanzig
Sperrdateien in zwei Sekunden, und geprüft wird nicht nur die eigene
Version, sondern **jede** Kiste dieses Repositoriums in **jeder**
Sperrdatei.

Vier neue Prüfungen, alle gegengeprüft. Die Oberfläche steht bei
sechzehn, `myl-client` bei siebenundachtzig, und `cargo deny` geht über
alle dreiundzwanzig Kisten ohne Fehlschlag.

### v0.14.0 und früher

Siehe `README/Fahrplan-v1.md`, Abschnitt Changelog: der Gesprächsaufbau,
die Startanimation, die Marke, die Werkzeuge mit Einhängegrenze und
Phase 0.
