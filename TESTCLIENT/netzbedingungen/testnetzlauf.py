#!/usr/bin/env python3
"""Das geschlossene Testnetz `test-int4`, nachgestellt: je Knoten ein eigener
Behaelter, Laufzeiten und Verlust wie zwischen Orten, ueber Stunden.

# Wozu

Die anderen Netzlaeufe (`netzlauf_prozesse.py`, `podlauf.py`) fahren alle
Knoten in einem Behaelter auf der Probekette, mit gerafftem Takt. Dieser
Lauf stellt das geschlossene Testnetz nach, wie es mit echten Betreibern
anlaeuft:

- das Netz `test-int4` mit seinem Takt (2 s je Slot, 1 800 Bloecke je
  Epoche) und einem Startsatz aus fuenf gleich schweren Validatoren;
- ein Einstiegsknoten mit `--erzeuger` unter einem Namen
  (`/dns4/einstieg/...`), ein zweiter Einstieg, drei Validatoren wie Miner
  ueber `--konto`, Testverkehr mit Transaktionen;
- je Behaelter eine eigene Netzschnittstelle mit `tc netem` (Laufzeit,
  Streuung, Verlust je Ort);
- Stoerungen nach Plan: Neustarts, ein Absturz, ein abgeschnittener
  Validator, eine Spaltung zwei zu drei, ein Einsteiger von null, der
  Einstieg faellt aus, schlechtes Netz, ein Validator ohne seine Kette,
  zwei zugleich neu gestartet.

Gemessen wird jede Minute: Hoehe, bestaetigter Pruefpunkt und Gegenstellen
je Knoten (aus der Zustandsaufnahme), CPU und Speicher je Behaelter, Groesse
von Kette und Protokoll. Am Ende eine Auswertung: eine Wurzel, Gabeln,
Ablehnungen, Rueckstand der Finalitaet, Aufholzeiten, Wachstum des
Speichers.

Ohne Modell und Shards: Fuenf Shards des Netzmodells passen nicht auf eine
Maschine. Was der Lauf zeigt, ist Netz, Kette und Konsens unter
Bedingungen wie im Testnetz.

# Aufruf (aus der Wurzel des Repositoriums)

    docker run --rm -v "$PWD":/repo -v myl-netem-ziel:/ziel -w /repo/NODE/myl-node \\
      -e CARGO_TARGET_DIR=/ziel/nacht -e CARGO_HOME=/repo/SYSTEM/crates-lager/cargo-home \\
      -e CARGO_NET_OFFLINE=true myelith-netem sh -c 'cargo build --release --locked --offline'
    python3 -u TESTCLIENT/netzbedingungen/testnetzlauf.py --aus <ordner> --kurz     # rund 12 Minuten
    python3 -u TESTCLIENT/netzbedingungen/testnetzlauf.py --aus <ordner> --minuten 400
    python3 -u TESTCLIENT/netzbedingungen/testnetzlauf.py --aus <ordner> --kurz --ein-as

`--ein-as` gibt jedem Knoten eine AS-Zuordnung, die alle Behaelter in ein AS
legt (`--asmap`): Dann haelt jeder hoechstens zwei ausgehende Verbindungen zu
den anderen, der schaerfste Fall der Grenze je Netzgruppe (Audit A9), etwa ein
Testnetz, dessen Betreiber alle beim selben Anbieter sind.

Laeuft auf dem Rechner mit Docker, nicht im Behaelter; braucht das Abbild
`myelith-netem` und das Volume `myl-netem-ziel` mit dem Knoten unter
`/ziel/nacht/release/myl-node`.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import secrets
import subprocess
import sys
import time

WURZEL = pathlib.Path(__file__).resolve().parents[2]
ABBILD = "myelith-netem"
VOLUME = "myl-netem-ziel"
NETZ = "mylnacht"
PROGRAMM = "/ziel/nacht/release/myl-node"
PRAEFIX = "myln-"

# Orte: Laufzeit in eine Richtung, Streuung, Verlust. Zwischen zwei Knoten
# addieren sich beide Seiten (jede Schnittstelle verzoegert ihren Ausgang).
ORTE = {
    "einstieg": (8, 3, 0.2),   # privater Anschluss des Projektinhabers
    "v2": (12, 4, 0.2),        # zweiter Einstieg, anderes Netz
    "v3": (20, 6, 0.3),
    "v4": (30, 8, 0.5),
    "v5": (45, 12, 0.5),
    "n1": (25, 6, 0.3),        # Einsteiger von null
    "n2": (35, 8, 0.3),        # Einsteiger, waehrend der Einstieg fehlt
}
VALIDATOREN = ["einstieg", "v2", "v3", "v4", "v5"]


def sh(*args: str, pruefen: bool = True, still: bool = False) -> str:
    r = subprocess.run(list(args), capture_output=True, text=True)
    if pruefen and r.returncode != 0:
        raise RuntimeError(f"{' '.join(args)}: {r.stderr.strip()[:400]}")
    return r.stdout if not still else ""


def jetzt_ms() -> int:
    return int(time.time() * 1000)


class Lauf:
    def __init__(self, aus: pathlib.Path, faktor: float, minuten: int, ein_as: bool = False):
        self.aus = aus
        self.ein_as = ein_as
        self.faktor = faktor
        self.minuten = minuten
        self.t0 = time.time()
        self.ereignisse: list[dict] = []
        self.konten: dict[str, str] = {}
        self.peer: dict[str, str] = {}
        self.messung = open(aus / "messung.jsonl", "a", encoding="utf-8")
        self.ereignisdatei = open(aus / "ereignisse.jsonl", "a", encoding="utf-8")

    # ── Behaelter ────────────────────────────────────────────────────────
    def daten(self, n: str) -> pathlib.Path:
        return self.aus / n

    def behaelter(self, n: str) -> None:
        d = self.daten(n)
        (d / "logs").mkdir(parents=True, exist_ok=True)
        sh("docker", "run", "-d", "--name", PRAEFIX + n, "--hostname", n, "--network", NETZ,
           "--cap-add", "NET_ADMIN", "-v", f"{VOLUME}:/ziel:ro", "-v", f"{WURZEL}:/repo:ro",
           "-v", f"{d}:/daten", ABBILD, "sleep", "infinity")
        ms, streu, verlust = ORTE[n]
        self.netem(n, ms, streu, verlust)

    def netem(self, n: str, ms: int, streu: int, verlust: float) -> None:
        sh("docker", "exec", PRAEFIX + n, "tc", "qdisc", "replace", "dev", "eth0", "root", "netem",
           "delay", f"{ms}ms", f"{streu}ms", "distribution", "normal", "loss", f"{verlust}%")

    def ip(self, n: str) -> str:
        return sh("docker", "inspect", "-f", f"{{{{(index .NetworkSettings.Networks \"{NETZ}\").IPAddress}}}}",
                  PRAEFIX + n).strip()

    # ── Knoten ───────────────────────────────────────────────────────────
    def argumente(self, n: str) -> list[str]:
        a = ["--name", n, "--netz", "test-int4", "--stimmsatz", "/daten/satz.stimmsatz",
             "--port", "4150", "--schluessel", "/daten/knoten.key", "--kette", "/daten/kette.dat",
             "--protokolle", "/daten/logs", "--aufnahme", "30", "--ohne-tuer",
             "--beobachtung", "127.0.0.1:4151", "--still",
             "--konformitaet", "/repo/INTEGER_LLM/conformance/vectors/op"]
        if n in VALIDATOREN:
            a += ["--konsensschluessel", "/daten/konsens.key"]
        if self.ein_as:
            a += ["--asmap", "/daten/asmap.txt"]
        if n == "einstieg":
            a += ["--erzeuger", "--zeugen-bereithalten", "--oeffentlich", "/dns4/einstieg/tcp/4150"]
            return a
        a += ["--bootstrap", f"/dns4/einstieg/tcp/4150/p2p/{self.peer['einstieg']}"]
        if n == "v2":
            a += ["--zeugen-bereithalten", "--oeffentlich", "/dns4/v2/tcp/4150"]
        elif "v2" in self.peer:
            a += ["--bootstrap", f"/dns4/v2/tcp/4150/p2p/{self.peer['v2']}"]
        if n in VALIDATOREN:
            a += ["--konto", self.konten[n], "--testverkehr", "10"]
        return a

    def starten(self, n: str) -> None:
        arg = " ".join(self.argumente(n))
        # Das Abbild hat kein pkill: Die Prozessnummer steht in /daten/pid.
        sh("docker", "exec", "-d", PRAEFIX + n, "sh", "-c",
           f"echo $$ > /daten/pid; exec {PROGRAMM} {arg} >> /daten/aus.txt 2>&1")

    def laeuft(self, n: str) -> bool:
        r = subprocess.run(["docker", "exec", PRAEFIX + n, "sh", "-c", "kill -0 $(cat /daten/pid) 2>/dev/null"],
                           capture_output=True)
        return r.returncode == 0

    def beenden(self, n: str, signal: str = "TERM") -> None:
        sh("docker", "exec", PRAEFIX + n, "sh", "-c", f"kill -{signal} $(cat /daten/pid)", pruefen=False)
        for _ in range(30):
            if not self.laeuft(n):
                return
            time.sleep(0.5)
        sh("docker", "exec", PRAEFIX + n, "sh", "-c", "kill -KILL $(cat /daten/pid)", pruefen=False)

    def peer_aus_protokoll(self, n: str, frist: float = 90) -> str:
        ende = time.time() + frist
        while time.time() < ende:
            for f in sorted((self.daten(n) / "logs").glob("*.jsonl")):
                with open(f, encoding="utf-8") as h:
                    for z in h:
                        try:
                            p = json.loads(z).get("peer")
                        except json.JSONDecodeError:
                            continue
                        if p:
                            return p
            time.sleep(1)
        raise RuntimeError(f"{n}: keine Peer-ID im Protokoll")

    # ── Spaltung ─────────────────────────────────────────────────────────
    def trennen(self, gruppe_a: list[str], gruppe_b: list[str]) -> None:
        ips = {n: self.ip(n) for n in gruppe_a + gruppe_b}
        for a in gruppe_a:
            for b in gruppe_b:
                for x, y in ((a, b), (b, a)):
                    sh("docker", "exec", PRAEFIX + x, "iptables", "-A", "INPUT", "-s", ips[y], "-j", "DROP")
                    sh("docker", "exec", PRAEFIX + x, "iptables", "-A", "OUTPUT", "-d", ips[y], "-j", "DROP")

    def heilen(self, knoten: list[str]) -> None:
        for n in knoten:
            sh("docker", "exec", PRAEFIX + n, "iptables", "-F", pruefen=False)

    # ── Messen ───────────────────────────────────────────────────────────
    def letzte_aufnahme(self, n: str) -> dict | None:
        dateien = sorted((self.daten(n) / "logs").glob("*.jsonl"), key=lambda p: p.stat().st_mtime)
        for f in reversed(dateien):
            with open(f, "rb") as h:
                h.seek(0, os.SEEK_END)
                groesse = h.tell()
                h.seek(max(0, groesse - 200_000))
                zeilen = h.read().decode("utf-8", "replace").splitlines()
            for z in reversed(zeilen):
                if '"art":"aufnahme"' in z:
                    try:
                        return json.loads(z)
                    except json.JSONDecodeError:
                        continue
        return None

    def messen(self, knoten: list[str]) -> dict:
        stats = {}
        roh = sh("docker", "stats", "--no-stream", "--format", "{{.Name}};{{.CPUPerc}};{{.MemUsage}}", pruefen=False)
        for z in roh.splitlines():
            name, cpu, mem = z.split(";")
            if name.startswith(PRAEFIX):
                stats[name[len(PRAEFIX):]] = (cpu, mem.split("/")[0].strip())
        m = {"t_min": round((time.time() - self.t0) / 60, 1), "zeit_ms": jetzt_ms(), "knoten": {}}
        for n in knoten:
            a = self.letzte_aufnahme(n) or {}
            kette = self.daten(n) / "kette.dat"
            logs = sum(f.stat().st_size for f in (self.daten(n) / "logs").glob("*"))
            m["knoten"][n] = {
                "hoehe": a.get("hoehe"), "bestaetigt": a.get("pruefpunkt_bestaetigt"),
                "peers": a.get("peers"), "wurzel": a.get("zustandswurzel"), "aufnahme_ms": a.get("zeit_ms"),
                "cpu": stats.get(n, ("", ""))[0], "mem": stats.get(n, ("", ""))[1],
                "kette_bytes": kette.stat().st_size if kette.exists() else 0, "log_bytes": logs,
            }
        self.messung.write(json.dumps(m, ensure_ascii=False) + "\n")
        self.messung.flush()
        return m

    def ereignis(self, art: str, **mehr) -> None:
        e = {"t_min": round((time.time() - self.t0) / 60, 1), "zeit_ms": jetzt_ms(), "art": art, **mehr}
        self.ereignisse.append(e)
        self.ereignisdatei.write(json.dumps(e, ensure_ascii=False) + "\n")
        self.ereignisdatei.flush()
        print(f"[testnetz] {e['t_min']:7.1f} min  {art} {mehr if mehr else ''}", flush=True)


def plan(faktor: float) -> list[tuple[float, str, dict]]:
    """Die Stoerungen in Minuten ab Start (bei `faktor` 1)."""
    p = [
        (20, "neustart", {"knoten": ["v3"], "pause": 2}),
        (45, "absturz", {"knoten": ["v4"], "pause": 5}),
        (75, "abschneiden", {"knoten": ["v5"], "dauer": 10}),
        (110, "spaltung", {"a": ["einstieg", "v2"], "b": ["v3", "v4", "v5"], "dauer": 5}),
        (150, "einsteiger", {"knoten": "n1"}),
        (180, "ausfall", {"knoten": ["einstieg"], "pause": 20, "einsteiger_nach": 5, "einsteiger": "n2"}),
        (220, "schlechtes_netz", {"faktor": 3, "verlust": 5.0, "dauer": 30}),
        (270, "ohne_kette", {"knoten": ["v2"], "pause": 2}),
        (300, "neustart", {"knoten": ["v3", "v4"], "pause": 3}),
    ]
    return [(t * faktor, art, {k: (v * faktor if k in ("pause", "dauer", "einsteiger_nach") else v) for k, v in d.items()})
            for t, art, d in p]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--aus", type=pathlib.Path, required=True)
    ap.add_argument("--kurz", action="store_true", help="der Plan auf rund ein Dreissigstel gerafft (rund 12 Minuten)")
    ap.add_argument("--minuten", type=int, default=400, help="Dauer des langen Laufs")
    ap.add_argument("--ein-as", action="store_true", help="alle Behaelter in einem AS (--asmap): hoechstens zwei ausgehende je Knoten")
    args = ap.parse_args()
    faktor = 1 / 30 if args.kurz else 1.0
    minuten = 12 if args.kurz else args.minuten
    aus = args.aus.resolve()
    aus.mkdir(parents=True, exist_ok=True)
    lauf = Lauf(aus, faktor, minuten, ein_as=args.ein_as)

    # Aufraeumen, was ein abgebrochener Lauf zuruecklaesst.
    alte = sh("docker", "ps", "-aq", "--filter", f"name={PRAEFIX}", pruefen=False).split()
    if alte:
        sh("docker", "rm", "-f", *alte, pruefen=False)
    sh("docker", "network", "rm", NETZ, pruefen=False)
    sh("docker", "network", "create", NETZ)

    # Schluessel und Startsatz, mit dem Linux-Knoten selbst.
    zeilen = []
    for n in VALIDATOREN:
        (aus / n).mkdir(parents=True, exist_ok=True)
        z = sh("docker", "run", "--rm", "-v", f"{VOLUME}:/ziel:ro", "-v", f"{aus / n}:/daten", ABBILD,
               PROGRAMM, "--konsensschluessel", "/daten/konsens.key", "--stimmsatzzeile", "100")
        zeilen.append(z.strip().splitlines()[-1])
        lauf.konten[n] = secrets.token_hex(32)
    satz = f"netz test-int4\nstart {jetzt_ms()}\n" + "\n".join(zeilen) + "\n"
    for n in list(ORTE):
        (aus / n).mkdir(parents=True, exist_ok=True)
        (aus / n / "satz.stimmsatz").write_text(satz, encoding="utf-8")
        if args.ein_as:
            # Das Netz von Docker liegt in 172.16.0.0/12.
            (aus / n / "asmap.txt").write_text("# alle Behaelter in einem AS\n172.16.0.0/12 65000\n", encoding="utf-8")
    lauf.ereignis("startsatz", validatoren=len(zeilen))

    # Der Einstieg zuerst, dann die anderen.
    lauf.behaelter("einstieg")
    lauf.starten("einstieg")
    lauf.peer["einstieg"] = lauf.peer_aus_protokoll("einstieg")
    lauf.behaelter("v2")
    lauf.starten("v2")
    lauf.peer["v2"] = lauf.peer_aus_protokoll("v2")
    for n in ["v3", "v4", "v5"]:
        lauf.behaelter(n)
        lauf.starten(n)
    lauf.ereignis("netz_gestartet", knoten=VALIDATOREN)

    aktiv = list(VALIDATOREN)
    offen = plan(faktor)
    nachher: list[tuple[float, str, dict]] = []  # (zeit, art, daten) fuer Rueckkehr
    ende = lauf.t0 + minuten * 60
    naechste_messung = 0.0
    letzte_hoehe, stillstand_seit = -1, time.time()
    while time.time() < ende:
        t = (time.time() - lauf.t0) / 60
        # Faellige Stoerungen
        while offen and offen[0][0] <= t:
            _, art, d = offen.pop(0)
            if art in ("neustart", "absturz", "ohne_kette", "ausfall"):
                for n in d["knoten"]:
                    lauf.beenden(n, "KILL" if art == "absturz" else "TERM")
                    if art == "ohne_kette":
                        for f in ("kette.dat",):
                            p = lauf.daten(n) / f
                            if p.exists():
                                p.unlink()
                lauf.ereignis(art, **d)
                nachher.append((t + d["pause"], "rueckkehr", {"knoten": d["knoten"]}))
                if art == "ausfall":
                    nachher.append((t + d["einsteiger_nach"], "einsteiger", {"knoten": d["einsteiger"]}))
            elif art == "abschneiden":
                andere = [x for x in aktiv if x not in d["knoten"]]
                lauf.trennen(d["knoten"], andere)
                lauf.ereignis(art, **d)
                nachher.append((t + d["dauer"], "heilen", {"knoten": aktiv[:]}))
            elif art == "spaltung":
                lauf.trennen(d["a"], d["b"])
                lauf.ereignis(art, **d)
                nachher.append((t + d["dauer"], "heilen", {"knoten": aktiv[:]}))
            elif art == "einsteiger":
                n = d["knoten"]
                lauf.behaelter(n)
                lauf.starten(n)
                aktiv.append(n)
                lauf.ereignis(art, **d)
            elif art == "schlechtes_netz":
                for n in aktiv:
                    ms, streu, verlust = ORTE[n]
                    lauf.netem(n, ms * d["faktor"], streu * d["faktor"], d["verlust"])
                lauf.ereignis(art, **d)
                nachher.append((t + d["dauer"], "netz_gut", {}))
            nachher.sort(key=lambda x: x[0])
        while nachher and nachher[0][0] <= t:
            _, art, d = nachher.pop(0)
            if art == "rueckkehr":
                for n in d["knoten"]:
                    lauf.starten(n)
            elif art == "heilen":
                lauf.heilen(d["knoten"])
            elif art == "einsteiger":
                n = d["knoten"]
                lauf.behaelter(n)
                lauf.starten(n)
                aktiv.append(n)
            elif art == "netz_gut":
                for n in aktiv:
                    lauf.netem(n, *ORTE[n])
            lauf.ereignis(art, **d)
        # Messen
        if time.time() >= naechste_messung:
            m = lauf.messen(aktiv)
            naechste_messung = time.time() + (60 if not args.kurz else 20)
            hoehen = {n: k["hoehe"] for n, k in m["knoten"].items()}
            hoechste = max((h for h in hoehen.values() if h is not None), default=0)
            if hoechste > letzte_hoehe:
                letzte_hoehe, stillstand_seit = hoechste, time.time()
            elif time.time() - stillstand_seit > 600:
                lauf.ereignis("stillstand", seit_min=round((time.time() - stillstand_seit) / 60, 1), hoehe=hoechste)
                stillstand_seit = time.time()
            if int(m["t_min"]) % 5 == 0 or args.kurz:
                best = {n: k["bestaetigt"] for n, k in m["knoten"].items()}
                print(f"[testnetz] {m['t_min']:7.1f} min  hoehen {hoehen}  bestaetigt {best}", flush=True)
        time.sleep(2)

    lauf.ereignis("ende")
    for n in aktiv:
        lauf.beenden(n)
    lauf.messen(aktiv)
    for n in list(ORTE):
        sh("docker", "rm", "-f", PRAEFIX + n, pruefen=False)
    sh("docker", "network", "rm", NETZ, pruefen=False)
    print("[testnetz] Fertig; Auswertung: testnetzauswertung.py --aus " + str(aus), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
