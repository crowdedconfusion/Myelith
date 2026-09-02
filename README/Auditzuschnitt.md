# Zuschnitt eines externen Sicherheitsreviews

**Stand:** 2026-09-02

> **Wozu dieses Dokument.** Ein Review ist so gut wie die Frage, die man
> ihm stellt. Dieses Dokument stellt sie: was das System behauptet,
> worauf die Behauptung ruht, wo ein Prüfer anfangen sollte, was
> ausdrücklich nicht dazugehört, und was bereits bekannt ist und
> deshalb nicht noch einmal gefunden werden muss.
>
> Es ist **kein Ersatz** für ein Review und behauptet nichts über die
> Qualität des Geprüften. Es ist die Vorarbeit, die ein Prüfer sonst
> selbst leisten müsste, auf unsere Kosten.

Wie eine Schwachstelle gemeldet wird, steht in
[`SECURITY.md`](../SECURITY.md). Welche Angriffsklassen heute wie
abgedeckt sind, steht in
[`SIMULATION/Sicherheitsaudit.md`](../SIMULATION/Sicherheitsaudit.md).

---

## 1. Der Zustand, auf den sich ein Review bezieht

⚑ **Ein Review ohne benannten Stand ist wertlos**, denn der Bericht
überlebt den Code. Vor Beginn wird deshalb ein **Commit-Hash**
festgehalten, und der Bericht nennt ihn.

Zum Zeitpunkt dieses Dokuments: neunzehn Crates, 2 324 Tests, alle grün;
Mindestfassung Rust 1.85; keine bekannte Schwachstelle im
Abhängigkeitsbaum (284 Pakete unter dem Knoten, geprüft mit
`cargo deny`).

**Es gibt kein Mainnet und keinen Genesis-Block.** Was heute läuft, ist
ein Probelauf; der Startwert der Probekette sagt das im Klartext. Ein
Befund kostet heute niemanden Geld, und **genau deshalb ist er jetzt am
meisten wert**: Eine Änderung an einer Commitment-Konstruktion kostet
vor dem Genesis-Block ein paar Prüfvektoren und danach eine
Kettenmigration.

---

## 2. Was das System behauptet

Drei Zusagen, und alles Weitere hängt an ihnen.

**Z1. Zwei ehrliche Knoten rechnen dasselbe, Bit für Bit.** Die
Inferenz läuft vollständig ganzzahlig, damit ist die Reihenfolge der
Reduktionen gleichgültig und das Ergebnis reproduzierbar.

> ⚑ **Der Stand dieser Zusage:** Sie ist aus dem Zahlenformat
> **begründet** und auf **einer** Rechnerarchitektur gemessen. Über zwei
> Architekturen hinweg ist sie nicht gemessen. Das ist der wichtigste
> offene Beleg des Projekts, und ein Review ändert daran nichts.

**Z2. Falsche Arbeit wird erkannt und kostet mehr, als sie einbringt.**
Drei Stufen: zwei redundante Pods vergleichen (Stufe 1), eine Stichprobe
rechnet nach (Stufe 2), im Streitfall grenzt ein Bisektionsspiel die
strittige Schicht ein und eine Schiedsrunde entscheidet. Wer verliert,
wird geschlachtet.

**Z3. Ein Agent kann nicht mehr tun, als sein Sitzungskontrakt erlaubt,
auch wenn ihm jemand Anweisungen unterschiebt.** Die Grenzen liegen im
Konsenszustand, nicht im Client, und ein Plan kennt keine Verzweigung
zur Laufzeit.

---

## 3. Das Vertrauensmodell, in einem Absatz

**Angenommen wird:** weniger als ein Drittel des Stimmgewichts ist
byzantinisch; die kryptografischen Primitiven halten (BLS12-381,
SHA-256, ChaCha20-Poly1305, Ed25519); ein neuer Knoten erreicht
mindestens einen ehrlichen Bootstrap-Knoten; die Uhren laufen nicht
beliebig weit auseinander.

**Nicht angenommen wird:** dass ein Miner ehrlich rechnet, dass ein
Koordinator ein Bündel korrekt bildet, dass ein Gateway den Inhalt nicht
liest, dass ein Teilnehmer über seinen Standort oder seine Hardware die
Wahrheit sagt.

