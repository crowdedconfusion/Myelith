# NODE — der Myelith-Knoten

> **Version:** 0.24.0
> **Datum:** 2026-09-01
> **Status:** Netzknoten lauffähig, Blockproduktion mit **Persistenz über
> Neustarts**, BFT-Runden über das Netz mit Rundenwechsel, und seit dem
> 1. September **schließt der Knoten die Epoche selbst ab**.
> **196 Tests grün.**
>
> ⚑ **Seit dem 27. August sind Blockhöhe und Epoche zwei Dinge.** Die
> Probekette schrieb ihre Höhe in das Epochenfeld des Blockkopfs; das
> trug, solange eine Epoche ein Block war. Jede Frist „je Epoche"
> bedeutete damit in Wahrheit „je Block".

## Aufgabe

Die Verdrahtung, die aus den Protokoll-Bibliotheken ein laufendes
Programm macht: Identität, Konfiguration, Netzanbindung über `myl-net`
(L0), Nutzlastprüfung gegen `myl-consensus` (L1) und ein
Betriebsprotokoll für die nachträgliche Fehlersuche.

## ⚑ Der Probelauf ist nicht das Testnetz

Steht hier oben, weil eine Verwechslung teuer wäre. Was der Knoten heute
fährt, ist eine **Trockenübung des Codes**: Der Zustand ist Wegwerfware,
jeder Start beginnt bei null, die MYL sind Spielgeld, und der Startwert
der Probekette lautet im Klartext `MYELITH-PROBELAUF-KEIN-TESTNETZ`.
**Ein Probeblock kann deshalb niemals an eine echte Kette anschließen**,
und ein Test hält das fest.

Wann das Testnetz beginnt, entscheidet das Projekt und nicht der
Umstand, dass dieser Code läuft.

## Warum es diese Komponente gibt

Bis zum 2026-08-24 hatte das Projekt dreizehn Komponenten, rund 1500
Tests und **kein Programm, das einen Myelith-Knoten startet**. `myl-net`
hatte im ganzen Repositorium **keinen einzigen Abnehmer**: Der Testclient
hängt an neun Crates, an der Netzschicht nicht.

Das ist keine Ordnungsfrage, sondern die Ursache einer ganzen Fundklasse.
**Fund 52** (der Vergütungspfad war unbenutzbar), **Fund 55** (der
dokumentierte Prüf-Einstieg war über die Laufzeit nicht erreichbar),
**Fund 56** (ein Relais ohne eigene Adresse antwortet ins Leere) und
**Fund 57** (die Formprüfung für Blöcke filtert kaum) haben eines
gemeinsam: Sie wurden sichtbar, als jemand die Teile zusammensteckte.
Eine Naht, die niemand belastet, hält alles aus.

## Was der Knoten heute ist, und was nicht

**Ist:** ein Netzknoten. Er findet Gegenstellen über Bootstrap und
Kademlia, verbreitet und empfängt die sechs Protokoll-Topics, misst
Latenzen, hält seine Verbindungsgrenzen ein, arbeitet hinter NAT über
Relais, und schreibt alles mit.

**Ist seit dem 2026-08-24 auch:** ein Blockproduzent. `src/kette.rs`
bringt Kettenzustand, Mempool und Rundentakt: Der Erzeuger baut aus dem
Mempool Blöcke, verkettet sie über den Vorgänger-Hash, wendet die
Transaktionen auf seinen `LedgerState` an und schreibt die entstandene
**Zustandswurzel** in den Block. Alle anderen rechnen nach und
vergleichen.

**Was das misst:** nicht „einigen sich die Knoten", sondern die Frage
davor: **Kommen zwei Maschinen aus denselben Blöcken zum selben
Zustand?** Das ist die Protokollhälfte derselben Frage, die der
Determinismus-Test für die Inferenz stellt. Weicht eine Wurzel ab, ist
irgendwo im Ledger-Pfad etwas nicht deterministisch, und das bricht den
Konsens genauso wie ein abweichendes Inferenzergebnis.

**Ist seit dem 2026-08-26 auch: BFT.** Die Knoten stimmen ab. Propose,
Vote und Commit laufen über ein eigenes Gossip-Topic
(`/myelith/consensus/1`), der Validator-Satz kommt aus einer
**Genesis-Datei**, und jeder Knoten signiert mit einem BLS-Schlüssel, der
von seiner Netzidentität getrennt ist.

**Gemessen, nicht behauptet:** Fünf eigenständige Betriebssystemprozesse
über libp2p/QUIC, alle fünf commiteten denselben Block, mit vollem
Stimmgewicht 900 000 000 von 900 000 000 gegen eine Schwelle von
600 000 001.

**Das Stimmgewicht ist ungleich, und das ist Absicht.** Die
Genesis-Verteilung des Probenetzes (250/230/200/120/100 MYL) ist so
gebaut, dass drei von fünf Köpfen das Quorum je nach Auswahl verfehlen
(420), es **exakt** treffen (600, reicht nicht) oder erreichen (680).
Bei gleichen Gewichten wären Kopfzählung und Gewichtszählung numerisch
dasselbe, und Fund A3 (der Zustandsautomat zählte Nachrichten statt
Gewicht) liefe grün durch.

**Ist seit dem 26. August auch: ausfallsicher gegen einen ausgefallenen
Leader.** Läuft die Frist ab, wechselt der Knoten die Runde, und der
nächste Leader schlägt vor. Wer nach einem Stimmen-Quorum gesperrt ist,
schlägt den gesperrten Block vor und bringt sein **Polka-Zertifikat**
mit, damit die anderen ihre Sperre gefahrlos lösen können.

**Gemessen an fünf echten Prozessen:** Der Leader von Runde 0 mit
`kill -9` beendet, die übrigen vier wechseln auf Runde 1 und commiten
denselben Block.

**Die Frist ist fest und wächst je Runde**, sie wird **nicht** aus
gemessenen Latenzen abgeleitet. Die Latenzwerte kommen aus Attesten, und
ein Timeout, der von dieser Fläche liest, gäbe einem Angreifer einen
Hebel auf die Liveness (Audit A10, A12).

⚑ **Wer allein vorauseilt, kommt seit dem 29. August zurück** (Fund 67).
Ein Knoten, dessen Frist ablief, bevor die anderen ihre Runde begonnen
hatten, stand danach dauerhaft vor dem Netz: Er verwarf jede Nachricht
aus einer fremden Runde, also gerade die, die belegten, dass er der
Irrende war. Die Safety hielt, seine eigene Liveness nicht.

Der Rückweg ist ein **Commit-Zertifikat**, der Beleg eines Quorums, und
er gilt ohne Rücksicht darauf, in welcher Runde der Empfänger steht.
Angefordert wird er nicht: Wer außer Takt ist, gibt sich durch seine
eigenen Nachrichten zu erkennen, und die Gegenseite antwortet darauf mit
dem Beleg, jedem genau einmal. Im Normalbetrieb kostet das nichts.

**In der Praxis weiterhin sinnvoll:** Knoten dicht beieinander starten
oder `--bft-frist` über den Startversatz legen. Der Ausgleich holt einen
Knoten zurück, er ersetzt nicht die Runde, die er verpasst hat.

**Ist nicht: Blockinhalt im Konsens.** Der Propose trägt einen
Block-*Hash*, nicht den Block. Was er bezeichnet, entscheidet die Kette,
und deren Persistenz ist ein eigener offener Punkt.

**Ist seit dem 26. August auch: neustartfähig.** Mit `--kette <datei>`
führt der Knoten ein anhängendes Blockprotokoll. Beim Start spielt er es
nach und **rechnet dabei jede Zustandswurzel neu**, durch dieselbe
`Kette::uebernimm`, durch die auch Gossip-Blöcke gehen.

**Gespeichert werden nur die Blöcke.** Höhe, letzter Hash und Zustand
folgen daraus. Ein abgeleiteter Wert, den man zusätzlich ablegt, ist eine
zweite Wahrheit, und sobald die beiden auseinanderlaufen, glaubt der
Knoten der falschen.

