# Anleitung: Tests mit mehreren Beteiligten und heterogener Hardware

**Version:** 1.2.0 · **Datum:** 2026-08-20

Diese Anleitung richtet sich an zwei Rollen:

- **Koordinator** legt die Parameter fest, sammelt die Protokolle und
  fällt das Urteil. Eine Person.
- **Teilnehmer** stellt eine Maschine, führt die Läufe aus und schickt
  die Protokolle. Beliebig viele, gern unerfahren.

Ein Teilnehmer muss das Projekt nicht kennen. Er braucht diese Seite und
einen Rechner.

---

## Der schnellste Weg, wenn du nur mitmachen willst

Doppelklick auf den Starter im Ordner `TESTCLIENT`:

| Dein System | Datei |
|---|---|
| macOS | `Myelith Testclient - macOS.app` |
| Windows | `Myelith Testclient - Windows (Batch).cmd` |
| Linux | `Myelith Testclient - Linux, macOS (Shell).sh` im Terminal |

Danach fragt der Client zwei Dinge, und beide kannst du mit der
Eingabetaste beantworten:

1. **Welche Testdatei?** Liegt eine bereit, wähle sie. Sie stellt alles
   ein, holt bei Bedarf das Modell und führt die Messung selbst aus.
2. **Welches Modell?** Nur, wenn keine Testdatei gewählt wurde.

Danach öffnet sich das Menü. Ein Punkt genügt dort: **[1] Testlauf
starten**.

Fehlt das Modell, bietet der Client an, es zu holen und zu bauen. Es
passiert nichts ohne Rückfrage, und du siehst währenddessen, wie lange
es schon läuft.

Am Ende steht auf dem Bildschirm eine Zeile wie diese:

```
Ergebnis  determinismus   bitgleich über zwei Läufe  [886124c7f002314a]
```

Der Wert in eckigen Klammern ist das Ergebnis. Er muss auf jeder
Maschine derselbe sein. **Genau das ist der ganze Test.**

Die Protokolldateien unter `TESTCLIENT/myl-testclient/logs/` schickst du
an den Koordinator. Fertig.

---

## Was hier eigentlich gemessen wird

Wer nichts mit Sprachmodellen zu tun hat, dem sagt „Bitgleichheit"
zunächst wenig. Der Kern in einem Bild:

Ein Sprachmodell rechnet mit Zahlen. Üblicherweise mit **Kommazahlen**,
und dabei gilt eine unangenehme Eigenheit: Rechnet man dieselbe Aufgabe
in anderer Reihenfolge, kommt ein leicht anderes Ergebnis heraus. Nicht
falsch, nur anders in den hintersten Stellen. Zwei ehrliche Rechner
liefern deshalb normalerweise verschiedene Zahlen.

Für ein Netzwerk, in dem Fremde füreinander rechnen, ist das fatal: Man
kann nicht unterscheiden, ob jemand betrogen hat oder ob er nur eine
andere Grafikkarte besitzt.

Myelith rechnet deshalb **nur mit ganzen Zahlen**. Bei ganzen Zahlen ist
die Reihenfolge egal, `(3+5)+7` und `3+(5+7)` sind beide 15, auf jeder
Maschine. Damit wird Gleichheit prüfbar: Wer abweicht, hat wirklich
etwas anderes gerechnet.

**Dieser Testclient prüft, ob das in der Praxis hält.** Er lässt dasselbe
Modell dieselbe Frage beantworten und bildet aus der Antwort eine
Prüfsumme. Läuft der Test auf einem Mac mit ARM-Chip und auf einem
Windows-PC mit Intel-Chip und kommt dieselbe Prüfsumme heraus, ist der
Nachweis erbracht.

---

## 0. Der Kern in drei Sätzen

Der Cross-Hardware-Nachweis braucht **zwei** Aussagen, nicht eine:

