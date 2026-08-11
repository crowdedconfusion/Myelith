#!/usr/bin/env python3
"""
Evidenz-Lauf 3: Durchsatz-Benchmark der Integer-Inferenz.

Misst Prefill- und Decode-Durchsatz (Tokens/s) des Referenz-Backends
(reine Skalar-Implementierung, noch ohne SIMD/CUDA/ROCm — die Backends
folgen laut Fahrplan 12.35–12.55 bewusst nachgelagert). Die Zeitmessung
läuft in der Rust-Probe selbst (runtime/src/bin/bench_probe.rs), also
ohne Modellladezeit und ohne Python-Overhead.

Einordnung: Dies ist ein QUALITÄTS-/Evidenz-Benchmark, kein
Leistungsversprechen. Die Zahlen dienen als Referenzpunkt, an dem sich
die späteren Backends und die Cross-Hardware-Messungen messen lassen.

Ergebnis: eval/results/evidence/benchmark.json
"""

import json
import platform
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ARTIFACTS = REPO / "artifacts" / "qwen2.5-0.5b"
BENCH = REPO / "runtime" / "target" / "release" / "bench_probe"
RESULTS_DIR = REPO / "eval" / "results" / "evidence"

# Kurzer Prompt (wenige Prompt-Tokens, Decode-dominiert) und längerer
# Prompt (Prefill-Anteil sichtbar). Repetitions für Stabilität.
SCENARIOS = [
    {"name": "decode_lastig", "prompt": "Once upon a time",
     "decode_tokens": 128, "repeats": 3},
    {"name": "prefill_anteil",
     "prompt": ("The history of decentralized networks begins with early "
                "peer-to-peer systems for file sharing, which demonstrated "
                "that coordination without a central authority is possible."),
     "decode_tokens": 64, "repeats": 3},
]


def run_once(prompt: str, decode_tokens: int) -> dict:
    result = subprocess.run(
        [str(BENCH), str(ARTIFACTS), prompt, str(decode_tokens)],
        capture_output=True, text=True, timeout=7200,
    )
    if result.returncode != 0:
        print(f"[bench] FEHLT: Lauf fehlgeschlagen: {result.stderr}",
              file=sys.stderr)
        sys.exit(1)
    vals = {}
    for line in result.stdout.strip().splitlines():
        key, _, value = line.partition(" ")
        vals[key] = value
    return {
        "prompt_tokens": int(vals["prompt_tokens"]),
        "prefill_ms": float(vals["prefill_ms"]),
        "prefill_tokens_per_s": float(vals["prefill_tokens_per_s"]),
        "decode_tokens": int(vals["decode_tokens"]),
        "decode_ms": float(vals["decode_ms"]),
        "decode_tokens_per_s": float(vals["decode_tokens_per_s"]),
        "decode_hash": vals["decode_hash"],
    }


def main():
    if not BENCH.exists():
        print(f"[bench] FEHLT: {BENCH} — zuerst 'cargo build --release --bins'.",
              file=sys.stderr)
        sys.exit(1)

    records = []
    for sc in SCENARIOS:
        runs = [run_once(sc["prompt"], sc["decode_tokens"])
                for _ in range(sc["repeats"])]
        # Determinismus-Querverweis: alle Wiederholungen müssen dieselben
        # Tokens (also denselben Hash) erzeugen.
        hashes = {r["decode_hash"] for r in runs}
        assert len(hashes) == 1, f"Determinismus-Verletzung in {sc['name']}: {hashes}"
        decode_tps = [r["decode_tokens_per_s"] for r in runs]
        prefill_tps = [r["prefill_tokens_per_s"] for r in runs]
        records.append({
            "name": sc["name"],
            "prompt_tokens": runs[0]["prompt_tokens"],
            "decode_tokens": runs[0]["decode_tokens"],
            "repeats": sc["repeats"],
            "decode_hash": runs[0]["decode_hash"],
            "prefill_tokens_per_s": {
                "min": min(prefill_tps), "max": max(prefill_tps),
                "median": sorted(prefill_tps)[len(prefill_tps) // 2]},
            "decode_tokens_per_s": {
                "min": min(decode_tps), "max": max(decode_tps),
                "median": sorted(decode_tps)[len(decode_tps) // 2]},
        })
        print(f"[bench] {sc['name']}: Prefill "
              f"{records[-1]['prefill_tokens_per_s']['median']:.1f} tok/s, "
              f"Decode {records[-1]['decode_tokens_per_s']['median']:.1f} tok/s "
              f"(Hash {runs[0]['decode_hash'][:12]}… in allen Läufen identisch)")

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out = RESULTS_DIR / "benchmark.json"
    out.write_text(json.dumps({
        "backend": "reference (skalar, ohne SIMD/CUDA/ROCm)",
        "build": "release",
        "platform": {
            "system": platform.system(),
            "machine": platform.machine(),
            "python": platform.python_version(),
        },
        "scenarios": records,
    }, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"[bench] Gesichert: {out}")


if __name__ == "__main__":
    main()
