# testclient (`myl-testclient`)

> **Version:** 0.3.0
> **Datum:** 2026-08-18
> **Status:** Phase 1 vollständig; dazu Protokoll-Durchlauf über alle
> Komponenten, interaktives Menü und Banner (32 Tests grün, alle Läufe
> gegen die echten Artefakte verifiziert).

Terminal-Testclient: Hardwaretests auf heterogener Hardware und
geshardete Inferenz — jeder Lauf mit einem Protokoll, das zwischen
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
   kann — und der gegen die Einzelknoten-Runtime gegenprüft.

## Der Kern: das Protokoll

Ein Testlauf ohne Protokoll ist wertlos. Er beantwortet dann nicht, was
bei einem abweichenden Ergebnis als Erstes zu klären ist: **auf welcher
Hardware, mit welchem Backend, gegen welchen Modellstand?** Genau diese
drei Angaben entscheiden bei einem Modellwechsel darüber, ob ein
verändertes Ergebnis Fortschritt oder Fehler ist.

Jeder Lauf schreibt deshalb **immer** zwei Dateien nach `logs/`:

| Datei | Zweck |
|---|---|
| `<lauf-id>.jsonl` | Eine JSON-Zeile je Ereignis, stabile Feldnamen und Reihenfolge — die Fassung, die zwischen Maschinen gediffed wird |
| `<lauf-id>.log` | Dieselben Ereignisse als Fließtext, für die Fehlersuche am Terminal |

**Sortiert nach Prüflauf, Datum und Einstellungen:**

```text
logs/
├── determinismus/
│   └── 2026-08-18_94be3bfc/       ← Datum + Kurzkennung der Einstellungen
│       ├── 081222-aarch64-macos-reference.jsonl
│       └── 143515-x86-64-linux-avx2.jsonl
└── stack/
    └── 2026-08-18_94be3bfc/
```

Die Kurzkennung ist der Hash genau der Parameter, die gleich sein
müssen (Prompt, Token, Shards, Modell). **Alle Teilnehmer eines
Testplans landen im gleichnamigen Ordner** — auf jeder Maschine. Wer
versehentlich andere Parameter nimmt, landet sichtbar woanders; die
Zuordnungsarbeit beim Auswerten entfällt. Der Dateiname trägt Uhrzeit
und Hardware-Kurzform, damit sich Protokolle mehrerer Maschinen in
einem Ordner nicht überschreiben.

## Testplan — die Datei, die der Koordinator verteilt

Damit „alle nehmen exakt dieselben Werte" keine Bitte bleibt:

```bash
# Koordinator:
myl-test plan --plan-id 2026-08-18-cross-arch-01 \
  --prompt "Die Hauptstadt von Frankreich ist" --steps 6 --shards 4 \
  --out cross-arch.plan

# Teilnehmer:
myl-test --plan cross-arch.plan determinismus
```

Die Datei trägt eine Prüfsumme über Prompt, Token, Shards und Modell.
Wird sie verändert, **verweigert der Client den Lauf** (Exit-Code 3)
statt einen abweichenden Digest zu liefern, der wie ein Befund
aussieht. Der Prompt steht in Anführungszeichen, damit auch ein
Randleerzeichen erhalten bleibt.

Kommentarzeilen dürfen frei ergänzt werden — sie gehen nicht in die
Prüfsumme ein. `plan_id` ebenfalls nicht: Zwei Koordinatoren mit
demselben Test unter verschiedenen Namen sollen vergleichbare
Ergebnisse bekommen.

Im Menü: Punkt 9 erzeugt, Punkt 8 lädt.

**Prompttexte werden gehasht, nicht gespeichert.** Testprotokolle wandern
per Copy-Paste in Tickets und Chats; ein Prompt, der dabei mitwandert,
ist eine Datenschutzlücke, die niemand beabsichtigt hat.

**Der Fingerabdruck beschreibt eine Hardware-Klasse, kein Gerät.** Keine
Seriennummern, keine MAC-Adressen, keine Hostnamen — abgesichert durch
einen Test, der Feldnamen und Wertformate prüft.

## Aufruf

Im Wurzelverzeichnis des Repositories liegen drei Starter. Sie tun
dasselbe: bauen bei Bedarf, finden das Binary und reichen alle Argumente
weiter. Welchen du nimmst, hängt nur davon ab, womit du arbeitest.

| Datei | Für wen |
|---|---|
| `Myelith Testclient - macOS.app` | macOS, Doppelklick im Finder |
| `Myelith Testclient - Windows (Batch).cmd` | Windows, Doppelklick im Explorer oder Aufruf aus cmd |
| `Myelith Testclient - Linux, macOS (Shell).sh` | Terminal auf Linux und macOS |

