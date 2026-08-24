# compute-pipeline (`myl-pod`)

> **Version:** 0.9.0
> **Datum:** 2026-08-23
> **Status:** 🎉 **Phase 2.1 abgeschlossen** (Punkte 1.1–1.4, 2.1):
> `shard_loop` mit Spur-Hashes und Manipulationserkennung,
> `coordinator_loop` mit Micro-Batching, KV-Cache-Session-Affinität,
> erasure-codierte DA-Archivierung, Micro-Batching-Fenster-Tuning,
> Pipeline-Tracker für überlappende Batch-Verarbeitung.

Pod-Orchestrierung über ein echtes Netzwerk: Pipeline-Routing,
Micro-Batching, KV-Cache-Verwaltung, spekulatives Decoding.
Referenzimplementierung von Whitepaper Kap. 4 und Anhang A.3.

## Aufgabe

Schicht L2 (Compute Layer / Inference Fabric): der „Mining-Loop" — ein Pod
aus k Shard-Minern führt gemeinsam Forward-Pässe aus, koordiniert von einem
Pod-Koordinator, der Micro-Batches bildet und PoI-Bündel einreicht
(Anhang A.3). Diese Komponente ist die **Netzwerk-Orchestrierung** um
INTEGER_LLM herum — sie ersetzt nicht dessen Rechenkerne (`kernels`/
`runtime`), sondern verteilt sie über echte Nodes, mit Session-Affinität,
Ausfallsicherung und Durchsatzoptimierung.

## Abhängigkeiten

NETWORKING (Aktivierungs-Streams zwischen Shards), CONSENSUS
(Epochen-Scheduler liefert Pod-Zusammensetzung) sowie als fachliche
Vorstufe die Mehrknoten-Pipeline von INTEGER_LLM (Stage-Runtime mit echter
Layer-Ausführung über mehrere Knoten): INTEGER_LLMs `pipeline`-Crate liefert
die Stage-Runtime für einzelne Knoten; diese Komponente hebt das auf
Netzwerk-Ebene mit echter Miner-Rotation, Redundanz und Epochen-Wechsel.
`myl-pod` konsumiert die INTEGER_LLM-Stage-API
(`embed_token`/`run_layers`/`head_logits`) und die Typen aus `myl-types`.

## Struktur

```
COMPUTE_PIPELINE/
├── README/                   diese Kurzübersicht + Fahrplan
└── myl-pod/                  das Pod-Crate (Bibliothek + Node-Binary)
    ├── src/
    │   ├── lib.rs             Crate-Wurzel: #![deny(unsafe_code)], Module
    │   ├── wire.rs            Wire-Protokoll zwischen Shards (Borsh, Flags)
    │   ├── trace.rs           Spur-Hashes + Übergangs-Signaturen (BLS)
    │   ├── shard.rs           shard_loop: Eingangs-Prüfung, Forward,
    │   │                      Signieren, Session-Affinität, DA-Archiv
    │   ├── da.rs              DA-Archivierung (ErasureCoder, XOR-Parität)
    │   ├── coordinator.rs     coordinator_loop: Micro-Batching, Dispatch,
    │   │                      PoI-Bündel-Aggregation
    │   └── main.rs            myl-pod-node-CLI
    └── tests/
        └── pod_e2e.rs         Akzeptanztest: Determinismus + Bitgleich +
                               Manipulationserkennung
```

## Changelog

### v0.9.0 – 2026-08-24 (⚑ Fund 52 geschlossen: der Vergütungspfad ist durchgängig)

`build_signed_poi_bundle` schließt die Naht: **erst steht das Bündel,
dann sehen es die Mitglieder, dann unterschreiben sie seine Botschaft,
dann aggregiert der Koordinator.**

**Die Reihenfolge ist die ganze Sicherheit.** Würde der Koordinator
zuerst sammeln und danach das Bündel bauen, könnte er `vtfe_claimed`
nachträglich erhöhen; die Unterschriften lägen über einer Botschaft, die
niemand gesehen hat.

