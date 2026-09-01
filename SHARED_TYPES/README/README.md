# shared-types (`myl-types`)

> **Version:** 0.28.0
> **Datum:** 2026-08-31
> **Status:** 🎉 **Phase 2 abgeschlossen** (Punkte 1.1–1.7, 2.1–2.3):
> Hash, Merkle-Baum, VRF (bit-exakt gegen RFC-9381-Vektoren), BLS12-381
> mit Aggregation **und Proof-of-Possession**, **Erasure-Codierung über
> GF(2⁸)**, ID-Newtypes, Kern-Structs
> aus Anhang A.1, Golden Vectors (18 Vektoren), Fuzz-Harness
> (100.000 Iterationen), Konformitätspaket.
> **226 Tests grün** über sieben Testbinaries. (Die Zeile stand heute
> früh auf 133 und zählte damit nur die Lib; 177 war der Stand vor den
> Zugängen dieses Tages.)

Protokollweite Kern-Datentypen, Hash-/Merkle-Primitiven und Serialisierung
für Myelith. Referenzimplementierung von Whitepaper Anhang A.1.

## Aufgabe

Ein einziges Crate, von dem alle anderen Komponenten (NETWORKING, CONSENSUS,
VERIFICATION, TOKENOMICS, COMPUTE_PIPELINE, AGENT_LAYER, TRAINING) dieselben
Basistypen beziehen, damit `Segment`, `PoIBundle`, Hashes und Signaturen
niemals in zwei Komponenten inkompatibel definiert werden.

## Abhängigkeiten

Keine — SHARED_TYPES ist die Basiskomponente des Protokolls.

## Struktur

```
SHARED_TYPES/
├── README/                   diese Kurzübersicht
└── myl-types/                das Protokoll-Crate (Bibliothek, kein Binary)
    └── src/
        ├── lib.rs             Crate-Wurzel: #![deny(unsafe_code)], Design-Doku
        ├── protocol.rs        Protokoll-Konstanten (Hash/VRF/Signatur/Serialisierung)
        ├── hash.rs            Hash-Newtype: SHA-256, Konstantzeit-Vergleich,
        │                      Borsh, Hex-Darstellung
        ├── merkle.rs          Merkle-Baum: Aufbau, Beweis-Erzeugung/-Prüfung,
        │                      Domain-Separation, Borsh-Beweise
        ├── vrf.rs             VRF: ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381),
        │                      Kanonizitätsprüfung, RFC-Testvektoren
        ├── bls.rs             BLS12-381 (min-pk, blst): KeyGen, Signatur,
        │                      Aggregation, FastAggregateVerify/AggregateVerify
        ├── ids.rs             ID-Newtypes: Address, MinerId, PodId, SegmentId,
        │                      MerkleRoot, ActivationHash, EpochId
        └── core_types.rs      Kern-Structs aus Anhang A.1: Segment, PoIBundle,
                               InferenceCredit (+ segments_root-Helfer)
```

## Changelog

### v0.28.0 – 2026-09-01 (die Anfrage wird gebunden, Punkte 39 und 47)

`Anfragebindung`. ⚑ **Der Prompt eines Nutzers kam im Konsens nicht
vor**: `Sitzungskontrakt` regelt Agentenbefugnisse, nicht Inferenz, und
ein PoI-Bündel bindet `(Id, Spurwurzel)` und sonst nichts.

Zwei Dinge gehen dadurch nicht. **Stufe 2 kann nicht nachrechnen**: Die
Entscheidung zu Punkt 47 lautet „Sitzungen ziehen", und ein Checker
braucht dafür den Prompt. Ohne Bindung müsste er ihn dem Pod glauben,
und dann prüft er, ob der Pod zu seiner **eigenen** Eingabe passt: eine
Frage, auf die der Gefragte beide Hälften wählt. Und **ein Nutzer kann
nicht belegen, was er gefragt hat**; ein Beleg ohne die Frage belegt nur
eine Antwort.

⚑ **Gebunden wird der Hash, nicht der Text.** Die Anfrage gehört nicht in
den Zustand: beliebig lang, und `commitment()` serialisiert ihn ganz
(D7). Wer nachrechnen will, holt den Text bei einem Beteiligten und
prüft ihn gegen den Hash. **Dieselbe Bauart wie beim Merkle-Beweis der
Spur: Die Kette trägt die Zusicherung, der Beteiligte den Inhalt, und
wer liefert, kann nicht wählen.**

⚑ **Die Sitzungsnummer geht in den Hash ein**, sonst hätte dieselbe
Anfrage in zwei Sitzungen denselben Wert und eine Bindung ließe sich
übertragen. Die Epoche steht daneben, damit eine Sitzungsnummer nicht
zeitlos wiederverwendbar ist.

### v0.27.1 – 2026-09-01 (⚑ Fund 118 an `Spurantwort::eingabe` vermerkt)

**Dieses Feld kann heute niemand füllen.** Die Entscheidung E10
(2026-08-30) hat das Archivieren der Aktivierungen abgeschafft; sie
kostete über die Streitfrist zwischen 65 GiB und 1,8 TiB je Knoten.

⚑ **Die Begründung von E10 trägt für die Bisektion und nicht für die
Stichprobe.** Dort legt der Ankläger die Eingabe offen, „denn er hat das
Segment gerade nachgerechnet"; ein Checker der Stufe 2 hat **noch nichts
gerechnet**.

Drei Wege stehen am Feld, samt Preis. **Das ist eine Entscheidung und
keine Verdrahtung**; das Bindungsgerüst darum herum ist vollständig,
geprüft und unabhängig davon richtig.

### v0.27.0 – 2026-09-01 (der Spur-Eintrag zieht dorthin, wo ihn beide lesen)

`activation_hash` liegt jetzt in `myl_types::uebergang`.

⚑ **Der Präzedenzfall stand seit dem 2026-08-29 in derselben Datei:**
`TransitionSig` zog aus `myl_pod::trace` hierher, „damit die
Schiedsstelle ihn lesen kann, ohne an dieses Crate und damit an die
ganze Inferenz-Laufzeit zu hängen". **Für `activation_hash` galt
derselbe Grund, und niemand hat ihn nachgezogen.**

Aufgefallen ist es, als der **Checker** eine Spur nachrechnen sollte: Er
hätte den Hash entweder aus `myl-pod` holen müssen, also der Prüfer vom
Geprüften, oder ihn ein zweites Mal schreiben. ⚑ **Beides ist falsch, und
das zweite ist Fund 111.**

**Ein Spur-Eintrag ist ein Konsensdatum.** Wer ihn anders rechnet,
bekommt eine andere Spur, und ein Streit darüber wäre nicht
entscheidbar, sondern nur zwei Meinungen.

### v0.26.1 – 2026-09-01 (⚑ Fund 117: der Name sagte etwas, das nicht hineinpasst)

`PeerIdBytes` trug seit je den Kommentar „PeerId als 32-Byte-Array … Die
Konvertierung erfolgt in NETWORKING". **Die Konvertierung gab es
nicht**, und sie hätte so nicht gehen können: Eine `PeerId` ist ein
Multihash und misst für Ed25519 **38 Bytes**.

⚑ **Aufgefallen ist es erst, als jemand das Feld benutzen wollte.**
Vorher trug der Typ nur Latenzatteste, in denen niemand zurückrechnete:
**Ein Feld, das keiner liest, kann jede Bedeutung tragen.**

Die 32 Bytes sind der **öffentliche Schlüssel**, und die `PeerId` folgt
daraus, denn sie ist sein Hash. Der Schlüssel trägt also mehr und nicht
weniger. Nur der Doc-Kommentar ändert sich; der Name bleibt, weil er im
Konsensvertrag steht.

### v0.26.0 – 2026-09-01 (⚑ Fund 116: die Netzadresse kommt in die Registrierung, Punkt 46)

Am 2026-08-26 wurde der **BLS-Konsensschlüssel von der Netzidentität
getrennt**, mit Besitznachweis. Richtig, und die Folge blieb liegen:
`MinerId` ist der Hash des Konsensschlüssels, die `PeerId` kommt aus dem
Netzschlüssel, und **nichts band die beiden**. Wer nur die Kette kannte,
konnte keinen Miner **erreichen**, und das hielt Stufe 2 auf.

