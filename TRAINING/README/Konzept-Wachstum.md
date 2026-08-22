# Konzept: Verifizierbares Training und ein Modell, das wächst

> **Stand:** 2026-08-22 · **Grundlage:** die Messungen 0.1 und 0.2
> (`tests/diag/results/entscheidung_0-1.md`, `entscheidung_0-2.md`)
>
> Dieses Dokument ist der Entwurf, aus dem der TRAINING-Fahrplan
> entsteht. Es steht auf Messungen, nicht auf Annahmen; wo es das nicht
> tut, steht es dabei.

## 1. Was gemessen ist

| Frage | Antwort | Beleg |
|---|---|---|
| Trägt das Quantisierungsschema im Rückwärtspass? | **ja**, +0,67 % gegen Gleitkomma | 0.1 |
| Sind die Gradienten das Problem? | **nein**, int8 je Block trifft die Referenz exakt | 0.1 |
| Was bricht das Training? | Rundung zur nächsten Stufe bei den Gewichten, +29,9 % | 0.1 |
| Hilft stochastisches Runden? | **ja**, eine Zeile Code dreht das Ergebnis | 0.1 |
| Geht ein Schritt ohne Gleitkommazustand? | **ja**, ganzzahliger Master, +0,75 % | 0.2 |
| Kostet der Zufall den Determinismus? | **nein**, zählerbasierter Würfel, geräteunabhängig | 0.2 |
| Ist eine Expansion exakt funktionserhaltend? | **ja**, bitgleich, Abweichung 0,00e+00 | Abschnitt 5 |
| Wird die Symmetrie der Kopien gebrochen? | **ja**, zweifach, davon einmal ohne Zufall | Abschnitt 5 |
| Bleibt eine Identitätsebene beim Tiefenwachstum tot? | **nein**, sie bewegt sich ab Schritt 0 | Abschnitt 5.3 |
| Ist der Vorwärtspfad schon ganzzahlig? | **ja, vollständig**, samt Einbettung und LM-Kopf | Abschnitt 3 |
| Wie groß ist die Lücke zum Rückwärtspass? | zwei Kernel, eine LUT, **kein neuer Operationstyp** | Abschnitt 3 |
| Wie breit müssen Master und Akkumulator sein? | F = 25, beides **int64** (für 0,5B bei lr 1e-5) | Abschnitt 2, 4 |

## 2. Der Trainingsschritt

```
Zustand        m       ganze Zahl je Gewicht, int8-Wert plus F Nachkommabits
               shift   Zweierpotenz-Skala je Ausgabezeile, eingefroren
               keim    Teil der Konsensdaten

Vorwärtspfad   w8 = klemmen(⌊m / 2^F⌋ + [würfel(ebene, schritt, index, keim) < frac(m / 2^F)])
               W  = w8 / 2^shift

Rückwärtspfad  g_q = int8 mit Zweierpotenz-Skala je Block (Anhang B.6.2)

Aktualisierung m ← m − round(lr · g · 2^shift · 2^F)
```

**Jede Zeile ist ganzzahlig.** Kein Gleitkommazustand überlebt einen
Schritt, und `round` trifft dabei auf Werte, die um Größenordnungen über
einem LSB liegen (gemessen: rund 20 LSB je Schritt).

**F ist ein θ_v-Parameter, keine Konstante**, und er ist zu rechnen.
`tests/diag/bitbudget.py` misst dafür je Zeile und je Schritt, wie groß
eine Aktualisierung gegen die Rasterstufe ist. Für 0,5B bei lr 1e-5:

| | Schritt / Rasterstufe | nötiges F |
|---|---|---|
| Median | 6,4e-6 | 18 Bits |
| 1. Perzentil | 5,7e-7 | 21 Bits |
| empfohlen (plus 4 Bits Reserve) | | **25 Bits** |

Das erste Perzentil zählt und nicht der Median: Sonst verschwindet das
untere Prozent der Zeilen still, und stille Verluste sind in diesem
Projekt schon zweimal teuer geworden (Fund 23, Fund 24).

Damit ist `W_master = 8 + 25 = 33 Bits`, der Master gehört also nach
**int64**. Die Simulation lief mit F = 16, weil float32 ganze Zahlen nur
bis 2^24 exakt hält; das ist eine Schranke des Messaufbaus, keine
Empfehlung.

