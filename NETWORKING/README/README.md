# networking (`myl-net`, `myl-siegel`)

> **Version:** 0.21.0
> **Datum:** 2026-10-23
> **Status:** **Phase 1 und 2 abgeschlossen** (1.1–1.6, 2.1–2.3),
> **Phase 3 umgesetzt** (3.1–3.4, Abschluss unter Reviewvorbehalt), dazu Punkt 4.2 (Fuzzing der Wire-Protocol-Parser), Punkt 4.3
> (Verbindungsgrenzen und Peer-Diversität) und seit dem 26. August
> Punkt 4.1 (Chaos-Tests; IP-Paketverlust seit dem 22. Oktober im Lauf mit
> getrennten Prozessen unter `tc netem`, TESTCLIENT).
>
> Phase 1: 20-Knoten-Vollkonnektivität unter 5 s, ungültige Nachrichten
> werden nicht weiterverbreitet, sechs Gossip-Topics, Anfragekanal.
> Phase 2: Paarlatenzmessung mit EMA-Glättung, Latenz-Atteste,
> Geo- und AS-Diversität. Phase 3: verschlüsselte
> Sitzungen zwischen Shard-Minern und über Gateways hinweg, mit
> Schlüsselrotation je Epoche. **163 Tests grün.**
>
> **Offen:** das **unabhängige kryptographische Review** des
> Sitzungsschemas, Punkt 4.4 (Lasttest bei Zielnetzgröße) und der
> IP-Paketverlust aus 4.1.
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
├── README/                   diese Kurzübersicht
└── myl-net/                  die L0-Netzwerk-Crate (Bibliothek)
    └── src/
        ├── lib.rs             Crate-Wurzel: #![deny(unsafe_code)], Design-Doku
        ├── config.rs          entschiedene Parameter (Ping 15 s, EMA α=1/4
        │                      als Festkomma, Attest 5 min, Größenlimits)
        ├── identity.rs        Node-Identität: Ed25519-Keypair, PeerId,
        │                      Datei-Persistenz (load_or_create)
        ├── node.rs            Swarm-Aufbau: Sperrliste, Verbindungsgrenzen,
        │                      Adressvielfalt, Gossipsub mit Peer-Scoring,
        │                      Kademlia, Identify, Ping, Relais und AutoNAT
        │                      über TCP und QUIC
        ├── limits.rs          Verbindungsgrenzen (Fund 53): getrennte
        │                      Budgets ein-/ausgehend, je Peer, je
        │                      Adressbereich (IPv4 /24, IPv6 /64)
        ├── scoring.rs         Gossipsub-Peer-Scoring: IP-Kolokation,
        │                      Verhaltensstrafe, Graylist-Schwellen
        ├── sitzung.rs         Ende-zu-Ende-verschlüsselte Sitzungen
        │                      (Punkte 3.1–3.3): X25519 je Epoche, HKDF mit
        │                      Epoche und Pod im Salz, ChaCha20-Poly1305,
        │                      ein Schlüssel je Richtung, Zähler als Nonce,
        │                      Rotation vernichtet Schlüssel und Kanäle;
        │                      Epochenpunkte mit dem Konsensschlüssel beglaubigt
        ├── anfrage.rs         Punkt-zu-Punkt-Anfragen (/myelith/anfrage/1):
        │                      längenpräfixierter Byte-Codec, undurchsichtige
        │                      Nutzlast, 4-MiB-Grenze für beide Richtungen
        ├── nat.rs             NAT-Überwindung: Relais-Horchadressen,
        │                      Konfigurationsprüfung, Erkennung
        │                      vermittelter und QUIC-Adressen
        ├── discovery.rs       Peer-Discovery: Bootstrap-Peers parsen und
        │                      anwählen, Kademlia-Bootstrap (/myelith/kad/1)
        ├── latency.rs         Paarlatenzen: EMA in Festkomma, Atteste
        │                      (Messung des Netzes, nicht der Pod-Bildung)
        ├── gossip.rs          die sechs Gossip-Topics (Blöcke,
        │                      Transaktionen, PoI-Bündel, Challenges,
        │                      Latenz-Atteste, Konsensnachrichten),
        │                      Subscribe und Publish mit Borsh-Nutzlast
        ├── validation.rs      Nachrichtenvalidierung vor Weiterverbreitung:
        │                      Größenlimits je Topic, Borsh-Strukturprüfung,
        │                      Accept/Reject an Gossipsub
        └── runtime.rs         Ereignisschleife des Knotens: Kommandos
                               (Publish, PeerCount, Zustand, Dial, Listen,
                               Anfrage, Sperren), Ereignisse (Horchadresse,
                               validierte Nachrichten, Latenz,
                               Erreichbarkeit); `run_node_mit` reicht den
                               PayloadValidator herein
    └── tests/
        ├── testnet.rs         Akzeptanztests: 20 Knoten voll verbunden
        │                      unter 5 s, ungültige Nachrichten werden
        │                      nicht weiterverbreitet (2 Tests)
        ├── adversarial.rs     Fuzzing der Gossip-Parser (14 Tests)
        ├── eclipse_sybil.rs   Verbindungsgrenzen gegen Flut, freies
        │                      ausgehendes Budget (5 Tests)
        ├── nat.rs             Relais-Pfad: ein Knoten ohne wählbare
        │                      Adresse wird über das Relais erreicht
        │                      (5 Tests)
        ├── chaos.rs           Partition und Heilung, Sperre gegen neuen
        │                      Wählversuch, Wiedereinstieg, hängender
        │                      Knoten, je mit Kontrolllauf (6 Tests)
        └── sitzung.rs         Sitzungen über echte Verbindungen: Shard zu
                               Shard, weiterleitendes Gateway ohne
                               Lesezugriff, Rotation, Wiedereinspielung,
                               untergeschobener Epochenpunkt (5 Tests)
