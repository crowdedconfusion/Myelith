"""
**Kalibrierung und Export Ebene fuer Ebene**, fuer die Bauart mit vier
Stroemen (`qwen4_exp`, Qwen3.8-Flash-Next; seit 2026-10-19).

# ⚑ Warum ein eigener Weg

`main.py` laedt das ganze Modell und schickt jede Kalibriersequenz einmal
hindurch. Bei einem eingeblendeten Modell, das groesser ist als der
Arbeitsspeicher, liest **jeder** Durchgang praktisch alle Gewichte von der
Platte: beim 30B 57 GiB mal rund siebzig. Flash-Next hat 360 GB in BF16;
siebzig Durchgaenge waeren ueber 25 TB.

Hier ist es umgekehrt: **Der Zustand aller Sequenzen bleibt im Speicher, und
die Gewichte kommen einmal.** Ebene `i` wird aus den safetensors geladen, alle
Sequenzen laufen durch sie hindurch (mit den Haken der Statistik), dann wird
sie quantisiert, geschrieben und freigegeben.

⚑ **Gerechnet wird mit den Modulen der Vorlage** (`transformers`, Modul
`qwen4_exp`, ab 5.18.0), jedes einzeln. Was `Qwen4ExpTextModel.forward` um die
Ebenen herum tut (Einbettung, Vervierfachung der Stroeme, Drehtabellen,
Masken, Strommischer am Ende), steht hier nachgebaut und ist gegen den ganzen
Vorwaertspass der Vorlage geprueft (`tests/test_flashnext_mini.py`).

⚠️ **Die Tabelle der N-Gramm-Einbettung wird nie ganz geladen.** Sie hat
320 Millionen Zeilen (51 Mrd. Parameter, 102 GB in BF16). Die Kalibrierung
liest nur die Zeilen, die ihre Token treffen; der Export laeuft in Bloecken.
"""

import gc
import hashlib
import json
import os
import shutil
import struct
from pathlib import Path

import numpy as np
import torch

from .quantize import (normiere_namen, quantisiere_parameter,
                       quantize_symmetric_int8_per_channel,
                       quantize_symmetric_int16_per_channel)
from .scales import compute_scales_from_stats, choose_pow2_shift
from .stats import ActivationStatsCollector
from .export_weights import export_quantized_weights, export_lm_head, hash_file
from .export import export_theta_v
from .luts import luts_erzeugen

# Die beiden Namensformen der Vorlage: als `ForConditionalGeneration` (der
# echte Checkpoint) und als `ForCausalLM` (das Probemodell).
_PRAEFIXE = ("model.language_model.", "model.")

FAMILIE = "qwen4-exp"


# ============================================================================
# Die Konfiguration
# ============================================================================

def text_konfiguration_lesen(model_dir: Path) -> dict:
    """`text_config` aus `config.json`, oder die Datei selbst, wenn sie schon
    eine Textkonfiguration ist."""
    roh = json.loads((Path(model_dir) / "config.json").read_text(encoding="utf-8"))
    return roh.get("text_config", roh)


