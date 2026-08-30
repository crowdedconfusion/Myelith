# consensus (`myl-consensus` + `myl-ledger` + `myl-scheduler`)

> **Version:** 0.20.2 (`myl-consensus` 0.17.2, `myl-scheduler` 0.4.0,
> `myl-ledger` 0.5.0)
> **Datum:** 2026-08-29
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
> **380 Tests grün** (258 `myl-consensus`, 68 `myl-scheduler`,
> 54 `myl-ledger`).
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

### v0.20.2 (`myl-consensus` 0.17.2) – 2026-08-30 (die Registrierung, die es schon gab)

Als offener Punkt stand notiert: „es fehlt eine Registrierung Miner zu
Schlüssel", ohne die die Anfechtungsprüfung im echten Netz nur
Validatoren erfasst.

⚑ **Für den Streitpfad fehlt sie nicht.** Ein Herausforderer ist
Mitglied des redundanten Pods, und `PodMembership` führt die Schlüssel
ihrer Mitglieder ohnehin mit. Was fehlte, war ein Zugriff auf einen
**einzelnen**: `pubkeys()` liefert alle für die Aggregat-Prüfung, ein
Anfechtungsbeleg braucht einen. `pubkey(&MinerId)` schließt das.

Eine zweite, globale Registrierung wäre eine zweite Quelle für dieselbe
Zuordnung gewesen.

**Was damit nicht gelöst ist**, und das steht auch so im Code: die
Prüfung im Gossip-Pfad. Dort kennt der Knoten die Pod-Zuteilung eines
fremden Segments nicht und darf sie nicht raten; ein unbekannter
Absender geht weiterhin durch, und geurteilt wird erst beim Schlachten.

### v0.20.1 (`myl-consensus` 0.17.1) – 2026-08-29 (eine Abschrift weniger)

`Transaktion::absender_adresse` rechnete `sha256(pubkey)` selbst. Diese
Regel stand am 29. August in sechs Dateien ausgeschrieben, jede für sich
richtig; sie steht jetzt einmal in `myl_types` und wird hier gerufen.
Der Schaden einer solchen Verdopplung entsteht beim Ändern, nicht beim
Schreiben (SHARED_TYPES v0.11.0).

Der Gegentest im selben Modul rechnet sie weiterhin von Hand, und das
mit Absicht: Ein Test, der über denselben Helfer rechnet, den er prüfen
soll, prüft sich selbst.

### v0.20.0 (`myl-consensus` 0.17.0) – 2026-08-29 (ein Quorumsbeleg gilt ohne Rücksicht auf die Runde)

### ⚑ Fund 67 geschlossen: Wer allein vorauseilt, kommt jetzt zurück

Ein Knoten, dessen Frist ablief, bevor die anderen ihre Runde begonnen
hatten, stand danach dauerhaft vor dem Netz. Aufgezeichnet am 26. August
über fünf Prozesse: Der erste hatte nach 1 ms ein volles Mesh und begann
Runde 0, die anderen vier begannen ihre erst 522 ms später, seine
Vote-Frist von 500 ms lief vorher ab. Er stand am Ende bei Runde 5,
während die vier Runde 0 längst commitet hatten.

**Der Grund lag eine Ebene tiefer, als er aussah.** `receive_propose`,
`receive_vote` und `receive_commit` verwerfen jede Nachricht aus einer
anderen Runde. Für einzelne Nachrichten ist das richtig. Für den
Vorausgeeilten heißt es: **Er verwirft genau die Nachrichten, die
belegen, dass er der Irrende ist.**

**Was hier zuvor als Lösung stand, trug nicht.** Notiert war, der
Rückweg hänge an der Kettenpersistenz. Beim Nachlesen fiel auf, dass ein
Commit bis heute keinen Block in die Kette legt und auch keinen
veröffentlicht, er schreibt eine Protokollzeile. Über die Kette wäre
nichts zurückgekommen, gleich wie lange man wartete.

