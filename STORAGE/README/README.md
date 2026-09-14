# STORAGE — die Rolle Store und die Verfügbarkeitsschicht

> **Version:** 0.5.2
> **Datum:** 2026-09-14
> **Status:** Gegenstandsformat, **Verfügbarkeitsnachweis** und
> **Speicherentgelt** stehen (`myl-store` v0.5.2, 22 Tests; Format und Stichproben-Ableitung liegen in `myl-types`). Sechs der
> sieben Entwurfsfragen sind entschieden, eine ausdrücklich nicht: Ob ein
> Gegenstand vervielfältigt oder erasure-kodiert wird, ist Latenz gegen
> Platz. **Die Platz-Zahl steht seit dem 2026-08-30**, die Latenz-Zahl
> wartet auf echten Abrufverkehr.
>
> ⚑ **Zwei Ablagen im Konsenszustand** (2026-08-31): Infrastruktur steht
> einzeln darin, Wissen nur über eine Wurzel. Der Grund ist die Bauart
> des Zustands-Commitments, das den ganzen Zustand serialisiert und
> hasht: Eine unbegrenzt wachsende Menge würde jeden Block belasten.
>
> ⚑ **Das Entgelt wird unabhängig vom Mining ausgezahlt** (Festlegung
> des Projektinhabers, 2026-08-30). Ein Knoten kann Wissen halten, ohne
> eine Token-Position zu rechnen. Drei Klassen: Protokollkritisches und
> **Netzwerkwissen** trägt die Allgemeinheit und beides verfällt nie,
> eine private Einlage zahlt ihr Einleger in Byte-Epochen und sie
> verfällt. **Ohne Nachweis wird nichts abgebucht und nichts
> ausgezahlt.**
>
> ⚑ **Fund 106 (2026-08-30): Der entworfene Nachweis belegte keine
> Speicherung.** Er verlangte Blatt und Merkle-Pfad, und die Blätter
> dieses Baums **sind** die Teil-Hashes: Wer nur sie hält, antwortet für
> immer richtig, bei 30 GB mit 0,87 MiB statt 30 GB. Die Antwort trägt
> jetzt den Teil selbst. Das ist die Voraussetzung für jedes
> Speicherentgelt, denn bezahlt werden darf nur, was bewiesen ist.

## Aufgabe

Das Netzwerk rechnet mit Dingen, die es irgendwoher bekommen muss:
Skalenpakete, Shard-Gewichte, Nachschlagetabellen, und künftig eine
**Wissensdatenbank** samt Skills. Bis heute regelt kein Kapitel und
keine Zeile Code, **woher**. Diese Komponente schließt die Lücke und
bildet zugleich die Netzwerkrolle **Store** ab, die Kap. 3.3 noch nicht
kennt.

## Warum das eine eigene Komponente ist

Weil die Anreize andere sind. Ein Shard-Miner wird aus der Prägung
bezahlt, die am Burn hängt. **Speicherung erzeugt keinen Burn.** Sie
wird deshalb wie das Training aus Kap. 7 aus dem Treasury finanziert und
ist eine nachrangige Arbeitsklasse mit eigener Vergütung, eigenem
Nachweis und eigener Verstoßfolge. Das in NETWORKING oder CONSENSUS
unterzubringen hieße, ein zweites Wirtschaftsmodell in einer Komponente
zu führen, die keines hat.

## Die vier Dinge, die es zu halten gilt

| Gegenstand | Größe | Leseprofil | Redundanzform |
|---|---|---|---|
| Skalenpakete (θ_v) | 1,8 MB | selten, von jedem | volle Kopien |
| Shard-Gewichte | GB je Shard | je Epochenwechsel, eilig | volle Kopien |
| Tabellen | klein | mit dem Artefakt | volle Kopien |
| Wissensdatenbank, Skills (κ_v) | wächst mit dem Netz | bei jeder Anfrage | Reed-Solomon |

**Warum nicht überall dasselbe:** Sieben volle Kopien überstehen sechs
Verluste und kosten das Siebenfache. Reed-Solomon mit k = 8 und m = 6
übersteht dieselben sechs Verluste und kostet das 1,75-fache. Dafür
liest man eine Kopie bei einem Halter und ein kodiertes Teil bei acht.
Für das Kalte lohnt der Tausch, für das Heiße nicht.

## Drei Dinge, die beim Entwurf auffielen

**Der Hash gehört auf den Klartext, nicht auf das Komprimat.** Zwei
zstd-Versionen komprimieren dieselben Bytes verschieden. Wer den Hash
des Komprimats verankert, macht den Kompressor zum Konsensvertrag, und
ein Bibliotheksupdate wird zum Betrugsvorwurf. Kap. 6.2 hat dieselbe
Unterscheidung für die Ausführung längst getroffen: Der Inhalt ist
verbindlich, die Kodierung nicht.