#### Ein Mitglied prüft, bevor es unterschreibt

Eine Unterschrift, die ohne Prüfung gegeben wird, ist **keine
Zustimmung**, sondern eine Anwesenheitsnotiz. Kap. 5.5 belegt falsche
PoI-Aggregation mit 100 % Slash des Koordinators, und das setzt voraus,
dass die Mitglieder etwas anderes bezeugen als „ich war dabei".

`ShardNode::signiere_buendel` rechnet deshalb den Anspruch nach: Passt
`vtfe_claimed` zu der Segmentzahl, die dieses Mitglied gesehen hat, mit
derselben Regel, die der Koordinator anwendet? Weicht sie ab, wird nicht
unterschrieben, und dann gibt es kein Bündel: Ein Aggregat gilt gegen
**alle** Mitglieder, nicht gegen eine Mehrheit.

**Was ein Mitglied lokal nicht prüfen kann, steht dabei:** die
Merkle-Wurzel über die Segmentmenge. Es kennt seine eigenen Segmente,
aber eine Wurzel ist ohne die vollständige Liste nicht nachrechenbar; der
Koordinator liefert sie mit, und die Prüfung ist dann eine über
gelieferte Daten und nicht über eigene. Eine schwächere Aussage, und sie
steht im Code, damit niemand die Unterschrift für mehr hält, als sie ist.

#### Belegt gegen das echte Modell

Der Test fährt einen Vier-Shard-Pod gegen die INTEGER_LLM-Runtime: Das
**unsignierte** Bündel wird von `myl_consensus::verify_bundle_signature`
abgelehnt, das **signierte** gilt, und ein nachträglich erhöhter Anspruch
fällt wieder durch. Ohne Artefakte überspringt er sich.

*Zur Domain-Separation:* Die Bündelbotschaft trägt `DST_POI_BUNDLE` und
ist damit von der Übergangssignatur mit `DST_SHARD_TRANSITION` getrennt.
Ein eigenes Rollenbyte wie in `trace::Rolle` braucht es hier nicht: **Wo
eine Klasse ihre eigene DST hat, ist die Rolle darin schon enthalten.**

### v0.8.0 – 2026-08-24 (Punkt 4.3: der byzantinische Koordinator, und ⚑ Fund 52)

Der Koordinator ist die einzige Stelle im Pod, die **für alle spricht**.
`tests/koordinator_byzantinisch.rs` wehrt fünf Angriffe ab, die
Gegenprobe steht davor.

#### ⚑ Fund 52: Der Pod baut ein Bündel, das der Konsens nicht prüfen kann

`build_poi_bundle` aggregiert die **Übergangs-Signaturen** der Segmente
(`DST_SHARD_TRANSITION ‖ Rolle ‖ Borsh(TransitionSig)`).
`myl_consensus::verify_bundle_signature` prüft gegen die
**Bündelbotschaft** (`DST_POI_BUNDLE ‖ epoch ‖ pod ‖ segments_root ‖
vtfe_claimed`). **Zwei verschiedene Botschaften; ein Bündel aus dem Pod
verifiziert nie.**

**Die Richtung ist die gute:** abgelehnt statt angenommen, niemand bekäme
Vergütung, die ihm nicht zusteht. Es heißt aber auch, dass **überhaupt
niemand** Vergütung bekommt — der PoI-Pfad ist nicht ungeprüft, er ist
unbenutzbar.

**Was fehlt, ist ein Protokollschritt, keine Zeile Code.** Die Mitglieder
müssen das **fertige** Bündel sehen und seine Botschaft unterschreiben;
erst dann gibt es ein Aggregat, das gegen die Mitgliedermenge gilt. Der
Koordinator kann das nicht allein, sonst wäre die Zustimmung der
Mitglieder eine Fiktion, und genau gegen diese Fiktion ist die Signatur
da (Kap. 5.5: 100 % Slash bei falscher Aggregation).
`Coordinator::signierbotschaft` liefert die Botschaft; die Runde steht im
Fahrplan.

