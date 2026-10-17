#!/usr/bin/env python3
"""**Belegungssimulation**: Wie schnell laeuft ein Gemisch, das nicht in den
Arbeitsspeicher passt, je nachdem, wie der Speicher verteilt wird?

# ⚑ Was diese Datei tut, und was nicht

Sie **spielt eine gemessene Expertenwahl nach**. Die Laufzeit schreibt mit
`expertenprobe --spur` je Token und Ebene auf, welche Experten gewaehlt
wurden; hier wird diese Folge gegen verschiedene Belegungen des
Arbeitsspeichers gelegt, und gezaehlt, wie viele Experten von der Platte
kaemen. Die Tokenzeit folgt daraus mit zwei gemessenen Zahlen:

    Tokenzeit = Rechenzeit + Fehlbytes / Plattendurchsatz

⚠️ **Sie misst keine fremde Laufzeit.** Die Strategien unten sind
Verteilregeln, wie sie verbreitet sind, nachgebaut auf derselben Rechnung
und derselben Platte. Unterschiede der Rechenkerne zaehlen hier nicht mit,
und das ist gewollt: Verglichen wird die Belegung, nicht der Kern.

⚠️ **Der Dateicache des Systems ist als LRU nachgebaut.** Ein echter haelt
eine Annaeherung daran (aktive und inaktive Listen). Die Zahlen sind ein
Modell mit gemessenen Eingaben und keine Messung der Tokenrate; Rechnung
und Platte gelten als nacheinander, nicht ueberlappend.

⚠️ **Der KV-Speicher zaehlt in voller Hoehe des Kontexts**, also fuer
einen Verlauf, der die Grenze erreicht hat. Ein kurzer Verlauf belegt
weniger, und dort liegen alle Strategien naeher beieinander.

# Die Strategien

| Name | Kontext (KV) | Experten |
|---|---|---|
| `alles_laden` | Arbeitskontext | alle fest im Speicher, oder die Last laeuft nicht |
| `seitencache_kurz` | kurzer Vorgabekontext (4096) | per mmap, LRU ueber den Rest, kalt |
| `seitencache_voll` | ganzer Trainingskontext | per mmap, LRU ueber den Rest, kalt |
| `seitencache_arbeit` | Arbeitskontext (65536) | per mmap, LRU ueber den Rest, kalt |
| `plan` | aus `dispatcher plan`: heisse Experten vor dem Kontext | per mmap, LRU ueber den Rest, mit den heissen vorgewaermt |
| `plan_festgehalten` | wie `plan` | die heissen fest, der Rest LRU (verworfen, zum Vergleich) |
| `optimum` | wie `plan` | beste moegliche Ersetzung (Belady), kennt die Zukunft |

⚑ **Der Plan begrenzt den Dateicache nicht.** Das System nimmt jedes freie
Byte dafuer, auch die Ruecklage; der Plan entscheidet, wie viel Kontext
und welche Sinne daneben Platz bekommen, und mit welchen Experten der
Cache beginnt.

# Aufruf, aus der Wurzel

    python3 BENCHMARKS/Inferenz/belegungssimulation.py \\
        --artefakt <ordner> --profil <profil> \\
        --lernspur <spur> --pruefspur <spur> [--aus <datei.md>] [--json <datei>]

Die Spuren und das Profil schreibt
`expertenprobe <artefakt> <text> <neue> --profil <datei> --spur <datei>`.
Den Bedarf und den Kontext je Speichergroesse fragt das Skript beim
Programm `dispatcher` ab (`--angebot-mib`), damit die Regeln des Plans
nicht ein zweites Mal hier stehen.
"""

from __future__ import annotations

import argparse
import heapq
import json
import subprocess
from collections import OrderedDict
from pathlib import Path

WURZEL = Path(__file__).resolve().parents[2]
# Gebaut von `cargo build --bin dispatcher` in COMPUTE_PIPELINE/dispatcher.
DISPATCHER = ("SYSTEM/full-build/release/dispatcher", "SYSTEM/full-build/debug/dispatcher")
GIB = 1 << 30
MIB = 1 << 20

