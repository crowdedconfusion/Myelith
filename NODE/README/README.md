# NODE — der Myelith-Knoten

> **Version:** 0.99.2
> **Datum:** 2026-10-29
> **Status:** Netzknoten lauffähig, Blockproduktion mit **Persistenz über
> Neustarts**, BFT-Runden über das Netz mit Rundenwechsel, und seit dem
> 1. September **schließt der Knoten die Epoche selbst ab**.
> **423 Tests grün** (gezählt am 2026-10-28).
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

### v0.99.2 (2026-10-30): Nur die Antwort auf die Zeitprobe zählt

- ⛔️ **Zeitproben gehen mit Zweck hinaus** (`NodeCommand::AnfrageMitZweck`,
  NETWORKING 0.24.0), und nur ein `Nichts` oder ein Fehlschlag **mit diesem
  Zweck** zählt als Abweisung der Probe. Bis hierher zählte jedes `Nichts`
  einer gemessenen Gegenstelle, auch eines auf eine Blocknachforderung
  oder aus der Drossel; eine Folge davon wird zum Verdacht und zum Zeugnis
  gegen ein ehrliches Mitglied. Die offene Empfehlung „Antwortmarken für alle
  Nachforderungen“ ist damit für die Messung erledigt.
- 📌 **Zweiter Fehler an derselben Stelle**: Ein `Nichts` einer gemessenen
  Gegenstelle wurde abgefangen und erreichte die Buchführung des Nachholens
  nie (wer leer antwortete, wurde nicht übergangen). Jetzt geht es den
  gewöhnlichen Weg; nur die Quelle des Dateiholens bleibt eigens behandelt.
- Test `nur_die_antwort_auf_die_probe_zaehlt` über den Ereignisweg (ohne
  Zweck berührt nichts die Messung, mit Zweck ist es eine Abweisung).

### v0.99.1 (2026-10-30): Der zustandslose Knoten holt im Ganzen nach

📌 Kurzlauf `kurz-ablage` (abgebrochen): Nach einer Trennung folgte der
zustandslose Miner genau im Takt des Erzeugers, 130 Blöcke dahinter, und
holte nie auf. Weil er nie 30 s stillstand, griff auch die Selbstheilung
nicht. Im Protokoll drei Verschwendungen:

- ⚑ **Ein Block, der neu ins Wartefenster kommt, zählt als geliefert**:
  Bis hierher galt nur ein angenommener Block, und beim zustandslosen
  Knoten wird keiner sofort angenommen. Die Kette des Nachforderns riss ab,
  und der nächste Bereich kam erst mit dem nächsten Block über das Gossip,
  einmal je Takt.
- ⚑ **Die Bereichsplanung kennt das Wartefenster**: Blöcke, die schon auf
  ihren Zeugen warten, werden nicht noch einmal geholt (im Protokoll immer
  wieder einzelne Blöcke derselben Höhe).
- ⚑ **Zustandslos nur bis zum Rand des Wartefensters**: Was darüber liegt,
  wurde geholt und gleich verworfen.
- Die Lieferung nennt `gewartet`. Test
  `ein_zurueckgefallener_holt_mit_zeugen_auf_waehrend_die_kette_laeuft`
  (Beta steht 300 Blöcke, holt dann ohne Neuverankerung auf, während Alpha
  baut). ⚠️ Er besteht auch ohne die Korrektur: Mit einer einzigen
  Gegenstelle zeigt sich der Fehler nicht. Er sichert das Aufholen bei
  laufender Kette, nicht diesen Fehler.
- Kurzlauf `kurz-aufholen` danach bestanden: Nach der ersten Trennung (rund
  1000 Blöcke Rückstand, mehr als das Fenster) sprang der zustandslose
  Miner zum Prüfpunkt und füllte mit Zeugen auf, nach der zweiten holte er
  mit Zeugen auf. Ob die Korrektur oder der Zufall der Schwelle den
  Unterschied machte, belegt ein einzelner Lauf nicht; der Nachtlauf mit
  vielen Trennungen soll es zeigen.
- 📌 Unterwegs: `cargo build` in `myl-pod` als Prüfbau überschrieb die
  vorgebauten Shard-Programme ohne `pruefnetz`, und fünf Testclient-Tests
  scheiterten scheinbar an Last. Prüfbau dort nur mit `cargo test --no-run`.

### v0.99.0 (2026-10-30): Die Zeugen auf der Platte

Der offene Punkt aus v0.98.1: Die Vorhaltung der Zeugen muss den Abstand vom
Prüfpunkt zur Spitze decken, sonst trägt die Selbstheilung eines
zustandslosen Knotens nicht.

- ⚑ **`zeugenablage`** (neues Modul): Erzeuger und Zeugenhalter legen die
  unterschriebenen Stücke jedes Blocks auch auf die Platte, eine Datei je
  Block, neben der Kettendatei (`<kette>.zeugen`, sonst im
  Protokollverzeichnis). **Ein Fenster nach Höhe**, drei
  Prüfpunktabstände (450 Blöcke), nicht nach Bytes; 4 GiB als Notbremse.
  Ein Zwischenspeicher: beim Start geleert, ohne Schreibprotokoll. Eine
  Nachfrage liest erst den Speicher, dann die Platte.
- **Vorbild NEAR**, geprüft: Dort halten Validatoren Zeugen nur für die
  Spitze und holen beim Rückstand den Zustand (State Sync). Unser leichter
  Abzug ist das Gegenstück, verankert aber am Prüfpunkt; deshalb das
  Fenster bis zur Spitze. Unter echter Last (ein voller Block bei einer
  Million Konten bringt Zeugen im MB-Bereich) fassten 64 MiB im Speicher
  nur einige Dutzend Blöcke.
- Keine neue Angriffsfläche: Die Dateinamen entstehen allein aus Höhe und
  Hash, der Empfänger prüft jedes Stück wie bisher, eine beschädigte Datei
  liest sich als nichts.
- ⚑ **`zustandslos_nur_abzug`**: Nach drei Neuverankerungen ohne einen
  Block mit Zeugen dazwischen meldet der Knoten, dass er nur noch springt.
  `groessen` trägt `zeugen_platte_bytes`, `zeugen_platte_bloecke` und
  `zeugen_platte_fehler`.
- Tests `das_fenster_folgt_der_hoehe`, `die_grenze_und_eine_beschaedigte_datei`,
  `zeugenstuecke_kommen_auch_von_der_platte`.

### v0.98.2 (2026-10-30): Doppelbelege am Epochenwechsel aufgeräumt

- Der Epochenwechsel ruft `doppelbelege_aufraeumen` (CONSENSUS 0.76.0)
  neben dem Aufräumen der Sitzungen.
- Kurzlauf `kurz-morgen` mit Trennungen und Laststufen bestanden: Der
  zustandslose Miner folgte nach beiden Trennungen wieder mit Zeugen (zwei
  Neuverankerungen, danach 1934 Blöcke mit Zeugen), Zeugen im Median unter
  1 KB; die Halter lieferten.

### v0.98.1 (2026-10-30): Zeugennachfragen reihum

📌 Fund aus dem langen Podlauf `nacht-bereit`: Der zustandslose Miner fragte
nie einen der vollen Miner, die Zeugen bereithielten. Weitere Gegenstellen
waren immer dieselben ersten vier, und in die Quellen kommt nur, wer schon
geliefert hat. Über den ganzen Lauf kam jedes Stück vom Erzeuger; nach einer
Trennung lief er bei 188 Nachfragen in Zeitüberschreitungen, und der
zustandslose Knoten kam viermal nur über einen leichten Abzug weiter (die
Selbstheilung trug, die Zeugen nicht).

- ⚑ **Reihum durch die Gegenstellen** (`reihum`): je Runde höchstens vier
  weitere, beginnend dort, wo die letzte aufhörte; nach `ceil(len / 4)`
  Runden war jede dabei, und wer liefert, bleibt als Quelle. Keine neue
  Angriffsfläche: Es bleiben vier Anfragen je Runde, angenommen wird wie
  bisher nur von Erzeugern oder eingetragenen Minern.
- Test `jede_gegenstelle_kommt_dran`.
- 📌 **Warum der zustandslose Miner ab 04:15 gar nicht mehr folgte**: Die
  Zeugen waren auf einen Median von 526 KB gewachsen (Ursache: CONSENSUS
  0.75.0, Credits ohne Zusammenlegen). Gehalten werden 64 MiB, also rund
  100 Blöcke; der leichte Abzug setzt am Prüfpunkt an, 300 bis 400 Blöcke
  hinter der Spitze. Nach jedem Sprung hielt niemand mehr den Zeugen des
  nächsten Blocks, und alle 30 s folgte der nächste Sprung.
- ⚠️ **Offen, zur Entscheidung**: Die Vorhaltung muss mindestens den
  Abstand zwischen Prüfpunkt und Spitze decken, auch bei grossen Zeugen
  (ein Block darf bis 48 MiB Zeuge haben). Vorschlag: Zeugenhalter legen
  die Zeugen der letzten zwei Prüfpunktabstände auf die Platte (Ring nach
  Höhe), im Speicher bleibt nur das Jüngste.

### v0.98.0 (2026-10-30): `--zeugen-bereithalten`

📌 Anlass aus dem Kurzlauf: Nur der Erzeuger hielt Zeugen. Als er am Ende
anhielt, lag der zustandslose Miner 136 Blöcke zurück und kam nie mehr
weiter.

- ⚑ **`--zeugen-bereithalten`**: Ein voller Knoten zeichnet beim Übernehmen
  auf wie ein Erzeuger, hält den Zeugen jedes Blocks und unterschreibt die
  Stücke mit seinem Konsensschlüssel; zustandslose Knoten bekommen sie auf
  Nachfrage. Gemessen kostet die Aufzeichnung rund 10 ms je Block bei einer
  Million Konten.
- ⛔️ **Wer direkt liefern darf**: ein Erzeuger aus dem Stimmsatz wie bisher,
  sonst nur ein **eingetragener Miner unter seinem eingetragenen
  Schlüssel**. Ein Wegwerfschlüssel kostet nichts, eine Eintragung schon.
  Weitergeleitet wird über das Gossip weiter nur, was ein Erzeuger
  unterschrieb.
- ⛔️ **Ein falscher Zeuge wirft keinen Block hinaus**: Trägt der Zeuge eines
  anderen als des Erzeugers nicht, wartet der Block weiter, und der
  Unterzeichner ist gesperrt (höchstens 64 gemerkt). Sicher war es schon
  vorher, denn ein Zeuge belegt sich über die Wurzel vor dem Block;
  ohne diese Sperre hätte aber jeder Miner einem zustandslosen Knoten
  Blöcke aus der Warteschlange werfen können.
- Tests `ein_voller_knoten_haelt_zeugen_bereit` (Alpha hält zurück, Beta
  folgt 100 Blöcke allein mit Gammas Zeugen, ohne leichten Abzug) und
  `ein_unbekannter_unterzeichner_liefert_keine_zeugen` (ein nicht
  eingetragener Gamma bringt Beta keinen Block weiter).

### v0.97.0 (2026-10-30): `--baum-auf-platte`

Punkt 1 der Festlegung vom 2026-10-29 (SHARED_TYPES 0.62.0, CONSENSUS
0.73.0).

- ⚑ **`--baum-auf-platte`**: Ein voller Knoten lagert den grossen
  Zustandsbaum in Seiten aus, neben der Kettendatei (`<kette>.baum`, sonst
  im Protokollverzeichnis); im Speicher bleibt der obere Baum. Der Knoten
  baut ihn beim Start neu (ein Zwischenspeicher). `groessen` trägt
  `baum_im_speicher` und `baum_seiten_bytes`.
- Test `ein_erzeuger_mit_dem_baum_auf_der_platte`: Alpha erzeugt mit
  ausgelagertem Baum, Gamma (voll, im Speicher) und Beta (zustandslos, mit
  den Zeugen aus dem ausgelagerten Baum) folgen mit derselben Wurzel.
- Alle Kisten grün (Knoten 439), Clippy sauber; im Testclient fielen
  `netzlauf` und `pod_ueber_knoten` im vollen Lauf unter Last aus und
  liefen allein grün.

### v0.96.0 (2026-10-30): Die Ausschüttung in Partitionen

Punkt 2 der Festlegung vom 2026-10-29 (nach Solanas SIMD-0118, geprüft
gegen Cosmos F1 und Bittensor [3]).

- ⚑ **Der Epochenwechsel merkt die Ausschüttung vor**
  (`epochenausschuettung_vorgemerkt`, TOKENOMICS 0.25.0), und **jeder Block
  bucht höchstens 4096 Vormerkungen** (`auszahlungen_buchen`, gleich nach
  dem Abschluss eines Wechsels, also auch im Wechselblock selbst). Bei
  wenigen Minern ist damit alles im selben Block bezahlt wie bisher; bei
  sehr vielen bleibt jeder Block und sein Zeuge begrenzt. `groessen` trägt
  `auszahlungen_offen`.
- Test `der_abzug_besteht_nur_gegen_seinen_anker` fälscht jetzt einen Wert
  in kanonischer Form (das letzte Byte des Zustands ist nun eine
  Längenangabe und liest sich verfälscht gar nicht mehr).
- Alle Kisten grün (Knoten 438), Clippy sauber.

### v0.95.0 (2026-10-30): Der jüngste Anker, Selbstheilung, keine stummen Antworten

Festlegung des Projektinhabers vom 2026-10-29: die drei offenen Punkte der
Skalierung umsetzen, nach Recherche und kritischer Prüfung. Dies ist Punkt 3
(nach dem Vorbild der Checkpoint-Synchronisation von Ethereum: einsteigen am
jüngsten bestätigten Punkt, abgeglichen mit mehreren Quellen).

- ⚑ **Der jüngste Anker**: Ein zustandsloser Knoten sammelt Zertifikate
  `ANKER_SAMMELN` (2 s) lang von mehreren Gegenstellen und nimmt das höchste,
  das gegen seinen Stimmsatz trägt; eines unter der eigenen Höhe zählt nicht.
  Vorher galt das erste, und ein gültiges, aber altes Zertifikat einer
  einzelnen Gegenstelle führte zu einem Abzug, den niemand mehr hat.
- ⚑ **Selbstheilung**: Steht ein zustandsloser Knoten `STILLSTAND_HOECHSTENS`
  (30 s) still, obwohl Blöcke mit belegtem Erzeuger darüber ankamen (etwa
  weil ein Erzeuger seine Zeugen zurückhält), sucht er einen jüngeren
  verankerten leichten Abzug und springt dorthin; die leichten Abzüge legen
  alle vollen Knoten ab, unabhängig vom Erzeuger. Derselbe Schutz wie beim
  Einstieg: ein Zertifikat von mehr als zwei Dritteln. `auf_abzug_setzen`
  erlaubt einem zustandslosen Knoten dafür, vorwärts neu aufzusetzen.
  Gemessen wird an der höchsten Höhe mit belegtem Erzeuger, nicht an
  Behauptungen von Gegenstellen (die leere Antworten senken).
- ⛔️ 📌 **Die neuen Nachforderungen antworten nie mit `Nichts`**: Antworten
  tragen keine Kennung, und ein `Nichts` wurde der laufenden Nachforderung
  an dieselbe Gegenstelle zugeschrieben. Ein voller Knoten ohne Zeugen
  antwortete auf Zeugenstücke mit `Nichts`, und der Zustandslose hielt ihn
  darauf für leer an Blöcken und Abzügen und sperrte ihn; ein Erzeuger, der
  Zeugen zurückhält, konnte so auch den Abzug sperren. Jetzt leere
  `Zeugenstuecke`, `Kontobeweise` mit Höhe 0 und
  `LeichterAnker(Option<…>)`. Während ein Abzug kommt, fragt der Knoten
  keine Zeugen nach.
- Test `ein_zustandsloser_knoten_heilt_sich_ueber_einen_juengeren_anker`:
  Alpha hält Zeugen über zwei Prüfpunkte zurück (Testschalter
  `zeugen_zurueckhalten`), Beta springt über einen leichten Abzug und folgt
  danach; dieselbe Wurzel. Sechs zustandslose Netztests zweimal grün, Clippy
  sauber.

### v0.94.0 (2026-10-29): Zustandslos im Probenetz, Zeugen in den Grössen

- **Probenetz**: Ein zustandsloser Knoten führt die zwölf Probekonten wie
  eigene (aus Zeugen und Kontobeweisen); sonst sähe er sie leer und käme
  über den Probehahn nie an Guthaben, Einsatz und Anmeldung. Der Probehahn
  liest sie über `Kette::konto` und nimmt nur bekannte.
- **`groessen`** trägt `zustandslos`, `zeugenlos`, `zeugenstuecke_bytes` und
  `eigene_unbekannt`, für die Auswertung im Podlauf.
- ⚠️ **Offen, für eine eigene Entscheidung**: Die Ausschüttung am
  Epochenwechsel berührt das Auszahlungskonto jedes Miners mit Arbeit. Bei
  sehr vielen Minern (über rund 50 000) wird der Zeuge dieses einen Blocks
  grösser als 48 MiB. Abhilfe wäre eine Auszahlungswarteschlange über
  mehrere Blöcke; sie verschiebt Auszahlungszeitpunkte.
- 📌 **Aus dem ersten Podlauf** (`kurz-zustandslos` bis `5`):
  - Der zustandslose Miner fragte Zeugen beim Weiterreicher (ein voller
    Knoten ohne Zeugen), beim Erzeuger nur, wenn er als Miner eingetragen
    ist, und bei den ersten vier Gegenstellen; der Erzeuger war nicht
    darunter, und Block 1 bekam nie seinen Zeugen. Jetzt fragt er auch, wer
    ihm zuletzt tragende Zeugenstücke brachte (`zeugen_quellen`).
  - Er holte mit rund 11 Blöcken je Sekunde nicht an eine Spitze mit 16
    auf: Nachgeforderte Blöcke fragen ihren Zeugen jetzt sofort
    (`zeugen_eilig`), die Pflege läuft auch nach jeder Blocklieferung, das
    Fenster ist 128 Blöcke, eine Nachforderung nennt bis 128 Stücke.
  - Protokollzeilen `zeuge_fertig`, `wartet_auf_zeuge`,
    `zeugenstueck_liegt` für die Auswertung.
- **Kurzlauf `kurz-zustandslos5` bestanden**: Der zustandslose Miner
  übernahm 7730 Blöcke mit Zeugen, sagte zu und bekam 40,0 Mio. auf sein
  Auszahlungskonto (die anderen im Mittel 39,5 Mio.), mit 0 Konten im
  Zustand; der späte zustandslose Einsteiger stieg über einen leichten
  Abzug bei 7500 (7668 Bytes) ein und war nach 5 s an der Spitze;
  `zeugenlos` 0, eine Wurzel.
- Test `ein_zustandsloser_knoten_steigt_ueber_einen_leichten_abzug_ein`
  wartet jetzt auf alle eigenen Konten (sie kommen in Bündeln zu acht).
  Clippy sauber.

### v0.93.0 (2026-10-29): Der leichte Abzug, Kontobeweise, keine Kettendatei für Zustandslose

- ⚑ **Leichte Abzüge an jedem Prüfpunkt** (alle 150 Blöcke): Ein voller
  Knoten mit Ablage legt neben die vollen `leicht-<höhe>.dat` ab (Register,
  Kopf, Wurzel des grossen Baums; im Test 752 Bytes), behält die jüngsten
  vier und merkt sich die zuletzt angewandten Prüfpunktzertifikate. So
  rechnet ein zustandsloser Neuling höchstens rund 150 Blöcke mit Zeugen
  nach; an Epochengrenzen allein wären es bis zu 1800, und so viele Zeugen
  hält niemand bereit.
- ⚑ **Der Anker** eines leichten Abzugs: der des Betreibers (`--abzug`),
  oder **ein Prüfpunktzertifikat**, das der Knoten gegen seinen Stimmsatz
  prüft (`Kette::anker_aus_zertifikat`; ohne Satz keiner). Eine
  Gegenstelle liefert nur Bytes. Ohne Anker und Satz (Probekette) folgt
  er ab Genesis, nach zehn Sekunden Suche.
