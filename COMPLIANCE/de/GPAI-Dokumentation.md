# Technische Dokumentation der KI-Modelle mit allgemeinem Verwendungszweck

**Stand:** 2026-09-25 · **English:** [GPAI-Documentation.md](../en/GPAI-Documentation.md)

Diese Dokumentation erfüllt Artikel 53 Absatz 1 Buchstaben a und b der
Verordnung (EU) 2024/1689 (KI-Verordnung) in Verbindung mit den
Anhängen XI und XII. Ihr Aufbau folgt dem Formular zur
Modelldokumentation aus dem Verhaltenskodex für KI-Modelle mit
allgemeinem Verwendungszweck (Kapitel Transparenz).

## 1. Allgemeine Angaben

| Feld | Angabe |
|---|---|
| Anbieter | Joschka Benjamin Hänsler, Privatperson, Deutschland, nicht kommerziell |
| Kontakt | über die Issues des Repositoriums |
| Rolle | **Anbieter** der Myelith-Artefakte im Sinne von Art. 3 Nr. 3 und Art. 53. Die Artefakte sind Bearbeitungen fremder Grundmodelle; Myelith trainiert sie selbst nach und behandelt sich deshalb für die eigenen Bearbeitungen als Anbieter mit allen Pflichten nach Art. 53. Die Pflichten der Anbieter der Grundmodelle bleiben davon unberührt. |
| Modelle | `myelith-0.6b`, `myelith-4b`, `myelith-8b`, `myelith-30b-a3b`, `myelith-35b-a3b` |
| Fassung der Ausführungsspezifikation | θ_v 0.22.0 |
| Grundmodelle | Qwen3-0.6B, Qwen3-4B, Qwen3-8B, Qwen3-30B-A3B, Qwen3.6-35B-A3B (Alibaba Cloud), je Apache 2.0; Revision und Prüfung stehen in [`MODELS/llm/KATALOG.json`](../../MODELS/llm/KATALOG.json) |
| Lizenz | Artefakte und Code: PolyForm Shield License 1.0.0; Grundgewichte: Apache 2.0 (siehe [NOTICES](../NOTICES.md)) |
| Systemisches Risiko | nein; die Schwelle von Art. 51 Abs. 2 (10²⁵ Gleitkommaoperationen) wird durch die eigene Bearbeitung um viele Größenordnungen unterschritten |

## 2. Modelleigenschaften

### 2.1 Architektur

Alle Artefakte sind **Transformer-Sprachmodelle** der Qwen3-Familie,
umgerechnet auf **reine Ganzzahlarithmetik**. Die Ausführung ist
bitgenau festgelegt, sodass jeder Rechner aus derselben Eingabe dieselbe
Ausgabe erzeugt; das ist die Voraussetzung für die Prüfung von
Rechenarbeit im Myelith-Netz.

| Artefakt | Bauart | Ebenen | Breite | Köpfe (Q/KV) | Experten (aktiv) | Wortschatz | Größe |
|---|---|---|---|---|---|---|---|
| myelith-0.6b | dicht | 28 | 1024 | 16/8 | keine | 151 936 | 0,9 GB |
| myelith-4b | dicht | 36 | 2560 | 32/8 | keine | 151 936 | 4,5 GB |
| myelith-8b | dicht | 36 | 4096 | 32/8 | keine | 151 936 | 8,8 GB |
| myelith-30b-a3b | Expertengemisch | 48 | 2048 | 32/4 | 128 (8) | 151 936 | 29 GB |
| myelith-35b-a3b | Expertengemisch, hybrid mit rekurrenten Zustandsebenen | 40 | 2048 | 16/2 | 256 (8) | 248 320 | 33 GB |

**Zahlenformate** (Ausführungsspezifikation θ_v 0.22.0): Gewichte `int8`
mit einer Skala je Ausgabekanal, Aktivierungen `int16`, Akkumulator
`int64`. Nichtlineare Funktionen (Softmax, SiLU, RMSNorm, RoPE und
weitere) laufen über Tabellen und ganzzahlige Reihen. Division
ausschließlich als arithmetische Verschiebung mit Rundung zur nächsten
geraden Zahl. Kein Gleitkomma im Rechenweg.

