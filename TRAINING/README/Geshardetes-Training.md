# Geshardetes Training: Entwurf

**Datum:** 2026-09-05
**Stand (2026-09-05, dritter Durchgang): geshardetes Training steht.**

Ein Pod aus vier Shards über 24 Ebenen rechnet **bitgleich** zu einem
Rechner, der alle Ebenen am Stück hält, und liefert sein Segment über
`Anweisung::TrainingssegmentEinreichen` in den Kettenzustand ab.

Offen sind die **Prüfung** (Redundanz und Bisektion auf
Trainingssegmente, und damit die Vergütung) und der **Draht** (dieselben
Nachrichten zwischen echten Prozessen statt in einem).

---

## 1. Wie die Podzuteilung heute läuft

**Ja, das Modul gibt es**, und zwar seit langem. Es ist eine Kette aus
vier Schritten, und sie hat genau einen Einstiegspunkt.

| Schritt | Wo | Was |
|---|---|---|
| 1 | `myl_scheduler::miner_filter::filter_miners` | Filtert das Register nach Anmeldeschluss und Hardware-Klasse |
| 2 | `myl_scheduler::zonenzuteilung::zonen_cluster` | Gruppiert nach `GeoRegion`. Eine Zone mit genug Minern wird ein eigener Topf, dünne Zonen fallen in **einen** Sammeltopf. Jeder Topf wird mit einer zonenabhängigen Saat gemischt |
| 3 | `myl_scheduler::shard_assignment::assign_pods` | Schneidet jeden Topf in Scheiben zu `k + 2` und macht daraus Pods. Was übrig bleibt, steht in `Zuteilung::ohne_pod` |
| 4 | `myl_scheduler::shard_assignment::assign_shards` | Fisher-Yates mit podabhängiger Saat: die ersten `k` werden Shard-Positionen, die letzten zwei die Reserve |

**Der Einstieg:** `zonenzuteilung::zuteilung_der_epoche(register, epoche,
epochensaat, shards_je_pod)`.

⚑ **Er wird an genau einer Stelle gerufen**,
`myl_node::kette::Kette::zuteilung_der_laufenden_epoche`, und das ist
das Ergebnis von Fund 143: Vorher stand die Ableitung an drei Stellen,
zwei davon mit dem falschen Blockhash. Der Abschluss rechnete gegen eine
Zuteilung, die während der Epoche niemand kannte.

⚑ **Die Saat kommt aus `e−2`** und steht im Kettenzustand. Sie ist bei
der Anmeldung nicht bekannt, also lässt sich eine Position nicht
anzielen.

### Was die Zuteilung heute **nicht** kennt

**Sie kennt keine Podarten.** Jeder Pod ist ein Inferenzpod. `k` ist eine
Konstante der Probekette (`PROBE_SHARDS = 4`).

### Die bekannte offene Flanke

⚑ **Die Zone ist eine Erklärung** (Fund 108). Wer eine Zone angibt, in
der sonst niemand steht, bekommt daraus einen eigenen Topf und damit
ganze Pods. Zwölf Anmeldungen in zwei leeren Zonen ergeben zwei ganze
Pods und ein zonendiverses, also bevorzugtes Redundanzpaar. Eine
Anmeldung kostet nichts.

Das Mischen nimmt den **Rechenangriff** auf die Kennung weg, nicht den
**Zonenhebel**. Er ist gemessen, benannt und offen; ihn zu schliessen
hiesse, die Zone aus der Besetzung zu nehmen, und das kostet Latenz.

**Für das Training ändert sich dadurch nichts zum Schlechteren**, aber
auch nichts zum Besseren: Wer den Hebel für Inferenz hat, hat ihn auch
fürs Training.

---

## 2. Was von der Trainingsseite schon steht, und was keinen Aufrufer hatte

| Baustein | Wo | Aufrufer vor dem 2026-09-05 |
|---|---|---|
| Ganzzahliger Rückwärtspass | `integer-llm-kernels::backward` | Trainingsschleife |
| Optimierer, Master-Gewichte | `kernels::optimierer` | Trainingsschleife |
| Aggregation, dünn und dicht | `optimierer::aggregiere`, `::aggregiere_duenn` | **keiner** |
| Trainingsabdruck, Δ-Commitment | `optimierer::trainingsabdruck`, `::delta` | Trainingsschleife |
| Arbeitsklasse `Trainingssegment` | `myl_types::trainingssegment` | **keiner in Pod, Node, Consensus** |
| Datenprovenienz | `myl_train::provenienz` | **keiner** |
| Korpuszuweisung nach VRF | `myl_train::zuweisung` | **keiner** |
| Wachstumsoperator | `myl_train::wachstum` | **keiner** |
| Auslastung | `myl_tokenomics::utilization` | **nur Tests** |
| Trainingsvergütungsdeckel | `myl_tokenomics::training` | **nur Tests** |

