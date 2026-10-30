# tokenomics (`myl-tokenomics`)

> **Version:** 0.25.0
> **Datum:** 2026-10-30
> **Status:** Design-Entscheidungen getroffen (Fixed-Point bestätigt,
> vTFE-Skalierung 10⁻⁶, MYL-Kleinstbeträge 10⁶, EMA-Fenster 30 Epochen
> α=2/31); 🎉 **Phasen 1 bis 5 abgeschlossen** (Auslastungsboden und
> Subventionsplan seit dem 1. September).
> **243 Tests grün** (219 Modultests, 17 adversariale, 4 Eigenschafts-, 3 Akzeptanztests).
>
> **Seit dem 27. August greift die Slashing-Staffelung wirklich.** Bis
> dahin war sie eine Tabelle mit drei Stufen, von denen immer die erste
> galt: Die Zahl der Vorverstöße war eine Eingabe, und niemand füllte
> sie. Der Ledger führt sie jetzt, und `urteil_buchen_gestaffelt` setzt
> Lesen, Bestimmen, Buchen und Vermerken in die einzige Reihenfolge, in
> der sie zusammenpassen.
>
> Gebaut sind damit: Prägefunktion und EMA, Verteilung nach Kap. 5.3, die
> vTFE-Zuschreibungsregel, ganzzahliges `exp()` und Credit-Preisbildung,
> Stake-Hinterlegung und Slashing-Matrix, die Sicherheitsbedingung
> S_min als Prüffunktion, Anlaufphase, Genesis-Verteilungsmechanik und
> der Self-Dealing-Schutz.

Prägefunktion, Burn-and-Mint-Kreislauf, Credit-Preisbildung,
Staking/Slashing-Matrix, Ausgabestruktur und Genesis. Referenzimplementierung
von Whitepaper Kap. 5 und Anhang B.1–B.4, B.7–B.8.

## Aufgabe

Der geschlossene Wertkreislauf (Kap. 5.1): Nutzer verbrennen MYL gegen
Inferenz-Credits, Miner erhalten neu geprägte MYL proportional zur
verifizierten Arbeit. Diese Komponente implementiert die konkreten Formeln
(Prägefunktion, Credit-Preisbildung, Sicherheitsbedingung S_min) auf Basis
der Ledger-Zustandsübergänge aus CONSENSUS. Wo das Protokoll `exp()`
verwendet (Credit-Preisbildung), muss die Approximation ganzzahlig erfolgen
(LUT-basiert), um dieselbe Determinismus-Anforderung wie die Inferenzseite zu
erfüllen.

## Abhängigkeiten

CONSENSUS (Ledger-Zustandsübergänge `burn`/`mint_credits`/`apply_verdict`,
Anhang A.5). Benötigt wird nur die Zustandsübergangs-Schnittstelle — die
fertige BFT-Blockproduktion ist dafür noch nicht vorausgesetzt.

## Struktur

```
TOKENOMICS/
├── README/                   diese Kurzübersicht
└── myl-tokenomics/           Tokenomik-Berechnungen (Kap. 5)
    └── src/
        ├── lib.rs             Fixed-Point-Grundregeln, Einheiten-Skalierungen
        │                      (1 MYL = 10⁶ Kleinstbeträge, vTFE 10⁻⁶)
        ├── ema.rs             Ganzzahlige EMA für B̄_e (α = 2/31, i128,
        │                      dokumentierte Totzone)
        ├── mint.rs            Prägefunktion M_e = min(B̄_e·(1+s), M_max)
        ├── distribute.rs      Kap.-5.3-Verteilung (Basispunkte, exakte
        │                      Summe, Redundanz-Normierung, proportionale
        │                      Aufteilung)
        └── training.rs        Trainingsvergütungs-Obergrenze (≤ 70 %)
```

## Tests

**62 Modultests**, dazu **17 adversariale Tests** in `tests/adversarial.rs`
(Kritikpunkt K4).

