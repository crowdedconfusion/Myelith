#!/usr/bin/env python3
"""Agentenmessung: Erledigt die Werkzeugschleife eine Aufgabe?

# ⚑ Der Unterschied zu Inferenz und Training ist der Pruefer

Dort faellt die Zahl aus dem Modell selbst, hier aus dem Dateisystem:
Ist die Datei entstanden, steht das Richtige darin, nennt die Antwort
das Wort, das nur aus einer Werkzeugantwort stammen kann.

⚑ **Deshalb wird je Auftrag ein frisches Verzeichnis gebaut.** Ein
Auftrag, der in den Ueberresten des vorigen laeuft, kann bestehen, ohne
etwas getan zu haben; genau diese Sorte stiller Erfolg ist das, wovor
die uebrigen Proben dieses Projektes warnen.

# Die drei Stufen, und warum sie getrennt sind

| Stufe | Was sie prueft |
|---|---|
| 1 | Ruft das Modell ueberhaupt ein Werkzeug, und in gueltiger Form |
| 2 | Verwertet es die **Antwort** des Werkzeugs |
| 3 | Veraendert es die Aussenwelt richtig |

Ein Modell kann Stufe 1 koennen und an Stufe 2 scheitern. Beides in
eine Zahl zu werfen ergaebe eine Quote, die nichts erklaert.

Aufruf:

```text
python3 BENCHMARKS/Agent/agentenprobe.py <artefakt> [--stufe N] [--laeufe N]
```

⚑ **Mehrere Laeufe je Auftrag sind sinnvoll**, obwohl das Modell gierig
zieht: Die Werkzeugantwort geht in den naechsten Prompt ein, und schon
ein anderes Verzeichnislisting aendert ihn. Gleiche Eingabe gibt gleiche
Ausgabe, gleiche **Aufgabe** nicht notwendig.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path

WURZEL = Path(__file__).resolve().parents[2]
MYL = WURZEL / "SYSTEM/full-build" / "release" / "myl"
AUFTRAEGE = Path(__file__).resolve().parent / "auftraege.json"


def gerufene_werkzeuge(ausgabe: str) -> list[str]:
    """Welche Werkzeuge der Lauf wirklich gerufen hat.

    ⚑ **Warum das mitgeschrieben wird (2026-09-23).** Diese Probe
    urteilt ueber die **Wirkung**: Ist die Datei entstanden, nennt die
    Antwort das Wort. Das beantwortet „hat es die Aufgabe geloest" und
    nicht „hat es das Werkzeug gerufen", und die beiden fallen genau
    dann auseinander, wenn es interessant wird.

    📌 **Ohne diese Zeile kostet jede Diagnose einen neuen Lauf.** Nach
    der Messreihe vom selben Tag war die Frage „ruft das Modell jedes
    Werkzeug korrekt" aus den Protokollen nicht zu beantworten, weil nur
    das Urteil darin stand.

    `myl` meldet jeden Aufruf als `  → name argumente`.
    """
    namen = []
    for zeile in ausgabe.splitlines():
        z = zeile.strip()
        if z.startswith("→ "):
            namen.append(z[2:].split()[0] if len(z) > 2 else "")
    return [n for n in namen if n]


def einen_lauf(
    artefakt: str, auftrag: dict, schritte: int, deutsch: bool, voll: bool
) -> tuple[bool, str, float, list[str]]:
    """Baut das Verzeichnis, faehrt den Auftrag, urteilt.

    Gibt zurueck: bestanden, der Grund, die Dauer, die gerufenen
    Werkzeuge.
    """
    with tempfile.TemporaryDirectory(prefix="myl-agentenprobe-") as tmp:
        wurzel = Path(tmp)
        for name, inhalt in (auftrag.get("dateien") or {}).items():
            (wurzel / name).parent.mkdir(parents=True, exist_ok=True)
            (wurzel / name).write_text(inhalt, encoding="utf-8")
        # ⚑ **Echte Dateien statt geschriebener** (2026-09-23). Ein Bild
        #   und eine Tonaufnahme lassen sich nicht als Text hinschreiben,
        #   und ohne sie sind `describe_image` und `transcribe_audio`
        #   nicht messbar. Die Quelle ist relativ zur Wurzel des
        #   Repositoriums; ein Pfad daraus hinaus waere ein Auftrag, der
        #   auf dieser Maschine etwas anderes misst als auf der naechsten.
        for name, quelle in (auftrag.get("kopien") or {}).items():
            q = (WURZEL / quelle).resolve()
            if not q.is_file() or WURZEL not in q.parents:
                raise SystemExit(
                    f"[agentenprobe] {auftrag['name']}: {quelle} fehlt oder liegt "
                    "ausserhalb des Repositoriums"
                )
            (wurzel / name).parent.mkdir(parents=True, exist_ok=True)
            (wurzel / name).write_bytes(q.read_bytes())

        # ⚑ Die Einstellungen kommen ueber Schalter und nicht ueber die
        # Ablage des Nutzers: Eine Messung, die die Konfiguration des
        # Rechners umschreibt, ist keine Messung, sondern ein Eingriff.
        befehl = [
            str(MYL), "agent", artefakt, auftrag["auftrag"],
            "--schritte", str(schritte), "--bezeugtes",
            "--wurzel", str(wurzel),
        ]
        if auftrag.get("schreiben"):
            befehl.append("--schreiben")
        # ⚑ **Der Vergleichsschalter, und er ist der eigentliche Zweck
        # dieses Laufs.** Bis zum 2026-09-09 sagte der Client dem Modell
        # seine Werkzeuge auf Deutsch an, mit deutschen Namen und einer
        # Paraphrase der Vorlage, auf die es geschliffen wurde. Ob das
        # etwas kostet, ist eine Messfrage, und diese Probe ist die
        # Stelle, an der sie beantwortet wird.
        if deutsch:
            befehl.append("--deutsch")
        if voll:
            # ⛔️ **`Advanced` und nicht `voll`** (Fund 437, 2026-09-23).
            #    `myl` kennt nur `Base`, `Advanced` und `1337`; auf
            #    `voll` warnte es und **fuhr mit der eingestellten Kiste
            #    weiter**. Die Probe schrieb darueber „Werkzeugsatz:
            #    voll", gemessen wurde Base, und `run_command` war in
            #    keinem einzigen Lauf im Angebot.
            #
            # 📌 **Eine Messung, die ihren eigenen Aufbau falsch
            #    benennt, ist schlimmer als keine.** Gefunden hat es
            #    erst die Abdeckungsmessung, weil dort ein Werkzeug
            #    fehlte, das es haette geben muessen.
            befehl += ["--werkzeuge", "Advanced"]

        anfang = time.monotonic()
        try:
            fertig = subprocess.run(
                befehl, capture_output=True, text=True, timeout=600
            )
            ausgabe = fertig.stdout + fertig.stderr
        except subprocess.TimeoutExpired:
            return False, "ZEITUEBERSCHREITUNG nach 600 s", time.monotonic() - anfang, []
        dauer = time.monotonic() - anfang

        # --- Das Urteil -----------------------------------------------
        gruende = []
        for wort in auftrag.get("antwort_enthaelt") or []:
            if wort not in ausgabe:
                gruende.append(f"die Antwort nennt {wort!r} nicht")
        for name, wort in (auftrag.get("datei_enthaelt") or {}).items():
            p = wurzel / name
            if not p.is_file():
                gruende.append(f"{name} ist nicht entstanden")
            elif wort not in p.read_text(encoding="utf-8", errors="replace"):
                gruende.append(f"{name} enthaelt {wort!r} nicht")

        # ⚑ **Was ueberlebt haben muss.** Genau das unterscheidet
        # `edit_file` von `write_file`: Ein Agent, der die ganze Datei
        # neu schreibt, trifft die verlangte Stelle vielleicht und
        # loescht dabei den Rest. Ohne diese Pruefung sae ein solcher
        # Lauf wie ein bestandener aus.
        for name, woerter in (auftrag.get("datei_enthaelt_auch") or {}).items():
            p = wurzel / name
            if not p.is_file():
                gruende.append(f"{name} fehlt")
                continue
            inhalt = p.read_text(encoding="utf-8", errors="replace")
            for wort in woerter:
                if wort not in inhalt:
                    gruende.append(f"{name} hat {wort!r} verloren")

        werkzeuge = gerufene_werkzeuge(ausgabe)
        # ⛔️ **Ein erwartetes Werkzeug ist eine eigene Zusicherung.**
        #   Bei der Abdeckungsmessung ist die Wirkung manchmal nicht
        #   vorhersagbar (was ein Sehmodell ueber ein Bild sagt, weiss
        #   niemand vorher), und ein Auftrag ohne pruefbaren Ausgang
        #   bestuende immer. **Eine Pruefung, die nichts auswaehlt, sieht
        #   aus wie eine, die nichts ablehnt.**
        erwartet = auftrag.get("werkzeug_erwartet")
        if erwartet and erwartet not in werkzeuge:
            gerufen = ", ".join(werkzeuge) if werkzeuge else "keines"
            gruende.append(f"{erwartet} wurde nicht gerufen (gerufen: {gerufen})")
        if gruende:
            return False, "; ".join(gruende), dauer, werkzeuge
        return True, "", dauer, werkzeuge


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("artefakt")
    p.add_argument("--stufe", type=int, default=0, help="nur diese Stufe")
    p.add_argument("--laeufe", type=int, default=1, help="Wiederholungen je Auftrag")
    p.add_argument("--schritte", type=int, default=6)
    p.add_argument(
        "--werkzeuge",
        choices=["knapp", "voll"],
        default="knapp",
        help="knapp = die drei bis 2026-09-09, voll = mit Suche und Aendern",
    )
    p.add_argument(
        "--deutsch",
        action="store_true",
        help="Werkzeuge deutsch ansagen (die Fassung vor dem 2026-09-09)",
    )
    p.add_argument("--json", type=Path, help="Ergebnis zusaetzlich hierhin")
    # ⚑ **Ein zweiter Auftragssatz, und das ist kein Beiwerk.** Die
    #   gestufte Messung fragt „loest es die Aufgabe"; die Abdeckung
    #   fragt „ruft es jedes Werkzeug". Zwei Fragen, zwei Dateien: In
    #   einen Satz geworfen ergaeben sie eine Quote, die keine von beiden
    #   beantwortet.
    p.add_argument(
        "--auftraege",
        type=Path,
        default=AUFTRAEGE,
        help="ein anderer Auftragssatz, etwa werkzeugabdeckung.json",
    )
    a = p.parse_args()

    if not MYL.is_file():
        print(f"[agentenprobe] {MYL} fehlt; erst `cargo build --release` im Client.")
        return 2
    if not Path(a.artefakt).is_dir():
        print(f"[agentenprobe] Artefaktverzeichnis {a.artefakt} fehlt.")
        return 2

    auftraege = json.loads(a.auftraege.read_text(encoding="utf-8"))["auftrag"]
    # 📌 **Auftraege, die ein Werkzeug brauchen, das nicht angeboten
    # wird, werden uebersprungen und nicht als Fehlschlag gezaehlt.**
    # Ein Auftrag, den das Modell mangels Werkzeug nicht loesen KANN,
    # waere keine Aussage ueber das Modell, sondern ueber die Auswahl,
    # und er verschoebe die Quote des knappen Arms nach unten, ohne
    # dass jemand den Grund saehe.
    if a.werkzeuge != "voll":
        braucht_voll = [x for x in auftraege if x.get("satz") == "voll"]
        auftraege = [x for x in auftraege if x.get("satz") != "voll"]
        if braucht_voll:
            print(
                f"[agentenprobe] {len(braucht_voll)} Auftrag/Auftraege uebersprungen, "
                "sie brauchen --werkzeuge voll: "
                + ", ".join(x["name"] for x in braucht_voll)
            )
    if a.stufe:
        auftraege = [x for x in auftraege if x.get("stufe") == a.stufe]
    if not auftraege:
        print("[agentenprobe] kein Auftrag passt zur Auswahl")
        return 2

    print(f"[agentenprobe] {len(auftraege)} Auftraege, {a.laeufe} Lauf/Laeufe je Auftrag")
    print(f"[agentenprobe] Artefakt: {a.artefakt}")
    print(f"[agentenprobe] Ansageform: {'deutsch' if a.deutsch else 'amtlich'}")
    print(f"[agentenprobe] Werkzeugsatz: {a.werkzeuge}")
    ergebnisse = []
    je_stufe: dict[int, list[bool]] = {}

    for auf in auftraege:
        for i in range(a.laeufe):
            ok, grund, dauer, werkzeuge = einen_lauf(
                a.artefakt, auf, a.schritte, a.deutsch, a.werkzeuge == "voll"
            )
            je_stufe.setdefault(auf.get("stufe", 0), []).append(ok)
            ergebnisse.append(
                {
                    "name": auf["name"],
                    "stufe": auf.get("stufe", 0),
                    "lauf": i + 1,
                    "bestanden": ok,
                    "grund": grund,
                    "sekunden": round(dauer, 1),
                    # ⚑ In der Reihenfolge des Laufs, mit Wiederholungen:
                    #   Wer zweimal dasselbe ruft, hat etwas anderes
                    #   getan als wer es einmal ruft.
                    "werkzeuge": werkzeuge,
                }
            )
            marke = "ok  " if ok else "NEIN"
            zusatz = f"  ({grund})" if grund else ""
            # ⚑ **Die gerufenen Werkzeuge stehen in der Zeile**, denn
            #   genau sie unterscheiden „falsches Werkzeug" von „richtiges
            #   Werkzeug, falsch verwertet".
            gerufen = f"  [{' '.join(werkzeuge)}]" if werkzeuge else "  [kein Aufruf]"
            print(f"[agentenprobe] {marke} Stufe {auf.get('stufe', 0)} "
                  f"{auf['name']:<22} {dauer:5.1f} s{gerufen}{zusatz}")

    print("[agentenprobe] ---")
    for stufe in sorted(je_stufe):
        w = je_stufe[stufe]
        print(f"[agentenprobe] Stufe {stufe}: {sum(w)} von {len(w)}")
    alle = [e["bestanden"] for e in ergebnisse]
    print(f"[agentenprobe] gesamt: {sum(alle)} von {len(alle)}")

    if a.json:
        a.json.write_text(
            json.dumps(
                {"artefakt": a.artefakt, "schritte": a.schritte, "laeufe": ergebnisse},
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        print(f"[agentenprobe] geschrieben: {a.json}")

    # ⚑ **Rueckgabe 0 auch bei Fehlschlaegen.** Dies ist eine Messung
    # und keine Pruefung: Eine Quote von drei Sechsteln ist ein
    # Ergebnis und kein Fehler des Laufs. Wer eine Schranke will, liest
    # die JSON-Datei.
    return 0


if __name__ == "__main__":
    sys.exit(main())
