# training (`myl-train`)

> **Version:** 0.4.3 (`myl-train` 0.3.0)
> **Datum:** 2026-09-11
> **Status:** **Die Komponente hat Code**, 32 Tests. Zwei Punkte sind
> gebaut, und beide sind genau die, die **nicht** am ganzzahligen
> Rückwärtspass hängen:
>
> - **Datenprovenienz** (v0.1.0): Merkle-verankerte Korpora,
>   Segmentreferenz per Beweis, gebündelte Beweise, VRF-gesteuerte
>   Zuweisung. Steht in der Planung hinten, weil er inhaltlich dorthin
>   gehört, ist aber technisch unabhängig.
> - **Wachstumsoperator, Bitbudget und Tiefenwachstum** (v0.2.0):
>   ganzzahlige Aufteilung, Identitätsebene, Digestvergleich vor und
>   nach.
>
> **Der Rückwärtspass ist seit dem 2026-09-04 kein Wartegrund mehr**
> (INTEGER_LLM `kernels` 0.34.0, `runtime` 0.25.0). Ein Kreis aus
> Vorwärtspass, Mitschnitt der Zwischenwerte, Gradient und Optimierer
> schliesst sich über **beide Blöcke** einer Ebene: über den ganzen
> MLP-Block und über den Aufmerksamkeitsblock samt RoPE, Softmax,
> Kopfgewichtung und gruppierter Aufmerksamkeit. Gemessen auf echten
> Gewichten und auf einem Experten eines Expertengemischs. Was fehlt,
> ist der Zusammenbau zur ganzen Ebene und die Schleife über eine
> Folge. **Die Zuordnung bleibt wie bisher:** Die Rechenkerne stehen in
> INTEGER_LLM, die Arbeitsklasse und die Aggregation stehen hier.
>
> ⚑ **Der Aufmerksamkeitsblock rechnet über eine Folge**, und das ist
> Bedingung und nicht Bequemlichkeit: Bei einer einzigen Position
> liefert der Softmax exakt eins, seine Ableitung ist exakt null, und
> Q und K bekämen keinen Gradienten.
>
> ⚑ **Eine Lernrate passt nicht zu allen Ebenen** (Fund 172). Ebene 0
> von Qwen2.5-0,5B trägt sechs Bit mehr Ausgabeskala als die mittleren
> Ebenen, derselbe Schritt wirkt dort vierundsechzigmal schwächer, und
> ihre typischen Werte liegen dicht unter der `i16`-Grenze. Die
> Obergrenze für die Rate ist damit nicht die Konvergenz, sondern die
> Sättigung. Das ist die nächste inhaltliche Frage der Blockskalierung.
>
> **Die eine Messung ist gemacht.** Punkt 0.1 ist beantwortet:
> Das Schema **trägt**, sofern die Gewichte stochastisch gerundet werden
> (+0,67 % gegen die Gleitkomma-Referenz; mit Rundung zur nächsten Stufe
> +29,9 %). Dazu 0.2: Ein Trainingsschritt **ohne Gleitkommazustand**
> geht, mit ganzzahligem Master und zählerbasiertem Würfel, +0,75 %.
> Protokolle:
> [`entscheidung_0-1.md`](../tests/diag/results/entscheidung_0-1.md) und
> [`entscheidung_0-2.md`](../tests/diag/results/entscheidung_0-2.md).
> Das Konzept daraus steht in
> [`Konzept-Wachstum.md`](Konzept-Wachstum.md): der Trainingsschritt,
> seine Verifikation, die Aggregation und ein Modell, das wächst.
> **Was am Expertengemisch anders ist**, steht daneben in
> [`Konzept-Gemischtraining.md`](Konzept-Gemischtraining.md): der
> Router und sein absorbierender Zustand, die gemessene Schieflage der
> Experten, das Wachstum in der dritten Achse und die vierte
> Shardachse.
> Die Planung steht damit; ihr erster Punkt liegt in INTEGER_LLM,
> nicht hier: Solange Vorwärts- und Rückwärtspass in Gleitkomma
> rechnen, ist der Gradient geräteabhängig und mit ihm jedes Δm.