⚑ **Sieben von zehn Bausteinen waren gebaut, geprüft und unerreicht**
(Funde 183 und 184). Das ist die bekannteste Fehlerklasse dieses
Projekts, hier in ihrer grössten Ausprägung: nicht eine Funktion ohne
Aufrufer, sondern eine ganze Kiste.

Die gemeinsame Ursache ist eine einzige: **Es gab keinen Weg, auf dem
ein Pod ein Trainingssegment zugewiesen bekommt.** `zuweisen`
beantwortet die Frage „welches Bündel bekommt Pod *i*?", und niemand
stellte sie.

---

## 3. Der Trainingspod: gebaut am 2026-09-05

`myl_scheduler::trainingszuteilung` schliesst die Lücke aus Abschnitt 2.

### 3.1 Wie viel des Netzes trainiert

```text
anteil = grundrate + freianteil · frei
frei   = max(0, 1 − auslastung)
```

| Auslastung | frei | Anteil bei 200 / 1000 bp |
|---|---|---|
| 100 % | 0 % | **2,0 %** |
| 80 % | 20 % | 4,0 % |
| 40 % | 60 % | 8,0 % |
| 0 % | 100 % | 12,0 % |

**Konstant bei normaler Last, steigend bei fallender Last.** Genau das
war die Anforderung.

⚑ **Die Grundrate ist eine Abweichung vom Whitepaper.** Kap. 7.1
bemisst die Trainingsmenge allein an der **freien** Kapazität: „Damit
ist ausgeschlossen, dass Training Inferenzkapazität verdrängt." Wörtlich
gelesen heisst das, dass bei voller Auslastung niemand trainiert, und
ein Netz, das Erfolg hat, hört auf, sein Modell zu verbessern.

Die Grundrate hält eine Untergrenze frei. Sie steht als
`Parameter::TrainingsGrundrate` mit der Herkunft **„Entwurf"**, weil sie
niemand beschlossen hat, und **null stellt Kap. 7.1 wörtlich wieder
her**. Das ist eine Entscheidung des Projektinhabers und keine
Ableitung.

### 3.2 Wo die Auslastung herkommt, und dass es sie vorher nicht gab

Bis zum 2026-09-05 stand im Kettenzustand **keine gemessene Nachfrage**.
`calculate_utilization` hatte keine Eingangsgrösse, und deshalb keinen
Aufrufer (Fund 184).

Gebaut:

- `LedgerState::vtfe_epoche` zählt in `credit_spend`, dem **einen**
  Engpass, durch den jede bezahlte Anfrage geht.
- ⚑ **Nach der Prüfung und nicht davor.** Eine abgelehnte Ausgabe ist
  keine bediente Nachfrage; wer sie mitzählte, liesse die Auslastung
  durch Fehlversuche steigen, und das wäre ein kostenloser Hebel auf die
  Trainingsmenge des ganzen Netzes.
- `LedgerState::vtfe_vorepoche` hält den Stand der abgeschlossenen
  Epoche. **Die Zuteilung liest die Vorepoche**, weil sie feststeht,
  bevor die Epoche läuft. Dasselbe Kausalitätsargument wie beim
  Lastausgleich des MoE-Routers, der die vorige Charge benutzt.
- Beide rollen im **Epochenabschluss**, unter dem Wächter, den es dort
  schon gibt. Ein zweiter Abschluss bräuchte einen zweiten Wächter.

Die Auslastungsrechnung selbst ist nach `myl_types::auslastung`
gewandert: Zwei Schichten brauchen sie, also gehört sie unter beide. Die
`f64`-Anzeigehelfer sind in `myl-tokenomics` geblieben, denn `myl-types`
liegt im Konsenspfad des Gleitkomma-Audits. **Der Schnitt fällt genau
auf die Konsensgrenze.**

### 3.3 Warum kein Pod sich das aussuchen darf

Trainingsvergütung ist auf 70 Prozent der Inferenzvergütung gedeckelt
(Kap. 5.6). Ein Pod, der wählen dürfte, wählte Inferenz, und zwar
**jeder**. Die Zuteilung wäre nicht bloss verzerrt, sondern leer.

