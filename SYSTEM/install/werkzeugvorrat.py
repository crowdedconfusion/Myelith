#!/usr/bin/env python3
"""Der Vorrat fremder Werkzeuge fuer einen Bau ohne Netz: holen, pruefen, zeigen.

Aufruf:

```text
python3 SYSTEM/install/werkzeugvorrat.py holen            # alles aus werkzeugvorrat.json (Netz)
python3 SYSTEM/install/werkzeugvorrat.py holen --nur nix   # nur einen Teil: rust, python, windows, nix
python3 SYSTEM/install/werkzeugvorrat.py pruefen          # jede Datei gegen ihre Pruefsumme
python3 SYSTEM/install/werkzeugvorrat.py zeigen           # was da ist, je Teil und Groesse
```

# ⚑ Wozu, und warum nicht im Repositorium

Der Paketvorrat (`SYSTEM/crates-vorrat/`) traegt die Rust-Pakete; bauen
braucht daneben aber eine Werkzeugkette, einen C-Compiler und unter Linux die
Bibliotheken des Fensters. Auf einem Rechner ohne Netz fehlt genau das. Dieser
Vorrat haelt es bereit: fuer NixOS die ganze Bauumgebung aus `flake.nix` als
Nix-Binaercache, fuer macOS und Windows die eigenstaendigen Installer von Rust
und Python, dazu Laufzeit und Buildwerkzeuge fuer Windows.

⛔️ **Er gehoert nie ins Repositorium.** Fremde Laufzeiten sind je System
verschieden, wiegen Gigabyte und tragen ihre eigenen Lizenzen; die
Buildwerkzeuge von Microsoft duerfen ohnehin nur auf eigenen Rechnern
liegen. Der Ordner `SYSTEM/werkzeug-vorrat/` ist von git ausgenommen, und die
Sicherheitskopie (`sicherung.py`) nimmt ihn mit.

# ⚑ Was festgelegt ist und was gemessen

Fassungen und Quellen stehen in `werkzeugvorrat.json`, versioniert; der
Stand von nixpkgs steht in `flake.lock`. Was geholt wurde, steht mit SHA-256 in
`SYSTEM/werkzeug-vorrat/LISTE.json`, erzeugt; `pruefen` haelt jede Datei
dagegen. Rust wird zusaetzlich gegen die Pruefsumme geprueft, die das
Rust-Projekt neben jedes Archiv legt.

⚑ **Der Nix-Teil** entsteht je System in einem Behaelter `nixos/nix`
(Docker): `nix flake archive` legt die Quellen aller Eingaben aus
`flake.lock` ab, und die Closure von
`devShells.<system>.default.inputDerivation` traegt jedes Paket, das die
Bauumgebung braucht, zur Bauzeit wie zur Laufzeit. Dazu `bashInteractive`,
mit dem `nix develop` die Umgebung startet. Eingespielt wird mit
`SYSTEM/install/nix-vorrat-einspielen.sh` (als root, denn die selbst gebauten
Pfade tragen keine Unterschrift); die Pruefsummen der Archive prueft Nix
dabei selbst. ⚠️ Die Buildwerkzeuge
fuer Windows lassen sich nur auf Windows zu einem Offline-Bestand ausbauen;
hier liegt der Starter, und `SYSTEM/install/README.md` nennt den Befehl.
"""

from __future__ import annotations

import datetime
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import urllib.request

# ⚠️ `parents[2]`: Diese Datei liegt unter `SYSTEM/install/`.
WURZEL = pathlib.Path(__file__).resolve().parents[2]
KONFIG = WURZEL / "SYSTEM" / "install" / "werkzeugvorrat.json"
VORRAT = WURZEL / "SYSTEM" / "werkzeug-vorrat"
LISTE = VORRAT / "LISTE.json"
TEILE = ("rust", "python", "windows", "nix")


def sha256(p: pathlib.Path) -> str:
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for stueck in iter(lambda: f.read(1 << 20), b""):
            h.update(stueck)
    return h.hexdigest()


def liste_lesen() -> dict:
    if LISTE.is_file():
        return json.loads(LISTE.read_text(encoding="utf-8"))
    return {"dateien": {}}


