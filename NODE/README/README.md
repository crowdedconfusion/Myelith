# NODE — der Myelith-Knoten

> **Version:** myl-node v0.1.0
> **Datum:** 2026-08-24
> **Status:** Netzknoten lauffähig, keine Blockproduktion

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
Kademlia, verbreitet und empfängt die fünf Protokoll-Topics, misst
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

**Ist nicht: BFT.** Es stimmt niemand ab. **Genau ein Knoten erzeugt**
(`--erzeuger`), die übrigen übernehmen. Zwei Erzeuger gabeln die Kette
sofort, weil niemand entscheidet, welcher Block gilt, und genau das täte
eine Abstimmungsrunde. `myl-consensus::bft` hat sie fertig; ihr fehlen
ein eigenes Gossip-Topic, ein Validator-Satz mit Stake und BLS-Schlüssel
je Knoten. **Ein neues Topic ist eine Protokollentscheidung** und gehört
nicht nebenbei getroffen.

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
        ├── validator.rs      Nutzlastprüfung Blöcke/Transaktionen (L1)
        ├── nachschub.rs      Blocknachforderung: Bereich, Deckelung,
        │                     Nachlieferung
        ├── knoten.rs         Start, Ereignisschleife, Zustandsaufnahme
        └── main.rs           Kommandozeile
    └── tests/
        └── zwei_knoten.rs    zwei Knoten, echte Sockets, Protokoll
                              zurückgelesen (5 Tests)
```

## Changelog

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
