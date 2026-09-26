#!/usr/bin/env python3
"""Erzeugt `COMPLIANCE/NOTICES.md`: jede Fremdkomponente mit ihrer Lizenz.

Aufruf: `python3 COMPLIANCE/werkzeuge/notices.py [--pruefe]`

## ⚑ Drei Quellen, jede fuer genau eine Sorte

- **Rust-Kisten** aus `cargo metadata`, in jeder Kiste mit eigener
  Sperrdatei, offline aus `SYSTEM/crates-vorrat`. Was eine Kiste zieht,
  weiss Cargo besser als jede Liste.
- **Sprachmodelle** aus `MODELS/llm/KATALOG.json`. Dort stehen die Lizenz
  der Gewichte und die des Artefakts schon; eine zweite Angabe hier liefe
  auseinander.
- **Alles andere** (Modelle fuer Hoeren, Sehen und Sprechen, Programme,
  Python-Pakete, Medien) aus `COMPLIANCE/fremdkomponenten.json`, von
  Hand gepflegt, mit Pruefdatum.

## ⚑ Warum erzeugt und nicht geschrieben

Eine von Hand gefuehrte Liste von ueber hundert Kisten ist nach der
naechsten Versionsanhebung falsch, und niemand merkt es. `--pruefe`
erzeugt neu und vergleicht; die CI faehrt genau das.

## ⚠️ Was sie nicht leistet

Sie liest Lizenzangaben, nicht Recht, und keine Lizenztexte. Eine Kiste,
die sich falsch auszeichnet, steht hier falsch. Pakete ohne Lizenzfeld
werden als solche genannt und nicht verschwiegen.
"""

import json
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
ZIEL = REPO / "COMPLIANCE" / "NOTICES.md"
LISTE = REPO / "COMPLIANCE" / "fremdkomponenten.json"
KATALOG = REPO / "MODELS" / "llm" / "KATALOG.json"

# Was kein Kistenverzeichnis ist, obwohl dort ein Cargo.toml liegt.
#
# 📌 **Fund 478: Hier stand `SYSTEM/`**, gemeint war das Kistenlager.
# Seit der Einrichtungsassistent von GolemOS unter
# `SYSTEM/golemos/einrichten` liegt, haette das eine ausgelieferte Kiste
# still aus den Lizenzhinweisen genommen. Heute hat sie keine
# Abhaengigkeit, und genau deshalb waere es erst mit der ersten
# aufgefallen, also zu spaet.
AUSGENOMMEN = ("SYSTEM/crates-lager/", "/fuzz", "/target", "full-build")

WEGE = {
    "mitgeliefert": "mitgeliefert / shipped",
    "nachgeladen": "vom Einrichtungsskript geholt / fetched by the setup script",
    "vorausgesetzt": "vom Nutzer installiert / installed by the user",
}


def kistenverzeichnisse() -> list:
    """Jede versionierte Kiste mit eigener Sperrdatei, relativ zur Wurzel.

    ⚑ **Nur, was git verfolgt oder verfolgen wird.** Ein Durchsuchen des
    Baums fand auch fremde Projekte in nicht versionierten Ordnern, und
    deren Lizenzen haben hier nichts zu suchen. Neue Dateien, die kein
    Ausschluss trifft, zaehlen dagegen mit: Sonst nennte die erzeugte
    Datei vor dem Commit eine Kiste weniger als die Pruefung im CI
    danach, und die fiele rot aus, ohne dass sich etwas geaendert hat.
    """
    verfolgt = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "*Cargo.lock"],
        cwd=REPO, capture_output=True, text=True,
    ).stdout.split("\n")
    aus = []
    for zeile in sorted(verfolgt):
        if not zeile:
            continue
        rel = str(Path(zeile).parent.as_posix())
        if any(a in "/" + rel for a in AUSGENOMMEN):
            continue
        if (REPO / rel / "Cargo.toml").is_file():
            aus.append(rel)
    return aus


