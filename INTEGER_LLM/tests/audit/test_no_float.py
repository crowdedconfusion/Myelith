#!/usr/bin/env python3
"""
Gleitkomma-Audit des Inferenzpfads (Fahrplan-Punkt 12.22).

Eigenständiges Skript nach Projektkonvention (kein pytest). Die
Kerneigenschaft des Projekts — vollständig ganzzahlige Inferenz ohne
Gleitkomma im Rechenpfad — wird hier automatisch geprüft statt nur
implizit angenommen.

Ansatz (statische Quell-Analyse, deterministisch und CI-fähig):
  1. Die Heißpfad-Rust-Quellen werden gescannt (kernels ohne bin/,
     runtime ohne loader).
  2. Kommentare, String-Literale und #[cfg(test)]-Module werden entfernt
     (Test-Fixtures dürfen Gleitkomma verwenden, z. B. um Referenz-LUTs
     zu erzeugen; das ist nicht der Inferenzpfad).
  3. Verbleibende Gleitkomma-Nutzung (f32/f64-Typen, as f32/as f64-Casts,
     float-Literale, float-Methoden wie .exp()/.sqrt()) wird gemeldet.
  4. Fund im Heißpfad => Test failt.

Dokumentiert erlaubte Zonen (kein Heißpfad):
  - #[cfg(test)]-Module (Test-Fixtures, z. B. LUT-Erzeugung in
    kernels/src/{mlp,attention,rmsnorm}.rs)
  - kernels/src/bin/golden_runner.rs (Offline-Referenz-Erzeugung)
  - runtime/src/loader.rs (Kalibrier-Metadaten + Skalen-Validierung,
    Setup statt Inferenzpfad)

Akzeptanzkriterium: null Gleitkomma-Treffer im Inferenzpfad.
"""

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent

# Heißpfad-Dateien (Inferenz-Rechenpfad). runtime/loader.rs ist bewusst
# ausgenommen (Setup/Metadaten), ebenso kernels/src/bin (Offline-Tools).
HOT_PATH = [
    REPO / "kernels" / "src" / "lib.rs",
    REPO / "kernels" / "src" / "fixed_point.rs",
    REPO / "kernels" / "src" / "rmsnorm.rs",
    REPO / "kernels" / "src" / "linear.rs",
    REPO / "kernels" / "src" / "rope.rs",
    REPO / "kernels" / "src" / "attention.rs",
    REPO / "kernels" / "src" / "mlp.rs",
    REPO / "kernels" / "src" / "softmax.rs",
    REPO / "kernels" / "src" / "backend.rs",
    REPO / "kernels" / "src" / "integer_math.rs",
    REPO / "kernels" / "src" / "prng.rs",
    REPO / "kernels" / "src" / "sampling.rs",
    REPO / "kernels" / "src" / "backends" / "mod.rs",
    REPO / "kernels" / "src" / "backends" / "reference.rs",
    REPO / "kernels" / "src" / "backends" / "simd.rs",
    REPO / "kernels" / "src" / "backends" / "cuda.rs",
    REPO / "kernels" / "src" / "backends" / "rocm.rs",
    REPO / "runtime" / "src" / "model.rs",
    REPO / "runtime" / "src" / "kv_cache.rs",
    REPO / "runtime" / "src" / "generate.rs",
]

# Gleitkomma-Indikatoren (angewandt nach Entfernen von Kommentaren,
# Strings und Test-Modulen).
FLOAT_PATTERNS = [
    (re.compile(r"\bf32\b"), "f32-Typ"),
    (re.compile(r"\bf64\b"), "f64-Typ"),
    (re.compile(r"\bas\s+f32\b"), "as-f32-Cast"),
    (re.compile(r"\bas\s+f64\b"), "as-f64-Cast"),
    (re.compile(r"\.exp\(\)"), ".exp()-Methode"),
    (re.compile(r"\.sqrt\(\)"), ".sqrt()-Methode"),
    (re.compile(r"\.ln\(\)"), ".ln()-Methode"),
    (re.compile(r"\.powf\("), ".powf()-Methode"),
    (re.compile(r"\.floor\(\)"), ".floor()-Methode"),
    (re.compile(r"\.ceil\(\)"), ".ceil()-Methode"),
    # float-Literal (z. B. 2.0, 0.5, 256.0) — nicht 0..129 (Range).
    (re.compile(r"\b\d+\.\d+(?:f32|f64)?\b"), "float-Literal"),
]


def strip_comments(src: str) -> str:
    """Entfernt Zeilen- (//) und Block-Kommentare (/* */)."""
    # Block-Kommentare (nicht-gierig, auch mehrzeilig).
    src = re.sub(r"/\*.*?\*/", "", src, flags=re.DOTALL)
    # Zeilen-Kommentare.
    src = re.sub(r"//[^\n]*", "", src)
    return src


def strip_strings(src: str) -> str:
    """Entfernt String-Literale (inkl. Escapes), behält Struktur."""
    # Einfache Handhabung: "..." mit Backslash-Escapes.
    return re.sub(r'"(?:\\.|[^"\\])*"', '""', src)


def strip_test_modules(src: str) -> str:
    """Entfernt #[cfg(test)]-Module per Brace-Matching."""
    out = []
    i = 0
    n = len(src)
    marker = "#[cfg(test)]"
    while i < n:
        pos = src.find(marker, i)
        if pos == -1:
            out.append(src[i:])
            break
        out.append(src[i:pos])
        # Finde das öffnende '{' des Moduls nach dem Attribut.
        j = pos + len(marker)
        # Überspringe optionale weitere Attribute/Whitespace bis 'mod'.
        brace = src.find("{", j)
        if brace == -1:
            out.append(src[pos:])
            break
        # Brace-Matching.
        depth = 0
        k = brace
        while k < n:
            if src[k] == "{":
                depth += 1
            elif src[k] == "}":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        i = k + 1  # hinter das schließende '}' springen
    return "".join(out)


def audit_file(path: Path):
    """Liefert eine Liste von (Zeilennummer, Muster-Label, Zeile)-Treffern."""
    src = path.read_text(encoding="utf-8")
    src = strip_comments(src)
    src = strip_strings(src)
    src = strip_test_modules(src)
    findings = []
    for line_no, line in enumerate(src.splitlines(), start=1):
        for pat, label in FLOAT_PATTERNS:
            for _ in pat.finditer(line):
                findings.append((line_no, label, line.strip()))
    return findings


def main():
    print("[no-float] Gleitkomma-Audit des Inferenzpfads")
    print(f"[no-float] Heißpfad-Dateien: {len(HOT_PATH)}")

    # Sanity: alle Heißpfad-Dateien müssen existieren.
    missing = [p for p in HOT_PATH if not p.exists()]
    if missing:
        for p in missing:
            print(f"[no-float] FEHLT: {p}")
        print("[no-float] FEHLGESCHLAGEN (Heißpfad-Dateien fehlen)")
        sys.exit(1)

    total = 0
    for path in HOT_PATH:
        findings = audit_file(path)
        if findings:
            for line_no, label, line in findings:
                print(f"[no-float] TREFFER {path.name}:{line_no} ({label}): {line}")
            total += len(findings)

    if total == 0:
        print("[no-float] PASSED: null Gleitkomma-Treffer im Inferenzpfad")
        sys.exit(0)
    else:
        print(f"[no-float] FEHLGESCHLAGEN: {total} Gleitkomma-Treffer im Inferenzpfad")
        sys.exit(1)


if __name__ == "__main__":
    main()
