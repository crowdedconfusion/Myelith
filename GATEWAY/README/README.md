# gateway (`myl-gateway`)

> **Version:** 0.6.0
> **Datum:** 2026-09-03
> **Status:** Stufe 1 steht: die Tür auf `localhost` und der Beleg. Sie
> nimmt entgegen und schreibt fest; **an einen Pod gibt sie noch nichts**.

## ⚑ Die Rolle stand im Papier und hatte keine Komponente (Fund 87)

Kap. 3.3 führt Gateways als eine von sechs Rollen: Nutzeranfragen
entgegennehmen, an Pods geben, Ergebnisse zurückliefern. **Es gab weder
Komponente noch Code noch einen HTTP-Server im ganzen Repositorium.**
Zwei gebaute Schutzmechanismen hatten damit keinen Betreiber, und das
Netzmodell war ohne den Agent Layer nicht benutzbar.

## Der Schnitt: Stufe 1

Das eigene Gateway auf `localhost`. **Der Betreiber ist der
Kontoinhaber, also entfällt die ganze Bezahlfrage**, und mit ihr
Zugangsschlüsselverwaltung, Ratenbegrenzung und Missbrauchsschutz. Was
bleibt, ist die Tür und der Beleg, und der Beleg ist ohnehin das
Produkt.

## ⚑ Keine HTTP-Bibliothek, und das ist eine Entscheidung

Sie ist die größte neue Abhängigkeitsfläche seit `libp2p` und gehört
deshalb **vor** den ersten Code entschieden. Sie lautet: keine.

Was ein Rahmenwerk mitbrächte, ist Wegewahl und Mittelschicht für
Anforderungen, die Stufe 1 nicht hat; `axum` zöge `hyper` und `tower`
nach, **drei Bäume für eine Tür mit einem Weg**. `tokio` liegt ohnehin in
NODE und NETWORKING.

⚑ **Der eigentliche Gewinn ist der Zeitpunkt.** Sobald Stufe 2 kommt,
also ein öffentliches Gateway mit TLS, Zugang und Ratenbegrenzung, ist
die Rahmenwerksfrage eine **echte** Frage mit echten Anforderungen. Sie
jetzt zu entscheiden hieße, sie ohne Anforderungen zu entscheiden. **Wer
die Abhängigkeit für die Stufe nimmt, die sie nicht braucht, trifft die
Wahl im ungünstigsten Augenblick.**

**Wird umgestoßen, wenn** Stufe 1 mehr als einen Weg braucht, oder ein
Klient mehr verlangt als `Content-Length` und einen Rumpf.

## ⚑ Handgeschriebenes HTTP: wo die Gefahr liegt und wie sie gefasst ist

Sie liegt dort, wo **Zerlegung auf fremde Eingaben** trifft. Deshalb
steht das Zerlegen als **reine Funktion** ohne Netz und wird einzeln
geprüft; der Teil, der Sockets anfasst, bleibt so dünn, dass an ihm
nichts schiefgehen kann. Dieselbe Bauart wie `anfragen_fuer` im Knoten.

**Abgelehnt wird ausdrücklich, statt geraten:**

- **`Transfer-Encoding: chunked`.** Es stillschweigend als Rumpf zu
  lesen wäre die klassische Schmuggelstelle: zwei Leser, zwei Meinungen
  über die Nachrichtengrenze.
- **Fehlendes oder doppeltes `Content-Length`.** Ohne Länge weiß niemand,
  wo die Nachricht endet; mit zweien wissen es zwei verschieden.
- **Ein Rumpf, der kürzer ankommt als angekündigt.** Ihn als kurze
  Anfrage zu nehmen hieße, **eine andere Frage festzuschreiben als die
  gestellte**.
- **Alles über den Grenzen** (Kopf 8 KiB, Rumpf 1 MiB). Der Deckel ist
  eine Ablehnung und kein Abschneiden.

⚑ **`Tuer::binden` nimmt keine Adresse entgegen**, sondern bindet fest an
`127.0.0.1`. Eine Adresse als Parameter wäre eine Einladung, sie auf
`0.0.0.0` zu setzen, und dann stünde eine Tür ohne Schloss im Netz. **Wer
öffentlich hören will, braucht Stufe 2 und nicht ein anderes Argument.**

