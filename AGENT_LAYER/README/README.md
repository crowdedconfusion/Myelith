# agent-layer (`myl-agent`)

> **Version:** 0.20.0 (`myl-agent` 0.7.0, `myl-local-agent` 0.13.0)
> **Datum:** 2026-09-21
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

### v0.20.0 – 2026-09-25 (`myl-local-agent` 0.13.0: ein Lauf kann vom Menschen angehalten werden, und die Risikoklassen liegen unter COMPLIANCE)

- ⚑ **`Tuerfehler::Abgebrochen { bisher }`**: Der Notaus des Clients hält
  die Erzeugung an, und `chat` meldet das als eigenen Fehler. Die Schleife
  endet damit wie bei jedem Fehler der Tür (`Ende::Tuer`), und der Text
  bis zum Halt kommt mit, damit der Mensch ihn sieht und nichts verloren
  geht. `Display`: „vom Menschen angehalten (Notaus)".
- ⚑ **`ETHICS/` ist nach `COMPLIANCE/ethics/` gezogen**: `risiko.rs` liest
  `Risikoklassen.toml` dort (`include_str!`), Kommentare nennen den neuen
  Ort.

**Belegt:** alle Proben grün; der Weg über `Abgebrochen` ist im Client an
einem echten Modell geprüft (`myl-client`, `tests/notaus.rs`).

### v0.19.0 – 2026-09-23 (`myl-local-agent` 0.12.0: ein Patzer beendet den Lauf nicht mehr, und Schweigen heisst nicht mehr Erfolg)

⛔️ **Der Anlass war gemessen.** In einer Agentenprobe hat das grösste
Modell beide gesuchten Arbeiten gefunden, beide richtigen Seiten
gelesen und die Tatsachen sauber herausgezogen. Dann rief es
`write_file` mit dem Parameterschema eines anderen Werkzeugs auf. Die
Prüfung erkannte das und schrieb eine **gute** Absage in den Verlauf.
Der Lauf endete im selben Augenblick, mit der Meldung „fertig", und die
Absage wurde nie abgeschickt. Zehn von vierzehn Schritten lagen frei,
und die richtige Antwort stand in den Argumenten des abgelehnten
Aufrufs.

**Drei Fälle, die bisher einer waren:**

| Schritt | vorher | jetzt |
|---|---|---|
| kein Vorschlag | `Fertig` | `Fertig` |
| etwas lief | weiter | weiter |
| vorgeschlagen, nichts lief | `Fertig` | Rückmeldung, bis zu zwei weitere Runden |

⚑ **Die Kostenschranke bleibt, sie wandert nur.** Der Grund im
Quelltext war und ist richtig: Ein Modell, das immer wieder dasselbe
Verbotene vorschlägt, soll nicht die ganze Schrittzahl verbrennen. Neu
ist, dass es vorher **zwei** Gelegenheiten bekommt, den Grund zu lesen
und es besser zu machen. Gezählt wird hintereinander; ein ausgeführtes
Werkzeug setzt den Zähler zurück, denn danach ist das Modell
nachweislich wieder auf Kurs.

⛔️ **Ein unlesbarer Aufruf bekommt jetzt eine Antwort.** Vorher stand in
der Schleife `for v in roh.into_iter().flatten()`, und das warf jedes
`Err` **wortlos** weg: Ein Aufruf, der etwa an der Tokengrenze mitten im
String abbrach, hinterliess danach gar nichts. Keine Ausführung, keine
Ablehnung, keine Nachricht. 📌 **Ein Fehler, den niemand meldet, wird
nicht berichtigt.**

⛔️ **Und der neue Ausgang `Steckengeblieben` heisst nicht `Fertig`.**
Ein Lauf, der an einem Tippfehler stirbt, sah vorher genauso aus wie
einer, der seine Arbeit getan hat, bis hinunter zum Rückgabewert 0.
📌 **Schweigen sieht aus wie Erfolg**, und das ist dieselbe Fehlerklasse
wie eine Schleife, die an der kürzeren Seite abbricht, ohne ein Wort zu
sagen.

