#!/usr/bin/env python3
"""Proben fuer calibrate/src/gittervergleich.py (eigenstaendig, kein pytest).

Aufruf aus der Wurzel:  $V INTEGER_LLM/tests/test_gittervergleich.py
"""

import hashlib
import json
import os
import sys
import tempfile
from pathlib import Path

import numpy as np
import torch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "calibrate" / "src"))
import gittervergleich as gv  # noqa: E402
import ternaer_gptq as tg  # noqa: E402


def lage(E=2, zeilen=16, spalten=256, saat=1):
    """Gewichte in int8-Einheiten, korrelierte Eingaenge, H wie im Werkzeug."""
    g = torch.Generator().manual_seed(saat)
    W = torch.round(torch.randn(E, zeilen, spalten, generator=g) * 30).clamp(-127, 127)
    A = torch.randn(spalten, spalten, generator=g) * 0.3 + torch.eye(spalten)
    X = torch.randn(1200, spalten, generator=g) @ A
    lern = X[:900].double()
    H = lern.T @ lern
    H = H + gv.DAEMPFUNG * torch.diagonal(H).mean() * torch.eye(spalten, dtype=torch.float64)
    return W, torch.stack([H] * E), X[900:]


def test_ternaer_ist_das_werkzeug():
    """Das Gitter `ternaer` ist dieselbe Rundung wie `ternaer_gptq.py`
    (dort zusaetzlich mit dem int8-Raster des Betrags). Der ganze Betrag
    waehlt in einzelnen Gruppen einen anderen Faktor; daher 5 % Spiel."""
    W, H, Xh = lage(E=4, zeilen=32)
    _, Q, _ = gv.verfahren("ternaer", W, H, [Xh] * 4)
    Qt = tg.ternaer_gptq(W, H, torch.zeros(W.shape[0], W.shape[1]))
    a = sum(gv.fehler(Xh, W[i], Q[i]) for i in range(4)) / 4
    b = sum(gv.fehler(Xh, W[i], Qt[i]) for i in range(4)) / 4
    assert abs(a - b) < 0.05 * b, f"ternaer {a:.4f} gegen Werkzeug {b:.4f}"
    print(f"ok ternaer ist das Werkzeug ({a:.4f} gegen {b:.4f})")


def test_zweiganz_sind_zwei_ganze_ebenen():
    """Jede Gruppe traegt hoechstens 9 Stufen der Form s1 t1 + s2 t2 mit
    ganzen s1, s2 und abs(s1) + abs(s2) <= 127."""
    W, H, Xh = lage(saat=2)
    _, Q, _ = gv.verfahren("zweiganz", W, H, [Xh] * 2)
    for gruppe in Q.reshape(-1, gv.G):
        stufen = sorted({float(v) for v in gruppe})
        assert all(v == int(v) for v in stufen), f"keine ganzen Stufen: {stufen}"
        assert len(stufen) <= 9, f"mehr als 9 Stufen: {stufen}"
        assert max(abs(v) for v in stufen) <= 127, f"ausserhalb int8: {stufen}"
        positiv = [v for v in stufen if v > 0]
        if len(positiv) == 4:
            # Die beiden groessten sind s1 und s1 + s2 (s1 der groessere Betrag).
            s1, s2 = positiv[2], positiv[3] - positiv[2]
            assert sorted(positiv[:2]) == sorted([s2, s1 - s2]), f"keine zwei Ebenen: {positiv}"
    print("ok zweiganz sind zwei ganze Ebenen")


def vierbittig(gruppe):
    """Wie der Packer der Laufzeit: Versatz der kleinste Wert, Schritt der
    groesste gemeinsame Teiler der Abstaende, hoechstens 15 Schritte."""
    werte = [int(v) for v in gruppe]
    b = min(werte)
    a = 0
    for v in werte:
        a = np.gcd(a, v - b)
    return a == 0 or (max(werte) - b) // a <= 15


