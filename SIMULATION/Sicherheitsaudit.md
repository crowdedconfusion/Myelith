# Sicherheitsaudit

**Stand:** 2026-08-27
**Grundlage:** die Angreiferklassen aus Whitepaper Kap. 5.6 und 9.2, die
Sicherheitsargumente aus Kap. 6.8 und Anhang B, sowie die Funde 41 bis 53
dieses Projekts.

**Was dieses Dokument ist:** eine Bestandsaufnahme, welche Angriffe heute
abgewehrt werden, welche gemessen offen sind und welche niemand geprüft
hat. Es ist **kein externes Review** (K5, K9) und ersetzt keines.

**Der wichtigste Satz vorweg:** Von den dreizehn Angriffsklassen unten
sind **acht abgewehrt und belegt**, **drei offen mit gemessener Lücke**
und **zwei ungeprüft**. Keine der Lücken ist neu entdeckt worden, ohne
dass sie hier steht.

---

## 1. Zusammenfassung nach Schwere

| # | Angriff | Stand | Beleg / Lücke |
|---|---|---|---|
| A1 | Falsches Rechenergebnis | ✅ | Stufe 1 (Redundanz), Stufe 2 (Bisektion), Kontrollsegmente |
| A2 | Rogue-Key auf Aggregatsignaturen | ✅ | Proof-of-Possession, Fund 27, `rogue_key.rs` |
| A3 | Koordinator fälscht PoI-Bündel | ✅ | 7 Angriffe in `koordinator_byzantinisch.rs`, Fund 52 geschlossen |
| A4 | Double-Signing im BFT | ✅ | Beweis mit BLS, 9 Angriffe auf das Polka-Zertifikat |
| A5 | Manipulierte Aktivierungen im Pod | ✅ | Eingangs-Hash-Prüfung, Fund 41 geschlossen |
| A6 | Self-Dealing in der Subventionsphase | ✅ | `s < c/(1−c)` gegen das untere Bandende, Fund 49 |
| A7 | Verbrauchs-Stoß mit Ausstieg | ✅ | Burn-Cap je Adresse, gemessen |
| A8 | Parametervorschlag, der Invarianten bricht | ✅ | Registry prüft **vor** der Abstimmung |
| A9 | **Eclipse: Umzingelung eines Knotens** | ⚠️ | **Fund 53 geschlossen** (2026-08-24); Restbedingung: ehrlicher Bootstrap-Knoten |
| A10 | **Latenzwerte fälschen** | ⚠️ | Signatur wird **geprüft** (2026-08-25); Schlüsselherkunft im Probelauf ableitbar |
| A11 | **Kontrollsegmente erkennen** | ⚠️ | **Fund 58** gemessen und am 2026-08-27 geschlossen (Vorrat als Parameter mit Schranke); Prompt-Profil bleibt offen |
| A12 | Kollusion beider Pods | ⚠️ | Schranke gemessen (β^2k trifft), Gegenmaßnahme unbelegt |
| A13 | Angriff auf die Krypto-Primitiven | ❓ | nie extern geprüft (K5) |

---

## 2. Abgewehrt und belegt

### A1 Falsches Rechenergebnis

Drei Stufen, und sie decken verschiedene Fälle ab:

- **Stufe 1 (Redundanz)** greift, wenn nur *ein* Pod lügt. Binärer
  Vergleich ohne Toleranzfenster; es gibt keinen Bereich, in dem sich
  eine Manipulation verstecken ließe.
- **Stufe 2 (Bisektion)** grenzt im Streitfall die abweichende Layer ein.
  ⚑ Sie tat das bis zum 2026-08-23 **falsch** (Fund 42: sie nannte `d−1`
  statt `d`, hätte den Betrüger freigesprochen und den ehrlichen Checker
  geschlachtet, in 15 von 16 Fällen). Behoben und über jede Position
  jeder Spurlänge geprüft.
- **Kontrollsegmente** greifen gegen den **einmaligen** Eingriff, auch
  wenn der Angreifer beide Pods hält. Siehe aber A11.

### A2 Rogue-Key