- ⚑ **Kontobeweise**: Nach einem leichten Abzug kennt der Knoten seine
  eigenen Konten nicht; er fragt nach Beweisen, hält sie, bis seine Höhe
  ihre erreicht, und setzt sie nur ein, wenn sie gegen seine eigene Wurzel
  tragen. Ein Zeuge, der das Konto berührt, bringt es ebenso. Solange es
  unbekannt ist, sendet er keine eigenen Transaktionen (die Nummer läge
  sonst am Boden einer neuen Epoche, weit über der wirklichen).
- ⚑ **Ein zustandsloser Knoten führt keine Kettendatei**: Er könnte sie ohne
  Zeugen nicht nachrechnen. Nach einem Neustart steigt er über einen
  leichten Abzug ein.
- Nachforderungen `LeichterAnker`, `LeichterAbzug`, `Kontobeweise`
  (angehängt; gedrosselt wie Blöcke beziehungsweise Archiv).
- ⛔️ **Aus der Angriffsprüfung:**
  - Gehalten werden die niedrigsten Kontobeweise ab der eigenen Höhe,
    höchstens 64 voraus. Hielte er die höchsten, belegte eine erfundene
    ferne Höhe den Platz für immer.
  - Eine lernende Kette übernimmt den Erzeuger des nächsten Blocks auch
    nach einem Abzug in die Erzeugerliste (höchstens vier). Bisher nur vor
    Block 1, und nach einem Abzug wies der Netzfaden alle Zeugenstücke ab.
- **Restpunkt** (Lebendigkeit, nicht Sicherheit): Ein gültiges, aber altes
  Zertifikat führt zu einem Abzug, den niemand mehr hat, oder zu einem
  Abstand ohne Zeugen. Abhilfe später: kurz sammeln, den jüngsten nehmen.
- Tests:
  - `ein_zustandsloser_knoten_steigt_ueber_einen_leichten_abzug_ein`:
    Anker bei 150, leichter Abzug, Aufholen mit Zeugen, eigenes Konto per
    Kontobeweis, keine Kettendatei, dieselbe Wurzel.
  - `erfundene_kontobeweise_belegen_den_platz_nicht`.
  - `ein_anker_kommt_nur_aus_einem_tragenden_zertifikat`.
- 437 Tests grün (gesperrt, offline), Clippy sauber.

### v0.92.0 (2026-10-29): Die Rolle `--zustandslos` und die Zeugen im Netz

- ⚑ **`--zustandslos`** (Festlegung des Projektinhabers vom 2026-10-29:
  Rollen nach NEAR): Der Knoten lädt seine Kette wie jeder, wird dann
  zustandslos (die eigenen Konten bleiben: Netz-, Konto-, Miner- und
  Auszahlungskonto), übernimmt Blöcke nur mit Zeugen, erzeugt keine Blöcke
  (mit `--erzeuger` zusammen startet er nicht), führt keinen Mempool und
  abonniert keine Transaktionen (myl-net `Rolle`).
- ⚑ **Der Erzeuger verbreitet zu jedem Block den Zeugen** in unterschriebenen
  Stücken (CONSENSUS 0.69.0) auf `GossipTopic::Zeugen`; volle Knoten
  abonnieren es nicht. Er hält die Stücke bis 64 MB bereit
  (`Kette::zeugenstuecke`) und beantwortet **`Nachforderung::Zeugenstuecke`**
  gebündelt (bis 32 je Anfrage, so viele, wie in eine Antwort passen).
- ⚑ **Der Netzfaden leitet ein Zeugenstück nur weiter**, wenn es vollständig
  lesbar ist, zu seinem Kopf passt und ein bekannter Blockerzeuger den Kopf
  unterschrieben hat (`ProtokollValidator::zeugenstueck_traegt`, geteilte
  `Erzeugerliste` aus dem Stimmsatz; leer weist alles ab). Nachgeforderte
  Stücke gehen durch dieselbe Prüfung im Knoten.
- **Der zustandslose Empfang**: Ein Block mit belegtem Erzeuger wartet je
  Höhe auf seinen Zeugen (höchstens 32 voraus, der erste je Höhe gilt);
  der Sammler erwartet dann nur noch dessen Kopf. Fehlt ein Zeuge nach 1 s,
  fragt der Knoten gebündelt beim Weiterreicher, beim Erzeuger (wenn er als
  Miner eingetragen ist) und bei bis zu vier weiteren Gegenstellen nach.
- ⛔️ **Aus der Angriffsprüfung:**
  - Ein Block löst erst etwas aus (Nachforderung, Erwartung an einen
    Zeugen), wenn sein Erzeuger belegt ist; sonst könnten gefälschte hohe
    Blöcke Nachforderungen anstossen oder echte Zeugen verdrängen.
  - Eine lernende Kette (Probekette) übernimmt den Erzeuger in die Liste
    nur für Block 1 bei Höhe 0 und höchstens vier; sonst brächte jeder
    unterschriebene Block seinen Schlüssel hinein.
  - Nachforderungen werden nur aus schon Unterschriebenem bedient; sonst
    zwänge jede Anfrage den Erzeuger zu bis zu 32 BLS-Unterschriften.
- 📌 **Beim Bau gefunden**:
  - Das erste Zeugenstück kam vor dem Block an, und der Netzfaden kannte
    den Erzeuger noch nicht; dafür die Nachforderung.
  - Eine Verbindung trägt nur 16 Anfragen zugleich, und ein aufholender
    Knoten verlor die Hälfte; deshalb gebündelt.
  - Der Weiterreicher eines Blocks ist oft ein voller Knoten ohne Zeugen;
    deshalb weitere Gegenstellen.
- Tests in `tests/zustandslos.rs`:
  - `ein_zustandsloser_knoten_folgt_mit_zeugen`: drei Knoten, Beta
    zustandslos, Gamma voll ohne Zeugenstücke; dieselbe Wurzel, keine
    Konten bei Beta.
  - `ein_zustandsloser_knoten_holt_mit_zeugen_auf`: Beta kommt bei Höhe 25
    dazu.
  - `ein_gefaelschter_block_loest_beim_zustandslosen_nichts_aus`.
  - Dazu `zeugenstuecke_nur_von_bekannten_erzeugern`.
- 434 Tests grün, Clippy sauber.

### v0.91.0 (2026-10-29): Der zustandslose Knoten kennt seine eigenen Konten

- 📌 **Ohne eigene Konten keine Entlohnung**: Der Knoten liest sein Konto
  auch ausserhalb der Blöcke, für die Nummern seiner Transaktionen
  (Bündel, Festlegen und Aufdecken der Prüfaufträge, Schiedsstimmen,
  Differenzmeldungen), für Einsatz, Guthaben und Probehahn. Zustandslos
  stand dort ein leeres Konto; er hätte seine Arbeit mit falschen Nummern
  eingereicht und wäre nicht entlohnt worden.
- ⚑ **`zustandslos_machen(eigene)`** nimmt die eigenen Konten aus dem
  vollen Stand mit; nach jedem geprüften Block werden die fortgeschrieben,
  die der Zeuge abdeckt (aus geprüften Blöcken, also so verlässlich wie
  die Wurzel). **`Kette::konto`** liefert sie; der Knoten liest seine
  Konten nur noch darüber.
- **Mempool beim zustandslosen Knoten**: Fremde Nummern kann er nicht
  beurteilen und prüft sie nicht (das tun die vollen Knoten); die eigenen
  schon. `mempool_bereinigen` liest je Absender einmal statt je
  Transaktion.
- Test `die_entlohnung_rechnet_auch_der_zustandslose_folger_nach`: sechs
  Miner mit Auszahlungskonto, ein Pod, ein Bündel, Burns, Rückhalt nach
  der Anlaufregel und Freigabe; der zustandslose Folger kommt zur selben
  Wurzel, sieht den Lohn auf seinen eigenen Konten, jeder Block war
  bezeugbar. `ein_zustandsloser_folger_rechnet_mit_zeugen_mit` prüft
  zusätzlich Nummer und Guthaben der eigenen Konten.

### v0.90.0 (2026-10-29): Der Zeuge je Block, der zustandslose Folger

- ⚑ **Der Erzeuger hält zu jedem Block den Zeugen bereit** (zustandslose
  Validierung, CONSENSUS 0.68.0): `baue_block` zeichnet auf, was das
  Anwenden an grossen Sammlungen berührt, und legt den Zeugen (Borsh) zum
  Blockhash, zusammen höchstens `ZEUGEN_HOECHSTENS_BYTES` (64 MB);
  `eigener_zeuge`. **`zeugenlos`** zählt Blöcke ohne Zeugen (ein Zugriff
  über eine ganze grosse Sammlung im Blockpfad): null, sonst ein Fund.
- ⚑ **`zustandslos_machen` und `uebernimm_mit_zeuge`**: Ein zustandsloser
  Knoten hält keine Konten, Sitzungen, Manifeste und Hinterlegungen und
  übernimmt einen Block nur mit Zeugen; trägt er nicht (fremde Wurzel,
  falscher Eintrag, ein Zugriff daneben), bleibt der Zustand, wie er war:
  `KettenFehler::ZeugeTraegtNicht`, im Protokoll `zeuge-traegt-nicht`.
  Er erzeugt keine Blöcke und legt keine Abzüge an. Der Transport im Netz
  und die Rolle im Knoten folgen.
- ⚑ **Abgelaufene Sitzungen aus den Ablauffächern**; die Hinterlegung geht
  für genau die weggefallenen zurück. 📌 Bis hierher suchte der
  Epochenwechsel über alle Hinterlegungen.
- **Leere Konten fallen am Ende jedes Blocks weg**
  (`leere_konten_entfernen`), mit dem Nonce-Boden.
- Test `ein_zustandsloser_folger_rechnet_mit_zeugen_mit` (über die
  Epochengrenze, `zeugenlos` null; ohne Zeugen und mit unvollständigem
  wird abgewiesen, nichts bewegt sich). 429 Tests, davon 428 grün; die
  mDNS-Probe `zwei_knoten_ohne_angabe_finden_einander_im_lokalen_netz`
  fiel im vollen Lauf und lief allein grün (Umgebung, bekannt). Clippy
  sauber.

### v0.89.0 (2026-10-29): Der Hauptfaden bleibt frei, nichts verfällt im Rückstau

- **Die gebündelte Relaisleitung prüft in einer Rechenaufgabe**; aufgenommen
  und beantwortet wird danach in der Schleife (`Weckruf::RelaisGeprueft`).
  Im Hauptfaden hielt der Stapel bei 3000 je Sekunde Blockbau und Gossip
  auf.
- 📌 **`WARTEN_HOECHSTENS_HOEHEN` 64 auf 256**: Unter Rückstau verfielen
  angenommene Transaktionen nach knapp 4 s (`durchsatz4`: 97 bei 3000/s),
  bevor der volle Mempool Gegendruck gab; Unsinn begrenzt die Platzzahl.
- 428 Tests grün, Clippy sauber.

### v0.88.0 (2026-10-29): Der Speicher der geprüften Unterschriften

- **`Kette::aufnehmen_geprueft`** nimmt eine `GepruefteTransaktion` (es
  gibt sie nur durch eine Prüfung) und merkt sich ihre Kennung; der Block
  prüft sie nicht noch einmal (CONSENSUS 0.67.0). Höchstens doppelt so
  viele Kennungen, wie der Mempool Plätze hat; die ältesten fallen zuerst.
  Gossip, gebündelte und einzelne Relaisleitung gehen darüber.
- 428 Tests grün, Clippy sauber.

### v0.87.0 (2026-10-29): Je Block nur das Geänderte

- ⚑ **Die Wurzel je Block über den Zustandsbaum** (CONSENSUS 0.66.0):
  `baue_block` und `uebernimm` schreiben sie fort; `uebernimm` rechnet auf
  dem Zustand selbst, gesichert statt kopiert, und spielt bei falscher
  Wurzel zurück. Die Protokollzeilen je Block nehmen `Kette::wurzel`
  (bis hierher rechneten sie die volle Wurzel bei jedem Block neu).
  `zustandswurzel()` bleibt die volle Rechnung für Tests und seltene
  Prüfungen.
- 📌 **Zwei volle Kopien des Zustands je Durchlauf der Miner-Schleife**
  (`dateien_nachholen`, für die Zuteilung der nächsten Epoche) sind
  durch einen Blick unter deren Epoche und Saat ersetzt.
- 📌 **Der Abzug** prüfte über SHA-256 der Bytes, was die alte Wurzel war;
  jetzt gelesen, gerechnet und auf dieselbe Form beim Zurückschreiben
  geprüft.
- 428 Tests grün, Clippy sauber.

### v0.86.0 (2026-10-29): Ein Mempool, der Andrang aushält

- ⚑ **Erst geprüft, dann im Mempool** (Skalierung, Festlegung des
  Projektinhabers vom 2026-10-29): Transaktionen aus dem Gossip kamen
  bisher ungeprüft in den Mempool, und ungültige verdrängten gültige.
  Jetzt sammelt der Knoten sie bis 20 ms oder 256 Stück und prüft sie als
  Stapel in einer Rechenaufgabe; die Hauptschleife wacht für das Ergebnis
  auf (`Weckruf::Geprueft`).
- ⚑ **Der Mempool** als eigene Struktur mit Zählern je Absender und für
  Neulinge (die Aufnahme zählte bisher jedes Mal über alle Wartenden),
  Bytes und Neulingsstand je Eintrag einmal bei der Aufnahme, 16 384 Plätze
  statt 4 096. **Verdrängt wird nach dem Preis, gewählt nach Eingang**
  (kritisch geprüft: Ein Trinkgeld gibt es nicht, eine Reihenfolge nach
  Gebot belohnte nur hohe Obergrenzen): Ist er voll, fallen die
  billigsten Enden der Nummernketten, die weniger bieten als die neue
  Transaktion, nie eine aus der Mitte. Test
  `ein_voller_mempool_verdraengt_das_billigste_ende`.
- **Gezählt statt je Zeile**: Transaktionen stehen bis 100 je Sekunde
  einzeln im Protokoll, darüber in der Sammelzeile `tx_zaehler`
  (`tx_aufgenommen` entfällt). Bei 2000/s schrieb jeder Knoten zuvor rund
  150 000 Zeilen in sieben Minuten.
- 428 Tests grün, Clippy sauber.

### v0.85.0 (2026-10-29): Gebündelte Annahme über die Relaisleitung

- ⚑ **`Relaisfrage::Transaktionen`** (SHARED_TYPES 0.58.0): Die
  Unterschriften werden als Stapel geprüft (`pruefe_alle`, gemessen rund
  90 statt 470 µs je Stück), jede gültige wird verbreitet, die Antwort
  nennt, wie viele der Mempool nahm. 📌 Einzeln (eine Verbindung und eine
  Einzelprüfung je Transaktion im Hauptfaden) war die Leitung der Engpass:
  Im Nachtlauf vom 2026-10-29 kam keine Laststufe über rund 350 je
  Sekunde; gemessen war die Leitung, nicht das Netz.
- 427 Tests grün, Clippy sauber.

### v0.84.0 (2026-10-29): Einsatz und Rückbuchung

- ⚑ **`--einsatz <betrag>`**: Ein Miner hinterlegt nach der Anmeldung
  selbst so viel Einsatz, sobald sein Guthaben ihn trägt (der Probehahn
  plant ihn ein). Bis hierher gab es im Betrieb keinen Weg dazu, nur in
  Tests, und ohne Einsatz schlachtet ein Schiedsurteil nichts
  (`gezielt19` und `gezielt21`: `NoStake`). Protokoll `einsatz_hinterlegt`.
  Der Stimmsatz des laufenden Knotens hängt nicht am Einsatz.
- ⚑ **Die Rückbuchung** (CONSENSUS 0.65.0): Am Wechsel werden die Anteile
  uneiniger Paare als Vorbehalt abgelegt, ein Urteil „Pod falsch“
  entscheidet ihn (auch wenn die Schlachtung mangels Einsatz scheitert),
  und freie Vorbehalte gehen in die Ausschüttung. `aufnahme` nennt
  `rueckbuchungen`.
- ⚠️ `zwei_knoten_ohne_angabe_finden_einander_im_lokalen_netz` (mDNS) fällt
  auf dieser Maschine gelegentlich, auch einzeln (einer von drei); die
  übrigen 426 Tests grün, Clippy sauber.

### v0.83.1 (2026-10-29): Spuren nach dem Wechsel werden angenommen

- 📌 **Fund 680, Folge**: Seit das Podwerk erst einreicht und dann die
  Spuren ablegt (COMPUTE_PIPELINE 0.56.0), kommen sie oft nach dem
  Wechsel an, und die Ablage scheiterte mit „diesen Pod kennt die
  Zuteilung nicht“ (`gezielt20`). `Kette::pod_der_kennung` sieht jetzt
  zuerst in der aufgehobenen Zuteilung der abgelaufenen Epoche nach, die
  bis zur Ziehung steht. Probe in `bezeugte_arbeit_erreicht_ein_konto`.

### v0.83.0 (2026-10-29): Uneinige Zwillinge werden genau geprüft

- ⚑ **Uneinige Paare am Wechsel** (Whitepaper 6.4, Festlegung des
  Projektinhabers vom 2026-10-29): Die Zwillingsabrechnung vermerkt Paare
  mit zwei verschiedenen Bündeln (`verschieden`); die Kette legt sie als
  `Uneinigkeit` ab. **`Anweisung::DifferenzMelden`** legt daraus die
  Prüfaufträge an (CONSENSUS 0.64.0).
- **Der Weg des Beweises**: Der Ortsdienst schickt ihn
  (`Relaisfrage::DifferenzMelden`, myl-pod 0.48.0); der Knoten reicht ihn
  an die Koordinatoren beider Pods (`Nachforderung::Differenz`, Marke 18),
  und jeder meldet ihn mit seinem Schlüssel; die Kette nimmt den ersten.
  Schweigt der eine Koordinator, meldet der andere. Protokoll
  `differenz_gemeldet`.
- ⚑ **Der Checker richtet sich nach den Aufträgen im Zustand**, nicht nach
  der letzten Ziehung (ein Differenzbeweis legt Aufträge auch ausserhalb
  an), fragt die Besetzung des Auftrags und prüft gegen die Wurzel des
  Auftrags.
- 427 Tests grün, Clippy sauber.

### v0.82.0 (2026-10-29): Die Selbstprobe und die Grössen je Epoche

- ⚑ **Die Selbstprobe** (Fund 677, Festlegung des Projektinhabers vom
  2026-10-29): Einmal je Epoche, in ihrer zweiten Hälfte (die Mitglieder
  messen am Anfang, der Shard hat einen Platz für Zeitproben), misst der
  Knoten seinen eigenen Shard mit einer Zeitprobe in der Grösse, die die
  Mitglieder stellen. Braucht er je Token mehr als das Anderthalbfache
  seiner Zusage (`frist::selbstprobe_zu_langsam`), reicht er für die
  nächste Epoche eine langsamere ein (`frist::angepasste_ebenenzeit`: die
  gemessene Zeit mit einem Viertel Luft); schneller wird sie nie von
  selbst. Protokoll `selbstprobe`, mit Warnung. Whitepaper: Zusage und
  Frist stehen dort nicht (Festlegung vom 2026-10-22); die Reserve (6.8)
  bleibt unberührt. Test
  `die_selbstprobe_verlangsamt_erst_ueber_dem_anderthalbfachen`.
- **Die Grössen je Epoche** (Fund 677, um das Wachstum von rund 2,3 MB je
  Epoche im Nachtlauf zu finden): `Kette::groessen` (Bytes des Zustands,
  den jeder Block ganz hasht, und die Einträge jeder Sammlung) und die
  eigenen Sammlungen des Knotens, als Protokollzeile `groessen`.
- Der Nachrechner prüft jetzt als Prefill (VERIFICATION 0.27.0).
- 427 Tests grün, Clippy sauber.

### v0.81.0 (2026-10-29): Die Schiedsrunde und das Ende des Schlachtens ohne Beweis

- ⛔️ **Fund 679: `SchuldspruchEinreichen` schlachtete ohne Beweis der
  falschen Rechnung.** Der Beleg war die Unterschrift des Beschuldigten
  über einen Übergang; er belegte, wen es trifft, und setzte voraus, dass
  falsch gerechnet wurde. Jeder, der mit einem Miner gerechnet hatte,
  besass eine solche Unterschrift und konnte einen Ehrlichen schlachten
  lassen (`tests/urteilsweg.rs` zeigte genau das als Erfolg). Die
  Anweisung bleibt in der Kodierung und wirkt nicht mehr; der Test belegt
  jetzt das Gegenteil.