**Belegt:** vier Proben, darunter der abgelehnte Aufruf, der berichtigt
werden darf, der abgeschnittene Aufruf, der beantwortet wird, und der
dreifach wiederholte Verbotene, der steckenbleibt statt fertig zu
werden. 19 Proben der Schleife grün, Clippy ohne Befund.

### v0.18.1 – 2026-09-21 (`myl-local-agent` 0.11.1: die Vorlage liegt woanders)

Die Probe, die die Werkzeugansage gegen die echte Vorlage des Modells
hält, liest `tokenizer_config.json` aus dem Quellmodell. Das liegt seit
heute unter `MODELS/llm`. ⚑ **Die zweischichtige Prüfung bleibt, wie sie
ist:** gegen eine abgelegte Kopie, die in der CI läuft, und die Kopie
gegen die echte Vorlage, die läuft, wo das Modell liegt. Nur der Weg
dorthin ist ein anderer.

### v0.18.0 – 2026-09-17 (`myl-local-agent` 0.11.0: eine Hausregel hinter der Werkzeugansage)

⚑ **Neu ist `angebot_mit_regel` und das Feld `Lauf::hausregel`**, beides
mit `None` als Vorgabe. Die Regel steht als eigener Absatz **hinter**
der Vorlage: Kopf und Fuß der amtlichen Ansage sind zeichengleich die
Literale aus `tokenizer_config.json`, und eine Prüfung hält sie dagegen.
**Was sich dazwischenschöbe, änderte den Schliff, auf den das Modell
trainiert wurde.**

⛔️ **Sie ist nie freier Text eines einzelnen Knotens.** Im Netz rüsten
alle Knoten gleich, sonst rechnen sie Verschiedenes; was in die Ansage
geht, gehört zu den Protokollgrößen. Dieselbe Überlegung wie bei der
Werkzeugkiste, die im Netz dem Modell folgt statt einer Einstellung.

⚑ **Gebaut, weil die Frage „hilft ein strengerer Systemprompt?" eine
Messung verdient und keine Meinung.** Das Ergebnis über 63 Läufe steht
im Komponenten-README des Clients; hier nur das Kurze: **Beim 30B hebt
eine Regel die Trefferquote von 17 aus 24 auf 9 aus 9.** ⛔️ **Und beim
0,6B macht die strenge Fassung eine sonst richtige Antwort kaputt: 9 von
9 richtig ohne Regel, 1 von 9 mit ihr.** Eine Regel, die ein Modell
nicht befolgen kann, verdrängt Platz und verschlechtert den Rest.

### v0.17.0 – 2026-09-14 (der Verlauf geht mit, und der Kontext verdichtet sich am Rand)

`myl-local-agent` **0.10.0**.

⚑ **Ein Lauf trägt das bisherige Gespräch vor dem Auftrag**
(`Lauf::fahren_mit_verlauf`, Entscheidung C2 beantwortet): Schrittbudget
und Belegkette bleiben je Auftrag, der Verlauf ist Eingabe und im
Commitment des ersten Schritts gebunden.

⚑ **Läuft der Kontext voll, wird verdichtet statt abgebrochen**
(`verdichtung`): Das Modell fasst den mittleren Teil zusammen, Auftrag und
letzter Schritt bleiben wörtlich; eine einzelne zu lange Werkzeugantwort
wird in der Mitte gekürzt. `Modellweg::kontext` sagt, wie viel Kontext ein
Prompt belegt; `None` heißt unbekannt, dann verdichtet niemand von selbst.

### v0.16.1 – 2026-09-10 (die Artefakte heissen nach dem Modell, das sie sind)

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

### v0.16.0 – 2026-09-10 (`myl-local-agent` 0.9.0: die Schleife meldet, während sie läuft)

`Lauf` trägt seit heute einen `melder`, und die Schleife sagt damit, was
sie gerade tut: welcher Schritt, welches Werkzeug läuft, was es zurückgab
und was abgewiesen wurde.

