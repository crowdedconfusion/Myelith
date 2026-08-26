# Anleitung: Tests mit mehreren Beteiligten und heterogener Hardware

> **Teil A und B** behandeln den Determinismus-Nachweis: Rechnen zwei
> Maschinen dasselbe Ergebnis, Bit für Bit.
> **Teil C** behandelt den Netzlauf: Finden mehrere Rechner einander
> über das Internet, und kommen die Nachrichten an. Zwei getrennte
> Nachweise, die einander nicht brauchen.

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
  ❯ 1  Artefakt wählen
        Welches Modell gerechnet wird. Liegt nur eines da, überspringt
        der Client die Frage.
    2  Testdatei wählen
    3  Testlauf starten
    4  Mit dem Modell sprechen
    5  Am Netz teilnehmen (Knoten betreiben)

    6  Anleitung lesen
    0  Beenden

  ↑ ↓ bewegen · Enter wählen · Ziffer direkt · Esc zurück
```

**Wo ist Punkt 9?** Das Entwickler-Menü erscheint nur, wenn beim
Nutzernamen **`admin`** eingegeben wurde (Groß- und Kleinschreibung
gleichgültig). Es enthält Punkte, die ein Teilnehmer nicht braucht und
mit denen er sich schaden kann, etwa das Löschen der Artefakte.

Das ist **kein Schutz, sondern eine Aufräumhilfe**: Der Name steht im
Quelltext, wer ihn kennt, kommt hinein. Er hält nur den Bildschirm frei
von Punkten, die niemanden angehen.

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

Der Client fragt dabei nur nach, was noch fehlt: Steht unten in den
Einstellungen bereits ein Artefakt und eine Testdatei, läuft er sofort
los. Beim ersten Öffnen steht dort „nicht ausgewählt", und er stellt
beide Fragen.

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

Punkt **[9] Entwickler-Menü** (erscheint nach Anmeldung als `admin`),
dort **Protokolle vergleichen**, fragt
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

## B4a. Nach einem Modellwechsel: `modellstaende`

`vergleich` fragt, ob zwei **Maschinen** dasselbe rechnen, und schließt
verschiedene Modellstände aus. Nach einem θ_v-Wechsel ist der Wechsel
aber genau der Gegenstand, und die Frage lautet nicht „gleich oder
nicht", sondern **„erwartet oder nicht"**:

```bash
myl-test modellstaende
```

Der Befehl liest denselben Ordner wie `vergleich` und stellt die
Vergleichswerte über die Modellstände hinweg gegenüber:

```
     Stände:
       [1] 0.17.0 / 97869982   Digest über logits+token
       [2] 0.17.0 / c42bb8a8   Digest über logits+token
       [3] 0.18.0 / c42bb8a8   Digest über logits+token

     determinismus    teils gleich          51d50d1c…  aca90b79…  aca90b79…

     [2] → [3]  unverändert: determinismus
