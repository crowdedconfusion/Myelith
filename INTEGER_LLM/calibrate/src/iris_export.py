"""
**Export und Kalibrierung des Sichtturms** in ein Teilartefakt `iris/`.

Laedt den Gleitkomma-Turm (`iris_referenz.turm_laden`), misst auf
Kalibrierbildern die Betraege an jeder Stelle, an der der ganzzahlige Turm
eine Skala braucht, legt daraus die Zweierpotenz-Skalen fest, quantisiert
die 333 Tensoren wie die des Decoders und schreibt:

    <artefakt>/iris/weights_manifest.json   Gewichte, Format des Decoders
    <artefakt>/iris/luts.json               GELU (tanh, erf), Drehung
    <artefakt>/iris/iris.json            Masse und Aktivierungsskalen
    <artefakt>/iris/theta_v.json            Abdruck der drei

⚑ **Das Teilartefakt haengt am Decoder, aber baut ihn nicht neu.** Die
Ausgabe des Projektors traegt die Eingangsskala der ersten Decoderebene;
die liest die Laufzeit aus dem Decoder, nicht von hier.

⚑ **Zwei Umformungen beim Export, beide exakt bis auf Gleitkommarundung:**
1. Die Flickeneinbettung ist eine `Conv3d` ueber zwei gleiche Zeitscheiben;
   ihre beiden Kernscheiben werden addiert (1536 -> 768 Eingaenge).
2. Die Normierung `(p/255 - 0,5)/0,5` ist `(2p - 255)/255`. Der Eingang ist
   die ganze Zahl `2p - 255` mit **8 Bruchstellen**, also `(2p - 255)/256`;
   der Rest `256/255` geht in dieselben Gewichte. ⛔️ Nicht `1/255` in die
   Gewichte: Sie wurden so klein, dass die Zeilenskalen an der Obergrenze
   20 anstiessen und ein Drittel der Gewichte unter 8 Stufen lag (gemessen
   am 2026-10-23).

Aufruf aus `INTEGER_LLM`:

    calibrate/.venv/bin/python3 -m calibrate.src.iris_export \\
        --artefakt raven-35b-a3b-int4 --bilder calibrate/lokal/iris/bilder
"""

import argparse
import json
import math
from pathlib import Path

import numpy as np
import torch

from .export import export_theta_v
from .export_weights import export_quantized_weights, hash_file
from .luts import generate_rope_luts, load_nonlinear_spec
from .paths import model_artifacts_dir, models_dir
from .quantize import quantize_bias_int16_per_element, quantize_symmetric_int8_per_channel
from .scales import choose_pow2_shift
from . import iris_referenz as ref

# Bruchstellen des Flickeneingangs `2p - 255` (siehe Kopf, Umformung 2).
FLICKEN_EIN_FRAC = 8

# ⚑ Bruchstellen der Drehtabelle des Turms: 14 statt der 8 des Decoders. Die
# Bilddrehung dreht die volle Kopfbreite; mit 8 Bit lag die Achtsamkeit
# allein dadurch 0,07 % daneben, mit 14 Bit 0,001 % (2026-10-23). Die
# Tabelle gehoert dem Turm, also kostet das nichts.
DREH_FRAC = 14

# Stellen der Drehtabelle je Achse: so viele Flicken darf eine Bildseite
# haben. 256 Flicken sind 4096 Pixel.
DREH_STELLEN = 256


# ⚑ **Die GELU des Turms hat eine eigene Tabelle**, nicht die der SiLU.
# Mit deren Raster (Eingang 1/64, Ausgang 1/256) lag die GELU in Block 0 bei
# 3,9 % relativem Fehler und die MLP-Ausgabe bei 13,9 % (2026-10-23): Im Turm
# liegen die meisten Werte vor der GELU nahe null, und `fc2` summiert 4304
# solcher Fehler. GELU ist ausserhalb von +-8 bis auf 1e-15 die Identitaet
# bzw. null; die Tabelle braucht also nur diesen Bereich, dafuer fein.
GELU_EIN_FRAC = 10
GELU_AUS_FRAC = 12
GELU_BEREICH = (-8192, 8191)


def gelu_tabelle(art: str):
    """GELU als Tabelle: Index `x` in `GELU_BEREICH` steht fuer `x / 2^10`,
    der Eintrag ist `round(gelu * 2^12)`. `art` ist `tanh` oder `erf`."""
    lo, hi = GELU_BEREICH
    ein = 1 << GELU_EIN_FRAC
    aus = 1 << GELU_AUS_FRAC
    tab = []
    for x in range(lo, hi + 1):
        r = x / ein
        if art == "tanh":
            y = 0.5 * r * (1.0 + math.tanh(math.sqrt(2.0 / math.pi) * (r + 0.044715 * r ** 3)))
        else:
            y = 0.5 * r * (1.0 + math.erf(r / math.sqrt(2.0)))
        tab.append(int(round(y * aus)))
    return tab