- ⚑ **Geschlachtet wird über die Schiedsrunde** (Whitepaper 6.6,
  Festlegung des Projektinhabers vom 2026-10-29): Eine aufgedeckte
  Abweichung öffnet einen Streitfall (CONSENSUS 0.63.0). Jeder Validator
  im Stimmsatz holt beim Anfechter den Schiedsbeleg
  (`Nachforderung::Schiedsbeleg`, Marke 17: Verlauf mit Beweisen und die
  Eingänge der strittigen Layer), prüft ihn gegen die Wurzel auf der
  Kette, rechnet die Layer nach (`myl_verifier::schied_nachrechnen`) und
  stimmt ab. Ohne Beleg bis zur Hälfte des Fensters stimmt er gegen die
  Anfechtung. Die Kette entscheidet mit mehr als zwei Dritteln und bucht
  nach der Matrix (`schiedsurteil_buchen`): der Beschuldigte verliert 100 %
  seines Einsatzes, der Anfechter bekommt das Kopfgeld; oder der Anfechter
  verliert seine Kaution. Fenster eine Epoche (`SCHIED_FENSTER_BLOECKE`).
- Der Anfechter berechnet seinen Beleg gleich beim Nachrechnen und hält
  ihn bereit. Protokoll: `schiedsbeleg_angefragt`,
  `schiedsbeleg_eingegangen`, `schiedsstimme`, beim Erzeuger
  `streitfall_entschieden`.
- Damit ist auch die Lücke „Grenzen beim Beschuldigen nach dem laufenden
  Stand“ geschlossen: Der Beschuldigte kommt aus der Besetzung der
  gezogenen Epoche.
- ⚠️ Ein Beleg muss in eine Antwort passen (4 MiB): beim 0,6B-Modell rund
  tausend Positionen. Längere Sitzungen und grössere Modelle brauchen
  einen Beleg in Stücken.
- Test `ein_streitfall_schlachtet_den_beschuldigten_und_zahlt_dem_anfechter_kopfgeld`:
  Abweichung aufgedeckt, Streitfall gegen das Mitglied mit Layer 0, eine
  Stimme ausserhalb des Stimmsatzes zählt nicht, die berechtigte
  entscheidet, 100 % des Einsatzes geschlachtet, 30 % Kopfgeld beim
  Anfechter, der Folger auf derselben Wurzel. 426 Tests grün, Clippy
  sauber.

### v0.80.0 (2026-10-29): Zwei Checker, Festlegen vor Aufdecken, der Checker-Pool

- ⚑ **Die Checker stehen in der Kette** (Festlegung des Projektinhabers vom
  2026-10-29, Fragen 1, 2 und 4): Der Epochenwechsel legt die Kandidaten
  ab (`Kette::pruefkandidaten`: Miner ohne Pod, je Pod der Ersatz aus den
  übrigen Zwillingsgruppen), die Ziehung legt je Segment einen Prüfauftrag
  mit zwei Checkern an, Fenster je eine Viertelepoche
  (`PRUEF_FENSTER_BLOECKE`). `pruefer_der_stichprobe` kommt aus dem
  Zustand; die eigene Wahl im Knoten ist entfallen.
- **Der Ablauf im Knoten**: nachrechnen, den Befund samt Logits festlegen
  (frisches Salz), aufdecken, sobald die Kette es erlaubt, beides gebündelt.
  Protokoll: `pruefung_festgelegt`, `pruefung_aufgedeckt`,
  `pruefung_abgeschlossen` (beim Erzeuger jeder Auftrag, sonst die
  eigenen).
- **Der Checker-Pool zahlt am Wechsel** nach `pruefvolumen`
  (TOKENOMICS 0.24.0).
- Test `ein_checker_ohne_pod_legt_fest_deckt_auf_und_wird_bezahlt`: sieben
  Miner, einer ohne Pod; er legt fest, deckt auf, der Auftrag schliesst
  einig ab, und der Pool bezahlt ihn am nächsten Wechsel; ein Folger kommt
  auf dieselbe Wurzel. 425 Tests grün, Clippy sauber.
- ⚠️ Die Zustandswurzel ändert sich (CONSENSUS 0.62.0).

### v0.79.0 (2026-10-29): Der Checker aus der Bereitschaft rechnet nach

- ⚑ **Zuteilung des Checkers** (Whitepaper 6.8, Festlegung des
  Projektinhabers vom 2026-10-27): je Ziehung der Stichprobe ein Miner ohne
  Pod der gezogenen Epoche, darunter der Pod in Bereitschaft, über
  `SHA-256("myelith-pruefer" ‖ Epoche ‖ Pod ‖ Index)` modulo der geordneten
  Kandidaten (`pruefer_waehlen`, `Kette::pruefer_der_stichprobe`). Pod und
  Index sind schon die Ausgabe der Saat aus dem Zertifikat; es gibt keinen
  zweiten Spielraum. Test `der_pruefer_ist_ein_kandidat_und_fuer_alle_derselbe`.
- **Der Ablauf im Knoten**: Der zugeteilte Checker fragt die Mitglieder des
  Pods nach dem Verlauf (`Nachforderung::Verlauf`, angehängt, Marke 16),
  prüft die erste Antwort mit `verlauf_pruefen` gegen die Spurwurzel des
  Zertifikats, lädt das Netzmodell bei Bedarf einmal (eingeblendet, also
  mit den Shards geteilt) und rechnet in einer eigenen Rechenaufgabe nach
  (`myl_verifier::verlauf_nachrechnen`). Protokoll: `pruefer_zugeteilt`,
  `verlaufanfrage_eingegangen`, `verlauf_abgewiesen`,
  `stichprobe_nachgerechnet` mit Ergebnis, Position, Layer und dem Mitglied,
  dessen Bereich die Layer enthält.
- 📌 **Jeder Knoten merkt sich die Ziehung**, nicht nur der Erzeuger:
  `Kette::uebernimm` warf sie bisher weg, und im ersten gezielten Lauf
  wurde viermal gezogen und kein Checker erfuhr von seiner Zuteilung. Beide
  Wege gehen jetzt über `ziehung_merken` (dazu `ablaufende_zuteilung`,
  `zuteilung_aufheben`); `bezeugte_arbeit_erreicht_ein_konto` führt einen
  Folger mit, der dieselbe Ziehung, dieselben Checker und denselben Pod
  kennen muss. Die Spuranfragen verschickt weiter nur der Erzeuger.
- `stichprobe_gefragt` nennt die Checker der Ziehung (`pruefer`, als
  Netzadressen); leer heisst: kein Miner ohne Pod in der gezogenen Epoche.
- `ablage_pruefen` weist eine Ablage ab, deren Tokens nicht zu ihren
  Einträgen passen.
- Merkmal `pruefnetz` (reicht an `integer-llm-runtime` durch) für die
  Netzproben mit dem kleinen Modell.
- ⚠️ **Noch ohne Folgen auf der Kette**: Eine Abweichung steht nur im
  Protokoll; eingereicht wird nichts. Die Grenzen beim Beschuldigten kommen
  aus dem laufenden Stand, nicht aus der gezogenen Epoche; eine Sitzung über
  einen Epochenwechsel ergibt eine Lücke.
- 424 Tests grün (`zwei_knoten_ohne_angabe_finden_einander_im_lokalen_netz`
  fiel einmal unter Last und lief danach dreimal durch).

### v0.78.0 (2026-10-28): Der Probehahn trocknet nicht mehr aus

- 📌 **Fund 675: Die Probekonten werden am Epochenwechsel nachgefüllt.**
  Sie sind der Vorrat des Probehahns, und nichts füllte sie nach. Der
  Testverkehr zahlt seine Gebühren aus ihnen, rund einmal je Sekunde und
  Knoten; im Nachtlauf mit 30 Knoten auf zwölf Konten waren alle nach etwa
  40 Minuten leer. Danach hatte die Lastphase nichts mehr zum Ausstatten
  (ab 400/s null aufgenommen), und der späte Einsteiger fand 452 mal einen
  leeren Hahn und kam nie zur Zusage. Jetzt füllt `Kette::probepraegung`
  ein Probekonto unter der Hälfte wieder auf `PROBEGUTHABEN` auf, nur auf
  der Probekette (wo die Probeprägung nicht null ist). Erst unter der
  Hälfte, damit kleine Abgänge wie eine Hinterlegung in der Rechnung
  bleiben. Test `der_wechsel_fuellt_ein_leeres_probekonto_auf`, dazu die
  Gegenprobe im Testnetz.
- ⚠️ Die Zustandswurzel der Probekette ändert sich damit; eine Kette einer
  älteren Fassung lässt sich nicht weiterführen.

### v0.77.0 (2026-10-28): Stufe 1, vergütet wird nur, was zwei Pods gleich rechnen

- ⚑ **Die Kette vergütet Inferenz nach Zwillingspaaren** (Whitepaper 3.4,
  3.5.2 und 5.3; Festlegung des Projektinhabers vom 2026-10-28).
  `Kette::zwillinge_der_laufenden_epoche` bildet die Paare aus den Pods mit
  Schnitt (`myl_scheduler::zwillingspaare`, CONSENSUS 0.61.0). Am
  Epochenwechsel (`nach_zwillingen`): Haben beide ein gültig unterschriebenes
  Bündel mit derselben Ausgabenwurzel (nicht null) und derselben
  Segmentzahl (`zwillinge_einig`), bekommt jeder die Hälfte seiner vTFE
  (Redundanz-Normierung); uneinig oder ohne Bündel des Zwillings keiner
  (ein Fall für die Stichprobe, die weiter aus allen gültigen Bündeln
  zieht). ⚠️ Ein Pod ohne Zwilling (ein Netz mit einem Pod, oder ein
  übriger) wird wie bisher allein vergütet. Vorher bezahlte die Kette das
  unterschriebene Bündel eines Pods; ein Pod, in dem alle logen, wurde
  bezahlt.
- **Das Podbild der Relaisleitung nennt den Zwilling**; der Ortsdienst rechnet
  damit jeden Auftrag auch über ihn (COMPUTE_PIPELINE 0.54.0).
- **Zähler** `zwillinge_einig`, `zwillinge_uneinig` im Zustand, in der
  Zeile `aufnahme` und als `myelith_zwillinge_einig_total`,
  `myelith_zwillinge_uneinig_total`; dazu `credit_preis` und
  `myelith_credit_preis`.
- Probe `zwillinge_werden_nur_einig_vergutet` mit Gegenproben (andere
  Wurzel, andere Segmentzahl, Wurzel null, ein Bündel fehlt: keiner;
  keiner hat etwas: nichts gezählt; ohne Zwilling: voll).
- ⚑ **Die Reserve übernimmt eine ausgefallene Position** (Whitepaper 6.8,
  Festlegung des Projektinhabers vom 2026-10-28, wie Petals: der Weg ersetzt
  sofort). Neue Nachforderung `PositionUebernehmen` (Marke 14) mit Antwort
  `Nachlieferung::Uebernahme` (Marke 13), über die Relaisleitung
  (`Relaisfrage::Uebernahme`) an das Reservemitglied. Es prüft gegen die
  Kette (`shardfuehrung::uebernahme_pruefen`: laufende Epoche, Reserve
  dieses Pods, Position vorhanden, Zusage hält die Ebenen) und rechnet die
  Position bis zum Ende der Epoche (`uebernahme`, Protokollzeile
  `position_uebernommen`). Die Kette nimmt für eine Position die
  Unterschrift des Vertreters an (`unterzeichner_des_buendels` mit
  `PoIBundle.vertreter`: nur ein Reservemitglied dieses Pods, jede Position
  und jeder Vertreter höchstens einmal), lässt ihn für die erste Position
  einreichen und vergütet die Position an ihn. ⚠️ **Abweichung von der
  Festlegung:** vergütet wird je Bündel an den, der für die Position
  unterschreibt, nicht je Segment; dafür braucht es Unterschriften je
  Segment (mit dem Nachrechnen). Proben `die_reserve_vertritt_eine_position`
  und `nur_die_reserve_uebernimmt_eine_position` (je mit Gegenproben),
  Markenprobe erweitert.
- ⚑ **Die Stichprobe bekommt eine Spur** (Stufe 2, Whitepaper 3.5.2 und
  6.4). Jedes Mitglied hebt die Spuren des eigenen Pods auf (`spurablagen`,
  höchstens 16), abgelegt über die neue Nachforderung `SpurenAblegen`
  (Marke 15, Antwort `Nachlieferung::Abgelegt`, Marke 15) und
  `Relaisfrage::SpurenAblegen`. ⚑ Angenommen wird nur, was zum Knoten
  gehört (`shardfuehrung::ablage_pruefen`: Mitglied des Pods, Position oder
  Reserve, mit Einträgen, deren Zeugnisse die genannte Wurzel ergeben);
  sonst Protokollzeile `spurablage_abgewiesen`. Eine Spuranfrage beantwortet
  der Knoten aus der Ablage, deren Wurzel die des Bündels in der Kette ist
  (`stichprobe_wurzeln`, CONSENSUS 0.61.0), mit Zeugnis, Beweis und Spur
  (`Nachlieferung::Spur`, Marke 14). Der Erzeuger prüft die Lieferung mit
  `myl_types::lieferung_pruefen` und schreibt `spur_geprueft` mit dem
  Ergebnis (`traegt`, `ohne Wurzel` oder der Fehler). ⚠️ Nachgerechnet
  wird noch nicht; dafür fehlt die Eingabe des Segments. ⚠️ Wer ablegt, ist
  nicht unterschrieben: Ein Mitglied desselben Pods kann eine erfundene
  Ablage danebenlegen, zählt beim Antworten aber nicht, weil nur die Wurzel
  aus der Kette gilt. Probe `nur_ein_mitglied_legt_eine_stimmige_spur_ab`
  mit Gegenproben (kein Mitglied, falsche Wurzel, ein Eintrag fehlt, leer),
  Markenprobe erweitert.
- ⚑ **Zwillingsgruppen und Anlaufregel** (Whitepaper 3.4 und 3.5.2,
  Festlegung des Projektinhabers vom 2026-10-28). Die Kette vergütet nach
  `myl_scheduler::zwillingsgruppen` (CONSENSUS 0.61.0). ⚑ Ein übriger Pod
  steht in **Bereitschaft**: Sein Bündel wird weder vergütet noch
  zurückgehalten (Festlegung des Projektinhabers: statt einer
  Dreiergruppe; später Checker). ⚑ **Ein Pod allein** (gar kein Paar, also
  ein Netz mit einem Pod) wird nicht mehr sofort vergütet: Seine Gutschrift wartet bis nach der Streitfrist
  (`SPERRFRIST_EPOCHEN`) in `zurueckgehalten` und kommt dann in die
  Ausschüttung jener Epoche (`zurueckhalten_und_freigeben`), und jedes
  seiner Segmente wird gezogen, höchstens `VOLL_GEZOGEN_HOECHSTENS` = 64
  (`voll_ziehen`). Vorher wurde er voll und sofort bezahlt, ohne Vergleich;
  im Lauf saß der Fälscher einmal allein in einem Pod und wurde bezahlt.
  ⚠️ Abweichung vom Whitepaper, nur für den Anlauf eines kleinen Netzes:
  r = 1 mit voller Stufe 2. Bis zum Nachrechnen hält die Regel die Zahlung
  zurück und verlangt die Spuren mit Beweis. Die Zeile `aufnahme` trägt
  `zurueckgehalten`. Proben `zwillinge_werden_nur_einig_vergutet` (ein Pod
  allein: zurück; einer in Bereitschaft, der trotzdem einreicht: weder Lohn
  noch Rückhalt) und
  `ein_pod_allein_wird_zurueckgehalten_und_voll_gezogen` (Gegenproben: eine
  Epoche vor der Freigabe nichts, Nullen nicht gehalten, Deckel der
  Ziehung).
- ⚑ **Der Pod in Bereitschaft kommt nicht in die Zuteilung**
  (`zuteilung_der_laufenden_epoche`): Seine Miner stehen bei `ohne_pod`.
  So startet er keine Shards, wird von der Zeitprobe nicht gemessen (sonst
  sperrte sie ihn als Verweigerer) und reicht nichts ein, an jeder Stelle
  gleich. Die Paare der übrigen bleiben dieselben, weil die Paarbildung ihn
  nie wählte. Probe `die_zuteilung_aendert_sich_waehrend_ihrer_epoche_nicht`
  angepasst: achtzehn Miner ergeben zwei rechnende Pods und sechs in
  Bereitschaft. `bezeugte_arbeit_erreicht_ein_konto` folgt der
  Anlaufregel (ein Pod allein: zurückgehalten, voll gezogen).
- 📌 **Fund 674: Nicht erreicht ist keine Weigerung.** Ein Knoten hält
  höchstens 16 ausgehende Verbindungen (`myl_net::MAX_AUSGEHEND`); sind sie
  belegt, scheitert das Wählen zu einem Pod-Mitglied, mit dem er noch nicht
  verbunden ist („Failed to dial the requested peer“). Die Zeitprobe zählte
  das seit Fund 660 wie eine Abweisung: Im ersten Lauf mit 29 Knoten (vier
  Pods) bekamen fünf ehrliche Miner sieben Zeugnisse; mit 13 Knoten war
  jeder mit jedem verbunden, also fiel es nie auf. Jetzt fragt die Messung
  nach einem Wählfehler (`ist_waehlfehler`) in 4 bis 8 s neu, ohne zu
  zählen (`messung_unerreicht`, Protokollzeile `zeitprobe_unerreicht`).
  ⚠️ Restlücke: Wer sich für Messende unerreichbar macht, wird nicht
  gemessen; sein Pod erreicht ihn dann aber auch nicht, und er wird nicht
  vergütet. ⚑ **Abhilfe gebaut (N1):** `vorrang_pflegen` setzt je Epoche
  den Vorrang in den Verbindungsgrenzen (NETWORKING 0.22.0) für die
  Mitglieder des eigenen Pods, ein Torknoten mit Relaisleitung dazu für die
  aller Pods (Protokollzeile `vorrang`). Ein Wählfehler zählt weiter nicht
  als Weigerung; das bleibt, bis der Vorrang im Netz belegt ist.
  Probe `nicht_erreicht_ist_keine_weigerung` (zehn Wählfehler: kein
  Verdacht, die Messung bleibt; Gegenproben: Zeitüberschreitung und
  geschlossene Verbindung sind keine Wählfehler, eine Abweisung in
  derselben Lage endet im Verdacht; der Text von libp2p festgehalten).
- Die Proben der Abweisung (`abweisung_tests`) räumen ihren Ordner unter
  dem Zwischenordner nach dem Bestehen auf; bisher blieb je Lauf einer
  liegen.
- ⚠️ Zustand, Bündel und Podbild ändern ihr Format; eine Kette einer älteren
  Fassung lässt sich nicht weiterführen.
- `myl-node` 0.77.0. 420 Tests grün, Clippy sauber; mit den neuen Typen
  geprüft: `myl-types` 335, `myl-scheduler` 132, `myl-ledger` 118,
  `myl-consensus` 285, `myl-tokenomics` 243, `myl-net` 120, `myl-verifier`
  122, `myl-governance` 96, `myl-pod` 166, `myl-gateway` 55, `myl-client`
  208, `myl-testclient` 299, `myl-simulation` 6.

### v0.76.0 (2026-10-28): Eine Verweigerung der Zeitprobe zählt; die Messplanung im Protokoll

- 📌 **Fund 660 behoben: Wer jede Zeitprobe abwies, wurde nie gemessen.**
  Ein `Nichts` oder eine gescheiterte Anfrage warf die Messung samt ihrer
  beantworteten Proben weg, und bei der nächsten Aufnahme begann eine
  neue. ⚑ Festlegung des Projektinhabers vom 2026-10-28: Eine Abweisung
  durch einen Positionsinhaber zählt wie eine Überschreitung, mit
  denselben zwei Fenstern und derselben Mehrheit; nach einer Abweisung
  fragt der Messende mit Abstand erneut. Jetzt bleibt die Messung stehen,
  fragt nach 2 bis 4 s, danach nach 4 bis 8 s mit neuer Saat erneut
  (`frist::abstand_nach_abweisung`), und endet erst als Überschreitung,
  wenn mindestens vier Abweisungen in Folge zusammen länger dauern als
  dreimal so lange, wie alle Messenden des Pods je eine Probe brauchen
  (`frist::nach_abweisung`, `frist::abweisungsdauer`). Ohne beantwortete
  Probe zählt sie, als wäre die Probe in ihre Grenze gelaufen. Eine Antwort
  setzt die Folge zurück.
