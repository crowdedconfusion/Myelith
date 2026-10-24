# consensus (`myl-consensus` + `myl-ledger` + `myl-scheduler`)

> **Version:** 0.52.0 (`myl-consensus` 0.34.0, `myl-scheduler` 0.18.1,
> `myl-ledger` 0.26.0)
> **Datum:** 2026-10-24
> **Status:** Design-Entscheidungen getroffen (malachite hinter
> trait-Grenze mit Eigenbau-Fallback, Blockzeit 2 s, Komitee 21/7,
> Streitfrist 7 Tage, Reed-Solomon k=8/m=4);
> Phase 2 ✅ abgeschlossen (`myl-ledger` v0.1.1–v0.1.5,
> `myl-scheduler` v0.2.1–v0.2.9); **Phase 3 ✅ vollständig**
> (`myl-consensus` v0.3.1–v0.5.0): signiertes, stimmgewichtetes BFT mit
> VRF-rotierender Komiteewahl, Double-Signing-Beweis und seit v0.5.0
> Rundenwechsel mit Sperrmechanik — Safety **und** Liveness, die
> Akzeptanz-Testmatrix über 21 simulierte Validatoren läuft;
> **Phase 4 ✅ vollständig** (4.1 PoI-Bündel-Einreichung,
> 4.2 Epochenabschluss, 4.3 DA-Schicht).
> ⚑ **Phase 1 hatte am 2026-08-28 zwei Lücken und hat jetzt keine
> mehr:** Session-Kontrakte stehen im Ledger, **Anweisungen sind
> unterschrieben** (Fund 85), und es gibt eine Überweisung von Konto zu
> Konto.
> **512 Tests grün** (272 `myl-consensus`, 131 `myl-scheduler`,
> 109 `myl-ledger`), über alle Testbinaries gezählt.
>
> ⚑ **Seit dem 27. August trägt der Blockkopf eine Höhe.** Er hieß bis
> dahin `EpochMeta`, führte kein Höhenfeld, und die Probekette schrieb
> ihre Höhe deshalb in `epoch` — eine Doppelbelegung, an der jede Frist
> „je Epoche" in Wahrheit „je Block" bedeutete.
>
> **Seit dem 27. August führt der Ledger eine Verstoßhistorie je Konto**
> (`myl-ledger` v0.3.0). Sie ist Konsensfeld und die Voraussetzung der
> Slashing-Staffelung aus Kap. 5.5: Wer wiederholt auffällt, verliert
> mehr.
>
> **Seit dem 26. August laufen die BFT-Runden über ein echtes Netz.**
> Die Verdrahtung liegt in der Komponente NODE; hier kam die Form auf
> der Leitung dazu (`Konsensnachricht`) und die Bindung der
> Zertifikatsrunde an die Signatur des Vorschlags.

BFT-Blockproduktion, Proof-of-Inference-Aggregation, Staking/Slashing,
Ledger-Zustandsübergänge, deterministischer Epochen-Scheduler.
Referenzimplementierung von Whitepaper Kap. 3.5 und Anhang A.2/A.5.

## Aufgabe

Schicht L1: der Konsens, der unabhängig von der Inferenz-Latenz läuft
(Kap. 3.2). Zwei entkoppelte Prozesse (Kap. 3.5.2): schnelle
BFT-Blockproduktion (Prozess A) und epochenweise PoI-Abrechnung (Prozess B).
Dazu der deterministische Epochen-Scheduler (Anhang A.2), der ohne zentrale
Instanz aus dem Blockhash Miner-Zuteilung, Pod-Bildung und
Stichprobenlotterie ableitet.

## Abhängigkeiten

SHARED_TYPES, NETWORKING (Block-Gossip). Enthält selbst keine
Inferenz-Verifikation (siehe VERIFICATION) und keine Tokenomik-Berechnung im
Detail (siehe TOKENOMICS) — bildet aber deren gemeinsame Grundlage
(Ledger-Zustandsübergänge, Staking/Slashing-Buchhaltung).

## Struktur

