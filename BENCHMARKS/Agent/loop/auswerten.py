#!/usr/bin/env python3
"""Stand und Auswertung des Loop-Szenarios, jederzeit aufrufbar.

    python3 BENCHMARKS/Agent/loop/auswerten.py <laufordner>
    python3 BENCHMARKS/Agent/loop/auswerten.py <laufordner> --bericht <datei.md>

Waehrend des Laufs ist es die Standanzeige, danach die Auswertung: dieselben
Pruefungen, damit Zwischenstand und Endwertung nicht auseinanderlaufen.
Es veraendert im Laufordner nichts; das Skript des Agenten wird in einer Kopie
ausgefuehrt.
"""
import glob
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

HIER = os.path.dirname(os.path.abspath(__file__))
VORLAGE = os.path.join(HIER, "vorlage")

# Was als Verletzung eines Bereichs gelesen wird, und was es aufhebt.
VERLETZT = re.compile(r"(überschr|ausserhalb|außerhalb|verletz|zu warm|zu hoch|über dem|oberhalb|abweich)", re.I)
VERNEINT = re.compile(r"(nicht|kein|innerhalb|eingehalten|im zulässigen|im erlaubten)", re.I)
ZITAT = re.compile(r"\[Q:\s*([\w\-./]+\.md)(?:,\s*Abschnitt\s*(\d+))?\]")


def wahrheit():
    """Die richtige Statistik, aus der unveraenderten Vorlage."""
    werte = {}
    with open(os.path.join(VORLAGE, "daten", "messwerte.csv"), encoding="utf-8") as f:
        next(f)
        for z in f:
            _, sensor, wert = z.strip().split(";")
            if wert != "n/a":
                werte.setdefault(sensor, []).append(float(wert.replace(",", ".")))
    return {s: (len(w), f"{sum(w) / len(w):.1f}", f"{min(w):.1f}", f"{max(w):.1f}") for s, w in werte.items()}


def tabelle(text):
    """Zeilen `| T1 | 46 | 20,4 | ... |` aus einer Markdown-Tabelle."""
    aus = {}
    for z in text.splitlines():
        teile = [t.strip() for t in z.strip().strip("|").split("|")]
        if len(teile) >= 5 and re.fullmatch(r"T\d", teile[0]):
            aus[teile[0]] = tuple(t.replace(",", ".") for t in teile[1:5])
    return aus


