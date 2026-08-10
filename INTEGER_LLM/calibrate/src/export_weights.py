"""
Exportiert quantisierte Gewichte als raw binary + JSON-Metadaten.
"""

import json
import struct
import numpy as np
from pathlib import Path
from typing import Dict


def export_quantized_weights(quantized: Dict[str, dict], output_dir: Path):
    """
    Exportiert jeden Tensor als eigenes .bin File.
    Format: raw int8 bytes, row-major, little-endian.
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest = {}

    for name, meta in quantized.items():
        safe_name = name.replace(".", "_")
        bin_path = output_dir / f"{safe_name}.bin"

        # Numpy array als raw bytes (C-contiguous)
        data = meta["int8"]
        raw = data.tobytes()
        with open(bin_path, "wb") as f:
            f.write(raw)

        manifest[safe_name] = {
            "original_name": name,
            "file": str(bin_path.name),
            "shape": meta["shape"],
            "scale": meta["scale"],
            "shift": meta["shift"],
            "dtype": "int8",
            "hash": hash_file(bin_path),
        }

    # Manifest schreiben
    manifest_path = output_dir / "weights_manifest.json"
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, sort_keys=True, separators=(",", ":"))

    print(f"[export_weights] {len(quantized)} Tensoren exportiert nach {output_dir}")
    return manifest


def hash_file(path: Path) -> str:
    import hashlib
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read())
    return h.hexdigest()
