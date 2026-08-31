# Myelith — Ethik-Manifest

**Version:** 1.1.0
**Datum:** 2026-08-31
**Geltung:** Für alle Komponenten dieses Repositoriums. Bei Konflikt mit
einer Komponentenplanung gilt dieses Dokument; Abweichungen sind zu
begründen und hier zu vermerken.

---

## Warum dieses Dokument existiert — und was es nicht ist

Myelith baut ein Netzwerk, in dem niemand die Kontrolle hat. Das ist der
Zweck: keine zentrale Instanz, die abschalten, zensieren oder
protokollieren kann. Genau diese Eigenschaft macht die üblichen
Ethik-Zusagen der KI-Branche hier **unmöglich**. Ein zentraler Anbieter
kann versprechen, ein Modell abzuschalten. Ein Protokoll ohne Betreiber
kann das nicht — und ein Versprechen, das man nicht halten kann, ist
schlimmer als keines.

Dieses Manifest zieht deshalb zuerst eine Grenze: **Was kann das
Protokoll erzwingen, was können nur Menschen entscheiden, und was ist
schlicht nicht kontrollierbar?** Erst danach folgen Zusagen — und zwar
nur solche, für die es einen Mechanismus gibt.

---

## 1. Die Grenze der Durchsetzbarkeit

Drei Kategorien, die im weiteren Text konsequent auseinandergehalten
werden:

### 1.1 Protokoll-durchsetzbar

Eigenschaften, die aus der Mechanik folgen und die kein Teilnehmer
umgehen kann, auch nicht die Autoren. Beispiele: Ein Trainingssegment
ohne gültigen Merkle-Beweis wird nicht vergütet (Whitepaper Kap. 7.3).
Ein Agent kann kein Budget überschreiten, das im Session-Kontrakt steht
(Kap. 8.2). Eine Berechnung, die von der Referenz abweicht, wird
geslasht (Kap. 6).

**Diese Kategorie ist die einzige, in der „garantiert" ein zulässiges
Wort ist.**

### 1.2 Governance-abhängig

Eigenschaften, die von einer Mehrheitsentscheidung der
Stimmberechtigten abhängen. Beispiele: welche Korpora kanonisch sind,
welche Modellversion übernommen wird, wie hoch Slash-Anteile liegen.
Das Protokoll setzt durch, *was* beschlossen wurde — nicht, *dass* gut
beschlossen wird.

**Hier gilt: Wir können den Prozess gestalten, nicht das Ergebnis
garantieren.** Ein Manifest, das Governance-Ergebnisse verspricht, ist
unehrlich.

### 1.3 Nicht kontrollierbar

Eigenschaften, die außerhalb der Reichweite des Protokolls liegen.
Beispiele: Was ein Nutzer mit einer Antwort tut. Ob ein Miner die
Gewichte kopiert und außerhalb des Netzes einsetzt (er kann — sie
liegen zwangsläufig auf seiner Hardware, Kap. 10.1). Ob das Modell in
einer konkreten Situation gut entscheidet.

**Hier ist die einzige ehrliche Zusage: Transparenz darüber, dass es
nicht kontrollierbar ist.**

---

## 2. Grundsätze

Jeder Grundsatz nennt den **Mechanismus**, der ihn trägt, und seine
**Kategorie** nach Abschnitt 1. Ein Grundsatz ohne Mechanismus ist eine
Absichtserklärung und als solche gekennzeichnet.

### G1 — Objektive Entscheidbarkeit statt Ermessen

Wo das Protokoll urteilt, urteilt es über nachrechenbare Tatsachen,
nicht über Inhalte. Ein Segment ist bitgleich oder nicht. Ein
Merkle-Beweis gilt oder nicht.

**Warum das ethisch relevant ist:** Jeder Bewertungsspielraum ist eine
Machtposition. Wer entscheidet, welcher Text „schädlich" ist,
entscheidet auch, welcher Text unbequem ist. Myelith vermeidet diese
Position nicht aus Gleichgültigkeit, sondern weil sie in einem
anonymen, offenen Netz nicht legitim besetzbar ist.

**Preis, den wir dafür zahlen:** Das Protokoll kann Missbrauch der
Inferenz nicht am Inhalt erkennen. Siehe Abschnitt 4.

