# testclient (`myl-testclient`)

> **Version:** 0.6.0
> **Datum:** 2026-08-21
> **Status:** Phase 1 vollständig, dazu Fahrplanpunkt 2.1 (`vergleich`)
> und 3.1 (Modellstand im Protokoll). 101 Tests grün, alle Läufe gegen die
> echten Artefakte verifiziert.

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
Koordinators — von wem stammt diese Datei.

Dieselben Angaben stehen **auch im Protokoll**: `run_started` trägt
Befehl, Teilnehmer und Einstellungs-Kennung. Der Dateiname ist eine
Bequemlichkeit; die Zuordnung leisten die Daten, denn eine Datei wird
umbenannt, ein Feld nicht.

**Ein Protokoll je Testlauf, nicht eines je Stufe.** Hardware,
Determinismus, geshardete Inferenz und Protokoll-Durchlauf sind eine
Messung. Vier Dateien wären vier Teilaussagen, die der Koordinator wieder
zusammensetzen müsste — und beim Verschicken geht die eine verloren, die
den Befund trägt.

## `vergleich` — vom Protokoll zum Urteil

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
| `UNVERGLEICHBAR (Modellstand)` | θ_v oder Ankerdigest weichen ab — **kein** Hardware-Befund |
| `ABWEICHUNG` | Gleicher Modellstand, gleiche Eingabe, verschiedene Ergebnisse |
| `ZU WENIG PROTOKOLLE` | Weniger als zwei mit derselben Kennung |

**Der Befehl verweigert den Nachweis, wenn alle Protokolle denselben
Hardware-Fingerabdruck tragen.** Das ist ein Akzeptanzkriterium des
Fahrplans und keine Höflichkeit: Ein Werkzeug, das zwei gleiche Werte von
derselben Maschine als Nachweis ausgibt, wäre schlimmer als keines, weil
sein Ergebnis geglaubt wird.

**Der Modellstand wird vor den Digests geprüft.** Bei verschiedenen
Modellen *müssen* die Werte verschieden sein; das als Determinismusfehler
zu melden wäre genau die Verwechslung, gegen die es `artefakte` gibt.

Exit-Code 0 nur dann, wenn jede Gruppe den Nachweis trägt — damit taugt
der Befehl für die CI.

**Zwei Ordner, und die Trennung ist der Punkt:**

```text
TESTCLIENT/Vergleiche/            Eingabe: die zugesandten .jsonl
TESTCLIENT/Vergleiche/Berichte/   Ausgabe: vergleich_<datum>_<uhrzeit>.md
```

Der Vergleich liest **alles**, was er an `.jsonl` findet. Läge er über dem
eigenen Protokollverzeichnis, mischten sich die zugesandten Läufe mit den
eigenen — und ein Urteil über eine Gruppe, in der die eigene Maschine
mehrfach steckt, sagt etwas anderes aus, als es zu sagen scheint. Der
Bericht landet aus demselben Grund eine Ebene tiefer: neben seiner
Eingabe würde ihn der nächste Aufruf mitlesen.

Der Bericht trägt, was auf dem Bildschirm keinen Platz hat —
**vollständige** Digests statt der Kurzform, Dateinamen, Artefakt-Digest
je Teilnehmer, Zeitpunkt. Er ist die Fassung, die weitergereicht wird.
Ein **Laufprotokoll** schreibt `vergleich` dagegen nicht: Er misst nichts,
er wertet aus.

## Testplan — die Datei, die der Koordinator verteilt

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

Zwei Pläne liegen bei: `wikitext2-0.5b-standard.plan` (6 Prompts) und
`qwen2.5-7b-standard.plan` (4 Prompts, rund fünf Minuten Laufzeit).

Die Datei trägt eine Prüfsumme über Prompts, Token, Shards und Modell.
Wird sie verändert, **verweigert der Client den Lauf** (Exit-Code 3)
statt einen abweichenden Digest zu liefern, der wie ein Befund
aussieht. Der Prompt steht in Anführungszeichen, damit auch ein
Randleerzeichen erhalten bleibt.

Kommentarzeilen dürfen frei ergänzt werden — sie gehen nicht in die
Prüfsumme ein. `plan_id` ebenfalls nicht: Zwei Koordinatoren mit
demselben Test unter verschiedenen Namen sollen vergleichbare
Ergebnisse bekommen.

Im Menü: Nutzerpunkt [2] wählt einen Plan, Entwicklerpunkt [9] [6] erzeugt einen.

**Prompttexte werden gehasht, nicht gespeichert.** Testprotokolle wandern
per Copy-Paste in Tickets und Chats; ein Prompt, der dabei mitwandert,
ist eine Datenschutzlücke, die niemand beabsichtigt hat.