**Der Mempool überlebt bewusst nicht.** Wartende Transaktionen sind
unbestätigt und kommen über den Gossip wieder; sie aufzuheben hieße, nach
einem Tag Stillstand alte Transaktionen einzuspeisen, deren Absender
längst andere geschickt hat.

**Belegt an zwei laufenden Knoten:** Der Erzeuger wurde abgeräumt und aus
seiner Datei neu aufgebaut, während der zweite durchlief. Beide standen
danach bei derselben Zustandswurzel. **Der Vergleich gegen den
durchlaufenden Knoten ist der Kern**; ein Vergleich gegen die eigene
letzte Zustandsaufnahme wäre wertlos, weil sie Sekunden und Blöcke vor
dem Abbruch liegt.

⚑ **Ein Block kennt seine eigene Höhe nicht.** `Block` trägt `epoch`,
`prev_block_hash`, `timestamp_ms` und `state_root`, aber kein Höhenfeld.
Die Kette hängt allein am Vorgänger-Hash, jeder Knoten führt die Höhe
selbst, und wer einen Block verpasst, **kann die Lücke nicht benennen**:
Er merkt nur, dass der nächste nicht anschließt. Das muss man wissen,
bevor man Synchronisierung baut.

## Benutzung

```
myl-node --name alpha --port 4150
myl-node --name beta  --port 4151 --bootstrap /ip4/…/tcp/4150/p2p/12D3Koo…
myl-node --name relais --rolle relais --oeffentlich /ip4/203.0.113.5/tcp/4150
```

**Mitstimmen bei BFT-Runden:**

```
# Einmal je Betreiber: die eigene Zeile für die Genesis-Datei erzeugen
myl-node --name alpha --genesiszeile 250000000

# Die Zeilen einsammeln, Datei bauen, dann starten
myl-node --name alpha --port 4150 --genesis genesis.txt

# Über ein WAN mit ungleichen Startzeiten: die Frist über den
# Startversatz legen, damit niemand die erste Runde verpasst (Fund 67)
myl-node --name alpha --port 4150 --genesis genesis.txt --bft-frist 30000
```

Die Genesis-Datei nennt den Netznamen und je eine Zeile
`validator <pubkey-hex> <pop-hex> <stake>`. Die **Kennung wird aus dem
Schlüssel abgeleitet**, nicht aufgeschrieben: Zwei Quellen für dieselbe
Wahrheit widersprechen sich irgendwann. Der Hash der Datei liegt auf dem
**Inhalt**, nicht auf den Bytes, also ändert eine umsortierte Zeile
nichts.

⚑ **Mindestens vier Validatoren.** Die Datei lehnt jeden Satz ab, in dem
einer ein Drittel oder mehr hält, denn ein solches Netz hat keine
BFT-Safety. Daraus folgt eine Mindestzahl: Drei Werte unter je einem
Drittel ergeben nie ihre eigene Summe.

`--hilfe` nennt alle Angaben. Der Knoten horcht auf **TCP und QUIC**:
QUIC ist der Pfad, auf dem Lochstanzen zuverlässig ist, TCP der, der
auch durch Firewalls kommt, die UDP verwerfen.

**Die Schlüsseldatei bestimmt die Identität.** Bleibt sie erhalten,
behält der Knoten seine Peer-Id über Neustarts, und nur dann lassen sich
die Protokolle mehrerer Läufe zusammenführen.

**Wo sie liegt:** Über den Testclient in `TESTCLIENT/Schluessel/`, das
seinen Inhalt selbst von der Versionsverwaltung ausschließt. Auf der
Befehlszeile dort, wo `--schluessel` hinzeigt, sonst im aktuellen
Verzeichnis; `*.key` steht deshalb auch in der Wurzel-`.gitignore`. Die
Datei bekommt auf Unix die Rechte 0600. **Wer den Schlüssel hat, kann im
Netz als dieser Knoten auftreten**, das ist keine Ordnungsfrage.

## Das Betriebsprotokoll

Eine Zeile JSON je Zustandsänderung, sofort auf die Platte geschrieben.
Der interessanteste Zeitpunkt ist der letzte vor dem Absturz, und ein
gepuffertes Protokoll verliert genau die Zeilen, wegen derer man es
liest.

Jede Zeile trägt **`folge`** (lückenlos je Knoten), **`zeit_ms`**,
**`knoten`** und **`peer`**. Fehlt eine Folgenummer, ist das eine
Aussage: Entweder ist die Datei beschädigt, oder der Knoten ist
gestorben. Ohne sie ließe sich beides nicht von „es ist nichts passiert“
unterscheiden.

Die Arten im Protokoll: `start`, `horcht`, `horchadresse`, `bootstrap`,
`relais_reservierung`, `eigene_adresse`, `verbunden`, `getrennt`,
`abgewiesen`, `lochstanzen`, `erreichbarkeit`, `gesendet`, `empfangen`,
`verworfen`, **`block_erzeugt`**, **`block_uebernommen`**,
**`block_abgelehnt`**, `tx_aufgenommen`, `aufnahme`, `ende`. Die Zustandsaufnahme trägt
zusätzlich die Mesh-Größe je Topic und die **Spanne der Paarlatenzen**
im Fenster seit der vorigen Aufnahme.

**Latenzen werden gesammelt, nicht einzeln geschrieben.** Ein Ping je
Peer alle 15 Sekunden ergäbe über eine Stunde bei drei Peers 720 Zeilen,
die einzeln nichts sagen. Interessant ist die Spanne.

Ausgewertet wird mit dem Testclient:

```
myl-test netz --logs <verzeichnis>
```

Der Befehl beantwortet eine andere Frage als `vergleich`: nicht „rechnen
zwei Maschinen dasselbe“, sondern „haben mehrere Knoten einander
gesehen“. Er urteilt **bewusst nicht über die Uhr**, weil Zeitstempel
verschiedener Maschinen nicht verlässlich synchron sind. Verglichen
werden Aussagen über Verbindungen, und die trägt jede Seite selbst bei.

## Struktur

```
NODE/
├── README/                  diese Übersicht
└── myl-node/
    └── src/
        ├── lib.rs            Crate-Wurzel
        ├── konfig.rs         Konfiguration, prüft sich beim Start selbst
        ├── protokoll.rs      Betriebsprotokoll (JSONL)
        ├── validator.rs      Nutzlastprüfung Blöcke/Transaktionen/
        │                     Konsensnachrichten (L1)
        ├── validatorsatz.rs  wer darf attestieren: Kennung → Schlüssel,
        │                     Urteil mit Grund (Audit A10)
        ├── genesis.rs        Validator-Satz zu Genesis: Datei, Hash auf
        │                     dem Inhalt, Besitznachweis, Ein-Drittel-
        │                     Schranke (23 Tests)
        ├── schluessel.rs     BLS-Konsensschlüssel, getrennt von der
        │                     Netzidentität; Dateirechte 0600 (12 Tests)
        ├── konsens.rs        eine BFT-Runde aus Sicht eines Knotens:
        │                     wann er selbst etwas sagen muss, samt
        │                     Rundenwechsel und Zertifikat (22 Tests)
        ├── nachschub.rs      Blocknachforderung: Bereich, Deckelung,
        │                     Nachlieferung
        ├── speicher.rs       Blockprotokoll auf der Platte: anhängend,
        │                     abbruchfest, an die eigene Kette gebunden
        │                     (11 Tests)
        ├── knoten.rs         Start, Ereignisschleife, Zustandsaufnahme
        └── main.rs           Kommandozeile
    └── tests/
        ├── zwei_knoten.rs    zwei Knoten, echte Sockets, Protokoll
        │                     zurückgelesen (5 Tests)
        ├── bft_ueber_das_netz.rs
        │                     fünf Knoten, eine Runde, ein Block;
        │                     Vorlauf-Puffer, Rundenwechsel und
        │                     Protokollformat (6 Tests)
        └── neustart.rs       Wiederanlauf aus der Kettendatei, Abbruch
                              mitten im Schreiben, Vergleich gegen einen
                              durchlaufenden Knoten (6 Tests)
```