```

**Die interessante Zeile ist die letzte, nicht die Tabelle.** Dass sich
Werte bei einem Modellwechsel ändern, ist der Normalfall. Ein Wert, der
einen Wechsel *unbeschadet übersteht*, ist der Befund: Entweder hängt er
gar nicht am Modell, oder die Änderung hat die Rechnung nicht erreicht.

Verglichen wird **je Paar von Ständen**, nicht über alle auf einmal. Eine
Zusammenfassung über alle Stände verdeckte genau diese Paare, und die
erste Fassung dieses Befehls tat das auch: Sie meldete „alles geändert",
während zwei Stände denselben Wert trugen.

**Der Befehl fällt kein Determinismusurteil** und endet immer mit
Exit-Code 0, solange er die Protokolle lesen konnte. Wer eine Erwartung
durchsetzen will, gibt sie am Messlauf mit `--erwarte` an:

```bash
myl-test --plan wikitext2-0.5b-standard.plan determinismus --erwarte aca90b797f1cf756
```

Der Lauf schlägt dann fehl, sobald er einen anderen Wert erzeugt. Die
Kurzform vom Bildschirm genügt; das Protokoll hält fest, über wie viele
Zeichen verglichen wurde.

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
diese Verweigerung ein Akzeptanzkriterium und keine
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
   SIMD-Paritätsfehler und gehört in den INTEGER_LLM.

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
- Den Befund schriftlich festhalten: welche Maschinen, welche
  Architekturen, welcher Modellstand, welche Werte auseinanderliefen.
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

# Teil C: Probelauf mit mehreren Knoten

> ## ⚑ Das ist **nicht** das Testnetz
>
> Ein Probelauf ist eine **Trockenübung des Codes**, keine
> Inbetriebnahme der Blockchain.
>
> - **Der Zustand ist Wegwerfware.** Jeder Start beginnt bei null. Keine
>   Fortsetzung, keine Wiederherstellung, keine Historie.
> - **Die MYL sind Spielgeld.** Wer hier Guthaben sieht, besitzt nichts.
> - **Ein Probeblock kann nie an eine echte Kette anschließen.** Er
>   hängt an einem Startwert, der ausdrücklich für Proben gewählt wurde
>   und im Klartext `MYELITH-PROBELAUF-KEIN-TESTNETZ` lautet.
> - **Es stimmt niemand ab.** Genau ein Knoten baut die Blöcke.
>
> Wann das Testnetz beginnt, entscheidet das Projekt, nicht dieser Code.
>
> **Wozu der Lauf dann gut ist:** Der Durchlauf aus Punkt [3] prüft die
> Bausteine **im selben Prozess**, zehn Stufen von der Kryptografie bis
> zur Preisbildung. Dort liegen die Werte im Speicher nebeneinander.
> Hier gehen dieselben Werte durch Serialisierung, Gossip,
> Größenprüfung, Strukturprüfung und Deserialisierung, über Maschinen-
> und Ländergrenzen. **Das ist ein anderer Weg mit anderen Fehlerarten**,
> und die Funde dieses Projekts liegen fast alle genau dort: an Nähten,
> nicht in Modulen.

## C0. Schnellstart: drei Rechner an drei Orten, Schritt für Schritt

Wer nur wissen will, was zu tun ist, folgt dieser Liste. Die Begründungen
stehen danach in C1 bis C10.

**Rollen vorab:** Eine der drei Maschinen ist die **Anlaufstelle**. Sie
muss aus dem Internet erreichbar sein: ein kleiner Mietserver, oder ein
Anschluss, an dem jemand eine Portweiterleitung einrichten kann. Die
beiden anderen dürfen an ganz gewöhnlichen Heimanschlüssen stehen.

Nennen wir sie **A** (Anlaufstelle), **B** und **C**.

---

**Schritt 1: Auf allen drei Maschinen den Testclient starten.**

Wer den Client schon für den Determinismus-Test benutzt hat, ist hier
fertig: **Der Knoten steckt darin.** Es gibt kein zweites Programm zu
bauen und keinen Pfad zu suchen.

Für Server ohne Bildschirm gibt es den Knoten auch einzeln:

```
cargo build --release --manifest-path NODE/myl-node/Cargo.toml
```

Beides braucht kein Modell, keine Gewichte und kein Python.

**Schritt 2: Auf A den Port öffnen.**

Port 4150, **für TCP und für UDP**. Der Knoten spricht beides, und je
nach Gegenüber wird das eine oder das andere gebraucht. Bei einem
Mietserver ist das eine Firewall-Regel, zu Hause eine Portweiterleitung
im Router.

**Schritt 3: Auf A die öffentliche Adresse herausfinden.**

```
curl -4 https://ifconfig.me
```

Die Zahl, die dabei herauskommt, heißt im Folgenden `DEINE-IP`.

**Schritt 4: Auf A den Knoten starten.**

```
myl-node --name anlaufstelle --rolle relais --port 4150 \
         --oeffentlich /ip4/DEINE-IP/tcp/4150 \
         --testverkehr 10 --aufnahme 30
```

Der Knoten nennt danach seine Adressen, die **quic-v1-Adresse zuerst**:

```
myl-node: erreichbar unter /ip4/DEINE-IP/udp/4150/quic-v1/p2p/12D3KooW…
myl-node: erreichbar unter /ip4/DEINE-IP/tcp/4150/p2p/12D3KooW…
```

**Die erste Zeile vollständig kopieren und an B und C schicken.** Sie ist
die Einladung ins Netz, der `/p2p/12D3KooW…`-Teil gehört dazu.

**Warum die quic-v1-Adresse und nicht die TCP-Adresse:** Der Transport
folgt der Adresse, die weitergegeben wird. Verteilst du die TCP-Adresse,
läuft das ganze Netz über TCP, auch wenn jeder Knoten QUIC spricht. Für
den Durchstich durch Heimrouter ist das der Unterschied zwischen
„gelingt meistens" und „gelingt selten". Die Auswertung sagt es dir
hinterher, aber dann ist der Lauf gelaufen.

**Schritt 5: Auf B den Knoten starten.**

```
myl-node --name maschine-b --port 4150 \
         --bootstrap /ip4/DEINE-IP/tcp/4150/p2p/12D3KooW… \
         --relais /ip4/DEINE-IP/tcp/4150/p2p/12D3KooW… \
         --testverkehr 10 --aufnahme 30
