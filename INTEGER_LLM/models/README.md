# models/

Ablageort für das Quellmodell, aus dem die θ_v-Artefakte entstehen.
Zweck: reproduzierbare Herkunft statt implizitem Hugging-Face-Cache.

Der Inhalt wird nicht versioniert (siehe `.gitignore`); nur dieses README,
`KATALOG.json` und die `.gitignore` bleiben im Repository.

> **Diese Datei wird erzeugt.** Quelle sind `models/KATALOG.json`
> (kuratiert: Herkunft, Revision, Lizenz, Status) und
> `scale_packs/REGISTER.json` (erzeugt: Digest, θ_v). Änderungen gehören
> in eine der beiden Dateien, danach `python tools/modelle_liste.py`.

Jede Variante braucht eine **eigene Lizenzprüfung** (Whitepaper Kap. 10.1,
ETHICS-Grundsatz G7: Apache 2.0 oder MIT) und eine **fixierte Revision**:
ohne beides ist der Lauf weder zulässig noch reproduzierbar. Es werden
ausschließlich **Basis-Varianten** verwendet, keine Instruct-Varianten
(Scope-Entscheidung 12.15).


## Modelle

| Modell | Hugging Face | Revision | Lizenz | Parameter | Layer | Gewichte | Artefakt | θ_v | Status |
|---|---|---|---|---|---|---|---|---|---|
| `qwen2.5-0.5b` | [Qwen/Qwen2.5-0.5B](https://huggingface.co/Qwen/Qwen2.5-0.5B) | `060db6499f32…` | Apache-2.0 | 0,5 Mrd. | 24 | rund 1 GB | 0,74 GB | 0.17.0 | verifiziert |
| `qwen2.5-7b` | [Qwen/Qwen2.5-7B](https://huggingface.co/Qwen/Qwen2.5-7B) | `d14972939875…` | Apache-2.0 | 7 Mrd. | 28 | rund 14 GB | 8,1 GB | 0.17.0 | verifiziert |

**Status:**

- **verifiziert**: Artefakte gebaut, Perplexität gegen die Gleitkomma-Referenz gemessen, Akzeptanzkriterium erfüllt, Skalenpaket im Repository.
- **erprobt**: Artefakte gebaut und lauffähig, Qualität noch nicht gegen die Referenz gemessen.
- **vorgemerkt**: Lizenz geprüft und Revision festgelegt, aber noch nicht geholt oder gebaut.

**Gemessene Qualität** (Perplexität, WikiText-2):

- `qwen2.5-0.5b`: 15,27 gegen BF16 14,95 (+2,11 %)
- `qwen2.5-7b`: 8,78 gegen BF16 8,68 (+1,14 %)

**Anmerkungen:**

- `qwen2.5-0.5b`: Die Messgröße des Projekts: Der Entscheidungspunkt 12.21 hängt an diesem Modell, und alle Diagnosen sind daran gemessen. Wer mittestet, fängt hier an.
- `qwen2.5-7b`: Die zweite Größe, an der die Skalierungsfrage hängt (Kritikpunkt K6). Rechnet rund 2 Token je Sekunde: ein Testlauf dauert Minuten, nicht Sekunden. Nur wählen, wenn 23 GB Platte frei sind.

## Woher die Gewichte kommen

Der Testclient holt sie selbst, wenn er sie braucht: Menüpunkt
**[4] Artefakt wählen** oder beim ersten Lauf, der ein Modell benötigt.
Von Hand geht es auch:

```bash
huggingface-cli download <hf_repo> --revision <hf_revision> \
    --local-dir INTEGER_LLM/models/<hf_verzeichnis>
```

## Wie daraus Artefakte werden

```bash
cd INTEGER_LLM
INTEGER_LLM_MODEL=<modell> python -m calibrate.src.main
```

Der Bau nutzt das versionierte Skalenpaket aus `scale_packs/<modell>/` und
ist damit **plattformübergreifend bitgleich**: Die Aktivierungsstatistik,
der einzige nichtdeterministische Schritt (Fund 32), entfällt. Er dauert
Sekunden statt Minuten.

## Zur Lizenzangabe

Die Spalte nennt, was die jeweilige Modellkarte angibt, ohne eigene
Rechtsprüfung. Die Lizenzlage **quantisierter Ableitungen** ist Gegenstand
einer separaten, nicht-technischen Klärung (`docs/01_licenses.md`) und im
Fahrplan als offener Punkt geführt.
