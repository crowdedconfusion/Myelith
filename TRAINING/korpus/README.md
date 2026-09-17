# Die Buchkette: aus einem Buch wird Material

```text
Buch (.epub/.pdf/.txt/.html)
   │  buch_zu_md.py          Format weg, Gliederung bleibt
   ▼
Markdown
   ├─ buchkorpus.py          Lern- und Haltezeilen  →  in die Gewichte
   └─ md_zu_mappe.py         Wissensmappe           →  neben den Kontext
```

⚑ **Die beiden Ausgänge sind kein Entweder-oder.** Ein Trainingslauf
wirkt danach ohne Kontext und kostet Stunden; eine Mappe wirkt sofort
und kostet bei jeder Frage ein Nachschlagen. **Schnell und dauerhaft
sind zwei Ziele**, und es gibt für beide einen Weg.

⚑ **Warum die Kette hier liegt und nicht bei den Messungen.** Sie misst
nichts, sie **stellt her**. Ihr Nachbar ist die Datenprovenienz in
`myl-train`: Was hier entsteht, ist ein Korpus, und ein Korpus wird
verankert, nicht gemessen. 📌 Sie lag einen halben Tag unter den
Messwerkzeugen, weil dort schon ein `datasets/`-Ordner stand; das war
die Nähe des Nachbarn und nicht die Sache.

## Einrichten, und warum meistens gar nichts einzurichten ist

⚑ **Die Kette braucht nichts zwingend.** EPUB, HTML, Text und Markdown
gehen mit der Standardbibliothek; ein frischer Klon kann sofort
arbeiten. Zwei Dinge machen sie besser, und beide sind freiwillig:

| Baustein | Wofür | Ohne ihn |
|---|---|---|
| `pypdf` (oder `pdftotext`) | PDF auslesen | PDF wird **abgelehnt**, mit Ansage |
| `tokenizers` | Token **zählen** statt schätzen | Es wird geschätzt, und der Bericht sagt es |

```bash
sh TRAINING/korpus/einrichten.sh        # legt .venv an, ohne Netz
python3 TRAINING/korpus/umgebung.py     # sagt, was hier geht
```

⛔️ **Das Einrichten läuft ohne Netz**, und das ist der Sinn: Die Räder
liegen unter `raeder/` im Repositorium, `pip` bekommt `--no-index` und
sieht deshalb nur sie. **Nachgeprüft an einem frischen Klon mit
blockiertem Netz** (Proxy auf einen toten Port): Einrichten, drei
Selbsttests, PDF und EPUB umgewandelt, Korpus und Mappe gebaut, alles
ohne eine einzige Verbindung.

📌 **Beim ersten Versuch scheiterte genau das**, und der Fehler ist
lehrreich: Die Räder waren mit `--no-deps` geholt, und `pypdf` braucht
unter Python vor 3.11 noch `typing_extensions`. **„Alle Abhängigkeiten"
heißt auch die, die eine Abhängigkeit selbst mitbringt**, und sie hängt
hier sogar von der Python-Fassung ab. Wer Räder nachlegt, tut es mit der
**ältesten** Fassung, die getragen werden soll.

### ⛔️ Warum kein Python mitgeliefert wird

Ein Interpreter im Repositorium wäre je Plattform ein eigenes Paket von
zig Megabyte, das niemand prüft und das bei jeder Sicherheitslücke
nachgezogen werden müsste. **Ein Repositorium, das eine Laufzeit
mitliefert, liefert auch deren Lücken mit**, und zwar so lange, bis
jemand daran denkt.

⚑ **Stattdessen liegt die Anforderung so tief, dass sie überall erfüllt
ist: Python 3.9.** Das ist die Fassung, die macOS selbst mitbringt und
die in jeder gepflegten Linux-Verteilung seit Jahren steht. ⚑ **Und sie
ist gemessen und nicht geraten:** Sie kommt vom jüngsten benutzten
Sprachmittel, der Vereinigung zweier Wörterbücher mit `|`.

⚑ **Geprüft wird sie auch**, statt an einer unverständlichen Meldung
aufzufallen: Jedes Werkzeug hält an und sagt, was es braucht und was es
vorfindet.

⚠️ **`tokenizers` liegt bewusst nicht dabei.** Es ist übersetzt und damit
je Plattform ein eigenes Rad; drei Plattformen wären rund zwanzig
Megabyte im Repositorium für eine Genauigkeit, die der Bericht sonst als
Schätzung ausweist. Wer es hat, dessen Zahlen sind exakt.

---

## ⚑ An echtem Material geprüft (2026-09-17)