```

## Changelog

### v0.21.0 (2026-10-23): Sicherheitsaudit: der Anfragekanal, die Identitätsdatei und die Kapsel (Funde 599, 605 bis 608; `myl-net` 0.21.0, `myl-siegel` 0.3.0)

Aus dem Sicherheitsaudit vom 2026-10-23.

- **Fund 599:** RUSTSEC-2026-0385 in `ctutils` 0.4.2 (Maskenbildung nicht in
  konstanter Zeit, über `digest` 0.11 unter `libp2p-identity`); auf 0.4.5.
  Dazu `chacha20` 0.10.1 (zurückgezogen) auf 0.10.2, womit die letzte
  Ausnahme in `deny.toml` fällt.
- **Fund 605, `myl-net`:** Der Anfragekanal lief mit der Vorgabe von
  libp2p, 100 Ströme je Verbindung zu je bis zu 4 MiB; eine fremde
  Gegenstelle mit ihren zwei Verbindungen konnte 800 MiB in Arbeit halten.
  Jetzt `MAX_ANFRAGEN_ZUGLEICH` = 16 (legitim höchstens sieben: vier
  Nachforderungen, zwei der Hintergrundprüfung, ein Auftrag), 64 MiB je
  Verbindung, als Konstantenzusicherung.
- **Fund 606, `myl-net`:** Ein Antwortkanal verliess `offene_anfragen` nur,
  wenn der Knoten antwortete; abgelaufene und abgebrochene Anfragen blieben
  für immer liegen, und die Tabelle wuchs ohne Ende. Jetzt wird an
  `ResponseSent` und `InboundFailure` aufgeräumt, die libp2p für **jede**
  eingegangene Anfrage meldet. `Netzzustand::offene_anfragen` zeigt die
  Zahl.
- **Gegenprobe beider** (`tests/anfragegrenzen.rs`, neu): Ein nackter Swarm
  mit 100 Strömen schickt 40 Anfragen, der Knoten antwortet nicht.
  Höchstens 16 kommen an, nach dem Ablauf ist keine mehr offen. Ohne 605:
  „40 Anfragen zugleich angekommen, erlaubt 16“; ohne 606: „16
  Antwortkanaele nach dem Ablauf noch gehalten“.
- **Fund 607, `myl-net`:** Die Knotenidentität wurde mit `fs::write`
  angelegt und danach auf `0600` gesetzt; jetzt über
  `myl_types::geheimdatei` mit den Rechten ab dem Anlegen.
- **Fund 608, `myl-siegel`:** Die **erste** Kapsel setzte den
  Empfangsschlüssel, jede spätere wurde mit `Ok` übergangen (die Doku sagte
  „abgelehnt“). Eine Kapsel ist nicht signiert, und eine verfälschte fällt
  erst am Tag auf: Wer einen Umschlag mit gefälschter Kapsel vor dem echten
  zustellte (ein Inferenzauftrag genügt, und den darf jede Gegenstelle
  schicken), sperrte die Richtung für die ganze Epoche; ein echter Neustart
  der Gegenstelle sperrte sie genauso. Jetzt ist eine neue Kapsel ein
  **Angebot** (höchstens vier); gültig wird sie mit der ersten Nachricht,
  die unter ihr den Tag besteht, und das kann nur, wer das
  Diffie-Hellman-Geheimnis der Gegenstelle hat. Die vorige ist dann
  abgelöst und kommt nie wieder (`KapselAbgeloest`), sonst spielte eine alte
  Nachricht mit ihrer Kapsel den Zählerstand zurück. **Gegenproben:** Mit
  „die erste gewinnt“ fallen alle drei neuen Tests (Fälschung vorweg, über
  den Umschlag, Neustart); ohne die Sperre der abgelösten Kapsel fällt der
  dritte.
- Geprüft und in Ordnung: Längenpräfix vor dem Anlegen, Gossip streng mit
  Obergrenze, Verbindungsgrenzen; in `myl-siegel` je Richtung ein eigener
  Schlüssel, frische Kapsel je Sitzung (keine Nonce-Wiederholung nach einem
  Neustart), Beitragsprüfung, Kopf als AAD, Schutz gegen Wiedereinspielung,
  Schlüssel in `Zeroizing`.
- Belege: Clippy und alle Tests beider Kisten grün; die Sitzungstests von
  `myl-net` und `myl-pod` grün.

### v0.20.0 (2026-10-23): libp2p 0.57; zwei Sicherheitsmeldungen in `hickory-proto` behoben (`myl-net` 0.20.0)

- ⚑ **libp2p 0.56 auf 0.57.1.** Anlass: RUSTSEC-2026-0118 (NSEC3-Prüfung
  ohne Ende) und RUSTSEC-2026-0119 (quadratische Namenskompression beim
  Kodieren) in `hickory-proto` 0.25.2, das nur `libp2p-mdns` 0.48 hereinzog.
  Erreichbar war keine von beiden: übersetzt ohne `dnssec`, und mDNS kodiert
  seine Pakete nicht über hickory. Behoben sind sie erst mit `libp2p-mdns`
  0.49 (`hickory-proto` 0.26.3), und das gibt es nur mit libp2p 0.57. Keine
  Ausnahme in `deny.toml`, mDNS bleibt.
- **Eine Codeänderung:** `request_response::Codec` braucht kein
  `#[async_trait]` mehr; `ByteCodec` implementiert die Methoden jetzt als
  gewöhnliche `async fn`, und `async-trait` fällt aus den Abhängigkeiten.