def test_int4ganz_packt_jede_gruppe():
    """Jede Gruppe von `int4ganz` ist a q + b mit ganzen a, b und q in
    0..15, alle Werte in -127..127; der Fehler bleibt nahe am freien int4.
    Gegenprobe: Das freie int4, auf ganze Zahlen gerundet, packt nicht."""
    W, H, Xh = lage(saat=3)
    W[0, 0, :8] = 127.0
    W[0, 0, 8:16] = -127.0
    _, Q, _ = gv.verfahren("int4ganz", W, H, [Xh] * 2)
    _, Qf, _ = gv.verfahren("int4", W, H, [Xh] * 2)
    for gruppe in Q.reshape(-1, gv.G):
        assert all(float(v) == int(v) for v in gruppe), "keine ganzen Stufen"
        assert max(abs(float(v)) for v in gruppe) <= 127, "ausserhalb int8"
        assert vierbittig(gruppe), f"nicht vierbittig: {sorted({float(v) for v in gruppe})}"
    frei = Qf.round().clamp(-127, 127).reshape(-1, gv.G)
    nicht = sum(not vierbittig(g) for g in frei)
    assert nicht > len(frei) // 2, f"Gegenprobe zu schwach: nur {nicht} von {len(frei)} freien Gruppen packen nicht"
    a = sum(gv.fehler(Xh, W[i], Q[i]) for i in range(2)) / 2
    b = sum(gv.fehler(Xh, W[i], Qf[i]) for i in range(2)) / 2
    assert a < 1.15 * b, f"int4ganz {a:.4f} gegen int4 {b:.4f}"
    print(f"ok int4ganz packt jede Gruppe ({a:.4f} gegen int4 {b:.4f}; frei packen {nicht} von {len(frei)} nicht)")


def test_mehr_stufen_weniger_fehler():
    """Die Reihenfolge, die die Messung am 35B zeigt, gilt auch hier:
    int4 < zwei < ternaer. Gegenprobe: ternaer ist nicht besser als zwei."""
    W, H, Xh = lage(saat=3)
    f = {}
    for v in ("ternaer", "zwei", "int4"):
        Wv, Q, Xv = gv.verfahren(v, W, H, [Xh] * 2)
        f[v] = gv.fehler(Xv[0], Wv[0], Q[0])
    assert f["int4"] < f["zwei"] < f["ternaer"], f
    print(f"ok mehr Stufen, weniger Fehler {', '.join(f'{k} {x:.4f}' for k, x in f.items())}")


def test_drehung_aendert_die_ausgabe_nicht():
    """`had` dreht W und X gemeinsam: Ohne Rundung bleibt X W^T gleich."""
    W, H, Xh = lage(saat=4)
    D = gv.hadamard(W.shape[2]).float()
    soll = Xh @ W[0].T
    ist = (Xh @ D) @ (W[0] @ D).T
    assert float((soll - ist).abs().max()) < 1e-2 * float(soll.abs().max())
    print("ok die Drehung aendert die Ausgabe nicht")


def test_schranke_ohne_buendelung_ist_shannon():
    """Gleich verteilte Eingaenge und gausssche Gewichte: Die ideale
    Codierung erreicht 2^(-2R), und Wasserfuellen verteilt nichts um.
    Gegenprobe: Gebuendelte Eingaenge liegen deutlich darunter."""
    g = np.random.default_rng(7)
    W = g.standard_normal((64, 256))
    X = g.standard_normal((20000, 256))
    H = X.T @ X
    lam, V = np.linalg.eigh(H)
    for R in (1.58, 3.0):
        d = gv.schranke_fehler(W, V, lam, H, R * 256)
        assert abs(d / 2 ** (-2 * R) - 1) < 0.1, (R, d)
    skala = np.exp(-np.arange(256) / 20.0)
    Xb = X * skala
    Hb = Xb.T @ Xb
    lam_b, Vb = np.linalg.eigh(Hb)
    d = gv.schranke_fehler(W, Vb, lam_b, Hb, 1.58 * 256)
    assert d < 0.5 * 2 ** (-2 * 1.58), d
    print(f"ok die Schranke ohne Buendelung ist Shannon, gebuendelt {d:.4f}")


def test_schrumpfen_hilft_wenigen_token():
    """Ein Experte mit 16 Lerntoken: H allein ueberpasst, H + n0 Sigma der
    Ebene rundet auf den Haltetoken besser. Gegenprobe: n0 = 0 ist genau die
    bisherige Daempfung."""
    W, _, _ = lage(E=1, zeilen=32, saat=6)
    g = torch.Generator().manual_seed(6)
    A = torch.randn(256, 256, generator=g) * 0.3 + torch.eye(256)
    X = torch.randn(9000, 256, generator=g) @ A
    lage_x = gv.kovarianz(X[:8000])
    Xk, Xh = X[8000:8016], X[8016:]
    H0 = gv.hesse(Xk)
    alt = Xk.double().T @ Xk.double()
    alt = alt + gv.DAEMPFUNG * torch.diagonal(alt).mean() * torch.eye(256, dtype=torch.float64)
    assert torch.allclose(H0, alt), "n0 = 0 ist nicht mehr die bisherige Daempfung"
    f = {}
    for n0 in (0.0, 8192.0):
        _, Q, _ = gv.verfahren("int4", W, gv.hesse(Xk, lage_x, n0).unsqueeze(0), [])
        f[n0] = gv.fehler(Xh, W[0], Q[0])
    assert f[8192.0] < 0.9 * f[0.0], f
    print(f"ok Schrumpfen hilft wenigen Token ({f[0.0]:.4f} auf {f[8192.0]:.4f})")