**Gebaut ist stattdessen ein `Commitzertifikat`:** Runde, Block,
Unterzeichner in strenger Ordnung, BLS-Aggregat über
`commit_message`. Es belegt eine Entscheidung und ist deshalb **nicht an
die Runde des Empfängers gebunden**: Die Rundennummer ist ein örtliches
Mittel gegen Stillstand, ein Quorumsbeleg ist eine Tatsache über das
Netz. `RoundDriver::apply_commitzertifikat` übernimmt ihn aus jeder
Runde. Der Knoten springt dabei **nicht** in die alte Runde zurück, er
nimmt ihr Ergebnis an: Eine Runde zurückzusetzen wäre angreifbar, denn
dann zöge altes Nachrichtenmaterial einen Knoten beliebig weit nach
hinten.

Das ist nicht eigens erfunden, sondern der übliche Weg: In Tendermint
trägt der commitete Block seine Commit-Signaturen mit sich und wird über
die Blocksynchronisation unabhängig vom Konsens-Reaktor übernommen, in
QBFT stehen die Commit-Siegel im Blockkopf, in HotStuff gilt ein
Quorum-Zertifikat für sich, ohne dass der Empfänger in der passenden
Sicht säße.

**Der Beleg geht nur hinaus, wenn ihn jemand braucht.** Der
naheliegende Weg, ihn nach jedem Commit zu veröffentlichen, kostet bei
`n` Validatoren `n` Nachrichten je Entscheidung, immer, auch wenn alle
dieselben Commits ohnehin gesehen haben. Stattdessen ist die Abweisung
`WrongRound` das Signal: Wer in einer fremden Runde steht, sendet
Nachrichten dieser Runde und gibt sich damit selbst zu erkennen. Im
Normalbetrieb kostet der Rückweg nichts. Bedient wird nur, wer
stimmberechtigt ist, und jeder genau einmal, sonst löst ein Beliebiger
mit erfundenen Bytes den Versand aus, so oft er will: Die Rundenprüfung
steht im Automaten **vor** der Signaturprüfung, ist also billig zu
erreichen.

### ⚑ Zwei Quoren für zwei Blöcke sind ein Befund, keine Störung

`RoundError::ConflictingCommit` ist eigens dafür da. Wer bereits einen
anderen Block commitet hat und einen gültigen Beleg für einen zweiten
sieht, sieht den Bruch der Mehrheitsannahme. Unter einem Sammelposten
gebucht wäre das unauffindbar; im Betriebsprotokoll des Knotens heißt
diese Zeile `gabelung` und gilt ausdrücklich nicht als harmlos.

Geprüft wird **vor** dem Urteil über den Widerspruch, sonst löst jeder
mit erfundenen Bytes einen Sicherheitsalarm aus. Umgekehrt wird ein
Beleg über die **schon getroffene** Entscheidung gar nicht erst geprüft,
sonst kostete jede überzählige Kopie eine Aggregat-Verifikation. Beide
Reihenfolgen sind eigens getestet, eine davon mit einem absichtlich
kaputten Aggregat, das durchgehen **muss**.

### Der Prüfkern liegt jetzt einmal da

`PolkaCertificate::verify` und `Commitzertifikat::verify` teilen sich
`pruefe_aggregat`. Ein Zertifikat ist so viel wert wie seine schwächste
Prüfung; zwei Abschriften desselben Ablaufs driften auseinander, sobald
eine nachgebessert wird, und die Lücke säße dann in der Art, die gerade
niemand ansieht. Dass ein Polka sich nicht als Commit-Beleg ausgeben
lässt, hängt allein an den getrennten Präfixen der Signierbotschaft und
ist als eigener Test festgehalten.

### Kleinigkeiten

- `Konsensnachricht::Commitzertifikat` ist die **fünfte** Marke, hinten
  angehängt: Die Kodierung der vier bisherigen bleibt Byte für Byte
  dieselbe, keine erzeugte Signatur wird ungültig.
- `Konsensnachricht::absender()` gibt jetzt `Option<MinerId>` zurück. Ein
  Aggregat hat keinen Absender, es hat Unterzeichner. Einen davon
  herauszugreifen ergäbe eine zweite, erfundene Auskunft neben der wahren
  Liste.
- `Konsensnachricht::runde()` trägt eine Warnung: Für den Beleg ist das
  die Runde, die er **bezeugt**, nicht eine, in der der Empfänger stehen
  müsste. Wer danach filtert, wirft die Nachricht weg, die den
  Vorausgeeilten zurückholt.