## Changelog

### v0.24.0 – 2026-09-01 (das Fragen, Punkt 45)

**Der Knoten verschickt die Stichprobe.** Die Lotterie zog seit heute
(Fund 114), die Adresse stand seit heute in der Kette (Fund 116), die
Prüfung einer Antwort war vollständig — ⚑ **ohne diesen Aufruf wäre alles
drei ohne Wirkung geblieben, und genau so ist Fund 114 entstanden.**

`anfragen_fuer` trifft die Entscheidung, der Knoten verschickt nur ihr
Ergebnis. ⚑ **Damit ist sie prüfbar, ohne ein Netz zu starten**, und der
Versandteil bleibt so dünn, dass an ihm nichts schiefgehen kann.

**Gefragt wird jedes Mitglied**, Reserve eingeschlossen. Und **einmal je
Epoche**: Zwischen zwei Wechseln liegen viele Blöcke, und dieselbe Frage
hundertmal zu stellen wäre eine Flut, die der Gefragte zu Recht als
Angriff läse.

⚑ **Pods ohne Adresse werden gezählt und protokolliert**, nicht
übergangen. Sonst wäre „ich nenne keine Adresse" die billigste Art, sich
der Prüfung zu entziehen.

**Was noch fehlt:** die Antwort zu verarbeiten. `pruefe_spurantwort`
steht in VERIFICATION; was fehlt, ist ein **Nachrechner mit Modell**, und
der hängt an Artefakten.

### v0.23.0 – 2026-09-01 (an wen ein Checker fragt, Punkt 46)

`adressen_des_pods` gibt die Netzadressen **aller** Mitglieder zurück,
Reserve eingeschlossen.

⚑ **An den Pod, nicht an den Koordinator.** Naheliegend wäre der
Koordinator, denn er sammelt die Spuren ein; **dann genügte sein
Schweigen, um die Prüfung zu vereiteln.** Alle Mitglieder haben das
Bündel unterschrieben, und der Merkle-Beweis bindet eine Antwort an die
unterschriebene Wurzel statt an den Antwortenden. **So muss ein ganzer
Pod schweigen statt einer.**

⚑ **Die Nulladresse ist keine.** Wer keine nennt, taucht nicht in der
Liste auf; bleibt sie leer, ist der Pod **nicht prüfbar**, und das ist
ein Befund. Sonst wäre „ich nenne keine Adresse" die billigste Art, sich
der Prüfung zu entziehen.

### v0.22.0 – 2026-09-01 (⚑ Fund 114: Stufe 2 wird gezogen, Punkt 45)

**`sample_segments` und `check_segment` hatten null Aufrufer.** Beide
waren seit dem 2026-08-17 gebaut, geprüft und abgehakt; außerhalb der
Tests rief sie nichts. **Damit lief Stufe 2 der Verifikation in keinem
Knoten**, und die gesamte Sicherheitsbedingung aus Anhang B.1 hängt an
`p`, der Wahrscheinlichkeit einer Nachrechnung. Ohne Ziehung ist `p = 0`.

Der Epochenabschluss zieht jetzt, aus den **bezeugten** Bündeln, also
denen mit gültiger Aggregatsignatur: Eine Flut ungültiger Bündel soll
die Rate der ehrlichen nicht verdünnen.

⚑ **Ein gemeinsamer Indexraum, kein Zug je Pod.** Zöge man je Pod, so
bekäme ein Pod mit drei Segmenten bei jeder Aufrundung eines und damit
33 Prozent statt 2. **`p` ist eine Wahrscheinlichkeit je Segment**, und
die ist nur in einem gemeinsamen Raum für alle dieselbe.

⛑ **Zwei eigene Fehler auf dem Weg, beide von Gegenproben gefunden.**
Die Ziehung stand zuerst in einem blanken `Vec` und wurde **von jedem
Block überschrieben**; sie gehört zu einer Epoche, nicht zu einem Block.
Und der Test „kleine Pods bekommen keine höhere Rate" prüfte `klein <= 1`
und ließ damit **genau den schlechten Fall zu**, denn der Zug je Pod
ergibt eins. Er misst jetzt den Anteil über zweihundert Saaten: 1 Prozent
statt 14.

**Was noch nicht geschieht:** das Nachrechnen selbst, denn es braucht die
Spur des Segments, und die liegt beim Koordinator. ⚑ Und die **Saat**
ist heute der Blockhash und damit mahlbar; sie steht deshalb als
**Argument** an der Aufrufstelle, nicht als Ableitung im Modul.

### v0.21.0 – 2026-09-01 (⚑ Punkt 40 ganz: die Unterschrift wird geprüft)

Der Knoten baut die Pod-Mitgliedschaft aus der **Zuteilung** und prüft
damit die Aggregatsignatur jedes Bündels. Ein Bündel, das sie nicht
besteht, fällt weg.

⚑ **Damit ist die letzte Lücke zu**: Bis hierher konnte ein angemeldeter
Miner ein Bündel für einen **echten fremden** Pod einreichen und dessen
Mitgliedern eine unverdiente Gutschrift verschaffen. Jetzt braucht es
die Unterschriften aller Mitglieder.

**Die Funktion steht im Knoten und nicht im Scheduler**, denn der bildet
Pods und soll von Bündeln nichts wissen; und nicht in `myl-consensus`,
der die Zuteilung nicht kennt. **Der Knoten ist die einzige Stelle, die
beides sieht.**

⛑ **Und der Test, an dem der Punkt hängt, fiel beim Einschalten sofort
um.** Er benutzte eine Attrappe als Signatur; die Prüfung tat also genau
das, was sie soll. Er unterschreibt jetzt mit allen Mitgliedern.

### v0.20.0 – 2026-09-01 (⚑ Punkt 40 geschlossen: bezeugte Arbeit erreicht ein Konto)

Der Knoten leitet am Epochenwechsel die Zuteilung ab, ordnet die Bündel
ihren Pods zu und schüttet aus. **Der Weg ist damit durchgehend:**
Register → Zuteilung → Bündel → Anteile → Konto.

⚑ **Jeder Schritt ist eine Ableitung, keine Angabe.** Wer im Register
steht, sagt die Kette; wer in welchem Pod sitzt, folgt aus Register,
Zone und Blockhash; welcher Pod ein Bündel eingereicht hat, folgt aus
der abgeleiteten Pod-Kennung. Nur die Gewichtung ist gesetzt, und zwar
von der Governance, nicht von einem Teilnehmer.

**Der Test, an dem der Punkt hängt:** Sechs Miner melden sich an, tragen
ein Auszahlungskonto ein, jemand verbrennt, ein Bündel kommt in die
Kette, und am Epochenwechsel wächst das Konto eines Pod-Mitglieds.
Höchstens vier Konten wachsen, denn ein Pod hat vier Positionen und zwei
in Reserve.

⚑ **Ein Bündel mit erfundener Pod-Kennung zahlt nichts aus**, und ein
eigener Test hält das fest. **Das ist die Schranke gegen ein
gefälschtes Bündel**, solange die Aggregatsignatur noch nicht geprüft
wird: Wer eine Kennung erfindet, trifft keine Platznummer dieser Epoche.

⚑ **Ohne Arbeitsverteilung bekommt niemand etwas.** Lieber nichts
ausschütten als nach einer Gewichtung, die niemand gesetzt hat.

**Die Verteilung wird nicht über eine Anweisung gesetzt**, sondern vom
Betreiber. Eine Anweisung stünde jedem Absender offen, und wer die
Gewichte setzt, setzt die Verteilung des Ertrags.

