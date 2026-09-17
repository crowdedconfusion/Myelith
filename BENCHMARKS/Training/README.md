# Trainingsmessung

Zwei Erzeuger, zwei Fragestellungen.

## ⚑ Was hier liegt, und was nicht

Hier stehen **Messinstrumente mit bekannter Wahrheit**: erfundene Daten,
deren richtige Antwort feststeht, damit eine Zahl am Ende etwas heisst.
Sie beantworten „kann das Training ueberhaupt etwas" und „was wurde
tatsaechlich abgelegt".

⛔️ **Werkzeuge, die echtes Material herstellen, gehoeren nicht hierher.**
Die Buchkette (aus einem Buch wird ein Trainingskorpus oder eine
Wissensmappe) liegt unter `TRAINING/korpus/`. Sie misst nichts, sondern
stellt her, und ihr Nachbar ist die Datenprovenienz in `myl-train`.

📌 **Sie lag einen halben Tag hier**, weil nebenan schon ein
`datasets/`-Ordner stand. Das war die Naehe des Nachbarn und nicht die
Sache selbst; aufgefallen ist es an der Ueberschrift „Zwei Sorten
Werkzeug hier", die dieser Abschnitt ersetzt hat. **Wer erklaeren muss,
dass ein Ordner zweierlei enthaelt, hat das zweite meist falsch
abgelegt.**

---

## `referenzleiter.py`: wo endet die Fähigkeit?

Fünf Stufen wachsender Schwierigkeit, je 4000 Lern- und 800 **frische**
Haltezeilen mit derselben Regel und disjunkten Beispielen.

| Stufe | Regel | Rolle |
|---|---|---|
| 1 Kopie | `TANOLGRA ist TANOLGRA .` | die Untergrenze |
| 2 Regel | Endbuchstabe bestimmt die Farbe | echte Verallgemeinerung |
| 3 Umkehr | `ABCDE rueckwaerts ist EDCBA .` | Abbildung über der Zeichenfolge |
| 4 Add2 | `47 + 82 = 129 .` | Übertragslogik |
| 5 Add3 | `347 + 512 = 859 .` | obere Schranke, keine Erwartung |

⚑ **Warum eine Leiter und nicht ein Satz.** Ein einzelner Datensatz
liefert ja oder nein. Die Leiter sagt, **wo** die Fähigkeit endet, und
das ist der Unterschied zwischen einem Befund und einer Diagnose.

## `wissenssaat.py`: was wurde tatsächlich abgelegt?

60 erfundene Personen mit erfundenen Lebensläufen und ein erfundenes
Ereignis, jede Tatsache in mehreren Formulierungen, dazu 320
Prüfungsfragen mit bekannter Antwort.

⚑ **Die Umformulierungen sind kein Beiwerk.** Wird eine Tatsache nur in
einer Form trainiert, speichert ein Modell sie, kann sie aber nicht
abrufen, in der Messung der zugrundeliegenden Arbeit bis hinunter auf
null Prozent richtige Antworten.

Fünf Fragearten, und sie messen verschiedene Ausprägungen:

| Art | Prüft | Erwartung |
|---|---|---|
| `fortsetzung` | blosse Fortsetzung, **ohne jedes Schema** | ⚑ siehe unten |
| `direkt` | dieselbe Formulierung wie im Training | leicht |
| `umformuliert` | dieselbe Tatsache, andere Frage | echter Abruf |
| `umkehr` | Antwort gegeben, Person gesucht | schwer |
| `fremd` | eine Person, die **nicht** trainiert wurde | ⚑ muss scheitern |

⚑ **`fremd` ist die wichtigste Zeile.** Eine Prüfung, die nur Treffer
zählt, besteht auch ein Modell, das immer dieselbe Antwort rät. Bleiben
die fremden Personen nicht unbeantwortet, misst die Prüfung das Format
und nicht das Wissen.

