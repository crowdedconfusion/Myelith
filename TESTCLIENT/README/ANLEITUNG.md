# Anleitung: Tests mit mehreren Beteiligten und heterogener Hardware

**Version:** 2.6.0 · **Datum:** 2026-08-21

Diese Anleitung hat zwei Hälften:

- **Teil A, für Teilnehmer.** Du stellst einen Rechner zur Verfügung und
  lässt einen Test laufen. Du musst vom Projekt nichts wissen und nichts
  programmieren können. Teil A ist so geschrieben, dass er ohne
  Vorkenntnisse funktioniert.
- **Teil B, für Koordinatoren.** Du legst fest, was gemessen wird,
  sammelst die Ergebnisse ein und fällst das Urteil. Das macht eine
  Person, und diese Person sollte Teil B ganz lesen.

---

# Teil A: für Teilnehmer

## A1. Worum es geht, in einfachen Worten

Ein Sprachmodell rechnet mit Zahlen. Üblicherweise mit **Kommazahlen**,
und dabei gilt eine unangenehme Eigenheit: Rechnet man dieselbe Aufgabe
in anderer Reihenfolge, kommt ein leicht anderes Ergebnis heraus. Nicht
falsch, nur anders in den hintersten Stellen. Zwei ehrliche Rechner
liefern deshalb normalerweise **verschiedene** Zahlen.

Für ein Netzwerk, in dem Fremde füreinander rechnen, ist das fatal: Man
kann nicht unterscheiden, ob jemand betrogen hat oder ob er nur eine
andere Grafikkarte besitzt.

Myelith rechnet deshalb **nur mit ganzen Zahlen**. Bei ganzen Zahlen ist
die Reihenfolge egal: `(3+5)+7` und `3+(5+7)` sind beide 15, auf jeder
Maschine. Damit wird Gleichheit prüfbar. Wer abweicht, hat wirklich
etwas anderes gerechnet.

**Dieser Testclient prüft, ob das in der Praxis hält.** Er lässt dasselbe
Modell dieselben Fragen beantworten und bildet aus den Antworten eine
Prüfsumme. Läuft der Test auf einem Mac mit ARM-Chip und auf einem
Windows-PC mit Intel-Chip und kommt **dieselbe** Prüfsumme heraus, ist
der Nachweis erbracht.

Dein Beitrag ist genau das: **eine Maschine, die anders ist als die
anderen.**

## A2. Was du brauchst