- ⚑ **Eine bestandene Probe beendet die Messung** (`frist::messung_fertig`):
  Hält die kürzeste Probe schon die Frist, können weitere sie nur
  verkürzen; das Urteil bleibt dasselbe, und der einzige Platz für
  Zeitproben des Gemessenen wird früher frei.
- **Warum diese Zahlen, simuliert statt geschätzt** (Probe
  `ehrliches_gedraenge_ist_keine_verweigerung`): Ein Ehrlicher weist ab,
  wenn sein Platz belegt ist, und zu Beginn des Fensters fragen alle
  zugleich. Mit vier Abweisungen ohne Mindestdauer endeten bei fünf
  Messenden rund 18 % der Runden mit einer falschen Überschreitung, bei
  sieben über 80 %; mit der einfachen Dauer bis zu 80 % (5 bis 23
  Messende); mit der dreifachen keine von 72 000 (4 bis 23 Messende,
  Startstreuung 2 s und 30 s, Weg 50 bis 600 ms). Gegenproben in der Probe:
  einfache Dauer und fünf Proben statt des frühen Endes werden rot; ein
  Verweigerer erreicht bei jedem Messenden die Überschreitung. Probe
  `eine_lange_verweigerung_ist_eine_ueberschreitung` mit Gegenproben (eine
  Abweisung zu wenig, eine Mikrosekunde zu kurz), Probe
  `eine_bestandene_probe_beendet_die_messung`.
- 📌 **Fund 670 (neu, beim Bau): Eine Messung über den Epochenwechsel wurde
  in der neuen Epoche bewertet**, mit deren Pod und Frist. Jetzt trägt
  jede Messung ihre Epoche und verfällt beim Wechsel
  (`fristmessung_verfallen`); mit gezählten Abweisungen wäre sonst eine
  Abweisung, die nur der Wechsel verursachte, als Verweigerung gezählt
  worden.
- **Die Messplanung im Protokoll** (Diagnose zu den Funden 664 und 667):
  `fristmessung_gestellt` (wer, Epoche, Höhe, Positionen, Zahl der
  Messenden), `zeitprobe_abgewiesen` (wie, Zahl in Folge, seit wann),
  `fristmessung_verloren`, `fristmessung_verfallen`; `fristmessung` trägt
  dazu Epoche, Höhe, Zahl der Proben und Abweisungen und wie die Messung
  endete (`beantwortet`, `grenze`, `verweigert`).
- **Die Verdrahtung im Knoten** hat eigene Proben gegen einen gestarteten
  Knoten mit Pod (`knoten::abweisung_tests`):
  `eine_lange_verweigerung_wird_im_knoten_zum_verdacht` (vier in Folge
  über die Dauer ergeben den Verdacht; zu kurz, eine zu wenig und ein
  `Nichts` während des Wartens nicht) und
  `eine_messung_aus_der_vorigen_epoche_verfaellt`. Gegenproben je Zeile:
  ohne den Aufruf von `messung_abgewiesen`, ohne die Prüfung der Epoche und
  ohne die Wartebedingung wird jeweils genau die passende Probe rot.
- 📌 **Fund 672 (Knotenteil): Der Testverkehr leerte die Probekonten.**
  Jeder Knoten verbrannte je Sekunde `1 000 + Zähler × 100` Kleinstbeträge,
  aufsummiert rund 50 · T² nach T Sekunden; ein Probekonto mit 10 MYL war
  nach rund 7,5 Minuten leer. Im kurzen Podlauf vom 2026-10-28 fehlten so
  rund 90 der 120 MYL der Probekonten, und zusammen mit dem Lastgeber
  (TESTCLIENT 0.56.0) fand der Probehahn kein Konto mehr: Der späte
  Einsteiger konnte sich nicht anmelden (451 Mal `probehahn_leer`). Jetzt
  verbrennt der Testverkehr 1; den Zustand bewegen Nummer und Gebühr. Auch
  so trägt ein Probekonto bei einer Gebühr von rund 1 200 je Sekunde nur
  rund 2,3 Stunden Testverkehr; danach scheitert er still, wie schon
  vorher.
- ⚑ **Der Credit-Preis folgt der Auslastung** (Whitepaper 5.4, Festlegung
  des Projektinhabers vom 2026-10-28). Am Epochenwechsel
  `P · exp(κ (u − 0,8))` mit κ = 1/6 (`CREDIT_PREIS_KAPPA`), aus derselben
  Auslastung wie die Trainingsabgabe, in Feinauflösung
  (`LedgerState::credit_preis_fein`, CONSENSUS 0.60.0); abgerechnet wird
  gerundet zur geraden Zahl, mindestens 1 (`credit_preis_abgerechnet`).
  Ohne Nachfrage fällt er je Epoche um ein Achtel, bei voller Auslastung
  steigt er um rund 3,4 %. Vorher stand er für immer auf dem Genesis-Wert;
  `update_price_mit_untergrenze` hatte keinen Aufrufer. Kritisch geprüft
  vor dem Bau: ein ganzzahliger Preis bliebe unter rund 15 stehen (daher
  Feinauflösung), null verweigerte jeden Burn (daher mindestens 1). ⚠️ Die
  Auslastung ist bestätigte Arbeit durch Kapazität und nie über 1;
  Nachfrage ohne Platz sieht der Preis nicht. Proben
  `der_credit_preis_folgt_der_auslastung`,
  `ein_kleiner_credit_preis_bleibt_beweglich` (Gegenprobe ganzzahlig: bleibt
  bei 4), `der_credit_preis_rundet_zur_geraden_zahl`,
  `der_epochenwechsel_schreibt_den_credit_preis_fort`; Gegenproben je Zeile
  (Fortschreibung, Mindestwert, Runden) beißen.
- `myl-node` 0.76.0. 416 Tests grün, Clippy sauber; mit dem neuen Ledger
  geprüft: `myl-ledger` 118, `myl-consensus` 285, `myl-tokenomics` 243,
  `myl-verifier` 122, `myl-governance` 96, `myl-pod` 165, `myl-client` 208,
  `myl-testclient` 299, `myl-simulation` 6.

### v0.75.0 (2026-10-28): Gebühren nach Prüfeinheiten, Validatorentopf, Zeitprobe mit Saat und Unterschrift

Festlegungen des Projektinhabers vom 2026-10-28:

- ⚑ **Gebühr = Grundpreis × Einheiten** (Bytes plus eine Prüfeinheit je
  Unterschrift, CONSENSUS 0.59.0). Ziel des Grundpreises 500 Transaktionen
  je Block (`GRUNDPREIS_ZIEL_EINHEITEN`, 600 000 Einheiten); der Erzeuger
  baut höchstens das Doppelte in einen Block
  (`BLOCK_EINHEITEN_HOECHSTENS`). Probe
  `ein_block_traegt_hoechstens_das_doppelte_ziel`, Gegenprobe ohne Grenze rot.
- ⚑ **Der Validatorentopf geht nach Stake mal Stimmen.** Die Kette zählt
  beim Anwenden eines Prüfpunktzertifikats jede Unterschrift; am
  Epochenwechsel wird der Topf an die Mitglieder des Stimmsatzes nach Stake
  mal Zahl ihrer Unterschriften verteilt, an die Adresse ihres Schlüssels.
  Ohne Stimmen oder ohne Stimmsatz bleibt er liegen, ebenso der Rest der
  Teilung. Wer Stake hat, aber nicht unterschreibt, bekommt nichts. Probe
  `der_topf_der_validatoren_geht_nach_stake_mal_stimmen` mit Gegenproben.
- 📌 **Fund 669: Die Zeitprobe hatte keine frische Eingabe.** Der Abdruck
  war für dieselbe Anfrage immer derselbe; ein Lügner hätte ihn einmal
  gerechnet und danach sofort geschickt. Jetzt trägt jede Probe eine
  frische Saat (`Nachforderung::ZeitprobeMitSaat`, angehängt,
  `frist::frische_saat`), und die Antwort ist mit dem Minerschlüssel über
  Saat, Positionen und Abdruck unterschrieben
  (`Nachlieferung::ZeitprobeUnterschrieben`, angehängt). Der Messende zählt
  nur eine Antwort, deren Unterschrift unter dem Schlüssel des Mitglieds in
  der Kette trägt; eine ohne oder mit falscher bleibt liegen, die Probe
  läuft in ihre Grenze. Ein falscher Abdruck ist damit ein unterschriebener
  Beleg (Grundlage für die Prüfung durch Checker). Probe
  `nur_die_unterschrift_ueber_die_eigene_saat_traegt`, Gegenproben je Teil
  der Nachricht und mit fremdem Schlüssel.
- Die Hinterlegung je Manifest ist als ruhend vermerkt (Fund 663): Sie
  greift erst mit der Speicherrolle.
- `myl-node` 0.75.0. 407 Tests grün, Clippy sauber.

### v0.74.0 (2026-10-28): Schneller unter Last; die Relaisleitung sagt, was abgewiesen ist

Aus dem Nachtlauf vom 2026-10-27 (Podlauf mit `--real`, Laststufen bis
1 600/s): bis 200/s alles aufgenommen, darüber ein Einbruch. Gemessen danach
an der Kette selbst:

- 📌 **Fund 668: Die Aufnahme in den Mempool war quadratisch.** `aufnehmen`
  zählte je neuer Transaktion den ganzen Mempool je Absender durch und
  rechnete dabei für jeden Eintrag die Adresse neu (SHA-256): 27 µs je
  Aufnahme bei 512 wartenden, 413 µs bei 4 096. Jetzt trägt jeder Eintrag
  seinen Absender (`Wartend`): 2 µs bei 4 096.
- ⚑ **Die Unterschriften eines Blocks als Stapel** (`pruefe_alle`, CONSENSUS
  0.58.0): Anwenden eines Blocks mit 4 096 Transaktionen 362 ms statt
  1 909 ms, für Erzeuger und Folger.
- 📌 **Die Relaisleitung meldet die Abweisung des Mempools.** Bis hierher
  antwortete sie einer Wallet „verbreitet“, auch wenn der Mempool des
  Erzeugers die Transaktion abwies (Nummer verbraucht, Mempool voll, Deckel
  je Absender); im Nachtlauf hielt das jede spätere Nummer eines Absenders
  auf. Jetzt `Fehler("vom Mempool abgewiesen …")`, und sie geht nicht
  hinaus. Probe in `TESTCLIENT` (`bezahlen`), Gegenprobe mit dem alten
  Verhalten rot.
- **Im Netz belegt** (kurzer Podlauf mit `--real`, Laststufen je 30 s,
  Lastgeber mit Nachfüllen): 200/s und 400/s ganz aufgenommen, 800/s und
  1 600/s je alles Gesendete (388/s und 369/s), kein Verlust; im Nachtlauf
  davor 131/s bei 400 und ein Einbruch auf 5/s. Die Grenze liegt jetzt in
  der Annahme über die Relaisleitung, die jede Unterschrift einzeln prüft.
- `myl-node` 0.74.0. 404 Tests grün, Clippy sauber.

### v0.73.0 (2026-10-27): Gebühren und Prüfpunkte von außen sichtbar

- Neue Metriken: `myelith_grundpreis_je_byte`, `myelith_gebuehren_epoche`,
  `myelith_gebuehren_validatoren`, `myelith_treasury_guthaben`,
  `myelith_pruefpunkt_bestaetigt`; Grundpreis, Validatorentopf und
  bestätigte Höhe auch in der Zeile `aufnahme`. Gelesen werden nur die fünf
  Zahlen, nicht der ganze Zustand.
- `/konto/<hex>` nennt `hinterlegt`: was die Adresse für Anmeldung und
  Sitzungen hinterlegt hat.
- 📌 **Probehahn: Ein Wiederholer beginnt beim nächsten Probekonto.** Trafen
  zwei Knoten dieselbe Nummer desselben Kontos, kam nur einer durch, und der
  andere traf beim nächsten Anlauf wieder dasselbe Konto; im kurzen Podlauf
  bis zu sechs Anläufe, im Netzlauf der Prüfsammlung waren nach 60 Höhen
  13 von 14 Minern angemeldet. Danach Zusagen bei Höhe 200 statt 399.
- 📌 **Fund 665: kein Probehahn, solange der Knoten aufholt.** Ein
  Einsteiger sah beim Aufholen einen alten Zustand (kein Guthaben, alte
  Nummern der Probekonten) und zapfte bei jeder Aufnahme, im kurzen Podlauf
  dreizehnmal, zwölfmal mit veralteten Nummern. Jetzt erst, wenn er nicht
  mehr als die Lückenfrist hinter der gehörten Höhe liegt.
- `myl-node` 0.73.0. 404 Tests grün, Clippy sauber.

### v0.72.0 (2026-10-27): Gebühren, Hinterlegungen, Probehahn

Festlegungen des Projektinhabers, Zahlen vorläufig (`netz::GEBUEHREN_VORLAEUFIG`).

- ⚑ **Die Gebühr**: Grundpreis je Byte mal Bytes der Transaktion. Geprüft
  vor der Nummer: Wer sie nicht decken kann oder weniger zugesagt hat
  (`max_gebuehr`), wartet und behält seine Nummer. Gezahlt wird auch, wenn
  die Anweisung danach scheitert; ihre Bytes stehen im Block. Ohne
  Trinkgeld und ohne Ausnahmen.
- ⚑ **Der Grundpreis** (`naechster_grundpreis`) wandert je Block um
  höchstens ein Achtel um das Ziel, einen halben Block
  (`GRUNDPREIS_ZIEL_BYTES`); über dem Ziel steigt er mindestens um eins,
  sonst bliebe er bei eins für immer; nie unter dem Boden des Netzes.
- ⚑ **Die Teilung** am Epochenwechsel: drei Dreizehntel an die Treasury,
  zehn in den Topf der Validatoren (`gebuehren_validatoren`). Ausgezahlt
  wird der Topf, sobald es eine Gewichtsquelle für Validatoren gibt; die
  Prägung lässt ihren Validatorenanteil aus demselben Grund aus.
- ⚑ **Ein neues Konto** kostet den Absender einmal den Aufschlag, an die
  Treasury: Konten werden nie gelöscht, die Nummer schützt vor
  Wiederholung.
- ⚑ **Hinterlegungen**: eine neue Anmeldung als Miner (zurück beim
  Abmelden; wer seine Angaben ändert, hinterlegt nicht noch einmal) und
  eine Sitzung (zurück, wenn sie aufgeräumt wird). Das Auszahlungskonto
  bleibt nach der Abmeldung eingetragen.
- ⚑ **Abgelaufene Sitzungen werden aufgeräumt** (Fund 662): Die Kette rief
  `sitzung_aufraeumen` nie, jede Sitzung blieb für immer im Zustand. Jetzt
  am Epochenwechsel, nach der Nachfrist.
- ⚠️ **Fund 663, offen**: Für Speichermanifeste gibt es keine Anweisung
  (`speicher_aufnehmen` hat ausserhalb der Ledgerproben keinen Aufrufer);
  die Hinterlegung dafür steht in den Zahlen und greift erst mit ihr.
- ⚑ **Der Probehahn** (`Netz::probehahn`, nur Testnetze): Ein Knoten, dessen
  Konto Hinterlegung und Gebühr nicht deckt, überweist sich vor der
  Anmeldung aus einem Probekonto (Zeile `probehahn`), danach füllt er ein
  Polster von einem MYL nach, wenn es unter die Hälfte fällt. Ebenso das
  bisherige Auszahlungskonto, das einen Wechsel unterschreibt.
- Die Kontoauskunft der Relaisleitung nennt Grundpreis, Aufschlag und ob das
  Konto schon besteht (für die Vorschau der Wallet).
- Neulinge ohne Guthaben kommen in keinen Block mehr; die Drossel im
  Mempool bleibt.
- Proben: `wer_die_gebuehr_nicht_zusagt_wartet`,
  `eine_gescheiterte_anweisung_zahlt_die_gebuehr`,
  `ein_neues_konto_kostet_einmal_den_aufschlag`,
  `die_anmeldung_hinterlegt_und_die_abmeldung_gibt_zurueck`,
  `eine_abgelaufene_sitzung_gibt_ihre_hinterlegung_zurueck`,
  `der_grundpreis_wandert_um_hoechstens_ein_achtel`,
  `die_gebuehren_teilen_sich_drei_zu_zehn`, je mit Gegenprobe; ohne
  Probehahn scheitert `auszahlung` an der Anmeldung.
- `myl-node` 0.72.0. 404 Tests grün, Clippy sauber.

### v0.71.0 (2026-10-27): Vertrauensfrist; wer doppelt unterschreibt, wird geschlachtet

- ⚑ **Vertrauensfrist** (CONSENSUS 0.56.0): Kommt ein Block, dessen Epoche
  mehr als 112 Epochen über dem zuletzt bestätigten eigenen Prüfpunkt liegt,
  hält der Knoten an (`vertrauensfrist_ueberschritten`) und braucht einen
  jüngeren Anker. Darüber könnte ihm jeder eine zweite Geschichte vorlegen,
  unterschrieben mit Schlüsseln, die nicht mehr haften; eine Prüfung ab
  Genesis hilft dagegen nicht, denn die Fälschung trägt gültige alte
  Unterschriften. Der Anker wandert mit, solange der Knoten folgt. ⚑ Nicht auf
  der Probekette, die ihren Satz ohnehin aus Block 1 lernt
  (`kette::vertrauen_traegt`).
- ⚑ **Doppelbelege werden gebucht** (`Kette::doppelbeleg_buchen`): Der Beleg
  trägt, die Tat liegt in der Sperrfrist, und derselbe Widerspruch ist noch
  nicht gebucht; dann schlachtet die Kette nach der Matrix (Validator,
  Doppelsignatur oder Zensur: 30, 65, 100 % gestaffelt) über
  `urteil_buchen_gestaffelt`, das Kopfgeld geht an den Melder, wer sich selbst
  meldet, wird abgewiesen. Ohne Einsatz bleibt der Beleg ohne Folge.
- **Knoten erkennen und melden**: eine zweite Prüfpunktstimme eines Wählers
  für dieselbe Höhe (`pruefpunktstimme_aufnehmen`) und einen zweiten Block
  desselben Erzeugers an einer Höhe, die die Kette schon hat
  (`doppelt_erzeugt_pruefen`); gemeldet je Widerspruch einmal über den
  Minerschlüssel (`doppelbeleg_erkannt`, Feld `belegart`; ein Feld `art`
  hätte die feste Spalte überschrieben, und das Protokoll weist es ab).
- Proben: `eine_doppelunterschrift_kostet_den_einsatz` (30 % beim ersten
  Mal, Kopfgeld; Gegenproben: derselbe Widerspruch, kein Einsatz,
  Selbstmeldung, Tat vor der Sperrfrist; ohne den Vermerk rot),
  `die_vertrauensfrist_traegt_bis_112_epochen` (mit einer Epoche zu viel
  rot), `ein_zweiter_block_derselben_hoehe_wird_als_doppelbeleg_erkannt`
  (der Block eines Fremden ist keiner; ohne die Erkennung rot).
- `myl-node` 0.71.0. 397 Tests grün, Clippy sauber.

### v0.70.0 (2026-10-27): Prüfpunkte, und die Saat kommt aus ihrem Zertifikat

- ⚑ **Prüfpunkte im Knoten** (CONSENSUS 0.55.0, Festlegungen des
  Projektinhabers): `Knoten::pruefpunkt_pflegen` stimmt bei jeder Aufnahme
  und nach jedem eigenen Block über den jüngsten unbestätigten Prüfpunkt ab,
  wenn der eigene Schlüssel im Stimmsatz steht, wiederholt die Stimme alle
  zehn Sekunden bis zur Bestätigung, sammelt die Stimmen der anderen
  (`pruefpunktstimme_aufnehmen`, nur tragende, nur über dem bestätigten
  Stand) und baut ab zwei Dritteln das Zertifikat. Der Erzeuger legt es in
  den nächsten Block. Protokollzeilen `pruefpunkt_gestimmt`,
  `pruefpunkt_zertifikat`, `pruefpunktstimme_abgelehnt`.