⚑ **Er bekommt zu sehen und entscheidet nichts.** Diese Kiste trägt eine
Vollmacht; ein Haken, der den Lauf beeinflussen könnte, wäre eine zweite
Quelle für Erlaubnisse neben `Erlaubnis` und `Betriebsart`. Der Melder
gibt nichts zurück und wird an Stellen gerufen, an denen die Entscheidung
schon gefallen ist. `ein_melder_aendert_den_lauf_nicht` fährt denselben
Auftrag zweimal und vergleicht Ende und Nachrichtenverlauf Wort für Wort.

⚑ **Gemeldet wird vor **und** nach der Ausführung.** Ein Werkzeug, das
ein Verzeichnis durchsucht, läuft merklich lange; wer nur das Ergebnis
meldet, zeigt in dieser Zeit ein Fenster, das stillsteht.

⚑ **Eine Ablehnung wird ebenfalls gemeldet.** Ein Agent, dem ein Werkzeug
verwehrt wurde, sieht für den Nutzer aus wie einer, der nichts tut; der
Grund stand sonst nur im Sitzungsstrom.

### v0.15.0 – 2026-09-09 (`myl-local-agent` 0.8.1: die Ansage wird die Vorlage und nicht ihre Paraphrase)

📌 **Fund 221: Die Werkzeugansage war eine deutsche Umschreibung**, und
ihr fehlte gerade das Aufrufbeispiel, das die Vorlage des Modells als
Literal zeigt. Dazu standen die JSON-Schlüssel alphabetisch, weil
`serde_json::Map` ohne `preserve_order` ein `BTreeMap` ist.

⚑ **Wo ein Modell eine Vorlage mitbringt, gilt sie zeichengenau.**
`Ansageform::Amtlich` ist seither die Vorgabe und wortgleich zur
Vorlage; `Ansageform::Deutsch` bleibt als Vergleichsschalter stehen.
Die Vorgabe ruht dabei auf der Vorlage und nicht auf einer Messung, und
das steht ausdrücklich am Typ, damit niemand sie für belegt hält.

⚑ **Gebunden in zwei Schichten**, denn eine Kopie kann altern:
`tests/werkzeugansage.rs` prüft die erzeugte Ansage gegen eine im
Repositorium abgelegte Kopie (läuft in der CI), und die Kopie gegen die
echte Vorlage des Modells (läuft, wo das Modell liegt).

📌 **Nachgetragen am 2026-09-10.** Das Manifest von `myl-local-agent`
trug 0.8.1 seit dem 2026-09-09, dieser Changelog stand auf 0.14.0 mit
`myl-local-agent` 0.7.0 in der Kopfzeile.

### v0.14.0 – 2026-09-05 (Punkt 5.7: ein Steckplatz, der tun darf und nicht erlauben, und die Schleife, die beides zusammenhält)

### ⚑ Die Erlaubnis prüfte gegen ein Angebot, das niemand einlösen konnte

Bis gestern kannte diese Kiste `Werkzeug`, `Vorschlag`, `Erlaubnis`,
`Betriebsart` und `Risikoklassen`, und keines davon führte je ein
Werkzeug aus. Die Prüfkette war vollständig und lief ins Leere.

`src/ausfuehrung.rs` schliesst das mit dem Merkmal
[`Werkzeugausfuehrung`](../local-agent/src/ausfuehrung.rs) und dem
`Werkzeugkasten`, der Angebot und Ausführung aneinander bindet.

### ⚑ Ein Steckplatz darf TUN, nie ERLAUBEN

Die Anregung war eine Steckplatz-Architektur nach dem Vorbild von
OpenClaw und DeepSeek Harness. Sie ist gebaut, mit einer Grenze, die
dort nicht so scharf gezogen ist: **`ausfuehren` bekommt keine
`Erlaubnis`, keinen Kontrakt, keinen Strom und keine Registratur.** Ein
Steckplatz sieht seine Argumente und gibt Text zurück.

Damit kann er nicht entscheiden, ob er laufen darf. Das hat die
Schleife vier Stufen früher entschieden, und ein neu eingehängter
Steckplatz kann diese Entscheidung nicht aufweichen, weil er sie nicht
sieht. Die Methode heisst `ausfuehren_ungeprueft`, damit die Abwesenheit
der Prüfung an der Aufrufstelle im Text steht.

