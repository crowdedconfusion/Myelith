# Bedrohungsmodell je Signaturverwendung

**Stand:** 2026-08-28
**Anlass:** Kritikpunkt K5 aus dem Review vom 2026-08-18

> „Dass `myl-types` gegen die RFC-9381-Testvektoren stimmt, ist geprüft.
> Dass die *Verwendung* der Primitiven trägt (Domain Separation,
> Aggregationsreihenfolge, Rogue-Key-Schutz, die Bindung der
> Signaturnachricht an den Rundenkontext) ist Eigenbau-Beurteilung. Bei
> einem Protokoll, dessen Sicherheit an Slashing hängt, ist das die Stelle
> mit dem größten Schadenshebel."

Dieses Dokument **ersetzt kein externes Kryptografie-Review** und soll es
auch nicht. Es soll das Review vorbereiten: Ein Prüfer, der hier anfängt,
sieht in einer Tabelle, was signiert wird, wogegen es schützt und woran
der Schutz hängt, statt sich das aus sieben Crates zusammenzusuchen.

Wo eine Zusage nur durch Zufall gilt, steht das hier ausdrücklich. Genau
das ist der Zweck.

---

## 1. Die Primitiven

| | |
|---|---|
| Kurve | BLS12-381, min-pk (Schlüssel in G1, Signaturen in G2) |
| Bibliothek | `blst` |
| Hash-to-Curve | `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_` (`BLS_DST`) |
| Besitznachweis | `BLS_POP_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_` (`BLS_POP_DST`) |
| Aggregation | `fast_aggregate_verify` (eine Botschaft, viele Schlüssel) |

**Zwei Ebenen der Domain-Separation, nicht eine.** Die untere ist die
Hash-to-Curve-DST der Bibliothek; sie trennt gewöhnliche Signaturen von
Besitznachweisen. Die obere ist das **Präfix in der Botschaft selbst**,
und sie trennt die Nachrichtenklassen des Protokolls voneinander. Die
untere allein reicht nicht: Ohne Präfix wäre eine Vote für Block B in
Runde r zugleich ein gültiger Commit für denselben Block, und beide
lägen unter derselben Hash-to-Curve-DST.

---

## 2. Die Verwendungen im Überblick

| # | Verwendung | Botschaft | Länge | DST im Klartext | Schützt gegen |
|---|---|---|---|---|---|
| 1 | BFT Propose | `DST ‖ round ‖ block` | 62 B | `MYELITH_BFT_PROPOSE_v1` | Umdeutung als Vote/Commit |
| 2 | BFT Vote | `DST ‖ round ‖ block` | 59 B | `MYELITH_BFT_VOTE_v1` | dito, plus Quorum-Fälschung |
| 3 | BFT Commit | `DST ‖ round ‖ block` | 61 B | `MYELITH_BFT_COMMIT_v1` | dito |
| 4 | Propose mit Polka | `DST ‖ round ‖ block ‖ valid_round` | 74 B | `MYELITH_BFT_PROPOSE_POL_v1` | Hochsetzen der `valid_round` |
| 5 | PoI-Bündel | `DST ‖ epoch ‖ pod ‖ segments_root ‖ vtfe` | 101 B | `MYELITH_POI_BUNDLE_v1` | nachträgliche Erhöhung des Anspruchs |
| 6 | Besitznachweis | komprimierter Schlüssel | 48 B | eigene Hash-to-Curve-DST | Rogue-Key (Fund 27) |
| 7 | **Shard-Übergang** | Borsh über `TransitionSig` | **112 B** | **keine** | siehe 4.1 |

---

## 3. Was jede Verwendung leistet

### 3.1 BFT-Nachrichten (1–4)

**Was signiert wird:** `(Runde, Block-Hash)`, je Klasse mit eigenem
Präfix. Der **Absender ist nicht Teil der Botschaft** — er ergibt sich
aus dem Schlüssel, gegen den verifiziert wird.

**Warum der Absender fehlen muss:** Genau das macht den
Double-Signing-Beweis möglich. Zwei gültige Signaturen desselben
Schlüssels über dieselbe Runde bei verschiedenen Block-Hashes sind der
Beweis, und sie wären es nicht, wenn der Absender in der Botschaft
stünde und mitvariieren könnte.

**Rundenbindung.** Ohne sie ließe sich ein altes Polka in einer neuen
Runde wiedereinsetzen, gesperrte Validatoren entsperren und damit zwei
Blöcke auf derselben Höhe erzeugen, also BFT-Safety brechen. Geprüft in
`myl-consensus/tests/adversarial.rs::ein_zertifikat_aus_einer_anderen_runde_wird_abgelehnt`.