def liste_schreiben(liste: dict) -> None:
    VORRAT.mkdir(parents=True, exist_ok=True)
    LISTE.write_text(json.dumps(liste, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def laden(url: str, ziel: pathlib.Path) -> None:
    """Laedt in eine Zwischendatei und benennt erst danach um."""
    ziel.parent.mkdir(parents=True, exist_ok=True)
    halb = ziel.with_name(ziel.name + ".halb")
    anfrage = urllib.request.Request(url, headers={"User-Agent": "myelith-werkzeugvorrat"})
    with urllib.request.urlopen(anfrage, timeout=120) as a, open(halb, "wb") as f:
        gesamt = int(a.headers.get("Content-Length") or 0)
        fertig = 0
        for stueck in iter(lambda: a.read(1 << 20), b""):
            f.write(stueck)
            fertig += len(stueck)
            if gesamt:
                print(f"\r[werkzeug]   {ziel.name}: {fertig / gesamt * 100:5.1f} %", end="", flush=True)
    print()
    halb.replace(ziel)


def eintragen(liste: dict, p: pathlib.Path, quelle: str) -> None:
    liste["dateien"][p.relative_to(VORRAT).as_posix()] = {
        "sha256": sha256(p),
        "groesse": p.stat().st_size,
        "quelle": quelle,
        "geholt": datetime.date.today().isoformat(),
    }


def schon_da(liste: dict, p: pathlib.Path) -> bool:
    e = liste["dateien"].get(p.relative_to(VORRAT).as_posix())
    return bool(e) and p.is_file() and p.stat().st_size == e["groesse"]


# ── Die Teile ───────────────────────────────────────────────────────


def holen_rust(k: dict, liste: dict) -> int:
    r = k["rust"]
    for ziel, endung in r["ziele"].items():
        url = r["quelle"].format(fassung=r["fassung"], ziel=ziel, endung=endung)
        p = VORRAT / "rust" / url.rsplit("/", 1)[1]
        if schon_da(liste, p):
            print(f"[werkzeug] rust {ziel}: schon da")
            continue
        print(f"[werkzeug] rust {ziel}")
        laden(url, p)
        # ⛔️ Gegen die Pruefsumme des Rust-Projekts, sofort.
        with urllib.request.urlopen(url + ".sha256", timeout=60) as a:
            erwartet = a.read().decode().split()[0]
        if sha256(p) != erwartet:
            p.unlink()
            print(f"[werkzeug] FEHLER: {p.name} passt nicht zur Pruefsumme des Rust-Projekts")
            return 1
        eintragen(liste, p, url)
    # 📌 **Die vorige Fassung geht** (2026-10-22). Nach dem Wechsel von 1.99.1
    #    auf 1.99.0 lagen beide da; `pruefen` meldete nichts, die Kopie haette
    #    beide mitgenommen, und welches Archiv die Einrichtung nimmt, hing an
    #    der Sortierung.
    behalten = {r["quelle"].format(fassung=r["fassung"], ziel=z, endung=e).rsplit("/", 1)[1] for z, e in r["ziele"].items()}
    for rel in [d for d in liste["dateien"] if d.startswith("rust/") and d.split("/", 1)[1] not in behalten]:
        (VORRAT / rel).unlink(missing_ok=True)
        del liste["dateien"][rel]
        print(f"[werkzeug] rust: alte Fassung entfernt: {rel}")
    return 0


def holen_einzeln(liste: dict, teil: str, dateien: dict[str, str]) -> int:
    for name, url in dateien.items():
        datei = url.rsplit("/", 1)[1] if "fwlink" not in url else f"{name}.exe"
        if teil == "windows":
            datei = {"webview2": "MicrosoftEdgeWebView2RuntimeInstallerX64.exe"}.get(name, datei)
        p = VORRAT / teil / datei
        if schon_da(liste, p):
            print(f"[werkzeug] {teil} {name}: schon da")
            continue
        print(f"[werkzeug] {teil} {name}")
        laden(url, p)
        eintragen(liste, p, url)
    return 0


def holen_nix(k: dict) -> int:
    if shutil.which("docker") is None:
        print("[werkzeug] FEHLER: Fuer den Nix-Teil braucht es Docker.")
        return 1
    n = k["nix"]
    for system, plattform in n["systeme"].items():
        ordner = VORRAT / "nix" / system
        # 📌 „Schon da“ heisst: fuer dieses flake.lock (2026-10-22). Hier stand nur,
        #    ob es den Ordner gibt; nach einem neuen flake.lock verlangte
        #    `pruefen` ein Neuholen, und `holen` meldete „schon da“.
        stand = ordner / "FLAKE_LOCK_SHA256"
        if (ordner / "UMGEBUNG").is_file() and (ordner / "nix-cache-info").is_file() \
                and stand.is_file() and stand.read_text().strip() == sha256(WURZEL / "flake.lock"):
            print(f"[werkzeug] nix {system}: schon da")
            continue
        print(f"[werkzeug] nix {system}: Eingaben und Bauumgebung im Behaelter {n['abbild']} ({plattform})")
        shutil.rmtree(ordner, ignore_errors=True)
        ordner.mkdir(parents=True)
        # ⚑ Nur `flake.nix` und `flake.lock`: Mehr braucht die Auswertung der
        #   Bauumgebung nicht, und der Baum bliebe sonst im Behaelter.
        #   ⚠️ Die Sandbox ist aus, weil Docker sie nicht zulaesst; gebaut wird
        #   hier ohnehin nur die Huelle der Umgebung, alles andere kommt aus
        #   dem Cache von NixOS.
        skript = (
            "set -e; M='--extra-experimental-features nix-command --extra-experimental-features flakes "
            "--option sandbox false --option filter-syscalls false'; "
            "mkdir /f; cp /quelle/flake.nix /quelle/flake.lock /f/; "
            "Z='file:///aus?compression=zstd'; "
            "nix $M flake archive --to \"$Z\" /f; "
            f"P=$(nix $M build --no-link --print-out-paths '/f#devShells.{system}.default.inputDerivation'); "
            "B=$(nix $M build --no-link --print-out-paths --inputs-from /f nixpkgs#bashInteractive); "
            "nix $M copy --to \"$Z\" $P $B; "
            "echo $P > /aus/UMGEBUNG; echo $B > /aus/BASH; nix --version > /aus/NIX"
        )
        lauf = subprocess.run([
            "docker", "run", "--rm", "--platform", plattform,
            "-v", f"{WURZEL}/flake.nix:/quelle/flake.nix:ro",
            "-v", f"{WURZEL}/flake.lock:/quelle/flake.lock:ro",
            "-v", f"{ordner}:/aus", n["abbild"], "sh", "-c", skript,
        ])
        if lauf.returncode != 0:
            print(f"[werkzeug] FEHLER: Behaelter fuer {system} endete mit {lauf.returncode}")
            return 1
        (ordner / "FLAKE_LOCK_SHA256").write_text(sha256(WURZEL / "flake.lock") + "\n", encoding="utf-8")
        z = list(ordner.rglob("*"))
        g = sum(p.stat().st_size for p in z if p.is_file())
        print(f"[werkzeug] nix {system}: {sum(1 for p in z if p.suffix == '.narinfo')} Pfade, {g / 1e6:.0f} MB")
    return 0


def pruefen_nix() -> list[str]:
    """Jede `.narinfo` gegen ihr Archiv, und der Stand gegen `flake.lock`."""
    fehler = []
    for ordner in sorted((VORRAT / "nix").glob("*")) if (VORRAT / "nix").is_dir() else []:
        n = 0
        for info in ordner.glob("*.narinfo"):
            felder = dict(z.split(": ", 1) for z in info.read_text().splitlines() if ": " in z)
            nar = ordner / felder.get("URL", "")
            if not nar.is_file():
                fehler.append(f"{ordner.name}: Archiv fehlt fuer {info.name}")
            elif "FileSize" in felder and nar.stat().st_size != int(felder["FileSize"]):
                fehler.append(f"{ordner.name}: Archiv unvollstaendig: {nar.name}")
            n += 1
        lock = (ordner / "FLAKE_LOCK_SHA256").read_text().strip() if (ordner / "FLAKE_LOCK_SHA256").is_file() else ""
        if lock != sha256(WURZEL / "flake.lock"):
            fehler.append(f"{ordner.name}: geholt fuer ein anderes flake.lock; neu holen mit `holen --nur nix`")
        print(f"[werkzeug] nix {ordner.name}: {n} Pfade")
    return fehler


def holen(nur: list[str]) -> int:
    k = json.loads(KONFIG.read_text(encoding="utf-8"))
    liste = liste_lesen()
    rc = 0
    try:
        if "rust" in nur:
            rc |= holen_rust(k, liste)
        if "python" in nur:
            rc |= holen_einzeln(liste, "python", k["python"]["dateien"])
        if "windows" in nur:
            rc |= holen_einzeln(liste, "windows", k["windows"])
        if "nix" in nur:
            rc |= holen_nix(k)
    finally:
        liste["rust_fassung"] = k["rust"]["fassung"]
        liste_schreiben(liste)
    if rc == 0:
        print("[werkzeug] Fertig")
    return rc


def festgelegte_quellen(k: dict) -> list[tuple[str, str]]:
    """Jede Datei, die `werkzeugvorrat.json` festlegt, als (Teil, Quelle)."""
    r = k["rust"]
    quellen = [("rust", r["quelle"].format(fassung=r["fassung"], ziel=z, endung=e)) for z, e in r["ziele"].items()]
    quellen += [("python", url) for url in k["python"]["dateien"].values()]
    quellen += [("windows", url) for url in k["windows"].values()]
    return quellen


def nicht_geholt(k: dict, liste: dict) -> list[str]:
    """Was festgelegt ist, aber mit dieser Quelle nicht im Vorrat liegt.

    📌 **Bis zum 2026-10-22 verglich `pruefen` nur die Fassung von Rust.**
    Eine neue Python-Fassung oder ein neues Rust-Ziel in der Festlegung
    blieb unbemerkt: Die alten Dateien passten zu ihren Pruefsummen, also
    hiess es PASSED, und die Sicherheitskopie nahm den alten Stand mit.
    """
    geholt = {e.get("quelle") for e in liste["dateien"].values()}
    return [f"{teil}: festgelegt, aber nicht geholt: {url} (holen --nur {teil})"
            for teil, url in festgelegte_quellen(k) if url not in geholt]


def pruefen() -> int:
    liste = liste_lesen()
    if not liste["dateien"] and not (VORRAT / "nix").is_dir():
        print(f"[werkzeug] kein Vorrat unter {VORRAT}")
        return 1
    fehler = pruefen_nix()
    for rel, e in sorted(liste["dateien"].items()):
        p = VORRAT / rel
        if not p.is_file():
            fehler.append(f"fehlt: {rel}")
        elif sha256(p) != e["sha256"]:
            fehler.append(f"Pruefsumme passt nicht: {rel}")
    bekannt = set(liste["dateien"]) | {"LISTE.json"}
    fremd = [
        p.relative_to(VORRAT).as_posix()
        for p in VORRAT.rglob("*")
        if p.is_file() and p.relative_to(VORRAT).as_posix() not in bekannt and not p.name.endswith(".halb")
        and "/vs_buildtools/" not in "/" + p.relative_to(VORRAT).as_posix()
        and not p.relative_to(VORRAT).as_posix().startswith("nix/")
    ]
    k = json.loads(KONFIG.read_text(encoding="utf-8"))
    if liste.get("rust_fassung") != k["rust"]["fassung"]:
        fehler.append(f"Rust {liste.get('rust_fassung')} im Vorrat, festgelegt ist {k['rust']['fassung']}")
    fehler += [f"Rust einer anderen Fassung im Vorrat: {d} (holen --nur rust raeumt auf)"
               for d in liste["dateien"] if d.startswith("rust/") and f"-{k['rust']['fassung']}-" not in d]
    fehler += nicht_geholt(k, liste)
    for f in fehler[:10]:
        print(f"   {f}")
    if fremd:
        print(f"   {len(fremd)} Dateien ohne Eintrag (nicht geprueft), etwa {fremd[:3]}")
    if fehler:
        print("[werkzeug] FAILED")
        return 1
    nix = "der Nix-Teil ist vollstaendig" if (VORRAT / "nix").is_dir() else "kein Nix-Teil"
    print(f"[werkzeug] PASSED: {len(liste['dateien'])} Dateien passen zu ihren Pruefsummen, {nix}")
    return 0


def zeigen() -> int:
    liste = liste_lesen()
    je: dict[str, list[int]] = {}
    for ordner in sorted((VORRAT / "nix").glob("*")) if (VORRAT / "nix").is_dir() else []:
        dateien = [p for p in ordner.rglob("*") if p.is_file()]
        je[f"nix/{ordner.name}"] = [len(dateien), sum(p.stat().st_size for p in dateien)]
    for rel, e in liste["dateien"].items():
        teil = rel.split("/")[0]
        z = je.setdefault(teil, [0, 0])
        z[0] += 1
        z[1] += e["groesse"]
    for teil, (n, g) in sorted(je.items()):
        print(f"[werkzeug] {teil:28} {n:5} Dateien {g / 1e6:9.1f} MB")
    vs = VORRAT / "windows" / "vs_buildtools"
    print(f"[werkzeug] Offline-Bestand der Buildwerkzeuge fuer Windows: {'da' if vs.is_dir() else 'fehlt (nur unter Windows zu bauen)'}")
    return 0


def main(argv: list[str]) -> int:
    if not argv or argv[0] not in ("holen", "pruefen", "zeigen"):
        print(__doc__)
        return 2
    if argv[0] == "holen":
        nur = list(TEILE)
        if "--nur" in argv:
            nur = argv[argv.index("--nur") + 1].split(",")
            unbekannt = [t for t in nur if t not in TEILE]
            if unbekannt:
                print(f"[werkzeug] unbekannt: {unbekannt}; moeglich: {', '.join(TEILE)}")
                return 2
        return holen(nur)
    return {"pruefen": pruefen, "zeigen": zeigen}[argv[0]]()


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