- Die Kette nimmt ein Zertifikat nur für einen Prüfpunkt nach dem
  bestätigten Stand, für den eigenen Block an dieser Höhe und mit mehr als
  zwei Dritteln des Satzes (`KettenFehler::PruefpunktTraegtNicht`, Marke
  `pruefpunkt-traegt-nicht`); Lücken sind erlaubt, denn ein späterer
  Prüfpunkt deckt die früheren mit.
- ⚑ **Die Ziehung der Stichprobe wartet auf das Zertifikat des
  Epochenwechsels**; die Saat kommt aus ihm (`stichprobe::saat_aus_zertifikat`,
  `stichprobe_aus_anteilen`). Der Rückfall auf den Blockhash (Fund 120,
  unbegrenzter Mahlraum) ist entfallen. Die Zuteilung der abgelaufenen Epoche
  wird bis zur Ziehung aufgehoben (vgl. Fund 658).
- 📌 Beim Bau gefunden: Ein Neuling aus einem Abzug hatte seinen Stimmsatz
  noch nicht gelernt und wies den ersten Block danach ab, sobald er ein
  Zertifikat trug; jetzt prüft eine lernende Kette gegen den Satz, den sie
  aus genau diesem Block lernt. Abzugstests und Netzlauf vorher rot.
- Proben: `ein_pruefpunkt_wird_bestaetigt` (Gegenproben: dasselbe noch
  einmal, ein anderer Block, ein Fremder, keine Prüfpunkthöhe, ein
  nachträglich eingesetztes Zertifikat), `die_saat_kommt_aus_dem_zertifikat`;
  in `bezeugte_arbeit_erreicht_ein_konto` zieht die Kette erst mit dem
  Zertifikat. Gegenprobe ohne Prüfung des Zertifikats rot.
- `myl-node` 0.70.0. 394 Tests grün, Clippy sauber; Netzlauf grün.

### v0.69.0 (2026-10-27): Stimmsatz ab Genesis, jeder Block unterschrieben

- ⚑ **Festlegung des Projektinhabers vom 2026-10-27:** Jede Kette hat ab
  Genesis einen Stimmsatz, und jeder Block braucht die Unterschrift eines
  Mitglieds unter dem Schlüssel, den der Satz für es führt. Unsignierte
  Blöcke gibt es nicht mehr (`Kette::erzeuger_traegt`).
- **Woher der Satz kommt:** aus der Stimmsatzdatei (`--stimmsatz`), jetzt für
  jeden Knoten, nicht nur für die, die mitstimmen; wer nicht darin steht,
  hört zu, statt abzubrechen. ⛔️ Ohne Datei startet ein Netz nicht, ausser
  der Probekette (`Netz::lernt_erzeuger`): Sie lernt ihren Erzeuger aus
  Block 1, als einziges Mitglied mit Gewicht eins, und verlangt ihn danach.
  Das ist Vertrauen beim ersten Block und so schwach wie die Probekette
  selbst, die keinen Anker hat.
- Ein Erzeuger ohne eigenen Schlüssel unterschreibt auf der Probekette mit
  dem Probeerzeuger (`kette::PROBEERZEUGER`, jeder kann ihn ableiten;
  Protokollzeile `erzeuger_probeschluessel`), auf jedem anderen Netz startet
  er nicht. Ein Erzeuger, dessen Schlüssel nicht im Satz steht, startet
  nicht. `Kette::probestand` unterschreibt mit dem Probeerzeuger.
- Die Prüfung im Hintergrund rechnet mit demselben Satz.
- Probe `nur_ein_belegter_erzeuger_kommt_durch` neu gefasst: unsigniert nie,
  der Erzeuger von Block 1 wird Mitglied und ein zweiter kommt nicht durch,
  ein Netz ohne Lernen nimmt ohne Satz nichts. Gegenprobe (die alte Regel
  „ohne Satz geht alles“) rot. Vier Tests, die einen Block verfälschen,
  unterschreiben ihn danach neu (`neu_unterschreiben`): Gefangen werden muss
  der falsche Block eines berechtigten Erzeugers, nicht erst die Unterschrift.
  Ebenso `tests/schuldspruch.rs`; in `tests/zwei_knoten.rs` ist der Knoten,
  der Blöcke baut, jetzt Erzeuger.
- 📌 Beim ersten Testlauf hielten Netzlauf und Abzugstests an: Der Erzeuger
  der Läufe hatte keinen Schlüssel und baute unsignierte Blöcke (106 statt
  rund 250 Byte); daraus der Probeerzeuger.
- `myl-node` 0.69.0. 395 Tests grün, Clippy sauber; Netzlauf
  grün (Lügner gemeldet bei Höhe 4 332).

### v0.68.0 (2026-10-27): Die Zeitprobe ohne Ping, nach der Zusage bemessen (Fund 646)

- ⚑ **Festlegung des Projektinhabers vom 2026-10-27:** Es wird nichts mehr
  abgezogen, was der Gemessene verlängern kann. Bestanden ist eine Messung,
  wenn die kürzeste von fünf Zeitproben ≤ k · Frist + Zugabe ist
  (`frist::gemessen_je_token`, `ZUGABE_US` = 2 s). Der Ping, der Wechsel aus
  0.67.0 und `fristmessung_verworfen` entfallen; eine Messung kann nicht mehr
  negativ werden. Die Zahl im Fristzeugnis ist (Probe − Zugabe) / k; die
  Kette hält sie gegen die Frist wie bisher.
- 📌 **Fund 646 geschlossen:** Wer seine Pongs zurückhielt, liess jede
  Messung verwerfen und wurde nie gemessen (Audit A28).
- ⚑ **Die Probe richtet sich nach der Zusage** (`frist::probe_positionen`,
  nachgeschärft beim Bau): k = 2 s · `FRIST_FAKTOR` / Frist, 64 bis 2 048
  Positionen. Mit fester Grösse fiel ein Lügner erst bei 31 ms je Token über
  der Frist auf, ein Shard rechnet aber 2 bis 8 ms. Jetzt fällt einer auf,
  sobald er mehr als rund das Dreifache seiner Zusage rechnet, an der
  Obergrenze ab rund 1 ms über der Frist; ein Ehrlicher zahlt je Probe rund
  2 s und hat für den Weg Zugabe plus Ziel.
- 📌 **Beim Bau gefunden: Wer langsam genug war, entkam.** Die grosse Probe
  des Lügners lief über die Zeitgrenze einer Anfrage (10 s), die Anfrage
  scheiterte, und die Messung wurde verworfen. Jetzt endet eine Probe nach
  `frist::grenze_der_probe` als Überschreitung, ob die Antwort noch kommt
  oder nicht (`zeitprobe_ueberschritten`); im Netzlauf vorher rot.
- ⚑ **Die Probe trägt ihre Grenze** (`Nachforderung::ZeitprobeMitGrenze`,
  angehängt, Marke 12; `frist::probe`): Der gemessene Knoten gibt sie an
  seinen Shard weiter (SHARED_TYPES 0.51.0), und der hört dort auf. Sonst
  rechnete der Shard eines Lügners seine grosse Probe rund 50 s zu Ende, und
  jeder andere Messende bekam „besetzt“; im Netzlauf kam keine Mehrheit
  zusammen. Die Antwort auf eine Zeitprobe steht jetzt in
  `Knoten::zeitprobe_beantworten`, für beide Formen.
- Die Zeitgrenze einer laufenden Messung steigt von 30 auf 120 s.
- ⚠️ **Offen (Fund 660):** Ein Mitglied, das jede Zeitprobe sofort mit
  `Nichts` beantwortet, wird nie gemessen. Zur Entscheidung.
- ⚠️ **Offen (Fund 661, beim Bau gefunden):** Der Abdruck, den eine
  Zeitprobe zurückgibt, wird nicht nachgerechnet; wer sofort mit einem
  beliebigen Abdruck antwortet, besteht. Zur Entscheidung.
- Proben: `der_luegner_bekommt_die_groessere_probe` (Gegenproben: feste 64
  Positionen und die alte Regel mit Ping lassen ihn durch),
  `ein_ehrlicher_besteht_mit_langem_weg`, `die_grenze_der_probe_ist_die_frist`,
  `die_kuerzeste_probe_traegt_einen_verlust`.
- `myl-node` 0.68.0. 395 Tests grün, Clippy sauber.

### v0.67.0 (2026-10-27): Die Stichprobe findet ihren Pod; Ping und Probe im Wechsel

- 📌 **Fund 658: Keine Stichprobe fand ihren Pod.** Gezogen wird beim
  Epochenwechsel, für die **abgelaufene** Epoche; `Kette::pod_der_kennung`
  kannte nur die Zuteilung der **laufenden** und gab für jede andere `None`.
  Jede Ziehung endete als `ohne_adresse` (im kurzen Podlauf: drei Ziehungen,
  null Anfragen), noch bevor Fund 656 zum Tragen kam. Jetzt hält die Kette
  in `baue_block` die Pods der Ziehung fest, aus der Zuteilung der
  ablaufenden Epoche, bevor der Wechsel sie ersetzt; `pod_der_kennung`
  findet sie für die Epoche der Ziehung. Probe in
  `bezeugte_arbeit_erreicht_ein_konto`, ohne die Korrektur rot.
- **Im Netz belegt** (kurzer Podlauf mit Tor, 2026-10-27): drei Ziehungen, 18
  Anfragen, keine ohne Adresse, alle 18 bei den Minern angekommen
  (`spuranfrage_eingegangen`); im Lauf davor drei Ziehungen, null Anfragen.
  Beantwortet wird noch nicht (Fund 656, NODE 0.66.0).
- 📌 **Fund 646, Teil (a): Ping und Zeitprobe im Wechsel**, je fünf
  (`frist::naechster_schritt`, `PINGS_JE_MESSUNG` gleich
  `PROBEN_JE_MESSUNG`). Bis hierher kamen drei Pings, dann fünf Proben; die
  Hälfte aller Messungen wurde verworfen (im langen Podlauf vom 2026-10-26/27
  1 591 von 3 188), weil der Weg über Sekunden um Hunderte Millisekunden
  schwankte und die Rechnung einer Probe bei rund 50 ms lag. Im kurzen
  Podlauf jetzt 37 % statt 45 bis 52 % in fünf Läufen davor. Probe
  `pings_und_proben_wechseln_sich_ab`, mit der alten Folge rot.
- ⚠️ **Offen aus Fund 646:** Ein Lügner, der seine Pongs länger zurückhält
  als seine Proben, lässt jede Messung verwerfen und wird nie gemessen. Das
  braucht eine Festlegung (die Frist mit fester Zugabe für den Weg statt
  eines abgezogenen Pings).
- `myl-node` 0.67.0. 393 Tests grün, Clippy sauber.

### v0.66.0 (2026-10-27): Die Stichprobe kommt an (Fund 656, der Transport)

- 📌 **Fund 656: Keine Spuranfrage kam je an.** `stichprobe_verschicken`
  kodierte die nackte `Spuranfrage`; der Empfänger las jede Anfrage als
  `Nachforderung`, das niedrigste Byte der Epoche wurde zur Marke, und die
  Anfrage war unlesbar (oder, je nach Epoche, etwas anderes). Die
  Protokollzeile `stichprobe_gefragt` zählte Anfragen, die niemand verstand.
  Gefunden beim Lesen, nicht im Lauf.
- Neu `Nachforderung::Spur` (angehängt, Marke 11). `stichprobe::anfragen_fuer`
  liefert jetzt die fertige Nachforderung statt der nackten Anfrage; der
  Aufrufer kann die Hülle nicht mehr vergessen.
- ⚑ **Beantwortet wird noch nicht.** Die Segmentzeugnisse eines Bündels
  hält nur der Koordinator, und die Eingabe eines Segments hebt seit E10
  niemand auf (Fund 118). Der Empfänger antwortet `Nichts` und schreibt
  `spuranfrage_eingegangen` (von, Epoche, Pod, Segment) ins Protokoll; damit
  ist im Netz sichtbar, dass die Anfragen ankommen. Wer antwortet und woher
  die Eingabe kommt, ist eine Entscheidung des Projektinhabers.
- Proben: `eine_spuranfrage_kommt_als_spuranfrage_an` (die nackte Anfrage
  geht für keine Epoche als Spuranfrage durch, die Marke steht hinten);
  `jede_ziehung_ergibt_fragen_an_alle_mitglieder` liest jede ausgehende
  Anfrage zurück. Gegenprobe: eine andere Nachforderung statt `Spur`, rot.
- `myl-node` 0.66.0. 392 Tests grün, Clippy sauber.

### v0.65.0 (2026-10-27): Der Übernehmer prüft den Erzeuger; der Stapel darf durch

- ⚑ **Die Unterschrift des Erzeugers beim Übernehmen** (CONSENSUS 0.54.0):
  `Kette::uebernimm` prüft vor jeder Rechnung, wer den Block erzeugt hat.
  Eine Unterschrift, die dasteht, muss tragen, immer (Fund 192). **Mit
  Stimmsatz** muss jeder Block unterschrieben sein, von einem Mitglied, unter
  genau dem Schlüssel, den der Satz für es führt. **Ohne Stimmsatz**
  (Probebetrieb mit einem Erzeuger) darf ein Block unsigniert sein. Neuer
  Fehler `ErzeugerUnbelegt`, Ablehnungsmarke `erzeuger-unbelegt`.
- Die Kette unterschreibt ihre Blöcke in `baue_block`, bevor sie geschrieben
  und verbreitet werden, wenn ein Erzeugerschlüssel gesetzt ist: beim
  Erzeuger aus `--konsensschluessel` (auch ohne Miner zu sein), im BFT mit dem
  Schlüssel der Runde.
- Probe `nur_ein_belegter_erzeuger_kommt_durch`: Mitglied angenommen;
  unsigniert, fremd und untergeschobener Schlüssel abgewiesen; ohne
  Stimmsatz unsigniert und fremd unterschrieben angenommen, eine Unterschrift,
  die nicht trägt, abgewiesen. Ohne die Prüfung rot.
- 📌 **Der Archivbeleg schickt die Bytes, die der Blockhash deckt**
  (`Block::hashbytes`, CONSENSUS 0.54.0), nicht die volle Kodierung. Die
  Kette prüft sie gegen die Historie der Hashes; mit der Unterschrift im
  Block trugen die vollen Bytes nicht mehr, und kein Knoten verdichtete
  (sechs rote Tests in `tests/verdichtung.rs`, jetzt grün). Der Test der
  Splitter liest die Blöcke jetzt und hasht wie jeder Block.
- `Nachlieferung::Abzug` trägt den Block in einer `Box` (Clippy:
  grosser Unterschied zwischen den Varianten, seit der Block die
  Unterschrift trägt); an der Kodierung ändert das nichts.
- Die Liste der weitergereichten Shardanfragen
  (`nachschub::SHARDANFRAGEN_ERLAUBT`) ist eine Aufzählung statt eines
  Bereichs und enthält Marke 11, `RechneStapel` (Fund 650, COMPUTE_PIPELINE);
  das Training (8 bis 10) bleibt gesperrt. Probe im TESTCLIENT
  (`shardmarken.rs`).
- `myl-node` 0.65.0. 391 Tests grün, Clippy sauber.

### v0.64.0 (2026-10-26): Zeitproben nach dem Anlauf und in zwei Fenstern (Fund 655); Überlaufprüfung im Freigabebau

- ⛔️📌 **Fund 655: Ein ehrlicher Miner wurde gesperrt.** Im langen
  Podlauf mit Tür und Störer zweimal derselbe: drei Messungen über seiner
  Frist (130 bis 394 ms bei 100 ms), alle in den ersten 25 s einer Epoche,
  14 bis 25 s nach dem Start seines Shards, der zugleich schon für die Tür
  rechnete; sonst lag er bei 2 bis 14 ms. Die Zeitprobe mass den Anlauf
  mit, und drei ehrliche Zeugen genügten. **Jetzt (Festlegung des
  Projektinhabers: Anlaufzeit und zwei Fenster):** Gemessen wird erst ab
  einem Viertel der Epoche (`frist::darf_messen`, in Blöcken). Eine
  Überschreitung ist nur ein Verdacht; ein Zeugnis gibt es erst, wenn eine
  zweite Messung mindestens ein Achtel Epoche später wieder darüber liegt,
  und bezeugt wird der kleinere Wert (`frist::urteil`). Liegt die zweite
  darunter, ist der Verdacht erledigt. Ein Lügner liegt immer darüber und
  wird weiter bezeugt, nur später in der Epoche. Die Kette ändert sich
  nicht; die Regel gilt für die Messenden. Die Zeile `fristmessung` nennt
  das Urteil. Probe `erst_nach_dem_anlauf_und_in_zwei_fenstern` mit
  Gegenprobe (der Lügner wird bezeugt). Der Beleg im Netz steht aus: der
  lange Podlauf ohne Sperre eines ehrlichen Miners.
- Überlaufprüfung im Freigabebau für die Protokollkisten (Festlegung des Projektinhabers): `[profile.release.package.<kiste>] overflow-checks = true` für jede Protokollkiste im Baum dieses Programms; die Rechenkerne (`integer-llm-*`) bleiben ohne. Bis hierher war sie nur im Testprofil an, und eine Rechnung im Konsenspfad wäre im ausgelieferten Programm an den Rändern still umgelaufen. Belegt am Knoten: `myl_ledger` wird mit `-C overflow-checks=on` übersetzt, `integer_llm_kernels` ohne. Alle 4 359 Tests liefen schon immer mit eingeschalteter Prüfung.
- `myl-node` 0.64.0.

### v0.63.0 (2026-10-26): Die Tür sieht eine Sitzung, sobald sie in der Kette steht, und wartet auf einen Pod über Knoten (Funde 647, 648, 650)

- 📌 **Fund 647: Die erste Anfrage nach einer Pause traf auf die Epoche vom
  Beginn des Wartens.** Die Türschleife las Epoche, Schlüsselwechsel und Uhr
  und wartete danach auf die nächste Verbindung, beliebig lange. Ein
  Sitzungskontrakt, der in einer späteren Epoche eröffnet wurde, galt für
  die Tür noch nicht (`jetzt < gueltig_ab`): 401, obwohl Vollmacht und
  Kontrakt stimmten. Gesehen im Torlauf über Docker, wo die erste Anfrage
  Epochen nach dem Start kam; der Test zu Fund 166 setzte die Epoche, bevor
  die Tür wartete, und sah es nicht. Jetzt nimmt die Schleife zuerst an
  (`Tuer::annehmen`, GATEWAY 0.9.0), wechselt dann den Sitzungsschlüssel
  und liest Epoche und Uhr; `tuer::eine_anfrage` und `eine_v1_anfrage`
  nehmen die angenommene Verbindung. Der Test
  `ein_kontrakt_der_erst_spaeter_gilt_kommt_erst_spaeter_durch` lässt die
  Kette jetzt wandern, während die Tür schon wartet; Gegenprobe mit dem
  alten Ablauf (lesen, dann warten): 403, rot.
- 📌 **Fund 648: Die Abschrift der Tür wurde nur bei der Aufnahme
  aufgefrischt**, im Vorgabefall alle dreissig Sekunden, obwohl die Doku
  des Feldes „bei jedem Block“ sagte. Wer eine Sitzung eröffnete und gleich
  fragte, bekam bis dahin 401; ebenso spät wirkte ein Widerruf. Jetzt zieht
  `abschrift_nachziehen` die Abschrift nach, sobald die Kette gewachsen ist.
  ⚠️ Das kopiert je Block alle Kontrakte; bei vielen Sitzungen gehört ein
  Abgleich nur der geänderten hin. Beleg im TESTCLIENT (`bezahlen`, mit der
  Abschrift des Knotens selbst und Aufnahme nach Vorgabe); Gegenprobe ohne
  das Nachziehen: rot.
- Der Rechenweg zur Ortsleitung nennt jetzt, wo er endet
  (`myl-node: Rechenweg in Epoche …: <Grund>`): Ankündigung, Gegenstelle,
  Pipeline-Stand, Kanal, Form, Leitung oder die Ablehnung des Shards. Die
  Tür meldet dem Klienten weiter nur 502.
