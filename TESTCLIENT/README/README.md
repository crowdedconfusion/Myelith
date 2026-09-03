# testclient (`myl-testclient`)

> **Version:** 0.21.0
> **Datum:** 2026-09-02
> **Status:** Phase 1 und **Phase 3 vollständig**, dazu Punkt 2.1
> (`vergleich`), **2.2** (Backend-Vergleich innerhalb einer Maschine, seit
> dem 2026-08-30) und 2.4 (`--repeat`); **Phase 4 vollständig** (4.3 die
> Fremdmaschinen-Automatik, 4.1 der Konformitätslauf als fünfte Stufe,
> 4.2 die Maschinenbeschreibung im Protokoll). Offen bleibt in Phase 2
> der Lauf selbst, also die zweite Architektur, und 2.3 (bestätigte
> Ergebnisse ablegen). 278 Tests grün,
> alle Läufe gegen die echten Artefakte verifiziert. Der
> Fremdmaschinen-Test ist auf einem nachgebauten frischen Klon gefahren
> (aarch64/macOS); Windows und der Weg über Modellbeschaffung und
> Artefaktbau stehen weiter aus.
>
> **Drei Funde am Messgerät selbst (2026-08-22).** Sie betreffen nicht das
> Netz, sondern das Werkzeug, mit dem es geprüft wird, und alle drei
> hätten einen bestandenen Nachweis geliefert, den es nicht gab:
> **Fund 34** meldete `cpu-simd` als eigenen Rechenpfad auf x86_64, wo
> keiner existiert; **Fund 35** urteilte `NACHWEIS` über einen Lauf, der
> nach einem von sechs Prompts abgebrochen war; **Fund 36** hashte nur die
> erzeugten Token und übersah damit Rechenabweichungen, solange kein
> Argmax kippte. Einzelheiten unten.

Terminal-Testclient: Hardwaretests auf heterogener Hardware und
geshardete Inferenz: jeder Lauf mit einem Protokoll, das zwischen
Maschinen und Modellständen vergleichbar bleibt.

## Aufgabe

Zwei Lücken, die das Projekt bisher offen hatte:

1. **Der Cross-Hardware-Determinismus-Nachweis** (Whitepaper Kap. 6.2)
   ist als projektweiter offener Punkt geführt. Er verlangt, dass
   derselbe Prompt auf verschiedenen Architekturen und Backends
   **bitgleiche** Ergebnisse liefert. Bisher gab es dafür kein Werkzeug,
   das auf einer fremden Maschine ohne Einarbeitung läuft.
2. **Die geshardete Inferenz war nur als Integrationstest sichtbar.**
   `myl-pod` kann einen Pod fahren, aber nur als Bibliothek. Der Client
   macht daraus einen Befehl, dessen Ausgabe man einem Dritten zeigen
   kann, und der gegen die Einzelknoten-Runtime gegenprüft.

## Der Kern: das Protokoll

Ein Testlauf ohne Protokoll ist wertlos. Er beantwortet dann nicht, was
bei einem abweichenden Ergebnis als Erstes zu klären ist: **auf welcher
Hardware, mit welchem Backend, gegen welchen Modellstand?** Genau diese
drei Angaben entscheiden bei einem Modellwechsel darüber, ob ein
verändertes Ergebnis Fortschritt oder Fehler ist.

Jeder Lauf schreibt deshalb **immer** zwei Dateien nach `logs/`:

| Datei | Zweck |
|---|---|
| `<lauf-id>.jsonl` | Eine JSON-Zeile je Ereignis, stabile Feldnamen und Reihenfolge, die Fassung, die zwischen Maschinen gediffed wird |
| `<lauf-id>.log` | Dieselben Ereignisse als Fließtext, für die Fehlersuche am Terminal |

**Flach in `logs/`, benannt nach Teilnehmer und Einstellungen:**

```text
logs/
├── anna_12a1e91e_2026-08-21_143022.jsonl
├── anna_12a1e91e_2026-08-21_143022.log
└── bjoern_12a1e91e_2026-08-21_150411.jsonl
```

Die Kurzkennung ist der Hash genau der Parameter, die gleich sein müssen
(Prompts, Token, Shards, Modell). **Alle Teilnehmer eines Testplans
tragen dieselbe Einstellungs-Prüfsumme**, auf jeder Maschine. Wer
versehentlich andere Parameter nimmt, ist am Dateinamen sofort
erkennbar. Der Name davor beantwortet die zweite Frage des
Koordinators: von wem stammt diese Datei.

Dieselben Angaben stehen **auch im Protokoll**: `run_started` trägt
Befehl, Teilnehmer und Einstellungs-Kennung. Der Dateiname ist eine
Bequemlichkeit; die Zuordnung leisten die Daten, denn eine Datei wird
umbenannt, ein Feld nicht.

**Ein Protokoll je Testlauf, nicht eines je Stufe.** Hardware,
Determinismus, geshardete Inferenz und Protokoll-Durchlauf sind eine
Messung. Vier Dateien wären vier Teilaussagen, die der Koordinator wieder
zusammensetzen müsste, und beim Verschicken geht die eine verloren, die
den Befund trägt.

## `vergleich`: vom Protokoll zum Urteil

```bash
myl-test vergleich
```

Liest die zugesandten Protokolle aus **`TESTCLIENT/Vergleiche/`**,
gruppiert nach Prüflauf und Einstellungs-Kennung und stellt jeden
Vergleichswert gegenüber. Mit `--logs <ordner>` auch über ein anderes
Verzeichnis. Je Gruppe ein Urteil:

| Urteil | Bedeutung |
|---|---|
| `NACHWEIS` | Fingerabdrücke verschieden, Werte gleich, Modellstand gleich |
| `KEIN NACHWEIS (eine Maschine)` | Werte gleich, aber alles von derselben Maschine |
| `UNVERGLEICHBAR (Modellstand)` | θ_v oder Ankerdigest weichen ab: **kein** Hardware-Befund |
| `ABWEICHUNG` | Gleicher Modellstand, gleiche Eingabe, verschiedene Ergebnisse |
| `ZU WENIG PROTOKOLLE` | Weniger als zwei mit derselben Kennung |
| `UNVOLLSTÄNDIG (Lauf nicht zu Ende)` | Ein Lauf ist abgebrochen, mit Fehlern beendet, oder deckt andere Vergleichswerte ab als die übrigen |

**Der Befehl verweigert den Nachweis, wenn alle Protokolle denselben
Hardware-Fingerabdruck tragen.** Das ist ein Akzeptanzkriterium des
Akzeptanzkriterium und keine Höflichkeit: Ein Werkzeug, das zwei gleiche Werte von
derselben Maschine als Nachweis ausgibt, wäre schlimmer als keines, weil
sein Ergebnis geglaubt wird.

**Der Modellstand wird vor den Digests geprüft.** Bei verschiedenen
Modellen *müssen* die Werte verschieden sein; das als Determinismusfehler
zu melden wäre genau die Verwechslung, gegen die es `artefakte` gibt. Zum
Modellstand zählt seit v0.8.0 auch der **Digest-Umfang**: Zwei Protokolle,
deren Vergleichswerte verschiedene Dinge abdecken, messen nicht dasselbe
(Fund 36).

**Unvollständige Läufe zählen nicht (Fund 35, 2026-08-22).** Verglichen
wird je Wert nur unter den Protokollen, die ihn **haben**. Ein Lauf, der
nach dem ersten von sechs Prompts abbrach, stimmte deshalb in allem
überein, was er noch erreicht hatte, und fehlte im Rest unbemerkt; das
Urteil lautete `NACHWEIS`. Maßgeblich ist jetzt der Abschlusseintrag
`run_finished`, und nicht das Fehlen eines Abbruchvermerks: Strg-C und ein
geschlossenes Fenster beenden den Prozess, ohne dass noch etwas
geschrieben wird. Die Datei sieht dann tadellos aus, jede Zeile ist
vollständig, und nur die letzte fehlt. Werte, die nicht alle Läufe tragen,
stehen im Bericht mit `·` statt `=` und mit der Angabe, wie viele
beigetragen haben.

Exit-Code 0 nur dann, wenn jede Gruppe den Nachweis trägt: damit taugt
der Befehl für die CI.

**Zwei Ordner, und die Trennung ist der Punkt:**

```text
TESTCLIENT/Vergleiche/            Eingabe: die zugesandten .jsonl
TESTCLIENT/Vergleiche/Berichte/   Ausgabe: vergleich_<datum>_<uhrzeit>.md
```

Der Vergleich liest **alles**, was er an `.jsonl` findet. Läge er über dem
eigenen Protokollverzeichnis, mischten sich die zugesandten Läufe mit den
eigenen, und ein Urteil über eine Gruppe, in der die eigene Maschine
mehrfach steckt, sagt etwas anderes aus, als es zu sagen scheint. Der
Bericht landet aus demselben Grund eine Ebene tiefer: neben seiner
Eingabe würde ihn der nächste Aufruf mitlesen.

Der Bericht trägt, was auf dem Bildschirm keinen Platz hat:
**vollständige** Digests statt der Kurzform, Dateinamen, Artefakt-Digest
je Teilnehmer, Zeitpunkt. Er ist die Fassung, die weitergereicht wird.
Ein **Laufprotokoll** schreibt `vergleich` dagegen nicht: Er misst nichts,
er wertet aus.

## Testplan, die Datei, die der Koordinator verteilt

Damit „alle nehmen exakt dieselben Werte" keine Bitte bleibt:

```bash
# Koordinator:
myl-test plan --plan-id 2026-08-21-cross-arch-01 --model qwen2.5-0.5b \
  --prompt "Die Hauptstadt von Frankreich ist" \
  --prompt "The capital of France is" \
  --steps 32 --shards 4 \
  --out "TESTCLIENT/Testpläne/cross-arch.plan"

# Teilnehmer:
myl-test --name anna --plan cross-arch.plan determinismus
```

**`--prompt` ist mehrfach angebbar.** Ein einzelner Prompt übt einen
einzigen Pfad durch das Modell aus; ein Rundungsfehler, der nur bei
langen Sequenzen oder in einem selten getroffenen LUT-Bereich auftritt,
bliebe unentdeckt, und der Vergleichswert sähe trotzdem beruhigend aus.
Je Prompt entsteht ein Einzelwert, darüber ein Gesamtwert.

**Fünf Pläne liegen bei**, keiner davon an ein Modell gebunden:

| Datei | Prompts × Token | Was er ausübt |
|---|---|---|
| `standard.plan` | 6 × 32 | Der Regelfall: englische und deutsche Prosa, Fachtext, Rechenaufgabe, lange Faktenzeile |
| `standard-kurz.plan` | 4 × 16 | Dasselbe kürzer, für langsame Modelle. Wer 7B testet, fängt hier an |
| `benchmark-1-zahlen.plan` | 7 × 24 | Ziffernfolgen, Überträge, Zehnerpotenzen, Einheiten. Zwei Schreibweisen derselben Aufgabe ergeben völlig verschiedene Tokenfolgen |
| `benchmark-2-sprachen.plan` | 8 × 24 | Vier lateinische Schriften plus Chinesisch, Griechisch, Kyrillisch, dazu Umlaut und Eszett: weit auseinanderliegende Bereiche der Embedding-Tabelle und Mehrbyte-Token |
| `benchmark-3-code-kontext.plan` | 6 × 32 | Rust, Python, JSON, SQL (Klammer-Konsistenz über viele Token) und zwei lange Prompts, die die Generierung auf hohe Positionen schieben: RoPE und KV-Cache dort, wo die Winkel groß werden |

**Was diese Pläne nicht sind: Genauigkeitsmessungen.** Der Client
vergleicht Digests, er bewertet keine Antworten. Ob ein Modell richtig
rechnet, beantwortet `INTEGER_LLM/eval` über die Perplexität gegen die
Gleitkomma-Referenz. Die Benchmark-Pläne führen das Modell an
ungewöhnliche Stellen, **damit** die Bitgleichheit dort geprüft wird und
nicht nur auf dem eingefahrenen Pfad: Fund 15 (RoPE) und Fund 16
(Attention nur auf den ersten Key) fielen bei kurzen Prompts kaum auf.

Die Datei trägt eine Prüfsumme über Prompts, Token, Shards und Modell.
Wird sie verändert, **verweigert der Client den Lauf** (Exit-Code 3)
statt einen abweichenden Digest zu liefern, der wie ein Befund
aussieht. Der Prompt steht in Anführungszeichen, damit auch ein
Randleerzeichen erhalten bleibt.

Kommentarzeilen dürfen frei ergänzt werden: sie gehen nicht in die
Prüfsumme ein. `plan_id` ebenfalls nicht: Zwei Koordinatoren mit
demselben Test unter verschiedenen Namen sollen vergleichbare
Ergebnisse bekommen.

Im Menü: Nutzerpunkt [2] wählt einen Plan, Entwicklerpunkt [9] [6] erzeugt einen.

**Prompttexte werden gehasht, nicht gespeichert.** Testprotokolle wandern
per Copy-Paste in Tickets und Chats; ein Prompt, der dabei mitwandert,
ist eine Datenschutzlücke, die niemand beabsichtigt hat.

**Der Fingerabdruck beschreibt eine Hardware-Klasse, kein Gerät.** Keine
Seriennummern, keine MAC-Adressen, keine Hostnamen: abgesichert durch
einen Test, der Feldnamen und Wertformate prüft.

## Aufruf

Es gibt drei Starter. Sie tun dasselbe: bauen bei Bedarf, finden das
Binary und reichen alle Argumente weiter. Welchen du nimmst, hängt nur
davon ab, womit du arbeitest.

| Datei | Für wen |
|---|---|
| `Myelith Testclient - macOS.app` | macOS, Doppelklick im Finder |
| `Myelith Testclient - Windows (Batch).cmd` | Windows, Doppelklick im Explorer oder Aufruf aus cmd |
| `Myelith Testclient - Linux, macOS (Shell).sh` | Terminal auf Linux und macOS |

Alle drei liegen in diesem Ordner, zusammen mit dem Quelltext unter
`myl-testclient/`, den Protokollen unter `logs/` und den
Helfern für Symbol und Anwendungsmenü unter `werkzeuge/`.

**Sie dürfen aber überall liegen.** Die Starter suchen die
Repository-Wurzel aufwärts anhand von `TESTCLIENT/myl-testclient/Cargo.toml`
statt mit einer festen Verzeichnistiefe zu rechnen. Wer sie ins
Wurzelverzeichnis kopiert, auf den Schreibtisch legt oder verschiebt,
muss nichts anpassen. Die erste Fassung rechnete mit einer festen Ablage
und war beim ersten Verschieben sofort kaputt.

### Der kürzeste Weg

**Doppelklick.** Unter macOS auf das App-Bündel, unter Windows auf
die `.cmd`. Es öffnet sich ein Terminalfenster mit dem Menü; jeder Punkt ist
dort in zwei Zeilen erklärt, und es steht dabei, was ein Modell braucht
und was nicht. Beim ersten Start dauert der Bau einige Minuten, danach
wenige Sekunden.

Wer keine Artefakte hat, wird gefragt, ob die Gewichte von Hugging Face
geholt und die Artefakte gebaut werden sollen. Es passiert nichts ohne
Rückfrage.

### Aus dem Terminal