Die Auswahl ergibt sich deshalb aus der Epochensaat, mit einer eigenen
Ableitung: Nähme sie dieselbe Permutation wie die Shard-Positionen,
wüsste wer seine Position kennt etwas über seine Trainingslast.

⚑ **Dasselbe Argument steht schon in `myl_train::zuweisung`** für den
Datenfall: „Wer keine Daten fälschen kann, kann immer noch
**auswählen**."

### 3.4 Ein kleines Netz trainiert nicht

Abgerundet, ohne Mindestzahl. Bei zehn Pods und vier Prozent ergibt das
null. Ein einzelner Trainingspod wären zehn Prozent des Netzes, also das
Dreifache dessen, was Anhang B.7.2 als verträglich beziffert.

---

## 4. Der Rückwärtsweg über Shardgrenzen

**Gebaut am 2026-09-05**, `integer_llm_runtime::shardtraining` und
`myl_pod::trainingswerk`. Was hier steht, ist der Entwurf und daneben,
was das Bauen daran geändert hat.

### 4.0 Das Ergebnis zuerst

| | |
|---|---|
| Zuschnitt | 4 Shards, 24 Ebenen, Grenzen `[0, 6, 12, 18, 24]` |
| Bewegt | 246 399 263 von 357 826 560 Gewichten über zwei Schritte |
| Gegen den Einzelknoten | **Ebene für Ebene bitgleich** |
| Gradient am Bereichseingang | identisch, samt Skala |

⚑ **Das ist die Bedingung, an der alles hing.** Wäre es anders, kämen
zwei Pods verschiedenen Zuschnitts zu verschiedenen Δm, ohne dass einer
falsch gerechnet hätte, und die Redundanzprüfung fiele.

### 4.1 Die Beobachtung, aus der alles folgt

Ein Inferenzpod läuft heute so: Shard *j* hält die Ebenen
`[L_j, L_{j+1})`, rechnet vorwärts und reicht den verborgenen Zustand an
Shard *j+1*.

Ein Trainingspod ist **dieselbe Pipeline, zweimal**:

1. **Vorwärts**, wie bei der Inferenz, aber jeder Shard **behält seinen
   Mitschnitt**.
2. Der letzte Shard rechnet den Verlust gegen das Ziel und erzeugt
   `dL/dZ` am Ausgang der letzten Ebene.
3. **Rückwärts**: Shard *j+1* rechnet die Gradienten seiner Ebenen und
   reicht `dL/dX` an seinem **Eingang** zurück an Shard *j*, der es als
   `dL/dZ` an seinem **Ausgang** liest.
4. Jeder Shard schreibt seine eigenen Master-Gewichte fort.
5. Jeder Shard liefert ein Δm für seine Ebenen.

⚑ **Schritt 3 ist bereits definiert.** Die Gradientenbus-Konvention
dieses Projekts sagt: Ein Gradientenfeld trägt `dL/dZ` **nach dem realen
Wert** von Z, auf einer Skala, die der Aufrufer wählt. Genau das ist die
Schnittstelle zwischen zwei Shards.

### 4.2 Drei Anforderungen, die daraus folgen

**(a) Die Skala muss über den Draht, und sie ist ein Feld.** Der
empfangende Shard muss die Skala kennen, sonst weiss er nicht, was er
bekommen hat. Ein Gradient ohne Skala ist eine Zahl ohne Einheit.

⚑ **Beim Bauen kam eine Verschärfung dazu:** Der Residualstrom dieses
Projekts trägt **eine Verschiebung je Kanal**, nicht eine für alle.
`Shardergebnis::eingang_frac` ist deshalb ein `Vec<u8>`. Wer hier ein
`u8` erwartete, hätte jeden Kanal ausser einem falsch skaliert, und der
Fehler wäre klein genug, um wie Rauschen auszusehen.

⚑ **Das ist genau die Fehlerklasse von Fund 179**: Der Router-Zweig
rechnete mit `router_frac` statt `act_frac` und war exakt achtfach
daneben. Der Abstandstest blieb grün. Gefunden wurde es nur über eine
geschlossene Form.

**(b) Der Mitschnitt überlebt die Lücke.** Bei der Inferenz ist ein
Shard je Token zustandslos. Beim Training hält er zwischen Vorwärts- und
Rückwärtslauf Zustand.

Das ändert sein Ausfallverhalten: **Ein Shard, der mitten im Segment
stirbt, nimmt den Mitschnitt mit**, und das Segment muss von vorn
gerechnet werden. Die Reserve kann nicht einspringen, weil sie den
Mitschnitt nicht hat.

