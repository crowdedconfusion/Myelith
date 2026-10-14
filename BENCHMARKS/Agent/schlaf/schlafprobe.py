#!/usr/bin/env python3
"""Die Schlafprobe: ein Projekt ueber Wochen, einmal mit Schlaf, einmal ohne.

    python3 BENCHMARKS/Agent/schlaf/schlafprobe.py <artefakt> <laufordner> [--saat N] [--nur-schlaf]

Legt im Laufordner an:
  mit/        Arbeitsordner, der nach jedem Schlaftag schlaeft
  ohne/       derselbe Ordner, der nie schlaeft
  cfg/        eigene Einstellungen (MYL_EINSTELLUNGEN); die echten bleiben unberuehrt
  antworten/  die ganze Ausgabe jedes Auftrags, je Bedingung
  ergebnis.json  alles Gemessene; die letzte Zeile der Ausgabe ist das Urteil

Ablauf (`szenario.json`): An jedem Tag kommen die Mitschnitte dieses Tages in
beide Ordner. Der Ordner `mit` schlaeft an jedem Schlaftag (`myl schlafen
--tag`), und an den Abruftagen fragt ein Auftrag nach seinem Gedaechtnis
(`myl gedaechtnis --auftrag`), wie es ein echter Lauf taete. Nach dem letzten
Tag prueft die Probe das Gedaechtnis gegen feste Erwartungen und stellt beiden
Ordnern dieselben Fragen ueber `myl agent` (gierig, ohne Denken, feste Saat).

⚑ **Ohne Schlaf entsteht kein Gedaechtnis.** Verglichen wird deshalb, was
der Agent danach weiss und tut, mit denselben Mitschnitten auf der Platte.
⚑ Eine Frage gilt als bestanden, wenn jedes Muster aus `muss` in der Antwort
steht und keines aus `verboten`. Die Muster sind grob; die ganzen Antworten
liegen unter `antworten/` und gehoeren gelesen.

Kein pytest: ein eigenstaendiges Skript wie die anderen Proben hier.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import time

HIER = os.path.dirname(os.path.abspath(__file__))
WURZEL = os.path.abspath(os.path.join(HIER, "..", "..", ".."))
MYL = os.path.join(WURZEL, "SYSTEM", "full-build", "release", "myl")
SZENARIO = os.path.join(HIER, "szenario.json")

# ⚑ Fest und ausdruecklich, damit die Reihe vergleichbar bleibt.
SCHRITTE = 6
TOKEN = 1600
FRIST = 900


def zahl(args, name, vorgabe):
    if name in args:
        return int(args[args.index(name) + 1])
    return vorgabe


def mitschnitt(ordner, sitzung, tag, nachrichten):
    """Schreibt einen Mitschnitt im Format des Clients (Kopf, Verzeichnis, Verlauf)."""
    sekunden = tag * 86400 + 12 * 3600
    zeit = time.strftime("%Y-%m-%d %H:%M:%S UTC", time.gmtime(sekunden))
    verlauf = []
    eintraege = []
    # Kopf (4 Zeilen), Leerzeile, „## Verzeichnis“, Leerzeile, je Nachricht
    # eine Zeile, Leerzeile, „## Verlauf“: Erst danach beginnt der Verlauf.
    zeile = 4 + 1 + 1 + 1 + len(nachrichten) + 1 + 1 + 1
    for rolle, text in nachrichten:
        von = zeile
        block = [f"### {rolle}", ""] + text.split("\n")
        verlauf += block
        zeile += len(block)
        eintraege.append(f"{von}-{zeile - 1} {rolle} {text.splitlines()[0][:60]}")
    kopf = [
        f"# Mitschnitt {zeit}",
        f"Sitzung: {sitzung}",
        "Modell: schlafprobe",
        f"Nachrichten: {len(nachrichten)}, Zeilen: {zeile - 1}",
        "",
        "## Verzeichnis",
        "",
    ]
    inhalt = kopf + eintraege + ["", "## Verlauf"] + verlauf
    d = os.path.join(ordner, ".AGENT", sitzung)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, f"verlauf-{sekunden}.md"), "w", encoding="utf-8") as f:
        f.write("\n".join(inhalt) + "\n")


def gedaechtnis(ordner):
    p = os.path.join(ordner, ".AGENT", "gedaechtnis", "gedaechtnis.json")
    if not os.path.isfile(p):
        return {"eintraege": []}
    with open(p, encoding="utf-8") as f:
        return json.load(f)


def aktiv(e):
    return e["zustand"].get("stand") == "aktiv"


def pruefen(p, ordner, umgebung, material):
    """Eine Gedaechtnispruefung; gibt (bestanden, was gesehen wurde)."""
    g = gedaechtnis(ordner)["eintraege"]
    art = p["art"]
    m = re.compile(p.get("muster", "x^"))
    if art == "vertraut_mit":
        treffer = [e["text"] for e in g if aktiv(e) and e["vertraut"] and m.search(e["text"])]
        return bool(treffer), treffer
    if art == "nie_vertraut":
        treffer = [e["text"] for e in g if e["vertraut"] and m.search(e["text"])]
        return not treffer, treffer
    if art == "kein_aktives_vertrautes":
        treffer = [e["text"] for e in g if aktiv(e) and e["vertraut"] and m.search(e["text"])]
        return not treffer, treffer
    if art == "abgeloest_oder_aufgehoben":
        # Die alte gilt als abgeloest, wenn sie nicht mehr aktiv ist oder eine
        # aktive vertraute sie ausdruecklich aufhebt (so bleibt eine
        # zusammengesetzte stehen, deren Rest nicht gelang).
        alt = [e["text"] for e in g if aktiv(e) and e["vertraut"] and m.search(e["text"])]
        auf = [e["text"] for e in g if aktiv(e) and e["vertraut"] and re.search(p["aufhebung"], e["text"])]
        return (not alt or bool(auf)), alt + auf
    if art == "kein_aktives":
        treffer = [e["text"] for e in g if aktiv(e) and m.search(e["text"])]
        return not treffer, treffer
    if art == "hoechstens_aktive_vertraute":
        treffer = [e["text"] for e in g if aktiv(e) and e["vertraut"] and m.search(e["text"])]
        return len(treffer) <= p["anzahl"], treffer
    if art == "zahlen_belegt":
        belegt = set(re.findall(r"\d+", material))
        falsch = [e["text"] for e in g if any(z not in belegt for z in re.findall(r"\d+", e["text"]))]
        return not falsch, falsch
    if art in ("auftrag_enthaelt", "auftrag_ohne"):
        # ⚠️ Ohne Vermerk als Abruf: Eine Pruefung soll nichts staerken.
        block = myl_block(ordner, p["auftrag"], umgebung, vermerken=False)
        gefunden = bool(m.search(block))
        return (gefunden if art == "auftrag_enthaelt" else not gefunden), [block]
    raise SystemExit(f"unbekannte Pruefung: {art}")


def myl_block(ordner, auftrag, umgebung, vermerken=True):
    """Der Erinnerungsblock, den ein Auftrag bekaeme."""
    if not vermerken:
        # Eine Kopie des Gedaechtnisses, damit der Abruf nirgends zaehlt.
        kopie = ordner + "-pruefkopie"
        shutil.rmtree(kopie, ignore_errors=True)
        shutil.copytree(os.path.join(ordner, ".AGENT", "gedaechtnis"), os.path.join(kopie, ".AGENT", "gedaechtnis"))
        ordner = kopie
    r = subprocess.run([MYL, "gedaechtnis", ordner, "--auftrag", auftrag], env=umgebung, capture_output=True, text=True)
    if not vermerken:
        shutil.rmtree(ordner, ignore_errors=True)
    return r.stdout


def fragen(ordner, artefakt, saat, umgebung, ziel, frist=FRIST):
    with open(SZENARIO, encoding="utf-8") as f:
        sz = json.load(f)
    aus = []
    os.makedirs(ziel, exist_ok=True)
    for i, q in enumerate(sz["fragen"]):
        befehl = [
            MYL, "agent", artefakt, q["auftrag"],
            "--schritte", str(SCHRITTE), "--wurzel", ordner, "--werkzeuge", "Advanced",
            "--saat", str(saat), "--token", str(TOKEN), "--ohne-denken", "--gierig",
        ]
        anfang = time.monotonic()
        try:
            r = subprocess.run(befehl, env=umgebung, capture_output=True, text=True, timeout=frist)
            stdout, stderr = r.stdout, r.stderr
        except subprocess.TimeoutExpired:
            stdout, stderr = "", f"ZEITUEBERSCHREITUNG nach {frist} s"
        dauer = time.monotonic() - anfang
        with open(os.path.join(ziel, f"{i + 1:02d}-{q['name']}.txt"), "w", encoding="utf-8") as f:
            f.write(f"$ {' '.join(befehl[1:4])}\n\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}")
        # ⚑ **Nur die Schlussantwort**: Werkzeugzeilen („  → …“, „  ← …“)
        #   tragen Systemmeldungen, deren Woerter sonst mitzaehlten
        #   (gemessen: eine englische Meldung zu `write_file` liess einen
        #   deutschen Bericht als englisch durchgehen).
        zeilen = stdout.strip().splitlines()
        letzte = max([i for i, z in enumerate(zeilen) if z.lstrip().startswith(("→", "←"))], default=-1)
        antwort = "\n".join(zeilen[letzte + 1:]).strip()
        fehlt = [m for m in q["muss"] if not re.search(m, antwort)]
        verbot = [m for m in q["verboten"] if re.search(m, antwort)]
        # `myl agent` meldet jeden Aufruf als Zeile „  → name argumente“.
        werkzeuge = re.findall(r"^\s*→ (\S+)", stdout + stderr, re.M)
        aus.append({
            "name": q["name"],
            "bestanden": not fehlt and not verbot,
            "fehlt": fehlt,
            "verboten_gefunden": verbot,
            "sekunden": round(dauer, 1),
            "werkzeuge": werkzeuge,
            "antwort": antwort[-600:],
        })
        print(f"    {'ok  ' if aus[-1]['bestanden'] else 'FEHL'} {q['name']} ({dauer:.0f} s)"
              + (f" fehlt {fehlt}" if fehlt else "") + (f" verboten {verbot}" if verbot else ""), flush=True)
    return aus


def main(args):
    if len(args) < 2:
        print(__doc__)
        return 2
    artefakt = os.path.abspath(args[0])
    lauf = os.path.abspath(args[1])
    saat = zahl(args, "--saat", 20261013)
    nur_schlaf = "--nur-schlaf" in args
    if not os.path.isfile(MYL):
        print(f"myl fehlt: {MYL}\n  cd CLIENT/myl-client && cargo build --release --bin myl")
        return 1
    if not os.path.isfile(os.path.join(artefakt, "model_config.json")):
        print(f"kein Artefakt: {artefakt}")
        return 1
    if os.path.exists(lauf):
        print(f"{lauf} gibt es schon; ein Lauf beginnt in einem frischen Ordner")
        return 1
    # ⚑ Ausserhalb des Repositoriums, wie bei der Loop-Probe: Ein Lauf legt
    #   Mitschnitte, Gedaechtnis und Antworten ab, und die gehoeren nicht in
    #   den Baum.
    if os.path.commonpath([lauf, WURZEL]) == WURZEL:
        print(f"{lauf} liegt im Repositorium; bitte einen Ordner ausserhalb nennen")
        return 1
    with open(SZENARIO, encoding="utf-8") as f:
        sz = json.load(f)

    mit, ohne, cfg = (os.path.join(lauf, n) for n in ("mit", "ohne", "cfg"))
    for d in (mit, ohne, cfg):
        os.makedirs(d)
    umgebung = dict(os.environ, MYL_EINSTELLUNGEN=os.path.join(cfg, "client.json"), MYELITH_WURZEL=WURZEL)
    for feld, wert in [
        ("modell.artefakt", artefakt),
        ("agent.warnung", "aus"),
        ("agent.modus", "auto"),
        ("schlaf.an", "an"),
        # ⚑ Auch auf dem Akku: Eine Messung, die je nach Netzteil anders
        #   ausfaellt, misst den Stecker mit.
        ("schlaf.nur_am_netz", "aus"),
    ]:
        r = subprocess.run([MYL, "setzen", feld, str(wert)], env=umgebung, capture_output=True, text=True)
        if r.returncode != 0:
            print(f"myl setzen {feld}: {r.stderr.strip()}")
            return 1

    # ⚑ Tag 0 ist heute: Die Tage des Szenarios zaehlen ab hier.
    heute = int(time.time() // 86400)
    material = "\n".join(t for tag in sz["tage"] for _, t in tag["nachrichten"])
    schlaefe = []
    pruefungen = []
    letzter = max([t["tag"] for t in sz["tage"]] + sz["schlaftage"] + [a["tag"] for a in sz["abrufe"]])
    print(f"[schlafprobe] {sz['name']}: {letzter + 1} Tage, Saat {saat}", flush=True)
    for tag in range(letzter + 1):
        for t in sz["tage"]:
            if t["tag"] == tag:
                for ordner in (mit, ohne):
                    mitschnitt(ordner, t["sitzung"], heute + tag, t["nachrichten"])
        for a in sz["abrufe"]:
            if a["tag"] == tag:
                block = myl_block(mit, a["auftrag"], umgebung)
                print(f"  Tag {tag}: Abruf „{a['auftrag']}“: {len(block.splitlines()) - 1} Erinnerungen", flush=True)
        if tag in sz["schlaftage"]:
            anfang = time.monotonic()
            r = subprocess.run([MYL, "schlafen", mit, "--tag", str(heute + tag)], env=umgebung, capture_output=True, text=True)
            dauer = time.monotonic() - anfang
            zeilen = r.stdout.strip().splitlines()
            aufrufe = re.search(r"(\d+) Modellaufrufe", r.stdout)
            schlaefe.append({
                "tag": tag, "zeile": zeilen[0] if zeilen else r.stderr.strip(), "sekunden": round(dauer, 1),
                "modellaufrufe": int(aufrufe.group(1)) if aufrufe else None, "rueckgabe": r.returncode,
            })
            print(f"  Tag {tag}: {schlaefe[-1]['zeile']} ({dauer:.0f} s, {schlaefe[-1]['modellaufrufe']} Aufrufe)", flush=True)
        for p in sz["gedaechtnis_pruefungen"]:
            if p["nach_tag"] == tag:
                ok, gesehen = pruefen(p, mit, umgebung, material)
                pruefungen.append({"name": p["name"], "tag": tag, "bestanden": ok, "gesehen": gesehen})
                print(f"  {'ok  ' if ok else 'FEHL'} Gedaechtnis: {p['name']}" + ("" if ok else f": {gesehen}"), flush=True)

    g = gedaechtnis(mit)["eintraege"]
    bestand = {
        "aktiv": sum(1 for e in g if aktiv(e)),
        "archiviert": sum(1 for e in g if e["zustand"].get("stand") == "archiviert"),
        "ersetzt": sum(1 for e in g if e["zustand"].get("stand") == "ersetzt"),
        "vertraut_aktiv": sum(1 for e in g if aktiv(e) and e["vertraut"]),
        "eintraege": [
            {"nr": e["nr"], "art": e["art"], "vertraut": e["vertraut"], "zustand": e["zustand"], "staerke": e["staerke"],
             "text": e["text"], "stichwoerter": e.get("stichwoerter", [])}
            for e in g
        ],
    }
    ergebnis = {"szenario": sz["name"], "artefakt": artefakt, "saat": saat, "heute": heute,
                "schlaefe": schlaefe, "gedaechtnis_pruefungen": pruefungen, "bestand": bestand}
    if not nur_schlaf:
        for name, ordner in (("mit", mit), ("ohne", ohne)):
            print(f"  Fragen an den Ordner {name} Schlaf:", flush=True)
            ergebnis[f"fragen_{name}"] = fragen(ordner, artefakt, saat, umgebung, os.path.join(lauf, "antworten", name))
    with open(os.path.join(lauf, "ergebnis.json"), "w", encoding="utf-8") as f:
        json.dump(ergebnis, f, ensure_ascii=False, indent=1)

    gp = sum(p["bestanden"] for p in pruefungen)
    teile = [f"Gedaechtnis {gp}/{len(pruefungen)}"]
    if not nur_schlaf:
        for name in ("mit", "ohne"):
            fr = ergebnis[f"fragen_{name}"]
            teile.append(f"Fragen {name} Schlaf {sum(q['bestanden'] for q in fr)}/{len(fr)}")
    teile.append(f"Schlaf gesamt {sum(s['sekunden'] for s in schlaefe):.0f} s")
    print("Fertig: " + ", ".join(teile), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
