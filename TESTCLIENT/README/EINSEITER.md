# Myelith mittesten: eine Seite

Du bekommst diese Seite, weil ein Testlauf auf **deiner** Maschine etwas
belegen kann, was auf unserer nicht belegbar ist.

## Worum es geht, in vier Sätzen

Myelith rechnet Sprachmodelle in **ganzen Zahlen** statt in Gleitkomma.
Der Grund ist nicht Geschwindigkeit, sondern Nachprüfbarkeit: Ganze
Zahlen liefern auf jeder Maschine dasselbe Ergebnis, Gleitkomma nicht
zuverlässig. Darauf steht das ganze Projekt, denn ein Netz, in dem
Fremde füreinander rechnen, muss nachrechnen können, ob jemand
geschummelt hat. Bewiesen ist das erst, wenn zwei **verschiedene**
Rechner dieselbe Zahl liefern, und wir haben nur einen.

Ein Nachweis, der ausschließlich auf unseren eigenen Rechnern läuft, ist
außerdem kein guter Nachweis. Deine Maschine ist ein unabhängiger Zeuge.

## Was wir brauchen

| | |
|---|---|
| **Architektur** | **x86_64**, also ein gewöhnlicher Intel- oder AMD-Rechner. Ein Apple-Silicon-Mac hilft leider nicht: Wir haben schon einen, und zwei gleiche Maschinen beweisen nichts. Windows, Linux und Intel-Mac sind alle recht. |
| **Rust** | Einmalig von <https://rustup.rs>. Sonst nichts. |
| **Python mit PyTorch** | Nur für die beiden Modellstufen, rund 2 GB. Ohne geht es auch, siehe unten. |
| **Platte** | Rund 1,7 GB für das kleine Modell. Hinterher wieder freigebbar: im Menü [9] Entwickler, dort [6] Artefakte und Gewichte löschen. |
| **Zeit** | Erster Start einige Minuten, weil sich der Client selbst baut. Der Lauf danach: Minuten. |

Ein Konto bei Hugging Face brauchst du **nicht**.

## Was du tust

1. Repository klonen (Adresse steht in der Mail).
2. Im Ordner `TESTCLIENT` den Starter für dein System doppelklicken:
   `Myelith Testclient - Windows (Batch).cmd`,
   `Myelith Testclient - macOS.app` oder
   `./"Myelith Testclient - Linux, macOS (Shell).sh"`.
3. Nutzernamen eingeben, wenn er danach fragt.
4. **[1] Artefakt wählen.** Liegt genau ein Modell da, überspringt der
   Client die Frage.
5. **[3] Testlauf starten**. Er fragt nach der Testdatei: nimm die, die
   wir mitgeschickt haben. Danach durchlaufen lassen.
6. Die entstandene **`.jsonl`** aus `TESTCLIENT/logs/` zurückschicken.
   Die `.log` daneben ist dasselbe als Fließtext und ist für dich.

Mehr ist es nicht. Menü mit den Pfeiltasten, Enter wählt aus.

## Drei Dinge, die wir vorher sagen wollen

**Der Lauf muss durchlaufen.** Brichst du ihn mit Strg-C ab, ist das
Protokoll unvollständig, und unser Vergleich weist es zurück, statt es
halb zu verwerten. Das ist Absicht: Ein halber Lauf, der wie ein ganzer
aussieht, wäre schlimmer als gar keiner. Wenn dir die Zeit ausgeht, sag
uns Bescheid, dann kürzen wir den Plan.

**Ohne Python bist du trotzdem nützlich.** Der Lauf erhebt in jedem Fall
die Hardware und fährt den Protokoll-Durchlauf über Kryptografie,
Konsens und Ledger. Nur die beiden Modellstufen fallen aus. Das ist ein
gültiges Teilergebnis, kein Fehlschlag.

**Wenn etwas schiefgeht, ist das ein Ergebnis.** Der Client ist auf
Windows noch nie von einem Menschen vollständig durchgespielt worden.
Findest du einen Fehler, hast du genau das geliefert, wofür der Test da
ist. Schick uns die Meldung im Wortlaut.

## Was im Protokoll steht, und was nicht

**Drin:** Architektur, Betriebssystem, Backend, welches Modell, Zeiten,
die Vergleichswerte, die erzeugten Zahlen, ein **Hash** der Fragen.

**Nicht drin:** der Klartext der Fragen und Antworten, dein
Benutzername, dein Rechnername, Seriennummern, MAC-Adressen.

Die Datei ist reiner Text. Du darfst sie vorher lesen.

## Wenn du mehr wissen willst

`TESTCLIENT/README/ANLEITUNG.md`, Teil A, erklärt jeden Schritt
ausführlich, samt einer Liste dessen, was zu tun ist, wenn etwas klemmt.
Diese Seite hier ist die Kurzfassung davon.

Danke. Ohne eine zweite Maschine bleibt die wichtigste Behauptung des
Projekts unbewiesen.
