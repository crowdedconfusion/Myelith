# testclient (`myl-testclient`)

> **Version:** 0.2.0
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

Die Lauf-Kennung ist `<unix-sekunden>-<befehl>`: sortierbar und ohne
Rückfrage einem Befehl zuzuordnen.

**Prompttexte werden gehasht, nicht gespeichert.** Testprotokolle wandern
per Copy-Paste in Tickets und Chats; ein Prompt, der dabei mitwandert,
ist eine Datenschutzlücke, die niemand beabsichtigt hat.

**Der Fingerabdruck beschreibt eine Hardware-Klasse, kein Gerät.** Keine
Seriennummern, keine MAC-Adressen, keine Hostnamen — abgesichert durch
einen Test, der Feldnamen und Wertformate prüft.

## Aufruf

**Interaktiv — ohne Unterbefehl öffnet sich ein Menü:**

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
