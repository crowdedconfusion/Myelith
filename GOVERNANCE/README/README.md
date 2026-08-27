# governance (`myl-governance`)

> **Version:** 0.2.1
> **Datum:** 2026-08-27
> **Status:** **Phase 1 abgeschlossen** (Punkte 1.1–1.4): Parameter-Registry
> mit Änderbarkeits-Rang, technische Durchsetzung des Verfassungsrangs,
> Invarianten-Kopplung, seit v0.2.0 auch die Kontrollsegment-Schranke aus
> Fund 58. **38 Tests grün** (22 Akzeptanz, 16 Gleichstand).
>
> **Was fehlt, ist die Abstimmungsmechanik selbst** (Phasen 2 und 3).
> Heute prüft die Registry, ob ein Vorschlag zulässig **wäre**; wer über
> ihn abstimmt und wie ausgezählt wird, ist nicht gebaut.
>
> ⚑ **Fund 50 beim ersten Gleichstands-Test:** Die Streitfrist stand auf
> 7 Epochen mit dem Kommentar „entspricht 7 Tagen". Unter den
> Stunden-Epochen, mit denen der Rest des Projekts rechnet, waren das
> **7 Stunden**, ein Faktor 24 im Anfechtungsfenster.
>
> ⚑ **Fund 49:** Die Self-Dealing-Grenze aus Anhang B.4 lässt sich in
> **zwei je zulässigen Schritten** umgehen. Nicht behoben, als Tatsache
> festgehalten, siehe Changelog.

Genesis-Modellwahl, Modell-Update-Prozess, Parameter-Governance.
Referenzimplementierung von Whitepaper Kap. 10.

## Aufgabe

Anders als die übrigen Komponenten ist dies überwiegend eine **Prozess-,
nicht Code-Komponente**: Anforderungen an das Basismodell und Verantwortung
für die Quantisierung (Kap. 10.1), dreistufiger Modell-Update-Prozess aus
Vorschlag, Shadow-Phase und Abstimmung (Kap. 10.2) sowie die Frage, welche
Parameter änderbar sind und welche Verfassungsrang haben (Kap. 10.3). Der
Code-Anteil ist klein: Abstimmungsmechanik, Parameter-Registry mit
Änderbarkeits-Flags, Shadow-Phase-Automatisierung.

## Abhängigkeiten

CONSENSUS (Abstimmung nutzt Stake × Arbeitshistorie, dieselbe Gewichtung wie
die Validator-Wahl, Kap. 10.2), TOKENOMICS (Parameter wie p, s, κ, γ_train
sind dort implementiert — GOVERNANCE ändert sie, TOKENOMICS führt sie aus).

## Struktur

```
GOVERNANCE/
├── README/                   diese Kurzübersicht
└── myl-governance/
    ├── Cargo.toml
    ├── src/
    │   ├── lib.rs
    │   ├── registry.rs       alle Parameter, je mit Änderbarkeits-Rang (1.1)
    │   ├── vorschlag.rs      Rang, Art, Invarianten — geprüft vor der Abstimmung (1.2)
    │   └── invarianten.rs    die Sicherheitsbedingungen aus Anhang B (1.3)
    ├── examples/
    │   └── schranke_meldung.rs   zeigt die Ablehnungstexte im Klartext
    └── tests/
        ├── akzeptanz.rs      die Akzeptanzkriterien der Phase, wörtlich
        └── gleichstand.rs    Registry gegen die Konstanten der anderen Crates
```

## Wie ein Vorschlag geprüft wird

Drei Schranken, **bevor** abgestimmt wird, in dieser Reihenfolge:

1. **Rang.** Ein Verfassungsrang-Parameter ist kein Gegenstand einer
   Abstimmung (Kap. 10.3). Der Rang hängt am Typ und nicht an einem
   Datensatz: Stünde er als Feld in der Registry, wäre er selbst mit
   einem Vorschlag änderbar, und damit wäre er eine Vereinbarung statt
   einer Schranke.
2. **Art.** Ein Vorschlag, der aus einer Rate einen Schalter macht, ist
   eine Protokolländerung und keine Parameteränderung.
3. **Invarianten**, geprüft am **entstehenden Zustand**. `s < c/(1−c)`
   verbindet zwei Parameter; wer nur den geänderten ansieht, kann die
   Bedingung nicht prüfen.

**Warum vor der Abstimmung:** Ein Parametersatz, der `S_min`
unterschreitet, ist auch dann falsch, wenn eine Mehrheit dafür stimmt.
Käme die Prüfung danach, bliebe nur, das Ergebnis zu verwerfen (was die
Abstimmung entwertet) oder es anzuwenden (was die Invariante entwertet).

## Die Invarianten

