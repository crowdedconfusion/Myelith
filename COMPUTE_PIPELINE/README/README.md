# compute-pipeline (`myl-pod`)

> **Version:** 0.25.1
> **Datum:** 2026-09-01
> **Status:** Phase 1 vollständig, Phase 2.1, **Phase 3 vollständig**
> (3.1 bis 3.3) und Punkt 4.3. `shard_loop` mit Spur-Hashes und
> Manipulationserkennung, `coordinator_loop` mit Micro-Batching,
> KV-Cache-Session-Affinität, erasure-codierte DA-Archivierung,
> Ausfallsicherung mit Standby-Übernahme, Epochenübergang, seit dem
> 26. August die **Verdrahtung mit dem Epochen-Scheduler** und seit dem
> 1. September die **Nachbesetzung aus der Reserve des Netzes** (3.4)
> samt **Gegenzeichnung und Frist** für die Ausfallmeldung (3.5) und der
> **Staffelung nach dem Verstoß-Zähler** (3.6). **115 Tests grün.**
>
> **Offen:** die Verbreitung der Meldung über Gossip (der zweite Teil von
> 3.5) und der Pod-Lauf über getrennte Hosts.

Pod-Orchestrierung über ein echtes Netzwerk: Pipeline-Routing,
Micro-Batching, KV-Cache-Verwaltung, spekulatives Decoding.
Referenzimplementierung von Whitepaper Kap. 4 und Anhang A.3.

## Aufgabe

Schicht L2 (Compute Layer / Inference Fabric): der „Mining-Loop" — ein Pod
aus k Shard-Minern führt gemeinsam Forward-Pässe aus, koordiniert von einem
Pod-Koordinator, der Micro-Batches bildet und PoI-Bündel einreicht
(Anhang A.3). Diese Komponente ist die **Netzwerk-Orchestrierung** um
INTEGER_LLM herum — sie ersetzt nicht dessen Rechenkerne (`kernels`/
`runtime`), sondern verteilt sie über echte Nodes, mit Session-Affinität,
Ausfallsicherung und Durchsatzoptimierung.

## Abhängigkeiten

NETWORKING (Aktivierungs-Streams zwischen Shards), CONSENSUS
(Epochen-Scheduler liefert Pod-Zusammensetzung) sowie als fachliche
Vorstufe die Mehrknoten-Pipeline von INTEGER_LLM (Stage-Runtime mit echter
Layer-Ausführung über mehrere Knoten): INTEGER_LLMs `pipeline`-Crate liefert
die Stage-Runtime für einzelne Knoten; diese Komponente hebt das auf
Netzwerk-Ebene mit echter Miner-Rotation, Redundanz und Epochen-Wechsel.
`myl-pod` konsumiert die INTEGER_LLM-Stage-API
(`embed_token`/`run_layers`/`head_logits`) und die Typen aus `myl-types`.

## Struktur

```
COMPUTE_PIPELINE/
├── README/                   diese Kurzübersicht
└── myl-pod/                  das Pod-Crate (Bibliothek + Node-Binary)
    ├── src/
    │   ├── lib.rs             Crate-Wurzel: #![deny(unsafe_code)], Module
    │   ├── wire.rs            Drahtformat zwischen Shards (Borsh, Flags)
    │   ├── trace.rs           Spur-Hashes und Übergangssignaturen (BLS)
    │   ├── shard.rs           shard_loop: Eingangsprüfung, Forward,
    │   │                      Signieren, Session-Affinität, DA-Archiv
    │   ├── da.rs              DA-Archivierung (ErasureCoder, XOR-Parität)
    │   ├── micro_batch.rs     Micro-Batching-Fenster und Pipeline-Tracker
    │   ├── standby.rs         Besetzung eines Pods: k Positionen, zwei in
    │   │                      Reserve, Übernahme und Epochenwechsel
    │   ├── zuteilung.rs       vom geprüften Epochenseed zur Besetzung:
    │   │                      die Verbindung zum Epochen-Scheduler
    │   ├── coordinator.rs     coordinator_loop: Micro-Batching, Dispatch,
    │   │                      PoI-Bündel-Aggregation
    │   └── main.rs            myl-pod-node-CLI
    └── tests/
        ├── pod_e2e.rs         Akzeptanztest: Determinismus, Bitgleichheit,
        │                      Manipulationserkennung
        ├── adversarial.rs     böswillige Nachbarn und verstümmelte Spuren
        ├── layer_granular.rs  variable Knotenzahl, Zuschnittsinvarianz
        └── koordinator_byzantinisch.rs
                               ein Koordinator, der falsch aggregiert
```

## Changelog

### v0.25.1 – 2026-09-03 (⚑ Fund 162: die Shardzahl ist jetzt gebunden)

Die Shardzahl der Pipeline und die der Gewichtsableitung standen beide
auf vier, und **nichts im Code hielt sie zusammen**. Eine Abweichung
hätte stumm dazu geführt, dass ein Pod für seine Arbeit nichts bekommt.
Zwei `const`-Zusicherungen binden sie jetzt.

⚑ **Der erste Anlauf war Zierde**, und die Gegenprobe fing ihn: Ein
assoziiertes `const` im `impl`-Block wird erst berechnet, wenn es
jemand benutzt. Auf Modulebene verschoben, beisst es.

### v0.25.0 – 2026-09-03 (das Rechenwerk, das wirklich rechnet)

`pipelinewerk` setzt hinter die Entsiegelung, was bis heute nur ein
Merkmal war: Wortschatz, vier Shards, Koordinator über den geladenen
Artefakten. Damit läuft der Weg von der Türklinke bis zum Modell.

⚑ **Der Pipeline-Stand wird übergeben und nicht ausgerechnet.** Welche
Artefakte gelten, steht **gemessen** in `scale_packs/REGISTER.json`; ihn
hier ein zweites Mal aus den Dateien zu bilden hiesse, zwei Wahrheiten
über die Modellfassung zu führen.

**Der Deckel des Betreibers schlägt den des Auftrags.** Wer eine
schwache Maschine fährt, will nicht, dass ein Auftrag über
`MAX_NEUE_TOKEN` sie minutenlang belegt. Beide sind Obergrenzen: Wer
weniger bekommt als verlangt, bekommt trotzdem eine Antwort.

**Eine Anfrage nach der anderen.** Der Koordinator hält KV-Cache und
Segmentspur; zwei Aufträge gleichzeitig stritten um beide. Das ist keine
Einschränkung, sondern die Aussage darüber, was ein Shard ist: ein
Rechenwerk und keine Warteschlange.

⚑ **Der Artefakt-Wächter steht jetzt in der Bibliothek**, weil ihn eine
zweite Kiste braucht: Der Test von der Türklinke bis zum Modell liegt in
`myl-testclient`, und ein Testmodul ist von dort nicht erreichbar. Zwei
Wächter wachen irgendwann verschieden.

**Und `Inferenzantwort::Ergebnis` trägt die Promptlänge in Token**
(Fund 160), gezählt vom Rechnenden. Die Tür gab vorher die Byte-Länge
als `usage.prompt_tokens` aus, und das ist ein Feld, mit dem ein Klient
Kosten rechnet.

### v0.24.0 – 2026-09-03 (der Shard entsiegelt und prüft die Bindung)

**Der verschlüsselte Kanal zu den Pod-Enden ist zu** (GATEWAY Stufe 4,
Fund 155). `entsiegelung` nimmt einen versiegelten Prompt an, öffnet ihn
und prüft **danach** die Bindung.

⚑ **Der Koordinator ist ein Fremder, und deshalb geht das überhaupt.**
Der Prompt reist versiegelt; wer ihn weiterleitet, sieht ihn nicht. Der
Empfänger entsiegelt und kann die Bindung dann **selbst** prüfen, weil
sie den Klartext bindet, den er nun in Händen hält. Ohne die Prüfung
rechnete er etwas, und niemand könnte später zeigen, dass genau diese
Anfrage es ausgelöst hat.

⚑ **Drei Tore, und ihre Reihenfolge ist die Aussage:** die Form (im
Ortsdienst, vor dem Aufruf), das Entsiegeln (eine KEM-Dekapselung, also
beschränkte Rechenzeit), die Bindung (ein SHA-256). **Erst danach sieht
das Rechenwerk irgendetwas.** Wer das Rechnen vor die Bindung stellte,
liesse jeden, der einen Kanal aufbauen darf, den Pod beliebig rechnen
lassen.

