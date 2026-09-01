# networking (`myl-net`)

> **Version:** 0.14.1
> **Datum:** 2026-09-01
> **Status:** **Phase 1 und 2 abgeschlossen** (1.1–1.6, 2.1–2.3),
> **Phase 3 umgesetzt** (3.1–3.4, Abschluss unter Reviewvorbehalt), dazu Punkt 4.2 (Fuzzing der Wire-Protocol-Parser), Punkt 4.3
> (Verbindungsgrenzen und Peer-Diversität) und seit dem 26. August
> Punkt 4.1 (Chaos-Tests, **außer** IP-Paketverlust).
>
> Phase 1: 20-Knoten-Vollkonnektivität unter 5 s, ungültige Nachrichten
> werden nicht weiterverbreitet, sechs Gossip-Topics, Anfragekanal.
> Phase 2: Paarlatenzmessung mit EMA-Glättung, Latenz-Atteste,
> LatencyGraph, Geo- und AS-Diversität. Phase 3: verschlüsselte
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
        ├── latency.rs         Paarlatenzen: EMA in Festkomma, Atteste,
        │                      LatencyGraph für die Pod-Bildung
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

### v0.14.1 – 2026-09-02 (⛑ ein Test, der nur auf einer ruhigen Maschine bestand)

`eine_partition_trennt_und_heilt_wieder` fiel im vollen Lauf um und
bestand einzeln dreimal. Der Unterschied war die **Last**: 28,6 Sekunden
statt 3,6. In der Zeit war die Sperre noch nicht wirksam, die Nachricht
kam durch, und der Test meldete einen Fehler, den es nicht gab.

⚑ **Ein Test, der nur auf einer unbeschäftigten Maschine besteht, ist
kein Test, sondern ein Wetterbericht.**

Vier feste Wartezeiten in `chaos.rs` warten jetzt auf die **Wirkung**
statt auf die Uhr: `warte_auf_trennung` mit großzügiger Frist. Wer früher
fertig ist, wartet nicht länger; wer länger braucht, bekommt die Zeit.

⚑ **Eine feste Wartezeit bleibt, und das ist Absicht:** Dort wird
erwartet, dass **nichts** geschieht, und auf ein Ausbleiben kann man
nicht warten. Sie ist die ungefährliche Richtung, denn zu kurz ergibt ein
falsches **Bestehen** und keine falsche Meldung.

**Die Stelle war am selben Tag benannt und stehen gelassen worden**, als
die vier Kopien von `warte_auf_peers` zusammengeführt wurden. Sie hat
sich noch am selben Tag gemeldet.

### v0.14.0 – 2026-09-01 (die Umwandlung, die seit je versprochen war)

`peer_id_aus_bytes` und `netzadresse` (Fund 117). Der Kommentar an
`PeerIdBytes` verwies für die Umwandlung seit je auf NETWORKING; **hier
stand sie nicht.**

⚑ **Und eine Prüfung, die man erwarten würde, gibt es nicht:** Jede Folge
von 32 Bytes ergibt eine gültige `PeerId`, denn ein Ed25519-Punkt wird
erst beim Rechnen geprüft. Ein Test hielt zuerst das Gegenteil fest und
fiel um; **er hatte unrecht, nicht der Code**. Für die Registrierung
heißt das: **Eine falsche Netzadresse ist beim Eintragen nicht
erkennbar**, und das einzige Signal bleibt die ausbleibende Antwort. Das
passt zur Entscheidung zu Punkt 46 und macht sie schärfer.

### v0.13.0 – 2026-09-01 (das Topic für Ausfallmeldungen, Punkt 22)

**`/myelith/pod-failures/1`**, das siebte Topic. Ein Mitglied behauptet,
eine Position sei ausgefallen, die übrigen zeichnen gegen; **ohne
Verbreitung erreicht die Behauptung die Gegenzeichner nicht**, und genau
die fehlte. Frist, Gegenzeichnung und Beschluss standen seit dem
2026-09-01 in COMPUTE_PIPELINE.

⚑ **Ein eigenes Topic und nicht `PoiBundles`**, aus demselben Grund, aus
dem Stimmen nicht zu den Blöcken gehören: klein, kurzlebig und nur für
einen Pod gegen groß, endgültig und für jeden. Im selben Topic teilten
sie Mesh, Bandbreite und Bewertung.

**Die Grenze ist gerechnet, nicht geraten:** Epoche (8), Pod (4),
Position (4), Gemeldeter (32), Melder (32), Unterschrift (96), also 176
Bytes; aufgerundet auf 512. Den Inhalt prüft ein `PayloadValidator`, denn
sein Typ liegt oberhalb der Netzschicht, genau wie bei Blöcken.

