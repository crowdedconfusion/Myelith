# networking (`myl-net`)

> **Version:** 0.3.0
> **Datum:** 2026-08-23
> **Status:** 🎉 **Phase 2 abgeschlossen** (Punkte 1.1–1.4, 2.1–2.3),
> dazu Punkt 4.2 (Fuzzing der Wire-Protocol-Parser).
> Phase 1: 20-Node-Voll-Konnektivität < 5 s, ungültige Nachrichten
> werden nicht weiterverbreitet. Phase 2: Paarlatenzmessung mit
> EMA-Glättung, Latenz-Atteste, LatencyGraph, Geo-/AS-Diversität.
> 37 + 14 Tests grün.
>
> ⚑ **Fund 44:** Die Latenz-EMA rechnete in `f64`, obwohl der Kopf des
> Crates seit dem ersten Tag Festkomma zusagt. Der Gleitkomma-Audit
> konnte es nicht finden, weil `myl-net` mit **keiner Datei** in seiner
> Liste stand. Beides behoben.

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
        ├── node.rs            Swarm-Aufbau: Verbindungsgrenzen +
        │                      Adressvielfalt + Gossipsub (mit Peer-Scoring)
        │                      + Kademlia + Identify + Ping über
        │                      TCP/Noise/Yamux
        ├── limits.rs          Verbindungsgrenzen (Fund 53): getrennte
        │                      Budgets ein-/ausgehend, je Peer, je
        │                      Adressbereich (IPv4 /24, IPv6 /64)
        ├── scoring.rs         Gossipsub-Peer-Scoring: IP-Kolokation,
        │                      Verhaltensstrafe, Graylist-Schwellen
        ├── anfrage.rs         Punkt-zu-Punkt-Anfragen (/myelith/anfrage/1):
        │                      längenpräfixierter Byte-Codec, undurchsichtige
        │                      Nutzlast, 4-MiB-Grenze für beide Richtungen
        ├── nat.rs             NAT-Überwindung: Relais-Horchadressen,
        │                      Konfigurationsprüfung, Erkennung
        │                      vermittelter und QUIC-Adressen
        ├── discovery.rs       Peer-Discovery: Bootstrap-Peers parsen und
        │                      anwählen, Kademlia-Bootstrap (/myelith/kad/1)
        ├── gossip.rs          Gossip-Topics (Blöcke, Transaktionen,
        │                      PoI-Bündel, Challenges, Latenz-Atteste),
        │                      Subscribe/Publish mit Borsh-Payloads
        ├── validation.rs      Nachrichtenvalidierung vor Weiterverbreitung:
        │                      Größenlimits je Topic, Borsh-Strukturprüfung,
        │                      Accept/Reject an Gossipsub
        └── runtime.rs         Node-Event-Loop: Kommandos (Publish,
                               PeerCount, Dial), Ereignisse (Listen-Adresse,
                               validierte Nachrichten); run_node_mit()
                               reicht den PayloadValidator herein
└── tests/
    ├── testnet.rs             Akzeptanztests: 20-Node-Voll-Konnektivität
    │                          < 5 s, adversarialer Nicht-Weiterverbreitungs-
    │                          Test
    ├── adversarial.rs         Fuzzing der Gossip-Parser (14 Tests)
    ├── eclipse_sybil.rs       Verbindungsgrenzen gegen Flut, freies
    │                          ausgehendes Budget (5 Tests)
    └── nat.rs                 Relais-Pfad: Knoten ohne wählbare Adresse
                               wird über das Relais erreicht (5 Tests)
