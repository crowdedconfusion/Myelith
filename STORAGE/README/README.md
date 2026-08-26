# STORAGE — die Rolle Store und die Verfügbarkeitsschicht

> **Version:** Entwurf, kein Crate
> **Datum:** 2026-08-25
> **Status:** Phase 0, Design. Sechs Design-Entscheidungen offen.

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

Entwurf. Die sechs Design-Entscheidungen aus dem Abschnitt oben sind
offen, und ohne sie gibt es kein Crate: Wer Speicherung baut, bevor
Redundanzform, Rotationsperiode und Nachweisverfahren feststehen, baut
sie zweimal.
