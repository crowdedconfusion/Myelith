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

Noch nicht dabei (naechste Stufe): Anfragen ueber die Tuer, die der Pod
rechnet und bucht, und die Stichprobe der Segmente samt Nachrechnung.

Die letzte Zeile ist `Fertig: bestanden` oder `FEHLER: ...`; Rueckgabe 0, 1
(ein Befund) oder 2 (die Umgebung fehlt).
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import random
import signal
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import netzlauf_prozesse as nl  # noqa: E402
from netzlauf_prozesse import Befund, Knoten, Stoerer, melde, warte  # noqa: E402

EPOCHE = 1800

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

    alle = [Knoten(i, ordner, *stoerung(i)) for i in range(gesamt)]
    erzeuger = alle[0]
    miner = alle[1:]
    rolle = {k.name: m for k, m in zip(miner, MINER)}
    luegner = next(k for k in miner if rolle[k.name][0] == "luegner")
    ehrlich = [k for k in miner if k is not luegner]
    # ⚑ Vergleich der Speicherhaltung: jeder zweite Miner mit gedeckelten
    #   Arenen von glibc, die anderen wie ausgeliefert.
    if args.arenen:
        for k in miner[1::2]:
            k.umgebung["MALLOC_ARENA_MAX"] = str(args.arenen)

    def epoche() -> int:
        return max(0, erzeuger.hoehe()) // EPOCHE

    def angaben(k: Knoten) -> list[str]:
        _, ebenen, us = rolle[k.name]
        return [
            "--konsensschluessel", str(k.ordner / "konsens.key"),
            "--modellzusage", f"{ebenen}:{us}",
            "--zone", "Europe",
            "--shard-programm", str(args.shard),
            "--artefakte", str(args.artefakte),
        ]

    nl.bruecke_anlegen()
    for k in alle:
        k.netz_anlegen()
    namen = ", ".join(f"{k.name}={rolle[k.name][0]}" for k in miner)
    melde(f"Erzeuger k0 und {len(miner)} Miner ({namen}), Takt {takt} ms")

    erzeuger.starten(args.programm, None, True, takt)
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

    luegner_epoche = None
    # Pods gibt es ab Epoche 1 (der erste Entwurf sah erst ab 2 hin und las
    # dann eine aeltere Sperre).
    # 📌 Fund 641: Die Positionen verteilt der Zufall der Kette; bis
    #    Epoche 8 blieb der Luegner in einem von rund zehn kurzen Laeufen
    #    ohne. Sechzehn Epochen kosten nur, wenn er spaet drankommt.
    for e in range(1, LUEGNER_BIS + 1):
        in_epoche(e)
        if luegner.name in pods_melden(e):
            luegner_epoche = e
            break
    if luegner_epoche is None:
        raise Befund(f"der Luegner bekam bis Epoche {LUEGNER_BIS} keine Position")

    # ---- 3. Rechnen ueber das Netz, 4. Meldung und Sperre -----------------
    # Die Sperre gilt fuer die Epoche nach der Meldung; verlangt ist die erste
    # ab seiner Position.
    def neue_sperre() -> dict | None:
        return next((z for z in zeilen(luegner, "fristsperre") if z.get("epoche", 0) > luegner_epoche), None)

    warte(lambda: neue_sperre() is not None, 3 * EPOCHE * takt / 1000 + 120, "die Kette sperrt den Luegner", alle)
    gesperrt_e = int(neue_sperre()["epoche"])
    gemessen = [z for k in ehrlich for z in zeilen(k, "fristmessung")]
    meldungen = [z for k in ehrlich for z in zeilen(k, "fristmeldung")]
    melde(
        f"Luegner auf Position in Epoche {luegner_epoche}, gesperrt fuer Epoche {gesperrt_e}; "
        f"{len(gemessen)} Zeitproben ueber das Netz, {len(meldungen)} Meldungen"
    )
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
        dauer = warte(lambda: len(zeilen(opfer, "shard_gestartet")) > vorher and shard_pids(opfer), 120,
                      f"der Shard von {rolle[opfer.name][0]} startet neu", alle)
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
            if e % 5 == 0:
                shard_absturz()
            if e % 7 == 0:
                miner_absturz(r)
        time.sleep(5)
    speicher_ablegen(epoche())
    if stoerer:
        stoerer.beenden()

    # ---- Kein ehrlicher Miner gesperrt -----------------------------------
    falsch = [rolle[k.name][0] for k in ehrlich if zeilen(k, "fristsperre")]
    if falsch:
        raise Befund(f"ehrliche Miner gesperrt: {', '.join(falsch)}")

    # ---- 6. Einigkeit --------------------------------------------------------
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
        "shard_gestartet", "shard_beendet", "fristmessung", "fristmessung_verworfen", "fristmeldung",
        "dateien_holen", "stille_nachfrage",
    )}
    melde("Zaehlung: " + ", ".join(f"{a}={n}" for a, n in zaehlung.items()))
    melde(f"Ende: {len(wurzeln)} Miner bei Hoehe {hoehe}, eine Wurzel {next(iter(wurzeln.values()))}")


# Bis zu dieser Epoche muss der Luegner einmal eine Position bekommen.
LUEGNER_BIS = 16


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
    p.add_argument("--protokolle", type=pathlib.Path)
    args = p.parse_args()
    args.programm = args.programm.resolve()
    args.shard = args.shard.resolve()
    args.artefakte = args.artefakte.resolve()
    fehlt = nl.umgebung_pruefen(args.programm)
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
