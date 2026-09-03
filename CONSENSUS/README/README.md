# consensus (`myl-consensus` + `myl-ledger` + `myl-scheduler`)

> **Version:** 0.40.0 (`myl-consensus` 0.29.0, `myl-scheduler` 0.10.0,
> `myl-ledger` 0.17.0)
> **Datum:** 2026-09-03
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
> **448 Tests grün** (269 `myl-consensus`, 86 `myl-scheduler`,
> 93 `myl-ledger`), über alle Testbinaries gezählt.
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

### v0.40.0 – 2026-09-04 (⚑ Fund 167: das Auszahlungskonto bekommt einen Weg)

`Anweisung::AuszahlungskontoEintragen { kennung, konto }`, angehängt
(Borsh kodiert den Index, eine eingeschobene Variante verschöbe jede
dahinter).

`myl_ledger::transitions::auszahlungskonto_eintragen` stand seit dem
2026-09-01 fertig da, mit Berechtigungsregel und Tests, und **keine
`Anweisung` trug sie**. **Ohne Eintrag kein Anteil**, also hätte auf
einem echten Netz kein einziger Miner etwas bekommen, ohne dass
irgendwo ein Fehler entstanden wäre.

⚑ **Die Kennung ist ein Feld, anders als bei `MinerAnmelden`.** Dort
folgt sie aus dem Absender, weil nur der Miner selbst darf; hier gehört
jede weitere Eintragung dem eingetragenen **kalten Konto**, und das ist
gerade nicht die Kennung.

### v0.39.0 – 2026-09-03 (⚑ Fund 160: eine gerechnete Anfrage wird jetzt wirklich abgebucht)

**Der Projektinhaber fragte, ob die Umsetzung von MYL in Credits samt
Burn schon durchgespielt ist.** `burn_to_credits` war verdrahtet;
**`sitzung_ausgeben`, der Übergang, der Credits nach einer gerechneten
Anfrage abbucht, hatte ausserhalb der eigenen Tests keinen Aufrufer.**
Ein Nutzer konnte unbegrenzt fragen, ohne dass sein Budget sank.

⚑ **`sitzung_ausgeben` akzeptiert jetzt eine Vollmacht als
Autorisierung.** Ein Harness hält einen Bearer-Token und keinen
Schlüssel; es kann keine Kettentransaktion signieren. Der Betreiber
reicht ein, die Kette prüft die Vollmacht des Agenten
(`myl_types::vollmacht`, aus `myl-gateway` hierher gezogen, weil die
Kette sie prüfen muss und `myl-ledger` `myl-gateway` nicht kennen
darf).

⚑ **Und ein Riegel gegen die zweite Abbuchung.** `Vorhaben` trägt jetzt
eine `nummer`, die über die zuletzt gebuchte steigen muss
(`Sitzungszustand::hoechste_abrechnung`). Der Transaktionsnonce schützt
nur vor der Wiederholung *derselben* Transaktion; solange nur der Agent
selbst einreichen konnte, schadete eine zweite Einreichung ihm selbst.
**Seit ein Fremder mit Vollmacht einreicht, ist das ein Angriff auf das
Budget des Nutzers.**

**Verdrahtet bis zum Ende:** `myl-node`s Rechenweg baut nach jeder
echten Antwort eine `SitzungAusgeben`-Anweisung (Betrag = erzeugte
Token, mindestens eins), signiert sie mit dem eigenen Kettenschlüssel
und verbreitet sie. Ein Test über den ganzen Weg (Anfrage → Rechnen →
Kanal → Signatur → Kettenzustand) beweist den Verbrauch, samt
Gegenprobe auf die doppelte Buchung.

### v0.38.0 – 2026-09-03 (⚑ der Einsatz bekommt einen Weg in den Zustand, Punkt B11)

⚑ **Fund 145: `staked` stand seit Langem im Zustand, und niemand
schrieb es.** Der ganze wirtschaftliche Sicherheitsbau hing daran:
`S_min = g/p²`, das Stimmgewicht, die Slashing-Staffelung, das
Kopfgeld. Keine der acht Anweisungen setzte einen Einsatz, also war
`staked` im Betrieb **immer null**, also schlachtete `apply_verdict`
immer null, also hatte `MindestStake` nichts zu begrenzen.

**Drei neue Anweisungen:** `EinsatzHinterlegen`, `EinsatzKuendigen`,
`EinsatzAbholen`.

⚑ **Die Sperrfrist ist hergeleitet, nicht gewählt.** Ein Einsatz, den
man sofort abziehen kann, ist keiner: Wer falsch rechnet, zöge ab, bevor
das Urteil da ist. Also muss die Frist mindestens so lang sein wie das
Fenster, in dem noch ein Urteil kommen kann, und das ist die
**Streitfrist**: 168 Epochen, sieben Tage bei Stunden-Epochen.

⚑ **Und die Frist allein genügt nicht.** Wer kündigt, hätte den Betrag
aus `staked` heraus und damit aus der Schlachtmasse, obwohl er noch
haftet. `apply_verdict` zählt das Gekündigte deshalb **mit** und nimmt
es in der Reihenfolge der Freigabe, also die Kündigung zuerst, die der
Auszahlung am nächsten ist: die Richtung, die einem Fliehenden zuerst
nimmt, was er zu retten versucht. **Die Frist verschiebt die Auszahlung,
sie beendet die Haftung nicht.**

**Gegen Zustandswachstum geschlüsselt.** `gekuendigt` ist eine Karte
nach Freigabe-Epoche, nicht eine Liste: Kündigungen derselben Epoche
werden zusammengelegt, es gibt also höchstens einen Eintrag je Epoche
und nie mehr als 169. Eine Liste wäre unbegrenzt gewachsen, und das ist
die Klasse von Fund 144.

