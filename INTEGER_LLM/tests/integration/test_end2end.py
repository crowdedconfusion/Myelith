#!/usr/bin/env python3
"""
End-to-End Integrationstest fuer Single-Node Integer-Inferenz.

Prueft:
1. Rust-Runtime kompiliert
2. Modell laedt aus einem vollstaendigen, synthetischen Artefakt (echte
   Gewichts-/Skalen-/LUT-Validierung ueber das kompilierte Binary, nicht nur
   in Rust-Unit-Tests)
3. Forward-Pass laeuft durch
4. Ausgabe ist deterministisch (gleicher Prompt -> gleicher Hash)
5. Unvollstaendige oder manipulierte Artefakte werden abgelehnt, nicht
   stillschweigend geladen

Bis Punkt 12.10 nutzte dieser Test ein leeres Artefakt-Verzeichnis,
das der Loader mit Dummy-Gewichten auffuellte. Seit 12.10 laedt load_model()
ausschliesslich vollstaendige, echte Artefakte (siehe runtime/src/loader.rs);
ein leeres Verzeichnis schlaegt seither korrekt fehl. Dieser Test baut daher
ein vollstaendiges synthetisches Artefakt, das exakt dem Format folgt, das
calibrate/ tatsaechlich exportiert (weights_manifest.json, scales.json,
luts.json, model_config.json) - kleine Dimensionen, aber strukturell
identisch zu einem echten Export.
"""

import hashlib
import json
import struct
import subprocess
from pathlib import Path

RUNTIME_DIR = Path(__file__).parent.parent.parent / "runtime"
import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parent.parent))
import cargo_paths  # noqa: E402

BINARY = cargo_paths.binary("runtime", "integer-llm-runtime")
SPEC_JSON_PATH = Path(__file__).parent.parent.parent / "theta_v" / "spec.json"

# Kleine, aber strukturell vollstaendige Dimensionen (dieselbe GQA-Asymmetrie
# wie das echte Qwen2.5-0.5B: num_heads != num_kv_heads).
HIDDEN = 4
NUM_HEADS = 2
NUM_KV_HEADS = 1
HEAD_DIM = 2
INTERMEDIATE = 4
VOCAB = 4  # deckt den Test-Tokenizer ("H","e","l","o") ab


def _sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _write_json(path: Path, obj) -> None:
    path.write_text(json.dumps(obj, sort_keys=True, separators=(",", ":")))


def _spec_version() -> str:
    """Liest die theta_v-Version aus der echten spec.json - dieselbe Datei,
    die runtime/src/loader.rs per include_str! einbettet. Kein Hardcoding
    hier, damit der Test nicht stillschweigend von der echten Spezifikation
    abweicht."""
    spec = json.loads(SPEC_JSON_PATH.read_text())
    return spec["theta_v"]["version"]


def write_theta_v(root: Path) -> None:
    """Schreibt theta_v.json mit der echten spec-Version und echten Hashes
    der bereits vorhandenen weights_manifest.json/scales.json/luts.json -
    muss nach diesen drei Dateien aufgerufen werden (siehe
    build_synthetic_artifact), und erneut, wenn ein Test eine dieser Dateien
    nachtraeglich veraendert und dabei gezielt NICHT den theta_v-Hash-Check
    aus 12.13 testen will."""
    weights_hash = _sha256_hex((root / "weights_manifest.json").read_bytes())
    scales_hash = _sha256_hex((root / "scales.json").read_bytes())
    luts_hash = _sha256_hex((root / "luts.json").read_bytes())
    _write_json(root / "theta_v.json", {
        "version": _spec_version(),
        "weights_hash": weights_hash,
        "scales_hash": scales_hash,
        "luts_hash": luts_hash,
    })


