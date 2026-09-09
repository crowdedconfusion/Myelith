#!/usr/bin/env python3
"""Traegt jede Sperrdatei die Version ihrer eigenen Kiste?

# ⛑ Warum es dieses Skript gibt

Zweimal hat derselbe Fehler die CI rot gemacht. Am 2026-09-08 waren es
neun Sperrdateien auf einmal, am 2026-09-09 die von `myl-oberflaeche`:
Die Kiste stand auf `0.13.0`, die Sperrdatei fuehrte sie als `0.8.0`.
Jeder Bau mit `--locked` bricht dann ab, und zwar erst nach dem
Einrichten der Toolchain und dem Aufwaermen des Zwischenspeichers, also
nach Minuten und in einem Job, der mit der Aenderung nichts zu tun hat.

Der Grund ist immer derselbe: Wer eine Version in `Cargo.toml` erhoeht,
sieht die Sperrdatei nicht. Sie aendert sich nur, wenn cargo laeuft, und
ein `sed` ueber die Kistendatei laesst cargo nicht laufen.

# ⚑ Was es prueft und was nicht

**Jede Version einer Kiste aus diesem Repositorium**, gleich ob sie in
ihrer eigenen Sperrdatei steht oder in der einer anderen. Das ist mehr
als die eigene Version, und der Unterschied ist der teure Fall: Wird
`integer-llm-runtime` erhoeht, steht sie in **zehn** Sperrdateien, und
neun davon gehoeren Kisten, die niemand angefasst hat.

Ob die Abhaengigkeiten aufloesen, sagt allein cargo; das tut ohnehin
jeder `--locked`-Bau. Diese Pruefung ist die billige Vorstufe davon:
zwei Sekunden statt Minuten, und die Meldung nennt die Datei und beide
Zahlen.

Aufruf: `python3 werkzeuge/sperrdateien.py`, Rueckgabe 1 bei Abweichung.
"""

# ⛑ Ohne diese Zeile faellt das Skript auf Python 3.9 beim Einlesen um,
#   und zwar an der Signatur und nicht an der Arbeit. Der Runner der CI
#   ist neuer, die Maschine des Projektinhabers ist es nicht.
from __future__ import annotations

import pathlib
import re
import sys


def eigene_version(text: str) -> tuple[str, str] | None:
    """Name und Version aus dem `[package]`-Abschnitt einer Kistendatei."""
    abschnitt = re.search(r"(?ms)^\[package\]\n(.*?)(?=^\[|\Z)", text)
    if not abschnitt:
        return None
    k = abschnitt.group(1)
    name = re.search(r'(?m)^\s*name\s*=\s*"([^"]+)"', k)
    ver = re.search(r'(?m)^\s*version\s*=\s*"([^"]+)"', k)
    return (name.group(1), ver.group(1)) if name and ver else None


def eigene_kisten(wurzel: pathlib.Path) -> dict[str, tuple[str, pathlib.Path]]:
    """Name zu (Version, Kistendatei) fuer jede Kiste dieses Repositoriums."""
    aus: dict[str, tuple[str, pathlib.Path]] = {}
    for kiste in sorted(wurzel.rglob("Cargo.toml")):
        if any(t in ("target", "target-shared") for t in kiste.parts):
            continue
        eigen = eigene_version(kiste.read_text(encoding="utf-8"))
        if eigen is None:
            continue
        name, ver = eigen
        # ⛑ Zwei Kisten gleichen Namens waeren ein eigener Fehler; hier
        #   wird die erste behalten und die zweite gemeldet.
        if name in aus and aus[name][0] != ver:
            print(f"  {name}: zwei Kistendateien mit verschiedenen Versionen")
        aus.setdefault(name, (ver, kiste))
    return aus


def main() -> int:
    wurzel = pathlib.Path(__file__).resolve().parent.parent
    schief: list[str] = []
    geprueft = 0

    # ── Jede Kiste dieses Repositoriums in JEDER Sperrdatei ──────────
    eigen = eigene_kisten(wurzel)
    for lock in sorted(wurzel.rglob("Cargo.lock")):
        if any(t in ("target", "target-shared") for t in lock.parts):
            continue
        text = lock.read_text(encoding="utf-8")
        wo = lock.relative_to(wurzel)
        for treffer in re.finditer(
            r'(?ms)^\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"', text
        ):
            name, hat = treffer.group(1), treffer.group(2)
            if name not in eigen:
                continue
            soll = eigen[name][0]
            if hat != soll:
                schief.append(f"  {name}: {eigen[name][1].parent.name} sagt {soll}, {wo} sagt {hat}")

    for lock in sorted(wurzel.rglob("Cargo.lock")):
        # ⚑ Ein Bauverzeichnis traegt Kopien; sie gehoeren niemandem.
        if any(t in ("target", "target-shared") for t in lock.parts):
            continue
        kiste = lock.with_name("Cargo.toml")
        if not kiste.exists():
            continue
        eigen = eigene_version(kiste.read_text(encoding="utf-8"))
        if eigen is None:
            continue
        name, soll = eigen
        geprueft += 1
        text = lock.read_text(encoding="utf-8")
        treffer = re.search(
            r'(?ms)^\[\[package\]\]\nname = "%s"\nversion = "([^"]+)"' % re.escape(name),
            text,
        )
        wo = lock.relative_to(wurzel)
        if treffer is None:
            schief.append(f"  {name}: fehlt ganz in {wo}")
        # Die Versionsabweichung meldet bereits die Schleife oben; hier
        # bleibt der Fall „steht gar nicht drin".

    if schief:
        print("Sperrdatei und Kiste laufen auseinander:")
        print("\n".join(schief))
        print(
            "\nJeder Bau mit --locked bricht daran ab. Zu beheben mit\n"
            "  cargo metadata --offline --format-version 1 \\\n"
            "    --manifest-path <Kiste>/Cargo.toml >/dev/null\n"
            "fuer jede genannte Sperrdatei. ⚑ `metadata` loest nur auf und\n"
            "uebersetzt nichts; `cargo check` taete dasselbe und braeuchte\n"
            "Minuten statt Sekunden."
        )
        return 1

    print(
        f"{geprueft} Sperrdateien geprueft: jede eigene Kiste und jede\n"
        f"Pfadabhaengigkeit aus diesem Repositorium tragen dieselbe Version."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