**Ein Hash belegt Empfang, nicht Speicherung.** Wer prüft und danach
löscht, besteht jede Prüfung, die nur den Hash des Ganzen verlangt.
Nötig ist Challenge-Response über einen zufälligen Byte-Bereich, und die
Bereiche müssen unvorhersehbar sein. Sonst wiederholt sich Fund 58
wortwörtlich: Dort erkennt ein Miner mit Gedächtnis nach 5.000 Aufträgen
96,8 % der Kontrollen ohne einen einzigen Fehlalarm, weil der Vorrat
endlich war.

**Siebenfach ist eine Invariante, kein Mittelwert.** Rotation muss erst
besetzen und dann freigeben. Andernfalls fällt die Replikation bei jedem
Rotationsschritt kurz unter die Schranke, und bei Tausenden Teilen ist
„kurz" ein Dauerzustand.

## Verhältnis zum Whitepaper

Kap. 3.3 kennt sechs Rollen, **Store ist keine davon**. Sidequest 2
beschreibt Bootstrapping und Dauerhaftigkeit als Lücke, ohne sie zu
schließen. Beides ist für die nächste Fassung des Papiers vorgemerkt;
das veröffentlichte v0.3 bleibt unangetastet.

## Stand

**Phase 1 steht** (`myl-store` v0.1.0). Wie beim Agent Layer entsteht
zuerst kein Speicher, sondern ein **Format**: was ein Gegenstand ist,
wie er in Teile zerfällt, was in seinem Manifest steht.

⚑ **Gehasht wird der Klartext, nicht das Komprimat.** Zwei
zstd-Fassungen komprimieren dieselben Bytes verschieden; wer den Hash
des Komprimats verankert, macht den Kompressor zum Konsensvertrag, und
ein Bibliotheksupdate wird zum Betrugsvorwurf. **Der Preis:** Ein
Verfügbarkeitsnachweis muss dann über den Klartext fragen, der Halter
entpackt also zum Antworten.

⚑ **Und eine Frage wurde ausdrücklich nicht entschieden.** Ob ein
Gegenstand vervielfältigt oder erasure-kodiert wird, ist Latenz gegen
Platz: Wer etwas **ganz** liest, holt es von acht Gegenstellen schneller
als von einer; wer in kleinen Stücken liest, zahlt acht Abrufe je
Stelle. **Beide Zahlen fehlen**, solange es keinen echten Abrufverkehr
gibt. Statt zu raten, ist die Wahl je Gegenstandsart einstellbar; die
Zahlen dahinter stehen als Test, nicht als Behauptung.

**Seither dazugekommen:** die Kapazitätszusage und die Zuteilung eines
Gegenstands an Halter (beide in `myl-types`), der
Verfügbarkeitsnachweis samt Quittung und das Speicherentgelt.
**Was noch fehlt:** die Diversitätsbedingung der Zuteilung, Rotation und
Mindestreplikation, die Bewertung einer Quittung und der Abruf.

## Changelog

*Nachgetragen am 2026-09-14.* Bis dahin führte dieses README keinen
Changelog, und die Prüfung, die Kopfversion und Changelog vergleicht,
übersprang es still (Fund 354). Die Einträge sind aus den
Versionssprüngen der Kiste und dem Planungsverlauf zusammengestellt.

### v0.5.2 – 2026-09-02 (der Satz des Speicherentgelts ist hergeleitet)

Modulkopf von `entgelt.rs`: Das Entgelt ist ein **Verhältnis zum
Credit-Preis**, kein eigener Parameter, weil ein fester Satz dem
dynamischen Rechenpreis davonliefe. Eine Byte-Epoche kostet real so viel
wie rund 1 420 Recheneinheiten, über fünf Empfindlichkeitsfälle
zwischen 793 und 2 840.

### v0.5.1 – 2026-08-31 (das Format wandert zu den gemeinsamen Typen)

`gegenstand.rs` ist nach `myl-types` umgezogen, weil Kapazitätszusage
und Zuteilung dort liegen und denselben Gegenstand brauchen. **Zwei
Ablagen im Konsenszustand:** Infrastruktur einzeln, Wissen über eine
Wurzel, weil das Zustands-Commitment den ganzen Zustand serialisiert.

### v0.3.0 – 2026-08-31 (Fund 106, und das Speicherentgelt)

`nachweis.rs`: Stichprobe je Halter aus Epochenseed, Wurzel, Fassung,
Epoche und Halterkennung. ⚑ **Fund 106:** Der Entwurf verlangte Blatt
und Merkle-Pfad, und die Blätter **sind** die Teil-Hashes; wer nur sie
hält, besteht jede Stichprobe, bei 30 GB mit 0,87 MiB. Die Antwort trägt
jetzt die Bytes. `entgelt.rs`: Das Entgelt wird unabhängig vom Mining
ausgezahlt, **ohne Nachweis wird nichts abgebucht**, und
`Netzwerkwissen` ist als Gegenstandsart angehängt.

### v0.1.0 – 2026-08-29 (was ein Gegenstand ist)

Die Kiste entsteht, zuerst als **Format**: Teilebildung, Merkle-Wurzel,
Manifest und κ_v-Versionierung. Gehasht wird der Klartext, nicht das
Komprimat; dieselben Bytes über verschiedene Puffergrenzen ergeben
dieselbe Wurzel.
