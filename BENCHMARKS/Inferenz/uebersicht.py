#!/usr/bin/env python3
"""Erzeugt `results/UEBERSICHT.md`: die ganze Modellreihe auf einer Seite.

# ⚑ Warum es diese Datei gibt (2026-09-11)

`results/` enthaelt je Modell ein Entscheidungsprotokoll
(`decision_12-21_*.md`) und einen Vergleich
(`perplexity_comparison_*.json`). **Das ist richtig so**: Jedes einzelne
ist der Beleg seines Modells, und sie zu einem zusammenzufassen hiesse,
den Beleg zu verlieren.

**Falsch war, dass es keine Uebersicht gab.** Wer wissen wollte, wie die
Reihe insgesamt dasteht, musste sechs Dateien oeffnen und die Zahlen im
Kopf nebeneinanderlegen. Diese Datei macht das einmal und maschinell.

⚠️ **Von Hand gepflegt waere sie in einer Woche falsch.** Sie entsteht
aus denselben JSON-Dateien, die die Messung schreibt, und traegt das im
Kopf. Wer sie bearbeitet, arbeitet umsonst.

Aufruf aus der Wurzel:

    python3 BENCHMARKS/Inferenz/uebersicht.py
"""

from __future__ import annotations

import json
from datetime import date
from pathlib import Path

HIER = Path(__file__).resolve().parent
ERGEBNISSE = HIER / "results"
KATALOG = HIER.parents[1] / "INTEGER_LLM" / "models" / "KATALOG.json"

# ⚑ **Die Reihenfolge kommt aus dem Katalog, nicht aus dem Dateisystem.**
# `reihung` ist die Parameterzahl in Milliarden, also die Achse, um die
# es geht. Alphabetisch stuende `myelith-14b` vor `myelith-4b`.
ZIEL = ERGEBNISSE / "UEBERSICHT.md"


def _zahl(x: float) -> str:
    """Deutsche Schreibweise mit Komma."""
    return f"{x:.2f}".replace(".", ",")


def _abstand(p: float) -> str:
    """Der Abstand, und bei zu kleinem Betrag die ehrliche Auskunft.

    ⚑ **Bei 435 Positionen ist ein halbes Prozent nicht aufloesbar.**
    Ein Vorzeichen unterhalb dieser Schwelle als Ergebnis auszuweisen
    waere eine Behauptung ueber Rauschen.
    """
    if abs(p) < 1.0:
        return "**kein messbarer Abstand**"
    return f"**{'+' if p > 0 else '−'}{_zahl(abs(p))} %**"


def _eintraege() -> list[tuple[str, dict, dict]]:
    katalog = json.loads(KATALOG.read_text(encoding="utf-8"))
    modelle = [(k, v) for k, v in katalog.items() if not k.startswith("_")]
    modelle.sort(key=lambda kv: kv[1].get("reihung", 0.0))

    raus = []
    for schluessel, eintrag in modelle:
        # Der Dateiname entsteht wie in `wikitext_common.ergebnis_pfad`.
        suffix = f"_{schluessel.replace('.', '')}"
        datei = ERGEBNISSE / f"perplexity_comparison{suffix}.json"
        # ⛔️ **Kein Rueckfall auf die namenlose Datei.**
        #
        # Die erste Fassung dieses Erzeugers versuchte
        # `perplexity_comparison.json`, wenn die modellbezogene fehlte.
        # Diese Datei gehoert aber Qwen2.5-0,5B, und der Rueckfall traf
        # deshalb **jedes** Modell ohne eigene Messung: In der ersten
        # erzeugten Uebersicht standen bei 4B und 30B-A3B die Zahlen der
        # 0,5B, dreimal dieselbe Zeile, ohne dass etwas darauf hinwies.
        #
        # ⚑ **Ein fehlender Wert wird benannt, nicht ersetzt.** Dasselbe
        # Muster wie Fund 340 und Fund 341: Ein Rueckfall auf einen
        # anderen Gegenstand ist schlimmer als eine Luecke, weil eine
        # Luecke auffaellt.
        raus.append((schluessel, eintrag, json.loads(datei.read_text()) if datei.exists() else None))
    return raus


