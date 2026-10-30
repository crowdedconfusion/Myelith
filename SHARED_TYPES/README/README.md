# shared-types (`myl-types`)

> **Version:** 0.62.0
> **Datum:** 2026-10-29
> **Status:** 🎉 **Phase 2 abgeschlossen** (Punkte 1.1–1.7, 2.1–2.3):
> Hash, Merkle-Baum, VRF (bit-exakt gegen RFC-9381-Vektoren), BLS12-381
> mit Aggregation **und Proof-of-Possession**, **Erasure-Codierung über
> GF(2⁸)**, ID-Newtypes, Kern-Structs
> aus Anhang A.1, Golden Vectors (18 Vektoren), Fuzz-Harness
> (100.000 Iterationen), Konformitätspaket.
> **298 Tests grün** über acht Testbinaries (gezählt am 2026-10-21; hier
> stand bis dahin 226 über sieben, der Stand vom 2026-09-03).

Protokollweite Kern-Datentypen, Hash-/Merkle-Primitiven und Serialisierung
für Myelith. Referenzimplementierung von Whitepaper Anhang A.1.

## Aufgabe

Ein einziges Crate, von dem alle anderen Komponenten (NETWORKING, CONSENSUS,
VERIFICATION, TOKENOMICS, COMPUTE_PIPELINE, AGENT_LAYER, TRAINING) dieselben
Basistypen beziehen, damit `Segment`, `PoIBundle`, Hashes und Signaturen
niemals in zwei Komponenten inkompatibel definiert werden.

## Abhängigkeiten

Keine — SHARED_TYPES ist die Basiskomponente des Protokolls.

## Struktur

```
SHARED_TYPES/
├── README/                   diese Kurzübersicht
└── myl-types/                das Protokoll-Crate (Bibliothek, kein Binary)
    └── src/
        ├── lib.rs             Crate-Wurzel: #![deny(unsafe_code)], Design-Doku
        ├── protocol.rs        Protokoll-Konstanten (Hash/VRF/Signatur/Serialisierung)
        ├── hash.rs            Hash-Newtype: SHA-256, Konstantzeit-Vergleich,
        │                      Borsh, Hex-Darstellung
        ├── merkle.rs          Merkle-Baum: Aufbau, Beweis-Erzeugung/-Prüfung,
        │                      Domain-Separation, Borsh-Beweise
        ├── vrf.rs             VRF: ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381),
        │                      Kanonizitätsprüfung, RFC-Testvektoren
        ├── bls.rs             BLS12-381 (min-pk, blst): KeyGen, Signatur,
        │                      Aggregation, FastAggregateVerify/AggregateVerify
        ├── ids.rs             ID-Newtypes: Address, MinerId, PodId, SegmentId,
        │                      MerkleRoot, ActivationHash, EpochId
        └── core_types.rs      Kern-Structs aus Anhang A.1: Segment, PoIBundle,
                               InferenceCredit (+ segments_root-Helfer)
```

## Changelog

### v0.62.0 (2026-10-30): Der Zustandsbaum auf der Platte

Punkt 1 der Festlegung vom 2026-10-29 (nach dem Vorbild von NOMT, geprüft
gegen QMDB; eigene Umsetzung).

- ⚑ **`Zustandsbaum::mit_ablage`**: Teilbäume unterhalb einer Tiefe, die mit
  der Zahl der Einträge wächst (rund 128 Einträge je Seite), liegen als
  Seiten in einer Datei (`Seitenablage`: Grössenklassen ab 256 Bytes, freie
  Plätze wiederverwendet); im Speicher steht der obere Baum, an jeder
  ausgelagerten Stelle `Knoten::Ausgelagert { seite, hash }`. Vor jedem
  Zugriff wird der Pfad eingelagert (beim Zeugen eines entfernten
  Schlüssels auch die Geschwister), nach jeder Wurzel werden genau die
  eingelagerten Seiten und berührten Pfade wieder ausgelagert (O(k ·
  Tiefe), nicht über den ganzen Baum); eine unveränderte Seite wird
  wiederverwendet. Auch während grosser Änderungsfolgen (erster Bau)
  zwischendurch, alle 65 536 Pfade.
- ⚑ **Gehasht wird wie zuvor**: gleiche Wurzel, gleiche Beweise, Byte für
  Byte gleiche Zeugen, gleich, was wo liegt. Die Datei ist ein
  Zwischenspeicher (der Zustand wird beim Start hergeleitet), braucht also
  keine Absturzsicherheit.
- ⛔️ Die Marken einer Seite (4 ausgelagert, 5 Zweig mit Hash) liest nur die
  eigene Ablage; in einem Zeugen werden sie abgewiesen, dort wird jeder
  Hash selbst gerechnet. Eine Kopie eines ausgelagerten Baums liegt ganz im
  Speicher (die Seiten gehören dem Original). `wurzel_gerechnet` liest die
  Wurzel ohne Kopie, `knoten_im_speicher` für Angaben.
- 📌 Erste Fassung ohne Hashes in den Seiten: Jede eingelagerte Seite wurde
  ganz neu gehasht, ein Block mit 2000 Überweisungen bei einer Million
  Konten brauchte 272 statt 34 ms; mit Hashes 76 ms.