⚑ **Die interessanten Fragen liegen zwischen diesen beiden Listen.**
Die schwersten Funde dieses Projekts saßen nicht in einer Komponente,
sondern zwischen zweien: eine leere Spur, die als Nachweis durchging;
eine Bisektion, die systematisch die falsche Schicht nannte und damit
das Verfahren umkehrte; ein Schuldspruch, den nichts an die strittige
Arbeit band. Alle drei waren grün getestet.

---

## 4. Wo anzufangen ist, nach Schadenshebel

Die Reihenfolge ist eine Empfehlung und keine Einschränkung.

### 4.1 Die Verwendung der Signaturen

`SHARED_TYPES/myl-types/src/{bls.rs, uebergang.rs, challenge.rs,
sitzung.rs, quittung.rs, zusage.rs}`,
`CONSENSUS/myl-consensus/src/signing.rs`.

**Warum zuerst:** Die Sicherheit des Protokolls hängt am Slashing, das
Slashing an einem Beleg, der Beleg an einer Signatur. Dass die
Primitiven gegen ihre Testvektoren stimmen, ist geprüft; dass ihre
**Verwendung** trägt, ist Eigenbeurteilung.

**Was vorbereitet ist:**
[`SHARED_TYPES/README/Signatur-Bedrohungsmodell.md`](../SHARED_TYPES/README/Signatur-Bedrohungsmodell.md)
führt jede Verwendung in einer Tabelle: was signiert wird, wogegen es
schützt, woran der Schutz hängt.

**Konkrete Fragen:** Ist die Domänentrennung vollständig? Achtzehn
`DST_`-Präfixe sind vergeben; deckt jede signierte Klasse eines ab, und
ist keines doppelt belegt? Bindet jede Signaturbotschaft ihren Kontext
(Runde, Epoche, Kette), oder gibt es eine, die in einem anderen
Zusammenhang wiederverwendbar ist? Hält der Besitznachweis gegen
gewählte fremde Schlüssel in **jeder** Aggregation, nicht nur in der
geprüften?

⚑ **Ein bekannter Sonderfall, und er ist der lehrreichste:** Die
Aggregatsignatur ist deterministisch für eine **feste**
Unterzeichnermenge. Wer die Menge wählen darf, wählt das Ergebnis. An
jeder Stelle, an der ein Aggregat als Zufallsquelle dient, ist die Frage
also nicht „ist BLS deterministisch", sondern „wer bestimmt die Menge".

### 4.2 Quorum, Sperre und Rundenwechsel

`CONSENSUS/myl-consensus/src/{bft.rs, round_change.rs, voting_weight.rs}`.

**Warum:** Ein Rundenwechsel ohne Sperrmechanik bricht die Safety, und
zwar so, dass das Protokoll schneller aussieht und falsch ist.

**Konkrete Fragen:** Schneiden sich zwei Quoren wirklich in mehr als
einem Drittel des Gewichts, auch bei ungleichen Gewichten und
Rundungen? Kann ein Zertifikat aus einer Epoche in einer anderen gelten?
Was passiert an der Epochengrenze, wenn sich die stimmberechtigte Menge
ändert?

⚑ **Dies ist der einzige Sicherheitssatz des Projekts, für den es kein
Bindeglied zu einem Eigenschaftstest gibt**, weil die Aussage über
Mengen von Validatoren und Nachrichtenfolgen gilt und nicht über eine
Funktion. Wer hier prüft, prüft die am wenigsten maschinell abgesicherte
Stelle.

### 4.3 Die Zustandsübergänge

`CONSENSUS/myl-ledger/src/{transitions.rs, state.rs}`.

**Konkrete Fragen:** Schafft irgendein Übergang Wert, ohne dass eine
Quelle benannt ist? Bleibt ein **abgelehnter** Übergang bitgleich ohne
Wirkung? Läuft eine Rechnung an den Rändern des Zahlbereichs um, und
zwar in **beiden** Bauprofilen? Der letzte Punkt hat hier schon dreimal
etwas geliefert: Eine Zusicherung, die nur im Freigabebau still
umläuft, ist von außen nicht zu unterscheiden.

