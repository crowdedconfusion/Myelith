#!/usr/bin/env python3
"""Faehigkeiten des Agenten in Szenarien mit Phasen: Erinnerung, Web, Skill, Werkzeug.

Ein Szenario ist ein Arbeitsordner und eine oder mehrere **Phasen** darin.
Jede Phase ist ein eigener Lauf von `myl agent` mit frischem Kontext und
eigener Abnahme. So laesst sich messen, was ein Lauf anlegt und ein
spaeterer nutzt: einen Skill aus einem PDF, ein Werkzeug als Manifest.

    python3 BENCHMARKS/Agent/faehigkeiten.py pruefen                 # Abnahmen in beide Richtungen
    python3 -u BENCHMARKS/Agent/faehigkeiten.py starten --modelle raven-35b-a3b-int4 \\
        --saaten 1,2,3 --mitschriften <ordner ausserhalb>
    python3 BENCHMARKS/Agent/faehigkeiten.py auswerten BENCHMARKS/Agent/results/faehigkeiten-<datum>

⚑ **Faellt eine Phase durch, bekommt die naechste die Musterloesung der
vorigen** und wird trotzdem gefahren, mit der Marke „mit Muster“. Sonst
saehe ein Skill, der nicht entstand, aus wie einer, der nicht genutzt
wurde, und die zweite Frage bliebe unbeantwortet.

⚑ **Der Lauf selbst steht nur in `agentenprobe.py`** (`im_ordner_fahren`):
Befehl, Saat, Einstellungen ab Werk und Urteil. Hier steht nur, was ein
Szenario mehr hat als ein Auftrag: die Phasen und was zwischen ihnen
geschieht (`danach`).
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HIER = Path(__file__).resolve().parent
sys.path.insert(0, str(HIER))
import agentenprobe  # noqa: E402

ARTEFAKTE = agentenprobe.WURZEL / "INTEGER_LLM" / "artifacts"
SATZ = HIER / "faehigkeiten.json"


def szenarien(nur: str = "") -> list[dict]:
    alle = json.loads(SATZ.read_text(encoding="utf-8"))["szenario"]
    namen = {n.strip() for n in nur.split(",") if n.strip()}
    return [s for s in alle if not namen or s["name"] in namen]


def dateien_schreiben(wurzel: Path, dateien: dict) -> None:
    for name, inhalt in (dateien or {}).items():
        (wurzel / name).parent.mkdir(parents=True, exist_ok=True)
        (wurzel / name).write_text(inhalt, encoding="utf-8")


def muster_anwenden(wurzel: Path, phase: dict) -> None:
    dateien_schreiben(wurzel, phase.get("_muster") or {})
    if phase.get("_muster_befehl"):
        subprocess.run(["sh", "-c", phase["_muster_befehl"]], cwd=wurzel, check=True, capture_output=True)


def abnahme(wurzel: Path, befehl: str) -> tuple[bool, str]:
    r = subprocess.run(["sh", "-c", befehl], cwd=wurzel, capture_output=True, text=True, timeout=120)
    return r.returncode == 0, (r.stdout + r.stderr).strip().replace("\n", " ")[-300:]


def danach(wurzel: Path, phase: dict) -> None:
    if phase.get("danach"):
        subprocess.run(["sh", "-c", phase["danach"]], cwd=wurzel, check=True, capture_output=True)


def pruefen(a: argparse.Namespace) -> int:
    """Jede Abnahme muss am Stand vor der Phase scheitern und mit dem Muster bestehen."""
    fehler = 0
    for s in szenarien(a.nur):
        with tempfile.TemporaryDirectory(prefix="myl-faehigkeiten-") as t:
            wurzel = Path(t)
            agentenprobe.ausgangsstand_anlegen(wurzel, s)
            for k, p in enumerate(s["phasen"], start=1):
                dateien_schreiben(wurzel, p.get("dateien"))
                vorher, _ = abnahme(wurzel, p["abnahme"])
                muster_anwenden(wurzel, p)
                nachher, grund = abnahme(wurzel, p["abnahme"])
                gut = (not vorher) and nachher
                fehler += not gut
                print(f"{'ok  ' if gut else 'FEHL'} {s['name']:24} Phase {k}: vorher {'besteht' if vorher else 'scheitert'}, "
                      f"Muster {'besteht' if nachher else 'scheitert: ' + grund}", flush=True)
                danach(wurzel, p)
    print("Fertig" if not fehler else f"FEHLER: {fehler} Abnahmen trennen nicht")
    return 1 if fehler else 0


def ein_szenario(artefakt: str, s: dict, saat: int, mitschriften: Path | None) -> list[dict]:
    ergebnisse = []
    with tempfile.TemporaryDirectory(prefix="myl-faehigkeiten-") as t:
        wurzel = Path(t)
        agentenprobe.ausgangsstand_anlegen(wurzel, s)
        hilfe = False
        for k, p in enumerate(s["phasen"], start=1):
            dateien_schreiben(wurzel, p.get("dateien"))
            auftrag = dict(p, name=f"{s['name']}#{k}")
            mitschrift = mitschriften / f"{s['name']}-p{k}-s{saat}.txt" if mitschriften else None
            gut, grund, dauer, werkzeuge = agentenprobe.im_ordner_fahren(
                wurzel, artefakt, auftrag, p.get("schritte", 60), False, True, p.get("frist", 3600),
                mitschrift, saat, 1600, "aus", True,
            )
            ergebnisse.append({
                "name": s["name"], "art": s.get("art", ""), "phase": k, "saat": saat, "bestanden": gut,
                "grund": grund, "sekunden": round(dauer, 1), "werkzeuge": werkzeuge, "mit_muster": hilfe,
            })
            print(f"[faehigkeiten] {'ok  ' if gut else 'NEIN'} {s['name']:24} Phase {k} s{saat} {dauer:7.1f} s"
                  f"{' (mit Muster)' if hilfe else ''}  [{' '.join(werkzeuge)}]{'' if gut else '  (' + grund[:200] + ')'}",
                  flush=True)
            if not gut:
                muster_anwenden(wurzel, p)
                hilfe = True
            danach(wurzel, p)
    return ergebnisse


def starten(a: argparse.Namespace) -> int:
    if not agentenprobe.MYL.is_file():
        print(f"[faehigkeiten] {agentenprobe.MYL} fehlt; erst `cargo build --release` im Client.")
        return 2
    datum = datetime.date.today().isoformat()
    ziel = a.ausgabe or HIER / "results" / f"faehigkeiten-{datum}"
    ziel.mkdir(parents=True, exist_ok=True)
    saaten = [int(x) for x in a.saaten.split(",") if x.strip()]
    modelle = [m.strip() for m in a.modelle.split(",") if m.strip()]
    lauf = {"begonnen": datetime.datetime.now().isoformat(timespec="seconds"), "modelle": modelle, "saaten": saaten,
            "token": 1600, "denken": "aus", "werkzeuge": "voll", "einstellungen": "ab Werk, je Phase ergaenzt",
            "nur": a.nur}
    (ziel / "lauf.json").write_text(json.dumps(lauf, indent=2) + "\n", encoding="utf-8")
    for m in modelle:
        anfang = time.monotonic()
        print(f"[faehigkeiten] === {m} ===", flush=True)
        alle = []
        for s in szenarien(a.nur):
            for saat in saaten:
                alle += ein_szenario(str(ARTEFAKTE / m), s, saat, a.mitschriften / m if a.mitschriften else None)
                (ziel / f"{m}.json").write_text(json.dumps({"laeufe": alle}, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        print(f"[faehigkeiten] {m} fertig nach {(time.monotonic() - anfang) / 60:.1f} min", flush=True)
        auswerten_ordner(ziel)
    print("[faehigkeiten] Fertig", flush=True)
    return 0


def auswerten_ordner(ziel: Path) -> Path:
    lauf = json.loads((ziel / "lauf.json").read_text(encoding="utf-8"))
    zeilen = [
        f"# Fähigkeiten des Agenten, {ziel.name.removeprefix('faehigkeiten-')}",
        "",
        f"Saaten: {', '.join(map(str, lauf['saaten']))}. Antwortlänge eingestellt 1600 (ein Agentenlauf nimmt "
        "mindestens 4000, mit großem Kontext mehr), Denken aus, Werkzeugsatz voll, "
        "Einstellungen ab Werk, je Phase ergänzt. Jede Phase ist ein eigener Lauf im selben Ordner; bestanden "
        "heißt: Die Abnahme von außen endet mit 0. „mit Muster“: Die Phase davor fiel durch und wurde durch "
        "ihre Musterlösung ersetzt.",
        "",
    ]
    for m in lauf["modelle"]:
        p = ziel / f"{m}.json"
        if not p.is_file():
            zeilen += [f"## {m}", "", "läuft oder steht aus", ""]
            continue
        laeufe = json.loads(p.read_text(encoding="utf-8"))["laeufe"]
        zeilen += [f"## {m}", "", f"**{sum(x['bestanden'] for x in laeufe)}/{len(laeufe)} Phasen** in "
                   f"{sum(x['sekunden'] for x in laeufe) / 60:.0f} Minuten", "",
                   "| Szenario | Art | Phase | Saaten | Sekunden | gerufen (erste Saat) | Grund, falls nicht |",
                   "|---|---|---|---|---|---|---|"]
        schluessel = sorted({(x["name"], x["phase"]) for x in laeufe}, key=lambda k: [x["name"] for x in laeufe].index(k[0]) * 10 + k[1])
        for name, phase in schluessel:
            w = [x for x in laeufe if x["name"] == name and x["phase"] == phase]
            marken = " ".join(("✓" if x["bestanden"] else "✗") + ("ᴹ" if x["mit_muster"] else "") for x in w)
            werkzeuge = ", ".join(sorted(set(w[0]["werkzeuge"])))
            gruende = "; ".join(sorted({x["grund"][:120] for x in w if x["grund"]}))
            sekunden = " / ".join(f"{x['sekunden']:.0f}" for x in w)
            zeilen.append(f"| `{name}` | {w[0]['art']} | {phase} | {marken} | {sekunden} | {werkzeuge} | {gruende} |")
        zeilen.append("")
    zeilen.append("ᴹ: mit Muster (die Phase davor fiel durch).")
    bericht = ziel.parent / f"{ziel.name}.md"
    bericht.write_text("\n".join(zeilen) + "\n", encoding="utf-8")
    return bericht


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    u = p.add_subparsers(dest="was", required=True)
    pr = u.add_parser("pruefen")
    pr.add_argument("--nur", default="")
    s = u.add_parser("starten")
    s.add_argument("--modelle", default="raven-35b-a3b-int4")
    s.add_argument("--saaten", required=True, help="feste Saaten durch Komma")
    s.add_argument("--nur", default="", help="nur diese Szenarien, durch Komma")
    s.add_argument("--ausgabe", type=Path)
    s.add_argument("--mitschriften", type=Path, help="Ordner fuer die ganze Ausgabe je Phase (nicht ins Repositorium)")
    w = u.add_parser("auswerten")
    w.add_argument("ordner", type=Path)
    a = p.parse_args()
    if a.was == "pruefen":
        return pruefen(a)
    if a.was == "starten":
        return starten(a)
    print(auswerten_ordner(a.ordner))
    return 0


if __name__ == "__main__":
    os.environ.setdefault("PYTHONIOENCODING", "utf-8")
    sys.exit(main())