def build_synthetic_artifact(root: Path, tie_word_embeddings: bool = True) -> None:
    """
    Baut ein vollstaendiges Artefakt-Verzeichnis im selben Format, das
    calibrate/ exportiert (weights_manifest.json aus export_weights.py,
    scales.json aus scales.py, luts.json aus export.py, model_config.json
    wie in runtime/src/loader.rs::ModelDims erwartet).
    """
    root.mkdir(parents=True, exist_ok=True)

    _write_json(root / "model_config.json", {
        "family": "test",
        "variant": "synthetic",
        "num_layers": 1,
        "hidden_size": HIDDEN,
        "intermediate_size": INTERMEDIATE,
        "num_heads": NUM_HEADS,
        "num_kv_heads": NUM_KV_HEADS,
        "head_dim": HEAD_DIM,
        "vocab_size": VOCAB,
        "max_context": 8,
        "tie_word_embeddings": tie_word_embeddings,
        "attention_bias": True,
    })

    (root / "tokenizer.json").write_text(
        '{"version":"1.0","model":{"type":"BPE",'
        '"vocab":{"H":0,"e":1,"l":2,"o":3},"merges":[]}}'
    )

    # Per-Layer-Aktivierungsskalen: seit v0.12.20 Pflicht (der Forward-Pass
    # verbraucht alle Eintraege; Schluessel-Konvention identisch zu
    # calibrate/src/stats.py).
    _scale = lambda shift, absmax: {
        "shift": shift, "scale": 2.0 ** (-shift), "absmax_observed": absmax,
    }
    _write_json(root / "scales.json", {
        "model.layers.0.input_layernorm": _scale(4, 10.0),
        "model.layers.0.self_attn.q_proj": _scale(5, 20.0),
        "model.layers.0.self_attn.k_proj": _scale(5, 20.0),
        "model.layers.0.self_attn.v_proj": _scale(5, 20.0),
        "model.layers.0.self_attn": _scale(6, 15.0),
        "model.layers.0.post_attention_layernorm": _scale(3, 40.0),
        "model.layers.0.mlp.gate_proj": _scale(4, 30.0),
        "model.layers.0.mlp.up_proj": _scale(3, 60.0),
        "model.layers.0.mlp.down_proj.input": _scale(0, 100.0),
        "model.layers.0.input_layernorm.input": _scale(12, 0.06),
        "model.layers.0.post_attention_layernorm.input": _scale(5, 25.0),
        "model.norm": _scale(2, 120.0),
        "model.norm.input": _scale(4, 80.0),
    })

    # Gewichte: dieselben Tensor-Namen wie calibrate/src/quantize.py erzeugt.
    weights_manifest = {}

    def put_weight(original_name: str, shape):
        n = 1
        for d in shape:
            n *= d
        data = bytes((i % 7) for i in range(n))
        safe_name = original_name.replace(".", "_")
        (root / f"{safe_name}.bin").write_bytes(data)
        weights_manifest[safe_name] = {
            "original_name": original_name,
            "file": f"{safe_name}.bin",
            "shape": list(shape),
            "scale": 1.0,
            "shift": 0,
            "dtype": "int8",
            "hash": _sha256_hex(data),
        }

    def put_bias(original_name: str, n: int):
        """Attention-Bias im int16-Format (theta_v 0.13.0, Fund 23).

        Biases lagen bis 0.12.0 in int8 und saettigten dort still bei
        Betraegen ueber 127; bei Qwen2.5-7B traf das k_proj.bias mit
        Werten bis 414. Das Fixture spiegelt bewusst das ECHTE Format,
        nicht ein vereinfachtes (Projektkonvention: Tests arbeiten mit
        strukturell identischen Artefakten).
        """
        import struct
        werte = [(i % 11) - 5 for i in range(n)]
        data = b"".join(struct.pack("<h", w) for w in werte)
        shifts = bytes((i % 3) for i in range(n))
        safe_name = original_name.replace(".", "_")
        (root / f"{safe_name}.bin").write_bytes(data)
        (root / f"{safe_name}_shifts.bin").write_bytes(shifts)
        weights_manifest[safe_name] = {
            "original_name": original_name,
            "file": f"{safe_name}.bin",
            "shape": [n],
            "scale": -1.0,
            "shift": -1,
            "dtype": "int16",
            "hash": _sha256_hex(data),
            "shifts_file": f"{safe_name}_shifts.bin",
            "shifts_hash": _sha256_hex(shifts),
        }

    put_weight("model.embed_tokens.weight", [VOCAB, HIDDEN])
    put_weight("model.norm.weight", [HIDDEN])
    if not tie_word_embeddings:
        put_weight("lm_head.weight", [VOCAB, HIDDEN])
    put_weight("model.layers.0.input_layernorm.weight", [HIDDEN])
    put_weight("model.layers.0.post_attention_layernorm.weight", [HIDDEN])
    put_weight("model.layers.0.self_attn.q_proj.weight", [NUM_HEADS * HEAD_DIM, HIDDEN])
    put_weight("model.layers.0.self_attn.k_proj.weight", [NUM_KV_HEADS * HEAD_DIM, HIDDEN])
    put_weight("model.layers.0.self_attn.v_proj.weight", [NUM_KV_HEADS * HEAD_DIM, HIDDEN])
    # Attention-Biases wie im echten Qwen2.5-Format (attention_bias=true):
    # Laenge = Ausgabe-Dimension der Projektion.
    put_bias("model.layers.0.self_attn.q_proj.bias", NUM_HEADS * HEAD_DIM)
    put_bias("model.layers.0.self_attn.k_proj.bias", NUM_KV_HEADS * HEAD_DIM)
    put_bias("model.layers.0.self_attn.v_proj.bias", NUM_KV_HEADS * HEAD_DIM)
    put_weight("model.layers.0.self_attn.o_proj.weight", [HIDDEN, NUM_HEADS * HEAD_DIM])
    put_weight("model.layers.0.mlp.gate_proj.weight", [INTERMEDIATE, HIDDEN])
    put_weight("model.layers.0.mlp.up_proj.weight", [INTERMEDIATE, HIDDEN])
    put_weight("model.layers.0.mlp.down_proj.weight", [HIDDEN, INTERMEDIATE])

    _write_json(root / "weights_manifest.json", weights_manifest)

    # LUTs: raw int16 little-endian, Format aus calibrate/src/export.py.
    luts_manifest = {}

    def put_lut(name: str, values):
        raw = struct.pack(f"<{len(values)}h", *values)
        (root / f"{name}.lut.bin").write_bytes(raw)
        luts_manifest[name] = {
            "file": f"{name}.lut.bin",
            "hash": _sha256_hex(raw),
            "length": len(values),
            "dtype": "int16",
        }

    put_lut("cos", [256, 0, -256, 0])
    put_lut("sin", [0, 256, 0, -256])
    put_lut("exp", [256, 128, 64])
    put_lut("silu", [-10, 0, 10, 20])
    put_lut("rsqrt", [256, 181, 148])

    _write_json(root / "luts.json", luts_manifest)

    # theta_v.json zuletzt: Version und Hashes muessen zu den gerade
    # geschriebenen Manifest-Dateien passen (Punkt 12.13).
    write_theta_v(root)