`fast_aggregate_verify` ist ohne Besitznachweis angreifbar, und die
Konstruktion `pk_rogue = g₁^x · pk_opfer⁻¹` besteht `key_validate()`.
Nachgewiesen, nicht vermutet; behoben nach
draft-irtf-cfrg-bls-signature §3.3. Die Regression hält **beide**
Tatsachen fest: dass der Rogue Key die Validierung besteht **und** dass
der Besitznachweis ihn ausschließt.

### A3 Koordinator fälscht

Der Koordinator ist die einzige Stelle im Pod, die für alle spricht.
Abgewehrt: Anspruch nachträglich erhöhen, Segmente hinzudichten,
weglassen, umsortieren, fremdes Bündel einreichen, allein oder zu zweit
unterschreiben.

⚑ **Fund 52 dabei:** Bis zum 2026-08-24 verifizierte **kein** Bündel aus
dem Pod, weil Pod und Konsens über verschiedene Botschaften redeten. Die
Richtung war die gute (abgelehnt statt angenommen), aber der
Vergütungspfad war unbenutzbar. Geschlossen durch eine Signaturrunde über
das fertige Bündel, in der **jedes Mitglied den Anspruch prüft, bevor es
unterschreibt**.

### A6 Self-Dealing

⚑ **Fund 49:** Die Grenze `s < c/(1−c)` ließ sich in zwei je zulässigen
Schritten verschieben, weil `c` abstimmbar war. Geschlossen: geprüft wird
gegen das **untere** Ende des Bandes aus Anhang B.4 (c = 0,6 ⇒ s < 1,5),
und die Prüfung nimmt kein `c` mehr entgegen.

### A7 Verbrauchs-Stoß

Kap. 5.6 nannte den Burn-Cap je Adresse seit v0.1 als Gegenmittel; **er
war nicht implementiert.** Jetzt ist er es. Gemessen bei einer EMA von
20 000 MYL: Ein Stoß von 200 000 MYL hebt die EMA ohne Deckel um
11 612 MYL, mit Deckel gar nicht.

**Was er nicht leistet, gehört dazu:** Zwanzig Adressen mit je eigener
Deckung erreichen denselben Stoß. Der Deckel macht daraus eine
**Kapitalfrage statt einer Sybil-Frage** — die MYL müssen wirklich da
sein.

---

## 3. Offen, mit gemessener Lücke

### A9 Eclipse (Fund 53) — geschlossen, mit benannter Restbedingung

**War gemessen (2026-08-24, früh):** Zwanzig Sybil-Identitäten verbinden
sich mit demselben Opfer, **alle zwanzig werden angenommen**.
`build_swarm` hatte kein `connection_limits`, kein Peer-Scoring, keine
Diversitätsregel.

**Warum das teuer war:** Wer beliebig viele Verbindungen aufbauen darf,
füllt die Peer-Menge des Opfers und entscheidet danach, **welche
Nachrichten es sieht**, nicht durch Fälschung, sondern durch Auswahl.
Die Sicherheit dieses Protokolls hängt daran, dass Checker fremde
Segmente **beobachten**; wer die Beobachtung steuert, steuert die
Verifikation.

**Behoben (2026-08-24, myl-net v0.4.0):** `src/limits.rs` und
`src/scoring.rs`. Der Kern ist eine Trennung: Eingehende Verbindungen,
die der Angreifer wählt, sind bei 48 gedeckelt; ausgehende, die der
Knoten selbst wählt, haben ein eigenes Budget von 16, und die
Gesamtgrenze ist die Summe. **Eine Flut kann die ausgehenden Plätze
deshalb nicht aufzehren.** Dazu vier eingehende je Adressbereich
(IPv4 /24, IPv6 /64) und Gossipsub-Peer-Scoring mit Graylist.

Belegt in `tests/eclipse_sybil.rs` als Kette: eingehend gedeckelt,
ausgehendes Budget unter Flut frei, und über die selbst gewählte
Verbindung kommt auch etwas an.

**⚠️ Die Restbedingung, ausdrücklich:** Garantiert ist, dass der Knoten
**wählen darf**, nicht dass er **richtig wählt**. Adressen kommen aus der
Bootstrap-Liste und aus Kademlia. **Der Eclipse-Angriff reduziert sich
damit auf die Bedingung: Die Bootstrap-Liste enthält mindestens einen
ehrlichen Knoten.** Wer sie stellt, umgeht die Verteidigung. Deshalb
steht A9 hier auf ⚠️ und nicht auf ✅.