**Auch das Delta gehört stochastisch gerundet.** Bei zu kleinem F ist
`round(lr · g · 2^shift · 2^F)` derselbe Fehler eine Ebene tiefer: Ein
Schritt von 0,42 LSB wird meistens zu null. Gemessen bei F = 16:
+0,73 % mit stochastischem Delta gegen +0,79 % mit Rundung zur nächsten
Stufe, bei 22 % mehr wirksamen Aktualisierungen. Mit ausreichend großem F
verliert die Frage an Gewicht, verschwindet aber nicht.

**Der Würfel ist eine Funktion, kein Zustand.** `splitmix64` über
(Ebene, Schritt, Index, Keim), also die Funktion, die als
`kernels/src/prng.rs::splitmix64` bereits im Projekt steht. Gemessen:
identisch zwischen CPU und MPS und zwischen Prozessen. Ein RNG mit
Zustand wäre hier unbrauchbar, und der von PyTorch ist auf MPS bei
gleichem Keim nachweislich nicht reproduzierbar.

## 3. Was ein Trainingssegment ist, und warum es verifizierbar ist

Die Verifikationseinheit ist ein **Trainingssegment**:

```
Eingabe    θ_v, Batch-Kennung, Startschritt, Schrittzahl, keim, lr
Ausgabe    Δm je Gewicht, ganzzahlig, plus Commitment-Hash darüber
```

Ein Segment ist eine **reine Funktion** seiner Eingabe. Zwei Miner mit
denselben Angaben müssen dasselbe Δm liefern; der Vergleich ist ein
Hashvergleich, genau wie bei der Inferenz (Kap. 6.1, 6.6). **Es braucht
keinen neuen Verifikationsmechanismus**, nur eine zweite Arbeitsklasse.

Die Streitfallauflösung aus Kap. 6.6 überträgt sich unverändert: Weichen
zwei Pods ab, wird bisektioniert, bis der erste abweichende Schritt
feststeht, und dieser eine Schritt wird von einem Arbiter nachgerechnet.

**Wie weit die Bedingung erfüllt ist, nachgesehen statt geschätzt.**

Der **Vorwärtspfad ist bereits vollständig ganzzahlig**, einschließlich
Einbettung (int8 je Token-Zeile), aller Ebenen, der finalen Normierung
und des LM-Kopfs (`runtime/src/model.rs::forward_token`, belegt durch
30/30 Konformitätsvektoren). Eine frühere Fassung dieses Abschnitts
behauptete das Gegenteil; sie beschrieb die **Simulation**, in der
`nn.Linear` ersetzt und der LM-Kopf ausgenommen wurde, nicht die
Auslieferung.

Was fehlt, ist der **Rückwärtspass**, und er ist kleiner als gedacht:

| Vorwärts | Rückwärts braucht | vorhanden |
|---|---|---|
| `linear_w8a16` | `gx = g·W`, `gW = gᵀ·x` | `dot_i8_i16`, dieselbe Form |
| Residual-Addition | Gradient geht durch | trivial |
| `rmsnorm_i16` | Ableitung mit rsqrt und zwei Summen | rsqrt-LUT da, **Kernel neu** |
| SiLU über LUT | `s(x)·(1 + x·(1 − s(x)))` | **LUT neu**, erzeugt wie die anderen |
| `softmax_int` | Jacobi `g·p − p·Σ(g·p)` | exp-LUT da, **Kernel neu** |
| `attention_int` | zwei Produkte plus Softmax-Rückwärts | Zusammensetzung der beiden |
| `rope` | Drehung um −θ | dieselbe sin/cos-LUT |
| Embedding-Nachschlag | Streuaddition auf eine Zeile | trivial |

**Zwei neue Kernel, eine neue LUT, kein neuer Operationstyp.** Alles ist
Nachschlag, Shift, Produkt und Summe, also dieselben Bausteine, die der
Vorwärtspfad schon bit-exakt beherrscht.

