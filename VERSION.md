0.8.0

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.8.0 (2026-11-09)

⚠️ **Neue Regeln für Prägung und Preise.** Eine Kette aus einer älteren
Fassung lässt sich nicht weiterführen; ein Knoten beginnt sie neu.

- **Adressen tragen jetzt eine Prüfsumme** (`tmyl1…` im Testnetz), ein
  Tippfehler fällt also auf. Aus zwölf Wörtern entstehen Konten über einen
  festen Pfad, und jedes Wallet zeigt seinen Verlauf.
- **Wallets lassen sich mit einem Passwort schützen.** `myl wallet neu` fragt
  danach, `myl wallet verschluesseln <name>` stellt ein vorhandenes um; danach
  liegt der Schlüssel nur noch verschlüsselt auf der Platte. Ein Passwort lässt
  sich nicht wiederherstellen, die Sicherung bleiben die zwölf Wörter.
- **„Mining starten“ rechnet mit**: Mit gewähltem Wallet und Modell meldet sich
  der Knoten als Miner an und startet seinen Teil des Modells selbst.
- **Module von Dritten laufen abgeschottet.** Ein Modul als WebAssembly hat von
  sich aus weder Dateien noch Netz. Rechner, die es erreichen möchte, gibst du mit
  `/module genehmigen <modul>` frei.
- **Die Tür bedient mehrere Anfragen zugleich**, und parallele Anfragen gehen
  nicht mehr verloren, wenn eine die andere überholt.
- **Prägung und Preise folgen der Nachfrage:** Die Anlaufsubvention läuft nach
  festem Plan aus, und der Burn ist je Epoche gedeckelt.
- **Ein Knoten kann über einen Anker von außen einsteigen** und prüft ihn selbst;
  ein zu alter Anker wird abgewiesen.