1. **Die Maschinen sind verschieden.** (Hardware-Fingerabdrücke ungleich)
2. **Das Ergebnis ist trotzdem gleich.** (Digests gleich)

Fehlt (1), beweist (2) nichts: zwei gleiche Ergebnisse von derselben
Maschine sind trivial. Fehlt (2) bei erfülltem (1), ist die Kernthese
des Projekts widerlegt, und das wäre der wichtigste Befund seit
Bestehen des Repositoriums.

---

## 0a. Der Testplan, die Datei, die alles zusammenhält

### Wo Pläne liegen und wie sie gewählt werden

Pläne liegen im Ordner `TESTCLIENT/Testpläne/` und tragen die Endung
`.plan`. Beim Start sieht der Client dort nach, **bevor** er nach dem
Modell fragt, und listet auf, was er gefunden hat:

```
  Testpläne in TESTCLIENT/Testpläne:
   [1] wikitext2-0.5b-standard · qwen2.5-0.5b, 32 Token, 4 Shards, Prüfsumme 1dc4d4ab
      Prompt: " The 2010 Haitian earthquake was a catast…"
   [0] keiner, Einstellungen von Hand wählen

  Auswahl [0]:
```

Die Reihenfolge ist kein Zufall. Ein Plan legt das Modell fest; wer
zuerst nach dem Modell fragt und danach den Plan lädt, hat entweder die
falsche Frage gestellt oder muss sie zurücknehmen.

**Wählst du einen Plan, macht der Client den Rest allein:**

1. Er übernimmt Prompt, Tokenzahl, Shards und Modell.
2. Er prüft, ob das Modell auf dieser Maschine liegt, und stimmt es mit
   dem veröffentlichten Prüfwert überein.
3. Fehlt es, fragt er einmal nach und holt es dann von Hugging Face,
   baut die Artefakte und prüft sie.
4. Er führt beide Messungen aus, Determinismus und Shard-Lauf.
5. Er zeigt die erzeugte Antwort im Klartext und die Prüfsummen.

Danach steht das Menü für weitere Läufe bereit.

**Wählst du `[0]` oder liegt kein Plan im Ordner**, läuft alles wie
zuvor: Der Client fragt nach dem Modell, und du startest die Läufe von
Hand aus dem Menü.

Ein Plan mit falscher Prüfsumme wird **übersprungen und gemeldet**, nicht
stillschweigend geladen. Eine veränderte Datei ist genau der Fall, den
die Prüfsumme abfangen soll.

### Aufbau einer Plandatei


Damit „alle nehmen exakt dieselben Werte" keine Bitte bleibt, verteilt
der Koordinator eine **Plandatei**. Sie legt Prompt, Tokenzahl, Shards
und Modell fest und trägt eine Prüfsumme darüber.

```text
plan_id     = 2026-08-18-cross-arch-01
prompt      = "Die Hauptstadt von Frankreich ist"
steps       = 6
shards      = 4
model       = qwen2.5-0.5b

spec_sha256 = 94be3bfc…
```

**Zwei Dinge macht diese Datei:**

1. **Sie verhindert den häufigsten Fehlalarm.** Wer den Prompt ändert,
   bekommt beim nächsten Lauf einen Fehler und Exit-Code 3, statt
   eines abweichenden Digests, der wie ein Befund aussieht:

   ```
   myl-test: Der Testplan wurde verändert.
        Prüfsumme in der Datei: 94be3bfc…
        tatsächlicher Inhalt:   5b6bde79…
        Verwende die Originaldatei des Koordinators …
   ```

   Der Prompt steht in Anführungszeichen, damit auch ein
   Randleerzeichen erhalten bleibt. Es ist Teil des Prompts und
   verändert das Ergebnis.

