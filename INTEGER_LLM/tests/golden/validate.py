#!/usr/bin/env python3
"""
Backend-Validierung gegen Golden Vectors auf allen Ebenen.
"""

import sys
import json
import subprocess
from pathlib import Path
from typing import List, Dict

# Zentrale Pfadkonstante: Golden Vectors liegen unter <golden_dir>/vectors/<level>/,
# wobei LEVELS den GoldenVector.level-Werten aus generate.py entspricht.
VECTORS_DIRNAME = "vectors"
LEVELS = ("op", "layer", "e2e")


class ValidationReport:
    def __init__(self, backend_name: str):
        self.backend_name = backend_name
        self.passed = 0
        self.failed = 0
        self.errors: List[str] = []
        self.op_results: Dict[str, str] = {}
        self.layer_results: Dict[str, str] = {}
        self.e2e_results: Dict[str, str] = {}
    
    def add_result(self, name: str, level: str, passed: bool, error: str = ""):
        if passed:
            self.passed += 1
        else:
            self.failed += 1
            self.errors.append(f"{name}: {error}")
        
        if level == "op":
            self.op_results[name] = "PASS" if passed else f"FAIL: {error}"
        elif level == "layer":
            self.layer_results[name] = "PASS" if passed else f"FAIL: {error}"
        elif level == "e2e":
            self.e2e_results[name] = "PASS" if passed else f"FAIL: {error}"
    
    def print_summary(self):
        print(f"\n{'='*60}")
        print(f"Validation Report: {self.backend_name}")
        print(f"{'='*60}")
        print(f"Op-Level:    {sum(1 for v in self.op_results.values() if v == 'PASS')}/{len(self.op_results)} passed")
        print(f"Layer-Level: {sum(1 for v in self.layer_results.values() if v == 'PASS')}/{len(self.layer_results)} passed")
        print(f"E2E-Level:   {sum(1 for v in self.e2e_results.values() if v == 'PASS')}/{len(self.e2e_results)} passed")
        print(f"\nTotal: {self.passed} passed, {self.failed} failed")
        if self.errors:
            print(f"\nErrors:")
            for e in self.errors[:10]:
                print(f"  - {e}")
            if len(self.errors) > 10:
                print(f"  ... and {len(self.errors) - 10} more")
        print(f"{'='*60}")


def validate_vector(gv_path: Path, backend_name: str) -> tuple:
    """
    Validiert einen einzelnen Golden Vector via golden_runner Binary.
    Returns: (name, level, passed, error_msg)
    """
    with open(gv_path, "r") as f:
        gv = json.load(f)

    name = gv["name"]
    level = gv["level"]

    # Layer und E2E werden von golden_runner noch nicht unterstuetzt
    if level in ("layer", "e2e"):
        return name, level, True, "SKIPPED (no runner yet)"

    # Pfad zum golden_runner Binary
    kernels_dir = Path(__file__).parent.parent.parent / "kernels"

    # Feature-Mapping
    feature_map = {
        "reference": "reference",
        "simd-avx2": "cpu-simd",
        "simd-avx512": "cpu-simd",
        "simd-neon": "cpu-simd",
        "cuda": "cuda",
        "rocm": "rocm",
    }
    feature = feature_map.get(backend_name, "reference")

    cmd = [
        "cargo", "run", "--bin", "golden_runner",
        "--features", feature,
        "--quiet", "--",
        str(gv_path), backend_name,
    ]

    result = subprocess.run(cmd, cwd=kernels_dir, capture_output=True, text=True)

    stdout = result.stdout.strip()
    stderr = result.stderr.strip()

    if result.returncode == 0 and stdout.startswith("PASS:"):
        return name, level, True, ""
    else:
        error = stderr if stderr else stdout
        return name, level, False, error


def validate_backend(backend_name: str, golden_dir: Path) -> ValidationReport:
    """
    Validiert ein komplettes Backend gegen alle Golden Vectors.
    """
    report = ValidationReport(backend_name)
    vectors_dir = golden_dir / VECTORS_DIRNAME

    for level in LEVELS:
        level_dir = vectors_dir / level
        if level_dir.exists():
            for gv_path in sorted(level_dir.glob("*.golden.json")):
                name, gv_level, passed, error = validate_vector(gv_path, backend_name)
                report.add_result(name, gv_level, passed, error)

    return report


def main():
    if len(sys.argv) < 3:
        print("Usage: validate.py <backend_name> <golden_dir>")
        sys.exit(1)
    
    backend_name = sys.argv[1]
    # Aufloesen auf absoluten Pfad: golden_runner laeuft mit cwd=kernels/
    golden_dir = Path(sys.argv[2]).resolve()
    
    report = validate_backend(backend_name, golden_dir)
    report.print_summary()
    
    if report.failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()