# ⚑ **Gemessen, nicht gesetzt**: das int8-35B auf dem M5 Pro, warm, 18,6
# Token/s bei 34,9 MB je Token von der SSD. Mit dem Plattendurchsatz unten
# sind das 10,2 ms Platte und 43,5 ms Rechnung je Token.
RECHENZEIT_MS = 43.5
# Abbild mit Rat an den Kern (`MADV_WILLNEED`), gemessen an kalten Ebenen.
PLATTE_GBS = 3.41
# Der gemessene Zuschlag des int8-35B (`myl eichen`) und die Ruecklage des
# Plans, die nur die Wache der Sinne betrifft.
ZUSCHLAG = int(0.6 * GIB)
RUECKLAGE = GIB
KONTEXT_ARBEIT = 65536
KONTEXT_KURZ = 4096
# ⚠️ **Annahme**: Was System und Programme auf einer Maschine belegen, bevor
# die Inferenz beginnt. Auf dem M5 Pro mit 24 GiB meldete die Erhebung 18
# GiB frei; die sechs GiB gelten hier fuer jede Groesse.
SYSTEM = 6 * GIB
# Sehen, gemessen: Dateien 2,75 GiB, dazu 0,9 GiB mit begrenztem Kontext
# (`-c`) oder 15,5 GiB mit dem Vorgabekontext der Laufzeit.
SEHEN_BEGRENZT = int(3.65 * GIB)
SEHEN_VORGABE = int(18.25 * GIB)
# Wie viele Token der Kaltstart umfasst.
KALT_TOKEN = 500

STRATEGIEN = ["alles_laden", "seitencache_kurz", "seitencache_voll", "seitencache_arbeit", "plan", "plan_festgehalten", "optimum"]


def dispatcher() -> Path:
    """Das juengere der beiden Programme: Ein altes Release neben einem
    frischen Debug-Bau kennt die neuen Schalter nicht."""
    da = [WURZEL / d for d in DISPATCHER if (WURZEL / d).is_file()]
    if not da:
        raise SystemExit("dispatcher fehlt: cd COMPUTE_PIPELINE/dispatcher && cargo build --bin dispatcher")
    return max(da, key=lambda p: p.stat().st_mtime)


