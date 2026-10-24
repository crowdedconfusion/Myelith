# governance (`myl-governance`)

> **Version:** 0.15.2
> **Datum:** 2026-10-22
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

### v0.15.2 (2026-10-22): Eine Werkzeugkette für alle Kisten

- `rust-version` auf 1.99 in `myl-governance`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  Nur das Manifest, kein Code.
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen.

### v0.15.1 – 2026-10-21 (die Modelle heißen nach Tieren)

Die Projektmodelle heißen seit heute nach Tieren, mit Größe und Speicherform als Anhängsel (Festlegung des Projektinhabers): `myelith-0.6b` ist `sparrow-0.6b-int8`, `myelith-4b` `pigeon-4b-int8`, `myelith-30b-a3b` `magpie-30b-a3b-int8`, `myelith-35b-a3b` `raven-35b-a3b-int8`. In den Dateien eines Artefakts steht kein Name, also bleiben Abdrücke und Register gültig. Nur das Beispielmanifest der Proben.

### v0.15.0 – 2026-09-12 (das Gemisch trägt jetzt die Schranken)

⛔️ **Das dichte 14B ist entfallen** (Festlegung des Projektinhabers),
und damit ging das Modell, an dem die Schranken beider Werkzeuge
hingen. **Sie sind neu gerechnet und nicht nachgezogen.**

| Schranke | vorher (14B) | jetzt (30B-A3B) |
|---|---|---|
| `bytes_je_segment` | 614 400 | **294 912** |
| Eingang je Knoten | 182 GiB | **140 GiB** |
| Spur je Knoten | 6,2 GiB | **13,1 GiB** |
| Layer je Shard | 10 | **12** |

⚑ **Der Bezug ist jetzt das Modell, das voraussichtlich das
Primärmodell wird**, und das ist die richtige Bezugsgrösse für eine
Speicherplanung.

⚑ **Und das Gemisch trägt erstmals eine gemessene Rate** (10,1 Token je
Sekunde, gemessen am 2026-09-11). Es ist schmaler als das abgelöste
dichte Modell (2048 gegen 5120), rechnet aber schneller (10,1 gegen
5,25); die Segmentzahl hängt am Durchsatz, die Segmentgrösse an der
Breite, **und die Breite gewinnt**.

⚠️ **Die Zeile zur Spur trägt jetzt eine grössere Zahl als vorher**
(13,1 statt 6,2 GiB), und die Überschrift der Gegenprobe heisst
entsprechend „unter zwanzig" statt „unter zehn". Der Befund von E10
bleibt derselbe, nämlich dass die Spur den Bedarf um eine
Grössenordnung senkt; die Zahl daneben ist eine andere, weil das Modell
eine andere ist. **Eine Schranke, die nicht mehr stimmt, wird neu
gerechnet und nicht weggelassen.**

### v0.14.1 – 2026-09-11 (die Modelltabelle folgt der neuen Reihe)

⚑ **Qwen2.5-0,5B und Qwen2.5-7B sind durch Qwen3-0,6B und Qwen3-14B
ersetzt** (Festlegung des Projektinhabers). `streitlast` rechnet damit
an anderen Zahlen, und die Schranken der Gegenproben sind **neu
gerechnet** statt angepasst: 182 statt 254 GiB je Knoten, weil das
groessere Modell zwar breiter ist (5120 gegen 3584), aber langsamer
rechnet (5,25 gegen 10,74 Token je Sekunde) und die Segmentzahl am
Durchsatz haengt.

⚠️ **`tok_s` bleibt leer, wo nichts gemessen ist.** Die alten Werte
galten fuer die abgeloesten Modelle; sie zu uebernehmen waere eine
Zahl ohne Messung.

📌 **Nachtrag vom selben Tag: `speicherlast` war uebersehen worden.**
Das zweite Werkzeug dieser Komponente fuehrt eine **eigene**
Modelltabelle, und sie stand noch auf Qwen2.5-0,5B (757 960 KiB, 24
Ebenen) und Qwen2.5-7B (8 512 912 KiB, 28 Ebenen). Die vier Groessen
sind jetzt an den Artefaktverzeichnissen dieses Standes abgezaehlt:
898 782, 4 701 995, 15 957 650 und 30 467 751 KiB. **Zwei Werkzeuge
derselben Komponente fuehrten dieselbe Liste, und nur eines wurde
nachgezogen.**

📌 **Und `streitlast` nannte in einer Ueberschrift ein anderes Modell
als in den Zahlen darunter.** Der Satz „je Pod, Qwen2.5-7B" stand fest
im Rumpf, waehrend `MODELLE[2]` laengst Qwen3-14B war. Der Name kommt
jetzt aus der Tabelle.

### v0.13.1 – 2026-09-10 (die Artefakte heissen nach dem Modell, das sie sind)

**Umbenennung, keine Verhaltensänderung.** Die Artefakte unter
`INTEGER_LLM/artifacts/` heissen seit heute `myelith-0.5b`,
`myelith-7b`, `myelith-4b` und `myelith-30b-a3b`; die Pfade in einem Testaufbau
sind nachgezogen.

⚑ **Ein Artefakt ist nicht das Basismodell, sondern das Modell, mit dem
dieses Projekt rechnet.** Es trägt deshalb einen eigenen Namen; die
Basismodelle unter `models/` behalten ihre und stehen weiter mit
Herkunft im Katalog.