def bauen() -> str:
    eintraege = _eintraege()
    zeilen = [
        "# Perplexität der Modellreihe",
        "",
        "> ⚙️ **Erzeugt von `BENCHMARKS/Inferenz/uebersicht.py`.**",
        "> Nicht von Hand bearbeiten: Der nächste Lauf überschreibt die Datei.",
        f"> Stand: {date.today().isoformat()}",
        "",
        "Gemessen wird Perplexität auf WikiText-2 mit Teacher-Forcing, für",
        "beide Pfade auf **identischen Sequenzen**; niedriger ist besser.",
        "Der **Abstand** ist der relative Aufschlag des Ganzzahlpfads auf",
        "seine eigene Gleitkomma-Referenz, nicht auf ein anderes Modell.",
        "",
        "| Modell | Gleitkomma | Ganzzahl | Positionen | Abstand | Kriterium ≤ 5 % |",
        "|---|---|---|---|---|---|",
    ]
    for schluessel, eintrag, v in eintraege:
        name = eintrag.get("anzeigename", schluessel)
        if v is None:
            zeilen.append(f"| {name} | | | | **nicht gemessen** | |")
            continue
        zeilen.append(
            f"| {name} | {_zahl(v['baseline_perplexity'])} "
            f"| {_zahl(v['integer_perplexity'])} "
            f"| {v.get('evaluated_tokens', '?')} "
            f"| {_abstand(v['delta_pct'])} "
            f"| {'erfüllt' if v.get('accepted') else '**verfehlt**'} |"
        )

    zeilen += [
        "",
        "## Was daraus folgt",
        "",
        "⚑ **Je kleiner das Modell, desto teurer die Quantisierung**, und",
        "das ist keine Vermutung mehr, sondern über die ganze Reihe",
        "gemessen. Das kleinste Modell liegt als einziges nennenswert nahe",
        "an der Grenze; wer es wählt, wählt auch das, bei dem die",
        "Quantisierung am meisten kostet.",
        "",
        "⚠️ **Was diese Tabelle nicht sagt.** Die Zahlen einer Zeile lassen",
        "sich mit denen einer anderen **nicht** vergleichen: Jede misst",
        "gegen ihre eigene Referenz. Ein Modell mit 33,29 ist nicht",
        "schlechter als eines mit 11,54, sondern kleiner.",
        "",
        "⚠️ **Der Boden des Quantisierungsschemas fehlt hier.** Er ist an",
        "einem Modell gemessen, das nicht mehr Teil des Projekts ist, und",
        "für diese Reihe nicht neu bestimmt.",
        "",
        "## Wo die Einzelbelege stehen",
        "",
        "Je Modell ein Entscheidungsprotokoll und ein Vergleich, beide",
        "erzeugt von `perplexity.py`:",
        "",
    ]
    for schluessel, eintrag, _ in eintraege:
        suffix = f"_{schluessel.replace('.', '')}"
        zeilen.append(
            f"- **{eintrag.get('anzeigename', schluessel)}**: "
            f"`decision_12-21{suffix}.md`, `perplexity_comparison{suffix}.json`"
        )

    zeilen += [
        "",
        "⚑ **Die Einzeldateien bleiben, und das ist Absicht.** Jede ist der",
        "Beleg ihres Modells, mit Methode, Datensatz und Einordnung. Diese",
        "Übersicht ersetzt sie nicht, sie erspart nur das Nebeneinanderlegen.",
        "",
        "⚠️ **Ältere Dateien ohne Modellnamen** (`decision_12-21.md`,",
        "`baseline_wikitext2.json`, `perplexity_comparison.json`) gehören zu",
        "Qwen2.5-0,5B und bleiben als Aufzeichnung stehen. Das Modell ist",
        "seit dem 2026-09-11 aus dem Projekt heraus; seine Messung ist in",
        "Whitepaper-Vorarbeit und Changelog zitiert.",
        "",
    ]
    return "\n".join(zeilen)


def main() -> None:
    ZIEL.parent.mkdir(parents=True, exist_ok=True)
    ZIEL.write_text(bauen(), encoding="utf-8")
    print(f"Geschrieben: {ZIEL.relative_to(HIER.parents[1])}")


if __name__ == "__main__":
    main()