**Gebaut und geprüft (kernels v0.20.0, 2026-08-22): alle acht.** Dazu
die SiLU-Ableitungs-LUT und drei Golden Vectors aus einer unabhängigen
Nachbildung; der Konformitäts-Prüflauf steht bei **33 von 33** statt 30.
Offen bleibt allein der Nachweis, dass zwei Maschinen denselben
Gradienten liefern, und dafür fehlt dieselbe zweite Maschine wie beim
Vorwärtspfad.

## 4. Aggregation vieler Miner

Viele Miner trainieren verschiedene Segmente. Ihre Ergebnisse addieren
sich:

```
m_{v+1} = klemmen(m_v + Σ Δm_i)
```

**Die Summe ist ordnungsfrei**, und zwar aus demselben Grund wie die
Reduktion in `kernels/src/dot.rs`: Ganzzahlige Addition ohne Überlauf ist
assoziativ und kommutativ. Es gibt keine vorgeschriebene
Aggregationsreihenfolge, kein „wer zuerst kommt", keine Abhängigkeit von
der Netztopologie. Dieselbe Eigenschaft, die den GPU-Kernel parallel
reduzieren lässt, lässt hier beliebig viele Miner beitragen.

**Überlauf wird genau einmal behandelt, ganz am Ende.** Auch das ist der
Determinismus-Vertrag aus `dot.rs`: Wer zwischendurch klemmt, macht die
Summe reihenfolgeabhängig.

**Gerechnet** (`bitbudget.py`, 10 000 Beiträge à 1000 Schritte, F = 25):
Die Summe erreicht im schlimmsten Fall rund 2,2 · 10⁹ LSB. **int32 hat
dann keinen Abstand mehr** (Faktor 1), **int64 hat Faktor 4,3 · 10⁹**.
Der Akkumulator gehört also nach int64, wie die Reduktion in `dot.rs`
auch.

**Robuste Aggregation** (Ausreißer verwerfen, Kap. 7) sitzt eine Ebene
darüber und ändert daran nichts: Sie entscheidet, **welche** Δm in die
Summe eingehen, nicht wie summiert wird.

## 5. Wachstum: ein Modell, das immer weiter wächst

Kap. 7.5 sieht funktionserhaltende Expansion vor (Net2Net, bert2BERT).
In Gleitkomma ist sie nur *näherungsweise* funktionserhaltend und braucht
künstliches Rauschen, um die Symmetrie der Kopien zu brechen.
**Ganzzahlig ist beides besser lösbar.**

### 5.1 Breitenwachstum: die ganzzahlige Aufteilung

Eine Einheit `j` wird verdoppelt:

- **eingehend:** Zeile `j` kopieren, samt ihrer Skala.
- **ausgehend:** die Spalte `j` **aufteilen** statt halbieren:

```
a = ⌊m / 2⌋        b = m − a        a + b = m
```

`a + b = m` gilt für jede ganze Zahl, gerade wie ungerade. Es gibt keinen
Rundungsfehler, weil nichts gerundet wird.