`MinerRegistration.netzadresse`, additiv angehängt.

⚑ **Eine Angabe, kein Besitznachweis, und das ist eine Entscheidung.**
Dieselbe wie bei der Zone: **Eine falsche Adresse bestraft den, der sie
angibt.** Wer nicht erreichbar ist, kann keine Spur liefern, und
Schweigen zählt wie eine falsche Antwort.

⛑ **Was das nicht deckt:** Wer viele Miner auf die Adresse eines
**Dritten** anmeldet, schickt ihm fremden Verkehr. Heute frei, weil eine
Anmeldung nichts kostet; **sobald Stake daran hängt, ist es bepreist**,
und erst dann wäre ein Besitznachweis die passende Antwort statt der
teureren auf ein billiges Problem.

### v0.25.0 – 2026-09-01 (die Frage nach einer Spur, und ihre Antwort)

`Spuranfrage` und `Spurantwort` (Punkt 45, Stufe 2). Die Kette hält nur
eine Wurzel; wer ein gezogenes Segment nachrechnen will, braucht seine
Eingabe und die behauptete Spur, und beide liegen beim Koordinator.

⚑ **Der Merkle-Beweis gehört in die Antwort, sonst reicht der
Koordinator ein anderes Segment heraus**, nämlich eines, das er richtig
gerechnet hat. Die Ziehung wäre dann eine Frage, auf die der Gefragte
die Antwort wählt.

### v0.24.0 – 2026-09-01 (⚑ Fund 115: die Kette konnte nicht zählen, was sie bezahlt)

`PoIBundle` trug bis heute nur die **Wurzel** über seine
Segmentzeugnisse. **Eine Wurzel sagt nichts über die Zahl ihrer
Blätter**, und damit war aus dem Kettenzustand nicht ableitbar, wie
viele Segmente eine Epoche hatte.

⚑ **Das war die stille Vorbedingung, an der Stufe 2 scheiterte.**
`sample_segments` zieht aus `num_segments`, und diese Zahl gab es
nirgends. Ohne sie ist keine Stichprobe herleitbar, und ohne Stichprobe
ist `p` aus Anhang B.1 null.

`segmente: u32` ist **additiv angehängt** und steht **in der signierten
Botschaft**, aus demselben Grund wie `vtfe_claimed` und mit anderem
Schaden: Wer sie nachträglich erhöht, **verdünnt die
Stichprobenwahrscheinlichkeit je Segment**, ohne das Aggregat ungültig zu
machen. Aus `p` würde `p/k`.

### v0.23.0 – 2026-09-01 (der Schlüssel im Register, und der Besitz ist damit bewiesen)

`MinerRegistration.schluessel`.

⚑ **Er steht dort, weil die Kennung ihn nicht hergibt.** `MinerId` ist
`SHA-256` über den Schlüssel, und aus einem Hash folgt kein Urbild. Ohne
ihn kann der Konsens **keine Aggregatsignatur eines Pods prüfen**, denn
er wüsste nicht, gegen welche Schlüssel. Genau daran hing Glied 2 von
Punkt 40.

⚑ **Und der Besitz ist damit bewiesen, ohne eigenen Nachweis.** Eine
Anmeldung kommt als **unterschriebene Transaktion**, und die Unterschrift
entsteht mit genau diesem Schlüssel: **Wer unterschreiben kann, besitzt
den geheimen Teil.** Das ist dasselbe, was ein `BlsProofOfPossession`
belegt, nur bereits erbracht.

**Das ist keine Feinheit.** Ohne Besitznachweis wäre ein
**Rogue-Key-Angriff** möglich: Jemand veröffentlicht einen Schlüssel, der
als Differenz fremder Schlüssel gebildet ist, und fälscht damit
Aggregate. **Wer so einen Schlüssel bildet, kann mit ihm nicht
unterschreiben** und kommt gar nicht erst ins Register.

### v0.22.0 – 2026-09-01 (die Arbeitsverteilung, und warum kein Modellprofil)

`arbeitsverteilung.rs`: je Shard-Position ein Gewicht, gebunden an einen
Pipeline-Stand. Neun Tests.

⚑ **Warum Gewichte und nicht das Modellprofil.** Die Zuschreibung folgt
aus den Multiplikations-Additionen des Zuschnitts; dafür bräuchte der
Konsens das Profil (zehn Felder) und den Zuschnitt je Position (vier je
Position). **Ein Profil im Zustand ist genauso eine Erklärung wie ein
Gewicht, nur mit zehnfacher Fläche**, und es zöge die Modellinnereien in
einen Konsenstyp: Eine neue Architektur änderte die **Form des
Zustands** und verlangte eine harte Gabelung. Mit Gewichten ändert sie
die Zahlen.

**Erklärt, aber nachrechenbar:** Die Gewichte sind eine Angabe der
Governance, nicht eines Teilnehmers, und der Pipeline-Stand bindet θ_v.
Wer beides hat, rechnet nach und widerspricht. Genau diesen Maßstab
setzt die vTFE-Regel selbst.

⚑ **Und die Aufteilung ist exakt.** Jeder Anteil wird abgerundet, der
Rest geht in Positionsreihenfolge je eine Einheit an Positionen mit
Gewicht über null. **Die Summe der Anteile ist stets der Betrag**; ein
Rest, der verschwindet, wäre Geld, das niemand bekommt.

**Keine Tokenzahl darin:** Ein Bündel nennt die vTFE seines Pods, das
Verhältnis genügt. **Ein Feld weniger im Drahtformat ist ein Feld
weniger, über das jemand lügen kann.**

### v0.21.0 – 2026-09-01 (⚑ Fund 109: das Bündel nannte einen Pod, den die Zuteilung nicht kannte)

`MinerRegistration.zone` (Entscheidung 3b) und `pod_kennung`.

⚑ **Fund 109.** `PoIBundle` trägt seit jeher ein Feld `pod: PodId`, die
Zuteilung nummeriert ihre Pods mit `pod_index`, **und zwischen beiden
gab es keine Verbindung**. Im ganzen Repositorium entstand eine `PodId`
allein über `PodId::new([b; 32])`, und zwar ausschließlich in Tests:
**keine einzige Ableitung**. Damit war der Weg vom Bündel zur Besetzung
unterbrochen, ohne dass es auffiel, denn beide Seiten waren für sich
vollständig und getestet. **Dieselbe Klasse wie Fund 83 und Fund 87.**

`pod_kennung(epoche, pod_index)` leitet sie ab statt sie zu vergeben:
**Eine vergebene Kennung bräuchte eine Stelle, die vergibt**, und die
wäre ein Eintrag im Zustand samt Reihenfolge und Streitfrage. Die Epoche
gehört hinein, weil Pod 3 der Epoche 7 und Pod 3 der Epoche 8
verschiedene Besetzungen haben; ohne sie ließe sich ein altes Bündel
unter neuer Besetzung abrechnen.

**Die Zone** steht in der Registrierung, weil die Pod-Bildung Nähe
braucht und ein gemessener Latenzgraph im Konsens einem Angreifer einen
Hebel auf die Pod-Bildung gäbe (Entscheidung 3b). ⚑ **`GeoRegion` hat
seither `Ord`, und zwar aus einem Konsensgrund:** Die Gruppen müssen in
kanonischer Reihenfolge entstehen, sonst kämen zwei Knoten zu
verschiedenen Pods, ohne dass etwas kaputt wäre.

### v0.20.0 – 2026-09-01 (wer minen darf, zieht dorthin, wo beide Seiten es brauchen)

`miner.rs` mit `HardwareClass` und `MinerRegistration`, aus
`myl-scheduler` hierher.

⚑ **Der Doc-Kommentar sagte es seit Monaten, und es stimmte nicht.**
`MinerRegistration` trug den Satz „wird bei der Miner-Registrierung
erstellt und **im Ledger gespeichert**". Der Ledger kannte sie nicht;
der Scheduler bekam seine Liste vom Aufrufer, **und wer sie liefert,
entscheidet über die Pod-Bildung.**

Seit die Kette ein Miner-Register führt, brauchen beide Seiten denselben
Typ: das Kontenbuch zum Speichern, der Scheduler zum Pods-Bilden. Ein
eigener Typ je Seite wären zwei Quellen für dieselbe Aussage. Derselbe
Grund, aus dem das Gegenstandsformat am 31. August hierher zog. Die
**Filterung** bleibt im Scheduler: Sie ist ein Algorithmus, kein Typ.

