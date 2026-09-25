# Strategie zur Einhaltung des Urheberrechts

**Stand:** 2026-09-25 · **English:** [Copyright-Policy.md](../en/Copyright-Policy.md)

Strategie nach Artikel 53 Absatz 1 Buchstabe c der Verordnung (EU)
2024/1689, insbesondere zur Ermittlung und Wahrung von Rechtsvorbehalten
nach Artikel 4 Absatz 3 der Richtlinie (EU) 2019/790. Aufbau angelehnt an
das Kapitel Urheberrecht des Verhaltenskodex für KI-Modelle mit
allgemeinem Verwendungszweck.

## 1. Geltungsbereich

Diese Strategie gilt für alle Daten, mit denen Myelith-Artefakte
kalibriert oder trainiert werden, für fremden Code und fremde Modelle im
Projekt und für die Ausgaben des Systems. Verantwortlich ist der Anbieter
(siehe [GPAI-Dokumentation](GPAI-Dokumentation.md)).

## 2. Trainings- und Kalibrierdaten

1. **Kein Sammeln aus dem Internet.** Myelith betreibt heute keinen
   Crawler. Die verwendeten Daten sind regelbasiert selbst erzeugt oder
   ein öffentlicher Datensatz mit freier Lizenz (siehe
   [Trainingsdaten](Trainingsdaten.md)).
2. **Wird künftig aus dem Web gesammelt**, dann nur so:
   - `robots.txt` wird gelesen und befolgt, auch für Kennungen, die
     KI-Training ausschließen.
   - Andere maschinenlesbare Vorbehalte (Metadaten, Protokolle wie
     TDMRep, Angaben in den Nutzungsbedingungen, soweit maschinenlesbar)
     werden erkannt und befolgt.
   - Keine Inhalte hinter Bezahlschranken oder Zugangssperren, die
     umgangen werden müssten.
   - Keine Quellen, die für Urheberrechtsverletzungen bekannt sind.
   - Der Crawler nennt sich mit einer eigenen, dokumentierten Kennung.
3. **Jedes Korpus für das Netz** durchläuft das
   Korpus-Aufnahmeverfahren: Herkunft und Rechtsgrundlage jedes
   Bestandteils, Nachweis zu jeder Klasse des Ausschlusskatalogs,
   Verankerung per Merkle-Wurzel ([Vorlage](../ethics/vorlagen/korpus-aufnahmeantrag.json)).
4. **Nachtraining durch Nutzer** mit eigenem Material (Buchkette) findet
   auf dem Rechner des Nutzers statt. Ob er das Material dafür verwenden
   darf, entscheidet und verantwortet er selbst; die Kette weist darauf
   hin.

## 3. Fremde Modelle und fremder Code

- **Grundmodelle** werden nur verwendet, wenn ihre Lizenz Apache 2.0 oder
  MIT ist (Grundsatz G7 des [Ethik-Manifests](../ethics/Manifest.md));
  geprüft je Variante ([Lizenzlage](../ethics/Lizenzlage.md)) und im CI
  ([`lizenzprobe.py`](../ethics/werkzeuge/lizenzprobe.py)).
- **Alle Fremdkomponenten** stehen mit Lizenz in [NOTICES](../NOTICES.md),
  erzeugt aus den Paketdaten und im CI gegen sie geprüft.
- **Fremder Quelltext wird nicht übernommen.** Eine Idee aus einem
  anderen Projekt wird gelesen, verstanden und neu geschrieben; Zeilen
  tragen eine Lizenz, eine Idee nicht.

## 4. Ausgaben

- Sprachmodelle können sich in seltenen Fällen an Textstellen aus ihren
  Trainingsdaten erinnern und sie wiedergeben. Myelith trainiert die
  Grundmodelle nicht auf urheberrechtlich geschützten Werken nach und
  verstärkt dieses Risiko damit nicht.
- **Inhalte aus der Web-Recherche** gehen als Fundstelle mit Adresse in
  die Antwort ein; das Modell wird angewiesen, zusammenzufassen und zu
  verweisen statt lange Passagen wörtlich zu übernehmen.
- **Synthetische Ausgaben** (Stimme) werden als KI-erzeugt gekennzeichnet
  (siehe [Zweckbestimmung](Zweckbestimmung.md), Abschnitt 3).
- Die Zweckbestimmung untersagt, Ausgaben zur Verletzung von
  Urheberrechten zu verwenden. Verantwortlich für die Verwendung einer
  Ausgabe ist, wer sie verwendet.

## 5. Beschwerden von Rechteinhabern

Wer meint, dass Myelith seine Rechte verletzt, meldet das über die Issues
des Repositoriums, mit dem betroffenen Werk und der Fundstelle. Jede
Meldung wird geprüft und beantwortet; berechtigte Einwände führen zur
Entfernung des Materials aus künftigen Korpora und, wo möglich, zu einer
Korrektur.

## 6. Überprüfung

Diese Strategie wird bei jeder Änderung der Datenquellen und mindestens
einmal im Jahr überprüft. Stand und Datum stehen oben.