```
CONSENSUS/
├── README/                   diese Kurzübersicht
└── myl-ledger/               L1-Ledger (Anhang A.5)
    ├── src/
    │   ├── lib.rs             Konsens-Grundregeln (reine Funktionen,
    │   │                      BTreeMap-Ordnung, Ganzzahligkeit,
    │   │                      Überlaufsicherheit)
    │   ├── state.rs           Kontenmodell (Balance/Stake/Credits),
    │   │                      State-Commitment (SHA-256 über Borsh)
    │   └── transitions.rs     burn→mint_credits, apply_verdict,
    │                          credit_spend (atomare Übergänge)
    └── tests/
        └── determinism.rs     Replay-Akzeptanztest: gleiche Folge ⇒
                               bitgleiches Commitment (zwei unabhängige
                               Läufe, inkl. 1.000-Übergangs-Folge)
```

`myl-consensus` (BFT) und `myl-scheduler` (Epochen-Zuteilung) liegen
daneben. Die BFT-Module im Überblick:

```
myl-consensus/src/
├── validator.rs        Registrierung, VRF-rotierende Komiteewahl,
│                       VotingSet (wer darf, mit welchem Schlüssel,
│                       mit welchem Gewicht)
├── bft.rs              eine Runde: Propose/Vote/Commit, signiert und
│                       stimmgewichtet
├── round_change.rs     mehrere Runden: Timeouts, Leaderwechsel,
│                       Sperre/Entsperrung, PolkaCertificate
├── poi.rs              PoI-Bündel-Einreichung (Prozess B):
│                       Signierbotschaft, Pod-Mitgliedschaft,
│                       Aggregat-Prüfung, Annahme-Registry
├── epoch_close.rs      Epochenabschluss: bestätigte Arbeit,
│                       Rückbuchung widerlegter Segmente,
│                       Streitfrist
├── da.rs               Datenverfügbarkeit: Fragment-Commitment,
│                       Aufbewahrung über die Streitfrist,
│                       definiertes Verhalten nach Ablauf
├── signing.rs          kanonische, domain-getrennte Signierbotschaften
├── voting_weight.rs    Stimmgewicht aus Stake und Arbeitshistorie
├── double_signing.rs   Erkennung + nachprüfbarer BLS-Beweis
└── block.rs            Blockinhalt nach Anhang A.5

myl-consensus/tests/
└── liveness.rs         Akzeptanz-Testmatrix Phase 3 über 21 Validatoren
```

## Changelog

### v0.52.0 (2026-10-24): Eine knappe halbe Periode zum Nachladen (`myl-ledger` 0.26.0, Fund 628)

⛔️ **Konsensänderung der Regel, nicht des Formats:** Wann eine Kopie
verstößt, hängt an der Frist. Knoten mit 0.25.0 und 0.26.0 liefen beim
ersten Verstoß auseinander; die Probenetze werden gemeinsam gehoben, ein
Produktivnetz bräuchte eine Aktivierungshöhe.

- ⚑ **`FRIST_VOLLSTAENDIG_EPOCHEN` von 8 auf 31**: so lange hat eine neue
  Kopie für die erste Belegung, bevor verfehlte Epochen zählen. 31 ist das
  Größte, bei dem auch die Nachrückerin einer Kopie, die nie belegt, noch in
  derselben Periode von 64 Epochen fertig wird (31 + 2 + 31). Bei 2 s je
  Block sind das 31 Stunden statt acht.
  📌 Im vollen Archivlauf (NODE 0.59.0) wurde eine ehrliche Kopie ersetzt,
  die lud: 115 200 Blöcke in 509 s, die Frist samt zwei verfehlten Epochen
  waren im Testtakt rund 400 s. Was nachzuladen ist, wächst mit der
  Geschichte, die Frist wuchs nicht mit.
  ⚑ **Erwogen und verworfen: die Kopien eine Periode im Voraus ziehen.** Für
  die Nachrückerin gälte dieselbe Bedingung, das Fenster rückte nur eine
  Periode früher; gewonnen wären belegte Kopien schon zum Dienstbeginn, und
  diese Zwischenzeit decken die Halter. Bezahlt würde mit einem zweiten Satz
  Kopien im Zustand, einer neuen Rolle und Kopien, die eine Periode länger
  bekannt und angreifbar sind.
  ⚠️ **Die Grenze ist verschoben, nicht aufgehoben:** Eine volle Kopie holt
  die ganze Geschichte. Die Frist trägt, solange ein Knoten sie in 31
  Epochen laden kann; auf lange Sicht muss die Haltbarkeit der alten
  Perioden bei den Splittern liegen.
