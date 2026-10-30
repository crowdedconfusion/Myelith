# verification (`myl-verifier`)

> **Version:** 0.27.0
> **Datum:** 2026-10-29
> **Status:** 🎉 **Phasen 1, 2 und 3 abgeschlossen** (Punkte 1.1–1.3,
> 2.1–2.5, 3.1–3.6), Phase 4 zu drei Vierteln (4.1, 4.2 und 4.4 ✅,
> 4.3 ✅ seit 2026-08-24): Redundanzvergleich (Stufe 1),
> Bisektions-Spiel (Stufe 2) mit Checker-Modul, Challenge-Erzeugung,
> Bisektionsprotokoll, On-Chain-Schiedsrunde, Slash-Logik und die
> Sicherheitssimulationen gegen Anhang B.2 und Kap. 6.8.
>
> ⚑ **Die Kontrollsegmente sind seit dem 2026-09-02 fort** (Entscheidung
> A1, rund 1 700 Zeilen: Mechanik, Spurdetektor und Messgerät). Dass
> dieser Kopf sie bis zum 2026-09-03 noch führte, ist Fund 158. Der
> Grabstein mit der vollständigen Begründung steht in
> `myl-verifier/src/lib.rs`; `gamma` ist in die Stichprobenrate
> aufgegangen.
> **144 Tests grün** (111 Modultests, 19 adversariale, 9 Simulation, 3 Bisektion, 2 Doku-Tests).
>
> ⚑ **Punkt 3.2 ist seit dem 2026-09-03 nicht mehr offen, sondern
> gegenstandslos** (Fund 158). Die Ununterscheidbarkeit galt für
> **eingeschleuste** Kontrollsegmente, die wie echte Arbeit aussehen
> mussten. Die Stichprobe zieht **echte** Arbeitseinheiten, und die sind
> von echter Arbeit nicht zu unterscheiden, **weil sie es sind**. Die
> Anforderung entfällt, statt erfüllt zu werden.
>
> **Der Satz „das Messgerät steht, der Verkehr fehlt" stand hier seit
> dem 2026-08-27 und stimmt seit dem 2026-09-02 nicht mehr:** Das
> Messgerät ist mit seinem Gegenstand gegangen.
>
> ⚑ **Die adversariale Ebene fand Fund 42:** Das Bisektions-Spiel nannte
> systematisch die **falsche Layer** und hätte damit den Betrüger
> freigesprochen und den ehrlichen Checker geschlachtet. Behoben in
> v0.4.0, siehe Changelog.

Verifikations-Subsystem: Redundanzvergleich (Stufe 1) und
Bisektions-Spiel (Stufe 2). Referenzimplementierung von Whitepaper Kap. 6.4–6.9
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

### v0.27.0 (2026-10-29): Nachrechnen als Prefill

- ⚑ **`verlauf_nachrechnen` prüft als Prefill** (Festlegung des
  Projektinhabers vom 2026-10-29, VeriLLM): `verlauf_als_prefill` rechnet
  alle Positionen eines Fensters Ebene für Ebene
  (`vorbereiten_stapel_mit_ebenen`) und vergleicht nach jeder Ebene alle
  Ausgänge mit der Spur; die erste Abweichung ist die kleinste nach
  Position und Eintrag, wie beim schrittweisen Weg. Der schrittweise Weg
  bleibt als `verlauf_schrittweise`: Rückfall für Modelle ohne Ausgang je
  Ebene im Stapel, und Gegenprobe.
- **Erst die Konformitätsprobe, dann umgestellt**: Test
  `der_prefill_ist_bitgleich_zum_schrittweisen_rechnen` in `myl-pod`, 520
  Positionen (über die Fenstergrenze), Ketten wie ein Shard sie rechnet:
  jede Layer jeder Position gleich, dieselben Logits, eine Fälschung
  hinter der Grenze an derselben Stelle. Gemessen im Testprofil: 6,9 s
  gegen 20,8 s, rund dreimal schneller.

### v0.26.0 (2026-10-29): Beleg und Schiedsrechnung

