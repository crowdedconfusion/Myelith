#!/usr/bin/env python3
"""
Tests fuer den Export-Workflow (Punkte 12.14 und 12.15): calibrate/src/export.py,
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
import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parent))
import cargo_paths  # noqa: E402

BINARY = cargo_paths.binary("runtime", "integer-llm-runtime")


class FakeInt8Array:
    """Ersetzt numpy.ndarray fuer den Zweck dieses Tests: nur .tobytes() noetig."""

    def __init__(self, values):
        self._bytes = bytes((v & 0xFF) for v in values)

    def tobytes(self):
        return self._bytes


def _sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def test_get_export_model_config_accepts_verified_variant():
    config = get_export_model_config("myelith-0.6b")
    assert config["num_kv_heads"] == 8
    assert config["tie_word_embeddings"] is True


def test_get_export_model_config_rejects_unverified_variant():
    # Die Instruct-Eintraege sind Groessenangaben fuer die Planung und
    # tragen kein geprueftes num_kv_heads/tie_word_embeddings.
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
                "shifts": np.zeros(2, dtype=np.int8),
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
                "shifts": np.zeros(4, dtype=np.int8),
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
                "shifts": np.array([1, 1], dtype=np.int8),
            },
            "model.norm.weight": {
                "int8": FakeInt8Array([64, 64, 64, 64]),
                "shape": [4],
                "scale": 1.0,
                "shift": 0,
                "shifts": np.zeros(4, dtype=np.int8),
            },
        }
        manifest = export_quantized_weights(quantized, out_dir)
        assert len(manifest) == 2
        for safe_name, entry in manifest.items():
            file_bytes = (out_dir / entry["file"]).read_bytes()
            assert entry["hash"] == _sha256_hex(file_bytes), safe_name
            assert entry["dtype"] == "int8"
            assert entry["scale"] == -1.0 and entry["shift"] == -1  # Sentinels
            shifts_bytes = (out_dir / entry["shifts_file"]).read_bytes()
            assert entry["shifts_hash"] == _sha256_hex(shifts_bytes), safe_name
            assert len(shifts_bytes) == entry["shape"][0], safe_name
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
                paths_mod.local_model_dir("Qwen3-0.6B")
                raise AssertionError("Fehlendes Modell-Verzeichnis haette fehlschlagen muessen")
            except FileNotFoundError as e:
                assert "fetch_model.sh" in str(e)
            (Path(tmp) / "models" / "Qwen3-0.6B").mkdir(parents=True)
            assert paths_mod.local_model_dir("Qwen3-0.6B") == Path("models") / "Qwen3-0.6B"
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
                "shifts": np.zeros(2, dtype=np.int8),
            },
            "model.norm.weight": {
                "int8": FakeInt8Array([64, 64, 64, 64]),
                "shape": [4],
                "scale": 1.0,
                "shift": 0,
                "shifts": np.zeros(4, dtype=np.int8),
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
            return {
                "int8": FakeInt8Array(range(n)),
                "shape": list(shape),
                "scale": 1.0,
                "shift": 0,
                "shifts": np.zeros(shape[0], dtype=np.int8),
            }

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

        # Per-Layer-Aktivierungsskalen: seit v0.12.20 Pflicht (der
        # Forward-Pass verbraucht alle Eintraege).
        _scale = lambda shift: {"shift": shift, "scale": 2.0 ** (-shift), "absmax_observed": 1.0}
        scales = {
            "model.layers.0.input_layernorm": _scale(4),
            "model.layers.0.self_attn.q_proj": _scale(5),
            "model.layers.0.self_attn.k_proj": _scale(5),
            "model.layers.0.self_attn.v_proj": _scale(5),
            "model.layers.0.self_attn": _scale(6),
            "model.layers.0.post_attention_layernorm": _scale(3),
            "model.layers.0.mlp.gate_proj": _scale(4),
            "model.layers.0.mlp.up_proj": _scale(3),
            "model.layers.0.mlp.down_proj.input": _scale(0),
            "model.layers.0.input_layernorm.input": _scale(12),
            "model.layers.0.post_attention_layernorm.input": _scale(5),
            "model.norm": _scale(2),
            "model.norm.input": _scale(4),
        }
        # ⚑ Acht RoPE-Zeilen, damit die Kontextgrenze (min aus max_context 8
        # und den Tabellenzeilen) den Prompt "Hello" (fuenf Token) plus drei
        # erzeugte deckt (Fund 368). head_dim 2 heisst half 1, also eine
        # cos/sin-Zahl je Zeile; vier Zeilen liessen den Lader an Position 4
        # anhalten.
        luts = {
            "cos": [256, 0, -256, 0, 256, 0, -256, 0],
            "sin": [0, 256, 0, -256, 0, 256, 0, -256],
            "exp": [256, 128, 64],
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


# ---------------------------------------------------------------------------
# LM-Head int16/per-channel (spec-Ausnahme 0.6.0, Eskalation nach 12.21).
# Diese Tests brauchen numpy (und für die Quantisierungs-Rundung torch);
# ohne die Abhängigkeiten werden sie übersprungen.
# ---------------------------------------------------------------------------

try:
    import numpy as np
    HAS_NUMPY = True
except ImportError:
    HAS_NUMPY = False

try:
    import torch
    HAS_TORCH = True
except ImportError:
    HAS_TORCH = False


def test_export_lm_head_writes_files_and_manifest():
    if not HAS_NUMPY:
        print("[test] SKIPPED (numpy fehlt): export_lm_head")
        return
    from src.export_weights import export_lm_head

    with tempfile.TemporaryDirectory() as tmp:
        out_dir = Path(tmp)
        # Leeres Manifest voraussetzen (export_lm_head ergänzt den Eintrag).
        (out_dir / "weights_manifest.json").write_text("{}", encoding="utf-8")

        data = np.arange(12, dtype=np.int16).reshape(3, 4)
        shifts = np.array([17, 18, 19], dtype=np.int8)
        entry = export_lm_head({"int16": data, "shifts": shifts, "shape": [3, 4]}, out_dir)

        assert (out_dir / "lm_head.bin").read_bytes() == data.astype("<i2").tobytes()
        assert (out_dir / "lm_head_shifts.bin").read_bytes() == shifts.tobytes()
        assert entry["dtype"] == "int16"
        assert entry["shifts_file"] == "lm_head_shifts.bin"
        assert entry["scale"] == -1.0 and entry["shift"] == -1  # Sentinels
        manifest = json.loads((out_dir / "weights_manifest.json").read_text())
        assert manifest["lm_head"]["hash"] == entry["hash"]
        assert manifest["lm_head"]["shifts_hash"] == entry["shifts_hash"]


def test_quantize_int16_per_channel_roundtrip():
    if not (HAS_NUMPY and HAS_TORCH):
        print("[test] SKIPPED (torch/numpy fehlt): quantize_int16_per_channel")
        return
    from src.quantize import quantize_symmetric_int16_per_channel

    # Zwei Zeilen mit sehr unterschiedlichen Absmax: eigene Skalen je Zeile.
    t = torch.tensor([
        [0.05, -0.05, 0.01, -0.01],   # absmax 0.05 -> hoher Shift
        [2.0, -1.0, 0.5, -0.25],      # absmax 2.0  -> niedriger Shift
    ])
    q = quantize_symmetric_int16_per_channel(t)
    assert q["int16"].dtype == np.int16
    assert q["shifts"].shape == (2,)
    assert q["shifts"][0] > q["shifts"][1], "kleinere Werte brauchen feinere Skalen"

    # Rundlauf: Dequantisierung darf pro Zeile um höchstens einen halben
    # Quantisierungsschritt abweichen.
    for row in range(2):
        step = 2.0 ** (-int(q["shifts"][row]))
        deq = q["int16"][row].astype(np.float64) * step
        err = np.abs(deq - t[row].numpy())
        assert err.max() <= step / 2 + 1e-9, f"Zeile {row}: max. Fehler {err.max()}"


def test_quantize_int8_per_channel_1d_keeps_shape():
    # Regressionstest Fund 11: bei 1D-Tensoren (Bias, Gamma) blies das
    # Broadcasting t[n] * shifts[n,1] das Ergebnis zu einer [n,n]-Matrix
    # auf (q_proj.bias: 896 -> 802816 Elemente), und der Runtime-Loader
    # verweigerte die Modell-Ladung.
    if not (HAS_NUMPY and HAS_TORCH):
        print("[test] SKIPPED (torch/numpy fehlt): quantize_int8_per_channel_1d")
        return
    from src.quantize import quantize_symmetric_int8_per_channel

    t = torch.tensor([0.5, -0.25, 0.0625])
    q = quantize_symmetric_int8_per_channel(t)
    assert q["shape"] == [3], f"1D-Tensor muss 1D bleiben, war {q['shape']}"
    assert q["int8"].shape == (3,)
    assert q["shifts"].shape == (3,)

    # Rundlauf je Element: Fehler höchstens ein halber Quantisierungsschritt.
    for i in range(3):
        step = 2.0 ** (-int(q["shifts"][i]))
        deq = float(q["int8"][i]) * step
        assert abs(deq - float(t[i])) <= step / 2 + 1e-9, f"Element {i}"


def test_30b_config_matches_published_hf_config():
    """
    Die 30B-A3B-Eintraege gegen die veroeffentlichte config.json von
    Qwen/Qwen3-30B-A3B festgenagelt. Der Test haelt fest, WAS geprueft
    wurde, nicht, dass jemand geprueft hat.

    ⛔️ **Hier stand bis zum 2026-09-12 die 14B**, und davor die 7B. Das
    dichte 14B ist auf Festlegung des Projektinhabers entfernt; dieser
    Test steht jetzt auf dem groessten verbliebenen Modell, und das ist
    ein **Gemisch**.

    📌 **Und die Lehre aus dem Wechsel davor bleibt stehen:** Eine
    Ersetzung ueber achtzig Dateien hat damals den Aufruf umgeschrieben
    und die **erwarteten Werte stehen gelassen**. Der Test war rot und
    behauptete dabei etwas ueber ein Modell, das es nicht mehr gab.
    **Eine Ersetzung, die Namen trifft und Zahlen liegen laesst, macht
    aus einem Test eine Behauptung ueber nichts.**

    Was hier verankert ist und beim Gemisch anders liegt als bei einem
    dichten Modell: `num_experts`, `moe_intermediate_size` und
    `num_experts_per_tok`. Die Rechenarbeit bemisst sich an den beiden
    letzten, nicht an `intermediate_size`.
    """
    from src.model_configs import get_export_model_config

    c = get_export_model_config("myelith-30b-a3b")
    erwartet = {
        "num_layers": 48,          # num_hidden_layers
        "hidden_size": 2048,
        "num_heads": 32,           # num_attention_heads
        "num_kv_heads": 4,         # num_key_value_heads
        "vocab_size": 151936,
        "tie_word_embeddings": False,
        "attention_bias": False,   # Qwen3 fuehrt keine q/k/v-Biases
        "qk_norm": True,           # Qwen3 normiert q und k je Kopf
        "head_dim": 128,
    }
    for k, v in erwartet.items():
        assert c[k] == v, f"30B-Feld {k}: {c[k]!r} statt {v!r}"

    # ⚑ **Das Gemisch, und genau hier liegt der Unterschied.**
    assert c["num_experts"] == 128, f"num_experts: {c['num_experts']}"
    assert c.get("num_experts_per_tok") == 8, "Top-8 fehlt"
    assert c.get("moe_intermediate_size") == 768, "die Expertenbreite fehlt"

    # ⚑ head_dim ist bei Qwen3 eine eigene Angabe und **nicht**
    # hidden_size/num_heads: 2048/32 waere 64.
    assert c["hidden_size"] // c["num_heads"] == 64
    assert c["head_dim"] == 128

    # Basis-Variante, nicht Instruct (Scope-Entscheidung 12.15).
    assert c["hf_model_id"] == "Qwen/Qwen3-30B-A3B"
    assert "instruct" not in c["hf_model_id"].lower()


def test_artifact_model_config_omits_provenance_fields():
    """
    model_config.json traegt Modellparameter, keine Herkunftsnachweise.
    "verified"/"hf_model_id" sind fuer Menschen da und haben im vom Loader
    gelesenen Artefakt nichts zu suchen.
    """
    from src.model_configs import artifact_model_config, _REQUIRED_EXPORT_FIELDS

    cfg = artifact_model_config("myelith-30b-a3b")
    assert "verified" not in cfg and "hf_model_id" not in cfg
    # Alles Uebrige, was der Loader braucht, ist noch da.
    for feld in _REQUIRED_EXPORT_FIELDS:
        if feld in ("verified", "hf_model_id"):
            continue
        assert feld in cfg, f"model_config.json fehlt {feld}"


def test_gptq_hessian_bedarf_waechst_quadratisch():
    """
    Der Hessian-Speicher skaliert **quadratisch mit
    `intermediate_size`**, nicht linear mit der Parameterzahl. Genau
    deshalb gibt es den schichtweisen Pfad.

    ⚑ **Geprueft wird das Gesetz und das Kriterium, nicht eine Zahl.**
    Absolute Schranken altern mit jedem Modellwechsel, und dieser Test
    hat es zweimal vorgefuehrt: Er stand auf der 7B, dann auf der 14B,
    und beide sind aus dem Projekt heraus. **Eine Schranke, die an
    einem bestimmten Modell haengt, ist keine Schranke, sondern ein
    Ablaufdatum.**

    Die Aussage, die traegt: Das groesste dichte Modell passt **nicht**
    in den Rahmen, den `gptq_group_size` sich selbst gibt, naemlich
    zwei Drittel des vorhandenen Arbeitsspeichers. Dasselbe Kriterium,
    dasselbe Urteil, gleichgueltig welches Modell gerade das groesste
    ist.
    """
    import os

    from src.model_configs import get_export_model_config
    from src.main import gptq_hessian_bytes, gptq_group_size

    c_klein = get_export_model_config("myelith-0.6b")
    c_gross = get_export_model_config("myelith-4b")
    klein = gptq_hessian_bytes(c_klein)
    gross = gptq_hessian_bytes(c_gross)

    # Das Gesetz: je Ebene waechst der Bedarf mit (6h^2 + i^2).
    def erwartet(c):
        return c["num_layers"] * (6 * c["hidden_size"] ** 2 + c["intermediate_size"] ** 2) * 4

    assert klein == erwartet(c_klein)
    assert gross == erwartet(c_gross)

    # ⚑ **Der Sprung ist nicht die Parameterzahl.** Das groessere Modell
    # hat rund 6,7-mal so viele Parameter wie das kleine und braucht
    # rund elfmal so viel Hessian-Speicher.
    verhaeltnis = gross / klein
    parameter = c_gross["num_layers"] * c_gross["hidden_size"] ** 2
    parameter /= c_klein["num_layers"] * c_klein["hidden_size"] ** 2
    assert verhaeltnis > parameter, (
        f"der Bedarf waechst nicht schneller als die Groesse: "
        f"{verhaeltnis:.1f} gegen {parameter:.1f}"
    )

    # ⚑ **Und das Kriterium, das der Code selbst anlegt.** `0,6B` passt
    # in einer Gruppe, das groesste dichte Modell nicht.
    assert gptq_group_size(c_klein) == c_klein["num_layers"], (
        "0,6B muss in einer Gruppe passen"
    )
    ram = os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")
    if gross > ram * 2 // 3:
        assert gptq_group_size(c_gross) < c_gross["num_layers"], (
            f"{gross / 2**30:.1f} GiB passen nicht in zwei Drittel von "
            f"{ram / 2**30:.1f} GiB, und der Pfad teilt trotzdem nicht"
        )


def test_gptq_entscheidung_env_override():
    """Die Umgebungsvariable schaltet GPTQ **ein**; ohne sie ist es aus.

    ⚑ **Hier stand bis zum 2026-08-25 das Gegenteil**: „ohne Vorgabe
    laeuft GPTQ immer". Am 2026-08-20 hat der Projektinhaber das
    Gegenteil festgelegt, und `gptq_entscheidung` setzt es seitdem um.
    Der Test war seit diesem Tag rot.

    **Dass es fuenf Tage niemandem auffiel, liegt an der CI:** Sie
    startet von den Python-Testdateien nur die vier Audit-Skripte. Eine
    Pruefung, die nicht laeuft, meldet nichts - dieselbe Klasse wie
    Fund 44 und wie `moe.rs`, das nicht in der Audit-Liste stand.

    **Die Begruendung der Festlegung** steht im Docstring von
    `gptq_entscheidung` und ist der Grund, warum der Test der
    Festlegung folgt und nicht umgekehrt: GPTQ ist als
    Ausschlussbeweis dokumentiert, nicht als Verbesserung. Gemessen
    verbesserte es die Perplexitaet **nicht** (3 242 -> 3 318), kostet
    bei 7B aber zweieinhalb Stunden statt zwanzig Minuten.
    """
    import os
    from src.model_configs import get_export_model_config
    from src.main import gptq_entscheidung

    alt = os.environ.get("INTEGER_LLM_GPTQ")
    try:
        os.environ["INTEGER_LLM_GPTQ"] = "0"
        an, _ = gptq_entscheidung(get_export_model_config("myelith-0.6b"))
        assert an is False, "INTEGER_LLM_GPTQ=0 muss GPTQ abschalten"

        os.environ["INTEGER_LLM_GPTQ"] = "1"
        an, grund = gptq_entscheidung(get_export_model_config("myelith-4b"))
        assert an is True, f"INTEGER_LLM_GPTQ=1 muss GPTQ einschalten, Grund: {grund}"

        os.environ.pop("INTEGER_LLM_GPTQ")
        an, grund = gptq_entscheidung(get_export_model_config("myelith-4b"))
        assert an is False, (
            "ohne Vorgabe ist GPTQ aus (Festlegung vom 2026-08-20), "
            f"Grund: {grund}"
        )
        assert "INTEGER_LLM_GPTQ=1" in grund, (
            "die Begruendung muss sagen, womit man es einschaltet: " + grund
        )
    finally:
        os.environ.pop("INTEGER_LLM_GPTQ", None)
        if alt is not None:
            os.environ["INTEGER_LLM_GPTQ"] = alt


def test_gptq_group_size_fits_within_ram_budget():
    """
    Fund 20/21-Nachtrag: schichtweise Hessian-Berechnung statt Abschaltung.
    gptq_group_size() muss so viele Ebenen waehlen, dass ihr gemeinsamer
    Hessian-Bedarf unter zwei Dritteln des (vorgegaukelten) RAM bleibt -
    und fuer 0,6B (das komplett in echten RAM passt) alle Ebenen auf
    einmal waehlen, damit dort weiterhin nur EIN Kalibrier-Durchlauf noetig
    ist.
    """
    from src.model_configs import get_export_model_config
    from src.main import gptq_group_size, gptq_hessian_bytes_per_layer

    cfg_klein = get_export_model_config("myelith-0.6b")
    groesse_klein = gptq_group_size(cfg_klein)
    assert groesse_klein == cfg_klein["num_layers"], (
        f"0,6B muss in einer Gruppe passen, war {groesse_klein} von "
        f"{cfg_klein['num_layers']} Ebenen"
    )

    cfg_gross = get_export_model_config("myelith-4b")
    groesse_gross = gptq_group_size(cfg_gross)
    per_layer = gptq_hessian_bytes_per_layer(cfg_gross)
    assert groesse_gross < cfg_gross["num_layers"], (
        "14B darf auf einer 24-GB-Maschine NICHT in einer Gruppe passen "
        f"(waere wieder der alte 45,5-GB-Sprengsatz), war {groesse_gross}"
    )
    assert groesse_gross >= 1, "Gruppengroesse muss mindestens 1 Ebene sein"
    # Die gewaehlte Gruppe darf den Speicher, den main() ihr zugesteht
    # (zwei Drittel des tatsaechlichen RAM), nicht ueberschreiten.
    import os
    ram = os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")
    assert groesse_gross * per_layer <= ram * 2 // 3


def test_model_name_folgt_umgebungsvariable():
    """
    Ein Wechsel der Modellgroesse darf keine Codeaenderung sein. Der Test
    laedt calibrate.src.main frisch mit gesetzter Variable und prueft, dass
    Name UND HF-ID mitwandern (die ID kommt aus der Config, nicht aus einer
    zweiten Konstante).
    """
    import os
    import importlib

    alt = os.environ.get("INTEGER_LLM_MODEL")
    try:
        os.environ["INTEGER_LLM_MODEL"] = "myelith-30b-a3b"
        main_mod = importlib.reload(importlib.import_module("src.main"))
        assert main_mod.MODEL_NAME == "myelith-30b-a3b"
        assert main_mod.HF_MODEL_ID == "Qwen/Qwen3-30B-A3B"

        os.environ.pop("INTEGER_LLM_MODEL")
        main_mod = importlib.reload(importlib.import_module("src.main"))
        assert main_mod.MODEL_NAME == "myelith-0.6b", "Vorgabe ist der Anker"
        assert main_mod.HF_MODEL_ID == "Qwen/Qwen3-0.6B"
    finally:
        os.environ.pop("INTEGER_LLM_MODEL", None)
        if alt is not None:
            os.environ["INTEGER_LLM_MODEL"] = alt
        importlib.reload(importlib.import_module("src.main"))


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
    test_export_lm_head_writes_files_and_manifest()
    print("[test] export_lm_head schreibt int16/Per-Channel-Artefakte: PASSED")
    test_quantize_int16_per_channel_roundtrip()
    print("[test] int16-Per-Channel-Quantisierung Rundlauf: PASSED")
    test_quantize_int8_per_channel_1d_keeps_shape()
    print("[test] int8-Per-Channel-Quantisierung 1D behaelt Shape: PASSED")
    test_30b_config_matches_published_hf_config()
    print("[test] 30B-Config stimmt mit veroeffentlichter HF-config.json: PASSED")
    test_artifact_model_config_omits_provenance_fields()
    print("[test] model_config.json ohne Herkunftsfelder: PASSED")
    test_gptq_hessian_bedarf_waechst_quadratisch()
    print("[test] GPTQ-Hessian-Bedarf waechst quadratisch: PASSED")
    test_gptq_entscheidung_env_override()
    print("[test] GPTQ-Entscheidung folgt der Umgebungsvariable: PASSED")
    test_gptq_group_size_fits_within_ram_budget()
    print("[test] GPTQ-Gruppengroesse passt in den RAM-Rahmen: PASSED")
    test_model_name_folgt_umgebungsvariable()
    print("[test] Modellwahl per INTEGER_LLM_MODEL: PASSED")
    print("[test] Alle Tests bestanden.")
