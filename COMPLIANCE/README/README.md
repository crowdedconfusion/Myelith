# compliance

> **Version:** 0.6.4
> **Datum:** 2026-10-23
> **Status:** Die Pflichten aus der KI-Verordnung sind dokumentiert und,
> soweit technisch, umgesetzt; die Einordnung Artikel für Artikel steht in
> der [Selbsteinschätzung](../de/Selbsteinschaetzung.md). Offen: ein
> C2PA-Manifest, die maschinenlesbare Kennzeichnung geschriebener Texte,
> die Lizenz der Schrift der Wortmarke und eine anwaltliche Prüfung.

Alles, was Myelith gegenüber dem Recht zusagt und nachweist. Die
öffentliche Einstiegsseite ist [`../README.md`](../README.md); hier steht,
wie die Komponente gebaut ist und was sich an ihr geändert hat.

## Struktur

```
COMPLIANCE/
├── README.md                 Einstieg, zweisprachig
├── de/                       die Dokumente auf Deutsch (maßgeblich)
├── en/                       dieselben auf Englisch
├── NOTICES.md                erzeugt: Fremdkomponenten und Lizenzen
├── fremdkomponenten.json     gepflegt: was kein Paketverwalter kennt
├── werkzeuge/notices.py      erzeugt und prüft NOTICES.md
└── ethics/                   Manifest, Ausschlusskatalog, Risikoklassen,
                              Lizenzlage, Modellkarte (eigene Komponente)
```

⚑ **Erzeugt und gepflegt getrennt.** `NOTICES.md` entsteht aus
`cargo metadata`, dem Modellkatalog und `fremdkomponenten.json`; die CI
prüft sie mit `--pruefe`. Wer eine Abhängigkeit hinzufügt, schreibt sie
neu.

⚑ **Wo die Technik steht.** Die Dokumente beschreiben, der Code setzt um:
Kennzeichnung als KI (`CLIENT/myl-client/src/kennzeichnung.rs`),
Kennzeichnung der Stimme (`CLIENT/myl-senses/src/kennzeichnung.rs`),
Schutzfilter (`CLIENT/myl-client/src/schutzfilter.rs`), Notaus
(`AGENT_LAYER/local-agent/src/notaus.rs`).

## Changelog

### v0.6.4 (2026-10-23): Dependabot für die festgenagelten Actions

- `.github/dependabot.yml` neu: nur `github-actions`, für die Wurzel und
  `.github/actions/einrichten`, wöchentlich am Montag, alles in **einem**
  gebündelten Vorschlag, höchstens einer offen. `cooldown` von sieben
  Tagen: Eine Fassung wird erst vorgeschlagen, wenn sie eine Woche alt ist.
- ⚑ **Nicht `cargo` und nicht `pip`:** Die Kisten bauen offline aus dem
  Vorrat; ein Lockfile, das ohne neuen Vorrat wandert, baut dort nicht
  mehr. Diese Updates bleiben bei cargo-deny und dem Vorrat.
- ⚑ **Zusammengeführt wird lokal**, nicht über die Oberfläche: Dort
  unterschriebe GitHub den Merge, und eine künftige Prüfung der
  Unterschrift beim Aktualisieren sähe einen fremden Schlüssel.

### v0.6.3 (2026-10-23): Keine Ausnahme mehr in `deny.toml`; die CI mit engen Rechten und festgenagelten Actions (Funde 599, 603)

Aus dem Sicherheitsaudit vom 2026-10-23.

- **Fund 599:** Mit frisch geholter Datenbank war RUSTSEC-2026-0385
  (`ctutils` 0.4.2) in sieben Kisten offen; auf 0.4.5. `chacha20` 0.10.1,
  die einzige Ausnahme in `deny.toml`, hat mit 0.10.2 einen nicht
  zurückgezogenen Nachfolger; angehoben, die Ausnahme gestrichen
  (`ignore = []`). Danach sind alle ausgelieferten Kisten grün. Die beiden
  Fuzz-Kisten sind nur an der Lizenz NCSA von `libfuzzer-sys` rot; sie
  werden nie ausgeliefert, und die CI-Schleife erfasst sie absichtlich
  nicht. Vorrat (778 Archive) und `NOTICES.md` nachgezogen.
