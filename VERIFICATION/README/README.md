# verification (`myl-verifier`)

> **Version:** 0.10.0
> **Datum:** 2026-08-27
> **Status:** 🎉 **Phasen 1, 2 und 3 abgeschlossen** (Punkte 1.1–1.3,
> 2.1–2.5, 3.1–3.6), Phase 4 zu drei Vierteln (4.1, 4.2 und 4.4 ✅,
> 4.3 ✅ seit 2026-08-24): Redundanzvergleich (Stufe 1),
> Bisektions-Spiel (Stufe 2) mit Checker-Modul, Challenge-Erzeugung,
> Bisektionsprotokoll, On-Chain-Schiedsrunde, Slash-Logik,
> Kontrollsegmente samt der Vorratsschranke aus Fund 58 und die
> Sicherheitssimulationen gegen Anhang B.2 und Kap. 6.8.
> **135 Tests grün** (105 Modultests, 19 adversariale, 9 Simulation, 2 Doku-Tests).
>
> ⚑ **Punkt 3.2 trägt weiterhin ein ⚠ und keinen Haken, aber aus einem
> anderen Grund als bisher.** Die Ununterscheidbarkeit ist eine
> Eigenschaft der Daten und braucht ein Prompt-Profil aus echtem
> Betrieb. Was seit v0.9.0 nicht mehr fehlt, ist das **Messgerät**
> (Punkt 3.6): Es fehlt der Verkehr, nicht das Verfahren.
>
> ⚑ **Die adversariale Ebene fand Fund 42:** Das Bisektions-Spiel nannte
> systematisch die **falsche Layer** und hätte damit den Betrüger
> freigesprochen und den ehrlichen Checker geschlachtet. Behoben in
> v0.4.0, siehe Changelog.

Verifikations-Subsystem: Redundanzvergleich, Bisektions-Spiel (Stufe 2),
Kontrollsegmente (Stufe 3). Referenzimplementierung von Whitepaper Kap. 6.4–6.9
und Anhang A.4.

## Aufgabe

Die Komponente, die INTEGER_LLMs Determinismus-Eigenschaft (Kap. 6.2) erst
wirtschaftlich nutzbar macht: Sie entscheidet, wann zwei Berechnungen als
"gleich" gelten, was bei Abweichung passiert, und wie eine Manipulation
wirtschaftlich unattraktiv gemacht wird.

**Drei Verifikationsstufen:**
1. **Redundanz (Stufe 1):** Commitment-Hash-Vergleich zweier Pods
2. **Stichproben (Stufe 2):** Bisektions-Spiel bei Abweichung
3. **zkML-Anker (Stufe 3):** Zukunftspfad (noch nicht implementiert)

## Abhängigkeiten

- **INTEGER_LLM:** Determinismus-Eigenschaft (Kap. 6.2) — ✅ Phase 12.21 akzeptiert
- **CONSENSUS:** BFT-Blockproduktion (Phase 3) für On-Chain-Schiedsrunde — ⏳ offen
- **NETWORKING:** Verschlüsselte Aktivierungs-Streams (Phase 3) für DA-Fragmente — ⏳ offen

## Struktur

```
VERIFICATION/
├── README/                   diese Kurzübersicht
└── myl-verifier/             das Verifikations-Crate (Bibliothek)
    ├── Cargo.toml
    └── src/
        ├── lib.rs            Crate-Root
        ├── redundancy.rs     Commitment-Hash-Vergleich (Stufe 1)
        └── delivery.rs       Auslieferungsmodi (Optimistic/Confirmed)
```

## Module

### `redundancy` — Commitment-Hash-Vergleich

Vergleicht die Commitment-Hashes zweier Pods an allen Spur-Positionen.
Binärer Vergleich (gleich/ungleich), parameterfrei (kein Schwellenwert).

```rust
use myl_verifier::compare_commitments;

let result = compare_commitments(&primary_trace, &redundant_trace)?;
match result {
    CompareResult::Match => { /* Pods stimmen überein */ },
    CompareResult::Mismatch { first_divergence } => { /* Abweichung bei Position */ },
}
```

### `delivery` — Auslieferungsmodi

Zwei Modi für die Ergebnis-Auslieferung:
- **Optimistic:** Sofortige Auslieferung + asynchroner Abgleich
- **Confirmed:** Zurückhalten bis Übereinstimmung bestätigt

