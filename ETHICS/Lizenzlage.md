# Lizenzlage der Basismodelle

**Stand:** 2026-09-01 (Inhalt vom 2026-08-23, Fundort gewechselt)

> ⚑ **Diese Datei lag bis zum 2026-09-01 unter
> `INTEGER_LLM/docs/01_licenses.md`.** Sie steht jetzt in ETHICS, weil
> sie hierher gehört: Grundsatz **G7** verlangt Apache 2.0 oder MIT, und
> dies ist der Beleg dafür, welche Varianten das erfüllen.
> `werkzeuge/lizenzprobe.py` liest Dateien und kann Recht nicht lesen;
> **die variantenscharfe Bewertung steht hier und stammt von Menschen.**
>
> Das Verzeichnis `INTEGER_LLM/docs/` ist im selben Zug entfallen. Die
> beiden anderen Dateien darin waren überholt, eine davon nannte für
> denselben Entscheidungspunkt andere Zahlen als das Protokoll, das ihn
> führt.

| Komponente | Lizenz | Nutzung |
|---|---|---|
| Qwen2.5, geprüfte Varianten (siehe unten) | Apache 2.0 | Modellgewichte |
| Hugging Face Transformers | Apache 2.0 | Offline-Kalibrierung |
| Safetensors | Apache 2.0 | Gewicht-Export |
| Eigener Code | PolyForm Shield License 1.0.0 | Integer-Inferenzsystem |

## Hinweis

- Alle HF-Abhängigkeiten dürfen **nur in der Kalibrierung** genutzt werden.
- Der Inferenzpfad hat keine Python/HF-Abhängigkeiten.
- Die Code-Lizenz (PolyForm Shield License 1.0.0, siehe `LICENSE.md` im
  Repository-Wurzelverzeichnis) ist von der Modelllizenz unabhängig.

---

## Lizenzprüfung je Modellvariante (2026-08-23)

**Warum je Variante und nicht je Familie.** Whitepaper Kap. 10.1 und
`ETHICS/Manifest.md` G7 verlangen Apache 2.0 oder MIT, und zwar nicht aus
Prinzipienreiterei: Ein offenes Protokoll kennt seine Nutzerzahl nicht
und kann sie nicht begrenzen, eine Lizenz mit Nutzerzahl-Obergrenze ist
für es schlicht nicht einhaltbar. G7 warnte bereits, das gelte „nicht
automatisch für alle Qwen2.5-Größen". **Die Warnung war berechtigt: Zwei
der sieben Größen fallen durch.**

**Methode.** Abgefragt wurde je Variante das `license`-Feld der
Modellkarte über die Hugging-Face-API (`/api/models/<id>`), bei den
beiden Abweichlern zusätzlich der Volltext der Lizenzdatei. Das ist eine
Prüfung der **Angabe des Anbieters**, keine Rechtsberatung und keine
eigene juristische Würdigung.

| Variante | `license` | Lizenz | Für Myelith |
|---|---|---|---|
| Qwen2.5-0.5B | `apache-2.0` | Apache 2.0 | ✅ geeignet |
| Qwen2.5-1.5B | `apache-2.0` | Apache 2.0 | ✅ geeignet |
| **Qwen2.5-3B** | `other` | **Qwen Research License** | ❌ **ausgeschlossen** |
| Qwen2.5-7B | `apache-2.0` | Apache 2.0 | ✅ geeignet |
| Qwen2.5-14B | `apache-2.0` | Apache 2.0 | ✅ geeignet |
| Qwen2.5-32B | `apache-2.0` | Apache 2.0 | ✅ geeignet |
| **Qwen2.5-72B** | `other` | **Qwen License** | ❌ **ausgeschlossen** |

### Warum 3B ausscheidet

Die Qwen Research License erteilt die Rechte nach §2(a) ausdrücklich
**„FOR NON-COMMERCIAL PURPOSES ONLY"**; §2(b) verlangt für kommerzielle
Nutzung eine gesonderte Lizenz. Ein Netz, in dem Miner für Rechenarbeit
bezahlt werden, ist keine nicht-kommerzielle Nutzung.

### Warum 72B ausscheidet

Die Qwen License erlaubt kommerzielle Nutzung, aber §4 verlangt: *„If you
are commercially using the Materials, and your product or service has
more than 100 million monthly active users, you shall request a license
from us."*

**Das ist genau der Fall, den G7 als nicht einhaltbar benennt.** Ein
offenes Protokoll hat keine Instanz, die monatlich aktive Nutzer zählt,
und keine, die eine Lizenz beantragen könnte. Die Schwelle mag heute weit
entfernt scheinen; eine Bedingung, die man **strukturell** nicht erfüllen
kann, ist unabhängig von ihrer Höhe ein Ausschlussgrund.

