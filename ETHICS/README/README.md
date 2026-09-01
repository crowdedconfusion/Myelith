# ethics

> **Version:** 0.4.0
> **Datum:** 2026-08-31
> **Status:** Manifest v0.2.0 steht (neu: **G9**, der Ausschlusskatalog),
> **Phase 1 abgeschlossen**: aus den
> Zusagen sind Dateien geworden, die man erzeugen, diffen und im CI
> prüfen kann. Die Design-Entscheidungen bleiben offen, und Phase 2
> braucht eine Kanzlei, keine Arbeit.

Ethische und rechtliche Standards für alle Komponenten — als
prüfbare Anforderungen, nicht als Absichtserklärung.

## Aufgabe

Diese Komponente hat **keinen eigenen Konsenspfad und keinen eigenen
Crate**. Sie ist eine Querschnittskomponente: Sie formuliert
Anforderungen, die andere Komponenten erfüllen, und liefert die
Werkzeuge, mit denen sich das nachprüfen lässt.

Der Grund für die eigene Komponente statt eines Abschnitts im
Whitepaper: Ethische Zusagen verfallen, wenn sie nicht an Artefakte
gebunden sind, die mit dem Code mitwandern. Eine Modellkarte, die bei
jedem θ_v-Wechsel neu erzeugt werden muss, ist wirksam. Ein Absatz im
Whitepaper ist es nicht.

**Drei Adressaten:**

- **INTEGER_LLM** — Reproduzierbarkeit der Quantisierung,
  Lizenzprüfung der Modellvariante, Modellkarte je θ_v-Version.
- **TRAINING** — Korpus-Aufnahmeverfahren, Provenienznachweis,
  Opt-out-Behandlung, Personenbezug.
- **AGENT_LAYER** — Kontraktgrenzen außerhalb des Modellkontexts,
  Risikoklassen-Offenlegung, Protokollierung der
  Verantwortungskette.

## Abhängigkeiten

Keine technische Abhängigkeit — diese Komponente kann jederzeit
bearbeitet werden. Inhaltlich setzt sie auf Whitepaper v0.3 auf
(Kap. 6, 7.3, 8.2–8.5, 9.1–9.3, 10.1–10.3).

**Wer davon abhängt:** GOVERNANCE (die Selbstbindungen aus dem Manifest
brauchen Verankerung in der Parameter-Registry, sonst sind sie
unverbindlich), TRAINING (Korpus-Aufnahmeverfahren ist
Voraussetzung für Phase 2), AGENT_LAYER (Risikoklassen-Offenlegung),
CLIENT (Anzeige der Vertraulichkeitsklasse).

## Struktur

```
ETHICS/
├── Manifest.md               Das normative Dokument: Grenze der
│                             Durchsetzbarkeit, acht Grundsätze mit
│                             Mechanismus, sechs Selbstbindungen,
│                             vier ungelöste Spannungen
└── README/
    └── README.md             diese Kurzübersicht
```

Phase 1 legt zusätzlich an:

```
ETHICS/
├── modelcard/                Vorlage + Generator für Modellkarten
├── corpus-intake/            Vorlage für Korpus-Aufnahmeanträge
└── checklists/               Prüflisten je Komponente
```

## Was diese Komponente ausdrücklich nicht tut

- **Keine Inhaltsfilterung.** Das Netzwerk bewertet Inhalte nicht
  (Manifest G1). Wer Moderation braucht, baut sie über dem Netzwerk.
- **Keine Rechtsberatung.** Die rechtlichen Einordnungen sind
  Arbeitsgrundlage für eine anwaltliche Prüfung.
- **Keine Zusagen ohne Mechanismus.** Jeder Grundsatz im Manifest
  nennt, was ihn trägt — oder ist als Absichtserklärung markiert.

## Changelog