⚑ **Beim Anlegen aufgefallen: `GossipTopic::ALLE` und `all()` führten
dieselbe Menge zweimal auf**, und die neue Variante landete prompt nur in
einer von beiden. `all()` gibt jetzt `ALLE` zurück. Ein Test prüft
zusätzlich, dass keine zwei Topics denselben Namen tragen; zwei gleiche
Namen teilten ein Mesh, ohne dass es jemand sähe.

**Berichtigt:** Der Doc-Kommentar zu `TOPIC_LATENCY_ATTESTS` nannte die
Atteste „Grundlage des LatencyGraph für die Pod-Bildung". Die
Entscheidung 3b hat die gemessene Latenz aus der Pod-Bildung genommen;
die Atteste bleiben als Messung, **sie bestimmen nichts mehr**.

### v0.12.1 – 2026-09-01 (vier Kopien einer Wartefunktion, die auseinandergelaufen waren)

`warte_auf_peers` stand in vier Testdateien, und die vier waren nicht
gleich. ⚑ **Der gefährliche Unterschied war der letzte:** `chaos` brach
ab, wenn der Knoten weg war (`expect`), `nat` zählte ihn als null Peers.
In einem Chaos-Test ist ein verschwundener Knoten der **Versuchsaufbau**;
ein Abbruch mitten im Szenario ist kein Ergebnis.

Dazu: drei gaben die Zahl zurück, eine brach bei Fristablauf ab; der Takt
war einmal 50 und dreimal 100 ms. **Wer ein Muster übernahm, bekam ein
anderes Fehlerverhalten, ohne es zu merken.**

Jetzt eine Fassung in `tests/gemeinsam/`: Ein fortgefallener Knoten zählt
als null Peers, bei Fristablauf kommt die Zahl zurück, und die Bewertung
macht der Aufrufer. `sitzung` behält seinen Abbruch, aber als **eigene
Zeile** über der gemeinsamen Fassung.

### v0.12.0 – 2026-09-01 (die Pingfrist wird geprüft statt abgewartet)

`cleanup_stale_pings` las die Uhr selbst. Damit war ihre Grenze **gar
nicht prüfbar**: Ein Test konnte nur echte Zeit verstreichen lassen.
`cleanup_stale_pings_zu(jetzt)` nimmt das Jetzt als Argument, die Frist
heißt jetzt `PING_FRIST` und steht an einer Stelle.

⛑ **Der alte Test schlief sechs echte Sekunden** und war damit die
gesamte Laufzeit der Bibliothekssuite. ⚑ **Geprüft hat er die Frist
trotzdem nicht:** Sechs Sekunden liegen weit hinter fünf, er hätte auch
eine Frist von einer Sekunde bestanden. Der neue prüft **beide Ränder**,
eine Millisekunde davor und genau darauf.

**6,01 s auf 0,01 s**, und schärfer als vorher. Das fällt selten
zusammen; hier fällt es zusammen, weil beides dieselbe Ursache hatte:
eine Funktion, die ihre Zeit selbst besorgt.

### v0.11.3 – 2026-08-31 (die NAT-Prüfung hing an einer Frist, die für etwas anderes bemessen war)

⛑ **Ein Fehlschlag, der nichts über den Code sagte.** `tests/nat.rs` fiel
einmal aus, während auf derselben Maschine zwei Übersetzungsläufe
liefen; zwanzig Wiederholungen danach waren grün. Ein Test, der unter
Last falsch rot wird, ist kein Test mehr, sondern ein Geräusch, das man
sich abgewöhnt zu lesen.

**Die Ursache war eine geteilte Zahl.** Der Hilfsknoten wartete fünf
Sekunden auf seine erste Horchadresse, und diese fünf Sekunden waren für
den **Negativtest** bemessen, der das *Ausbleiben* einer Reservierung
belegt. Dort ist die Frist die Behauptung selbst und gehört kurz. Auf
dem Positivpfad ist sie nur eine Geduldsgrenze: **Zu lange zu warten
kann dort nichts falsch bestätigen**, zu kurz zu warten dagegen lässt
einen richtigen Lauf scheitern.

Die beiden Fristen sind jetzt getrennt, `FRIST_ERWARTET` mit dreißig
Sekunden und `FRIST_AUSBLEIBEN` mit fünf, jede mit ihrer Begründung. Wer
keine Adresse bekommt, liest im Abbruch, welche Frist verstrichen ist.

⚑ **Und ein blindes Warten bleibt vermerkt:** Nach `ExterneAdresse`
schläft der Test 200 ms, weil dieses Kommando als einziges keinen
Rückkanal trägt; `Dial`, `Publish` und `PeerCount` haben einen. Der
saubere Weg wäre einer an dieser Marke, und das ist eine Änderung am
Kommando-Typ, nicht am Test.

### v0.11.2 – 2026-08-30 (die Bündelwurzel bezeugt jetzt auch das Ergebnis)