## Der Beleg, und warum er zuerst kam

Ein Gateway, das nur eine Antwort liefert, ist ein Weiterleiter. **Was
Myelith anders macht, ist der Beleg.**

⚑ **Und ohne die Bindung der Anfrage geht auch Stufe 2 der Verifikation
nicht.** Der Prompt kam im Konsens nicht vor; ein Checker müsste ihn dem
Pod glauben und prüfte dann, ob der Pod zu seiner **eigenen** Eingabe
passt. Eine Frage, auf die der Gefragte beide Hälften wählt. Deshalb
schreibt das Gateway **zuerst** fest und leitet **dann** weiter.

Gebunden wird `SHA-256` über Trennstring, Sitzungsnummer und Anfrage;
die Sitzungsnummer geht mit ein, damit dieselbe Anfrage in zwei
Sitzungen nicht denselben Wert bekommt und eine Bindung nicht
übertragbar ist.

## Changelog

### v0.6.0 – 2026-09-03 (die Vollmacht zieht in `myl-types`, `modell()` fragt statt zu raten)

**`vollmacht` wohnt jetzt in `myl-types`**, weil die Kette sie prüfen
muss, um eine gerechnete Anfrage abzubuchen. `myl-gateway` reicht sie
weiter; `myl_gateway::vollmacht::Vollmacht` bleibt gültig.

⚑ **`Rechenweg::modell` ist jetzt `async` und gibt `Option`** (Fund
160). Die synchrone Fassung gab `"unbekannt"` als Pipeline-Stand
zurück, solange die erste Inferenz den Zwischenspeicher nicht gefüllt
hatte, und ein Harness fragt die Modelle **zuerst**: Ohne Gegenseite
antwortet die Tür jetzt mit `502` statt mit einem Platzhalter, den
niemand bestätigt hat.

### v0.5.0 – 2026-09-03 (⚑ Fund 160: die Modellliste log, und `prompt_tokens` zählte Bytes)

**Beide Fehler kamen beim ersten Lauf gegen ein echtes Modell heraus**,
und beide sind dieselbe Klasse: ein Feld mit festgelegter Bedeutung,
gefüllt mit etwas anderem.

⚑ **`/v1/models` meldete `"myelith_pipeline":"unbekannt"`.** Der
Rechenweg gab den Stand aus einem Zwischenspeicher, den erst die erste
Inferenz füllt. **Ein Harness fragt die Modelle als Erstes**, also war
das der Normalfall.

Die Ursache steckte im Typ: `Rechenweg::modell` war **synchron**,
obwohl die Antwort eine Frage an einen fremden Prozess ist. Jetzt ist
sie `async` und gibt `Option`; ohne Gegenseite antwortet die Tür mit
`502` statt mit einem Platzhalter, den niemand bestätigt hat.

⚑ **`usage.prompt_tokens` zählte Bytes.** Ein Klient rechnet damit
Kosten. Der Wortschatz liegt beim Shard, und daraus folgt nicht „dann
schätzt die Tür", sondern „dann zählt der Shard":
`Rechenergebnis::prompt_token` kommt jetzt von dort.

**Beim `temperature`-Feld hatte ich genau das gesehen** und die Antwort
deshalb `myelith_deterministisch` tragen lassen. Zwei Felder weiter ist
mir dasselbe passiert, und gefunden hat es kein Test, sondern der erste
echte Aufruf.

### v0.4.0 – 2026-09-03 (Stufe 3: die Fläche nach aussen)

**`/v1/chat/completions` und `/v1/models`, in der OpenAI-Form.** Der
Zuschnitt steht in B6-3: `http://127.0.0.1:<port>/v1`, Bearer, kein TLS,
kein Rahmenwerk.

⚑ **Warum ausgerechnet diese Form:** Weil der Nutzer den Schlüssel
irgendwo einkleben muss. Das war Fund 150; ein Wechsel des Anbieters
heisst in jedem Werkzeug dieser Welt „Basis-URL und Schlüssel tauschen",
und wer eine eigene Form erfindet, verlangt von jedem Nutzer einen
eigenen Klienten.