2. **Sie sortiert die Protokolle.** Die ersten acht Zeichen der
   Prüfsumme benennen das Protokollverzeichnis:

   ```text
   logs/
   ├── determinismus/
   │   └── 2026-08-18_94be3bfc/
   │       ├── 081222-aarch64-macos-reference.jsonl
   │       └── 143515-x86-64-linux-avx2.jsonl
   └── stack/
       └── 2026-08-18_94be3bfc/
   ```

   Alle Teilnehmer mit demselben Plan landen im **gleichnamigen
   Ordner**, auf jeder Maschine. Wer versehentlich andere Parameter
   nimmt, landet sichtbar woanders. Die Zuordnungsarbeit entfällt.
   Der Dateiname trägt Uhrzeit und Hardware-Kurzform, damit sich die
   Protokolle mehrerer Maschinen in einem Ordner nicht überschreiben.

Ohne Plan läuft alles weiter wie bisher; die Kennung heißt dann
`ohne-plan`.

---

## 1. Für Teilnehmer, die Kurzfassung

### 1.1 Einmalig einrichten

```bash
git clone <repo-url>
cd Repository
```

**Für Hardware- und Protokolltests reicht Rust.** Falls du es nicht hast,
sagt der Starter beim ersten Aufruf, wie es installiert wird (eine Zeile,
keine Administratorrechte nötig). Der Client selbst hat außer `sha2` und
`borsh` keine Fremd-Abhängigkeiten.

**Für Determinismus- und Shard-Tests brauchst du zusätzlich Python mit
PyTorch.** Das Modell wird nicht mitgeliefert, der Client holt es bei
Bedarf und baut daraus die Artefakte; dafür braucht er eine
Python-Umgebung:

```bash
cd INTEGER_LLM/calibrate
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt      # Windows: .venv\Scripts\pip
```

Das sind rund 2 GB und dauert einige Minuten. Danach findet der Client
die Umgebung von selbst; fehlt sie, sagt er genau diese Zeilen an. Er
sucht auch ein System-Python, falls die Pakete dort schon liegen.

Der Modelldownload selbst braucht **kein** Hugging-Face-Konto. Bei vielen
Downloads hintereinander kann die Gegenstelle bremsen; ein gesetztes
`HF_TOKEN` hebt die Grenze an, nötig ist es nicht.

### 1.2 Starten

Im Ordner `TESTCLIENT`:

| Dein System | Wie |
|---|---|
| macOS | Doppelklick auf `Myelith Testclient - macOS.app` |
| Windows | Doppelklick auf `Myelith Testclient - Windows (Batch).cmd` |
| Linux | `./"Myelith Testclient - Linux, macOS (Shell).sh"` im Terminal |

Beim ersten Start baut sich der Client selbst; das dauert einige
Minuten und zeigt dabei, wie lange es schon läuft. Danach sind es
Sekunden.

Die Starter finden das Repository von selbst. Du darfst sie verschieben,
kopieren oder auf den Schreibtisch legen.

**Hast du eine `.plan`-Datei bekommen?** Lege sie in
`TESTCLIENT/Testpläne/`. Der Client bietet sie beim nächsten Start zur
Auswahl an und führt den ganzen Durchgang allein aus.

Für Skripte gibt es weiterhin den direkten Weg:

```bash
./"Myelith Testclient - Linux, macOS (Shell).sh" --plan Testpläne/cross-arch.plan determinismus
```

**Ändere die Plandatei nicht.** Der Client lehnt eine veränderte Datei
ab, genau dafür ist die Prüfsumme da.

### 1.3 Was du ausführst

Mit Plan: nichts, das macht der Client. Ohne Plan wählst du im Menü:

Das Menü hat drei Punkte:

| Punkt | Was er tut |
|---|---|
| **1 Testlauf starten** | Hardware, Determinismus, Shards und Protokoll-Durchlauf nacheinander. Der vollständige Bericht dieser Maschine. |
| **2 Testdatei wählen** | Übernimmt Prompt, Token, Shards und Modell aus einem Plan und beschafft das Modell, falls es fehlt. Danach mit [1] starten. |
| **3 Anleitung lesen** | diese Seite |
| **9 Entwickler-Menü** | Einzelläufe, Artefaktprüfung, Testpläne erzeugen, Einstellungen |