```rust
use myl_verifier::{decide_delivery, VerificationMode};

let decision = decide_delivery(
    VerificationMode::Optimistic,
    &primary_trace,
    &redundant_trace,
)?;

match decision {
    DeliveryDecision::Deliver => { /* Ausliefern */ },
    DeliveryDecision::Hold => { /* Zurückhalten */ },
    DeliveryDecision::DeliverAndSlash { first_divergence } => { /* Ausliefern + Slashing */ },
}
```

## Tests

**67 Modultests** über alle sieben Module, dazu **19 adversariale Tests**
in `tests/adversarial.rs` (Punkt 4.4, Kritikpunkt K4).

Die Modultests belegen den Erfolgsfall: Commitment-Vergleich,
Auslieferungsentscheidungen, Fehlerbehandlung, Rundenzahl der Bisektion.
Die adversariale Ebene verlangt das Gegenteil, nämlich dass ein Angriff
**scheitert**:

| Angriff | abgewehrt, weil |
|---|---|
| eine andere Eingabe unterschieben, die zum erwarteten Ergebnis führt | die Offenlegung ist an `input_hash` aus der Spur gebunden (Fund A11) |
| ein Hash, der nicht zur offengelegten Aktivierung passt | Selbstauskunft wird nachgerechnet |
| eine Antwort aus einem anderen Streitfall wiedereinspielen | Segment-Id und Position sind gebunden |
| gar nicht antworten | Schweigen ist kein Freispruch |
| eine Eingabe liefern, an der die Ausführung scheitert | Fehlschlag zählt als Schuld |
| in der falschen Runde antworten | Rundenbindung, und die Runde wird nicht verbraucht |
| eine Offenlegung zu einer anderen Position einsetzen | `PositionMismatch` |
| das Spiel endlos ziehen | Rundendeckel, danach `AlreadyComplete` |
| eine unbrauchbare Spurlänge unterschieben (0, 2⁶³) | `InvalidTraceLength` statt Schuldspruch bzw. Panik |
| eine Challenge ohne Abweichung eröffnen | `NoDivergence` |
| eine Challenge außerhalb der Spur | `InvalidPosition` |
| eine verkürzte Spur einreichen, um den Betrug abzuschneiden | `LengthMismatch`, kein `Match` |
| sich selbst herausfordern, um das Kopfgeld zu kassieren | `IdenticalMiners` |
| 20 000 zufällige Schiedsrunden | keine spricht frei, keine stürzt ab |
| 20 000 zufällige Antwortfolgen im Spiel | keine Panik, jede endet, jede genannte Position liegt in der Spur |

**Drei Gegenproben stehen davor**, denn eine Prüfung, die alles ablehnt,
lehnt auch jeden Angriff ab: Das Spiel muss die **richtige** Layer nennen,
wer nirgends abweicht darf nicht verurteilt werden, und die ehrliche
Schiedsrunde muss freisprechen.

Die erste dieser Gegenproben ist es, die Fund 42 gefunden hat. Sie ist
gegen zwei eingebaute Fehler geeicht worden (Grenzverschiebung um eins,
umgedrehter Vergleich); beide fliegen auf.

## Changelog

### v0.10.0 – 2026-08-29 (Fund 42 bekommt seinen Generator, drei Jahre zu spät)

**Die Bisektion wird jetzt erschöpfend geprüft:** für **jede**
Spurlänge bis 64 und **jede** erste Abweichung darin nennt das Spiel
genau diese Position. 2 080 Spiele, Millisekunden.

⚑ **Genau das hat bei Fund 42 gefehlt.** Damals waren drei Tests grün,
sie hießen „konvergiert nach O(log L) Runden" und „grenzt auf ein
Intervall der Länge 1 ein", und **keiner prüfte, ob die genannte
Position die richtige ist.** Das Verfahren belohnte in fünfzehn von
sechzehn Fällen den Betrüger. Festgehalten wurde damals, ein Generator
hätte es in Sekunden gezeigt; er existierte nur nicht.

⚑ **Dazu die andere Hälfte, ohne die der erste Test erfüllbar wäre,
indem man immer schuldig sagt:** Wer nirgends abweicht, wird für keine
Spurlänge verurteilt.

⚑ **Erschöpfend, wo der Raum es zulässt, und erst dann ein Generator.**
Die Frage nach `proptest` war im Projekt zweimal verschieden
beantwortet worden: einmal ablehnend wegen der Abhängigkeit, einmal
befürwortend wegen der verkleinerten Gegenbeispiele. **Aufgelöst durch
eine dritte Möglichkeit, die beide übersahen:** Für einen großen Teil
dieser Aussagen ist der Eingaberaum klein genug, um ihn **ganz**
abzugehen. Ein erschöpfender Test ist stärker als jeder Zufallstest,
braucht keine Abhängigkeit, und ein Gegenbeispiel muss man nicht
verkleinern, wenn man ohnehin bei den kleinsten anfängt.

