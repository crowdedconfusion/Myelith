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
    divergieren (Akzeptanzkriterium Punkt 12.15: alle .bin-Dateien haben
    korrekte SHA-256-Eintraege im Manifest):
    - dtype muss int8 sein (falls das Array einen dtype traegt),
    - Byte-Laenge muss exakt dem Produkt der shape entsprechen,
    - Anzahl Shifts muss der Zeilenzahl (shape[0]) entsprechen,
    - nach dem Schreiben wird jede Datei neu gehasht und gegen den
      Manifest-Eintrag verifiziert.
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest = {}

    # `quantized` darf ein Woerterbuch **oder** ein Strom von Paaren sein.
    # Der Strom haelt nie mehr als einen Tensor und ist der Grund, warum
    # Qwen3-30B-A3B ueberhaupt exportierbar ist; das Woerterbuch bleibt
    # fuer die Tests und die kleinen Modelle. Die geschriebenen Bytes sind
    # in beiden Faellen dieselben, und das Manifest wird ohnehin sortiert.
    eintraege = quantized.items() if hasattr(quantized, "items") else quantized

    for name, meta in eintraege:
        safe_name = name.replace(".", "_")
        bin_path = output_dir / f"{safe_name}.bin"
        shifts_path = output_dir / f"{safe_name}_shifts.bin"

        # Fund 23 (theta_v 0.13.0): Biases liegen in int16, alles andere
        # in int8. Biases saettigten zuvor still bei Betraegen ueber 127
        # (Qwen2.5-7B, k_proj.bias: 414 -> 127) und verfaelschten die
        # Attention ab Ebene 0. Sie sind winzig (1D), int16 kostet daher
        # kaum Platz.
        ist_int16 = "int16" in meta
        data = meta["int16"] if ist_int16 else meta["int8"]
        shifts = meta["shifts"]
        erwarteter_dtype = "int16" if ist_int16 else "int8"
        bytes_je_wert = 2 if ist_int16 else 1

        dtype = getattr(data, "dtype", None)
        if dtype is not None and str(dtype) != erwarteter_dtype:
            raise ValueError(
                f"Tensor '{name}': erwartet {erwarteter_dtype}, bekommen '{dtype}'. "
                "Das verletzt das theta_v-Binaerformat (raw, row-major)."
            )

        raw = data.tobytes()

        expected_bytes = bytes_je_wert
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
            "dtype": erwarteter_dtype,
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

    # `len(manifest)` statt `len(quantized)`: Ein Strom ist an dieser
    # Stelle bereits verbraucht und hat ohnehin keine Laenge.
    print(f"[export_weights] {len(manifest)} Tensoren exportiert nach {output_dir}")
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


TERNAER_DTYPE = "ternaer_g128"


