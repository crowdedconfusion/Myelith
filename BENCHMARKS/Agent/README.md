# Agentenmessung

Misst, ob die Werkzeugschleife eine **Aufgabe erledigt**.

⚑ **Der Unterschied zu den anderen beiden Ordnern ist der Prüfer.**
Inferenz und Training messen an einer Zahl, die aus dem Modell selbst
fällt. Ein Agentenlauf braucht ein Urteil über die Außenwelt: Ist die
Datei entstanden, steht das Richtige darin, nennt die Antwort ein Wort,
das nur aus einer Werkzeugantwort stammen kann.

```text
python3 BENCHMARKS/Agent/agentenprobe.py INTEGER_LLM/artifacts/myelith-4b
python3 BENCHMARKS/Agent/agentenprobe.py <artefakt> --stufe 1 --laeufe 3
```

## Die drei Stufen, und warum sie getrennt bleiben

| Stufe | Was sie prüft | Aufträge |
|---|---|---|
| 1 | Ruft das Modell überhaupt ein Werkzeug, in gültiger Form | `auflisten`, `lesen` |
| 2 | Verwertet es die **Antwort** des Werkzeugs | `lesen_und_folgern`, `waehlen` |
| 3 | Verändert es die Außenwelt richtig | `schreiben`, `lesen_dann_schreiben` |

⚑ **Ein Modell kann Stufe 1 können und an Stufe 2 scheitern.** Beides in
eine Quote zu werfen ergäbe eine Zahl, die nichts erklärt.

## Was die Messung nicht anfasst

⚑ **Je Auftrag ein frisches Verzeichnis.** Ein Auftrag, der in den
Überresten des vorigen läuft, kann bestehen, ohne etwas getan zu haben;
genau diese Sorte stiller Erfolg ist das, wovor die übrigen Proben
dieses Projektes warnen.

⚑ **Und die Ablage des Nutzers bleibt, wie sie war.** Einhängung und
Schreibrecht kommen über `--wurzel` und `--schreiben`, also je Lauf. Eine
Messung, die die Konfiguration des Rechners umschreibt, ist keine
Messung, sondern ein Eingriff: Wer sie zweimal fährt, misst beim zweiten
Mal etwas anderes.

## Rückgabewert

⚑ **Null auch bei Fehlschlägen.** Dies ist eine Messung und keine
Prüfung: Drei von sechs ist ein Ergebnis und kein Fehler des Laufs. Wer
eine Schranke will, liest die JSON-Datei aus `--json`.

## Der erste Lauf (2026-09-08, Qwen3-4B)

`results/erstlauf-myelith-4b-2026-09-08.log`, zwei Aufträge von Hand, noch
nicht die Sammlung:

```text
  → verzeichnis
  ← {"name": "verzeichnis", "content": "notizen.md\t84 Bytes"}
Das Arbeitsverzeichnis enthält die folgende Datei: notizen.md (84 Bytes)

  → datei_lesen pfad=notizen.md
  ← {"name": "datei_lesen", "content": "# Notizen\n\nDas Geheimwort lautet Goldfisch.…"}
Das Geheimwort ist **Goldfisch**.
```

⚑ **Stufe 1 und Stufe 2 bestanden.** Der zweite ist der eigentliche
Nachweis: Das Modell hat das Werkzeug gewählt, den richtigen Pfad
übergeben, die Antwort **gelesen** und daraus geschlossen. „Ein Werkzeug
wurde gerufen" wäre Stufe 1 allein.

⛑ **Was das nicht ist:** eine Messung. Zwei Aufträge von Hand sind ein
Nachweis, dass die Kette trägt, und keine Quote. Dafür gibt es
`agentenprobe.py`, und die ist noch nie gelaufen, weil den ganzen Tag
ein Trainingslauf die Maschine hatte.

## ⛑ Was noch fehlt

**Ein Lauf der ganzen Sammlung**, sechs Aufträge über drei Stufen, mit
`--laeufe 3`. Erst der ergibt Zahlen; die beiden oben ergeben eine
Gewissheit.

## ⚑ Der Vergleich, der als Nächstes ansteht (angelegt 2026-09-09)