- Gemessen (Release, eine Million Konten, mit zählendem Allokator):
  Baumknoten im Speicher 16 383 statt 2,88 Mio.; belegt 183 statt 380 MB
  (der Rest sind die Konten selbst); RSS fällt kaum (macOS behält
  freigegebenen Speicher). Block mit Zeugen 76 statt 34 ms, erster Bau
  10 statt 4 s.
- Tests `ausgelagert_rechnet_wie_im_speicher` (30 Runden gegen den Baum im
  Speicher: Wurzel, Werte, Beweise, Zeuge Byte für Byte; Kopie; nach dem
  Entfernen aller Einträge keine belegte Seite) und
  `eine_seitenmarke_ist_kein_zeuge`.

### v0.61.0 (2026-10-29): Ein Zeuge wird nur kanonisch gelesen

- ⛔️ **`Teilbaum::aus_bytes` liest nur Kanonisches**: kein Zweig über zwei
  Leeren oder über Leerem und einem Blatt, kein gekürzter Knoten mit dem
  Hash des Leeren. Ein solcher Knoten kostete im Draht zwei Bytes und im
  Speicher ein Vielfaches, bevor die Wurzel ihn verwerfen könnte.
- ⛔️ **`TEILBAUM_KNOTEN_HOECHSTENS`** (2²¹): Zweige mit leerem Geschwister
  bleiben kanonisch und kosten zwei Bytes je Ebene; die Schranke bindet den
  Speicher an die Zahl der Knoten. Ein ehrlicher Zeuge hat bei einer
  Milliarde Einträgen und 8000 berührten rund eine halbe Million.
- Test `unkanonisches_wird_nicht_gelesen`; 344 Tests grün, Clippy sauber.

### v0.60.0 (2026-10-29): Der Teilbaum als Zeuge

- ⚑ **`Zustandsbaum::teilbaum(schluessel, entfernt)`** (zustandslose
  Validierung, Festlegung des Projektinhabers vom 2026-10-29): ein
  **`Teilbaum`** über die Pfade einiger Schlüssel, alles daneben als Hash;
  gemeinsame obere Pfade stehen einmal darin. Er hat dieselbe Wurzel wie der
  volle Baum; `lesen` und `setzen` gehen auf seinen Pfaden, und die neue
  Wurzel ist die des vollen Baums nach denselben Änderungen. Ein Zugriff
  daneben ist `Ausserhalb` und ändert nichts. Kodierung `als_bytes` und
  `aus_bytes` (höchstens 256 tief, kein Rest).
- ⚑ **Gekürzt wird nur ein Zweig, nie ein Blatt**, und beim Entfernen darf
  kein gekürzter Knoten neben Leerem stehen bleiben: Der Hash eines
  einzelnen Blatts, als gekürzter Zweig ausgegeben, gäbe dieselbe
  Vorwurzel, fiele aber nicht zusammen, wo der volle Baum es tut. Prüfer und
  volle Knoten kämen zu verschiedenen Wurzeln. Deshalb öffnet der Zeuge
  neben dem Pfad eines entfernten Schlüssels den Zweig eine Ebene weit; ein
  geöffneter Zweig ist über die getrennten Hash-Domänen als Zweig belegt
  und hat im kanonischen Baum mindestens zwei Einträge.
- 📌 **Erst entscheiden, dann ändern**: Die erste Fassung von
  `Teilbaum::setzen` sicherte den ganzen Teilbaum vor jeder Änderung; bei
  4000 Änderungen 8 s je Block. Jetzt entscheidet ein Trockenlauf über den
  Pfad in O(Tiefe), danach wird ohne Kopie geändert: 29 ms.
- Tests `der_zeuge_traegt_wie_der_volle_baum`,
  `ein_als_zweig_getarntes_blatt_faellt_auf`,
  `der_zeuge_haelt_auch_gemischten_runden_stand` (40 Runden aus Setzen,
  Neuanlegen und Entfernen); 343 Tests grün, Clippy sauber.

### v0.59.0 (2026-10-29): Der Zustandsbaum

- ⚑ **`zustandsbaum::Zustandsbaum`** (Skalierung, Festlegung des
  Projektinhabers vom 2026-10-29; Vorbild Jellyfish Merkle Tree und
  EIP-7864, eigene Umsetzung): ein binärer, dünn besetzter Merkle-Baum über
  256-Bit-Schlüssel. Ein Teilbaum mit genau einem Eintrag ist dieser
  Eintrag; die Wurzel hängt nur an der Menge, nicht an der Folge der
  Änderungen. Zweige merken ihren Hash und rechnen erst bei
  `wurzel()` nach, was ein Block mehrfach berührt, wird einmal gehasht.
  `Baumbeweis` für Vorhandensein und Fehlen; `wurzel_von` als Gegenprobe
  ohne Baum.
- Gemessen (Release): eine Million Einträge, 2000 Änderungen samt Wurzel
  11 ms; Beweistiefe 24.
- Tests `leer_und_ein_eintrag`, `die_wurzel_haengt_nur_an_der_menge`,
  `beweise_fuer_vorhandene_und_fehlende`; 340 Tests grün.

### v0.58.0 (2026-10-29): Transaktionen gebündelt über die Relaisleitung

