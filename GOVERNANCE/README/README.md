# governance (`myl-governance`)

> **Version:** 0.5.2
> **Datum:** 2026-08-28
> **Status:** **Phasen 1 und 2 abgeschlossen** (1.1–1.4, 2.1–2.3),
> Phase 3 zur Hälfte (3.1 und 3.4 ✅). Parameter-Registry mit
> Änderbarkeits-Rang, technische Durchsetzung des Verfassungsrangs,
> Invarianten-Kopplung, die Kontrollsegment-Schranke aus Fund 58, die
> Abstimmungsmechanik und das Modellmanifest.
> **87 Tests grün** (37 Modultests, 6 Streitlast, 26 Akzeptanz, 18 Gleichstand).
>
> **Was fehlt:** die Punkte 3.2 (Shadow-Phase) und 3.3 (koordinierter
> Rollout). Beide brauchen laufende Pods, die zwei Modellversionen
> gleichzeitig fahren; das ist Arbeit in COMPUTE_PIPELINE und
> INTEGER_LLM und ohne Betrieb nicht messbar. Ein Häkchen dafür wäre
> eine Behauptung.
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

### v0.5.2 – 2026-08-30 (die dritte Stufe: 2 bis 8 GiB)

Die Rechnung kennt jetzt drei Stufen je Shard und Segment: jede
Layer-Ausgabe (73 KiB), nur der Eingang (10 KiB), nur die Spur (336 B).

⚑ **Der Sprung zur dritten Stufe kommt nicht vom Sparen, sondern von der
Beweislast.** Die Bisektion endet an der ersten Abweichung, also sind
sich beide Seiten bei `j-1` einig, und der Ankläger hat den strittigen
Wert ohnehin. Er bringt ihn mit; der Angeklagte wird gar nicht mehr
gefragt (E10, VERIFICATION v0.13.0).

**Aus 455 GiB bis 1,8 TiB je Knoten werden 2 bis 8 GiB.** Und die Spur
ist kein zusätzlicher Speicher: Sie ist der Arbeitsnachweis, den es
ohnehin gibt.

### v0.5.1 – 2026-08-29 (die eigene Zahl berichtigt: Faktor 7, nicht 224)

Die Rechnung nannte „nur die Spur, Faktor 224" und setzte dabei voraus,
dass ein Shard aus den Token nachrechnen kann. ⚑ **Er kann es nicht.**
Er hält `layer_start..layer_end` und sonst nichts; die vorderen Layer
liegen bei anderen Shards. Aufgefallen ist es beim Umsetzen, nicht beim
Rechnen: Die Annahme stand nie da, sie war nur nicht nachgeprüft.

Was ein Shard **allein** nachrechnen kann, beginnt bei seiner
eingehenden Aktivierung, und die muss bleiben. Der Faktor ist damit die
Zahl der Layer je Shard, also 7 bei 28 Layern auf vier Shards: Aus
455 GiB bis 1,8 TiB je Knoten werden **65 bis 260 GiB**, nicht 8,1 GiB.

Das Programm rechnet die neuen Zahlen, zwei Tests halten sie fest, und
die Herleitung steht bei der Funktion. Der nächste Hebel wäre
gemeinsames Nachrechnen im Pod (dann hielte nur Shard 0 die Token, rund
99 MiB), zum Preis, dass die Antwort eines Angeklagten an seinen
Nachbarn hinge.

### v0.5.0 – 2026-08-29 (die Speicherlast der Streitfrist, gerechnet statt geschätzt)

### Die Abwägung war eine halbe

Die Streitfrist steht seit dem 2026-08-13 auf sieben Tagen, und die
Begründung ist gut: Sieben Stunden sind knapp, wenn ein Checker nicht
rund um die Uhr läuft, und ein Angreifer legte seine Segmente in die
Nacht des Zielmarktes. Was fehlte, war die andere Seite: **Was kostet
das an Speicher?** Solange die Zahl niemand kennt, ist „sieben Tage sind
es wert" keine Abwägung, sondern eine Behauptung. Dasselbe galt für den
Gegenvorschlag einer zweistufigen Frist.

`cargo run --bin streitlast` rechnet es aus. Archiviert wird je Paar
(Segment, Layer) die Ausgabe-Aktivierung als `i16`, erasure-codiert; ein
Segment ist ein Vorwärtspass, also eine Token-Position. Weder Gewichte
noch KV-Cache gehen ein: Die Gewichte hat jeder Shard ohnehin, der
KV-Cache ist Betriebszustand und kein Beweismittel.

| Modell | je Segment | je Pod, 168 Epochen | je Knoten |
|---|---|---|---|
| Qwen2.5-0,5B | 63 KiB | 1,7 TiB | 0,4 bis 1,7 TiB |
| Qwen2.5-7B | 294 KiB | 1,8 TiB | 0,4 bis 1,8 TiB |