- ⚑ **`schiedsbeleg(modell, tokens, position, layer)`**: die Eingänge der
  strittigen Layer für die Positionen bis zur strittigen, aus den Tokens
  gerechnet. Bis dorthin war der Anfechter mit dem Pod einig, also trifft
  jeder Eingang den Hash, den der Pod bezeugt hat. Für Layer null leer.
- ⚑ **`schied_nachrechnen`** (Whitepaper 6.6): das Komitee rechnet **eine**
  Layer an **einer** Position nach. Die Eingänge (Einbettungen für Layer
  null, sonst der Beleg) werden gegen die Hashes des Pods geprüft; trägt
  der Beleg nicht, verliert der Anfechter (Beweislast). `true` heisst, der
  Pod rechnete falsch.
- Geprüft im Test mit echtem Modell in `myl-pod`.

### v0.25.0 (2026-10-29): Der Nachrechner liefert die Logits

- ⚑ **`verlauf_nachrechnen` gibt `Verlaufspruefung { ergebnis, logits }`**
  zurück (Festlegen vor Aufdecken, Festlegung des Projektinhabers vom
  2026-10-29): dazu den Hash der Logits an der letzten Position
  (`logits_hash`, je Wert `i32` in Little Endian). Er steht in keiner
  Spur; zwei Checker, die ihn gleich aufdecken, haben beide bis zum
  LM-Kopf gerechnet. Nach einer Abweichung rechnet der Nachrechner mit den
  eigenen Werten weiter, die Logits sind also immer die ehrlichen.

### v0.24.0 (2026-10-29): Der Checker rechnet einen Verlauf ab den Tokens nach

- ⚑ **`verlauf_nachrechnen(modell, tokens, ketten)`** (Whitepaper 3.4 und
  6.6): rechnet eine Sitzung ab Position null Layer für Layer über
  `integer_llm_runtime` nach, mit eigenem KV-Cache, und vergleicht jeden
  `activation_hash` mit der Spur, die der Pod bezeugt hat. Ergebnis
  `Verlaufsergebnis::Gleich` oder die erste `Abweichung { position, eintrag }`;
  der Eintrag ist die Layer plus eins (null ist der Eingang). Eine Spur
  falscher Länge ist eine Abweichung. Damit ist der fehlende KV-Verlauf beim
  Nachrechnen hinter Position null geschlossen.
- `CheckError::Verlauf { position }` für ein Token ausserhalb des Vokabulars.
- Gerechnet wird nicht über den Code des Pods, sondern über die Laufzeit
  selbst; der Test `ein_checker_rechnet_den_verlauf_ab_den_tokens_nach` in
  `myl-pod` prüft beide gegeneinander. 122 Tests grün.

### v0.23.4 (2026-10-28): Die Lieferung wird geprüft wie im Knoten

- `checker::pruefe_spurantwort` ruft `myl_types::lieferung_pruefen`
  (SHARED_TYPES 0.55.0) statt einer eigenen Abschrift derselben Prüfung;
  Knoten und Prüfer können so nicht auseinanderlaufen. Die Fehler bleiben
  dieselben. Kein anderes Verhalten.
- `myl-verifier` 0.23.4.

### v0.23.3 (2026-10-23): RUSTSEC-2026-0385 in der Sperrdatei (Fund 599)

- `ctutils` 0.4.2 auf 0.4.5 (Maskenbildung nicht in konstanter Zeit). Aus
  dem Sicherheitsaudit vom 2026-10-23; die Kiste selbst ist dort ohne Befund (Indizes vor dem Zugriff
  geprüft, keine Panik an fremden Daten).

### v0.23.2 (2026-10-22): Eine Werkzeugkette für alle Kisten; Fund 597

