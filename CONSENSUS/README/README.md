# consensus (`myl-consensus` + `myl-ledger` + `myl-scheduler`)

> **Version:** 0.12.0 (`myl-scheduler` 0.2.11, `myl-ledger` 0.2.0)
> **Datum:** 2026-08-26
> **Status:** Design-Entscheidungen getroffen (malachite hinter
> trait-Grenze mit Eigenbau-Fallback, Blockzeit 2 s, Komitee 21/7,
> Streitfrist 7 Tage, Reed-Solomon k=8/m=4 — Details im Fahrplan);
> Phase 2 ✅ abgeschlossen (`myl-ledger` v0.1.1–v0.1.5,
> `myl-scheduler` v0.2.1–v0.2.9); **Phase 3 ✅ vollständig**
> (`myl-consensus` v0.3.1–v0.5.0): signiertes, stimmgewichtetes BFT mit
> VRF-rotierender Komiteewahl, Double-Signing-Beweis und seit v0.5.0
> Rundenwechsel mit Sperrmechanik — Safety **und** Liveness, die
> Akzeptanz-Testmatrix über 21 simulierte Validatoren läuft;
> **Phase 4 ✅ vollständig** (4.1 PoI-Bündel-Einreichung,
> 4.2 Epochenabschluss, 4.3 DA-Schicht) — **alle vier Phasen
> abgeschlossen**.

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
├── README/                   diese Kurzübersicht + Fahrplan
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

### myl-consensus v0.12.0 – 2026-08-26 (die Form auf der Leitung)

`Konsensnachricht` fasst Propose, Vote und Commit zu einem Typ zusammen,
den ein Gossip-Topic tragen kann, und `Propose`/`Vote`/`Commit`
bekommen Borsh-Ableitungen. Damit ist der Zustandsautomat aus `bft.rs`
zum ersten Mal über ein Netz erreichbar; die Verdrahtung liegt in
`NODE/myl-node`.

**Was der Typ zusätzlich kann:** `runde()` und `absender()`, damit ein
Knoten eine Nachricht der falschen Runde verwerfen kann, **ohne** sie
erst dem Zustandsautomaten vorzulegen.

⚑ **Was der Borsh-Parse hier leistet, ist gemessen: fast nichts.** Von
20 000 verstümmelten Nachrichten kommen **99 %** durch, weil alle Felder
feste Breite haben (Runde 8, Hash 32, Miner-Id 32, Signatur 96). Das ist
dieselbe Eigenschaft wie in Fund 45 und Fund 57.

**Der Unterschied zu Fund 45 ist, dass die eigentliche Prüfung hier
erreichbar ist.** Bei PoI-Bündeln blieb die Aggregatsignatur ungeprüft,
weil niemand sie prüfte. Hier prüfen `receive_propose`, `receive_vote`
und `receive_commit` jede Nachricht gegen Runde, Mitgliedschaft,
Duplikat und BLS-Signatur, und der Knoten ruft sie auch auf. Der Parse
ist die Eingangstür, nicht die Prüfung.

### myl-consensus v0.11.0 – 2026-08-23 (adversariale Testebene, K4)

`liveness.rs` prüft, dass ehrliche Validatoren zu einem Block kommen,
also den **Erfolgsfall**. K4 verlangt den Gegenfall, und der stand
bisher nicht da.

`tests/adversarial.rs` beschreibt neun Angriffe auf das
Polka-Zertifikat, und jeder muss scheitern:

| Angriff | wird abgelehnt weil |
|---|---|
| dieselbe Stimme fünfzehnmal einsetzen | Unterzeichner sind streng aufsteigend, Duplikate strukturell ausgeschlossen |
| unsortierte Unterzeichnerliste | ein Stimmensatz hat genau eine Kodierung |
| ein Unterzeichner außerhalb des Komitees | sein Schlüssel steht nicht im `VotingSet` |
| knapp unter dem Quorum (14 statt 15) | Stimmgewicht unter der Schwelle |
| Block nachträglich austauschen | die Unterschrift gilt dem alten Text |
| Zertifikat in einer anderen Runde einsetzen | Rundenbindung; sonst ließe sich ein altes Polka wiederverwenden und ein gesperrter Validator entsperren (BFT-Safety, vgl. Fund 27) |
| erfundene Aggregatsignatur | `fast_aggregate_verify` schlägt fehl |
| leeres Zertifikat | null Stimmen sind kein Quorum |
| 20 000 zufällige Zertifikate | keines gilt, keines stürzt ab |

**Der erste Test ist die Gegenprobe**, und er ist der wichtigste: Das
**ehrliche** Zertifikat muss gelten. Ohne ihn wären die neun darunter
wertlos, denn eine Prüfung, die alles ablehnt, lehnt auch jeden Angriff
ab. Genau diese Falle hat dieses Projekt schon zweimal bezahlt
(Fund 33, und der erste Anlauf des Pod-Fuzzers).

**Die Angriffe sind nicht ausgedacht.** Wo ein Kommentar im Quelltext
sagt „das schließt X aus", steht jetzt der Test, der X versucht.

### myl-ledger v0.2.0 – 2026-08-23 (Invarianten statt Erfolgsfall, K4)

Kritikpunkt K4 lautet: *„Die Tests belegen überwiegend den
Erfolgsfall."* Für dieses Crate stimmte das. `determinism.rs` prüft, dass
zwei Läufe derselben Folge denselben Zustand ergeben, und das ist
richtig und wichtig, **sagt aber nichts darüber, ob der Zustand
stimmt**: Zwei Läufe derselben falschen Rechnung sind ebenso bitgleich.