class Betraege:
    """Groesste Betraege je Messstelle, je Tensor und fuer den
    Residualstrom je Kanal."""

    def __init__(self):
        self.tensor = {}
        self.kanal = {}

    def tensorweise(self, name, t):
        v = float(t.abs().max())
        self.tensor[name] = max(self.tensor.get(name, 0.0), v)

    def kanalweise(self, name, t):
        v = t.reshape(-1, t.shape[-1]).abs().amax(dim=0).double().numpy()
        alt = self.kanal.get(name)
        self.kanal[name] = v if alt is None else np.maximum(alt, v)


def messen(turm, bilder, hoechstens, modellordner):
    """Der Gleitkomma-Turm ueber alle Bilder, mit Haken an jeder Messstelle."""
    b = Betraege()
    haken = []
    tiefe = len(turm.blocks)
    zwischen = {}

    def vor_block(i):
        def h(_m, args):
            b.kanalweise(f"block_{i}.rest_ein", args[0])
            zwischen["rest_ein"] = args[0]
        return h

    def nach_block(i):
        def h(_m, _a, aus):
            if i == tiefe - 1:
                b.kanalweise("rest_ende", aus)
        return h

    def nach(name, kanal=False):
        def h(_m, _a, aus):
            (b.kanalweise if kanal else b.tensorweise)(name, aus)
        return h

    def vor(name):
        def h(_m, args):
            b.tensorweise(name, args[0])
        return h

    for i, blk in enumerate(turm.blocks):
        haken.append(blk.register_forward_pre_hook(vor_block(i)))
        haken.append(blk.register_forward_hook(nach_block(i)))
        haken.append(blk.norm1.register_forward_hook(nach(f"block_{i}.n1")))
        haken.append(blk.norm2.register_forward_hook(nach(f"block_{i}.n2")))
        haken.append(blk.mlp.linear_fc1.register_forward_hook(nach(f"block_{i}.fc1")))
        haken.append(blk.mlp.linear_fc1.register_forward_hook(nach(f"block_{i}.fc1", kanal=True)))
        haken.append(blk.mlp.linear_fc2.register_forward_pre_hook(vor(f"block_{i}.gelu")))

        def qkv(i):
            def h(_m, _a, aus):
                q, k, v = aus.chunk(3, dim=-1)
                b.tensorweise(f"block_{i}.q", q)
                b.tensorweise(f"block_{i}.k", k)
                b.tensorweise(f"block_{i}.v", v)
            return h
        haken.append(blk.attn.qkv.register_forward_hook(qkv(i)))

        def attn(i):
            def h(_m, _a, aus):
                b.kanalweise(f"block_{i}.attn", aus)
                b.kanalweise(f"block_{i}.rest_mitte", zwischen["rest_ein"] + aus)
            return h
        haken.append(blk.attn.register_forward_hook(attn(i)))
        haken.append(blk.mlp.register_forward_hook(nach(f"block_{i}.mlp", kanal=True)))

    haken.append(turm.merger.norm.register_forward_hook(nach("projektor.n")))
    haken.append(turm.merger.linear_fc1.register_forward_hook(nach("projektor.fc1")))
    haken.append(turm.merger.linear_fc1.register_forward_hook(nach("projektor.fc1", kanal=True)))
    haken.append(turm.merger.linear_fc2.register_forward_pre_hook(vor("projektor.gelu")))
    haken.append(turm.merger.register_forward_hook(nach("projektor.aus")))
    haken.append(turm.merger.register_forward_hook(nach("projektor.aus", kanal=True)))
    haken.append(turm.patch_embed.register_forward_hook(nach("flicken.aus")))

    # ⛔️ **Das Produkt jeder linearen Schicht ohne Bias**, je Kanal. Wo
    # Produkt und Bias sich fast aufheben, passt die Summe auf eine feine
    # Skala, das Produkt allein aber nicht; es klemmte dort, und `fc2` in
    # Block 0 lag auf demselben Eingang 16 % neben der Gleitkommarechnung,
    # 99 % davon in fuenf Kanaelen (gemessen am 2026-10-23).
    def roh(name):
        def h(m, _a, aus):
            b.kanalweise(f"{name}.roh", aus - m.bias)
        return h

    for name, modul in turm.named_modules():
        if isinstance(modul, torch.nn.Linear) and modul.bias is not None:
            haken.append(modul.register_forward_hook(roh(name)))

    def flicken_roh(m, _a, aus):
        b.kanalweise("patch_embed.proj.roh", aus - m.proj.bias)
    haken.append(turm.patch_embed.register_forward_hook(flicken_roh))

    # Die Drehung ist eine freie Funktion; q und k danach misst ein Mantel.
    import transformers.models.qwen3_5_moe.modeling_qwen3_5_moe as mod
    drehen = mod.apply_rotary_pos_emb_vision
    zaehler = {"i": 0}

    def gedreht(q, k, cos, sin):
        qd, kd = drehen(q, k, cos, sin)
        i = zaehler["i"] % tiefe
        zaehler["i"] += 1
        b.tensorweise(f"block_{i}.q_gedreht", qd)
        b.tensorweise(f"block_{i}.k_gedreht", kd)
        return qd, kd

    mod.apply_rotary_pos_emb_vision = gedreht
    try:
        for n, pfad in enumerate(bilder):
            gitter = ref.gitter_aus_bild(pfad, hoechstens)
            pv, thw = ref.vorverarbeiten(modellordner, gitter)
            with torch.no_grad():
                turm(pv, grid_thw=thw)
            if (n + 1) % 20 == 0:
                print(f"  {n + 1} von {len(bilder)} Bildern")
    finally:
        mod.apply_rotary_pos_emb_vision = drehen
        for h in haken:
            h.remove()
    return b