**Mehr brauchst du nicht.** Wer eine Maschine beisteuert, drückt [1] und
schickt das Protokoll. Hast du eine Testdatei bekommen, vorher [2].

Alles Weitere liegt unter **[9]**, weil ein Menü mit zehn Punkten, von
denen fünf nie gebraucht werden, den Einstieg behindert statt ihn zu
erleichtern.

**Auch ohne Modell bist du nützlich.** Der Testlauf erhebt in jedem Fall
die Hardware, und der Protokoll-Durchlauf unter [9] [4] deckt die gesamte
Protokollschicht ab: Kryptografie, Konsens, Verifikation, Ledger,
Tokenomics.

### 1.4 Was du auf dem Bildschirm siehst

Nach jedem Lauf zeigt der Client den Prompt und die erzeugte Antwort im
Klartext:

```
  Prompt:   The 2010 Haitian earthquake was a catastrophic magnitude 7.0 earthquake
  Antwort:  that struck the island of Hispaniola in the Caribbean Sea, killing over
            200,000 people and causing widespread destruction. The earthquake occurred on
```

Das ist zum Zuschauen gedacht. **Bewertet wird der Klartext nicht.**
Maßgeblich ist der Vergleichswert:

```
Ergebnis  determinismus   bitgleich über zwei Läufe  [886124c7f002314a]
```

### 1.5 Was du zurückschickst

Eine einzige Datei: `TESTCLIENT/myl-testclient/logs/myl-test.jsonl`.

Alle Läufe stehen darin, angehängt statt in Unterordnern verteilt. Jede
Zeile trägt Laufkennung, Befehl und Einstellungs-Prüfsumme, die
Zuordnung leisten also die Daten und nicht der Pfad. Daneben liegt
`myl-test.log` mit denselben Ereignissen als Fließtext, für die
Fehlersuche am Terminal.

**Was darin steht:** Architektur, Betriebssystem, Backend, Zeiten,
Vergleichswerte, der **Hash** deines Prompts, die erzeugten Token.
**Was nicht darin steht:** der Klartext der Antwort, dein Prompttext,
dein Benutzername, dein Rechnername, Seriennummern, MAC-Adressen.

Der Klartext bleibt bewusst draußen: Aus den Token ist er ableitbar, und
die Datei bleibt schlank und gut vergleichbar.

Die Dateien sind reiner Text und dürfen unverändert weitergegeben
werden.

---

## 2. Für Koordinatoren, das Verfahren

### 2.1 Plan erzeugen und verteilen

```bash
myl-test plan \
  --plan-id 2026-08-18-cross-arch-01 \
  --prompt "Die Hauptstadt von Frankreich ist" \
  --steps 6 --shards 4 \
  --out cross-arch.plan
```

Oder im Menü über **Punkt 9**. Der Client zeigt danach die
Einstellungs-ID und den Ordner, in dem alle Protokolle landen werden.

Die Datei `cross-arch.plan` unverändert an alle Teilnehmer schicken.
Sie legen sie in `TESTCLIENT/Testpläne/` ab; der Client bietet sie beim
nächsten Start zur Auswahl an und führt den Durchgang selbst aus. Ein
Beispielplan liegt dort bereits: `wikitext2-0.5b-standard.plan`, mit dem
0,5B-Modell und einem Prompt aus WikiText-2, dem Korpus, gegen den auch
die Perplexität des Projekts gemessen wird.
Chat, Mail, Repository, egal. Sie ist reiner Text und enthält keine
personenbezogenen Daten.

### 2.2 Was der Plan festlegt

| Parameter | Beispiel | Warum es exakt gleich sein muss |
|---|---|---|
| Prompt | `Die Hauptstadt von Frankreich ist` | Ein anderes Zeichen → anderer Digest |
| Token (`--steps`) | `8` | Bestimmt, wie viele Schritte in den Digest eingehen |
| θ_v / Artefaktstand | `qwen2.5-0.5b`, Stand vom … | Ein anderes Modell → anderer Digest, völlig zu Recht |