`segments_root` nimmt seit SHARED_TYPES v0.13.0 Zeugnisse entgegen,
`Id ‖ Spurwurzel` statt der bloßen Id (Fund 100). Für die Netzschicht
ändert sich nichts an der Prüfung, wohl aber an den Beispielbündeln
ihrer Tests: Sie bauen jetzt Zeugnisse, sonst prüften sie einen Weg, den
es nicht mehr gibt.

### v0.11.1 – 2026-08-29 (die Größe einer Anfechtung nachgerechnet)

Eine Anfechtung trägt seit SHARED_TYPES v0.12.0 eine Unterschrift und
wuchs damit von 176 auf **272 Bytes**. Aus dem Typ gerechnet, alle
Felder haben feste Breite. Die Grenze bleibt bei 64 KiB, also beim
240-fachen: eng genug, dass eine Flut Bandbreite kostet, weit genug,
dass sie keinen Entwurf einschränkt. Die Herleitung steht jetzt bei der
Konstante, wie bei `MAX_CONSENSUS_BYTES` auch.

### v0.11.0 – 2026-08-29 (die Mindestfassung stand da und war falsch)

`rust-version` nannte `1.85`. Dieses Crate baut dort nicht: Über libp2p
hängt die `icu_*`-Kette (via url/idna, verlangt 1.86) und `time`
(verlangt 1.88). ⚑ **Aufgefallen ist es nie, weil nichts es prüfte:**
Alle CI-Jobs bauen mit `stable`, derzeit 1.97, und clippy prüft die
Angabe nur gegen die eigene Kiste, nicht gegen die Abhängigkeiten.

Gemessen gegen echte Toolchains mit `--locked`, nicht geschätzt: 1.85
und 1.86 scheitern, 1.88 trägt. Die Angabe lautet jetzt `1.88`, mit der
Herleitung daneben, und ein CI-Job fährt sie.

Das ist keine Verschärfung, sondern eine Berichtigung: Der Code brauchte
1.88 schon vorher, es stand nur etwas anderes da.

### v0.10.2 – 2026-08-29 (die Größentabelle nachgerechnet)

Die Herleitung von `MAX_CONSENSUS_BYTES` verlangt von jedem, der eine
Nachricht an das Konsens-Topic anschließt, dass er die Tabelle
nachrechnet. Mit dem Commit-Zertifikat (⚑ Fund 67, CONSENSUS v0.20.0)
ist eine dazugekommen. Gemessen statt geschätzt: 301 B bei 5, 813 B bei
21, 4237 B bei 128 Unterzeichnern. Es trägt dieselbe Unterzeichnerliste
wie ein Polka, aber keinen Vorschlag davor, bleibt also stets die
kleinere der beiden, und die 8 KiB stehen unverändert.

Die Strukturprüfung der Netzschicht musste nichts lernen: Sie liest die
Nutzlast vollständig als `Konsensnachricht` zurück, und eine hinten
angehängte Marke geht darin von selbst auf.

Die Testzahl im Kopf lag bei 155 und stimmte nicht mehr; nachgezählt sind
es 163.

### v0.10.1 – 2026-08-29 (⚑ Fund 94: ein Test wartete auf etwas Verlorenes)

`ungueltige_nachrichten_werden_nicht_weiterverbreitet` schlug in der CI
fehl: Die gültige Kontrollnachricht erreichte C nicht.

### ⚑ Die Ursache war nicht Langsamkeit, und das ändert die Behebung

`publish_bundle_retry` wartet, bis **Gossipsub den Publish annimmt**,
und dafür genügt B **ein** Mesh-Peer, nämlich A. **Über das Mesh von A sagt
das nichts.** Ist C dort noch nicht drin, nimmt A die Nachricht an, hat
niemanden zum Weiterreichen, und **die Nachricht ist weg**: Gossipsub
sendet nicht nach.

⚑ **Deshalb hätte eine längere Frist nichts geholfen.** Der Lauf
verbrauchte die vollen fünfzehn Sekunden, weil nichts mehr kommen
konnte, nicht weil noch etwas unterwegs war. **Ein Test, der auf etwas
Verlorenes wartet, wartet immer vergeblich**, und wer daraufhin die
Frist erhöht, macht den Lauf langsamer statt grüner.

**Behoben, indem der Test auf das wartet, was er wirklich braucht:** A
muss **beide** im Mesh für dieses Topic haben, bevor B publiziert. Die
Zahl steht in `Netzzustand.mesh` und war schon da; sie wurde nur nicht
benutzt.

⚑ **Und die Wartezeit macht den eigentlichen Prüfschritt erst
aussagekräftig.** Der Test behauptet, eine ungültige Nachricht erreiche
C nicht. Ohne stehendes Mesh hätte diese Stille auch daher rühren
können, dass noch kein Weg bestand — **die Behauptung wäre aus dem
falschen Grund wahr gewesen.**

Der Lauf dauert jetzt 3,9 statt 15 Sekunden; zehn Wiederholungen grün.