Neu ist `tests/invarianten.rs` mit fünf Eigenschaften, die nach **jedem**
Übergang gelten müssen, geprüft über Folgen, die niemand von Hand
ausgesucht hat:

1. **MYL steigt niemals.** Kein Übergang prägt; `burn_to_credits`
   verbrennt, `apply_verdict` schlachtet und verteilt einen Teil weiter.
   Ein Übergang, der Geld erzeugt, wäre ein Loch in der Geldmenge.
2. **Credits sind durch verbranntes MYL gedeckt.** `Credits · Preis` darf
   den MYL-Schwund nie übersteigen; die Abrundung geht zu Lasten des
   Käufers, nie zu Lasten der Deckung.
3. **Ein abgelehnter Übergang lässt den Zustand bitgleich.** Fünf Fälle,
   die fehlschlagen müssen, jeweils gegen das State-Commitment geprüft.
   Ein halb angewendeter Übergang wäre ein Konsensbruch, weil zwei Knoten
   ihn an verschiedenen Stellen abbrechen könnten. **Hier wird
   ausschließlich der Fehlschlag geprüft**, also genau das, was K4
   vermisst.
4. **Das Kopfgeld übersteigt nie den geschlachteten Betrag.**
5. **Extreme Beträge laufen nicht um**, geprüft an der u64-Bereichsgrenze.

**Zwei Gegenproben, weil ein grüner Test nichts beweist.** Erstens bewegt
die Zufallsfolge echten Zustand: bei Keim 1 verschwinden 1,1 Mio. MYL und
2707 Credits entstehen, die Übergänge werden also nicht reihenweise
abgelehnt. Zweitens wurde ein Übergang eingebaut, der ein einziges MYL
erzeugt; die Invariante fliegt bei Keim 1, Schritt 5 auf und nennt die
Beträge. Danach zurückgenommen.

**Kein `proptest`, kein `quickcheck`.** Beide wären bequem und beide eine
weitere Abhängigkeit in einem Crate, das den Konsens rechnet; die Kosten
trägt jeder, der das Repositorium baut. Ein xorshift64 in zehn Zeilen
leistet dasselbe, solange die Folge reproduzierbar ist. Was fehlt, ist
das automatische Verkleinern eines Gegenbeispiels; dafür nennt jeder
Fehlschlag Keim und Schritt.

### myl-consensus v0.10.0 – 2026-08-23 (Stimmgewicht: Bezugswert und Deckel)

**Der Arbeitsanteil des Stimmgewichts war um drei bis fünf
Größenordnungen zu hoch bewertet.** Die Wiedervorlage vom 2026-08-18
nannte zwei offene Punkte, beide blockiert durch dieselbe fehlende Zahl:
*„die real erreichbare vTFE-Menge pro Epoche, die noch nicht gemessen
ist."*

Seit der Festlegung der vTFE-Zuschreibung (`myl_tokenomics::vtfe`,
selber Tag) ist sie ausrechenbar. **`VTFE_UNIT` als Bezug entspricht dem
Vorwärtspass eines einzigen Tokens.** An den gemessenen Durchsatzwerten
und einer Stunden-Epoche:

| Fall | Verdopplung nach | Faktor nach einer Epoche | volle Historie |
|---|---|---|---|
| 0,5B, ganzes Modell, 49,17 tok/s | 0,020 s | **177 012** | 1 420 568 |
| 0,5B, Viertel-Shard | 0,081 s | 44 253 | 355 142 |
| 7B, ganzes Modell, 10,74 tok/s | 0,093 s | 38 664 | 310 289 |
| 7B, Viertel-Shard | 0,404 s | 8 921 | 71 593 |

*Durchsatzwerte vom 2026-08-24, nach der Zeilen-Parallelisierung. Die
erste Fassung dieser Tabelle rechnete mit 38,19 und 2,07 tok/s und blieb
stehen, als sich der Durchsatz verschob; gefunden von der Härtungsschleife
(Fund 51).*

Der Stake hörte damit nach wenigen Sekunden Arbeit auf, Angriffskosten
zu sein. Genau davor warnte der zweite offene Punkt der Wiedervorlage;
die Zahlen zeigen, dass es der Normalfall ab der ersten Epoche gewesen
wäre.

**Zwei Sicherungen**, `StimmgewichtsParameter`:

- `arbeitsbezug` (Vorgabe **1,7 · 10⁹**): die vTFE-Menge, die einen
  Bonus in Höhe des Stakes wert ist. Hergeleitet aus dem Referenzfall
  „ein Viertel von 7B, eine Stunden-Epoche, 10,74 tok/s"; die erste Herleitung rechnete mit 2,07 tok/s, dem Durchsatz vor der Zeilen-Parallelisierung (Fund 51).
- `hoechstfaktor` (Vorgabe **10**): Das Gesamtgewicht übersteigt den
  Stake nie um mehr als diesen Faktor.

**Warum zwei und nicht eine:** Der Bezug ist parametrisch und kann
falsch gesetzt werden, der Deckel nicht. Als Test festgehalten
(`der_deckel_faengt_eine_fehlkalibrierung_ab`): Mit dem alten Bezugswert
und dem neuen Deckel landet dieselbe Arbeit bei Faktor 10 statt bei 1719.

Ein Knoten mit Referenzdurchsatz über die volle Historie liegt bei rund
dem Achtfachen, also knapp unter dem Deckel. Absicht: Der Deckel soll
erreichbar sein, aber erst oberhalb des Referenzdurchsatzes.

