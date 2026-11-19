# GATEWAY: die Tür zum Netzmodell (`myl-gateway`)

> **Version:** 0.16.1
> **Datum:** 2026-11-18
> **Status:** Die eigene Tür eines Knotens auf der Rückschleife
> (Vorgabe `127.0.0.1:4160`): `/v1/models`, `/v1/completions` und
> `/v1/chat/completions`. Zugang über eine Vollmacht aus einem
> Sitzungskontrakt der Kette; Guthaben und Höchstbetrag werden vor dem
> Rechnen geprüft, und jede Anfrage hinterlässt einen Beleg. Gerechnet
> wird über den Knoten im Shard. **Ein öffentliches Gateway** mit TLS,
> Zugang für Fremde und Ratenbegrenzung **gibt es noch nicht.**

Der Weg, auf dem eine Anfrage das Netzmodell erreicht. Die Tür spricht
das Format, das viele Werkzeuge schon kennen, prüft, ob die Anfrage
bezahlt ist, gibt sie an den Knoten und schreibt fest, was geliefert
wurde. Whitepaper Kap. 3.3 führt Gateways als eine der Rollen im Netz.

| Wer | Was hier wichtig ist |
|---|---|
| **Nutzer** | `myl frage --tuer HOST:PORT --agent NAME TEXT` und `myl sitzung --tuer`; die Vollmacht entsteht mit `myl wallet sitzung`. |
| **Betreiber** | Die Tür bindet nur an die Rückschleife. Nach außen gebunden würde ein Überlastangriff gegen die Tür einer gegen den Konsens. `--ohne-tuer` schließt sie; `myl mining` startet den Knoten so. |
| **Entwicklung** | Keine HTTP-Bibliothek, handgeschriebenes HTTP: Begründung und Grenzen in den Abschnitten unten. |

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

### v0.16.1 (2026-11-18): Der Kopf für Nutzer, Betreiber und Entwicklung

- **Der Kopf ist für Nutzer, Betreiber und Entwicklung geschrieben:** ein Absatz in
  einfachen Worten, eine Tabelle, was wer hier findet, und ein Stand, der nennt,
  was heute in Gebrauch ist und was offen ist. Punktnummern des Plans und
  gezählte Tests fallen aus dem Kopf; sie alterten schneller als der Text.
- ⛔️ **Der alte Stand war überholt:** „an einen Pod gibt sie noch nichts“. Die Tür im Knoten gibt Aufträge über die Ortsleitung an den Shard (`--ortsleitung`); offen ist das öffentliche Gateway. Der Kopf nennt die drei Wege unter `/v1`.
- Am Code nichts geändert.

### v0.16.0 (2026-11-17): Zusagen werden vorgemerkt, Aufrufe nur angebotener Werkzeuge

- ⚑ **`zugang::Zusage`: Eine Zusage mindert das Guthaben, solange ihre
  Anfrage läuft.** 📌 Bis hierher zog das Guthaben nur ab, was abgerechnet
  und noch nicht in der Kette war. Ein Inhaber mit mehreren Sitzungen (A23
  lässt je Sitzung eine Anfrage zugleich zu) bekam dasselbe Guthaben jeder
  zugesagt; die Kette nahm nur so viele Abrechnungen an, wie es deckte, und
  der Rest war Rechenarbeit, die er nicht bezahlte. Jetzt ermittelt
  `Zusage::geben` den Höchstbetrag und merkt ihn unter derselben Sperre für
  den Inhaber vor; jeder Ausgang der Anfrage gibt ihn frei (`Drop`). Danach
  steht der abgerechnete Betrag in der Liste der Quelle, kurz doppelt, nie
  gar nicht. ⚠️ Je Tür; zwei Türen wissen nichts voneinander. Probe
  `laufende_zusagen_mindern_das_guthaben` (80, dann 20, dann 0; frei nach
  dem Fallen; Gegenprobe ohne Vormerkung zweimal 80). Im Zweig „Prompt
  ungedeckt“ wird die eigene Vormerkung vor dem Grund freigegeben.