⚑ **Daraus folgt eine Obergrenze für die Segmentlänge**, und sie ist
keine Bequemlichkeit: Sie ist der Erwartungswert verlorener Arbeit
geteilt durch die Ausfallrate.

**(c) Der Würfel muss über Shardgrenzen hinweg derselbe sein.**

Das stochastische Runden entscheidet über jedes einzelne Gewicht, und
der Würfel ist eine reine Funktion aus `(ebene, schritt, index)`. Wäre
`index` shardlokal, wäre geshardetes Training **nicht bitgleich** zum
Einzelknoten, und damit fiele die Redundanzprüfung aus, auf der die
ganze Verifikation ruht.

⚑ **Gemessen und nicht behauptet** (2026-09-05,
`optimierer::shardtransparenz`): Eine Ebene in Scheiben fortzuschreiben
ergibt dasselbe wie am Stück, sofern jede Scheibe ihren **globalen**
Versatz kennt. Die Gegenprobe zeigt, dass der falsche Versatz ein
anderes Δ ergibt.

⚑ **Und die Gegenprobe hat einen Fehler im Test selbst gefunden.** Der
erste Entwurf nahm Gradienten, die glatt durch den Nenner teilbar waren;
dann ist die Schrittweite exakt, das Runden hat nichts zu entscheiden,
und der Würfel kommt gar nicht vor. Beide Tests waren grün und prüften
nichts.

**Die Anforderung an den Shard lautet damit:** Er muss seine **globale**
Ebenennummer und seinen globalen Versatz innerhalb der Ebene kennen. Ein
Shard, der seine Ebenen bei null durchnummerierte, würfelte anders, und
niemand sähe es dem Ergebnis an: Es wäre ein plausibles Delta, nur ein
anderes.

### 4.3 Die verworfene Abkürzung: lokale Verlustblöcke

Kap. 7.2 nennt „lokale Verlustblöcke auf Shard-Grenzen". Jeder Shard
rechnete dann seinen eigenen Verlust und bräuchte **keine** Gradienten
von stromabwärts. Der ganze Rückwärtsweg entfiele.

**Der Preis:** Ein lokaler Verlust ist nicht der Verlust des Modells.
Die Ebenen lernen, ihre eigene Zwischenaufgabe zu lösen, und niemand
lernt die eigentliche.

⚑ **Und ein Punkt, der schwerer wiegt als der Lernerfolg:** Ein
lokal-verlustbasiertes Ergebnis lässt sich **nicht gegen eine
Einzelknotenrechnung prüfen**, weil es eine andere Rechnung ist. Jeder
Konformitätsvektor dieses Projekts ruht darauf, dass derselbe Eingang
denselben Ausgang ergibt, gleich auf welcher Maschine und in welchem
Zuschnitt. Der volle Rückwärtsweg erhält diese Eigenschaft, der lokale
Verlust gibt sie auf.

**Empfehlung: der volle Rückwärtsweg.** Die lokalen Verlustblöcke
bleiben als Rückfall benannt, falls sich die Mitschnittkosten als
untragbar erweisen; dann aber als bewusster Verzicht auf
Nachrechenbarkeit gegen den Einzelknoten, nicht als Optimierung.

### 4.4 Was der Pod abliefert

`myl_types::trainingssegment::Trainingssegment` steht bereits:
`{id, modell_version, charge, startschritt, schrittzahl, lr_zaehler,
lr_nenner, delta_commitment, pod_pfad, signaturen}`.

Jeder Shard liefert ein Δm für seine Ebenen; das `delta_commitment` des
Pods geht über die Verkettung **in Shardreihenfolge**. Das ist genau,
was `trainingsabdruck` tut.

⚑ **Und der Fall, den die Simulation gefunden hat, gehört in die
Meldung:** Ein Lauf, dessen Gewichte die Übertragungsform verlassen, hat
gerechnet und bewegt und sieht von aussen aus wie ein Erfolg. Seit dem
2026-09-05 meldet `Trainingsergebnis::aus_der_form` den Schritt, an dem
es geschah, und die Schleife hört dort auf. **Eine Panik wäre keine
Antwort:** Ein Absturz ist von einem ausgefallenen Miner nicht zu
unterscheiden.

### 4.4b Was das Bauen zusätzlich gelehrt hat