**Konsensrelevant.** Beide Werte gehören in die Governance-Registry und
stehen hier als Startparameter; unbrauchbare Werte fallen auf die
Vorgabe zurück, statt eine Division durch null oder ein Gewicht von null
zu erzeugen. Ein Gewicht von null wäre die Bootstrap-Blockade, gegen die
die Summenform überhaupt gebaut wurde.

**Zwei Tests, die die alte Kalibrierung festhielten, sind umgeschrieben.**
`calculate_voting_weight_basic` behauptete, eine vTFE-Einheit verdopple
das Gewicht. Das war richtig beschrieben und falsch kalibriert; der Test
prüft jetzt den Bezugswert, und ein zweiter hält fest, dass ein einzelnes
Token das Gewicht **nicht** mehr nennenswert bewegt.

### myl-consensus v0.9.0 – 2026-08-19 (Punkt 4.3: DA-Schicht — Phase 4 vollständig)

Segmentdaten werden erasure-codiert abgelegt und über die Streitfrist
vorgehalten. Damit ist **Phase 4 abgeschlossen und der CONSENSUS-Fahrplan
vollständig.**

**Die Erasure-Mathematik liegt in `myl-types::erasure`**, nicht hier. Sie
gehört zu den Primitiven wie Hash, Merkle, VRF und BLS; eine zweite Kopie
in einer Komponente wäre genau der Fehler aus Fund A6.

**Cauchy statt Vandermonde.** Bei einer Vandermonde-Matrix ist die
Invertierbarkeit **jeder** k×k-Teilmatrix nicht automatisch gegeben, und
dieses Loch äußert sich nicht als Fehler, sondern als Rekonstruktion, die
für bestimmte Ausfallmuster stillschweigend falsche Daten liefert. Bei
`C[i][j] = 1/(x_i ⊕ y_j)` mit disjunkten Mengen ist jede quadratische
Teilmatrix invertierbar. Der Test fährt alle **495** Teilmengen von 8 aus
12 durch — die Eigenschaft ist geprüft, nicht angenommen.

**„Abgelaufen" ist nicht „nicht vorhanden".** Das Akzeptanzkriterium
verlangt definiertes Verhalten nach Fristablauf, und das ist eine
Sicherheitsanforderung: Gäbe es nur „habe ich nicht", wäre Zurückhalten
von regulärem Ablauf nicht zu unterscheiden und damit folgenlos — man
müsste nur behaupten, es sei alt. `DaStore::fetch` prüft die Frist
deshalb **vor** dem Nachschlagen und antwortet `Expired`, auch wenn die
Daten noch dort liegen. Die Antwort ist aus öffentlichen Größen
nachrechenbar. Innerhalb der Frist bekommt ein Zurückhaltender
`FragmentMissing` — ein Vorwurf, kein Normalzustand.

Ein Nebeneffekt derselben Regel: Aufräumen ändert das Protokollverhalten
nicht, also darf jeder Knoten zu einem anderen Zeitpunkt aufräumen, ohne
dass die Antworten auseinanderlaufen (`aufraeumen_aendert_die_antwort_nicht`).

**Der Fragmentindex geht ins Merkle-Blatt ein.** Ohne ihn wären Fragmente
gleichen Inhalts austauschbar, und ein Speicher könnte Fragment 3 als
Antwort auf die Anfrage nach Fragment 7 ausliefern.

**Tests:** 20 neu hier, 17 in `myl-types`. Crate **212 grün**
(202 Unit + 10 Akzeptanzmatrix), clippy sauber.

### myl-consensus v0.8.0 – 2026-08-19 (Punkt 4.2: Epochenabschluss)

Aus den eingereichten Ansprüchen wird die **bestätigte** Arbeit. Der
Unterschied ist der ganze Punkt: 4.1 stellt fest, dass ein Pod eine Menge
geschlossen behauptet hat, 4.2 stellt fest, ob sie ihm zusteht. Neues
Modul `epoch_close.rs`.

**Entwurfsgrundsatz: alles, was nicht positiv belegt ist, zählt nicht.**
Myelith ist quelloffen — ein Angreifer kennt jede Regel dieses Moduls.
Eine Regel, die nur schützt, solange niemand sie kennt, schützt nicht.
Die Grundeinstellung ist Ablehnung; jede Gutschrift braucht einen
positiven Beleg.

Der wichtigste Einzelfall: **`PodAgreement::Missing` ist nicht `Match`.**
Ein fehlendes Vergleichsergebnis führt zu null, nicht zur Gutschrift.
Wäre es umgekehrt, hätte ein Angreifer eine billige Strategie — den
Redundanzpartner unerreichbar machen und für die ausbleibende Aussage
bezahlt werden. Dass ein fehlender Eintrag genauso behandelt wird wie ein
ausdrückliches `Missing`, ist eigens getestet.

Weiter: Rückbuchungen sind über die **Segment-Id** idempotent (ohne diese
Bindung ließe sich ein ehrlicher Pod durch Wiederholung auf null
bringen), können nicht ins Minus laufen (ein negativer Saldo wäre eine
Gutschrift an alle anderen), und ein Urteil über einen Pod ohne Bündel
schafft keinen Anspruch.

**Die Stufe-1-Ergebnisse kommen als Abbildung, nicht als
Rückruffunktion.** Der Abschluss muss auf jedem Knoten aus denselben
Eingaben denselben Wert ergeben. Eine Abbildung ist ein Datum, das mit
dem Block reisen und geprüft werden kann; eine Rückruffunktion wäre
knotenlokales Verhalten.

