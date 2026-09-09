#!/usr/bin/env python3
"""Erzeugt das Programmsymbol in allen Groessen, aus der Marke.

# ⛑ Warum es das braucht

Der Buendler von Tauri legt fuer Windows ein `.msi` und fuer macOS ein
`.dmg` an, und beide verlangen ein Symbol im Format ihres Systems. Im
Verzeichnis lagen nur PNG-Dateien. Ein Bau, dem das Symbol fehlt,
bricht nicht immer ab: Er liefert unter Umstaenden ein Buendel mit dem
Platzhalter des Werkzeugs, und das faellt erst dem auf, der es
anklickt.

# ⚑ Aus der Marke und nicht aus einer abgelegten PNG

Die Quelle ist `werkzeuge/marke.py`, dieselbe wie fuer die Marke in der
Seitenleiste. Eine PNG danebenzulegen und von Hand nachzuziehen hiesse,
zwei Wahrheiten zu haben; die eine wuerde irgendwann vergessen.

⛑ **Ein Ding ist im Symbol anders als in der Marke: Der Rahmen faellt
weg.** Er konkurriert mit der abgerundeten Kachel des
Programmsymbols, und der grosse Kreis ist dort Grenze genug.

⚑ Die Deckkraft war frueher ein zweiter Unterschied. Seit dem
2026-09-09 traegt die Marke selbst ueberall dieselbe, es gibt hier also
nichts mehr abzuschalten.

# ⚑ Ohne neue Abhaengigkeit

Das Projekt hat keine Bildbibliothek in Python, und dafuer soll es auch
keine bekommen. Gezeichnet wird mit `qlmanage`, verkleinert mit `sips`,
gepackt mit `iconutil`, alles drei bringt macOS mit; die ICO-Datei
entsteht hier von Hand, denn ihr Aufbau ist ein Kopf, ein Eintrag je
Groesse und die PNG-Dateien unveraendert dahinter.

⚑ **Das Ergebnis wird abgelegt und nicht bei jedem Bau erzeugt.** Damit
braucht die CI weder macOS noch dieses Skript, und `freigabe.sh` auf
der eigenen Maschine ebenso wenig. Wer das Symbol aendert, tauscht
`icon.png` und laesst dieses Skript einmal laufen.

Aufruf: `python3 werkzeuge/symbole.py` (macOS).
"""

from __future__ import annotations

import pathlib
import re
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


# Der Grund des Symbols und die Farbe der Linien darauf.
GRUND, LINIE, RUNDUNG = "#141416", "#e8e8ea", 22
# Wie viel von der Kachel die Marke einnimmt.
ANTEIL = 0.80


def symbol_svg(wurzel: pathlib.Path) -> str:
    """Die Marke als Programmsymbol: ohne Rahmen, volle Deckkraft."""
    marke = subprocess.run(
        [sys.executable, str(wurzel / "werkzeuge" / "marke.py")],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    inneres = marke[marke.index(">") + 1 : marke.rindex("</svg>")]
    # ⛑ Der Rahmen ist das einzige `rect` der Marke.
    inneres = re.sub(r"<rect[^>]*/>\n?", "", inneres)
    return (
        '<svg viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg" '
        f'style="color:{LINIE}">\n'
        f'<rect x="0" y="0" width="100" height="100" rx="{RUNDUNG}" fill="{GRUND}"/>\n'
        f'<g transform="translate(50 50) scale({ANTEIL}) translate(-50 -50)">\n'
        f"{inneres}\n</g>\n</svg>\n"
    )


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
    verzeichnis.mkdir(parents=True, exist_ok=True)
    for werkzeug in ("sips", "iconutil", "qlmanage"):
        if not shutil.which(werkzeug):
            print(f"Dieses Skript braucht `{werkzeug}`, also macOS.")
            return 1

    with tempfile.TemporaryDirectory() as tmp:
        arbeit = pathlib.Path(tmp)

        # ── Das Symbol zeichnen ──────────────────────────────────
        svg = arbeit / "symbol.svg"
        svg.write_text(symbol_svg(wurzel), encoding="utf-8")
        subprocess.run(
            ["qlmanage", "-t", "-s", "1024", "-o", str(arbeit), str(svg)],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        quelle = arbeit / "symbol.svg.png"
        if not quelle.is_file():
            print("qlmanage hat nichts gezeichnet.")
            return 1

        # ── Die Groessen, die `tauri.conf.json` anmeldet ─────────
        for name, kante in (
            ("icon.png", 512),
            ("32x32.png", 32),
            ("128x128.png", 128),
            ("128x128@2x.png", 256),
        ):
            verkleinern(quelle, verzeichnis / name, kante)

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

    for name in (
        "32x32.png",
        "128x128.png",
        "128x128@2x.png",
        "icon.png",
        "icon.ico",
        "icon.icns",
    ):
        p = verzeichnis / name
        print(f"  {name:<16} {p.stat().st_size:>7} Byte")
    return 0


if __name__ == "__main__":
    sys.exit(main())
