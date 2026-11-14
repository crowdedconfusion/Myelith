# Protokollsimulation und Sicherheitsaudit

> **Version:** `myl-simulation` 0.2.0
> **Datum:** 2026-11-14 (Zeitmessung für den Skalarkörper)
> **Status:** Ein Szenario läuft (Segmentweg über alle Schichten) mit
> Abdeckungsbericht. `latency_sim.py` und `security_sim.py` laufen seit
> dem 2026-09-01 und **behaupten**, statt nur zu drucken.


Ein Ort für das, was keine einzelne Komponente prüfen kann: **den Weg
eines Segments durch alle Schichten.**

Die Begründung ist durch die Fundhistorie dieses Projekts belegt:
Fast jeder schwere Fund dieses Projekts saß nicht *in* einer Komponente,
sondern **zwischen zweien**. Fund 52 ist das jüngste Beispiel — Pod und
Konsens waren jeder für sich getestet, und ihre Bündelbotschaften passten
nicht zusammen.

Die Simulation liegt bewusst **außerhalb** der Komponenten-Crates: Sie
darf an alle hängen, und keine hängt an ihr.

⚑ **Was das am 27. August gekostet hätte, wenn niemand hingesehen
hätte:** Die Naht „Urteil → Ledger" buchte mit einer getippten `0` als
Vorgeschichte. Eine Naht, deren Zweck das Zusammenspiel dreier
Komponenten ist, hätte damit auch dann grün gemeldet, wenn niemand
Verstöße gezählt hätte. Sie fährt jetzt über
`urteil_buchen_gestaffelt` und prüft, dass ein gebuchtes Urteil auch
zählt — **eine Naht, die ihre eigenen Eingaben erfindet, prüft nichts.**

## Die Zahlen aus Anhang B (`zahlen/`)

Die Programme unter `README/Whitepaper/simulations` drucken zum großen Teil
nur ihr Ergebnis. `zahlen/zahlenprobe.py` hält sie fest: Jede Ausgabe muss
ihrer Golden-Datei gleichen, und jede Zahl, die der Anhang nennt, muss im
Whitepaper und in der Ausgabe stehen. Ändert sich ein Programm mit Absicht,
schreibt `--neu` die Golden-Dateien; die Zahlen des Anhangs bleiben auch dann
geprüft. Belegt unter macOS mit Python 3.9 und unter Linux mit 3.12, die
Ausgaben sind bitgleich.

Was die Probe nicht leistet: Sie sagt nichts darüber, ob ein Modell richtig
ist, nur, dass es dasselbe liefert wie das, was im Whitepaper steht.

⚠️ Zwei Stellen des Anhangs haben kein Programm: der adaptive Angriff und das
Aufschaukeln über Layer in B.5.6, und B.8 seit dem Entfernen von
`genesis_supply_sim.py`.

## Changelog

### 0.2.0 (2026-11-14): Eine Zeitmessung für den Skalarkörper

- **Neu `examples/zeitprobe.rs`**: prüft nach dem Verfahren dudect
  (Reparaz, Balasch, Verbauwhede 2017), ob die Rechnungen des Skalarkörpers
  aus `myl-types` mit geheimen Zahlen in konstanter Zeit laufen. Fester
  gegen zufälligen Wert in zufälliger Reihenfolge, Welch-t über alle Proben
  und die schnellsten Anteile; dazu eine Kontrolle zufällig gegen zufällig
  und eine Gegenprobe, die absichtlich verzweigt und auffallen muss. Auf
  aarch64 wahlweise mit Data Independent Timing.
- **Ergebnis auf einem Apple M5 Pro** (acht Läufe zu 200 000 Proben): Die
  Inversion, 255 Quadrate und Multiplikationen über dem Geheimnis, zeigt nie
  einen Unterschied (|t| bis 2,5), die Kontrolle bleibt unauffällig, die
  Gegenprobe fällt jedes Mal auf. ⚠️ Die kurzen Einzelrechnungen ergaben
  kein stabiles Bild (|t| zwischen 2 und 55, abhängig von der Anordnung);
  nicht als Abhängigkeit belegt, nicht ausgeschlossen. Festgehalten im
  Sicherheitsaudit unter A13.

### 0.1.5 (2026-11-08)

- `zahlen/zahlenprobe.py` mit einer Golden-Datei je Programm und 44 Zahlen
  aus Anhang B; die CI führt sie statt des bloßen Ausführens.
- **Fund 669:** `training_capacity_sim.py` zählte beim gebündelten Beweis die
  Geschwister im Teilbaum mit (1,00 % statt 0,63 % bei 16 Segmenten). B.6.4 hat
  recht; das Programm folgt ihm jetzt.
- **Fund 670:** `integer_determinism_sim.py` rechnete die Division aus B.5.4
  gar nicht, obwohl B.9 sie ihm zuschreibt. Jetzt tut es das.
- **Fund 671:** `accum_alternatives_sim.py` schloss gegen die eigene Tabelle
  (alle Angriffe „erkennbar“, in Abschnitt 4 steht „NICHT erkannt“). Der Text
  folgt jetzt den Zahlen. Die Variante ist verworfen, an keinem Ergebnis ändert
  das etwas.