```bash
cd TESTCLIENT
./"Myelith Testclient - Linux, macOS (Shell).sh"                 # Menü
./"Myelith Testclient - Linux, macOS (Shell).sh" artefakte       # Artefakte prüfen
./"Myelith Testclient - Linux, macOS (Shell).sh" determinismus   # Bitgleichheit
./"Myelith Testclient - Linux, macOS (Shell).sh" --help          # alle Befehle
```

Die Anführungszeichen sind wegen der Leerzeichen im Namen nötig; die
Tabulator-Vervollständigung setzt sie von selbst. Wer den Client oft aus
dem Terminal aufruft, legt sich eine Abkürzung in `~/.bashrc` oder
`~/.zshrc` an:

```bash
alias myl-test='"/pfad/zum/Repository/TESTCLIENT/Myelith Testclient - Linux, macOS (Shell).sh"'
```

Die doppelten Anführungszeichen stehen dabei **innerhalb** der einfachen.
Ohne sie zerfiele der Pfad beim Aufruf an den Leerzeichen in mehrere
Argumente.

Der Starter funktioniert aus jedem Unterverzeichnis, weil er seine eigene
Lage auflöst.

### In welcher Reihenfolge

1. **`artefakte`** zuerst. Der Befehl sagt, ob auf dieser Maschine ein
   Modell liegt und ob es dem veröffentlichten Digest entspricht. Ohne
   diese Prüfung sähe ein abweichendes Artefakt später aus wie eine
   gescheiterte Bitgleichheit, und der Client berichtete das Gegenteil
   dessen, wofür es ihn gibt.
2. **`hardware`** braucht kein Modell und ist der erste sinnvolle Lauf
   auf einer neuen Maschine.
3. **`konformitaet`** prüft die Golden Vectors gegen diesen Bau, also ob
   er bitgleich mit der Referenz rechnet. Die Operations-Vektoren laufen
   immer; die Layer- und E2E-Vektoren nur gegen das Artefakt, mit dem sie
   erzeugt wurden. Lädt nichts herunter.
4. **`determinismus`** und **`shard`** sind die eigentlichen Tests. Beide
   brauchen ein Modell und lösen es selbst auf.
5. **`stack`** geht ohne Modell durch Krypto, Epochenseed, Komiteewahl,
   BFT, Verifikation, Ledger und Tokenomics.

Jeder Lauf schreibt ein Protokoll nach `TESTCLIENT/logs/`,
maschinenlesbar als `.jsonl` und lesbar als `.log`. Für einen Vergleich
zwischen zwei Maschinen zählen diese Dateien, nicht die Bildschirmausgabe.

### Voraussetzungen

Das Repository auf der Platte; alles Weitere richtet sich nach Rückfrage
selbst ein. Fehlt Rust, fragt der Starter und installiert es (Windows
einschließlich der C++-Werkzeugkette); ist cargo nicht da, aber ein
gebautes Binary vorhanden, benutzt er dieses. Fehlt für den Artefaktbau
ein einsatzbereites Python, fragt der Client und legt die virtuelle
Umgebung samt Paketen selbst an. Wer beides ablehnt, bekommt die
Handgriffe von Hand genannt.

### Symbol

Das Zeichen ist das **M** aus der Titelgrafik, zugeschnitten und in drei
Formaten abgelegt: `README/Grafiken/myelith-icon.png`, `.icns` (macOS)
und `.ico` (Windows, sechs Größen von 16 bis 256 px).

Wie es an den Starter kommt, unterscheidet sich je System, und der Grund
ist jeweils eine Grenze des Formats:

- **macOS:** Das Symbol einer Datei liegt in einem erweiterten Attribut.
  Git speichert keine erweiterten Attribute, ein so gesetztes Symbol
  überlebt also weder `git clone` noch `git checkout`. Deshalb liegt hier
  ein **App-Bündel** bei: ein Verzeichnis mit `.icns` darin, versionierbar
  wie jede andere Datei. Es öffnet Terminal.app und führt darin
  den Shell-Starter aus.
- **Windows:** Eine Stapeldatei kann kein Symbol tragen. Eine
  Verknüpfung kann es, speichert aber absolute Pfade und wäre auf jedem
  anderen Rechner kaputt. `TESTCLIENT/werkzeuge/verknuepfung-erstellen.cmd` legt sie
  deshalb dort an, wo sie gebraucht wird.
- **Linux:** `.desktop`-Dateien verlangen absolute Pfade in `Exec` und
  `Icon`. `TESTCLIENT/werkzeuge/desktop-eintrag-erstellen.sh` setzt sie ein und legt
  den Eintrag unter `~/.local/share/applications/` ab.

Zur Schärfe: Das M ist in der Titelgrafik rund 70 px hoch. Größere
Symbolgrößen sind hochskaliert und entsprechend weich. Für eine scharfe
Fassung bräuchte es das Zeichen als Vektor.

**Warum es diese Starter gibt:** Der Client soll auf fremden Maschinen
laufen, oft auf solchen, deren Besitzer mit Rust nichts zu tun hat. Wer
erst herausfinden muss, in welches Verzeichnis er wechseln und welche
Cargo-Flagge er setzen muss, führt den Test seltener aus. Genau diese
Hürde darf ein Test nicht haben, dessen Zweck es ist, auf möglichst
vielen verschiedenen Maschinen zu laufen.

**Direkt über Cargo,** wenn ohnehin eine Rust-Umgebung eingerichtet ist:

```bash
cd TESTCLIENT/myl-testclient
cargo run --release --bin myl-test
```

Das ist der vorgesehene Weg für Testläufe auf fremden Maschinen: Wer
erst eine Hilfeseite lesen muss, führt den Test seltener aus. Das Menü
erklärt jeden Punkt in zwei Zeilen und zeigt, was Artefakte braucht und
was nicht.

**Als Befehl: für Skripte und CI:**

```bash
cd TESTCLIENT/myl-testclient

# Hardware erheben, der erste Befehl auf einer neuen Maschine.
# Braucht kein Modell und keine Artefakte.
cargo run --bin myl-test -- hardware

# Protokoll-Durchlauf über alle Komponenten. Kein Modell nötig, ~1 s.
cargo run --release --bin myl-test -- stack

# Determinismus: derselbe Prompt zweimal, bitgleich?
cargo run --release --bin myl-test -- determinismus --steps 8

# Geshardete Inferenz gegen die Einzelknoten-Runtime.
cargo run --release --bin myl-test -- shard --shards 4 --steps 4
```

`--release` lohnt sich: Der Determinismuslauf dauert im Debug-Build
etwa 40 s je Durchlauf, im Release-Build Bruchteile davon.

Optionen: `--prompt`, `--steps`, `--shards`, `--artifacts`, `--logs`,
`--quiet`. `myl-test --help` zeigt sie mit Erklärung.
`MYL_NO_BANNER=1` unterdrückt das Banner dauerhaft.

## Was der Client abdeckt

| Befehl | Geprüfte Komponenten | Artefakte nötig |
|---|---|---|
| `hardware` |: (nur Erhebung) | nein |
| `stack` | myl-types, -scheduler, -consensus, -verifier, -ledger, -tokenomics | nein |
| `konformitaet` | INTEGER_LLM (kernels, runtime) über die Golden Vectors | teilweise: Operations-Vektoren nie, Layer/E2E nur mit passendem |
| `determinismus` | INTEGER_LLM (runtime, kernels) | **ja** |
| `shard` | COMPUTE_PIPELINE (myl-pod) + INTEGER_LLM | **ja** |

**Nicht abgedeckt:** `myl-net` (Gossip über echte Sockets gehört in die
NETWORKING-Testsuite) und die BFT-Runden selbst: Der Client stimmt nicht
mit, aus der bewussten Grenze heraus, die der Changelog unter v0.14.1
beschreibt. Rundenwechsel und Kettenpersistenz sind im Knotenbetrieb mit
`myl-node` belegt (fünf eigenständige Prozesse, Leaderausfall,
Neustart); offen ist daran nur das Nachholen der Konsensrunde für einen
Knoten, der allein vorauseilt — seine Blöcke holt er nach, seine Runde
nicht. Die vollständige Abgrenzung steht in
[ANLEITUNG.md](ANLEITUNG.md), Abschnitt 5.

## Anleitung für Tests mit mehreren Beteiligten

**[ANLEITUNG.md](ANLEITUNG.md)**: nach Rollen getrennt: Ein Teilnehmer
liest Abschnitt 1 und ist fertig; der Koordinator bekommt die
Urteilstabelle, die Ausschlussfragen bei abweichenden Digests und eine
Meldevorlage. Enthält außerdem, welche Hardware-Kombinationen sich
lohnen und was die Tests **nicht** abdecken.

Kurzfassung auch im Menü unter Punkt 7.

## Cross-Hardware-Nachweis, das Verfahren

```bash
# 1. Auf JEDER Maschine:
myl-test hardware
#    → die Fingerabdrücke MÜSSEN sich unterscheiden.
#      Tun sie es nicht, prüft Schritt 2 nichts.

# 2. Auf JEDER Maschine, mit identischem Prompt:
myl-test determinismus --prompt "..." --steps 8
#    → die Digests MÜSSEN übereinstimmen.
```

**Beides zusammen ist der Nachweis; eines allein ist keiner.** Zwei
gleiche Digests von derselben Maschine belegen nichts, und zwei
verschiedene Maschinen ohne gleichen Digest widerlegen die Kernthese.

## Abgrenzung zu CLIENT

`CLIENT/` ist der spätere Nutzer-Client (Wallet, Inferenz-Oberfläche,
Session-Kontrakte). Dieser hier ist ein **Diagnosewerkzeug für
Entwickler und Miner**: keine Konten, keine Zahlungen, keine
Netzwerkverbindung. Die Trennung ist bewusst: ein Diagnosewerkzeug darf
laut, gesprächig und roh sein; ein Nutzer-Client nicht.

## Abhängigkeiten

INTEGER_LLM (`runtime`, `kernels`), COMPUTE_PIPELINE (`myl-pod`),
SHARED_TYPES (`myl-types`), CONSENSUS (`myl-ledger`, `-scheduler`,
`-consensus`), TOKENOMICS, VERIFICATION, der `stack`-Lauf braucht sie
alle. Fremd-Crates: `sha2`, `borsh` und seit v0.6.0 `crossterm` für die
Pfeiltastenauswahl. Sonst nichts, der Client soll auf einer fremden
Maschine mit möglichst wenig Voraussetzungen bauen, und deshalb sind
Argumentauswertung und JSON-Leser weiterhin von Hand geschrieben.

`crossterm` ist eine bewusste Ausnahme von dieser Linie (Entscheidung des
Projektinhabers) und mit einer Bedingung verbunden: Wo kein Terminal
vorhanden ist oder das Fenster zu klein, fällt die Auswahl auf
zeilenweise Eingabe zurück. Ein Werkzeug, das im Skript auf eine Tastatur
wartet, hängt still.

## Struktur

```
TESTCLIENT/
├── README/
│   ├── README.md             diese Kurzübersicht
│   ├── SCHNELLSTART.md       eine Seite, für den ersten Lauf
│   └── ANLEITUNG.md          Tests mit mehreren Beteiligten
├── Testpläne/                .plan-Dateien des Koordinators
├── logs/                     eigene Laufprotokolle (gitignored)
├── Vergleiche/               zugesandte Protokolle (gitignored)
│   └── Berichte/             Vergleichsberichte (gitignored)
└── myl-testclient/
    ├── src/
    │   ├── lib.rs            Einstieg, Abgrenzung zu CLIENT
    │   ├── main.rs           Argumentauswertung, Hilfetext
    │   ├── logging.rs        Laufprotokolle (JSONL + Text)
    │   ├── hardware.rs       Fingerabdruck (Klasse, nicht Gerät)
    │   ├── banner.rs         ASCII-Banner zum Projektbanner
    │   ├── animation.rs      Startbild: Zeichenregen, dann Logoaufbau
    │   ├── farben.rs         Neonpalette für Schriftzug und Menütitel
    │   ├── banner.rs         Schriftzug und Netzmotiv je Fensterbreite
    │   ├── auswahl.rs        Pfeiltastenauswahl mit zeilenweisem Rückfall
    │   ├── menu.rs           interaktives Menü
    │   ├── artefakte.rs      finden, prüfen, beschaffen, freigeben
    │   ├── plaene.rs         Testpläne im Planordner finden
    │   ├── runs.rs           Hardware, Determinismus, Shards
    │   ├── spec.rs           Testplan (erzeugen, prüfen, laden)
    │   ├── vergleich.rs      Protokolle gegenüberstellen, urteilen, berichten
    │   └── stack.rs          Protokoll-Durchlauf (10 Stufen)
    └── Cargo.toml
```

Die drei Ordner, mit denen ein Teilnehmer zu tun hat: `Testpläne/`,
`logs/`, `Vergleiche/`: liegen bewusst **neben** dem Crate, nicht darin.
Wer sein Protokoll verschicken soll, hat in einem Quellcodeverzeichnis
nichts zu suchen.

## Belegte Läufe (2026-08-22, aarch64/macos, θ_v 0.17.0)

Die Werte aus den Modellläufen stammen aus dem **neuen** Digest-Umfang
(Logits und Token, Fund 36) und sind mit den Werten früherer Fassungen
nicht vergleichbar.

| Lauf | Ergebnis |
|---|---|
| `determinismus --repeat 2`, `reference` gegen `cpu-simd` | derselbe Wert `272f1ee8f45f2c78`, jetzt **über die gerechneten Zahlen**, nicht nur über die erzeugten Token |
| `determinismus --repeat 4`, 2 Prompts × 8 Token | je 4 Läufe bitgleich, 5,7 s |
| Negativprobe: 0,0101 % eines Tensors verändert, Hashkette nachgezogen | alter Digest `e19372337dab1f3d` **unverändert**, neuer Digest `272f1ee8f45f2c78` → `4e34276060530427` |
| Negativprobe: ein einzelnes Byte verändert, Kette **nicht** nachgezogen | Lauf abgelehnt, der betroffene Tensor wird benannt, Exit 1 |
| Abbruch mit SIGINT mitten im Lauf | Protokoll lesbar, letzte Zeile gültiges JSON, kein `run_finished`; `vergleich` kennzeichnet es als unvollständig |
| `determinismus --plan standard` | 6 Prompts × 32 Token, je zwei Läufe **bitgleich**, Gesamtwert `fd64588fd46a7af8…`, 29 s |
| `shard --shards 4 --steps 4` | Pod (Layer 0–6/6–12/12–18/18–24) **bitgleich** zur Einzelknoten-Runtime, Digest `6541c129…` |
| `stack` | 10 von 10 Stufen bestanden in 54 ms, Gesamtwert `8c74519a…` (bis zum 2026-08-27 `a9af743f…`; der Wert hängt am Code, nicht an der Maschine) |
| `vergleich` über zwei Läufe derselben Maschine | Urteil `KEIN NACHWEIS (eine Maschine)`, Exit-Code 1, die Verweigerung greift |

Der Shard-Lauf erfüllt damit das Akzeptanzkriterium aus
COMPUTE_PIPELINE Phase 1: erstmals über einen aufrufbaren Befehl statt
über einen Integrationstest.

## Changelog

### v0.21.0 – 2026-09-03 (die Abbuchung, Ende zu Ende)