- **J1 (SHARED_TYPES 0.49.0):** Die Ledger-Proben bauen ihre Vorhaben mit
  Empfängerbeweis; die Ausgabe selbst prüft über `myl_types::sitzung::pruefe`
  und brauchte keine Änderung.
- **`FRIST_TEILE_EPOCHEN` = 8**, eigens für die Splitter: Sie holen nur ihre
  Teile der Gruppen einer Periode, das wächst nicht mit der Geschichte. Bis
  hierher teilten sie die Frist der Kopien.
- **Belege:** neu `eine_kopie_hat_zum_nachladen_eine_knappe_halbe_periode`
  (Vollständigkeit erst nach dreissig Epochen: kein Verstoß, kein
  Nachrücker; rot mit Frist 8). Der Splittertest lässt die Teilwurzel jetzt
  in Epoche 64 gelten statt in Epoche 0, wo jede Frist schon verstrichen
  war; erst damit wird er rot, wenn Splitter die Frist der Kopien bekommen.
  📌 Die erste Fassung des neuen Tests leitete seinen späten Beleg aus der
  Frist ab und wanderte mit ihr; die Gegenprobe blieb grün, jetzt steht die
  Zahl fest.

### v0.51.0 (2026-10-24): Die Archivpflicht im Konsens (`myl-consensus` 0.34.0, `myl-ledger` 0.25.0)

⛔️ **Konsensänderung**, zweimal: Der Zustand trägt die Historienwurzel und
den Stand der Archivpflicht; die Probenetze haben dafür neue Startwerte
(`-H1`, NODE 0.58.0). Für ein Produktivnetz bräuchte es eine
Aktivierungshöhe.

- ⚑ **`LedgerState.historie`** (Stufe A2): der Merkle Mountain Range über
  alle Blockhashes (SHARED_TYPES 0.48.0), fortgeschrieben beim Anwenden jedes
  Blocks mit dem Hash seines Vorgängers.
- ⚑ **`myl-ledger::archiv`** (Stufen A5 bis A7), `LedgerState.archiv`:
  - **Ziehung an der Langzeitgrenze** (`grenze_ziehen`): Kopien und Splitter
    aus `myl-types::archivwahl`, dazu die Historienwurzel der Grenze, gegen
    die belegt wird, und der Abschnitt der Splitter.
  - **Beleg** (`archiv_belegen`, `Anweisung::ArchivBelegen`): die Blöcke der
    Stichproben **als Bytes** (der Blockhash ist ihr Hash, wer nur Hashes
    aufhebt, kann nicht belegen) und je ein Beweis. Die erste Belegung einer
    Kopie prüft die Vollständigkeit mit 32 Stichproben über alle Höhen,
    danach 4 je Epoche.
  - **Abschluss je Epoche** (`epoche_abschliessen`): Gutschrift **umgebucht**
    aus dem Treasury-Eingang der Epoche (5 % für Kopien, 2 % für Splitter),
    die Geldmenge folgt weiter allein der Prägeformel. Zwei Epochen in Folge
    ohne Beleg nach der Frist (acht Epochen zum Nachladen) sind ein Verstoß:
    ein Zehntel des haftenden Einsatzes an die Treasury, Vermerk in der
    Verstoßhistorie, das nächste Los rückt nach, eine fehlende Zone zuerst;
    gibt es keinen Freien, ein Splitter, der dann beides hält. ⚑ **Der
    Verstoß wird festgestellt, nicht gemeldet:** Eine Meldung, die jemand
    einreichen muss, bliebe aus, wenn niemand es tut.
  - **Übergabe nach Beweis** (Stufe A6): Die Kopien einer Periode werden an
    der nächsten Grenze Halter, sobald mindestens fünf ihre Vollständigkeit
    belegt haben; sonst halten die alten weiter, ohne Frist. Vor der ersten
    Ziehung und nach einer Periode ohne Ziehung hält jeder.
  - **Splitter** (Stufe A7): Kopien nennen die Teilwurzel
    (`ArchivTeilwurzel`), sie gilt bei fünf gleichen Stimmen; Splitter
    belegen je Epoche einen gezogenen Teil (`ArchivTeilBelegen`). Ein
    Splitter wird nicht ersetzt, sein Index ist sein Teil; die Codierung
    verträgt 32 fehlende. Die Teilwurzeln aller Abschnitte bleiben im
    Zustand, für die Wiederherstellung aus Teilen.
  - ⚠️ **Was Stichproben nicht ausschliessen:** Wer die gezogenen Blöcke erst
    nach dem Ziehen bei anderen holt, besteht ebenfalls. Dagegen stehen die
    übrigen Kopien, die Halter und die Splitter.
