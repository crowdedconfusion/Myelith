# testclient (`myl-testclient`)

> **Version:** 0.8.0
> **Datum:** 2026-08-22
> **Status:** Phase 1 vollständig, dazu Fahrplanpunkt 2.1 (`vergleich`),
> 2.4 (`--repeat`) und 3.1 (Modellstand im Protokoll). 160 Tests grün, alle
> Läufe gegen die echten Artefakte verifiziert.
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
Fahrplans und keine Höflichkeit: Ein Werkzeug, das zwei gleiche Werte von
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

Zwei Pläne liegen bei: `qwen2.5-0.5b-standard.plan` (6 Prompts) und
`qwen2.5-7b-standard.plan` (4 Prompts, rund fünf Minuten Laufzeit).

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
3. **`determinismus`** und **`shard`** sind die eigentlichen Tests. Beide
   brauchen ein Modell und lösen es selbst auf.
4. **`stack`** geht ohne Modell durch Krypto, Epochenseed, Komiteewahl,
   BFT, Verifikation, Ledger und Tokenomics.

Jeder Lauf schreibt ein Protokoll nach `TESTCLIENT/logs/`,
maschinenlesbar als `.jsonl` und lesbar als `.log`. Für einen Vergleich
zwischen zwei Maschinen zählen diese Dateien, nicht die Bildschirmausgabe.

### Voraussetzungen

Rust. Fehlt es, nennt der Starter die Installationszeile und bricht ab.
Ist cargo nicht da, aber ein gebautes Binary vorhanden, benutzt er
dieses. Für den Artefaktbau kommt Python hinzu, das mit dem Repository
mitgeliefert wird (`INTEGER_LLM/calibrate/.venv`).

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
| `determinismus` | INTEGER_LLM (runtime, kernels) | **ja** |
| `shard` | COMPUTE_PIPELINE (myl-pod) + INTEGER_LLM | **ja** |

**Nicht abgedeckt:** `myl-net` (Gossip über echte Sockets gehört in die
NETWORKING-Testsuite) und BFT-Liveness (Rundenwechsel fehlt noch,
CONSENSUS Punkt 3.6). Die vollständige Abgrenzung steht in
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
│   ├── ANLEITUNG.md          Tests mit mehreren Beteiligten
│   └── Fahrplan-v1.md        Phasenplan
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
| `determinismus --plan qwen2.5-0.5b-standard` | 6 Prompts × 32 Token, je zwei Läufe **bitgleich**, Gesamtwert `fd64588fd46a7af8…`, 29 s |
| `shard --shards 4 --steps 4` | Pod (Layer 0–6/6–12/12–18/18–24) **bitgleich** zur Einzelknoten-Runtime, Digest `6541c129…` |
| `stack` | 10 von 10 Stufen bestanden in 54 ms, Gesamtwert `a9af743f…` |
| `vergleich` über zwei Läufe derselben Maschine | Urteil `KEIN NACHWEIS (eine Maschine)`, Exit-Code 1, die Verweigerung greift |

Der Shard-Lauf erfüllt damit das Akzeptanzkriterium aus
COMPUTE_PIPELINE Phase 1: erstmals über einen aufrufbaren Befehl statt
über einen Integrationstest.

## Changelog

### v0.8.0 – 2026-08-22 (drei Funde am Messgerät)

Diese Fassung baut fast nichts Neues. Sie behebt drei Stellen, an denen
das Werkzeug einen Nachweis geliefert hätte, den es nicht gab. Anlass war
die Vorbereitung des ersten Laufs auf einer fremden Maschine: Der Lauf
findet einmal statt, und was er misst, entscheidet sich vorher.

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
- **`EINSEITER.md`**: eine Seite zum Verschicken an einen Partner. Die
  `ANLEITUNG.md` hat 869 Zeilen und ist dafür zu lang.
- **Punkt 2.2 zurückgestellt statt gebaut.** Ein Backend-Vergleich
  innerhalb einer Maschine hat auf x86_64 bis zum AVX2-Pfad keinen
  Gegenstand (Fund 34). Ihn trotzdem zu bauen hieße, ein Werkzeug zu
  liefern, das auf der Zielmaschine zweimal dasselbe misst.

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

- **`vergleich`** (neuer Befehl, Fahrplanpunkt 2.1): liest alle `.jsonl`
  eines Ordners, gruppiert nach Prüflauf und Einstellungs-Kennung, stellt
  die Vergleichswerte gegenüber und fällt ein Urteil. Damit endet der
  Client nicht mehr bei der Messung; das Auswerten war bis hierher
  Handarbeit mit `grep`, und die Anleitung führte vier Kommandozeilen
  dafür auf.
- **Der Nachweis wird verweigert, wenn alle Protokolle denselben
  Hardware-Fingerabdruck tragen.** Akzeptanzkriterium des Fahrplans, mit
  Test festgehalten.
- **Modellstand im Protokoll** (Fahrplanpunkt 3.1): θ_v, Gewichts-,
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
- **Artefakte und Gewichte freigeben** (Entwicklerpunkt [9]). Getrennt,
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
  Beispielplan: `qwen2.5-0.5b-standard.plan`.
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