### ⚑ `Send + Sync` ist eine Zusicherung, keine Formalie

Ein Steckplatz, der nicht zwischen Fäden wandern darf, hält
fadenlokalen Zustand, und dann rechnet er je nach Faden anders. Das ist
genau die Sorte Nichtdeterminismus, die dieses Projekt überall sonst
ausschliesst. Hier fällt sie beim Übersetzen auf statt im Betrieb.

### Die Reihenfolge in `Lauf::fahren`, und warum sie festliegt

`src/schleife.rs` führt acht Stufen in dieser Folge zusammen:
Schrittzahl, Modell, Vorschläge, Erlaubnis, Argumentform, Betriebsart,
Ausführen, Strom.

⚑ **Die Betriebsart steht an Stufe 6**, also nachdem das Werkzeug
bekannt ist und bevor es läuft. Früher ginge nicht, weil die Stufe eines
Werkzeugs erst feststeht, wenn man weiss, welches gemeint ist; später
wäre zu spät, weil dann schon etwas geschehen wäre.

⚑ **Ein Schritt, in dem alles abgelehnt wurde, beendet den Lauf.** Sonst
könnte ein Modell, das beharrlich Verbotenes vorschlägt, das Schrittbudget
des Sitzungskontrakts aufbrauchen, ohne dass je etwas geschieht.

### ⚑ Der Beleg an der echten Tür, und ein Fund aus dem eigenen Testaufbau

`TESTCLIENT/myl-testclient/tests/harness_bis_modell.rs` fährt die
Schleife jetzt gegen die **echte** Tür und die echte Shard-Pipeline: Die
Vollmacht wird geprüft, der Knoten versiegelt, vier Shards rechnen. Der
Strom trägt danach echte Segmentkennungen, also eine Kette.

⚑ **Was der Test NICHT prüft, und warum:** dass das Modell ein Werkzeug
vorschlägt. Ein 0,5B-Modell tut das unzuverlässig. Geprüft wird die
Schleife und der Beleg, nicht die Bereitwilligkeit des Modells; die
Entscheidungslogik liegt in `tests/schleife.rs` gegen einen Stummel.

⚑ **Der erste Entwurf des Tests blieb stehen.** Er bediente die Tür
zweimal, weil der Kontrakt zwei Schritte erlaubt. Das Modell schlug kein
Werkzeug vor, die Schleife endete nach einem Schritt, und die Tür
wartete auf einen zweiten Aufruf, der nie kam: null Prozent CPU nach 32
Sekunden. Wie viele Aufrufe kommen, weiss nur die Schleife. Der Test
fragt jetzt nicht mehr danach, sondern hört auf, wenn sie fertig ist.

### v0.13.0 – 2026-09-05 (Punkt 5.5: eine Warnung, die niemand las, und ein Bericht, der etwas sagt)

**Phase 5 ist damit vollständig.**

### ⚑ `ETHICS/Risikoklassen.toml` hatte null Leser

Die Datei sagt in ihrem eigenen Kopf: „CLIENT und AGENT_LAYER binden
diese Datei ein, statt den Text abzuschreiben." **Bis heute tat das
niemand.** Sie war eine Quelle ohne Leser, und die Warnung erreichte
damit genau niemanden.

Das ist bitter, weil die Begründung im selben Kopf steht: „Eine Warnung,
die an drei Stellen steht, steht irgendwann in drei Fassungen da, und
die mildeste wird die gelesene." **Der Entwurf war richtig; es fehlte
der Aufrufer.** Die fünfte Kiste dieser Art an einem Tag.

`myl_local_agent::risiko` bettet sie über `include_str!` ein: Damit kann
sie zur Laufzeit **weder fehlen noch bearbeitet werden**, und wer sie
milder haben will, muss das Programm neu übersetzen.

⚑ **Klasse C wird abgelehnt, nicht gewarnt**, und die Begründung kommt
**aus der Quelle**. Wer sie neu formulierte, hätte die zweite Fassung
geschaffen, vor der die Datei warnt.

