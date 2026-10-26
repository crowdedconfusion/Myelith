# agent-layer

> **Version:** 0.55.1 (`myl-agent` 0.7.2, `myl-local-agent` 0.46.1)
> **Datum:** 2026-10-23
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

## Netz und Ort: die Trennung

⚑ **Festlegung des Projektinhabers (2026-09-30):** Zwischen
verifizierbaren agentischen Netzaktionen und lokalen Aktionen gibt es eine
strikte Trennung, und sie steht im Namen. Was mit `myl-` beginnt, gehört
zum Protokoll; was mit `local-` beginnt, wirkt nur auf dem Rechner des
Nutzers und ist nie Teil einer prüfbaren Netzaktion.

| Ordner | Seite | Was darin liegt |
|---|---|---|
| `myl-agent/` | Netz | Manifeste, Herkunftsstufe, Registratur, Session-Kontrakt, Plan, Segmentkette |
| `local-agent/` | Ort | der ganze örtliche Agent als eine Kiste (`myl-local-agent`): Schleife, Türklient, Risikoklassen, das Merkmal `Modellweg`, Werkzeuge mit Einhängegrenze, das Laden der Kisten, Skills, Loop, Notaus, Systemprompt |
| `local-toolkits/` | Ort | die mitgelieferten Werkzeugkisten als Manifeste und Skripte (`Base`, `Advanced`) |
| `local-skills/` | Ort | die mitgelieferten Skills und die Vorlage |

**Der Pfeil geht nur in eine Richtung.** `local-agent` hängt an
`myl-agent`, nie umgekehrt; der Client hängt an `local-agent`, nie
umgekehrt. `myl-agent` prüft das selbst (`tests/trennung.rs`): Weder seine
Sperrdatei noch sein Manifest darf etwas Örtliches nennen, mit Gegenprobe.

⚠️ Die Kiste in `local-agent/` heißt aus der Zeit davor `myl-local-agent`.
Ihr Ordner trägt das richtige Präfix; der Paketname wurde nicht
umbenannt, weil er in jedem `use` steht.

## Struktur von `myl-agent`

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

### v0.55.1 (2026-10-26): Proben räumen ihre Ordner auf (Fund 638; `myl-local-agent` 0.46.1)

- 📌 **Fund 638:** Die Proben in `vorhaben.rs`, `verlauf.rs`, `gedaechtnis.rs` und `tests/schlaf.rs` legten Ordner im Zwischenordner des Systems an und ließen sie liegen (19 je Lauf nach den ersten beiden). Jetzt hält sie ein `Probeordner`, der beim Fallen den Ordner entfernt. Die zwei Proben in `tests/dateiwerkzeuge.rs`, die ein Geheimnis eine Ebene über den Anhang legen, legen den Anhang in einen äußeren Probeordner, und das Geheimnis geht mit ihm. Mit `TMPDIR` auf einen leeren Ordner geprüft: 480 grün, nichts bleibt.
- Nur Proben geändert, das Programm nicht.

### v0.55.0 (2026-10-23): Kein Abruf ins eigene Netz, kein Schreiben über einen Verweis hinaus (Funde 615 bis 617; `myl-local-agent` 0.46.0)

Aus dem Sicherheitsaudit vom 2026-10-23.

- **Nachtrag vom 2026-10-24, J1 (SHARED_TYPES 0.49.0):** Die Probe
  `vollmacht_grenzen` baut ihre Vorhaben mit Empfängerbeweis aus der Liste
  des Inhabers; für einen Empfänger, der nicht darauf steht, gibt es keinen,
  und die Grenze greift wie zuvor.
