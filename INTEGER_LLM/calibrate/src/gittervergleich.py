#!/usr/bin/env python3
"""
**Gitter fuer die Experten im Vergleich**: welche trainingsfreie Rundung
traegt wie viel, und als Artefakt zum Nachmessen.

# ⚑ Wozu

Ternaer gerundete Experten verlieren am 35B fast die Haelfte der Qualitaet
(Perplexitaet 11,01 gegen 21,21). Die Literatur kennt viele Verfahren, die
das ohne Training mildern sollen; dieses Werkzeug misst sie **am selben
Modell, an denselben Token, mit demselben Fehlermass** wie `ternaer_gptq.py`:
dem relativen quadrierten Ausgabefehler ||X W^T - X Q^T||^2 / ||X W^T||^2
auf zurueckgehaltenen Token.

Jedes Verfahren ist GPTQ (Frantar u. a. 2022) mit einem eigenen Gitter je
Zeile und Gruppe von 128 Eingaengen, gelegt auf die schon kompensierten
Gewichte am Gruppenanfang und gewaehlt nach dem mit H gewichteten Fehler:

| Name | Stufen je Gruppe |
|---|---|
| `ternaer` | {-a, 0, a}, Skalensuche wie `ternaer_gptq.py` |
| `asym` | {m - a, m, m + a}, Versatz m je Gruppe |
| `zwei` | s1 t1 + s2 t2 mit t in {-1, 0, 1}: zwei Trit-Ebenen, 9 Stufen |
| `zweiganz` | dasselbe mit ganzzahligen s1, s2 und abs(s1) + abs(s2) <= 127, also exakt im int8-Raster |
| `zweischnell` | wie `zweiganz`, 4 statt 12 Runden der Anpassung |
| `neun` | gebundene Ebenen, s1 = 3 s2: gleichmaessig -4..4 |
| `int2`, `int3`, `int4` | gleichmaessig mit Versatz, Kappungssuche |

Zusaetze, mit `+` verbunden: `had` (Hadamard-Drehung der Eingaenge vor dem
Runden), `ausN` (N Promille der Gewichte mit dem groessten w^2 H_ii bleiben
exakt), `rN` (Ausgleich des Rests mit Rang N im Eigenraum von H, wie EoRA).

Gleitkomma steht hier mit Absicht: Das ist die Kalibrierung. Heraus kommen
Fehlerzahlen oder ein Artefakt; was alle Knoten gleich rechnen, ist das
Artefakt.

# Aufruf, aus der Wurzel (der Mitschnitt kommt aus `expertenmitschnitt`)

    $V INTEGER_LLM/calibrate/src/gittervergleich.py vergleich <artefakt> <mitschnitt> <ebenen,...> <experten je ebene> <halte> <verfahren,...>
    $V INTEGER_LLM/calibrate/src/gittervergleich.py bauen <int8-artefakt> <mitschnitt> <ziel> <verfahren> [--ebenen a-b] [--paket 32] [--schrumpf n0] [--gruppe 128]
    $V INTEGER_LLM/calibrate/src/gittervergleich.py abschluss <int8-artefakt> <ziel>
    $V INTEGER_LLM/calibrate/src/gittervergleich.py schranke <artefakt> <mitschnitt> <ebenen,...> <raten,...> <halte>
    $V INTEGER_LLM/calibrate/src/gittervergleich.py seltenheit <artefakt> <mitschnitt> <ebenen,...> <experten je ebene> <k,...> <n0,...> <halte> [verfahren]

# ⚑ Seltene Experten: `--schrumpf n0`

Ein Experte sieht im Mitschnitt im Mittel `token * top_k / 256` Token, die
meisten also weniger als die 2048 Eingaenge seiner Matrix. Dann ist sein H
fast nur Daempfung plus ein Unterraum, und GPTQ gleicht auf genau diese
Token aus: `seltenheit` misst am 35B (int4, Haltetoken), dass ein Experte
mit 256 Lerntoken nicht besser rundet als einer mit 4, und deutlich
schlechter als mit allen. `--schrumpf n0` setzt H = X^T X + n0 Sigma mit
der Lage der Ebene: fuer gate und up die Kovarianz aller x der Ebene (jeder
Experte sieht dieselben normierten Eingaenge), fuer down die Kovarianz des
eigenen h, nachgerechnet auf `LAGE_TOKEN` Token der Ebene aus x und den
echten Gewichten von gate und up (`h_nachrechnen`, die beiden Shifts je
Ebene gesucht in `h_skala`, Rest unter 1e-3). Mit n0 = 8192 erreicht jede
Lernmenge den Fehler, den alle Token gaeben.

`schranke` sagt, was bei R Bit je Gewicht **bestenfalls** erreichbar ist:
ideale Transformationscodierung (umgekehrtes Wasserfuellen in der Basis
der Eingaenge, gaussche Naeherung), gemessen auf Haltetoken. ⚠️ Ohne
Haltetoken taeuscht sie: Im Eigenraum eines einzelnen Experten, der nur
einige hundert Token sieht, lag der Fehler auf den Lerntoken bei 0,002 bis
0,015 und auf den Haltetoken bei 0,05 bis 0,11.

`bauen` schreibt die Experten in den **int8-Behaelter**: gleiche Dateien,
gleiche Shifts, die Werte liegen auf dem Gitter des Verfahrens. Damit misst
jedes vorhandene Werkzeug (Perplexitaet, Fragen) die Qualitaet eines Gitters,
ohne dass es dafuer ein gepacktes Format und eigene Kerne geben muss; fuer
`zweiganz` rechnet der Behaelter genau die ganzen Zahlen, die zwei ternaere
Ebenen rechnen wuerden. Mehrere `bauen` duerfen mit getrennten `--ebenen`
nebeneinander laufen; `abschluss` verlinkt danach einmal, was fehlt, und
schreibt Manifest und `weights_hash` neu.
"""

