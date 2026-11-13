#!/usr/bin/env python3
"""Erzeugt `COMPLIANCE/DRITTLIZENZEN.txt`: die Lizenztexte alles Mitgelieferten.

Aufruf: `python3 COMPLIANCE/werkzeuge/lizenztexte.py [--pruefe]`

## ⚑ Warum neben `NOTICES.md`

`NOTICES.md` nennt, was unter welcher Lizenz steht. MIT, BSD und Apache
verlangen beim Weitergeben aber den Text samt Urhebervermerk. Der steht in
den Paketen selbst, darum wird er aus `SYSTEM/crates-vorrat` gelesen und
nicht abgeschrieben. Gleiche Texte stehen einmal, mit allen Paketen davor.

## ⚠️ Was sie nicht leistet

Sie ersetzt keine Rechtsberatung. Ein Paket ohne Lizenzdatei wird genannt,
samt der Angabe aus seinem Manifest und seinen Autoren.
"""

import io
import json
import re
import sys
import tarfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from notices import rust_pakete  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
ZIEL = REPO / "COMPLIANCE" / "DRITTLIZENZEN.txt"
VORRAT = REPO / "SYSTEM" / "crates-vorrat"
LISTE = REPO / "COMPLIANCE" / "fremdkomponenten.json"
APACHE = REPO / "CLIENT" / "myl-scan" / "modelle" / "LICENSE-APACHE-2.0"
LIZENZDATEI = re.compile(r"(?i)^(licen[cs]e|copying|copyright|notice|unlicense)([-_.].*)?$")
BIP39_MIT = """BIP-39-Wortliste, Copyright (c) Marek Palatinus, Pavol Rusnak, Aaron Voisine, Sean Bowe.

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
"""


# Der Wortlaut fuer Pakete, die ihre Lizenz nur im Manifest nennen.
ZLIB = """This software is provided 'as-is', without any express or implied warranty. In no event will the authors be held liable for any damages arising from the use of this software.

Permission is granted to anyone to use this software for any purpose, including commercial applications, and to alter it and redistribute it freely, subject to the following restrictions:

1. The origin of this software must not be misrepresented; you must not claim that you wrote the original software. If you use this software in a product, an acknowledgment in the product documentation would be appreciated but is not required.
2. Altered source versions must be plainly marked as such, and must not be misrepresented as being the original software.
3. This notice may not be removed or altered from any source distribution.
"""
BSD3 = """Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.
3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
"""


def tomlwert(toml: str, feld: str) -> str:
    m = re.search(rf'(?m)^{feld}\s*=\s*(\[[^\]]*\]|"[^"\n]*")', toml)
    return re.sub(r'[\[\]"\n]', "", m.group(1)).strip() if m else ""


def aus_kiste(name: str, fassung: str) -> tuple:
    """(Dateien als (Name, Text), Autoren) aus dem Archiv im Vorrat."""
    pfad = VORRAT / f"{name}-{fassung}.crate"
    if not pfad.is_file():
        raise SystemExit(f"[lizenztexte] {pfad.name} fehlt im Vorrat")
    dateien, autoren = [], ""
    with tarfile.open(fileobj=io.BytesIO(pfad.read_bytes()), mode="r:gz") as t:
        for m in t.getmembers():
            if not m.isfile():
                continue
            teile = m.name.split("/")[1:]
            inhalt = lambda: t.extractfile(m).read().decode("utf-8", "replace")
            if teile == ["Cargo.toml"]:
                autoren = tomlwert(inhalt(), "authors")
            elif (len(teile) == 1 and LIZENZDATEI.match(teile[0])) or (len(teile) == 2 and teile[0].upper() == "LICENSES"):
                dateien.append(("/".join(teile), inhalt().replace("\r\n", "\n").strip() + "\n"))
    return sorted(dateien), autoren


def abschnitt(titel: str, text: str) -> str:
    return f"{'=' * 72}\n{titel}\n{'=' * 72}\n\n{text.rstrip()}\n\n"


