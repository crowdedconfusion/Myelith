"""
Ternaere Experten mit datenabhaengiger Rundung (GPTQ-Fehlerkompensation).

Bloesses Runden eines Gewichts auf {-a, 0, +a} minimiert den Fehler je
Eintrag; fuer das Modell zaehlt der Fehler der AUSGABE, ||X W^T - X Q^T||^2
auf den Eingaben, die ein Experte wirklich sieht. GPTQ (Frantar et al. 2022)
verteilt den Rundungsfehler jeder Spalte ueber die Hessesche Matrix
H = X^T X auf die noch nicht gerundeten Spalten. Fuer Expertengemische hat
dasselbe Verfahren ternaere Experten mit geringem Verlust getragen (QMoE,
Frantar und Alistarh 2023), mit zwei Anpassungen, die hier gelten:
10 % Daempfung statt 1 %, und je Experte nur die Token, die der Router ihm
zuteilt.

Das Gitter ist das des ternaeren Formats und des Trainings: je Zeile und
Gruppe von 128 Eingaengen {-a, 0, +a}; ein Wert wird null, wenn sein
doppelter Betrag nicht ueber a liegt. a ist das Betragsmittel der Gruppe,
wie in der Ableitung des Trainings, aber genommen NACH der Kompensation der
Spalten davor, und gerundet auf das int8-Raster der Zeile, damit die
Kompensation mit dem Wert rechnet, der wirklich gespeichert wird.

Gleitkomma steht hier mit Absicht: Das ist die Kalibrierung. Heraus kommen
int8-Werte und Zeilenshifts, also ein Artefakt; was alle Knoten gleich
rechnen, ist das Artefakt und nicht dieses Skript.

Aufruf aus der Wurzel des Repositoriums (der Mitschnitt kommt aus
`expertenmitschnitt`):

    $V INTEGER_LLM/calibrate/src/ternaer_gptq.py <artefakt> <mitschnitt> <ziel> \
        [--ebenen 0-39] [--paket 32] [--halte 512]

`ziel` erhaelt je umgewandelter Matrix `<datei>.bin` und `<shifts_datei>.bin`
unter den Namen des Manifests. `--halte` haelt die letzten N Token jeder
Ebene zurueck und meldet auf ihnen den relativen Ausgabefehler von GPTQ und
von blossem Runden.
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

import numpy as np
import torch

GRUPPE = 128
MASTER_FRAC = 20
DAEMPFUNG = 0.1

# Vielfache des Betragsmittels, unter denen je Gruppe der Betrag a gewaehlt
# wird. Gemessen am 35B (2026-10-05, Ebenen 3, 20, 37, je 16 Experten): die
# Suche senkt den Ausgabefehler gegenueber a = Betragsmittel um 20 bis 29 %,
# eine Hadamard-Drehung allein um 4 bis 12 %.
SKALENFAKTOREN = (0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.35, 1.5)


def raster_bits(W: torch.Tensor) -> torch.Tensor:
    """Zusaetzliche Bruchstellen je Zeile, damit der groesste Gruppenbetrag
    knapp in 127 passt. W: [E, zeilen, spalten] in int8-Einheiten."""
    a = W.abs().reshape(W.shape[0], W.shape[1], -1, GRUPPE).mean(dim=3)
    amax = a.amax(dim=2).clamp(min=1e-9)
    return torch.floor(torch.log2(127.0 / amax)).clamp(min=-30, max=30)


def ternaer_runden(w: torch.Tensor, a_q: torch.Tensor) -> torch.Tensor:
    """Naechster Wert aus {-a, 0, +a}; null, wenn 2|w| nicht ueber a liegt."""
    return torch.where(2.0 * w.abs() > a_q, torch.sign(w) * a_q, torch.zeros_like(w))


def obere_inverse_cholesky(H: torch.Tensor) -> torch.Tensor:
    """U mit H^-1 = U^T U, je Experte; H bereits gedaempft, float64."""
    L = torch.linalg.cholesky(H)
    Hinv = torch.cholesky_inverse(L)
    Hinv = (Hinv + Hinv.transpose(-1, -2)) / 2.0
    return torch.linalg.cholesky(Hinv).transpose(-1, -2)


def ternaer_gptq(W: torch.Tensor, H: torch.Tensor | None, k: torch.Tensor,
                 faktoren=SKALENFAKTOREN) -> torch.Tensor:
    """W: [E, zeilen, spalten] (int8-Einheiten, float32), H: [E, spalten, spalten]
    oder None (blosses Runden), k: [E, zeilen] Rasterbits.
    Gibt Q in int8-Einheiten * 2^k zurueck, also ganze Zahlen bis 127.

    Je Gruppe wird a unter `faktoren` mal dem Betragsmittel gewaehlt: der
    Wert, dessen Rundungsfehler d der Gruppe d^T H d am kleinsten macht (ohne
    H: d^T d). `faktoren=(1.0,)` ist das Betragsmittel allein."""
    E, zeilen, spalten = W.shape
    W = W.clone()
    Q = torch.zeros_like(W)
    skala = torch.pow(2.0, k).unsqueeze(2)  # [E, zeilen, 1]
    U = None
    Hd = None
    if H is not None:
        Hd = H.double().clone()
        mittel = torch.diagonal(Hd, dim1=1, dim2=2).mean(dim=1).clamp(min=1e-9)
        Hd += DAEMPFUNG * mittel.view(E, 1, 1) * torch.eye(spalten, dtype=torch.float64)
        U = obere_inverse_cholesky(Hd).float()
    for g0 in range(0, spalten, GRUPPE):
        g1 = g0 + GRUPPE
        basis = W[:, :, g0:g1].abs().mean(dim=2, keepdim=True)
        Hg = None if H is None else Hd[:, g0:g1, g0:g1].float()
        a_q, bester = None, None
        for f in faktoren:
            kandidat = torch.clamp(torch.round(basis * f * skala), 0, 127) / skala
            d = W[:, :, g0:g1] - ternaer_runden(W[:, :, g0:g1], kandidat)
            fehler = (d * d).sum(dim=2) if Hg is None else torch.einsum("erc,ecd,erd->er", d, Hg, d)
            if bester is None:
                a_q, bester = kandidat, fehler
            else:
                besser = (fehler < bester).unsqueeze(2)
                a_q = torch.where(besser, kandidat, a_q)
                bester = torch.minimum(fehler, bester)
        if U is None:
            Q[:, :, g0:g1] = ternaer_runden(W[:, :, g0:g1], a_q)
            continue
        fehler = torch.zeros(E, zeilen, GRUPPE)
        for j in range(GRUPPE):
            i = g0 + j
            w = W[:, :, i]
            q = ternaer_runden(w.unsqueeze(2), a_q).squeeze(2)
            Q[:, :, i] = q
            d = U[:, i, i].unsqueeze(1)
            e = (w - q) / d
            fehler[:, :, j] = e
            # Innerhalb der Gruppe sofort, dahinter gesammelt (blockweise).
            W[:, :, i:g1] -= e.unsqueeze(2) * U[:, i, i:g1].unsqueeze(1)
        if g1 < spalten:
            W[:, :, g1:] -= torch.bmm(fehler, U[:, g0:g1, g1:])
    return torch.round(Q * skala)


def ausgabefehler(X: torch.Tensor, W: torch.Tensor, Q: torch.Tensor) -> float:
    """||X W^T - X Q^T||^2 / ||X W^T||^2 fuer einen Experten."""
    if X.shape[0] == 0:
        return float("nan")
    soll = X @ W.T
    return float(((soll - X @ Q.T) ** 2).sum() / (soll ** 2).sum().clamp(min=1e-9))


def lies(pfad: Path, dtype, form) -> np.ndarray:
    return np.fromfile(pfad, dtype=dtype).reshape(form)


def ebene_umwandeln(artefakt: Path, mitschnitt: Path, ziel: Path, e: int, kopf: dict,
                    manifest: dict, paket: int, halte: int) -> dict:
    n, hidden, top_k, inter = kopf["token"], kopf["hidden"], kopf["top_k"], kopf["moe_intermediate"]
    x = lies(mitschnitt / f"x_{e}.bin", "<i2", (n, hidden)).astype(np.float32)
    wahl = lies(mitschnitt / f"wahl_{e}.bin", "<u2", (n, top_k))
    h = lies(mitschnitt / f"h_{e}.bin", "<i2", (n, top_k, inter)).astype(np.float32)
    lern = n - halte
    experten = sorted({int(k.split("_experts_")[1].split("_")[0])
                       for k in manifest if k.startswith(f"model_layers_{e}_mlp_experts_")})
    bericht = {"gptq": [], "rtn": [], "ohne_token": 0}
    for p0 in range(0, len(experten), paket):
        teil = experten[p0:p0 + paket]
        for matrix in ("gate_proj", "up_proj", "down_proj"):
            Ws, ks, Hs, Xh, alt_shifts, eintraege = [], [], [], [], [], []
            for j in teil:
                name = f"model_layers_{e}_mlp_experts_{j}_{matrix}_weight"
                ein = manifest[name]
                zeilen, spalten = ein["shape"]
                w = lies(artefakt / ein["file"], np.int8, (zeilen, spalten)).astype(np.float32)
                Ws.append(torch.from_numpy(w))
                alt_shifts.append(np.fromfile(artefakt / ein["shifts_file"], dtype=np.uint8))
                eintraege.append(ein)
                if matrix == "down_proj":
                    zeilen_j, slot_j = np.nonzero(wahl == j)
                    eingabe = h[zeilen_j, slot_j, :]
                    halt_maske = zeilen_j >= lern
                else:
                    zeilen_j = np.nonzero((wahl == j).any(axis=1))[0]
                    eingabe = x[zeilen_j]
                    halt_maske = zeilen_j >= lern
                X = torch.from_numpy(eingabe[~halt_maske])
                Xh.append(torch.from_numpy(eingabe[halt_maske]))
                if X.shape[0] == 0:
                    bericht["ohne_token"] += 1
                Hs.append(X.double().T @ X.double())
            W = torch.stack(Ws)
            # Hoechstens MASTER_FRAC Bruchstellen je Zeile: mehr traegt weder
            # das Format noch die Ableitung des Trainings. Eine Zeile mit sehr
            # kleinen Gewichten wird dadurch groeber, wie dort auch.
            alt = torch.from_numpy(np.stack(alt_shifts).astype(np.float32))
            k = torch.minimum(raster_bits(W), MASTER_FRAC - alt)
            H = torch.stack(Hs)
            # Ohne ein einziges Token bleibt nur das blosse Runden; die
            # Daempfung allein machte daraus dasselbe, aber auf Umwegen.
            ohne = torch.tensor([float(Hj.trace()) == 0.0 for Hj in Hs])
            Q = ternaer_gptq(W, H, k)
            if ohne.any():
                Q[ohne] = ternaer_gptq(W[ohne], None, k[ohne])
            # Vergleich: blosses Runden mit dem Betragsmittel, wie die
            # Ableitung des Trainings es waehlt.
            R = ternaer_gptq(W, None, k, faktoren=(1.0,))
            for idx, j in enumerate(teil):
                skala = torch.pow(2.0, k[idx]).unsqueeze(1)
                bericht["gptq"].append(ausgabefehler(Xh[idx], W[idx], Q[idx] / skala))
                bericht["rtn"].append(ausgabefehler(Xh[idx], W[idx], R[idx] / skala))
                neue_shifts = alt_shifts[idx].astype(np.int32) + k[idx].numpy().astype(np.int32)
                if neue_shifts.min() < 0 or neue_shifts.max() > MASTER_FRAC:
                    raise SystemExit(f"[ternaer_gptq] FEHLER: Ebene {e} Experte {j} {matrix}: "
                                     f"Shift {neue_shifts.min()}..{neue_shifts.max()} ausserhalb 0..{MASTER_FRAC}")
                q = Q[idx].numpy().astype(np.int8)
                q.tofile(ziel / eintraege[idx]["file"])
                neue_shifts.astype(np.uint8).tofile(ziel / eintraege[idx]["shifts_file"])
    return bericht


def main() -> int:
    args = sys.argv[1:]
    if len(args) < 3:
        print(__doc__)
        return 2
    artefakt, mitschnitt, ziel = Path(args[0]), Path(args[1]), Path(args[2])
    opt = {"--ebenen": None, "--paket": "32", "--halte": "512"}
    for i in range(3, len(args), 2):
        opt[args[i]] = args[i + 1]
    kopf = json.loads((mitschnitt / "kopf.json").read_text())
    manifest = json.loads((artefakt / "weights_manifest.json").read_text())
    ebenen = kopf["ebenen"]
    if opt["--ebenen"]:
        v, b = (int(s) for s in opt["--ebenen"].split("-"))
        ebenen = [e for e in ebenen if v <= e <= b]
    ziel.mkdir(parents=True, exist_ok=True)
    torch.set_num_threads(max(1, torch.get_num_threads()))
    for e in ebenen:
        t0 = time.time()
        b = ebene_umwandeln(artefakt, mitschnitt, ziel, e, kopf, manifest,
                            int(opt["--paket"]), int(opt["--halte"]))
        g = np.nanmean(b["gptq"])
        r = np.nanmean(b["rtn"])
        print(f"[ternaer_gptq] Ebene {e}: relativer Ausgabefehler auf der Haltemenge "
              f"GPTQ {g:.4f}, blosses Runden {r:.4f} ({100 * (1 - g / r):.1f} % weniger); "
              f"{b['ohne_token']} Matrizen ohne Token; {time.time() - t0:.0f} s", flush=True)
    print("Fertig")
    return 0


if __name__ == "__main__":
    sys.exit(main())