- `rust-version` auf 1.99 in `myl-verifier`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  ⚑ **Dazu Code, und zwar mit Absicht:** Mit `rust-version = "1.99"` gibt Clippy
  zwei Regeln frei, die es bei 1.85 zurückhielt (`manual_is_multiple_of`,
  `chunks_exact` mit fester Größe). Festlegung des Projektinhabers:
  umschreiben, nicht abschalten. Hier: `nachrechner.rs` 2 Zeilen.
  `x % n == 0` wird `x.is_multiple_of(n)`, `chunks_exact(N)` wird
  `as_chunks::<N>()`. Für `n ≠ 0` bitgleich; jede Stelle mit variablem
  Teiler ist einzeln angesehen und vorher gegen null geschützt (bei
  `is_multiple_of(0)` stünde `x == 0`, wo `x % 0` abbricht).
- 📌 **Fund 597:** `tests/trainingspruefung.rs` fragte die Erlaubnis
  `MYL_OHNE_ARTEFAKTE` nicht ab und sprang nur ab, wenn das Artefakt fehlte.
  Wo es liegt, rechnete sie trotz Erlaubnis am Probeartefakt, 58 s und
  14,8 GB Speicher; in einem Linux-Behälter mit 8 GB wurde sie dabei mit
  SIGKILL beendet. Jetzt über `myl_pod::artefakte::vorhanden` (mit Erlaubnis
  überspringen, ohne Erlaubnis und ohne Artefakt auffallen); mit Erlaubnis
  0,00 s.
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen. Nach dem
  Umschreiben: Clippy mit `-D warnings` still (die Kerne über alle vier
  Merkmale), `cargo test` rc=0 in allen zehn berührten Kisten (die Kerne auch
  mit `cpu-simd`), Konformität mit `sparrow-0.6b-int8` 55/55, die vier
  Abdrücke unverändert (`cb696efd3817e575` über 24 Vektoren), Gleitkomma-Audit
  grün.

### v0.23.1 – 2026-10-21 (die Modelle heißen nach Tieren)

Die Projektmodelle heißen seit heute nach Tieren, mit Größe und Speicherform als Anhängsel (Festlegung des Projektinhabers): `myelith-0.6b` ist `sparrow-0.6b-int8`, `myelith-4b` `pigeon-4b-int8`, `myelith-30b-a3b` `magpie-30b-a3b-int8`, `myelith-35b-a3b` `raven-35b-a3b-int8`. In den Dateien eines Artefakts steht kein Name, also bleiben Abdrücke und Register gültig. Nur eine Probe.

### v0.23.0 – 2026-09-14 (eine Position hinter der ersten braucht den KV-Verlauf)

⛔️ **Fund 373:** Der Nachrechner rechnete ein Segment an Position `p` mit
leerem KV-Speicher und kam für jede Position hinter der ersten zu einer
anderen Spur als ein ehrlicher Shard, der die Positionen davor gehalten
hat; geprüft war nur Position 0. Er lehnt jetzt ab
(`CheckError::OhneVerlauf`), statt eine Abweichung zu erzeugen, die keine
ist und einen ehrlichen Shard schlüge. Wie der Verlauf in den Streitfall
kommt, ist eine Protokollfrage (Bericht `Kontext-1M-Netzmodell-2026-09-14.md`).

### v0.22.1 – 2026-09-10 (die Artefakte heissen nach dem Modell, das sie sind)

**Umbenennung, keine Verhaltensänderung.** Die Artefakte unter
`INTEGER_LLM/artifacts/` heissen seit heute `myelith-0.5b`,
`myelith-7b`, `myelith-4b` und `myelith-30b-a3b`; die Pfade in einer Prüfung
sind nachgezogen.

⚑ **Ein Artefakt ist nicht das Basismodell, sondern das Modell, mit dem
dieses Projekt rechnet.** Es trägt deshalb einen eigenen Namen; die
Basismodelle unter `models/` behalten ihre und stehen weiter mit
Herkunft im Katalog.

⚑ **Der Konformitätswert ist unverändert**, gemessen nach dem Umbau:
`894d8357ae92b5c1` über sechs Vektoren und `6da384ba301b9454` über
siebzehn. **Die Namen stehen in keiner Bytefolge, die gehasht wird.**

### v0.22.0 – 2026-09-06 (die Belege wandern in die geteilten Typen)

