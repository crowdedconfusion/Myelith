# Sicherheitsaudit

**Stand:** 2026-08-28
**Grundlage:** die Angreiferklassen aus Whitepaper Kap. 5.6 und 9.2, die
Sicherheitsargumente aus Kap. 6.8 und Anhang B, sowie die Funde 41 bis 53
dieses Projekts.

**Was dieses Dokument ist:** eine Bestandsaufnahme, welche Angriffe heute
abgewehrt werden, welche gemessen offen sind und welche niemand geprüft
hat. Es ist **kein externes Review** (K5, K9) und ersetzt keines.

**Der wichtigste Satz vorweg:** Von den fünfzehn Angriffsklassen unten
sind **zehn abgewehrt und belegt**, **drei offen mit gemessener Lücke**
und **zwei ungeprüft**. Keine der Lücken ist neu entdeckt worden, ohne
dass sie hier steht.

⚑ **Neu und am selben Tag geschlossen: A15 (2026-08-28).** Die erste
Klasse dieses Dokuments, die nicht aus einem Angriffsmodell kam, sondern
aus einer Codelektüre: Der Merkle-Baum war nicht injektiv, zwei
verschiedene Blattfolgen konnten dieselbe Wurzel tragen. Sie stand einen
halben Tag als latent und ist behoben.

⚑ **Der Grund für die Eile steht in der Klasse selbst und gilt über sie
hinaus:** Es war der einzige Punkt dieses Dokuments, dessen
Behebungskosten mit jedem Betriebstag gestiegen wären, ohne dass jemand
etwas falsch macht. Eine Änderung an einer Commitment-Konstruktion
kostet vor dem Genesis-Block fünf Prüfvektoren und danach eine
Kettenmigration.

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
| A11 | **Kontrollsegmente erkennen** | ⚠️ | **Fund 58** geschlossen (Vorrat als Parameter mit Schranke), **Fund 72** als Nullergebnis mit Beweis; Prompt-Profil bleibt offen, das Messgerät dafür steht seit 2026-08-27 |
| A12 | Kollusion beider Pods | ⚠️ | Schranke gemessen (β^2k trifft), Gegenmaßnahme unbelegt |
| A13 | Angriff auf die Krypto-Primitiven | ❓ | nie extern geprüft (K5) |
| A14 | **Ein Gateway liest den Inhalt mit** | ✅ | Sitzungs-E2E seit `myl-net` v0.9.0, über echte Verbindungen gemessen; **Verkehrsdaten bleiben sichtbar**, und das Schema selbst fällt unter A13 |
| A15 | **Zwei Blattfolgen, eine Merkle-Wurzel** | ✅ | **Fund 77** (2026-08-28), gemessen und am selben Tag behoben: die Wurzel bindet die Blattzahl (`myl-types` v0.6.0) |

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

### A14 Ein Gateway liest den Inhalt mit

**Die Klasse stand bis zum 2026-08-27 nicht in diesem Dokument**, obwohl
seine Grundlage Kap. 9.2 ausdrücklich einschließt und Kap. 9.2 die
kompromittierten Gateways ausdrücklich nennt. Nachgetragen mit dem
Schritt, der sie schließt: Ein Audit, das seine eigene Grundlage nur zur
Hälfte abbildet, zählt richtig und deckt zu wenig ab.

**Der Angriff:** Nutzer und erster Shard sprechen über ein Gateway. Mit
Transportverschlüsselung allein sind das zwei Noise-Verbindungen mit
Klartext dazwischen. Das Gateway rechnet nicht, es leitet weiter, und
genau deshalb liefe dort der Verkehr vieler Nutzer zusammen: der
Sammelpunkt, den Kap. 9.2 ausschließen will.

**Abgewehrt seit `myl-net` v0.9.0.** Sitzungsschlüssel je Epoche und
Pod, Ende zu Ende zwischen den beiden wirklichen Gesprächspartnern. Der
Klartextkopf trägt, was zum Weiterleiten nötig ist, und geht vollständig
in die Authentisierung ein: Das Gateway darf lesen, was es zum Leiten
braucht, und nichts davon ändern.

**Gemessen (`tests/sitzung.rs`):** Drei Knoten, ein wirklich
weiterleitendes Gateway, das annimmt, weitergibt, zurückgibt und
unterwegs zu öffnen versucht. Es scheitert, der Shard öffnet, und der
Klartext steht nachweislich nicht in den Bytes, die über das Gateway
gehen. Die Gegenprobe steht im selben Test: Der erlaubte Fall gelingt,
sonst hieße „niemand konnte lesen" nur „es kam nichts an".

⚑ **Die Abwehr hing zwischenzeitlich an der falschen Identität.** Der
Punkt, gegen den verschlüsselt wird, wird angekündigt und beglaubigt;
die erste Fassung beglaubigte ihn mit der Netzidentität. Der Pod-Pfad
nennt aber `MinerId`s, und die Zuordnung dorthin gibt es nicht. Seitdem
unterschreibt der Konsensschlüssel, und weil die `MinerId` der Hash
eben dieses Schlüssels ist, braucht die Prüfung nichts weiter. **Ohne
diese Berichtigung wäre die ganze Abwehr dieser Klasse nur scheinbar
gewesen**, und zwar mit grünen Tests daneben.