### v0.19.0 – 2026-08-31 (⚑ Fund 108: die Unterschrift belegt den Absender, nicht den Inhalt)

Keine Verhaltensänderung, eine berichtigte Zusage. `node_metadata.rs`
las sich, als belege es etwas über den Standort eines Knotens. **Region
und ASN erklärt jeder Knoten über sich selbst**, und
`validate_structure` prüft davon allein den Zeitstempel; der Name sagt
„Struktur", und genau so weit reicht die Aussage.

Damit ist jede Diversitätsprüfung auf diesen Feldern eine Prüfung gegen
die Angabe des Geprüften. **Drei Maschinen in einem Schrank, die drei
Regionen und drei AS eintragen, bestehen sie.** Ein Test hält das jetzt
fest, damit niemand den Mechanismus später für mehr hält, als er ist.

**Was er trotzdem leistet:** Er hält versehentliche Bündelung fern, also
den ehrlichen Betreiber, der nicht aufpasst. Gegen einen Angreifer
leistet er nichts.

⚑ **Woran eine belastbare Prüfung hängen müsste:** an einer
**gemessenen** Größe oder an einer, die **Geld kostet**. Der Latenzgraph
liegt bereits vor und wird für die Clusterbildung benutzt. Eine erklärte
Angabe kostet nichts, deshalb wählt ein Angreifer sie frei.

### v0.18.0 – 2026-08-31 (die Treasury, ein Konto ohne Schlüssel)

`treasury.rs`: eine feste Adresse, aus einem Trennstring abgeleitet,
**ohne bekanntes Schlüssel-Urbild**.

⚑ **Ein Treasury-Konto mit privatem Schlüssel wäre ein Honigtopf und
eine Machtposition.** G1 nennt eine Machtposition ausdrücklich als das,
was in einem anonymen offenen Netz nicht legitim besetzbar ist; für Geld
gilt das eine Stufe schärfer. Eine gewöhnliche Adresse ist `SHA-256`
über einen öffentlichen Schlüssel, wer für die Treasury unterschreiben
wollte, müsste einen Schlüssel finden, dessen Hash auf diesen festen
Wert fällt. **Damit ist „nur das Protokoll kann sie belasten" eine
Tatsache der Bauart und keine Zusage**, derselbe Unterschied, den G6 für
die Vertraulichkeit macht.

Das Muster ist nicht neu: Cosmos nennt es ModuleAccount, eine Adresse
ohne Schlüssel, die ausschließlich Modul-Logik bewegt.

**Auszahlung nur über einen angenommenen Governance-Beschluss**
(Festlegung des Projektinhabers). Was sie nicht leistet: Sie schützt
nicht davor, dass eine Mehrheit sich selbst auszahlt; das ist eine Frage
des Abstimmungsverfahrens und steht dort.

### v0.17.0 – 2026-08-31 (der Stichprobenlauf: niemand fragt)

`quittung.rs`, 8 Tests, zwei Gegenproben. Dazu zieht die Ableitung des
verlangten Teils aus `myl-store` hierher; `nachweis.rs` führt sie aus,
statt eine zweite Fassung zu halten.

⚑ **Niemand fragt, und das ist der ganze Entwurf.** Die Stichprobe folgt
aus Epochenseed, Gegenstand, Epoche und Halterkennung. Jeder kann sie
ausrechnen, also auch der Halter: **Er weiß ohne Anfrage, was er
schuldet.** Damit entfällt eine ganze Klasse von Fragen, wer fragt, was
gilt, wenn niemand fragt, und wie man einen unterbliebenen Anruf
beweist. Dieselbe Wendung wie bei E10, nur in die andere Richtung: Dort
bringt der Ankläger alles mit, hier der Halter.

**Vorgelegt wird eine Quittung**, rund 130 Byte statt eines Mebibytes:
der Hash über die Bytes des verlangten Teils, gebunden an Epoche,
Gegenstand, Teilnummer und Halter, unterschrieben in der Rolle Store.

⚑ **Eine Quittung beweist nichts, sie verpflichtet.** Hashen kann jeder
irgendetwas. Wer eine abgibt, lässt sich später auf die Bytes
festnageln; wer keine abgibt, ist schon ohne Ankläger auffällig.

⚑ **Und das ist der Kern: Die fehlende Quittung braucht keinen
Ankläger.** Die Zuteilung ist nachrechenbar, jeder sieht dieselbe Liste
der Schuldner, und wessen Quittung fehlt, steht objektiv fest. Kein
Zeuge, keine Behauptung, kein Beweis eines Negativs. Die Schuldnerliste
entsteht deshalb aus der **Zuteilung**, nicht aus den vorgelegten
Quittungen; die Gegenprobe zeigt, dass andernfalls genau die
unsichtbar werden, um die es geht.

**Schweigen und ein untauglicher Versuch bleiben getrennt.** Das eine
kann ein Ausfall sein, das andere ist eine Handlung; sie in einen Topf
zu werfen hieße, dem Abgestürzten dasselbe vorzuwerfen wie dem, der es
versucht hat.

**Was hier nicht entschieden wird:** ob eine abgegebene Quittung wahr
ist. Dafür müsste jemand das Mebibyte anfordern und nachrechnen, und das
ist der optimistische Teil, der wie die Verifikation der Inferenz
arbeitet.

### v0.16.0 – 2026-08-31 (die Zuteilung: wer welchen Gegenstand hält)

`zuteilung.rs`, 11 Tests, drei Gegenproben. Aus Register, Zusagen und
Epochenseed ergibt sich deterministisch, wer welchen Gegenstand hält.
Dazu `Redundanzform::halterzahl` und `anteil_je_halter`.

⚑ **`halterzahl` ist nicht `halter_je_abruf`.** Das eine ist die Zahl
der Halter, die es geben muss, das andere die Untergrenze für einen
vollständigen Abruf. Bei Erasure k=8/m=6 sind das 14 gegen 8; wer das
eine für das andere nimmt, teilt sechs Halter zu wenig zu und merkt es
erst, wenn sechs ausfallen. Ein Test bindet beide an den Platzfaktor,
damit die drei Größen nicht auseinanderlaufen.

⚑ **Sie wird gerechnet, nicht gespeichert.** Der Zustandshash entsteht
über eine Serialisierung des ganzen Zustands; eine Zuteilung über
Tausende Teile machte ihn je Epoche neu und groß. Aus demselben Grund
liegt der Code hier: **Wer eine Abrechnung prüft, muss die Zuteilung
nachrechnen können**, ohne an der Store-Rolle zu hängen. Wer sie nur
entgegennimmt, überlässt dem Einreicher die Wahl, wer bezahlt wird, und
das ist der Fehler aus Fund 96.

**Je Gegenstand ein eigener Seed**, wie beim Pod-Shuffle im Scheduler:
Mit dem blanken Epochenseed bekäme jeder Gegenstand dieselbe Reihenfolge
und dieselben Halter liefen zuerst voll. **Fehlender Platz wird
gemeldet**, nicht verschwiegen; `assign_redundant_pods` überging
fehlende Metadaten stillschweigend, und genau das soll hier nicht
passieren.

⛑ **Zwei Berichtigungen an der eigenen Arbeit.** Der erste Test zum
eigenen Seed prüfte die Ableitungsfunktion statt ihre Wirkung und blieb
grün, als der Aufruf versuchsweise durch den blanken Seed ersetzt wurde:
Er prüfte, dass das Werkzeug funktioniert, nicht dass es benutzt wird.
Und der Kommentar an der Sortierung nannte einen Grund, den sie nicht
hat: Zwei Rechner geben ohnehin dieselbe Liste aus. Sie steht als
Vorsorge, damit die Ausgabe kanonisch bleibt, wenn jemand das
Auswahlverfahren ändert, und jetzt hält ein Test sie fest.

**Was hier nicht geschieht:** keine Diversität über Geo-Zone und ASN,
keine Rotation, keine Nachbesetzung. Alle drei setzen die Zuteilung
voraus und kommen darauf.

### v0.15.0 – 2026-08-31 (das Gegenstandsformat zieht in die gemeinsame Kiste)

