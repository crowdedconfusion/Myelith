#!/usr/bin/env python3
"""Startet das Loop-Szenario: zwei Aufgaben, ein Testskill, eine Frist.

    python3 BENCHMARKS/Agent/loop/starten.py <artefakt> <laufordner> [--frist-min 180]

Legt im Laufordner an:
  arbeit/     eine frische Kopie von vorlage/ (Arbeitsordner des Agenten)
  cfg/        eigene Einstellungen (MYL_EINSTELLUNGEN), die echten bleiben unberuehrt
  loop.log    die Ausgabe von `myl loop`
  lauf.json   Start, Frist, Artefakt, Prozessnummer
  starter.log was dieses Skript tut; die letzte Zeile ist "Fertig: <grund>"

Die Frist ist hart: Danach bekommt `myl loop` ein SIGINT (geordnetes Schliessen,
der Stand bleibt), eine Minute spaeter ein SIGKILL.
"""
import json
import os
import shutil
import signal
import subprocess
import sys
import time

HIER = os.path.dirname(os.path.abspath(__file__))
WURZEL = os.path.abspath(os.path.join(HIER, "..", "..", ".."))
MYL = os.path.join(WURZEL, "SYSTEM", "full-build", "release", "myl")


def zahl(args, name, vorgabe):
    if name in args:
        return int(args[args.index(name) + 1])
    return vorgabe


def main(args):
    if len(args) < 2:
        print(__doc__)
        return 2
    artefakt = os.path.abspath(args[0])
    lauf = os.path.abspath(args[1])
    frist_min = zahl(args, "--frist-min", 180)
    runden = zahl(args, "--runden", 10)
    schritte = zahl(args, "--schritte", 16)
    if not os.path.isfile(MYL):
        print(f"myl fehlt: {MYL}\n  cd CLIENT/myl-client && cargo build --release --bin myl")
        return 1
    if not os.path.isfile(os.path.join(artefakt, "model_config.json")):
        print(f"kein Artefakt: {artefakt}")
        return 1
    # ⛔️ Ein Laufordner im Repositorium wuerde mit versioniert.
    if os.path.commonpath([lauf, WURZEL]) == WURZEL:
        print("der Laufordner darf nicht im Repositorium liegen")
        return 1
    if os.path.exists(lauf):
        print(f"{lauf} gibt es schon; ein Lauf beginnt in einem frischen Ordner")
        return 1

    arbeit = os.path.join(lauf, "arbeit")
    cfg = os.path.join(lauf, "cfg")
    shutil.copytree(os.path.join(HIER, "vorlage"), arbeit)
    os.makedirs(cfg)
    umgebung = dict(os.environ, MYL_EINSTELLUNGEN=os.path.join(cfg, "client.json"), MYELITH_WURZEL=WURZEL)
    protokoll = open(os.path.join(lauf, "starter.log"), "a", buffering=1)

    def notiz(text):
        protokoll.write(f"{time.strftime('%H:%M:%S')} {text}\n")

    def setzen(feld, wert):
        r = subprocess.run([MYL, "setzen", feld, str(wert)], env=umgebung, capture_output=True, text=True)
        if r.returncode != 0:
            raise RuntimeError(f"myl setzen {feld}: {r.stderr.strip()}")

    for feld, wert in [
        ("modell.artefakt", artefakt),
        ("agent.wurzel", arbeit),
        ("agent.schreiben", "an"),
        ("agent.kistenordner", os.path.join(WURZEL, "CLIENT", "werkzeugkisten", "Advanced")),
        ("agent.warnung", "aus"),
        # ⚠️ auto: Ohne Terminal kann niemand bestaetigen. Der Arbeitsordner
        #    ist eine Kopie, und die Einstellungen sind abgeschirmt.
        ("agent.modus", "auto"),
        ("loop.runden", runden),
        ("loop.stunden", max(1, (frist_min + 59) // 60)),
        ("loop.schritte", schritte),
        ("loop.stillstand", 3),
        ("loop.pruefen", "an"),
    ]:
        setzen(feld, wert)
    notiz(f"Einstellungen gesetzt: {runden} Runden, {schritte} Schritte, Frist {frist_min} min")

    with open(os.path.join(HIER, "aufgaben.json"), encoding="utf-8") as f:
        aufgaben = json.load(f)["aufgaben"]
    kennungen = {}
    for a in aufgaben:
        befehl = [MYL, "tasks", "add", a["ziel"]]
        # Die Abnahme entscheidet ueber „fertig“, wo es einen pruefbaren Befehl gibt.
        if a.get("abnahme") and "--ohne-abnahme" not in args:
            befehl += ["--abnahme", a["abnahme"]]
        r = subprocess.run(befehl, env=umgebung, capture_output=True, text=True)
        if r.returncode != 0:
            raise RuntimeError(f"myl tasks add: {r.stderr.strip()}")
        kennungen[a["kennung"]] = r.stdout.strip()
        time.sleep(1.1)  # Kennungen tragen die Sekunde; so bleibt die Reihenfolge eindeutig
    notiz(f"Tasks angelegt: {kennungen}")

    start = time.time()
    frist = start + frist_min * 60
    log = open(os.path.join(lauf, "loop.log"), "w", buffering=1)
    p = subprocess.Popen([MYL, "loop"], cwd=arbeit, env=umgebung, stdout=log, stderr=subprocess.STDOUT)
    with open(os.path.join(lauf, "lauf.json"), "w", encoding="utf-8") as f:
        json.dump({"start": start, "frist": frist, "artefakt": artefakt, "pid": p.pid,
                   "tasks": kennungen, "runden": runden, "schritte": schritte,
                   "abnahme": "--ohne-abnahme" not in args}, f, indent=2)
    notiz(f"myl loop gestartet, Prozess {p.pid}")

    grund = "alle Tasks beendet"
    while p.poll() is None:
        if time.time() >= frist:
            grund = f"Frist von {frist_min} min erreicht"
            notiz(grund + ": SIGINT")
            p.send_signal(signal.SIGINT)
            try:
                p.wait(timeout=60)
            except subprocess.TimeoutExpired:
                notiz("nach 60 s noch da: SIGKILL")
                p.kill()
                p.wait()
            break
        time.sleep(5)
    notiz(f"myl loop endete mit {p.returncode} nach {(time.time() - start) / 60:.1f} min")
    notiz(f"Fertig: {grund}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Exception as f:
        print(f"FEHLER: {f}")
        sys.exit(1)
