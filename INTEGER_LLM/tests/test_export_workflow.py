#!/usr/bin/env python3
"""
Tests fuer den Export-Workflow (Fahrplan-Punkte 12.14/12.15): calibrate/src/export.py,
calibrate/src/export_weights.py, calibrate/src/paths.py, calibrate/src/model_configs.py.

Bewusst ohne torch/numpy-Abhaengigkeit (nicht in jeder Umgebung installiert,
insbesondere nicht in einer reinen Test-Sandbox): ein winziger Fake-Array-Typ
mit .tobytes() ersetzt numpy fuer den Zweck dieses Tests. Die eigentliche
Quantisierung (quantize.py) wird hier nicht getestet - dafuer siehe
tests/test_calibration.py.

Letzter Test laeuft zusaetzlich das echte kompilierte Runtime-Binary gegen
das so erzeugte synthetische Artefakt - der eigentliche Beweis, dass der
Python-Export und der Rust-Loader zueinander passen, nicht nur behauptet wird.

Eigenstaendiges Skript nach Projektkonvention (siehe test_fixed_point.py),
kein pytest.
"""

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate"))

from src import export as export_mod
from src import paths as paths_mod
from src.export_weights import export_quantized_weights
from src.model_configs import get_export_model_config, get_model_config

RUNTIME_DIR = Path(__file__).parent.parent / "runtime"
BINARY = RUNTIME_DIR / "target" / "release" / "integer-llm-runtime"


class FakeInt8Array:
    """Ersetzt numpy.ndarray fuer den Zweck dieses Tests: nur .tobytes() noetig."""

    def __init__(self, values):
        self._bytes = bytes((v & 0xFF) for v in values)

    def tobytes(self):
        return self._bytes


def _sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def test_get_export_model_config_accepts_verified_variant():
    config = get_export_model_config("qwen2.5-0.5b")
    assert config["num_kv_heads"] == 2
    assert config["tie_word_embeddings"] is True


def test_get_export_model_config_rejects_unverified_variant():
    # 7B hat (noch) kein verifiziertes num_kv_heads/tie_word_embeddings.
    assert "num_kv_heads" not in get_model_config("qwen2.5-7b-instruct")
    try:
        get_export_model_config("qwen2.5-7b-instruct")
        raise AssertionError("Unvollstaendige Variante haette fehlschlagen muessen")
    except ValueError as e:
        assert "num_kv_heads" in str(e)


def test_paths_artifacts_dir_default_and_env():
    os.environ.pop(paths_mod.ARTIFACTS_DIR_ENV, None)
    assert paths_mod.artifacts_dir() == Path("artifacts")
    assert paths_mod.model_artifacts_dir("x") == Path("artifacts") / "x"

    os.environ[paths_mod.ARTIFACTS_DIR_ENV] = "/tmp/integer-llm-calibrate-test"
    assert paths_mod.artifacts_dir() == Path("/tmp/integer-llm-calibrate-test")
    os.environ.pop(paths_mod.ARTIFACTS_DIR_ENV, None)


def test_spec_version_reads_real_spec_json():
    spec_path = Path(__file__).parent.parent / "theta_v" / "spec.json"
    expected = json.loads(spec_path.read_text())["theta_v"]["version"]
    assert export_mod.spec_version() == expected


def test_export_theta_v_requires_weights_manifest_first():
    with tempfile.TemporaryDirectory() as tmp:
        out_dir = Path(tmp)
        export_mod.export_json({}, out_dir / "scales.json")
        try:
            export_mod.export_theta_v(scales={}, luts={}, output_dir=out_dir)
            raise AssertionError("Fehlendes weights_manifest.json haette fehlschlagen muessen")
        except FileNotFoundError as e:
            assert "weights_manifest.json" in str(e)


def test_export_weights_rejects_shape_byte_mismatch():
    with tempfile.TemporaryDirectory() as tmp:
        quantized = {
            "model.norm.weight": {
                "int8": FakeInt8Array([1, 2, 3]),
                "shape": [2, 4],  # 8 Bytes erwartet, 3 geliefert
                "scale": 1.0,
                "shift": 0,
            },
        }
        try:
            export_quantized_weights(quantized, Path(tmp))
            raise AssertionError("Shape/Byte-Laengen-Divergenz haette fehlschlagen muessen")
        except ValueError as e:
            assert "shape" in str(e)