`tests/tuer_bis_rechenwerk.rs` bekommt eine zweite Naht:
`eine_gerechnete_anfrage_bucht_credits_ab` fährt eine Anfrage bis zum
Modell und zurück und prüft danach im **Kettenzustand**, dass der
Verbrauch steht, mit Gegenprobe auf die doppelte Buchung derselben
Abrechnung.

### v0.20.5 – 2026-09-03 (ein Nutzeraufruf erreicht das echte Modell)

`tests/harness_bis_modell.rs` fährt den Weg mit den
Qwen2.5-0,5B-Artefakten: HTTP mit Bearer über einen Socket, Vollmacht
gegen Sitzungskontrakt, Beleg, Versiegelung, lokale Leitung,
Entsiegelung, Bindungsprüfung, vier Shards, Wortschatz, zurück als JSON.

**Was ankam:** `"Die Hauptstadt von Frankreich ist Paris"`, zehn
Prompt-Token, acht erzeugte, Segment `2900…` mit der Sitzungsnummer 41
darin.

⚑ **Er hat beim ersten Lauf zwei Fehler gefunden** (Fund 160):
`/v1/models` meldete `"unbekannt"` als Pipeline-Stand, weil die Frage
synchron gestellt wurde, obwohl sie an einen fremden Prozess geht; und
`usage.prompt_tokens` zählte Bytes statt Token.

**Und einen dritten in sich selbst:** Die erste Fassung las das JSON mit
`find` und Indizes und nahm den halben Rumpf als `content`. Jetzt liest
sie mit `serde_json`, **mit demselben Werkzeug, mit dem ein Klient
liest**.

**Ohne Artefakte schlägt er fehl**, mit einem Satz, der sagt was zu tun
ist; wer sie bewusst nicht hat, setzt `MYL_OHNE_ARTEFAKTE=1`. Das ist
Fund 113, und der Wächter ist in beide Richtungen gegengeprüft.

### v0.20.4 – 2026-09-03 (die Naht von der Türklinke bis zum Rechenwerk)

`tests/tuer_bis_rechenwerk.rs` fährt den ganzen Weg: HTTP mit Bearer an
die Tür, Beleg, Versiegelung, lokale Leitung, Entsiegelung,
Bindungsprüfung, Rechenwerk, und zurück als JSON.

⚑ **Der Weg berührt fünf Kisten, und keine sieht mehr als ihren
Nachbarn.** `myl-testclient` ist die einzige Stelle, die alle fünf
sieht, also ist er hier richtig und nirgends sonst. Dieselbe Begründung
wie beim kalten Pfad, und dieselbe Fehlerklasse dahinter: beide Seiten
gebaut, beide für sich geprüft, die Naht fehlt.

**Geprüft wird der Weg und nicht die Inferenz.** Ein Modell hier machte
den Test langsam und sagte über den Weg nichts; die Pipeline hat ihre
eigenen Tests gegen echte Artefakte.

### v0.20.3 – 2026-09-03 (die Naht des kalten Pfades)

**Der einzige Ort, der beide Enden sieht.** Ein echter Knoten aus
`myl-node` und ein echter Shard-Dienst aus `myl-pod`: Die beiden Kisten
kennen einander nicht und sollen es nicht, also kann nur ein Dritter
zeigen, dass sie zusammenpassen.

⚑ **Das ist die Fehlerklasse, die dieses Projekt neunmal getroffen
hat:** beide Seiten gebaut, beide für sich geprüft, die Naht fehlt.

`tests/kalter_pfad.rs` fährt den ganzen Weg: Auftrag über das Netz an
den Knoten, über die lokale Leitung an den Shard, Antwort zurück. Dazu
die Gegenprobe, dass derselbe Knoten **ohne** Ortsleitung ablehnt, und
die, dass er ohne Ausweis gar nicht erst startet.

⚑ **Kein Modell im Test**, und das ist Absicht: Auf dem Prüfstand steht
der **Weg** und nicht die Inferenz. Die Pipeline hat ihre eigenen Tests
gegen echte Artefakte; ein Modell hier machte den Test langsam und sagte
über den Weg nichts.

### v0.20.2 – 2026-09-03 (auch die eigene Tür bleibt hier zu)

Der Knoten öffnet seit `myl-node` v0.31.0 ab Werk eine eigene Tür auf
`127.0.0.1:4160`. Der Testklient fährt mehrere Knoten in einem Prozess,
und ein fester Port lässt sich nur einmal vergeben: Beide Knotenaufbauten
setzen deshalb `tuer: None`, wie schon `beobachtung: None`.

### v0.20.1 – 2026-09-02 (der Beobachtungsendpunkt bleibt hier zu)

Der Knoten öffnet seit `myl-node` v0.28.0 ab Werk einen
Beobachtungsendpunkt auf `127.0.0.1:4151`. **Der Testklient fährt
mehrere Knoten in einem Prozess**, und ein fester Port lässt sich nur
einmal vergeben: Der zweite und jeder weitere bekämen eine Warnung über
einen besetzten Port, die nichts bedeutet. Beide Knotenaufbauten setzen
deshalb `beobachtung: None`.

### v0.20.0 – 2026-08-30 (⚑ Fund 105: der Nachweis ließ sich auf einer einzigen Maschine erzeugen)

⚑ **Fund 105, und er gehört in dieselbe Familie wie 34, 35 und 36:** ein
Werkzeug, das einen bestandenen Nachweis liefert, den es nicht gibt.

`canonical_bytes` lief über **alle** erhobenen Felder, und drei davon
beschreiben nicht die Maschine, sondern den **Bau**:
`backends_compiled`, `backends_rechnend`, `backend_selected`. Damit
genügte ein zweiter `cargo build`:

```
myl-test --name ref-bau konformitaet          # ohne Feature
cargo build --release --features cpu-simd
myl-test --name simd-bau konformitaet         # dieselbe CPU
myl-test vergleich

   ref-bau    aarch64-macos-reference       894d8357ae92b5c1
   simd-bau   aarch64-macos-cpu-simd/neon   894d8357ae92b5c1
   Urteil: NACHWEIS
   Das ist der Cross-Hardware-Determinismus-Nachweis für diese Einstellung.
```

Ein Laptop, zwei Übersetzungen, und das Werkzeug bescheinigt eine Aussage
über Hardware. **Genau so nachgestellt am 2026-08-30**, mit dem echten
Client, nicht am Modell.

**Behoben durch eine Trennung, nicht durch eine weitere Prüfung.** Der
Fingerabdruck deckt seither nur die Maschinenfelder ab; die drei
Bau-Felder bilden einen eigenen Wert `rechenpfad_sha256`. Damit zerfällt
die Frage in die zwei, die sie immer war:

| Maschinen | Rechenpfade | Urteil |
|---|---|---|
| ≥ 2 | beliebig | `NACHWEIS`, der Cross-Hardware-Beleg |
| 1 | ≥ 2 | `RECHENPFAD-NACHWEIS`, der Backend-Vergleich |
| 1 | 1 | `KEIN NACHWEIS (eine Maschine)` |

✅ **Damit ist Punkt 2.2 erledigt**, und zwar als Nebenwirkung: Der
Backend-Vergleich innerhalb einer Maschine brauchte kein neues
Unterkommando, sondern genau diese Unterscheidung. Zurückgestellt war er
mit der Begründung, auf x86_64 gebe es bis zum AVX2-Pfad kein zweites
Backend; auf aarch64 gibt es NEON, und dort trägt der Vergleich heute.
Gemessen: gleicher Konformitätswert `894d8357ae92b5c1` über beide Bauten,
Urteil `RECHENPFAD-NACHWEIS`.

⚑ **Eine Schema-Marke hält zwei Client-Fassungen auseinander.** Ein
`fingerprint_sha256` von vor dem 2026-08-30 deckt eine andere Feldmenge
ab: gleich lang, gleich aussehend, auf derselben Maschine verschieden.
Ohne Marke entstünde Fund 105 ein zweites Mal, nur über zwei Fassungen
statt über zwei Bauten. Protokolle ohne Marke bekommen jetzt
`UNVERGLEICHBAR (Fingerabdruck-Verfahren)` statt eines Urteils.

**Und die Abweichung wird eingegrenzt, ohne einen zweiten Lauf.** Bisher
sagte `ABWEICHUNG` nur, dass die Werte auseinandergehen. Der Sammellauf
trägt den Konformitätswert als eigenen Vergleichswert im selben
Protokoll, und daraus folgt die Einengung unmittelbar:

- Weichen **die Konformitätsvektoren** ab, sitzt der Unterschied
  unterhalb des Modells, in den Kerneln. Nächster Schritt:
  `myl-test konformitaet`, eine Protokollzeile je Vektor.
- Stimmen sie überein, rechnen die Kernel gleich, und der Unterschied
  liegt darüber: Artefakt, Laden, Zuschnitt, Abtastung.
- Fehlt der Wert ganz, sagt der Hinweis genau das, statt zu raten.

Dieselbe Fallunterscheidung steht jetzt auch hinter einer verfehlten
`--erwarte`, also auf der Maschine, auf der es auffiel. 278 Tests grün.

### v0.19.1 – 2026-08-30 (der Durchstich baut Pods ohne Archiv)

`ShardNode` nimmt seit COMPUTE_PIPELINE v0.14.0 keinen `DaStore` mehr
entgegen: Der Shard archiviert nichts, die strittige Aktivierung bringt
im Streitfall der Ankläger mit. Stufe 7 des Durchstichs zieht mit.

### v0.19.0 – 2026-08-29 (gebaute Binaries, und jedes belegt vor der Auslieferung, dass es richtig rechnet)

Wer heute mitmachen will, klont das Repositorium und übersetzt selbst;
der Starter bietet an, Rust zu installieren, wenn es fehlt. Das ist eine
Hürde vor genau dem Test, der ohne Hürde stattfinden soll.

Ein Release-Workflow baut jetzt `myl-test` und `myl-node` für fünf
Ziele: Linux x86_64 und aarch64, Windows x86_64, macOS arm64 und x86_64.
Ausgelöst durch eine Marke `v*` oder von Hand.

### ⚑ Was ihn von einem gewöhnlichen Release-Job unterscheidet

**Jedes Binary belegt vor der Veröffentlichung, dass es richtig
rechnet.** Der Konformitätslauf der sechs Operations-Vektoren läuft mit
genau diesem Binary auf dem Rechner, der es gebaut hat; weicht der
Gesamtwert ab, bricht das Release ab. Ein Binary, das anders rechnet,
ist kein langsames, sondern ein schädliches: Es liefert Segmente, die
von den redundanten abweichen, wird geschlachtet, und bis dahin hat es
den Auftragsstrom verschmutzt.

Dass diese Prüfung durchgehen kann, ist gemessen und nicht gehofft: Ein
auf aarch64/macOS **quergebautes** x86_64-Binary lieferte denselben Wert
wie das native. Zwei Architekturen, ein Wert.

### Drei Grenzen, ausdrücklich

- **`x86_64-apple-darwin` prüft sich nicht selbst.** Der macOS-Runner
  ist arm64 und startet dieses Binary ohne Rosetta nicht. Es wird
  gebaut und ausgeliefert, aber mit einer Datei `UNGEPRUEFT-*.txt`
  daneben, statt es stillschweigend den geprüften gleichzustellen.
- **Die Linux-Binaries binden gegen die glibc des Runners.** Auf einer
  älteren Verteilung starten sie nicht. Eine musl-Zusage, die niemand
  geprüft hat, wäre schlimmer als die fehlende.
- **Prüfsummen sind keine Signaturen.** Sie belegen, dass die Datei
  unverändert ankam, nicht von wem sie stammt.

Jeder Befehl des Workflows wurde einzeln lokal ausgeführt, bevor er
hineingeschrieben wurde, samt Gegenprobe: Mit verfälschtem Wert bricht
der Lauf ab. **Der Workflow selbst ist damit trotzdem ungeprüft**, bis
er das erste Mal läuft; das lässt sich örtlich nicht nachstellen.

### v0.18.0 – 2026-08-29 (die Mindestfassung berichtigt)

`rust-version` nannte `1.85` und war falsch, aus demselben Grund wie in
NETWORKING v0.11.0: die libp2p-Kette. Gemessen, jetzt `1.88`.

⚑ **Für dieses Crate zählt das mehr als für die anderen**, denn es ist
das Stück, das ein Teilnehmer als Erstes übersetzt. Eine falsche Angabe
schickt ihn mit einer Toolchain los, die nicht baut, und der Fehler
zeigt sich als Wand aus Meldungen über fremde Pakete.

### ⚑ Der Konformitätswert wurde nur auf Windows geprüft

Er muss auf jeder Maschine derselbe sein; das ist die Zusage, auf der
das ganze Protokoll steht. Geprüft hat ihn bis heute allein der
Windows-Lauf der CI. **Auf Linux, wo die meisten Knoten laufen werden,
prüfte ihn niemand.**

Der Ubuntu-Lauf prüft ihn jetzt mit, und zwar ohne eigenen
Release-Bau: Der Wert ist profilunabhängig, am 29. August auf
aarch64/macOS in beiden Profilen gemessen und identisch. Ein
Unterschied zwischen den Profilen wäre selbst ein Befund.

Dass macOS keinen eigenen Runner hat, war damit begründet, dass „jeder
Patch dort ohnehin durch die Konformitätsvektoren läuft". Das
beschrieb eine Gewohnheit, keine Prüfung: Der Lauf stand in keiner
Schleife, die jemand ausführen musste. Er steht jetzt in einer. Damit
liegt die These auf zwei Plattformen in der CI und auf der dritten bei
jedem Patch.

### v0.17.3 – 2026-08-29 (der Durchstich baut den Schuldbeleg wie im Betrieb)

Stufe 8 und Stufe 9 wiesen die Schuld einer ausgedachten Kennung zu:
32 Bytes ohne Schlüssel dahinter. Seit VERIFICATION v0.11.0 verlangt ein
Schuldspruch gegen den primären Pod einen unterschriebenen Übergang, und
eine Kennung ohne Schlüssel kann keinen liefern.

Beide Stufen erzeugen den Beleg jetzt so, wie ein Shard ihn erzeugt: Der
Schlüssel unterschreibt den Übergang, die Kennung wird aus dem Schlüssel
abgeleitet. Damit prüft der Durchstich den Weg, den die Sache im Betrieb
nimmt, statt einen, den es dort nicht gibt.

### v0.17.2 – 2026-08-28 (Stufe 7 unterschreibt)

Der Blockdurchlauf legt keine nackte Transaktion mehr in den Block: Er
unterschreibt sie und prüft die eigene Unterschrift nach. **Kein neuer
Prüfschritt, sondern derselbe an einem Typ, der jetzt einen Absender
kennt.**

### v0.17.1 – 2026-08-28 (der Gesamtwert ändert sich, ohne dass hier eine Zeile anders ist)

⚑ **Der Gesamtwert des Protokoll-Durchlaufs geht von `8c74519a11dceae5`
auf `d02dcacb6aa37026`, und diesmal liegt die Ursache vollständig
außerhalb dieses Crates.** `myl-types` v0.6.0 bindet die Blattzahl in
die Merkle-Wurzel (Fund 77), und die erste Stufe des Durchlaufs baut
einen Merkle-Baum über vier Blätter. Kein Testclient-Code ist geändert.