`gegenstand.rs` kommt aus `myl-store`: Teile, Manifest, Gegenstandsart,
Redundanzform, dazu **neu** `Ablage` und `Finanzierung`.

⚑ **Der Grund ist derselbe wie beim Übergangs-Signaturvertrag zwei Tage
zuvor.** Das Manifest wandert in den Konsenszustand, also muss der Ledger
es lesen können. `myl-ledger` an `myl-store` zu hängen hieße, die ganze
Store-Rolle an den Konsens zu hängen: Abruf, Auslieferung, Rotation und
später Netz-Ein- und -Ausgabe. **Ein gemeinsamer Vertrag gehört in die
gemeinsame Kiste**; die Trennlinie ist das Format hier, die Rolle dort.

`myl-store` führt die Namen weiter aus, damit ein Aufrufer nicht wissen
muss, in welcher Kiste ein Vertrag wohnt.

### v0.14.0 – 2026-08-31 (die Kapazitätszusage, und eine siebte Rolle)

`zusage.rs`, 9 Tests: Was ein Knoten zu halten anbietet, signiert und
epochengebunden. Dazu `Rolle::Store` als fünfte Marke, **angehängt und
nicht eingefügt**; die Nummern 1 bis 4 bleiben, wo sie sind, und ein
Test hält das fest.

⚑ **Sie gilt ab der nächsten Epoche, nie ab sofort.** `ab_epoche` muss
echt größer als die laufende Epoche sein. Damit ist „mitten im Auftrag
abschalten" durch Konstruktion ausgeschlossen: Wer abschalten will, sagt
für die nächste Epoche weniger zu. Nebenbei erledigt dieselbe Regel den
Wiedereinspielungsangriff, denn eine alte, höhere Zusage trägt ihre
Epoche im signierten Teil.

⚑ **Null ist die Abmeldung, kein Fehler.** Wer sie verböte, machte den
Beitritt zu einer Falle: Ein Knoten käme aus der Speicherpflicht nur
noch durch Verschwinden, und Verschwinden ist im Protokoll ein Ausfall
mit Folgen. Der geordnete Ausstieg muss ausdrücklich möglich sein.

**Unterschrift und Identität in einem Schritt**, nach dem Muster von
Fund 96: Der Schlüssel muss zum genannten Halter gehören. Nur die
Unterschrift zu prüfen hieße, jeden Beliebigen im Namen eines anderen
zusagen zu lassen. Die Rolle wird mitsigniert, eine Unterschrift aus
einer anderen Rolle trägt hier nicht. Drei Gegenproben gefahren, jede
fällt auf ihren eigenen Test.

⚑ **Nur Speicher, obwohl der Schalter vier Größen nennt.** CPU und GPU
sind über die verifizierte Arbeit bereits bezahlt, Arbeitsspeicher ist
Voraussetzung und keine eigene Größe. **Ein Feld, das niemand liest, ist
in diesem Repositorium ein benannter Fehler** (Fund 98). Sobald die
Zuteilung Rechenkapazität wirklich auswertet, gehören die Felder dazu;
vorher nicht.

**Eine Zusage ist eine Obergrenze, keine Zusicherung.** Bezahlt wird
davon nichts; bezahlt wird, was nachgewiesen ist.

### v0.13.0 – 2026-08-30 (⚑ Fund 100: das Bündel bezeugte nicht, was gerechnet wurde)

`segments_root` war eine Merkle-Wurzel über die bloßen Segment-Ids, und
eine `SegmentId` ist `(Sitzungsnummer, Position)` mit Nullen aufgefüllt.
Sie bindet **nichts**: weder die Spur noch Ein- oder Ausgabe.

**Damit beanspruchte ein PoI-Bündel Arbeit, ohne zu sagen, was gerechnet
wurde.** Ein Pod konnte Paare `(Sitzung, Position)` aufzählen und dafür
vergütet werden; die Spur lag nur örtlich beim Koordinator und war an
nichts gebunden.

Der ganze Streitpfad hing daran. Die Schiedsrunde will feststellen, ob
der Angeklagte **das** gerechnet hat, was er behauptet hat. Ohne
Zusicherung gibt es kein „behauptet", nur zwei einander widersprechende
Aussagen und keinen Grund, einer zu glauben.

Das Blatt ist jetzt `Id ‖ Spurwurzel`, und `spurwurzel` baut eine Wurzel
mit **einem Blatt je Spur-Eintrag**, damit sich ein einzelner beweisen
lässt: Die Schiedsrunde streitet über eine Layer, nicht über ein Segment.

Drei Tests, darunter der entscheidende: **Dieselben Ids mit anderer Spur
müssen eine andere Wurzel ergeben.** Vorher taten sie es nicht.

### v0.12.0 – 2026-08-29 (⚑ Fund 96, zweite Hälfte: eine Anfechtung bindet jetzt den, der sie stellt)

`Challenge` nannte `primary_miner` und `redundant_miner` als **Felder**,
und nichts band einen davon an denjenigen, der die Anfechtung
einreichte. Dieselbe Gestalt wie Fund 85 im Ledger und wie die
Slash-Entscheidung: Wer anzeigt, bestimmte, wen er anzeigt und in wessen
Namen.

**Das zählt, weil eine Anfechtung Kosten verursacht.** Der Angeklagte
muss antworten, und nach der beschlossenen Umstellung des
Beweisarchivs auf Nachrechnen heißt das, eine ganze Folge neu zu
rechnen. Ohne Bindung wäre das ein Hebel zum Schikanieren, den jeder
ohne Einsatz ziehen kann.

Die Anfechtung trägt jetzt eine BLS-Signatur über
`DST_CHALLENGE ‖ Rolle ‖ Borsh(Felder ohne Signatur)`, in der Rolle
`Checker` und in keiner anderen. `ist_vom_herausforderer` prüft
Unterschrift **und** Zuordnung in einem Schritt: Eine gültige
Unterschrift unter einer Anfechtung, die einen anderen nennt, belegt
nur, dass irgendjemand unterschrieben hat. Getrennte Prüfungen könnten
getrennt vergessen werden.

Sechs Tests, darunter: jede Feldänderung bricht die Unterschrift, eine
Unterschrift aus einer anderen Rolle gilt nicht, und die Botschaft
beginnt mit dem Präfix im Klartext, an den Bytes geprüft und nicht an
einer Länge.

**Das Format ändert sich damit**, von 176 auf 272 Bytes. Der billigste
Zeitpunkt dafür ist der, an dem noch niemand darauf zeigt; nach dem
ersten Partnerlauf wäre es eine Protokolländerung mit
Abstimmungsbedarf. Dieselbe Begründung wie bei der Latenz-EMA (Fund 44)
und beim Übergangs-Präfix.

### v0.11.0 – 2026-08-29 (der Signaturvertrag zieht dorthin, wo geurteilt wird)

### ⚑ Ein Beleg nützt nur dem, der ihn lesen kann

`TransitionSig` lag in `myl-pod`: die Unterschrift, mit der ein Shard
jeden Rechenschritt bezeugt, den er ausführt. Gebraucht wird sie aber
nicht dort, wo sie entsteht, sondern dort, wo geurteilt wird, und
`myl-verifier` hängt nicht an `myl-pod` und soll es auch nicht, daran
hinge die ganze Inferenz-Laufzeit.

Die Folge war, dass die Unterschriften erzeugt, eingesammelt, aggregiert
und **von niemandem geprüft** wurden. Sie stehen jetzt als
`myl_types::uebergang` in der gemeinsamen Kiste, samt Präfix,
Rollenbyte und Feldreihenfolge; `myl-pod` reicht sie weiter durch, also
ändert sich für seine Nutzer nichts. Eine Nachbildung auf der
Richterseite wäre eine zweite Quelle für dieselbe Wahrheit gewesen, und
die beiden hätten sich beim ersten Formatwechsel getrennt.

### ⚑ `sha256(pubkey)` stand an sechs Stellen

`MinerId` und `Address` sind laut ihrer eigenen Beschreibung
`SHA-256(komprimierter BLS-Public-Key)`. Ausgeschrieben war diese Regel
am 29. August in sechs Dateien, jede für sich richtig.