### 2.2 Modalitäten

| | Eingabe | Ausgabe |
|---|---|---|
| Sprachmodelle (diese Dokumentation) | Text | Text |
| System Myelith insgesamt | Text, gesprochene Sprache, Bilder | Text, synthetische Sprache |

Hören, Sehen und Sprechen übernehmen **eigenständige Fremdmodelle**, die
Myelith nicht bearbeitet und nicht trainiert (siehe [NOTICES](../NOTICES.md),
Abschnitt 2). Sie sind nicht Gegenstand dieser Dokumentation; für sie
gelten die Angaben ihrer Anbieter. Myelith erzeugt **keine Bilder und
keine Videos**.

## 3. Vertrieb

- **Quelltext** öffentlich im Repositorium.
- **Artefakte** werden nicht als fertige Datei verteilt, sondern auf dem
  Rechner des Nutzers aus den öffentlich verfügbaren Grundgewichten
  gebaut (Werkzeuge in `INTEGER_LLM/`); das Ergebnis ist bitgleich.
- **Freigabebündel** für macOS, Windows und Linux, jedes mit Prüfsumme.
- **Im Myelith-Netz** rechnen Knoten mit denselben Artefakten.

## 4. Verwendung

Wofür Myelith bestimmt ist und wofür nicht, steht in der
[Zweckbestimmung](Zweckbestimmung.md). Die Sprachmodelle arbeiten in
zwei Arten von KI-Systemen: im **lokalen Assistenten** (Fenster und
Konsole, mit Agentenschleife und Werkzeugen) und in den **Rollen des
Netzes** (Rechnen, Prüfen).

**Hinweise für nachgelagerte Anbieter** (Anhang XII): Wer ein Artefakt in
ein eigenes System einbaut, erhält hier Architektur, Zahlenformat,
Grenzen und Messwerte. Die Werkzeugaufrufe folgen dem ChatML-Format des
Grundmodells. Die Ausgabe ist deterministisch, solange gierig gewählt
wird; mit Abtastung hängt sie von der Saat ab.

## 5. Trainingsverfahren

### 5.1 Vortraining (Anbieter der Grundmodelle)

Die Grundmodelle wurden von ihrem Anbieter vortrainiert und
nachtrainiert (Anweisungsfolge, Denkmodus). Umfang, Daten und Verfahren
beschreibt der Anbieter; Myelith hat darauf keinen Einfluss und keine
darüber hinausgehenden Kenntnisse.

### 5.2 Umrechnung in Ganzzahlen (Myelith)

1. Gewichte aus der festgelegten Revision laden und je Ausgabekanal auf
   `int8` quantisieren.
2. **Kalibrieren**: Mit Texten aus WikiText-2 werden die Skalen der
   Aktivierungen bestimmt, damit sie im `int16`-Bereich bleiben. Die
   Kalibrierdaten verändern keine Gewichte.
3. Tabellen für die Nichtlinearitäten erzeugen.
4. Gegen die Gleitkomma-Referenz messen (Abschnitt 7).

Werkzeuge: eigene Rust-Laufzeit; PyTorch und Transformers nur für die
Referenz und die Kalibrierung.

### 5.3 Eigenes Nachtraining (Myelith)

Myelith kann die Artefakte **ganzzahlig nachtrainieren**: Vorwärtspass,
Rückwärtspass und Optimiererschritt laufen ohne Gleitkomma, mit
normiertem Schritt und stochastischer Rundung der Gewichte. Eingesetzt
wird das heute für

- **Messungen mit bekannter Wahrheit** (synthetische Lernleitern und
  erfundene Fakten, siehe [Trainingsdaten](Trainingsdaten.md)), die
  belegen, was das Training ablegt, und
- **Nachtraining durch den Nutzer** mit eigenem Material über die
  Buchkette (`TRAINING/korpus/`).

