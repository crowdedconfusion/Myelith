#!/usr/bin/env python3
"""Capture-the-Flag-Simulation: prueft die Werkzeuge des Agenten an einer
Aufgabe mit einem ueberpruefbaren Ausgang.

# ⚑ Was das ist und was es nicht ist

**Ein Pruefstand fuer die eigenen Werkzeuge**, kein Angriffswerkzeug.
Jede Herausforderung ist **selbstenthalten und offline**: Der Laeufer
baut ein frisches Verzeichnis mit einer versteckten Flagge der Form
`MYL{...}`, gibt dem Agenten die Aufgabe, sie zu finden, und urteilt an
**einer** Tatsache: Nennt die Antwort die Flagge? Kein echtes Ziel, kein
Netz, keine fremde Maschine.

Das ist dieselbe Idee wie `agentenprobe.py` (der Pruefer sieht in die
Aussenwelt, nicht ins Modell), zugeschnitten auf die Sicherheits- und
Suchwerkzeuge des 1337-Toolkits: eine Aufgabe ist erst dann geloest,
wenn die Werkzeugkette wirklich zum Ziel gefuehrt hat.

# ⚑ Nach Werkzeug getaggt

Jede Herausforderung nennt in `braucht`, welche Werkzeuge sie braucht.
Was das geladene Modell in seiner Kiste nicht hat, wird **uebersprungen**
statt zu scheitern (wie `"satz": "voll"` in der Agentenprobe): Eine
Aufgabe, die mangels Werkzeug nicht laeuft, ist kein Fehler des Modells.
Heute laufen die `base`-Aufgaben (`list_directory`, `read_file`,
`search`); die `elite`-Aufgaben warten auf das 1337-Toolkit, die
Sicherheitswerkzeuge der Elite-Kiste.

# Aufruf

    python3 BENCHMARKS/Agent/ctf/ctf.py --selbsttest
    python3 BENCHMARKS/Agent/ctf/ctf.py INTEGER_LLM/artifacts/myelith-4b
    python3 BENCHMARKS/Agent/ctf/ctf.py <artefakt> --nur flagge_durchsuchen

`--selbsttest` baut jede Herausforderung und prueft **ohne Modell**, dass
die Flagge im Fixture wirklich vorkommt und der Pruefer sie findet: Eine
Herausforderung, die selbst unloesbar ist, misst nichts.

# Rueckgabewert

0, wenn jede gelaufene Herausforderung bestanden ist (bei `--selbsttest`:
jedes Fixture in Ordnung), sonst 1.
"""

from __future__ import annotations

import argparse
import base64
import json
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

WURZEL = Path(__file__).resolve().parents[3]
MYL = WURZEL / "SYSTEM/full-build" / "release" / "myl"
HERAUS = Path(__file__).resolve().parent / "herausforderungen"

# Welche Werkzeuge eine Kiste heute hat. ⚑ Von Hand und mit Absicht: Die
# eine Quelle im Code ist `CLIENT/myl-client/src/werkzeuge.rs`; diese
# Liste ist die Sicht der Probe darauf und faellt auf, wenn sie
# auseinanderlaeuft (eine base-Aufgabe, die ploetzlich uebersprungen
# wird, oder eine elite-Aufgabe, die ploetzlich laeuft).
KISTE = {
    "base": {"list_directory", "read_file", "search"},
    "advanced": {"list_directory", "read_file", "search", "fetch_url", "web_search"},
    "elite": {"list_directory", "read_file", "search", "crypto_tools", "scan_secrets", "run_command"},
}

FLAGGE = re.compile(r"MYL\{[^}]*\}")


def fixture_bauen(wurzel: Path, h: dict) -> None:
    """Legt die Dateien einer Herausforderung an, auch in Unterordnern."""
    for name, inhalt in (h.get("dateien") or {}).items():
        p = wurzel / name
        p.parent.mkdir(parents=True, exist_ok=True)
        # ⚑ `@@BASE64:...@@` wird beim Schreiben kodiert, damit die Flagge
        # nicht im Klartext im Fixture steht: Sonst faende `search` sie
        # ohne das Dekodierwerkzeug, und die Aufgabe pruefte nichts.
        m = re.search(r"@@BASE64:(.*?)@@", inhalt)
        if m:
            roh = base64.b64encode(m.group(1).encode()).decode()
            inhalt = inhalt.replace(m.group(0), roh)
        p.write_text(inhalt, encoding="utf-8")
    streu = h.get("streu")
    if streu:
        for i in range(streu["anzahl"]):
            zeile = streu["treffer"] if i == streu["treffer_bei"] else streu["muster"]
            (wurzel / f"eintrag_{i:03}.log").write_text(zeile.replace("{i}", str(i)), encoding="utf-8")


