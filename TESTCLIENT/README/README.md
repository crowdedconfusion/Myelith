# testclient (`myl-testclient`)

> **Version:** 0.51.0
> **Datum:** 2026-10-25
> **Status:** Phase 1 und **Phase 3 vollständig**, dazu Punkt 2.1
> (`vergleich`), **2.2** (Backend-Vergleich innerhalb einer Maschine, seit
> dem 2026-08-30) und 2.4 (`--repeat`); **Phase 4 vollständig** (4.3 die
> Fremdmaschinen-Automatik, 4.1 der Konformitätslauf als fünfte Stufe,
> 4.2 die Maschinenbeschreibung im Protokoll). Offen bleibt in Phase 2
> der Lauf selbst, also die zweite Architektur, und 2.3 (bestätigte
> Ergebnisse ablegen). 281 Tests grün,
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
myl-test plan --plan-id 2026-08-21-cross-arch-01 --model sparrow-0.6b-int8 \
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
rechnet, beantwortet `BENCHMARKS/Inferenz` über die Perplexität gegen die
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

⚑ **Und ein Lauf, der kein Befehl ist.** `tests/gesamtlauf.rs`
(`cargo test --test gesamtlauf -- --nocapture`) geht **einen** Auftrag
durch **zwölf Stationen**, von der Parametertabelle bis zur Auszahlung,
und führt dabei eine Stationsliste mit, die am Ende gegen eine feste
Reihenfolge geprüft wird. Er steht hier, weil `myl-testclient` die
einzige Kiste ist, die alle anderen sieht.

⚑ **Und ein zweiter, der weiter greift.** `tests/verzahnte_simulation.rs`
(`cargo test --test verzahnte_simulation -- --nocapture`) fährt **zwölf
Stationen über eine laufende Kette**, und zwar nicht nacheinander,
sondern ineinander: Zwölf Miner in drei Zonen melden sich an, der
Scheduler bildet daraus Pods (auch aus dünn besetzten Zonen, über den
Sammeltopf), ein Nutzer verbrennt MYL und gibt Credits aus, die
Nachfrage rollt in die nächste Epoche, der Trainingsplan lost ein
Podpaar, beide liefern ihr Segment, ein Segment mit fremder Charge wird
abgewiesen, der Epochenschluss rückt die Modellfassung und zahlt aus,
ein uneiniges Paar bewegt nichts, und ein überführter Miner wird
geschnitten.

⚑ **Was dieser Lauf ausdrücklich nicht kann, und warum das dort
steht.** Die Auslastung bleibt null. `PodKapazitaet` ist mit 3,6
Milliarden vTFE je Pod und Epoche eine Zahl für ein echtes Netz;
achtzehntausend vTFE aus einer Probeausgabe sind dagegen auf der
Festkommaskala exakt null. **Das ist die Grenze der Simulation und kein
Fehler in ihr:** Geprüft wird, dass die Zahl ankommt und in die
Vorepoche rollt. Wie Trainingsanteil und Abgabe daraus folgen, prüft der
Lauf an der Regel und nicht an der Kette, und er schreibt beides ins
Protokoll, damit der Unterschied sichtbar bleibt.

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
    ├── tests/                die Nähte zwischen den Kisten
    │   ├── gesamtlauf.rs     alle zwölf Stationen an einem Auftrag
    │   ├── harness_bis_modell.rs  ein Nutzeraufruf bis zum echten Modell
    │   ├── tuer_bis_rechenwerk.rs der Weg, fünf Kisten, echte Sockets
    │   └── kalter_pfad.rs    Auftrag über das Netz, ohne Gateway
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

### v0.51.0 (2026-10-25): Der Podlauf: Netz, Rechnen und Prüfen in getrennten Prozessen

- `netzbedingungen/podlauf.py` (neu, auf dem Netzlauf mit getrennten
  Prozessen): ein Erzeuger und zehn Miner, jeder mit Laufzeit und Verlust
  (`tc netem`), jeder Miner mit eigenem `myl-shard` und dem Probeartefakt.
  Verlangt werden Anmeldung und Zusage über die Kette, Pods mit den Grenzen
  der Kette, Zeitproben über die Shards der anderen (echte
  Vorwärtsrechnungen über die gestörten Leitungen), Meldung und Sperre des
  Lügners samt einer Epoche ohne seinen Shard, kein ehrlicher Miner
  gesperrt, Neustart eines abgestürzten Shards und eines abgestürzten
  Miners, am Ende eine Wurzel. `--kurz` für die Mechanik, sonst
  `--epochen <n>` Dauer mit Störungen, `--stoerer` dazu.
