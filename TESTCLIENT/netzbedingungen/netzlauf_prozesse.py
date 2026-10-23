#!/usr/bin/env python3
"""Der Netzlauf mit getrennten Prozessen unter echten Netzbedingungen.

Aufruf (Linux, als root, denn Namensraeume und `tc` brauchen es):

```text
sudo python3 TESTCLIENT/netzbedingungen/netzlauf_prozesse.py            # der volle Lauf, rund drei Minuten
sudo python3 TESTCLIENT/netzbedingungen/netzlauf_prozesse.py --kurz     # derselbe Weg im Kleinen, unter einer Minute
sudo python3 TESTCLIENT/netzbedingungen/netzlauf_prozesse.py --programm <myl-node>
```

# Was hier anders ist als im Netzlauf der Pruefsammlung

Der Netzlauf in `TESTCLIENT/myl-testclient/tests/netzlauf.rs` faehrt alle
Knoten in einem Prozess, ueber die Rueckschleife, im Zeitraffer. Das prueft
die Logik, aber nicht das Netz: keine Laufzeit, kein Verlust, keine
getrennten Prozesse, kein Neustart eines Programms. Hier laeuft jeder Knoten
als eigenes `myl-node` in einem eigenen Netz-Namensraum, verbunden ueber eine
Bruecke, und jeder bekommt mit `tc netem` eine eigene Laufzeit (50 bis 200 ms
in eine Richtung, dazu Schwankung) und einen eigenen Verlust (1 bis 5 %).

# Die Stolpersteine

1. **Einstieg ohne Angabe:** ein Knoten ohne Bootstrap findet das Netz ueber
   die Suche im lokalen Netz (mDNS).
2. **Trennung:** ein Knoten verliert 20 s lang jedes Paket und muss danach
   aufholen.
3. **Absturz:** ein Knoten wird hart beendet und startet aus seiner
   Kettendatei und seinem Schluessel neu, mit derselben Peer-Id.
4. **Neuling:** ein Knoten ohne Kette und ohne Angabe kommt spaet dazu und
   holt jeden Block nach, ueber die gestoerten Leitungen.
5. **Neuling aus dem Abzug:** ein zweiter Neuling beginnt beim Zustandsabzug
   der juengsten Epochengrenze. Den Anker liest er aus dem Protokoll des
   Erzeugers, wie ein Betreiber es taete; seine Hintergrundpruefung muss die
   Basis bestaetigen.

Dazu: Kein Knoten waehlt je die Rueckschleife einer Gegenstelle. Wer auf
allen Schnittstellen horcht, nennt sie mit; bis zum 2026-10-22 landete sie
im Adressbuch der anderen, und eine Neuwahl erreichte den Waehlenden selbst.

Am Ende haelt der Erzeuger an, und alle uebrigen muessen auf derselben Hoehe
zur Ruhe kommen und dort dieselbe Zustandswurzel nennen wie der Erzeuger bei
diesem Block. Dieselbe Hoehe wie der Erzeuger verlangt der Lauf nicht: Die
Bloecke seiner letzten Augenblicke hatte nur er, und mit ihm sind sie fort;
wie viele das waren, meldet der Lauf, und mehr als drei Sekunden duerfen es
nicht sein. Die uebrigen Knoten schicken Transaktionen, und die Wurzel muss
sich auf dem Weg bewegen, sonst sagte die gleiche Wurzel nichts.

⚑ **Und der Anteil ueber Gossip:** Jeder Knoten, der nicht neu startete,
muss mindestens die Haelfte seiner Bloecke ueber Gossip uebernommen haben
statt ueber das Nachholen. Darunter hinkt er der Spitze dauerhaft hinterher
(so geschehen bis zum 2026-10-22: 4 von 2 168).

Gelesen wird von aussen, wie ein Betreiber es taete: Hoehe und Gegenstellen
ueber `/metriken`, die Wurzel aus dem Protokoll des Knotens.

Die letzte Zeile ist `Fertig: bestanden` oder `FEHLER: ...`; der
Rueckgabewert ist 0, 1 (ein Befund) oder 2 (die Umgebung fehlt).
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.request

WURZEL = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(WURZEL / "INTEGER_LLM" / "tests"))
import cargo_paths  # noqa: E402

# Das Netz des Laufs. Ein eigener Bereich, der auf einem Laeufer sonst
# nicht vorkommt; die Bruecke traegt 10.77.0.1, Knoten i sitzt auf 10.77.i.10.
#
# ⚑ **Jeder Knoten in einem eigenen /24**, alle an derselben Bruecke (ein
# /16). Ein Knoten nimmt aus einem /24 hoechstens vier eingehende
# Verbindungen an (Adressvielfalt gegen Sybil-Fluten); mit allen im selben
# /24 wies der fuenfte Knoten den sechsten ab, und der Lauf bildete ein
# Rechenzentrum nach statt des Netzes, um das es geht.
BRUECKE = "mylbr0"
NETZ = "10.77"
PORT = 4150
BEOBACHTUNG = 4151


class Befund(Exception):
    """Ein Lauf, der nicht traegt."""


def sh(*befehl: str, pruefen: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(befehl, check=pruefen, capture_output=True, text=True)


def melde(text: str) -> None:
    print(f"[netzlauf] {text}", flush=True)


class Knoten:
    """Ein `myl-node` in seinem Namensraum."""

    def __init__(self, nummer: int, ordner: pathlib.Path, latenz_ms: int, verlust: float):
        self.nummer = nummer
        self.name = f"k{nummer}"
        self.ns = f"myl-{self.name}"
        self.ip = f"{NETZ}.{nummer}.10"
        self.ordner = ordner / self.name
        self.latenz_ms = latenz_ms
        self.verlust = verlust
        self.prozess: subprocess.Popen | None = None
        self.peer_id: str | None = None

    # ---- Netz ---------------------------------------------------------
    def netz_anlegen(self) -> None:
        aussen = f"v{self.nummer}p"
        sh("ip", "netns", "add", self.ns)
        sh("ip", "link", "add", aussen, "type", "veth", "peer", "name", "eth0", "netns", self.ns)
        sh("ip", "link", "set", aussen, "master", BRUECKE, "up")
        sh("ip", "netns", "exec", self.ns, "ip", "addr", "add", f"{self.ip}/16", "dev", "eth0")
        sh("ip", "netns", "exec", self.ns, "ip", "link", "set", "eth0", "up")
        sh("ip", "netns", "exec", self.ns, "ip", "link", "set", "lo", "up")
        self.stoerung(self.verlust)

    def stoerung(self, verlust: float) -> None:
        """Laufzeit und Verlust auf beiden Seiten der Leitung: Eine Trennung
        mit 100 % Verlust nur in eine Richtung liesse Pakete hinein."""
        schwankung = max(1, self.latenz_ms // 10)
        regel = ["netem", "delay", f"{self.latenz_ms}ms", f"{schwankung}ms", "loss", f"{verlust}%"]
        for ns, geraet in ((self.ns, "eth0"), (None, f"v{self.nummer}p")):
            vorne = ["ip", "netns", "exec", ns] if ns else []
            sh(*vorne, "tc", "qdisc", "replace", "dev", geraet, "root", *regel)

    def netz_entfernen(self) -> None:
        sh("ip", "netns", "del", self.ns, pruefen=False)

    # ---- Prozess ------------------------------------------------------
    def starten(self, programm: pathlib.Path, bootstrap: str | None, erzeuger: bool, blocktakt: int,
                zusatz: list[str] | None = None) -> None:
        self.ordner.mkdir(parents=True, exist_ok=True)
        befehl = [
            "ip", "netns", "exec", self.ns, str(programm),
            "--name", self.name,
            "--port", str(PORT),
            "--schluessel", str(self.ordner / "knoten.key"),
            "--kette", str(self.ordner / "kette.dat"),
            "--protokolle", str(self.ordner),
            "--beobachtung", f"{self.ip}:{BEOBACHTUNG}",
            "--ohne-tuer",
            "--aufnahme", "2",
            "--testverkehr", "1",
            "--lokale-suche",
        ]
        if bootstrap:
            befehl += ["--bootstrap", bootstrap]
        if erzeuger:
            befehl += ["--erzeuger", "--blocktakt", str(blocktakt)]
        befehl += zusatz or []
        fehler = open(self.ordner / "stderr.txt", "a", encoding="utf-8")
        # Aus der Wurzel, damit der Knoten die Konformitaetsvektoren findet.
        self.prozess = subprocess.Popen(befehl, cwd=WURZEL, stdout=fehler, stderr=subprocess.STDOUT)

    def warte_auf_peer_id(self, frist: float) -> str:
        ende = time.monotonic() + frist
        while time.monotonic() < ende:
            self.lebt_noch()
            text = (self.ordner / "stderr.txt").read_text(encoding="utf-8", errors="replace")
            for zeile in text.splitlines():
                if "Peer-Id " in zeile:
                    self.peer_id = zeile.split("Peer-Id ", 1)[1].strip()
                    return self.peer_id
            time.sleep(0.2)
        raise Befund(f"{self.name} nannte keine Peer-Id")

    def lebt_noch(self) -> None:
        if self.prozess is not None and self.prozess.poll() is not None:
            raise Befund(f"{self.name} endete mit {self.prozess.returncode}, siehe {self.ordner / 'stderr.txt'}")

    def beenden(self, hart: bool = False, frist: float = 15) -> None:
        if self.prozess is None or self.prozess.poll() is not None:
            return
        # Das Signal geht an `ip netns exec`, und das ersetzt sich durch das
        # Programm: dieselbe Prozessnummer.
        self.prozess.send_signal(signal.SIGKILL if hart else signal.SIGTERM)
        try:
            self.prozess.wait(timeout=frist)
        except subprocess.TimeoutExpired:
            self.prozess.kill()
            self.prozess.wait()
            raise Befund(f"{self.name} beendete sich nicht in {frist} s")

    # ---- Ablesen ------------------------------------------------------
    def metriken(self) -> dict[str, float]:
        # 📌 Bis zu drei Versuche: Die Abfrage laeuft ueber dieselbe gestoerte
        #    Leitung wie das Netz, und eine einzelne lief bei 5 % Verlust in
        #    die Zeitgrenze. Ihr `-1` wurde als Hoehe gelesen, und das Skript
        #    meldete 721 Bloecke Rueckstand, wo es drei waren (2026-10-22).
        text = None
        for _ in range(3):
            try:
                with urllib.request.urlopen(f"http://{self.ip}:{BEOBACHTUNG}/metriken", timeout=2) as antwort:
                    text = antwort.read().decode()
                break
            except OSError:
                continue
        if text is None:
            return {}
        werte = {}
        for zeile in text.splitlines():
            if zeile.startswith("#") or " " not in zeile:
                continue
            name, wert = zeile.rsplit(" ", 1)
            try:
                werte[name] = float(wert)
            except ValueError:
                pass
        return werte

    def hoehe(self) -> int:
        return int(self.metriken().get("myelith_kette_hoehe", -1))

    def peers(self) -> int:
        return int(self.metriken().get("myelith_peers", -1))

    def protokoll(self) -> list[dict]:
        zeilen = []
        for datei in sorted(self.ordner.glob("*.jsonl")):
            for z in datei.read_text(encoding="utf-8", errors="replace").splitlines():
                try:
                    zeilen.append(json.loads(z))
                except json.JSONDecodeError:
                    pass
        return zeilen


# Liest das Kernelmodul br_netfilter mit, laufen die Rahmen der Bruecke durch
# die FORWARD-Kette von iptables, und auf einem Rechner mit Docker steht die
# auf DROP. Fuer die Dauer des Laufs aus, danach wie vorher.
NETFILTER = pathlib.Path("/proc/sys/net/bridge/bridge-nf-call-iptables")
_netfilter_vorher: str | None = None


def bruecke_anlegen() -> None:
    global _netfilter_vorher
    if NETFILTER.exists():
        _netfilter_vorher = NETFILTER.read_text().strip()
        NETFILTER.write_text("0")
    sh("ip", "link", "add", BRUECKE, "type", "bridge")
    # Ohne Querier haelt die Bruecke Multicast zurueck, sobald sie ihn
    # mitliest; mDNS braucht ihn ungefiltert.
    sh("ip", "link", "set", BRUECKE, "type", "bridge", "mcast_snooping", "0")
    sh("ip", "addr", "add", f"{NETZ}.0.1/16", "dev", BRUECKE)
    sh("ip", "link", "set", BRUECKE, "up")


def bruecke_entfernen() -> None:
    sh("ip", "link", "del", BRUECKE, pruefen=False)
    if _netfilter_vorher is not None:
        NETFILTER.write_text(_netfilter_vorher)


def warte(bedingung, frist: float, was: str, knoten: list[Knoten]) -> float:
    """Wartet auf einen Zustand, nicht auf die Uhr; gibt die Dauer zurueck."""
    beginn = time.monotonic()
    while time.monotonic() - beginn < frist:
        for k in knoten:
            k.lebt_noch()
        if bedingung():
            return time.monotonic() - beginn
        time.sleep(0.5)
    stand = ", ".join(f"{k.name} Hoehe {k.hoehe()} Peers {k.peers()}" for k in knoten)
    raise Befund(f"{was}: nicht erreicht in {frist:.0f} s ({stand})")


def umgebung_pruefen(programm: pathlib.Path) -> str | None:
    if sys.platform != "linux":
        return "nur unter Linux (Netz-Namensraeume)"
    if os.geteuid() != 0:
        return "braucht root (ip netns, tc)"
    for werkzeug in ("ip", "tc"):
        if shutil.which(werkzeug) is None:
            return f"{werkzeug} fehlt (Paket iproute2)"
    if not programm.is_file():
        return f"{programm} fehlt: cd NODE/myl-node && cargo build --release"
    probe = sh("ip", "link", "add", "mylprobe0", "type", "dummy", pruefen=False)
    if probe.returncode != 0:
        return f"ip link: {probe.stderr.strip()}"
    netem = sh("tc", "qdisc", "add", "dev", "mylprobe0", "root", "netem", "delay", "1ms", pruefen=False)
    sh("ip", "link", "del", "mylprobe0", pruefen=False)
    if netem.returncode != 0:
        return f"tc netem: {netem.stderr.strip()} (Kernelmodul sch_netem)"
    return None


def lauf(args: argparse.Namespace, ordner: pathlib.Path) -> None:
    programm = args.programm
    # Takt und Zeiten. Im kurzen Lauf derselbe Weg mit der kleinsten Menge:
    # weniger Knoten, kuerzere Abschnitte, aber jeder Stolperstein einmal.
    n = 4 if args.kurz else 6
    epoche = 1800  # Bloecke je Epoche, wie im Protokoll
    betrieb = 5 if args.kurz else 30
    trennung = 5 if args.kurz else 20
    ausfall = 5 if args.kurz else 15
    vorsprung = 40 if args.kurz else 800  # Bloecke, bevor der Neuling kommt
    # Im kurzen Lauf schneller, damit auch er die erste Epochengrenze und
    # damit den Abzug in unter einer Minute erreicht. Nicht schneller als 25 ms:
    # Ueber die schlechteste Leitung (200 ms, 5 % Verlust) holt ein Neuling
    # aus einer Quelle rund 75 Bloecke je Sekunde nach, bei 10 ms waechst
    # das Netz schneller (2026-10-22).
    takt = args.blocktakt if args.blocktakt else (25 if args.kurz else 50)
    erlaubter_rueckstand = max(40, 3000 // takt)  # drei Sekunden Bloecke

    def stoerung(i: int, anzahl: int) -> tuple[int, float]:
        anteil = i / max(1, anzahl - 1)
        return int(args.latenz_min + anteil * (args.latenz_max - args.latenz_min)), round(
            args.verlust_min + anteil * (args.verlust_max - args.verlust_min), 2
        )

    alle = [Knoten(i, ordner, *stoerung(i, n + 2)) for i in range(n + 2)]
    netz = alle[:n]
    neuling = alle[n]
    aus_abzug = alle[n + 1]
    erzeuger, ohne_angabe, getrennt, abstuerzer = netz[0], netz[-1], netz[1], netz[2]

    bruecke_anlegen()
    for k in alle:
        k.netz_anlegen()
    for k in alle:
        melde(f"{k.name} {k.ip}: {k.latenz_ms} ms je Richtung, {k.verlust} % Verlust")

    erzeuger.starten(programm, None, True, takt)
    pid = erzeuger.warte_auf_peer_id(30)
    einstieg = f"/ip4/{erzeuger.ip}/tcp/{PORT}/p2p/{pid}"
    for k in netz[1:]:
        # Stolperstein 1: der letzte ohne Angabe, nur ueber mDNS.
        k.starten(programm, None if k is ohne_angabe else einstieg, False, takt)
    for k in netz:
        k.warte_auf_peer_id(30)

    # ---- Jeder kennt jeden ---------------------------------------------
    dauer = warte(lambda: all(k.peers() >= n - 1 for k in netz), 90, "jeder kennt jeden", netz)
    melde(f"jeder kennt jeden nach {dauer:.1f} s, {ohne_angabe.name} ohne Angabe (mDNS)")

    # ---- Betrieb: alle folgen dem Erzeuger -----------------------------
    warte(lambda: erzeuger.hoehe() > 20, 60, "der Erzeuger baut", netz)
    # 📌 Erst den ersten Abgleich abwarten: Bei schnellem Takt hatte ein
    #    Knoten, der nur ueber mDNS kam, nach 0,2 s noch nichts, und das
    #    zaehlte als Rueckstand im Betrieb (2026-10-23).
    warte(lambda: all(k.hoehe() >= max(0, erzeuger.hoehe() - erlaubter_rueckstand) for k in netz[1:]), 60,
          "der erste Abgleich", netz)
    ende = time.monotonic() + betrieb
    groesster = 0
    while time.monotonic() < ende:
        h = erzeuger.hoehe()
        hoehen = [k.hoehe() for k in netz[1:]]
        # Eine unlesbare Hoehe ist -1 und keine Zahl ueber den Abstand.
        if h >= 0 and all(x >= 0 for x in hoehen):
            groesster = max(groesster, max(h - x for x in hoehen))
        time.sleep(1)
    melde(f"Betrieb {betrieb} s: Hoehe {erzeuger.hoehe()}, groesster Rueckstand {groesster} Bloecke")
    if groesster > erlaubter_rueckstand:
        raise Befund(f"Rueckstand {groesster} Bloecke im Betrieb, erlaubt {erlaubter_rueckstand}")

    def holt_auf(k: Knoten) -> bool:
        h, e = k.hoehe(), erzeuger.hoehe()
        return h >= 0 and e >= 0 and h >= e - erlaubter_rueckstand

    # ---- Stolperstein 2: Trennung --------------------------------------
    # Abgelesen wird vor und nach der Trennung: Waehrenddessen ist auch die
    # Metrik des Getrennten nicht erreichbar.
    vorher = getrennt.hoehe()
    getrennt.stoerung(100)
    time.sleep(trennung)
    getrennt.stoerung(getrennt.verlust)
    dauer = warte(lambda: holt_auf(getrennt), 120, f"{getrennt.name} holt nach der Trennung auf", netz)
    melde(f"Trennung {trennung} s: {getrennt.name} von {vorher} auf {getrennt.hoehe()} in {dauer:.1f} s")

    # ---- Stolperstein 3: Absturz und Neustart --------------------------
    alte_id = abstuerzer.peer_id
    abstuerzer.beenden(hart=True)
    time.sleep(ausfall)
    abstuerzer.starten(programm, einstieg, False, takt)
    if abstuerzer.warte_auf_peer_id(30) != alte_id:
        raise Befund(f"{abstuerzer.name} kam mit einer anderen Peer-Id zurueck")
    dauer = warte(lambda: holt_auf(abstuerzer), 120, f"{abstuerzer.name} holt nach dem Absturz auf", netz)
    melde(f"Absturz: {abstuerzer.name} nach {ausfall} s neu gestartet, aufgeholt in {dauer:.1f} s")

    # ---- Stolperstein 4: ein Neuling ohne Kette und ohne Angabe --------
    warte(lambda: erzeuger.hoehe() >= vorsprung, 600, f"Hoehe {vorsprung}", netz)
    neuling.starten(programm, None, False, takt)
    neuling.warte_auf_peer_id(30)
    mit_neuling = netz + [neuling]
    dauer = warte(lambda: holt_auf(neuling), 300, "der Neuling holt auf", mit_neuling)
    melde(f"Neuling: {neuling.hoehe()} Bloecke nachgeholt in {dauer:.1f} s, {neuling.peers()} Gegenstellen")

    # ---- Stolperstein 5: ein Neuling aus dem Zustandsabzug ---------------
    # 📌 Der juengste Anker, nicht der erste: Gewoehnliche Grenzen behaelt
    #    jeder Knoten nur zwei Epochen lang, und bei schnellem Takt war die
    #    erste laengst geraeumt, als der Neuling kam (2026-10-22).
    warte(lambda: erzeuger.hoehe() >= epoche + 50, 900, f"Hoehe {epoche + 50}", mit_neuling)
    anker = None
    anker_hoehe = 0
    def anker_da() -> bool:
        nonlocal anker, anker_hoehe
        zeilen = [z for z in erzeuger.protokoll() if z.get("art") == "abzug_angelegt"]
        if zeilen:
            anker, anker_hoehe = zeilen[-1]["anker"], zeilen[-1]["hoehe"]
        return anker is not None
    warte(anker_da, 30, "der Erzeuger meldet seinen Abzug", mit_neuling)
    aus_abzug.starten(programm, None, False, takt, ["--abzug", anker])
    aus_abzug.warte_auf_peer_id(30)
    mit_neuling = mit_neuling + [aus_abzug]
    def bestaetigt() -> bool:
        return any(z.get("art") == "abzug_bestaetigt" for z in aus_abzug.protokoll())
    dauer = warte(lambda: holt_auf(aus_abzug) and bestaetigt(), 300, "der Neuling aus dem Abzug holt auf und bestaetigt", mit_neuling)
    uebernommen = [z for z in aus_abzug.protokoll() if z.get("art") == "abzug_uebernommen"]
    if not uebernommen or uebernommen[0].get("hoehe") != anker_hoehe:
        raise Befund(f"{aus_abzug.name} begann nicht beim Abzug {anker_hoehe}: {uebernommen}")
    melde(f"Neuling aus dem Abzug bei {anker_hoehe}: {uebernommen[0].get('bytes')} Bytes Zustand, an der Spitze und bestaetigt in {dauer:.1f} s")

    # ---- Ende: der Erzeuger haelt an, alle kommen auf einer Hoehe an ---
    erzeuger.beenden()
    erzeugt = {z["hoehe"]: z["zustandswurzel"] for z in erzeuger.protokoll() if z.get("art") == "block_erzeugt"}
    schluss = max(erzeugt)
    uebrige = mit_neuling[1:]

    def ruhig_und_einig() -> bool:
        hoehen = [k.hoehe() for k in uebrige]
        return len(set(hoehen)) == 1 and hoehen[0] > 0

    # 📌 Einig **und still**: Nach dem ersten Gleichstand kam bei 5 % Verlust
    #    noch ein spaeter letzter Block an (2026-10-22). Fertig ist es erst,
    #    wenn dieselbe Hoehe fuenf Sekunden lang bei allen steht.
    ende = time.monotonic() + 120
    while True:
        warte(ruhig_und_einig, max(1, ende - time.monotonic()), "alle uebrigen auf einer Hoehe", uebrige)
        hoehe = uebrige[0].hoehe()
        time.sleep(5)
        if ruhig_und_einig() and uebrige[0].hoehe() == hoehe:
            break
        if time.monotonic() > ende:
            raise Befund("die Hoehen kamen nicht zur Ruhe")
    for k in uebrige:
        k.beenden()
    verloren = schluss - hoehe
    if verloren > erlaubter_rueckstand:
        raise Befund(f"{verloren} Bloecke des Erzeugers erreichten niemanden, erlaubt {erlaubter_rueckstand}")

    wurzeln = {"k0 (erzeugt)": erzeugt[hoehe]}
    for k in uebrige:
        aufnahmen = [z for z in k.protokoll() if z.get("art") == "aufnahme" and z.get("hoehe") == hoehe]
        if not aufnahmen:
            raise Befund(f"{k.name}: keine Aufnahme bei Hoehe {hoehe}")
        wurzeln[k.name] = aufnahmen[-1]["zustandswurzel"]
    for k in mit_neuling:
        if not any(z.get("art") == "ende" for z in k.protokoll()):
            raise Befund(f"{k.name}: kein Abschluss im Protokoll")
    if len(set(wurzeln.values())) != 1:
        raise Befund(f"die Ketten laufen auseinander: {wurzeln}")
    # 📌 Nicht „Transaktionen im Block“, sondern „die Wurzel bewegt sich“:
    #    Bis zum 2026-10-22 trugen Bloecke Transaktionen des Testverkehrs,
    #    die Kette verwarf jede (die Nonce begann bei 1), und die Wurzel blieb
    #    ueber 1 342 Bloecke dieselbe. Eine Zaehlung der Transaktionen sah
    #    davon nichts.
    bewegt = len({w for h, w in erzeugt.items() if h <= hoehe})
    if bewegt < 10:
        raise Befund(f"die Wurzel nahm nur {bewegt} Werte an; der Zustand bewegte sich nicht, die gleiche Wurzel sagte nichts")
    melde(
        f"Ende: {len(wurzeln)} Knoten bei Hoehe {hoehe}, eine Wurzel {erzeugt[hoehe]}, {bewegt} verschiedene Wurzeln "
        f"auf dem Weg; {verloren} Bloecke gingen mit dem Erzeuger"
    )

    # ---- Niemand waehlt die Rueckschleife eines anderen (Fund 594) ------
    for k in mit_neuling:
        selbst = [z for z in k.protokoll() if z.get("art") == "abgewiesen" and "127.0.0.1" in z.get("grund", "")]
        if selbst:
            raise Befund(f"{k.name} waehlte {len(selbst)} Mal die Rueckschleife einer Gegenstelle: {selbst[0]['grund'][:120]}")

    # ---- Der Anteil ueber Gossip ----------------------------------------
    for k in uebrige:
        p = k.protokoll()
        gossip = sum(1 for z in p if z.get("art") == "block_uebernommen")
        voraus = sum(z.get("aus_dem_voraus", 0) for z in p if z.get("art") == "nachschub_erhalten")
        nachgeholt = sum(z.get("angenommen", 0) for z in p if z.get("art") == "nachschub_erhalten")
        melde(f"{k.name}: {gossip} ueber Gossip, {voraus} aus dem Vorausspeicher, {nachgeholt} nachgeholt")
        if k in (abstuerzer, neuling, aus_abzug):
            continue
        if gossip + voraus < (gossip + voraus + nachgeholt) // 2:
            raise Befund(f"{k.name} hinkt dauerhaft hinterher: nur {gossip + voraus} von {gossip + voraus + nachgeholt} Bloecken ohne Nachholen")


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    p.add_argument("--programm", type=pathlib.Path, default=cargo_paths.binary("../NODE/myl-node", "myl-node"))
    p.add_argument("--kurz", action="store_true", help="derselbe Weg im Kleinen")
    p.add_argument("--blocktakt", type=int, default=0, help="Millisekunden je Block (Vorgabe: 50, kurz 25)")
    p.add_argument("--latenz-min", type=int, default=50)
    p.add_argument("--latenz-max", type=int, default=200)
    p.add_argument("--verlust-min", type=float, default=1.0)
    p.add_argument("--verlust-max", type=float, default=5.0)
    p.add_argument("--protokolle", type=pathlib.Path, help="Ordner fuer die Protokolle (Vorgabe: ein neuer im Temp)")
    args = p.parse_args()
    args.programm = args.programm.resolve()

    fehlt = umgebung_pruefen(args.programm)
    if fehlt:
        print(f"FEHLER: Umgebung: {fehlt}", flush=True)
        return 2
    ordner = args.protokolle or pathlib.Path(tempfile.mkdtemp(prefix="myl-netzlauf-prozesse-"))
    ordner.mkdir(parents=True, exist_ok=True)
    melde(f"Protokolle unter {ordner}")
    knoten_namen = [f"myl-k{i}" for i in range(8)]
    rc = 0
    try:
        lauf(args, ordner)
        print("Fertig: bestanden", flush=True)
    except Befund as b:
        print(f"FEHLER: {b}", flush=True)
        rc = 1
    finally:
        # Alles weg, auch nach einem Abbruch: keine Prozesse, keine Namensraeume.
        for ns in knoten_namen:
            pids = sh("ip", "netns", "pids", ns, pruefen=False).stdout.split()
            for pid in pids:
                try:
                    os.kill(int(pid), signal.SIGKILL)
                except (OSError, ValueError):
                    pass
            sh("ip", "netns", "del", ns, pruefen=False)
        bruecke_entfernen()
    if rc:
        for d in sorted(ordner.iterdir()):
            fehler = d / "stderr.txt"
            if fehler.exists():
                print(f"---- {d.name}: letzte Zeilen", flush=True)
                print("\n".join(fehler.read_text(errors="replace").splitlines()[-8:]), flush=True)
    return rc


if __name__ == "__main__":
    sys.exit(main())
