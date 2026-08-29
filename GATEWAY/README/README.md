# gateway (`myl-gateway`)

> **Version:** –
> **Datum:** 2026-08-29
> **Status:** Konzept und Phasen stehen, noch keine Zeile Code.

Die Tür zum Netz für alle, die nicht mitrechnen: Nutzeranfragen
entgegennehmen, an einen Pod geben, das Ergebnis samt Beleg
zurückliefern. Referenzimplementierung der Gateway-Rolle aus Whitepaper
Kap. 3.3.

## Aufgabe

**Das Netzwerkmodell soll auch ohne unseren Agent Layer benutzbar
sein.** Wer eine eigene Agenten-Umgebung betreibt, eine fremde
Bibliothek oder gar nichts davon, soll das Modell wie jede andere
Inferenz-Schnittstelle ansprechen können. Der Agent Layer ist ein
Aufsatz, kein Zugangsweg.

⚑ **Das Gateway ist dabei kein neuer Mechanismus, sondern der Ort, an
dem sechs vorhandene zusammenkommen:** die ganzzahlige Inferenz
(INTEGER_LLM), die Pod-Pipeline (COMPUTE_PIPELINE), Credits aus
verbrannten MYL (TOKENOMICS), der Session-Kontrakt als Vollmacht
(SHARED_TYPES), die Wahl des Auslieferungsmodus (VERIFICATION) und der
verschlüsselte Kanal (NETWORKING). Neu ist allein die Fläche nach außen.

⚑ **Und eine Sicherheitsaufgabe hängt daran, die heute niemand
wahrnimmt.** Kap. 6.7 sieht vor, dass Gateways Kontrollsegmente mit
einem Anteil γ in den Auftragsstrom einschleusen. Die Mechanik dafür ist
in VERIFICATION gebaut und geprüft; **eingeschleust wird bisher
nichts**, weil es die Stelle nicht gibt, an der es geschähe.

## Was es ausdrücklich nicht leistet

**Ohne Plan gilt die Kettenzusage nicht.** Führt eine fremde Umgebung
die Schrittfolge, kann das Protokoll nicht mehr belegen, dass keine
Schritte ausgelassen, eingefügt oder vertauscht wurden (Kap. 8.4);
ebenso wenig greift die architektonische Trennung gegen eingeschleuste
Anweisungen (Kap. 8.3). **Beides sind Zusagen des Agent Layer, nicht des
Netzes.**

Was bleibt, ist genau das, wofür jemand hierherkommt: Budget, Frist und
Empfängerliste bleiben durchgesetzt, und **jedes einzelne Segment bleibt
nachrechenbar**. Der Beleg sagt das ausdrücklich, statt es offenzulassen.

## Abhängigkeiten

COMPUTE_PIPELINE (rechnet), CONSENSUS (Credits und Kontrakt),
VERIFICATION (Auslieferungsmodus, Kontrollsegmente), NETWORKING
(verschlüsselter Kanal), SHARED_TYPES (Kontrakt- und Segmenttypen).

## Struktur

Noch keine. Fünf Entwurfsfragen sind entschieden, eine bleibt offen:
**wer das Gateway bezahlt.**

## ⚑ Die offene Frage, und sie ist wirtschaftlich

Die Rollentabelle in Kap. 3.3 nennt als Anreiz einen „Anteil der
Inferenzgebühr". **Eine Inferenzgebühr in diesem Sinn gibt es im
gebauten Modell nicht:** Nutzer verbrennen MYL zu Credits, die Credits
werden beim Rechnen ersatzlos verbraucht, und vergütet wird aus frisch
geprägten MYL. Die Prägung teilt sich in fünf Anteile, und Gateways sind
keiner davon.

Es ist also nicht so, dass der Anteil zu klein wäre. **Es gibt ihn
nicht.** Zu entscheiden ist, ob Gateways ein sechster Anteil der Prägung
werden oder ob sie außerhalb des Protokolls abrechnen; beides hat
Folgen, die über diese Komponente hinausgehen.