⚑ **Umnummeriert am 2026-08-31** (Festlegung des Projektinhabers): Diese
Komponente lief als einzige auf `1.x` und steht jetzt wie alle anderen
auf `0.x`. **Eine Eins vorn heißt Produktionsreife**, und die ist nicht
erreicht. Am Inhalt ändert das nichts, die Einträge behalten ihre
Reihenfolge: `v1.0.0` heißt jetzt `v0.1.0`, `v1.1.0` heißt `v0.2.0`,
`v1.2.0` heißt `v0.3.0`. Ebenso `Manifest.md` (`1.1.0` zu `0.2.0`) und
die Fassung von `Ausschluss.json` (`1.0.0` zu `0.1.0`).

### v0.4.0 – 2026-09-01 (die Lizenzlage zieht dorthin, wo sie gilt)

`Lizenzlage.md` ist neu hier und lag vorher unter `INTEGER_LLM/docs/`.
Sie gehört hierher: **G7 verlangt Apache 2.0 oder MIT, und dies ist der
Beleg dafür, welche Varianten das erfüllen.** `lizenzprobe.py` sagt in
seinem eigenen Kopf, dass es Dateien liest und nicht Recht; die
variantenscharfe Bewertung stammt von Menschen und stand bis heute in
einer fremden Komponente.

**Das Verzeichnis `INTEGER_LLM/docs/` ist im selben Zug entfallen**
(Festlegung des Projektinhabers). ⚑ Von seinen drei Dateien waren zwei
überholt, und eine davon auf eine Art, die teuer werden kann: Sie führte
für Entscheidungspunkt 12.21 **θ_v 0.10.0 und +4,29 %**, während
`eval/results/decision_12-21.md` bei jedem Lauf neu geschrieben wird und
θ_v 0.17.0 mit **+2,11 %** ausweist. **Zwei Orte für dieselbe Messung
sind einer zu viel**, und der veraltete gewinnt, sobald jemand ihn
zuerst liest.

⚑ **Und eine Festlegung dreht eine frühere um.** In `Lizenzlage.md`
stand, die Lizenzdatei gehöre zu den Gewichten und nicht ins
Repositorium. Künftig trägt jedes **für den Produktionsbetrieb
empfohlene** Modell sein Verzeichnis samt Lizenzdatei im Repositorium,
die Gewichte bleiben draußen. Der Grund ist der Ablauf beim Miner: Er
holt die Gewichte per Skript direkt von der Quelle und erführe die
Bedingungen sonst erst **danach**. Die vier Lizenzdateien der
katalogisierten Modelle liegen seit heute im Repositorium.

⚑ **Damit prüft `lizenzprobe.py` in der CI erstmals etwas.** Sie fand
dort bisher einen leeren Ordner und ging durch, was sie im eigenen Kopf
als Nicht-Beleg benennt. Jetzt liest sie vier echte Lizenztexte. **Was
sie weiterhin nicht leistet:** Sie liest Dateien, nicht Recht; die
Beurteilung steht in `Lizenzlage.md` und stammt von Menschen.

### v0.3.0 – 2026-08-31 (G9: was das Netz nicht lernt und nicht bedient)

Auf Festlegung des Projektinhabers. `Ausschluss.json` mit fünf Klassen:
Massenvernichtungswaffen, konventionelle Waffen und Sprengstoff,
Angriffswerkzeuge gegen fremde Systeme, Missbrauchsdarstellungen und
Verfolgung von Personen, Täuschung über die Person. Dazu
`ausschlussprobe.py`.

⚑ **Jede Klasse trägt eine Abgrenzung, und ohne sie fällt sie durch.**
„Waffen" als Stichwort verschluckt Geschichte, Chemie, Metallurgie,
Rüstungskontrolle und den halben Journalismus. Eine Klasse ohne
Abgrenzung ist kein Ausschluss, sondern ein Ermessensspielraum, und G1
nennt den Grund, warum es den nicht geben soll: Wer entscheidet, welcher
Text schädlich ist, entscheidet auch, welcher Text unbequem ist. **Der
Maßstab ist Befähigung, nicht Thema.**