⚑ **Was dabei sichtbar bleibt, und es ist nicht nichts:** Der
Klartextkopf nennt Epoche, Pod, Absender, Empfänger und Zähler, und die
Länge einer Nachricht steht ohnehin auf dem Draht. Ein Gateway lernt
daraus die Pod-Zusammensetzung, wer mit wem spricht und wie viel. Es
lernt **nicht**, was gesprochen wird.

Das Whitepaper verspricht an dieser Stelle auch nichts anderes: Kap. 9.2
spricht von Prompt-Inhalt und Aktivierungswerten. Verkehrsdatenschutz
wäre eine eigene Aufgabe mit eigenen Kosten (Füllverkehr, feste
Nachrichtengrößen, Umwege), und er stünde in Spannung zu einer
Pod-Bildung, die auf gemessener Latenz beruht. **Hier steht er als
benannte Grenze, nicht als offene Lücke:** Niemand hat ihn zugesagt.

⚑ **Das Schema selbst ist ungeprüft, und das gehört hierher und nicht
in eine Fußnote.** A13 sagt „nie extern geprüft" über die
Krypto-Primitiven; seit dem 2026-08-27 gibt es zusätzlich ein eigenes
Schema aus X25519, HKDF und ChaCha20-Poly1305, und niemand von außen hat
es angesehen. Die Bausteine sind Standard, die **Zusammensetzung** ist
es nicht, und Fehler in Verschlüsselungsschemata sitzen erfahrungsgemäß
in der Zusammensetzung. Die neunzehn Gegenproben zeigen, dass die
gebauten Prüfungen greifen; sie zeigen nicht, dass die richtigen
Prüfungen gebaut wurden.