Sie liegen im Wurzelverzeichnis, weil sie dort gefunden werden sollen.
Alles Übrige zum Testclient steht in diesem Ordner: der Quelltext unter
`myl-testclient/`, die Protokolle unter `myl-testclient/logs/`, und die
Helfer für Symbol und Anwendungsmenü unter `werkzeuge/`.

### Der kürzeste Weg

**Doppelklick.** Unter macOS auf das App-Bündel, unter Windows auf die
`.cmd`. Es öffnet sich ein Terminalfenster mit dem Menü; jeder Punkt ist
dort in zwei Zeilen erklärt, und es steht dabei, was ein Modell braucht
und was nicht. Beim ersten Start dauert der Bau einige Minuten, danach
wenige Sekunden.

Wer keine Artefakte hat, wird gefragt, ob die Gewichte von Hugging Face
geholt und die Artefakte gebaut werden sollen. Es passiert nichts ohne
Rückfrage.

### Aus dem Terminal

```bash
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
alias myl-test='"/pfad/zum/Repository/Myelith Testclient - Linux, macOS (Shell).sh"'
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

Jeder Lauf schreibt ein Protokoll nach `TESTCLIENT/myl-testclient/logs/`,
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

**Als Befehl — für Skripte und CI:**

```bash
cd TESTCLIENT/myl-testclient

# Hardware erheben — der erste Befehl auf einer neuen Maschine.
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
| `hardware` | — (nur Erhebung) | nein |
| `stack` | myl-types, -scheduler, -consensus, -verifier, -ledger, -tokenomics | nein |
| `determinismus` | INTEGER_LLM (runtime, kernels) | **ja** |
| `shard` | COMPUTE_PIPELINE (myl-pod) + INTEGER_LLM | **ja** |

**Nicht abgedeckt:** `myl-net` (Gossip über echte Sockets gehört in die
NETWORKING-Testsuite) und BFT-Liveness (Rundenwechsel fehlt noch,
CONSENSUS Punkt 3.6). Die vollständige Abgrenzung steht in
[ANLEITUNG.md](ANLEITUNG.md), Abschnitt 5.

## Anleitung für Tests mit mehreren Beteiligten

**[ANLEITUNG.md](ANLEITUNG.md)** — nach Rollen getrennt: Ein Teilnehmer
liest Abschnitt 1 und ist fertig; der Koordinator bekommt die
Urteilstabelle, die Ausschlussfragen bei abweichenden Digests und eine
Meldevorlage. Enthält außerdem, welche Hardware-Kombinationen sich
lohnen und was die Tests **nicht** abdecken.

Kurzfassung auch im Menü unter Punkt 7.

## Cross-Hardware-Nachweis — das Verfahren

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
Netzwerkverbindung. Die Trennung ist bewusst — ein Diagnosewerkzeug darf
laut, gesprächig und roh sein; ein Nutzer-Client nicht.

## Abhängigkeiten

INTEGER_LLM (`runtime`, `kernels`), COMPUTE_PIPELINE (`myl-pod`),
SHARED_TYPES (`myl-types`), CONSENSUS (`myl-ledger`, `-scheduler`,
`-consensus`), TOKENOMICS, VERIFICATION — der `stack`-Lauf braucht sie
alle. Keine Fremd-Crates außer `sha2` und `borsh`; der Client soll auf
einer fremden Maschine mit möglichst wenig Voraussetzungen bauen. Die
Argumentauswertung ist deshalb von Hand, und das Menü kommt ohne
TUI-Bibliothek aus.

## Struktur

```
TESTCLIENT/
├── README/
│   ├── README.md             diese Kurzübersicht
│   ├── ANLEITUNG.md          Tests mit mehreren Beteiligten
│   └── Fahrplan-v1.md        Phasenplan
└── myl-testclient/
    ├── src/
    │   ├── lib.rs            Einstieg, Abgrenzung zu CLIENT
    │   ├── main.rs           Argumentauswertung, Hilfetext
    │   ├── logging.rs        Laufprotokolle (JSONL + Text)
    │   ├── hardware.rs       Fingerabdruck (Klasse, nicht Gerät)
    │   ├── banner.rs         ASCII-Banner zum Projektbanner
    │   ├── menu.rs           interaktives Menü
    │   ├── runs.rs           Hardware, Determinismus, Shards
    │   ├── spec.rs           Testplan (erzeugen, prüfen, laden)
    │   └── stack.rs          Protokoll-Durchlauf (10 Stufen)
    └── logs/                 Laufprotokolle (gitignored)
```