```

Zweimal dieselbe Adresse, und das ist kein Versehen: `--bootstrap` sagt
„dort finde ich das Netz", `--relais` sagt „dorthin soll man mich
erreichen können, wenn mein Router niemanden durchlässt".

**Schritt 6: Auf C dasselbe**, nur mit `--name maschine-c`.

**Die Namen müssen verschieden sein.** Sie benennen die Protokolldatei
und ordnen sie später zu.

**Schritt 7: Nachsehen, ob es geklappt hat.**

Auf B und C erscheint innerhalb weniger Sekunden eine Zeile mit
`"art":"verbunden"`. Kommt sie nicht, weiter bei C8.

**Schritt 8: Laufen lassen.**

Eine Stunde ist ein guter erster Wert. Beenden mit Strg-C, oder von
vornherein `--laufzeit 3600` anhängen.

**Schritt 9: Die drei Protokolle einsammeln.**

Auf jeder Maschine liegt eine Datei in `logs/`. Alle drei in **einen**
Ordner kopieren, am besten `TESTCLIENT/Vergleiche`.

**Schritt 10: Auswerten.**

Testclient starten, Entwickler-Menü, Punkt **8**. Teilnehmer sehen ihren
eigenen Lauf unter Punkt 5, Untermenü Punkt 2. Oder direkt:

```
myl-test netz --logs TESTCLIENT/Vergleiche
```

Was dabei herauskommen soll, steht in C6.

---

**Wenn keine Maschine öffentlich erreichbar ist:** Dann geht es nicht.
Irgendwo muss der erste Kontakt stattfinden, und wenn kein Rechner
Verbindungen annehmen kann, gibt es keine Stelle dafür. Ein kleiner
Mietserver für ein paar Euro im Monat reicht aus, er muss nichts können
außer erreichbar sein.

## C0a. Die Menüpfade auf einen Blick

| Wer | Wo | Was |
|---|---|---|
| Koordinator | [9] Entwickler → **[7]** | Knoten als Anlaufstelle betreiben |
| | | *[9] erscheint nach Anmeldung als `admin`* |
| Teilnehmer | Hauptmenü → **[5]** → [1] | Am Netz teilnehmen |
| Teilnehmer | Hauptmenü → **[5]** → [2] | eigenen Lauf ansehen |
| Koordinator | [9] Entwickler → **[8]** | Netzlauf auswerten |

Der Knoten steckt **im Testclient**. Es gibt kein zweites Programm zu
bauen. `myl-node` als eigenständiges Programm bleibt daneben bestehen,
für Server ohne Bildschirm.

## C1. Worum es hier geht, und worum nicht

Teil A und B beantworten **eine** Frage: Rechnen zwei verschiedene
Maschinen dasselbe Ergebnis, Bit für Bit. Teil C beantwortet eine
andere: **Finden mehrere Rechner einander über das Internet, und kommen
die Nachrichten an.**

Das sind zwei getrennte Nachweise, und sie brauchen einander nicht. Wer
nur den Determinismus prüfen will, liest Teil A und ist fertig.

**Was ein Netzlauf heute prüft:** dass Knoten einander finden, sich
verbinden, Nachrichten verbreiten und einander weiterreichen, auch hinter
einem Heimrouter. **Und seit dem 2026-08-24: dass sie aus denselben
Blöcken zum selben Zustand kommen.**

Die Anlaufstelle baut Blöcke, die anderen schicken Transaktionen und
rechnen die Blöcke nach. Am Ende vergleicht die Auswertung die
**Zustandswurzeln** Höhe für Höhe. Weicht eine ab, haben zwei Maschinen
aus denselben Daten verschiedene Zustände errechnet, und das bricht den
Konsens genauso wie ein abweichendes Inferenzergebnis.

**Was er nicht prüft: die Einigung.** Es stimmt niemand ab. **Genau ein
Knoten baut die Blöcke**, die übrigen übernehmen. Zwei Erzeuger würden
die Kette sofort gabeln, weil niemand entscheidet, welcher Block gilt,
und genau das täte eine Abstimmungsrunde (BFT). Die liegt fertig in
`myl-consensus`, ihr fehlen ein eigenes Gossip-Topic, ein Validator-Satz
mit Stake und Signaturschlüssel je Knoten. Ein neues Topic ist eine
Protokollentscheidung und gehört nicht nebenbei getroffen.

## C2. Was du brauchst

- Zwei bis vier Rechner. Sie dürfen an verschiedenen Orten stehen, das
  ist der Sinn der Sache.
- **Mindestens einer davon muss von außen erreichbar sein.** Ein
  Wurzelserver, ein kleiner Mietserver, oder ein Anschluss mit
  eingerichteter Portweiterleitung. Die übrigen dürfen hinter einem
  ganz gewöhnlichen Heimrouter stehen, dafür gibt es die
  Relais-Vermittlung.
- Das Programm `myl-node` auf jedem Rechner.

Kein Modell, keine Gewichte, kein Python. Ein Netzlauf braucht nichts
davon.

## C3. Den ersten Knoten starten (der erreichbare)

Auf dem Rechner mit öffentlicher Adresse: Testclient starten,
**[9] Entwickler-Menü**, dort **[7] Knoten als Anlaufstelle betreiben**.

Er fragt der Reihe nach:

| Frage | Was einzutragen ist |
|---|---|
| Name dieser Maschine | frei wählbar, benennt das Protokoll |
| Öffentliche IP-Adresse | die Zahl aus `curl -4 https://ifconfig.me` |
| Port | 4150, oder ein anderer freigegebener |
| Laufzeit in Minuten | 60 ist ein guter erster Wert |
| Testnachricht alle wie viele Sekunden | 10, oder 0 für keine |
| Teilnehmer | die Namen **aller** Maschinen, durch Komma getrennt |