- Noch nicht dabei: Anfragen über die Tür und die Stichprobe der Segmente.
- Langer Lauf bestanden: 381 Epochen unter `tc netem` mit Störer, 1 755
  Zeitproben, 102 Meldungen, kein ehrlicher Miner gesperrt, 77 Shard- und
  55 Minerabstürze (Neustart bis 2,5 s, aufgeholt bis 26,6 s), eine Wurzel.
- 📌 **Fund 638:** `tests/zwei_prozesse.rs` räumt Ausweis und Schlüssel beim
  Fallen weg. Mit `TMPDIR` auf einen leeren Ordner geprüft: Die ganze Kiste
  hinterlässt nichts.
- 📌 **Fund 640:** Im Netzlauf verlangte die Probe, dass der Holer eine
  Datei holt, auch wenn er nie eine Position bekam; die Pods verteilt der
  Zufall der Kette, und in einem von zwanzig Läufen stand er nur in der
  Reserve. Jetzt gilt: Hat er seinen Shard gestartet, muss er geholt haben
  (ohne Lückenfüllung startet er nicht). Gegenprobe mit ganzem Ordner für
  den Holer: rot. Netzlauf 20 mal: 19 grün, der rote war dieser Fall;
  Fund 618 kam nicht wieder.
- Der Podlauf legt je Epoche und am Ende der Dauerphase ein Speicherbild
  je Miner ab (`speicher.jsonl`: anonym und aus Dateien, Fäden, Arenen von
  glibc samt beschriebenem Teil); `--arenen <n>` setzt `MALLOC_ARENA_MAX`
  für jeden zweiten Miner, zum Vergleich der Speicherhaltung. Im kurzen
  Lauf: ungedeckelt 46 bis 48 Arenen (eine je Faden, 47 Fäden), 19 bis
  26 MiB beschrieben; gedeckelt eine, 5 bis 7 MiB.
- 📌 **Fund 641:** Der Podlauf verlangte eine Position des Lügners bis
  Epoche 8; in einem von rund zehn kurzen Läufen bekam er keine. Jetzt bis
  Epoche 16 (`LUEGNER_BIS`), was nur kostet, wenn er spät drankommt.
- 📌 **Fund 642:** `tests/zwei_prozesse.rs` benannte seine Ordner nach
  Prozessnummer und Uhr; die Uhr von macOS zählt Mikrosekunden, und zwei
  Proben bekamen im frischen Gesamtlauf denselben Ordner. Seit Fund 638
  räumt `Aufbau` ihn beim Fallen weg, und die eine nahm der anderen Ausweis
  und Schlüssel („Tür nicht geöffnet: No such file or directory“). Jetzt
  mit einem Zähler im Namen. Gegenprobe mit erzwungen gleichem Namen: drei
  von drei rot; danach zehn von zehn grün, ganze Kiste 295 grün ohne Reste.
- `myl-testclient` 0.51.0 (der Lauf ist ein Skript, die Kiste zieht mit).

### v0.50.2 (2026-10-25): Netzlauf gleicht vor der Schlussprüfung an, netem-Auswertung ohne letzten Erzeugerblock (Funde 636, 637; Fund 618 eingegrenzt)

- **Fund 636:** Die Schlussprüfung des Netzlaufs (`tests/netzlauf.rs`)
  verglich die Zustandswurzeln direkt nach einer Schleife, die je Runde
  einen Block baut; ein Knoten konnte ihn in seinen 25 ms noch nicht haben.
  In 4 von 20 Nachtläufen rot mit „die Ketten laufen auseinander“, mit
  Höhen in der Meldung `[5490, 5489, …]`: ein Block zurück, keine Gabelung.
  Jetzt erst angleichen, dann vergleichen; die Meldung nennt die Höhen.
- **Fund 637:** Die Auswertung von `netzbedingungen/netzlauf_prozesse.py`
  stürzte mit `KeyError` ab, wenn das Signal den Erzeuger zwischen
  Verbreiten und `block_erzeugt` traf: Die übrigen standen einen Block über
  seinem Protokoll. Dann vergleichen jetzt die übrigen untereinander; mehr
  als ein Block darüber bleibt ein Befund.
