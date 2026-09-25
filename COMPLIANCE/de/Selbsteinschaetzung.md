# Selbsteinschätzung nach der KI-Verordnung

**Stand der Überprüfung:** 2026-09-25 · **English:** [Self-Assessment.md](../en/Self-Assessment.md)

> ⚠️ **Selbsteinschätzung, keine Rechtsberatung.** Diese Übersicht hat
> der Anbieter selbst erstellt, nach bestem Verständnis der Verordnung
> (EU) 2024/1689 und der Leitlinien der Kommission. Sie ersetzt keine
> anwaltliche Prüfung und keine Entscheidung einer Behörde.

## Einordnung in Kürze

| Frage | Antwort |
|---|---|
| Wer ist Anbieter? | Joschka Benjamin Hänsler, Privatperson, Deutschland, nicht kommerziell |
| Was wird angeboten? | Ein **KI-System** (der lokale Assistent Myelith: Fenster, Konsole, Kommandozeile) und **Bearbeitungen von KI-Modellen mit allgemeinem Verwendungszweck** (die Myelith-Artefakte) |
| Hochrisiko? | **Nein.** Die Zweckbestimmung schließt alle Bereiche aus Anhang III aus |
| GPAI mit systemischem Risiko? | **Nein.** Weit unter der Schwelle von 10²⁵ Gleitkommaoperationen |
| Ausnahme für freie und quelloffene Lizenzen? | **Nicht in Anspruch genommen.** Die PolyForm Shield License 1.0.0 schränkt die Nutzung für konkurrierende Produkte ein und ist damit keine freie und quelloffene Lizenz im Sinne von Art. 2 Abs. 12, Art. 53 Abs. 2 und Erwägungsgrund 102 |
| Rolle bei den GPAI-Pflichten | Anbieter mit allen Pflichten nach Art. 53 für die eigenen Bearbeitungen, weil Myelith selbst nachtrainiert (Entscheidung des Anbieters) |

## Übersicht je Artikel

Status: ✅ umgesetzt · 🟡 teilweise umgesetzt · ➖ nicht anwendbar