- **Was sich mit 0.57 im Verhalten ändert** (aus den Changelogs der
  libp2p-Kisten, nichts davon verlangte Code):
  - gossipsub 0.50.1 behebt zwei weitere Sicherheitsmeldungen (unbegrenztes
    Wachstum der Topics je Gegenstelle durch GRAFT; PRUNE für nicht
    abonnierte Topics). Neu ist ein Deckel von 100 abonnierten Topics je
    Gegenstelle; wir haben sechs.
  - mDNS 0.49 nennt nur noch Adressen, die zur Schnittstelle passen, also
    keine Rückschleife mehr. Das ist dieselbe Richtung wie Fund 594;
    `fuer_das_adressbuch` bleibt, denn `identify` nennt sie weiter.
  - QUIC 0.14 bevorzugt den Schlüsseltausch X25519MLKEM768
    (post-quantensicher). Gegenstellen mit 0.56 sollten nach TLS 1.3 auf
    X25519 zurückfallen; ein gemischtes Netz ist nicht gemessen.
  - yamux 0.14, prost statt quick-protobuf im Drahtformat der Protokolle
    (die Bytes sind dieselben).
- ⚠️ **QUIC rechnet jetzt mit `aws-lc-rs` statt `ring`.** `libp2p-quic` 0.14
  und `libp2p-tls` 0.7 schreiben es fest vor, ohne Schalter. `aws-lc-sys`
  ist C und wird beim Bau übersetzt; auf macOS und Linux ohne cmake und
  nasm (gemessen), auf Windows mit den vorgefertigten Objekten der Kiste.
  Das Archiv hat 9,7 MB; der Vorrat ist trotzdem kleiner als vorher
  (125 MB, 51 alte Archive entfernt).