**Nicht angefasst:** `zwanzig_nodes_erhalten_die_nachricht` wartet
ebenfalls nur auf Verbindungen statt auf Meshes. Er ist grün, und ein
grüner Test auf Verdacht umzubauen ist geraten, nicht begründet. Die
Bauart ist hiermit benannt.

### v0.10.0 – 2026-08-28 (der Schlüsselaustausch wird hybrid, und der Entwurf verliert eine Eigenschaft)

**Der Sitzungsschlüssel entsteht jetzt aus zwei Geheimnissen: X25519 wie
bisher und zusätzlich ML-KEM-768 (FIPS 203).** Beide gehen verkettet in
dieselbe HKDF-Ableitung. Ein Angreifer muss **beide** Zweige brechen,
den klassischen mit einem Quantenrechner und den anderen gegen die
Gitterannahme.

⚑ **Warum die Signaturen nicht mitkommen, und das ist kein Versehen.**
Ein Polka-Zertifikat ist heute **96 Byte**, unabhängig von der
Validatorenzahl, weil BLS aggregiert. ML-DSA kann das nicht: 21
Validatoren mal 2 420 Byte sind rund **51 KB**, in jedem Rundenwechsel.
Ein aggregierbares Post-Quantum-Signaturverfahren ist nicht
standardisiert. Ein Umstieg wäre kein Bibliothekstausch, sondern ein
Neuentwurf der Konsens-Nachrichtenschicht.

⚑ **Und die Dringlichkeit liegt ohnehin woanders.** Eine Signatur, die
2040 gebrochen wird, ist wertlos: Der Block ist längst final. „Heute
aufzeichnen, später entschlüsseln" trifft die **Vertraulichkeit**, und
die steckt genau hier. Deshalb kommt dieser Teil zuerst und die
Signaturen später.

### Was der Entwurf dabei verliert

**Der bisherige Austausch war nicht-interaktiv.** Jeder Miner kündigte je
Epoche einen signierten Punkt an, und zwei beliebige Miner leiteten
daraus **ohne Handschlag** einen gemeinsamen Schlüssel ab. Das geht mit
einem KEM nicht: Ein KEM ist keine Gruppenoperation, sondern hat eine
Richtung. Der Sender kapselt gegen den Schlüssel des Empfängers, und das
dabei entstehende **Chiffrat muss übertragen werden**.

Daraus folgt eine neue Nachricht, die `Kapsel` (1 088 Byte), und ein
neuer Zustand im Kanal: Die **Senderichtung** steht sofort, die
**Empfangsrichtung** erst, wenn die Kapsel der Gegenstelle da ist. Wer zu
früh öffnet, bekommt `EmpfangNochNichtBereit` statt eines falschen
Schlüssels.

**Jede Seite kapselt für ihre eigene Senderichtung.** Damit bleibt die
Trennung nach Richtung erhalten, die vorher allein aus dem
HKDF-`info`-Feld kam, und sie wird stärker: Die beiden Richtungen teilen
jetzt kein Geheimnis mehr.

### Drei Eigenschaften, die dabei wichtig sind

- ⚑ **Die Kapsel braucht keine Signatur.** Wer sie verändert, führt den
  Empfänger auf einen anderen Schlüssel, und der Tag der ersten
  Nachricht schlägt fehl. Wer sie mitliest, gewinnt nichts.
- ⚑ **Eine gelogene Blattzahl gibt es hier nicht, aber eine gelogene
  Zugehörigkeit schon**, und sie wird geprüft: Eine Kapsel aus fremder
  Epoche, fremdem Pod oder mit fremdem Absender wird abgewiesen, statt
  still einen Schlüssel zu setzen.
- ⚑ **Der Kapselpunkt hängt an der Signatur der Ankündigung.** Ohne diese
  Bindung könnte ein Angreifer den Post-Quantum-Zweig durch einen
  eigenen Schlüssel ersetzen und ihn damit **abschalten**, ohne die
  Signatur zu brechen. Beim Bauen ist genau das einmal
  durchgerutscht: Die Funktion nahm den Kapselpunkt entgegen und
  unterschrieb ihn nicht. Gefunden hat es der Übersetzer über eine
  unbenutzte Variable, nicht ein Test.

### Der KEM-Zweig ist nicht beidseitig beisteuernd, und das ist in Ordnung

Wer kapselt, wählt die Zufälligkeit allein; der Empfänger steuert nichts
bei. Bei Diffie-Hellman ist das anders, und `was_contributory` prüft es
dort weiterhin. Für den Hybrid genügt es, dass **einer** der beiden
Zweige beidseitig ist: Die Ableitung bleibt ununterscheidbar von
zufällig, solange auch nur ein Eingang es ist. Genau deshalb steht hier
ein Hybrid und kein Ersatz.