| Bedingung | Fundstelle | verhindert |
|---|---|---|
| `S ≥ g/p²` | Kap. 5.5, Anhang B.1 | Betrug wird rational, wenn `p` sinkt oder `g` steigt, ohne dass der Stake folgt |
| `s < c/(1−c)` | Anhang B.4 | Self-Dealing wird in der Subventionsphase profitabel |
| Trainingsvergütung `< 1` | Kap. 5.6 | Miner verlagern Kapazität von der Inferenz aufs Training |
| `0 < α ≤ 1` | strukturell, Fund 47 | ein überkorrigierender EMA-Schritt |
| `r ≥ 2` | Kap. 4.4 | ohne zweiten Pod entfällt Stufe 1 der Verifikation ersatzlos |
| `n ≥ 4` | strukturell | BFT verlangt `n ≥ 3f+1` mit `f ≥ 1` |
| `0 < c ≤ 0,8` | Anhang B.4 | eine beliebig hohe Self-Dealing-Grenze |
| `Vorrat ≥ ⌈Fenster · γ⌉` | Kap. 6.7, Fund 58 | ein Kontrollsegment-Vorrat, den ein Miner mit Gedächtnis vollständig erkennt |
| Raten in `[0,1]`, Nenner ≠ 0, `k ≥ 1` | strukturell | undefinierte Rechnungen |

**Jede Invariante hat eine Fundstelle**, und die steht im Code. Eine
Schranke, die niemand entschieden hat, ist in einem Governance-Modul kein
Schutz, sondern eine heimliche Festlegung: Sie verhindert Werte, über die
eine Abstimmung befinden dürfte, und niemand könnte sagen, warum.

## Tests

**22 Akzeptanztests + 13 Gleichstandstests.** Die beiden
Akzeptanzkriterien der Phase stehen wörtlich als Test. Drei Gegenproben
stehen davor: Der Vorgabesatz erfüllt seine eigenen Bedingungen, ein
sinnvoller Vorschlag geht durch, und jeder Parameter hat einen Wert.

*(Hier stand bis zum 2026-08-27 „14 Akzeptanztests + 7
Gleichstandstests" und weiter unten „14 162 angenommen". Beide Zahlen
waren der Stand von v0.1.0; die Kopfzeile daneben führte längst andere.
Zwei Zählungen derselben Sache in einer Datei, und keine davon gegen den
Lauf geprüft.)*

Die Eigenschaft, auf die alles hinausläuft, steht als Property-Test:
**Kein angenommener Vorschlag führt je zu einem Zustand, der eine
Invariante verletzt** — über 50 000 zufällige Vorschläge bis an die Ränder
des Zahlbereichs, von denen 15 826 angenommen werden. Die Zahl steht in
der Zusicherung: Würde nichts angenommen, wäre die Aussage darüber leer.

Beide Prüfungen sind gegen entfernte Prüfschritte geeicht und fliegen auf.

### Der wichtigste Test ist `gleichstand.rs`

Ohne ihn wäre die Registry das gefährlichste Artefakt im Repositorium. Sie
behauptet, die maßgebliche Liste zu sein, während gerechnet wird mit den
Konstanten in `myl-tokenomics` und `myl-consensus`: zwei Orte für
denselben Wert, und einer davon wird gelesen. Genau dieses Muster hat das
Projekt dreimal bezahlt (A7, Fund 25, Fund 44), und jedes Mal war die
richtige Fassung vorhanden und lief nicht.

Er hat sich beim ersten Lauf bezahlt gemacht, siehe Fund 50.

## Changelog

### v0.2.1 – 2026-08-27 (die drei Zehn-Epochen-Fenster, Blöcke je Epoche)

Kein Bibliotheksbau, zwei Tests. Mit dem Verstoß-Zähler im Ledger gibt
es **drei** Konstanten von zehn Epochen, und sie liegen in drei Crates:
die Aufbewahrung der Verstoßhistorie (`myl-ledger`), das
Staffelungsfenster der Slashing-Matrix (`myl-tokenomics`, seither
dieselbe Konstante) und die Arbeitshistorie des Stimmgewichts
(`myl-consensus`).

**Die dritte ist absichtlich gleich, aber nicht gekoppelt:** Alle drei
beantworten dieselbe Frage — wie lange das Verhalten eines Teilnehmers
nachwirkt —, und zwei verschiedene Antworten darauf wären schwer zu
begründen. Sie stehen trotzdem getrennt, damit eine spätere Entscheidung
sie auseinanderziehen darf. Der Test macht daraus eine Entscheidung
statt eines Versehens.

**Warum hier:** `myl-tokenomics` kennt `myl-consensus` nicht und
umgekehrt. Diese Komponente kennt beide; sie ist der einzige Ort, an dem
die drei nebeneinanderliegen. Genau dafür gibt es `gleichstand.rs`.

