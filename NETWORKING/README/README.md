# networking (`myl-net`)

> **Version:** 0.1.1
> **Datum:** 2026-08-13
> **Status:** Design-Entscheidungen getroffen (rust-libp2p, Latenzmessung
> 15 s/EMA/Attest alle 5 min, zwei Verschlüsselungsschichten mit
> verpflichtender Session-E2E — Details und Quantum-Einordnung im
> Fahrplan), Phase 1 in Umsetzung; Punkt 1.1 abgeschlossen

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
        └── node.rs            Swarm-Aufbau: Gossipsub + Identify + Ping
                               über TCP/Noise/Yamux
```

## Changelog

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
