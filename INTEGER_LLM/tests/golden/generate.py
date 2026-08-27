#!/usr/bin/env python3
"""
Golden-Vector-Generator

Erzeugt deterministische Test-Vektoren auf 3 Ebenen:
1. Operation-Level (rmsnorm, linear, softmax, etc.)
2. Layer-Level (kompletter Transformer-Block)
3. End-to-End (Prompt -> Token-Sequenz)

Nur das Referenz-Backend darf Vektoren erzeugen.
"""

import math
import sys

import json


def clamp_i8(x: int) -> int:
    if x < -128:
        return -128
    elif x > 127:
        return 127
    else:
        return x


def clamp_i16(x: int) -> int:
    if x < -32768:
        return -32768
    elif x > 32767:
        return 32767
    else:
        return x


def rshift_round(value: int, shift: int) -> int:
    if shift == 0:
        return value
    mask = (1 << shift) - 1
    half = 1 << (shift - 1)
    quotient = value >> shift
    remainder = value & mask
    if remainder > half or (remainder == half and (quotient & 1)):
        return quotient + 1
    return quotient


def inv_n_q20(n: int) -> int:
    """Reziproken-Konstante 2^20/n (gerundet), wie kernels::rmsnorm::inv_n_q20."""
    return ((1 << 20) + n // 2) // n


def spec_rsqrt_lut(length: int, input_shift: int = 8, output_frac: int = 8) -> list:
    """rsqrt-LUT im spec-Format: lut[x] = round(rsqrt(x * 2^-input_shift) * 2^output_frac)."""
    lut = []
    for x in range(length):
        if x == 0:
            lut.append(1 << output_frac)
        else:
            real = x / (1 << input_shift)
            lut.append(int(round((1.0 / math.sqrt(real)) * (1 << output_frac))))
    return lut


def rmsnorm_i16_ref(x, gamma, gamma_shift, rsqrt_lut, lut_input_shift,
                    lut_output_frac, inv_n, out_frac):
    """Referenz-Implementierung von kernels::rmsnorm::rmsnorm_i16 (theta_v 0.5.0):
    LUT-gestuetztes rsqrt mit dynamischem geradem Index-Shift, divisionsfrei."""
    n = len(x)
    acc = sum(v * v for v in x)
    if acc == 0:
        return [0] * n
    m = (acc * inv_n) >> 20
    max_idx = len(rsqrt_lut) - 1
    q = 0
    while (m >> q) > max_idx:
        q += 2
    idx = min(rshift_round(m, q), max_idx)
    lut_val = rsqrt_lut[idx]
    norm_frac = lut_output_frac + lut_input_shift // 2 + q // 2
    total_frac = norm_frac + gamma_shift
    out = []
    for v, g in zip(x, gamma):
        prod = v * lut_val * g
        out.append(clamp_i16(rescale(prod, total_frac, out_frac)))
    return out


def rescale(acc: int, in_frac: int, out_frac: int) -> int:
    shift = in_frac - out_frac
    if shift >= 0:
        return rshift_round(acc, shift)
    else:
        return acc << (-shift)


def linear_w8a16_ref(x, W, act_frac, weight_frac, out_frac):
    """Referenz-Implementierung von kernels::linear::linear_w8a16
    (Gewichte int8, Aktivierungen int16, i64-Akkumulator)."""
    in_frac = act_frac + weight_frac
    out = []
    for row in W:
        acc = sum(w * v for w, v in zip(row, x))
        y = rescale(acc, in_frac, out_frac)
        out.append(clamp_i16(y))
    return out


def exp_lut_lookup(x, exp_lut, lut_shift, one):
    if x <= 0:
        return one
    idx = x >> lut_shift
    if idx >= len(exp_lut):
        return 0
    return exp_lut[idx]


def softmax_int_ref(logits, exp_lut, lut_shift, frac_bits):
    one = 1 << frac_bits
    m = max(logits)
    exps = []
    for z in logits:
        diff = m - z
        exps.append(exp_lut_lookup(diff, exp_lut, lut_shift, one))
    s = sum(exps)
    if s == 0:
        base = one // len(exps)
        rem = one - base * len(exps)
        return [base + (1 if i < rem else 0) for i in range(len(exps))]
    probs = []
    for e in exps:
        num = e * one
        q = num // s
        r = num % s
        twice = abs(r) * 2
        den_abs = abs(s)
        if twice > den_abs or (twice == den_abs and (q & 1)):
            if (num > 0) == (s > 0):
                q += 1
            else:
                q -= 1
        probs.append(q)
    return probs


def splitmix64(state: int):
    state = (state + 0x9E3779B97F4A7C15) & 0xFFFFFFFFFFFFFFFF
    z = state
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9 & 0xFFFFFFFFFFFFFFFF
    z = (z ^ (z >> 27)) * 0x94D049BB133111EB & 0xFFFFFFFFFFFFFFFF
    z = z ^ (z >> 31)
    return state, z & 0xFFFFFFFFFFFFFFFF


def deterministic_tensor(rows, cols, seed):
    data = []
    state = seed
    for _ in range(rows * cols):
        state, z = splitmix64(state)
        val = z & 0xFF
        if val >= 128:
            val -= 256
        data.append(val)
    return data

import struct
import hashlib
from pathlib import Path
from typing import Dict, List, Any

# Zentrale Pfadkonstante: alle Golden Vectors liegen unter vectors/<level>/,
# wobei <level> "op", "layer" oder "e2e" entspricht (siehe GoldenVector.level).
VECTORS_DIR = Path(__file__).parent / "vectors"

# Die Layer- und E2E-Vektoren werden mit diesem Artefakt erzeugt
# (generate_layer_vectors). Das Manifest verzeichnet die Bindung, damit
# ein Prüfer vor dem Lauf entscheiden kann, ob sein Artefakt zu den
# Vektoren passt — statt es zu raten oder blind zu laden.
MANIFEST_MODELL = "qwen2.5-0.5b"


class GoldenVector:
    def __init__(self, name: str, level: str, theta_v_hash: str):
        self.name = name
        self.level = level  # "op", "layer", "e2e"
        self.theta_v_hash = theta_v_hash
        self.inputs: Dict[str, Any] = {}
        self.outputs: Dict[str, Any] = {}
        self.metadata: Dict[str, Any] = {}
    
    def add_input(self, key: str, tensor: List[int], dtype: str = "int8"):
        self.inputs[key] = {
            "dtype": dtype,
            "shape": [len(tensor)] if isinstance(tensor, list) else None,
            "hash": self._hash_tensor(tensor, dtype),
            "data": tensor,
        }
    
    def add_output(self, key: str, tensor: List[int], dtype: str = "int8"):
        self.outputs[key] = {
            "dtype": dtype,
            "shape": [len(tensor)] if isinstance(tensor, list) else None,
            "hash": self._hash_tensor(tensor, dtype),
            "data": tensor,
        }
    
    def _hash_tensor(self, tensor, dtype: str) -> str:
        if dtype == "int8":
            payload = struct.pack(f"<{len(tensor)}b", *tensor)
        elif dtype == "int16":
            payload = struct.pack(f"<{len(tensor)}h", *tensor)
        elif dtype == "int32":
            payload = struct.pack(f"<{len(tensor)}i", *tensor)
        else:
            payload = json.dumps(tensor).encode()
        return hashlib.sha256(payload).hexdigest()
    
    def save(self, path: Path):
        obj = {
            "name": self.name,
            "level": self.level,
            "theta_v_hash": self.theta_v_hash,
            "metadata": self.metadata,
            "inputs": self.inputs,
            "outputs": self.outputs,
        }
        with open(path, "w", encoding="utf-8") as f:
            json.dump(obj, f, sort_keys=True, separators=(",", ":"))
    
    @classmethod
    def load(cls, path: Path) -> "GoldenVector":
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
        gv = cls(data["name"], data["level"], data["theta_v_hash"])
        gv.inputs = data["inputs"]
        gv.outputs = data["outputs"]
        gv.metadata = data.get("metadata", {})
        return gv


def generate_op_vectors(theta_v_hash: str, output_dir: Path):
    """Generiert Golden Vectors fuer einzelne Operationen (theta_v 0.5.0)."""
    output_dir.mkdir(parents=True, exist_ok=True)

    # RMSNorm (int16, LUT-gestuetzt, dynamischer gerader Index-Shift).
    # Die LUT hat hier 1024 Eintraege (kompakte Test-Variante; der
    # dynamische Index-Shift passt den Mittelwert an den LUT-Bereich an,
    # derselbe Mechanismus wie mit der vollen 32768er-LUT im Artefakt).
    rsqrt_lut = spec_rsqrt_lut(1024, input_shift=8, output_frac=8)
    x = [512, 512, -512, 0, 256, -256]  # Restgrossen bei frac 3
    gamma = [64, 64, 64, 64, 64, 64]    # gamma 1.0 bei shift 6
    inv_n = inv_n_q20(len(x))
    gv = GoldenVector("rmsnorm_basic", "op", theta_v_hash)
    gv.add_input("x", x, "int16")
    gv.add_input("gamma", gamma, "int8")
    gv.metadata = {
        "gamma_shift": 6,
        "rsqrt_lut": rsqrt_lut,
        "lut_input_shift": 8,
        "lut_output_frac": 8,
        "inv_n_q20": inv_n,
        "out_frac": 6,
    }
    y = rmsnorm_i16_ref(x, gamma, 6, rsqrt_lut, 8, 8, inv_n, 6)
    gv.add_output("y", y, "int16")
    gv.save(output_dir / "rmsnorm_basic.golden.json")

    # Linear W8A16 (Identitaet): 127/128 ~ 1.0 bei weight_frac 7,
    # RNE-Rundung erhaelt die Werte exakt.
    gv = GoldenVector("linear_w8a16_identity", "op", theta_v_hash)
    gv.add_input("x", [64, -64], "int16")
    gv.metadata = {"W": [[127, 0], [0, 127]], "act_frac": 6, "weight_frac": 7, "out_frac": 6}
    y = linear_w8a16_ref([64, -64], [[127, 0], [0, 127]], 6, 7, 6)
    gv.add_output("y", y, "int16")
    gv.save(output_dir / "linear_w8a16_identity.golden.json")

    # Softmax (spec 0.5.2: exp-LUT-Domaene [0, 64), Eingang frac 4,
    # Ausgang frac 8; lut_shift = score_frac(8) - exp_input_frac(4) = 4).
    # Die LUT wird im Vektor mitgefuehrt, damit der golden_runner exakt
    # dieselbe Tabelle verwendet (er wuerde sonst eine eigene bauen).
    gv = GoldenVector("softmax_basic", "op", theta_v_hash)
    gv.add_input("logits", [100, 200, 50, 300], "int32")
    exp_lut = [int(round(math.exp(-i / 16.0) * 256)) for i in range(1025)]
    gv.metadata = {"lut_shift": 4, "frac_bits": 8, "exp_lut": exp_lut}
    y = softmax_int_ref([100, 200, 50, 300], exp_lut, 4, 8)
    gv.add_output("probs", y, "int32")
    gv.save(output_dir / "softmax_basic.golden.json")

    print(f"[golden] {len(list(output_dir.glob('*.golden.json')))} Op-Vektoren erzeugt.")


def dummy_layer_forward(hidden, layer_idx, theta_v_hash, hidden_size=896):
    seed = int(hashlib.sha256(f"{theta_v_hash}:{layer_idx}".encode()).hexdigest(), 16) % (2**64)
    w = deterministic_tensor(hidden_size, hidden_size, seed)
    out = [0] * hidden_size
    for row in range(hidden_size):
        acc = 0
        for col in range(hidden_size):
            acc += w[row * hidden_size + col] * hidden[col]
        out[row] = clamp_i8(rshift_round(acc, 14))
    for i in range(hidden_size):
        out[i] = clamp_i8(hidden[i] + out[i])
    return out


def generate_layer_vectors(theta_v_hash: str, output_dir: Path):
    """Generiert Golden Vectors fuer komplette Transformer-Layer.

    Seit v0.12.36 werden die Vektoren vom Rust-Binary ``golden_generate``
    mit dem echten kalibrierten Modell erzeugt (kein Dummy-Forward mehr).
    """
    import subprocess
    project_root = Path(__file__).parent.parent.parent
    runtime_dir = project_root / "runtime"
    artifact_dir = project_root / "artifacts" / MANIFEST_MODELL
    # ACHTUNG: `golden_generate` haengt selbst `vectors/` an den uebergebenen
    # Pfad an (runtime/src/bin/golden_generate.rs). Uebergeben wird deshalb
    # `tests/golden`, NICHT `tests/golden/vectors` — sonst landen die Vektoren
    # in `vectors/vectors/`, waehrend `validate.py` weiter aus `vectors/layer`
    # und `vectors/e2e` liest. Genau das ist am 2026-08-20 zum zweiten Mal
    # passiert; das verwaiste Duplikat war sogar eingecheckt.
    golden_dir = VECTORS_DIR.parent
    assert golden_dir.name == "golden", f"unerwarteter Zielpfad: {golden_dir}"

    cmd = [
        "cargo", "run", "--bin", "golden_generate",
        "--quiet", "--",
        str(artifact_dir), str(golden_dir),
    ]
    result = subprocess.run(cmd, cwd=runtime_dir, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"[golden] FEHLER bei golden_generate: {result.stderr}", file=sys.stderr)
        sys.exit(1)
    # Ausgabe des Binaries weiterleiten
    for line in result.stdout.strip().splitlines():
        print(f"[golden] {line}")


def dummy_forward_token(token_id, pos, num_layers, hidden_size, vocab_size, seed):
    emb_table = deterministic_tensor(vocab_size, hidden_size, seed)
    hidden = emb_table[token_id * hidden_size:(token_id + 1) * hidden_size]

    for layer_idx in range(num_layers):
        layer_seed = (seed + layer_idx + 1) & 0xFFFFFFFFFFFFFFFF
        w = deterministic_tensor(hidden_size, hidden_size, layer_seed)
        out = [0] * hidden_size
        for row in range(hidden_size):
            acc = 0
            for col in range(hidden_size):
                acc += w[row * hidden_size + col] * hidden[col]
            out[row] = clamp_i8(rshift_round(acc, 14))
        for i in range(hidden_size):
            hidden[i] = clamp_i8(hidden[i] + out[i])

    lm_head = deterministic_tensor(vocab_size, hidden_size, (seed + num_layers + 1) & 0xFFFFFFFFFFFFFFFF)
    logits = [0] * vocab_size
    for row in range(vocab_size):
        acc = 0
        for col in range(hidden_size):
            acc += lm_head[row * hidden_size + col] * hidden[col]
        logits[row] = rshift_round(acc, 14)

    best_i = 0
    best_v = logits[0]
    for i in range(1, vocab_size):
        if logits[i] > best_v:
            best_v = logits[i]
            best_i = i
    return best_i


def generate_e2e_vectors(theta_v_hash: str, output_dir: Path):
    """Generiert End-to-End Golden Vectors (Prompt -> Tokens).

    Seit v0.12.36 werden die Vektoren vom Rust-Binary ``golden_generate``
    mit dem echten kalibrierten Modell und Tokenizer erzeugt.
    """
    # golden_generate erzeugt Layer- und E2E-Vektoren in einem Aufruf.
    # Diese Funktion ist ein No-Op, da generate_layer_vectors den
    # Rust-Aufruf bereits getaetigt hat.
    pass


def main():
    # theta_v_hash = SHA-256 der eingebetteten Ausfuehrungsspezifikation.
    # Identisch zu loader::spec_hash() im Rust-Runtime.
    spec_path = Path(__file__).parent.parent.parent / "theta_v" / "spec.json"
    spec_bytes = spec_path.read_bytes()
    theta_v_hash = "sha256:" + hashlib.sha256(spec_bytes).hexdigest()

    generate_op_vectors(theta_v_hash, VECTORS_DIR / "op")
    generate_layer_vectors(theta_v_hash, VECTORS_DIR / "layer")
    generate_e2e_vectors(theta_v_hash, VECTORS_DIR / "e2e")

    schreibe_manifest(theta_v_hash)

    print("[golden] Alle Golden Vectors erzeugt.")


def schreibe_manifest(theta_v_hash: str) -> None:
    """Verzeichnet, zu welchem Artefakt die Layer-/E2E-Vektoren gehören.

    Der Prüfer liest das Manifest, bevor er die Vektoren gegen ein
    Artefakt laufen lässt: Passt das gewählte Artefakt nicht zum
    verzeichneten Modell, sind die Vektoren kein Maßstab für es, und der
    Lauf wird übersprungen statt blind geladen. Ohne diese Zeile stünde
    die Bindung nur im Erzeugerskript — und damit nirgends, wo ein
    Prüfer sie sieht.
    """
    manifest = {
        "modell": MANIFEST_MODELL,
        "theta_v_hash": theta_v_hash,
    }
    text = json.dumps(manifest, indent=2, ensure_ascii=False) + "\n"
    # Einmal neben die erzeugten Vektoren …
    VECTORS_DIR.joinpath("manifest.json").write_text(text, encoding="utf-8")
    # … und einmal dorthin, wo der Konformitätslauf die Vektoren liest.
    conformance = Path(__file__).parent.parent.parent / "conformance" / "vectors"
    if conformance.is_dir():
        conformance.joinpath("manifest.json").write_text(text, encoding="utf-8")


if __name__ == "__main__":
    main()