Der Schaden einer solchen Verdopplung entsteht nicht beim Schreiben,
sondern beim Ändern: Wer die Regel an fünf Stellen nachzieht und die
sechste übersieht, bekommt zwei Kennungen für denselben Schlüssel, und
der Fehler zeigt sich als „unbekannter Aussteller" irgendwo weit weg von
seiner Ursache. Jetzt gibt es `Address::aus_schluessel` und
`MinerId::aus_schluessel`, und die fünf Abschriften im Produktivcode
sind fort.

**Die drei Abschriften in Tests bleiben stehen**, und zwar mit Absicht:
Ein Test, der die Ableitung über denselben Helfer rechnet, den er prüfen
soll, prüft sich selbst.

### v0.10.0 – 2026-08-29 (der Kontrakt begrenzt auch die Arbeit)

`Sitzungskontrakt` trägt eine **Höchstzahl der Schritte** (Kap. 8.4).
⚑ **Sie ist nicht dasselbe wie das Budget, obwohl beides begrenzt:** Das
Budget begrenzt, was ausgegeben wird, die Schrittzahl, wie lange
gearbeitet wird. Ein Agent, der in einer Schleife nachschlägt, ohne je
zu zahlen, verbraucht kein Budget und liefe endlos.

**Das Feld kommt jetzt und nicht später**, weil die Adresse eines
Kontrakts der Hash seiner Felder ist: Ein nachgereichtes Feld änderte
jede bestehende Kontraktadresse.

### v0.9.0 – 2026-08-28 (beide Währungen gelten)

`Waehrung::durchgesetzt` und `Befund::WaehrungNichtDurchgesetzt` sind
wieder verschwunden, und das ist die gute Nachricht: Sie standen da, weil
es im Ledger keine MYL-Überweisung gab und **eine Grenze, die niemand
durchsetzt, ablehnen muss statt durchzulassen**. Seit es die Überweisung
gibt, wäre die Variante unerreichbarer Code, und unerreichbarer Code, der
eine Zusicherung behauptet, ist genau das, was diese Wache verhindern
sollte.

Dazu das **Testprofil**: `opt-level = 2`, aber `debug-assertions` und
`overflow-checks` bleiben an, und `tests/profil.rs` belegt das zur
Laufzeit. ⚑ **`--release` wäre die naheliegende und falsche Antwort
gewesen**, weil es die Überlaufprüfung abschaltet.

### v0.8.0 – 2026-08-28 (der Session-Kontrakt, und warum er keine Sprache ist)

`sitzung.rs`: die vier Grenzen aus Whitepaper Kap. 8.2 als Typ.
Gesamtbudget, Einzeltransaktionslimit, Empfängerliste, Zeitfenster,
dazu die Betragsschwelle für bestätigte Auslieferung.

### ⚑ Warum eine Struktur und keine Sprache

Die naheliegende Frage lautet „eigenes DSL oder Erweiterung der
Transaktionstypen". Beide Antworten sind falsch, und zwar aus demselben
Grund: **Ein Kontrakt ist kein Programm, sondern ein Sprengradius.** Was
der Agent tun *soll*, steht im Prompt. Was im schlimmsten Fall geschehen
kann, steht im Kontrakt. Sobald man Bedingungen schreiben kann,
schreiben Leute Ziele hinein.

Dazu drei technische Gründe: Ein Parser im Konsenspfad ist eine
Angriffsfläche und ein Auswerter eine zweite; ein Auswerter braucht ein
Kostenmodell, das dieses Protokoll nicht hat; und ⚑ **eine Sprache, die
den Inhalt lesen kann, risse auf, was Kap. 8.3 strukturell schließt**,
weil „erlaube, wenn im Verwendungszweck 'Rechnung' steht" fremden Text
zum Steuerfluss macht.

**Neue Grenzenarten kommen deshalb über Governance**, so wie jeder
andere Protokollparameter, und nicht über eine Nutzereingabe.

### Der unveränderliche Teil und der veränderliche

Die Adresse ist der Hash der Kodierung, also ist der Kontrakt
unveränderlich. Der Verbrauch liegt als eigener Zustand darunter; stünde
er im Kontrakt, änderte sich die Adresse bei jeder Ausgabe.

⚑ **Die Empfängerliste steht in Normalform**, aufsteigend und
duplikatfrei, und der Konstruktor sortiert sie **nicht** still, sondern
lehnt ab. Ohne Normalform hätte dieselbe Menge je nach Reihenfolge zwei
Adressen, und das ist dieselbe Injektivitätsfrage wie beim Merkle-Baum.
Still zu sortieren wäre die schlechtere Antwort: Eine Liste, die die
Bibliothek verändert hat, ist nicht mehr die, die der Nutzer gesehen
hat.

### ⚑ Was diese Konstruktion nicht leistet, und das gehört hierher

Kap. 8.2 verlangt, dass die Grenzen für den Agenten **nicht lesbar und
nicht änderbar** sind. **Die beiden sind nicht gleich stark, und nur
eines ist eine Sicherheitseigenschaft.** Eine Ablehnung kostet nichts,
also lässt sich das Einzellimit in etwa zwanzig abgelehnten Versuchen
abtasten und die Empfängerliste durch Aufzählen. **Geheimhaltung der
Zahl ist damit nahezu wertlos; Unveränderlichkeit trägt alles.**

Gebaut ist trotzdem beides. `Befund::fuer_agenten` gibt genau ein Bit
heraus, denn der Fehlerkanal ist der Weg, über den die Zahlen sonst
ohnehin durchsickerten. Aber **der Akzeptanztest prüft
Unveränderlichkeit**: Ein Agent kann keinen Kontrakt ändern, er kann nur
einen anderen bauen, und ein anderer hat eine andere Adresse.

### Zwei kleinere Entscheidungen mit Begründung

**Zeit in Epochen, nicht in Sekunden.** Eine Wanduhr im Konsenspfad
existiert nicht; zwei Knoten mit verschiedenen Uhren kämen zu
verschiedenen Zuständen.

**MYL-Grenzen stehen schon im Typ**, obwohl das Ledger keine
MYL-Überweisung kennt, denn ein später ergänztes Feld änderte jede
Kontraktadresse. ⚑ **Eine Grenze, die niemand durchsetzt, lehnt ab; sie
lässt nicht durch.** Ein Feld ohne Durchsetzung liest sich sonst als
Zusicherung.

Dazu ein siebter Newtype, `SitzungId`: Eine Sitzungsadresse und ein
Eingabe-Commitment haben beide 32 Bytes, und eine Verwechslung im
Konsenspfad fiele nicht auf. **22 neue Tests**, zusammen 168.

### v0.7.0 – 2026-08-28 (der Schalter für den Verfahrenswechsel)

**Neu: `pq`.** Das Format für einen Wechsel des Signaturverfahrens, und
der Schalter dazu. **Nicht das Verfahren selbst**: Welches es einmal
wird, ist offen, und ein Verfahren einzubauen, das später gegen ein
anderes getauscht wird, wäre Arbeit gegen die eigene Annahme.

**Warum das jetzt kommt, obwohl der Wechsel nicht ansteht.** Ein
Schalter funktioniert nur, wenn alle Validatoren ihren neuen Schlüssel
**vorher** veröffentlicht haben. Solange der Validator-Satz kein Feld
dafür hat, kann niemand anfangen. Vor dem Genesis-Block ist das Feld
eine Zeile, danach eine Kettenmigration. **Dieselbe Klasse wie Fund 77:**
eine Lücke im Konsensformat, deren Behebung mit jedem Betriebstag teurer
wird, ohne dass jemand etwas falsch macht.

⚑ **Drei Stufen, nicht zwei, und die Folge ist einbahnig.** Ein Sprung
von „nur klassisch" auf „nur quantensicher" machte jeden Validator
ungültig, der seinen zweiten Schlüssel noch nicht veröffentlicht hat,
und hielte damit die Kette an. Ein Rückschritt öffnete das gebrochene
Verfahren wieder, und zwar genau dann, wenn jemand es gebrochen hat:
**Der Rückweg wäre der Angriff.** Erlaubt ist deshalb genau ein Schritt
nach vorn.

```text
NurKlassisch  →  Beide  →  NurQuantensicher
```

**Das Fenster `Beide` ist die verwundbarste Stellung**, und das ist
unvermeidlich. Es gehört so kurz wie möglich gehalten, und der Schritt
danach hängt **nicht an einer Frist, sondern an einer Bedingung**: dass
alle Validatoren bereit sind.