- ⚑ **Nur Aufrufe angebotener Werkzeuge werden `tool_calls`**
  (`werkzeuge::aufrufe_lesen` nimmt die Namen, `Chatanfrage::angebotene_namen`).
  Ein anderer Name bleibt im Text, wie bei OpenAI. Ein Harness, das nach
  Namen verteilt, bekäme sonst einen Aufruf eines Werkzeugs, das es für
  diese Anfrage nicht angeboten hat, und eine eingeschleuste Anweisung
  hätte dafür nur den Namen zu raten. Probe mit Gegenprobe in
  `aufrufe_werden_gelesen_und_unlesbares_bleibt_stehen`.

### v0.15.0 (2026-11-17): Die Tür sagt zu, was gedeckt ist (Fund 711)

- ⚑ **Prompt und Ausgabe kosten** (Entscheidung des Projektinhabers vom
  2026-11-17). Vor dem Rechnen kennt die Tür den Preis nicht, denn die
  Prompt-Token zählt nur der Pod. Sie prüft deshalb nicht einen geschätzten
  Preis, sondern ermittelt mit **`Zugangsstelle::hoechstbetrag`** das
  Grösste, was Vollmacht, Kontrakt und Guthaben decken, höchstens die
  Schranke aus Bytes und Deckel, und gibt es als
  **`Rechenauftrag::hoechstens`** mit. Halbierend gesucht, weil jede der drei
  Prüfungen monoton im Betrag ist. Eine Prüfung gegen eine Schätzung aus den
  Bytes hätte gedeckte Anfragen abgewiesen: Die Bytes sind rund das
  Dreifache der Token.
- **`Rechenweg::rechne`** gibt `Result<Rechenergebnis, Rechenfehler>`
  zurück. `Rechenfehler::NichtGerechnet` bleibt 502; **`Ungedeckt`** (schon
  der Prompt kostet mehr als zugesagt) wird **402** mit dem Grund aus
  **`Zugangsstelle::was_fehlt`** und dem Preis des Prompts, damit ein Klient
  nachkauft statt neu anzulaufen. Ist nicht einmal ein Credit gedeckt, kommt
  402 vor dem Rechnen.
- Eine Antwort, die der Pod auf die Zusage gekürzt hat, endet mit
  `finish_reason` `length`, wie eine am Deckel.
- Proben `der_hoechstbetrag_ist_das_groesste_was_alle_decken` (Guthaben,
  Einzellimit, Vorbehalt der Vollmacht und Schranke begrenzen je für sich;
  ein Credit mehr ist nicht gedeckt) und
  `die_tuer_sagt_zu_was_gedeckt_ist_und_ein_ungedeckter_prompt_bekommt_402`
  über einen echten Socket.

### v0.14.0 (2026-11-16): Die Tür prüft das Guthaben des Inhabers (Fund 707)

- ⚑ **Festlegung des Projektinhabers vom 2026-11-16:** Credits bleiben kurz
  gültig, die Tür prüft das Guthaben des Inhabers abzüglich offener
  Abrechnungen, der Klient kauft je Epoche nach.
- **`Kontraktquelle::guthaben`** (Vorgabe `None`): die nicht verfallenen
  Credits des Inhabers laut Kette, abzüglich dessen, was diese Tür schon
  abgerechnet hat und die Kette noch nicht kennt. Weiß die Quelle es nicht
  (eine Probe ohne Kette), prüft die Tür es nicht.
- **`Zugangsstelle::deckt_das_guthaben`**, in `bedienen_v1` nach Vollmacht und
  Kontrakt: Reicht das Guthaben für den Höchstbetrag der Anfrage nicht, kommt
  `402` mit „das Guthaben des Inhabers deckt diesen Auftrag nicht“, bevor
  gerechnet wird. 📌 Die Kette bucht eine Abrechnung vom Konto des Inhabers
  ab (`credit_spend`) und nimmt sie ohne Deckung nicht an; bis hierher
  rechnete die Tür dann umsonst, und Credits verfallen nach einer Epoche.
- Probe `das_guthaben_muss_den_hoechstbetrag_decken` mit Gegenproben im selben
  Zug (genug Guthaben deckt, ohne Kenntnis wird nicht geprüft, ohne Kontrakt
  deckt nichts).

### v0.13.0 (2026-11-16): Die Tür setzt den Chat in der Vorlage des Netzmodells und kennt Werkzeuge (Fund 181)