⚑ **Und eine unbekannte Klasse ist keine milde Klasse.** Dieselbe
Überlegung wie bei `Segmentstufe::Unbekannt` aus 5.4: In etwas, das
niemand kennt, lässt sich nicht einwilligen.

### ⚑ Der Bericht: Festhalten ist nicht Zeigen

Die Herkunftskennzeichnung ist nach Kap. 8.1 eine **sichtbare**
Anforderung. Ein Strom, der die Stufe je Schritt trägt und sie niemandem
zeigt, erfüllt sie nicht. `Sitzungsstrom::bericht` nennt Betriebsart,
abgelehnte Vorschläge mit Stelle, und die nicht nachrechenbaren
Schritte.

⚑ **Ein sauberer Lauf sagt das ausdrücklich**, statt zu schweigen:
„Keine Ablehnungen" ist eine Aussage, eine leere Zeile ist keine, und
der Leser könnte sie für ein fehlendes Protokoll halten. Ein Test
verlangt, dass ein sauberer Bericht **keine Flagge** trägt.

### v0.12.0 – 2026-09-05 (Punkt 5.4: die Betriebsart, und wo die Wahl des Nutzers aufhört)

`myl_local_agent::betrieb`. Der Nutzer wählt, ob er Schritte zulässt,
die **niemand nachrechnen kann**: `NurVerankert` oder `Alles`. Die Stufe
kommt aus `myl_agent::Registratur::stufe` und ist das **Minimum über
alles Benutzte**, also ergibt ein verankerter Skill neben einem lokalen
ein Segment, das niemand nachrechnen kann.

**Die Wahl gehört dem Nutzer und nicht dem Programm.** Ein lokaler Skill
ist zulässig und bequem; sein Preis ist, dass ein Dritter das Ergebnis
nur glauben kann.

### ⚑ Aber „unbekannt" ist keine Wahl, sondern ein Defekt

`Segmentstufe` kennt **drei** Zustände, und der dritte ist keine
schwächere Form des zweiten:

| Stufe | Was der Nutzer in Kauf nähme |
|---|---|
| `Nachrechenbar` | nichts |
| `Bezeugt` | „ich kann das nicht nachrechnen" |
| `Unbekannt` | **„ich weiss nicht, was gelaufen ist"** |

⚑ **In die dritte Zeile lässt sich nicht einwilligen.** Wer nicht weiss,
welcher Skill benutzt wurde, weiss auch nicht, wozu er ja sagt. Eine
Einwilligung ohne Gegenstand ist keine, und deshalb lehnen **beide**
Betriebsarten sie ab. `Alles` heisst nicht alles.

### ⚑ Gefragt wird vor dem Schritt, nicht danach

Ein Segment, das niemand nachrechnen kann, hinterher als solches
auszuweisen, ist zu spät: Der Nutzer hat dann schon bezahlt, und die
Antwort steht schon in seinem Kontext.

⚑ **Und die Betriebsart steht im Sitzungsstrom.** Sonst liesse sich
später nicht unterscheiden, ob alle Schritte nachrechenbar waren, **weil
die Betriebsart es erzwang** oder weil es sich zufällig so ergab. Zwei
verschiedene Aussagen, und nur die erste ist eine Zusage. Der Strom
nennt dazu, **wo** es kippte: die Stufe hängt am Schritt, nicht an der
Sitzung, denn die Stelle ist das, was ein Prüfer sucht.

**Die Vorgabe ist `NurVerankert`.** Wer nichts sagt, bekommt das Engere;
eine Vorgabe, die mehr zulässt als nötig, ist eine Entscheidung, die
niemand getroffen hat.

### v0.11.0 – 2026-09-05 (Punkt 5.3: der Sitzungsstrom, und eine Ablehnung gehört hinein)

`myl_local_agent::strom`. Ein Strom aus Anfrage-, Antwort- und
Ergebnis-Commitments, den Vorschlägen samt Entscheidung, und der
Segmentkennung je Schritt. Daraus fallen die Kettenglieder nach
Kap. 8.4 ab, und der Kettenwert rechnet sich nach.