def export_konfiguration(text: dict, variant: str, verified: str, hf_model_id: str) -> dict:
    """**Die Exportkonfiguration aus der Textkonfiguration der Vorlage.**

    ⚑ Jedes Feld wird gelesen, nicht vorausgesetzt; was fehlt, ist ein
    Fehler. Felder, die das Artefakt anders zaehlt als die Quelle, tragen ihren
    Grund daneben.
    """
    def feld(name):
        if name not in text:
            raise ValueError(f"config.json: Feld `{name}` fehlt; die Bauart qwen4_exp braucht es")
        return text[name]

    head_dim = feld("head_dim")
    rope = text.get("rope_parameters") or text.get("rope_scaling") or {}
    anteil = rope.get("partial_rotary_factor", text.get("partial_rotary_factor", 1.0))
    drehbreite = int(head_dim * anteil)
    eos = feld("eos_token_id")
    eos = eos[0] if isinstance(eos, list) else eos
    # ⚠️ `ple_layer_ids` zaehlt 1-basiert (`layer_idx + 1`): `[2]` ist die
    #   zweite Ebene, im Checkpoint `layers.1`. Das Artefakt zaehlt wie die
    #   Ebenen selbst, also ab null.
    ple_ebenen = [i - 1 for i in (text.get("ple_layer_ids") or [])]
    cfg = {
        "family": FAMILIE,
        "variant": variant,
        "num_layers": feld("num_hidden_layers"),
        "hidden_size": feld("hidden_size"),
        # Wie beim 35B: Jede Ebene ist ein Gemisch, eine dichte Breite gibt
        # es nicht.
        "intermediate_size": 0,
        "num_heads": feld("num_attention_heads"),
        "num_kv_heads": feld("num_key_value_heads"),
        "head_dim": head_dim,
        "vocab_size": feld("vocab_size"),
        "max_context": feld("max_position_embeddings"),
        "tie_word_embeddings": bool(text.get("tie_word_embeddings", False)),
        "attention_bias": bool(feld("attention_bias")),
        "qk_norm": True,
        "rms_norm_eps": feld("rms_norm_eps"),
        "num_experts": feld("num_experts"),
        "num_experts_per_tok": feld("num_experts_per_tok"),
        "moe_intermediate_size": feld("moe_intermediate_size"),
        # Fehlt in der Quelle; die Vorgabe der Vorlage ist True
        # (`norm_topk_prob: bool = True`), und der Router normiert danach.
        "norm_topk_prob": bool(text.get("norm_topk_prob", True)),
        "mlp_only_layers": [],
        "shared_expert_intermediate_size": feld("shared_expert_intermediate_size"),
        "experts_packed": False,
        # ⚠️ **`indexed_attention` heisst hier `full_attention`.** Die Vorlage
        #   benennt die Aufmerksamkeitsebenen beim Speichern um, das echte
        #   `config.json` sagt `full_attention`. Die Blockauswahl steht als
        #   Merkmal daneben (`blockauswahl`), die Ebenenart bleibt dieselbe.
        "layer_types": ["full_attention" if t == "indexed_attention" else t
                        for t in feld("layer_types")],
        "linear_num_key_heads": feld("linear_num_key_heads"),
        "linear_num_value_heads": feld("linear_num_value_heads"),
        "linear_key_head_dim": feld("linear_key_head_dim"),
        "linear_value_head_dim": feld("linear_value_head_dim"),
        "linear_conv_kernel_dim": feld("linear_conv_kernel_dim"),
        "output_gate_type": feld("output_gate_type"),
        # ⚑ **Das Tor der Norm im Gated DeltaNet** ist bei dieser Bauart
        #   dieselbe Funktion wie `output_gate_type` (`RMSNormGated(...,
        #   activation=output_gate_type or hidden_act)`), beim 35B dagegen
        #   immer `silu`. Ein eigenes Feld, weil es die Rechnung aendert.
        "zustandstor": feld("output_gate_type") or feld("hidden_act"),
        # MTP wird nicht exportiert (wie beim 35B, Fund 416).
        "mtp_num_hidden_layers": 0,
        "rotary_dim": drehbreite,
        "partial_rotary_factor": anteil,
        "rope_theta": float(rope.get("rope_theta", text.get("rope_theta", 1e6))),
        # Fuer reinen Text ohne Wirkung; hier, damit der Feldabgleich sie
        # gegen die Quelle haelt.
        "mrope_section": list(rope.get("mrope_section", [])),
        "hidden_act": feld("hidden_act"),
        "rms_norm_offset": 1.0,
        # --- Die vier Stroeme
        "hc_count": feld("hc_count"),
        "hc_lowrank": feld("hc_lowrank"),
        # --- Die N-Gramm-Einbettung
        "ple_ebenen": ple_ebenen,
        "ple_embed_dim": text.get("ple_embed_dim") or feld("hidden_size"),
        "ple_conv_kernel_size": feld("ple_conv_kernel_size"),
        "ngram_size": feld("ngram_size"),
        "heads_per_ngram": feld("heads_per_ngram"),
        "ngram_vocab_size_base": feld("ngram_vocab_size_base"),
        "make_ngram_vocab_size_divisible_by": feld("make_ngram_vocab_size_divisible_by"),
        "ngram_seed": text.get("seed", 1234),
        "eos_token_id": eos,
        # --- Die Blockauswahl
        "indexer_n_heads": feld("indexer_n_heads"),
        "indexer_kv_heads": feld("indexer_kv_heads"),
        "indexer_head_dim": feld("indexer_head_dim"),
        "indexer_budget": feld("indexer_budget"),
        "indexer_compress_ratio": feld("indexer_compress_ratio"),
        "verified": verified,
        "hf_model_id": hf_model_id,
    }
    # ⚑ Nur beschreibend, die Ebenenfolge steht in `layer_types`; uebernommen,
    #   wenn die Quelle es fuehrt (ein mit `save_pretrained` gespeichertes
    #   Modell mit ausdruecklichen `layer_types` laesst es weg).
    if "full_attention_interval" in text:
        cfg["full_attention_interval"] = text["full_attention_interval"]
    if cfg["hc_count"] & (cfg["hc_count"] - 1):
        raise ValueError(f"hc_count {cfg['hc_count']} ist keine Zweierpotenz; die Laufzeit teilt per Shift")
    if cfg["indexer_kv_heads"] != 1:
        raise ValueError("die Blockauswahl kennt genau einen Schluesselkopf (indexer_kv_heads = 1)")
    return cfg


