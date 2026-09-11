# Konzept: das Expertengemisch trainieren

> **Stand:** 2026-09-11 · **Grundlage:** `Konzept-Wachstum.md` (das
> Allgemeine), `Geshardetes-Training.md` (die Zuteilung und der
> Shardweg), der ganzzahlige Rueckwaertspass durch das Gemisch
> (`kernels` 0.25.0 bis 0.27.0), die Routersonde vom 2026-09-05 und die
> Expertensonde vom 2026-09-11.
>
> ⚠️ **Dieses Dokument entwirft wenig und ordnet viel.** Der Anlass war
> die Entscheidung des Projektinhabers vom 2026-09-11, dass
> Qwen3-30B-A3B und nicht das dichte 27B-Modell die erste Netzinferenz
> traegt. Beim Nachsehen stellte sich heraus: **Das meiste, was ein
> Gemisch zusaetzlich braucht, ist gebaut.** Was fehlt, sind Zahlen fuer
> dieses Modell und drei Entscheidungen.

## 0. ⛑ Was zuerst zu sagen ist: drei Entwuerfe waren schon Code

Der erste Durchgang dieses Dokuments entwarf eine Routerschranke, einen
ganzzahligen Ausgleichsterm und einen Operator fuer Expertenwachstum.
**Alle drei stehen seit dem 2026-08-28 in `kernels`**, und alle drei
sind besser geloest als der Entwurf:

| Entworfen | Gebaut | Warum das Gebaute besser ist |
|---|---|---|
| Logitgrenze `L_max` mit Straight-Through | `backward::router_spreizung` | Liest **Logit-Abstaende** statt quantisierter Gewichte und hat ihren groessten Wert genau dort, wo der Softmax-Gradient verschwunden ist. Der Vorwaertspfad bleibt unberuehrt, θ_v bleibt, wo es ist. |
| Hilfsverlust `α · N · Σ f_i · P_i` | `moe::Expertenwacht` | Zaehlt ueber die **Segmentfolge** statt ueber den Batch. Die Batchzusammensetzung waehlt der Miner, die Segmentfolge legt das Protokoll fest. |
| Experten an der verborgenen Achse halbieren, `k → 2k` | `moe::experte_einhaengen` | Gibt dem neuen Experten ein Logit **unter allen anderen**. Exakt funktionserhaltend per Konstruktion, ohne Rundungsargument. |

⛑ **Die Lehre ist die von Fund 328 und Fund 329, ein drittes Mal:**
Wer einen Entwurf schreibt, sagt vorher, wo er nachgesehen hat. Hier
wurde erst nach dem Schreiben in `kernels/src/moe.rs` gesehen.

⛔️ **Und eine Aussage war schlicht falsch.** Der erste Durchgang
schrieb, den absorbierenden Zustand des Routers gebe es „in Gleitkomma
nicht". Genau dieser Satz ist am 2026-08-28 schon einmal
zurueckgenommen worden: Gleitkomma hat denselben Zustand, nur spaeter. Die
Schwelle ist `(frac + 1) · ln2`, also **10,4 nats** bei
`prob_frac_bits = 14`, **104** bei `f32` und rund **745** bei `f64`. Der
Ganzzahlpfad kollabiert rund zehnmal frueher als `f32`; Routerkollaps
ist ein bekanntes Problem von Expertengemischen und kein Erzeugnis
dieses Projekts.

## 1. Was gebaut ist, und wogegen es hilft

Ein Gemisch hat **zwei** absorbierende Zustaende, nicht einen, und sie
brauchen verschiedene Mittel.

| Zustand | Was passiert | Gegenmittel | Stand |
|---|---|---|---|
| **gewaehlt, Gewicht rundet auf null** | Softmax-Gradient exakt null, fuer Verlierer **und** Gewinner | Boden in `route_top_k` (θ_v 0.18.0) und `router_spreizung` | gebaut |
| **nie gewaehlt** | nie gerechnet, nie ein Gradient, **still tot** | `Expertenwacht` (Hungerzaehler ueber die Segmentfolge) | gebaut |
| **neuer Experte** | haette nie ein Logit in der Top-k | `experte_einhaengen` plus `Expertenwacht` | gebaut |