**Der Probelauf bleibt reproduzierbar.** Die KEM-Saat wird per HKDF aus
derselben 32-Byte-Saat abgeleitet wie der X25519-Schlüssel; dieselbe Saat
ergibt denselben Kapselpunkt. **Die Kapselung selbst ist frische
Zufälligkeit, und das ist kein Determinismusbruch:** Sie wird übertragen,
nicht abgeleitet, und beide Seiten kommen darüber auf denselben
Schlüssel.

### Zwei Tests sind dabei stärker geworden

Der Lauscher-Test und der Gateway-Test scheiterten nach der Umstellung
zunächst **früher** als vorher, an der fehlenden Empfangsrichtung. Das
wäre ein schwächerer Nachweis gewesen: Er hieße nur, dass ein Lauscher
nichts hat. Beide bekommen jetzt **die Kapsel** und scheitern trotzdem,
weil gegen den Kapselpunkt des richtigen Empfängers gekapselt wurde. Das
ist die Aussage, um die es geht.

⚑ **Die Bereitschaftsprüfung steht bewusst zuletzt.** Eine Nachricht aus
der falschen Epoche oder dem falschen Pod ist falsch, gleich ob der
Empfangsschlüssel schon steht; wer zuerst auf die Bereitschaft prüft,
meldet dafür „Kapsel fehlt" und verdeckt den eigentlichen Grund. Vier
Bestandstests haben genau das gezeigt.

### Kosten

`ml-kem` 0.3 (RustCrypto, Apache-2.0 oder MIT, reines Rust). ⚑ **Sie
verlangt Rust 1.85, das Projekt erklärte bisher 1.82.** Angehoben sind
**alle** Crates, nicht nur die drei betroffenen: Zwei Zahlen in einem
Repositorium sind eine Stelle mehr, die veralten kann, und der
Unterschied schützte niemanden. Kein LTS-System liefert 1.82 (Debian 12
hat 1.63); wer dieses Projekt baut, braucht ohnehin `rustup`. Ankündigung plus
1 184 Byte, erste Nachricht je Richtung plus 1 088 Byte, beides weit
unter der Nachrichtengrenze von 4 MB. **Acht neue Gegenproben**, darunter
der Test, dass ein getauschter Kapselpunkt die Signatur bricht.

### v0.9.0 – 2026-08-27 (Punkte 3.1 bis 3.3: verschlüsselte Sitzungen)

⚑ **Umgesetzt heißt hier nicht abgenommen.** Ein
Verschlüsselungsschema, das nur die Augen gesehen haben, die es
geschrieben haben, ist ungeprüft. Ein unabhängiges kryptographisches
Review steht aus und ist die Bedingung, unter der diese Phase als
abgeschlossen gelten darf.

Die drei Punkte kamen in einem Schritt, weil sie
dasselbe Verfahren sind: Ein Kanal ohne Rotation hat kein
Vorwärtsgeheimnis, eine Rotation ohne Kanal hat nichts zu rotieren, und
die Gateway-Strecke ist derselbe Kanal mit einem Dritten dazwischen.

**Warum das nicht der Transport erledigt.** libp2p verschlüsselt jede
Verbindung mit Noise, und für zwei Knoten, die direkt miteinander
sprechen, wäre damit alles gesagt. Zwei Gründe stehen dagegen.

Der erste ist das Gateway. Nutzer und erster Shard sprechen nicht
direkt, sondern über eines. Mit Transportverschlüsselung allein sind das
zwei Verbindungen mit Klartext dazwischen, also genau der Sammelpunkt,
den Kap. 9.2 des Whitepapers ausschließt. Entfallen tut er nur, wenn das
Gateway den Inhalt nicht lesen kann.

Der zweite ist die Herkunft der Zusage. Eine Eigenschaft, die aus dem
Transport kommt, muss jeder neue Transport neu verdienen: TCP, QUIC,
Relais, morgen etwas anderes, und eine Konfiguration entscheidet
darüber. Hier hängt die Verschlüsselung an der Nutzlast, nicht am Weg.

**Das Verfahren.** X25519 je Epoche, HKDF-SHA256 mit Epoche und Pod im
Salz, ChaCha20-Poly1305, ein eigener Schlüssel je Richtung, der
Sendezähler als Nonce. Kein Handschlag: Beide Seiten kennen den
angekündigten Punkt der Gegenseite aus der Pod-Zuteilung und rechnen
denselben Schlüssel aus. Jeder Shard-Übergang kostet Latenz, und eine
Umlaufzeit vor der ersten Aktivierung wäre Latenz ohne Gegenwert.

**Der Epochenschlüssel wird gezogen, nicht abgeleitet.** Ihn aus einer
Langzeitsaat und der Epochennummer zu berechnen wäre bequem, hieße aber:
Wer die Saat bekommt, bekommt jede vergangene Epoche mit. Vorwärts-
geheimnis heißt, dass das Geheimnis nirgends mehr herleitbar ist. Der
Preis gehört dazu und wird nicht verschwiegen: Ein Knoten, der neu
startet, hat einen neuen Schlüssel und muss ihn ankündigen, bevor er
wieder empfangen kann.