**JSON schon, HTTP nicht.** Das Format kommt von aussen und ist nicht
verhandelbar; einen Zerleger für fremde Eingaben selbst zu schreiben
wäre die schlechtere Wahl als eine geprüfte Bibliothek. Die Entscheidung
gegen ein HTTP-Rahmenwerk bleibt davon unberührt: Wegewahl und
Mittelschicht braucht eine Tür mit zwei Wegen nicht.

⚑ **`temperature` wird angenommen und wirkt nicht, und die Antwort sagt
das.** Myelith rechnet ganzzahlig und deterministisch; das ist keine
Einstellung, sondern die Geschäftsgrundlage, ohne die zwei Pods
derselben Redundanzpaarung nicht vergleichbar wären. **Still zu
ignorieren wäre falsch**, denn wer `temperature` setzt, erwartet
Streuung: Die Antwort trägt `myelith_deterministisch`, und `/v1/models`
sagt es vor der ersten Anfrage. Eine Ablehnung hätte jedes Harness
gebrochen, das den Wert immer mitschickt.

⚑ **`stream: true` bekommt einen Grund und keine stille
Falschbedienung.** Ein Strom verlangt stückweise Übertragung, und die
Tür lehnt die ausdrücklich ab; ein Auftrag wird ausserdem als Ganzes
bezeugt und abgerechnet. Ein Klient, der einen Strom erwartet und eine
ganze Antwort bekommt, hängt in seiner Leseschleife.

⚑ **Der Ausweis steht vor dem Zerlegen.** Wer erst zerlegt, lässt einen
Fremden die Arbeit auslösen und verrät ihm über die Fehlermeldung, wie
gut sein JSON war. **Auch `/v1/models` verlangt ihn:** Wer die Liste
frei herausgäbe, sagte einem Fremden, welcher Pipeline-Stand hier läuft.

⚑ **Erst der Weg, dann das Verfahren.** Andersherum bekäme ein `GET` auf
einen Weg, den es gar nicht gibt, die Auskunft „falsches Verfahren", und
das sagte dem Fragenden, dass der Weg existiert.

**Und `502` statt `500`, wenn kein Pod gerechnet hat.** „Ich habe
niemanden gefunden" ist eine Aussage über die Gegenseite; bei 502
wiederholt ein Klient sinnvoll.

**Der Rechenweg ist ein Merkmal und kein Code in dieser Kiste.** Das
Gateway kann nicht rechnen lassen: Zuteilung, Sitzungskanal und
Transport kennt es nicht, und sie hereinzuziehen hiesse, die Tür an den
Konsensstapel zu binden. `myl-node` erfüllt das Merkmal.

**Zehn Tests über echte Sockets.**

### v0.3.2 – 2026-09-03 (Stufe 4 steht, und die Tür führt jetzt irgendwohin)

**Kein Code in dieser Kiste, und trotzdem der wichtigste Eintrag seit
Stufe 2.** Bis heute war die Tür fertig und führte nirgendwohin: Es gab
keinen Weg zu einem Pod, auf keiner der beiden Seiten. Deshalb wurde am
2026-09-03 entschieden, Stufe 4 vor Stufe 3 zu bauen, erst das Zimmer
und dann die Adresse.

**Seit heute geht der kalte Pfad durch.** Auftrag über das Netz an einen
Knoten, von dort über eine lokale Leitung an den Shard-Prozess, Antwort
denselben Weg zurück. Alle Sprünge sind echt, zwei Sockets, kein
nachgebautes Ende; die Naht liegt in `myl-testclient`, weil nur dort
beide Enden sichtbar sind.

**Was Stufe 4 noch fehlt:** der verschlüsselte Kanal zu den Pod-Enden.
⚑ **Er ist gebaut und hat keinen Aufrufer** (Fund 155):
`myl_net::sitzung` ist der hybride Austausch aus X25519 und ML-KEM 768,
2428 Zeilen mit fünf grünen Tests, und niemand ruft ihn. Wo er hingehört,
damit ein Shard-Prozess ihn erreichen kann ohne libp2p zu bauen, ist die
offene Entscheidung B6-4.

**Offen bleiben Stufe 3** (die Fläche nach aussen) **und Stufe 5** (die
Ununterscheidbarkeitsmessung).

### v0.3.1 – 2026-09-03 (die Tür nimmt einen fremden Lauscher)