- **`Relaisfrage::Transaktionen`** (angehängt), höchstens
  `TRANSAKTIONEN_JE_FRAGE` (512): viele Transaktionen in einer Frage; der
  Knoten prüft sie als Stapel. Wie ein Gateway viele Nutzer bündelt.

### v0.57.0 (2026-10-29): Der Differenzbeweis zweier Zwillinge

- ⚑ **`Differenzbeweis`** (Whitepaper 6.4 und 6.6, Festlegung des
  Projektinhabers vom 2026-10-29): zwei `Differenzseite`n zu derselben
  Kennung, je mit dem Ausgabenzeugnis und seinem Beweis gegen die
  Ausgabenwurzel und mit dem Segmentzeugnis und seinem Beweis gegen
  `segments_root` (das legt die Stelle im Bündel fest).
  `differenzseite` baut eine Seite aus den Blättern eines Bündels,
  `differenz_pruefen` prüft beide gegen die Wurzeln und liefert die Stelle
  in jedem Bündel. Gleiche Blätter sind keine Differenz. Der Beweis
  verortet die Abweichung, wie Verde und Arbitrum BoLD einen Streit
  eingrenzen; wer ihn meldet, muss niemand glauben.
- `Relaisfrage::DifferenzMelden` (angehängt).
- Test `ein_differenzbeweis_verortet_die_abweichung`; 337 Tests grün.

### v0.56.0 (2026-10-29): Die Tokens in der Ablage, der Verlauf für das Nachrechnen

- ⚑ **`Spurablage::eingaben`** (angehängt, parallel zu `eintraege`): je
  Segment die Nutzlast seines Eingangs, das Token als zwei `i16`, vier
  Bytes. Aktivierungen bleiben draussen. Leer heisst: eine Ablage ohne
  Tokens (ältere Fassung); sie beantwortet Spuranfragen wie bisher, aber
  keine Verläufe.
- **`Spurablage::verlauf`**: zu einem gezogenen Segment alle Segmente
  derselben Sitzung bis zu ihm, nach Position geordnet, jedes mit Beweis
  gegen dieselbe Spurwurzel und mit seinem Token.
- **`verlauf_pruefen`**: jede Lieferung trägt, eine Sitzung, Positionen
  lückenlos ab null bis zum gezogenen Segment, und der Hash jedes Tokens ist
  der erste Eintrag seiner Spur. Ergebnis sind die Tokens; ein Pod kann so
  keine anderen unterschieben als die, über die er gerechnet hat. Fehler
  als `Verlauffehler`.
- Dazu `sitzung_und_position`, `token_aus_eingabe`, `eingabe_aus_bytes`.
- Test `der_verlauf_liefert_die_tokens_bis_zum_gezogenen_segment`; 318 Tests
  grün.

### v0.55.0 (2026-10-28): Die Ausgabenwurzel für Stufe 1

- ⚑ **`PoIBundle.ausgabenwurzel`** (angehängt; Whitepaper 3.4, Festlegung
  des Projektinhabers vom 2026-10-28): Merkle-Wurzel über Kennung, Eingang
  und letzte Ausgabe je Segment (`Ausgabenzeugnis`, `ausgabenwurzel`),
  **nach Kennung geordnet**. Zwei Zwillingspods, die dieselben Segmente
  gleich gerechnet haben, tragen dieselbe, gleich in welcher Folge sie
  abschlossen; daran vergütet die Kette (NODE 0.77.0). `segments_root`
  taugt dafür nicht, weil sie der Folge des Abschlusses folgt. Null heisst:
  keine. `poi_botschaft::bundle_message` hängt sie an, sobald sie nicht
  null ist; die Felder davor haben feste Länge, die Botschaft ist also
  eindeutig, und wer die Wurzel nach dem Unterschreiben ändert oder
  streicht, macht das Aggregat ungültig.
- **`relais::Podeintrag.zwilling`** (angehängt): der Pod, der dieselben
  Segmente rechnet, aus denselben Paaren, nach denen die Kette vergütet.
- Proben `die_ausgabenwurzel_ist_von_der_reihenfolge_frei` (Gegenproben:
  anderer Ein- oder Ausgang, andere Kennung, ein Segment weniger) und
  `die_botschaft_traegt_die_ausgabenwurzel`. ⚠️ Das Format des Bündels und
  des Podbilds ändert sich; der Fuzz-Korpus `poi_buendel` läuft weiter
  durch, seine alten Einträge sind keine gültigen Bündel mehr.
- ⚑ **Übernahme durch die Reserve** (Whitepaper 6.8, Festlegung des
  Projektinhabers vom 2026-10-28): `PoIBundle.vertreter` (angehängt), je
  Position das Reservemitglied, das sie übernahm; `bundle_message` hängt die
  Liste hinter einer eigenen Marke an, sobald sie nicht leer ist (die
  Folge aus Wurzel und Vertretern bleibt so eindeutig lesbar).
  `relais::Podeintrag.reserve` (angehängt) nennt dem Ortsdienst die
  Reserve; `relais::Relaisfrage::Uebernahme` (angehängt) bittet über die
  Relaisleitung ein Reservemitglied, eine Position zu übernehmen.