**Nicht entschieden:** was eine vTFE-Einheit zählt. Die offene Festlegung
„Layer statt Shards" wird nicht implizit getroffen — `vtfe_claimed` geht
als Zahl durch.

**Tests:** 19 neu, Crate **191 grün** (181 Unit + 10 Akzeptanzmatrix),
clippy sauber.

### myl-consensus v0.7.0 – 2026-08-19 (Fund 27 geschlossen: Besitznachweis Pflicht)

**Der Rogue-Key-Schutz, auf dem 3.6 und 4.1 stehen, existierte nicht.**
`myl-types` sagte zu, Identitäts- und Subgruppen-Prüfung schützten gegen
Rogue-Key-Angriffe auf `FastAggregateVerify`. Das ist widerlegt: zu einem
fremden `pk_opfer` lässt sich `pk_rogue = g₁^x · pk_opfer⁻¹` bilden, der
beide Prüfungen besteht, und danach gilt eine allein vom Angreifer
erzeugte Signatur als Aggregat beider Schlüssel.

Betroffen waren beide Aggregat-Prüfungen dieses Crates:

- **`round_change.rs`** — ein Validator hätte allein ein
  `PolkaCertificate` erzeugen, gesperrte Validatoren entsperren und damit
  zwei Blöcke auf derselben Höhe ermöglichen können. **BFT-Safety.**
- **`poi.rs`** — ein einzelnes Pod-Mitglied hätte die Bestätigung des
  ganzen Pods fälschen und Arbeit beanspruchen können, die niemand
  geleistet hat.

**Geschlossen an der Wurzel, nicht an den Aufrufstellen.** `myl-types`
v0.3.0 liefert `BlsProofOfPossession`; dieses Crate verlangt ihn dort, wo
ein fremder Schlüssel zum ersten Mal ins Verfahren kommt:

- `ValidatorRegistry::register(miner_id, pubkey, pop, stake, epoch)` —
  neue Fehlervariante `ValidatorError::InvalidProofOfPossession`.
- `PodMembership::new(...)` nimmt je Mitglied
  `(MinerId, BlsPublicKey, BlsProofOfPossession)` — neue Variante
  `PoIError::InvalidProofOfPossession { member }`. Der Nachweis wird
  geprüft, aber nicht gespeichert: er gehört zur Aufnahme, nicht zum
  Zustand.

**Anmerkung zum Ort der Pod-Prüfung.** Sie gehört eigentlich in eine
Miner-Registrierung — einmal beim Eintritt statt bei jeder Pod-Bildung.
`myl-scheduler::MinerRegistration` trägt heute aber gar keinen Schlüssel,
deshalb ist `PodMembership::new` derzeit die erste Stelle, an der ein
fremder Miner-Schlüssel auftaucht. Im Modul vermerkt, damit die Prüfung
mitwandert, sobald es die Registrierung gibt.

**Breaking:** beide Signaturen geändert; `myl-testclient` nachgezogen.

**Tests:** neu `register_verlangt_besitznachweis` und
`mitglied_ohne_gueltigen_besitznachweis_wird_abgelehnt`; die eigentliche
Regression liegt bei `myl-types` (`tests/rogue_key.rs`, 5 Tests). Crate
**175 grün** (165 Unit + 10 Akzeptanzmatrix), clippy sauber.

### myl-consensus v0.6.0 – 2026-08-19 (Phase 4 begonnen: PoI-Bündel-Einreichung)

**Punkt 4.1.** Prozess B (Kap. 3.5.2): Ein Pod-Koordinator reicht am
Epochenende ein `PoIBundle` ein, das die Inferenzarbeit seines Pods
beansprucht. Neues Modul `poi.rs` mit `poi_bundle_message`,
`PodMembership`, `verify_bundle_signature` und `PoIRegistry`.

**Die tragende Regel: die Schlüsselmenge kommt aus der Zuteilung des
Schedulers, nie aus dem eingereichten Bündel.** Das klingt
selbstverständlich und ist der Punkt, an dem sich Aggregat-Signaturen
still aushebeln lassen. `FastAggregateVerify` prüft ein Aggregat gegen
eine Liste öffentlicher Schlüssel; nimmt man diese Liste aus dem
eingereichten Objekt, prüft man nur noch „haben die, die unterschrieben
haben, unterschrieben?". Ein Pod aus fünf Mitgliedern könnte dann mit
der Signatur eines einzigen einreichen. `PodMembership` stammt deshalb
aus `myl-scheduler` (Anhang A.2) und ist die maßgebliche Quelle.

**Akzeptanzkriterium erfüllt.** „Ein PoI-Bündel mit fehlender oder
falscher Signatur eines Pod-Mitglieds wird abgelehnt" —
`fehlende_signatur_eines_mitglieds_wird_abgelehnt` fährt das für **jedes**
Mitglied einzeln durch, dazu Tests für die Einzelsignatur statt aller
und für die fremde Signatur anstelle eines Mitglieds.

**`vtfe_claimed` ist mitsigniert.** Stünde die beanspruchte Arbeitsmenge
nicht in der Botschaft, könnte der Koordinator sie nach dem Einsammeln
der Signaturen hochsetzen — die Mitglieder hätten eine Menge bestätigt,
die sie nie gesehen haben, und das Aggregat bliebe gültig.