⚑ **Das Abholen ist eine eigene Anweisung**, weil das Gegenteil hiesse,
jedes Konto in jeder Epoche anzufassen: eine Arbeit in der Grösse des
Netzes für einen Vorgang, der einzelne betrifft.

**Wo das Modul steht, war ein Umweg.** Der erste Anlauf legte es zu den
übrigen wirtschaftlichen Grössen nach TOKENOMICS. Das ging nicht, denn
`myl-tokenomics` hängt an `myl-ledger` und nicht umgekehrt, **und die
Einordnung war ohnehin falsch:** Was hier steht, ist keine Formel,
sondern Zustandsmechanik.

**Belegt:** elf Tests, vier Gegenproben. Die schärfste davon baut den
Fluchtweg wieder ein, indem sie das Gekündigte aus der Masse nimmt; dann
meldet das Urteil `NoStake` und der Test fällt.

### v0.37.0 – 2026-09-02 (⚑ die Pod-Besetzung war wählbar, und die Prüfung stand am falschen Ort)

**Drei Funde aus Tor B, Punkt 2, und der erste ist der schwerste.**

⚑ **Fund 142: Die Besetzung folgte der Registerreihenfolge, nicht der
Saat.** `zonen_cluster` erhielt die Eingabereihenfolge, `assign_pods`
schnitt sie an festen Stellen, und die Eingabe ist `state.miner.values()`,
also nach `MinerId` sortiert. `MinerId` ist `SHA-256` über einen frei
erzeugbaren BLS-Schlüssel: **Wer eine Kennung an einer bestimmten Stelle
haben wollte, erzeugte Schlüssel, bis eine dort landete.**

**Damit fiel die Annahme, auf der Stufe 1 steht.** Zwei Pods rechnen
dieselbe Arbeit doppelt, und das trägt nur, solange ein Angreifer nicht
bestimmen kann, mit wem er in einem Pod sitzt.

⚑ **Der Kommentar in `assign_pods` behauptete das Mischen bereits.** Er
stammte aus `geo_clustering.rs`, die am 2026-09-01 entfernt wurde; der
Shuffle ging mit ihr, der Satz blieb stehen. **Eine Zusicherung, deren
Code verschwunden ist, ist gefährlicher als gar keine**, denn sie hält
den nächsten Leser vom Nachsehen ab.

**Gemessen** (`podbesetzung_sim.py`): Ein ganzer Pod kostete **0,06
Sekunden** Schlüsselerzeugung bei tausend ehrlichen Minern und drei
Sekunden bei hunderttausend. Der Anteil blinder Redundanzpaare bei einem
Angreifer an der byzantinischen Schranke lag bei **0,107**, also über
der Stichprobenrate der Stufe 2 (fünf Prozent) statt vier
Größenordnungen darunter.

**Jetzt wird gemischt**, je Zone mit einer eigenen abgeleiteten Saat,
und das Sammelcluster ebenso. Aus dem Anteil `f` an den Kennungen wird
ein Anteil `f⁶` an den ganz besetzten Pods.

⚑ **Was das Mischen nicht schließt, und das steht ausdrücklich im
Code:** Die Zone ist eine **Erklärung**. Wer eine angibt, in der sonst
niemand steht, bekommt daraus ganze Pods, gemischt oder nicht. Sie zu
schließen hieße, die Zone aus der Besetzung zu nehmen, und das kostet
Latenz in einer Pipeline, deren Shards nacheinander rechnen. **Das ist
eine Entscheidung des Projektinhabers und keine Ableitung**; sie ist
gefallen und lautet: Die Zone bleibt, die Antwortzeit zählt.

⚑ **Fund 143: Die Epochensaat kam vom Ende derselben Epoche.** Der
Knoten reichte den letzten Blockhash durch, und beim Epochenabschluss
ist das der letzte Block der Epoche, die gerade abgerechnet wird. **Die
Zuteilung stand also erst fest, wenn die Epoche vorbei war**, während
ein Bündel während ihr eingereicht sein muss: Kein Pod konnte wissen,
dass er einer ist.

Die Saat steht jetzt im Ledger-Zustand, gilt die ganze Epoche und stammt
aus `e−2`. **Genauso weit reicht der Registrierungsschluss aus Anhang
A.2**, und das ist kein Zufall: Wäre die Saat näher als der Schluss,
könnte sich jemand anmelden, nachdem er sie kennt. Ethereum nennt
dieselbe Konstruktion `MIN_SEED_LOOKAHEAD`.

⚑ **Warum der Fehler unentdeckt blieb:** Der große Test des Punktes hat
sechs Miner, also genau einen Pod, und der enthält alle sechs, gleich
welche Saat man nimmt. **Fund 142 hat Fund 143 unsichtbar gemacht.**

⚑ **Fund 144: Die Aufnahme eines Bündels prüfte nichts als die
Anmeldung.** `buendel_einreichen` prüfte Miner, Epoche und Dublette;
Mitgliedschaft, Koordinator und Aggregatsignatur prüfte es nicht,
sondern erst der Epochenabschluss.

**Die vollständige Prüfung gab es im Baum zweimal, und die vollständige
Hälfte war die ungenutzte:** `PoIRegistry::submit` prüft alle fünf
Schritte und hatte außerhalb von Tests keinen einzigen Aufrufer.

**Die Folge war Zustandswachstum, nicht nur eine späte Prüfung.**
`state.buendel` ist nach `PodId` geschlüsselt, und die Kennung wählt der
Einreichende frei: Jeder angemeldete Miner konnte den Zustand bis zur
Blockgrenze mit Bündeln für erfundene Pods füllen, rund 212 Bytes je
Stück, und jedes davon steckte bis zum Epochenwechsel in jeder
Zustandswurzel.