- **Fund 618 eingegrenzt:** Jedes gescheiterte Angleichen im Netzlauf hat
  eine eigene Meldung mit den Höhen aller Knoten (vorher dreimal wortgleich
  „die Knoten holten nicht auf“). Nachgestellt in rund 5 von 20 Läufen, auf
  dem Stand vom 2026-10-23 ebenso oft; meist nach der Epochengrenze
  („in der Epoche danach“), seltener nach dem Neustart der drei Ausfälle.
- `myl-testclient` 0.50.2.

### v0.50.1 (2026-10-24): `tokio` 1.54.1 wegen RUSTSEC-2026-0386

- Die Sperrdatei hebt `tokio` von 1.53.1 auf 1.54.1: Die sicheren Konstruktoren von `AsyncFd` verletzen die I/O-Sicherheit (RUSTSEC-2026-0386, `unsound`, neu seit dem 2026-10-23), und cargo-deny war damit rot. Der eigene Code nutzt `AsyncFd` nicht; geändert hat sich nur die Sperrdatei, der Vorrat trägt die neue Fassung. `myl-testclient` 0.50.1.

### v0.50.0 (2026-10-24): Der Archivlauf nennt ersetzte ehrliche Kopien und den Wiederanlauf

- ⚑ **Ersetzt werden nur Sybil:** Der volle Lauf liest je ehrlichem Knoten
  die Folge seiner Rollen; wird aus einer Kopie etwas anderes als ein
  Halter, ist sie ersetzt worden, und der Lauf meldet sie beim Namen und
  ist rot. Gegen die Protokolle des Laufs von v0.49.0 gefahren, meldet die
  Prüfung genau die beiden dort ersetzten (k7 bei 361 800, k11 bei
  354 600); die Zählung des Zustands allein unterschied sie nicht von den
  Sybil.
- **J1:** Die Proben mit Abbuchung (`gesamtlauf`, `tuer_bis_rechenwerk`)
  hängen der Vollmacht den Empfängerbeweis des Betreibers an, wie es ein
  Inhaber tut; ohne ihn bucht die Kette nicht mehr ab, und genau daran
  wurden beide rot.
- **Wiederanlauf im Absturzbericht:** ab welchem Abzug der Knoten nach dem
  Absturz ansetzte, wie viele Blöcke er nachrechnete und ob er den Abzug
  verwarf (NODE 0.60.0, Fund 629).
- 📌 **Fund 631, im ersten vollen Lauf mit 0.50.0 gefunden:** Die neue
  Prüfung nannte ihre Schleifenvariable `vorher`, und derselbe Name trägt
  weiter unten die Wurzel der Einigung. Die Einfuhr rechnete richtig
  (367 278 Blöcke ab Genesis auf `34b9fc574c82d934`, die Wurzel der
  Einigung), geprüft wurde sie aber gegen die zuletzt gelesene Rolle
  („erwartet Splitter“), und der Lauf war rot. Die Variable heisst jetzt
  `vorige_rolle`.
- **Voll, bis zur Einfuhr** (NODE 0.60.0, `myl-ledger` 0.26.0): zwölf
  ehrliche, sechs Sybil, Störer, drei Langzeitgrenzen; elf Folger einig bei
  367 278; **keine ehrliche Kopie ersetzt** (vorher zwei); zehn Abstürze,
  aufgeholt in 3,6 bis 15,5 s (vorher 18 bis 208 s), jeder ab dem eigenen
  Abzug mit 141 bis 1 699 nachgerechneten Blöcken. Die Wiederholung bis zum
  Ende läuft.

### v0.49.0 (2026-10-24): Der Archivlauf (Stufe A9 der Archivknoten)

- ⚑ **Neu: `netzbedingungen/archivlauf.py`**, neben dem netem-Lauf und mit
  dessen Bausteinen (Knoten in Namensräumen, Brücke, Störer). Ehrliche
  Knoten, jeder als Miner angemeldet und über die Zonen verteilt, mit einem
  Platzbudget von einem MiB, damit jeder verdichtet, sobald er darf; dazu
  ein Drittel **Sybil-Miner**, die sich anmelden und verschwinden: Gezogen
  belegen sie nie und müssen ersetzt werden. Über mehrere Langzeitgrenzen,
  mit einem Absturz alle 15 Minuten und auf Wunsch dem Störer. Verlangt:
  Einigkeit am Ende, Belege, eine geltende Teilwurzel, eine bestätigte
  Übergabe, Verdichtung, mindestens eine Kette ab Genesis. Dann der
  **Totalausfall**: alle Knoten halten an, alle Ketten werden gelöscht, die
  kalte Ausfuhr eines Halters kommt in einen Knoten zurück, der sie ab
  Genesis nachrechnet und auf derselben Wurzel weiterbaut; zwei Knoten
  steigen über den Abzug ein und folgen.