### 4.4 Bisektion, Schuldbeleg und Schiedsrunde

`VERIFICATION/myl-verifier/src/{bisection.rs, anzeige.rs,
nachrechner.rs, kontrollsegmente.rs}`.

**Konkrete Fragen:** Nennt das Spiel den kleinsten abweichenden Index,
für **jede** Spurlänge und **jede** Position? Kann ein Angeklagter
gewinnen, indem er schweigt? Kann ein Ankläger jemanden anzeigen, ohne
selbst etwas zu riskieren? Sind Kontrollsegmente von echter Arbeit
ununterscheidbar, und woran hängt das?

⚑ **Der Vorrat der Kontrollsegmente ist ein Governance-Parameter mit
einer Untergrenze.** Ein Vorrat kleiner als die Zahl der Einschleusungen
wiederholt Kennungen, und echte Arbeit tut das nie.

### 4.5 Sitzungsverschlüsselung und Schlüsselableitung

`NETWORKING/myl-net/src/sitzung.rs` (2 428 Zeilen, viermal so lang wie
die nächstgrößte Datei des Netzcodes), dazu `identity.rs` und
`NODE/myl-node/src/schluessel.rs`.

**Konkrete Fragen:** Ist die Schlüsselableitung an alles gebunden, was
sie binden muss? Trägt jede versiegelte Nachricht ihren Kopf als
authentifizierte Zusatzdaten? Was geschieht mit Nachrichten, die im
Augenblick der Epochenrotation unterwegs sind? (Das ist ein bekannter
offener Punkt, siehe Abschnitt 6.)

### 4.6 Die Drahtformate

`SHARED_TYPES/myl-types/tests/fuzzziele/`,
`CONSENSUS/myl-consensus/tests/fuzzziele/`, dazu
`NODE/myl-node/src/validator.rs`.

**Was schon läuft:** Siebzehn Fuzz-Ziele prüfen **Kanonizität**, also
dass eine Bytefolge, die sich vollständig als Protokolltyp liest, sich
wieder genau so schreiben lässt. Wo zwei Bytefolgen denselben Wert
ergeben, ist eine formbar, und im Gossip hieße das zwei Nachrichten für
einen Inhalt.

**Was offen ist:** Die Ziele prüfen **keine Signatur und keine
Semantik**, weil eine BLS-Prüfung je Eingabe die Rate um drei
Größenordnungen senkt. Ein Ziel mit vorbereiteten Schlüsseln ist noch
nicht geschrieben.

### 4.7 Die Governance-Invarianten

`GOVERNANCE/myl-governance/src/{invarianten.rs, registry.rs}`.

**Konkrete Frage:** Die Registry prüft vor der Abstimmung, ob ein
Vorschlag zulässig **wäre**. Gibt es eine Kombination zulässiger
Einzelwerte, die zusammen eine Invariante bricht?

---

## 5. Was ausdrücklich nicht im Zuschnitt ist

- **Die GPU-Rückenden.** `INTEGER_LLM/kernels/src/backends/{cuda,rocm}.rs`
  reichen an die Referenzkerne weiter, statt selbst zu rechnen. Ein
  Konformitätslauf mit `cuda` wird aus diesem Grund abgelehnt.
- **Was das Whitepaper beschreibt und der Code nicht hat.** Die
  Komponententabelle des README trennt Gebautes von Entworfenem.
- **Die Wahl der wirtschaftlichen Parameter.** Ob eine Prägekurve
  klug ist, ist eine andere Frage als ob sie überläuft. Das Zweite
  gehört dazu, das Erste nicht.
- **Die Frage, ob ein Agent richtig entscheidet.** Das System sichert
  zu, dass er seine Grenzen einhält, und ausdrücklich nicht, dass er
  gute Entscheidungen trifft.
- **Ein Beweis, dass eingeschleuste Anweisungen unmöglich sind.**
  Beansprucht wird die Einhaltung der Grenzen **trotz** Täuschung, nicht
  die Täuschungsfreiheit.

---

## 6. Was bereits bekannt ist

Damit niemand seine Zeit damit verbringt, es wiederzufinden. Der
vollständige Stand nach Angriffsklassen steht in
[`SIMULATION/Sicherheitsaudit.md`](../SIMULATION/Sicherheitsaudit.md).