| Artikel | Inhalt | Status | Umsetzung oder Begründung |
|---|---|---|---|
| Art. 2 | Anwendungsbereich | ✅ | Myelith wird vorsorglich als in der Union bereitgestellt behandelt; die Ausnahme für freie und quelloffene Lizenzen greift nicht (siehe oben) |
| Art. 4 | KI-Kompetenz | 🟡 | Die Pflicht gilt für Personal von Anbietern und Betreibern; der Anbieter ist eine Einzelperson. Nutzer erfahren Fähigkeiten und Grenzen im Starthinweis und in der [Zweckbestimmung](Zweckbestimmung.md) |
| Art. 5 | Verbotene Praktiken | ✅ | [Zweckbestimmung](Zweckbestimmung.md) Abschnitt 2.1; Schutzfilter an allen Eingängen (`CLIENT/myl-client/src/schutzfilter.rs`), Abweisungen im Aktionsprotokoll ohne Klartext; Regel für das Sehmodell (`CLIENT/myl-senses/src/sehen.rs`, `SEHREGEL`). Grenze: ein Filter lässt sich umschreiben |
| Art. 6, Anhang III | Einstufung als Hochrisiko | ➖ | Nicht für Anhang-III-Bereiche bestimmt ([Zweckbestimmung](Zweckbestimmung.md) Abschnitt 2.2). Wer Myelith dort einsetzt, ändert die Zweckbestimmung (Art. 25) |
| Art. 8 bis 15 | Anforderungen an Hochrisiko-Systeme | ➖ | Nicht anwendbar. **Freiwillig** nach ihrem Vorbild: Aktionsprotokoll (Art. 12), Bestätigung vor Handlungen und Notaus (Art. 14) |
| Art. 12 (freiwillig) | Protokollierung | ✅ | `CLIENT/myl-client/src/protokoll.rs`: jede Handlung des Agenten als JSON-Zeile mit Zeit, Art, Werkzeug, Fingerabdruck von Ein- und Ausgabe (SHA-256 mit Schlüssel, kein Klartext), Entscheidung und Ergebnis; 30 Tage; Anzeige im Fenster (Einstellungen, Aktionsprotokoll) und mit `myl protokoll` |
| Art. 14 (freiwillig) | Menschliche Aufsicht | ✅ | Vorgabe `agent.modus = manual`: Schreiben, Befehle und Web-Anfragen werden vor dem Ausführen vorgelegt. Notaus im Fenster (Knopf, ⌘. oder Strg+.) und in der Konsole (Strg-C oder Esc während eines Laufs); er hält die Erzeugung und jedes weitere Werkzeug an, das Gespräch bleibt erhalten |
| Art. 16 bis 22 | Pflichten bei Hochrisiko-Systemen, Bevollmächtigte | ➖ | Nicht anwendbar. Art. 22 betrifft Anbieter aus Drittländern; der Anbieter sitzt in Deutschland |
| Art. 25 | Verantwortung entlang der Wertschöpfungskette | ✅ | [Zweckbestimmung](Zweckbestimmung.md) Abschnitt 2.2: Wer die Zweckbestimmung ändert, übernimmt Anbieterpflichten |
| Art. 50 Abs. 1 | Hinweis auf KI | ✅ | Hinweis bei **jedem** Start, aktiv zu bestätigen (Fenster und Konsole), Zeile auf der Fehlerausgabe bei `myl`; dauerhafte KI-Marke im Kopf des Fensters und in der Fußzeile der Konsole; jede Antwort im Fenster trägt „KI-generiert" (`CLIENT/myl-client/src/kennzeichnung.rs`) |
| Art. 50 Abs. 2 | Kennzeichnung synthetischer Inhalte | 🟡 | **Sprache ✅**: jede erzeugte Tondatei trägt Metadaten (XMP mit IPTC-Quellentyp `trainedAlgorithmicMedia`, RIFF-INFO) und ein Wasserzeichen im Signal, bevor sie abgespielt oder abgelegt wird; was sich nicht kennzeichnen lässt, wird nicht gespielt (`CLIENT/myl-senses/src/kennzeichnung.rs`); prüfbar mit `myl kennzeichen <datei>`. **Text 🟡**: in der Oberfläche gekennzeichnet, in Dateien, die der Agent schreibt, noch nicht maschinenlesbar. **Bild, Video ➖**: werden nicht erzeugt. **C2PA-Signatur**: noch nicht umgesetzt |
| Art. 50 Abs. 3 | Emotionserkennung, biometrische Kategorisierung | ➖ | Wird nicht angeboten und ist ausgeschlossen (Art. 5) |
| Art. 50 Abs. 4 | Deepfakes | ✅ | Pflicht der Betreiber; unterstützt durch die Kennzeichnung jeder erzeugten Stimme und die Einwilligung beim Hochladen einer Stimmprobe (im Fenster und im Befehl geprüft) |
| Art. 51, 52, 55 | GPAI mit systemischem Risiko | ➖ | Unter der Schwelle |
| Art. 53 Abs. 1 lit. a, b | Technische Dokumentation | ✅ | [GPAI-Dokumentation](GPAI-Dokumentation.md) (Anhänge XI und XII, Formular des Verhaltenskodex) |
| Art. 53 Abs. 1 lit. c | Urheberrechtsstrategie | ✅ | [Urheberrecht](Urheberrecht.md) |
| Art. 53 Abs. 1 lit. d | Zusammenfassung der Trainingsdaten | ✅ | [Trainingsdaten](Trainingsdaten.md) (Vorlage des AI Office) |
| Art. 54 | Bevollmächtigte für GPAI | ➖ | Anbieter in der Union |

## Weitere Rechtsgebiete, die hier berührt werden

| Gebiet | Stand |
|---|---|
| Datenschutz (DSGVO) | Myelith verarbeitet alles auf dem Rechner des Nutzers. Eine Stimmprobe ist ein personenbezogenes Datum; sie wird nur mit bestätigter Einwilligung abgelegt und verlässt den Rechner nicht. Das Aktionsprotokoll enthält keinen Klartext |
| Lizenzen der Fremdkomponenten | [NOTICES](../NOTICES.md), erzeugt und im CI geprüft. Offen: die Schrift der Wortmarke (Lizenz ungeklärt) |

## Offene Punkte

1. **C2PA-Manifest** für erzeugte Tondateien (braucht ein Signaturzertifikat).
2. **Maschinenlesbare Kennzeichnung von Texten**, die der Agent in Dateien schreibt.
3. **Lizenz der Schrift der Wortmarke** klären oder den Ausschnitt entfernen.
4. **Anwaltliche Prüfung** dieser Einschätzung.

## Überprüfung

Diese Selbsteinschätzung wird bei jeder Änderung, die einen der
genannten Punkte berührt, und mindestens alle sechs Monate überprüft.