### v0.9.0 – 2026-08-27 (Punkt 3.6: das Messgerät für die Ununterscheidbarkeit)

`src/unterscheidbarkeit.rs`. Ein Zweistichprobentest auf ganzzahligen
Merkmalen, durchgehend ohne Gleitkomma: totaler Variationsabstand über
einem Fächerraster als Teststatistik, Signifikanz aus einem
**Vertauschungstest**, p-Wert als Bruch zweier Ganzzahlen.

**Warum ein Vertauschungstest und kein χ².** Ein χ²- oder KS-Test bringt
Tabellen kritischer Werte und Verteilungsannahmen mit, die für
Prompt-Längen niemand geprüft hat, und beide rechnen in Gleitkomma. Der
Vertauschungstest ist exakt für die Daten, die er bekommt, braucht keine
Tabelle, und gemischt wird mit dem vorhandenen `SeedRng`: Derselbe Seed
ergibt denselben p-Wert, ein Messwert ist also nachrechenbar.

⚑ **Der Fallstrick sitzt im Akzeptanzkriterium der Phase selbst.** Es
lautet „statistische Analyse zeigt **keinen signifikanten
Unterschied**", und so formuliert ist es von einem schlechten Test
**leichter** zu erfüllen als von einem guten: Ein Raster mit einem
einzigen Fach findet nie etwas und meldet immer Erfolg. „Kein
Unterschied gefunden" ist keine Aussage, solange nicht danebensteht, was
dieser Aufbau überhaupt hätte finden können.

Deshalb gibt es hier keinen Weg zu einem Nein ohne die Trennschärfe:
`Befund::KeinNachweis` trägt `erkennbar_ab` im Wert, und
`traegt_die_aussage()` bleibt falsch, solange dort nichts steht. Wer das
Ergebnis weiterreicht, reicht seine Grenze mit. Dieselbe Bauart wie an
anderen Stellen des Projekts: Die Regel steht nicht im Kommentar,
sondern im Typ.

**Geeicht, nicht behauptet.** Zwei Messungen halten das Gerät fest: Unter
gleicher Verteilung schlägt es selten an (Fehlalarmrate im erwarteten
Bereich), und eine deutliche Verschiebung findet es zuverlässig. Ohne
die zweite hieße die erste nur „schlägt nie an". Die Eichverteilung ist
eine gleichverteilte Modellgröße und wird ausdrücklich als Annahme
genannt; was die Eichung liefert, ist eine Eigenschaft **des Geräts**,
nämlich wie viele Proben es für eine Abweichung gegebener Größe braucht.
Genau diese Zahl braucht man im Betrieb, **bevor** man misst.

⚑ **Fund 72: Die Kontingentgrenze verrät nichts, und das folgt aus einem
Satz.** `einschleusungsplan` zieht ohne Zurücklegen, also enthält ein
Strom von `n` Aufträgen **genau** `⌈n·γ⌉` Kontrollen statt einer
schwankenden Zahl. γ ist ein Governance-Parameter und damit öffentlich,
`n` sieht ein Miner am Strom: Er kennt das Kontingent. Hätte er so viele
Kontrollen erkannt, wie hineinpassen, wäre jeder weitere Auftrag mit
Sicherheit echt, und auf einem sicher echten Auftrag kostet Betrug
nichts.

Gemessen (`messe_kontingent`, γ = 2 %, 100 000 Aufträge, Kontingent
2 000): **null** sicher echte Aufträge, über jeden geprüften Vorrat von
1 bis 4 096.

Die Null ist kein Zufall. Die Wiederholung erkennt ein Segment
frühestens beim **zweiten** Auftreten, also bleiben von `v`
Vorratssegmenten die ersten `min(v, k)` Auftreten unerkannt, und damit
gilt `verdächtig ≤ k − min(v, k) < k` für jeden Vorrat `v ≥ 1`. Die
gemessenen Zahlen sind exakt `2 000 − Vorrat`, also der Satz mit
Gleichheit.

**Folgerung: `einschleusungsplan` bleibt, wie er ist.** Der naheliegende
Umbau wäre, je Position unabhängig mit Wahrscheinlichkeit γ zu ziehen;
er würde ein Loch schließen, das es nicht gibt, und dafür die exakte
Einhaltung von γ je Strom aufgeben, an der die Kostenbuchhaltung aus
Punkt 3.4 hängt. Erschöpfen ließe sich das Kontingent nur mit einem
Unterscheider, der **jede** Kontrolle erkennt, und wer den hat, braucht
das Kontingent nicht mehr.

