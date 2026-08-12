# networking (`myl-net`)

> **Version:** 0.1.4
> **Datum:** 2026-08-13
> **Status:** 🎉 **Phase 1 vollständig** (Punkte 1.1–1.4,
> Akzeptanzkriterien empirisch erfüllt: 20-Node-Voll-Konnektivität
> < 5 s, ungültige Nachrichten werden nicht weiterverbreitet).
> Design-Entscheidungen und Quantum-Einordnung im Fahrplan; als
> Nächstes folgt Phase 2 (Latenztopologie).

P2P-Gossip, latenzbasierte Topologie-Erkennung, verschlüsselte
Aktivierungs-Streams. Referenzimplementierung von Whitepaper Kap. 3.2
(L0 Networking Layer), Grundlage für die Latenzmessung aus Kap. 4.1/4.3.

## Aufgabe

Die unterste Schicht (L0): Peer-Discovery, Gossip-Verbreitung von
Blöcken/Transaktionen/Attestierungen, kontinuierliche Paarlatenzmessung für
die Pod-Bildung sowie Ende-zu-Ende-verschlüsselte Punkt-zu-Punkt-Kanäle für
Aktivierungs-Streams zwischen Shard-Minern (Kap. 9.2).

## Abhängigkeiten

Nur SHARED_TYPES (Nachrichtenformate); parallel zu INTEGER_LLM möglich.

## Struktur

```
NETWORKING/
├── README/                   diese Kurzübersicht + Fahrplan
└── myl-net/                  die L0-Netzwerk-Crate (Bibliothek)
    └── src/
        ├── lib.rs             Crate-Wurzel: #![deny(unsafe_code)], Design-Doku
        ├── config.rs          entschiedene Parameter (Ping 15 s, EMA α=1/4
        │                      als Festkomma, Attest 5 min, Größenlimits)
        ├── identity.rs        Node-Identität: Ed25519-Keypair, PeerId,
        │                      Datei-Persistenz (load_or_create)
        ├── node.rs            Swarm-Aufbau: Gossipsub + Kademlia + Identify
        │                      + Ping über TCP/Noise/Yamux
        ├── discovery.rs       Peer-Discovery: Bootstrap-Peers parsen und
        │                      anwählen, Kademlia-Bootstrap (/myelith/kad/1)
        ├── gossip.rs          Gossip-Topics (Blöcke, Transaktionen,
        │                      PoI-Bündel, Challenges, Latenz-Atteste),
        │                      Subscribe/Publish mit Borsh-Payloads
        ├── validation.rs      Nachrichtenvalidierung vor Weiterverbreitung:
        │                      Größenlimits je Topic, Borsh-Strukturprüfung,
        │                      Accept/Reject an Gossipsub
        └── runtime.rs         Node-Event-Loop: Kommandos (Publish,
                               PeerCount), Ereignisse (Listen-Adresse,
                               validierte Nachrichten)
└── tests/
    └── testnet.rs             Akzeptanztests: 20-Node-Voll-Konnektivität
                               < 5 s, adversarialer Nicht-Weiterverbreitungs-
                               Test
```

## Changelog

### v0.1.4 – 2026-08-13 (Punkt 1.4) — Phase 1 vollständig
- Dreistufige Validierung vor Weiterverbreitung: Gossipsub-Authentizität
  (`ValidationMode::Strict` — unsignierte/imitierte Nachrichten scheitern
  auf Protokollebene), Größenlimits je Topic (Blöcke 2 MiB, PoI-Bündel
  512 KiB, Transaktionen/Challenges 64 KiB, Latenz-Atteste 4 KiB —
  später Governance-Parameter), Borsh-Strukturprüfung für Topics mit
  myl-types-Typ (aktuell PoI-Bündel).
- Gehaltene Nachrichten (`validate_messages()`): nichts wird
  weiterverbreitet, bevor `validation::report` es freigibt; `Reject`
  senkt den Gossipsub-Peer-Score des Absenders (Spammer-Isolation).
- Node-Event-Loop (`runtime::run_node`): Kommandos (Publish mit
  Ergebnis-Rückmeldung, PeerCount) und Ereignisse (Listen-Adressen,
  validierte Nachrichten) über Kanäle.
- **Akzeptanzkriterien Phase 1 erfüllt:** 20 lokale Nodes, Voll-
  Konnektivität über Gossip in < 5 s; adversarialer Node: ungültige
  Nutzlast wird vom Zwischen-Node verworfen und erreicht den dritten
  Node nicht, gültiger Verkehr läuft weiter. 23 Tests grün, keine
  Warnungen.

### v0.1.3 – 2026-08-13 (Punkt 1.3)
- Gossip-Topic-Struktur: fünf Topics mit versioniertem Namensschema
  (`/myelith/blocks/1`, `/myelith/transactions/1`, `/myelith/poi-bundles/1`,
  `/myelith/challenges/1`, `/myelith/latency-attests/1`) — Konsens-Feld,
  Änderung nur über Governance; das Latenz-Topic wird ab Phase 2 genutzt.
- Payload-Konvention: Borsh-Serialisierung der zugehörigen
  `myl-types`-Datentypen (kanonisch, bitstabil — Voraussetzung für alle
  Hashes/Signaturen über Nachrichten).
- `subscribe`/`subscribe_all`/`publish` mit benannten Fehlern
  (Subscribe-, Serialisierungs-, Publish-Fehler).
- End-to-End-Test: zwei Nodes, Node B publiziert ein echtes `PoIBundle`
  auf dem PoI-Bündel-Topic, Node A empfängt dieselben Borsh-Bytes — grün.
  17 Tests grün, keine Warnungen.

### v0.1.2 – 2026-08-13 (Punkt 1.2)
- Peer-Discovery: Kademlia-DHT unter dem Myelith-eigenen Protokoll-Namen
  `/myelith/kad/1` (Protokoll-Isolation — kein Mitsprechen in fremden
  Kademlia-Netzen auf demselben Port; Konsens-Feld).
- Bootstrap-Peer-Parsing mit sauberer Fehlerbehandlung (ungültige
  Multiaddr, fehlender `p2p/…`-Anteil), `bootstrap_from_config`
  (leere Liste zulässig — der erste Node eines Netzes hat keine
  Bootstrap-Peers) und `start_bootstrap` (mit `NoKnownPeers` als
  dokumentiertem Normalfall des ersten Nodes).
- Akzeptanznaher Test: Zwei lokale Nodes verbinden sich über Bootstrap
  und Kademlia innerhalb von 15 s — grün. 14 Tests grün, keine Warnungen.

### v0.1.1 – 2026-08-13 (Punkt 1.1)
- Crate-Grundgerüst `myl-net` auf rust-libp2p 0.56: Swarm mit Gossipsub
  (signierte Nachrichten, `max_transmit_size` 4 MiB), Identify
  (`myelith/0.1`) und Ping (Intervall = entschiedene 15 s) über
  TCP + Noise + Yamux.
- Node-Identität: Ed25519-Keypair mit PeerId-Ableitung und
  Datei-Persistenz (`load_or_create`, Protobuf-Kodierung). Quantum-Vermerk:
  Ed25519-Identitäten sind Shor-anfällig, PeerId-Ableitung hash-basiert —
  derselbe dokumentierte Migrationshorizont wie BLS/ECVRF.
- Konfiguration: alle Latenz-Parameter als Ganzzahl-Konstanten
  (EMA-Glättung als Festkomma 1/4, keine Gleitkomma-Arithmetik).
- 9 Tests grün, keine Warnungen.