Enter übernimmt jeweils den Wert in eckigen Klammern.

**Die Teilnehmerliste ist nicht optional.** Latenz-Atteste tragen eine
Signatur, und prüfen lässt sie sich nur gegen den Schlüssel des
Ausstellers. Die Zuordnung entsteht aus den Namen. Fehlt einer, werden
dessen Atteste verworfen; fehlt die Liste ganz, werden alle verworfen.
Das ist Absicht: Ungeprüfte Atteste durchzulassen wäre schlechter, weil
ein ungeprüftes Signaturfeld für einen Schutz gehalten wird, der es
nicht ist.

**Die Namen an alle Teilnehmer mitschicken**, zusammen mit der Adresse.

Für einen Server ohne Bildschirm geht dasselbe direkt:

```
myl-node --name relais --rolle relais --port 4150 \
         --oeffentlich /ip4/DEINE.OEFFENTLICHE.IP/tcp/4150 \
         --testverkehr 10
```

`--oeffentlich` ist bei `--rolle relais` **Pflicht**, und der Knoten
startet ohne sie gar nicht erst. Der Grund steckt in der Sache: Ein
Relais schreibt seine eigene Adresse in die Antwort an die Knoten, die
es vermittelt. Kennt es sie nicht, nimmt es Anfragen an und antwortet
ins Leere. Alles läuft, nur niemand kommt an. Deshalb ist es lieber ein
Fehler beim Start.

Beim Start nennt der Knoten drei Dinge:

```
myl-node: Peer-Id 12D3KooW…
myl-node: Protokoll logs/relais-1787590434078.jsonl
myl-node: erreichbar unter /ip4/…/tcp/4150/p2p/12D3KooW…
```

**Die letzte Zeile ist das, was die anderen brauchen.** Schick sie
weiter.

## C4. Die übrigen Knoten starten

Auf jedem weiteren Rechner: Testclient starten, im Hauptmenü
**[5] Am Netz teilnehmen**, dann im Untermenü **[1] Jetzt teilnehmen**.

Er fragt:

| Frage | Was einzutragen ist |
|---|---|
| Adresse vom Koordinator | die Zeile aus C3, vollständig, mit `/p2p/…` |
| Dein Name für das Protokoll | frei wählbar, je Rechner verschieden |
| Laufzeit in Minuten | dasselbe wie bei der Anlaufstelle |
| Testnachricht alle wie viele Sekunden | dasselbe wie dort |
| Teilnehmer | die Namen aller Maschinen, wie der Koordinator sie geschickt hat |

**Mehr ist nicht nötig.** Kein Port, keine Router-Einstellung, kein
Modell, kein Python. Der Client trägt die Adresse selbst zweimal ein:
einmal als Einstiegspunkt, einmal als Relais. Sitzt der Rechner hinter
einem Heimrouter, besorgt er sich dort eine Adresse, unter der andere
ihn erreichen, obwohl der Router nichts durchlässt.

**Die Adresse wird sofort geprüft.** Fehlt der `/p2p/…`-Teil, sagt der
Client es dir, bevor irgendetwas startet. Das ist der häufigste
Abtippfehler, und ohne diesen Teil kann keine Verbindung zustande
kommen.

Für einen Rechner ohne Bildschirm:

```
myl-node --name alpha --port 4150 \
         --bootstrap /ip4/…/tcp/4150/p2p/12D3KooW… \
         --relais /ip4/…/tcp/4150/p2p/12D3KooW… \
         --testverkehr 10
```

**Was `--testverkehr 10` tut:** Der Knoten schickt alle zehn Sekunden
eine Testnachricht ins Netz. Ohne das belegt der Lauf nur, dass die
Knoten einander **finden**, nicht dass Nachrichten **fließen**, und das
sind zwei verschiedene Aussagen. Für ein echtes Netz gehört die Angabe
weg: Dort wäre ein Knoten, der bedeutungslose Nachrichten einspeist, ein
Störer.

**Der Name muss je Rechner verschieden sein.** Er benennt die
Protokolldatei und ordnet sie später zu.

## C5. Laufen lassen und einsammeln

Der Knoten läuft die eingestellte Zeit und beendet sich dann selbst.
Strg-C bricht vorher ab; das Protokoll bleibt trotzdem lesbar, weil jede
Zeile sofort geschrieben wird.

Danach sammelst du von **jedem** Rechner die Datei ein und legst alle in
**einen** Ordner: `TESTCLIENT/Vergleiche`.

**Beide Arten dürfen dort zusammenliegen.** Determinismusläufe und
Betriebsprotokolle unterscheiden sich am Inhalt, nicht am Namen, und
jede Auswertung erkennt ihre eigenen. Sie sagt außerdem, wie viele
Dateien sie übergangen hat:

```
  3 Betriebsprotokoll(e), 1 andere Datei(en) übergangen (…)
```

Das ist Absicht. Wer nicht sagt, dass er Dateien liegen lässt, lässt
offen, ob sie fehlen oder nicht dazugehören.

**Was nicht mitgeschickt wird:** die Dateien aus `TESTCLIENT/Schluessel/`.
Das sind die privaten Kennungen der Knoten. Wer sie hat, kann im Netz als
dieser Knoten auftreten. Das Betriebsprotokoll nennt nur die
**öffentliche** Kennung, und die darf jeder sehen.

## C6. Auswerten

Im Entwickler-Menü: Punkt **8**, „Netzlauf auswerten". Oder auf der
Befehlszeile:

```
myl-test netz --logs TESTCLIENT/Vergleiche
```

Was dann kommt, sieht so aus:

```
  Knoten        Zeilen Dauer s    sah  abgew   empf  gesd
  gamma             25      20      1      0      9     4
  beta              25      20      1      0      9     4
  alpha             29      26      2      0      8     5

  Nachrichtenwege (Fingerabdruck der Nutzlast):
    381da344664206f8 von gamma → alle 2 erreicht
    547a30893d474de6 von beta  → alle 2 erreicht
    …

  Urteil: alle Knoten verbunden, alle Protokolle vollständig
```

## C7. Die Spalten und was sie bedeuten

| Spalte | Bedeutung |
|---|---|
| **Zeilen** | Einträge im Protokoll. Fehlt eine Nummer, meldet die Auswertung eine Lücke |
| **Dauer s** | Wie lange der Knoten lief, nach seiner eigenen Uhr |
| **sah** | Wie viele andere Knoten er gesehen hat. **0 ist das Alarmzeichen** |
| **abgew** | Abgewiesene Verbindungsversuche. Über null ist nicht automatisch schlecht: Der Knoten hat Verbindungsgrenzen, und dass sie greifen, gehört sichtbar zu sein |
| **empf** | Empfangene Nachrichten |
| **gesd** | Gesendete und vom Netz angenommene Nachrichten |

Darunter kommt zuerst die **Abdeckung**: welche Protokollfunktion in
diesem Lauf überhaupt ausprobiert wurde.

```
  Probelauf: welche Funktion wurde ausprobiert
  (dies ist eine Trockenübung des Codes, nicht der Beginn der Kette)

    Funktion           gesendet  gefehlt empfangen   belegt
    blockkette                8        0        16   aus denselben Blöcken …
    poi-buendel              10        0        20   PoI-Bündel überstehen …
    challenge                12        0        24   Challenges überstehen …
    transaktion              14        0        28   Transaktionen erreichen …
```

**Eine Probe, die nie lief, ist kein Erfolg.** Deshalb nennt die Tabelle
auch die Funktionen, für die nichts vorliegt, und darunter steht dann
ausdrücklich: „Über diese Funktionen sagt der Lauf nichts."

Danach kommen vier weitere Blöcke:

**Kette** nennt je Knoten die erreichte Höhe, wie viele Blöcke er selbst
gebaut und wie viele er übernommen hat, und darunter das wichtigste
Urteil des ganzen Berichts:

```
  ✓ Zustandswurzeln stimmen auf allen 7 vergleichbaren Höhen überein.
```

„Vergleichbar" heißt: von mindestens zwei Knoten belegt. Eine Höhe, die
nur einer kennt, zählt nicht, sonst sähe ein Lauf, in dem niemand
übernommen hat, wie ein bestandener Abgleich aus.

Steht dort stattdessen **⚠⚠ ZUSTANDSWURZELN WEICHEN AB**, ist das der
schwerste Befund, den dieser Lauf erzeugen kann. Schick den Bericht und
alle Protokolle zurück.

**Verbindungen nach Transport** zählt QUIC, TCP und Vermittlungen über
ein Relais. Steht dort überall QUIC 0, hat jemand die TCP-Adresse als
Einladung verteilt, und der interessanteste Teil der Messung hat nicht
stattgefunden.

**Lochstanzen (DCUtR)** zählt, wie oft aus einer vermittelten eine
direkte Verbindung wurde. **Auf einer Maschine immer null**, dort gibt
es nichts zu durchstoßen. Über getrennte Anschlüsse ist das die
wertvollste Zahl des ganzen Berichts.

**Paarlatenz** nennt die Spanne der gemessenen Laufzeiten. Ein
Höchstwert weit über dem Kleinstwert heißt Schwankung, und die erklärt
mehr als jeder Einzelwert. Ab Faktor zehn steht ein Hinweis daneben.