**Zwei Funde dabei:**

- **Fund 54:** Die erste Fassung setzte die IP-Kolokationsschwelle auf 4
  und schaltete damit den ehrlichen Knoten mit stumm (−245 bei Graylist
  −80). Die Zahl war zusätzlich wirkungslos, weil die
  Adressbereichsgrenze bereits schärfer bindet. Eine Härtung, die
  niemand durchgerechnet hat, ist eine Vermutung mit Vorzeichen.
- **Fund 55:** Der dokumentierte Weg für die Nutzlastprüfung
  (`report_with`) war über `run_node` nicht erreichbar. Behoben mit
  `run_node_mit()`.

**Weiterhin offen:** Diversität je **ASN** statt je Adressbereich. Ein
Angreifer mit einem großen Provider bekommt viele /24. Die Metadaten
liegen in `myl-types/node_metadata.rs`; es fehlt eine vertrauenswürdige
Quelle für die Zuordnung.

### A10 Latenzwerte fälschen — Prüfung vorhanden, Schlüsselherkunft offen

**War:** `myl_types::LatencyAttest` trug ein `signature`-Feld, das **im
ganzen Projekt niemand verifizierte**, und niemand erzeugte ein Attest.

**Warum das schlimmer war als ein fehlendes Feld:** Ein ungeprüftes
Signaturfeld ist gefährlicher als gar keines, weil ein Leser es für
einen Schutz hält. Die Latenzwerte gehen ins Geo-Clustering der Pods;
wer sie frei setzen kann, sucht sich seine Pod-Nachbarn aus, und das ist
die Vorstufe zur Kollusion beider Pods (A12).

**Behoben (2026-08-25):**

- `LatencyAttest::sign` und `::verify` in `myl-types`, 7 Tests. Sie
  fehlten schlicht: Es gab `signable_bytes()`, aber nichts, was damit
  signierte oder prüfte.
- `myl_node::Validatorsatz` ordnet Kennung zu Schlüssel und liefert den
  **Grund** einer Ablehnung, nicht nur ein Nein.
- `ProtokollValidator` prüft Atteste. Dort stand vorher `_ => true`.
- Der Knoten **erzeugt** Atteste aus seinen tatsächlich gemessenen
  Latenzen, nicht aus erfundenen Zahlen.

**Live belegt, drei Knoten:** Alpha und Beta kennen einander und nehmen
gegenseitig an; ein dritter, der in keiner Liste steht, bekommt alle
seine Atteste verworfen und verwirft selbst alle fremden.

**⚠️ Warum nicht ✅:** Im Probelauf werden die Schlüssel aus den
Teilnehmernamen abgeleitet, die der Koordinator ohnehin verteilt. **Wer
die Namen kennt, kann die Schlüssel ableiten** und damit in fremdem
Namen signieren. Für eine Trockenübung ist das hinnehmbar, für ein
echtes Netz nicht.

Die Trennlinie liegt **nicht im Prüfcode**, sondern in der Herkunft der
Schlüssel: Dieselbe Prüffunktion arbeitet unverändert gegen echte
Schlüssel, sobald die Validator-Registrierung zu Genesis steht, mit
Besitznachweis (Fund 27). Das ist dieselbe Voraussetzung, die auch die
BFT-Runden brauchen.

### ⚑ A11 Kontrollsegmente erkennen — eine Spur gemessen und geschlossen

Kap. 6.7 verlangt Ununterscheidbarkeit als **erste**
Konstruktionsanforderung. Die Mechanik steht seit myl-verifier v0.5.0;
die Ununterscheidbarkeit ist eine **Eigenschaft der Daten, nicht des
Codes**, und das Whitepaper führt sie als offene Messfrage (Kap. 11,
Punkt 5).

**Was sich trotzdem messen ließ**, und das war der Ertrag vom
2026-08-25: ob die **Einschleusung selbst** eine Spur hinterlässt,
unabhängig vom Inhalt der Prompts. `myl-verifier/src/unterscheider.rs`
baut den Angriff, statt ihn zu beschreiben.

⚑ **Fund 58: Ein zu kleiner Vorrat verrät sich vollständig.**

