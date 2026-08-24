# NODE — der Myelith-Knoten

> **Version:** myl-node v0.1.0
> **Datum:** 2026-08-24
> **Status:** Netzknoten lauffähig, keine Blockproduktion

## Aufgabe

Die Verdrahtung, die aus den Protokoll-Bibliotheken ein laufendes
Programm macht: Identität, Konfiguration, Netzanbindung über `myl-net`
(L0), Nutzlastprüfung gegen `myl-consensus` (L1) und ein
Betriebsprotokoll für die nachträgliche Fehlersuche.

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

**Ist nicht:** ein Blockproduzent. Die Zustandsmaschinen in
`myl-consensus` sind vollständig, aber **niemand treibt sie über die
Zeit**. Dafür fehlen Rundentakt, Mempool und Kettenzustand. Sie
vorzutäuschen wäre genau die Sorte Häkchen, gegen die dieses Projekt
seine Regeln geschrieben hat.

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
        ├── knoten.rs         Start, Ereignisschleife, Zustandsaufnahme
        └── main.rs           Kommandozeile
    └── tests/
        └── zwei_knoten.rs    zwei Knoten, echte Sockets, Protokoll
                              zurückgelesen (5 Tests)
```

## Changelog

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