### ⚑ Warum `fortsetzung` dazugehört, und was ohne sie nicht zu trennen ist

Ein Modell kann eine Tatsache gebunden haben und trotzdem an
`Frage: ... Antwort:` scheitern, weil es nie gelernt hat, was an dieser
Stelle erwartet wird. **Das wäre eine Formatlücke und keine
Wissenslücke**, und ohne eine schemafreie Frage sehen beide gleich aus,
nämlich nach null richtigen Antworten.

📌 **Am 2026-09-07 gemessen, und die erste Fassung dieses Satzes hatte
den Unterschied nicht:** Ein Lauf lernte die Tatsachen nachweisbar, die
Haltemenge fiel um acht Prozent gegen einen Rauschnullpunkt von 0,04,
**und beantwortete keine einzige Frage**. Erst die Fortsetzungsfrage
sagt, welches der beiden Probleme vorliegt.

## Gemessen wird mit zwei Massen

**Treffer** heisst, die gierige Fortsetzung enthält die erwartete
Zeichenfolge. **Rang** ist die Position des erwarteten Tokens in der
Logit-Ordnung. ⚑ Der Rang fällt lange bevor der Treffer kommt und zeigt
Lernen, das noch nicht durchschlägt.

### 📌 Der Rang prüft BEIDE Schreibweisen, und das war ein Tagesfehler

BPE kodiert ein Wort **am Zeichenkettenanfang anders als nach einem
Leerzeichen**, und im Satz folgt immer die zweite Variante. Wer nur
`erwartet` kodiert, misst ein Token, das an der geprüften Stelle
grammatisch nicht stehen kann.

**Die Gegenprobe an einer Tatsache, die das Modell sicher kennt:**

| Erwartung | Rang |
|---|---|
| `Paris` | **558** |
| ` Paris` | **0** |

⚑ Gemessen wird deshalb der **bessere** beider Ränge. Und wer eine
solche Messung neu baut, prüft sie **zuerst an etwas, das das Modell
sicher weiss**. Diese Prüfung kostet Sekunden; sie zu überspringen
kostete einen Tag voller Rangzahlen, die nichts bedeuteten.

## 📌 Und eine Lehre vom 2026-09-08, die hierher gehört

**Die Messläufe dieses Tages benutzten einen selbstgebauten Korpus mit
einer einzigen Person**, nicht `wissenssaat.py`. Das sah harmlos aus und
war der Grund, warum ein Tag lang die falsche Frage gemessen wurde.

⚑ **Aus einem einzigen Beispiel kann kein Gradient lernen, dass es auf
die Person ankommt.** Die einfachste Hypothese, die zu allen Daten
passt, lautet „Geheimwort heißt Goldfisch", und genau die lernt das
Modell. Sichtbar wurde es erst, als eine `fremd`-Probe dazukam: Faktor
1,9 für eine **untrainierte** Person gegen 2,3 für die Zielperson.

**`wissenssaat.py` hatte diese Probe von Anfang an**, und sein eigener
Kopf nennt sie „die wichtigste Zeile". Sie zu haben und nicht zu
benutzen ist teurer als sie nicht zu haben, denn wer sie nicht hat,
weiß es.

| Was der Korpus braucht | warum |
|---|---|
| **mehrere** Personen mit verschiedenen Antworten | sonst gibt es keinen Grund, auf die Person zu achten |
| mehrere Formulierungen je Tatsache | Allen-Zhu und Li: eine Formulierung speichert, ohne abrufbar zu machen |
| eine `fremd`-Probe | sonst ist ein Treffer nicht von einer Verallgemeinerung zu unterscheiden |
| allgemeinen Text dazu | gegen die Erosion des Vorwissens |

⚑ **Die letzte Zeile fehlt dem Generator**, und das ist kein Mangel an
ihm: Er erzeugt Lernstoff, keine Mischung. Wer damit misst, mischt
selbst, und schreibt dazu, in welchem Verhältnis.