- ⚑ **Die Spur liegt bei allen Mitgliedern** (Stufe 2, Whitepaper 3.5.2
  und 6.4): `Spurablage` (Epoche, Pod, `segments_root`, je Segment Zeugnis
  und Spur in Bündelfolge) mit `antwort`, die auf eine `Spuranfrage` Zeugnis,
  Merkle-Beweis gegen `segments_root` und Spur liefert (`None`, wenn die
  Anfrage nicht passt oder die Ablage ihre Wurzel nicht ergibt).
  `lieferung_pruefen` mit `Lieferfehler` ist die eine Prüfung einer
  Lieferung: Index, Blattzahl gleich der Segmentzahl des Bündels, Beweis
  gegen die unterschriebene Wurzel, Spur gleich der bezeugten Spurwurzel.
  `relais::Relaisfrage::SpurenAblegen` (angehängt) reicht eine Ablage an
  den Knoten eines Mitglieds. Probe `eine_abgelegte_spur_traegt_gegen_die_wurzel` mit
  Gegenproben (falscher Index, falsche Blattzahl, fremde Wurzel, andere
  Spur, fremder Pod, fehlender Index, Ablage mit falscher Wurzel).
- `myl-types` 0.55.0. 335 Tests grün, Clippy sauber.

### v0.54.0 (2026-10-28): Die Zeitprobe mit Saat

- ⚑ `zeitprobe::anfrage_mit_saat`: Die Anfrage trägt eine Saat des
  Messenden (32 Byte); die Tür liest sie mit `als_anfrage_voll` (4, 8 oder
  40 Bytes nach der Marke). Anlass Fund 669: Ohne frische Eingabe war der
  Abdruck für dieselbe Anfrage immer derselbe und liess sich vorrechnen.
- `zeitprobe::fragen_mit_saat` und `zeitprobe::antwortnachricht`: die
  Nachricht, über die das gemessene Mitglied seine Antwort unterschreibt
  (Saat, Positionen, Abdruck, mit eigener Domäne).
- `myl-types` 0.54.0. 332 Tests grün, Clippy sauber.

### v0.53.0 (2026-10-28): Stapelprüfung der Unterschriften

- ⚑ `bls::verify_batch`: viele Einzelsignaturen über je eigene Nachrichten
  in einem Aufruf (`blst` `verify_multiple_aggregate_signatures`), mit
  Zufallsskalaren von 64 Bit aus `RandomState` der Standardbibliothek, die
  das Betriebssystem schlüsselt (keine neue Kiste, kein `unsafe`). `true`
  heisst: jede trägt, bis auf 2⁻⁶⁴; `false`: mindestens eine nicht, oder
  etwas ist nicht dekodierbar.
- Gemessen (M5 Pro): rund 90 µs je Signatur samt Dekodieren gegen rund
  470 µs mit `verify` je Signatur.
- Probe `die_stapelpruefung_faellt_mit_einer_falschen_signatur`
  (vertauschte Signaturen, fremde Nachricht, leer, ungleich lang).
- `examples/pruefrate.rs` (nicht ausgeliefert): misst Einzel- gegen
  Stapelprüfung auf der eigenen Maschine (`cargo run --release --example
  pruefrate -- 1000`); hier 452 gegen 87 µs je Unterschrift auf 15 Fäden.
  Für das Ziel des Grundpreises zählt die Prüfrate der kleinsten
  zugelassenen Maschine.
- 332 Tests grün, Clippy sauber.

### v0.52.0 (2026-10-27): Gebühren in der Kontoauskunft

- `relais::Kontostand` trägt `grundpreis_je_byte`, `bekannt` und
  `aufschlag_konto`, damit eine Wallet die Gebühr vor dem Unterschreiben
  zeigen kann. Ändert die Kodierung der Antwort.
- 331 Tests grün.

### v0.51.0 (2026-10-27): Die Zeitprobe mit Grenze

- `zeitprobe::anfrage_mit_grenze`, `als_anfrage_mit_grenze`,
  `fragen_mit_grenze`: dazu vier Bytes, nach wie vielen Millisekunden der
  Shard aufhört (Fund 646, NODE 0.68.0). Die alte Form mit vier Bytes bleibt
  lesbar; ein Anhängsel macht beide ungültig.
- `myl-types` 0.51.0. 331 Tests grün, Clippy sauber.

### v0.50.1 (2026-10-26): Fuzz-Ziele für die lokalen Leitungen und die Vollmacht; Mutationen auf stable

- Fünf neue Ziele in `tests/fuzzziele/mod.rs` (und je ein libfuzzer-Ziel
  unter `fuzz/`): `ortsfrage`, `ortsantwort`, `relaisfrage`,
  `relaisantwort`, `vollmacht`. Alles, was über eine Leitung oder durch die
  Tür kommt, steht damit unter dem Prädikat der Kanonizität (Angriffsklassen
  A16 und A20 in `SIMULATION/Sicherheitsaudit.md`). Startkorpus aus
  gültigen Kodierungen der echten Typen, 20 Saaten, alle kanonisch; 18
  Ziele.
- ⚑ **`mutationen_der_saat_bleiben_kanonisch`** (`tests/fuzzkorpus.rs`):
  Jede Korpuseingabe wird mit einem reproduzierbaren Würfel tausendfach
  verändert (Bit kippen, Byte setzen, kürzen, verlängern), und jede
  Mutation muss unlesbar oder kanonisch sein. Der Fuzzer braucht Nightly
  und `libfuzzer-sys`, das nicht im Offline-Lager liegt; diese Probe läuft
  in jedem `cargo test`. Gemessen: 163 000 Mutationen, 9 514 lesbar, alle
  kanonisch, kein Fund. Das Prädikat selbst hat seine Gegenprobe
  (`das_praedikat_findet_eine_nicht_kanonische_kodierung`).