- ⚑ **Fund 615, Web-Werkzeuge:** Umleitungen folgte curl selbst
  (`--location`); `--proto-redir` hielt nur das Schema, und das Endziel
  wurde erst **nach** dem Abruf gegen Schranke 5 gehalten. Den Inhalt bekam
  das Modell dann nicht, die Anfrage war aber schon im Heimnetz (blinde
  SSRF). Ein öffentlicher Name mit privater Adresse kam ganz ohne Umleitung
  durch. Jetzt folgt der Agent Umleitungen selbst (höchstens drei), jede
  Station geht **vor** dem Abruf durch `pruefe_ziel`, der Name wird selbst
  aufgelöst und gegen das eigene Netz gehalten (`ist_oeffentlich`:
  Rückschleife, privat, link-lokal, CGNAT, Dokumentation, Multicast,
  Reserviertes, ULA, IPv4 in IPv6, NAT64), und die geprüfte Adresse wird mit
  `--resolve` gebunden, gegen eine zweite Antwort des Namensdienstes.
  **Gegenprobe:** Ein Filter, der alles durchlässt, fällt an der Tabelle und
  an `localhost`. Die beiden Netzproben (eine Seite, ein PDF von arXiv)
  gehen über den neuen Weg durch.
- ⚑ **Fund 616, `write_file`:** Der letzte Namensteil wurde nicht
  aufgelöst, „denn es gibt ihn nicht“; beim Überschreiben gibt es ihn. Ein
  Verweis `notizen.md -> ~/.bashrc` im Arbeitsordner (ein fremdes
  Repositorium bringt so etwas mit) lag nach der Prüfung innerhalb, und der
  Agent schrieb hinaus, **ohne Freigabe**, denn Dateiwerkzeuge brauchen laut
  `secure_flow` keine. Jetzt wird ein vorhandener letzter Teil aufgelöst wie
  beim Lesen, ein Verweis ohne Ziel abgelehnt. **Gegenprobe:** Ohne die
  Auflösung steht im Opfer „eingeschleust“.
- **Fund 617, Syntaxwache:** Die Zwischendatei mit vorhersagbarem Namen
  wurde per `fs::write` angelegt; unter einem gemeinsamen `/tmp` hätte ein
  anderer Nutzer dort einen Verweis legen können. Jetzt `create_new`.
  **Gegenprobe:** Mit `fs::write` steht der Inhalt des Agenten in der Datei,
  auf die der Verweis zeigt.
- Geprüft und in Ordnung: der Informationsfluss mit Labels und Freigabe vor
  der Shell, `curl -q` und `--` vor der Adresse, nur `https` auf 443, keine
  IP-Literale, Fremdtext eingefasst und entschärft; die Sperrliste von
  `run_command` sagt selbst, dass sie kein Filter ist.
- Belege: Clippy und alle Tests grün.

### v0.54.1 (2026-10-22): Eine Werkzeugkette für alle Kisten (`myl-agent` 0.7.2, `myl-local-agent` 0.45.1)

- `rust-version` auf 1.99 in `myl-agent`, `myl-local-agent`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  ⚑ **Dazu Code, und zwar mit Absicht:** Mit `rust-version = "1.99"` gibt Clippy
  zwei Regeln frei, die es bei 1.85 zurückhielt (`manual_is_multiple_of`,
  `chunks_exact` mit fester Größe). Festlegung des Projektinhabers:
  umschreiben, nicht abschalten. Hier: `myl-local-agent` 2 Zeilen (`gespraech.rs`, `verlauf.rs`).
  `x % n == 0` wird `x.is_multiple_of(n)`, `chunks_exact(N)` wird
  `as_chunks::<N>()`. Für `n ≠ 0` bitgleich; jede Stelle mit variablem
  Teiler ist einzeln angesehen und vorher gegen null geschützt (bei
  `is_multiple_of(0)` stünde `x == 0`, wo `x % 0` abbricht).
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen. Nach dem
  Umschreiben: Clippy mit `-D warnings` still (die Kerne über alle vier
  Merkmale), `cargo test` rc=0 in allen zehn berührten Kisten (die Kerne auch
  mit `cpu-simd`), Konformität mit `sparrow-0.6b-int8` 55/55, die vier
  Abdrücke unverändert (`cb696efd3817e575` über 24 Vektoren), Gleitkomma-Audit
  grün.

### v0.54.0 (2026-10-22): Fund 568 zurückgenommen, die Messschalter entfernt (`myl-local-agent` 0.45.0)