Trainings als nachrangige Arbeitsklasse, lokale Verlustblöcke,
Datenprovenienz, robuste Aggregation, Modellwachstum.
Referenzimplementierung von Whitepaper Kap. 7.

## Warum hier nur ein Punkt steht

Whitepaper Kap. 7 setzt voraus, dass „die ganzzahlige Ausführung aus
Kapitel 6 unverändert auf den Rückwärtspass überträgt". Das ist eine
**Annahme, keine Messung** — und sie trägt alles: Ohne bit-exakten
ganzzahligen Rückwärtspass gibt es keine verifizierbare Trainingsarbeit,
ohne die keine Vergütung, ohne die kein Modellwachstum.

Es gibt einen konkreten Grund für Zweifel, und er stammt aus der eigenen
Erfahrung des Projekts. **Fund 20** hat gezeigt, dass der Residualstrom
eine Skala **je Kanal** braucht, weil einzelne Kanäle um Größenordnungen
aus der Verteilung ragen (Massive Activations); eine Skala je Tensor
löschte die feinskalierten Kanäle aus. Gradienten haben typischerweise
einen **größeren** Dynamikbereich als Aktivierungen — und über die
Trainingsschritte hinweg einen wandernden. Ob die Block-Skalierung aus
Anhang B.6.2 das trägt, ist offen.

Die Planung hatte 22 Punkte in vier Phasen, die alle darauf ruhten. Am
2026-08-19 wurde sie auf die Messung zurückgeschnitten, die das
entscheidet. Die alte Fassung bleibt als Vorüberlegung erhalten, ohne
Statusmarken: Sie geht nicht verloren, wird aber nach dem Ergebnis neu
geschnitten, möglicherweise anders.

**Die Methode ist erprobt:** In der 7B-Fehlersuche haben zwei
PyTorch-Referenzsimulationen in Stunden entschieden, was vorher tagelang
im falschen Code gesucht wurde. Sie trennen „trägt das Verfahren?" von
„ist unsere Implementierung richtig?" — und nur die erste Frage steht
hier an. Deshalb: erst simulieren, dann bauen.

## Aufgabe

Ermöglicht dem Netzwerk, das Netzwerkmodell fortzuschreiben, ohne
Inferenzkapazität zu verdrängen (Kap. 7.1) und ohne die inhaltliche
Bewertungsfrage zu öffnen, die das Protokoll sonst vermeidet (Kap. 7.3:
Herkunfts- statt Inhaltsprüfung). Diese Komponente ist laut Whitepaper
selbst die am wenigsten abgesicherte: Kap. 7.6 benennt drei ungelöste Punkte
(Finanzierungs-Fehlanreize, unbelegte Verfahrenskombination, unbekanntes
Verhalten unter offenen Netzbedingungen).

## Abhängigkeiten

COMPUTE_PIPELINE (Trainingssegmente nutzen dieselbe Pod-Infrastruktur),
CONSENSUS (VRF-Datenzuweisung nutzt den Epochen-Scheduler,
Ledger-Buchhaltung der Trainingsvergütung), VERIFICATION (Aggregations- und
Gradienten-Segmente brauchen dieselbe Bisektions-/Redundanzlogik),
TOKENOMICS (Trainingsvergütungs-Obergrenze) sowie ein **ganzzahliger
Rückwärtspass** in INTEGER_LLM.

📌 **Hier stand bis zum 2026-09-09, der Rückwärtspass sei „dort noch
nicht implementiert" und INTEGER_LLM behandle „bislang ausschliesslich
Inferenz".** Beides ist überholt. Der Rückwärtspass rechnet geshardet
und bitgleich, und am 2026-09-09 ist der erste inhaltliche Nachweis
gefallen: ein Artefakt, das eine hineingeschriebene Tatsache
beantwortet und drei Nachbartatsachen unverändert lässt.

## Wo das ganzzahlige Training beschrieben ist

⚑ **Nicht hier.** Diese Kiste koordiniert Training im **Netz**:
Datenprovenienz, Wachstumsoperator, VRF-Zuweisung. Wie ein Lauf
aussieht, der eine Tatsache in ein Modell schreibt, steht dort, wo das
Werkzeug liegt:

| Frage | Ort |
|---|---|
| Wie fahre ich einen Lauf, der trifft? | `INTEGER_LLM/README/README.md`, Abschnitt „Eine Tatsache hineinschreiben" |
| Wie baue ich einen Datensatz dafür? | Ebenda, derselbe Abschnitt |
| Was ist dabei offen geblieben? | Ebenda, am Ende des Changelog-Eintrags zum Treffer |

**Die drei Zahlen, die ein Lauf einhalten muss**, damit er überhaupt
etwas misst, und sie stehen hier, weil sie sonst niemand findet, der
von dieser Komponente kommt:

1. Der **Zielrang** muss fallen; halbiert er sich nicht alle paar
   Durchgänge, stimmt der Aufbau nicht.
2. Die **Trennschärfe** `Faktor(ziel) / Faktor(fremd)` muss **wachsen**.
   Bleibt sie bei 1,0, wird eine Form gelernt und keine Tatsache.
3. Die **Kontrolle** muss auf Rang 0 bleiben. Bricht sie weg, ist der
   Schritt zu gross.

## Struktur

Entsteht mit der Implementierung.

## Changelog

### v0.4.3 – 2026-09-25 (die Buchkette sagt, wofür man ein Buch verwenden darf)

`korpus/buchkorpus.py` schreibt vor jedem Bau eines Trainingskorpus auf
die Fehlerausgabe, dass nur Material verwendet werden darf, für das man
die Rechte hat (eigene Werke, freie Lizenzen, Erlaubnis der
Rechteinhaber), mit Verweis auf `COMPLIANCE/de/Urheberrecht.md`. Ein
Hinweis und keine Sperre: Ob ein Buch verwendet werden darf, weiß das
Skript nicht, der Mensch schon. Selbsttest grün.

### v0.4.2 – 2026-09-17 (eine eigene Umgebung, die ohne Netz entsteht)

`myl-train` unverändert; `korpus/` bekommt `einrichten.sh`,
`requirements.txt`, `umgebung.py` und `raeder/`.

⚑ **Die Kette braucht nichts zwingend**, und das bleibt so: EPUB, HTML,
Text und Markdown gehen mit der Standardbibliothek. Die Umgebung ist für
PDF (`pypdf`) und wird **ohne Netz** gebaut: Die Räder liegen im
Repositorium, `pip` bekommt `--no-index`.

⚑ **Nachgeprüft an einem frischen Klon mit blockiertem Netz**: Einrichten,
drei Selbsttests, PDF und EPUB umgewandelt, Korpus und Mappe gebaut,
ohne eine Verbindung.

📌 **Der erste Versuch scheiterte, und der Fehler ist die Lehre:** Die
Räder waren mit `--no-deps` geholt, und `pypdf` braucht unter Python vor
3.11 noch `typing_extensions`. **„Alle Abhängigkeiten" heißt auch die,
die eine Abhängigkeit selbst mitbringt**, und welche das sind, hängt hier
sogar von der Python-Fassung ab. ⛔️ **Und die Fehlermeldung riet zuerst
zu `--no-deps`**, also zu genau dem Fehler, der sie ausgelöst hatte; das
fiel erst beim Gegenprobieren auf.

⛔️ **Kein Interpreter im Repositorium**, und das ist eine Entscheidung
mit Begründung: Er wäre je Plattform zig Megabyte, die niemand prüft, und
bei jeder Sicherheitslücke nachzuziehen. Stattdessen eine Untergrenze,
die überall erfüllt ist (**Python 3.9**, die Fassung von macOS selbst),
**gemessen am jüngsten benutzten Sprachmittel** und von jedem Werkzeug
geprüft.

### v0.4.1 – 2026-09-17 (an echten Büchern geprüft: fünf Fehler, die kein Selbsttest hatte)

`myl-train` unverändert; `korpus/` nachgebessert.

⛔️ **Drei frei verfügbare Bücher in drei Formaten**, und fünf Fehler, von
denen die Selbsttests keinen hatte: Der **eigene YAML-Vorspann** wurde
zur ersten Trainingszeile, der **Lizenzrahmen der Buchquelle** (ein
gutes Drittel der Blöcke) landete im Korpus, zwei Formate desselben
Buches **überschrieben einander stillschweigend**, die Teilung nach
Abschnitten lieferte **32 % Haltemenge statt 15 %** (das Buch hat drei
Kapitel), und im PDF wurden **Tabellenzeilen zu Überschriften**.

