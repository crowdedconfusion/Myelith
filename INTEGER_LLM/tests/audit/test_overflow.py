#!/usr/bin/env python3
"""
Überlaufverhalten-Audit (Punkt 12.25).

Eigenständiges Skript nach Projektkonvention (kein pytest). Das
Überlaufverhalten ist in spec.json festgelegt und wird hier geprüft:
Sättigung (explicit_clamp_only), kein Wrap.

Zwei Stufen:
  1. spec.json erklärt das Überlaufverhalten explizit:
     numeric.overflow.behavior == explicit_clamp_only, wrap == false,
     und die Sättigungsgrenzen für i8/i16/i32 sind hinterlegt.
  2. Der maßgebliche Rust-Test (kernels::fixed_point::
     overflow_saturation_vector) wird per cargo ausgeführt und muss
     bestehen — er fixiert die Sättigungsgrenzen und das
     Multiplikationsverhalten für CI.

Akzeptanzkriterium: Überlaufvektor besteht auf dem Reference-Backend.
"""

import json
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
KERNELS = REPO / "kernels"
SPEC = REPO / "theta_v" / "spec.json"

# Erwartete Sättigungsgrenzen (müssen mit spec.json und der
# Implementierung übereinstimmen).
EXPECTED_SATURATION = {
    "i8": [-128, 127],
    "i16": [-32768, 32767],
    "i32": [-2147483648, 2147483647],
}


def check_spec() -> bool:
    spec = json.loads(SPEC.read_text(encoding="utf-8"))
    overflow = spec.get("theta_v", {}).get("numeric", {}).get("overflow")
    if overflow is None:
        print("[overflow] FEHLGESCHLAGEN: spec.json numeric.overflow fehlt")
        return False

    # overflow kann ein String (alt) oder ein Objekt (neu) sein.
    if isinstance(overflow, str):
        behavior = overflow
        wrap = None
        saturation = None
    else:
        behavior = overflow.get("behavior")
        wrap = overflow.get("wrap")
        saturation = overflow.get("saturation")

    if behavior != "explicit_clamp_only":
        print(f"[overflow] FEHLGESCHLAGEN: overflow.behavior = {behavior!r}, erwartet 'explicit_clamp_only'")
        return False
    if isinstance(overflow, dict) and wrap is not False:
        print(f"[overflow] FEHLGESCHLAGEN: overflow.wrap = {wrap!r}, erwartet false (kein Wrap)")
        return False

    # Sättigungsgrenzen prüfen (falls als Objekt hinterlegt).
    if saturation is not None:
        for dtype, expected in EXPECTED_SATURATION.items():
            got = saturation.get(dtype)
            if got != expected:
                print(f"[overflow] FEHLGESCHLAGEN: saturation.{dtype} = {got!r}, erwartet {expected!r}")
                return False
        print("[overflow] spec.json: Sättigungsgrenzen i8/i16/i32 hinterlegt  OK")
    else:
        print("[overflow] Hinweis: keine strukturierten Sättigungsgrenzen in spec.json (nur behavior)")

    print("[overflow] spec.json: overflow.behavior = explicit_clamp_only  OK")
    return True


def run_rust_vector() -> bool:
    result = subprocess.run(
        ["cargo", "test", "-p", "integer-llm-kernels", "--lib", "overflow_saturation_vector"],
        cwd=str(KERNELS),
        capture_output=True,
        text=True,
        timeout=600,
    )
    if result.returncode != 0:
        print("[overflow] Rust-Test overflow_saturation_vector FEHLGESCHLAGEN:")
        print(result.stdout[-2000:])
        print(result.stderr[-2000:])
        return False
    if "test result: ok" not in result.stdout:
        print("[overflow] Rust-Test lieferte kein 'test result: ok'")
        print(result.stdout[-2000:])
        return False
    return True


def main():
    print("[overflow] Überlaufverhalten-Audit")

    if not check_spec():
        print("[overflow] FEHLGESCHLAGEN (spec.json)")
        sys.exit(1)

    if not run_rust_vector():
        print("[overflow] FEHLGESCHLAGEN (Rust-Vektor)")
        sys.exit(1)
    print("[overflow] Rust-Vektor overflow_saturation_vector: bestanden")

    print("[overflow] PASSED: Überlaufverhalten fixiert (Sättigung, kein Wrap)")
    sys.exit(0)


if __name__ == "__main__":
    main()