⚑ **Der Konformitätswert ist unverändert**, gemessen nach dem Umbau:
`894d8357ae92b5c1` über sechs Vektoren und `6da384ba301b9454` über
siebzehn. **Die Namen stehen in keiner Bytefolge, die gehasht wird.**

### v0.13.0 – 2026-09-06 (die Trainingsabgabe als gerechneter Parameter)

`TrainingsAbgabeMax` = 3017 Basispunkte. Damit sind es 35 Parameter.

⚑ **Als „gerechnet" geführt und nicht als Entwurf**, und die
Herkunftsprüfung rechnet sie nach: Sie folgt aus Auslastungsziel,
Shard-Anteil, Trainings-Grundrate und Freianteil. **Ändert sich eine
davon, ändert sich die Abgabe**, und eine abgeschriebene Zahl wäre ab
diesem Tag falsch, ohne dass es jemandem auffiele.

Damit steht sie neben `MindestStake` als zweiter Wert, der nicht gesetzt,
sondern hergeleitet ist.

### v0.12.0 – 2026-09-05 (der Freianteil steigt auf 8000)

`TrainingsFreianteil` steht auf 8000 statt 1000 Basispunkten.

⚑ **Die Begründung dreht sich um.** Kap. 7.1 fragt, wie wenig Training
das Netz verträgt. Die andere Frage ist, was leerlaufende Miner sonst
tun sollen, und die Antwort ist: nichts.

**Warum nicht 10000:** Die Auslastung ist die der Vorepoche. Die
fehlenden zwanzig Prozent der freien Kapazität sind Luft für
Nachfragewachstum innerhalb der laufenden Epoche, kein Rundungsrest. Wie
viel Luft nötig ist, ist **nicht gemessen**.

### v0.11.0 – 2026-09-05 (drei Parameter für die Trainingszuteilung)

Neu: `PodKapazitaet`, `TrainingsGrundrate`, `TrainingsFreianteil`.
Damit sind es 34 statt 31.

⚑ **Warum Parameter und nicht Konstanten:** Die Pod-Kapazität hängt vom
Modell und von der zugelassenen Hardware-Klasse ab. Ein grösseres Modell
senkt sie, schnellere Hardware hebt sie. Eine einbetonierte Zahl wäre ab
dem ersten Modellwechsel falsch, ohne dass es jemandem auffiele.

### ⚑ Die Entwurfs-Sperrklinke ist von sieben auf neun gestiegen, und das ist ein Rückschritt

Sie soll fallen, nicht steigen. Zwei der drei neuen Werte hat niemand
beschlossen, und beide sind **ehrlich** Entwürfe:

- `PodKapazitaet` ist eine Grössenordnung. Aus dem Gesamtlauf ist der
  vTFE-Betrag je Segment bekannt (rund 1,0 Mio.), die Segmente je
  Sekunde sind es nicht. **Sie fällt, sobald jemand misst**, und das ist
  eine Messung, kein Beschluss.
- `TrainingsGrundrate` ist eine Abweichung von Kap. 7.1. Sie fällt,
  sobald der Projektinhaber sie entscheidet, oder sie geht auf null und
  Kap. 7.1 gilt wörtlich.

**Beide haben einen benannten Weg heraus.** Ein Entwurf ohne solchen Weg
gehörte nicht hinzugefügt.

### v0.10.0 – 2026-09-04 (⚑ Fund 171: zwei Vorgaben leiten ab statt abzuschreiben)

`Stichprobenrate` und `MindestStake` kommen aus
`myl_tokenomics::STICHPROBE_*` statt aus Literalen.

⚑ **Fund 146 wollte eine kopierte Zahl beseitigen und hat dabei eine
zweite eingeführt.** Der Kommentar über `MindestStake` versprach: „Wer
`p` oder `g` ändert, ändert die Vorgabe mit." Für `g` stimmte das; `p`
stand daneben als Literal `5, 100`, und die Kette lief mit 2/100.

Die Regel dahinter ist die, auf die alle gereiften Systeme kommen: eine
maßgebliche Stelle je Größe, alles andere leitet ab. Was Governance
ändern kann, liegt im Zustand und wird beim Ausführen gelesen; eine
Konstante daneben ist das Antimuster. Der Endzustand
bleibt, dass der Konsens die Registry liest, denn Kap. 10 nennt `p`
ausdrücklich abstimmbar.

### v0.9.0 – 2026-09-03 (die Naht zwischen Sperrfrist und Streitfrist)

⚑ **Drei Zahlen, drei Kisten, und nur GOVERNANCE sieht alle drei:** die
Streitfrist als Parameter, `DEFAULT_DISPUTE_EPOCHS` in `myl-consensus`,
`SPERRFRIST_EPOCHEN` in `myl-ledger`. Keine der drei Kisten kennt die
beiden anderen, denn beide hängen an der Registry und nicht sie an
ihnen. **Der Gleichstands-Test ist die Naht.**

Die Aussage: Ein Einsatz, den man vor dem Urteil abziehen kann, ist
keiner. Die Sperrfrist muss deshalb mindestens so lang sein wie das
Fenster, in dem noch ein Urteil kommen kann, **und nicht länger**, sonst
wäre sie eine Härte ohne Begründung. Der Test verlangt Gleichheit; ein
Abstand müsste begründet werden und stünde dann dort.

**Ältere Einträge** (v0.8.0 und davor, 11 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- GOVERNANCE/README/README.md`.
