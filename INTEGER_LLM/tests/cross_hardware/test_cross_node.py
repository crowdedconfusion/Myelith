#!/usr/bin/env python3
"""
Cross-Hardware und Cross-Node Tests.

Prueft Bit-Exaktheit zwischen:
- Referenz-Backend (CPU, Pure Rust)
- SIMD-Backend (CPU, AVX2/AVX-512/Neon)
- CUDA-Backend (NVIDIA GPU)
- ROCm-Backend (AMD GPU)

Jeder Backend muss gegen die Golden Vectors der Referenz validiert werden.
"""

import subprocess
import sys
from pathlib import Path
from typing import List


HARDWARE_TARGETS = [
    ("reference", "cpu-generic", []),
    ("simd-avx2", "cpu-x86_64", ["cpu-simd"]),
    ("simd-avx512", "cpu-x86_64", ["cpu-simd"]),
    ("simd-neon", "cpu-arm64", ["cpu-simd"]),
    ("cuda", "nvidia-gpu", ["cuda"]),
    ("rocm", "amd-gpu", ["rocm"]),
]


def build_backend(backend: str, features: List[str]) -> bool:
    """Kompiliert ein Backend."""
    kernels_dir = Path(__file__).parent.parent.parent / "kernels"
    features_str = ",".join(features) if features else "reference"
    
    result = subprocess.run(
        ["cargo", "build", "--release", "--features", features_str],
        cwd=kernels_dir,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"[cross] {backend}: COMPILE FAILED")
        print(result.stderr)
        return False
    return True


def run_golden_tests(backend: str, golden_dir: Path) -> bool:
    """Fuehrt Golden-Vector-Tests fuer ein Backend aus."""
    validate_script = Path(__file__).parent.parent / "golden" / "validate.py"
    result = subprocess.run(
        [sys.executable, str(validate_script), backend, str(golden_dir)],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"[cross] {backend} stdout:\n{result.stdout}")
        print(f"[cross] {backend} stderr:\n{result.stderr}")
    return result.returncode == 0


def test_all_hardware():
    """Testet alle Hardware-Backends."""
    golden_dir = Path(__file__).parent.parent / "golden"
    
    results = {}
    for backend, hw_family, features in HARDWARE_TARGETS:
        print(f"\n[cross] Testing {backend} ({hw_family})...")
        
        if not build_backend(backend, features):
            results[backend] = "COMPILE_FAIL"
            continue
        
        if run_golden_tests(backend, golden_dir):
            results[backend] = "PASS"
            print(f"[cross] {backend}: PASS")
        else:
            results[backend] = "GOLDEN_FAIL"
            print(f"[cross] {backend}: GOLDEN_FAIL")
    
    print(f"\n{'='*60}")
    print("Cross-Hardware Test Results:")
    for backend, result in results.items():
        status = "✅" if result == "PASS" else "❌"
        print(f"  {status} {backend:20s} -> {result}")
    print(f"{'='*60}")
    
    return all(r == "PASS" for r in results.values())


if __name__ == "__main__":
    if test_all_hardware():
        print("\n[cross] ALL HARDWARE TESTS PASSED")
        sys.exit(0)
    else:
        print("\n[cross] SOME HARDWARE TESTS FAILED")
        sys.exit(1)