⛑ **Und drei Tests von mir sahen stärker aus, als sie waren**, alle drei
in der Gegenprobe aufgefallen: einer prüfte „ohne Bündel keine
Auszahlung" statt „ohne Verteilung keine Auszahlung", die anderen beiden
stehen im Scheduler-Changelog.

### v0.19.0 – 2026-09-01 (Punkt 40, Glied 1: das Bündel über die Kette)

`Anweisung::BuendelEinreichen` angewandt, und der Epochenabschluss leert
die Bündel der abgerechneten Epoche.

⚑ **Heute werden sie verworfen, ohne zugeschrieben zu werden**, weil
dafür die Pod-Besetzung im Zustand fehlt (Glied 3c). **Verloren geht
dabei nichts**, denn geprägt wurde für sie auch nichts; gewonnen ist,
dass der Weg in die Kette steht.

Zwei Tests, eine Gegenprobe: Ohne das Leeren bleiben die Bündel über den
Epochenwechsel stehen, und der Zustand wüchse unbegrenzt.

### v0.18.0 – 2026-09-01 (Punkt 40, Glied 3a: Anmeldung über die Kette)

Zwei angehängte Anweisungen, `MinerAnmelden` und `MinerAbmelden`, und
ihre Anwendung in `anwenden`.

⚑ **Die Kennung steht nicht in der Anweisung**, sie folgt aus dem
Schlüssel, mit dem unterschrieben wurde. Ein zusätzliches Feld ließe
sich abweichend füllen, und dann meldete A für B an. Dieselbe
Begründung, aus der `Anweisung` kein Absenderfeld trägt.

⚑ **Und die Registrierungsepoche steht auch nicht darin.** Sie setzt
die Kette aus ihrem eigenen Zustand; ein selbst gewähltes Datum hübe den
Registrierungsschluss auf.

Vier Tests, darunter die Gegenprobe zu Fund 85 für die neue Anweisung
(eine verfälschte Unterschrift wirkt nicht) und die Wurzelgleichheit
zwischen Erzeuger und Übernehmer.

### v0.17.0 – 2026-09-01 (⚑ Punkt 38: der Knoten schließt die Epoche ab)

Die Rechnung stand seit dem 31. August vollständig: Zuschreibung,
Ausschüttung, `praegen`. **Was fehlte, war der Aufruf.** Er steht jetzt
in `anwenden`, also an der einzigen Stelle, an der sich der Zustand
ändert.

⚑ **Und genau dort, weil beide Seiten sie durchlaufen.** Ein Abschluss,
den nur der Blockerzeuger rechnet, wäre eine abweichende Zustandswurzel
und damit ein Konsensbruch. Die Reihenfolge ist tragend: Erst wird die
**vorige** Epoche abgeschlossen, dann gilt die neue. Eine Gegenprobe mit
vertauschter Reihenfolge fällt durch.

⚑ **Die Zuschreibung ist heute leer, und das ist kein Versehen.** Sie
leitet sich aus bestätigten PoI-Bündeln ab, und **diese Kette trägt
keine**: `Anweisung` kennt Burn, Überweisung und die drei
Sitzungsanweisungen, kein Bündel. Ohne bezeugte Arbeit gibt es nichts
zuzuschreiben.

**Die Folge ist die sichere.** Der Shard-Miner-Anteil wird **nicht
geprägt**, weil ihm kein Empfänger gegenübersteht; geprägt wird allein
der Treasury-Anteil. Ein Test hält fest, dass die Geldmenge um genau
diesen Anteil wächst und um sonst nichts. **Was damit noch fehlt, ist
eine Anweisung, die ein Bündel in die Kette trägt, samt ihrer Prüfung.**

⚑ **Ein Test prüft Übereinstimmung, nicht Richtigkeit**, und das steht
jetzt an ihm dran. Erzeuger und Übernehmer kommen über die
Epochengrenze zur selben Wurzel; **beide laufen durch denselben Code**,
ein falsch rechnender Abschluss rechnete auf beiden Seiten gleich falsch
und der Test bliebe grün. Zwei Gegenproben haben genau das gezeigt: Sie
brachten die anderen beiden Tests zu Fall, diesen nicht.

**Die Prägeparameter sind fest**, solange die Kette eine Probekette ist.
Ein Parameter, der sich ändern kann, gehört an eine Stelle, die beide
Seiten gleich sehen, und die Governance-Registry ist noch nicht an die
Kette gebunden. Bis dahin ist ein fester Wert ehrlicher als ein
beweglicher, den nur einer kennt.

### v0.16.0 – 2026-08-30 (beide Richtungen aus Fund 67 treffen sich an einer Stelle)

Der Knoten springt auf eine höhere Runde, sobald mehr als ein Drittel
des Stimmgewichts von dort geprüft eingegangen ist. Das ist die zweite
Hälfte des Zustandsabgleichs: Bisher konnte nur der zurückgeholt werden,
der **voraus** war.

**Beide Wege treffen sich im selben Zweig**, und das ist kein Zufall:
Beide zeigen sich als `WrongRound`. Wer voraus ist, bekommt von uns das
Commit-Zertifikat; wer zurück ist, springt selbst. Sie schließen
einander aus, ohne dass es geprüft werden müsste, denn das Helfen
verlangt ein eigenes Zertifikat, und wer commitet hat, springt nicht
mehr.

Neues Urteil `gesprungen` im Protokoll, mit alter und neuer Runde. Es
steht neben `falsche-runde`, und der Unterschied ist der Befund: Eine
verirrte Nachricht der alten Runde ist der Normalfall, ein Sprung ist
ein Ereignis.

⛑ **Das Aufräumen nach einem Rundenwechsel stand nur im Fristweg.**
Stimmen und Commits der alten Runde werden verworfen, das Zertifikat
bleibt. Wäre der Sprung daran vorbeigelaufen, hätte der Knoten aus
Stimmen der alten Runde ein Zertifikat gebaut, das keine Runde mehr
bezeugt. Beide Wege benutzen jetzt denselben Rumpf, und ein Test hält
fest, dass der Sprung die alten Stimmen wegwirft.

**Gemessen an den Stakes des Probenetzes:** 250, 230, 200, 120 und 100
Millionen ergeben 900, die Schranke liegt bei 300 000 001. Alpha allein
trägt 250 Millionen und bewegt nichts; mit beta sind es 480, und erst
dann springt es. Der Test zeigt beide Hälften, sonst bliebe offen, ob
die Schranke wirkt oder ob jede fremde Runde genügt.

**Was der Sprung nicht heilt:** Wer auf Runde 5 springt, hat deren
Vorschlag verpasst und wartet bis zur Frist auf Runde 6. Das ist
trotzdem der kürzere Weg, denn er überspringt jede Frist dazwischen.

### v0.15.1 – 2026-08-30 (die Probe baut ein Bündel, das etwas bezeugt)

`probe_poi_buendel` zählte Segment-Ids auf und bildete daraus die
Bündelwurzel. Seit Fund 100 bezeugt die Wurzel `Id ‖ Spurwurzel`, also
baut die Probe jetzt auch eine Spur. Ein Bündel, das nur Positionen
aufzählt, prüfte den Weg nicht, den ein echtes nimmt.

### v0.15.0 – 2026-08-29 (⚑ Fund 96: Anfechtungen werden geprüft, aber nur, wo geprüft werden kann)

Eine Anfechtung fiel im `ProtokollValidator` unter `_ => true`, und bis
zum selben Tag trug sie auch gar keine Unterschrift. Beides zusammen
hieß: Jeder konnte im Namen jedes Miners anfechten. Das kostet den
Angeklagten etwas, denn er muss antworten.

Seitdem prüft der Knoten sie, und die Probe-Anfechtung wird
unterschrieben: Der Herausforderer ist der Absender, seine Kennung wird
aus dem Probeschlüssel abgeleitet. Eine Probe mit erfundenem
Herausforderer prüfte den Weg nicht, den eine echte Anfechtung nimmt,
sondern einen, den es nicht mehr gibt.