| | |
|---|---|
| **Immer** | Das Repository auf der Platte und [Rust](https://rustup.rs). Der Starter sagt dir beim ersten Aufruf, was fehlt. |
| **Nur für die Modellläufe** | Python mit PyTorch, siehe [A7](#a7-python-für-die-modellläufe-einrichten). Rund 2 GB. |
| **Plattenplatz** | 0,5B-Modell: rund 1,7 GB. 7B-Modell: rund 23 GB. Beides lässt sich hinterher wieder freigeben, siehe [A8](#a8-platz-wieder-freigeben). |

**Ohne Python bist du trotzdem nützlich.** Der Testlauf erhebt in jedem
Fall die Hardware und fährt den Protokoll-Durchlauf; nur die beiden
Modellstufen fallen aus. Das ist kein Makel, sondern ein gültiges
Teilergebnis.

Ein Konto bei Hugging Face brauchst du **nicht**.

## A3. Starten

Im Ordner `TESTCLIENT` liegt für jedes System ein Starter:

| Dein System | Was du tust |
|---|---|
| **macOS** | Doppelklick auf `Myelith Testclient - macOS.app` |
| **Windows** | Doppelklick auf `Myelith Testclient - Windows (Batch).cmd` |
| **Linux** | Im Terminal: `./"Myelith Testclient - Linux, macOS (Shell).sh"` |

Beim ersten Doppelklick auf das macOS-Bündel fragt das System einmal, ob
„sh" das Programm „Terminal" steuern darf. Erlaube das: Der Starter öffnet
damit das Terminalfenster, in dem der Client läuft. Andere Anwendungen
werden nicht angesprochen. Wurde die Frage versehentlich abgelehnt, steht
sie in Systemeinstellungen, Datenschutz & Sicherheit, Automatisierung und
lässt sich dort wieder erlauben.

Das Fenster öffnet sich mittig auf dem Bildschirm, 120 Spalten breit und
44 Zeilen hoch. Wer den Client in einem bereits offenen Terminal startet,
behält dessen Fenster unverändert; das Banner passt sich dann der
vorhandenen Breite an.

Beim **ersten** Start baut sich der Client selbst. Das dauert einige
Minuten; danach sind es Sekunden. Du darfst die Starter verschieben,
kopieren oder auf den Schreibtisch legen: sie finden das Repository von
selbst.

Zuerst läuft ein kurzes Startbild: Zeichen fallen, eine Spirale wächst
darin auf, bunte Artefakte strömen auf ihren Armen nach innen und setzen
in der Mitte das Logo zusammen, während der Regen im Hintergrund
weiterläuft; danach gleitet das Logo an seinen Platz. Ein Tastendruck überspringt es;
`MYL_NO_ANIMATION=1` schaltet es dauerhaft ab.

## A4. Bedienung: Pfeiltasten und Enter

Überall, wo der Client etwas zur Auswahl stellt, gilt dasselbe:

```
  ── Was möchtest du tun? ──
  ❯ 1  Mit dem Modell sprechen
        Freie Eingabe, das Artefakt antwortet. Zum Ansehen, nicht zum
        Messen: kein Protokoll, kein Vergleichswert.
    2  Testlauf starten
    3  Testdatei wählen
    4  Artefakt wählen
    5  Anleitung lesen
    9  Entwickler-Menü
    0  Beenden

  ↑ ↓ bewegen · Enter wählen · Ziffer direkt · Esc zurück
```

| Taste | Wirkung |
|---|---|
| **↑ ↓** | Auswahl bewegen |
| **Enter** | Ausgewähltes ausführen |
| **Ziffer** | Direkt zu diesem Punkt springen und ihn ausführen |
| **Esc** | Eine Ebene zurück |

Unter der Auswahl stehen die **aktuellen Einstellungen**: erst die Frage,
was du tun willst, dann der Zustand, unter dem es geschieht. Alles steht
mittig unter dem Logo; die Zeilen, auf die du drückst oder in die du
tippst, sitzen am linken Rand.

Läuft der Client in einer Umgebung ohne Tastatursteuerung: in einer
Pipe, in einem Skript, in einer schlichten seriellen Konsole , zeigt er
dieselbe Liste und wartet auf eine getippte Ziffer mit Enter. Beide Wege
führen zum selben Ergebnis.

Die Farben würfelt der Client bei jedem Start neu und behält sie dann für
die ganze Sitzung: eine Farbe für das Logo und zwei dazu passende für die
Menütitel. **Das ist Schmuck und sonst
nichts:** Kein Urteil und kein Ergebnis hängt an einer Farbe, dafür
stehen überall Wörter. Wer schwarzweiß liest, verliert nichts.

**Der Bildschirm wird vor jeder Auswahl aufgeräumt.** Oben steht das
Logo, darunter genau das, was ansteht: nichts sonst. Nach einer Aktion
bleibt ihre Ausgabe stehen, bis du eine Taste drückst:

```
  ── Weiter mit einer beliebigen Taste ──
```

Du bestimmst also, wie lange du das Ergebnis ansiehst. Verloren geht
dabei nichts: Alles, was zählt, steht im Protokoll.

## A5. Der Ablauf, Schritt für Schritt

### Schritt 1: Dein Nutzername

```
  Unter welchem Nutzernamen sollen die Protokolle dieser Sitzung laufen?
  Er steht im Dateinamen und im Protokoll, damit der Koordinator sie
  ohne Rückfrage zuordnen kann. Leer lassen ist erlaubt.

  Nutzername:
```

Danach begrüßt dich der Client mit deinem Namen, dann kommt das Menü.

Ein Vorname, ein Spitzname oder eine Bezeichnung der Maschine: was dem
Koordinator hilft, dein Protokoll wiederzuerkennen. Lässt du das Feld
leer, heißen deine Dateien `ohne-name`; das funktioniert, macht dem
Koordinator aber Arbeit.

### Schritt 2: Testdatei wählen

Nach dem Nutzernamen stehst du **direkt im Menü**. Die Testdatei fragt
der Client dann ab, wenn du [3] Testlauf starten wählst; über [2] kannst
du sie auch vorher festlegen.

Hat dir der Koordinator eine Datei mit der Endung `.plan` geschickt,
lege sie vorher in den Ordner `TESTCLIENT/Testpläne/`. Der Client listet
auf, was er dort findet:

```
  ── Testpläne in TESTCLIENT/Testpläne ──
  ❯ 1  standard · 6 Prompts, 32 Token, 4 Shards
        Prompt: " The 2010 Haitian earthquake was a catast…" (+5 weitere)
    2  standard-kurz · 4 Prompts, 16 Token, 4 Shards
        Prompt: "The capital of France is" (+3 weitere)
    0  keiner. Einstellungen von Hand wählen
```

**Die Testdatei ist der Kern des Verfahrens.** Sie legt fest, welche
Fragen gestellt werden, wie viele Wörter geantwortet wird und welches
Modell rechnet. Alle Beteiligten müssen dieselbe Datei verwenden, sonst
sind die Ergebnisse nicht vergleichbar. Damit das keine Bitte bleibt,
trägt die Datei eine Prüfsumme: **Wird sie verändert, verweigert der
Client den Lauf.** Ändere sie also nicht: auch kein Leerzeichen.

Wählst du einen Plan, macht der Client den Rest allein: Modell prüfen,
bei Bedarf beschaffen, messen.

Liegt keine Datei bereit, wähle `0`. Dann misst der Client mit
Standardwerten. Das ist für einen ersten Versuch in Ordnung, für einen
gemeinsamen Test nicht.

### Schritt 3: Modell

Punkt **[1] Artefakt wählen** führt **alle** Modelle auf, die der Client
kennt, und schreibt daneben, ob sie schon hier liegen:

```
  ❯ 1  qwen2.5-0.5b, liegt bereit
          Digest wird nach der Wahl geprüft.
    2  qwen2.5-7b, nicht vorhanden
          Download rund 15 GB von Hugging Face, Bau danach in Sekunden.
```

Du kannst also jederzeit ein weiteres Modell holen, auch wenn schon eines
da ist, und ein freigegebenes zurückholen. Liegt das Modell schon auf
deiner Maschine, übernimmt der Client es beim Start von selbst, und du
kannst diesen Schritt überspringen. Fehlt es, bietet [4] an, die Gewichte zu
holen und die Artefakte daraus zu bauen. **Es passiert nichts ohne
Rückfrage**, und die Größe des Downloads steht dabei. Während es läuft,
siehst du, wie lange es schon dauert.

Liegen mehrere Modelle bereit, fragt er, welches.

Der Punkt ist bewusst eigenständig: Bis v0.6.0 löste die Testdatei das
Modell gleich mit auf, und aus einer Menüwahl wurden ungefragt bis zu
15 GB Download. Jetzt entscheidest du, wann das geschieht.

### Zwischendurch: mit dem Modell sprechen

Punkt **[1]** ist der einzige, der nichts misst. Du tippst etwas, das
Modell antwortet, höchstens 64 Token lang; eine leere Eingabe beendet
das Gespräch. Es beantwortet die Frage, die sich jeder stellt, der seine
Maschine hergibt: Was rechnet das Ding da eigentlich?

Die Antwort erscheint **Wort für Wort**, während gerechnet wird. Bei 7B
dauert eine Antwort über eine halbe Minute; so siehst du die ganze Zeit,
dass es vorangeht.

**Zurück ins Menü** kommst du auf drei Wegen, und der Hinweis steht in
jeder Eingabezeile:

```
  Prompt [Esc = Menü]:
```

| Weg | |
|---|---|
| **Escape** | Die Taste, mit der man ein Menü verlässt |
| **Strg-D** | Die Kombination, die in jeder Kommandozeile „fertig" heißt |
| **`menu` tippen** | Auch `exit`, `q`, `zurück` und `/menu` |

**Enter allein tut nichts.** Du kannst also jederzeit Enter drücken, um
zu sehen, ob sich der Client noch meldet, ohne das Gespräch zu verlieren.

**Nicht Strg-C.** Das beendet den ganzen Client, nicht das Gespräch, und
dein Sitzungsname wäre weg.

Die Auswahl ist **gierig**, ohne Sampling und ohne Zufall. Dieselbe
Frage liefert auf demselben Modellstand dieselbe Antwort, hier wie im
Testlauf. Genau deshalb gibt es keine Temperatur einzustellen. Ein
Protokoll schreibt dieser Punkt nicht: Prompt und Länge bestimmst du
frei, das wäre kein Messwert.

### Schritt 4: Der Testlauf

Punkt **[3] Testlauf starten**. Das ist alles.

Der Lauf hat vier Stufen und schreibt **ein einziges Protokoll** über
alle vier:

| Stufe | Was sie tut |
|---|---|
| 1 Hardware | Erhebt, was für eine Maschine das ist (Architektur, System, Rechenkerne) |
| 2 Determinismus | Rechnet jede Frage **zweimal** und prüft, ob dasselbe herauskommt |
| 3 Geshardete Inferenz | Verteilt das Modell auf vier Teile und prüft gegen das ungeteilte |
| 4 Protokoll-Durchlauf | Prüft die Protokollschicht, ohne Modell: Kryptografie, Konsens, Ledger |

Alle vier laufen auch dann durch, wenn eine fehlschlägt: Eine
fehlgeschlagene Modellstufe macht die Hardware-Erhebung nicht wertlos,
sondern erst recht wichtig.

### Schritt 5: Ergebnis lesen

Zu jeder Frage zeigt der Client die erzeugte Antwort im Klartext:

```
  Prompt 2:  The capital of France is
  Antwort:   Paris. It is the largest city in France and one of the most
```

**Das ist zum Zuschauen.** Bewertet wird der Text nicht. Maßgeblich sind
die Zeilen mit dem Wort `Ergebnis`, besonders diese beiden:

```
  Ergebnis  determinismus          6 Prompts, je zwei Läufe  [fd64588fd46a7af8]
  Ergebnis  shard_vs_einzelknoten  6 Prompts bitgleich       [3a1c…]
```

Der Wert in eckigen Klammern ist der **Vergleichswert**. Er muss auf
jeder Maschine derselbe sein. Genau das ist der ganze Test.

### Schritt 6: Protokoll zurückschicken

Am Ende nennt der Client den Pfad. Die Protokolle liegen in
**`TESTCLIENT/logs/`**: auf derselben Ebene wie `Testpläne/` und
`Vergleiche/`, also dort, wo du ohnehin schon warst:

```
Protokoll: …/TESTCLIENT/logs/anna_12a1e91e_2026-08-21_143022.jsonl
           und …_143022.log. Lauf 2026-08-21-143022-aarch64-macos-…
```

Schicke die **`.jsonl`** an den Koordinator. Die `.log` daneben enthält
dasselbe als Fließtext und ist für dich.

Der Dateiname sagt schon alles: dein Name, die Kennung der Einstellungen,
Datum und Uhrzeit.

**Was im Protokoll steht:** Architektur, Betriebssystem, Backend, welches
Modell, Zeiten, Vergleichswerte, die erzeugten Zahlen (Token), der
**Hash** deiner Fragen.
**Was nicht darin steht:** der Klartext der Fragen und Antworten, dein
Benutzername, dein Rechnername, Seriennummern, MAC-Adressen.

Die Datei ist reiner Text und darf unverändert weitergegeben werden.

## A6. Selbst nachsehen: Protokolle vergleichen

Punkt **[9] Entwickler-Menü**, dort **Protokolle vergleichen**, fragt
zuerst, *welche*:

| Auswahl | Was verglichen wird |
|---|---|
| **Zugesandte Protokolle** | Was in `TESTCLIENT/Vergleiche/` liegt, der Weg des Koordinators |
| **Eigene Läufe** | Die Protokolle dieser Maschine aus `TESTCLIENT/logs/` |

Für dich als Teilnehmer ist meist die zweite Auswahl richtig. Sie ergibt
für sich **keinen** Nachweis: dazu fehlt eine zweite Maschine , zeigt
aber, ob wiederholte Läufe übereinstimmen. Hast du die Dateien der
anderen bekommen, leg sie in `TESTCLIENT/Vergleiche/` und nimm die erste
Auswahl:

```
  ── testlauf · Einstellungen 12a1e91e · 2 Protokolle ──
     anna             aarch64-macos-reference    θ_v 0.17.0   35613afa…
     björn            x86-64-linux-avx2          θ_v 0.17.0   9c02fe11…

     = determinismus            fd64588fd46a7af8
     = shard_vs_einzelknoten    3a1c88b0e4d2f760

     Urteil: NACHWEIS
```

Die Urteile stehen in [B5](#b5-die-urteile-und-was-sie-bedeuten).

## A7. Python für die Modellläufe einrichten

Nur nötig, wenn du die Stufen 2 und 3 fahren willst.

```bash
cd INTEGER_LLM/calibrate
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
```

Unter Windows heißt die letzte Zeile `.venv\Scripts\pip install -r requirements.txt`.

Das sind rund 2 GB und dauert einige Minuten. Danach findet der Client
die Umgebung von selbst. Fehlt sie, sagt er genau diese Zeilen an; er
sucht auch ein System-Python, falls die Pakete dort schon liegen.

## A8. Platz wieder freigeben

Im Entwickler-Menü (Punkt 9), dort Punkt „Artefakte und Gewichte
löschen". Du kannst
einzelne Einträge löschen oder **alles auf einmal**. Beim Alles-Löschen
fragt der Client **zweimal** nach und listet dazwischen jeden betroffenen
Pfad auf: Artefakte sind aus dem Skalenpaket in Sekunden wieder da,
Gewichte kosten einen erneuten Download.

Entwickler-Menü **[9]**, dann **[6] Artefakte und Gewichte löschen**.
Der Client zeigt, was belegt ist:

```
  Belegt auf dieser Maschine: 24,9 GB

  ❯ 1  qwen2.5-7b · Artefakte · 8,1 GB
        Aus dem Skalenpaket in Sekunden wiederherstellbar.
    2  qwen2.5-7b · Gewichte · 15,2 GB
        Erneut zu holen kostet einen Download über Hugging Face.
```

**Artefakte und Gewichte sind verschieden teuer wiederzubeschaffen.**
Artefakte entstehen in Sekunden neu, die Gewichte kosten einen Download
über Gigabyte. Wer den Test später wiederholen will, gibt deshalb die
Artefakte frei und behält die Gewichte.

Vor dem Löschen musst du „ja" tippen. Enter allein genügt hier
absichtlich nicht: Löschen ist der einzige Vorgang in diesem Client, der
etwas zerstört.

## A9. Wenn etwas nicht klappt

**„Artefaktverzeichnis fehlt"**
Erwartet wird `INTEGER_LLM/artifacts/qwen2.5-0.5b/`. Liegen die Artefakte
woanders, im Entwickler-Menü unter [7] den Pfad setzen oder beim Aufruf
`--artifacts <PFAD>` angeben.

**„Der Testplan wurde verändert"**
Die Datei wurde nach dem Erzeugen bearbeitet: auch ein zusätzliches
Leerzeichen zählt. Fordere die Originaldatei neu an. Kommentarzeilen mit
`#` darfst du dagegen frei ergänzen, die gehen nicht in die Prüfsumme
ein.

**Der Lauf dauert sehr lange**
Beim 7B-Modell ist das normal, rechne mit rund fünf Minuten. Läuft es
auch beim 0,5B-Modell zäh, wurde vermutlich ein Debug-Build gestartet;
die Starter bauen mit `--release`.

**Umlaute erscheinen als Kauderwelsch (Windows)**
Der mitgelieferte Starter stellt die Konsole selbst auf UTF-8 um. Wer
`myl-test.exe` direkt aufruft, setzt vorher `chcp 65001`.

**Das Menü sieht zerrissen aus**
Das Fenster ist zu klein. Der Client erkennt das eigentlich selbst und
schaltet auf die einfache Liste um; hilf notfalls mit einem größeren
Fenster nach.

**Zwei Läufe auf derselben Maschine ergeben verschiedene Werte**
Das wäre schwerwiegend und kein Bedienfehler. Protokoll sichern und
melden.

---

# Teil B: für Koordinatoren

## B1. Der Kern in drei Sätzen

Der Cross-Hardware-Nachweis braucht **zwei** Aussagen, nicht eine:

1. **Die Maschinen sind verschieden.** (Hardware-Fingerabdrücke ungleich)
2. **Das Ergebnis ist trotzdem gleich.** (Vergleichswerte gleich)

Fehlt (1), beweist (2) nichts: zwei gleiche Ergebnisse von derselben
Maschine sind trivial. Fehlt (2) bei erfülltem (1), ist die Kernthese des
Projekts widerlegt, und das wäre der wichtigste Befund seit Bestehen des
Repositoriums.

Der `vergleich`-Befehl setzt das durch und **verweigert** ein positives
Urteil, wenn (1) fehlt.

## B2. Einen Testplan erstellen

### Der schnelle Weg: im Menü

Entwickler-Menü **[9]**, dann **[2] Testplan erzeugen und speichern**.
Der Client fragt jeden Wert einzeln ab, in dieser Reihenfolge:

1. **Token je Prompt.** Entertaste übernimmt die Vorgabe.
2. **Shards** für den Shard-Lauf. Ebenso.
3. **Prompt 1.** Danach die Frage, ob noch einer folgen soll; wer „ja"
   antwortet, bekommt die nächste Zeile, und das so oft er mag.
4. **Der Dateiname**, ganz zum Schluss. Er steht am Ende, weil man einen
   Plan erst sinnvoll benennen kann, wenn man weiß, was darin steht.

Die Datei landet im Planordner. Gibt es sie schon, wird gefragt.

Ein Abbruch (Strg-D) schreibt **nichts**: Eine halb erhobene Datei, die
an alle Teilnehmer geht, wäre schlimmer als keine.

### Der genaue Weg: auf der Befehlszeile

```bash
myl-test plan \
  --plan-id 2026-08-21-cross-arch-01 \
  --prompt "Die Hauptstadt von Frankreich ist" \
  --prompt "The capital of France is" \
  --prompt "In quantum mechanics, the wave function describes" \
  --prompt "The result of 17 times 23 is" \
  --steps 32 --shards 4 \
  --out "TESTCLIENT/Testpläne/2026-08-21-cross-arch-01.plan"
```

**`--prompt` ist mehrfach angebbar.** Jede Angabe hängt eine Frage an die
Reihe an, die der Lauf nacheinander abarbeitet.

### Was der Plan festlegt

| Parameter | Warum es exakt gleich sein muss |
|---|---|
| **Prompts** (Reihenfolge zählt) | Ein anderes Zeichen → anderer Vergleichswert |
| **`--steps`** | Bestimmt, wie viele Schritte in den Wert eingehen |
| **`--shards`** | Bestimmt die Aufteilung in Stufe 3 |

Nicht abgesichert ist `plan_id`. Zwei Koordinatoren, die denselben Test
unter verschiedenen Namen fahren, sollen vergleichbare Ergebnisse
bekommen.

**Das Modell steht seit dem 2026-08-22 nicht mehr im Plan.** Es war eine
Fessel ohne Nutzen: Ein Plan, der nur mit 0,5B geht, muss für 7B neu
geschrieben werden, und dann tragen zwei Dateien dieselben Prompts unter
verschiedenen Prüfsummen. Der Plan legt jetzt fest, *was* gemessen wird;
*woran*, entscheidet sich vor dem Lauf, entweder über **[1] Artefakt
wählen** oder ungefragt, wenn genau eines vorliegt.

Abgesichert bleibt es trotzdem, nur an der richtigen Stelle: Der
Modellstand (θ_v und Artefakt-Digest) steht in **jedem Protokoll**, und
`vergleich` verweigert das Urteil, wenn zwei Läufe gegen verschiedene
Modelle gerechnet haben. Eine Datei kann man ignorieren, diese Prüfung
nicht.

Eine alte Datei mit `model`-Zeile bleibt lesbar; ihre Prüfsumme stimmt
allerdings nicht mehr, weil das Feld aus der Rechnung entfallen ist.
Solche Pläne sind neu zu erzeugen.

### Wie viele Prompts, wie viele Token?

**Mehrere Prompts sind kein Luxus.** Ein einzelner Prompt übt einen
einzigen Pfad durch das Modell aus. Ein Rundungsfehler, der nur bei
langen Sequenzen, nur bei bestimmten Zeichen oder nur in einem selten
getroffenen Tabellenbereich auftritt, bliebe unentdeckt, und der
Vergleichswert sähe trotzdem beruhigend aus. Vier bis acht Prompts, die
sich in Sprache, Länge und Art unterscheiden, sind ein guter Schnitt.

**Die Laufzeit rechnest du vorher aus**, statt sie zu raten:

```
Token je Durchgang   = Prompts × steps
Determinismus        = 2 Durchgänge
Shard-Lauf           = 2 Durchgänge (Pod und Einzelknoten)
```

Mit den gemessenen Raten aus `INTEGER_LLM/bench/README.md`: **0,5B rund
24 Token/s, 7B rund 2 Token/s**: plus Modellladen (0,5B ein paar
Sekunden, 7B rund eine Minute je Lauf).

| Beispiel | Rechnung | Dauer |
|---|---|---|
| 0,5B, 6 Prompts, 32 Token | 6·32·4 = 768 Token bei 24/s | rund 40 s |
| 7B, 4 Prompts, 16 Token | 4·16·4 = 256 Token bei 2/s | rund 5 min |

Sage die Zahl den Teilnehmern an. Ohne sie ist für sie nicht
entscheidbar, ob sich Warten lohnt oder ob etwas hängt.

### Die mitgelieferten Pläne

**Keiner davon ist an ein Modell gebunden** (seit 2026-08-22). Der Plan
legt fest, *was* gemessen wird; *woran*, entscheidet sich vor dem Lauf.
Derselbe Plan gilt für 0,5B und für 7B, und niemand muss zwei Dateien
pflegen, die dieselben Prompts tragen.

| Datei | Umfang | Wofür |
|---|---|---|
| `standard.plan` | 6 Prompts, 32 Token, 4 Shards | Der Regelfall |
| `standard-kurz.plan` | 4 Prompts, 16 Token, 4 Shards | Für langsame Modelle, etwa 7B |
| `benchmark-1-zahlen.plan` | 7 Prompts, 24 Token, 4 Shards | Ziffern, Überträge, Einheiten |
| `benchmark-2-sprachen.plan` | 8 Prompts, 24 Token, 4 Shards | Sieben Sprachen, drei Schriften |
| `benchmark-3-code-kontext.plan` | 6 Prompts, 32 Token, 4 Shards | Quelltext und lange Prompts |

Die beiden `standard`-Pläne nehmen Prompts aus dem Evidenz-Paket des
Projekts (Deutsch und Englisch, Fachsprache, Arithmetik); `standard.plan`
beginnt zusätzlich mit einem Satz aus WikiText-2, dem Korpus, gegen den
auch die Perplexität gemessen wird.

Die drei `benchmark`-Pläne führen das Modell absichtlich an
ungewöhnliche Stellen: Ziffernfolgen werden anders tokenisiert als
Wörter, fremde Schriften liegen weit auseinander in der
Einbettungstabelle, und lange Prompts schieben die Generierung auf hohe
Positionen, wo RoPE-Winkel groß werden. Fund 15 (RoPE falsch) und
Fund 16 (Attention nur auf den ersten Key) waren beide Fehler, die bei
kurzen Prompts kaum auffielen.

**Wichtig zur Einordnung:** Der Client misst **Bitgleichheit, nicht
Genauigkeit**. Ob eine Antwort inhaltlich stimmt, beantwortet dieser
Lauf nicht und soll er nicht; dafür gibt es die Perplexitätsmessung in
`INTEGER_LLM/eval` gegen die Gleitkomma-Referenz. Ein „Benchmark" heißt
hier: ein Prompt, der schwer zu rechnen ist, nicht einer, der bewertet
wird.

Jeder Plan trägt im Kopf, was er ausübt und wie lange er läuft.

### Aufbau der Datei

```text
# Kommentare gehen NICHT in die Prüfsumme ein.

plan_id     = 2026-08-21-cross-arch-01
prompt      = "Die Hauptstadt von Frankreich ist"
prompt      = "The capital of France is"
steps       = 32
shards      = 4

spec_sha256 = 12a1e91e4fa75f6e…
```

Reiner Text, von Hand lesbar und von Hand schreibbar. Die Prompts stehen
in Anführungszeichen, damit ein Randleerzeichen erhalten bleibt: es ist
Teil des Prompts und verändert das Ergebnis.

**Ändert jemand eine Zeile, verweigert der Client den Lauf:**

```
myl-test: Der Testplan wurde verändert.
     Prüfsumme in der Datei: 12a1e91e…
     tatsächlicher Inhalt:   5b6bde79…
     Verwende die Originaldatei des Koordinators …
```

Exit-Code 3. Damit ist der häufigste Fehlalarm technisch ausgeschlossen:
Ein vertippter Prompt liefert keinen abweichenden Vergleichswert mehr,
sondern einen Fehler.

## B3. Verteilen

Die `.plan`-Datei unverändert an alle Teilnehmer schicken. Chat, Mail,
Repository, egal. Sie ist reiner Text und enthält keine
personenbezogenen Daten.

Dazu diese vier Sätze:

> Leg die Datei in `TESTCLIENT/Testpläne/`.
> Starte den Client, gib deinen Namen ein, wähle den Plan, drücke [1].
> Rechne mit **&lt;Dauer&gt;**.
> Schick mir die `.jsonl` aus `TESTCLIENT/logs/`.

## B4. Einsammeln und auswerten

Für die eingehenden Dateien gibt es einen eigenen Ordner:

```text
TESTCLIENT/Vergleiche/            ← hier die zugesandten .jsonl ablegen
TESTCLIENT/Vergleiche/Berichte/   ← hierhin schreibt der Vergleich seinen Bericht
```

Alle eingegangenen `.jsonl` dort hineinlegen: **nicht umbenennen**, der
Dateiname trägt bereits Teilnehmer, Einstellungs-Kennung, Datum und
Uhrzeit. Dann:

```bash
myl-test vergleich
```

Ohne weitere Angabe liest der Befehl genau diesen Ordner. Im Menü:
Punkt [3], dort „Zugesandte Protokolle".

**Warum ein eigener Ordner und nicht der Protokollordner des Clients:**
Der Vergleich liest *alles*, was er an `.jsonl` findet. Lägen die
zugesandten Läufe zwischen den eigenen, enthielte die Gruppe die eigene
Maschine mehrfach, und ein Urteil darüber sagt etwas anderes aus, als es
zu sagen scheint.

**Warum der Bericht in einem Unterordner landet:** Läge er neben seiner
Eingabe, würde ihn der nächste Aufruf mitlesen.

Der Befehl gruppiert nach Prüflauf und Einstellungs-Kennung, stellt jeden
Vergleichswert gegenüber und fällt je Gruppe ein Urteil:

```
  ── testlauf · Einstellungen 12a1e91e · 3 Protokolle ──
     anna             aarch64-macos-reference    θ_v 0.17.0   35613afaedb6757e
     björn            x86-64-linux-avx2          θ_v 0.17.0   9c02fe1148ab3d05
     carla            x86-64-windows-avx2        θ_v 0.17.0   77b1e0c9a4128f3e

     = determinismus            fd64588fd46a7af8
     ≠ shard_vs_einzelknoten    2 verschiedene Werte:
         3a1c88b0e4d2f760  anna, björn
         81ff20c4de915a03  carla

     Urteil: ABWEICHUNG
```

Ein `=` heißt, alle Protokolle stimmen in diesem Wert überein; ein `≠`
listet auf, wer was gerechnet hat. Der Befehl endet mit Exit-Code 0 nur
dann, wenn **jede** Gruppe den Nachweis trägt: er taugt damit für die CI.

Zusätzlich zur Bildschirmausgabe entsteht in `Vergleiche/Berichte/` ein
ausführlicher Bericht als Markdown: `vergleich_<datum>_<uhrzeit>.md`. Er
trägt, was auf dem Bildschirm keinen Platz hat: **vollständige** Digests
statt der Kurzform, die Dateinamen, den Artefakt-Digest je Teilnehmer und
den Zeitpunkt des Vergleichs. Diese Datei reichst du weiter.

Ein **Laufprotokoll** schreibt `vergleich` dagegen nicht: Es misst nichts,
es wertet aus.

Der Berichtsordner wird nicht versioniert. Was bleiben soll, gehört nach
`INTEGER_LLM/eval/results/`: siehe [B7](#b7-ergebnis-dauerhaft-festhalten).

## B5. Die Urteile und was sie bedeuten

| Urteil | Bedeutung | Was zu tun ist |
|---|---|---|
| **NACHWEIS** | Fingerabdrücke verschieden, Werte gleich, Modellstand gleich | Festhalten, siehe [B7](#b7-ergebnis-dauerhaft-festhalten) |
| **KEIN NACHWEIS (eine Maschine)** | Werte gleich, aber alle Protokolle von derselben Maschine | Es fehlt eine zweite Architektur, nicht ein weiterer Lauf |
| **UNVERGLEICHBAR (Modellstand)** | θ_v oder Artefakt-Digest weichen ab | **Kein Hardware-Befund.** Erst gleichziehen, siehe [B6](#b6-modellstand-gleichziehen) |
| **ABWEICHUNG** | Gleicher Modellstand, gleiche Eingabe, verschiedene Ergebnisse | Der eigentliche Befund, siehe [B8](#b8-bei-einer-abweichung) |
| **ZU WENIG PROTOKOLLE** | Weniger als zwei mit derselben Kennung | Weichen die Kennungen ab, liefen verschiedene Parameter |

Der wichtigste Fall in dieser Tabelle ist der zweite. Ein Werkzeug, das
zwei gleiche Werte von derselben Maschine als Nachweis ausgibt, wäre
schlimmer als gar keines, weil sein Ergebnis geglaubt wird. Deshalb ist
diese Verweigerung ein Akzeptanzkriterium des Fahrplans und keine
Höflichkeit.

## B6. Modellstand gleichziehen

Vor jedem Vergleichslauf sollte auf **jeder** Maschine laufen:

```bash
myl-test artefakte
```

Der Befehl rechnet den Digest über die Ankerkette des Artefakts und hält
ihn gegen den veröffentlichten Wert aus
`INTEGER_LLM/scale_packs/REGISTER.json`. Weicht er ab, sagt er das
ausdrücklich:

> FEHLER  Digest weicht ab … Das ist KEIN Hardware-Befund. Hier liegt ein
> anderes Modell als beim Vergleichspartner; ein Bitgleichheitstest
> darüber hätte keine Aussage.

Ohne diesen Satz sähe ein abweichendes Artefakt aus wie eine
gescheiterte Hardware-Bitgleichheit, der Client würde also das Gegenteil
dessen berichten, wofür es ihn gibt.

Seit dem Skalenpaket ist der Bau plattformübergreifend bitgleich und
dauert Sekunden. Ein abweichender Digest heißt deshalb in aller Regel:
veralteter Stand, nicht kaputte Hardware.

## B7. Ergebnis dauerhaft festhalten

Laufprotokolle sind flüchtig, `logs/` ist gitignored. Ein bestätigter
Cross-Hardware-Nachweis gehört nach `INTEGER_LLM/eval/results/`, mit
Datum, beteiligten Architekturen, Backends, θ_v-Stand und den
Vergleichswerten.

## B8. Bei einer Abweichung

In dieser Reihenfolge prüfen:

1. **Haben alle denselben Plan verwendet?** Der `vergleich`-Befehl
   gruppiert danach; verschiedene Kennungen ergeben verschiedene Gruppen
   und sind sofort sichtbar. Gegenprobe auf die Prompts selbst:
   `grep '"prompt_sha256"' *.jsonl`.
2. **Ist der Modellstand identisch?** Steht als `θ_v` und
   `artefakt_digest` in jedem Protokoll und in der Übersicht des
   Vergleichs. Bei Abweichung urteilt der Befehl ohnehin
   `UNVERGLEICHBAR`.
3. **Läuft dasselbe Backend?** `grep '"key":"backend_selected"' *.jsonl`.
   Referenz gegen `cpu-simd/neon` ist ein **gewollter** Vergleich, aber
   auch er muss bitgleich sein. Weicht er ab, ist es ein
   SIMD-Paritätsfehler und gehört in den INTEGER_LLM-Fahrplan.

   **`cpu-simd` gibt es heute nur auf aarch64** (Fund 34, 2026-08-22).
   Auf x86_64 hat `kernels/src/dot.rs` noch keine vektorisierte Fassung;
   ein Bau mit `--features cpu-simd` würde dort die Referenzkernel unter
   fremdem Namen protokollieren, und der Client verweigert ihn deshalb
   mit einem Hinweis auf `cargo build --release`. Für den
   Cross-Hardware-Nachweis ändert das nichts: Verglichen werden zwei
   Maschinen, nicht zwei Backends.

Erst wenn alle drei übereinstimmen und die Werte trotzdem abweichen, ist
es ein Befund an der Kernthese aus Whitepaper Kap. 6.2. Dann:

- Beide vollständigen `.jsonl` sichern.
- Fund in `INTEGER_LLM/README/Fahrplan-v3.md` eintragen.
- **Nicht** vorschnell auf die Hardware schieben. Der wahrscheinlichste
  Grund ist eine Gleitkomma-Operation, die in den Rechenpfad geraten ist.
  `INTEGER_LLM/tests/audit/test_no_float.py` ist der erste Griff.

## B9. Welche Hardware lohnt sich

Nach abnehmendem Erkenntniswert:

| Kombination | Was sie prüft |
|---|---|
| **x86_64 + aarch64** | Verschiedene Befehlssätze, verschiedene Compiler-Backends. Der wichtigste Vergleich. |
| **Referenz + NEON** | Ob die SIMD-Kernel wirklich bit-identisch sind. Nur auf aarch64: Ein AVX2-Pfad existiert noch nicht (Fund A19), und seit Fund 34 behauptet der Client auch nicht mehr, es gäbe einen. |
| **Linux + macOS + Windows** | libm- und Toolchain-Unterschiede. Hier hätte die alte `f64::exp()`-LUT zugeschlagen (Fund A5). |
| **Debug + Release** | Überlaufverhalten. Debug panickt, Release läuft um: genau der Unterschied aus Fund A14. |
| Zwei x86_64-Maschinen derselben Generation | Wenig. Nur als Rauschprüfung. |

**Big-Endian** wäre der schärfste Test überhaupt, das Protokoll ist
durchgehend Little-Endian kodiert. Realistisch verfügbar ist das kaum;
falls doch, hat dieser Lauf Vorrang vor allen anderen.

## B10. Teilnehmer ohne Modell einbinden

Wer die Artefakte nicht hat, liefert trotzdem Stufe 1 und Stufe 4. Der
Protokoll-Durchlauf prüft in etwa einer Sekunde:

| Stufe | Was geprüft wird |
|---|---|
| `krypto` | Merkle inkl. Negativprobe, BLS (Signatur, Fremdbotschaft, Aggregat), VRF |
| `epochenseed` | deterministisch, epochenabhängig, leerer Blockhash abgelehnt |
| `stichprobe` | 20 von 1000 Segmenten, sortiert, deterministisch |
| `komiteewahl` | 21 Producer + 7 Arbiter, rotiert über Epochen |
| `bft` | Quorum über Stimmgewicht; **Fremdstimme und Fremdsignatur abgelehnt** |
| `double_signing` | erkannter Beweis gilt, erfundener wird abgelehnt |
| `block` | kanonische Typen, `state_root` wirkt auf den Blockhash |
| `verifikation` | Abweichung lokalisiert, Schuld zugewiesen |
| `ledger` | Verifier-Entscheidung wird tatsächlich durchgebucht |
| `tokenomics` | EMA, Prägung, exp-LUT exakt, Preis richtungsrichtig |

Der Wert `stack_gesamt` muss bei gleichem Code auf **jeder** Maschine
identisch sein: Er enthält keine Zeitwerte und keine Zufallszahlen ohne
festen Seed. Weicht er ab, ist entweder der Code verschieden oder es
liegt eine Plattformabhängigkeit vor.

## B11. Was diese Tests **nicht** abdecken

Ehrlichkeitshalber, damit niemand mehr hineinliest, als drin ist:

- **Kein Netzwerkbetrieb.** Alles läuft in einem Prozess. `myl-net`
  (Gossip, Peer-Discovery) wird nicht berührt; echte Sockets gehören in
  die NETWORKING-Testsuite.
- **Keine Liveness.** Der BFT-Durchlauf prüft Safety (Quorum,
  Signaturen, Mitgliedschaft).
- **Keine Lastprüfung.** Zeiten stehen im Protokoll, weil sie bei der
  Fehlersuche helfen. Für Durchsatzmessungen gibt es
  `runtime/src/bin/bench_probe`.
- **Kein Krypto-Review.** Dass BLS und VRF *korrekt* implementiert sind,
  belegen die RFC-Testvektoren in `myl-types`, nicht dieser Client. Er
  prüft nur, dass die Aufrufe zusammenpassen.
- **Kein Sicherheitsaudit der Artefakte.** Ein manipuliertes Artefakt
  liefert einen anderen Digest, aber der Client sagt nicht, *warum*.

## B12. Meldevorlage für Teilnehmer

```
Testplan:        <plan_id>   (Einstellungs-ID: <einstellungen_id>)
Name im Lauf:    <teilnehmer>
Maschine:        z. B. Apple M2 Pro / Ryzen 5950X
Architektur:     aus dem Protokoll (arch)
Betriebssystem:  aus dem Protokoll (os)
Backend:         aus dem Protokoll (backend_selected)
θ_v:             aus dem Protokoll (theta_v)
Build:           release / debug

Fingerabdruck:   <fingerprint_sha256>
Determinismus:   <determinismus>          (oder: keine Artefakte)
Shard-Lauf:      <shard_vs_einzelknoten>  (oder: keine Artefakte)
Stack-Gesamt:    <stack_gesamt>

Auffälligkeiten: <Fehler/Abweichungen aus dem Protokoll, sonst "keine">
Anhang:          <name>_<einstellungen>_<datum>_<uhrzeit>.jsonl
```

---

## Changelog

### v2.6.0 – 2026-08-21 (Spirale, Rückweg)
- Der Regen läuft im Hintergrund weiter, während sich das Logo bildet.
- Startbild: Die Spirale wächst nach außen, während farbige Artefakte auf
  ihren Armen nach innen strömen und in der Mitte das Logo bilden; danach
  gleitet es an seinen Platz.
- Aus dem Gespräch führen Escape, Strg-D und getippte Wörter zurück; der
  Hinweis steht in jeder Eingabezeile. Enter allein tut nichts mehr.
- Die Antwort erscheint Wort für Wort, während gerechnet wird.

### v2.5.0 – 2026-08-21 (Inferenz, Menüaufbau)
- Die aktuellen Einstellungen stehen jetzt **unter** dem Menü.
- Nach der Namenseingabe läuft eine geschriebene Begrüßung.
- **Nach dem Nutzernamen geht es direkt ins Menü.** Die Testdatei fragt
  Punkt [3] ab, wenn gemessen werden soll.
- **Neuer Punkt [1]: Mit dem Modell sprechen.** Freie Eingabe, höchstens
  64 Token je Antwort, kein Protokoll.
- **Artefakt wählen** ist jetzt Punkt [4] und ein eigener Schritt. Die
  Testdatei beschafft nichts mehr ungefragt.
- **Protokolle vergleichen** ist ins Entwickler-Menü gewandert: Es ist die
  Arbeit des Koordinators, nicht die des Teilnehmers.
- Startbild: Zwischen Regen und Schriftzug sammelt sich das Gefallene
  kreisend in der Bildmitte.

### v2.4.0 – 2026-08-21 (Alles löschen, Frischklon)
- **Alles löschen** im Entwicklermenü, mit zwei Bestätigungen.
- Starter bauen jetzt zuverlässig auch dann, wenn sie aus einem anderen
  Verzeichnis aufgerufen werden (Verknüpfung auf dem Schreibtisch).
- Der Beispielplan heißt `standard.plan`; er gilt für jedes Modell.

### v2.3.0 – 2026-08-21 (Fenster, breites Logo)
- **Fenstergröße und Lage** setzt der Starter, nicht der Client: mittig,
  120 x 40. Ein Programm im Terminal kann sein Fenster nicht zuverlässig
  bewegen.
- **Das Logo füllt die Fensterbreite.** Der Schriftzug bleibt unverzerrt
  und steht mittig, das Netzmotiv wird für die jeweilige Breite erzeugt.
- Hinweis zur einmaligen Berechtigungsfrage unter macOS aufgenommen.

### v2.2.0 – 2026-08-21 (aufgeräumter Bildschirm, flachere Ablage, Farbe)
- **Farbe:** Menütitel fett in wechselnden Neontönen, das Logo bei jedem
  Erscheinen in einer anderen. Farbe trägt keine Aussage.
- **Vor jeder Auswahl wird aufgeräumt:** Logo oben, darunter nur das, was
  ansteht. Nach einer Aktion wartet der Client auf einen Tastendruck,
  damit ihre Ausgabe gelesen werden kann, bevor sie weicht.
- **Protokolle liegen jetzt in `TESTCLIENT/logs/`** statt zwei Ebenen
  tiefer in `myl-testclient/logs/`. Ein Teilnehmer, der seine Datei
  verschicken soll, hat im Quellcodeverzeichnis nichts zu suchen.

### v2.1.0 – 2026-08-21 (Vergleichsordner)
- **`TESTCLIENT/Vergleiche/`** als Ablage der zugesandten Protokolle,
  `Vergleiche/Berichte/` für den ausführlichen Bericht. `myl-test
  vergleich` liest den ersten und schreibt in den zweiten; Menüpunkt [3]
  lässt zwischen zugesandten und eigenen Protokollen wählen.

### v2.0.0 – 2026-08-21 (nach Rollen geteilt, für Laien geschrieben)
- **Zwei Teile statt sieben Kapitel.** Teil A führt einen Teilnehmer ohne
  Vorkenntnisse vom Doppelklick bis zum verschickten Protokoll; Teil B
  behandelt das Verfahren des Koordinators ausführlich. Die alte
  Mischung zwang beide Rollen, den Text der jeweils anderen zu
  überspringen.
- **Bedienung neu beschrieben:** Pfeiltasten und Enter, Namenseingabe
  beim Start, ein Protokoll je Testlauf statt vier.
- **Auswertung neu:** `myl-test vergleich` ersetzt die
  `grep`-Anleitung; die Urteilstabelle nennt jetzt die fünf Urteile des
  Befehls samt Folgerung.
- **Testpläne:** mehrere Prompts je Plan, Laufzeitrechnung mit Beispielen,
  zweiter mitgelieferter Plan für das 7B-Modell.
- **Neu aufgenommen:** Plattenbedarf und das Freigeben von Artefakten und
  Gewichten.

### v1.2.0 – 2026-08-20
- Testplan-Auswahl beim Start, Artefaktbeschaffung, Meldevorlage.

### v1.1.0 – 2026-08-18
- Abschnitt „Was hier eigentlich gemessen wird" für Leser ohne Bezug zu
  Sprachmodellen; Stolpersteine, Urteilstabelle.

### v1.0.0 – 2026-08-18
- Erstfassung, nach Rollen getrennt.