def veraendert(name: str) -> str:
    if name.startswith("Zeilennetz"):
        return "Von diesem Projekt verändert: nur Netz, Codes und Zeichensatz übernommen, ohne Wörterbücher, und in ein eigenes Format gepackt."
    return "Von diesem Projekt verändert: die Gewichte auf 8 Bit je Kanal quantisiert und neu gepackt."


def erzeugen() -> str:
    pakete, _ = rust_pakete()
    texte: dict = {}
    ohne = []
    for (name, fassung), info in sorted(pakete.items()):
        dateien, autoren = aus_kiste(name, fassung)
        if not dateien:
            ohne.append(f"{name} {fassung}: {info['lizenz']}; Autoren: {autoren or 'nicht angegeben'}")
        for datei, text in dateien:
            texte.setdefault(text, []).append(f"{name} {fassung} ({datei})")
    teile = [
        "Drittlizenzen von Myelith / Third-party licences of Myelith\n\n"
        "Erzeugt von COMPLIANCE/werkzeuge/lizenztexte.py, nicht bearbeiten. Welche Komponente unter welcher Lizenz steht, "
        "sagt NOTICES.md. Myelith selbst steht unter der PolyForm Shield License 1.0.0 (LICENSE.md). Keine Rechtsberatung.\n\n"
    ]
    liste = json.loads(LISTE.read_text(encoding="utf-8"))
    teile.append(abschnitt("Eingebaute Modelle / Built-in models", "\n\n".join(
        f"{m['name']}\n  Anbieter: {m.get('anbieter', '')}\n  Quelle: {m['quelle']}\n  Lizenz: {m['lizenz']}\n  {veraendert(m['name'])}" + (f"\n  {m['hinweis']}" if m.get("hinweis") else "")
        for m in liste["modell"] if m.get("weg") == "mitgeliefert")))
    teile.append(abschnitt("Apache License 2.0 (für die eingebauten Modelle und die so lizenzierten Pakete)", APACHE.read_text(encoding="utf-8")))
    teile.append(abschnitt("BIP-39-Wortliste (MIT)", BIP39_MIT))
    for i, (text, wer) in enumerate(sorted(texte.items(), key=lambda kv: kv[1][0]), 1):
        teile.append(abschnitt(f"Text {i}, gilt für:\n" + "\n".join(wer), text))
    if ohne:
        mit = BIP39_MIT.split("\n\n", 1)[1]
        teile.append(abschnitt("Pakete ohne Lizenzdatei im Archiv (Lizenz und Autoren aus dem Manifest)", "\n".join(ohne)))
        teile.append(abschnitt("Wortlaut für diese Pakete: MIT (Copyright bei den genannten Autoren)", mit))
        teile.append(abschnitt("Wortlaut für diese Pakete: Zlib (Copyright bei den genannten Autoren)", ZLIB))
        teile.append(abschnitt("Wortlaut für diese Pakete: BSD-3-Clause (Copyright bei den genannten Autoren)", BSD3))
        teile.append("Apache-2.0 steht oben; MPL-2.0 steht bei den Texten der Pakete, die sie mitbringen.\n")
    return "".join(teile)


def main() -> int:
    neu = erzeugen()
    if "--pruefe" in sys.argv:
        alt = ZIEL.read_text(encoding="utf-8") if ZIEL.is_file() else ""
        if alt != neu:
            print("[lizenztexte] FEHLGESCHLAGEN: COMPLIANCE/DRITTLIZENZEN.txt passt nicht zu ihren Quellen")
            return 1
        print("[lizenztexte] PASSED: COMPLIANCE/DRITTLIZENZEN.txt entspricht den Quellen")
        return 0
    ZIEL.write_text(neu, encoding="utf-8")
    print(f"[lizenztexte] geschrieben: {ZIEL.relative_to(REPO)}, {len(neu.encode('utf-8'))} Bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