⚑ **Festlegung des Projektinhabers** nach dem Lauf mit drei Varianten
(5.27): Die verankerten Werkzeuge stehen wieder neben `write_file` in
Ansage und Erlaubnis, in jeder Betriebsart.

- Der Grund: In Phase 2 von „Werkzeug bauen“ bestand der Agent mit Fund 568
  7 von 14 Mal, ohne ihn 13 von 14 Mal (exakter Test nach Fisher p = 0,033),
  mit 568 und einer Zeile in der Hausregel 9 von 14 Mal. Ohne 568 liest das
  Modell seine Ergebnisdatei zurück und bessert sie nach, aus „Ozdemir“ wird
  wieder „Özdemir“; mit 568 tat es das in keinem Lauf. Der Preis: Phase 2
  dauert rund ein Viertel länger.
- `verankert::ansagen` und seine Probe entfallen; `Lauf` und
  `gespraech::ansage` sagen wieder an, was der Kasten anbietet.
- Die Hausregel-Zeile kommt nicht; `crate::messschalter` mit
  `MYL_MESSUNG_OHNE_568` und `MYL_MESSUNG_ZUSATZREGEL` ist entfernt.
- **Belege:** `cargo test` rc=0 in `myl-local-agent` und seinen Nutzern
  (`myl-client`, `myl-console`, `myl-oberflaeche`, `myl-testclient`),
  Clippy rc=0.

### v0.53.1 – 2026-10-21 (zwei Messschalter für die Entscheidung zu Fund 568; `myl-local-agent` 0.44.1)

⚑ **Festlegung des Projektinhabers** (5.27): Über Fund 568 wird nach einem
Lauf mit drei Varianten entschieden: mit 568, ohne 568, mit 568 und einer
Zeile in der Regel. Für die beiden letzten gibt es **Messschalter**, keine
Betriebsschalter; sie werden nach der Entscheidung entfernt.

- `crate::messschalter` (neu): `MYL_MESSUNG_OHNE_568` sagt die verankerten
  Werkzeuge wieder neben `write_file` an (wie `NurVerankert`, also auch in
  der Erlaubnis); `MYL_MESSUNG_ZUSATZREGEL` hängt eine Zeile hinter die
  geprüfte Regel, deren Prüfsumme bleibt. Beide stehen im Debug-Log, wenn
  sie greifen.
- 📌 Ein erster Entwurf las die Umgebung in `verankert.rs`, und die Probe
  `nichts_hier_beruehrt_die_aussenwelt` hat ihn zurückgewiesen, zu Recht:
  Die verankerten Werkzeuge müssen nachrechenbar bleiben. Der Schalter
  wohnt jetzt im eigenen Modul und wirkt beim Aufrufer.
- **Belege:** echt am Sparrow mit Debug-Log: mit beiden Schaltern zwei
  Einträge „MESSSCHALTER“, `fill_template` in der Ansage, die Zusatzzeile in
  der Regel; ohne sie nichts davon. `cargo test` (477) und Clippy rc=0.

### v0.53.0 – 2026-10-21 (der Debug-Log bekommt Ablauf und Deckel; `myl-local-agent` 0.44.0)

⚑ **Festlegung des Projektinhabers** (Rest der Entscheidung zum Aktionsprotokoll
vom 2026-10-17): Der Debug-Log enthält alles, also soll er nicht unbegrenzt
liegen bleiben, und ein Lauf, der hängt, soll die Platte nicht füllen.

- **Ablauf:** Dateien aus Monaten vor den letzten drei gehen, der laufende
  mitgezählt (`AUFBEWAHREN_MONATE`), auch über den Jahreswechsel.
- **Deckel:** Der Ordner hält höchstens 512 MiB (`ORDNER_GRENZE`); darüber
  gehen die ältesten Teile zuerst. Ab 64 MiB (`TEIL_GRENZE`) schreibt der
  Monat in einen neuen Teil (`debug-JJJJ-MM-2.log`), damit nie ein ganzer
  Monat auf einmal gehen muss. Die Datei, in die gerade geschrieben wird,
  bleibt immer; fremde Dateien im Ordner bleiben.
