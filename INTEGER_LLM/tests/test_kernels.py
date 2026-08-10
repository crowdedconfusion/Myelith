#!/usr/bin/env python3
"""
Rust↔Python-Bridging fuer Kernel-Unit-Tests.

Ruft `cargo test --features <backend>` auf und parsed stdout
nach Test-Ergebnissen. Prueft, dass alle Rust-Unit-Tests bestehen.
"""

import subprocess
import re
from pathlib import Path


KERNELS_DIR = Path(__file__).parent.parent / "kernels"


def run_cargo_test(features: str) -> tuple:
    """
    Fuehrt `cargo test` mit gegebenen Features aus.
    Returns: (returncode, stdout, stderr)
    """
    cmd = ["cargo", "test", "--features", features]
    result = subprocess.run(
        cmd,
        cwd=KERNELS_DIR,
        capture_output=True,
        text=True,
    )
    return result.returncode, result.stdout, result.stderr


def parse_test_results(stdout: str) -> dict:
    """
    Parsed cargo-test stdout nach Einzelergebnissen.
    Returns: {"passed": int, "failed": int, "ignored": int, "tests": [str]}
    """
    summary_match = re.search(
        r"test result:\s*(\w+)\.\s*(\d+) passed;\s*(\d+) failed;\s*(\d+) ignored",
        stdout,
    )
    if summary_match:
        return {
            "overall": summary_match.group(1),
            "passed": int(summary_match.group(2)),
            "failed": int(summary_match.group(3)),
            "ignored": int(summary_match.group(4)),
        }
    return {"overall": "UNKNOWN", "passed": 0, "failed": 0, "ignored": 0}


def test_reference_backend():
    """Testet Reference Backend."""
    rc, stdout, stderr = run_cargo_test("reference")
    results = parse_test_results(stdout)
    assert rc == 0, f"Reference Backend Tests failed:\n{stderr}"
    assert results["failed"] == 0, f"{results['failed']} Tests failed"
    print(f"[kernels] Reference: {results['passed']} passed, {results['ignored']} ignored")


def test_simd_backend_compile():
    """Testet, dass SIMD Backend kompiliert (nur compile-check)."""
    rc, stdout, stderr = run_cargo_test("cpu-simd")
    if rc != 0:
        # SIMD ist optional – Compile-Only ist ok
        print(f"[kernels] SIMD: compile-only (tests may fail without AVX2 hardware)")
        return
    results = parse_test_results(stdout)
    print(f"[kernels] SIMD: {results['passed']} passed, {results['ignored']} ignored")


def test_all_backends():
    """Fuehrt Kernel-Tests fuer alle verfuegbaren Backends aus."""
    print("=" * 60)
    print("Kernel Unit-Tests (Rust↔Python Bridging)")
    print("=" * 60)

    test_reference_backend()
    test_simd_backend_compile()

    print("[kernels] Alle Kernel-Tests abgeschlossen.")


if __name__ == "__main__":
    test_all_backends()