⚑ **Die Gewichte müssen den Schritt überleben.** Der erste Entwurf gab
`rueckwaerts` den Mitschnitt und änderte darin eine **Kopie** der
Gewichte, die danach wegfiel. Über einen einzigen Schritt wäre das nicht
aufgefallen; über dreissig wären es dreissig Mal derselbe erste Schritt
gewesen. Die Gewichte stehen jetzt in `Shardgewichte` und gehören zum
**Segment**, der Mitschnitt zu **einem Vorwärtslauf**.

⚑ **Die Anweisung gehört ans Ende des Enums.** Beim ersten Anlauf stand
`TrainingssegmentEinreichen` mitten drin, und Borsh kodiert Varianten
als Index: Jede dahinter wäre verschoben worden. Der Kommentar an der
Variante davor warnt genau davor.

⚑ **Ein Segment trägt eine Unterschrift je Mitglied**, anders als ein
PoI-Bündel mit fertigem Aggregat. Die Form bleibt, weil sie sagt, wer
haftet; geprüft wird trotzdem in **einem** Pairing, indem der Übergang
sie zusammenfasst.

### 4.5 Prüfung und Aggregation

**Prüfung:** wie bei der Inferenz. Redundanz `r = 2`, zwei Pods rechnen
dasselbe Segment, die `delta_commitment` werden verglichen, die
Bisektion lokalisiert. `Trainingssegment` trägt bereits `signaturen`.
Die Stichprobenrate für Trainingssegmente ist ein eigener Parameter und
liegt höher (Kap. 5.5).

**Aggregation:** `optimierer::aggregiere` summiert ganzzahlig,
ordnungsfrei, mit Sättigung genau einmal am Ende. Für MoE ist
`aggregiere_duenn` da, weil nur berührte Experten ein Δ haben.

**Offen bleibt 2.3, die robuste Aggregation.** Die Frage ist hier eine
andere als in der Literatur: Redundanz erkennt ein **falsch gerechnetes**
Δm bereits, also beantwortet statistische Robustheit eine schon
beantwortete Frage. Was Redundanz **nicht** erkennt, ist korrektes
Rechnen auf schlechten Daten, und das ist Datenprovenienz.

---

## 5. Baureihenfolge

| # | Was | Hängt ab von |
|---|---|---|
| 1 | ✅ Trainingspods erzeugen, Auslastung messen | nichts |
| 1b | ✅ Zonenreste in einen Topf, damit die Anfangsphase keine Kapazität verschenkt | nichts |
| 1c | ✅ Reihum-Auswahl mit Gedächtnis am Miner | 1 |
| 3 | ✅ `myl_train::zuweisung` an den Trainingsplan hängen: welcher Pod welches Korpusbündel | 1 |
| 2 | Kettenanweisung `TrainingssegmentEinreichen`, Streitfall mit Zähnen | 1 |
| 4 | Mitschnitt über die Shardgrenze halten, Segmentlänge aus der Ausfallrate | nichts |
| 5 | Rückwärtsweg über die Gegenstelle, mit Skala auf dem Draht | 4 |
| 6 | Δm je Shard einsammeln, Commitment in Shardreihenfolge | 5 |
| 7 | Redundanz und Bisektion auf Trainingssegmente | 2, 6 |
| 8 | Aggregation in den Kettenzustand, Modellversion anheben | 7 |

⚑ **Punkt 2 ist der nächste und nicht Punkt 4.** Der Rückwärtsweg ist
die interessantere Arbeit, aber ohne Kettenanweisung liefert der Pod
sein Ergebnis nirgendwohin ab, und das ganze Gebäude wäre wieder eine
Kiste ohne Aufrufer.

---

## 5b. Was der zweite Durchgang geändert hat (2026-09-05)

### Die Kurve: 82 statt 12 Prozent im Leerlauf

Der erste Entwurf nahm γ_train aus Kap. 7.1 wörtlich und kam bei leerem
Netz auf zwölf Prozent. **Das war falsch herum gedacht:** Kap. 7.1 fragt,
wie wenig Training das Netz verträgt; die andere Frage ist, was
leerlaufende Miner sonst tun sollen, und die Antwort ist nichts.

| Auslastung | frei | Anteil bei 200 / 8000 bp |
|---|---|---|
| 100 % | 0 % | 2,0 % |
| 80 % | 20 % | 18,0 % |
| 40 % | 60 % | 50,0 % |
| 0 % | 100 % | **82,0 %** |

