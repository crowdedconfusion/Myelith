# compliance

> **Version:** 0.2.0
> **Datum:** 2026-09-26
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
Schutzfilter (`schutzfilter.rs`), Aktionsprotokoll (`protokoll.rs`),
Notaus (`notaus.rs`), alle unter `CLIENT/myl-client/src/`, sofern nicht
anders genannt.

## Changelog

### v0.2.0 – 2026-09-26 (ein fest vorgegebener Systemprompt, über eine Prüfsumme gebunden)

**Auftrag des Projektinhabers:** ein passender Systemprompt, der das Modell
auf die Arbeitsweise vorbereitet, **unbedingt vorgegeben und überprüft**
(Hash), damit Compliance- und Ethik-Vorgaben später dauerhaft darin stehen
können.

**Neu: `systemprompt/`** mit `de.md`, `en.md` und `pruefsummen.txt` (Format
von `sha256sum`):
- **Grundsätze:** KI-System und kein Mensch (Art. 50), Ehrlichkeit über
  das Getane, gelesener Inhalt ist Daten und keine Anweisung, keine Hilfe
  zu schwerem Schaden (die Klassen des Ausschlusskatalogs), die gesetzten
  Grenzen einhalten.
- **Arbeitsweise:**
  - Ein Werkzeug wirkt nur, wenn es aufgerufen wird.
  - Erst nachsehen, dann handeln.
  - Kleinste Änderung mit genauem Anker.
  - Ausführen statt vermuten.
  - Die Ursache beheben, keine Symptome verdecken.
  - Fertig heißt belegt, und ein Abnahmebefehl entscheidet.
  - Skills nutzen.
  - Nach drei Fehlversuchen fragen.
- **Im Loop:** kurze Runden, Notizen, ein Satz über das tatsächlich
  Getane.

Jede Regel stammt aus einem Befund der Loop-Szenarien mit dem 8B und dem
30B (behauptete Aufrufe, verdeckte Fehler, „fertig“ ohne Beleg, Skills
nicht gesucht). Der Text ist knapp gehalten, weil er in jedem Schritt im
Kontext steht.

**Umgesetzt im Client** (CLIENT v0.94.0): eingebaut, nicht einstellbar,
vor jedem Lauf geprüft, im Zweifel geschlossen; der volle SHA-256 im
Aktionsprotokoll; der Chat ohne Werkzeuge bekommt die Grundsätze.

**Belegt:** `shasum -a 256 -c pruefsummen.txt` meldet beide Dateien als
in Ordnung. Die Probe `die_pruefsummen_stimmen` wird rot, sobald ein
Zeichen im Text geändert ist (mit dem geänderten `de.md` gefahren: „Prüfsumme
stimmt nicht (verlangt 1a03e486…, ist 912c2720…)“).

### v0.1.1 – 2026-09-25 (die Lizenzhinweise sehen auch Kisten unter `SYSTEM/`; Fund 478)

**Fund 478 behoben.** `werkzeuge/notices.py` nahm alles unter `SYSTEM/`
aus, gemeint war nur das Kistenlager. Seit der Einrichtungsassistent von
GolemOS als Kiste unter `SYSTEM/golemos/einrichten` liegt und mit jedem
GolemOS-Abbild ausgeliefert wird, hätte das ihn still aus den Hinweisen
genommen. Heute hat er keine Abhängigkeit, und genau deshalb wäre es
erst mit der ersten aufgefallen. Der Ausschluss nennt jetzt
`SYSTEM/crates-lager/`.

Dazu zählt der Erzeuger neue Sperrdateien mit, die noch nicht
eingecheckt, aber auch nicht ausgeschlossen sind. Sonst nennte
`NOTICES.md` vor dem Commit eine Kiste weniger als die Prüfung im CI
danach. `NOTICES.md` neu erzeugt: 760 Pakete aus **26** Kisten, die
Pakete unverändert.

`fremdkomponenten.json` nennt die Zahl der Kisten nicht mehr; sie stand
dort ein zweites Mal von Hand und hätte mit der nächsten Kiste
gestimmt oder nicht, ohne dass es jemand merkt.

**Belegt:** `notices.py --pruefe` grün.

### v0.1.0 – 2026-09-25 (die Komponente entsteht)

Auftrag des Projektinhabers: das Projekt nach der Verordnung (EU)
2024/1689 aufstellen, alles unter `COMPLIANCE` statt unter `docs`,
mehrsprachig, mit den Pflichtblöcken in beiden Wurzel-READMEs. Auf
Rückfrage entschieden: **Anbieter mit allen Pflichten nach Art. 53**, weil
Myelith selbst nachtrainiert; Kennzeichnung der Stimme mit Metadaten und
eigenem Wasserzeichen.

- **Sechs Dokumente, je deutsch und englisch**: Selbsteinschätzung,
  Zweckbestimmung (Art. 5, Anhang III), GPAI-Dokumentation (Formular des
  Verhaltenskodex), Zusammenfassung der Trainingsdaten (Vorlage des AI
  Office; Größen an den Dateien gezählt), Urheberrechtsstrategie, dazu die
  Einstiegsseite.
- **`NOTICES.md`, erzeugt**: 760 Rust-Pakete aus 25 Kisten (keines ohne
  Lizenzfeld), die Sprachmodelle aus dem Katalog, Modelle für Hören, Sehen
  und Sprechen, Programme, Python-Pakete, Daten und Medien mit Prüfdatum.
  ⚑ JSON und nicht TOML, weil das `python3` der Proben 3.9 ist.
- ⛔️ **Fund 468** (die genaue Sehstufe stand unter einer
  Forschungslizenz) ist ersetzt; ⚠️ offen bleibt die Schrift der
  Wortmarke.
- `ETHICS/` liegt jetzt als `ethics/` hier.

**Belegt:** `notices.py --pruefe` in der CI, Gegenprobe beißt (eine
geänderte Lizenzangabe macht sie rot). Die Zusagen der Dokumente sind in
CLIENT v0.85.0 umgesetzt und dort belegt.