**`valid_round` (4).** Ein Leader, der einen Block aus einer früheren
Runde erneut vorschlägt, muss die Runde mitsignieren, aus der sein Polka
stammt. Ohne diese Bindung könnte ein Angreifer die Zahl in einer
abgefangenen Nachricht hochsetzen; die Signatur bliebe gültig, weil sie
die Zahl gar nicht abdeckt.

**Eigenes Präfix statt Erweiterung von `DST_PROPOSE`:** additiv, also
ohne Invalidierung zuvor erzeugter Signaturen.

**Restrisiko.** Die Aggregation über `fast_aggregate_verify` verlangt
Rogue-Key-Schutz; siehe 3.3.

### 3.2 PoI-Bündel (5)

**Was signiert wird:** Epoche, Pod, Merkle-Wurzel der Segmente und
**`vtfe_claimed`**. Der letzte Punkt ist der wichtige: Ohne ihn könnte
der Koordinator die beanspruchte Arbeitsmenge nach dem Einsammeln der
Signaturen erhöhen, ohne das Aggregat ungültig zu machen. Die
Mitglieder hätten dann einem Anspruch zugestimmt, den sie nie gesehen
haben.

**Die maßgebliche Unterzeichnermenge** ist `PodMembership` aus der
Scheduler-Zuteilung, nicht eine vom Koordinator mitgelieferte Liste.
Sonst bestimmte der Angreifer, gegen welche Schlüssel geprüft wird.

**Doppel-Sperre je `(Epoche, Pod)`:** Ein Bündel gilt einmal.

⚑ **Bis zum 2026-08-28 mit einer Einschränkung, siehe 4.4:** Die
Merkle-Wurzel bestimmte die Segmentfolge **nicht eindeutig**, es passte
also mehr als eine Folge auf dieselbe Signatur. Seit `myl-types` v0.6.0
bindet die Wurzel die Segmentzahl mit, und die Aussage oben gilt ohne
Vorbehalt.

### 3.3 Besitznachweis (6) — der Fund 27

**Was er verhindert:** Rogue-Key-Angriffe auf `fast_aggregate_verify`.

Der Modulkopf von `bls.rs` sagte einmal zu, Identitäts- und
Subgruppenprüfung schützten dagegen. **Das war falsch, und es war
nachgewiesen falsch, nicht vermutet:** Die Konstruktion
`pk_rogue = g₁^x · pk_opfer⁻¹` besteht `blst_p1_uncompress` *und*
`key_validate()`, und danach gilt
`fast_aggregate_verify([pk_opfer, pk_rogue], msg, σ)` für ein σ, das der
Angreifer allein mit seinem Geheimnis x erzeugt hat. Das Opfer hat nie
unterschrieben.

**Reichweite waren beide Aufrufstellen:** `poi.rs` (ein einzelnes
Pod-Mitglied fälscht die Bestätigung des ganzen Pods und beansprucht
Arbeit, die niemand geleistet hat) und `round_change.rs` (ein Validator
erzeugt allein ein Polka, entsperrt gesperrte Validatoren, zwei Blöcke
auf derselben Höhe).

**Behoben** nach draft-irtf-cfrg-bls-signature §3.3; der Angreifer kennt
den diskreten Logarithmus von `pk_rogue` nicht (er wäre `x − sk_opfer`)
und kann keinen Nachweis liefern. Regression:
`myl-types/tests/rogue_key.rs`, das **beide** Tatsachen festhält — dass
der Rogue Key die Validierung besteht und dass der Besitznachweis ihn
ausschließt.

**Was hier schiefging und der Grund für dieses Dokument:** Der Schutz
stand in der Dokumentation, bevor er im Code stand, und niemand hat den
Satz gegen die Literatur geprüft. Eine schriftliche Zusage ohne Beleg
ist keine Sicherheitseigenschaft.

**Offen:** Die Pod-Prüfung sitzt in `PodMembership::new`, weil
`myl-scheduler::MinerRegistration` heute keinen Schlüssel trägt. Sobald
es eine Miner-Registrierung mit Schlüsseln gibt, gehört sie dorthin:
einmal beim Eintritt statt bei jeder Pod-Bildung.

---

## 4. Was **nicht** trägt

### 4.1 ⚑ Die Shard-Übergangssignatur hat keine Domain-Separation