**Die Nachrichtenwege sind der eigentliche Ertrag.** Jede Nachricht
bekommt einen kurzen Fingerabdruck, und der steht sowohl beim Absender
als auch bei jedem Empfänger im Protokoll. Damit ist „kam an, was
losgeschickt wurde" keine Vermutung, sondern eine Textsuche, und zwar
**ohne dass die Uhren der Rechner übereinstimmen müssen**.

Deshalb urteilt die Auswertung auch bewusst **nicht** über Zeitpunkte:
Die Zeitstempel stammen von verschiedenen Maschinen und weichen
voneinander ab. Ein Werkzeug, das daraus eine Reihenfolge ableitet,
täuscht eine Genauigkeit vor, die es nicht gibt.

## C8. Wenn etwas nicht klappt

**„sah 0" bei einem Knoten.** Er hat niemanden erreicht. Der Reihe nach:
Stimmt die Bootstrap-Adresse buchstabengenau, inklusive des
`/p2p/12D3KooW…`-Teils? Läuft der erreichbare Knoten noch? Ist der Port
in der Firewall offen, und zwar **für TCP und UDP**? Der Knoten spricht
beides.

**Kein Eintrag „relais_reservierung", obwohl `--relais` angegeben war.**
Das Relais hat die Anfrage nicht bestätigt. Fast immer fehlt ihm
`--oeffentlich`, siehe C3.

**Einträge der Art „verworfen".** Der Knoten hat Nachrichten bekommen
und weggeworfen. Der Grund steht daneben:

| Grund | Bedeutung |
|---|---|
| `transportregel` | zu groß oder strukturell kaputt |
| `nutzlastpruefung` | ließ sich nicht als das lesen, was das Thema ankündigt |
| `fremdes-topic` | gehört nicht zu diesem Protokoll |

**Einträge der Art „attest_verworfen".** Ein Latenz-Attest hat die
Signaturprüfung nicht bestanden. Steht dabei `nutzlastpruefung`, ist
fast immer ein Name in der Teilnehmerliste vergessen worden, kein
Angriff. Der Eintrag sagt das auch dazu.

**„Diese Knoten kennen keinen Aussteller".** Dort fehlt die
Teilnehmerliste ganz. Solche Knoten verwerfen jedes Attest, und der Teil
des Laufs, der A10 prüfen sollte, findet nicht statt.

**Einträge der Art „block_abgelehnt".** Ein Block ist nicht in die Kette
gekommen. Der Grund steht daneben:

| Art | Bedeutung |
|---|---|
| `dublette` | schon übernommen. Gossip verbreitet mehrfach, völlig normal |
| `passt-nicht-an` | schließt nicht an den eigenen letzten Block an. Bei einem Knoten, der später dazukam, der Normalfall: Er hat die früheren Blöcke nie gesehen |
| `zustand-weicht-ab` | **der schwere Fall.** Der Block behauptet eine Zustandswurzel, die dieser Knoten aus denselben Transaktionen nicht errechnet |

**„Ohne Abschlusseintrag beendet".** Dieser Knoten ist abgestürzt, hart
abgeschossen worden, oder er läuft noch. Ein Lauf, der regulär endet
oder mit Strg-C abgebrochen wird, schreibt einen `ende`-Eintrag mit
Grund. Fehlt der, ist das Protokoll bis zur letzten Zeile trotzdem
brauchbar, aber der Grund für das Ende ist unbekannt.

**„Uhrversatz zwischen den Maschinen".** Die Auswertung misst ihn aus
den Daten: Wenn A eine Verbindung zu B vermerkt und B dieselbe zu A, ist
das ein Ereignis, zweimal notiert. Steht dort eine Warnung, hat eine
Maschine eine falsch gestellte Uhr, und alle zeitbezogenen Hinweise sind
mit Vorsicht zu lesen. Abhilfe: Zeitabgleich (NTP) einschalten.

**Viele `passt-nicht-an` und Höhe 0.** Dieser Knoten ist später
dazugekommen als der erste Block. **Es gibt keinen
Nachholmechanismus:** Jeder folgende Block zeigt auf einen Vorgänger,
den er nie gesehen hat, also lehnt er alles ab. Die Auswertung benennt
das von selbst.

Abhilfe für den nächsten Lauf: Alle Knoten starten lassen, **bevor** der
Erzeuger beginnt. Der Erzeuger wartet inzwischen von sich aus auf den
ersten Peer, aber wer mitten im Lauf dazukommt, hängt weiterhin fest.
Eine Blocksynchronisierung fehlt und gehört vor ein echtes Testnetz.

**Das ist die wichtigste Unterscheidung bei jeder Fehlersuche:** „nichts
kam an" und „es kam an und wurde weggeworfen" sehen von außen gleich
aus und haben völlig verschiedene Ursachen.

**Eine Lücke in der Folge.** Zwischen zwei Zeilen fehlt eine Nummer.
Entweder ist die Datei beim Kopieren beschädigt worden, oder der Knoten
ist mitten im Schreiben gestorben. In beiden Fällen trägt der Rest des
Urteils weniger weit, und die Auswertung sagt das dazu.