- 📌 **Fund 650: Die Tür gab nach 45 s auf, während der Pod richtig
  rechnete.** Gemessen im Lauf über Docker (50 bis 200 ms je Richtung,
  1 bis 5 % Verlust): 0,5 bis 2,7 s je Runde zu einem Mitglied, rund 4 s je
  Token bei vier Positionen, 9 s Schlüsselaufbau. Die Frist der Ortsleitung
  (`FRIST`, 45 s) gilt dem Weg über das Netz und seiner Leerlauffrist; an
  der Tür wartet ein Klient auf derselben Maschine 600 s. Aufträge von der
  Tür bekommen jetzt `TUERFRIST` (540 s), Probe
  `die_tuerfrist_liegt_zwischen_netz_und_klient`. ⚠️ Die Tür bedient eine
  Anfrage nach der anderen; so lange steht sie für alle anderen. Und der
  Prefill geht Token für Token durch alle Positionen; dort liegt der
  grösste Hebel.
- Jede über das Netz weitergereichte Shardanfrage schreibt bei ihrer
  Antwort eine Zeile `shardrelais` (Mitglied, Dauer, Bytes, Fehler): Ein
  Token geht durch jede Position einmal, und hier steht, was eine Runde
  kostet.
- `myl-node` 0.63.0.

### v0.62.0 (2026-10-26): Auszahlungskonto über die Kette, Probeprägung für Testwallets (Fund 643)

- 📌 **Fund 643: `--konto` wurde gelesen und nie benutzt.** Die Hilfe
  versprach „Wohin die Erträge gehen“; im Betrieb trug kein Knoten je ein
  Auszahlungskonto ein, und ohne Eintrag gibt es keinen Anteil. Jetzt meldet
  sich ein Knoten mit `--konto` und `--konsensschluessel` als Miner an (wenn
  nicht schon über die Zusage) und trägt das Konto über die Kette ein.
  Die erste Eintragung unterschreibt der Miner, jede weitere das bisher
  eingetragene Konto: `--kontowechsel <schlüssel>` (neu). Ohne diesen
  Schlüssel bleibt das alte stehen, und die Zeile `auszahlungskonto` nennt
  beide. Test `das_konto_geht_ueber_die_kette_und_wechselt_nur_mit_dem_alten_schluessel`
  (Eintragung, Wechsel ohne Schlüssel bleibt, Wechsel mit Schlüssel);
  Gegenprobe ohne die Pflege: rot.
- ⚑ **Probeprägung** (Festlegung des Projektinhabers): An jedem
  Epochenwechsel bekommt das Auszahlungskonto jedes angemeldeten, in dieser
  Epoche nicht gesperrten Miners den Betrag des Netzes, auf der Probekette
  10 MYL, damit ein Testwallet beim Mining sichtbar wächst. Neben der
  Ausschüttung, nicht in ihr: ohne Burn, ohne Bündel, und fällt sie weg,
  ändert sich an der echten Rechnung nichts. ⛔️ Auf jedem anderen Netz null
  (`Netz::probepraegung`, Probe `nur_die_probekette_praegt_zur_probe`).
  Test `die_probekette_praegt_je_epoche_ins_auszahlungskonto` (eingetragen
  bekommt, gesperrt und ohne Eintrag nicht, das Testnetz prägt nichts);
  Gegenprobe ohne den Aufruf: rot. Die drei Proben der echten Ausschüttung
  ziehen den Probebetrag ab und prüfen den echten Teil wie zuvor.
- Aufnahme und Metriken nennen Guthaben und Eintrag des Auszahlungskontos
  (`myelith_auszahlung_guthaben`, `myelith_auszahlung_eingetragen`).
- ⚑ **Kontoabfrage** `/konto/<64 hex>` am Beobachtungsendpunkt: Höhe,
  Guthaben, Nonce, ob als Miner angemeldet, für wie viele Miner
  Auszahlungskonto, und ob die Adresse damit „bekannt“ ist. Der Dienst
  reicht die Anfrage über einen Kanal an die Schleife des Knotens, die aus
  dem Zustand antwortet; kopiert wird nichts. Der Client fragt damit, ob
  ein Wallet schon Spuren hat. ⚠️ Eine Auskunft, kein Beweis gegen die
  Zustandswurzel. Proben im Endpunkt (unbekannte Adresse 200, kaputte 404)
  und in `tests/auszahlung.rs`.
- ⚑ **Ein Pod über Knoten**: `--podrelais <host:port>` und
  `--podrelaisausweis <datei>` öffnen eine Relaisleitung für einen
  Koordinator auf derselben Maschine. Er fragt nach dem Podbild der Epoche
  (Pods, Mitglieder in Shardreihenfolge, Grenzen aus der Kette) und reicht
  Shardanfragen an Mitglieder; der Knoten schickt sie als
  `Nachforderung::Shard` über das Netz (mit Kennung zur Zuordnung), das
  Mitglied gibt die Bytes unverändert an seinen Shard und die Antwort
  zurück. Ist das Mitglied der Knoten selbst, geht die Anfrage direkt an
  den eigenen Shard. Weitergereicht werden nur die Marken von Rechnen,
  Zuschnitt, Schlüssel, Unterschreiben, Sitzung vergessen, Abdruck, Profil
  und gehaltenen Sitzungen (`SHARDANFRAGEN_ERLAUBT`); Training über fremde
  Knoten nicht. Eigene Plätze (`shard_plaetze`), eine Ablehnung kommt mit
  Grund zurück. Der Knoten rechnet dabei nichts und hängt nicht an
  `myl-pod`. Proben: Leitung nur mit Ausweis (mit Gegenprobe), der ganze
  Weg im TESTCLIENT (`pod_ueber_knoten`).
- ⚑ **Das Bündel in die Kette:** Über die Leitung gibt ein Koordinator ein
  unterschriebenes PoI-Bündel ab (`Relaisfrage::BuendelEinreichen`). Der
  Knoten prüft es wie das Einreichen (`Kette::buendel_pruefen`: Epoche,
  noch kein Bündel des Pods, Pod in der Zuteilung, Aggregat) und gibt es
  der ersten Position, denn nur sie darf einreichen
  (`Nachforderung::Buendel`); die prüft erneut, ob sie Koordinator ist, und
  reicht mit ihrem Konsensschlüssel ein. Ein gefälschtes Bündel kommt so
  gar nicht erst in die Kette.
- 📌 **Fund 645: Ein Bündel mussten auch die Reservemitglieder
  unterschreiben.** Prüfung beim Einreichen und Zuschreibung am
  Epochenwechsel nahmen `pod.mitglieder()`, also auch die Reserve; die hat
  keinen Shard, sieht die Arbeit nie und kann nicht nachrechnen, was sie
  unterschriebe. Ein echter Pod mit Reserve bekam so nie ein Bündel durch.
  Jetzt unterschreiben die Positionen (`unterzeichner_des_buendels`), an
  beiden Stellen dieselbe Menge. Gegenprobe mit der alten Menge: die Probe
  der bezeugten Arbeit rot. ⚠️ Trainingssegmente prüfen weiter gegen alle
  Mitglieder; dort ist zu klären, wer rechnet.
- Belegt im TESTCLIENT (`pod_ueber_knoten`): Bündel über den Torknoten an
  die erste Position eingereicht, mit einem Burn derselben Epoche wachsen
  die Konten der Positionen am Epochenwechsel um die Probeprägung und ihren
  Anteil an der Ausschüttung (der Kopf am meisten), die der Reserve nur um
  die Probeprägung; ein Bündel mit erhöhtem Anspruch wird am Aggregat
  abgewiesen.
- ⚑ **Ein Wallet bezahlt über die Leitung:** `Relaisfrage::Konto` nennt
  Nonce, Guthaben, Credits, Netzwert und das eigene Konto (den Betreiber
  einer Tür); `Relaisfrage::Transaktion` nimmt eine unterschriebene
  Transaktion, prüft Unterschrift und Netz und verbreitet sie. Eine
  verfälschte kommt nicht ins Netz (Probe im TESTCLIENT, `bezahlen`).
- Mit `--konsensschluessel` nennt der Knoten beim Start seinen Endpunkt
  (`myl-node: Endpunkt …`), den ein Ortsdienst als `--knoten` erwartet.
  Das Podbild nennt die Länge einer Epoche (`bloecke_je_epoche`), damit ein
  Koordinator sie nicht ein zweites Mal hinschreibt.
- Schlüsseldateien nehmen neben 32 auch 64 Bytes Ausgangsmaterial, also
  einen Seed nach BIP-39 aus einem Testwallet des Clients; daraus ergibt
  `key_gen` den Hauptschlüssel nach EIP-2333. So liest `--kontowechsel`
  auch diese Wallets. Probe `ein_seed_aus_64_bytes_ist_ein_schluessel`.
- `myl-node` 0.62.0.

### v0.61.0 (2026-10-25): Nachfrage bei Stille, die kürzeste von fünf Zeitproben (Funde 618, 639)

- ⚑ **Ein Knoten, der kein Gossip mehr hört, fragt selbst nach.** Bis
  hierher erfuhr er nur über Gossip, dass er zurücklag. Im Netzlauf hing so
  in rund einem von vier Läufen ein Knoten, meist einer, der nur über mDNS
  eingestiegen war: Der Erzeuger führte ihn unter der Gossip-Schwelle, er
  verpasste den letzten Schub, und weil danach kein Block mehr kam, stand er
  mit lebenden Verbindungen für immer (bei `m07` 1 392 Blöcke zurück).
  Jetzt fordert der Knoten ab der eigenen Höhe an, wenn sie
  `STILLE_NACHFRAGE` (10 s) stand und keine Nachforderung läuft, und fragt
  weiter, solange die Gegenstelle liefert. Eine Behauptung (Fund 634)
  entsteht dabei nicht; die Sperre für leere Antworten (Fund 626) und die
  Drossel (Fund 632) gelten wie sonst. Protokollzeile `stille_nachfrage`.
- Die Aufnahme nennt die schlecht bewerteten Gegenstellen mit Kennung
  (`schlecht_bewertete`). Ein Miner schreibt seine eigene Fristsperre ins
  Protokoll (`fristsperre`, einmal je Epoche): Von außen war nur zu sehen,
  dass gemeldet wurde, nicht, dass die Kette sperrte.
- Test `ohne_gossip_fragt_ein_knoten_bei_stille_selbst_nach`: Der Erzeuger
  baut 700 Blöcke, bevor der zweite Knoten kommt, danach keinen mehr.
  Gegenprobe mit abgeschalteter Nachfrage: rot, der zweite Knoten bleibt
  bei null.
- 📌 **Fund 639: Eine einzige Zeitprobe trug jeden Verlust des Netzes als
  Rechenzeit.** Im neuen Podlauf unter `tc netem` (50 bis 200 ms, bis 5 %
  Verlust) wurden zwei ehrliche Miner mit je einer Ebene (Frist 40 und
  80 ms) zwischen 1 und 770 ms gemessen, drei Zeugen lagen darüber, und die
  Kette sperrte sie; im Netzlauf im selben Prozess war das nicht zu sehen.
  Jetzt zählt die kürzeste von `PROBEN_JE_MESSUNG` (5) Proben, wie schon
  der kürzeste Ping: Verlust verlängert eine Probe nur, und unter seine
  echte Rechenzeit kommt auch ein Lügner damit nicht. Test
  `die_kuerzeste_probe_traegt_einen_verlust` mit den Zahlen aus dem Lauf.
- 📌 **Fund 638:** `tests/beendigungssignal.rs` und `tests/beobachtung.rs`
  räumen auch den Ordner des Schlüssels weg. Mit `TMPDIR` auf einen leeren
  Ordner geprüft: Die ganze Kiste hinterlässt nichts.
- Langer Podlauf mit dieser Fassung bestanden: 381 Epochen, kein ehrlicher
  Miner gesperrt, der Lügner ab der ersten Epoche nach seiner Messung ohne
  Shard, 77 Shard- und 55 Minerabstürze überstanden, eine Wurzel.
- `myl-node` 0.61.0.

### v0.60.1 (2026-10-24): `tokio` 1.54.1 wegen RUSTSEC-2026-0386

- Die Sperrdatei hebt `tokio` von 1.53.1 auf 1.54.1: Die sicheren Konstruktoren von `AsyncFd` verletzen die I/O-Sicherheit (RUSTSEC-2026-0386, `unsound`, neu seit dem 2026-10-23), und cargo-deny war damit rot. Der eigene Code nutzt `AsyncFd` nicht; geändert hat sich nur die Sperrdatei, der Vorrat trägt die neue Fassung. `myl-node` 0.60.1.

### v0.60.0 (2026-10-24): Wiederanlauf ab dem eigenen Abzug, schnelleres Nachladen, Fristen ab der Spitze, die Frist der Kopien, Drossel, Blockgrenze, gehörte Höhe, Neulinge und Empfängerbeweis (Funde 628 bis 635, J1)

Der volle Archivlauf bestand, aber zwei ehrliche Kopien verloren ihren Platz,
und Splitter meldeten Lücken in den eigenen Nummern, die keine waren. Alle
drei Ursachen wuchsen mit der Geschichte oder mit dem Rückstand, nicht mit
dem Fehlverhalten eines Knotens.

- ⚑ **Fund 629: Der Wiederanlauf setzt beim jüngsten eigenen Abzug an.**
  Bis hierher rechnete jeder Start die ganze Kettendatei ab der Basis nach,
  bei einer Kopie ab Genesis: 166 s für 349 646 Blöcke im Archivlauf. Der
  Knoten war so lange fort, holte danach weitere 40 s auf, und seine Belege
  zweier Epochen entstanden erst beim Nachverarbeiten, als die Epochen
  vorbei waren; er wurde ersetzt. Jetzt werden die Blöcke bis zum jüngsten
  Abzug nur auf Anschluss, Höhe und Epoche geprüft (`Kette::vorspulen`),
  der Abzug setzt den Zustand ein (`Kette::abzug_einsetzen`: sein Block muss
  der vorgespulte letzte sein, und lebt die Kette ab Genesis, muss der
  Historienbaum aus der Datei die Historie des Abzugs treffen), und erst die
  Blöcke danach gehen durch `uebernimm`. Scheitert der Einsatz (fremder
  Block, Datei endet davor, Anschluss bricht), wird ab der Basis
  nachgerechnet wie bisher. `kette_geladen` nennt dazu `ab_abzug`,
  `vorgespult` und gegebenenfalls `abzug_verworfen`.
  ⚑ **Was nicht mehr nachgerechnet wird**, sind die Übergänge bis zum
  Abzug: Der Knoten hat diese Blöcke selbst angewandt, bevor er sie schrieb,
  und den Abzug danach selbst abgelegt. Eine Einfuhr nach einem Totalausfall
  bringt keine Abzüge mit und wird weiter ab Genesis nachgerechnet.
- ⚑ **Fund 630: Das Nachladen der Geschichte holt acht Lieferungen
  zugleich**, zwei je Gegenstelle (`ARCHIV_ZUGLEICH`,
  `ARCHIV_JE_GEGENSTELLE`). Es teilte sich bis hierher die zwei Plätze der
  Hintergrundprüfung; bei 50 bis 200 ms Laufzeit je Seite und bis 5 %
  Verlust kam je Gegenstelle alle 3,6 s eine Lieferung zu 512 Blöcken,
  rund 226 Blöcke je Sekunde, und eine neue Kopie brauchte 509 s für
  115 200 Blöcke. Ihre Frist von acht Epochen plus zwei verfehlten sind im
  Testtakt von 25 ms rund 400 s; sie wurde ersetzt. Eine Lieferung wird der
  Anfrage mit derselben Anfangshöhe zugeordnet. Zwei je Gegenstelle bleiben
  in der Herleitung der Ströme je Verbindung (`myl-net`, sieben legitime),
  denn eine Gegenstelle, bei der eine Nachforderung oder die
  Hintergrundprüfung läuft, bekommt keine.
- ⚑ **Fund 628: Fristen eigener Transaktionen zählen ab der gehörten
  Spitze**, nicht ab der eigenen Höhe (`frist_bezug`, `netz_luecke`). Folger
  lagen bei 25 ms je Block meist 40 bis 220 Höhen hinter dem Erzeuger; ihre
  Transaktion stand längst im Block, und nach 64 eigenen Höhen meldeten sie
  eine Lücke, 25 Mal bei einem einzigen Splitter, und wiederholten die
  Pflicht mit einer neuen Nummer. Die Spitze zählt höchstens 1024 Höhen über
  der eigenen (`NETZ_SPITZE_HOECHSTENS`), denn sie kommt auch aus
  abgelehnten Blöcken: Eine erfundene verzögert die Heilung einer echten
  Lücke, verhindert sie nicht. Dasselbe gilt für die Wiederholfrist
  (`schon_gesendet`).
- ⚑ **Die Frist einer neuen Kopie ist eine knappe halbe Periode**
  (`myl-ledger` 0.26.0, 31 statt 8 Epochen). Der Knoten prüft beim
  Übersetzen, dass Frist, zwei verfehlte Epochen und noch einmal Frist in
  seine Periode von 64 Epochen passen (`archiv.rs`); mit 32 bricht der Bau
  ab, das ist geprüft.
- ⛔️ **Fund 633: Ein Block hatte keine Bytegrenze.** Jede Transaktion, die
  ihre Nummer verbrauchte, ging in den nächsten Block; Gossip nimmt Blöcke
  aber nur bis 2 MiB an. Rund achtzig Archivbelege zu je 27 KB von einem
  beliebigen Konto, ohne Gebühr, ergaben einen Block, den kein Folger je
  bekam, und die Kette stand für alle. Jetzt trägt ein Block höchstens
  `BLOCK_TXS_HOECHSTENS_BYTES` (die Hälfte der Gossip-Grenze); was nicht
  passt, wartet auf den nächsten, was allein grösser ist, fällt heraus,
  sonst stünde es für immer vorn. Dazu nimmt der Mempool je Absender
  höchstens `MEMPOOL_JE_ABSENDER` (64) auf, sonst füllte ein Konto alle
  4096 Plätze. Konsens unberührt: Der Erzeuger wählt nur aus, was er
  anwendet.
- ⚑ **Fund 632: Teure Anfragen ohne Drossel.** `Bloecke` (512 Blöcke von
  der Platte), `Archiv` (512 Blöcke und eine Gruppe für den Beweis) und
  `Teil` (für jede andere Gruppe 1024 Blöcke neu codiert) konnte jede
  Gegenstelle beliebig oft stellen, in der Ereignisschleife; eine Flut hielt
  den Konsens an (Klasse von Fund 609). Jetzt `drossel.rs`: je Art ein Eimer
  je Gegenstelle und einer für alle (Blöcke 32/s je Gegenstelle, 128/s
  gesamt; Archiv und Teil 8/s und 32/s), ganzzahlig in Tausendsteln, die
  Tabelle der Gegenstellen gedeckelt auf 1024, volle Eimer machen Platz.
  Darüber gibt es `Nichts`; `gedrosselt` steht in jeder Zustandsaufnahme.
  Dazu hält die Kette acht codierte Gruppen statt einer
  (`TEILGRUPPEN_BEHALTEN`), damit ehrliche Splitter einander nicht die
  Codierung wegnehmen.
- ⚑ **Fund 634: Eine erfundene Höhe hielt den Knoten für immer „nicht
  bereit“.** Die gehörte Höhe war eine Zahl, die nur wuchs, gespeist aus
  jedem abgelehnten Block, auch einem, der an die eigene Spitze anschliesst
  und nur eine falsche Höhe nennt; ein einziger solcher Block genügte, und
  der Knoten fragte dazu ohne Ende nach Blöcken, die es nicht gibt. Jetzt
  `gehoert.rs`: Die Höhe ist die Behauptung der Gegenstelle, die sie nannte;
  sie fällt, wenn diese ab einer behaupteten Höhe nichts liefert (danach
  zählen ihre Behauptungen fünf Minuten nicht), und mit ihrer Trennung.
  Gezählt werden nur noch Blöcke, die vorn nicht anschliessen, denn nur sie
  belegt oder widerlegt die Nachforderung.
- ⚑ **Fund 635: Neulinge ohne Grenze.** Eine Transaktion kostet nichts, und
  die Anmeldung als Miner auch nicht; mit frischen Schlüsseln liessen sich
  Mempool und Zustand ohne Grenze füllen. Jetzt kommen Absender ohne
  Guthaben, Einsatz und Anmeldung nur gedrosselt hinein: höchstens 32
  zugleich in den Mempool, höchstens 16 in einen Block. Ein neuer Knoten
  braucht eine Transaktion, die Anmeldung.
- ⚑ **J1: Die Abbuchung legt den Empfängerbeweis bei** (SHARED_TYPES
  0.49.0). Der Kontrakt trägt nur noch die Wurzel seiner Empfängerliste; der
  Knoten nimmt den Beweis für seine eigene Adresse aus der Vollmacht, die der
  Inhaber ausgestellt hat. Fehlt er, weist die Kette die Abbuchung ab, wie
  bisher bei einem Empfänger, der nicht auf der Liste steht.