**Ein Nullergebnis mit Begründung ist mehr wert als eines ohne.** Ohne
den Satz stünde hier eine Messung über fünf Vorratsgrößen, und niemand
wüsste, ob die sechste anders ausfällt.

**Gemessen:** 135 Tests grün (105 Modultests, davon 17 zum Messgerät und
5 zu Fund 72, dazu 19 adversariale, 9 Simulation, 2 Doku-Tests). Acht
Gegenproben: Abstand auf null, Raster immer brauchbar, p-Wert ohne die
beobachtete Anordnung, Etiketten ungemischt, Trennschärfe unterschlagen,
blindes Nein als Aussage, Kontingent auf null, Kontingent um den Vorrat
gesenkt. Jedes Mal rot, jedes Mal beim Test mit der Eigenschaft im
Namen. `examples/a11_kurve.rs` druckt beide Kurven.

**Was weiterhin fehlt und wofür dieses Modul nicht steht:** die Messung
selbst. Sie braucht echte Prompt-Längen, echte Zwischenankunftszeiten
und echte Kontextprofile, und die entstehen erst im Betrieb. Punkt 3.2
trägt deshalb weiterhin kein volles Häkchen, und das Messgerät ist kein
Beleg, sondern die Voraussetzung dafür, später einen zu bekommen.

### v0.8.0 – 2026-08-27 (Punkt 3.5: die Vorratsgröße wird ein Parameter)

Der offene Punkt aus Fund 58 ist geschlossen, gemeinsam mit
`myl-governance` v0.2.0: Vorratsgröße und Beobachtungsfenster stehen dort
als Parameter, eine Invariante bindet beide an γ. Einzelheiten oben bei
Fund 58.

⚑ **Dabei fiel eine zweite Fassung derselben Formel auf.**
`KontrollsegmentVorrat::reicht_fuer` rechnete die Schranke
ausgeschrieben nach, statt `noetiger_vorrat` zu rufen: zwei Fassungen
einer Sicherheitsschranke im selben Crate, von denen eine hätte
davonlaufen können, ohne dass ein Test es bemerkt. Genau das Muster, das
`gleichstand.rs` in GOVERNANCE zwischen Crates verhindert — hier lag es
innerhalb eines. Zusammengeführt, mit einer Gegenprobe über den ganzen
Bereich: Genau der nötige Vorrat reicht, einer weniger nicht.

**`noetiger_vorrat` und `reichweite` rechnen jetzt in `u64`** statt in
`usize`. Der Wert wandert in die Parameter-Registry, und die rechnet in
`u64`; eine Umrechnung an der Grenze wäre die Stelle, an der auf einer
32-Bit-Maschine eine Sicherheitsschranke stillschweigend abgeschnitten
würde. Beide Funktionen sättigen jetzt, statt abzuschneiden — eine
Schranke, die bei einer Eingabe außerhalb ihres Bereichs eine *kleine*
Zahl liefert, wäre gerade dort wirkungslos, wo sie greifen müsste.

**Neu: `KontrollsegmentVorrat::fuer_fenster`.** Der bisherige Weg nahm
jede Zahl an, und die einzigen je geschriebenen waren 10 und 100. Wer
hier hereinkommt, muss sich über das Fenster äußern und bekommt die
Größe ausgerechnet.

### v0.7.0 – 2026-08-25 (A11: Ununterscheidbarkeit gemessen)

`src/unterscheider.rs` baut den Angriff auf Kontrollsegmente, statt ihn
zu beschreiben. Kap. 6.7 verlangt Ununterscheidbarkeit als **erste**
Konstruktionsanforderung, und `kontrollsegmente.rs` sagt selbst, dass es
sie nicht erfüllen kann: Sie ist eine Eigenschaft der Daten.

**Was sich trotzdem messen ließ:** ob die **Einschleusung** eine Spur
hinterlässt, unabhängig vom Inhalt der Prompts.

⚑ **Fund 58: Ein zu kleiner Vorrat verrät sich vollständig.** Der Vorrat
ist endlich, der Auftragsstrom nicht. Wird öfter eingeschleust, als der
Vorrat verschiedene Segmente hält, wiederholen sich Ids. **Echte Arbeit
wiederholt sich nie**, also ist jedes zweite Auftreten ein Beweis, kein
Verdacht. Bei γ = 2 % über 100 000 Aufträge:

| Vorrat | erkannte Kontrollen | Fehlalarme |
|---|---|---|
| 64 | **96,8 %** | 0 |
| 256 | 87,2 % | 0 |
| 1024 | 48,8 % | 0 |
| 2048 | 0 % | 0 |

**Null Fehlalarme ist der schlimmste Teil:** Der Angreifer geht kein
Risiko ein. Er rechnet die erkannten Kontrollen ehrlich und manipuliert
den Rest.

**Was den Fund schärfte:** γ war ein Governance-Parameter, die
Vorratsgröße keiner. Die einzigen je geschriebenen Werte standen in
Tests: 10 und 100. Bei γ = 2 % trägt 100 genau 5 000 Aufträge. **Die
Zahl, an der die Wirksamkeit hängt, war nirgends festgelegt.**

**Geschlossen am 2026-08-27 (v0.8.0, Punkt 3.5).** Vorratsgröße und
Beobachtungsfenster sind jetzt Governance-Parameter, und eine Invariante
bindet beide an γ: `Vorrat ≥ ⌈Fenster · γ⌉`, geprüft am **Vorschlag**
statt nach der Abstimmung. Der Wert ist abstimmbar, das Unterschreiten
der Schranke nicht — dieselbe Bauart wie `S_min`.

⚑ **Der gefährliche Vorschlag sieht aus wie eine Verschärfung:** γ von 2
auf 4 Prozent heben, ohne den Vorrat mitzuziehen. Das verdoppelt die
Einschleusungen bei gleichem Vorrat und halbiert die Reichweite. Vorher
wäre er zulässig gewesen.

**Warum das Fenster ein eigener Parameter ist:** „Der Vorrat muss größer
sein als die Zahl der Einschleusungen" ist für einen unbegrenzten
Auftragsstrom von **keinem** endlichen Vorrat erfüllbar. Erst ein
Fenster macht die Bedingung entscheidbar, und wer es senkt, senkt die
Schranke.

**Der Anfangswert ist vorläufig:** 2 048 ist der gemessene Wert der
Tabelle oben, 100 000 die Stromlänge, über die gemessen wurde. Was
fehlt, ist die **Auftragsrate des Netzes** — erst mit ihr ließe sich
sagen, wie lange ein Fenster dieser Größe in Sekunden ist.

**Offen bleibt** das Prompt-Profil (Länge, Timing, Kontext). Die
Schranke beseitigt **eine** sichere Spur, die der Mechanik; gegen
Unterscheidung an Länge, Timing oder Inhalt hilft sie nicht. Dafür
braucht es echte Verteilungen aus dem Betrieb.


### myl-verifier v0.6.0 – 2026-08-24 (Punkt 4.3: Liveness gemessen)

Kap. 6.8 macht eine **quantitative** Liveness-Zusage: „Session-Verlust
nur bei mehr als zwei gleichzeitigen Ausfällen im selben Pod." Zwei
Aussagen stecken darin, und `tests/simulation.rs` prüft **beide**: bis zu
zwei überstehen die Session, **drei nicht**. Gemessen über alle
Ausfallmuster bis zur Podgröße, nicht über eine ausgesuchte Folge.

Dazu die Kostenseite: Ein Rebuild kostet `Position · Layer`, an Position
10 000 also die Arbeit von 10 000 Token für diesen Shard. „Standby
übernimmt" ist nicht kostenlos, und der Test hält die Größenordnung fest,
damit die Zusage nicht auf dem Papier gilt und im Betrieb unbezahlbar
ist.

*Zur Abhängigkeit:* `myl-pod` steht als **Dev-Abhängigkeit** in der
`Cargo.toml`, nicht als normale. Es hängt seinerseits an `myl-verifier`;
als normale Abhängigkeit wäre das ein Zyklus, für Dev-Abhängigkeiten
erlaubt Cargo ihn, weil sie nicht in den Bibliotheksbau eingehen. Die
Standby-Übernahme wohnt in `myl-pod`, die Zusage, die sie einhalten muss,
steht in Kap. 6.8 und damit in dieser Komponente.

### myl-verifier v0.5.0 – 2026-08-24 (Kontrollsegmente, und die Simulationen gegen das Papier)

#### Phase 3: Kontrollsegmente (Kap. 6.7)

`src/kontrollsegmente.rs`: Vorrat, Einschleusungsplan mit Anteil γ,
Erneuerung aus geprüften Echtsegmenten, Prüfung gegen das Soll. γ steht
als Governance-Parameter in der Registry.

Der Mechanismus ist der **einzige der Architektur, der gegen den
einmaligen Eingriff wirkt**: Stufe 1 und 2 setzen beide voraus, dass der
Zwillings-Pod ehrlich rechnet oder der Angreifer wiederholt auffällt.