### ⚑ Der Unterschied zum Latenz-Attest, und er hätte beinahe gefehlt

Ein Attest kommt von einem **Validator**, und die Validatorenliste ist
genau die Menge, gegen die geprüft wird. Ein Herausforderer ist dagegen
ein **Miner** des redundanten Pods, und der muss dort nicht stehen. Wer
ihn deshalb abwiese, verwürfe aus **geratener Unkenntnis**, und im
Gossipsub-Scoring trifft das den ehrlichen Absender, nicht den
Angreifer. Dieselbe Überlegung, aus der Konsensnachrichten hier nur
strukturell geprüft werden.

Also: unbekannter Herausforderer geht durch, falsche Unterschrift eines
**bekannten** nicht. Eine Anfechtung, deren Absender niemand zuordnen
kann, führt trotzdem zu nichts, denn die Slash-Entscheidung verlangt den
Schlüssel.

⚑ **Die Zuordnung Miner zu Schlüssel gehört in eine Registrierung, die
es noch nicht gibt.** Solange die Teilnehmerliste die einzige Quelle
ist, prüft dieser Zweig im echten Netz nur die Validatoren unter den
Herausforderern. Steht so im Code.

### v0.14.0 – 2026-08-29 (die Mindestfassung berichtigt)

`rust-version` nannte `1.85` und war falsch: Über libp2p hängen `icu_*`
(1.86) und `time` (1.88). Gemessen gegen echte Toolchains mit
`--locked`, jetzt `1.88`, und ein CI-Job fährt sie. Keine Verschärfung,
sondern eine Berichtigung: Der Code brauchte es schon vorher.

Siehe NETWORKING v0.11.0 für die Herleitung.

### v0.13.1 – 2026-08-29 (drei Abschriften weniger)

`Kette::probekonto`, `GenesisValidator::kennung`,
`Konsensschluessel::kennung` und `probe_kennung` rechneten die Ableitung
`sha256(pubkey)` jeweils selbst aus. Sie steht jetzt einmal in
`myl_types` und wird hier gerufen (SHARED_TYPES v0.11.0). An der
abgeleiteten Kennung ändert sich nichts, geprüft von den Gegentests in
`genesis.rs` und `validatorsatz.rs`, die die Regel weiterhin von Hand
nachrechnen.

### v0.13.0 – 2026-08-29 (⚑ Fund 67 geschlossen: der Rückweg für den, der vorauseilt)

Der aufgezeichnete Vorfall vom 26. August: Ein Knoten hatte nach 1 ms ein
volles Mesh und begann Runde 0, die anderen vier begannen ihre erst
522 ms später, seine Vote-Frist von 500 ms lief vorher ab. Er stand am
Ende bei Runde 5, während die vier Runde 0 längst commitet hatten, und
von allein kam er nicht zurück.

**Hier stand, der Rückweg hänge an der Kettenpersistenz.** Das war
falsch, und es fiel erst beim Nachlesen auf: Ein Commit legt bis heute
keinen Block in die Kette und veröffentlicht auch keinen, er schreibt
eine Protokollzeile. Über die Kette wäre nichts zurückgekommen. Der
Rückweg geht über einen Quorumsbeleg, der unabhängig von der Runde des
Empfängers gilt, und er ist in CONSENSUS v0.20.0 beschrieben.

Was der Knoten dazu beiträgt:

- **`Konsensrunde` sammelt jetzt auch Commits**, nicht nur Stimmen, und
  baut daraus den Beleg, sobald das Quorum steht. Aus demselben Grund wie
  bei den Stimmen: Der Automat speichert Commits ohne Signatur, ein
  Zertifikat ist aber ihr Aggregat. Beim Rundenwechsel werden die
  gesammelten Commits verworfen, der fertige Beleg **nicht**: Er bezeugt
  eine Entscheidung, keine Runde.
- **`hilf_beim_aufholen`** antwortet einem Absender, dessen Nachricht mit
  `WrongRound` abprallt, mit dem Beleg der eigenen Entscheidung. Drei
  Bedingungen, jede mit Grund: nur bei falscher Runde, nur mit eigenem
  Beleg, nur an Stimmberechtigte und an jeden genau einmal.
- **`konsens_commitet` trägt ein Feld `uebernommen`.** Ohne das sähen
  „lief mit" und „musste zurückgeholt werden" im Betriebsprotokoll gleich
  aus, und genau dieser Unterschied ist es, den man nach einem Vorfall
  wie dem vom 26. August sucht.
- **`gabelung`** ist eine eigene Marke im Urteil, kein Sammelposten, und
  gilt nicht als harmlos. Sie ist der einzige Befund dieser Liste, der
  nicht über den Absender spricht, sondern über das Netz.

Der Test, der den Defekt festhielt, war eigens so geschrieben, dass er
umschlägt, sobald jemand den Ausgleich baut. Er läuft jetzt den vollen
Weg: Der Vorausgeeilte dreht bis Runde 5, in der er wieder Leader ist,
und gibt sich durch seinen Vorschlag zu erkennen; die Gegenseite
antwortet mit dem Beleg; er übernimmt die Entscheidung und landet auf
demselben Block. Zweimal gegengeprobt, einmal ohne den Sendeweg, einmal
ohne die Übernahme, beide Male schlägt er fehl.

### v0.12.0 – 2026-08-29 (das Tor: wer anders rechnet, kommt nicht ins Netz)

Der Knoten fährt vor dem Netz die Op-Konformitätsvektoren und startet
nicht, wenn seine Maschine abweicht.

### ⚑ Warum das vor dem Netz steht und nicht daneben

Die ganze Zusage des Protokolls ist, dass zwei beliebige Rechner
dasselbe Ergebnis bekommen. **Ein Knoten, dessen Maschine anders
rechnet, ist kein langsamer, sondern ein schädlicher:** Er liefert
abweichende Segmente, wird dafür geschlachtet, und bis dahin
verschmutzt er den Auftragsstrom.

Prüfen ließ sich das schon, mit dem Testclient. **Das ist freiwillig und
getrennt vom Betrieb**, passiert also beim ersten Mal und danach nie
wieder, und niemand merkt es, wenn unter dem Knoten eine Bibliothek
ausgetauscht wird.

### ⚑ Fehlend ist etwas anderes als falsch

Ein **falscher** Vektor heißt: Diese Maschine rechnet anders. Ein
**fehlendes** Verzeichnis heißt: Wir wissen es nicht. Beides hält den
Start an, aber unter verschiedenem Namen, und die Meldung sagt welchen.

**Wer ohne Vektoren starten will, sagt es ausdrücklich**
(`--ohne-konformitaet`). Dann steht „übersprungen" in der Kommandozeile
und im Protokoll, **und das ist etwas ganz anderes als „bestanden"** —
ein eigener Wert, kein Sonderfall des guten.

### ⚑ Der Vorgabepfad sucht an zwei Orten, und das ist kein Komfort

Im Repositorium arbeitet man vom Wurzelverzeichnis, ein Betreiber
entpackt ein Archiv und startet darin. **Ein Vorgabepfad, der nur den
ersten Fall trifft, macht das Tor für genau die Leute unbrauchbar, für
die es gedacht ist** — und sie greifen dann zu `--ohne-konformitaet`,
womit die Prüfung praktisch abgeschafft wäre.

### Was das Tor nicht belegt

Es fährt die **Op-Vektoren**: einzelne Kernel gegen eingefrorene
Sollwerte, ohne Modell, in Millisekunden. Layer- und
Ende-zu-Ende-Vektoren verlangen Artefakte in Gigabyte-Größe; ein Start,
der davon abhinge, wäre für die meisten kein Start. ⚑ **Das Tor belegt
also, dass die Kernel übereinstimmen, nicht dass die ganze Kette es
tut.**