def test_h_skala_findet_die_shifts():
    """h = 2^lb silu(2^la g) u mit bekannten Shifts und int8-Gewichten mit
    Zeilenshifts: `h_skala` findet la und lb wieder, mit kleinem Rest.
    Gegenprobe: ein h ohne silu wird abgewiesen."""
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        rng = np.random.default_rng(8)
        hid, inter, n, top = 64, 32, 400, 2
        man = {}
        for j in range(4):
            for m in ("gate_proj", "up_proj"):
                name = f"model_layers_0_mlp_experts_{j}_{m}_weight"
                rng.integers(-127, 128, (inter, hid), dtype=np.int8).tofile(d / f"{name}.bin")
                rng.integers(6, 9, inter, dtype=np.uint8).tofile(d / f"{name}_shifts.bin")
                man[name] = {"file": f"{name}.bin", "shape": [inter, hid], "shifts_file": f"{name}_shifts.bin"}
        x = rng.integers(-3000, 3000, (n, hid)).astype(np.float32)
        wahl = np.stack([np.arange(n) % 4, (np.arange(n) + 1) % 4], 1).astype(np.uint16)
        h = np.zeros((n, top, inter), dtype=np.float32)
        for j in range(4):
            Wg = gv.echt(d, man[f"model_layers_0_mlp_experts_{j}_gate_proj_weight"])
            Wu = gv.echt(d, man[f"model_layers_0_mlp_experts_{j}_up_proj_weight"])
            zj, sj = np.nonzero(wahl == j)
            h[zj, sj] = np.round(gv.h_nachrechnen(torch.from_numpy(x[zj]), Wg, Wu, -12, -3).numpy())
        la, lb, rest = gv.h_skala(d, man, 0, x, wahl, h)
        assert (la, lb) == (-12, -3) and rest < 1e-3, (la, lb, rest)
        try:
            gv.h_skala(d, man, 0, x, wahl, np.abs(h) + 50.0)
            raise AssertionError("ein falsches h wurde nicht abgewiesen")
        except SystemExit:
            pass
    print(f"ok h_skala findet die Shifts (Rest {rest:.6f})")


def test_h_skala_nimmt_nur_experten_mit_token():
    """Fund 574: acht Experten, nur zwei bekommen Token. Vorher rueckten
    Experten ohne Token nach, auch solche, die es nicht gibt (`KeyError`).
    Gegenprobe: ganz ohne Token ein klarer Abbruch statt eines Absturzes."""
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        rng = np.random.default_rng(9)
        hid, inter, n, top = 64, 32, 300, 2
        man = {}
        for j in range(8):
            for m in ("gate_proj", "up_proj"):
                name = f"model_layers_0_mlp_experts_{j}_{m}_weight"
                rng.integers(-127, 128, (inter, hid), dtype=np.int8).tofile(d / f"{name}.bin")
                rng.integers(6, 9, inter, dtype=np.uint8).tofile(d / f"{name}_shifts.bin")
                man[name] = {"file": f"{name}.bin", "shape": [inter, hid], "shifts_file": f"{name}_shifts.bin"}
        x = rng.integers(-3000, 3000, (n, hid)).astype(np.float32)
        wahl = np.tile(np.array([[5, 2]], dtype=np.uint16), (n, 1))
        h = np.zeros((n, top, inter), dtype=np.float32)
        for j in (2, 5):
            Wg = gv.echt(d, man[f"model_layers_0_mlp_experts_{j}_gate_proj_weight"])
            Wu = gv.echt(d, man[f"model_layers_0_mlp_experts_{j}_up_proj_weight"])
            zj, sj = np.nonzero(wahl == j)
            h[zj, sj] = np.round(gv.h_nachrechnen(torch.from_numpy(x[zj]), Wg, Wu, -11, -2).numpy())
        la, lb, rest = gv.h_skala(d, man, 0, x, wahl, h)
        assert (la, lb) == (-11, -2) and rest < 1e-3, (la, lb, rest)
        try:
            gv.h_skala(d, man, 0, x, wahl[:0], h[:0])
            raise AssertionError("eine Ebene ohne Token wurde nicht abgewiesen")
        except SystemExit as f:
            assert "kein Experte hat Token" in str(f), f
    print(f"ok h_skala nimmt nur Experten mit Token (Rest {rest:.6f})")