**Der Zähler wohnt beim Schlüssel.** ChaCha20-Poly1305 verträgt keine
zweite Nachricht mit demselben Nonce unter demselben Schlüssel, und bei
Wiederholung fällt nicht nur die Vertraulichkeit, sondern auch die
Authentisierung. Ein zurückgesetzter Zähler bei stehendem Schlüssel wäre
deshalb kein Schönheitsfehler. Zähler und Schlüssel liegen darum im
selben Wert: Es gibt keinen Weg, den einen zurückzusetzen, ohne den
anderen neu zu bauen. Die Regel ist nicht dokumentiert, sie ist gebaut.

**⚑ Der beglaubigte Schlüsselaustausch gehört dazu, sonst trägt nichts
davon.** Alles oben rechnet gegen den Punkt, den die Gegenstelle
angekündigt hat. Wer einen eigenen unterschieben kann, führt beide
Seiten in eine Sitzung mit sich selbst, liest mit und reicht weiter, und
kein einziges Tag geht dabei daneben. Verschlüsselung ohne beglaubigten
Schlüsselaustausch ist keine halbe Sicherheit, sie ist gar keine.

`Epochenankuendigung` trägt den Punkt deshalb mit einer Unterschrift des
**Konsensschlüssels** (BLS) über Trennzeichenkette, Epoche, öffentlichen
Schlüssel und Punkt. Geprüft wird gegen den Endpunkt, mit dem gesprochen
werden soll, und **gegen sonst nichts**: Weil `MinerId` und `Address`
beide `sha256(pubkey)` sind, ist der Endpunkt der Hash des
unterschreibenden Schlüssels. Die Frage „gehört dieser Punkt zu dieser
Gegenstelle?" ist damit vollständig in der Netzschicht beantwortbar,
ohne Register und ohne Zuordnungstabelle.

⚑ **Der erste Entwurf unterschrieb mit der Netzidentität, und das war
prüfbar und trotzdem unbrauchbar.** Der Pod-Pfad nennt `MinerId`s; eine
Unterschrift der `PeerId` beantwortet eine andere Frage, und **die
Zuordnung von `PeerId` zu `MinerId` gibt es im Protokoll nirgends**. Die
Prüfung war echt, sie prüfte nur das Falsche, und aufgefallen ist es
erst beim Nachsehen, wie die Pod-Zuteilung ihre Mitglieder benennt.
Ausgeliefert war diese Fassung nie.

Die Epoche ist mitunterschrieben und wird zusätzlich verglichen: Ohne
die Unterschrift ließe sich eine echte Ankündigung aus Epoche 9 auf 10
umdatieren, der Vergleich wäre zufrieden, und der alte Punkt gälte in
der neuen Epoche. Der Schlüssel wird vor dem Hashen als Gruppenpunkt
geprüft, denn auch ein ungültiger Schlüssel hat einen Hash.

Das Feld ist privat, und `pruefe` ist der einzige Weg heraus. Ein
öffentliches Feld daneben wäre eine Abkürzung, die irgendwann jemand
nimmt, und sie sähe an der Aufrufstelle harmlos aus.

Ein Gleichstandstest in NODE (`tests/gleichstand.rs`) hält die
Ableitung des Endpunkts mit der `MinerId` der Genesis-Datei zusammen.
Die beiden Stellen können einander wegen der Schichtung nicht sehen,
und liefen sie auseinander, passte keine einzige Ankündigung mehr,
während jede Meldung nach einem Angriff aussähe.

**Der Klartextkopf ist lesbar und trotzdem nicht änderbar.** Ein Gateway
braucht Empfänger und Epoche zum Weiterleiten und sieht beides. Der Kopf
geht vollständig als authentisierte Daten ins AEAD ein: ein geändertes
Byte, und das Tag stimmt nicht mehr.

**Rotation ohne Schonfrist.** Nachrichten der alten Epoche, die nach der
Rotation eintreffen, sind verloren. Eine Schonfrist von einer Epoche
würde sie retten und das Vorwärtsgeheimnis um genau diese Epoche
verschieben.

⚑ **Verkraftbar ist der Verlust aus einem anderen Grund, als zuerst
dastand.** Die erste Begründung lautete, eine Sitzung über die
Epochengrenze verliere ohnehin ihre Gegenstelle, weil die
Pod-Zusammensetzung am selben Punkt wechsele. Das stimmt nicht: Ein Pod
tauscht am Epochenwechsel nur die Positionen, deren Miner sich wirklich
ändert, alle anderen laufen mit ihrem Zwischenstand weiter. Tragfähig
ist stattdessen, dass der Verlust die Nachrichten eines Augenblicks je
Stunde betrifft und **sichtbar** ist: Wer eine alte Nachricht bekommt,
erhält einen benannten Fehler und nicht stillschweigend nichts. Die
Wiederholung gehört damit dorthin, wo die Sequenz geführt wird, und
nicht in einen aufgeweichten Schlüsselplan. **Sie ist noch nicht
gebaut.**