Hardware: ein Rechner mit Apple M5 Pro (15 Kerne, 24 GiB). Ein
Messlauf dauert Minuten.

## 6. Daten

Siehe die öffentliche [Zusammenfassung der Trainingsdaten](Trainingsdaten.md)
(Art. 53 Abs. 1 lit. d) und die [Urheberrechtsstrategie](Urheberrecht.md)
(Art. 53 Abs. 1 lit. c).

## 7. Bewertung

**Treue zur Gleitkomma-Referenz** (Perplexität auf WikiText-2, Testsplit;
Quelle: erzeugte [Modellkarte](../ethics/Modellkarte.md)):

| Artefakt | ganzzahlig | Gleitkomma | Abstand |
|---|---|---|---|
| myelith-0.6b | 43,49 | 42,26 | +2,92 % |
| myelith-4b | 19,95 | 19,63 | +1,65 % |
| myelith-8b | 13,27 | 12,79 | +3,75 % |
| myelith-30b-a3b | 10,42 | 10,48 | −0,59 % |

**Bitgleichheit**: 48 von 48 Konformitätsvektoren (Einzeloperationen, Ebenen, ganze Läufe) stimmen mit der Referenz überein.

**Fangfragen mit Denkbudget** (12 Fragen, 2026-09-25): ohne Überlegung
10/12 (30B und 8B), mit 32 Token Überlegung 12/12.

## 8. Grenzen und bekannte Fehlermodi

- **Erfundene Aussagen**: Die Modelle können falsche Tatsachen flüssig
  und überzeugt formulieren.
- **Wissensstand** der Grundmodelle; neuere Ereignisse kennen sie nicht,
  außer über die Web-Recherche.
- **Rechenfehler und Fangfragen**: Ohne Überlegung scheitern auch die
  großen Artefakte an Aufgaben wie dem Zählen von Buchstaben.
- **Abweichung durch die Ganzzahlarithmetik**: bis 3,75 % höhere
  Perplexität als die Referenz.
- **Sprache**: Deutsch ist schwächer als Englisch; die kleinen Artefakte
  schweifen eher ins Englische ab.
- **Werkzeuge**: Der Agent kann Werkzeuge falsch aufrufen oder Dateien
  unbeabsichtigt ändern; deshalb die Bestätigung vor Handlungen.
- **Verzerrungen** der Trainingsdaten der Grundmodelle werden geerbt.

## 9. Sicherheit und Ausrichtung

- **Ausrichtung** stammt aus dem Nachtraining der Grundmodelle durch
  ihren Anbieter. Myelith trainiert keine eigene Ausrichtung und
  schwächt die vorhandene nicht bewusst ab.
- **Schutzmaßnahmen auf Systemebene** stehen in der
  [Zweckbestimmung](Zweckbestimmung.md), Abschnitt 3.
- **Werkzeuge mit Grenze**: Der Agent sieht nur den freigegebenen
  Ordner; Befehle laufen eingehängt; die Risikoklassen je Werkzeug
  stehen in [`../ethics/Risikoklassen.toml`](../ethics/Risikoklassen.toml).
- **Nachvollziehbarkeit**: Bitgenaue Ausführung macht jede Antwort
  wiederholbar und im Netz prüfbar.

## 10. Rechenaufwand und Energie

| Schritt | Rechenaufwand | Energie (geschätzt) |
|---|---|---|
| Vortraining der Grundmodelle | beim Anbieter; nicht veröffentlicht | nicht bekannt |
| Umrechnung und Kalibrierung je Artefakt | Minuten bis wenige Stunden auf einem Rechner | unter 0,5 kWh |
| Eigene Nachtrainings-Messläufe | Minuten je Lauf | unter 0,1 kWh je Lauf |

Die Schätzung nimmt rund 60 W Leistungsaufnahme unter Last an. Der
eigene Rechenaufwand liegt weit unter einem Drittel des Aufwands für das
Vortraining der Grundmodelle.
