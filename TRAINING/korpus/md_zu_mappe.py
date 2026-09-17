#!/usr/bin/env python3
"""Aus Markdown wird eine Wissensmappe, die ein Agent nachschlägt.

# ⚑ Der Unterschied zum Korpus, und warum es beides braucht

`buchkorpus.py` bringt ein Buch **in die Gewichte**: Das dauert einen
Trainingslauf und wirkt danach ohne Kontext. Diese Mappe bringt es
**neben den Kontext**: Das wirkt sofort und kostet bei jeder Frage ein
Nachschlagen. ⚑ **Schnell und dauerhaft sind zwei verschiedene Ziele**,
und für beide gibt es hier einen Weg.

# ⛔️ Was die Messung vom 2026-09-17 dieser Mappe vorschreibt

An diesem Tag ist nachgemessen worden, wie ein Modell eine Einzelheit
wiederfindet, die nicht mehr im Kontext steht. Drei Ergebnisse gelten
hier unverändert:

**1. Ein Verzeichnis, das mitwächst, frisst genau den Kontext, den es
sparen soll.** Eine Zeile je Abschnitt wog bei einem langen Verlauf
3 597 Token. Die Eingangsseite dieser Mappe hat deshalb eine
**Obergrenze** und nicht eine Zeile je Kapitel.

**2. Gesucht wird nach einem Wort, nicht nach einer Gliederung.** Mit
Verzeichnis fand das Modell die Stelle in 6 von 9 Läufen und füllte
dabei den Kontext; mit einer Suche in 9 von 9 bei einem Viertel davon.
Die Mappe legt deshalb ein **Stichwortverzeichnis** an, das von einem
Wort auf eine Datei zeigt.

**3. Ein Zeigefinger wird für die Auskunft gehalten.** Wer nur
Dateinamen nennt, bekommt „die Antwort steht in ch03.md" als Antwort.
Jeder Eintrag trägt deshalb den **Satz**, um den es geht, und nicht nur
den Ort.

Aufruf:

```text
python3 TRAINING/korpus/md_zu_mappe.py <md> ... --name <kennung> [--ausgabe <ordner>]
python3 TRAINING/korpus/md_zu_mappe.py --selbsttest
```
"""

from __future__ import annotations

import argparse
import re
import sys
import unicodedata
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import umgebung  # noqa: E402
from buchkorpus import Abschnitt, gliedern, saeubern_inline, trennstriche_heilen  # noqa: E402

VORGABE_AUSGABE = Path(__file__).resolve().parent / "datasets" / "mappen"

# ⛔️ **Die Eingangsseite hat eine Obergrenze, und sie ist der Punkt.**
# Sie steht bei jeder Frage im Kontext; was hier steht, kostet immer.
EINGANG_ZEILEN = 40
# Wie viele Zeichen eine Kapiteldatei höchstens hat, bevor sie geteilt wird.
KAPITEL_ZEICHEN = 12_000
# Wie viele Stichwörter das Verzeichnis höchstens führt.
STICHWOERTER = 200

# Muster, an denen ein Buch eine Sache erklärt.
# ⚑ **Zwischen Wort und Verb darf etwas stehen**, und das ist nicht
# Bequemlichkeit: Buecher schreiben „Die Schranke 1 ist …", „Der
# Rauschnullpunkt eines Laufs ist …". Wer das Verb direkt hinter dem
# Wort verlangt, findet die Haelfte der Erklaerungen nicht, und zwar
# stillschweigend. ⚠️ Zwei Woerter Abstand sind die Grenze; mehr faengt
# an, Satzteile einzusammeln, die nichts erklaeren.
ERKLAERT = re.compile(
    r"^(?:Ein|Eine|Der|Die|Das)?\s*(?P<wort>[A-ZÄÖÜ][\w\-]{3,30})"
    r"(?:\s+\S{1,14}){0,2}\s+"
    r"(?:ist|sind|bezeichnet|heisst|heißt|meint|nennt man|bedeutet)\b"
)


def schlank(t: str) -> str:
    t = unicodedata.normalize("NFKD", t.lower())
    t = "".join(c for c in t if not unicodedata.combining(c))
    t = t.replace("ß", "ss")
    return re.sub(r"[^a-z0-9]+", "-", t).strip("-")[:48] or "abschnitt"


def kapitel_bilden(abschnitte: list[Abschnitt]) -> list[tuple[str, list[Abschnitt]]]:
    """Bündelt Abschnitte unter ihrer obersten eigenen Überschrift.

    ⚑ **Ein Kapitel ist, was das Buch ein Kapitel nennt**, und nicht
    eine Zahl von Zeichen: Wer nach Länge schneidet, trennt eine
    Begründung von ihrer Aussage.
    """
    kapitel: dict[str, list[Abschnitt]] = {}
    for a in abschnitte:
        titel = a.pfad[1] if len(a.pfad) > 1 else a.pfad[0]
        kapitel.setdefault(titel, []).append(a)
    return list(kapitel.items())


