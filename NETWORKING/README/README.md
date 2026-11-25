# NETWORKING: das Netz zwischen den Knoten (`myl-net`, `myl-siegel`)

> **Version:** 0.30.1
> **Datum:** 2026-11-25
> **Status:** In Gebrauch im Knoten: Gegenstellen finden (Einstiegsliste,
> Kademlia, mDNS, Adressen mit Namen), Gossip, das nur Geprüftes
> weitergibt, ein Anfragekanal, Latenzmessung mit Attesten,
> Verbindungsgrenzen und Vielfalt nach Netz und Ort, Abwehr gegen
Umzingelung (Fühler, ausgehend höchstens zwei je Netz oder AS, Anker über
Neustarts), NAT über AutoNAT,
> Relais und Lochstanzen, verschlüsselte Sitzungen zwischen Shard-Minern
> mit Schlüsselwechsel je Epoche. **Offen:** ein unabhängiges
> kryptographisches Review des Sitzungsschemas und ein Lasttest in der
> Zielgröße des Netzes.

Die unterste Schicht (L0). Sie bringt Blöcke, Transaktionen und Atteste
zu allen Knoten, misst laufend, wie weit die Knoten voneinander entfernt
sind (daraus entstehen die Pods), und verschlüsselt die Ströme zwischen
den Shard-Minern eines Pods. Gebaut auf libp2p mit TCP und QUIC.
Referenzimplementierung von Whitepaper Kap. 3.2, Grundlage der
Latenzmessung aus Kap. 4.1 und 4.3.