**Keine neue Abhängigkeit.** `chacha20poly1305`, `hkdf`, `x25519-dalek`,
`sha2` und `zeroize` lagen bereits im Lock, weil libp2p sie für Noise
mitbringt; der Paketzähler steht vor und nach diesem Schritt auf 362.

**Der Aktivierungstransport bekommt kein eigenes Protokoll**, sondern
reist über `/myelith/anfrage/1`. Anders als bei Gossip gibt es hier kein
geteiltes Mesh und kein gemeinsames Peer-Scoring, das ein Vielredner
vergiften könnte: `request-response` öffnet je Anfrage einen eigenen
Substream.

⚑ **Fund 71: drei Tests, grün aus dem falschen Grund.** Sie waren nach
der Schlüsselableitung benannt und prüften in Wahrheit den Kopfvergleich
des Empfängers, weil der vor der Entschlüsselung greift. Sie wären auch
dann grün geblieben, wenn Epoche und Pod gar nicht ins Salz eingegangen
wären, also genau in dem Fall, den sie ausschließen sollten. Seitdem
vergleicht ein eigener Test die Ableitung selbst, und die drei heißen
nach dem, was sie prüfen.

⚑ **Fund 73: ein Test, der nur meistens grün war.**
`ein_mitschnitt_ueberlebt_die_rotation_nicht` schob den antwortenden
Knoten mit `tokio::spawn` in eine Aufgabe und ließ ihn am Ende fallen.
Sein Kommandokanal schloss, `run_node` fuhr herunter, und die eben erst
abgeschickte Antwort ging mit: „Connection was closed before a response
was received".

**Der Fehler war zwei Tests weiter schon einmal behoben worden**, dort
durch Zurückgeben des Knotens aus der Aufgabe. Behoben wurde die
Stelle, nicht das Muster, und das Muster stand nebenan weiter.

**Gefunden hat ihn der Gesamtdurchlauf, nicht die Testdatei.** Seriell
mit `--test-threads=1` lief sie fünfzehnmal grün; parallel, wie `cargo
test` es von sich aus tut, fiel sie. Ein Fehler, der nur unter Last
auftritt, ist genau der, den ein einzelner Lauf nicht findet.

Behoben wurde diesmal das Muster: Beide Knoten bleiben dem Test
gehören, gearbeitet wird mit `tokio::join!` über zwei geliehene
Verweise, und es gibt keine Stelle mehr, an der ein Knoten zu früh
fallen kann. **Gemessen:** mit dem alten Muster 2 von 15 Läufen rot, mit
dem neuen 0 von 15.

**Ein Test, der nur meistens grün ist, ist schlimmer als keiner.** Er
bringt jemandem bei, noch einmal zu starten.

**Gemessen:** 155 Tests grün (118 Unit, davon 43 in `sitzung.rs`, dazu
37 über Integrationstests), plus drei Gleichstandstests in NODE. `tests/sitzung.rs` fährt echte Knoten: eine
Aktivierung von Shard zu Shard, ein weiterleitendes Gateway, das
annimmt, weitergibt, zurückgibt und beim Öffnen scheitert, ein
Mitschnitt, der die Rotation nicht überlebt, eine Wiedereinspielung über
den echten Draht, und ein Gateway, das dem Nutzer seinen eigenen
Epochenpunkt als den des Shards unterschiebt. Zu jedem Ausschluss steht der erlaubte Fall im
selben Test: „Niemand konnte lesen" beweist nichts, solange nicht
feststeht, dass etwas zu lesen war.

Zwanzig Gegenproben: Für jede Zusage wurde die zugehörige Zeile
gebrochen und nachgesehen, welcher Test rot wird. Jedes Mal war es der
Test, der die Eigenschaft im Namen führt.

**Was diese Schicht nicht leistet, und das steht auch im Modulkopf:**
Sie schützt nicht vor den beteiligten Shard-Minern. Deren Aufgabe ist
die Verarbeitung des Inhalts. Kap. 9.2 sagt das ausdrücklich, Kap. 9.3
zieht daraus die Risikoklasse C. Diese Fassung verschiebt die Grenze
nicht, sie hält sie ein.

### v0.8.0 – 2026-08-26 (Chaos-Tests und die Sperrliste, Punkt 4.1)

**Neu: `NodeCommand::Sperren`.** Eine Sperre wirkt auf die **Peer-Id**,
nicht auf die Adresse. Im Betrieb, um eine als böswillig erkannte
Gegenstelle loszuwerden; im Test, um eine Partition herzustellen, die
sich nicht umgehen lässt.