⚑ **Die Lehre in einem Satz:** Erfundene Eingaben prüfen, was man sich
vorgestellt hat; echte prüfen, was man vergessen hat.

⚑ **Der Quervergleich, der am meisten sagt:** EPUB und HTML desselben
Buches ergeben **Byte für Byte denselben Text**. Zwei Auswertungswege,
dieselbe Ausgabe.

**Behoben und nachgemessen:** Vorspann abgetrennt und als Herkunft
benutzt, Rahmen an seinen Marken geschnitten (beide Marken oder keine),
Formatzusatz bei belegtem Namen, Teilung in zusammenhängenden Blöcken
von höchstens 20 Zeilen (32 % wurden 20 %, und der Bericht nennt Soll
und Ist), drei Wachen gegen Tabellenzeilen (55 erkannte Überschriften
wurden 32, **bei gleichem Wortbestand**).

### v0.4.0 – 2026-09-17 (die Buchkette: aus einem Buch wird Material)

`myl-train` unverändert; neu ist `korpus/` mit drei Python-Werkzeugen
(Auftrag des Projektinhabers).

⚑ **Aus einem Buch wird entweder Trainingsmaterial oder eine
Wissensmappe**, und beides hat seinen Weg: `buch_zu_md.py` nimmt das
Format weg und lässt die Gliederung stehen, `buchkorpus.py` baut Lern-
und Haltezeilen in der Form, die `trainingsguete` liest, und
`md_zu_mappe.py` baut eine Mappe zum Nachschlagen. **Ein Trainingslauf
wirkt danach ohne Kontext und kostet Stunden, eine Mappe wirkt sofort
und kostet je Frage ein Nachschlagen.**

⛔️ **Die wichtigste Zusage des Korpus: geteilt wird nach Abschnitten,
nicht nach Zeilen.** Ein Buch wiederholt seine Aussagen; wer zeilenweise
teilt, misst mit der Haltemenge das Gedächtnis statt der
Verallgemeinerung. Danach wird nachgeprüft, und zwar mit einer
**strengeren** Schwelle als beim Entdoppeln: Mit derselben konnte die
Prüfung gar nichts finden, weil das Entdoppeln vorher über den ganzen
Korpus läuft. **Eine Gegenprobe, die nicht beißen kann, ist keine.**

⚑ **Der Nachbar ist die Datenprovenienz**, und das ist der Grund, warum
die Kette hier liegt und nicht bei den Messwerkzeugen: Was hier
entsteht, ist ein Korpus, und ein Korpus wird verankert, nicht gemessen.
📌 Sie lag einen halben Tag unter `BENCHMARKS/Training/`, weil dort schon
ein `datasets/`-Ordner stand. **Das war die Nähe des Nachbarn und nicht
die Sache**, und aufgefallen ist es an einer Überschrift, die erklären
musste, dass ein Ordner zweierlei enthält.

⚠️ **PDF braucht `pdftotext` oder `pypdf`**, beides fehlt auf der
Entwicklungsmaschine; EPUB, HTML und Text gehen ohne fremde Kiste. **Bei
fehlendem Hilfsmittel gibt es eine Absage und keine leere Datei.**

### v0.3.3 – 2026-09-11 (was am Expertengemisch anders ist, und drei Entwürfe, die schon Code waren)

**Anlass:** Der Projektinhaber hat entschieden, dass Qwen3-30B-A3B und
nicht das dichte 27B-Modell die erste Netzinferenz trägt. Damit ist das
Primärmodell ein Expertengemisch, und `Konzept-Wachstum.md` deckt vier
Dinge nicht ab: wer einen Gradienten bekommt, wer darüber entscheidet,
wie ein Gemisch wächst und wo sein absorbierender Zustand sitzt. Neu:
[`Konzept-Gemischtraining.md`](Konzept-Gemischtraining.md).

