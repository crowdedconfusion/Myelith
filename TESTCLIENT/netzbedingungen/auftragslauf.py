#!/usr/bin/env python3
"""Der Auftragslauf: bezahlte agentische Auftraege ueber Zwillingspods mit
dem 35B, ueber mehrere Epochen einer Kette, auf einer Maschine.

Aufruf (macOS oder Linux, ohne root, aus der Wurzel):

```text
python3 -u TESTCLIENT/netzbedingungen/auftragslauf.py --kurz          # die Mechanik mit einem Auftrag
python3 -u TESTCLIENT/netzbedingungen/auftragslauf.py --slot 600      # alle Auftraege der Mappe
python3 -u TESTCLIENT/netzbedingungen/auftragslauf.py --bis 05:30     # Auftraege bis zu dieser Uhrzeit
```

Gebaut werden vorher (Freigabebau, denn nur dort steht das 35B auf der
Netzliste): `myl-node` (NODE), `myl-shard` und `myl-pod-node`
(COMPUTE_PIPELINE/myl-pod), `myl` (CLIENT/myl-client) und das Beispiel
`probegeld` (TESTCLIENT/myl-testclient, `--example probegeld`).

# Was der Lauf aufbaut

Ein Torknoten (Erzeuger, Tuer, Relaisleitung, Ortsdienst) und dreizehn
Miner im Testnetz mit dem 35B in vier Bit (`--netz test-int4`), jeder ein
eigener Prozess mit eigenen Ports auf der Rueckschleife. Dreizehn Miner
ergeben zwei Pods, also ein Zwillingspaar: Der Ortsdienst rechnet jeden
Auftrag ueber den zugelosten Pod und im Hintergrund ueber dessen Zwilling,
und nur ein Paar mit gleicher Ausgabe wird verguetet (Stufe 1). Die Shards
startet jeder Knoten selbst, mit dem Bereich, den die Kette seiner Position
zuteilt.

Ein Nutzer mit Wallet bekommt Probegeld aus den Probekonten (das Testnetz
praegt keines), kauft Credits und eroeffnet je Auftrag eine Sitzung mit
Vollmacht fuer einen Agenten. Der Agent (`myl sitzung --tuer`) arbeitet mit
seinen Werkzeugen in einem eigenen Ordner; jede Runde seines Modells geht
als fertiger Prompt ueber die Tuer an die Pods und wird aus der Sitzung
bezahlt. Vor jeder Runde kauft der Klient mit dem Wallet des Inhabers
Credits nach (`--nachkauf`), auf wenige Runden Vorrat und nie ueber das
Budget der Sitzung; so traegt ein Auftrag ueber Epochenwechsel.

# Was der Lauf misst und meldet

- je Auftrag: Ergebnis nach den Merkmalen der Mappe, Runden, Token hinein
  und heraus, Dauer, Epochen, abgebuchte Credits;
- je Epoche: Hoehe, Zwillingspaare einig und uneinig, Credit-Preis, was die
  Auszahlungskonten der Miner bekamen;
- am Ende: eine Hoehe und eine Zustandswurzel bei allen Knoten.

Mit `--luegner` sagt der letzte Miner 1 µs je Ebene zu, bei sechs Ebenen
Kapazitaet: Er rechnet richtig, aber weit langsamer als angesagt. Verlangt ist,
dass die Kette ihn sperrt, sobald er auf einer Position stand, und keinen
anderen. Mit `--stoerer` stoert ein Prozess die Netzports aller Knoten und
die Tuer (halbe Verbindungen, Muell, Stoesse), wie im Netzlauf. Der Bericht
zaehlt die Aufrufe von `run_command` und nennt die Sandbox-Zeile des Agenten.

Befunde werden gesammelt und am Ende gemeldet, statt den Lauf beim ersten
abzubrechen. Alles liegt im Laufordner unter `Temporär/auftragslauf/`:
`verlauf.jsonl` (je Ereignis eine Zeile), `bericht.md`, je Auftrag sein
Arbeitsordner und das Protokoll des Agenten.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import pathlib
import re
import signal
import socket
import subprocess
import sys
import time
import urllib.request

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import auftragsmappe  # noqa: E402
from netzlauf_prozesse import STOERER_SKRIPT  # noqa: E402

WURZEL = pathlib.Path(__file__).resolve().parents[2]
TEMPORAER = WURZEL.parent / "Temporär"
BAU = WURZEL / "SYSTEM" / "full-build" / "release"
ARTEFAKT = WURZEL / "INTEGER_LLM" / "artifacts" / "raven-35b-a3b-int4"

# `myl_consensus::block::BLOECKE_JE_EPOCHE`.
EPOCHE = 1800
# Die Ports: je Knoten zwei (Netz, Beobachtung), dazu Tuer, Ortsleitung und
# Relaisleitung des Torknotens; alle auf der Rueckschleife.
BASIS = 47100
TUER = "127.0.0.1:47090"
ORT = "127.0.0.1:47091"
RELAIS = "127.0.0.1:47092"
# Die Kennung des Sitzungskanals zwischen Tuer und Ortsdienst.
POD_HEX = "71" * 32
# Die Woerter des Zahlers: ein Pruefvektor aus BIP-39, im Test bekannt.
ZAHLER_WOERTER = "legal winner thank year wave sausage worth useful legal winner thank yellow"
SCHLUESSELKOPF = "# Myelith-Konsensschluessel (BLS12-381). GEHEIM. Nicht weitergeben."
# Jeder Miner hinterlegt 0,5 MYL; ohne Einsatz schlachtet ein Urteil nichts.
EINSATZ = 500_000
# Dreizehn Miner: zwei Pods zu je vier Positionen und zwei Reserven, dazu
# eine Reserve mehr. Jeder sagt alle Ebenen samt Kopf zu (40 und 4); die
# Zeit je Ebene liegt beim Zehnfachen des Gemessenen (rund 2,5 ms je Ebene
# und Token mit allen Kernen), denn zwei Pods teilen sich die Maschine.
MINER = [(f"m{i:02d}", 44, 20_000 + 2_500 * (i % 5)) for i in range(13)]
# Der Luegner (`--luegner`): sechs Ebenen, 1 µs je Ebene angesagt. Sechs,
# damit er mehr als eine Ebene haelt und die Zeitprobe ihn sicher misst
# (Fund 655: eine Ebene bleibt unter der Zugabe).
LUEGNER = ("m12", 6, 1)
UNITS_PER_MYL = 1_000_000
# Die Miner im Stimmsatz, neben dem Erzeuger.
VALIDATOREN = ["m01", "m06", "m09"]


def melde(text: str) -> None:
    print(f"[auftragslauf {dt.datetime.now():%H:%M:%S}] {text}", flush=True)


class Befund(Exception):
    """Ein Lauf, der nicht traegt."""


def abdruck(artefakt: pathlib.Path) -> str:
    """Der Abdruck eines Artefakts, wie `integer_llm_runtime::netzfreigabe`."""
    text = "myelith-artefakt-abdruck 1\n"
    for d in ("theta_v.json", "model_config.json", "tokenizer.json"):
        p = artefakt / d
        text += f"{d} {hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else '-'}\n"
    return hashlib.sha256(text.encode()).hexdigest()


def testkonto(name: str) -> str:
    """Ein festes Auszahlungskonto ohne Schluessel fuer einen Miner."""
    return hashlib.sha256(f"myelith-testlauf-konto-{name}".encode()).hexdigest()


def frei(port: int) -> bool:
    with socket.socket() as s:
        # Wie die Programme selbst: Reste im Nachlauf (TIME_WAIT) eines eben
        # beendeten Laufs belegen den Port nicht.
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            s.bind(("127.0.0.1", port))
            return True
        except OSError:
            return False


class Ortsknoten:
    """Ein `myl-node` als Prozess auf dieser Maschine, mit eigenen Ports."""

    teilnehmer: list[str] = []

    def __init__(self, nummer: int, ordner: pathlib.Path, rolle: tuple[str, int, int] | None):
        self.nummer = nummer
        self.name = f"k{nummer}"
        self.ordner = ordner / self.name
        self.port = BASIS + 10 * nummer
        self.beob = self.port + 1
        self.rolle = rolle
        self.prozess: subprocess.Popen | None = None
        self.peer_id: str | None = None
        self._gelesen: dict[str, int] = {}

    @property
    def miner(self) -> str:
        return self.rolle[0] if self.rolle else "k0"

    def starten(self, programm: pathlib.Path, bootstrap: str | None, slot: int, zusatz: list[str]) -> None:
        self.ordner.mkdir(parents=True, exist_ok=True)
        befehl = [
            str(programm), "--name", self.name, "--netz", "test-int4",
            "--port", str(self.port),
            "--schluessel", str(self.ordner / "knoten.key"),
            "--kette", str(self.ordner / "kette.dat"),
            "--protokolle", str(self.ordner),
            "--beobachtung", f"127.0.0.1:{self.beob}",
            "--aufnahme", "2",
            "--testverkehr", "5",
            "--blocktakt", str(slot),
        ]
        for name in Ortsknoten.teilnehmer:
            befehl += ["--teilnehmer", name]
        if bootstrap:
            befehl += ["--bootstrap", bootstrap]
        befehl += zusatz
        fehler = open(self.ordner / "stderr.txt", "a", encoding="utf-8")
        self.prozess = subprocess.Popen(befehl, cwd=WURZEL, stdout=fehler, stderr=subprocess.STDOUT,
                                        env=dict(os.environ), start_new_session=True)

    def warte_auf_peer_id(self, frist: float) -> str:
        ende = time.monotonic() + frist
        while time.monotonic() < ende:
            self.lebt_noch()
            for zeile in (self.ordner / "stderr.txt").read_text(encoding="utf-8", errors="replace").splitlines():
                if "Peer-Id " in zeile:
                    self.peer_id = zeile.split("Peer-Id ", 1)[1].strip()
                    return self.peer_id
            time.sleep(0.2)
        raise Befund(f"{self.name} nannte keine Peer-Id")

    def lebt_noch(self) -> None:
        if self.prozess is not None and self.prozess.poll() is not None:
            raise Befund(f"{self.name} endete mit {self.prozess.returncode}, siehe {self.ordner / 'stderr.txt'}")

    def shards(self) -> list[int]:
        """Die Shards, die dieser Knoten gestartet hat: seine Kinder."""
        if self.prozess is None or self.prozess.poll() is not None:
            return []
        r = subprocess.run(["pgrep", "-P", str(self.prozess.pid)], capture_output=True, text=True)
        return [int(x) for x in r.stdout.split()]

    def beenden(self) -> None:
        """Den Knoten samt seinen Shards: Er fuehrt eine eigene Prozessgruppe."""
        if self.prozess is None or self.prozess.poll() is not None:
            return
        try:
            os.killpg(self.prozess.pid, signal.SIGTERM)
        except OSError:
            self.prozess.send_signal(signal.SIGTERM)
        try:
            self.prozess.wait(timeout=20)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(self.prozess.pid, signal.SIGKILL)
            except OSError:
                self.prozess.kill()
            self.prozess.wait()

    def metriken(self) -> dict[str, float]:
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{self.beob}/metriken", timeout=3) as r:
                text = r.read().decode()
        except OSError:
            return {}
        aus = {}
        for z in text.splitlines():
            if z.startswith("#") or " " not in z:
                continue
            name, wert = z.rsplit(" ", 1)
            try:
                aus[name] = float(wert)
            except ValueError:
                pass
        return aus

    def hoehe(self) -> int:
        return int(self.metriken().get("myelith_kette_hoehe", -1))

    def konto(self, adresse: str) -> dict | None:
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{self.beob}/konto/{adresse}", timeout=5) as r:
                return json.loads(r.read())
        except (OSError, ValueError):
            return None

    def zeilen(self, art: str) -> list[dict]:
        """Die neuen Protokollzeilen einer Art, ohne alles zu zerlegen."""
        marke = f'"{art}"'
        aus = []
        for datei in sorted(self.ordner.glob("*.jsonl")):
            for z in datei.read_text(encoding="utf-8", errors="replace").splitlines():
                if marke in z:
                    try:
                        aus.append(json.loads(z))
                    except json.JSONDecodeError:
                        pass
        return aus


def warte(bedingung, frist: float, was: str, knoten: list[Ortsknoten], takt: float = 2.0) -> float:
    beginn = time.monotonic()
    while True:
        for k in knoten:
            k.lebt_noch()
        if bedingung():
            return time.monotonic() - beginn
        if time.monotonic() - beginn > frist:
            raise Befund(f"nach {frist:.0f} s nicht erreicht: {was}")
        time.sleep(takt)


def shard_pids(ordner: pathlib.Path) -> list[int]:
    """Uebrig gebliebene Shards dieses Laufs: Prozesse von `myl-shard`, deren
    Aufruf den Laufordner nennt (ihr Ausweis liegt darin)."""
    # Das Programm steht vorn: Ein Knoten nennt `myl-shard` nur hinter
    # `--shard-programm`.
    r = subprocess.run(["pgrep", "-f", f"^[^ ]*myl-shard .*{ordner.name}"], capture_output=True, text=True)
    return [int(p) for p in r.stdout.split()]


def lauf(args: argparse.Namespace, ordner: pathlib.Path) -> int:
    verlauf = open(ordner / "verlauf.jsonl", "a", encoding="utf-8")

    def notiere(art: str, **felder) -> None:
        verlauf.write(json.dumps({"art": art, "zeit": dt.datetime.now().isoformat(timespec="seconds"), **felder},
                                 ensure_ascii=False) + "\n")
        verlauf.flush()

    befunde: list[str] = []
    slot = args.slot
    pipeline = abdruck(args.artefakte)
    melde(f"Laufordner {ordner}; Slot {slot} ms, Epoche {EPOCHE * slot / 60000:.1f} min; Artefakt {args.artefakte.name} "
          f"(Abdruck {pipeline[:16]})")
    for port in [BASIS + 10 * i + d for i in range(len(MINER) + 1) for d in (0, 1)] + [int(TUER.split(":")[1]),
                                                                                         int(ORT.split(":")[1]),
                                                                                         int(RELAIS.split(":")[1])]:
        if not frei(port):
            raise Befund(f"Port {port} ist belegt")

    rollen = MINER[:-1] + [LUEGNER] if args.luegner else MINER
    alle = [Ortsknoten(0, ordner, None)] + [Ortsknoten(i + 1, ordner, m) for i, m in enumerate(rollen)]
    luegner = next((k for k in alle if args.luegner and k.rolle == LUEGNER), None)
    stoerer: subprocess.Popen | None = None
    erzeuger, miner = alle[0], alle[1:]
    Ortsknoten.teilnehmer = [k.name for k in alle]
    umgebung = {**os.environ, "MYL_WALLETS": str(ordner / "wallets"), "MYL_EINSTELLUNGEN": str(ordner / "client.json")}
    relais_angaben = ["--relais", RELAIS, "--relaisausweis", str(erzeuger.ordner / "relais.ausweis")]

    def myl(*befehl: str, eingabe: str | None = None, frist: float = 900) -> subprocess.CompletedProcess:
        return subprocess.run([str(args.myl), *befehl], input=eingabe, capture_output=True, text=True, env=umgebung,
                              cwd=WURZEL, timeout=frist)

    # ---- Der Zahler und der Torknoten -----------------------------------------
    r = myl("wallet", "wiederherstellen", "zahler", eingabe=ZAHLER_WOERTER + "\n")
    if r.returncode != 0:
        raise Befund(f"das Wallet des Zahlers: {r.stderr.strip()}")
    zahler = next(z.split()[-1] for z in r.stdout.splitlines() if "wiederhergestellt" in z)
    erzeuger.ordner.mkdir(parents=True, exist_ok=True)
    schluessel = erzeuger.ordner / "konsens.key"
    for k in [erzeuger] + [m for m in miner if m.miner in VALIDATOREN]:
        k.ordner.mkdir(parents=True, exist_ok=True)
        (k.ordner / "konsens.key").write_text(f"{SCHLUESSELKOPF}\n{os.urandom(32).hex()}\n")
        (k.ordner / "konsens.key").chmod(0o600)
    # ⚑ **Der Stimmsatz** (Testnetz): Jeder Block braucht die Unterschrift
    #   eines Mitglieds, und keiner darf ein Drittel halten. Vier Validatoren
    #   zu gleichen Teilen, der Erzeuger und drei Miner; ein Pruefpunkt
    #   braucht drei Stimmen. Jeder Knoten liest dieselbe Datei.
    zeilen_satz = ["netz test-int4", f"start {int(time.time() * 1000)}"]
    for k in [erzeuger] + [m for m in miner if m.miner in VALIDATOREN]:
        k.ordner.mkdir(parents=True, exist_ok=True)
        r = subprocess.run([str(args.programm), "--name", k.name, "--netz", "test-int4", "--stimmsatzzeile", "1000",
                            "--konsensschluessel", str(k.ordner / "konsens.key")], capture_output=True, text=True,
                           cwd=WURZEL, timeout=60)
        zeile = next((z for z in r.stdout.splitlines() if z.startswith("validator ")), None)
        if zeile is None:
            raise Befund(f"keine Stimmsatzzeile fuer {k.name}: {r.stderr.strip()[-300:]}")
        zeilen_satz.append(zeile)
    stimmsatz = ordner / "stimmsatz.txt"
    stimmsatz.write_text("\n".join(zeilen_satz) + "\n")
    (erzeuger.ordner / "ort").mkdir(exist_ok=True)
    ortsdienst = subprocess.Popen(
        [str(args.pod_node), "--artefakte", str(args.artefakte), "--ortsleitung", ORT,
         "--ausweis", str(erzeuger.ordner / "ort"), "--pod", POD_HEX, "--knotenschluessel", str(schluessel),
         "--pipeline", pipeline, "--epoche", "0", "--deckel", str(args.deckel),
         "--podrelais", RELAIS, "--podrelaisausweis", str(erzeuger.ordner / "relais.ausweis")],
        cwd=WURZEL, stdout=open(erzeuger.ordner / "ortsdienst.txt", "a"), stderr=subprocess.STDOUT,
        start_new_session=True)
    try:
        warte(lambda: "Shard-Dienst auf" in (erzeuger.ordner / "ortsdienst.txt").read_text(errors="replace"),
              120, "der Ortsdienst oeffnet", [])
        erzeuger.starten(args.programm, None, slot, [
            "--erzeuger", "--konsensschluessel", str(schluessel), "--stimmsatz", str(stimmsatz),
            "--tuer", TUER, "--ortsleitung", ORT, "--ortsausweis", str(erzeuger.ordner / "ort"), "--pod", POD_HEX,
            "--podrelais", RELAIS, "--podrelaisausweis", str(erzeuger.ordner / "relais.ausweis")])
        einstieg = f"/ip4/127.0.0.1/tcp/{erzeuger.port}/p2p/{erzeuger.warte_auf_peer_id(60)}"
        melde(f"Torknoten k0 mit Tuer {TUER} und Ortsdienst; Zahler {zahler[:16]} ist Konto von m00")
        for k in miner:
            name, ebenen, us = k.rolle
            k.starten(args.programm, einstieg, slot, [
                "--konsensschluessel", str(k.ordner / "konsens.key"), "--stimmsatz", str(stimmsatz),
                "--modellzusage", f"{ebenen}:{us}", "--zone", "Europe",
                "--shard-programm", str(args.shard), "--artefakte", str(args.artefakte),
                # Alle Shards dieses Laufs teilen sich eine Maschine und damit
                # eine GPU. Die Einmessung misst jeden allein und waehlte am
                # 35B Metal ab 64 Eingaben; im Pod verloren dann alle, weil die
                # Stufen sich an der einen GPU anstellen (80,6 s gegen 61,5 s fuer
                # 3 563 Prompt-Token). Der Lauf gibt deshalb nur die CPU frei,
                # wie es ein Besitzer mit dieser Maschine taete.
                "--rechenwerke", args.rechenwerke,
                "--einsatz", str(EINSATZ), "--konto", zahler if name == "m00" else testkonto(name),
                "--ohne-tuer"])
        for k in miner:
            k.warte_auf_peer_id(60)
        if args.stoerer:
            ziele = [f"127.0.0.1:{k.port}" for k in alle] + [TUER]
            (ordner / "stoerer").mkdir(exist_ok=True)
            stoerer = subprocess.Popen([sys.executable, "-c", STOERER_SKRIPT, "0", *ziele],
                                       stdout=open(ordner / "stoerer" / "stoerer.txt", "a"), stderr=subprocess.STDOUT,
                                       start_new_session=True)
            melde(f"Stoerer gegen {len(ziele)} Ziele")
        if luegner is not None:
            melde(f"Luegner {luegner.name} ({LUEGNER[0]}): {LUEGNER[1]} Ebenen, {LUEGNER[2]} µs je Ebene angesagt")
        notiere("start", slot=slot, miner=len(miner), artefakt=args.artefakte.name, abdruck=pipeline)

        def epoche() -> int:
            return max(0, erzeuger.hoehe()) // EPOCHE

        def stand() -> dict[str, dict]:
            """Wer einen laufenden Shard hat, mit seiner letzten Startzeile."""
            aus = {}
            for k in miner:
                if k.shards():
                    g = k.zeilen("shard_gestartet")
                    if g:
                        aus[k.miner] = g[-1]
            return aus

        def pods_melden() -> dict[str, dict]:
            st = stand()
            pods: dict = {}
            for name, z in sorted(st.items(), key=lambda kv: (kv[1].get("pod", -1), kv[1].get("position", -1))):
                pods.setdefault(z.get("pod"), []).append(f"{name}[{z.get('von')}..{z.get('bis')})")
            melde(f"Epoche {epoche()}: " + "; ".join(f"Pod {p}: {', '.join(v)}" for p, v in sorted(pods.items(), key=str)))
            notiere("pods", epoche=epoche(), pods={str(p): v for p, v in pods.items()})
            return st

        def epochenstand() -> dict:
            m = erzeuger.metriken()
            konten = {}
            for k in miner:
                kt = erzeuger.konto(zahler if k.miner == "m00" else testkonto(k.miner))
                konten[k.miner] = int(kt["guthaben"]) if kt else -1
            return {
                "hoehe": int(m.get("myelith_kette_hoehe", -1)),
                "epoche": int(m.get("myelith_kette_epoche", -1)),
                "einig": int(m.get("myelith_zwillinge_einig_total", 0)),
                "uneinig": int(m.get("myelith_zwillinge_uneinig_total", 0)),
                "credit_preis": int(m.get("myelith_credit_preis", 0)),
                "bearbeitet": int(m.get("myelith_inferenz_bearbeitet_total", 0)),
                "abgelehnt": int(m.get("myelith_inferenz_abgelehnt_total", 0)),
                "auszahlungskonten": konten,
            }

        # ---- 1. Anmeldung, Zusage, Pods ------------------------------------------
        warte(lambda: all(k.zeilen("zusage_eingereicht") for k in miner), 600, "jeder Miner reicht seine Zusage ein", alle, 5)
        melde(f"Zusagen von allen {len(miner)} Minern bei Hoehe {erzeuger.hoehe()}")
        warte(lambda: len(stand()) >= 8, 6 * EPOCHE * slot / 1000 + 300, "acht laufende Shards (zwei Pods)", alle, 10)
        pods_melden()

        # ---- 2. Geld, Credits ------------------------------------------------------
        r = subprocess.run([str(args.probegeld), RELAIS, str(erzeuger.ordner / "relais.ausweis"), zahler,
                            str(args.probegeld_myl * UNITS_PER_MYL)], capture_output=True, text=True, timeout=300)
        if r.returncode != 0:
            raise Befund(f"Probegeld: {r.stderr.strip()[-400:]}")
        melde(r.stdout.strip())

        def nonce() -> int:
            k = erzeuger.konto(zahler)
            return int(k["nonce"]) if k else -1

        def credits() -> int:
            """Die Credits des Zahlers, ueber die Relaisleitung (die Beobachtung
            nennt nur MYL); -1, wenn die Abfrage scheitert."""
            r = subprocess.run([str(args.probegeld), RELAIS, str(erzeuger.ordner / "relais.ausweis"), zahler, "stand"],
                               capture_output=True, text=True, timeout=60)
            m = re.search(r"credits=(\d+)", r.stdout)
            return int(m.group(1)) if m else -1

        n0 = nonce()
        r = myl("wallet", "credits", "zahler", str(args.credits_myl), *relais_angaben)
        if r.returncode != 0:
            raise Befund(f"Credits kaufen: {r.stderr.strip()}")
        warte(lambda: nonce() > n0, 180, "der Burn kommt in die Kette", alle)
        konto = erzeuger.konto(zahler) or {}
        melde(f"Credits gekauft fuer {args.credits_myl} MYL; Konto des Zahlers: {json.dumps(konto, ensure_ascii=False)[:300]}")
        notiere("credits", myl=args.credits_myl, konto=konto)

        # ---- 3. Die Auftraege -------------------------------------------------------
        mappe = auftragsmappe.mappe()
        if args.auftraege:
            gewaehlt = args.auftraege.split(",")
            mappe = [a for a in mappe if a.kennung in gewaehlt]
        if args.kurz:
            mappe = [a for a in mappe if a.kennung == "aufgaben"][:1]
        letzte = epochenstand()
        notiere("epochenstand", **letzte)
        ergebnisse = []
        # 📌 Die Uhrzeit als Zeitpunkt, nicht als Text: „22:47“ liegt als Text
        #    hinter „04:30“, und der erste lange Lauf liess alle Auftraege
        #    liegen. Eine Uhrzeit vor jetzt meint den naechsten Tag.
        schluss_um = None
        if args.bis:
            stunde, minute = (int(x) for x in args.bis.split(":"))
            jetzt = dt.datetime.now()
            schluss_um = jetzt.replace(hour=stunde, minute=minute, second=0, microsecond=0)
            if schluss_um <= jetzt:
                schluss_um += dt.timedelta(days=1)
            melde(f"Kein neuer Auftrag nach {schluss_um:%d.%m. %H:%M}")
        for nummer, a in enumerate(mappe, 1):
            if schluss_um is not None and dt.datetime.now() >= schluss_um and not args.kurz:
                melde(f"Uhrzeit {args.bis} erreicht, {len(mappe) - nummer + 1} Auftraege bleiben liegen")
                notiere("abbruch_uhrzeit", offen=[x.kennung for x in mappe[nummer - 1:]])
                break
            arbeit = ordner / "auftraege" / a.kennung
            arbeit.mkdir(parents=True, exist_ok=True)
            auftragsmappe.anlegen(a, arbeit)
            n0 = nonce()
            r = myl("wallet", "sitzung", "zahler", a.kennung, str(args.budget), str(args.sitzung_epochen), *relais_angaben)
            if r.returncode != 0:
                befunde.append(f"{a.kennung}: Sitzung nicht eroeffnet: {r.stderr.strip()[-300:]}")
                continue
            warte(lambda: nonce() > n0, 180, f"die Sitzung {a.kennung} kommt in die Kette", alle)
            sitzungszeile = r.stdout.strip().splitlines()[0]
            vorher_konto = erzeuger.konto(zahler) or {}
            credits_vorher = credits()
            e_vor, beginn = epoche(), time.monotonic()
            melde(f"Auftrag {nummer}/{len(mappe)} {a.kennung} ({a.titel}) in Epoche {e_vor}: {sitzungszeile}")
            protokoll = ordner / "auftraege" / f"{a.kennung}.txt"
            befehl = [str(args.myl), "sitzung", "--tuer", TUER, "--agent", a.kennung, "--wurzel", str(arbeit), "--schreiben",
                      "--schritte", str(a.schritte), "--token", str(args.token), "--frist", str(args.frist), "--roh"]
            # Der Klient kauft selbst nach, vor jeder Runde und nie ueber das
            # Budget der Sitzung: Credits gelten kurz, und ein Auftrag ueber
            # einen Epochenwechsel stuende sonst ohne Guthaben da (Fund 707).
            if args.vorrat > 0:
                befehl += ["--nachkauf", "zahler", "--vorrat", str(args.vorrat), *relais_angaben]
            try:
                p = subprocess.run(befehl, input=a.text + "\n:ende\n", capture_output=True, text=True, env=umgebung,
                                   cwd=WURZEL, timeout=args.auftragsfrist)
                ausgabe, fehler_aus, rc = p.stdout, p.stderr, p.returncode
            except subprocess.TimeoutExpired as t:
                ausgabe = (t.stdout or b"").decode(errors="replace") if isinstance(t.stdout, bytes) else (t.stdout or "")
                fehler_aus = (t.stderr or b"").decode(errors="replace") if isinstance(t.stderr, bytes) else (t.stderr or "")
                rc = -1
                befunde.append(f"{a.kennung}: nach {args.auftragsfrist} s abgebrochen")
            dauer = time.monotonic() - beginn
            protokoll.write_text(f"$ {' '.join(befehl)}\n\n--- stdout ---\n{ausgabe}\n--- stderr ---\n{fehler_aus}\n", encoding="utf-8")
            tuer = re.findall(r"über die Tür: (\d+) Runden, (\d+) Token hinein, (\d+) heraus", fehler_aus)
            runden, hinein, heraus = (int(x) for x in tuer[-1]) if tuer else (0, 0, 0)
            ende = re.findall(r"\[myl\] Ende: (\w+)", fehler_aus)
            kaeufe = [(int(c), int(e)) for c, e in re.findall(r"\[myl\] Nachkauf: (\d+) Credits .*?Epoche (\d+)", fehler_aus)]
            merkmale = a.pruefen(arbeit)
            erfuellt = sum(merkmale.values())
            nachher_konto = erzeuger.konto(zahler) or {}
            # Die letzte Abrechnung braucht einen Block, bis sie gilt.
            time.sleep(max(5, 3 * args.slot / 1000))
            credits_nachher = credits()
            e_nach = epoche()
            zeile = {
                "auftrag": a.kennung, "titel": a.titel, "rueckgabe": rc, "ende": ende[-1] if ende else None,
                "dauer_s": round(dauer), "epoche_von": e_vor, "epoche_bis": e_nach,
                "runden": runden, "token_hinein": hinein, "token_heraus": heraus,
                "merkmale": merkmale, "erfuellt": erfuellt, "von": len(merkmale),
                "fehler_der_tuer": len(re.findall(r"die Tür antwortete nicht|Abgelehnt|status: (4|5)\d\d", fehler_aus)),
                "konto_vorher": vorher_konto, "konto_nachher": nachher_konto,
                "credits_vorher": credits_vorher, "credits_nachher": credits_nachher,
                "nachkaeufe": len(kaeufe), "nachgekauft": sum(c for c, _ in kaeufe),
                "nachkauf_epochen": sorted({e for _, e in kaeufe}),
                "befehle": len(re.findall(r'"name": "run_command", "arguments"', ausgabe + fehler_aus)),
                "sandbox": next(iter(re.findall(r"Sandbox:\s+(.*)", ausgabe + fehler_aus)), None),
            }
            ergebnisse.append(zeile)
            notiere("auftrag", **zeile)
            melde(f"  {a.kennung}: {erfuellt}/{len(merkmale)} Merkmale, Ende {zeile['ende']}, {runden} Runden, "
                  f"{hinein} Token hinein, {heraus} heraus, {dauer / 60:.1f} min, Epochen {e_vor} bis {e_nach}; "
                  f"Credits {credits_vorher} -> {credits_nachher}, {len(kaeufe)} Nachkaeufe ueber {sum(c for c, _ in kaeufe)} Credits; "
                  f"run_command {zeile['befehle']}mal")
            for k, v in merkmale.items():
                melde(f"    {'✓' if v else '✗'} {k}")
            jetzt = epochenstand()
            notiere("epochenstand", **jetzt)
            if jetzt["epoche"] != letzte["epoche"]:
                pods_melden()
            letzte = jetzt

        # ---- 4. Abrechnung abwarten -------------------------------------------------
        e_end = epoche()
        # Ein Wechsel rechnet die letzte Epoche ab (Zwillinge, Auszahlung).
        nach = 1
        melde(f"Auftraege fertig in Epoche {e_end}; warte {nach} Wechsel ab, damit Zwillinge und Auszahlung abgerechnet sind")
        warte(lambda: epoche() >= e_end + nach, (nach + 1) * EPOCHE * slot / 1000 + 300, f"Epoche {e_end + nach}", alle, 10)
        # ⚑ Mit Luegner: bis zu drei Epochen mehr, bis die Kette ihn sperrt;
        #   er muss erst auf eine Position gelost und in zwei Fenstern gemessen
        #   sein (Fund 655).
        if luegner is not None and not luegner.zeilen("fristsperre"):
            mehr = 3
            melde(f"der Luegner ist noch nicht gesperrt; warte bis zu {mehr} Epochen")
            try:
                warte(lambda: bool(luegner.zeilen("fristsperre")) or epoche() >= e_end + nach + mehr,
                      (mehr + 1) * EPOCHE * slot / 1000 + 300, "die Sperre des Luegners", alle, 15)
            except Befund as b:
                befunde.append(str(b))
        time.sleep(10)
        schluss = epochenstand()
        notiere("epochenstand", **schluss)

        # ---- 5. Einigkeit -----------------------------------------------------------
        # Verglichen werden Pruefpunkte: Die Aufnahmen der Knoten liegen auf
        # verschiedenen Hoehen, ein gerechtfertigter Block auf derselben.
        def je_pruefpunkt(k: Ortsknoten) -> dict[int, str]:
            paare: dict[int, str] = {}
            for z in k.zeilen("aufnahme"):
                if "gerechtfertigt" in z:
                    paare[int(z["gerechtfertigt"])] = str(z.get("gerechtfertigt_block"))
            return paare

        def gemeinsam() -> tuple[int, dict[str, str]]:
            alle_paare = [je_pruefpunkt(k) for k in alle]
            hoehen = set.intersection(*(set(p) for p in alle_paare)) - {0}
            if not hoehen:
                return 0, {}
            h = max(hoehen)
            return h, {k.name: p[h] for k, p in zip(alle, alle_paare)}

        verlangt = max(je_pruefpunkt(erzeuger) or {0: ""})
        try:
            warte(lambda: gemeinsam()[0] >= verlangt, 240, f"ein gemeinsamer Pruefpunkt ab {verlangt}", alle, 5)
        except Befund as b:
            befunde.append(str(b))
        h, wurzeln = gemeinsam()
        verschieden = len(set(wurzeln.values()))
        if h == 0:
            befunde.append("kein Pruefpunkt, den alle Knoten haben")
        elif verschieden != 1:
            befunde.append(f"am Pruefpunkt {h} {verschieden} verschiedene Bloecke: {wurzeln}")
        melde(f"Einigkeit: Pruefpunkt {h}, {verschieden} Block/Bloecke bei {len(wurzeln)} Knoten")
        notiere("einigkeit", pruefpunkt=h, bloecke=verschieden, knoten=len(wurzeln))

        # ---- 6. Befunde aus dem Lauf -------------------------------------------------
        if schluss["einig"] == 0:
            befunde.append("kein Zwillingspaar war einig: Stufe 1 hat nichts verguetet")
        if schluss["uneinig"] > 0:
            befunde.append(f"{schluss['uneinig']} uneinige Zwillingspaare ohne Faelscher")
        if not any(e["erfuellt"] == e["von"] for e in ergebnisse):
            befunde.append("kein Auftrag ganz geloest")
        sperren = [k.miner for k in miner if k is not luegner and k.zeilen("fristsperre")]
        if sperren:
            befunde.append(f"Fristsperren gegen ehrliche Miner: {sperren}")
        if luegner is not None:
            positionen = sorted({int(z.get("epoche", -1)) for z in luegner.zeilen("shard_gestartet")})
            gesperrt = sorted({int(z.get("epoche", -1)) for z in luegner.zeilen("fristsperre")})
            meldungen = sum(len(k.zeilen("fristmeldung")) for k in miner if k is not luegner)
            zeile_l = f"Luegner: auf Position in Epochen {positionen}, gesperrt fuer Epochen {gesperrt}, {meldungen} Meldungen der anderen"
            melde(zeile_l)
            notiere("luegner", positionen=positionen, gesperrt=gesperrt, meldungen=meldungen)
            if positionen and not gesperrt:
                befunde.append(f"der Luegner stand auf Position und wurde nie gesperrt ({zeile_l})")
            if not positionen:
                befunde.append("der Luegner stand nie auf einer Position; nichts belegt")
        if stoerer is not None:
            zaehlung = (ordner / "stoerer" / "stoerer.txt").read_text(errors="replace").splitlines()
            melde(f"Stoerer: {zaehlung[-1] if zaehlung else 'keine Zaehlung'}")
        befehle = sum(e.get("befehle", 0) for e in ergebnisse)
        melde(f"run_command im ganzen Lauf: {befehle}mal; Sandbox: {next((e['sandbox'] for e in ergebnisse if e.get('sandbox')), '?')}")
        notiere("ende", befunde=befunde, ergebnisse=len(ergebnisse))
        bericht_schreiben(ordner, args, ergebnisse, befunde, schluss)
        for b in befunde:
            melde(f"BEFUND: {b}")
        melde("Fertig" + (" mit Befunden" if befunde else ", ohne Befund"))
        return 1 if befunde else 0
    finally:
        if stoerer is not None and stoerer.poll() is None:
            stoerer.kill()
        for k in alle:
            k.beenden()
        if ortsdienst.poll() is None:
            ortsdienst.terminate()
            try:
                ortsdienst.wait(timeout=20)
            except subprocess.TimeoutExpired:
                ortsdienst.kill()
        for pid in shard_pids(ordner):
            try:
                os.kill(pid, signal.SIGTERM)
            except OSError:
                pass
        verlauf.close()


def bericht_schreiben(ordner: pathlib.Path, args, ergebnisse: list[dict], befunde: list[str], schluss: dict) -> None:
    z = [f"# Auftragslauf {ordner.name}", "",
         f"Slot {args.slot} ms, Epoche {EPOCHE * args.slot / 60000:.1f} min, Artefakt {args.artefakte.name}, "
         f"Deckel je Runde {args.token} Token.", "",
         "| Auftrag | Merkmale | Ende | Runden | Token hinein | Token heraus | Dauer | Epochen |",
         "|---|---|---|---|---|---|---|---|"]
    for e in ergebnisse:
        z.append(f"| {e['titel']} | {e['erfuellt']}/{e['von']} | {e['ende']} | {e['runden']} | {e['token_hinein']} | "
                 f"{e['token_heraus']} | {e['dauer_s'] / 60:.1f} min | {e['epoche_von']} bis {e['epoche_bis']} |")
    z += ["", "## Merkmale je Auftrag", ""]
    for e in ergebnisse:
        z.append(f"**{e['titel']}**: " + "; ".join(f"{'✓' if v else '✗'} {k}" for k, v in e["merkmale"].items()))
        z.append("")
    z += ["## Kette am Ende", "", "```", json.dumps(schluss, ensure_ascii=False, indent=1), "```", "", "## Befunde", ""]
    z += [f"- {b}" for b in befunde] or ["- keine"]
    (ordner / "bericht.md").write_text("\n".join(z) + "\n", encoding="utf-8")


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("--programm", type=pathlib.Path, default=BAU / "myl-node")
    p.add_argument("--shard", type=pathlib.Path, default=BAU / "myl-shard")
    p.add_argument("--pod-node", type=pathlib.Path, default=BAU / "myl-pod-node")
    p.add_argument("--myl", type=pathlib.Path, default=BAU / "myl")
    p.add_argument("--probegeld", type=pathlib.Path, default=BAU / "examples" / "probegeld")
    p.add_argument("--artefakte", type=pathlib.Path, default=ARTEFAKT)
    p.add_argument("--rechenwerke", default="metal=aus,cuda=aus,rocm=aus",
                   help="Freigabeplan fuer die Shards (Vorgabe: nur die CPU, weil alle Shards eine Maschine teilen)")
    p.add_argument("--slot", type=int, default=600, help="Laenge eines Slots in ms (Epoche = 1800 Slots)")
    p.add_argument("--kurz", action="store_true", help="ein kurzer Auftrag, ein Wechsel danach")
    p.add_argument("--auftraege", help="nur diese Auftraege der Mappe, mit Komma")
    p.add_argument("--bis", help="keinen Auftrag mehr nach dieser Uhrzeit beginnen (HH:MM)")
    p.add_argument("--token", type=int, default=1536, help="hoechstens so viele Token je Runde des Agenten")
    p.add_argument("--deckel", type=int, default=4096, help="Deckel des Ortsdienstes je Auftrag")
    p.add_argument("--frist", type=int, default=1800, help="Frist des Agenten je Runde in s")
    p.add_argument("--auftragsfrist", type=int, default=7200, help="Frist je Auftrag in s")
    # Seit Fund 711 kostet eine Runde Prompt und Ausgabe: Ein Auftrag
    # brauchte in den Laeufen vom 16. und 17. bis rund 38 000 Credits, eine
    # Runde bis rund 10 000. Das Einzellimit ist ein Zehntel des Budgets.
    p.add_argument("--budget", type=int, default=400_000, help="Credits je Sitzung (Einzellimit ein Zehntel)")
    p.add_argument("--sitzung-epochen", type=int, default=40)
    p.add_argument("--probegeld-myl", type=int, default=12)
    p.add_argument("--credits-myl", type=int, default=10)
    p.add_argument("--vorrat", type=int, default=4,
                   help="der Klient kauft vor jeder Runde auf so viele Runden Vorrat nach; 0 schaltet den Nachkauf ab")
    p.add_argument("--ordner", type=pathlib.Path)
    p.add_argument("--luegner", action="store_true", help="der letzte Miner sagt 1 µs je Ebene zu (sechs Ebenen)")
    p.add_argument("--stoerer", action="store_true", help="ein Stoerer gegen Netzports und Tuer")
    args = p.parse_args()
    for pfad in (args.programm, args.shard, args.pod_node, args.myl, args.probegeld):
        if not pfad.exists():
            print(f"[auftragslauf] fehlt: {pfad}", file=sys.stderr)
            return 2
    ordner = args.ordner or TEMPORAER / "auftragslauf" / f"lauf-{dt.datetime.now():%Y%m%d-%H%M}"
    ordner.mkdir(parents=True, exist_ok=True)
    os.environ["TMPDIR"] = str(TEMPORAER)
    try:
        return lauf(args, ordner)
    except Befund as b:
        melde(f"FEHLER: {b}")
        return 1
    except Exception as e:  # noqa: BLE001
        melde(f"FEHLER: {type(e).__name__}: {e}")
        raise


if __name__ == "__main__":
    sys.exit(main())