**Warum keine Adresstrennung und kein Proxy:** `identify` und `kad`
verteilen die echten Horchadressen weiter. Ein Proxy zwischen zwei Knoten
wäre nach kurzer Zeit umgangen, und ein Test, der die Umgehung nicht
bemerkt, misst nichts und meldet Erfolg. Ein eigener Test weist deshalb
nach, dass die Sperre einen **neuen Wählversuch über dieselbe Adresse**
überlebt.

**`tests/chaos.rs`, sechs Tests:** Partition und Heilung, ein
Kontrolllauf ohne Sperre, Sperre gegen neuen Wählversuch, Wiedereinstieg
mit neuer Identität, vier Trennungen hintereinander, ein hängender
Knoten.

**Die Tests fahren echte Knoten über Kommandos**, nicht am Swarm vorbei.
Ein Test, der `behaviour_mut()` benutzt, prüft libp2p; einer, der
Kommandos schickt, prüft den Weg, den ein Knoten im Betrieb nimmt, und
genau dort saßen die Funde 55 bis 57.

⚑ **Was diese Datei ausdrücklich nicht misst: IP-Paketverlust.** Das
Akzeptanzkriterium von Phase 4 verlangt „funktionsfähig bei 10 %
zufälligem Paketverlust". Echter Paketverlust entsteht unter der
Transportschicht und braucht `tc netem` (Linux, root). Verbindungen
abzuschneiden und das Ergebnis „10 % Paketverlust" zu nennen wäre eine
Überbehauptung, **und eine Überbehauptung in einem Härtungstest ist
schlimmer als eine Lücke: Sie wird geglaubt.** Diese Messung gehört auf
Maschinen, auf denen sich der Netzstapel des Betriebssystems
konfigurieren lässt, und nicht in diese Testsuite.

**Zu jedem Störlauf gehört ein Kontrolllauf.** `partitionslauf(true)`
und `partitionslauf(false)` sind derselbe Code mit einem Flag
Unterschied und liefern (0, 1) gegen (1, 1). Ohne das Paar bewiese der
Partitionstest nur, dass in diesem Aufbau nichts ankommt.

### v0.7.1 – 2026-08-26 (eine Herleitung korrigiert)

Nur Dokumentation: Die Begründung der 8-KiB-Grenze rechnete mit einer
Teilnahme-Bitmaske, die es im Typ nicht gibt. Zahlen jetzt gemessen,
siehe unten.

### v0.7.0 – 2026-08-26 (sechstes Topic: die BFT-Runden selbst)

`/myelith/consensus/1` trägt Propose, Vote und Commit.

**Getrennt von `/myelith/blocks/1`, und das ist eine Entscheidung, keine
Ergänzung** (Projektinhaber, 2026-08-25). Beide Klassen tragen
Konsensverkehr, verhalten sich aber entgegengesetzt: Ein Block ist groß,
selten und für jeden interessant; eine Stimme ist 169 Bytes,
rundengebunden und nach der Runde wertlos. In einem gemeinsamen Topic
teilen sie Mesh, Bandbreite und **Bewertung**, und wer das Topic mit
Stimmen flutet, trifft die Blockverbreitung mit.

**Ein Topic für alle drei Nachrichtenarten**, weil sie derselben Runde
angehören und dieselbe Zustellung brauchen: Wer die Votes bekommt, aber
die Commits nicht, hängt.

**Größengrenze 8 KiB, gemessen:** Ein Propose ist 169 Bytes, ein Propose
mit Polka-Zertifikat 469 Bytes bei fünf Unterzeichnern, 981 bei 21 und
4405 bei 128.

*Hier stand zunächst „bleibt unter 512", gerechnet mit einer
Teilnahme-Bitmaske. `PolkaCertificate` führt aber keine Bitmaske, sondern
die Unterzeichner einzeln als `Vec<MinerId>`, also 32 Bytes je Stimme.
Die Schlussfolgerung hielt, die Zwischenrechnung nicht; die Zahlen sind
jetzt ein Test in `myl_consensus::bft`.*

Die Strukturprüfung liegt beim
`PayloadValidator` des Knotens, weil die Typen in `myl-consensus` (L1)
liegen; die Netzschicht darf nicht daran hängen.

⚑ **Eine kopierte Zahl in `limits.rs` behoben.** Die Herleitung von
`MAX_AUSGEHEND` sprach von „alle fünf Topics" und „nicht 5 mal 12". Die
Zahl geht in die Herleitung gar nicht ein, war aber beim sechsten Topic
falsch. Jetzt steht sie nicht mehr da.

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

Repository-weiter Block; die Einzelheiten stehen im Changelog der
jeweiligen Komponente.

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
- Gossip-Topic-Struktur: zunächst fünf Topics mit versioniertem Namensschema
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