**Belegt statt angenommen:** Mit der alten `merkle.rs` liefert derselbe
Lauf auf derselben Maschine weiterhin `8c74519a11dceae5`; geändert hat
sich allein die Krypto-Stufe, von `d2347febaedfebe9` auf
`504713ed640fe164`. Die übrigen neun Stufenwerte sind gleich geblieben.

**Wer Protokolle über diese Fassung hinweg vergleicht, vergleicht zwei
Codestände**, und `vergleich` meldet das als Befund. Das ist dasselbe
Verhalten wie bei v0.17.0 und aus demselben Grund richtig.

⚑ **Und es ist der Beleg dafür, wofür dieser Wert da ist.** Eine
Änderung an einer Konsens-Primitive in einem anderen Crate wird hier
sichtbar, ohne dass jemand daran gedacht hätte, sie hier einzutragen.
Ein Fingerabdruck, der nur die eigene Komponente abdeckt, hätte
geschwiegen.

⚑ **Fund 78, aufgefallen beim Absichern dieser Fassung: Neun
Test-Hilfsfunktionen bauten feste Temp-Pfade.** `tempdir()` setzte
`$TMPDIR/myl-testclient-<name>` zusammen, ohne Prozesskennung, und
löschte das Verzeichnis beim Betreten. **Zwei gleichzeitige Testläufe
räumen einander damit ab**, und das Ergebnis sind rote Tests ohne
Codefehler. Genau das ist hier passiert: neun Fehlschläge, keiner davon
echt.

**Der Rest des Projekts machte es längst richtig.** `myl-node`,
`myl-net` und die INTEGER_LLM-Runtime hängen `std::process::id()` an;
nur `netz.rs` tat es hier, die übrigen neun nicht. Es war also keine
unbekannte Regel, sondern eine ungleich angewandte.

**Behoben an allen neun Stellen, mit Gegenprobe:** zwei gleichzeitige
Läufe derselben Suite, beide 265/265. Ohne die Behebung fallen dabei
neun Tests. **Ein Fehlschlag ohne Fehler ist teurer als er aussieht**,
denn er lehrt, rote Tests zu deuten statt ihnen zu glauben.

### v0.17.0 – 2026-08-27 (der Protokoll-Durchlauf prüft die Slashing-Staffelung)

⚑ **Der Gesamtwert des Protokoll-Durchlaufs ändert sich mit dieser
Fassung**, von `a9af743fba0e77dc` auf `8c74519a11dceae5`. Das ist kein
Befund über eine Maschine, sondern über den Code, und es sind **zwei**
Ursachen: Der Ledger führt seit `myl-ledger` v0.3.0 eine Verstoßhistorie
je Konto, die in die Zustandsverpflichtung eingeht, und der Blockkopf
trägt seit `myl-consensus` v0.14.0 ein Höhenfeld, das die Blockkodierung
von 137 auf 145 Byte bringt.
**Wer Protokolle über diese Fassung hinweg vergleicht, vergleicht zwei
Codestände** — `vergleich` meldet das als Befund, und zu Recht.

**Die Stufe prüft dafür mehr.** Sie belegt jetzt drei Dinge, die vorher
niemand prüfte: dass ein gebuchtes Urteil beim Schuldigen **gezählt**
wird, dass der gemeldete Stand der Stand **vor** dem Urteil ist, und dass
die Staffelung aus Kap. 5.5 über drei Urteile 1/3/5 % ergibt. Vorher galt
immer die erste Stufe, weil die Zahl der Vorverstöße eine Eingabe war,
die niemand füllte.

### v0.16.0 – 2026-08-27 (Konformitätslauf und Maschinenbeschreibung)

**Der Konformitätslauf ist im Client (Punkt 4.1).** Fünfte Stufe des
Sammellaufs und eigener Unterbefehl `konformitaet`: Er prüft die Golden
Vectors gegen diesen Bau und schreibt wie jeder Lauf eine `.jsonl`, eine
Zeile je Vektor plus Gesamtwert. Die Prüflogik ist aus den beiden
Golden-Binaries in die INTEGER_LLM-Bibliotheken gewandert (`kernels`
und `runtime` tragen sie jetzt, die Binaries sind dünne Starter
geblieben); der Client ruft die Bibliotheken, statt ein zweites
Programm zu starten.

**Die Vektoren wissen jetzt, wofür sie gelten.** Ein Manifest bei den
Vektoren nennt das Modell, mit dem die Layer- und E2E-Vektoren erzeugt
wurden; läuft ein anderes Artefakt dagegen, werden diese Stufen
übersprungen — ausdrücklich und mit Begründung im Protokoll, nicht
still. Ohne Artefakt laufen die sechs Operations-Vektoren, und das
bleibt ein gültiges Protokoll: Der Vergleichswert trägt den Umfang
(`op` oder `op+layer+e2e`), und `vergleich` behandelt zwei verschiedene
Umfänge wie zwei verschiedene Modellstände — unvergleichbar, kein
Hardware-Befund.

**Das Protokoll nennt die Maschine (Punkt 4.2).** CPU-Modell,
Speichergröße, Virtualisierung und bei GPU-Bauten der Kartenname stehen
jetzt im Protokoll. **Nicht im Fingerabdruck:** Zwei baugleiche
Mietmaschinen müssen denselben Fingerabdruck tragen, sonst hielte der
Vergleich zwei gleiche Architekturen für zwei verschiedene und gäbe ein
Urteil, das nichts belegt. Ein Test hält die Bytegleichheit der
kanonischen Fingerabdruck-Bytes fest.

**Eine Gleitkomma-Falle weniger.** Ein Vektor ohne die exp-LUT in den
Metadaten fiel bisher in eine `f64`-Nachbildung zurück; das war gegen
die Ganzzahldisziplin. Solche Vektoren schlagen jetzt begründet fehl;
alle Vektoren des Repositoriums tragen die LUT.

⚑ **Fund 68: Das Menü versprach vier Stufen, der Lauf hatte fünf.** Die
Zahl stand dreimal getippt da — in der Protokollzeile je Stufe, in der
Beschreibung des Menüpunkts [3] und in der Kurzanleitung. Nachgezogen
wurde nur die erste. Wer dem Menü folgte, bekam eine fünfte Stufe, die
niemand angekündigt hatte; für jemanden, der das Werkzeug zum ersten Mal
auf einer fremden Maschine startet, sieht das aus wie ein Fehler. Die
Stufenliste ist jetzt **eine** Quelle: Die Beschreibung des Menüpunkts
entsteht daraus, die Protokollzeile greift darauf zu, und eine sechste
Stufe ohne Eintrag bricht beim ersten Lauf ab, statt still eine falsche
Zahl zu behaupten. Die Kurzanleitung bleibt fester Text, weil sie auf die
Bildschirmhöhe gerechnet ist und ein Test genau das prüft; ein zweiter
verbindet ihre Zahl mit der Liste. **Die Gegenprobe ist gefahren:** Mit
dem alten Wortlaut schlägt er fehl.

⚑ **Fund 69: Die Bauanleitung für die Artefakte war nicht ausführbar.**
Wer die automatische Beschaffung ablehnt oder wessen Lauf scheitert,
bekommt eine Anleitung aus zwei Befehlen — und die beiden brauchen
**verschiedene Arbeitsverzeichnisse**, ohne dass eines dastand. Der
Download legt nach `INTEGER_LLM/models/…` ab, gilt also von der Wurzel
aus; der Bau ruft ein Python-Paket auf, das unterhalb von `INTEGER_LLM`
liegt. Aus der Wurzel ausgeführt endet er in `No module named calibrate`,
nachgestellt und vor der Behebung reproduziert. **Dazu die Schreibweise:**
`VAR=wert befehl` ist eine Eigenheit der Unix-Shell; `cmd` verlangt `set`
in einer eigenen Zeile, PowerShell `$env:VAR`. Das wiegt hier schwerer
als anderswo, weil eine nicht gesetzte Variable **nicht auffällt**: Die
Kalibrierung nimmt dann das Vorgabemodell, läuft durch, und der
Teilnehmer hat ein anderes Artefakt gebaut als das, das er messen wollte.
Ein Fehler, der wie ein Erfolg aussieht. Die Anleitung nennt jetzt je
Schritt das Verzeichnis und die Schreibweise des laufenden Systems;
Verzeichnis, Variablenname und Modulpfad stehen als Konstanten, die
Aufruf und Anleitung teilen. Zwei Tests prüfen beide Fassungen, auch die
Windows-Fassung, die auf der Entwicklermaschine sonst niemand zu Gesicht
bekäme.

**Der Konformitätslauf läuft jetzt auch in der CI unter Windows.** Ohne
Artefakt, also die sechs Operations-Vektoren, und geprüft wird der
Gesamtwert selbst gegen `894d8357ae92b5c1`. Der Grund ist nicht
Vollständigkeit: Ein Windows-Bau, der hier einen anderen Wert erzeugt,
wäre ein Befund über die Kernthese und kein Werkzeugfehler, und er fällt
so **vor** dem Partnertermin auf statt an ihm. Der Lauf schreibt in ein
eigenes Verzeichnis, weil der Schritt daneben prüft, dass `vergleich` ein
**leeres** Verzeichnis ablehnt; läge dort ein Protokoll, prüfte er
stattdessen die Verweigerung bei einer einzigen Maschine, und die
ursprüngliche Aussage wäre still verlorengegangen.

**Der Fremdmaschinen-Test ist gefahren, wenn auch noch auf derselben
Architektur.** Ein Verzeichnis mit genau den Dateien, die ein frischer
Klon mitbringt — 582 Stück, ohne Ausgabeverzeichnis, ohne Modelle, ohne
Artefakte, ohne Python-Umgebung —, gebaut und gestartet über den Starter:
Der Bau läuft durch, der Protokoll-Durchlauf liefert 10 von 10 Stufen mit
dem damals bekannten Gesamtwert `a9af743fba0e77dc`, `konformitaet` ohne Artefakt
6 von 6 mit `894d8357ae92b5c1`, `artefakte` meldet zu Recht Exit 1,
`vergleich` verweigert bei einer Maschine das Urteil, und das Menü läuft
auch ohne Terminal. **Was damit nicht belegt ist:** derselbe Durchgang
unter Windows und der Weg über Modellbeschaffung und Artefaktbau. Beides
braucht eine fremde Maschine oder einen Download, und beides steht noch
aus.

### v0.15.0 – 2026-08-27 (Fremdmaschinen-Automatik)

**Eine frische Maschine richtet sich selbst ein.** Der erste Start auf
einem Rechner ohne Rust und ohne Python verlangte bisher zwei Sätze von
Handgriffen; beide übernimmt jetzt das Werkzeug selbst, nach Rückfrage
und mit sichtbarem Fortschritt, und nichts davon läuft still ab.

**Die Starter installieren Rust.** Fehlt `cargo`, fragt der Starter, ob
er es holen soll. Unter Windows lädt er `rustup-init.exe` passend zur
Prozessorarchitektur und installiert ins Benutzerprofil, ohne
Administratorrechte; danach fragt er dasselbe für die Microsoft
C++-Werkzeugkette, ohne die der Übersetzer nicht binden kann, und holt
sie über winget. Unter Unix läuft der rustup-Installer über curl. Wer
ablehnt, bekommt die Handgriffe von Hand genannt. Ein Fenster ohne
Eingabekanal installiert nichts: nicht fragen heißt hier nicht
installieren.

**Der Client richtet Python ein.** Die Kalibrierpakete waren bisher der
zweite Satz Handgriffe: `python3 -m venv .venv`, dann `pip install -r
requirements.txt`. Fehlt ein einsatzbereites Python, fragt der Client
jetzt und tut beides selbst; unter Windows ohne Python bietet er zuerst
die winget-Installation an. Die Entscheidung darüber, welcher Weg
gegangen wird, steht als reine Funktion im Code und ist getestet:
einsatzbereit hat Vorrang, dann die Reparatur des Vorhandenen, dann der
Windows-Weg, sonst die Anleitung.

**Ein Fund an der eigenen Kette.** Die Funktion, die fremde Prozesse
startet und ihre Ausgabe durchreicht, las den stderr-Strom nie. Die
Leitung ist rund 64 KB groß; pip und der Download schreiben
Fortschrittszeilen dorthin, und ohne Leser blockierte der Kindprozess,
sobald sie voll war — nach einigen Minuten, also genau dann, wenn
niemand mehr damit rechnet. Ein Nebenfaden leert den Strom jetzt, und
bei einem Fehlschlag nennt die Meldung seine letzten Zeilen.

### v0.14.2 – 2026-08-26 (der Client reicht die Ketten-Ablage durch)

Der Knoten kann seine Kette jetzt als Datei führen und nach einem
Abbruch oder Neustart dort wieder ansetzen. Die Konfiguration dafür
trägt ein neues Feld, und der Client reicht es durch: als `None`, aus
demselben Grund wie bei Genesis-Datei und Konsensschlüssel in der
Fassung davor. Ein Kettenlauf aus dem Menü ist ein Probenetz für
Stunden; was er baut, bleibt im Speicher und endet mit dem Programm.
Die Ablage auf Platte gehört dem Knotenbetrieb auf Servern, der seine
Kette über Tage behalten will.

### v0.14.1 – 2026-08-26 (BFT-Runden über das Netz — der Client stimmt nicht mit)

Die Knoten führen jetzt Abstimmungsrunden über ein eigenes Gossip-Topic,
mit dem Validator-Satz aus einer hashgebundenen Genesis-Datei und einem
Konsensschlüssel, der getrennt von der Netzidentität liegt. Die
Knoten-Konfiguration bekam dafür zwei neue Felder, und der Client reicht
sie durch: beide als `None`.

Das ist eine bewusste Grenze, kein Versehen: Der Testclient fährt keine
BFT-Runden. Eine Genesis-Datei mit Validator-Satz und ein
Konsensschlüssel mit Besitznachweis entstehen nicht nebenbei aus einer
Einladungsadresse, und ein Menü, das danach fragt, ohne sie erklären zu
können, wäre eine Hürde mehr für den Teilnehmerlauf, für den der Client
da ist. Wer die Runden sehen will, betreibt `myl-node` direkt; der
Client bleibt der Einstieg, der Knoten die Vollform.

### v0.14.0 – 2026-08-25 (der Katalog führt vorgemerkte Modelle)

Zwei weitere Basismodelle stehen im Modellkatalog, mit geprüfter Lizenz
und festgelegter Revision — aber noch nicht im Register, weil ihre
Gewichte nicht geholt und ihre Artefakte nicht gebaut sind. Der
Katalog führt dafür den Status „vorgemerkt".

Die Konsistenzprüfung zwischen Katalog und Register verlangte trotzdem
für **jedes** Katalogmodell einen Registereintrag und machte genau
diesen Status unbenutzbar: Ein Register trägt Digests gebauter
Artefakte; ein Modell, das bewusst nicht gebaut ist, **kann** dort
nicht stehen. Die Prüfung nimmt vorgemerkte Modelle jetzt aus, und eine
Gegenprobe hält das Schlupfloch zu: Ein vorgemerktes Modell, das im
Register stünde, wäre gebaut, und sein Status wäre falsch. Ohne die
Gegenprobe könnte sich ein gebautes Modell mit dem Status der
Digest-Prüfung entziehen.