Die Modultests belegen, dass die Formeln die vorgesehenen Werte liefern,
also den Erfolgsfall. Die adversariale Ebene prüft die **Eigenschaften,
die nach jeder Rechnung gelten müssen**, über Eingaben, die niemand
ausgesucht hat, einschließlich der Ränder des Zahlbereichs:

| Eigenschaft | warum sie zählt |
|---|---|
| die Verteilung gibt genau die Prägung aus | ein verschwundener Rest ist Geld, das niemand bekommt; ein doppelter ist Geld aus dem Nichts |
| kein Empfänger bekommt mehr als seinen Schlüssel | sonst wäre „Rundungsrest" ein Kanal |
| die Prägung übersteigt nie `M_max` | der einzige harte Deckel der Geldmenge |
| extreme Subventionsparameter prägen nicht aus dem Nichts | ⚑ Fund 46 |
| die proportionale Aufteilung zahlt exakt `total` aus | weder mehr noch weniger |
| doppelte Adressen zahlen nicht doppelt | Gewichte werden zusammengeführt |
| ein EMA-Schritt geht nie über die Stichprobe hinaus | sonst folgte die Prägung einem Wert, den niemand verbrannt hat |
| ein α über 1 läuft nicht um | ⚑ Fund 47 |
| der Preis läuft an keiner Eingabe um | ⚑ Fund 46, dritte Stelle |
| `exp_approx` hält jede Eingabe aus | ein Index außerhalb der Tabelle wäre eine Panik im Konsenspfad |
| die Trainingsvergütung bleibt unter 70 % | Kap. 5.6: sonst wäre Training attraktiver als Inferenz |
| die Redundanz-Normierung rundet nach unten | sonst bekämen zwei Pods zusammen mehr als eine volle Gutschrift |
| ein Zuschnitt beansprucht nie mehr als das ganze Modell | die Abrechnungsgrundlage des Netzes |
| ein Zuschnitt außerhalb des Modells wird abgelehnt | sonst ließe sich Arbeit abrechnen, die es nicht gibt |

**Warum die Ränder und nicht nur plausible Werte:** Alle Parameter dieses
Crates sind für Governance vorgesehen (Kap. 10.3). Eine Abstimmung kann
jeden auf jeden Wert setzen, den der Typ hergibt. „So wird das niemand
konfigurieren" ist keine Zusicherung, sondern eine Hoffnung.

**Die Gegenprobe steht dabei:** Ein vollständiger Zuschnitt muss die
volle Gutschrift bekommen. Eine Funktion, die immer null liefert,
verletzt keine Obergrenze.

## Changelog

### v0.25.0 (2026-10-30): Die Ausschüttung vorgemerkt

- ⚑ **`epochenausschuettung_vorgemerkt`** (verteilte Ausschüttung, nach dem
  Vorbild von Solanas „Partitioned Epoch Rewards“, SIMD-0118): dieselben
  Beträge, Klassen und derselbe Abschluss wie
  `epochenausschuettung_mit_pruefern`, aber jeder Anteil kommt in
  `LedgerState::auszahlungen_offen` und wird in den folgenden Blöcken
  begrenzt gebucht. Kein Konto wird dabei berührt (ausser der Treasury,
  wenn aus ihrem Bestand gezahlt wird); die Überlaufprüfung je Konto
  entfällt, denn sie läse jedes Konto, und gebucht wird gekappt. ⚑ Abwägung
  gegen Cosmos F1 (Abruf durch den Miner): F1 macht den Wechsel O(1),
  verlangt aber, dass jeder abholt, und je Miner einen offenen Posten; die
  Partitionen behalten Bedeutung und Bedienung und begrenzen trotzdem je
  Block. Die Bibliothek und die Simulation buchen weiter sofort.
- Whitepaper Kap. 5.3: Aufteilung und Beträge unverändert; nur der
  Zeitpunkt der Gutschrift liegt bei sehr vielen Minern in den ersten
  Blöcken der neuen Epoche (bei bis zu 4096 Empfängern im Wechselblock
  selbst).