⚑ **Die Signierbotschaft ist dafür nach `myl-types` gezogen.** Sie lag
in `myl_consensus::poi`, und dort konnte `myl-ledger` sie nicht sehen,
denn `myl-consensus` hängt an `myl-ledger` und nicht umgekehrt. **Die
Prüfung stand am falschen Ort, weil die Botschaft am falschen Ort
stand.** Verschoben, nicht kopiert: Zwei Kodierungen wären zwei
Meinungen darüber, was unterschrieben wurde.

**Belegt:** fünf neue Ablehnungstests im Ledger, zwei im Scheduler, vier
im Knoten, und für jede eingebaute Prüfung eine eigene Gegenprobe.
⚑ **Eine davon hat zunächst nicht gebissen:** Der Test zur erfundenen
Pod-Kennung änderte die Kennung **nach** dem Unterschreiben und
scheiterte deshalb am Aggregat statt an der Kennungssuche. Er
unterschreibt jetzt neu und trifft die Suche.

### v0.36.0 – 2026-09-02 (`myl-consensus` v0.25.0: Arbeit qualifiziert, Stake wiegt)

**Entscheidung A3 des Projektinhabers, recherchiert und gebaut.** Der
Arbeitsanteil verlässt die Gewichtsformel; `voting_weight` ist der
Stake. Eine Mindestarbeit wird stattdessen Voraussetzung, gemessen als
Bruchteil des **Netzmedians** und mit Startwert **null**.

⚑ **Zwei Messungen haben die alte Formel erledigt.**

**Fund 135:** Der Höchstfaktor griff ab **1,13-fachem**
Referenzdurchsatz; darüber ergaben 1,2-fach und hundertfach denselben
Wert. Der Arbeitsanteil unterschied in einem Band von dreizehn Prozent.
**Und die Sicherheitsaussage stand daneben, ungesagt:** Ein MYL im
gedeckelten Validator wog zehnmal so viel wie eines im arbeitslosen. Der
Höchstfaktor war der **Divisor der Angriffskosten**.

**Fund 137:** `ValidatorRegistry::record_work` hatte außerhalb seiner
eigenen Tests **keinen Aufrufer**. Die Historie war im Betrieb immer
leer, `voting_weight == stake` galt bereits. **Die Umstellung ist
deshalb keine Verhaltensänderung, sondern eine Berichtigung des
Vertrags**, und die sechste Ausprägung des häufigsten Fehlerbilds dieses
Projekts.

**Was die Recherche sagt:** Ethereum kennt keinen Arbeitsanteil, das
Gewicht **ist** der Stake. Filecoin kennt denselben Faktor 10, verlangt
dafür aber zehnfache Sicherheit und schlachtet zehnfach. Bittensor
mischt, und die Auswertung zeigt Stake-zu-Belohnung 0,80 bis 0,95 gegen
rund 0,50 für Leistung. RepuCoin trägt arbeitsgewichtete Stimmen nur mit
Integration über die **gesamte** Kettengeschichte; das Fenster hier war
zehn Stunden.

**`myl-scheduler` v0.9.1: die Zahl, die seit dem 2026-08-26 offen war.**
`redundancy.rs` führte die Abwägung „ab wann schlägt Streuung die
Diversität" als **benannt und nicht gesetzt**. Gerechnet
(`security_sim.py`, Abschnitt 9): Die Verengung beträgt
`(km−1)/((k−1)m)` bei `k` Zonen, also rund `k/(k−1)`. Zwei Zonen kosten
Faktor 1,95, drei 1,48, zehn 1,11. ⚑ **Die Größe, an der es hängt, ist
die Zahl der Zonen und nicht der Anteil der größten.**

Dazu ein neuer Eigenschaftstest über die **Pod-Disjunktheit**: 2 800
erzeugte Pod-Mengen mit 27 201 überlappenden Paaren in den Eingaben,
Gegenprobe fällt bei Keim 1. Damit steht der Satz auf der Beweisliste
von `geprüft (Beispiele)` auf `geprüft`.

**`myl-ledger` v0.14.1: die halbe Invariante ist ganz geworden
(⚑ Fund 136).** Der Zufallslauf würfelte über **drei von achtzehn**
Übergängen, und `transfer` und `praegen` fehlten, also ausgerechnet die
beiden, die MYL bewegen und erzeugen. Der Satz der Beweisliste lautet
vollständig „die Summe bleibt gleich **oder die Quelle ist benannt**";
geprüft wurde die erste Hälfte über eine Menge, in der die zweite gar
nicht vorkommen konnte. Jetzt beide, und der Zuwachs muss **genau** dem
geprägten Betrag entsprechen.

**Nicht die Behauptung war falsch, sondern die Auswahl.**

### v0.35.0 – 2026-09-01 (`myl-consensus` v0.24.0: der Block trägt seine Saatquelle)

`BlockHeader.saatquelle`, additiv angehängt (Punkt 44).

⚑ **Warum sie in den Block muss.** Die Saat entscheidet, **wer
nachgerechnet wird**, und das ist eine Konsensentscheidung. Das
Commitzertifikat entsteht in der Runde und liegt danach **lokal** bei
dem, der es zusammengesetzt hat; zwei Knoten mit verschiedenen
Zertifikaten zögen verschiedene Segmente. **Eine Saat aus lokalem
Zustand ist keine Saat, sondern eine Meinung.**

Dieselbe Bauart wie `LastCommit` in Tendermint: Ein Block trägt den
Beleg für seinen Vorgänger.