- **Kurz** (vier ehrliche, zwei Sybil, keine Langzeitgrenze): Anmeldung von
  sechs Minern, Ausfuhr von 3 956 Blöcken, Einfuhr nach dem Totalausfall,
  nachgerechnet auf dieselbe Wurzel, Einsteiger an der Spitze, bestanden.
  📌 **Der erste kurze Lauf war rot und fand Fund 626** (NODE 0.58.0): Die
  Einsteiger blieben auf der Höhe ihres Abzugs, weil der andere Einsteiger,
  der selbst nichts hatte, jedes Mal den vordersten Bereich bekam.
- **Voll** (zwölf ehrliche, sechs Sybil, Takt 25 ms, Störer, drei
  Langzeitgrenzen bis Höhe 367 200, gelaufen 2026-10-24 04:10 bis 06:50):
  bestanden. Elf Folger einig bei 367 363 auf einer Wurzel; je Periode
  sieben Kopien, alle sieben vollständig belegt, die Übergabe bestätigt,
  drei Teilwurzeln galten. 720 Belege, 757 Teilbelege, sechs
  Verdichtungen, kein abgelehnter Abruf. Ersetzt wurden je Periode 4, 1 und
  3: in den ersten beiden nur Sybils, in der dritten dazu zwei ehrliche
  (siehe unten). Neun Abstürze, aufgeholt in 18 bis 208 s; Störer 100
  Runden, 35 158 Verbindungen. Mempool beim Erzeuger höchstens 15, bei den
  Folgern meist unter 250, beim Aufholen nach einem Absturz bis 1718.
  Totalausfall: Ausfuhr von 367 363 Blöcken in vier Dateien, Einfuhr ab
  Genesis in 162 s nachgerechnet auf dieselbe Wurzel, zwei Einsteiger über
  den Abzug 367 200 an der Spitze.
  📌 **Der erste volle Lauf war rot und fand Fund 627** (NODE 0.59.0):
  Eine überholte Transaktion fiel aus dem Block, und ihr Absender zählte
  weiter, bis 16 von 18 Minern ersetzt waren und die Folger an ihren
  wachsenden Mempools zurückfielen.
  ⚠️ **Was der Takt von 25 ms verzerrt:** Fristen zählen in Höhen,
  Nachladen, Neustart und Laufzeit im Netz dauern Sekunden. Eine neue Kopie,
  die als Splitter verdichtet hatte, brauchte 509 s, um 115 200 Blöcke
  nachzuladen, und ihre Frist von acht Epochen sind hier rund 400 s; eine
  andere war nach einem Absturz 200 s fort, hier fast fünf Epochen. Beide
  wurden ersetzt. Ebenso melden Folger, die mehr als 64 Höhen (1,6 s)
  hinter dem Erzeuger liegen, eine Lücke in den eigenen Nummern, obwohl die
  Transaktion schon im Block steht; die erneut vergebene Nummer weist der
  Erzeuger als verbraucht ab, und die Pflicht wiederholt sich, bis der
  Zustand sie zeigt.
- `myl-testclient`: die Knotenkonfigurationen der Proben tragen das neue
  Feld `archiv_ausfuhr` (NODE 0.58.0).

### v0.48.0 (2026-10-23): Der netem-Lauf bekommt einen Dauermodus mit Störungen und einen Störer; `myl-test netz` liest rotierte Protokolle

- **`myl-test netz` und rotierte Protokolle** (zu Fund 624, NODE 0.57.0):
  Ein Knoten beginnt eine neue Protokolldatei, wenn die laufende voll ist,
  mit einer ersten Zeile `fortsetzung`. `sammle_mit_bericht` legt die
  Dateien eines Laufs darüber wieder zusammen (`lies_lauf`), je Knoten den
  jüngsten Lauf; ein Lauf, dessen Anfang unter dem Deckel geräumt wurde,
  beginnt mit der ersten Fortsetzung. `Knotenbild::erste_folge`: lückenlos
  heisst jetzt „keine fehlende Nummer im Aufbewahrten“, ein fehlender Anfang
  ist keine Lücke. Probe `rotierte_dateien_eines_laufs_werden_zusammengelegt`;
  **Gegenprobe** (Fortsetzung nicht verbunden): rot.