def ternaer_muster(codes: np.ndarray) -> np.ndarray:
    """Codes `uint8 [zeilen, n]` (0 bis 3, `c = t + 1`) in das Format der
    Laufzeit: je Gruppe von 128 zwei Bloecke zu 16 Byte, Byte `j` eines
    Blocks traegt in den Bits `2s` den Code des Gewichts `16s + j`.

    ⚑ **Dieselbe Festlegung wie `integer_llm_kernels::ternaer`**; die Probe
    `test_ternaer_export.py` vergleicht beide Byte fuer Byte.
    """
    z, n = codes.shape
    if n % 128:
        raise ValueError(f"ternaer: {n} Spalten sind kein Vielfaches von 128")
    c = codes.reshape(z, n // 128, 2, 4, 16).astype(np.uint8)
    byte = c[:, :, :, 0, :] | (c[:, :, :, 1, :] << 2) | (c[:, :, :, 2, :] << 4) | (c[:, :, :, 3, :] << 6)
    return np.ascontiguousarray(byte).reshape(z, n // 4)


def ternaer_betraege(skalen: np.ndarray):
    """FP16-Skalen `[zeilen, gruppen]` exakt als `i16`-Betrag mal
    Zweierpotenz je Zeile: `s = M / 2^shift`.

    ⚑ **Exakt oder gar nicht.** Jede FP16-Zahl ist `M * 2^E` mit hoechstens
    elf Bit `M`. Je Zeile wird der kleinste gemeinsame Exponent gesucht; passt
    ein Betrag danach nicht in `i16` oder der Shift nicht in 0 bis 127, ist
    das ein Fehler und keine Rundung (gemessen am Bonsai-27B: Spanne der
    Exponenten je Zeile hoechstens 3, also hoechstens 2047 * 8).
    """
    s = skalen.astype(np.float64)
    if np.any(s < 0) or not np.all(np.isfinite(s)):
        raise ValueError("ternaer: Skalen muessen endlich und nicht negativ sein")
    m, e = np.frexp(s)
    M = m * 2048.0
    if not np.all(M == np.floor(M)):
        raise ValueError("ternaer: eine Skala hat mehr als elf Bit Mantisse")
    M = M.astype(np.int64)
    k = 11 - e.astype(np.int64)            # s = M / 2^k
    # Nachlaufende Nullen der Mantisse abstreifen, damit k klein bleibt.
    for _ in range(11):
        gerade = (M != 0) & ((M & 1) == 0)
        M = np.where(gerade, M >> 1, M)
        k = np.where(gerade, k - 1, k)
    k = np.where(M == 0, np.iinfo(np.int64).min, k)
    shift = k.max(axis=1)
    shift = np.where(shift == np.iinfo(np.int64).min, 0, shift)
    if np.any(shift < 0) or np.any(shift > 127):
        raise ValueError(f"ternaer: Zeilenshift ausserhalb 0 bis 127 ({shift.min()} bis {shift.max()})")
    betrag = np.where(M == 0, 0, M << np.maximum(shift[:, None] - k, 0))
    if np.any(betrag > 32767):
        raise ValueError(f"ternaer: ein Betrag passt nicht in i16 ({betrag.max()})")
    # Gegenprobe: exakt dieselbe Zahl
    if not np.array_equal(betrag.astype(np.float64) / np.exp2(shift[:, None].astype(np.float64)), s):
        raise ValueError("ternaer: die Zerlegung der Skalen ist nicht exakt")
    return betrag.astype("<i2"), shift.astype(np.int8)


def export_ternaere_gewichte(model, output_dir: Path) -> int:
    """Exportiert jede gepackte, gedrehte Linearschicht eines Pakets
    (`gedrehtes_paket.GepackteLinear`) ternaer, samt Vorzeichendateien, und
    ergaenzt das Manifest. Muss nach `export_quantized_weights` laufen.

    Der Kopf bekommt zwei Eintraege, `lm_head` (der Kopf) und
    `lm_head_weight` (der Rueckfallweg der Laufzeit), auf dieselben Dateien.
    """
    from .gedrehtes_paket import GepackteLinear, codes_entpacken
    from .quantize import normiere_namen

    manifest_path = output_dir / "weights_manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    vorzeichen_dateien = {}
    anzahl = 0
    for name, modul in model.named_modules():
        if not isinstance(modul, GepackteLinear):
            continue
        name = normiere_namen(name)
        safe = (name + ".weight").replace(".", "_")
        zeilen = modul.out_features
        muster_path = output_dir / f"{safe}.muster.bin"
        betraege_path = output_dir / f"{safe}.betraege.bin"
        shifts_path = output_dir / f"{safe}_shifts.bin"
        drei = 0
        with open(muster_path, "wb") as fm, open(betraege_path, "wb") as fb:
            shifts_alle = []
            for a in range(0, zeilen, 8192):
                e = min(a + 8192, zeilen)
                codes = codes_entpacken(modul.woerter[a:e]).numpy()
                drei += int((codes == 3).sum())
                fm.write(ternaer_muster(codes).tobytes())
                betrag, shift = ternaer_betraege(modul.skalen[a:e].numpy())
                fb.write(betrag.tobytes())
                shifts_alle.append(shift)
        if drei:
            raise ValueError(f"{name}: {drei} Gewichte mit Code 3, das Paket ist nicht ternaer")
        shifts_path.write_bytes(np.concatenate(shifts_alle).tobytes())

        vz = modul.drehung.vorzeichen.numpy()
        breite = vz.shape[0]
        if breite not in vorzeichen_dateien:
            vpfad = output_dir / f"drehung_vorzeichen_{breite}.bin"
            vpfad.write_bytes(vz.astype(np.int8).tobytes())
            vorzeichen_dateien[breite] = vpfad
        elif not np.array_equal(np.frombuffer(vorzeichen_dateien[breite].read_bytes(), dtype=np.int8), vz.astype(np.int8)):
            raise ValueError(f"{name}: andere Vorzeichen fuer Breite {breite} als eine fruehere Schicht")
        vpfad = vorzeichen_dateien[breite]

        eintrag = {
            "original_name": name + ".weight",
            "file": muster_path.name,
            "hash": hash_file(muster_path),
            "betraege_file": betraege_path.name,
            "betraege_hash": hash_file(betraege_path),
            "shifts_file": shifts_path.name,
            "shifts_hash": hash_file(shifts_path),
            "shape": [zeilen, modul.in_features],
            "scale": -1.0,
            "shift": -1,
            "dtype": TERNAER_DTYPE,
            "drehung_file": vpfad.name,
            "drehung_hash": hash_file(vpfad),
        }
        if name == "lm_head":
            manifest["lm_head"] = dict(eintrag)
            manifest["lm_head_weight"] = dict(eintrag)
        else:
            manifest[safe] = eintrag
        anzahl += 1
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, sort_keys=True, separators=(",", ":"))
    print(f"[export_weights] {anzahl} ternaere Tensoren exportiert, "
          f"Vorzeichen fuer die Breiten {sorted(vorzeichen_dateien)}")
    return anzahl


def hash_file(path: Path) -> str:
    import hashlib
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read())
    return h.hexdigest()
