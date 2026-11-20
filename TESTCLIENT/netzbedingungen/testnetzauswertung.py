#!/usr/bin/env python3
"""Auswertung eines Laufs von `testnetzlauf.py`.

Liest die Protokolle der Knoten (`<aus>/<knoten>/logs/*.jsonl`), die
Messungen (`messung.jsonl`) und die Stoerungen (`ereignisse.jsonl`) und
schreibt `auswertung.md` in denselben Ordner:

- **Einigkeit:** Endhoehe und Zustandswurzel je Knoten; Hoehen, an denen
  verschiedene Bloecke uebernommen oder erzeugt wurden (Gabeln), und ob am
  Ende jeder auf demselben Zweig steht.
- **Erzeugung:** Bloecke je Erzeuger, Ablehnungen nach Art, Zweigwechsel,
  Wartezeiten vor dem Erzeugen.
- **Finalitaet:** bestaetigter Pruefpunkt gegen die Hoehe, ueber die Zeit.
- **Stoerungen:** je Rueckkehr, Heilung und Einsteiger die Zeit, bis der
  Knoten den Kopf des Netzes erreicht.
- **Ressourcen:** Speicher und CPU je Knoten, erste gegen letzte Stunde;
  Wachstum von Kette und Protokoll je Stunde.

Aufruf: `python3 TESTCLIENT/netzbedingungen/testnetzauswertung.py --aus <ordner>`
"""
from __future__ import annotations

import argparse
import collections
import json
import pathlib
import re
import statistics
import sys


def zeilen(p: pathlib.Path):
    with open(p, encoding="utf-8", errors="replace") as h:
        for z in h:
            try:
                yield json.loads(z)
            except json.JSONDecodeError:
                continue