- Die neuen Hauptfassungen von sha2 (0.11), curve25519-dalek (5) und
  ed25519-dalek (3) hängen nur unter `libp2p-identity`, nie unter eigenem
  Code. Der Konsens signiert mit BLS und hasht mit dem eigenen sha2 0.10.
- **Belege:** Clippy (`-D warnings`) und `cargo test --locked --offline`
  gegen den Vorrat: `myl-net` 119, `myl-node` 327, `myl-testclient` 278
  grün, darunter der Netzlauf mit fünfzehn Knoten (41 s). cargo-deny für
  jede Kiste grün. Lauf mit getrennten Prozessen unter `tc netem` (`--kurz`,
  sechs Knoten, 50 bis 200 ms, 1 bis 5 % Verlust): jeder kennt jeden nach
  5,8 s, Neuling 2 478 Blöcke in 18,1 s, Neuling aus dem Abzug bestätigt in
  20,4 s, am Ende eine Wurzel bei Höhe 3 484, bestanden.

### v0.19.0 (2026-10-22): Keine Rückschleife eines anderen im Adressbuch; Funde 594 und 596 (`myl-net` 0.19.0, `myl-siegel` 0.2.1)

- 📌 **Fund 594: Knoten wählten die Rückschleife ihrer Gegenstellen.** Wer
  auf allen Schnittstellen horcht, nennt über `identify` und mDNS jede
  davon, `127.0.0.1` eingeschlossen; beides kam ungefiltert in die
  Kademlia-Tabelle, und `identify` hielt die Adressen zusätzlich in einem
  eigenen Speicher, den libp2p beim Wählen befragt. Wer die Rückschleife
  eines anderen wählt, erreicht sich selbst („Unexpected peer ID“). Im
  ersten Lauf mit getrennten Prozessen stand nach einer Trennung nur noch
  diese Adresse da, und zwei Knoten fanden nie wieder zueinander. Im Lauf
  im selben Prozess fiel es nie auf, denn dort sitzen alle wirklich auf der
  Rückschleife.
- `discovery::fuer_das_adressbuch`: die Rückschleife nur, wenn eine
  Gegenstelle keine andere Adresse nennt (dann sitzt sie auf derselben
  Maschine, wie in jeder Probe), eine unspezifische Adresse (`0.0.0.0`,
  `::`) nie. Gerufen für `identify` und für mDNS (dort je Gegenstelle
  gesammelt).
- `identify` ohne eigenen Adressspeicher (`with_cache_size(0)`): Adressen
  kommen nur noch aus Kademlia, also gefiltert; eine Quelle statt zwei.
- **Belege:** `die_rueckschleife_nur_ohne_andere_adresse`. Im Lauf mit
  getrennten Prozessen vorher 24 Wahlen über die Rückschleife nach dem
  Filter allein (der Speicher von `identify` lieferte sie weiter), danach
  null; der Lauf prüft das seither selbst.