- Test `vorgemerkt_ergibt_dieselben_guthaben`: 10 000 Miner, die
  vorgemerkte Ausschüttung berührt höchstens ein Konto (aufgezeichnet),
  nach drei Partitionen dieselben Guthaben und dieselbe Zustandswurzel wie
  sofort gebucht.

### v0.24.0 (2026-10-29): Der Checker-Pool zahlt aus

- ⚑ **`epochenausschuettung_mit_pruefern`** (Anhang B.3: „Checker-Vergütung
  = Grundvergütung (4 % der Prägung, proportional zu geprüftem Volumen)“;
  Festlegung des Projektinhabers vom 2026-10-29): Die 4 % gehen nach dem
  geprüften Volumen der Epoche an die Auszahlungskonten der Checker. Ohne
  Volumen bleibt der Pool ungeprägt wie bisher (`KeineGewichtsquelle`);
  ohne Auszahlungskonto zählt ein Checker nicht. Das Kopfgeld (b = 30 %)
  kommt weiter aus `apply_verdict`. `epochenausschuettung_mit_training`
  ruft sie mit leerem Volumen.
- Test `der_checker_pool_zahlt_nach_geprueftem_volumen`.

### v0.23.1 (2026-10-22): Eine Werkzeugkette für alle Kisten

- `rust-version` auf 1.99 in `myl-tokenomics`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  ⚑ **Dazu Code, und zwar mit Absicht:** Mit `rust-version = "1.99"` gibt Clippy
  zwei Regeln frei, die es bei 1.85 zurückhielt (`manual_is_multiple_of`,
  `chunks_exact` mit fester Größe). Festlegung des Projektinhabers:
  umschreiben, nicht abschalten. Hier: `vtfe.rs` 1 Zeile, eine Probe 2.
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

### v0.23.0 (2026-10-22): Das Profil kennt Ebenenarten; Fund 586

⚑ **Festlegung des Projektinhabers:** Die Kette schreibt das int4-35B nach
einem Profil gut, das seine Ebenen kennt, und nicht nach einer Näherung
als dichtes Modell.

- `ModellProfil` trägt `voll_alle` (jede wievielte Ebene volle Achtsamkeit
  hat), `zustand` (`Zustandsprofil` der linearen Aufmerksamkeit),
  `geteilter_experte` und `ausgangstor`. `macs_der_ebene(i)` und
  `macs_ebenen(von, bis)` rechnen je Ebene; ohne Zustandsschicht genau wie
  bisher. Gezählt werden weiter die Gewichtsmatrizen; die Fortschreibung
  des Zustands bleibt draußen wie die Attention-Scores.
- `RAVEN_35B_A3B` (neu): das Profil des Netzmodells, aus der
  Modellkonfiguration des int4. Am echten Artefakt nachgesehen: Das aus dem
  geladenen Modell abgeleitete Profil (`myl-pod`) ist gleich.
- `arbeitsverteilung_aus_grenzen` (neu): jeder Pod seine Verteilung aus
  seinen Grenzen; `Podabrechnung::verteilung` trägt sie.
- 📌 **Fund 586:** `PROBE_MODELL` beschrieb Qwen2.5-0,5B (24 Ebenen,
  Breite 896), das Probeartefakt `sparrow-0.6b-int8` ist ein Qwen3-0.6B
  mit 28 Ebenen und Breite 1024. Die Kette schrieb nach einem Modell gut,
  das kein Pod rechnete. Behoben; der Stand der Probeverteilung heißt
  jetzt `myelith-probe-qwen3-0.6b`.
- **Belege:** das Netzmodell von Hand nachgerechnet, dichte Modelle
  bitgleich, Verteilung nach Grenzen, eigene Verteilung in der Abrechnung;
  `cargo test` rc=0.

### v0.22.2 – 2026-10-21 (die Modelle heißen nach Tieren)

Die Projektmodelle heißen seit heute nach Tieren, mit Größe und Speicherform als Anhängsel (Festlegung des Projektinhabers): `myelith-0.6b` ist `sparrow-0.6b-int8`, `myelith-4b` `pigeon-4b-int8`, `myelith-30b-a3b` `magpie-30b-a3b-int8`, `myelith-35b-a3b` `raven-35b-a3b-int8`. In den Dateien eines Artefakts steht kein Name, also bleiben Abdrücke und Register gültig. Nur ein Kommentar.