def skalen_festlegen(b: Betraege, tiefe: int, spec: dict):
    """Die Aktivierungsskalen aus den gemessenen Betraegen.

    ⚑ **Eingang der GELU je Kanal, Ausgang je Tensor, beide gedeckelt**:
    hoechstens das Raster der Tabelle (Eingang) und ihre Aufloesung
    (Ausgang); feiner braechte nichts. 📌 Groeber muss es gehen: Die letzte
    Ebene erreicht vor der GELU 518 (gemessen am 2026-10-23), und je Kanal,
    weil wenige grosse Kanaele sonst allen anderen das Raster verderben. Ein
    grober Eingang wird fuer die Tabelle nach links geschoben; was ueber der
    Tabelle liegt, geht ueber die Identitaet.
    """
    kanal = lambda name: [int(choose_pow2_shift(float(v))) for v in b.kanal[name]]
    tensor = lambda name: int(choose_pow2_shift(b.tensor[name]))
    gelu_ein = lambda name: [min(GELU_EIN_FRAC, f) for f in kanal(name)]
    gelu_aus = lambda name: min(GELU_AUS_FRAC, tensor(name))
    bloecke = []
    for i in range(tiefe):
        bloecke.append({
            "rest_ein": kanal(f"block_{i}.rest_ein"),
            "n1": tensor(f"block_{i}.n1"),
            # Die Skala von q und k gilt vor und nach der Drehung; massgeblich
            # ist der groessere der beiden Betraege.
            "q": int(choose_pow2_shift(max(b.tensor[f"block_{i}.q"], b.tensor[f"block_{i}.q_gedreht"]))),
            "k": int(choose_pow2_shift(max(b.tensor[f"block_{i}.k"], b.tensor[f"block_{i}.k_gedreht"]))),
            "v": tensor(f"block_{i}.v"),
            # ⛔️ **Die Zweige haben eigene Skalen**, nicht die der Summe. In
            # den grossen Kanaelen heben sich Residuum und Zweig oft fast auf
            # (+500 und -490); die Summe ist klein, ihre Skala fein, und der
            # Zweig allein klemmte darauf. Gemessen am 2026-10-23: Block 5 bei
            # 45 % relativem Fehler, einzelne Flicken mit negativem Kosinus.
            "attn": kanal(f"block_{i}.attn"),
            "rest_mitte": kanal(f"block_{i}.rest_mitte"),
            "mlp": kanal(f"block_{i}.mlp"),
            "n2": tensor(f"block_{i}.n2"),
            "fc1": gelu_ein(f"block_{i}.fc1"),
            "gelu": gelu_aus(f"block_{i}.gelu"),
        })
    projektor = {
        "rest_ende": kanal("rest_ende"),
        "n": tensor("projektor.n"),
        "fc1": gelu_ein("projektor.fc1"),
        "gelu": gelu_aus("projektor.gelu"),
        # ⚑ Eigene Skala der Bildtoken, je Kanal. Die Eingangsskala der
        # ersten Decoderebene ist an Text kalibriert und traegt nur +-0,06;
        # Bildtoken erreichen 3,4 (gemessen am 2026-10-23). Auf die Skala
        # des Decoders bringt sie erst die Einspeisung.
        "aus": kanal("projektor.aus"),
    }
    # Eine Skala von null heisst: Der Betrag reicht bis an i16 ohne
    # Bruchstelle. Darueber saettigt es, und das soll laut sein.
    warnungen = [f"{k} {v:.1f}" for k, v in b.tensor.items() if v > 32767]
    return bloecke, projektor, warnungen