def test_export_weights_rejects_wrong_dtype():
    class FakeTypedArray(FakeInt8Array):
        def __init__(self, values, dtype):
            super().__init__(values)
            self.dtype = dtype

    with tempfile.TemporaryDirectory() as tmp:
        quantized = {
            "model.norm.weight": {
                "int8": FakeTypedArray([1, 2, 3, 4], "float32"),
                "shape": [4],
                "scale": 1.0,
                "shift": 0,
            },
        }
        try:
            export_quantized_weights(quantized, Path(tmp))
            raise AssertionError("Nicht-int8-Tensor haette fehlschlagen muessen")
        except ValueError as e:
            assert "int8" in str(e)


def test_export_weights_hashes_match_manifest():
    """Jeder Manifest-Eintrag muss den tatsaechlichen SHA-256 der Datei tragen."""
    with tempfile.TemporaryDirectory() as tmp:
        out_dir = Path(tmp)
        quantized = {
            "model.embed_tokens.weight": {
                "int8": FakeInt8Array([1, 2, 3, 4, 5, 6, 7, 8]),
                "shape": [2, 4],
                "scale": 0.5,
                "shift": 1,
            },
            "model.norm.weight": {
                "int8": FakeInt8Array([64, 64, 64, 64]),
                "shape": [4],
                "scale": 1.0,
                "shift": 0,
            },
        }
        manifest = export_quantized_weights(quantized, out_dir)
        assert len(manifest) == 2
        for safe_name, entry in manifest.items():
            file_bytes = (out_dir / entry["file"]).read_bytes()
            assert entry["hash"] == _sha256_hex(file_bytes), safe_name
            assert entry["dtype"] == "int8"
            n = 1
            for d in entry["shape"]:
                n *= d
            assert len(file_bytes) == n, safe_name


def test_local_model_dir_missing_and_present():
    with tempfile.TemporaryDirectory() as tmp:
        old_cwd = os.getcwd()
        os.chdir(tmp)
        try:
            try:
                paths_mod.local_model_dir("Qwen2.5-0.5B")
                raise AssertionError("Fehlendes Modell-Verzeichnis haette fehlschlagen muessen")
            except FileNotFoundError as e:
                assert "fetch_model.sh" in str(e)
            (Path(tmp) / "models" / "Qwen2.5-0.5B").mkdir(parents=True)
            assert paths_mod.local_model_dir("Qwen2.5-0.5B") == Path("models") / "Qwen2.5-0.5B"
        finally:
            os.chdir(old_cwd)


def test_export_workflow_order_produces_consistent_theta_v():
    """
    Simuliert main.py's Reihenfolge (Gewichte -> model_config.json -> theta_v)
    mit synthetischen Daten und prueft, dass theta_v.json am Ende echte,
    konsistente Hashes traegt - keine Platzhalter, keine falsche Reihenfolge.
    """
    with tempfile.TemporaryDirectory() as tmp:
        out_dir = Path(tmp)

        quantized = {
            "model.embed_tokens.weight": {
                "int8": FakeInt8Array([1, 2, 3, 4, 5, 6, 7, 8]),
                "shape": [2, 4],
                "scale": 1.0,
                "shift": 0,
            },
            "model.norm.weight": {
                "int8": FakeInt8Array([64, 64, 64, 64]),
                "shape": [4],
                "scale": 1.0,
                "shift": 0,
            },
        }
        export_quantized_weights(quantized, out_dir)
        assert (out_dir / "weights_manifest.json").exists()

        scales = {"model.embed_tokens": {"shift": 0, "scale": 1.0, "absmax_observed": 1.0}}
        luts = {"exp": [256, 128, 64]}
        export_mod.export_theta_v(scales=scales, luts=luts, output_dir=out_dir)

        theta_v = json.loads((out_dir / "theta_v.json").read_text())
        real_weights_hash = _sha256_hex((out_dir / "weights_manifest.json").read_bytes())
        real_scales_hash = _sha256_hex((out_dir / "scales.json").read_bytes())
        real_luts_hash = _sha256_hex((out_dir / "luts.json").read_bytes())

        assert theta_v["weights_hash"] == real_weights_hash
        assert theta_v["scales_hash"] == real_scales_hash
        assert theta_v["luts_hash"] == real_luts_hash
        assert theta_v["version"] == export_mod.spec_version()