⚑ **`None` heißt Blockhash**, und das ist der schlechtere Rückfall
(unbegrenzter Mahlraum statt höchstens sechzehn Bit, Fund 120). Er ist
deshalb ausdrücklich benannt statt stillschweigend.

### v0.34.0 – 2026-09-01 (`myl-consensus` v0.23.0, `myl-ledger` v0.14.0: die Anmeldung nennt eine Adresse)

`Anweisung::MinerAnmelden` trägt `netzadresse`, `miner_anmelden` nimmt
sie entgegen (Punkt 46, Fund 116). ⚑ **Sie steht in der Anweisung und
nicht daneben**, aus demselben Grund wie Hardware und Zone: Ein Feld
außerhalb der unterschriebenen Anweisung ließe sich abweichend füllen,
und dann meldete A für B an.

### v0.33.0 – 2026-09-01 (`myl-consensus` v0.22.0: die Segmentzahl wird mitunterschrieben)

`poi_bundle_message` bindet jetzt `segmente` (Fund 115). Der Aufbau
lautet `DST_POI_BUNDLE ‖ u64_le(epoch) ‖ pod ‖ segments_root ‖
u64_le(vtfe_claimed) ‖ u32_le(segmente)`.

⚑ **Ohne diese Bindung könnte der Koordinator die Segmentzahl nach dem
Einsammeln der Unterschriften erhöhen** und damit die
Stichprobenwahrscheinlichkeit je Segment verdünnen. `vtfe_claimed` steht
aus demselben Grund darin; der Schaden ist nur ein anderer.

### v0.32.0 – 2026-09-01 (`myl-scheduler` v0.9.0; ⚑ Funde 110, 111 und 112: die Paarung las eine Quelle außerhalb des Konsens)

**`assign_redundant_pods` nahm die Zone eines Pods aus der gegossipten
`NodeMetadata` seiner Mitglieder.** Seit der Entscheidung 3b steht die
Zone in der **Registrierung**, also im Konsenszustand, und der Pod trägt
die Registrierung jedes Mitglieds ohnehin bei sich. Die alte Quelle blieb
stehen, und sie hatte drei Löcher.

⚑ **Zwei Knoten mit verschiedener Gossip-Sicht paarten verschieden.** Wer
wessen Ergebnis nachrechnet, ist eine Konsensentscheidung. Sie aus einer
Quelle zu treffen, die nicht Teil des Konsens ist, bricht die
Gleichheit, auf der alles ruht. Das ist der schwerste der drei.

⚑ **Ein einzelnes Mitglied konnte seinen Pod aus jeder Paarung nehmen**,
indem es eine abweichende Region gossipte: Dann war die Zone des Pods
unbestimmt, und unbestimmt schloss ihn überall aus. Genug davon, und eine
ganze Epoche bekam keine Redundanz. **Genau diesen Verweigerungshebel
sollte die Entscheidung 3b vermeiden**, und er saß die ganze Zeit eine
Ebene tiefer.

**Fehlende Metadaten wirkten wie Widerspruch.** Ein frisch gestarteter
Knoten, dessen Gossip noch nicht durch war, fiel aus der Paarung, ohne
etwas falsch gemacht zu haben.

⚑ **Fund 111: Die Pod-Bildung gab es zweimal.**
`myl_pod::zuteilung::plane_epoche` rechnete die Zuteilung einer Epoche
**selbst** aus, und sie stimmte mit dem Weg der Kette in **keinem** der
drei Schritte überein: Cluster nach gemessener Latenz statt nach Zone,
VRF-Saat statt Blockhash, nur die Klassen aus den Parametern statt aller.
**Zwei Knoten, die denselben Pod auf verschiedenen Wegen ausrechnen,
bekamen verschiedene Pods.** `zuteilung_aus_saat` ist jetzt die eine
Regel; `zuteilung_der_epoche` und `plane_epoche` sind Eingänge in sie.
⚑ **`geo_clustering.rs` ist entfernt** (287 Zeilen samt acht Tests):
`form_clusters` und `LatencyMatrix` standen für den Weg, den 3b
verworfen hat, und **ein Grund im Entwurf hat den Aufruf nicht
verhindert**. `MinerCluster` ist geblieben und steht jetzt bei `Pod` und
`Zuteilung` in `shard_assignment.rs`, mit einer Notiz, was dort war und
warum es weg ist.

⚑ **Fund 112: Eine dünne Zone schloss ihre Miner aus, und das lud zum
Lügen ein.** Ein Pod braucht `k + 2` Mitglieder. Trug eine Zone weniger,
so trug sie **keinen einzigen Pod**, und ihre Miner landeten in
`ohne_pod`. Bei sieben Zonen und `k = 8` wären das siebzig Miner, bevor
jede Zone einen Pod trägt. **Der Schaden ist nicht der Ausschluss,
sondern der Anreiz:** Wer allein in seiner Zone steht, verdient nichts,
solange er die Wahrheit sagt, und alles, sobald er eine volle Zone
angibt. Das Verfahren drängte die Angabe zur Unwahrheit, genau dort, wo
sie am meisten wert gewesen wäre. Zonen unter der Mindestbesetzung kommen
jetzt in **ein gemeinsames Sammelcluster** in kanonischer
Zonenreihenfolge. Seine Pods haben keine bestimmte Ausfallzone, und die
Paarung sieht das, statt ihnen ein falsches Etikett zu geben.

**Was sich sonst ändert:**