**Dazu ein vierter Gleichstand:** `myl_consensus::BLOECKE_JE_EPOCHE`
gegen `Epochenlaenge` und `Blockzeit` (3600 s / 2 s = 1800). Die
Konstante ordnet jede Blockhöhe einer Epoche zu und geht damit in die
**Blockprüfung** ein; sie steht deshalb dort und nicht als Abfrage
dieser Registry — eine Blockprüfung, die einen abstimmbaren Wert liest,
macht die Gültigkeit eines Blocks von einem Zustand abhängig, der sich
ändern kann, während der Block schon in der Kette steht. Der Test ist
die Verbindung, dieselbe Bauart wie bei der Streitfrist.

### v0.2.0 – 2026-08-27 (Punkt 1.4: die Kontrollsegment-Schranke, ⚑ Fund 58)

**Zwei Parameter und eine Invariante**, aus einer Messung in
VERIFICATION. Der Vorrat an Kontrollsegmenten ist endlich, der
Auftragsstrom nicht; wird öfter eingeschleust, als der Vorrat
verschiedene Segmente hält, wiederholen sich Segment-Ids. **Echte Arbeit
wiederholt sich nie**, also ist jedes zweite Auftreten kein Verdacht,
sondern ein Beweis. Gemessen bei γ = 2 % über 100 000 Aufträge: Vorrat 64
→ **96,8 % erkannt, null Fehlalarme**. Der Angreifer geht kein Risiko
ein, er rechnet die erkannten Kontrollen ehrlich und manipuliert den
Rest.

Neu: `Kontrollsegmentvorrat` und `Kontrollsegmentfenster`, dazu
`VorratTraegtEinschleusung` mit `Vorrat ≥ ⌈Fenster · γ⌉`. **Die Bauart
ist die von `S_min`:** Der Wert ist abstimmbar, das Unterschreiten der
Schranke nicht.

⚑ **Der gefährliche Vorschlag sieht aus wie eine Verschärfung.** γ von 2
auf 4 Prozent zu heben, ohne den Vorrat mitzuziehen, verdoppelt die
Einschleusungen bei gleichem Vorrat und halbiert die Reichweite. Vorher
wäre er zulässig gewesen. Er steht als Test, samt Gegenprobe: mit
verdoppeltem Vorrat geht derselbe Zug durch.

**Das Fenster ist ein Parameter, kein Hilfswert.** Ohne es ist die
Bedingung nicht entscheidbar — ein unbegrenzter Auftragsstrom erschöpft
jeden endlichen Vorrat. Wer das Fenster senkt, senkt die Schranke.

**Die Formel steht in VERIFICATION und wird hier benutzt**, nicht
wiederholt (`myl_verifier::noetiger_vorrat`), dieselbe Arbeitsteilung wie
bei `s_min`. `gleichstand.rs` prüft über 800 Vorratsgrößen, dass beide
Seiten dieselbe Grenze ziehen, und bindet die Vorgabewerte an die
Konstanten dort.

**Der Anfangswert ist vorläufig, und das steht dabei:** 2 048 ist der
gemessene Wert, 100 000 die Stromlänge der Messung. Was fehlt, ist die
Auftragsrate des Netzes — erst mit ihr ließe sich das Fenster in
Sekunden ausdrücken.

**Neu daneben: `examples/schranke_meldung.rs`** gibt die drei
Ablehnungstexte im Klartext aus. Eine Fehlermeldung wird im Fehlerfall
als Einzige gelesen und fällt beim Lesen der Dokumentation nicht auf; wer
die Schranke ändert, sieht in einem Aufruf, was ein Antragsteller danach
zu lesen bekommt. Die Meldung nennt jeweils den Vorrat, das Fenster, γ
und die nötige Zahl — genug, um den Vorschlag zu reparieren statt ihn nur
abgewiesen zu bekommen.

**Nebenbei richtiggestellt:** Der Testabschnitt oben führte seit v0.1.0
Zahlen, die die Kopfzeile derselben Datei längst überholt hatte.

### v0.1.0 – 2026-08-24 (Phase 1: Parameter-Registry)

Die erste Zeile Code dieser Komponente. Anlass waren drei Funde des
Vortags, die alle hierher zeigten: Fund 46 und 47 in TOKENOMICS endeten
mit dem Satz „die Prüfung gehört in die Governance-Schicht", und die
offene Frage nach einer Untergrenze für den Credit-Preis ist ebenfalls
eine Parameterfrage. **Ein Crate, das Grenzen prüft, ist die fehlende
Gegenseite zu drei Crates, die Grenzen einhalten.**

#### ⚑ Fund 50: Die Streitfrist war 7 Stunden statt 7 Tagen