- Größe auf der Leitung gemessen, nicht geschätzt: 301 B bei 5, 813 B bei
  21, 4237 B bei 128 Unterzeichnern. Die Herleitung von
  `MAX_CONSENSUS_BYTES` verlangt das von jedem, der eine Nachricht
  anschließt.

### v0.19.0 (`myl-consensus` 0.16.0, `myl-ledger` 0.5.0) – 2026-08-28 (eine Transaktion hat jetzt einen Absender)

### ⚑ Fund 85: Eine Transaktion trug keine Unterschrift

`Transaction::Burn(BurnTx { sender, amount })` nannte den Absender als
**Feld**, und nichts verglich ihn mit dem, der die Transaktion
eingereicht hatte. Jeder konnte im Namen jedes Kontos anweisen.

**Warum es niemandem auffiel:** Es gab genau eine Anweisung, und die
*zerstört* Geld. Ein fremder Burn ist Sachbeschädigung, kein Diebstahl,
und ein Testnetz ohne Wert merkt den Unterschied nicht. ⚑ **Eine
Überweisung darauf zu setzen, hätte daraus Diebstahl gemacht**, und zwar
still: Der Code hätte sich nicht geändert, nur die Anweisung daneben.

**Jetzt:** `Transaktion { absender, nonce, anweisung, signatur }`. Der
Absender ist der **öffentliche Schlüssel**, das belastete Konto folgt
daraus als `SHA-256`. Es gibt kein Absenderfeld mehr, das sich abweichend
füllen ließe.

**Die Kennung der Kette steht in den unterschriebenen Bytes und nicht in
der Transaktion.** Eine Transaktion für Kette A scheitert damit auf Kette
B an der Prüfung, ohne dass 32 Bytes durch jedes Netz wandern. Ohne die
Bindung wäre jede Testnetz-Überweisung auf dem Hauptnetz gültig.

**Eine Nummer je Konto gegen Wiedereinspielung**, streng aufsteigend ohne
Lücken. Eine Fensterlogik erlaubte Umordnung, und zwei Knoten mit
verschiedener Reihenfolge kämen zu verschiedenen Zuständen. ⚑ **Sie wird
auch dann verbraucht, wenn die Anweisung danach scheitert** — sonst wäre
eine ungedeckte Überweisung unverändert gültig und beliebig oft
einreichbar.

### Wo die Unterschrift geprüft wird, und warum nicht woanders

**Beim Anwenden, nicht bei der Aufnahme in den Mempool.** Ein Block kommt
über Gossip und sieht den Mempool nie; läge die Prüfung dort, könnte ein
Leader eine unsignierte Anweisung in einen Block schreiben, und die
ehrlichen Knoten wendeten sie an. Erzeuger und Übernehmer durchlaufen
dieselbe Funktion und überspringen deshalb dasselbe.

### Die Überweisung (Fund 83)

`transfer` bewegt nur `balance`, nicht gestaktes MYL. ⚑ **Die Überweisung
an sich selbst wird abgewiesen**, und nicht aus Ordnungsliebe: Der
naheliegende Weg, eine Überweisung zu schreiben, ist „vom Absender
abziehen, beim Empfänger addieren", und bei gleichem Konto verdoppelt das
den Betrag, wenn der Absenderstand vorher gelesen wurde. **Ein
abgewiesener Sonderfall kann nicht falsch gerechnet werden.**

### ⚑ Und zwei Löcher, die erst beim Verdrahten sichtbar wurden

Nichts band `kontrakt.inhaber` an den, der die Eröffnung einreicht, und
nichts band `vorhaben.handelnder` an den, der die Ausgabe einreicht.
`pruefe` vergleicht den *im Vorhaben genannten* Handelnden mit dem
Agenten des Kontrakts; wer ihn wirklich geschickt hat, steht dort nicht.
**Ein Fremder hätte den echten Agenten ins Feld geschrieben und unter
dessen Kontrakt gezahlt.**

Beide Prüfungen stehen jetzt **im Übergang** und nicht im Aufrufer, damit
kein zweiter Aufrufer sie vergessen kann. Ein Test führt alle drei Wege
vor, auf denen jemand unter fremdem Namen handeln wollte.

**19 neue Tests**, `myl-ledger` 54 und `myl-consensus` 246.

