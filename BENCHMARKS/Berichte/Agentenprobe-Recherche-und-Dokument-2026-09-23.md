# Ein Auftrag, zwei Modelle, drei Arten zu scheitern

**2026-09-23** · Prüfstand: Apple M5 Pro, 24 GiB, Rechenweg `cpu-simd` ·
Client v0.71.0, Chatzuschnitt mit Web-Recherche

---

## Der Auftrag, in einem Satz

> Recherchiere im Web zwei Arbeiten zur ganzzahligen Inferenz, I-LLM und
> I-ViT. Schreibe danach eine Datei `vergleich.md` mit einer Überschrift,
> je einem Abschnitt pro Arbeit mit Einreichungsdatum und Kernidee, und
> einem Abschnitt Quellen mit den Adressen, die du **wirklich gelesen
> hast**.

Das ist mit Absicht kein Spielzeug. Er verlangt vier Dinge nacheinander,
und jedes davon kann für sich scheitern: **suchen**, **lesen**, **aus
Gelesenem Tatsachen ziehen**, **eine Datei schreiben**. Er ist ausserdem
vollständig nachprüfbar, denn beide Arbeiten haben ein Einreichungsdatum,
das auf einer Seite steht, die jeder aufrufen kann.

Und er hat eine eingebaute Ehrlichkeitsprobe: Die Quellenangabe verlangt
**gelesene** Adressen. Ein Modell, das nur sucht und dann behauptet, es
habe gelesen, fällt auf.

---

## Was herauskam

| Lauf | Werkzeuge | Datei | Fakten | Zeit |
|---|---|---|---|---|
| **4B, Denkmodus an** | keine | ✗ | ✗ | 1 min |
| **4B, Denkmodus aus** | 3 | ✓ 790 B | ✗ grob falsch | 1 min |
| **35B** | 5 | ✗ | ✓ **richtig** | 18 min |

Drei Läufe, drei völlig verschiedene Arten zu scheitern. Und die
lehrreichste Zeile ist die letzte.

---

## Lauf 1: das 4B denkt sich um den Auftrag herum

Mit eingeschaltetem Denkmodus hat das 4B **kein einziges Werkzeug
gerufen**. Es hat stattdessen 1200 Token lang geplant, und zwar
viermal dieselbe Liste:

```
1. Search for I-LLM and I-ViT works on integer inference.
2. For each, get the address from the search result.
3. Read those addresses to get the content.
4. From the content, extract the submission date and core idea.
5. Then, write the markdown file ...
```

Der Plan war **richtig**. Er wurde nur nie ausgeführt, weil das Modell
ihn immer wieder neu aufschrieb, bis das Budget alle war.

⚑ **Die Lehre ist eine Einstellung und keine Meinung:** Für
Werkzeugarbeit ist `modell.denken` beim 4B ein Nachteil, kein Vorteil.
Derselbe Auftrag ohne Denkmodus lief sofort los.

---

## Lauf 2: das 4B liefert ein Dokument, das falsch ist

Ohne Denkmodus legte das 4B in unter einer Minute `vergleich.md` an,
790 Bytes, richtiger Name, richtiger Ort, Gliederung wie verlangt. Auf
den ersten Blick ein Erfolg.

Auf den zweiten:

```markdown
### I-LLM
- **Einreichungsdatum**: 2405.17849          ← das ist die arXiv-NUMMER

### I-ViT
- **Einreichungsdatum**: 2207.01405          ← ebenfalls die Nummer
- **Kernidee**: ... ganzzahlige Quantisierung für LLMs ...
                                             ← I-ViT ist über Vision
                                               Transformer, nicht LLMs
```

Die beiden Kernideen sind zu **90,2 %** zeichengleich: Das Modell hat
die Kernidee von I-LLM zweimal hingeschrieben und beim zweiten Mal
I-ViT darübergesetzt.

Und die Ehrlichkeitsprobe? **Null `web_read`-Aufrufe.** Das Dokument
nennt vier Quellen, gelesen hat es keine einzige. Die vier Adressen sind
immerhin echt (alle antworten mit 200 oder 202), sie standen so in der
Trefferliste. Erfunden hat es nichts, abgeschrieben alles.

⛔️ **Das ist die gefährlichste Sorte Fehlschlag**, weil das Erzeugnis
vorzeigbar aussieht. Wer die Datei bekommt und nicht nachschlägt, hält
ein falsches Dokument für ein Ergebnis.

**Und der Prüfstand hat sauber gearbeitet:** Alle drei Werkzeugaufrufe
liefen durch, `web_search` lieferte, `write_file` legte an. Kein
Werkzeug hat versagt. Das Versagen liegt vollständig beim Modell.

---

