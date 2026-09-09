# client (Nutzer-Client inkl. Wallet)

> **Version:** 0.15.0 (`myl-client` 0.11.1, `myl-oberflaeche` 0.14.0)
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

### v0.15.0 – 2026-09-09 (Punkt 1.8 zu: die Oberfläche liegt der Freigabe bei)

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