| Wer | Was hier wichtig ist |
|---|---|
| **Nutzer** | Nichts zu tun. |
| **Betreiber** | Ein Port (Vorgabe 4150) für TCP und für UDP. Ein Knoten hinter NAT ohne Weiterleitung erreicht andere, ist selbst aber nur über ein Relais erreichbar. Ein Einstieg mit Namen (`/dns4/<name>/tcp/<port>/p2p/<kennung>`) übersteht einen Umzug; ein gefälschter DNS-Eintrag schiebt keinen fremden Knoten unter, denn der Handshake prüft die Kennung. |
| **Entwicklung** | [Struktur](#struktur). Läufe mit getrennten Prozessen und Paketverlust unter `tc netem` fährt der Testclient. Kein Gleitkomma auch hier: Die Latenz-EMA rechnete bis Fund 44 in `f64`, heute in Festkomma. |

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
        │                      Budgets ein-/ausgehend, je Peer, eingehend je
        │                      Adressbereich (IPv4 /24, IPv6 /64), ausgehend
        │                      je Netzgruppe (A9)
        ├── netzgruppe.rs      Netzgruppen (AS aus der Zuordnung des
        │                      Betreibers, sonst IPv4 /16, IPv6 /32) und das
        │                      Lesen der AS-Zuordnung (A9)
        ├── fuehler.rs         Fühler: eingehende Gegenstellen erst nach
        │                      eigener Wahl in die Routingtabelle, aus einem
        │                      eigenen Swarm mit flüchtiger Identität (A9)
        ├── anker.rs           Anker über Neustarts: Auswahl und Dateiformat (A9)
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

### v0.30.1 (2026-11-25): `url` 2.5.9 wegen RUSTSEC-2026-0592 (`myl-net` 0.30.1)

- Die Sperrdatei hebt `url` von 2.5.8 auf 2.5.9 (mit `form_urlencoded` 1.2.3): `Url::origin()` rief sich bei verschachtelten `blob:`-Adressen ohne Grenze selbst auf und konnte den Stapel sprengen (RUSTSEC-2026-0592 vom 2026-11-20), und cargo-deny war damit rot. `url` kommt über `multiaddr` und `hickory-proto` herein; der eigene Code ruft `origin()` nicht auf. Geändert hat sich nur die Sperrdatei, der Vorrat trägt die neue Fassung.

### v0.30.0 (2026-11-24): Eine Vorrangspur mit Zweck, eine Schlange nach Bytes und die Frist einer Anfrage (`myl-net` 0.30.0)

- **Gefunden im Auftragslauf vom 24. November (Fund 720 im Knoten):** Am
  Wechsel schickt die Stichprobe eines Knotens jedem Mitglied eines Pods
  eine Spuranfrage je gezogenem Segment, bei 5 % von rund 12 500 Segmenten
  über 600 auf einmal. Die Schlange (16 unterwegs, höchstens 256 wartend)
  lief über; der Rest der Stichprobe scheiterte, und die Stapel des
  Koordinators im selben Knoten an dasselbe Mitglied scheiterten mit „die
  Warteschlange zu dieser Gegenstelle ist voll“. Die Runde ging verloren
  (sichtbar, wie seit Fund 719 gewollt) und wurde neu gerechnet.
- **Die Schlange ist nach Bytes begrenzt** (`WARTESCHLANGE_HOECHSTENS_BYTES`,
  16 MiB je Gegenstelle, statt 256 Anfragen): Eine Spuranfrage hat 45 Byte,
  die Stichprobe am Deckel (5 000 je Pod) rund 225 KB.
- **Neu `NodeCommand::AnfrageVorrang`:** wartet in einer eigenen Spur
  (höchstens `VORRANG_HOECHSTENS`, 256) und geht vor jeder wartenden Anfrage
  ohne Vorrang hinaus, sobald ein Platz frei ist; das Bild des Staus zählt
  beide Spuren. Gedacht für das, was ein Knoten für seinen eigenen Pod
  weiterreicht. Die Zeitprobe (`AnfrageMitZweck`) wartet weiter nie.
- **Der Vorrang trägt einen Zweck** (Fund 721 im Knoten): Antwort und
  Fehlschlag melden ihn zurück, auch wenn die Anfrage in der Schlange
  scheitert (voll, oder die Verbindung geht zu, bevor sie hinausgeht);
  `Unterwegs::vergessen` gibt dafür die Zwecke der Wartenden zurück.
- **Fund 722: die Frist einer Anfrage ausdrücklich, 30 s statt der Vorgabe
  von libp2p (10 s)** (`ANFRAGEFRIST`, in beide Richtungen). Im dritten
  Kurzlauf vom 24. brauchten Stapel des Prefills an kalten Shards am Anfang
  der Epoche bis 9,7 s; die nächsten liefen in die 10 s („Timeout while
  waiting for a response“, beim Mitglied danach „unexpected end of file“),
  obwohl der Koordinator einem Shard 30 s lässt. Der Preis: Eine schweigende
  Gegenstelle hält die Plätze zu ihr 30 s; das trifft nur Anfragen an sie.
- Proben: `stau` (7: Grenze nach Bytes, die Stichprobe am Deckel passt ganz
  hinein, Vorrang überholt die Wartenden); `anfragegrenzen` im echten Swarm
  mit 600 Spuranfragen und zwei mit Vorrang hinter belegten Plätzen, keine
  scheitert.

### v0.29.0 (2026-11-23): Über der Grenze warten statt scheitern, und ein Stau sagt, wer ihn macht (`myl-net` 0.29.0)

- **Gefunden in der Simulation vom 20. und den Kurzläufen vom 23. November:**
  Je Verbindung laufen höchstens `MAX_ANFRAGEN_ZUGLEICH` (16) eigene
  Anfragen; jede weitere scheiterte sofort mit „max sub-streams reached“,
  tausendfach je Lauf (Fund 715 im Knoten). Was die Plätze hielt, stand
  nirgends.
- **Neu `stau.rs`:** je eigener Anfrage Gegenstelle, Art (erstes Byte der
  Nachricht) und Beginn. Scheitert eine am Stau, meldet `NodeEvent::Stau`
  je Art die Zahl unterwegs, das Alter der ältesten und die Wartenden, je
  Gegenstelle höchstens einmal in 10 s. Das erste Bild nannte die Ursache:
  Salven von Spuranfragen der Stichprobe, 27 bis 35 auf einmal, die
  älteste 1 bis 3 ms alt.
- **Warten statt scheitern:** Eine Anfrage ohne Zweck wartet, wenn zur
  Gegenstelle schon 16 unterwegs sind, in einer Schlange (höchstens 256) und
  geht hinaus, sobald eine Antwort oder ein Fehlschlag einen Platz frei
  macht; geht die letzte Verbindung zu, scheitern die wartenden. ⛔️ Eine
  Anfrage mit Zweck (die Zeitprobe) wartet nie, denn die Wartezeit zählte
  sonst als Rechenzeit des Gemessenen. Eine volle Schlange scheitert mit
  `stau::WARTESCHLANGE_VOLL`, und `stau::ist_stau` erkennt beide Texte.
- Proben: `stau` (5), `tests/anfragegrenzen.rs`
  `ein_stau_nennt_was_die_verbindung_belegt` (eine schweigende Gegenstelle:
  vier ohne Zweck warten, eine mit Zweck scheitert, ein Bild mit 16 der
  Art 9). Im Kurzlauf mit Lügner und Störer gefahren: ohne Befund.

### v0.28.0 (2026-11-19): Abwehr gegen Umzingelung (A9), Verfahrenskennzeichen der Sitzungen (`myl-net` 0.28.0, `myl-siegel` 0.4.0)

Audit A9, nach Heilman et al. 2015 (Bitcoin), Marcus et al. 2018 (Ethereum)
und Tran et al. 2020 (Angriffe über wenige AS); Vorlage für Fühler, Anker und
Gruppen sind die Gegenmaßnahmen von Bitcoin Core (feeler, `anchors.dat`,
asmap), nachgebaut, nicht übernommen.

- ⛔️ **Die Lücke:** libp2p-kad nimmt eine Gegenstelle nur über eine selbst
  gewählte Verbindung in die Tabelle (bei einer eingehenden ist die Adresse
  unbekannt). Seit Fund 587 trug die Laufzeit aber jede Horchadresse, die
  eine Gegenstelle über `identify` nannte, selbst ein, auch bei eingehenden
  Verbindungen. Ein Angreifer füllte so die Tabelle mit vielen Kennungen und
  beliebigen Adressen, ohne je erreichbar zu sein.
- ⚑ **Fühler** (`fuehler.rs`): Meldet sich eine Gegenstelle eingehend, wählt
  der Knoten sie selbst unter einer ihrer genannten Adressen an, mit der
  erwarteten Kennung. Erst wenn der Noise-Handshake dort genau diese
  Gegenstelle zeigt, geht die Adresse in die Tabelle; die Verbindung schließt
  sofort. Höchstens vier zugleich, 64 wartend, je Gegenstelle einmal je
  Stunde, Frist 20 s. Ereignis `NodeEvent::Fuehler`.
- 📌 **Der Fühler wählt aus einem eigenen, kleinen Swarm mit flüchtiger
  Identität.** Zuerst wählte er aus dem Swarm des Knotens; dann verteilte die
  Gegenseite ihre Anfragen reihum auch auf die Fühlerverbindung, und die
  rissen beim Schließen ab (`tests/sitzung.rs`, zwei von fünf Läufen rot).
  Mit fremder Kennung landet dort nichts. Zweiter Fund beim Bau: Der Fühler
  der Gegenseite kam eingehend an, und der Knoten fühlte zurück; das
  unterbleibt jetzt, wenn schon eine eigene ausgehende Verbindung besteht.
- ⚑ **Ausgehend höchstens zwei je Netzgruppe** (`netzgruppe.rs`,
  `limits::MAX_AUSGEHEND_JE_GRUPPE`): Gruppe ist das AS aus der Zuordnung des
  Betreibers (`NetConfig::as_zuordnung`, Format wie `table.txt` von bgp.tools,
  längstes Präfix gilt), sonst IPv4 /16 und IPv6 /32. Private Netze,
  Rückschleife, link-lokale Adressen und Relaiswege zählen nicht; eine
  Zuordnung geht vor diese Ausnahme. Pod-Mitglieder mit Vorrang sind
  ausgenommen. Wer alle 16 ausgehenden Plätze stellen will, braucht damit
  Adressen in acht Netzen. Abgewiesene Wahl löscht in Kademlia keine Adresse
  (geprüft im Quelltext von libp2p-kad 0.49).
- ⚑ **Anker** (`anker.rs`, `NodeCommand::Anker`): die zwei ältesten
  ausgehenden Verbindungen ab fünf Minuten, aus verschiedenen Gruppen, als
  Adresse mit `/p2p/`; der Knoten legt sie ab und wählt sie beim Start zuerst.
  Eine veränderte Datei bringt höchstens zwei gültige Einträge.
- 📌 **Vor dem Wählen abweisen, solange die Gruppe voll ist:** Im ersten Lauf
  mit allen Behältern in einem AS baute Kademlia alle paar Sekunden eine
  Verbindung ganz auf (TCP, Noise, yamux), nur damit sie an der Grenze fiel
  (772 Wahlen über alle Knoten in 12 Minuten, dazu 914 abgebrochene
  eingehende Verbindungen bei den Gegenstellen). Jetzt merkt sich
  `Adressvielfalt` die Gruppe einer abgewiesenen Gegenstelle und weist die
  nächste Wahl schon in `handle_pending_outbound_connection` ab, bis dort ein
  Platz frei ist. Im zweiten Lauf: 34 Abweisungen, 108 abgebrochene
  eingehende, weniger als im Lauf ohne A9 vom Vortag.
- 📌 **Der Grund einer abgewiesenen Verbindung steht im Protokoll:** libp2p
  nennt sie nur „Dial error“; `mit_ursachen` hängt die Kette der Ursachen an
  („… Netzgruppe AS65000 hat bereits 2 ausgehende Verbindungen“).
- `NodeCommand::Routingtabelle` für Diagnose und Proben.
- **Proben:** Bibliothek 107 (Netzgruppen 7, Fühler 5, Anker 5, Grenze je
  Gruppe 4 neu, eine angepasst: ausgehende zählen weiter nicht gegen die
  eingehende Grenze je /24), `tests/fuehler.rs` neu: Eine ehrliche eingehende
  Gegenstelle steht nach dem Fühler in der Tabelle, eine mit toter Adresse
  nicht, und der Fühler belegt keine Verbindung; eine selbst gewählte braucht
  keinen. Gegenprobe mit der alten Regel (jede genannte Adresse eintragen):
  rot. Volle Sammlung grün, `sitzung`, `testnet`, `fuehler`, `chaos` sechsmal
  hintereinander grün, Clippy grün.
- **Im Behälter, der schärfste Fall** (`testnetzlauf.py --kurz --ein-as`,
  sieben Knoten, alle in einem AS, also höchstens zwei ausgehende je Knoten,
  mit Neustart, Absturz, Abschneiden, Spaltung, Einsteigern, Ausfall des
  Einstiegs, schlechtem Netz): eine Endhöhe (332), ein Block, eine Wurzel;
  aufgeholt in 2 bis 4 s wie ohne A9; Abstimmung unverändert (Wechsel je
  Commit gleich dem Lauf vom Vortag); Gegenstellen im Median 4 statt 5, Mesh
  im Median 3 bis 4, höchstens 9 % der Aufnahmen ohne Mesh für Blöcke (Vortag
  8 %); alle Fühler gelungen; Anker beim Neustart gewählt.
- ⚠️ **Was erst mehrere Maschinen zeigen:** echte Gruppen in echten AS, und
  ob zwei je Gruppe in einem kleinen Netz mit wenigen Gruppen trägt.
- ⚑ **Verfahrenskennzeichen in den Botschaften der Sitzungen** (3b.4,
  `myl-siegel` 0.4.0): ein Byte `VERFAHREN` (1 = X25519 und ML-KEM-768
  hybrid, HKDF-SHA256, ChaCha20-Poly1305) vorn in `Kopf` (113 statt 112 Byte,
  in den authentisierten Daten), `Kapsel` (Rahmen 1 185 statt 1 184 Byte vor
  dem Chiffrat) und `Epochenankuendigung` (in den unterschriebenen Bytes
  direkt hinter der Kennung). Ein anderes Kennzeichen heißt
  `SitzungsFehler::UnbekanntesVerfahren` und wird vor allem anderen geprüft.
  Damit ist ein späterer Wechsel des Verfahrens kein harter Bruch mehr, und
  niemand kann eine Botschaft auf ein anderes Verfahren umschreiben, ohne Tag
  oder Unterschrift zu brechen. ⚠️ Drahtformat geändert: Knoten und Shards
  dieses Stands sprechen nicht mit älteren (vor dem Start des Testnetzes ohne
  Folgen). Proben 62 (zwei neu, zwei erweitert: Kopf, Ankündigung samt
  Unterschrift, Kapsel, festgenagelter Aufbau), Gegenprobe ohne die Prüfung
  im Öffnen rot, Clippy grün. 📌 `tests/sitzung.rs` in `myl-net` rechnete
  die Rahmenlänge selbst nach und wurde rot (vier Proben „unleserlicher
  Rahmen“); jetzt nimmt sie `KAPSELRAHMEN_LEN`. Alle Kisten, die `myl-siegel`
  benutzen (Netz, Pod, Verifier, Simulation, Knoten, Testclient), mit Clippy
  und allen Proben grün.

### v0.27.0 (2026-11-19): Was nicht beurteilbar ist, wird ignoriert, nicht bestraft (`myl-net` 0.27.0)

- ⛔️ **Fund im nachgestellten Testnetz** (`TESTCLIENT/netzbedingungen/testnetzlauf.py`,
  fünf Validatoren in getrennten Behältern): Nach wenigen Minuten hatte kein
  Knoten mehr ein Mesh, in keinem Topic (99 bis 100 % der
  Zustandsaufnahmen). Blöcke kamen nur noch über das Fluten des Erzeugers
  an; ein Validator, der neu startete, bekam kein Mesh für den Konsens und
  beendete sich nach 60 s (dort behoben in NODE 0.135.0).
- **Ursache:** Jede Ablehnung der Nutzlastprüfung ging als `Reject` an
  Gossipsub, auch eine, bei der die Anwendung nur nicht urteilen konnte.
  Latenz-Atteste von einem Aussteller, den der Knoten nicht kennt, wurden so
  bestraft (−5 je Nachricht); im Lauf nannte kein Knoten `--teilnehmer`,
  also kannte niemand einen Aussteller, jede Bewertung fiel unter null, und
  Gossipsub nimmt dann kein GRAFT mehr an. Verschickt werden Atteste heute
  nur als Testverkehr (`--testverkehr`); `myl mining` tut das nicht, im
  Betrieb hätte dieser Weg also nicht ausgelöst. Die Semantik war trotzdem
  falsch: Jede Nachricht, die ein Knoten nur nicht beurteilen kann, hätte
  dieselbe Strafe gezogen. Auf der Rückschleife fiel es nie auf, weil dort
  keine Laufzeiten gemessen und also keine Atteste verschickt wurden.
- ⚑ **Dreistufig:** `PayloadValidator::nicht_beurteilbar` (Vorgabe: nie)
  und `Ablehnungsgrund::NichtBeurteilbar` („nicht-beurteilbar“);
  `validation::annahme` macht daraus `Ignore` (nicht weitergeben, ohne
  Strafe), nachweislich Falsches bleibt `Reject`. Das ist die Unterscheidung
  aus Gossipsub v1.1, die das Scoring vor Unkenntnis schützt.
- Probe `nicht_beurteilbares_wird_ignoriert_und_nicht_bestraft` samt
  Gegenprobe (falsch bleibt `Reject`).

### v0.26.1 (2026-11-18): Der Kopf für Nutzer, Betreiber und Entwicklung (`myl-net` 0.26.1)

- **Der Kopf ist für Nutzer, Betreiber und Entwicklung geschrieben:** ein Absatz in
  einfachen Worten, eine Tabelle, was wer hier findet, und ein Stand, der nennt,
  was heute in Gebrauch ist und was offen ist. Punktnummern des Plans und
  gezählte Tests fallen aus dem Kopf; sie alterten schneller als der Text.
- Der Betreiber erfährt im Kopf, was der Port braucht und warum ein Einstieg mit Namen sicher ist; die Latenz in Festkomma (Fund 44) steht dort statt im alten Stand.
- Am Code nichts geändert; die Kiste hebt ihre Fassung, weil Kopf und Manifest dieselbe Zahl tragen.

### v0.26.0 (2026-11-18): Adressen mit Namen werden gewählt (`myl-net` 0.26.0)

- ⛔️ **Fund:** `libp2p` war ohne das Merkmal `dns` eingebunden. Eine
  Adresse `/dns4/…` wurde gelesen (die Adressvielfalt kannte sie schon),
  aber nie gewählt. Die Einstiegsliste des Testnetzes soll Namen tragen,
  damit ein Umzug des Einstiegsknotens kein Release braucht (Entscheidung
  des Projektinhabers vom 18.).
- **Merkmal `dns` und `.with_dns()` im Aufbau des Transports**, nach TCP
  und QUIC: Namen löst der Resolver über die Einstellungen des Systems auf.
  Die Pakete (`libp2p-dns`, `hickory-resolver`, unter Windows `ipconfig`)
  lagen schon im Vorrat und in der Sperrdatei; der Bau mit `--locked`
  brauchte keine neue Auflösung.
- **Sicherheit:** Ein falscher DNS-Eintrag schiebt keinen fremden Knoten
  unter, denn die Adresse trägt die Peer-ID und der Noise-Handshake prüft
  sie; er kann höchstens die Verbindung verhindern (wie eine falsche
  Adresse).
- Probe `ein_einstieg_mit_namen_wird_gewaehlt` (`tests/testnet.rs`, unter
  Unix): Knoten B wählt `/dns4/localhost/tcp/<port>/p2p/<A>` und beide
  sehen einander. Gegenprobe von Hand: ohne `.with_dns()` bleibt sie nach
  10 s bei null Peers stehen.

### v0.25.4 (2026-11-16): Das Bündel endet mit zwei Listen (Fund 706, `myl-net` 0.25.4)

- Das PoI-Bündel trägt hinter den Vertretern die Liste seiner Sitzungen
  (SHARED_TYPES 0.72.0); seine Borsh-Form endet damit in zwei
  Längenfeldern statt in einem. Am Code der Leitung ändert sich nichts, nur
  die Bündel der Proben und die Worte in `validation.rs`.
- 📌 **Zwei Verstümmelungsproben wurden rot und waren im Recht:**
  `fuer_feste_typen_ist_die_borsh_pruefung_eine_laengenpruefung` nahm 0 von
  2 000 Zufallsfolgen an, `verstuemmelte_nachrichten_stuerzen_nicht_ab` 3 von
  20 000. Beide schonten die letzten 4 Bytes als das eine Längenfeld; jetzt
  die letzten 8. Die Aussage bleibt: Für diesen Typ prüft Borsh nur die
  Länge, über den Inhalt entscheidet der Validator der Kette.

### v0.25.3 (2026-11-15): Netz und Kette haben dieselbe Punktregel (Fund 701, `myl-net` 0.25.3)

- Probe `netz_und_kette_haben_dieselbe_punktregel`: Für alle 256 gleichen
  Folgen und 10 000 zufällige ergibt `peer_id_aus_bytes` genau dann eine
  `PeerId`, wenn `PeerIdBytes::ist_ed25519_punkt` (die Regel der Kette,
  CONSENSUS 0.88.0) ja sagt; sie wählt aus (über 1 000 je Seite).
  Gegenprobe: mit einer falschen Regel in SHARED_TYPES rot. Am Code nichts
  geändert.

### v0.25.2 (2026-11-11): Nicht jede Bytefolge ist eine Peer-ID (Fund 701, `myl-net` 0.25.2)

- 📌 **Fund 701:** Die Probe `jede_bytefolge_ergibt_eine_peer_id` und ihr
  Kommentar behaupteten, jede Folge von 32 Bytes ergebe eine `PeerId`, ein
  Ed25519-Punkt werde erst beim Rechnen geprüft. Geprüft war das an einer
  Folge (`[1; 32]`). Über einen Generator: Das Einlesen dekomprimiert den
  Punkt, und 5 006 von 10 003 Folgen werden abgewiesen. Die Probe heißt
  jetzt `nur_ein_gueltiger_punkt_ergibt_eine_peer_id`, verlangt rund die
  Hälfte Abweisungen und prüft die Gegenrichtung an 200 echten Schlüsseln.
- **Folge für den Knoten:** keine. Jeder Aufrufer außerhalb der Proben
  fängt den Fehler ab (`.ok()`, `let … else`). **Folge für die
  Registrierung:** Eine Netzadresse, die kein Punkt ist, wäre beim
  Eintragen erkennbar; heute nimmt die Kette sie an.
- `myl-net` 0.25.2.

### v0.25.1 (2026-11-09): Überholte Nachrichten gehen auf (`myl-net` 0.25.1, `myl-siegel` 0.3.2)

- 📌 **Fund 682:** Ein Kanal nahm nur streng steigende Zähler an. Die Tür
  versiegelt aber nacheinander und schickt jede Anfrage über eine eigene
  Verbindung; überholte eine die andere, scheiterte die frühere als
  Wiedereinspielung, und die Sitzung bekam keine Antwort. Jetzt gilt ein
  Gleitfenster von 64 Zählern (`EMPFANGSFENSTER`, nach RFC 4303): Jeder Zähler
  geht genau einmal auf, vermerkt wird erst nach dem Tag, hinter dem Fenster wird
  abgewiesen. Gegenprobe mit Fenster 1: alle drei neuen Tests fallen.
- `myl-net` 0.25.1 (ohne Änderung, Komponentenstand), `myl-siegel` 0.3.2.

### v0.25.0 (2026-11-08): Weiter geht nur Geprüftes, und Falsches zählt (`myl-net` 0.25.0)

⛔️ **Sicherheitsaudit A47.** Die Netzschicht reichte jeden lesbaren Block und
jede lesbare Transaktion sofort weiter, auch ohne gültige Unterschrift: eine
Flut mit Verstärkung über das ganze Netz, und die Gegenstelle blieb unbewertet,
denn ohne Einträge je Topic zählt gossipsub verworfene Nachrichten nicht.

- `validation::urteil_folgt_spaeter`: Blöcke und Transaktionen werden gehalten,
  bis der Knoten mit `NodeCommand::Urteil` entscheidet (`Pruefmarke` in
  `InboundMessage`, `Urteil::{Annehmen, Ignorieren, Verwerfen}`).
- `scoring::standard_parameter` trägt je Topic eine Bewertung nur für verworfene
  Nachrichten (P4, Gewicht -5, quadratisch: vier kurz hintereinander erreichen
  die Grauliste). Die Bewertung der Zustellung bleibt aus. Die Probe, die leere
  Topics verlangte, prüft jetzt die Gewichte.

### v0.24.0 (2026-10-30): Anfragen mit Zweck (`myl-net` 0.24.0)

- ⚑ **`NodeCommand::AnfrageMitZweck`**: wie `Anfrage`, dazu ein Zweck, den
  `NodeEvent::AntwortEingegangen` und `NodeEvent::AnfrageGescheitert` als
  `zweck` zurücktragen (sonst `None`). Die Zuordnung hält nur, was
  unterwegs ist: libp2p meldet für jede ausgehende Anfrage eine Antwort
  oder einen Fehlschlag, und daran wird sie entfernt.
- 📌 Anlass (NODE 0.99.2): Ohne Zweck wusste ein Knoten nicht, worauf ein
  `Nichts` antwortete. Jedes `Nichts` einer gemessenen Gegenstelle zählte als
  abgewiesene Zeitprobe, auch eines auf eine Blocknachforderung; ehrliche
  Mitglieder konnten so Zeugnisse bekommen.

### v0.23.0 (2026-10-29): Das Thema der Zeugen und die Rollen (`myl-net` 0.23.0)

- ⚑ **`GossipTopic::Zeugen`** (`/myelith/zeugen/1`, angehängt): Stücke der
  Zeugen für zustandslose Knoten, höchstens `MAX_ZEUGEN_BYTES` (1 MiB und
  der unterschriebene Kopf). Strukturell prüft der `PayloadValidator` des
  Knotens (Unterschrift eines bekannten Erzeugers).
- ⚑ **`Rolle` und `subscribe_rolle`**: Ein voller Knoten abonniert alles
  ausser den Zeugen (sie sind grösser als der Block, und er braucht sie
  nicht), ein zustandsloser alles ausser den Transaktionen und dazu die
  Zeugen. ⛔️ Keine Transaktionen, weil er ihre Nummern nicht prüfen kann:
  Er reichte sonst alte, gültig unterschriebene Transaktionen weiter, die
  ein Angreifer kostenlos wiederholt.
- Test `die_rollen_trennen_zeugen_und_transaktionen`; 122 Tests grün,
  Clippy sauber. (Der Kopf nannte hier noch 0.21.4.)

### v0.22.0 (2026-10-28): Vorrang für Pod-Mitglieder in den Verbindungsgrenzen (`myl-net` 0.22.0)

- ⚑ **`NodeCommand::Vorrang { peers }`** (Fund 674): Die genannten
  Gegenstellen zählen nicht gegen die Zahlengrenzen
  (`connection_limits` mit seiner Ausnahmeliste, ersetzt die vorige
  Liste), höchstens **`MAX_VORRANG` = 64**. Der Knoten setzt dort die
  Mitglieder seines Pods (NODE 0.77.0). Vorher scheiterte das Wählen zu
  einem Mitglied, sobald die 16 ausgehenden Plätze belegt waren, und die
  Zeitprobe erreichte es nicht (im Lauf mit 29 Knoten 810 von rund 1 500
  Proben). ⚠️ Für Gegenstellen mit Vorrang gilt auch die Grenze je Peer
  nicht (die Ausnahmeliste ist ganz oder gar nicht); der Deckel hält die
  Fläche klein, die Grenze je Adressbereich bleibt.
- Probe `ein_pod_mitglied_mit_vorrang_bekommt_seinen_platz` (ein
  ausgehender Platz, belegt: ohne Vorrang scheitert der zweite, mit Vorrang
  gelingt er, ein Dritter ohne Vorrang bleibt draußen).
- Im Podlauf mit 29 Knoten (vier Pods) belegt: Zeitproben nicht erreicht
  0 statt 810, Messungen verfallen oder verloren 2 von 476 statt 68 von
  716 (TESTCLIENT 0.57.0).
- `myl-net` 0.22.0. Tests grün, Clippy sauber.

### v0.21.4 (2026-10-28): Bündel mit Ausgabenwurzel (`myl-net` 0.21.4)

- Die Bündel in Proben und Prüfwegen tragen die neuen Felder
  `ausgabenwurzel` und `vertreter` (SHARED_TYPES 0.55.0); kein anderes
  Verhalten. Mit der Vertreterliste am Ende ist ein Bündel nicht mehr
  durchweg fester Länge: Die Proben `fuer_feste_typen_ist_die_borsh_pruefung_eine_laengenpruefung`
  und `verstuemmelte_nachrichten_stuerzen_nicht_ab` lassen die Länge der
  Liste unberührt und prüfen die Felder davor wie bisher; die Moduldoku von
  `validation.rs` sagt es, wie die Probe es verlangt.
- `myl-net` 0.21.4. 120 Tests grün, Clippy sauber.

### v0.21.3 (2026-10-26): `myl-siegel` ohne `unsafe`, festgeschrieben (`myl-net` 0.21.3, `myl-siegel` 0.3.1)

- `#![deny(unsafe_code)]` in `myl-siegel`. Zwölf der dreizehn
  Protokollkisten trugen die Zeile schon; ausgerechnet die Kiste mit der
  Sitzungsverschlüsselung nicht. Sie enthielt kein `unsafe`, jetzt kann
  keines unbemerkt hinein. Gegenprobe: ein eingefügter `unsafe`-Block
  bricht den Bau. Gefunden bei der Bestandsaufnahme für das Audit.
- `myl-net` 0.21.3 (ohne Änderung, Komponentenstand), `myl-siegel` 0.3.1.

### v0.21.2 (2026-10-25): Schlecht bewertete Peers mit Kennung (Fund 618; `myl-net` 0.21.2)

- `scoring::schlecht_bewertete` nennt die Peers unter der Gossip-Schwelle
  mit Kennung, `Netzzustand::schlecht_bewertete` trägt sie zum Knoten;
  `schlechte_peers` zählt daraus. Bis hierher stand nur die Zahl im
  Protokoll: Im Netzlauf bewertete der Erzeuger sechs Gegenstellen unter der
  Schwelle, während ein Knoten kein Gossip mehr bekam, und welche es waren,
  ließ sich nicht sagen.

### v0.21.1 (2026-10-24): `tokio` 1.54.1 wegen RUSTSEC-2026-0386 (`myl-net` 0.21.1)

- Die Sperrdatei hebt `tokio` von 1.53.1 auf 1.54.1: Die sicheren Konstruktoren von `AsyncFd` verletzen die I/O-Sicherheit (RUSTSEC-2026-0386, `unsound`, neu seit dem 2026-10-23), und cargo-deny war damit rot. Der eigene Code nutzt `AsyncFd` nicht; geändert hat sich nur die Sperrdatei, der Vorrat trägt die neue Fassung.

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
