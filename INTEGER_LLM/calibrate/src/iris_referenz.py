"""
**Die Gleitkomma-Referenz des Sichtturms**, allein geladen.

Laedt aus einem Quellmodell nur die Tensoren unter `model.visual.` (333
Stueck, rund 0,4 Mrd. Gewichte) in die Turmklasse der Vorlage und legt fuer
Probebilder die Zwischenwerte ab: Pixelgitter, Flicken, Flickeneinbettung,
den Strom nach jedem Block und den Ausgang des Projektors. Gegen diese
Werte wird der ganzzahlige Turm gemessen.

⚑ **Der Turm allein und nicht das ganze Modell.** raven sind 67 GB in
voller Genauigkeit, gegen 24 GB Speicher; der Turm sind 1,6 GB in fp32.
Die Klasse ist dieselbe, die das ganze Modell benutzt, und `strict` beim
Laden sagt, dass kein Tensor fehlt und keiner uebrig ist.

⚑ **Das Pixelgitter wird hier festgelegt und nicht von der Vorlage.** Das
Bild wird vorher auf die Groesse gebracht, die `smart_resize` waehlen
wuerde, und als `u8` abgelegt. Der Bildprozessor der Vorlage findet die
Groesse dann schon passend vor und skaliert nicht mehr; geprueft wird das
ueber die Rueckrechnung von `pixel_values` auf ganze Pixel. So beginnt die
Referenz an derselben Stelle wie der ganzzahlige Weg: am Gitter.

Aufruf aus `INTEGER_LLM`:

    calibrate/.venv/bin/python3 -m calibrate.src.iris_referenz \\
        --bild ../README/Grafiken/fig_schichten.png [--bild ...] \\
        [--modell Qwen3.6-35B-A3B] [--hoechstens 262144]
"""

import argparse
import json
import math
from pathlib import Path

import numpy as np
import torch
from PIL import Image
from safetensors import safe_open

from .paths import models_dir, repo_root

# Der Turm nimmt Flicken zu 16 und legt je 2x2 zusammen; Hoehe und Breite
# sind deshalb Vielfache von 32.
RASTER = 32

# ⚑ Vorgabe fuer die Pixelzahl: 512 x 512, also 1024 Flicken und 256
# Bildtoken. Die Vorlage erlaubt bis 16,7 Mio. Pixel; die Achtsamkeit
# ueber alle Flicken waechst quadratisch, und fuer den Anfang reicht das.
HOECHSTENS_PIXEL = 512 * 512
MINDESTENS_PIXEL = 65536


# Bloecke, deren Inneres mitgeschnitten wird.
INNERE_BLOECKE = (0, 5, 26)


def lokal_dir() -> Path:
    return repo_root() / "INTEGER_LLM" / "calibrate" / "lokal" / "iris"


def zielgroesse(hoehe: int, breite: int, hoechstens: int, mindestens: int = MINDESTENS_PIXEL):
    """Dieselbe Regel wie `smart_resize` der Vorlage, ganzzahlig gerechnet.

    Beide Seiten auf Vielfache von 32 gerundet; liegt die Pixelzahl
    darueber oder darunter, wird mit gleichem Seitenverhaeltnis
    verkleinert oder vergroessert und **ab- bzw. aufgerundet**, damit die
    Grenze sicher gilt. Hier steht noch `math.sqrt`; die Regel laeuft im
    Client und nicht im Rechenpfad, und der Client rechnet sie ganzzahlig
    nach (Stufe S1).
    """
    h = max(RASTER, round(hoehe / RASTER) * RASTER)
    w = max(RASTER, round(breite / RASTER) * RASTER)
    if h * w > hoechstens:
        beta = math.sqrt(hoehe * breite / hoechstens)
        h = max(RASTER, math.floor(hoehe / beta / RASTER) * RASTER)
        w = max(RASTER, math.floor(breite / beta / RASTER) * RASTER)
    elif h * w < mindestens:
        beta = math.sqrt(mindestens / (hoehe * breite))
        h = math.ceil(hoehe * beta / RASTER) * RASTER
        w = math.ceil(breite * beta / RASTER) * RASTER
    return h, w