**Gemessen:** Die Ausgabe nach der Expansion ist **bitgleich** zur
Ausgabe davor, maximale Abweichung `0,00e+00`. Damit ist das
Akzeptanzkriterium aus Phase 4 („verhält sich nachweislich identisch zum
Vorgänger") kein Toleranzvergleich, sondern ein Digestvergleich, und
genau die Art Prüfung, die dieses Projekt sonst auch verlangt.

Zum Vergleich: Ein erster Versuch, die Spalte über die Skala zu
halbieren, lag um `1,24e-03` daneben. Der Unterschied zwischen „fast
gleich" und „bitgleich" ist hier der Unterschied zwischen einer Aussage
und keiner.

### 5.2 Die Symmetrie bricht zweifach, davon einmal ohne Zufall

Zwei exakte Kopien bekämen identische Gradienten und blieben für immer
identisch: Die neue Kapazität wäre tot. Gemessen über 20 Schritte:

| Mechanismus | Kopien identisch? |
|---|---|
| eingehende Zeilen, Rundung zur nächsten Stufe | in 20 von 20 Schritten **ja** |
| eingehende Zeilen, **stochastisches Runden** | in 0 von 20 Schritten, also **gebrochen** |
| ausgehende Spalten, **ganzzahlige Aufteilung** | **nein**, 7 von 16 Einträgen verschieden |

Die Aufteilung trennt `a` und `b` bei jedem ungeraden Eintrag um 1 LSB,
**ohne jeden Zufall**. Das stochastische Runden trennt zusätzlich die
eingehenden Zeilen. Beide Mechanismen sind deterministisch und
nachrechenbar; Net2Nets künstliches Rauschen wird nicht gebraucht.

### 5.3 Tiefenwachstum und Shards

Kap. 7.5 koppelt Tiefenwachstum an die Shard-Anzahl. Eine neue Ebene ist
funktionserhaltend, wenn sie als Identität startet: im Residualstrom
heißt das ein Ausgabegewicht von null, also ein Master von null. Das ist
exakt darstellbar und exakt prüfbar.

Der Anschluss an `k` im Scheduler ist reine Neuverteilung
(Sidequest 2, Abschnitt c): keine bestehende Struktur ändert sich
rückwirkend, die Speicherkapazität wächst mit dem Netz mit.

**Gemessen (2026-08-22), und die Sorge war unbegründet.** Der Verdacht
war, eine als Identität startende Ebene bleibe tot: Ausgabegewicht null,
Beitrag null, Gradient null. Der Gradient nach dem Ausgabegewicht ist
aber `aᵀ·g` und hängt **nicht** vom Ausgabegewicht ab. Ein Nullgewicht
macht den Beitrag null, nicht den Gradienten.

Nachgemessen an einer Ebene mit Residualpfad: Die Ausgabe ist bitgleich
zur Eingabe (Funktionserhaltung), und die Ebene bewegt sich **ab dem
ersten Schritt**. Mit Rundung zur nächsten Stufe bewegen sich 63 von 128
Gewichten, mit stochastischem Runden alle 128. Beleg:
`tests/diag/expansion_simulation.py` und die Probe im Protokoll.

## 6. Modellversionen, Übergang und Verfall

Damit sind zwei offene Sidequests entscheidbar.

**θ_v+1 ist eine reproduzierbare Funktion von θ_v** (Fahrplanpunkt 4.3):

```
θ_v+1 = f(θ_v, Wachstumsoperator, Σ Δm, keim, Aggregationsregel)
```

Jeder Bestandteil ist ganzzahlig oder eine benannte Regel. Wer θ_v und
die Segmente hat, rechnet θ_v+1 nach; die Verankerung ist ein Hash, keine
Vertrauensfrage.

**Übergangsfrist (Sidequest 2c):** θ_v und θ_v+1 sind eine Zeit lang
parallel gültig, und **jedes Segment nennt seine Modellversion
ausdrücklich**, statt sich auf einen globalen Umschaltzustand zu
verlassen. Der Testclient tut das seit v0.6.0 bereits: Jedes Protokoll
trägt θ_v und den Artefakt-Digest, und `vergleich` verweigert das Urteil,
wenn zwei Läufe gegen verschiedene Modellstände gerechnet haben. **Die
Mechanik ist gebaut und erprobt**, sie muss nur ins Protokoll gehoben
werden.

**Verfall (Sidequest 6):** Eine Version muss verfügbar bleiben, solange
Einsprüche gegen ihre Segmente möglich sind, also für die Dauer der
Streitfrist aus Kap. 6.6. Danach darf sie ins Speichernetz aus
Sidequest 2b abwandern. Keine eigene Infrastruktur, nur eine
Fristregelung, und sie hängt jetzt an einer Größe, die ohnehin
festgelegt ist.

**Wachstum braucht keine Übergangsregel**, weil es rückwirkend nichts
ändert: Alte Segmente bleiben gegen ihre alte Version prüfbar.

## 7. Was zu bauen ist, in dieser Reihenfolge

1. **Ganzzahliger Rückwärtspass in INTEGER_LLM.** Der Vorwärtspfad steht
   bereits. Zu bauen sind der RMSNorm-Rückwärtskernel, der
   Softmax-Rückwärtskernel und die SiLU-Ableitungs-LUT; alles Übrige ist
   Zusammensetzung oder trivial. Ohne ihn ist der Gradient
   geräteabhängig, und die Verifikationskette hängt in der Luft. **Kein
   TRAINING-Punkt.**
2. **F und die Akkumulatorbreite je Modell rechnen.** Das Werkzeug steht
   (`bitbudget.py`); für 0,5B bei lr 1e-5 sind es F = 25 und int64 für
   beides. Für 7B ist es noch nicht gerechnet, weil das Modell in
   float32 nicht auf diese Maschine passt.
3. **Der Wachstumsoperator als Bibliothek**: ganzzahlige Aufteilung,
   Identitätsebene, Digestvergleich vor und nach. Kleine, gut prüfbare
   Einheit, unabhängig vom Rest baubar.
4. **Trainingssegment als Arbeitsklasse** im Konsens: Eingabe, Ausgabe,
   Commitment, Streitfall. Erbt die Mechanik der Inferenz.
5. **Robuste Aggregation** und die Frage, wer welchen Batch bekommt.
6. **Datenprovenienz** (die alte Phase 1). Technisch unabhängig,
   inhaltlich Voraussetzung dafür, dass Training überhaupt zulässig ist.

## 8. Was dieses Konzept nicht deckt

- **Skalierung.** Alles gemessen an 0,5B. Kritikpunkt K6 bleibt offen,
  und er wird mit dem Wachstum wichtiger, nicht unwichtiger: Ein Modell,
  das immer weiter wächst, verlässt den gemessenen Bereich per Definition.
- **Ein ausgewachsener Trainingslauf.** Gemessen ist, dass das Verfahren
  die Gleitkomma-Referenz über 200 Schritte reproduziert, nicht dass ein
  langer Lauf Gleitkommaqualität erreicht.
- **Der Rückwärtspass selbst.** Zwei Kernel und eine LUT, kein neuer
  Operationstyp, aber gebaut ist er nicht.
- **Ökonomie und Governance.** Wer Wachstum beschließt, wer es bezahlt,
  wie Trainingsarbeit vergütet wird: TOKENOMICS und GOVERNANCE, nicht
  hier.

## 9. Was offen bleibt, und warum das vertretbar ist

Nach diesem Durchgang sind von den ursprünglich acht offenen Punkten
fünf geschlossen. Was bleibt:

| Offen | Warum es bleibt | Blockiert es? |
|---|---|---|
| **Rückwärtspass bauen** | Zwei Kernel, eine LUT. Beziffert, nicht gebaut | ja, aber die Aufgabe ist umrissen |
| **F für 7B** | 7B in float32 passt nicht auf diese Maschine | nein, das Werkzeug steht |
| **Ausgewachsener Trainingslauf** | Braucht Korpus, Zeit und den Rückwärtspass | nein, folgt daraus |
| **Skalierung (K6)** | Alles gemessen an 0,5B | nein, aber es wächst mit dem Modell |

**Was „gesichert" hier heißt und was nicht.** Gesichert ist das
**Verfahren**: Jede Behauptung dieses Konzepts, die sich messen ließ, ist
gemessen, und zwei davon sind dabei umgekippt (die Ursache lag nicht bei
den Gradienten; der Vorwärtspfad war längst ganzzahlig). Nicht gesichert
ist die **Umsetzung**, denn sie existiert nicht. Wer aus diesem Dokument
liest, das Training sei fertig, liest es falsch; wer liest, es sei
entworfen und an jeder prüfbaren Stelle geprüft, liest es richtig.

## 10. Die Belege nachrechnen

```bash
cd INTEGER_LLM/calibrate
S=../../TRAINING/tests/diag

# 0.1: traegt das Schema im Rueckwaertspass
.venv/bin/python $S/backward_reference_simulation.py \
    --schritte 200 --sequenzen 20 --halte 8 --lr 1e-5 --stochastisch

# 0.2: Trainingsschritt ohne Gleitkommazustand
.venv/bin/python $S/integer_master_simulation.py \
    --schritte 200 --sequenzen 20 --halte 8 --lr 1e-5

# Abschnitt 5: Expansion, Funktionserhaltung, Symmetrie
.venv/bin/python $S/expansion_simulation.py

# Abschnitt 2 und 4: F, Masterbreite, Akkumulator
.venv/bin/python $S/bitbudget.py --lr 1e-5
```