### Der Befund: zu viel für eine gewöhnliche Maschine

**Ein Knoten trägt zwischen 0,4 und 1,8 TiB, allein für das
Beweisarchiv**, und zwar zusätzlich zur Modellgröße. Für ein
Rechenzentrum ist das nichts. Für dieses Netz ist es der falsche
Maßstab: Wer niedrigschwellige Teilhabe will, kann nicht 455 GiB
verlangen, bevor überhaupt gerechnet wird.

### ⚑ Der starke Hebel ist nicht die Frist, sondern das Nachrechnen

Die Frist zu kürzen wirkt linear und kostet die Begründung, aus der die
sieben Tage stammen. Der eigentliche Hebel liegt woanders: **Das Archiv
hält die Aktivierungen vor, und die sind ableitbar.**

Bei bitgenauer Ganzzahl-Inferenz ist jeder Vorwärtspass exakt
nachrechenbar. Derselbe Eingang ergibt stets denselben Ausgang, also
muss nur der Eingang bleiben.

⚑ **Hier stand zuerst „nur die Spur, Faktor 224", und das war falsch.**
Es setzte voraus, dass ein Shard aus den Token nachrechnen kann. Er kann
es nicht: Er hält `layer_start..layer_end` und sonst nichts, die
vorderen Layer liegen bei anderen. Was er **allein** nachrechnen kann,
beginnt bei seiner **eingehenden** Aktivierung, und die muss bleiben.

| je Segment und Shard | | je Knoten, 168 Epochen |
|---|---|---|
| jede Layer-Ausgabe (vorher) | 73 KiB | 455 GiB bis 1,8 TiB |
| nur der Eingang, Rest nachgerechnet | 10 KiB | **65 bis 260 GiB** |

**Faktor 7**, nämlich die Zahl der Layer je Shard. Umgesetzt in
COMPUTE_PIPELINE v0.13.0.

**Warum nicht noch weniger.** Rechnete der Pod **gemeinsam** nach, müsste
nur Shard 0 die Token halten, rund 99 MiB über die ganze Frist. Dann
hinge die Antwort eines Angeklagten aber an der Mitwirkung seiner
Nachbarn, und „Schweigen heißt Schuld" träfe den, dessen Nachbar
schweigt. Die eingehende Aktivierung ist die Grenze dessen, was ein
Shard eigenständig beantworten kann, und diese Eigenständigkeit ist die
Voraussetzung dafür, dass die Frist fair ist. Der gemeinsame Weg bleibt
als eigener Punkt.

⚑ **Was dabei verloren geht, und das ist der eigentliche Streitpunkt:**
Heute liegen die Fragmente erasure-codiert im Pod, die Gegenseite bekommt
die Daten also auch dann, wenn der Angeklagte gerade nicht erreichbar
ist. Rechnet nur er nach, heißt Schweigen Schuld, und ein ehrlicher
Knoten mit einem Ausfall verliert seinen Stake. Ob das hinnehmbar ist,
hängt an der Antwortfrist: Bei sieben Tagen ist ein Knoten, der die
ganze Zeit stumm bleibt, ohnehin kein laufender Knoten. Die Schiedsrunde
hat heute keine fest verdrahtete Frist, sie wäre also zu setzen.

### Die Entscheidung: nachrechnen

⚑ **Der Vorteil des Aufbewahrens besteht nicht.** Er setzt voraus, dass
die Fragmente im Pod verteilt liegen. Nachgeprüft: Der Speicher ist ein
lokales Verzeichnis im Shard, `put` legt alle zwölf Fragmente dort ab,
und es gibt **kein Gossip-Topic für Fragmente**. Ein Fragment verlässt
seinen Knoten nie. Ist er erreichbar, kann er ebenso gut nachrechnen;
ist er weg, versagen beide Wege gleich. Die Erasure-Kodierung kostet in
dieser Verdrahtung das Anderthalbfache und trägt gegen lokale
Beschädigung, nicht gegen Unerreichbarkeit.

**Und das Nachrechnen macht die verteilte Datenverfügbarkeit erst
baubar:** 8,1 GiB je Pod lassen sich verteilen, 455 GiB nicht. Es ist
kein Rückzug von diesem Entwurf, sondern seine Voraussetzung.

**Was es kostet, und das gehört zur Entscheidung:** Das Nachrechnen geht
über den ganzen Vorlauf, denn eine Position hängt über den KV-Cache an
allen vorherigen. Die Antwortfrist der Schiedsrunde gab es dafür nicht;
sie ist seitdem gesetzt (VERIFICATION v0.12.0) und beträgt eine Epoche,
also das 18-fache der Referenzrechnung. Und Schweigen heißt weiterhin
Schuld, auch für einen ehrlichen Knoten mit einem Ausfall; bei sieben
Tagen Frist ist ein durchgehend stummer Knoten allerdings ohnehin kein
laufender Knoten.

