# compute-pipeline (`myl-pod`, `myl-dispatcher`)

> **Version:** 0.52.1 (`myl-pod` 0.43.1, `myl-dispatcher` 0.11.2)
> **Datum:** 2026-10-26
> **Status:** Phase 1 vollständig, Phase 2.1, **Phase 3 vollständig**
> (3.1 bis 3.3) und Punkt 4.3. `shard_loop` mit Spur-Hashes und
> Manipulationserkennung, `coordinator_loop` mit Micro-Batching,
> KV-Cache-Session-Affinität, erasure-codierte DA-Archivierung,
> Ausfallsicherung mit Standby-Übernahme, Epochenübergang, seit dem
> 26. August die **Verdrahtung mit dem Epochen-Scheduler** und seit dem
> 1. September die **Nachbesetzung aus der Reserve des Netzes** (3.4)
> samt **Gegenzeichnung und Frist** für die Ausfallmeldung (3.5) und der
> **Staffelung nach dem Verstoß-Zähler** (3.6). **115 Tests grün.**
>
> **Offen:** die Verbreitung der Meldung über Gossip (der zweite Teil von
> 3.5) und der Pod-Lauf über getrennte Hosts.
>
> **Seit dem 2026-10-15 der Dispatcher** (`COMPUTE_PIPELINE/dispatcher`,
> Kiste `myl-dispatcher`): wer auf einer Maschine wie viel
> Arbeitsspeicher, Grafikspeicher und Kerne bekommt, mit Vorrang Agent vor
> Vision vor Sprache. Der Teil für das Netz (`network`) ist geplant, nicht
> gebaut.

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

## ⚑ Offener Mangel: Fund 164 (2026-09-03)

**Drei Sitzungsverzeichnisse wachsen unbegrenzt.** `ShardNode` führt
`caches: HashMap<u64, KVCache>` (Session-Affinität, Kap. 4.2) und
`dekodier_digest: HashMap<u64, DekodierDigest>`, der `Coordinator` führt
`completed: Vec<CompletedSegment>`. **Keine wird je verkleinert.**

Dazu kommt: **Jede Anfrage bekommt vom Gateway eine neue
Sitzungsnummer**, und `run_prompt` füllt immer ab Position 0 vor. Der
KV-Cache wird also geschrieben und **nie wieder gelesen**. Die
Session-Affinität steht im Whitepaper und im Feld, aber kein Aufrufer
zieht Nutzen daraus.

Gemessen an der Probepipeline, streng linear:

| Anfragen | abgeschlossene Segmente |
|---|---|
| 1 | 14 |
| 10 | 140 |
| 30 | 420 |

**Warum es hier steht und nicht behoben ist:** `completed` einfach zu
leeren geht nicht, weil `build_signed_poi_bundle` daraus `segmente`
zählt und die Unterschriften aggregiert. Wer räumt, muss zuerst sagen,
**wann ein Bündel gezogen wird** und **wann eine Sitzung endet**. Beides
gehört zu dem Bauschritt, der ohnehin ansteht: ein Knoten, der genau
**einen** Shard hält. Gefunden vom Gesamtlauf
(`TESTCLIENT/myl-testclient/tests/gesamtlauf.rs`).

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
├── README/                   diese Kurzübersicht
└── myl-pod/                  das Pod-Crate (Bibliothek + Node-Binary)
    ├── src/
    │   ├── lib.rs             Crate-Wurzel: #![deny(unsafe_code)], Module
    │   ├── wire.rs            Drahtformat zwischen Shards (Borsh, Flags)
    │   ├── trace.rs           Spur-Hashes und Übergangssignaturen (BLS)
    │   ├── shard.rs           shard_loop: Eingangsprüfung, Forward,
    │   │                      Signieren, Session-Affinität, DA-Archiv
    │   ├── da.rs              DA-Archivierung (ErasureCoder, XOR-Parität)
    │   ├── micro_batch.rs     Micro-Batching-Fenster und Pipeline-Tracker
    │   ├── standby.rs         Besetzung eines Pods: k Positionen, zwei in
    │   │                      Reserve, Übernahme und Epochenwechsel
    │   ├── zuteilung.rs       vom geprüften Epochenseed zur Besetzung:
    │   │                      die Verbindung zum Epochen-Scheduler
    │   ├── coordinator.rs     coordinator_loop: Micro-Batching, Dispatch,
    │   │                      PoI-Bündel-Aggregation
    │   └── main.rs            myl-pod-node-CLI
    └── tests/
        ├── pod_e2e.rs         Akzeptanztest: Determinismus, Bitgleichheit,
        │                      Manipulationserkennung
        ├── adversarial.rs     böswillige Nachbarn und verstümmelte Spuren
        ├── layer_granular.rs  variable Knotenzahl, Zuschnittsinvarianz
        └── koordinator_byzantinisch.rs
                               ein Koordinator, der falsch aggregiert
