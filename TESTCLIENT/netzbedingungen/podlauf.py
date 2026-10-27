"""Der Podlauf: Netz, Rechnen und Pruefen zusammen unter echten
Netzbedingungen, jeder Knoten und jeder Shard ein eigener Prozess.

Aufruf (Linux, als root, wie der Netzlauf daneben):

```text
sudo python3 TESTCLIENT/netzbedingungen/podlauf.py --programm <myl-node> --shard <myl-shard> \
    --artefakte <artefakt> --kurz            # die Mechanik in rund zehn Minuten
sudo python3 TESTCLIENT/netzbedingungen/podlauf.py ... --epochen 200   # Dauer mit Stoerungen
```

⚠️ `--shard` braucht einen `myl-shard` mit Pruefnetz (`cargo build --release
--bin myl-shard --features integer-llm-runtime/pruefnetz` in
`COMPUTE_PIPELINE/myl-pod`): Das Probeartefakt steht nur dort auf der
Netzliste, und ein ausgeliefertes Programm verweigert es.

Was der Lauf aufbaut: ein Erzeuger und zehn Miner, jeder mit Laufzeit und
Verlust auf seiner Leitung (`tc netem`). Jeder Miner meldet sich ueber die
Kette an, sagt Ebenen und Zeit je Ebene zu und startet den Shard, den die
Kette ihm zuteilt, mit dem echten Probeartefakt. Ein Miner luegt: Er sagt
eine Mikrosekunde je Ebene zu.

Was der Lauf verlangt:

1. **Anmeldung und Zusage ueber die Kette**, von jedem Miner.
2. **Pods aus der Kette**: Jede Position fuehrt ihren Shard, mit den Grenzen,
   die die Kette schneidet.
3. **Rechnen ueber das Netz**: Die Mitglieder messen einander mit Zeitproben,
   echten Vorwaertsrechnungen ueber die Shards der anderen, ueber die
   gestoerten Leitungen.
4. **Pruefen und Folgen**: Der Luegner wird gemeldet, von einer Mehrheit
   gegengezeichnet, und die Kette sperrt ihn fuer die naechste Epoche; danach
   fuehrt er keinen Shard. Kein ehrlicher Miner wird gesperrt.
5. **Stoerungen**: Ein abgestuerzter Shard wird neu gestartet; ein
   abgestuerzter Miner holt auf und fuehrt wieder seinen Shard.
6. **Einigkeit**: am Ende eine Hoehe und eine Wurzel bei allen.

Mit `--tor` dazu der Weg eines Empfaengers (2026-10-26): Der Erzeuger ist
zugleich Torknoten mit Tuer, Relaisleitung und einem Ortsdienst
(`myl-pod-node --podrelais`), der ueber den Pod der Kette rechnet. Ein
Testwallet (`myl`) ist Auszahlungskonto eines Miners, verdient so seine
Test-MYL, kauft Credits, eroeffnet eine Sitzung mit Vollmacht fuer einen
Agenten, und der fragt ueber die Tuer. Verlangt: Antworten, ein
eingereichtes Buendel, verbreitete Abrechnungen und eine echte Auszahlung
an Miner (ein Betrag, der kein Vielfaches der Probepraegung ist). Dazu
`--pod-node <myl-pod-node> --myl <myl>` und ein langsamerer Takt, etwa
`--blocktakt 60` (eine Epoche 108 s): Ueber Knoten mit Latenz kostet ein
Token Sekunden, und eine Antwort muss in eine Epoche passen.

Mit `--real` naeher an echten Bedingungen (2026-10-27): Knoten in drei
Zonen (Europa, Nordamerika, Asien) mit Rundlauf je Paar, Zugangsleitung,
Bandbreite nach oben und Verlust in Schueben; vier Validatoren (Erzeuger und
je ein Miner je Zone), deren Pruefpunkte ueber das Netz gestimmt werden;
Trennungen einer Zone und des Atlantiks mit Messung von Aufholen und
Pruefpunkten; ein spaeter Einsteiger in Asien; mit `--tor` dazu eine
Ueberweisung aus der Wallet an ein neues Konto (Betrag, Gebuehr, Aufschlag)
und eine kurze Sitzung, deren Hinterlegung nach der Nachfrist zurueckkommen
muss; mit `--lastgeber <lastgeber>` Last in Stufen, gemessen Aufnahme und
Grundpreis. Befunde werden gesammelt und am Ende gemeldet, statt den Lauf
beim ersten abzubrechen.

Noch nicht dabei: die Nachrechnung der Stichprobe.

Die letzte Zeile ist `Fertig: bestanden` oder `FEHLER: ...`; Rueckgabe 0, 1
(ein Befund) oder 2 (die Umgebung fehlt).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import random
import signal
import subprocess
import sys
import tempfile
import time
import urllib.request

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import netzlauf_prozesse as nl  # noqa: E402
from netzlauf_prozesse import Befund, Knoten, Stoerer, melde, warte  # noqa: E402

EPOCHE = 1800

# Der Torknoten (`--tor`): Tuer, Ortsleitung und Relaisleitung im
# Namensraum des Erzeugers, nur auf seiner Rueckschleife.
TUER = "127.0.0.1:4160"
ORT = "127.0.0.1:4170"
RELAIS = "127.0.0.1:4190"
# Die Kennung des Sitzungskanals zwischen Tuer und Ortsdienst; beide Seiten
# nennen dieselbe.
POD_HEX = "71" * 32
# Der Abdruck des Probemodells, als Pipeline-Stand des Ortsdienstes.
PIPELINE_HEX = "9240fa59ddb460eb1b2efc77879cc75e68882a5c430b1cdc0222cd02d4ba51bd"
# Die Woerter des Zahlers: ein Pruefvektor aus BIP-39, im Test bekannt.
ZAHLER_WOERTER = "legal winner thank year wave sausage worth useful legal winner thank yellow"
SCHLUESSELKOPF = "# Myelith-Konsensschluessel (BLS12-381). GEHEIM. Nicht weitergeben."
PROBEPRAEGUNG = 10_000_000
# Die vorlaeufigen Gebuehren der Probekette (`netz::GEBUEHREN_VORLAEUFIG`)
# und die Nachfrist einer Sitzung (`myl_ledger::state::SITZUNG_NACHFRIST`).
AUFSCHLAG = 10_000
HINTERLEGUNG_SITZUNG = 100_000
SITZUNG_NACHFRIST = 100


def testkonto(name: str) -> str:
    """Ein festes Auszahlungskonto ohne Schluessel fuer einen Miner."""
    return hashlib.sha256(f"myelith-testlauf-konto-{name}".encode()).hexdigest()


def konto_abfragen(k: Knoten, adresse: str) -> dict | None:
    """`/konto/<adresse>` am Beobachtungsendpunkt eines Knotens."""
    try:
        with urllib.request.urlopen(f"http://{k.ip}:{nl.BEOBACHTUNG}/konto/{adresse}", timeout=5) as r:
            return json.loads(r.read())
    except (OSError, ValueError):
        return None

# Name, Ebenen samt Kopf, Mikrosekunden je Ebene; wie im Netzlauf im selben
# Prozess, damit beide dieselben Pods bilden koennen.
MINER = [
    ("m00", 48, 20_000),
    ("m01", 40, 40_000),
    ("m02", 48, 60_000),
    ("luegner", 48, 1),
    ("m04", 30, 30_000),
    ("m05", 48, 90_000),
    ("m06", 44, 25_000),
    ("m07", 48, 50_000),
    ("m08", 36, 35_000),
    ("m09", 48, 45_000),
]


# ---- Naeher an echten Bedingungen (`--real`, 2026-10-27) -----------------
#
# ⚑ **Laufzeit je Paar statt je Knoten.** Bis hierher trug jeder Knoten eine
#   Laufzeit fuer allen Verkehr, und zwei Knoten sahen einander mit 200 bis
#   800 ms Rundlauf. Jetzt sitzt jeder in einer Zone; zwischen zwei Knoten
#   liegt der Rundlauf der Zonen plus beide Zugangsleitungen. Die Zahlen
#   sind Groessenordnungen fuer Rechenzentren und Hausanschluesse, nicht
#   gemessen.
ZONEN = ["Europe", "NorthAmerica", "Asia"]
RUNDLAUF_MS = {
    ("Europe", "Europe"): 20,
    ("NorthAmerica", "NorthAmerica"): 30,
    ("Asia", "Asia"): 40,
    ("Europe", "NorthAmerica"): 90,
    ("NorthAmerica", "Asia"): 150,
    ("Europe", "Asia"): 200,
}
# Wo jeder sitzt; der Erzeuger (Torknoten) in Europa, im Rechenzentrum.
MINERZONE = {
    "m00": "Europe", "m01": "Europe", "m02": "Europe", "luegner": "Europe", "m04": "Europe",
    "m05": "NorthAmerica", "m06": "NorthAmerica", "m07": "NorthAmerica",
    "m08": "Asia", "m09": "Asia", "m10": "Asia",
}
# ⚑ **Vier Validatoren statt eines**: der Erzeuger und je ein Miner aus jeder
#   Zone. Keiner haelt ein Drittel; ein Pruefpunkt braucht drei Stimmen.
VALIDATOREN = ["m01", "m06", "m09"]
STAKE = 1000
# Der spaete Einsteiger: ein neuer Miner in Asien, mitten im Lauf.
EINSTEIGER = ("m10", 48, 40_000)


def rundlauf(a: str, b: str) -> int:
    return RUNDLAUF_MS.get((a, b)) or RUNDLAUF_MS[(b, a)]


class Ortsknoten(Knoten):
    """Ein Knoten mit Zone, Zugangsleitung und Bandbreite.

    Ausgehend (`eth0` im Namensraum): ein Token-Bucket fuer die Bandbreite,
    darunter je Zielzone ein `netem` mit halbem Zonenrundlauf plus Zugang,
    Verlust in Schueben (Korrelation 25 %). Eingehend (`v<n>p`): nur der
    Zugang. Rundlauf zweier Knoten: Zonenrundlauf plus zweimal beide Zugaenge.
    """

    def __init__(self, nummer: int, ordner: pathlib.Path, zone: str, zugang_ms: int, verlust: float, mbit: int):
        super().__init__(nummer, ordner, zugang_ms, verlust)
        self.zone = zone
        self.zugang_ms = zugang_ms
        self.mbit = mbit
        self.getrennt: set[str] = set()

    def regel(self, zone: str) -> list[str]:
        if zone in self.getrennt:
            return ["netem", "loss", "100%"]
        einweg = rundlauf(self.zone, zone) // 2 + self.zugang_ms
        return ["netem", "delay", f"{einweg}ms", f"{max(1, einweg // 20)}ms", "distribution", "normal",
                "loss", f"{self.verlust}%", "25%"]

    def netz_anlegen(self) -> None:
        aussen = f"v{self.nummer}p"
        nl.sh("ip", "netns", "add", self.ns)
        nl.sh("ip", "link", "add", aussen, "type", "veth", "peer", "name", "eth0", "netns", self.ns)
        nl.sh("ip", "link", "set", aussen, "master", nl.BRUECKE, "up")
        ns = ["ip", "netns", "exec", self.ns]
        nl.sh(*ns, "ip", "addr", "add", f"{self.ip}/16", "dev", "eth0")
        nl.sh(*ns, "ip", "link", "set", "eth0", "up")
        nl.sh(*ns, "ip", "link", "set", "lo", "up")
        nl.sh(*ns, "tc", "qdisc", "replace", "dev", "eth0", "root", "handle", "1:", "tbf",
              "rate", f"{self.mbit}mbit", "burst", "256kb", "latency", "400ms")
        nl.sh(*ns, "tc", "qdisc", "add", "dev", "eth0", "parent", "1:1", "handle", "10:", "prio",
              "bands", "4", "priomap", *(["3"] * 16))
        for i, z in enumerate(ZONEN):
            nl.sh(*ns, "tc", "qdisc", "add", "dev", "eth0", "parent", f"10:{i + 1}", "handle", f"{20 + i}:", *self.regel(z))
        # Alles uebrige (Bruecke, Stoerer): nur der Zugang.
        nl.sh(*ns, "tc", "qdisc", "add", "dev", "eth0", "parent", "10:4", "handle", "24:",
              "netem", "delay", f"{self.zugang_ms}ms", "loss", f"{self.verlust}%")
        nl.sh("tc", "qdisc", "replace", "dev", aussen, "root", "netem", "delay", f"{self.zugang_ms}ms",
              f"{max(1, self.zugang_ms // 5)}ms")

    def ziel_eintragen(self, andere: "Ortsknoten") -> None:
        """Verkehr zu `andere` geht ueber das Band seiner Zone."""
        nl.sh("ip", "netns", "exec", self.ns, "tc", "filter", "add", "dev", "eth0", "parent", "10:", "protocol", "ip",
              "prio", "1", "u32", "match", "ip", "dst", f"{andere.ip}/32", "flowid", f"10:{ZONEN.index(andere.zone) + 1}")

    def zone_trennen(self, zone: str, getrennt: bool) -> None:
        """Verkehr in eine Zone ganz verwerfen oder wieder durchlassen."""
        (self.getrennt.add if getrennt else self.getrennt.discard)(zone)
        i = ZONEN.index(zone)
        nl.sh("ip", "netns", "exec", self.ns, "tc", "qdisc", "change", "dev", "eth0", "parent", f"10:{i + 1}",
              "handle", f"{20 + i}:", *self.regel(zone))


def zeilen(k: Knoten, art: str) -> list[dict]:
    """Die Protokollzeilen einer Art, ohne alles zu zerlegen."""
    marke = f'"{art}"'
    aus = []
    for datei in sorted(k.ordner.glob("*.jsonl")):
        with open(datei, encoding="utf-8", errors="replace") as f:
            for z in f:
                if marke in z:
                    try:
                        d = json.loads(z)
                    except json.JSONDecodeError:
                        continue
                    if d.get("art") == art:
                        aus.append(d)
    return aus


def shard_pids(k: Knoten) -> list[int]:
    """Die Prozesse von `myl-shard` im Namensraum eines Knotens."""
    pids = []
    for p in nl.sh("ip", "netns", "pids", k.ns, pruefen=False).stdout.split():
        try:
            befehl = pathlib.Path(f"/proc/{p}/cmdline").read_bytes()
        except OSError:
            continue
        # Nur der Programmname: Die Befehlszeile des Knotens nennt den Shard
        # in `--shard-programm` mit, und der erste Entwurf schoss so den
        # Knoten statt seines Shards ab.
        if pathlib.Path(befehl.split(b"\0", 1)[0].decode(errors="replace")).name == "myl-shard":
            pids.append(int(p))
    return pids


def lauf(args: argparse.Namespace, ordner: pathlib.Path) -> None:
    takt = args.blocktakt
    gesamt = 1 + len(MINER)

    def stoerung(i: int) -> tuple[int, float]:
        anteil = i / max(1, gesamt - 1)
        return int(args.latenz_min + anteil * (args.latenz_max - args.latenz_min)), round(
            args.verlust_min + anteil * (args.verlust_max - args.verlust_min), 2
        )

    # Mit `--real`: Zone, Zugang (Erzeuger 1 ms, Miner 3 bis 12 ms), Verlust
    # (0,05 % im Rechenzentrum, 0,1 bis 1 % zu Hause) und Bandbreite nach oben
    # (1 Gbit/s, zu Hause 20 bis 100 Mbit/s), fest aus der Saat.
    ort = random.Random(args.saat * 7919)

    def ortsknoten(i: int, name: str | None) -> Ortsknoten:
        if name is None:
            return Ortsknoten(i, ordner, "Europe", 1, 0.05, 1000)
        return Ortsknoten(i, ordner, MINERZONE[name], ort.randint(3, 12), round(ort.uniform(0.1, 1.0), 2),
                          ort.choice([20, 30, 50, 100]))

    if args.real:
        alle = [ortsknoten(0, None)] + [ortsknoten(i + 1, m[0]) for i, m in enumerate(MINER)]
    else:
        alle = [Knoten(i, ordner, *stoerung(i)) for i in range(gesamt)]
    erzeuger = alle[0]
    miner = alle[1:]
    rolle = {k.name: m for k, m in zip(miner, MINER)}
    # ⚑ **Befunde sammeln statt abbrechen** (mit `--real`): Ein Lauf ueber
    #   Nacht soll alles zeigen, was er findet, nicht nur das erste.
    befunde: list[str] = []
    bericht: list[str] = []
    luegner = next(k for k in miner if rolle[k.name][0] == "luegner")
    ehrlich = [k for k in miner if k is not luegner]
    # ⚑ Vergleich der Speicherhaltung: jeder zweite Miner mit gedeckelten
    #   Arenen von glibc, die anderen wie ausgeliefert.
    if args.arenen:
        for k in miner[1::2]:
            k.umgebung["MALLOC_ARENA_MAX"] = str(args.arenen)

    def epoche() -> int:
        return max(0, erzeuger.hoehe()) // EPOCHE

    zahler_adresse = None

    def angaben(k: Knoten) -> list[str]:
        name, ebenen, us = rolle[k.name]
        aus = [
            "--konsensschluessel", str(k.ordner / "konsens.key"),
            "--modellzusage", f"{ebenen}:{us}",
            "--zone", MINERZONE[name] if args.real else "Europe",
            "--shard-programm", str(args.shard),
            "--artefakte", str(args.artefakte),
        ]
        if args.real:
            aus += ["--stimmsatz", str(ordner / "stimmsatz.txt")]
        # Mit Torknoten tragen alle ein Auszahlungskonto ein; das von m00 ist
        # das Wallet des Zahlers.
        if args.tor:
            aus += ["--konto", zahler_adresse if name == "m00" else testkonto(name)]
        return aus

    def myl(*befehl: str, eingabe: str | None = None) -> subprocess.CompletedProcess:
        """`myl` im Namensraum des Torknotens, mit Wallets in seinem Ordner."""
        umgebung = {**os.environ, "MYL_WALLETS": str(erzeuger.ordner / "wallets"),
                    "MYL_EINSTELLUNGEN": str(erzeuger.ordner / "client.json")}
        return subprocess.run(["ip", "netns", "exec", erzeuger.ns, str(args.myl), *befehl], input=eingabe,
                              capture_output=True, text=True, env=umgebung, timeout=900)

    relais_angaben = ["--relais", RELAIS, "--relaisausweis", str(erzeuger.ordner / "relais.ausweis")]

    nl.bruecke_anlegen()
    for k in alle:
        k.netz_anlegen()
    if args.real:
        for k in alle:
            for j in alle:
                if j is not k:
                    k.ziel_eintragen(j)
        melde("Zonen: " + ", ".join(
            f"{z}: " + " ".join(("k0" if k is erzeuger else rolle[k.name][0]) + f"({k.zugang_ms}ms,{k.mbit}M,{k.verlust}%)"
                                for k in alle if k.zone == z) for z in ZONEN))
    namen = ", ".join(f"{k.name}={rolle[k.name][0]}" for k in miner)
    melde(f"Erzeuger k0 und {len(miner)} Miner ({namen}), Takt {takt} ms")

    ortsdienst = None
    zusatz_erzeuger: list[str] = []
    # Der Konsensschluessel des Erzeugers vorab: fuer den Ortsdienst (`--tor`)
    # und fuer seine Zeile im Stimmsatz (`--real`).
    schluessel = erzeuger.ordner / "konsens.key"
    if args.tor or args.real:
        erzeuger.ordner.mkdir(parents=True, exist_ok=True)
        schluessel.write_text(f"{SCHLUESSELKOPF}\n{os.urandom(32).hex()}\n")
        schluessel.chmod(0o600)
        zusatz_erzeuger = ["--konsensschluessel", str(schluessel)]
    if args.real:
        # ⚑ Der Stimmsatz aus den Zeilen der Validatoren, wie ein Betreiber
        #   sie mit `--stimmsatzzeile` erzeugt; jeder Knoten liest dieselbe
        #   Datei, Nichtmitglieder hoeren zu.
        zeilen_satz = ["netz probe"]
        for k in [erzeuger] + [m for m in miner if rolle[m.name][0] in VALIDATOREN]:
            k.ordner.mkdir(parents=True, exist_ok=True)
            r = subprocess.run([str(args.programm), "--name", k.name, "--stimmsatzzeile", str(STAKE),
                                "--konsensschluessel", str(k.ordner / "konsens.key")],
                               capture_output=True, text=True, cwd=nl.WURZEL, timeout=60)
            zeile = next((z for z in r.stdout.splitlines() if z.startswith("validator ")), None)
            if zeile is None:
                raise Befund(f"keine Stimmsatzzeile fuer {k.name}: {r.stderr.strip()[-300:]}")
            zeilen_satz.append(zeile)
        (ordner / "stimmsatz.txt").write_text("\n".join(zeilen_satz) + "\n")
        zusatz_erzeuger += ["--stimmsatz", str(ordner / "stimmsatz.txt")]
        melde(f"Stimmsatz: k0 und {', '.join(VALIDATOREN)}, je {STAKE}; ein Pruefpunkt braucht drei Stimmen")
    if args.tor:
        # Der Zahler aus seinen Woertern; seine Adresse wird Konto von m00.
        r = myl("wallet", "wiederherstellen", "zahler", eingabe=ZAHLER_WOERTER + "\n")
        if r.returncode != 0:
            raise Befund(f"das Wallet des Zahlers: {r.stderr.strip()}")
        zahler_adresse = next(z.split()[-1] for z in r.stdout.splitlines() if "wiederhergestellt" in z)
        (erzeuger.ordner / "ort").mkdir(exist_ok=True)
        ortsdienst = subprocess.Popen(
            ["ip", "netns", "exec", erzeuger.ns, str(args.pod_node), "--artefakte", str(args.artefakte),
             "--ortsleitung", ORT, "--ausweis", str(erzeuger.ordner / "ort"), "--pod", POD_HEX,
             "--knotenschluessel", str(schluessel), "--pipeline", PIPELINE_HEX, "--epoche", "0", "--deckel", "16",
             "--podrelais", RELAIS, "--podrelaisausweis", str(erzeuger.ordner / "relais.ausweis")],
            stdout=open(erzeuger.ordner / "ortsdienst.txt", "a"), stderr=subprocess.STDOUT)
        warte(lambda: "Shard-Dienst auf" in (erzeuger.ordner / "ortsdienst.txt").read_text(errors="replace"),
              60, "der Ortsdienst oeffnet", alle)
        zusatz_erzeuger += ["--tuer", TUER, "--ortsleitung", ORT,
                            "--ortsausweis", str(erzeuger.ordner / "ort"), "--pod", POD_HEX,
                            "--podrelais", RELAIS, "--podrelaisausweis", str(erzeuger.ordner / "relais.ausweis")]
        melde(f"Torknoten k0 mit Tuer {TUER} und Ortsdienst; Zahler {zahler_adresse[:16]}... ist Konto von m00")
    erzeuger.starten(args.programm, None, True, takt, zusatz_erzeuger)
    pid = erzeuger.warte_auf_peer_id(30)
    einstieg = f"/ip4/{erzeuger.ip}/tcp/{nl.PORT}/p2p/{pid}"
    for k in miner:
        k.starten(args.programm, einstieg, False, takt, angaben(k))
    for k in miner:
        k.warte_auf_peer_id(30)
    erlaubt = max(40, 3000 // takt)

    def holt_auf(k: Knoten) -> bool:
        h, e = k.hoehe(), erzeuger.hoehe()
        return h >= 0 and e >= 0 and h >= e - erlaubt

    warte(lambda: all(holt_auf(k) for k in miner), 120, "alle an der Spitze", alle)

    # ---- 1. Anmeldung und Zusage ------------------------------------------
    warte(lambda: all(zeilen(k, "zusage_eingereicht") for k in miner), 300, "jeder Miner reicht seine Zusage ein", alle)
    melde(f"Zusagen eingereicht von allen {len(miner)} Minern bei Hoehe {erzeuger.hoehe()}")

    # ---- 2. Pods aus der Kette, 4. der Luegner auf einer Position ----------
    # 📌 Was ein Miner fuehrt, steht nicht je Epoche im Protokoll: Behaelt er
    #    seinen Bereich, laeuft der Shard weiter, ohne neues
    #    `shard_gestartet`. Der erste Entwurf zaehlte Startzeilen je Epoche und
    #    hielt in Epoche 10 an. Jetzt zaehlt, wer einen laufenden Shard hat,
    #    mit dem Bereich aus seiner letzten Startzeile.
    def stand() -> dict[str, dict]:
        aus = {}
        for k in miner:
            if shard_pids(k):
                g = zeilen(k, "shard_gestartet")
                if g:
                    aus[k.name] = g[-1]
        return aus

    def in_epoche(e: int) -> None:
        """Bis Epoche `e` laeuft und die Knoten ihre Shards umgestellt haben."""
        warte(lambda: epoche() >= e, (e + 1) * EPOCHE * takt / 1000 + 120, f"Epoche {e}", alle)
        time.sleep(8)  # vier Aufnahmen: Zeit, Shards zu beenden und zu starten
        warte(lambda: len(stand()) >= 4, EPOCHE * takt / 1000 + 60, f"vier laufende Shards in Epoche {e}", alle)

    def pods_melden(e: int) -> dict[str, dict]:
        st = stand()
        pods: dict[int, list[str]] = {}
        for name, z in sorted(st.items(), key=lambda kv: (kv[1].get("pod"), kv[1].get("position"))):
            pods.setdefault(z.get("pod"), []).append(f"{rolle[name][0]}[{z.get('von')}..{z.get('bis')})")
        melde(f"Epoche {e}: " + "; ".join(f"Pod {p}: {', '.join(v)}" for p, v in sorted(pods.items())))
        return st

    # ---- Geld wie ein Nutzer (`--real`): Ueberweisung und Hinterlegung -----
    # Die Epoche der kurzen Sitzung; ihre Hinterlegung muss nach der
    # Nachfrist (100 Epochen) zurueckkommen.
    kurzsitzung: dict[str, int] = {}

    def ueberweisen_und_hinterlegen(guthaben, nonce) -> None:
        an = testkonto("empfaenger-neu")
        e0, vorher = epoche(), guthaben()
        r = myl("wallet", "ueberweisen", "zahler", an, "1", "--ja", *relais_angaben)
        if r.returncode != 0:
            befunde.append(f"Ueberweisen aus der Wallet: {r.stderr.strip()[-300:]}")
            return
        gebuehr_zeile = next((z.strip() for z in r.stdout.splitlines() if "Gebühr" in z), "?")
        warte(lambda: nonce() >= 3, 120, "die Ueberweisung kommt in die Kette", alle)
        erhalten = int((konto_abfragen(erzeuger, an) or {}).get("guthaben", -1))
        # Die Probepraegung kommt in Vielfachen von 10 MYL dazu; ueber eine
        # Epochengrenze kaeme auch echte Auszahlung, dann ist es nicht exakt.
        abgang = (vorher - guthaben()) % PROBEPRAEGUNG
        gebuehr = abgang - 1_000_000 - AUFSCHLAG
        exakt = epoche() == e0
        melde(f"Ueberweisung an ein neues Konto: erhalten {erhalten}, abgebucht 1 MYL + Aufschlag {AUFSCHLAG} + "
              f"Gebuehr {gebuehr if exakt else '?'} (Vorschau: {gebuehr_zeile})")
        if erhalten != 1_000_000:
            befunde.append(f"der Empfaenger der Ueberweisung hat {erhalten} statt 1 000 000")
        if exakt and not 0 < gebuehr < 10_000:
            befunde.append(f"abgebucht passt nicht zu Betrag, Aufschlag und Gebuehr: Gebuehr {gebuehr}")
        # Eine Sitzung fuer eine Epoche: Ihre Hinterlegung wird nach der
        # Nachfrist zurueckerwartet.
        r = myl("wallet", "sitzung", "zahler", "kurzagent", "100", "1", *relais_angaben)
        if r.returncode != 0:
            befunde.append(f"kurze Sitzung: {r.stderr.strip()[-300:]}")
            return
        warte(lambda: nonce() >= 4, 120, "die kurze Sitzung kommt in die Kette", alle)
        hinterlegt = int((konto_abfragen(erzeuger, zahler_adresse) or {}).get("hinterlegt", -1))
        kurzsitzung.update(epoche=epoche(), hinterlegt=hinterlegt)
        melde(f"Zwei Sitzungen offen, hinterlegt {hinterlegt} (erwartet 2 x {HINTERLEGUNG_SITZUNG}); "
              f"Rueckgabe der kurzen ab Epoche {epoche() + 1 + SITZUNG_NACHFRIST + 1}")
        if hinterlegt != 2 * HINTERLEGUNG_SITZUNG:
            befunde.append(f"hinterlegt fuer zwei Sitzungen: {hinterlegt}")

    # ---- 7. Der Empfaenger: Wallet, Sitzung, Agent ueber die Tuer ----------
    # ⚑ **Vor der Pruefung des Luegners** (2026-10-26): Der Weg braucht nur
    #   Pods und die Probepraegung. Im zweiten Lauf hing er sonst an Fund 646
    #   (der Luegner blieb ungesperrt, weil jede Messung verworfen wurde).
    def empfaenger_pruefen() -> None:
        def guthaben() -> int:
            k = konto_abfragen(erzeuger, zahler_adresse)
            return int(k["guthaben"]) if k else -1

        warte(lambda: guthaben() >= 6_000_000, 4 * EPOCHE * takt / 1000 + 120, "der Zahler verdient Test-MYL", alle)
        melde(f"Zahler hat {guthaben() / 1e6:g} MYL aus der Probepraegung von m00")
        # 📌 Gewartet wird auf die Nonce des Wallets, nicht auf sein Guthaben:
        #    Die Probepraegung aendert das Guthaben nebenher, und der erste
        #    Entwurf las „vorher“ erst nach dem Burn und wartete ewig.
        def nonce() -> int:
            k = konto_abfragen(erzeuger, zahler_adresse)
            return int(k["nonce"]) if k else -1

        r = myl("wallet", "credits", "zahler", "5", *relais_angaben)
        if r.returncode != 0:
            raise Befund(f"Credits kaufen: {r.stderr.strip()}")
        warte(lambda: nonce() >= 1, 120, "der Burn kommt in die Kette", alle)
        # Die Sitzung gilt so lange wie der Lauf: Im Dauerteil fragt der
        # Agent je Epoche weiter.
        dauer = "20" if args.kurz else str(args.epochen + 30)
        r = myl("wallet", "sitzung", "zahler", "agent", "5000", dauer, *relais_angaben)
        if r.returncode != 0:
            raise Befund(f"Sitzung eroeffnen: {r.stderr.strip()}")
        warte(lambda: nonce() >= 2, 120, "die Sitzung kommt in die Kette", alle)
        melde("Credits gekauft, Sitzung fuer den Agenten eroeffnet: " + r.stdout.strip().splitlines()[0])
        if args.real:
            ueberweisen_und_hinterlegen(guthaben, nonce)
        # Kurze Prompts: Der Prefill geht Token fuer Token durch alle
        # Positionen, ueber die schlechteste Leitung (185 ms, 4,6 % Verlust)
        # bis 15 s je Runde. Lange Prompts gehoeren in den langen Lauf.
        fragen = ["Der Himmel ist", "Zwei plus zwei", "Hallo"]
        e_frage = epoche()
        for f in fragen:
            # 📌 Fund 649: Am Epochenwechsel beenden die Knoten ihre Shards
            #    sofort, auch mitten in einer Anfrage, und ueber Knoten mit
            #    Latenz braucht ein Token Sekunden. Gefragt wird deshalb am
            #    Anfang einer Epoche, wenn vier Shards laufen; jede Frage
            #    trifft so einen anderen Pod.
            #    Vier Token: Ueber Knoten mit Latenz kostet eines rund 4 s
            #    (Fund 650), die Antwort muss in die Epoche passen.
            in_epoche(epoche() + 1)
            r = myl("frage", "--tuer", TUER, "--agent", "agent", "--token", "4", f)
            if r.returncode != 0 or not r.stdout.strip():
                fehler = " | ".join(z for z in r.stderr.splitlines() if not z.startswith("Hinweis:"))
                ort = (erzeuger.ordner / "ortsdienst.txt").read_text(errors="replace").splitlines()[-5:]
                melde(f"Rueckgabe {r.returncode}, Ausgabe {r.stdout!r}, Fehler {fehler!r}; Ortsdienst: {ort}")
                # Zum Nachsehen im laufenden Behaelter (docker exec).
                if os.environ.get("MYL_HALTEN"):
                    melde("angehalten fuer die Fehlersuche (MYL_HALTEN)")
                    time.sleep(int(os.environ["MYL_HALTEN"]))
                raise Befund(f"die Tuer antwortete nicht auf {f!r}")
            melde(f"Agent ueber die Tuer: {f!r} -> {r.stdout.strip()!r} ({r.stderr.strip().splitlines()[-1]})")
        # Das Buendel am Ende der Epoche, dann die Auszahlung danach.
        def buendel() -> str | None:
            text = (erzeuger.ordner / "ortsdienst.txt").read_text(errors="replace")
            treffer = [z for z in text.splitlines() if "Bündel" in z]
            return treffer[-1] if treffer else None
        warte(lambda: buendel() is not None, 2 * EPOCHE * takt / 1000 + 120, "der Ortsdienst reicht ein Buendel ein", alle)
        melde(f"Ortsdienst: {buendel()}")
        if "eingereicht" not in (buendel() or ""):
            raise Befund(f"das Buendel kam nicht durch: {buendel()}")
        in_epoche(e_frage + 1 if epoche() <= e_frage else epoche())
        time.sleep(5)
        echt = {}
        for k in miner:
            name = rolle[k.name][0]
            # 📌 Das Konto von m00 ist der Zahler: Seit den Gebuehren ist sein
            #    Guthaben ohnehin kein Vielfaches der Probepraegung mehr, es
            #    zaehlte immer. Die anderen Miner tragen die Pruefung.
            if name == "m00":
                continue
            stand_k = konto_abfragen(erzeuger, testkonto(name))
            if stand_k and int(stand_k["guthaben"]) % PROBEPRAEGUNG != 0:
                echt[name] = int(stand_k["guthaben"])
        abrechnungen = sum(int(z.get("anzahl", 0)) for z in zeilen(erzeuger, "abrechnungen_verbreitet"))
        melde(f"Auszahlung fuer bezeugte Arbeit an {sorted(echt)}; Abrechnungen verbreitet: {abrechnungen}")
        if not echt:
            raise Befund("kein Miner bekam eine Auszahlung fuer bezeugte Arbeit")
        if abrechnungen == 0:
            raise Befund("die Tuer hat keine Abrechnung verbreitet")


    if args.tor:
        in_epoche(1)
        empfaenger_pruefen()

    luegner_epoche = None
    # Pods gibt es ab Epoche 1 (der erste Entwurf sah erst ab 2 hin und las
    # dann eine aeltere Sperre).
    # 📌 Fund 641: Die Positionen verteilt der Zufall der Kette; bis
    #    Epoche 8 blieb der Luegner in einem von rund zehn kurzen Laeufen
    #    ohne. Sechzehn Epochen kosten nur, wenn er spaet drankommt.
    # 📌 Die tatsaechliche Epoche, nicht die Zaehlvariable: Lief schon eine
    #    spaetere (der Torteil braucht drei), kehrte `in_epoche` sofort zurueck,
    #    und die Meldung nannte Epoche 1, wo es Epoche 4 war (2026-10-27).
    e = 1
    while e <= LUEGNER_BIS:
        in_epoche(e)
        e = epoche()
        if luegner.name in pods_melden(e):
            luegner_epoche = e
            break
        e += 1
    if luegner_epoche is None:
        raise Befund(f"der Luegner bekam bis Epoche {LUEGNER_BIS} keine Position")

    # ---- 3. Rechnen ueber das Netz, 4. Meldung und Sperre -----------------
    # Die Sperre gilt fuer die Epoche nach der Meldung; verlangt ist die erste
    # ab seiner Position.
    # 📌 Bis zu zwoelf Epochen statt drei: Mit kurzen Wegen und einer Epoche
    #    von 108 s schaffen in einem Messfenster (ab einem Viertel der Epoche,
    #    zwei Ueberschreitungen je Messendem im Abstand eines Achtels) oft nur
    #    zwei von vier Mitgliedern ihr Zeugnis; dann entkommt der Luegner bis
    #    zu seiner naechsten Position (kurzer Lauf mit `--real`, 2026-10-27).
    def neue_sperre() -> dict | None:
        return next((z for z in zeilen(luegner, "fristsperre") if z.get("epoche", 0) > luegner_epoche), None)

    try:
        # Im kurzen Lauf mit `--real` vier Epochen: Der Rest des Laufs soll nicht warten.
        frist = (4 if args.kurz and args.real else LUEGNER_SPERRE_EPOCHEN) * EPOCHE * takt / 1000 + 120
        warte(lambda: neue_sperre() is not None, frist, "die Kette sperrt den Luegner", alle)
    except Befund as b:
        if not args.real:
            raise
        # ⚑ Mit `--real` ein Befund mit Zahlen, kein Abbruch: Die Pruefungen
        #   danach sollen trotzdem laufen.
        # Seine Kennung an der winzigen Frist: Er sagt 1 us je Ebene zu.
        alle_messungen = [z for k in ehrlich for z in zeilen(k, "fristmessung")]
        kennung = next((z.get("mitglied") for z in alle_messungen if int(z.get("frist_us", 10**9)) < 1000), None)
        gegen = [z for z in alle_messungen if z.get("mitglied") == kennung]
        urteile = {u: sum(1 for z in gegen if str(z.get("urteil", "")).startswith(u)) for u in ("Verdacht", "Zeugnis")}
        positionen = sorted({int(z.get("epoche", -1)) for z in zeilen(luegner, "shard_gestartet")})
        befunde.append(f"{b}; Positionen in Epochen {positionen}, {len(gegen)} Messungen gegen ihn, {urteile}, "
                       f"Meldungen {sum(len(zeilen(k, 'fristmeldung')) for k in ehrlich)}")
        melde(f"Befund: {befunde[-1]}")
    sperre = neue_sperre()
    gesperrt_e = int(sperre["epoche"]) if sperre else None
    gemessen = [z for k in ehrlich for z in zeilen(k, "fristmessung")]
    meldungen = [z for k in ehrlich for z in zeilen(k, "fristmeldung")]
    melde(
        f"Luegner auf Position in Epoche {luegner_epoche}, gesperrt fuer Epoche {gesperrt_e}; "
        f"{len(gemessen)} Zeitproben ueber das Netz, {len(meldungen)} Meldungen"
    )
    if gesperrt_e is not None:
        in_epoche(gesperrt_e)
        if epoche() == gesperrt_e and shard_pids(luegner):
            raise Befund(f"der gesperrte Luegner fuehrt in Epoche {gesperrt_e} einen Shard")
        melde(f"Epoche {gesperrt_e}: der Luegner fuehrt keinen Shard")

    # ---- 5. Stoerungen -----------------------------------------------------
    def shard_absturz() -> None:
        e = epoche()
        st = stand()
        # 📌 Nur ein Shard, der in dieser Epoche gestartet wurde: Ein noch
        #    laufender aus der vorigen kann zu einem Miner gehoeren, der keine
        #    Position mehr hat und dessen Knoten ihn gleich beendet; der wird
        #    zu Recht nicht neu gestartet (Epoche 95 des ersten langen Laufs).
        kandidaten = [k for k in ehrlich if k.name in st and st[k.name].get("epoche") == e]
        if not kandidaten:
            melde(f"Epoche {e}: kein laufender Shard fuer den Absturz")
            return
        opfer = random.Random(e).choice(kandidaten)
        vorher = len(zeilen(opfer, "shard_gestartet"))
        for p in shard_pids(opfer):
            os.kill(p, signal.SIGKILL)
        # 📌 Endet die Epoche vor dem Neustart und hat das Opfer danach keine
        #    Position, startet zu Recht nichts neu (langer Lauf 3, Epoche 30:
        #    das Geruest wartete 120 s auf einen Shard, den es nicht mehr gab).
        def zurueck() -> bool:
            if len(zeilen(opfer, "shard_gestartet")) > vorher and shard_pids(opfer):
                return True
            return epoche() > e and not shard_pids(opfer) and opfer.name not in stand()
        dauer = warte(zurueck, 120, f"der Shard von {rolle[opfer.name][0]} startet neu", alle)
        if epoche() > e and not shard_pids(opfer):
            melde(f"Shard von {rolle[opfer.name][0]} abgestuerzt; die Epoche endete davor, ohne Position kein Neustart")
            return
        melde(f"Shard von {rolle[opfer.name][0]} abgestuerzt, neu gestartet in {dauer:.1f} s")

    def miner_absturz(r: random.Random) -> None:
        opfer = r.choice(ehrlich)
        opfer.neu_starten()
        dauer = warte(lambda: holt_auf(opfer), 300, f"{rolle[opfer.name][0]} holt nach dem Absturz auf", alle)
        melde(f"Miner {rolle[opfer.name][0]} abgestuerzt, aufgeholt in {dauer:.1f} s")

    shard_absturz()
    r = random.Random(args.saat)
    if not args.kurz:
        miner_absturz(r)

    # ---- Naeher an echten Bedingungen (`--real`) ---------------------------
    def bestaetigt() -> int:
        return int(erzeuger.metriken().get("myelith_pruefpunkt_bestaetigt", -1))

    def trennung(zonen: list[str], sekunden: int) -> None:
        """Trennt `zonen` vom Rest, heilt, und misst Aufholen und Pruefpunkte.

        ⚑ Ein Pruefpunkt braucht drei von vier Stimmen. Liegt der Erzeuger
        mit drei Validatoren auf einer Seite, muessen waehrend der Trennung
        Pruefpunkte entstehen; mit zweien duerfen keine entstehen.
        """
        seite = [k for k in alle if k.zone in zonen]
        drueben = [k for k in alle if k.zone not in zonen]
        mit_erzeuger = seite if erzeuger in seite else drueben
        stimmen = sum(1 for k in mit_erzeuger if k is erzeuger or rolle.get(k.name, ("",))[0] in VALIDATOREN)
        b0 = bestaetigt()
        for k in seite:
            for z in ZONEN:
                if z not in zonen:
                    k.zone_trennen(z, True)
        for k in drueben:
            for z in zonen:
                k.zone_trennen(z, True)
        time.sleep(sekunden)
        b1 = bestaetigt()
        for k in alle:
            for z in list(k.getrennt):
                k.zone_trennen(z, False)
        aufholen = warte(lambda: all(holt_auf(k) for k in miner), 600, f"alle holen nach der Trennung von {zonen} auf", alle)
        weiter = warte(lambda: bestaetigt() > max(b1, b0), 600, "der naechste Pruefpunkt nach der Trennung", alle)
        zeile = (f"Trennung {'+'.join(zonen)} fuer {sekunden} s ({stimmen} von 4 Stimmen beim Erzeuger): "
                 f"bestaetigt {b0} -> {b1} waehrend, aufgeholt in {aufholen:.0f} s, naechster Pruefpunkt nach {weiter:.0f} s")
        melde(zeile)
        bericht.append(zeile)
        if stimmen >= 3 and b1 <= b0:
            befunde.append(f"waehrend der Trennung von {zonen} kein Pruefpunkt, obwohl {stimmen} Stimmen beim Erzeuger")
        # Ein Pruefpunkt, der schon fast stand, darf noch durchgehen.
        if stimmen < 3 and b1 > b0 + 150:
            befunde.append(f"Pruefpunkt ohne Mehrheit waehrend der Trennung von {zonen}: {b0} -> {b1}")

    def einsteiger() -> None:
        """Ein neuer Miner in Asien, mitten im Lauf: Er holt auf, versorgt
        sich ueber den Probehahn, meldet sich an und sagt zu."""
        name = EINSTEIGER[0]
        k = ortsknoten(len(alle), name)
        k.netz_anlegen()
        for j in alle:
            j.ziel_eintragen(k)
            k.ziel_eintragen(j)
        rolle[k.name] = EINSTEIGER
        k.starten(args.programm, einstieg, False, takt, angaben(k))
        k.warte_auf_peer_id(30)
        alle.append(k)
        miner.append(k)
        ehrlich.append(k)
        h0 = erzeuger.hoehe()
        oben = warte(lambda: holt_auf(k), 900, "der Einsteiger holt auf", alle)
        dabei = warte(lambda: bool(zeilen(k, "zusage_eingereicht")), 900, "der Einsteiger meldet sich an und sagt zu", alle)
        zeile = (f"Einsteiger {name} (Asien) bei Hoehe {h0}: an der Spitze nach {oben:.0f} s, angemeldet und "
                 f"zugesagt nach weiteren {dabei:.0f} s, Probehahn {len(zeilen(k, 'probehahn'))} mal")
        melde(zeile)
        bericht.append(zeile)

    def last() -> None:
        """Burns in Stufen ueber die Relaisleitung des Torknotens: Wie viel
        nimmt die Kette auf, und wie antwortet der Grundpreis?"""
        stufen, sekunden = ("50,200", "20") if args.kurz else ("50,100,200,400,800,1600", "60")
        r = subprocess.run(["ip", "netns", "exec", erzeuger.ns, str(args.lastgeber), RELAIS,
                            str(erzeuger.ordner / "relais.ausweis"), "100", stufen, sekunden],
                           capture_output=True, text=True, timeout=3600)
        for z in r.stdout.splitlines():
            if z.startswith("[last]"):
                melde(z)
                bericht.append(z)
        if r.returncode != 0:
            befunde.append(f"Lastgeber: {r.stderr.strip()[-300:]}")
        dauer = warte(lambda: all(holt_auf(k) for k in miner), 900, "alle nach der Last an der Spitze", alle)
        zeile = f"Nach der Last alle an der Spitze in {dauer:.0f} s"
        melde(zeile)
        bericht.append(zeile)

    def platz() -> tuple[int, int]:
        """Kettendatei des Erzeugers und Protokolle eines Miners, in Bytes."""
        kette = sum(f.stat().st_size for f in erzeuger.ordner.glob("kette.dat*") if f.is_file())
        protokoll = sum(f.stat().st_size for f in miner[0].ordner.glob("*.jsonl") if f.is_file())
        return kette, protokoll

    platz_anfang = (epoche(), *platz())
    if args.real:
        if args.tor and args.lastgeber:
            last()
        trennung(["Asia"], 60)
        trennung(["NorthAmerica", "Asia"], 60)
        if args.kurz:
            einsteiger()

    # ---- Dauer: Epoche um Epoche, mit Stoerungen ---------------------------
    stoerer = None
    if args.stoerer:
        stoerer = Stoerer(ordner)
        stoerer.starten([k.ip for k in alle])
    def speicher_ablegen(e: int) -> None:
        with open(ordner / "speicher.jsonl", "a", encoding="utf-8") as f:
            for k in miner:
                bild = k.speicherbild()
                if bild:
                    f.write(json.dumps({"epoche": e, "knoten": k.name,
                                        "arenen_max": k.umgebung.get("MALLOC_ARENA_MAX"), **bild}) + "\n")

    ziel_epoche = epoche() + (1 if args.kurz else args.epochen)
    letzte = epoche()
    # ⚑ **Verkehr im Dauerteil** (mit `--tor`): je Epoche eine Frage des
    #   Agenten ueber die Tuer, vor den Abstuerzen dieser Epoche. Gezaehlt
    #   wird, wie viele beantwortet wurden; Abstuerze und Stoerer duerfen
    #   einzelne kosten, nicht die meisten.
    verkehr = {"gefragt": 0, "beantwortet": 0, "sekunden": []}
    def dauerfrage(e: int) -> None:
        f = ["Der Himmel ist", "Zwei plus zwei", "Hallo"][e % 3]
        beginn = time.monotonic()
        a = myl("frage", "--tuer", TUER, "--agent", "agent", "--token", "4", f)
        verkehr["gefragt"] += 1
        if a.returncode == 0 and a.stdout.strip():
            verkehr["beantwortet"] += 1
            verkehr["sekunden"].append(time.monotonic() - beginn)
        else:
            fehler = " | ".join(z for z in a.stderr.splitlines() if not z.startswith("Hinweis:"))[-200:]
            melde(f"Epoche {e}: Frage ohne Antwort ({fehler})")
    def rueckgabe_pruefen() -> None:
        jetzt = int((konto_abfragen(erzeuger, zahler_adresse) or {}).get("hinterlegt", -1))
        zeile = (f"Kurze Sitzung aus Epoche {kurzsitzung['epoche']}: hinterlegt {kurzsitzung['hinterlegt']} -> {jetzt} "
                 f"nach der Nachfrist")
        melde(zeile)
        bericht.append(zeile)
        if jetzt != kurzsitzung["hinterlegt"] - HINTERLEGUNG_SITZUNG:
            befunde.append(f"die Hinterlegung der abgelaufenen Sitzung kam nicht zurueck: {zeile}")

    def dauer_real(e: int) -> None:
        """Im Dauerteil (`--real`): der Einsteiger nach zwanzig Epochen, je
        dreissig Epochen zwei Trennungen, die Rueckgabe der kurzen Sitzung,
        alle 25 Epochen der Platz."""
        if e == platz_anfang[0] + 20:
            einsteiger()
        if e % 30 == 15:
            trennung(["Asia"], 60)
        if e % 30 == 0:
            trennung(["NorthAmerica", "Asia"], 60)
        if kurzsitzung and e == kurzsitzung["epoche"] + SITZUNG_NACHFRIST + 4:
            rueckgabe_pruefen()
        if e % 25 == 0:
            kette, protokoll = platz()
            melde(f"Epoche {e}: Kettendatei {kette // 1_000_000} MB, Protokolle eines Miners {protokoll // 1_000_000} MB")

    while epoche() < ziel_epoche:
        for k in alle:
            k.lebt_noch()
        e = epoche()
        if e > letzte:
            letzte = e
            in_epoche(e)
            speicher = " ".join(f"{k.name}={max(0, k.speicher()[0]) // 1024}M" for k in miner)
            speicher_ablegen(e)
            messungen = sum(len(zeilen(k, "fristmessung")) for k in ehrlich)
            melde(f"Epoche {e}: {len(stand())} Shards, {messungen} Zeitproben bisher; {speicher}"
                  + (f"; Stoerer {stoerer.zaehler()}" if stoerer else ""))
            # Die Abstuerze am Anfang der Epoche, die Frage danach: Ein Absturz
            # kurz vor dem Wechsel traefe einen Shard, der ohnehin endet.
            if e % 5 == 0:
                shard_absturz()
            if e % 7 == 0:
                miner_absturz(r)
            if args.real and not args.kurz:
                dauer_real(e)
            if args.tor and not args.kurz:
                dauerfrage(e)
        time.sleep(5)
    speicher_ablegen(epoche())
    if args.tor and not args.kurz:
        sek = sorted(verkehr["sekunden"])
        mitte = f", Median {sek[len(sek) // 2]:.0f} s" if sek else ""
        abrechnungen = sum(int(z.get("anzahl", 0)) for z in zeilen(erzeuger, "abrechnungen_verbreitet"))
        melde(f"Verkehr: {verkehr['beantwortet']} von {verkehr['gefragt']} Fragen beantwortet{mitte}; "
              f"Abrechnungen verbreitet insgesamt: {abrechnungen}")
        if verkehr["gefragt"] and 2 * verkehr["beantwortet"] < verkehr["gefragt"]:
            text = f"nur {verkehr['beantwortet']} von {verkehr['gefragt']} Fragen im Dauerteil beantwortet"
            if not args.real:
                raise Befund(text)
            befunde.append(text)
    if stoerer:
        stoerer.beenden()

    # ---- Kein ehrlicher Miner gesperrt -----------------------------------
    falsch = [f"{rolle[k.name][0]} (Epochen {sorted(z.get('epoche') for z in zeilen(k, 'fristsperre'))})"
              for k in ehrlich if zeilen(k, "fristsperre")]
    if falsch:
        if not args.real:
            raise Befund(f"ehrliche Miner gesperrt: {', '.join(falsch)}")
        befunde.append(f"ehrliche Miner gesperrt: {', '.join(falsch)}")

    # ---- 6. Einigkeit --------------------------------------------------------
    if ortsdienst is not None:
        ortsdienst.terminate()
    erzeuger.beenden()

    def ruhig_und_einig() -> bool:
        hoehen = [k.hoehe() for k in miner]
        return len(set(hoehen)) == 1 and hoehen[0] > 0

    ende = time.monotonic() + 180
    while True:
        warte(ruhig_und_einig, max(1, ende - time.monotonic()), "alle Miner auf einer Hoehe", miner)
        hoehe = miner[0].hoehe()
        time.sleep(5)
        if ruhig_und_einig() and miner[0].hoehe() == hoehe:
            break
        if time.monotonic() > ende:
            raise Befund("die Hoehen kamen nicht zur Ruhe")
    for k in miner:
        k.beenden()
    wurzeln = {}
    for k in miner:
        a = [z for z in zeilen(k, "aufnahme") if z.get("hoehe") == hoehe]
        if not a:
            raise Befund(f"{k.name}: keine Aufnahme bei Hoehe {hoehe}")
        wurzeln[k.name] = a[-1]["zustandswurzel"]
    if len(set(wurzeln.values())) != 1:
        raise Befund(f"die Ketten laufen auseinander: {wurzeln}")
    zaehlung = {art: sum(len(zeilen(k, art)) for k in miner) for art in (
        "shard_gestartet", "shard_beendet", "fristmessung", "fristmeldung",
        "dateien_holen", "stille_nachfrage", "spuranfrage_eingegangen",
    )}
    melde("Zaehlung: " + ", ".join(f"{a}={n}" for a, n in zaehlung.items()))
    # Die Prüfpunkte: Der Erzeuger stimmt und baut das Zertifikat, die Kette
    # trägt es im nächsten Block (CONSENSUS 7.2).
    zertifikate = zeilen(erzeuger, "pruefpunkt_zertifikat")
    melde(f"Pruefpunkte: {len(zeilen(erzeuger, 'pruefpunkt_gestimmt'))} gestimmt, {len(zertifikate)} Zertifikate, "
          f"zuletzt bis Hoehe {max((int(z.get('hoehe', 0)) for z in zertifikate), default=0)}")
    # Die Stichprobe zieht der Erzeuger; angekommen ist sie bei den Minern.
    gezogen = zeilen(erzeuger, "stichprobe_gefragt")
    melde(f"Stichprobe: {len(gezogen)} Ziehungen, {sum(int(z.get('anfragen', 0)) for z in gezogen)} Anfragen, "
          f"{sum(int(z.get('ohne_adresse', 0)) for z in gezogen)} ohne Adresse; "
          f"angekommen {zaehlung['spuranfrage_eingegangen']}")
    melde(f"Ende: {len(wurzeln)} Miner bei Hoehe {hoehe}, eine Wurzel {next(iter(wurzeln.values()))}")
    if args.real:
        # Wer hat gestimmt, und was hat der Lauf belegt.
        stimmen = {"k0": len(zeilen(erzeuger, "pruefpunkt_gestimmt"))}
        for k in miner:
            if rolle[k.name][0] in VALIDATOREN:
                stimmen[rolle[k.name][0]] = len(zeilen(k, "pruefpunkt_gestimmt"))
        melde("Stimmen je Validator: " + ", ".join(f"{n}={s}" for n, s in stimmen.items()))
        # Der Erzeuger ist beendet; die Epoche folgt aus der Hoehe der Miner.
        e1 = hoehe // EPOCHE
        kette, protokoll = platz()
        if e1 > platz_anfang[0]:
            melde(f"Platz: Kettendatei {platz_anfang[1] // 1_000_000} -> {kette // 1_000_000} MB, Protokolle eines Miners "
                  f"{platz_anfang[2] // 1_000_000} -> {protokoll // 1_000_000} MB ueber {e1 - platz_anfang[0]} Epochen")
        for z in bericht:
            melde(f"Bericht: {z}")
        if befunde:
            raise Befund(f"{len(befunde)} Befund(e): " + " | ".join(befunde))


# Bis zu dieser Epoche muss der Luegner einmal eine Position bekommen.
LUEGNER_BIS = 16
# So viele Epochen nach seiner ersten Position darf die Sperre dauern.
LUEGNER_SPERRE_EPOCHEN = 12


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--programm", type=pathlib.Path, required=True, help="myl-node")
    p.add_argument("--shard", type=pathlib.Path, required=True, help="myl-shard")
    p.add_argument("--artefakte", type=pathlib.Path, required=True, help="das Probeartefakt")
    p.add_argument("--kurz", action="store_true", help="die Mechanik in rund zehn Minuten")
    p.add_argument("--epochen", type=int, default=100, help="so viele Epochen Dauer mit Stoerungen")
    p.add_argument("--blocktakt", type=int, default=25)
    p.add_argument("--latenz-min", type=int, default=50)
    p.add_argument("--latenz-max", type=int, default=200)
    p.add_argument("--verlust-min", type=float, default=1.0)
    p.add_argument("--verlust-max", type=float, default=5.0)
    p.add_argument("--saat", type=int, default=1)
    p.add_argument("--stoerer", action="store_true")
    p.add_argument("--arenen", type=int, help="MALLOC_ARENA_MAX fuer jeden zweiten Miner (Speichervergleich)")
    p.add_argument("--tor", action="store_true", help="Torknoten mit Tuer, Ortsdienst und einem Zahler mit Agent")
    p.add_argument("--pod-node", type=pathlib.Path, help="myl-pod-node (mit --tor)")
    p.add_argument("--myl", type=pathlib.Path, help="myl, der Client (mit --tor)")
    p.add_argument("--protokolle", type=pathlib.Path)
    p.add_argument("--real", action="store_true",
                   help="naeher an echten Bedingungen: Zonen, vier Validatoren, Trennungen, Einsteiger, Geld wie ein Nutzer")
    p.add_argument("--lastgeber", type=pathlib.Path, help="das Beispiel lastgeber aus myl-testclient (mit --tor --real)")
    args = p.parse_args()
    args.programm = args.programm.resolve()
    args.shard = args.shard.resolve()
    args.artefakte = args.artefakte.resolve()
    fehlt = nl.umgebung_pruefen(args.programm)
    if not fehlt and args.tor:
        if not args.pod_node or not args.myl:
            fehlt = "--tor braucht --pod-node und --myl"
        else:
            args.pod_node, args.myl = args.pod_node.resolve(), args.myl.resolve()
            fehlt = next((f"{p} fehlt" for p in (args.pod_node, args.myl) if not p.exists()), None)
    if not fehlt and not args.shard.exists():
        fehlt = f"{args.shard} fehlt"
    if not fehlt and not (args.artefakte / "weights_manifest.json").exists():
        fehlt = f"kein Artefakt unter {args.artefakte}"
    if fehlt:
        print(f"Umgebung fehlt: {fehlt}")
        return 2
    ordner = args.protokolle or pathlib.Path(tempfile.mkdtemp(prefix="myl-podlauf-"))
    ordner.mkdir(parents=True, exist_ok=True)
    melde(f"Protokolle unter {ordner}")
    rc = 0
    try:
        lauf(args, ordner)
        print("Fertig: bestanden", flush=True)
    except Befund as b:
        print(f"FEHLER: {b}", flush=True)
        rc = 1
    finally:
        # Alles weg, auch nach einem Abbruch: keine Prozesse, keine Namensraeume.
        for ns in nl.sh("ip", "netns", "list", pruefen=False).stdout.split():
            if not ns.startswith("myl-"):
                continue
            for pid in nl.sh("ip", "netns", "pids", ns, pruefen=False).stdout.split():
                try:
                    os.kill(int(pid), signal.SIGKILL)
                except (OSError, ValueError):
                    pass
            nl.sh("ip", "netns", "del", ns, pruefen=False)
        nl.bruecke_entfernen()
    if rc:
        for d in sorted(p for p in ordner.iterdir() if p.is_dir()):
            fehler = d / "stderr.txt"
            if fehler.exists():
                print(f"---- {d.name}: letzte Zeilen", flush=True)
                print("\n".join(fehler.read_text(errors="replace").splitlines()[-8:]), flush=True)
    return rc


if __name__ == "__main__":
    sys.exit(main())