`TransitionSig::to_sign_bytes()` ist eine reine Borsh-Serialisierung von
`(segment_id, shard_index, position, prev_hash, next_hash)`. **Kein
Präfix.** Sie ist damit die einzige Signaturverwendung im Projekt ohne
das Merkmal, das jede andere trägt.

**Heute ist keine Verwechslung möglich, aber nicht durch Design.** Die
Botschaftslängen aller Klassen sind paarweise verschieden:

| Klasse | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|
| Bytes | 62 | 59 | 61 | 74 | 101 | 48 | 112 |

Eine Botschaft der Klasse 7 kann keine der Klassen 1–6 sein, weil sie
112 Bytes lang ist und keine andere das ist. **Der Schutz ist ein
Längenzufall.** Er hält, solange niemand

- eine Nachrichtenklasse mit 112 Bytes hinzufügt,
- `TransitionSig` um ein Feld erweitert oder eines entfernt,
- eine Klasse um ein Feld erweitert, das sie auf 112 Bytes bringt,
- oder eine variabel lange Botschaft einführt.

Jede dieser vier Änderungen ist harmlos aussehend und würde den Schutz
**still** beseitigen: Es gibt keinen Test, der fehlschlägt, und kein
Kompilat, das bricht.

**Wann es zählt.** Ein Miner benutzt seinen BLS-Schlüssel sowohl für
Shard-Übergänge als auch als Pod-Mitglied für PoI-Bündel. Kollidierten
zwei Klassen, ließe sich eine in der einen Rolle abgegebene Signatur in
der anderen einsetzen. Die Wirkung wäre je nach Richtung ein erschlichener
Arbeitsanspruch oder ein gefälschter Rechenschritt in einem Streitfall.

**Empfehlung, nicht umgesetzt:** ein `DST_SHARD_TRANSITION_v1` vor die
Borsh-Bytes. Additiv wie `DST_PROPOSE_POL`, kostet eine Zeile.
**Bewusst nicht in diesem Zug gemacht**, weil es das Drahtformat des Pods
ändert und damit eine Protokolländerung ist, die zusammen mit den anderen
offenen Punkten von COMPUTE_PIPELINE entschieden gehört. Bis dahin steht
sie hier.

### 4.2 Latenz-Atteste tragen eine Signatur, die niemand prüft

`myl_types::LatencyAttest` hat ein Feld `signature`. **Es gibt im
gesamten Projekt keine Stelle, die es verifiziert**, und keine, die ein
Attest erzeugt. `myl-net::validation` prüft ausdrücklich nur die
strukturelle Plausibilität und verweist für die Signatur auf einen
`PayloadValidator`, den bisher niemand verdrahtet hat.

Solange die Latenzwerte in die Pod-Bildung eingehen (Geo-Clustering,
Kap. 4.1/4.3), ist das die Stelle, an der ein Angreifer sich seine
Pod-Nachbarn aussuchen könnte, und das ist die Vorstufe zur Kollusion.
Das Feld allein schützt nicht; es schützt die Prüfung, und die fehlt.

### 4.3 Die Aggregationsreihenfolge

`fast_aggregate_verify` ist von der Reihenfolge der Schlüssel unabhängig
(Punktaddition in G1 ist kommutativ). Die **Kodierung** der
Unterzeichnerliste ist es nicht: `PolkaCertificate` verlangt streng
aufsteigende Unterzeichner, damit ein Stimmensatz genau eine Kodierung
hat und Duplikate strukturell ausgeschlossen sind. Beide Eigenschaften
sind in `myl-consensus/tests/adversarial.rs` geprüft.

**Was daran nicht geprüft ist:** dass jede andere Stelle, die Schlüssel
aggregiert, dieselbe Disziplin hält. `poi.rs` leitet die Unterzeichner
aus `PodMembership` ab und ist damit nicht auf eine mitgelieferte
Reihenfolge angewiesen; das ist gut, aber es ist eine Eigenschaft dieser
einen Stelle und keine Regel, die etwas erzwingt.

---

### 4.4 ✅ Die Merkle-Wurzel bestimmt die Segmentfolge (Fund 77, behoben)

**War der Fall bis zum 2026-08-28.** Der Merkle-Baum füllte eine Ebene
mit ungerader Knotenzahl auf, indem er den letzten Knoten mit sich
selbst paarte (Bitcoin-Stil), und erbte damit CVE-2012-2459: Bei
ungerader Blattzahl ab 3 hatten `[l₁ … lₙ]` und `[l₁ … lₙ, lₙ]`
dieselbe Wurzel.

