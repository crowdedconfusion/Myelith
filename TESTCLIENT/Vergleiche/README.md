# Vergleiche — Arbeitsfläche des Koordinators

Hier legst du die Protokolle ab, die dir die Teilnehmer geschickt haben,
und lässt sie vergleichen:

```bash
myl-test vergleich
```

Ohne weitere Angabe liest der Befehl **diesen** Ordner und schreibt
seinen Bericht nach `Berichte/`.

## Ablauf

1. Alle eingegangenen `.jsonl` hier hineinlegen. Umbenennen ist nicht
   nötig und nicht erwünscht: Der Dateiname trägt Teilnehmer,
   Einstellungs-Kennung, Datum und Uhrzeit, und dieselben Angaben stehen
   noch einmal im Protokoll.
2. `myl-test vergleich` — oder im Menü Punkt [3], dort „Zugesandte
   Protokolle".
3. Das Urteil steht auf dem Bildschirm, der ausführliche Bericht in
   `Berichte/`.

## Was hier **nicht** hingehört

- **Eigene Läufe.** Die stehen in `TESTCLIENT/logs/`. Der Menüpunkt
  lässt zwischen beiden wählen; wer sie vermischt, vergleicht am Ende
  seine eigene Maschine mit sich selbst.
- **Bestätigte Ergebnisse.** Ein erbrachter Cross-Hardware-Nachweis
  gehört nach `INTEGER_LLM/eval/results/` (Fahrplanpunkt 2.3). Dieser
  Ordner ist Arbeitsfläche und wird nicht versioniert; was hier liegt,
  ist beim nächsten Klon weg.

## Warum ein eigener Ordner

Der Vergleich liest **alles**, was er an `.jsonl` findet. Läge er über
dem eigenen Protokollverzeichnis, mischten sich die zugesandten Läufe mit
den eigenen — und ein Urteil über eine Gruppe, in der die eigene Maschine
mehrfach vertreten ist, sagt etwas anderes aus, als es zu sagen scheint.

Der Bericht landet aus demselben Grund in einem **Unterordner**: Läge er
neben seiner Eingabe, würde er beim nächsten Aufruf mitgelesen.
