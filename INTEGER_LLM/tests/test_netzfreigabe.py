"""Die Netzliste: der Abdruck hat dieselbe Form wie in der Laufzeit.

Laeuft ohne Abhaengigkeiten:  python3 INTEGER_LLM/tests/test_netzfreigabe.py
"""

import importlib.util
import json
import sys
import tempfile
from pathlib import Path

WURZEL = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("netzfreigabe", WURZEL / "INTEGER_LLM" / "tools" / "netzfreigabe.py")
nf = importlib.util.module_from_spec(spec)
spec.loader.exec_module(nf)


def artefakt(ort: Path, konfig: bytes, tokenizer: bool = True) -> Path:
    ort.mkdir(parents=True, exist_ok=True)
    (ort / "theta_v.json").write_bytes(b'{"version":"0.22.0"}')
    (ort / "model_config.json").write_bytes(konfig)
    if tokenizer:
        (ort / "tokenizer.json").write_bytes(b"{}")
    return ort


def test_dieselbe_form_wie_die_laufzeit():
    # ⚑ Derselbe Wert steht in `der_abdruck_hat_eine_feste_form`
    #   (INTEGER_LLM/runtime/src/netzfreigabe.rs).
    with tempfile.TemporaryDirectory() as t:
        a = artefakt(Path(t), b'{"family":"qwen3"}')
        assert nf.abdruck(a) == "5ed8ae39e54827172da0389bea0164058bedfb3ca77de633d6ad1c9c239d6e7e", nf.abdruck(a)


def test_eine_geaenderte_konfiguration_aendert_den_abdruck():
    with tempfile.TemporaryDirectory() as t:
        a = artefakt(Path(t), b'{"betrieb":"lokal"}')
        vorher = nf.abdruck(a)
        (a / "model_config.json").write_bytes(b'{"betrieb":"netz"}')
        assert nf.abdruck(a) != vorher


def test_der_katalog_bleibt_beim_schreiben_wie_er_ist():
    text = nf.KATALOG.read_text(encoding="utf-8")
    assert json.dumps(json.loads(text), ensure_ascii=False, indent=2) + "\n" == text


def test_kein_privates_modell_ist_freigegeben():
    k = nf.katalog_lesen()
    for n, e in nf.eintraege(k).items():
        for block in nf.BLOECKE:
            if block in e:
                assert e.get("betrieb") != "lokal", n
                assert len(e[block]["abdruck"]) == 64, n


def test_im_netz_rechnet_nur_das_35b():
    # ⚑ Festlegung des Projektinhabers (2026-10-21): das 35B als untere
    #   Grenze der Leistungsfaehigkeit; kleine Modelle nur im Pruefnetz.
    netz = [n for n, e in nf.eintraege(nf.katalog_lesen()).items() if "netz" in e]
    assert netz and all(n.startswith("raven-35b-a3b-") for n in netz), netz


if __name__ == "__main__":
    fehler = 0
    for name, f in sorted(globals().items()):
        if name.startswith("test_") and callable(f):
            try:
                f()
                print(f"ok    {name}")
            except AssertionError as a:
                fehler += 1
                print(f"FEHLER {name}: {a}")
    print("PASSED" if not fehler else f"FAILED ({fehler})")
    sys.exit(1 if fehler else 0)