**„nur ein Knoten".** Es lag nur eine Datei im Ordner, oder alle Dateien
stammen vom selben Knoten. Über ein Netz sagt das nichts, genauso wenig
wie zwei gleiche Vergleichswerte von derselben Maschine über
Determinismus etwas sagen.

## C9. Was im Protokoll steht

Eine Zeile JSON je Ereignis, sofort auf die Platte geschrieben. Das ist
Absicht und kostet Geschwindigkeit: **Der interessanteste Zeitpunkt ist
der letzte vor einem Absturz**, und ein gepuffertes Protokoll verliert
genau die Zeilen, wegen derer man es liest.

Jede Zeile trägt `folge` (lückenlos), `zeit_ms`, `knoten` und `peer`.
Die letzten beiden sorgen dafür, dass eine eingesammelte Datei sich
selbst zuordnet, auch wenn jemand sie umbenannt hat.

Die Arten im Überblick: `start`, `horcht`, `horchadresse`, `bootstrap`,
`relais_reservierung`, `eigene_adresse`, `verbunden`, `getrennt`,
`abgewiesen`, `gesendet`, `empfangen`, `verworfen`, `aufnahme`, `ende`.

`aufnahme` ist die regelmäßige Zustandsmeldung. Sie ist der Gegenpol zu
den Ereignissen: Die sagen, **was** passiert ist, sie sagt, **wie es
steht**. Ohne sie ließe sich „zwanzig Minuten kam nichts" nicht von
„zwanzig Minuten lief nichts" unterscheiden.

## C10. Was dieser Lauf **nicht** abdeckt

- **Keinen Konsens.** Siehe C1.
- **Kein Lochstanzen zwischen zwei Heimanschlüssen.** Zwei Knoten
  hinter NAT können heute über das Relais miteinander sprechen. Ob sie
  danach eine direkte Verbindung aufbauen können, ist ungeprüft: Das
  lässt sich nur auf getrennten Anschlüssen messen, nicht auf einem
  Rechner. **Das ist die interessanteste Messung, die euer erster echter
  Mehrmaschinenlauf liefern kann**, und zwar getrennt nach TCP und QUIC.
- **Keine Aussage über Geschwindigkeit.** Die Auswertung zählt, sie misst
  keine Laufzeiten über Maschinengrenzen, weil die Uhren dafür nicht
  genau genug übereinstimmen.

---

## Changelog

### v2.14.0 – 2026-08-25 (Teilnehmerliste, Attest-Prüfung)

C3 und C4 fragen jetzt nach der **Teilnehmerliste**. Sie ist nicht
optional: Latenz-Atteste tragen eine Signatur, und prüfen lässt sie sich
nur gegen den Schlüssel des Ausstellers, dessen Zuordnung aus den Namen
entsteht (Sicherheitsaudit A10). Ohne Liste wird jedes Attest verworfen.

C8 erklärt die beiden neuen Meldungen: `attest_verworfen` und „kennt
keinen Aussteller". Beide sind fast immer eine unvollständige
Teilnehmerliste, kein Angriff, und der Bericht sagt das dazu.

### v2.13.0 – 2026-08-25 (Durchsicht vor dem ersten Mehrmaschinenlauf)

Eine gezielte Funktionsprüfung vor den ersten Läufen über getrennte
Maschinen, mit vier Funden.

⚑ **Der Bericht schlug bei einem gesunden Lauf Alarm.** Er meldete 51
von 78 Nachrichten als nicht angekommen, fast alle an Knoten gerichtet,
die zu dem Zeitpunkt noch nicht liefen oder schon beendet waren. Ein
Bericht, der bei einem gesunden Lauf Alarm schlägt, wird nicht gelesen.
Jetzt zählt nur, wer zur Sendezeit lief: 63 von 78 im selben Lauf, und
die restlichen sind echte Randfälle beim Ein- und Austritt.

⚑ **Die erste Fassung der Nachsicht war wirkungslos.** Sie nahm eine
Minute „gegen Uhrabweichung"; bei einem Lauf über sechzig Sekunden
deckte das den ganzen Lauf ab. **Eine Nachsicht muss kleiner sein als
das, was sie unterscheiden soll.** Jetzt fünf Sekunden, und der
tatsächliche Uhrversatz wird aus den Daten gemessen und im Bericht
genannt.

Neu außerdem: Der Bericht meldet Knoten ohne Abschlusseintrag
(abgestürzt oder noch laufend) und solche, die mit Strg-C abgebrochen
wurden.

Zwei Funde am Knoten selbst stehen in `NODE/README/README.md`, v0.5.0:
Es wurde nur die TCP-Adresse angezeigt, und der Mempool eines
Nicht-Erzeugers wuchs ohne Ende.

### v2.12.0 – 2026-08-24 (Probelauf statt Netzlauf)

