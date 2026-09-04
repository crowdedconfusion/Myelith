# agent-layer (`myl-agent`)

> **Version:** 0.8.0
> **Datum:** 2026-08-28
> **Status:** Manifeste, Herkunftsstufe, Registratur, der
> **Session-Kontrakt** mit Durchsetzung im Ledger, der **Plan** und seit
> v0.7.0 die **Segmentkette**. 52 Tests. ⚑ **Was jetzt fehlt, ist keine
> Zusicherung mehr, sondern eine Laufzeit:** Nichts führt einen Plan
> aus.

Session-Kontrakte, Schadensbegrenzung, Dual-LLM-Trennung gegen
eingeschleuste Anweisungen, Segmentketten-Verifikation, Agentengedächtnis.
Referenzimplementierung von Whitepaper Kap. 8 (L3 Agent Layer).

## Aufgabe

Macht aus dem Inferenznetz ein handlungsfähiges System, ohne die
Verifikationsgarantien aus Kap. 6 stillschweigend zu überdehnen:
Werkzeugergebnisse werden als attestierte, nicht verifizierte Eingabe
behandelt (Kap. 8.1); Budget, Empfängerliste und Zeitfenster stehen im
Session-Kontrakt außerhalb des Modellkontexts und werden vom Konsens
durchgesetzt, nicht vom Agenten selbst (Kap. 8.2); architektonische Trennung
(Dual-LLM-Muster) begrenzt den Schaden eingeschleuster Anweisungen auf das
gesetzte Budget (Kap. 8.3).

## Abhängigkeiten

COMPUTE_PIPELINE (jeder Agentenschritt ist ein Inferenz-Segment), CONSENSUS
(Session-Kontrakt-Durchsetzung ist ein Ledger-Zustandsübergang),
VERIFICATION (Kopplung von Transaktionshöhe und bestätigter Auslieferung,
Kap. 8.2).

## Struktur

- `src/manifest.rs` — was ein Skill und was ein Werkzeug ist, mit der
  **Herkunftsstufe**, an der hängt, ob ein Segment nachrechenbar ist.
- `src/registratur.rs` — was verfügbar ist, und wie viel ein Segment
  daraus wert ist.
- `src/beobachtung.rs` — was ein Gateway bezeugen kann, und was nicht.
- `src/sitzung.rs` — die Naht zwischen Kontrakt und Bezeugung: wie viele
  Zeugen dieser Betrag verlangt, und ob ein externes Ergebnis damit
  benutzbar ist.
- `src/plan.rs` — was ein Agent tun wird, festgelegt bevor er anfängt.
- `src/kette.rs` — dass er es auch so getan hat, und wann er aufhört.

## Changelog

### v0.8.0 – 2026-09-04 (das lokale Harness bekommt seinen Ort und seine Grenze)

Neue Kiste `myl-local-agent` in `AGENT_LAYER/local-agent/`, **neben
`myl-agent` und nicht darin**: eigene Fassung, eigener Lebenslauf.

⚑ **Sie trägt heute ihre Grenze und sonst nichts, und das ist Absicht.**
Das Harness darf die Kette nicht kennen; seine einzige Berührung mit ihr
ist ein Token, das ihm gereicht wurde. Es unterschreibt keine
Transaktion, liest keinen Kettenzustand und hält keinen Schlüssel.
Abgebucht wird vom Knoten, nachdem gerechnet wurde.

`tests/isolation.rs` hält die eigene `Cargo.toml` gegen eine Liste
verbotener Kisten (`myl-consensus`, `myl-ledger`, `myl-node`), mit einer
Gegenprobe darauf, dass die Suche einen eingebauten Verstoss auch
findet. **Eine Grenze, die nur im Text steht, überlebt den ersten
eiligen Nachmittag nicht.**

