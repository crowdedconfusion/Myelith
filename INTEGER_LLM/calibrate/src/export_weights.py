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
    Exportiert jeden Tensor als eigenes .bin File (theta_v 0.7.0:
    per-channel INT8 mit eigener Zweierpotenz-Skala je Ausgabe-Zeile).
    Format: raw int8 bytes, row-major, little-endian; die Zeilen-Shifts
    liegen in einer eigenen Datei `<name>_shifts.bin` (raw int8).

    Prüfungen vor und nach dem Schreiben, damit Manifest und Dateien nie
    divergieren (Akzeptanzkriterium Fahrplan 12.15: alle .bin-Dateien haben
    korrekte SHA-256-Eintraege im Manifest):
    - dtype muss int8 sein (falls das Array einen dtype traegt),
    - Byte-Laenge muss exakt dem Produkt der shape entsprechen,
    - Anzahl Shifts muss der Zeilenzahl (shape[0]) entsprechen,
    - nach dem Schreiben wird jede Datei neu gehasht und gegen den
      Manifest-Eintrag verifiziert.
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest = {}

    for name, meta in quantized.items():
        safe_name = name.replace(".", "_")
        bin_path = output_dir / f"{safe_name}.bin"
        shifts_path = output_dir / f"{safe_name}_shifts.bin"

        data = meta["int8"]
        shifts = meta["shifts"]

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
        if shifts.shape[0] != meta["shape"][0]:
            raise ValueError(
                f"Tensor '{name}': {shifts.shape[0]} Shifts, aber "
                f"{meta['shape'][0]} Zeilen erwartet."
            )

        with open(bin_path, "wb") as f:
            f.write(raw)
        shifts_path.write_bytes(np.ascontiguousarray(shifts).astype("int8").tobytes())

        manifest[safe_name] = {
            "original_name": name,
            "file": str(bin_path.name),
            "shape": meta["shape"],
            "scale": -1.0,  # Sentinel: Per-Channel-Skalen in shifts_file
            "shift": -1,    # Sentinel: Per-Channel-Shifts in shifts_file
            "dtype": "int8",
            "shifts_file": str(shifts_path.name),
            "hash": hash_file(bin_path),
            "shifts_hash": hash_file(shifts_path),
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
        actual_shifts = hash_file(output_dir / entry["shifts_file"])
        if actual_shifts != entry["shifts_hash"]:
            raise IOError(
                f"SHA-256-Verifikation der Shifts fehlgeschlagen fuer "
                f"{entry['shifts_file']} (Tensor '{safe_name}')."
            )

    print(f"[export_weights] {len(quantized)} Tensoren exportiert nach {output_dir}")
    return manifest


def export_lm_head(lm_head_quant: dict, output_dir: Path) -> dict:
    """
    Exportiert den LM-Head als INT16 mit Per-Channel-Skalen (benannte
    spec-Ausnahme theta_v 0.6.0, Eskalation nach Entscheidungspunkt 12.21).

    Schreibt `lm_head.bin` (raw int16, row-major, little-endian) und
    `lm_head_shifts.bin` (raw int8, ein Shift je Zeile) und ergänzt den
    zugehörigen Eintrag in der bestehenden weights_manifest.json (muss nach
    export_quantized_weights aufgerufen werden). Nachschreiben-Verifikation
    wie bei den übrigen Gewichten.
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    data = lm_head_quant["int16"]
    shifts = lm_head_quant["shifts"]

    if str(getattr(data, "dtype", None)) != "int16":
        raise ValueError("LM-Head: erwartet int16-Daten")
    if data.shape[0] != shifts.shape[0]:
        raise ValueError(
            f"LM-Head: {data.shape[0]} Zeilen, aber {shifts.shape[0]} Shifts"
        )

    bin_path = output_dir / "lm_head.bin"
    shifts_path = output_dir / "lm_head_shifts.bin"
    raw = np.ascontiguousarray(data).astype("<i2").tobytes()
    expected_bytes = data.shape[0] * data.shape[1] * 2
    if len(raw) != expected_bytes:
        raise ValueError(
            f"LM-Head: {len(raw)} Bytes passen nicht zur shape {list(data.shape)}"
        )
    bin_path.write_bytes(raw)
    shifts_path.write_bytes(np.ascontiguousarray(shifts).astype("int8").tobytes())

    entry = {
        "original_name": "lm_head.weight",
        "file": bin_path.name,
        "shape": list(data.shape),
        "scale": -1.0,  # Sentinel: Per-Channel-Skalen in shifts_file
        "shift": -1,    # Sentinel: Per-Channel-Shifts in shifts_file
        "dtype": "int16",
        "shifts_file": shifts_path.name,
        "hash": hash_file(bin_path),
        "shifts_hash": hash_file(shifts_path),
    }

    manifest_path = output_dir / "weights_manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["lm_head"] = entry
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, sort_keys=True, separators=(",", ":"))

    if hash_file(bin_path) != entry["hash"] or hash_file(shifts_path) != entry["shifts_hash"]:
        raise IOError("SHA-256-Verifikation des LM-Heads nach dem Schreiben fehlgeschlagen")

    print(
        f"[export_weights] LM-Head exportiert (int16, per-channel, "
        f"{data.shape[0]} Zeilen) nach {output_dir}"
    )
    return entry


def hash_file(path: Path) -> str:
    import hashlib
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read())
    return h.hexdigest()