### v0.22.1 – 2026-09-10 (die Artefakte heissen nach dem Modell, das sie sind)

**Umbenennung, keine Verhaltensänderung.** Die Artefakte unter
`INTEGER_LLM/artifacts/` heissen seit heute `myelith-0.5b`,
`myelith-7b`, `myelith-4b` und `myelith-30b-a3b`; die Pfade in einem Doc-Kommentar
sind nachgezogen.

⚑ **Ein Artefakt ist nicht das Basismodell, sondern das Modell, mit dem
dieses Projekt rechnet.** Es trägt deshalb einen eigenen Namen; die
Basismodelle unter `models/` behalten ihre und stehen weiter mit
Herkunft im Katalog.

⚑ **Der Konformitätswert ist unverändert**, gemessen nach dem Umbau:
`894d8357ae92b5c1` über sechs Vektoren und `6da384ba301b9454` über
siebzehn. **Die Namen stehen in keiner Bytefolge, die gehasht wird.**

### v0.22.0 – 2026-09-06 (die Trainingsabgabe: ein antizyklischer Puffer)

### ⚑ Das Problem, in einer Zahl

Die Treasury bekommt 3 Prozent der Prägung, die Shard-Miner 78. Bei
gleicher Vergütung je Rechenstunde deckt sie damit **3,85 Prozent des
Inferenzvolumens** an Training. Kap. 7.1 lässt am Auslastungsziel 26
Prozent der Pods trainieren, im Leerlauf 82.

⚑ **Und die Lücke ist strukturell.** Training ist genau dann am
erwünschtesten, wenn Inferenz leerläuft, und beide Geldquellen sind
genau dann am kleinsten: Die Prägung folgt dem Burn, der Burn der
Nachfrage, ein Gebührenaufschlag ebenfalls.

### Die Antwort: bei hoher Auslastung abführen, bei niedriger auszahlen

`λ(u) = λ_max · u`, ein Anteil der Prägung, der von den Shard-Minern in
die Treasury geht. Das Netz spart, wenn es reich ist, und investiert,
wenn Kapazität übrig ist. Genau dann ist Training auch am billigsten,
denn die Pods stünden sonst still.

| Vorbild | Was dort geschieht | Was hier davon gilt |
|---|---|---|
| Basel III, antizyklischer Kapitalpuffer | im Aufschwung aufbauen, im Abschwung auflösen | die Richtung |
| EIP-1559, Basisgebühr | eine protokollgesetzte Gebühr steigt über einem Auslastungsziel | die Form |

⚑ **Der Unterschied zu EIP-1559 ist der Verbleib.** Dort wird die
Basisgebühr **verbrannt**; hier fliesst sie in einen Fonds und kommt als
Trainingsvergütung zu denselben Minern zurück. Verbrennen nähme dem Netz
genau das Geld, mit dem es sein Modell verbessert.

### ⚑ λ_max ist gerechnet, nicht gesetzt

Aus einer Bedingung: Am Auslastungsziel trägt die Abgabe das Training
genau.

```text
u*² · λ_max = t(u*) · (S − u* · λ_max)
λ_max = t(u*) · S / (u*² + t(u*) · u*)   =  3017 Basispunkte
```

| u | λ(u) | Shard-Miner | Zufluss | Kosten | Saldo |
|---|---|---|---|---|---|
| 1,0 | 30,2 % | 47,8 % | 0,302 | 0,010 | **+0,292** |
| **0,7** | **21,1 %** | **56,9 %** | **0,148** | **0,148** | **0,000** |
| 0,1 | 3,0 % | 75,0 % | 0,003 | 0,555 | **−0,552** |

⚑ **Rund zwei volle Epochen tragen eine leere**, und die Formel steht
als `abgabe_max_bps`: Die Herkunftsprüfung der Registry rechnet sie bei
jedem Lauf nach, damit eine abgeschriebene Zahl nicht veralten kann.