Diese Werte stehen im Plan und sind durch die Prüfsumme abgesichert.
**`plan_id` geht bewusst nicht in die Prüfsumme ein.** Zwei
Koordinatoren, die denselben Test unter verschiedenen Namen fahren,
sollen vergleichbare Ergebnisse bekommen.

### 2.3 Auswerten

Alle Läufe stehen in derselben Datei und tragen die Prüfsumme des Plans
in jeder Zeile (`settings_id`). Die
Teilnehmerdateien einfach dort hineinlegen:

```bash
cd logs/determinismus/2026-08-18_94be3bfc/

grep '"name":"hardware_fingerprint"' *.jsonl   # muss sich unterscheiden
grep '"name":"determinismus"'        *.jsonl   # muss übereinstimmen
grep '"key":"backend_selected"'      *.jsonl   # zur Einordnung
grep '"key":"einstellungen_id"'      *.jsonl   # muss überall gleich sein
```

Die letzte Zeile ist die Gegenprobe: Steht dort bei jemandem etwas
anderes, hat er einen anderen Plan verwendet, und ein Vergleich
gegenstandslos, ganz gleich was die Digests sagen.

### 2.4 Urteilstabelle

| Fingerabdrücke | Digests | Urteil |
|---|---|---|
| verschieden | gleich | ✅ **Nachweis erbracht** für diese Architekturen und Backends |
| **gleich** | gleich | ⚠️ **Nichts bewiesen.** Es war dieselbe Maschinenklasse. Andere Hardware besorgen. |
| verschieden | **verschieden** | 🔴 **Befund.** Erst die drei Ausschlussfragen in 2.5 durchgehen. |

### 2.5 Bei verschiedenen Digests, in dieser Reihenfolge prüfen

1. **Haben alle denselben Plan verwendet?**
   `grep '"key":"einstellungen_id"' *.jsonl`; bei gleichem Plan steht
   überall derselbe Wert, und die Protokolle liegen ohnehin im selben
   Ordner. Zusätzlich `grep '"prompt_sha256"' *.jsonl` als Gegenprobe
   auf den Prompt selbst.
2. **Ist θ_v identisch?**
   `grep '"kind":"artifact"' *.jsonl`; verschiedene Modelldimensionen
   oder Artefaktstände erklären jeden Unterschied.
3. **Läuft dasselbe Backend?**
   `grep '"key":"backend_selected"' *.jsonl`; Referenz gegen
   `cpu-simd/avx2` ist ein *gewollter* Vergleich, aber er muss trotzdem
   bitgleich sein. Weicht er ab, ist es ein SIMD-Paritätsfehler und
   gehört in den INTEGER_LLM-Fahrplan.

Erst wenn alle drei übereinstimmen und die Digests trotzdem abweichen,
ist es ein Befund an der Kernthese aus Whitepaper Kap. 6.2. Dann:

- Beide vollständigen `.jsonl` sichern.
- Fund in `INTEGER_LLM/README/Fahrplan-v3.md` eintragen.
- **Nicht** vorschnell auf die Hardware schieben. Der wahrscheinlichste
  Grund ist eine Gleitkomma-Operation, die in den Rechenpfad geraten ist.
  `INTEGER_LLM/tests/audit/test_no_float.py` ist der erste Griff.

### 2.6 Ergebnis festhalten

Laufprotokolle sind flüchtig (`logs/` ist gitignored). Ein bestätigter
Cross-Hardware-Nachweis gehört dauerhaft nach
`INTEGER_LLM/eval/results/`, mit Datum, beteiligten Architekturen,
Backends, θ_v-Stand und den Digests.

---

## 3. Welche Hardware lohnt sich

Nach abnehmendem Erkenntniswert:

| Kombination | Was sie prüft |
|---|---|
| **x86_64 + aarch64** | Verschiedene Befehlssätze, verschiedene Compiler-Backends. Der wichtigste Vergleich. |
| **Referenz + AVX2** | Ob die SIMD-Kernel wirklich bit-identisch sind. Deckt die Paritätslücke aus Fund A19 mit ab. |
| **Linux + macOS + Windows** | libm- und Toolchain-Unterschiede. Hier hätte die alte `f64::exp()`-LUT zugeschlagen (Fund A5). |
| **Debug + Release** | Überlaufverhalten. Debug panickt, Release läuft um, genau der Unterschied aus Fund A14. |
| Zwei x86_64-Maschinen derselben Generation | Wenig. Nur als Rauschprüfung. |

**Big-Endian** wäre der schärfste Test überhaupt (das Protokoll ist
durchgehend Little-Endian kodiert). Realistisch verfügbar ist das kaum;
falls doch, hat dieser Lauf Vorrang vor allen anderen.

---

## 4. Ohne Artefakte mitmachen

Wer die Modellartefakte nicht hat, führt **Menüpunkt 1 und 4** aus. Der
Stack-Durchlauf prüft in etwa einer Sekunde:

| Stufe | Was geprüft wird |
|---|---|
| `krypto` | Merkle inkl. Negativprobe, BLS (Signatur, Fremdbotschaft, Aggregat), VRF |
| `epochenseed` | deterministisch, epochenabhängig, leerer Blockhash abgelehnt |
| `stichprobe` | 20 von 1000 Segmenten, sortiert, deterministisch |
| `komiteewahl` | 21 Producer + 7 Arbiter, rotiert über Epochen |
| `bft` | Quorum über Stimmgewicht; **Fremdstimme und Fremdsignatur abgelehnt** |
| `double_signing` | erkannter Beweis gilt, erfundener wird abgelehnt |
| `block` | kanonische Typen, `state_root` wirkt auf den Blockhash |
| `verifikation` | Abweichung lokalisiert, Schuld zugewiesen |
| `ledger` | Verifier-Entscheidung wird tatsächlich durchgebucht |
| `tokenomics` | EMA, Prägung, exp-LUT exakt, Preis richtungsrichtig |

Der Wert `stack_gesamt` muss bei gleichem Code auf **jeder** Maschine
identisch sein, denn er enthält keine Zeitwerte und keine Zufallszahlen ohne
festen Seed. Weicht er ab, ist entweder der Code verschieden oder es
liegt eine Plattformabhängigkeit vor.

---

## 5. Was diese Tests **nicht** abdecken

Ehrlichkeitshalber, damit niemand mehr hineinliest, als drin ist:

- **Kein Netzwerkbetrieb.** Alles läuft in einem Prozess. `myl-net`
  (Gossip, Peer-Discovery) wird nicht berührt; echte Sockets gehören
  in die NETWORKING-Testsuite.
- **Keine Liveness.** Der BFT-Durchlauf prüft Safety (Quorum,
  Signaturen, Mitgliedschaft). Rundenwechsel und Timeouts gibt es noch
  nicht (CONSENSUS Punkt 3.6); ein Leader-Ausfall ist nicht testbar.
- **Keine Lastprüfung.** Zeiten stehen im Protokoll, weil sie bei der
  Fehlersuche helfen. Für Durchsatzmessungen gibt es
  `runtime/src/bin/bench_probe`.
- **Kein Krypto-Review.** Dass BLS und VRF *korrekt* implementiert sind,
  belegen die RFC-Testvektoren in `myl-types`, nicht dieser Client. Er
  prüft nur, dass die Aufrufe zusammenpassen.
- **Kein Sicherheitsaudit der Artefakte.** Ein manipuliertes Artefakt
  liefert einen anderen Digest, aber der Client sagt nicht, *warum*.

---

## 6. Häufige Stolpersteine