⚑ **Und die Schichtung gehört genau gefasst:** `myl-agent` läuft nicht
im Block. Durchgesetzt wird eine Ebene tiefer, in `myl_types::sitzung`
und `myl_ledger::transitions`; `myl-agent` ist die deterministische
Schicht darüber, deren Erzeugnisse **verankerbar** sind, und
`myl-local-agent` ist die isolierte Schicht daneben.

### v0.7.0 – 2026-08-29 (die Kette, und ein Loch in der Zusage von gestern)

**Punkte 4.1 und 4.2, Whitepaper Kap. 8.4.** Jeder Agentenschritt ist
ein eigenes Segment; damit auch der *Ablauf* nachprüfbar bleibt, hängt
jeder Schritt am Ausgabe-Commitment seines Vorgängers.

### ⚑ Zuerst ein Loch in der Zusage von gestern

Punkt 3.1 sagt: „Die Folge der Aufrufe stand fest, bevor der erste
geschah." **Das lässt sich von außen nur glauben, solange niemand
belegen kann, wann sie feststand.** Wer den Plan hinterher passend zu
dem baut, was geschehen ist, erfüllt jede Prüfung an ihm.

Der Plan hat deshalb jetzt eine **Adresse**, und die geht in den Anker
der Kette ein. Ein nachträglich geänderter Plan bricht damit die ganze
Kette. Aufgefallen ist das beim Bauen von 4.1, nicht beim Bauen von 3.1.

### ⚑ Eine Kette allein belegt zu wenig

Eine Folge von Gliedern, die sauber aneinanderhängen, ist **in sich**
stimmig, und das heißt nicht, dass sie richtig ist. Wer einen Schritt
auslässt und danach neu knüpft, bekommt wieder eine in sich stimmige
Kette, nur eine kürzere.

**Geprüft wird deshalb gegen den Plan.** Er sagt, wie viele Schritte es
sind und welches Werkzeug an welcher Stelle läuft; beides geht in jeden
Faltungsschritt ein. Damit fallen die drei Fälle aus Kap. 8.4
auseinander: **ausgelassen** und **eingefügt** an der Länge,
**vertauscht** am Wert, weil die Stelle mitgehasht wird.

**Der Anker bindet an Session und Plan**, beides und nicht eines von
beidem: Ohne die Session ließe sich eine Kette unter einen anderen
Kontrakt mit anderen Grenzen legen.

### Wo eine Kette bricht, sagt dieses Modul nicht

Der Wert stimmt oder nicht; welcher Schritt schuld ist, findet die
Bisektion in VERIFICATION, die es dafür schon gibt. **Eine zweite Suche
daneben wäre eine zweite Quelle für dieselbe Aussage.** Wer zwei Ketten
hat, kommt billiger davon: `erster_unterschied` nennt die Stelle sofort,
und das ist der Redundanzvergleich aus Kap. 6.4 eine Ebene höher.

### Die Abbruchbedingungen, und eine, die keine ist

Der Kontrakt trägt jetzt eine **Höchstzahl der Schritte**. ⚑ **Sie ist
nicht dasselbe wie das Budget, obwohl beides begrenzt:** Das Budget
begrenzt, was ausgegeben wird, die Schrittzahl, wie lange gearbeitet
wird. Ein Agent, der in einer Schleife nachschlägt, ohne je zu zahlen,
verbraucht kein Budget und liefe endlos.

Geprüft wird **vor dem ersten Schritt**, nicht nach dem letzten: Ein zu
langer Plan liefe sonst zur Hälfte und bräche dann ab; bezahlt wäre die
Hälfte, erreicht nichts.

⚑ **„Zielerreichung" steht in Kap. 8.4 und ist nicht maschinell
entscheidbar.** Ob ein Auftrag erfüllt ist, beurteilt ein Mensch; eine
Maschine sieht nur, dass der *Plan* zu Ende ist. Genau das heißt
`Ende::Vollstaendig`, und es heißt nicht „gelungen". Die Unterscheidung
hier zu verwischen hieße, dem Konsens eine Beurteilung zuzuschreiben,
die er nicht leisten kann.