from __future__ import annotations

import hashlib
import json
import os
import sys
import time
from pathlib import Path

import numpy as np
import torch

G = 128
DAEMPFUNG = 0.1
# Token, auf denen `--schrumpf` das h eines Experten fuer die Lage von down
# nachrechnet; Sigma ist 512 x 512, dafuer reichen sie weit.
LAGE_TOKEN = 8192
# Wie `pruefstand::MARKENNAME` der Laufzeit.
PRUEFMARKE = ".geprueft"
FAKT = (0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.35, 1.5)


def lies(p, dt, form):
    return np.fromfile(p, dtype=dt).reshape(form)


def nachstes(w, stufen):
    """w: [E,R], stufen: [E,R,L] -> naechste Stufe."""
    i = (w.unsqueeze(2) - stufen).abs().argmin(dim=2, keepdim=True)
    return torch.gather(stufen, 2, i).squeeze(2)


# ---- Gitter je Gruppe: (wg [E,R,G], hdiag [E,G], Hg [E,G,G]) -> stufen [E,R,L]

def hfehler(wg, stufen, Hg):
    d = wg - torch.gather(stufen, 2, (wg.unsqueeze(3) - stufen.unsqueeze(2)).abs().argmin(dim=3))
    return torch.einsum("erc,ecd,erd->er", d, Hg, d)


def waehle(wg, Hg, kandidaten):
    bester, wahl = None, None
    for st in kandidaten:
        f = hfehler(wg, st, Hg)
        if bester is None:
            bester, wahl = f, st
        else:
            m = (f < bester).unsqueeze(2)
            wahl = torch.where(m, st, wahl)
            bester = torch.minimum(f, bester)
    return wahl


def gitter_ternaer(wg, hd, Hg):
    b = wg.abs().mean(dim=2, keepdim=True)
    return waehle(wg, Hg, [torch.cat([-b * f, 0 * b, b * f], 2) for f in FAKT])


def gitter_asym(wg, hd, Hg):
    mu = (wg * hd.unsqueeze(1)).sum(2, keepdim=True) / hd.sum(1).view(-1, 1, 1).clamp(min=1e-9)
    b = (wg - mu).abs().mean(dim=2, keepdim=True)
    return waehle(wg, Hg, [torch.cat([mu - b * f, mu, mu + b * f], 2) for f in FAKT])


def gitter_zwei(wg, hd, Hg, iter_=12, ganz=False):
    """Zwei Trit-Ebenen: Stufen s1*t1 + s2*t2, t in {-1,0,1}; s1, s2 je Zeile
    und Gruppe abwechselnd: Zuordnung, dann 2x2-Ausgleich mit diag(H)."""
    E, R, _ = wg.shape
    t = torch.tensor([(a, b) for a in (-1, 0, 1) for b in (-1, 0, 1)], dtype=wg.dtype)  # [9,2]
    kandidaten = []
    for f in (0.5, 0.7, 0.9):
        s1 = wg.abs().amax(2) * f
        s2 = s1 / 3
        hw = hd.unsqueeze(1).expand(E, R, G)
        for _ in range(iter_):
            st = s1.unsqueeze(2) * t[:, 0] + s2.unsqueeze(2) * t[:, 1]  # [E,R,9]
            i = (wg.unsqueeze(3) - st.unsqueeze(2)).abs().argmin(dim=3)  # [E,R,G]
            a, b = t[i, 0], t[i, 1]
            A11 = (hw * a * a).sum(2); A12 = (hw * a * b).sum(2); A22 = (hw * b * b).sum(2)
            y1 = (hw * a * wg).sum(2); y2 = (hw * b * wg).sum(2)
            det = A11 * A22 - A12 * A12
            ok = det.abs() > 1e-12
            n1 = torch.where(ok, (A22 * y1 - A12 * y2) / det.where(ok, torch.ones_like(det)), s1)
            n2 = torch.where(ok, (A11 * y2 - A12 * y1) / det.where(ok, torch.ones_like(det)), s2)
            s1, s2 = n1, n2
        if ganz:
            s1, s2 = torch.round(s1), torch.round(s2)
            s1 = torch.sign(s1) * torch.minimum(s1.abs(), 127 - s2.abs())
        kandidaten.append(s1.unsqueeze(2) * t[:, 0] + s2.unsqueeze(2) * t[:, 1])
    return waehle(wg, Hg, kandidaten)