- Geprüft wird beim Einschalten und bei jedem neuen Teil, nicht bei jedem
  Eintrag. `datei_fuer` nennt den laufenden Teil; die Konsole zeigt ihn bei
  `/debug`.
- **Gemessen vor der Grenze:** 3,5 MB in neun Tagen Entwicklung (203 Starts,
  3 818 Einträge, der größte 38 KB). Im Alltag greift also der Ablauf, der
  Deckel nur bei einem Ausreißer.
- **Belege:** Proben `ablauf_und_deckel` (drei Monate bleiben, älteste Teile
  zuerst, laufende Datei bleibt, voller Teil beginnt einen neuen, fremde
  Datei bleibt) und `ablauf_ueber_den_jahreswechsel`; `cargo test` und
  Clippy mit `-D warnings` für `myl-local-agent` und `myl-console` rc=0.

### v0.52.2 – 2026-10-21 (die Modelle heißen nach Tieren; `myl-local-agent` 0.43.2)

Die Projektmodelle heißen seit heute nach Tieren, mit Größe und Speicherform als Anhängsel (Festlegung des Projektinhabers): `myelith-0.6b` ist `sparrow-0.6b-int8`, `myelith-4b` `pigeon-4b-int8`, `myelith-30b-a3b` `magpie-30b-a3b-int8`, `myelith-35b-a3b` `raven-35b-a3b-int8`. In den Dateien eines Artefakts steht kein Name, also bleiben Abdrücke und Register gültig. Nur Proben und Beispiele.

### v0.52.1 – 2026-10-21 (der Ort der Einstellungen nimmt die Umgebung als Argument; Fund 579; `myl-local-agent` 0.43.1)

- ⛔️ **Fund 579: Im Repositorium lag `CLIENT/myl-client/.config/myelith/wurzel`**
  mit dem absoluten Pfad des Entwicklungsrechners, eingecheckt seit
  `d63e477` und seither bei Prüfläufen neu geschrieben. Die Probe der
  Plattformorte in `myl-client` stellte `HOME` und `XDG_CONFIG_HOME`
  prozessweit um; fehlte `HOME` gerade, legte ein nebenlaeufiger Testfaden
  seine Ablage im Arbeitsverzeichnis an, und das ist bei `cargo test` der
  Ordner der Kiste. Dort lag auch ein `verlauf.key` (von `*.key`
  ausgenommen, nie eingecheckt). Auf einem anderen Rechner verwarf
  `ort::wurzel()` den Zettel, weil am genannten Ort keine Wurzel liegt; er
  schadete also nicht, verriet aber Pfad und Nutzernamen.
- **Jetzt:** `ort::plattformpfad_aus` nimmt die Umgebung als Argument,
  `plattformpfad` reicht die des Prozesses hinein. Die Probe (CLIENT
  v0.173.1) prüft die reine Form und ändert die Umgebung nicht mehr.
  `.gitignore` nimmt `*/*/.config/` aus.
- **Belege:** `cargo test --lib ort::` (8) und die Probe in `myl-client`
  grün; Gegenprobe: liest die Funktion `HOME` wieder aus dem Prozess, fällt
  die Probe.
- **Die Proben der Einrichtungsskripte** (`ort.rs`) lesen auch
  `installieren-linux.sh`; der Menüeintrag muss das Symbol beim Namen nennen
  und darf nicht in den Baum zeigen (CLIENT v0.174.0, Fund 580).
- **Die Agentenmessung** (`BENCHMARKS/Agent/agentenprobe.py`) schreibt den
  Pfad des Artefakts relativ zur Wurzel ins Ergebnis; bisher stand dort der
  absolute des Messrechners samt Nutzername, in vierzehn versionierten
  Ergebnissen, die jetzt umgeschrieben sind (Fund 580).

### v0.52.0 – 2026-10-19 (der eigene Mitschnitt sperrt die Shell nicht mehr, fremd ist nur ein Mitschnitt mit Fremdem; Fund 570; `myl-local-agent` 0.43.0)