- 📌 **Fund 596, eine Folge von 594: Verbindungen vom eigenen Rechner.**
  Ohne die Rückschleife wählen Knoten auf einem Rechner einander über dessen
  LAN-Adresse, und die zählte in `Adressvielfalt` wie jede andere aus diesem
  /24. Im Netzlauf mit fünfzehn Knoten in einem Prozess wurden 141
  Verbindungen abgewiesen, und zwei Knoten fanden einander nie. Jetzt ist
  eine eingehende Verbindung ausgenommen, wenn die Gegenstelle dieselbe IP
  hat wie die Adresse, auf der sie ankam: Der Wählende steht auf dieser
  Maschine, und es gilt die Begründung der Rückschleife. Andere Rechner im
  selben /24 bleiben gezählt (die offene Frage unten berührt das nicht).
  Probe `verbindungen_vom_eigenen_rechner_belegen_keinen_platz`, Gegenprobe
  ohne die Ausnahme rot; Netzlauf danach grün (jeder kennt alle 14).
- ⚠️ **Gesehen, nicht geändert:** Aus einem /24 nimmt ein Knoten höchstens
  vier eingehende Verbindungen an (`MAX_JE_ADRESSBEREICH`). Mehr als fünf
  Knoten im selben lokalen Netz vernetzen sich deshalb nicht voll. Ob
  private Bereiche wie die Rückschleife ausgenommen werden, ist eine
  Abwägung gegen Sybil-Fluten innerhalb eines Rechenzentrums und bleibt
  offen.
- `rust-version` auf 1.99 in `myl-net` und `myl-siegel` (0.2.1); die Werkzeugkette steht einmal, in
  `rust-toolchain.toml` (1.99.0), mit der auch die CI baut. Nur das Manifest.

### v0.18.0 (2026-10-22): Knoten finden einander im lokalen Netz ohne Angabe

- **mDNS** (`lokale_suche` in `NetConfig`, Vorgabe aus): `MylBehaviour::mdns`
  als `Toggle`; was es findet, kommt in die Kademlia-Tabelle und wird
  angewählt, danach geht es wie bei jeder Gegenstelle weiter (`identify`,
  Suche). Die Vorgabe ist aus, weil Proben auf derselben Maschine einander
  sonst fänden; ein echtes Netz schaltet sie ein (NODE `Netz::lokale_suche`).
- ⚑ **Wer kein Myelith-Kademlia anbietet, wird nach `identify` getrennt.**
  mDNS findet jedes libp2p-Programm im lokalen Netz (derselbe Dienstname),
  und ein fremdes belegte sonst einen Verbindungsplatz.
- 📌 mDNS sucht über die Schnittstellen des Rechners. Ein Knoten, der nur auf
  `127.0.0.1` horcht, ist im lokalen Netz nicht zu finden.
- Sperrdatei unverändert: `libp2p-mdns` 0.48.0 war schon aufgelöst und liegt
  im Vorrat.
- **Belege:** NODE `tests/entdeckung.rs` (zwei Knoten ohne jede Angabe finden
  einander in 0,12 s); TESTCLIENT Netzlauf mit drei Knoten ohne Angabe, jeder
  kennt alle 14 anderen, zweimal grün. `cargo test` rc=0, Clippy still.
  ⚠️ Lokal nicht geprüft: mDNS in den CI-Läufern.

### v0.17.1 (2026-10-22): Sicherheitsmeldung zu `rustls-webpki`; nur die Sperrdatei

- `rustls-webpki` 0.103.14 auf 0.103.16, über `libp2p-quic` und `rustls`
  eingezogen. Anlass: Die Abhängigkeitsprüfung war nach dem Push von
  v0.17.0 rot wegen einer neuen Meldung (RUSTSEC-2026-0382: Zertifikatspfade
  mit nicht mehr vertrauenswürdigen Zwischenzertifikaten von emSign). Kein
  Code geändert.
- **Belege:** cargo-deny grün; Clippy mit `-D warnings` und alle Tests
  offline gegen den Vorrat (`--locked --offline`) mit Rückgabe 0.

### v0.17.0 (2026-10-22): Knoten finden einander über Kademlia; Funde 587 und 591

- 📌 **Fund 587, gefunden im Netzlauf:** Kademlia wurde nie angestoßen.
  `start_bootstrap` hatte keinen Aufrufer, die Horchadressen aus `identify`
  kamen nicht in die Routing-Tabelle, und ohne bestätigte äußere Adresse
  blieb Kademlia im Klientenmodus und beantwortete keine Suche. Jeder
  Knoten kannte nur seine Bootstrap-Gegenstelle; Gossip lief über den
  Stern, aber eine direkte Anfrage an ein anderes Pod-Mitglied fand keinen
  Weg.
