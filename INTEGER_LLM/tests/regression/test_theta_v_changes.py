#!/usr/bin/env python3
"""
Regression-Tests fuer theta_v-Aenderungen.

Regel: Jede Aenderung an theta_v/spec.json erfordert:
1. Neue Golden Vectors (ihr Feld `theta_v_hash` nennt die Fassung, unter
   der sie erzeugt wurden)
2. Re-Validierung ALLER Backends
3. Aktualisierung der Pipeline-Manifeste

📌 Fund 358 (2026-09-14): Dieser Test hat bis dahin nie etwas abgelehnt,
aus drei Gruenden zugleich. Er suchte die Vektoren unter `tests/golden`
statt unter `conformance/vectors`, er rechnete den Hash ueber
umsortiertes JSON statt ueber die Bytes (die Erzeuger und
`loader::spec_hash` nehmen die Bytes), und er gab eine Abweichung als
WARNUNG aus und endete mit 0. Sechs der 49 Vektoren trugen seit dem
2026-08-20 den Hash einer aelteren Fassung, und die Pipeline-Manifeste
den Hash der falschen Groesse. Eine Pruefung, die nur warnen kann, ist
ein Kommentar mit Laufzeit.

Die Manifeste tragen uebrigens NICHT den Hash von spec.json, sondern die
kanonische theta_v-Kennung des Artefakts: SHA-256 ueber
`version|weights_hash|scales_hash|luts_hash` (pipeline/src/stage.rs).
Die Stufe lehnt beim Start ab, wenn sie nicht passt.
"""

import hashlib
import json
import sys
from pathlib import Path

INTEGER_LLM = Path(__file__).resolve().parent.parent.parent
SPEC = INTEGER_LLM / "theta_v" / "spec.json"
VEKTOREN = INTEGER_LLM / "conformance" / "vectors"
CONFIGS = INTEGER_LLM / "configs"
ARTEFAKTE = INTEGER_LLM / "artifacts"

# Welches Artefakt zu welchem Manifest gehoert. Ein neues Manifest ohne
# Eintrag hier ist ein Befund, kein stiller Sprung.
MANIFEST_ARTEFAKT = {
    "pipeline_4node.json": "myelith-0.6b",
    "pipeline_8node.json": "myelith-0.6b",
    "pipeline_uneven4node.json": "myelith-0.6b",
    "pipeline_4node_myelith-30b-a3b.json": "myelith-30b-a3b",
}


def spec_hash() -> str:
    """Wie loader::spec_hash() und die Erzeuger: SHA-256 ueber die Bytes."""
    return "sha256:" + hashlib.sha256(SPEC.read_bytes()).hexdigest()


def kanonische_kennung(artefakt: Path) -> str:
    """Wie pipeline::stage::canonical_theta_v_id."""
    t = json.loads((artefakt / "theta_v.json").read_text(encoding="utf-8"))
    kanon = f"{t['version']}|{t['weights_hash']}|{t['scales_hash']}|{t['luts_hash']}"
    return "sha256:" + hashlib.sha256(kanon.encode()).hexdigest()


def test_theta_v_hash_stability() -> list[str]:
    h1, h2 = spec_hash(), spec_hash()
    if h1 != h2:
        return ["theta_v-Hash nicht stabil"]
    print(f"[regression] theta_v-Hash stabil: {h1[:23]}...")
    return []


def test_golden_vectors_match_theta_v() -> list[str]:
    aktuell = spec_hash()
    dateien = sorted(VEKTOREN.rglob("*.golden.json")) + [VEKTOREN / "manifest.json"]
    if len(dateien) < 2:
        return [f"keine Golden Vectors unter {VEKTOREN} gefunden"]
    abweichend = []
    for p in dateien:
        if json.loads(p.read_text(encoding="utf-8")).get("theta_v_hash") != aktuell:
            abweichend.append(str(p.relative_to(VEKTOREN)))
    if abweichend:
        return [f"{len(abweichend)} von {len(dateien)} Vektordateien passen nicht zu theta_v: "
                + ", ".join(abweichend[:5])]
    print(f"[regression] alle {len(dateien)} Vektordateien passen zu theta_v")
    return []


def test_manifest_theta_v_consistency() -> list[str]:
    befunde = []
    for manifest in sorted(CONFIGS.glob("pipeline_*.json")):
        modell = MANIFEST_ARTEFAKT.get(manifest.name)
        if modell is None:
            befunde.append(f"{manifest.name}: kein Artefakt zugeordnet")
            continue
        artefakt = ARTEFAKTE / modell
        if not (artefakt / "theta_v.json").is_file():
            print(f"[regression] SKIP {manifest.name}: Artefakt {modell} fehlt")
            continue
        soll = kanonische_kennung(artefakt)
        ist = json.loads(manifest.read_text(encoding="utf-8")).get("theta_v_hash", "")
        if ist != soll:
            befunde.append(f"{manifest.name}: theta_v_hash {ist[:23]} statt {soll[:23]}")
        else:
            print(f"[regression] {manifest.name}: theta_v konsistent")
    return befunde


def main() -> int:
    print("=" * 60)
    print("theta_v Regression Tests")
    print("=" * 60)
    befunde = (test_theta_v_hash_stability() + test_golden_vectors_match_theta_v()
               + test_manifest_theta_v_consistency())
    for b in befunde:
        print(f"[regression] FEHLGESCHLAGEN: {b}")
    if befunde:
        return 1
    print("\n[regression] PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