⚑ **Und die Gegenprobe kostete zwei Anläufe.** Der erste verfälschte
blind die erste `1` im Vektor, die in einem Feld stand, das die Prüfung
nicht ansieht; der zweite tauschte Ziffern in einem Wert, der keine der
getauschten enthielt. **Eine Gegenprobe, die nichts verändert, belegt
nichts** und hätte hier ein Tor als wirksam ausgewiesen, das es nicht
ist. Jetzt wird der Erwartungswert ersetzt, und der Test prüft vorher,
dass sich der Text wirklich geändert hat.

**Sechs neue Tests.**

### v0.11.0 – 2026-08-28 (der Testverkehr ist unterschrieben, und die Kette wendet fünf Anweisungen an)

Der Knoten wendet die neuen Transaktionen an: Burn, Überweisung,
Session eröffnen, widerrufen und ausgeben.

⚑ **Die Unterschrift wird beim Anwenden geprüft, nicht beim Aufnehmen
in den Mempool.** Ein Block kommt über Gossip und sieht den Mempool nie;
läge die Prüfung dort, könnte ein Leader eine unsignierte Anweisung in
einen Block schreiben, und die ehrlichen Knoten wendeten sie an.
**Erzeuger und Übernehmer überspringen dasselbe**, weil beide dieselbe
Funktion durchlaufen.

⚑ **Ein Probekonto hat jetzt einen Schlüssel.** Vorher war es nur eine
Zeichenkette, aus der eine Adresse gehasht wurde, und in seinem Namen
konnte jeder anweisen. Jetzt folgt die Adresse aus dem Schlüssel, wie
jede Adresse im Protokoll.

⚑ **Und der Testverkehr zählt seine Nummer hoch.** Ohne das wäre jede
dieser Transaktionen eine Wiedereinspielung ihrer Vorgängerin und würde
verworfen; der Zustand bewegte sich nicht, und die Übereinstimmung der
Zustandswurzeln belegte wieder nichts. **Dieselbe Falle wie beim
fehlenden Guthaben, nur eine Ebene weiter.**

**Vier neue Tests**, darunter die Gegenprobe zu Fund 85: Eine gefälschte
Unterschrift bewegt nichts, und ein fremder Schlüssel belastet sein
eigenes Konto statt des angegriffenen. Zusammen 170.

### v0.10.1 – 2026-08-27 (Gleichstand mit der Netzschicht)

Nur Tests, kein Verhalten. `tests/gleichstand.rs` hält zwei Ableitungen
zusammen, die einander wegen der Schichtung nicht sehen können:
`GenesisValidator::kennung` bildet aus dem BLS-Schlüssel eines
Validators seine `MinerId`, `myl_net::endpunkt_aus_schluessel` bildet
aus demselben Schlüssel den Endpunkt einer verschlüsselten Sitzung.
`myl-net` ist L0 und darf die Genesis-Datei nicht kennen; der Preis
dafür ist eine Doppelrechnung.

⚑ **Was ohne diesen Test auseinanderliefe, und wie es aussähe.** Die
Sitzungsschicht prüft eine Epochenankündigung, indem sie den Endpunkt
aus dem mitgeführten Schlüssel ableitet und mit dem vergleicht, den der
Pod-Pfad nennt. Rechnen beide Seiten verschieden, passt **keine
einzige** Ankündigung mehr, und jede Meldung lautet „gehört zu einem
anderen Endpunkt". Das liest sich wie ein Angriff, und niemand käme
darauf, dass beide Seiten für sich recht haben. Ein Fehler, der wie ein
Angriff aussieht, kostet beim Suchen ein Vielfaches.