- **Offen, mit Grund:** Ganz schliessen Fund 635 erst Gebühren (Richtung vom
  Projektinhaber festgelegt, ausgearbeitet in den offenen Entscheidungen).
- **Belege:** fünf neue Tests, je mit Gegenprobe:
  `wer_zurueckliegt_meldet_keine_falsche_luecke`,
  `eine_echte_luecke_wird_gemeldet` und
  `eine_erfundene_spitze_haelt_die_heilung_nicht_auf` (rot, wenn der Bezug
  die eigene Höhe ist, und rot ohne den Deckel);
  `der_wiederanlauf_setzt_beim_juengsten_abzug_an` (3700 Blöcke, Abzug bei
  3600, 100 übernommen, dieselbe Wurzel wie beim vollständigen Nachrechnen)
  und `ein_fremder_abzug_wird_verworfen` (rot ohne die Prüfung des Blocks
  und rot ohne die Prüfung des Dateiendes). Zu Fund 633
  `ein_block_bleibt_unter_seiner_bytegrenze`,
  `eine_uebergrosse_transaktion_haelt_den_block_nicht_auf`,
  `ein_absender_fuellt_den_mempool_nicht` (je rot ohne ihre Zeile); zu
  Fund 632 fünf Tests in `drossel.rs` (rot ohne den eigenen Eimer, ohne den
  für alle, mit zusammengelegten Arten und ohne das Aufräumen; der Test fürs
  Aufräumen kam dazu, weil die erste Gegenprobe dort nicht biss). Der Test
  zur Mempool-Grenze verteilt sich jetzt auf 64 Absender mit Guthaben. Zu
  Fund 634 vier Tests in `gehoert.rs` (je rot ohne Widerlegen, ohne Sperre,
  ohne Verfall bei Trennung, und wenn jedes Nichts widerlegte); zu Fund 635
  `neulinge_kommen_nur_gedrosselt_hinein` (rot ohne die Grenze im Mempool und
  ohne die im Block).
- **Gemessen auf echten Ketten:** Die Kettendatei von k11 aus dem vollen
  Archivlauf (367 363 Blöcke, Abzug bei 367 200) lädt in 1,3 s, 163 Blöcke
  nachgerechnet, auf die Endwurzel des Laufs `1bd4d53b08cab1ec`; vorher
  166 s. Die des kurzen Laufs (3797 Blöcke) mit Abzug in 36 ms, ohne die
  Abzüge vollständig in 258 ms, beide auf dieselbe Wurzel. Der kurze
  Archivlauf mit 0.60.0 bestand.
- **Im vollen Archivlauf** (drei Grenzen, Störer, zehn Abstürze): jeder
  Wiederanlauf ab dem eigenen Abzug, aufgeholt in 3,6 bis 15,5 s statt 18
  bis 208 s; keine ehrliche Kopie ersetzt; elf Knoten einig. Die
  Schlussphase war rot an einem Fehler des Laufskripts (TESTCLIENT 0.50.0,
  Fund 631), nicht des Knotens.

### v0.59.0 (2026-10-24): Mempool und eigene Nummern ohne Lücke (Fund 627)

Gefunden im vollen Archivlauf (zwölf ehrliche Knoten, sechs Sybil, Störer).
Nach der ersten Langzeitgrenze fielen die übernehmenden Knoten nach und nach
hinter den Erzeuger zurück; bei Höhe 175 138 des Erzeugers standen sie
zwischen 156 163 und 164 073, und der Lauf brach beim Aufholen nach einem
Absturz ab. Zugleich verloren ehrliche Archivkopien ihren Platz: 16 von 18
Minern standen am Ende unter `ersetzt`, die Kopien schrumpften von sieben auf
zwei. Eine Ursache mit drei Folgen:

- 📌 **Eine überholte Transaktion sperrte ihren Absender für immer.** Eine
  Kopie reicht Beleg (bis zu 27 KB) und Teilwurzel (wenige Bytes) im selben
  Augenblick ein, mit den Nummern n und n+1. Die kleine kommt beim Erzeuger
  zuerst an. Der Erzeuger legte den ganzen Mempool in den Block, n+1
  scheiterte dort an der Nummer und war verloren. Der Absender zählte weiter
  (`netz_nonce`), also galt keine seiner späteren Transaktionen mehr. Belegt
  im Protokoll von k6: der Beleg der Epoche 65 alle zwei Sekunden neu, die
  ganze Epoche lang, nie im Zustand.
- 📌 **Die Wiederholungen füllten die Mempools der übrigen Knoten.**
  `schon_gesendet` wartete acht Höhen, bei 25 ms je Block also nicht einmal
  bis zur nächsten Aufnahme. Ein Knoten, der keine Blöcke baut, nahm jede
  Transaktion auf und strich je Block per Bytevergleich, was der Block
  enthielt; eine, die nach ihrem Block ankam, blieb für immer liegen. Das
  Leck gab es schon vor dem Archiv, rund 50 je Minute; mit den Belegen lagen
  über 5000 im Mempool, und jeder Block serialisierte sie alle. Daran fielen
  die Knoten zurück.
- ⚑ **Erzeuger (`baue_block`):** In den Block kommt nur, was seine Nummer
  verbraucht hat (`anwenden` meldet das je Transaktion). Was an Unterschrift
  oder Nummer scheitert, hat den Zustand nicht angefasst, die Wurzel ist mit
  und ohne es dieselbe; der Konsens ändert sich nicht. Eine Transaktion mit
  späterer Nummer wartet im Mempool auf ihre Vorgängerin.
- ⚑ **Mempool:** `aufnehmen` lehnt ab, was unter der Nummer im Zustand liegt
  (sie wächst nur, also gilt so etwas nie mehr), und nimmt höchstens
  `MEMPOOL_HOECHSTENS` = 4096 auf. Nach jedem Block, gebaut oder übernommen,
  streicht `mempool_bereinigen` Verbrauchtes und alles, was länger als
  `WARTEN_HOECHSTENS_HOEHEN` = 64 Höhen wartet. Das ersetzt
  `streiche_verarbeitete` und seinen Bytevergleich.
- ⚑ **Absender (`knoten.rs`):** Steht die eigene Nummer im Zustand 64 Höhen
  still, während eigene Transaktionen ausstehen, zählt der Knoten ab der
  Nummer im Zustand neu (`netz_luecke_pruefen`, Zeile `netz_luecke` im
  Protokoll). Solange etwas aussteht, wartet ein neuer Versuch derselben Art
  64 statt 8 Höhen.
- 📌 **Ein Test hielt die alte Regel fest:**
  `eine_doppelt_eingereichte_transaktion_verschwindet_nur_einmal` verlangte,
  dass die zweite Kopie derselben Transaktion nach dem Block weiter wartet.
  Sie trägt dieselbe Nummer und hätte nie mehr gegolten; der Test heisst
  jetzt `…_faellt_mit_dem_block_ganz_heraus`.
- **Tests:** vier neue in `kette.rs` (überholende Transaktion wartet, der
  Block trägt nur Verbrauchtes und der Folger rechnet gleich, Mempool eines
  Folgers bleibt klein, Obergrenze). **Gegenprobe:** mit abgeschaltetem
  Filter im Block, ohne Nummernprüfung bei der Aufnahme und ohne Streichen
  nach Nummer werden die ersten drei rot. 361 Tests grün, Clippy mit
  `-D warnings` ohne Befund.

### v0.58.0 (2026-10-24): Archivknoten mit Beweis, Splittern und kalter Ausfuhr (Stufen A2 bis A9, Fund 626)

Festlegung des Projektinhabers vom 2026-10-23: Die Archivknoten sollen
„absolut bulletproof gegen Angriffe und Totalausfall“ sein, alle fünf
Schichten, Redundanz nach kritischer Prüfung. Gewählt: sieben über die Zonen
verteilte volle Kopien und Reed-Solomon-Splitter (32 aus 64) je Periode. Die
Regeln stehen im Konsens (SHARED_TYPES 0.48.0, CONSENSUS 0.51.0); hier, was
der Knoten damit tut.

- ⚑ **Historienwurzel im Zustand (A2):** `anwenden` hängt je Block den Hash
  des Vorgängers an (`LedgerState.historie`); Erzeuger und Übernehmer gehen
  denselben Weg. Neue Startwerte mit `-H1`, denn die Zustandswurzel ab
  Genesis ist eine andere. 📌 **Dabei gefunden:** Die Probe
  `eine_gefaelschte_unterschrift_bewegt_nichts` verglich die Wurzel vor und
  nach einem Block und setzte voraus, dass ein leerer Block den Zustand nicht
  bewegt; seit der Historie tut er das. Sie vergleicht jetzt mit einem
  Zwilling, der einen leeren Block baut.
- ⚑ **Die Rolle kommt aus dem Zustand (A3, A6):** `archiv.rs` rechnet nicht
  mehr selbst, sondern liest Kopie, Halter, Splitter oder „jeder behält“ aus
  `zustand.archiv`. Verdichtet wird nur, wer laut Zustand nicht hält. Bis
  zur ersten bestätigten Übergabe hält jeder.
- ⚑ **Nachladen mit Beweisen (A4, `archivabruf.rs`):** Eine Kopie mit Basis
  holt die Blöcke davor über `Nachforderung::Archiv { ab, bis, anzahl }`;
  jede Lieferung kommt mit einem MMR-Beweis für ihren letzten Block gegen
  die Historie der eigenen Basis, rückwärts legt jeder bewiesene Block seinen
  Vorgänger fest. Jede Lieferung wird für sich geprüft, Müll fällt bei der
  ersten auf, die Gegenstelle wird gesperrt; gefragt werden Halter und
  Kopien zuerst. Liefern kann jede Kette, die ab Genesis lebt: Sie führt
  einen Historienbaum mit (`Kette::archivlieferung`), nach dem Einfügen einer
  Geschichte wird er aus der Datei neu gebaut und gegen den Zustand
  geprüft. Das Nachladen beginnt an jeder Epochengrenze, sobald es nötig
  ist, nicht erst an der nächsten Langzeitgrenze: Eine Kopie, die mitten in
  der Periode nachrückt, wäre sonst wieder ersetzt, bevor sie anfängt.
- ⚑ **Belege (A5):** Eine Kopie belegt je Epoche selbst
  (`Anweisung::ArchivBelegen`, `Kette::archivbeleg`): die Blöcke ihrer
  Stichproben als Bytes und je ein Beweis gegen die Historienwurzel der
  Grenze. Ziehung, Abschluss je Epoche und Anweisung laufen in `anwenden`.
- ⚑ **Splitter (A7):** Kopien nennen die Teilwurzel (`ArchivTeilwurzel`),
  aus den Gruppenwurzeln aller fertigen Gruppen der Periode; Splitter holen
  ihren Teil jeder Gruppe (`Nachforderung::Teil`, `Kette::teilstueck`,
  codiert auf Anfrage aus 1024 Blöcken), prüfen ihn gegen die Teilwurzel,
  legen ihn ab (`teilablage.rs`, `<kette>.teile/`) und belegen je Epoche einen
  gezogenen (`ArchivTeilBelegen`). Teile früherer Abschnitte bleiben liegen.
- ⚑ **Kalte Ausfuhr und Einfuhr (A8, `archivausfuhr.rs`):** je Periode eine
  Datei und ein Verzeichnis mit Summen. `--archiv-ausfuhr <ordner>` schreibt
  jede fertige Periode, solange der Knoten die Kette ab Genesis hat;
  `myl-node archiv-ausfuhr <kette> <ordner>` und `myl-node archiv-einfuhr
  <ordner> <kette>` arbeiten am angehaltenen Knoten. Die Einfuhr prüft
  Summen, Höhen, Verkettung ab dem Startwert und die Anker des Netzes; in
  eine leere Kette, die der Start dann ab Genesis nachrechnet, oder vor eine
  verdichtete, geprüft gegen die Historie ihrer Basis.
- ⚑ **Stand je Epoche im Protokoll:** `archiv_stand` mit Rolle, Basis,
  Ziehung, Kopien, Vollständigen, Splittern, Haltern, Ersetzten und
  Teilwurzeln; dazu `archiv_beleg`, `archiv_teilwurzel`, `archiv_teilbeleg`,
  `archiv_teile_holen`/`_vollstaendig`, `archiv_geliefert`,
  `archiv_abgelehnt`, `archiv_ausfuhr`.
- ⚑ **Fund 626, Nachholen hing an einer Gegenstelle ohne Blöcke:** Eine
  Gegenstelle, die selbst zurücklag, antwortete sofort „nichts“, war damit
  als erste wieder frei und bekam jedes Mal den vordersten Bereich; die
  einzige, die ihn hatte, war mit späteren beschäftigt, die der volle
  Vorausspeicher verwarf. Gefunden im Archivlauf: Nach einem Totalausfall
  hatte nur ein Knoten die Kette, und zwei Einsteiger aus dem Abzug standen
  240 Runden lang auf derselben Höhe. Jetzt wird eine solche Gegenstelle für
  Bereiche ab dieser Höhe 30 s lang nicht gefragt (`LEER_SPERRE`). Gegenprobe:
  derselbe Lauf war vorher rot und ist jetzt grün.
- **Proben:** `tests/verdichtung.rs` belegt wie echte Kopien (jede Epoche,
  Schlüssel der Probekonten) und prüft dazu: eine Kopie lädt mit Beweisen
  nach und liefert danach selbst; Splitter holen, legen ab und belegen, aus
  beliebigen 32 der 64 Teile jeder Gruppe kommt die Geschichte mit derselben
  Historie zurück; eine kalte Ausfuhr kommt vor die verdichtete Kette, eine
  zu kurze wird abgewiesen. Dazu `archivabruf`, `archivausfuhr`,
  `teilablage` mit eigenen Proben. Gegenproben für jede Prüfung, alle rot.
  ⚠️ Die Proben mit Belegen brauchen je rund 100 s: Jeder Beweis liest die
  1024 Blöcke seiner Gruppe und hasht sie neu. Vorgemerkt: die Bytes aus der
  Datei hashen, ohne zu zerlegen und neu zu serialisieren.

### v0.57.0 (2026-10-23): Begrenzter Platz und Arbeitsspeicher, Verdichtung, ausgeloste Archivknoten (Funde 624, 625)

Aus dem ersten Dauerlauf (TESTCLIENT 0.48.0): Ein Knoten verlor Platz und
Arbeitsspeicher ohne Grenze. Festlegungen des Projektinhabers vom selben Tag:
den Arbeitsspeicher frei halten, so wenig Dateien wie möglich und so viele
wie nötig, ein Budget von „ein paar GB“, keine festen Archivknoten, aber
eine Handvoll ausgeloster, rotierender, damit auch ein Totalausfall aller
Knoten die Geschichte nicht kostet.

- ⚑ **Fund 625, Arbeitsspeicher:** Der Kettenspeicher führte zwei Verweise
  über jeden Block (`orte`, `nach_hoehe`), gemessen rund 45 Byte je Block
  (der Kommentar sagte acht), bei 1 s Blockzeit 1,4 GB im Jahr. Gefunden durch
  Messung von zwei Knoten über die Rückschleife mit und ohne Testverkehr: Der
  Testverkehr war unschuldig, nach dem Einschwingen wuchsen beide gleich um
  rund 45 Byte je Block. Jetzt ein **dünner Verweis**, eine Stütze je 1024
  Sätze (`STUETZABSTAND`); die Nachlieferung springt zur Stütze und liest
  vorwärts (`bloecke_ab`, Arbeit begrenzt durch Stützabstand und Spanne), der
  Neustart liest die Datei der Reihe nach. Probe: 5 000 Blöcke halten 5
  Stützen, jede Höhe wird gefunden.
- ⚑ **Fund 624, Betriebsprotokoll:** wuchs ohne Grenze (rund 740 Byte je
  Block, jede Gossip-Nachricht eine Zeile, je Start eine neue Datei, nie
  geräumt), bei geplanter Blockzeit 12 bis 23 GB im Jahr. Jetzt Rotation
  (`DATEI_HOECHSTENS` 16 MiB) und ein Deckel über alle Dateien des Knotens,
  auch früherer Starts (`--protokoll-mb`, Vorgabe 128); die älteste fällt
  zuerst. Eine neue Datei beginnt mit `fortsetzung` und dem Namen der
  vorigen, die Folge läuft weiter. 📌 **Dabei gefunden:** Die Probe war in der
  vollen Sammlung rot. Rotierte das Protokoll schneller als eine Millisekunde,
  bekam die neue Datei den Namen einer eben geräumten und wurde als Nächstes
  geräumt; jetzt liegt jeder neue Name streng über allen alten.
- ⚑ **Verdichtung nach Budget** (`--kette-mb`, Vorgabe 2048): An einer
  Langzeitgrenze, wenn die Kettendatei über dem Budget liegt, wird ein
  Langzeitabzug zur Basis, und die Datei behält nur die Blöcke danach. Gewählt
  wird der älteste, nach dem höchstens das halbe Budget liegt, **aber nie
  weniger als ab dem vorletzten Langzeitabzug**. Absturzfest in drei
  Schritten (neue Datei auf die Platte, Basis setzen, umbenennen); eine
  liegengebliebene `.neu` verwirft das Öffnen. Die eigene Basis gilt als
  bestätigt; Langzeitabzüge vor der Basis fallen weg. Ein Neustart rechnet ab
  der Basis nach statt ab Genesis.
- ⚑ **Archivknoten** (`src/archiv.rs`): je Periode (64 Epochen) die fünf
  registrierten Miner mit den kleinsten Losen
  `sha256("myl-archiv" ‖ Epochensaat ‖ Kennung)`, für jeden nachrechenbar.
  Wer in der laufenden oder vorigen Periode gezogen ist, verdichtet nicht;
  ohne registrierte Miner verdichtet niemand. Ein gezogener Knoten mit Basis
  holt die Blöcke ab Genesis über die Hintergrundprüfung, die sie dabei prüft
  und in `<kette>.archiv` aufbewahrt, und setzt sie danach vor seine Kette
  (`vorne_einfuegen`, erst umbenennen, dann Basis entfernen). Protokoll:
  `archiv`, `archiv_nachladen`, `archiv_vollstaendig`, `archiv_fehler`,
  `kette_verdichtet`. 📌 Entworfen war „jeder mit Wahrscheinlichkeit 5/N“;
  die Probe zeigte, dass dann etwa jede 150. Periode leer bliebe. Die fünf
  kleinsten Lose ergeben genau fünf.
- **Anker aus dem Quelltext** (`Netz::ist_anker`): Eine Basis genau dort
  braucht keine Hintergrundprüfung; sie verdient dasselbe Vertrauen wie
  Genesis.
- **Proben:** `tests/verdichtung.rs` an echten Langzeitgrenzen (345 600
  Blöcke in rund 3 s): ein nicht gezogener Knoten verdichtet an der zweiten
  und dritten Grenze und läuft aus der verdichteten Datei mit derselben
  Wurzel wieder an; ohne Miner verdichtet niemand; ein gezogener nicht; die
  Geschichte kommt zurück und ein Neustart ab Genesis trifft dieselbe Wurzel.
  Dazu Speicher (Verdichten, halbe Verdichtung), Protokoll (Deckel, frühere
  Starts, Namen), Auslosung (genau fünf in jeder von 200 Perioden, Reihenfolge
  egal). **Gegenproben**, je Behebung einzeln herausgenommen, alle rot:
  Rotation, steigende Namen, Stützabstand, Verdichtung, Archiv-Ausnahme, und
  in TESTCLIENT das Verbinden der Fortsetzungen.
- ⚠️ **Noch nicht im Netz belegt:** das Nachladen eines neu gezogenen
  Archivknotens über Gegenstellen. Die Bausteine sind geprüft
  (Hintergrundprüfung, Einfügen), der Weg über das Netz braucht einen Lauf
  mit registrierten Minern über mehrere Perioden.

### v0.56.1 (2026-10-23): Die Stakes der Stimmsatzdatei müssen zusammen in `u64` passen (Fund 619)

