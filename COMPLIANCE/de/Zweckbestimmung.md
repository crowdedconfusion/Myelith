# Zweckbestimmung und ausgeschlossene Verwendungen

**Stand:** 2026-09-25 · **English:** [Intended-Use-Policy.md](../en/Intended-Use-Policy.md)

Diese Zweckbestimmung legt fest, wofür Myelith entwickelt wurde und
wofür es nicht verwendet werden darf. Sie ist Teil der Nutzungsbedingungen
im Sinne der Verordnung (EU) 2024/1689 (KI-Verordnung) und gilt für jede
Verwendung der Software, der Modelle und der Freigabebündel.

## 1. Wofür Myelith entwickelt wurde

Myelith ist ein **allgemeiner, lokal laufender KI-Assistent** mit einem
Sprachmodell als Kern. Vorgesehen sind:

- **Gespräch und Auskunft**: Fragen beantworten, Texte erklären,
  zusammenfassen, übersetzen, Ideen entwickeln.
- **Arbeit an eigenen Dateien**: Texte und Quelltext in einem Ordner
  lesen, ändern und anlegen, den der Nutzer ausdrücklich freigibt.
- **Recherche im Web**, wenn der Nutzer sie einschaltet.
- **Sinne**: gesprochene Sprache in Text umwandeln (Hören), Bilder
  beschreiben (Sehen), Antworten vorlesen (Sprechen), auf Wunsch mit
  einer Stimme, die der Nutzer selbst als Aufnahme bereitstellt.
- **Teilnahme am Myelith-Netz**: Rechenarbeit für das Netz leisten und
  prüfen, wie sie das Whitepaper beschreibt.

Myelith ist für **Privatpersonen und Entwickler** gedacht, die einen
Assistenten auf ihrem eigenen Rechner betreiben wollen.

## 2. Ausgeschlossene Verwendungen

### 2.1 Verbotene Praktiken nach Artikel 5 KI-Verordnung

Myelith darf **nie** verwendet werden für:

1. **Unterschwellige, manipulative oder täuschende Beeinflussung** von
   Menschen, die ihr Verhalten wesentlich verzerrt und ihnen schadet
   (Art. 5 Abs. 1 lit. a).
2. **Ausnutzen von Schwächen** aufgrund von Alter, Behinderung oder
   sozialer oder wirtschaftlicher Lage (Art. 5 Abs. 1 lit. b).
3. **Social Scoring**: Bewertung oder Einstufung von Menschen nach ihrem
   Sozialverhalten oder ihren persönlichen Eigenschaften mit
   benachteiligender Folge (Art. 5 Abs. 1 lit. c).
4. **Vorhersage von Straftaten** allein aufgrund von Profiling oder
   Persönlichkeitsmerkmalen (Art. 5 Abs. 1 lit. d).
5. **Ungezieltes Auslesen von Gesichtsbildern** aus dem Internet oder aus
   Überwachungsaufnahmen zum Aufbau von Gesichtsdatenbanken
   (Art. 5 Abs. 1 lit. e).
6. **Emotionserkennung am Arbeitsplatz und in Bildungseinrichtungen**
   (Art. 5 Abs. 1 lit. f).
7. **Biometrische Kategorisierung** nach Herkunft, politischer Meinung,
   Gewerkschaftszugehörigkeit, religiöser oder weltanschaulicher
   Überzeugung, Sexualleben oder sexueller Orientierung
   (Art. 5 Abs. 1 lit. g).
8. **Biometrische Echtzeit-Fernidentifizierung** in öffentlich
   zugänglichen Räumen (Art. 5 Abs. 1 lit. h).

### 2.2 Hochrisiko-Bereiche nach Anhang III KI-Verordnung

Myelith ist **nicht** für die folgenden Bereiche bestimmt und darf dort
nicht eingesetzt werden. Wer es dennoch dort einsetzt, verändert die
Zweckbestimmung und übernimmt damit selbst die Pflichten eines Anbieters
eines Hochrisiko-KI-Systems (Art. 25 KI-Verordnung).

1. **Biometrie**: Fernidentifizierung, biometrische Kategorisierung,
   Emotionserkennung.