**Der billigste Vergleich zuerst:** Ein Auftrag für einen fremden
Pipeline-Stand kostet einen Hashvergleich und keine Dekapselung.

**Die Zuteilung ist ein Merkmal und keine Nachbildung.** Wer zu einer
Sitzung gehört, steht in der Kette, und `myl-pod` kennt die Kette nicht;
eine eigene Tabelle hier wäre eine zweite Wahrheit darüber, wer zu einem
Pod gehört.

⚑ **Der Text wird hier gerendert, weil nur hier der Wortschatz liegt.**
`Inferenzantwort::Ergebnis` trägt ihn seit heute mit. Bezeugt und
nachgerechnet werden aber die **Token**: Der Text ist eine Auskunft und
kein Beweis.

**Fünf Tests, jeder gegen ein Tor.**

### v0.23.0 – 2026-09-03 (der Shard bekommt eine Tür)

**Bis heute bekam ein Pod seine Arbeit von der Kommandozeile.**
`myl-pod-node --prompt "<text>"`. Ein Auftrag aus dem Netz hatte keinen
Weg hierher, auf keiner der beiden Seiten.

`ortsdienst` ist die lokale Tür: Der Knoten fragt, der Shard antwortet.
Der heisse Pfad (Aktivierungen je Token) bleibt bei `wire` und geht
direkt zum Nachbarshard; **nur der kalte Pfad geht hier durch.**

⚑ **Hier steht die Formprüfung, und hier hat sie zwei Ausgänge.** Am
selben Tag stand dieselbe Prüfung schon einmal im Knoten und wurde
wieder ausgebaut (Fund 154): Dort lieferten beide Zweige dasselbe
`Abgelehnt`, weil der Knoten ohnehin nicht rechnet. Hier liegt hinter
dem einen Zweig ein Rechenwerk und hinter dem anderen nicht. **Eine
Prüfung gehört an die Naht, an der sie etwas unterscheidet.**

⚑ **Das Rechenwerk ist ein Merkmal und keine feste Verdrahtung.** Die
Pipeline braucht geladene Artefakte; ein Test, der jedes Mal ein Modell
lädt, wird nicht gefahren. Der Dienst ist gegen das Merkmal geprüft, die
Pipeline gegen ihre eigenen Tests, und `myl-pod-node` setzt beides
zusammen.

**Eine Verbindung nach der anderen**, und das ist Absicht: Ein Shard ist
ein Rechenwerk, zwei gleichzeitige Aufträge stritten um dasselbe Modell
und denselben KV-Cache. Was das kostet, gehört gesagt: **Ein langsamer
Klient hält die Leitung.** Dagegen stehen Lese- und Schreibfristen,
nicht mehr und nicht weniger.

⚑ **Fund 156, halb hier:** Der Notdeckel gegen einen wachsenden Puffer
ist ausgebaut, weil er nie greifen kann. `NutzlastFehlt` kommt nur,
solange weniger als `KOPF_LEN + laenge` Bytes da sind, und `laenge`
liegt unter der Grenze, sonst wäre es `ZuLang` und die Verbindung fiele
oben heraus. **Regel von Fund 154, auf eigenen frischen Code angewandt.**

⚑ **Fund 157:** Ein Test prüfte „keine gültige Antwort", zugesichert war
„keine Auskunft". Vier Bytes „nein" hätten ihn nicht gestört, und die
sagen einem Fremden, dass er die richtige Tür und den falschen Ausweis
hat. Jetzt zählt er Bytes.

**Sechs Tests über echte Sockets, vier Gegenproben je Zeile.**

### v0.22.1 – 2026-09-01 (Beweiser gegen Prüfer, mit echtem Modell)

`tests/nachrechnen.rs`: `myl-pod` rechnet die Spur eines Shards, wie im
Betrieb; `myl_verifier::ModellAuditor` rechnet sie **auf eigenem Weg**
nach. Vier Tests, gegen echte Artefakte.

⚑ **Ohne diesen Test wäre der Nachrechner eine Vermutung.** Er könnte
richtig aussehen und systematisch etwas anderes rechnen; dann
beschuldigte Stufe 2 ehrliche Miner, und zwar **alle**.

Geprüft wird auch, dass eine verfälschte Spur an der **richtigen
Stelle** gefunden wird: Die Bisektion beginnt bei der ersten Abweichung,
und eine falsch gemeldete Position stritte über die falsche Layer.

`activation_hash` kommt jetzt aus `myl_types::uebergang`; hier steht die
Wiederausfuhr, denn dies ist die Stelle, an der er **erzeugt** wird.

### v0.22.0 – 2026-09-01 (die Signierbotschaft zieht nach, und eine Gegenprobe hat sich bezahlt)

`signierbotschaft` bindet `segmente` mit (Fund 115). ⚑ **Die Kodierung
steht hier zwangsläufig ein zweites Mal**, denn `myl-consensus` ist nur
dev-dependency: `myl-pod` darf zur Bauzeit nicht am Konsens hängen.

⚑ **Was die Kopie zusammenhält, ist der Test
`die_signierbotschaft_des_pods_ist_die_des_konsenses`, und er hat sich
heute bezahlt gemacht.** Als `PoIBundle` das neue Feld bekam, band die
Konsensfassung es und diese nicht; **der Test fiel um, bevor irgendetwas
auseinanderlaufen konnte.** Eine erzwungene Kopie ist tragbar, solange
eine Gegenprobe an ihr hängt; ohne sie wäre sie Fund 111.

### v0.21.0 – 2026-09-01 (Punkt 43: die Zuteilung wird hier nicht mehr gerechnet)

**`plane_epoche` und `Planparameter` sind entfernt.** Fund 111 hatte
gezeigt, dass es die Pod-Bildung zweimal gab; die halbe Antwort war, beide
auf dieselbe Regel zu setzen. **Die ganze ist, dass es den zweiten
Eingang nicht gibt.** Wer die Zuteilung braucht, ruft
`myl_scheduler::zuteilung_der_epoche`.

⚑ **Die Saat ist der vorherige Blockhash, nicht der VRF**, und dafür gibt
es vier Gründe:

- **Den Blockhash gibt es immer.** Ein VRF-Seed muss erzeugt,
  veröffentlicht und geprüft werden; schweigt der Halter, gibt es keine
  Zuteilung. Das ist eine Liveness-Abhängigkeit für nichts.
- **Gemahlen werden können beide.** Wer den letzten Block einer Epoche
  erzeugt, sieht bei beiden Verfahren die entstehende Zuteilung. Begrenzt
  wird das vom Registrierungsschluss bei `e-2`, und der bleibt.
- ⚑ **Der Vorteil des VRF trägt hier nicht.** Er bringt
  Unvorhersehbarkeit **für alle anderen**; die zahlt sich aus, wo jemand
  von frühem Wissen profitiert. Die Zuteilung rechnet jeder im selben
  Augenblick aus demselben Kettenzustand.
- ⚑ **„Verifiziert statt geglaubt" (D3) bleibt, und zwar stärker.** An
  einem Blockhash ist nichts zu glauben, er **ist** die Kette. Auch die
  Epochenbindung bleibt und ist jetzt Bau statt Prüfung: Sie steckt in
  `epochenseed(hash, epoche)`.

⚑ **Wo der VRF hingehört, ist die Stichprobenlotterie.** Dort hilft
frühes Wissen sehr wohl: Wer weiß, welche Segmente geprüft werden, weiß
auch, bei welchen er sich nicht anstrengen muss. Als eigener Punkt
vermerkt.

⛑ **Ein Test kam wieder heraus, statt zu bleiben.** Er rief zweimal
dieselbe reine Funktion und verglich die Ergebnisse; scheitern hätte er
nur können, wenn die Zuteilung zufällig wäre. **Die Zusage „die Regel
steht nur einmal da" hält kein Test, sondern die Abwesenheit des
Codes.**

### v0.20.0 – 2026-09-01 (⚑ Funde 111 und 113)

## ⚑ Fund 113: Elf Tests sprangen still ab und meldeten „ok"

In vier Testdateien stand elfmal `if !dir.exists() { eprintln!("SKIP:
Artefakte fehlen"); return; }`. **`cargo test` fängt die
Standardfehlerausgabe bestandener Tests ab**, die Zeile war also nur mit
`--nocapture` zu sehen. Ohne Artefakte meldete `pod_e2e` „8 passed",
ohne ein einziges Gewicht angefasst zu haben, und das sind die Tests,
die die **Bitgleichheit** belegen.