```

## Changelog

### v0.52.1 (2026-10-27): Die Zeitprobe rechnet bis zu 2 048 Positionen (`myl-pod` 0.43.1)

- `shard::ZEITPROBE_HOECHSTENS` von 16 auf 2 048: Die Probe eines Mitglieds
  richtet sich jetzt nach seiner Zusage (NODE 0.68.0, Fund 646); wer sehr
  schnell ansagt, bekommt die grosse Probe.
- `ShardNode::zeitprobe_bis`: Mit Grenze hört der Shard dort auf (vor jeder
  Position geprüft); die Tür liest sie aus der Probe
  (`myl_types::zeitprobe::als_anfrage_mit_grenze`).
- `myl-pod` 0.43.1. 165 Tests grün, Clippy sauber.

### v0.52.0 (2026-10-27): Der Prefill als Stapel, eine Runde je Shard (Fund 650; `myl-pod` 0.43.0)

- 📌 **Fund 650, der grösste Hebel an der Tür:** Über Knoten mit Latenz
  kostete jedes Prompt-Token eine Runde je Shard, rund 4 s je Token bei vier
  Shards; im langen Lauf lag der Median einer Antwort bei 75 bis 93 s, und
  Aufträge liefen über Epochengrenzen. Jetzt geht der Prefill als
  `Shardanfrage::RechneStapel` (angehängt, Marke 11, höchstens
  `MAX_STAPEL` = 64 Nachrichten) in **einer** Runde je Shard; der Shard
  rechnet die Nachrichten der Reihe nach über dieselbe Prüfung und dieselbe
  Funktion wie einzeln (`Shardantwort::StapelGerechnet`). Jede Position
  bleibt ihr eigenes Segment mit eigener Spur und Unterschrift; an den
  PoI-Segmenten ändert sich nichts.
- **Im Netz belegt** (kurzer Podlauf mit Tor, netem 50 bis 200 ms, 2026-10-27):
  Rechenzeit des Pods je Auftrag 18,6 und 22,0 s (der erste, mit Anlauf,
  35,5 s) statt 37 bis 62 s in den vier kurzen Läufen davor; Antwort an der
  Tür 37 bis 60 s statt 47 bis 83 s. Was bleibt, ist das Dekodieren: Jedes
  neue Token kostet weiter eine Runde je Shard.
- Scheitert der Stapel (auch an einem Shard, der ihn nicht kennt), greift
  die Wiederholung aus Fund 654, dann Token für Token.
- `tests/stapel_prefill.rs` (neu): Bitgleichheit mit dem Weg Token für
  Token (Token, Segmente, Spuren, Unterschriften), vier Stapelrunden statt
  sieben mal vier; der Weg Token für Token als Gegenprobe über einen Shard,
  der den Stapel ablehnt.
- 📌 **Fund 657: `sitzung_vergessen` liess den Generierungszähler stehen.**
  Er wurde nur entfernt, wenn eine Sitzung den Deckel des Shards voll
  erreichte; das Podwerk fragt meist weniger an, also blieb je Sitzung ein
  Eintrag für immer (dieselbe Klasse wie Fund 164), und eine Wiederholung
  derselben Sitzung (Fund 654) bekam nur noch den Rest des Deckels.
  Sichtbar wurde es, als die verlorene Runde mit dem Stapel ins Dekodieren
  rückte: zwei Token statt vier. Jetzt räumt `sitzung_vergessen` ihn mit ab;
  `verlorene_runde.rs` ohne die Korrektur rot.
- `myl-pod` 0.43.0. 165 Tests grün, Clippy sauber.

### v0.51.1 (2026-10-26): Erst rechnen, wenn der neue Pod steht (Fund 649, zweiter Teil; `myl-pod` 0.42.1)

- 📌 Im zweiten langen Lauf griff die Wiederholung über den neuen Pod, kam
  aber zu früh: Gleich nach dem Wechsel liefen bei den neuen Mitgliedern
  teils noch die Shards der alten Epoche; einer meldete „kein Shard“, ein
  anderer bekam ein Token ohne Einbettung. Jetzt wartet das Podwerk, bevor
  es für eine Epoche einen Koordinator baut, bis jedes Mitglied den
  Zuschnitt meldet, den die Kette ihm gibt (`von..bis` aus dem Podbild),
  höchstens `BEREIT_FRIST` (60 s). Das gilt für jeden ersten Auftrag einer
  Epoche, nicht nur für die Wiederholung.
- `tests/podwerk_epochenwechsel.rs` geschärft: Nach dem Wechsel antworten
  die neuen Mitglieder fünf Fragen lang „kein Shard an diesem Knoten“, wie
  ein Knoten, der seinen Shard noch startet; das Podbild nennt echte
  Ebenenbereiche. Gegenprobe ohne das Warten: keine Antwort, rot.
- `myl-pod` 0.42.1.

### v0.51.0 (2026-10-26): Ein Auftrag über die Epochengrenze bekommt seine Antwort (Fund 649; `myl-pod` 0.42.0)

- 📌 **Fund 649**, im langen Lauf über Docker fünf von sechs Ausfällen: Am
  Epochenwechsel beenden die Knoten ihre Shards, auch mitten in einem
  Auftrag; die Wiederholung des Koordinators (Fund 654) trifft dann nur
  noch die alten Plätze, und das Podwerk gab auf. Jetzt fragt das Podwerk
  nach einem leeren Ergebnis das Podbild neu; steht die Kette in einer
  neuen Epoche, rechnet deren Pod den Auftrag **einmal** neu. Eine Kosten-
  oder Wertfrage ist das nicht (gescheiterte Aufträge wurden nie
  abgerechnet), aber eine der Verfügbarkeit: Bei 60 bis 80 s je Auftrag
  über Knoten mit Latenz fiel jeder fünfzehnte Auftrag auf eine Grenze.
- `tests/podwerk_epochenwechsel.rs` (neu): ein Relais im Prozess mit
  demselben Drahtformat wie der Knoten, je Epoche ein Pod aus vier Shards,
  Wechsel nach der sechsten Rechenrunde mitten im Prefill; danach antworten
  die alten Mitglieder „kein Shard an diesem Knoten“. Die Antwort gleicht
  derselben Rechnung ohne Wechsel. Gegenprobe ohne die Wiederholung über
  den neuen Pod: abgelehnt, rot.
- `myl-pod` 0.42.0.

### v0.50.1 (2026-10-26): Überlaufprüfung im Freigabebau (`myl-pod` 0.41.1)

- Überlaufprüfung im Freigabebau für die Protokollkisten (Festlegung des Projektinhabers): `[profile.release.package.<kiste>] overflow-checks = true` für jede Protokollkiste im Baum dieses Programms; die Rechenkerne (`integer-llm-*`) bleiben ohne. Bis hierher war sie nur im Testprofil an, und eine Rechnung im Konsenspfad wäre im ausgelieferten Programm an den Rändern still umgelaufen. Belegt am Knoten (NODE 0.64.0): `myl_ledger` wird mit `-C overflow-checks=on` übersetzt, `integer_llm_kernels` ohne. Alle 4 359 Tests liefen schon immer mit eingeschalteter Prüfung.
- `myl-pod` 0.41.1.

### v0.50.0 (2026-10-26): Eine verlorene Runde kostet nicht den Auftrag (Fund 654; `myl-pod` 0.41.0)

- 📌 **Fund 654**, gesehen im langen Lauf über Docker mit Störer: Eine
  Runde zu Shard 1 ging auf der Leitung verloren (IO-Fehler), dem Shard
  fehlte damit eine Position im KV-Speicher, und jede folgende lehnte er ab
  („hinter dem Ende des KV-Speichers“). Der Koordinator schickte trotzdem
  alle weiter, und der Auftrag scheiterte. Jetzt bricht die erste
  fehlgeschlagene Runde den Durchlauf ab (`pump` meldet sie), die Segmente
  dieses Durchlaufs werden verworfen, die Shards vergessen die Sitzung, und
  `run_prompt` rechnet **einmal** neu. Scheitert auch das, kommt nichts
  zurück und kein halbes Segment bleibt im Bündel. Vorher wurde ein halber
  Durchlauf sogar als Segment vermerkt, sobald ein Shard etwas lieferte.
- `tests/verlorene_runde.rs` (neu): Ein Weg im Prozess verliert genau eine
  Rechenrunde zu Shard 1; die Antwort gleicht der ohne Verlust, die Zahl
  der Segmente auch, keine Kennung doppelt. Gegenprobe im selben Test:
  Bleibt Shard 1 weg, kommt nichts und kein Segment bleibt stehen. Ohne die
  Wiederholung rot.
- `myl-pod` 0.41.0.

### v0.49.1 (2026-10-26): Das Podwerk nennt je Auftrag seine Dauer (`myl-pod` 0.40.1)

- Je Auftrag eine Zeile `[myl-pod] Epoche …: … Prompt-Token, … neue in … ms`.
  Gebraucht im Lauf über Docker: Dort gab die Ortsleitung des Knotens nach
  45 s auf, während das Podwerk noch rechnete, und ohne Zeit war nicht zu
  sagen, ob die Shards rechneten oder das Netz wartete. Gegenstück im Knoten:
  `shardrelais` mit der Dauer jeder Runde.

### v0.49.0 (2026-10-26): Ein Pod über Knoten (`myl-pod` 0.40.0)

- ⚑ `shardweg::UeberDenKnoten` (neu): Der Koordinator fragt die Shards der
  Pod-Mitglieder über die Relaisleitung seines Knotens; der reicht über das
  Netz an den Knoten des Mitglieds und der an seinen Shard. Die Mitglieder
  kommen aus dem Podbild der Kette in Shardreihenfolge. Belegt im TESTCLIENT
  (`pod_ueber_knoten`): ein Torknoten ohne Shard, sechs Miner, ein Pod aus
  vier Shards, acht Token Token für Token wie dieselben Grenzen im Prozess.
- ⚑ **`podwerk::Podwerk`** (neu): ein Ortsdienst, der über den Pod der
  Kette rechnet statt über ein Modell im eigenen Prozess. Er hält nur den
  Wortschatz, fragt je Auftrag das Podbild, baut je Epoche einen
  Koordinator über `UeberDenKnoten` und reicht im letzten Sechstel der
  Epoche das von den Positionen unterschriebene Bündel über die Leitung ein
  (einmal je Epoche, die Länge nennt das Podbild). `myl-pod-node
  --podrelais <host:port> --podrelaisausweis <datei>` wählt ihn; den
  Ausweis liest er erst, wenn er ihn braucht, denn der Knoten startet nach
  ihm. `--knotenschluessel <datei>` leitet den erwarteten Knoten aus dessen
  Konsensschlüssel ab, statt ihn als Hex zu verlangen.
- 📌 Fund 638, Rest: `tests/vier_prozesse_ein_pod.rs` und
  `tests/training_ueber_den_draht.rs` ließen je Shard einen Ordner
  `myl-shardausweis-*` liegen; jetzt weg, sobald der Shard seine Adresse
  nennt (er hat den Ausweis dann gelesen). Mit leerem Temp-Ordner geprüft.
- `myl-pod` 0.40.0.

### v0.48.0 (2026-10-23): Ein Ausweis und eine Gesamtfrist für die Shardtür; das Token gegen das Vokabular (Funde 611, 613; `myl-pod` 0.39.0, `myl-dispatcher` 0.11.2)

Aus dem Sicherheitsaudit vom 2026-10-23.

- ⚑ **Fund 611:** Die Tür des Shards nahm Inferenzschritte, Zeitproben und
  **Trainingsaufträge** ohne Ausweis an. Sie hört auf der Rückschleife, aber
  jeder Prozess der Maschine kann wählen, und den Port zeigt `lsof`: Ein
  fremdes Programm belegte die Tür, liess auf Kosten des Miners rechnen oder
  veränderte den Trainingszustand, den der Miner danach unterschreibt. Mit
  Pods über Maschinen wäre es ein Fernangriff. Jetzt verlangt
  `Shardstelle::oeffnen` (und `oeffnen_mit_training`) einen `Ausweis`
  (32 Bytes), jede Anfrage beginnt mit ihm, in konstanter Zeit verglichen,
  vor der Länge; `UeberDenDraht::neu` nimmt Adresse **und** Ausweis je Shard;
  `myl-shard` verlangt `--ausweis <datei>` und startet ohne nicht.
  **Zweiter Teil:** Tür und Ortsdienst setzten ihre Frist je Lesevorgang;
  wer alle 29 s ein Byte schickte, lief nie ab, auch schon vor dem Ausweis
  (eine Viertelstunde je Verbindung). Jetzt `Fristleser`: eine Frist für die
  ganze Anfrage, beim Ortsdienst je Rahmen.
  **Gegenproben:** eine Zeitprobe mit falschem Ausweis bekommt keine
  Antwort und die Tür meldet „falscher Ausweis“ (`grenzen_der_kette`); ein
  Leser mit Frist je Vorgang lässt zwanzig getröpfelte Bytes durch, der neue
  bricht nach 300 ms ab.
- ⚑ **Fund 613:** Der Shard gab eine Token-Kennung aus der Nachricht
  ungeprüft an `embed_token`; eine Kennung hinter dem Vokabular brachte den
  Prozess zum Absturz, nachdem er den Cache der Sitzung schon herausgenommen
  hatte. Der Zweig der Aktivierungen prüft seine Länge seit Fund 41 genau
  aus diesem Grund. Jetzt wird die Kennung vorher gegen das Vokabular
  gehalten. **Gegenprobe:** Ohne die Prüfung stürzt
  `ein_token_hinter_dem_vokabular_wird_abgelehnt` in `model.rs` ab.
- **Fund 599:** `ctutils` 0.4.5 in der Sperrdatei von `myl-pod`.
- `myl-dispatcher`: Die Liste der `unsafe`-Stellen im Kopf von
  `local/mod.rs` nannte `proc_listchildpids` nicht.
- Belege: Clippy und alle Tests beider Kisten grün, auch die beiden mit
  vier echten `myl-shard`-Prozessen.

### v0.47.1 (2026-10-22): Eine Werkzeugkette für alle Kisten; Fund 595 (`myl-pod` 0.38.1, `myl-dispatcher` 0.11.1)

- `rust-version` auf 1.99 in `myl-pod`, `myl-dispatcher`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  ⚑ **Dazu Code, und zwar mit Absicht:** Mit `rust-version = "1.99"` gibt Clippy
  zwei Regeln frei, die es bei 1.85 zurückhielt (`manual_is_multiple_of`,
  `chunks_exact` mit fester Größe). Festlegung des Projektinhabers:
  umschreiben, nicht abschalten. Hier: `myl-pod` 2 Zeilen (`wire.rs`, eine Probe).
  `x % n == 0` wird `x.is_multiple_of(n)`, `chunks_exact(N)` wird
  `as_chunks::<N>()`. Für `n ≠ 0` bitgleich; jede Stelle mit variablem
  Teiler ist einzeln angesehen und vorher gegen null geschützt (bei
  `is_multiple_of(0)` stünde `x == 0`, wo `x % 0` abbricht).
- 📌 **Fund 595:** `tests/grenzen_der_kette.rs` behauptete das Probeartefakt
  mit `assert!(vorhanden(..))`. Mit `MYL_OHNE_ARTEFAKTE` meldet `vorhanden`
  „überspringen“, und die Probe brach mit „das Probeartefakt fehlt“ ab,
  auch auf einer Maschine, die es hat. Jetzt kehrt sie zurück; ein fehlendes
  Artefakt ohne Erlaubnis meldet `vorhanden` selbst. Die CI fuhr die Datei
  nicht und sah es deshalb nie.
- **Eine Abfrage für alle Proben mit Artefakt** (2026-10-23): Sechs Dateien
  sprangen ohne Artefakt still ab (`if !dir.exists() { return; }`), genau
  das, was Fund 113 für die übrigen Proben abgeschafft hatte, und zwei
  weitere in `grenzen_der_kette.rs` behaupteten es hart. Alle fragen jetzt
  `artefakte::vorhanden`: mit Erlaubnis überspringen, ohne Erlaubnis und ohne
  Artefakt auffallen. Damit fährt die CI `myl-pod` ganz statt nur `--lib`,
  `adversarial` und `koordinator_byzantinisch`.
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen. Nach dem
  Umschreiben: Clippy mit `-D warnings` still (die Kerne über alle vier
  Merkmale), `cargo test` rc=0 in allen zehn berührten Kisten (die Kerne auch
  mit `cpu-simd`), Konformität mit `sparrow-0.6b-int8` 55/55, die vier
  Abdrücke unverändert (`cb696efd3817e575` über 24 Vektoren), Gleitkomma-Audit
  grün.

### v0.47.0 (2026-10-22): Der Pod rechnet nach den Grenzen der Kette, Netz vor Mensch (`myl-pod` 0.38.0, `myl-dispatcher` 0.11.0)

Stufe 4 des Dispatchers (5.7), abgeschlossen mit NODE v0.52.0.

- `myl-pod`:
  - `Pipelinewerk::laden_mit_grenzen`, `Trainingswerk::aus_modell_mit_grenzen`
    und `myl-pod-node --grenzen`: die Grenzen der Kette, geprüft mit
    `grenzen_pruefen` und abgelehnt statt zurechtgerückt.
  - `ShardNode::modell_profil` liest Ebenenarten, Zustandsmaße, geteilten
    Experten und Ausgangstor aus dem geladenen Modell; am echten int4-35B
    gleich dem Profil der Kette.
  - `ShardNode::zeitprobe`; die Tür des Shards beantwortet die Zeitprobe aus
    `myl_types::zeitprobe` vor jeder anderen Anfrage.
  - `myl-shard --konsensschluessel`: Der Knoten startet den Shard mit dem
    Schlüssel, unter dem der Miner angemeldet ist.
  - `myl-shard --von --bis` lädt über `load_model_fuer_netz_bereich`: Ein
    Knoten mit Teilablage startet seinen Shard aus den Dateien seines
    Bereichs (Fund 589, INTEGER_LLM v0.145.0).
  - Die Gegenzeichnung kommt aus `myl_types::gegenzeichnung`.
- `myl-dispatcher` (P8): `Vorrang::Netz` vor `Agent`; `Modellbedarf::bereich`
  für den Shard eines Rechners; eine Ablehnung nennt, wer den Platz hat und
  dass man für die nächste Epoche weniger zusagt (Festlegung E7).
- **Belege:** `tests/grenzen_der_kette.rs` (Inferenz und Training mit
  ungleichen Grenzen gleich, falsche Grenzen abgelehnt, Zeitprobe gebunden,
  auch über die Tür), `das_netz_kommt_vor_dem_agenten_und_sagt_es`;
  `cargo test` rc=0 in beiden Kisten.

### v0.46.0 (2026-10-21): Ebenenzeit im Eichbuch, ein Shard mit beliebigem Ebenenbereich (`myl-pod` 0.37.0, `myl-dispatcher` 0.10.0)

Stufe 4 des Dispatchers, vorgezogen und geplant auf Bitte des Projektinhabers (2026-10-21): Ein Pod verteilt seine Ebenen nach Kapazität und gemessener Ebenenzeit seiner Mitglieder statt gleich. Hier P1 (Eichbuch) und P2 (Ladeweg).

- **Eichbuch** (`myl-dispatcher`): `Eichung` trägt `ebenenzeit_us` und den
  `rechenweg` der Messung, `Eichbuch::ebenenzeit` liest sie; alte Bücher
  ohne die Felder lesen sich weiter.
- ⚑ **`myl-shard --von a --bis b`** (`myl-pod`): der Ebenenbereich als
  Eingabe statt aus `--shard`/`--shards` gerechnet; ohne beide gilt der
  gleiche Schnitt. Erster und letzter Shard folgen aus dem Bereich. Ein
  ungültiger Bereich endet mit Rückgabe 2 und Grund.
- **Was P2 nicht brauchte:** Ein Shard berührt schon heute nur seine
  Ebenen; angewärmt wird im Rechenweg, und nur, was ein Token wählt. Die
  Platte trägt weiter das ganze Artefakt (Festlegung E6); die erste Ladung
  prüft die Summen aller Dateien einmal.
- **Belege:** neue Probe `ungleiche_bereiche_rechnen_dasselbe`: vier echte
  Prozesse mit 3, 7, 13 und 5 Ebenen liefern dieselben Token, denselben
  Abdruck (`2a13b710…`), dieselbe Arbeit (12 999 998) und dieselben 13
  Segmente wie der gleiche Schnitt; Gegenproben für umgekehrten, zu langen
  und halben Bereich; `die_ebenenzeit_steht_im_buch`. `cargo test` und
  Clippy rc=0 für beide Kisten.

### v0.45.3 (2026-10-21): Die Modelle heißen nach Tieren (`myl-pod` 0.36.2, `myl-dispatcher` 0.9.1)

Die Projektmodelle heißen seit heute nach Tieren, mit Größe und Speicherform als Anhängsel (Festlegung des Projektinhabers): `myelith-0.6b` ist `sparrow-0.6b-int8`, `myelith-4b` `pigeon-4b-int8`, `myelith-30b-a3b` `magpie-30b-a3b-int8`, `myelith-35b-a3b` `raven-35b-a3b-int8`. In den Dateien eines Artefakts steht kein Name, also bleiben Abdrücke und Register gültig. Die Proben des Pods rechnen am `sparrow-0.6b-int8` (Vorgabe von `MYL_POD_MODELL`); ein Etikett in den Proben des Dispatchers heißt `raven-35b-a3b-int8`. Kein Verhalten geändert.

### v0.45.2 (2026-10-21): Im Netz rechnet nur das 35B; die Netzproben rechnen im Prüfnetz (`myl-pod` 0.36.1)

**Festlegung des Projektinhabers:** Netzmodell ist allein das 35B (int8 und
int4), als untere Grenze der Leistungsfähigkeit. Einzelheiten unter
INTEGER_LLM v0.141.0.

- **Das ausgelieferte Programm lädt kein kleines Modell mehr im Netz.** Der
  Release-Bau von `myl-shard` lehnt `myelith-0.6b` und `myelith-4b` mit
  ihrem Abdruck ab („steht nicht auf der Netzliste“) und startet mit
  `myelith-35b-a3b-int4` den Shard.
- **Die Proben behalten das 0,6B**: `integer-llm-runtime` steht zusätzlich
  als Entwicklungsabhängigkeit mit dem Merkmal `pruefnetz` im Manifest. Damit
  baut `cargo test` den Pod und seine Programme mit dem Block `pruefnetz` des
  Katalogs, `cargo build --release` nicht; das 35B passt in keinen
  Probenlauf.
- **Belege:** `cargo test` Rückgabe 0 (143 s, alle Netzproben mit dem echten
  0,6B); der Testbau von `myl-shard` startet mit dem 0,6B, der Release-Bau
  nicht. Sperrdateien nachgezogen.

### v0.45.1 (2026-10-21): Der Pod lädt nur, was auf der Netzliste steht (keine Kiste geändert; Fund 585)

- Der Pod lädt auf jedem Weg über `load_model_fuer_netz` (v0.45.0), und das
  prüft seit runtime 0.100.0 den **Abdruck** des Artefakts gegen die
  Netzliste im Modellkatalog, nicht mehr nur `betrieb` (Fund 585, INTEGER_LLM
  v0.140.0). Ein Artefakt, dessen Konfiguration von Hand verändert wurde,
  oder eines ohne Freigabe startet keinen Shard. Nur die Sperrdatei
  nachgezogen; die Proben mit dem echten 0,6B laden es über die Liste.

### v0.45.0 (2026-10-19): Der Netzbetrieb lädt kein privates Modell (`myl-pod` 0.36.0)

Anlass: private lokale Modelle (INTEGER_LLM v0.137.0). Ein Artefakt mit
`betrieb: lokal` in `model_config.json` ist nicht für den Netzbetrieb
zugelassen, etwa Flash-Next unter `qwen-community-1.0`, dessen Lizenz
Inferenz für Dritte nicht erlaubt, oder jedes mit dem Bauskript selbst
gebaute Modell.

- ⛔️ **Jeder Ladeweg des Pods geht über `load_model_fuer_netz`**: der Start
  (`main.rs`), das Pipelinewerk, das Trainingswerk und `myl-shard`. Er prüft
  die Angabe vor dem Laden und bricht mit dem Grund ab; ein unbekannter Wert
  gilt nicht als zugelassen. Der Aufruf steht ausdrücklich da und nicht als
  umbenanntes `load_model`, damit ein neuer Ladeweg ihn nicht still verliert.
- **Beleg:** `cargo test` mit Rückgabe 0, Clippy mit `-D warnings` still;
  die Sperre selbst prüft `ein_privates_modell_ist_fuer_das_netz_gesperrt`
  in der Laufzeit.

### v0.44.0 (2026-10-18): Ein Arbeitskontext für den Agenten vor den häufigen Experten (`myl-dispatcher` 0.9.0, Fund 562)

Anlass: die neue Stufe extrem der Agentenmessung. Das int8-35B schaffte
auf 24 GiB 0 von 15 Aufgaben, und jeder Lauf endete nach höchstens fünf
Werkzeugaufrufen mit „Kontext voll“ bei 8192 Token.

- **Fund 562:** Der Plan gab nach dem Minimum (`KONTEXT_MINDESTENS`, 8192)
  jedem häufigen Experten Vorrang vor jedem weiteren Token Kontext
  (Festlegung vom 2026-10-15). Lief das int8 eine Weile, belegte der
  Dateicache den Speicher, und es blieb beim Minimum. Ein Agent braucht
  aber allein für Systemprompt mit vollem Werkzeugsatz (gemessen 4200 bis
  4540 Token) und Antwortreserve (4000) mehr als 8192; nach dem ersten
  Werkzeugergebnis war nichts mehr zu verdichten. Zwei Zahlen an zwei
  Orten, die nichts voneinander wussten.
- **Behebung, mit Zustimmung des Projektinhabers:** Neue Stufe im Minimum,
  `plan::KONTEXT_VOR_EXPERTEN` = 32 768. Nach den dichten Gewichten kommt
  der Arbeitskontext bis dorthin (höchstens der eingestellte), dann die
  häufigen Experten, dann der Rest des Kontexts. Die Regel vom 2026-10-15
  gilt weiter oberhalb dieser Schwelle. Gemeinsamer Teil beider Schritte:
  `kontext_erweitern`.
- **Warum nicht einfach ein höheres Minimum:** Das Minimum entscheidet über
  die Aufnahme. Die dichten kleinen Modelle brauchen 112 bis 144 KiB je
  Token; bei 32 768 hieße das 3,5 bis 4,5 GiB fest, und ein 4B würde auf
  einem kleinen Rechner abgelehnt. Beim 35B kostet die neue Stufe 480 MiB,
  rund 160 der seltensten unter gut 2000 häufigen Experten.
- **Belege:** `die_haeufigen_experten_kommen_vor_dem_arbeitskontext`
  angepasst (vorher Kontext 4096 und zwei Experten, jetzt 55 296 und einer;
  der alte Wert steht im 📌-Absatz), neu
  `der_kontext_vor_den_experten_geht_vor_und_nur_bis_dahin` (zu wenig
  Speicher: aller Kontext, kein Experte; genau 32 768; 32 768 und ein
  Experte). `cargo test` 48/48, Clippy still. Der Hinweis im Plan sagt
  jetzt „vor dem restlichen Arbeitskontext“.

### v0.43.1 (2026-10-17): Ein Verzeichnis ist auch unter Windows kein leeres Eichbuch (`myl-dispatcher` 0.8.1, Fund 559)

Anlass: Der neue CI-Job „Klient und Agent (windows-latest)“ war beim ersten
Lauf rot, im Schritt `cargo test` dieser Kiste, mit genau einem Test:
`local::eichung::proben::ein_kaputtes_buch_ist_ein_fehler` scheiterte an
der Zusicherung „ein Verzeichnis ist kein Buch“. Unter Wine war derselbe
Test grün.

- **Fund 559:** `Eichbuch::lesen` las eine fehlende Datei als leeres Buch
  und erkannte „fehlt“ an der Fehlerart `NotFound`. Windows meldet diese
  Art auch beim Lesen eines **Verzeichnisses** (hier das Temp-Verzeichnis
  des Läufers), das Buch galt dann als leer. Folge außerhalb des Tests:
  Läge unter dem Pfad des Eichbuchs ein Verzeichnis, rechnete der
  Dispatcher still mit Vorgaben statt einen Fehler zu melden.
- **Behebung:** Leer ist das Buch nur, wenn `NotFound` kommt **und** unter
  dem Pfad nichts liegt (`symlink_metadata` scheitert). Liegt dort etwas,
  ist es ein Fehler mit Pfad und Meldung, auf jedem System gleich.
- Belege: `cargo test` 47 von 47 auf macOS, Clippy mit `-D warnings` ohne
  Meldung. Die Bestätigung auf echtem Windows bringt der nächste CI-Lauf;
  Wine bildet diese Fehlerart nicht nach und taugt dafür nicht als Probe.

**Ältere Einträge** (v0.43.0 und davor, 44 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- COMPUTE_PIPELINE/README/README.md`.