**Und der Grund, warum der Wechsel nicht ansteht, steht als Zahl im
Code:** `Signaturverfahren::signatur_len`. BLS12-381 aggregiert beliebig
viele Signaturen auf 96 Byte. ML-DSA-65 aggregiert nicht: 21 Validatoren
sind 69 489 Byte, in jedem Rundenwechsel. Ein Test hält beide Zahlen
fest, damit ein späterer Zusatz eines aggregierbaren Verfahrens auffällt.

**Acht Tests**, darunter die volle Übergangstabelle: Von neun Paaren
sind genau zwei erlaubt. Ein Test über drei Beispiele ließe offen, ob
der Rückweg wirklich zu ist.

### v0.6.0 – 2026-08-28 (⚑ Fund 77: Die Merkle-Wurzel bestimmt jetzt die Blattfolge)

⚑ **Änderung am Konsensvertrag: jede Merkle-Wurzel ist eine andere.**
Der Baum füllte eine Ebene mit ungerader Knotenzahl auf, indem er den
letzten Knoten mit sich selbst paarte, im Bitcoin-Stil, und erbte damit
den Fehler des Vorbilds (CVE-2012-2459): Die Abbildung von Blattfolgen
auf Wurzeln war **nicht injektiv**.

Gemessen, und die Kollisionsfamilie ist größer als der bekannte
Einzelfall:

| Blattzahl | kollidierte mit |
|---|---|
| 3, 5, 7, 9, … (ungerade ab 3) | derselben Folge plus wiederholtem letzten Blatt |
| 6, 14, 22, … (`n ≡ 2 mod 4`, ab 6) | derselben Folge plus den letzten **zwei** Blättern |
| 1, 2, 4, 8, … | nichts davon |

**Die Wurzel bindet jetzt die Blattzahl:**
`SHA-256(0x02 || u64_le(n) || innere Wurzel)`. Der innere Aufbau samt
Duplikationsregel ist unverändert geblieben.

**Der Beweis in einem Satz:** Aus gleicher Wurzel folgt gleiches Urbild,
also gleiche Blattzahl und gleiche innere Wurzel; bei fester Blattzahl
liegt die Baumform fest, und über die Domain-Separation bestimmt jeder
Knoten seine beiden Kinder eindeutig.

**Warum diese Behebung und nicht eine andere.** Drei Wege standen offen:
die Blattzahl binden, beim Auffüllen einen Fremdwert paaren, oder die
ungerade Ebene unverändert hochziehen. Der dritte wirkt am sparsamsten,
weil er kein zusätzliches Feld braucht, ⚑ **hat den Bedarf aber nur
verdeckt**: Beweise bekommen dort je nach Index verschiedene Längen, und
ein Prüfer kann die Ebenen nicht ablaufen, ohne die Blattzahl zu kennen.
Der gewählte Weg ist der einzige, dessen Argument in einen Satz passt,
und er lässt den inneren Aufbau unberührt.

**`MerkleProof` führt `leaf_count` mit.** ⚑ Der Wert muss aus keiner
vertrauenswürdigen Quelle stammen: Er geht in die Wurzelberechnung ein,
eine falsche Zahl ergibt eine andere Wurzel, und der Vergleich
scheitert. Ein Angreifer kann darüber nicht lügen, sondern nur
scheitern. Belegt in `eine_gefaelschte_blattzahl_im_beweis_scheitert`
über sechs erfundene Blattzahlen.

**Für die Verwender ändert sich kein Code.** Die Blattzahl steckt in der
Wurzel, nicht in einer zusätzlichen Angabe der Aufrufer. `PoIBundle`
braucht deshalb kein Feld für die Segmentzahl, obwohl genau dessen
Fehlen den Fund scharf gemacht hätte: Eine Aggregatsignatur über
`segments_root` bindet die Zahl seither mit. Kein einziges der sechzehn
Crates musste angepasst werden.

**Was sich ändert, sind Werte:**

- **Alle vier eingefrorenen Prüfvektoren** (`conformance/vectors/merkle.json`)
  sind neu erzeugt, und **ein fünfter ist dazugekommen**:
  `four_leaves_last_repeated`. Er ist der einzige, den eine Umsetzung im
  Bitcoin-Stil nicht trifft, denn ohne die Bindung trüge er dieselbe
  Wurzel wie `three_leaves`. Wer nach diesen Vektoren implementiert und
  die Blattzahl vergisst, fällt hier durch und nur hier. Die Prüfung
  vergleicht zusätzlich beide Wurzeln miteinander, denn die Aussage
  steht **zwischen** zwei Vektoren und in keinem einzelnen.
- **Der Gesamtwert des Protokoll-Durchlaufs im Testclient** ändert sich
  von `8c74519a11dceae5` auf `d02dcacb6aa37026`. Gemessen, nicht
  angenommen: Mit der alten Konstruktion liefert derselbe Lauf weiterhin
  `8c74519a11dceae5`, und geändert hat sich allein die Krypto-Stufe
  (`d2347febaedfebe9` auf `504713ed640fe164`).

**Warum jetzt und nicht später.** Die Behebung verschiebt jede
bestehende Wurzel. Heute entsteht jede Wurzel im System zur Laufzeit
neu, es gibt keinen Genesis-Block und keine gespeicherte Kette;
betroffen waren fünf Prüfvektoren und ein Fingerabdruck. θ_v, Artefakte
und Modelle sind nicht betroffen, denn θ_v ist über `theta_v_hash`
gebunden und läuft durch keinen Merkle-Baum. Nach dem Genesis-Block wäre
dieselbe Änderung eine Kettenmigration gewesen.

⚑ **Was daran über den Fund hinaus lehrreich ist.** Die
Duplikationsregel **war getestet**, und die Domain-Separation ist sauber
und mit dem richtigen Argument begründet (`0x00` für Blätter, `0x01` für
Knoten, gegen Second-Preimage). Geprüft wurde, dass die Regel tut, was
sie soll. Nicht geprüft wurde, was daraus **folgt**. Der neue Test fährt
deshalb die Nachbarschaft jeder Blattzahl von 1 bis 12 ab, statt einen
Einzelfall zu behaupten, und daneben steht die Gegenprobe
`ohne_die_blattzahl_waeren_die_wurzeln_gleich`: Sie hält fest, dass die
inneren Wurzeln weiterhin zusammenfallen und allein das Präfix sie
trennt. Ohne sie hinge die Behebung an einer Behauptung, denn ein grüner
Injektivitätstest bewiese nicht, dass die Bindung die Ursache ist.

**Dieselbe Überlegung stand schon an anderer Stelle im Projekt:**
`myl-governance::modell::Modellmanifest::wurzel` setzt vor jedes Feld
ein Längenpräfix, damit `("ab", "c")` und `("a", "bc")` nicht dieselben
Bytes ergeben. Der Merkle-Baum ist älter als diese Einsicht und hat sie
jetzt nachgeholt.

### v0.5.0 – 2026-08-28 (⚑ Fund 74: `Hash` bekommt eine Ordnung)

`Hash` leitete `Clone, Copy, Eq` ab und hatte einen
Konstantzeit-Vergleich, aber **kein `Ord`**. Damit ließ sich eine
`BTreeSet<Hash>` anlegen und niemals füllen: Ein leeres `BTreeSet`
braucht keine Ordnung, ein `insert` schon.

**Aufgefallen ist es in GOVERNANCE**, drei Wochen nachdem der Typ
gebraucht wurde. Die Kernel-Whitelist aus Kap. 10.3 steht dort seit
Punkt 1.1 als `Wert::Hashmenge(BTreeSet<Hash>)`, mit dem Vorgabewert
„leere Menge, bis zum Genesis-Manifest". Der Parameter hatte Typ,
Vorgabewert und Dokumentation und war nicht befüllbar. **Der Kommentar
nannte sogar den Schritt, an dem es brechen würde**, und niemand hat
nachgesehen, ob es dann geht.

Die ID-Typen aus `ids.rs` leiten `Ord` seit jeher ab; dass ausgerechnet
`Hash` es nicht tat, war kein Entwurf, sondern eine Lücke.