⚑ **Der zweite Zustand ist der stillere, und er ist der Normalfall.**
Bei 128 Experten und Top-8 ist „nicht gewaehlt" der Zustand von 120
Experten je Token. Tot ist einer erst, wenn es ueber **viele** Token so
bleibt, und keine einzige Zahl weicht dabei ab.

⚑ **`Expertenwacht` ist dieselbe Einsicht wie „Loss-Free Balancing"**
(Wang u. a. 2024, in DeepSeek-V3 im Einsatz), hier unabhaengig gefunden
und mit einer schaerferen Begruendung: Dort ist der Grund Kausalitaet,
hier Determinismus.

⚑ **Und der Shardweg durch eine Gemischebene ist gebaut und am echten
30B gemessen** (`Geshardetes-Training.md`, Abschnitt 5d): Experten
werden erst gehalten, **wenn sie gewaehlt sind** (25 von 128 bei sechs
Positionen und Top-8; alle 128 waeren 2,4 GB je Ebene), und der Versatz
im Wuerfelraum haengt an der **Expertennummer** statt an der
Auswahlreihenfolge.

## 2. Was fehlt: Zahlen fuer dieses Modell

Die Vorgaben in `Trainingsvorgaben::vorgabe()` sind gemessen, aber an
einem kleinen Modell:

```
geduld              4       Segmente ohne Wahl, dann hungert ein Experte
wacht_staerke      64       Logit-Einheiten Schub
spreizung_schwelle 8192     ab hier greift die Strafe
spreizung_daempfung 4       Rechtsschieber auf die Strafe
```

Dagegen steht, was am **echten** 30B gemessen ist
(`runtime/src/bin/router_saettigung.rs`, 2026-09-05, 720 Routerstellen
ueber 48 Ebenen):

| | |
|---|---|
| voll gesaettigt | 0 von 720 |
| Mischgewicht auf null | 0 von 5 760 |
| groesstes Mischgewicht | 16 107 von 16 384, also **98,3 %** |
| Logitspanne ueber alle 128 | groesste **22 294**, mittlere 13 996 |
| Abstand bis zur Saettigung | rund **41 167** |

⚠️ **Eine Schwelle von 8 192 liegt bei einer mittleren Logitspanne von
13 996 nicht am Rand, sondern mitten im Normalbetrieb.** Ob das
richtig ist, entscheidet ein Lauf und keine Ueberlegung: Eine Strafe,
die immer greift, ist keine Strafe, sondern ein zweiter Verlustterm mit
unbekanntem Gewicht.

**Zu messen ist deshalb, in dieser Reihenfolge:**

1. **Die vier Vorgaben gegen das 30B**, mit `trainingsguete` und der
   Deckungskurve aus Abschnitt 3 als zweitem Massstab. Der
   Vergleichsfall steht mit `Lastausgleich::aus()` bereit.
2. **`F` je Klasse**, und zwar getrennt fuer Aufmerksamkeit, Experten
   und Router. Die drei haben verschieden grosse Gradienten: Der
   Routergradient traegt den Faktor `p_i · (1 − p_i)`, und der ist bei
   98,3 % Spitzengewicht klein, **bevor irgendetwas saettigt**. Ein
   kleiner Gradient bei gleicher Rasterstufe heisst grosses `F`.

⚑ **Und `F` ist fuer dieses Modell jetzt erst rechenbar, ohne dass es
in Gleitkomma passen muesste.** `Konzept-Wachstum.md` Abschnitt 9 fuehrt
„F fuer 7B" als offen, weil das Modell in float32 nicht auf diese
Maschine passt. Die Rasterstufe steht aber **im Artefakt**: Sie ist
`2^-shift` je Zeile und liegt in den `*_shifts.bin`. Was fehlt, ist
allein die Groesse eines Schritts, und die liefert der ganzzahlige
Rueckwaertspfad, der seit `kernels` 0.25.0 steht. **Die Messung braucht
kein Gleitkommamodell mehr.**

## 3. Der Befund vom 2026-09-11: Ausgleich kostet Durchsatz