*Mechanismus: Kap. 6 (Verifikation), Kap. 7.3 (Provenienz statt
Bewertung). Kategorie: protokoll-durchsetzbar.*

### G2 — Herkunft ist prüfbar, Inhalt wird nicht bewertet

Trainingsdaten werden nicht inhaltlich gefiltert, sondern auf Herkunft
geprüft: Jedes Segment referenziert eine Position in einem kanonischen
Korpus mit on-chain verankerter Merkle-Wurzel.

**Was das leistet:** Niemand kann eigene Daten einschleusen. Die
Zusammensetzung des Trainingskorpus ist öffentlich nachvollziehbar und
nicht durch einzelne Miner beeinflussbar (VRF-gesteuerte Zuweisung).

**Was das nicht leistet:** Es sagt nichts darüber, ob der Korpus selbst
rechtmäßig zusammengestellt wurde. Diese Frage verlagert sich damit
vollständig auf die **Aufnahme eines Korpus in die kanonische Liste** —
und das ist ein Governance-Akt, kein technischer.

*Mechanismus: Kap. 7.3. Kategorie: Provenienzprüfung
protokoll-durchsetzbar, Korpusauswahl governance-abhängig.*

### G3 — Korpus-Aufnahme ist der eigentliche Kontrollpunkt

Aus G2 folgt: Die einzige Stelle, an der über die Legitimität von
Trainingsdaten entschieden wird, ist die Aufnahme eines Korpus in die
kanonische Liste. **Diese Entscheidung darf nicht formlos sein.**

Ein Korpus-Aufnahmeantrag muss enthalten:

1. **Herkunft und Rechtsgrundlage** jedes Bestandteils — bei Web-Daten
   die Erhebungsmethode und der Umgang mit maschinenlesbaren
   Nutzungsvorbehalten (§ 44b Abs. 3 UrhG, Art. 4 Abs. 3 DSM-RL).
2. **Ausschluss-Nachweis:** Welche Filter liefen (Opt-out-Listen,
   bekannte Piraterie-Sammlungen, Material mit
   Kindesmissbrauchsbezug), mit welchem Werkzeug in welcher Version.
3. **Personenbezug:** Ob und in welchem Umfang personenbezogene Daten
   enthalten sind, und auf welcher Rechtsgrundlage.
4. **Reproduzierbarkeit:** Merkle-Wurzel, Erzeugungsskript,
   Werkzeugversionen — sodass ein Dritter den Korpus aus den Quellen
   nachbauen und die Wurzel bestätigen kann.

Ohne diese vier Angaben ist ein Antrag unvollständig und wird nicht zur
Abstimmung gestellt. **Das ist eine Selbstbindung der Governance-Regeln,
nicht des Codes** — sie muss in der Parameter-Registry (GOVERNANCE)
verankert werden, um Bestand zu haben.

*Kategorie: governance-abhängig. Verankerung: Komponente GOVERNANCE.*

### G4 — Nachvollziehbarkeit statt Vertrauen

Für jede Ausgabe des Netzwerks lässt sich rekonstruieren, welcher Pod
welchen Schritt gerechnet hat, mit welcher Modellversion, auf welchen
Eingaben. Für jedes Modell-Update lässt sich rekonstruieren, aus
welchen Gewichten und welchem Operator es hervorging.

**Das ist die stärkste Zusage dieses Manifests**, weil sie
protokoll-durchsetzbar ist und weil sie die Voraussetzung für alles
andere bildet: Ohne Rekonstruierbarkeit gibt es keine Verantwortung,
keine Haftung und keine Aufsicht.

*Mechanismus: Kap. 6 (Segmentkette, Attestierungen), Kap. 8.5, Kap. 10.2.
Kategorie: protokoll-durchsetzbar.*

### G5 — Schaden wird begrenzt, nicht verhindert

Agenten mit Verfügungsrechten operieren unter einem Session-Kontrakt:
Budget, Empfänger-Whitelist, Zeitfenster stehen **im Kontrakt, nicht im
Modellkontext**, und werden vom Konsens durchgesetzt — nicht vom Modell
und nicht vom Client.