- ⚑ **Festlegung des Projektinhabers vom 2026-11-16**, nach kritischer
  Prüfung: Es passt zum Konzept (AGENT_LAYER: „Die Tür des eigenen Knotens
  spricht die OpenAI-Form … ein gewöhnlicher Klient und kein Sonderweg“).
  Verworfen: ein lokaler Proxy im Klienten (die Tür bliebe für gewöhnliche
  Klienten falsch) und eine Jinja-Vorlage im Artefakt (das Artefakt trägt
  keine, sein Abdruck wäre berührt, fremder Vorlagencode liefe in der Tür).
- **`Chatanfrage::prompt_in(vorlage)` statt `prompt()`**: Die Unterhaltung
  steht in der Vorlage des Netzmodells (`myl_types::vorlage`) und nicht mehr
  als `rolle: inhalt`, eine Form, auf der das Modell nie trainiert wurde.
  Wie in der Vorlage der Qwen-Familie: die Werkzeugansage in der
  Systemnachricht nach deren Text, ein Aufruf im Verlauf als
  `<tool_call>`-Block, Werkzeugantworten hintereinander als
  `<tool_response>`-Blöcke in einer Nutzerrunde, der Denkblock leer.
- **`tools` und `tool_choice`**: höchstens 128 Werkzeuge, je `type: function`
  mit Namen; `tool_choice` `auto` oder `none`. ⚑ `required` oder ein
  bestimmtes Werkzeug lehnt die Tür ab, statt still `auto` daraus zu machen:
  Erzwingen kann sie keinen Aufruf. `Nachricht` trägt `tool_calls` und
  `tool_call_id`, `content: null` gilt als leer.
- **`Chatantwort::werkzeugaufrufe_lesen`**: Die `<tool_call>`-Blöcke der
  Antwort werden OpenAI-`tool_calls` (Kennung `call_<sitzung>_<n>`,
  Argumente als JSON-Text in fester Form), `finish_reason` heißt
  `tool_calls`, der Text ohne die Aufrufe wird `content`. Was sich nicht
  lesen lässt, bleibt Text.
- **`werkzeuge`** (neu): die Werkzeugansage mit sortierten Schlüsseln (die
  Reihenfolge eines `serde_json::Value` hängt am Merkmal `preserve_order`,
  das irgendeine Kiste im Baum einschalten kann), Aufruf und Antwort im
  Verlauf, nachsichtiges Lesen der Aufrufe wie im Agenten.
- **`Rechenweg::vorlage`**, Vorgabe ChatML; der Knoten nennt die seines Netzes.
- Proben: Vorlage und Verlauf (Gegenprobe: `tool_choice: none` sagt nichts
  an), Ablehnung von `required` und namenlosen Werkzeugen, Aufrufe werden
  `tool_calls` (Gegenprobe: ohne Aufruf kein Feld), über einen echten Socket
  `ein_harness_mit_werkzeugen_bekommt_tool_calls` (Gegenprobe: ohne die
  Verdrahtung in der Tür rot). Zwei Proben erwarteten noch `rolle: inhalt`
  und sind umgestellt.

### v0.12.0 (2026-11-15): Ein fertiger Prompt über `/v1/completions`

- ⚑ **Ein zweiter Weg der Tür, `WEG_TEXT`**: Ein Klient, der die Vorlage
  seines Modells kennt, schickt den Prompt fertig (`Textanfrage`: `prompt`
  als eine Zeichenkette, `max_tokens`), und die Tür reicht ihn unverändert
  weiter. Ausweis, Deckel, Beleg, Kontrakt und Abrechnung sind dieselben wie
  beim Chat; die Antwort (`Textantwort`) trägt `choices[0].text` und dieselben
  Zusätze. Der Agent von Myelith nutzt das über die Tür (CLIENT 0.237.0).
- ⚑ **Kein neues Recht:** Auch über `/v1/chat/completions` gehen
  Sonderzeichen wie `<|im_start|>` im Inhalt unverändert zum Wortschatz; hier
  fällt nur die Zusammensetzung weg. Eine Liste von Prompts oder Tokennummern
  wird abgelehnt (mehrere Aufträge unter einem Beleg, ein Wortschatz beim
  Klienten).
- 📌 **`finish_reason` sagt jetzt, ob der Deckel griff** (`oai::abschluss`):
  `length`, wenn so viele Token kamen, wie erlaubt waren, sonst `stop`. Bis
  hierher stand immer `stop`, und ein Klient, der abgeschnittene Antworten
  fortsetzt, hielt jede für vollständig. Seit Fund 702 (COMPUTE_PIPELINE
  0.66.0) hält der Pod an der Endmarke, vorher kam jede Antwort bis zum Deckel.