| Bekannt | Kurz |
|---|---|
| **Bitgleichheit über Architekturen** | begründet, auf einer Architektur gemessen |
| **Kryptografie nie extern geprüft** | genau der Anlass dieses Dokuments |
| **Ein Drittel des Stimmgewichts** | Voraussetzung des Verfahrens, keine Lücke |
| **Eclipse** | abgewehrt, mit der Restbedingung eines ehrlichen Bootstrap-Knotens |
| **Kollusion beider Pods** | Schranke gemessen, Gegenmaßnahme unbelegt |
| **Prompt-Profil der Kontrollsegmente** | Messgerät steht, echter Verkehr fehlt |
| **Nachrichten an der Epochengrenze** | die Schlüsselrotation kennt keine Schonfrist, unterwegs befindliche Nachrichten gehen verloren |
| **Wer die Prüfsegmente einspeist** | eine offene Entscheidung, nicht nur eine offene Zeile |
| **Standort und Latenz** | beruhen auf Selbstauskunft, eine erklärte Angabe trägt die Ausfalldiversität und nie die Sicherheit |
| **Miner zu Schlüssel** | eine Registrierung fehlt, deshalb greift die Prüfung einer Anfechtung im echten Netz nur bei Validatoren |
| **Betriebsfragen** | kein produktiver Genesis-Zustand, Persistenz standardmäßig aus, kein SIGTERM, keine Betriebsbeobachtung |

---

## 7. Was an Werkzeug bereitliegt

- **2 324 Tests** über neunzehn Crates, darunter fünf adversariale
  Ebenen (`tests/adversarial.rs` in NETWORKING, CONSENSUS,
  COMPUTE_PIPELINE, VERIFICATION, TOKENOMICS), Chaos-Tests über
  Partition, Neustart und Paketverlust, und ein
  Fuzz-Harness je Drahtformat.
- **Ein Konformitätspaket:** 33 Golden Vectors, deren Gesamtwert auf
  jeder Maschine gleich sein muss. Ein Knoten, der beim Start anders
  rechnet, kommt nicht ins Netz. ⚑ **Das Tor beim Start fährt die
  Operations-Vektoren**, nicht die Layer- und Ende-zu-Ende-Vektoren:
  Letztere verlangen Modellartefakte in Gigabyte-Größe, und ein Start,
  der davon abhinge, wäre für die meisten Betreiber kein Start. Es
  belegt damit, dass die Kernel übereinstimmen, und nicht, dass die
  ganze Kette es tut.
- **Deterministische Wiederholung:** Der Wiederanlauf eines Knotens
  rechnet jede Zustandswurzel neu, statt sie zu lesen. Zwei Läufe über
  dieselbe Kettendatei müssen bitgleich enden.
- **Eigenschaftstests** über zufällige, reproduzierbare Folgen, ohne
  externe Abhängigkeit; ein Zufallsgenerator in zehn Zeilen genügt,
  solange die Folge wiederholbar ist.

⚑ **Ein Hinweis zur Testlage, und er gehört hierher statt in eine
Fußnote:** Rund sechzig Testnamen dieses Repositoriums tragen eine
Allaussage im Namen („immer", „nie", „jede", „exakt"), und vier davon
sind als Eigenschaftstest über erzeugte Eingaben geschrieben. Die
übrigen prüfen zwei bis fünf getippte Beispiele. **Wo ein Name mehr
verspricht als die Prüfung hält, ist das ein guter Ort, um zu
beginnen.**

---

## 8. Was ein Befund uns wert ist

Meldeweg, Umfang und Fristen stehen in [`SECURITY.md`](../SECURITY.md).
Es gibt kein Kopfgeldprogramm.

⚑ **Was wir ausdrücklich auch hören wollen:** eine Zusage, die zu stark
formuliert ist. Mehrere der schwersten Funde dieses Projekts waren keine
Fehler im Code, sondern Behauptungen, die der Code nicht deckte, und
gefunden wurden sie beim Nebeneinanderlegen von Aussage und
Implementierung. Ein Prüfer, der eine Formulierung zu weit findet,
meldet damit dieselbe Klasse.