def gewichte_quantisieren(turm):
    """Alle Tensoren des Turms, in der Form, die die Laufzeit liest."""
    erg = {}
    for name, p in turm.state_dict().items():
        p = p.detach().float()
        if name == "patch_embed.proj.weight":
            # (1152, 3, 2, 16, 16): Zeitscheiben addiert, 256/255 eingerechnet;
            # der Eingang traegt FLICKEN_EIN_FRAC Bruchstellen.
            w = p.sum(dim=2).reshape(p.shape[0], -1) * (256.0 / 255.0)
            erg["patch_embed.proj.weight"] = quantize_symmetric_int8_per_channel(w)
        elif name == "pos_embed.weight":
            # Transponiert, Skala je Kanal: (2304, 1152) -> (1152, 2304).
            erg[name] = quantize_symmetric_int8_per_channel(p.T.contiguous())
        elif name.endswith(".bias"):
            erg[name] = quantize_bias_int16_per_element(p)
        else:
            erg[name] = quantize_symmetric_int8_per_channel(p)
    return erg


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--modell", default="Qwen3.6-35B-A3B", help="Ordner unter MODELS/llm")
    ap.add_argument("--artefakt", required=True, help="Artefakt, neben dessen Decoder der Turm liegt")
    ap.add_argument("--bilder", required=True, type=Path, help="Ordner mit Kalibrierbildern")
    ap.add_argument("--hoechstens", type=int, default=ref.HOECHSTENS_PIXEL)
    a = ap.parse_args()

    spec = load_nonlinear_spec()
    modellordner = models_dir() / a.modell
    turm, konfig, n = ref.turm_laden(modellordner)
    vk = konfig["vision_config"]
    bilder = sorted(p for p in a.bilder.iterdir() if p.suffix.lower() in (".png", ".jpg", ".jpeg"))
    print(f"Turm: {n} Tensoren; {len(bilder)} Kalibrierbilder")

    b = messen(turm, bilder, a.hoechstens, modellordner)
    bloecke, projektor, warnungen = skalen_festlegen(b, len(turm.blocks), spec)
    for w in warnungen:
        print(f"⚠️ saettigt: {w}")

    ziel = model_artifacts_dir(a.artefakt) / "iris"
    ziel.mkdir(parents=True, exist_ok=True)
    export_quantized_weights(gewichte_quantisieren(turm), ziel)

    drehbasis = float((turm.config.rope_parameters or {}).get("rope_theta", 10000.0))
    kopf = vk["hidden_size"] // vk["num_heads"]
    sin, cos = generate_rope_luts(DREH_STELLEN, kopf // 2, drehbasis, DREH_FRAC)
    luts = {
        "gelu_tanh": gelu_tabelle("tanh"),
        "gelu_erf": gelu_tabelle("erf"),
        "dreh_cos": cos,
        "dreh_sin": sin,
    }
    masse = {
        "quelle": a.modell,
        "tiefe": vk["depth"],
        "breite": vk["hidden_size"],
        "mlp": vk["intermediate_size"],
        "koepfe": vk["num_heads"],
        "kopf": kopf,
        "flicken": vk["patch_size"],
        "flicken_ein_frac": FLICKEN_EIN_FRAC,
        "zusammen": vk["spatial_merge_size"],
        "positionsseite": int(math.isqrt(vk["num_position_embeddings"])),
        "ausgabe": vk["out_hidden_size"],
        "gelu_ein_frac": GELU_EIN_FRAC,
        "gelu_aus_frac": GELU_AUS_FRAC,
        "gelu_versatz": -GELU_BEREICH[0],
        "drehbasis": drehbasis,
        "drehstellen": DREH_STELLEN,
        "dreh_frac": DREH_FRAC,
        "drehfrequenzen": kopf // 4,
        # LayerNorm-Epsilon der Vorlage, 1e-6, in Q40 (wie Fund 419).
        "eps_q40": int(round(1e-6 * 2 ** 40)),
        "bildtoken": konfig.get("image_token_id"),
        "bild_anfang": konfig.get("vision_start_token_id"),
        "bild_ende": konfig.get("vision_end_token_id"),
        "hoechstens_pixel": a.hoechstens,
        "kalibrierbilder": len(bilder),
        "skalen": {"bloecke": bloecke, "projektor": projektor},
        "roh": {k[: -len(".roh")]: [int(choose_pow2_shift(float(x))) for x in v] for k, v in sorted(b.kanal.items()) if k.endswith(".roh")},
        "betraege": {k: round(v, 4) for k, v in sorted(b.tensor.items())},
    }
    (ziel / "iris.json").write_text(json.dumps(masse, ensure_ascii=False, indent=1, sort_keys=True))
    export_theta_v({"iris": hash_file(ziel / "iris.json")}, luts, ziel)
    print(f"Fertig: {ziel}")


if __name__ == "__main__":
    main()
