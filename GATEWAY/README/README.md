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

## ⚑ Zwei Rollen, nicht eine

Das Whitepaper führt unter „Gateways" zwei Aufgaben zusammen, deren
Vertrauensanforderungen entgegengesetzt sind.

**Die Tür** nimmt Anfragen an und leitet weiter. Sie braucht **kein
Vertrauen**: Der Kanal ist Ende zu Ende verschlüsselt, sie liest nichts,
und wenn sie lügt oder verschluckt, merkt der Nutzer es sofort. Jeder
darf eine betreiben, auch der Nutzer für sich selbst.

**Die Prüfeinspeisung** gibt Kontrollsegmente mit Anteil γ in den
Auftragsstrom (Kap. 6.7). Sie braucht **vollständiges Vertrauen**, denn
der Mechanismus wirkt nur, solange ein Miner die Kontrollen nicht
erkennt.

⚑ **Betreibt jeder sein eigenes Gateway, schleust niemand
Kontrollsegmente ein.** Ein Nutzer hat keinen Anreiz, für Köder zu
zahlen. Die Einspeisung gehört deshalb zu einer Rolle, die etwas zu
verlieren hat, und liegt nicht in dieser Komponente. Diese hier ist nur
die Tür.

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

Noch keine Zeile Code. ⚑ **Und das Crate wird kleiner, als der Name
vermuten lässt.** Beim Durchsehen, was hier wirklich hingehört, blieb
wenig übrig:

- **Der Beleg gehört nach SHARED_TYPES**, nicht hierher. Er ist kein
  neuer Typ, sondern das vorhandene `Segment` plus zwei Felder,
  Auslieferungsmodus und Reifegrad. ⚑ **Läge er hier, bräuchte jeder,
  der einen Beleg prüfen will, einen Webserver** — der Prüfweg muss der
  leichteste sein, nicht der schwerste.
- **Der Zugangsschlüssel** ist der Session-Kontrakt und steht in
  SHARED_TYPES und CONSENSUS.
- **Die Prüfeinspeisung** ist zu den Validatoren gewandert.
- **Zuteilung, verschlüsselter Kanal und Rechnen** stehen in
  COMPUTE_PIPELINE, NETWORKING und CONSENSUS.

**Hier bleibt die Fläche nach außen und der Zusammenbau.** Der Grund für
ein eigenes Crate ist genau einer: **Die HTTP-Abhängigkeit darf nicht in
ein Crate, das andere ohnehin einbinden.** Eine Rolle braucht ein
eigenes Crate, wenn sie eine eigene Angriffsfläche mitbringt, und sonst
nicht.

## Wer die Tür bezahlt

Die Rollentabelle in Kap. 3.3 nennt als Anreiz einen „Anteil der
Inferenzgebühr". **Eine Inferenzgebühr in diesem Sinn gibt es im
gebauten Modell nicht:** Nutzer verbrennen MYL zu Credits, die Credits
werden beim Rechnen ersatzlos verbraucht, und vergütet wird aus frisch
geprägten MYL. Die Prägung teilt sich in fünf Anteile, und Gateways sind
keiner davon.

**Entschieden ist deshalb ein dritter Weg: Der Auftraggeber hängt seiner
Anfrage ein Entgelt an**, fällig, sobald das Segment festgeschrieben ist
und die Tür darin steht. Keine Prägungsänderung, kein sechster Anteil.

⚑ **Der Grund ist, dass die naheliegende Alternative erprobt und
verworfen ist.** Erlaubnisfreies, unvergütetes Weiterleiten führt nicht
zu vielen kleinen Betreibern, sondern dazu, dass es nur noch die tun,
die ohnehin große Knoten betreiben. Andere Netze haben das durchlaufen
und nachträglich ein Entgelt je Zustellung eingeführt.

**Wer für sich selbst weiterleitet, zahlt an sich selbst, also nichts.**
Der Selbstbetrieb bleibt kostenlos; er ist nur nicht mehr die einzige
Möglichkeit.
