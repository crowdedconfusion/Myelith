#!/usr/bin/env python3
"""Erzeugt models/README.md aus KATALOG.json und REGISTER.json.

WARUM ERZEUGT UND NICHT VON HAND: Die Modellangaben standen an drei
Stellen. In `models/README.md` als Tabelle, in `scale_packs/REGISTER.json`
als Digest, und in `myl-testclient/src/artefakte.rs` als `match`-Ausdruck
mit stillem Rueckfall auf Qwen2.5-0,5B. Drei Stellen laufen auseinander;
die dritte hatte den unangenehmsten Fehler, denn ein drittes Modell haette
dort die falschen Gewichte geladen.

Jetzt gibt es zwei Quellen, und beide sind es aus einem Grund:

    models/KATALOG.json          KURATIERT: Herkunft, Revision, Lizenz,
                                 Status. Aus keinem Artefakt ableitbar.
    scale_packs/REGISTER.json    ERZEUGT: Digest und theta_v-Stand.
                                 Aus den Artefakten gerechnet.

Diese Datei fuehrt beide zusammen. Wer die Tabelle von Hand bearbeitet,
verliert seine Aenderung beim naechsten Lauf: Das ist Absicht.

Usage:
    python tools/modelle_liste.py            schreibt models/README.md
    python tools/modelle_liste.py --pruefen  meldet nur, ob sie aktuell ist
"""
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
KATALOG = REPO / "models" / "KATALOG.json"
REGISTER = REPO / "scale_packs" / "REGISTER.json"
ZIEL = REPO / "models" / "README.md"

KOPF = """# models/

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
"""

FUSS = """
## Woher die Gewichte kommen

Der Testclient holt sie selbst, wenn er sie braucht: Menüpunkt
**[4] Artefakt wählen** oder beim ersten Lauf, der ein Modell benötigt.
Von Hand geht es auch:

```bash
huggingface-cli download <hf_repo> --revision <hf_revision> \\
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
"""


def zeile(name: str, k: dict, r: dict) -> str:
    repo_link = f"[{k['hf_repo']}](https://huggingface.co/{k['hf_repo']})"
    rev = k.get("hf_revision", "")
    return (
        f"| `{name}` | {repo_link} | `{rev[:12]}…` | {k.get('lizenz','')} | "
        f"{k.get('parameter','')} | {k.get('layer','')} | "
        f"{k.get('gewichte_anzeige','')} | {k.get('artefakt_anzeige','')} | "
        f"{r.get('theta_v','—')} | {k.get('status','')} |"
    )


def bauen() -> str:
    katalog = json.loads(KATALOG.read_text(encoding="utf-8"))
    register = json.loads(REGISTER.read_text(encoding="utf-8"))
    modelle = {k: v for k, v in katalog.items() if not k.startswith("_")}

    t = [KOPF, "\n## Modelle\n"]
    t.append("| Modell | Hugging Face | Revision | Lizenz | Parameter | Layer "
             "| Gewichte | Artefakt | θ_v | Status |")
    t.append("|---|---|---|---|---|---|---|---|---|---|")
    for name in sorted(modelle):
        t.append(zeile(name, modelle[name], register.get(name, {})))

    t.append("\n**Status:**\n")
    for wert, bedeutung in katalog.get("_status_bedeutung", {}).items():
        t.append(f"- **{wert}**: {bedeutung}")

    t.append("\n**Gemessene Qualität** (Perplexität, WikiText-2):\n")
    for name in sorted(modelle):
        pp = modelle[name].get("perplexitaet")
        if pp:
            t.append(f"- `{name}`: {pp}")

    t.append("\n**Anmerkungen:**\n")
    for name in sorted(modelle):
        bem = modelle[name].get("bemerkung", "").replace("\n", " ")
        if bem:
            t.append(f"- `{name}`: {bem}")

    t.append(FUSS)
    return "\n".join(t)


def main() -> int:
    neu = bauen()
    if "--pruefen" in sys.argv:
        alt = ZIEL.read_text(encoding="utf-8") if ZIEL.is_file() else ""
        if alt == neu:
            print(f"{ZIEL.relative_to(REPO)} ist aktuell.")
            return 0
        print(f"{ZIEL.relative_to(REPO)} ist NICHT aktuell. "
              "Erzeugen mit: python tools/modelle_liste.py")
        return 1
    ZIEL.write_text(neu, encoding="utf-8")
    print(f"Geschrieben: {ZIEL.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