`Schuldbeleg` und `Anfechtungsbeleg` liegen jetzt in `myl-types` und
werden hier weiterexportiert; für Aufrufer ändert sich nichts.

⚑ **Der Grund ist Fund 192.** Der Konsens muss einen Beleg **auf den
Draht** legen können, damit ein Schuldspruch im Block prüfbar ist, und
`myl-consensus` darf nicht an VERIFICATION hängen. Eine Kopie hier
hätte dieselbe Aussage an zwei Orten getroffen, und die erste
Abweichung zwischen ihnen wäre von einem Rechenfehler nicht zu
unterscheiden.

### v0.21.0 – 2026-09-06 (der Abdruck kommt aus einer Hand)

`delta_abdruck` ruft `Shardgewichte::deltas` statt eine eigene Schleife
zu fahren.

⚑ **Bei einer Gemischebene hängt die Reihenfolge an den
Expertennummern**, und wer sie hier nachbaute, baute die Konsensordnung
ein zweites Mal. Ein Experte, den nur eine Seite gewählt hat, fällt
dadurch auf: Er steht in der einen Liste und in der anderen nicht.

### v0.20.0 – 2026-09-05 (die Prüfung von Trainingssegmenten)

`trainingspruefung` bringt den Trainingsweg auf dieselbe Mechanik wie
die Inferenz: Zwei Pods, ein Vergleich, bei Abweichung sagt die **Spur**,
welcher Shard es war, und ein Nachrechner entscheidet.

### ⚑ Der Nachrechner braucht nichts, was der Beschuldigte liefert

Das ist der Unterschied zur Inferenzprüfung, und er ist ein Vorteil.
Dort beginnt die Nachrechnung mit den **Eingangsaktivierungen aus der
Spur des Beschuldigten**; wer sie fälscht, verschiebt die Prüfung.

Ein Trainingssegment ist vollständig bestimmt durch die Gewichte der
Modellfassung, die Charge, die Lernrate und die Schrittzahl. Alle vier
stehen im Segment oder folgen aus dem verankerten Korpus.
`Modelltrainingsauditor` fängt beim Artefakt an und rechnet den ganzen
Lauf.

⚑ **Der Preis ist der ganze Lauf statt eines Shards.** Kein Versäumnis:
Der Δ eines Shards hängt über den Rückwärtsweg an allen Shards hinter
ihm, und wer nur einen nachrechnete, müsste dessen Ausgangsgradienten
vom Beschuldigten nehmen.

### ⚑ Ein Vergleich sagt dass, nie wer

Zwei Pods, die sich widersprechen, sind zwei Behauptungen; erst das
Nachrechnen ist eine dritte, unabhängige. Wer aus dem blossen
Widerspruch eine Schuld ableitete, bestrafte in der Hälfte der Fälle den
Ehrlichen. `Trainingsbefund::Uneinig` trägt deshalb keine Schuld.

⚑ **Und verschiedene Aufträge sind kein Streitfall.** Zwei Pods mit
verschiedener Charge oder Lernrate haben verschiedene Arbeit getan; ihre
Ergebnisse zu vergleichen wäre sinnlos, und eine daraus abgeleitete
Schuld wäre erfunden.

### ⚑ Die Spur ersetzt die Bisektion, sie ergänzt sie nicht

Die Spur ist so lang wie der Pod Shards hat, also vier bis acht
Einträge; sie vollständig zu übertragen kostet weniger als eine einzige
Bisektionsrunde. Erst **innerhalb** eines Shards, über seine Ebenen,
lohnt das Halbieren.

**Gemessen** (`tests/trainingspruefung.rs`): Der Nachrechner bestätigt
alle vier Shards eines ehrlichen Pods und widerlegt genau den
gefälschten. `myl-pod` steht dabei als Dev-Abhängigkeit: Der Nachrechner
**ruft** den Pod nicht, er wird nur **gegen** ihn gehalten.

### v0.19.0 – 2026-09-03 (der Kopf sagt endlich, was der Code seit einem Tag tut)

