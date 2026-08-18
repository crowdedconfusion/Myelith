# Anleitung: Tests mit mehreren Beteiligten und heterogener Hardware

**Version:** 1.0.0 · **Datum:** 2026-08-18

Diese Anleitung richtet sich an zwei Rollen:

- **Koordinator** — legt die Parameter fest, sammelt die Protokolle,
  fällt das Urteil. Eine Person.
- **Teilnehmer** — stellt eine Maschine, führt die Läufe aus, schickt
  die Protokolle. Beliebig viele, gern unerfahren.

Ein Teilnehmer muss das Projekt nicht kennen. Er braucht diese Seite,
ein Terminal und die Artefakte.

---

## 0. Der Kern in drei Sätzen

Der Cross-Hardware-Nachweis braucht **zwei** Aussagen, nicht eine:

1. **Die Maschinen sind verschieden.** (Hardware-Fingerabdrücke ungleich)
2. **Das Ergebnis ist trotzdem gleich.** (Digests gleich)

Fehlt (1), beweist (2) nichts — zwei gleiche Ergebnisse von derselben
Maschine sind trivial. Fehlt (2) bei erfülltem (1), ist die Kernthese
des Projekts widerlegt, und das wäre der wichtigste Befund seit
Bestehen des Repositoriums.

---

## 1. Für Teilnehmer — die Kurzfassung

### 1.1 Einmalig einrichten

```bash
git clone <repo-url> && cd Repository/TESTCLIENT/myl-testclient
cargo build --release
```

Rust ab 1.82. Sonst nichts — der Client hat außer `sha2` und `borsh`
keine Fremd-Abhängigkeiten.

### 1.2 Starten

```bash
cargo run --release --bin myl-test
```

Ohne Unterbefehl öffnet sich das Menü. Alles Weitere per Ziffer.

### 1.3 Was du ausführst

| Menüpunkt | Wann | Braucht Artefakte? |
|---|---|---|
| **1 Hardware** | immer, als Erstes | nein |
| **4 Stack** | immer | nein |
| **2 Determinismus** | wenn du Artefakte hast | **ja** |
| **3 Shards** | wenn du Artefakte hast | **ja** |

**Auch ohne Artefakte bist du nützlich.** Punkt 1 und 4 laufen überall
und decken die gesamte Protokollschicht ab — Kryptografie, Konsens,
Verifikation, Ledger, Tokenomics.

### 1.4 Was du zurückschickst

Die `.jsonl`-Dateien aus `TESTCLIENT/myl-testclient/logs/`.

**Was darin steht:** Architektur, Betriebssystem, Backend, Zeiten,
Vergleichswerte, der **Hash** deines Prompts.
**Was nicht darin steht:** dein Prompttext, dein Benutzername, dein
Rechnername, Seriennummern, MAC-Adressen.

Die Dateien sind reiner Text und dürfen unverändert weitergegeben
werden.

---

## 2. Für Koordinatoren — das Verfahren

### 2.1 Vorher festlegen und an alle verteilen

| Parameter | Beispiel | Warum es exakt gleich sein muss |
|---|---|---|
| Prompt | `Die Hauptstadt von Frankreich ist` | Ein anderes Zeichen → anderer Digest |
| Token (`--steps`) | `8` | Bestimmt, wie viele Schritte in den Digest eingehen |
| θ_v / Artefaktstand | `qwen2.5-0.5b`, Stand vom … | Ein anderes Modell → anderer Digest, völlig zu Recht |

**Tipp:** Verteile die Parameter als fertige Befehlszeile, nicht als
Fließtext. Ein Leerzeichen zu viel im Prompt kostet einen Testlauf.

```bash
cargo run --release --bin myl-test -- determinismus \
  --prompt "Die Hauptstadt von Frankreich ist" --steps 8
```

### 2.2 Auswerten

Aus jedem Protokoll drei Werte ziehen:

```bash
grep '"name":"hardware_fingerprint"' *.jsonl   # muss sich unterscheiden
grep '"name":"determinismus"'        *.jsonl   # muss übereinstimmen
grep '"name":"stack_gesamt"'         *.jsonl   # muss übereinstimmen
grep '"key":"backend_selected"'      *.jsonl   # zur Einordnung
```

### 2.3 Urteilstabelle

| Fingerabdrücke | Digests | Urteil |
|---|---|---|
| verschieden | gleich | ✅ **Nachweis erbracht** — für diese Architekturen und Backends |
| **gleich** | gleich | ⚠️ **Nichts bewiesen.** Es war dieselbe Maschinenklasse. Andere Hardware besorgen. |
| verschieden | **verschieden** | 🔴 **Befund.** Erst die drei Ausschlussfragen in 2.4 durchgehen. |

### 2.4 Bei verschiedenen Digests — in dieser Reihenfolge prüfen

1. **Ist der Prompt zeichengleich?**
   `grep '"prompt_sha256"' *.jsonl` — genau dafür steht der Hash im
   Protokoll. Unterschiedliche Hashes heißen: unterschiedliche Prompts,
   kein Befund.