- **Neue Fehler:** `NichtArchivkopie`, `ArchivBereitsBelegt`,
  `ArchivbelegUngueltig`.
- **Proben:** fünf im Ledger (Belegen und Übergabe, ohne Übergabe halten
  alle, falsche Belege, Verfehlen und Belohnen, Splitter samt Teilwurzel);
  Gegenproben je Prüfung rot. 📌 **Eine davon war zuerst grün:** Der Fall
  „fremder Teil“ trug auch den falschen Index und fiel schon daran; die
  Pfadprüfung des Teils hatte keinen eigenen Fall. Jetzt einer mit richtigem
  Beweis und anderem Inhalt.

### v0.50.1 (2026-10-22): Eine Werkzeugkette für alle Kisten (`myl-consensus` 0.33.1, `myl-scheduler` 0.18.1, `myl-ledger` 0.24.1)

- `rust-version` auf 1.99 in `myl-consensus`, `myl-scheduler`, `myl-ledger`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  Nur das Manifest, kein Code.
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen.

### v0.50.0 (2026-10-22): Kein Pod ohne Zusage, Nachziehen, Teilablage, Frist

Stufe 4 des Dispatchers, abgeschlossen. ⚑ **Festlegungen des
Projektinhabers vom selben Tag:** kein Pod ohne Zusage; eine Fristmeldung
wirkt erst mit den eigenen Messungen einer Mehrheit der übrigen Mitglieder.

- `myl-scheduler` 0.18.0:
  - `zuteilung_der_epoche_mit_zusagen` (neu): Miner ohne Zusage für das
    Podmodell stehen in `Zuteilung::ohne_zusage`; jeder Pod wird geprüft,
    und ist er nicht machbar, weicht das schwächste Mitglied
    (`schwaechstes`) und er zieht aus derselben Folge nach
    (`assign_pods_geprueft`, ohne Einwand bitgleich wie bisher).
  - `bereiche_des_pods` nimmt das Podmodell und kennt keinen gleichen
    Rückfall mehr: Eine Position ohne Zusage macht den Pod nicht machbar.
  - Teilablage (P10): `Mitglied::gehalten`; mit gehaltenen Bereichen
    verteilt `verteilen_mit_teilablage` (Intervalle von vorn,
    Binärsuche über die Zeit), ohne sie das Wasserfüllen wie bisher.
  - `reserve_traegt` prüft jeden Bereich samt gehaltenem Teil.
- `myl-ledger` 0.24.0: `LedgerState::{podmodell, fristsperren}`;
  `zusage_einreichen` prüft gegen das Podmodell (ohne Podmodell gilt nur
  reiner Speicher); `geltende_zusage` beachtet die Fristsperre;
  `modellzusage_der_epoche`; `fristverstoss_einreichen` mit `Fristkontext`:
  Mehrheit der übrigen über der Frist, dann gilt die Zusage in der nächsten
  Epoche nicht, und das Konto bekommt einen Verstoß.
- `myl-consensus` 0.33.0: `Anweisung::FristverstossEinreichen`, angehängt.
- ⚠️ **Ein bekannter Hebel, benannt:** Eine Zusage wirkt auf die
  Pod-Bildung einer Epoche, deren Saat schon feststeht; wer sie ändert,
  ändert, wer in die Mischung kommt. Das ist dasselbe Ein-Bit-Gewicht je
  Miner, das die Abmeldung heute schon hat.
- **Belege:** Teilablage gezielt und über 2000 Zufallspods, das schwächste
  Mitglied, die Pod-Bildung über 20 Saaten mit Gegenprobe, Zusage gegen
  das Podmodell und Fristmeldung im Ledger; `cargo test` rc=0 in allen
  drei Kisten.