```

## Changelog

### v0.6.0 – 2026-08-24 (Punkt 1.5: Anfragekanal)

`/myelith/anfrage/1`, ein Punkt-zu-Punkt-Kanal für Nachforderungen.
Gossip verbreitet an alle; „schick mir das noch einmal" gehört an
**einen**.

**Die Nutzlast bleibt undurchsichtig.** Der Kanal trägt Bytes und weiß
nicht, was ein Block ist. Stünde hier ein `Blockanfrage`-Typ, wäre die
Schichtung umgekehrt. Was die Bytes bedeuten, entscheidet die Anwendung.

`InboundMessage` trägt jetzt `von`, den letzten Weiterleiter (nicht den
Urheber). **Ohne dieses Feld war eine Nachforderung nicht
adressierbar.**

### v0.5.0 – 2026-08-24 (Punkt 3.4: NAT-Überwindung)

AutoNAT v2, Circuit Relay v2, DCUtR und **QUIC** als zweiter Transport.
Vorher sprach der Stack nur TCP ohne NAT-Behandlung: Ein Knoten hinter
einem Heimrouter konnte hinaus wählen, aber niemand konnte ihn
anwählen.

**Warum QUIC dazugehört:** Lochstanzen über TCP („simultaneous open")
scheitert an vielen verbreiteten NAT-Bauarten; über UDP gelingt es
verlässlich. TCP allein wäre ein Stack, der DCUtR enthält und bei dem
das Lochstanzen trotzdem oft scheitert.

**Warum das mehr ist als Bequemlichkeit:** Ein Netz, in dem nur
öffentlich erreichbare Knoten mitmachen können, ist kleiner und in
wenigen Händen. Die Kollusionsrechnung aus Anhang B.2 hängt daran, dass
β klein bleibt; wer Heimanschlüsse ausschließt, treibt β nach oben.

⚑ **Fund 56: Ein Relais ohne eigene Adresse ist keins.** Erster Entwurf:
ein Schalter `dient_als_relais: bool`. Das Relais nahm Reservierungen
**an** und antwortete **ohne Adressen** (`NoAddressesInReservation`), weil
es nur Adress-*Kandidaten* hatte und keine bestätigten. Seitdem verlangt
`NatKonfig` für den Relais-Dienst eine öffentliche Adresse, und
`nat::pruefe()` weist sonst beim Start ab. Alles lief, nur niemand kam
an.

**Neu in der Laufzeit:** `NodeCommand::Listen` (Relais-Reservierung im
Betrieb, denn erst AutoNAT sagt, ob eine gebraucht wird) und
`NodeCommand::ExterneAdresse`.

**Nicht geprüft, ausdrücklich:** das Lochstanzen selbst. Es braucht zwei
echte NATs; auf Loopback gibt es nichts zu durchstoßen. Erste Messung
des Mehrmaschinenlaufs, getrennt nach TCP und QUIC.

**Gemessen:** 94 Tests grün (68 Unit, 14 adversarial, 5 Eclipse/Sybil,
5 NAT, 2 Testnetz).

### v0.4.0 – 2026-08-24 (Punkt 4.3: Verbindungsgrenze und Peer-Diversität)

Schließt **Fund 53**. Neu: `src/limits.rs` und `src/scoring.rs`;
`node.rs` nimmt beide Behaviours **vor** Gossipsub und Kademlia auf,
damit eine abgelehnte Verbindung abgelehnt ist, bevor jemand Zustand für
sie anlegt.

**Der Mechanismus in einem Satz:** Eingehende und ausgehende
Verbindungen bekommen **getrennte Budgets** (48 und 16, Gesamtgrenze die
Summe). Weil eingehende eigenständig gedeckelt sind, kann eine Flut die
ausgehenden Plätze nicht aufzehren, und der Knoten kann jederzeit
Gegenstellen eigener Wahl anwählen.

**Was das nicht ist:** Der Angriff wird auf eine Bedingung reduziert,
nicht beseitigt. Die Zusage lautet „der Knoten darf wählen", nicht „er
wählt richtig". Kontrolliert ein Angreifer auch die Bootstrap-Liste,
nützt das freie Budget nichts. Steht so im Kopf von `limits.rs` und in
`tests/eclipse_sybil.rs`.

Dazu die Adressbereichsgrenze (IPv4 /24, IPv6 /64, vier eingehende je
Bereich): Das Füllen der 48 Plätze braucht damit 12 verschiedene
Bereiche statt 20 Prozesse auf einer Maschine. **Eine Kostenverschiebung,
keine Sperre**, und so ist sie dokumentiert.

⚑ **Fund 54: Eine strengere Schwelle war schlechter, nicht besser.** Der
erste Entwurf setzte die IP-Kolokationsschwelle des Peer-Scorings auf 4,
„gleichgezogen" mit der Adressbereichsgrenze. Der Integrationstest hat es
binnen einer Minute widerlegt: Elf Knoten auf `127.0.0.1` ergeben einen
Score von −245 bei einer Graylist-Schwelle von −80, **die Härtung hatte
den ehrlichen Knoten mit stummgeschaltet**. Beim Nachrechnen zeigte sich,
dass die Zahl zusätzlich wirkungslos war: Die Kolokation zählt
Identitäten je Einzeladresse, und dort deckelt die Adressbereichsgrenze
bereits schärfer. Übernommen wurde die Vorgabe der Bibliothek (10) plus
eine Ausnahme für Loopback. Rechnung und Tabelle im Kopf von
`src/scoring.rs`.

⚑ **Fund 55: Der dokumentierte Weg für die Nutzlastprüfung war nicht
erreichbar.** `validation::report_with()` nimmt einen `PayloadValidator`
entgegen, und drei Stellen der Doku sagten seit dem 2026-08-18, die
Node-Verdrahtung reiche ihn herein. **`run_node` hatte dafür keinen
Parameter** und rief die Fassung mit `AcceptAllValidator`. Aufgefallen
beim Schreiben der Knoten-Verdrahtung, nicht im Betrieb: `myl-net` hatte
bis dahin keinen einzigen Abnehmer im Repositorium, und eine Naht, die
niemand belastet, hält alles aus. Behoben mit `run_node_mit()`.

Ebenfalls neu: `NodeCommand::Dial`. Ein freies ausgehendes Budget nützt
nur, wenn jemand es benutzen kann; ohne Dial-Kommando konnte ein
laufender Knoten nach dem Start keine Verbindung mehr aufbauen.

**Gemessen:** 79 Tests grün (58 Unit, 14 adversarial, 5 eclipse/sybil,
2 testnet). Die 20-Knoten-Voll-Konnektivität bleibt bei 3,97 s, das
Peer-Scoring kostet sie nichts.

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