def stichwoerter_sammeln(kapitel: list[tuple[str, list[Abschnitt]]]) -> dict[str, tuple[str, str]]:
    """Wort zu (Datei, erklärender Satz).

    ⚠️ **Gefunden über Muster und nicht verstanden.** „X ist ..." trifft
    die Stellen, an denen ein Buch etwas erklärt, und es trifft auch
    „Die Messung ist dann gescheitert". **Das Verzeichnis ist ein
    Wegweiser und keine Autorität**, und deshalb steht der Satz dabei:
    Wer ihn liest, sieht in einer Zeile, ob er am richtigen Ort ist.
    """
    aus: dict[str, tuple[str, str]] = {}
    for nr, (titel, abschnitte) in enumerate(kapitel, 1):
        datei = f"kapitel/{nr:02d}-{schlank(titel)}.md"
        for a in abschnitte:
            for absatz in a.absaetze:
                for satz in re.split(r"(?<=[.!?])\s+", absatz):
                    t = ERKLAERT.match(satz.strip())
                    if not t:
                        continue
                    wort = t.group("wort")
                    if wort.lower() in aus:
                        continue
                    aus[wort.lower()] = (datei, satz.strip()[:200])
                    if len(aus) >= STICHWOERTER:
                        return aus
    return aus


def mappe_bauen(dateien: list[Path], name: str, ziel: Path, mit_code: bool = False) -> dict:
    abschnitte: list[Abschnitt] = []
    for d in sorted(dateien):
        roh, _ = trennstriche_heilen(d.read_text(encoding="utf-8", errors="replace"))
        teile, _ = gliedern(roh, d.name, mit_code)
        for a in teile:
            a.absaetze = [x for x in (saeubern_inline(p) for p in a.absaetze) if x]
        abschnitte.extend(teile)
    if not abschnitte:
        raise SystemExit("[md_zu_mappe] kein Text gefunden")

    kapitel = kapitel_bilden(abschnitte)
    ziel = ziel / name
    (ziel / "kapitel").mkdir(parents=True, exist_ok=True)
    for alt in (ziel / "kapitel").glob("*.md"):
        alt.unlink()

    teile_geschrieben = 0
    eintraege: list[tuple[str, str, int]] = []
    for nr, (titel, teile) in enumerate(kapitel, 1):
        text = [f"# {titel}", ""]
        for a in teile:
            if len(a.pfad) > 2:
                text.append(f"## {' > '.join(a.pfad[2:])}")
                text.append("")
            text.extend(p + "\n" for p in a.absaetze)
        ganz = "\n".join(text).strip() + "\n"
        datei = ziel / "kapitel" / f"{nr:02d}-{schlank(titel)}.md"
        datei.write_text(ganz, encoding="utf-8")
        teile_geschrieben += 1
        # ⚑ Der erste Satz des Kapitels ist der Eintrag, nicht sein Name.
        erster = next(
            (p for a in teile for p in a.absaetze if len(p.split()) > 5),
            titel,
        )
        eintraege.append((datei.name, erster[:160], len(ganz)))

    stich = stichwoerter_sammeln(kapitel)
    (ziel / "stichwoerter.md").write_text(
        "# Stichwörter\n\n"
        "⚑ Ein Wort, die Datei, und der Satz, in dem es erklärt wird.\n\n"
        + "".join(f"- **{w}** ({d}): {s}\n" for w, (d, s) in sorted(stich.items())),
        encoding="utf-8",
    )

    # ⛔️ **Die erste Zeile ist eine Aussage über den Inhalt und keine
    # Statistik.** Sie ist es, die `list_skills` zeigt und die nach einer
    # Verdichtung in der Zusammenfassung steht; „25 Kapitel, 893 Absätze"
    # sagt dort nichts darüber, ob diese Mappe zur Frage passt.
    erster_satz = next(
        (p for _, teile in kapitel for a in teile for p in a.absaetze if len(p.split()) > 8),
        f"Wissensmappe {name}.",
    )
    kopf = [
        f"# {name}",
        "",
        erster_satz[:300],
        "",
        f"Wissensmappe aus {len(dateien)} Datei(en), {len(kapitel)} Kapitel, "
        f"{sum(len(a.absaetze) for a in abschnitte)} Absätze.",
        "",
        "⚑ **So wird hier gesucht:** erst `stichwoerter.md`, dort steht ein Wort",
        "mit der Datei und dem Satz, in dem es erklärt wird. Führt das nicht",
        "weiter, sucht `search_files` nach dem Wort im Ordner `kapitel/`.",
        "",
        "## Kapitel",
        "",
    ]
    for datei, satz, groesse in eintraege[:EINGANG_ZEILEN]:
        kopf.append(f"- `kapitel/{datei}` ({groesse // 1000} kB): {satz}")
    if len(eintraege) > EINGANG_ZEILEN:
        kopf.append(
            f"- … und {len(eintraege) - EINGANG_ZEILEN} weitere; "
            "`list_directory` zeigt sie, `search_files` findet darin"
        )
    (ziel / "MAPPE.md").write_text("\n".join(kopf) + "\n", encoding="utf-8")

    return {
        "kapitel": len(kapitel),
        "stichwoerter": len(stich),
        "eingang_zeichen": len((ziel / "MAPPE.md").read_text(encoding="utf-8")),
        "ordner": str(ziel),
    }