### ⚑ Die Abgabe ist kein Verlust für die Miner

Sie ist eine Umschichtung zwischen zwei Arbeitsarten derselben Leute.
Über einen Zyklus bekommen sie dasselbe Geld für mehr Arbeit, und die
Arbeit verbessert das Modell, von dem ihre künftige Vergütung lebt.

⚑ **Die drei Prozent der Treasury bleiben unangetastet.** Eine Abgabe,
die eine bestehende Haushaltsstelle still aufbraucht, wäre keine
Finanzierung, sondern eine Umwidmung.

### ⚑ Der Griff in den Puffer prägt nicht, er überweist

Was in einer Epoche zufliesst, wird geprägt: Es wäre sonst an die
Treasury geprägt worden. Was aus dem **Bestand** kommt, ist vorhandenes
Geld und wird überwiesen.

**Der Unterschied ist der ganze Punkt von Kap. 5.6.** Wer den Bestand
prägte, verdoppelte die Netto-Inflation genau so, wie es dort
ausgeschlossen wird, und der Puffer wäre keiner: Er nähme nichts weg, er
schüfe. Ein Test hält die Geldmenge dagegen.

### Die Grenze, und sie bleibt stehen

Ein Netz mit **dauerhaft** niedriger Nachfrage kann Training nicht voll
bezahlen. Das ist keine Lücke im Entwurf, sondern die Wahrheit, dass ein
Netz ohne Einnahmen nichts zu verteilen hat.

### v0.21.0 – 2026-09-06 (Training wird wie Inferenz vergütet)

### ⚑ Der Deckel aus Kap. 5.6 hat seinen Gegenstand verloren

Anhang B.7.3 begründet ihn: „Miner wählen zwischen beiden Arbeitsklassen
nach der Vergütung je Rechenstunde." **Seit die Zuteilung erzwungen ist,
wählt niemand mehr**, und derselbe Anhang sagt für diesen Fall: „Bei
Gleichstand entscheidet allein die Zuteilung."

⚑ **Gleichstand ist sogar besser als ein Deckel.** Läge Training
darunter, hätte ein zugeteilter Trainingspod einen Anreiz, absichtlich
zu scheitern und auf eine Inferenzrunde zu hoffen.

`vtfe_training_gutschrift` rechnet die Arbeit eines Trainingssegments
mit dem **Faktor drei**: Für jede Matrix `Y = X·W` fallen vorwärts eine
und rückwärts zwei Matrixmultiplikationen an. Das ist die
Standardzählung, keine Hausnummer. Sie ist eine **Obergrenze**, weil sie
den eingefrorenen LM-Kopf dreifach zählt.

### ⚑ Gleiche Rate heisst nicht gleiche Quelle, und daraus folgt eine Zahl

Kap. 5.6 zahlt Training aus der **Treasury**, nicht aus Zusatzprägung,
weil das die Netto-Inflation beinahe verdoppelte. Die Treasury bekommt
3 Prozent der Prägung, die Shard-Miner 78.

**Bei gleicher Rate deckt die Treasury damit rund 3,85 Prozent des
Inferenzvolumens**, und `trainingsdeckung_bps` macht die Zahl lesbar.
Anhang B.7.2 sagt dasselbe von der anderen Seite.

⚑ **Damit kollidiert sie mit dem Trainingsanteil aus Kap. 7.1**, wenn
dieser hoch steht: Bei einem Freianteil von 8000 Basispunkten trainieren
im Leerlauf 82 Prozent der Pods.

⚑ **Und die Kollision ist strukturell.** Training ist genau dann am
erwünschtesten, wenn Inferenz leerläuft, und **beide** Geldquellen sind
genau dann am kleinsten: Die Prägung folgt dem Burn, der Burn folgt der
Nachfrage, der Gebührenaufschlag ebenfalls.

**Die ehrliche Folge ist keine Kürzung der Rate, sondern eine Teilung
dessen, was da ist.** Bei viel Training fällt die Vergütung je Einheit;
ein leerlaufender Miner, der wenig verdient, steht immer noch besser da
als einer, der nichts tut. Ein Test hält das Verhältnis fest.