**Der entscheidende Punkt:** Ein durch Prompt-Injection übernommener
Agent kann sein Budget trotzdem nicht überschreiten. Die Grenze liegt
außerhalb dessen, was das Modell beeinflussen kann.

**Was das nicht leistet:** Innerhalb des Budgets kann ein
kompromittierter Agent Schaden anrichten. Wer einem Agenten Rechte
einräumt, trägt dieses Risiko (Kap. 8.5).

*Mechanismus: Kap. 8.2, 8.3. Kategorie: protokoll-durchsetzbar
(Grenze), nicht kontrollierbar (Verhalten innerhalb der Grenze).*

### G6 — Vertraulichkeit wird nicht behauptet, wo sie nicht besteht

Aktivierungen liegen im Klartext auf Miner-Hardware. Das ist eine
Architektureigenschaft, keine Implementierungslücke. Nutzer erfahren
das **vor** der Nutzung, nicht im Kleingedruckten.

Die Risikoklassen aus Kap. 9.3 sind verbindlich zu kommunizieren:
Welche Inhalte sind für dieses Netz geeignet, welche nicht. Ein Client,
der das verschweigt, verstößt gegen dieses Manifest.

*Mechanismus: Kap. 9.1–9.3. Kategorie: Architektur-Tatsache;
Kommunikationspflicht ist Selbstbindung.*

### G7 — Das Basismodell muss frei nachnutzbar sein

Nur Basismodelle unter Apache 2.0 oder MIT kommen in Frage (Kap. 10.1).
Nicht aus ideologischen Gründen: Ein offenes Protokoll kennt seine
Nutzerzahl nicht und kann sie nicht begrenzen — Lizenzen mit
Nutzerzahl-Obergrenzen oder geografischen Beschränkungen sind schlicht
nicht einhaltbar.

**Stand (geprüft 2026-08-23, alle sieben Größen):** Die Warnung dieses
Punktes war berechtigt, **zwei von sieben Varianten fallen durch.**

| Variante | Lizenz | |
|---|---|---|
| 0.5B, 1.5B, 7B, 14B, 32B | Apache 2.0 | ✅ |
| **3B** | Qwen Research License, §2(a) „FOR NON-COMMERCIAL PURPOSES ONLY" | ❌ |
| **72B** | Qwen License, §4: gesonderte Lizenz ab 100 Mio. monatlich aktiven Nutzern | ❌ |

Die 72B-Klausel ist genau der Fall, den dieser Punkt als nicht
einhaltbar benennt: Ein offenes Protokoll hat keine Instanz, die
monatlich aktive Nutzer zählt, und keine, die eine Lizenz beantragen
könnte. Vollständige Prüfung samt Methode in
`INTEGER_LLM/docs/01_licenses.md`.

**Für die Skalierungsfrage (K6) folgt daraus:** Die nächste Größe nach
7B ist **14B**, nicht 72B.

**Fund bei dieser Prüfung:** Dieser Absatz berief sich zuvor auf
`INTEGER_LLM/models/Qwen2.5-0.5B/LICENSE`. Die Datei existierte nicht.
Die Beschaffung lud mit `allow_patterns=['*.json','*.safetensors',
'*.txt']`, und eine Lizenzdatei trägt keine Endung. Behoben in
`myl-testclient` v0.11.0; sie liegt jetzt neben den Gewichten. Ein
Grundsatz, dessen Beleg niemand nachgesehen hat, ist dieselbe Klasse wie
Fund 27.

Vor jedem Modellwechsel bleibt die Lizenz der **konkreten Variante** zu
prüfen, nicht die der Familie.

*Kategorie: Auswahlkriterium, governance-abhängig.*

### G8 — Energie wird nicht verschwendet

Der Redundanzfaktor 2 plus Stichproben ist der Preis der Verifikation
und liegt bei etwa 2,1× der Nutzarbeit. Das ist erheblich — aber es ist
**Nutzarbeit**, keine verworfene Hash-Suche. Diese Unterscheidung ist
der ökologische Kern des Entwurfs und darf nicht durch Wachstum um
seiner selbst willen entwertet werden.