- ⚠️ **Fund 181 bleibt offen:** Über den Chatweg bekommt das Modell die
  Unterhaltung weiter als `rolle: inhalt`.
- Geprüft: vier neue Tests in `oai.rs` (Anfrage unverändert, Ablehnungen,
  Antwortform, Abschluss) und `tests/tuer.rs::ein_fertiger_prompt_kommt_unveraendert_an`
  (über einen echten Socket: ohne Rollen beim Rechenweg, Textform,
  `length` bei einem Deckel von eins). Gegenproben je Zeile: `>` statt `>=`
  im Abschluss, Strom erlaubt, leerer Prompt erlaubt, Deckel übergangen,
  Textweg nie gewählt, Textweg nicht erlaubt; jeweils genau der passende Test
  rot.

### v0.11.0 (2026-11-08): Mehrere Anfragen zugleich

⚑ **Sicherheitsaudit A23.** `Tuer::v1_beantworten` und `bedienen_v1` nehmen
Annahme und Zugang geteilt (`Mutex`) und sperren nur für die Buchung, nie über
die Inferenz. `tuer::Besetzung` lässt je Sitzung eine Anfrage rechnen; die
zweite bekommt 429 und keine Sitzungsnummer. `Tuer::voll` antwortet 503, ohne
zu lesen. Die Statuszeile kennt 429 und 503. 📌 `bool::then_some` baute den
Griff auch bei besetzter Sitzung, und sein `Drop` wartete auf die Sperre, die
derselbe Ausdruck noch hielt; die neue Probe hing daran. Probe
`eine_sitzung_rechnet_nur_einmal_zugleich` mit Gegenprobe.

### v0.10.0 (2026-10-26): Der Kontrakt deckt den Höchstbetrag, bevor gerechnet wird (Fund 651)

- 📌 **Fund 651: Die Tür rechnete über die Grenzen des Kontrakts.** Vor dem
  Rechnen prüfte sie die Vorbehalte der Vollmacht und den Kontrakt mit dem
  Betrag eins; Einzellimit und Restbudget prüfte erst die Kette beim
  Abbuchen. Eine Anfrage darüber wurde gerechnet und beantwortet, die
  Abbuchung scheiterte, die Arbeit war verschenkt. Jetzt hält
  `Zugangsstelle::deckt_der_kontrakt` den Höchstbetrag (`max_tokens`) mit
  derselben Prüfung gegen den Kontrakt, die die Kette anlegt
  (`pruefe_zugang`), und die Tür antwortet vorher mit 402. Beleg im
  TESTCLIENT (`einzellimit_und_restbudget_greifen_vor_dem_rechnen`: über
  dem Einzellimit und über dem Restbudget läuft das Rechenwerk nicht,
  innerhalb der Grenzen schon); ohne die Prüfung rot. Gefunden beim
  Nachtrag der Angriffsklassen A16 bis A24 in `SIMULATION/Sicherheitsaudit.md`.
- `myl-gateway` 0.10.0.

### v0.9.0 (2026-10-26): Annehmen und Beantworten getrennt (Fund 647)

- ⚑ **`Tuer::annehmen`** wartet auf eine Verbindung und gibt sie als
  `Verbindung` zurück, ungelesen; `mit_zugang_beantworten` und
  `v1_beantworten` bedienen sie. So liest ein Wirt Epoche und Uhr **nach**
  dem Warten und nicht davor. 📌 Fund 647: Der Knoten las beides vor dem
  Warten, und die erste Anfrage nach einer Pause wurde gegen die alte
  Epoche geprüft und abgewiesen. `bedienen_mit_zugang` und `bedienen_v1`
  bleiben als Kurzform (annehmen, beantworten) und sagen in ihrer Doku,
  dass ihre Werte vor dem Warten feststehen. Der Beleg steht im Knoten
  (Test mit Gegenprobe). 55 Tests grün. `myl-gateway` 0.9.0.

### v0.8.1 (2026-10-24): `tokio` 1.54.1 wegen RUSTSEC-2026-0386