def turm_laden(modellordner: Path):
    """Baut die Turmklasse der Vorlage und laedt die `model.visual.`-Tensoren."""
    from transformers.models.qwen3_5_moe.configuration_qwen3_5_moe import Qwen3_5MoeVisionConfig
    from transformers.models.qwen3_5_moe.modeling_qwen3_5_moe import Qwen3_5MoeVisionModel

    konfig = json.loads((modellordner / "config.json").read_text())
    vk = dict(konfig["vision_config"])
    # ⚑ Dieselbe Klasse fuer alle drei Modelle: Die Tuerme unterscheiden
    # sich nur in `out_hidden_size`, und die steht in `vision_config`.
    vk.pop("model_type", None)
    turmkonfig = Qwen3_5MoeVisionConfig(**vk)
    turmkonfig._attn_implementation = "eager"
    turm = Qwen3_5MoeVisionModel(turmkonfig).float().eval()

    karte = json.loads((modellordner / "model.safetensors.index.json").read_text())["weight_map"]
    vorne = "model.visual."
    je_datei = {}
    for name, datei in karte.items():
        if name.startswith(vorne):
            je_datei.setdefault(datei, []).append(name)
    zustand = {}
    for datei, namen in sorted(je_datei.items()):
        with safe_open(str(modellordner / datei), framework="pt") as f:
            for n in namen:
                zustand[n[len(vorne):]] = f.get_tensor(n).float()
    fehlt, uebrig = turm.load_state_dict(zustand, strict=True)
    assert not fehlt and not uebrig, (fehlt, uebrig)
    return turm, konfig, len(zustand)


def gitter_aus_bild(pfad: Path, hoechstens: int) -> np.ndarray:
    """Das Bild als `u8`-RGB auf der Zielgroesse, mit PIL bikubisch."""
    bild = Image.open(pfad).convert("RGB")
    h, w = zielgroesse(bild.height, bild.width, hoechstens)
    return np.asarray(bild.resize((w, h), Image.BICUBIC), dtype=np.uint8)