### v0.18.0 (`myl-ledger` 0.4.0) – 2026-08-28 (der Kontrakt wird durchgesetzt)

**Session-Kontrakte stehen im Ledger-Zustand** und gehen in die
Zustandsverpflichtung ein (Whitepaper Kap. 8.2). Vier Übergänge:
eröffnen, widerrufen, unter dem Kontrakt Credits ausgeben, nach Frist
aufräumen.

⚑ **Das ist die Stelle, an der ein Kontrakt etwas bedeutet.** Ein
Client, der die Grenzen selbst prüft, prüft sie freiwillig; hier prüft
sie jeder Knoten, bevor er den Zustand fortschreibt. Der Kontrakt liegt
deshalb **im Zustand** und wird nicht von irgendwem vorgelegt: Genau
darin besteht der Unterschied zwischen „vom Konsens durchgesetzt" und
„vom Client behauptet".

**Belastet wird das Konto des Inhabers**, nicht das des Agenten. Der
Agent ist ein Schlüssel mit einer Vollmacht.

⚑ **Der Verbrauchszähler wächst erst, wenn die Credits geflossen sind.**
Ein Budget, das an einer fehlgeschlagenen Ausgabe schrumpfte, wäre über
wiederholte Fehlschläge leerzuräumen. Ein Test hält das fest: Der
Kontrakt erlaubt 1000, das Konto trägt 50, und nach dem Fehlschlag steht
der Zähler weiter auf null.

**Der Widerruf steht nicht im Whitepaper und gehört trotzdem hierher.**
Ohne ihn ist das Zeitfenster das einzige Mittel gegen einen Agenten, der
sich falsch verhält, und dieses Mittel heißt warten. Nur der Inhaber,
und zweimal widerrufen ist kein Fehler: Zwei Blöcke mit demselben
Widerruf dürfen nicht dazu führen, dass der zweite ungültig wird.

⚑ **Eine Aufbewahrungsfrist für Sessions**, aus demselben Grund wie das
Verstoßfenster und aus einem dringenderen: Kontrakte legt jeder Nutzer
selbst an. Ohne Frist wüchse der Konsenszustand mit jedem jemals
eröffneten Kontrakt, und die Größe hinge an einer Eingabe, die ein
Angreifer bestimmt. **Aufgeräumt wird in einem Übergang, nicht beim
Lesen** — sonst hinge der Zustand daran, wer wann gelesen hat.

### ⚑ Und eine Lücke, die dabei zum Vorschein kam

Das Ledger kennt `apply_verdict`, `burn_to_credits` und `credit_spend`.
**Eine Überweisung von Konto zu Konto gibt es nicht.** Kap. 8.2 setzt
sie voraus, sowohl für das MYL-Budget als auch für die Empfängerliste.
Solange sie fehlt, lehnt der Kontrakt jedes MYL-Vorhaben ab, statt es
durchzulassen.

**11 neue Tests**, `myl-ledger` zusammen 49.

### v0.17.0 (`myl-consensus` 0.15.0) – 2026-08-28 (der zweite Schlüssel je Validator)

**`Validator` trägt ein Feld für einen quantensicheren Schlüssel**,
heute `None` und nur `None`, denn ein zweites Verfahren gibt es nicht.

**Warum es trotzdem jetzt kommt:** Ein Schalter für den Wechsel des
Signaturverfahrens funktioniert nur, wenn alle Validatoren ihren neuen
Schlüssel **vorher** veröffentlicht haben. Solange das Feld fehlt, kann
niemand anfangen. Vor dem Genesis-Block ist es eine Zeile, danach eine
Kettenmigration.

⚑ **`alle_bereit_fuer` ist scharf: alle, nicht die meisten.** Ein
einziger Validator ohne zweiten Schlüssel verliert mit dem Schritt auf
„nur quantensicher" seine Stimme, und ein Netz, das sich seiner
Validatoren nach Gutdünken entledigt, ist kein Konsens mehr. Der Schritt
wartet, bis der letzte bereit ist, oder das Netz entfernt ihn vorher auf
dem geordneten Weg. `noch_nicht_bereit` nennt die Knoten beim Namen,
denn ein „nein" ohne Namen hilft niemandem.