**Acht neue Tests**, zusammen 52.

### v0.6.0 – 2026-08-29 (die Trennung liegt im Typ, nicht in der Ausführungsumgebung)

**Punkt 3.1, Whitepaper Kap. 8.3.** Das Kapitel verlangt wörtlich, dass
abgerufene Daten den Kontrollfluss nicht beeinflussen können,
**strukturell statt filterbasiert**.

### ⚑ Die Frage bot zwei Antworten an, und beide waren am Thema vorbei

Zur Wahl standen zwei Modellinstanzen auf getrennten Pods oder ein
erzwungener Kontextwechsel in einer Session. **Beide beschreiben, wo das
Modell läuft, und die Anforderung handelt nicht davon.** Sie ist eine
Aussage über Datenfluss. Zwei Pods geben davon nichts, wenn der planende
Teil den abgerufenen Text als Zeichenkette zugestellt bekommt; und ein
„erzwungener Kontextwechsel" ist eine Zusage über den Prompt-Bau, also
genau die filterbasierte Absicherung, die das Kapitel ausschließt.

### Der Plan ist eine Datenstruktur, kein Text

Ein Plan nennt Schritte, jeder Schritt ein Werkzeug und seine Argumente.
Ein Argument ist ein Wert aus dem Auftrag oder die Ausgabe eines
**früheren** Schritts.

⚑ **Die Zusicherung folgt aus dem, was der Typ nicht kann:** keine
Verzweigung, keine Schleife, keine Werkzeugwahl zur Laufzeit. Damit
steht die Folge der Werkzeugaufrufe fest, **bevor der erste Aufruf
geschieht**. Zwei Läufe, die sich nur in abgerufenen Inhalten
unterscheiden, rufen dieselben Werkzeuge in derselben Reihenfolge auf.

**Das ist keine Prüfung, sondern eine Abwesenheit.** Eine Prüfung kann
man vergessen; ein Konstrukt, das es nicht gibt, kann man nicht
benutzen. Deshalb nimmt `werkzeugfolge` auch keine Eingaben entgegen:
Könnte sie es, wäre die Zusage eine Behauptung über ihren Rumpf statt
über ihre Signatur.

⚑ **Und der Test kann das nur vorführen, nicht belegen.** Getragen wird
es davon, dass die Schrittliste privat ist und keine Schnittstelle zum
Erweitern existiert; das Fehlen einer Schnittstelle lässt sich zur
Laufzeit nicht prüfen. Wer die Datei liest, prüft mit, dass unterhalb
kein `push` hinzugekommen ist.

### ⚑ Eine Regel, die zu streng gewesen wäre

Naheliegend war: Ein abgerufener Wert nie an eine sicherheitsrelevante
Stelle, also weder Empfänger noch Betrag. **Das hätte die Aufgabe
verboten und nicht den Angriff.** „Finde den günstigsten Flug und buche
ihn" liefert Flugnummer und Preis aus einem Werkzeug; beide sind
Argumente einer Buchung.

Ein getrübter Wert darf deshalb in ein Werkzeugargument fließen. Er darf
nicht bestimmen, **welches** Werkzeug läuft, **ob** es läuft und **wie
oft**. Empfänger und Betrag deckt der Session-Kontrakt, und der wird vom
Konsens durchgesetzt. **Die Trübung sperrt den Kontrollfluss, der
Kontrakt den Schaden**; sie zu vermengen machte den Agenten unbrauchbar,
ohne die Zusage zu stärken.

### Trübung ist keine neue Achse

Sie folgt aus dem Werkzeugmanifest, das seit v0.1.0 dasteht: extern oder
nicht nachrechenbar heißt getrübt, und Trübung erbt sich über Argumente
weiter. Ein unbekanntes Werkzeug gilt als getrübt, denn **wer nicht
weiß, was es tut, weiß auch nicht, dass es rechnet.** Ein zweites
Etikett wäre eine zweite Quelle für dieselbe Aussage gewesen.