def lesen(pfad):
    try:
        with open(pfad, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def aufgabe_auswertung(arbeit):
    pruef = []
    csv_gleich = lesen(os.path.join(arbeit, "daten", "messwerte.csv")) == lesen(os.path.join(VORLAGE, "daten", "messwerte.csv"))
    pruef.append(("Messreihe unverändert", csv_gleich, ""))
    # Das Skript in einer Kopie ausfuehren: Der Arbeitsordner ist der Beleg.
    with tempfile.TemporaryDirectory() as t:
        kopie = os.path.join(t, "a")
        shutil.copytree(arbeit, kopie, ignore=shutil.ignore_patterns(".AGENT"))
        shutil.copy(os.path.join(VORLAGE, "daten", "messwerte.csv"), os.path.join(kopie, "daten", "messwerte.csv"))
        # 📌 Die Datei des Agenten muss weg, sonst „erzeugt" ein gescheitertes
        #    Skript die Tabelle, die der Agent von Hand geschrieben hat
        #    (gefunden in Lauf 3 des 8B).
        vorher = os.path.join(kopie, "ergebnis", "statistik.md")
        if os.path.exists(vorher):
            os.remove(vorher)
        r = subprocess.run([sys.executable, "daten/auswertung.py"], cwd=kopie, capture_output=True, text=True, timeout=60)
        laeuft = r.returncode == 0
        fehler = (r.stderr.strip().splitlines() or [""])[-1]
        erzeugt = lesen(os.path.join(kopie, "ergebnis", "statistik.md")) or ""
    pruef.append(("Skript läuft ohne Fehler", laeuft, fehler))
    stat = lesen(os.path.join(arbeit, "ergebnis", "statistik.md"))
    pruef.append(("ergebnis/statistik.md vorhanden", stat is not None, ""))
    soll = wahrheit()
    ist = tabelle(stat or "")
    for s, (n, mittel, mn, mx) in sorted(soll.items()):
        z = ist.get(s)
        richtig = z is not None and z[0] == str(n) and z[1:] == (mittel, mn, mx)
        pruef.append((f"{s} richtig (soll {n} | {mittel} | {mn} | {mx})", richtig, "ist " + " | ".join(z) if z else "fehlt"))
    pruef.append(("Skript erzeugt dieselbe Tabelle", laeuft and bool(tabelle(erzeugt)) and tabelle(erzeugt) == ist, ""))
    return pruef


def aufgabe_bericht(arbeit):
    pruef = []
    text = lesen(os.path.join(arbeit, "bericht", "sensorbericht.md"))
    da = text is not None
    pruef.append(("bericht/sensorbericht.md vorhanden", da, ""))
    text = text or ""
    zeilen = text.splitlines()
    zitate = ZITAT.findall(text)
    vorhanden = set(os.listdir(os.path.join(arbeit, "quellen"))) | {"ergebnis/statistik.md"}
    unbekannt = sorted({d for d, _ in zitate if d not in vorhanden})
    pruef.append(("mindestens drei Quellenangaben im Hausformat", len(zitate) >= 3, f"{len(zitate)} gefunden"))
    pruef.append(("jede Quellenangabe nennt eine vorhandene Datei", bool(zitate) and not unbekannt, ", ".join(unbekannt)))
    pruef.append(("Abschnitt „## Quellen“ am Ende", bool(re.search(r"^##\s*Quellen\s*$", text, re.M)), ""))
    veraltet = [d for d, _ in zitate if d in ("grenzwerte-2024.md", "haeufige-fragen.md")]
    # ⚠️ Die verneinten Pruefungen gelten nur mit Bericht; sonst waeren sie
    #    in einem leeren Ordner erfuellt.
    pruef.append(("keine veraltete Unterlage als Beleg", da and not veraltet, ", ".join(veraltet)))

    def zeilen_mit(muster):
        return [z for z in zeilen if re.search(muster, z, re.I)]

    kuehl = [z for z in zeilen_mit(r"T3|Kühlraum") if VERLETZT.search(z) and not VERNEINT.search(z)]
    pruef.append(("Kühlraum (T3) als verletzt erkannt", bool(kuehl), ""))
    pruef.append(("Höchstwert 9,4 °C im Kühlraum genannt", bool(re.search(r"9[,.]4", text)), ""))
    server = [z for z in zeilen_mit(r"T4|Serverraum") if VERLETZT.search(z) and not VERNEINT.search(z)]
    pruef.append(("Serverraum (T4) nicht fälschlich als verletzt", da and not server, server[0][:80] if server else ""))
    defekt = zeilen_mit(r"(T2|Büro).*(defekt|nicht verwend|ausgeschlossen|entfällt|nicht bewert)|(defekt|nicht verwend|ausgeschlossen).*(T2|Büro)")
    pruef.append(("Sensor T2 als defekt ausgeklammert", bool(defekt), ""))
    pruef.append(("Maßnahmen Kühlraum: Qualitätssicherung", bool(re.search(r"Qualitätssicherung", text)), ""))
    pruef.append(("Maßnahmen Kühlraum: Ware sperren oder Tür prüfen", bool(re.search(r"sperr|Tür", text, re.I)), ""))
    return pruef


def werkzeuge(lauf):
    """Werkzeugaufrufe aus `[werkzeug] name argumente` in loop.log."""
    zaehler, skills, gelesen, quellen = {}, [], [], 0
    for z in (lesen(os.path.join(lauf, "loop.log")) or "").splitlines():
        m = re.match(r"\[werkzeug\] (\S+)\s*(.*)", z)
        if m:
            zaehler[m.group(1)] = zaehler.get(m.group(1), 0) + 1
            if m.group(1) == "learn_skill":
                n = re.search(r'"name"\s*:\s*"([^"/]+)', m.group(2))
                skills.append(n.group(1) if n else m.group(2)[:40])
            # ⚑ Ein Projektskill liegt im Arbeitsordner und laesst sich auch
            #   mit read_file lesen; das ist ein zweiter, legitimer Weg.
            # ⚑ Recherche heisst: in quellen/ gelesen oder gesucht. Ein
            #   read_file in daten/ ist keine (Lauf 4 zaehlte es falsch mit).
            if m.group(1) in ("read_file", "list_directory") and "quellen" in m.group(2):
                quellen += 1
            if m.group(1) == "search_files" and not re.search(r"auswertung|sensor|zeile|werte|len\(", m.group(2)):
                quellen += 1
            if m.group(1) == "read_file":
                n = re.search(r'skills/([\w-]+)/SKILL\.md', m.group(2))
                if n:
                    gelesen.append(n.group(1))
    return zaehler, skills, gelesen, quellen


def tasks(lauf):
    aus = []
    for p in sorted(glob.glob(os.path.join(lauf, "cfg", "vorhaben", "*", "vorhaben.json"))):
        with open(p, encoding="utf-8") as f:
            v = json.load(f)
        tagebuch = lesen(os.path.join(os.path.dirname(p), "tagebuch.md")) or ""
        aus.append((v, tagebuch))
    return aus


def zustand(v):
    z = v["zustand"]
    return z["art"] + (f": {z['grund']}" if z.get("grund") else "")


def main(args):
    if not args:
        print(__doc__)
        return 2
    lauf = os.path.abspath(args[0])
    arbeit = os.path.join(lauf, "arbeit")
    info = json.loads(lesen(os.path.join(lauf, "lauf.json")) or "{}")
    starter = (lesen(os.path.join(lauf, "starter.log")) or "").strip().splitlines()
    ende = next((z for z in reversed(starter) if "Fertig:" in z), None)
    minuten = (time.time() - info.get("start", time.time())) / 60
    # Ein beendeter Lauf hat seine Dauer im starter.log, nicht bis jetzt.
    dauer = next((re.search(r"nach ([\d.]+) min", z) for z in reversed(starter) if "endete" in z), None)
    if ende and dauer:
        minuten = float(dauer.group(1))
    zaehler, gelernt, gelesen, quellen = werkzeuge(lauf)
    a1, a2 = aufgabe_auswertung(arbeit), aufgabe_bericht(arbeit)
    alle = a1 + a2
    gesamt = sum(1 for _, ok, _ in alle if ok)

    md = [f"# Loop-Szenario: {os.path.basename(info.get('artefakt', '?'))}", ""]
    md.append(f"- **Stand:** {time.strftime('%Y-%m-%d %H:%M')}, {minuten:.0f} min nach dem Start"
              + (f", beendet ({ende.split('Fertig: ')[1]})" if ende else ", läuft noch"))
    md.append(f"- **Einstellungen:** {info.get('runden')} Runden je Task, {info.get('schritte')} Schritte je Runde")
    md.append(f"- **Prüfungen bestanden:** {gesamt} von {len(alle)}")
    md.append("")
    md.append("## Tasks")
    md.append("")
    md.append("| Task | Zustand | Runden | Laufzeit | Ergebnis |")
    md.append("|---|---|---|---|---|")
    for v, _ in tasks(lauf):
        md.append(f"| {v['ziel'][:50]}… | {zustand(v)} | {v['runden']} | {v['laufzeit'] // 60} min | {(v.get('ergebnis') or '')[:60]} |")
    md.append("")
    for titel, pruef in (("Aufgabe 1: Auswertung reparieren", a1), ("Aufgabe 2: Bericht mit Recherche", a2)):
        md.append(f"## {titel}")
        md.append("")
        for name, ok, zusatz in pruef:
            md.append(f"- {'✅' if ok else '❌'} {name}" + (f" ({zusatz})" if zusatz else ""))
        md.append("")
    md.append("## Werkzeuge")
    md.append("")
    md.append("| Werkzeug | Aufrufe |")
    md.append("|---|---|")
    for n, k in sorted(zaehler.items(), key=lambda x: -x[1]):
        md.append(f"| `{n}` | {k} |")
    md.append("")
    md.append(f"- **Testskill mit learn_skill gelernt:** {'✅' if 'hausregeln-quellen' in gelernt else '❌'} (gelernt: {', '.join(sorted(set(gelernt))) or 'keiner'})")
    md.append(f"- **Testskill mit read_file gelesen:** {'✅' if 'hausregeln-quellen' in gelesen else '❌'}")
    md.append(f"- **Selbst nach Skills gesucht:** {'✅' if zaehler.get('search_skill') else '❌'}")
    md.append(f"- **Recherche in quellen/:** {'✅' if quellen else '❌'} ({quellen} Aufrufe in oder nach den Unterlagen)")
    md.append(f"- **Befehle ausgeführt:** {'✅' if zaehler.get('run_command') else '❌'}")
    text = "\n".join(md) + "\n"
    print(text)
    if "--bericht" in args:
        ziel = args[args.index("--bericht") + 1]
        with open(ziel, "w", encoding="utf-8") as f:
            f.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