**Die zweistufige Frist ist verworfen:** Sie bräuchte mehr Mechanik und
brächte weniger als das Nachrechnen.

⚑ **Die Spanne ist keine Ungenauigkeit, sondern eine Annahme, die
offengelegt gehört.** Der gemessene Durchsatz stammt von einem Knoten
mit dem ganzen Modell. In einem Pod hält jeder Shard ein Viertel der
Layer, die Stufen laufen überlappend, und der Pod schafft im besten Fall
das Vierfache. Dann vervierfacht sich die Zahl der Segmente, und die
Ersparnis aus der Aufteilung ist wieder aufgezehrt. Ohne diese Schranke
läse sich die untere Zahl wie eine Zusage.

**Was die Rechnung nicht sagt**, und das steht auch so im Programm: was
die Bandbreite kostet, wenn ein Angeklagter sein Archiv ausliefern muss,
und was passiert, wenn ein Knoten mehrere Pods gleichzeitig bedient.
Beides ist eine eigene Rechnung.

### Herkunft der Zahlen

Frist aus `myl_consensus::DEFAULT_DISPUTE_EPOCHS`, Erasure-Parameter aus
`myl_types::erasure`, beide **benutzt statt wiederholt**: Als sich die
Frist mit Fund 50 von 7 auf 168 Epochen korrigierte, hätte eine getippte
Zahl still weitergerechnet. Ein Test hält beides gegen die Konstanten.

Die Modellmaße lassen sich nicht so holen, sie stehen unter
`INTEGER_LLM/artifacts/`, und diese Komponente darf dort nicht anhängen.
Die unvermeidliche Verdopplung ist deshalb bewacht:
`tests/audit/test_streitlast.py` liest beide Seiten und vergleicht sie,
zwei Gegenproben, in der CI.

### v0.4.0 – 2026-08-28 (der Parameter `Signaturstufe`)

**Der Schalter für den Wechsel des Signaturverfahrens steht als
Parameter in der Registry** (33 statt 32), Vorgabewert „nur klassisch".

⚑ **Drei Stufen, nicht zwei, und die Folge ist einbahnig.** Ein Sprung
von „nur klassisch" auf „nur quantensicher" machte jeden Validator
ungültig, der seinen zweiten Schlüssel noch nicht veröffentlicht hat,
und hielte damit die Kette an. Ein Rückschritt öffnete das gebrochene
Verfahren wieder, und zwar genau dann, wenn jemand es gebrochen hat:
**Der Rückweg wäre der Angriff.** Erlaubt ist deshalb genau ein Schritt
nach vorn.

```text
NurKlassisch  →  Beide  →  NurQuantensicher
```

**Das Fenster `Beide` ist die verwundbarste Stellung**, und das ist
unvermeidlich. Es gehört so kurz wie möglich gehalten, und der Schritt
danach hängt **nicht an einer Frist, sondern an einer Bedingung**: dass
alle Validatoren bereit sind.

⚑ **Die Reihenfolgeregel ist keine Invariante, und das ist Absicht.**
Alle drei Stufen sind **gültige Zustände**; verboten ist nicht die
Stellung, sondern der Weg dorthin. Eine Invariante prüft einen Zustand
und könnte das gar nicht sehen. Die Prüfung sitzt deshalb in
`pruefe_vorschlag`, wo beide Seiten des Übergangs vorliegen, und hat
einen eigenen Fehlerfall statt sich unter die Invarianten zu mischen.

⚑ **Und eine zweite Bedingung prüft die Registry ausdrücklich nicht:**
ob alle Validatoren bereit sind. Die Registry kennt Parameter und keine
Validatoren; die Prüfung liegt in CONSENSUS
(`validator::alle_bereit_fuer`). Das ist dieselbe Trennung wie beim
Stimmgewicht, das ebenfalls von dort kommt.

**Sieben Gegenproben**, darunter: Der Sprung über das Fenster wird
abgelehnt, alle drei Rückwege werden abgelehnt, Stillstand gilt nicht
als Übergang, und ein anderer Parameter läuft nicht versehentlich durch
diese Prüfung.

### v0.3.0 – 2026-08-28 (Phase 2 und die Hälfte von Phase 3)

**Die Abstimmungsmechanik** (`src/abstimmung.rs`, Punkte 2.1 bis 2.3)
und **das Modellmanifest** (`src/modell.rs`, Punkte 3.1 und 3.4).