⚑ **Was das Modul nicht leistet, und es gehört vor den Haken gesagt.**
Kap. 6.7 nennt die **Ununterscheidbarkeit** als erste
Konstruktionsanforderung. Das ist eine **Eigenschaft der Daten, nicht des
Codes**: Kein Datentyp erzwingt, dass ein Prompt im Timing-, Längen- und
Kontextprofil unauffällig ist. Das Whitepaper führt es selbst als offene
Messfrage (Kap. 11, Punkt 5). **Punkt 3.2 trägt deshalb kein volles
Häkchen**, sondern „Mechanik ✅, Ununterscheidbarkeit ❌". Erkennt ein
Miner Kontrollsegmente an statistischen Merkmalen, trägt auch der Rest
dieser Phase nichts.

⚑ **Die Sicherheitsbedingung der Einschleusung** kann im Code ebenfalls
nicht erzwungen werden: Der Seed gehört dem Gateway und darf erst nach
Auslieferung offengelegt werden. Wer ihn vorher kennt, weiß, welche
Aufträge Kontrollen sind, und manipuliert genau die anderen.

#### Phase 4.1 und 4.2: die Simulationen

**Sie rechnen die Formeln nicht nach, sie messen an den echten
Zuteilungsfunktionen.** `β^{2k}` in einem Test noch einmal auszurechnen
belegt nichts außer der Rechenfähigkeit des Testrahmens. Die Formeln des
Papiers unterstellen unabhängige, gleichverteilte Ziehungen; die
Implementierung zieht nicht so, denn Pods entstehen aus Geo-Clustern und
die Redundanzpaarung verlangt disjunkte, zonendiverse Pods. Anhang B.2
nennt diese Frage selbst und verschiebt sie auf Meilenstein M1.

| Simulation | Papier | gemessen |
|---|---|---|
| Kollusion, β = 50 %, k = 4, 10 000 Zuteilungen | β^2k = 3,906 · 10⁻³ | **3,900 · 10⁻³** |
| Soundness, 200 000 Segmente | Produkt der Einzelraten 0,96040 | **0,96045**, Abweichung 0,01 % |

Beide Aussagen des Papiers halten gegen die Implementierung. Bei β = 20 %
liegt die erwartete Ereigniszahl bei 0,026 und ist mit dieser Stichprobe
nicht messbar; 0 von 10 000 belegt dort nichts, und das steht so im Test.

*Nebenbefund zur Soundness:* Auch bei **demselben** Seed für Stichprobe
und Einschleusung bleibt die Abweichung bei 0,00 %, weil die beiden
Verfahren verschieden ziehen (Lotterie gegen Sortierschlüssel). Die
Betriebsregel verschiedener Seeds bleibt richtig, hängt dann aber nicht an
der Unabhängigkeit, sondern daran, dass ein gemeinsamer Seed beide Mengen
auf einmal verrät.

⚑ **Beim Bau aufgefallen:** `myl_scheduler::assign_redundant_pods` gibt
einen **leeren Vektor** zurück, wenn für die Miner keine `NodeMetadata`
vorliegen. Fail-closed und damit die richtige Richtung, aber **still**:
nicht zu unterscheiden von „keine Segmente angefragt". Offener Punkt.

### myl-verifier v0.4.0 – 2026-08-23 (adversariale Testebene, Punkt 4.4; ⚑ Fund 42 und 43)

**Punkt 4.4 „Adversariales Fuzzing Challenge/Verdict" erfüllt**,
und er hat sich sofort bezahlt gemacht.

#### ⚑ Fund 42: Das Bisektions-Spiel nannte die falsche Layer

Bei einer Spur der Länge 16 und einer echten Abweichung an Position `d`
nannte das Spiel für **jedes** `d` von 1 bis 15 die Position `d − 1`. Nur
`d = 0` traf zu, und das aus Versehen: dort kann die untere Grenze nicht
mehr fallen.

Ursache war eine Grenzverschiebung um eins. Bei Einigkeit an `mid` wurde
`lower = mid` gesetzt statt `mid + 1`, obwohl `mid` als Kandidat damit
ausgeschlossen ist; die Suche kam eine Position zu früh zum Stehen.

**Die Wirkung ist die Umkehrung des Verfahrens.** Die Schiedsrunde rechnet
die genannte Layer nach. Layer `d − 1` hat der Angeklagte korrekt
gerechnet, sein Ergebnis stimmt, und er wird **freigesprochen**;
anschließend verliert der Checker, der die Abweichung zu Recht gemeldet
hat, und wird geschlachtet. Stufe 2 der Verifikationsarchitektur hätte in
15 von 16 Fällen den Betrüger belohnt und den ehrlichen Prüfer bestraft.

