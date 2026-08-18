# networking (`myl-net`)

> **Version:** 0.2.5
> **Datum:** 2026-08-18
> **Status:** 🎉 **Phase 1 + 2 vollständig** (Punkte 1.1–1.4, 2.1–2.3).
> Phase 1: 20-Node-Voll-Konnektivität < 5 s, ungültige Nachrichten
> werden nicht weiterverbreitet. Phase 2: Paarlatenzmessung mit
> EMA-Glättung, Latenz-Atteste, LatencyGraph, Geo-/AS-Diversität.

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


### Audit-Block 5 – 2026-08-18 (Warnungsfreiheit, Tests, Float-Audit)

Repository-weiter Block; die Einzelheiten stehen im jeweiligen Fahrplan.

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


### v0.2.4 – 2026-08-18 (Audit-Block 4: Gossip-Validierung vervollständigt)

**Fund A12 — die Strukturprüfung war auf PoI-Bündel beschränkt.**
`validate_payload()` prüfte für Blöcke, Transaktionen, Challenges und
Latenz-Atteste nur die Größe, mit dem Kommentar „die zugehörigen Typen
entstehen in CONSENSUS/VERIFICATION bzw. in Phase 2". Diese Typen
existierten längst (myl-consensus v0.4.0, myl-verifier v0.2.6,
`myl_types::LatencyAttest`) — der Kommentar war veraltet, und jede
Bytefolge unterhalb des Limits wurde weiterverbreitet.

Behoben:
- **Challenges** werden gegen `myl_types::Challenge` deserialisiert und
  strukturell geprüft (verschiedene Miner, verschiedene Hashes) — das
  ist alles, was ohne Kenntnis der Segment-Spur entscheidbar ist.
- **Latenz-Atteste** werden gegen `myl_types::LatencyAttest`
  deserialisiert und feldgeprüft.
- **Blöcke und Transaktionen bleiben bewusst bei der Größenprüfung.**
  Ihre Typen liegen in `myl-consensus` (L1); `myl-net` ist L0 und darf
  nicht an die Konsensschicht hängen, sonst kehrt sich die Schichtung
  um. Stattdessen neuer Trait `PayloadValidator` + `report_with()`:
  die Node-Verdrahtung, die beide Seiten kennt, reicht die
  vollständige Prüfung herein. Das ist eine dokumentierte Entscheidung,
  keine Auslassung.
- **Weiterhin offen und bewusst so:** Diese Schicht prüft keine
  BLS-Signaturen. Ein Latenz-Attest trägt eine, deren Gültigkeit aber
  nur gegen die Validator-Registry entscheidbar ist — also ebenfalls
  über `PayloadValidator`.
- 31 → 38 Tests.

### v0.2.3 – 2026-08-17 (Phase 2.3: Geo-/AS-Diversitäts-Metadaten)
- Geo-/AS-Diversitäts-Metadaten in SHARED_TYPES `node_metadata.rs`:
  GeoRegion (7 Regionen: NorthAmerica, SouthAmerica, Europe, Africa,
  Asia, Oceania, MiddleEast), Asn (32-bit ASN), NodeMetadata,
  DiversityChecker für Pod-Bildung (Kap. 4.4). 7 Tests grün.

### v0.2.2 – 2026-08-17 (Phase 2.2: Latenz-Atteste + LatencyGraph)
- Latenz-Atteste und LatencyGraph in SHARED_TYPES `latency_attest.rs`:
  LatencyAttest (signierte Latenzwerte), LatencyGraph (ungerichteter
  Graph mit Cleanup), PeerIdBytes, BlsSignatureBytes. 8 Tests grün.

### v0.2.1 – 2026-08-17 (Phase 2.1: Paarlatenzmessung)
- Paarlatenzmessung mit EMA-Glättung in `latency.rs`: Ping/Pong-
  Nachrichten, LatencyTracker mit EMA (α = 0,25), Cleanup-Mechanismus
  für veraltete Pings. 8 Tests grün.

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