**„Artefaktverzeichnis fehlt"**
Erwartet wird `INTEGER_LLM/artifacts/qwen2.5-0.5b/`. Liegen die
Artefakte woanders: `--artifacts <PFAD>` oder Menüpunkt 6. Alternativ
`INTEGER_LLM_ARTIFACTS_DIR` setzen.

**Der Determinismuslauf dauert ewig**
Im Debug-Build ~40 s je Durchlauf. Immer `--release` verwenden.

**Das Banner zerschießt die Ausgabe**
`MYL_NO_BANNER=1` setzen oder `--quiet` verwenden. In Skripten ohnehin
zu empfehlen.

**„Der Testplan wurde verändert"**
Die Datei wurde nach dem Erzeugen bearbeitet. Auch ein zusätzliches
Leerzeichen im Prompt zählt. Originaldatei vom Koordinator neu anfordern.
Kommentarzeilen (`#`) dürfen dagegen frei ergänzt werden, sie gehen
nicht in die Prüfsumme ein.

**Zwei Läufe auf derselben Maschine ergeben verschiedene Digests**
Das wäre schwerwiegend: Die Determinismusprüfung meldet es als
`ABWEICHUNG`. Protokoll sichern und melden; hier liegt kein
Bedienfehler vor.

**Der Stack-Lauf schlägt fehl, obwohl nichts geändert wurde**
Dann ist eine Annahme *zwischen* zwei Komponenten gebrochen. Die
fehlgeschlagene Stufe steht im Protokoll mit Grund. Genau dafür gibt es
diesen Lauf.

---

## 7. Meldevorlage

```
Testplan:        <plan_id>   (Einstellungs-ID: <einstellungen_id>)
Maschine:        z. B. Apple M2 Pro / Ryzen 5950X
Architektur:     aus dem Protokoll (arch)
Betriebssystem:  aus dem Protokoll (os)
Backend:         aus dem Protokoll (backend_selected)
Build:           release / debug

Fingerabdruck:   <hardware_fingerprint>
Stack-Gesamt:    <stack_gesamt>
Determinismus:   <determinismus>   (oder: keine Artefakte)
Shard-Lauf:      <shard_vs_einzelknoten>   (oder: keine Artefakte)

Auffälligkeiten: <Fehler/Abweichungen aus dem Protokoll, sonst "keine">
Anhang:          <lauf-id>.jsonl
```

---

## Changelog

### v1.1.0 – 2026-08-18
- **Testplan ergänzt** (Abschnitt 0a): Die Vorgabe der Parameter ist
  jetzt eine Datei mit Prüfsumme statt einer Bitte im Fließtext. Der
  häufigste Fehlalarm, ein versehentlich veränderter Prompt, der wie
  ein Befund an der Kernthese aussieht, ist damit technisch
  ausgeschlossen.
- **Protokoll-Ablage** nach Prüflauf, Datum und Einstellungs-Kennung.
  Alle Teilnehmer eines Plans tragen dieselbe `settings_id`; die
  Zuordnungsarbeit beim Auswerten entfällt.
- Abschnitt 2 auf das Planverfahren umgestellt, neue Gegenprobe über
  `einstellungen_id`, Meldevorlage um den Plan erweitert.

### v1.0.0 – 2026-08-18
- Erstfassung. Vorher gab es nur acht Zeilen im README, die zwar die
  zwei Befehle nannten, aber nicht beantworteten, wie sich mehrere
  Beteiligte koordinieren, was bei Abweichung zu tun ist und wie ein
  Ergebnis dauerhaft festgehalten wird.
- Bewusst nach Rollen getrennt: Ein Teilnehmer soll Abschnitt 1 lesen
  können und fertig sein.
- Abschnitt 5 („Was diese Tests nicht abdecken") ist Absicht. Eine
  Testanleitung ohne Grenzbeschreibung verleitet dazu, Ergebnisse
  überzuinterpretieren.