def rust_pakete() -> tuple:
    """(Pakete, Kisten): jedes fremde Paket einmal, dazu die gelesenen Kisten."""
    pakete: dict = {}
    kisten = kistenverzeichnisse()
    for rel in kisten:
        # ⚑ **Erst ohne Netz, dann mit.** Auf einer Arbeitsmaschine liegt
        #   alles im Vorrat; die CI holt die Kisten dagegen ueber das Netz.
        #   `--locked` haelt die Versionen in beiden Faellen fest, also
        #   kommt dasselbe heraus.
        befehl = ["cargo", "metadata", "--format-version", "1", "--locked"]
        lauf = subprocess.run(befehl + ["--offline"], cwd=REPO / rel, capture_output=True, text=True)
        if lauf.returncode != 0:
            lauf = subprocess.run(befehl, cwd=REPO / rel, capture_output=True, text=True)
        if lauf.returncode != 0:
            raise SystemExit(f"[notices] cargo metadata in {rel} schlug fehl:\n{lauf.stderr[-800:]}")
        for p in json.loads(lauf.stdout)["packages"]:
            if not p.get("source"):
                continue  # eigene Kiste
            schluessel = (p["name"], p["version"])
            pakete.setdefault(schluessel, {
                "lizenz": p.get("license") or ("Datei: " + p["license_file"] if p.get("license_file") else "KEINE ANGABE"),
                "quelle": p.get("repository") or "",
            })
    return pakete, kisten


def zelle(text: str) -> str:
    return str(text).replace("|", "\\|").replace("\n", " ")