### Der Preis, und er steht hier statt in einer Fußnote

Ein gerader Plan kann **nicht auf ein Ergebnis reagieren**. „Wenn der
Preis unter 500 liegt, buche" ist nicht ausdrückbar. Für die Sicherheit
ist das kein Verlust, denn die Obergrenze steht im Kontrakt; für die
Ergebnisqualität ist es einer. Der Ausweg wäre, den Planer erneut laufen
zu lassen, und dabei liefe der abgerufene Inhalt in seinen Kontext
zurück. **Das ist eine eigene Entscheidung und keine Lücke, die man
nebenbei schließt.**

**Acht neue Tests**, zusammen 44.

### v0.5.0 – 2026-08-28 (der Kontrakt, und die Zahl aus Design 1 bekommt einen Ort)

**Der Session-Kontrakt selbst liegt in `myl-types`**, weil er in L1
durchgesetzt und in L3 benutzt wird und kein Crate dieses Repositoriums
nach oben zeigt. Hier liegt die Agentenseite.

### ⚑ Die Zeugenleiter

Design-Entscheidung 1 ließ eine Zahl offen: **wie viele unabhängige
Gateways ein externes Ergebnis bezeugt haben müssen.** Die Antwort war,
dass sie nicht ins Protokoll gehört, sondern in den Kontrakt, gekoppelt
an den Betrag. Der Kontrakt trägt sie jetzt als **Leiter**: je
Betragsstufe eine Zeugenzahl.

**Dieselbe Beobachtung kann für einen kleinen Betrag genügen und für
einen großen nicht**, und genau das ist der Sinn der Kopplung. Ein Test
zeigt es an zwei Aufrufen mit denselben Attestierungen.

**Eine Sprosse, die bei höherem Betrag weniger Zeugen verlangt, wird
abgelehnt.** ⚑ Das ist die Form eines Versehens und zugleich die, die
ein Angreifer sich wünschte: Je mehr auf dem Spiel steht, desto weniger
Belege.

**Und der Agent kann die Zahl nicht senken.** Eine mildere Leiter ist
ein anderer Kontrakt, ein anderer Kontrakt hat eine andere Adresse, und
die Session läuft unter der alten.

**Die Zeitspanne wird durchgereicht, nicht bewertet.** Einigkeit über
200 Millisekunden bedeutet etwas anderes als über 30 Sekunden, und wer
das beurteilt, ist der Mensch oder der Agent. Eine Frist hier
hineinzuschreiben hieße, die Entscheidung zu treffen, die Design 1
ausdrücklich nicht getroffen hat.

### ⚑ Ein Fund beim Verdrahten: `beobachte` meldete Uneinigkeit über die leere Menge

Bis heute lieferte `beobachte` für eine **leere** Attestierungsliste
`Uneinig` mit null Varianten, also die Meldung „die Zeugen sahen
Verschiedenes" über Zeugen, die es nicht gab. Wer sie las, erfuhr das
Gegenteil von dem, was der Fall war.

**Aus nichts folgt weder Einigkeit noch Uneinigkeit.** Die leere Liste
ist jetzt ein Fehler (`KeineAussagen`). Wer ohne Bezeugung auskommen
darf, fragt gar nicht erst an und bekommt `Verwendbar::OhneBezeugung`:
**erlaubt, aber ausdrücklich keine Zusicherung** — es wurde nichts
geprüft, weil nichts verlangt war.

**Sechs neue Tests**, zusammen 36.

### v0.4.0 – 2026-08-28 (Phase 1 ist zu, und eine Prüfung gab es geschenkt)