- Die Knotenkonfiguration der Proben trägt die neuen Felder `protokoll_mb`
  und `kette_mb`.

`TESTCLIENT/netzbedingungen/netzlauf_prozesse.py`, für die langen Läufe am
Abend und in der Nacht:

- **`--dauer <Minuten>`**: Nach den fünf Stolpersteinen und vor dem Schluss
  läuft das Netz weiter, und eine feste Folge (`--saat`) stört es Runde für
  Runde: Trennung (5 bis 30 s), Absturz mit Neustart (dieselbe Peer-Id
  verlangt), Verlustspitze (alle Leitungen 20 %), Laufzeitsprung (dreifach),
  oder Ruhe. Nach jeder Störung müssen alle in 180 s an die Spitze, in jeder
  Ruhe darf der Rückstand die Grenze des Laufs nicht überschreiten.
- **Speicher je Runde** aus `/proc` (`VmRSS`, offene Dateien). Ein Knoten,
  der nie neu startete und dessen offene Dateien sich mehr als verdoppeln,
  ist ein Befund (Leck); der Speicher steht im Protokoll.
- **`--stoerer`**: eine bösartige Gegenstelle in eigenem Namensraum und
  eigenem /24, als eigener Prozess. Je Runde und Ziel: bis zu 16 halbe
  Verbindungen offen halten (ein Byte je Runde), 4 KiB Zufall schicken und
  schließen, 20 Verbindungen öffnen und schließen. Sie meldet sich beim Start
  und nach jeder Runde. 📌 In der ersten Probe zählte sie nur alle 30 Runden,
  und eine Runde dauert über die gestörten Leitungen rund eine Minute: In
  fünf Minuten kam keine Zeile, und ob sie wirkte, liess sich nicht belegen.
- Der Anteil über Gossip wird für Knoten, die der Dauerlauf getrennt oder
  neu gestartet hat, nicht mehr verlangt.
- **Probe** (`--kurz --dauer 5 --stoerer`, Docker, 2026-10-23): fünf Runden
  (zwei Abstürze, zwei Trennungen bis 29 s, ein Laufzeitsprung), aufgeholt in
  6,9 bis 28,4 s, grösster Rückstand in der Ruhe 56 bei erlaubten 120; am Ende
  sechs Knoten auf einer Wurzel bei Höhe 19 348; Speicher je Knoten von rund
  20 auf 22 bis 34 MiB, offene Dateien unverändert bis 30.
- Auf dem heutigen Stand (myl-node 0.56.1 mit den Audit-Änderungen) davor
  bestanden: kurz (jeder kennt jeden 4,4 s, Neuling 2 505 Blöcke in 21,4 s,
  Abzug bestätigt 33,3 s) und voll (acht Knoten, 7,7 s, Rückstand 3, Trennung
  8,5 s, Absturz 7,2 s, Neuling 2 293 Blöcke in 21,7 s, Abzug 29,1 s, eine
  Wurzel bei 2 967, kein Block verloren). Fund 618 kam dabei nicht wieder.
- **Erster Dauerlauf** (acht Knoten, Störer, 2026-10-23 19:47, auf Wunsch
  des Projektinhabers nach 1 h 40 min abgebrochen): 88 Runden (16 Abstürze,
  14 Trennungen, 12 Verlustspitzen, 24 Laufzeitsprünge, 22 Ruhe), aufgeholt
  im Mittel in 16,2 s, höchstens 59,4 s (Grenze 180); Störer 79 Runden,
  18 376 Verbindungen, 166 abgewiesen, 2 494 Müllsendungen, 12 535 Stöße.
  **Einigkeit vor dem Abbruch** aus den Protokollen geprüft: alle acht Knoten
  auf 13 969 gemeinsamen Höhen (114 823 bis 130 471) mit derselben Wurzel und
  demselben Blockhash, Spitzen innerhalb von 14 Blöcken. Offene Dateien je
  Knoten 22 bis 43, ohne Wachstum; Speicher von 20 bis 25 auf 34 bis 56 MiB
  (Fund 625).

### v0.47.1 (2026-10-23): Der Shard des Knotens antwortet nur mit Ausweis (Funde 599, 611)

Aus dem Sicherheitsaudit vom 2026-10-23.

- `stufe4.rs` (`der_knoten_startet_seinen_shard_aus_der_kette`): Der Test
  holt Adresse **und** Ausweis über `Shardfuehrung::tuer`, fragt damit Schlüssel,
  Zuschnitt und Zeitprobe, und prüft neu, dass der echte, vom Knoten
  gestartete Shard **ohne** Ausweis nicht antwortet (Fund 611).
  **Gegenprobe:** Mit dem richtigen Ausweis im „fremden“ Weg fällt der Test
  mit „der Shard antwortete ohne Ausweis“.
