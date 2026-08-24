# Protokollsimulation und Sicherheitsaudit

Ein Ort für das, was keine einzelne Komponente prüfen kann: **den Weg
eines Segments durch alle Schichten.**

Die Begründung steht in AGENTS.md und ist durch die Fundhistorie belegt:
Fast jeder schwere Fund dieses Projekts saß nicht *in* einer Komponente,
sondern **zwischen zweien**. Fund 52 ist das jüngste Beispiel — Pod und
Konsens waren jeder für sich getestet, und ihre Bündelbotschaften passten
nicht zusammen.

Die Simulation liegt bewusst **außerhalb** der Komponenten-Crates: Sie
darf an alle hängen, und keine hängt an ihr.