def flagge_im_fixture(wurzel: Path, flagge: str) -> bool:
    """Steht die Flagge irgendwo im Baum (dekodiert), also ist die Aufgabe loesbar?"""
    for p in wurzel.rglob("*"):
        if not p.is_file():
            continue
        text = p.read_text(encoding="utf-8", errors="replace")
        if flagge in text:
            return True
        for roh in re.findall(r"[A-Za-z0-9+/=]{16,}", text):
            try:
                if flagge in base64.b64decode(roh, validate=True).decode("utf-8", "replace"):
                    return True
            except Exception:
                pass
    return False


def selbsttest() -> int:
    fehler = 0
    for datei in sorted(HERAUS.glob("*.json")):
        h = json.loads(datei.read_text(encoding="utf-8"))
        flagge = h["flagge"]
        if not FLAGGE.fullmatch(flagge):
            print(f"[ctf] {h['name']}: Flagge {flagge!r} passt nicht zur Form MYL{{...}}")
            fehler += 1
            continue
        with tempfile.TemporaryDirectory(prefix="myl-ctf-") as tmp:
            wurzel = Path(tmp)
            fixture_bauen(wurzel, h)
            if flagge_im_fixture(wurzel, flagge):
                print(f"[ctf] {h['name']} ({h['stufe']}, braucht {', '.join(h['braucht'])}): Fixture ok, loesbar")
            else:
                print(f"[ctf] {h['name']}: die Flagge steht NICHT im Fixture, die Aufgabe ist unloesbar")
                fehler += 1
    print(f"[ctf] Selbsttest: {fehler} Fehler")
    return 1 if fehler else 0


def laufen(artefakt: str, stufe_der_kiste: str, nur: str | None, schritte: int) -> int:
    if not MYL.exists():
        print(f"[ctf] {MYL} fehlt; erst `cargo build --release` im Client.")
        return 1
    hat = KISTE.get(stufe_der_kiste, KISTE["base"])
    bestanden = uebersprungen = gescheitert = 0
    for datei in sorted(HERAUS.glob("*.json")):
        h = json.loads(datei.read_text(encoding="utf-8"))
        if nur and h["name"] != nur:
            continue
        fehlt = [w for w in h["braucht"] if w not in hat]
        if fehlt:
            print(f"[ctf] {h['name']}: uebersprungen, Kiste {stufe_der_kiste!r} hat {', '.join(fehlt)} nicht")
            uebersprungen += 1
            continue
        with tempfile.TemporaryDirectory(prefix="myl-ctf-") as tmp:
            wurzel = Path(tmp)
            fixture_bauen(wurzel, h)
            befehl = [str(MYL), "agent", artefakt, h["auftrag"],
                      "--schritte", str(schritte), "--bezeugtes", "--wurzel", str(wurzel)]
            anfang = time.monotonic()
            try:
                fertig = subprocess.run(befehl, capture_output=True, text=True, timeout=600)
                ausgabe = fertig.stdout + fertig.stderr
            except subprocess.TimeoutExpired:
                print(f"[ctf] {h['name']}: ZEITUEBERSCHREITUNG")
                gescheitert += 1
                continue
            dauer = time.monotonic() - anfang
            if h["flagge"] in ausgabe:
                print(f"[ctf] {h['name']}: BESTANDEN ({dauer:.0f} s)")
                bestanden += 1
            else:
                print(f"[ctf] {h['name']}: GESCHEITERT, die Antwort nennt die Flagge nicht ({dauer:.0f} s)")
                gescheitert += 1
    print(f"[ctf] {bestanden} bestanden, {gescheitert} gescheitert, {uebersprungen} uebersprungen")
    return 1 if gescheitert else 0


def main() -> int:
    ap = argparse.ArgumentParser(description="Capture-the-Flag-Simulation fuer die Agentenwerkzeuge.")
    ap.add_argument("artefakt", nargs="?", help="Artefaktverzeichnis eines Modells")
    ap.add_argument("--selbsttest", action="store_true", help="Fixtures ohne Modell pruefen")
    ap.add_argument("--kiste", default="base", choices=sorted(KISTE), help="welche Werkzeugkiste das Modell hat")
    ap.add_argument("--nur", help="nur diese Herausforderung")
    ap.add_argument("--schritte", type=int, default=12)
    args = ap.parse_args()
    if args.selbsttest or not args.artefakt:
        return selbsttest()
    return laufen(args.artefakt, args.kiste, args.nur, args.schritte)


if __name__ == "__main__":
    sys.exit(main())
