0.11.1

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.11.1 (2026-11-18)

- **Updates kommen nur noch mit der Signatur des Projekts.** Ab dieser
  Fassung spielt die Aktualisierung nur einen neuen Stand ein, den der
  Freigabeschlüssel `myelith-freigabe` signiert hat; wer den Zugang zu
  GitHub übernimmt, bringt damit allein keinen Code mehr auf einen
  Rechner. Dieses eine Update kommt noch ohne die Prüfung herein, weil die
  Liste der Schlüssel bisher leer war, und sagt das beim Einspielen.

Seit 0.11.0 außerdem:

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
- **Schneller auf Apple ab M2:** Die Vorbereitung eines Prompts rechnet mit
  int8-Modellen bis zu 40 Prozent schneller, Token für Token dasselbe
  Ergebnis.
- **Mining nimmt, was freigegeben ist** (Kerne, Grafikkarten), und misst
  beim Start, wo jede Matrix am schnellsten läuft.
- **Sicherheit:** Steuerzeichen des Modells in gelesenen Texten oder
  Webseiten schreiben keine eigene Runde mehr in den Verlauf, und die
  Dateiwerkzeuge folgen keinem Verweis, der nach ihrer Prüfung entstand.