def test_bauskript_schlaegt_vier_bit_nur_mit_ganzen_gruppen_vor():
    """Fund 574, zweite Haelfte: Das Bauskript bietet `-int4` nur an, wenn
    beide Spaltenzahlen der Experten Vielfache der Gruppe sind. Seine Gruppe
    ist die dieses Werkzeugs; die echten Gemische gehen, das kleine nicht."""
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
    import modell_bauen as mb
    from src import model_configs
    assert mb.VIERBIT_GRUPPE == gv.G, (mb.VIERBIT_GRUPPE, gv.G)
    for name in ("magpie-30b-a3b-int8", "raven-35b-a3b-int8", "vulture-126b-a7b-int8"):
        assert mb.vierbit_hindernis(model_configs.MODEL_CONFIGS[name]) == "", name
    grund = mb.vierbit_hindernis({"hidden_size": 64, "moe_intermediate_size": 32})
    assert "hidden_size 64" in grund, grund
    # Ein Gemisch ohne vier Bit behaelt sein `-a<aktiv>b`.
    assert mb.anhaengsel(125.7, 7.3, True, True) == ["126b-a7b-int4", "126b-a7b-int8"]
    assert mb.anhaengsel(0.001, 0.0003, True, False) == ["0.001b-a1b-int8"]
    assert mb.anhaengsel(0.596, 0.596, False, False) == ["0.6b-int8"]
    print(f"ok das Bauskript schlaegt vier Bit nur mit ganzen Gruppen vor ({grund})")


def test_namen_sind_tier_und_anhaengsel():
    """Ein Name ist ein Wort und das Anhaengsel (2026-10-21). Die `variant`
    im Artefakt bleibt, was sie vor der Umbenennung war, denn sie steht im
    Abdruck; ohne Eingabe waehlt das Skript ein freies Tier."""
    import random
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
    import modell_bauen as mb
    from src import model_configs
    for name, cfg in model_configs.MODEL_CONFIGS.items():
        if name.endswith("-int8"):  # das Skript baut aus int8-Namen
            assert mb.variante(name) == cfg["variant"], (name, mb.variante(name), cfg["variant"])
    katalog = json.loads((Path(__file__).resolve().parents[2] / "MODELS/llm/KATALOG.json").read_text())
    for name in (k for k in katalog if not k.startswith("_")):
        assert mb.NAMENSMUSTER.match(name), name
        assert "myelith" not in name, name
        assert mb.stamm(name) not in mb.TIERE, f"{name}: das Tier eines Projektmodells steht im Zufall"
    assert mb.variante("mein-otter-126b-int8") == "126b"
    assert mb.name_aus("Otter", "126b-a7b-int4") == "otter-126b-a7b-int4"
    assert mb.name_aus("otter-126b-a7b-int8", "126b-a7b-int4") == "otter-126b-a7b-int8", "ein ganzer Name bleibt"
    gezogen = {mb.stamm(mb.name_aus("", "4b-int8", random.Random(s))) for s in range(40)}
    belegt = {mb.stamm(n) for n in mb.vergebene_namen()}
    assert gezogen <= set(mb.TIERE) and not gezogen & belegt and len(gezogen) > 10, gezogen
    assert mb.vierbit_name("raven-35b-a3b-int8") == "raven-35b-a3b-int4"
    assert mb.quelle_des_viers("raven-35b-a3b-int4") == "raven-35b-a3b-int8"
    print(f"ok Namen sind Tier und Anhaengsel ({len(gezogen)} Tiere aus 40 Zuegen)")


