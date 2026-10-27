#!/usr/bin/env python3
"""Ein Probenetz im Docker, dem der Client beim Mining beitritt.

Ein Erzeuger (Blocktakt 100 ms, eine Epoche rund drei Minuten) und drei
Mitlaeufer, alle auf der Probekette und in einem Container. Die Mitlaeufer
sind als Miner angemeldet und tragen je ein festes Testkonto ein, ohne
Schluessel: Ihr Spielgeld aus der Probepraegung kann niemand ausgeben, und
das ist fuer Gegenstellen gewollt.

Der Erzeuger horcht auf einem Port, den Docker an den Mac reicht. Seine
Adresse steht danach in `<ordner>/einstieg.txt`; der Client liest sie von
dort und startet seinen Knoten mit `--bootstrap` statt als Erzeuger.

Aufruf (aus der Wurzel des Klons):

    python3 TESTCLIENT/netzbedingungen/probenetz.py starten --ordner <ordner>
    python3 TESTCLIENT/netzbedingungen/probenetz.py stand --ordner <ordner>
    python3 TESTCLIENT/netzbedingungen/probenetz.py stoppen

Vorher einmal das Linux-Programm in das Volume `myl-netem-ziel` bauen (siehe
`--programm`); das Bild `myelith-netem` bringt die Werkzeugkette mit.

⚠️ Nur ein Pruefgeraet: Kette, Schluessel und Guthaben sind Spielgeld auf
der Probekette und gelten nirgends sonst.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import signal
import subprocess
import sys
import time

WURZEL = pathlib.Path(__file__).resolve().parents[2]
BEHAELTER = "myl-probenetz"
BILD = "myelith-netem"
VOLUME = "myl-netem-ziel"
PROGRAMM = "/ziel/n0620/release/myl-node"
PORT = 4100
# Der Beobachtungsendpunkt des Erzeugers, fuer `/konto/<adresse>`: Der
# Client fragt dort, ob ein Wallet schon Spuren auf der Kette hat.
BEOBACHTUNG = 4180
MITLAEUFER = 3
BLOCKTAKT_MS = 100


def testkonto(name: str) -> str:
    """Ein festes Konto ohne Schluessel, aus dem Namen."""
    return hashlib.sha256(f"myelith-probenetz-testkonto-{name}".encode()).hexdigest()


# ---- Auf dem Mac -----------------------------------------------------------

def starten(args: argparse.Namespace) -> int:
    ordner = args.ordner.resolve()
    ordner.mkdir(parents=True, exist_ok=True)
    if laeuft():
        print(f"{BEHAELTER} laeuft schon; Einstieg in {ordner / 'einstieg.txt'}")
        return 0
    (ordner / "einstieg.txt").unlink(missing_ok=True)
    befehl = [
        "docker", "run", "-d", "--rm", "--name", BEHAELTER,
        "-e", "PYTHONDONTWRITEBYTECODE=1",
        "-p", f"127.0.0.1:{args.port}:{PORT}",
        "-p", f"127.0.0.1:{args.abfrage}:{BEOBACHTUNG}",
        "-v", f"{WURZEL}:/repo:ro", "-v", f"{VOLUME}:/ziel:ro", "-v", f"{ordner}:/aus",
        "-w", "/repo", BILD,
        "python3", "-u", "TESTCLIENT/netzbedingungen/probenetz.py", "innen",
        "--programm", args.programm, "--host-port", str(args.port), "--host-abfrage", str(args.abfrage),
    ]
    r = subprocess.run(befehl, capture_output=True, text=True)
    if r.returncode != 0:
        print(r.stderr.strip(), file=sys.stderr)
        return 1
    ende = time.monotonic() + 60
    while time.monotonic() < ende:
        if (ordner / "einstieg.txt").exists():
            print(f"Probenetz laeuft. Einstieg: {(ordner / 'einstieg.txt').read_text().strip()}")
            print(f"Fuer den Client: MYL_PROBENETZ={ordner / 'einstieg.txt'}")
            return 0
        if not laeuft():
            print("der Behaelter endete; siehe docker logs", file=sys.stderr)
            return 1
        time.sleep(1)
    print("kein Einstieg nach 60 s", file=sys.stderr)
    return 1


def laeuft() -> bool:
    r = subprocess.run(["docker", "ps", "-q", "-f", f"name=^{BEHAELTER}$"], capture_output=True, text=True)
    return bool(r.stdout.strip())


def stoppen(_: argparse.Namespace) -> int:
    if not laeuft():
        print(f"{BEHAELTER} laeuft nicht")
        return 0
    subprocess.run(["docker", "stop", "-t", "20", BEHAELTER], capture_output=True)
    print(f"{BEHAELTER} gestoppt; Kette und Schluessel bleiben im Ordner")
    return 0


def letzte_aufnahme(ordner: pathlib.Path) -> dict | None:
    dateien = sorted(ordner.glob("*.jsonl"), key=lambda p: p.stat().st_mtime)
    for datei in reversed(dateien):
        for zeile in reversed(datei.read_text(encoding="utf-8", errors="replace").splitlines()):
            if '"art":"aufnahme"' in zeile:
                try:
                    return json.loads(zeile)
                except ValueError:
                    continue
    return None


def stand(args: argparse.Namespace) -> int:
    ordner = args.ordner.resolve()
    print(f"{BEHAELTER}: {'laeuft' if laeuft() else 'steht'}")
    for name in ["k0"] + [f"k{i}" for i in range(1, MITLAEUFER + 1)]:
        a = letzte_aufnahme(ordner / name)
        if a is None:
            print(f"  {name}: noch keine Aufnahme")
            continue
        teile = [f"Hoehe {a.get('hoehe')}", f"Peers {a.get('peers')}"]
        if "auszahlung_guthaben" in a:
            teile.append(f"Testkonto {a['auszahlung_guthaben'] / 1_000_000:g} MYL")
        print(f"  {name}: " + ", ".join(teile))
    return 0


# ---- Im Behaelter ----------------------------------------------------------

def innen(args: argparse.Namespace) -> int:
    aus = pathlib.Path("/aus")
    prozesse: dict[str, subprocess.Popen] = {}

    # `docker stop` schickt SIGTERM; die Knoten sollen sauber enden.
    def abbruch(*_: object) -> None:
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, abbruch)

    def start(name: str, zusatz: list[str]) -> subprocess.Popen:
        d = aus / name
        d.mkdir(parents=True, exist_ok=True)
        befehl = [
            args.programm, "--name", name,
            "--schluessel", str(d / "knoten.key"),
            "--kette", str(d / "kette.dat"),
            "--protokolle", str(d),
            "--aufnahme", "5",
            "--still",
        ] + zusatz
        fehler = open(d / "stderr.txt", "a", encoding="utf-8")
        return subprocess.Popen(befehl, stdout=fehler, stderr=subprocess.STDOUT)

    def peer_id(name: str, frist: float = 30) -> str:
        ende = time.monotonic() + frist
        while time.monotonic() < ende:
            text = (aus / name / "stderr.txt").read_text(encoding="utf-8", errors="replace")
            ids = [z.split("Peer-Id ", 1)[1].strip() for z in text.splitlines() if "Peer-Id " in z]
            if ids:
                return ids[-1]
            time.sleep(0.2)
        raise SystemExit(f"{name} nannte keine Peer-Id")

    erzeuger = ["--port", str(PORT), "--erzeuger", "--blocktakt", str(BLOCKTAKT_MS),
                "--beobachtung", f"0.0.0.0:{BEOBACHTUNG}"]
    prozesse["k0"] = start("k0", erzeuger)
    innen_einstieg = f"/ip4/127.0.0.1/tcp/{PORT}/p2p/{peer_id('k0')}"
    mitlaeufer = {}
    for i in range(1, MITLAEUFER + 1):
        name = f"k{i}"
        mitlaeufer[name] = [
            "--port", str(PORT + i), "--bootstrap", innen_einstieg,
            "--konsensschluessel", str(aus / name / "konsens.key"),
            "--konto", testkonto(name),
        ]
        prozesse[name] = start(name, mitlaeufer[name])
    # Erst wenn alle stehen, bekommt der Mac den Einstieg.
    for name in mitlaeufer:
        peer_id(name)
    einstieg = f"/ip4/127.0.0.1/tcp/{args.host_port}/p2p/{peer_id('k0')}"
    # Zweite Zeile: wo sich ein Knoten befragen laesst (`/konto/…`).
    (aus / "einstieg.txt").write_text(f"{einstieg}\nbeobachtung 127.0.0.1:{args.host_abfrage}\n", encoding="utf-8")
    print(f"[probenetz] Einstieg {einstieg}", flush=True)

    # Wer abstuerzt, wird mit denselben Angaben neu gestartet.
    try:
        while True:
            for name, p in list(prozesse.items()):
                if p.poll() is not None:
                    print(f"[probenetz] {name} endete mit {p.returncode}, neu gestartet", flush=True)
                    prozesse[name] = start(name, erzeuger if name == "k0" else mitlaeufer[name])
            time.sleep(2)
    except KeyboardInterrupt:
        pass
    finally:
        for p in prozesse.values():
            p.terminate()
        for p in prozesse.values():
            try:
                p.wait(timeout=15)
            except subprocess.TimeoutExpired:
                p.kill()
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    unter = p.add_subparsers(dest="befehl", required=True)
    s = unter.add_parser("starten")
    s.add_argument("--ordner", type=pathlib.Path, required=True, help="Kette, Schluessel, Protokolle, Einstieg")
    s.add_argument("--port", type=int, default=PORT, help="der Port auf dem Mac")
    s.add_argument("--abfrage", type=int, default=BEOBACHTUNG, help="der Port des Beobachtungsendpunkts auf dem Mac")
    s.add_argument("--programm", default=PROGRAMM, help="myl-node fuer Linux, im Volume")
    s.set_defaults(tun=starten)
    s = unter.add_parser("stand")
    s.add_argument("--ordner", type=pathlib.Path, required=True)
    s.set_defaults(tun=stand)
    s = unter.add_parser("stoppen")
    s.set_defaults(tun=stoppen)
    s = unter.add_parser("innen")
    s.add_argument("--programm", required=True)
    s.add_argument("--host-port", type=int, required=True)
    s.add_argument("--host-abfrage", type=int, required=True)
    s.set_defaults(tun=innen)
    args = p.parse_args()
    return args.tun(args)


if __name__ == "__main__":
    sys.exit(main())