`Tuer::aus_lauscher` übernimmt einen schon gebundenen `TcpListener`.

⚑ **Für einen Wirt, der selbst bindet.** Der Knoten beherbergt seit
`myl-node` v0.31.0 die eigene Tür und will beim Binden **melden können,
ob es geklappt hat**, bevor er eine Aufgabe abzweigt; ein
fehlgeschlagenes Binden in einer abgezweigten Aufgabe wäre eine Meldung,
die niemand liest.

**Damit trägt der Wirt die Verantwortung für die Adresse.**
`Tuer::binden` bindet weiterhin fest auf die Rückschleife; wer den neuen
Weg nimmt, muss selbst wissen, wohin er bindet, und der Knoten warnt,
wenn es nicht die Rückschleife ist.

### v0.3.0 – 2026-09-03 (⚑ der API-Schlüssel, den ein Harness einkleben kann)

**Eine Berichtigung derselben Stufe, noch am selben Tag.**

⚑ **v0.2.0 hat eine Frage übergangen, nicht beantwortet.** Sie verlangte
je Anfrage eine BLS-Unterschrift über Sitzung, Nummer, Epoche und
Prompt. Das ist die schärfere Zusicherung und **in kein bestehendes
Harness einzukleben**: Jeder Inferenzanbieter authentifiziert per
`Authorization: Bearer`, und ein Wechsel heisst Basis-URL und Schlüssel
tauschen. Ein Klient, der BLS signiert und die Konsensepoche kennt, ist
kein Schlüssel mehr, sondern ein Programm.

**Der Einwand kam vom Projektinhaber**, nicht aus einer Prüfung. ⚑ **Er
gehört zur selben Klasse wie Fund 147:** Kein Test der Welt merkt, dass
eine Schnittstelle korrekt und unbenutzbar ist.

**G2 bleibt gültig und war nie das Hindernis.** „Der Zugangsschlüssel
ist ein Sitzungskontrakt und kein Datenbankmerkmal" sagt, **wo die
Befugnis lebt**, nicht **was ein Nutzer einklebt**. Beides geht
zusammen.

### Zwei Wege, und der Beleg sagt, welcher es war

| Weg | Läuft im Harness | Gateway kann Anfragen erfinden |
|---|---|---|
| **Vollmacht** (`Authorization: Bearer`) | ja | ja, im Rahmen der Vorbehalte |
| **Unterschrift je Anfrage** | nein | nein, der Prompt ist gebunden |

⚑ **Der Unterschied wird vermerkt, nicht versteckt.** `Beleg::weg` nennt
ihn, damit ein Nutzer sieht, welche Zusicherung er hat, statt die
stärkere anzunehmen.

⚑ **Der Kopf entscheidet, welcher Weg gilt, nicht der Rumpf.** Mit
Bearer ist der Rumpf der nackte Prompt, und genau das macht die Tür
harnessfähig; ohne ihn ist er eine Hülle mit Unterschrift. **Zu raten,
welche Form vorliegt, wäre die Schmuggelstelle.**

### Die Bauart: Biscuits Signaturkette, nicht sein Datalog

**Macaroons prüft ihr Aussteller** mit einem Wurzelgeheimnis, das er
selbst hält. Hier ist der Aussteller der **Nutzer** und der Prüfer das
**Gateway**, also zwei Parteien: Ein HMAC über ein gemeinsames Geheimnis
scheidet aus, denn das Gateway darf den Schlüssel des Nutzers nicht
haben. **Biscuit löst genau das** mit einer Kette signierter Blöcke.

1. Der Vollmachtsblock ist mit dem Agentenschlüssel unterschrieben und
   nennt den **nächsten** öffentlichen Schlüssel.
2. Jeder weitere Block ist mit dem Schlüssel unterschrieben, den sein
   Vorgänger genannt hat, und nennt wieder einen nächsten.
3. Der **Nachweis** am Ende ist die Saat des zuletzt genannten
   Schlüssels.

⚑ **Abschwächen kann jeder Halter, ohne jemanden zu fragen.** Ein Agent
gibt einem Unteragenten weniger, als er selbst hat, und niemand muss
davon wissen.

