#!/usr/bin/env python3
"""
Evidenz-Lauf 1: Bit-Identität (Determinismus) der Integer-Inferenz.

Zeigt plastisch, worauf die gesamte Verifikationsarchitektur aufbaut
(Whitepaper Kap. 6): Dieselbe Eingabe ergibt auf derselben Runtime
laufübergang EXAKT dieselbe Tokenfolge — nicht „ungefähr", sondern
bitidentisch, nachweisbar über Hashes.

Methodik:
  - Feste Prompt-Menge (DE/EN), greedy-Decodierung, feste Max-Token-Zahl.
  - N unabhängige Prozess-Läufe der Runtime-CLI pro Prompt
    (jeder Lauf lädt das Modell neu — gemeinsamer Zustand ist
    ausgeschlossen).
  - Verglichen werden die generierten Tokenfolgen selbst, der
    Runtime-interne Token-Hash und ein zusätzlich pythonseitig
    berechneter SHA-256 über die Tokenfolge.

Ergebnis: eval/results/evidence/determinism.json
"""

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ARTIFACTS = REPO / "artifacts" / "qwen2.5-0.5b"
# Seit alle Crates in ein gemeinsames target-shared/ bauen (.cargo/config.toml)
# liegt das Binary nicht mehr unter runtime/target/. Derselbe Resolver wie in
# eval/perplexity.py: prueft CARGO_TARGET_DIR, target-shared/ und den
# Cargo-Standardort der Reihe nach.
sys.path.insert(0, str(REPO / "tests"))
from cargo_paths import binary, fehlt_hinweis  # noqa: E402

CLI = binary("runtime", "integer-llm-runtime")
RESULTS_DIR = REPO / "eval" / "results" / "evidence"

PROMPTS = [
    "The capital of France is",
    "In quantum mechanics, the wave function describes",
    "Die Hauptstadt von Frankreich ist",
    "In der Quantenmechanik beschreibt die Wellenfunktion",
    "The result of 17 times 23 is",
]
RUNS = 5
MAX_TOKENS = 40

TOKEN_RE = re.compile(r"\[runtime\] Generierte Token: \[(.*)\]")
HASH_RE = re.compile(r"\[runtime\] Token-Hash: ([0-9a-f]+)")


def run_once(prompt: str) -> tuple:
    result = subprocess.run(
        [str(CLI), str(ARTIFACTS), prompt, str(MAX_TOKENS)],
        capture_output=True, text=True, timeout=7200,
    )
    if result.returncode != 0:
        print(f"[determinism] FEHLT: Lauf fehlgeschlagen: {result.stderr}",
              file=sys.stderr)
        sys.exit(1)
    tokens = None
    token_hash = None
    for line in result.stdout.splitlines():
        m = TOKEN_RE.search(line)
        if m:
            raw = m.group(1).strip()
            tokens = [int(t) for t in raw.split(",")] if raw else []
        m = HASH_RE.search(line)
        if m:
            token_hash = m.group(1)
    if tokens is None or token_hash is None:
        print(f"[determinism] FEHLT: Ausgabe unvollständig:\n{result.stdout}",
              file=sys.stderr)
        sys.exit(1)
    sha = hashlib.sha256(",".join(str(t) for t in tokens).encode()).hexdigest()
    return tokens, token_hash, sha


def main():
    if not CLI.exists():
        print(f"[determinism] FEHLT: {CLI} — zuerst 'cargo build --release'.",
              file=sys.stderr)
        sys.exit(1)

    records = []
    all_passed = True
    for prompt in PROMPTS:
        runs = [run_once(prompt) for _ in range(RUNS)]
        token_sets = {json.dumps(r[0]) for r in runs}
        hash_sets = {r[1] for r in runs}
        sha_sets = {r[2] for r in runs}
        identical = len(token_sets) == 1 and len(hash_sets) == 1 and len(sha_sets) == 1
        all_passed = all_passed and identical
        records.append({
            "prompt": prompt,
            "runs": RUNS,
            "bit_identical": identical,
            "token_hash": runs[0][1],
            "sha256": runs[0][2],
            "tokens": runs[0][0],
        })
        status = "PASSED" if identical else "FAILED"
        print(f"[determinism] {status} ({RUNS} Läufe identisch): {prompt!r}")
        print(f"              Token-Hash: {runs[0][1]}  SHA-256: {runs[0][2][:16]}…")

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out = RESULTS_DIR / "determinism.json"
    out.write_text(json.dumps({
        "binary": str(CLI.relative_to(REPO)),
        "runs_per_prompt": RUNS,
        "max_tokens": MAX_TOKENS,
        "decoding": "greedy (deterministisch, Seed irrelevant)",
        "all_bit_identical": all_passed,
        "prompts": records,
    }, indent=2, ensure_ascii=False), encoding="utf-8")

    print(f"[determinism] Ergebnis: {'PASSED' if all_passed else 'FAILED'} "
          f"({len(PROMPTS)} Prompts × {RUNS} Läufe)")
    print(f"[determinism] Gesichert: {out}")
    if not all_passed:
        sys.exit(1)


if __name__ == "__main__":
    main()