Kap. 10.2 legt das Stimmgewicht fest und schweigt zum Verfahren. Das
war der Blocker für Punkt 2.2, und er ist keiner mehr: Quorum,
Mehrheitsschwelle und Abstimmungsfenster stehen als
Governance-Parameter. Die Mechanik ist gebaut, die Zahlen bleiben
entscheidbar, und wer sie später festlegt, ändert keine Zeile Code.

⚑ **Der Parameter, der über sich selbst abstimmt.**
`Abstimmungsmehrheit` ist änderbar und entscheidet über Änderungen.
Ohne Untergrenze genügten **zwei** Abstimmungen, um die Governance
einer Minderheit zu übergeben: erst die Schwelle auf null, dann alles
Übrige, und die zweite Abstimmung bräuchte keine Mehrheit mehr, weil
die erste sie abgeschafft hat. Die Invariante `AbstimmungBleibtBindend`
hält deshalb drei strukturelle Untergrenzen: Mehrheit mindestens 500
Promille, Quorum mindestens 1 Promille, Fenster mindestens 1 Epoche.
**Welche Werte richtig sind, prüft sie ausdrücklich nicht** — das ist
Politik, und Politik gehört nicht in eine Invariante.

Derselbe Gedanke wie der Verfassungsrang, eine Stufe tiefer: Dort ist
ein Parameter gar nicht änderbar, hier ist er änderbar, aber nicht bis
zur Wirkungslosigkeit.

**Die Formel wird gerufen, nicht abgeschrieben.** `abstimmung::gewicht`
ruft `myl_consensus::calculate_voting_weight_mit`; ein Gleichstandstest
hält zusätzlich Arbeitsbezug und Höchstfaktor mit CONSENSUS zusammen.
Ohne ihn könnten die Eingaben auseinanderlaufen, ohne dass ein einziger
Aufruf falsch aussieht.

**Drei Entwurfsentscheidungen, jede mit einem Test:** Das Gewicht steht
bei der **Eröffnung** fest, nicht bei der Auszählung, sonst verschöbe
die zerfallende Arbeitshistorie jedes Gewicht während des Fensters. Das
Quorum misst gegen **alle** Berechtigten, nicht gegen die abgegebenen
Stimmen. Enthaltungen zählen zum Quorum, **nicht** zur Mehrheit: Wer
sich enthält, nimmt teil und stimmt nicht zu.

⚑ **Der angenommene Vorschlag wird beim Anwenden ein zweites Mal
geprüft.** Bei der Eröffnung galt die Registry von damals. Zwei
Vorschläge, jeder für sich zulässig, können zusammen eine Invariante
brechen; der Fall steht als Test da, mit den Kontrollsegment-Parametern
aus Fund 58. Die zweite Prüfung entscheidet die Reihenfolge.

**Das Modellmanifest ist ein Rezept, keine Beschreibung.** Kap. 10.1
verlangt es wörtlich: „sodass jeder Teilnehmer die Ableitung
nachvollziehen kann". Herkunft (Quelle, festgenagelte Revision, Lizenz,
Gewichts-Digest), Ableitung (Werkzeug, Werkzeugversion,
Kalibrierdaten-Digest, θ_v), und der Digest, den der Nachbau haben
muss. Genesis und Update benutzen dieselbe Struktur; ein Update, das
weniger nachweisen müsste, wäre der bequeme Weg an der Anforderung
vorbei.

⚑ **Woran ein Rezept still scheitert:** an einer fehlenden Revision.
Ein Modellname ohne Commit heißt „was gerade dort liegt". Ein Manifest
mit leerem Feld sieht vollständig aus und ist es nicht; der Fehler
fällt erst auf, wenn jemand Jahre später nachbaut und einen anderen
Digest bekommt.

⚑ **Fund 74: Die Kernel-Whitelist war nicht befüllbar.**
`Wert::Hashmenge(BTreeSet<Hash>)` steht seit Punkt 1.1 in der Registry,
mit Vorgabewert „leere Menge, bis zum Genesis-Manifest". Genau bis
dahin fiel nichts auf: Ein leeres `BTreeSet` braucht kein `Ord`, ein
`insert` schon, und `myl_types::Hash` hatte keines. Der Parameter stand
mit Typ, Vorgabewert und Dokumentation da und ließ sich nicht füllen.
**Der Kommentar am Vorgabewert nannte den Schritt, an dem es brechen
würde**, und niemand hat nachgesehen, ob es dann geht. Behoben in
`myl-types` v0.5.0.

**Gemessen:** 74 Tests grün (30 Modultests, 26 Akzeptanz, 18
Gleichstand). Einundzwanzig Gegenproben: für jede Zusage die zugehörige
Zeile gebrochen, jedes Mal rot beim Test mit der Eigenschaft im Namen.

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