**Diese Prüfung liegt hier und nicht in der Governance-Registry**, und
das ist eine Schnittstelle: Die Registry kennt Parameter, nicht
Validatoren. Dieselbe Trennung wie beim Stimmgewicht, das umgekehrt von
hier nach GOVERNANCE geht.

**Vier Gegenproben**, darunter: Ein Schlüssel des **klassischen**
Verfahrens im Post-Quantum-Feld macht niemanden bereit. Ohne diese
Prüfung ließe sich der Schalter mit einem BLS-Schlüssel im falschen Feld
umlegen.

### myl-consensus v0.14.0 – 2026-08-27 (Höhe und Epoche im Blockkopf)

**`EpochMeta` heißt `BlockHeader` und trägt ein Höhenfeld.** Der alte
Name war der Fehler: Ein Kopf ohne Höhe zwingt jeden, der eine Höhe
braucht, sich eine zu suchen — und die Probekette fand sie im
Epochenfeld. Das trägt, solange eine Epoche ein Block ist, und ist
still falsch, sobald es das nicht mehr ist. Jede Frist „je Epoche"
bedeutete damit „je Block".

- `height` — die Stellung in der Kette, um genau eins wachsend.
- `epoch` — folgt aus der Höhe (`epoche_fuer_hoehe`), steht trotzdem im
  Kopf, damit ein Block für sich lesbar bleibt, und wird beim Übernehmen
  dagegen geprüft. Ein mitgeführter Wert, den niemand nachrechnet, ist
  ein Feld, das jeder setzen darf.

**`BLOECKE_JE_EPOCHE = 1800`**, also Epochenlänge durch Blockzeit
(3600 s / 2 s). Beide sind Governance-Parameter, und ein Test in
`myl-governance` hält die Konstante gegen sie — dieselbe Bauart wie bei
der Streitfrist (⚑ Fund 50).

⚑ **Warum die Zahl trotzdem eine Konstante ist und keine Abfrage der
Registry:** Die Zuordnung Höhe → Epoche geht in die **Blockprüfung**
ein. Eine Blockprüfung, die einen abstimmbaren Wert liest, macht die
Gültigkeit eines Blocks von einem Zustand abhängig, der sich ändern
kann, während der Block schon in der Kette steht. Wer die Epochenlänge
ändern will, ändert damit einen Konsensvertrag, keinen Parameter.

⚑ **Und warum die Epoche aus der Höhe folgt und nicht aus der Uhr:**
Eine Zuordnung über Zeitstempel wäre nicht deterministisch. Zwei
ehrliche Knoten mit leicht verschiedenen Uhren ordneten denselben Block
verschiedenen Epochen zu, und damit fiele die Zustandswurzel
auseinander. **Was das kostet, gehört dazugesagt:** Stehen die Blöcke
still, stehen auch die Epochen still — Prägung, EMA und Fristen hängen
am Fortschritt der Kette und nicht an der Wanduhr.

### myl-ledger v0.3.0 – 2026-08-27 (Verstoßhistorie je Konto)

**Ein neues Konsensfeld.** `AccountState` trägt eine Verstoßhistorie:
wann dieses Konto geschlachtet wurde, je Epoche gezählt. Sie geht in
`commitment()` ein, denn zwei Knoten mit verschiedenen Vorgeschichten
schlachten beim nächsten Urteil verschieden hoch und laufen damit
auseinander.

**`apply_verdict` vermerkt den Verstoß selbst**, beim Schuldigen und im
selben Übergang. Ein Urteil, das gebucht wird, ohne gezählt zu werden,
macht die Staffelung zu einer Absichtserklärung — der nächste Verstoß
wäre wieder der erste. Weil der Vermerk im Übergang steht, kann er nicht
vergessen werden; ein **abgelehntes** Urteil zählt dagegen nicht, sonst
wäre „ohne Deckung anklagen" ein Weg, die Vorgeschichte eines anderen zu
füllen.

**`VerdictEffect` nennt jetzt `vorverstoesse`**, den Stand **vor** dem
Urteil. Der Satz der Slashing-Matrix hängt daran: `0` ist der erste
Verstoß. Wer den Wert nach dem Buchen abfragte, bekäme einen zu hohen und
schlüge die nächste Stufe zu früh auf.

**Drei Eigenschaften, die zusammengehören und je einen Test haben:**