Anlass: der Prüflauf nach v0.51.0, „Werkzeug bauen“ mit dem int4-35B. Das
Modell rief zu Beginn `read_history` und bekam „noch kein Mitschnitt“. Weil
der Vertrag `ERINNERN` jedes Ergebnis fremd machte, war `run_command` danach
für den ganzen Lauf vom Informationsfluss gesperrt; das Modell konnte sein
Skript nicht ausführen und rechnete Prüfsummen von Hand.

- **Fund 570:** Der Mitschnitt galt als Ganzes fremd, weil er frühere
  Webseiten tragen kann, auch wenn er keine trug und auch ohne jeden Inhalt.
- **Festlegung des Projektinhabers vom 2026-10-19:** Fremd ist, was aus einer
  Sitzung stammt, die Fremdes gesehen hatte. `verlauf::schreiben` schreibt
  dazu die Zeile `Herkunft: vertraut` oder `Herkunft: fremd` in den Kopf,
  gelesen mit `secure_flow::label_aus_verlauf`, also aus den Fremdmarken im
  Verlauf selbst, die auch Ausblenden, Kürzen und Zusammenfassen überleben
  (dieselbe Lesart wie über die Runden eines Gesprächs, Fund 536).
- **Neuer Vertrag `MITSCHNITT`** (privat, vertraut) für `read_history`,
  `list_history` und `search_history`. Die Werkzeuge setzen die Fremdmarke
  selbst vorn (`herkunft_vorn`), sobald einer der gelesenen Mitschnitte nicht
  `verlauf::nur_vertrautes` ist. ⛔️ **Vertraut nur, was es ausdrücklich
  sagt:** Ein Mitschnitt ohne Herkunftszeile (älter als dieser Fund), mit
  einer Fremdmarke irgendwo im Text oder unlesbar gilt weiter als fremd.
  Eine Auskunft ohne gelesenen Mitschnitt bleibt vertraut. `search_memory`
  (die Erinnerungen des Schlafs) behält `ERINNERN`.
- ⚠️ **Was ein fremder Mitschnitt nicht mehr tut:** nichts. Er macht das
  Gespräch fremd wie vorher, und danach braucht die Shell die Freigabe. Eine
  Wäsche über eine selbst geschriebene Herkunftszeile scheitert an zwei
  Stellen: `.AGENT/` ist ein verborgener Pfad (nach Fremdem nur mit
  Freigabe beschreibbar), und eine Fremdmarke im Text überstimmt die Zeile.
- Der Kopf eines Mitschnitts hat damit acht statt sieben Zeilen; das
  Verzeichnis wird weiter beim Schreiben gegen die Datei geprüft, und ältere
  Mitschnitte lesen sich wie bisher.
- **Beleg:** `nur_ein_mitschnitt_mit_fremdem_macht_das_gespraech_fremd`
  (fünf Fälle durch `Bewacht` und den Fluss: kein Mitschnitt, eigener
  Verlauf mit gelesener Datei, Webseite im Verlauf, alter Kopf ohne Zeile,
  Marke mitten im Auftrag; jedes Mal folgt die Shell dem Label). `cargo test`
  von `myl-local-agent`, `myl-client`, `myl-console`, `myl-oberflaeche` und
  `myl-testclient` mit Rückgabe 0, Clippy mit `-D warnings` still. Cargo.lock
  in `local-agent`, `myl-client`, `myl-console`, `myl-oberflaeche` und
  `myl-testclient` nachgezogen.

### v0.51.0 – 2026-10-19 (Werkzeugaufrufe nachsichtiger gelesen, verankerte Werkzeuge neben `write_file` ausgeblendet, abgerissene Aufrufe weitergeschrieben, Antwortlänge mit dem Kontext; Funde 566 bis 569; `myl-local-agent` 0.42.0)

