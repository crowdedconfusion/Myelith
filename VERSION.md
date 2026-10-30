0.6.0

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.6.0 (2026-10-30)

⚠️ **Neue Regeln für Gebühren und Belohnungen.** Eine Kette aus einer
älteren Fassung lässt sich nicht weiterführen; ein Knoten beginnt sie neu.

- **Die Gebühr zählt jetzt auch die Prüfung einer Unterschrift**, nicht nur
  die Grösse in Bytes; das ist bei einer Überweisung der teuerste Teil der
  Arbeit eines Knotens. Die Vorschau im Wallet zeigt die Gebühr wie bisher,
  bevor etwas unterschrieben wird.
- **Viel mehr Überweisungen je Sekunde:** Ein Block prüft seine
  Unterschriften auf einmal, und der Wartebereich eines Knotens bleibt auch
  unter grosser Last schnell. Weist ein Knoten eine Überweisung ab,
  meldet er das jetzt, statt sie als verschickt zu bestätigen.
- **Validatoren werden nach ihrer Arbeit bezahlt:** nach Einsatz mal Zahl
  der Prüfpunkte, die sie unterschrieben haben. Wer nicht unterschreibt,
  bekommt aus diesem Topf nichts.
- **Jede Antwort rechnen zwei Pods:** Bezahlt wird ein Miner nur noch, wenn
  der Zwilling seines Pods dasselbe Ergebnis bezeugt. Die Antwort an den
  Nutzer wartet darauf nicht.
  Rechnet in einem kleinen Netz ein Pod allein, kommt seine Arbeit ganz
  in die Stichprobe und wird erst nach der Streitfrist bezahlt.
- **Mehr Anfragen zugleich:** Jede Sitzung wird einem der Pod-Paare des
  Netzes zugelost, und ein Knoten bedient mehrere Anfragen gleichzeitig.
  Wächst das Netz, rechnen alle Paare, nicht nur das erste. Ein Pod ohne
  Partner wartet in Bereitschaft, bis genug Miner für ein weiteres Paar da
  sind.
- **Der Preis für Credits folgt der Auslastung des Netzes:** Ist es
  wenig ausgelastet, werden Credits von Epoche zu Epoche billiger, ist es
  voll, langsam teurer. Wie viele Credits ein Kauf bringt, rechnet die Kette
  zum Preis der laufenden Epoche.
- **Ein Miner, der Zeitproben verweigert, wird gesperrt** wie einer, der
  seine Zusage nicht hält.
- **Die Zeitprobe eines Miners lässt sich nicht mehr vorrechnen:** Jede
  Probe bekommt eine neue Zufallseingabe, und der Miner unterschreibt seine
  Antwort.
- **Fällt ein Miner mitten in einer Epoche aus, springt die Reserve
  seines Pods ein** und rechnet seinen Teil weiter; bezahlt wird, wer die
  Arbeit unterschreibt.
- **Stichproben bekommen Antwort:** Jedes Mitglied eines Pods hebt die
  Rechenspuren seines Pods auf und liefert sie auf Nachfrage mit einem
  Beweis, dass sie zur eingereichten Arbeit gehören.
- **Falsch gerechnete Arbeit kostet den Einsatz:** Prüfer rechnen gezogene
  Stichproben nach; widersprechen sich zwei Pods, rechnet das Netz genau die
  strittige Stelle nach, und wer falsch gerechnet hat, verliert seinen
  Einsatz. Der ehrliche Zwilling bekommt seine Vergütung zurück. Einen
  Einsatz hinterlegt ein Miner jetzt beim Start (`--einsatz`).
- **Deutlich mehr Durchsatz:** Ein Knoten nimmt im Probenetz rund 1700
  Überweisungen je Sekunde auf; ist der Wartebereich voll, kommt zuerst
  durch, wer die höhere Gebühr zahlt.
- **Ein Knoten ohne volle Kontenliste** (`--zustandslos`): Er prüft jeden
  Block mit einem Beweis über genau die Konten, die der Block berührt, und
  braucht dafür nur einen Bruchteil des Speichers. Er steigt an einem
  bestätigten Prüfpunkt ein, statt die ganze Geschichte nachzuholen, und
  findet bei einem Rückstand selbst wieder Anschluss. Er rechnet, reicht
  ein und wird bezahlt wie jeder Miner.
- **Für volle Knoten:** `--baum-auf-platte` legt den grossen Teil des
  Zustands auf die Platte statt in den Speicher; `--zeugen-bereithalten`
  liefert Knoten ohne volle Kontenliste die Beweise, damit sie nicht von
  einem einzigen Knoten abhängen.
- **Ehrliche Miner werden nicht mehr fälschlich verdächtigt:** Eine leere
  Antwort auf eine andere Frage zählt nicht mehr als verweigerte Zeitprobe.