- Die Sperrdatei hebt `tokio` von 1.53.1 auf 1.54.1: Die sicheren Konstruktoren von `AsyncFd` verletzen die I/O-Sicherheit (RUSTSEC-2026-0386, `unsound`, neu seit dem 2026-10-23), und cargo-deny war damit rot. Der eigene Code nutzt `AsyncFd` nicht; geändert hat sich nur die Sperrdatei, der Vorrat trägt die neue Fassung. `myl-gateway` 0.8.1.

### v0.8.0 (2026-10-24): Die Zugangsprüfung fragt nicht mehr nach dem Empfänger

- Der Sitzungskontrakt trägt nur noch die Wurzel seiner Empfängerliste
  (SHARED_TYPES 0.49.0). Die Tür fragt nach dem **Zugang** und bucht nichts;
  sie prüft deshalb mit `pruefe_zugang` (Agent, Nummer, Widerruf,
  Zeitfenster, Grenzen) statt mit dem ersten Eintrag einer Liste, die es auf
  der Kette nicht mehr gibt. 55 Tests grün.

### v0.7.0 (2026-10-23): Eine Lesefrist für die Tür (Fund 610)

Aus dem Sicherheitsaudit vom 2026-10-23.

- ⚑ **Fund 610:** Die Tür las ohne Frist, und ihr Wirt (der Knoten) bedient
  eine Verbindung nach der anderen. Ein Prozess auf derselben Maschine, der
  eine Verbindung öffnet und nichts schickt, sperrte sie für alle,
  unbegrenzt. Jetzt `LESEFRIST` = 10 s für Kopf und Rumpf zusammen, danach
  `408`; für Tests `Tuer::mit_lesefrist`.
- Die Leseschleife stand zweimal (`/inferenz` und `/v1`) und steht jetzt
  einmal (`anfrage_lesen`); die Fehlertexte sind dieselben.
- **Gegenprobe** (`eine_schweigende_verbindung_sperrt_die_tuer_nicht`): Ein
  Schweiger hält die Verbindung, die nächste Anfrage bekommt ihren Beleg, er
  ein `408`. Ohne Frist: „die Tuer hat innerhalb von fuenf Sekunden nicht
  geantwortet“.
- Geprüft und in Ordnung: doppeltes `Content-Length` und
  `Transfer-Encoding` abgelehnt, Kopf und Rumpf gedeckelt, ein kurzer Rumpf
  nicht als kurze Anfrage genommen, Ausweis vor jedem Zerlegen, Deckel auf
  die Signaturprüfungen je Block der Vollmacht. CSRF und DNS-Rebinding aus
  dem Browser nützen nichts, denn jeder Weg verlangt ein Geheimnis.
- Belege: Clippy und alle Tests grün.

### v0.6.1 (2026-10-22): Eine Werkzeugkette für alle Kisten

- `rust-version` auf 1.99 in `myl-gateway`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  Nur das Manifest, kein Code.
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen.

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

### Die Bauart: eine Signaturkette, kein Logikprogramm

Der geläufige Bauplan für abschwächbare Zugangsmarken lässt den
**Aussteller** prüfen, mit einem Wurzelgeheimnis, das er selbst hält.
**Hier geht das nicht:** Aussteller ist der **Nutzer**, Prüfer das
**Gateway**, also zwei Parteien. Ein HMAC über ein gemeinsames Geheimnis
scheidet aus, denn das Gateway darf den Schlüssel des Nutzers nicht
haben. **Eine Kette signierter Blöcke löst genau das.**

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

⚑ **Was ausdrücklich nicht kommt: ein Logikprogramm an der Tür.** Ein
Interpreter, der vom Anfragenden gelieferte Regeln auswertet, ist eine
Angriffsfläche, die zu dieser Stufe nicht passt:
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

📌 **Der erste Testlauf blieb hängen**, und das war lehrreich: Der
Testklient schloss seine Schreibseite nicht, also wartete der Server auf
den angekündigten Rest und der Klient auf die Antwort. **Ein Deadlock,
und er hat gezeigt, dass der Server einen abgebrochenen Rumpf am
Dateiende erkennt und nicht an einer Frist** — die richtige Reihenfolge.
Der Klient schließt jetzt, und eine Fünf-Sekunden-Frist darüber ist der
zweite Riegel: **Ein hängender Test sagt nichts, ein fehlgeschlagener
sagt etwas.**

**Was Stufe 1 nicht tut:** an einen Pod geben. Dafür braucht sie eine
Sitzung im Netz, und die ist eigene Arbeit.