⚑ **Gefährlich wurde das durch eine anstehende Entscheidung:**
`INTEGER_LLM/artifacts/` belegt 42 GB. Wer sie wegräumt, um Platz zu
gewinnen, bekommt danach eine grüne Suite, die nichts mehr prüft, und
nichts sagt es ihm.

**Jetzt schlägt ein fehlendes Modell fehl**, mit einem Satz, der sagt,
was zu tun ist. Wer bewusst ohne Artefakte läuft, setzt
`MYL_OHNE_ARTEFAKTE=1`; die CI tut das, und dort steht auch warum.
Dieselbe Klasse wie „eine Zählung, die null zählt, ist kein Befund".

## ⚑ Fund 111: die Pod-Bildung gab es zweimal

**`plane_epoche` rechnete die Zuteilung selbst aus**, und sie stimmte mit
dem Weg der Kette in **keinem** der drei Schritte überein:

- Sie clusterte nach **gemessener Latenz**. Die Entscheidung 3b hat das
  am selben Tag verworfen, weil wer wählt, mit wem er attestiert,
  mitformt, in welchem Topf er gemischt wird. **Der Aufruf blieb stehen.**
- Sie nahm die **VRF-Saat**, die Kette die Saat aus dem Blockhash.
- Sie ließ nur die Klassen aus `Planparameter` zu, die Kette alle.

⚑ **Zwei Knoten, die denselben Pod auf verschiedenen Wegen ausrechnen,
bekamen verschiedene Pods.** Dieselbe Lehre wie bei Fund 34: Zwei Quellen
für dieselbe Aussage laufen auseinander.

**Die Regel steht jetzt einmal**, in
`myl_scheduler::zonenzuteilung::zuteilung_aus_saat`; `plane_epoche` prüft
die Saat und ruft sie. `Planparameter::max_latenz_ms` und der
Latenzparameter sind entfallen, **es geht keine gemessene Größe mehr
ein**. Ein Test hält fest, dass beide Eingänge bei gleicher Saat dieselbe
Zuteilung ergeben.

⚑ **Offen bleibt allein, welche Saat gilt**, VRF oder Blockhash. Das ist
eine Entscheidung und kein Algorithmus, und sie ist jetzt eine Zeile groß
statt einer Datei.

### v0.19.0 – 2026-09-01 (Punkt 3.6: wer wiederholt ausfällt, kommt später dran)

Der Ledger führt seit dem 27. August einen Verstoß-Zähler je Konto. Er
entstand für die Slashing-Staffelung und trägt hier ein zweites Mal:
`verteilen_gestaffelt` ordnet die freien Miner nach ihrer
Auffälligkeit im Fenster und erst danach nach der Mischung.