**Punkt 1.6: deterministische Werkzeuge.** Ein Aufruf geht als Tripel in
die Spur: Werkzeugadresse, Eingabe-Hash, Ausgabe-Hash. Nur Hashes, denn
die Spur trägt, was zum Nachrechnen nötig ist, und nicht den Inhalt.

### ⚑ Die Prüfung, die beim Bauen abfiel

Ein deterministisches Werkzeug sagt zu: **gleiche Eingabe, gleiche
Ausgabe**. Kommt dieselbe Paarung in einer Spur zweimal mit
**verschiedenen** Ausgaben vor, ist das ein Widerspruch, den man **ohne
jede Ausführung** sieht. Entweder ist das Werkzeug nicht
deterministisch, oder jemand hat eine Ausgabe erfunden.

**Das ist derselbe Gedanke wie der Redundanzvergleich eine Ebene
höher:** zwei Aussagen über dieselbe Rechnung gegeneinander halten,
statt die Rechnung zu wiederholen. Nur kostet er hier nichts, weil die
Aussagen ohnehin in der Spur stehen.

⚑ **Und sie ist gelegentlich, nicht vollständig.** Sie kann nur
zuschlagen, wenn dieselbe Paarung wirklich zweimal vorkommt; bei einem
einzelnen Aufruf schweigt sie, und das heißt **nicht**, dass er stimmt.
Ein Test hält genau das fest, damit niemand sie für einen Beweis hält:
Eine glatt erfundene Ausgabe fällt nicht auf, solange sie allein steht.

**Ein externes Werkzeug darf sich widersprechen**, und das ist der Sinn
von „extern". Ohne diese Ausnahme meldete die Prüfung jeden
Wetterbericht als Defekt.

### Die Schwere ist geordnet, und die Reihenfolge sagt etwas

`Widerspruch` steht über `Unbekannt`. ⚑ **Ein Widerspruch ist ein Beleg
für einen Defekt, „unbekannt" nur das Fehlen von Wissen.** Beides macht
ein Segment unprüfbar, aber nur auf eines davon kann jemand reagieren.

**Damit ist Phase 1 abgeschlossen:** Manifeste, Herkunftsstufe,
Registratur mit Stufenrechnung, Gateway-Attestierung, Mehrfachabruf und
deterministische Werkzeuge. **Sieben neue Tests**, zusammen 30.

### v0.3.0 – 2026-08-28 (Gateways bezeugen, sie entscheiden nicht)

**Design-Entscheidung 1 ist gefallen, und sie fiel gegen die
naheliegende Antwort.** Die Frage lautete „wie viele Gateways müssen
übereinstimmen". Sie unterstellt, Übereinstimmung sei ein
Wahrheitsbeweis.

⚑ **Ist sie nicht, und zwar aus zwei Richtungen.** Uneinigkeit heißt
nicht Bosheit: Zwei ehrliche Gateways bekommen verschiedene Antworten
bei Geo-Routing, A/B-Tests oder schlicht, weil sich die Welt zwischen
zwei Abrufen geändert hat. Und Einigkeit heißt nicht Wahrheit: Lügt der
Ursprungsserver, lügen alle Gateways bytegleich. **Die Fälle „böse",
„veraltet" und „anders geroutet" sind aus den Attestierungen allein
nicht unterscheidbar.**

### Aufzeichnen statt abstimmen

`beobachte` löst **nichts** auf. Drei Zustände, und `Uneinig` behält
**alle** Varianten mit ihren Zeugen. Wer daraus etwas macht, ist der
Agent oder der Mensch, nicht das Protokoll. Kein „nimm die häufigste":
Bei volatilen Daten ist die Mehrheit bedeutungslos, und bei zwei gegen
eins kann die Minderheit die ehrliche sein.

Derselbe Grundsatz wie bei `Segmentstufe::Unbekannt` und bei
`Befund::KeinNachweis`: **Ungewissheit wird benannt, nicht versteckt.**

