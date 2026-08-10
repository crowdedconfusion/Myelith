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

import json


def clamp_i8(x: int) -> int:
    if x < -128:
        return -128
    elif x > 127:
        return 127
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


def sqrt_q(x: int, frac_bits: int) -> int:
    if x <= 0:
        return 0
    target = x << frac_bits
    lo = 0
    hi = min(target + 1, 2**31 - 1)
    while lo < hi:
        mid = (lo + hi + 1) >> 1
        if mid > 0 and mid <= target // mid:
            lo = mid
        else:
            hi = mid - 1
    return lo


def rsqrt_q(x: int, frac_bits: int) -> int:
    if x <= 0:
        return 1 << frac_bits
    s = sqrt_q(x, frac_bits)
    if s == 0:
        return 1 << frac_bits
    val = (1 << (2 * frac_bits)) // s
    if val < -32768:
        return -32768
    elif val > 32767:
        return 32767
    return val


def rmsnorm_int8(x, gamma, frac_bits, eps):
    n = len(x)
    acc = sum(v * v for v in x)
    mean_sq = acc // n
    rms = rsqrt_q(mean_sq + eps, frac_bits)
    if rms == 0:
        return [0] * n
    one = 1 << frac_bits
    out = []
    for v, g in zip(x, gamma):
        y = (v << frac_bits) // rms
        y2 = (y * g) // one
        out.append(clamp_i8(y2))
    return out


def rescale(acc: int, in_frac: int, out_frac: int) -> int:
    shift = in_frac - out_frac
    if shift >= 0:
        return rshift_round(acc, shift)
    else:
        return acc << (-shift)


def linear_w8a8_ref(x, W, act_frac, weight_frac, out_frac):
    in_frac = act_frac + weight_frac
    out = []
    for row in W:
        acc = sum(w * v for w, v in zip(row, x))
        y = rescale(acc, in_frac, out_frac)
        out.append(clamp_i8(y))
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
    """Generiert Golden Vectors fuer einzelne Operationen."""
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # RMSNorm
    gv = GoldenVector("rmsnorm_basic", "op", theta_v_hash)
    gv.add_input("x", [64, 64, -64, 0, 32, -32], "int8")
    gv.add_input("gamma", [64, 64, 64, 64, 64, 64], "int8")
    gv.metadata = {"frac_bits": 6, "eps": 1}
    y = rmsnorm_int8([64, 64, -64, 0, 32, -32], [64, 64, 64, 64, 64, 64], 6, 1)
    gv.add_output("y", y, "int8")
    gv.save(output_dir / "rmsnorm_basic.golden.json")
    
    # Linear W8A8
    # Eigentlich 128 = 2^7 (Identitaet bei weight_frac 7), aber int8 ist laut
    # theta_v auf 127 begrenzt; via RNE-Rundung bleibt das Ergebnis exakt.
    gv = GoldenVector("linear_w8a8_identity", "op", theta_v_hash)
    gv.add_input("x", [64, -64], "int8")
    gv.metadata = {"W": [[127, 0], [0, 127]], "act_frac": 6, "weight_frac": 7, "out_frac": 6}
    y = linear_w8a8_ref([64, -64], [[127, 0], [0, 127]], 6, 7, 6)
    gv.add_output("y", y, "int8")
    gv.save(output_dir / "linear_w8a8_identity.golden.json")
    
    # Softmax
    gv = GoldenVector("softmax_basic", "op", theta_v_hash)
    gv.add_input("logits", [100, 200, 50, 300], "int32")
    gv.metadata = {"lut_shift": 0, "frac_bits": 8}
    exp_lut = [int(round(math.exp(-i / 256.0) * 256)) for i in range(128)]
    y = softmax_int_ref([100, 200, 50, 300], exp_lut, 0, 8)
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
    """Generiert Golden Vectors fuer komplette Transformer-Layer."""
    output_dir.mkdir(parents=True, exist_ok=True)
    
    for layer_idx in range(24):
        gv = GoldenVector(f"transformer_layer_{layer_idx}", "layer", theta_v_hash)
        seed_in = int(hashlib.sha256(f"{theta_v_hash}:layer_in:{layer_idx}".encode()).hexdigest(), 16)
        hidden_in = [clamp_i8((seed_in >> (i % 32)) ^ (i * 7)) for i in range(896)]
        hidden_out = dummy_layer_forward(hidden_in, layer_idx, theta_v_hash)

        gv = GoldenVector(f"transformer_layer_{layer_idx}", "layer", theta_v_hash)
        gv.add_input("hidden", hidden_in, "int8")
        gv.add_input("position", [0], "int32")
        gv.metadata = {"layer_idx": layer_idx, "seq_len": 1}
        gv.add_output("hidden_out", hidden_out, "int8")
        gv.save(output_dir / f"layer_{layer_idx:02d}.golden.json")
    
    print(f"[golden] 24 Layer-Vektoren erzeugt.")


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
    """Generiert End-to-End Golden Vectors (Prompt -> Tokens)."""
    output_dir.mkdir(parents=True, exist_ok=True)

    seed = int(hashlib.sha256(f"{theta_v_hash}:e2e".encode()).hexdigest(), 16) % (2**64)
    
    test_prompts = [
        ("hello", [ord(c) for c in "hello"]),
        ("world", [ord(c) for c in "world"]),
        ("test", [ord(c) for c in "test"]),
    ]

    for prompt, prompt_tokens in test_prompts:
        gv = GoldenVector(f"e2e_prompt_{prompt}", "e2e", theta_v_hash)
        gv.add_input("prompt_tokens", prompt_tokens, "int32")
        gv.metadata = {"max_new_tokens": 3, "greedy": True, "seed": 42}

        tokens = []
        current_seed = seed
        next_token = prompt_tokens[0] if prompt_tokens else 0
        for _ in range(3):
            next_token = dummy_forward_token(next_token, len(tokens), 24, 896, 151936, current_seed)
            tokens.append(next_token)
            current_seed = (current_seed + 0x9E3779B97F4A7C15) & 0xFFFFFFFFFFFFFFFF

        gv.add_output("tokens", tokens, "int32")
        gv.save(output_dir / f"e2e_{prompt}.golden.json")
    
    print(f"[golden] {len(test_prompts)} E2E-Vektoren erzeugt.")


def main():
    theta_v_hash = "sha256:abc123def456"  # Wuerde aus spec.json berechnet

    generate_op_vectors(theta_v_hash, VECTORS_DIR / "op")
    generate_layer_vectors(theta_v_hash, VECTORS_DIR / "layer")
    generate_e2e_vectors(theta_v_hash, VECTORS_DIR / "e2e")
    
    print("[golden] Alle Golden Vectors erzeugt.")


if __name__ == "__main__":
    main()