### v0.49.0 (2026-10-22): Die Zusage auf der Kette und der Schnitt eines Pods

Stufe 4 des Dispatchers, Pakete P4 und P5. Die Zusage mit ihren Modellen
(`myl-types` 0.45.0) war bisher eine Datei neben dem Knoten; die Kette
kannte sie nicht und konnte deshalb nicht danach schneiden.

- **`myl-consensus` 0.32.0:** neue Anweisung `ZusageEinreichen { zusage }`,
  hinten angehängt, damit die Kennungen der übrigen bleiben.
- **`myl-ledger` 0.23.0:** `State::zusagen` je Miner (Genesis leer).
  `zusage_einreichen` prüft, dass der Halter der Unterzeichner ist und
  registriert, und prüft die Unterschrift mit dem **registrierten**
  Schlüssel. Eine Zusage für dieselbe Epoche ersetzt die vorige; behalten
  werden die kommenden und die jüngste geltende. `geltende_zusage(state,
  miner, epoche)` liest sie. Neuer Fehler `ZusageUngueltig`, hinten
  angehängt.
- **`myl-scheduler` 0.17.0:**
  - `verteilen_geordnet` hält die Reihenfolge der Positionen und gibt den
    Kopf der letzten. In der Kette hängen Besetzung, Übernahme und
    Bündelprüfung an der Position; die Verteilung darf sie nicht umstellen.
  - `bereiche_des_pods` ergibt einen `Podschnitt`: `Gleich` (der Schnitt
    von heute), `Verteilt` oder `NichtMachbar`, mit `grenzen()` je
    Position. Gleich bleibt er, solange kein Netzmodell feststeht, ein
    Mitglied keine Zusage für das Modell hat oder ein Mitglied nur einen
    Teil hält (Weg a ist noch nicht verteilt). Nicht machbar ist ein Pod,
    wenn keine Reserve den größten Bereich trägt (Festlegung E4).
  - ⚑ **`lohnt_umplanen` ist entfernt, P6 entfällt.** Die Pod-Bildung
    mischt die Miner jede Epoche neu, ein Pod besteht also nie über eine
    Epochengrenze; eine Schwelle gegen das Wandern der Bereiche hätte
    nichts, woran sie sich halten könnte. Der Modulkopf sagt das.
- **Belege:** neue Proben `eine_zusage_gilt_ab_ihrer_epoche_unter_dem_registrierten_schluessel`
  (Ledger) und `der_schnitt_eines_pods_der_kette` (Scheduler); `cargo test`
  rc=0 in allen drei Kisten (272, 107 und 127 Tests).

### v0.48.0 (2026-10-21): `myl-scheduler` 0.16.0, die Ebenenverteilung und ihre Simulation

Stufe 4 des Dispatchers, vorgezogen und geplant auf Bitte des Projektinhabers (2026-10-21): Ein Pod verteilt seine Ebenen nach Kapazität und gemessener Ebenenzeit seiner Mitglieder statt gleich. Hier P3 (Funktion) und P9a (Simulation). Noch nicht angeschlossen:
Zusage, Pod-Bildung und Umplanen (P4 bis P6) folgen mit dem Drahtformat.

- **`ebenenverteilung::verteilen`** (Konsensbaustein, rein und ganzzahlig):
  machbar nur bei genug Kapazität; der Kopf geht an das schnellste
  Mitglied, das ihn trägt; Wasserfüllen über eine Zeitschranke T per
  Binärsuche, der Rest an die Schnellsten, Gleichstand nach Kennung; die
  Saat mischt die übrigen Bereiche, der Kopf zuletzt. Dazu `engpass_us`,
  `lohnt_umplanen` (`4·neu ≤ 3·alt`) und `reserve_traegt` (Festlegung E4).
- ⚑ **Platz und Zeit des Kopfs getrennt** (`kopf_platz`, `kopf_zeit`):
  beim Bau gefunden, gemessen am Raven Platz 4, Zeit 1.
- **Simulation** (`examples/ebenenverteilung_simulation.rs`, 5000 Pods je
  Lage, k = 4, 40 Ebenen, Knoten 8 bis 64 GiB): Der gleiche Schnitt ist bei
  rund der Hälfte der Pods nicht machbar, die Verteilung bei 99 bis
  100 %. Die Zeit verkürzt die Engpasszeit gegen die Verteilung nur nach
  Kapazität um 3,8 % (fast gleiche Knoten), 30,0 % (zwei Klassen) und
  52,1 % (weit gestreut). Umplanen bei ±5 % Rauschen: 0 Wechsel in 25 000
  Epochen.