⚑ **Der Katalog hebt G1 nicht auf, und er wirkt nicht überall gleich.**
Bei der Korpus-Aufnahme greift er über den Governance-Akt, den G2 ohnehin
dort verortet: Ein Antrag muss zu **jeder** Klasse Werkzeug, Version und
Zahl der ausgeschlossenen Stücke nennen, sonst wird er nicht zur
Abstimmung gestellt. Bei der Abfrage bindet er Betreiber von Gateways
und Clients, **nicht den Konsens**; das Protokoll kann Missbrauch der
Inferenz nicht am Inhalt erkennen, und nach G7 kann das Modell ohnehin
jeder lokal ausführen. **Die Zusage lautet: dieses Netz bedient es nicht
und lernt es nicht. Sie lautet nicht: es ist unmöglich.**

**Der Antrag ist mitgewachsen:** `ausschluss` ist kein freies Textfeld
mehr, sondern eine Zeile je Katalogklasse. Bis dahin konnte ein Antrag
„Dubletten entfernt" eintragen und galt als vollständig, während über
die Klassen, um die es geht, nichts dastand.

⛑ **Und die CI prüfte nur eine Richtung.** Sie verlangte, dass die leere
Vorlage durchfällt; eine Prüfung, die **alles** ablehnt, besteht diesen
Test ebenfalls. Neu ist ein vollständiger Beispielantrag als
Positivprobe, und beide Richtungen laufen im selben Schritt.

### v0.2.0 – 2026-08-29 (Phase 1: aus Zusagen werden Dateien)

Fünf Artefakte, drei davon als Skript im CI:

- **`werkzeuge/modellkarte.py`** erzeugt `Modellkarte.md` aus
  `theta_v/spec.json` und `eval/results/`. ⚑ **Erzeugt, nicht
  geschrieben**, denn eine von Hand gepflegte Karte ist so aktuell wie
  die Erinnerung dessen, der sie zuletzt anfasste. `--pruefe` meldet,
  wenn die abgelegte nicht mehr die erzeugte ist.
- **`vorlagen/korpus-aufnahmeantrag.json`** und
  **`werkzeuge/pruefe_antrag.py`** für die vier Pflichtangaben aus G3.
  Die leere Vorlage fällt durch, und der CI-Schritt prüft genau das.
- **`werkzeuge/lizenzprobe.py`** liest den Lizenz**text**, nicht den
  Dateinamen, und kennt drei Sperrmuster. Vier lokale Modelle, alle
  Apache 2.0.
- **`Risikoklassen.toml`** als **eine** Quelle für Kap. 9.3. ⚑ Eine
  Warnung, die an drei Stellen steht, steht irgendwann in drei
  Fassungen da, und die mildeste wird die gelesene.
- **`checklists/README.md`** sagt bei jedem Punkt dazu, **ob eine
  Maschine ihn prüfen kann**. Drei können es, der Rest nicht.

⚑ **Was keine der Prüfungen leistet, und es steht in jeder von ihnen:**
Sie prüfen Vollständigkeit und Form, nicht Wahrheit. Ob eine
Merkle-Wurzel stimmt oder ein Filter wirklich lief, sieht kein Skript.
Sie sorgen dafür, dass jemand es hingeschrieben hat und dafür einsteht.

### v0.1.0 – 2026-08-18 (bis zum 2026-08-31 als v1.0.0 geführt)
- Komponente angelegt, `Manifest.md` v1.0.0.
- Aufbau bewusst als Grenzziehung: Was ist protokoll-durchsetzbar, was
  governance-abhängig, was nicht kontrollierbar. Erst danach Zusagen.
- **Ein konkreter Fund beim Anlegen:** Whitepaper Kap. 10.1 verlangt für
  das Basismodell Apache 2.0 oder MIT. Das lokal vorliegende
  Qwen2.5-0.5B erfüllt das (`INTEGER_LLM/models/Qwen2.5-0.5B/LICENSE`
  ist Apache 2.0) — die seit 2026-08-12 offene Frage „Lizenzlage des
  Basismodells" ist damit **für diese Variante** beantwortet, aber
  nicht für die Modellfamilie: einzelne Qwen2.5-Größen stehen unter
  abweichenden Lizenzen. Die Prüfung muss variantenscharf erfolgen,
  nicht familienweit. Vermerkt in `ETHICS/Lizenzlage.md` und
  unter Punkt 1.3.