- ⚑ **Fund 603, CI und Release:**
  1. `ci.yml` nannte keine Rechte für das Token; jetzt
     `permissions: contents: read`.
  2. `release.yml` gab jedem Job `contents: write`, auch den Bau-Jobs, die
     fremde Actions und die Tauri-Kommandozeile ausführen. Jetzt lesend für
     alle, schreibend nur für `veroeffentlichen`; `tauri-cli` fest auf
     `=2.12.4` statt `^2`.
  3. Alle 29 fremden Actions hingen an verschiebbaren Marken (`@v4`,
     `@stable`), das Muster von CVE-2025-30066. Jetzt an Commits, mit der
     Fassung im Kommentar.
  Dazu `github.event.before` über eine Umgebungsvariable statt im Shelltext.
- ⚠️ **Gelernt beim Nachfahren:** `--config "$PWD/deny.toml"` nach einem
  `cd` in die Kiste zeigt ins Leere, und cargo-deny läuft dann still mit
  seinen Vorgaben (14 Kisten rot mit Meldungen, die das Projekt absichtlich
  ausnimmt). Den Pfad zur Wurzel vor dem `cd` festhalten.

### v0.6.2 (2026-10-23): Lizenzliste nach dem Sprung auf libp2p 0.57

- `NOTICES.md` neu erzeugt: libp2p 0.57.1 und seine Teile in NETWORKING,
  NODE und TESTCLIENT (Anlass RUSTSEC-2026-0118 und -0119 in
  `hickory-proto`, jetzt 0.26.3). Neu sind `aws-lc-rs` und `aws-lc-sys`,
  die QUIC jetzt fest vorschreibt, mit zwei neuen Lizenzkombinationen aus
  ISC, Apache-2.0, MIT und BSD-3-Clause; alle durch `deny.toml` erlaubt.
- **Belege:** `notices.py --pruefe` PASSED, cargo-deny `licenses ok` für
  jede Kiste.

### v0.6.1 (2026-10-22): Lizenzliste nach der Sicherheitsmeldung zu `rustls-webpki`

- `NOTICES.md` neu erzeugt: `rustls-webpki` 0.103.16 statt 0.103.14 und
  0.103.15 (RUSTSEC-2026-0382, in NETWORKING, NODE und TESTCLIENT
  angehoben). Lizenz unverändert.
- **Belege:** `notices.py --pruefe` PASSED.

### v0.6.0 – 2026-10-21 (die Modelle heißen nach Tieren)

Die Projektmodelle heißen seit heute nach Tieren, mit Größe und Speicherform als Anhängsel (Festlegung des Projektinhabers): `myelith-0.6b` ist `sparrow-0.6b-int8`, `myelith-4b` `pigeon-4b-int8`, `myelith-30b-a3b` `magpie-30b-a3b-int8`, `myelith-35b-a3b` `raven-35b-a3b-int8`. In den Dateien eines Artefakts steht kein Name, also bleiben Abdrücke und Register gültig.

- Modellkarte und Lizenzliste neu erzeugt, mit den neuen Namen.
- `ethics/werkzeuge/modellkarte.py` findet eine Messung von vor dem
  2026-10-21 unter dem früheren Namen (`frueher` im Katalog); die
  Messdateien behalten ihn als Aufzeichnung.
- GPAI-Dokumentation und Trainingsdaten (de, en) nennen die neuen Namen.

### v0.5.1 – 2026-10-19 (die Lizenzprobe kennt private Modelle; Fund 575)