**Kein Code, nur eine Berichtigung, und sie ist Fund 158.** A1 hat am
2026-09-02 rund 1 700 Zeilen entfernt: `kontrollsegmente`,
`unterscheider` **und das Messgerät `unterscheidbarkeit` selbst**.
Dieser Kopf führte sie einen Tag länger.

⚑ **Dieselbe Form wie Fund 151:** Eine Entscheidung entzieht einem Punkt
den Boden, und der Punkt steht weiter da, weil ihn niemand daraufhin
angesehen hat. Betroffen waren ausserdem zwei Zeilen der internen
Abhak-Liste, eine davon mit einem Verweis auf eine Datei, die es nicht
gibt, und der offene GATEWAY-Punkt zur Ununterscheidbarkeit.

**Die Regel daraus:** Wer eine Entscheidung umsetzt, die etwas
**entfernt**, geht danach die Punkte durch, die davon lebten. Der
Grabstein im Code ist die halbe Arbeit; die Liste ist die andere.

### v0.18.0 – 2026-09-02 (die Kontrollsegmente entfallen, rund 1 700 Zeilen)

**Entscheidung A1 des Projektinhabers, recherchiert und gebaut.** Drei
Module gehen: `kontrollsegmente` (Mechanik und Vorrat), `unterscheider`
(die Spur der Mechanik, Fund 58) und `unterscheidbarkeit` (das Messgerät
für Kap. 6.7 Anforderung 1), dazu das Beispiel `a11_kurve` und sechs
Tests in `tests/simulation.rs`.

⚑ **Der Grund ist kein Mangel an ihnen, sondern einer an der Rolle, die
sie voraussetzten.** Kap. 6.7 legt die Einschleusung den Gateways zu,
und das trägt nicht: Ein Gateway hat nichts zu verlieren, ein
selbstbetriebenes speist gar nichts ein. **Truebit** schließt den
Auftraggeber aus demselben Grund ausdrücklich vom Wissen aus,
**Proof of Sampling** ersetzt ihn durch ein BFT-Zufallsverfahren, und
Myelith hat dieses Verfahren seit Punkt 44 im Block.

⚑ **Und beim Nebeneinanderlegen stand der Mechanismus zweimal da.**
Kontrollsegmente und Stufe 2 haben dieselbe Form, denselben Zweck und
decken denselben Angriff, den einmaligen Eingriff bei beiden Pods in
einer Hand. Nur einer von beiden braucht einen Einspeiser, einen
endlichen Vorrat und eine Ununterscheidbarkeit, die er nachweislich
nicht herstellen kann.

**Was damit geschlossen ist:** die Einspeisung der Kontrollsegmente in
CONSENSUS, die drei Befunde zu ihrer Ununterscheidbarkeit, die offene
Entwurfsfrage dazu und der Punkt 3.2 dieses Bereichs, der eine
Ununterscheidbarkeit verlangte, die niemand herstellen kann.

**Anforderung 1 wird erfüllbar, indem sie entfällt.** Unauffällig zu
sein ist eine Eigenschaft der Daten und von keinem Code herstellbar;
eine gezogene **echte** Arbeitseinheit ist von echter Arbeit nicht zu
unterscheiden, weil sie es ist.

**Was nicht verlorengeht, steht als Notiz in `lib.rs`, wo die Module
standen:** Fund 58 als Erkenntnis (ein endlicher Vorrat in einem
unbegrenzten Strom wiederholt Ids, echte Arbeit nie), der Grund für die
Unerfüllbarkeit von Anforderung 1, und Fund 138 mit der Rechnung, die
die Zusammenlegung korrigiert hat.

**Und eine Voraussetzung weniger in Kap. 6.8:** Die multiplikative
Rechnung setzte Unabhängigkeit von Stichprobe und Einschleusung voraus.
Mit einer Ziehung gibt es nichts mehr, was korreliert sein könnte. **Eine
Voraussetzung, die wegfällt, ist besser als eine, die geprüft wird.**

**Ältere Einträge** (v0.17.0 und davor, 16 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- VERIFICATION/README/README.md`.