### ⚑ Die Zeitspanne ist das Maß dafür, was Einigkeit wert ist

Das Whitepaper sagt, der Mehrfachabruf „versagt bei sich laufend
ändernden Daten". Das steht jetzt **im Ergebnis** statt in einer
Fußnote: Drei übereinstimmende Attestierungen innerhalb von 200
Millisekunden bedeuten etwas anderes als dieselben über 30 Sekunden.

⚑ **Und die Spanne ist ein Hinweis, kein Beweis.** Die Zeitstempel
kommen von den Gateways selbst; wer lügt, lässt sie besser aussehen. Sie
hilft gegen **Trägheit**, nicht gegen **Absicht**, und wer sie liest,
soll das wissen.

### Zwei Bindungen, die leicht gefehlt hätten

⚑ **Die Anfrage steht in der Unterschrift.** Ohne sie bezeugte eine
Attestierung nur „ich habe irgendwann diese Bytes gesehen", und dieselbe
Unterschrift ließe sich für eine **andere** Frage vorlegen.

⚑ **Derselbe Zeuge zweimal ist ein Zeuge.** Wer eine Aussage doppelt
vorlegt, bläht die Zeugenzahl auf und unterläuft genau die Zahl, die der
Session-Kontrakt verlangt hat. Wird abgewiesen.

**Und die Prüfung lässt sich nicht vergessen:** In `beobachte` kommt
man nur mit `GepruefteAttestierung`, und die gibt es nur aus
`Attestierung::pruefe`. Eine ungeprüfte sieht aus wie eine geprüfte; der
Typ erinnert daran.

### Wie viele Zeugen es braucht, steht nirgends

**Es gibt keine Konstante dafür, und das ist Absicht.** Der Aufrufer
verlangt eine Zahl, und der Session-Kontrakt kann ein Minimum erzwingen,
gekoppelt an den Betrag (Kap. 8.2, Punkt 2.3). Wie viele Zeugen es
braucht, skaliert damit mit dem, was auf dem Spiel steht, statt mit einer
Zahl, die jemand einmal geraten hat.

**Was das ausdrücklich nicht leistet:** Es macht externe Daten nicht
verifizierbar. Ein Segment mit externem Eingang bleibt `Bezeugt`, gleich
wie viele Gateways unterschrieben haben. Zeugenzahl verbessert die
**Glaubwürdigkeit**, nicht die **Verifizierbarkeit**.

**Acht Tests.**

### v0.2.0 – 2026-08-28 (die Registratur, und der schwächste Eingang)

**Ein Agentenschritt benutzt selten einen einzelnen Skill.** Er zieht
Wissen aus zwei Quellen, ruft ein Werkzeug, und was herauskommt, ist
**ein** Segment mit **einer** Verifikationsstufe.

⚑ **Diese Stufe ist das Minimum über alles Benutzte.** Ein verankerter
Skill neben einem lokalen ergibt ein Segment, das niemand nachrechnen
kann; der verankerte hilft nichts. Das ist unbequem und richtig: Wer
eine Kette prüft, prüft das schwächste Glied und nicht den Durchschnitt.

⚑ **Und „unbekannt" ist nicht dasselbe wie „nur bezeugt".** Ein Segment,
das eine Adresse nennt, die niemand kennt, ist nicht schwach belegt,
sondern **gar nicht** belegt: Es steht nicht einmal fest, was benutzt
wurde. Das als bezeugt zu führen hieße zu behaupten, man kenne den
Eingang. Es ist deshalb ein eigener Zustand, und der schlechteste.

**Die Adresse wird gerechnet, nicht geglaubt.** Die Registratur legt
unter der Adresse ab, die sie selbst aus dem Manifest rechnet. Nähme sie
eine mitgelieferte, ließe sich ein lokaler Skill unter der Adresse eines
verankerten eintragen, und die ganze Stufenrechnung wäre wertlos.