⚑ **Warum nicht 100 Prozent der freien Kapazität:** Die Auslastung ist
die der Vorepoche. Steigt die Nachfrage innerhalb der laufenden Epoche,
können die Trainingspods sie nicht bedienen. Die fehlenden zwanzig
Prozent sind Luft, kein Rundungsrest, und **nicht gemessen**.

### Kaufmännisch gerundet statt abgerundet

Abrunden war bei vier Prozent richtig und kippt bei 82: Ein Netz mit
**einem** Pod träfe nie ein Training, auch vollständig leerlaufend. Der
Schutz kleiner Netze bleibt, zehn Pods bei vier Prozent ergeben weiter
null.

### Reihum, mit Gedächtnis am Miner

⚑ **„Ein Pod, der zuletzt trainiert hat, wird nicht gewählt" hat keinen
Gegenstand.** Pods werden jede Epoche neu gebildet; Pod 5 der Epoche 100
und Pod 5 der Epoche 101 sind verschiedene Leute. Was über Epochen
besteht, ist der Miner.

`LedgerState::trainingsstand` hält je Miner die Epoche seiner letzten
Heranziehung. Die Frische eines Pods ist die seines **zuletzt** dran
gewesenen Mitglieds, also das Maximum: Sonst liesse sich ein Vielrechner
immer wieder mit Neulingen zusammen nach vorn tragen.

**Geordnet und nicht ausgeschlossen.** Ein Ausschluss bräche bei hohem
Anteil zusammen: Bei 82 Prozent hätten fast alle trainiert, fast kein
Pod wäre wählbar, und das Training pendelte zwischen 82 Prozent und
null.

⚑ **Vorhersagbar, und das ist hier richtig.** Wer sich abmeldet und neu
anmeldet, gilt als „nie trainiert" und steht damit ganz vorn. Der
Ausweichversuch beschleunigt die eigene Heranziehung.

### Zonenreste kommen in einen Topf

`assign_pods` schnitt jeden Zonentopf und liess den Rest fallen. **Drei
Zonen mit je elf Minern ergaben drei Pods und liessen fünfzehn Miner
liegen**, vierzig Prozent der Kapazität. Genau in der Anfangsphase, in
der noch nicht jede Region besetzt ist.

Jetzt kommen die Reste aller Töpfe in einen gemeinsamen, gemischten
Topf: aus 33 Minern fünf Pods statt drei.

⚑ **Es nimmt keinem zonenreinen Pod etwas weg.** Die Töpfe werden zuerst
gierig geschnitten; gepoolt wird nur, was sonst gar keinen Pod bildete.
Und es ist keine neue Art von Pod: Das Sammelcluster der dünnen Zonen
bildet seit jeher zonengemischte Pods.

### Der Korpus, und warum ihn niemand verankern darf

`Trainingsplan::zuweisungen` sagt, welches Bündel jeder Trainingspod
bearbeitet, und ruft dafür `myl_train::zuweisung::zuweisen`. **Damit hat
`myl-train` seinen ersten Aufrufer** (Fund 183).

⚑ **Es gibt keine Anweisung `KorpusVerankern`, und das ist Absicht.**
Wessen Daten das Modell trainieren, ist die schwerste Entscheidung
dieses Systems; eine Anweisung, die jeder einreichen kann, übergäbe sie
an jeden. Der richtige Weg wäre ein Governance-Beschluss, und den gibt
es nicht: Die Parameter-Registry ist vom Kettenzustand aus nicht
erreichbar. **Solange das so ist, steht der Anker im Genesis.**

---

## 6. Was dieser Entwurf offen lässt

- **Der Knoten liest die Parameter-Registry nicht.** Er spiegelt die
  drei neuen Zahlen als `const`, und ein Test hält beide Fassungen
  gegeneinander. Die eigentliche Antwort wäre, die Parameter in den
  Kettenzustand zu nehmen, damit ein Beschluss sie bewegt statt eines
  neuen Baus.
- **`PodKapazitaet` ist eine Grössenordnung, kein Messwert.** Aus dem
  Gesamtlauf ist der vTFE-Betrag je Segment bekannt (rund 1,0 Mio.), die
  Segmente je Sekunde sind es nicht.
- **Einbettung und Kopf werden nicht trainiert** (3.2). Bei 0,5B ist die
  Einbettung wegen Weight-Tying zugleich der LM-Kopf: 136 Mio. Parameter
  in einem einzigen Δm.
- **Expertenwachstum braucht ein Wachstumsereignis.**
  `myl_train::wachstum` kann Breite und Tiefe wachsen lassen; wer es
  auslöst und wie die Kette es beschliesst, ist offen.