- `run_node_mit`: jede gemeldete Horchadresse eines Kademlia-Knotens in die
  Tabelle, eine Suche nach der ersten Meldung, danach in Abständen ab
  `KAD_ERSTER_ABSTAND` (1 s), verdoppelt bis `KAD_TAKT` (30 s).
  `build_swarm`: Kademlia fest im Servermodus.
  📌 Zuerst lief die Suche gleich im Takt von 30 s. Die erste Suche kam aber,
  wenn die Bootstrap-Gegenstelle oft nur sich selbst kannte, und wer nach
  ihr dazukam, war erst eine halbe Minute später zu finden.
- 📌 **Fund 591, gefunden in derselben Probe unter Last:** Jede Wahl bindet
  ihren TCP-Anschluss an den eigenen Horchport (`PortUse::Reuse`, die
  Vorgabe des Swarms, gebraucht vom Lochstanzen). Finden sich zwei Knoten
  zugleich, wählen sie einander zugleich an, vom Horchport zum Horchport.
  TCP macht daraus eine einzige Verbindung, der Handshake scheitert, und
  danach liegt das Adressenpaar in TIME_WAIT. Der TCP-Transport von libp2p
  weicht dann nur bei `AddrNotAvailable` auf einen neuen Port aus (so meldet
  es Linux); macOS meldet `AddrInUse` (os error 48), und die beiden Knoten
  blieben einander unerreichbar. In der Probe mit drei Knoten scheiterten
  so unter Last 6 von 48 Läufen; im Netzlauf stand die Meldung bei mehreren
  Knoten.
- `portwahl.rs` (neu): `MitAusweichport`, eine Hülle um den TCP-Transport.
  Scheitert eine Wahl vom Horchport, weil das Paar belegt oder nicht
  verfügbar ist, wählt sie einmal von einem neuen Port; alles andere geht
  unverändert durch. `build_swarm` rüstet sie über `with_other_transport`
  auf wie bisher `with_tcp` (Noise, Yamux). `tokio` mit `net`.
- **Belege:** NODE `tests/entdeckung.rs` (A und C kennen nur B und finden
  einander): zehnmal hintereinander grün in je etwa einer Sekunde, unter
  Last (acht gleichzeitig) 96 von 96, vor Fund 591 42 von 48. Die neue
  Probe `portwahl::die_wahl_weicht_auf_einen_neuen_port_aus` besetzt das
  Paar selbst; ohne die Hülle scheitert sie mit genau `os error 48`. Im
  Netzlauf mit fünfzehn Knoten kennt jeder alle vierzehn anderen.
  `cargo test` rc=0, Clippy mit `-D warnings` still.

### v0.16.2 – 2026-10-21 (Kommentare: kein `LatencyGraph` mehr; `myl-types` 0.44.0)

`myl-types` hat `LatencyGraph` entfernt, denn die Pod-Bildung liest seit
der Entscheidung für Zonen keine gemessene Latenz. Hier nannten ihn noch
der Kopf von `lib.rs`, drei Kommentare in `latency.rs`, die Beschreibung
in `Cargo.toml` und diese README als Grundlage der Pod-Bildung. Sie sagen
jetzt, was gilt: Paarlatenzen und Atteste sind eine Messung des Netzes.
Kein Code geändert; Topic, Prüfung und Größengrenze der Atteste bleiben.

**Belege:** `cargo test` rc=0, Clippy mit `-D warnings` still.

### v0.16.1 – 2026-10-14 (Sicherheitsmeldungen zu `quinn-proto` und `rustls`; nur die Sperrdatei)

**Die Abhängigkeitsprüfung (cargo-deny) war rot**, ohne dass sich hier
Code geändert hatte: Zwischen zwei Läufen sind neue Meldungen erschienen.