⚑ **Wegnehmen kann niemand.** Wer den letzten Block streicht, hat einen
Nachweis, der nicht mehr zum zuletzt genannten Schlüssel passt; um einen
passenden zu bauen, bräuchte er die Saat des Vorgängers, und die hat der
Abschwächende weggeworfen. **Der Test dazu ist der wichtigste des
Moduls.**

⚑ **Was ausdrücklich nicht übernommen wird: Biscuits Datalog.** Ein
Logikinterpreter, der vom Anfragenden gelieferte Programme an der Tür
auswertet, ist eine Angriffsfläche, die zu dieser Stufe nicht passt:
unbegrenzte Laufzeit, unbegrenzter Speicher, eine eigene Zerlegung.
**Die Vorbehalte hier sind ein Aufzählungstyp mit vier Fällen**, jeder
in konstanter Zeit prüfbar.

### Die Grenzen, die daraus folgen

**Die Kettenlänge ist auf acht gedeckelt**, sonst bestimmt der
Anfragende, wie viele Paarungen die Tür rechnet, und der Deckel vor der
teuren Arbeit zählt **je Block** und nicht je Anfrage.

**Die Textform ist Base64 in der URL-sicheren Fassung, ohne Polster**,
damit eine Vollmacht in eine Kopfzeile, eine Umgebungsvariable und eine
Kommandozeile passt, ohne dass jemand quotiert. **Nicht kanonische
Kodierung wird abgewiesen**, sonst gäbe es zwei Zeichenketten für
dieselbe Vollmacht: dieselbe Formbarkeitsklasse, die das Fuzzing sonst
prüft.

**Zwei `Authorization`-Köpfe werden abgewiesen**, aus demselben Grund
wie zwei Längenangaben: zwei Ausweise heissen zwei Meinungen darüber,
wer da ist.

**Belegt:** zwölf neue Tests, davon vier über einen echten Socket, und
sechs Gegenproben, darunter die drei Angriffe auf die Kette
(abschneiden, Vorbehalt ändern, Nachfolger unterschieben).

### v0.2.0 – 2026-09-03 (Stufe 2: der Kontrakt ist der Zugangsschlüssel)

**Zwei neue Module, `zugang` und `takt`, und beide sind verdrahtet.**
`Tuer::bedienen_mit_zugang` ist der Weg der Stufe 2; `Tuer::bedienen`
bleibt der Weg der Stufe 1 auf der Rückschleife.

⚑ **Der Zugangsschlüssel ist ein Sitzungskontrakt und kein
API-Schlüssel** (Entscheidung G2). Ein API-Schlüssel wäre eine Zeile in
einer Tabelle, die der Betreiber führt; ein Kontrakt steht im Konsens,
und **wer ihn widerruft, widerruft ihn für alle Gateways zugleich**.
Das ist der Unterschied zwischen einem Betreiber, dem man vertrauen
muss, und einer Regel, die jeder nachrechnen kann.

⚑ **Der Agent ist eine Adresse, und eine Adresse gibt keinen Schlüssel
her.** Dieselbe Stelle wie Fund 109 und wie Glied 2 von Punkt 40: Aus
`SHA-256` folgt kein Urbild, also kann das Gateway mit dem Kontrakt
allein keine Unterschrift prüfen. Der Anfragende bringt seinen
öffentlichen Schlüssel mit.

⚑ **Verglichen wird er an genau einer Stelle, und die stand schon da.**
Der erste Entwurf hatte den Vergleich zusätzlich im Gateway; **die
Gegenprobe hat ihn als tot entlarvt**, denn `myl_types::sitzung::pruefe`
hält `vorhaben.handelnder` ohnehin gegen `kontrakt.agent`, und
`handelnder` folgt aus dem mitgebrachten Schlüssel. Ausgebaut blieb
alles grün. **Eine Prüfung, die nichts auswählt, ist schlimmer als
keine**, denn sie sieht nach Schutz aus und kann mit ihrer
Zwillingsprüfung auseinanderlaufen.

### ⚑ Genau ein Bit nach draussen, und das ist die halbe Stufe

Antwortete die Tür verschieden auf „diesen Kontrakt gibt es nicht", „er
ist widerrufen", „falscher Schlüssel" und „unlesbare Hülle", **wäre sie
ein Auskunftsdienst über fremde Kontrakte**: Wer Adressen durchprobiert,
erführe, welche existieren. Das ist das Abtasten, gegen das die Stufe
gebaut ist.