- **Die Historie wächst nicht.** Gekürzt wird beim Vermerken auf
  `VERSTOSS_FENSTER` Epochen; nach jedem Vermerk stehen höchstens so
  viele Einträge da. Ohne diese Grenze hinge die Größe des
  Konsenszustands daran, wie oft jemand auffällt — eine Größe, die ein
  Angreifer selbst bestimmt.
- **Lesen verändert nichts.** `verstoesse_im_fenster` räumt nicht auf.
  Täte es das, hinge der Zustand daran, **wer wann gelesen hat**, und
  zwei Knoten mit verschiedener Lesereihenfolge kämen zu verschiedenen
  Verpflichtungen. Gekürzt wird ausschließlich im Übergang, dasselbe
  Muster wie bei den verfallenen Credits.
- **Ein Fenster über die Epoche 0 hinaus läuft nicht um.** Ohne
  sättigende Subtraktion wäre die Untergrenze `u64::MAX` und die
  Vorgeschichte in den ersten Epochen des Netzes leer — die Staffelung
  wäre genau dann abgeschaltet, wenn sie am ehesten gebraucht wird.

⚑ **Das Zustandsformat ändert sich damit.** Ein Ledger-Commitment aus
der Zeit davor stimmt nicht mehr mit einem von heute überein. Das ist
folgenlos, solange keine Kette daran hängt, die nicht neu gerechnet
werden kann: Der Probelauf ist Wegwerfware, und `myl-node` speichert
Blöcke und rechnet jede Zustandswurzel beim Wiederanlauf neu.

### myl-scheduler v0.4.0 – 2026-08-27 (`assign_redundant_pods` nennt den Grund)

*Nachgetragen am 2026-08-27: Der Eintrag fehlte, obwohl die Änderung am
selben Tag committet wurde. Die Kopfzeile dieser Datei führte weiter
`myl-scheduler 0.3.0`.*

Die Funktion gab bei fehlenden Metadaten einen **leeren Vektor** zurück,
und der sah aus wie „nichts angefragt". Jetzt liefert sie ein `Result`:
`ZuWenigPods` (weniger als zwei Cluster) oder `KeinGueltigesPaar`
(Cluster vorhanden, aber kein Paar disjunkt und zonendivers zugleich).
Fail-closed bleibt die Richtung, nur nennt sie den Grund — die beiden
Fälle sind verschiedene Befunde, und die Gegenmaßnahmen sind es auch.
**Null Segmente bleiben `Ok` mit leerer Liste:** Wer nichts verlangt,
bekommt nichts, und das ist kein Scheitern.

### myl-scheduler v0.3.0 – 2026-08-26 (⚑ Entscheidung D3: ein Miner je Shard)

`assign_shards` legte bis dahin **mehrere** Miner in jeden Shard
(gemessen: sechs Miner auf vier Shards ergaben `[2, 2, 1, 1]`). Das
widersprach drei anderen Stellen:

| Quelle | Aussage |
|---|---|
| Anhang A.2 | `cfg: &ShardConfig, // k Shards, **Pod-Größe k+2**` |
| Kap. 6.8, `myl_pod::PodBesetzung` | ein Miner je Position, dazu zwei in Reserve |
| `README/Glossar.md`, Eintrag *Shard* | „den ein **einzelner** Miner im Speicher hält" |

**Jede Seite war für sich stimmig und vollständig getestet.** Genau
deshalb konnte der Widerspruch bestehen: Niemand rechnete ihn nach, weil
niemand beide Seiten zugleich brauchte. Aufgefallen beim Verdrahten von
COMPUTE_PIPELINE 3.3.

**Entschieden am 2026-08-26:** Der Code richtet sich nach dem Papier.
`Shard` trägt **einen** Miner, `Pod` bekommt ein Feld `reserve`, und
`assign_pods` liefert eine [`Zuteilung`] aus Pods **und** den Minern, die
in keinen vollständigen Pod passten.

