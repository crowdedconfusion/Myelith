# Diagnoseskripte

Messwerkzeuge aus den Fehlersuchen von INTEGER_LLM. **Keines davon ist Teil
des Auslieferungspfads** — sie dürfen Gleitkomma benutzen, weil sie
Referenzmessungen durchführen und nicht rechnen.

## Warum sie stehen bleiben

Die Sammlung sieht nach Ballast aus: 34 Dateien, viele davon aus einer
Frage entstanden, die längst beantwortet ist. Sie bleibt trotzdem, und
zwar aus drei Gründen.

**Ein Fund ohne sein Instrument ist schwer nachzuprüfen.** Der Changelog
hält fest, *was* gemessen wurde; diese Dateien halten fest, *womit*. Wer
eine Zahl anzweifelt, kann sie damit reproduzieren.

**Die Fragen kommen wieder.** Vor dem Sprung auf ein wesentlich größeres
dichtes Modell stellen sich genau dieselben: Wo sitzt der Fehler, welche
LUT kostet wie viel, trägt die Skalenwahl. Ein Werkzeug neu zu schreiben
ist teurer, als es zu behalten.

**Auch die falschen Instrumente sind Belege.** Elf Instrumentenfehler sind
in dieser Suche aufgetreten, jeder einzelne gefunden durch ein
unmögliches Ergebnis und keiner durch Codelesen. Wer die betroffenen
Skripte löscht, löscht die Lehre mit.

Zusammen sind es 216 KB. Der Platz ist nicht das Argument.

## Was aktuell ist

| Skript | Frage |
|---|---|
| `position_layer_error.py` | Ebenenfehler über **alle** Positionen, ein Instrument, eine Referenz |
| `scheme_position_error.py` | Zeigt das **Schema allein** denselben Positionsverlauf? |
| `token_scale_simulation.py` | Was kostet die statische Skala gegenüber einer dynamischen? |
| `activation_scale_simulation.py` | Was kostet **eine** Skala je Ebene für die Zwischenaktivierungen? |
| `layer_stage_compare.py` | Operationsweiser Vergleich einer Ebene gegen Gleitkomma |
| `layer_bulk_error.py` | Akkumuliert der Quantisierungsfehler über die Ebenen? |
| `w8a16_reference_simulation.py` | Der Boden des Quantisierungsschemas (+0,84 %) |
| `fortschritt.py` | Fortschrittsbalken mit Restzeitschätzung (Hilfsmodul) |

## Was Geschichte ist

Die übrigen stammen aus abgeschlossenen Abschnitten der Suche. Zwei
verdienen eine Warnung, weil ihr Ergebnis **überholt** ist:

- `nonlinearity_ablation.py` sagte für das SiLU-Eingangsraster rund
  0 % Perplexitätswirkung voraus. Der Tensorvergleich fand 6,83 %, und die
  Neumessung gab dem Tensorvergleich recht. Die Simulation bildete die
  Wechselwirkung mit der nachfolgenden Multiplikation nicht ab.
- `score_raster_probe.py` maß nur die Softmax-**Ausgabe** und fand ±0 %.
  Der Langkontext-Defekt (Fund 29) lag im Eingangsraster und blieb dabei
  unsichtbar. Ersetzt durch `runtime/src/bin/attn_probe.rs --ebene`.

## Gemeinsame Regeln

**Referenz ist float32, nicht bfloat16.** Bei einem Wert von 1704 beträgt
die bf16-ULP 8; an einer Auslöschung trägt die Referenz dann mehr Fehler
als das Gemessene (zehnter Instrumentenfehler, 2026-08-20).

**Jede Sonde braucht eine Selbstprüfung** — und die muss den fraglichen
Baustein einschließen. `attn_probe` bestand ihre Prüfung bei n=1 mit
0,00 %, genau dem Fall, in dem das fehlende RoPE nichts tut.

**Läufe über einer Minute schätzen ihre Dauer** und geben einen
Fortschrittsbalken aus (`fortschritt.py`, Ausgabe mit `python -u`).