| Paket | vorher | jetzt | Meldungen |
|---|---|---|---|
| `quinn-proto` (QUIC unter `libp2p-quic`) | 0.11.16 | **0.11.19** | RUSTSEC-2026-0338 bis 0343: entfernte Gegenstellen bringen den Empfänger mit kleinen Rahmen zum Absturz oder lassen Puffer unbegrenzt wachsen |
| `rustls` (TLS unter `libp2p-tls` und QUIC) | 0.23.45 | **0.23.46** | RUSTSEC-2026-0346 bis 0349: unter anderem ein nicht beglaubigtes ServerHello, das spätere Verbindungen auf klassischen Schlüsselaustausch herabstuft |

Angehoben mit `cargo update -p quinn-proto` und `-p rustls`; sonst bewegt
sich keine Fassung. Die beiden Archive liegen im Vorrat der Fremdquellen,
die drei abgelösten sind entfernt, sodass der Bau weiter ohne Netz geht.

**Belege:** `cargo deny check` grün für `myl-net`, `myl-node` und
`myl-testclient` (vorher je rot); dieselben drei Kisten offline gegen den
Vorrat (`--locked --offline`), Clippy mit `-D warnings` und alle Tests
mit Rückgabe 0.

### v0.16.0 – 2026-09-03 (⚑ Fund 159: der Umschlag passte nicht durch den Kanal)

**`MAX_KLARTEXT_BYTES` zog den Kapselvorspann nicht ab.** Die Herleitung
stammte aus der Zeit vor dem hybriden Austausch; seit dem KEM-Zweig geht
jede versiegelte Nachricht im Umschlag, und der ist 1 192 Byte länger.
Der grösste erlaubte Klartext ergab damit **4 195 496 Byte gegen eine
Grenze von 4 194 304**.

⚑ **Gefunden hat es kein Test, sondern eine Simulation**
(`umschlag_sim.py`). Jeder Test sah einen Umschlag, der aufgeht; keiner
sah den grössten. Berichtigt als Übersetzungszusicherung, mit einer
Gegenprobe darauf, dass ein Byte mehr nicht passt.

**Und die Zahl im Kommentar war falsch:** Der Vorspann ist 1 324 Byte
und nicht 1 088. Bei einer gewöhnlichen Chatanfrage von 300 Byte sind
das **81,5 Prozent Verpackung**.

### v0.15.0 – 2026-09-03 (der Sitzungskanal zieht aus, B6-4)

**`sitzung.rs` ist jetzt die eigene Kiste `myl-siegel`.** 2428 Zeilen
hybrider Schlüsselaustausch, X25519 und ML-KEM-768, ChaCha20-Poly1305.

⚑ **Der Anlass war Fund 155: Sie hatte keinen Aufrufer.** Alle 21
Fundstellen von `sitzung::` im Baum waren `myl_types::sitzung`, also der
Kontrakt; innerhalb dieser Kiste benutzte sie keine andere Datei, und
`lib.rs` reichte sie nur weiter. **Aufgefallen ist es, als GATEWAY
Stufe 4 sie brauchte:** Der Shard-Prozess muss entsiegeln, hängt aber
nicht am Netz.

**Gemessen statt geschätzt:** Über `myl-net` hätte `myl-pod` **181
zusätzliche Kisten** gebaut, über `myl-siegel` sind es **21**.

⚑ **Die Wiederausfuhr bleibt:** `myl_net::sitzung` und alle Namen aus
`lib.rs` tragen weiter. Ein Umzug soll keinen Aufrufer kosten.

**Der Integrationstest bleibt hier**, und das ist die Nahtregel: Er
fährt das Siegel **über das echte Netz**, und nur diese Kiste sieht
beide. Die Einheitstests des Kanals sind mitgegangen.

⚑ **Und eine Behauptung ist zu einer Prüfung geworden.**
`MAX_ANFRAGE_BYTES` wohnt jetzt in `myl-types`, weil drei Kisten Grenzen
daraus ableiten; die Gleichziehung mit `MAX_GOSSIP_MESSAGE_BYTES` stand
seit dem 2026-08-12 als Satz im Kommentar und ist seit heute eine
Übersetzungszusicherung. **Ein Satz, den kein Code liest, hält keine
zwei Zahlen zusammen.**

**Ältere Einträge** (v0.14.1 und davor, 27 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- NETWORKING/README/README.md`.