**Doppel-Sperre je `(Epoche, Pod)`** als Konsensregel, nicht als
Aufräumhilfe: ohne sie könnte derselbe Anspruch mehrfach eingereicht und
mehrfach geprägt werden. Eine fehlgeschlagene Prüfung hinterlässt keinen
Zustand — sonst sperrte ein geschickt gebautes Falschbündel den ehrlichen
Koordinator aus (`abgelehntes_buendel_hinterlaesst_keinen_zustand`).

**Bewusst nicht entschieden:** ob `vtfe_claimed` inhaltlich stimmt. Das
Modul stellt fest, dass der Pod die Menge geschlossen bestätigt hat —
nicht, dass sie korrekt ist. Die Bestätigung ist Punkt 4.2 und hängt an
der offenen Festlegung, **was eine vTFE-Einheit zählt** (Layer statt
Shards). Sie wird hier nicht implizit getroffen; `vtfe_claimed` ist
Eingabe.

**⚠ Fund 27 — die Aggregat-Prüfung trägt noch nicht allein.**
`myl-types` sagt zu, dass Identitäts- und Subgruppen-Prüfung gegen
Rogue-Key-Angriffe auf `FastAggregateVerify` schützen. Diese Zusage ist
falsch, und zwar nachgewiesen: ein Schlüssel `pk_rogue = g₁^x · pk_opfer⁻¹`
besteht beide Prüfungen, und danach gilt
`fast_aggregate_verify([pk_opfer, pk_rogue], msg, σ)` für ein σ, das der
Angreifer allein erzeugt hat. Betroffen sind beide Aufrufstellen im
Projekt: dieses Modul und `round_change.rs`. Heute nicht ausnutzbar, weil
Registrierung zwei Epochen vor Gruppenbildung schließt — eine Eigenschaft
des Zeitplans, keine kryptografische Garantie. Empfehlung:
Proof-of-Possession bei der Registrierung. Konsensrelevant und
komponentenübergreifend, deshalb dokumentiert und nicht nebenbei
behoben; Details im Fahrplan-Master.

**Tests:** 26 neue in `poi.rs`, Crate insgesamt **173 Tests grün**
(163 Unit + 10 Akzeptanzmatrix), clippy mit `-D warnings` sauber.

### myl-consensus v0.5.0 – 2026-08-19 (Phase 3 abgeschlossen: Rundenwechsel)

**Punkt 3.6, der letzte offene der Phase.** Bis hierher deckte
`bft.rs` genau **eine** Runde ab. Fiel der Leader aus, blieb sie stehen:
niemand schlug vor, nichts schaltete weiter. Safety war erfüllt (nichts
Falsches wurde commitet), Liveness nicht (unter Umständen wurde gar
nichts commitet). Die Akzeptanz-Testmatrix der Phase war damit nicht
durchführbar — ein Test, der auf einen Fortschritt wartet, der nicht
kommen kann, prüft nichts.

Neu: `round_change.rs` mit `RoundDriver`, `TimeoutConfig`, `Lock` und
`PolkaCertificate`.

**Der Rundenwechsel bringt die Sperrmechanik zwingend mit.** Der naive
Wechsel — „Timeout, nächster Leader, neuer Vorschlag" — ist nicht bloß
unvollständig, er ist falsch: Erreicht Block A in Runde 1 ein Quorum,
sehen das aber wegen einer Partition nur einige Knoten, so wechseln die
übrigen in Runde 2 und commiten dort B. Zwei Blöcke auf derselben Höhe,
erzeugt durch genau den Mechanismus, der die Liveness herstellen sollte.
Deshalb sperrt sich ein Validator mit dem Quorum auf `(A, r)` und löst
die Sperre nur gegen ein `PolkaCertificate` für B aus einer Runde echt
zwischen Sperrrunde und laufender Runde — dann kann A nicht commitet
worden sein.

**Fristen wachsen linear mit der Rundennummer.** Ein fester Timeout
stellt keine Liveness her: ist er kürzer als die reale
Nachrichtenlaufzeit, platzt jede Runde vor Eintreffen der Votes und das
Protokoll wechselt endlos. Da die Laufzeit vor GST unbeschränkt ist, kann
kein fester Wert richtig sein. Mit `basis + runde × delta` gibt es eine
Runde, ab der die Frist das reale Δ überschreitet — ab dort commitet das
Protokoll. `TimeoutConfig::is_live()` macht kenntlich, dass `delta = 0`
sicher, aber möglicherweise dauerhaft blockiert ist.

**Keine Uhr im Modul.** Jede zeitabhängige Funktion bekommt `now_ms`
übergeben. Ein Zustandsautomat, der selbst `SystemTime::now()` aufruft,
ist nicht reproduzierbar und damit nicht nachprüfbar (Kap. 10.3). Als
Nebeneffekt läuft die ganze Testmatrix ohne Threads und ohne Warten.

**Konsensvertrag additiv erweitert:** neues Domain-Präfix
`DST_PROPOSE_POL` für Vorschläge mit Polka-Bezug, statt `DST_PROPOSE`
zu ändern — so bleibt jede zuvor erzeugte Signatur gültig. Die
`valid_round` ist mitsigniert; ohne diese Bindung könnte ein Angreifer
sie hochsetzen und gesperrte Validatoren zum Entsperren bewegen, bei
weiterhin gültiger Signatur.

**Härtung des Zertifikats.** Die Unterzeichnerliste muss streng
aufsteigend sein. Das ist nicht Kosmetik: ohne Duplikatschutz erreicht
ein einzelner Schlüssel das Quorum, indem er dieselbe Stimme mehrfach
einreicht. Geprüft wird in der Reihenfolge billig-vor-teuer, damit die
Aggregat-Verifikation nicht als DoS-Fläche vorn steht.