Das ist der eine Punkt, den dieses Dokument beitraegt und der vorher
nirgends stand.

### 3.1 Die Schieflage, gemessen

`runtime/src/bin/expertenprobe.rs`, 289 Token, 110 976 Expertenaufrufe
ueber 48 Ebenen und 6 144 Experten:

| Anteil der Experten | Deckung der Aufrufe | Speicher |
|---|---|---|
| 5 % (307) | 40,6 % | 1,4 GB |
| 10 % (614) | 58,7 % | 2,9 GB |
| 20 % (1 228) | **78,3 %** | 5,8 GB |
| 30 % (1 843) | 89,0 % | 8,7 GB |
| 50 % (3 072) | 98,5 % | 14,5 GB |

Beruehrt: 3 971 von 6 144 (64,6 %). **Die haeufigsten 20 % tragen das
3,92-fache dessen, was sie bei Gleichverteilung truegen.**

### 3.2 Was das an der Maschine bedeutet

Gemessen am selben Tag (Fund 331): Das Artefakt ist 30,8 GB, der
Seitenpuffer dieser Maschine haelt rund **10 GB**. Ein
Token liest 1,81 GB Expertengewichte. Woher sie kommen, entscheidet ueber
den Faktor zwanzig:

| | Durchsatz |
|---|---|
| warm, aus dem Seitenpuffer | 16 bis 35 GB/s |
| kalt, von der Platte | 1,75 bis 5,2 GB/s |

**Eine schiefe Verteilung passt in den Puffer, eine flache nicht.**

### 3.3 Die Abwaegung, ausdruecklich benannt

`Expertenwacht` **flacht die Verteilung ab**, und genau das ist ihr
Zweck: Ein Experte, der nie gewaehlt wird, lernt nie. Dieselbe
Abflachung verschlechtert auf jedem Knoten, der weniger Speicher hat als
das Artefakt gross ist, die Trefferquote des Seitenpuffers. Und das
werden die meisten Knoten sein.

⚑ **Damit ist der Lastausgleich keine reine Qualitaetsfrage mehr.** Wer
`wacht_staerke` und `geduld` waehlt, entscheidet zugleich ueber den
Durchsatz des Netzes. Die Deckungskurve aus 3.1 gehoert deshalb neben
die Guetezahlen in jeden Abstimmungslauf, und nicht erst in die
Nachbetrachtung.

⚠️ **Was hier ausdruecklich nicht behauptet wird:** dass das eine
gegen das andere aufzuwiegen sei. Es ist nicht gemessen, wie viel
Guete ein Prozentpunkt Deckung wert ist, und diese Abwaegung gehoert
dem Projektinhaber und nicht einer Vorgabe im Code.

## 4. Die dritte Wachstumsachse

`Konzept-Wachstum.md` Abschnitt 5 kennt zwei, und beide gelten fuer ein
Gemisch unveraendert: **Breite** je Experte (die ganzzahlige Aufteilung
`a = ⌊m/2⌋`, `b = m − a`, gemessen bitgleich) und **Tiefe** (eine neue
Ebene als Identitaet, gemessen funktionserhaltend und nicht tot).

Die dritte ist die Zahl der Experten, und der Operator dafuer steht:
`experte_einhaengen` gibt dem Neuen ein Logit unter allen anderen. Die
Ausgabe aendert sich um exakt nichts, **und der Neue lernt trotzdem**,
weil die `Expertenwacht` ihn nach `geduld` Segmenten hochschiebt.

⛑ **Warum der naheliegende Weg nicht traegt, steht schon im Code, und
diese Sitzung hat ihn unabhaengig noch einmal verworfen:** Zwei Kopien
mit gleichem Logit verdraengen einen dritten aus der Top-k, und der
Verlust ist das kleinste der `k` Mischgewichte, gemessen im Mittel
**1 068 von 16 384**, also 6,5 % des MLP-Beitrags dieser Ebene. Das ist
messbar, aber es ist keine Funktionserhaltung.

**Was offen bleibt, und das steht so auch in `Geshardetes-Training.md`
Abschnitt 6:**