📌 **Und der erste Durchgang dieses Dokuments war zu drei Vierteln
überflüssig.** Er entwarf eine Routerschranke, einen ganzzahligen
Ausgleichsterm und einen Operator für Expertenwachstum. **Alle drei
stehen seit dem 2026-08-28 in `kernels`** (`router_spreizung`,
`Expertenwacht`, `experte_einhaengen`), und alle drei sind besser gelöst
als der Entwurf. Das Dokument ist daraufhin neu geschrieben: **es ordnet
jetzt, statt zu entwerfen.**

⛔️ **Eine Aussage war schlicht falsch** und ist zurückgenommen: Den
absorbierenden Zustand des Routers gibt es in Gleitkomma **auch**, nur
später. Die Schwelle ist `(frac + 1) · ln2`, also 10,4 nats bei
`prob_frac_bits = 14` gegen 104 bei `f32`.

⚑ **Was das Dokument beiträgt, ist ein gemessener Zielkonflikt.** Die
Expertenverteilung des 30B ist schief (die häufigsten 20 % tragen
78,3 % der Aufrufe), und der Lastausgleich flacht sie ab. Auf einer
Maschine, deren Seitenpuffer kleiner ist als das Artefakt, entscheidet
genau diese Schiefe über den Durchsatz: warm 16 bis 35 GB/s, kalt 1,75
bis 5,2. **Wer `wacht_staerke` wählt, entscheidet zugleich über den
Durchsatz des Netzes**, und wie viel Güte ein Prozentpunkt Deckung wert
ist, ist nicht gemessen.

⚑ **Und `F` ist für ein 30B jetzt rechenbar**, ohne dass das Modell in
Gleitkomma passen müsste: Die Rasterstufe steht als `2^-shift` je Zeile
im Artefakt, die Schrittgrösse liefert der ganzzahlige Rückwärtspfad.

### v0.3.2 – 2026-09-10 (der Wegweiser zeigte auf etwas, das kein Klon hat)

📌 **Fund 275:** Drei Zeilen der Wegweisertabelle zeigten auf ein
Papier, das kein Klon dieses Repositoriums mitbekommt, und ein Wegweiser
ins Leere ist schlechter als keiner. Die
Tabelle zeigt jetzt nur noch dorthin, wo das Werkzeug liegt und wo die
Anleitung öffentlich steht.

⚑ **Die Kopfzeile nennt seit heute die Kiste**, also `myl-train` 0.3.0
neben der Komponentenversion. Beide sind auseinandergelaufen, als die
Komponente am 2026-09-09 für zwei berichtigte Sätze auf 0.3.1 ging und
die Kiste zu Recht stehen blieb: Am Code hat sich nichts geändert. Die
Abhakprobe verglich bis dahin beide miteinander und meldete eine
Abweichung, die keine war.

### v0.3.1 – 2026-09-09 (zwei überholte Sätze, und ein Wegweiser)

📌 **Diese Datei behauptete, INTEGER_LLM behandle „bislang
ausschliesslich Inferenz" und der ganzzahlige Rückwärtspass sei dort
„noch nicht implementiert".** Beides war zum Zeitpunkt des Schreibens
richtig und ist es seit Monaten nicht mehr. Ein Abhängigkeitseintrag,
der aus einer überholten Annahme stammt, schickt den Nächsten in die
Irre.

⚑ Neu ist ein Wegweiser darauf, wo das ganzzahlige Training
beschrieben ist, samt den drei Zahlen, die ein Lauf einhalten muss.
Diese Kiste bleibt, was sie ist: die Koordination im Netz, nicht die
Rechenart.

### myl-train v0.3.0 – 2026-09-05 (Fund 183: die Kiste bekommt ihren ersten Aufrufer)

`myl_train::zuweisung::zuweisen` wird jetzt aus
`myl_scheduler::trainingszuteilung` gerufen: Jeder Trainingspod bekommt
sein Korpusbündel aus der Epochensaat.

⚑ **Bis heute hatte diese Kiste ausserhalb ihrer eigenen Tests keinen
einzigen Aufrufer.** Datenprovenienz, Wachstumsoperator und
VRF-Zuweisung waren gebaut, geprüft und unerreicht: die bekannteste
Fehlerklasse dieses Projekts in ihrer grössten Ausprägung, nicht eine
Funktion ohne Aufrufer, sondern eine ganze Kiste.