# ============================================================================
# Gewichte aus den safetensors
# ============================================================================

_DTYPES = {"BF16": (np.uint16, torch.bfloat16), "F16": (np.float16, None),
           "F32": (np.float32, None), "I64": (np.int64, None)}


class Gewichtsquelle:
    """**Die Gewichte eines Snapshots, nach Namen**, ohne das Modell zu bauen.

    Liest `model.safetensors.index.json` oder eine einzelne
    `model.safetensors`; die Koepfe der Dateien werden einmal gelesen, die
    Daten erst, wenn ein Tensor gebraucht wird (eingeblendet ueber `numpy`).
    """

    def __init__(self, model_dir: Path):
        self.dir = Path(model_dir)
        index = self.dir / "model.safetensors.index.json"
        if index.exists():
            karte = json.loads(index.read_text(encoding="utf-8"))["weight_map"]
            dateien = sorted(set(karte.values()))
        else:
            dateien = ["model.safetensors"]
        self.eintraege = {}
        for datei in dateien:
            pfad = self.dir / datei
            with open(pfad, "rb") as f:
                laenge = struct.unpack("<Q", f.read(8))[0]
                kopf = json.loads(f.read(laenge))
            for name, e in kopf.items():
                if name == "__metadata__":
                    continue
                self.eintraege[name] = (pfad, 8 + laenge, e)

    def namen(self):
        return self.eintraege.keys()

    def voller_name(self, kurz: str) -> str:
        """Der Name im Checkpoint zu einem Namen ohne Praefix (`layers.3.…`)."""
        for p in _PRAEFIXE:
            if p + kurz in self.eintraege:
                return p + kurz
        if kurz in self.eintraege:
            return kurz
        raise KeyError(f"Gewicht `{kurz}` steht nicht im Checkpoint (gesucht mit {_PRAEFIXE})")

    def abbild(self, name: str) -> np.ndarray:
        """Der Tensor als eingeblendetes `numpy`-Feld, ohne zu kopieren."""
        pfad, start, e = self.eintraege[name]
        np_typ, _ = _DTYPES[e["dtype"]]
        a, b = e["data_offsets"]
        return np.memmap(pfad, dtype=np_typ, mode="r", offset=start + a, shape=tuple(e["shape"]))

    def tensor(self, name: str, dtype=torch.float32) -> torch.Tensor:
        """Der Tensor, kopiert und auf `dtype`."""
        pfad, start, e = self.eintraege[name]
        roh = np.array(self.abbild(name))
        _, torch_typ = _DTYPES[e["dtype"]]
        t = torch.from_numpy(roh)
        if torch_typ is not None:
            t = t.view(torch_typ)
        return t.to(dtype) if t.is_floating_point() else t

    def mit_praefix(self, kurz_praefix: str):
        """Alle Namen unter `layers.<i>.` (ohne Modellpraefix), als
        `(kurz, voll)`."""
        for voll in self.eintraege:
            for p in _PRAEFIXE:
                if voll.startswith(p + kurz_praefix):
                    yield voll[len(p):], voll
                    break