def erzeugen() -> str:
    liste = json.loads(LISTE.read_text(encoding="utf-8"))
    katalog = json.loads(KATALOG.read_text(encoding="utf-8"))
    pakete, kisten = rust_pakete()

    z: list = []
    a = z.append
    a("# NOTICES")
    a("")
    a("> **Erzeugt, nicht bearbeiten.** Quelle: `COMPLIANCE/werkzeuge/notices.py`")
    a("> aus `cargo metadata`, `MODELS/llm/KATALOG.json` und")
    a("> `COMPLIANCE/fremdkomponenten.json`.")
    a(">")
    a("> **Generated, do not edit.** Source: `COMPLIANCE/werkzeuge/notices.py`")
    a("> from `cargo metadata`, `MODELS/llm/KATALOG.json` and")
    a("> `COMPLIANCE/fremdkomponenten.json`.")
    a("")
    a("Myelith selbst steht unter der PolyForm Shield License 1.0.0 (siehe")
    a("`LICENSE.md`). Diese Datei nennt alles Fremde, was Myelith benutzt,")
    a("mitliefert oder auf den Rechner des Nutzers holt, mit der Lizenz, die")
    a("sein Anbieter angibt. Sie ist eine Auskunft und keine Rechtsberatung.")
    a("")
    a("Myelith itself is licensed under the PolyForm Shield License 1.0.0")
    a("(see `LICENSE.md`). This file lists every third-party component that")
    a("Myelith uses, ships or fetches onto the user's machine, with the licence")
    a("stated by its provider. It is information, not legal advice.")
    a("")

    a("## 1. Sprachmodelle / Language models")
    a("")
    a("Die Artefakte sind ganzzahlig umgerechnete Bearbeitungen der Grundgewichte.")
    a("The artefacts are integer-converted derivative works of the base weights.")
    a("")
    a("| Artefakt / artefact | Grundgewichte / base weights | Lizenz der Gewichte / weights licence | Lizenz des Artefakts / artefact licence |")
    a("|---|---|---|---|")
    for name in sorted(k for k in katalog if not k.startswith("_")):
        e = katalog[name]
        a(f"| `{name}` | [{e.get('hf_repo','')}](https://huggingface.co/{e.get('hf_repo','')}) | "
          f"{zelle(e.get('lizenz_gewichte',''))} | {zelle(e.get('lizenz_artefakt',''))} |")
    a("")

    a("## 2. Weitere Modelle / Further models")
    a("")
    a("| Modell / model | Zweck / purpose | Lizenz / licence | Weg / how it arrives | geprüft / checked |")
    a("|---|---|---|---|---|")
    for m in liste["modell"]:
        a(f"| [{zelle(m['name'])}]({m['quelle']}) | {zelle(m['zweck'])} | **{zelle(m['lizenz'])}** | "
          f"{WEGE[m['weg']]} | {zelle(m.get('geprueft',''))} |")
    for m in liste["modell"]:
        if m.get("hinweis"):
            a("")
            a(f"- **{m['name']}:** {m['hinweis']}")
    a("")

    a("## 3. Programme und Systembestandteile / Programs and system components")
    a("")
    a("| Programm / program | Zweck / purpose | Lizenz / licence | Weg / how it arrives |")
    a("|---|---|---|---|")
    for p in liste["programm"]:
        a(f"| [{zelle(p['name'])}]({p['quelle']}) | {zelle(p['zweck'])} | {zelle(p['lizenz'])} | {WEGE[p['weg']]} |")
    a("")

    a("## 4. Python-Pakete / Python packages")
    a("")
    a("| Paket / package | Zweck / purpose | Lizenz / licence | Anforderung / requirement file | Weg / how it arrives |")
    a("|---|---|---|---|---|")
    for p in sorted(liste["python"], key=lambda x: x["name"].lower()):
        a(f"| `{p['name']}` | {zelle(p['zweck'])} | {zelle(p['lizenz'])} | `{p['anforderung']}` | {WEGE[p['weg']]} |")
    a("")

    a("## 5. Rust-Kisten / Rust crates")
    a("")
    a(f"{len(pakete)} Pakete aus {len(kisten)} Kisten, offline aus `SYSTEM/crates-vorrat`. In die")
    a("Freigabebündel gehen nur die, die das jeweilige Programm zieht.")
    a("")
    a(f"{len(pakete)} packages from {len(kisten)} crates, offline from `SYSTEM/crates-vorrat`.")
    a("Release bundles contain only those the respective program pulls in.")
    a("")
    lizenzen: dict = {}
    for (_, _), e in pakete.items():
        lizenzen[e["lizenz"]] = lizenzen.get(e["lizenz"], 0) + 1
    a("| Lizenz / licence | Pakete / packages |")
    a("|---|---|")
    for lz, n in sorted(lizenzen.items(), key=lambda x: (-x[1], x[0])):
        a(f"| {zelle(lz)} | {n} |")
    a("")
    a("| Paket / package | Version | Lizenz / licence | Quelle / source |")
    a("|---|---|---|---|")
    for (name, version) in sorted(pakete, key=lambda x: (x[0].lower(), x[1])):
        e = pakete[(name, version)]
        a(f"| `{name}` | {version} | {zelle(e['lizenz'])} | {e['quelle']} |")
    a("")

    a("## 6. Daten / Data")
    a("")
    a("| Datensatz / dataset | Zweck / purpose | Lizenz / licence | Weg / how it arrives | geprüft / checked |")
    a("|---|---|---|---|---|")
    for m in liste["daten"]:
        a(f"| [{zelle(m['name'])}]({m['quelle']}) | {zelle(m['zweck'])} | {zelle(m['lizenz'])} | "
          f"{WEGE[m['weg']]} | {zelle(m.get('geprueft',''))} |")
    a("")

    a("## 7. Medien / Media")
    a("")
    a("| Inhalt / content | Urheber / author | Lizenz / licence | Weg / how it arrives |")
    a("|---|---|---|---|")
    for m in liste["medien"]:
        a(f"| {zelle(m['name'])} | {zelle(m['urheber'])} | {zelle(m['lizenz'])} | {WEGE[m['weg']]} |")
    for m in liste["medien"]:
        if m.get("hinweis"):
            a("")
            a(f"- **{m['name']}:** {m['hinweis']}")
    a("")
    return "\n".join(z)


def main() -> int:
    neu = erzeugen()
    if "--pruefe" in sys.argv:
        alt = ZIEL.read_text(encoding="utf-8") if ZIEL.is_file() else ""
        if alt != neu:
            print("[notices] FEHLGESCHLAGEN: COMPLIANCE/NOTICES.md passt nicht zu ihren Quellen")
            print("[notices] `python3 COMPLIANCE/werkzeuge/notices.py` schreibt sie neu")
            return 1
        print("[notices] PASSED: COMPLIANCE/NOTICES.md entspricht den Quellen")
        return 0
    ZIEL.write_text(neu, encoding="utf-8")
    print(f"[notices] geschrieben: {ZIEL.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