- `myl-types` 0.50.1.

### v0.50.0 (2026-10-26): Die Relaisleitung zwischen Knoten und Koordinator

- `relais.rs` (neu): `Relaisfrage` (Podbild, Shard), `Relaisantwort`,
  `Podbild` mit Pods und Mitgliedern in Shardreihenfolge (Netzadresse,
  Ebenenbereich), `fragen` (blockierend, Rahmen mit Ausweis wie die
  Ortsleitung), `ausweis_lesen` (64 Hexzeichen). Größengrenze wie zwischen
  zwei Knoten. Proben: Hin und zurück, Anhängsel abgewiesen, Ausweis mit
  falscher Länge abgewiesen.
- Dazu `Relaisfrage::BuendelEinreichen` (ein unterschriebenes PoI-Bündel
  für den Koordinator des Pods) und `Relaisantwort::Erledigt`, angehängt.
- Dazu `Relaisfrage::Konto` (Antwort `Kontostand`: Höhe, Epoche,
  Guthaben, Nonce, Credits, Netzwert, Konto des Knotens) und
  `Relaisfrage::Transaktion` für ein Wallet, das selbst unterschreibt.
- `myl-types` 0.50.0.

### v0.49.0 (2026-10-24): Die Empfängerliste eines Sitzungskontrakts steht nur noch als Wurzel auf der Kette

⛔️ **Konsensänderung des Formats:** `Sitzungskontrakt` und `Vorhaben`
kodieren anders, damit jede Kontraktadresse und jede Ausgabe. Die
Probenetze werden gemeinsam gehoben; ein Produktivnetz bräuchte eine
Aktivierungshöhe, und es gibt noch keinen produktiven Kontrakt.

- ⚑ **`Sitzungskontrakt::empfaenger` wird `empfaenger_wurzel`**: die
  Merkle-Wurzel über die sortierte Liste (`empfaengerwurzel`), null für die
  leere. Bis hierher stand die Liste selbst im Kontrakt und damit im
  Kettenzustand: lesbar für jeden, und dazu abtastbar, weil eine abgelehnte
  Zahlung nichts kostet. Wer die Empfänger kannte, wusste, mit wem der
  Inhaber Geschäfte macht. Der Konstruktor nimmt weiter die sortierte Liste
  und prüft sie wie bisher.
- **`Vorhaben::empfaenger_beweis`**: der Merkle-Beweis für genau diesen
  Empfänger (`empfaengerbeweis` baut ihn aus der Liste, die der Inhaber
  hält; `kein_beweis` belegt nichts). `zahlt_an` prüft ihn gegen die Wurzel;
  eine Ablehnung sagt nur noch „für diese Adresse gibt es keinen Beweis“.
- **`pruefe_zugang`**: wie `pruefe`, ohne den Empfänger, für eine Tür, die
  nur nach dem Zugang fragt und nichts bucht.
- **`Vollmacht::empfaengerbeweise`**: Der Inhaber hängt die Beweise für die
  Knoten an, die bei ihm abrechnen sollen (`mit_empfaengerbeweis`,
  `empfaengerbeweis`); ausserhalb der Unterschriftenkette, denn ein Beweis
  prüft sich selbst gegen die Wurzel. Höchstens 16 je Vollmacht, eine
  längere liest `aus_bearer` nicht.
- **Belege:** neu `die_empfaenger_stehen_nicht_im_kontrakt` (keine
  Empfängeradresse in den Bytes des Kontrakts),
  `ein_beweis_aus_einer_anderen_liste_belegt_nichts`,
  `die_zugangspruefung_fragt_nicht_nach_dem_empfaenger`,
  `empfaengerbeweise_reisen_mit_und_sind_gedeckelt`; die Positivliste prüft
  jetzt Beweise. Gegenproben: ohne Beweisprüfung fünf Tests rot, ein Zugang,
  der den Empfänger prüft, rot, ohne Deckel rot, die Liste gehasht statt als
  Merkle-Wurzel rot. 311 Tests grün.

### v0.48.0 (2026-10-24): Historienwurzel, Ziehung der Archivknoten, Reed-Solomon-Teile der Geschichte

Für die Archivknoten (Festlegung des Projektinhabers vom 2026-10-23:
„absolut bulletproof gegen Angriffe und Totalausfall“). Drei neue Module,
alle im Konsenspfad und deshalb in der Gleitkommaprüfung gelistet.