class Tabellenteile(torch.nn.Module):
    """**Die N-Gramm-Tabelle als Nachschlagen in ihren Teilen**, ohne sie zu
    laden.

    Der Checkpoint fuehrt sie in Teilen gleicher Zeilenzahl
    (`ngram_embedding.shard_<k>.weight`); Zeile `z` liegt in Teil
    `z // zeilen_je_teil`. Die Vorlage setzt sie beim Laden zu einer Tabelle
    zusammen; hier wird je Aufruf nur gelesen, was die Token treffen.
    """

    def __init__(self, quelle: Gewichtsquelle, praefix_voll: str, dtype=torch.float32):
        super().__init__()
        self.teile = []
        k = 0
        while f"{praefix_voll}shard_{k}.weight" in quelle.eintraege:
            self.teile.append(quelle.abbild(f"{praefix_voll}shard_{k}.weight"))
            k += 1
        if not self.teile:
            ganz = f"{praefix_voll}weight"
            if ganz not in quelle.eintraege:
                raise KeyError(f"keine N-Gramm-Tabelle unter `{praefix_voll}`")
            self.teile = [quelle.abbild(ganz)]
        self.zeilen_je_teil = self.teile[0].shape[0]
        if any(t.shape[0] != self.zeilen_je_teil for t in self.teile[:-1]):
            raise ValueError("die Teile der N-Gramm-Tabelle sind verschieden lang")
        self.breite = self.teile[0].shape[1]
        self.bf16 = self.teile[0].dtype == np.uint16
        self.dtype = dtype
        # ⚠️ Die Vorlage fragt `ngram_embedding.weight.device`, um zu wissen,
        #   wohin sie die Kennungen schickt. Ein leerer Puffer ohne Gewicht
        #   beantwortet das, ohne als Parameter mitexportiert zu werden.
        self.register_buffer("weight", torch.empty(0, dtype=dtype), persistent=False)

    @property
    def zeilen(self) -> int:
        return sum(t.shape[0] for t in self.teile)

    def zeilenblock(self, von: int, bis: int) -> torch.Tensor:
        """Zeilen `[von, bis)` am Stueck, fuer den Export."""
        stuecke = []
        while von < bis:
            teil, rest = divmod(von, self.zeilen_je_teil)
            ende = min(bis, (teil + 1) * self.zeilen_je_teil)
            roh = np.array(self.teile[teil][rest:rest + (ende - von)])
            stuecke.append(self._torch(roh))
            von = ende
        return torch.cat(stuecke, 0)

    def _torch(self, roh: np.ndarray) -> torch.Tensor:
        t = torch.from_numpy(roh)
        return (t.view(torch.bfloat16) if self.bf16 else t).to(self.dtype)

    def forward(self, ids: torch.Tensor) -> torch.Tensor:
        flach = ids.reshape(-1).cpu().numpy()
        zeilen = torch.empty((flach.shape[0], self.breite), dtype=self.dtype)
        teil = flach // self.zeilen_je_teil
        for k in np.unique(teil):
            maske = teil == k
            roh = np.array(self.teile[k][flach[maske] - k * self.zeilen_je_teil])
            zeilen[torch.from_numpy(np.nonzero(maske)[0])] = self._torch(roh)
        return zeilen.reshape(*ids.shape, self.breite).to(ids.device)


def _module_der_vorlage():
    try:
        from transformers.models.qwen4_exp import configuration_qwen4_exp as k
        from transformers.models.qwen4_exp import modeling_qwen4_exp as m
    except ImportError as e:
        import transformers
        raise ImportError(
            f"transformers {transformers.__version__} kennt `qwen4_exp` nicht; die Bauart "
            "kam mit 5.18.0. Fuer Flash-Next eine Umgebung mit transformers >= 5.18.0 nehmen."
        ) from e
    return k, m


def vorlagen_konfiguration(text: dict):
    k, _ = _module_der_vorlage()
    felder = {n: v for n, v in text.items() if n not in ("model_type", "dtype", "torch_dtype")}
    cfg = k.Qwen4ExpTextConfig(**felder)
    # Die Indexmaske braucht eine 4D-Maske, wie sie `eager` liefert.
    cfg._attn_implementation = "eager"
    return cfg