**Selbstbindung:** Der Redundanzfaktor wird nicht ohne
Sicherheitsbegründung erhöht. Jede Erhöhung ist mit ihrer
Energiewirkung zu beziffern.

*Mechanismus: Kap. 4.4, 6.10. Kategorie: Absichtserklärung mit
Governance-Verankerung.*

### G9 — Was das Netz nicht lernt und nicht bedient

**Festlegung des Projektinhabers, 2026-08-31.** Es gibt einen benannten
Ausschlusskatalog (`ETHICS/Ausschluss.json`): Massenvernichtungswaffen,
konventionelle Waffen und Sprengstoff, Angriffswerkzeuge gegen fremde
Systeme, Missbrauchsdarstellungen und Verfolgung von Personen, Täuschung
über die Person.

**Wo er greift, und wo nicht.** Die Aufnahme eines Korpus in die
kanonische Liste ist nach G2 ausdrücklich ein **Governance-Akt, kein
technischer**, und ein Governance-Akt darf Ablehnungsgründe haben. Dort
wirkt der Katalog: Ein Antrag muss zu **jeder** Klasse Werkzeug, Version
und Zahl der ausgeschlossenen Stücke nennen, sonst wird er nicht zur
Abstimmung gestellt.

⚑ **Bei der Abfrage wirkt er nicht auf Protokollebene, und das steht
hier, weil es sonst jemand anders behaupten würde.** G1 sagt, das
Protokoll könne Missbrauch der Inferenz nicht am Inhalt erkennen; daran
ändert dieser Grundsatz nichts. Der Katalog bindet **Betreiber von
Gateways und Clients**. Da das Basismodell nach G7 frei nachnutzbar sein
muss, kann es ohnehin jeder lokal ausführen.

**Die Zusage lautet deshalb: Dieses Netz bedient es nicht und lernt es
nicht.** Sie lautet nicht, es sei unmöglich. Abschnitt 7 verbietet
Zusagen ohne Mechanismus; die stärkere Fassung wäre genau das.

⚑ **Jede Klasse trägt eine Abgrenzung, und ohne sie fällt sie durch die
Prüfung.** „Waffen" als Stichwort verschluckt Geschichte, Chemie,
Metallurgie, Rüstungskontrolle und den halben Journalismus. Eine Klasse
ohne Abgrenzung ist kein Ausschluss, sondern ein Ermessensspielraum, und
G1 nennt den Grund, warum es den nicht geben soll: Wer entscheidet,
welcher Text schädlich ist, entscheidet auch, welcher Text unbequem ist.
**Der Maßstab ist Befähigung, nicht Thema.**

**Selbstbindung:** Der Katalog wird nicht stillschweigend erweitert.
Jede Klasse und jede Abgrenzung steht in einer versionierten Datei, und
`ausschlussprobe.py` weist eine Klasse ohne Abgrenzung ab.

*Mechanismus: Governance-Akt bei der Korpus-Aufnahme (Kap. 7.3, 10.3).
Kategorie: bei der Aufnahme governance-durchsetzbar, bei der Abfrage
Betreiberpflicht und nicht protokoll-durchsetzbar.*

---

## 3. Selbstbindungen der Autoren

Zusagen, die nicht vom Protokoll erzwungen werden, sondern von den
Autoren dieses Repositoriums eingehalten werden — und an denen sie
gemessen werden können.

| # | Selbstbindung | Prüfbar woran |
|---|---|---|
| S1 | Kein Genesis-Vorverkauf, keine Vorab-Zuteilung an die Autoren außerhalb der dokumentierten Treasury | Genesis-Manifest, Kap. 5.7 |
| S2 | Die Quantisierung des Genesis-Modells ist vollständig reproduzierbar dokumentiert (Ausgangsgewichte, Verfahren, Kalibrierungsdaten, Werkzeugversionen) | `theta_v/spec.json`, `INTEGER_LLM/artifacts/`, Konformitätspaket |
| S3 | Bekannte Schwächen werden im Whitepaper und in den Fahrplänen benannt, nicht in Fußnoten versteckt | Kap. 11, Abschnitt „Was dieser Entwurf offenlässt" in jedem Kapitel |
| S4 | Negative Messergebnisse werden veröffentlicht, auch wenn sie das Projekt schlecht aussehen lassen | `eval/results/decision_12-21.md` (verfehltes Kriterium wurde dokumentiert, nicht kaschiert) |
| S5 | Audit-Funde werden vollständig dokumentiert, auch wenn sie eigene frühere Zusagen widerlegen | Abschnitt 4.6 |
| S6 | Keine Zusammenarbeit mit Vorhaben, deren Zweck Massenüberwachung oder autonome Waffenwirkung ist | Selbstbindung ohne technischen Mechanismus — ausdrücklich als solche gekennzeichnet |