Für den Client ändert sich nichts Sichtbares: Vorgemerkte Modelle sind
nicht wählbar, denn ohne Registereintrag ist ihr Digest nicht prüfbar.

### v0.13.0 – 2026-08-23 (Windows-Bereitschaft: ein Fund und mehr CI)

Anlass war die Frage, ob der Partnerlauf auf einer Windows-Maschine
starten kann. Beim Durchsehen der Kette fiel eine Stelle auf, die nur
dort bricht.

**Fund: Der Zielpfad stand als Quelltext im Python-Aufruf.**
`beschaffen` baute das Downloadskript mit `local_dir=r'{ziel}'`. Auf
einem Rechner, dessen Benutzername ein Apostroph enthält, etwa
`C:\Users\O'Brien\…`, war das erzeugte Python **syntaktisch falsch**,
und der Teilnehmer bekam einen `SyntaxError` statt eines Downloads.
Nachgestellt und vor der Behebung reproduziert.

Pfad, Repo-Kennung und Revision gehen jetzt über `sys.argv`. Damit gibt
es diese Fehlerklasse nicht mehr: Das Betriebssystem reicht die
Zeichenkette durch, ohne dass sie je Quelltext wird. Gegenprobe mit
demselben Pfad gelaufen.

**Warum das hier steht und nicht unter „Kleinigkeit":** Die Stelle liegt
in genau dem Teil der Kette, den die CI **nicht** abdeckt, nämlich
Artefaktbeschaffung, Bau und Modellauf. Dass der erste Fund beim
Hinsehen ausgerechnet dort lag, ist die Antwort auf die Frage, wie
belastbar dieser Teil ist.

**Die Windows-CI deckt jetzt mehr ab.** Zusätzlich zu Clippy,
Unit-Tests und dem `stack`-Lauf: `--help`, `plan` (schreibt eine Datei,
prüft also Pfade und Prüfsumme) und `modellstaende` (liest ein
Verzeichnis). Dazu eine **Gegenprobe**: `vergleich` muss ein leeres
Verzeichnis ablehnen, denn ein Werkzeug, das aus nichts einen Nachweis
macht, wäre schlimmer als keines.

**Nicht in der CI: `artefakte`.** Ohne gebaute Artefakte meldet der
Befehl zu Recht Exit 1, das ist seine Aufgabe. Ein `|| true` davor würde
die Aussage wegwerfen, für die es ihn gibt.

### v0.12.0 – 2026-08-23 (die Lizenzdatei kam nie an)

Bei der Lizenzprüfung des Basismodells fiel auf, dass die Beschaffung mit
`allow_patterns=['*.json','*.safetensors','*.txt']` lud. **Eine
Lizenzdatei trägt keine Endung** und kam deshalb nie an.

