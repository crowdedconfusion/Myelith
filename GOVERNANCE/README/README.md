# governance (`myl-governance`)

> **Version:** 0.1.0
> **Datum:** 2026-08-26
> **Status:** **Phase 1 abgeschlossen** (Punkte 1.1–1.3): Parameter-Registry
> mit Änderbarkeits-Rang, technische Durchsetzung des Verfassungsrangs,
> Invarianten-Kopplung. **28 Tests grün** (17 Akzeptanz, 11 Gleichstand).
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
| Raten in `[0,1]`, Nenner ≠ 0, `k ≥ 1` | strukturell | undefinierte Rechnungen |

**Jede Invariante hat eine Fundstelle**, und die steht im Code. Eine
Schranke, die niemand entschieden hat, ist in einem Governance-Modul kein
Schutz, sondern eine heimliche Festlegung: Sie verhindert Werte, über die
eine Abstimmung befinden dürfte, und niemand könnte sagen, warum.

## Tests

**14 Akzeptanztests + 7 Gleichstandstests.** Die beiden Akzeptanzkriterien
der Phase stehen wörtlich als Test. Drei Gegenproben stehen davor: Der
Vorgabesatz erfüllt seine eigenen Bedingungen, ein sinnvoller Vorschlag
geht durch, und jeder Parameter hat einen Wert.

Die Eigenschaft, auf die alles hinausläuft, steht als Property-Test:
**Kein angenommener Vorschlag führt je zu einem Zustand, der eine
Invariante verletzt** — über 50 000 zufällige Vorschläge bis an die Ränder
des Zahlbereichs, von denen 14 162 angenommen werden. Die Zahl steht in
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