**Warum es niemand sah:** Die Bestandstests prüften „konvergiert nach
O(log L) Runden" und „grenzt auf ein Intervall der Länge 1 ein". Beides
war wahr. Dass die genannte Position die **richtige** ist, prüfte keiner.
Das Modul hat außerdem bis heute keinen Aufrufer außerhalb des Crates,
also fiel es auch im Betrieb nicht auf.

#### ⚑ Fund 43: Die Antwort des Angeklagten war ohne Wirkung

`process_response_with_comparison` entschied aus zwei Hashes, die der
**Aufrufer** mitgab, und ließ `response.activation_hash` unbenutzt. Ein
Checker, der beide Spuren ohnehin hat, braucht dafür kein Gegenüber; das
Protokoll war also nicht interaktiv, und die Offenlegung des Angeklagten
war an nichts gebunden. Dieselbe Lücke, die Fund A11 in der Schiedsrunde
geschlossen hat, eine Ebene höher.

Die zweite Fassung `process_response` bekam den erwarteten Hash zwar
übergeben, verglich ihn in einem **leeren `if`-Block** und setzte danach
weder `lower` noch `upper`. Sie verbrauchte nur Runden und endete
zwangsläufig in `Incomplete`, obwohl ihre Dokumentation „aktualisiert den
Session-Zustand" zusagte.

#### Drei kleinere Funde im selben Modul

- **Die leere Spur war ein Schuldspruch.** `BisectionSession::new(id, 0)`
  lieferte sofort `DivergenceFound { position: 0 }`, also eine
  Verurteilung ohne eine einzige Runde. Jetzt `InvalidTraceLength`.
- **Eine absurde Spurlänge war eine Panik.** `next_power_of_two()` läuft
  jenseits von 2⁶³ über. Jetzt abgewiesen, Obergrenze 2⁶².
- **`(lower + upper) / 2` konnte überlaufen.** Im Debug-Build eine Panik,
  im Release-Build eine stille Falschrechnung, und damit zwei
  Schiedsrichter mit verschiedenen Bauprofilen bei verschiedenen Urteilen.
  Jetzt `lower + (upper − lower) / 2`.
- **`InProgress` ersetzt `NoDivergence` für die laufende Session.** Wer
  das Ergebnis einer laufenden Bisektion abfragte, bekam einen Freispruch;
  „noch nicht entschieden" und „nichts gefunden" sind zwei Aussagen.
  `NoDivergence` bedeutet jetzt, was der Name sagt.

#### Schnittstelle (breaking, ohne Aufrufer)

- `BisectionSession::new` liefert `Result`
- `BisectionResponse` trägt `position`, womit `PositionMismatch` erstmals
  tatsächlich eintreten kann (die Variante war definiert und wurde nie
  zurückgegeben)
- `BisectionSession` trägt `trace_len`
- `process_response_with_comparison` entfällt, `process_response` grenzt
  jetzt wirklich ein
- `BisectionResult::InProgress` neu

### myl-verifier v0.3.2 – 2026-08-23 (die Spur ist Layer-granular geworden)

**An der Rechnung dieses Crates ändert sich nichts**, an der Aussage
seiner Ergebnisse schon. Bisektion und Redundanzvergleich arbeiten auf
Indizes und Hashes; wie fein die Spur geschnitten ist, entscheidet
`myl-pod`. Seit dessen v0.5.0 trägt sie einen Eintrag **je Layer** statt
je Shard.

**Was das löst:** [`redundancy::compare_commitments`] lehnt ungleiche
Spurlängen mit `LengthMismatch` ab, und das war richtig. Solange die
Spur Shard-granular war, hing ihre Länge aber am Zuschnitt des Pods, und
zwei redundante Pods mussten denselben tragen. Genau daran war der
Entwurf für **variable Knotenzahl je Pipeline** blockiert, dessen
gemischte Paarung rund 600-mal sicherer ist als zwei schnelle Pipelines.

**Was das ändert:** Die Bisektion grenzt jetzt die fehlerhafte **Layer**
ein statt der fehlerhaften Layer-Gruppe, bei unverändertem O(log L). Die
Schuldzuweisung wird damit feiner, ohne dass das Protokoll mehr Runden
braucht. Die Modulköpfe und Feldkommentare sagen das jetzt auch so;
vorher stand dort durchgehend „Layer-Gruppe".