**Für dieses Dokument war das erheblich.** Die Signierbotschaft des
PoI-Bündels ist `DST ‖ epoch ‖ pod ‖ segments_root ‖ vtfe_claimed`, und
sie führt **kein Feld für die Segmentzahl**. Eine Aggregatsignatur der
Pod-Mitglieder über ein Bündel von `n` Segmenten war damit zugleich eine
gültige Signatur über ein Bündel von `n+1` Segmenten mit wiederholtem
letzten. Die Mitglieder hätten etwas anderem zugestimmt, als sie gesehen
haben.

**Das war genau die Lücke, gegen die `vtfe_claimed` in Abschnitt 3.2
aufgenommen wurde**, eine Ebene tiefer: Dort war es die beanspruchte
Arbeitsmenge, hier die Menge der Segmente.

**Behoben mit `myl-types` v0.6.0.** Die Wurzel bindet die Blattzahl:
`SHA-256(0x02 ‖ u64_le(n) ‖ innere Wurzel)`. ⚑ **Und die Behebung
brauchte kein neues Feld in `PoIBundle`**, denn die Segmentzahl steckt
seither in `segments_root` selbst. Die Aggregatsignatur bindet sie mit,
ohne dass die Botschaft länger wird.

**Warum der Zeitpunkt zählte.** Die Änderung verschiebt jede bestehende
Wurzel. Zum Zeitpunkt der Behebung gab es keinen Genesis-Block und keine
gespeicherte Kette; betroffen waren fünf Prüfvektoren und ein
Fingerabdruck des Testclients. Nach dem Genesis-Block wäre dieselbe
Änderung eine Kettenmigration gewesen. Es war der letzte billige
Zeitpunkt, und er war Zufall.

**Was ausdrücklich nicht betroffen war:** die Domain-Separation. Sie ist
sauber (`0x00` für Blätter, `0x01` für Knoten) und mit dem richtigen
Argument begründet; der Fund lag nicht dort. Und der Baum in `da.rs`
über die Erasure-Fragmente war nie betroffen, weil `k` und `m` im selben
Commitment stehen und die Blattzahl damit binden. **Diese Stelle hatte
also von Anfang an das Richtige getan**, ohne dass jemand die Regel
daraus verallgemeinert hätte.

---

## 5. Was ein externer Prüfer zuerst ansehen sollte

Nach Schadenshebel geordnet:

0. ✅ ~~**`merkle.rs` und die Frage, was eine Wurzel eigentlich
   festlegt**~~ (4.4, Fund 77) — behoben am 2026-08-28, am Tag des
   Fundes. Der Punkt stand hier vor allen anderen, weil seine
   Behebungskosten als einziger mit jedem Betriebstag gestiegen wären.
   **Was ein Prüfer stattdessen ansehen sollte:** ob die Bindung der
   Blattzahl überall greift, wo eine Wurzel veröffentlicht oder
   signiert wird, und ob der später geplante Merkle-Aufbau über
   Berechnungsspuren sie mitnimmt.
1. **`poi.rs` und `round_change.rs`** — die beiden Aufrufstellen von
   `fast_aggregate_verify`. Beide waren von Fund 27 betroffen, an beiden
   hängt Geld beziehungsweise BFT-Safety.
2. **Der fehlende DST in `trace.rs`** (4.1) und die Frage, ob der
   Längenzufall als Schutz gelten darf.
3. **Die Schlüsselverwendung über Rollen hinweg**: Ein Miner ist Shard,
   Pod-Mitglied und möglicherweise Validator. Ob dieselbe Identität in
   allen drei Rollen dasselbe Schlüsselpaar benutzen darf, ist im Projekt
   nirgends entschieden, sondern ergibt sich aus dem Code.
4. **Die ungeprüfte Attest-Signatur** (4.2) im Zusammenhang mit der
   Pod-Bildung.
5. **`bls.rs` selbst** gegen draft-irtf-cfrg-bls-signature, mit besonderem
   Blick auf die Frage, die einmal falsch beantwortet war: Wogegen schützt
   `key_validate()` und wogegen nicht.

---

## 6. Änderungshistorie

| Datum | Änderung |
|---|---|
| 2026-08-23 | Erstfassung (K5). Dabei gefunden: 4.1 (kein DST beim Shard-Übergang), 4.2 (ungeprüfte Attest-Signatur) |
