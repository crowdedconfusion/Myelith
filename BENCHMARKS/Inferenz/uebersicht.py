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
KATALOG = HIER.parents[1] / "MODELS" / "llm" / "KATALOG.json"

# ⚑ **Die Reihenfolge kommt aus dem Katalog, nicht aus dem Dateisystem.**
# `reihung` ist die Parameterzahl in Milliarden, also die Achse, um die
# es geht. Alphabetisch stuende `myelith-14b` vor `myelith-4b`.
ZIEL = ERGEBNISSE / "UEBERSICHT.md"


def _zahl(x: float) -> str:
    """Deutsche Schreibweise mit Komma."""
    return f"{x:.2f}".replace(".", ",")


def _prozent(p: float) -> str:
    """Vorzeichenbehaftet, deutsch, mit echtem Minuszeichen.

    📌 **Hier stand ein festes Pluszeichen** in der Bodenspalte, und der
    erste negative Boden (4B, das Schema kostet nichts) waere als
    `+-2,45 %` erschienen. Ein Vorzeichen gehoert gerechnet, nicht
    angenommen.
    """
    return f"{'+' if p >= 0 else '−'}{_zahl(abs(p))} %"


def _abstand(p: float) -> str:
    """Der Abstand, und bei zu kleinem Betrag die ehrliche Auskunft.

    ⚑ **Bei 435 Positionen ist ein halbes Prozent nicht aufloesbar.**
    Ein Vorzeichen unterhalb dieser Schwelle als Ergebnis auszuweisen
    waere eine Behauptung ueber Rauschen.
    """
    if abs(p) < 1.0:
        return "**kein messbarer Abstand**"
    return f"**{'+' if p > 0 else '−'}{_zahl(abs(p))} %**"


def _eintraege() -> list[tuple[str, dict, dict | None, dict | None]]:
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
        #
        # Der Boden des Schemas, falls er fuer dieses Modell gemessen ist
        # (`tests/diag/w8a16_reference_simulation.py`). Fehlt er, bleibt
        # die Zelle leer: derselbe Grundsatz wie oben, eine Luecke wird
        # benannt und nicht gefuellt.
        boden_datei = ERGEBNISSE / f"schema_boden{suffix}.json"
        raus.append((
            schluessel,
            eintrag,
            json.loads(datei.read_text()) if datei.exists() else None,
            json.loads(boden_datei.read_text()) if boden_datei.exists() else None,
        ))
    return raus