- ⛔️ **Fund 575: Die CI zu `81a3fb0` war rot** im Schritt „COMPLIANCE/ethics:
  Lizenz, Modellkarte, Ausschlusskatalog, Aufnahmeantrag“. `lizenzprobe.py`
  maß jede Lizenzdatei unter `MODELS/llm` an Kap. 10.1 und kannte keine
  privaten Modelle; die Lizenz von Qwen3.8-Flash-Next (Qwen Community
  License, Namensnennung ab 100 Mio. monatlichen Nutzern) galt ihr als
  „unbrauchbar“. Für ein Modell im Netz stimmt das, für ein privates nicht.
  📌 Dieselbe Lehre wie Fund 557: Die CI prüft mehr als die sieben Proben
  der Kurzanleitung, und diese Probe gehört nicht dazu.
- **Jetzt:** Die Probe liest `KATALOG.json`. Ein Eintrag mit `betrieb: lokal`
  wird gelesen und mit seinen Sperren genannt, aber nicht an Kap. 10.1
  gemessen. Ein Ordner ohne Katalogeintrag wird geprüft, wenn er eine
  Lizenzdatei hat (die Ternary-Bonsai-Quellen wie bisher); ohne sie ist er
  ein Hinweis (die eben erkundete Quelle eines selbst gebauten Modells). Ein
  Katalogeintrag ohne Lizenzdatei bleibt ein Fehler. Der Abschlusssatz nennt,
  dass private Modelle nicht gemessen sind.
- **Belege:** der ganze Schritt der CI lokal mit Rückgabe 0; dieselbe Probe
  auf einem Baum nur aus versionierten Dateien grün; Gegenprobe: ohne
  `betrieb: lokal` fällt Flash-Next wie vorher durch.

### v0.5.0 – 2026-10-19 (Modellkarte mit einer Rubrik für private lokale Modelle)

> 📌 *Die Überschrift dieses Eintrags fehlte bis zur Übergabe vom
> 2026-10-21; beim Einfügen von v0.5.1 blieb nur ihr Klammerteil stehen
> (Fund 576).*

- **Die erzeugte Modellkarte** (`ethics/Modellkarte.md`) hat den Abschnitt
  „Private lokale Modelle (nicht für den Netzbetrieb zugelassen)“: Er führt
  jeden Eintrag aus `MODELS/llm/KATALOG.json` mit `betrieb: lokal` und sagt,
  wer die Sperre durchsetzt (Knoten und Klient) und dass selbst gebaute
  Modelle im lokalen Katalog stehen und immer privat sind. Die Messtabelle
  der Netzmodelle lässt private Modelle aus. Der Kopf nennt den Katalog als
  dritte Quelle.
- **`NOTICES.md`** neu erzeugt: `myelith-125b-a6b` mit
  `qwen-community-1.0` und der Artefaktangabe „privat, nur lokal“.
- **Beleg:** `modellkarte.py --pruefe` und `notices.py --pruefe` bestanden.

### v0.4.2 – 2026-10-17 (Selbsteinschätzung, Zweckbestimmung und Manifest ohne Aktionsprotokoll)

- Das Aktionsprotokoll entfällt (AGENT_LAYER v0.49.0). Nach dem EU AI Act
  ist eine Protokollierung nur für Hochrisiko-Systeme vorgeschrieben
  (Art. 12, 19, 26 Abs. 6); Myelith ist keines, die Zweckbestimmung schließt
  diese Zwecke aus. Selbsteinschätzung (deutsch und englisch), Zweckbestimmung
  und Intended Use Policy, Manifest und dieses README beschreiben den Stand
  ohne es: Art. 12 steht nicht mehr als freiwillige Maßnahme da, die Fassung
  des Systemprompts nennt der Debug-Log, wenn er läuft, und der Datenschutz
  sagt, dass der Debug-Log aus ist, bis ihn jemand einschaltet.
- Der Ort des Notaus berichtigt: `AGENT_LAYER/local-agent/src/notaus.rs`.