2. **Kritische Infrastruktur**: Sicherheitsbauteile in der Verwaltung
   und im Betrieb digitaler Infrastruktur, des Straßenverkehrs, der
   Wasser-, Gas-, Wärme- und Stromversorgung.
3. **Bildung und Berufsausbildung**: Zugang und Zulassung, Bewertung von
   Lernergebnissen, Einstufung des Bildungsniveaus, Überwachung bei
   Prüfungen.
4. **Beschäftigung und Personalmanagement**: Auswahl von Bewerbern,
   Entscheidungen über Einstellung, Beförderung und Kündigung,
   Aufgabenzuweisung, Leistungs- und Verhaltensbewertung.
5. **Wesentliche private und öffentliche Dienste**: Anspruch auf
   Sozialleistungen, Kreditwürdigkeit und Bonität, Risiko und Preise bei
   Lebens- und Krankenversicherungen, Einstufung von Notrufen und
   Einsatzplanung von Rettungsdiensten.
6. **Strafverfolgung**: Risikobewertung von Personen, Lügendetektoren,
   Beweiswürdigung, Profiling.
7. **Migration, Asyl und Grenzkontrolle**: Risikobewertung, Prüfung von
   Anträgen, Identifizierung von Personen.
8. **Rechtspflege und demokratische Prozesse**: Unterstützung
   richterlicher Entscheidungen, Beeinflussung von Wahlen und
   Abstimmungen.

### 2.3 Weitere ausgeschlossene Verwendungen

- **Eine Stimme ohne Einwilligung nachbilden.** Die Stimmnachbildung ist
  für die eigene Stimme des Nutzers gedacht oder für die einer Person,
  die ausdrücklich eingewilligt hat. Eine nachgebildete Stimme darf nicht
  als echte Aufnahme einer Person ausgegeben werden.
- **Die Kennzeichnung synthetischer Inhalte entfernen**, um sie als echt
  auszugeben.
- Alles, was der Ausschlusskatalog des Projekts nennt
  ([`../ethics/Ausschluss.json`](../ethics/Ausschluss.json)).

## 3. Technische Maßnahmen gegen Missbrauch

| Maßnahme | Wirkung |
|---|---|
| Hinweis beim Start, aktiv zu bestätigen | Niemand nutzt Myelith, ohne zu wissen, dass es eine KI ist und wofür sie nicht gedacht ist |
| Dauerhafte Kennzeichnung „KI" in der Oberfläche | Jede Antwort ist als KI-erzeugt erkennbar |
| Schutzfilter für Anfragen | Anfragen, die erkennbar auf eine verbotene Praxis zielen, werden mit Verweis auf diese Zweckbestimmung abgelehnt; die Ablehnung wird ohne Klartext protokolliert |
| Kennzeichnung der synthetischen Stimme | Metadaten und Wasserzeichen in jeder erzeugten Tondatei |
| Bestätigung vor Handlungen | Schreiben, Netzzugriffe und Befehle des Agenten werden in der Vorgabe vorgelegt, bevor sie ausgeführt werden |
| Notaus | Ein Knopf und ein Tastenkürzel brechen jede laufende Handlung des Agenten ab |
| Aktionsprotokoll | Jede Handlung des Agenten wird lokal protokolliert, ohne Klartext, 30 Tage lang |
| Kein Personenbezug im Sehen | Das Sehmodell wird angewiesen, Personen nicht zu identifizieren und keine Gefühle oder sensiblen Merkmale zuzuschreiben |

Der Stand der Umsetzung je Maßnahme steht in der
[Selbsteinschätzung](Selbsteinschaetzung.md).

⚠️ **Grenzen.** Ein Filter erkennt Absichten nicht zuverlässig, und ein
lokal laufendes Programm lässt sich verändern. Die Maßnahmen erschweren
Missbrauch, sie verhindern ihn nicht. Verantwortlich für eine Verwendung
entgegen dieser Zweckbestimmung ist, wer sie so verwendet.

## 4. Meldung

Wer eine Verwendung entgegen dieser Zweckbestimmung bemerkt oder eine
Lücke in den Schutzmaßnahmen findet, meldet sie über die Issues des
Repositoriums.
