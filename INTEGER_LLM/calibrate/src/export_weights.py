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

    Prüfungen vor und nach dem Schreiben, damit Manifest und Dateien nie
    divergieren (Akzeptanzkriterium Fahrplan 12.15: alle .bin-Dateien haben
    korrekte SHA-256-Eintraege im Manifest):
    - dtype muss int8 sein (falls das Array einen dtype traegt),
    - Byte-Laenge muss exakt dem Produkt der shape entsprechen,
    - nach dem Schreiben wird jede Datei neu gehasht und gegen den
      Manifest-Eintrag verifiziert.
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest = {}

    for name, meta in quantized.items():
        safe_name = name.replace(".", "_")
        bin_path = output_dir / f"{safe_name}.bin"

        data = meta["int8"]

        dtype = getattr(data, "dtype", None)
        if dtype is not None and str(dtype) != "int8":
            raise ValueError(
                f"Tensor '{name}': erwartet int8, bekommen '{dtype}'. "
                "Nur int8 darf exportiert werden — alles andere verletzt "
                "das theta_v-Binaerformat (raw int8, row-major)."
            )

        raw = data.tobytes()

        expected_bytes = 1
        for dim in meta["shape"]:
            expected_bytes *= dim
        if len(raw) != expected_bytes:
            raise ValueError(
                f"Tensor '{name}': {len(raw)} Bytes passen nicht zur shape "
                f"{meta['shape']} ({expected_bytes} Bytes erwartet). "
                "Manifest und .bin-Datei wuerden divergieren."
            )

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

    # Nachschreiben-Verifikation: jede Datei erneut hashen und mit dem
    # Manifest vergleichen (faengt Schreib-/Dateisystemfehler ab, bevor
    # theta_v.json die Hashes uebernimmt).
    for safe_name, entry in manifest.items():
        actual = hash_file(output_dir / entry["file"])
        if actual != entry["hash"]:
            raise IOError(
                f"SHA-256-Verifikation nach dem Schreiben fehlgeschlagen fuer "
                f"{entry['file']} (Tensor '{safe_name}'): Manifest sagt "
                f"{entry['hash']}, Datei ist {actual}."
            )

    print(f"[export_weights] {len(quantized)} Tensoren exportiert nach {output_dir}")
    return manifest


def hash_file(path: Path) -> str:
    import hashlib
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read())
    return h.hexdigest()