⚑ **Die Liste der Schuldigen ist sortiert, und das ist kein Stil.** Sie
wandert in die Spur, und zwei ehrliche Knoten müssen dieselbe schreiben.
Hinge sie an der Aufrufreihenfolge, meldete der Redundanzvergleich zwei
ehrliche Pods als abweichend — dieselbe Klasse wie das für den
Vorwärtspfad verbotene Token-Dropping.

**Sieben Tests**, darunter: Ein lokaler Skill zieht zwei verankerte
herunter; unbekannt schlägt bezeugt; gleicher Inhalt mit anderer
Herkunft liegt nebeneinander statt sich zu überschreiben; und ein
Manifest ohne Lizenz kommt nicht herein, sonst wäre ETHICS G7 eine
Absichtserklärung statt einer Prüfung.

### v0.1.0 – 2026-08-28 (die erste Zeile, und sie ist ein Format)

⚑ **Die Blockade war gefallen, ohne dass es jemandem auffiel.** Diese
Komponente galt seit dem 10. August als blockiert. Nachgesehen sind
alle drei Abhängigkeiten erfüllt: Pods rechnen bitgleich mit
Ausfallsicherung, der Konsens hat alle vier Phasen, und die Kopplung
aus Kap. 8.2 steht als `should_deliver_confirmed`. **Was aufhält, sind
Entscheidungen und kein Code.**

**Zuerst entsteht kein Agent, sondern ein Format.** Jeder weitere Teil
setzt voraus, dass feststeht, was ein Skill und was ein Werkzeug ist,
und das stand nirgends.

### ⚑ Die Herkunftsstufe, und warum sie am Manifest hängt

Kap. 8.1 unterscheidet **deterministische** Werkzeuge, die vollständig
verifiziert werden, von **externen**, deren Ergebnis attestiert wird.
Dort ist das eine Eigenschaft des Werkzeugs. Hier ist es eine
Eigenschaft des **Manifests** und wandert in die Spur:

| Herkunft | Wer hat den Inhalt | Was ein Dritter kann |
|---|---|---|
| verankert | alle, Hash im Konsens | vollständig nachrechnen |
| Bibliothek | alle, kuratiert, Hash verankert | dito |
| **lokal** | nur der Nutzer | **nichts** |

**Ohne diese Stufe sieht ein Prüfer einem Segment nicht an, ob er es
nachrechnen kann oder nur glauben muss.** Beides ist zulässig; beides
gleich aussehen zu lassen ist es nicht.

⚑ **Ein lokaler Skill ist der Preis einer Freiheit, und er gehört dem
Nutzer gesagt.** Er lässt sich frei einpflegen, und genau deshalb kann
niemand sonst prüfen, was er getan hat. Ein Hash belegt, **welcher**
Skill benutzt wurde, wenn man ihn schon hat; er erlaubt keinem Dritten,
das Ergebnis nachzurechnen.

**Die Stufe geht in die Adresse ein.** Zwei Skills mit gleichem Inhalt
und verschiedener Herkunft sind **verschiedene Gegenstände**, denn ein
Prüfer kann mit ihnen Verschiedenes anfangen. Stünde die Stufe daneben
statt darin, ließe sich ein lokaler Skill unter der Adresse eines
verankerten ausgeben.

⚑ **Und zwei Felder sind nicht unabhängig: Ein externes Werkzeug kann
nicht verankert sein.** Verankert heißt nachrechenbar, und ein Ergebnis
aus der Außenwelt ist es nicht, gleich wo sein Hash steht. Ohne diese
Prüfung ließe sich ein externer Abruf als deterministisches Segment
ausgeben, und Kap. 8.1 verlöre seine Grenze.

**Acht Tests**, darunter alle vier Paarungen von Art und Herkunft, das
Längenpräfix gegen die Feldgrenzen-Verwechslung, und die Gegenprobe,
dass gleiche Herkunft dieselbe Adresse ergibt.