**Ein Cluster liefert so viele Pods, wie hineinpassen.** Zwölf Miner bei
`k = 4` ergeben zwei Pods, nicht einen überfüllten: Mehr Miner heißt mehr
Kapazität, nicht mehr Belegung je Position. Das ist die Lesart von
Anhang A.2 Schritt 2 („Pods so bilden") und Schritt 3 („Fisher-Yates
**innerhalb** des Pods").

⚑ **Zwei Dinge fielen dabei zusätzlich auf.**

**Erstens: `pods_are_disjoint` sah die Reserve nicht.** Die Prüfung war
vollständig, solange ein Pod keine getrennte Reserve hatte; seit D3 wäre
sie es nicht mehr. **Stünde dieselbe Maschine in der Reserve beider Pods
eines Redundanzpaars**, übernähme sie bei einem Ausfall auf beiden Seiten,
und Stufe 1 der Verifikation verglände zwei Ergebnisse derselben
Maschine. Behoben über `Pod::mitglieder`, zwei Regressionstests.

**Zweitens: der Shuffle-Seed war je Pod derselbe.**
`deterministic_shuffle` erzeugt zu einem Seed und einer Länge immer
dieselbe Permutation; mit dem blanken Epochenseed landete das dritte
Mitglied jedes gleich großen Pods auf derselben Shard-Position. Wer seine
Stellung in der Clusterreihenfolge beeinflussen kann, wüsste damit seine
Position im Voraus, und die Shard-Zuweisung soll gerade **nicht**
vorhersagbar sein (Kap. 4.3). Jetzt `sha256("MYELITH_POD_SHUFFLE_v1" ‖
seed ‖ pod_index)`.

**Das Whitepaper braucht keine Änderung:** Der Code kommt zu ihm, nicht
umgekehrt.

### myl-consensus v0.13.0 – 2026-08-26 (das Zertifikat reist mit)

`PolkaCertificate` bekommt Borsh-Ableitungen und `Konsensnachricht` eine
vierte Marke `ProposeMitPolka`. **Additiv:** Die Kodierung des einfachen
Propose bleibt Byte für Byte dieselbe, und keine zuvor erzeugte Signatur
wird ungültig. Dieselbe Begründung, aus der `DST_PROPOSE_POL` seinerzeit
ein eigenes Präfix bekam statt einer Erweiterung.

⚑ **Fund 66: Die Signatur deckte die `valid_round` nicht ab.**
`DST_PROPOSE_POL` und `propose_pol_message` existieren seit v0.5.0, sind
in ihrem Doc-Kommentar als notwendig begründet, und **wurden von nichts
aufgerufen**. `RoundDriver::receive_propose` nahm das Zertifikat entgegen
und ließ die Signatur weiterhin gegen `propose_message` prüfen.

**Was möglich war:** Ein Abhörer nimmt einen ehrlichen Propose für Block
B und hängt ein **anderes** gültiges Zertifikat für denselben Block an.
Beides prüft durch, denn `cert.verify` steht für sich und die Signatur
deckt das Zertifikat nicht. Zwei Nachrichten mit derselben Aussage,
verschiedenen Nachrichten-Ids und beide gültig; der Leader kann für keine
von beiden zur Verantwortung gezogen werden, und das trifft den
Double-Signing-Beweis.

**Dieselbe Klasse wie Audit-Punkt A10:** ein Schutz, den ein Leser für
vorhanden hält, weil er dasteht.

**Der Beleg lag im eigenen Test.** Der einzige bestehende Test, der den
Zertifikatspfad benutzte, signierte mit `propose_message` und kam durch.
Er schlug nach der Behebung fehl, und genau das war der Nachweis.

Behoben mit `BftState::receive_propose_mit_polka`; drei Tests halten
beide Richtungen und die veränderte `valid_round` fest.

**Die Größe ist jetzt gemessen statt gerechnet:** Propose 169 Bytes,
Propose mit Zertifikat 469 (5 Unterzeichner), 981 (21) und 4405 (128).
Die Topic-Grenze von 8 KiB trägt damit auch das größte plausible
Komitee.

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
vorgehalten. Damit ist **Phase 4 abgeschlossen und CONSENSUS
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
behoben. Umgesetzt am 2026-08-18 in `myl-types`: `BlsSecretKey::
prove_possession` und `BlsPublicKey::verify_possession`, verlangt von
`ValidatorRegistry::register`.

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
Validatoren (verlangt sind ≥ 20) — Leader-Ausfall über drei Runden,
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
implementiert, bislang ohne eigenen Punkt.

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