`--deutsch` schaltet die Werkzeugansage auf die Fassung vor dem
2026-09-09 zurück: deutsche Werkzeugnamen, deutsche Beschreibungen und
eine Paraphrase der Vorlage statt der Vorlage selbst. Ohne den Schalter
läuft die amtliche Form, also die, auf die Qwen3 geschliffen ist.

**Der Lauf, der zu fahren ist**, sobald der Trainingslauf die Maschine
freigibt:

```sh
python3 BENCHMARKS/Agent/agentenprobe.py INTEGER_LLM/artifacts/myelith-4b \
  --laeufe 3 --json BENCHMARKS/Agent/results/amtlich-<datum>.json
python3 BENCHMARKS/Agent/agentenprobe.py INTEGER_LLM/artifacts/myelith-4b \
  --laeufe 3 --deutsch --json BENCHMARKS/Agent/results/deutsch-<datum>.json
```

⚑ **Der erste Lauf ist zugleich der Nullwert.** Diese Sammlung ist bis
heute nie vollständig gelaufen, es gibt also keine Zahl, gegen die
irgendetwas verglichen werden könnte. Wer nur den zweiten führe, hätte
zwei Zahlen ohne Bezug.

⛑ **Was der Vergleich nicht beantwortet.** Der Schalter ändert **drei
Dinge auf einmal**: den Werkzeugnamen, den Wortlaut der Ansage und die
Beschreibung. Fällt er zugunsten der amtlichen Form aus, ist belegt,
dass die Form des Modells hilft, **nicht welcher Teil davon**. Das ist
die richtige erste Frage; wer die zweite beantworten will, braucht drei
weitere Läufe.

⚠️ **Und was die Zahl nicht ist.** Sechs Aufträge sind wenig, und das
Modell zieht gierig. Bei drei Läufen je Auftrag stehen achtzehn
Ergebnisse je Arm gegen achtzehn; ein Unterschied von einem Auftrag ist
Rauschen. Nur ein deutlicher Ausschlag trägt eine Entscheidung, und
nichts spricht dagegen, ihn dann mit mehr Läufen zu erhärten.

## ⚑ Und der zweite Vergleich: drei Werkzeuge gegen fünf

Seit dem 2026-09-09 gibt es `search_files` und `edit_file`, aber
**nicht in der Vorgabe**. `--werkzeuge voll` nimmt sie dazu.

```sh
python3 BENCHMARKS/Agent/agentenprobe.py INTEGER_LLM/artifacts/myelith-4b \
  --laeufe 3 --werkzeuge voll --json BENCHMARKS/Agent/results/voll-<datum>.json
```

⛑ **Die Arme sind nicht gleich gross, und das ist Absicht.** Drei
Aufträge (`suchen_ueber_ebenen`, `suchen_statt_lesen`,
`aendern_statt_ueberschreiben`) brauchen die neuen Werkzeuge und werden
im knappen Arm **übersprungen**, nicht als Fehlschlag gezählt: Ein
Auftrag, den das Modell mangels Werkzeug nicht lösen *kann*, wäre keine
Aussage über das Modell, sondern über die Auswahl.

**Vergleichbar sind deshalb nur die sechs gemeinsamen Aufträge.** Die
Frage lautet: Wird das Modell auf den alten sechs *schlechter*, weil
zwei weitere Werkzeuge im Kontext stehen? Die drei neuen zeigen
daneben, ob die Werkzeuge überhaupt gefunden und richtig benutzt
werden.

⚑ **`aendern_statt_ueberschreiben` prüft das Entscheidende:** Die Datei
muss die neue Stelle enthalten **und** alles andere behalten. Ein
Agent, der zu `write_file` greift und die Datei neu schreibt, trifft
die verlangte Stelle vielleicht und löscht den Rest. Ohne
`datei_enthaelt_auch` sähe genau das wie ein bestandener Lauf aus.

**Kosten:** je Auftrag rund 165 Sekunden auf Qwen3-4B, also je Arm
etwa 50 Minuten und zusammen knapp zwei Stunden. Nicht neben einem
Trainingslauf fahren: Beide wollen dieselben Kerne, und die Messung
wird dadurch nicht falsch, aber unvergleichbar.