## Belegte Läufe (2026-08-18, aarch64/macos/reference)

| Lauf | Ergebnis |
|---|---|
| `determinismus --steps 6` | bitgleich über zwei Läufe, Digest `977ff1b4…` |
| `shard --shards 4 --steps 4` | Pod (Layer 0–6/6–12/12–18/18–24) **bitgleich** zur Einzelknoten-Runtime, Digest `6541c129…` |
| `stack` | 10 von 10 Stufen bestanden in 54 ms, Gesamtwert `a9af743f…` |

Der Shard-Lauf erfüllt damit das Akzeptanzkriterium aus
COMPUTE_PIPELINE Phase 1 — erstmals über einen aufrufbaren Befehl statt
über einen Integrationstest.

## Changelog

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
  beim Einlesen verschwindet — es ist aber Teil des Prompts und
  verändert den Digest. Ein Test deckt Randleerzeichen, `=`,
  Anführungszeichen, Backslash und Zeilenumbrüche ab.
- **Protokoll-Ablage** nach `logs/<befehl>/<datum>_<einstellungs-id>/`
  mit `<uhrzeit>-<hardware>` als Dateiname. Alle Teilnehmer eines Plans
  landen im gleichnamigen Ordner. Die Einstellungs-Kennung steht auch
  **im** Protokoll, nicht nur im Pfad — Protokolle werden einzeln
  weitergereicht.
- Datum und Uhrzeit in UTC, von Hand aus Unix-Sekunden gerechnet (kein
  Datums-Crate). UTC bewusst: Teilnehmer sitzen in verschiedenen
  Zeitzonen, und ein Ordner je Zeitzone wäre genau die
  Zuordnungsarbeit, die vermieden werden soll.
- Argumentauswertung akzeptiert Optionen **vor** dem Befehl
  (`myl-test --plan x stack`) — beim ersten Praxistest landete genau
  dieser Aufruf im Menü statt im Prüflauf.
- Menüpunkte 8 (Plan laden) und 9 (Plan erzeugen).
- 32 → 50 Tests.

### v0.2.0 – 2026-08-18

**Der Client prüfte nur zwei von neun Crates.** Determinismus (INTEGER_LLM)
und Sharding (COMPUTE_PIPELINE) waren abgedeckt; `myl-types`, `-ledger`,
`-scheduler`, `-consensus`, `-tokenomics` und `-verifier` fasste er nicht
an. Die haben Unit-Tests, aber niemand prüfte, ob sie **zusammen**
funktionieren — und genau dort lagen die schwersten Audit-Funde.

- **Neuer Befehl `stack`**: zehn Stufen von der Kryptografie über
  Epochenseed, Komiteewahl, BFT (mit echten Signaturen und Negativproben),
  Double-Signing, Blockstruktur, Verifikation und Ledger-Buchung bis zur
  Preisbildung. Läuft ohne Artefakte in ~1 s.
- **Fund A20, gefunden vom neuen Stack-Lauf:** `derive_epoch_seed` nahm
  die Epoche als Parameter entgegen, speicherte sie im `EpochSeed` — und
  ließ sie **nicht in den VRF-Eingang einfließen**. Folge: Ein Seed für
  Epoche 42 galt unverändert als gültiger Seed für Epoche 99, mit
  demselben Beweis (empirisch bestätigt). Zusätzlich hätten zwei Epochen
  mit demselben Vorgängerblock exakt dieselbe Zuteilung ergeben. Behoben
  in `myl-scheduler` v0.2.11 durch domain-getrenntes Alpha
  (`MYELITH_EPOCH_SEED_v1 ‖ block ‖ epoch`). **Konsensrelevant.**
- **Interaktives Menü**: `myl-test` ohne Unterbefehl öffnet eine
  Ziffernauswahl mit Erklärung je Punkt, Einstellungen und
  Kurzanleitung. Bewusst ohne TUI-Bibliothek — der Client soll über SSH
  und in einer seriellen Konsole funktionieren.
- **ASCII-Banner** nach dem Projektbanner (Knotennetz, Schriftzug, Zeile
  und die drei Schlagworte). Unterdrückbar über `--quiet` und
  `MYL_NO_BANNER`.
- **[ANLEITUNG.md](ANLEITUNG.md)** für Tests mit mehreren Beteiligten —
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
  einen Client, der von überall gestartet wird, nicht — der erste
  Determinismuslauf fand die Artefakte nicht. Der Client löst jetzt
  absolut auf; `INTEGER_LLM_ARTIFACTS_DIR` behält Vorrang.