Hinzu kommt §5(b): Bearbeitungen müssen „Built with Qwen" oder „Improved
using Qwen" ausweisen. Das wäre erfüllbar, ändert am Befund aber nichts.

### Was das für die Skalierungsfrage bedeutet

K6 fragt nach weiteren Modellgrößen. Innerhalb der Qwen2.5-Reihe stehen
dafür **14B und 32B** offen, beide Apache 2.0 und beide dense. **3B und
72B sind keine Option**, auch nicht für eine reine Messung: Ein
Messergebnis an einem Modell, das nicht Genesis-Modell werden kann, hilft
der Frage nicht weiter, und die Gewichte lägen dafür auf der Platte.

Damit ist die nächste Größe nach 7B die **14B**, nicht die 72B.

## Quantisierte Ableitungen

Frühere Fassungen dieses Dokuments führten die Lizenzlage für
quantisierte Ableitungen als offen (Stand 2026-08-12). Für die
Apache-2.0-Varianten ist sie es nicht:

- **§2** erteilt das Recht, das Werk zu vervielfältigen und
  **Bearbeitungen** („Derivative Works") herzustellen und zu verbreiten.
- **§4(a)** verlangt, jedem Empfänger einer Bearbeitung eine Kopie der
  Lizenz mitzugeben.
- **§4(b)** verlangt, geänderte Dateien als geändert zu kennzeichnen.
- **§4(d)** greift nur, wenn das Werk eine `NOTICE`-Datei enthält.
  **Geprüft am 2026-08-23:** Die Qwen2.5-Repositorien enthalten keine.

Ein ganzzahliges Artefakt ist eine Bearbeitung in diesem Sinn und damit
zulässig, solange §4(a) und §4(b) eingehalten werden. Das Skalenpaket
(`scale_packs/`) enthält im Übrigen **keine** abgeleiteten Gewichte,
sondern eigene Messwerte, und berührt die Frage deshalb ohnehin nicht.

*Das ist eine Lesart des Lizenztextes, keine Rechtsberatung. Vor einem
Genesis-Block gehört sie von jemandem geprüft, der dafür haftet. Die
Komponente ETHICS führt die rechtliche Einordnung als offenen Punkt.*

## Fund bei dieser Prüfung: die Lizenzdatei kam nie an

`ETHICS/Manifest.md` berief sich für G7 auf
`INTEGER_LLM/models/Qwen2.5-0.5B/LICENSE`. **Diese Datei existierte
nicht.** Die Beschaffung im Testclient lud mit
`allow_patterns=['*.json','*.safetensors','*.txt']`, und eine Lizenzdatei
trägt keine Endung.

Dieselbe Klasse wie Fund 27 und Fund 37: eine schriftliche Zusage, die
niemand nachgesehen hat. Sie wog hier doppelt, weil Apache 2.0 §4(a)
verlangt, die Lizenz an Empfänger einer Bearbeitung weiterzugeben, und
weil ein Partner, der die Gewichte für einen Testlauf holt, sonst nie
erfährt, unter welchen Bedingungen sie bei ihm liegen.

**Behoben** in `myl-testclient` v0.11.0: `LICENSE*` steht in den
Mustern. Beide lokal vorhandenen Modelle tragen die Datei jetzt; sie ist
in beiden Fällen bytegleich (SHA-256
`832dd9e00a68dd83b3c3fb9f5588dad7dcf337a0db50f7d9483f310cd292e92e`) und
ist der unveränderte Apache-2.0-Text.

`models/` war nicht versioniert, die Datei lag also auf jeder Maschine
neben ihren Gewichten und nicht im Repositorium.

⚑ **Am 2026-09-01 wurde das umgedreht** (Festlegung des
Projektinhabers), und der Grund ist der Ablauf beim Miner: Er holt die
Gewichte **per Skript direkt von der Quelle**. Liegt die Lizenz nur
neben den heruntergeladenen Gewichten, erfährt er die Bedingungen
**nachdem** er sie geholt hat. Künftig trägt jedes empfohlene Modell
sein Verzeichnis samt Lizenzdatei bereits im Repositorium, die Gewichte
bleiben draußen. Damit ist die Lizenz lesbar, **bevor** jemand etwas
herunterlädt, und ein Modellordner ohne Lizenz fällt in
`lizenzprobe.py` auf, statt erst nach dem Download zu entstehen.

**Angelegt wird eine solche Datei, sobald ein Modell für den
Produktionsbetrieb empfohlen ist**, nicht für jedes, das jemand einmal
zum Messen geladen hat.
