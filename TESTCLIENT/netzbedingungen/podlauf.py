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

Mit `--verweigerer` (2026-10-28) fuehrt ein Miner keinen Shard: Steht er
auf Position, beantwortet er jede Zeitprobe mit `Nichts`. Das zaehlt nach
einer langen Folge wie eine Ueberschreitung; verlangt ist seine Sperre nach
einer gehaltenen Position.

Mit `--zwillinge` (2026-10-28) dreizehn Miner, also zwei Pods: Der
Ortsdienst rechnet jeden Auftrag ueber beide, und die Kette verguetet nur,
was beide gleich gerechnet haben (Stufe 1). Verlangt ist mindestens ein
einiges Paar; der Bericht nennt einige und uneinige.

Mit `--zustandslos` (2026-10-29) haelt ein gewoehnlicher Miner (m08) keine
Konten: Er uebernimmt Bloecke nur mit ihren Zeugen, sagt zu, rechnet und wird
bezahlt; verlangt sind Bloecke mit Zeugen, eine Zusage und Lohn wie bei den
anderen. Dazu steigt ein Knoten ohne Miner-Rolle spaet zustandslos ein, mit
`--real` ueber einen leichten Abzug an einem Pruefpunkt; verlangt ist, dass
er die Spitze und dieselbe Wurzel erreicht. Der Erzeuger darf keinen Block
ohne Zeugen bauen. Dazu halten einige volle Miner Zeugen bereit
(2026-10-30, `--zeugen-bereithalten`): Der zustandslose Miner muss auch
dann folgen, wenn der Erzeuger endet; verlangt ist, dass sie gestartet sind
und keiner ihrer Zeugen fehlschlaegt.

Mit `--baum-auf-platte` (2026-10-30) lagern der Erzeuger und einige Miner
den Zustandsbaum auf die Platte aus; verlangt ist dieselbe Wurzel wie bei
allen anderen und belegte Seiten.

Mit `--faelscher` (2026-10-28, braucht `--zwillinge` und einen Shard mit
dem Merkmal `faelscher`) rechnet ein Miner falsch, aber in sich stimmig:
Jede Epoche, in der er auf Position stand und sein Pod Arbeit hatte, muss
als uneinig zaehlen, und kein einiges Paar darf dabei sein.

Mit `--paare` (2026-10-28, braucht `--zwillinge`) fuenfzehn Miner mehr, also
vier Pods und zwei Zwillingspaare: Der Ortsdienst lost jede Sitzung einem
Paar zu und bedient zugleich. Sechs Sitzungen fragen zugleich; verlangt
sind alle Antworten und dass beide Paare rechnen.

Mit `--nur tor,luegner,...` (2026-10-28) laufen nur diese Phasen, fuer
kurze, gezielte Laeufe; die Auswertung am Ende liest immer alles. Phasen:
`tor` (Wallet, Sitzung, Tuer, Uebernahme, Auszahlung), `luegner` (Position,
Sperre), `stoerung` (Abstuerze), `last`, `trennung`, `einsteiger`, `dauer`.

Noch nicht dabei: die Nachrechnung der Stichprobe.