Der Vorrat ist endlich, der Auftragsstrom nicht. Wird öfter
eingeschleust, als der Vorrat verschiedene Segmente hält, wiederholen
sich Ids. **Echte Arbeit wiederholt sich nie**, also ist jedes zweite
Auftreten einer Id ein Beweis, kein Verdacht. Ein Miner braucht dafür
keine Statistik, keine Schwellenwerte und keine Annahme über Prompts,
nur ein Gedächtnis.

Gemessen bei γ = 2 % über 100 000 Aufträge:

| Vorrat | erkannte Kontrollen | Fehlalarme |
|---|---|---|
| 64 | **96,8 %** | 0 |
| 256 | 87,2 % | 0 |
| 1024 | 48,8 % | 0 |
| 2048 | 0 % | 0 |

**Null Fehlalarme ist der schlimmste Teil.** Der Angreifer geht kein
Risiko ein: Er rechnet die erkannten Kontrollen ehrlich und manipuliert
den Rest. Der Mechanismus, der als einziger gegen den **einmaligen**
Eingriff wirkt, wäre damit wirkungslos, und mit ihm A1 für den Fall,
dass der Angreifer beide Pods hält.

**Was den Fund schärfte: γ war ein Governance-Parameter, die
Vorratsgröße keiner.** Die einzigen je geschriebenen Werte standen in
Tests: 10 und 100. Bei γ = 2 % trägt ein Vorrat von 100 genau 5 000
Aufträge. **Die Zahl, an der die Wirksamkeit hängt, war nirgends
festgelegt.**

**Geschlossen am 2026-08-27.** Vorratsgröße und Beobachtungsfenster sind
Governance-Parameter, und eine Invariante bindet beide an γ:
`Vorrat ≥ ⌈Fenster · γ⌉`, geprüft am **Vorschlag** statt nach der
Abstimmung. Der Wert ist abstimmbar, das Unterschreiten der Schranke
nicht — dieselbe Bauart wie beim Mindest-Stake.

⚑ **Der Zug, gegen den die Invariante gebaut ist, sieht wie eine
Verschärfung aus:** γ von 2 auf 4 Prozent heben, ohne den Vorrat
mitzuziehen. Das verdoppelt die Einschleusungen bei gleichem Vorrat und
halbiert die Reichweite. Vorher wäre er zulässig gewesen. **Das ist die
übertragbare Lehre:** Ein Parameter, der eine Schutzwirkung verstärkt,
kann eine andere aufzehren, und wer nur den geänderten ansieht, sieht es
nicht.

**Das Beobachtungsfenster musste ein eigener Parameter werden.** „Der
Vorrat muss größer sein als die Zahl der Einschleusungen" ist für einen
unbegrenzten Auftragsstrom von **keinem** endlichen Vorrat erfüllbar;
erst ein Fenster macht die Bedingung entscheidbar, und wer es senkt,
senkt die Schranke.

**⚠️ Warum weiterhin nicht ✅:** Geschlossen ist genau **eine** Spur, die
der Mechanik. Länge, Timing und Kontextprofil der Prompts bleiben
ungemessen; dafür braucht es echte Prompt-Verteilungen, und die
entstehen erst im Betrieb. Das bleibt die offene Messfrage aus Kap. 11
Punkt 5. **Und der Anfangswert ist vorläufig:** 2 048 ist der gemessene
Wert, 100 000 die Stromlänge der Messung; die Auftragsrate des Netzes
ist ungemessen.

---

## 4. Teilweise belegt

### A12 Kollusion beider Pods

Anhang B.2 gibt `P_koll ≈ β^{2k}` an. **Gemessen an der echten
Zuteilung** (β = 50 %, k = 4, 10 000 Zuteilungen): 3,900 · 10⁻³ gegen
3,906 · 10⁻³ der Formel, Übereinstimmung auf drei Stellen.

**Was das nicht belegt:** Die Formel unterstellt gleichverteilte
Ziehungen. Anhang B.2 vermerkt selbst, dass die geografische Clusterung β
**lokal erhöht**, und verschiebt die Analyse auf Meilenstein M1. Die
Messung oben läuft über künstlich gleichverteilte Regionen; ein
realistisches Clusterbild ist nicht geprüft.

