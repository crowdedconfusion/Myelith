#!/usr/bin/env python3
"""**Reproduziert eine Neuberechnung das Skalenpaket?**

## ⛔️ Wozu diese Probe da ist

Ein Skalenpaket ersetzt den einzigen nichtdeterministischen Bauschritt,
die Aktivierungsstatistik (Fund 32). Damit wird ein Bau
plattformuebergreifend bitgleich, und das ist der Zweck.

⚠️ **Der Zweck traegt nur, solange das Paket auch das ist, was eine
Neuberechnung ergaebe.** Weicht es ab, ist es kein Ersatz mehr, sondern
**eingefrorene alte Rechnung**, und jeder Bau uebernimmt sie.

⛔️ **Und das faellt von allein nie auf**, weil jeder Bau das Paket
nimmt: Der Weg, auf dem man es merken wuerde, wird nie gegangen.
Gefunden wurde es am 2026-09-21 nur deshalb, weil ein θ_v-Sprung das
Paket ungueltig machte und ein Bau ausnahmsweise voll kalibrierte. **105
von 422 Skalen des 0,6B wichen ab**, systematisch nach oben.

📌 **Eine Abkuerzung, die nie gegen den langen Weg gehalten wird, ist
keine Abkuerzung, sondern eine zweite Wahrheit.**

## ⚠️ Was diese Probe kostet

Sie rechnet den Korpus durch, also rund zwei Minuten fuer das 0,6B und
mehr fuer groessere Modelle. **Sie gehoert deshalb nicht zu den
schnellen Proben**, sondern vor die Verwendung eines Pakets und in jeden
θ_v-Sprung.

Aufruf:
    python3 INTEGER_LLM/tests/test_skalenpaket_reproduzierbar.py [modell]
"""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

HIER = Path(__file__).resolve().parent
LLM = HIER.parent
REPO = LLM.parent


def paket_scales(modell: str):
    p = LLM / "scale_packs" / modell / "scales.json"
    if not p.is_file():
        return None, f"kein Skalenpaket unter {p}"
    return json.loads(p.read_text(encoding="utf-8")), None


def neu_berechnen(modell: str, ziel: Path) -> str:
    """Baut das Modell mit voller Kalibrierung und gibt scales.json zurueck."""
    umgebung = dict(os.environ)
    umgebung["INTEGER_LLM_MODEL"] = modell
    # ⚑ **Der Kern der Probe**: ausdruecklich OHNE Paket.
    umgebung["INTEGER_LLM_SCALE_PACK"] = "0"
    umgebung["INTEGER_LLM_ARTIFACTS_DIR"] = str(ziel)
    lauf = subprocess.run(
        ["sh", str(LLM / "scripts" / "build_artifacts.sh")],
        cwd=str(LLM), env=umgebung, capture_output=True, text=True,
    )
    if lauf.returncode != 0:
        print(lauf.stdout[-2000:])
        print(lauf.stderr[-2000:])
        raise SystemExit(f"Bau fehlgeschlagen (rc={lauf.returncode})")
    return (ziel / modell / "scales.json").read_text(encoding="utf-8")


def main() -> int:
    modell = sys.argv[1] if len(sys.argv) > 1 else "myelith-0.6b"
    paket, fehler = paket_scales(modell)
    if fehler:
        print(f"[paketprobe] ⚠️ {fehler}")
        print("[paketprobe] uebersprungen: ohne Paket gibt es nichts zu pruefen")
        return 0

    print(f"[paketprobe] {modell}: rechne die Skalen neu (Korpusdurchlauf) ...")
    with tempfile.TemporaryDirectory(prefix="paketprobe-") as tmp:
        neu = json.loads(neu_berechnen(modell, Path(tmp)))

    nur_paket = sorted(set(paket) - set(neu))
    nur_neu = sorted(set(neu) - set(paket))
    gemeinsam = sorted(set(paket) & set(neu))

    abweichend = [
        k for k in gemeinsam
        if paket[k].get("shift") != neu[k].get("shift")
    ]
    print(f"[paketprobe] {len(gemeinsam)} gemeinsame Skalen, "
          f"{len(nur_paket)} nur im Paket, {len(nur_neu)} nur neu")
    print(f"[paketprobe] abweichende Schiebungen: {len(abweichend)}")

    for k in abweichend[:10]:
        print(f"    {k}: Paket shift={paket[k]['shift']} "
              f"(absmax {paket[k].get('absmax_observed')}) -> "
              f"neu shift={neu[k]['shift']} "
              f"(absmax {neu[k].get('absmax_observed')})")
    if len(abweichend) > 10:
        print(f"    ... und {len(abweichend) - 10} weitere")

    if not abweichend and not nur_paket and not nur_neu:
        print("[paketprobe] ⚑ BESTANDEN: das Paket ist genau das, was eine "
              "Neuberechnung ergibt.")
        return 0

    print()
    print("[paketprobe] ⛔️ FEHLGESCHLAGEN: das Paket ist NICHT das, was eine")
    print("             Neuberechnung ergibt. Damit ersetzt es den")
    print("             nichtdeterministischen Schritt nicht mehr, sondern")
    print("             friert eine aeltere Rechnung ein.")
    print()
    print("             Entweder das Paket neu erzeugen")
    print("             (tools/skalenpaket_bauen.py) oder erklaeren, warum")
    print("             die Neuberechnung heute etwas anderes ergibt.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