- ⚑ **`historie`** (Stufe A1): ein Merkle Mountain Range über die
  Blockhashes, mit getrennten Domänen für Blatt, Knoten, Gipfel und Wurzel;
  die Wurzel bindet die Blattzahl. `Historie` (Blattzahl und Gipfel) steht im
  Zustand, `Beweis` prüft eine Stelle gegen eine Wurzel. `Historienbaum`
  hält die Knoten ab Stufe zehn (rund 64 Byte je 1024 Blöcke) und rechnet
  darunter aus den Blättern einer Gruppe. Neu am 2026-10-24:
  **`beweis_bei` gegen eine frühere Länge**; wer nachlädt, prüft gegen die
  Historie seiner Basis, die kürzer ist als die des Liefernden. Ohne
  gespeicherte alte Gipfel: Jeder Gipfel und jedes Geschwister einer
  früheren Länge ist ein vollständiger Teilbaum. Probe: jede Länge bis 150,
  jede Stelle, gleich der Referenz; eine verfälschte Gruppe ergibt keinen
  Beweis.
- ⚑ **`archivwahl`** (Stufe A3): sieben Kopien über die Zonen verteilt (je
  Zone nach Los, reihum), 64 Splitter getrennt gelost und ohne die Kopien,
  teilnehmen nur nach einer Wartezeit; unter sieben Berechtigten behält
  jeder alles. `nachruecken` füllt zuerst eine fehlende Zone. 📌 Nicht „jeder
  mit Wahrscheinlichkeit k/N“: Bei einem Erwartungswert von fünf bliebe etwa
  jede 150. Periode leer. Neue Rolle `Halter`.
- ⚑ **`archivteile`** (Stufe A7): je Gruppe von 1024 Höhen 32 Daten- und 32
  Paritätsteile (Cauchy über GF(2⁸), `erasure`), Gruppenwurzel über die 64
  Teile, Teilwurzel über die Gruppen eines Abschnitts, `Teilbeweis` mit zwei
  Pfaden. Probe: aus beliebigen 32 von 64 kommt die Gruppe zurück (204
  Auswahlen), aus 31 nicht. 📌 **Die Gegenprobe zeigte eine Lücke:** Die
  Prüfung, dass hinter dem letzten Block nur Auffüllung steht, hatte keinen
  Fall; jetzt einer mit verfälschter Auffüllung.
- **Proben:** 324 grün (vorher 298), Gegenproben je Prüfung rot. Dazu
  zwei Stellen, die Clippy mit `-D warnings` meldet (`chunks_exact` in
  `historie`, ein überflüssiges `clone` in einer Probe), behoben.

### v0.47.0 (2026-10-23): Geheimdateien mit Rechten ab dem ersten Byte; ein Ausweis für die Shardtür (Funde 607, 611)

Aus dem Sicherheitsaudit vom 2026-10-23.

- ⚑ **Neu: `geheimdatei::schreiben`** (Fund 607). Knotenidentität
  (`myl-net`) und Validatorschlüssel (`myl-node`) wurden mit `fs::write`
  angelegt und **danach** auf `0600` gesetzt; dazwischen lag der Schlüssel
  mit den Vorgaberechten (meist `0644`) da, lesbar für jeden auf dem
  Rechner. `ortsleitung::schluessel_ablegen` machte es richtig und sagte
  warum, die beiden anderen hatten es nicht gelesen. Jetzt eine Stelle für
  alle drei: Zwischendatei frisch (`create_new`) und mit `0600` **angelegt**,
  auf die Platte gebracht, dann umbenannt; eine liegengebliebene
  Zwischendatei mit alten Rechten wird nicht weiterbenutzt.
  **Gegenprobe:** Mit dem alten Muster meldet der Test „die Zwischendatei
  lag mit 644 offen“.
- ⚑ **Die Shardtür verlangt einen Ausweis** (Fund 611):
  `zeitprobe::rahmen_senden_mit_ausweis` und
  `rahmen_empfangen_mit_ausweis`, 32 Bytes vor der Länge, in
  gleichbleibender Zeit verglichen; `zeitprobe::fragen` nimmt den Ausweis
  mit. Wer ihn nicht hat, bringt die Tür nicht dazu, die Länge zu lesen.
  **Gegenprobe:** ein falscher Ausweis vor einer angekündigten Länge von
  vier Gigabyte wird als „falscher Ausweis“ abgewiesen.
- Belege: Clippy (`-D warnings`) und alle Tests grün (290 in der
  Bibliothek).

### v0.46.1 (2026-10-22): Eine Werkzeugkette für alle Kisten

- `rust-version` auf 1.99 in `myl-types`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  ⚑ **Dazu Code, und zwar mit Absicht:** Mit `rust-version = "1.99"` gibt Clippy
  zwei Regeln frei, die es bei 1.85 zurückhielt (`manual_is_multiple_of`,
  `chunks_exact` mit fester Größe). Festlegung des Projektinhabers:
  umschreiben, nicht abschalten. Hier: `merkle.rs` 2 Zeilen.
  `x % n == 0` wird `x.is_multiple_of(n)`, `chunks_exact(N)` wird
  `as_chunks::<N>()`. Für `n ≠ 0` bitgleich; jede Stelle mit variablem
  Teiler ist einzeln angesehen und vorher gegen null geschützt (bei
  `is_multiple_of(0)` stünde `x == 0`, wo `x % 0` abbricht).
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen. Nach dem
  Umschreiben: Clippy mit `-D warnings` still (die Kerne über alle vier
  Merkmale), `cargo test` rc=0 in allen zehn berührten Kisten (die Kerne auch
  mit `cpu-simd`), Konformität mit `sparrow-0.6b-int8` 55/55, die vier
  Abdrücke unverändert (`cb696efd3817e575` über 24 Vektoren), Gleitkomma-Audit
  grün.