def test_synthetic_export_loads_in_real_runtime_binary():
    """
    Baut ein minimales, aber vollstaendiges Artefakt ueber genau die Funktionen,
    die main.py verwendet (export_quantized_weights, export_theta_v), und
    prueft, dass das echte kompilierte Runtime-Binary es laedt - der Beweis,
    dass Python-Export und Rust-Loader zueinander passen, nicht nur je fuer
    sich genommen "korrekt aussehen".
    """
    if not BINARY.exists():
        result = subprocess.run(
            ["cargo", "build", "--release", "--features", "reference"],
            cwd=RUNTIME_DIR, capture_output=True, text=True,
        )
        assert result.returncode == 0, f"Compile failed: {result.stderr}"

    hidden, heads, kv_heads, head_dim, inter, vocab = 4, 2, 1, 2, 4, 4

    with tempfile.TemporaryDirectory() as tmp:
        out_dir = Path(tmp)

        def w(shape):
            n = 1
            for d in shape:
                n *= d
            return {"int8": FakeInt8Array(range(n)), "shape": list(shape), "scale": 1.0, "shift": 0}

        quantized = {
            "model.embed_tokens.weight": w([vocab, hidden]),
            "model.norm.weight": w([hidden]),
            "model.layers.0.input_layernorm.weight": w([hidden]),
            "model.layers.0.post_attention_layernorm.weight": w([hidden]),
            "model.layers.0.self_attn.q_proj.weight": w([heads * head_dim, hidden]),
            "model.layers.0.self_attn.k_proj.weight": w([kv_heads * head_dim, hidden]),
            "model.layers.0.self_attn.v_proj.weight": w([kv_heads * head_dim, hidden]),
            "model.layers.0.self_attn.o_proj.weight": w([hidden, heads * head_dim]),
            "model.layers.0.mlp.gate_proj.weight": w([inter, hidden]),
            "model.layers.0.mlp.up_proj.weight": w([inter, hidden]),
            "model.layers.0.mlp.down_proj.weight": w([hidden, inter]),
        }
        export_quantized_weights(quantized, out_dir)

        model_config = {
            "family": "qwen2.5", "variant": "test", "num_layers": 1,
            "hidden_size": hidden, "intermediate_size": inter, "num_heads": heads,
            "num_kv_heads": kv_heads, "head_dim": head_dim, "vocab_size": vocab,
            "max_context": 8, "tie_word_embeddings": True,
            "attention_bias": False,
        }
        (out_dir / "model_config.json").write_text(json.dumps(model_config))

        scales = {}
        luts = {
            "cos": [256, 0, -256, 0], "sin": [0, 256, 0, -256], "exp": [256, 128, 64],
            "silu": [-10, 0, 10, 20], "rsqrt": [256, 181, 148],
        }
        export_mod.export_theta_v(scales=scales, luts=luts, output_dir=out_dir)

        (out_dir / "tokenizer.json").write_text(
            '{"version":"1.0","model":{"type":"BPE",'
            '"vocab":{"H":0,"e":1,"l":2,"o":3},"merges":[]}}'
        )

        result = subprocess.run(
            [str(BINARY), str(out_dir), "Hello", "3"],
            capture_output=True, text=True,
        )
        assert result.returncode == 0, f"Runtime lehnte echtes Export-Format ab: {result.stderr}"
        assert "Token-Hash:" in result.stdout


if __name__ == "__main__":
    test_get_export_model_config_accepts_verified_variant()
    print("[test] get_export_model_config akzeptiert 0.5B: PASSED")
    test_get_export_model_config_rejects_unverified_variant()
    print("[test] get_export_model_config lehnt unvollstaendige Variante ab: PASSED")
    test_paths_artifacts_dir_default_and_env()
    print("[test] paths.artifacts_dir Default/Env: PASSED")
    test_spec_version_reads_real_spec_json()
    print("[test] spec_version liest echte spec.json: PASSED")
    test_export_theta_v_requires_weights_manifest_first()
    print("[test] export_theta_v verlangt vorherige Gewichte: PASSED")
    test_export_weights_rejects_shape_byte_mismatch()
    print("[test] Export lehnt Shape/Byte-Laengen-Divergenz ab: PASSED")
    test_export_weights_rejects_wrong_dtype()
    print("[test] Export lehnt Nicht-int8-Tensoren ab: PASSED")
    test_export_weights_hashes_match_manifest()
    print("[test] Manifest-Hashes stimmen mit Dateien ueberein: PASSED")
    test_local_model_dir_missing_and_present()
    print("[test] local_model_dir prueft models/-Snapshot: PASSED")
    test_export_workflow_order_produces_consistent_theta_v()
    print("[test] Export-Reihenfolge erzeugt konsistente Hashes: PASSED")
    test_synthetic_export_loads_in_real_runtime_binary()
    print("[test] Echtes Runtime-Binary laedt synthetischen Export: PASSED")
    print("[test] Alle Tests bestanden.")