**Warum die Ordnung nicht in Konstantzeit läuft, und warum das richtig
ist:** `PartialEq` vergleicht bewusst in Konstantzeit. Eine Ordnung kann
das nicht, denn sie bricht beim ersten unterschiedlichen Byte ab, und
genau daraus besteht ein Größenvergleich. Dieselbe Abwägung, die in
derselben Datei schon für `std::hash::Hash` getroffen ist: Sortieren
und Nachschlagen sind keine Geheimnisoperationen. Wer wissen will, ob
zwei Hashes gleich sind, nimmt `==`.

**Was zusammenpassen muss:** `cmp` gibt genau dann `Equal` zurück, wenn
`eq` wahr ist. Liefen die beiden auseinander, verhielte sich jede
`BTreeMap` mit Hash-Schlüssel undefiniert und fände Einträge nicht, die
sie enthält. Ein Test hält es fest, ein weiterer die Stabilität der
Reihenfolge über Läufe: Eine Menge, deren Reihenfolge wechselt, ergibt
verschiedene Wurzeln für denselben Inhalt.

Vier neue Tests, eine Gegenprobe (eine Ordnung, die immer `Equal`
meldet, macht alle drei rot).

### v0.4.0 – 2026-08-19 (Erasure-Codierung als Primitive)

Neues Modul `erasure.rs` für die Datenverfügbarkeits-Schicht
(CONSENSUS 4.3): Reed-Solomon-artige Codierung in **systematischer
Cauchy-Form** über GF(2⁸), Startparameter k=8/m=4.

**Warum hier und nicht in CONSENSUS:** Erasure-Codierung ist eine
Primitive wie Hash, Merkle, VRF und BLS. Eine zweite Kopie in einer
Komponente wäre genau der Fehler aus Fund A6 (der Fisher-Yates-Shuffle
lag in vier Fassungen vor, drei davon fehlerhaft).

**Cauchy statt Vandermonde — der Grund ist eine Falle.** Bei einer
Vandermonde-Matrix ist die Invertierbarkeit **jeder** k×k-Teilmatrix
nicht automatisch gegeben. Das Loch äußert sich nicht als Fehler,
sondern als Rekonstruktion, die für bestimmte Ausfallmuster
stillschweigend falsche Daten liefert — die schlechteste Art von Bug.
Bei `C[i][j] = 1/(x_i ⊕ y_j)` mit disjunkten Mengen `{x_i}`, `{y_j}` ist
jede quadratische Teilmatrix invertierbar.

**Geprüft, nicht angenommen:** `jede_k_aus_n_teilmenge_rekonstruiert`
fährt alle **495** Teilmengen von 8 aus 12 durch; eine zweite
Parametrierung (3 aus 5, alle 10 Teilmengen) prüft, dass die
Konstruktion nicht nur für die Standardwerte trägt.

**Beschädigte Eingaben ergeben Fehler, keine Rekonstruktion.** Doppelte
Indizes machten die Matrix singulär, uneinheitliche Längen lieferten
Müll — beides wird abgewiesen, statt still falsche Daten zu erzeugen.
Zu wenige Fragmente sind ein **definierter** Ausfall
(`NotEnoughFragments`), kein Bug.

**Ganzzahligkeit:** GF(2⁸) ist reine Bitarithmetik — kein Gleitkomma,
keine Ordnungsabhängigkeit, bitgleich auf jeder Hardware. Dieselbe
Eigenschaft, auf der die Inferenz beruht, hier für die
Datenverfügbarkeit.

17 Tests; Crate 95 → 112 Unit-Tests.

### v0.3.0 – 2026-08-19 (Fund 27: Rogue-Key-Schutz nachgerüstet)

**Eine Sicherheitszusage in diesem Crate war falsch.** Der Modulkopf von
`bls.rs` sagte zu: „Öffentliche Schlüssel werden vor jeder
Aggregat-Verifikation validiert (Identitäts- und Subgruppen-Prüfung) —
**schützt gegen Rogue-Key-Angriffe bei `FastAggregateVerify`**." Das
stimmt nicht. Die beiden Prüfungen wehren Kleine-Untergruppen-Angriffe
ab, nicht Rogue Keys.

**Nicht bezweifelt, sondern gebrochen.** Zu einem fremden `pk_opfer`
bildet der Angreifer mit eigenem Geheimnis `x` den Schlüssel
`pk_rogue = g₁^x · pk_opfer⁻¹`. Der Punkt liegt in der richtigen
Untergruppe, ist nicht die Identität und besteht damit `key_validate()`.
Weil `pk_opfer · pk_rogue = g₁^x` gilt, verifiziert eine Signatur, die
der Angreifer **allein** erzeugt hat, als Aggregat beider Schlüssel — das
Opfer hat nie unterschrieben.

**Nachgerüstet:** `BLS_POP_DST`, `BlsProofOfPossession`,
`BlsSecretKey::prove_possession()` und `BlsPublicKey::verify_possession()`
nach draft-irtf-cfrg-bls-signature §3.3. Der Nachweis signiert die
komprimierten Bytes des eigenen öffentlichen Schlüssels unter einem
**eigenen** Domain-Tag — ohne diese Trennung wäre eine gewöhnliche
Signatur über die eigenen Schlüsselbytes ein gültiger Nachweis und
umgekehrt. Wer einen Nachweis liefern kann, kennt den diskreten
Logarithmus seines Schlüssels; der Erzeuger eines Rogue Keys kann das
nicht (er wäre `x − sk_opfer`).

**Regression:** `tests/rogue_key.rs` hält beide Tatsachen als
ausführbaren Nachweis fest — dass der Rogue Key die Validierung besteht
und `FastAggregateVerify` täuscht, **und** dass der Besitznachweis ihn
ausschließt. Als Integrationstest, weil die Konstruktion
`blst`-Punktarithmetik und damit `unsafe` braucht, was dieses Crate per
`#![deny(unsafe_code)]` ausschließt.

**Aufrufer:** `ValidatorRegistry::register` und `PodMembership::new` in
`myl-consensus` verlangen den Nachweis jetzt (dort v0.7.0). Die
Prüfungen `validate()`/`decode_validated_pk` bleiben unverändert — sie
waren nie falsch, nur falsch beschrieben.

**Konsensrelevant** (Kap. 10.3): neues Domain-Tag, geänderte
Registrierungsbedingung. 89 → 95 Unit-Tests, dazu 5 Regressionstests.


### Audit-Block 5 – 2026-08-18 (Warnungsfreiheit, Tests, Float-Audit)

Repository-weiter Block; die Einzelheiten stehen im Changelog der
jeweiligen Komponente.

- **Fund A17 behoben:** 111 Compiler-Warnungen → **0** über alle elf
  Crates. Dabei kamen drei echte Lücken zum Vorschein, die sich hinter
  „harmlosen" Warnungen versteckten (siehe unten).
- **clippy sauber** über alle Crates; `RUSTFLAGS: -D warnings` und ein
  eigener `lint`-Job in der CI verankern den Zustand. Bewusste Ausnahmen
  stehen als `#![allow(...)]` **mit Begründung** im Modulkopf (die
  Kernel-Signaturen tragen den vollständigen Fixed-Point-Vertrag; die
  Matrix-Namen `W`, `W_gate` folgen Whitepaper-Anhang B).
- **Fund A18 behoben:** Das Gleitkomma-Audit prüfte nur INTEGER_LLM
  (20 Dateien). Es deckt jetzt auch den **Konsenspfad** ab (37 weitere
  Dateien aus myl-types, -ledger, -scheduler, -consensus, -tokenomics,
  -verifier). Beide Pfade: null Treffer.


### v0.2.5 – 2026-08-18 (Audit-Block 4: Challenge als Protokolltyp)
- Neues Modul `challenge.rs` mit `Challenge` (Anhang A.4) und
  `validate_structure()`.
- **Warum hier (Fund A8/A12):** Der Typ wird von drei Komponenten
  gebraucht, die einander nicht kennen dürfen — VERIFICATION erzeugt
  ihn, NETWORKING validiert ihn beim Gossip, CONSENSUS nimmt ihn in den
  Block auf. Läge er in einer davon, müsste die Schichtung verletzt
  werden (L0 Networking hinge an L1 Consensus). Vorher existierten
  **zwei** unabhängige `Challenge`-Definitionen mit verschiedenen
  Feldern; der Block konnte gar nicht aufnehmen, was der Verifier
  produziert.