- Zonendiversität ist **Vorliebe statt Bedingung**. Gibt es kein
  zonendiverses Paar, wird auf disjunkte Paare derselben Zone
  ausgewichen, und `Redundanzzuteilung::zonendivers` sagt es. **Keine
  Redundanz ist schlechter als Redundanz in einer Zone**, denn ohne Paar
  entfällt Stufe 1 der Verifikation ganz, und ein Netz, das in einer Zone
  anfängt, käme nie in Gang.
- Die Wahl zwischen beiden Mengen fällt **einmal für die ganze
  Zuteilung**, nicht Segment für Segment. Mischte man sie, käme jedes
  Segment zuerst an die diversen Paare, und wer zwei Zonen angibt, säße
  bevorzugt in jedem Vergleich.
- `ZuweisungsHindernis::KeinGueltigesPaar` heißt jetzt „kein Paar ist
  disjunkt", also eine Aussage über den **Aufbau** der Pods statt über
  Angaben ihrer Mitglieder.
- Der Anspruch, die Zonenprüfung schütze vor derselben Selbstbestätigung
  wie `pods_are_disjoint`, ist **zurückgezogen**. Eine erklärte Angabe
  trägt die Ausfalldiversität und nicht die Sicherheit (Fund 108).

⛑ **Ein Test prüfte eine Aussage, die unter beiden Fassungen gilt.** Er
sollte zeigen, dass ein abweichendes Mitglied seinen Pod nicht aus der
Paarung nimmt, und prüfte, dass die **übrigen** Pods weiter gepaart
werden. Das gilt auch mit dem alten Ausschluss. Die Zusage sitzt nicht in
der Paarung, sondern in der **Pod-Bildung**: Cluster entstehen je Zone,
also teilen die Mitglieder eines Pods seine Zone durch Konstruktion. Der
Test prüft jetzt die echte Zuteilung.

### v0.31.0 (`myl-consensus` 0.21.0, `myl-ledger` 0.13.0) – 2026-09-01 (⚑ Punkt 40, Glied 2: die Aggregatsignatur wird geprüft)

**`verify_bundle_signature` war seit Langem gebaut, geprüft und wurde von
der Kette nie gerufen.** Ihr fehlten die öffentlichen Schlüssel der
Pod-Mitglieder: `MinerId` ist `SHA-256` über den Schlüssel, und aus einem
Hash folgt kein Urbild. **Dieselbe Klasse wie Fund 87 und Fund 109**, zum
dritten Mal an diesem Vorhaben.

**`miner_anmelden` trägt jetzt den Schlüssel ein** und prüft, dass er zur
Kennung passt: Sonst trüge das Register einen fremden Schlüssel unter
dieser Kennung, und die Aggregatprüfung liefe gegen den falschen.

⚑ **`PodMembership::ohne_besitznachweis`, und warum das kein Loch ist.**
`PodMembership::new` verlangt je Mitglied einen Besitznachweis gegen
Rogue Keys. Kommen die Schlüssel aus dem **Register**, ist er bereits
erbracht, und zwar stärker: Ein Schlüssel gelangt nur über eine
**unterschriebene Anmeldung** hinein. **Wer einen Rogue Key als Differenz
fremder Schlüssel bildet, kann mit ihm nicht unterschreiben** und kommt
gar nicht erst hinein. Ihn ein zweites Mal zu verlangen hieße, je Epoche
eine Paarung je Mitglied zu rechnen, für eine Aussage, die feststeht.

⚑ **Die Mitgliedschaft kommt aus der Zuteilung, nie aus dem Bündel.**
Wer sie aus dem Bündel nähme, ließe den Einreicher bestimmen, gegen
welche Schlüssel geprüft wird.

⛑ **Der Test, an dem der Punkt hängt, fiel beim Einschalten sofort um**,
und das war die richtige Antwort: Er benutzte eine Attrappe als
Signatur. Er unterschreibt jetzt mit **allen** Mitgliedern, Reserve
eingeschlossen, denn gegen deren Schlüsselmenge wird geprüft.

### v0.30.0 (`myl-ledger` 0.12.0) – 2026-09-01 (die Arbeitsverteilung im Zustand)

`LedgerState.arbeitsverteilung` und `arbeitsverteilung_setzen`.

⚑ **Eine Verteilung je Pipeline-Stand, und nicht zwei.** Steht für
denselben Stand schon eine, wird abgelehnt: **Dieselbe Pipeline zweimal
verschieden zu gewichten hieße, dass die Gewichte nicht aus ihr folgen**,
und dann wären sie frei wählbar. Wer anders gewichten will, wechselt den
Stand, und der Wechsel ist sichtbar.

⚑ **Wer setzen darf, ist noch nicht durchgesetzt.** Das ist ein
Governance-Akt, und der Draht von einem angenommenen Beschluss hierher
fehlt, wie bei der Belastung der Treasury. **Es gibt deshalb keine
Anweisung dafür**, und das ist die sichere Wahl: Eine stünde jedem
Absender offen, und wer die Gewichte setzt, setzt die Verteilung des
Ertrags.

**`None` heißt: es wird nichts zugeschrieben**, der Shard-Miner-Anteil
bleibt ungeprägt.

### v0.29.0 (`myl-scheduler` 0.7.0) – 2026-09-01 (Punkt 40, Glied 3c: die Zuteilung, abgeleitet statt gespeichert)

`zonenzuteilung.rs`: Register, Registrierungsschluss, je Zone ein
Cluster, Pods per Seed. Zwölf Tests, vier Gegenproben.

⚑ **Abgeleitet und nicht gespeichert.** Die Zuteilung ist eine reine
Funktion aus Register, Epoche und Blockhash; sie in den Zustand zu
schreiben wäre eine zweite Quelle für dieselbe Aussage. Nebenbei
erspart es die D7-Frage ganz.

