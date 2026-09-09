#!/usr/bin/env python3
"""Erzeugt `icon.ico` und `icon.icns` aus `icons/icon.png`.

# ⛑ Warum es das braucht

Der Buendler von Tauri legt fuer Windows ein `.msi` und fuer macOS ein
`.dmg` an, und beide verlangen ein Symbol im Format ihres Systems. Im
Verzeichnis lagen nur PNG-Dateien. Ein Bau, dem das Symbol fehlt,
bricht nicht immer ab: Er liefert unter Umstaenden ein Buendel mit dem
Platzhalter des Werkzeugs, und das faellt erst dem auf, der es
anklickt.

# ⚑ Ohne neue Abhaengigkeit

Das Projekt hat keine Bildbibliothek in Python, und fuer zwei Dateien
soll es auch keine bekommen. Verkleinert wird mit `sips`, gepackt mit
`iconutil`, beides bringt macOS mit; die ICO-Datei entsteht hier von
Hand, denn ihr Aufbau ist ein Kopf, ein Eintrag je Groesse und die
PNG-Dateien unveraendert dahinter.

⚑ **Das Ergebnis wird abgelegt und nicht bei jedem Bau erzeugt.** Damit
braucht die CI weder macOS noch dieses Skript, und `freigabe.sh` auf
der eigenen Maschine ebenso wenig. Wer das Symbol aendert, tauscht
`icon.png` und laesst dieses Skript einmal laufen.

Aufruf: `python3 werkzeuge/symbole.py` (macOS).
"""

from __future__ import annotations

import pathlib
import shutil
import struct
import subprocess
import sys
import tempfile

# ⚑ Die Groessen, die Windows in einer ICO-Datei erwartet. 256 liegt als
#   PNG darin, die kleineren ebenso: Seit Vista ist das erlaubt und
#   spart gegenueber dem alten Bitmap-Format ein Vielfaches.
ICO_GROESSEN = [16, 24, 32, 48, 64, 128, 256]

# Die Groessen, die macOS in einem Iconset erwartet, je einfach und doppelt.
ICNS_GROESSEN = [16, 32, 128, 256, 512]


def verkleinern(quelle: pathlib.Path, ziel: pathlib.Path, kante: int) -> None:
    subprocess.run(
        ["sips", "-z", str(kante), str(kante), str(quelle), "--out", str(ziel)],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def ico_schreiben(bilder: list[tuple[int, bytes]], ziel: pathlib.Path) -> None:
    """Packt fertige PNG-Daten in eine ICO-Datei.

    Der Kopf ist sechs Byte, danach folgt je Bild ein Eintrag von
    sechzehn Byte, danach die Daten. Eine Kantenlaenge von 256 wird als
    `0` geschrieben; das Feld ist ein Byte breit und 256 passt nicht
    hinein.
    """
    kopf = struct.pack("<HHH", 0, 1, len(bilder))
    versatz = len(kopf) + 16 * len(bilder)
    eintraege = b""
    daten = b""
    for kante, png in bilder:
        eintraege += struct.pack(
            "<BBBBHHII",
            kante if kante < 256 else 0,
            kante if kante < 256 else 0,
            0,  # keine Farbtabelle
            0,  # vorbehalten
            1,  # Farbebenen
            32,  # Bit je Bildpunkt
            len(png),
            versatz,
        )
        daten += png
        versatz += len(png)
    ziel.write_bytes(kopf + eintraege + daten)


def main() -> int:
    wurzel = pathlib.Path(__file__).resolve().parent.parent
    verzeichnis = wurzel / "CLIENT" / "myl-oberflaeche" / "icons"
    quelle = verzeichnis / "icon.png"
    if not quelle.exists():
        print(f"Fehlt: {quelle}")
        return 1
    if not shutil.which("sips") or not shutil.which("iconutil"):
        print("Dieses Skript braucht `sips` und `iconutil`, also macOS.")
        return 1

    with tempfile.TemporaryDirectory() as tmp:
        arbeit = pathlib.Path(tmp)

        bilder = []
        for kante in ICO_GROESSEN:
            p = arbeit / f"{kante}.png"
            verkleinern(quelle, p, kante)
            bilder.append((kante, p.read_bytes()))
        ico_schreiben(bilder, verzeichnis / "icon.ico")

        satz = arbeit / "icon.iconset"
        satz.mkdir()
        for kante in ICNS_GROESSEN:
            verkleinern(quelle, satz / f"icon_{kante}x{kante}.png", kante)
            verkleinern(quelle, satz / f"icon_{kante}x{kante}@2x.png", kante * 2)
        subprocess.run(
            ["iconutil", "-c", "icns", str(satz), "-o", str(verzeichnis / "icon.icns")],
            check=True,
        )

    for name in ("icon.ico", "icon.icns"):
        p = verzeichnis / name
        print(f"  {name:<12} {p.stat().st_size:>7} Byte")
    return 0


if __name__ == "__main__":
    sys.exit(main())