**Drei Bücher in drei Formaten**, alle frei verfügbar: Kafkas „Die
Verwandlung" als EPUB und als HTML, dazu ein wissenschaftliches Papier
als PDF (zweispaltig, mit Formeln und Tabellen).

⚑ **Der Quervergleich, der am meisten sagt:** EPUB und HTML desselben
Buches ergeben **Byte für Byte denselben Text** (19 286 Wörter, vier
Überschriften). Zwei Auswertungswege, dieselbe Ausgabe.

⛔️ **Fünf Fehler, und keinen davon hätte ein Selbsttest gefunden.** Sie
stehen hier, weil sie zusammen eine Lehre ergeben: **Erfundene Eingaben
prüfen, was man sich vorgestellt hat; echte prüfen, was man vergessen
hat.**

| Was | Wie es aufgefallen ist |
|---|---|
| **Der eigene Kopf wurde zur Trainingszeile** | Die erste Zeile des Korpus lautete „quelle: verwandlung.epub sha256: 0f87a4 …". `buch_zu_md.py` schreibt einen YAML-Vorspann, `buchkorpus.py` kannte ihn nicht. ⚑ **Ein Werkzeug, dessen Ausgabe das nächste vergiftet**, und im Selbsttest schreibt niemand einen Kopf |
| **Der Rahmen der Buchquelle** | Von 166 Blöcken gehörten 22 dem Vorspann und 38 der Lizenz am Ende, zusammen **ein gutes Drittel**. Ein Modell hätte die Nutzungsbedingung mit derselben Ernsthaftigkeit gelernt wie den Roman |
| **Stilles Überschreiben** | `verwandlung.epub` und `verwandlung.html` schrieben beide `verwandlung.md`; die zweite überschrieb die erste **ohne ein Wort** |
| **Die Teilung war zu grob** | Das Buch hat drei Kapitel. Nach Abschnitten geteilt ist die kleinste Einheit ein Drittel: bestellt 15 % Haltemenge, geliefert **32 %**, nämlich Kapitel I ganz |
| **Tabellen als Überschriften** | Im PDF wurden „)V (1)", „EN-DE EN-FR EN-DE EN-FR" und eine Ergebniszeile zu Überschriften. **Eine falsche Überschrift zerlegt den Text an der falschen Stelle** |

**Behoben und nachgemessen:**

- Der Vorspann wird abgetrennt **und als Herkunft benutzt**: Die
  Herkunftsdatei nennt jetzt `verwandlung.epub`, also das Buch, statt
  der Zwischendatei.
- Der Rahmen wird an seinen Marken geschnitten, **beide oder keine**: Ein
  halb erkannter Rahmen nimmt entweder den Anfang des Buches mit oder
  sein Ende. Der Kopf der Ausgabe nennt, wie viel wegfiel.
- Bei belegtem Namen kommt das Format dazu (`verwandlung-html.md`), mit
  einer Meldung.
- Die Teilungseinheit ist ein **zusammenhängender Block** von höchstens
  20 Zeilen innerhalb eines Abschnitts: 32 % sind damit 20 % geworden,
  und der Bericht nennt seither Soll **und** Ist.
- Drei Wachen gegen Tabellenzeilen (Ziffernanteil, doppelte Wörter,
  Wortzahl): **55 erkannte Überschriften wurden 32**, und der Wortbestand
  blieb bei 6 064 statt 6 087. **Die Wachen fressen keine Prosa.**

**Der Stand danach**, jeweils mit dem Wortschatz des 0,6B gezählt:

| Buch | Abschnitte | Lernzeilen | Haltezeilen | Median |
|---|---|---|---|---|
| Die Verwandlung (EPUB) | 5 | 124 | 31 (20 %) | 227 Token |
| Attention Is All You Need (PDF) | 32 | 45 | 10 (18 %) | 186 Token |

⚑ **Und die Entdopplung ist am Ernstfall belegt.** Alle drei Bücher in
**einem** Ordner, also dasselbe Buch zweimal (EPUB und HTML): Der Korpus
meldet **107 wörtliche Wiederholungen** und behält 166 Lernzeilen. Wer
aus Versehen beide Formate umwandelt, bekommt das Buch trotzdem nur
einmal.

⚠️ **Was weiterhin gilt:** PDF braucht `pdftotext` oder `pypdf`. Geprüft
ist es mit `pypdf` in einer Wegwerfumgebung; in der Projektumgebung ist
bewusst nichts dazugekommen.

⚠️ **Und was das Werkzeug nicht entscheidet:** Ob ein Literaturverzeichnis
oder ein Anhang ins Training gehört, weiß nur der, der den Korpus baut.
Die Werkzeuge schneiden den **Rahmen der Quelle**, nicht den Inhalt des
Buches.