`myl_consensus::epoch_close::DEFAULT_DISPUTE_EPOCHS` stand auf `7`, mit
dem Kommentar „Entspricht der Design-Entscheidung ‚7 Tage' bei 2 s
Blockzeit und einer Epochenlänge, die GOVERNANCE festlegt."

Der Satz nennt die Epochenlänge als offen und rechnet zugleich mit ihr:
`7 Epochen = 7 Tage` gilt nur bei **Tages**-Epochen. Der Rest des Projekts
rechnet mit **Stunden**-Epochen — Anhang B.1 („Bei Stunden-Epochen: etwa
ein Tag Einkommen als Pfand") und die Stimmgewichts-Kalibrierung vom
2026-08-23 („Faktor nach einer Stunden-Epoche"). **Ein Faktor 24.**

Die Streitfrist ist die Zeit, in der ein Betrug angefochten werden kann
und in der der `DaStore` die Fragmente vorhalten muss. Sieben Stunden sind
die Zeit, die ein Checker hat, um eine Abweichung zu bemerken, das
Bisektionsspiel zu führen und die Schiedsrunde zu erreichen.

**Ursache war ein fehlender Parameter:** Die Epochenlänge steht in keinem
Kapitel und in keiner Design-Entscheidung, wird aber überall gebraucht,
sobald eine Frist „je Epoche" gilt. Zwei Teile des Projekts haben sie
deshalb stillschweigend verschieden angenommen. Sie ist jetzt ein
Registry-Parameter, und der Gleichstands-Test verbindet Frist,
Epochenlänge und Konstante.

**Korrigiert auf 168 Epochen**, der Entscheidung von 2026-08-13 folgend.
**Die Kosten gehören genannt:** Die Vorhaltung im `DaStore` dauert damit
24-mal so lange. Ob 7 Tage der richtige Wert sind, ist eine Abwägung
zwischen Speicherkosten und Anfechtungsfenster und ist ein offener
Punkt.

Nebenbei: Vier Tests im `DaStore` und einer im Epochenabschluss prüften
die Frist gegen **getippte Zahlen** statt gegen die Konstante und schlugen
bei der Korrektur fehl, ohne dass an der geprüften Regel etwas falsch
gewesen wäre. Nachgezogen.

#### ⚑ Fund 49: Die Self-Dealing-Grenze ist in zwei Schritten zu umgehen

Anhang B.4 verlangt `s < c/(1−c)` und nennt `c` „empirisch 0,6–0,8". `c`
ist damit **keine Protokollgröße, sondern eine Beobachtung über die
Welt** — die realen Hardware- und Stromkosten als Anteil am Reward. Ohne
einen Wert für `c` ist die Ungleichung nicht auswertbar, also steht er in
der Registry.

Damit entsteht eine Lücke, die keine einzelne Prüfung schließt: Ein
Angreifer hebt zuerst `c` (für sich zulässig, die Ungleichung bleibt
erfüllt) und danach `s` unter die neue, höhere Grenze. **Beide Vorschläge
bestehen die Prüfung, das Ergebnis verletzt die Bedingung**, denn das
wahre `c` hat sich nicht bewegt.

Eine Obergrenze für `c` (0,8, das obere Ende des Bandes) begrenzt den
Schaden auf `s < 4`, **schließt die Lücke aber nicht**. Was sie schlösse,
ist eine Entscheidung, die dieses Modul nicht treffen kann: `c` gehört
gemessen statt abgestimmt, oder `s` gehört gegen das **untere** Ende des
Bandes geprüft (c = 0,6 ⇒ s < 1,5). Offen; als Tatsache festgehalten in
`tests/akzeptanz.rs`.

#### ⚑ Fund 48: „Gesamtangebot" ist ein Verfassungsrang ohne Gegenstand

Kap. 10.3 nennt drei nicht änderbare Festlegungen: Gesamtangebot,
Burn-and-Mint-Prinzip, Determinismus-Pflicht. Die ersten beiden vertragen
sich nicht: **Burn-and-Mint hat kein Gesamtangebot.** Der Umlauf ergibt
sich aus Prägung minus Verbrennung, und Anhang B.8.3 rechnet ausdrücklich
durch, was ein Emissionsdeckel bewirkt, mit dem Ergebnis „Ein Deckel wirkt
damit nicht als Knappheitsgarantie, sondern als Kapazitätsbremse". Der
einzige Deckel des Protokolls ist `M_max` je Epoche, und der steht im
dieser Komponente als **änderbarer** Parameter geführt.

Umgesetzt als die einzige Lesart, die etwas Durchsetzbares ergibt: „Es
gibt keine andere Quelle von MYL als die Prägung gegen verifizierte
Arbeit." Die endgültige Entscheidung gehört dem Projektinhaber.

Noch keine Version veröffentlicht.