Anlass: die Agentenläufe des int4-35B vom 2026-10-17 bis 2026-10-19 (Stufen,
Extrem, Szenarien). In allen Mitschriften zusammen wurde `write_file` 155 Mal
gerufen und 37 Mal mit „das Feld `felder` steht nicht im Schema“ abgewiesen;
dazu kamen 15 unlesbare Aufrufe, deren Sinn eindeutig war, und 8 ohne
`</tool_call>`. Jede Absage kostet eine ganze Antwort, in der derselbe Inhalt
noch einmal entsteht.

- **Fund 566 (Parser, `werkzeug::aus_dem_wert`):** drei weitere Formen mit
  genau einer Lesart. `{"type": "search_skill", "arguments": …}` (sechsmal,
  kostete einen Web-Lauf): `type` trägt den Namen, wenn `name` fehlt, und
  `"function"` ist nie ein Name. `{"name": {"name": "read_file", "arguments":
  …}}` (viermal): die doppelte Hülle wird ausgepackt, wenn `name` das einzige
  Feld ist. `{"name": "write_file", "pfad": …, "inhalt": …}`: Fehlen
  `arguments` und `parameters`, sind die übrigen Schlüssel die Argumente,
  ohne die Rahmenfelder `id` und `type`. ⚠️ Tippfehler in Schlüsseln
  (`"arguments>{`, ein fehlendes Anführungszeichen) bleiben unlesbar: Sie zu
  reparieren hieße raten.
- **Fund 567 (Hülle, `werkzeug::huelle_aufloesen`):** Die Hülle kam selten
  allein. `{"felder": {}, "pfad": …, "inhalt": …}` und `{"felder":
  {"inhalt": …}, "pfad": …}` werden jetzt aufgelöst: Ein leeres fremdes Feld
  (`{}`, `[]`, `""`, `null`) fällt weg, höchstens eine Hülle aus lauter
  Schemafeldern ergänzt, was außen fehlt, und das Ergebnis muss das Schema
  vollständig erfüllen. Zwei Werte für dasselbe Feld, zwei Hüllen und ein
  fremdes Feld mit Text (`"vorlage": "…"` ohne `pfad`) bleiben Fehler.
- **Fund 568 (Werkzeugsatz, `verankert::ansagen`):** `fill_template` und
  `join_sections` wurden in keinem Lauf mit Arbeitsordner gerufen, aber ihre
  Felder landeten in `write_file`. Neben einem laufenden Schreibwerkzeug
  stehen sie nicht mehr in der Ansage und nicht in der Erlaubnis; im Chat
  ohne Ordner, bei abgeschaltetem Schreiben und in `NurVerankert` bleiben
  sie. Die Kontextanzeige (`gespraech::ansage`) zählt dieselbe Liste.
  `abgeschaltet::ist_platzhalter` ersetzt den Präfixvergleich in `lauf.rs`.
- **Fund 569 (abgerissene Aufrufe, `schleife`):** Endete eine Antwort an der
  Länge mitten in einem `<tool_call>`, war der Aufruf verloren, und die
  Meldung sagte „vermutlich abgeschnitten, fass dich kürzer“, auch wenn die
  Antwort von selbst geendet hatte. Jetzt schreibt die Schleife sie weiter
  (`Modellweg::weiterschreiben`, höchstens `FORTSETZUNGEN_HOECHSTENS` = 2
  Mal, nur mit Platz für eine weitere ganze Länge), und erst danach wird
  gelesen. Die Meldung sagt den wirklichen Grund: an der Grenze auch nach
  den Fortsetzungen (dann aufteilen, mit den Werkzeugnamen der Ansageform)
  oder vor der Endmarke geendet. Ein Weg, der nicht weiterschreiben kann
  (die Tür eines Knotens), liefert die Antwort unverändert.
- **Antwortlänge (Festlegung des Projektinhabers vom 2026-10-19):**
  `agentenwahl::agent_antwortlaenge` gibt einem Agentenlauf ein Achtel der
  Kontextgrenze, mindestens 4 000 und von sich aus höchstens 8 192
  (`AGENT_ANTWORT_HOECHSTENS_NACH_KONTEXT`); eine höhere Einstellung gilt.
  Bei 65 536 Kontext sind das 8 192 statt 4 000. Die Länge ist auch Platz,
  den jeder Schritt freihält, deshalb wächst sie mit dem Kontext und nicht
  fest; was darüber hinausgeht, schreibt die Schleife weiter.