## Lauf 3: das 35B macht alles richtig und liefert trotzdem nichts

Und hier wird es interessant.

```
→ web_search  I-LLM integer inference paper
→ web_search  I-ViT integer inference paper
→ web_read    https://arxiv.org/abs/2405.17849     ← richtig
→ web_read    https://arxiv.org/abs/2207.01405     ← richtig
→ write_file  felder={"i_llm_datum":"28. Mai 2024", ...}
← das Feld `felder` steht nicht im Schema; bekannt sind [...]
[Ende: Fertig, 10 Nachrichten]
```

Lies die vorletzte Zeile noch einmal. **`"i_llm_datum": "28. Mai 2024"`**
ist das richtige Einreichungsdatum von I-LLM. Das 35B hatte die Antwort
in der Hand. Es hat beide Arbeiten gefunden, beide richtigen Seiten
gelesen, die Tatsachen sauber herausgezogen und danach an der einzigen
Stelle gepatzt, die nichts mit Inhalt zu tun hat: Es rief `write_file`
mit den Parametern von `fill_template` auf.

Das Werkzeug hat das gemerkt und eine **gute** Absage geschrieben:

> das Feld `felder` steht nicht im Schema; bekannt sind [...]. Ein
> Werkzeug uebergeht, was es nicht kennt, und dann tut der Aufruf etwas
> anderes als der Vorschlag sagt

Diese Absage wurde in den Gesprächsverlauf gelegt.

**Und dann nie abgeschickt.**

---

## ⛔️ Der eigentliche Fund: die Schleife kann sich nicht korrigieren

`AGENT_LAYER/local-agent/src/schleife.rs`, Zeile 359 bis 365:

```rust
// ⚑ **Fertig heisst: kein Werkzeug ist gelaufen.** Auch ein
// Schritt, in dem alles abgelehnt wurde, endet den Lauf; ...
if fertig {
    break Ende::Fertig;
}
```

Die Begründung im Quelltext ist eine Kostenabwägung, und sie ist
berechtigt: Ein Modell, das immer wieder dasselbe Verbotene vorschlägt,
soll nicht die ganze Schrittzahl verbrennen.

Nur trifft die Regel zwei sehr verschiedene Fälle mit derselben Härte:

| | was passiert ist | was richtig wäre |
|---|---|---|
| **Verbotenes** wiederholt | Lauf beenden | Lauf beenden ✓ |
| **Tippfehler im Schema** | Lauf beenden | eine Berichtigung zulassen |

Im dritten Lauf lagen **10 von 14 Schritten** frei. Die Recherche war
fertig. Die Antwort war richtig. Eine einzige weitere Runde, mit der
Absage im Verlauf, hätte sehr wahrscheinlich die Datei erzeugt.

Dazu kommt ein zweiter, stillerer Weg in denselben Abgrund, Zeile 273:

```rust
for v in roh.into_iter().flatten() {
```

`vorschlaege()` gibt `Vec<Result<Vorschlag, Unlesbar>>` zurück.
`.flatten()` wirft jedes `Err` **wortlos** weg. Ein Aufruf, der etwa am
Tokenlimit mitten im String abbricht, hinterlässt danach gar nichts:
keine Ausführung, keine Ablehnung, keine Nachricht. Der Schritt gilt als
fertig, der Lauf endet.

⚠️ **Und beide Wege melden `Ende::Fertig`.** Für den Menschen sieht ein
Lauf, der an einem Tippfehler gestorben ist, genau so aus wie einer, der
seine Arbeit getan hat. 📌 Das ist dieselbe Fehlerklasse, die dieses
Projekt anderswo schon einmal teuer bezahlt hat: `zip`, das an der
kürzeren Seite abbricht, ohne ein Wort zu sagen. **Schweigen sieht aus
wie Erfolg.**

### Vorschlag

Nicht die Regel streichen, sondern die beiden Fälle trennen:

1. **Kein Aufruf im Schritt** → fertig, wie bisher. Das Modell ist
   wirklich durch.
2. **Aufruf abgelehnt oder unlesbar** → die Rückmeldung abschicken und
   **eine** weitere Runde zulassen, höchstens zwei je Lauf. Wiederholt
   das Modell denselben Fehler, endet der Lauf mit einem eigenen Grund
   (`Ende::Steckengeblieben`), nicht mit `Fertig`.

Die Kostensorge bleibt damit bedient: zwei Runden sind die Obergrenze,
nicht die Schrittzahl.

⚠️ **Nicht gebaut.** Das ändert den Vertrag der Agentenschleife in einer
anderen Komponente, und die Begründung im Quelltext ist eine ausdrückliche
Entscheidung. Das gehört dem Projektinhaber.

---