---

## `buch_zu_md.py`: Format weg, Gliederung bleibt

EPUB, HTML und Text gehen **ohne fremde Kiste** (Zip und der
HTML-Auswerter der Standardbibliothek reichen), PDF braucht `pdftotext`
oder `pypdf`. ⛔️ **Fehlt beides, gibt es eine Absage und keine leere
Datei**: Eine stille Fehlausgabe fiele erst auf, wenn ein Modell auf
nichts trainiert wurde.

⚑ **Die Lesereihenfolge kommt aus dem OPF und nicht aus dem Archiv.**
Ein Zip hat keine Reihenfolge, ein Buch schon; wer die Einträge nimmt,
wie sie liegen, bekommt Kapitel 10 vor Kapitel 2.

⚠️ **Ein Auszug ist kein Buch.** Was herauskommt, trägt Kopfzeilen,
Trennstriche und Bildunterschriften; das Aufräumen macht die nächste
Stufe, und sie zählt mit, wie viel sie wegwerfen musste.

## `buchkorpus.py`: Lern- und Haltezeilen

Eine Zeile je Beispiel, genau die Form, die `trainingsguete` liest. Vier
Eigenschaften, jede mit ihrem Grund:

⛔️ **Geteilt wird nach Abschnitten, nicht nach Zeilen.** Ein Buch
wiederholt sich (Zusammenfassungen, Definitionen, Beispiele in zwei
Formulierungen); wer zeilenweise teilt, hat dieselbe Aussage auf beiden
Seiten, und die Haltemessung misst das Gedächtnis statt der
Verallgemeinerung. **Danach wird nachgeprüft**, mit einer **strengeren**
Schwelle als beim Entdoppeln: Mit derselben konnte die Prüfung gar
nichts finden, denn das Entdoppeln läuft vorher über den ganzen Korpus.
**Eine Gegenprobe, die nicht beißen kann, ist keine.**

⚑ **Seitenköpfe und Doppelungen fallen vor dem Zusammenlegen weg.** Die
erste Fassung filterte die fertigen Zeilen, und bis dahin waren fünf
„Seite 17" längst zu einer Zeile Prosa verschmolzen: **Aus zwei Fehlern
war ein unauffälliger geworden.**

⚑ **Zusammengelegt wird nur innerhalb eines Abschnitts**, und getrennt
nur an Satzenden. Ein Beispiel über einen Themenwechsel hinweg gibt es
im Buch nicht.

⚑ **Die Tokenzahl nennt ihre Quelle.** Mit `--tokenizer <artefakt>` zählt
der Wortschatz des Modells, sonst wird geschätzt, und der Bericht sagt,
welches von beidem galt. ⚠️ Der Unterschied ist nicht klein: an zwei
Dateien dieses Repositoriums 244 Zeilen geschätzt gegen 341 gezählt.

## `md_zu_mappe.py`: die Wissensmappe

Eingangsseite, Kapiteldateien, Stichwortverzeichnis. ⚑ **Drei Vorgaben
kommen aus der Messung vom 2026-09-17**, bei der nachgesehen wurde, wie
ein Modell eine Einzelheit wiederfindet, die nicht mehr im Kontext
steht:

1. **Die Eingangsseite hat eine Obergrenze.** Ein Verzeichnis, das
   mitwächst, frisst genau den Kontext, den es sparen soll.
2. **Gesucht wird nach einem Wort, nicht nach einer Gliederung**, daher
   das Stichwortverzeichnis.
3. **Jeder Eintrag trägt einen Satz und nicht nur einen Ort**, sonst wird
   der Zeigefinger für die Auskunft gehalten.

⚑ **Wohin die Mappe gehört:** in einen Skill-Ordner des Clients, also
`<einhängung>/.AGENT/skills/` für ein Projekt oder
`<konfiguration>/skills/` für alles. `myl skills` nennt beide Orte.

## Selbsttests

Jedes Werkzeug hat `--selbsttest`: ein erfundenes Buch mit bekannten
Eigenschaften, gegen das die Zusagen geprüft werden. ⚑ **Jede Zusage ist
gegengeprobt**, also einmal absichtlich gebrochen worden, um zu sehen,
dass die Prüfung anschlägt.

```bash
python3 TRAINING/korpus/buch_zu_md.py --selbsttest
python3 TRAINING/korpus/buchkorpus.py --selbsttest
python3 TRAINING/korpus/md_zu_mappe.py --selbsttest
```

⚠️ **Was herauskommt, wird nicht versioniert.** `datasets/` schließt sich
aus; das gilt für erzeugte Daten und erst recht für Buchtext.
