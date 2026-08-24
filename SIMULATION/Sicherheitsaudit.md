# Sicherheitsaudit

**Stand:** 2026-08-24
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
| A9 | **Eclipse: Umzingelung eines Knotens** | ❌ | **Fund 53**, gemessen: keine Verbindungsgrenze |
| A10 | **Latenzwerte fälschen** | ❌ | Attest-Signatur wird von niemandem geprüft |
| A11 | **Kontrollsegmente erkennen** | ❌ | Ununterscheidbarkeit ist offene Messfrage |
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

### ⚑ A9 Eclipse (Fund 53) — die teuerste Lücke der Netzschicht

**Gemessen:** Zwanzig Sybil-Identitäten verbinden sich mit demselben
Opfer, **alle zwanzig werden angenommen**. `build_swarm` hat kein
`connection_limits`, kein Peer-Scoring, keine Diversitätsregel.

**Warum das teuer ist:** Wer beliebig viele Verbindungen aufbauen darf,
füllt die Peer-Menge des Opfers und entscheidet danach, **welche
Nachrichten es sieht** — nicht durch Fälschung, sondern durch Auswahl.
Die Sicherheit dieses Protokolls hängt daran, dass Checker fremde
Segmente **beobachten**; wer die Beobachtung steuert, steuert die
Verifikation.

**Was hält:** Eine Sybil kann keine fremde Nachricht fälschen (Gossipsub
`Signed` + `Strict`) — fluten ja, fälschen nein. Und eine einzige ehrliche
Verbindung genügt, damit das Opfer weiter empfängt. Damit ist die
Anforderung präzise: nicht „Sybils abwehren", sondern **mindestens eine
ehrliche Verbindung garantieren**.

**Nötig:** `libp2p::connection_limits`, Gossipsub-Peer-Scoring, eine
reservierte Zahl ausgehender Verbindungen zu Bootstrap-Knoten, eine
Schranke je Adressbereich.

### ⚑ A10 Latenzwerte fälschen

`myl_types::LatencyAttest` trägt ein `signature`-Feld, das **im ganzen
Projekt niemand verifiziert**, und niemand erzeugt ein Attest. Die
Latenzwerte gehen ins Geo-Clustering der Pods.

**Wirkung:** Wer sie frei setzen kann, sucht sich seine Pod-Nachbarn aus,
und das ist die Vorstufe zur Kollusion (A12). Ein ungeprüftes
Signaturfeld ist dabei **gefährlicher als gar keines**, weil ein Leser es
für einen Schutz hält.

**Nötig:** `PayloadValidator` verdrahten — die Stelle entsteht ohnehin,
sobald `myl-net` und `myl-consensus` in einem Prozess zusammenkommen.

### ⚑ A11 Kontrollsegmente erkennen

Kap. 6.7 verlangt Ununterscheidbarkeit als **erste**
Konstruktionsanforderung. Die Mechanik steht seit myl-verifier v0.5.0;
die Ununterscheidbarkeit ist eine **Eigenschaft der Daten, nicht des
Codes**, und das Whitepaper führt sie selbst als offene Messfrage
(Kap. 11, Punkt 5).

**Wirkung, wenn sie fehlt:** Erkennt ein Miner Kontrollsegmente an
Timing, Länge oder Kontextprofil, ist der einzige Schutz gegen den
einmaligen Eingriff wirkungslos, und mit ihm A1 für den Fall, dass der
Angreifer beide Pods hält.

**Nötig:** die statistische Analyse aus den Akzeptanzkriterien von
VERIFICATION Phase 3.

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

1. **A9 Verbindungsgrenze** (Fund 53). Wer die Beobachtung steuert,
   steuert die Verifikation. Der Aufwand ist klein:
   `libp2p::connection_limits` ist eine Behaviour-Zeile, das Peer-Scoring
   eine Konfiguration.
2. **A10 Attest-Signatur prüfen.** Sie ist der Hebel auf β_lokal und
   damit auf A12. Entsteht ohnehin mit dem Knoten-Binary.
3. **A11 statistische Analyse.** Ohne sie trägt A1 im Fall „Angreifer
   hält beide Pods" nicht.
4. **A13 externes Review.** Vor dem Mainnet, nicht danach.

**Was ausdrücklich nicht auf dieser Liste steht:** ein weiterer
Testdurchlauf derselben Art. Die acht abgewehrten Klassen sind belegt;
mehr Tests derselben Machart erhöhen die Zahl, nicht die Sicherheit.