⚑ **Der Seed und was an ihm schwach ist.** Er folgt aus Blockhash und
Epoche, benutzt dabei denselben Trennstring wie der VRF-Seed, damit es
**eine** Kodierung gibt. **Der Erzeuger des letzten Blocks einer Epoche
kann ihn mahlen**: Er sieht für jeden möglichen Block die entstehende
Zuteilung. **Ein VRF-Seed behebt das nicht** — der Erzeuger hält den
Schlüssel und kann ebenso wählen; was der VRF bringt, ist
Unvorhersehbarkeit für alle anderen, nicht Mahlfestigkeit. Wogegen es
hilft, steht schon im Entwurf: Der Registrierungsschluss bei `e-2`
friert die **Menge** der Teilnehmer ein. Ein Mahlender kann umschichten,
wer wo landet, nicht, wer dabei ist.

⚑ **Und `pod_zu_kennung` schließt Fund 109**: Ein Bündel nennt seinen
Pod über eine `PodId`, die Zuteilung über `pod_index`, und zwischen
beiden gab es keine Verbindung.

⛑ **Zwei Tests sahen stärker aus, als sie waren**, und beide fielen erst
in der Gegenprobe auf. Der eine rief `filter_miners` selbst und prüfte
damit das Werkzeug statt seines Gebrauchs; der andere listete erst alle
Europäer, dann alle Asiaten, und weil die Zuteilung Cluster der Reihe
nach in Pod-Portionen schneidet, wären die Pods auch **ohne**
Zonengruppierung sortenrein gewesen. **Er prüfte seine eigenen Daten.**
Beide gehen jetzt durch die Zuteilung und mit verschränkter Eingabe.

### v0.28.0 (`myl-ledger` 0.11.0, `myl-consensus` 0.20.0) – 2026-09-01 (Punkt 40, Glied 1: das Bündel erreicht die Kette)

`Anweisung::BuendelEinreichen` (angehängt), `LedgerState.buendel`, dazu
`buendel_einreichen`, `buendel_der_epoche` und `buendel_leeren`.

**Geprüft wird:** angemeldeter Miner, laufende Epoche, kein zweites
Bündel für denselben Pod. Sieben Tests, drei Gegenproben.

⚑ **Was ausdrücklich nicht geprüft wird: die Aggregatsignatur gegen die
Pod-Mitglieder.** Sie ist die eigentliche Prüfung und setzt voraus, dass
der Zustand weiß, wer im Pod sitzt; das ist Glied 3c und steht aus.
**Solange sie fehlt, ist „angemeldeter Miner" eine schwache Schranke**,
und das gehört gesagt statt verschwiegen: Ein Angemeldeter kann heute
ein Bündel für irgendeinen Pod einreichen. Was ihn bremst, ist allein,
dass ohne Besetzung ohnehin nichts ausgeschüttet wird.

⚑ **Das Bündel trägt die Leistung des Pods, nicht die des Einzelnen.**
Ein Feld „mein Anteil" gäbe es hier nicht, auch wenn jemand eines
wollte: Ein Pod könnte damit intern umverteilen, und niemand außerhalb
könnte widersprechen (Festlegung vom 2026-08-31).

⚑ **Und die Bündel fallen am Epochenwechsel weg.** Ohne das wüchse der
Zustand unbegrenzt und **D7 wäre gebrochen**; begrenzt ist die Menge,
weil sie geleert wird, nicht weil sie klein anfängt. Die Historie steht
in den Blöcken.

### v0.27.0 (`myl-ledger` 0.10.0, `myl-consensus` 0.19.0) – 2026-09-01 (Punkt 40, Glied 3a: das Miner-Register)

**Der Ledger führt jetzt, wer sich als Miner angemeldet hat**, und die
Kette trägt die Anweisungen dafür (`MinerAnmelden`, `MinerAbmelden`,
angehängt wie `ProposeMitPolka`).

⚑ **Der Doc-Kommentar behauptete es seit Monaten.**
`MinerRegistration` trug den Satz „wird … im Ledger gespeichert"; der
Ledger kannte sie nicht. Der Scheduler bekam seine Liste vom Aufrufer,
**und wer sie liefert, entscheidet über die Pod-Bildung**: Zwei Knoten
mit verschiedenen Listen kommen zu verschiedenen Pods.

⚑ **Die Registrierungsepoche setzt die Kette, nicht der Antragsteller.**
Ein selbst gewähltes Datum hübe den Registrierungsschluss aus Anhang A.2
auf, der gerade verhindern soll, dass sich jemand kurzfristig anmeldet,
um eine Zuteilung zu beeinflussen. Die Anweisung hat deshalb **kein
Feld** dafür, und auch keins für die Kennung: Die folgt aus dem
Schlüssel, mit dem unterschrieben wurde.

⚑ **Eine Klassenänderung behält das Datum.** Sonst machte sie den Miner
jünger und damit für die nächste Zuteilung unqualifiziert.

**Die Abmeldung wirkt sofort.** Der Registrierungsschluss schützt die
Zuteilung vor **Zugängen**, die sie beeinflussen wollen, nicht vor
Abgängen; wer geht, bis zum Epochenwechsel weiterzuführen hieße, ihn in
Pods zu setzen, die er nicht mehr besetzt.

⚑ **Warum das Register in den Zustand darf und die Wissensdatenbank
nicht:** D7 hält **unbegrenzt wachsende** Mengen heraus, weil
`commitment()` den ganzen Zustand je Block serialisiert. Das Register
wächst mit der Zahl der Miner, und die ist die Größe des Netzes selbst.
**Latenz-Atteste gehören aus demselben Grund nicht hinein**; sie wären
bei tausend Minern gut vier Megabyte je Epoche.

