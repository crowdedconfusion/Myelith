# compliance

> **Version:** 0.1.0
> **Datum:** 2026-09-25
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