def _abgeloeste(lebende: set[str]) -> list[tuple[str, str, dict, dict | None]]:
    """Gemessene Reihen, deren Modell nicht mehr im Katalog steht.

    ⚑ **Eine Messung verfällt nicht, weil das Modell geht** (Festlegung
    des Projektinhabers, 2026-09-15). Qwen2.5-0,5B, Qwen2.5-7B und das
    dichte 14B sind aus dem Projekt heraus, ihre Zahlen sind trotzdem
    gemessen und in Whitepaper-Vorarbeit und Changelog zitiert. Sie lagen
    bis heute nur als Dateien da: Der Katalog kennt sie nicht, also kam
    die Uebersicht nie an ihnen vorbei, und wer sie suchte, musste wissen,
    dass es sie gibt.

    ⚑ **Gefunden wird auf der Platte, nicht in einer Liste hier.** Eine
    Liste im Skript waere eine zweite Stelle, die beim naechsten
    Abloesen nachgezogen werden muesste, und genau die zieht niemand
    nach. Der Name kommt aus der Grundlinie, die ihn selbst traegt.
    """
    raus = []
    for datei in sorted(ERGEBNISSE.glob("perplexity_comparison*.json")):
        suffix = datei.stem[len("perplexity_comparison"):]
        if suffix.lstrip("_") in lebende:
            continue
        grundlinie = ERGEBNISSE / f"baseline_wikitext2{suffix}.json"
        name = suffix.lstrip("_") or "ohne Modellnamen"
        if grundlinie.exists():
            name = json.loads(grundlinie.read_text()).get("model", name)
            name = name.replace(" (HF, BF16)", "")
        boden = ERGEBNISSE / f"schema_boden{suffix}.json"
        raus.append((
            name,
            suffix,
            json.loads(datei.read_text()),
            json.loads(boden.read_text()) if boden.exists() else None,
        ))
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
    for schluessel, eintrag, v, b in eintraege:
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
        "⚑ **Je kleiner das Modell, desto grösser der gemessene Abstand**,",
        "und das ist keine Vermutung mehr, sondern über die ganze Reihe",
        "gemessen. Das kleinste Modell liegt als einziges nennenswert nahe",
        "an der Grenze; wer es wählt, wählt auch das, bei dem der Abstand",
        "am grössten ist.",
        "",
        "⛔️ **Der Abstand allein sagt nicht, woher er kommt.** Wieviel",
        "das Quantisierungsschema selbst kostet, steht im nächsten",
        "Abschnitt; der Rest ist Umsetzung.",
        "",
    ]

    boeden = []
    for schluessel, eintrag, _v, _b in eintraege:
        suffix = f"_{schluessel.replace('.', '')}"
        for datei in sorted(ERGEBNISSE.glob(f"schema_boden*{suffix}.json")):
            boeden.append((eintrag.get("anzeigename", schluessel), json.loads(datei.read_text())))
    if boeden:
        zeilen += [
            "",
            "## Der Boden des Schemas",
            "",
            "⚑ **Was das Verfahren selbst kostet**, gemessen mit W8A16 und",
            "sonst Gleitkomma (`INTEGER_LLM/tests/diag/w8a16_reference_simulation.py`).",
            "Der Rest des Abstands oben ist Umsetzung.",
            "",
            "⛔️ **Die Spalte mit den Positionen entscheidet, ob eine Zeile",
            "mit der Tabelle oben verrechnet werden darf.** Wer 13 797 gegen",
            "435 rechnet, rechnet über verschiedenen Text, und das Ergebnis",
            "wäre erfunden; das Messwerkzeug weigert sich deshalb.",
            "**Für einen belastbaren Umsetzungsverlust muss der Ganzzahlpfad",
            "über denselben Umfang laufen wie der Boden**, und über 13 797",
            "Positionen steht er noch aus.",
            "",
            "⛔️ **Ein negativer Boden ist kein Ergebnis, sondern eine",
            "Meldung über die Stichprobe.** Quantisierung vernichtet",
            "Information; dass ein quantisiertes Modell besser vorhersagt",
            "als seine eigene Gleitkomma-Referenz, kann nicht sein. Über",
            "435 Positionen steht er beim 4B bei −2,45 % und beim",
            "30B-Gemisch bei −0,43 %; beim 4B blieb über 13 797 Positionen",
            "davon −0,07 %. **Wo das Vorzeichen negativ ist, trägt die Zeile",
            "keine Aussage**, auch wenn ihre Stichprobe zur Tabelle oben",
            "passt.",
            "",
            "📌 **Warum so viele Positionen.** Ein einzelnes Token, dessen",
            "Wahrscheinlichkeit um eine Grössenordnung springt, verschiebt",
            "bei 435 Positionen die Perplexität schon um ein halbes Prozent.",
            "Unterschiede unter rund einem Prozent tragen deshalb keine",
            "Information und sind über verschiedene Sequenzmengen nicht",
            "einmal monoton.",
            "",
            "| Modell | Positionen | Gleitkomma | nur Gewichte (W8) | Boden (W8A16) |",
            "|---|---|---|---|---|",
        ]
        for name, d in boeden:
            zeilen.append(
                f"| {name} | {d['ausgewertete_positionen']} "
                f"| {_zahl(d['baseline_perplexitaet'])} "
                f"| {_zahl(d['nur_gewichte_perplexitaet'])} "
                f"| {_zahl(d['boden_perplexitaet'])} ({_prozent(d['boden_prozent'])}) |"
            )
        zeilen.append("")

    zeilen += [
        "## Wo die Einzelbelege stehen",
        "",
        "Je Modell ein Entscheidungsprotokoll und ein Vergleich, beide",
        "erzeugt von `perplexity.py`:",
        "",
    ]
    for schluessel, eintrag, _, _ in eintraege:
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
    ]

    # 📌 **Verglichen wird in der Form des Dateinamens, nicht in der des
    # Katalogs.** Hier stand der blanke Katalogschluessel, und der traegt
    # den Punkt (`myelith-0.6b`), den `ergebnis_pfad` aus dem Suffix
    # entfernt (`myelith-06b`). Kein lebendes Modell traf sich selbst, und
    # das 0,6B tauchte prompt unter den abgeloesten auf. Dieselbe Angabe
    # in zwei Schreibweisen, wie sie dieses Projekt schon oefter hatte.
    abgeloest = _abgeloeste({s.replace(".", "") for s, _, _, _ in eintraege})
    if abgeloest:
        zeilen += [
            "## Abgelöste Modelle (Aufzeichnung)",
            "",
            "⚑ **Eine Messung verfällt nicht, weil das Modell geht.** Diese",
            "Größen sind nicht mehr Teil des Projekts; ihre Zahlen sind",
            "gemessen, mit derselben Methode und auf denselben 435",
            "Positionen, und in Whitepaper-Vorarbeit und Changelog zitiert.",
            "Sie stehen hier, damit man sie findet, ohne zu wissen, dass es",
            "sie gibt.",
            "",
            "⛔️ **Nicht mit der Tabelle oben verrechnen.** Zwei der drei",
            "stammen aus einer anderen Modellfamilie (Qwen2.5); ein",
            "Größenvergleich über die Grenze hinweg misst den",
            "Familienunterschied mit.",
            "",
            "| Modell | Gleitkomma | Ganzzahl | Positionen | Abstand | Boden des Schemas |",
            "|---|---|---|---|---|---|",
        ]
        for name, _suffix, v, b in abgeloest:
            boden = _prozent(b["boden_prozent"]) if b else "*keine Datei*"
            zeilen.append(
                f"| {name} | {_zahl(v['baseline_perplexity'])} "
                f"| {_zahl(v['integer_perplexity'])} "
                f"| {v.get('evaluated_tokens', '?')} "
                f"| {_abstand(v['delta_pct'])} "
                f"| {boden} |"
            )
        zeilen += [
            "",
            "⚠️ **Der Boden von Qwen2.5-7B (+0,84 %) hat keine Ergebnisdatei.**",
            "Er wurde am 2026-08-20 gemessen, als das Werkzeug sein Ergebnis",
            "nur ausgab und nicht ablegte; die Zahl steht im Ergebnisblock",
            "von `INTEGER_LLM/README/README.md` und gilt dort. Hier bleibt",
            "die Zelle leer, statt eine Datei zu erfinden, die es nicht gibt.",
            "",
            "⚠️ **Die Dateien ohne Modellnamen** (`decision_12-21.md`,",
            "`baseline_wikitext2.json`, `perplexity_comparison.json`) gehören",
            "zu Qwen2.5-0,5B und behalten ihre Namen mit Absicht: Sie sind",
            "unter diesen Namen zitiert.",
            "",
        ]

    return "\n".join(zeilen)


def main() -> None:
    ZIEL.parent.mkdir(parents=True, exist_ok=True)
    ZIEL.write_text(bauen(), encoding="utf-8")
    print(f"Geschrieben: {ZIEL.relative_to(HIER.parents[1])}")


if __name__ == "__main__":
    main()