def mib(text: str) -> float | None:
    m = re.match(r"([\d.]+)\s*([KMG]i?B)", text or "")
    if not m:
        return None
    zahl, einheit = float(m.group(1)), m.group(2)
    return zahl * {"KiB": 1 / 1024, "MiB": 1, "GiB": 1024, "KB": 1 / 1000, "MB": 1, "GB": 1000}.get(einheit, 1)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--aus", type=pathlib.Path, required=True)
    args = ap.parse_args()
    aus = args.aus.resolve()
    knoten = sorted(p.name for p in aus.iterdir() if (p / "logs").is_dir())
    ereignisse = list(zeilen(aus / "ereignisse.jsonl")) if (aus / "ereignisse.jsonl").exists() else []
    messungen = list(zeilen(aus / "messung.jsonl")) if (aus / "messung.jsonl").exists() else []
    t0 = ereignisse[0]["zeit_ms"] if ereignisse else 0

    bloecke = collections.defaultdict(set)          # hoehe -> {block}
    zeitachse = []                                   # (zeit_ms, knoten, hoehe)
    je = {}
    for n in knoten:
        z = collections.Counter()
        abgelehnt = collections.Counter()
        warten = []
        mesh = []                                   # (bloecke, konsens) je Aufnahme
        fuehler = collections.Counter()             # gelungen / gescheitert (A9)
        gruppe_voll = 0                             # ausgehend abgewiesen, Netzgruppe voll
        anker = {"gewaehlt": [], "gesichert": 0}
        letzte = {}
        erzeugt = 0
        for f in sorted((aus / n / "logs").glob("*.jsonl")):
            for d in zeilen(f):
                a = d.get("art")
                z[a] += 1
                if a in ("block_erzeugt", "block_uebernommen") and "block" in d:
                    bloecke[d["hoehe"]].add(d["block"])
                    zeitachse.append((d["zeit_ms"], n, d["hoehe"]))
                    letzte = {"hoehe": d["hoehe"], "block": d["block"], "wurzel": d.get("zustandswurzel")}
                    erzeugt += a == "block_erzeugt"
                elif a == "block_abgelehnt":
                    abgelehnt[d.get("ablehnungsart") or (d.get("grund") or "?")[:40]] += 1
                elif a == "erzeugung_wartet":
                    warten.append(d.get("grund"))
                elif a == "aufnahme" and "mesh_blocks" in d:
                    mesh.append((d["mesh_blocks"], d.get("mesh_consensus", 0)))
                elif a == "fuehler":
                    fuehler["gelungen" if d.get("gelungen") else "gescheitert"] += 1
                elif a == "abgewiesen" and "Netzgruppe" in (d.get("grund") or ""):
                    gruppe_voll += 1
                elif a == "anker_gewaehlt":
                    anker["gewaehlt"].append(d.get("gewaehlt", 0))
                elif a == "anker_gesichert":
                    anker["gesichert"] += 1
        je[n] = {"arten": z, "abgelehnt": abgelehnt, "warten": warten, "letzte": letzte, "erzeugt": erzeugt, "mesh": mesh,
                 "fuehler": fuehler, "gruppe_voll": gruppe_voll, "anker": anker}

    out = [f"# Auswertung `{aus.name}`", ""]
    dauer = (messungen[-1]["t_min"] if messungen else 0)
    out.append(f"Dauer {dauer:.0f} min, {len(knoten)} Knoten, {len(ereignisse)} Ereignisse.")
    out.append("")

    # Einigkeit
    out += ["## Einigkeit", "", "| Knoten | Endhoehe | Block | Wurzel | erzeugt | uebernommen | abgelehnt | Zweigwechsel |", "|---|---|---|---|---|---|---|---|"]
    for n in knoten:
        j = je[n]
        out.append(f"| {n} | {j['letzte'].get('hoehe')} | {j['letzte'].get('block')} | {j['letzte'].get('wurzel')} | "
                   f"{j['erzeugt']} | {j['arten']['block_uebernommen']} | {sum(j['abgelehnt'].values())} {dict(j['abgelehnt'])} | "
                   f"{j['arten']['zweig_gewechselt']} |")
    gabeln = {h: b for h, b in bloecke.items() if len(b) > 1}
    endhoehe = min((je[n]["letzte"].get("hoehe") or 0) for n in knoten)
    gemeinsam = {je[n]["letzte"].get("block") for n in knoten if (je[n]["letzte"].get("hoehe") or 0) == max(
        (je[m]["letzte"].get("hoehe") or 0) for m in knoten)}
    out.append("")
    out.append(f"Hoehen mit mehr als einem Block (Gabeln, auch verworfene Zweige): **{len(gabeln)}**"
               + (f", zuerst bei {sorted(gabeln)[:10]}" if gabeln else ""))
    out.append(f"Kleinste Endhoehe {endhoehe}; Bloecke an der hoechsten Endhoehe: {len(gemeinsam)}.")
    out.append("")

    # Mesh: Zerfaellt es, kommen Bloecke nur noch ueber das Fluten des
    # Erzeugers an (Fund vom 2026-11-19, Atteste ohne bekannten Aussteller).
    out += ["## Mesh (Gossipsub)", "", "| Knoten | Aufnahmen | Bloecke Median | Konsens Median | Anteil ohne Mesh (Bloecke) |", "|---|---|---|---|---|"]
    for n in knoten:
        m = je[n]["mesh"]
        if m:
            null = sum(1 for b, _ in m if b == 0) / len(m)
            out.append(f"| {n} | {len(m)} | {statistics.median(b for b, _ in m):.0f} | {statistics.median(k for _, k in m):.0f} | {100 * null:.0f} % |")
    out.append("")

    # Umzingelung (A9): Fuehler fuer eingehende Gegenstellen, ausgehende
    # Wahl an der Grenze je Netzgruppe, Anker ueber Neustarts.
    out += ["## Umzingelung (A9)", "", "| Knoten | Fuehler gelungen | gescheitert | ausgehend abgewiesen (Gruppe voll) | Anker gesichert | Anker gewaehlt beim Start |", "|---|---|---|---|---|---|"]
    for n in knoten:
        j = je[n]
        out.append(f"| {n} | {j['fuehler']['gelungen']} | {j['fuehler']['gescheitert']} | {j['gruppe_voll']} | "
                   f"{j['anker']['gesichert']} | {j['anker']['gewaehlt']} |")
    out.append("")

    # Abstimmung: haelt die Frist des Vorschlags (Vorgabe 1 000 ms) gegen
    # die Laufzeiten?
    out += ["## Abstimmung (BFT-Runden)", "", "| Knoten | Runden begonnen | Rundenwechsel | Commits | Wechsel je Commit |", "|---|---|---|---|---|"]
    for n in knoten:
        z = je[n]["arten"]
        c = z["konsens_commitet"]
        w = z["konsens_rundenwechsel"]
        out.append(f"| {n} | {z['konsens_runde_beginnt']} | {w} | {c} | {w / c if c else float('nan'):.2f} |")
    out.append("")

    # Erzeugung und Warten
    out += ["## Warten vor dem Erzeugen", ""]
    for n in knoten:
        if je[n]["warten"]:
            out.append(f"- {n}: {dict(collections.Counter(je[n]['warten']))}")
    out.append("")

    # Stoerungen: Aufholzeit
    zeitachse.sort()
    out += ["## Stoerungen und Aufholen", "", "| t (min) | Ereignis | Knoten | Kopf erreicht nach |", "|---|---|---|---|"]
    for e in ereignisse:
        if e["art"] not in ("rueckkehr", "heilen", "einsteiger"):
            if e["art"] not in ("startsatz", "netz_gestartet"):
                out.append(f"| {e['t_min']} | {e['art']} | {e.get('knoten', e.get('a', ''))} | |")
            continue
        betroffen = e.get("knoten")
        betroffen = betroffen if isinstance(betroffen, list) else [betroffen]
        for n in betroffen:
            if n not in je:
                continue
            ab = e["zeit_ms"]
            erreicht = None
            kopf = 0
            for zeit, wer, h in zeitachse:
                if zeit < ab:
                    if wer != n:
                        kopf = max(kopf, h)
                    continue
                if wer != n:
                    kopf = max(kopf, h)
                elif h >= kopf and kopf > 0:
                    erreicht = (zeit - ab) / 1000
                    break
            out.append(f"| {e['t_min']} | {e['art']} | {n} | {'%.0f s' % erreicht if erreicht is not None else 'nicht erreicht'} |")
    out.append("")

    # Finalitaet
    out += ["## Finalitaet", ""]
    rueck = collections.defaultdict(list)
    for m in messungen:
        for n, k in m["knoten"].items():
            if k.get("hoehe") is not None and k.get("bestaetigt") is not None:
                rueck[n].append(k["hoehe"] - k["bestaetigt"])
    for n in knoten:
        r = rueck.get(n)
        if r:
            out.append(f"- {n}: Abstand Hoehe zu bestaetigtem Pruefpunkt Median {statistics.median(r):.0f}, "
                       f"hoechstens {max(r)} Bloecke ({len(r)} Messungen)")
    if messungen:
        letzte = messungen[-1]["knoten"]
        best = ", ".join(f"{n}: {k.get('bestaetigt')}" for n, k in letzte.items())
        out.append(f"- zuletzt bestaetigt: {best}")
    out.append("")

    # Ressourcen
    out += ["## Ressourcen", "", "| Knoten | Speicher erste Stunde (MiB) | letzte Stunde | hoechstens | CPU Median | Kette MB/h | Protokoll MB/h |", "|---|---|---|---|---|---|---|"]
    for n in knoten:
        reihe = [(m["t_min"], m["knoten"][n]) for m in messungen if n in m["knoten"]]
        if not reihe:
            continue
        speicher = [(t, mib(k.get("mem"))) for t, k in reihe if mib(k.get("mem")) is not None]
        if not speicher:
            continue
        t_ende = speicher[-1][0]
        erste = [s for t, s in speicher if t <= speicher[0][0] + 60]
        letzte_h = [s for t, s in speicher if t >= t_ende - 60]
        cpu = [float(k["cpu"].rstrip("%")) for t, k in reihe if k.get("cpu", "").endswith("%")]
        spanne_h = max((reihe[-1][0] - reihe[0][0]) / 60, 1e-9)
        kette = (reihe[-1][1]["kette_bytes"] - reihe[0][1]["kette_bytes"]) / 1e6 / spanne_h
        log = (reihe[-1][1]["log_bytes"] - reihe[0][1]["log_bytes"]) / 1e6 / spanne_h
        out.append(f"| {n} | {statistics.mean(erste):.0f} | {statistics.mean(letzte_h):.0f} | {max(s for _, s in speicher):.0f} | "
                   f"{statistics.median(cpu) if cpu else 0:.1f} % | {kette:.2f} | {log:.2f} |")
    out.append("")

    text = "\n".join(out) + "\n"
    (aus / "auswertung.md").write_text(text, encoding="utf-8")
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
