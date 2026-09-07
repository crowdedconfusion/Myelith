# Geshardetes Training: Entwurf

**Datum:** 2026-09-05
**Stand (2026-09-05, dritter Durchgang): geshardetes Training steht.**

Ein Pod aus vier Shards über 24 Ebenen rechnet **bitgleich** zu einem
Rechner, der alle Ebenen am Stück hält, und liefert sein Segment über
`Anweisung::TrainingssegmentEinreichen` in den Kettenzustand ab.

**Die Prüfung steht seit dem vierten Durchgang** (Redundanzpaare,
Spurvergleich, Nachrechner), und die Modellfassung steigt aus
bestätigten Δm.

Offen sind drei Dinge: **Ebenen des Expertengemischs über
Shardgrenzen** (der nächste Punkt, denn das Primärmodell wird
wahrscheinlich MoE), der **Draht** zwischen echten Prozessen, und die
**Vergütung** bestätigter Segmente.

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

## 5c. Was der vierte Durchgang gebracht hat (2026-09-05)

### ⚑ Ohne Paare ist ein Trainingssegment nicht prüfbar

Der Plan gab bis dahin **jedem** Pod ein eigenes Bündel aus seiner
Nummer. Das sah nach Vielfalt aus und war ein Loch: Ein Ergebnis, das
nur einer gerechnet hat, lässt sich mit nichts vergleichen, und die
Vergütung hinge an einer Behauptung.

`Trainingsplan::paare` bildet Redundanzpaare, und **beide Pods eines
Paars bekommen dasselbe Bündel**, abgeleitet aus der **Paarnummer**.
Eine ungerade Zahl lässt einen übrig, und der bekommt kein Bündel: Ihm
eines zu geben hiesse, unprüfbare Arbeit zu bestellen.

### Die Prüfung, und worin sie stärker ist als die der Inferenz

| | Inferenz | Training |
|---|---|---|
| Spur über | Layer | **Shards** |
| Nachrechnung beginnt bei | Eingangsaktivierungen **aus der Spur des Beschuldigten** | dem **Artefakt** |
| Umfang der Nachrechnung | ein Shard | der **ganze Lauf** |

⚑ **Der Trainingsprüfer braucht nichts, was der Beschuldigte liefert.**
Ein Segment ist vollständig bestimmt durch die Gewichte der
Modellfassung, die Charge, die Lernrate und die Schrittzahl.

⚑ **Der Preis ist der ganze Lauf statt eines Shards**, und das ist kein
Versäumnis: Der Δ eines Shards hängt über den Rückwärtsweg an allen
Shards hinter ihm.

### Die Modellfassung steigt, aber die Kette rechnet nicht

`LedgerState::modell_version` steigt nur durch bestätigtes Training. Die
Kette hält das **Rezept** (alte Fassung plus geordnete Liste der
bestätigten Δ-Commitments), nicht das Ergebnis: Wer die Gewichte hat,
wendet es an und rechnet die Wurzel nach.

⚑ **Die Reihenfolge ist nur für die Kennung, nicht für die Arithmetik.**
Wäre sie für die Summe nötig, wäre die Aggregation nicht ordnungsfrei,
und zwei Knoten mit verschieden sortierten Eingaben kämen zu
verschiedenen Gewichten.

### ⚑ Ein Netz mit einem Pod kann nicht trainieren

Die Probekette hatte acht Konten, also einen Pod, also kein Paar. Der
Trainingstest fiel, als die Paarbildung dazukam, **und das war
richtig**. `PROBEKONTEN` steht jetzt auf zwölf: zwei Pods, ein Paar, das
Minimum eines Netzes, das trainieren kann.

---

## 5d. Was der fünfte Durchgang gebracht hat (2026-09-06)

### Der Draht, und dass er auch der Inferenz fehlte

Die Shards eines Pods lagen bis heute **in einem Prozess**, und zwar
nicht nur beim Training. `myl-shard` ist ein Shard mit einem Schlüssel
und einer Tür; `Shardweg` ist die Sternverbindung des Koordinators
dorthin. Vier eigenständige Prozesse liefern dieselben Token, denselben
Digest, dieselbe vTFE-Zuschreibung und eine gemeinsame aggregierte
Unterschrift wie der Einzelknoten (Fund 186).

⚑ **Stern und nicht Kette.** Ein weiterreichender Shard müsste seinen
Nachfolger selbst wählen und beim Ausfall selbst entscheiden. Beides
gehört zum Koordinator, weil nur er sieht, wer noch da ist.

### Die Gemischebene über Shardgrenzen

Der oben als „wichtigster offener Punkt" geführte Fall ist gebaut und am
echten 30B gemessen. Zwei Dinge waren daran anders als bei einer dichten
Ebene: Die Experten werden erst gehalten, **wenn sie gewählt sind** (25
von 128 bei sechs Positionen und Top-8; alle 128 wären 2,4 GB je Ebene),
und der Versatz im Würfelraum hängt an der **Expertennummer** statt an
der Auswahlreihenfolge.

### Fund 188, und was er über das Messen sagt

Jede Ebene schrieb ihre Ausgabe auf `final_residual_frac` statt auf die
Eingangsskala der **nächsten**. Solange nur die letzte Ebene trainiert
wurde, waren beide dasselbe, und der Fehler war unsichtbar. Perplexität
30 474 statt 24,04.

⚑ **Gefunden hat ihn kein Codelesen, sondern ein Test, der den
Shardweg gegen `run_layers` hält**, also gegen den Weg der Inferenz. Ein
Test, der den Shardweg nur mit sich selbst vergleicht, hätte
zugestimmt: Zwei Zuschnitte rechneten dasselbe Falsche.

### Fund 190: das Paar hatte keine Bedingung an die Unabhängigkeit

Gepaart wurde nach Podnummer. Die Inferenz sucht seit jeher zuerst ein
**zonendiverses** Paar und meldet, wenn es keines gab; das Training tat
beides nicht. `Trainingsplan::zonendivers` schliesst das, und
`paare_bilden` sucht den Partner zuerst in einer anderen Zone.

⚑ **Erzwungen wird es nicht, und das hängt am Sammeltopf oben.** Ein
Pod aus dem Topf ist zonengemischt und damit unbestimmt; zwölf Miner auf
drei Zonen ergeben zwei gemischte Pods und **kein** diverses Paar. Wer
hier auf Diversität besteht, trainiert nie.

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
- ⚑ **Die Gradientensammlung innerhalb eines Segments, und das ist
  jetzt der wichtigste offene Punkt** (4.14, Fund 189). Bei kleiner
  Lernrate ist `gradient / nenner` fast immer null mit grossem Rest, und
  das stochastische Runden entscheidet über jedes Gewicht: mit richtigem
  Erwartungswert, aber einer Streuung, die das Signal überdeckt.
  Gemessen streuen drei Würfelreihen bei sonst identischem Lauf über
  38,4, 20,8 und 20,1 gegen einen Ausgangsstand von 23,3. **Und die
  Redundanz fängt das nicht**, weil beide Pods denselben Würfel werfen.
- **Die Lernrate hängt an der Tiefe** (4.15). `TRAININGS_LR_NENNER` ist
  eine Zahl; über 24 Ebenen ist sie eine andere als über eine.
- **Expertenwachstum braucht ein Wachstumsereignis.**
  `myl_train::wachstum` kann Breite und Tiefe wachsen lassen; wer es
  auslöst und wie die Kette es beschliesst, ist offen.