### v0.4.1 – 2026-10-17 (die Lizenzliste kennt das 35B in vier Bit; das Aktionsprotokoll steht nicht mehr im Fenster; Fund 557)

- ⛔️ **Fund 557: `NOTICES.md` fehlte ein Modell, und die CI fiel daran.**
  Am 2026-10-17 kam `myelith-35b-a3b-int4` in den Katalog der Sprachmodelle
  (INTEGER_LLM v0.135.0), und die Liste wurde nicht neu erzeugt. Der
  Schritt „NOTICES entspricht den Quellen“ im Job der Abhängigkeitsprüfung
  war damit rot, obwohl cargo-deny selbst grün war. Neu erzeugt; jetzt mit
  der Zeile für das int4-35B (Apache-2.0 für die Grundgewichte, PolyForm
  Shield für das Artefakt). 📌 **Wer den Katalog ändert, erzeugt die Liste
  neu**, nicht nur wer eine Abhängigkeit anhebt: `notices.py` liest beides.
- **Selbsteinschätzung, Art. 12 (freiwillig), deutsch und englisch:** Das
  Fenster zeigt das Aktionsprotokoll seit CLIENT v0.167.0 nicht mehr auf
  der Einstellungsseite an. Geschrieben wird es weiter, wie bisher ohne
  Klartext und 30 Tage lang; lesbar mit `myl protokoll`. Der Satz nannte
  noch die Anzeige im Fenster.
- Belege: `notices.py --pruefe` PASSED; Gegenprobe mit der gepushten
  Fassung FEHLGESCHLAGEN, wie in der CI.

### v0.4.0 – 2026-10-01 (der Systemprompt, gestrafft und am 4B gemessen besser)

**Festlegung des Projektinhabers:** den Systemprompt straffer fassen
(Kontext), dann die schwächeren Punkte mit mehreren Saaten prüfen, und zwar
am 4B, damit der Vergleich stimmt.

- **`de.md` 703 → 566 Wörter, `en.md` 743 → 604**, Inhalt gleich. Drei
  Stellen über das Kürzen hinaus:
  - Punkt 3 nennt „Psyche“ statt „seelische Leiden“; Punkt 4 regelt den
    Umgang mit Leid, Punkt 3 die Empfehlung im Einzelfall.
  - Punkt 4: „Leidet der Nutzer **selbst** seelisch“ statt der Klammer
    „(und fragt nicht nur aus Wissensgründen)“; in einer Krise nennt das
    Modell die Nummern **zuerst**, deutsch mit „(Deutschland, rund um die
    Uhr)“. Anlass: Das 4B erfand „in der Schweiz“ und ließ die 112 weg.
  - „nur mit seiner Bestätigung“ wird „nur mit Bestätigung des Nutzers“;
    im kürzeren Satz fehlte sonst der Bezug.
- **Gemessen am 4B**, 13 Prüffragen, Saaten 1 bis 5, beide Fassungen mit
  denselben Saaten (`BENCHMARKS/Agent/results/verhalten-2026-09-30/saaten/`):
  Krise mit 112 in 5 von 5 (vorher 3), Telefonseelsorge 4 von 5 (vorher 3),
  seelisches Leid mit Verweis an Fachleute 4 von 5 (vorher 0), Geldanlage
  mit Hinweis 3 von 5 (vorher 1). Ablehnungen (20 von 20) und Wissensfragen
  (25 von 25) unverändert; jede `PRUEFEN`-Zeile am Wortlaut gelesen.
- Prüfsummen neu (`de.md` 181f4776…, `en.md` cf34eca9…), wortgleich mit dem
  gemessenen Bau. Die Probe `beide_fassungen_tragen_die_regeln` hält die neue
  Wendung fest.

**Ältere Einträge** (v0.3.2 und davor, 8 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- COMPLIANCE/README/README.md`.
