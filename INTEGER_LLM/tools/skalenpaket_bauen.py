#!/usr/bin/env python3
"""Erzeugt aus fertigen Artefakten ein versionierbares Skalenpaket.

Ein Paket enthaelt genau die Dateien, die den nichtdeterministischen Teil
des Artefaktbaus ersetzen (Fund 32): die Aktivierungsskalen und die LUTs.
Alles Uebrige — die Gewichtsquantisierung — ist reine
Zweierpotenz-Ganzzahlarithmetik und auf jeder Plattform bitgleich.

Groesse: unter 1,5 MB je Modell. Die Gewichte selbst werden NICHT
mitverteilt; sie kommen von Hugging Face.

Usage:
    python tools/skalenpaket_bauen.py qwen2.5-0.5b [qwen2.5-7b ...]
"""
import hashlib
import json
import shutil
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ARTEFAKTE = REPO / "artifacts"
PAKETE = REPO / "scale_packs"

# scales.json und luts.json ersetzen die Gleitkomma-Schritte; theta_v.json
# bindet das Paket an seine Spec-Version.
PFLICHT = ("scales.json", "luts.json", "theta_v.json")


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def bauen(modell: str) -> dict:
    quelle = ARTEFAKTE / modell
    ziel = PAKETE / modell
    if not (quelle / "scales.json").is_file():
        sys.exit(f"FEHLER: keine Artefakte unter {quelle} — erst kalibrieren.")
    ziel.mkdir(parents=True, exist_ok=True)

    dateien = list(PFLICHT) + sorted(p.name for p in quelle.glob("*.lut.bin"))
    for name in dateien:
        shutil.copy2(quelle / name, ziel / name)

    version = json.loads((ziel / "theta_v.json").read_text())["version"]
    groesse = sum((ziel / n).stat().st_size for n in dateien)
    # **Der Pruefanker.** Digest ueber die drei Dateien, an denen die
    # Identitaet des Modells haengt — nicht ueber den Verzeichnisinhalt.
    #
    # `theta_v.json` enthaelt die Hashes von `weights_manifest.json`,
    # `scales.json` und `luts.json`; jene wiederum enthalten den Hash jeder
    # einzelnen Gewichts- und LUT-Datei, und `loader.rs` prueft diese Kette
    # beim Laden. Wer also `theta_v.json`, `model_config.json` und
    # `tokenizer.json` trifft, hat dasselbe Modell.
    #
    # **Warum nicht ueber alle Dateien:** Diese Fassung gab es, und sie hat
    # am 2026-08-20 sofort falschen Alarm ausgeloest. Auf der Entwickler-
    # maschine hatte ein Synchronisationswerkzeug 432 inhaltsgleiche
    # Kopien in den Artefaktordner gelegt ("theta_v 2.json" und so fort).
    # Der Lader ignoriert solche Dateien; sie aendern das Modell nicht. Ein
    # Anker, der bei belanglosen Streudateien anschlaegt, macht den echten
    # Befund unglaubwuerdig — und der echte Befund ist der einzige Zweck.
    anker = ("theta_v.json", "model_config.json", "tokenizer.json")
    fehlend = [n for n in anker if not (quelle / n).is_file()]
    if fehlend:
        sys.exit(f"FEHLER: {quelle} unvollstaendig, es fehlt: {', '.join(fehlend)}")
    eintraege = sorted((n, sha256(quelle / n)) for n in anker)
    digest = hashlib.sha256(
        "\n".join(f"{name}  {h}" for name, h in eintraege).encode()
    ).hexdigest()

    eintrag = {
        "theta_v": version,
        "dateien": dateien,
        "bytes": groesse,
        "artefakt_digest_sha256": digest,
        "digest_ueber": list(anker),
        "weights_manifest_sha256": sha256(quelle / "weights_manifest.json"),
    }
    (ziel / "paket.json").write_text(json.dumps(eintrag, indent=2) + "\n")
    print(f"[paket] {modell}: {len(dateien)} Dateien, {groesse / 1024:.0f} KiB, "
          f"theta_v {version}")
    print(f"        Artefakt-Digest (Ankerkette) = {digest}")
    return eintrag


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    PAKETE.mkdir(parents=True, exist_ok=True)
    reg_pfad = PAKETE / "REGISTER.json"
    register = json.loads(reg_pfad.read_text()) if reg_pfad.is_file() else {}
    for modell in sys.argv[1:]:
        register[modell] = bauen(modell)
    reg_pfad.write_text(json.dumps(register, indent=2, sort_keys=True) + "\n")
    print(f"[paket] Register geschrieben: {reg_pfad}")


if __name__ == "__main__":
    main()
