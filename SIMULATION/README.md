# Protokollsimulation und Sicherheitsaudit

> **Version:** `myl-simulation` 0.1.1
> **Datum:** 2026-08-26
> **Status:** Ein Szenario läuft (Segmentweg über alle Schichten) mit
> Abdeckungsbericht. `latency_sim.py` und `security_sim.py` aus M1 sind
> noch Platzhalter.


Ein Ort für das, was keine einzelne Komponente prüfen kann: **den Weg
eines Segments durch alle Schichten.**

Die Begründung ist durch die Fundhistorie dieses Projekts belegt:
Fast jeder schwere Fund dieses Projekts saß nicht *in* einer Komponente,
sondern **zwischen zweien**. Fund 52 ist das jüngste Beispiel — Pod und
Konsens waren jeder für sich getestet, und ihre Bündelbotschaften passten
nicht zusammen.

Die Simulation liegt bewusst **außerhalb** der Komponenten-Crates: Sie
darf an alle hängen, und keine hängt an ihr.
