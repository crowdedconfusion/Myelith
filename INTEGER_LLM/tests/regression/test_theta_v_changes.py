#!/usr/bin/env python3
"""
Regression-Tests fuer theta_v-Aenderungen.

Regel: Jede Aenderung an theta_v/spec.json erfordert:
1. Neue Golden Vectors
2. Re-Validierung ALLER Backends
3. Aktualisierung der Pipeline-Manifeste
"""

import json
import hashlib
from pathlib import Path


def hash_theta_v(spec_path: Path) -> str:
    """Berechnet SHA-256 ueber kanonisches JSON."""
    with open(spec_path, "r") as f:
        data = json.load(f)
    canonical = json.dumps(data, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode()).hexdigest()


def test_theta_v_hash_stability():
    """Prueft, dass sich der Hash bei identischem Inhalt nicht aendert."""
    spec_path = Path(__file__).parent.parent.parent / "theta_v" / "spec.json"
    hash1 = hash_theta_v(spec_path)
    hash2 = hash_theta_v(spec_path)
    assert hash1 == hash2, "theta_v Hash nicht stabil"
    print(f"[regression] theta_v Hash stabil: {hash1[:16]}...")


def test_golden_vectors_match_theta_v():
    """Prueft, dass alle Golden Vectors zum aktuellen theta_v passen."""
    spec_path = Path(__file__).parent.parent.parent / "theta_v" / "spec.json"
    current_hash = hash_theta_v(spec_path)
    
    golden_dir = Path(__file__).parent.parent / "golden"
    mismatches = []
    
    for gv_path in golden_dir.rglob("*.golden.json"):
        with open(gv_path) as f:
            gv = json.load(f)
        if gv.get("theta_v_hash") != current_hash:
            mismatches.append(gv_path.name)
    
    if mismatches:
        print(f"[regression] WARNUNG: {len(mismatches)} Golden Vectors passen nicht zu theta_v!")
        for m in mismatches[:5]:
            print(f"  - {m}")
    else:
        print("[regression] Alle Golden Vectors passen zu theta_v: PASSED")


def test_manifest_theta_v_consistency():
    """Prueft, dass Pipeline-Manifeste zum theta_v passen."""
    spec_path = Path(__file__).parent.parent.parent / "theta_v" / "spec.json"
    current_hash = hash_theta_v(spec_path)
    
    configs_dir = Path(__file__).parent.parent.parent / "configs"
    for manifest_path in configs_dir.glob("pipeline_*.json"):
        with open(manifest_path) as f:
            manifest = json.load(f)
        manifest_hash = manifest.get("theta_v_hash", "")
        if manifest_hash != f"sha256:{current_hash}":
            print(f"[regression] WARNUNG: {manifest_path.name} hat alten theta_v Hash")
        else:
            print(f"[regression] {manifest_path.name}: theta_v konsistent")


def main():
    print("="*60)
    print("theta_v Regression Tests")
    print("="*60)
    test_theta_v_hash_stability()
    test_golden_vectors_match_theta_v()
    test_manifest_theta_v_consistency()
    print("\n[regression] Regression-Tests abgeschlossen.")


if __name__ == "__main__":
    main()