Die Ursache war eine einzige: Es gab keinen Weg, auf dem ein Pod ein
Trainingssegment zugewiesen bekommt. `zuweisen` beantwortet die Frage
„welches Bündel bekommt Pod *i*?", und niemand stellte sie.

**Provenienz und Wachstum haben weiterhin keinen Aufrufer.** Sie hängen
an Punkten, die noch offen sind: der Segmentprüfung und dem
Wachstumsereignis.

### myl-train v0.2.0 – 2026-08-23 (Wachstumsoperator, Bitbudget, Tiefenwachstum)

Drei Punkte, alle drei ohne fremde Hardware machbar und alle drei
unabhängig vom ganzzahligen Rückwärtspass.

**1.2, der Wachstumsoperator** (`src/wachstum.rs`). Die ganzzahlige
Aufteilung `a = ⌊m/2⌋`, `b = m − a` statt der Halbierung, die die
Literatur vorsieht. `a + b = m` gilt für jede ganze Zahl, es wird nichts gerundet,
und bei jedem ungeraden Eintrag trennt die Aufteilung die beiden Kopien
um genau ein LSB, **ohne jeden Zufall**.

Dazu die Identitätsebene für das Tiefenwachstum (Ausgabegewicht null,
exakt darstellbar) und der Digest über Form **und** Werte.

**Das Akzeptanzkriterium ist ein Digestvergleich, kein Toleranzvergleich.**
Geprüft über alle drei Einheiten des Beispiels und über 200 zufällige
Matrizen aus einem reproduzierbaren xorshift: Die Ausgabe vor und nach der
Expansion ist bitgleich.

**Ein Detail, an dem es hätte scheitern können:** `⌊m/2⌋` muss
**abrunden**, auch bei negativen Zahlen. Rusts `/` trunkiert zur Null, und
`-3 / 2 = -1` ergäbe zwar `a + b = -3`, aber ein anderes `a` als die
Referenzsimulation mit `torch.floor`. Zwei Implementierungen desselben
Operators müssen dasselbe liefern, sonst ist der Digestvergleich wertlos.
Als Test festgehalten.

**1.1, das Bitbudget** (`tests/diag/results/bitbudget_uebersicht.md`).
Vier Lernraten gemessen statt einer:

| Lernrate | F empfohlen | W_master | Wort |
|---|---|---|---|
| 1e-3 | 19 | 27 | **int32** |
| 1e-4 | 22 | 30 | **int32** |
| 1e-5 | 25 | 33 | int64 |
| 1e-6 | 29 | 37 | int64 |

**Die Grenze zwischen int32 und int64 liegt zwischen 1e-4 und 1e-5.** Der
bisherige einzelne Messpunkt bei 1e-5 empfahl int64, und das stimmt dort
und nur dort. Der Master ist die größte Datenstruktur des
Trainingsschritts; ob er 32 oder 64 Bit breit ist, entscheidet über die
Hälfte seines Speichers.

Die Abhängigkeit ist auch herleitbar (`log2(10) = 3,32` Bit je Faktor 10)
und stimmt mit der Messung überein: 3, 3 und 4 Bit über die drei
Übergänge. **Die Modellgrößen-Achse bleibt bei einem Punkt**, weil 7B in
float32 rund 30 GB bräuchte und diese Maschine 24 GB hat.

**1.3, das Tiefenwachstum** (`tests/diag/tiefenwachstum_simulation.py`).
Die Frage war, ob eine als Identität startende Ebene tot bleibt.

**Sie bleibt es nicht**, und zwar ab dem ersten Schritt: Der Gradient nach
dem Ausgabegewicht ist `aᵀ·g` und hängt nicht vom Ausgabegewicht ab.
Über 20 Schritte bewegen sich mit stochastischem Runden 120 von 128
Gewichten, mit Rundung zur nächsten Stufe 33.