def test_bauen_schreibt_nie_in_die_quelle():
    """Ein Ziel mit harten Links in die Quelle (wie nach `abschluss`):
    `bauen` ersetzt die Datei, die Quelle bleibt unberuehrt."""
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        quelle, mit, ziel = d / "quelle", d / "mit", d / "ziel"
        for p in (quelle, mit, ziel):
            p.mkdir()
        zeilen, spalten, n, top, inter = 4, 128, 64, 1, 128
        man = {}
        g = np.random.default_rng(5)
        for m, form in (("gate_proj", (zeilen, spalten)), ("up_proj", (zeilen, spalten)), ("down_proj", (spalten, inter))):
            name = f"model_layers_0_mlp_experts_0_{m}_weight"
            man[name] = {"file": f"{name}.bin", "shape": list(form)}
            g.integers(-127, 128, form, dtype=np.int8).tofile(quelle / f"{name}.bin")
        (quelle / "weights_manifest.json").write_text(json.dumps(man))
        (quelle / "theta_v.json").write_text(json.dumps({"weights_hash": ""}))
        (mit / "kopf.json").write_text(json.dumps({"token": n, "hidden": spalten, "top_k": top,
                                                     "moe_intermediate": inter, "ebenen": [0]}))
        g.integers(-300, 300, (n, spalten), dtype=np.int16).astype("<i2").tofile(mit / "x_0.bin")
        np.zeros((n, top), dtype="<u2").tofile(mit / "wahl_0.bin")
        g.integers(-300, 300, (n, top, inter), dtype=np.int16).astype("<i2").tofile(mit / "h_0.bin")
        (quelle / gv.PRUEFMARKE).write_text("Marke der Quelle")
        gv.abschluss(quelle, ziel)
        assert not (ziel / gv.PRUEFMARKE).exists(), "die Pruefmarke der Quelle wurde mitgenommen (Fund 551)"
        vorher = {f: (quelle / f).read_bytes() for f in os.listdir(quelle)}
        gv.bauen(["bauen", str(quelle), str(mit), str(ziel), "ternaer"])
        for f, b in vorher.items():
            assert (quelle / f).read_bytes() == b, f"die Quelle hat sich geaendert: {f}"
        neu = ziel / man["model_layers_0_mlp_experts_0_gate_proj_weight"]["file"]
        assert neu.stat().st_nlink == 1, "das Ziel teilt noch die Datei der Quelle"
        # Der Abschluss danach traegt die Summen der neuen Dateien ein, im
        # Manifest des Ziels und nicht in dem der Quelle.
        gv.abschluss(quelle, ziel)
        m_ziel = json.loads((ziel / "weights_manifest.json").read_text())
        for name, e in m_ziel.items():
            assert e["hash"] == hashlib.sha256((ziel / e["file"]).read_bytes()).hexdigest(), name
        assert (quelle / "weights_manifest.json").read_bytes() == vorher["weights_manifest.json"]
        theta = json.loads((ziel / "theta_v.json").read_text())
        assert theta["weights_hash"] == hashlib.sha256((ziel / "weights_manifest.json").read_bytes()).hexdigest()
        # --gruppe 32: je 32 Eingaenge hoechstens 16 Stufen,
        # je 64 mehr (sonst waere die Gruppe nicht angekommen); danach gilt
        # wieder die alte Gruppe.
        gv.bauen(["bauen", str(quelle), str(mit), str(ziel), "int4", "--gruppe", "32"])
        assert gv.G == 128, f"die Gruppe des Baus ist haengen geblieben: {gv.G}"
        e = man["model_layers_0_mlp_experts_0_up_proj_weight"]
        q = np.fromfile(ziel / e["file"], dtype=np.int8).reshape(e["shape"])
        st32 = max(len(set(q[r, g:g + 32].tolist())) for r in range(q.shape[0]) for g in range(0, q.shape[1], 32))
        st64 = max(len(set(q[r, g:g + 64].tolist())) for r in range(q.shape[0]) for g in range(0, q.shape[1], 64))
        assert st32 <= 16 < st64, (st32, st64)
    print("ok bauen schreibt nie in die Quelle, der Abschluss traegt die neuen Summen ein, --gruppe kommt an")


if __name__ == "__main__":
    test_ternaer_ist_das_werkzeug()
    test_zweiganz_sind_zwei_ganze_ebenen()
    test_int4ganz_packt_jede_gruppe()
    test_mehr_stufen_weniger_fehler()
    test_drehung_aendert_die_ausgabe_nicht()
    test_schranke_ohne_buendelung_ist_shannon()
    test_schrumpfen_hilft_wenigen_token()
    test_h_skala_findet_die_shifts()
    test_h_skala_nimmt_nur_experten_mit_token()
    test_bauskript_schlaegt_vier_bit_nur_mit_ganzen_gruppen_vor()
    test_namen_sind_tier_und_anhaengsel()
    test_bauen_schreibt_nie_in_die_quelle()
    print("Fertig")
