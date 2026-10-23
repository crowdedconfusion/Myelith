# STORAGE — die Rolle Store und die Verfügbarkeitsschicht

> **Version:** 0.7.1
> **Datum:** 2026-10-22
> **Status:** Gegenstandsformat, **Verfügbarkeitsnachweis** und
> **Speicherentgelt** stehen (`myl-store` v0.6.0, 29 Tests; Format und Stichproben-Ableitung liegen in `myl-types`). Sechs der
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

### v0.7.1 (2026-10-22): Eine Werkzeugkette für alle Kisten

- `rust-version` auf 1.99 in `myl-store`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  Nur das Manifest, kein Code.
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen.

### v0.7.0 (2026-10-22): Teilablage eines Artefakts und die Netzlast

Stufe 4 des Dispatchers (P10, P11, P8).

- `teilablage` (neu): `manifestdateien`, `dateien_des_bereichs` (die Ebenen
  samt Nebendateien, die Einbettung am ersten, Kopf und Norm am letzten
  Bereich), `passt` und `name_ist_flach`. Ein Knoten, der nur die Dateien
  seines Bereichs hält, jede gegen das Manifest geprüft, hat denselben
  Abdruck.
- `Netzlast` und `NETZLAST_DATEI`: das Format, das der Knoten schreibt und
  der Klient liest, an einer Stelle.
- `serde_json` als Abhängigkeit.
- **Belege:** Proben zu Bereich und Dateien, Summe und Name, Netzlast hin
  und zurück; `cargo test` rc=0.

### v0.6.1 (2026-10-17): Die Proben der Speicherorte laufen unter Windows

- 📌 **Zwei Proben fielen unter Windows**, gefunden unter Wine mit den
  Testprogrammen für `x86_64-pc-windows-gnu`: Sie legten Orte mit
  Unix-Pfaden an (`/Volumes/Platte`, `/x`), und unter Windows ist ein Pfad
  ohne Laufwerk nicht absolut. Die Prüfung im Code war richtig und wies sie
  ab; falsch waren die Proben. Sie nehmen jetzt einen absoluten Pfad dieses
  Systems (`abs`, in TOML in einfachen Anführungszeichen).
- **Neue Probe nur unter Windows:** `ein_laufwerkspfad_kommt_heil_zurueck`.
  Ein Pfad wie `D:\Users\Netz\Myelith` kommt durch Schreiben und Lesen
  heil zurück; geschrieben wird mit `toml` und nicht mit `format!`, sonst
  wäre `\U` ein Fluchtzeichen.
- Belegt: 29 Proben unter macOS, 30 unter Windows (Wine) grün.

### v0.6.0 (2026-10-17): Speicherorte und Kapazitätszusage (`orte`)

Auftrag des Projektinhabers: Gewichte auf eine angeschlossene Platte
auslagern und einen physischen Ort samt Volumen ans Netz vergeben, dazu
eine Datei, in der die Speicherorte stehen, damit das System später weiß,
wo es auf dem Knoten nach Gewichten und nach entgeltlich gespeicherten
Gegenständen sucht.

- **`speicherorte.toml`** (neu, `orte::Speicherorte`): je Ort Name, Pfad
  (absolut), Obergrenze `volumen_gib` und `zwecke` (`gewichte`, `netz`).
  Unter dem Pfad liegen `gewichte/` (ein Artefakt je Unterordner) und
  `netz/` (ein Gegenstand je Unterordner, erkannt an `manifest.borsh`).
  ⚑ **Die Obergrenze gilt für den ganzen Ort**, beide Zwecke zusammen.
  Eine fehlende Datei heißt keine Orte, eine kaputte ist ein Fehler.
  Geschrieben wird erst daneben, dann an den Platz; jede geschriebene
  Datei trägt einen Kopf, der das Format erklärt, denn sie darf von Hand
  bearbeitet werden.
- **`belegung(ort)`**: zählt auf der Platte, was am Ort liegt:
  Gegenstände, davon Shard-Gewichte, Ordner ohne lesbares Manifest (sie
  zählen nicht, werden aber genannt), Bytes über den ganzen Ort,
  Verknüpfungen nicht verfolgt.
- **`Zusage`** und **`botschaft`**: was ein Knoten dem Netz zusagt (Orte
  mit Obergrenze, Kerne, Rechenwerke, `probelauf`), als TOML; die
  Unterschrift geht über Trenner `MYELITH-KAPAZITAETSZUSAGE-V1`, SHA-256
  der **Dateibytes** und die Epoche. ⚑ `probelauf` steht im Text und damit
  unter der Unterschrift: Eine Zusage aus einem Probelauf lässt sich
  nicht als eine fürs Netz ausgeben. Eine fremde Fassung wird abgelehnt.
- Neue Abhängigkeiten `serde` und `toml` 0.8 (dieselbe Fassung wie
  `myl-local-agent`, liegt im Vorrat).
- **Belegt:** 29 Tests grün (7 neu), darunter die Belegung an echten
  Manifesten und die Abhängigkeit der Botschaft von Datei und Epoche.

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