### v0.46.0 (2026-10-22): Podmodell, Fristmeldung, Zeitprobe

Stufe 4 des Dispatchers, abgeschlossen mit CONSENSUS v0.50.0 und NODE
v0.52.0.

- `podmodell::Podmodell` (neu): Abdruck, Ebenenzahl und Kopf in Platz und
  Zeit, eines je Netz; es steht im Kettenzustand.
- `Kapazitaetszusage::pruefen_im_netz`: nur das Podmodell, höchstens alle
  Ebenen samt Kopf, ein gehaltener Bereich innerhalb der Ebenen. Neue
  Fehler `ModellNichtImNetz`, `ZuVieleEbenen`, `GehaltenAusserhalb`.
- `fristmeldung::{Fristmeldung, Fristzeugnis}` (neu): Jedes Zeugnis ist die
  eigene, unterschriebene Messung eines Pod-Mitglieds (Rolle
  `PodMitglied`, Trennstring `MYELITH_FRISTZEUGNIS_v1`).
- `gegenzeichnung::mindestzeichner` (neu, aus `myl-pod` hierher): Pod und
  Kette rechnen dieselbe Mehrheit.
- `zeitprobe` (neu): Marke, Rahmen und Klient der Zeitprobe an der Tür
  eines Shards, damit der Knoten sie stellen kann, ohne `myl-pod`
  einzubinden; der Rahmen der Shardtür kommt seither von hier.
- **Belege:** neue Proben zur Zusage gegen das Podmodell, zum Zeugnis, zur
  Mehrheit und zur Zeitprobe über eine echte Verbindung; `cargo test`
  rc=0.

### v0.45.0 (2026-10-22): Die Zusage nennt ihre Modelle

Stufe 4 des Dispatchers, Paket P4 (Festlegungen E1 bis E3 und E6 des
Projektinhabers vom 2026-10-21). Bisher sagte eine Kapazitätszusage nur,
wie viele Bytes ein Knoten hält. Für die Verteilung der Ebenen in einem Pod
fehlte, wie viele Ebenen eines Modells er tragen kann und wie schnell er sie
rechnet.

- **`Modellzusage`** (neu): `abdruck` (der Abdruck des Artefakts auf der
  Netzliste, nicht der Name), `ebenen_hoechstens`, `ebenenzeit_us` und
  optional `gehalten: (von, bis)` für einen Knoten, der nur einen Teil des
  Artefakts hält (Teilablage, Weg a; leer heißt das ganze Artefakt).
- **`Kapazitaetszusage::modelle`**, höchstens `MODELLE_HOECHSTENS` = 8
  Einträge, mit `mit_modell()` und `modell(&abdruck)`. Die Unterschrift deckt
  die Liste; die Trennkennung ist jetzt `MYELITH_KAPAZITAETSZUSAGE_v2`, eine
  Unterschrift der alten Form gilt nicht mehr.
- ⚑ **Die Zeit ist eine Frist** (`FRIST_FAKTOR` = 2, `frist_us(ebenen)`):
  Wer sich schneller ansagt, als er rechnet, bekommt mehr Ebenen und bremst
  seinen Pod; seine Gutschrift folgt der Arbeit am Gewicht und nicht der
  angesagten Zeit. Die Durchsetzung der Frist steht noch aus.
- **Prüfung** in `pruefen`: neue Fehler `ZuVieleModelle`, `ModellDoppelt`
  (ein Abdruck zweimal) und `ModellUngueltig` (null Ebenen, null
  Mikrosekunden oder ein leerer gehaltener Bereich).
- **Belege:** neue Probe `die_modelle_sind_unterschrieben_und_geprueft`
  (eine geänderte Liste bricht die Unterschrift, jeder Fehlerfall einzeln);
  `cargo test` rc=0 (299 Tests über acht Binaries).

### v0.44.0 – 2026-10-21 (`LatencyGraph` entfernt)

⚑ **`LatencyGraph` ist entfernt**, samt seinen drei Tests und dem
Wiederexport aus der Wurzel. Die Pod-Bildung liest seit der Entscheidung
für Zonen (2026-09-01) keine gemessene Latenz mehr: Wer wählt, mit wem er
attestiert und welche Zahl er nennt, formte sonst mit, in welchem Topf er
gemischt wird. Gebaut hat den Graphen seitdem niemand.

Zwei Gründe, ihn nicht als Vorrat liegen zu lassen: `insert_attest` setzte
die 32 Bytes einer Netzadresse ohne Umrechnung als Miner-Kennung ein („in
der realen Implementierung müsste hier eine Mapping-Tabelle verwendet
werden“), und `cleanup_stale_entries` las die Systemuhr. Beides wäre der
erste Fehler gewesen, sobald ihn jemand benutzt.

**Was bleibt:** `LatencyAttest` mit Signatur und Prüfung, `PeerIdBytes`
und `BlsSignatureBytes`. Die Atteste sind eine Messung des Netzes; das
Gossip von NETWORKING und der Validatorsatz von NODE prüfen sie. Der
Graph war nie serialisiert, also ändert sich kein Leitungsformat. Der
Modulkopf sagt jetzt, dass die Atteste die Pod-Bildung nicht bestimmen.