- **Fund 599:** `ctutils` 0.4.5 und `chacha20` 0.10.2 in der Sperrdatei.

### v0.47.0 (2026-10-23): Der Lauf unter `tc netem` mit gemischten Fassungen

- `netzlauf_prozesse.py --zweitprogramm <myl-node>`: Die ungeraden Knoten
  laufen mit einem zweiten Programm, etwa der Fassung vor einem Update. Das
  trifft den Getrennten (k1), den Knoten, der nur über mDNS findet, und den
  Neuling aus dem Abzug; Erzeuger, Abstürzer und der Neuling, der alles
  nachholt, laufen mit `--programm`. Die Zeilen der Knoten nennen, wer das
  zweite Programm fährt. Ohne den Schalter bleibt der Lauf, wie er war.
- ⚑ **Erster Einsatz: libp2p 0.56 neben 0.57.** Zweites Programm war
  `myl-node` aus dem Stand vor dem Sprung (`myl-net` 0.19.0, libp2p 0.56;
  im Programm abgelesen: mdns 0.48, quic 0.13, swarm 0.47). Beide Läufe
  bestanden:
  - kurz: jeder kennt jeden nach 4,4 s, Rückstand 0, Neuling 2 848 Blöcke
    in 27,1 s, Neuling (alt) aus dem Abzug bestätigt in 35,2 s, eine Wurzel
    bei Höhe 4 380;
  - voll (acht Knoten): jeder kennt jeden nach 5,8 s, Rückstand höchstens 8,
    Trennung (alt) aufgeholt in 7,5 s, Absturz in 13,4 s, Neuling 2 205
    Blöcke in 11,8 s von 6 Gegenstellen, Neuling (alt) aus dem Abzug
    bestätigt in 26,8 s, eine Wurzel bei Höhe 2 815.
  - **QUIC zwischen den Fassungen:** 125 von 135 Verbindungen liefen über
    QUIC, auch zwischen alten und neuen Knoten in beide Richtungen; der neue
    Schlüsseltausch fällt also sauber auf X25519 zurück. Die Abweisungen
    gleichen nach Art und Zahl denen des Laufs mit nur 0.57.

### v0.46.1 (2026-10-23): libp2p 0.57; nur die Sperrdatei

- Sperrdatei auf libp2p 0.57.1 (über `myl-net` 0.20.0 und `myl-node`
  0.55.1); damit `hickory-proto` 0.26.3 statt 0.25.2 (RUSTSEC-2026-0118,
  -0119).
- **Belege:** Clippy (`-D warnings`) und 278 Tests offline grün, darunter
  der Netzlauf mit fünfzehn Knoten, ungleichen Pods und Stolpersteinen
  (41 s).

### v0.46.0 (2026-10-22): Der Netzlauf mit getrennten Prozessen unter `tc netem`

- `netzbedingungen/netzlauf_prozesse.py` (neu, eigenständiges Skript): jeder
  Knoten ein eigenes `myl-node` in einem eigenen Netz-Namensraum, alle an
  einer Brücke, jeder in einem eigenen /24 eines gemeinsamen /16. `tc netem`
  auf beiden Seiten jeder Leitung: 50 bis 200 ms je Richtung mit
  Schwankung, 1 bis 5 % Verlust. Der Erzeuger baut mit `--blocktakt 50`
  (20 Blöcke je Sekunde), die übrigen schicken Testverkehr, einer findet das
  Netz nur über mDNS. Stolpersteine: Trennung (20 s ohne jedes Paket),
  harter Absturz mit Neustart aus Kettendatei und Schlüssel (gleiche
  Peer-Id), ein Neuling ohne Kette und ohne Angabe. Gelesen wird von aussen,
  wie ein Betreiber es täte: Höhe und Gegenstellen über `/metriken`, Wurzeln
  aus den Protokollen.
- Am Ende muss gelten: alle übrigen auf einer Höhe und dort dieselbe Wurzel
  wie der Erzeuger bei diesem Block; die Wurzel bewegt sich (mindestens zehn
  Werte, Fund 593); kein Knoten wählt die Rückschleife eines anderen (Fund
  594); wer nicht neu startete, übernimmt mindestens die Hälfte seiner
  Blöcke über Gossip statt über das Nachholen (Fund 592). Was der Erzeuger
  beim Ende noch allein hatte, darf verloren gehen, höchstens drei Sekunden.
