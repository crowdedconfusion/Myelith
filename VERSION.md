0.11.0

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.11.0 (2026-11-18)

⚠️ **Seit 0.11.0 gelten neue Regeln für die Abrechnung der Pods und für die
Saat.** Eine Kette von 0.10.0 lässt sich nicht weiterführen; ein Knoten
beginnt sie neu.

- **Befehle des Agenten laufen in einer Sandbox des Betriebssystems**
  (Seatbelt unter macOS, Landlock unter Linux): schreiben nur im
  Arbeitsordner, kein Netz, nicht lesen im übrigen Benutzerordner. Auch was
  die Dateiwerkzeuge schreiben, geht durch sie. Abschalten mit
  `agent.sandbox`; die Startzeile sagt, was gilt.
- **Eine Runde kostet Prompt und Ausgabe**, und der Klient kauft vor jeder
  Runde nur so viele Credits nach, wie sie braucht. Fällt eine Runde auf
  einen Epochenwechsel, versucht er es selbst noch einmal.
- **Updates werden geprüft:** Ein neuer Stand wird nur eingespielt, wenn er
  die Signatur eines Freigabeschlüssels aus `SYSTEM/install/freigabeschluessel`
  trägt. Solange dort keiner steht, sagt die Aktualisierung, dass sie nicht
  prüft.
- **Schneller auf Apple ab M2:** Die Vorbereitung eines Prompts rechnet mit
  int8-Modellen bis zu 40 Prozent schneller, Token für Token dasselbe
  Ergebnis.
- **Mining nimmt, was freigegeben ist** (Kerne, Grafikkarten), und misst
  beim Start, wo jede Matrix am schnellsten läuft.
- **Sicherheit:** Steuerzeichen des Modells in gelesenen Texten oder
  Webseiten schreiben keine eigene Runde mehr in den Verlauf, und die
  Dateiwerkzeuge folgen keinem Verweis, der nach ihrer Prüfung entstand.
