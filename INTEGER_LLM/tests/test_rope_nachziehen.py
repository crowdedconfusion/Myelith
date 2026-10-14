"""
Proben fuer `rope_nachziehen.py`: die Drehtabellen eines vorhandenen
Artefakts auf seine volle Kontextlaenge, ohne Gewichte anzufassen.

Laeuft ohne torch und ohne Artefakt; ein kleines Schein-Artefakt im
Zwischenordner genuegt.

    python3 INTEGER_LLM/tests/test_rope_nachziehen.py
"""

import hashlib
import json
import os
import struct
import sys
import tempfile
from pathlib import Path

# Ohne Bytecode: Python legte sonst `__pycache__` in `calibrate/src`.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate" / "src"))
from export import export_json, hash_file  # noqa: E402
from luts import generate_rope_luts  # noqa: E402
import rope_nachziehen  # noqa: E402

PAARE = 4


def schein_artefakt(ordner: Path, zeilen: int, basis_tabelle: float, basis_modell: float, max_context: int = 64):
    """Ein Artefakt mit Tabellen fuer `zeilen` Positionen, gerechnet mit
    `basis_tabelle`; das Modell selbst nennt `basis_modell`."""
    ordner.mkdir(parents=True)
    export_json({"head_dim": 2 * PAARE, "max_context": max_context, "rope_theta": basis_modell}, ordner / "model_config.json")
    sin, cos = generate_rope_luts(max_seq_len=zeilen, head_dim=2 * PAARE, rope_theta=basis_tabelle, frac_bits=8)
    luts = {}
    for name, tab in (("sin", sin), ("cos", cos)):
        (ordner / f"{name}.lut.bin").write_bytes(struct.pack(f"<{len(tab)}h", *tab))
        luts[name] = {"dtype": "int16", "file": f"{name}.lut.bin", "hash": hash_file(ordner / f"{name}.lut.bin"), "length": len(tab)}
    export_json(luts, ordner / "luts.json")
    export_json({"luts_hash": hash_file(ordner / "luts.json"), "version": "x", "weights_hash": "w", "scales_hash": "s"}, ordner / "theta_v.json")


def frac_bits_der_spezifikation():
    from luts import load_nonlinear_spec
    return load_nonlinear_spec()["rope"]["frac_bits"]


def test_die_tabelle_waechst_und_ihr_anfang_bleibt():
    assert frac_bits_der_spezifikation() == 8, "die Probe nimmt 8 Nachkommabits an wie theta_v"
    with tempfile.TemporaryDirectory() as d:
        a = Path(d) / "artefakt"
        schein_artefakt(a, 16, 1e6, 1e6)
        alt_sin = (a / "sin.lut.bin").read_bytes()
        # ⚑ Ein harter Verweis wie zwischen int8- und ternaerem 35B bleibt alt.
        geteilt = {n: Path(d) / f"geteilt.{n}" for n in ("cos.lut.bin", "luts.json", "theta_v.json")}
        for n, p in geteilt.items():
            os.link(a / n, p)
        alt_geteilt = {n: p.read_bytes() for n, p in geteilt.items()}
        b = rope_nachziehen.nachziehen(a)
        assert (b["alt"], b["neu"], b["geschrieben"]) == (16, 64, True), b
        neu_sin = (a / "sin.lut.bin").read_bytes()
        assert len(neu_sin) == 64 * PAARE * 2 and neu_sin[: len(alt_sin)] == alt_sin, "der Anfang ist nicht bytegleich"
        for n, p in geteilt.items():
            assert p.read_bytes() == alt_geteilt[n], f"die geteilte {n} wurde mitgeaendert"
        luts = json.loads((a / "luts.json").read_text())
        assert luts["sin"]["length"] == 64 * PAARE
        assert luts["sin"]["hash"] == hashlib.sha256(neu_sin).hexdigest()
        assert json.loads((a / "theta_v.json").read_text())["luts_hash"] == hash_file(a / "luts.json"), "theta_v nennt das alte luts.json"
        assert rope_nachziehen.nachziehen(a)["geschrieben"] is False, "ein zweiter Lauf hat nichts zu tun"


def test_eine_abweichende_tabelle_bleibt_ohne_berichtigen():
    with tempfile.TemporaryDirectory() as d:
        a = Path(d) / "artefakt"
        schein_artefakt(a, 16, 1e6, 1e7)
        vorher = {n: (a / n).read_bytes() for n in ("sin.lut.bin", "cos.lut.bin", "luts.json", "theta_v.json")}
        try:
            rope_nachziehen.nachziehen(a)
            raise AssertionError("eine mit anderer Basis gebaute Tabelle wurde verlaengert")
        except ValueError as f:
            assert "anderen Massen" in str(f), f
        assert all((a / n).read_bytes() == v for n, v in vorher.items()), "trotz Abbruch geschrieben"
        b = rope_nachziehen.nachziehen(a, berichtigen=True)
        assert b["geschrieben"] and b["berichtigt"]["zeile"] >= 1, b
        assert 5e5 < b["berichtigt"]["alte_basis"] < 2e6, f"alte Basis {b['berichtigt']['alte_basis']} nicht als 1e6 erkannt"
        sin, _ = generate_rope_luts(max_seq_len=64, head_dim=2 * PAARE, rope_theta=1e7, frac_bits=8)
        assert (a / "sin.lut.bin").read_bytes() == struct.pack(f"<{len(sin)}h", *sin), "berichtigt, aber nicht mit der Basis des Modells"


def test_nicht_ueber_das_modell_hinaus():
    with tempfile.TemporaryDirectory() as d:
        a = Path(d) / "artefakt"
        schein_artefakt(a, 16, 1e6, 1e6)
        try:
            rope_nachziehen.nachziehen(a, zeilen=128)
            raise AssertionError("mehr Zeilen, als das Modell Positionen kann")
        except ValueError:
            pass


if __name__ == "__main__":
    test_die_tabelle_waechst_und_ihr_anfang_bleibt()
    print("[test] die Tabelle waechst, ihr Anfang bleibt bytegleich, Hashes nachgezogen: PASSED")
    test_eine_abweichende_tabelle_bleibt_ohne_berichtigen()
    print("[test] eine abweichende Tabelle bleibt, mit --berichtigen die Basis des Modells: PASSED")
    test_nicht_ueber_das_modell_hinaus()
    print("[test] nicht ueber max_context hinaus: PASSED")
    print("[test] Alle Tests bestanden.")