- **Das Wachstumsereignis.** `myl_train::wachstum` kann Breite und Tiefe
  wachsen lassen, `experte_einhaengen` die Zahl. **Wer es ausloest und
  wie die Kette es beschliesst, ist offen.** Fuer die Expertenachse
  kommt eine Frage dazu, die es bei Breite und Tiefe nicht gibt:
  **welchen Experten der neue kopiert.** Die Deckungskurve aus 3.1 ist
  dafuer der naheliegende Massstab, und sie ist schon gemessen.
- **Der Operator auf dem Artefakt.** `experte_einhaengen` kennt nur die
  Logits; die Gewichte des Vorbilds kopiert der Aufrufer. Fuer ein
  Artefakt heisst das: eine Zeile mehr in der Routermatrix, drei Dateien
  mehr je Ebene, ein neues Manifest, ein neues θ_v. Gebaut ist das
  nicht.

## 5. Was von den grossen offenen Punkten das Gemisch betrifft

Drei der offenen Punkte aus `Geshardetes-Training.md` Abschnitt 6
wiegen beim Gemisch schwerer als bei einem dichten Modell.

**Die Gradientensammlung im Segment (Fund 189).** Bei kleiner Lernrate
ist `gradient / nenner` fast immer null mit grossem Rest; das
stochastische Runden entscheidet dann ueber jedes Gewicht, mit richtigem
Erwartungswert und einer Streuung, die das Signal ueberdeckt. Gemessen
streuen drei Wuerfelreihen bei sonst gleichem Lauf ueber 38,4, 20,8 und
20,1 gegen einen Ausgangsstand von 23,3.

⚠️ **Beim Gemisch ist das schlimmer, und zwar um den Faktor sechzehn.**
Ein Experte sieht nur die Token, die ihn gewaehlt haben, also bei Top-8
von 128 im Mittel ein Sechzehntel. **Dieselbe Zahl Schritte sammelt
ueber ein Sechzehntel der Beobachtungen**, und die Streuung des
Wuerfels bleibt dieselbe. Wer Fund 189 fuer ein dichtes Modell loest,
hat ihn fuer ein Gemisch noch nicht geloest.

**Die Lernrate haengt an der Tiefe (4.15).** 48 Ebenen sind mehr als
die 24, an denen gemessen wurde.

**Einbettung und Kopf werden nicht trainiert (3.2).** Bei 30B-A3B ist
der LM-Kopf int16 ueber 151 936 Zeilen, also 0,62 GB und kein
Weight-Tying. Ob er drinbleiben muss, ist eine eigene Frage.

## 6. Die Reihenfolge

1. **Die vier Lastausgleichsvorgaben gegen das 30B messen**, mit der
   Deckungskurve als zweitem Massstab (Abschnitt 2 und 3).
2. **`F` je Klasse rechnen** (Aufmerksamkeit, Experten, Router) aus den
   Shifts des Artefakts und dem ganzzahligen Rueckwaertspfad.
3. **Fund 189 fuer das Gemisch**, mit dem Faktor sechzehn aus
   Abschnitt 5.
4. **Den Artefaktoperator fuer einen neuen Experten** bauen, und erst
   danach die Frage nach dem Wachstumsereignis stellen.

## 7. Was dieses Dokument nicht deckt

- **Einen ausgewachsenen Lauf.** Alle Zahlen hier stammen aus Sonden am
  **untrainierten** Ausgangspunkt oder aus Laeufen an kleinen Modellen.
- **Den Korpus.** `Geshardetes-Training.md` Abschnitt 5b.
- **Die Oekonomie.** TOKENOMICS und GOVERNANCE.

## 8. Die Belege nachrechnen

```bash
cd INTEGER_LLM

# Abschnitt 2: wie weit der Router von der Saettigung ist
cargo run --release --bin router_saettigung -- artifacts/myelith-30b-a3b

# Abschnitt 3.1: die Deckungskurve der Experten
cargo run --release --bin expertenprobe -- artifacts/myelith-30b-a3b

# Abschnitt 3.2: warm gegen kalt auf dieser Maschine
#   Zweimal dieselben Expertendateien lesen und die Zeiten
#   vergleichen; ueber rund 10 GB ist der zweite Lauf langsamer.
```