Jede Ablehnung ist `403` mit leerem Rumpf, und ein Test über einen
echten Socket verlangt, dass vier verschiedene Ablehnungen **byteweise
dieselbe Antwort** ergeben.

**Und eine abgelehnte Anfrage bekommt keine Sitzungsnummer**, sonst
verriete die Nummernfolge im nächsten Beleg, wie oft geklopft wurde.

### ⚑ Zwei Ratengrenzen, und die zweite ist die, die gefehlt hätte

**Die naheliegende** ist die je Kontrakt: sechs Anfragen je Minute.

⚑ **Die wichtigere ist die davor.** Eine Unterschrift zu prüfen kostet
eine Paarung, **gemessen 0,45 Millisekunden**. Wer Unsinn schickt,
zwingt das Gateway zu genau dieser Rechnung: ein paar hundert Bytes
hinein, Millisekunden hinaus. **Ohne eine Grenze vor der Prüfung ist die
Prüfung selbst der Angriff.** Dieselbe Klasse wie Fund 141, nur mit
Rechenzeit statt Bandbreite.

⚑ **Die Reihenfolge ist die Aussage:** Der Deckel auf Prüfungen steht
**vor** der Paarung, der Zähler je Kontrakt **danach**. Stünde er davor,
könnte jeder die Rate eines fremden Kontrakts aufbrauchen, indem er
dessen Sitzungsnummer nennt; **eine Sperre, die man gegen andere richten
kann, ist eine Waffe und keine Grenze.** Beide Grenzen einzeln
umzudrehen ergibt eine Lücke, und keine von beiden fällt beim Lesen
auf, deshalb stehen sie zusammen in `Zugangsstelle::durchlassen`.

**Die Zählkarte ist begrenzt und verdrängt nicht.** Sonst wäre der
Zähler selbst der Angriff (Klasse von Fund 144), und wer verdrängt,
lässt sich die Grenze eines anderen herausdrücken.

### Das Drahtformat: eine Hülle statt Kopfzeilen

Zugangsdaten in HTTP-Kopfzeilen wären das Übliche und hier die falsche
Wahl: Das Gateway zerlegt HTTP von Hand, und jede weitere Kopfzeile, die
etwas entscheidet, ist eine weitere Stelle, an der Zerlegung auf fremde
Eingaben trifft. **Eine Borsh-Hülle ist ein einziger Wert mit einer
einzigen Kodierung** und geht durch dieselbe Kanonizitätsprüfung wie
jeder andere Protokolltyp.

**Belegt:** dreizehn neue Tests, davon drei über einen echten Socket,
und fünf Gegenproben. ⚑ **Eine davon hat den toten Vergleich gefunden**,
und eine zweite zeigt, dass der langsamste Test des Crates (2,7
Sekunden) zugleich die Begründung seiner Grenze ist: Er misst, was ein
Angreifer sonst umsonst bekäme.

**Was Stufe 2 nicht ist:** Sie hört weiter auf der Rückschleife. Die
Fläche nach aussen ist Stufe 3, und sie ist eine eigene Entscheidung.

### v0.1.0 – 2026-09-01 (die Tür und der Beleg, Punkt 39)

Das **neunzehnte Crate**. 21 Tests: 15 auf die reinen Funktionen, 6 über
einen echten Socket.

⛑ **Der erste Testlauf blieb hängen**, und das war lehrreich: Der
Testklient schloss seine Schreibseite nicht, also wartete der Server auf
den angekündigten Rest und der Klient auf die Antwort. **Ein Deadlock,
und er hat gezeigt, dass der Server einen abgebrochenen Rumpf am
Dateiende erkennt und nicht an einer Frist** — die richtige Reihenfolge.
Der Klient schließt jetzt, und eine Fünf-Sekunden-Frist darüber ist der
zweite Riegel: **Ein hängender Test sagt nichts, ein fehlgeschlagener
sagt etwas.**

**Was Stufe 1 nicht tut:** an einen Pod geben. Dafür braucht sie eine
Sitzung im Netz, und die ist eigene Arbeit.