- 90 → 94 Tests.

### v0.2.4 – 2026-08-18 (Audit-Block 3: geteilter Seed-RNG)
- Neues Modul `seed_rng.rs`: `SeedRng` (SHA-256 im Zählermodus),
  `deterministic_shuffle` und `weighted_sample_without_replacement`.
- **Warum hier:** Beide Verwendungen sind Konsens-Feld — der
  Epochen-Scheduler (Shard-Zuweisung, Redundanz, Stichprobenlotterie,
  Geo-Clustering) und die Komiteewahl im Konsens. Vorher lag der
  Shuffle in vier Kopien in `myl-scheduler`; mit `myl-consensus` wäre
  eine fünfte dazugekommen. Protokollweite Primitive gehören in
  `myl-types`, damit es genau eine Fassung gibt.
- `weighted_sample_without_replacement` ist die Grundlage der
  VRF-rotierenden, stimmgewichteten Komiteewahl (Whitepaper Kap. 3.5:
  „gewählt nach Stake, rotierend per VRF").
- 74 → 90 Tests.

### v0.1.7 – 2026-08-13
- ID-Newtypes leiten zusätzlich `PartialOrd`/`Ord` ab — benötigt für
  `BTreeMap`-Schlüssel (u. a. das Kontenregister in `myl-ledger`,
  dessen deterministische Ordnung Konsens-Eigenschaft ist). Rein
  additive Änderung, keine Serialisierungs-Änderung.

### v0.1.6 – 2026-08-13 (Punkt 1.5) — Phase 1 vollständig
- Kern-Structs aus Anhang A.1, Feldnamen und -reihenfolge exakt wie im
  Whitepaper (Borsh-Reihenfolge ist Konsens-Vertrag): `Segment`
  (id, input_commitment, model_version, pod_path, output_commitment,
  trace, signatures), `PoIBundle` (epoch, pod, segments_root,
  vtfe_claimed, aggregate_sig), `InferenceCredit` (owner, vtfe, expiry).
- `segments_root`-Helfer: Merkle-Wurzel über Segment-Ids (die
  `PoIBundle.segments_root`-Konstruktion).
- Akzeptanzkriterium erfüllt: `serialize(deserialize(x)) == x` für je
  10.000 pseudozufällige Instanzen (deterministischer Xorshift-PRNG,
  reproduzierbar) plus Golden-Byte-Test der Feldreihenfolge —
  54 Tests grün, keine Warnungen.

### v0.1.5 – 2026-08-13 (Punkt 1.6, vor 1.5 umgesetzt)
- ID-Newtypes: `Address`, `MinerId`, `PodId`, `SegmentId`, `MerkleRoot`,
  `ActivationHash` (alle 32 Bytes, Borsh, kanonische Hex-Darstellung)
  und `EpochId` (u64). Typ-Verwechslung ist ein Compile-Fehler.
- Adress-Konvention: `Address = SHA-256(komprimierter BLS-Public-Key)`
  (hash-basiert, quantensicher, unabhängig vom Signaturschema).

### v0.1.4 – 2026-08-13 (Punkt 1.4)
- BLS-Signaturschnittstelle: BLS12-381 in der min-pk-Variante
  (Public Key G1/48 B, Signatur G2/96 B, Ethereum-DST
  `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_`) über das `blst`-Crate
  (Supranational-Referenzimplementierung).
- KeyGen nach BLS-Draft §2.3 (HKDF, IKM ≥ 32 Bytes), deterministisches
  Signieren, `aggregate_signatures`, `fast_aggregate_verify` (der
  PoI-Bündel-Fall: gleiche Nachricht, viele Unterzeichner) und
  `aggregate_verify` (verschiedene Nachrichten).
- Konsens-Sicherheitsfestlegungen: Signatur-Gruppenprüfung bei jeder
  Verifikation, Public-Key-Validierung (Identität + Untergruppe) vor
  jeder Aggregat-Verifikation als Rogue-Key-Schutz.
- Geheimschlüssel-Typ bewusst ohne Debug/PartialEq/öffentliche
  Serialisierung — 44 Tests grün, keine Warnungen.

### v0.2.3 – 2026-08-17 (Phase 2.3: Konformitätspaket)
- `conformance/`-Verzeichnis mit 18 eingefrorenen Golden Vectors
  (4 Hash, 4 Merkle, 5 VRF, 5 BLS) und README für Drittimplementierungen.
- Validierungstest (`tests/validate_conformance.rs`) prüft alle Vektoren
  gegen die Referenz-Implementierung — 4 Tests grün.
- Phase 2 damit vollständig abgeschlossen.

### v0.2.2 – 2026-08-17 (Phase 2.2: Fuzz-Harness)
- Fuzz-Test (`tests/fuzz_deserialization.rs`) für alle Borsh-Deserialisierungspfade:
  100.000 Iterationen pro Typ (Hash, MerkleProof, VRF, BLS, IDs, Core-Types)
  mit zufälligen/adversarialen Eingaben — keine Panics, nur `Ok` oder `Err`.
- Deterministischer PRNG (SplitMix64) für reproduzierbare Tests.

### v0.2.1 – 2026-08-17 (Phase 2.1: Golden Vectors)
- Golden Vector Generator (`src/bin/generate_golden_vectors.rs`) erzeugt
  18 deterministische Testvektoren für Hash, Merkle, VRF und BLS.
- Vektoren dienen als Referenz für Drittimplementierungen in anderen Sprachen.

### v0.1.3 – 2026-08-12 (Punkt 1.3)
- VRF-Schnittstelle: ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381 §5.5) —
  `VrfSecretKey`/`VrfPublicKey`/`VrfProof`/`VrfOutput`, Try-and-Increment-
  Hash-to-Curve mit Cofactor-Bereinigung, deterministische Nonce
  (RFC-8032-Variante), validate_key gegen Kleinordnungs-Schlüssel.
- Gegen die **offiziellen RFC-Testvektoren** (Anhang B.3, Beispiele 16–18)
  geprüft: Beweis-Erzeugung und Verifikation bit-exakt.
- Konsens-Verschärfung: kanonische Punkt-Dekodierung (y < p,
  Vorzeichen-Bit maskiert) — curve25519-dalek allein akzeptiert nicht
  kanonische Kodierungen, die der RFC ablehnt.
- `VrfOutput.algorithm` trägt das Versionsfeld für den dokumentierten
  Post-Quantum-Migrationspfad (GOVERNANCE, Krypto-Agilität) —
  34 Tests grün, keine Warnungen.

### v0.1.2 – 2026-08-12 (Punkt 1.2)
- Merkle-Baum über SHA-256: Aufbau (Duplikationsregel für ungerade
  Ebenen, Ein-Blatt-Sonderfall), Beweis-Erzeugung und -Prüfung
  (`MerkleProof` mit Borsh-Serialisierung, explizite Index-Bindung).
- Konsens-Festlegungen dokumentiert: Domain-Separation
  (`0x00`-Blatt-Präfix, `0x01`-Knoten-Präfix, Second-Preimage-Schutz),
  leerer Baum ist ein Fehler, Ordnung der Blätter ist Teil des Vertrags.
- Akzeptanzkriterium erfüllt: JEDE Einzelbit-Verfälschung eines Blatts
  oder des serialisierten Beweises wird abgelehnt (exhaustive
  Bitflip-Tests) — 21 Tests grün, keine Warnungen.

### v0.1.1 – 2026-08-12 (Punkt 1.1)
- Crate-Grundgerüst `myl-types`: `#![deny(unsafe_code)`, keine
  Gleitkomma-Arithmetik (Konsens-Determinismus ist Verfassungsrang).
- `Hash`-Newtype über SHA-256: Konstantzeit-Gleichheit
  (`subtle::ConstantTimeEq`), Borsh-Serialisierung, Hex-Darstellung,
  NIST-Testvektoren (leere Eingabe, „abc"), Roundtrip-Tests — 9 Tests grün.
- Protokoll-Konstanten als maschinenlesbare Anker der fünf
  Design-Entscheidungen (inkl. VRF-/Signatur-Algorithms-Versionsfelder
  für den dokumentierten Post-Quantum-Migrationspfad).
