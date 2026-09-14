#!/usr/bin/env python3
"""Modellkarten-Generator (Punkt 1.1, Manifest G4/G7).

Erzeugt je θ_v-Fassung eine Karte aus `theta_v/spec.json` und
`BENCHMARKS/Inferenz/results/`. **Nicht von Hand gepflegt**, und das ist der Punkt.

## ⚑ Warum erzeugt und nicht geschrieben

Eine Modellkarte, die jemand von Hand nachträgt, ist genau so aktuell
wie seine Erinnerung. Eine Durchsicht am 2026-08-29 fand in diesem
Repositorium zehn falsch gesetzte Haken und sechs Versionsnummern, die
vor ihrem eigenen Changelog lagen: **alle von Hand gepflegt, alle
irgendwann auseinandergelaufen.**

Eine erzeugte Karte kann veralten, aber sie kann nicht **falsch** sein:
Sie sagt, was in den Dateien steht, und wenn dort nichts steht, sagt sie
das.

## ⚑ Leerstellen bleiben leer

Was nicht gemessen ist, wird als „nicht gemessen" ausgewiesen und nicht
weggelassen. Eine Karte ohne Zeile für die Kalibrierung liest sich, als
gäbe es die Frage nicht; eine Karte mit einer leeren Zeile liest sich,
wie es ist.

Aufruf: `python3 ETHICS/werkzeuge/modellkarte.py [--pruefe]`
`--pruefe` schreibt nichts, sondern meldet, ob die abgelegte Karte
noch dem entspricht, was die Quellen hergeben. Für den CI-Lauf.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SPEC = REPO / "INTEGER_LLM" / "theta_v" / "spec.json"
# 📌 Fund 357 (2026-09-14): Hier stand `INTEGER_LLM/eval/results`, und
# der Ordner ist am 2026-09-07 umgezogen. Die Karte meldete seither
# „keine Datei", waehrend daneben die Messungen lagen. Dazu lasen vier
# von fuenf Zeilen der Ausfuehrungsspezifikation Schluessel, die es in
# `spec.json` nie gab (`numeric.weight_dtype` statt
# `numeric.formats.weight.dtype`). **Eine ehrliche Leerstelle ist nur
# ehrlich, wenn an der richtigen Stelle gesucht wurde.**
ERGEBNISSE = REPO / "BENCHMARKS" / "Inferenz" / "results"
KATALOG = REPO / "INTEGER_LLM" / "models" / "KATALOG.json"
ZIEL = REPO / "ETHICS" / "Modellkarte.md"

NICHT_GEMESSEN = "*nicht gemessen*"


def lade_spec() -> dict:
    return json.loads(SPEC.read_text(encoding="utf-8"))["theta_v"]


def lade_messungen() -> dict[str, dict]:
    """Die Vergleichsmessung je eingesetztem Modell.

    Eingesetzt ist, was `KATALOG.json` fuehrt. Messungen entfernter
    Modelle bleiben als Aufzeichnung unter `results/` liegen, gehoeren
    aber nicht auf die Karte der Modelle, die das Netz rechnet.
    Die Datei heisst nach dem Modellnamen ohne Punkt
    (`myelith-0.6b` wird `perplexity_comparison_myelith-06b.json`).
    """
    aus: dict[str, dict] = {}
    if not KATALOG.is_file():
        return aus
    katalog = json.loads(KATALOG.read_text(encoding="utf-8"))
    for modell in sorted(k for k in katalog if not k.startswith("_")):
        kurz = modell.replace(".", "")
        vergleich = ERGEBNISSE / f"perplexity_comparison_{kurz}.json"
        grundlinie = ERGEBNISSE / f"baseline_wikitext2_{kurz}.json"
        if not vergleich.is_file():
            continue
        try:
            d = json.loads(vergleich.read_text(encoding="utf-8"))
            g = json.loads(grundlinie.read_text(encoding="utf-8")) if grundlinie.is_file() else {}
        except json.JSONDecodeError:
            continue
        aus[modell] = {**d, "dataset": g.get("dataset"), "quelle": vergleich.name}
    return aus


def wert(d: dict, *pfad: str) -> str:
    """Ein Wert aus dem Spec-Baum, oder die ehrliche Leerstelle."""
    stelle = d
    for k in pfad:
        if not isinstance(stelle, dict) or k not in stelle:
            return NICHT_GEMESSEN
        stelle = stelle[k]
    return f"`{stelle}`" if not isinstance(stelle, (dict, list)) else NICHT_GEMESSEN


def baue() -> str:
    spec = lade_spec()
    messungen = lade_messungen()
    z: list[str] = []
    a = z.append

    a("# Modellkarte (erzeugt)")
    a("")
    a("> ⚑ **Diese Datei wird erzeugt, nicht geschrieben.**")
    a("> Quelle: `INTEGER_LLM/theta_v/spec.json` und")
    a("> `BENCHMARKS/Inferenz/results/`. Wer sie von Hand ändert, verliert")
    a("> die Änderung beim nächsten Lauf von")
    a("> `ETHICS/werkzeuge/modellkarte.py`.")
    a("")
    a(f"**θ_v-Fassung:** `{spec.get('version', '?')}`")
    a("")
    a("## Ausführungsspezifikation")
    a("")
    a("| Feld | Wert |")
    a("|---|---|")
    a(f"| Zahlenformat der Gewichte | {wert(spec, 'numeric', 'formats', 'weight', 'dtype')} |")
    a(f"| Zahlenformat der Aktivierungen | {wert(spec, 'numeric', 'formats', 'activation', 'dtype')} |")
    a(f"| Akkumulator | {wert(spec, 'numeric', 'formats', 'accumulator', 'dtype')} |")
    nichtlinear = spec.get("nonlinear", {})
    verfahren = [f"{name}: `{d['method']}`" for name, d in nichtlinear.items()
                 if isinstance(d, dict) and "method" in d]
    a(f"| Nichtlinearitäten | {', '.join(verfahren) if verfahren else NICHT_GEMESSEN} |")
    a(f"| Abtastung | {wert(spec, 'sampling', 'method')} |")
    a("")
    a("## Gemessene Qualität gegen die Gleitkomma-Referenz")
    a("")
    if not messungen:
        a(NICHT_GEMESSEN + ", keine Datei unter `BENCHMARKS/Inferenz/results/`.")
    else:
        a("| Modell | Datensatz | Positionen | ganzzahlig | Gleitkomma | Abstand | Quelle |")
        a("|---|---|---|---|---|---|---|")
        for modell, d in sorted(messungen.items()):
            def zahl(k: str, nachkomma: int) -> str:
                v = d.get(k)
                return f"{v:.{nachkomma}f}".replace(".", ",") if isinstance(v, (int, float)) else NICHT_GEMESSEN
            a(
                f"| {modell} | {d.get('dataset') or NICHT_GEMESSEN} | "
                f"{d.get('evaluated_tokens', NICHT_GEMESSEN)} | {zahl('integer_perplexity', 2)} | "
                f"{zahl('baseline_perplexity', 2)} | {zahl('delta_pct', 2)} % | `{d['quelle']}` |"
            )
    a("")
    a("## Was diese Karte nicht sagt")
    a("")
    a("⚑ **Sie sagt nichts über Eignung.** Wofür das Netz geeignet ist")
    a("und wofür nicht, steht in `ETHICS/Risikoklassen.toml`; das ist eine")
    a("Aussage über Vertraulichkeit und keine über Qualität.")
    a("")
    a("⚑ **Und sie bewertet den Inhalt der Trainingsdaten nicht.**")
    a("Grundsatz G1 verbietet das ausdrücklich: Geprüft wird die Herkunft")
    a("eines Korpus, nicht seine Meinung. Die Herkunft steht im")
    a("Aufnahmeantrag, nicht hier.")
    return "\n".join(z) + "\n"


def main() -> int:
    if not SPEC.is_file():
        print(f"[modellkarte] FEHLGESCHLAGEN: {SPEC.relative_to(REPO)} fehlt")
        return 1
    neu = baue()
    if "--pruefe" in sys.argv:
        alt = ZIEL.read_text(encoding="utf-8") if ZIEL.is_file() else ""
        if alt == neu:
            print("[modellkarte] PASSED: die abgelegte Karte entspricht den Quellen")
            return 0
        print("[modellkarte] FEHLGESCHLAGEN: die abgelegte Karte ist nicht mehr die erzeugte")
        print("[modellkarte] `python3 ETHICS/werkzeuge/modellkarte.py` schreibt sie neu")
        return 1
    ZIEL.write_text(neu, encoding="utf-8")
    print(f"[modellkarte] geschrieben: {ZIEL.relative_to(REPO)} ({len(neu.splitlines())} Zeilen)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