**Der Fingerabdruck beschreibt eine Hardware-Klasse, kein Gerät.** Keine
Seriennummern, keine MAC-Adressen, keine Hostnamen — abgesichert durch
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
`myl-testclient/`, den Protokollen unter `myl-testclient/logs/` und den
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
alle. Fremd-Crates: `sha2`, `borsh` und seit v0.6.0 `crossterm` für die
Pfeiltastenauswahl. Sonst nichts — der Client soll auf einer fremden
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
    │   ├── auswahl.rs        Pfeiltastenauswahl mit zeilenweisem Rückfall
    │   ├── menu.rs           interaktives Menü
    │   ├── artefakte.rs      finden, prüfen, beschaffen, freigeben
    │   ├── plaene.rs         Testpläne im Planordner finden
    │   ├── runs.rs           Hardware, Determinismus, Shards
    │   ├── spec.rs           Testplan (erzeugen, prüfen, laden)
    │   ├── vergleich.rs      Protokolle gegenüberstellen, urteilen, berichten
    │   └── stack.rs          Protokoll-Durchlauf (10 Stufen)
    └── logs/                 Laufprotokolle (gitignored)
```

## Belegte Läufe (2026-08-21, aarch64/macos/reference, θ_v 0.17.0)

| Lauf | Ergebnis |
|---|---|
| `determinismus --plan wikitext2-0.5b-standard` | 6 Prompts × 32 Token, je zwei Läufe **bitgleich**, Gesamtwert `fd64588fd46a7af8…`, 29 s |
| `shard --shards 4 --steps 4` | Pod (Layer 0–6/6–12/12–18/18–24) **bitgleich** zur Einzelknoten-Runtime, Digest `6541c129…` |
| `stack` | 10 von 10 Stufen bestanden in 54 ms, Gesamtwert `a9af743f…` |
| `vergleich` über zwei Läufe derselben Maschine | Urteil `KEIN NACHWEIS (eine Maschine)`, Exit-Code 1 — die Verweigerung greift |

Der Shard-Lauf erfüllt damit das Akzeptanzkriterium aus
COMPUTE_PIPELINE Phase 1 — erstmals über einen aufrufbaren Befehl statt
über einen Integrationstest.

## Changelog

### v0.6.0 – 2026-08-21 (vom Protokoll zum Urteil)

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
  dazwischen — sonst wären es zwei Bilder nacheinander statt eines
  Vorgangs. Ein Tastendruck bricht ab, `MYL_NO_ANIMATION=1` schaltet sie
  ab, ohne Terminal läuft sie gar nicht.
- **Netzmotiv nach dem Vorbild des Projektbanners** überarbeitet: Knoten
  verschiedener Größe (`◉ ● ○ ∘ ·`), Naben mit acht abgehenden Kanten,
  lange Kanten quer durchs Feld. Die alte Fassung war ein regelmäßiger
  Zickzack und las sich als Ornament, nicht als Netz.
- **Fund am Namen:** Die Säuberung für den Dateinamen lief auch über den
  Namen im Protokoll — aus „Björn" wurde „bj-rn", auch im Bericht des
  Koordinators. Jetzt trägt das Protokoll den eingegebenen Namen, und nur
  der Dateiname wird umgeschrieben; Umlaute werden dabei umschrieben
  (`Bjoern`), nicht getilgt.
- **Artefakte und Gewichte freigeben** (Entwicklerpunkt [9]). Getrennt,
  weil Artefakte in Sekunden aus dem Skalenpaket entstehen und die
  Gewichte einen Download über Gigabyte kosten. Der Löschpfad ist auf
  direkte Unterverzeichnisse von `INTEGER_LLM/{artifacts,models}`
  eingegrenzt und verlangt ein getipptes „ja" — Enter allein genügt an
  der einen Stelle absichtlich nicht, die etwas zerstört.
- **Fund beim Bauen:** Zwei Läufe in derselben Sekunde bekamen denselben
  Dateinamen, und der zweite überschrieb den ersten **stillschweigend**.
  Im Menü tritt der Fall regelmäßig auf. Der Name weicht jetzt auf einen
  Zähler aus.
- **Zweiter Fund:** Die Menüschleife hielt `stdin.lock()`, während die
  neue Auswahl im Rückfallweg `io::stdin().read_line()` aufruft — das
  wäre derselbe Stillstand gewesen wie in v0.4.0 bei der
  Artefaktbeschaffung. Alle Eingaben laufen jetzt über eine Stelle.
- **Windows geprüft**, soweit ohne Windows-Maschine möglich: `auswahl`,
  `animation`, `banner` und `vergleich` übersetzen für
  `x86_64-pc-windows-msvc`; die Press/Release-Verdopplung der
  Windows-Konsole ist abgefangen. Ein Lauf auf echter Hardware steht aus.
- 53 → 101 Tests.


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
  Beispielplan: `wikitext2-0.5b-standard.plan`.
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
  beim Einlesen verschwindet — es ist aber Teil des Prompts und
  verändert den Digest. Ein Test deckt Randleerzeichen, `=`,
  Anführungszeichen, Backslash und Zeilenumbrüche ab.
- **Protokoll-Ablage** nach `logs/<befehl>/<datum>_<einstellungs-id>/ (bis v0.4.0; seit v0.5.0 eine gemeinsame Datei)`
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