- ⚑ **Fund 619:** `Stimmsatzdatei::gesamtstake` rechnete die Summe in
  `u128` und schnitt sie mit `as u64` still ab. Jeder einzelne Stake
  passte beim Lesen, die Summe nicht zwingend. Zwölf Validatoren zu je
  `2^61` ergaben `1,5 · 2^64`, abgeschnitten `2^63`; jeder einzelne lag
  unter einem Drittel davon, also kam die Datei durch. Das `VotingSet`
  dazu sättigt sein Gesamtgewicht bei `u64::MAX`, und das Quorum lag bei
  zwei Dritteln davon statt der echten Summe: **weniger als zwei Drittel
  des wirklichen Gewichts hätten genügt.** Jetzt prüft `pruefe` die Summe
  vor der Drittelschranke und weist mit `GesamtstakeZuGross` ab;
  `gesamtstake` sättigt nur noch, statt umzulaufen.
- Test `eine_stakesumme_ueber_u64_wird_abgewiesen`, mit der Gegenrichtung
  (dieselbe Verteilung geviertelt geht durch, Gesamtgewicht stimmt).
  **Gegenprobe:** Prüfung abgeschaltet, der Test wird rot und zeigt die
  angenommene Datei.
- Hervorgegangen aus einem offenen Punkt des Audits („Genesis verteilt
  höchstens `u64::MAX`“). Der Ledger selbst braucht keine Zusicherung:
  `genesis_verteilung` nimmt die Gesamtmenge als `u64` und verteilt sie
  exakt, und `LedgerState::genesis` legt keine Konten an.

### v0.56.0 (2026-10-23): Sicherheitsaudit: Arbeit, die eine Anfrage überdauert, der Validatorschlüssel und das Nachholen (Funde 607, 609, 611, 612)

Aus dem Sicherheitsaudit vom 2026-10-23.

- ⚑ **Fund 609**, drei Wege, auf denen eine fremde Gegenstelle Arbeit
  auslöste, die ihre Anfrage überdauerte:
  1. **Zeitprobe:** jeder Gegenstelle beantwortet, je Probe ein Faden bis
     zu 120 s. Die Tür des Shards bedient nacheinander; eine Flut stand vor
     jedem echten Auftrag, der Miner verpasste seine Fristen, und **die
     ehrlichen Messenden unterschrieben echte Fristzeugnisse gegen ihn**.
     Jetzt nur für Mitglieder des eigenen Pods laut Kette
     (`frist::misst_mich`, `frist::messende`), eine zugleich.
  2. **Inferenzaufträge:** je Auftrag eine Aufgabe bis zur Frist des
     Ortsanschlusses (45 s), unbegrenzt viele, je bis zu 4 MiB. Jetzt
     höchstens `INFERENZ_IN_ARBEIT_HOECHSTENS` = 4, jeder weitere sofort
     abgelehnt; `Knoten::inferenzstand` zeigt bearbeitet und abgelehnt.
  3. **Zustandsabzug:** Jedes Stück einer anderen Höhe als der zuletzt
     gelesenen las die ganze Datei (bis 1 GiB), bildete ihre Summe und
     dekodierte den Zustand, in der Ereignisschleife; abwechselnd zwei
     Höhen angefragt hielt den Konsens an. Jetzt `Ablage::stueck_lesen`:
     Kopf, Block und genau das Stück, ungeprüft, denn der Abholer prüft
     gegen seinen Anker. Der Zwischenspeicher `abzug_zwischen` entfällt.
  Die Plätze für 1 und 2 sind ein `Platz`, der beim Freigeben zurückgeht,
  auch nach einer Panik.
  **Gegenproben:** `misst_mich`, das jedem antwortet, fällt am Fremden; ohne
  Grenze lehnt Alpha in 15 s keinen von zehn Aufträgen ab, über zwei echte
  Knoten mit schweigendem Shard (`eine_flut_von_inferenzauftraegen_...`);
  ein `Platz` ohne Rückgabe fällt in beiden Tests; der alte Leseweg liefert
  das erste Stück nicht, sobald ein Byte im letzten kippt.
- **Fund 611:** Der Knoten zieht je Start des Shards einen Ausweis für
  dessen Tür, legt ihn mit `0600` als `shardausweis` neben die Netzlast,
  gibt ihn mit `--ausweis` und räumt ihn mit dem Shard weg;
  `Shardfuehrung::tuer` liefert Adresse und Ausweis (statt `adresse`).
- **Fund 612:** Beim Nachholen von Modelldateien galt die Grösse, die die
  liefernde Gegenstelle mit jedem Stück neu ansagte, geprüft wurde erst am
  Ende; eine böswillige Quelle füllte die Platte. Jetzt gilt die Ansage des
  ersten Stücks, höchstens `MAX_DATEI_BYTES` = 8 GiB; eine masslose oder
  wechselnde Ansage ist eine falsche Lieferung. **Gegenprobe:** Ohne die
  Regel geht „ein Terabyte angesagt“ als `Weiter` durch.
- **Fund 607:** Der Validatorschlüssel wurde mit `fs::write` angelegt und
  danach eingeschränkt; jetzt über `myl_types::geheimdatei`.
- **Fund 599:** `ctutils` 0.4.5, `chacha20` 0.10.2 in der Sperrdatei.
- Nebenbei: drei Startmeldungen trugen eine Lücke aus rund zwanzig
  Leerzeichen (ein verlorenes `\` am Zeilenende).
- Belege: Clippy (`-D warnings`) und alle Tests grün (267 in der
  Bibliothek, dazu die Testdateien).

### v0.55.1 (2026-10-23): libp2p 0.57; Sicherheitsmeldungen in `hickory-proto`

- Die direkte Abhängigkeit `libp2p` auf 0.57 (nur `tokio`), passend zu
  `myl-net` 0.20.0; Sperrdatei nachgezogen. Anlass sind RUSTSEC-2026-0118
  und -0119 in `hickory-proto` 0.25.2 über `libp2p-mdns` 0.48; mit 0.57
  liegt `hickory-proto` 0.26.3 im Baum. Kein Code geändert.
- **Belege:** Clippy (`-D warnings`) und 327 Tests offline gegen den Vorrat
  grün; der Lauf mit getrennten Prozessen unter `tc netem` (`--kurz`)
  bestanden, Zahlen im Changelog von NETWORKING (0.20.0).

### v0.55.0 (2026-10-22): Der Zustandsabzug: Neulinge beginnen an einer Epochengrenze; Nachholen von mehreren Quellen

⚑ **Festlegung des Projektinhabers vom 2026-10-22**, nach einer Recherche im
Quelltext von vier Systemen: Anker im Quelltext des Netzes **und** vom
Betreiber, Prüfung im Ganzen gegen die Wurzel, Hintergrundprüfung als
Vorgabe an, mit Halt bei Abweichung.

- `abzug.rs` (neu): Ein Abzug ist der Block bei einer Epochengrenze und die
  Borsh-Form des Zustands danach. Mehr braucht die Anwendung der folgenden
  Blöcke nicht. Geglaubt wird nur ein **Anker** (Höhe und Blockhash), nie
  eine Gegenstelle: Der Block muss genau diesen Hash haben, die Bytes genau
  die Wurzel in seinem Kopf, und gelesen wird nur ein Zustand in
  kanonischer Form. `Zusammensetzen` nimmt Stücke nur in Folge, hält den
  Block beim ersten gegen den Anker und deckelt die angekündigte Größe
  (1 GiB), bevor Speicher belegt wird.
- **Ablage** neben der Kettendatei (`kette.dat.abzuege/`): An jeder
  Epochengrenze legt ein Knoten einen Abzug ab, beim Bauen wie beim
  Übernehmen. Er behält die zwei jüngsten, jeden 64. auf Dauer (Anker im
  Quelltext dürfen nur auf solche zeigen, eine Probe hält das fest) und
  seine **Basis**. Geschrieben über eine Zwischendatei, mit SHA-256 über die
  ganze Datei.
- **Holen und Liefern** über den Anfragekanal (`Nachforderung::Abzug`,
  `Nachlieferung::Abzug`, angehängt): Stücke bis 1 MiB, so viel weniger,
  dass Block und Rahmen in eine Antwort passen. Eine Gegenstelle, die falsch
  oder nichts liefert, wird für den Abzug gesperrt; sind alle gesperrt,
  beginnt es nach einer Minute neu. Solange ein Abzug geholt wird, holt der
  Knoten nicht von Genesis nach.
- **Start:** Ein Knoten ohne eigene Kette mit Anker (`--abzug
  <hoehe>:<hash>`, sonst `Netz::abzuege`, heute in beiden Netzen leer) holt
  den Abzug, setzt die Kette darauf (`Kette::auf_abzug_setzen`) und legt
  ihn als Basis ab; die Kettendatei hält danach nur die Blöcke nach H. Ein
  Neustart beginnt bei der Basis und spielt die Datei nach.
- **Hintergrundprüfung** (Vorgabe an, `--ohne-hintergrundpruefung`): Eine
  zweite Kette ab Genesis holt die Blöcke bis H und rechnet nach. Treffen
  Hash und Wurzel bei H, ist die Basis bestätigt (`abzug_bestaetigt`, auch
  auf der Platte); treffen sie nicht, **hält der Knoten an**
  (`abzug_widerlegt`, Rückgabewert 3): Weitermachen hieße, auf einem fremden
  Zustand zu rechnen und ihn anderen zu liefern.
- Das Protokoll meldet jeden eigenen Abzug mit vollem Anker
  (`abzug_angelegt`, `anker: "<hoehe>:<hash>"`); aus dieser Zeile entsteht
  ein Anker für andere.
- 📌 **Nachholen von bis zu vier Gegenstellen zugleich** (`NACHHOLER`), je
  Gegenstelle eine Nachforderung, über aufeinanderfolgende Bereiche zu 512
  Blöcken. Bis dahin lief eine zur Zeit, und über eine Leitung mit 200 ms und
  5 % Verlust dauerte eine Lieferung rund 6 s: Eine einzelne Verbindung
  bricht unter Verlust ein, mehrere addieren sich. Was vor seiner Zeit
  ankommt, nimmt der Vorausspeicher (jetzt 2 048 Blöcke, vier Lieferungen);
  Gegenstellen, die gerade für einen Abzug oder die Hintergrundprüfung
  gefragt sind, bleiben frei, denn die Antworten tragen keine Marke.
  `tests/nachholen.rs`: Delta holt 2 500 Blöcke bei Alpha und Beta zugleich
  nach und endet mit derselben Wurzel (Gegenprobe mit einer Nachforderung
  zur Zeit: rot). Im Lauf mit getrennten Prozessen bei 100 Blöcken je
  Sekunde: ein Neuling holt 8 019 Blöcke in 33 s nach (rund 243 je Sekunde);
  vorher kam er mit rund 75 je Sekunde nie an.
- 📌 **Die Bereiche beginnen beim kleinsten ungedeckten Block**
  (`nachschub::naechster_bereich`). Die erste Fassung plante hinter dem
  höchsten laufenden Bereich; ging die vorderste Lieferung verloren, schloss
  niemand mehr die Lücke davor, und ein Knoten blieb nach einem Absturz bei
  1 217 von 5 926 stehen, während alles Spätere im Vorausspeicher lag.
  Gefunden im Lauf mit getrennten Prozessen, vor jeder Auslieferung; Probe
  `der_naechste_bereich_schliesst_zuerst_die_vorderste_luecke`, Gegenprobe
  mit der alten Regel rot.
- **Die Hintergrundprüfung holt aus zwei Quellen zugleich**
  (`HINTERGRUND_ZUGLEICH`, die Hälfte von `NACHHOLER`, damit das Nachholen der
  eigenen Kette Gegenstellen behält), mit derselben Planung
  (`naechster_bereich`) und einem eigenen Vorausspeicher; je Anfrage eine
  Zeile `hintergrund_angefordert`. Im Lauf mit getrennten Prozessen bei 100
  Blöcken je Sekunde: dieselben 7 200 Blöcke in 55,2 s statt 144,3 s.
- `abzug_nichts` im Protokoll, wenn eine Gegenstelle den Abzug des Ankers
  nicht hat: Ein Anker auf einer gewöhnlichen Grenze ist nach zwei Epochen
  geräumt, und bis dahin zeigte das Protokoll nur wiederholte Anfragen.
- **Belege:** fünf Proben in `abzug.rs`, eine in `netz.rs`;
  `tests/abzug.rs`: (1) Beta mit Anker übernimmt bei 1 800, holt die Spitze
  ein, die Hintergrundprüfung bestätigt, nach einem Neustart dieselbe Höhe
  und Wurzel ohne Netz; Gamma mit falschem Anker übernimmt nichts und holt
  auch nicht von Genesis. (2) ⛔️ Delta mit dem Anker einer anderen Kette,
  deren frühe Blöcke niemand liefert, rechnet die frühen Blöcke der ersten
  nach und **hält an**. Gegenprobe ohne den Vergleich am Ziel: Delta hält
  nicht an, die Probe wird rot. Im Lauf mit getrennten Prozessen
  (TESTCLIENT): ein Neuling aus dem Abzug bei 1 800 (6 325 Bytes Zustand)
  an der Spitze und bestätigt in 34,8 s, am Ende eine Wurzel mit allen.
  `cargo test` rc=0, Clippy still.

### v0.54.0 (2026-10-22): Blocktakt, Vorausspeicher, Nonce des Testverkehrs; Funde 592 und 593

⚑ **Gefunden im ersten Lauf mit getrennten Prozessen unter echten
Netzbedingungen** (jeder Knoten in einem eigenen Netz-Namensraum, 50 bis
200 ms Laufzeit je Richtung, 1 bis 5 % Verlust, 20 Blöcke je Sekunde). Der
Lauf im selben Prozess hatte beides nicht zeigen können: keine Laufzeit,
also keine Vertauschung; und er prüfte die Wurzel mit Transaktionen, die er
selbst ab Nonce 0 schickte.

- 📌 **Fund 592: Ein Block, der zu früh kam, wurde verworfen.** Gossip
  liefert ohne Reihenfolge; ein Block über der nächsten erwarteten Höhe
  schloss nicht an und fiel weg, der Knoten forderte nach. Bis die
  Nachlieferung da war, war die Spitze weiter, und so blieb jeder
  Nichterzeuger dauerhaft zurück: **4 von 2 168 Blöcken kamen über Gossip**,
  alle übrigen über das Nachholen, der Rückstand im Betrieb lag bei 28 bis 52
  Blöcken. Neu: `voraus.rs`, der **Vorausspeicher**. Er hebt Blöcke über der
  nächsten Höhe auf (höchstens 4 096 Höhen voraus, 4 Kandidaten je Höhe,
  1 024 Blöcke und 32 MiB zusammen; voll weicht der fernste) und spielt sie
  ein, sobald die Kette anschliesst, über Gossip oder über eine
  Nachlieferung, mit derselben Prüfung wie jeden anderen Block. Danach: 783
  bis 1 044 Blöcke je Knoten über Gossip, 14 bis 37 nachgeholt, Rückstand 0
  bis 1 Block. Die Aufnahme meldet `voraus`, die Nachlieferung
  `aus_dem_voraus`.
- 📌 **Fund 593: Der Testverkehr bewegte den Zustand nie.** Seine
  Transaktionen trugen als Nonce den Zähler des Testverkehrs, und der steht
  beim ersten Senden schon bei 1; die Kette verlangt die Nonce streng ab 0.
  Jede wurde verworfen, über 1 342 Blöcke blieb die Wurzel dieselbe, und
  der Vergleich der Wurzeln, für den es den Testverkehr gibt, belegte
  nichts. Die Probe in `tests/neustart.rs` rief `sende_transaktion` mit
  Zähler 0 und sah es deshalb nicht. Jetzt kommt die Nonce aus dem Zustand
  und wird fortgezählt, solange Gesendetes unterwegs ist; kommt 32 Blöcke
  lang nichts davon an, beginnt sie wieder beim Zustand
  (`testverkehr_nonce`). Danach 137 verschiedene Wurzeln auf 1 115 Blöcken.

- `--blocktakt <ms>` (`KnotenKonfig::blocktakt_ms`): Mit `--erzeuger` baut
  der Knoten in diesem Takt, unabhängig vom Testverkehr. Bisher baute er
  nur im Takt des Testverkehrs, in ganzen Sekunden; bei 1 800 Blöcken je
  Epoche brauchte ein Lauf mit getrennten Prozessen so eine halbe Stunde je
  Epoche. Mit Blocktakt prüft der Testverkehr nur noch, dass die Höhe seit
  dem vorigen Mal gewachsen ist (`Probe::Blockkette`), und schickt daneben
  seine Proben.
- 📌 **Fällig ab dem geplanten Zeitpunkt, und ohne Nachschiessen.** Ein
  verspäteter Block verschiebt die folgenden nicht; hinkt der Erzeuger mehr
  als einen Takt hinterher (etwa zwischen zwei Scheiben eines Laufs), setzt
  er neu auf, statt die verpassten Blöcke in einem Schwall zu bauen. Die
  Gegenprobe ohne diese Zeile baute 76 statt höchstens 51 Blöcke in einer
  Sekunde.
- `KonfigFehler::BlocktaktOhneErzeuger` und `BlocktaktNull`: ein Takt ohne
  Erzeuger hat keinen Gegenstand, einer von null baute ohne Pause.
- „Erst bauen, wenn jemand zuhört“ steht jetzt an einer Stelle
  (`block_wenn_jemand_hoert`), für den Testverkehr und den Blocktakt.
- `--lokale-suche`: mDNS auch in einem Netz, das sie nicht vorgibt (die
  Probekette); Startwert und Podmodell bleiben die des Netzes.
- **Belege:** `der_blocktakt_baut_im_eigenen_takt` (`tests/zwei_knoten.rs`:
  höchstens 51 Blöcke in der Sekunde nach einer Pause, Beta holt auf, eine
  Wurzel; ohne Blocktakt Höhe null), `der_blocktakt_gehoert_zum_erzeuger`;
  vier Proben in `voraus.rs` (nur was voraus liegt, Deckel je Höhe, voll
  weicht der fernste, Räumen und Bytes); zwei für `testverkehr_nonce`. Der
  Lauf mit getrennten Prozessen (TESTCLIENT) ist grün, und seine Gegenprobe
  ohne das Aufheben wird rot (k1: 0 Blöcke über Gossip, 1 399 nachgeholt).
- `rust-version` auf 1.99 in `myl-node`; die Werkzeugkette steht einmal, in
  `rust-toolchain.toml` (1.99.0), mit der auch die CI baut. Nur das Manifest.

### v0.53.0 (2026-10-22): Der Einstieg ins Netz ohne Handangabe

- `netz.rs`: `Netz::einstieg` (Adressen langlebiger Knoten, die jeder Knoten
  dieses Netzes beim Start kennt; kein Server, nur eine erste Tür) und
  `Netz::lokale_suche` (mDNS). Probekette: leer, aus. Testnetz int4: an, die
  Liste vorerst leer, denn noch gibt es keinen langlebigen Knoten; außerhalb
  des lokalen Netzes braucht der Einstieg bis dahin eine Bootstrap-Angabe.
- `KnotenKonfig::einstiegsliste()`: eigene Bootstrap-Angaben, danach die
  Einstiegsliste des Netzes, ohne Doppelte; `netz()` reicht beides und die
  lokale Suche an `myl-net` (0.18.0).
- 📌 **Nachholen: ein Deckel in Bytes, und 512 statt 64 Blöcke je
  Lieferung** (`nachschub::in_die_antwort`, `passende_anzahl`). Die Grenze
  zählte Blöcke, ihre Herleitung sprach von Bytes: Ein echter Block darf so
  groß sein wie die ganze Antwort (4 MiB), 64 davon passten nicht hinein,
  und das Nachholen bliebe an der ersten vollen Strecke stehen. Jetzt geht
  in eine Antwort, was passt, mindestens ein Block; der Nachfragende setzt
  an der Höhe danach fort. `VERLAUFSFENSTER` steht fest bei 256 (es hing am
  Vierfachen der Lieferung und wäre sonst 2048 geworden).
- **Belege:** `tests/entdeckung.rs` +2 (ohne Angabe im lokalen Netz;
  Einstiegsliste ergänzt die eigenen Angaben); `eine_lieferung_passt_in_die_antwort`
  (neu); der Deckel jetzt mit Kettendatei geprüft, denn er liegt über dem
  Fenster. `cargo test` rc=0 (309). Im Netzlauf mit Neuling 51 s und 58 s
  statt 92 s und 99 s.

**Ältere Einträge** (v0.52.1 und davor, 65 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- NODE/README/README.md`.
