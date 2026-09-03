# networking / siegel (`myl-siegel`)

> **Version:** 0.2.0
> **Datum:** 2026-09-03
> **Status:** Der vertrauliche Sitzungskanal, aus `myl-net` herausgelöst
> (B6-4). **54 Tests grün.**

## Aufgabe

Ende-zu-Ende-verschlüsselte Sitzungen zwischen zwei Endpunkten
(Whitepaper Kap. 9.2): hybrider Schlüsselaustausch aus **X25519** und
**ML-KEM-768**, abgeleitet über HKDF-SHA256, versiegelt mit
**ChaCha20-Poly1305**.

⚑ **Kein Transport.** Wer die Bytes bewegt, entscheidet der Aufrufer.
Diese Kiste kennt kein libp2p, kein tokio und kein `async`.

## ⚑ Warum sie eine eigene Kiste ist

Sie stand bis zum 2026-09-03 als `sitzung.rs` in `myl-net`, und dort
**hatte sie keinen Aufrufer** (Fund 155). Aufgefallen ist es, als
GATEWAY Stufe 4 sie brauchte: Der Shard-Prozess muss entsiegeln, denn
die Bindung bindet den Klartext und lässt sich erst danach prüfen. Über
`myl-net` hätte er dafür **181 zusätzliche Kisten** gebaut, über diese
**21**.

**Der Pfeil im Weg war das Symptom, nicht das Problem.** Sie importierte
nichts aus `myl-net`, öffnet keinen Socket, kennt kein `async`. Die
Komponente stimmte, der Kistenschnitt nicht.

⚑ **Die Alternative war ein Merkmalsschalter in `myl-net`**, und sie
war tragfähiger, als ich sie zunächst dargestellt hatte: Es gibt kein
Wurzel-Workspace, jede Kiste löst ihren eigenen Graphen auf. Entschieden
hat, dass ein Schalter **neun Module** hinter ein `cfg` gestellt hätte,
also zwei Kisten, die vorgeben, eine zu sein, und das auf dem
Konsenspfad. Dazu Tor A, wo ein hybrider PQ-Kanal das
prüfempfindlichste Stück im Baum ist.

## Warum der Transport nicht genügt

libp2p verschlüsselt jede Verbindung mit Noise, und für zwei Knoten, die
direkt sprechen, wäre damit alles gesagt. **Nutzer und erster Shard
sprechen aber nicht direkt**, sondern über ein Gateway: Mit
Transportverschlüsselung allein läge der Prompt dort im Klartext.

## Der Umschlag

⚑ **Seit dem hybriden Austausch reicht die versiegelte Nachricht
allein nicht.** Der KEM-Zweig hat eine Richtung: Der Absender kapselt
gegen den Kapselpunkt des Empfängers, und ohne das dabei entstehende
Chiffrat kann der Empfänger seinen Empfangsschlüssel gar nicht bilden.

`Umschlag` legt die Kapsel vor die Nachricht. **Er stand bis zum
2026-09-03 als Hilfsfunktion in einem Test von `myl-net`**; als
GATEWAY Stufe 4 ihn brauchte, wäre er ein zweites Mal geschrieben
worden, und zwei Rahmen für dieselbe Aussage laufen auseinander.

## Grenzen

`MAX_KLARTEXT_BYTES` ist **abgeleitet, nicht gesetzt**: aus
`myl_types::protocol::MAX_ANFRAGE_BYTES` abzüglich Kopf, Tag und
Längenpräfix. Eine versiegelte Nachricht muss durch den Anfragekanal
passen, sonst scheitert sie erst auf der Leitung.

## Was sie nicht leistet

**Kein Vorwärtsgeheimnis innerhalb einer Epoche.** Beide Seiten rechnen
denselben Schlüssel aus den angekündigten Punkten aus, ohne Handschlag
und ohne Umlaufzeit; das spart Latenz vor der ersten Aktivierung und
kostet genau diese Eigenschaft. Sie entsteht mit der Rotation.

## Changelog

### v0.2.0 – 2026-09-03 (⚑ Fund 159: der Umschlag passte nicht durch den Kanal)

`MAX_KLARTEXT_BYTES` zieht jetzt auch den **Kapselvorspann** ab. Die
alte Herleitung stammte aus der Zeit vor dem hybriden Austausch, als
eine versiegelte Nachricht allein auf die Leitung ging; seither geht sie
im Umschlag, und der ist 1 192 Byte länger.

⚑ **Gefunden hat es eine Simulation und kein Test**
(`umschlag_sim.py`): Jeder Test sah einen Umschlag, der aufgeht, und
keiner den grössten. Die Berichtigung ist eine Übersetzungszusicherung,
und die Simulation prüft zusätzlich, dass ein Byte mehr **nicht** passt.

### v0.1.0 – 2026-09-03 (aus `myl-net` herausgelöst, B6-4)

Der Umzug samt Tests, dazu zwei Zusätze, die beide aus dem Umzug
entstanden sind:

- **`Umschlag`**, aus einem Test von `myl-net` hierher gezogen, damit es
  nur eine Quelle für den Rahmen gibt.
- **`Sitzungen::kapselpunkt`**, symmetrisch zu `punkt()`. ⚑ **Beide
  gehören zusammen angekündigt:** Wer nur den X25519-Punkt nennt, lässt
  die Gegenstelle den KEM-Zweig nicht bilden, und dann ist die Nachricht
  unlesbar, ohne dass jemand sagen könnte, warum.