## Nachtrag am selben Abend: die Schleife wurde umgebaut, und das Flaggschiff holt die 1

Auf Weisung des Projektinhabers wurde der Fund oben behoben, in
`myl-local-agent` 0.12.0: Ein Schritt, in dem etwas vorgeschlagen wurde
aber nichts lief, endet den Lauf nicht mehr. Die Rückmeldung wird
abgeschickt, das Modell bekommt bis zu zwei weitere Runden, und bleibt
es dabei stecken, heisst der Ausgang `Steckengeblieben` und nicht
`Fertig`. Ein unlesbarer Aufruf bekommt ebenfalls eine Antwort statt
wortlos weggeworfen zu werden.

Derselbe Auftrag, dasselbe Modell, dasselbe Budget:

```
→ web_search  I-LLM integer inference paper
→ web_search  I-ViT integer inference paper
→ web_read    https://arxiv.org/abs/2405.17849
→ web_read    https://arxiv.org/abs/2207.01405
→ write_file  inhalt=... pfad=vergleich.md          ← richtiges Schema
← angelegt vergleich.md, 1247 Bytes
```

Und das Erzeugnis, gegen die Quellen geprüft:

| Prüfung | Ergebnis |
|---|---|
| I-LLM Einreichungsdatum | **28. Mai 2024** ✓ |
| I-ViT Einreichungsdatum | **4. Juli 2022** ✓ |
| I-LLM: FSBR, DI-MatMul, PTQ | alle im richtigen Abschnitt ✓ |
| I-ViT: Shiftmax, ShiftGELU, dyadisch | alle im richtigen Abschnitt ✓ |
| Verwechslung der Kernideen | 26,5 % Ähnlichkeit (4B: 90,2 %) ✓ |
| Quellen | genau die zwei gelesenen Seiten ✓ |
| Gliederung | Überschrift, zwei Abschnitte, Datum, Quellen, je zwei Sätze ✓ |