**Umbenannt und neu gerahmt.** Teil C heißt jetzt Probelauf, und ganz
oben steht in einem Kasten, was er nicht ist: das Testnetz. Der Zustand
ist Wegwerfware, die MYL sind Spielgeld, ein Probeblock kann nie an eine
echte Kette anschließen.

Neu ist die **Abdeckungstabelle**: welche Protokollfunktion in diesem
Lauf ausprobiert wurde, wie oft, mit wie vielen Fehlschlägen, und wie
oft sie beim Gegenüber ankam. Eine Probe, die nie lief, ist kein Erfolg,
und das war vorher nicht zu unterscheiden.

⚑ **Der erste Probelauf deckte sofort eine Lücke auf:** Der Erzeuger
baute acht Blöcke, bevor die anderen verbunden waren; sie wiesen alle
acht zurück und blieben auf Höhe 0. Es gibt keinen Nachholmechanismus.
Der Erzeuger wartet jetzt auf den ersten Peer, und die Auswertung
benennt das Muster, wenn es doch auftritt. Die Ursache bleibt offen und
gehört vor ein echtes Testnetz.

### v2.11.0 – 2026-08-24 (echte Blöcke und Kettenabgleich)

Die Anlaufstelle baut jetzt echte, verkettete Blöcke aus einem Mempool;
die übrigen Knoten schicken Transaktionen und rechnen nach. Die
Auswertung vergleicht die Zustandswurzeln Höhe für Höhe.

Damit prüft ein Netzlauf zum ersten Mal **den Zustand**, nicht nur das
Netz. C1 und C7 sagen dazu, was weiterhin fehlt: die Einigung. Es
stimmt niemand ab, und genau ein Knoten erzeugt.

### v2.10.0 – 2026-08-24 (Transport, Lochstanzen, Latenz)

Der Bericht nennt jetzt drei Dinge mehr, die vorher fehlten:
Verbindungen nach Transport, gelungene und gescheiterte
Lochstanzversuche, und die Spanne der Paarlatenzen.

**Der Anlass war eine Lücke in dieser Anleitung selbst.** Sie nannte das
Lochstanzen die interessanteste Messung eines Mehrmaschinenlaufs, aber
der Knoten schrieb DCUtR-Ereignisse nirgends mit: Die Messung wäre auch
über getrennte Anschlüsse nicht zustande gekommen.

C3 sagt jetzt außerdem, dass die **quic-v1-Adresse** weiterzugeben ist,
nicht die TCP-Adresse. Der Transport folgt der verteilten Adresse, und
über UDP gelingt der Durchstich durch Heimrouter deutlich
zuverlässiger. Der erste Dreiknotenlauf lief vollständig über TCP, ohne
dass es jemandem aufgefallen wäre.

### v2.9.0 – 2026-08-24 (Schlüsselordner, gemischte Sammlung)

Private Knotenschlüssel liegen jetzt in `TESTCLIENT/Schluessel/` statt
dort, wo der Client gestartet wurde. Beim Doppelklick war das die Wurzel
des Repositoriums, und dort stand die Datei in keiner `.gitignore`:
**Sie konnte in einen Commit geraten.** Der Ordner schließt seinen
Inhalt aus, `*.key` steht zusätzlich in der Wurzel-`.gitignore`, und die
Datei bekommt auf Unix die Rechte 0600.

C5 sagt jetzt, dass beide Protokollarten in denselben Ordner dürfen und
dass jede Auswertung meldet, wie viele Dateien sie übergeht.

### v2.8.0 – 2026-08-24 (Teil C zum Durchklicken)

Teil C beschreibt jetzt durchgehend den Weg über das Menü; die
Befehlszeile steht nur noch als Alternative für Server ohne Bildschirm
daneben. Neu C0a mit allen vier Menüpfaden auf einen Blick, und für C3
und C4 je eine Tabelle, welche Frage welche Antwort will.

Der Grund für die Umstellung: Der Knoten steckt seitdem **im
Testclient**. Ein Partner baut ein Programm und klickt, statt zwei zu
bauen und Adressen von Hand zusammenzusetzen.

`EINSEITER.md` heißt jetzt `SCHNELLSTART.md` und ist auf Stichpunkte
umgestellt, weil er inzwischen **zwei** Tests beschreibt und die Zahl im
alten Namen nicht mehr stimmte.

### v2.7.0 – 2026-08-24 (Teil C: Netzlauf mit mehreren Knoten)

Neuer Teil C für Läufe mit `myl-node` über mehrere Maschinen, und
Menüpunkt 7 „Netzlauf auswerten". Beantwortet eine andere Frage als der
Vergleich: nicht ob zwei Maschinen dasselbe rechnen, sondern ob mehrere
Knoten einander gesehen haben und ob die Nachrichten angekommen sind.

Der Abschnitt sagt ausdrücklich, was ein Netzlauf **nicht** prüft: Die
Knoten produzieren keine Blöcke, und das Lochstanzen zwischen zwei
Heimanschlüssen ist ungeprüft, weil es sich auf einem Rechner nicht
messen lässt.

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
