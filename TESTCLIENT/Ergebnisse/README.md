# Ergebnisse. Was du dem Koordinator schickst

Hier landet das Ergebnis deines Prüfstandslaufs, und nur das. Eine
Datei je Lauf, `.jsonl` und `.log` nebeneinander:

```
anna_32eafffb_2026-09-11_143022.jsonl    ← diese Datei geht raus
anna_32eafffb_2026-09-11_143022.log      ← dieselbe Auskunft zum Lesen
```

**Schick die `.jsonl`.** Sie ist die maschinenlesbare Fassung, die der
Vergleich auswertet. Die `.log` daneben ist für dich: derselbe Lauf in
Sätzen, falls du nachsehen willst, was gemessen wurde.

## ⚑ Warum dieser Ordner von `logs/` getrennt ist

`logs/` ist die Werkbank. Dort landet **jeder** Lauf: der abgebrochene,
der von Hand über die Befehlszeile gestartete, der dritte Versuch.

Dieser Ordner ist das Regal. Hier liegt genau das, was weitergegeben
werden soll.

**Der Unterschied ist der Grund für den Ordner.** Bis zum 2026-09-11
endete ein Lauf mit einem Pfad nach `logs/`, und wer sein Ergebnis
verschicken wollte, musste aus einem Verzeichnis voller Dateien die
richtige heraussuchen. Wer dabei die falsche greift, schickt einen
Abbruch, und das fällt erst dem Koordinator auf.

## Was im Dateinamen steht

`<name>_<kennung>_<datum>_<uhrzeit>`

Die **Kennung** ist die Prüfsumme des Prüfstands. Sie ist auf allen
Maschinen dieselbe, solange alle denselben Client benutzen; weicht sie
ab, hat jemand mit anderen Werten gemessen, und der Vergleich legt
seinen Lauf in eine eigene Gruppe.

⚠️ **Nicht umbenennen.** Der Name trägt die Zuordnung, und dieselben
Angaben stehen noch einmal im Protokoll. Ein umbenanntes Protokoll ist
nicht falsch, aber es kostet den Koordinator eine Rückfrage.

## Wenn eine Stufe abgewichen ist

**Dann schick es erst recht.** Eine Abweichung ist der interessante
Fall: Genau dafür läuft dieser Test. Ein Ergebnis, das niemand sieht,
weil es „nicht gut aussah", ist der einzige Weg, diesen Test wertlos zu
machen.