def ebene_laden(cfg_vorlage, i: int, quelle: Gewichtsquelle, geraet, dtype):
    """**Ebene `i` mit ihren Gewichten**, gebaut ohne Speicher (`meta`) und dann
    befuellt. Die N-Gramm-Tabelle wird durch [`Tabellenteile`] ersetzt."""
    _, m = _module_der_vorlage()
    with torch.device("meta"):
        ebene = m.Qwen4ExpTextDecoderLayer(cfg_vorlage, i)
    zustand = {}
    tabelle = None
    for kurz, voll in quelle.mit_praefix(f"layers.{i}."):
        rest = kurz[len(f"layers.{i}."):]
        if ".ngram_embedding." in f".{rest}":
            continue
        zustand[rest] = quelle.tensor(voll, dtype)
    # ⚑ **Experten in beiden Formen.** Der echte Checkpoint fuehrt sie
    #   verschmolzen (`experts.gate_up_proj`, `[E, 2*zw, breite]`), ein mit
    #   `save_pretrained` gespeichertes Modell einzeln (`experts.<e>.gate_proj`
    #   usw.). Das Modul der Vorlage will die verschmolzene Form: gate und up
    #   hintereinander, wie `entschmelze_experten` sie wieder trennt.
    if "mlp.experts.0.gate_proj.weight" in zustand:
        e_n = ebene.mlp.experts.num_experts
        zustand["mlp.experts.gate_up_proj"] = torch.stack([
            torch.cat([zustand.pop(f"mlp.experts.{e}.gate_proj.weight"),
                       zustand.pop(f"mlp.experts.{e}.up_proj.weight")], 0) for e in range(e_n)])
        zustand["mlp.experts.down_proj"] = torch.stack([
            zustand.pop(f"mlp.experts.{e}.down_proj.weight") for e in range(e_n)])
    if ebene.ple is not None:
        # Der volle Praefix der Tabelle (mit Modellpraefix).
        probe = quelle.voller_name(f"layers.{i}.ple.ple_embedding.layer_multipliers")
        teile_praefix = probe[: -len("layer_multipliers")] + "ngram_embedding."
        tabelle = Tabellenteile(quelle, teile_praefix, dtype)
        ebene.ple.ple_embedding.ngram_embedding = tabelle
    fehlend, zuviel = ebene.load_state_dict(zustand, strict=False, assign=True)
    fehlend = [n for n in fehlend if ".ngram_embedding." not in f".{n}"]
    if fehlend or zuviel:
        raise RuntimeError(f"Ebene {i}: fehlend {fehlend}, unerwartet {zuviel}")
    # Puffer, die nicht im Checkpoint stehen (keine bei dieser Bauart), stehen
    # noch auf `meta`; dann ist etwas faul.
    for n, b in ebene.named_buffers():
        if b.is_meta:
            raise RuntimeError(f"Ebene {i}: Puffer `{n}` steht nicht im Checkpoint")
    return ebene.to(geraet).eval(), tabelle


# ============================================================================
# Der Durchgang
# ============================================================================

def _masken(cfg_vorlage, eingabe: torch.Tensor):
    from transformers.masking_utils import create_causal_mask
    laenge = eingabe.shape[1]
    pos = torch.arange(laenge, device=eingabe.device)
    maske = create_causal_mask(config=cfg_vorlage, inputs_embeds=eingabe, attention_mask=None,
                               past_key_values=None, position_ids=pos.view(1, -1),
                               allow_is_causal_skip=False)
    return maske


class Kalibrierlauf:
    """**Die Sequenzen und ihr Zustand ueber die Ebenen.**"""

    def __init__(self, model_dir, sequenzen, geraet="cpu", dtype=torch.float32):
        _, m = _module_der_vorlage()
        self.quelle = Gewichtsquelle(model_dir)
        self.text = text_konfiguration_lesen(model_dir)
        self.cfg = vorlagen_konfiguration(self.text)
        self.geraet = torch.device(geraet)
        self.dtype = dtype
        self.ids = [torch.tensor([s], device=self.geraet) for s in sequenzen]
        self.drehung = m.Qwen4ExpTextRotaryEmbedding(self.cfg).to(self.geraet)
        einbettung = self.quelle.tensor(self.quelle.voller_name("embed_tokens.weight"), dtype).to(self.geraet)
        hc = self.cfg.hc_count
        self.zustand = []
        self.lagen = []
        for ids in self.ids:
            e = einbettung[ids]
            laenge = ids.shape[1]
            pos = torch.arange(laenge, device=self.geraet).view(1, 1, -1).expand(3, 1, -1)
            self.lagen.append({
                "position_embeddings": self.drehung(e, pos),
                "attention_mask": _masken(self.cfg, e),
                "conv_mask": None,
                "ple_input_ids": ids,
            })
            self.zustand.append(e.repeat(1, 1, hc))
        del einbettung

    @torch.no_grad()
    def ebene(self, ebene_modul):
        for k, lage in enumerate(self.lagen):
            self.zustand[k] = ebene_modul(self.zustand[k], past_key_values=None, **lage)

    @torch.no_grad()
    def strommischer(self, mischer):
        return [mischer(x) for x in self.zustand]


def _indexskalen_teilen(stats: dict, scales: dict, cfg: dict):
    """**Abfrage und Schluessel der Indexprojektion getrennt skalieren.**

    Die Projektion liefert beide in einem Tensor; sie werden danach getrennt
    normiert und duerfen verschieden gross sein (dieselbe Lage wie Fund 421).
    """
    q_breite = cfg["indexer_n_heads"] * cfg["indexer_head_dim"]
    for name, s in stats.items():
        if not name.endswith("indexer.index_qk_proj") or s.get("channel_absmax") is None:
            continue
        kanal = list(s["channel_absmax"])
        for teil, werte in (("q", kanal[:q_breite]), ("k", kanal[q_breite:])):
            absmax = max(werte) if werte else 0.0
            schiebung = choose_pow2_shift(absmax)
            scales[f"{name}.{teil}"] = {"shift": schiebung, "scale": 2.0 ** (-schiebung),
                                        "absmax_observed": absmax}