**Warum der Unterschied so groß ist**, und das ist ein eigener Befund:
Das stochastische Runden verändert auch die *Eingangs*gewichte je
Schritt, damit die Aktivierungen und damit, welche Einträge überhaupt
einen Gradienten sehen. Mit Rundung zur nächsten Stufe sind die
Aktivierungen über alle Schritte gleich, und ein Drittel der Einträge
bekommt **nie** einen.

**Fund dabei:** `Konzept-Wachstum.md` führte diese Messung seit dem
2026-08-22 als erledigt, mit konkreten Zahlen und einem Beleg, den es
nicht gab. Das genannte Skript misst ausschließlich Breitenwachstum, ein
Protokoll mit diesen Zahlen existierte nirgends. Der Punkt war zu Recht
als „nicht gemessen" geführt. Dieselbe Klasse wie Fund 27
und Fund 37; das Konzept trägt jetzt die gemessenen Zahlen und den
Vermerk.

### myl-train v0.1.0 – 2026-08-23 (Punkt 3.1: Datenprovenienz)

**Die erste Zeile Code dieser Komponente.** Bis hierher bestand TRAINING
aus Planung, Konzept und Diagnoseskripten; Kritikpunkt K7 führte sie
als eine von drei Komponenten ohne Code.

Gebaut ist Kap. 7.3, **Herkunft statt Inhalt**: Ein Miner, der
vergiftete Texte einspeist, rechnet bitgleich korrekt. Der Bitvergleich
aus Kap. 6 greift dort nicht, und eine inhaltliche Bewertung wäre genau
der subjektive Spielraum, den das Protokoll sonst vermeidet.

- **`provenienz`** — Korpora über eine Merkle-Wurzel verankern,
  Segmente per Beweis referenzieren statt per Rohdaten, Bündel
  zusammenhängender Segmente. Für eine nicht existierende Position gibt
  es keinen Beweis, und zwar nicht, weil die Prüfung ihn ablehnt,
  sondern weil er sich nicht erzeugen lässt.
- **`zuweisung`** — welcher Pod welche Abschnitte bearbeitet, folgt aus
  dem Epochen-Seed, nicht aus einer Wahl. Wer keine Daten fälschen kann,
  kann sonst immer noch auswählen: Bei freier Wahl entspräche der
  Einfluss dem Kapazitätsanteil (Anhang B.6.5).

**Der Seed wird hier nicht erzeugt**, sondern als 32 Bytes
entgegengenommen. Eine zweite Stelle, die Seeds erzeugt, wäre eine
zweite Quelle für dieselbe Aussage; die Bindung an den finalisierten
Block und die Epochennummer gehört genau einmal in den Scheduler
(Fund A20). Deshalb steht `myl-scheduler` auch nicht im Manifest.

**Ein Test, den es braucht:** Eine Referenz darf **nicht** gegen ihre
eigene mitgebrachte Wurzel geprüft werden, sonst baut sich ein Angreifer
mit selbstgewählten Daten einen gültigen Beweis. Das war Audit-Fund A11,
eine Ebene höher.

**Fund dabei: Anhang B.6.4 gibt gebündelte Beweise zu teuer an.** Wer
alle Blätter eines vollständigen Teilbaums hat, braucht für dessen
untere Ebenen keinen Geschwisterknoten; übertragen wird nur der Weg von
der Teilbaumwurzel zur Baumwurzel.

| Segmente | Knoten | Bytes | gerechnet | Anhang B.6.4 |
|---|---|---|---|---|
| 1 | 30 | 960 | 11,72 % | **11,7 %** ✅ |
| 16 | 26 | 832 | 0,63 % | **1 %** |
| 256 | 22 | 704 | **0,034 %** | **0,42 %** |

Der Einzelbeweis stimmte genau, dort gibt es keine Bündelung. Die
Abweichung ging in die sichere Richtung, der Anhang gab das Verfahren
teurer an, als es ist. **Das Papier ist am selben Tag korrigiert worden**
(DE und EN, MD und PDF); die gerechneten Werte stehen zusätzlich als
Test, damit Papier und Code nicht wieder auseinanderlaufen.

**Nicht gebaut:** die Ablehnungsquote für verweigerte Segmente. Sie ist
eine Buchführung über das Verhalten eines Miners über Epochen hinweg und
gehört zum Ledger.


Noch keine Version veröffentlicht.
