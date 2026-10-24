"""
**Der ganzzahlige Sichtturm gegen die Gleitkomma-Referenz**, Stufe fuer Stufe.

Nimmt die Referenzen aus `iris_referenz` (`calibrate/lokal/iris/<modell>/*.npz`),
rechnet dieselben Flicken mit `iris_probe` ganzzahlig und misst je Stufe
den relativen L2-Fehler und die Kosinusaehnlichkeit, gemittelt ueber die
Flicken (beim Projektor ueber die Bildtoken).

Aufruf aus `INTEGER_LLM`, nach einem Release-Bau von `iris_probe`:

    calibrate/.venv/bin/python3 -m calibrate.src.iris_vergleich \\
        --artefakt raven-35b-a3b-int4 [--modell Qwen3.6-35B-A3B]
"""

import argparse
import subprocess
import tempfile
from pathlib import Path

import numpy as np

from .paths import model_artifacts_dir, repo_root
from . import iris_referenz as ref


def probe_pfad() -> Path:
    return repo_root() / "SYSTEM" / "full-build" / "release" / "iris_probe"


def flicken768(npz) -> np.ndarray:
    """Die Flicken ohne die doppelte Zeitscheibe: (N, 3, 16, 16) flach."""
    f = npz["flicken"].astype(np.int16)
    n = f.shape[0]
    return f.reshape(n, 3, 2, 16, 16)[:, :, 0].reshape(n, 768)


def lesen(ordner: Path, stufe: str) -> np.ndarray:
    frac = np.fromfile(ordner / f"{stufe}_frac.bin", dtype=np.uint8)
    werte = np.fromfile(ordner / f"{stufe}.bin", dtype="<i2").reshape(-1, frac.shape[0])
    return werte.astype(np.float64) / (2.0 ** frac.astype(np.float64))


def masse(ist: np.ndarray, soll: np.ndarray):
    soll = soll.astype(np.float64)
    rel = np.linalg.norm(ist - soll) / max(np.linalg.norm(soll), 1e-30)
    kos = (ist * soll).sum(1) / np.maximum(np.linalg.norm(ist, axis=1) * np.linalg.norm(soll, axis=1), 1e-30)
    return rel, float(kos.mean()), float(kos.min())


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--artefakt", required=True)
    ap.add_argument("--modell", default="Qwen3.6-35B-A3B")
    ap.add_argument("--innen", action="store_true", help="auch das Innere einzelner Bloecke")
    ap.add_argument("--alle", action="store_true", help="jeden Block zeigen")
    a = ap.parse_args()

    artefakt = model_artifacts_dir(a.artefakt).resolve()
    referenzen = sorted((ref.lokal_dir() / a.modell).glob("*.npz"))
    if not referenzen:
        raise SystemExit("keine Referenzen; erst iris_referenz laufen lassen")
    for r in referenzen:
        d = np.load(r)
        _, gh, gw = (int(v) for v in d["grid_thw"][0])
        with tempfile.TemporaryDirectory() as t:
            t = Path(t)
            flicken768(d).astype("<i2").tofile(t / "flicken.bin")
            lauf = subprocess.run(
                [str(probe_pfad()), str(artefakt), str(t / "flicken.bin"), str(gh), str(gw), str(t / "aus")],
                capture_output=True, text=True, check=True,
            )
            print(f"== {r.stem}: {lauf.stdout.strip()}")
            print(f"   {'Stufe':14s} {'rel. L2':>9s} {'Kos. Mittel':>12s} {'Kos. min':>9s}")
            innen = ["n1", "qkv", "achtsam", "attn", "rest_mitte", "n2", "fc1", "gelu", "mlp"]
            stufen = ["vor_block_00"]
            for i in range(27):
                if a.innen and i in ref.INNERE_BLOECKE:
                    stufen += [f"b{i:02d}.{x}" for x in innen]
                stufen.append(f"block_{i:02d}")
            stufen += ["p.n", "p.fc1", "p.gelu", "projektor"]
            for s in stufen:
                if s not in d:
                    continue
                rel, kmit, kmin = masse(lesen(t / "aus", s), d[s])
                if a.alle or ("." in s and (a.innen or s.startswith("p."))) or s in ("vor_block_00", "projektor") or s.endswith(("00", "05", "10", "15", "20", "25", "26")):
                    print(f"   {s:14s} {rel:9.4f} {kmit:12.5f} {kmin:9.5f}")
    print("Fertig")


if __name__ == "__main__":
    main()