def plan_fragen(artefakt: Path, profil: Path, angebot: int) -> dict:
    """Der Plan des Dispatchers fuer `angebot` Bytes freien Speicher."""
    aus = subprocess.run(
        [str(dispatcher()), "plan", "--agent", str(artefakt), "--profil", str(profil), "--angebot-mib", str(angebot // MIB), "--json"],
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(aus.stdout)


def bedarf_aus(plan: dict) -> dict:
    b = plan["lasten"][0]["gewichte"]["Eigen"]
    ebenen = b["ebenen"]
    return {
        "name": b["name"],
        "dicht": b["rand_bytes"] + sum(e["dicht_bytes"] for e in ebenen),
        "experte": ebenen[0]["experten_bytes"][0],
        "ebenen": [e["index"] for e in ebenen],
        "je_ebene": len(ebenen[0]["experten_bytes"]),
        "kv_je_token": b["kv_je_token"],
        "zustand": b["zustand_je_folge"],
        "hoechstkontext": b["hoechstkontext"],
    }


def spur_lesen(pfad: Path, ebenen: list[int]) -> list[list[tuple[int, int]]]:
    """Je Token die Zugriffe `(Ebene, Experte)`; die Spalten der Spur sind
    die Gemischebenen in ihrer Reihenfolge."""
    aus = []
    for zeile in pfad.read_text().splitlines():
        if zeile.strip():
            aus.append([(ebenen[i], int(x)) for i, teil in enumerate(zeile.split(";")) for x in teil.split(",")])
    return aus


def rangfolge(profil: dict, b: dict) -> list[tuple[int, int]]:
    """Alle Experten, die haeufigsten zuerst, bei Gleichstand nach Lage,
    dieselbe Ordnung wie im Dispatcher."""
    z = profil["zaehler"]

    def zahl(e: int, x: int) -> int:
        return z[e][x] if e < len(z) and x < len(z[e]) else 0

    alle = [(e, x) for e in b["ebenen"] for x in range(b["je_ebene"])]
    alle.sort(key=lambda p: (-zahl(*p), p))
    return alle


class Lru:
    """Ein Cache, der den am laengsten Ungenutzten verdraengt; `fest` sind
    Experten, die nie verdraengt werden und nicht zur Menge zaehlen."""

    def __init__(self, plaetze: int, fest: set | None = None):
        self.plaetze = max(0, plaetze)
        self.fest = fest or set()
        self.inhalt: OrderedDict = OrderedDict()

    def zugriff(self, k: tuple[int, int]) -> bool:
        if k in self.fest:
            return True
        if k in self.inhalt:
            self.inhalt.move_to_end(k)
            return True
        if self.plaetze > 0:
            self.inhalt[k] = None
            if len(self.inhalt) > self.plaetze:
                self.inhalt.popitem(last=False)
        return False

    def schrumpfen(self, plaetze: int) -> None:
        """Ein Fremder nimmt Speicher: die Aeltesten gehen, der Platz bleibt."""
        while len(self.inhalt) > max(0, plaetze):
            self.inhalt.popitem(last=False)


def belady(folge: list, plaetze: int, ab: int) -> int:
    """Fehlzugriffe der besten moeglichen Ersetzung ab Position `ab`; der
    Teil davor waermt nur an. Verdraengt wird, was am spaetesten wieder
    gebraucht wird."""
    naechst = [0] * len(folge)
    zuletzt: dict = {}
    nie = len(folge) + 1
    for i in range(len(folge) - 1, -1, -1):
        naechst[i] = zuletzt.get(folge[i], nie)
        zuletzt[folge[i]] = i
    im: dict = {}
    haufen: list = []
    fehl = 0
    for i, k in enumerate(folge):
        if k not in im:
            if i >= ab:
                fehl += 1
            if plaetze <= 0:
                continue
            if len(im) >= plaetze:
                while True:
                    n, alt = heapq.heappop(haufen)
                    if im.get(alt) == -n:
                        del im[alt]
                        break
        im[k] = naechst[i]
        heapq.heappush(haufen, (-naechst[i], k))
    return fehl


def flach(spur: list) -> list:
    return [k for token in spur for k in token]


def zeit(b: dict, fehl: int, zugriffe: int, token: int) -> dict:
    mb_je_token = fehl * b["experte"] / token / 1e6
    ms = RECHENZEIT_MS + mb_je_token / PLATTE_GBS
    return {"treffer": 1 - fehl / zugriffe, "mb_je_token": mb_je_token, "tok_s": 1000 / ms}


def lage(b: dict, strategie: str, verfuegbar: int, plan: dict) -> dict | None:
    """Kontext, Platz fuer Experten im Dateicache und die heissen Experten
    einer Strategie; `None`, wenn die Last so nicht laeuft."""
    anonym_ohne_kv = ZUSCHLAG + b["zustand"]
    alle = len(b["ebenen"]) * b["je_ebene"]
    if strategie == "alles_laden":
        braucht = anonym_ohne_kv + b["kv_je_token"] * KONTEXT_ARBEIT + b["dicht"] + alle * b["experte"]
        return {"kontext": KONTEXT_ARBEIT, "plaetze": alle, "heiss": 0} if braucht <= verfuegbar else None
    if strategie in ("plan", "plan_festgehalten", "optimum"):
        z = plan["plan"]["zuteilungen"][0]["ergebnis"]
        if "Angenommen" not in z:
            return None
        kontext, heiss = z["Angenommen"]["kontext"], z["Angenommen"]["experten_heiss"]
    else:
        kontext = {"seitencache_kurz": KONTEXT_KURZ, "seitencache_voll": b["hoechstkontext"], "seitencache_arbeit": KONTEXT_ARBEIT}[strategie]
        heiss = 0
    rest = verfuegbar - anonym_ohne_kv - b["kv_je_token"] * kontext - b["dicht"]
    if rest < 0:
        return None
    return {"kontext": kontext, "plaetze": min(alle, rest // b["experte"]), "heiss": heiss}


def cache(strategie: str, l: dict, rang: list) -> Lru:
    if strategie == "plan_festgehalten":
        fest = set(rang[: min(l["heiss"], l["plaetze"])])
        return Lru(l["plaetze"] - len(fest), fest)
    c = Lru(l["plaetze"])
    if strategie == "plan":
        # Vorgewaermt, die seltensten zuerst: Die haeufigsten bleiben am laengsten.
        for k in reversed(rang[: min(l["heiss"], l["plaetze"])]):
            c.zugriff(k)
    return c


def dauerbetrieb(b, strategie, l, rang, lern, pruef) -> dict:
    """Nach der Lernspur (zum Anwaermen) die Pruefspur."""
    zl, zp = flach(lern), flach(pruef)
    if strategie == "alles_laden":
        return zeit(b, 0, len(zp), len(pruef))
    if strategie == "optimum":
        return zeit(b, belady(zl + zp, l["plaetze"], len(zl)), len(zp), len(pruef))
    c = cache(strategie, l, rang)
    for k in zl:
        c.zugriff(k)
    return zeit(b, sum(0 if c.zugriff(k) else 1 for k in zp), len(zp), len(pruef))


def kaltstart(b, strategie, l, rang, pruef, vorwaermen_zaehlt: bool = False) -> dict:
    """Die ersten Token nach dem Laden, ohne Anwaermen.

    ⚑ **Was vor dem ersten Token gelesen wird, zaehlt mit**: bei
    `alles_laden` alle Experten; beim Plan das Vorwaermen nur, wenn
    `vorwaermen_zaehlt`. Ohne zaehlt es als in der Wartezeit erledigt, etwa
    waehrend der Mensch die erste Frage tippt."""
    teil = pruef[:KALT_TOKEN]
    zp = flach(teil)
    if strategie == "alles_laden":
        return zeit(b, l["plaetze"], len(zp), len(teil))
    c = cache(strategie, l, rang)
    vorab = len(c.inhalt) if vorwaermen_zaehlt else 0
    return zeit(b, vorab + sum(0 if c.zugriff(k) else 1 for k in zp), len(zp), len(teil))


def mit_sehen(b, strategie, l, rang, lern, pruef, verfuegbar, sehen) -> dict:
    """**Ein Bild mitten in der Arbeit**: die erste Haelfte der Pruefspur,
    dann belegt das Sehen `sehen` Bytes, dann die zweite Haelfte, und die
    wird gezaehlt."""
    halb = len(pruef) // 2
    anonym = ZUSCHLAG + b["zustand"] + b["kv_je_token"] * l["kontext"]
    # ⚑ **Die Wache**: Beim Plan startet Sehen nur, wenn es neben den
    # dichten Gewichten und der Ruecklage passt; die Experten, auch die
    # heissen, darf es sich leihen. Ohne Plan startet es immer.
    if strategie == "plan" and sehen > verfuegbar - anonym - b["dicht"] - RUECKLAGE:
        return {"abgelehnt": True, "auslagerung": 0, **dauerbetrieb(b, strategie, l, rang, lern, pruef)}
    c = cache(strategie, l, rang)
    for k in flach(lern) + flach(pruef[:halb]):
        c.zugriff(k)
    frei = verfuegbar - anonym - b["dicht"] - l["plaetze"] * b["experte"]
    lru = len(c.inhalt) * b["experte"]
    verdraengt = min(max(0, sehen - frei), lru)
    c.schrumpfen((lru - verdraengt) // b["experte"])
    # Was ueber den ganzen Cache hinausgeht, trifft dichte Gewichte, feste
    # Experten und anonymen Speicher: Auslagerung, benannt und nicht bepreist.
    auslagerung = max(0, sehen - frei - lru)
    zp = flach(pruef[halb:])
    return {"abgelehnt": False, "auslagerung": auslagerung, **zeit(b, sum(0 if c.zugriff(k) else 1 for k in zp), len(zp), len(pruef) - halb)}


def zelle(r: dict | None, kontext: int | None = None) -> str:
    if r is None:
        return "läuft nicht"
    text = f"{r['tok_s']:.1f} ({100 * r['treffer']:.1f} %, {r['mb_je_token']:.0f} MB)"
    if r.get("abgelehnt"):
        return text + ", Sehen abgelehnt"
    if r.get("auslagerung"):
        text += f", {r['auslagerung'] / GIB:.1f} GiB Auslagerung"
    return text


def main() -> int:
    a = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    a.add_argument("--artefakt", type=Path, required=True)
    a.add_argument("--profil", type=Path, required=True)
    a.add_argument("--lernspur", type=Path, required=True)
    a.add_argument("--pruefspur", type=Path, required=True)
    a.add_argument("--speicher-gib", type=int, nargs="+", default=[16, 24, 32, 48])
    a.add_argument("--aus", type=Path)
    a.add_argument("--json", type=Path)
    x = a.parse_args()

    profil = json.loads(x.profil.read_text())
    plaene = {g: plan_fragen(x.artefakt, x.profil, g * GIB - SYSTEM) for g in x.speicher_gib}
    b = bedarf_aus(next(iter(plaene.values())))
    rang = rangfolge(profil, b)
    lern, pruef = spur_lesen(x.lernspur, b["ebenen"]), spur_lesen(x.pruefspur, b["ebenen"])

    erg: dict = {"bedarf": b, "token_lern": len(lern), "token_pruef": len(pruef), "laeufe": []}
    t = [f"# Belegungssimulation: {b['name']}", ""]
    t.append(
        f"Erzeugt von `belegungssimulation.py`. Lernspur {len(lern)} Token, Prüfspur {len(pruef)} Token, Profil aus "
        f"{profil['token']} Token. Dicht {b['dicht'] / GIB:.2f} GiB, {len(b['ebenen'])} × {b['je_ebene']} Experten zu "
        f"{b['experte'] / 1e6:.2f} MB. Rechenzeit {RECHENZEIT_MS} ms je Token, Platte {PLATTE_GBS} GB/s, System "
        f"{SYSTEM // GIB} GiB. Zellen: Token/s (Treffer im Speicher, MB je Token von der Platte)."
    )
    t.append("")
    t.append("## Kontext und Platz je Strategie\n")
    t.append("| Arbeitsspeicher | " + " | ".join(f"`{s}`" for s in STRATEGIEN[:5]) + " |")
    t.append("|---" * 6 + "|")
    lagen = {}
    for g in x.speicher_gib:
        v = g * GIB - SYSTEM
        zellen = []
        for s in STRATEGIEN:
            lagen[(g, s)] = lage(b, s, v, plaene[g])
        for s in STRATEGIEN[:5]:
            l = lagen[(g, s)]
            zellen.append(
                "läuft nicht"
                if l is None
                else f"Kontext {l['kontext']}, {l['plaetze'] * b['experte'] / GIB:.1f} GiB Experten"
                + (f" ({l['heiss']} heiß vorgewärmt)" if s == "plan" else "")
            )
        t.append(f"| {g} GiB | " + " | ".join(zellen) + " |")
    t.append("")
    t.append("## Dauerbetrieb (nach der Lernspur angewärmt)\n")
    t.append("| Arbeitsspeicher | " + " | ".join(f"`{s}`" for s in STRATEGIEN) + " |")
    t.append("|---" * (len(STRATEGIEN) + 1) + "|")
    for g in x.speicher_gib:
        zellen = []
        for s in STRATEGIEN:
            l = lagen[(g, s)]
            r = None if l is None else dauerbetrieb(b, s, l, rang, lern, pruef)
            erg["laeufe"].append({"art": "dauer", "speicher_gib": g, "strategie": s, "lage": l, "ergebnis": r})
            zellen.append(zelle(r))
        t.append(f"| {g} GiB | " + " | ".join(zellen) + " |")
    t.append("")
    # Beim Kaltstart ohne `optimum`: Aus leerem Cache ist Belady keine
    # Schranke fuer einen vorgewaermten.
    kalt = [s for s in STRATEGIEN if s != "optimum"]
    t.append(f"## Kaltstart (die ersten {KALT_TOKEN} Token nach dem Laden, Lesen vorab mitgezählt)\n")
    t.append("| Arbeitsspeicher | " + " | ".join(f"`{s}`" for s in kalt) + " | `plan`, Vorwärmen mitgezählt |")
    t.append("|---" * (len(kalt) + 2) + "|")
    for g in x.speicher_gib:
        zellen = []
        for s in kalt + ["plan+"]:
            l = lagen[(g, s.rstrip("+"))]
            r = None if l is None else kaltstart(b, s.rstrip("+"), l, rang, pruef, s.endswith("+"))
            erg["laeufe"].append({"art": "kalt", "speicher_gib": g, "strategie": s, "lage": l, "ergebnis": r})
            zellen.append(zelle(r))
        t.append(f"| {g} GiB | " + " | ".join(zellen) + " |")
    t.append("")
    reihe = ["seitencache_kurz", "seitencache_voll", "seitencache_arbeit", "plan"]
    for name, sehen in [("begrenzter Kontext, 3,65 GiB", SEHEN_BEGRENZT), ("Vorgabekontext der Laufzeit, 18,25 GiB", SEHEN_VORGABE)]:
        t.append(f"## Ein Bild mitten in der Arbeit ({name}): die Token danach\n")
        t.append("| Arbeitsspeicher | " + " | ".join(f"`{s}`" for s in reihe) + " |")
        t.append("|---" * (len(reihe) + 1) + "|")
        for g in x.speicher_gib:
            zellen = []
            for s in reihe:
                l = lagen[(g, s)]
                r = None if l is None else mit_sehen(b, s, l, rang, lern, pruef, g * GIB - SYSTEM, sehen)
                erg["laeufe"].append({"art": "sehen", "sehen_bytes": sehen, "speicher_gib": g, "strategie": s, "lage": l, "ergebnis": r})
                zellen.append(zelle(r))
            t.append(f"| {g} GiB | " + " | ".join(zellen) + " |")
        t.append("")
    text = "\n".join(t)
    print(text)
    if x.aus:
        x.aus.write_text(text + "\n")
    if x.json:
        x.json.write_text(json.dumps(erg, indent=1, ensure_ascii=False) + "\n")
    print("Fertig")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
