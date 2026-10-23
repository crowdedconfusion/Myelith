0.4.0

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.4.0 (2026-10-23)

- **Knoten finden einander im lokalen Netz** ohne jede Angabe einer
  Gegenstelle (Suche im lokalen Netz); ein Netz kann dazu eine Liste
  langlebiger Einstiegsknoten mitbringen.
- **Ein neuer Knoten kann bei einem Zustandsabzug beginnen** statt jeden Block
  ab dem ersten nachzuholen (`--abzug <hoehe>:<blockhash>`). Geglaubt wird nur
  ein Abzug, der genau zu diesem Anker passt. Im Hintergrund rechnet der
  Knoten die Blöcke bis dorthin trotzdem nach und hält an, wenn sie nicht zum
  Abzug führen. Jeder Knoten legt an jeder Epochengrenze selbst einen Abzug
  ab und liefert ihn anderen.
- **Schneller und verlässlicher nachholen:** von bis zu vier Gegenstellen
  zugleich, und Blöcke, die in falscher Reihenfolge ankommen, werden
  aufgehoben statt verworfen. Unter Laufzeit und Paketverlust hielt ein Knoten
  vorher dauerhaft Abstand zur Spitze.
- **Behoben:** Knoten wählten die interne Adresse anderer Knoten und
  erreichten dabei sich selbst; mehrere Knoten auf einem Rechner wiesen
  einander ab; der Testverkehr veränderte den Zustand nie.
- **Eine Rust-Fassung für alles** (1.99.0, festgelegt in
  `rust-toolchain.toml`). Ohne Netz baut ein Rechner mit der Fassung weiter,
  die er hat.
