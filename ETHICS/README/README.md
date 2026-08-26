# ethics

> **Version:** 1.0.0
> **Datum:** 2026-08-18
> **Status:** Manifest v1.0.0 steht,
> Design-Entscheidungen offen. Keine Phase begonnen.

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

### v1.0.0 – 2026-08-18
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
  nicht familienweit. Vermerkt in `INTEGER_LLM/docs/01_licenses.md` und
  unter Punkt 1.3.