⚑ **Ein Beleg und kein Protokoll.** Ein Protokoll schreibt man für die
Fehlersuche und kann es weglassen; hier gilt der Satz umgekehrt: Was der
Nutzer später prüfen will, **muss beim Laufen entstanden sein**.
Nachträglich liesse es sich nicht herstellen, denn dann bezeugte es sich
selbst.

### ⚑ Der Kern: eine abgelehnte Anfrage steht mit drin

Ein Strom, der nur zeigt, **was ausgeführt wurde**, verschweigt das
Interessanteste. Wer ihn liest, sieht einen ordentlichen Lauf und kann
nicht unterscheiden, ob das Modell brav geblieben ist oder dreimal
versucht hat zu überweisen und dreimal abgewiesen wurde. **Das ist
derselbe Lauf und ein völlig anderer Befund.**

⚑ **Ein Angriffsversuch ist ein Ereignis, kein Nichtereignis**, und er
ist das früheste Zeichen, das es überhaupt gibt: Wer ihn wegwirft,
erfährt von einem Angriff erst, wenn einer gelingt. `abgelehnte()` ist
die Zahl, die ein Mensch zuerst sehen will.

### ⚑ Und `kette.rs` hat seinen ersten Aufrufer

`myl_agent::kette` stand seit dem 2026-08-29 mit **null Aufrufern** da,
wie heute Morgen schon `Expertenwacht` und zwei Nachbarn im MoE-Pfad.
Der Strom ruft sie.