**Tests:** 34 neue Unit-Tests in `round_change.rs`, 3 in `signing.rs`,
dazu die Akzeptanz-Testmatrix `tests/liveness.rs` mit 21 simulierten
Validatoren (Fahrplan verlangt ≥ 20) — Leader-Ausfall über drei Runden,
wachsende Fristen, Sperre gegen konkurrierenden Block, byzantinische
Minderheit unter f < 1/3 (600 von 1401 nötigem Quorumgewicht),
Partition unter GST (nichts commitet) und über GST (alle commiten
denselben Block), Sperrtreue nach der Heilung, verzögerte Nachrichten,
Zustandsgleichheit aller 21 Knoten. Crate insgesamt **147 Tests grün**,
clippy mit `-D warnings` sauber.

**Grenze, bewusst offen gelassen:** Der Treiber deckt den Rundenwechsel
innerhalb einer Epoche ab. Ein Zertifikat wird gegen die übergebene
stimmberechtigte Menge geprüft; über eine Epochengrenze hinweg müsste
die Menge der Ursprungsepoche mitgeführt werden. Dokumentiert, nicht
implementiert — bislang kein Fahrplanpunkt.

### myl-scheduler v0.2.11 – 2026-08-18 (Fund A20: Epoche geht in den VRF-Seed)

**Gefunden vom neuen `stack`-Lauf des Testclients** — ein Beleg dafür,
dass komponentenübergreifende Durchläufe etwas anderes finden als
Unit-Tests.

`derive_epoch_seed(prev_block_hash, vrf_sk, epoch)` nahm die Epoche
entgegen, speicherte sie im `EpochSeed` — und ließ sie **nicht in den
VRF-Eingang einfließen**. Alpha war allein der Block-Hash. Zwei Folgen:

1. **Umetikettierung.** `verify_epoch_seed` prüfte Alpha ohne Epoche.
   Ein Seed für Epoche 42 galt unverändert als gültiger Seed für Epoche
   99, mit demselben Beweis. Empirisch bestätigt, bevor der Fix einging.
2. **Wiederholte Zuteilung.** Zwei Epochen mit demselben Vorgängerblock
   (Reorganisation, leere Epoche, Neustart aus einem Snapshot) hätten
   exakt dieselbe Pod-Bildung, Shard-Zuweisung und Stichprobenauswahl
   ergeben.