def _run(artifact_dir: Path, prompt: str = "Hello", max_tokens: str = "5"):
    return subprocess.run(
        [str(BINARY), str(artifact_dir), prompt, max_tokens],
        capture_output=True,
        text=True,
    )


def test_runtime_compiles():
    """Kompiliert die Rust-Runtime."""
    result = subprocess.run(
        ["cargo", "build", "--release", "--features", "reference"],
        cwd=RUNTIME_DIR,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"Compile failed: {result.stderr}"


def test_forward_determinism(tmp_path=None):
    """
    Fuehrt die Runtime zweimal mit gleichem Prompt gegen ein vollstaendiges
    synthetisches Artefakt aus. Erwartet: Gleiche Token-Hash-Ausgabe.
    """
    artifact_dir = (tmp_path or Path(__file__).parent / "_synthetic_artifact")
    build_synthetic_artifact(artifact_dir)

    def run():
        result = _run(artifact_dir)
        assert result.returncode == 0, f"Run failed: {result.stderr}"
        lines = result.stdout.strip().split("\n")
        hash_line = [l for l in lines if "Token-Hash:" in l][0]
        return hash_line.split(":")[1].strip()

    hash1 = run()
    hash2 = run()
    assert hash1 == hash2, f"Non-deterministic: {hash1} != {hash2}"
    print(f"[test] Deterministic hash: {hash1}")


def test_forward_works_untied_embeddings(tmp_path=None):
    """Dasselbe, aber mit eigenstaendigem lm_head.weight statt Weight-Tying."""
    artifact_dir = (tmp_path or Path(__file__).parent / "_synthetic_artifact_untied")
    build_synthetic_artifact(artifact_dir, tie_word_embeddings=False)

    result = _run(artifact_dir)
    assert result.returncode == 0, f"Run failed: {result.stderr}"
    assert "Token-Hash:" in result.stdout


def test_rejects_incomplete_artifact(tmp_path=None):
    """
    Ein Artefakt, dem ein Pflichtgewicht fehlt, muss ueber das kompilierte
    Binary sauber abgelehnt werden (Exit-Code != 0, keine Panic-Backtrace,
    Fehlermeldung nennt den fehlenden Tensor) - echte Gewichts-Validierung,
    nicht nur strukturelles Vorhandensein der Artefakt-Dateien.
    """
    artifact_dir = (tmp_path or Path(__file__).parent / "_incomplete_artifact")
    build_synthetic_artifact(artifact_dir)

    # model.norm.weight entfernen: Datei und Manifest-Eintrag. theta_v.json
    # danach neu schreiben, damit der Hash-Check aus 12.13 (der jetzt VOR dem
    # Tensor-Lookup laeuft) hier nicht schon vorher zuschlaegt - dieser Test
    # soll gezielt den "Gewicht fehlt"-Pfad pruefen, nicht die
    # Manifest-Konsistenzpruefung (siehe test_rejects_tampered_manifest_hash).
    (artifact_dir / "model_norm_weight.bin").unlink()
    manifest_path = artifact_dir / "weights_manifest.json"
    manifest = json.loads(manifest_path.read_text())
    del manifest["model_norm_weight"]
    _write_json(manifest_path, manifest)
    write_theta_v(artifact_dir)

    result = _run(artifact_dir)
    assert result.returncode != 0, "Unvollstaendiges Artefakt haette fehlschlagen muessen"
    assert "panicked" not in result.stderr.lower(), (
        f"Erwartete saubere Fehlermeldung, kein Panic: {result.stderr}"
    )
    assert "model.norm.weight" in result.stderr, f"Fehlermeldung: {result.stderr}"


def test_rejects_corrupted_weight_hash(tmp_path=None):
    """
    Ein Gewicht, dessen Bytes nach dem Export manipuliert wurden (Hash im
    Manifest stimmt nicht mehr), muss abgelehnt werden - direkter Test der
    SHA-256-Integritaetspruefung aus dem Loader gegen das echte Binary.
    """
    artifact_dir = (tmp_path or Path(__file__).parent / "_corrupted_artifact")
    build_synthetic_artifact(artifact_dir)

    weight_file = artifact_dir / "model_embed_tokens_weight.bin"
    corrupted = bytearray(weight_file.read_bytes())
    corrupted[0] ^= 0xFF
    weight_file.write_bytes(bytes(corrupted))

    result = _run(artifact_dir)
    assert result.returncode != 0, "Manipuliertes Gewicht haette fehlschlagen muessen"
    assert "panicked" not in result.stderr.lower(), (
        f"Erwartete saubere Fehlermeldung, kein Panic: {result.stderr}"
    )
    assert "SHA-256" in result.stderr, f"Fehlermeldung: {result.stderr}"


def test_rejects_missing_artifact_dir():
    """Fehlendes Artefakt-Verzeichnis muss die CLI-Pruefung aus 12.11 greifen."""
    result = _run(Path("/tmp/integer-llm-does-not-exist-e2e-test"))
    assert result.returncode != 0
    assert "nicht gefunden" in result.stderr


def test_rejects_theta_v_version_mismatch(tmp_path=None):
    """
    theta_v.json mit einer Version, die nicht zur im Binary eingebetteten
    spec.json passt, muss abgelehnt werden - auch wenn alle Hashes korrekt
    sind. Direkter End-to-End-Test von ThetaV::verify_version_against_spec()
    (Punkt 12.13) gegen das echte Binary.
    """
    artifact_dir = (tmp_path or Path(__file__).parent / "_badversion_artifact")
    build_synthetic_artifact(artifact_dir)

    theta_v_path = artifact_dir / "theta_v.json"
    manifest = json.loads(theta_v_path.read_text())
    manifest["version"] = "0.0.0-stale"
    _write_json(theta_v_path, manifest)

    result = _run(artifact_dir)
    assert result.returncode != 0, "Versions-Mismatch haette fehlschlagen muessen"
    assert "panicked" not in result.stderr.lower(), (
        f"Erwartete saubere Fehlermeldung, kein Panic: {result.stderr}"
    )
    assert "theta_v-Version" in result.stderr, f"Fehlermeldung: {result.stderr}"


def test_rejects_tampered_manifest_hash(tmp_path=None):
    """
    weights_manifest.json nach dem Export veraendert (z. B. ein
    Metadaten-Feld), ohne dass theta_v.json nachgezogen wurde: der
    Manifest-Hash passt nicht mehr, unabhaengig davon, ob einzelne Tensoren
    noch ladbar waeren. Prueft die allgemeine Konsistenzpruefung aus 12.13,
    nicht nur den Spezialfall "Gewicht fehlt".
    """
    artifact_dir = (tmp_path or Path(__file__).parent / "_tamperedmanifest_artifact")
    build_synthetic_artifact(artifact_dir)

    manifest_path = artifact_dir / "weights_manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["model_norm_weight"]["scale"] = 2.0
    _write_json(manifest_path, manifest)
    # theta_v.json bewusst NICHT nachziehen - genau das soll der Hash-Check fangen.

    result = _run(artifact_dir)
    assert result.returncode != 0, "Manipuliertes Manifest haette fehlschlagen muessen"
    assert "panicked" not in result.stderr.lower(), (
        f"Erwartete saubere Fehlermeldung, kein Panic: {result.stderr}"
    )
    assert "hash mismatch" in result.stderr, f"Fehlermeldung: {result.stderr}"


if __name__ == "__main__":
    test_runtime_compiles()
    print("[test] Compile: PASSED")
    test_forward_determinism()
    print("[test] Determinism (tied embeddings): PASSED")
    test_forward_works_untied_embeddings()
    print("[test] Determinism (untied embeddings): PASSED")
    test_rejects_incomplete_artifact()
    print("[test] Rejects incomplete artifact: PASSED")
    test_rejects_corrupted_weight_hash()
    print("[test] Rejects corrupted weight hash: PASSED")
    test_rejects_missing_artifact_dir()
    print("[test] Rejects missing artifact dir: PASSED")
    test_rejects_theta_v_version_mismatch()
    print("[test] Rejects theta_v version mismatch: PASSED")
    test_rejects_tampered_manifest_hash()
    print("[test] Rejects tampered manifest hash: PASSED")
    print("[test] All integration tests PASSED")