**Warum es niemandem aufgefallen ist:** `myl-pod` hing bis heute nicht an
`myl-consensus` und umgekehrt. Beide Seiten sind für sich getestet, die
Naht dazwischen hat nie jemand zusammengesteckt. Genau der Fall, für den
die Härtungsschleife geschrieben wurde.

*Nebenbei:* Die Signierbotschaft steht jetzt an zwei Orten, weil `myl-pod`
nicht an `myl-consensus` hängen soll. **Deshalb prüft ein Test über 1000
zufällige Bündel, dass beide Kodierungen bitgleich sind** — eine Dublette
ohne diesen Test liefe irgendwann auseinander, und dann wäre der Streit
nicht mehr entscheidbar.

### v0.7.0 – 2026-08-24 (Phase 3: Ausfallsicherung und Epochen-Übergang)

`src/standby.rs`: Standby-Übernahme (3.1) und KV-Cache-Rebuild bei
Ausfall oder Epochenwechsel (3.2).

**Kap. 6.8 macht eine quantitative Zusage**, und beide Hälften werden
geprüft: bis zu zwei gleichzeitige Ausfälle übersteht die Session, **drei
nicht**. Die zweite Hälfte ist die, die man vergisst; eine
Implementierung, die bei drei stillschweigend weiterliefe, verspräche
eine Redundanz, die es nicht gibt.

**Warum die Übernahme nicht „einen anderen Miner nehmen" ist:** Der
Standby hat keinen KV-Cache und muss ihn **bitgleich** nachbauen, sonst
weicht die Spur ab dem Übernahmezeitpunkt ab und der Redundanzvergleich
meldet einen ehrlichen Pod als fehlerhaft.

**Bitgleich ist hier billig zu haben, und das ist kein Zufall:** Die
Rechnung ist ganzzahlig und damit reihenfolgeunabhängig, ein Prefill über
dieselben Token liefert denselben Cache, unabhängig von Maschine und
Parallelisierung. **In einem Gleitkomma-System wäre der Cache-Rebuild ein
Bruch der Sitzung.** Das ist eine Folge der Grundentscheidung des
Projekts, die im Whitepaper so nicht steht.

