# agent-layer

> **Version:** 0.66.0 (`myl-agent` 0.10.0, `myl-local-agent` 0.50.0, `book2skill` 0.7.0)
> **Datum:** 2026-11-13
> **Status:** Manifeste, Herkunftsstufe, Registratur, der
> **Session-Kontrakt** mit Durchsetzung im Ledger, der **Plan** und seit
> v0.7.0 die **Segmentkette**, seit v0.8.0 die Herkunft im
> **Gedächtnis**. 59 Tests. ⚑ **Was jetzt fehlt, ist keine
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
- `src/gedaechtnis.rs`: was in den dauerhaften Speicher darf, nämlich nur,
  was die Ausgabe eines Schritts einer geltenden Kette ist.

## Changelog

### v0.66.0 (2026-11-13): Der Türklient liest eine Antwort nur bis zu einer Grenze (`myl-local-agent` 0.50.0)

- 📌 **Befund:** `Tuerklient::lesen` sammelte Köpfe und Rumpf, solange
  etwas kam. Mit dem Schalter „Netz“ ist die Gegenstelle ein fremder
  Knoten; eine, die nie eine Leerzeile schickt oder einen endlosen Rumpf,
  ließ den Speicher wachsen, und die Suche nach der Leerzeile lief jedes
  Mal über alles. Jetzt Köpfe bis `KOPF_HOECHSTENS` (64 KiB, sonst „kein
  HTTP“), Rumpf bis `RUMPF_HOECHSTENS` (64 MiB, angekündigt oder
  gelesen, sonst „unlesbar“ mit Grund).
- Probe `eine_gegenstelle_ohne_ende_fuellt_nicht_den_speicher` in
  `tests/tuerklient.rs` (drei Fälle, der Stummel hält danach offen, der
  Klient hat 1 s Frist); Gegenprobe mit abgeschalteten Prüfungen: Frist
  statt „kein HTTP“, rot.

### v0.65.0 (2026-11-13): Eine hängende Seite hält das Buch nicht fest (`book2skill` 0.7.0)

- `myl scan` endet mit 3, wenn eine Seite länger rendert als erlaubt, und
  legt sie als unlesbar ab. Book2Skill ruft dann für die übrigen Seiten neu
  auf, schreibt „[Seite nicht lesbar]“ an ihre Stelle und führt sie im
  Manifest unter `scan.seiten_unlesbar`; `seiten_erkannt` zählt sie nicht
  mit. Meldet `myl scan` 3, ohne eine Seite abzulegen, bricht Book2Skill ab
  statt ohne Ende neu zu rufen. `myl scan --seitenzahl` hat eine Frist von
  300 s.
- Probe `scan_haengt` (Fortsetzung, Text, Manifest, Schutz gegen endloses
  Neurufen); Gegenprobe, die den Rückgabewert übergeht, rot; grün unter
  Python 3.9 und 3.12. Von Ende zu Ende mit dem echten `myl` und einem
  Scan in altgriechischer Schrift: erster Aufruf 3, zweiter liefert Seite 2
  und 3, im Buch einmal „[Seite nicht lesbar]“, im Manifest
  `seiten_unlesbar: [1]`, `seiten_erkannt: 2`.

### v0.64.5 (2026-11-12): Ein verstümmeltes Werk endet sauber (`book2skill` 0.6.3)

- Neue Probe `verstuemmelt_endet_sauber`: 1 200 Abwandlungen von EPUB,
  DOCX, ODT, FB2, RTF und HTML (Inhalt der Archive verändert, Teile
  entfernt, Zeichen eingefügt, gekürzt) werden gelesen oder mit `Fehler`
  abgewiesen, nie mit einer anderen Ausnahme. Vorher erkundet mit 30 000
  Abwandlungen.