Drei Tests: die Gleichheit über vier Saaten, die Gegenprobe (zwei
verschiedene Schlüssel geben zwei verschiedene Endpunkte, sonst hieße
„gleich" auch bei einer konstanten Rückgabe gleich), und der Weg wie im
Betrieb: Ankündigung prüfen gegen die `MinerId` aus der Genesis-Datei,
und ein anderer Validator kommt darüber nicht herein.

### v0.10.0 – 2026-08-27 (Höhe und Epoche sind zwei Dinge)

⚑ **Die Probekette benutzte das Epochenfeld als Blockhöhe.** Das trägt,
solange eine Epoche ein Block ist, und bricht, sobald es das nicht mehr
ist — und es war nicht folgenlos, sondern **still falsch**: Jede Frist
„je Epoche" bedeutete in Wahrheit „je Block". Credits verfielen nach
einem Block statt nach einer Stunde; die Streitfrist von 168 Epochen
wären 168 Blöcke gewesen, also gut fünf Minuten statt sieben Tagen.

Der Blockkopf trägt jetzt beides (`myl-consensus` v0.14.0): `height`
wächst um genau eins je Block, `epoch` folgt aus der Höhe. Beim
Übernehmen wird beides geprüft — die Höhe gegen die eigene, die Epoche
gegen die Umrechnung. Zwei neue Ablehnungsgründe, `hoehe-weicht-ab` und
`epoche-weicht-ab`, mit eigenen Marken im Betriebsprotokoll: Sie sind
Befunde über den Absender und keine Anschlussprobleme, und wer sie unter
`passt-nicht-an` führte, löste damit auch noch eine Nachforderung aus.

⚑ **Und die Epoche des Ledger-Zustands stand auf null und blieb dort.**
`anwenden` benutzte die Epoche nur zum Rechnen des Verfalls und setzte
`zustand.epoch` nie. Jede Prüfung, die an der laufenden Epoche hängt,
lief damit gegen null. Sie wandert jetzt mit; ein Test baut 1 800 Blöcke
und verlangt, dass der Zustand danach Epoche 1 kennt.

**Die Nachforderung hängt jetzt an `height`.** Der Modulkopf von
`nachschub.rs` hat dieselbe Aussage in zwei Fassungen getragen und beide
Male danebengelegen — einmal „die Lücke ist nicht benennbar", einmal
„sie ist über `epoch` benennbar". Beide Male, weil dasselbe Feld zwei
Bedeutungen hatte.

**Die Formatfassung der Kettendatei steigt von 1 auf 2.** Ein Höhenfeld
ändert die Borsh-Kodierung jedes Satzes; eine Datei der alten Fassung
würde nicht scheitern, sondern **falsch geparst**. Genau dafür steht die
Zahl im Kopf.

### v0.9.0 – 2026-08-26 (die Kette überlebt den Neustart)

- **`speicher.rs` (neu):** ein anhängendes Blockprotokoll. Kopf mit
  Magie, Formatfassung und **Startwert**, der die Datei an ihre Kette
  bindet: Eine Datei aus einem anderen Netz wird abgewiesen, statt eine
  fremde Historie als eigene auszugeben. Je Satz Länge, Nutzlast und
  Prüfsumme.
- **Der Speicher gehört der Kette, nicht dem Aufrufer.** Die Zusage
  lautet „jeder Block, der in die Kette kommt, steht auch in der Datei".
  Läge das Schreiben beim Aufrufer, wäre sie auf drei Aufrufstellen
  verteilt, und die vierte, die jemand später hinzufügt, vergäße es.
- **Der Wiederanlauf ist ein Nachrechnen, kein Einlesen.** Ein Test
  verfälscht einen gespeicherten Block mit **gültiger Prüfsumme**: Der
  Speicher lässt ihn durch, die Kette nicht.
- ⚑ **Was `flush` zusichert, steht als Tabelle im Modulkopf.** `kill -9`
  des Prozesses überlebt das Protokoll, ein Stromausfall vielleicht
  nicht. Kein `fsync` je Block; für ein echtes Netz ist das eine offene
  Entscheidung, keine Empfehlung.
- **Neu auf der Kommandozeile:** `--kette <datei>`. Ohne sie beginnt
  jeder Start bei null, und der Knoten sagt das beim Hochfahren.
- **Ein doppeltes Literal beseitigt:** Der Startwert der Probekette stand
  zweimal da. Wären die beiden Stellen auseinandergelaufen, hätte eine
  frische Datei ihre eigene Kette abgelehnt, und der Knoten begänne nach
  jedem Neustart bei null, ohne dass es jemandem auffiele.
- **Ein veralteter Kommentar nachgezogen:** In `sende_testverkehr` stand
  „Eine Blocksynchronisierung fehlt und gehört vor ein echtes Testnetz."
  Sie gibt es seit v0.4.0 (`nachschub.rs`).

### v0.8.0 – 2026-08-26 (Rundenwechsel)

- **`konsens.rs` fährt jetzt `RoundDriver` statt `BftState`.** Damit
  kommen Uhr, Sperre und Rundenwechsel dazu. Der Knoten hält eine
  **monotone** Uhr (`Instant`), nicht die Wanduhr: Ein NTP-Sprung
  rückwärts verlängerte sonst eine laufende Frist, ein Sprung vorwärts
  ließe sie zu früh feuern, und eine grundlos gewechselte Runde sieht im
  Protokoll aus wie ein ausgefallener Leader.
- **Der Ereignistakt begrenzt seine Wartezeit auf die Konsensfrist.**
  Ohne das schliefe der Knoten bis zur nächsten Zustandsaufnahme, also
  bis zu 30 Sekunden, und ein ausgefallener Leader hielte die Runde so
  lange auf, obwohl die Frist längst abgelaufen wäre.
- **Der Leader sammelt die vollständigen Stimmen**, weil `BftState` sie
  nur als `MinerId → Hash` führt und ein Polka-Zertifikat ihr Aggregat
  ist.
- **Neu auf der Kommandozeile:** `--bft-frist`, `--bft-zuwachs`. Ein
  Zuwachs von null wird angenommen, aber gewarnt: sicher, möglicherweise
  dauerhaft blockiert.
- ⚑ **Fund 67: Wer allein vorauseilt, kommt nicht zurück.** Gemessen
  über fünf Prozesse. Der erste Knoten hatte nach 1 ms ein volles Mesh
  und begann seine Runde; die anderen vier begannen ihre erst 522 ms
  später. Seine Vote-Frist (500 ms) lief vorher ab, er wechselte **mit
  Stimmgewicht 0** auf Runde 1 und stand am Ende bei Runde 5, während
  die vier Runde 0 längst commitet hatten. **Ein volles Gossip-Mesh
  heißt nicht, dass die Gegenstellen mitstimmen.** Als Test festgehalten,
  nicht behoben: Der Rückweg braucht einen Zustandsabgleich.

### v0.7.0 – 2026-08-26 (BFT-Runden über das Netz)

- **`genesis.rs` (neu):** Der Validator-Satz kommt aus einer Datei, nicht
  aus dem Netz. Wer sich selbst ankündigen darf, kündigt sich fünfzehnmal
  an; genau dieser Fehler steckte bis v0.3.6 in `myl-consensus::bft`
  (Fund A3). Der Hash liegt auf dem **Inhalt**, nach Kennung sortiert,
  nicht auf den Dateibytes: dieselbe Unterscheidung wie in Kap. 6.2.
  Besitznachweis je Schlüssel (Fund 27), Ein-Drittel-Schranke als
  Startfehler. 23 Tests.
- **`schluessel.rs` (neu):** BLS-Konsensschlüssel, **getrennt** von der
  Netzidentität. Ein gemeinsames Geheimnis wäre eines weniger zu
  verwalten und kompromittierte bei einem Leck beide Ebenen. Datei mit
  Rechten 0600; eine für Gruppe oder Welt lesbare Datei wird **nicht**
  gelesen. Der Probeschlüssel bleibt, aber als eigener Aufruf mit
  eigener Herkunft, die in jeder Startzeile steht. 12 Tests.
- **`konsens.rs` (neu):** Wann ein Knoten selbst etwas sagen muss.
  `BftState` prüft und zählt, aber erzeugt nichts, weil ihm der
  Schlüssel fehlt. 16 Tests, darunter drei Teilmengen gleicher Kopfzahl
  mit drei verschiedenen Urteilen.
- **Kommandozeile:** `--genesis`, `--konsensschluessel`,
  `--probe-konsensschluessel`, `--genesiszeile`. Ohne den letzten müsste
  jeder Betreiber 288 Zeichen Hex von Hand abschreiben, und ein Weg, den
  niemand geht, ist kein Weg (Fund 55).
- ⚑ **Fund 63: 417 Millisekunden, und die Runde war tot.** Im ersten
  Lauf über fünf Prozesse kam der Propose des Leaders bei allen vier
  anderen an, **4 ms** nach dem Absenden. Ihre eigene Runde begann
  **417 ms** später, weil jeder erst auf sein Gossip-Mesh wartete. In
  dieser Lücke war `self.konsens` noch `None`, und die Nachricht wurde
  verworfen, **ohne Protokollzeile**. Danach wartete das ganze Netz auf
  einen Propose, den es längst hatte. Behoben mit einem beschränkten
  Vorlauf-Puffer, der beim Rundenbeginn nachreicht. **Der Modultest
  konnte das nicht sehen:** Eine Nachrichtenschlange serialisiert, was
  ein Netz parallel macht.
- ⚑ **Fund 64: zwei Felder namens `art` in einer Protokollzeile.**
  `block_abgelehnt` schrieb seit dem 2026-08-24 ein zweites `art`-Feld,
  `konsens_gesendet` tat es ihm nach. Solche Zeilen schlagen nirgends
  fehl: Ein Leser nimmt das erste, ein anderer das letzte. Der Leser in
  `tests/zwei_knoten.rs` filtert nach `z.art` und hätte je nach
  Reihenfolge etwas anderes gesehen. Behoben, plus `debug_assert` auf
  reservierte Feldnamen und ein Test über die geschriebenen Zeilen.
- ⚑ **Fund 65: dieses Crate lief in keinem CI-Job.** Weder Cache noch
  Clippy noch Tests. 132 Tests, keiner davon in der CI, und das
  ausgerechnet in dem Crate, das laut eigenem Modulkopf **die Nähte
  belastet**: Die Funde 55 bis 57 wurden hier sichtbar. Aufgenommen.

### v0.6.0 – 2026-08-25 (A10: Latenz-Atteste werden geprüft)

`src/validatorsatz.rs` ordnet Kennung zu Schlüssel und liefert den
**Grund** einer Ablehnung. `ProtokollValidator` prüft damit Atteste;
dort stand vorher `_ => true`. Der Knoten **erzeugt** Atteste aus seinen
tatsächlich gemessenen Latenzen, nicht aus erfundenen Zahlen.

**Warum das ein Fund war und nicht nur eine Lücke:** Das Feld
`signature` stand seit dem ersten Entwurf im Typ, und niemand hat es je
gesetzt oder geprüft. Der Sicherheitsaudit sagt scharf, warum das
schlimmer ist als ein fehlendes Feld: **Ein ungeprüftes Signaturfeld ist
gefährlicher als gar keines, weil ein Leser es für einen Schutz hält.**

⚑ **Die Vermutung, das entstehe nebenbei mit dem Binary, war falsch.**
Der Audit hatte notiert, die Stelle entstehe ohnehin, sobald `myl-net`
und `myl-consensus` in einem Prozess zusammenkommen. Die **Stelle** war
da, die **Sache** nicht: `LatencyAttest` hatte weder `sign` noch
`verify`. Die Prüfung, für die alles vorbereitet schien, brauchte erst
ihre Primitive.

**Live belegt, drei Knoten:** Alpha und Beta kennen einander und nehmen
gegenseitig an; ein dritter, der in keiner Liste steht, bekommt alle
Atteste verworfen und verwirft selbst alle fremden. Der Verwerfungs-
eintrag nennt den wahrscheinlichsten Grund, damit niemand nach einem
Angriff sucht, wo eine Kommandozeile unvollständig war.

**⚠️ Was offen bleibt: die Schlüsselherkunft.** Im Probelauf werden die
Schlüssel aus den Teilnehmernamen abgeleitet. Wer die Namen kennt, kann
in fremdem Namen signieren. Die Trennlinie liegt **nicht im Prüfcode**:
Dieselbe Funktion arbeitet unverändert gegen echte Schlüssel, sobald die
Validator-Registrierung zu Genesis steht. Das ist dieselbe
Voraussetzung, die auch die BFT-Runden brauchen.

### v0.5.0 – 2026-08-25 (Durchsicht vor dem ersten Mehrmaschinenlauf)

Vier Funde aus einer gezielten Funktionsprüfung, alle behoben. Sie
hätten den ersten Lauf über getrennte Maschinen jeweils verwirrend
scheitern lassen.

⚑ **Nur die TCP-Adresse wurde angezeigt.** `warte_auf_adresse` kehrte
zurück, sobald irgendeine Adresse vorlag, und TCP horcht schneller als
QUIC. Der Betreiber bekam die quic-v1-Adresse **nie zu sehen**, konnte
also nur die TCP-Adresse weitergeben, und das ganze Netz lief über TCP.
Der Rat in der Anleitung, die quic-v1-Adresse zu verteilen, war damit
unbefolgbar. Jetzt wird gewartet, bis die Adressen sich beruhigt haben,
und es gibt eine Warnung, wenn keine QUIC-Adresse zustande kommt.

⚑ **Der Mempool eines Nicht-Erzeugers wuchs für immer.** Er nahm jede
Transaktion aus dem Gossip auf und leerte nie, weil er keine Blöcke
baut. Gemessen: 0, 0, 3, 4 wartende Einträge über vierzig Sekunden, ohne
Ende. Schlimmer als der Speicher ist die zweite Folge: **Ein solcher
Knoten hätte, sobald er je Erzeuger würde, einen Block aus tausenden
längst verarbeiteter Transaktionen gebaut.** Jetzt streicht jeder
übernommene Block, was er enthält.

⚑ **Strg-C schrieb keinen Abschluss.** Mit `--laufzeit` behandelte
niemand das Abbruchsignal; der Prozess starb wortlos. Das Protokoll
blieb vollständig (jede Zeile wird sofort geschrieben), endete aber
mitten im Betrieb, und **„absichtlich beendet" ließ sich nicht von
„abgestürzt" unterscheiden.** Beide Wege laufen jetzt über
`laufen_bis` und schreiben `ende` mit Grund.

**Gemessen an vier Prozessen** mit QUIC-Einladung, einem Nachzügler nach
24 Sekunden und einem Abbruch nach 36: alle Proben ohne Fehlschlag,
Zustandswurzeln auf allen 13 Höhen gleich, Uhrversatz höchstens 1 ms.

### v0.4.0 – 2026-08-24 (Nachzügler holen auf)

`src/nachschub.rs`: Ein Knoten, der Blöcke verpasst hat, fordert sie
beim Absender des Hinweises nach. Gemessen an drei Prozessen: Gamma kam
zwanzig Sekunden später dazu, forderte die Blöcke 1 bis 8 an, bekam
alle acht und stand **zwei Millisekunden später** auf Höhe 8. Am Ende
stimmten die Zustandswurzeln auf allen zwölf vergleichbaren Höhen.

**Nachschub ist ein Transportweg, kein Vertrauensweg.** Nachgelieferte
Blöcke gehen durch dieselbe Anschlussprüfung und dieselbe Nachrechnung
der Zustandswurzel wie verbreitete. Wäre das ein zweiter, schwächerer
Weg, wäre er das Loch: Wer einen Knoten zum Nachfordern bringt, bekäme
einen Block hineingelegt, ohne dass er nachrechnet. Ein Test hält das
fest.

**Eine Anfrage zur Zeit**, und der Bereich ist auf 64 Blöcke gedeckelt.
Ohne die Sperre schickt ein Neuling für jeden abgelehnten Block eine
neue Anfrage; ohne den Deckel brächte er den Gegenüber dazu, beliebige
Datenmengen zu senden.

⚑ **Korrektur einer früheren Aussage.** In v0.2.0 stand, ein Knoten
könne die Lücke nicht benennen, weil `Block` kein Höhenfeld trägt. Das
war falsch: Die Probekette schreibt die Höhe in `epoch_meta.epoch`.
Richtig bleibt, dass der **Protokolltyp** kein Höhenfeld hat und `epoch`
dort eine Epoche bedeutet, keine Blockhöhe. Diese Doppelbelegung gehört
aufgelöst, bevor daraus ein echtes Netz wird.

### v0.3.0 – 2026-08-24 (Probelauf: benannte Funktionsproben)

`src/probe.rs` mit sechs benannten Proben und echten Protokollobjekten:
PoI-Bündel mit gültiger Merkle-Wurzel und echter BLS-Signatur,
strukturell gültige Challenges. Ein aus Zufallsbytes gebautes Objekt
käme durch dieselbe Netzprüfung (Fund 45) und belegte nichts über den
Weg, den ein echtes nimmt.

Die Rahmung ist umgestellt: aus „Testkette" wurde die **Probekette**,
aus `genesis()` der `probestand()`. Ein Genesis ist ein einmaliges
Ereignis mit Folgen; das hier entsteht bei jedem Start neu, und der
Unterschied gehört in den Namen.

⚑ **Der erste Probelauf mit drei Knoten deckte eine Lücke auf:** Der
Erzeuger baute acht Blöcke, bevor die anderen verbunden waren. Sie
wiesen alle acht mit „passt nicht an" zurück und blieben auf Höhe 0.
**Es gibt keinen Nachholmechanismus.** Der Erzeuger wartet jetzt auf den
ersten Peer, und die Auswertung benennt das Muster. Die Ursache bleibt:
Wer mitten im Lauf dazukommt, hängt fest. Blocksynchronisierung fehlt
und gehört vor ein echtes Testnetz.

### v0.2.0 – 2026-08-24 (Probekette: echte Blöcke)

`src/kette.rs` mit Kettenzustand, Mempool und Zustandswurzel. 13 Tests.
Über drei Prozesse gemessen: sieben Blöcke erzeugt, sieben übernommen,
Zustandswurzeln auf allen sieben Höhen gleich.

⚑ **Der erste Lauf war grün und maß nichts.** Die Zustandswurzel stand
bei jeder Höhe auf demselben Wert, obwohl Blöcke mit Transaktionen
ankamen: Die Burn-Transaktionen scheiterten an fehlender Deckung und
wurden übersprungen, wie vorgesehen. **Ein unveränderter Zustand ist auf
jeder Maschine gleich**, also belegte die Übereinstimmung genau nichts.
Seitdem stattet der Genesis acht Testkonten mit Guthaben aus, und ein
Test (`ein_wirksamer_burn_veraendert_die_zustandswurzel`) hält fest,
dass sich die Wurzel bewegen muss.

### v0.1.0 – 2026-08-24 (erster Knoten)

26 Tests grün (21 Unit, 5 Integration). Gemessen an zwei echten
Prozessen: Beide finden einander, beide protokollieren die Verbindung
mit korrekter Richtung, der Empfänger vermerkt den Abgang des anderen.

⚑ **Fund 57: Auch für Blöcke ist der Borsh-Parse fast nur eine
Längenprüfung.** Die erste Fassung der Moduldoku behauptete, `Block`
trage Vektoren und der Parse habe deshalb „Zähne“. **Der Test, der das
messen sollte, hat es widerlegt: rund 88 % der verstümmelten Blöcke
kommen durch.** Ein Block mit wenigen Einträgen besteht fast nur aus
Feldern fester Breite; empfindlich sind allein die Längenköpfe.
Dieselbe Eigenschaft wie in Fund 45, nur abgeschwächt, und die
Abschwächung reicht nicht, um von „Prüfung“ zu sprechen. Die
Verteidigung für Blöcke ist die Signatur gegen die Validator-Registry,
und die braucht Kettenzustand.