def flicken_aus_gitter(gitter: np.ndarray) -> np.ndarray:
    """**Die Flicken, ganzzahlig, in der Reihenfolge der Vorlage.**

    Wert je Pixel und Kanal `2p - 255`; der Realwert ist das durch 255,
    also genau `(p/255 - 0,5) / 0,5`. Je Flicken 1536 Werte in der Folge
    Kanal, Zeitscheibe, Zeile, Spalte; die zwei Zeitscheiben sind gleich.
    Die Flicken laufen in Bloecken zu 2x2: Blockzeile, Blockspalte, dann
    innen Zeile und Spalte.

    Das ist die Festlegung, die der ganzzahlige Vorbau nachbaut; geprueft
    wird sie in `vorverarbeiten` gegen den Bildprozessor der Vorlage.
    """
    h, w, _ = gitter.shape
    ps, m = 16, 2
    gh, gw = h // ps, w // ps
    v = gitter.astype(np.int16) * 2 - 255                       # (H, W, C)
    v = np.transpose(v, (2, 0, 1))                              # (C, H, W)
    v = np.stack([v, v], axis=0)                                # (T=2, C, H, W)
    v = v.reshape(2, 3, gh // m, m, ps, gw // m, m, ps)
    v = np.transpose(v, (2, 5, 3, 6, 1, 0, 4, 7))               # (bh, bw, mh, mw, C, T, ps, ps)
    return v.reshape(gh * gw, 3 * 2 * ps * ps)


def vorverarbeiten(modellordner: Path, gitter: np.ndarray):
    """`pixel_values` und `grid_thw` aus dem Bildprozessor der Vorlage.

    ⚑ Prueft zweierlei: dass er das Gitter nicht mehr skaliert hat (jeder
    Wert liegt auf einem ganzen Pixel), und dass [`flicken_aus_gitter`]
    dieselben Flicken in derselben Reihenfolge ergibt.
    """
    from transformers import AutoImageProcessor

    proz = AutoImageProcessor.from_pretrained(str(modellordner))
    aus = proz(images=[Image.fromarray(gitter)], return_tensors="pt")
    pv = aus["pixel_values"].float()
    thw = aus["image_grid_thw"]
    roh = (pv.double() * 0.5 + 0.5) * 255.0
    abstand = (roh - torch.round(roh)).abs().max().item()
    assert abstand < 1e-3, f"der Prozessor hat das Gitter veraendert (Abstand {abstand})"
    eigen = torch.from_numpy(flicken_aus_gitter(gitter)).double() / 255.0
    abweichung = (eigen - pv.double()).abs().max().item()
    assert abweichung < 1e-6, f"Flicken weichen von der Vorlage ab ({abweichung})"
    return pv, thw


def mitschneiden(turm, pv: torch.Tensor, thw: torch.Tensor) -> dict:
    """Der Vorwaertspass mit den Zwischenwerten je Stufe."""
    werte = {}
    haken = []

    def merke(name):
        def h(_m, _ein, aus):
            werte[name] = aus.detach().clone()
        return h

    haken.append(turm.patch_embed.register_forward_hook(merke("flickeneinbettung")))
    for i, blk in enumerate(turm.blocks):
        haken.append(blk.register_forward_hook(merke(f"block_{i:02d}")))
    haken.append(turm.merger.register_forward_hook(merke("projektor")))
    haken.append(turm.merger.norm.register_forward_hook(merke("p.n")))
    haken.append(turm.merger.linear_fc1.register_forward_hook(merke("p.fc1")))
    haken.append(turm.merger.linear_fc2.register_forward_pre_hook(
        lambda _m, args: werte.__setitem__("p.gelu", args[0].detach().clone())))
    # ⚑ Das Innere einzelner Bloecke, fuer die Fehlersuche Stufe fuer Stufe.
    # Nicht aller: Bei 924 Flicken waeren es rund 2 GB je Bild.
    for i in INNERE_BLOECKE:
        blk = turm.blocks[i]
        p = f"b{i:02d}."
        haken.append(blk.norm1.register_forward_hook(merke(p + "n1")))
        haken.append(blk.attn.qkv.register_forward_hook(merke(p + "qkv")))
        haken.append(blk.attn.proj.register_forward_pre_hook(
            lambda _m, args, p=p: werte.__setitem__(p + "achtsam", args[0].detach().clone())))
        haken.append(blk.attn.register_forward_hook(merke(p + "attn")))
        haken.append(blk.norm2.register_forward_hook(merke(p + "n2")))
        haken.append(blk.mlp.linear_fc1.register_forward_hook(merke(p + "fc1")))
        haken.append(blk.mlp.linear_fc2.register_forward_pre_hook(
            lambda _m, args, p=p: werte.__setitem__(p + "gelu", args[0].detach().clone())))
        haken.append(blk.mlp.register_forward_hook(merke(p + "mlp")))
    # Der Strom vor dem ersten Block (Flicken plus Positions-Einbettung).
    haken.append(turm.blocks[0].register_forward_pre_hook(
        lambda _m, args, kwargs=None: werte.__setitem__("vor_block_00", args[0].detach().clone())
    ))
    with torch.no_grad():
        aus = turm(pv, grid_thw=thw)
    for h in haken:
        h.remove()
    assert torch.equal(aus.pooler_output, werte["projektor"])
    for i in INNERE_BLOECKE:
        p = f"b{i:02d}."
        vor = werte["vor_block_00"] if i == 0 else werte[f"block_{i - 1:02d}"]
        werte[p + "rest_mitte"] = vor + werte[p + "attn"]
    return werte


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--modell", default="Qwen3.6-35B-A3B", help="Ordner unter MODELS/llm")
    ap.add_argument("--bild", action="append", required=True, type=Path)
    ap.add_argument("--hoechstens", type=int, default=HOECHSTENS_PIXEL, help="hoechste Pixelzahl")
    ap.add_argument("--aus", type=Path, default=None)
    a = ap.parse_args()

    torch.manual_seed(0)
    modellordner = models_dir() / a.modell
    turm, konfig, n = turm_laden(modellordner)
    print(f"Turm geladen: {n} Tensoren, Ausgabebreite {konfig['vision_config']['out_hidden_size']}")
    ziel = a.aus or (lokal_dir() / a.modell)
    ziel.mkdir(parents=True, exist_ok=True)
    for bild in a.bild:
        gitter = gitter_aus_bild(bild, a.hoechstens)
        pv, thw = vorverarbeiten(modellordner, gitter)
        werte = mitschneiden(turm, pv, thw)
        name = bild.stem
        np.savez(
            ziel / f"{name}.npz",
            gitter=gitter,
            flicken=flicken_aus_gitter(gitter),
            pixel_values=pv.numpy(),
            grid_thw=thw.numpy(),
            **{k: v.numpy() for k, v in werte.items()},
        )
        p = werte["projektor"]
        print(f"{name}: Gitter {gitter.shape[0]}x{gitter.shape[1]}, grid_thw {thw.tolist()}, "
              f"Flicken {pv.shape[0]}, Bildtoken {p.shape[0]}x{p.shape[1]}, "
              f"|Ausgang| max {p.abs().max():.3f}")
    print("Fertig")


if __name__ == "__main__":
    main()