- 📌 **Befund dabei:** Ein Buchstabe außerhalb von ASCII nach `\` (etwa
  `\ß`) galt im RTF als Steuerwort, das Muster passte nicht, und
  `None.group` brach mit `AttributeError` ab. Steuerwörter bestehen nur aus
  ASCII-Buchstaben; jetzt ist es ein Steuersymbol und fällt weg.
  Gegenprobe ohne die Prüfung rot; grün unter Python 3.9 und 3.12.

### v0.64.4 (2026-11-12): Emoji in RTF brechen Book2Skill nicht mehr ab (`book2skill` 0.6.2)

- 📌 **Befund:** RTF schreibt Zeichen außerhalb der BMP als zwei
  UTF-16-Hälften (`\uN`); `rtf_nach_text` setzte sie einzeln ein, und das
  Schreiben als UTF-8 scheiterte (`UnicodeEncodeError`), ein Buch mit einem
  Emoji brach ab. Eine Unsinnszahl wie `\u9999999` brach mit `ValueError`
  ab. Jetzt werden zwei Hälften ein Zeichen; was allein steht oder außerhalb
  von Unicode liegt, wird U+FFFD.
- Probe `rtf_unicode` (unter Python 3.9 und 3.12 grün; Gegenprobe mit dem
  alten Einsetzen rot).

### v0.64.3 (2026-11-12): Eigene XML-Entitäten werden überall abgewiesen (`book2skill` 0.6.1)

- 📌 **Befund:** `xml_lesen` (EPUB, DOCX, ODT, FB2) wies `<!ENTITY` nur in
  den ersten 64 KB ab. Ein Kommentar von 70 KB davor oder UTF-16 umging das,
  und das expat des Systems (2.2.8 bei Python 3.9 unter macOS) schützt nicht
  selbst: Neun Ebenen wurden ein Gigabyte Text, ein präpariertes Buch hätte
  den Speicher gesprengt. Jetzt läuft expat vorab mit einem
  `EntityDeclHandler`, der bei der ersten Deklaration abbricht, bevor etwas
  expandiert wird, gleich an welcher Stelle und in welcher Kodierung.
- Probe `xml_ohne_entitaeten` (direkt, hinter 70 KB Kommentar, UTF-16; dazu
  gewöhnliches XML mit `&amp;` und `&lt;`), grün unter Python 3.9 und 3.12;
  Gegenprobe mit der alten Prüfung rot. Andere Python-Werkzeuge lesen kein XML.

### v0.64.2 (2026-11-12): Ein Fuzzer für die Leser der Web-Recherche (`myl-local-agent` 0.49.2)

- Neu `local-agent/fuzz/` mit dem Ziel `html` (Rumpf in
  `tests/fuzzziele/mod.rs`, stabiler Lauf `tests/fuzzkorpus.rs`): Jede
  Seite geht ohne Panik durch `text_aus_html`, `treffer_aus`,
  `verweise_aus` und `adressen_aus`, der Text wird nicht länger als die
  Seite, Treffer und Verweise halten ihre Obergrenzen. Ungültiges UTF-8
  wird verlustbehaftet gelesen, wie eine Seite aus dem Netz.
- Lauf 600 s mit Seiten bis 200 KB und 2 s Zeitgrenze je Seite: 4,1
  Millionen ohne Fund. 📌 Die quadratische Suche aus 0.64.1 fand er wieder
  eingebaut in 180 s nicht: Sie braucht eine gezielt gebaute Seite, dafür
  steht die eigene Probe. Drei Saaten (Trefferseite, Artikel, Kaputtes).

### v0.64.1 (2026-11-12): Eine Seite voller `&` hält die Web-Recherche nicht mehr fest (`myl-local-agent` 0.49.1)

- 📌 **Befund:** `entitaeten` in `netzwerkzeuge` suchte für jedes `&` das
  nächste `;` auf der ganzen Seite und verwarf erst danach, was weiter als
  zehn Zeichen entfernt lag. Eine Seite aus einer Million `&` brauchte 12 s,
  bei der erlaubten Seitengröße von 16 MB wäre es fast eine Stunde gewesen.
  Jetzt wird nur in den nächsten elf Bytes gesucht; das Ergebnis ist
  dasselbe.
- Probe `viele_kaufmannsund_kosten_keine_quadratische_zeit` (vorher 12 s,
  rot; jetzt unter einer Sekunde). `myl-local-agent` 503 grün, Clippy
  sauber.

### v0.64.0 (2026-11-12): Der Hash jedes erzeugten Markdowns steht im Manifest (`book2skill` 0.6.0)

- `manifest.json` nennt jetzt neben dem Hash der Quelle `sha256_markdown`
  (je erzeugter Markdown-Datei im Buchordner) und `sha256_skill`, damit eine
  spätere Verankerung auf genau diese Bytes zeigen kann. Die Probe prüft jede
  Summe gegen die Datei; gleiche Eingabe gibt weiter gleiche Bytes, also
  auch dasselbe Manifest. Gegenprobe (nur `seiten/` gehasht): rot.
- Damit sind 0.3 (PDF, EPUB, Office nach Markdown, mit eigenen Lesern statt
  Docling, byteweise wiederholbar) und 0.4 belegt.

### v0.63.0 (2026-11-11): Der Abruf aus dem Gedächtnis (`myl-agent` 0.10.0)

- **Neues Modul `abruf`:** `abrufen(gedaechtnis, text, anfrage, anzahl)`,
  lexikalisch und in ganzen Zahlen. Wörter sind Bytefolgen zwischen
  ASCII-Trennern, Bytes ab `0x80` gehören zum Wort, nur ASCII wird klein
  geschrieben (eine Unicode-Faltung hinge an der Werkzeugkette), Wörter
  unter zwei Bytes zählen nicht. Wertung: verschiedene Wörter der Anfrage
  im Eintrag; Rangfolge (Wertung fallend, Stelle steigend) über alle
  Einträge, ohne Näherungsindex.
- **Der Text muss zum Commitment passen:** Der Speicher hält nur das
  Ausgabe-Commitment (SHA-256), der Aufrufer liefert den Text; ein Text,
  der nicht passt oder fehlt, wird nicht gewertet und steht in
  `ohne_text`.
- 📌 **Fund 695 ganz geschlossen:** Ein Abruf ist getrübt, sobald einer
  seiner Treffer es ist. Kritisch geprüft, ob ein getrübter Eintrag, der
  nicht zurückkommt, das Ergebnis lenken kann: Die Auswahl ist die Spitze
  einer festen Rangfolge, ein Eintrag darunter ändert sie nicht.
- Vier Proben (Rangfolge, Erbe der Trübung samt Gegenfall unter der
  Spitze und geerbter Trübung, passender Text, feste Zerlegung); zwei
  Gegenproben rot (Erbregel aus, Textprüfung aus). 64 Proben grün, Clippy
  sauber.
- ⚠️ Noch nicht als Werkzeug in der Registratur und nicht im Plan; Sprachen
  ohne Wortzwischenraum zerfallen nicht in Wörter.
- `myl-agent` 0.10.0.

### v0.62.0 (2026-11-11): Der Gedächtniseintrag trägt seine Trübung (`myl-agent` 0.9.0)

- 📌 **Fund 695 geschlossen, im Format:** Ein deterministisches
  Abrufwerkzeug galt in einem späteren Plan als rein, auch wenn der
  Eintrag aus einem Schritt kam, der die Welt befragt; der Abruf hat den
  Plan des Eintrags nicht zur Hand. Der Eintrag trägt jetzt das Feld
  `truebung`, die Aufnahme (`pruefe_herkunft`, `Gedaechtnis::nimm`, beide
  neu mit der Registratur) verlangt, dass es dem Wert gleicht, den sein
  Plan an dieser Stelle ergibt, und die Wurzel bindet es. Neuer Fehler
  `TruebungStimmtNicht`.
- ⚠️ Die zweite Hälfte gehört zum Abruf, der noch nicht gebaut ist: Ein
  Schritt, der Einträge zurückgibt, ist getrübt, sobald einer von ihnen es
  ist. Der Modulkopf nennt die Regel.
- Neue Probe `die_truebung_im_eintrag_wird_bei_der_aufnahme_geprueft`:
  ein getrübter Eintrag mit dem Feld auf rein, ein reiner mit dem Feld
  auf getrübt, beide abgewiesen; der echte kommt hinein, und ein
  nachträglich geändertes Feld besteht den Abruf gegen die Wurzel nicht.
  Gegenprobe ohne die Prüfung des Feldes: rot.
- `Truebung` ist jetzt kodierbar (Borsh). Das Format ändert sich, ehe es
  ausgeliefert ist; später wäre es eine neue Formatfassung.
- `myl-agent` 0.9.0.

### v0.61.0 (2026-11-10): Das Gedächtnis nimmt nur auf, was sich auf einen Schritt zurückführen lässt (`myl-agent` 0.8.0)

- **Neues Modul `gedaechtnis`**, die Herkunftshälfte von Whitepaper Kap. 8.4: Ein Eintrag nennt Session, Plan, Stelle und Kettenglied. `pruefe_herkunft` rechnet gegen die Segmentkette nach, dass er die Ausgabe genau dieses Schritts ist.
- **Der Speicher trägt Commitments, keine Inhalte**, wie der verankerte Korpus aus Kap. 7.3. Über alle Einträge steht eine Merkle-Wurzel; `pruefe_abruf` weist nach, dass ein Eintrag an seiner Stelle dazugehört. Hinein führt ein einziger Weg, und der prüft; der Typ lässt sich nicht aus Bytes lesen.
- **Trübung überlebt den Speicher:** Sie wird nicht gespeichert, sondern aus dem Plan abgeleitet, auf den der Eintrag zeigt.
- ⚠️ **Nicht gebaut:** der Abruf (er muss deterministisch sein, und wie, ist nicht entschieden) und der Betrieb als eigene Arbeitsklasse im Netz. Ob die Segmente einer Kette verifiziert sind, sagt weiterhin VERIFICATION und nicht dieses Modul.
- ⚠️ **Zwei Grenzen, beide im Modul benannt:** Aus einer Session, die vor dem Ende ihres Plans abbricht, lässt sich kein Eintrag belegen, weil der Kettenwert nur über eine vollständige Kette definiert ist. Und ein deterministisches Abrufwerkzeug gälte in einem späteren Plan als rein, auch wenn sein Eintrag getrübt hineinging; die Ableitung macht das prüfbar, sie schließt es nicht.
- Belege: 7 neue Tests (59 gesamt), Clippy grün. Vier Gegenproben, jede rot: Glied nicht verglichen, Kette nicht geprüft, Trübung ohne Planvergleich, Abruf ohne Stelle.

### v0.60.1 (2026-11-10): Loop-Erkennung am echten Modell gemessen, die Schwellen bleiben

- **Hinweis ab dem dritten, Ende beim fünften gleichen Schritt: bestätigt.** Am 8B, zehn Auftragsarten, die zum Wiederholen einladen oder es verlangen, drei Saaten: Der Hinweis fiel in 9 von 30 Läufen. In 6 davon kam danach sofort die Antwort (warten auf etwas, das nie kommt; drei verlangte gleiche Lesungen).
- **Kein Fehlalarm:** In den übrigen sieben Auftragsarten fiel der Hinweis nie. Eine zweite gleiche Wiederholung kam in gewöhnlichen Läufen vor und endete gut; eine Schwelle von zwei wäre zu früh.
- **Verlangt der Auftrag sechs gleiche Lesungen, endet der Lauf beim fünften Schritt ohne Antwort**, in 3 von 3 Saaten. Das Ende ist benannt (`Kreislauf`). ⚠️ Eine schärfere letzte Warnung („beim nächsten Mal endet der Lauf“) änderte daran in 3 von 3 Saaten nichts und ist verworfen; der Hinweis bleibt bei einem Wortlaut.
- **Was die Erkennung mit Absicht nicht sieht:** Eine Suche nach etwas, das im Buch fehlt, lief am 8B bis zu neunmal mit abgewandelten Mustern (bis 164 s) und endete von selbst richtig. Kein Schritt war gleich.
- Am 30B, dieselben zehn Auftragsarten, zwei Saaten: 20 von 20 Läufen ohne Wiederholung. Es setzt drei verlangte Lesungen als drei Aufrufe in einem Schritt ab.
- ⚠️ Am 30B dabei gesehen, kein Schleifenthema: Fehlt das zu ersetzende Wort in der Datei, ersetzte es in 2 von 2 Läufen ein anderes und meldete Erfolg (das 8B meldete in 3 von 3 richtig, dass das Wort fehlt). Und es kündigte sechs Lesungen an und endete ohne eine einzige (2 von 2).
- Geändert ist nur ein Kommentar; `myl-local-agent` bleibt bei 0.49.0.

### v0.60.0 (2026-11-10): Der Agent weiß von gelesenen Büchern und findet die Stelle (`myl-local-agent` 0.49.0)

- **Fester Buchhinweis hinter dem Auftrag**, sobald `literature/` ein Buch enthält: wie viele es sind, womit man darin sucht, und dass `seiten/<n>.md` die Seite n ist. Er ist für jeden Auftrag derselbe und trägt kein Zeichen aus einem Buch.
- **`search_files` zählt Wörter, wenn der Wortlaut fehlt:** Es zeigt die Zeilen mit den meisten Wörtern des Musters; eine Zahl im Muster muss in der Zeile stehen.
- **Eine Zahl als Muster sucht die ganze Zahl.** `30` trifft weder `1307` noch eine Prüfsumme. Teilfunde kommen nur, wenn es die ganze Zahl nirgends gibt.
- **Im Buch zählt der Buchtext.** Der Seitenkopf, `buch.md` und das Manifest sind keine Treffer; eine Seite ist nur dann ein Namenstreffer, wenn das Muster sie beim ganzen Namen nennt.
- Gemessen am 8B, 16 Fragen zu einem 40-seitigen Buch, drei Saaten: 48 von 48 richtig, vorher 23 von 48. Kein Lauf mehr ohne Werkzeugaufruf (vorher 11), Median 23 s. ⚠️ Die Fehler dieser 16 Fragen haben die Änderungen gelenkt; das unabhängige Maß ist die Haltemenge: 16 andere Fragen, zwei Saaten, 31 von 32 richtig, Median 21 s. Der eine Fehler ist ein Lesefehler des Modells, die Suche zeigte die richtige Zeile zuerst.
- Gegenprobe mit 9 gewöhnlichen Aufträgen: mit und ohne Bibliothek je 8 von 9, kein Gang ins Buch, kein Buchtext im Gespräch.
- Verworfen: ein Hinweis, der die Worte des Auftrags gegen die Bücher hält und ein Suchmuster vorschlägt. Bei 57 von 68 gewöhnlichen Aufträgen schlug er an.
- 🐛 Ein Namenstreffer aus einem Buchordner kam ohne Herkunftsmarke zurück, obwohl der Ordner nach dem Titel heißt.
- 🐛 Zwei Proben der Syntaxwache teilten sich einen Dateinamen; liefen sie gleichzeitig, fiel eine davon rot aus.

### v0.59.1 (2026-11-10): Nachschlagen in gelesenen Büchern (`myl-local-agent` 0.48.1, `book2skill` 0.5.1)

- 🐛 `read_file` gibt Binärdateien nicht mehr als Text zurück. Ein gescanntes PDF brachte 64 KB Zeichensalat in den Verlauf, und die Antwort lief in die Zeitgrenze.
- `learn_skill` sagt bei einem Buch-Skill zuletzt, dass es nur eine Zusammenfassung ist und wie man nachschlägt. `book_to_skill` verweist bei einem erfundenen Pfad auf die gelesenen Bücher.
- Gemessen an 16 Fragen zu einem 40-seitigen Buch: 8B von 4 auf 8 richtig, 35B von 14 auf 16.
- 🐛 Book2Skill: Mehrere Sätze gelten nicht mehr als Titel. Die Satzauswahl ohne Modell verwarf jede Schrift ohne Großbuchstaben.

### v0.59.0 (2026-11-10): Book2Skill übersetzt mit einem eigenen kleinen Modell und einer Weiche nach Sprache (`book2skill` 0.5.0)

- **`--uebersetzer ARTEFAKT`:** Das kleine Übersetzungsmodell macht die Arbeit; was ihm misslingt, bekommt das große Modell. Ohne großes Modell übersetzt es trotzdem.
- **Weiche nach Sprache** (`uebersetzungsweg`, an echten Scans belegt): Latein, Altgriechisch, Sanskrit und übriges Devanagari ohne Hindi, Assamesisch, Thai, Tamil, Telugu und Tibetisch gehen an das große Modell. Syrisch wird nicht übersetzt, weil kein gemessenes Modell den Sinn trifft; das Manifest sagt es.
- 🐛 Erfundene Marken wie `<assistant>` am Anfang einer Antwort fallen weg; Überlegungen statt einer Übersetzung gelten als misslungen.

### v0.58.0 (2026-11-09): Book2Skill übersetzt das Werk (`book2skill` 0.4.0, `myl-local-agent` 0.48.0)

- **Das Werk liegt auch in der Sprache des Nutzers vor.** Ist es nicht schon in dieser
  Sprache, übersetzt das Modell Seite für Seite nach `uebersetzung/<sprache>/` und der
  Skill entsteht aus der Übersetzung. Sein Verweis nennt die übersetzte Seite und das
  Original. Eine Seite, die misslingt (leer, Schleife, Wiederholung des Originals), bleibt im
  Original und ist gekennzeichnet; die Zahl steht im Manifest.
- 🐛 **Ein abgebrochener Lauf setzt jetzt wirklich fort.** Der fertige Teil des Buchs
  galt bisher als „gibt es schon“; erst Skill und Manifest machen ein Buch nun fertig.
- **Eine leere Skillsuche weist auf gelesene Bücher hin.** Ein 8B fragte bei wörtlichen
  Stellen immer wieder `search_skill` und gab auf.
- ⚠️ **Grenze:** Die Übersetzung wächst mit den Seiten und nicht mit den Kapiteln. Ohne
  Modell bleibt der Skill in der Sprache des Werks, das Manifest sagt es.

### v0.57.1 (2026-11-09): Book2Skill sucht den Titel auf acht Seiten

- 🐛 `book2skill` 0.3.1 sucht den Titel nun auf den ersten acht Seiten statt auf drei,
  denn alte Bücher beginnen mit Vorsatzblättern und Stempeln. Satzzeichen am Rand des
  Titels fallen weg.

### v0.57.0 (2026-11-09): Book2Skill liest Scans und wertet Abbildungen aus

- ⚑ **Gescannte PDFs:** Seiten ohne Textebene liest `myl scan` (CLIENT, `myl-scan`):
  Seitenaufbau, Lesereihenfolge über Spalten, Tabellen, Abbildungen als Ausschnitt, Kopf-
  und Fußzeilen bleiben draußen. Keine Abhängigkeit außer `myl`, kein Netz, auf allen
  Systemen derselbe Weg.
  Sechzehn Schriften samt Fraktur, Altgriechisch, Hebräisch und senkrechtem Satz; die
  Schrift eines Werks wird erkannt oder mit `--schrift` genannt.
- ⚑ **Rechenaufwand:** rund eine Sekunde je Seite auf der CPU, und nur für Seiten, die es
  brauchen: Seiten mit Textebene bleiben beim schnellen Weg. Abbrüche setzen seitenweise
  fort. Ein Sehmodell sieht nie ganze Seiten an, sondern nur die Abbildungen.
- ⚑ **Abbildungen:** Jede bekommt ihren maschinell erkannten Text und, wenn `myl bild
  --genau` ein genaues Sehmodell findet, eine Beschreibung; beides steht als ungeprüft
  gekennzeichnet im Markdown. Vektorgrafiken in Text-PDFs werden gefunden, weil Seiten
  mit viel Linienwerk zur Erkennung der Abbildungen gehen. Obergrenze mit
  `--bilder-hoechstens`, abschalten mit `--keine-bilder`.
- ⚑ **Der Agent kann es selbst** (`myl-local-agent` 0.47.0): `book_to_skill` macht aus einem
  Buch im Arbeitsordner einen Skill unter `.AGENT/skills`, mit Rückfrage und, nach Fremdem
  im Gespräch, nur mit Freigabe. Verdichtet wird ohne Modell, damit kein zweites neben dem
  des Agenten lädt; `myl buch --modell` verdichtet mit einem.
- ⛔️ **Ein Buch-Skill ist überall fremd** (Fund 683): Die Marke im Skill zählt in jedem
  Skillordner. Vorher kamen Titel und Stichworte eines Buchs als vertrauter Hinweis in jeden
  passenden Auftrag.
- ⚑ **Bibliothek und Belege:** Gelesene Werke liegen unter `literature/Titel - Autor - ISBN oder DOI/`
  als Markdown je Seite samt Grafiken, das Original nie; `.AGENT/skills/<name>/` hält nur den
  kurzen Skill. Jeder Punkt nennt seine Seite als `[12]`, `read_file` holt den Wortlaut und
  `search_files` findet, was nicht im Skill steht, wie beim verdichteten Gespräch. Alles daraus gilt
  als fremd. Titel, Autor und ISBN oder DOI kommen aus den Metadaten oder von den ersten Seiten.
- ⚠️ **Eine Beschreibung ist keine Abschrift.** Das Sehmodell verliest Beschriftungen
  und kann Pfeile falsch deuten; darum die Kennzeichnung und der erkannte Text daneben.

### v0.56.0 (2026-11-09): Book2Skill

- ⚑ **`werkzeuge/book2skill.py`:** aus einem Werk wird in zwei Stufen ein kurzer
  Skill, der Volltext bleibt daneben. Stufe 1 wandelt PDF, EPUB, DOCX, ODT, FB2, HTML,
  RTF, Markdown oder Text nach Markdown samt Grafiken: bei PDF mit Überschriften aus
  Gliederung oder Schriftgröße, ohne Kopf- und Fußzeilen, mit aufgelösten Trennungen.
  Stufe 2 verdichtet hierarchisch über `myl fragen` (Stücke, Kapitel, Werk), bricht
  sauber ab und setzt fort; ohne Modell ein Auszug der tragenden Sätze.
- ⛔️ **Belegprobe je Aussage:** Alle Zahlen und drei Fünftel der Wörter müssen in
  höchstens drei aufeinanderfolgenden Sätzen des Originals stehen. Am 4B gemessen: Ohne
  diese Enge bestanden Aussagen, die Zahlen aus einem anderen Zusammenhang holten.
- ⛔️ **Ein Werk ist fremder Text:** Jede Datei beginnt mit der Marke, an der der Agent
  Fremdes erkennt, auch der Skill. `--vertraut` lässt sie beim Skill weg und ist die
  Aussage eines Menschen, dass er ihn gelesen hat.
- Gleiche Eingabe und gleiches Modell geben gleiche Bytes. Nur die Standardbibliothek;
  PDF braucht `pypdf` (BSD), Grafiken daraus zum Teil Pillow.
- ⚠️ Gescannte PDFs ohne Text und MOBI gehen nicht; Tabellen im PDF kommen als Text. Index
  nach Design 4 und Ausgabe an STORAGE stehen aus.

### v0.55.2 (2026-11-08): Der Abruf ins eigene Netz, zwei Lücken (`myl-local-agent` 0.46.2)

- ⛔️ **Vier IPv6-Bereiche galten als öffentlich:** 6to4 (`2002::/16`) und
  Teredo (`2001::/32`) tragen eine IPv4-Adresse, also auch `127.0.0.1`,
  dazu IPv4-kompatible Adressen (`::/96`) und das veraltete standortlokale
  `fec0::/10`. Beim Audit gelesen; Probe `nur_oeffentliche_adressen_sind_oeffentlich`.
- ⛔️ **Kein `--compressed` mehr.** Die Grenze von 20 MB galt dem, was ankam;
  gepackte Nullen hätten beim Auspacken Gigabyte auf die Platte geschrieben.

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