Die letzte Zeile ist `Fertig: bestanden` oder `FEHLER: ...`; Rueckgabe 0, 1
(ein Befund) oder 2 (die Umgebung fehlt).
"""

from __future__ import annotations

import argparse
import collections
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
    "m08": "Asia", "m09": "Asia", "m10": "Asia", "m11": "NorthAmerica", "m12": "Asia", "m13": "Europe",
}
# ⚑ **Vier Validatoren statt eines**: der Erzeuger und je ein Miner aus jeder
#   Zone. Keiner haelt ein Drittel; ein Pruefpunkt braucht drei Stimmen.
VALIDATOREN = ["m01", "m06", "m09"]
STAKE = 1000
# Drei Miner mehr fuer zwei Pods (`--zwillinge`, 2026-10-28): Stufe 1 braucht
# Zwillinge, und erst zwoelf berechtigte Miner ergeben zwei Pods. 📌 Mit
# zwoelf genau fiel im ersten Lauf der zweite Pod weg, sobald einer gesperrt
# war, und die Tuer fragte gerade in den Sperrepochen des Verweigerers; drei
# mehr lassen eine Sperre Luft.
ZWILLINGSMINER = [
    ("m11", 44, 30_000),
    ("m12", 40, 35_000),
    ("m13", 48, 25_000),
]
# Der spaete Einsteiger: ein neuer Miner in Asien, mitten im Lauf.
EINSTEIGER = ("m10", 48, 40_000)
# ⚑ Fuer vier Pods, also zwei Zwillingspaare (`--paare`, 2026-10-28):
# Europa 14, Nordamerika 7, Asien 7. Die Zonen bilden ihre Pods getrennt
# (unter sechs teilen sie sich ein Sammelcluster); Europa traegt zwei Pods
# auch dann, wenn Verweigerer und Luegner gesperrt sind.
PAARMINER = [(f"m{i}", (40, 44, 48)[i % 3], 20_000 + 2_500 * (i % 7)) for i in range(14, 29)]
PAARZONE = {f"m{i}": ("Europe" if i < 22 else "NorthAmerica" if i < 25 else "Asia") for i in range(14, 29)}
MINERZONE.update(PAARZONE)
# Wie viele Sitzungen mit `--paare` zugleich fragen.
PAARSITZUNGEN = 6
# Der Einsatz je Miner in Kleinstbetraegen: 0,5 MYL.
EINSATZ = 500_000
# Der Verweigerer (`--verweigerer`, 2026-10-28): ein Miner, dessen Shard nie
# laeuft (`/bin/false`). Steht er auf Position, beantwortet er jede
# Zeitprobe mit `Nichts`; das muss wie eine Ueberschreitung zaehlen (Fund
# 660), und die Kette sperrt ihn.
VERWEIGERER = "m04"
# Der Faelscher (`--faelscher`, 2026-10-28): ein Miner, dessen Shard auf
# einer um eins verschobenen Eingabe rechnet (`MYL_SHARD_FAELSCHEN=1`, Shard
# mit dem Merkmal `faelscher`). Spur und Unterschrift stimmen; nur der
# Zwilling sieht eine andere Ausgabe.
FAELSCHER = "m07"
# ⚑ Zustandslos (`--zustandslos`, 2026-10-29): ein gewoehnlicher Miner ohne
#   Sonderrolle haelt keine Konten (er rechnet, reicht ein und wird bezahlt),
#   und ein spaeter Knoten ohne Miner-Rolle steigt ueber einen leichten Abzug
#   ein (mit `--real` ueber ein Pruefpunktzertifikat, sonst ab Genesis).
ZUSTANDSLOSER_MINER = "m08"
# Diese vollen Miner halten mit `--zustandslos` Zeugen bereit (2026-10-30):
# Die Pruefbarkeit haengt nicht am Erzeuger allein.
ZEUGEN_BEREIT = {"m01", "m04", "m10"}
# Mit `--baum-auf-platte` (2026-10-30) lagern der Erzeuger und diese Miner
# den Zustandsbaum aus.
BAUM_AUF_PLATTE = {"m00", "m02", "m05", "m09", "m12", "m14", "m16", "m18"}
# Die Phasen fuer `--nur` (2026-10-28): Ein kurzer Lauf prueft gezielt, was
# sich geaendert hat; der lange faehrt alle. Die Auswertung am Ende laeuft
# immer, sie liest nur.
PHASEN = ("tor", "luegner", "stoerung", "last", "trennung", "einsteiger", "zustandslos", "dauer")


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


# Je Datei und Art: wie weit gelesen, und was dabei gefunden wurde.
_GELESEN: dict[tuple[str, str], tuple[int, list[dict]]] = {}


def zeilen(k: Knoten, art: str) -> list[dict]:
    """Die Protokollzeilen einer Art, ohne alles zu zerlegen.

    📌 Fund 677 (Nachtlauf 2026-10-29): Bis hierher las jeder Aufruf jede
    Datei ganz, je Epoche rund 3 GB bei 30 Knoten, im selben Behaelter wie
    die gemessenen Shards, und mit den Protokollen wuchs die Last. Jetzt
    liest ein Aufruf nur, was seit dem letzten derselben Art dazukam, und
    nur bis zur letzten ganzen Zeile."""
    marke = f'"{art}"'
    aus = []
    for datei in sorted(k.ordner.glob("*.jsonl")):
        schluessel = (str(datei), art)
        stand, gefunden = _GELESEN.get(schluessel, (0, []))
        try:
            with open(datei, "rb") as f:
                if f.seek(0, os.SEEK_END) < stand:
                    # Die Datei wurde neu begonnen.
                    stand, gefunden = 0, []
                f.seek(stand)
                neu = f.read()
        except OSError:
            continue
        ende = neu.rfind(b"\n") + 1
        for z in neu[:ende].decode("utf-8", errors="replace").splitlines():
            if marke in z:
                try:
                    d = json.loads(z)
                except json.JSONDecodeError:
                    continue
                if d.get("art") == art:
                    gefunden.append(d)
        _GELESEN[schluessel] = (stand + ende, gefunden)
        aus.extend(gefunden)
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
    liste = MINER + (ZWILLINGSMINER if args.zwillinge else []) + (PAARMINER if args.paare else [])
    gesamt = 1 + len(liste)

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
        alle = [ortsknoten(0, None)] + [ortsknoten(i + 1, m[0]) for i, m in enumerate(liste)]
    else:
        alle = [Knoten(i, ordner, *stoerung(i)) for i in range(gesamt)]
    erzeuger = alle[0]
    miner = alle[1:]
    rolle = {k.name: m for k, m in zip(miner, liste)}
    # ⚑ **Befunde sammeln statt abbrechen** (mit `--real`): Ein Lauf ueber
    #   Nacht soll alles zeigen, was er findet, nicht nur das erste.
    befunde: list[str] = []
    bericht: list[str] = []
    # Die Epochen, in denen getrennt wurde (mit `--real`).
    trennungen: set[int] = set()
    luegner = next(k for k in miner if rolle[k.name][0] == "luegner")

    def verweigerer_auf_position(e: int) -> bool:
        """Steht der Verweigerer in Epoche `e` auf Position? Sein Knoten
        startet den Shard dann bei jeder Aufnahme neu."""
        return verweigerer is not None and any(int(z.get("epoche", -1)) == e
                                               for z in zeilen(verweigerer, "shard_gestartet"))
    verweigerer = next((k for k in miner if args.verweigerer and rolle[k.name][0] == VERWEIGERER), None)
    ehrlich = [k for k in miner if k is not luegner and k is not verweigerer]
    # ⚑ Vergleich der Speicherhaltung: jeder zweite Miner mit gedeckelten
    #   Arenen von glibc, die anderen wie ausgeliefert.
    if args.arenen:
        for k in miner[1::2]:
            k.umgebung["MALLOC_ARENA_MAX"] = str(args.arenen)
    faelscher = next((k for k in miner if args.faelscher and rolle[k.name][0] == FAELSCHER), None)
    if faelscher is not None:
        faelscher.umgebung["MYL_SHARD_FAELSCHEN"] = "1"
    # Die Epoche der gezielten Frage mit dem Faelscher auf Position.
    faelscher_frage: dict[str, int] = {}
    # Die Epochen der Fragen ueber die Tuer.
    frage_epochen: list[int] = []

    def faelscher_auf_position(e: int) -> bool:
        return faelscher is not None and any(int(z.get("epoche", -1)) == e for z in zeilen(faelscher, "shard_gestartet"))

    def faelscher_hat_gerechnet(e: int) -> bool:
        """Ob der Pod des Faelschers in Epoche `e` gerechnet hat: Mit mehreren
        Paaren (W7) rechnet nicht jedes in jeder Epoche, also zaehlt die Zeile
        des Ortsdienstes je Pod, nicht die Position allein."""
        import re
        z = [x for x in zeilen(faelscher, "shard_gestartet") if int(x.get("epoche", -1)) == e] if faelscher else []
        if not z:
            return False
        pod = int(z[-1].get("pod", -1))
        ort = (erzeuger.ordner / "ortsdienst.txt").read_text(errors="replace")
        return any(int(m.group(1)) == e and int(m.group(2)) == pod
                   for m in re.finditer(r"\[myl-pod\] Epoche (\d+), Pod (\d+):", ort))

    def epoche() -> int:
        return max(0, erzeuger.hoehe()) // EPOCHE

    def phase(name: str) -> bool:
        """Ob die Phase `name` laeuft (`--nur`); ohne Angabe alle."""
        return args.nur is None or name in args.nur

    zahler_adresse = None

    def angaben(k: Knoten) -> list[str]:
        name, ebenen, us = rolle[k.name]
        aus = [
            "--konsensschluessel", str(k.ordner / "konsens.key"),
            "--modellzusage", f"{ebenen}:{us}",
            "--zone", MINERZONE[name] if args.real else "Europe",
            "--shard-programm", "/bin/false" if k is verweigerer else str(args.shard),
            "--artefakte", str(args.artefakte),
            # ⚑ Jeder Miner hinterlegt 0,5 MYL (2026-10-29): Ohne Einsatz
            #   schlachtet ein Schiedsurteil nichts.
            "--einsatz", str(EINSATZ),
        ]
        if args.real:
            aus += ["--stimmsatz", str(ordner / "stimmsatz.txt")]
        if args.zustandslos and name == ZUSTANDSLOSER_MINER:
            aus += ["--zustandslos"]
        # ⚑ Jeder zweite Miner mit dem Zustandsbaum auf der Platte
        #   (`--baum-auf-platte`): Ihre Wurzeln muessen die der anderen sein.
        elif args.baum_auf_platte and name in BAUM_AUF_PLATTE:
            aus += ["--baum-auf-platte"]
        if args.zustandslos and name in ZEUGEN_BEREIT:
            aus += ["--zeugen-bereithalten"]
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
    if args.baum_auf_platte:
        zusatz_erzeuger += ["--baum-auf-platte"]
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
        # Der Verweigerer fuehrt keinen Shard; steht er auf Position, laufen
        # nur drei.
        soll = 4 if verweigerer is None else 3
        warte(lambda: len(stand()) >= soll, EPOCHE * takt / 1000 + 60, f"{soll} laufende Shards in Epoche {e}", alle)

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

    # ⚑ **W7, die Last ueber alle Paare** (`--paare`, 2026-10-28): Die
    #   Sitzungen paar0 bis paar5 fragen zugleich. Verlangt: alle beantwortet,
    #   und beide Paare haben gerechnet (im Ortsdienst je Auftrag eine Zeile
    #   je Pod, der rechnet). Im Dauerteil alle fuenf Epochen wieder.
    paare_stand = {"runden": 0, "beide": 0, "antworten": 0, "fragen": 0}

    def paare_pruefen(e_p: int) -> None:
        import concurrent.futures
        import re
        pods_melden(e_p)
        def eine(i: int) -> tuple[bool, float]:
            beginn = time.monotonic()
            a = myl("frage", "--tuer", TUER, "--agent", f"paar{i}", "--token", "4", "Hallo")
            return a.returncode == 0 and bool(a.stdout.strip()), time.monotonic() - beginn
        beginn = time.monotonic()
        with concurrent.futures.ThreadPoolExecutor(PAARSITZUNGEN) as ex:
            ergebnisse = list(ex.map(eine, range(PAARSITZUNGEN)))
        gesamt = time.monotonic() - beginn
        time.sleep(5)
        ort = (erzeuger.ordner / "ortsdienst.txt").read_text(errors="replace")
        gerechnet = sorted({int(m.group(2)) for m in re.finditer(r"\[myl-pod\] Epoche (\d+), Pod (\d+):", ort)
                            if int(m.group(1)) == e_p})
        sek = sorted(s for _, s in ergebnisse)
        beantwortet = sum(ok for ok, _ in ergebnisse)
        zeile = (f"Paare (W7) in Epoche {e_p}: {beantwortet} von {PAARSITZUNGEN} Sitzungen zugleich "
                 f"beantwortet in {gesamt:.1f} s (einzeln Median {sek[len(sek) // 2]:.1f} s, Summe {sum(sek):.1f} s); "
                 f"gerechnet haben die Pods {gerechnet}")
        melde(zeile)
        paare_stand["runden"] += 1
        paare_stand["beide"] += int(len(gerechnet) >= 4)
        paare_stand["antworten"] += beantwortet
        paare_stand["fragen"] += PAARSITZUNGEN
        if paare_stand["runden"] == 1:
            bericht.append(zeile)
            if beantwortet < PAARSITZUNGEN:
                befunde.append(f"nicht jede Sitzung bekam eine Antwort: {zeile}")
            if len(gerechnet) < 4:
                befunde.append(f"nicht beide Paare haben gerechnet: {zeile}")

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
        # ⚑ W7 (`--paare`): weitere Sitzungen, die spaeter zugleich fragen;
        #   jede wird einem Paar zugelost.
        if args.paare:
            # 📌 Eine nach der anderen: Das Wallet nimmt die Nonce aus dem
            #    bestaetigten Konto; sechs auf einmal trugen dieselbe, und nur
            #    eine kam an (erster Lauf mit `--paare`, 2026-10-28).
            for i in range(PAARSITZUNGEN):
                n0 = nonce()
                r = myl("wallet", "sitzung", "zahler", f"paar{i}", "1000", dauer, *relais_angaben)
                if r.returncode != 0:
                    raise Befund(f"Sitzung paar{i}: {r.stderr.strip()[-300:]}")
                warte(lambda: nonce() > n0, 120, f"die Sitzung paar{i} kommt in die Kette", alle)
            # Ihre Hinterlegungen bleiben bis zum Ende des Laufs stehen.
            if kurzsitzung:
                kurzsitzung["hinterlegt"] += PAARSITZUNGEN * HINTERLEGUNG_SITZUNG
            melde(f"{PAARSITZUNGEN} weitere Sitzungen offen (paar0 bis paar{PAARSITZUNGEN - 1})")
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
            # Steht der Verweigerer auf Position, wartet die Tuer auf die
            # Reserve (geprueft weiter unten). Dann die naechste Epoche,
            # hoechstens sechs.
            for _ in range(6):
                if not verweigerer_auf_position(epoche()):
                    break
                melde(f"Epoche {epoche()}: der Verweigerer steht auf Position, die Tuer wartet eine Epoche")
                in_epoche(epoche() + 1)
            if args.zwillinge:
                pods_melden(epoche())
            frage_epochen.append(epoche())
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
        # ⚑ Die Uebernahme durch die Reserve (2026-10-28, Whitepaper 6.8):
        #   In einer Epoche, in der der Verweigerer auf Position steht,
        #   besetzt der Ortsdienst seine Position aus der Reserve. Zweimal
        #   gefragt: Die erste Frage wartet auf den Shard der Reserve und
        #   kann an der Frist der Tuer scheitern, die zweite nicht mehr.
        if verweigerer is not None:
            for _ in range(8):
                in_epoche(epoche() + 1)
                if verweigerer_auf_position(epoche()):
                    break
            e_ue = epoche()
            if not verweigerer_auf_position(e_ue):
                bericht.append("Uebernahme nicht geprueft: der Verweigerer bekam in acht Epochen keine Position")
            else:
                antworten = []
                frage_epochen.append(e_ue)
                # 📌 Mit `--paare` gehoert jede Sitzung einem Paar; stand der
                #    Verweigerer im anderen, fragte ihn niemand, und es gab
                #    nichts zu uebernehmen (Nachtlauf 2026-10-29, Epoche 9:
                #    gefragt Pod 2 und 3, der Verweigerer in Pod 1). Jetzt
                #    fragen alle Sitzungen zugleich; gezaehlt wird, ob jede
                #    eine Antwort bekam.
                sitzungen = ["agent"] + ([f"paar{i}" for i in range(PAARSITZUNGEN)] if args.paare else [])
                for f in ("Der Himmel ist", "Hallo"):
                    import concurrent.futures
                    with concurrent.futures.ThreadPoolExecutor(len(sitzungen)) as ex:
                        rs = list(ex.map(lambda s: myl("frage", "--tuer", TUER, "--agent", s, "--token", "4", f),
                                         sitzungen))
                    antworten.append(all(r.returncode == 0 and bool(r.stdout.strip()) for r in rs))
                # Steht er im Pod des Zwillings, uebernimmt dessen Faden im
                # Hintergrund; darauf wird gewartet, nicht gezaehlt.
                def zahl() -> int:
                    return sum(1 for k in miner for z in zeilen(k, "position_uebernommen")
                               if int(z.get("epoche", -1)) == e_ue)
                ende_ue = time.monotonic() + 150
                while zahl() == 0 and time.monotonic() < ende_ue:
                    time.sleep(5)
                uebernommen = zahl()
                zeile = (f"Uebernahme durch die Reserve in Epoche {e_ue}: {uebernommen} Positionen uebernommen, "
                         f"Antworten {antworten}")
                melde(zeile)
                bericht.append(zeile)
                if uebernommen == 0 or not antworten[-1]:
                    befunde.append(f"die Reserve uebernahm die Position des Verweigerers nicht: {zeile}")
        # ⚑ Der Faelscher (`--faelscher`): eine Frage am Anfang einer Epoche,
        #   in der er auf Position steht. 📌 Sonst hing seine Pruefung am
        #   Zufall der Zuteilung: Im ersten gezielten Lauf stand er nur in der
        #   letzten Epoche, und deren Ergebnis zaehlt die Kette erst am
        #   naechsten Wechsel, nach dem Ende des Laufs (2026-10-28).
        schon = [e for e in frage_epochen if faelscher_hat_gerechnet(e)]
        if schon:
            # Er stand schon in einer Epoche mit Frage auf Position: keine
            # eigene Suche, gewartet wird auf die Zaehlung der letzten.
            faelscher_frage["epoche"] = max(schon)
        elif faelscher is not None:
            for _ in range(8):
                in_epoche(epoche() + 1)
                if faelscher_auf_position(epoche()) and not verweigerer_auf_position(epoche()):
                    break
            e_f = epoche()
            if faelscher_auf_position(e_f):
                # Mit mehreren Paaren fragen alle Sitzungen zugleich, sonst
                # traefe die eine Sitzung womoeglich das andere Paar.
                if args.paare:
                    paare_pruefen(e_f)
                else:
                    if args.zwillinge:
                        pods_melden(e_f)
                    r = myl("frage", "--tuer", TUER, "--agent", "agent", "--token", "4", "Zwei plus zwei")
                    melde(f"Frage mit dem Faelscher auf Position in Epoche {e_f}: "
                          + ("beantwortet" if r.returncode == 0 and r.stdout.strip() else "ohne Antwort"))
                faelscher_frage["epoche"] = e_f
                frage_epochen.append(e_f)
            else:
                bericht.append("Faelscher bekam in acht Epochen keine Position fuer eine Frage")
        # ⚑ W7 (`--paare`): sechs Sitzungen fragen zugleich, am Anfang einer
        #   Epoche ohne den Verweigerer auf Position.
        if args.paare:
            for _ in range(6):
                in_epoche(epoche() + 1)
                if not verweigerer_auf_position(epoche()):
                    break
            frage_epochen.append(epoche())
            paare_pruefen(epoche())
        # Das Buendel am Ende der Epoche, dann die Auszahlung danach.
        # 📌 Mit Zwillingen reichen zwei Pods ein; scheitert einer (der
        #    Luegner unterschreibt nicht), stand seine Meldung zuletzt und
        #    galt als Ergebnis (`gezielt20`). Ein eingereichtes genuegt.
        def buendel() -> str | None:
            text = (erzeuger.ordner / "ortsdienst.txt").read_text(errors="replace")
            treffer = [z for z in text.splitlines() if "Bündel" in z]
            eingereicht = [z for z in treffer if "eingereicht" in z]
            return (eingereicht or treffer)[-1] if treffer else None
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
        # ⚑ Keine Auszahlung ist richtig, wenn keine Gruppe einig war und
        #   entweder der Faelscher in jeder Epoche einer Frage sass (jede
        #   Gruppe uneinig) oder ein Pod allein rechnete (Anlaufregel: die
        #   Gutschrift wartet die Streitfrist ab, 2026-10-28).
        if not echt:
            a = (zeilen(erzeuger, "aufnahme") or [{}])[-1]
            einig_jetzt, zurueck = int(a.get("zwillinge_einig", 0)), int(a.get("zurueckgehalten", 0))
            nur_faelscher = bool(frage_epochen) and all(faelscher_auf_position(e) for e in frage_epochen)
            if einig_jetzt == 0 and (nur_faelscher or zurueck > 0):
                bericht.append(f"Keine Auszahlung, richtig: keine Gruppe einig; zurueckgehalten {zurueck}, "
                               f"Faelscher in jeder Frage-Epoche {nur_faelscher} ({frage_epochen})")
            else:
                raise Befund(f"kein Miner bekam eine Auszahlung fuer bezeugte Arbeit (einig {einig_jetzt}, zurueckgehalten {zurueck})")
        if abrechnungen == 0:
            raise Befund("die Tuer hat keine Abrechnung verbreitet")


    if args.tor and phase("tor"):
        in_epoche(1)
        empfaenger_pruefen()

    # ⚑ Phase `luegner`: seine Position, Meldung und Sperre.
    if phase("luegner"):
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

    r = random.Random(args.saat)
    if phase("stoerung"):
        shard_absturz()
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
        trennungen.add(epoche())
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
        trennungen.add(epoche())
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
        # 📌 Ein Einsteiger, der nicht ankommt, ist ein Befund und kein
        #    Abbruch (Nachtlauf vom 2026-10-28: nach 2,5 von 8 Stunden
        #    beendet, ohne Schlussbericht).
        try:
            oben = warte(lambda: holt_auf(k), 900, "der Einsteiger holt auf", alle)
            dabei = warte(lambda: bool(zeilen(k, "zusage_eingereicht")), 900, "der Einsteiger meldet sich an und sagt zu", alle)
        except Befund as b:
            if not args.real:
                raise
            befunde.append(f"Einsteiger {name}: {str(b)[:300]}, Probehahn leer {len(zeilen(k, 'probehahn_leer'))} mal")
            return
        zeile = (f"Einsteiger {name} (Asien) bei Hoehe {h0}: an der Spitze nach {oben:.0f} s, angemeldet und "
                 f"zugesagt nach weiteren {dabei:.0f} s, Probehahn {len(zeilen(k, 'probehahn'))} mal")
        melde(zeile)
        bericht.append(zeile)

    # Der spaete zustandslose Knoten (ohne Miner-Rolle), wenn gestartet.
    zustandslose: list[Knoten] = []

    def zustandsloser_einsteiger() -> None:
        """Ein Knoten ohne Miner-Rolle, zustandslos, mitten im Lauf: Er holt
        einen leichten Abzug (mit `--real` ueber ein Pruefpunktzertifikat
        gegen den Stimmsatz), rechnet mit Zeugen nach und haelt keine Konten."""
        k = (Ortsknoten(len(alle), ordner, "Europe", 5, 0.1, 100) if args.real
             else Knoten(len(alle), ordner, *stoerung(len(alle) % gesamt)))
        k.netz_anlegen()
        if args.real:
            for j in alle:
                j.ziel_eintragen(k)
                k.ziel_eintragen(j)
        extra = ["--zustandslos"] + (["--stimmsatz", str(ordner / "stimmsatz.txt")] if args.real else [])
        k.starten(args.programm, einstieg, False, takt, extra)
        k.warte_auf_peer_id(30)
        alle.append(k)
        zustandslose.append(k)
        h0 = erzeuger.hoehe()
        try:
            oben = warte(lambda: holt_auf(k), 900, "der zustandslose Einsteiger holt auf", alle)
        except Befund as b:
            if not args.real:
                raise
            befunde.append(f"zustandsloser Einsteiger {k.name}: {str(b)[:300]}")
            return
        leicht = zeilen(k, "abzug_uebernommen")
        zeile = (f"Zustandsloser Einsteiger {k.name} bei Hoehe {h0}: an der Spitze nach {oben:.0f} s, "
                 + (f"leichter Abzug bei {leicht[0].get('hoehe')} ({leicht[0].get('bytes')} Bytes)" if leicht else "ab Genesis"))
        melde(zeile)
        bericht.append(zeile)
        if args.real and not leicht:
            befunde.append(f"zustandsloser Einsteiger {k.name}: kein leichter Abzug, obwohl ein Stimmsatz gilt")

    def last() -> None:
        """Burns in Stufen ueber die Relaisleitung des Torknotens: Wie viel
        nimmt die Kette auf, und wie antwortet der Grundpreis?"""
        stufen, sekunden = ("50,200", "20") if args.kurz else ("50,100,200,400,800,1600", "60")
        if args.laststufen:
            stufen, sekunden = args.laststufen, str(args.lastdauer)
        r = subprocess.run(["ip", "netns", "exec", erzeuger.ns, str(args.lastgeber), RELAIS,
                            str(erzeuger.ordner / "relais.ausweis"), "100", stufen, sekunden],
                           capture_output=True, text=True, timeout=3600)
        gedeckelt = False
        for z in r.stdout.splitlines():
            if z.startswith("[last]"):
                melde(z)
                bericht.append(z)
                # 📌 Eine Stufe ohne Ausstattung misst nichts (Nachtlauf vom
                #    2026-10-28: ab 400/s null aufgenommen, und keiner sagte es).
                if z.startswith("[last] Ausstattung gedeckelt: 0 "):
                    befunde.append(f"Laststufe ohne Ausstattung: {z}")
                # Gedeckelt zählt erst unter 90 % (📌 `durchsatz4`: 99,8 % galt
                # als gedeckelt und verdeckte einen Verlust).
                if z.startswith("[last] Ausstattung gedeckelt"):
                    zahlen = [int(w) for w in z.replace(":", " ").split() if w.isdigit()]
                    gedeckelt = len(zahlen) >= 2 and 10 * zahlen[0] < 9 * zahlen[1]
                # ⚑ Was angenommen wurde, muss in einen Block (2026-10-29),
                #   ausser die Stufe war mangels Guthaben gedeckelt: Dann
                #   scheitern Transaktionen erst beim Anwenden.
                if z.startswith("[last] stufe="):
                    felder = dict(f.split("=", 1) for f in z.split()[1:] if "=" in f)
                    if int(felder.get("verloren", 0)) and not gedeckelt:
                        befunde.append(f"angenommen, aber nicht in einem Block: {z}")
                    gedeckelt = False
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
        if args.tor and args.lastgeber and phase("last"):
            last()
        if phase("trennung"):
            trennung(["Asia"], 60)
            trennung(["NorthAmerica", "Asia"], 60)
        if args.kurz and phase("einsteiger"):
            einsteiger()
        if args.kurz and args.zustandslos and phase("zustandslos"):
            # Der leichte Abzug braucht einen Pruefpunkt mit Zertifikat.
            warte(lambda: erzeuger.hoehe() >= 400, 3600, "zwei Pruefpunkte fuer den leichten Abzug", alle)
            zustandsloser_einsteiger()

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

    ziel_epoche = epoche() + (0 if not phase("dauer") else 1 if args.kurz else args.epochen)
    letzte = epoche()
    # ⚑ **Verkehr im Dauerteil** (mit `--tor`): je Epoche eine Frage des
    #   Agenten ueber die Tuer, vor den Abstuerzen dieser Epoche. Gezaehlt
    #   wird, wie viele beantwortet wurden; Abstuerze und Stoerer duerfen
    #   einzelne kosten, nicht die meisten.
    verkehr = {"gefragt": 0, "beantwortet": 0, "sekunden": [], "mit_verweigerer": 0, "mit_verweigerer_beantwortet": 0}
    def dauerfrage(e: int) -> None:
        f = ["Der Himmel ist", "Zwei plus zwei", "Hallo"][e % 3]
        beginn = time.monotonic()
        a = myl("frage", "--tuer", TUER, "--agent", "agent", "--token", "4", f)
        # Mit dem Verweigerer auf Position muss die Reserve erst
        # uebernehmen: getrennt gezaehlt, damit die Pruefung der Tuer nicht
        # daran haengt.
        if verweigerer_auf_position(e):
            verkehr["mit_verweigerer"] += 1
            verkehr["mit_verweigerer_beantwortet"] += int(a.returncode == 0 and bool(a.stdout.strip()))
            return
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
        if args.zustandslos and e == platz_anfang[0] + 22:
            zustandsloser_einsteiger()
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
            if args.tor and args.paare and not args.kurz and e % 5 == 0 and not verweigerer_auf_position(e):
                paare_pruefen(e)
        time.sleep(5)
    speicher_ablegen(epoche())
    if args.tor and not args.kurz:
        sek = sorted(verkehr["sekunden"])
        mitte = f", Median {sek[len(sek) // 2]:.0f} s" if sek else ""
        abrechnungen = sum(int(z.get("anzahl", 0)) for z in zeilen(erzeuger, "abrechnungen_verbreitet"))
        if verkehr["mit_verweigerer"]:
            zeile = (f"Fragen in Epochen mit dem Verweigerer auf Position: {verkehr['mit_verweigerer_beantwortet']} "
                     f"von {verkehr['mit_verweigerer']} beantwortet (ohne Uebernahme durch die Reserve)")
            melde(zeile)
            bericht.append(zeile)
        melde(f"Verkehr: {verkehr['beantwortet']} von {verkehr['gefragt']} Fragen beantwortet{mitte}; "
              f"Abrechnungen verbreitet insgesamt: {abrechnungen}")
        if verkehr["gefragt"] and 2 * verkehr["beantwortet"] < verkehr["gefragt"]:
            text = f"nur {verkehr['beantwortet']} von {verkehr['gefragt']} Fragen im Dauerteil beantwortet"
            if not args.real:
                raise Befund(text)
            befunde.append(text)
    if paare_stand["runden"] > 1:
        zeile = (f"Paare (W7) ueber den Lauf: {paare_stand['runden']} Runden, in {paare_stand['beide']} rechneten beide Paare; "
                 f"{paare_stand['antworten']} von {paare_stand['fragen']} Fragen beantwortet")
        melde(zeile)
        bericht.append(zeile)
        # Abstuerze und Stoerer duerfen einzelne kosten, nicht die meisten.
        if 4 * paare_stand["antworten"] < 3 * paare_stand["fragen"]:
            befunde.append(f"zu wenige Antworten bei zugleich fragenden Sitzungen: {zeile}")
        if 2 * paare_stand["beide"] < paare_stand["runden"]:
            befunde.append(f"meist rechnete nur ein Paar: {zeile}")
    if stoerer:
        stoerer.beenden()

    # ---- Die Messplanung der Zeitprobe ---------------------------------------
    def zeitproben_diagnose() -> None:
        """Wie viele Messungen je Epoche gestellt wurden und wie sie endeten;
        wer den Luegner in welcher Epoche gemessen hat, mit welchem Urteil
        (Funde 664 und 667: ob die Zeit einer Epoche fuer eine Mehrheit von
        Zeugnissen reicht); Zeugnisse gegen Ehrliche."""
        gestellt = [(k, z) for k in ehrlich for z in zeilen(k, "fristmessung_gestellt")]
        fertig = [(k, z) for k in ehrlich for z in zeilen(k, "fristmessung")]
        abgewiesen = [(k, z) for k in ehrlich for z in zeilen(k, "zeitprobe_abgewiesen")]
        verworfen = sum(len(zeilen(k, a)) for k in ehrlich for a in ("fristmessung_verfallen", "fristmessung_verloren"))
        je_epoche = collections.Counter(int(z.get("epoche", -1)) for _, z in gestellt)
        if je_epoche:
            w = sorted(je_epoche.values())
            enden = dict(collections.Counter(str(z.get("ende", "?")) for _, z in fertig))
            bericht.append(f"Zeitproben: {len(gestellt)} Messungen in {len(w)} Epochen gestellt (je Epoche "
                           f"min {w[0]}, Median {w[len(w) // 2]}, max {w[-1]}), beendet {len(fertig)} {enden}, "
                           f"Abweisungen {len(abgewiesen)}, verfallen oder verloren {verworfen}")
        urteil = lambda z: str(z.get("urteil", "?")).split("(")[0]  # noqa: E731
        kennung = next((z.get("mitglied") for _, z in fertig if int(z.get("frist_us", 10**9)) < 1000), None)
        # Der Verweigerer: wer am haeufigsten mit Ende `verweigert` gemessen
        # wurde, ausser dem Luegner (seine Kennung steht in keinem Protokoll).
        abweiser = collections.Counter(z.get("mitglied") for _, z in fertig
                                       if z.get("ende") == "verweigert" and z.get("mitglied") != kennung)
        verweigerer_kennung = abweiser.most_common(1)[0][0] if verweigerer is not None and abweiser else None
        if verweigerer_kennung:
            gegen = [z for _, z in fertig if z.get("mitglied") == verweigerer_kennung]
            bericht.append(f"Gegen den Verweigerer: {len(gegen)} Messungen, "
                           f"{sum(1 for z in gegen if urteil(z) == 'Zeugnis')} Zeugnisse, "
                           f"Epochen mit Zeugnis {sorted({int(z.get('epoche', -1)) for z in gegen if urteil(z) == 'Zeugnis'})}")
        gegen_ehrliche = [z for _, z in fertig if z.get("mitglied") not in (kennung, verweigerer_kennung)
                          and urteil(z) == "Zeugnis"]
        bericht.append(f"Zeugnisse gegen Ehrliche: {len(gegen_ehrliche)}, davon nach Verweigerung "
                       f"{sum(1 for z in gegen_ehrliche if z.get('ende') == 'verweigert')}")
        # 📌 Fund 674: Ein Waehlfehler zaehlte wie eine Weigerung, und mit 29
        #    Knoten bekamen Ehrliche Zeugnisse; das war nur berichtet, kein
        #    Befund. Jetzt ist es einer, und die nicht erreichten stehen dabei.
        unerreicht = sum(len(zeilen(k, "zeitprobe_unerreicht")) for k in ehrlich)
        bericht.append(f"Zeitproben nicht erreicht (Waehlfehler, nicht gezaehlt): {unerreicht}")
        if gegen_ehrliche:
            befunde.append(f"Zeugnisse gegen Ehrliche: {len(gegen_ehrliche)} "
                           f"({sorted({str(z.get('mitglied'))[:12] for z in gegen_ehrliche})})")
        positionen = sorted({int(z.get("epoche", -1)) for z in zeilen(luegner, "shard_gestartet")})
        if kennung is None:
            bericht.append(f"Gegen den Luegner: keine beendete Messung (Positionen in Epochen {positionen})")
            return
        je: dict[str, list[str]] = {}
        for k, z in gestellt:
            if z.get("mitglied") == kennung:
                je.setdefault(rolle[k.name][0], [])
        for k, z in fertig:
            if z.get("mitglied") == kennung:
                je.setdefault(rolle[k.name][0], []).append(f"e{z.get('epoche')} {urteil(z)}/{z.get('ende')}")
        ab = collections.Counter(rolle[k.name][0] for k, z in abgewiesen if z.get("mitglied") == kennung)
        bericht.append(f"Gegen den Luegner (Positionen in Epochen {positionen}): " + "; ".join(
            f"{n} [{', '.join(v) or 'nichts beendet'}; {ab.get(n, 0)} abgewiesen]" for n, v in sorted(je.items())))

    zeitproben_diagnose()

    # ---- Stufe 1: Zwillinge (`--zwillinge`) --------------------------------
    # Gezaehlt wird am Wechsel: auf eine Aufnahme nach der letzten Epoche
    # mit dem Faelscher und einer Frage warten, sonst fehlt genau sie (auch
    # in der Gesamtzeile).
    if "epoche" in faelscher_frage:
        nach = faelscher_frage["epoche"] + 1
        warte(lambda: any(int(z.get("hoehe", 0)) // EPOCHE >= nach and "zwillinge_einig" in z
                          for z in zeilen(erzeuger, "aufnahme")),
              2 * EPOCHE * takt / 1000 + 120, f"die Zaehlung der Epoche {nach - 1}", alle)
    letzte = (zeilen(erzeuger, "aufnahme") or [{}])[-1]
    einig, uneinig = int(letzte.get("zwillinge_einig", 0)), int(letzte.get("zwillinge_uneinig", 0))
    zeile = (f"Zwillinge: {einig} Gruppen einig (alle verguetet), {uneinig} uneinig oder unvollstaendig; "
             f"zurueckgehaltene Gutschriften (ein Pod allein) {letzte.get('zurueckgehalten')}; "
             f"Credit-Preis zuletzt {letzte.get('credit_preis')}")
    melde(zeile)
    bericht.append(zeile)
    # ⚑ Ohne `--zwillinge` rechnet ein Pod allein: Seine Gutschriften warten
    #   die Streitfrist ab (Anlaufregel, 2026-10-28), also muss nach Fragen
    #   etwas zurueckgehalten sein, und vergütet wurde keine Gruppe.
    if not args.zwillinge and frage_epochen and int(letzte.get("zurueckgehalten", 0)) == 0:
        befunde.append(f"ein Pod allein, aber nichts zurueckgehalten: {zeile}")
    # 📌 Auch der Luegner macht seine Gruppe unvollstaendig (sein Pod
    #    unterschreibt das Buendel nicht); mit 13 Minern gibt es ein Paar je
    #    Epoche, und in `gezielt17` bis `gezielt23` sassen beide oft darin.
    luegner_epochen = {int(z.get("epoche", -1)) for z in zeilen(luegner, "shard_gestartet")} if luegner else set()
    nur_mit_faelscher = bool(frage_epochen) and all(faelscher_auf_position(e) or e in luegner_epochen for e in frage_epochen)
    if args.zwillinge and einig == 0 and (phase("tor") or phase("dauer")) and not nur_mit_faelscher:
        befunde.append("kein Zwillingspaar war einig: Stufe 1 hat nichts verguetet")
    # ⚑ Der Faelscher (`--faelscher`): je Epoche der Zuwachs der Zaehler.
    #   Gezaehlt wird am Wechsel, also traegt der Stand am Ende von e+1 das
    #   Ergebnis von e.
    if faelscher is not None:
        stand: dict[int, tuple[int, int]] = {}
        for z in zeilen(erzeuger, "aufnahme"):
            if "zwillinge_einig" in z:
                stand[int(z.get("hoehe", 0)) // EPOCHE] = (int(z["zwillinge_einig"]), int(z["zwillinge_uneinig"]))
        je_epoche = {e: (stand[e + 1][0] - stand[e][0], stand[e + 1][1] - stand[e][1]) for e in stand if e + 1 in stand}
        auf_position = sorted({int(z.get("epoche", -1)) for z in zeilen(faelscher, "shard_gestartet")})
        # ⚑ Mit Arbeit heisst: sein Pod hat gerechnet (W7, mehrere Paare).
        mit_arbeit = {e: je_epoche[e] for e in auf_position if e in je_epoche and faelscher_hat_gerechnet(e)}
        zeile = (f"Faelscher {rolle[faelscher.name][0]}: auf Position in Epochen {auf_position}; "
                 f"sein Pod rechnete (einig, uneinig): {mit_arbeit}")
        melde(zeile)
        bericht.append(zeile)
        if "epoche" in faelscher_frage and faelscher_frage["epoche"] not in mit_arbeit:
            bericht.append(f"die Frage mit dem Faelscher in Epoche {faelscher_frage['epoche']} traf seinen Pod nicht")
        if not mit_arbeit:
            bericht.append("Faelscher nicht geprueft: keine Epoche, in der sein Pod rechnete")
        # Sein Paar muss uneinig sein; ein anderes Paar darf einig sein.
        nicht_erkannt = sorted(e for e, (_, u) in mit_arbeit.items() if u == 0)
        if nicht_erkannt:
            befunde.append(f"das Paar mit dem Faelscher war nicht uneinig, Epochen {nicht_erkannt}: {zeile}")

    # ---- Der Verweigerer (`--verweigerer`) ----------------------------------
    if verweigerer is not None:
        positionen = sorted({int(z.get("epoche", -1)) for z in zeilen(verweigerer, "shard_gestartet")})
        sperren = sorted({int(z.get("epoche", -1)) for z in zeilen(verweigerer, "fristsperre")})
        verweigert = sum(1 for k in ehrlich for z in zeilen(k, "fristmessung") if z.get("ende") == "verweigert")
        zeile = (f"Verweigerer {verweigerer.name}: auf Position in Epochen {positionen}, gesperrt fuer Epochen "
                 f"{sperren}; Messungen mit Ende verweigert (alle Gemessenen): {verweigert}")
        melde(zeile)
        bericht.append(zeile)
        # Verlangt ist eine Sperre nach einer Position, die er mindestens eine
        # ganze Epoche hielt (die letzte Epoche des Laufs zaehlt nicht).
        gehalten = [e for e in positionen if e < epoche()]
        if gehalten and not any(e - 1 in gehalten for e in sperren):
            befunde.append(f"der Verweigerer wurde nie gesperrt (Positionen {gehalten}, Sperren {sperren})")

    # ---- Kein ehrlicher Miner gesperrt -----------------------------------
    # 📌 Fund 667: Eine Sperre fuer die Epoche nach einer Trennung ist
    #    erwartet. Wer die Mehrheit seines Pods ueber zwei Fenster nicht
    #    erreicht, verpasst eine Epoche; mit 108 s je Epoche deckt eine
    #    Trennung von 60 s beide Fenster (im Nachtlauf vom 2026-10-27 zweimal).
    erwartet = []
    falsch = []
    for k in ehrlich:
        epochen = sorted(int(z.get("epoche", -1)) for z in zeilen(k, "fristsperre"))
        nach_trennung = [e for e in epochen if e - 1 in trennungen]
        sonst = [e for e in epochen if e - 1 not in trennungen]
        if nach_trennung:
            erwartet.append(f"{rolle[k.name][0]} (Epochen {nach_trennung})")
        if sonst:
            falsch.append(f"{rolle[k.name][0]} (Epochen {sonst})")
    if erwartet:
        bericht.append(f"Sperren nach Trennungen, erwartet: {', '.join(erwartet)}")
    if falsch:
        if not args.real:
            raise Befund(f"ehrliche Miner gesperrt: {', '.join(falsch)}")
        befunde.append(f"ehrliche Miner gesperrt: {', '.join(falsch)}")

    # 📌 Abgeschlossen wird erst eine halbe Epoche nach der Ziehung (zwei
    #    Fenster zu je einer Viertelepoche), entschieden ein Streitfall
    #    hoechstens eine Epoche danach. In den ersten Laeufen damit endete der
    #    Kurzlauf vorher, und das Warten stand hinter dem Beenden der Knoten.
    #    Jetzt wird vorher gewartet: auf den Abschluss jedes aufgedeckten
    #    Auftrags und auf die Entscheidung jedes Streitfalls.
    # Je nachgerechneter Ziehung ein Abschluss (📌 `gezielt18`: Gewartet
    # wurde nur auf den ersten, und der Streitfall der zweiten fiel heraus).
    nachgerechnete = {(z.get("epoche"), z.get("pod"), z.get("segment"))
                      for k in miner for z in zeilen(k, "stichprobe_nachgerechnet") if z.get("ergebnis") != "fehler"}
    if nachgerechnete and sum(int(z.get("ziehungen", 0)) for k in miner for z in zeilen(k, "pruefung_aufgedeckt")):
        def alle_abgeschlossen() -> bool:
            fertig = {(z.get("epoche"), z.get("pod"), z.get("segment")) for z in zeilen(erzeuger, "pruefung_abgeschlossen")}
            return nachgerechnete <= fertig
        try:
            warte(alle_abgeschlossen, EPOCHE * takt / 1000 + 120, "den Abschluss der Pruefauftraege", alle)
            if any(int(z.get("abweichungen", 0)) for z in zeilen(erzeuger, "pruefung_abgeschlossen")):
                warte(lambda: len(zeilen(erzeuger, "streitfall_entschieden")) > 0,
                      2 * EPOCHE * takt / 1000 + 120, "die Entscheidung der Streitfaelle", alle)
            # ⚑ Die Rueckbuchung (Whitepaper 6.4): Ist ein Pod eines
            #   uneinigen Paars ueberfuehrt, zahlt der naechste Wechsel den
            #   Zwilling aus.
            if any(z.get("ausgang") == "podfalsch" for z in zeilen(erzeuger, "streitfall_entschieden")) and \
                    zeilen(erzeuger, "differenz_gemeldet") + [z for k in miner for z in zeilen(k, "differenz_gemeldet")]:
                warte(lambda: int((zeilen(erzeuger, "aufnahme") or [{}])[-1].get("rueckbuchungen", 0)) > 0,
                      EPOCHE * takt / 1000 + 120, "die Rueckbuchung an den ehrlichen Zwilling", alle)
        except Befund as fehlt:
            befunde.append(str(fehlt))

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
    # Die zustandslosen Einsteiger muessen dieselbe Hoehe erreichen.
    for k in zustandslose:
        try:
            warte(lambda: k.hoehe() == hoehe, 120, f"{k.name} (zustandslos) auf Hoehe {hoehe}", [k])
        except Befund as b:
            befunde.append(str(b)[:300])
    # Der Speicher vor dem Beenden (danach gibt es keinen Prozess mehr).
    # ⚑ Anonym, nicht RSS (2026-10-30): Wer nachrechnet, hat das Modell
    #   gemappt; im Kurzlauf waren das 751 MB dateigestuetzte Seiten, die
    #   nichts ueber den Knoten sagen.
    speicherbilder = {k.name: (k.speicherbild() or {}) for k in miner + zustandslose}
    rss_am_ende = {n: b.get("rss", 0) for n, b in speicherbilder.items()}
    anon_am_ende = {n: b.get("anon", 0) for n, b in speicherbilder.items()}
    for k in miner + zustandslose:
        k.beenden()
    wurzeln = {}
    for k in miner + [z for z in zustandslose if z.hoehe() == hoehe]:
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
          f"angekommen {zaehlung['spuranfrage_eingegangen']}; mit Checker "
          f"{sum(1 for z in gezogen if z.get('pruefer'))} von {len(gezogen)}")
    # ⚑ Stufe 2, die Lieferung (2026-10-28): Wer gefragt wurde, antwortet aus
    #   seiner Ablage mit Spur und Beweis; der Erzeuger prueft gegen die Wurzel
    #   des Buendels aus der Kette.
    mit_spur = sum(1 for k in miner for z in zeilen(k, "spuranfrage_eingegangen") if z.get("antwort") == "spur")
    geprueft = collections.Counter(str(z.get("ergebnis")) for z in zeilen(erzeuger, "spur_geprueft"))
    zeile = f"Spuren: {mit_spur} Antworten mit Spur, beim Erzeuger geprueft {dict(geprueft)}"
    melde(zeile)
    bericht.append(zeile)
    if zaehlung["spuranfrage_eingegangen"] and geprueft.get("traegt", 0) == 0:
        befunde.append(f"keine gelieferte Spur trug: {zeile}")
    falsch = {e: n for e, n in geprueft.items() if e not in ("traegt", "ohne Wurzel")}
    if falsch:
        befunde.append(f"gelieferte Spuren trugen nicht: {falsch}")
    # ⚑ Stufe 2, Nachrechnen (2026-10-29): Der zugeteilte Checker, ein Miner
    #   ohne Pod in der gezogenen Epoche, holt den Verlauf und rechnet ihn ab
    #   den Tokens nach. Ehrliche Pods sind gleich; eine Abweichung darf nur
    #   den Faelscher treffen.
    zugeteilt = [z for k in miner for z in zeilen(k, "pruefer_zugeteilt")]
    nachgerechnet = [z for k in miner for z in zeilen(k, "stichprobe_nachgerechnet")]
    ergebnisse = collections.Counter(str(z.get("ergebnis")) for z in nachgerechnet)
    abgewiesen = [z for k in miner for z in zeilen(k, "verlauf_abgewiesen")]
    name_von = {k.peer_id: k.name for k in alle if getattr(k, "peer_id", None)}
    beschuldigt = collections.Counter(name_von.get(str(z.get("beschuldigt")), str(z.get("beschuldigt")))
                                      for z in nachgerechnet if z.get("ergebnis") == "abweichung")
    dauer = sorted(int(z.get("dauer_ms", 0)) for z in nachgerechnet)
    zeile = (f"Nachrechnen: {sum(int(z.get('ziehungen', 0)) for z in zugeteilt)} Ziehungen zugeteilt, "
             f"{len(nachgerechnet)} nachgerechnet {dict(ergebnisse)}, beschuldigt {dict(beschuldigt)}, "
             f"Verlauf abgewiesen {len(abgewiesen)}, Positionen hoechstens "
             f"{max((int(z.get('positionen', 0)) for z in nachgerechnet), default=0)}, "
             f"Dauer Median {dauer[len(dauer) // 2] if dauer else 0} ms")
    melde(zeile)
    bericht.append(zeile)
    # 📌 Der erste Lauf mit Nachrechnen (2026-10-29) zog viermal, teilte
    #    nichts zu und hiess trotzdem bestanden: Nur der Erzeuger kannte die
    #    Ziehung. Jede Aufstellung der Podlaeufe laesst Miner ohne Pod uebrig,
    #    also ist eine Ziehung ohne Checker ein Befund.
    if gezogen and not zugeteilt:
        befunde.append(f"gezogen, aber kein Checker zugeteilt: {zeile}")
    if zugeteilt and not nachgerechnet:
        befunde.append(f"zugeteilt, aber nichts nachgerechnet: {zeile}")
    if ergebnisse.get("fehler"):
        gruende = collections.Counter(str(z.get("grund"))[:80] for z in nachgerechnet if z.get("ergebnis") == "fehler")
        befunde.append(f"Nachrechnen scheiterte: {dict(gruende)}")
    if abgewiesen:
        befunde.append(f"Verlauf abgewiesen: {collections.Counter(str(z.get('grund'))[:60] for z in abgewiesen)}")
    # ⚑ Festlegen vor Aufdecken, auf der Kette (2026-10-29): Der Erzeuger
    #   protokolliert jeden abgeschlossenen Auftrag.
    abgeschlossen = zeilen(erzeuger, "pruefung_abgeschlossen")
    ausgaenge = collections.Counter(str(z.get("ausgang")) for z in abgeschlossen)
    zeile = (f"Pruefauftraege: festgelegt {sum(int(z.get('ziehungen', 0)) for k in miner for z in zeilen(k, 'pruefung_festgelegt'))}, "
             f"aufgedeckt {sum(int(z.get('ziehungen', 0)) for k in miner for z in zeilen(k, 'pruefung_aufgedeckt'))}, "
             f"abgeschlossen {dict(ausgaenge)}, mit Abweichung {sum(1 for z in abgeschlossen if int(z.get('abweichungen', 0)))}, "
             f"Differenzen gemeldet {sum(len(zeilen(k, 'differenz_gemeldet')) for k in miner)}")
    melde(zeile)
    bericht.append(zeile)
    if nachgerechnet and not ausgaenge.get("einig"):
        befunde.append(f"nachgerechnet, aber kein Auftrag schloss einig ab: {zeile}")
    # ⚑ Die Schiedsrunde (T6d, 2026-10-29): Streitfaelle, Stimmen, Urteile.
    #   Geschlachtet werden darf nur der Faelscher; eine verlorene Anfechtung
    #   eines ehrlichen Checkers ist ebenso ein Befund.
    urteile = zeilen(erzeuger, "streitfall_entschieden")
    stimmen = [z for k in miner for z in zeilen(k, "schiedsstimme")]
    gegen = collections.Counter((str(z.get("ausgang")), name_von.get(str(z.get("beschuldigt")), str(z.get("beschuldigt"))[:12]))
                                for z in urteile)
    zeile = (f"Schiedsrunde: {len(stimmen)} Stimmen ({sum(1 for z in stimmen if z.get('pod_falsch'))} Pod falsch), "
             f"Urteile {dict(gegen)}, Buchung {dict(collections.Counter(str(z.get('buchung')) for z in urteile))}, "
             f"Rueckbuchungen {(zeilen(erzeuger, 'aufnahme') or [{}])[-1].get('rueckbuchungen')}, "
             f"Einsatz hinterlegt {sum(len(zeilen(k, 'einsatz_hinterlegt')) for k in miner)} von {len(miner)}")
    melde(zeile)
    bericht.append(zeile)
    for (ausgang, wer), n in gegen.items():
        if ausgang == "podfalsch" and (faelscher is None or wer != faelscher.name):
            befunde.append(f"die Schiedsrunde schlachtete einen Ehrlichen: {wer} ({n} mal)")
        if ausgang == "anfechtungfalsch":
            befunde.append(f"eine Anfechtung verlor ({n} mal, Beschuldigter {wer}): ehrliche Checker irren nicht")
    ehrlich_beschuldigt = {n: c for n, c in beschuldigt.items() if faelscher is None or n != faelscher.name}
    if ehrlich_beschuldigt:
        befunde.append(f"Nachrechnen beschuldigt Ehrliche: {ehrlich_beschuldigt}")
    # ⚑ **Zustandslos** (2026-10-29): Folgt der zustandslose Miner mit Zeugen,
    #   reicht er seine Arbeit ein, wird er bezahlt, und was belegt er?
    if args.zustandslos:
        zl = next((k for k in miner if rolle[k.name][0] == ZUSTANDSLOSER_MINER), None)
        if zl is not None:
            mit_zeuge = sum(1 for z in zeilen(zl, "block_uebernommen") if z.get("mit_zeuge"))
            abgelehnt = collections.Counter(str(z.get("grund"))[:40] for z in zeilen(zl, "block_abgelehnt"))
            auf = [z for z in zeilen(zl, "aufnahme") if "auszahlung_guthaben" in z or "konto_guthaben" in z]
            feld = "auszahlung_guthaben" if any("auszahlung_guthaben" in z for z in auf) else "konto_guthaben"
            g0 = int(auf[0].get(feld, 0)) if auf else 0
            g1 = int(auf[-1].get(feld, 0)) if auf else 0
            vergleich = [int(z.get(feld, 0)) for k in miner if k is not zl and k in ehrlich
                         for z in zeilen(k, "aufnahme")[-1:]]
            bild = {"anon": anon_am_ende.get(zl.name, 0), "rss": rss_am_ende.get(zl.name, 0)}
            andere = [anon_am_ende.get(k.name, 0) for k in miner if k is not zl and anon_am_ende.get(k.name)]
            gr = zeilen(zl, "groessen")
            zeile = (f"Zustandsloser Miner {zl.name}: {mit_zeuge} Bloecke mit Zeugen, abgelehnt {dict(abgelehnt)}, "
                     f"zugesagt {len(zeilen(zl, 'zusage_eingereicht'))}, {feld} {g0} -> {g1} "
                     f"(die anderen zuletzt im Mittel {sum(vergleich) // max(1, len(vergleich))}), "
                     f"Konten im Zustand {gr[-1].get('konten') if gr else '?'}, "
                     f"anonym {bild.get('anon', 0) // 1024} MB gegen im Mittel {sum(andere) // max(1, len(andere)) // 1024} MB "
                     f"(RSS {bild.get('rss', 0) // 1024} MB)")
            melde(zeile)
            bericht.append(zeile)
            if mit_zeuge == 0:
                befunde.append(f"der zustandslose Miner {zl.name} uebernahm keinen Block mit Zeugen")
            if not zeilen(zl, "zusage_eingereicht"):
                befunde.append(f"der zustandslose Miner {zl.name} reichte keine Zusage ein")
            if g1 <= g0 and any(v > g0 for v in vergleich):
                befunde.append(f"der zustandslose Miner {zl.name} wurde nicht bezahlt ({feld} {g0} -> {g1})")
        # ⚑ **Folgt er am Ende mit Zeugen oder nur ueber leichte Abzuege?**
        #   (2026-10-30) 📌 Im langen Lauf `nacht-bereit` sprang der
        #   zustandslose Miner eine Stunde lang alle 30 s per leichtem Abzug
        #   und pruefte keinen Block; der Bericht sah es nicht. Dazu die
        #   Groesse der fertigen Zeugen (sie wuchs dort auf 526 KB im Median).
        for k in [z for z in [zl] if z is not None] + zustandslose:
            anker = zeilen(k, "zustandslos_neu_verankern")
            seit = int(anker[-1].get("zeit_ms", 0)) if anker else 0
            danach = sum(1 for z in zeilen(k, "block_uebernommen") if z.get("mit_zeuge") and int(z.get("zeit_ms", 0)) > seit)
            groessen_z = sorted(int(z.get("bytes", 0)) for z in zeilen(k, "zeuge_fertig"))
            median = groessen_z[len(groessen_z) // 2] // 1024 if groessen_z else 0
            zeile = (f"{k.name} (zustandslos): Neuverankerungen {len(anker)}, danach {danach} Bloecke mit Zeugen; "
                     f"Zeugen im Median {median} KB, hoechstens {(groessen_z[-1] // 1024) if groessen_z else 0} KB")
            melde(zeile)
            bericht.append(zeile)
            if anker and danach == 0:
                befunde.append(f"{k.name} (zustandslos) folgte nach der letzten Neuverankerung keinem Block mit Zeugen")
        # ⚑ Zeugen von vollen Minern (2026-10-30): gestartet, und was der
        #   zustandslose Miner direkt geliefert bekam.
        bereit = [k for k in miner if rolle[k.name][0] in ZEUGEN_BEREIT]
        ohne = [rolle[k.name][0] for k in bereit if not zeilen(k, "zeugen_bereithalten")]
        if ohne:
            befunde.append(f"--zeugen-bereithalten nicht gestartet bei {', '.join(ohne)}")
        for zl in [k for k in miner if rolle[k.name][0] == ZUSTANDSLOSER_MINER]:
            geliefert = zeilen(zl, "zeugenstuecke_nachgeliefert")
            gesperrt = [z for z in zeilen(zl, "block_abgelehnt") if z.get("zeuge_vom_erzeuger") is False]
            zeile = (f"Zeugen bereit bei {len(bereit) - len(ohne)} vollen Minern; {zl.name} bekam "
                     f"{sum(int(z.get('tragend', 0)) for z in geliefert)} tragende Stuecke direkt, "
                     f"von {len({z.get('von') for z in geliefert if int(z.get('tragend', 0)) > 0})} Gegenstellen; "
                     f"fremde Zeugen, die nicht trugen: {len(gesperrt)}")
            melde(zeile)
            bericht.append(zeile)
            if gesperrt:
                befunde.append(f"{zl.name}: {len(gesperrt)} Zeugen voller Miner trugen nicht")
        zeugenlos = max((int(z.get("zeugenlos", 0)) for z in zeilen(erzeuger, "groessen")), default=0)
        offen = max((int(z.get("auszahlungen_offen", 0)) for z in zeilen(erzeuger, "groessen")), default=0)
        zeile = f"Auszahlungen: hoechstens {offen} vorgemerkt am Ende einer Epoche"
        melde(zeile)
        bericht.append(zeile)
        # ⚑ Die Zeugen auf der Platte (2026-10-30): Bloecke im Fenster,
        #   Bytes, und ob ein Ablegen misslang.
        eg = (zeilen(erzeuger, "groessen") or [{}])[-1]
        platte_fehler = int(eg.get("zeugen_platte_fehler", 0))
        zeile = (f"Zeugen beim Erzeuger: zeugenlos {zeugenlos}; auf der Platte {eg.get('zeugen_platte_bloecke', 0)} Bloecke, "
                 f"{int(eg.get('zeugen_platte_bytes', 0)) // 1024} KB, Fehler {platte_fehler}")
        if platte_fehler:
            befunde.append(f"der Erzeuger konnte {platte_fehler} Zeugen nicht auf der Platte ablegen")
        melde(zeile)
        bericht.append(zeile)
        if zeugenlos:
            befunde.append(f"der Erzeuger baute {zeugenlos} Bloecke ohne Zeugen")
    # ⚑ **Zustandsbaum auf der Platte** (2026-10-30): Die Wurzel ist oben
    #   schon fuer alle verglichen; hier, wie viel ausgelagert ist.
    if args.baum_auf_platte:
        auf_platte = [k for k in [erzeuger] + miner if k is erzeuger or rolle[k.name][0] in BAUM_AUF_PLATTE]
        teile = []
        for k in auf_platte:
            g = zeilen(k, "groessen")
            if g:
                teile.append(f"{'k0' if k is erzeuger else rolle[k.name][0]} {g[-1].get('baum_im_speicher')} Knoten, "
                             f"{int(g[-1].get('baum_seiten_bytes', 0)) // 1024} KB")
        zeile = "Baum auf der Platte: " + "; ".join(teile)
        melde(zeile)
        bericht.append(zeile)
        if not any(int((zeilen(k, "groessen") or [{}])[-1].get("baum_seiten_bytes", 0)) > 0 for k in auf_platte):
            befunde.append("--baum-auf-platte: nichts ausgelagert")
    # ⚑ Fund 677 (2026-10-29): was mit der Laufzeit waechst, beim ersten
    #   Miner von der ersten zur letzten Epoche; dazu die Selbstproben.
    groessen = zeilen(miner[0], "groessen") if miner else []
    if len(groessen) >= 2:
        a, b = groessen[0], groessen[-1]
        gewachsen = {k: (a[k], b[k]) for k in b if isinstance(b.get(k), int) and k not in ("folge", "zeit_ms", "epoche")
                     and a.get(k) != b[k]}
        zeile = f"Groessen {miner[0].name} Epoche {a.get('epoche')} -> {b.get('epoche')}: {gewachsen}"
        melde(zeile)
        bericht.append(zeile)
    selbst = [z for k in miner for z in zeilen(k, "selbstprobe")]
    if selbst:
        zeile = (f"Selbstproben: {len(selbst)}, mit Warnung {sum(1 for z in selbst if z.get('warnung'))}, "
                 f"Zusage verlangsamt {sum(1 for z in selbst if z.get('ebenenzeit_us_neu'))}")
        melde(zeile)
        bericht.append(zeile)
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
    p.add_argument("--zwillinge", action="store_true",
                   help="dreizehn Miner, also zwei Pods: jeder Auftrag ueber beide, verguetet nur, was beide gleich rechnen")
    p.add_argument("--verweigerer", action="store_true",
                   help=f"{VERWEIGERER} fuehrt keinen Shard und weist jede Zeitprobe ab; verlangt ist seine Sperre")
    p.add_argument("--nur", type=lambda s: set(s.split(",")),
                   help=f"nur diese Phasen, mit Komma ({','.join(PHASEN)}); ohne Angabe alle. Fuer kurze, gezielte Laeufe")
    p.add_argument("--paare", action="store_true",
                   help="fuenfzehn Miner mehr, also vier Pods und zwei Zwillingspaare; sechs Sitzungen fragen zugleich (W7)")
    p.add_argument("--faelscher", action="store_true",
                   help=f"{FAELSCHER} rechnet falsch, aber stimmig (Shard mit dem Merkmal faelscher, braucht --zwillinge)")
    p.add_argument("--baum-auf-platte", action="store_true",
                   help="Erzeuger und einige Miner lagern den Zustandsbaum auf die Platte aus")
    p.add_argument("--zustandslos", action="store_true",
                   help="ein Miner zustandslos, dazu ein spaeter zustandsloser Knoten ueber einen leichten Abzug")
    p.add_argument("--lastgeber", type=pathlib.Path, help="das Beispiel lastgeber aus myl-testclient (mit --tor --real)")
    p.add_argument("--laststufen", help="Raten je Sekunde mit Komma, statt der Vorgabe (kurz 50,200; lang bis 1600)")
    p.add_argument("--lastdauer", type=int, default=30, help="Sekunden je Laststufe mit --laststufen")
    args = p.parse_args()
    if args.nur and not args.nur <= set(PHASEN):
        p.error(f"unbekannte Phase: {sorted(args.nur - set(PHASEN))}, bekannt sind {', '.join(PHASEN)}")
    if args.paare and not args.zwillinge:
        p.error("--paare braucht --zwillinge")
    if args.faelscher and not args.zwillinge:
        p.error("--faelscher braucht --zwillinge: nur ein Zwilling sieht die falsche Ausgabe")
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