**Belege:** `cargo test` rc=0 (298 Tests über acht Binaries), Clippy mit
`-D warnings` still; `cargo check --all-targets` in allen siebzehn Kisten,
die `myl-types` benutzen, rc=0; neunzehn Sperrdateien nachgezogen,
`sperrprobe` grün.

### v0.43.0 – 2026-09-06 (die Lernrate wird eine Protokollgrösse)

`lernrate::NORMIERTER_NENNER` und `schuldbeleg` sind neu.

⚑ **Die Rate bedeutet zum ersten Mal auf jedem Modell dasselbe.** Mit
`optimierer::schritt_normiert` bewegt sich das grösste Gewicht einer
Matrix um einen Bruchteil **ihres eigenen Betragsmaximums**; gemessen
lernen alle vier Modelle bei **derselben** Zahl 256, das Expertengemisch
am stärksten (Haltemenge −23,8 Prozent). Zuvor bewegte Qwen3-4B bei
jeder zulässigen Rate kein einziges Gewicht (Fund 194).

**`schuldbeleg` liegt hier, weil es Wiretypen sind.** Der Konsens muss
einen Beleg auf den Draht legen können, und `myl-consensus` darf nicht
an VERIFICATION hängen. VERIFICATION exportiert sie weiter; zwei
Fassungen desselben Typs wären das Schlimmere gewesen.

### v0.42.0 – 2026-09-06 (das Segment sagt jetzt, wie gross es ist)

`lernrate` ist neu, und `Trainingssegment` trägt zwei Felder mehr:
`bewegte_gewichte` und `folgen`. Die Botschaft steht damit auf `v3`.

⚑ **Beide aus demselben Grund: Das Protokoll prüfte die Absicht eines
Segments und nicht seine Grösse.** Eine Schrittzahl über null und eine
Lernrate über null sagen, dass jemand trainieren **wollte**. Sie sagen
nicht, wie weit sich etwas bewegt hat (`bewegte_gewichte`, Fund 191) und
über wie viele Gradienten (`folgen`).

**Ohne `folgen` ist die Lernrate keine Angabe über die Schrittweite.**
Mit Gradientensammlung summiert ein Schritt die Bewegung aller Folgen
und rundet einmal; die Bewegung wächst linear mit ihrer Zahl. Gemessen:
Dieselbe Ebene, dieselbe Rate 2⁻¹⁶, acht statt zwei Folgen, und die
Perplexität stieg von 41,6 auf 24 084. Ein Pod hätte tausend Folgen mit
einer Rate für sechzehn fahren können und jede Prüfung bestanden.

**`lernrate` bindet deshalb das Budget** `schritte · folgen / nenner`
an die Tiefe, nicht die Rate für sich. Die Kurve ist an vier Messpunkten
kalibriert und liegt bewusst **unter** ihnen; ihre Tabelle steht im
Modulkopf, samt der Zeile, die zeigt, dass eine zu grosse Rate zuerst
überanpasst und erst dann zerstört.

### v0.41.0 – 2026-09-06 (die Tokenzahl gehört zum Korpus)

`Korpusanker::tokens_je_segment` und `::tokens_je_buendel`.

⚑ **Die Verguetung haengt daran.** Die Arbeit eines Trainingssegments
ist Schrittzahl mal Tokenzahl mal Modellgroesse. Dürfte der Pod die
Tokenzahl nennen, nennte er seine eigene Vergütung; stünde sie nirgends,
könnte die Kette gar nicht vergüten.

### v0.40.0 – 2026-09-05 (Spurcommitment und Modellfortschreibung)

`Trainingssegment::commitment_aus_spur` bildet aus den Δ-Abdrücken der
Shards **einen** Wert.

⚑ **Eine Funktion und nicht zwei Rechnungen.** Der Pod schreibt den Wert
ins Segment, der Prüfer rechnet die Shards nach und muss zum selben
kommen. Rechneten beide auf eigene Faust, wäre eine Abweichung nicht von
einem Rechenfehler im Pod zu unterscheiden: dieselbe Lehre wie bei
Fund 34.

Reihenfolge **und** Zahl der Shards gehen ein: Ohne die Zahl liesse sich
ein Segment aus zwei Shards nicht von einem aus vier unterscheiden,
dessen letzte zwei Abdrücke leer sind.

### `modellversion`: das Rezept, nicht das Ergebnis

`naechste_version(alt, deltas)` bildet die neue Modellfassung aus der
alten und den bestätigten Δ-Commitments. **Die Kette hält keine
Gewichte, also kann sie nicht summieren**; sie hält das Rezept, und wer
die Gewichte hat, wendet es an und rechnet die Wurzel nach.

⚑ **Die Reihenfolge ist nur für die Kennung, nicht für die Arithmetik.**
`optimierer::aggregiere` summiert ordnungsfrei; wäre die Reihenfolge für
die Summe nötig, kämen zwei Knoten mit verschieden sortierten Eingaben
zu verschiedenen Gewichten.

⚑ **Eine leere Liste lässt die Fassung stehen.** Eine Epoche ohne
bestätigtes Training hat das Modell nicht verändert; eine neue Kennung
dafür wäre eine Änderung, die es nicht gab.

**Ältere Einträge** (v0.39.0 und davor, 51 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- SHARED_TYPES/README/README.md`.