def tabelle_exportieren(tabelle: Tabellenteile, name: str, ziel: Path, zeilen_soll: int,
                        block: int = 1 << 20) -> dict:
    """**Die N-Gramm-Tabelle in Bloecken**, int8 mit einer Skala je Zeile.

    ⚑ Je Zeile dieselbe Quantisierung wie jede andere Matrix
    (`quantize_symmetric_int8_per_channel`); Zeilen sind voneinander
    unabhaengig, also ist das Ergebnis dasselbe wie am Stueck. Gehasht wird
    beim Schreiben, damit 51 GB nicht ein zweites Mal gelesen werden.
    """
    if tabelle.zeilen != zeilen_soll:
        raise ValueError(f"{name}: {tabelle.zeilen} Zeilen im Checkpoint, die Masse sagen {zeilen_soll}")
    sicher = name.replace(".", "_")
    bin_pfad = ziel / f"{sicher}.bin"
    shifts_pfad = ziel / f"{sicher}_shifts.bin"
    h_daten, h_shifts = hashlib.sha256(), hashlib.sha256()
    with open(bin_pfad, "wb") as fd, open(shifts_pfad, "wb") as fs:
        for von in range(0, tabelle.zeilen, block):
            bis = min(tabelle.zeilen, von + block)
            q = quantize_symmetric_int8_per_channel(tabelle.zeilenblock(von, bis))
            roh = np.ascontiguousarray(q["int8"]).tobytes()
            sh = np.ascontiguousarray(q["shifts"]).astype("int8").tobytes()
            fd.write(roh)
            fs.write(sh)
            h_daten.update(roh)
            h_shifts.update(sh)
    return {
        "original_name": name,
        "file": bin_pfad.name,
        "shape": [tabelle.zeilen, tabelle.breite],
        "scale": -1.0,
        "shift": -1,
        "dtype": "int8",
        "shifts_file": shifts_pfad.name,
        "hash": h_daten.hexdigest(),
        "shifts_hash": h_shifts.hexdigest(),
    }