### v0.20.0 – 2026-09-05 (der Epochenabschluss rollt die Nachfrage mit)

`epochenabschluss_burn` schreibt jetzt auch `vtfe_vorepoche` fort und
nullt `vtfe_epoche`.

⚑ **Fachlich gehört das ins Kontenbuch und steht trotzdem hier.** Dieser
Abschluss ist die **eine Stelle mit dem Wächter**: `burn_ema_bis >=
epoch` verhindert die doppelte Fortschreibung. Ein zweiter, eigener
Epochenabschluss bräuchte einen zweiten Wächter, und der Kopf der Datei
warnt genau davor: „zweimal gerufen, verschiebt sie den Durchschnitt,
und niemand sähe es der Zahl an."

**Ein Wächter für zwei Zähler schlägt zwei Wächter.**

### `utilization` reicht durch statt zu rechnen

`UTILIZATION_SCALE` und `calculate_utilization` verweisen auf
`myl_types::auslastung`. Der Scheduler braucht dieselbe Skala, und eine
Konsenskonstante an zwei Orten ist eine zweite Wahrheit. Die
`f64`-Helfer bleiben hier: Sie dienen der Anzeige, und `myl-types` liegt
im Konsenspfad des Gleitkomma-Audits.

### v0.19.0 – 2026-09-04 (⚑ Fund 171: die Stichprobenrate bekommt eine Heimat)

`sicherheit::{STICHPROBE_ZAEHLER, STICHPROBE_NENNER, stichprobe_bp}`.

Die Rate stand an **drei** Stellen mit **zwei** Werten: als
Registry-Vorgabe (5/100, las niemand), als `Kette::STICHPROBE_BP`
(200 bp = 2/100, las die Kette bei jeder Ziehung) und als Literal in der
Vorgabe von `MindestStake` (5/100, setzte den Mindest-Einsatz).

⚑ **Der Unterschied war kein Schreibfehler, sondern eine vergessene
Entscheidung.** A1 hat die Kontrollsegmente entfernt; seither muss `p`
allein tragen, was vorher `p` und `gamma` teilten, und
`security_sim.py::zusammengelegte_rate` rechnet dafür 4,96 %. Die
Registry hat den Wert übernommen, die Kette nicht.

Alle drei leiten jetzt von hier ab. Solange der Konsens die Registry
nicht liest (B10), ist das die eine maßgebliche Stelle.

### v0.18.0 – 2026-09-03 (die Genesis-Verteilung verlässt das Repositorium)

`genesis.rs` ist entfernt. Sie war gebaut, geprüft und hatte **keinen
Aufrufer**; entfernt wurde sie nicht wegen Mängeln, sondern weil sie
**genau einmal laufen darf** und ihr Lauf unumkehrbar ist. Der Startwert
der Kette geht in jede Transaktionssignatur ein, und wer am Block 0 was
hält, lässt sich nie korrigieren.

**Was von der Konstruktion bleibt, weil es die Zusage trägt**, steht
weiter im Abschnitt zu Kap. 5.7: „Kein Vorverkauf" war nicht durch eine
Prüfung durchgesetzt, sondern durch die **Form der Funktion**.

### v0.17.1 – 2026-09-03 (⚑ Fund 162: die Shardzahl ist jetzt gebunden)

Die Shardzahl der Pipeline und die der Gewichtsableitung standen beide
auf vier, und **nichts im Code hielt sie zusammen**. Eine Abweichung
hätte stumm dazu geführt, dass ein Pod für seine Arbeit nichts bekommt.
Zwei `const`-Zusicherungen binden sie jetzt.

⚑ **Der erste Anlauf war Zierde**, und die Gegenprobe fing ihn: Ein
assoziiertes `const` im `impl`-Block wird erst berechnet, wenn es
jemand benutzt. Auf Modulebene verschoben, beisst es.

**Ältere Einträge** (v0.17.0 und davor, 19 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- TOKENOMICS/README/README.md`.