Belegt in `myl-pod`, nicht hier: vier gegen acht Shards ergibt `Match`,
und dieselbe Spur entsteht bei k = 1 bis 24.


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


### v0.3.0 – 2026-08-18 (Audit-Block 4: Slashing und Schiedsrunden-Bindung)

**Fund A9 — zwei unvereinbare Slashing-Modelle.** `slash.rs` rechnete
mit **festen Beträgen** (`SlashConfig`: 1 MYL Slash, 0,5 MYL Kopfgeld),
während `myl_ledger::apply_verdict` einen **Anteil des Stakes**
schlachtet (`SlashParams` als Zähler/Nenner-Paare) — so wie Whitepaper
Kap. 5.5 es vorgibt (30–100 % je nach Vergehen). `myl-verifier` hing
nicht einmal an `myl-ledger` und konnte gar nicht buchen. Ein fester
Betrag hat zudem keine Abschreckungswirkung: 1 MYL ist für einen
Großstaker nichts, und die Sicherheitsannahme der gesamten
Verifikationsarchitektur (Kap. 6.9: Betrug muss teurer sein als der
erwartete Gewinn) hängt genau daran.

Behoben: `SlashDecision` entscheidet nur noch über **Schuld**, nicht
über Beträge. `to_ledger_verdict()` liefert den Schiedsspruch im
Ledger-Format; die Beträge ergeben sich aus Stake und
Governance-Parametern beim Buchen. `myl-ledger` ist jetzt Dependency,
und ein Test bucht eine Entscheidung tatsächlich durch
(`entscheidung_wird_vom_ledger_gebucht`, `slash_skaliert_mit_dem_stake`).

**Fund A11 — die Schiedsrunde band die Eingabe nicht an die Spur.**
`adjudicate()` prüfte die offengelegte Aktivierung nur gegen den in
derselben Antwort mitgelieferten Hash — eine tautologische Prüfung, die
nur feststellte, dass der Angeklagte in sich konsistent geantwortet
hat. Der `AdjudicationRequest` trug keinen Hash von a_{j-1}. Ein
Angeklagter, der eine **andere** Eingabe fand, die unter seiner
Ausführung den erwarteten Ausgabe-Hash ergibt, wurde freigesprochen —
und damit die Zusage aus Kap. 6.6 („die Schuldzuweisung ist eindeutig,
weil das Ergebnis kanonisch ist") ausgehebelt: kanonisch ist das
Ergebnis nur bezogen auf die committete Eingabe.

Behoben: `AdjudicationRequest.input_hash` (aus `Segment.trace[j-1]`);
die offengelegte Aktivierung wird dagegen geprüft. Regressionstest
`untergeschobene_eingabe_wird_nicht_freigesprochen` stellt genau den
Angriff nach — mit einem Executor, der auf die untergeschobene Eingabe
exakt den erwarteten Hash liefert.

- `Challenge` kommt jetzt aus `myl-types` statt aus einer eigenen
  Definition (Fund A8).
- Compiler-Warnung in `bisection.rs` behoben.
- 67 → 66 Tests (Tests für die entfallenen Betrags-Helfer `net_transfer`
  und `has_sufficient_stake` gestrichen, dafür Ledger-Durchbuchung und
  die Schiedsrunden-Regression neu).

### v0.2.6 – 2026-08-17 (Phase 2.4: On-Chain-Schiedsrunde)
- On-Chain-Schiedsrunde (adjudicate) mit ShardExecutor-Trait (8 Tests)
- AdjudicationRequest/Response/Result Strukturen
- Hash-Vergleich für Schuldzuweisung
- 8 neue Tests grün, insgesamt 66 Tests

### v0.2.5 – 2026-08-17 (Phase 2: Bisektions-Spiel)
- Checker-Modul: Segment-Nachrechnung mit SegmentAuditor-Trait (7 Tests)
- Challenge-Erzeugung: Erste abweichende Position bestimmen (10 Tests)
- Bisektionsprotokoll: O(log L) Runden, binäre Eingrenzung (9 Tests)
- Slash-Logik: VerdictOutcome, SlashDecision, Kopfgeld-Auszahlung (11 Tests)
- 37 neue Tests grün, insgesamt 58 Tests

### v0.1.1 – 2026-08-17 (Phase 1: Redundanzvergleich)
- Commitment-Hash-Vergleich zweier Pods an allen Spur-Positionen
- Optimistische Auslieferung (sofort + asynchroner Abgleich)
- Bestätigte Auslieferung (zurückhalten bis Bestätigung)
- Binärer Vergleich (kein Schwellenwert), 21 Tests grün