2. **Ist θ_v identisch?**
   `grep '"kind":"artifact"' *.jsonl` — verschiedene Modelldimensionen
   oder Artefaktstände erklären jeden Unterschied.
3. **Läuft dasselbe Backend?**
   `grep '"key":"backend_selected"' *.jsonl` — Referenz gegen
   `cpu-simd/avx2` ist ein *gewollter* Vergleich, aber er muss trotzdem
   bitgleich sein. Weicht er ab, ist es ein SIMD-Paritätsfehler und
   gehört in den INTEGER_LLM-Fahrplan.

Erst wenn alle drei übereinstimmen und die Digests trotzdem abweichen,
ist es ein Befund an der Kernthese aus Whitepaper Kap. 6.2. Dann:

- Beide vollständigen `.jsonl` sichern.
- Fund in `INTEGER_LLM/README/Fahrplan-v3.md` eintragen.
- **Nicht** vorschnell auf die Hardware schieben — der wahrscheinlichste
  Grund ist eine Gleitkomma-Operation, die in den Rechenpfad geraten ist.
  `INTEGER_LLM/tests/audit/test_no_float.py` ist der erste Griff.

### 2.5 Ergebnis festhalten

Laufprotokolle sind flüchtig (`logs/` ist gitignored). Ein bestätigter
Cross-Hardware-Nachweis gehört dauerhaft nach
`INTEGER_LLM/eval/results/` — mit Datum, beteiligten Architekturen,
Backends, θ_v-Stand und den Digests.

---

## 3. Welche Hardware lohnt sich

Nach abnehmendem Erkenntniswert:

| Kombination | Was sie prüft |
|---|---|
| **x86_64 + aarch64** | Verschiedene Befehlssätze, verschiedene Compiler-Backends. Der wichtigste Vergleich. |
| **Referenz + AVX2** | Ob die SIMD-Kernel wirklich bit-identisch sind. Deckt die Paritätslücke aus Fund A19 mit ab. |
| **Linux + macOS + Windows** | libm- und Toolchain-Unterschiede. Hier hätte die alte `f64::exp()`-LUT zugeschlagen (Fund A5). |
| **Debug + Release** | Überlaufverhalten. Debug panickt, Release läuft um — genau der Unterschied aus Fund A14. |
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
identisch sein — er enthält keine Zeitwerte und keine Zufallszahlen ohne
festen Seed. Weicht er ab, ist entweder der Code verschieden oder es
liegt eine Plattformabhängigkeit vor.

---

## 5. Was diese Tests **nicht** abdecken

Ehrlichkeitshalber, damit niemand mehr hineinliest, als drin ist:

- **Kein Netzwerkbetrieb.** Alles läuft in einem Prozess. `myl-net`
  (Gossip, Peer-Discovery) wird nicht berührt — echte Sockets gehören
  in die NETWORKING-Testsuite.
- **Keine Liveness.** Der BFT-Durchlauf prüft Safety (Quorum,
  Signaturen, Mitgliedschaft). Rundenwechsel und Timeouts gibt es noch
  nicht (CONSENSUS Punkt 3.6) — ein Leader-Ausfall ist nicht testbar.
- **Keine Lastprüfung.** Zeiten stehen im Protokoll, weil sie bei der
  Fehlersuche helfen. Für Durchsatzmessungen gibt es
  `runtime/src/bin/bench_probe`.
- **Kein Krypto-Review.** Dass BLS und VRF *korrekt* implementiert sind,
  belegen die RFC-Testvektoren in `myl-types` — nicht dieser Client. Er
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

**Zwei Läufe auf derselben Maschine ergeben verschiedene Digests**
Das wäre schwerwiegend — die Determinismusprüfung meldet es als
`ABWEICHUNG`. Protokoll sichern und melden; hier liegt kein
Bedienfehler vor.

**Der Stack-Lauf schlägt fehl, obwohl nichts geändert wurde**
Dann ist eine Annahme *zwischen* zwei Komponenten gebrochen. Die
fehlgeschlagene Stufe steht im Protokoll mit Grund. Genau dafür gibt es
diesen Lauf.

---

## 7. Meldevorlage

```
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

### v1.0.0 – 2026-08-18
- Erstfassung. Vorher gab es nur acht Zeilen im README, die zwar die
  zwei Befehle nannten, aber nicht beantworteten, wie sich mehrere
  Beteiligte koordinieren, was bei Abweichung zu tun ist und wie ein
  Ergebnis dauerhaft festgehalten wird.
- Bewusst nach Rollen getrennt: Ein Teilnehmer soll Abschnitt 1 lesen
  können und fertig sein.
- Abschnitt 5 („Was diese Tests nicht abdecken") ist Absicht — eine
  Testanleitung ohne Grenzbeschreibung verleitet dazu, Ergebnisse
  überzuinterpretieren.