**S6 ist bewusst als schwach gekennzeichnet.** Das Protokoll kann eine
solche Nutzung nicht verhindern (Abschnitt 1.3). Die Selbstbindung
betrifft das Handeln der Autoren, nicht das Netzwerk.

---

## 4. Was wir ausdrücklich **nicht** versprechen

Dieser Abschnitt ist der wichtigste. Wer ihn überspringt, hat das
Manifest missverstanden.

1. **Keine Inhaltsmoderation.** Das Netzwerk filtert Ausgaben nicht.
   Es kann es architektonisch nicht — und wenn es könnte, wäre die
   Filterinstanz eine Machtposition, die das Protokoll gerade
   vermeidet (G1). Wer Moderation braucht, braucht sie in der
   Anwendungsschicht über dem Netzwerk.
2. **Keine Abschaltbarkeit.** Es gibt keinen Kill-Switch. Auch nicht
   für uns. Das ist beabsichtigt und irreversibel.
3. **Keine Löschgarantie.** Modellgewichte, die einmal trainiert
   wurden, tragen Information aus den Trainingsdaten. Ein
   „Herauslöschen" einzelner Inhalte aus Gewichten ist nach heutigem
   Stand nicht verlässlich möglich. Das steht in Spannung zu
   DSGVO Art. 17 und ist als **ungelöst** zu behandeln, nicht als
   gelöst zu behaupten (Phase 2).
4. **Keine Zusicherung, dass das Modell gut entscheidet.** Es kann der
   Fall eintreten, dass alle Beteiligten korrekt handelten, das
   Protokoll fehlerfrei arbeitete und dennoch Schaden entstand
   (Kap. 8.5).
5. **Keine Vertraulichkeit gegenüber Minern.** Siehe G6.
6. **Keine Aussage über die Rechtmäßigkeit der Nutzung in einer
   bestimmten Jurisdiktion.** Ein Protokoll ohne Betreiber lässt sich
   nicht jurisdiktionsweise konfigurieren.

---

## 5. Die vier ungelösten Spannungen

Ehrlich benannt, nicht wegdefiniert. Soweit sie technisch adressierbar
sind, werden sie abgearbeitet; wo nicht, steht hier, warum.

### T1 — Zensurresistenz gegen Rechtsdurchsetzung

Das Protokoll ist so gebaut, dass niemand Inhalte entfernen kann. Ein
Gericht, das die Entfernung anordnet, findet keinen Adressaten, der es
umsetzen könnte. **Wir halten das für richtig** — dieselbe Eigenschaft
schützt vor politischer Zensur — aber es ist eine Entscheidung mit
Kosten, keine Neutralität.

### T2 — Unveränderliche Kette gegen das Recht auf Löschung

Prompts gehen nicht in die Kette ein (nur Commitments), aber
Segment-Metadaten und Attestierungen sind dauerhaft. Für den
Agenten-Speicher (Kap. 8.4) gilt das ebenfalls. Ob
Commitment-Speicherung personenbezogene Daten im Sinne der DSGVO
darstellt, ist eine Rechtsfrage, die **vor** einem Produktivstart
geklärt werden muss.

### T3 — Anonyme Teilnahme gegen Verantwortlichkeit

Miner sind pseudonym. Wenn ein Agent Schaden anrichtet, ist der
Rechenweg rekonstruierbar (G4) — die dahinterstehende Person unter
Umständen nicht. Das Protokoll bietet Nachvollziehbarkeit der
*Berechnung*, nicht Identifizierbarkeit der *Beteiligten*.

### T4 — Offene Gewichte gegen Missbrauchskontrolle

