"""
Exportiert alle theta_v-Artefakte: Skalen, LUTs, theta_v-Manifest mit Hashes.

Gewichte werden NICHT hier exportiert (siehe export_weights.py) - export_theta_v()
muss deshalb aufgerufen werden, NACHDEM export_quantized_weights() bereits
weights_manifest.json in denselben output_dir geschrieben hat, da theta_v.json
diese Datei hasht. Siehe main.py fuer die verbindliche Reihenfolge.
"""

import json
import hashlib
import struct
from pathlib import Path

# calibrate/src/export.py -> calibrate/src -> calibrate -> INTEGER_LLM (Repo-Wurzel)
_REPO_ROOT = Path(__file__).parent.parent.parent


def export_json(obj: dict, path: Path):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f, sort_keys=True, separators=(",", ":"))


def export_binary(data: bytes, path: Path):
    with open(path, "wb") as f:
        f.write(data)


def hash_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read())
    return h.hexdigest()


def spec_version() -> str:
    """
    Liest die theta_v-Version aus theta_v/spec.json - derselben Datei, die
    runtime/src/loader.rs zur Kompilierzeit per include_str! einbettet und
    gegen die ThetaV::verify_version_against_spec() prueft (Punkt
    12.13). Ein hier exportiertes Artefakt muss dieselbe Version tragen wie
    die spec.json, gegen die der Loader zum Zeitpunkt des Exports gebaut ist,
    sonst schlaegt das Laden mit einem klaren Versions-Mismatch fehl.
    """
    spec_path = _REPO_ROOT / "theta_v" / "spec.json"
    spec = json.loads(spec_path.read_text(encoding="utf-8"))
    return spec["theta_v"]["version"]


def export_theta_v(scales, luts, output_dir: Path):
    output_dir.mkdir(parents=True, exist_ok=True)

    scales_path = output_dir / "scales.json"
    export_json(scales, scales_path)

    luts_meta = {}
    for name, table in luts.items():
        bin_path = output_dir / f"{name}.lut.bin"
        payload = struct.pack(f"<{len(table)}h", *table)
        export_binary(payload, bin_path)
        luts_meta[name] = {
            "file": str(bin_path.name),
            "hash": hash_file(bin_path),
            "length": len(table),
            "dtype": "int16",
        }

    luts_path = output_dir / "luts.json"
    export_json(luts_meta, luts_path)

    weights_manifest_path = output_dir / "weights_manifest.json"
    if not weights_manifest_path.exists():
        raise FileNotFoundError(
            f"{weights_manifest_path} fehlt. export_theta_v() muss NACH "
            "export_quantized_weights() aufgerufen werden (siehe main.py) - "
            "theta_v.json hasht das echte Gewichts-Manifest, nicht einen Platzhalter."
        )

    manifest = {
        "version": spec_version(),
        "weights_hash": hash_file(weights_manifest_path),
        "scales_hash": hash_file(scales_path),
        "luts_hash": hash_file(luts_path),
    }
    manifest_path = output_dir / "theta_v.json"
    export_json(manifest, manifest_path)

    print(f"[export] theta_v Artefakte geschrieben nach {output_dir}")
    print(f"[export] Manifest-Hash: {hash_file(manifest_path)}")