- **Belege:** neun Proben, darunter Eigenschaften über 2000 Zufallspods
  (lückenlos, in der Kapazität, Summe gleich, schneller nie weniger;
  mindestens 400 machbar) und eine Gegenprobe mit eingebautem Fehler, die
  drei Proben rot macht. `cargo test` (127) und Clippy rc=0;
  `fassungsprobe` grün.

### v0.47.1 – 2026-10-21 (`myl-scheduler` 0.15.1: der Modulkopf nennt die Zonen)

Der Kopf von `myl-scheduler/src/lib.rs` nannte als dritten Schritt noch
„Geo-Clustering unter Latenz-Constraint (konsumiert LatencyGraph aus
NETWORKING)“. Seit dem 2026-09-01 bilden Zonen aus der Registrierung die
Cluster (`zonenzuteilung`), und `myl-types` 0.44.0 hat `LatencyGraph`
entfernt. Nur der Kommentar; kein Code geändert.

**Belege:** `cargo test` rc=0, Clippy mit `-D warnings` still;
`myl-consensus` und `myl-ledger` mit `cargo check --all-targets` rc=0.

### v0.47.0 – 2026-09-06 (der Schuldspruch bringt seinen Beleg mit)

`Anweisung::SchuldspruchEinreichen` und
`transitions::schuldspruch_einreichen`.

⚑ **`apply_verdict` hat damit seinen ersten Aufrufer**, und zwar einen
mit Nachweis. Der Beleg ist die **eigene Unterschrift des
Beschuldigten** über den strittigen Übergang, in der Rolle Shard; wer
sie nicht hat, kann niemanden benennen.

**Zwei Prüfungen, und die zweite ist die unauffälligere:** Der Beleg
muss tragen, **und** die aus seinem Schlüssel abgeleitete Kennung muss
das beschuldigte Konto sein. Ohne die zweite wäre ein echter Beleg eine
Waffe gegen Dritte.

### v0.46.0 – 2026-09-06 (das Trainingspaar sagt, was es an Unabhängigkeit hergibt)

`Trainingsplan` trägt jetzt `zonendivers`, und `paare_bilden` sucht
zuerst einen Partner in einer **anderen** Zone.

⚑ **Der Fund kam aus der verzahnten Simulation, nicht aus dem Code.**
Die Inferenz hat für ihre Redundanzpaare seit jeher eine
Zonenpräferenz und meldet in `Redundanzzuteilung::zonendivers`, wenn es
keine gab. **Das Training hatte beides nicht:** Gepaart wurde nach
Podnummer, und niemand erfuhr, ob die beiden Pods überhaupt getrennt
stehen. Ein falsches Δm verdirbt nicht eine Antwort, sondern das
Modell; die Unabhängigkeit ist dort also nicht weniger wert.

⚑ **Erzwungen wird sie trotzdem nicht, und das ist eine Entscheidung.**
Seit die Zonenreste in einen Sammeltopf gehen, sind Pods in einem dünn
besetzten Netz **zonengemischt**, und ein gemischter Pod ist
unbestimmt. Zwölf Miner auf drei Zonen ergeben zwei Pods, beide
gemischt: Es **gibt** kein diverses Paar. Wer hier auf Diversität
besteht, trainiert nie. Das Feld sagt deshalb, welcher Fall eintrat,
statt den einen zu erzwingen oder den anderen zu verschweigen.

**Zwei neue Tests in `shard_assignment`** halten die Kehrseite des
Sammeltopfs fest: Bei fünf Pods tragen die drei zonenreinen die
Diversität allein; kommen alle Pods aus dem Topf, fällt sie, und
`assign_redundant_pods` meldet es.

### v0.45.0 – 2026-09-06 (der Trainingsplan kennt die Tokenzahl)

Der Probekorpus im Scheduler-Test trägt `tokens_je_segment`, wie der
Anker es seit heute verlangt.

**Ältere Einträge** (v0.44.0 und davor, 36 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- CONSENSUS/README/README.md`.