**Zudem:** A10 ist der Hebel, mit dem ein Angreifer β lokal selbst
erhöht.

---

## 5. Ungeprüft

### A13 Die Krypto-Primitiven

Dass `myl-types` gegen die RFC-9381-Testvektoren stimmt, ist geprüft.
Dass die **Verwendung** trägt, war bis zum 2026-08-23
Eigenbau-Beurteilung; seither gibt es ein schriftliches Bedrohungsmodell
je Signaturverwendung
(`SHARED_TYPES/README/Signatur-Bedrohungsmodell.md`).

**Es ersetzt kein externes Review** (K5). Fund 27 ist der Beleg, warum
nicht: Der Schutz stand in der Dokumentation, bevor er im Code stand, und
niemand hat den Satz gegen die Literatur geprüft.

---

## 6. Was die Simulation zusätzlich zeigt

`SIMULATION/myl-simulation/tests/durchlauf.rs` fährt den Weg eines
Segments durch alle Schichten. Ergebnisse:

| Prüfung | Ergebnis |
|---|---|
| ehrlicher Durchlauf | keine schweren Befunde |
| falscher Pod → Urteil → Ledger | gefunden, 100 % Slash gebucht, Kopfgeld gezahlt |
| Prägekurve über 200 Epochen | Verteilung stets exakt gleich der Prägung |
| zu kleines Netz | meldet die fehlende Redundanz, statt still weiterzulaufen |
| Burn-Cap gegen den Stoß | greift, gemessen |

**Ein Befund über die Simulation selbst:** Der 200-Epochen-Durchlauf
erreicht den Burn-Cap **nie**, weil die EMA sich der Schwelle von unten
nähert. Ein Test, der den interessanten Zweig nicht betritt, prüft ihn
nicht — dieselbe Falle wie beim ersten Pod-Fuzzer. Deshalb steht der
Stoß-Fall als eigener Test daneben.

---

## 7. Woran zuerst zu arbeiten wäre

Nach Schadenshebel, nicht nach Aufwand:

1. ~~**A9 Verbindungsgrenze** (Fund 53)~~ — erledigt am 2026-08-24.
   Nachtrag zur damaligen Aufwandsschätzung „der Aufwand ist klein,
   `connection_limits` ist eine Behaviour-Zeile": Das stimmte für die
   Zeile und nicht für die Aufgabe. Die Arbeit steckte im Herleiten der
   Zahlen und im Ausrechnen, wen sie treffen, und genau dort lag Fund 54.
2. ~~**A10 Attest-Signatur prüfen**~~ — erledigt am 2026-08-25. Die
   Vermutung „entsteht ohnehin mit dem Knoten-Binary" stimmte für die
   **Stelle**, nicht für die Sache: Der `PayloadValidator` war da, aber
   `LatencyAttest` hatte weder `sign` noch `verify`. Die Prüfung, für
   die alles vorbereitet schien, brauchte erst ihre Primitive.
   **Offen bleibt die Schlüsselherkunft**, und die hängt an derselben
   Validator-Registrierung wie die BFT-Runden.
3. ~~**A11 statistische Analyse**~~ — teilweise erledigt am 2026-08-25,
   die Behebung am 2026-08-27. Die Spur der **Mechanik** ist gemessen
   und geschlossen (Fund 58: Vorratsgröße und Beobachtungsfenster als
   Governance-Parameter mit einer Invariante, die beide an γ bindet);
   die Spur der **Daten** (Länge, Timing, Kontextprofil) braucht echte
   Prompt-Verteilungen aus dem Betrieb.

   Nachtrag zur damaligen Einordnung „offene Messfrage": Das stimmte
   für den Teil, an den alle gedacht hatten, und verdeckte den Teil, der
   schon heute messbar war. **Eine als offen abgelegte Frage wird nicht
   mehr gestellt**, und genau deshalb lag der Fund über Monate offen.
4. **A13 externes Review.** Vor dem Mainnet, nicht danach.

**Was ausdrücklich nicht auf dieser Liste steht:** ein weiterer
Testdurchlauf derselben Art. Die acht abgewehrten Klassen sind belegt;
mehr Tests derselben Machart erhöhen die Zahl, nicht die Sicherheit.