`ETHICS/Manifest.md` berief sich für Grundsatz G7 („das Basismodell muss
frei nachnutzbar sein") ausdrücklich auf
`INTEGER_LLM/models/Qwen2.5-0.5B/LICENSE`. Diese Datei existierte auf
keiner Maschine, die das Modell über diesen Weg geholt hatte. Dieselbe
Klasse wie Fund 27: eine schriftliche Zusage, die niemand nachgesehen
hat.

Es wiegt hier doppelt: Apache 2.0 §4(a) verlangt, jedem Empfänger einer
Bearbeitung eine Kopie der Lizenz mitzugeben, und ein Partner, der die
Gewichte für einen Testlauf holt, erfuhr sonst nie, unter welchen
Bedingungen sie bei ihm liegen.

`LICENSE*` steht jetzt in den Mustern. Beide lokal vorhandenen Modelle
tragen die Datei; sie ist in beiden Fällen bytegleich und der
unveränderte Apache-2.0-Text.

### v0.11.0 – 2026-08-23 (Phase 3 abgeschlossen: 3.2 und 3.3)

Beide Punkte beantworten dieselbe Frage von zwei Seiten. Bei einem
θ_v-Wechsel ändern sich die Vergleichswerte **zwangsläufig**; die Frage
ist dann nicht „gleich oder nicht", sondern **„erwartet oder nicht"**.

**`--erwarte <digest>`** setzt eine Erwartung am Messlauf durch. Wer den
neuen Wert einmal festgestellt hat, schreibt ihn in den CI-Aufruf; ab da
meldet sich jede weitere Änderung von selbst, statt beim nächsten
Partnerlauf aufzufallen.

```bash
myl-test --plan wikitext2-0.5b-standard.plan determinismus --erwarte aca90b797f1cf756
```

Die Kurzform vom Bildschirm genügt, denn genau die tippt jemand ab.
Verglichen wird so weit, wie angegeben ist, und das Protokoll hält fest,
wie weit das war: **64 Bit reichen gegen ein Versehen, nicht gegen
jemanden, der einen passenden Digest sucht.** Für diesen Zweck ist das in
Ordnung, denn die Erwartung steht in derselben Befehlszeile wie der Lauf.

**Kein stiller Durchlauf:** Ein Lauf ohne Vergleichswert erfüllt keine
Erwartung, sondern schlägt fehl. „Nichts gemessen" darf nie wie „stimmt
überein" aussehen.

**`modellstaende`** ist die Auswertung dazu. Sie liest denselben Ordner
wie `vergleich` und stellt die Vergleichswerte über die Modellstände
hinweg gegenüber:

```
     Stände:
       [1] 0.17.0 / 97869982   Digest über logits+token
       [2] 0.17.0 / c42bb8a8   Digest über logits+token
       [3] 0.18.0 / c42bb8a8   Digest über logits+token

     determinismus    teils gleich          51d50d1c…  aca90b79…  aca90b79…

     [2] → [3]  unverändert: determinismus
```

**Interessant ist nicht, was sich geändert hat, sondern was nicht.** Ein
Wert, der einen θ_v-Wechsel unbeschadet übersteht, hängt entweder nicht
am Modell, oder die Änderung hat ihn nicht erreicht.

Der Befehl fällt **kein Determinismusurteil** und endet mit Exit-Code 0,
solange er lesen konnte: Zwei Modellstände sollen verschiedene Zahlen
liefern, das ist kein Befund.

**Fund an der eigenen Arbeit.** Die erste Fassung urteilte je
Vergleichswert über **alle** Stände auf einmal. Bei drei Ständen, von
denen zwei denselben Wert trugen, meldete sie „jeder Vergleichswert hat
sich geändert" und verschwieg genau das Paar, nach dem die Prüfung
fragt. Verglichen wird jetzt je **Paar** von Ständen. Aufgefallen ist es
beim ersten Lauf gegen echte Protokolle, nicht beim Lesen des Codes; die
Nachstellung mit drei Ständen steht als Test.

### v0.10.0 – 2026-08-23 (der Shard-Vergleich misst jetzt Zahlen)

**Der letzte offene Teil von Fund 36.** `shard` hielt den Token-Digest
des Pods gegen den Token-Digest des Einzelknotens und meldete
`bitgleich`. Was er damit prüfte, war, ob die Aufteilung dieselbe
**Entscheidung** erzeugt. Ein Token ist ein Argmax über 151 936 Zahlen;
an 0,5B blieb er unverändert, während 0,1 % der Bytes eines Tensors
verschoben waren.

Der Grund war nicht Nachlässigkeit, sondern die Schnittstelle: Der Pod
gab über `run_prompt` nur Token heraus. `myl-pod` v0.3.0 liefert jetzt
einen `dekodier_digest` nach demselben Vertrag wie der Einzelknotenlauf,
und der Lauf hält beide gegeneinander.

**Neue Protokollwerte:** `prompt_N_pod_logits` und
`prompt_N_einzelknoten_logits`. Der Token-Vergleich bleibt daneben
stehen, aber als das schwächere der beiden Urteile.

**Kein stiller Rückfall.** Liefert der Pod keinen Digest, oder deckt er
weniger Schritte ab als verlangt, bricht der Lauf mit einer
ausformulierten Begründung ab. Ein Lauf, der auf den Token-Vergleich
zurückfällt, wäre genau der Zustand, aus dem Fund 36 kam.

**Gemessen** (0,5B, `reference`): Pod und Einzelknoten liefern
`df54ef6c89f1a840`, und zwar bei 1, 2, 3, 4, 6, 8, 12 und 24 Shards.

**Gegenprobe gelaufen, nicht nur gedacht.** Mit einem einzigen um eins
verschobenen Logit weit unterhalb des Argmax im letzten Shard bleiben
beide Token-Digests identisch bei `f1117a59462f9919`, die Logit-Digests
gehen auseinander, und der Lauf schlägt fehl mit dem Hinweis: *„Die Token
stimmen, die gerechneten Zahlen nicht. Vor dem Abschluss von Fund 36
hätte dieser Lauf `bitgleich` gemeldet."* Danach zurückgenommen.

### v0.9.0 – 2026-08-22 (Menü, Pläne ohne Modell, Modellkatalog)

**Das Entwickler-Menü führt keine Einzelstufen mehr.** Hardware,
Determinismus, geshardete Inferenz und Protokoll-Durchlauf standen dort je
einzeln; sie sind genau die vier Stufen, die der Testlauf im Nutzermenü
hintereinander ausführt. Einzeln gestartet schrieben sie vier getrennte
Protokolle, die der Koordinator wieder zusammensetzen müsste, und beim
Verschicken geht die eine verloren, die den Befund trägt. Auf der
Befehlszeile bleiben sie erreichbar; dort ist klar, dass man eine
Einzelmessung will. Neu sortiert nach Wichtigkeit, mit **Protokolle
vergleichen** an erster Stelle. „Artefakte und Gewichte freigeben" heißt
jetzt „löschen": Das Wort beschreibt, was geschieht.

**Das Nutzermenü folgt jetzt dem Ablauf**, nicht der gewachsenen
Reihenfolge: [1] Artefakt wählen, [2] Testdatei wählen, [3] Testlauf
starten, [4] Mit dem Modell sprechen. Vorher stand das Gespräch mit dem
Modell an erster und das Artefakt an vierter Stelle, also das Ergebnis
vor seiner Voraussetzung: Wer [1] wählte, ohne ein Artefakt zu haben,
bekam als Erstes eine Modellauswahl, die er nicht erwartet hatte.

Das Gespräch mit dem Modell steht hinter dem Lauf, obwohl man es davor
führen mag: Es ist der einzige Punkt, der **nicht misst**, und ein Menü
ordnet nach Aufgabe, nicht nach Neugier.

**Artefakt und Testdatei sind Auswahlzustände, keine Vorgaben.** Beim
Start steht in beiden Zeilen „nicht ausgewählt", samt Verweis auf den
Punkt, der sie setzt. Vorher zeigte der Client auf das Vorgabemodell und
auf die eingebauten Prompts: Das sah aus wie eine Entscheidung und war
eine Annahme. Wer den Testlauf startete, maß dann möglicherweise etwas
anderes als der Vergleichspartner, ohne dass ihm eine Frage gestellt
worden wäre.

**Der Testlauf fragt genau das ab, was fehlt.** Ist beides gewählt und
steht unten in der Übersicht, läuft er sofort los. Zwei Rückfragen auf
jeden Lauf sind bei einem Durchgang lästig und bei zehn ein Grund, den
Client nicht mehr zu benutzen. Fehlt die Testdatei und wird keine
gewählt, läuft der Test trotzdem, sagt aber vorher, dass er die
Vorgabewerte nimmt und mit keiner anderen Maschine vergleichbar ist.

Die Übersicht nennt das **Artefakt** beim Modellnamen statt beim Pfad,
und die **Testdatei** beim Namen statt bei ihrer Prüfsumme: An acht
Hexzeichen erkennt niemand seine Datei wieder. Ohne Testdatei stehen
Prompt, Token und Shards ausdrücklich als Vorgabewerte da; mit ihr nennt
ihre Zeile den Umfang, und die Wiederholung darunter entfällt.

Die vier Schritte sind durch eine **Leerzeile** von den drei
Nebenfunktionen abgesetzt ([5] Anleitung, [9] Entwickler, [0] Beenden).
Sieben gleichrangige Zeilen lesen sich wie sieben Möglichkeiten; vier
plus drei lesen sich wie ein Weg mit Beiwerk. Der Abstand hängt am Punkt
(`Punkt::abgesetzt`) und nicht als eigener, nicht wählbarer Eintrag in
der Liste: Ein solcher Platzhalter müsste in der Pfeilnavigation
übersprungen, in der Ziffernwahl ignoriert und in der Höhenrechnung
mitgezählt werden, also an drei Stellen, an denen sich ein Fehler
versteckt.

**Testpläne sind nicht mehr an ein Modell gebunden.** Das Feld `model`
ist entfallen, samt seiner Rolle in der Prüfsumme. Es war eine Fessel
ohne Nutzen: Ein Plan, der nur mit 0,5B geht, muss für 7B neu geschrieben
werden, und dann tragen zwei Dateien dieselben Prompts unter
verschiedenen Prüfsummen. Der Plan legt jetzt fest, **was** gemessen
wird; **woran**, entscheidet sich unmittelbar vor dem Lauf, entweder über
[4] oder ungefragt, wenn genau ein Artefakt vorliegt.

Abgesichert bleibt es an der Stelle, an der es wirkt: Der Modellstand
steht in jedem Protokoll, und `vergleich` verweigert das Urteil, wenn
zwei Läufe gegen verschiedene Modelle gerechnet haben. Eine Datei kann
man ignorieren, diese Prüfung nicht. Alte Pläne mit `model`-Zeile bleiben
lesbar; ihre Prüfsumme stimmt nicht mehr und sie sind neu zu erzeugen.

**Der Testplan-Assistent fragt jeden Wert einzeln ab**: Token, Shards,
dann Prompt für Prompt mit Nachfrage nach jedem, und den Dateinamen ganz
zum Schluss. Er steht am Ende, weil man einen Plan erst sinnvoll benennen
kann, wenn man weiß, was darin steht. Die Eingabe kommt über eine
übergebene Lesefunktion statt aus `stdin`, damit der ganze Ablauf im Test
durchspielbar ist, samt Abbruch: Ein Abbruch schreibt **nichts**, denn
eine halb erhobene Datei, die an alle Teilnehmer geht, wäre schlimmer als
keine.

**Fünf Testpläne statt zwei**, keiner an ein Modell gebunden. Neben
`standard` und `standard-kurz` drei Benchmark-Pläne, die das Modell
absichtlich an ungewöhnliche Stellen führen: Ziffernfolgen und
Überträge; sieben Sprachen in drei Schriften; Quelltext und lange
Prompts, die die Generierung auf hohe Positionen schieben. Jeder Plan
trägt im Kopf, was er ausübt und wie lange er läuft, ausgerechnet statt
geraten. **Kein Genauigkeitsmaß:** Der Client vergleicht Digests und
bewertet keine Antworten; ein „Benchmark" heißt hier ein Prompt, der
schwer zu rechnen ist. Fund 15 (RoPE) und Fund 16 (Attention nur auf den
ersten Key) fielen bei kurzen Prompts kaum auf.

**Modellkatalog (`INTEGER_LLM/models/KATALOG.json`).** Die Angaben zu den
Modellen standen an drei Stellen: als Tabelle in `models/README.md`, als
Digest in `scale_packs/REGISTER.json` und als `match`-Ausdruck in
`artefakte.rs`. Die dritte hatte den unangenehmsten Fehler, einen stillen
Rückfall `_ => "Qwen2.5-0.5B"`: Ein drittes Modell hätte die Gewichte von
Qwen2.5-0,5B geladen und wäre erst beim Bau aufgefallen, mit einer
Meldung, die nach allem aussieht außer nach der Ursache.

Jetzt gibt es **zwei** Quellen, und beide aus einem Grund: `KATALOG.json`
ist kuratiert und trägt, was jemand entschieden hat (Herkunft, Revision,
Lizenz, Status, Bemerkung); `REGISTER.json` ist erzeugt und trägt, was
gemessen wurde (Digest, θ_v). Der Client liest beide, ein Test verlangt,
dass sie dieselben Modelle führen, und `models/README.md` wird aus beiden
erzeugt (`tools/modelle_liste.py`, mit `--pruefen` für die CI). Die
Modellauswahl zeigt jetzt Parameterzahl, Herkunft, Lizenz und eine
Einordnung, statt nur die Downloadgröße.

**Fund vom Linux-Runner: Groß- und Kleinschreibung.** Der
Modellschlüssel (`qwen2.5-0.5b`) und der Verzeichnisname der Gewichte
(`Qwen2.5-0.5B`) unterscheiden sich **nur darin**. Solange die Zuordnung
im Code stand, fiel das nicht auf; mit dem Katalog bekam sie einen
Rückfall auf den Modellnamen, und der traf auf macOS und Windows
trotzdem das richtige Verzeichnis, weil deren Dateisysteme die
Schreibweise nicht unterscheiden. Auf dem Linux-Runner der CI nicht: Dort
fand `belegung` die Gewichte nicht mehr, und ein Nutzer hätte in
„Artefakte und Gewichte löschen" geglaubt, sie seien weg.

Getrennt in zwei Funktionen mit verschiedenen Aufgaben: `hf_id` ist eine
**Auskunft** und rät nicht; `gewichte_verzeichnis` ist eine **Suche** und
vergleicht in drei Stufen, Katalog, dann ohne Rücksicht auf die
Schreibweise über die vorhandenen Verzeichnisse, dann der Modellname
unverändert. Der Regressionstest prüft die Zeichenkette statt des
Zugriffs und greift deshalb auf jedem Dateisystem.

**Und der Download nimmt jetzt die festgelegte Revision.** Er stand auf
`repo_id='Qwen/{hf}'` ohne Revision, holte also, was gerade auf `main`
liegt, und nahm nebenbei an, jedes Modell dieses Projekts komme von Qwen.
`models/README.md` verlangt seit jeher eine fixierte Revision, ohne die
der Lauf nicht reproduzierbar ist. Ein Modell, das sich zwischen zwei
Teilnehmern ändert, erzeugt genau den Befund, gegen den dieses Werkzeug
gebaut ist. Fehlt der Katalogeintrag, bricht der Download ab und nennt
den Grund, statt zu raten.

**Nebenbefund:** Der handgeschriebene JSON-Leser reichte Werte
unverändert durch. Sobald der Katalog Text für Menschen aufnahm,
erschien ein `\n` wörtlich im Menü, und ein Anführungszeichen hätte die
Zeile zerlegt. Er löst jetzt `\n`, `\t`, `\"` und `\\` auf und lässt
alles andere stehen, statt es zu erraten.

### v0.8.0 – 2026-08-22 (drei Funde am Messgerät)

Diese Fassung baut fast nichts Neues. Sie behebt drei Stellen, an denen
das Werkzeug einen Nachweis geliefert hätte, den es nicht gab. Anlass war
die Vorbereitung des ersten Laufs auf einer fremden Maschine: Der Lauf
findet einmal statt, und was er misst, entscheidet sich vorher.

*Zwischen v0.6.0 und dieser Fassung stand die Crate kurzzeitig auf
**v0.7.0** (Behebung von Fund 34 in `hardware.rs`, Commit `bc36296`). Ein
eigener Abschnitt dafür fehlt hier, weil die Fassung nie für sich stand;
ihr Inhalt ist unten unter Fund 34 beschrieben. Vermerkt, damit die Lücke
in der Nummernfolge keine Frage aufwirft.*

**Fund 34: `cpu-simd` galt auf x86_64 als eigener Rechenpfad.**
`kernels/src/dot.rs` vektorisiert nur unter aarch64; `rechenpfad.rs`
führte `cpu-simd` dagegen unter `any(x86_64, aarch64)`, und
`hardware::selected_backend` schrieb sogar `cpu-simd/avx2` ins Protokoll,
sobald `is_x86_feature_detected!` AVX2 auf der **CPU** fand. Das ist eine
Auskunft über den Prozessor, nicht über unseren Code. Gemessen an
derselben Quelle für zwei Ziele (20 000 Durchläufe über 4096 Elemente):

| Ziel | `dot_scalar` | `dot_i8_i16` | Verhältnis |
|---|---|---|---|
| aarch64 | 6,97 ms | 2,70 ms | **2,58×** |
| x86_64 | 15,26 ms | 15,20 ms | **1,00×** |

Es ist Fund 33 eine Ebene tiefer: Ein Protokoll von der Partnermaschine
hätte `x86_64-…-cpu-simd/avx2` getragen, während beide Seiten denselben
skalaren Code rechneten, und die Anleitung führte „Referenz + AVX2" als
lohnende Kombination. Die Bedingung steht jetzt **einmal**, am `cfg` von
`dot::gewaehlt`; alle Auskünfte lesen `dot::VEKTORISIERT`. Ein Bau mit
`--features cpu-simd` auf einem Ziel ohne vektorisierten Pfad wird
abgelehnt, mit dem Hinweis, dass `cargo build --release` genügt.

**Fund 35: Ein abgebrochener Lauf trug den Nachweis.** Verglichen wird je
Wert nur unter den Protokollen, die ihn haben. Ein Lauf, der nach dem
ersten von sechs Prompts endete, stimmte damit in allem überein, was er
erreicht hatte, und fehlte im Rest, ohne dass es auffiel: Urteil
`NACHWEIS`. Neu ist das Urteil `UNVOLLSTÄNDIG`, geprüft wird der
Abschlusseintrag und die Gleichheit der Wertemengen. `RunLog` hinterlässt
zusätzlich einen Abbruchvermerk, wenn `finish` nie lief; für Strg-C und
`kill -9` hilft nur die Leseseite, und genau dort sitzt die Prüfung.

**Fund 36: Der Vergleichswert maß Token-Gleichheit, nicht
Bitgleichheit.** `greedy_digest` hashte nur die erzeugten Token. Ein
Token ist ein Argmax über 151 936 Zahlen und ändert sich erst, wenn die
Rangfolge kippt. Gemessen an Qwen2.5-0,5B, Bytes eines Tensors verschoben
und die Hashkette bis `theta_v.json` konsistent nachgezogen:

| geänderte Bytes | Anteil | Digest über Token | Digest über Logits |
|---|---|---|---|
| 9 | 0,0011 % | **unverändert** | verändert |
| 81 | 0,0101 % | **unverändert** | verändert |
| 803 | 0,1 % | **unverändert** | verändert |
| 8029 | 1,0 % | verändert | verändert |

In drei von vier Stufen rechnete das Modell nachweislich andere Zahlen,
und der Vergleichswert meldete „bitgleich". Der Digest deckt jetzt die
**Logits** jedes Schritts ab. Sie sind `i32`, das Hashen bleibt also
exakt und in der Ganzzahldisziplin. Alle Protokolle tragen den Umfang als
Feld `digest_umfang`; zwei verschiedene Umfänge gelten wie zwei
verschiedene Modellstände als unvergleichbar.

**Reichweite von Fund 36, geprüft statt vermutet.** Die erste Frage war,
ob das **Protokoll** denselben Fehler hat, ob also das Netz Miner nach
Token statt nach Zahlen vergleicht. Nein: `myl-verifier::adjudicate`
hasht die **Ausgabe-Aktivierungen**. Von den 30 Konformitätsvektoren
vergleichen 27 Tensoren, nur die drei `e2e`-Vektoren vergleichen Token.
Betroffen sind damit Messwerkzeuge, nicht das Protokoll. Offen bleiben
die drei `e2e`-Vektoren, `hash_tokens` in `bench/run.py`, und der
**Shard-Vergleich**: Er hält Token gegen Token, weil die Stage-API des
Pods nur Token herausgibt, und prüft damit, ob die Aufteilung dieselbe
Entscheidung erzeugt, nicht dieselben Zahlen. Ihn zu schärfen ist ein
Eingriff in COMPUTE_PIPELINE und ein eigener Punkt.

**Beim Beheben selbst zugefügt und beim Lauf gegen die echten Artefakte
gefunden:** Die Umstellung brach zunächst den Shard-Vergleich, weil dort
der Token-Digest des Pods gegen den neuen Digest des Einzelknotens
gehalten wurde. Kein Test hat das gefangen, denn `run_shard` braucht ein
Modell. Beide Werte heißen jetzt `…_tokens` und meinen dasselbe.

**Außerdem:**

- **Punkt 2.4, `--repeat N`:** Läufe je Prompt im Determinismuslauf,
  Vorgabe und Minimum 2. Verglichen wird gegen den **ersten** Lauf, nicht
  paarweise gegen den Vorgänger; bei einer Abweichung nennt das Protokoll
  die Nummer des ersten abweichenden Laufs. Alle Beteiligten müssen
  denselben Wert verwenden, sonst urteilt `vergleich` zu Recht
  `UNVOLLSTÄNDIG`.
- **Negativtest Artefaktwechsel** als Test: Jede einzelne Ankerdatei muss
  durchschlagen, der Digest muss reproduzierbar sein, und eine fehlende
  Ankerdatei ist ein Fehler statt eines anderen Digests. Am echten Modell
  nachgemessen: Ein verändertes Gewichtsbyte ohne nachgezogene Hashkette
  wird beim Laden abgelehnt, mit Nennung des Tensors.
- **`SCHNELLSTART.md`**: die Seite zum Verschicken an einen Partner
  (hieß bis zum 2026-08-24 `EINSEITER.md`, umbenannt, weil sie inzwischen
  **zwei** Tests beschreibt und die Zahl im Namen nicht mehr stimmte).
  Stichpunkte statt Fließtext, alles über das Menü, keine Befehlszeile:
  Test 1 ist die Rechnung (Determinismus), Test 2 das Netz. Die
  ausführliche Fassung ist `ANLEITUNG.md`, Teil A und Teil C.

### v0.6.0 – 2026-08-21 (vom Protokoll zum Urteil)

- **Alles unter dem Schriftzug steht mittig**, und zwar **als Block**:
  Menü, Einstellungen, Begrüßung, Namensabfrage und Kurzanleitung bekommen
  je denselben Einzug, ihre Zeilen bleiben untereinander ausgerichtet.
  Zeilenweise zentriert verrutschten die Menüpunkte gegeneinander, und die
  Liste wäre keine mehr. Die Blockbreite richtet sich nach der breitesten
  Zeile über Kopf, Punkte, Hinweise und Fuß hinweg; bliebe eines davon aus
  der Rechnung, stünde der Block schief, sobald gerade dieses das breiteste
  wäre. Die **interaktiven Zeilen** (Menüpunkte, Eingabeaufforderungen)
  sitzen am linken Rand ihres Blocks: Zentriert stünde der Cursor je nach
  getipptem Text an einer anderen Stelle. Ohne Terminal wird nicht
  eingerückt, ein mitgeschnittener Lauf soll diffbar bleiben.
- **Die Kurzanleitung in Punkt [5]** ist nach Rollen geordnet, nicht nach
  Menüpunkten: Wer den Client startet, ist entweder Teilnehmer oder
  Koordinator, und die beiden brauchen verschiedene Hälften. Sie räumt den
  Bildschirm auf, bevor sie sich zeigt, und passt danach genau ins
  Fenster. **Fund dabei:** Der erste Test rechnete nur Banner plus
  Anleitung und übersah, dass beim Aufruf schon 42 Zeilen Menü und
  Einstellungen dastehen; gemessen waren es 59 Zeilen in einem Fenster mit
  44, das Logo also weggescrollt. Der Test rechnet jetzt beides, und drei
  weitere prüfen, dass die genannten Menüpunkte zu den tatsächlichen
  passen: Eine Anleitung, die auf den falschen Punkt zeigt, ist schlechter
  als keine.
- **Die aktuellen Einstellungen stehen unter dem Menü**, nicht darüber:
  Zuerst die Frage, was man tun will, dann der Zustand, unter dem es
  geschieht. Technisch als **Fuß** der Auswahl (`waehlen_mit_fuss`) und
  nicht als eigener Druck davor, denn die Liste zeichnet sich bei jedem
  Tastendruck neu, indem sie um ihre eigene Höhe nach oben springt und von
  dort abwärts löscht. Alles, was unter ihr stünde, verschwände beim
  ersten Pfeildruck; der Fuß muss deshalb in ihre Höhenrechnung eingehen.
- **Begrüßung nach der Namenseingabe**, Zeichen für Zeichen geschrieben,
  der Name in der Farbe des eben entstandenen Schriftzugs. Der Nutzername
  ist die einzige Eingabe vor dem Menü; ohne Antwort darauf wirkt sie wie
  ein Formularfeld. Ohne Terminal, bei `MYL_NO_ANIMATION` und auf
  Tastendruck erscheint der Text sofort und vollständig, nicht gar nicht:
  Er trägt eine Aussage, keine Verzierung.
- **Nach dem Nutzernamen kommt sofort das Menü.** Bis v0.6.0 lief davor
  erst die Planauswahl und danach die Artefaktbeschaffung. Wer den Client
  zum ersten Mal öffnete, musste also zwei Entscheidungen treffen, die er
  noch nicht einordnen konnte, und eine davon zog bis zu 15 GB Download
  nach sich. Der Testplan gehört an die Stelle, an der er gebraucht wird:
  Punkt [2] fragt ihn ab und misst dann.
- **Nutzermenü in der Reihenfolge des Ablaufs:** [1] mit dem Modell
  sprechen, [2] Testlauf, [3] Testdatei, [4] Artefakt, [5] Anleitung.
  „Protokolle vergleichen" ist ins Entwicklermenü gewandert: Es ist die
  Arbeit des Koordinators, und für einen Teilnehmer, der eine Maschine
  beisteuert, ein Punkt, der ihm nichts nützt.
- **[1] Mit dem Modell sprechen:** der einzige Punkt, der nicht misst.
  Freie Eingabe, höchstens 64 Token je Antwort (0,5B rund 3 s, 7B rund
  32 s bei den dokumentierten Raten), Modell einmal geladen statt je
  Frage. Kein Protokoll: Prompt und Länge bestimmt der Nutzer frei, das
  wäre kein Messwert. Gerechnet wird derselbe gierige Pfad wie im
  Determinismuslauf, also ohne Sampling und ohne Zufall.

  **Die Antwort erscheint Token für Token**, nicht am Stück. Bei 7B dauert
  sie über eine halbe Minute; ohne laufende Ausgabe wäre in dieser Zeit
  nicht zu unterscheiden, ob gerechnet wird oder etwas hängt. Ausgegeben
  wird dabei die **Differenz des neu dekodierten Stroms**, nicht das
  einzelne Token: Ein Token ist bei BPE oft kein vollständiges Zeichen,
  einzeln dekodiert entstünden Bruchstücke und kaputte Umlaute. Gemessen:
  67 Ausgabestücke über 3,7 Sekunden auf 0,5B.

  **Zurück ins Menü führen Escape, Strg-D und getippte Wörter** (`menu`,
  `exit`, `q`, `zurück`); der Hinweis steht in jeder Eingabezeile, denn
  wer ein paar Fragen gestellt hat, hat den Kopf längst weggescrollt. Die
  **leere Eingabe ist bewusst keiner mehr**: Enter tippt man auch, um zu
  sehen, ob sich etwas aufgehängt hat, und wer sich vergewissern wollte,
  stand danach im Menü. Dafür braucht es eine Eingabezeile im Rohmodus
  (`auswahl::zeile_lesen`), denn `read_line` sieht Escape nicht als Taste,
  sondern als Zeichen in der Zeile. Strg-C ist weiterhin **kein** Rückweg:
  Es beendet den ganzen Client.
- **[4] Artefakt wählen führt eine Liste über alle bekannten Modelle**,
  vorhandene wie fehlende, mit dem Zustand daneben. **Fund:** Bis dahin
  gab es zwei getrennte Wege. Lag ein Artefakt vor, wurde daraus gewählt;
  lag keines vor, wurde aus dem Register gewählt und beschafft. Wer 0,5B
  hatte und 7B wollte, fand deshalb **keinen Weg dorthin**: Die
  Beschaffung stand nur hinter dem Fall „nichts vorhanden", und der trat
  nie wieder ein. Besonders bitter nach dem Freigeben von Plattenplatz,
  denn genau dann will jemand ein Modell zurückholen, das er eben gelöscht
  hat. Der Zustand ist jetzt eine Eigenschaft des Eintrags, kein eigener
  Programmzweig. Artefakte ohne Registereintrag stehen am Ende und sind
  als ungeprüft gekennzeichnet.
- **[4] Artefakt wählen** als eigener Schritt. Bis v0.6.0 löste die
  Testdatei das Modell gleich mit auf, und aus einer Menüwahl wurden
  ungefragt bis zu 15 GB Download. Übernommen wird jetzt nur noch
  stillschweigend, was ohnehin dalliegt (`artefakte::vorhandenes`).
- **Startbild in drei Stufen:** Regen, Einströmen, Gleiten. **Der Regen
  hört dabei nicht auf**, er läuft im Hintergrund weiter, während sich der
  Schriftzug bildet. Sonst entstünde das Logo auf schwarzer Fläche, und
  aus dem Wasserfall wäre ein Vorspann geworden, der abgeschlossen ist,
  bevor das Eigentliche beginnt. Beide Vorgänge teilen sich deshalb einen
  Takt: Je Bild ruft der Aufbau einmal `Wasserfall::schritt` auf, und wer
  zuletzt zeichnet, gewinnt (Reihenfolge: Regen, Arme, fliegende
  Artefakte, angekommene Zeichen).

  Das Einströmen ist **ein** Vorgang, nicht zwei. Vorher lief erst eine
  Spirale ein und danach baute sich der Schriftzug aus Rauschen auf; die
  Spirale hatte mit dem Schriftzug nichts zu tun und hätte fehlen können.
  Jetzt ist sie der **Weg**, auf dem die Zeichen ankommen: Jedes Artefakt
  gehört von Anfang an zu genau einer Stelle des Schriftzugs, trägt eine
  eigene Neonfarbe und läuft auf einem der drei Arme nach innen. Am Ziel
  glüht es zehn Bilder in seiner Farbe nach und nimmt dann die des
  Schriftzugs an. Der Schriftzug ist im Entstehen also bunt und am Ende
  einer.

  **Zwei gegenläufige Bewegungen:** Die Arme wachsen mit jedem Bild weiter
  hinaus, während auf ihnen Zeichen zur Mitte wandern. Später aufbrechende
  Artefakte starten weiter draußen, das Bild öffnet sich also, statt zu
  schrumpfen. Der Winkel hängt am Radius (`winkel = i * WINDUNG`), und
  genau das macht die Kurve sichtbar: Mit zufälligen Winkeln entstünde ein
  Strudel, aber keine Spirale.

  **Kurz vor dem Ziel verlässt ein Artefakt die Bahn.** Die Arme laufen in
  einen Punkt, der Schriftzug belegt eine Fläche; ohne diesen Übergang
  stauten sich alle Zeichen in der Mitte und sprängen dann an ihren Platz.
  Das Gewicht wächst quadratisch, das Artefakt bleibt also lange auf der
  Spirale und schwenkt spät ein.

  Der Schriftzug entsteht **im Mittelpunkt der Spirale** und gleitet erst
  danach an seinen Platz oben. Entstünde er gleich oben, hätte die Spirale
  ins Leere gearbeitet.

  Gerechnet mit einer 64-Einträge-Sinustabelle in Ganzzahlen, damit der
  Gleitkomma-Audit des Projekts keinen Treffer meldet, den erst jemand als
  harmlos einordnen muss.

  Gemessen am Bildstrom (120 x 44): Die Arme wachsen von 5,0 auf 22,4
  Zellen Radius, im Aufbau erscheinen 21 verschiedene Farbtöne, im
  Hintergrund laufen dabei 5267 Tropfenköpfe über die volle Fensterhöhe,
  und der fertige Schriftzug gleitet von Zeile 21,8 auf 11,0.
- **Backends sind wählbar, aber nur ehrlich.** Der Client baut jetzt
  auch mit `--features cuda` und `--features rocm`. Er **verweigert**
  darin jeden Messlauf, und zwar bevor er ein Artefakt sucht:

  ```
  FEHLER  Backend "cuda" hat auf dieser Übersetzung KEINEN eigenen Rechenpfad.
  FEHLER  Ein Prüflauf darüber würde die Referenzimplementierung unter fremdem
  FEHLER  Namen zertifizieren.
  ```

  **Der Befund dahinter (2026-08-22):** `conformance/run.sh cuda` meldete
  auf einem Mac ohne NVIDIA-Hardware 30/30 bestanden. Die Feature-Flags
  in `kernels/Cargo.toml` sind alle leer und schalten nur, ob
  `backends/cuda.rs` übersetzt wird; der Rechenpfad kennt keine
  cuda-Weiche, und `golden_runner` verwarf den Backend-Namen. Für zwei
  geplante Testmaschinen mit NVIDIA und AMD hätte das bedeutet: beide
  Läufe bitgleich, garantiert, weil beide dieselben CPU-Referenzkernel
  rechnen. Das Ergebnis hätte wie der erbrachte Cross-Backend-Nachweis
  ausgesehen.

  Maßgeblich ist `kernels/src/rechenpfad.rs`. Der Client führt keine
  eigene Liste, sondern fragt dort nach: eine zweite Wahrheit veraltete
  beim ersten echten CUDA-Kernel still.

  Das Protokoll trennt deshalb drei Angaben, die vorher zwei waren:

  | Feld | Bedeutung |
  |---|---|
  | `backends_compiled` | wofür dieser Bau **konfiguriert** ist |
  | `backends_rechnend` | was davon einen **eigenen Rechenpfad** hat |
  | `backend_selected` | was in diesem Lauf **tatsächlich** gerechnet hat |

  Ein Bau mit `--features cuda` führt `cuda` in der ersten Zeile und
  `reference` in den beiden anderen. Genau dieses Auseinanderfallen ist
  die Information.

  **Kein `--backend`-Schalter.** Das Backend wird beim Übersetzen
  gewählt, denn der Rechenpfad kennt keine Verzweigung zur Laufzeit; die
  Weiche steht als `cfg` in `kernels/src/dot.rs`. Ein Laufzeitschalter
  wäre eine Behauptung ohne Deckung.

  Gemessen über drei Bauten derselben Quelle: ohne Feature und mit
  `cpu-simd` läuft der Determinismuslauf durch und liefert **denselben**
  Digest (`272f1ee8f45f2c78`), mit `rocm` wird er abgelehnt (Exit 1).
  Seit Fund 36 deckt dieser Digest die **Logits** jedes Schritts ab und
  nicht mehr nur die erzeugten Token: Die Aussage lautet damit, dass NEON
  und die skalare Fassung dieselben Zahlen rechnen, nicht nur dieselbe
  Entscheidung treffen. Der frühere Wert `e19372337dab1f3d` stammt aus
  dem alten Umfang und ist mit dem neuen nicht vergleichbar; die
  Protokolle tragen den Umfang deshalb als eigenes Feld.
  **`cpu-simd` gilt nur auf aarch64** (Fund 34): Auf x86_64 gibt es
  keinen vektorisierten Pfad, und der Client lehnt einen solchen Bau ab,
  statt die Referenz unter fremdem Namen zu protokollieren.
- **Windows-Funde aus dem ersten CI-Lauf:** `menu::kurz` mischte die
  Trennzeichen (`…/d\e\f`), weil ein festes `…/` vor einem Pfad stand,
  den `PathBuf::collect` mit dem Trennzeichen des Systems zusammensetzt.
  Jetzt `MAIN_SEPARATOR` an beiden Stellen; ein Test prüft die Regel
  („nur ein Trennzeichen je Zeile") statt eines festen Ergebnisses.
  Außerdem sind `.plan`-Dateien in `.gitattributes` auf LF festgelegt, mit
  einem Test, dass CRLF weder Werte noch Prüfsumme verändert.
- **Die Repository-Wurzel wird zur Laufzeit gesucht, nicht beim
  Übersetzen.** **Fund:** Der Protokollordner und die Artefaktwurzel
  hingen an `env!("CARGO_MANIFEST_DIR")`, also am Pfad, der beim Bauen
  galt. Das trägt nur, solange das Repository dort liegt, wo es gebaut
  wurde. Wer es verschiebt oder umbenennt, und erst recht, wer ein
  gebautes Binary weitergibt (der Shell-Starter sieht das ausdrücklich
  vor: „cargo nicht gefunden, benutze das vorhandene Binary"), bekam
  „Artefaktverzeichnis fehlt", während die Artefakte danebenlagen.
  Gesucht wird jetzt aufwärts vom Arbeitsverzeichnis, dann vom Ort des
  Programms, und erst zuletzt gilt der Übersetzungspfad.
- **Alles-Löschen** im Entwicklermenü: Artefakte und Gewichte aller
  Modelle in einem Schritt, mit **zwei** getippten Bestätigungen und einer
  Auflistung jedes betroffenen Pfades dazwischen. Die zweite Frage ist die
  eigentliche: Erst nach der Liste weiß man, was verschwindet.
- **Nach jedem Tastendruck ein sauberer Bildschirm.** Das Aufräumen sitzt
  in `menu::weiter`, nicht am Anfang der Menüschleife, damit jeder Pfad
  gedeckt ist. Geleert wird mitsamt Rückblätterspeicher (`Clear::Purge`):
  Wer nach oben scrollt, findet nichts mehr.
- **Das Logo füllt die Fensterbreite.** Das Netzmotiv wird für die
  jeweilige Breite erzeugt (`banner::fuer_breite`), der 56 Zeichen breite
  Schriftzug bleibt unverzerrt und steht mittig. Unter der Mindestbreite
  kommt der feste Text zurück. Auch die **Höhe** zählt: Passt das Motiv
  nicht mitsamt Menü ins Fenster, fällt erst der untere, dann der obere
  Netzblock weg. Ein Logo, das man wegscrollen muss, um das Menü zu sehen,
  ist schlechter als ein kleineres Logo. Damit fiel die frühere Begründung „ein
  Generator wäre Aufwand für ein Bild, das sich nie ändert": Es ändert
  sich jetzt, bei jedem anderen Terminal.
- **Fenstergröße und Lage** setzen die Starter, nicht der Client: Ein
  Programm, das in einem Terminal läuft, kann sein Fenster nicht
  zuverlässig bewegen. Das macOS-Bündel öffnet 120 x 40 mittig auf dem
  Bildschirm, der Windows-Starter setzt beim Doppelklick dieselbe Größe.
  **Ohne zusätzliche Berechtigung:** Angesprochen wird ausschließlich
  Terminal.app, und die Fensterkosmetik steht in einem eigenen Aufruf
  nach dem Öffnen, damit ein Fehlschlag den Start nicht verhindert.
- **Ein Farbschema je Sitzung.** Gewürfelt wird **einmal beim Start**,
  während sich das Logo aus der Spirale bildet: eine Logofarbe aus einer
  Palette von achtzehn 256-Farben-Werten und dazu zwei Schlagwortfarben.
  Danach steht das Schema bis zum nächsten Start.

  Vorher wechselte die Farbe mit jedem Bildschirm. Das war unruhig und
  machte aus einer Eigenschaft der Sitzung eine des Augenblicks: Zwei
  Bildschirme desselben Vorgangs sahen aus, als gehörten sie nicht
  zusammen.

  **Die beiden Schlagwortfarben sind nicht die nächstliegenden.** Eine
  frühere Fassung nahm die beiden Nachbarn im Farbton, und die lagen zu
  nahe: Ein Menütitel in fast der Farbe des Schriftzugs hebt sich nicht
  ab, und zwei benachbarte Töne unterscheiden sich untereinander erst
  recht nicht. Gesucht ist deshalb ein Paar in einem **Band** um die
  Logofarbe:

  | Bedingung | |
  |---|---|
  | mindestens 25° von der Logofarbe | sonst verschwindet der Titel im Schriftzug darüber |
  | höchstens 110° von der Logofarbe | darüber liegt sie gegenüber, nicht daneben |
  | mindestens 40° untereinander | sonst ist der Wechsel nicht zu bemerken |

  Unter allen zulässigen Paaren gewinnt das mit dem kleinsten
  Gesamtabstand zur Logofarbe: so nah am Logo, wie die Bedingungen
  zulassen. Gemessen: Lavendel bringt Himmelblau und Purpur, Cyan bringt
  Grün und Lavendel, Rosa bringt Lavendel und Dunkelorange.

  Gerechnet wird über den **Farbkreis** aus dem 6×6×6-Würfel der Palette,
  nicht über die Reihenfolge im Feld. Die steht zwar ungefähr nach
  Spektrum, aber zwischen Orange und Magenta fehlt das Rot, und ein
  Nachbar im Feld wäre dort ein Sprung im Bild.

  **Farbe trägt dabei nie eine Aussage.** Urteile, Fehler und
  Vergleichswerte stehen als Wort da. Wer Graustufen sieht oder einen
  Mitschnitt liest, verliert nichts. Ohne Terminal wird keine einzige
  Steuersequenz ausgegeben.
- **Aufgeräumter Bildschirm:** Vor jeder Auswahl wird geleert, oben steht
  das Logo, darunter nur das, was ansteht. Nach einer Aktion wartet der
  Client auf einen Tastendruck, ohne ihn verschwände die Ausgabe eines
  Laufs in dem Augenblick, in dem sie fertig ist.
- **Protokolle in `TESTCLIENT/logs/`** statt `TESTCLIENT/myl-testclient/logs/`.
  Zwei Ebenen tief in einem Quellcodeverzeichnis fand sie niemand, der
  sie verschicken sollte.
- **`TESTCLIENT/Vergleiche/`** als Ablage der zugesandten Protokolle,
  `Vergleiche/Berichte/` für den Bericht. Ohne `--logs` liest `vergleich`
  den ersten Ordner und schreibt in den zweiten; Menüpunkt [3] lässt
  zwischen zugesandten und eigenen Protokollen wählen.

- **`vergleich`** (neuer Befehl, Punkt 2.1): liest alle `.jsonl`
  eines Ordners, gruppiert nach Prüflauf und Einstellungs-Kennung, stellt
  die Vergleichswerte gegenüber und fällt ein Urteil. Damit endet der
  Client nicht mehr bei der Messung; das Auswerten war bis hierher
  Handarbeit mit `grep`, und die Anleitung führte vier Kommandozeilen
  dafür auf.
- **Der Nachweis wird verweigert, wenn alle Protokolle denselben
  Hardware-Fingerabdruck tragen.** Akzeptanzkriterium, mit
  Test festgehalten.
- **Modellstand im Protokoll** (Punkt 3.1): θ_v, Gewichts-,
  Skalen- und LUT-Hash, Modellname und Ankerdigest. Vorher standen dort
  nur die Modelldimensionen, und die unterscheiden zwei θ_v-Stände
  desselben Modells nicht. `vergleich` prüft sie **vor** jedem
  Digest-Vergleich: Bei verschiedenen Modellen müssen die Werte
  verschieden sein, und das als Determinismusfehler zu melden wäre genau
  die Verwechslung, gegen die es `artefakte` gibt.
- **Ein Protokoll je Testlauf statt vier.** Hardware, Determinismus,
  Shard-Lauf und Protokoll-Durchlauf sind eine Messung.
- **Teilnehmername**, beim Start gefragt, im Protokoll und im Dateinamen:
  `<name>_<einstellungs-id>_<datum>_<uhrzeit>`. Für Skripte `--name`.
- **Testpläne tragen mehrere Prompts.** Wiederholte `prompt`-Zeilen statt
  `prompt.1`, `prompt.2`: Ein Plan mit einem einzigen Prompt bleibt damit
  unverändert gültig, und beim Erweitern hängt man eine Zeile an. Die
  Reihenfolge geht in die Prüfsumme ein. Zweiter Beispielplan für 7B.
- **Pfeiltasten und Enter** statt Ziffern (`crossterm`). Ziffern bleiben
  als zweiter Weg; ohne Terminal oder in einem zu kleinen Fenster fällt
  die Auswahl auf zeilenweise Eingabe zurück.
- **Startanimation:** Zeichenregen, der sich zu einem Sturm verdichtet,
  aus dem der Schriftzug Zelle für Zelle einrastet. Kein Löschen
  dazwischen: sonst wären es zwei Bilder nacheinander statt eines
  Vorgangs. Ein Tastendruck bricht ab, `MYL_NO_ANIMATION=1` schaltet sie
  ab, ohne Terminal läuft sie gar nicht.
- **Netzmotiv nach dem Vorbild des Projektbanners** überarbeitet: Knoten
  verschiedener Größe (`◉ ● ○ ∘ ·`), Naben mit acht abgehenden Kanten,
  lange Kanten quer durchs Feld. Die alte Fassung war ein regelmäßiger
  Zickzack und las sich als Ornament, nicht als Netz.
- **Fund am Namen:** Die Säuberung für den Dateinamen lief auch über den
  Namen im Protokoll: aus „Björn" wurde „bj-rn", auch im Bericht des
  Koordinators. Jetzt trägt das Protokoll den eingegebenen Namen, und nur
  der Dateiname wird umgeschrieben; Umlaute werden dabei umschrieben
  (`Bjoern`), nicht getilgt.
- **Artefakte und Gewichte löschen** (Entwicklerpunkt [9]). Getrennt,
  weil Artefakte in Sekunden aus dem Skalenpaket entstehen und die
  Gewichte einen Download über Gigabyte kosten. Der Löschpfad ist auf
  direkte Unterverzeichnisse von `INTEGER_LLM/{artifacts,models}`
  eingegrenzt und verlangt ein getipptes „ja". Enter allein genügt an
  der einen Stelle absichtlich nicht, die etwas zerstört.
- **Fund beim Bauen:** Zwei Läufe in derselben Sekunde bekamen denselben
  Dateinamen, und der zweite überschrieb den ersten **stillschweigend**.
  Im Menü tritt der Fall regelmäßig auf. Der Name weicht jetzt auf einen
  Zähler aus.
- **Zweiter Fund:** Die Menüschleife hielt `stdin.lock()`, während die
  neue Auswahl im Rückfallweg `io::stdin().read_line()` aufruft: das
  wäre derselbe Stillstand gewesen wie in v0.4.0 bei der
  Artefaktbeschaffung. Alle Eingaben laufen jetzt über eine Stelle.
- **Windows geprüft**, soweit ohne Windows-Maschine möglich: `auswahl`,
  `animation`, `banner` und `vergleich` übersetzen für
  `x86_64-pc-windows-msvc`; die Press/Release-Verdopplung der
  Windows-Konsole ist abgefangen. Ein Lauf auf echter Hardware steht aus.
- 53 → 148 Tests.


### v0.5.1 – 2026-08-20 (Nutzermenü auf drei Punkte)

- **Nutzermenü:** [1] Testlauf starten (Hardware, Determinismus, Shards,
  Stack in einem Zug), [2] Testdatei wählen, [3] Anleitung, [9]
  Entwickler-Menü. Mehr nicht.
- **Entwickler-Menü** statt Koordinator-Menü, mit den Einzelläufen, der
  Artefaktprüfung, dem Planerzeuger und den Einstellungen.
- **Gestrichen:** „Testplan laden (Pfad eintippen)". Punkt [2] listet die
  Dateien im Planordner und ersetzt das; für Skripte bleibt `--plan`.
  Eine Auswahlmöglichkeit, die niemand nutzt, kostet trotzdem
  Aufmerksamkeit.
- Ein Test hält das Nutzermenü bei höchstens fünf Punkten fest. Ohne
  solche Zusicherung wächst ein Menü über die Zeit wieder zu.

### v0.5.0 – 2026-08-20 (Testpläne, zwei Menüs, ein Protokoll)

- **Testplanauswahl beim Start,** vor der Modellfrage. Der Client sieht in
  `TESTCLIENT/Testpläne/` nach, listet auf, was er findet, und lässt
  wählen. Wird ein Plan gewählt, übernimmt er Prompt, Token, Shards und
  Modell, beschafft das Modell bei Bedarf und führt Determinismus- und
  Shard-Lauf selbst aus. Ein Plan mit falscher Prüfsumme wird
  übersprungen **und gemeldet**, nicht stillschweigend geladen.
  Beispielplan: `standard.plan`.
- **Zwei Menüs.** Das Nutzermenü hat fünf Punkte; alles, was Vorwissen
  voraussetzt, liegt unter [9] im Koordinator-Menü. Ein Menü mit zehn
  Punkten, von denen ein Teilnehmer fünf nie braucht, ist für ihn ein
  Hindernis.
- **Ein Protokoll statt vieler.** `logs/myl-test.jsonl` und
  `logs/myl-test.log`, angehängt statt ersetzt, keine Unterordner mehr.
  Nach wenigen Sitzungen standen dort Dutzende Ordner mit je zwei
  Dateien, und wer ein Ergebnis suchte, suchte zuerst den Ordner. Die
  Zuordnung leisten jetzt `run`, `command` und `settings_id` in jeder
  Zeile.
- **Klartext auf dem Bildschirm, nicht im Protokoll.** Nach jedem Lauf
  erscheinen Prompt und erzeugte Antwort lesbar. In der Protokolldatei
  stehen weiterhin nur Token und Prüfsummen; daraus ist der Text
  ableitbar, und die Datei bleibt schlank.
- **Fortschrittsanzeige** für Download und Bau: Schrittzahl und
  verstrichene Zeit, auf stderr, damit das Protokoll sauber bleibt.

### v0.4.0 – 2026-08-20 (Artefakte finden, wählen, beschaffen)

- **`artefakte`** (neuer Befehl): prüft für jedes bekannte Modell, ob es
  auf dieser Maschine liegt und ob es dem veröffentlichten Digest aus
  `INTEGER_LLM/scale_packs/REGISTER.json` entspricht.
- **Automatische Auflösung** für `determinismus` und `shard`: Findet der
  Client ein Artefakt, nimmt er es; findet er mehrere, fragt er; findet
  er keines, bietet er an, die Gewichte von Hugging Face zu holen und die
  Artefakte über das Skalenpaket zu bauen. Im Menü läuft das beim Start.
  Mit `--artifacts` gesetzt bleibt alles wie zuvor.
- **Bei `--quiet` wird nicht gefragt und nichts geladen.** Ein Zugriff
  über mehrere Gigabyte auf einen fremden Dienst gehört nicht in einen
  Skriptlauf, der ihn nicht angefordert hat.
- **Ein abweichender Digest wird ausdrücklich als *kein* Hardware-Befund
  gemeldet.** Ohne diesen Satz sähe ein anderes Artefakt aus wie eine
  gescheiterte Bitgleichheit, und der Client berichtete das Gegenteil
  dessen, wofür es ihn gibt.
- **Der Digest hängt an der Ankerkette,** nicht am Verzeichnisinhalt:
  `theta_v.json` pinnt die drei Manifeste, jene jede einzelne Gewichts-
  und LUT-Datei, und `loader.rs` prüft die Kette beim Laden. Die erste
  Fassung hashte den ganzen Ordner, dauerte über 8,7 GB Minuten und
  schlug bei belanglosen Streudateien Alarm.
- **Beim Bauen aufgefallen:** Die Beschaffung stand zunächst *nach*
  `stdin.lock()` der Menüschleife und rief `read_line()`. Der zweite
  Lock blockiert, solange der erste gehalten wird; der Client hing
  stumm. Sie steht jetzt davor.

### v0.3.0 – 2026-08-18 (Testplan und sortierte Ablage)

- **Testplan** (`spec.rs`): Der Koordinator erzeugt eine `.plan`-Datei
  mit Prompt, Token, Shards, Modell und einer Prüfsumme darüber; die
  Teilnehmer laden sie. Eine veränderte Datei wird abgelehnt
  (Exit-Code 3). Damit ist der häufigste Fehlalarm ausgeschlossen: ein
  versehentlich abweichender Prompt, dessen anderer Digest wie ein
  Befund an der Kernthese aussieht.
- **Der Prompt steht in Anführungszeichen.** Beim Bauen fiel auf, dass
  ein führendes oder abschließendes Leerzeichen im ungequoteten Format
  beim Einlesen verschwindet: es ist aber Teil des Prompts und
  verändert den Digest. Ein Test deckt Randleerzeichen, `=`,
  Anführungszeichen, Backslash und Zeilenumbrüche ab.
- **Protokoll-Ablage** nach `logs/<befehl>/<datum>_<einstellungs-id>/ (bis v0.4.0; seit v0.5.0 eine gemeinsame Datei)`
  mit `<uhrzeit>-<hardware>` als Dateiname. Alle Teilnehmer eines Plans
  landen im gleichnamigen Ordner. Die Einstellungs-Kennung steht auch
  **im** Protokoll, nicht nur im Pfad. Protokolle werden einzeln
  weitergereicht.
- Datum und Uhrzeit in UTC, von Hand aus Unix-Sekunden gerechnet (kein
  Datums-Crate). UTC bewusst: Teilnehmer sitzen in verschiedenen
  Zeitzonen, und ein Ordner je Zeitzone wäre genau die
  Zuordnungsarbeit, die vermieden werden soll.
- Argumentauswertung akzeptiert Optionen **vor** dem Befehl
  (`myl-test --plan x stack`): beim ersten Praxistest landete genau
  dieser Aufruf im Menü statt im Prüflauf.
- Menüpunkte 8 (Plan laden) und 9 (Plan erzeugen).
- 32 → 50 Tests.

### v0.2.0 – 2026-08-18

**Der Client prüfte nur zwei von neun Crates.** Determinismus (INTEGER_LLM)
und Sharding (COMPUTE_PIPELINE) waren abgedeckt; `myl-types`, `-ledger`,
`-scheduler`, `-consensus`, `-tokenomics` und `-verifier` fasste er nicht
an. Die haben Unit-Tests, aber niemand prüfte, ob sie **zusammen**
funktionieren, und genau dort lagen die schwersten Audit-Funde.

- **Neuer Befehl `stack`**: zehn Stufen von der Kryptografie über
  Epochenseed, Komiteewahl, BFT (mit echten Signaturen und Negativproben),
  Double-Signing, Blockstruktur, Verifikation und Ledger-Buchung bis zur
  Preisbildung. Läuft ohne Artefakte in ~1 s.
- **Fund A20, gefunden vom neuen Stack-Lauf:** `derive_epoch_seed` nahm
  die Epoche als Parameter entgegen, speicherte sie im `EpochSeed`, und
  ließ sie **nicht in den VRF-Eingang einfließen**. Folge: Ein Seed für
  Epoche 42 galt unverändert als gültiger Seed für Epoche 99, mit
  demselben Beweis (empirisch bestätigt). Zusätzlich hätten zwei Epochen
  mit demselben Vorgängerblock exakt dieselbe Zuteilung ergeben. Behoben
  in `myl-scheduler` v0.2.11 durch domain-getrenntes Alpha
  (`MYELITH_EPOCH_SEED_v1 ‖ block ‖ epoch`). **Konsensrelevant.**
- **Interaktives Menü**: `myl-test` ohne Unterbefehl öffnet eine
  Ziffernauswahl mit Erklärung je Punkt, Einstellungen und
  Kurzanleitung. Bewusst ohne TUI-Bibliothek, der Client soll über SSH
  und in einer seriellen Konsole funktionieren.
- **ASCII-Banner** nach dem Projektbanner (Knotennetz, Schriftzug, Zeile
  und die drei Schlagworte). Unterdrückbar über `--quiet` und
  `MYL_NO_BANNER`.
- **[ANLEITUNG.md](ANLEITUNG.md)** für Tests mit mehreren Beteiligten:
  vorher gab es nur acht Zeilen im README.
- 21 → 32 Tests.

### v0.1.0 – 2026-08-18
- Erstfassung mit drei Unterbefehlen: `hardware`, `determinismus`, `shard`.
- Protokollformat bewusst von Hand serialisiert: Wer zwei Läufe zweier
  Maschinen diffen will, braucht stabile Feldnamen und stabile
  Reihenfolge. Ein Logging-Framework mit konfigurierbarem Layout würde
  genau das aufweichen.
- 21 Tests, darunter: jede JSONL-Zeile ist für sich gültiges JSON,
  Sonderzeichen sprengen die Struktur nicht, ein unbeschreibbares
  Protokollverzeichnis bricht den Lauf nicht ab, der Fingerabdruck
  enthält keine Gerätekennung.
- **Ein Fund beim Bauen:** `integer_llm_runtime::paths` löst relativ zum
  Arbeitsverzeichnis auf. Für Läufe aus `INTEGER_LLM/` passt das, für
  einen Client, der von überall gestartet wird, nicht, der erste
  Determinismuslauf fand die Artefakte nicht. Der Client löst jetzt
  absolut auf; `INTEGER_LLM_ARTIFACTS_DIR` behält Vorrang.
