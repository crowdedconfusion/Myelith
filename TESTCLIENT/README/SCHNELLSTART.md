# Myelith mittesten: Schnellstart

Du bekommst diese Seite, weil ein Testlauf auf **deiner** Maschine etwas
belegen kann, was auf unserer nicht belegbar ist.

Alles hier geht **durch Anklicken im Menü**. Pfeiltasten bewegen,
Enter wählt. Es gibt keine Befehle zu tippen.

---

## Es gibt zwei Tests. Du kannst einen machen oder beide.

| | **Test 1: Rechnung** | **Test 2: Netz** |
|---|---|---|
| **Frage** | Rechnen zwei Rechner dasselbe, Bit für Bit? | Halten die Protokollfunktionen, wenn die Daten über eine Leitung gehen? |
| **Menüpunkt** | [3] Testlauf starten | [5] Am Netz teilnehmen |
| **Brauchst du** | x86_64-Rechner, ~1,7 GB Platte, Python | nur die Adresse vom Koordinator |
| **Dauer** | Minuten | so lange du magst, eine Stunde ist gut |
| **Ergebnis** | eine `.jsonl` aus `TESTCLIENT/logs/` | eine `.jsonl` aus `TESTCLIENT/Vergleiche/` |

---

## Einmalig einrichten

- Rust installieren: <https://rustup.rs>. Sonst nichts.
- Repository klonen, Adresse steht in der Mail.
- Im Ordner `TESTCLIENT` den Starter doppelklicken:
  - Windows: `Myelith Testclient - Windows (Batch).cmd`
  - macOS: `Myelith Testclient - macOS.app`
  - Linux: `./"Myelith Testclient - Linux, macOS (Shell).sh"`
- Erster Start dauert einige Minuten, der Client baut sich selbst.
- Nutzernamen eingeben, wenn er fragt. (Als Koordinator: **`admin`**
  eingeben, sonst fehlt das Entwickler-Menü.)

**Kein Konto bei Hugging Face nötig.**

---

## Test 1: Rechnung (Determinismus)

- **[1] Artefakt wählen.** Liegt nur ein Modell da, überspringt er die Frage.
- **[3] Testlauf starten.** Bei der Frage nach der Testdatei die nehmen,
  die wir mitgeschickt haben.
- Durchlaufen lassen. **Nicht mit Strg-C abbrechen**, sonst ist das
  Protokoll unvollständig und der Vergleich weist es zurück.
- Die **`.jsonl`** aus `TESTCLIENT/logs/` zurückschicken. Die `.log`
  daneben ist dasselbe als Fließtext und ist für dich.

**Beide Protokollarten dürfen in denselben Ordner.** Der Koordinator legt
alles nach `TESTCLIENT/Vergleiche/`; jede Auswertung nimmt sich ihre
eigenen Dateien und sagt, wie viele sie liegen lässt.

**Voraussetzungen im Einzelnen:**

- **x86_64**, also ein gewöhnlicher Intel- oder AMD-Rechner. Ein
  Apple-Silicon-Mac hilft leider nicht: Wir haben schon einen, und zwei
  gleiche Maschinen beweisen nichts.
- **Python mit PyTorch**, rund 2 GB, nur für die beiden Modellstufen.
  **Ohne geht es auch**, siehe unten.
- **Platte:** rund 1,7 GB. Hinterher freigeben über [9] Entwickler,
  dann [6] Artefakte und Gewichte löschen.

---

## Test 2: Probelauf (mehrere Rechner)

> **Das ist nicht der Start der Blockchain.** Ein Probelauf ist eine
> Trockenübung: Der Zustand ist Wegwerfware, jeder Start beginnt bei
> null, und die MYL darin sind Spielgeld. Geprüft wird, ob der Code
> hält, wenn die Daten über echte Leitungen gehen.


- Warte auf die **Adresse vom Koordinator**. Sie sieht so aus:
  `/ip4/203.0.113.5/udp/4150/quic-v1/p2p/12D3KooW…`
  (Er schickt die quic-v1-Adresse; über UDP kommt die Verbindung durch
  Heimrouter zuverlässiger zustande.)
- **[5] Am Netz teilnehmen**, dann **[1] Jetzt teilnehmen**.
- Adresse einfügen, Namen und Laufzeit bestätigen.
- **Teilnehmerliste eingeben**, die der Koordinator mitgeschickt hat.
  Ohne sie werden die Latenz-Atteste der anderen verworfen.
- Laufen lassen. Der Client zeigt am Ende, wo das Protokoll liegt.
- Die Datei aus `TESTCLIENT/Vergleiche/` zurückschicken.

**Was du dafür nicht brauchst:** keinen offenen Port, keine
Router-Einstellung, kein Modell, kein Python. Nur die Adresse.

**Dein Schlüssel** landet in `TESTCLIENT/Schluessel/`. Er ist die
Kennung deines Knotens: **nicht mitschicken, nicht weitergeben.** Bleibt
er liegen, behält dein Rechner über Neustarts dieselbe Kennung, und nur
dann lassen sich mehrere Läufe zusammenführen.

**Was dabei passiert:** Der Koordinator baut Blöcke, dein Rechner
schickt Transaktionen und rechnet die Blöcke nach. Am Ende vergleicht
die Auswertung, ob alle beim selben Zustand gelandet sind. Du musst
dafür nichts tun.

**Dein eigenes Ergebnis ansehen:** [5], dann [2].

---

## Drei Dinge, die wir vorher sagen wollen

- **Ohne Python bist du trotzdem nützlich.** Der Lauf erhebt in jedem
  Fall die Hardware und fährt den Protokoll-Durchlauf über Kryptografie,
  Konsens und Ledger. Nur die beiden Modellstufen fallen aus. Das ist
  ein gültiges Teilergebnis, kein Fehlschlag.
- **Wenn etwas schiefgeht, ist das ein Ergebnis.** Der Client ist auf
  Windows noch nie von einem Menschen vollständig durchgespielt worden.
  Findest du einen Fehler, hast du genau das geliefert, wofür der Test
  da ist. Schick uns die Meldung im Wortlaut.
- **Wenn die Zeit knapp ist, sag Bescheid**, dann kürzen wir den Plan.
  Ein halber Lauf, der wie ein ganzer aussieht, wäre schlimmer als
  keiner.

---

## Was im Protokoll steht, und was nicht

**Drin:** Architektur, Betriebssystem, Backend, welches Modell, Zeiten,
die Vergleichswerte, die erzeugten Zahlen, ein **Hash** der Fragen. Beim
Netztest zusätzlich: mit wem dein Rechner gesprochen hat und ob
Nachrichten ankamen.

**Nicht drin:** der Klartext der Fragen und Antworten, dein
Benutzername, dein Rechnername, Seriennummern, MAC-Adressen.

Die Datei ist reiner Text. Du darfst sie vorher lesen.

---

## Wenn du mehr wissen willst

`TESTCLIENT/README/ANLEITUNG.md`:
**Teil A** erklärt Test 1 Schritt für Schritt, **Teil C** erklärt
Test 2. Beide mit einer Liste, was zu tun ist, wenn etwas klemmt.

Danke. Ohne eine zweite Maschine bleibt die wichtigste Behauptung des
Projekts unbewiesen.