def bauen(model_dir: Path, export_cfg: dict, artifacts_dir: Path, sequenzen,
          geraet: str = "cpu", dtype=torch.float32, tokenizer_kopieren: bool = True):
    """**Kalibrieren und exportieren, Ebene fuer Ebene.**

    `sequenzen` sind Listen von Token-Kennungen; `export_cfg` kommt aus
    [`export_konfiguration`] oder aus `model_configs.py`.
    """
    _, m = _module_der_vorlage()
    artifacts_dir = Path(artifacts_dir)
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    lauf = Kalibrierlauf(model_dir, sequenzen, geraet, dtype)
    quelle, cfg_v = lauf.quelle, lauf.cfg
    versatz = float(export_cfg.get("rms_norm_offset", 0.0) or 0.0)
    sammler = ActivationStatsCollector()
    tabellen = []

    def strom():
        for i in range(export_cfg["num_layers"]):
            ebene, tabelle = ebene_laden(cfg_v, i, quelle, lauf.geraet, dtype)
            praefix = f"model.layers.{i}."
            sammler.attach(ebene, praefix=praefix.rstrip("."), vierstrom=True)
            lauf.ebene(ebene)
            sammler.detach()
            print(f"[ebenenweise] Ebene {i + 1}/{export_cfg['num_layers']} kalibriert", flush=True)
            if tabelle is not None:
                tabellen.append((f"{praefix}ple.ple_embedding.ngram_embedding.weight", tabelle,
                                 ebene.ple.ple_embedding))
            yield from quantisiere_parameter(
                ((praefix + n, p) for n, p in ebene.named_parameters()), versatz)
            del ebene
            gc.collect()
        # Der Strommischer am Ende.
        mischer = m.Qwen4ExpTextGatedResidual(cfg_v, use_combine=False)
        zustand = {kurz[len("hyper_connection_mixer."):]: quelle.tensor(voll, dtype)
                   for kurz, voll in quelle.mit_praefix("hyper_connection_mixer.")}
        mischer.load_state_dict(zustand, strict=True)
        mischer = mischer.to(lauf.geraet).eval()
        # ⚑ Der Praefix ist der Modulname selbst: `named_modules` liefert fuer
        #   das Modul den leeren Namen, fuer seine Teile `hc_norm` usw.
        sammler.attach(mischer, praefix="model.hyper_connection_mixer", vierstrom=True)
        lauf.strommischer(mischer)
        sammler.detach()
        yield from quantisiere_parameter(
            (("model.hyper_connection_mixer." + n, p) for n, p in mischer.named_parameters()), versatz)
        einbettung = quelle.tensor(quelle.voller_name("embed_tokens.weight"), dtype)
        yield "model.embed_tokens.weight", quantize_symmetric_int8_per_channel(einbettung)
        del einbettung
        # ⚑ **Der Kopf auch in int8**, wie im Korpusweg (`target_keys`
        #   enthaelt `lm_head`): Der Lader fuehrt ihn als Rueckfall neben dem
        #   int16-Kopf und verlangt ihn.
        kopf8 = quelle.tensor(quelle.voller_name("lm_head.weight"), dtype)
        yield "lm_head.weight", quantize_symmetric_int8_per_channel(kopf8)
        del kopf8

    export_quantized_weights(strom(), artifacts_dir)

    # Die N-Gramm-Tabellen, in Bloecken, und ihre Masse fuers Artefakt.
    manifest_pfad = artifacts_dir / "weights_manifest.json"
    manifest = json.loads(manifest_pfad.read_text(encoding="utf-8"))
    ple_masse = []
    for name, tabelle, modul in tabellen:
        zeilen = int(modul.ngram_heads_vocab_sizes.sum())
        teiler = export_cfg["make_ngram_vocab_size_divisible_by"]
        gepolstert = -(-zeilen // teiler) * teiler
        eintrag = tabelle_exportieren(tabelle, name, artifacts_dir, gepolstert)
        manifest[name.replace(".", "_")] = eintrag
        ple_masse.append({
            "multiplikatoren": [int(x) for x in modul.layer_multipliers.tolist()],
            "groessen": [int(x) for x in modul.ngram_heads_vocab_sizes.tolist()],
            "versaetze": [int(x) for x in modul.ngram_heads_offsets.tolist()],
        })
    manifest_pfad.write_text(json.dumps(manifest, sort_keys=True, separators=(",", ":")), encoding="utf-8")

    kopf = quelle.tensor(quelle.voller_name("lm_head.weight"), dtype)
    export_lm_head(quantize_symmetric_int16_per_channel(kopf), artifacts_dir)
    del kopf

    stats = sammler.compute()
    scales = compute_scales_from_stats(stats)
    _indexskalen_teilen(stats, scales, export_cfg)

    # model_config.json
    from .model_configs import _ARTIFACT_EXCLUDED_FIELDS
    model_config = {k: v for k, v in export_cfg.items() if k not in _ARTIFACT_EXCLUDED_FIELDS}
    from .model_configs import merkmale_ableiten
    model_config["merkmale"] = merkmale_ableiten(export_cfg)
    model_config["ple_puffer"] = ple_masse
    model_config["lm_head"] = {"dtype": "int16", "scale": "per_channel"}
    (artifacts_dir / "model_config.json").write_text(
        json.dumps(model_config, sort_keys=True, separators=(",", ":")))

    luts, lut_dtypes = luts_erzeugen(model_config)
    export_theta_v(scales=scales, luts=luts, output_dir=artifacts_dir, lut_dtypes=lut_dtypes)

    if tokenizer_kopieren:
        quelle_tok = Path(model_dir) / "tokenizer.json"
        if quelle_tok.is_file():
            shutil.copyfile(quelle_tok, artifacts_dir / "tokenizer.json")
    print(f"[ebenenweise] Fertig. Artefakt in {artifacts_dir}", flush=True)
    return lauf, stats, scales


# ============================================================================
# Der Aufruf
# ============================================================================

def konfiguration_gegen_snapshot(name: str, export_cfg: dict, model_dir: Path):
    """**Der Eintrag aus `model_configs.py` gegen den Snapshot**, Feld fuer
    Feld, bevor gerechnet wird.

    ⛔️ Ein Snapshot einer anderen Revision mit anderen Massen gaebe sonst ein
    Artefakt, dessen `model_config.json` nicht zu seinen Gewichten passt; der
    Lader merkte es erst an einer Form, und vielleicht gar nicht.
    """
    neu = export_konfiguration(text_konfiguration_lesen(model_dir), export_cfg["variant"],
                               export_cfg["verified"], export_cfg["hf_model_id"])
    ignoriert = {"verified", "hf_model_id", "variant", "betrieb"}
    abweichend = {k: (export_cfg.get(k), neu.get(k)) for k in set(export_cfg) | set(neu)
                  if k not in ignoriert and export_cfg.get(k) != neu.get(k)}
    if abweichend:
        zeilen = "\n".join(f"  {k}: Eintrag {a!r}, Snapshot {b!r}" for k, (a, b) in sorted(abweichend.items()))
        raise SystemExit(f"[ebenenweise] {name}: Eintrag und Snapshot ({model_dir}) weichen ab:\n{zeilen}")


def kalibriersequenzen(model_dir: Path, max_laenge: int):
    """Die Token des Kalibrierkorpus: dieselben Prompts und dieselbe
    WikiText-Stichprobe wie der Korpusweg (`main.py`)."""
    from tokenizers import Tokenizer
    from . import main as korpusweg
    tok = Tokenizer.from_file(str(Path(model_dir) / "tokenizer.json"))
    texte = list(korpusweg.KALIBRIER_PROMPTS) + korpusweg._wikitext_calibration_texts(
        korpusweg.CALIB_WIKITEXT_SEQUENCES)
    return [tok.encode(t, add_special_tokens=False).ids[:max_laenge] for t in texte]


def main():
    """`INTEGER_LLM_MODEL=<name> python -m calibrate.src.ebenenweise`

    Schalter ueber die Umgebung:
    - `INTEGER_LLM_EBENEN_PROBE=<n>`: der kurze Lauf vor dem langen. Nur die
      ersten `n` Ebenen, gemessen und gemeldet, **nichts geschrieben**.
    - `INTEGER_LLM_GERAET`: `cuda`, `mps` oder `cpu`; Vorgabe `cuda`, wenn es
      eine Karte gibt.
    """
    import time
    from .model_configs import get_export_model_config
    from .paths import local_model_dir, model_artifacts_dir
    name = os.environ.get("INTEGER_LLM_MODEL", "").strip()
    if not name:
        raise SystemExit("[ebenenweise] INTEGER_LLM_MODEL setzen (etwa myelith-125b-a6b)")
    export_cfg = get_export_model_config(name)
    if export_cfg.get("family") != FAMILIE:
        raise SystemExit(f"[ebenenweise] {name} ist keine Bauart {FAMILIE}; dafuer ist calibrate.src.main da")
    model_dir = local_model_dir(export_cfg["hf_model_id"].split("/")[-1])
    konfiguration_gegen_snapshot(name, export_cfg, model_dir)
    geraet = os.environ.get("INTEGER_LLM_GERAET") or ("cuda" if torch.cuda.is_available() else "cpu")
    from .main import CALIB_WIKITEXT_SEQ_LEN
    sequenzen = kalibriersequenzen(model_dir, CALIB_WIKITEXT_SEQ_LEN)
    token = sum(len(s) for s in sequenzen)
    print(f"[ebenenweise] {name}: {len(sequenzen)} Sequenzen, {token} Token, Geraet {geraet}", flush=True)

    probe = int(os.environ.get("INTEGER_LLM_EBENEN_PROBE", "0") or 0)
    if probe:
        start = time.monotonic()
        lauf = Kalibrierlauf(model_dir, sequenzen, geraet)
        sammler = ActivationStatsCollector()
        for i in range(min(probe, export_cfg["num_layers"])):
            t0 = time.monotonic()
            ebene, _ = ebene_laden(lauf.cfg, i, lauf.quelle, lauf.geraet, torch.float32)
            sammler.attach(ebene, praefix=f"model.layers.{i}", vierstrom=True)
            lauf.ebene(ebene)
            sammler.detach()
            groesst = max(float(x.abs().max()) for x in lauf.zustand)
            print(f"[ebenenweise] PROBE Ebene {i}: {time.monotonic() - t0:.1f} s, "
                  f"groesster Zustand {groesst:.3g}", flush=True)
            del ebene
            gc.collect()
        print(f"[ebenenweise] PROBE: {probe} Ebenen in {time.monotonic() - start:.1f} s, "
              f"{len(sammler.compute())} Skalen gemessen, nichts geschrieben", flush=True)
        print("Fertig")
        return
    ziel = model_artifacts_dir(name)
    if ziel.exists() and any(ziel.iterdir()):
        raise SystemExit(f"[ebenenweise] {ziel} ist nicht leer; hier wird nichts ueberschrieben")
    bauen(model_dir, export_cfg, ziel, sequenzen, geraet=geraet)
    print("Fertig")


if __name__ == "__main__":
    main()
