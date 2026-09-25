# Zusammenfassung der Trainingsdaten

**Stand:** 2026-09-25 · **English:** [Training-Data.md](../en/Training-Data.md)

Öffentliche Zusammenfassung der für das Training verwendeten Inhalte
nach Artikel 53 Absatz 1 Buchstabe d der Verordnung (EU) 2024/1689.
Gliederung nach der Vorlage des Büros für Künstliche Intelligenz:
allgemeine Angaben, Datenquellen, Datenverarbeitung.

## 1. Allgemeine Angaben

| Feld | Angabe |
|---|---|
| Anbieter | Joschka Benjamin Hänsler, Privatperson, Deutschland |
| Modelle | `myelith-0.6b`, `myelith-4b`, `myelith-8b`, `myelith-30b-a3b`, `myelith-35b-a3b` |
| Modalitäten der Trainingsdaten | Text |
| Umfang der eigenen Trainingsdaten | rund 127 000 Wörter (synthetisch), dazu 241 000 Wörter nur zur Kalibrierung |
| Zeitraum der Datenerhebung | 2026-08 bis 2026-09 |
| Grundmodelle | Qwen3-Reihe und Qwen3.6-35B-A3B (Alibaba Cloud); ihre Trainingsdaten beschreibt ihr Anbieter |

⚑ **Zwei Ebenen, und diese Zusammenfassung betrifft die zweite.** Die
Grundmodelle wurden von ihrem Anbieter mit einem sehr großen,
mehrsprachigen Korpus vortrainiert; dazu hat Myelith keine eigenen
Kenntnisse über das hinaus, was der Anbieter veröffentlicht. Die
Myelith-Artefakte entstehen daraus durch Umrechnung, Kalibrierung und
eigenes Nachtraining, und nur diese Daten stehen hier.

## 2. Datenquellen

### 2.1 Öffentlich verfügbare Datensätze

| Datensatz | Verwendung | Umfang | Lizenz |
|---|---|---|---|
| WikiText-2 (`wikitext-2-raw-v1`, Testsplit) | **Kalibrierung** der Aktivierungsskalen und Messung der Perplexität; verändert keine Gewichte | rund 241 000 Wörter | CC BY-SA 3.0 oder GFDL |

### 2.2 Lizenzierte, nicht öffentliche Datensätze

Keine.

### 2.3 Aus dem Internet gesammelte Daten

**Keine.** Myelith betreibt keinen eigenen Crawler und sammelt keine
Trainingsdaten aus dem Web. Die Web-Recherche des Assistenten liest
Seiten nur, um eine Frage des Nutzers zu beantworten; was sie liest,
fließt nicht in ein Training ein.

### 2.4 Daten der Nutzer

**Keine, die beim Projekt ankommen.** Myelith läuft auf dem Rechner des
Nutzers. Gespräche, Anhänge und Stimmproben verlassen diesen Rechner
nicht und werden nicht für das Training der veröffentlichten Artefakte
verwendet.

Ein Nutzer kann **seine eigenen** Artefakte mit eigenem Material
nachtrainieren (Buchkette unter `TRAINING/korpus/`). Dieses Training
findet auf seinem Rechner statt; für die Rechte an dem Material ist er
selbst verantwortlich (siehe [Urheberrecht](Urheberrecht.md)).

### 2.5 Synthetische Daten

| Datensatz | Inhalt | Umfang | Erzeuger |
|---|---|---|---|
| Referenzleiter | fünf Stufen mit bekannter Regel (Kopie, Endbuchstabe, Umkehr, Addition zwei- und dreistellig), je 4000 Lern- und 800 Haltezeilen | 24 000 Zeilen, rund 120 000 Wörter | `BENCHMARKS/Training/referenzleiter.py` |
| Wissenssaat | erfundene Personen und ein erfundenes Ereignis mit Prüfungsfragen | rund 700 Zeilen, rund 6 700 Wörter | `BENCHMARKS/Training/wissenssaat.py` |

Beide sind **regelbasiert erzeugt**, ohne ein Sprachmodell und ohne
fremde Texte. Sie dienen der Messung, was ein Trainingsschritt ablegt;
die Wissenssaat ist bewusst erfunden, damit nicht bereits bekanntes
Wissen gemessen wird. Sie werden bei Bedarf neu erzeugt und nicht
versioniert.

### 2.6 Sonstige Quellen

Keine.

## 3. Datenverarbeitung

### 3.1 Wahrung von Nutzungsvorbehalten (Text und Data Mining)

Da Myelith keine Inhalte aus dem Internet für das Training sammelt, gibt
es keine Vorbehalte nach Art. 4 Abs. 3 der Richtlinie (EU) 2019/790 zu
wahren. Sollte das Projekt künftig Web-Inhalte für ein Training
verwenden, gelten die Regeln der [Urheberrechtsstrategie](Urheberrecht.md)
(maschinenlesbare Vorbehalte, `robots.txt`).

### 3.2 Entfernung rechtswidriger Inhalte

Die eigenen Daten sind regelbasiert erzeugt oder stammen aus einem
etablierten öffentlichen Datensatz, der nur kalibriert und nicht
trainiert wird. Für künftige Korpora verlangt das
**Korpus-Aufnahmeverfahren** des Projekts einen Nachweis zu jeder Klasse
des Ausschlusskatalogs
([`../ethics/Ausschluss.json`](../ethics/Ausschluss.json)) sowie Herkunft
und Rechtsgrundlage jedes Bestandteils
([Vorlage](../ethics/vorlagen/korpus-aufnahmeantrag.json)).

### 3.3 Qualitätssicherung

- **Haltemengen** mit disjunkten Beispielen derselben Regel; ein
  Trainingslauf zählt nur gegen sie und gegen einen Rauschnullpunkt.
- **Provenienz**: Korpora für das Netz werden per Merkle-Wurzel verankert
  (`TRAINING/myl-train`), sodass jede Zeile auf ihre Quelle zurückgeführt
  werden kann.

### 3.4 Bekannte Lücken und Verzerrungen

- Die eigenen Daten sind klein und künstlich; sie ändern Wissen und
  Verhalten der Artefakte nicht in der Breite.
- Verzerrungen der Grundmodelle (Sprachen, Kulturräume, Themen) werden
  geerbt und von Myelith nicht ausgeglichen.
- WikiText-2 ist englischsprachig; die Kalibrierung ist deshalb auf
  englischen Text abgestimmt.