def gitter_neun(wg, hd, Hg):
    """Gebundene Ebenen (s1 = 3 s2): gleichmaessig -4..4."""
    b = wg.abs().amax(2, keepdim=True) / 4
    k = torch.arange(-4, 5, dtype=wg.dtype)
    return waehle(wg, Hg, [b * f * k for f in (0.6, 0.7, 0.8, 0.9, 1.0)])


def gitter_int(bits, ganz=False):
    """Gleichmaessig mit Versatz, 2^bits Stufen je Zeile und Gruppe.

    ganz: Schritt a und Versatz b sind ganze Zahlen, die Stufen also
    a*q + b mit q in 0..2^bits-1, und alle liegen in -127..127. Das ist
    genau die Form, die das gepackte Vierbitformat der Laufzeit traegt
    (`integer_llm_kernels::vierbit`): Der Behaelter rechnet dann dieselben
    ganzen Zahlen wie das gepackte Artefakt. Je Faktor zwei Kandidaten,
    Schritt abgerundet und aufgerundet."""
    def g(wg, hd, Hg):
        lo, hi = wg.amin(2, keepdim=True), wg.amax(2, keepdim=True)
        n = 2 ** bits - 1
        k = torch.arange(n + 1, dtype=wg.dtype)
        kand = []
        for f in (1.0, 0.95, 0.9, 0.85, 0.8, 0.7):
            m = (lo + hi) / 2
            l2, h2 = m + (lo - m) * f, m + (hi - m) * f
            s = ((h2 - l2) / n).clamp(min=1e-9)
            if not ganz:
                kand.append(l2 + s * k)
                continue
            for a in (torch.floor(s), torch.ceil(s)):
                a = a.clamp(1, 254 // n)
                b = torch.round(m - a * n / 2)
                b = torch.minimum(torch.maximum(b, torch.full_like(b, -127.0)), 127 - a * n)
                kand.append(b + a * k)
        return waehle(wg, Hg, kand)
    return g


def gptq(W, H, gitter, frei=None):
    """W [E,R,C], H [E,C,C] (gedaempft), frei: Maske [E,R,C] fuer exakt
    gehaltene Gewichte (Ausreisser)."""
    E, R, C = W.shape
    if C % G:
        raise SystemExit(f"[gittervergleich] FEHLER: {C} Spalten sind kein Vielfaches der Gruppe {G}")
    W = W.clone()
    Q = torch.zeros_like(W)
    L = torch.linalg.cholesky(H)
    Hinv = torch.cholesky_inverse(L)
    Hinv = (Hinv + Hinv.transpose(-1, -2)) / 2
    U = torch.linalg.cholesky(Hinv).transpose(-1, -2).float()
    Hf = H.float()
    hd = torch.diagonal(Hf, dim1=1, dim2=2)
    for g0 in range(0, C, G):
        g1 = g0 + G
        st = gitter(W[:, :, g0:g1], hd[:, g0:g1], Hf[:, g0:g1, g0:g1])
        err = torch.zeros(E, R, G)
        for j in range(G):
            i = g0 + j
            w = W[:, :, i]
            q = nachstes(w, st)
            if frei is not None:
                q = torch.where(frei[:, :, i], w, q)
            Q[:, :, i] = q
            e = (w - q) / U[:, i, i].unsqueeze(1)
            err[:, :, j] = e
            W[:, :, i:g1] -= e.unsqueeze(2) * U[:, i, i:g1].unsqueeze(1)
        if g1 < C:
            W[:, :, g1:] -= torch.bmm(err, U[:, g0:g1, g1:])
    return Q


def rang_ausgleich(W, Q, H, r):
    """EoRA: Rest im Eigenraum von H, auf Rang r gekuerzt."""
    out = torch.zeros_like(W)
    for e in range(W.shape[0]):
        lam, V = torch.linalg.eigh(H[e])
        lam = lam.clamp(min=lam.max() * 1e-8)
        s = lam.sqrt()
        Ed = (W[e] - Q[e]).double() @ V * s
        u, sv, vt = torch.linalg.svd(Ed, full_matrices=False)
        Er = (u[:, :r] * sv[:r]) @ vt[:r]
        out[e] = (Er / s @ V.T).float()
    return Q + out


def fehler(X, W, Q):
    if X.shape[0] == 0:
        return float("nan")
    soll = X @ W.T
    return float(((soll - X @ Q.T) ** 2).sum() / (soll ** 2).sum().clamp(min=1e-9))


# ---- Schrumpfen zur Lage der Ebene (seltene Experten)

def echt(art, ein):
    """Gewichte in echten Einheiten: int8 durch 2^shift je Zeile."""
    r, c = ein["shape"]
    W = lies(art / ein["file"], np.int8, (r, c)).astype(np.float32)
    if "shifts_file" in ein:
        s = np.fromfile(art / ein["shifts_file"], dtype=np.uint8).astype(np.float32)
    else:
        s = np.full(r, float(ein["shift"]), dtype=np.float32)
    return torch.from_numpy(W / (2.0 ** s)[:, None])


def h_nachrechnen(xs, Wg, Wu, la, lb):
    """Der Eingang von down, wie die Laufzeit ihn bildet, in Gleitkomma:
    2^lb silu(2^la g) u mit g, u aus den echten Gewichten."""
    g = (xs @ Wg.T) * 2.0 ** la
    return 2.0 ** lb * g * torch.sigmoid(g) * (xs @ Wu.T)


def h_skala(art, man, e, x, wahl, h):
    """la und lb sind je Ebene feste Zweierpotenzen (die Shifts von x und
    h), gesucht auf den vier haeufigsten Experten. Gibt (la, lb, Rest)."""
    teil = np.argsort(-np.bincount(wahl.ravel(), minlength=256))[:4]
    stuecke = []
    for j in teil:
        zj, sj = np.nonzero(wahl == j)
        zj, sj = zj[:2048], sj[:2048]
        xs = torch.from_numpy(x[zj])
        stuecke.append((xs, echt(art, man[f"model_layers_{e}_mlp_experts_{j}_gate_proj_weight"]),
                        echt(art, man[f"model_layers_{e}_mlp_experts_{j}_up_proj_weight"]),
                        torch.from_numpy(h[zj, sj]).double()))
    bester = None
    for la in range(-24, 5):
        ps = [h_nachrechnen(xs, Wg, Wu, la, 0).double() for xs, Wg, Wu, _ in stuecke]
        pp = sum(float((p * p).sum()) for p in ps)
        ph = sum(float((p * hh).sum()) for p, (_, _, _, hh) in zip(ps, stuecke))
        if pp <= 0 or ph <= 0:
            continue
        lb = int(round(np.log2(ph / pp)))
        rest = sum(float(((hh - 2.0 ** lb * p) ** 2).sum()) for p, (_, _, _, hh) in zip(ps, stuecke))
        rest /= max(sum(float((hh ** 2).sum()) for *_, hh in stuecke), 1e-30)
        if bester is None or rest < bester[2]:
            bester = (la, lb, rest)
    if bester is None or bester[2] > 0.01:
        raise SystemExit(f"[gittervergleich] FEHLER: Ebene {e}: h laesst sich nicht nachrechnen ({bester})")
    return bester


def kovarianz(X):
    """X^T X / n in double, stueckweise (X in float32)."""
    S = torch.zeros(X.shape[1], X.shape[1], dtype=torch.float64)
    for i in range(0, X.shape[0], 8192):
        t = X[i:i + 8192].double()
        S += t.T @ t
    return S / max(X.shape[0], 1)


def hesse(X, lage=None, n0=0.0):
    """H = X^T X + n0 Sigma der Ebene, dann gedaempft wie bisher."""
    X = X.double()
    H = X.T @ X
    if n0 and lage is not None:
        H = H + n0 * lage
    m = torch.diagonal(H).mean().clamp(min=1e-9)
    return H + DAEMPFUNG * m * torch.eye(H.shape[0], dtype=torch.float64)


GITTER = {
    "ternaer": gitter_ternaer, "asym": gitter_asym, "zwei": gitter_zwei, "zweiganz": lambda a, b, c: gitter_zwei(a, b, c, ganz=True),
    "zweischnell": lambda a, b, c: gitter_zwei(a, b, c, iter_=4, ganz=True), "neun": gitter_neun,
    "int2": gitter_int(2), "int3": gitter_int(3), "int4": gitter_int(4),
    "int4ganz": gitter_int(4, ganz=True),
}


def hadamard(n):
    h = torch.ones(1, 1, dtype=torch.float64)
    while h.shape[0] < n:
        h = torch.cat([torch.cat([h, h], 1), torch.cat([h, -h], 1)], 0)
    return h / np.sqrt(n)


def verfahren(v, W, H, Xh):
    """v: Teile mit '+': ein Gitter, dazu had (Drehung), ausN (N Promille
    exakt gehalten), rN (Rang-Ausgleich). Gibt Q und Xh im selben Raum."""
    teile = v.split("+")
    gitter = next(GITTER[t] for t in teile if t in GITTER)
    if "had" in teile:
        D = hadamard(W.shape[2])
        W = W @ D.float()
        H = D.T @ H @ D
        Xh = [x @ D.float() for x in Xh]
    frei = None
    for t in teile:
        if t.startswith("aus"):
            p = int(t[3:]) / 1000
            hd = torch.diagonal(H, dim1=1, dim2=2).float()
            s = (W ** 2) * hd.unsqueeze(1)
            k = max(1, int(p * W.shape[1] * W.shape[2]))
            schwelle = s.reshape(W.shape[0], -1).topk(k, dim=1).values[:, -1]
            frei = s >= schwelle.view(-1, 1, 1)
    Q = gptq(W, H, gitter, frei)
    for t in teile:
        if t.startswith("r") and t[1:].isdigit():
            Q = rang_ausgleich(W, Q, H, int(t[1:]))
    return W, Q, Xh


def vergleich(argv):
    art, mit = Path(argv[1]), Path(argv[2])
    ebenen = [int(s) for s in argv[3].split(",")]
    je = int(argv[4])
    halte = int(argv[5])
    namen = argv[6].split(",")
    kopf = json.loads((mit / "kopf.json").read_text())
    man = json.loads((art / "weights_manifest.json").read_text())
    n, hid, top, inter = kopf["token"], kopf["hidden"], kopf["top_k"], kopf["moe_intermediate"]
    lern = n - halte
    torch.manual_seed(0)
    ges = {v: {m: [] for m in ("gate_proj", "up_proj", "down_proj")} for v in namen}
    for e in ebenen:
        t0 = time.time()
        x = lies(mit / f"x_{e}.bin", "<i2", (n, hid)).astype(np.float32)
        wahl = lies(mit / f"wahl_{e}.bin", "<u2", (n, top))
        h = lies(mit / f"h_{e}.bin", "<i2", (n, top, inter)).astype(np.float32)
        # Experten mit genug Haltetoken, ueber die Haeufigkeit verteilt
        zahl = np.bincount(wahl[lern:].ravel(), minlength=256)
        kand = [j for j in np.argsort(-np.bincount(wahl.ravel(), minlength=256)) if zahl[j] >= 16]
        teil = [int(kand[i * len(kand) // je]) for i in range(je)]
        for matrix in ("gate_proj", "up_proj", "down_proj"):
            Ws, Hs, Xh = [], [], []
            for j in teil:
                ein = man[f"model_layers_{e}_mlp_experts_{j}_{matrix}_weight"]
                r, c = ein["shape"]
                Ws.append(torch.from_numpy(lies(art / ein["file"], np.int8, (r, c)).astype(np.float32)))
                if matrix == "down_proj":
                    zj, sj = np.nonzero(wahl == j)
                    ein_ = h[zj, sj, :]
                else:
                    zj = np.nonzero((wahl == j).any(axis=1))[0]
                    ein_ = x[zj]
                hm = zj >= lern
                X = torch.from_numpy(ein_[~hm]).double()
                Xh.append(torch.from_numpy(ein_[hm]))
                Hj = X.T @ X
                m = torch.diagonal(Hj).mean().clamp(min=1e-9)
                Hs.append(Hj + DAEMPFUNG * m * torch.eye(Hj.shape[0], dtype=torch.float64))
            W = torch.stack(Ws)
            H = torch.stack(Hs)
            for v in namen:
                Wv, Q, Xv = verfahren(v, W, H, Xh)
                for idx in range(len(teil)):
                    ges[v][matrix].append(fehler(Xv[idx], Wv[idx], Q[idx]))
        zeile = "  ".join(f"{v} " + "/".join(f"{np.nanmean(ges[v][m][-je:]):.3f}" for m in ges[v]) for v in namen)
        print(f"Ebene {e} ({time.time() - t0:.0f} s): {zeile}", flush=True)
    print("Mittel (gate/up/down, gesamt):")
    for v in namen:
        mm = [np.nanmean(ges[v][m]) for m in ges[v]]
        print(f"  {v:14s} {mm[0]:.4f} / {mm[1]:.4f} / {mm[2]:.4f}   {np.mean(mm):.4f}", flush=True)
    print("Fertig")


def bauen(argv):
    """Wie `_bauen`; `--gruppe` gilt nur fuer diesen Bau (G ist global, und
    ein Aufrufer danach soll nicht still mit der fremden Gruppe rechnen)."""
    global G
    alt = G
    try:
        _bauen(argv)
    finally:
        G = alt


def _bauen(argv):
    """Alle Experten der Ebenen nach `verf`, je Paket von Experten, in den
    int8-Behaelter. Gate und Up sehen `x` der Token, die den Experten
    gewaehlt haben; Down sieht sein `h` (wie `ternaer_gptq.py`)."""
    art, mit, ziel, verf = Path(argv[1]), Path(argv[2]), Path(argv[3]), argv[4]
    global G
    opt = {"--ebenen": None, "--paket": "32", "--schrumpf": "0", "--gruppe": str(G)}
    for i in range(5, len(argv), 2):
        opt[argv[i]] = argv[i + 1]
    kopf = json.loads((mit / "kopf.json").read_text())
    man = json.loads((art / "weights_manifest.json").read_text())
    n, hid, top, inter = kopf["token"], kopf["hidden"], kopf["top_k"], kopf["moe_intermediate"]
    ebenen = kopf["ebenen"]
    if opt["--ebenen"]:
        a, b = (int(x) for x in opt["--ebenen"].split("-"))
        ebenen = [e for e in ebenen if a <= e <= b]
    paket = int(opt["--paket"])
    n0 = float(opt["--schrumpf"])
    # Die Gruppe gilt fuer jedes Gitter; das Format braucht je Gruppe einen
    # Betrag (und bei intN einen Versatz), also 16 / G Bit mehr je Gewicht.
    G = int(opt["--gruppe"])
    ziel.mkdir(parents=True, exist_ok=True)
    for e in ebenen:
        t0 = time.time()
        x = lies(mit / f"x_{e}.bin", "<i2", (n, hid)).astype(np.float32)
        wahl = lies(mit / f"wahl_{e}.bin", "<u2", (n, top))
        h = lies(mit / f"h_{e}.bin", "<i2", (n, top, inter)).astype(np.float32)
        experten = sorted({int(k.split("_experts_")[1].split("_")[0]) for k in man if k.startswith(f"model_layers_{e}_mlp_experts_")})
        if n0:
            # Lage der Ebene: gate/up sehen dieselben x wie jeder Experte,
            # down bekommt sein eigenes h auf einer Auswahl aller Token.
            lage_x = kovarianz(torch.from_numpy(x))
            la, lb, rest = h_skala(art, man, e, x, wahl, h)
            xs = torch.from_numpy(x[::max(1, n // LAGE_TOKEN)])
        geklemmt = 0
        for p0 in range(0, len(experten), paket):
            teil = experten[p0:p0 + paket]
            for matrix in ("gate_proj", "up_proj", "down_proj"):
                Ws, Hs, ein = [], [], []
                for j in teil:
                    m = man[f"model_layers_{e}_mlp_experts_{j}_{matrix}_weight"]
                    r, c = m["shape"]
                    Ws.append(torch.from_numpy(lies(art / m["file"], np.int8, (r, c)).astype(np.float32)))
                    ein.append(m)
                    lage = None
                    if matrix == "down_proj":
                        zj, sj = np.nonzero(wahl == j); X = h[zj, sj, :]
                        if n0:
                            lage = kovarianz(h_nachrechnen(
                                xs, echt(art, man[f"model_layers_{e}_mlp_experts_{j}_gate_proj_weight"]),
                                echt(art, man[f"model_layers_{e}_mlp_experts_{j}_up_proj_weight"]), la, lb))
                    else:
                        zj = np.nonzero((wahl == j).any(axis=1))[0]; X = x[zj]
                        lage = lage_x if n0 else None
                    Hs.append(hesse(torch.from_numpy(X), lage, n0))
                _, Q, _ = verfahren(verf, torch.stack(Ws), torch.stack(Hs), [])
                Qi = torch.round(Q)
                geklemmt += int((Qi.abs() > 127).sum())
                Qi = Qi.clamp(-127, 127).numpy().astype(np.int8)
                for idx, m in enumerate(ein):
                    # 📌 Fund 549. Nie durch einen harten Link schreiben: neue Datei,
                    # dann umbenennen. Ein erster Entwurf verlinkte am Ende
                    # jedes Teillaufs die fehlenden Dateien aus dem int8; ein
                    # spaeterer Teillauf haette dann durch den Link in die
                    # Quelle geschrieben.
                    tmp = ziel / (m["file"] + ".tmp")
                    Qi[idx].tofile(tmp)
                    os.replace(tmp, ziel / m["file"])
        nach = f", h nachgerechnet mit Rest {rest:.5f}" if n0 else ""
        print(f"[bauen] Ebene {e}: {len(experten)} Experten, {geklemmt} Werte geklemmt{nach}, {time.time() - t0:.0f} s", flush=True)
    print("Fertig")


def abschluss(art, ziel):
    """Erst nach allen Teilen: was fehlt, als Link aus dem int8; danach ein
    eigenes Manifest mit den Pruefsummen der neu geschriebenen Dateien und
    ein eigener `weights_hash` (der Lader prueft jede Datei gegen ihr
    Manifest und weist ein Gemisch aus alten Summen und neuen Werten ab)."""
    for f in os.listdir(art):
        # Die Pruefmarke des Laders gilt nur fuer ihr eigenes Artefakt
        # (Fund 551: verlinkt, schrieb das Laden des Ziels in die der Quelle).
        if f.startswith(PRUEFMARKE):
            continue
        if f not in ("theta_v.json", "weights_manifest.json") and (art / f).is_file() and not (ziel / f).exists():
            os.link(art / f, ziel / f)
    man = json.loads((art / "weights_manifest.json").read_text())
    neu = 0
    for eintrag in man.values():
        for datei, summe in (("file", "hash"), ("shifts_file", "shifts_hash")):
            if datei not in eintrag:
                continue
            p = ziel / eintrag[datei]
            if os.path.samefile(p, art / eintrag[datei]):
                continue
            eintrag[summe] = hashlib.sha256(p.read_bytes()).hexdigest()
            neu += 1
    text = json.dumps(man, indent=2, sort_keys=True).encode()
    for name, inhalt in (("weights_manifest.json", text), ("theta_v.json", None)):
        if inhalt is None:
            theta = json.loads((art / "theta_v.json").read_text())
            theta["weights_hash"] = hashlib.sha256(text).hexdigest()
            inhalt = json.dumps(theta, indent=2, sort_keys=True).encode()
        tmp = ziel / (name + ".tmp")
        tmp.write_bytes(inhalt)
        os.replace(tmp, ziel / name)
    print(f"[abschluss] {neu} Pruefsummen neu")
    print("Fertig")


def _theta(s, bits):
    """Wasserstand des umgekehrten Wasserfuellens fuer `bits` je Zeile."""
    lo, hi = 1e-30, max(float(s.max()), 1e-30)
    for _ in range(200):
        th = np.sqrt(lo * hi)
        r = np.maximum(0, 0.5 * np.log2(np.maximum(s, 1e-300) / th)).sum()
        lo, hi = (th, hi) if r > bits else (lo, th)
    return th


def schranke_fehler(W, B, lam, Hh, bits):
    """Fehler einer idealen Transformationscodierung auf Haltetoken.

    Basis `B` und Varianzen `lam` der Eingaenge kommen aus den Lerntoken;
    jede Komponente c_j = (W b_j) sqrt(lam_j) bekommt Bits nach dem
    umgekehrten Wasserfuellen (gaussche Naeherung, also erreichbar mit
    idealer Vektorcodierung). Komponenten unter dem Wasserstand fallen ganz
    weg. Gemessen wird mit der Kovarianz `Hh` der Haltetoken."""
    m = ((W @ B) ** 2).mean(0)
    s = m * lam
    th = _theta(s, bits)
    hh = np.einsum("ij,ik,kj->j", B, Hh, B)
    fehler = np.where(s > th, th / np.maximum(lam, 1e-300), m) * hh
    return float(fehler.sum() / (m * hh).sum())


def schranke(argv):
    """Was ist bei R Bit je Gewicht bestenfalls erreichbar? Je Ebene die
    Basis der Ebene (gate/up: alle x, down: alle h), je Experte dessen
    eigener Eigenraum, und ohne Drehung; alles auf Haltetoken."""
    art, mit = Path(argv[1]), Path(argv[2])
    ebenen = [int(s) for s in argv[3].split(",")]
    raten = [float(s) for s in argv[4].split(",")]
    halte = int(argv[5])
    kopf = json.loads((mit / "kopf.json").read_text())
    man = json.loads((art / "weights_manifest.json").read_text())
    n, hid, top, inter = kopf["token"], kopf["hidden"], kopf["top_k"], kopf["moe_intermediate"]
    lern = n - halte
    erg = {}
    for e in ebenen:
        x = lies(mit / f"x_{e}.bin", "<i2", (n, hid)).astype(np.float64)
        wahl = lies(mit / f"wahl_{e}.bin", "<u2", (n, top))
        h = lies(mit / f"h_{e}.bin", "<i2", (n, top, inter)).astype(np.float64)
        zahl = np.bincount(wahl.ravel(), minlength=256)
        teil = [int(j) for j in np.argsort(-zahl)[::32]][:8]
        hl = h[:lern].reshape(-1, inter)
        basen = {"x": np.linalg.eigh(x[:lern].T @ x[:lern])[1], "h": np.linalg.eigh(hl.T @ hl)[1]}
        for j in teil:
            for matrix in ("gate_proj", "up_proj", "down_proj"):
                ein = man[f"model_layers_{e}_mlp_experts_{j}_{matrix}_weight"]
                W = lies(art / ein["file"], np.int8, ein["shape"]).astype(np.float64)
                if matrix == "down_proj":
                    zj, sj = np.nonzero(wahl == j)
                    X, BL = h[zj, sj], basen["h"]
                else:
                    zj = np.nonzero((wahl == j).any(axis=1))[0]
                    X, BL = x[zj], basen["x"]
                hm = zj >= lern
                if hm.sum() < 16:
                    continue
                Hl, Hh = X[~hm].T @ X[~hm], X[hm].T @ X[hm]
                Hl = Hl + DAEMPFUNG * np.diag(Hl).mean() * np.eye(Hl.shape[0])
                lam_e, Ve = np.linalg.eigh(Hl)
                lam_l = np.maximum(np.einsum("ij,ik,kj->j", BL, Hl, BL), 0)
                for R in raten:
                    b = R * W.shape[1]
                    erg.setdefault((matrix, R), []).append((
                        schranke_fehler(W, BL, lam_l, Hh, b),
                        schranke_fehler(W, Ve, lam_e, Hh, b),
                        schranke_fehler(W, np.eye(W.shape[1]), np.diag(Hl), Hh, b)))
    for matrix in ("gate_proj", "up_proj", "down_proj"):
        for R in raten:
            a = np.array(erg.get((matrix, R), [(np.nan,) * 3])).mean(0)
            print(f"{matrix} R={R}: Basis der Ebene {a[0]:.4f}, Eigenraum des Experten {a[1]:.4f}, "
                  f"ohne Drehung {a[2]:.4f}, ungewichtet {2 ** (-2 * R):.4f}", flush=True)
    print("Fertig")


def seltenheit(argv):
    """Seltene Experten nachgestellt: haeufige Experten, deren H nur aus k
    zufaelligen Lerntoken kommt, gemessen auf ihren vielen Haltetoken;
    ohne und mit Schrumpfen zur Lage der Ebene (n0 Token Gewicht)."""
    art, mit = Path(argv[1]), Path(argv[2])
    ebenen = [int(s) for s in argv[3].split(",")]
    je = int(argv[4])
    ks = [int(s) for s in argv[5].split(",")]
    n0s = [float(s) for s in argv[6].split(",")]
    halte = int(argv[7])
    verf = argv[8] if len(argv) > 8 else "int4"
    kopf = json.loads((mit / "kopf.json").read_text())
    man = json.loads((art / "weights_manifest.json").read_text())
    n, hid, top, inter = kopf["token"], kopf["hidden"], kopf["top_k"], kopf["moe_intermediate"]
    lern = n - halte
    rng = np.random.default_rng(0)
    erg = {}
    for e in ebenen:
        t0 = time.time()
        x = lies(mit / f"x_{e}.bin", "<i2", (n, hid)).astype(np.float32)
        wahl = lies(mit / f"wahl_{e}.bin", "<u2", (n, top))
        h = lies(mit / f"h_{e}.bin", "<i2", (n, top, inter)).astype(np.float32)
        lage_x = kovarianz(torch.from_numpy(x[:lern]))
        la, lb, rest = h_skala(art, man, e, x[:lern], wahl[:lern], h[:lern])
        xs = torch.from_numpy(x[:lern][::max(1, lern // LAGE_TOKEN)])
        zahl_h = np.bincount(wahl[lern:].ravel(), minlength=256)
        teil = [int(j) for j in np.argsort(-np.bincount(wahl[:lern].ravel(), minlength=256)) if zahl_h[j] >= 64][:je]
        for j in teil:
            zj, sj = np.nonzero(wahl == j)
            lz = zj < lern
            for matrix in ("gate_proj", "up_proj", "down_proj"):
                ein = man[f"model_layers_{e}_mlp_experts_{j}_{matrix}_weight"]
                W = torch.from_numpy(lies(art / ein["file"], np.int8, ein["shape"]).astype(np.float32))
                if matrix == "down_proj":
                    L, Xh = h[zj[lz], sj[lz]], torch.from_numpy(h[zj[~lz], sj[~lz]])
                    lage = kovarianz(h_nachrechnen(
                        xs, echt(art, man[f"model_layers_{e}_mlp_experts_{j}_gate_proj_weight"]),
                        echt(art, man[f"model_layers_{e}_mlp_experts_{j}_up_proj_weight"]), la, lb))
                else:
                    L, Xh, lage = x[zj[lz]], torch.from_numpy(x[zj[~lz]]), lage_x
                for k in ks + [len(L)]:
                    k = min(k, len(L))
                    Xk = torch.from_numpy(L[rng.choice(len(L), size=k, replace=False)])
                    for n0 in [0.0] + n0s:
                        _, Q, _ = verfahren(verf, W.unsqueeze(0), hesse(Xk, lage, n0).unsqueeze(0), [])
                        erg.setdefault((matrix, k if k in ks else "alle", n0), []).append(fehler(Xh, W, Q[0]))
        print(f"Ebene {e}: {len(teil)} Experten, h nachgerechnet mit Rest {rest:.5f}, {time.time() - t0:.0f} s", flush=True)
    print(f"Mittel {verf}-Fehler auf Haltetoken, je Zeile k Lerntoken; n0 = 0 ist ohne Schrumpfen:")
    for matrix in ("gate_proj", "up_proj", "down_proj"):
        for k in ks + ["alle"]:
            z = [(n0, np.mean(erg[(matrix, k, n0)])) for n0 in [0.0] + n0s if (matrix, k, n0) in erg]
            if z:
                print(f"  {matrix:9s} k={str(k):5s} " + "  ".join(f"n0={int(a)} {b:.4f}" for a, b in z), flush=True)
    print("Fertig")


def main() -> int:
    befehl = sys.argv[1] if len(sys.argv) > 1 else ""
    if befehl == "vergleich":
        vergleich(sys.argv[1:])
    elif befehl == "bauen":
        bauen(sys.argv[1:])
    elif befehl == "schranke":
        schranke(sys.argv[1:])
    elif befehl == "seltenheit":
        seltenheit(sys.argv[1:])
    elif befehl == "abschluss":
        abschluss(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