**Und was ausdrücklich nicht abgewehrt ist:** die beteiligten
Shard-Miner selbst. Ihre Aufgabe ist die Verarbeitung des Inhalts. Kap.
9.3 zieht daraus die Risikoklasse C („ungeeignet"), und daran ändert
diese Fassung nichts.

### A15 Zwei Blattfolgen, eine Merkle-Wurzel (Fund 77)

**War der Fall bis zum 2026-08-28.** Der Merkle-Baum füllte eine Ebene
mit ungerader Knotenzahl auf, indem er den letzten Knoten mit sich
selbst paarte, im Bitcoin-Stil, und erbte damit den Fehler des Vorbilds
(CVE-2012-2459). Die Abbildung von Blattfolgen auf Wurzeln war **nicht
injektiv**, und die Kollisionsfamilie war größer als der bekannte
Einzelfall:

| Blattzahl | kollidierte mit |
|---|---|
| 3, 5, 7, 9, … (ungerade ab 3) | derselben Folge plus wiederholtem letzten Blatt |
| 6, 14, 22, … (`n ≡ 2 mod 4`, ab 6) | derselben Folge plus den letzten **zwei** Blättern |
| 1, 2, 4, 8, … | nichts davon |

**Wo es scharf gewesen wäre.** Das PoI-Bündel führt kein Feld für die
Segmentzahl; die Signierbotschaft ist
`epoch ‖ pod ‖ segments_root ‖ vtfe_claimed`. Eine Aggregatsignatur über
`n` Segmente galt damit zugleich für `n+1` Segmente mit wiederholtem
letzten. Ein Angreifer gewann daraus **nichts**, weil `vtfe_claimed`
eigenständig signiert ist und nichts eine Mitgliedschaft gegen
`segments_root` prüfte. Es wäre in dem Augenblick scharf geworden, in
dem ein Streitverfahren oder eine Kontrollsegment-Prüfung einen
Merkle-Beweis dagegen laufen lässt.

**Behoben am Tag des Fundes** (`myl-types` v0.6.0): Die Wurzel ist
seither `SHA-256(0x02 ‖ u64_le(n) ‖ innere Wurzel)`. Der innere Aufbau
samt Duplikationsregel ist unverändert; gebunden wird nur die Blattzahl.

**Der Beweis in einem Satz:** Aus gleicher Wurzel folgt gleiches Urbild,
also gleiche Blattzahl und gleiche innere Wurzel; bei fester Blattzahl
liegt die Baumform fest, und über die Domain-Separation bestimmt jeder
Knoten seine beiden Kinder eindeutig.

**Belegt durch:** einen Test, der die Nachbarschaft jeder Blattzahl von
1 bis 12 abfährt (Verlängerung um ein und um zwei Blätter, keine trägt
dieselbe Wurzel); eine Gegenprobe, die festhält, dass die **inneren**
Wurzeln weiterhin zusammenfallen und allein das Präfix sie trennt; einen
Test über sechs erfundene Blattzahlen im Beweis, die alle scheitern; und
einen fünften Konformitätsvektor `four_leaves_last_repeated`, den eine
Umsetzung im Bitcoin-Stil als einzigen nicht trifft.

⚑ **Die Gegenprobe ist hier wichtiger als der Haupttest.** Ein grüner
Injektivitätstest allein bewiese nicht, dass die Bindung die Ursache
ist; er wäre auch grün, wenn sich nebenbei die Auffüllregel geändert
hätte. Deshalb steht die alte Kollision ausdrücklich im Testcode.

**Gemessene Folgen:** fünf neu erzeugte Konformitätsvektoren und ein
geänderter Gesamtwert des Protokoll-Durchlaufs, von `8c74519a11dceae5`
auf `d02dcacb6aa37026`. Die Ursache ist belegt statt angenommen: Mit der
alten Konstruktion liefert derselbe Lauf weiterhin den alten Wert, und
geändert hat sich allein die Krypto-Stufe. **Kein Crate musste angepasst
werden**, weil alle Wurzeln zur Laufzeit neu entstehen.

⚑ **Was daran über den Fund hinaus lehrreich ist.** Die
Duplikationsregel **war getestet**, und die Domain-Separation ist sauber
und mit dem richtigen Argument begründet. Geprüft wurde, dass die Regel
tut, was sie soll. Nicht geprüft wurde, was daraus **folgt**. Das ist
Fehlerklasse 3 dieses Projekts in einer Sicherheitsprimitive: ein Test,
dessen Form den Defekt verdeckt, weil er die richtige Frage nicht
stellt. Und der Baum in `da.rs` band die Blattzahl über `k` und `m` von
Anfang an mit, also hatte eine Stelle das Richtige getan, ohne dass
jemand die Regel daraus verallgemeinert hätte.

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

⚑ **Fund 72 (2026-08-27): Die zweite vermutete Spur gibt es nicht.**

Der Einschleusungsplan zieht **ohne Zurücklegen**: Ein Strom von `n`
Aufträgen enthält genau `⌈n·γ⌉` Kontrollen statt einer schwankenden
Zahl. γ ist ein Governance-Parameter und damit öffentlich, `n` sieht ein
Miner am Strom, also kennt er das Kontingent. Hätte er so viele
Kontrollen erkannt, wie hineinpassen, wäre jeder weitere Auftrag mit
Sicherheit echt, und auf einem sicher echten Auftrag kostet Betrug
nichts. Das sah nach einem Verstärker für Fund 58 aus.

**Gemessen: null.** Über jeden Vorrat von 1 bis 4 096, bei γ = 2 % und
100 000 Aufträgen, wurde das Kontingent nie erschöpft.

**Und die Null hat einen Grund, kein Glück.** Die Wiederholung erkennt
ein Segment frühestens beim **zweiten** Auftreten; von `v`
Vorratssegmenten bleiben die ersten `min(v, k)` Auftreten unerkannt.
Damit gilt `verdächtig ≤ k − min(v, k) < k` für jeden Vorrat `v ≥ 1`.
Die gemessenen Zahlen sind exakt `2 000 − Vorrat`, also der Satz mit
Gleichheit. Erschöpfen ließe sich das Kontingent nur mit einem
Unterscheider, der **jede** Kontrolle erkennt, und wer den hat, braucht
das Kontingent nicht.

**Folgerung: kein Umbau.** Unabhängiges Ziehen je Position würde ein
Loch schließen, das es nicht gibt, und dafür die exakte Einhaltung von γ
je Strom aufgeben, an der die Kostenbuchhaltung hängt.

**Was seit dem 2026-08-27 zusätzlich existiert: das Messgerät** für die
eigentliche, inhaltliche Ununterscheidbarkeit
(`myl-verifier/src/unterscheidbarkeit.rs`, Punkt 3.6). Ein
Vertauschungstest auf ganzzahligen Merkmalen, der kein Nein ohne seine
Trennschärfe herausgibt. **Das ist kein Beleg, sondern die
Voraussetzung dafür, später einen zu bekommen**: Länge, Timing und
Kontextprofil brauchen echten Verkehr, und A11 bleibt deshalb auf ⚠.

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

0. ✅ ~~**A15 entscheiden, bevor der Spur-Merkle gebaut wird**~~
   (Fund 77) — entschieden und behoben am 2026-08-28, am Tag des Fundes.
   Der Punkt stand hier vor allen anderen, nicht weil er schadete,
   sondern weil er **billig zu beheben war, solange keine Wurzel zählt,
   und teuer geworden wäre, sobald eine zählt**. ⚑ **Das Muster gilt
   über diesen Fall hinaus und gehört auf diese Liste:** Fehler in
   Commitment-Konstruktionen sind die einzige Klasse, deren
   Behebungskosten mit der Zeit steigen, ohne dass jemand etwas falsch
   macht. Wer eine findet, behebt sie sofort oder begründet, warum
   nicht.

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
Testdurchlauf derselben Art. Die neun abgewehrten Klassen sind belegt;
mehr Tests derselben Machart erhöhen die Zahl, nicht die Sicherheit.
