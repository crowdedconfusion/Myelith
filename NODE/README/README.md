# NODE — der Myelith-Knoten

> **Version:** 0.73.0
> **Datum:** 2026-10-27
> **Status:** Netzknoten lauffähig, Blockproduktion mit **Persistenz über
> Neustarts**, BFT-Runden über das Netz mit Rundenwechsel, und seit dem
> 1. September **schließt der Knoten die Epoche selbst ab**.
> **384 Tests grün** (gezählt am 2026-10-26).
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