Dreizehn neue Tests, drei Gegenproben.

### v0.26.0 (`myl-scheduler` 0.5.0) – 2026-08-31 (⚑ Fund 108: der Vertreter vertrat niemanden)

`pod_region` nahm die Region des **ersten** Miners als die des ganzen
Pods, mit dem Kommentar „in der Praxis sollten alle Miner in einem Pod
aus derselben Region kommen, da sie im selben Cluster sind".

⚑ **Das Cluster garantiert das nicht.** Die Clusterbildung arbeitet mit
**gemessener Latenz** und liest `region` an keiner Stelle; die Datei
erwähnt den Typ nicht einmal. Zwei Maschinen können zwanzig
Millisekunden auseinanderliegen und verschiedene Regionen angeben, und
dann trug der Pod das Etikett irgendeines Mitglieds.

**Jetzt gilt eine Region nur, wenn alle Mitglieder mit bekannten
Metadaten dieselbe nennen**, die Reserve eingeschlossen: Sie übernimmt
bei einem Ausfall und steht dann in derselben Zone wie die Position, die
sie ersetzt. Sind sie uneins, ist die Ausfallzone **unbekannt**, und das
ist etwas anderes als vielfältig. Sechs Tests, eine Gegenprobe, die auf
vieren zugleich beißt.

⚑ **Und die größere Hälfte von Fund 108 bleibt offen:** Die Region ist
eine Selbstauskunft, die niemand prüft. Wer beide Pods eines
Redundanzpaars im selben Rechenzentrum betreibt, trägt zwei Regionen ein
und besteht die Prüfung. **Damit ist der Redundanzvergleich in genau der
Lage, vor der `pods_are_disjoint` eine Ebene tiefer schützt.** Der
Modulkopf sagt das jetzt, statt Resilienz zu versprechen.

### v0.25.0 (`myl-ledger` 0.9.0) – 2026-08-31 (die eine Stelle, an der MYL entsteht)

`praegen` schreibt einem Konto geprägte MYL gut. Vier Tests, zwei
Gegenproben.

⚑ **Bis heute gab es diesen Übergang nicht.** Jeder andere schiebt
Guthaben oder vernichtet es; keiner ließ die Menge wachsen. Der Burn
wurde gezählt, geglättet, zu einer Prägung gerechnet, aufgeteilt, und
dann war Schluss.

**Die Funktion ist klein und prüft keine Bedingung selbst.** Wer prägen
darf und wie viel, entscheidet die Wirtschaftsrechnung in
`myl-tokenomics`; ein Kontenbuch, das die Prägeformel kennte, wäre ein
zweiter Ort für dieselbe Wahrheit. Zwei Dinge prüft sie doch: Ein Betrag
von null ist ein Fehler und kein Nichtstun, denn wer null prägt, hat sich
verrechnet und ein stiller Erfolg verdeckt das. Und ein Überlauf wird
**gemeldet statt gesättigt**: Eine gesättigte Prägung wäre
stillschweigend eine andere Geldmenge, und zwei Knoten mit verschiedenen
Geldmengen sind ein Konsensbruch.

### v0.24.0 (`myl-ledger` 0.8.0) – 2026-08-31 (das Auszahlungskonto gehört nicht dem heißen Schlüssel)

`LedgerState` führt, wohin ein Miner bezahlt wird; ein Übergang trägt es
ein. Fünf Tests, zwei Gegenproben.

⚑ **Die Miner-Kennung ist `SHA-256` über den Konsensschlüssel, und der
liegt heiß:** Er unterschreibt jeden Vote, jeden Commit, jeden Übergang,
jede Kapazitätszusage und jede Speicherquittung. Ihn zugleich zum Konto
zu machen, auf dem sich der Ertrag sammelt, ist der Fehler, den Ethereum
als Auszahlungsnachweis `0x00` gemacht und mit einer ökosystemweiten
Migration auf `0x01` korrigiert hat. Cosmos trennt von Anfang an,
Filecoin ebenso mit `owner` gegen `worker`, und das ist für Speicher
plus Beweise dieselbe Lage wie hier.

⚑ **Und die Änderung gehört dem kalten Konto.** Die **erste** Eintragung
unterschreibt der Miner selbst, er hat nichts zu verlieren; **jede
weitere das eingetragene Konto**. Damit leitet ein gestohlener
Konsensschlüssel den Ertrag nicht um, und es braucht keine Wartefrist,
über die jemand streiten könnte.

**Ohne Eintrag kein Anteil** (Festlegung des Projektinhabers): Wer
nichts eingetragen hat, wird bei der Verteilung übergangen, sein Gewicht
zählt nicht. So sammelt sich nie ein Ertrag unter einem heißen Schlüssel
an, und der Fehler fällt sofort auf, weil nichts ankommt.

### v0.23.0 (`myl-ledger` 0.7.0) – 2026-08-31 (der Zustand zählt den Burn mit)

Drei Felder: der Burn der laufenden Epoche, der geglättete Wert und die
Epoche, bis zu der fortgeschrieben ist. `burn_to_credits` zählt mit.

⚑ **Bis heute zerstörte der Übergang Münzen und vergaß sofort, wie viele
es waren.** Kap. 5.2 leitet die Prägung aus dem geglätteten Burn ab, den
geglätteten aus dem Burn je Epoche; ohne diese Zahl im Zustand hat die
Prägungsformel keine Eingabe. Die Fortschreibung selbst liegt in
TOKENOMICS, wo die Formel wohnt.

### v0.22.0 (`myl-ledger` 0.6.0) – 2026-08-31 (das Speicherregister im Zustand)