def main() -> int:
    umgebung.pruefen()
    p = argparse.ArgumentParser(description="Markdown zu Wissensmappe")
    p.add_argument("eingabe", nargs="*")
    p.add_argument("--name", default="mappe")
    p.add_argument("--ausgabe", default=str(VORGABE_AUSGABE))
    p.add_argument("--mit-code", action="store_true")
    p.add_argument("--selbsttest", action="store_true")
    args = p.parse_args()
    if args.selbsttest:
        return selbsttest()
    if not args.eingabe:
        p.print_help()
        return 2
    dateien: list[Path] = []
    for e in args.eingabe:
        q = Path(e)
        dateien.extend(sorted(q.rglob("*.md")) if q.is_dir() else [q])
    b = mappe_bauen(dateien, args.name, Path(args.ausgabe), args.mit_code)
    print(
        f"[md_zu_mappe] {b['kapitel']} Kapitel, {b['stichwoerter']} Stichwörter, "
        f"Eingangsseite {b['eingang_zeichen']} Zeichen, nach {b['ordner']}"
    )
    return 0


def selbsttest() -> int:
    import tempfile

    fehler = 0

    def pruefe(b: bool, was: str) -> None:
        nonlocal fehler
        if not b:
            print(f"  FEHLER: {was}")
            fehler += 1

    print("Selbsttest:")
    with tempfile.TemporaryDirectory(prefix="mappe-") as tmp:
        t = Path(tmp)
        md = t / "buch.md"
        kapitel = "\n\n".join(
            f"## Kapitel {i}\n\nDie Schranke {i} ist eine Grenze, die nicht "
            f"ueberschritten wird. Sie gilt fuer Stufe {i} und traegt dort die "
            f"ganze Last des Verfahrens. Der Rauschnullpunkt bezeichnet den "
            f"Abstand, den ein Lauf ohne Gradienten schon erreicht."
            for i in range(1, 60)
        )
        md.write_text("# Das Buch\n\n" + kapitel + "\n", encoding="utf-8")
        b = mappe_bauen([md], "probe", t / "aus")
        ordner = Path(b["ordner"])
        eingang = (ordner / "MAPPE.md").read_text(encoding="utf-8")

        pruefe(b["kapitel"] == 59, f"Kapitel gezählt: {b['kapitel']} statt 59")
        # ⛔️ Die erste Zeile sagt, worum es geht, nicht wie viel es ist.
        erste = [z for z in eingang.splitlines() if z.strip() and not z.startswith("#")][0]
        pruefe(
            "Schranke" in erste and "Kapitel," not in erste,
            f"die Eingangsseite beginnt mit einer Statistik: {erste[:80]}",
        )
        # ⛔️ Die Zusage aus der Messung: die Eingangsseite waechst nicht mit.
        pruefe(
            len([z for z in eingang.splitlines() if z.startswith("- `kapitel/")]) <= EINGANG_ZEILEN,
            "die Eingangsseite listet mehr Kapitel als erlaubt",
        )
        pruefe("und 19 weitere" in eingang, "die Eingangsseite verschweigt, dass sie kürzt")
        # ⚑ Jeder Eintrag traegt einen Satz und nicht nur einen Dateinamen.
        pruefe(
            all("): " in z and len(z.split("): ", 1)[1]) > 20
                for z in eingang.splitlines() if z.startswith("- `kapitel/")),
            "ein Eintrag nennt nur den Ort und nicht die Sache",
        )
        stich = (ordner / "stichwoerter.md").read_text(encoding="utf-8")
        # ⚑ Beide Formen: mit Einschub zwischen Wort und Verb und ohne.
        pruefe("schranke" in stich.lower(), "das Stichwort mit Einschub fehlt")
        pruefe("rauschnullpunkt" in stich.lower(), "das Stichwort ohne Einschub fehlt")
        pruefe(
            len(list((ordner / "kapitel").glob("*.md"))) == 59,
            "es fehlen Kapiteldateien",
        )
        # ⚑ Und die Mappe ist wiederholbar.
        b2 = mappe_bauen([md], "probe", t / "aus2")
        pruefe(
            (Path(b2["ordner"]) / "MAPPE.md").read_bytes() == (ordner / "MAPPE.md").read_bytes(),
            "zwei Läufe ergeben verschiedene Eingangsseiten",
        )

    if fehler:
        print(f"[md_zu_mappe] FAILED: {fehler} Zusage(n) nicht gehalten")
        return 1
    print("[md_zu_mappe] PASSED: Selbsttest ohne Befund")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