Die Gewichte liegen zwangsläufig auf Miner-Hardware und sind damit
faktisch öffentlich. Jede Fähigkeit des Modells ist außerhalb des
Netzwerks nutzbar, ohne jede Schranke. Ein Modell, dessen Fähigkeiten
nur unter Aufsicht vertretbar wären, gehört nicht in dieses Netzwerk —
diese Beurteilung ist ein Governance-Akt vor jedem Modell-Update.

---

## 6. Verhältnis zu den Komponenten

| Komponente | Was dieses Manifest dort verlangt |
|---|---|
| **INTEGER_LLM** | Reproduzierbare, dokumentierte Quantisierung (S2); Lizenzprüfung der konkreten Modellvariante vor jedem Wechsel (G7); Modellkarte je θ_v-Version |
| **TRAINING** | Provenienzpflicht ohne Ausnahme (G2); vollständiger Korpus-Aufnahmeantrag (G3); keine inhaltliche Bewertung (G1) |
| **AGENT_LAYER** | Kontraktgrenzen außerhalb des Modellkontexts (G5); Dual-LLM-Trennung; Offenlegung der Risikoklasse gegenüber dem Nutzer (G6) |
| **GOVERNANCE** | Verankerung von G3 und S1–S5 in der Parameter-Registry; Verfassungsrang für die nicht verhandelbaren Punkte |
| **CLIENT** | Risikoklassen-Anzeige vor der Nutzung (G6); keine Vertraulichkeitsbehauptung |
| **VERIFICATION** | Trägt G4 — ohne Verifikation keine Nachvollziehbarkeit |

---

## 7. Änderung dieses Manifests

Abschnitte 1 (Grenze der Durchsetzbarkeit) und 4 (Was wir nicht
versprechen) sind **Verfassungsrang** im Sinne von Kap. 10.3: Eine
Änderung, die dort Zusagen hinzufügt, für die es keinen Mechanismus
gibt, ist unzulässig. Änderungen, die Zusagen **streichen**, weil sie
sich als nicht haltbar erwiesen haben, sind ausdrücklich erwünscht und
mit Begründung im Changelog zu vermerken.

Alle übrigen Abschnitte folgen dem Governance-Prozess aus Kap. 10.3.

---

## Changelog

### v1.1.0 – 2026-08-31 (G9: was das Netz nicht lernt und nicht bedient)

Auf Festlegung des Projektinhabers. Ein benannter Ausschlusskatalog in
`ETHICS/Ausschluss.json`, fünf Klassen, jede mit Abgrenzung.

⚑ **Der Grundsatz fügt eine Zusage hinzu, und Abschnitt 7 verlangt dafür
einen Mechanismus.** Er hat einen, aber nur an einer der beiden Stellen:
Bei der Korpus-Aufnahme wirkt er über den Governance-Akt, den G2 ohnehin
dort verortet; bei der Abfrage bindet er Betreiber und nicht den
Konsens. Beides steht ausdrücklich im Text, damit die stärkere Fassung
nicht später hineingelesen wird.

⚑ **Und er hebt G1 nicht auf.** Das Protokoll bekommt keinen
Ermessensspielraum; es rechnet weiter, was ihm gegeben wird. Was sich
ändert, ist der Katalog der Ablehnungsgründe eines Antrags, und der
gehört seit jeher der Governance.

**Der Antrag ist mitgewachsen:** `ausschluss` ist kein freies Textfeld
mehr, sondern eine Zeile je Katalogklasse. Bis dahin konnte ein Antrag
„Dubletten entfernt" eintragen und galt als vollständig, während über
die Klassen, um die es geht, nichts dastand.

### v1.0.0 – 2026-08-18
- Erstfassung. Aufbau bewusst als Grenzziehung zuerst
  (durchsetzbar / governance-abhängig / nicht kontrollierbar), dann
  Grundsätze mit Mechanismus, dann die vier ungelösten Spannungen.
- Grundlage: Whitepaper v0.3 Kap. 6 (Verifikation), 7.3 (Provenienz),
  8.2–8.5 (Session-Kontrakte, Verantwortung), 9.1–9.3
  (Vertraulichkeit), 10.1–10.3 (Modell-Herkunft, Governance).