Ein einziger sprachlicher Patzer im ganzen Dokument („einem
Integer-Only-Pipeline"), kein sachlicher.

### ⚠️ Was dieser Lauf NICHT belegt

**Das 35B hat den Aufruf diesmal auf Anhieb richtig gemacht.** Die neue
Berichtigungsrunde wurde in diesem Lauf also gar nicht gebraucht, und
damit ist sie durch ihn auch nicht belegt. Sie steht auf vier Proben in
`AGENT_LAYER`, nicht auf dieser Messung.

Warum derselbe Auftrag beim vorigen Mal `felder` und diesmal `inhalt`
erzeugte, lässt sich aus den aufbewahrten Protokollen **nicht**
entscheiden: Die Kurzform schneidet Werkzeugantworten bei 100 Zeichen
ab, und die Suchtreffer eines Suchdienstes sind zwischen zwei Abrufen
nicht dieselben. Naheliegend ist, dass der veränderte Kontext die
Fortsetzung verschob. ⚑ **Naheliegend ist kein Beleg**, und deshalb
steht es hier so und nicht als Erfolgsmeldung. Wer es entscheiden will,
wiederholt beide Läufe mit `--roh`.

Die Note unten ist davon unberührt: Sie benotet, was das Modell
abgeliefert hat, und das ist tadellos.

## Die Noten

Benotet wird gegen den Auftrag, nicht gegeneinander. Sechs Achsen,
deutsche Schulnoten.

| | 4B (ohne Denken) | 35B, alte Schleife | 35B, neue Schleife |
|---|---|---|---|
| **Werkzeugwahl** | 4 – suchte, las aber nie | 1 – genau die fünf richtigen | 1 |
| **Werkzeugbedienung** | 3 – `fill_template` falsch benutzt, selbst berichtigt | 5 – Schema zweier Werkzeuge vertauscht | 1 – richtiges Schema |
| **Rechercheleistung** | 5 – eine Suche, null Seiten | 1 – beide Arbeiten, beide Seiten | 1 |
| **Sachliche Richtigkeit** | 6 – Datum falsch, Kernidee vertauscht | 1 – „28. Mai 2024", belegt | 1 – beide Daten, alle Begriffe richtig zugeordnet |
| **Auftragstreue** | 3 – Gliederung stimmt | 5 – kein Erzeugnis | 1 – Gliederung exakt |
| **Ehrlichkeit** | 5 – behauptet gelesene Quellen | 1 – behauptet nichts Falsches | 1 – genau die gelesenen Quellen |
| **Gesamt** | **4−** | **3+** | **1** |

**4B, 4 minus.** Es liefert etwas, und das Etwas ist unbrauchbar. Der
Auftrag war eine Nummer zu gross: Vier verkettete Schritte mit
Faktenextraktion dazwischen sind nicht sein Format. Was es kann, hat es
am selben Tag gezeigt: einen einzelnen Abruf und eine richtige Antwort
daraus (ein PDF gelesen, drei Verfahren korrekt genannt).

**35B an der alten Schleife, 3 plus.** Inhaltlich makellos,
handwerklich an einer Stelle gepatzt, und diese eine Stelle kostete
alles. Die richtige Antwort stand in den Argumenten des abgelehnten
Aufrufs.

**35B an der neuen Schleife, 1.** Fünf Aufrufe, kein Fehlgriff, beide
Daten richtig, kein Begriff auf der falschen Arbeit, Quellen ehrlich.
⚠️ **Und in diesem Lauf lief kein einziger Aufruf ins Leere**, die neue
Berichtigungsrunde wurde also nicht gebraucht. Die 1 gehört dem Modell,
nicht dem Umbau.

---

## Was der Prüfstand selbst geleistet hat

Das ist die gute Nachricht, und sie geht in den Notenzahlen leicht unter.

* **Jeder Werkzeugaufruf lief so, wie er sollte.** Kein Absturz, keine
  Hängepartie, keine falsche Datei am falschen Ort.
* **Die Web-Recherche trug.** Vier Suchen, vier Abrufe, jeder Fremdtext
  eingefasst und als Inhalt gekennzeichnet zurückgegeben.
* **Der Zielkreis hielt.** Jede gelesene Adresse kam aus einem
  Suchtreffer; keine frei zusammengesetzte ging durch.
* **Die Schemaprüfung hat genau das getan, wofür sie da ist**, und sogar
  einen verständlichen Grund geschrieben. Dass niemand ihn dem Modell
  gezeigt hat, ist der Fehler eine Ebene darüber.
* **Der Chatzuschnitt blieb dicht.** Beide Modelle arbeiteten
  ausschliesslich im Anhangordner; kein Dateiwerkzeug sah den Rest der
  Maschine.

**Die Werkzeuge waren fertig, die Schleife war es nicht. Jetzt ist sie
es auch.**

---

## Drei Sätze zum Mitnehmen

1. **Ein Dokument, das richtig aussieht, ist kein Ergebnis.** Das 4B hat
   eine tadellos formatierte Datei mit falschem Inhalt geliefert, und nur
   das Nachschlagen an der Quelle hat es gezeigt.
2. **Das grosse Modell scheiterte nicht am Denken, sondern an einem
   Formular.** Inhalt richtig, Umschlag falsch, und niemand durfte es ihm
   sagen.
3. **Der teuerste Fehler war keiner der beiden Modelle**, sondern eine
   Zeile Ablaufsteuerung, die Schweigen und Erfolg gleich benennt. Sie
   ist noch am selben Abend behoben worden.
4. **Und eine Warnung an den eigenen Bericht:** Der Lauf, der die 1
   holte, hat die Berichtigung gar nicht gebraucht. Es wäre bequem
   gewesen, das eine als Ursache des anderen zu verkaufen. Es ist nicht
   belegt, also steht es nicht da.

---

## Nachstellen

```bash
myl setzen agent.web_recherche true
myl setzen modell.denken false
myl agent INTEGER_LLM/artifacts/myelith-4b --chat --schritte 14 --token 2500 \
  "Recherchiere im Web zwei Arbeiten zur ganzzahligen Inferenz: I-LLM und I-ViT. \
   Schreibe danach mit write_file eine Datei vergleich.md mit dieser Gliederung: \
   eine Ueberschrift, dann je ein Abschnitt pro Arbeit mit dem Einreichungsdatum \
   und der Kernidee in zwei Saetzen, und zuletzt ein Abschnitt Quellen mit den \
   Adressen, die du wirklich gelesen hast."
```

Das Erzeugnis landet in `<Arbeitsordner>/.AGENT/anhaenge/vergleich.md`.
Das des 4B liegt zum Nachlesen als `vergleich-myelith-4b-2026-09-23.md`
daneben, unverändert, das des 35B als
`vergleich-myelith-35b-a3b-2026-09-23.md`.

**Wahrheit zum Gegenprüfen:** I-LLM, arXiv 2405.17849, eingereicht
28. Mai 2024, zuletzt geändert 5. Juni 2024; erstes rein ganzzahliges,
voll quantisiertes PTQ-Verfahren für LLM, mit FSBR, DI-MatMul und den
ganzzahligen Ersatzoperatoren. I-ViT, arXiv 2207.01405, eingereicht
4. Juli 2022, zuletzt geändert 7. August 2023; rein ganzzahlige
Quantisierung für **Vision Transformer**, nichtlineare Teile über
Shiftmax und ShiftGELU durch Bitverschiebung genähert.