- `--kurz` geht denselben Weg mit fünf Knoten und kurzen Abschnitten, als
  Verifikationslauf vor dem vollen. Rückgabe 0 bestanden, 1 ein Befund,
  2 die Umgebung fehlt (Linux, root, `ip`, `tc`, `sch_netem`).
- Die Literale von `KnotenKonfig` tragen das neue Feld `blocktakt_ms`
  (`src/knoten.rs`, `tests/netzlauf.rs`, `tests/kalter_pfad.rs`).
- **Belege:** Der erste Lauf fand drei Fehler im Knoten und im Netz (NODE
  v0.54.0, NETWORKING v0.19.0). Danach voller Lauf grün: sieben Knoten,
  jeder kennt jeden nach 7,4 s, Rückstand höchstens 3 Blöcke, Trennung in
  11,7 s und Absturz in 9,4 s aufgeholt, Neuling 2 570 Blöcke in 31,9 s,
  Ende bei Höhe 2 604 mit einer Wurzel und 381 verschiedenen auf dem Weg,
  kein Block verloren. Gegenprobe ohne Vorausspeicher rot. Der Netzlauf im
  selben Prozess mit Probeartefakt grün in 41 s (vorher 51 bis 58 s);
  `cargo test` ohne Artefakte rc=0 (294), Clippy still.
- `rust-version` auf 1.99 in `myl-testclient`; die Werkzeugkette steht einmal, in
  `rust-toolchain.toml` (1.99.0), mit der auch die CI baut. Nur das Manifest.
- **Stolperstein 5 (Zustandsabzug):** ein zweiter Neuling beginnt beim
  Abzug der ersten Epochengrenze; den Anker liest der Lauf aus der Zeile
  `abzug_angelegt` des Erzeugers, wie ein Betreiber es täte. Er muss die
  Spitze erreichen, seine Hintergrundprüfung die Basis bestätigen, und am
  Ende trägt er dieselbe Wurzel. Kurzer Lauf: an der Spitze und bestätigt in
  34,8 s, 6 325 Bytes Zustand. Der kurze Lauf baut dafür mit 25 ms je Block:
  📌 Bei 10 ms holte ein Neuling über die schlechteste Leitung (200 ms, 5 %
  Verlust) aus einer Quelle nur rund 75 Blöcke je Sekunde nach, rund 6 s je
  Lieferung von 512 Blöcken, und das Netz wuchs schneller.
- `tests/netzlauf.rs`: 📌 **Der Lügner bekommt verlässlich eine Position.**
  In rund drei von zehn Läufen stand er in Epoche 2 in der Reserve, und
  Stolperstein 1 fiel aus; jetzt läuft das Netz Epoche um Epoche weiter, bis
  er rechnet, höchstens bis Epoche 6.
- Die Literale von `KnotenKonfig` tragen `abzug` und `hintergrundpruefung`.

### v0.45.0 (2026-10-22): Netzlauf mit Einstieg ohne Angabe (Stolperstein 7)

- `tests/netzlauf.rs`: `m05`, `m07` und `m08` bekommen keine
  Bootstrap-Adresse und suchen im lokalen Netz; `m04` sucht ebenso und kennt
  den Erzeuger (die Brücke). Die Prüfung nach dem Aufbau verlangt jetzt,
  dass jeder alle 14 anderen kennt, statt nur eine Gegenstelle.
- **Stolperstein 6 härter:** drei Knoten (`m01`, `m06`, `m13`) fallen
  zugleich aus, das Netz baut 200 Blöcke weiter, alle drei kehren mit
  derselben Miner-Kennung zurück und holen auf.
- **Stolperstein 8 (neu): ein Neuling** ohne Kette, ohne Angabe und ohne
  Miner-Rolle findet das Netz über mDNS, holt jeden Block nach und führt
  danach dieselbe Kette wie alle (`pruefe_einigkeit`).
- **Belege:** zweimal grün, alle 15 Knoten mit 14 Gegenstellen (64 s, 74 s);
  mit beiden neuen Stolpersteinen noch zweimal grün (99 s, 92 s), der
  Neuling bei Höhe 3867 bzw. 3864 mit 15 Gegenstellen; mit
  `MYL_OHNE_ARTEFAKTE=1` Rückgabe 0.