⚑ **Sortiert und nicht ausgeschlossen**, gegen die naheliegende Lösung.
Eine Schwelle („ab drei Verstößen keine Reserve mehr") wäre eine Klippe:
Wer sie überschreitet, ist draußen, und wer knapp darunter liegt, ist so
gut wie ein Fehlerfreier. Eine Ordnung wirkt stetig und hat keinen Rand,
an dem sich rechnen ließe.

⚑ **Und niemand wird ausgeschlossen, auch nicht der Auffälligste.**
Läuft der Vorrat leer, rückt auch er ein, statt die Sitzung zu
verlieren. **Am Rand schlägt Liveness die Bestrafung**, denn ein
verlorener Pod schadet den Anfragenden, der Zähler nur dem Auffälligen.
Wer das anders will, braucht eine Schwelle, und damit den Rand.

⚑ **Die Sortierung ist stabil, und das ist keine Feinheit.** Bei
gleicher Auffälligkeit bleibt die Ordnung der Mischung erhalten, also
entscheidet weiterhin der Seed. Eine unstabile Sortierung machte die
Zuteilung von der Bauart der Standardbibliothek abhängig, und zwei
Knoten mit verschiedenen Rust-Fassungen kämen zu verschiedenen Pods.
Eine Gegenprobe hält genau das fest.

**Der Ledger wird hier nicht gelesen.** `myl-pod` hängt nicht an ihm;
die Zahlen kommen vom Aufrufer, der sie an **einer** Blockhöhe gelesen
hat. Nur so sind sie über zwei Knoten hinweg dieselben.

9 neue Tests, zwei Gegenproben.

### v0.18.0 – 2026-09-01 (Punkt 3.5: eine Ausfallmeldung ist eine Waffe)

Wer melden darf, dass ein anderer ausgefallen sei, kann einen
**ehrlichen** Knoten aus seinem Pod werfen und seinen Platz füllen
lassen. Bis heute genügte dafür ein Funktionsaufruf: eine Behauptung
ohne Absender, ohne Beleg und ohne Frist.

⚑ **Punkt 3.4 hat das Problem vergrößert**, nicht verkleinert: Die
Netzreserve verlängert den Vorrat, den wiederholte Meldungen leerziehen
können. `ausfallmeldung.rs` schließt die Lücke mit drei Stücken, und
jedes leistet etwas anderes.

**Erstens nennt die Meldung, wer ausgefallen sein soll**, nicht nur die
Position. Damit ist eine wiederholte Meldung erkennbar dieselbe Aussage
und eine über den Nachrücker erkennbar eine andere. Ohne dieses Feld ist
die Entprellung aus Punkt 3.1 wirkungslos, sobald einmal nachbesetzt
wurde.

**Zweitens die Gegenzeichnung:** Eine Meldung wirkt erst mit einer
**Mehrheit der übrigen Mitglieder**, mindestens aber zwei. Mehrheit und
nicht Einstimmigkeit, denn Einstimmigkeit gäbe jedem Einzelnen ein Veto
gegen jede Nachbesetzung, und ein Pod, der nicht nachbesetzen kann,
verliert die Sitzung: Der Angriff wechselte nur die Richtung.

**Drittens eine Frist von fünf Sekunden** seit der ersten Unterschrift.
Ohne sie ließen sich Unterschriften über Stunden sammeln und im
günstigen Moment zusammenlegen; ein Ausfall, der vor einer Stunde
bezeugt wurde, sagt über jetzt nichts. Eine zurückspringende Uhr ist ein
Fehler und keine Toleranz, denn sie machte aus einer abgelaufenen Frist
eine laufende.

⚑ **Und die Bauart setzt es durch, nicht die Disziplin.** Eine
Nachbesetzung über diesen Weg verlangt einen `Beschluss`, dessen Felder
privat sind und den es nur aus einer Sammlung mit genug Unterschriften
gibt. **„Ohne Gegenzeichnung wird niemand verdrängt" ist damit eine
Eigenschaft des Typs**, dieselbe Bauart wie bei der Treasury ohne
Schlüssel. Beim Ausführen wird zusätzlich geprüft, dass der Beschluss
Epoche, Pod, Position **und den tatsächlich Amtierenden** meint: Ein
Beschluss über einen längst Ersetzten ist verbraucht.

⚑ **Was das alles nicht leistet:** Gegen eine bösartige **Mehrheit** des
Pods hilft es nicht. Der Pod ist kein BFT-Komitee, und das
Verifikationsmodell geht ausdrücklich davon aus, dass ein ganzer Pod
falsch rechnen kann; genau dafür gibt es den zweiten Pod. Was bleibt,
ist zweierlei: Ein **einzelner** Angreifer kann es nicht mehr, und jede
Verdrängung hinterlässt **unterschriebene Aussagen mit Namen**, die eine
Schiedsstelle lesen kann.

**Offen bleibt der Transport.** Punkt 3.5 nennt „Ausfallmeldung ins
Gossip"; hier stehen Meldung, Sammlung, Frist und Beschluss, nicht ihre
Verbreitung über ein Topic. Das ist Arbeit in NETWORKING und steht als
solche da, statt als erledigt zu gelten.

19 neue Tests, sieben Gegenproben.

### v0.17.0 – 2026-09-01 (Punkt 3.4: der dritte Ausfall kostet die Sitzung nicht mehr)

Kap. 6.8 gibt jedem Pod zwei Reserveplätze und sagt Sitzungsverlust
„nur bei mehr als zwei gleichzeitigen Ausfällen" zu. Beim dritten war
die Sitzung verloren, **auch wenn im Netz hundert freie Miner standen**:
Die Ausfallsicherung kannte nur die eigene Reserve.

Die freien Miner gab es längst und sie waren sogar benannt:
`Zuteilung::ohne_pod` führt jeden registrierten Miner, der in keinen
vollständigen Pod passte. Bisher wartete er auf eine Zuweisung, die nie
kam.

⚑ **Verteilt und nicht gemischt, und das ist der ganze Entwurf.** Der
naheliegende Weg gibt jedem Pod eine gemischte Liste und lässt ihn vorne
zugreifen. Dann greifen zwei gleichzeitig ausfallende Pods nach
demselben Miner. Bei verschiedenen Mischungen ist das selten, und
**„selten" heißt im Konsens „es passiert, und dann sitzen zwei Knoten
mit verschiedenen Besetzungen da"**. Die Netzreserve wird deshalb
aufgeteilt: Jeder freie Miner gehört zu höchstens einem Pod, eine
Kollision ist nicht unwahrscheinlich, sondern unmöglich.

⚑ **Damit hält auch die Disjunktheit des Redundanzpaars von selbst.**
Sie wird bei der Zuteilung geprüft und danach nie wieder; eine
Nachbesetzung zur Laufzeit könnte sie unbemerkt einreißen und aus Stufe
1 der Verifikation eine Selbstbestätigung machen. Ein aufgeteilter
Vorrat kann das nicht.

**Woher jemand kam, steht jetzt im Ergebnis** (`Herkunft::PodReserve`
gegen `Herkunft::Netzreserve`). Die Pod-Reserve ist die Zusage aus
Kap. 6.8; die Netzreserve ist, was in dieser Epoche übrig war, und kann
leer sein. Wer beides gleich meldet, kann später nicht sagen, ob die
Zusage gehalten wurde oder ob es nur gut ausging. Pods ohne Netzreserve
werden genannt statt verschwiegen.

⛑ **Und ein Test, der mich korrigiert hat.** Er erwartete, dass eine
doppelte Ausfallmeldung keine zweite Reserve verbraucht, und lag falsch:
Nach einer geglückten Übernahme **sitzt wieder jemand** auf der
Position, und „der ist ausgefallen" ist dann eine neue Aussage über eine
neue Person. Die Entprellung aus Punkt 3.1 greift nur bei einer
**leeren** Position.

**Daraus folgt eine Eigenschaft, die zu kennen wichtiger ist als der
Test:** Wiederholte Meldungen über dieselbe Position ziehen den ganzen
Vorrat. Vorher endete das nach zwei Meldungen, jetzt nach zwei plus der
Netzreserve. **Das ist kein neues Leck, sondern dasselbe mit größerem
Eimer**, und es steht so im Modulkopf, weil ein größerer Eimer wie eine
Lösung aussieht. Frist und Gegenzeichnung sind Punkt 3.5.

19 neue Tests, drei Gegenproben; eine davon belegt, dass die
Positionsschranke einen echten Absturz verhindert.

### v0.16.0 – 2026-08-30 (der Protokoll-Beleg ist jetzt eine Projektion)

`myl_types::Segment` beschreibt den Segment-Beleg seit dem 2026-08-13
(Anhang A.1), und die Gateway-Planung setzt ihn voraus. **Erzeugt hat
ihn niemand.** Was der Pod wirklich festhielt, war `CompletedSegment`, und
die beiden führten verschiedene Felder.

Zwei Typen für dieselbe Sache sind eine zweite Quelle, und diese hier
hat schon Schaden angerichtet: `Segment` trägt ein `input_commitment`,
`CompletedSegment` hatte keines, und **genau daraus entstand Fund 102**,
die tautologische Prüfung an der ersten Layer.

`CompletedSegment::zu_segment` erzeugt den Beleg jetzt aus dem, was der
Pod ohnehin hat: keine zweite Aufzeichnung, sondern eine Projektion.
`model_version` ist der einzige Wert von außen, denn er gehört zum
Modell und nicht zum Segment.

### v0.15.0 – 2026-08-30 (⚑ Fund 102: die erste Layer hing an nichts)

Beim kritischen Nachlesen der eigenen Änderung gefunden, nicht beim
Bauen. Die Schiedsrunde bindet die strittige Eingabe an `trace[j-1]`,
also an die Ausgabe der Layer davor. **Bei `j = 0` gibt es keine Layer
davor.** Gemeint ist dort die Eingabe des Segments, und die stand
nirgends: `myl_types::Segment` führt ein `input_commitment`, aber diesen
Typ erzeugt niemand.

Seit E10 legt der **Ankläger** die strittige Eingabe vor. An der ersten
Layer prüfte die Schiedsrunde damit `hash(eingabe)` gegen einen Hash,
den derselbe Ankläger daneben schreibt: **eine tautologische Prüfung**,
und genau der Fehler, den Fund A11 an anderer Stelle schon einmal hatte.

`CompletedSegment` trägt jetzt ein `eingangs_commitment`, den Hash der
Token-Nutzlast, und die bezeugte Kette ist `[Eingang] ++ Spur`. Damit ist
`kette()[j]` die Eingabe der Layer `j` und `kette()[j+1]` ihre Ausgabe:
**jede** Layer ist beweisbar, auch die erste.

### Toter Code, den E10 hinterlassen hat

`src/da.rs` ist entfernt, 478 Zeilen. `DaStore`, `ErasureCoder` und die
beiden Kodierer waren nach E10 von niemandem mehr benutzt; die
Erasure-Kodierung des Protokolls sitzt ohnehin in `myl_types::erasure`.
Der Send/Sync-Test aus Fund A17 bleibt: Er bewacht die Eigenschaft, nicht
das damalige Gegenbeispiel.

### v0.14.0 – 2026-08-30 (E10: der Shard archiviert nichts mehr)

Zuvor legte er die eingehende Aktivierung je Segment ab, 65 bis 260 GiB
je Knoten über die Streitfrist. **Auch das war zu viel**, und die Lösung
lag nicht im Sparen, sondern in der Beweislast: Der **Ankläger** bringt
die strittige Aktivierung mit, denn die Bisektion endet an der ersten
Abweichung, bei `j-1` sind sich beide einig, und er hat das Segment
gerade nachgerechnet.

`DaStore` ist damit aus `ShardNode` verschwunden, samt Feld,
Konstruktor-Parameter und Mutex im heißen Pfad. Was bleibt, ist die
**Spur**: 32 Byte je Layer, und sie ist kein zusätzlicher Speicher,
sondern der Arbeitsnachweis, den es ohnehin gibt. **2 bis 8 GiB je
Knoten.**

### ⚑ Und die Spur bezeugt jetzt auch etwas (Fund 100)

`CompletedSegment` trägt eine `spurwurzel`, und sie geht in die
Bündelwurzel ein. Vorher stand dort nur die `SegmentId`, also
`(Sitzung, Position)`: Das Bündel beanspruchte Arbeit, **ohne zu sagen,
was gerechnet wurde**.

Die Wurzel entsteht beim Abschluss des Segments und nicht erst beim
Bündeln: Eine Zusicherung, die später entsteht, ist eine, die man sich
noch überlegen kann.

Der E2E-Test prüft jetzt die Spur statt des Archivs: Sie ist je Position
eine andere, und die Zusicherung ist wirklich die Wurzel über die eigene
Spur.

### v0.13.0 – 2026-08-29 (E9: das Archiv hält den Eingang, die Ausgaben werden nachgerechnet)

Bis dahin legte jeder Shard die Ausgabe **jeder** seiner Layer ab. Bei
28 Layern auf vier Shards sind das sieben Vektoren je Segment statt
einem, und über die Streitfrist zwischen **455 GiB und 1,8 TiB je
Knoten**, zusätzlich zur Modellgröße. Für ein Rechenzentrum ist das
nichts, für niedrigschwellige Teilhabe zu viel.

**Die Layer-Ausgaben sind ableitbar, die eingehende Aktivierung nicht.**
Bei bitgenauer Ganzzahl-Inferenz ergibt derselbe Eingang stets dieselben
Ausgänge; wer `a_{start-1}` hat, rechnet jede Ausgabe seines Bereichs
noch einmal, Bit für Bit. Der Eingang selbst kommt vom vorigen Shard und
ist von diesem Knoten aus nicht herstellbar.

| je Segment und Shard | | je Knoten, 168 Epochen |
|---|---|---|
| jede Layer-Ausgabe (vorher) | 73 KiB | 455 GiB bis 1,8 TiB |
| nur der Eingang | 10 KiB | **65 bis 260 GiB** |

### ⚑ Warum nicht noch weniger, etwa nur die Token

Ein Shard hält `layer_start..layer_end` und sonst nichts. Aus den Token
nachrechnen könnte nur der Pod **gemeinsam**. Dann hinge die Antwort
eines Angeklagten an der Mitwirkung seiner Nachbarn, und „Schweigen
heißt Schuld" träfe den, dessen Nachbar schweigt.

Die eingehende Aktivierung ist genau die Grenze dessen, was ein Shard
**allein** beantworten kann, und diese Eigenständigkeit ist die
Voraussetzung dafür, dass die Antwortfrist fair ist.

⚑ **Das berichtigt zugleich eine eigene Zahl.** Der Vorschlag, der zu
dieser Umstellung führte, nannte Faktor 224 und setzte dabei voraus,
dass ein Shard aus den Token nachrechnen kann. Er kann es nicht. Der
Faktor ist die Zahl der Layer je Shard, also 7.

### Der Test fordert die Nachrechnung, statt sie vorauszusetzen

Er prüfte bisher, dass der archivierte Wert selbst der von der Spur
committete ist. Jetzt prüft er, dass der Eingang es **nicht** ist, dass
die **nachgerechnete** Ausgabe es ist, und dass ein Shard eine fremde
Layer **nicht** beantwortet. Dass beim Nachrechnen Bit für Bit dasselbe
herauskommt, ist keine Nebensache: Ein Archiv, dessen Nachrechnung
abwiche, ließe jeden ehrlichen Knoten als schuldig dastehen.

### v0.12.0 – 2026-08-29 (der Übergangsvertrag zieht in die gemeinsame Kiste)

`TransitionSig`, `Rolle` und `DST_SHARD_TRANSITION` stehen jetzt in
`myl_types::uebergang`. Der Grund liegt außerhalb dieses Crates: Die
Unterschrift eines Shards unter seinen Rechenschritt wird nicht dort
gebraucht, wo sie entsteht, sondern bei der Schiedsstelle, und
`myl-verifier` hängt nicht an `myl-pod` und soll es auch nicht, daran
hinge die ganze Inferenz-Laufzeit. Die Folge war, dass die
Unterschriften erzeugt, eingesammelt, aggregiert und von niemandem
geprüft wurden.

Für Nutzer dieses Crates ändert sich nichts: `myl_pod::trace` reicht die
drei Namen weiter durch, und dies bleibt die Stelle, an der sie benutzt
werden. Fünf Tests sind mit dem Vertrag umgezogen, deshalb steht hier
76 statt 81.

⚑ **`PodMessage.signature` prüft weiterhin niemand beim Empfang**, und
das ist richtig so: Der Empfänger prüft die Aktivierungen gegen den
Spur-Hash, das ist die Manipulationserkennung. Die Signatur ist die
Zuschreibung für den Streitfall, und geprüft wird sie dort, wo
zugeschrieben wird (VERIFICATION v0.11.0).

### v0.11.0 – 2026-08-26 (Punkt 3.3: der Scheduler ist verdrahtet)

`src/zuteilung.rs` schließt die Lücke, die seit dem
2026-08-24 als „Schnittstelle ✅, Verdrahtung ❌" führte. `plane_epoche`
geht vom **geprüften** Epochenseed über Filter, Clusterbildung und
Pod-Zuteilung; `epochenwechsel_aus_zuteilung` speist das Ergebnis in die
`PodBesetzung`.

⚑ **Warum das keine Fleißarbeit war: die beiden Seiten passen nicht
zusammen.** `assign_shards` legt **mehrere** Miner in jeden Shard
(gemessen: sechs Miner auf vier Shards ergeben `[2,2,1,1]`), während
Kap. 6.8 und das Glossar von **einem** Miner je Shard-Position plus zwei
in Reserve sprechen. Zwei von drei Beschreibungen sagen dasselbe, der
Code des Schedulers etwas anderes. Solange niemand die Seiten
zusammensteckte, konnte das nicht auffallen: Jede ist für sich stimmig
und vollständig getestet.

✅ **Entschieden am selben Tag (D3): der Code richtet sich nach dem
Papier.** `myl-scheduler` v0.3.0 liefert Pods mit `k` Positionen zu je
einem Miner und zwei in Reserve. Damit ist `zuteilung.rs` eine
**Übersetzung** statt einer Brücke: Die Reihenfolge, die
`PodBesetzung::neu` erwartet, ist genau `Pod::mitglieder`.

Was in keinen vollständigen Pod passt, steht in `Zuteilung::ohne_pod`;
`ist_besetzbar` prüft **vor** dem Wechsel statt mitten in der Sitzung.

**Der Seed wird geprüft, nicht geglaubt.** `plane_epoche` verifiziert den
VRF-Beweis selbst und verlangt, dass die Epoche des Seeds passt. Wer den
Seed frei wählt, wählt seine eigenen Pod-Nachbarn; ein gültiger Seed der
vorigen Epoche hielte die alte Zuteilung fest.

### v0.9.0 – 2026-08-24 (⚑ Fund 52 geschlossen: der Vergütungspfad ist durchgängig)

`build_signed_poi_bundle` schließt die Naht: **erst steht das Bündel,
dann sehen es die Mitglieder, dann unterschreiben sie seine Botschaft,
dann aggregiert der Koordinator.**

**Die Reihenfolge ist die ganze Sicherheit.** Würde der Koordinator
zuerst sammeln und danach das Bündel bauen, könnte er `vtfe_claimed`
nachträglich erhöhen; die Unterschriften lägen über einer Botschaft, die
niemand gesehen hat.

#### Ein Mitglied prüft, bevor es unterschreibt

Eine Unterschrift, die ohne Prüfung gegeben wird, ist **keine
Zustimmung**, sondern eine Anwesenheitsnotiz. Kap. 5.5 belegt falsche
PoI-Aggregation mit 100 % Slash des Koordinators, und das setzt voraus,
dass die Mitglieder etwas anderes bezeugen als „ich war dabei".

`ShardNode::signiere_buendel` rechnet deshalb den Anspruch nach: Passt
`vtfe_claimed` zu der Segmentzahl, die dieses Mitglied gesehen hat, mit
derselben Regel, die der Koordinator anwendet? Weicht sie ab, wird nicht
unterschrieben, und dann gibt es kein Bündel: Ein Aggregat gilt gegen
**alle** Mitglieder, nicht gegen eine Mehrheit.

**Was ein Mitglied lokal nicht prüfen kann, steht dabei:** die
Merkle-Wurzel über die Segmentmenge. Es kennt seine eigenen Segmente,
aber eine Wurzel ist ohne die vollständige Liste nicht nachrechenbar; der
Koordinator liefert sie mit, und die Prüfung ist dann eine über
gelieferte Daten und nicht über eigene. Eine schwächere Aussage, und sie
steht im Code, damit niemand die Unterschrift für mehr hält, als sie ist.

#### Belegt gegen das echte Modell

Der Test fährt einen Vier-Shard-Pod gegen die INTEGER_LLM-Runtime: Das
**unsignierte** Bündel wird von `myl_consensus::verify_bundle_signature`
abgelehnt, das **signierte** gilt, und ein nachträglich erhöhter Anspruch
fällt wieder durch. Ohne Artefakte überspringt er sich.

*Zur Domain-Separation:* Die Bündelbotschaft trägt `DST_POI_BUNDLE` und
ist damit von der Übergangssignatur mit `DST_SHARD_TRANSITION` getrennt.
Ein eigenes Rollenbyte wie in `trace::Rolle` braucht es hier nicht: **Wo
eine Klasse ihre eigene DST hat, ist die Rolle darin schon enthalten.**

### v0.8.0 – 2026-08-24 (Punkt 4.3: der byzantinische Koordinator, und ⚑ Fund 52)

Der Koordinator ist die einzige Stelle im Pod, die **für alle spricht**.
`tests/koordinator_byzantinisch.rs` wehrt fünf Angriffe ab, die
Gegenprobe steht davor.

#### ⚑ Fund 52: Der Pod baut ein Bündel, das der Konsens nicht prüfen kann

`build_poi_bundle` aggregiert die **Übergangs-Signaturen** der Segmente
(`DST_SHARD_TRANSITION ‖ Rolle ‖ Borsh(TransitionSig)`).
`myl_consensus::verify_bundle_signature` prüft gegen die
**Bündelbotschaft** (`DST_POI_BUNDLE ‖ epoch ‖ pod ‖ segments_root ‖
vtfe_claimed`). **Zwei verschiedene Botschaften; ein Bündel aus dem Pod
verifiziert nie.**

**Die Richtung ist die gute:** abgelehnt statt angenommen, niemand bekäme
Vergütung, die ihm nicht zusteht. Es heißt aber auch, dass **überhaupt
niemand** Vergütung bekommt — der PoI-Pfad ist nicht ungeprüft, er ist
unbenutzbar.

**Was fehlt, ist ein Protokollschritt, keine Zeile Code.** Die Mitglieder
müssen das **fertige** Bündel sehen und seine Botschaft unterschreiben;
erst dann gibt es ein Aggregat, das gegen die Mitgliedermenge gilt. Der
Koordinator kann das nicht allein, sonst wäre die Zustimmung der
Mitglieder eine Fiktion, und genau gegen diese Fiktion ist die Signatur
da (Kap. 5.5: 100 % Slash bei falscher Aggregation).
`Coordinator::signierbotschaft` liefert die Botschaft; die
Signaturrunde darüber ist ein offener Punkt.

**Warum es niemandem aufgefallen ist:** `myl-pod` hing bis heute nicht an
`myl-consensus` und umgekehrt. Beide Seiten sind für sich getestet, die
Naht dazwischen hat nie jemand zusammengesteckt. Genau der Fall, für den
die Härtungsschleife geschrieben wurde.

*Nebenbei:* Die Signierbotschaft steht jetzt an zwei Orten, weil `myl-pod`
nicht an `myl-consensus` hängen soll. **Deshalb prüft ein Test über 1000
zufällige Bündel, dass beide Kodierungen bitgleich sind** — eine Dublette
ohne diesen Test liefe irgendwann auseinander, und dann wäre der Streit
nicht mehr entscheidbar.

### v0.7.0 – 2026-08-24 (Phase 3: Ausfallsicherung und Epochen-Übergang)

`src/standby.rs`: Standby-Übernahme (3.1) und KV-Cache-Rebuild bei
Ausfall oder Epochenwechsel (3.2).

**Kap. 6.8 macht eine quantitative Zusage**, und beide Hälften werden
geprüft: bis zu zwei gleichzeitige Ausfälle übersteht die Session, **drei
nicht**. Die zweite Hälfte ist die, die man vergisst; eine
Implementierung, die bei drei stillschweigend weiterliefe, verspräche
eine Redundanz, die es nicht gibt.

**Warum die Übernahme nicht „einen anderen Miner nehmen" ist:** Der
Standby hat keinen KV-Cache und muss ihn **bitgleich** nachbauen, sonst
weicht die Spur ab dem Übernahmezeitpunkt ab und der Redundanzvergleich
meldet einen ehrlichen Pod als fehlerhaft.

**Bitgleich ist hier billig zu haben, und das ist kein Zufall:** Die
Rechnung ist ganzzahlig und damit reihenfolgeunabhängig, ein Prefill über
dieselben Token liefert denselben Cache, unabhängig von Maschine und
Parallelisierung. **In einem Gleitkomma-System wäre der Cache-Rebuild ein
Bruch der Sitzung.** Das ist eine Folge der Grundentscheidung des
Projekts, die im Whitepaper so nicht steht.

`RebuildAnlass` hat **genau zwei Werte**, und der Typ ist die
Durchsetzung von Kap. 4.2 („nur bei Ausfall oder Epochenwechsel
ausgelöst"): Ein dritter Grund ließe sich nicht eintragen, ohne dass
jemand ihn benennt. Der Epochenwechsel liefert Rebuild-Aufträge **nur für
gewechselte Positionen**; wer bleibt, behält seinen Cache.

#### Zwei Fallen, die der Bau sichtbar gemacht hat

- **Eine doppelt gemeldete Ausfallmeldung darf keine zweite Reserve
  verbrauchen.** Im Netz sind doppelte Meldungen der Normalfall.
  Verbrauchte jede einen Platz, wäre die Zusage „zwei Ausfälle" in
  Wahrheit „eine Meldung", und ein Angreifer, der dieselbe Meldung
  dreimal schickt, verlöre die Session ohne jeden Ausfall.
- **Ein Miner darf nicht zweimal im Pod stehen.** Sonst wäre sein Ausfall
  zwei gleichzeitige, und die Zusage rechnete mit einer Redundanz, die es
  nicht gibt.

#### ⚠️ Punkt 3.3 trägt kein volles Häkchen

Die Schnittstelle steht, die **Verdrahtung nicht**: Es gibt keine Stelle,
die den Scheduler befragt und das Ergebnis einspeist, weil es **kein
Knoten-Binary gibt**, das `myl-scheduler` und `myl-pod` zusammenführt.
Dieselbe Lücke wie bei K1.

### myl-pod v0.6.0 – 2026-08-23 (Fund 41: die Manipulationserkennung ging leer durch)

Angelegt wurde eine adversariale Testebene für das Drahtformat (K4).
**Sie fand beim ersten Lauf einen Fehler**, und zwar einen ernsten.

`verify_input_hash` lieferte bei **leerer Spur** `true`, mit der
Begründung im Code: *„Shard 0: noch kein Spur-Eintrag, Token-Eingang."*
Für Shard 0 stimmt das, aber **dieser Zweig wird von dort gar nicht
erreicht**: Der Token-Eingang kehrt in `process` vorher zurück. Erreicht
wird er ausschließlich auf dem Aktivierungspfad, und dort heißt eine
leere Spur: Jemand schickt Aktivierungen ohne jeden Nachweis, woher sie
kommen.

**Zwei Folgen, beide schlecht.** Die Manipulationserkennung, also der
Kern von Anhang A.3 Schritt 2, ging **vacuously** durch: Ein Shard
rechnete auf fremden Zahlen weiter. Und passte deren Länge nicht zum
Modell, endete das in einer **Panik** im Kernel (`rmsnorm_i16` prüft per
`assert_eq!`), also in einem Absturz, den jeder auslösen kann, der Bytes
schicken darf. Im offenen Netz ist das ein Denial-of-Service.

**Behoben zweifach**: `None => false`, denn eine leere Spur belegt
nichts, und eine Längenprüfung vor dem Kernel, die aus der Panik ein
`Err` macht. Ein Kernel darf nie mit einer Eingabe laufen, deren Länge
nicht zum Modell passt.

**Die neue Testebene** (`tests/adversarial.rs`, fünf Tests): zufällige
und verstümmelte Nachrichten gegen die Deserialisierung, gekippte Bits in
gültigen Nachrichten, abgeschnittene Nachrichten, `unpack_tokens` gegen
jede Nutzlast, und 2000 fremde Nachrichten gegen einen echten Shard, die
**alle** abgelehnt werden müssen.

> **Auch dieser Test war zuerst wertlos, und das steht hier, weil es der
> Punkt ist.** Der erste Anlauf zog rein zufällige Bytes, und davon
> deserialisierte in 50 000 Versuchen **kein einziger**: Borsh liest
> zuerst Längenfelder, und ein zufälliges u32 verlangt gleich mehrere
> Gigabyte. Der Test prüfte also nur, dass Ablehnen nicht abstürzt, und
> nie, was mit einer angenommenen Nachricht geschieht. Aufgefallen ist es
> allein an der Auskunftszeile, die die Null zeigte. Jetzt wird
> strukturiert gezogen, gültiger Kopf und zufälliger Rest, und **1156 von
> 50 000 kommen durch**; eine Zusicherung hält das künftig fest.

### myl-pod v0.5.0 – 2026-08-23 (Spur je Layer, und zwei Funde am Weg dorthin)

**Der letzte Blocker der variablen Knotenzahl ist weg.** Die Spur hatte
einen Eintrag je **Shard**, ihre Länge hing also am Zuschnitt.
`myl_verifier::compare_commitments` lehnt ungleiche Spurlängen mit
`LengthMismatch` ab, und damit mussten zwei redundante Pods denselben
Zuschnitt tragen. Genau das verbietet der Entwurf für variable
Knotenzahl je Pipeline, dessen gemischte Paarung rund 600-mal sicherer
ist als zwei schnelle Pipelines.

Jetzt trägt die Spur einen Eintrag je **Layer**: `num_layers` Einträge,
gleichgültig ob ein Shard rechnet oder vierundzwanzig. **Gemessen** an
0,5B über k = 1, 2, 3, 4, 6, 8, 12 und 24: dieselbe Spur, Eintrag für
Eintrag, und `compare_commitments` urteilt bei vier gegen acht Shards
`Match`.

Dass ein Aufruf je Layer dasselbe liefert wie ein Bereichsaufruf, ist
nicht hergeleitet, sondern gemessen (`tests/layer_granular.rs`, drei
Positionen, echtes Modell samt KV-Cache). **Der Dekodier-Digest hat sich
nicht bewegt:** `272f1ee8f45f2c78` vor und nach dem Umbau, bei jedem
Zuschnitt.

**Eine Signatur je Shard, auch wenn die Spur je Layer wächst.** Die Spur
ist der Vergleichsgegenstand und braucht Layer-Granularität; die
Signatur ist die Zuschreibung und braucht Shard-Granularität, denn
geslasht wird ein Shard und keine Layer.

**Nebengewinn:** Die Bisektion grenzt die fehlerhafte **Layer** ein statt
der Layer-Gruppe, bei unverändertem O(log L).

### Fund 1: Spur und Datenarchiv überlebten nur die letzte Position

`DaStore` war mit `(segment_id, shard_index)` verschlüsselt und kannte
**keine Position**. `archive` wird je Token-Position aufgerufen, jede
Position überschrieb also die vorige; am Ende lag nur die letzte im
Archiv. Im Koordinator dasselbe eine Ebene höher: `trace = out_trace`
ersetzte die Spur je Position, `CompletedSegment.trace` trug am Ende nur
die letzte.

**Die Wirkung ist Slashing ehrlicher Knoten.** Die Streitfrist soll dem
Angeklagten erlauben, die Aktivierung an der strittigen Position
offenzulegen; `adjudicate` verlangt sie ausdrücklich. Lag die Abweichung
an irgendeiner Position außer der letzten, konnte ein **ehrlicher** Miner
sie nicht liefern, `adjudicate` sah `NoResponse`, und das heißt schuldig.

Behoben durch die Festlegung des Projektinhabers: **Ein Segment ist eine
Position**, also genau ein Vorwärtspass. Damit entfällt die
Positionsachse, statt nachgerüstet zu werden, und Spur, Archiv und
Bisektion tragen dieselbe Achse, nämlich die Layer.

**Folge für die Vergütung:** Prefill zählt mit. Eine Prompt-Position
emittiert kein Token, rechnet aber denselben vollständigen Vorwärtspass.
Aus 8 Token über 7 Prompt-Token werden 14 Segmente statt einem.

### Fund 2: Die Spur des letzten Shards landete nie im Segment

`ShardOut::Token` und `ShardOut::Prefill` trugen weder Spur noch
Signatur, und der Koordinator übernahm beides ausschließlich aus
`ShardOut::Forward`. Der **letzte** Shard endet aber immer in einem der
beiden anderen Zweige.

Der Redundanzvergleich verglich damit die Arbeit des Shards nicht, der
die Ausgabe erzeugt, also ausgerechnet die des LM-Kopfes. Und bei einem
Pod aus einem einzigen Shard gab es gar kein `Forward`: Die committete
Spur war **leer**, und ein PoI-Bündel darüber hätte nichts belegt.

Aufgefallen, weil die vTFE-Zuschreibung bei `k = 1` plötzlich null ergab.
Ein Test, der eine Eigenschaft prüft, die es vorher nicht gab, hat einen
Fehler gefunden, der vorher schon da war.

### myl-pod v0.4.0 – 2026-08-23 (der Pod beansprucht, was er gerechnet hat)

**`build_poi_bundle` beanspruchte als vTFE die Zahl der Segmente.** Im
Code stand dazu „Platzhalter für die FLOPs-Metrik". Ein Bündel über
tausend Token beanspruchte damit dieselbe eine Einheit wie eines über
zwei, und ein Shard mit sieben Layern dasselbe wie einer mit zweien.

Ausdrücklich vermerkt war die Warnung davor, die Zuschreibung
festzulegen, *„bevor die erste Implementierung sie stillschweigend
trifft"*. Sie hatte sie längst getroffen.

**Jetzt:** `Coordinator::beanspruchte_vtfe()` rechnet nach der Regel aus
`myl_tokenomics::vtfe` (v0.3.0), also nach dem Anteil eines Shards an den
Multiplikations-Additionen der Gewichtsmatrizen eines vollen
Vorwärtspasses, mal der Zahl der erzeugten Token. `CompletedSegment`
trägt dafür neu die Token-Zahl; ohne sie ist die Gutschrift nicht
auszurechnen. `ShardNode::modell_profil()` und `ShardNode::zuschnitt()`
reichen die nötigen Maße heraus.

**Gemessen:** acht Token über vier Shards ergeben **7 999 999** von
8 000 000 Einheiten; die eine fehlende ist die Abrundung, die
ausdrücklich nach unten geht.

**Als Test festgehalten ist die Eigenschaft, auf die es ankommt:**
Zuschnitte von 1 bis 24 Shards beanspruchen dieselbe Summe
(`beanspruchte_arbeit_haengt_nicht_am_zuschnitt`). Ohne sie wäre die
gemischte Paarung aus dem Entwurf für variable Knotenzahl ökonomisch
nicht neutral, und der billigere Zuschnitt setzte sich durch, ohne besser
zu sein. Ein zweiter Test hält fest, dass der LM-Kopf beim letzten Shard
durchschlägt: Bei gleich vielen Layern beansprucht er mehr als das
Doppelte des ersten.

**Neue Abhängigkeit:** `myl-tokenomics`. Sie hängt selbst nur an
`myl-types`; die Richtung stimmt, denn die Regel gehört zur Ökonomie und
nicht in die Pipeline.

### myl-pod v0.3.0 – 2026-08-23 (Fund 36 abgeschlossen: der Pod gibt seine Zahlen heraus)

**Der Vergleich „Pod gegen Einzelknoten" prüfte, ob die Aufteilung
dieselbe Entscheidung erzeugt, nicht dieselben Zahlen.** Das war der
letzte offene Teil von Fund 36 und der Grund, warum er dort ein ⚑ trug.
`run_prompt` liefert Token; die Logits entstehen im Shard mit dem LM-Head
und verließen ihn nie. Verglichen wurde deshalb Token gegen Token, und
das Ergebnis hieß trotzdem „bitgleich".

Wie grob das ist, steht in Fund 36 mit Zahlen: An 0,5B blieb der Token
unverändert, während 0,1 % der Bytes eines Tensors verschoben waren. Ein
Token ist ein Argmax über 151 936 Zahlen und kippt erst, wenn deren
Rangfolge kippt.

**Neu: `ShardNode::dekodier_digest` und `Coordinator::dekodier_digest`.**
Der Shard mit dem LM-Head führt je Session einen Digest über **Logits und
Token** mit, nach demselben Vertrag wie der Einzelknotenlauf
(`integer_llm_runtime::generate::DekodierDigest`, neu in runtime v0.18.0).
Beide Werte sind damit unmittelbar gegeneinander zu halten.

**Warum im Shard und nicht im Koordinator:** Die andere Lösung wäre, die
Logits herauszureichen. Das sind rund 600 KB je Token für einen Messwert;
der Digest sind 32 Bytes. Gehasht wird strömend, ein Zwischenpuffer über
den ganzen Lauf wären bei 0,5B und 32 Token rund 19 MB.

**Gemessen, nicht behauptet** (0,5B, `myl-test shard`): Pod und
Einzelknoten liefern denselben Wert `df54ef6c89f1a840`, und zwar bei 1, 2,
3, 4, 6, 8, 12 und 24 Shards. Die Gegenprobe lief auch: Mit einem
einzigen um eins verschobenen Logit weit unterhalb des Argmax bleiben die
Token identisch (`f1117a59462f9919` auf beiden Seiten), der Lauf schlägt
aber fehl und meldet ausdrücklich, dass er vor dieser Fassung „bitgleich"
gemeldet hätte.

**Der Akzeptanztest selbst trug den Fehler.**
`tests/pod_e2e.rs::pod_deterministisch_und_bitgleich_mit_einzelknoten`
verglich Token und hieß trotzdem so. Er hält jetzt die Digests
gegeneinander, prüft die Schrittzahl mit und hat eine Gegenprobe
daneben (`ein_verschobenes_logit_bewegt_den_digest`): Ein Digest, der sich
nie bewegt, besteht jeden Gleichheitstest.

**Fund 38, nebenbei und offen:** Die `pipeline_hash`-Bindung wurde
damit begründet, dass verschiedene Shard-Layouts verschiedene Token
lieferten. Das stimmte, solange die Boundary-Reskalierung existierte, und
die ist seit Fund 20/26 ersatzlos entfallen. Acht Layouts von 1 bis 24
Shards liefern heute denselben Digest. Die Bindung bleibt trotzdem im
Code: Sie kostet nichts, und wer sie entfernt, muss die Zuschnitts-
invarianz erst für jedes künftige Modell neu belegen.

*Zur Nummer: Die Crate stand bereits auf v0.2.4, dieser Kopf nannte noch
0.2.2. Der Sprung auf 0.3.0 zieht beides zusammen und markiert die neue
öffentliche Schnittstelle.*

### myl-pod v0.2.4 – 2026-08-19 (Reed-Solomon hinter der bestehenden Schnittstelle)

**Ein Fund über die eigene Arbeit.** Beim Bau der DA-Schicht für
CONSENSUS 4.3 hatte ich die Erasure-Mathematik in `myl-types` neu
angelegt — richtig — aber übersehen, dass `myl-pod` bereits eine
`ErasureCoder`-Schnittstelle mitbringt und der Modulkopf ausdrücklich
sagt: *„Die beschlossene Reed-Solomon-Variante (k=8/m=4) ist eine
Folge-Implementierung hinter derselben Schnittstelle."* Es gab also
einen vorgesehenen Platz, und ich hatte danebengebaut statt hinein.
Aufgefallen ist es erst, als `myl-testclient` nicht mehr übersetzte.

Jetzt: `ReedSolomonCoder` implementiert die vorhandene Schnittstelle und
setzt auf `myl_types::erasure` auf.

**Er behebt zugleich die dokumentierte Phase-1-Einschränkung.**
`XorParityCoder` legt den Längenkopf ungeschützt an den Anfang von
Fragment 0 und kann deshalb nicht rekonstruieren, wenn ausgerechnet
dieses Fragment fehlt (*„vollständige Kopf-Rekonstruktion folgt mit RS"*
stand im Code). `ReedSolomonCoder` stellt die Länge dem Klartext voran
und **codiert sie mit** — es gibt kein ausgezeichnetes Fragment mehr,
jede Teilmenge der Größe k genügt. Statt einem fehlenden Fragment
verträgt er **vier beliebige**; getestet über alle 495 Kombinationen.

`XorParityCoder` bleibt erhalten, der Modulkopf sagt jetzt aber, welcher
zu nehmen ist. `myl-testclient` nutzt den neuen.

### myl-pod v0.2.3 – 2026-08-19 (Fund 26 + Fund 20: Boundary-Schritt entfallen)

**Die Spur band den falschen Wert.** `ShardNode::process` bildete
`out_hash = activation_hash(&out)` über die Aktivierung in natürlicher
Ausgangsskala und schrieb ihn in die Spur; erst danach reskalierte
`finish()` auf die Boundary-Skala, und **dieser** Wert ging als `payload`
auf die Leitung. Der Folge-Shard prüfte mit
`verify_input_hash(&msg.payload, &msg.trace)` — also den Hash des
reskalierten Nutzdatensatzes gegen den Hash des unreskalierten. Beide
stimmen nur überein, solange die Reskalierung die Identität ist; seit
Fund 20 war sie es nicht. Der E2E-Test lehnte damit selbst die
**unmanipulierte** Aktivierung ab.

Das war mehr als ein roter Test: Die Spur ist die Commitment-Kette, die
VERIFICATION zwischen redundanten Pods vergleicht und die das
Bisektions-Spiel halbiert. Committet sie etwas anderes als das, was
übertragen wird, bindet sie nicht die ausgelieferte Arbeit.

**Behoben, indem der Boundary-Schritt ganz entfällt.** Er war reiner
Verlust ohne Gegenwert: Die Ausgangsskala des Senders ist
`layers[layer_end].residual_in_frac`, die Eingangsskala des Empfängers
`layers[layer_start].residual_in_frac` — und `layer_start` des Empfängers
**ist** `layer_end` des Senders. Beide Seiten lasen denselben Wert aus
demselben Artefakt (erzwungen durch `theta_v_hash`) und rechneten ihn
trotzdem über einen dritten, gröberen Skalar hin und zurück. Entfernt:
`rescale_von_kanal`, `rescale_zu_kanal`, `input_scale`, `output_scale`
und das Feld `boundary_frac`.

**Fund 20 fällt damit mit.** `test_pipeline_multinode.py` ist wieder
bitgleich mit der Einzelknoten-Runtime (vorher Divergenz ab dem sechsten
Token, 2746 gegen 2694); der weiche Zweig im Test ist zurück in ein
hartes `assert` überführt, wie es der Kommentar dort vorsah. Die
Phase-1-Akzeptanz „bitgleich mit Einzelknoten" gilt wieder
uneingeschränkt.

**Nachweis:** `pod_e2e.rs` 2/2 (vorher 0/2), Multi-Node-Integration
vollständig, Konformitätsvektoren 30/30, Gleitkomma-Audit null Treffer.
**θ_v unverändert** — die Einzelknoten-Inferenz war nie betroffen.

**Nachtrag am selben Tag — die Layout-Frage ist gemessen.** Drei Layouts
(4 Shards mit Grenzen 6/12/18, 8 Shards mit 3/6/9/…, und ungleichmäßig
1/7/23) liefern dieselben Token und sind bitgleich mit dem Einzelknoten
(`INTEGER_LLM/tests/integration/test_pipeline_layouts.py`). Damit trägt
der Entwurf „variable Knotenzahl je Pipeline" numerisch; von seinen zwei
Blockern ist der erste weg.

**Korrektur:** Oben stand zunächst, die Layout-Bindung aus Fund 25
blockiere diesen Entwurf. Das stimmt nicht — `verify_layout()` prüft das
Manifest gegen sich selbst, nicht gegen andere Pods. Cross-Pod-Gleichheit
erzwingt keine Codestelle. Die Prüfung bleibt trotzdem: Sie hat gerade
den `sha256:0000`-Platzhalter der 8-Node-Konfiguration gefangen.


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

### v0.2.1 – 2026-08-17 (Phase 2.1: Micro-Batching + Pipelining)
- Micro-Batching-Collector mit konfigurierbarem Zeitfenster (default 250 ms)
  und Max-Batch-Größe (default 32). Pipeline-Tracker für überlappende
  Batch-Verarbeitung (4 Stadien: Receiving, Processing, Finalizing, Completed).
- Neues Modul `micro_batch.rs` mit 10 Tests grün.

### v0.1.4 – 2026-08-13 (Phase 1)
- `shard_loop` (Anhang A.3): Aktivierungen empfangen, Eingangs-Hash gegen
  die Spur prüfen (Manipulationserkennung), Forward-Pass über die
  INTEGER_LLM-Stage-API, Spur fortschreiben, Übergang BLS-signieren,
  weiterreichen; KV-Cache je Session (Session-Affinität, Kap. 4.2);
  DA-Archivierung der Aktivierungen (Anhang A.3 Schritt 6).
- `coordinator_loop` (Anhang A.3): Micro-Batching-Fenster (Default 250 ms),
  Session-/Segment-Id-Zuweisung, Pipeline-Dispatch, PoI-Bündel-Aggregation
  (Segments-Wurzel + BLS-Aggregat).
- **Akzeptanzkriterien erfüllt:** 4-Node-Pod liefert bitgleiche
  Token-Sequenz bei wiederholtem identischem Prompt und ist bitgleich mit
  der Einzelknoten-Runtime; Eingangs-Hash-Prüfung lehnt manipulierte
  Aktivierungen/Spur-Hashes ab (`tests/pod_e2e.rs`, 2 Tests + 13
  Unit-Tests grün, keine Warnungen).