`LedgerState` bekommt ein Register der Gegenstände, deren Manifest
unmittelbar im Zustand steht, dazu zwei Übergänge zum Aufnehmen und
Entfernen. Sechs Tests.

⚑ **Nur ein Teil der Gegenstände steht darin, und der Grund ist die
Bauart des Commitments.** `commitment()` serialisiert den **ganzen**
Zustand und hasht ihn; es gibt keinen Baum mit Teilbeweisen. Jede
Änderung kostet O(Zustandsgröße) je Block. Eine unbegrenzt wachsende
Menge darf deshalb nicht einzeln darin stehen, sonst serialisiert jeder
Block die ganze Wissensdatenbank. **Die Infrastruktur steht direkt da**,
sie wächst nur durch Governance-Akte, und ein beitretender Miner muss
sie finden können, bevor er irgendetwas beweisen kann.

Die Wissensklassen weist der Übergang mit benanntem Grund ab, statt sie
stillschweigend aufzunehmen. Gegenprobe gefahren: Ohne die Prüfung
kommen sie durch.

⚑ **Das Register führt kein Guthaben, und das ist kein Vergessen:** Jede
Art, die direkt im Zustand steht, ist treasury-finanziert. Ein Guthaben
braucht nur, was ein Einleger bezahlt, und das läuft über die Wurzel.
Ein Test in `myl-types` hält den Zusammenhang fest.

Ein Test hält außerdem fest, dass das Register **in den Zustandshash
eingeht**: Täte es das nicht, wären sich zwei Knoten über den Inhalt
einig, ohne es zu sein.

### v0.21.0 (`myl-consensus` 0.18.0) – 2026-08-30 (wer zurückfällt, kommt zurück)

**Die zweite Richtung aus Fund 67.** `apply_commitzertifikat` holt seit
dem 2026-08-29 einen Knoten zurück, der **voraus** ist. Neu ist
`merke_hoehere_runde` für den häufigeren Fall: Ein Knoten hängt in Runde
2, während die anderen in Runde 5 sind, weil er später startete, kurz
die Verbindung verlor oder hinter einem langsamen Mesh sitzt.

Ohne die Regel holt er nur über die eigene Uhr auf, Runde für Runde, und
jede Frist ist um den Zuwachs länger als die vorige. Über ein WAN mit
echten Latenzen ist das der Unterschied zwischen einem Knoten, der
zurückkommt, und einem, der zusieht.

**Die Schranke ist ein Drittel des Stimmgewichts**, neu als
`VotingSet::drittel_schranke`. Mehr als ein Drittel kann nicht
vollständig byzantinisch sein; wer sie aus **einer** Runde
zusammenbekommt, hat von mindestens einem Ehrlichen gehört. Ein Quorum
zu verlangen wäre zu streng, denn der Zurückgefallene hört naturgemäß
nur einen Teil. Die Schranke ist **strikt**: Bei 900 Gesamtgewicht liegt
sie bei 301, nicht bei 300.

⚑ **Erst prüfen, dann zählen, und das ist die ganze Sicherheit dieser
Regel.** `BftState::receive_vote` lehnt eine fremde Runde ab, **bevor**
es die Signatur prüft; dort ist das richtig, es spart eine Paarung je
verirrter Nachricht. Wer die abgelehnten Nachrichten aber ungeprüft
zählte, hätte eine Liveness-Lücke gegen eine andere getauscht: **Ein
einzelner Byzantiner dürfte sich als beliebig viele Absender ausgeben
und jeden ehrlichen Knoten in jede Runde treiben, die er sich ausdenkt.**
Deshalb prüft `merke_hoehere_runde` die Unterschrift selbst, vor dem
Vermerk. Eine Gegenprobe hält es fest: Ohne die Prüfung fällt genau der
Test, der vier Stimmen mit fremden Absendernamen und einer Unterschrift
schickt.

**Die Sperre überlebt den Sprung**, wie sie den Wechsel über die Frist
überlebt. Ohne das wäre aus einer Liveness-Regel ein Sicherheitsloch
geworden, und auch dafür steht ein Test.

⚑ **Gezählt wird je Absender, nicht je Runde, und der erste Entwurf
machte es umgekehrt.** Er war in zwei Punkten schlechter, und beide
fielen erst beim Nachdenken über den Speicher auf:

- **Er wuchs unbegrenzt.** Die Signaturprüfung hält Fremde draußen,
  **nicht Mitglieder**. Ein einziger stimmberechtigter Byzantiner kann
  gültig unterschriebene Stimmen für beliebig viele Runden schicken; je
  Runde ein Eintrag heißt beliebig viele Einträge. Mit dem Absender als
  Schlüssel ist die Karte durch die stimmberechtigte Menge begrenzt, und
  Fluten hebt nur den eigenen Eintrag.
- **Er zählte zu wenig.** Wer für Runde 5 unterschreibt, hat Runde 4
  hinter sich. Zwei Knoten in Runde 4 und zwei in Runde 5 sind vier
  Knoten in Runde **mindestens 4**; je Runde getrennt gezählt blieben es
  zweimal zwei, und der Sprung unterblieb, obwohl Runde 4 belegt war.

Gezählt werden nur Vote und Commit: Ein Propose kommt je Runde von genau
einem Leader und trägt nie genug Gewicht, ein Commit-Zertifikat hat
seinen eigenen Weg. `RoundChange` bekommt zwei Marken, `Vorgemerkt` mit
Zwischenstand und Schranke sowie `Unerheblich`. **Der Zwischenstand
gehört ins Protokoll**, nicht nur der Sprung: Wer über echtes WAN misst
und einen hängenden Knoten sieht, will wissen, ob dessen Zähler steht
oder wächst.

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