- **Stolperstein 1 ehrlicher:** Steht der Lügner in Epoche 2 in der
  Reserve (in rund drei von zehn Läufen), meldet der Lauf das sichtbar.
  📌 Und die Schlussprüfung verlangte dann „niemand gesperrt“, obwohl er in
  Epoche 3 rechnen und gemeldet werden kann: ein versteckter Wackler. Jetzt
  darf nur der Lügner gesperrt sein, und mit Position in Epoche 2 muss er es.

### v0.44.1 (2026-10-22): Netzlauf und Stufe-4-Proben in der CI; Sicherheitsmeldung zu `rustls-webpki`

- 📌 **Die neuen Proben kannten `MYL_OHNE_ARTEFAKTE` nicht.** Der CI-Schritt
  „cargo test, TESTCLIENT“ war nach dem Push von v0.44.0 rot: Dort liegt
  kein Probeartefakt, und `tests/netzlauf.rs` sowie zwei Proben in
  `tests/stufe4.rs` verlangten es ohne Ausnahme. Jetzt fragen alle drei
  `myl_pod::artefakte::vorhanden`, wie die übrigen: ohne Artefakt nur mit
  ausdrücklicher Erlaubnis übersprungen, sonst ein lauter Fehlschlag.
- 📌 **Der Netzlauf wartete auf „irgendeinen Auftrag“.** Ein Knoten mit
  dem Auftrag der vorigen Epoche bestand die Warteschleife, und die Prüfung
  danach fand ihn mit dem alten Bereich (m02 mit (0, 7) statt (27, 28)).
  Der Knoten selbst tat das Richtige; der Wechsel kommt mit seiner nächsten
  Aufnahme. Jetzt warten beide Schleifen (Epoche 2 und 3), bis jede
  Position Epoche und Bereich ihres Platzes führt und die Reserve nichts
  (`soll_der_pods`, `fuehren_ihr_soll`).
- `rustls-webpki` 0.103.15 auf 0.103.16, über `myl-net` (0.17.1) und
  `myl-node` (0.52.1) eingezogen, wegen RUSTSEC-2026-0382.
- **Belege:** mit `MYL_OHNE_ARTEFAKTE=1` wie in der CI Rückgabe 0 und drei
  Sprünge; mit Artefakten Rückgabe 0, der Netzlauf dreimal grün;
  cargo-deny grün; Clippy mit `-D warnings` und alle Tests offline gegen
  den Vorrat mit Rückgabe 0.

### v0.44.0 (2026-10-22): Stufe 4 über den ganzen Weg

- `tests/stufe4.rs` (neu, P9b): Überzusage gemeldet und gesperrt (über die
  Kette, eine Epoche lang), Gleichstand auf zwei Knoten, zu wenig
  Kapazität ergibt keinen Pod, Ebenenzeit 1 begrenzt die Kapazität,
  Teilablage hält den Bereich; mit echtem Prozess: Der Knoten startet
  seinen Shard aus der Kette (Schlüssel der Anmeldung, Bereich der
  Position, Zeitprobe über die Tür); der Dateidienst holt eine Ebene des
  Probeartefakts.
- `gesamtlauf` und `verzahnte_simulation` mit Zusagen (kein Pod ohne
  Zusage).
- `tests/netzlauf.rs` (neu): **der Netzlauf**. Fünfzehn Knoten in einem
  Prozess (ein Erzeuger, vierzehn Miner), verbunden über Bootstrap und
  Kademlia, echte `myl-shard`-Prozesse mit dem Probeartefakt, ungleiche
  Zusagen, zwei ungleich geschnittene Pods. Stolpersteine: ein Lügner
  (1 µs je Ebene, gemessen, bezeugt, gesperrt), ein Schwacher ohne Pod,
  Teilablage, ein Holer mit einer Lücke je Ebene, ein abgestürzter Shard,
  ein ganz ausgefallener Knoten, der aus seiner Kettendatei zurückkehrt;
  dazu 200 Blöcke je Runde, also Blöcke in falscher Folge. Geprüft: dieselbe
  Kette, Zuteilung und Grenzen auf allen Knoten, jeder Shard mit seinem
  Bereich, nur der Lügner gesperrt, Dateien geholt und keine verworfen.
  Der Lauf fand die Funde 587, 588 und 589 und die vergessene Aufnahme; mit
  ihren Behebungen grün in rund 70 s.
- **Belege:** `cargo test` rc=0.

**Ältere Einträge** (v0.43.4 und davor, 66 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- TESTCLIENT/README/README.md`.