`RebuildAnlass` hat **genau zwei Werte**, und der Typ ist die
Durchsetzung von Kap. 4.2 („nur bei Ausfall oder Epochenwechsel
ausgelöst"): Ein dritter Grund ließe sich nicht eintragen, ohne dass
jemand ihn benennt. Der Epochenwechsel liefert Rebuild-Aufträge **nur für
gewechselte Positionen**; wer bleibt, behält seinen Cache.

#### Zwei Fallen, die der Bau sichtbar gemacht hat

- **Eine doppelt gemeldete Ausfallmeldung darf keine zweite Reserve
  verbrauchen.** Im Netz sind doppelte Meldungen der Normalfall.
  Verbrauchte jede einen Platz, wäre die Zusage „zwei Ausfälle" in
  Wahrheit „eine Meldung", und ein Angreifer, der dieselbe Meldung
  dreimal schickt, verlöre die Session ohne jeden Ausfall.
- **Ein Miner darf nicht zweimal im Pod stehen.** Sonst wäre sein Ausfall
  zwei gleichzeitige, und die Zusage rechnete mit einer Redundanz, die es
  nicht gibt.

#### ⚠️ Punkt 3.3 trägt kein volles Häkchen

Die Schnittstelle steht, die **Verdrahtung nicht**: Es gibt keine Stelle,
die den Scheduler befragt und das Ergebnis einspeist, weil es **kein
Knoten-Binary gibt**, das `myl-scheduler` und `myl-pod` zusammenführt.
Dieselbe Lücke wie bei K1.

### myl-pod v0.6.0 – 2026-08-23 (Fund 41: die Manipulationserkennung ging leer durch)

Angelegt wurde eine adversariale Testebene für das Drahtformat (K4).
**Sie fand beim ersten Lauf einen Fehler**, und zwar einen ernsten.

`verify_input_hash` lieferte bei **leerer Spur** `true`, mit der
Begründung im Code: *„Shard 0: noch kein Spur-Eintrag, Token-Eingang."*
Für Shard 0 stimmt das, aber **dieser Zweig wird von dort gar nicht
erreicht**: Der Token-Eingang kehrt in `process` vorher zurück. Erreicht
wird er ausschließlich auf dem Aktivierungspfad, und dort heißt eine
leere Spur: Jemand schickt Aktivierungen ohne jeden Nachweis, woher sie
kommen.

**Zwei Folgen, beide schlecht.** Die Manipulationserkennung, also der
Kern von Anhang A.3 Schritt 2, ging **vacuously** durch: Ein Shard
rechnete auf fremden Zahlen weiter. Und passte deren Länge nicht zum
Modell, endete das in einer **Panik** im Kernel (`rmsnorm_i16` prüft per
`assert_eq!`), also in einem Absturz, den jeder auslösen kann, der Bytes
schicken darf. Im offenen Netz ist das ein Denial-of-Service.

**Behoben zweifach**: `None => false`, denn eine leere Spur belegt
nichts, und eine Längenprüfung vor dem Kernel, die aus der Panik ein
`Err` macht. Ein Kernel darf nie mit einer Eingabe laufen, deren Länge
nicht zum Modell passt.

**Die neue Testebene** (`tests/adversarial.rs`, fünf Tests): zufällige
und verstümmelte Nachrichten gegen die Deserialisierung, gekippte Bits in
gültigen Nachrichten, abgeschnittene Nachrichten, `unpack_tokens` gegen
jede Nutzlast, und 2000 fremde Nachrichten gegen einen echten Shard, die
**alle** abgelehnt werden müssen.

> **Auch dieser Test war zuerst wertlos, und das steht hier, weil es der
> Punkt ist.** Der erste Anlauf zog rein zufällige Bytes, und davon
> deserialisierte in 50 000 Versuchen **kein einziger**: Borsh liest
> zuerst Längenfelder, und ein zufälliges u32 verlangt gleich mehrere
> Gigabyte. Der Test prüfte also nur, dass Ablehnen nicht abstürzt, und
> nie, was mit einer angenommenen Nachricht geschieht. Aufgefallen ist es
> allein an der Auskunftszeile, die die Null zeigte. Jetzt wird
> strukturiert gezogen, gültiger Kopf und zufälliger Rest, und **1156 von
> 50 000 kommen durch**; eine Zusicherung hält das künftig fest.

### myl-pod v0.5.0 – 2026-08-23 (Spur je Layer, und zwei Funde am Weg dorthin)

**Der letzte Blocker der variablen Knotenzahl ist weg.** Die Spur hatte
einen Eintrag je **Shard**, ihre Länge hing also am Zuschnitt.
`myl_verifier::compare_commitments` lehnt ungleiche Spurlängen mit
`LengthMismatch` ab, und damit mussten zwei redundante Pods denselben
Zuschnitt tragen. Genau das verbietet der Entwurf für variable
Knotenzahl je Pipeline, dessen gemischte Paarung rund 600-mal sicherer
ist als zwei schnelle Pipelines.

Jetzt trägt die Spur einen Eintrag je **Layer**: `num_layers` Einträge,
gleichgültig ob ein Shard rechnet oder vierundzwanzig. **Gemessen** an
0,5B über k = 1, 2, 3, 4, 6, 8, 12 und 24: dieselbe Spur, Eintrag für
Eintrag, und `compare_commitments` urteilt bei vier gegen acht Shards
`Match`.

Dass ein Aufruf je Layer dasselbe liefert wie ein Bereichsaufruf, ist
nicht hergeleitet, sondern gemessen (`tests/layer_granular.rs`, drei
Positionen, echtes Modell samt KV-Cache). **Der Dekodier-Digest hat sich
nicht bewegt:** `272f1ee8f45f2c78` vor und nach dem Umbau, bei jedem
Zuschnitt.

**Eine Signatur je Shard, auch wenn die Spur je Layer wächst.** Die Spur
ist der Vergleichsgegenstand und braucht Layer-Granularität; die
Signatur ist die Zuschreibung und braucht Shard-Granularität, denn
geslasht wird ein Shard und keine Layer.

**Nebengewinn:** Die Bisektion grenzt die fehlerhafte **Layer** ein statt
der Layer-Gruppe, bei unverändertem O(log L).

### Fund 1: Spur und Datenarchiv überlebten nur die letzte Position

`DaStore` war mit `(segment_id, shard_index)` verschlüsselt und kannte
**keine Position**. `archive` wird je Token-Position aufgerufen, jede
Position überschrieb also die vorige; am Ende lag nur die letzte im
Archiv. Im Koordinator dasselbe eine Ebene höher: `trace = out_trace`
ersetzte die Spur je Position, `CompletedSegment.trace` trug am Ende nur
die letzte.

**Die Wirkung ist Slashing ehrlicher Knoten.** Die Streitfrist soll dem
Angeklagten erlauben, die Aktivierung an der strittigen Position
offenzulegen; `adjudicate` verlangt sie ausdrücklich. Lag die Abweichung
an irgendeiner Position außer der letzten, konnte ein **ehrlicher** Miner
sie nicht liefern, `adjudicate` sah `NoResponse`, und das heißt schuldig.

Behoben durch die Festlegung des Projektinhabers: **Ein Segment ist eine
Position**, also genau ein Vorwärtspass. Damit entfällt die
Positionsachse, statt nachgerüstet zu werden, und Spur, Archiv und
Bisektion tragen dieselbe Achse, nämlich die Layer.

**Folge für die Vergütung:** Prefill zählt mit. Eine Prompt-Position
emittiert kein Token, rechnet aber denselben vollständigen Vorwärtspass.
Aus 8 Token über 7 Prompt-Token werden 14 Segmente statt einem.

### Fund 2: Die Spur des letzten Shards landete nie im Segment

`ShardOut::Token` und `ShardOut::Prefill` trugen weder Spur noch
Signatur, und der Koordinator übernahm beides ausschließlich aus
`ShardOut::Forward`. Der **letzte** Shard endet aber immer in einem der
beiden anderen Zweige.

Der Redundanzvergleich verglich damit die Arbeit des Shards nicht, der
die Ausgabe erzeugt, also ausgerechnet die des LM-Kopfes. Und bei einem
Pod aus einem einzigen Shard gab es gar kein `Forward`: Die committete
Spur war **leer**, und ein PoI-Bündel darüber hätte nichts belegt.

Aufgefallen, weil die vTFE-Zuschreibung bei `k = 1` plötzlich null ergab.
Ein Test, der eine Eigenschaft prüft, die es vorher nicht gab, hat einen
Fehler gefunden, der vorher schon da war.

### myl-pod v0.4.0 – 2026-08-23 (der Pod beansprucht, was er gerechnet hat)

**`build_poi_bundle` beanspruchte als vTFE die Zahl der Segmente.** Im
Code stand dazu „Platzhalter für die FLOPs-Metrik". Ein Bündel über
tausend Token beanspruchte damit dieselbe eine Einheit wie eines über
zwei, und ein Shard mit sieben Layern dasselbe wie einer mit zweien.

Der COMPUTE_PIPELINE-Fahrplan warnte wörtlich davor, die Zuschreibung
festzulegen, *„bevor die erste Implementierung sie stillschweigend
trifft"*. Sie hatte sie längst getroffen.

**Jetzt:** `Coordinator::beanspruchte_vtfe()` rechnet nach der Regel aus
`myl_tokenomics::vtfe` (v0.3.0), also nach dem Anteil eines Shards an den
Multiplikations-Additionen der Gewichtsmatrizen eines vollen
Vorwärtspasses, mal der Zahl der erzeugten Token. `CompletedSegment`
trägt dafür neu die Token-Zahl; ohne sie ist die Gutschrift nicht
auszurechnen. `ShardNode::modell_profil()` und `ShardNode::zuschnitt()`
reichen die nötigen Maße heraus.

**Gemessen:** acht Token über vier Shards ergeben **7 999 999** von
8 000 000 Einheiten; die eine fehlende ist die Abrundung, die
ausdrücklich nach unten geht.

**Als Test festgehalten ist die Eigenschaft, auf die es ankommt:**
Zuschnitte von 1 bis 24 Shards beanspruchen dieselbe Summe
(`beanspruchte_arbeit_haengt_nicht_am_zuschnitt`). Ohne sie wäre die
gemischte Paarung aus dem Entwurf für variable Knotenzahl ökonomisch
nicht neutral, und der billigere Zuschnitt setzte sich durch, ohne besser
zu sein. Ein zweiter Test hält fest, dass der LM-Kopf beim letzten Shard
durchschlägt: Bei gleich vielen Layern beansprucht er mehr als das
Doppelte des ersten.

**Neue Abhängigkeit:** `myl-tokenomics`. Sie hängt selbst nur an
`myl-types`; die Richtung stimmt, denn die Regel gehört zur Ökonomie und
nicht in die Pipeline.

### myl-pod v0.3.0 – 2026-08-23 (Fund 36 abgeschlossen: der Pod gibt seine Zahlen heraus)

**Der Vergleich „Pod gegen Einzelknoten" prüfte, ob die Aufteilung
dieselbe Entscheidung erzeugt, nicht dieselben Zahlen.** Das war der
letzte offene Teil von Fund 36 und der Grund, warum er dort ein ⚑ trug.
`run_prompt` liefert Token; die Logits entstehen im Shard mit dem LM-Head
und verließen ihn nie. Verglichen wurde deshalb Token gegen Token, und
das Ergebnis hieß trotzdem „bitgleich".

Wie grob das ist, steht in Fund 36 mit Zahlen: An 0,5B blieb der Token
unverändert, während 0,1 % der Bytes eines Tensors verschoben waren. Ein
Token ist ein Argmax über 151 936 Zahlen und kippt erst, wenn deren
Rangfolge kippt.

**Neu: `ShardNode::dekodier_digest` und `Coordinator::dekodier_digest`.**
Der Shard mit dem LM-Head führt je Session einen Digest über **Logits und
Token** mit, nach demselben Vertrag wie der Einzelknotenlauf
(`integer_llm_runtime::generate::DekodierDigest`, neu in runtime v0.18.0).
Beide Werte sind damit unmittelbar gegeneinander zu halten.

**Warum im Shard und nicht im Koordinator:** Die andere Lösung wäre, die
Logits herauszureichen. Das sind rund 600 KB je Token für einen Messwert;
der Digest sind 32 Bytes. Gehasht wird strömend, ein Zwischenpuffer über
den ganzen Lauf wären bei 0,5B und 32 Token rund 19 MB.

**Gemessen, nicht behauptet** (0,5B, `myl-test shard`): Pod und
Einzelknoten liefern denselben Wert `df54ef6c89f1a840`, und zwar bei 1, 2,
3, 4, 6, 8, 12 und 24 Shards. Die Gegenprobe lief auch: Mit einem
einzigen um eins verschobenen Logit weit unterhalb des Argmax bleiben die
Token identisch (`f1117a59462f9919` auf beiden Seiten), der Lauf schlägt
aber fehl und meldet ausdrücklich, dass er vor dieser Fassung „bitgleich"
gemeldet hätte.

**Der Akzeptanztest selbst trug den Fehler.**
`tests/pod_e2e.rs::pod_deterministisch_und_bitgleich_mit_einzelknoten`
verglich Token und hieß trotzdem so. Er hält jetzt die Digests
gegeneinander, prüft die Schrittzahl mit und hat eine Gegenprobe
daneben (`ein_verschobenes_logit_bewegt_den_digest`): Ein Digest, der sich
nie bewegt, besteht jeden Gleichheitstest.

**Fund 38, nebenbei und offen:** Der Fahrplan begründet die
`pipeline_hash`-Bindung damit, dass verschiedene Shard-Layouts
verschiedene Token lieferten. Das stimmte, solange die
Boundary-Reskalierung existierte, und die ist seit Fund 20/26 ersatzlos
entfallen. Acht Layouts von 1 bis 24 Shards liefern heute denselben
Digest. Die Bindung bleibt trotzdem im Code, siehe die Begründung im
Fahrplan.

*Zur Nummer: Die Crate stand bereits auf v0.2.4, dieser Kopf nannte noch
0.2.2. Der Sprung auf 0.3.0 zieht beides zusammen und markiert die neue
öffentliche Schnittstelle.*

### myl-pod v0.2.4 – 2026-08-19 (Reed-Solomon hinter der bestehenden Schnittstelle)

**Ein Fund über die eigene Arbeit.** Beim Bau der DA-Schicht für
CONSENSUS 4.3 hatte ich die Erasure-Mathematik in `myl-types` neu
angelegt — richtig — aber übersehen, dass `myl-pod` bereits eine
`ErasureCoder`-Schnittstelle mitbringt und der Modulkopf ausdrücklich
sagt: *„Die beschlossene Reed-Solomon-Variante (k=8/m=4) ist eine
Folge-Implementierung hinter derselben Schnittstelle."* Es gab also
einen vorgesehenen Platz, und ich hatte danebengebaut statt hinein.
Aufgefallen ist es erst, als `myl-testclient` nicht mehr übersetzte.

Jetzt: `ReedSolomonCoder` implementiert die vorhandene Schnittstelle und
setzt auf `myl_types::erasure` auf.

**Er behebt zugleich die dokumentierte Phase-1-Einschränkung.**
`XorParityCoder` legt den Längenkopf ungeschützt an den Anfang von
Fragment 0 und kann deshalb nicht rekonstruieren, wenn ausgerechnet
dieses Fragment fehlt (*„vollständige Kopf-Rekonstruktion folgt mit RS"*
stand im Code). `ReedSolomonCoder` stellt die Länge dem Klartext voran
und **codiert sie mit** — es gibt kein ausgezeichnetes Fragment mehr,
jede Teilmenge der Größe k genügt. Statt einem fehlenden Fragment
verträgt er **vier beliebige**; getestet über alle 495 Kombinationen.

`XorParityCoder` bleibt erhalten, der Modulkopf sagt jetzt aber, welcher
zu nehmen ist. `myl-testclient` nutzt den neuen.

### myl-pod v0.2.3 – 2026-08-19 (Fund 26 + Fund 20: Boundary-Schritt entfallen)

**Die Spur band den falschen Wert.** `ShardNode::process` bildete
`out_hash = activation_hash(&out)` über die Aktivierung in natürlicher
Ausgangsskala und schrieb ihn in die Spur; erst danach reskalierte
`finish()` auf die Boundary-Skala, und **dieser** Wert ging als `payload`
auf die Leitung. Der Folge-Shard prüfte mit
`verify_input_hash(&msg.payload, &msg.trace)` — also den Hash des
reskalierten Nutzdatensatzes gegen den Hash des unreskalierten. Beide
stimmen nur überein, solange die Reskalierung die Identität ist; seit
Fund 20 war sie es nicht. Der E2E-Test lehnte damit selbst die
**unmanipulierte** Aktivierung ab.

Das war mehr als ein roter Test: Die Spur ist die Commitment-Kette, die
VERIFICATION zwischen redundanten Pods vergleicht und die das
Bisektions-Spiel halbiert. Committet sie etwas anderes als das, was
übertragen wird, bindet sie nicht die ausgelieferte Arbeit.

**Behoben, indem der Boundary-Schritt ganz entfällt.** Er war reiner
Verlust ohne Gegenwert: Die Ausgangsskala des Senders ist
`layers[layer_end].residual_in_frac`, die Eingangsskala des Empfängers
`layers[layer_start].residual_in_frac` — und `layer_start` des Empfängers
**ist** `layer_end` des Senders. Beide Seiten lasen denselben Wert aus
demselben Artefakt (erzwungen durch `theta_v_hash`) und rechneten ihn
trotzdem über einen dritten, gröberen Skalar hin und zurück. Entfernt:
`rescale_von_kanal`, `rescale_zu_kanal`, `input_scale`, `output_scale`
und das Feld `boundary_frac`.

**Fund 20 fällt damit mit.** `test_pipeline_multinode.py` ist wieder
bitgleich mit der Einzelknoten-Runtime (vorher Divergenz ab dem sechsten
Token, 2746 gegen 2694); der weiche Zweig im Test ist zurück in ein
hartes `assert` überführt, wie es der Kommentar dort vorsah. Die
Phase-1-Akzeptanz „bitgleich mit Einzelknoten" gilt wieder
uneingeschränkt.

**Nachweis:** `pod_e2e.rs` 2/2 (vorher 0/2), Multi-Node-Integration
vollständig, Konformitätsvektoren 30/30, Gleitkomma-Audit null Treffer.
**θ_v unverändert** — die Einzelknoten-Inferenz war nie betroffen.

**Nachtrag am selben Tag — die Layout-Frage ist gemessen.** Drei Layouts
(4 Shards mit Grenzen 6/12/18, 8 Shards mit 3/6/9/…, und ungleichmäßig
1/7/23) liefern dieselben Token und sind bitgleich mit dem Einzelknoten
(`INTEGER_LLM/tests/integration/test_pipeline_layouts.py`). Damit trägt
der Entwurf „variable Knotenzahl je Pipeline" numerisch; von seinen zwei
Blockern ist der erste weg.

**Korrektur:** Oben stand zunächst, die Layout-Bindung aus Fund 25
blockiere diesen Entwurf. Das stimmt nicht — `verify_layout()` prüft das
Manifest gegen sich selbst, nicht gegen andere Pods. Cross-Pod-Gleichheit
erzwingt keine Codestelle. Die Prüfung bleibt trotzdem: Sie hat gerade
den `sha256:0000`-Platzhalter der 8-Node-Konfiguration gefangen.


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

### v0.2.1 – 2026-08-17 (Phase 2.1: Micro-Batching + Pipelining)
- Micro-Batching-Collector mit konfigurierbarem Zeitfenster (default 250 ms)
  und Max-Batch-Größe (default 32). Pipeline-Tracker für überlappende
  Batch-Verarbeitung (4 Stadien: Receiving, Processing, Finalizing, Completed).
- Neues Modul `micro_batch.rs` mit 10 Tests grün.

### v0.1.4 – 2026-08-13 (Phase 1)
- `shard_loop` (Anhang A.3): Aktivierungen empfangen, Eingangs-Hash gegen
  die Spur prüfen (Manipulationserkennung), Forward-Pass über die
  INTEGER_LLM-Stage-API, Spur fortschreiben, Übergang BLS-signieren,
  weiterreichen; KV-Cache je Session (Session-Affinität, Kap. 4.2);
  DA-Archivierung der Aktivierungen (Anhang A.3 Schritt 6).
- `coordinator_loop` (Anhang A.3): Micro-Batching-Fenster (Default 250 ms),
  Session-/Segment-Id-Zuweisung, Pipeline-Dispatch, PoI-Bündel-Aggregation
  (Segments-Wurzel + BLS-Aggregat).
- **Akzeptanzkriterien erfüllt:** 4-Node-Pod liefert bitgleiche
  Token-Sequenz bei wiederholtem identischem Prompt und ist bitgleich mit
  der Einzelknoten-Runtime; Eingangs-Hash-Prüfung lehnt manipulierte
  Aktivierungen/Spur-Hashes ab (`tests/pod_e2e.rs`, 2 Tests + 13
  Unit-Tests grün, keine Warnungen).
