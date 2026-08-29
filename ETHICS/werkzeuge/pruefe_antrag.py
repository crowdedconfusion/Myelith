#!/usr/bin/env python3
"""Prüft einen Korpus-Aufnahmeantrag gegen Manifest G3 (Punkt 1.2).

G3 nennt vier Pflichtangaben. **Ohne sie ist ein Antrag unvollständig
und wird nicht zur Abstimmung gestellt.** Dieses Skript sagt, welche
fehlt, statt „unvollständig".

## ⚑ Warum JSON und nicht TOML

TOML wäre lesbarer, und `tomllib` gibt es erst ab Python 3.11. Die
Arbeitsumgebung fährt 3.9, die CI 3.12. **Eine Prüfung, die nur in der
CI läuft, lässt sich vor dem Einreichen nicht ausführen**, und
ausgerechnet dieses Repositorium hat mit Fund 65 gelernt, was von
Prüfungen zu halten ist, die nirgends laufen.

Der Verlust sind die Kommentare. Er wird ausgeglichen: Die Begründungen
stehen als `_pflicht_*`-Felder in der Vorlage, und `--text` erzeugt den
Fließtext, den Punkt 1.2 verlangt.

## ⚑ Was es prüft und was es nicht prüft

Es prüft **Vollständigkeit**, nicht Wahrheit. Ob eine angegebene
Merkle-Wurzel stimmt, ob der genannte Filter wirklich lief, ob die
Rechtsgrundlage trägt: nichts davon sieht ein Skript. Es sorgt dafür,
dass **jemand es hingeschrieben hat und dafür einsteht**, und genau das
ist der Zweck von G3.

⚑ **Und es prüft ausdrücklich nichts am Inhalt.** Grundsatz G1 verbietet
die inhaltliche Bewertung. Wer hier ein Feld für „Qualität" oder
„Angemessenheit" ergänzt, hebt G1 auf.

Aufruf: `python3 ETHICS/werkzeuge/pruefe_antrag.py <antrag.json> [--text]`
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

import json

HEX64 = re.compile(r"^[0-9a-f]{64}$")


def leer(x) -> bool:
    return x is None or x == "" or x == [] or x == {}


def pruefe(d: dict) -> list[str]:
    m: list[str] = []

    for feld in ("name", "fassung", "antragsteller", "datum"):
        if leer(d.get("korpus", {}).get(feld)):
            m.append(f"korpus.{feld} fehlt")

    # 1: Herkunft und Rechtsgrundlage
    herkunft = d.get("herkunft") or []
    if not herkunft:
        m.append("Pflichtangabe 1 fehlt: kein Bestandteil unter [[herkunft]]")
    for i, h in enumerate(herkunft):
        for feld in ("bestandteil", "quelle", "rechtsgrundlage"):
            if leer(h.get(feld)):
                m.append(f"herkunft[{i}].{feld} fehlt")
        # ⚑ Bei Web-Daten ist der Nutzungsvorbehalt kein Beiwerk:
        # § 44b Abs. 3 UrhG macht ihn zur Bedingung der Schranke.
        if not leer(h.get("erhebungsmethode")) and leer(h.get("nutzungsvorbehalt")):
            m.append(
                f"herkunft[{i}]: Erhebungsmethode angegeben, aber "
                "nutzungsvorbehalt leer (§ 44b Abs. 3 UrhG)"
            )

    # 2: Ausschluss-Nachweis
    ausschluss = d.get("ausschluss") or []
    if not ausschluss:
        m.append("Pflichtangabe 2 fehlt: kein Eintrag unter [[ausschluss]]")
    for i, a in enumerate(ausschluss):
        for feld in ("was", "werkzeug", "werkzeugversion"):
            if leer(a.get(feld)):
                m.append(f"ausschluss[{i}].{feld} fehlt")

    # 3: Personenbezug
    pb = d.get("personenbezug")
    if pb is None:
        m.append("Pflichtangabe 3 fehlt: kein Abschnitt [personenbezug]")
    elif pb.get("enthalten"):
        for feld in ("umfang", "rechtsgrundlage"):
            if leer(pb.get(feld)):
                m.append(f"personenbezug.{feld} fehlt, obwohl enthalten = true")

    # 4: Reproduzierbarkeit
    rp = d.get("reproduzierbarkeit")
    if rp is None:
        m.append("Pflichtangabe 4 fehlt: kein Abschnitt [reproduzierbarkeit]")
    else:
        wurzel = rp.get("merkle_wurzel", "")
        if leer(wurzel):
            m.append("reproduzierbarkeit.merkle_wurzel fehlt")
        elif not HEX64.match(wurzel):
            m.append("reproduzierbarkeit.merkle_wurzel ist keine 64 Hex-Zeichen")
        for feld in ("erzeugungsskript", "werkzeugversionen", "nachgebaut_von"):
            if leer(rp.get(feld)):
                m.append(f"reproduzierbarkeit.{feld} fehlt")
    return m


def fliesstext(d: dict) -> str:
    """Der Antrag als Fließtext, wie Punkt 1.2 ihn verlangt.

    ⚑ **Erzeugt, nicht geschrieben**, damit Formular und Text nicht
    auseinanderlaufen können. Wer den Text ändern will, ändert das
    Formular.
    """
    k = d.get("korpus", {})
    z: list[str] = [
        f"# Korpus-Aufnahmeantrag: {k.get('name') or '(ohne Namen)'}",
        "",
        f"Fassung {k.get('fassung') or '?'}, eingereicht am {k.get('datum') or '?'} "
        f"von {k.get('antragsteller') or '(ohne Antragsteller)'}.",
        "",
        "## 1. Herkunft und Rechtsgrundlage",
        "",
    ]
    for h in d.get("herkunft") or []:
        satz = (
            f"- **{h.get('bestandteil') or '(ohne Bestandteil)'}** aus "
            f"{h.get('quelle') or '(ohne Quelle)'}, Rechtsgrundlage: "
            f"{h.get('rechtsgrundlage') or '(offen)'}."
        )
        if h.get("erhebungsmethode"):
            satz += (
                f" Erhoben durch {h['erhebungsmethode']}; Nutzungsvorbehalte: "
                f"{h.get('nutzungsvorbehalt') or '(offen)'}."
            )
        z.append(satz)
    z += ["", "## 2. Ausschluss-Nachweis", ""]
    for a in d.get("ausschluss") or []:
        z.append(
            f"- {a.get('was') or '(ohne Angabe)'}, geprüft mit "
            f"{a.get('werkzeug') or '(ohne Werkzeug)'} "
            f"{a.get('werkzeugversion') or ''}: {a.get('ausgeschlossen', 0)} ausgeschlossen."
        )
    pb = d.get("personenbezug") or {}
    z += ["", "## 3. Personenbezug", ""]
    z.append(
        f"Personenbezogene Daten sind {'enthalten' if pb.get('enthalten') else 'nicht enthalten'}."
        + (
            f" Umfang: {pb.get('umfang') or '(offen)'}; Rechtsgrundlage: "
            f"{pb.get('rechtsgrundlage') or '(offen)'}."
            if pb.get("enthalten")
            else ""
        )
    )
    rp = d.get("reproduzierbarkeit") or {}
    z += ["", "## 4. Reproduzierbarkeit", ""]
    z.append(f"Merkle-Wurzel `{rp.get('merkle_wurzel') or '(offen)'}`, erzeugt von "
             f"`{rp.get('erzeugungsskript') or '(offen)'}` mit "
             f"{', '.join(rp.get('werkzeugversionen') or ['(offen)'])}.")
    z.append(f"Unabhängig nachgebaut von: {rp.get('nachgebaut_von') or '(niemandem)'}.")
    return "\n".join(z)


def main() -> int:
    if len(sys.argv) < 2:
        print("[antrag] Aufruf: pruefe_antrag.py <antrag.json> [--text]")
        return 2
    p = Path(sys.argv[1])
    if not p.is_file():
        print(f"[antrag] FEHLGESCHLAGEN: {p} gibt es nicht")
        return 1

    d = json.loads(p.read_text(encoding="utf-8"))
    if "--text" in sys.argv:
        print(fliesstext(d))
        return 0
    maengel = pruefe(d)
    print(f"[antrag] {p.name}: {len(maengel)} Mangel/Mängel")
    for x in maengel:
        print(f"[antrag]   fehlt: {x}")
    if maengel:
        print("[antrag] UNVOLLSTÄNDIG: wird nicht zur Abstimmung gestellt (G3)")
        return 1
    print("[antrag] VOLLSTÄNDIG: alle vier Pflichtangaben sind ausgefüllt")
    print("[antrag] ⚑ Das ist keine Aussage über ihre Richtigkeit.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