Der bestehende Test `derive_seed_different_epochs` behauptete das alte
Verhalten ausdrücklich als gewollt („Beta sollte gleich sein"). Es war
also eine bewusste Festlegung — aber eine, die die eigene
Verifikationsfunktion unterläuft.

Behoben: neues `seed_alpha()` mit
`"MYELITH_EPOCH_SEED_v1" ‖ prev_block_hash ‖ u64_le(epoch)`; Ableitung
und Verifikation nutzen dieselbe Bytefolge. Regressionstest
`umetikettierter_seed_wird_abgelehnt`. 58 → 60 Tests.
**Konsensrelevant** — jede abgeleitete Zuteilung verschiebt sich.


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


### v0.4.1 – 2026-08-18 (Audit-Block 4: kanonische Blocktypen)

**Fund A8 — der Block definierte eigene Fassungen der Protokolltypen.**
`block.rs` hatte eigene `PoiBundle`, `Challenge` und `Verdict` mit
anderen Feldern als die Typen, die die übrigen Komponenten tatsächlich
produzieren:

| block.rs (alt) | kanonisch |
|---|---|
| `PoiBundle { segment_id, commitment_hash, pod_id: [u8;32], signature: [u8;96] }` | `myl_types::PoIBundle { epoch, pod, segments_root, vtfe_claimed, aggregate_sig }` |
| `Challenge { segment_id, first_divergence, challenger, accused }` | `myl_types::Challenge` (mit beiden Pods und beiden Hashes) |
| `Verdict { segment_id, winner, loser, slash_amount }` | `myl_ledger::Verdict { segment_id, miner, checker, outcome }` |

Die Folge war eine stille Integrationslücke: `myl-pod` erzeugt das
Epochen-Aggregat `myl_types::PoIBundle` (Anhang A.1), aber
`Block::add_poi_bundle` nahm eine per-Segment-Struktur — der Pfad
Pod → Block war nie verdrahtet, obwohl beide Seiten als „vollständig"
geführt wurden. Ebenso hätte kein Verdict des Verifiers je gebucht
werden können. Rohe `[u8; 32]`/`[u8; 96]`-Felder sind durch die
Newtypes aus `myl-types` ersetzt — genau dafür gibt es SHARED_TYPES.

**Fund A10 — `myl-ledger` war als Abhängigkeit deklariert, aber nie
benutzt** (null Referenzen im Quelltext). Konsequenz: `EpochMeta` trug
keinen `state_root`. Ein Validator konnte nur prüfen, ob die Bytes des
Blocks gleich sind — nicht, ob der Vorschlagende die Zustandsübergänge
korrekt angewendet hat. Ein Leader hätte einen syntaktisch
einwandfreien Block mit falsch gebuchtem Slashing vorschlagen können.
`EpochMeta` hat jetzt `state_root: Hash`
(`LedgerState::commitment()`), und der Test
`state_root_geht_in_den_blockhash_ein` sichert, dass er in den Hash
eingeht, über den abgestimmt wird.

- 97 → 100 Tests.

### v0.4.0 / myl-scheduler v0.2.9 – 2026-08-18 (Audit-Block 3: BFT-Kryptografie)

**Fund A3 — das BFT-Protokoll enthielt keine Kryptografie.**
`Propose`, `Vote` und `Commit` hatten kein Signaturfeld, und `BftState`
kannte das Komitee nicht: der Zustandsautomat zählte Nachrichten, ohne
zu prüfen, wer sie geschickt hat. Ein einzelner Angreifer erreichte den
Threshold mit 15 erfundenen Miner-IDs. `BftError::InvalidSignature` war
als „(Placeholder)" deklariert und wurde nirgends zurückgegeben.

Behoben:
- Alle drei Nachrichtentypen tragen eine `BlsSignature`.
- Neuer Typ `VotingSet` bündelt, was die Runde zur Prüfung braucht:
  wer stimmberechtigt ist, mit welchem Schlüssel geprüft wird und mit
  welchem Gewicht die Stimme zählt.
- Jede Nachricht durchläuft vier Prüfungen in der Reihenfolge billig
  vor teuer: Runde → Mitgliedschaft → Duplikat → BLS-Signatur.
- Validatoren registrieren sich mit ihrem BLS-Public-Key; ein
  ungültiger Schlüssel wird bei der Registrierung abgelehnt statt
  später jede Signaturprüfung scheitern zu lassen.

**Fund A7 — Stimmgewicht war berechnet, aber nirgends angeschlossen.**
`voting_weight.rs` (297 Zeilen, getestet) wurde von keinem anderen Modul
aufgerufen. `receive_vote` zählte Köpfe, `select_committee` sortierte
rein nach Stake und nahm die ersten 28 — eine feste Rangliste ohne die
im Whitepaper (Kap. 3.5) genannte VRF-Rotation, also in jeder Epoche
dieselben 21 Adressen.

Behoben:
- Quorum ist `> 2/3` des **Stimmgewichts** statt der Nachrichtenzahl.
- `select_committee(registry, epoch, vrf_seed)` zieht gewichtet ohne
  Zurücklegen aus dem VRF-Epochenseed → Rotation **und** Kopplung an
  Stake und Arbeit.
- Validatoren führen eine `InferenceHistory` statt eines flachen
  Zählers; `record_work(miner, epoch, work)` speist sie.
- **Formeländerung (konsensrelevant, bitte bestätigen):**
  `stake × Arbeit` → `stake + stake · Arbeit / VTFE_UNIT`. Das reine
  Produkt gab jedem Validator ohne Arbeitshistorie Gewicht 0 — bei
  Genesis wäre kein Komitee wählbar gewesen, und wer bei 0 startet,
  wird nie gewählt und kann nie Arbeit nachweisen.

**Weitere Korrekturen im selben Block:**
- `BftState::new` gibt `Result` zurück — vorher `(committee_size - 1) / 3`
  mit usize-Underflow bei leerem Komitee.
- `select_leader` gibt `Option` zurück — vorher Division durch null bei
  leerer Producer-Liste.
- `apply_decay` rechnet in u128 mit Sättigung — vorher `value * 95` in
  u64: Panic im Debug-Build, stiller Umlauf im Release-Build, also
  je nach Build-Profil verschiedene Stimmgewichte.
- `SeedRng`/`deterministic_shuffle` nach `myl-types` verschoben und um
  `weighted_sample_without_replacement` ergänzt; `myl-scheduler` nutzt
  jetzt die geteilte Fassung.
- 63 → 97 Tests.

### v0.3.6 / myl-scheduler v0.2.8 – 2026-08-18 (Audit-Block 2: Konsens-Determinismus)

**Fund A4 — Double-Signing-Beweise waren wertlos und zugleich fälschbar.**
`SignedBlocksRegistry::register_signed_block()` erzeugte bei erkanntem
Double-Signing einen Beweis mit `signature_1 = signature_2 = [0u8; 96]`,
während `DoubleSignProof::validate()` verlangte, dass die Signaturen
verschieden sind — der Erkennungspfad konnte also **nie** einen
verwertbaren Beweis liefern. Umgekehrt prüfte `validate()` die Signaturen
nie gegen einen öffentlichen Schlüssel: jeder Beliebige hätte mit zwei
erfundenen Bytefolgen einen „gültigen" Beweis gegen jeden Validator
fabrizieren können. Beide Funktionen waren einzeln getestet, nie gemeinsam.

Behoben:
- Die Registry speichert die tatsächlich abgegebene BLS-Signatur mit
  (`HashMap<u64, (Hash, BlsSignature)>`) und liefert echte Beweise.
- `validate()` ist durch `verify(&BlsPublicKey)` ersetzt — es gibt keine
  Prüfung ohne Schlüssel mehr, damit dieselbe Lücke nicht wiederkehren kann.
  Geprüft werden: verschiedene Block-Hashes, verschiedene Signaturen und
  **beide BLS-Signaturen gegen den Schlüssel des Beschuldigten**.
- `signature_1/2` sind jetzt `BlsSignature` statt nackter `[u8; 96]`.
- Neues Modul `signing.rs`: kanonische, domain-getrennte Signierbotschaften
  für Propose/Vote/Commit (`MYELITH_BFT_*_v1 ‖ u64_le(round) ‖ block_hash`).
  Ohne Domain-Separation wäre eine Vote zugleich ein gültiger Commit.
- Regressionstest `erkannter_beweis_besteht_die_eigene_pruefung` plus Tests
  für erfundene, fremde, rundenfremde und typfremde Signaturen.
- 53 → 63 Tests.

**Fund A6 — Die Stichproben-Lotterie war nicht gleichverteilt.**
Der Fisher-Yates-Shuffle zog den Vertauschungsindex aus einem einzigen
Byte (`state[0] as usize % (i + 1)`). Messung bei 1 000 Segmenten und 2 %
Rate: Index 0 wurde mit dem **0,14-fachen**, Index 256 mit dem
**3,87-fachen** des Erwartungswerts gezogen — Spreizung Faktor ~28. Für
die Lotterie, die entscheidet, welche Arbeit auditiert wird, hing die
Prüfwahrscheinlichkeit damit am Segmentindex statt am Zufall. Zusätzlich
nutzte der XOR-Shift nur `state[0..8]`: **192 der 256 VRF-Seed-Bits
gingen nie ein**. Dieselbe fehlerhafte Funktion lag in **vier Kopien** in
`sampling.rs`, `redundancy.rs`, `shard_assignment.rs` und
`geo_clustering.rs`.

Behoben:
- Neues Modul `shuffle.rs` mit **einer** Implementierung für alle vier
  Verwendungen. RNG: SHA-256 im Zählermodus (`sha256(seed ‖ counter_le)`),
  alle 256 Seed-Bits gehen ein. Index-Wahl per Verwerfungsverfahren statt
  `% n` (exakte Gleichverteilung, Determinismus bleibt erhalten).
- Nachmessung: Spreizung **0,89× – 1,14×** (reine Stichprobenstreuung).
- Tests für Seed-Vollständigkeit, Gleichverteilung über 1 000 Positionen
  und das Fehlen einer Stufe an der alten 256er-Grenze.
- 56 → 66 Tests.
- **Konsensrelevant:** Die Zuteilung aller Epochen verschiebt sich. Da MYL
  nicht im Umlauf ist, ist das der richtige Zeitpunkt.

### myl-scheduler v0.2.7 – 2026-08-18 (Fix: Testbuild wiederhergestellt)
- **Fund A1:** `myl-scheduler` ließ sich seit dem Roundhouse-Check-Commit
  nicht mehr im Testmodus bauen (`error[E0433]: cannot find type MinerId`).
  Beim Beheben einer `unused import`-Warnung wurde `use myl_types::ids::MinerId`
  aus `shard_assignment.rs` entfernt — im Lib-Rumpf tatsächlich unbenutzt,
  im `#[cfg(test)] mod tests` aber gebraucht. `cargo build` blieb grün,
  `cargo test` brach ab.
- Fix: Import in den Test-Modul verschoben. 56 Tests grün (die in der
  Doku bereits behauptete Zahl).
- **Ursachenanalyse:** Der Fehler konnte unentdeckt nach `main` gelangen,
  weil die CI `myl-scheduler` überhaupt nicht baute. Siehe CI-Ausweitung
  im selben Patch (`.github/workflows/ci.yml`): jetzt laufen alle acht
  `myl-*`-Crates plus `INTEGER_LLM/pipeline`.

### v0.3.5 – 2026-08-17 (Phase 3: BFT-Blockproduktion)
- `myl-consensus`: Neuer Crate mit 5 Modulen für BFT-Blockproduktion:
  - Validator-Registrierung mit Stake-Minimum und Komiteewahl (12 Tests)
  - BFT-Protokoll mit Propose/Vote/Commit-Zyklus (9 Tests)
  - Block-Struktur mit Borsh-Serialisierung (9 Tests)
  - Stimmgewichts-Kopplung mit Decay-Faktor (13 Tests)
  - Double-Signing-Erkennung und Slashing (10 Tests)
- 53 neue Tests grün, insgesamt 109 Tests

### v0.2.6 – 2026-08-17 (Phase 2: Deterministischer Epochen-Scheduler)
- `myl-scheduler`: Neuer Crate mit 6 Modulen für den deterministischen
  Epochen-Scheduler (Whitepaper Anhang A.2):
  - VRF-Seed-Ableitung aus finalisiertem Block (7 Tests)
  - Miner-Filterung nach Hardware-Klasse und Registrierungsschluss (11 Tests)
  - Geo-Clustering unter Latenz-Constraint (8 Tests)
  - Shard-Zuweisung mit Fisher-Yates (9 Tests)
  - Redundanz-Zuteilung (zonendivers, disjunkt) (9 Tests)
  - Stichproben-Lotterie für Checker (12 Tests)
- Alle Schritte sind deterministisch und von jedem Node unabhängig
  nachrechenbar. 56 Tests grün insgesamt.

### v0.1.1–v0.1.5 – 2026-08-13 (Phase 1)
- `myl-ledger`: Kontenmodell mit deterministischer BTreeMap-Ordnung,
  Zustandsübergänge nach Anhang A.5 (burn→mint_credits mit
  floor-Division, apply_verdict mit Slash/Kopfgeld als Ganzzahl-Brüche,
  credit_spend mit FIFO-Verbrauch nach Verfall), atomare Übergänge
  (Prüfphase vor Änderungsphase), State-Commitment via SHA-256 über
  kanonischem Borsh.
- Akzeptanzkriterium erfüllt: Replay derselben Übergangsfolge liefert
  auf zwei unabhängigen Läufen bitgleiche Commitments (23 Tests grün,
  keine Warnungen).
- Verdict-Minimaltyp als dokumentierte Zwischenlösung bis zur
  VERIFICATION-Definition; vTFE-Rückbuchung als Phase-4-Hook im
  `VerdictEffect`.
