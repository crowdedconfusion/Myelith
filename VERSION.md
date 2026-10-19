0.2.0

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.2.0 (2026-10-19)

- **Eigene Modelle bauen:** In der Konsole führt `/build artifact` Schritt
  für Schritt durch den Bau: Link auf Hugging Face oder Pfad zu einem Ordner,
  Prüfung der Bauart, bei einem Expertengemisch das Format (INT4 oder INT8),
  ein Name nach dem Schema, Platz, Bau, danach laden. Ohne Konsole geht
  dasselbe mit `bash INTEGER_LLM/scripts/modell_bauen.sh`.
- **Private Modelle:** Selbst gebaute Modelle und solche, deren Lizenz keinen
  Dienst für Dritte erlaubt (etwa Qwen3.8-Flash-Next), heissen in der Wahl
  „(privat)“, haben keinen Eintrag fürs Netz, und ein Knoten lädt sie nicht.
- **Befehle eines Moduls heissen jetzt `/module <modul> …`**, etwa
  `/module trading status` oder `/module cad`. Im Fenster werden keine
  Befehle mehr getippt: Was im Eingabefeld steht, geht an das Modell, die
  Befehle eines Moduls stehen in der Liste am Modul.
- **Der Agent:** längere Antworten ab Werk (4 096 Token), eine abgerissene
  Antwort oder ein abgerissener Werkzeugaufruf wird weitergeschrieben,
  Werkzeugaufrufe werden nachsichtiger gelesen, und ein Lauf bekommt genug
  Kontext, auch wenn viele Experten im Speicher liegen.
- **Qwen3.8-Flash-Next** lässt sich auf einer großen Maschine bauen (nur
  lokal).