- **Belege:** `type_statt_name_doppelte_huelle_und_offene_argumente_werden_gelesen`,
  `klasse_2_eine_teilhuelle_und_leere_fremde_felder_werden_aufgeloest` (mit
  fünf Gegenproben), `die_verankerten_weichen_nur_einem_laufenden_schreibwerkzeug`,
  `ein_abgerissener_aufruf_wird_weitergeschrieben`,
  `weitergeschrieben_wird_begrenzt_und_die_meldung_sagt_den_grund`,
  `die_antwortlaenge_waechst_mit_dem_kontext`. Zwei ältere Proben angepasst:
  `nach_einer_ablehnung_darf_das_modell_es_richtig_machen` lehnt jetzt mit
  einem fremden Feld mit Inhalt ab (ein leeres fällt weg), und
  `ein_abgeschnittener_aufruf_wird_beantwortet` prüft „endet ohne
  `</tool_call>`“ statt „abgeschnitten“. `cargo test` von `myl-local-agent`,
  `myl-client`, `myl-console` und `myl-testclient` mit Rückgabe 0, Clippy mit
  `-D warnings` dazu für `myl-oberflaeche`.

### v0.50.0 – 2026-10-19 (ein verdichtender Lauf legt seine Urfassung als Mitschnitt ab; Fund 565; `myl-local-agent` 0.41.0)

Anlass: die zweite Phase des Szenarios „Nadel im Heuhaufen“. Ein frischer
Lauf sollte eine Vorgangsnummer nennen, die nur noch in der früheren
Sitzung stand. Er suchte richtig (`search_history` mit dem genauen
Zeitstempel, dann `read_history`, `list_history`) und bekam „Nothing has
been recorded in this folder yet“, obwohl die erste Phase zweimal verdichtet
hatte.

- **Fund 565:** Den Mitschnitt schrieb nur die Verdichtung des Gesprächs
  (`gespraech::verdichten_mit_mitschnitt`). Verdichtete ein Agentenlauf in
  sich (`schleife::platz_schaffen`: ausblenden, zusammenfassen, kürzen), war
  die Urfassung weg. `read_history` sagte dem Modell trotzdem zu, die
  Einzelheiten „nach der Zusammenfassung“ nachzulesen.
- **Behebung:** `Lauf::mitschnitt` (Arbeitsordner und Kennung des Laufs).
  Bevor `platz_schaffen` etwas wegnimmt, legt es den ganzen Verlauf ohne
  Werkzeugansage unter `.AGENT/<kennung>/` ab, mit demselben Schreiber und
  demselben Verweis wie das Gespräch (ohne `read_history` in der Kiste der
  ehrliche Verweis). Der Verweis steht in der Zusammenfassung, oder, wenn
  es keine gibt, an der letzten Werkzeugantwort, **nach** dem Kürzen und
  mit freigehaltenem Platz. Ein Mitschnitt, der nicht geschrieben werden
  kann, hält die Verdichtung nicht auf; er steht im Debug-Log.
- `lauf::fahren_mit_hausregel` (alle Agentenläufe) setzt den Ort aus der
  Einhängung; die Kennung ist `lauf-<sekunden>-<prozess>-<nummer>`, damit
  Unteragenten eines Prozesses sich nicht überschreiben.
- **Beleg:** `ein_verdichtender_lauf_legt_die_urfassung_als_mitschnitt_ab`
  (fünf Antworten zu je 2000 Zeichen werden gekürzt; im Mitschnitt stehen
  alle fünf ungekürzt samt Auftrag; das Modell sieht den Verweis). Alle
  übrigen Aufbauten des Laufs bekommen `mitschnitt: None`. `cargo test` von
  `local-agent` und `myl-testclient` grün, Clippy still.

**Ältere Einträge** (v0.49.2 und davor, 54 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- AGENT_LAYER/README/README.md`.