📌 **Dabei fiel eine Lücke im eigenen 5.1 auf.** Der `Tuerklient` las
`id` („myl-42", eine Anzeigekennung) und warf `myelith_segment` weg,
also genau die 32 Bytes, an denen die Kette hängt. Ein Sitzungsstrom,
der sich darauf beruft, hätte sie nicht gehabt. ⚑ **Und wo sie fehlt,
sagt der Strom das:** Ein Schritt ohne Segmentkennung trägt **kein**
Kettenglied, und der Versuch, eine Kette zu bilden, scheitert mit
Begründung. Wer solche Schritte überspränge, bekäme eine kürzere Kette,
die in sich stimmig ist und zu einem anderen Plan gehört: genau der
Fall, den Kap. 8.4 „ausgelassen" nennt.

### v0.10.0 – 2026-09-05 (Punkt 5.2: der Aufruf ist ein Vorschlag)

`myl_local_agent::werkzeug`. Das Format ist die Hermes-Form, die Qwen
spricht: Werkzeuge als JSON in einer Systemnachricht, der Vorschlag als
`<tool_call>{…}</tool_call>` im Antworttext.

⚑ **Das Format ist die Nebensache. Die Aussage ist: ein Vorschlag ist
keine Erlaubnis.**

| Quelle der Erlaubnis | zulässig |
|---|---|
| `Erlaubnis`, gesetzt **vor** dem Lauf | ja |
| Sitzungskontrakt, `myl_agent::Plan` | ja |
| **die Antwort des Modells** | **nein** |
| **das Ergebnis eines Werkzeugs** | **nein** |

⚑ **Die Prüfung sieht den Vorschlag nicht an.** Sie fragt nicht, ob er
verdächtig aussieht, sondern ob sein Name in der Erlaubnis steht. Ein
Filter, der nach Aussehen sortiert, ist ein Wettrennen gegen den
Formulierungsspielraum einer Sprache; eine Positivliste ist keines. Und
die Argumente gehen bewusst nicht ein: Eine Prüfung, die je nach
Argument anders entscheidet, ist wieder ein Filter.

**Der Angriff steht als Test da**, nicht als Absatz: Ein
Werkzeugergebnis enthält präparierten Text mit einem `<tool_call>` für
`ueberweisen`, das Modell plappert ihn nach, **der Vorschlag entsteht**,
und die Erlaubnis lehnt ihn ab.

### ⚑ Kodieren ist nicht Filtern

Der erste Entwurf des Tests verlangte, dass der Marker im
Werkzeugergebnis **entschärft** wird. Er scheiterte, und das war richtig:
Das Ergebnis geht **unverändert** zurück, nur JSON-kodiert, damit der
Rahmen nicht zerbricht. Eine eigene Zeile prüft, dass das Dekodieren
Zeichen für Zeichen dasselbe ergibt: **Ein Filter bestünde sie nicht.**

Unschädlich ist der Text aus einem anderen Grund: `vorschlaege` wird auf
die Antwort des Modells angewendet und auf nichts sonst.

### ⚑ Fund 181: die Tür setzt den Prompt nicht in der Vorlage zusammen

Qwen2.5 und Qwen3 sind auf **ChatML** trainiert
(`<|im_start|>role\ncontent<|im_end|>`); `myl_gateway::oai` setzt
`role: content` aneinander. **Für eine einzelne Frage fällt das kaum
auf, für den Agent Layer fällt es auf**: Werkzeugaufrufe hängen an
genau dieser Vorlage, und ohne sie schlägt ein Modell seltener oder gar
keine vor, **ohne dass jemand dem Ergebnis ansieht, warum**.

**Nicht geändert**, weil die Vorlage die Token bestimmt, die Token die
E2E-Vektoren und die den Konformitätswert. Das ist eine Entscheidung
über den numerischen Vertrag, keine Verbesserung nebenbei.

⚑ **Was geprüft ist**: Angebot und Werkzeugantwort **kommen an**. Über
die echte Tür bis in die geshardete Pipeline, 127 Prompt-Token statt 10.
**Nicht geprüft** ist, ob das 0,5B-Modell daraufhin einen Aufruf
vorschlägt; das tut es unzuverlässig, und ein Test, der davon abhinge,
wäre flatterig statt scharf.

### v0.9.0 – 2026-09-05 (Punkt 5.1: das Harness spricht mit der Tür)

`myl_local_agent::Tuerklient` schickt eine Vervollständigung an
`/v1/chat/completions`, mit der Vollmacht als `Authorization: Bearer`,
und liest die Antwort als Text, Abschlussgrund, Segmentkennung und
Verbrauchszahlen.

⚑ **Der Beleg steht in `myl-testclient`, und das ist der Kern der
Sache.** Das Harness darf `myl-gateway` nicht kennen, das Gateway kennt
das Harness nicht, und beide schreiben die OpenAI-Form **unabhängig**
auf. Ein Klient, der die Typen des Servers benutzte, könnte die
Behauptung „ein gewöhnlicher OpenAI-Klient erreicht diese Tür" gar nicht
belegen: Er passte auch dann noch, wenn beide gemeinsam von der Form
abgewichen wären. Gemessen über einen echten Socket bis in die
geshardete Pipeline: **„Die Hauptstadt von Frankreich ist Paris"**, acht
Token, Abschlussgrund `stop`.

⚑ **Und keine HTTP-Bibliothek.** `reqwest` zöge `tokio`, `hyper` und
`rustls` herein, für eine POST-Anfrage mit einem Kopf und einem Rumpf.
`std::net::TcpStream` genügt, und jede Abhängigkeit weniger ist eine
Angriffsfläche weniger an der Stelle, die eine Vollmacht trägt.

⚑ **Der Status wird unterschieden und nicht eingeebnet.** Ein Harness,
das jede Nicht-200 gleich behandelt, kann „die Vollmacht ist abgelaufen"
nicht von „kein Guthaben" und nicht von „der Knoten rechnet gerade"
trennen. Der Mensch davor soll erfahren, was zu tun ist, und das ist je
nach Zahl etwas anderes.

📌 **Drei Testdateien lasen die Antwort bisher mit `read_to_end`**, sie
verlassen sich also darauf, dass die Gegenseite auflegt. Ein Klient, der
eine Vollmacht trägt und eine Abrechnung auslöst, darf nicht daran
hängen, ob der Server `keep-alive` beherrscht: Er läse bis zur Frist und
meldete eine Zeitüberschreitung für eine Antwort, die längst vollständig
da war. Der neue Klient liest `Content-Length` und meldet einen
abgeschnittenen Rumpf als Abbruch statt als „unlesbar".

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
